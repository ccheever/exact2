# The web paint classifier does not know a static flex or grid item with a z-index is a stacking context

**Status:** Open
**Systems:** web host, web JS target
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 D2, llp/reviews/code-2026-09-30-taffy-css.astra.md (finding 9), host/web/src/layers.rs `STACKS`

`STACKS` (`host/web/src/layers.rs`) lists the rows that make a stacking context so that what follows one in tree order can be isolated. A non-`auto` `z-index` on a flex or grid item makes one in CSS whatever the item's `position`, and Apple's `usedZIndex` honours it, but `ZIndex` is not in `STACKS` and `paint_of` does not know the parent's display.

Case: a 100-wide flex row; red item A, width 100, `z-index: 0`; blue item B, width 100, `margin-left: -100px`. Chrome paints A above B (A is a stacking context; B is not). The classifier sees nothing layered before B, does not isolate it, and native paints B over A (both used z-values are 0; the later sibling wins). The two disagree.

Fix: treat a non-`auto` `z-index` as `stacks` when the node is a flex or grid item (the parent's `display` is known to the live host and to the template walk), and re-decide when the parent's display changes or the node is re-parented. Do not classify every static block with a `z-index`; CSS ignores that one.
