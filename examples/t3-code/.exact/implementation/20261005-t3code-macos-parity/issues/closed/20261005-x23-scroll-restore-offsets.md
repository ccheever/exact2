---
name: 20261005-x23-scroll-restore-offsets
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261005-diff-review-engine, 20261005-pr-code-tab, 20261005-pr-handoffs-and-quick-actions, 20261005-round12-wrapup]
upstream_url: https://github.com/ccheever/exact2/issues/138
reproduced_on: null
rest_upstream_url: https://github.com/ccheever/exact2/issues/277
---

# X23: Scroll restoration by key, scroll padding/margin, animated scrollIntoView, and same-frame scroll offset compensation

Moved to main `issues/20261009-scroll-padding-settlement-restore.md` (2026-10-09); tracked there.

## Summary
T3 Code remembers where each thread's transcript was scrolled, lands jumps with animation and a
fixed lead, keeps keyboard-navigated rows clear of a menu's edges with `scroll-padding`, and folds
the pull request header while the body scrolls, adding the lost height back to `scrollTop` in the
same frame. exact2 had scroll events, anchoring and `scrollIntoView(id, block, behavior)`, but no
restore by key for a top-level list, no scroll padding or margin, no smooth native landing and
no same-frame offset write. The clone does the first three in Swift.

Sub-cases: **X23a** restore by key; **X23b** scroll-padding and scroll-margin; **X23c** animated
native `scrollIntoView` with a completion signal; **X23d** (added 2026-10-05) nested scroll offset
read plus same-frame write (the pull request panel's header fold).

## Why it arose

### The T3 Code behavior
- **X23a: per-thread position.** Leaving a thread scrolled up remembers the row at the top edge
  and the offset within it; reopening restores that row instead of the end
  (`apps/web/src/components/chat/timelineScrollAnchoring.ts:164-175` `readTimelinePosition` /
  `rememberTimelinePosition`; `ChatView.tsx:6552`; test `timelineScrollAnchoring.test.tsx:274`). A
  work group's own list restores by key too (`MessagesTimeline.tsx:3225-3300`).
- **X23b: scroll padding.** `scroll-py-6` on the palette results (`CommandPaletteResults.tsx:150`)
  and the work-group list (`MessagesTimeline.tsx:3366`), `scroll-pb-6` on the composer menu
  (`chat/ComposerCommandMenu.tsx:120`), `scroll-py-1/2` on combobox, command and autocomplete lists
  (`ui/combobox.tsx:231`, `ui/command.tsx:121`, `ui/autocomplete.tsx:200`), paired with
  `scrollIntoView({ block: "nearest" })` for keyboard highlight (`CommandPaletteResults.tsx:85`).
- **X23c: animated jumps.** Citation and anchor jumps call `scrollToIndex({ animated: true,
  viewPosition: 0, viewOffset: CHAT_LIST_ANCHOR_OFFSET })` and settle on `scrollend` with a 750 ms
  fallback (`ChatView.tsx:6425-6445`). The minimap lands a turn 24 pt under the top edge
  (`MessagesTimeline.tsx:1393`). Settings search uses smooth `scrollIntoView`
  (`settings/settingsLayout.tsx:80-95`). The diff tree reveals a file with `align: "start"` (`DiffPanel.tsx:518`).
- **X23d: header fold.** The pull request panel's header collapses to one row once the body
  has scrolled past the header block plus 32 px, and adds the removed height to the scroller's
  `scrollTop` in a layout effect (`PullRequestDetailPanel.tsx:545-555`, `:2656-2680`, `:2317-2335`).

### Where the clone hits it
Tickets that waited or carried the difference: `20261005-round12-wrapup` (F1),
`20261005-diff-review-engine` (reveal a file, per-tab scroll), `20261005-pr-code-tab` (scope jump,
scroll per tab), `20261005-pr-handoffs-and-quick-actions` (the fold row, held until X23d was answered).

## Clone workaround
- X23a: `modules/apple/R9Input.swift:15-25` implements the reference's rule natively: remember
  the row at the top edge and the offset (session memory, at most 100 threads), restore on
  reopening, cancel on wheel/key/press, commit after momentum. Open failure F1 of
  `20261005-round12-wrapup`: after a real wheel scroll, a thread switch and a return the position
  landed about 64 pt off (2 of 4 trials). The nested work-group restore has no clone code.
- X23b: the palette and menus keep their own scroll state (`palette.contract:352`) and compute
  offsets in TypeScript, not with `scroll-padding`.
- X23c: `modules/apple/T3TimelineTurns.swift:4-9,106-130` places a turn 24 pt under the top
  edge and re-places it for two seconds as rows measure (`hold`); `r5-composer-citation.ts`
  (`citationJump`, inset 2) uses the same op. The user sees a replaced-and-held landing, not the
  reference's smooth animation.
- X23d: no workaround was needed after main #210 (below).
- #277's decision (2026-10-08) made restoration the app's (a different design): at adoption, port
  `rememberTimelinePosition`/`readTimelinePosition` into app state over a first-visible key and
  offset, then retire the restore code in `R9Input.swift`. With X23b–c on main, the hold loop in
  `T3TimelineTurns.swift` and the menus' TypeScript offsets go.

## Evidence and history
- 2026-10-06: filed as [#138](https://github.com/ccheever/exact2/issues/138).
- 2026-10-07: #138 closed by main #210 (`b84fb5974`), in the branch since main `463acda68`
  ([20261007-adopt-main-fixes-r4](../../tasks/closed/20261007-adopt-main-fixes-r4.md)): macOS and iOS
  anchor a plain `scroll` box as CSS scroll anchoring does (X23d; #138's probe moved −48 pt before).
  The clone had no X23d workaround, so nothing was removed; `20261005-pr-handoffs-and-quick-actions`
  no longer held its fold row for X23d.
- 2026-10-08: the rest filed as [#277](https://github.com/ccheever/exact2/issues/277), reproduced on
  main `0365ad1a4`: a top-level list scrolled to 1500 reopens at 0 even with
  `scroll-restoration="auto"`; `scroll-padding` works on `list virtualized=true` only and is refused
  on a plain `scroll` (`lower-scroll-padding`); `scroll-margin-top` and `scrollend` are
  `lower-unknown-attr`; native element-form smooth lands at once (`IntoView.swift:6`).
- 2026-10-08 (adopt-main-fixes-r6, main `e200397ec`): main `20f5aff2d`, `2faf6c190` and `d6ded2e7d`
  give a virtualized `list` `scroll-padding-*`; the reference's scroll-padding sites are plain
  scrollers in the clone, so nothing was adopted. `R9Input.swift` and `T3TimelineTurns.swift` stay
  (the latter also measures which turns are in view, X22).
