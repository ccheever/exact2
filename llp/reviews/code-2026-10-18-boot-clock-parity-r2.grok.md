# Code review: the web and Linux boots start at their live clock's reading (parity with Apple's boot clock), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `2a27c7139`.
- **Method:** one brief (sha256 `a5169af91eba22e513f389bf56fd34a8a99caa18c9cd15820e0c377779c47c02`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two should-fix (a driver boot on the canvas reader and the Android loop took the wall reading against the drive's epoch; the Windows window still booted at zero) and a nit (the baked fallback reused the first attempt's reading).
- **Disposition:** all three taken in `530ed93a3`, checked in round 3.

Nothing here is a blocker. Two defects should be fixed; one edge is minor.

1. **SHOULD-FIX** — A driver boot on the canvas reader (and the Android loop) starts the runner clock at the wall reading while the date at clock zero stays the drive epoch. `host/linux/src/app.rs:331` and `host/linux/src/canvas.rs:1113`.

   `android --reader` writes `EXACT_AGENT=1` and `EXACT_AGENT_EPOCH` into the process before `CanvasHost::boot`. That boot sets `clock_started` from an `Instant` taken before `Config::from_env_static`, which can fetch `EXACT_DEV_PLAN`. `agent_time` then keeps the drive epoch and does not subtract the reading, but `Launch.clock` is still that elapsed time, so a fresh runner starts there (`Seed::Fresh` → `launch_clock()`). The date at boot is `epoch + elapsed`, not the epoch. The JS drive starts from `now = 0` (`scripts/agent.mjs`), so the first `clock +N` sends `to: N` and the host rejects it once `N` is below the runner (`the clock cannot go backwards`). Mount timers are due at `elapsed + delay`, so a `+10000` never meets an `after(10000)`. The same `started` is the clock later passed to `set_live_clock` (`canvas.rs:1122`, `android.rs:454`); the canvas reader only drops that wall clock on the first agent line (`canvas/agent.rs:93`), so a frame before that line samples it further forward. Linux stdio, smoke, and shot never set `clock_started`, so they still boot at 0.

   **Fix:** When `agent_time` is `Some`, pass `clock: None`. On the canvas reader and `android.rs`, do not set `clock_started` and do not install `wall_since(started)` when `EXACT_AGENT=1`.

2. **SHOULD-FIX** — The Windows window still boots fresh at 0 against a clock that has already been running. `host/windows/src/window.rs:402`.

   `started` is taken before `Config::from_env` (`window.rs:106`), which can fetch a plan. The window is created later, then `boot_presenter` runs with `clock_started` still `None`, and the presenter is given `wall_since(self.started)`. The first frame samples that elapsed time and fires every mount timer due in the gap. This is the same slow-start the new field is for; the display, Android, and canvas paths set it, and this one does not.

   **Fix:** Set `config.clock_started = Some(self.started)` immediately before `boot_presenter` in `resumed`.

3. **NIT** — A baked fallback reuses the clock sampled before the attempt that failed. `host/linux/src/app.rs:328` and `host/linux/src/app.rs:446`.

   The reading and the epoch are taken once, then the fetched plan may spend the painter open and up to a second of image metadata before it is refused. The baked runner is a fresh boot at that old reading, so timers that came due during the failed attempt are already due, and the first frame fires them. The URL fetch itself is outside this window: it finishes in `from_env` before the sample.

   **Fix:** Resample `clock` and `epoch_at_zero` together at the start of the baked `launching_with`.

`delivered`'s `set_time` (`app.rs:360`) retells the epoch captured at line 331. The runner already adopted that value from `Launch.time`, the difference is 0, and `set_time` returns with no commit and no jump. For a non-agent display, that epoch is `wall − elapsed` of the same `Instant` later passed to `wall_since`, so the date tracks that clock. Headless, smoke, and shot leave `clock_started` unset and never install that live clock, so they stay at 0. A Linux dev reload carries `now_ms` and copies the same epoch back (`restore_time`). On the web, a carried `boot_plan` keeps its clock; a first dev generation and a checkpoint that falls through to a fresh boot both go through `launch()`, which starts at the page clock. The new `boot_plan` test covers the fresh dev path.
