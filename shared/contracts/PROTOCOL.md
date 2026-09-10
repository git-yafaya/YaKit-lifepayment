# 轻账互通协议 v1

Windows 与 Android 工程共用本目录协议及 `shared/crates/ledger-sync`；Android 跨端双向联调尚未完成。协议变更必须增加版本，不能按语言默认序列化猜测认证内容。

## 数据及目录

UUID 使用小写带连字符字符串；金额使用非负十进制字符串；时间使用 UTC RFC3339 字符串。序号是每个空间、设备独立分配的正整数，线上 JSON 数值必须在接收端保持 64 位精度。操作源于本地数据库事务 Outbox。

| 对象 | 路径及行为 |
| --- | --- |
| 账务操作 | `LightLedger/v1/spaces/{spaceId}/ops/{deviceId}/{sequence:020}.enc` |
| 管理员授权或撤销 | `LightLedger/v1/spaces/{spaceId}/control/{revision:020}.json` |
| 重试 | `If-None-Match: *` 创建；412 后 GET 比较完整字节，一致才记上传成功 |
| 下载 | 先写本地 `.incoming` 密文，再验证、应用；失败保留密文且不前进游标 |
| 序号缺口 | 后续文件不能越过缺口；再次运行时重新拉取 |
| 未知版本 | 返回 `unsupported-version`，停止本空间本轮；本地账务保留 |
| 删除远端对象 | 不转换为删除本地账单；同步不清理历史操作 |

远端文件名、设备公钥列表和文件长度不是秘密。账务载荷始终加密。个人空间与各共同空间使用独立的随机 32 字节密钥。个人邀请只允许同成员，新设备必须在空本上接受成员身份。共同邀请仅授予指定共同空间。

## 密码套件与确切认证字节

所有 Base64 使用 RFC4648 标准字母表、带 `=` 补齐。RSA 密钥为 3072 bit；公钥为 SubjectPublicKeyInfo DER，私钥为 PKCS8 DER。本机私钥与 WebDAV 凭据由 DPAPI CurrentUser 封装，不写入邀请明文。

1. `header` 是 Header JSON 原始 UTF-8 字节的 Base64。字段依次是 `version, spaceId, deviceId, memberId, sequence, operationId, epoch`，但验证方直接使用传输字节，不反序列化再序列化。
2. `nonce` 是 CSPRNG 生成的 12 字节，单次加密随机生成。每个待发操作信封首次生成后持久化，重试复用完整字节。
3. 密码算法 AES-256-GCM；AAD 为 header 解码原始字节；`ciphertext` 是密文后接 16 字节认证标签。
4. 签名字节是 UTF-8 `LightLedger/operation/v1`、一个 NUL 字节、header 字节长度的 4 字节无符号大端表示、header 原始字节、12 字节 nonce、ciphertext+tag，顺序拼接。
5. `signature` 是 RSA-PSS SHA-256 签名，MGF1 SHA-256，盐长度 32 字节。先 SHA-256 上一步的字节串，再 PSS 签名摘要。验证签名、成员授权、版本、空间后解密；认证头必须与解密操作的 id/spaceId/deviceId/memberId/sequence 一致。
6. 空间密钥封装使用 RSA-OAEP SHA-256，MGF1 SHA-256，空 label。解封对象是原始 32 字节空间密钥。
7. 指纹是 SHA-256(SPKI DER) 的标准 Base64。批准和接受双方分别由用户核对指纹。

设备列表中的 revoked 设备不再允许提交操作。撤销递增 epoch，生成全新空间密钥，仅为剩余设备封装；旧数据已读出后无法收回。控制消息 revision 连续递增，密钥轮换 epoch 不可倒退或跨越版本。旧 epoch 的已批准设备历史仍可重放。

## 配对文件

Request: `{version:1,id,expires,memberId,deviceId,publicKey}`，expires 为 Unix 秒，默认 10 分钟。批准端持久化已使用 invitation ID，过期或重放拒绝。邀请不带 WebDAV 密码。

Grant: `{body,signature,signerPublicKey}`。body 是 GrantBody JSON 原始 UTF-8 字节的 Base64；signature 对 **body 字符串的 UTF-8 字节** 做上述 RSA-PSS SHA-256。GrantBody 包含 version/invitationId/expires/recipient/space/wrappedKeys；space.keys 为空，wrappedKeys 每个 epoch 都为接收方 OAEP 封装。

控制消息复用 Grant 外壳；body 解码为 `{version:1,space,wrappedKeys}`，wrappedKeys 按 deviceId 索引，只含当前 epoch。space.keys 为空。只有已信任的 adminDeviceId 公钥可以签署。收到连续 revision 后保留已有历史密钥并加入当前密钥。管理员新增设备时向其他设备广播控制消息，加入方以邀请内最新 revision 为基线。

## 账务应用

操作完整 JSON 原样交给核心，包含 id/spaceId/entityId/deviceId/memberId/sequence/payload/patch/baseVersions。payload 为完整账单，patch 为本次修改的字段。baseVersions 是每字段的已知版本。不同字段合并；同字段基础版本不匹配保留冲突待办；解决冲突经核心再生成新操作。账务变更、已应用 ID、连续游标在同一事务中提交。收件箱不再送入 OCR。

## 验证与限制

`cargo test -p lightledger-sync` 执行两个身份之间的加密/配对/篡改/空间隔离/设备撤销，以及本地模拟 WebDAV 的探测、失败重试、幂等、乱序缺口。测试 HTTP 仅内部测试构造器允许；生产接口强制 HTTPS，保持系统 TLS 证书验证并禁止重定向。

`examples/crypto-vector.json` 提供固定的合成密钥、公开身份、信封与预期明文，由测试验证解密与验签。尚无独立 Android 实现双向测试、服务器快照及日志自动压缩。真实 WebDAV 服务、Windows DPAPI、安装包和系统 API 仍需 Windows 实机验收。
