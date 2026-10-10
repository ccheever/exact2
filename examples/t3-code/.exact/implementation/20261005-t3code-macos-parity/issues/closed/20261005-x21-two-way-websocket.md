---
name: 20261005-x21-two-way-websocket
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-browser-surface, 20261005-client-activity-reporting, 20261005-embedded-server-runtime, 20261005-environment-routes, 20261005-live-automations-and-clones, 20261005-local-primary-environment, 20261005-managed-codex-chatgpt, 20261005-pr-code-tab, 20261005-pr-conversation-and-refresh, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-links-previews-and-routing, 20261005-pr-writing-and-metadata, 20261005-provider-settings-upkeep, 20261005-provider-sign-in-and-install, 20261005-remote-scopes-and-update-commands, 20261005-server-update-banner, 20261005-settings-scoped-controls-and-theme-editor, 20261005-sign-in-terminals, 20261005-t3-connect-sign-in, 20261005-terminal-drawer, 20261005-this-machine-network-access, 20261005-thread-commands-and-keys, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: https://github.com/ccheever/exact2/issues/126
reproduced_on: 4c893fef6
---

# X21: A two-way WebSocket for data modules (send, message, close, backpressure)

Moved to main `issues/20261009-runner-owned-bidirectional-streams.md` (2026-10-09); tracked there.

## Summary

T3 Code's client talks to its server over one WebSocket that carries Effect RPC: the client sends Request, Ack, Interrupt, Ping and Pong frames and receives Chunk, Exit and Defect frames.
exact2's WebSocket for data modules is receive-only: no frame is ever sent. The clone therefore ships its own Swift transport (about 2,200 lines across four files) that the TypeScript side
polls through a retained inbox.

## Why it arose

### The T3 Code behavior
- **Connection.** The client asks the server for a short-lived ticket over HTTP, then opens `/ws?wsTicket=<ticket>` plus client metadata (`packages/client-runtime/src/authorization/remote.ts:212-224`,
  `packages/client-runtime/src/connection/resolver.ts:80`). The open timeout is 15 seconds (`packages/client-runtime/src/rpc/session.ts:45`). The client uses the web `WebSocket` through
  `Socket.layerWebSocket` and `RpcClient.makeProtocolSocket` with JSON serialization (`session.ts:191-215`). The server side is `apps/server/src/ws.ts:3792,3825-3849`.
- **Frames.** Client to server: Request (method, payload, id), Ack (stream id), Interrupt (stream id), Ping, Pong. Server to client: Chunk (stream values), Exit (result or error), Defect, Ping, Pong.
  The methods are the `WS_METHODS` list (`packages/contracts/src/rpc.ts:339` onward): shell and thread subscriptions, VCS, pull requests, settings, providers, terminal (`terminalOpen/Attach/Write/Resize/Clear/Restart/Close`,
  `rpc.ts:1334-1368`), preview events, `previewAutomation.*`.
- **Flow control.** Streams are acknowledged per chunk. The terminal output window holds back chunks once 8 are unacknowledged or 64 KiB are pending (`apps/server/src/terminal/OutputProtocol.ts:5-6,28-33`):
  a client that never sends Ack stalls the terminal after a few chunks.
- **Device input.** The device viewer opens its own sockets (`/helper/ws?device=…`, `/ws?device=…&frame-meta=1`), sets `binaryType = "arraybuffer"` and sends tagged JSON for keys, touches and orientation
  (`packages/client-runtime/src/device/stream.ts:397-398,796-801,874-882`).
- Reference tests: 25 cases in `packages/client-runtime/src/rpc/client.test.ts` (14) and `session.test.ts` (11), counted by `it(` on 2026-10-05.

### Where the clone hit it
Every RPC method and stream the clone ports (the tickets in `blocks`) rides on the transport; the closest is `20261005-terminal-drawer` (Ack-based flow control and the 512 KiB output buffer).
Difference a user sees: none in normal use. The differences are limits and cost: the stream cap and the inbox reset can show as "resync" under load, and each new stream needs Swift work.

## Clone workaround

The app-local Swift transport ("App-local Effect RPC wire and retained inbox. No Exact transport is extended.", `modules/apple/T3Protocol.swift:1`):
- `modules/apple/T3Transport.swift` (846 lines): owns the `URLSessionWebSocketTask`, builds the ticketed URL (`:405-421`), sends Ack on Chunk (`:611-624`), Interrupt on unsubscribe (`:540`), Ping every 10 s and declares the link dead
  after 35 s without Pong (`:656-675`), and caps open streams at 16 (`:529`, "Too many subscriptions are open.") and queued frames at 4,096 (`:549`).
- `modules/apple/T3Protocol.swift` (323 lines): the retained inbox, 4,096 entries or 16 MiB, after which it resets and the client must fetch authoritative snapshots (`:114-139`); frames are capped at 16 MiB (`:28`).
- `modules/apple/T3Fleet.swift` (433 lines): one socket per background environment (`:104,298-303`). `modules/apple/R7DeviceClient.swift` (587 lines): the two device sockets (`:277-290`).
- The TypeScript side polls through the app's native module: `request`, `subscribe`, `unsubscribe`, `events`, `ack`, `readChunk`, `releaseChunk` (`T3Transport.swift:95`), and wakes on `changed("t3.events")`.

New RPCs (the Browser surface's preview RPCs and its `previewAutomation.*` stream among them) stay behind this one transport seam. Once main has runner-owned
two-way streams, the clone ports the Effect RPC client onto the stream handle and a mutation send (Ack and ping as protocol frames) and retires the
`T3Transport.swift`/`T3Fleet.swift` socket code; that adoption also needs `net.websocket` grants for origins the user enters.

## Evidence and history

- Filed as [#126](https://github.com/ccheever/exact2/issues/126) on 2026-10-06, reproduced on exact2 `4c893fef6` before filing.
- A local fix was built on 2026-10-06 (branch `daehyeon/fw-x21-websocket-send`, commits `f3663ceac`, `bf90fb4c5`, `1a3e668cd`, not pushed): a mapper's socket with the web's `send(text)`,
  `close(code, reason)`, `readyState`, `bufferedAmount`; an echo drive passed on the web JS target, the wasm web host and macOS. Superseded on 2026-10-08, when #126 chose runner-owned
  streams with a mutation send instead of the web's `WebSocket` in modules; not pursued (the branch is kept).
- #227 (daehyeon-mun, draft) covers refusal consistency only, not sending.
