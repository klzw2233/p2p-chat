# p2p-chat

基于 [P2PCore](https://github.com/klzw2233/P2PCore) 的点对点终端聊天示例。端到端加密、设备级身份、带外 SAS 核验。

**状态（2026-09-02）：** Ticket 2 完成——CLI 参数、Identity Key 持久化 / 临时模式、启动打印 64 字符 hex Peer ID。交互式 REPL 尚未落地。`cargo run -- --temp` 会 bind、打印 Peer ID、退出。

用词见 [CONTEXT.md](./CONTEXT.md)。架构见 [docs/architecture.md](./docs/architecture.md)。交接见 [HANDOFF.md](./HANDOFF.md)。

## 构建

Rust 1.91+（与 P2PCore MSRV 对齐）。

```bash
cargo fmt --all -- --check
cargo test --locked
cargo run -- --temp
cargo run -- --data-dir ./data --password secret
```

依赖 `p2p-core` / `p2p-trust`，来源 `https://github.com/klzw2233/P2PCore.git`。

CI（GitHub Actions）：`ubuntu-latest`，`cargo fmt --all -- --check` + `cargo test --locked`。Rust 1.91.0 用 `dtolnay/rust-toolchain` 预编译工具链；`Swatinem/rust-cache` 缓存依赖。live `dial`/`accept` 不是 CI 必过项。`P2PCore` 是私有仓库，test job 需要 repo secret `P2PCORE_TOKEN`（能读 `klzw2233/P2PCore` 的 PAT）。

## 帧格式（ADR-0001）

`[u32 BE length][JSON ChatMessage]`，单帧上限 1 MiB。

```json
{ "text": "hello", "timestamp": 1700000000 }
```

## CLI（Ticket 2）

```text
p2p-chat --temp
p2p-chat --data-dir ./data --password secret --relay https://relay.example
p2p-chat --n0-public --temp
```

- `--data-dir`：`identity.key`（Argon2id + ChaCha20Poly1305）+ `trust.store`（签名）。密码来自 `--password`、`P2P_PASSWORD`，或交互提示。
- `--temp` 或都不指定：内存 Identity Key，不写盘。
- `--relay` 与 `--n0-public` 互斥；默认无官方中继。
- 启动后打印 `Peer ID: <64 hex>`。

计划中的 REPL 斜杠命令（Ticket 3）：`/dial <hex>` `/sas` `/verify` `/info` `/close` `/help` `/quit`。

## 文档

- [CONTEXT.md](./CONTEXT.md) — 术语
- [HANDOFF.md](./HANDOFF.md) — 当前进度与下一步
- [docs/architecture.md](./docs/architecture.md)
- [docs/adr/0001-length-prefixed-json-framing.md](./docs/adr/0001-length-prefixed-json-framing.md)
