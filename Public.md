# 轻账 Windows 工程说明

轻账使用 Rust 实现账务与同步，通过 Tauri 2 在 Windows WebView2 中加载 TypeScript 界面；需要构建前端和原生程序。

## 仓库结构

```text
.github/workflows/windows.yml          # Windows 编译、自检与安装包构建
.gitignore                             # 本地材料、依赖与产物排除规则
AGENTS.md                              # 项目开发与验收约定
Cargo.toml                             # Rust 工作区定义
Cargo.lock                             # Rust 锁定依赖
package.json                           # 前端命令与 Tauri CLI 依赖
package-lock.json                      # Node 锁定依赖
tsconfig.json                          # TypeScript 检查选项
vite.config.ts                         # 页面构建与本地服务
README.md                              # 安装和使用说明
Public.md                              # 工程与接口说明
frontend/
├── index.html                         # WebView 页面入口
├── main.ts                            # 导航、账本范围与页面装配
├── bridge.ts                          # 原生命令与开发连接
├── types.ts                           # 数据类型、金额格式及文本转义
├── env.d.ts                           # 开发环境类型
├── style.css                          # 全部页面共用主题与动效
└── components/
    ├── ui.ts                          # 弹窗、提示与通用表单
    ├── transactions.ts                # 账单列表、分页与操作
    ├── editor.ts                      # 账单编辑和文本确认
    ├── analysis.ts                    # 日期筛选与分类统计
    ├── pending.ts                     # 重复、冲突与共同申请确认
    ├── settings.ts                    # 账户与设置页装配
    ├── rules.ts                       # 分类规则管理
    ├── files.ts                       # 账单导入导出与备份入口
    ├── devices.ts                     # 邀请文件与设备管理
    ├── sync-status.ts                 # 同步状态、暂停及结果提示
    └── platform.ts                    # OCR、通知与应用锁界面
crates/
├── ledger-core/
│   ├── Cargo.toml                     # 账务核心依赖
│   ├── examples/dev_bridge.rs         # 临时账本的本机 HTTP 开发桥
│   └── src/
│       ├── lib.rs                    # SQLCipher、事务、校验与内置自检
│       ├── commands.rs               # 账务动作分发与修改
│       ├── queries.rs                # 分页查询、规则和统计
│       ├── capture.rs                # 文本解析、来源与去重
│       ├── workflow.rs               # 候选、冲突、合并与来源保留
│       ├── transfer.rs               # CSV/JSON 和加密备份
│       └── sync.rs                   # Outbox、远端操作和字段版本
└── ledger-sync/
    ├── Cargo.toml                     # 同步和密码依赖
    └── src/
        ├── lib.rs                    # 模块入口及协议内置自检
        ├── crypto.rs                 # 密文信封、签名与空间密钥
        ├── dav.rs                    # HTTPS WebDAV 和读写探测
        ├── engine.rs                 # 操作上传、下载与连续游标
        ├── pairing.rs                # 邀请、批准、撤销与密钥轮换
        └── control.rs                # 可信设备名册传播
src-tauri/
├── Cargo.toml                         # 桌面壳与 Windows 依赖
├── build.rs                           # Tauri 资源构建
├── tauri.conf.json                    # 窗口、内容策略与安装包配置
├── capabilities/default.json          # 原生对话框权限
├── icons/
│   ├── icon.ico                       # Windows 程序图标
│   └── icon.png                       # 通用程序图标
└── src/
    ├── main.rs                        # 平台启动入口
    ├── app.rs                         # Tauri 命令、单实例与后台同步
    ├── runtime.rs                     # 同步装配、文件及本机设置
    ├── secrets.rs                     # Windows DPAPI 密钥封装
    ├── platform.rs                    # 能力检测与 Hello 验证
    └── platform/
        ├── ocr.rs                     # 图片解码与本机文字识别
        └── notifications.rs           # 通知授权、来源与读取
contracts/
├── PROTOCOL.md                        # 跨端协议和精确认证字节
├── schema/
│   ├── envelope.schema.json           # 密文信封结构
│   └── operation.schema.json          # 账务操作结构
└── examples/
    ├── operation.json                 # 合成操作示例
    └── crypto-vector.json             # 固定合成密码测试向量
packaging/windows/
├── AppxManifest.xml                   # MSIX 包身份与能力声明
└── Assets/
    ├── StoreLogo.png                  # 包图标
    ├── Square44x44Logo.png             # 小尺寸应用图标
    └── Square150x150Logo.png           # 应用图块图标
scripts/
├── check-platform.sh                  # Windows API 独立类型检查
├── check-ui.mjs                       # 金额精度和转义自检
└── windows-msix.ps1                    # MSIX 构建与证书签名
```

设计文档保留在本地并由 `.gitignore` 排除；当前实现路线见本文件，功能规划见 `轻账-Windows设计与技术方案.md`。Rust crate 默认构建目标为核心与同步，Windows 桌面壳独立构建。

## 加载与数据流

1. Tauri 创建统一窗口并加载打包后的本地页面。
2. Rust 定位当前用户的应用数据目录，通过 DPAPI 解封数据库密钥；首次运行创建随机密钥。
3. SQLCipher 校验加密支持并打开数据库，恢复本人成员、设备身份和本地状态。
4. 前端调用 `ledger_command` 查询账单、汇总和待处理内容；空账本显示记账和导入入口。
5. 用户命令进入 Rust 校验，数据库变更、历史和 Outbox 在同一事务中提交；失败回滚。CSV/JSON 导入按行使用保存点，失败行回滚，其余有效行保留并报告行号。
6. 已配置 WebDAV 时启动后及每 60 秒补同步；网络操作在账务锁之外执行，每次读取或应用操作时短暂持有账务锁。关闭窗口即退出，不持续后台采集。
7. 图片识别和系统通知经 Windows 适配层转换为候选输入，确认关键字段后进入相同账务路径。

### 账务边界

| 输入或状态 | 处理原则 |
| --- | --- |
| 金额 | 使用最小货币单位整数字符串；Rust 校验范围，前端用 BigInt 格式化 |
| 不完整输入 | 要求补充必要字段，不能把缺失金额作为零保存 |
| 收入、支出、转账、退款 | 类型独立；转账不计入收支；退款关联同币种原支出 |
| 重复来源 | 保持捕获幂等；疑似重复进入待处理，不重复增加统计 |
| 人工修改 | 保留字段锁和历史，后续自动来源不得静默覆盖 |
| 查询 | 个人范围仅包含本人所有记录，共同范围包含共享投影；时间保存为 UTC、按设备本地日期筛选，币种分别汇总 |
| 网络错误 | 保留本地变更和待发送操作，网络恢复后重试；格式或认证错误暂停对应空间并显示状态 |
| 乱序操作 | 按空间与设备的连续序号推进；缺口未补齐不跳过 |
| 同字段并发修改 | 保留冲突候选供人工选择；不同字段可合并；选择结果再次生成同步操作 |
| 共同操作 | 通过可信成员名册验证申请人；核心修改和共同删除由相应成员确认 |
| 个人删除共享记录 | 个人记录进入回收站，共同快照继续保留；共同审批删除后移除投影，个人恢复不会自动重新共享 |
| 无效密钥、损坏数据 | 报错并保留原始文件，不创建空账本覆盖旧数据 |
| 备份恢复 | 校验口令和数据后事务恢复；失败回滚，成功后重新建立设备信任 |

### 同步与密钥

`personal` 空间授权给本人的设备，`shared` 空间授权给共同账本成员。账单事实与共享投影分别处理，不把对方未共享的个人记录加入共同空间。

WebDAV 使用 HTTPS。配置探测包含目录读取、临时对象写入、读取比对和清理。同步文件加密并认证设备身份，上传失败重试使用原操作的相同密文字节；远端操作经过认证、空间和成员校验后才能进入账务层。

本机数据库密钥与同步凭据通过 Windows 当前用户的 DPAPI 保护。备份使用独立恢复密码；不要把应用数据目录中的设备密钥文件作为跨设备导入方式。

## 设置与主题

| 设置或机制 | 默认值或含义 |
| --- | --- |
| 窗口 | 1200×800，最小尺寸由 Tauri 配置约束，各页面复用同一窗口 |
| 默认币种 | CNY；统计按币种分组 |
| 本地数据 | 当前用户应用本地目录，数据库采用 SQLCipher |
| WebDAV | 初始未配置；读写探测通过后保存凭据 |
| 共享 | 显式选择需要共享的账单 |
| Windows 通知 | 用户单独授权并选择来源，仅读取 |
| OCR | 检查包身份和本机识别语言，缺少能力时返回原因 |
| Windows Hello | 默认关闭；开启后启动和手动锁定时遮挡界面，解锁调用系统验证；属于界面锁，Rust 数据库服务仍运行 |
| 来源保留 | 默认 7 天，核心在命令事务中清理过期原文并保留幂等指纹；支持保留时长设置，尚未提供设置界面 |
| 主题 | 暖白与珊瑚强调色，通过统一 CSS 变量使用 |
| 动效 | 页面共享样式并响应 `prefers-reduced-motion` |

通知仅在用户点击读取时取回已选择来源的现有通知；目前未接入持续通知监听或后台自动入账。权限是否可用与用户是否已授权分开显示。Windows 系统能力由 Rust 提供，WebView 不保存数据库密钥或 WebDAV 凭据。

## 公开 API

当前接口是本仓库前端与 Rust 的内部命令契约，尚未发布独立第三方 SDK。

| Tauri 命令 | 参数 | 返回与错误 |
| --- | --- | --- |
| `ledger_command` | `action: string`、`payload: object` | 成功返回 JSON；业务校验失败以 rejected Promise 返回中文错误 |
| `system_command` | `action: string`、`payload: object` | 文件、设备与 Windows 能力结果；无能力或无授权时返回真实错误 |

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
| Windows | `capabilities`、`ocrImage`、`notificationRequestAccess`、`notificationSources`、`notificationRead`、`helloVerify`、`deviceSettings`、`saveDeviceSettings` |

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
| `exportBackup` / `restoreBackup` | `path`、`password` | 导出 / 恢复结果；错误通过 rejected Promise 返回 |

内部同步应用和身份变更动作只允许受信任的原生装配层调用，不能由页面自由指定成员身份。`syncStatus` 返回 `configured`、`paused`、`lastSync`、设备指纹与空间名册，不返回密码或空间密钥。序列化细节与密文格式以 `contracts/` 为准。

## 当前接入状态

| 项目 | 状态 |
| --- | --- |
| Rust 核心与前端 | 已实现；前端生产构建与 Rust 核心/同步自检通过 |
| 本地加密与备份 | 已接入 SQLCipher、密码备份和 DPAPI 适配 |
| WebDAV 与操作加密 | 已实现网络协议与加密模块，模拟服务验证已通过 |
| Windows OCR / 通知 / Hello | 已实现 Windows API 适配，独立 MSVC 目标类型检查通过，运行行为待 Windows 实机验收 |
| 普通安装程序 | 配置了 Tauri NSIS/MSI 构建；不会自动获得 MSIX 包身份 |
| MSIX | 提供清单及构建签名脚本，需 Windows SDK 和已有受信任发布证书 |
| Windows 整体构建 | 当前 Linux 缺少 MSVC 原生工具，整体构建及安装运行尚未验证；已提供 Windows CI |
| 尚未接入 | Android 客户端、跨端双向联调、持续通知监听、托盘后台、开机启动、远端快照与日志压缩 |
| 账务待完善 | 转账目前仅作独立流水，不维护收付款双账户余额；共同删除后没有共同回收站恢复或到期物理清理；来源模板、识别置信度和专门账户澄清状态机未实现 |
| WebView 交互验收 | 子代理直连 IAB 被工具限制阻断，按项目规则交人工验收 |

当前 IAB 错误为 `IAB visibility is not supported in a subagent thread`；未取得页面状态，不能将前端编译或后端自测称为 UI 验收通过。

## 开发与验证

界面只修改 `frontend/`；账务与同步分别位于两个 Rust crate；系统能力位于 `src-tauri/`。改变跨层契约时先对齐动作、参数和错误语义，再分别修改相关层。中文注释说明业务限制和系统边界。

| 命令或脚本 | 用途 | 环境 |
| --- | --- | --- |
| `npm ci` | 按锁文件安装前端依赖 | Node.js 22 |
| `npm run build` | TypeScript 检查及前端生产构建 | 开发机 |
| `cargo test --locked -p lightledger-core -p lightledger-sync` | 核心和同步内置自检 | Rust 与本地 C/Perl 工具链 |
| `node --experimental-strip-types scripts/check-ui.mjs` | 金额精度、币种小数位及 HTML 转义检查 | Node.js 22 |
| `cargo fmt --all --check` | 检查 Rust 格式 | Rustfmt |
| `npm run tauri -- dev` | 运行 Windows 桌面开发程序 | Windows |
| `npm run tauri -- build --bundles nsis` | 生成 Windows 安装程序 | Windows |
| `scripts/check-platform.sh` | 独立检查 Windows 平台 API 类型 | Rust Windows MSVC target |
| `scripts/windows-msix.ps1` | 构建带包身份的签名 MSIX | Windows SDK 与已有证书 |

2026-09-10 复测代码提交 `179a920`，环境为 Linux x86_64、Rust 1.97.1、Node.js 22.22.2：

| 检查 | 本次结果 |
| --- | --- |
| `npm run build` | TypeScript 检查及 Vite 生产构建通过 |
| `node --experimental-strip-types scripts/check-ui.mjs` | 金额精度、币种小数位和文本转义通过 |
| `cargo fmt --all --check` | 通过 |
| `cargo test --locked -p lightledger-core -p lightledger-sync -- --nocapture` | 核心 6/6、同步 2/2 通过，无失败；分别耗时 12.03 秒、26.62 秒 |
| `CARGO_NET_OFFLINE=true scripts/check-platform.sh` | Windows OCR、通知与 Hello 平台模块的独立 MSVC 目标类型检查通过 |
| `cargo check --locked -p lightledger-app --target x86_64-pc-windows-msvc` | 退出码 101；OpenSSL/ring 原生构建失败，明确报错缺少 `lib.exe`，尚不能确认 Windows 整体编译通过 |
| 浏览器交互 | 子代理直连 IAB 返回 `IAB visibility is not supported in a subagent thread`；已停止，未读取页面或执行操作 |

核心测试覆盖加密数据库、备份错误密码回滚、金额与退款约束、导入幂等和逐行失败、个人/共同删除与审批、跨午夜查询和字段冲突。同步测试覆盖签名与加密、固定密码向量、配对撤销、模拟 WebDAV 重试及连续序号；不代表真实 NAS 或 Windows 运行验收。错误密钥检查产生的 SQLCipher 解密错误日志是预期结果。

本次 50,000 条记录的 debug 内存 SQLCipher 测量：填充约 1.37 秒，分页 50 条加汇总约 359 毫秒，1,000 行导入约 4.34 秒；不是 Windows 磁盘性能或整机验收结果。

MSIX 构建示例（证书已放在当前用户证书库，目标设备已信任签发链）：

```powershell
.\scripts\windows-msix.ps1 -CertificateThumbprint $CertificateThumbprint
```

脚本查找 Windows SDK 的 MakeAppx/SignTool，构建、打包、签名并验证，结果位于 `artifacts/`。需要用户提供真实发布证书指纹；脚本不会安装或信任新证书。普通 NSIS/MSI 包不提供 OCR/通知所需的 MSIX 包身份。

浏览器开发验证连接实际 Rust 核心，并使用临时账本：先运行 `cargo run -p lightledger-core --example dev_bridge`，再为 Vite 设置 `VITE_LEDGER_DEV_BRIDGE=http://127.0.0.1:1421` 后运行 `npm run dev`。页面地址为 `http://127.0.0.1:1420/`；普通生产构建只使用 Tauri 命令桥。

验收遵循项目 `AGENTS.md`：内置浏览器必须读取状态并实际操作，直连失败时停止自动 UI 验收并交人工。Windows 专属能力需在 Windows 实机完成，不能用浏览器预览代替。
