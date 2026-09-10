# Linux 开发

本目录用于 Linux 平台入口、系统适配和打包。当前尚未实现 Linux 原生客户端。

可复用模块位于 `shared/crates/`（账务和同步）、`shared/app/`（Tauri 装配）、`shared/ui/`（WebView 页面）和 `shared/contracts/`（协议）。平台专属代码在本目录开发，共享模块保持单份。

当前可在仓库根目录运行 `npm run dev` 查看共享页面，或运行 `cargo test --locked -p lightledger-core -p lightledger-sync` 检查共享 Rust 模块；这些命令不代表 Linux 客户端已完成。
