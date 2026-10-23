# Code review: long-poll streams (LLP 1016.000 D8), round the path sweep's final check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `4151eb52a9`.
- **Method:** one brief (sha256 `6ffaaaec73f55fa7e4523b2c8a51c70538a181479a60162622027a4d415a3183`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** none. Every place a runner request is taken or admitted refuses a long poll on other work before that work runs, and each refusal settles. Apple and Linux raise the ordered barrier; the renderer, terminal, and web do not fence this class, which is what their rows say.

Checked against `take_requests`, `dispatch_work`, `release_work`, `native_work`, and `Dispatch::Again`. Production loops are `host/apple/src/abi.rs:263`, `host/linux/src/presenter.rs:771` (Windows and Android use that presenter), `host/render/src/lib.rs:852`, `host/render/src/pages.rs:63`, `host/terminal/src/requests.rs:64`, and `host/web/src/host.rs:1002`. Seams: `data/src/placed.rs:426`, `data/src/mixed.rs:349`, `data/src/envelope.rs:269`, `data/host/src/lib.rs:89`, `logic/abi/src/lib.rs:286` and `:530`, `js/src/wire.rs:96` via `js/src/door.rs:23`, `runner/src/runner/snapshot.rs:64`. Canvas2d tables and presenter platform queues are not runner requests. Reloads re-enter those loops. Admission is `host/apple/src/executor_core.rs:458` before any job is queued; `executor_async.rs` closure and handoff run only after that.

Settlement: Apple and Linux record the refusal and pump it when the ordered lane is idle (`abi.rs:349`, `executor_core.rs:549`). The terminal lands it on the outcome channel at the first `announced` after listen (`requests.rs:67`, `term/unix.rs:362`). Web wasm emits `refuse`, and the page fulfills kind 2 (`host.rs:1055`, `glue.js:712`). The renderer puts the ticket in `held.refused` and returns Complete with the placeholder kept (`lib.rs:856`, `:817`); that ticket stays in `pending()` until the runner is retired, which is the updated row.

Tests, all green: apple 3 passed (1 real-connection ignored), linux 3 passed (same ignore), render 1, terminal 1, data 1, web 1.

**Should-fix:**
- `host/web/http-body.js:373` returns the `exact-native:` branch before `longPollRefusal` at `:385`. A TypeScript `fetch('exact-native:', { exactStream, exactLongPoll })` is allowed by `host/web-js/ts-stream.js:27` and reaches `streamed` at `:154`, which answers HTTP 500 (kind 0) at `http-body.js:380`. The page-native loader rejects, so `module.later` is not called and nothing is fetched. The wasm host never emits this op (`batch.rs:27` runs first).

**Nits:**
- `host/render/tests/it/longpoll.rs:46` only asserts `settled != Busy`, so a deadline would pass. The code returns Complete and keeps the placeholder.
- `host/web/src/batch_tests.rs:479` calls `request_refusal` only. `emit_request` (`host.rs:1055`) does use it as its first line.
- `js/tests/it/castle.rs:470` checks prelude strings. The wire refusal is `js/src/wire.rs:96`, returned by `door.rs:28` before the request is pushed.
- `data/src/envelope.rs:350` round-trips a valid long poll. The refusal is `envelope.rs:269`, and that test does not drive it.
- `data/src/mixed.rs:390` (main placement) encodes the Later request and does not discard the child token. The worker branch at `:349` does the placed check. `request_from` refuses on apply, and `release` at `:727` drops a child that was never registered, so the work does not run. The row's "same check as placed" is the worker branch.
- `data/host/src/lib.rs:89` returns the error without `discard`. The token is not inserted into `pending` (`:134` is after the check); `dispatch` of an unknown token is Missing (`:349`). The closure can sit in the child until drop.
