# p2p-chat 软件架构设计说明书 (Architecture Design Document)

## 1. 系统概述 (System Overview)

`p2p-chat` 是基于底层 `P2PCore` 传输与信任库构建的点对点（P2P）终端即时通信参考应用。它通过 Rust 异步运行时（Tokio）与命令行交互界面（REPL），向开发者与用户完整演示点对点 NAT 穿透打洞、端到端加密通道建立、身份长期密钥持久化、首次使用信任（TOFU）以及带外安全码（SAS）人工核验等核心流程。

---

## 2. 架构设计原则 (Design Principles)

1. **职责分层清晰 (Layered Separation)**：底层密码学与传输协议完全委托给 `P2PCore` 与 `p2p-trust`，本应用仅关注应用层协议帧封装、会话生命周期管理与终端人机交互。
2. **异步非阻塞 (Async & Non-blocking Concurrency)**：后台监听接入、远端消息读取、本地终端按行输入解耦并行，避免任何单点 I/O 阻塞整体交互。
3. **确定性安全 (Deterministic Security Guarantee)**：严格遵循零信任模型，不妥协端到端机密性与认证性；身份公钥即地址，带外比对 SAS 升级至 Verified 状态。
4. **极简与可测性 (Simplicity & Testability)**：遵循 Ponytail 原则，不做未请求的过度设计与抽象。帧编解码在纯函数缝隙上测（`encode_frame` / `decode_frame`），不依赖 Relay。跨 Peer 的 Session 集成测依赖 `p2p-core` 的测试专用 Relay TLS 跳过，当前库尚未对该能力开放公开 API，因此 Ticket 1 不把 live dial/accept 作为 CI 必过项。

---

## 3. 分层架构视图 (Layered Architecture)

```
+-------------------------------------------------------------------------+
|                         User / Terminal (TTY)                           |
+-------------------------------------------------------------------------+
                                    ▲
                                    │ CLI Input / Terminal Output
                                    ▼
+-------------------------------------------------------------------------+
|                     Application Layer (p2p-chat)                        |
|                                                                         |
|  +-------------------------------------------------------------------+  |
|  |                     REPL & Command Dispatcher                     |  |
|  |       (/dial, /sas, /verify, /info, /close, /help, /quit)         |  |
|  +-------------------------------------------------------------------+  |
|                                   │                                     |
|  +--------------------------------┴----------------------------------+  |
|  |                         App State Machine                         |  |
|  |             (Endpoint, Active Session, Trust Context)             |  |
|  +--------------------------------┬----------------------------------+  |
|                                   │                                     |
|  +--------------------------------▼----------------------------------+  |
|  |                      Message Framing Layer                        |  |
|  |        [Length: 4-byte BE u32] + [Payload: JSON ChatMessage]      |  |
|  +-------------------------------------------------------------------+  |
+-------------------------------------------------------------------------+
                                    ▲
                                    │ Session / ByteStream API
                                    ▼
+-------------------------------------------------------------------------+
|                         P2PCore (Core Transport)                        |
|                                                                         |
|  - Endpoint / Session Management (1 live session per remote peer)       |
|  - iroh (QUIC / TLS 1.3 / Hybrid PQ: X25519MLKEM768 / Hole Punching)    |
|  - Relay / Discovery Integration (Custom / n0-public / Local)           |
+-------------------------------------------------------------------------+
                                    ▲
                                    │ Trust Engine Gate
                                    ▼
+-------------------------------------------------------------------------+
|                         p2p-trust (Trust Layer)                         |
|                                                                         |
|  - IdentityKey (Ed25519 Seed & Public Key)                              |
|  - KeyStore (FileKeyStore via Argon2id+ChaCha20Poly1305 / Memory)       |
|  - TrustStore (Signed FileTrustStore / Memory)                          |
|  - SAS Generator (Normalized Order-Independent 8x5-digit Sha256 SAS)    |
+-------------------------------------------------------------------------+
```

---

## 4. 核心模块与职责 (Core Modules)

### 4.1 CLI 与配置模块 (`cli.rs`) — Ticket 2 已落地
- 使用 `clap` 解析启动参数：
  - `--data-dir <PATH>`：指定持久化存储路径。
  - `--temp`：启用纯内存临时运行模式（默认或显式指定）。
  - `--password <PWD>`：身份解锁口令（优先取参数或环境变量 `P2P_PASSWORD`，未提供时终端交互提示）。
  - `--relay <URL>` / `--n0-public`：配置打洞中继服务器。

### 4.2 存储与身份初始化模块 (`store.rs`) — Ticket 2 已落地
- 封装 `KeyStore` 与 `TrustStore` 的实例化工厂：
  - **持久化模式**：使用 `p2p_trust::FileKeyStore`（Argon2id + ChaCha20Poly1305 加密）与 `p2p_trust::FileTrustStore`（签名保护）。
  - **临时模式**：使用 `p2p_trust::MemoryKeyStore` 与 `p2p_trust::MemoryTrustStore`。
- 负责绑定生成/加载 `p2p_core::Endpoint`。

### 4.3 应用层消息协议帧模块 (`frame.rs`) — Ticket 1 已落地
- **数据结构**：
  ```rust
  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
  pub struct ChatMessage {
      pub text: String,
      pub timestamp: u64,
  }
  ```
- **帧协议设计（ADR-0001）**：
  - `encode_frame`：JSON 序列化后前置 4 字节大端 `u32` 长度。
  - `decode_frame`：校验长度前缀、1MB 上限、截断与非法 JSON。
  - `write_frame` / `read_frame`：把上述帧写到 / 从 `p2p_core::Session` 读写；`read_frame` 在干净 EOF 时返回 `Ok(None)`。
  - 安全边界：`MAX_FRAME_PAYLOAD_SIZE = 1 MiB`。

### 4.4 命令与交互分发模块 (`command.rs`)
- 将终端输入的原始行解析为结构化命令枚举：
  ```rust
  pub enum UserInput {
      Message(String),
      Dial(PeerId),
      Sas,
      Verify,
      Info,
      Close,
      Help,
      Quit,
  }
  ```

### 4.5 异步会话与状态机 (`app.rs` / `main.rs`)
- 调度三个并发事件流（`tokio::select!`）：
  1. **后台呼入监听 (Inbound Accept Stream)**：无活动连接时，等待 `endpoint.accept()`。接入成功后转为主会话。
  2. **活动会话接收流 (Session Message Recv Stream)**：有活动连接时，持续异步调用 `frame::read_frame`，接收对端消息并打印渲染；若对端关闭连接，自动清理会话并重置为待连接状态。
  3. **标准输入流 (Stdin Command/Text Stream)**：异步逐行读取用户终端输入，交由 `command` 模块分发并执行相应操作。

---

## 5. 核心业务流程 (Core Workflows)

### 5.1 呼叫建立与信任升级时序 (Dial, Chat & SAS Verification)

```
[Peer A (Dialer)]                                                [Peer B (Acceptor)]
        │                                                                 │
        │── 1. dial(PeerId_B) ──[ QUIC / TLS 1.3 Handshake ]─────────────▶│ (accepts)
        │                                                                 │
        │◀── Session Established (Both trust state = TOFU) ──────────────▶│
        │                                                                 │
        │── 2. ChatMessage("Hello") [4-byte len + JSON] ─────────────────▶│ (renders text)
        │                                                                 │
  (User runs /sas)                                                  (User runs /sas)
        │                                                                 │
        ├── 3. Display SAS: "12345 67890 ..."                             ├── 3. Display SAS: "12345 67890 ..."
        │                                                                 │
        │◀══════════ 4. Out-of-band Channel (Phone / In-Person) ═════════▶│
        │                         (Verify SAS match)                      │
        │                                                                 │
  (User runs /verify)                                               (User runs /verify)
        │                                                                 │
        ├── 5. mark_verified(PeerId_B)                                    ├── 5. mark_verified(PeerId_A)
        │   (Trust state: TOFU ➔ Verified)                                │   (Trust state: TOFU ➔ Verified)
```

### 5.2 异常与断开处理
- **主动断开**：用户输入 `/close` 时调用 `session.close()`，对端接收时立即感知 `EOF (None)` 并显示对端已断开提示，两端各自释放 Session 槽位，重新进入后台监听接入状态。
- **程序退出**：用户输入 `/quit` 时，调用 `session.close()` 及 `endpoint.close()`，干净退出进程。

---

## 6. 测试与质量保证策略 (Testing & Quality Strategy)

1. **协议层单元测试 (`src/frame.rs`)** — Ticket 1 已落地：
   - 往返、空文本、截断 header/payload、声称超长、非法 JSON、编码侧超长拒绝。
2. **命令解析单元测试 (`src/command.rs`)** — Ticket 3。
3. **CLI / 持久化测试** — Ticket 2 已落地：
   - `tests/cli_args.rs`：标志解析与互斥。
   - `tests/identity.rs`：`--data-dir` 重启 Peer ID 不变；密码错；`--temp` 不写盘。
   - `tests/cli_bin.rs`：进程启动打印 64 字符 hex Peer ID；持久化两次启动相同。
4. **跨 Peer Session 集成测试**：
   - `p2p-core` 内部用 `iroh::test_utils::run_relay_server()` + `RelayConfig::with_insecure_tls()`（`#[cfg(test)]`，非公开）。
   - 本 crate 不能调用该 API。用 `n0_public()` 的 live 拨号在本环境约 20s 超时。
   - 因此 Ticket 1 的 crate 测试只覆盖帧字节形状；live Session 等 P2PCore 开放测试 Relay 钩子，或 Ticket 3 用人工双进程验证。
5. **GitHub Actions CI**：`push`/`pull_request` → `main`。`ubuntu-latest` 上 `cargo fmt --all -- --check` 与 `cargo test --locked`。工具链 1.91.0（`dtolnay/rust-toolchain@master`，预编译），依赖缓存 `Swatinem/rust-cache@v2`。`actions/checkout@v5`（Node 24）。不加 OS 矩阵、clippy、cargo-deny。
