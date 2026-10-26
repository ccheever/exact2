# Code review: The write check judges written shorthand and flex rows as lowering expands them, narrow re-check 1, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `72a796ea11`.
- **Method:** one brief (sha256 `f99d3c008b170c98d1b21f63294eb47d553c45dc51c45bd861888e1510ec0188`), blind to the other reviewer; 3 rounds (theme2's ruling on the shorthand plan), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **P3 — Expansion reachability runs without the design bound** — [rows.rs:623](contract/lower/src/theme/rows.rs:623). For `border="1px SOLID ink"` with color tokens `ink` and `solid`, `component_of()` cannot resolve `ink` during this scan, so `.ok()` discards every component. Changing `solid` to `2` is missed: the authored scan sees `SOLID`, while lowering checks the normalized `solid` and rejects it. Expand under the bound design, or conservatively include rows whose expansion fails.

The `none` handling and prose fixes look correct. No new padding/margin false refusal found; those remain on the computed path.

Read-only source review; tests not run.
