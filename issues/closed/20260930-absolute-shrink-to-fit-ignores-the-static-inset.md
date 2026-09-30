# An absolute box without a width is measured in the whole containing block, not from its static position

**Status:** Closed
**Resolution:** Subtract the static inline inset before the CSS intrinsic min/available/max width clamp; the recorded 400px containing block / 100px inset fixture now yields Chrome’s 300px width.
**Systems:** vendored Taffy, kernel
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 §4, vendor/taffy/EXACT-PATCHES.md patch 18, llp/reviews/code-2026-09-30-taffy-css.astra.md (finding 7), vendor/taffy/src/compute/common/absolute.rs

CSS 2.1 §10.3.7: when `left`, `width` and `right` are all `auto`, `left` is set to the static position *before* the shrink-to-fit width is computed, so the available width is the containing block's width less that static inset. The solver (`AbsoluteBox::layout`) computes `inset_space` from explicit insets only and measures the box before it resolves the static position.

Case: positioned block R, width 400; static block S, `margin-left: 100px; width: 300px`; absolute child A with `left`, `right` and `width` auto and wrappable text whose min-content width is 50 and max-content width 400. Chrome gives A width 300 (400 − 100). The solver measures it at 400 and then places the 400-wide box at x = 100. The ratio path (`aspect-ratio` with neither dimension given) uses the same available width.

Fix in the solver: resolve the static position's inline inset first (the record's rectangle and alignment give it; for `End` alignment the inset is from the far edge) and subtract it from the available width, in both directions, then measure. Add the case to `browser_containing_block.tsv` with text of known advance (the fixtures use `font: 16px/18px monospace`; the kernel's monospace measurer advances 0.625 em, Chrome's about 0.6 em, so use text whose wrap points are the same under both).
