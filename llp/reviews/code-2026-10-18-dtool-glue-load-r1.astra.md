# Code review: the JS page's input glue loaded where frames are held (LLP 1122 D8), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `592d970714`.
- **Method:** one brief (sha256 `6739ab8fb54510ae24bbad742b647b407f6cf4f948aef164eb50d580dc185fb5`), shared by both reviewers, blind to the other review; commit 592d970714. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (smaller points).
- **Disposition:** nothing blocks (both). Taken in `e6598647a0`: the runtime hands the dev page its `pressFeedback` (`globalThis.exact.pressFeedback`), so a hold on a bundled JS page that never started its input piece starts it (the dev page's `rt.js` import 404s on a bundled page). Not taken: a runtime-loader test with frames withheld (rt.js needs a page; the T4 gate's after-rebuild line runs it end to end: the hold held 2 s after the reload with the canvas out of view, the new look shown once in view); the wasm client shell's own two-frame activation, the same class of stall on a path the design tool's canvas does not use (QUEUE); timers clamped in a long-hidden tab (unchanged by this).

Nothing blocks this commit’s narrow fix. I found no correctness regression in the changed line.

At [rt.js:1150](/tmp/rv-dt-glue3/host/web-js/rt.js:1150), the normal `rAF → setTimeout` path stays intact. When frames never arrive, the independent timer starts loading the glue. Both callbacks resolve the same Promise, so even if frames resume during the import, initialization and the `inflight` decrement happen once. Repeated `input()` calls remain guarded.

The fallback uses wall time without advancing the agent clock. [The render guard](/tmp/rv-dt-glue3/host/web-js/rt.js:1145) still prevents scheduling or loading under `__exactRender`. The deliberate tradeoff is that a page which has not painted after a second may initialize its glue before its first paint; normal startup retains the existing deferral.

Two related correctness gaps remain, both pre-existing:

- **The dev fallback cannot start loading on a page that never calls `input()`.** [inspect-dev.js:490](/tmp/rv-dt-glue3/host/web-js/inspect-dev.js:490) imports `/rt.js`, but [the bundler](/tmp/rv-dt-glue3/host/web-js/build.mjs:578) emits `app.js` and hashed chunks. For example, holding `hover` on a plain node in an app with no input-glue consumer still encounters the 404 and eventually reports “the input glue is not loaded.” This needs a callable entry into the bundled runtime; the new timer helps only after `input()` runs.
- **The wasm client has the same frame dependency.** [glue.js:1493](/tmp/rv-dt-glue3/host/web/glue.js:1493) gates `activate()` on nested rAF callbacks; [input-glue loading](/tmp/rv-dt-glue3/host/web/glue.js:1473) sits inside that activation. Reloading a wasm client page in the same offscreen iframe can therefore stall glue and data activation. It needs equivalent fallback handling if this scenario must work there. Ensure activation runs once—adding an independent `setTimeout(activate, 1000)` would permit duplicate activation.

The principal test gap is that [hold-wait.test.mjs:10](/tmp/rv-dt-glue3/host/web-js/hold-wait.test.mjs:10) supplies `exact.hold` itself; it never exercises the runtime loader. Add a regression that withholds rAF, fires the fallback, observes initialization, then releases the saved frame callback and verifies no second initialization. Cover the normal frame-first path, import failure, unchanged agent time, and the render guard alongside it.

Smaller points: the losing callback remains scheduled, producing one harmless extra wakeup. Also, “1 s” is a scheduling delay, not a loading deadline: background timers can be delayed, and frozen pages suspend timer callbacks. [Chrome timer throttling](https://developer.chrome.com/blog/timer-throttling-in-chrome-88/), [page lifecycle](https://developer.chrome.com/docs/web-platform/page-lifecycle-api).

Validation: four existing hold/inspect tests passed. Seven in-memory probes of the actual `input()` body, with loading stubbed, passed for callback ordering, duplicate calls, guards, failure, and accounting. I did not rerun the browser T4 gate. No files changed.
