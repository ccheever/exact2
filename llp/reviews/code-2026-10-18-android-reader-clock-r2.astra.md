# Code review: the Android reader's driver launch on the runner's clock from its boot, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `529890789`.
- **Method:** one brief (sha256 `3b1496de040d72c6d5e3ed86c6b5e88aafa46c916e5e98cc160c11abc247cc6d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking, no should-fix.
- **Disposition:** nothing to take.

Nothing blocking. No SHOULD-FIX findings in these two commits.

1. **INFO — Presented-frame mode stays off.** `Host::frame()` is the only Linux-host setter of `present_frames(true)`, and its sole caller is `Presenter::animation_frame()`. The [new Canvas guard](/tmp/rv-andclock-2/host/linux/src/canvas.rs:1230) excludes that call from an agent launch’s first turn. Painting through `Presenter::frame()` does not enable it.

2. **INFO — Driver advances still fire virtual frames.** With `presenting=false`, [runner advances include frame-task deadlines](/tmp/rv-andclock-2/runner/src/runner/advance.rs:210). Driver clock requests reach those advances; [hatch scheduling](/tmp/rv-andclock-2/host/linux/src/agent.rs:1173) separately visits virtual ticket ticks and `after` deadlines.

3. **INFO — No readiness dependency was removed.** Pumping, motion sampling, surface synchronization/rendering, and [painting plus `display_complete()`](/tmp/rv-andclock-2/host/linux/src/canvas.rs:1239) remain active at the held clock. Android releases `ready` after drawing the first stream; it does not require `animation_frame()`. Hatch frame tickets can wait for driver advances without blocking that path.

4. **INFO — Non-agent launches retain their behavior.** They still install [the display’s live clock](/tmp/rv-andclock-2/host/linux/src/canvas/agent.rs:68), pass the new frame guard, and retain the display-based boot time.

Read-only source review only; no edits, builds, tests, or device execution.
