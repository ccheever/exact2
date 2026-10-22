# Code review: the terminal taps a pressable box (QUEUE), narrow re-check 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `09c17cc9c6`.
- **Method:** one brief (sha256 `51d0657e02a6deda4a6f2dc361c893f8505e888155e9853d936181c29265e5bb`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a host fix, the lead's 20:00Z task), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (1 SHOULD-FIX, 1 NIT).
- **Disposition:** both taken: a disabled or inert box swallows every activation path; the QUEUE wording.

**RIGHT WITH CHANGES**

1. **SHOULD-FIX — Disabled/inert activation still has bypasses.** [Host::presses](host/terminal/src/host.rs:683) removes boxes from `hits`, but [on_screen](host/terminal/src/pointer.rs:259) still accepts them through an enabled pressable ancestor; agent `tap` then dispatches the disabled/inert box’s own handler. A styled inline label also bypasses the filter: `press_at` accepts the run, and `press_run` finds the excluded box’s handler without checking eligibility. Apply eligibility to these activation paths. The [new test](host/terminal/tests/it/screen.rs:914) checks standalone boxes with plain text, so it misses both cases; add nested-parent and inline-label coverage.

2. **NIT — The autofocus entry overstates click-away behavior.** [QUEUE.md:1592](QUEUE.md:1592) should qualify this as refilling **after a subsequent commit**. A bare no-hit click calls `focus(None)` and need not commit; even a refill during its blur handler is overwritten by `self.focus = to`. The next key therefore need not type into the old field. The refill in `after_commit`, including after the old box-press path, is pre-existing in HEAD’s parent.

By inspection, plain styled button labels now preserve the button’s press and `commandfor`; the new test meaningfully checks both. Controls retain their existing hit rule.

`cargo test -p exact-terminal --test it` was blocked by read-only permissions on `.cargo-build-lock`. The existing test binary predates the amendment. Tree unchanged.
