# A pending orientation lock survives removal of its last holder

**Status:** Closed
**Severity:** P2
**Systems:** web host, orientationlock
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, removing the last orientation-lock holder, or introducing a conflict, can leave the browser locked. This happens when the DOM changes after `screen.orientation.lock()` starts but before its promise callback runs.

`host/web/orientationlock-glue.js:85–90` sets `holding` only in the promise's fulfillment callback. `release()` at lines 95–97 calls `unlock()` only when that flag is true. The no-holder and conflict paths at lines 109–120 clear `asked` and call `release()` while the flag can still be false. The eventual callback then returns because `asked` changed, without releasing the browser's lock.

The Screen Orientation algorithm applies the active lock before the task that resolves its promise. The API's `unlock()` releases that active lock and rejects its pending promise. A local fulfillment flag therefore cannot decide whether the browser needs unlocking. See the [applying an orientation lock](https://www.w3.org/TR/screen-orientation/#applying-an-orientation-lock) and [unlock method](https://www.w3.org/TR/screen-orientation/#unlock-method) algorithms.

Run `bun /tmp/exact-review-orientation-pending.mjs`. It imports the actual, unmodified glue with a local `screen.orientation` stand-in that records an applied lock while leaving its promise pending, calls `hold()` for a portrait holder, removes all holders, and then resolves that promise:

```json
{"nativeLocked":"portrait","unlocks":0,"state":{"held":false,"by":[],"words":[],"lock":null,"conflict":null,"applied":"unavailable"}}
```

The helper restores its globals afterward. This proves the glue's ordering with an API stand-in, consistent with the specification; it does not exercise a physical device's orientation or browser lock implementation. Existing orientation tests resolve each requested lock before removing its holders and miss this window.

Track pending requests as owned work, call `unlock()` when a pending or applied request loses its holders, and invalidate stale callbacks by request identity. Preserve a newer requested lock when an older callback arrives. Cover removal and a conflict while a request is pending.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

## Resolution

Track pending native lock requests by identity. Release pending and applied locks when ownership ends, and ignore stale callbacks even when a newer request uses the same lock type.

## Verification

`bun test host/web/tests/orientationlock.test.mjs` passes Chromium stand-in checks for pending removal, conflict and out-of-order same-type callbacks, plus existing orientation cases.
