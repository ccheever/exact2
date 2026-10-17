# A data module cannot answer a socket frame in the turn it arrives

**Status:** Closed
**Resolution:** LLP 1016.000 D7 amended (Charlie, 2026-10-16); a socket's mapper replies on its own socket during open and message, sent with each frame's commit on the web JS target, Apple and Linux
**Systems:** runner streams, data modules, Contract
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** issues/20261009-runner-owned-bidirectional-streams.md, LLP 1016.000 D7, LLP 1092 D11, issues/closed/20261017-websocket-origin-chosen-at-run-time.md

## Problem

The T3 Code clone's RPC protocol answers frames as they arrive: an `Ack` for each `Chunk` (the server holds a terminal's output back at 8 unacknowledged chunks or 64 KiB) and a `Pong` for each `Ping` (its Swift transport does both inside its receive loop, `T3Transport.swift` `consume`, on daehyeon/t3-code-main). A browser client does the same in its `message` handler. A data module cannot.

## Cause

D7 refuses sends from the mapper ("Sending from a resource, the mapper, an overlay, or an async continuation refuses", `llp/1016.000-answers-that-keep-coming.rfc.md:217`); `socket.send` and `close` throw outside a mutation's synchronous source (`js/src/prelude.js:611`; `host/web-js/ts-stream.js:32`). A resource's answer cannot start a mutation either: `then` follows a mutation's answer, not a resource's (LLP 1016.001). The nearest form is a gated task (LLP 1092 D11): the mapper keeps unanswered request ids in module state and counts them in its snapshot, `task ack when feed.unacked > 0 key=feed.seq` arms `after(1, flushAcks)`, and the action's mutation sends every pending frame. So each reply waits for the snapshot's commit, a timer at least 1 ms later, the action's commit and the mutation's commit, on the runner's turns rather than the transport's, and a busy runner stalls the server. A mutation whose source awaits cannot send at all (the streams issue's "a continuation needs ownership across awaits").

## Options

1. **The mapper may stage protocol replies**, sent after the snapshot it returns commits, under D7's bounds (256 operations, 1 MiB). It keeps D7's rule that frames leave only with a standing commit.
2. **Keep D7 and document the gated-task form** in `docs/reference.md` as the way to answer frames, with its latency.

Recommendation: 1; the reply is part of receiving the frame, not an action the view takes.

## Acceptance

An echo server that requires an `Ack` per frame before sending the next gets one per frame from a data module, sent in the frame's commit, on every host D7 covers; a refused snapshot sends none.

## Ruling (Charlie, 2026-10-16)

Option 1; LLP 1016.000 D7 is amended before it is built.

- The mapper may `send` and `close` on the socket it was handed, during `open` and
  `message` only. T3's subscribe on `open` is the same need as its `Ack`. On `error` and
  `close` the handle is already CLOSED (`js/src/prelude.js:1402`) and refuses as now; a
  resource, an overlay, an async continuation and another socket's handle still refuse.
- What a mapper call stages is sent once the commit that settles that frame's snapshot
  stands, in arrival order and ahead of later mutations' frames. Snapshots may coalesce;
  staged operations never do: a commit that settles N frames sends all N frames'
  operations, and its refusal sends none of them.
- A throw, a malformed snapshot or a refused commit sends nothing, and the journal says
  so as it does for a mutation (`runner/src/runner/sockets.rs`). D7's bounds apply per
  mapper call (256 operations, 1 MiB, the transport's buffered bytes); overload fails
  that snapshot. The call makes no `fetch`.
- Not a guard flip: no path collects a mapper's staged operations today.
  `Module::message` never reads the host's socket operations (`js/src/lib.rs:1202-1257`)
  and the next `answer()` clears them; on the web JS target a stream message lands
  through `land` (`host/web-js/rt.js`), which has no effects hook. Each needs one, and
  `SocketSend` and the journal's wording, which name a mutation, need its origin.

## Fix

LLP 1016.000 D7 carries the ruling as its amendment of 2026-10-16. Natively,
the prelude keeps a socket mapper call's operations apart from its stream's
answer and hands them over (door op 14) as the call returns; `Module::message`
stages them on the store, and the runner addresses them to the frame's own
stream ticket as a `SocketSend` from `SocketOrigin::Mapper`, held in that
frame's commit checkpoint. On the web JS target they ride the frame's answer
as effects, which `reply` in `host/web-js/rt.js` queues with the commit's
outgoing work. A frame that staged operations and did not stand journals
`mapper <resource> failed: …; socket frames were refused`. Tests:
`runner/tests/it/sockets.rs`, `js/tests/it/socket.rs`,
`host/web-js/socket.test.mjs` (an echo path that sends each frame only after
the last Ack), `host/apple/src/executor_socket_tests.rs`. Driven with the
agent on the web JS target, macOS and the Linux host against such a server:
it logged `subscribe`, `Ack 1` to `Ack 5`, no Ack for the refused snapshot,
then the release's close.
