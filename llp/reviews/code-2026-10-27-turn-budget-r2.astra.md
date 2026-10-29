# Code review: the data-module turn budget in the calling thread's CPU time, round 2 of 3, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `4aabdb2324`.
- **Method:** one brief (sha256 `cb6ba7bc5fc75d9720d99641685335ab1b07611bc840492cc50199a860c39b21`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**SHOULD — [js/tests/it/turn_budget.rs:37](js/tests/it/turn_budget.rs:37): accepted-wait test can flake under load.** `sleep(250 ms)` establishes no upper bound. Roughly 750 ms of additional descheduling crosses the 1,000 ms wall backstop, correctly returning `Unavailable` and failing the helper’s `unwrap()`. Use controlled clock readings for this acceptance assertion.

No new client-reachable runtime defect found. All four `judge` sites preserve cleanup order and journal delivery before error propagation. Round 1’s zero-budget wording, CPU-driven test loop, and 1,487-line `lib.rs` fixes check out. Release logging remains ungated—the existing open question.

Read-only source review; tests weren’t run because this tree has no built artifacts.
