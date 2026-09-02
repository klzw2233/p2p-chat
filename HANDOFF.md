# 工作状态交接

**最后更新: 2026-09-02**
**当前阶段: Ticket 2 编码完成，未合入 main。下一步是 Ticket 3（REPL / SAS / 斜杠命令）。**

本文件供接手的 Claude Code 会话阅读。先读 [CONTEXT.md](./CONTEXT.md)，再读本文，再读 [docs/architecture.md](./docs/architecture.md)。

---

## 给接手者的规则

1. **用词以 [CONTEXT.md](./CONTEXT.md) 为准。** Local Peer / Remote Peer / Peer ID / Chat Session / Slash Command / SAS Display / Verified Status。不要写成好友、用户、房间。
2. **不要重新讨论已定的决定。** 见下方「已锁定」。推翻则写新 ADR。
3. **Git 工作流：** 当前在 `chore/github-actions-ci`，不要直接改 `main`。
4. **Ponytail：** 最短能用的实现。不要提前写 REPL / `/dial` / SAS。那些是 Ticket 3。

---

## 仓库与追踪

| 项 | 位置 |
|---|---|
| Remote | `git@github.com:klzw2233/p2p-chat.git` |
| 分支 | `chore/github-actions-ci`（相对 `main` @ `2280c2b`） |
| Spec | [#1](https://github.com/klzw2233/p2p-chat/issues/1) |
| Ticket 1 脚手架 + 帧 | [#2](https://github.com/klzw2233/p2p-chat/issues/2) — **已关**，合入 `main` via #6 |
| Ticket 2 CLI / 身份 / 持久化 | [#3](https://github.com/klzw2233/p2p-chat/issues/3) — **已关**，合入 `main` via #7 |
| Ticket 3 REPL / SAS / 斜杠命令 | [#4](https://github.com/klzw2233/p2p-chat/issues/4) — blocked by #3 |
| Ticket 4 文档 | [#5](https://github.com/klzw2233/p2p-chat/issues/5) — blocked by #4 |
| 底层库 | `https://github.com/klzw2233/P2PCore.git`（git 依赖，commit 以 `Cargo.lock` 为准） |
| 本地检出 | 同级目录 `../P2PCore`（只读参考；本 crate 不要 path 依赖） |

Issue tracker 约定：`docs/agents/issue-tracker.md`。关票用 `gh issue close`，并在 body 里写清验证了什么。

---

## 已锁定的决定（勿重开）

| 决策 | 结论 |
|---|---|
| 形态 | 终端 CLI / REPL，不是 GUI / Web |
| 依赖 | `p2p-core` + `p2p-trust` 用 **git** URL，不是 path |
| Relay | `--relay <url>` / `--n0-public` / 默认无官方中继 |
| 存储 | `--data-dir` 持久化；`--temp` 内存。密码：`--password` / `P2P_PASSWORD` / 交互提示 |
| 帧 | 4 字节大端长度 + JSON，见 [ADR-0001](./docs/adr/0001-length-prefixed-json-framing.md) |
| Peer ID 展示 | 64 字符 hex，界面可截断 |
| REPL | 单进程：后台 `accept`，前台 `/dial` `/sas` `/verify` `/info` `/close` `/help` `/quit`，非斜杠当聊天文本 |
| 范围外 | 群聊、离线投递、文件传输、GUI |

grilling 记录在会话里，不单独成文。

---

## Ticket 2 落地了什么

已有代码（叠在 Ticket 1 帧层之上）：

- `src/cli.rs`：`clap` 解析 `--data-dir` / `--temp` / `--password`（env `P2P_PASSWORD`）/ `--relay` / `--n0-public`。`--data-dir` 与 `--temp` 互斥，`--relay` 与 `--n0-public` 互斥。
- `src/store.rs`：`--data-dir` → `FileKeyStore` + `FileTrustStore`（先 load/generate 身份，再 `FileTrustStore::open`）；无 `--data-dir`（含 `--temp`）→ `MemoryKeyStore` + `MemoryTrustStore`。`peer_id_hex` 输出 64 字符小写 hex。
- `src/main.rs`：bind 后打印 `Peer ID: <hex>`，然后 `close`。无 `--password`/`P2P_PASSWORD` 且用了 `--data-dir` 时 `rpassword` 提示。REPL 仍是 Ticket 3。
- 测试：`tests/cli_args.rs`、`tests/identity.rs`、`tests/cli_bin.rs`

未加 `hex` crate：`format!("{b:02x}")` 足够。

---

## 已知缺口（接手后不要踩）

1. **Live Session 集成测目前 CI 跑不通。**
   - `p2p-core` 的双端测试靠 `iroh::test_utils::run_relay_server()` + `RelayConfig::with_insecure_tls()`。后者是 `#[cfg(test)]` **私有**方法，应用 crate 调不到。
   - `RelayConfig::n0_public()` + `DialHints::none()` 在本环境约 20s 超时（曾经试过，已从测试里删掉，避免 CI 空等）。
   - Ticket 1 验收里的「in-process 双端收发」因此改成：帧编解码在纯函数上测；真正的 `dial`/`accept` 等 P2PCore 暴露测试 Relay 钩子，或 Ticket 3 用两个进程人工验证。
   - **不要**为了绿测去改 P2PCore，除非单独开 P2PCore issue。也不要再加依赖 n0 公网的测试当必过项。

2. **GitHub Actions CI 已落地。** `.github/workflows/ci.yml`：`checkout@v5`（Node 24）、`dtolnay/rust-toolchain@master` + 1.91.0 预编译 rustc、`Swatinem/rust-cache@v2`（`shared-key: test` + `cache-all-crates`，缓存 crates.io 源码和 `target/`）、`cargo fmt --all -- --check`、`cargo test --locked`。没有 `cargo install`。test job 用 `CARGO_NET_GIT_FETCH_WITH_CLI` + secret `P2PCORE_TOKEN` 拉私有 git 依赖 P2PCore。冷启动会编整棵 iroh 树并写入 ~467MB cache；之后同 lockfile 应命中。live Session 仍不是 CI 必过项。

3. **`main.rs` 还不是聊天客户端。** 跑 `cargo run -- --temp` 会 bind、打印 Peer ID、退出。stdin REPL 是 Ticket 3。

---

## 建议的下一步（Ticket 3 = #4）

按 #4 做，不要提前做群聊 / 文件：

1. stdin 行循环：斜杠命令 vs 聊天文本
2. `/dial <hex>` `/sas` `/verify` `/info` `/close` `/help` `/quit`
3. 后台 `accept` + 前台输入，`tokio::select!`
4. SAS 展示与 `mark_verified`

Ticket 4 再收文档。

---

## 动手前必读（按顺序）

1. [CONTEXT.md](./CONTEXT.md)
2. 本文
3. [docs/architecture.md](./docs/architecture.md)
4. [docs/adr/0001-length-prefixed-json-framing.md](./docs/adr/0001-length-prefixed-json-framing.md)
5. P2PCore：`../P2PCore/notes/04-app-layer.md` 和 `../P2PCore/CONTEXT.md`
6. GitHub `#4`
