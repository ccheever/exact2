# The JS web runtime retains removed motion views for the page's lifetime

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, motion, DOM lifetime
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, repeatedly replacing a region or removing ordinary rows with motion leaves every retired element strongly reachable. Memory grows with the number of elements the page has ever displayed.

`host/web-js/rt.js:741-744` stores each registered element in the global `Views` Map. `mo` at lines 1018-1022 registers a view and installs a scope-end callback, but that callback only calls `motion.gone`. `host/web-js/motion.js:195` removes the engine's private state and destroys its browser motion state; neither it nor `host/web/motion-glue.js:638-647` removes the element from `Views`. Scope disposal (`rt.js:42` and `:131`) has no other view cleanup. Virtualized lists remove their own wrapper/root entries, but ordinary region and row elements do not take that path.

Run `bun /tmp/exact-review-view-retention-browser.mjs`. It serves repository JavaScript directly to the pinned Chromium, imports the actual runtime, then creates 100 real DOM sections, calls `mo` inside a runtime scope, removes each section, and ends its scope. The result is `{"retainedViews":100,"disconnected":100,"liveChildren":0}`. The reproduction sets `__exactRender=true` to skip loading the motion WASM; the installed engine's cleanup was checked separately in source. This exercises runtime registration and scope disposal, not a compiled app or a heap-size measurement. No source files were changed.

Remove retired entries once their consumers have finished, including presence exits and any active motion. Retain weak identity separately if reattached elements need stable IDs. Add a lifetime check that repeatedly replaces an ordinary transitioned region and verifies its disconnected views leave the registry.

No duplicate found in the available exact2/exact issue corpora, including closed tickets. The older `exact/issues/20260805-webgpu-detached-registry-scan-spin-burns-cpu-in-suite-runs.md` concerns native GPU test workers and does not cover this DOM registry.

## Resolution

Release disconnected view entries on scope retirement and actual DOM removal, retaining connected presence exits and stable identity in the weak ID map.

## Verification

`bun test host/web-js/layout-transition.test.mjs` passes a compiled app transition drive and Chromium checks for 100 retired motion views, a connected exit, later removal and reattachment.
