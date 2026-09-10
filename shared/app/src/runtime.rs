use crate::{files, platform, secrets};
use lightledger_core::LedgerService;
use lightledger_sync::{
    crypto::{Identity, Space},
    dav::WebDav,
    pairing,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Serialize, Deserialize)]
pub(crate) struct SyncConfig {
    pub(crate) url: String,
    pub(crate) username: String,
    pub(crate) password: String,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct Vault {
    pub(crate) identity: Identity,
    pub(crate) spaces: BTreeMap<String, Space>,
    pub(crate) used: BTreeSet<String>,
    pub(crate) config: Option<SyncConfig>,
    #[serde(default)]
    pub(crate) controls: BTreeMap<String, pairing::Grant>,
    #[serde(default)]
    pub(crate) device_settings: Value,
}
pub(crate) struct Runtime {
    pub(crate) ledger: Arc<Mutex<LedgerService>>,
    pub(crate) vault: Vault,
    pub(crate) dir: PathBuf,
    pub(crate) last_sync: Value,
    pub(crate) paused: bool,
    pub(crate) paused_spaces: BTreeSet<String>,
}
impl Runtime {
    fn refresh_trust(&self) -> Result<(), String> {
        for space in self.vault.spaces.values() {
            lightledger_sync::engine::update_trust(space, |a, p| {
                self.ledger
                    .lock()
                    .map_err(|_| "数据库不可用")?
                    .dispatch(a, p)
            })?;
        }
        Ok(())
    }
    pub(crate) fn save(&self) -> Result<(), String> {
        secrets::write(
            &self.dir.join(secrets::VAULT_FILE),
            &serde_json::to_vec(&self.vault).map_err(|e| e.to_string())?,
        )
    }
    pub(crate) fn system(&mut self, action: &str, p: Value) -> Result<Value, String> {
        let text = |key: &str| p[key].as_str().unwrap_or("");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        match action {
            "capabilities" => Ok(platform::capabilities()),
            "deviceSettings" => Ok(self.vault.device_settings.clone()),
            "saveDeviceSettings" => {
                self.vault.device_settings = json!({"appLockEnabled":p["appLockEnabled"].as_bool().unwrap_or(false),"notificationSources":p["notificationSources"].as_array().cloned().unwrap_or_default()});
                self.save()?;
                Ok(self.vault.device_settings.clone())
            }
            "ocrImage" => {
                #[cfg(target_os = "android")]
                return platform::dispatch(action, &p);
                #[cfg(windows)]
                {
                    let path = PathBuf::from(text("path"));
                    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 20 * 1024 * 1024
                    {
                        return Err("图片不能超过20MB".into());
                    }
                    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                    let mut result = platform::dispatch(action, &p)?;
                    result["captureId"] = json!(format!(
                        "image:{}",
                        lightledger_sync::crypto::digest(&bytes)
                    ));
                    Ok(result)
                }
            }
            "readFile" => files::read_text(text("path")),
            "saveFile" => {
                files::write_text(text("path"), text("text"))?;
                Ok(json!({"saved":true}))
            }
            "probeWebDav" => {
                WebDav::new(text("url"), text("username"), text("password"))?.probe()?;
                Ok(json!({"ok":true}))
            }
            "syncConfigure" => {
                let config: SyncConfig = serde_json::from_value(p).map_err(|e| e.to_string())?;
                WebDav::new(&config.url, &config.username, &config.password)?.probe()?;
                self.vault.config = Some(config);
                self.paused = false;
                self.paused_spaces.clear();
                self.refresh_trust()?;
                self.save()?;
                Ok(json!({"configured":true}))
            }
            "syncStatus" => Ok(
                json!({"configured":self.vault.config.is_some(),"paused":self.paused,"lastSync":self.last_sync,"deviceId":self.vault.identity.device_id,"memberId":self.vault.identity.member_id,"fingerprint":lightledger_sync::crypto::fingerprint(&self.vault.identity.public_key()?)?,"spaces":self.vault.spaces.values().map(|s|json!({"id":s.id,"kind":s.kind,"epoch":s.epoch,"devices":s.devices})).collect::<Vec<_>>()}),
            ),
            "pairingRequest" => {
                let mut request = pairing::request(&self.vault.identity, now)?;
                if !text("memberId").is_empty() {
                    request.member_id = text("memberId").into();
                }
                Ok(serde_json::to_value(request).map_err(|e| e.to_string())?)
            }
            "createSharedSpace" => {
                let id = self
                    .ledger
                    .lock()
                    .map_err(|_| "数据库不可用")?
                    .dispatch("syncIdentity", json!({}))?["sharedSpaceId"]
                    .as_str()
                    .ok_or("缺少共同空间")?
                    .to_string();
                if self.vault.spaces.contains_key(&id) {
                    return Ok(json!({"spaceId":id}));
                }
                let space = Space::new(id.clone(), "shared".into(), &self.vault.identity)?;
                self.vault.spaces.insert(id.clone(), space);
                self.refresh_trust()?;
                self.save()?;
                Ok(json!({"spaceId":id}))
            }
            "pairingApprove" => {
                let request =
                    serde_json::from_value(p["request"].clone()).map_err(|e| e.to_string())?;
                let grant = pairing::approve(
                    &request,
                    &self.vault.identity,
                    self.vault
                        .spaces
                        .get_mut(text("spaceId"))
                        .ok_or("空间不存在")?,
                    now,
                    text("fingerprint"),
                    &mut self.vault.used,
                )?;
                let space = &self.vault.spaces[text("spaceId")];
                let control = pairing::control(space, &self.vault.identity)?;
                self.vault.controls.insert(
                    format!(
                        "LightLedger/v1/spaces/{}/control/{:020}.json",
                        space.id, space.revision
                    ),
                    control,
                );
                self.refresh_trust()?;
                self.save()?;
                Ok(serde_json::to_value(grant).map_err(|e| e.to_string())?)
            }
            "pairingAccept" => {
                let grant =
                    serde_json::from_value(p["grant"].clone()).map_err(|e| e.to_string())?;
                let mut used = self.vault.used.clone();
                let space = pairing::accept(
                    &grant,
                    &self.vault.identity,
                    text("fingerprint"),
                    now,
                    &mut used,
                )?;
                let id = space.id.clone();
                if space.kind == "personal" {
                    self.ledger.lock().map_err(|_| "数据库不可用")?.dispatch(
                        "adoptIdentity",
                        json!({"memberId":space.owner_member_id,"personalSpaceId":space.id}),
                    )?;
                    self.vault.identity.member_id = space.owner_member_id.clone();
                    self.vault.spaces.retain(|_, s| s.kind != "personal");
                }
                if space.kind == "shared" {
                    self.ledger
                        .lock()
                        .map_err(|_| "数据库不可用")?
                        .dispatch("adoptSharedSpace", json!({"sharedSpaceId":id}))?;
                }
                self.vault.used = used;
                self.vault.spaces.insert(id.clone(), space);
                self.refresh_trust()?;
                self.save()?;
                Ok(json!({"spaceId":id}))
            }
            "revokeDevice" => {
                let grant = pairing::rotate(
                    self.vault
                        .spaces
                        .get_mut(text("spaceId"))
                        .ok_or("空间不存在")?,
                    &self.vault.identity,
                    text("deviceId"),
                )?;
                let space = &self.vault.spaces[text("spaceId")];
                let control = pairing::control(space, &self.vault.identity)?;
                self.vault.controls.insert(
                    format!(
                        "LightLedger/v1/spaces/{}/control/{:020}.json",
                        space.id, space.revision
                    ),
                    control,
                );
                self.refresh_trust()?;
                self.save()?;
                Ok(serde_json::to_value(grant).map_err(|e| e.to_string())?)
            }
            "applyRotation" => {
                let grant =
                    serde_json::from_value(p["grant"].clone()).map_err(|e| e.to_string())?;
                pairing::apply_rotation(
                    self.vault
                        .spaces
                        .get_mut(text("spaceId"))
                        .ok_or("空间不存在")?,
                    &self.vault.identity,
                    &grant,
                )?;
                self.refresh_trust()?;
                self.save()?;
                Ok(json!({"rotated":true}))
            }
            "syncRun" | "syncTick" => {
                if action == "syncRun" {
                    self.paused_spaces.clear();
                }
                let config = self.vault.config.as_ref().ok_or("请先配置WebDAV")?;
                let dav = WebDav::new(&config.url, &config.username, &config.password)?;
                let mut results = Vec::new();
                for space in self.vault.spaces.values_mut() {
                    if self.paused_spaces.contains(&space.id) {
                        continue;
                    }
                    let outcome: Result<Value, String> = (|| {
                        lightledger_sync::control::exchange(
                            &dav,
                            &self.vault.identity,
                            space,
                            &self.vault.controls,
                        )?;
                        lightledger_sync::engine::run(
                            &dav,
                            &self.vault.identity,
                            space,
                            &self.dir.join("sync-cache"),
                            |a, p| {
                                self.ledger
                                    .lock()
                                    .map_err(|_| "数据库不可用")?
                                    .dispatch(a, p)
                            },
                        )
                    })();
                    match outcome {
                        Ok(value) => {
                            if value["errors"].as_array().is_some_and(|errors| {
                                errors.iter().any(|entry| {
                                    let error = entry["error"].as_str().unwrap_or("");
                                    !error.contains("等待缺失操作")
                                        && !error.contains("网络错误或超时")
                                })
                            }) {
                                self.paused_spaces.insert(space.id.clone());
                            }
                            results.push(json!({"spaceId":space.id,"result":value}));
                        }
                        Err(error) => {
                            if !error.contains("网络错误或超时") {
                                self.paused_spaces.insert(space.id.clone());
                            }
                            results.push(json!({"spaceId":space.id,"error":error}));
                        }
                    }
                }
                self.paused = self
                    .vault
                    .spaces
                    .keys()
                    .all(|id| self.paused_spaces.contains(id));
                self.last_sync =
                    json!({"at":now,"results":results,"pausedSpaces":self.paused_spaces});
                self.refresh_trust()?;
                self.save()?;
                Ok(self.last_sync.clone())
            }
            _ => platform::dispatch(action, &p),
        }
    }
}
