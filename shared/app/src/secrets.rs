use std::{fs, path::Path};
#[cfg(windows)]
#[path = "../../../Windows/src-tauri/src/secrets_windows.rs"]
mod windows;
#[cfg(target_os = "android")]
use tauri_plugin_device::protect;
#[cfg(windows)]
pub use windows::protect;

pub const DATABASE_FILE: &str = if cfg!(target_os = "android") {
    "database.keystore"
} else {
    "database.dpapi"
};
pub const VAULT_FILE: &str = if cfg!(target_os = "android") {
    "sync.keystore"
} else {
    "sync.dpapi"
};

pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    protect(&fs::read(path).map_err(|e| e.to_string())?, true)
}
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let encrypted = protect(bytes, false)?;
    let temp = path.with_extension("pending");
    fs::write(&temp, encrypted).map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| e.to_string())
}
