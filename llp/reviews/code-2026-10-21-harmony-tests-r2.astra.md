# Code review: Harmony's web tests against a Snapback server of their own (scripts/memory-backend-test.mjs, the async lane), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `1b34b366d7`.
- **Method:** one brief (sha256 `f757caa4ae5ed59d3ee64e2f634108ebb4ad151e21ba9c2ff4c710357a74bce3`), blind to the other reviewer; 2 rounds (the lead's 20:50Z), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Blocking findings remain.

1. **P1 — The tests replace the build whose grants were checked.** [scripts/memory-backend-test.mjs:105](scripts/memory-backend-test.mjs:105) invokes `test web`, which rebuilds through `app-cli.mjs:218`. A concurrent source change adding another loopback grant can therefore reach `resetBackends`, which resets that server without checking its PID or `temporary` status. The same environment does not guarantee unchanged inputs. **Fix:** invoke `scripts/agent.mjs` directly against the checked dist, or enforce the expected origin and server identity in the reset path after the final build.

2. **P2 — Shutdown can launch tests after deciding there are none to stop.** [scripts/memory-backend-test.mjs:81](scripts/memory-backend-test.mjs:81): if a signal arrives during `await status(origin)`, `end()` can pass `stop(children.tests)` while it is null. An already-in-flight successful status response can then resume `main`, build, and spawn tests while shutdown awaits the server. `process.exit()` leaves those tests running and bypasses `main`’s cleanup. **Fix:** set cancellation state immediately, check it before subsequent work, and funnel signal handling and normal completion through one awaited cleanup path.

3. **P2 — Asynchronous spawn errors bypass cleanup.** [scripts/memory-backend-test.mjs:88](scripts/memory-backend-test.mjs:88) and [line 106](scripts/memory-backend-test.mjs:106) listen for `exit` but never `error`. Spawn failures such as `EACCES` or `EAGAIN` are not caught by the surrounding `try/finally`; a test-process spawn failure can leave the backend running. A failed spawn also need not emit `exit`. **Fix:** attach error handlers immediately, reject the awaited operation, distinguish an unstarted child, and dispose of readiness timers/readers on every outcome.

4. **P2 — The initial web build still blocks cancellation.** [scripts/memory-backend-test.mjs:102](scripts/memory-backend-test.mjs:102) calls `spawnSync` after starting the backend. During a stalled build, JavaScript signal handlers cannot perform cleanup, so the advertised ten-second termination deadline never starts. **Fix:** run and track this build asynchronously, with cancellation covering its owned subprocesses.

5. **P2 — Direct spawning drops Snapback’s parent-death supervision.** [scripts/memory-backend-test.mjs:85](scripts/memory-backend-test.mjs:85) omits `SNAPBACK4_SUPERVISOR_PID`. The 0.4.33 wrapper supplies that variable; the binary installs no parent-death protection without it. An abrupt parent exit—including a second SIGINT during cleanup—can leave the server listening. **Fix:** explicitly pass `SNAPBACK4_SUPERVISOR_PID: String(process.pid)` when spawning the binary.

Argument refusal, private dist selection, `EXACT_APP_DIR`/`EXACT_WEB_DIST` propagation, PID comparison, and first-line URL parsing look correct. The lane, nested-root assertions, and release-identity reversion also look consistent.

Static review only: no files changed; no tests, servers, installs, or network commands run.
