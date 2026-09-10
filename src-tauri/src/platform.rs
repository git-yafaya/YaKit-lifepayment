#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(target_os = "android")]
pub fn capabilities() -> serde_json::Value {
    serde_json::json!({
        "platform":"android", "packaged":true,
        "ocr":{"available":false,"languages":[],"reason":"Android OCR 尚未接入"},
        "notifications":{"available":false,"reason":"Android 通知读取尚未接入","access":"unavailable"},
        "hello":{"available":false,"reason":"Android 生物识别尚未接入","status":"unavailable"},
        "background":false,"startOnLogin":false
    })
}

#[cfg(target_os = "android")]
pub fn dispatch(action: &str, _: &serde_json::Value) -> Result<serde_json::Value, String> {
    if action == "helloAvailability" {
        return Ok(serde_json::json!({"available":false}));
    }
    Err(format!("Android 尚未接入此系统功能：{action}"))
}
