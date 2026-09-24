# A flowed link runs both navigate and press

**Status:** Open
**Systems:** Web host, Text flow
**Severity:** P1
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1043.000

A `wrap-flow` inline link is detached and replaced by a `cloneNode(false)` clone (`host/web/textflow-glue.js`). Expando properties are not copied, so the clone has no `exactHandlers`. The document click listener, in capture, treats it as a plain link. The clone then forwards a second click to the original, which still has `press`.

On a real `<a>`, `press` makes `host/web/input-glue.js` prevent the default and skip `navigate`. On the clone, `press` is false, so a same-origin declared route is navigated in-app, and the forwarded click still dispatches `press`. If the root has no `navigate` handler, the interceptor returns without `preventDefault` and the browser does a full load as well.

Copy `exactHandlers` onto the clone, or make the interceptor read `data-exact-on`, which `cloneNode` does copy. Done when a flowed link with `press` does only what an unflowed link with `press` does, and a flowed link with no `press` still navigates once.
