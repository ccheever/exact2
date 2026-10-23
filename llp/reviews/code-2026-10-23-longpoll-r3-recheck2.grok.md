# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `0da59c229a`.
- **Method:** one brief (sha256 `10cf7a9522794e5646b8d446b8ac71d5ffc323e4daccbc6953bd3e3f559c1d4f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

## Blocking

- `host/apple/src/executor_core.rs:456` (Linux and the renderer compile this same core). `admit` refuses a deadline with `timeout_refusal` before any job, and an ordered failure sets `ordered_barrier` at `:548`. `long_poll_refusal` is not part of that check. Apple (`host/apple/src/abi.rs:266`), Linux (`host/linux/src/presenter.rs:773`), and `hand_out` (`host/render/src/lib.rs:854`) only pass `Dispatch::Missing`. A non-stream continuation, storage request, or ordered HTTP long poll is admitted as a normal ordered job. The next ordered request in that `take_requests` batch is admitted behind it and can start (a POST leaves, a continuation runs) before the refusal is fulfilled. `complete` (`executor_core.rs:1031`) does not raise the barrier. A continuation or storage job never reaches the long-poll check at `:1332`: `:1306` and `:1312` return first. With no work attached, the continuation body is not called, and the outcome is `Unsupported` ("missing or consumed native continuation" / "storage requires an app storage adapter"). The async starter does the same: `Kind::Closure` (`executor_async.rs:945`) runs `closure` (`:1027`) and never looks at `long_poll_refusal` (`:648` is only the HTTP arm).

- `data/host/src/lib.rs:89`. Native storage is rewritten here before the runner ever queues it. A deadline is refused at `:89`. A long poll is not. At `:136` the request is replaced with `Request::continuation`, which drops `long_poll`, and the storage operation is what the host later runs. Web keeps the storage request (that rewrite is not compiled for wasm) and `request_refusal` still refuses it.

- `host/terminal/src/requests.rs:64` and `host/render/src/pages.rs:63`. Both call `dispatch_work` and run a continuation with no `long_poll_refusal` check. `take_requests` arms a background round, so a background long poll on the terminal host runs its continuation. Page enumeration does the same for a continuation answer.

## Should-fix

- `host/apple/src/executor_core.rs:1109`. A stream on native, storage, or continuation work — the shape `Answer::stream` and the runner fixture actually build (`runner/src/runner/stream_tests.rs:30`) — dies in `reservation` as "a stream is independent HTTP" before `long_poll_refusal`. On Apple and Linux that is an admission refusal: the pump is woken and, when the request is ordered, the barrier does go up. The source never sees the long-poll reason. On the renderer an `admit` error is `held.busy` (`host/render/src/lib.rs:909`), so the page settles `Busy` (503) with the ticket still pending.

## Nits

- `host/apple/src/executor_longpoll_tests.rs:599` only hands the executor a non-stream `Request::native(...).long_poll(...)`, the one shape `execute` refuses with `exactLongPoll`. It does not cover a stream, a continuation, storage, or the ordered fence. `runner/src/runner/stream_tests.rs:329` only asserts the predicate on `take_requests`.
- `host/render/src/lib.rs:798`. A `Refused` completion is recorded in `held.refused` and not fulfilled, so the ticket stays pending and the source never shapes the failure. `settle` still returns. That matches the old renderer path for an admission refusal.
