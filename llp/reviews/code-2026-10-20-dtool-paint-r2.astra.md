# Code review: a canvas's after-paint pieces load within a second without frames (the QUEUE follow-up to LLP 1122 queue item (3)), round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `c83452843f`.
- **Method:** one brief (sha256 `023efd20376c433041ed5667808d5c4f5e664e7dc727ea1954e2273d3a3e1a0c`), shared by both reviewers, each blind to the other's review; commits fb6e101056 and c83452843f (round 1's fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 3 SHOULD-FIX, 1 NIT.
- **Disposition:** all taken post-round, in the commit named in the sign-off request (and a narrow verification round on it):
  - (1) A watched canvas gone before its second waits for nothing: `firstGeometry` takes its id out of `waiting`, unobserves it and settles; a newer watch of its id owns its own wait (`watched`), so an old element's second neither reports nor settles the new one; tested (canvas2d-first-geometry.test.mjs, failing on c83452843f).
  - (2) A new element's first geometry is always sent: each watch drops its id's last report key; tested.
  - (3) The content box is the computed size, fractional as the observer's, less padding and border where the box sizes its border; tested.
  - (4) The throttled-frame case counts every animation-frame callback and ResizeObserver delivery in the framed page from its first script, and asserts no observer delivery and fewer than two frames over its whole life to ready.
  - (5) The GPU modules wait is checked after the second alone, before any frame.

**One BLOCKER remains.** Reviewed both commits and the round 1 records read-only; no builds or tests were run.

1. **BLOCKER — Removing a watched canvas can still hang readiness indefinitely.**  
   [canvas2d-glue.js:245](/tmp/rv-dt-paint2/host/web/canvas2d-glue.js:245) returns when the element has disappeared or been replaced, but leaves its ID in `waiting`. The [destroy path](/tmp/rv-dt-paint2/host/web/glue.js:822) never removes Canvas 2D bookkeeping.

   Concrete scenario: the piece watches a canvas in the starved iframe, then a root timer removes that canvas before its geometry timeout. The timeout returns, ResizeObserver never delivers, and [`settled()`](/tmp/rv-dt-paint2/host/web/canvas2d-glue.js:322) keeps `activateData()` and `exact.ready` pending. The identity guard prevents a stale report but does not finish the abandoned wait. Destroy/reset needs to retire the observation and settle any remaining wait.

2. **SHOULD-FIX — Geometry deduplication survives replacement and can suppress the new canvas’s first report.**  
   [canvas2d-glue.js:221](/tmp/rv-dt-paint2/host/web/canvas2d-glue.js:221) deduplicates solely by view ID and dimensions. The Canvas 2D instance survives [boot’s replacement of the views](/tmp/rv-dt-paint2/host/web/glue.js:1348); its `reported` map is never reset.

   Concrete scenario: canvas 7 reports `200×100`, then an offscreen dev restart creates a new canvas 7 of the same size. Its fallback deletes the new wait and returns at the cached key, without calling `exact_canvas_geometry`. Readiness can complete while the new runner has no geometry and draws nothing. Deduplication must include the element/lifetime or be cleared on replacement. This was missed in round 1.

3. **SHOULD-FIX — Integer layout measurements can cause a spurious resize and erase retained drawing.**  
   [canvas2d-glue.js:247](/tmp/rv-dt-paint2/host/web/canvas2d-glue.js:247) correctly subtracts padding, but `clientWidth/Height` return integers; ResizeObserver’s content sizes retain fractions. [CSSOM definition](https://drafts.csswg.org/cssom-view/#dom-element-clientwidth), [ResizeObserver definition](https://drafts.csswg.org/resize-observer/#resizeobserversize).

   Concrete scenario: a padding-free, 100.25px-wide canvas at DPR 2 reports 100px through the fallback, producing a 200px bitmap. The observer later reports 100.25px, producing 201px. The [runner correctly starts another generation](/tmp/rv-dt-paint2/runner/src/runner/canvas2d.rs:589), and the [replayer clears its bitmap/context](/tmp/rv-dt-paint2/host/web/canvas2d-glue.js:71). Accumulated drawing made while offscreen can therefore disappear when shown, despite unchanged CSS. Fractional padding can also make a genuinely zero content dimension appear positive. Use a fractional content-box measurement.

   Two geometry reports do **not** corrupt the runner protocol: equal backing sizes are retained, changed sizes restart the generation, and stale replies are rejected. The problem is the unnecessary resize.

4. **SHOULD-FIX — The browser test proves starvation only after readiness.**  
   [activation-noframes.test.mjs:103](/tmp/rv-dt-paint2/host/web/tests/activation-noframes.test.mjs:103) waits for readiness before installing the rAF/ResizeObserver probe at line 105.

   The iframe is genuinely cross-origin and below the viewport, but the assertion cannot exclude startup observer delivery. A run receiving the necessary geometry entry before becoming fully throttled could pass with `firstGeometry` removed, then report zero callbacks during the later probe. Chromium explicitly permits an initial animation frame in an offscreen iframe. [Chromium test](https://chromium.googlesource.com/chromium/src/+/e76cb75a51d4ac988b6067b1664c2ecfe3d0814b/third_party/blink/renderer/core/scheduler_integration_tests/frame_throttling_test.cc).

   The recorded negative-control result may be valid; this assertion does not enforce it. Observe actual startup deliveries, or establish starvation before mounting the canvas.

5. **NIT — The GPU wait test can pass with a nonfunctional timer.**  
   [after-paint.test.mjs:96](/tmp/rv-dt-paint2/host/web/tests/after-paint.test.mjs:96) fires the timer, then both frames, before checking resolution. Replacing the timer callback with a no-op would still pass because the frames resolve the promise. Check completion after the timer alone before delivering frames.

The remaining round 1 dispositions hold by inspection: [gpu-modules.js:22](/tmp/rv-dt-paint2/host/web/gpu-modules.js:22) now bounds the later-artifact wait, with Promise resolution ensuring once-only continuation. The [source checks](/tmp/rv-dt-paint2/host/web/tests/after-paint.test.mjs:80) catch literal reversions of both GPU scheduling sites and destroy cleanup, though they do not execute replacement/restart integration.

Agent mode arms neither new fallback timer. `display:none` reports zero geometry and releases readiness; the runner skips empty backing stores. On a visible page, an observer arriving before the deadline makes the fallback a no-op. If the observer takes over a second, the fallback does fire; matching geometry is deduplicated, while differing geometry has the effects described in finding 3.
