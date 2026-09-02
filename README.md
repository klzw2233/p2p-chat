# p2p-chat

基于 [P2PCore](https://github.com/klzw2233/P2PCore) 的点对点终端聊天示例。端到端加密、设备级身份、带外 SAS 核验。

**状态（2026-09-02）：** Ticket 1 完成——应用层长度前缀 JSON 帧。交互式 REPL 尚未落地。`cargo run` 目前只打印占位行。

用词见 [CONTEXT.md](./CONTEXT.md)。架构见 [docs/architecture.md](./docs/architecture.md)。交接见 [HANDOFF.md](./HANDOFF.md)。

## 构建

Rust 1.91+（与 P2PCore MSRV 对齐）。

```bash
cargo test
cargo run
```

依赖 `p2p-core` / `p2p-trust`，来源 `https://github.com/klzw2233/P2PCore.git`。

## 帧格式（ADR-0001）

`[u32 BE length][JSON ChatMessage]`，单帧上限 1 MiB。

```json
{ "text": "hello", "timestamp": 1700000000 }
```

## 计划中的 CLI（Ticket 2–3）

```text
p2p-chat --temp
p2p-chat --data-dir ./data --password secret --relay https://relay.example
p2p-chat --n0-public --temp
```

REPL 斜杠命令：`/dial <hex>` `/sas` `/verify` `/info` `/close` `/help` `/quit`。

## 文档

- [CONTEXT.md](./CONTEXT.md) — 术语
- [HANDOFF.md](./HANDOFF.md) — 当前进度与下一步
- [docs/architecture.md](./docs/architecture.md)
- [docs/adr/0001-length-prefixed-json-framing.md](./docs/adr/0001-length-prefixed-json-framing.md)
