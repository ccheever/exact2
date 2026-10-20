# Code review: Stage 11 (c): the terminal's spaces in whole cells, laid out by the kernel (Env::cell_spaces), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ef13b38d65`.
- **Method:** one brief (sha256 `b8888fafe72fab0cd3e2f86c22e6801ccfcc392823630de0009f0310ed9bb943`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (line caps over 1,500 in style.rs and instance.rs), three should-fix (multicol gap, taken in round 1's Grok fix; a reply with no grid passing; f32 precision losing a cell)
- **Disposition:** all taken in 370578281f

- **Blocking — source caps fail:** [kernel/src/style.rs:207](/tmp/rv-s11k2/kernel/src/style.rs:207) and [runner/src/instance.rs:466](/tmp/rv-s11k2/runner/src/instance.rs:466). `caps.mjs` reports **1,502 and 1,503 lines**, respectively. Move helpers into existing modules or shorten the files until both satisfy the 1,500-line check.

- **Should-fix — multicolumn gaps bypass rounding:** [kernel/src/fragment.rs:204](/tmp/rv-s11k2/kernel/src/fragment.rs:204). A shared plan using `column-count` takes its gap from raw `StyleProps`, independently of the rounded `s.gap`. Thus `column-gap:12px` remains 12 instead of 8 under `cell_spaces`. Apply the column rule in this path and add an enabled/disabled regression test.

- **Should-fix — missing grid metadata still passes:** [scripts/scale-check.mjs:258](/tmp/rv-s11k2/scripts/scale-check.mjs:258), also line 291. Both checks depend on `layout.cell` existing. Reproduced: removing the grid record and cell rectangles lets the old asymmetric `even` fixture pass; an unrounded 8px vertical gap also passes. Require valid grid metadata for `host === 'terminal'` and test its absence.

- **Should-fix — rounding changes already integral cell lengths:** [kernel/src/style/cells.rs:36](/tmp/rv-s11k2/kernel/src/style/cells.rs:36). At large finite values, subtracting `0.5` in `f32` loses precision: `space(134217744, 16)` becomes `134217728`, losing one row; negatives behave similarly. Compare the fractional part against `0.5`, or widen the intermediate arithmetic, and test both signs.

Scale-check tests: **12 passed**. Env equality, terminal-only activation, and schema/digest inputs look sound. Rust tests and release-byte comparisons were not run. No files modified or builds performed.
