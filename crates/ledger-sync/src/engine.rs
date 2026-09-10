use crate::{
    crypto::{self, Envelope, Identity, Result, Space},
    dav::WebDav,
};
use serde_json::{Value, json};
use std::{fs, path::Path};
pub fn run(
    dav: &WebDav,
    identity: &Identity,
    space: &Space,
    cache: &Path,
    mut dispatch: impl FnMut(&str, Value) -> Result<Value>,
) -> Result<Value> {
    update_trust(space, &mut dispatch)?;
    fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let root = format!("LightLedger/v1/spaces/{}/ops", space.id);
    let local = format!("{}/{}/", root, identity.device_id);
    dav.ensure_collection(&local)?;
    let pending = dispatch("pendingUploads", json!({}))?;
    let known = dav.list(&local)?;
    for entry in fs::read_dir(cache).map_err(|e| e.to_string())? {
        let file = entry.map_err(|e| e.to_string())?.path();
        if file.extension().and_then(|s| s.to_str()) != Some("enc") {
            continue;
        }
        let envelope: Envelope =
            serde_json::from_slice(&fs::read(&file).map_err(|e| e.to_string())?)
                .map_err(|_| "本地同步缓存损坏")?;
        let header: crypto::Header = serde_json::from_slice(&crypto::unb64(&envelope.header)?)
            .map_err(|_| "本地同步缓存头损坏")?;
        if header.space_id == space.id
            && header.device_id == identity.device_id
            && !pending
                .as_array()
                .ok_or("发件箱格式错误")?
                .iter()
                .any(|p| p["id"] == header.operation_id)
            && !known.contains(&format!("{local}{:020}.enc", header.sequence))
        {
            return Err("已上传的远端操作丢失，请检查服务器或备份；本地账单已保留".into());
        }
    }
    let mut uploaded = 0;
    let mut applied = 0;
    let mut errors = Vec::new();
    for op in pending
        .as_array()
        .ok_or("发件箱格式错误")?
        .iter()
        .filter(|v| v["spaceId"].as_str() == Some(&space.id))
    {
        let id = op["id"].as_str().ok_or("缺少操作ID")?;
        uuid::Uuid::parse_str(id).map_err(|_| "无效操作ID")?;
        let seq = op["sequence"].as_u64().ok_or("缺少连续序号")?;
        let file = cache.join(format!("{id}.enc"));
        // 首次加密后保存完整信封，网络失败重试仍发送相同字节。
        let bytes = if file.exists() {
            fs::read(&file).map_err(|e| e.to_string())?
        } else {
            let bytes = serde_json::to_vec(&crypto::seal(space, identity, seq, id, op)?)
                .map_err(|e| e.to_string())?;
            let temp = file.with_extension("pending");
            fs::write(&temp, &bytes).map_err(|e| e.to_string())?;
            fs::rename(&temp, &file).map_err(|e| e.to_string())?;
            bytes
        };
        dav.put_immutable(&format!("{local}{seq:020}.enc"), &bytes)?;
        dispatch("markUploaded", json!({"id":id}))?;
        uploaded += 1;
    }
    for (device, authorized) in &space.devices {
        if authorized.revoked || device == &identity.device_id {
            continue;
        }
        let path = format!("{root}/{device}/");
        let files = match dav.list(&path) {
            Ok(v) => v,
            Err(e) if e.contains("404") => continue,
            Err(e) => {
                errors.push(json!({"deviceId":device,"error":e}));
                continue;
            }
        };
        let mut available = std::collections::BTreeMap::new();
        for file in files {
            if let Some(name) = file
                .strip_prefix(&path)
                .and_then(|s| s.strip_suffix(".enc"))
            {
                if let Ok(seq) = name.parse::<u64>() {
                    available.insert(seq, file);
                }
            }
        }
        let mut cursor =
            dispatch("syncCursor", json!({"spaceId":space.id,"deviceId":device}))?["sequence"]
                .as_u64()
                .unwrap_or(0);
        while let Some(path) = available.get(&(cursor + 1)) {
            let sequence = cursor + 1;
            let bytes = dav.get(path)?;
            let incoming = cache.join(format!("{}-{device}-{sequence}.incoming", space.id));
            fs::write(&incoming, &bytes).map_err(|e| e.to_string())?;
            let result: Result<()> = (|| {
                let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|_| "无效信封")?;
                let (header, value) = crypto::open(space, &envelope)?;
                if header.device_id != *device
                    || header.sequence != sequence
                    || value["id"] != header.operation_id
                    || value["spaceId"] != header.space_id
                    || value["memberId"] != header.member_id
                    || value["deviceId"] != header.device_id
                    || value["sequence"] != header.sequence
                {
                    return Err("操作与认证头不匹配".into());
                }
                dispatch("applyOperation", value)?;
                Ok(())
            })();
            if let Err(error) = result {
                if error == "unsupported-version" {
                    return Err(error);
                }
                errors.push(json!({"deviceId":device,"sequence":sequence,"error":error}));
                break;
            }
            cursor = sequence;
            applied += 1;
            let _ = fs::remove_file(incoming);
        }
        if available.keys().any(|n| *n > cursor + 1) {
            errors.push(json!({"deviceId":device,"error":"等待缺失操作","nextSequence":cursor+1}));
        }
    }
    Ok(json!({"uploaded":uploaded,"applied":applied,"errors":errors}))
}

pub fn update_trust(
    space: &Space,
    mut dispatch: impl FnMut(&str, Value) -> Result<Value>,
) -> Result<()> {
    if space.kind != "shared" {
        return Ok(());
    }
    let members = space
        .devices
        .values()
        .map(|d| d.member_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    for member in members {
        let active = space
            .devices
            .values()
            .any(|d| d.member_id == member && !d.revoked);
        dispatch(
            if active {
                "registerTrustedMember"
            } else {
                "revokeTrustedMember"
            },
            json!({"memberId":member,"sharedSpaceId":space.id}),
        )?;
    }
    Ok(())
}
