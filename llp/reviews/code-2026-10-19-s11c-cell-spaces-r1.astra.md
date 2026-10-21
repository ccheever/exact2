# Code review: Stage 11 (c): the terminal's spaces in whole cells, laid out by the kernel (Env::cell_spaces), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f7f2acc7b0`.
- **Method:** one brief (sha256 `39f89621f23d29d6fbde042dbaa1de19d15ad76f5d174b4fd5e9acca561df71f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (a virtualized keyed jump read row margins unrounded), two should-fix (a cell-rule toggle left styles stale; scale-check skipped missing cells), one nit (lowering tests)
- **Disposition:** all taken in ef13b38d65

Completed three read-only review rounds.

- **Blocking — [into_view.rs:131](/tmp/rv-s11k1/runner/src/instance/collection/into_view.rs:131):** Virtualized reveal still reads unrounded margins. With 16 px rows and an 8 px bottom margin, layout uses zero margin, but reveal subtracts 8: item 30 in a 160 px viewport gets offset 328 instead of 336, placing it on offscreen row 10. Use margins resolved under the kernel’s spacing rule. Add a virtualized reveal regression; the new test covers an ordinary scroller.

- **Should-fix — [environment.rs:123](/tmp/rv-s11k1/kernel/src/kernel/environment.rs:123):** Toggling `cell_spaces` after layout leaves plain px padding/margins/gaps stale: `replace_env` only restyles environment-dependent nodes, covered nodes and alerts. Invalidate affected styles when this flag changes, or explicitly enforce initialization-only use.

- **Should-fix — [scale-check.mjs:289](/tmp/rv-s11k1/scripts/scale-check.mjs:289):** Terminal gap checks silently skip painted geometry when either child lacks `cells`. A read-only probe returned `ok: true` with both missing. Require valid cell rectangles and test missing/malformed replies.

- **Nit — [cells.rs:134](/tmp/rv-s11k1/kernel/src/style/cells.rs:134):** The lowering tests omit point-valued `calc()`, `env()`/viewport resolution, mixed-percentage calculations, and negative margins through Taffy. Add enabled/disabled cases covering these explicit conditions.

The flag defaults off, participates in derived equality, and introduces no wire/schema change. The documented numbers and percentage limit are consistent.

Scale-check tests: **12 passed**. No files changed or builds run; Rust execution and release/conformance byte parity were not verified.
