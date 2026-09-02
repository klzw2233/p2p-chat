# 用 P2PCore 接 p2p-chat

对照底层库 `../P2PCore/notes/04-app-layer.md`（相对本仓库根，本地只读检出）。本 crate **不** `use iroh::…`，日常碰 `p2p-core`；算 SAS、落盘时才碰 `p2p-trust`。依赖是 git URL，不是 path。

## bind

`store::bind_endpoint` 按 CLI 选存储，再 `Endpoint::bind`。

| 标志 | KeyStore / TrustStore |
|---|---|
| `--data-dir` | `FileKeyStore` + `FileTrustStore` |
| `--temp` 或都不指定 | `MemoryKeyStore` + `MemoryTrustStore` |

持久化必须 **先** `keys.load()`（没有则 `IdentityKey::generate` + `save`），再用同一把 Identity Key `FileTrustStore::open`。密码错 → `Error::Trust` / `UnlockFailed`。落盘文件是 `{dir}/identity.key` 与 `{dir}/trust.store`。

Relay 在 **bind** 时定：`--n0-public` → `RelayConfig::n0_public()`；`--relay` → `RelayConfig::custom`；默认 `RelayConfig::disabled()`。生产默认不断 n0，必须显式 opt-in。

## dial 的 DialHints 不是 bind 的 RelayConfig

`endpoint.dial(peer, hints)` 要另传 `DialHints`。p2p-core **不暴露** iroh 的 n0 URL 列表，所以 `--n0-public` 时 `main.rs` 抄了 iroh 1.1.0 四个 hostname 塞进 `DialHints::relays`。`--relay` 把同一个 URL 传给 DialHints。空列表 → `DialHints::none()`。

两端要能互相找到：共享 **Peer ID + 同一套 Relay**（或局域网里 iroh 自己能发现）。没有 Address Lookup。

## 帧是应用的事

Session 是无边界字节流。`frame.rs` 实现 ADR-0001：`[u32 BE length][JSON ChatMessage]`，1 MiB 上限。`write_frame` / `read_frame` 调 `session.send` / `recv` / `recv_exact`。干净 EOF → `Ok(None)`。库不帮你加长度前缀。

## 信任是 UI，不是线上协议

库不发「我已验证你」。本 REPL：

1. 粘贴 Peer ID → `/dial`（首次成功记 TOFU）。inbound `accept` 同样。
2. `/sas` → `p2p_trust::sas(本端公钥, 对端公钥)`，打印 8×5 位数字。两端各自算，必然相同（库已规范排序）。
3. 人用电话 / 当面核对。库不检查他们是否真对过。
4. `/verify` → `endpoint.mark_verified`。这是 TOFU → Verified **唯一**升级路径。

TOFU **挡不住第一次冒充**，只能发现以后钥匙换了。本示例不实现 `introduce` / `accept_tofu_replacement`；`Rejected` / `Alert` 只 `eprintln`。

## 单活动 Chat Session

P2PCore：同一 Remote Peer 同时最多一条 Session。本 REPL 更窄：**整个进程一条**。有 Session 时不 `accept`、拒绝再 `/dial`。`/close` 后重新听 inbound。

调度是 `tokio::select!`：stdin、`accept`（无 Session）、`read_frame`（有 Session）。见 `app.rs`。

## 拨号失败怎么显示

对照库的错误分型，这里全是打印后继续 REPL，不自动重试：

| `p2p-core::Error` | 用户侧 |
|---|---|
| `PeerOffline` | 对端不在线（约 5s 内返回） |
| `RelayUnreachable` | 没配 Relay 或 Relay 不可达 |
| `AlreadyConnected` | 这条 Peer 已有 Session |
| `Rejected` / `Alert` | 钥匙对不上；没有 Session 交给你 |
| 其它 | bind / 流 / 关闭等 |

## 明确不做

群聊、离线投递、文件传输、GUI。那些会挤掉这条最小回路：bind → 交换 Peer ID → dial/accept → 帧 → SAS → verify。
