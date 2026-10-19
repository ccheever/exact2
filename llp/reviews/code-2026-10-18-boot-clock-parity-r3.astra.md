# Code review: the web and Linux boots start at their live clock's reading (parity with Apple's boot clock), round 3, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `530ed93a3`.
- **Method:** one brief (sha256 `f09307dfb53af57646f16225afbf66d07920a4900959fb49415bb3f8b3db18de`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one should-fix: the Android reader's agent launch still runs its first frames on the live clock, so with the boot at zero they fire the boot's timers before `ready`.
- **Disposition:** the agent filter reverted in `f0e876a3a` (the reader's drive boots at the display's reading, so those frames fire nothing early; the stdio agent and headless never set the clock's start and boot at zero); giving the reader's drive the seek clock from its boot is QUEUEd.

1. **SHOULD-FIX — [host/linux/src/app.rs:329](/tmp/rv-ios-7/host/linux/src/app.rs:329):** Android `--reader` correctly sets `config.agent` through `EXACT_AGENT=1`, but [canvas.rs:1122](/tmp/rv-ios-7/host/linux/src/canvas.rs:1122) still installs a live clock. It switches to the agent clock only after first content is drawn. With a 15-second URL fetch, the new zero boot arms `after(10000)` at 10000, so the first live frame fires it before `ready`. **Fix:** select the seek clock from CanvasHost boot for agent launches, including `now()`, and cover the first frame in the regression test.

**No BLOCKERs found.** The fallback’s resampled clock remains consistent with its unchanged epoch, and the Windows assignment is correct. Read-only review; no builds or tests run.
