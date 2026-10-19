# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 4, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the net change after round 3.
- **Method:** one brief (sha256 `ce0e0080d4d3d3171380dc27dd7ecf0a66337d1dc760429d25acb73d06521094`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the writeIn stub; the dedupe; text-node targets).
- **Disposition:** fixed in round 4's commit.

---

The round-3 fixes for focus tracking and the key check are sound, and the two cases theme2 already accepted are still just those cases. Two holes are left in the mouse-edge dedupe, and the `writeIn` stub still blocks a GPU surface that appears after the input piece loads.

1. **Blocking** — `host/web-js/rt.js:1153` (writer at `767`). A look loads the input piece, and after the animation frame plus the import the callback does `x.writeIn ??= () => 0`. That stub never records the string. `gs()` installs the real writer with `??=` only while `written` is still empty, so a GPU canvas created after that (a later route, a conditional, a lazy row) keeps the stub. `gpu-glue.js` then calls `exact_surface_record(exact.writeIn(name + "\0" + json))`, the reader parses `""`, and the surface record is dropped. A canvas already in the first tree still wins, because `gs()` runs before the import callback. Mark the stub and let `gs()` replace only that function, so a writer installed earlier stays.

2. **Should-fix** — `host/web/input-glue.js:485`. `pointerEdge` is kept forever and never consumed. Firefox scroll-under-a-still-pointer fires `mouseover`/`mouseout` only. Leaving still snaps (`over` differs). Scrolling back across the same pair does not: after pointer-entering B from A, `pointerEdge` is `{over:true, target:B, related:A}`; the scroll-away `mouseout` does not match it and snaps B off; the scroll-back `mouseover` matches that stored edge and returns, so B's hover look eases in over 400ms. Clear `pointerEdge` when a mouse event matches it, and only then return.

3. **Should-fix** — `host/web/input-glue.js:490`. The hover walk starts at `e.target` and bails unless that node is an `Element`. A `mouseover`/`mouseout` target is often the text node inside the looked element (pointer events target the element, so Chrome's paired path still snaps). A mouse-only edge, the Firefox scroll case, never walks up to the element with the `:hover` look, so nothing is held still. Start at `e.target.nodeType === 3 ? e.target.parentElement : e.target`.
