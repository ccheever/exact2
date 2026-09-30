# Native context panels clamp to their superview, not to the panel's containing block

**Status:** Open
**Systems:** Apple host
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1001 §1 (context previews), LLP 1074 T1, llp/reviews/code-2026-09-30-taffy-css.astra.md (finding 14), host/apple/Sources/ExactKit/Mac/PresenterMac.swift `positionContexts`, IOS/PresenterIOS.swift

`positionContexts` finds the nearest enclosing absolute panel and clamps it inside `panel.superview.bounds`. The web does the same with `panel.offsetParent` (`host/web/glue.js:326`), which is the panel's containing block. Before LLP 1074 the two were the same box (every box was a containing block); now the panel's containing block is its nearest positioned ancestor, which may be above one or more static boxes.

Case: positioned root 400×300; a static box at y = 100, height 50; under it an absolute panel, height 100, holding a preview with magnification off whose source is at y = 20. The panel's containing block is the root, so the browser can align the panel at y = 20. Native clamps to the static box's bounds and forces the panel's local top to zero, so it sits at page y = 100.

Fix: find the panel's containing block (the nearest ancestor `NodeView` whose `position_type` is not `static`, or the root) and convert its bounds into the panel's superview's coordinates before clamping. The Messages context panels are inside positioned boxes, so they do not show it.
