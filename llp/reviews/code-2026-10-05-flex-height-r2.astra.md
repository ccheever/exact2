# Code review: a column flex item's main size measured at its fit-content width (Taffy patch 28; numbered 27 when reviewed, renumbered on landing after a multicol patch took 27), round 2, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x26`.
- **Method:** one brief (sha256 `a83149d3f54dc8976c1c9ad9551b69fc55a68bd8de5031dc62cc3666c1ef31a6`), shared with grok. Round 2, blind to the other review. Reviewed the staged diff in a worktree at 39955bf5d. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r3):** Astra 1 taken, with a correction from Chrome: the probe now uses the final width's own helper (`common::fit_content_width`) and condition (adding its compressible-replaced exclusion), so the two cannot disagree. The counterexample measured in Chrome 154 through the web host gives a 100-wide, one-line row: min-content wins, as CSS Sizing 3's max(min-content, min(max-content, available)) says, and as round 1's probe had it; the shared helper's order (max-content winning) was the outlier. The helper is corrected to CSS's order, for patch 25's grid, column-flex and absolute widths alike; the Chrome case "intrinsic sizes inverted by a negative margin" is added (it fails before on both the old helper and the round-1 probe); all kernel browser cases (incl. browser_position's 28) still pass. Grok: READY, no findings.

---

NOT READY

1. **[vendor/taffy/src/compute/flexbox.rs:953](/tmp/x26/vendor/taffy/src/compute/flexbox.rs:953) — must-fix: match the final cross-size clamp when intrinsic bounds invert.** The new expression gives min-content precedence; `common::fit_content_width`, used by `determine_hypothetical_cross_size`, gives max-content precedence.

   Source-traced counterexample: an 80px-wide column with `align-items:flex-start`, containing a wrapping row with four `flex:none`, 10px-high children of widths **80, 20, 10, 100px**; the third has `margin-left:-150px`. Taffy measures min-content **100px** and max-content **60px**. The new probe uses **100px**, fitting everything on one line and returning **10px** height. Final layout uses **60px**, producing two lines inside that 10px height. Previously, measurement at the available 80px produced two lines.

   **Fix:** use `available.max(min_content).min(max_content)` consistently with the final-width helper, and add this negative-margin regression.

The accepted round-1 fixes are otherwise resolved. I found no additional borrowing, lazy-caching, or patch-11 cache-key defect. Both height probes receive the same cached known width, and the padding/border floor follows the min/max clamp.

Validation: the existing executable passed **36 browser-fixture tests and 14 layout-equality tests**. The counterexample above was source-traced, not executed; no files were changed.
exit 0
