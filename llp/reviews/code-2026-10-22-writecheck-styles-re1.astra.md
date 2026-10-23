# Code review: The write check judges a style's rows where an element applies it (stage 13 D QUEUE), narrow re-check 1, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `034d08af8d`.
- **Method:** one brief (sha256 `23e23b5e56d0a8e308421196f3a19baa48741f7bd2d88061f44249a3653d3461`), blind to the other reviewer; 3 rounds (theme2's 02:08 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**P2 — [rows.rs:618](contract/lower/src/theme/rows.rs:618): diagnostic spans can reverse attribute order after component expansion.** With `Pair(bg="mute", fg="mute")` expanding to `color=(true ? fg : "#000000") background-color=(true ? bg : "#000000")`, removing `mute` makes the build report `color` first. This sort reports `background-color` first because its failing literal appears earlier at the call site. Both rows are judged. Preserve attribute traversal order separately from diagnostic location; the added test misses this case.

(2) is acceptable under the stated guarantee: omitted shorthand/flex failures limit completeness, while reported refusals remain the build’s.

Read-only verification: the compiler reproduction confirmed the first refusal; the existing per-element unit test passed.
