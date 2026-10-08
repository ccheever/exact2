---
name: 20261008-pr-list-live-refresh
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-list-live-refresh
pr_url: https://github.com/ccheever/exact2/pull/265
verified_commit: null
---

# The Pull Requests list re-reads on focus and every 5 minutes

## Outcome

The Pull Requests list reads again when the reader comes back to the page or the window, and every
five minutes while somebody reads it, as the reference page does; it stops while the window is
hidden and once the reader has been idle for six minutes.

## Evidence and scope

Found by pr-writing-and-metadata's live checks of 2026-10-08 (#261,
[task](20261005-pr-writing-and-metadata.md#attempts-and-evidence)): after Close and then Reopen
with comment on #158, the reopened row did not come back to the open list. The server lists through
GitHub search (`is:pr is:open`); its one read after the reopen ran 0.07 s after it and had not seen
it, and nothing read the list again: the clone's list re-read only on server announcements. #247
(pr-conversation-and-refresh) ported `useLiveRefresh` for the detail panel only.

Reference (T3 Code `1e2ecbd975`): `apps/web/src/routes/_chat.pull-requests.tsx` calls
`useLiveRefresh(() => refreshList(true), { enabled: pullRequestsSupported })` ("it reads again on
the way back to the window, and once a minute while somebody is reading it. Those reads go through
the server's cache and stop when the reader stops"); `apps/web/src/hooks/useLiveRefresh.ts`:
`LIVE_REFRESH_MIN_INTERVAL_MS` 10 s, `LIVE_REFRESH_INTERVAL_MS` 5 min, `LIVE_REFRESH_IDLE_AFTER_MS`
6 min; reads on mount (when the view read before), window `focus`, `visibilitychange` to visible,
and the interval while visible and not idle. The server keeps a list answer 30 s
(`PullRequestService.ts` `LIST_CACHE_TTL`).

Scope: the list's live reads only. No UI changes (no before/after images). The 5-minute interval runs
on the app's minute clock tick (`task clock … every(60000, tick)`), as the detail's does.

## What was built

- `pages-pr-refresh.ts`: `liveRefreshDue` (the hook's onArrival/onInterval, moved out of
  `pages-pr-detail.ts` so the detail and the list share it) and `liveRefreshAsked` (whether there is
  anything to ask, without waiting). The detail's `liveRefresh` now calls them; its behaviour is
  unchanged (its tests pass as before).
- `pages-prs.ts`: the page notes its open state, visibility and focus per client (`listLiveAsk`);
  an arrival (the page opened again, the window focused or shown again) or the interval asks
  `liveRefreshDue` for the view `pull-requests-list`, and a due read re-reads the list through the
  server's cache (no `pullRequests.invalidate`). The page's first read notes the view, so arriving
  the first time costs nothing more.
- `app.contract`: the `prList` resource passes `page.visibilityState == "visible"` and `windowReturns`
  (on the same line); `task windowBack when page.hasFocus and page.visibilityState == "visible"` runs
  `windowReturned`, which reads the clock (`elapsed = now()`) and counts the return, so a focus read is
  measured from the moment of the focus, as the reference's `Date.now()` is, not from the last minute tick
  (5 lines; `app.contract` 1,462). `pages-sources.ts` hands them on.
- A due live read stays due until a read lands (`ListLive.due`): a later run of the page (a tick, a
  revision) that overtakes the read's answer reads too, so the newer answer is what shows.
- The detail panel (#247) takes `windowReturns` in place of `page.hasFocus` (`DetailInput.returns`), so
  its focus read also counts from the focus; its 5-minute interval and visibility rules are unchanged.

## Acceptance

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Focus reads the list once (not again within 10 s) | pass (unit) | "focusing the window again reads the list once; not again within 10 s; the first opening reads only its own read" | — |
| The detail's focus read counts from the focus | pass (unit + live) | "a focus read counts from the focus, not from the last minute tick (app.contract windowReturned)" (pages-pr-conversation.test.ts; fails without the fix); live attempt 3: blur and refocus 12 s after the last read, one `pullRequests.detail` read | — |
| An overtaken live read still shows | pass (unit) | "a live read that a later run overtakes still shows: the later run reads too (the five ticks of the live check)" | — |
| Visible again reads the list | pass (unit) | same test (`visibilitychange`) | — |
| Coming back to the page reads it | pass (unit) | "coming back to the page reads the list again" | — |
| A 5-minute tick reads it once | pass (unit) | "a 5-minute tick reads the list once" | — |
| No reads while idle past 6 min, or while hidden | pass (unit) | "no reads while the reader has been idle past six minutes, or while the window is hidden" | — |
| Live reads go through the server's cache | pass (unit) | the focus test: no `pullRequests.invalidate` | — |
| #158: close, list re-read, reopen, refocus → the row is back | pass (unit + live) | unit: "close #158, the list read without it, reopen, an answer that has not seen the reopen yet, then the window focused again: the row is back" (with #261's `notedListEntry`). Live attempt 3 at `8d462530e` ([image](https://raw.githubusercontent.com/ccheever/exact2/ef6e686bb1d44cfbd6515ff07f1de2c47ccbda25/pr-list-live-refresh/01-row-back-after-refocus.png), [record](https://raw.githubusercontent.com/ccheever/exact2/fbf02bab15dc2eab0c537e49d4d1109f019bea71/pr-list-live-refresh/record-attempt3.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/f0a110bca760410c2a549acb16b27e08d0625135/pr-list-live-refresh/drive-attempt3.mjs.txt)): close with comment (GitHub `closed`, bytes equal, toast, row gone), the list read again without it, reopen with comment (GitHub `open`, bytes equal, toast; the two reads right after it still without the row); 35 s on, blur and refocus: the list read again and #158 was back 5.4 s later ("#158 · Count the vowels in a line · just now"); GitHub's open list holds it. Attempts 1 and 2: below | — |

All new tests fail on the base (the list never reads again) and pass here; the row test also fails
without #261's `notedListEntry`, and the overtaken-read test without `ListLive.due`.

Observed in attempt 3, outside this task: when the refocus read brought #158 back, the list scrolled to its
top (it had been scrolled to the Others group with #158 selected); the row is in the list below the fold
([image](https://raw.githubusercontent.com/ccheever/exact2/ef6e686bb1d44cfbd6515ff07f1de2c47ccbda25/pr-list-live-refresh/01-row-back-after-refocus.png)). A removal (the close) kept the scroll. Not investigated here.

The refocus read the list twice (two runs of the page 0.09 s apart, the second while the first read was
out, which reads again until one lands, as the server-announcement path does). A later change could have
the second run wait for the first read instead.

## Framework problem met (not filed)

`scripts/agent.mjs` `clock +N real` on macOS: the loop computes `end = from + N` and steps
`to = min(end, from + elapsed)`; when the host lands an earlier step a hair past it, the last step
asks for `to = end` behind the host's clock and the host refuses: `clock: the clock cannot go
backwards (34850.0 → 34849.999999999985)` (`host/apple/Sources/ExactKit/Agent.swift:521`). Seen once,
about 2 minutes into a session of `clock +250 real` / `clock settle` pairs while pull request RPCs
were in flight. A guard in the driver (`to = max(s.now, …)`) would avoid it. The retry script skips
such a step and settles instead.

## Progress

2026-10-08: implemented on `feat(example)/t3-code-pr-list-live-refresh` (base `b7761f556`, merged
up to `07dcef1ab` with #261, #264 and #263); draft PR #265. Unit tests, the clone checks and the five checks (Checks in
the PR). One live session (approved, no retry) reached the reopen and ended on the agent driver's
clock error before the refocus; #158 was restored (open, both comments deleted, read back).
2026-10-08: attempt 2 (approved retry, `7d8907f89`): the row did not come back. The refocus asked at the
app's last minute tick (no time had passed for the 10 s rule) and the interval's answer, which held #158,
was overtaken by a later tick's run. Fixed (`windowReturned` reads the clock on the window's return; a due
read stays due until one lands) with failing-then-passing tests; #158 restored.
2026-10-08: the detail panel's focus read moved onto `windowReturns` too (test failing without it).
Attempt 3 (approved, no retry, `8d462530e`): the row came back after the refocus, and the detail's refocus
read once. #158 restored (open, both comments deleted, read back with the lane gh).

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (live, approved) | `db795f298` | close with comment: GitHub `closed`, bytes equal, "Pull request closed", row gone; list re-read 0.06 s after the close landed, without the row; reopen with comment: GitHub `open`, "Pull request reopened"; the session ended on `clock: the clock cannot go backwards` before the 35 s wait and the refocus | [record](https://raw.githubusercontent.com/ccheever/exact2/379571ddbcbb2d21ce4bce0d3780c6c0f790187d/pr-list-live-refresh/record-attempt1.txt) | a retry (the script now tolerates the clock step) |
| 2 (live, approved retry) | `7d8907f89` | close, re-read, reopen as attempt 1; 35 s wait; blur/refocus: no list read; `clock +300000`: one list read (its answer held #158) but the row stayed out — the read's answer was overtaken by a later tick's run | [record](https://raw.githubusercontent.com/ccheever/exact2/166a84ecdc41f791fec45a81fe70522fe8a4387b/pr-list-live-refresh/record-attempt2.txt) | fixed after; the live row needs one more session |
| 3 (live, approved) | `8d462530e` | close/re-read/reopen as before; 35 s; blur and refocus: two list reads, #158 back in 5.4 s, one detail read; 12 s later, blur and refocus: one `pullRequests.detail` read, no activity read (unchanged `updatedAt`), one list read | [image](https://raw.githubusercontent.com/ccheever/exact2/ef6e686bb1d44cfbd6515ff07f1de2c47ccbda25/pr-list-live-refresh/01-row-back-after-refocus.png), [record](https://raw.githubusercontent.com/ccheever/exact2/fbf02bab15dc2eab0c537e49d4d1109f019bea71/pr-list-live-refresh/record-attempt3.txt) | — |

## Next action

Review. Follow-ups outside this task: the list's scroll returning to the top when a read brings a row back;
the second list read on a refocus (could wait for the first).
