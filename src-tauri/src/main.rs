#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#[cfg(windows)]
fn main() {
    lightledger_app_lib::run();
}
#[cfg(not(windows))]
fn main() {
    eprintln!("轻账支持 Windows 和 Android；此目标只用于检查共享代码。");
}
