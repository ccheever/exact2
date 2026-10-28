# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), round 2, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `e75974479f`.
- **Method:** one brief (sha256 `4a9d525e2f18ceaf143028b22a321651a1a3fb0362cdb3becc1003ac63989ed7`), blind to the other reviewer; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Both reviewers found the abort unsafe on native (the runtime drops the reply and keeps the request running): removed entirely in d7da823515, so a page in flight lands as on main and one page per channel stays on the wire; caps (app.contract over 1,500 lines) fixed; the lead starts in history after a jump or restore.

The `reading` guard does not stall a window that has to load, and an aborted page is asked again. Two should-fix gaps remain: the lead does not start on its own after a restore or a jump, and on the native runtime an abort frees the channel's slot while that request is still on the wire.

## 1. Should-fix — the lead does not start after a restore or a jump until a scroll is seen with no jump owed

`leadOlder` now runs only while `away` is set:

```371:372:apps/excord/app.contract
  task leadOlder when reading and current and target == "" and away and win.need == "" and win.hasBefore and topAt >= 0 and topAt < 100 and anchor != win.firstId key=win.firstId
    after(1, older)
```

`away` is set in one place, `scrolled`, and only when the reader has moved, nothing is owed, and the list is not settling (`app.contract:603-604`). `openChannel` clears it on every open, including a restored place (`app.contract:481`). The restore copies `anchor`, `target`, and `offset` (`app.contract:490-494`) and does not copy the place's `atEnd` into `away`. It also leaves `topKey` alone, so `topAt` (`app.contract:257`) stays the previous channel's row until a scroll. `jumpTo` (`app.contract:647-658`) and a deep link (`routing.contract:179-184`) set the anchor and the target without setting `away`. `topKey` is written at the end of `scrolled` even while a target is owed (`app.contract:623`); `away` is not.

So a reader put back into history does not grow the lead until some later scroll is delivered with `target == ""`. That is the host `leadOlder` is for: the new test says a prepend correction is itself a scroll on this host, and that `leadOlder` is what keeps the chain where the host sends none (`app.test.contract:129-130`). On that host the first scroll is the fling, which is the thing the lead was meant to have loaded for.

What still loads without the lead:

- A restored anchor within `EDGE` (15) of the window start names one older page (`app.ts:329`). After it lands the anchor is about 50 rows down, the edge rule stops, and nothing moves the anchor to the new first row. A reader who was at the top then has about 50 rows above them.
- An uncached jump gets one 50-row `around` page. `windowAround` centers it (`discord/store.ts:1033-1038`), so the target sits near row 25, past the edge, and `loadWindow` names no older page at all. About 25 rows stand above the target.

Apple and the web list usually recover, because the landing scroll is queued after `landJump` has cleared `target` (`app.contract:691-695`; `NodeViewIOS.swift:813` and `NodeViewMac.swift:678-688` queue it on the next turn; `list.js:733-741` only records a correction). A landing scroll whose offset does not change is not sent (`NodeViewIOS.swift:819`, `CollectionIOS.swift:337`), and a scroll delivered while `target` is still set records `topKey` without setting `away`. The in-channel fling is intact: that scroll sets `away`, and at the live end the threshold stays 15 (`app.contract:616`), so a short tail still gets one page.

## 2. Should-fix — on native, aborting a page lets a second history request for that channel onto the wire, and drops a 429

The stated native behavior holds. Abort rejects the fetch promise immediately and does not cancel the host request (`js/src/prelude.js:744-745`, `js/src/prelude.js:767`). The later reply, including a 429 and its rate-limit headers, is dropped (`js/src/prelude.js:1483-1494`). `page()` then returns without throwing (`live.ts:628-629`), `call()` reports the attempt finished (`live.ts:599-601`), and `finally` frees that channel's slot (`live.ts:642-643`). Nothing is written into the store. On the web host the browser request is cancelled once the ticket is released (`host/web/glue.js:952`).

Main never aborted, so the slot and the limiter's `inFlight` both stayed held until the response arrived. An unknown bucket refuses another request on that route only while `inFlight > 0` (`discord/rest.ts:176`). F1 clears both at once, while the native request is still running. `before`, `around`, and `latest` on one channel are one route: the query string is stripped and the channel id is the major parameter (`discord/rest.ts:22-42`, `discord/rest.ts:64-65`). A jump to an uncached message, or Jump to Present with no tail cached, aborts the older page and asks another page of the same channel, so two history GETs for that channel are in flight together. Switching channels is a different route, which main already overlapped; switching back before the aborted request finishes is the same overlap. The aborted response's `retry_after` never reaches `onResponse`, so `blockedUntil` is not set (`discord/rest.ts:234-242`).

One extra request is the ordinary case. A pile-up takes repeated aborts before those replies return. The app map still holds one page per channel (`live.ts:620`). The same `onResponse(0, null headers)` path throws away `response.status` and the headers when the body fails (`live.ts:605-608`). That gives the slot back, which is the fix, and it also ignores a bucket that had already arrived. On this runtime `Response.text` is the buffered body, so that body path is the web one.

## 3. Nit — the new lead test passes on this host without `leadOlder`

`app.test.contract:120-136` expects the lead to reach `topAt == 109` and then stop. Its own comment says the prepend correction is a scroll here, and that `scrolled` follows it. Deleting `leadOlder` still satisfies the test on this host. The comment is accurate. It does not pin the task.

## Reading guard — nothing found

`reading` is the channel route on top and the screen unlocked (`app.contract:268`). Launch restore, a host route, a deep link, and wide resume all end in `openChannel` or `stayOpen` (`routing.contract:72-83`, `routing.contract:140-144`, `routing.contract:173-179`), which puts that route on top, so the gate becomes true in that commit and `fetchPage` arms. A gated task arms again when its gate becomes true, including with the same key (`docs/agent-pitfalls.md:290-296`). Wide home does not show the transcript. The lock and a photo make `reading` false, so paging waits; closing either makes the gate true again. The photo path does not abort the page already in flight. Tray notifications only bring the window forward (`controls.contract:80-83`). No path needs a page while the transcript is not the screen in front.

## Aborted page counted as success — nothing found

An aborted `page()` returns without throwing, so `write` resolves ok and `afterPage` sets `pageTries` back to 0 (`app.contract:427-430`). `fetchPage`'s gate includes `not pending(paged)` (`app.contract:375`), so the in-flight request drops the gate and the ok result raises it again. That re-arms the same `` `${pageKey}/${pageTries}` ``. Leaving and returning goes through `openChannel`, which sets `pageTries` and `pageAsks` to 0 (`app.contract:498-499`). A jump changes `pageKey`, and `pageNow` starts `pageAsks` at 1 when the key differs (`app.contract:422-425`). `historyFailed` would stop asking after three asks of one key (`app.contract:265`); nothing aborts the page the current window still needs three times. `view` keeps the open channel's page (`live.ts:571`). Re-viewing it does not abort and does not reset the counters.

## Latest-branch abort — nothing found

The abort runs only for an uncached tail: no anchor, no confirmed first row, and `fetchLatest` or some rows (`app.ts:314-319`). The keep function retains every other channel and this channel's `latest` query, built with the same `QUERIES.latest` factory `loadPage` uses (`app.ts:340-342`). `AbortController.abort` is idempotent, and a later read keeps the `latest` query already in flight, so it does not abort that page again or start another request. A short cached tail names `before` and does not abort (`app.ts:321`). A cached jump never enters this branch. A missed anchor recurses into the tail window (`app.ts:286`); aborting the `around` page there is the window being shown.

## RFC — request shape and page size, nothing else

Pages are still `limit` plus one of `before`, `after`, or `around`, and `limit` is 50 (`app.ts:340-343`). `pageQuery` rejects a limit outside 1–50 and rejects more than one cursor (`discord/rest.ts:56-60`). `WINDOW_CAP` is 300 rows in the client window (`discord/store.ts:966-971`), and the contract header and the shape comment say 300 (`app.contract:4`, `shapes.contract:193`). `leadOlder` runs only when `win.need == ""`, and `fetchPage` runs only when it is not, so they do not ask two pages. The one-page-per-channel map still holds. The native wire overlap in finding 2 is the RFC miss: history is supposed to have one page in flight per channel (`llp/0001-excord.rfc.md:103`).
