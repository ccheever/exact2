# Code review: long-poll streams (LLP 1016.000 D8), round the sweep's re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `6b968c7f57`.
- **Method:** one brief (sha256 `0f19e8beafd90eb70f965ccc3bfce7e73e6e8429af275e3b8ebdb948094ce382`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking:** none.

**Should-fix:** none.

**Nits:**
- `host/render/tests/it/longpoll.rs:46` — the comment says the placeholder is kept. The test asserts `Settled::Complete` (`:47`) and that the work did not run (`:45`). It never reads the placeholder.
- `data/src/envelope.rs:377` — the new test refuses a continuation long poll. The encode guard (`:51`) refuses every `long_poll_refusal()`, including a surface, and `request_json` (`:165`) still carries no surface. The test never builds a surface request.

Every production place that takes a runner request checks `long_poll_refusal()` before the work is dispatched or admitted: terminal `requests.rs:67`, web `host.rs:1055` via `batch.rs:36`, page `http-body.js:363` (before the native branch at `:365`), renderer `lib.rs:856`, page enumeration `pages.rs:65` and executor admit `executor_core.rs:458`, Apple `abi.rs:268`, Linux `presenter.rs:777`. Seams match: envelope `:51` (main placement included), worker `mixed.rs:349`, `placed.rs:426`, `data/host` `lib.rs:89`, `js/src/wire.rs:96`, ABI encode `logic/abi/src/lib.rs:287`, JS Rust seam `rust-data.js:105` (throw has no `refuse` flag, so `callWith` at `:140` does not apply the turn's writes), TypeScript `ts-stream.js:25`. A long poll that fails the predicate never parks, so a later `release_work` does not admit it. A valid HTTP long poll still encodes and round-trips (`a_worker_s_long_poll_keeps_its_terms`).

The terminal change settles `Host` and `Missing` on the same outcome channel the long-poll path already used (`requests.rs:103`). `Run`, `Again`, and `Held` are unchanged (`:91`). `announced` calls `run_requests` when a landing committed nothing (`host.rs:332`), which is how a background round's next request leaves. A failed background round stalls (`js/src/background.rs:120`) and `background_landed` does not queue another (`runner/src/runner/background.rs:98`), so that wake does not re-ask a host that just refused it.

Tests, all green: `bun test host/web/http-body.test.mjs` (18; POST body, websocket URL, `stream: false`, and a deep `from` refuse with nothing sent — no `exact-native:` case). `cargo test -p exact-data --lib` (81, including the new envelope refusal). `cargo test -p exact-terminal` (45 unit + 68 integration, including the long-poll refusal and `a_continuation_runs_and_its_answer_lands`). `cargo test -p exact-render --test it longpoll` (the Complete assertion). The terminal suite has no new case for a fetch or surface refusal, or for the no-commit `run_requests` branch.
