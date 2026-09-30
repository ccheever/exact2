# An absolute box's percentage insets and height resolve against a containing block whose height came from content

**Status:** Open
**Systems:** vendored Taffy, kernel
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 §4, vendor/taffy/EXACT-PATCHES.md patch 18, llp/reviews/code-2026-09-30-taffy-css.grok.md (finding 7), vendor/taffy/src/compute/common/absolute.rs

`AbsoluteBox::of` resolves `top`, `bottom`, `height`, `min-height` and `max-height` against `area_size.height`, the containing block's used padding-box height, whether or not that height was definite. CSS treats a percentage against an indefinite containing-block height as `auto` (CSS 2.1 §10.5; for insets the used size is definite once laid out, and browsers do resolve percentage *insets* of an absolute box against the used size — but a percentage `height` of an absolutely positioned box is against the containing block's padding box, which is definite for abspos). The review's claim needs checking in Chrome before anything is changed: measure `top: 50%` and `height: 50%` on an absolute box inside a `position: relative; height: auto` parent with a 100 px in-flow child, and inside a flex and a grid parent, and pin whatever Chrome does.

This is how the three old absolute passes resolved percentages too; the shared solver carries it. The fixtures (`browser_containing_block.tsv`, the percentage rows) use a root with a definite height, so they do not decide it.
