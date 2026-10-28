# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), narrow re-check 2, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8b26ddea8e`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 70ea4efa83 and 48f9f4d96a, later replaced by leadHeld (4d87dbeab6) after the third re-check. Separately, 5fed875bd9 fixed a web test flake found in the full suite.

The two commits fix the five items from the first re-check. One low side effect of clearing `topKey`. The highlight timing, the other `topKey` readers, the jump assertion, and the rate-limit test are clean.

## 1. Low — re-selecting the open channel leaves `topKey` empty until something scrolls

`openChannel` always clears `topKey` (`apps/excord/app.contract:476`), including when `id == open`. The place of the channel being left is saved earlier, and only when the id actually changes (`452-453`). The transcript list is `for ch in [open] key=ch` (`1351`), so the same channel does not remount and `scroll-start="end"` does not run again. A re-tap at the live tail also takes the `else` branch (`490-492`): `anchor` and `target` stay `""`, so nothing calls `scrollIntoView`.

`topKey` then stays `""` until a later `scrolled` (`613`). Two readers change in that window:

- Fixture edits. `injectNow` passes `topKey` as `near` (`1134`). `message`, `burst`, `typing`, `japanese`, and `links` ignore `near` (`apps/excord/fixture/inject.ts:33-59`). `edit-above`, `delete-above`, `react-above`, and `embed-above` do not: an empty `near` selects `history.length - 11` (`71-72`), while a real id selects three rows above it. The menu labels are “a message above” (`app.contract:1317-1324`). After a click on the channel that is already open (`directory.contract:91`, `103`), those commands hit the fallback row. A switch to a different channel already did that, because the previous channel’s id is absent from the new history and `findIndex` misses.
- A missed jump’s return row. `jumpTo` stores `jumpFromKey = topKey` only for an id that is not in the current rows (`637-647`). `openWant` overwrites that with `"end"` (`routing.contract:181-188`). If `jumpFromKey` is still `""` and the target is not in history, `landJump` assigns `target = ""` (`app.contract:689-695`). The land task is `target != ""` (`351`), so it does not scroll back to the previous row. `anchor` is still restored to `jumpFromAnchor`.

A restore that does set `target` (`485-489`) still calls `scrollIntoView` from `landJump` (`681-682`). That is the report the lead is meant to wait for. This finding is the reopen that does not move the list.

## (a) The other `topKey` readers are clean

**Place saved on leaving.** On a real channel change the place is written at `453` from the current `topKey`, and the clear is at `476`. `leaving` (`558`) reads `topKey` at leave time. The only way that value is the `""` just written by `openChannel` is if no `scrolled` has run since. The same action set `away = false` (`475`), and `atEnd` is `not away`, so the stored place is at the end. `placeOf` restores only when `not p.atEnd` (`442`, `485`). A later `scrolled` that sets `away` also assigns `topKey` in that same action (`594`, `613`).

**Prepend restore.** `afterPage` pins only when `historyStart` and `topKey != ""` (`427-430`). `openChannel` clears both (`476-477`). `scrolled` sets both (`612-613`). There is no commit where a stale key is pinned, and a prepend before the first report is skipped because `historyStart` is false.

**`scrolled`’s move check.** `moved` is `k != topKey` or an offset change past half a pixel (`592`). The first report whose key is non-empty is a move, so the previous channel’s `topOffset` (not cleared) does not suppress it. `topOffset` is consulted only when that report’s key is also `""`.

**Lead after a real report.** `leadOlder` requires `keys.indexOf(topKey) >= 0` and `< 100` (`366`), the same test as the removed `topAt` derive. `""` is `-1`, so the lead stays quiet until the placement’s `scrolled` stores the reported row. Inlining the call twice is the same value in one gate. `app.contract` ends at line 1500, which is the cap.

## (b) The wash people see — clean

The yellow wash is `lit=(m.id == highlight)` (`1357`) with opacity `0.22` (`transcript.contract:89-90`). `unhighlight` is armed only while `highlight != ""` and `target == ""`, then waits 2.5 s (`347-348`, `404-405`). The gate drops the timer while it is false, and raising the gate starts a new 2.5 s, same as any other `task … when` (`docs/contract-for-humans.md:1426-1428`).

- A jump that lands. `jumpTo` sets `highlight` and, if the id is outside the rows, `target` (`637-647`). `landJump` clears `target` in the same action as `scrollIntoView` (`680-685`). The 2.5 s starts then, so the wash is on screen for 2.5 s after the row is placed. That is the change from the old timer, which started when the jump was asked.
- An in-window jump, including one already in the current rows. `target` is left alone (`639-640`). With `target` already `""`, the timer still starts at the click.
- A cached jump whose id is not in this window. `loadWindow` answers from the store without `need: "around"` (`app.ts:288-320`). `land` is `after(1)` once `current` and the window is not pending (`351-352`). The wash runs from that landing, one millisecond after the cached window is current.
- A jump that never lands. `clearHighlight` does not run while `target` stays set. Nothing is painted yellow: `jumpTo` sets `target` only when the id is absent from `rows` (`639`), and an anchor the store does not hold is an empty window (`app.ts:300-318`, `need: "around"`). The miss path clears `highlight` itself (`689`). A failed around-page shows “Couldn’t load messages.” on an empty list (`1358-1363`). `openWant` can light an id that is still in the previous rows while the new read is pending (`routing.contract:181-185`; a pending read keeps the previous value, `docs/contract-for-humans.md:641`). That lasts until `loadWindow` answers, not for the life of a stuck `target`. With slow history, `askAnchor` lags until the 150 ms pump (`214`, `1147-1148`); after the pump an uncached anchor is the empty window, so the wash is gone until the page arrives and `land` starts the 2.5 s.

`leadOlder` still requires `highlight == ""` (`366`), so it waits out that 2.5 s. The 800 ms opacity fade starts when `highlight` is cleared; the lead is tied to the clear, and the placement has already been sitting for 2.5 s.

## (c) The rest of the two commits — clean

**Jump test.** After the landing it expects `topKey == "1518631444790909538"`, and the same value after the lead has moved `win.firstId` (`app.test.contract:867-880`). `topKey` is written only from `e.firstVisibleKey` (`613`) and from this clear. A prepend that changed the first visible row would change it. The target row staying mounted was the old assertion (`tree has "message-…"`); this one is the reported first row, which is what the lead reads. The removed `topAt == 109` check (`app.test.contract` around the lead test) had to go with the derive. That test still locks the stop: `firstId` is unchanged and `pageTries == 0` after five seconds (`133-139`).

**Body-failure test.** The second reply carries `X-RateLimit-Remaining: 0` and `X-RateLimit-Reset-After: 5`, then fails the body (`live.test.ts:816-830`). `call` applies those headers in the `response.text()` catch and rethrows (`live.ts:594-600`). `onResponse` records `remaining` and `resetAt` before it looks at the body; the body is used only for a 429 (`discord/rest.ts:221-234`). The next `admit` sees `remaining <= 0` and a reset 5 s ahead, and throws `Discord asks to wait 5 s` without `fetch` (`rest.ts:177-179`, `live.ts:582-586`). `asked` stays 2, so a catch that reported status 0 and no headers would still send the third page and fail the test. The first failure intentionally has no limit headers, so the second page is the one that must go out.

**Comment.** The abort sentence sat on the window test, which names one older page per anchor and grows to the 300-row cap (`live.test.ts:779-808`). The replacement matches that test.
