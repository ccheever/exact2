# JavaScript renders keep HTTP reads alive after their deadline

**Status:** Closed
**Severity:** P2
**Systems:** host/web-js, server rendering, HTTP
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, a JavaScript server render that reaches its deadline leaves its HTTP requests running after returning the placeholder page. Repeated requests to a page whose upstream response stalls accumulate open reads and retain their render contexts beyond the render budget.

`host/web-js/build.mjs:567–582` generates `__render`, which stops waiting at its deadline and returns the pending resource names. `host/web-js/render.mjs:182–212` composes the 503 page and finishes without disposing that render. Its fetch wrapper at line 148 passes requests to the shared cache without a render-owned cancellation signal. There is no cleanup path for the unfinished reads. The native renderer explicitly drops its executor at a deadline (`host/render/src/lib.rs:503–507`), aborting its remaining requests.

Run `bun /tmp/exact-review-render-deadline.mjs`. The fixture builds a temporary app with the actual `host/web-js/build.mjs --render js`, a TypeScript resource that reads a local HTTP response, and a request-rendered route. Its local server holds each response body open. Three calls through the actual renderer, each with a 200 ms deadline, report:

```json
{"statuses":[503,503,503],"began":3,"cancelled":0,"bodiesOpen":3}
```

The response bodies remain open 100 ms after all three renders finish. The script then closes its own response streams, stops its server, and removes the temporary app. This exercises the actual generated app, fetch path, and renderer on Bun 1.4.2; it does not measure retained heap or a deployed server under load. The fixture uses GET reads, so it does not depend on a policy for writes that outlive an application resource.

Give each render an owner for its HTTP work and timers. Dispose that owner in a `finally` path when rendering ends, aborting unfinished reads and preventing their continuations from keeping a discarded realm alive. Verify both a stalled response header and a stalled body after the render deadline.

No duplicate found in the available exact2/exact issue corpora, including closed tickets. The closed `20260924-render-overload-blocks-accept.md` concerns the native server's accept loop, and `20260925-async-cc43410e.md` concerns a historical native placeholder test failure.

## Resolution

Give each render an AbortController and timer owner. Dispose it in finally when rendering finishes, aborting unfinished reads and clearing timers; compose caller cancellation with render cancellation.

## Verification

`bun test host/web-js/http-cache.test.mjs` builds a real JS-rendered app and verifies all six stalled header/body requests close after their 503 render deadline.
