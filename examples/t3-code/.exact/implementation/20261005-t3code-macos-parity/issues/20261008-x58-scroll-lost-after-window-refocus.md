---
name: 20261008-x58-scroll-lost-after-window-refocus
plan: 20261005-t3code-macos-parity
status: closed-not-reproduced
kind: framework-gap (unconfirmed)
blocks: [20261008-pr-list-live-refresh]
upstream_url: null
reproduced_on: not reproduced on main 0365ad1a4 (one-file app)
---

# X58: a wheel-scrolled `scroll` goes back to its top after the window is focused again (macOS, unconfirmed)

## Summary

On the macOS host, the Pull Requests list (a plain `scroll` holding keyed `each` rows, no `scroll-top`
binding, no `scrollFollowEnd`) loses its wheel-scrolled offset and shows its top after the window is
focused again (`page.hasFocus` false → true) and the list re-renders. The same list keeps its offset when
it re-renders for any other reason: a new answer from the server with no focus change, or the app's
minute tick. The expected result is the web's: a scroller keeps its offset across the window losing and
regaining focus and across re-renders that do not change its content's size above the visible rows.

## Why this issue arose

### The T3 Code behavior
`apps/web/src/routes/_chat.pull-requests.tsx` reads the list again on the window's `focus`
(`useLiveRefresh`); the rows stay in place (React keeps the DOM, and the answer is held while the read is
out: `listData = answered ?? carried`), so a reader scrolled down the list stays where they were.

### What exact2 does today
Agent drives on the clone (`scripts/agent.mjs` `open({ host: 'macos', size: [1280, 840] })`, the real-GitHub
lane on 127.0.0.1:16760, 99 rows), the probe row `pr-row-78`'s `layout(...).node.space.viewport.y`:

| Drive | What happened after the wheel (`tap pull-requests-scroll wheel 0 1200`: y 2469 → 1269) | List reads | y after | Kept |
| --- | --- | --- | --- | --- |
| A (investigation, `02c61c110`) | `prefer has-focus false`, `prefer has-focus true` | 1 (the focus read) | 2469 | no |
| B (same session) | another client's `pullRequests.invalidate` (the server's announcement re-reads the list); no focus change | 1 | 1269 | yes |
| C (same session) | `clock +60000` (the app's minute tick re-runs every resource reading the clock); nothing else | 0 | 69 → 69 | yes |
| attempt 4 (`02c61c110`) | two refocuses in quick succession | 1 (0 ms, the server's cache, identical rows) | 2469 | no |
| attempt 3 (`8d462530e`) | a refocus whose read brought one row back (scroll from the agent's `tap` into view, not a wheel) | 2 | top | no |
| attempt 2 (`7d8907f89`, before `windowReturned`) | a refocus with no list read (scroll from the agent's `tap` into view) | 0 | kept | yes |

Ruled out: a blank list in between (every read succeeded; the app now keeps its rows on a failed read
too), re-keying (the scroll and its groups keep their keys; B re-renders the same rows and keeps the
offset), the app's clock move on the window's return (C), an app `scroll-top` binding (none).

### Where it shows in the clone
After the window comes back, a reader scrolled down the Pull Requests list is put back at its top.

## Reproduction (clone)

1. `bun host/apple/build.mjs t3-code-macos --bundle` with `EXACT_APP_DIR=examples/t3-code` on
   `feat(example)/t3-code-pr-list-live-refresh` (`02c61c110` or later); the lane server per
   `tools/github-lane/README.md` (`lane.mjs start primary --port 16760`, `project primary`).
2. Agent: pair with the lane server, `tap open-pull-requests`, wait for the rows, `tap pull-requests-scroll
   wheel 0 1200`, `layout pr-row-78` (viewport y 1269), then `prefer has-focus false`, `prefer has-focus true`,
   `clock settle` a few times, `layout pr-row-78`: y 2469 (the list at its top). Script:
   [drive-investigation.mjs.txt](https://raw.githubusercontent.com/ccheever/exact2/97c10167543b66c18e9d4a496de42084db886a89/pr-list-live-refresh/drive-investigation.mjs.txt); [record](https://raw.githubusercontent.com/ccheever/exact2/28aa135ac8f6fbfd4b41eb5598e24fb65f2dc421/pr-list-live-refresh/record-investigation.txt); [screenshots A, B, C](https://raw.githubusercontent.com/ccheever/exact2/c25fc13f1a41e4035577cee8bd27c860458ded22/pr-list-live-refresh/03-scroll-investigation.png).

Not yet tried: a one-file app (a `scroll` of 100 rows, a resource taking `page.hasFocus`), and the same
drive with the focus change but the list read held off, which would tell whether the focus batch alone or
the focus batch followed by a content batch moves it. Leads in the host: the page-fact batch from
`Session.tellPage()` (`runtime.setPage`), and `ScrollAnchoringMac.swift` (`captureScrollPosition` /
`restoreScrollPosition`: an anchor shift clamped at 0 lands at the top).

## Why it must be resolved (parity impact)
A list read again on focus (the reference's rule, ported in pr-list-live-refresh) puts a reader who had
scrolled down back at the top every time they come back to the window.

## Progress
2026-10-08: drafted from pr-list-live-refresh's investigation session (A, B, C above). Not reproduced on
`main` with a one-file app, not searched upstream, not published.

## Upstream (2026-10-08)

Upstream: not reproduced on main `0365ad1a4`, closed (2026-10-08); not filed. The relevant files (`Session.tellPage`, `ScrollAnchoringMac.swift`) are unchanged on main `e200397ec`. One-file app: `resource page = exactPage()`, and `resource rows = listRows(page.hasFocus)` answering the same 100 keyed rows in a `scroll` of height 400, beside a plain `scroll` of 20 literal rows. Drive: `tap list wheel 0 1200`, `prefer has-focus false`, `prefer has-focus true`, `clock settle`, `layout row-40`. Four variants each kept the offset:
1. A synchronous re-read: `row-40` at viewport y 431 before and after; the plain scroller's row also kept (533).
2. The same read behind a 1.5 s network `fetch`: 431 throughout, read pending included.
3. Two refocuses in quick succession: 431.
4. Rows inside two keyed groups, with a `when pending(rows)` line at the scroller's top: 455 throughout.

What the clone does beyond these (its own carried answer while a read is out, its grouping, its live-refresh rules) remains the lead. A drive on the clone with the list read held off would tell whether a host or the app moves it. This record also travels on PR #265's branch; this copy adds this section and closes it.
