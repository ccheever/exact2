# Transforms a host applies at run time make containing blocks the compiler did not lower

**Status:** Open
**Systems:** Contract compiler, web host, Apple host
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1074 D1, llp/reviews/code-2026-09-30-taffy-css.astra.md (finding 13), contract/lower/src/tags.rs `contains_absolute`, host/web/glue.js `positionContexts`, host/web/motion-glue.js:249

D1 lowers `position: relative` onto a box with a row that makes a containing block on some host. The web host also transforms boxes that have no such row: the *siblings* of a context preview (`glue.js` `positionContexts`, "top-aligned immediate side siblings translate"), and a reorder's raised row (`motion-glue.js:249`, which also makes a static raised row `relative`). A non-`none` transform makes a containing block in a browser, so an absolute descendant of such a box is placed against it on the web while the kernel places it against the ancestor. The preview itself is covered since 2e21c345a (`contextTarget` contains).

Fix: either list the recipients (a preview's siblings; a reorder row) in the compiler rule, or apply those transforms through host-owned wrappers so the authored box keeps its containing-block relationships. Low priority: neither Messages nor the reorder fixtures hold an absolute box inside such a sibling.
