# A tool handler's fetch after its first await runs as the app

**Status:** Closed
**Systems:** Web JS data (`host/web-js/ts-data.js`), fetch origin (`host/web-js/ts-fetch.js`), local-network admission (`host/web/address-space.js`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A `tools` handler that awaits, then fetches or calls a module, does that work as the app. The local-network check treats the app as allowed to reach loopback and private addresses. The handler was started with the tool's origin, which is not.

**Cause.** `data.tool` records `answering.call` with the handler's origin, invokes the handler, and clears `answering.call` in a `finally` that runs when the handler returns its promise. The call is never added to `answering.untraced`. After that await, `fetchOrigin` uses another unsettled untraced call if one exists, and otherwise `'app'`. `__exact_module` reads `answering.call?.origin` and substitutes `'app'` when the call is gone. `reachRefusal` returns immediately for origin `'app'`. An ordinary answer does add a non-app call to `answering.untraced` before clearing `answering.call`, which is why its later fetches keep an origin.

**Reproduction.** Export a tool handler that `await`s once, then `fetch`es `https://127.0.0.1/` while the app's grants include `net.fetch https:` and the loopback permission is still unanswered. The fetch is sent. The same URL fetched before the await is refused as `person-required`.

**Fix.** Keep the tool call in `answering.untraced` until its promise settles, as an answer's call is kept, and leave it as `answering.call` for the synchronous part of a module call made from that continuation.

**Acceptance.** A fetch and a module call made after the handler's first await still carry the tool origin. With loopback permission unanswered, that fetch is refused. A fetch made while the handler is still synchronous is unchanged.

## Resolution

Retain non-app tool calls in the pending-origin set until they settle, restore nested synchronous context, and carry the same origin through module forwarding and nested fetches.

## Verification

The async-tool regression in `host/web-js/upload.test.mjs` passes synchronous, post-await and module-forwarded local-network refusals, synchronous-throw cleanup and a subsequent app-origin request.
