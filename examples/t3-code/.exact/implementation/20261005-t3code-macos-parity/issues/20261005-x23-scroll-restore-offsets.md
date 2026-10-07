---
name: 20261005-x23-scroll-restore-offsets
plan: 20261005-t3code-macos-parity
status: closed-upstream
kind: framework-gap
blocks: [20261005-diff-review-engine, 20261005-pr-code-tab, 20261005-pr-handoffs-and-quick-actions, 20261005-round12-wrapup]
upstream_url: https://github.com/ccheever/exact2/issues/138
reproduced_on: null
---

# X23: Scroll restoration by key, scroll padding/margin, animated scrollIntoView, and same-frame scroll offset compensation

## Summary
T3 Code remembers where each thread's transcript was scrolled, lands jumps with animation and a
fixed lead, keeps keyboard-navigated rows clear of a menu's edges with `scroll-padding`, and folds
the pull request header while the body scrolls, adding the lost height back to `scrollTop` in the
same frame. exact2 has scroll events, anchoring and `scrollIntoView(id, block, behavior)`. It has
no restore by key for a top-level list, no scroll padding or margin, no smooth native landing and
no documented same-frame offset write. The clone does the first three in Swift, not the fourth.

Sub-cases, named so a fix and the tickets can cite them: **X23a** restore by key; **X23b**
scroll-padding and scroll-margin; **X23c** animated native `scrollIntoView` with a completion
signal; **X23d** (added 2026-10-05) nested scroll offset read plus same-frame write (the pull
request panel's header fold).

## Why this issue arose

### The T3 Code behavior
- **X23a: per-thread position.** Leaving a thread scrolled up remembers the row at the top edge
  and the offset within it; reopening restores that row instead of the end
  (`apps/web/src/components/chat/timelineScrollAnchoring.ts:164-175` `readTimelinePosition` /
  `rememberTimelinePosition`; `ChatView.tsx:6552`; test `timelineScrollAnchoring.test.tsx:274`). A
  work group's own list restores by key too, with the position tracked on scroll
  (`MessagesTimeline.tsx:3225-3300`: `scrollPositions`, `initialScrollIndex`,
  `restoringPosition`, reconcile once at load).
- **X23b: scroll padding.** `scroll-py-6` on the palette results (`CommandPaletteResults.tsx:150`)
  and the work-group list (`MessagesTimeline.tsx:3366`), `scroll-pb-6` on the composer menu
  (`chat/ComposerCommandMenu.tsx:120`), `scroll-py-1/2` on combobox, command and autocomplete lists
  (`ui/combobox.tsx:231`, `ui/command.tsx:121`, `ui/autocomplete.tsx:200`). They pair with
  `scrollIntoView({ block: "nearest" })` for keyboard highlight (`CommandPaletteResults.tsx:85`).
- **X23c: animated jumps.** Citation and anchor jumps call `scrollToIndex({ animated: true,
  viewPosition: 0, viewOffset: CHAT_LIST_ANCHOR_OFFSET })` and settle on `scrollend` with a 750 ms
  fallback (`ChatView.tsx:6425-6445`). The minimap lands a turn 24 pt under the top edge
  (`MessagesTimeline.tsx:1393`). Settings search uses `scrollIntoView({ behavior: reduced ? "auto"
  : "smooth", block: "center" })` (`settings/settingsLayout.tsx:80-95`). The diff tree reveals a
  file with `align: "start"` (`DiffPanel.tsx:518`).
- **X23d: header fold.** The pull request panel's header collapses to one row once the body
  has scrolled past the header block plus 32 px and reopens at a hard top (offset < 4). The fold
  height change would shift the content, so the panel records the removed height and adds it to
  the scroller's `scrollTop` in a layout effect (`PullRequestDetailPanel.tsx:545-555`
  `compensationRef`/`useLayoutEffect`; `:2656-2680` `onScrollCapture`; the 200 ms grid-row
  transition at `:2317-2335`).

### What exact2 does today
- `EXACT2-GAPS.md` summary row X23: "Scroll restore by key on a top-level list; `scroll-margin`;
  animated native scrollIntoView | Per-thread scroll position, minimap/citation jumps | framework
  feature | `R9Input.swift`, `T3TimelineTurns.swift`". Detail: "Main added `wheel`, ScrollEvent
  extents, scroll anchoring and `scrollIntoView(id, block, behavior)` (`2bfebe63e`, `09fc9b0d4`,
  `cef67560c`, `fbcc4ecb2`). Missing: restore by key for a top-level list (LLP 1070:261), offsets,
  and smooth landing on native hosts." (written from framework source at `c1522fdac`, checked
  against `main` `d2cb661eb`). Layout facts are the separate item X22.
- Bundled library (`20261005-platforms-v3`): not covered; **unknown**. Not reproduced.
- Observed in the clone (code reading, mc-orch tree 2026-10-05): offset **reads** exist
  (`scroll=scrolled` on the thread list, `sidebar.contract:110`, `app.contract:1028
  sidebarScrolled(x, y)`; `scrollLeft=tabsScroll scroll=tabsScrolled`, `r4-surfaces.contract:93`)
  and a bound **write** exists (`scrollTop=(1000000 + jumpRequest)` on the virtualized transcript,
  `app-main.contract:143`, used to jump to the end). Whether a bound write is applied before the
  frame that paints a layout change is unknown (to confirm at `issue-open`).

### Where the clone hits it
- X23a: `modules/apple/R9Input.swift:15-25` implements the reference's rule natively: remember
  the row at the top edge and the offset (session memory, at most 100 threads), restore on
  reopening, cancel on wheel/key/press, commit after momentum. Open failure F1 of
  `20261005-round12-wrapup`: after a real wheel scroll, a thread switch and a return the position
  lands about 64 pt off (2 of 4 trials); the fix is newer than its test run. The nested work-group
  restore has no clone code (grep of the mc-orch tree: no `scrollPositions` equivalent).
- X23b: the palette and menus keep their own scroll state (`palette.contract:352`) and compute
  offsets in TypeScript, not with `scroll-padding`.
- X23c: `modules/apple/T3TimelineTurns.swift:4-9,106-130` places a turn 24 pt under the top
  edge and re-places it for two seconds as rows measure (`hold`); `r5-composer-citation.ts`
  (`citationJump`, inset 2) uses the same op. The user sees a replaced-and-held landing, not the
  reference's smooth animation, and the end state depends on the 2 s hold, not a settle signal.
- X23d: not implemented. `20261005-pr-handoffs-and-quick-actions` carries the fold row. No known
  workaround avoids a one-frame jump.

## Why it must be resolved
Parity means a thread reopens where the user left it, jumps land like the reference, and the pull
request panel does not jump when its header folds. X23a is a known real-input failure (F1) that the
Swift workaround has not closed in two rounds. X23c's hold loop and X23b's TypeScript offsets
duplicate what the host knows. X23d has no workaround. Tickets that wait or carry the difference:
`20261005-round12-wrapup` (F1), `20261005-diff-review-engine` (reveal a file, per-tab scroll),
`20261005-pr-code-tab` (scope jump, scroll per tab), `20261005-pr-handoffs-and-quick-actions` (the
fold row is held until X23d is answered). Keeping the workaround costs an attended real-wheel
session per change in `R9Input.swift`.

## Requested support
The web way where one exists, macOS host first:
- **X23a.** Restore by key for a top-level list: when a list with key K is unmounted and later
  re-mounted, the host restores the first visible row id and the offset within it. The web has no
  equivalent API (history `scrollRestoration` is page-level). Alternative: expose the first visible
  row id with its offset, and "set first visible row", so the app restores it exactly.
- **X23b.** `scroll-padding-*` on containers and `scroll-margin-*` on items, honored by
  `scrollIntoView` and anchored jumps.
- **X23c.** `scrollIntoView({ behavior: "smooth" })` on native hosts, honoring
  `prefers-reduced-motion`, and a `scrollend` event so apps settle on completion.
- **X23d.** For a nested `scroll`: the `scroll` event delivers `scrollTop`; a bound `scrollTop`
  write takes effect in the same frame as the layout change in the same batch, or an
  `overflow-anchor`-style rule keeps content stable when the viewport (not the content) resizes.
  X23a is the largest; X23b–d are small and independent.

## How to reproduce
Mark "to confirm on the pinned `main` at `issue-open`".
- **X23a.** Clone: scroll a long thread up with a real wheel, open another thread, return
  (`AGENT-HANDOFF.md` item 17, F1). Expected: same row, same offset. Actual: about 64 pt off in 2 of
  4 trials (`20261005-round12-wrapup`). Minimal app: two screens with a keyed virtualized list; swap
  and return.
- **X23d.** Minimal app: a header box above a `scroll` of 100 rows. The header height changes
  48→0 when `y > 80`, and the handler writes `scrollTop = y + 48` in the same action. Expected
  (Chrome): a marked row's screen y is equal before and after the fold. Actual: to confirm with
  `screenshot fold.apng over 200 every 20` and `layout`.

## Acceptance for the fix
- AppKit/agent: after a restore, `layout <row>` equals the remembered row and offset within 1 pt
  across 10 real-wheel trials (attended session).
- Conformance case against Chrome for `scroll-padding`/`scroll-margin` with `scrollIntoView
  ({block:"nearest"})` and for `scrollend`.
- X23d: the fold app shows zero shift of the marked row across the fold frame, and the fold row of
  `20261005-pr-handoffs-and-quick-actions` passes its film check.

## App adoption after resolution
Delete the restore, settle-tail and momentum code in `R9Input.swift`, the `hold` loop in
`T3TimelineTurns.swift` and the TypeScript offsets in the palette and menus. Reopen F1 as an
acceptance row of `20261005-round12-wrapup`. Unblock the fold row in
`20261005-pr-handoffs-and-quick-actions`. `issue-close` verifies that the Swift workarounds are
gone and the real-wheel row passes.

## Status and next action
Draft; not reproduced on the pinned `main`; not searched upstream; not published. Next:
`issue-open` (reproduce, search for duplicates, prepare the report for the user's approval;
publication only after approval). X23a–d may be split into separate reports.

## Merged upstream; partly fixed (2026-10-07, adopt-main-fixes-r4)

[#138](https://github.com/ccheever/exact2/issues/138) was closed by main #210 (`b84fb5974`), in the feature
branch since main `463acda68` ([20261007-adopt-main-fixes-r4](../tasks/closed/20261007-adopt-main-fixes-r4.md)):
macOS and iOS anchor a plain `scroll` box as CSS scroll anchoring does, so a box above the port that
changes size or is inserted no longer moves the reader's content (X23d; #138's probe moved −48 pt before).

Not on `463acda68`: X23a (restore by key of a top-level list), X23b (`scroll-padding`, `scroll-margin`:
still unknown attributes), X23c (smooth native `scrollIntoView`, `scrollend`), and the
`overflow-anchor: none` opt-out (main's QUEUE also leaves Linux undriven).

Adoption: the clone had no X23d workaround (the pull request header fold is not built), so nothing is
removed. `R9Input.swift` (X23a) and `T3TimelineTurns.swift`'s hold loop (X23c) stay for the missing parts.
`20261005-pr-handoffs-and-quick-actions` no longer holds its fold row for X23d; when it is built, measure
whether the reference's own `compensationRef` write is still needed on top of anchoring (Chrome anchors
too), against the reference rather than assumed.
