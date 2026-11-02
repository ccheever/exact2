# Text selection watches retain removed nodes until another selection

**Status:** Closed
**Severity:** P2
**Systems:** web host, text selection, memory
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, repeatedly mounting and removing text with a `selectionchange` handler retains every removed element and its callback until the document dispatches another selection event. A reader can navigate or scroll for a long time without changing a text selection, so the registry can grow with retired content.

`host/web/navigation.js:1452` holds a strong `selectedTexts` map. `onSelection()` at lines 1467–1480 adds the element and callback but returns no disposer. Its only deletion runs inside the document's `selectionchange` listener at line 1471. Both runtimes register through this helper (`host/web/glue.js:565`, `host/web-js/rt.js:958`); ending a view's scope does not clear the helper's entry.

Run `bun /tmp/exact-review-selection-retention.mjs`. In Chromium, the fixture registers 100 actual DOM paragraphs with the helper, removes them, and waits two frames. It then dispatches one selection event:

```json
{"before":{"retained":100,"disconnected":100},"afterSelection":0}
```

The served module is the actual `navigation.js` with one added read-only export of its private registry for inspection. Its registration and cleanup code are unchanged. This verifies DOM and registry retention; it does not measure heap use or drive a whole compiled app.

Return a disposer from `onSelection()` and call it when each runtime retires the element. Keep one document listener per registry lifetime, with explicit listener ownership; the current `if (!selectedTexts.size)` condition can also register another listener after a selection event empties the registry.

No duplicate found in the available exact2/exact issue corpora, including closed tickets. The separate `20261102-web-js-views-retain-removed-dom.md` concerns the JavaScript motion view table; this helper also affects the wasm host and text without motion.

## Resolution

Return an idempotent selection disposer and call it at JS scope retirement and wasm view destruction/reboot. Use one named document listener, removed when the registry empties.

## Verification

The text-selection regression in `host/web-js/layout-transition.test.mjs` passes Chromium checks for 100 retired JS scopes, listener ownership and replacement-disposer safety. Wasm teardown wiring is covered by source review; a wasm app was not launched in this lane.
