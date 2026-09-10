#![cfg(target_os = "android")]
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tauri::{
    Wry,
    plugin::{Builder, PluginHandle, TauriPlugin},
};

static DEVICE: OnceLock<PluginHandle<Wry>> = OnceLock::new();

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("device")
        .setup(|_, api| {
            let handle =
                api.register_android_plugin("com.yakit.lightledger.device", "DevicePlugin")?;
            DEVICE
                .set(handle)
                .map_err(|_| "Android 设备服务重复初始化")?;
            Ok(())
        })
        .build()
}

fn call(command: &str, payload: Value) -> Result<Value, String> {
    DEVICE
        .get()
        .ok_or("Android 设备服务尚未初始化")?
        .run_mobile_plugin(command, payload)
        .map_err(|e| e.to_string())
}

pub fn protect(bytes: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    let result = call(
        "protect",
        json!({"data": STANDARD.encode(bytes), "decrypt":decrypt}),
    )?;
    STANDARD
        .decode(result["data"].as_str().ok_or("密钥保护结果无效")?)
        .map_err(|e| e.to_string())
}

pub fn read_text(path: &str) -> Result<Value, String> {
    call("readText", json!({"path":path}))
}

pub fn write_text(path: &str, text: &str) -> Result<(), String> {
    call("writeText", json!({"path":path,"text":text})).map(|_| ())
}

pub fn copy(source: &str, target: &str) -> Result<(), String> {
    call("copyFile", json!({"source":source,"target":target})).map(|_| ())
}
