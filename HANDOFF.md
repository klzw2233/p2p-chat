# 工作状态交接

**最后更新: 2026-09-02**
**当前阶段: Ticket 1 编码完成，未合入 main。下一步是 Ticket 2（CLI / 身份持久化）。**

本文件供接手的 Claude Code 会话阅读。先读 [CONTEXT.md](./CONTEXT.md)，再读本文，再读 [docs/architecture.md](./docs/architecture.md)。

---

## 给接手者的规则

1. **用词以 [CONTEXT.md](./CONTEXT.md) 为准。** Local Peer / Remote Peer / Peer ID / Chat Session / Slash Command / SAS Display / Verified Status。不要写成好友、用户、房间。
2. **不要重新讨论已定的决定。** 见下方「已锁定」。推翻则写新 ADR。
3. **Git 工作流：** 当前在 `feat/initial-p2p-chat`，不要直接改 `main`。
4. **Ponytail：** 最短能用的实现。不要提前写 REPL / clap / 文件存储。那些是后面的 ticket。

---

## 仓库与追踪

| 项 | 位置 |
|---|---|
| Remote | `git@github.com:klzw2233/p2p-chat.git` |
| 分支 | `feat/initial-p2p-chat`（相对空 `main` 的第一批提交） |
| Spec | [#1](https://github.com/klzw2233/p2p-chat/issues/1) |
| Ticket 1 脚手架 + 帧 + headless 循环 | [#2](https://github.com/klzw2233/p2p-chat/issues/2) — **代码已写，Issue 未关** |
| Ticket 2 CLI / 身份 / 持久化 | [#3](https://github.com/klzw2233/p2p-chat/issues/3) — blocked by #2 |
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

## Ticket 1 落地了什么

已有代码：

- `src/frame.rs`：`ChatMessage`、`encode_frame` / `decode_frame`、`write_frame` / `read_frame`、1 MiB 上限
- `src/lib.rs`：只导出 `frame`
- `src/main.rs`：占位 `println`，真正的 binary 在 Ticket 3
- `tests/headless_session.rs`：只断言帧字节形状（长度前缀 + JSON）
- `Cargo.toml`：git 依赖 P2PCore；tokio 只开了后续 REPL 会用的 features，Ticket 2 再加 `clap` / `hex` / `rpassword`

`cargo test`（2026-09-02）8 个测试全过：7 个 `frame` 单测 + 1 个 crate 测试。

---

## 已知缺口（接手后不要踩）

1. **Live Session 集成测目前 CI 跑不通。**
   - `p2p-core` 的双端测试靠 `iroh::test_utils::run_relay_server()` + `RelayConfig::with_insecure_tls()`。后者是 `#[cfg(test)]` **私有**方法，应用 crate 调不到。
   - `RelayConfig::n0_public()` + `DialHints::none()` 在本环境约 20s 超时（曾经试过，已从测试里删掉，避免 CI 空等）。
   - Ticket 1 验收里的「in-process 双端收发」因此改成：帧编解码在纯函数上测；真正的 `dial`/`accept` 等 P2PCore 暴露测试 Relay 钩子，或 Ticket 3 用两个进程人工验证。
   - **不要**为了绿测去改 P2PCore，除非单独开 P2PCore issue。也不要再加依赖 n0 公网的测试当必过项。

2. **`#2` 还没 close。** 提交后：在 #2 评论实测结果（`cargo test` 8 passed），说明 live Session 缺口，再 `gh issue close 2`。关了 #2 才算 #3 解阻。

3. **`main.rs` 还不是聊天客户端。** 跑 `cargo run` 只会打印一行占位。

---

## 建议的下一步（Ticket 2 = #3）

按 #3 做，不要提前做 REPL：

1. `clap`：`--data-dir`、`--temp`、`--password`（env `P2P_PASSWORD`）、`--relay`、`--n0-public`
2. `--data-dir`：`FileKeyStore` + `FileTrustStore`（先从 KeyStore load/generate 身份，再 `FileTrustStore::open`，见 P2PCore `notes/04-app-layer.md`）
3. `--temp`：`MemoryKeyStore` + `MemoryTrustStore`，不写盘
4. `Endpoint::bind` 后打印 64 字符 hex Peer ID
5. 测：持久化重启 Peer ID 不变；密码错；`--temp` 无文件

Ticket 3 再接 stdin 循环、`/dial`、SAS、`mark_verified`。

---

## 动手前必读（按顺序）

1. [CONTEXT.md](./CONTEXT.md)
2. 本文
3. [docs/architecture.md](./docs/architecture.md)
4. [docs/adr/0001-length-prefixed-json-framing.md](./docs/adr/0001-length-prefixed-json-framing.md)
5. P2PCore：`../P2PCore/notes/04-app-layer.md` 和 `../P2PCore/CONTEXT.md`
6. GitHub `#3`
