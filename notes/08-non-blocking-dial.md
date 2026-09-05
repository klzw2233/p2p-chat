# Ticket 8: Non-blocking `/dial`

**Issue:** [#20](https://github.com/klzw2233/p2p-chat/issues/20)  
**Status:** Closed

## Problem

`/dial` blocked the entire REPL event loop during the handshake (up to ~20s with `--n0-public`):

1. **Inbound `accept()` starved.** If peer B dialed A while A was still dialing C, A wouldn't accept B's connection until A's outbound dial completed or timed out. In a mutual-dial scenario (A↔B both dialing each other), the connection that should have succeeded in <1s could wait 20s.
2. **Cannot interrupt.** Ctrl+C and `/quit` were ignored until the dial future returned, because `select!` wasn't polling the `input.next()` branch. A mistyped Peer ID or network stall forced the user to wait for the full timeout.

## Solution

Store the dial as a pinned `Future` in an `Option<Pin<Box<dyn Future<...> + 'a>>>` and poll it as a 4th `select!` branch. The future borrows `&Endpoint` (via the `'a` lifetime tied to the loop), so it doesn't require `Endpoint: Clone` or `tokio::spawn` with `'static`.

### Changes

- `src/app.rs:24-26`: `dialing` type is now `Option<Pin<Box<dyn Future<Output = Result<Session, Error>> + Send + 'a>>>`.
- `src/app.rs:71-77`: 4th `select!` branch awaits `dialing.as_mut().unwrap().as_mut().await`.
- `src/app.rs:220-244`: `dial()` constructs `Box::pin(endpoint.dial(peer, hints))` inline, no spawn.
- `handle_line` and `dispatch` are `async` to preserve await points for message send.

### Manual verification

1. Terminal A: `cargo run -- --temp --n0-public`
2. Terminal B: `cargo run -- --temp --n0-public`, copy A's Peer ID.
3. In B: `/dial <A_peer_id>` — "dialing …" appears immediately.
4. **Before dial completes**: in A, `/dial <B_peer_id>`.
5. Expected: A's outbound dial to B and A's inbound accept from B both succeed within ~1s (the faster of the two directions wins), not 20s.
6. Alternatively: in B while dial is in flight, type `/quit` or Ctrl+C — should exit immediately, not wait for timeout.

## Trade-offs

- The `'a` lifetime on `DialFuture` ties it to the `run_repl` loop iteration, which is correct (the borrow of `endpoint` is valid for the loop body) but requires propagating `'a` through `handle_line`, `dispatch`, and `dial`.
- `tokio::spawn` would have been simpler but requires `Endpoint: Clone`, which P2PCore doesn't expose (and shouldn't, since `Endpoint` wraps shared state and cloning the wrapper adds no value).

## Why this matters

The original HANDOFF.md gap-4 comment framed blocking as "no input needed during dial anyway," but the real issue wasn't about typing messages — it was about **control flow**:

- Mutual-dial (both peers dial each other simultaneously) is common when bootstrapping or reconnecting. One direction will typically succeed first; the REPL needs to accept whichever arrives.
- Being unable to cancel a stuck dial (wrong Peer ID, network partition, relay down) makes the tool feel broken.

The fix removes both blockers with minimal diff: one new `select!` branch, no new dependencies, no spawn overhead.
