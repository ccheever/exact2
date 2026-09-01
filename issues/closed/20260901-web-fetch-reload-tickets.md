# In-flight web fetches keep their tickets across reload

**Status:** Closed
**Resolution:** Web requests now carry reload generations, abort superseded fetches, and ignore stale settlements.
**Systems:** Web host, Runner
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1016 D5 (a forgotten ticket is dropped; a still-wanted reply is not), LLP 1007 §6 (reload carries slots, matching resources, the clock, the store — not in-flight requests)

`glue.js` `boot()` clears the `inflight` set and the DOM, then calls `exact_boot` / `exact_boot_plan`. It does not abort the `fetch` promises. `fulfill` has no generation: a later completion still calls `exact_fulfill(ticket, …)`.

`Runner::boot` / `boot_carrying` starts `next_ticket` at 1 and does not carry `pending`. A fetch that left as ticket 1 on the old incarnation can land as ticket 1 on the new one (`Runner::fulfill` matches by number only) and commit into the new session.

The grant-refusal path is worse in a different way: `fulfill` is synchronous inside `apply`, so a refusal's commit is applied while the outer `for (const op of batch.ops)` is still running.

Caltrain's baked data does not fetch, so the smoke cannot see this. The dev loop and any `--plan` reload during a live `login`/`send` can.

Fix: an incarnation counter on `fulfill` (ignore tickets from a previous boot); `AbortController` per request, aborted in `boot`; do not call `fulfill` re-entrantly from inside `apply`.
