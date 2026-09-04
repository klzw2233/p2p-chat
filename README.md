# p2p-chat

基于 [P2PCore](https://github.com/klzw2233/P2PCore) 的点对点终端聊天示例。端到端加密、设备级身份、带外 SAS 核验。

用词见 [CONTEXT.md](./CONTEXT.md)。架构见 [docs/architecture.md](./docs/architecture.md)。P2PCore 接入笔记见 [notes/](./notes/)。

## 安装

- Rust **1.91+**（与 P2PCore MSRV 对齐）。推荐 [rustup](https://rustup.rs/)。
- Git 能读私有仓库 `klzw2233/P2PCore`（本 crate 用 git 依赖，不是 path）。CI 用 repo secret `P2PCORE_TOKEN`；本地需对那个仓库有读权限。

```bash
git clone git@github.com:klzw2233/p2p-chat.git
cd p2p-chat
```

## 构建

```bash
cargo fmt --all -- --check
cargo test --locked
cargo build
```

CI（GitHub Actions）：`ubuntu-latest`，`cargo fmt --all -- --check` + `cargo test --locked`。Rust 1.91.0 用 `dtolnay/rust-toolchain` 预编译 rustc；`Swatinem/rust-cache`（`shared-key` + `cache-all-crates`）缓存 crates.io 与 `target/`。live `dial`/`accept` 不是 CI 必过项。

## 启动

```text
p2p-chat --temp
p2p-chat --data-dir ./data --password secret
p2p-chat --temp --n0-public
p2p-chat --temp --relay https://relay.example
```

| 标志 | 作用 |
|---|---|
| `--data-dir <PATH>` | 持久化 `identity.key`（Argon2id + ChaCha20Poly1305）和 `trust.store`（签名） |
| `--temp` | 内存 Identity Key，不写盘。都不指定时同样走内存 |
| `--password` / `P2P_PASSWORD` | FileKeyStore 口令。`--data-dir` 且未提供时交互提示 |
| `--relay <URL>` | 自定义 Relay。与 `--n0-public` 互斥 |
| `--n0-public` | 使用 iroh 1.1.0 生产 n0 Relay（硬编码副本，见下方） |

启动后打印 `Peer ID: <64 hex>`，进入 REPL，后台 `accept`。

### Prompt 与按键

TTY 模式的 prompt 会显示当前 Chat Session：

- `[idle]> `：没有活动 Chat Session。
- `[<8-hex>|TOFU]> `：连接到尚未完成 Verified Status 的 Remote Peer。
- `[<8-hex>|Verified]> `：Remote Peer 已通过 `/verify` 标记为 Verified。
- `[<8-hex>|unknown]> `：Trust State 不可用或未知。

其中 `<8-hex>` 是 Remote Peer ID 的前 8 个小写十六进制字符。支持常规 Emacs 风格行编辑；Up/Down 只回忆**当前进程**中输入过的行，不写入历史文件。Ctrl-C 放弃当前 draft 并显示新 prompt，不会退出进程或关闭 Chat Session；Ctrl-D、`/quit` 与 `/exit` 才会干净退出。raw mode 下 Ctrl-C 不再向进程发送 SIGINT。

当 stdin 或 stdout 任一被 pipe 或重定向时，程序回退到普通逐行模式：不显示 prompt，普通输出写 stdout，错误写 stderr，也不会进入 raw mode。`/dial` 等待解析期间 prompt 暂停渲染；按键会排队并在操作完成后处理，不会丢失。

## 双实例本地测试

跨 NAT / 不同机器用 `--n0-public`（或双方同一个 `--relay`）。不要把依赖公网 n0 的 live 拨号当 CI 必过项。

**终端 A**

```bash
cargo run -- --temp --n0-public
```

记下打印的 64 字符 hex Peer ID。

**终端 B**

```bash
cargo run -- --temp --n0-public
```

```text
/dial <A 的 Peer ID>
hello
/sas
```

A 侧应看到 inbound Chat Session，并打出 `< hello`。两边都跑 `/sas`，带外核对 SAS Display（8 组 5 位数字）。一致后各自 `/verify`，再用 `/info` 看 Trust State 是否为 Verified。

### TTY 双终端人工验证记录（2026-09-04）

以下流程已由两个终端使用 `cargo run -- --temp --n0-public` 实际验证：

1. A、B 成功建立 Chat Session；双方均显示 8 字符 Remote Peer ID 与 `TOFU` prompt。
2. 双方消息和 SAS Display 能互相收发，SAS 一致。
3. `/verify` 成功后，双方 prompt 立即变为 `Verified`。
4. 对端发送消息时，正在编辑的输入仍可继续使用，终端没有发生输入中断或丢失；Ctrl-C 可放弃当前 draft 而不退出进程。
5. Ctrl-D 可结束输入；对端断开后显示错误/断开信息，prompt 回到 `[idle]> `，后续输入无 Chat Session 时得到预期错误。
6. 终端退出后 shell 保持可用，无需执行 `stty sane`。

结束：`/close` 只关当前 Chat Session；`/quit` 或 `/exit` 退出进程。

同机、无 Relay 时（`cargo run -- --temp`）只在 iroh 能自己找到对端时才通，NAT 后通常不行。

## Relay

| 模式 | 何时用 |
|---|---|
| 默认（无标志） | `RelayConfig::disabled()`。不断官方中继 |
| `--n0-public` | 开发期跨网。bind 用 `RelayConfig::n0_public()`；dial 的 `DialHints` 抄 iroh 1.1.0 四个 hostname（p2p-core 不暴露该列表） |
| `--relay <URL>` | 自建 Relay。**两端用同一个 URL** |

`--relay` 与 `--n0-public` 互斥。iroh 改默认 n0 URL 时要对一下 `src/main.rs` 里的副本。

## Slash Commands

非 `/` 开头的行当作聊天文本发送（无 Chat Session 时会报错）。

| 命令 | 作用 |
|---|---|
| `/dial <peer-id>` | 向 Remote Peer 拨号（64 hex，大小写不敏感） |
| `/sas` | 展示 SAS Display |
| `/verify` | 把当前 Remote Peer 标为 Verified |
| `/info` | Local / Remote Peer ID 与 Trust State |
| `/close` | 结束当前 Chat Session，继续听 inbound |
| `/help` | 命令列表 |
| `/quit`、`/exit` | 干净退出 |

同一时刻只有一条活动 Chat Session。再 `/dial` 先 `/close`。

## 帧格式（ADR-0001）

`[u32 BE length][JSON ChatMessage]`，单帧上限 1 MiB。

```json
{ "text": "hello", "timestamp": 1700000000 }
```

## 文档

- [CONTEXT.md](./CONTEXT.md) — 术语
- [HANDOFF.md](./HANDOFF.md) — 进度与已知缺口
- [docs/architecture.md](./docs/architecture.md)
- [docs/adr/0001-length-prefixed-json-framing.md](./docs/adr/0001-length-prefixed-json-framing.md)
- [notes/](./notes/) — 应用层怎么接 P2PCore
