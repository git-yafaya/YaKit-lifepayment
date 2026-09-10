# Windows 开发

本目录保存 Windows 程序入口、系统接口、权限和安装包资源；账务、同步、Tauri 装配与 WebView 页面从 `shared/` 复用。

| 路径 | 职责 |
| --- | --- |
| `src-tauri/` | `lightledger-app` 原生包及 Windows API |
| `packaging/` | MSIX 清单与图标 |
| `scripts/` | Windows API 检查和 MSIX 构建 |
| `package.json` | 本平台 Tauri 命令入口 |

在仓库根目录执行：

```powershell
npm ci
npm run tauri:windows -- dev
npm run tauri:windows -- build --bundles nsis
```

需要 Windows SDK、MSVC C++ 工具、Rust、Node.js 22、WebView2 和 Perl。安装包位于根目录 `target/release/bundle/nsis/`。MSIX 构建使用 `Windows/scripts/windows-msix.ps1`，需要已有发布证书。

也可进入本目录执行 `npm run dev` 或 `npm run build`。共享页面修改在 `shared/ui/`，Windows 系统接口修改在本目录；具体能力和验证状态见 [工程说明](../Public.md)。
