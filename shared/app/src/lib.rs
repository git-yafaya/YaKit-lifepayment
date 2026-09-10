#[cfg(any(windows, target_os = "android"))]
mod app;
#[cfg(any(windows, target_os = "android", test))]
mod files;
#[cfg(windows)]
#[path = "../../../Windows/src-tauri/src/platform/windows.rs"]
mod platform;
#[cfg(target_os = "android")]
#[path = "../../../Android/src-tauri/src/platform.rs"]
mod platform;
#[cfg(any(windows, target_os = "android"))]
mod runtime;
#[cfg(any(windows, target_os = "android"))]
mod secrets;

#[cfg(any(windows, target_os = "android"))]
#[cfg_attr(target_os = "android", tauri::mobile_entry_point)]
pub fn run() {
    app::run();
}
