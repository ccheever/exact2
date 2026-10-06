# Code review: a column flex item's main size measured at its fit-content width (Taffy patch 28; numbered 27 when reviewed, renumbered on landing after a multicol patch took 27), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x26`.
- **Method:** one brief (sha256 `eb450708aaa3929561b998b566439dff6de90e5cc694b53dcb3ce6179dcba60e`), shared with grok. Round 1, blind to the other review. Reviewed the staged diff in a worktree at 39955bf5d. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r2):** Astra 1 taken: the fit-content width is floored at the item's padding and border after the min/max clamp; Chrome case "a fit-content width under max-width: 0 keeps its padding" (fails without the floor). Astra 2 / Grok 1 taken: the branch requires an auto cross-size style (`size_style.cross(dir).is_auto()`), the condition determine_hypothetical_cross_size uses, so a sizing keyword keeps its own width; Exact's dimension codec cannot express width keywords, so no kernel test reaches it. Grok 4 taken: the two intrinsic measures run lazily, only when step E or the automatic-minimum probe needs them (`fit_content_cross_size`, cached per item); the test comment's width is corrected to 290 (29 x 10). Grok 4's "all four fail before" checked and not so: only the row holding a wrapping row fails without the patch (the others have two lines, where determine_container_main_size already takes the available width); the patch note says which. NOT TAKEN: Grok 2 (aspect-ratio items) and Grok 3 (a wrapping column in a row): both change paths beyond this bug (the final width's own ratio bailout, and a row item's main-axis probe); recorded in the patch note as not covered.

---

NOT READY

1. **[vendor/taffy/src/compute/flexbox.rs:1060](/tmp/x26/vendor/taffy/src/compute/flexbox.rs:1060) — must-fix: floor the clamped cross size by padding + border.** With `box-sizing:border-box`, `max-width` can be smaller than that sum. The new known width then produces a negative content width, whereas patch 25 floors the final width. A concrete case is a wrapping row with `max-width:0`, 10px left/right padding, and two `0×10` children: tracing this code gives a 20px height during the probe, although both children fit on one line at the final 20px border-box width. Add `.max((child.padding + child.border).cross_axis_sum(dir))` after the min/max clamp, and cover this case. This follows the [CSS content-box floor](https://www.w3.org/TR/css-sizing-3/#box-sizing).

2. **[vendor/taffy/src/compute/flexbox.rs:1040](/tmp/x26/vendor/taffy/src/compute/flexbox.rs:1040) — should-fix: distinguish `auto` from unresolved sizing keywords.** `cross(...).is_none()` also admits Taffy’s `width:max-content`. A nonwrapping row containing text with a 300px unwrapped width inside a 200px column now measures its height at 200px, while final layout honors `max-content` and uses 300px. The wrapped height survives incorrectly. Require an auto cross-size style for this branch, consistent with [§9.2’s condition](https://www.w3.org/TR/css-flexbox-1/#algo-main-item), and add a Taffy regression. Exact’s current dimension codec does not expose this keyword, so this is a vendored-library issue.

The existing compiled executable passed all 36 browser-fixture tests, including both new tests. The counterexamples above are source-traced; I did not rebuild or modify files.
exit 0
