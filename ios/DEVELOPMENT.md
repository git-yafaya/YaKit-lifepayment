# iOS 开发

本目录用于 iOS 平台入口、系统适配和 Xcode 工程。当前尚未实现 iOS 客户端。

可复用模块位于 `shared/crates/`（账务和同步）、`shared/app/`（Tauri 装配）、`shared/ui/`（WebView 页面）和 `shared/contracts/`（协议）。iOS 系统接口和签名配置在本目录开发，共享模块保持单份。

原生构建与设备验证需要 macOS 和 Xcode。当前可先在其他开发机检查共享模块，平台接入状态见 [工程说明](../Public.md)。
