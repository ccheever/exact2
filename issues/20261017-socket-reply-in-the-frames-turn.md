# A data module cannot answer a socket frame in the turn it arrives

**Status:** Open
**Systems:** runner streams, data modules, Contract
**Severity:** P2
**Author:** Claude (Opus 5.5)
**Date:** 2026-10-17
**Related:** issues/20261009-runner-owned-bidirectional-streams.md, LLP 1016.000 D7, LLP 1092 D11, issues/20261017-websocket-origin-chosen-at-run-time.md

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
