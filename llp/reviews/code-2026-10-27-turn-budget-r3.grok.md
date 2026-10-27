# Code review: the data-module turn budget in the calling thread's CPU time, round 3 of 3 (security pass), 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `45f3789a78`.
- **Method:** one brief (sha256 `302b80f783aee61f4a1f0e80660404d338b96b12e056f22a1c45c076bd95ec49`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**SHOULD** — The journal-ring test panics on the backstop refusal it tries to allow — `js/tests/it/turn_budget.rs:116`

`a_turn_that_held_the_ui_thread_is_said_in_the_runners_journal` waits 150 ms, then calls `Runner::boot(...).unwrap()`. The assertion also accepts a journal line containing `a turn may take`, but boot never reaches `journal()`. A wall time past the 1 s backstop makes the turn `Unavailable`. `Runner::settle` returns that error (`runner/src/runner/settlement.rs:732`), and `boot` fails at `settle()?` (`runner/src/runner.rs:1064`). The refusal is not in the ring either: `TurnClock::judge` writes the hold line only when the turn is inside its budget (`js/src/turn.rs:71`), and the runner journals `take_logs()`, not the error text. The direct answer test at `js/tests/it/turn_budget.rs:44` does let a backstop refusal through, and a budget refusal still fails that test.

No blocker.

Security pass, against the seven commits on `x2/turn-limit` (`45f3789a`, merge-base `c540dae0`). `origin/main` is 197 commits ahead of that base and does not touch these files.

An accepted turn at the default budget stays inside 100 ms of the calling thread's CPU and 1 s of wall (`budget_ms * 10` in `Took::over`). `set_budget_ms` is host-only. `begin`, `overlay`, `resume`, and `message` all call `judge`. A failed CPU-clock read falls back to the wall clock, which is the tighter cap. Snapshot turns stay on the wall time left until `snapshot_deadline`, and they skip the backstop and the hold line. A returning wait on the UI thread can now commit for up to that 1 s; a call that never returns is still ended only by the interrupt. Both match the amended LLP.

The journal ring holds 4096 lines (`JOURNAL_RING` in `runner/src/runner.rs:609`). One hold line is added per accepted main-thread turn whose wall time passed its budget. The source in that line, and in the refusal, is a contract identifier (ASCII letters, digits, `_`, `-`). The clocks are formatted numbers. The line is one ring entry, on the same `take_logs` path as the slow-load line.

LLP 1027's CPU-or-wall clock and backstop of ten budgets match `Took::over`. LLP 1027.002's 1 s backstop and the 100 ms journal line match the default budget. A zero budget with a coarse clock reading 0 CPU and any wall time still says `over the 0 ms budget` (`js/src/turn.rs:108`).
