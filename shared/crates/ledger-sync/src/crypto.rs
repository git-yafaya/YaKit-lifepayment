use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::{RngCore, rngs::OsRng};
use rsa::{
    Oaep, Pss, RsaPrivateKey, RsaPublicKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
pub type Result<T> = std::result::Result<T, String>;
pub fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}
pub fn unb64(text: &str) -> Result<Vec<u8>> {
    STANDARD.decode(text).map_err(|_| "无效编码".into())
}
pub fn random_key() -> Vec<u8> {
    let mut k = vec![0; 32];
    OsRng.fill_bytes(&mut k);
    k
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub member_id: String,
    pub device_id: String,
    pub private_key: String,
}
impl Identity {
    pub fn create(member_id: String, device_id: String) -> Result<Self> {
        let k = RsaPrivateKey::new(&mut OsRng, 3072).map_err(|e| e.to_string())?;
        Ok(Self {
            member_id,
            device_id,
            private_key: b64(k.to_pkcs8_der().map_err(|e| e.to_string())?.as_bytes()),
        })
    }
    fn private(&self) -> Result<RsaPrivateKey> {
        RsaPrivateKey::from_pkcs8_der(&unb64(&self.private_key)?)
            .map_err(|_| "设备密钥不可用".into())
    }
    pub fn public_key(&self) -> Result<String> {
        Ok(b64(self
            .private()?
            .to_public_key()
            .to_public_key_der()
            .map_err(|e| e.to_string())?
            .as_bytes()))
    }
    pub fn sign(&self, bytes: &[u8]) -> Result<String> {
        Ok(b64(&self
            .private()?
            .sign_with_rng(&mut OsRng, Pss::new::<Sha256>(), &Sha256::digest(bytes))
            .map_err(|e| e.to_string())?))
    }
    pub fn unwrap(&self, value: &str) -> Result<Vec<u8>> {
        self.private()?
            .decrypt(Oaep::new::<Sha256>(), &unb64(value)?)
            .map_err(|_| "密钥封装认证失败".into())
    }
}
pub fn digest(bytes: &[u8]) -> String {
    b64(&Sha256::digest(bytes))
}
pub fn fingerprint(public: &str) -> Result<String> {
    Ok(b64(&Sha256::digest(unb64(public)?)))
}
pub fn verify(public: &str, bytes: &[u8], signature: &str) -> Result<()> {
    RsaPublicKey::from_public_key_der(&unb64(public)?)
        .map_err(|_| "无效设备公钥".to_string())?
        .verify(
            Pss::new::<Sha256>(),
            &Sha256::digest(bytes),
            &unb64(signature)?,
        )
        .map_err(|_| "设备签名不匹配".into())
}
pub fn wrap(public: &str, key: &[u8]) -> Result<String> {
    Ok(b64(&RsaPublicKey::from_public_key_der(&unb64(public)?)
        .map_err(|e| e.to_string())?
        .encrypt(&mut OsRng, Oaep::new::<Sha256>(), key)
        .map_err(|e| e.to_string())?))
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub member_id: String,
    pub public_key: String,
    pub revoked: bool,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub kind: String,
    pub owner_member_id: String,
    pub admin_device_id: String,
    pub epoch: u64,
    pub revision: u64,
    pub keys: BTreeMap<u64, String>,
    pub devices: BTreeMap<String, Device>,
}
impl Space {
    pub fn new(id: String, kind: String, identity: &Identity) -> Result<Self> {
        if kind != "personal" && kind != "shared" {
            return Err("未知空间类型".into());
        }
        Ok(Self {
            id,
            kind,
            owner_member_id: identity.member_id.clone(),
            admin_device_id: identity.device_id.clone(),
            epoch: 1,
            revision: 1,
            keys: BTreeMap::from([(1, b64(&random_key()))]),
            devices: BTreeMap::from([(
                identity.device_id.clone(),
                Device {
                    member_id: identity.member_id.clone(),
                    public_key: identity.public_key()?,
                    revoked: false,
                },
            )]),
        })
    }
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub version: u32,
    pub space_id: String,
    pub device_id: String,
    pub member_id: String,
    pub sequence: u64,
    pub operation_id: String,
    pub epoch: u64,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub header: String,
    pub nonce: String,
    pub ciphertext: String,
    pub signature: String,
}
// 签名直接覆盖二进制头、随机数和密文；头的原始字节同时作为 GCM 附加认证数据。
fn signed(header: &[u8], nonce: &[u8], cipher: &[u8]) -> Vec<u8> {
    let mut out = b"LightLedger/operation/v1\0".to_vec();
    out.extend((header.len() as u32).to_be_bytes());
    out.extend(header);
    out.extend(nonce);
    out.extend(cipher);
    out
}
pub fn seal(
    space: &Space,
    identity: &Identity,
    sequence: u64,
    operation_id: &str,
    value: &serde_json::Value,
) -> Result<Envelope> {
    let device = space.devices.get(&identity.device_id).ok_or("设备未批准")?;
    if device.revoked || device.public_key != identity.public_key()? {
        return Err("设备已撤销".into());
    }
    let h = Header {
        version: 1,
        space_id: space.id.clone(),
        device_id: identity.device_id.clone(),
        member_id: identity.member_id.clone(),
        sequence,
        operation_id: operation_id.into(),
        epoch: space.epoch,
    };
    let header = serde_json::to_vec(&h).map_err(|e| e.to_string())?;
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    let key = unb64(space.keys.get(&space.epoch).ok_or("缺少空间密钥")?)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "无效空间密钥")?
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &serde_json::to_vec(value).map_err(|e| e.to_string())?,
                aad: &header,
            },
        )
        .map_err(|_| "加密失败")?;
    Ok(Envelope {
        header: b64(&header),
        nonce: b64(&nonce),
        signature: identity.sign(&signed(&header, &nonce, &cipher))?,
        ciphertext: b64(&cipher),
    })
}
pub fn open(space: &Space, e: &Envelope) -> Result<(Header, serde_json::Value)> {
    let header = unb64(&e.header)?;
    let h: Header = serde_json::from_slice(&header).map_err(|_| "无效操作头")?;
    if h.version != 1 {
        return Err("unsupported-version".into());
    }
    if h.space_id != space.id || h.sequence == 0 {
        return Err("操作空间或序号不匹配".into());
    }
    let d = space.devices.get(&h.device_id).ok_or("设备未批准")?;
    if d.revoked || d.member_id != h.member_id {
        return Err("设备已撤销或成员不匹配".into());
    }
    let nonce = unb64(&e.nonce)?;
    if nonce.len() != 12 {
        return Err("无效随机数".into());
    }
    let cipher = unb64(&e.ciphertext)?;
    verify(
        &d.public_key,
        &signed(&header, &nonce, &cipher),
        &e.signature,
    )?;
    let key = unb64(space.keys.get(&h.epoch).ok_or("缺少密钥版本")?)?;
    let plain = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "无效密钥")?
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &cipher,
                aad: &header,
            },
        )
        .map_err(|_| "密文完整性校验失败")?;
    Ok((
        h,
        serde_json::from_slice(&plain).map_err(|_| "无效操作内容")?,
    ))
}
