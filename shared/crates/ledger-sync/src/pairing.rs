use crate::crypto::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub version: u32,
    pub id: String,
    pub expires: u64,
    pub member_id: String,
    pub device_id: String,
    pub public_key: String,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    pub body: String,
    pub signature: String,
    pub signer_public_key: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantBody {
    version: u32,
    invitation_id: String,
    expires: u64,
    recipient: String,
    space: Space,
    wrapped_keys: BTreeMap<u64, String>,
}
pub fn request(identity: &Identity, now: u64) -> Result<Request> {
    Ok(Request {
        version: 1,
        id: uuid::Uuid::new_v4().to_string(),
        expires: now + 600,
        member_id: identity.member_id.clone(),
        device_id: identity.device_id.clone(),
        public_key: identity.public_key()?,
    })
}
pub fn approve(
    request: &Request,
    identity: &Identity,
    space: &mut Space,
    now: u64,
    confirmed_fingerprint: &str,
    used: &mut BTreeSet<String>,
) -> Result<Grant> {
    if request.version != 1 || now > request.expires || used.contains(&request.id) {
        return Err("邀请过期、重放或版本不兼容".into());
    }
    if identity.device_id != space.admin_device_id {
        return Err("仅空间管理员可授权".into());
    }
    if fingerprint(&request.public_key)? != confirmed_fingerprint {
        return Err("请核对设备指纹".into());
    }
    if space.kind == "personal" && request.member_id != space.owner_member_id {
        return Err("个人空间只允许同一成员".into());
    }
    let mut exported = space.clone();
    exported.revision += 1;
    exported.devices.insert(
        request.device_id.clone(),
        Device {
            member_id: request.member_id.clone(),
            public_key: request.public_key.clone(),
            revoked: false,
        },
    );
    let wrapped_keys = exported
        .keys
        .iter()
        .map(|(epoch, key)| Ok((*epoch, wrap(&request.public_key, &unb64(key)?)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    exported.keys.clear();
    let body = b64(&serde_json::to_vec(&GrantBody {
        version: 1,
        invitation_id: request.id.clone(),
        expires: request.expires,
        recipient: request.device_id.clone(),
        space: exported.clone(),
        wrapped_keys,
    })
    .map_err(|e| e.to_string())?);
    let grant = Grant {
        signature: identity.sign(body.as_bytes())?,
        body,
        signer_public_key: identity.public_key()?,
    };
    space.devices = exported.devices;
    space.revision = exported.revision;
    used.insert(request.id.clone());
    Ok(grant)
}
pub fn accept(
    grant: &Grant,
    identity: &Identity,
    expected_fingerprint: &str,
    now: u64,
    used: &mut BTreeSet<String>,
) -> Result<Space> {
    if fingerprint(&grant.signer_public_key)? != expected_fingerprint {
        return Err("可信设备指纹不匹配".into());
    }
    verify(
        &grant.signer_public_key,
        grant.body.as_bytes(),
        &grant.signature,
    )?;
    let mut g: GrantBody =
        serde_json::from_slice(&unb64(&grant.body)?).map_err(|_| "邀请格式无效")?;
    if g.version != 1
        || now > g.expires
        || g.recipient != identity.device_id
        || used.contains(&g.invitation_id)
    {
        return Err("邀请过期、重放或接收设备不匹配".into());
    }
    if g.space
        .devices
        .get(&g.space.admin_device_id)
        .map(|d| &d.public_key)
        != Some(&grant.signer_public_key)
    {
        return Err("签名者不是空间管理员".into());
    }

    for (epoch, key) in g.wrapped_keys {
        g.space.keys.insert(epoch, b64(&identity.unwrap(&key)?));
    }
    used.insert(g.invitation_id);
    Ok(g.space)
}
// 撤销后只为仍获准的设备封装新密钥；已读取的数据无法收回。
pub fn rotate(space: &mut Space, identity: &Identity, revoke_device: &str) -> Result<Grant> {
    if identity.device_id != space.admin_device_id || revoke_device == identity.device_id {
        return Err("需要有效管理员且不能撤销自身".into());
    }
    space
        .devices
        .get_mut(revoke_device)
        .ok_or("未知设备")?
        .revoked = true;
    space.epoch += 1;
    space.keys.insert(space.epoch, b64(&random_key()));
    space.revision += 1;
    control(space, identity)
}
pub fn control(space: &Space, identity: &Identity) -> Result<Grant> {
    if identity.device_id != space.admin_device_id {
        return Err("仅管理员可发布授权".into());
    }
    let mut metadata = space.clone();
    metadata.keys.clear();
    let keys = space
        .devices
        .iter()
        .filter(|(_, d)| !d.revoked)
        .map(|(id, d)| {
            Ok((
                id.clone(),
                wrap(&d.public_key, &unb64(&space.keys[&space.epoch])?)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let body = b64(&serde_json::to_vec(
        &serde_json::json!({"version":1,"space":metadata,"wrappedKeys":keys}),
    )
    .map_err(|e| e.to_string())?);
    Ok(Grant {
        signature: identity.sign(body.as_bytes())?,
        body,
        signer_public_key: identity.public_key()?,
    })
}
pub fn apply_rotation(space: &mut Space, identity: &Identity, grant: &Grant) -> Result<()> {
    let admin = space
        .devices
        .get(&space.admin_device_id)
        .ok_or("缺少管理员")?;
    if admin.public_key != grant.signer_public_key {
        return Err("轮换管理员不匹配".into());
    }
    verify(&admin.public_key, grant.body.as_bytes(), &grant.signature)?;
    let value: serde_json::Value =
        serde_json::from_slice(&unb64(&grant.body)?).map_err(|e| e.to_string())?;
    if value["version"] != 1 {
        return Err("unsupported-version".into());
    }
    let mut next: Space =
        serde_json::from_value(value["space"].clone()).map_err(|e| e.to_string())?;
    if next.id != space.id
        || next.revision != space.revision + 1
        || next.epoch < space.epoch
        || next.epoch > space.epoch + 1
        || next.admin_device_id != space.admin_device_id
    {
        return Err("轮换顺序或空间不匹配".into());
    }
    let key = value["wrappedKeys"][&identity.device_id]
        .as_str()
        .ok_or("本设备已撤销")?;
    next.keys = space.keys.clone();
    next.keys.insert(next.epoch, b64(&identity.unwrap(key)?));
    *space = next;
    Ok(())
}
