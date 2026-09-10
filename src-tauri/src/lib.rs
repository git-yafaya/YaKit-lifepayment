#[cfg(any(windows, target_os = "android"))]
mod app;
#[cfg(any(windows, target_os = "android", test))]
mod files;
#[cfg(any(windows, target_os = "android"))]
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
