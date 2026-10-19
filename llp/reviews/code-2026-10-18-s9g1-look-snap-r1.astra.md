# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 4: high), sandbox read-only, a throwaway worktree.
- **Scope:** the look-snap commit.
- **Method:** one brief (sha256 `43ca1cf8dca8640d64960b23d4d794a1be7ed9e78973f3750eab8b6d79de19bf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: other transitions ended at an edge, accepted by theme2 as a deviation; focus containment, keyboard modality, layout hover, tests).
- **Disposition:** fixed in round 1's commit.

---

I found one blocking regression.

1. **Blocking — [host/web/input-glue.js:436](/tmp/rv-g1-1/host/web/input-glue.js:436): entering a look cancels unrelated base transitions.**  
   Start an authored opacity or scale transition on an element whose hover look changes only its background, then move the pointer onto it. `snap()` sets `transition: none` and flushes style, cancelling **all** its CSS transitions and jumping them to their targets. Restoring the declaration does not resume them. This also happens for a focus-only look on pointer entry, or a hover look suppressed by `(hover:none)`. [CSS transition cancellation rules](https://drafts.csswg.org/css-transitions/#starting) require this behavior.  
   **Fix:** suppress only properties affected by an actual look change, preserving unrelated running transitions. Test a base transition *across* an input edge.

2. **Should-fix — [host/web/input-glue.js:435](/tmp/rv-g1-1/host/web/input-glue.js:435): subtree containment is wrong for `:focus-visible`.**  
   Give a focusable parent a focus look and transition, then Tab from it into a focusable child. The parent’s `focusout` stops immediately because it contains `relatedTarget`; the child’s `focusin` also stops before processing the parent. Its departing look therefore eases. Moving focus back misses the parent’s entering look too. I confirmed these skipped calls using the extracted walker.  
   **Fix:** handle focus changes on the actual focused/blurred element; reserve the containment-based ancestor walk for hover.

3. **Should-fix — [host/web/input-glue.js:439](/tmp/rv-g1-1/host/web/input-glue.js:439): keyboard modality can activate the focus look without either focus event.**  
   Click a focusable box, then press Enter while focus stays there. `:focus-visible` can become true without `focusin` or `focusout`, so its authored transition still runs. This exact modality change has a [Web Platform Test](https://github.com/web-platform-tests/wpt/blob/master/css/selectors/focus-visible-007.html).  
   **Fix:** reconcile the focused element’s actual `:focus-visible` state on relevant keyboard/modality changes, snapping only when that state changes.

4. **Should-fix — [host/web/input-glue.js:428](/tmp/rv-g1-1/host/web/input-glue.js:428): the hover timing assumption fails for layout-driven updates.**  
   Move an existing looked element beneath a stationary pointer. WebKit’s `updateMouseEventTargetAfterLayoutIfNeeded()` dispatches boundary events **before** calling `updateHoverActiveState()`: this listener flushes the old look, restores transitions, and then the new look can animate. WebKit also has a hover timer that updates state without dispatching boundary events. [WebKit implementation](https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebCore/page/EventHandler.cpp)  
   **Fix:** reconcile after the browser updates hover, including changes caused by layout, scrolling and DOM updates; alternatively separate look presentation from the properties carrying base transitions. Merely adding more pointer listeners is insufficient.

5. **Should-fix — [host/web/tests/looks.test.mjs:86](/tmp/rv-g1-1/host/web/tests/looks.test.mjs:86), [line 101](/tmp/rv-g1-1/host/web/tests/looks.test.mjs:101): timing and fixtures leave important failures undetected.**  
   A delayed frame can observe a broken 400 ms transition after it finishes, falsely passing the snap assertion. Conversely, the 100 ms timer can resume after 400 ms and fail a correct base transition. The “nothing is touched” assertion only checks an unchanged color, so redundant snapping passes. Also, the fixture’s map contains only view `11`; the existing press-test views never enter the new snap path.  
   **Fix:** control animation time or inspect transition creation/cancellation directly; populate the map faithfully; exercise focus, nested looks, an ongoing base transition, and a press animation across an edge.

Other checks: numeric map keys are correct—the batch emits numeric `u32` IDs. Restoration is synchronous, so an ordinary host style rewrite cannot interleave inside `snap()`. Nested hover edges reach both looked elements. The held path already snaps and can now flush twice. Setting `transition` does not directly cancel `showPress`’s WAAPI animations, although the style mutations trigger the press-paint observer.

Ordinary mouse-hover timing is supported by current [Blink](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/input/event_handler.cc), [Gecko](https://raw.githubusercontent.com/mozilla-firefox/firefox/main/dom/events/EventStateManager.cpp), and [WebKit](https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebCore/page/EventHandler.cpp) source; ordinary focus transfers update focus state before bubbling focus events. That does not establish touch/pen or iframe coverage. Top-layer sheets remain DOM descendants and are covered; actual outside-root portals and shadow-internal looked nodes are not traversed, but I found no supported producer establishing another regression here. Each visited looked edge incurs a forced style read; batching suppression and caching the sheet lookup per event would reduce that cost.

Browser conclusions are source-based. I ran the in-memory walker probe, not the browser suite, whose harness creates profile directories. No files were edited.
