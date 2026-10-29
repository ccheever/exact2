# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), round 1, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `9e4ceebb47`.
- **Method:** one brief (sha256 `3fcad3f1652b56fa60114713f9a5e527febf02aaa93e8803c246e3549a9aa35c`), blind to the other reviewer; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 1c454f3957 and b2c3be30a5: a body that fails after its headers gives the route's limiter slot back (tested red without, green with); the lead no longer chains at the live end; tests and stale comments fixed.

The lead stops, and one abort does not stick in `live.paging`. An abort that lands while the history body is still being read does wedge that channel's later pages.

## Findings

**1. Blocker — an abort during the body read never releases the rate-limit slot, so that channel's history requests are refused from then on.** `apps/excord/live.ts:597`

`call` only reports the request back to the limiter if `fetch` itself rejects:

```597:606:apps/excord/live.ts
  try {
    response = await fetch(request.url, { method: request.method, headers: request.headers, body: request.body, signal });
  } catch (error) {
    rest.onResponse(limiter, request, 0, () => null, null, clock.now);
    throw error;
  }
  const text = await response.text();
```

The host resolves `fetch` when the headers arrive and aborts the body separately (`vendor/ibex/crates/ibex2/src/bindings/fetch.js:144` returns the `Response`, and `observe` at line 158 fails the body). `response.text()` then rejects outside that `catch`, so `onResponse` never runs and `bucket.inFlight` stays above zero. `admit` treats `remaining === null && inFlight > 0` as an infinite wait (`apps/excord/discord/rest.ts:176`), which is the state before any rate-limit headers have been seen (the fixture never sends them). The next history GET for that channel throws `Another request on this route is still out` before a new `fetch`. `page`'s `finally` still clears `live.paging`, so retries are attempted, each fails the same way, and after three `historyFailed` shows "Couldn't load more messages." Retry does not clear `inFlight`. This is the phone-network case F1 aborts on: headers in, body still downloading, reader leaves or jumps.

**2. Should-fix — once any anchor is set, the lead keeps paging while the reader is still at the live end.** `apps/excord/app.contract:369`

`scrolled` uses 100 only when this event is away, and 15 otherwise (`app.contract:613`). `leadOlder` does not. Its gate is `away or anchor != ""`. The 15-row branch sets `anchor = win.firstId` while `isAway` is still false (a short tail whose first visible row is within 15 of the top and whose last row is still on screen). The lead then treats that anchor as permission to chain until `topAt >= 100`, and `follow` / `ack` both require `anchor == ""`. The comment at `app.contract:577` says the 15 exists so a short tail does not leave the live end by itself. The chain undoes that. `anchor != ""` is what lets the lead run after a jump before `away` flips, so the fix is to keep that case and not the at-end 15-row case.

## The six checks

**1. Lead.** It stops, and it does not run away. Each landed page moves the reader's row down by 50 (`live.test.ts` expects index 50 after one page; the transcript world puts the §3.4 reader at index 9960 and the new `firstId` at 9851, which is 9 + 50 + 50 = 109). `topAt < 100` then fails, so the next `leadOlder` does not fire. The worst page-by-page stop is a reader at 99, plus one page of 50, which is 149. `fetchPage` is held by `win.need == ""` and `not pending(paged)`; `page` refuses a second call while `live.paging` has the channel. `leadOlder` and `older` both require `target == ""`, so a jump's around page is the only request while `target` is set. At the live end with `anchor == ""` and `away` false, the lead does not start. There is no cycle: `leadOlder` only writes the anchor, `fetchPage` only sends the one page `loadWindow` named, and the key `win.firstId` changes when a page actually prepends rows.

**2. Abort.** `view` keeps the channel being opened and aborts every other (`live.ts:571`). The around branch keeps that jump's own query and aborts the same channel's other query (`app.ts:321`). A kept around page is not cancelled by a later `loadWindow` of the same jump. `page`'s `finally` deletes only its own map entry (`live.ts:631`) and does run when the aborted `fetch` rejects, so `live.paging` does not stick and the identity check does not block the next page. One abort completes the mutation as `ok: false` and increments `pageTries`, but `pageNow` resets `pageTries` when `pageKey` changes and is scheduled `after(1)`, which is after `afterPage`. That is not a retry storm and it does not show `historyFailed` by itself. The wedge in finding 1 is the limiter, not `live.paging`.

**3. Cap 300.** Nothing else still assumes a 200-row window in a way that breaks. `windowAround` centers with `floor(cap / 2) = 150` (`store.ts:1037`). The lead's anchor is the first shown row, about index 51 after a page lands, which is inside that half, so the page just prepended stays in the slice. Callers that pass `200` still get 200 (`windowLimit` is `min(300, limit)`). `HALF = 100` still places the pin 100 from the end of the latest window, farther than `EDGE` from both edges, so the pin does not name a page. The store clamp test was updated to 300. The comment "200, plus the two pages" undershoots the 149-row lead, but the 300 cap still holds the landing page.

**4. RFC.** No new Discord request shape. Every history query is still `limit: 50` with one of `before` / `after` / `around` (`app.ts:335`). `--history-delay` only stalls the fixture's `before` reply. One page in flight per channel: the map is one entry, and `page` throws if it is taken (`live.ts:614`).

**5. Tests.** The changed contract anchors match the lead's stop, not a runaway. In the transcript world, §3.4's reader is message 9960; the new anchor is 9901 and `firstId` is 9851 (109 above the reader, the old one-page figures plus one more page). The away-from-end anchor 9901 is the same stop. The jump anchor 6681 is 74 before target 6755, one lead step past the around window, not the channel start. Dropping the "anchor equals the jumped message" asserts follows that, and those tests still require the target row to be mounted, `target == ""`, and `pageTries == 0`. The `clock +3000` pairs advance the test clock (the slow pump is `every(150)`); two pages need about 300 ms of it, so they are not wall-clock races.

The new `--history-delay` test is wall-clock: it requires the undelayed replies to finish in under 250 ms and the delayed one in at least 290 ms (`fixture/transcript-server.test.ts:148`). That fails on a loaded machine.

The live "F1" test does not run `leadOlder`. It passes anchors into `loadWindow` by hand and then keeps re-anchoring until the window is 299 rows (`live.test.ts:796`), which is the cap, not the 100-row stop. The comment at line 786 says the lead stops within a page of 100. The abort test checks `view` and a copy of the keep predicate pasted at `live.test.ts:840`; it never calls `loadWindow`, and it never starts another page for channel A after the abort even though the comment says A may page again.

**6. Other.** No leftover `LEAD` constant (`compose.ts`'s `LEAD` is the mention regex). No new read-after-assign: the 100-vs-15 threshold reads the `isAway` let captured before `away` is written (`app.contract:593` and `613`). The file header still says paging moves the anchor only through `scrolled` (`app.contract:6`); the lead is the other writer.
