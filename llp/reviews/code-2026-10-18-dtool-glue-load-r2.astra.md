# Code review: the JS page's input glue loaded where frames are held (LLP 1122 D8), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `b4cf3fbf75`.
- **Method:** one brief (sha256 `c9211f162cc18bbf4c0b7d8dcb01496689441961f33b44f320e4ae17f29b6e97`), shared by both reviewers, blind to the other review; commit e6598647a0. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken in `74693a204d` (both reviewers' one point): the runtime's pressFeedback handle is its own global, `__exactPressFeedback`, not set on `exact`, whose presence the dev loop takes to mean the runtime is up (a page whose boot threw would otherwise keep a partial `exact` and not be reloaded); the dev page calls it inside the promise, so a throw is caught. Not taken: a cold-load loader test (rt.js's `input()` needs a document; the reviewers' in-memory probes ran it, and the T4 gate's after-rebuild line runs the warm path end to end); the wasm shell's two-frame activation and hidden-tab timer clamps, as in round 1.

**One P2 correctness issue blocks this round.**

- **Early `exact` creation can break recovery after a failed boot** — [rt.js:1141](/tmp/rv-dt-glue4/host/web-js/rt.js:1141). Concrete scenario: `/__dev/inspect.js` is still loading, and the app’s boot throws before [build.mjs:406](/tmp/rv-dt-glue4/host/web-js/build.mjs:406) installs `clock`, `state`, and `ready`. On the next rebuild, the dev client now sees `{ pressFeedback }`, considers it ready at [dev.mjs:694](/tmp/rv-dt-glue4/host/web-js/dev.mjs:694), and attempts checkpoint capture. [dev.mjs:691](/tmp/rv-dt-glue4/host/web-js/dev.mjs:691) throws on `x.clock.now`; the reload handler retains the broken page. I reproduced this with the actual runtime and generated client in memory: round 1 reloads; round 2 reports “checkpoint handoff failed” and does not reload. Publish the handle with completed runtime setup, or distinguish a partial `exact` object from a ready runtime.

The accepted round 1 point **is fixed** at [inspect-dev.js:491](/tmp/rv-dt-glue4/host/web-js/inspect-dev.js:491). An in-memory bundle using the real runtime, inspector, and input glue successfully held `hover` on a plain node without any earlier `input()` call. Both timer-first and frame-first paths initialized once; releasing the losing callback caused no second initialization. `inflight` returned to zero and agent time stayed unchanged.

Other checks found no additional regression: existing `exact` properties survive, module evaluation starts no input work, the render guard still prevents loading, and the agent waits for boot separately.

Smaller point: declining a loader regression test because “rt.js needs a page” is not convincing. [hold-wait.test.mjs:10](/tmp/rv-dt-glue4/host/web-js/hold-wait.test.mjs:10) supplies `hold` itself, and the reported T4 exercise already had an input consumer. Neither covers this cold-load path or early publication. Deferring the unchanged wasm activation stall and hidden-tab timer limitations is reasonable for this JS-page change.

Validation: 14 existing tests passed, plus the in-memory probes above. Browser T4 was not rerun. No files changed.
