# Code review: a canvas's after-paint pieces load within a second without frames (the QUEUE follow-up to LLP 1122 queue item (3)), the narrow verification round, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `dc15e11595`.
- **Method:** one brief (sha256 `9a4c39f5629b6745dd048c58212284f1945f0387dffcf2ee29c09719c3831e9d`) quoting round 2's two reviews verbatim, asking of each finding "closed? broke anything?"; commit dc15e11595. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its first finding; its narration before left out).
- **Verdict:** nothing blocks (1 SHOULD-FIX, 1 NIT).
- **Disposition:** taken in the commit named in the sign-off request:
  - (6) as Astra's 3 (no box: none).
  - (7) as Astra's 8 (the held waiter).
  - The rest closed, as the review says.

1. **BLOCKER — closed.** A watched canvas removed before its geometry second is dropped from `waiting`, unobserved, and settled at `host/web/canvas2d-glue.js:249` and `:255`. A later second for an older element returns at `:248` because `watched` (`:215`, set at `:320`) no longer points at it, so it neither reports nor resolves the new wait. `host/web/tests/canvas2d-first-geometry.test.mjs:63-90` covers destroy, replacement, and that stale second. Destroy in `host/web/glue.js:822-826` still does not call this glue; the timer is what retires the wait. Agent mode still arms no timer (`canvas2d-glue.js:320`), same as before.

2. **SHOULD-FIX — closed.** Each watch deletes that id's report key at `canvas2d-glue.js:320` before `observe`. The new element's first `report` (`:221-233`) sends even when the CSS size string matches the previous canvas. The replacement case sends twice (`canvas2d-first-geometry.test.mjs:74-79`).

3. **SHOULD-FIX — closed** for a rendered horizontal box. `firstGeometry` reads computed `width`/`height` (`canvas2d-glue.js:251-253`). Those are fractional used pixels. For `content-box` that is already the content box; for `border-box`, CSS UI defines that used value as the border box, and the code subtracts padding and border. The 100.25×50 case and the 120×60 border box (content 106×56) are what the unit test sends. Equal backing sizes still stay one generation in the runner. A vertical writing mode still reports physical width and height where the observer reports logical `inlineSize`/`blockSize` (`:237-238`).

4. **SHOULD-FIX — closed.** `host/web/tests/activation-noframes.test.mjs:99-116` counts animation-frame callbacks and `ResizeObserver` deliveries from the framed page's first script and, after ready, requires `sizes === 0`, `frames < 2`, and one piece request. Ready with zero observer deliveries means the geometry came from `firstGeometry`. The wrapper returns the native frame id.

5. **NIT — closed.** `host/web/tests/after-paint.test.mjs:96` resolves the GPU wait from the timer, flushes the promise, and expects completion before either frame is delivered.

Review B's three items are the same repairs: the content box (A3), the leaked wait (A1), and the post-ready starve probe (A4).

`watched`, `waiting`, and `reported` stay consistent. `report` still clears `waiting` before the size dedupe. The observer then settles if the set is empty (`:235-241`). A watch clears the dedupe key, records the element, and only that element may report or settle. On a rendered horizontal box the computed size matches `contentBoxSize`, so the later observer entry dedupes. The new unit test loads the real module in a vm; its style object is the computed-style stand-in the formula reads. The throttled-frame counters are whole-life counts, not a probe installed after ready.

6. **SHOULD-FIX — a canvas with no box reports its computed width, not the observer's 0.** `canvas2d-glue.js:251-253`. CSSOM's resolved value for `width`/`height` is the used pixel length only when `display` is not `none` or `contents`; otherwise it is the computed value. `clientWidth` was 0 for `display:none`, which is what both round-2 reviews relied on, and what `ResizeObserver` reports for a box that is not rendered. A canvas whose CSS width is `150px` now contributes 150 through the fallback. A computed `100%` is parsed by `parseFloat` as 100. The runner builds that backing and can draw; the observer's later 0, or the real used size once the box exists, is a different store, so the generation restarts and the bitmap is cleared (`runner/src/runner/canvas2d.rs:589-597`). Laid-out boxes are unaffected: their used values are pixel lengths and match the observer.

7. **NIT — the destroy test never holds the in-flight settle promise.** `canvas2d-first-geometry.test.mjs:67-68` calls `settled()` only after the timer has already emptied `waiting`, so it takes the fast path at `canvas2d-glue.js:331`. Removing `settle.resolve()` at `:255` would still pass. `activateData` is the caller that already holds the promise.
