---
name: 20261005-x21-two-way-websocket
plan: 20261005-t3code-macos-parity
status: fix-built
kind: framework-gap
blocks: [20261005-auto-balance, 20261005-browser-surface, 20261005-client-activity-reporting, 20261005-embedded-server-runtime, 20261005-environment-routes, 20261005-live-automations-and-clones, 20261005-local-primary-environment, 20261005-managed-codex-chatgpt, 20261005-pr-code-tab, 20261005-pr-conversation-and-refresh, 20261005-pr-handoffs-and-quick-actions, 20261005-pr-header-actions-and-stacks, 20261005-pr-links-previews-and-routing, 20261005-pr-writing-and-metadata, 20261005-provider-settings-upkeep, 20261005-provider-sign-in-and-install, 20261005-remote-scopes-and-update-commands, 20261005-server-update-banner, 20261005-settings-scoped-controls-and-theme-editor, 20261005-sign-in-terminals, 20261005-t3-connect-sign-in, 20261005-terminal-drawer, 20261005-this-machine-network-access, 20261005-thread-commands-and-keys, 20261005-usage-pooled-view, 20261005-usage-reset-and-feedback]
upstream_url: null
reproduced_on: null
---

# X21: A two-way WebSocket for data modules (send, message, close, backpressure)

## Summary

T3 Code's client talks to its server over one WebSocket that carries Effect RPC: the client sends Request, Ack, Interrupt, Ping and Pong frames and receives Chunk, Exit and Defect frames.
exact2's WebSocket for data modules is receive-only: no frame is ever sent. The clone therefore ships its own Swift transport (about 2,200 lines across four files) that the TypeScript side
polls through a retained inbox. Support for the web `WebSocket` API in data modules would let the app move the protocol into TypeScript, as the reference has it, and remove the polling limits.

## Why this issue arose

### The T3 Code behavior
- **Connection.** The client asks the server for a short-lived ticket over HTTP, then opens `/ws?wsTicket=<ticket>` plus client metadata (`packages/client-runtime/src/authorization/remote.ts:212-224`,
  `packages/client-runtime/src/connection/resolver.ts:80`). The open timeout is 15 seconds (`packages/client-runtime/src/rpc/session.ts:45`). The client uses the web `WebSocket` through
  `Socket.layerWebSocket` and `RpcClient.makeProtocolSocket` with JSON serialization (`session.ts:191-215`). The server side is `apps/server/src/ws.ts:3792,3825-3849`.
- **Frames.** Client to server: Request (method, payload, id), Ack (stream id), Interrupt (stream id), Ping, Pong. Server to client: Chunk (stream values), Exit (result or error), Defect (and `ClientProtocolError` in the clone's reader, `T3Transport.swift:609`), Ping, Pong.
  The methods are the `WS_METHODS` list (`packages/contracts/src/rpc.ts:339` onward): shell and thread subscriptions, VCS, pull requests, settings, providers, terminal (`terminalOpen/Attach/Write/Resize/Clear/Restart/Close`,
  `rpc.ts:1334-1368`), preview events, `previewAutomation.*`.
- **Flow control.** Streams are acknowledged per chunk. The terminal output window holds back chunks once 8 are unacknowledged or 64 KiB are pending (`apps/server/src/terminal/OutputProtocol.ts:5-6,28-33`):
  a client that never sends Ack stalls the terminal after a few chunks.
- **States the user sees.** Connecting, connected, reconnecting, disconnected, and per-subscription retry; errors arrive as typed failures. Ping and Pong keep the link alive.
- **Device input.** The device viewer opens its own sockets (`/helper/ws?device=…`, `/ws?device=…&frame-meta=1`), sets `binaryType = "arraybuffer"` and sends tagged JSON for keys, touches and orientation
  (`packages/client-runtime/src/device/stream.ts:397-398,796-801,874-882`).
- Reference tests: 25 cases in `packages/client-runtime/src/rpc/client.test.ts` (14) and `session.test.ts` (11), counted by `it(` on 2026-10-05.

### What exact2 does today
- `EXACT2-GAPS.md` X21 (written from framework source at exact2 `c1522fdac`, checked against `main` `d2cb661eb`): "receive-only WebSocket … no frame is ever sent (`docs/reference.md:405-408`)."
  The summary table calls it a framework feature, "LLP 1016.000: receive-only", with the clone workaround "Swift transport (`T3Transport.swift`, `T3Fleet.swift`)".
- Library (`20261005-platforms-v3`, state-and-data.md): data resources and mutations are covered (`resource result = source(args)`, `mutation reply`, `send reply = source(args)`); an app-owned two-way socket
  is not covered: unknown in this library.
- Observed in the clone (clone code, mc-orch tree, 2026-10-05): the module header says "App-local Effect RPC wire and retained inbox. No Exact transport is extended." (`modules/apple/T3Protocol.swift:1`).

### Where the clone hits it
- `modules/apple/T3Transport.swift` (846 lines): owns the `URLSessionWebSocketTask`, builds the ticketed URL (`:405-421`), sends Ack on Chunk (`:611-624`), Interrupt on unsubscribe (`:540`), Ping every 10 s and declares the link dead
  after 35 s without Pong (`:656-675`), and caps open streams at 16 (`:529`, "Too many subscriptions are open.") and queued frames at 4,096 (`:549`).
- `modules/apple/T3Protocol.swift` (323 lines): the retained inbox, 4,096 entries or 16 MiB, after which it resets and the client must fetch authoritative snapshots (`:114-139`); frames are capped at 16 MiB (`:28`).
- `modules/apple/T3Fleet.swift` (433 lines): one socket per background environment (`:104,298-303`). `modules/apple/R7DeviceClient.swift` (587 lines): the two device sockets (`:277-290`).
- The TypeScript side polls through the app's native module: `request`, `subscribe`, `unsubscribe`, `events`, `ack`, `readChunk`, `releaseChunk` (`T3Transport.swift:95`), and wakes on `changed("t3.events")`.
- Difference a user sees: none in normal use. Differences are limits and cost: the stream cap and the inbox reset can show as "resync" under load, and each new stream (the terminal is next) needs Swift work.

## Why it must be resolved

The goal is a complete clone, and a complete clone is a long tail of RPC methods plus streams (24 plan tickets cite this issue). Today each one rides on the Swift transport; that works and is
the reason the issue is nonblocking. The costs are that the protocol lives in two languages with a polled seam between them, that backpressure for fast streams (the terminal) depends on an inbox with hard limits,
that the TypeScript client cannot reuse the reference's `rpc/session.ts`-style logic directly, and that every other exact2 app that talks to a WebSocket server has to build the same transport.
Waiting tickets are in `blocks`; the closest is `20261005-terminal-drawer` (Ack-based flow control and the 512 KiB output buffer). Either the fix lands and the app adopts it, or the user decides that the app-local
Swift transport is the permanent design and the issue closes by that decision.

## Requested support

The web way: the `WebSocket` API in data modules: `new WebSocket(url, protocols)`, `send(string | ArrayBuffer)`, `onopen`, `onmessage`, `onerror`, `onclose` (code, reason, wasClean), `close(code, reason)`,
`readyState`, `bufferedAmount`, `binaryType`. macOS host first, then the web host (which has it natively) and others.

| Piece | Request |
| --- | --- |
| Send | Text and binary frames; `send` while connecting throws as on the web; ordered delivery |
| Receive | Message events in order; binary as `ArrayBuffer`; no 4,096-entry poll window |
| Backpressure | `bufferedAmount` that drops as frames leave; a documented limit for a single frame |
| Close and error | Close codes and reasons reach the app; reconnect is the app's job (a new socket) |
| URL | `ws` and `wss`; query parameters (the reference authenticates with a query ticket; custom headers are not part of the web API; the clone also sets `x-t3-orchestration-protocol` at `T3Transport.swift:414`, and whether the server accepts the query alone is to confirm at `issue-open`) |
| Lifetime | Sockets close with the data module or the window; behavior at quit is X6 |

Alternative B: a Contract-level connection resource with a typed `send` operation. It fits the Contract style but not the web, and each app would define its own frame types. A is preferred.

## How to reproduce

To confirm on the pinned `main` at `issue-open`.
1. Minimal app: a data module that opens a WebSocket to a local echo server (for example Bun) and tries to send a text frame.
2. Expected (web): the server receives the frame and echoes it; `onmessage` fires. Actual: the receive-only form never sends.
3. Clone scenario: trace a lane session with `trace-proxy.mjs`; the frames on the wire come from `T3Transport.swift`, not from TypeScript. Open 17 subscriptions: the 17th is refused with "Too many subscriptions are open." (`:529`).

## Acceptance for the fix

- A conformance case against Chrome with an echo server: text and binary frames both ways, order, close code and reason, `bufferedAmount` going to 0, `send` before open, a 10 MiB frame.
- Agent run: `state` and `logs` show the socket state; the data module's received frames appear in `state`.
- A TypeScript RPC client can connect to the lane backend, run `server.getConfig`, open a stream, send Ack, and a trace diff (`trace-diff.mjs`) of the frames equals the Swift transport's trace for the same scenario.
- Quit behavior is documented (see X6).

## App adoption after resolution

A new refactor ticket (created at `issue-close`) ports the Effect RPC client logic to TypeScript over the new socket, retires `T3Transport.swift`/`T3Fleet.swift` socket code and the device sockets in
`R7DeviceClient.swift`, removes the 16-stream cap and the inbox poll, and keeps Keychain, SSH tunnels and file reads in Swift. The RPC tickets in `blocks` keep using the Swift transport until then; rows that must pass
after the move: the trace diff against the oracle for each ticket's scenario, the terminal flood row (`seq 1 200000`, Acks in trace) and the reconnect rows. `issue-close` checks the trace equivalence and that no
socket remains in the Swift modules except where a native reason is written down.

## Status and next action

Draft; not reproduced on the pinned `main`; not searched upstream; not published.
Next: `issue-open` (reproduce, search for duplicates, prepare the report for the user's approval; publication only after approval).

## Fix built (2026-10-06)

Built on exact2 `origin/main`, branch `daehyeon/fw-x21-websocket-send` (worktree `~/orca/workspaces/exact2/t3-fw-x21`), commits `f3663ceac`, `bf90fb4c5`, `1a3e668cd` (three review rounds). Not pushed.
- `fetch("wss://…", {exactStream})` stays the open; a mapper written `(event, socket) => …` gets a socket with the web's `send(text)`, `close(code, reason)`, `readyState`, `bufferedAmount`; a module may keep it and send from a mutation. Frames go from the module to a per-socket writer at the host; the runner sees none. A listen-only mapper behaves as before (coalescing); a talking socket coalesces until its first send, then delivers every message, bounded (4,096 messages / 8 MiB waiting). Closes are handshakes on every transport, with a 5 s deadline. On Apple the socket is now Network.framework and follows the app's App Transport Security.
- Not built: binary frames, handshake headers, subprotocols. Pings and reconnects are the app's (an `every` task's mutation, `refresh`).
- Evidence: the five checks; ibex2 socket tests (19, both transports), executor tests (Apple 41, Linux 42), runner/js/web tests; an echo drive passes on the web JS target, the wasm web host and macOS. Not run: a real Linux machine, iOS.
- Before main: LLP 1016.000 "Design line: client frames" (proposed) needs Charlie's ruling (D3/D4).
- Clone adoption also needs `net.websocket <origin>` grants for user-chosen hosts (a grant names one exact origin: a separate gap), and the `x-t3-orchestration-protocol` header moved to the query (no handshake headers).
