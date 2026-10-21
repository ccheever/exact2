# Code review: a canvas's after-paint pieces load within a second without frames (the QUEUE follow-up to LLP 1122 queue item (3)), the narrow verification round, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `dc15e11595`.
- **Method:** one brief (sha256 `9a4c39f5629b6745dd048c58212284f1945f0387dffcf2ee29c09719c3831e9d`) quoting round 2's two reviews verbatim, asking of each finding "closed? broke anything?"; commit dc15e11595. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (the round 2 BLOCKER and SHOULD-FIXes closed or partly; 1 SHOULD-FIX new, 2 NIT new).
- **Disposition:** taken in the commit named in the sign-off request:
  - (3) A canvas with no box (`display: none` here or above, no client rects) reports none, as the observer would, not its authored size; a vertical writing mode reports inline then block, as the observer's entry does; tested.
  - (6) The observer's callback reports nothing for an element its id's latest watch is not on (a replaced or removed one, whose removal entry is 0 × 0), and a new watch unobserves the old element; tested with the old element's removal entry. (A destroyed, already-reported canvas's `watched` entry stays until its id is watched again, bounded by the ids, as `reported`'s already does.)
  - (7) The throttled-frame case counts frames by their timestamps, not callbacks.
  - (8) The unit tests hold the waiter readiness held from before and assert it resolves; the observer stand-in keeps its callback, so a stale target's entry is tested.

**Nothing blocks.** Reviewed HEAD `dc15e11595` read-only; changed nothing and ran no builds or tests. The original readiness blocker is closed, but geometry/lifetime issues remain.

1. **BLOCKER — CLOSED (A1 / B2): abandoned readiness wait.**  
   [canvas2d-glue.js:248](/tmp/rv-dt-paintn/host/web/canvas2d-glue.js:248) now distinguishes the current watch, removes a disappeared element from `waiting` and `watched`, unobserves it, and resolves the pending settlement at line 255. This closes the reported non-agent, no-observer scenario.

2. **SHOULD-FIX — CLOSED (A2): replacement suppressed by cached geometry.**  
   [canvas2d-glue.js:320](/tmp/rv-dt-paintn/host/web/canvas2d-glue.js:320) deletes `reported[id]` on each watch. A replacement’s same-size first report is sent. The new identity guard also prevents an old timer from consuming the replacement’s wait.

3. **SHOULD-FIX — PARTIALLY CLOSED (A3 / B1): measurement mismatch.**  
   [canvas2d-glue.js:251](/tmp/rv-dt-paintn/host/web/canvas2d-glue.js:251) removes integer `clientWidth` rounding and handles the cited `100.25px` example. However:
   
   - **New regression:** `display:none; width:100px; height:50px` now reports `100×50`, although it has no layout box. Computed dimensions remain authored dimensions in this case; percentages can likewise be parsed as pixel counts. This allocates/draws a hidden backing store and permits another reset when the observer supplies zero. [CSSOM resolved values](https://drafts.csswg.org/cssom/#resolved-values).
   - **B1’s axis mismatch remains:** the fallback uses physical width/height, while [the observer callback](/tmp/rv-dt-paintn/host/web/canvas2d-glue.js:238) still passes logical inline/block sizes. Vertical writing can therefore produce different geometry.

4. **SHOULD-FIX — CLOSED (A4 / B3): starvation checked only after readiness.**  
   [activation-noframes.test.mjs:99](/tmp/rv-dt-paintn/host/web/tests/activation-noframes.test.mjs:99) installs counters before navigation; line 116 requires zero observer deliveries through readiness. Startup geometry delivery can no longer silently satisfy the test. Playwright runs this script before the child document’s scripts. [Playwright documentation](https://playwright.dev/docs/api/class-page#page-add-init-script).

5. **NIT — CLOSED (A5): GPU timer assertion.**  
   [after-paint.test.mjs:96](/tmp/rv-dt-paintn/host/web/tests/after-paint.test.mjs:96) now checks resolution after firing only the timer, before delivering either frame. A no-op timer fails that assertion.

6. **SHOULD-FIX — REMAINS: observer callbacks bypass the new lifetime protection.**  
   [The watch path](/tmp/rv-dt-paintn/host/web/canvas2d-glue.js:320) never unobserves the previous element, and [the callback](/tmp/rv-dt-paintn/host/web/canvas2d-glue.js:235) checks neither `watched` nor `views` before calling `report`.

   Concrete sequence: an old canvas reports while visible; an offscreen restart replaces it; the replacement’s fallback reports and accumulates drawing. On reveal, the old element’s removal notification reports `0×0` against the replacement’s ID, then the replacement reports its real size. Those changes [restart the runner’s generation](/tmp/rv-dt-paintn/runner/src/runner/canvas2d.rs:589), losing retained drawing. Removal notifications are expected [ResizeObserver behavior](https://drafts.csswg.org/resize-observer/#intro). Reject stale targets and retire old observations. Destroying an already-reported canvas also leaves its new `watched` entry retained.

7. **NIT — NEW: the frame counter counts callbacks, not frames.**  
   [activation-noframes.test.mjs:103](/tmp/rv-dt-paintn/host/web/tests/activation-noframes.test.mjs:103) increments for every callback, but line 116 requires fewer than two. Activation and Canvas 2D loading can each have a callback in Chrome’s single initial frame, falsely failing this assertion while neither nested two-frame wait has completed. Count distinct frame timestamps or track the relevant callback chain. [HTML callback algorithm](https://html.spec.whatwg.org/multipage/imagebitmap-and-animations.html#run-the-animation-frame-callbacks).

8. **NIT — NEW: the unit harness does not verify resolution of an existing waiter.**  
   [canvas2d-first-geometry.test.mjs:35](/tmp/rv-dt-paintn/host/web/tests/canvas2d-first-geometry.test.mjs:35) creates a fresh `settled()` call for each check. After `waiting` becomes empty, that fresh call resolves even if the earlier promise was never resolved. Retain and assert the original waiter. Also, [the observer stub](/tmp/rv-dt-paintn/host/web/tests/canvas2d-first-geometry.test.mjs:23) discards its callback, so these tests cannot expose finding 6.
