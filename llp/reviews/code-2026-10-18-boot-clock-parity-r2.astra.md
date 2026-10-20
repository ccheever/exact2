# Code review: the web and Linux boots start at their live clock's reading (parity with Apple's boot clock), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `2a27c7139`.
- **Method:** one brief (sha256 `a5169af91eba22e513f389bf56fd34a8a99caa18c9cd15820e0c377779c47c02`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; three should-fix: the Android reader's agent boots took the display clock's start; the baked fallback reused the first attempt's reading; the Windows window was not covered.
- **Disposition:** all three taken in `87ecb43b3` (agent boots at zero; each attempt reads the clock; Windows tells its clock's start), checked in round 3.

Nothing blocking. Three **SHOULD-FIX** findings:

1. **SHOULD-FIX — [host/linux/src/canvas.rs:1113](/tmp/rv-ios-6/host/linux/src/canvas.rs:1113): Android reader agent boots inherit real startup time.**  
   `clock_started` is set even when `config.agent` is true. With `android --reader --url` taking 15 seconds, the runner starts around 15,000 ms, shifting mount timers and adding that delay to the supplied agent epoch. The first agent request only disables sampling; it does not rebase the clock. **Fix:** select the zero-based seek clock before boot for agent launches and preserve that ownership through the first frame.

2. **SHOULD-FIX — [host/linux/src/app.rs:330](/tmp/rv-ios-6/host/linux/src/app.rs:330): Baked fallback reuses a stale clock reading.**  
   The clock is sampled once and reused for the fallback at line 446. A selected candidate can spend a second waiting for image metadata before being rejected; the baked fallback’s `after(500)` then starts already overdue. **Fix:** resample `Launch.clock` for each fresh boot attempt while retaining the original `epoch_at_zero` and the identical value passed to `delivered`.

3. **SHOULD-FIX — [host/windows/src/window.rs:402](/tmp/rv-ios-6/host/windows/src/window.rs:402): Windows remains an uncovered caller.**  
   It captures `started` before config loading and later installs `wall_since(self.started)`, but leaves `clock_started=None`. A 15-second URL load therefore still boots `after(10000)` due at 10,000 while the live clock exceeds 15,000; its wall-date origin is also misaligned. **Fix:** set `self.config.clock_started = Some(self.started)` before the window’s boot. This is a remaining gap, not a newly introduced regression.

Otherwise, the epoch calculation matches `wall_since(started)`, and `delivered` repeats the exact captured fact without a time commit. Linux agent/headless/smoke/shot paths retain zero; ordinary dev reloads retain carried time; web checkpoint fallbacks inherit the launch clock.

Static review only; no builds or worktree/index changes.
