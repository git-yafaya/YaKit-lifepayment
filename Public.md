# 轻账工程说明

轻账按 Windows、Linux、Android、iOS 分目录开发，使用 Rust 账务与同步核心、Tauri 装配和 TypeScript WebView 页面；共享模块集中在 `shared/`，原生程序各自构建。

## 仓库结构

```text
├── Windows/
│   ├── DEVELOPMENT.md  # 本平台开发入口与状态
│   ├── package.json  # 本平台 npm 命令入口
│   ├── packaging/
│   │   ├── AppxManifest.xml  # MSIX 包身份与能力声明
│   │   └── Assets/
│   │       ├── Square150x150Logo.png  # 应用图标资源
│   │       ├── Square44x44Logo.png  # 应用图标资源
│   │       └── StoreLogo.png  # 应用图标资源
│   ├── scripts/
│   │   ├── check-platform.sh  # Windows API 独立类型检查
│   │   └── windows-msix.ps1  # Windows MSIX 打包签名
│   └── src-tauri/
│       ├── Cargo.toml  # 本模块 Rust 依赖与源码入口
│       ├── capabilities/
│       │   └── default.json  # 本平台原生命令权限
│       ├── src/
│       │   ├── main.rs  # Windows 可执行程序入口
│       │   ├── platform/
│       │   │   ├── notifications.rs  # 通知授权与读取
│       │   │   ├── ocr.rs  # 本机图片识别
│       │   │   └── windows.rs  # Windows 能力检测与验证
│       │   └── secrets_windows.rs  # Windows DPAPI 封装
│       └── tauri.conf.json  # 本平台窗口与构建配置
├── linux/
│   └── DEVELOPMENT.md  # 本平台开发入口与状态
├── Android/
│   ├── DEVELOPMENT.md  # 本平台开发入口与状态
│   ├── package.json  # 本平台 npm 命令入口
│   ├── scripts/
│   │   └── android.sh  # Android SDK 与构建入口
│   ├── src-tauri/
│   │   ├── Cargo.toml  # 本模块 Rust 依赖与源码入口
│   │   ├── capabilities/
│   │   │   └── default.json  # 本平台原生命令权限
│   │   ├── gen/
│   │   │   └── android/
│   │   │       ├── .editorconfig  # 生成工程编辑格式
│   │   │       ├── .gitignore  # 构建产物与本地配置排除规则
│   │   │       ├── app/
│   │   │       │   ├── .gitignore  # 构建产物与本地配置排除规则
│   │   │       │   ├── build.gradle.kts  # Android Gradle 构建配置
│   │   │       │   ├── proguard-rules.pro  # 代码压缩规则
│   │   │       │   └── src/
│   │   │       │       └── main/
│   │   │       │           ├── AndroidManifest.xml  # Android 组件及权限声明
│   │   │       │           ├── java/
│   │   │       │           │   └── com/
│   │   │       │           │       └── yakit/
│   │   │       │           │           └── lightledger/
│   │   │       │           │               └── MainActivity.kt  # Android Activity 入口
│   │   │       │           └── res/
│   │   │       │               ├── layout/
│   │   │       │               │   └── activity_main.xml  # Android 容器布局
│   │   │       │               ├── mipmap-anydpi-v26/
│   │   │       │               │   └── ic_launcher.xml  # Android 自适应启动图标
│   │   │       │               ├── mipmap-hdpi/
│   │   │       │               │   ├── ic_launcher.png  # Android 启动图标
│   │   │       │               │   ├── ic_launcher_foreground.png  # Android 启动图标
│   │   │       │               │   └── ic_launcher_round.png  # Android 启动图标
│   │   │       │               ├── mipmap-mdpi/
│   │   │       │               │   ├── ic_launcher.png  # Android 启动图标
│   │   │       │               │   ├── ic_launcher_foreground.png  # Android 启动图标
│   │   │       │               │   └── ic_launcher_round.png  # Android 启动图标
│   │   │       │               ├── mipmap-xhdpi/
│   │   │       │               │   ├── ic_launcher.png  # Android 启动图标
│   │   │       │               │   ├── ic_launcher_foreground.png  # Android 启动图标
│   │   │       │               │   └── ic_launcher_round.png  # Android 启动图标
│   │   │       │               ├── mipmap-xxhdpi/
│   │   │       │               │   ├── ic_launcher.png  # Android 启动图标
│   │   │       │               │   ├── ic_launcher_foreground.png  # Android 启动图标
│   │   │       │               │   └── ic_launcher_round.png  # Android 启动图标
│   │   │       │               ├── mipmap-xxxhdpi/
│   │   │       │               │   ├── ic_launcher.png  # Android 启动图标
│   │   │       │               │   ├── ic_launcher_foreground.png  # Android 启动图标
│   │   │       │               │   └── ic_launcher_round.png  # Android 启动图标
│   │   │       │               ├── values/
│   │   │       │               │   ├── colors.xml  # Android 容器颜色
│   │   │       │               │   ├── ic_launcher_background.xml  # 启动图标背景色
│   │   │       │               │   ├── strings.xml  # Android 应用文字
│   │   │       │               │   └── themes.xml  # Android 原生容器主题
│   │   │       │               ├── values-night/
│   │   │       │               │   └── themes.xml  # Android 原生容器主题
│   │   │       │               └── xml/
│   │   │       │                   ├── data_extraction_rules.xml  # 系统备份与迁移排除
│   │   │       │                   └── file_paths.xml  # 文件提供器路径
│   │   │       ├── build.gradle.kts  # Android Gradle 构建配置
│   │   │       ├── buildSrc/
│   │   │       │   ├── build.gradle.kts  # Android Gradle 构建配置
│   │   │       │   └── src/
│   │   │       │       └── main/
│   │   │       │           └── java/
│   │   │       │               └── com/
│   │   │       │                   └── yakit/
│   │   │       │                       └── lightledger/
│   │   │       │                           └── kotlin/
│   │   │       │                               ├── BuildTask.kt  # Gradle 调用 Tauri 构建
│   │   │       │                               └── RustPlugin.kt  # Rust ABI 构建任务
│   │   │       ├── gradle/
│   │   │       │   └── wrapper/
│   │   │       │       ├── gradle-wrapper.jar  # Gradle Wrapper 执行器
│   │   │       │       └── gradle-wrapper.properties  # Gradle 发行版配置
│   │   │       ├── gradle.properties  # Gradle 参数
│   │   │       ├── gradlew  # Gradle Unix 启动器
│   │   │       ├── gradlew.bat  # Gradle Windows 启动器
│   │   │       └── settings.gradle  # Android 模块装配
│   │   ├── icons/
│   │   │   └── icon.svg  # 与主页一致的 Android 启动图标源
│   │   ├── src/
│   │   │   └── platform.rs  # Android 能力声明
│   │   └── tauri.conf.json  # 本平台窗口与构建配置
│   └── tauri-plugin-device/
│       ├── Cargo.toml  # 本模块 Rust 依赖与源码入口
│       ├── android/
│       │   ├── build.gradle.kts  # Android Gradle 构建配置
│       │   └── src/
│       │       └── main/
│       │           ├── AndroidManifest.xml  # Android 组件及权限声明
│       │           └── java/
│       │               └── DevicePlugin.kt  # Keystore 与系统文件适配
│       ├── build.rs  # Android 原生插件构建
│       └── src/
│           └── lib.rs  # Android Kotlin 插件桥
├── ios/
│   └── DEVELOPMENT.md  # 本平台开发入口与状态
├── shared/
│   ├── app/
│   │   ├── build.rs  # Tauri 平台资源构建
│   │   ├── icons/
│   │   │   ├── icon.ico  # 应用图标资源
│   │   │   └── icon.png  # 应用图标资源
│   │   └── src/
│   │       ├── app.rs  # 共享 Tauri 命令与启动装配
│   │       ├── files.rs  # 跨平台文件与备份中转
│   │       ├── lib.rs  # 公共应用库及平台选择
│   │       ├── runtime.rs  # 共享同步和设备状态
│   │       └── secrets.rs  # 共用密钥文件读写
│   ├── contracts/
│   │   ├── PROTOCOL.md  # 同步协议及认证字节
│   │   ├── examples/
│   │   │   ├── crypto-vector.json  # 固定密码验证向量
│   │   │   └── operation.json  # 合成操作示例
│   │   └── schema/
│   │       ├── envelope.schema.json  # 密文信封结构
│   │       └── operation.schema.json  # 账务操作结构
│   ├── crates/
│   │   ├── ledger-core/
│   │   │   ├── Cargo.toml  # 本模块 Rust 依赖与源码入口
│   │   │   ├── examples/
│   │   │   │   └── dev_bridge.rs  # 临时账本开发服务
│   │   │   └── src/
│   │   │       ├── backup.rs  # 加密备份、同步进度恢复与旧格式迁移
│   │   │       ├── capture.rs  # 文本解析和来源去重
│   │   │       ├── commands.rs  # 账务命令分发
│   │   │       ├── lib.rs  # SQLCipher、事务及核心自检
│   │   │       ├── queries.rs  # 分页查询与统计
│   │   │       ├── sync.rs  # 账务操作及字段合并
│   │   │       ├── transfer.rs  # CSV/JSON 导入导出与退款顺序处理
│   │   │       └── workflow.rs  # 候选、冲突和来源保留
│   │   └── ledger-sync/
│   │       ├── Cargo.toml  # 本模块 Rust 依赖与源码入口
│   │       └── src/
│   │           ├── control.rs  # 设备名册传播
│   │           ├── crypto.rs  # 加密信封和签名
│   │           ├── dav.rs  # WebDAV 探测及读写
│   │           ├── engine.rs  # 上传下载和连续游标
│   │           ├── lib.rs  # 同步模块入口及协议自检
│   │           └── pairing.rs  # 邀请与密钥轮换
│   └── ui/
│       ├── bridge.ts  # 原生命令和开发连接
│       ├── components/
│       │   ├── analysis.ts  # 日期筛选和分析显示
│       │   ├── devices.ts  # 设备邀请与配对
│       │   ├── editor.ts  # 账单编辑和文本确认
│       │   ├── files.ts  # 文件导入导出和备份入口
│       │   ├── icons.ts  # 共用 SVG 图标与绘制规范
│       │   ├── pending.ts  # 待办确认与审批
│       │   ├── platform.ts  # 平台能力和应用锁界面
│       │   ├── rules.ts  # 分类规则设置
│       │   ├── settings.ts  # 设置页面装配
│       │   ├── sync-status.ts  # 同步状态和结果提示
│       │   ├── transactions.ts  # 账单列表及操作
│       │   └── ui.ts  # 通用弹窗和表单
│       ├── env.d.ts  # 前端环境类型
│       ├── index.html  # 共享 WebView 页面入口
│       ├── main.ts  # 导航与页面装配
│       ├── style.css  # 全部页面共用主题与动效
│       └── types.ts  # 类型、金额格式和文本转义
├── .github/
│   └── workflows/
│       ├── android.yml  # Android 调试包 CI
│       └── windows.yml  # Windows 编译及安装包 CI
├── .gitignore  # 构建产物与本地配置排除规则
├── AGENTS.md  # 项目开发与验收约定
├── Cargo.lock  # Rust 锁定依赖
├── Cargo.toml  # Rust 工作区与平台包路径
├── Public.md  # 工程与接口说明
├── README.md  # 安装和使用说明
├── package-lock.json  # Node 锁定依赖
├── package.json  # 根依赖与各平台命令
├── scripts/
│   └── check-ui.mjs  # 金额和转义自检
├── tsconfig.json  # TypeScript 检查选项
└── vite.config.ts  # 共享页面构建与服务
```

设计文档保留在本地并由 `.gitignore` 排除；当前实现路线见本文件，功能规划见 `轻账-Windows设计与技术方案.md`。Rust 工作区默认构建共享核心与同步；Windows 和 Android 使用独立 Cargo 包，Linux 和 iOS 当前仅保留开发目录。

## 加载与数据流

1. Tauri 创建统一窗口并加载打包后的本地页面。
2. 平台入口编译同一份 `shared/app` 装配代码，Rust 定位本机应用数据目录。Windows 通过 DPAPI、Android 通过 Keystore 解封数据库密钥；首次运行创建随机密钥。
3. SQLCipher 校验加密支持并打开数据库，恢复本人成员、设备身份和本地状态。
4. 前端调用 `ledger_command` 查询账单、汇总和待处理内容；空账本显示记账和导入入口。
5. 用户命令进入 Rust 校验，数据库变更、历史和 Outbox 在同一事务中提交；失败回滚。CSV/JSON 导入按行使用保存点，先处理非退款、再处理退款，结果保持文件原行号和顺序。JSON 非对象行与 CSV 格式错误逐行返回失败，其余有效行继续；同一文件的来源标识与原行号绑定。
6. 已配置 WebDAV 时启动后及每 60 秒补同步；网络操作在账务锁之外执行，每次读取或应用操作时短暂持有账务锁。关闭窗口即退出，不持续后台采集。
7. 图片识别和系统通知经 Windows 适配层转换为候选输入，确认关键字段后进入相同账务路径。Android 文件选择器的 `content://` 地址由 Kotlin 插件读取，按返回的真实文件名判断导入格式；备份只中转已加密内容并清理暂存目录。

### 账务边界

| 输入或状态 | 处理原则 |
| --- | --- |
| 金额 | 使用最小货币单位整数字符串；Rust 校验范围，前端用 BigInt 格式化 |
| 不完整输入 | 要求补充必要字段，不能把缺失金额作为零保存 |
| 收入、支出、转账、退款 | 类型独立；转账不计入收支；退款关联同币种原支出，有有效退款时不能单独修改原支出币种；合并前旧编号与现编号的退款合并校验额度和币种，旧编号引用在恢复后仍有效 |
| 重复来源 | 保持捕获幂等；疑似重复进入待处理，不重复增加统计 |
| 人工修改 | 保留字段锁和历史，后续自动来源不得静默覆盖 |
| 查询 | 个人范围仅包含本人所有记录，共同范围包含共享投影；时间保存为 UTC、按设备本地日期筛选，币种分别汇总 |
| 网络错误 | 保留本地变更和待发送操作，网络恢复后重试；格式或认证错误暂停对应空间并显示状态 |
| 乱序操作 | 按空间与设备的连续序号推进；缺口未补齐不跳过 |
| 同字段并发修改 | 保留冲突候选供人工选择；不同字段可合并；解决时只补充本次新建的操作，已存在的待发操作和密文不改写，共同操作仅包含投影字段 |
| 共同操作 | 通过可信成员名册验证申请人；修改审批覆盖编辑器的金额、币种、类型、时间、商户、分类、付款人、账户、备注和退款关联，保留标识、所有权及共享状态；未展示的私有字段被表单补为空时保留所有者原值 |
| 共同变更转发 | 他人对本人账单的共同变更回送个人空间；所有者自行保存已产生个人操作，不再重复转发。多台设备转发同一事实沿用共同源字段版本；移除字段发送 null，清除其他设备旧快照 |
| 个人删除共享记录 | 个人记录进入回收站，共同快照继续保留；共同审批删除后移除投影，个人恢复不会自动重新共享 |
| 无效密钥、损坏数据 | 报错并保留原始文件，不创建空账本覆盖旧数据 |
| Android 文件输入 | 支持已授权的 content URI、file URI 和本机路径；其他地址报错。文本读取上限为 20 MiB，非法 UTF-8 报错，文件格式按显示文件名判断 |
| Android 备份中转 | 使用私有临时目录，只中转加密备份；成功和失败都清理暂存，启动时清理上次中断的暂存。目标写入失败会报错，所选目标文件可能不完整 |
| Android 密钥迁移 | 数据库已存在但密钥文件丢失时中止启动；Keystore 密钥不导出，系统云备份和换机迁移排除账本，换机使用加密备份或同步 |
| 备份恢复 | 校验口令和数据后事务恢复，失败回滚；新备份保存 applied/cursors/outbox，恢复保留历史进度，未上传操作保留 ID 和字段依赖、按新设备重新编号，同成员同空间可在非空账本重新配对 |
| 旧备份恢复 | 缺少完整同步进度的 QACCOUNT1 保留本地账本与历史，建立新的个人/共同空间并重建个人待发快照；不能切回旧空间，需从恢复设备邀请空账本设备加入，共同空间需重新创建和配对 |

### 同步与密钥

`personal` 空间授权给本人的设备，`shared` 空间授权给共同账本成员。账单事实与共享投影分别处理，不把对方未共享的个人记录加入共同空间。

WebDAV 使用 HTTPS。配置探测包含目录读取、临时对象写入、读取比对和清理。同步文件加密并认证设备身份，上传失败重试使用原操作的相同密文字节；远端操作经过认证、空间和成员校验后才能进入账务层。

0.1.1 新发送的账务信封使用协议 v2，并继续读取 v1 历史；目录布局、配对文件与控制消息保持原格式。个人转发操作可含 `sourceVersion`，作为该补丁的字段版本；收到已在 applied 或本机 outbox 中的相同操作 ID 时跳过合并，仍按接收设备的连续序号推进游标。旧版客户端遇到 v2 暂停对应空间，升级后继续；确切认证字节见 `shared/contracts/PROTOCOL.md`。

本机数据库密钥与同步凭据在 Windows 上通过当前用户的 DPAPI 保护，在 Android 上通过 Keystore 保护。备份使用独立恢复密码；不要把应用数据目录中的设备密钥文件作为跨设备导入方式。

## 设置与主题

| 设置或机制 | 默认值或含义 |
| --- | --- |
| 窗口 | 1200×800，最小尺寸由 Tauri 配置约束，各页面复用同一窗口 |
| 手机布局 | 不超过 560px 时使用顶部六项文字导航和整宽页面；共用 safe area、动态视口高度、弹窗滚动及主页微动效 |
| Android 系统能力 | OCR、通知读取、生物识别尚未接入；返回不可用及原因，对应入口禁用 |
| 默认币种 | CNY；统计按币种分组 |
| 本地数据 | 当前用户应用本地目录，数据库采用 SQLCipher |
| WebDAV | 初始未配置；读写探测通过后保存凭据 |
| 共享 | 显式选择需要共享的账单 |
| Windows 通知 | 用户单独授权并选择来源，仅读取 |
| OCR | 检查包身份和本机识别语言，缺少能力时返回原因 |
| Windows Hello | 默认关闭；开启后启动和手动锁定时遮挡界面，解锁调用系统验证；属于界面锁，Rust 数据库服务仍运行 |
| 来源保留 | 默认 7 天，核心在命令事务中清理过期原文并保留幂等指纹；支持保留时长设置，尚未提供设置界面 |
| 主题 | 暖白与珊瑚强调色，通过统一 CSS 变量使用 |
| 界面图标 | 共用内联 SVG，统一 24×24 网格、1.8 线宽、圆角端点及 `currentColor`；折叠标记使用同规范的 SVG 遮罩。各页面沿用主页配色、占位尺寸和动效，装饰图标不重复朗读，独立按钮保留文字名称 |
| 动效 | 页面共享样式并响应 `prefers-reduced-motion` |

通知仅在用户点击读取时取回已选择来源的现有通知；目前未接入持续通知监听或后台自动入账。权限是否可用与用户是否已授权分开显示。Windows 系统能力由 Rust 提供，WebView 不保存数据库密钥或 WebDAV 凭据。

## 公开 API

当前接口是本仓库前端与 Rust 的内部命令契约，尚未发布独立第三方 SDK。

| Tauri 命令 | 参数 | 返回与错误 |
| --- | --- | --- |
| `ledger_command` | `action: string`、`payload: object` | 成功返回 JSON；业务校验失败以 rejected Promise 返回中文错误 |
| `system_command` | `action: string`、`payload: object` | 文件、设备与本机能力结果；无能力或无授权时返回真实错误 |

Android 沿用上述命令，文件与能力动作的契约如下。Keystore 桥只供 Rust 调用，不向页面开放密钥操作。

| 动作 | 参数与返回 |
| --- | --- |
| `readFile` | 输入 `{path}`，返回 `{text,name}`；`name` 为系统显示文件名，无法取得时为空字符串 |
| `saveFile` | 输入 `{path,text}`，返回 `{saved:true}` |
| `exportBackup` / `restoreBackup` | Android 的 `path` 可直接传系统授权的 content URI |
| `capabilities` | Android 增加 `platform:"android"`，不可用的 `ocr`、`notifications`、`hello` 各自提供 `reason` |

在仓库前端 TypeScript 模块中可直接使用：

```typescript
import { invoke } from '@tauri-apps/api/core';

// 读取本人账本第一页，不在页面里扫描全部记录。
const records = await invoke('ledger_command', {
  action: 'list',
  payload: { page: 0, pageSize: 50, search: '', shared: false, deleted: false },
});
console.log(records);
```

主要动作按职责分组：

| 类别 | 动作 |
| --- | --- |
| 账务 | `create`、`update`、`delete`、`restore`、`share`、`list`、`summary`、`analysis`、`history` |
| 捕获 | `parseText`、`importText`、`capture`、`pending`、`confirmCandidate`、`resolveDuplicate`、`resolveConflict`、`resolveDifference` |
| 共同审批 | `requestSharedDelete`、`approveSharedDelete`、`approveModification`、`members` |
| 数据管理 | `importCsv`、`importJson`、`exportCsv`、`exportJson`、`exportBackup`、`restoreBackup` |
| 设置 | `accounts`、`categories`、`rules`、`addAccount`、`addCategory`、`learnRule`、`setRule`、`revokeRule` |
| 同步装配 | `syncConfigure`、`syncStatus`、`syncRun`、`createSharedSpace`、`pairingRequest`、`pairingApprove`、`pairingAccept`、`revokeDevice`、`applyRotation` |
| 系统能力 | `capabilities`、`ocrImage`、`notificationRequestAccess`、`notificationSources`、`notificationRead`、`helloVerify`、`deviceSettings`、`saveDeviceSettings`；Android 未接入的操作返回不可用原因 |

常用账务载荷如下；金额和标识按表传入，不能把金额转换成 JavaScript 浮点数。

| 动作 | payload 主要字段 | 成功结果 |
| --- | --- | --- |
| `create` | `amountMinor` 整数字符串，`currencyCode` 为 CNY/USD/EUR/JPY，`kind` 为 expense/income/transfer/refund，`occurredAt` 为带时区 RFC3339，`payerId`、`accountId`、`merchant`、`category`、`note`；退款需 `originalTransactionId` | 新记录标识或 `existing` / `duplicate` 状态；疑似重复附 `pendingId` |
| `update` | `id` 与要更新的账务字段 | 更新记录；修改他人共享账单返回审批 `pendingId` |
| `list` | `shared=false`、`deleted=false`、`page=0`、`pageSize=50`、`search`、`from/to`（YYYY-MM-DD） | 记录数组；每页限制 1–200 条 |
| `summary` / `analysis` | `shared` 与可选 `from/to` | 按币种汇总 / monthly、categories、payers 数组 |
| `parseText` | `text` | `draft` 与 `missingFields`，不会直接创建账单 |
| `confirmCandidate` | 待办 `id`、确认后的 `draft` | 账单处理结果 |
| `resolveDuplicate` | 待办 `id`、`merge` 布尔值 | 合并来源，或保存为独立账单 |
| `resolveConflict` / `resolveDifference` | 待办 `id`、`choices` 字段选择对象 | 确认结果并生成修改操作 |
| `approveSharedDelete` / `approveModification` | 待办 `id`、`approve` 布尔值 | 由当前已认证成员完成对应确认 |
| `exportBackup` / `restoreBackup` | `path`、`password` | 导出返回 `saved`；恢复返回 `restored`、`requiresPairing`、`independentLedger`，其中 `independentLedger` 表示旧格式已恢复为独立账本；错误通过 rejected Promise 返回 |

内部同步应用和身份变更动作只允许受信任的原生装配层调用，不能由页面自由指定成员身份。`syncStatus` 返回 `configured`、`paused`、`lastSync`、设备指纹与空间名册，不返回密码或空间密钥。序列化细节与密文格式以 `shared/contracts/` 为准。

## 当前接入状态

| 项目 | 状态 |
| --- | --- |
| Rust 核心与前端 | 已实现；前端生产构建与 Rust 核心/同步自检通过 |
| 本地加密与备份 | 共用 SQLCipher 与密码备份；Windows 接入 DPAPI，Android 接入 Keystore 和文件中转 |
| WebDAV 与操作加密 | 已实现网络协议与加密模块，模拟服务验证已通过 |
| Windows OCR / 通知 / Hello | 已实现 Windows API 适配，独立 MSVC 目标类型检查通过，运行行为待 Windows 实机验收 |
| 普通安装程序 | 配置了 Tauri NSIS/MSI 构建；不会自动获得 MSIX 包身份 |
| MSIX | 提供清单及构建签名脚本，需 Windows SDK 和已有受信任发布证书 |
| Windows 整体构建 | 当前 Linux 缺少 MSVC 原生工具，整体构建及安装运行尚未验证；已提供 Windows CI |
| Android | ARM64 调试 APK 已构建并通过签名、16 KB 对齐检查；接入移动端页面、Keystore 与文件插件，安装和系统操作待实机验证 |
| Android 尚未接入 | 截图 OCR、通知读取、指纹/人脸应用锁、系统分享接收、常驻后台同步及正式发布签名 |
| Linux / iOS | 已分配开发目录，尚未接入原生入口和打包 |
| 尚未接入 | 跨端双向联调、持续通知监听、托盘后台、开机启动、远端快照与日志压缩 |
| 账务待完善 | 转账目前仅作独立流水，不维护收付款双账户余额；共同删除后没有共同回收站恢复或到期物理清理；来源模板、识别置信度和专门账户澄清状态机未实现 |
| WebView 迁移验收 | 已在本窗口内置浏览器验证首页 → 设置 → 账单 → 首页导航，页面加载与临时账本连接正常；系统功能仍由原生端验收 |

平台拆分复用同一份界面和业务源码。Windows 与 Android 各有独立配置、权限和 Cargo 包，共享库通过 `cfg` 引入对应平台适配；Linux 和 iOS 目录记录接入位置。

## 开发与验证

2026-09-11 v0.1.1 业务修复验证：`cargo test --locked --offline --workspace` 全部 22 项通过（核心 18、同步 2、Windows/Android 文件中转各 1）；前端生产构建、金额与文本检查、Rust 格式、Cargo 路径和版本检查均通过。回归覆盖异常导入、退款顺序与合并引用、新旧加密备份恢复、完整修改审批、冲突解决及多设备共同状态转发；新发 v2 与读取 v1 历史信封均通过验证。本次未修改界面，Windows/Android 实机同步和恢复流程仍待验收。

2026-09-10 SVG 图标验证：生产构建和现有金额/文本检查通过；本窗口内置浏览器完成六页导航、临时账本收支/转账图标、规则箭头、弹窗关闭与历史折叠操作。1200、600、390、320px 视口检查通过，无横向溢出，320px 下关闭按钮保持 44×44px。锁屏品牌复用同一图标组件，Windows Hello 原生流程仍需实机验收。

界面修改位于 `shared/ui/`；账务与同步分别位于 `shared/crates/`；Tauri 装配位于 `shared/app/`；系统能力由各平台目录维护。改变跨层契约时先对齐动作、参数和错误语义，再分别修改相关层。中文注释说明业务限制和系统边界。

| 命令或脚本 | 用途 | 环境 |
| --- | --- | --- |
| `npm ci` | 按锁文件安装前端依赖 | Node.js 22 |
| `npm run build` | TypeScript 检查及前端生产构建 | 开发机 |
| `cargo test --locked -p lightledger-core -p lightledger-sync` | 核心和同步内置自检 | Rust 与本地 C/Perl 工具链 |
| `cargo test --locked --workspace` | 核心、同步、备份恢复与平台文件中转自检 | Rust、C/Perl 与宿主 Tauri 开发依赖 |
| `node --experimental-strip-types scripts/check-ui.mjs` | 金额精度、币种小数位及 HTML 转义检查 | Node.js 22 |
| `cargo fmt --all --check` | 检查 Rust 格式 | Rustfmt |
| `cargo metadata --locked --no-deps --format-version 1` | 检查平台与共享包的路径解析 | Rust |
| `npm run android:build -- --ci -- --locked` | 构建 ARM64 调试包 | JDK 17、Android SDK/NDK |
| `npm run android:dev` | 连接 Android 手机调试 | 已授权 USB 调试的手机与 Android 工具链 |
| `npm run tauri:windows -- dev` | 运行 Windows 桌面开发程序 | Windows |
| `npm run tauri:windows -- build --bundles nsis` | 生成 Windows 安装程序 | Windows |
| `Windows/scripts/check-platform.sh` | 独立检查 Windows 平台 API 类型 | Rust Windows MSVC target |
| `Windows/scripts/windows-msix.ps1` | 构建带包身份的签名 MSIX | Windows SDK 与已有证书 |

2026-09-10 目录迁移验证：前端构建、金额检查、Rust 格式、Cargo 路径解析及 Windows API 类型检查通过；共享核心 6/6、同步 2/2、Windows/Android 文件与备份中转各 1/1 通过。本窗口内置浏览器已完成首页、设置、账单页面切换。Tauri 构建钩子也已实际验证：从平台原生目录以 `cwd=../..` 调用根前端构建，避免重复进入平台构建命令。本次未改业务与 UI 行为，Android 原有依赖兼容问题由 Android 开发任务继续处理。

2026-09-10 Android 首轮构建与验收：

| 检查 | 结果 |
| --- | --- |
| Tauri 运行时 | 锁定 `tauri-runtime` / `tauri-runtime-wry` 2.10.0，与 Tauri 2.10.2 配套；ARM64 Rust、Kotlin 与 Gradle 编译通过 |
| 构建环境 | Linux x86_64、JDK 17、SDK 36、AGP 8.11.0、Gradle 8.14.3、NDK 28.2.13676358；AGP 实际使用 Build Tools 35.0.0 |
| 调试 APK | `Android/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`；包名 `com.yakit.lightledger`，版本 0.1.0（1000），最低 API 24，目标 API 36，仅 `arm64-v8a` |
| APK 校验 | `apksigner verify --verbose` 通过 APK v2 签名；`zipalign -c -P 16 4` 通过，原生库各 LOAD 段均按 0x4000 对齐 |
| 源码检查 | `cargo test --locked --workspace` 10/10、前端生产构建、金额/转义检查、Rust 格式和 Android 脚本语法检查通过 |
| 本窗口手机视口交互 | 在实际 Rust 临时账本创建账户和 12.34 元支出，核对账单与分析，再删除到回收站；六页导航可操作，360/390px 无横向溢出，短屏弹窗可滚动，1200px 恢复桌面布局 |
| 待实机验收 | 安装启动、Keystore 重启后解密、系统文件导入导出、加密备份恢复、软键盘与系统返回键、进程生命周期、Android/Windows 双端同步 |

Android 启动图标保留 SVG 源并生成自适应图标，与主页的账本形状和珊瑚色一致。重生成命令见 `Android/DEVELOPMENT.md`。调试 APK 包含调试符号，尚未进行正式发布签名；上述编译和浏览器结果不代表 Android 实机运行已通过。

迁移前代码 `179a920` 的原生构建记录（Linux x86_64、Rust 1.97.1、Node.js 22.22.2）：

| 检查 | 本次结果 |
| --- | --- |
| `npm run build` | TypeScript 检查及 Vite 生产构建通过 |
| `node --experimental-strip-types scripts/check-ui.mjs` | 金额精度、币种小数位和文本转义通过 |
| `cargo fmt --all --check` | 通过 |
| `cargo test --locked -p lightledger-core -p lightledger-sync -- --nocapture` | 核心 6/6、同步 2/2 通过，无失败；分别耗时 12.03 秒、26.62 秒 |
| `CARGO_NET_OFFLINE=true Windows/scripts/check-platform.sh` | Windows OCR、通知与 Hello 平台模块的独立 MSVC 目标类型检查通过 |
| `cargo check --locked -p lightledger-app --target x86_64-pc-windows-msvc` | 退出码 101；OpenSSL/ring 原生构建失败，明确报错缺少 `lib.exe`，尚不能确认 Windows 整体编译通过 |
| 浏览器交互 | 子代理直连 IAB 返回 `IAB visibility is not supported in a subagent thread`；已停止，未读取页面或执行操作 |

核心测试覆盖加密数据库、备份错误密码回滚、金额与退款约束、导入幂等和逐行失败、个人/共同删除与审批、跨午夜查询和字段冲突。同步测试覆盖签名与加密、固定密码向量、配对撤销、模拟 WebDAV 重试及连续序号；不代表真实 NAS 或 Windows 运行验收。错误密钥检查产生的 SQLCipher 解密错误日志是预期结果。

2026-09-11 的 50,000 条记录 debug 内存 SQLCipher 测量：填充约 1.52 秒，分页 50 条加汇总约 397 毫秒，1,000 行导入约 0.90 秒；不是 Windows 磁盘性能或整机验收结果。

MSIX 构建示例（证书已放在当前用户证书库，目标设备已信任签发链）：

```powershell
.\Windows\scripts\windows-msix.ps1 -CertificateThumbprint $CertificateThumbprint
```

脚本查找 Windows SDK 的 MakeAppx/SignTool，构建、打包、签名并验证，结果位于 `artifacts/`。需要用户提供真实发布证书指纹；脚本不会安装或信任新证书。普通 NSIS/MSI 包不提供 OCR/通知所需的 MSIX 包身份。

浏览器开发验证连接实际 Rust 核心，并使用临时账本：先运行 `cargo run -p lightledger-core --example dev_bridge`，再为 Vite 设置 `VITE_LEDGER_DEV_BRIDGE=http://127.0.0.1:1421` 后运行 `npm run dev`。页面地址为 `http://127.0.0.1:1420/`；普通生产构建只使用 Tauri 命令桥。

验收遵循项目 `AGENTS.md`：内置浏览器必须读取状态并实际操作，直连失败时停止自动 UI 验收并交人工。Windows 专属能力需在 Windows 实机完成，不能用浏览器预览代替。
