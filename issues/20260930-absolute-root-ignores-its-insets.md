# An absolutely positioned root keeps its content size at the origin, whatever its insets

**Status:** Open
**Systems:** vendored Taffy, kernel
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1010 §1, LLP 1074 §4, vendor/taffy/EXACT-PATCHES.md patch 19, llp/reviews/code-2026-09-30-taffy-css.grok.md (finding 8), vendor/taffy/src/compute/mod.rs

Patch 19 makes an in-flow root a block-level box in its offer. An absolutely positioned root is left as it was: `known_dimensions` is `NONE` and its location is the origin, so `position: absolute; left: 10px; top: 20px; width: 30px; height: 40px` on a root lays out 30×40 at (0, 0), where a browser puts the corresponding box at (10, 20) and would stretch it between `left` and `right`. LLP 1010 says only that such a root "keeps its content size".

Fix: run the shared absolute solver (patch 18) for an absolute root against the offer as its containing block, with no static position (the origin). Say so in LLP 1010 §1 and LLP 1074 §4. Low priority: no app has an absolute root, and the kernel's rewrite exempted one for the same reason.
