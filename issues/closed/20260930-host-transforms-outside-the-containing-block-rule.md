# Transforms a host applies at run time make containing blocks the compiler did not lower

**Status:** Closed
**Resolution:** Position context-preview structural recipients and source scroll content during compiler lowering, including repeated flow roots; position generated reorder wrappers before gestures. Regression fixtures reproduce the old failure and verify descendant frames before/during/after transforms; live Chrome confirms stable containing blocks.
**Systems:** Contract compiler, web host, Apple host
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 D1, llp/reviews/code-2026-09-30-taffy-css.astra.md (finding 13), contract/lower/src/tags.rs `contains_absolute`, host/web/glue.js `positionContexts`, host/web/motion-glue.js:249

D1 lowers `position: relative` onto a box with a row that makes a containing block on some host. The web host also transforms boxes that have no such row: the *siblings* of a context preview (`glue.js` `positionContexts`, "top-aligned immediate side siblings translate"), and a reorder's raised row (`motion-glue.js:249`, which also makes a static raised row `relative`). A non-`none` transform makes a containing block in a browser, so an absolute descendant of such a box is placed against it on the web while the kernel places it against the ancestor. The preview itself is covered since 2e21c345a (`contextTarget` contains).

Fix: either list the recipients (a preview's siblings; a reorder row) in the compiler rule, or apply those transforms through host-owned wrappers so the authored box keeps its containing-block relationships. Low priority: neither Messages nor the reorder fixtures hold an absolute box inside such a sibling.


Verification (2026-09-30): the new Contract regressions fail against the previous
implementation and pass after lowering structural recipients. They exercise
absolute descendants of preview side and trailing siblings, a source scroller,
transparent conditional regions, ordinary and virtual repeated rows, explicit
static refusal, and unchanged static unrelated descendants. Reorder coverage
checks generated wrapper containment before, during and after preview, while
non-reorder wrappers remain static. A live headless Chrome probe executed the
existing `positionContexts` function: before the fix the side/trailing children's
parent-relative right/bottom offsets changed from (50,160)/(130,140) to (0,0)
on opening; with the lowered positions all four offsets stay zero.

Review follow-up: inspect the authored root before ending either ancestor walk;
the host carrier is above it. Two additional regressions reproduce absolute
root-panel and scrolling-root source containment failures and verify their
corrected descendant frames. Chrome running `positionContexts` confirms offsets
remain zero before/after both root cases, instead of jumping from (50,160)
and (130,160) respectively.
