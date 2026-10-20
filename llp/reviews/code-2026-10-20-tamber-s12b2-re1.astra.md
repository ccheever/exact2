# Code review: LLP 1121 stage 12 B2, expressions over base names per host column, narrow re-check 1 after round 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `7b4c0774fb`.
- **Method:** one brief (sha256 `82a84316eac152b77aed70c5187ec108238d835b4d843f28f53d3426fb4e7b30`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a kernel and compiler change, theme2's 00:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX).
- **Disposition:** both taken ("Re-check 1").

- **SHOULD-FIX — [tamber/src/residual.rs:188](tamber/src/residual.rs:188):** `min`/`max` discard NaNs from opposing infinities; `clamp` does likewise. Read-only probes confirm `min(space.md * 1e308 - space.sm * 1e308, 20px)` exports **20px**, while the kernel evaluates it to **0px**. Preserve the kernel’s NaN semantics.

- **SHOULD-FIX — [contract/lower/src/params.rs:367](contract/lower/src/params.rs:367):** The restored refusal duplicates the compiler’s theme diagnostic. A ranged param containing `calc(space.md * 1e38)` now reports both `tamber-param-range` and `tamber-kind` at the same span, confirmed with `build --json --errors-only`. Keep shared validation and deduplicate in the compiler.
