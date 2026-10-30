# Code review: the data-module turn budget in the calling thread's CPU time, round 1 of 3, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `174a8c3b6b`.
- **Method:** one brief (sha256 `55700ed545cf6f6bf449d2b1ac334dac555ff29c43815f414b43b6af26872245`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**SHOULD** — A zero CPU delta at budget 0 is refused with the backstop sentence, so existing checks for `over the 0 ms budget` miss it — `js/src/turn.rs:69`

`Took::over` treats the budget as exceeded only when `own_ms > budget_ms`. A reading of 0 does not, and control falls through to the wall backstop (`budget_ms * 10`, also 0). Any positive wall time is still refused, and `overruns` still increments, but the text is `over the 0 ms a turn may take` rather than `over the 0 ms budget`. `GetThreadTimes` often reports a 0 delta for a short turn (about 15.6 ms ticks), so `js/tests/it/caltrain.rs:188`, `js/tests/it/pure.rs:296`, and `js/tests/it/castle.rs:1283` can fail on Windows even though the turn was refused. A zero delta should use the budget sentence whenever `wall_ms > budget_ms` and `budget_ms` is 0.

No blocker. The rest of the clock change holds.

The four data turns all measure with `TurnClock` and `Took::over`: `begin` (`js/src/lib.rs:943`), `overlay` (`1137`), `resume` (`1235`), and `message` (`1381`). `draw`, retire, let-go, and background delivery were unbudgeted before and still are. Snapshot turns stay on the wall time left until `snapshot_deadline`, and they skip the 1 s backstop and the UI-hold line. `f64::INFINITY` never refuses. The interrupt is still consulted first, and only after the call returns; `js/src/watch.rs` is unchanged.

The whole measured interval stays on one OS thread. `begin` and `resume` are synchronous, Hermes runs on that caller, and `NativeModule::call` is synchronous on it. Worker placement and continuations are separate turns on the owner thread, not a hop mid-call. Apple hosted `native.call` blocks this thread while the main thread runs; that wait is not CPU time and is capped by the 1 s wall backstop. Synchronous end-of-call collection in `exact_js_call` is on this thread and is counted. Hades work on its own thread is not.

`GetThreadTimes` / `GetCurrentThread` / `FILETIME` match windows-sys 0.61, and the crate features cover them. `libc` on `cfg(unix)` supplies `clock_gettime(CLOCK_THREAD_CPUTIME_ID)` for the Apple and Android targets this crate builds. A failed read falls back to the wall clock.

On-thread work is still capped at 100 ms of CPU. A waiting or off-thread host call can now commit for up to 1 s of wall time instead of being discarded after 100 ms. That is the approved bound. The refusal is still after the fact, so the watchdog is what stops a turn that does not return. Windows can accept about one extra timer tick of CPU past 100 ms. That slack is the platform clock, not an open budget.
