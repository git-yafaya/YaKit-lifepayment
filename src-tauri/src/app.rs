use crate::{
    platform,
    runtime::{Runtime, Vault},
    secrets,
};
use lightledger_core::LedgerService;
use lightledger_sync::crypto::{Identity, Space};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use tauri::Manager;
struct Shared {
    ledger: Arc<Mutex<LedgerService>>,
    runtime: Arc<Mutex<Runtime>>,
}
#[tauri::command]
async fn ledger_command(
    state: tauri::State<'_, Shared>,
    action: String,
    payload: Value,
) -> Result<Value, String> {
    if matches!(
        action.as_str(),
        "applyOperation"
            | "markUploaded"
            | "registerTrustedMember"
            | "revokeTrustedMember"
            | "adoptIdentity"
            | "adoptSharedSpace"
    ) {
        return Err("该操作仅允许通过已认证同步执行".into());
    }
    if action == "restoreBackup" {
        let runtime = state.runtime.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let mut runtime = runtime.lock().map_err(|_| "同步状态不可用")?;
            let result = runtime
                .ledger
                .lock()
                .map_err(|_| "数据库不可用")?
                .dispatch(&action, payload)?;
            let ids = runtime
                .ledger
                .lock()
                .map_err(|_| "数据库不可用")?
                .dispatch("syncIdentity", json!({}))?;
            let identity = Identity::create(
                ids["memberId"].as_str().ok_or("成员身份缺失")?.into(),
                ids["deviceId"].as_str().ok_or("设备身份缺失")?.into(),
            )?;
            let space = Space::new(
                ids["personalSpaceId"]
                    .as_str()
                    .ok_or("个人空间缺失")?
                    .into(),
                "personal".into(),
                &identity,
            )?;
            runtime.vault = Vault {
                identity,
                spaces: BTreeMap::from([(space.id.clone(), space)]),
                used: BTreeSet::new(),
                config: None,
                controls: BTreeMap::new(),
                device_settings: json!({"appLockEnabled":false,"notificationSources":[]}),
            };
            runtime.paused = false;
            runtime.paused_spaces.clear();
            runtime.last_sync = Value::Null;
            runtime.save()?;
            let cache = runtime.dir.join("sync-cache");
            if cache.exists() {
                std::fs::remove_dir_all(cache).map_err(|e| e.to_string())?;
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let shared = state.ledger.clone();
    tauri::async_runtime::spawn_blocking(move || {
        shared
            .lock()
            .map_err(|_| "数据库工作线程不可用")?
            .dispatch(&action, payload)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn system_command(
    window: tauri::Window,
    state: tauri::State<'_, Shared>,
    action: String,
    payload: Value,
) -> Result<Value, String> {
    if action == "notificationRequestAccess" || action == "helloVerify" {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        window
            .run_on_main_thread(move || {
                let _ = tx.send(platform::dispatch_ui(&action, &payload, hwnd));
            })
            .map_err(|e| e.to_string())?;
        return tauri::async_runtime::spawn_blocking(move || {
            rx.recv().map_err(|e| e.to_string())??()
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let shared = state.runtime.clone();
    tauri::async_runtime::spawn_blocking(move || {
        shared
            .lock()
            .map_err(|_| "后台工作线程不可用")?
            .system(&action, payload)
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let key_path = dir.join("database.dpapi");
            let key = if key_path.exists() {
                secrets::read(&key_path)?
            } else {
                let key = lightledger_sync::crypto::random_key();
                secrets::write(&key_path, &key)?;
                key
            };
            let mut ledger = LedgerService::open(&dir.join("ledger.db"), &key)?;
            let vault_path = dir.join("sync.dpapi");
            let vault = if vault_path.exists() {
                serde_json::from_slice(&secrets::read(&vault_path)?)?
            } else {
                let ids = ledger.dispatch("syncIdentity", json!({}))?;
                let identity = Identity::create(
                    ids["memberId"].as_str().ok_or("缺少成员身份")?.into(),
                    ids["deviceId"].as_str().ok_or("缺少设备身份")?.into(),
                )?;
                let space = Space::new(
                    ids["personalSpaceId"]
                        .as_str()
                        .ok_or("缺少个人空间")?
                        .into(),
                    "personal".into(),
                    &identity,
                )?;
                Vault {
                    identity,
                    spaces: BTreeMap::from([(space.id.clone(), space)]),
                    used: BTreeSet::new(),
                    config: None,
                    controls: BTreeMap::new(),
                    device_settings: json!({"appLockEnabled":false,"notificationSources":[]}),
                }
            };
            let ledger = Arc::new(Mutex::new(ledger));
            let runtime = Runtime {
                ledger: ledger.clone(),
                vault,
                dir,
                last_sync: Value::Null,
                paused: false,
                paused_spaces: BTreeSet::new(),
            };
            runtime.save()?;
            let runtime = Arc::new(Mutex::new(runtime));
            let background = runtime.clone();
            std::thread::spawn(move || {
                loop {
                    if let Ok(mut runtime) = background.try_lock() {
                        if runtime.vault.config.is_some() && !runtime.paused {
                            let _ = runtime.system("syncTick", json!({}));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(60));
                }
            });
            app.manage(Shared { ledger, runtime });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ledger_command, system_command])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        let title = "轻账启动失败\0".encode_utf16().collect::<Vec<_>>();
        let message =
            format!("无法打开轻账：{error}\n本地数据已保留。请检查 Windows 用户与数据目录。\0")
                .encode_utf16()
                .collect::<Vec<_>>();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}
