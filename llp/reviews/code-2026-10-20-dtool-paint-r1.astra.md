# Code review: a canvas's after-paint pieces load within a second without frames (the QUEUE follow-up to LLP 1122 queue item (3)), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `fb6e101056`.
- **Method:** one brief (sha256 `10a548da3aeec3fae6aa4d1d61d441f797b5a5a4e5fe6a638e4b25ac03071fd5`), shared by both reviewers, each blind to the other's review; commit fb6e101056. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 2 SHOULD-FIX.
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - (1) canvas2d-glue.js reports a watched canvas's first geometry from layout a second on when no ResizeObserver entry came (`firstGeometry`), so readiness settles in a frame Chrome gives no frames and no observer entries; the browser case now runs canvas-gallery in a cross-origin frame below the fold, asserts the frame gets no frames and no observer entries, and fails without the fallback ("not ready offscreen; the piece asked 1 time(s)").
  - (2) gpu-modules.js's own wait for a later artifact races the second too (frames only under the agent).
  - (3) after-paint.test.mjs checks glue.js's surface, restart and destroy sites call the helpers, so reverting either fails, and runs gpu-modules.js's wait.

One blocker remains: the real offscreen Canvas 2D readiness path still hangs. Reviewed `fb6e101056` against its parent without changing files, building, or running tests.

1. **BLOCKER — Loading Canvas 2D still leaves readiness waiting on rendering.**  
   [navigation.js:502](/tmp/rv-dt-paint1/host/web/navigation.js:502) now loads the piece without frames, but `c2dPending()` subsequently waits for `c2d.settled()`. [canvas2d-glue.js:303](/tmp/rv-dt-paint1/host/web/canvas2d-glue.js:303) puts watched canvases into `waiting`; only ResizeObserver delivery clears them and resolves settlement. Chromium throttles **both rAF and ResizeObserver** in offscreen cross-origin frames. [Chromium explanation](https://groups.google.com/a/chromium.org/g/blink-dev/c/op-z7fMMmWY).

   Concrete scenario: open Canvas Gallery in the design tool’s scrolled-away frame. The timer loads its piece, but geometry never reports, so `activateData()` still awaits `pieces.pending()` and `exact.ready` remains unresolved. The browser test misses this because [its rAF override](/tmp/rv-dt-paint1/host/web/tests/activation-noframes.test.mjs:36) leaves rendering and ResizeObserver running. Initial geometry/readiness needs a path independent of observer delivery, tested in a genuinely throttled iframe.

2. **SHOULD-FIX — Subsequent GPU artifacts retain an unbounded frame wait.**  
   Once `exact.gpu` exists, [glue.js:678](/tmp/rv-dt-paint1/host/web/glue.js:678) calls its `surface()` directly. For an app declaring GPU modules, [gpu-modules.js:20](/tmp/rv-dt-paint1/host/web/gpu-modules.js:20) still waits exclusively for two frames before importing an unseen artifact.

   Concrete scenario: load artifact A, scroll the frame away, then a dev restart introduces artifact B. B never loads; the new restart callback finds no `pendingSurfaces`, because the router owns that queue. Initial router backlog bypasses this wait, so initial-load coverage misses it. This is a remaining pre-existing gap in the stated GPU goal.

3. **SHOULD-FIX — The GPU regression test does not exercise the changed integration.**  
   [after-paint.test.mjs:12](/tmp/rv-dt-paint1/host/web/tests/after-paint.test.mjs:12) does extract and execute the real helper bodies. However, [the GPU cases](/tmp/rv-dt-paint1/host/web/tests/after-paint.test.mjs:52) manually schedule `loadGpuIfNeeded` and manually call `dropSurface`. Leaving those functions present while reverting both GPU scheduling sites—or removing `dropSurface` from the destroy op—would still pass. Exercise the real surface/destroy path, including replacement and restart, to protect review2’s condition.

The other requested checks look sound by inspection:

- The flag guards `undefined` returns in either race order. `gpuLoading` prevents competing callbacks from loading twice. A Canvas 2D load failure resets `c2dLoading`; a subsequent op schedules a fresh attempt, while the old callback stays consumed.
- `dropSurface` handles destroy-then-recreate correctly. Boot clears the queue synchronously when advancing incarnation, so current queue entries cannot span generations. Filtering after GPU destruction is harmless.
- Agent mode receives no fallback timer. The `agent.test` stand-in preserves its existing commit/ticker-ordering assertions.
- GPU sizing reads current layout, and first-frame metrics remain tied to frame callbacks. The one-second fallback is an acceptable approved liveness tradeoff on a slow visible page; it cannot guarantee paint occurred.
- The readiness regex matches both fixtures’ producer messages. The browser test’s eight-second watchdog and 1.5-second sleep establish eventual readiness and request count, not a precise one-second bound or proof that the losing callback executed.
