# Android 开发

本目录保存 Android Tauri 入口、Gradle 工程及 Kotlin 设备插件，共用 `shared/` 中的账务、同步、应用装配和页面。

| 路径 | 职责 |
| --- | --- |
| `src-tauri/` | `lightledger-android` 原生包和平台配置 |
| `src-tauri/gen/android/` | Android Studio / Gradle 工程 |
| `tauri-plugin-device/` | Android Keystore 和系统文件操作 |
| `scripts/android.sh` | 已安装 SDK/NDK 的构建入口 |
| `package.json` | 本平台 Tauri 命令入口 |

在仓库根目录执行：

```bash
npm ci
rustup target add aarch64-linux-android
npm run android:build -- --ci -- --locked
```

需要 JDK 17、Android SDK 36、Build Tools 36.0.0、NDK 28.2.13676358，设置 `JAVA_HOME`、`ANDROID_HOME`，自定义 NDK 路径使用 `NDK_HOME`。`npm run android:dev` 用于连接手机调试；`npm run tauri:android -- android <command>` 直接调用本平台 Tauri CLI。

APK 输出目录为 `Android/src-tauri/gen/android/app/build/outputs/apk/`。独立 Rust 库名为 `lightledger_android_lib`，由 Tauri 构建生成对应的 JNI 加载代码。平台迁移保留原有 Gradle 工程，不需要重新初始化。

共享页面修改在 `shared/ui/`；Kotlin 系统操作留在本目录。构建和实机验证状态见 [工程说明](../Public.md)。
