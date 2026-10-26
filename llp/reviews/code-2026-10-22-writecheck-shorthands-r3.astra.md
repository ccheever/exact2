# Code review: The write check judges written shorthand and flex rows as lowering expands them, round 3, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `8e653194d8`.
- **Method:** one brief (sha256 `2a606b7cd8cf0ed6ca037c2f2f67907b6c65a8df1d6d307e88861a769b8b8f2a`), blind to the other reviewer; 3 rounds (theme2's ruling on the shorthand plan), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **P3 — Track token names introduced by expansion** — [rows.rs:594](contract/lower/src/theme/rows.rs:594). Reachability scans the unexpanded value. With `auto = 8px` and `column flex="none" width="auto"`, changing `auto` to `2` reaches only `width`. Lowering expands `flex="none"` to a basis of `"auto"` and refuses **flex first**; the write check instead refuses **width**, with different words and location. Fix: include expanded longhand literals, including generated defaults, in reachability, and add a `same_words` regression.

No other findings. Tests could not run: the read-only sandbox blocked Cargo’s target-directory creation. No files edited.
