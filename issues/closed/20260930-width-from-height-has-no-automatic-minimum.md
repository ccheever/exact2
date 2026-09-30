# A width derived from a height through `aspect-ratio` gets no automatic minimum

**Status:** Closed
**Resolution:** Measure the non-replaced ratio box’s automatic inline minimum across block/flex/grid/absolute/root paths; live Chrome fixtures cover intrinsic width, explicit min-width:0, max-width and scrolling opt-outs.
**Systems:** vendored Taffy, kernel
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1053 G1, LLP 1074 §4, vendor/taffy/EXACT-PATCHES.md patch 12

Patch 12 floors a ratio-derived *height* by the content's min-content height. The inline axis is not treated the same: `height: 50px; aspect-ratio: 1` holding a word wider than 50 px gets width 50 in the kernel, where Chrome gives it the word's min-content width (the automatic minimum applies in the ratio-dependent axis). Unbuilt because the case depends on the measurer's advance; a fixture needs text whose min-content width is the same under the kernel's 0.625 em monospace and Chrome's font, or a replaced element with a natural size.
