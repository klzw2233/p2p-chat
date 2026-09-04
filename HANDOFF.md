# 工作状态交接

**最后更新: 2026-09-02**
**当前阶段: Tickets 1–4 与 Spec #1 已全部关闭。**

本文件供接手的 Claude Code 会话阅读。先读 [CONTEXT.md](./CONTEXT.md)，再读本文，再读 [docs/architecture.md](./docs/architecture.md)。

---

## 给接手者的规则

1. **用词以 [CONTEXT.md](./CONTEXT.md) 为准。** Local Peer / Remote Peer / Peer ID / Chat Session / Slash Command / SAS Display / Verified Status。不要写成好友、用户、房间。
2. **不要重新讨论已定的决定。** 见下方「已锁定」。推翻则写新 ADR。
3. **Git 工作流：** 不要直接改 `main`。
4. **Ponytail：** 最短能用的实现。不要提前写群聊 / 文件传输。那些不在范围内。

---

## 仓库与追踪

| 项 | 位置 |
|---|---|
| Remote | `git@github.com:klzw2233/p2p-chat.git` |
| Spec | [#1](https://github.com/klzw2233/p2p-chat/issues/1) — **已关** |
| Ticket 1 脚手架 + 帧 | [#2](https://github.com/klzw2233/p2p-chat/issues/2) — **已关**，合入 `main` via #6 |
| Ticket 2 CLI / 身份 / 持久化 | [#3](https://github.com/klzw2233/p2p-chat/issues/3) — **已关**，合入 `main` via #7 |
| Ticket 3 REPL / SAS / 斜杠命令 | [#4](https://github.com/klzw2233/p2p-chat/issues/4) — **已关**，合入 `main` via #9 |
| Ticket 4 文档 | [#5](https://github.com/klzw2233/p2p-chat/issues/5) — README + `notes/` |
| Spec #12 Terminal REPL Presentation | [#12](https://github.com/klzw2233/p2p-chat/issues/12) — **进行中** |
| Ticket 5 Presentation Layer | [#13](https://github.com/klzw2233/p2p-chat/issues/13) — **已关** |
| Ticket 6 Dual-Mode Event Loop | [#14](https://github.com/klzw2233/p2p-chat/issues/14) — **已关** |
| Ticket 7 文档与人工验证 | [#15](https://github.com/klzw2233/p2p-chat/issues/15) — **进行中** |
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
| REPL | 单进程：后台 `accept`，前台 `/dial` `/sas` `/verify` `/info` `/close` `/help` `/quit`，非斜杠当聊天文本；TTY 使用 `rustyline-async`，非 TTY 使用普通逐行模式 |
| 范围外 | 群聊、离线投递、文件传输、GUI |

grilling 记录在会话里，不单独成文。

---

## 落地了什么

- Ticket 1：`frame.rs` 长度前缀 JSON。
- Ticket 2：`cli.rs` / `store.rs`，身份持久化。
- Ticket 3：`command.rs` + `app.rs` REPL；`/sas` → `p2p_trust::sas`；`/verify` → `endpoint.mark_verified`。
- Ticket 4：README 安装 / 构建 / 双实例 / Relay / Slash Commands；`notes/01-p2pcore-integration.md`。
- Ticket 5：`ui.rs` presentation layer、纯 `prompt_for`、TTY / 非 TTY 输入。
- Ticket 6：`app.rs` 接入单一 dual-mode event loop 与 interruption-free prompt。
- Ticket 7：架构、使用说明与双终端人工验证同步。

---

## 已知缺口（接手后不要踩）

1. **Live Session 集成测目前 CI 跑不通。**
   - `p2p-core` 的双端测试靠 `iroh::test_utils::run_relay_server()` + `RelayConfig::with_insecure_tls()`。后者是 `#[cfg(test)]` **私有**方法，应用 crate 调不到。
   - `RelayConfig::n0_public()` 的 live 拨号在本环境约 20s 超时（曾经试过，已从测试里删掉，避免 CI 空等）。
   - **不要**为了绿测去改 P2PCore，除非单独开 P2PCore issue。也不要再加依赖 n0 公网的测试当必过项。
   - 人工验证：两进程 `cargo run -- --temp --n0-public`，把 A 的 Peer ID 贴到 B 的 `/dial`。见 README。

2. **GitHub Actions CI 已落地。** `.github/workflows/ci.yml`：`checkout@v5`（Node 24）、`dtolnay/rust-toolchain@master` + 1.91.0 预编译 rustc、`Swatinem/rust-cache@v2`（`shared-key: test` + `cache-all-crates`）、`cargo fmt --all -- --check`、`cargo test --locked`。test job 用 `CARGO_NET_GIT_FETCH_WITH_CLI` + secret `P2PCORE_TOKEN` 拉私有 git 依赖 P2PCore。live Session 仍不是 CI 必过项。

3. **`--n0-public` 的 DialHints URL 是硬编码副本。** p2p-core 不暴露 iroh 的 n0 URL 列表；应用抄了 `iroh::defaults::prod` 四个 hostname。iroh 改默认时要对一下。

4. **已知的展示限制：`/dial` 会暂时阻塞渲染。** `endpoint.dial()` 等待期间约 20s（`--n0-public`）没有轮询 readline；输入会排队但不会丢失。若未来需要无冻结交互，应将 dial 放入 `tokio::spawn`，通过 oneshot completion channel 作为第四个 `select!` 分支。
   - 这是当前 ticket 的刻意边界，不要在没有需求时提前扩展。

## 建议的下一步

Tickets 1–4 与 #1 均已关闭。新工作请开新 issue。不要提前做群聊 / 文件。
---

## 动手前必读（按顺序）

1. [CONTEXT.md](./CONTEXT.md)
2. 本文
3. [docs/architecture.md](./docs/architecture.md)
4. [docs/adr/0001-length-prefixed-json-framing.md](./docs/adr/0001-length-prefixed-json-framing.md)
5. [notes/01-p2pcore-integration.md](./notes/01-p2pcore-integration.md)
6. P2PCore：`../P2PCore/notes/04-app-layer.md` 和 `../P2PCore/CONTEXT.md`
