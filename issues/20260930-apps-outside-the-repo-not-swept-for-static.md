# Weird Castle and Interview are not swept for the `position: static` default

**Status:** Open
**Systems:** apps outside the repo (Weird Castle, Interview)
**Severity:** P2
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 §3.5, LLP 1001 §5, feedback: Charlie 2026-09-29 (breaking outside apps is fine; pick the ideal design)

LLP 1074 made `position: static` the default. In the repo only Messages needed changes (29 boxes). Apps that consume exact2 by path were not swept: Weird Castle (`~/projects/weird-castle`) and Interview.

What to do in each, once they move to a main at or after d8a4a0ebf, found with the same temporary compiler audit (a stack of "is this ancestor positioned" during lowering; the code is in this session's transcript and is easy to re-add):
- give `position="relative"` to the parent of every `position="absolute"` element (and of a `dialog`) that relied on the parent as its containing block;
- give `position="relative"` to every element that has `top`/`left`/`right`/`bottom`/`inset` and no `position`, since a static box's insets do nothing;
- a `z-index` on a static box that is not a flex or grid item does nothing now; add `position="relative"` where it mattered.

The compiler already lowers `relative` onto boxes that clip, scroll, transform or animate, so those need nothing.
