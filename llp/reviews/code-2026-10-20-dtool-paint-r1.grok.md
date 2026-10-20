# Code review: a canvas's after-paint pieces load within a second without frames (the QUEUE follow-up to LLP 1122 queue item (3)), round 1, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fb6e101056`.
- **Method:** one brief (sha256 `10a548da3aeec3fae6aa4d1d61d441f797b5a5a4e5fe6a638e4b25ac03071fd5`), shared by both reviewers, each blind to the other's review; commit fb6e101056. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 1 NIT.
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - (1) as Astra's 1.
  - (2) as Astra's 3.

The once-guard, the GPU queue, and `dropSurface` hold. One path still never becomes ready in the frame this change is for.

1. **BLOCKER** — A frameless Canvas 2D page still never becomes ready. `host/web/navigation.js:502` resolves the piece after the timer, then `pending()` waits on `c2d.settled()`. The replayed watch op only calls `observer.observe` (`host/web/canvas2d-glue.js:303`); the first box is reported from that `ResizeObserver` (`canvas2d-glue.js:234`), and `settled()` stays pending while `waiting` is non-empty (`canvas2d-glue.js:313`). `activateData` sets `moduleReady` and opens input only after that promise (`host/web/glue.js:1274`).

   Chrome's offscreen cross-origin throttle (the design-tool canvas scrolled away) skips rendering updates entirely, so both `requestAnimationFrame` and `ResizeObserver` stop. Concrete sequence: the 1s timer fires, `canvas2d-glue.js` loads, the boot batch's watch is replayed, `observe` runs, no resize notification is delivered, `pieces.pending()` never resolves, `dataset.moduleReady` stays unset. Scrolling the iframe back into view is what finally notifies. That is the same stuck `exact.ready` this change is meant to clear; the wait moved from the two frames before the fetch to the observer after it.

   The new browser case does not model that. `activation-noframes.test.mjs:36` replaces `requestAnimationFrame` with a function that never runs, and the page is still rendered, so `ResizeObserver` still fires and the 8s `moduleReady` poll passes. It would pass with the piece's ready-gate unchanged.

2. **NIT** — `host/web/tests/after-paint.test.mjs:12` does extract the real `afterPaint`, `loadGpuIfNeeded`, and `dropSurface` and run them. The once-guard test would fail if the guard were the function's return value (`undefined` is what `loadGpuIfNeeded` and the canvas resolver return). It would not fail if the `surface` op stopped calling `afterPaint`, or if `destroy` stopped calling `dropSurface`: the test invokes `dropSurface` itself (`after-paint.test.mjs:67`) and never runs `apply`. The canvas browser case is what locks the Canvas 2D call site.

Nothing else in the asked matrix fails.

Frames-then-timer, and timer-then-frames, run the work once. A second `afterPaint(loadGpuIfNeeded)` hits `gpuLoading` and does not fetch again. A restart replaces `pendingSurfaces` at `glue.js:1337` before the new batch; an older callback then sees the new array, and `gpuLoading` still collapses the fetches to one. An in-flight `gpu-glue.js` assigns `exact.gpu` before its drain, and a surface that arrives in that window is created from the module's own `surfaces` map when the load finishes. The canvas error path nulls `c2dLoading` and the next op schedules a new `afterPaint`; the old once-flag does not cover it. Agent mode passes `o.agent()` / `agentMode` and arms no timer, same as the old frames-only wait. `agent.test.mjs`'s stand-in still pushes the one `raf` those boot tests assert (`fixture` `requestAnimationFrame` never invokes its callback, which was already true of the inlined double `requestAnimationFrame`).

`dropSurface` matches a same-batch replacement. Receipts emit `destroy` before any `surface` op (`host/web/src/host.rs:933`, then `:998`). Destroy removes the queued id; the later `surface` op does not find it and pushes the new one. It cannot drop another generation's entry: boot installs a new array before that batch, and only that array is filtered. After the module has run, `pendingSurfaces` is already `[]` (`gpu-glue.js:1170`, `gpu-modules.js:127`) and `exact.gpu.destroy` has already dropped the live surface, so the filter is a no-op.

A visible page that is merely slow still takes the two frames; the timer wins only when a full second passes with no rendering opportunity. GPU size is `getBoundingClientRect` at create time (minimum 1px) and `firstFrameMs` is its own two frames after submit, not this timer. The 1s fallback is acceptable there. The case that is not is the one in finding 1, where that same missing rendering update is what `settled()` waits on.

The dev-loop regex is not a flake. `canvas-gallery` is a TypeScript app, and `module generation ready` is logged only after `captureGeneration` (`host/web/dev.mjs:785`); `plan ready:` still matches `theme-fixture`, and `edit → plan ready` has no colon so it does not. The loop still waits for the listen banner too. The 8s poll starts after `goto`'s load, and the extra 1.5s covers the 1s timer.
