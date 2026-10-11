# Code review round 2: exactBodyFrom (efa4789c5), astra

Reviewer: GPT-6 Astra (codex, max reasoning, read-only), 2026-10-08, on the detached review worktree at efa4789c5, after round 1 was folded.

1. **P1 — Older shells remain eligible.** [compat.rs:393](/private/tmp/bf-astra/bake/src/compat.rs:393) changes the ID, but [receipt.rs:480](/private/tmp/bf-astra/bake/src/receipt.rs:480) never adds `typescriptRuntime` to artifact requirements. Delivery intentionally permits differing IDs. The classifier probe returned `bundle:true` for an old cohort, preserving the empty-POST failure. **Fix:** declare the runtime requirement and test old-cohort rejection.

2. **P2 — Native file reads still cannot meet the deadline.** [executor_core.rs:932](/private/tmp/bf-astra/host/apple/src/executor_core.rs:932) waits synchronously for `read_capped`; neither it nor `read_to_end` observes cancellation. A stalled read keeps the request and worker blocked after timeout. **Fix:** make file acquisition cancellation-aware and allow timeout settlement without waiting for the blocked read.

3. **P2 — Both web carriers can send after expiry.** [ts-fetch.js:54](/private/tmp/bf-astra/host/web-js/ts-fetch.js:54) clamps exhausted time to another millisecond; [http-body.js:200](/private/tmp/bf-astra/host/web/http-body.js:200) checks only whether the timeout event fired. With file loading blocking the event loop for 30 ms, both probes successfully sent despite a 5 ms deadline. **Fix:** check an absolute monotonic deadline before sending.

4. **P2 — Pre-aborted web reads can hang indefinitely.** [http-body.js:195](/private/tmp/bf-astra/host/web/http-body.js:195) subscribes without checking `signal.aborted`. An already-aborted signal never invokes that listener; a stalled loader remained pending even with `timeoutMs:5`. **Fix:** reject before starting file acquisition when the signal is already aborted.

5. **P3 — The stream `Request` fix remains incomplete.** [ts-stream.js:23](/private/tmp/bf-astra/host/web-js/ts-stream.js:23) inherits the method but still converts the input to `"[object Request]"` and drops its headers. The upload fails with `Invalid URL`. **Fix:** inherit `input.url` and headers, respecting explicit overrides.

6. **P3 — JavaScript’s path cap counts UTF-16 units.** [ts-fetch.js:18](/private/tmp/bf-astra/host/web-js/ts-fetch.js:18), the prelude and HTTP helper use `.length`. A 6,339-byte Unicode path passed; native rejects it. **Fix:** consistently enforce 4,096 UTF-8 bytes.

Pinned roots, Windows refusal, capacity charging, WebSocket refusal, pre-Blob size checking and grant-before-import checking close their specific findings. The guarded timer/performance issue is avoided correctly.

Verification used source tracing and in-memory probes; no builds or files changed.

**BLOCKS landing: YES — findings 1–4.**