# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), narrow re-check 1, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `d7da823515`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 9cf6884bb0 (a jump's highlight runs from its landing; the jump test pins topKey through the lead) and 8b26ddea8e (openChannel clears a stale topKey; the body test holds the headers' limits).

The lead can still page before a jump or a restore has finished placing the reader. Abort is gone, the live-end lead does not chain, and the limiter change is right.

## Findings

### 1. Should-fix: the lead can page before placement has settled

A jump that is still owed cannot start the lead. `leadOlder` requires `target == ""` (`app.contract:367`), and `older` checks it again before moving the anchor (`app.contract:624-626`). `landJump` is what clears `target`, and it does that only after `scrollIntoView` (`app.contract:680-685`).

The settle wait is `highlight == ""`. `unhighlight` clears it 2.5 s after `highlight` was set (`app.contract:348-349`, `405-406`), not 2.5 s after the row is placed.

Two paths miss that wait:

- **Restore.** `openChannel` clears `highlight` (`app.contract:475`) and, for a saved place, sets `anchor` and `target` (`app.contract:485-489`). Nothing sets `highlight` again. As soon as `landJump` clears `target`, `leadOlder` is eligible on the next tick. `topKey` is only written from `scrolled` (`app.contract:613`), so reopening the same channel still has a `topAt`. A cached history window (`anchor != ""` and `win.hasAfter`) then moves the anchor to `win.firstId` and `fetchPage` asks for the older page. That is a lead page immediately after `scrollIntoView`, which is the macOS failure `420666666f` added the highlight wait for. The new test only covers a jump (`app.test.contract:849-878`).

- **A jump whose around page outlives the highlight.** `jumpTo` sets `highlight` and `target` together (`app.contract:637-647`). `unhighlight` does not wait for `current` or for `land`. The added test lands history while the highlight is still set (`clock +400`, then `highlight` is still the target at `app.test.contract:865-869`) and only then steps past 2.5 s. If the around page arrives after those 2.5 s, `highlight` is already `""` when `landJump` clears `target`, and the lead runs `after(1)`.

### 2. Nit: one abort comment is still there

`live.test.ts:779-780` still says a page in flight is aborted when the reader leaves, switches, or jumps. `d7da823515` removed those tests and left the comment above the lead test.

Nothing else from that feature remains. There is no `AbortController`, no `abortPages`, and no abort call under `apps/excord`. `live.ts` matches `origin/main` except the body-failure `try` in `call` (`live.ts:594-600`).

`live.test.ts:116-123` and test 6 (`live.test.ts:391-400`) are on `origin/main`. They watch the fixture request's own signal and assert that a second ask does not abort the page already in flight.

## Clean

**Live end and history.** The new gate is `away or (anchor != "" and win.hasAfter)` (`app.contract:367`).

- At the live end, `away` is false. The 15-row rule can set `anchor` to the first row (`app.contract:605-607`). `loadWindow` then names one older page, because that anchor sits within 15 of the top (`app.ts:319`, `EDGE` at `app.ts:282`). After that page the window still ends on the newest message, so `hasAfter` is false (`app.ts:309`, `shapes.contract:193-195`). Both sides of the gate are false, and the lead does not take a second page.
- A window that already reaches the newest message has `hasAfter` false, so the new clause does not fire. If the reader has scrolled up, `away` is true and the 100-row lead still runs. That `away` gate was already there.
- In history the anchor is set and `hasAfter` is true, so the lead does not wait for a scroll to set `away`. It still waits for a real row (`topAt >= 0` and `topAt < 100`), `reading`, `current`, `win.need == ""`, `win.hasBefore`, and `anchor != win.firstId`. With 100 rows already above the reader it stays quiet. A jump whose window still reaches the newest message stays on that one window until `away` is set, same as the short tail.

**`reading` on `fetchPage`.** `reading` means the channel route is the top screen and the lock is not up (`app.contract:266`, `370`). The guard was already on `fetchPage`; these commits kept it.

- Launch restore, a host route, and a deep link all end in `openChannel`, which puts the channel route on top (`app.contract:504`, `routing.contract:173-185`). `fetchPage` runs on the next tick. While locked, the transcript is covered and the page is asked after unlock.
- Forward to a picture changes nothing (`routing.contract:95-96`).
- The browser split keeps the channel route when the list's address is shown (`routing.contract:97-98`). The routing drive expects `reading` to follow that split (`fixture/routing.e2e.test.ts:352`).
- The narrow list and the photo route are not the channel screen (`inert=covered` on the channel column). The page already on the wire still finishes. The next one is asked when the channel route is on top again.

**Limiter.** On a body failure, `call` reports that response once, with its status and headers (`live.ts:598-599`), then throws. `onResponse` drops `inFlight` (`discord/rest.ts:217-219`), applies `X-RateLimit-*` (`discord/rest.ts:221-232`), and blocks on a 429 (`discord/rest.ts:234-243`). The success-path `onResponse` does not also run.

The test is a real proof of the slot. `live.test.ts:812-827` sends a 200 whose body errors, then issues a second page. `asked` becomes 2 only if that second `fetch` runs. If the limiter still held the route, `admit` would refuse before `fetch` (`live.ts:582-586`, `discord/rest.ts:176`) with "Another request on this route is still out", not "connection reset". If `paging` were still held, the second `page` would throw "still loading" before `fetch` (`live.ts:612`). The mock sends no rate-limit headers, so the same test would pass if the `catch` went back to status 0 and empty headers. Header application is what the code does, and what the test does not lock.

**RFC 0001.** History is still at most 50 messages a page, one page in flight per channel (`llp/0001-excord.rfc.md:102-104`). `page` still refuses a second page of that channel until `call` finishes (`live.ts:612-613`, `627-628`). Removing abort leaves the slot occupied until the request ends, which is what keeps a second page off the wire. The four queries are still `limit: 50` (`app.ts:330-334`). No new request shape. `app.contract` is 1,499 lines.
