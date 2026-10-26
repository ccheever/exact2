# Code review: the checker's work in token, param and dial counts (QUEUE PRIORITY), narrow re-check 3 (the names index's Debug, met on the rebase), 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `b4b905cdcf`.
- **Method:** one brief (sha256 `4e61e625044dc1c115783cc6949c4a085e288cf362e03028823bd47ede99ad0a`), blind to the other reviewer; a narrow re-check of one commit after the counts' three rounds. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

CLEAN.

- [tamber_params.rs:52](contract/cli/src/tamber_params.rs:52): Correct fix. The index derives solely from `tokens`, which remain fully represented in `Debug`. No in-tree mutation leaves the index stale.
- `AppParams` has no equality, hash, or serialization implementation. Source/design cache keys and slot/scrub `Rc::ptr_eq` comparisons are unaffected.
- Besides the [walk verification](contract/cli/src/tamber_walk.rs:448), the other `AppParams` debug use is [tamber_params_scope.rs:522](contract/cli/tests/it/tamber_params_scope.rs:522): an assertion’s failure message, not a comparison.

Read-only review; tests not run.
