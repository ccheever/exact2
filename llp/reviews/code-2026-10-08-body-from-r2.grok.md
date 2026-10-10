# Code review round 2: exactBodyFrom (efa4789c5), grok

Reviewer: Grok 4.7 (xhigh, read-only), 2026-10-08, on the detached review worktree at efa4789c5, after round 1 was folded.

I'll review the round-2 diff read-only: whether each fix closes its round-1 finding, and whether anything new blocks landing.The request is a read-only defect review of `HEAD~1..HEAD`, so I’ll inspect the diff and the round-1 findings without writing anything.Round-1 findings are in. Next I’ll read the patched paths and check each claimed fix against the code that actually runs.The JS-target deadline still starts the upload after `exactTimeout` has elapsed. That blocks landing. The other round-1 fixes hold.

**[P1] `fromFile` sends the file after the deadline.** `host/web-js/ts-fetch.js:53`

`fromFile` races the read against `AbortSignal.timeout`, then in `finally` removes that listener and always continues:

```54:55:host/web-js/ts-fetch.js
  const left = ms === undefined ? undefined : Math.max(1, Math.ceil(ms - (clock.now() - started)));
  return fetchWith(tsGrantSet, input, { ...rest, body, ...(left === undefined ? {} : { exactTimeout: left }) });
```

If the read settles once `performance.now()` is already past `exactTimeout` but before the timer callback runs (a stalled event loop that delivers the file task first, or a throttled timer), the timeout listener is gone and `Math.max(1, …)` turns “already overdue” into `exactTimeout: 1`. `fetchWith` then calls `browserFetch` with the blob. A 10 ms deadline and a read that finishes at 11 ms still uploads. It uses `globalThis.performance` and `AbortSignal.timeout`, so it does avoid the module’s guarded `performance` and `setTimeout`.

Fix: if `ms` is set and `clock.now() - started >= ms`, throw the timeout `FetchError` and do not call `fetchWith`. Leave the original timeout listener in place until that check has run.

**[P2] An already-aborted wasm read waits for the whole file.** `host/web/http-body.js:195`

The read is raced with `signal.addEventListener('abort', …)`. An `AbortSignal` that is already aborted does not fire that listener. Closing the page aborts the controller in `glue.js` before `request()` subscribes (helpers still loading, or the turn already aborted). `bodyFile` then runs to completion, and only the check after the race returns “request aborted”. Nothing is sent. The caller stays pending for the whole read, which is the abort half of the round-1 finding.

Fix: if `signal.aborted` before starting `bodyFile`, return the timeout or abort failure immediately. `fromFile` already does this with `if (signal?.aborted)`.

The rest of the fold closes its round-1 finding:

- `typescriptRuntime` is digested only for a Hermes app (`bake/src/compat.rs`). An old shell’s baked id does not match, and it only polls its own cohort, so a new `exactBodyFrom` bundle is not activated on a prelude that would POST an empty body.
- `set_app_roots` creates the roots and pins `AppDirectories` once. Apple empties a fresh scratch tree in `endow` before that pin. Linux empties it in `store_snapshot` during boot, before the presenter builds the executor. The later activation empty is a no-op, and `set_roots` only unlinks `tmp/picked` inside the pinned directory. A root swapped after the pin is not followed.
- The native `execute` arms the deadline before `body::resolve` and refuses with `Timeout` or `Aborted` before `fetch_request`, so a finished read does not send. The read itself stays a blocking call. The wasm host keeps one signal across the read and the fetch, so a late timer still aborts the exchange.
- Windows returns `Unsupported` before any read. The 4096-byte cap is charged via `body_from` capacity and refused before enqueue. WebSocket plus `exactBodyFrom` is refused in the prelude, in `body_from_refusal` before the socket branch, and in `http-body.js` before `readSocket`. The web store checks the record size before `new Blob`.

**BLOCKS landing: yes.** The P1 on `fromFile` is the JS-target half of the round-1 deadline finding, and this commit does not close it.
