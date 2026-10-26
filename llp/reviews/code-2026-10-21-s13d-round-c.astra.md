# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), D1 round 3, D2 round 2, D3 (svg subtrees) round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `b87124b3db`.
- **Method:** one brief (sha256 `a4b5e5fcef8d07e360aeb4a9e3e3d488e861671030a2986f0d30b0847968da26`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two findings block approval.

1. **Blocker — D1: retargeting a font symlink bypasses the stale-file guard.**  
   [fonts.rs:165](contract/lower/src/fonts.rs:165) records the canonical target; [rows.rs:114](contract/lower/src/theme/rows.rs:114) subsequently checks only that target.

   Warm the server with an auto-weight font whose asset path symlinks to a static face. Retarget that symlink to a variable font supporting weight 700, leaving the old target untouched, then change a used type token from weight 400 to 700. The cached metadata still matches, so the write check rejects with `lower-font-face`; a fresh build resolves the new target and accepts. Both targets can be inside the permitted asset root.

   **Fix:** retain the authored asset path and resolved target identity. Re-resolve it during the stale check and defer font rows whenever resolution, identity, or metadata changes. Add a warm-server symlink-retarget regression asserting both write `ok` and fresh-build success.

2. **Major — D3: SVG rows are collected even when element preparation prevents their validation.**  
   [scan.rs:174](contract/lower/src/theme/scan.rs:174) applies length coercion but omits the preceding validation performed by [nodes.rs:139](contract/lower/src/nodes.rs:139).

   For example, put `rect padding=1 x="pos" width=10 height=10` inside an SVG, with `pos = 8px`. Retention still produces a map. Changing `pos` to `#123456` makes the write check report `tamber-kind` at the `x` value. The build instead stops at `padding` with `lower-svg-attr`: “`padding` on `rect`: it does not apply to an SVG element”. It never checks `x`. Thus the diagnostic ID, message, and location disagree.

   **Fix:** account for the same element-preparation checks and ancestor reachability when retaining/judging SVG rows, conservatively leaving uncertain cases to the build. Filtering only the offending attribute is insufficient: its failure prevents the element’s other rows and descendants from being lowered. Add this retained-but-lowering-invalid regression and assert the write is `ok`.

I found no additional blocking issue in D2’s scope/type path or span-preserving grouping.

Read-only source review; no files changed. `git diff --check` passed. Behavioral tests and the 48-plan comparison were not rerun.
