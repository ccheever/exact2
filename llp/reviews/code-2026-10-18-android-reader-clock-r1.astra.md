# Code review: the Android reader's driver launch on the runner's clock from its boot, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f9cda636e`.
- **Method:** one brief (sha256 `911e261e04927e6891e84931070f95fc8c1b7504f874e93e7afc41e004d105ac`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking for first content or ready; one should-fix: display turns still ran frame tasks at a frozen clock.
- **Disposition:** taken in 5298907892 (no presented frame under the seek clock).

No **BLOCKER** found for first content or `ready`. One **SHOULD-FIX**:

1. **SHOULD-FIX — Display frames still execute tasks at frozen clock zero.** [host/linux/src/canvas/agent.rs:73](/tmp/rv-andclock-1/host/linux/src/canvas/agent.rs:73) selects seek time, but [canvas.rs:1230](/tmp/rv-andclock-1/host/linux/src/canvas.rs:1230) still calls `animation_frame(now)` every display turn. [`Runner::frame`](/tmp/rv-andclock-1/runner/src/runner/advance.rs:135) executes frame tasks even when the timestamp repeats. A mounted `every(frame, step)` counter therefore increases before the first request while the clock remains zero; its value depends on display scheduling. Hatch frame tickets have the same unconditional callback path. The post-attachment problem predates HEAD; HEAD extends it to startup. **Fix:** under seek ownership, keep pumping and painting, but leave frame tasks in virtual-frame mode and dispatch hatch ticks only when due. Add coverage for repeated pre-request frames followed by an explicit clock advance.

The remaining checks look sound from source:

- **First content/ready:** the initial dirty paint produces a stream; [`onDraw`](/tmp/rv-andclock-1/host/android/canvas/src/main/java/dev/exact/canvas/ExactView.kt:460) releases readiness when that stream is replayed. It does not await positive-delay timers, completed animations, images, or data. Data/image completions wake independently; font loading does not depend on runner time.
- **Clock consumers:** Rust input and GPU render timestamps use the selected clock. Android flings, display scheduling, and diagnostic timing still use real time; these do not advance the runner clock. `lower=0` prevents reader-played layer animations.
- **Epoch/non-agent behavior:** agent boot uses clock zero with `agent_time`’s epoch and offset. Non-agent launches retain the existing display-clock setup.

Reviewed only `f9cda636ed`. No edits, builds, or tests run.
