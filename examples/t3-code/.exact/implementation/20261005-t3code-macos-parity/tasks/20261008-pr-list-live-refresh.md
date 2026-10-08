---
name: 20261008-pr-list-live-refresh
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-pr-list-live-refresh
pr_url: null
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
- `app.contract`: the `prList` resource passes `page.visibilityState == "visible", page.hasFocus`
  (the same line, net zero); `pages-sources.ts` hands them on.

## Acceptance

| Row | Result | Proof | Blocker |
| --- | --- | --- | --- |
| Focus reads the list once (not again within 10 s) | pass (unit) | "focusing the window again reads the list once; not again within 10 s; the first opening reads only its own read" | — |
| Visible again reads the list | pass (unit) | same test (`visibilitychange`) | — |
| Coming back to the page reads it | pass (unit) | "coming back to the page reads the list again" | — |
| A 5-minute tick reads it once | pass (unit) | "a 5-minute tick reads the list once" | — |
| No reads while idle past 6 min, or while hidden | pass (unit) | "no reads while the reader has been idle past six minutes, or while the window is hidden" | — |
| Live reads go through the server's cache | pass (unit) | the focus test: no `pullRequests.invalidate` | — |
| #158: close, list re-read, reopen, refocus → the row is back | unit pass; live not reached | unit: "close #158, the list read without it, reopen, an answer that has not seen the reopen yet, then the window focused again: the row is back" (needs #261's `notedListEntry`, merged). Live attempt 1 ([record](https://raw.githubusercontent.com/ccheever/exact2/379571ddbcbb2d21ce4bce0d3780c6c0f790187d/pr-list-live-refresh/record-attempt1.txt), [script](https://raw.githubusercontent.com/ccheever/exact2/7d71c81ef0dab42d46e817923023fbc42e7e1d95/pr-list-live-refresh/drive-attempt1.mjs.txt)): close (GitHub `closed`, bytes equal, toast, row gone), list re-read without it, reopen (GitHub `open`, toast); then the agent driver failed before the refocus | agent driver clock (below); one retry requested |

All new tests fail on the base (the list never reads again) and pass here; the row test also fails
without #261's `notedListEntry`.

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
up to `19714be51` with #261 and #264). Unit tests, the clone checks and the five checks (Checks in
the PR). One live session (approved, no retry) reached the reopen and ended on the agent driver's
clock error before the refocus; #158 was restored (open, both comments deleted, read back).

## Attempts and evidence

| Attempt | Revision | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (live, approved) | `db795f298` | close with comment: GitHub `closed`, bytes equal, "Pull request closed", row gone; list re-read 0.06 s after the close landed, without the row; reopen with comment: GitHub `open`, "Pull request reopened"; the session ended on `clock: the clock cannot go backwards` before the 35 s wait and the refocus | [record](https://raw.githubusercontent.com/ccheever/exact2/379571ddbcbb2d21ce4bce0d3780c6c0f790187d/pr-list-live-refresh/record-attempt1.txt) | a retry (the script now tolerates the clock step) |

## Next action

Review. Owed: one retry of the live row (close #158 → list re-read → reopen → wait out the server's
30 s cache → blur and refocus → row back; then `clock +300000` for one interval read), with the
script that tolerates the agent clock's backwards step.
