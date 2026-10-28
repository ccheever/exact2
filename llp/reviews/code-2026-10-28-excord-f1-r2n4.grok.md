# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), narrow re-check 4, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `7c4a085e20`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** No change: both findings are on main too. Astra's (a same-channel restore can leave away false; main's openChannel also resets away and keeps topKey) and Grok's (settling never ends on a host without scrollend, so away stops updating after a send; F1 there falls back to main's 15-row rule) are raised with the lead as follow-ups.

The 15-row anchor in `scrolled` matches the pre-F1 rule again (`app.contract:603-608`). `leadOlder` is the only 100-row anchor (`app.contract:366-367`). The lead test's one scroll lands 9 rows from the top, so the first page is that 15-row rule; with no further reader scroll, only `leadOlder` asks for the next page (`app.test.contract:127-138`). I did not re-run the mutation.

## (a) Fling from the live end

On a normal fling (`settling` false) the lead starts, and it does not miss the band.

Leaving the end latches `away` and, while `anchor` is still empty, sets `anchor` to `win.pin` in that same `scrolled` (`app.contract:591-598`). For a full tail, `pin` is row 15 (`app.ts:326-327`, `EDGE` is 15), which is not `win.firstId`, and it sits on the edge rather than inside it, so `win.need` stays empty (`app.ts:319`). The reader of a tail shorter than 100 rows is already inside the band. `leadOlder` then sees `away`, `keys.indexOf(topKey) < 100`, and `anchor != win.firstId`, and runs `after(1, older)` (`app.contract:366-367`, `623-625`).

That is one timer tick later than the old in-handler write, which assigned `anchor = win.firstId` inside the scroll event as soon as `at < 100`. A gated `after` arms when the gate becomes true and restarts only when its key changes or the gate drops. Further fling events only change `topKey`; `win.firstId` stays put, so the tick is not reset. One millisecond is inside the same frame. Once the index is in `0..99` it stays there until a prepend, so the tick cannot fall through the band.

If that same event is already at `at < 15`, `scrolled` assigns `win.firstId` itself (`app.contract:604-605`) and the first page does not wait for the task.

**1. Bug.** After a send from history, a host that never delivers `scrollend` never gets this path again.

`post` clears `away`, clears `anchor`, and sets `settling` (`app.contract:806-810`). `settling` is cleared only in `scrollEnded` (`app.contract:618-622`). The file's own comment is that Windows sends no `scrollend`, and a list replaced mid-scroll drops its own (`app.contract:107-108`). The 200 ms quiet timer only restores hover (`app.contract:346-347`, `1111-1113`).

While `settling` is true, `scrolled` will not update `away` (`app.contract:591`). From the live end, `anchor` is `""` and the tail still reaches the newest row, so `win.hasAfter` is false. Both halves of `leadOlder`'s gate stay shut: `away` is false, and `(anchor != "" and win.hasAfter)` is false. A later fling can still anchor through the 15-row rule when `at < 15`, which does not read `settling`. It cannot arm the 100-row lead. On a tail of up to 300 rows, a 60–90 row fling never reaches that 15-row band, so no older page is asked for. `settling` never clears, so every later fling from the live end has the same hole. Sending from the live end does not set `settling`; sending after reading back does.

## (b) Stale `topKey` on restore

Clean. Reopening a saved place still goes through the offset placement, and every path that skips that placement leaves the lead shut.

`openChannel` no longer clears `topKey`. A place that is not at the end sets `target`, `targetBlock = "offset"`, and the saved anchor (`app.contract:484-488`). `land` then runs `landJump`, and the offset branch sets `leadHeld` (`app.contract:679-682`). Until that landing, `target != ""` keeps `leadOlder` idle. After it, `not leadHeld` does. A carried `topKey` that is still the saved row is in the new window only as that placement's row, and the hold covers it.

Paths that skip the offset branch:

- The saved key is not in the window. `landJump` clears `target` and does not set `leadHeld` (`app.contract:686-691`). That key is the only same-channel id that can be carried across a channel that never reported a scroll, and it is absent, so `keys.indexOf(topKey)` is `-1` (`app.contract:366`). A `topKey` written by the other channel's own scroll is that channel's id, also absent.
- The open is at the end (`app.contract:489-491`). There is no `landJump`. A same-channel `topKey` can sit in the new tail, but this path sets `away = false` and `anchor = ""` (`app.contract:475`, `490`), so the lead gate stays shut. `historyStart` is cleared in the same action (`app.contract:476`), so `afterPage` will not restore from that key either (`app.contract:427-430`).
- A slow prepend at the top sets an offset target (`app.contract:427-430`) and lands on the same branch that sets `leadHeld`.

A message key is that message's id, or the sender's nonce (`shapes.contract:146-159`). Another channel's row is not in this window.

## (c) `leadHeld` sticking

It can stick, and that is acceptable.

It is set only when an offset placement lands (`app.contract:682`) and cleared only at the end of every `scrolled`, with no other condition (`app.contract:612`). A placement that never reports leaves it true, so the no-scroll chain waits. That is the same wait as clearing `topKey` until a report, and `topKey` itself is kept for the prepend restore.

The next scroll clears it in that same `scrolled`, after the away and 15-row updates, so the task sees the clear. A later fling is a scroll: the hold does not outlive it, and that scroll can still apply the 15-row rule (`app.contract:604-612`). `scrollIntoView` is a command the host applies after the action, so a report that does arrive is a later event and clears the flag rather than landing before it is set.

## (d) Anything else

Nothing else in this diff. The lead test's assertions are unchanged and match the continuation above; the new comment matches the 15-row first page plus `leadOlder` after it.
