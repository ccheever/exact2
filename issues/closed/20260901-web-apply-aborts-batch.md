# A missing view or bad fetch body aborts the rest of a web batch

**Status:** Closed
**Resolution:** Web batch application now isolates individual operations so one DOM refusal does not abort later operations.
**Systems:** Web host
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 0382 (fail closed loudly)

`glue.js` `apply` does `views.get(op.id).style.cssText = op.css` and `views.get(op.id).animate(...)` without a guard. Apple's presenter uses `views[id]?.applyStyle` and `guard let parent`. A missing id throws, and the rest of the kernel's committed batch is not applied — the DOM is then a prefix of the tree.

`request` does `atob(body)` in the same loop. Invalid base64 throws the same way. Grant refusal calls `fulfill` re-entrantly inside `apply` (see `20260901-web-fetch-reload-tickets`).

The kernel will not emit style for an unknown id on a healthy path. A destroy/create race, a glue bug, or a corrupt batch should skip and name the id, not desync the rest of the page.

Fix: skip missing views with `console.error`, wrap `atob` / `fulfill` so one op cannot abort the loop, and queue grant-refusal fulfills after the batch.
