#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#[cfg(windows)]
mod app;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod secrets;
#[cfg(windows)]
fn main() {
    app::run();
}
#[cfg(not(windows))]
fn main() {
    eprintln!("轻账桌面程序仅支持 Windows；此目标只用于检查共享代码。");
}

#[cfg(windows)]
mod runtime;
