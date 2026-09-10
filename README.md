# 轻账 LightLedger

轻账是一款面向 Windows 和 Android 的本地记账应用，用简洁的界面管理个人账单、共同账本和待处理事项。

## 功能特性

- 用统一的简洁线条图标辨认页面与操作，放大后依然清晰。
- 输入一句话整理为账单，确认金额、账户和付款人后保存。
- 搜索、修改、删除和恢复账单，查看修改历史。
- 导入标准 CSV/JSON 账单，识别重复来源并处理疑似重复记录。
- 分别查看不同币种的收支、分类和付款人统计。
- 管理账户、分类与规则，按需要共享账单。
- 使用加密数据库保存本地记录，导出账单或创建带恢复密码的备份。
- 配置 WebDAV，通过邀请文件连接其他设备并交换账本变更，跨端运行效果仍待联调。
- 在 Android 手机上使用整宽页面和文字导航，选择手机文件导入、导出或备份账单。
- 在 Windows 上核对截图识别或选定来源的通知，补齐信息后保存账单。
- 在 Windows 上使用 Windows Hello 开启账本界面锁。

当前版本处于开发验证阶段。Android 已开始接入；截图识别、通知读取和应用锁暂未提供。具体构建及验收结果见 [Public.md](Public.md)。

## 安装方法

当前仓库提供源码，尚未发布经过实机验收的安装包。

源码按平台分开管理，账务、同步和页面由各平台共同使用：

| 目录 | 开发入口 |
| --- | --- |
| `Windows/` | [Windows 开发说明](Windows/DEVELOPMENT.md) |
| `linux/` | [Linux 开发说明](linux/DEVELOPMENT.md)，原生客户端待接入 |
| `Android/` | [Android 开发说明](Android/DEVELOPMENT.md) |
| `ios/` | [iOS 开发说明](ios/DEVELOPMENT.md)，原生客户端待接入 |
| `shared/` | 共用的账务、同步、原生装配、界面和协议 |


1. 在 Windows 11 x64 上安装 Node.js 22、Rust stable，以及包含“使用 C++ 的桌面开发”的 Visual Studio Build Tools。
2. 准备 Windows SDK、WebView2 Runtime 和 Windows 原生 Perl。SQLCipher 的捆绑构建需要编译 OpenSSL。
3. 在仓库目录运行以下命令：

   ```powershell
   npm ci
   npm run tauri:windows -- dev
   ```

4. 生成 Windows 安装程序：

   ```powershell
   npm run tauri:windows -- build --bundles nsis
   ```

5. 安装程序输出到 `target/release/bundle/nsis/`。具备 Windows 包身份的版本及系统识别能力按 [Public.md](Public.md) 的平台说明验证。

环境要求参见 [Tauri 官方前置要求](https://v2.tauri.app/start/prerequisites/)。

Android 开发安装（Linux 或 macOS，需要 Bash）：

1. 安装 Node.js 22、Rust stable、JDK 17，以及 Android SDK 平台 36、Build Tools 36.0.0、Platform Tools 和 NDK 28.2.13676358。Linux 还需 C 编译工具、Make 和 Perl。
2. 设置 `JAVA_HOME` 指向 JDK，`ANDROID_HOME` 指向 SDK；脚本默认使用上述 NDK 版本，其他安装位置通过 `NDK_HOME` 指定。
3. 在仓库目录执行：

   ```bash
   npm ci
   rustup target add aarch64-linux-android
   npm run android:build -- --ci -- --locked
   ```

4. 在 `Android/src-tauri/gen/android/app/build/outputs/apk/universal/debug/` 找到 APK，传到 ARM64 手机并按系统提示安装。它是开发调试包，不是正式发布版本。
5. 需要开发调试时，连接已开启 USB 调试的手机，执行 `npm run android:dev`。选择其他架构的模拟器前，先安装相应的 Rust target。

仓库已包含 Android 工程，无需再次运行 `android init`。SDK 与 NDK 的安装说明见 [Android SDK 工具](https://developer.android.com/studio#command-line-tools-only)。

## 使用说明

1. 打开应用，通过导航中的图标和文字进入页面；在首页点击带加号的“记一笔”，或用一句话描述交易。
2. 核对金额、日期、类型、付款人和账户，保存后在“账单”中查看。
3. 在“待处理”中确认疑似重复、字段冲突或共同账本申请；确认前可查看对应内容。
4. 在“账单”中搜索商户、分类或备注，打开记录查看详情及修改历史。
5. 在“分析”中选择范围和日期，查看收支与分类情况。
6. 在“设置”中管理账户、配置 WebDAV，以及导出账单、创建或恢复加密备份。
7. 需要共同记账时，建立共同空间、完成设备邀请，再在账单详情中共享记录；“我的 / 我们”切换查看范围。

8. 在首页选择“导入截图”，核对识别文字后补齐账单；在“设置 → 隐私与自动录入”授权通知并选择来源，主动读取后逐笔确认。
9. 在“设置”中开启应用锁，下次打开或点击“立即锁定”后使用 Windows Hello 解锁。

第 8、9 步目前仅适用于 Windows。Android 使用顶部导航进入各页面；点击“导入文件”或在设置中导出、备份时，由系统文件选择器选择文件或保存位置。不支持的系统能力会显示原因。

导入使用应用导出的标准账单字段。第一次导入第三方账单时，先检查日期、金额单位和字段是否匹配；当前支持范围以实际导入结果为准。

## 主/副 API 配置说明

本地记账与解析不需要大模型 API 密钥。WebDAV 同步需要填写服务地址、用户名和密码，完成读写连接测试后保存。

恢复密码用于加密备份，与 WebDAV 密码用途不同。请保存好恢复密码，恢复备份时需要输入。

## 兼容性

目标系统为 Windows 11 x64，桌面界面由 WebView2 显示。建议从 Windows 11 24H2 或更新系统开始验收。

Android 工程最低为 Android 7.0（API 24），默认生成 ARM64 调试包。页面依赖系统 WebView，请保持 WebView 更新；目前尚未完成 Android 实机运行和双端同步验收。

文件、系统通知、图片识别和设备密钥保护使用 Windows 能力，必须在实际 Windows 环境检查。普通浏览器可用于开发界面验收，不能代替这些系统能力的验证。

## 常见问题

**数据保存在哪里？**

桌面应用将账本保存到当前用户的应用本地数据目录。数据库加密密钥由 Windows 当前用户保护。

Android 将账本保存在应用私有目录，密钥由 Android Keystore 保护。更换手机前请导出加密备份；系统备份和换机迁移不包含账本，卸载应用会删除本机数据。

**断网还能记账吗？**

可以。配置 WebDAV 后，应用运行期间每分钟尝试同步，也可以点击“立即同步”。关闭窗口后应用退出。

Android 进程存活时会尝试同步，但没有常驻后台服务；系统结束进程后不会继续同步。

**为什么某些通知不能自动记账？**

通知可能没有交易明细，也可能缺少访问授权或应用包身份。遇到缺失信息时使用手动记账、截图或文件导入补齐，实际支持入口以设置页能力状态为准。

**同步可以代替备份吗？**

不能。同步用于设备之间交换变更；加密备份用于误删、损坏或更换设备后的恢复。

## 更新日志

<details>
<summary>查看版本记录</summary>

### v0.1.0（Android 开发增量，未发布）

- 按 Windows、Linux、Android、iOS 分目录，共用账务、同步和界面模块。
- 统一界面中的导航、账单、助手、空状态及弹窗图标，采用简洁的 SVG 线条绘制。
- 建立 Android 工程，复用现有记账、加密存储和同步核心。
- 增加手机布局、Android Keystore 密钥保护及系统文件读写。
- 增加 ARM64 调试包构建命令与 Android CI，运行验收状态见工程说明。

### v0.1.0

- 建立 Rust + WebView2 Windows 工程。
- 接入本地账务、导入去重、统计、加密备份与同步基础能力。
- 接入截图识别、主动通知读取和 Windows Hello 界面锁，运行效果待实机验证。
- 增加 Windows 构建流程与开发验证入口。

</details>
