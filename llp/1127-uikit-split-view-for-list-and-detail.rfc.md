# LLP 1127: A list and its detail on iOS are UIKit's split view

**Type:** RFC (a decision brief: each decision carries a proposed answer; nothing here is built until Charlie has read it)
**Status:** Draft r8, 2026-10-17. Not in `llp/current/` (Q4).
**Systems:** the iOS host's navigation; the runner (the detail selection, `inactive`, point reads); the C ABI (`navigate`'s source) and the native module surface (`ExactTab.controller`); the agent; `apps/duo-lab` and `smoke duo`; docs. Contract: nothing new
**Author:** Claude (Opus 5.5) for ide
**Implementer:** to be assigned on acceptance
**Date:** 2026-10-16
**Revised:** 2026-10-17 (r8)
**Related:**
- **Exact LLPs.** 1116.004 D1.2 (the shared rule; amended here); 1115.002 D4 (the Mac's split); 1038 (the router; D12, one value, two presentations); 1116.006 D2 (the empty detail) and D4 (`back`); 1115 D5 and 1115.000 D5 (Back, and the bar of a route without a header); 1075.003 §3.6, §3.7, §3.9 (the tab container hatch, one bar per stack, the kept surface); 1078 (`device-posture` and viewport segments; `apps/duo-lab`; `smoke duo`); 1123 (content owners and local clearance; D6, owner-local segments, deferred).
- **Apple.** [HIG: Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo); [HIG: Split views](https://developer.apple.com/design/human-interface-guidelines/split-views); [Preparing your app for iPhone Duo](https://developer.apple.com/documentation/technologyoverviews/preparing-your-app-for-iphone-duo); [`UISplitViewController`](https://developer.apple.com/documentation/uikit/uisplitviewcontroller), its delegate, [`show(_:)`](https://developer.apple.com/documentation/uikit/uisplitviewcontroller/show(_:)), [`Column.compact`](https://developer.apple.com/documentation/uikit/uisplitviewcontroller/column/compact); [`transitionCoordinator`](https://developer.apple.com/documentation/uikit/uiviewcontroller/transitioncoordinator); Tech Talk [Strike a pose with adaptive layouts on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111463/). iPhone Duo material is iOS 27.1 SDK and simulator evidence (Xcode 27.1 RC), not hardware.
- **Evidence.** §1 states every observation this document relies on. The raw record (`MEASUREMENTS.md`, the probe app, its UI tests, logs and captures) is on the author's machine, in `~/.config/tuft/sessions/1791430083.031489/notes/duo/split/`; ask ide for it.

## Summary

A stack whose root route is a list whose rows push a detail shows the two side by side on the web at 900 px and up and on the Mac at every width (LLP 1116.004 D1.2, LLP 1115.002 D4). On iOS it pushes at every width: the iOS host has no `UISplitViewController`. On the iPhone Duo that is a tell. Apple's guidance is that "a split view expands on the inner display and collapses to a single pane on the outer display", and that "when built with standard components, split views adapt to reserved regions automatically, adjusting width and margins to adapt to the fold" (HIG, Designing for iPhone Duo).

This document projects the stacks the shared rule splits as a two-column `UISplitViewController`, with nothing new for the author to write. UIKit decides how many columns show, their widths and where the bars stand, and on the upright inner panel it keeps both columns clear of the fold. The router decides what each column holds and which is on top when UIKit collapses the split. On the tall inner panel folded, the detail spans the hinge, which stays content's to handle (D9).

## 1. What a hand-built UIKit split does on the Duo

These are observations of a plain UIKit app on the iPhone Duo simulator, iOS 27.1, at the default text size. The app is a `.doubleColumn` split: a table list, and a detail with two bar items; each column's bar prefers large titles. Its columns' navigation controllers were first the ones UIKit makes, then ones the app makes, as Exact would (N0 for the list, N1 for the detail), with the same geometry. Back buttons and edge drags were real touches from XCUITest. Frames are in points; UIKit reports the 130° pose as 127.8°.

| Pose | Window, safe area | The split | Content |
|---|---|---|---|
| Open 180°, upright | 951 × 669; right 84, bottom 34 | both columns (`oneBesideSecondary`); the list 0–320 | the detail's view spans 0–951, its safe left 320; detail text 340–867 |
| 130° and 90°, upright | the same; a division 455.5–495.5 (margins 20 and 20) | both columns; the list 0–475.7, the fold's centre | the list's cells reach 475.7, their content 455.7; the detail's safe left 495.7 (475.7 and the margin), text from 515.7 |
| Closed (the cover) | 466 × 678, safe right 84; turned, 678 × 466, safe left 84 | collapsed | the detail over the list, Back in the side bar |
| Open 180°, turned | 669 × 951; top 82, bottom 34 | the detail alone (`secondaryOnly`), the list behind UIKit's sidebar button, or over the detail on request (`oneOverSecondary`) | detail text 20–649 |
| 130° and 90°, turned | the same; the division across the panel | the detail alone | the detail runs across the fold |

1. **Bars.** Upright and open, the list's bar runs along its column's top and the detail's items stand in the vertical bar, as Apple says ("the system shows bars horizontally for the sidebar or content view, and vertically for the detail view", Preparing your app for iPhone Duo). Turned, every bar runs along the top.
2. **Collapse nests N1 in N0.** UIKit pushed N1 onto N0 (`pushViewController`, unanimated), hid N1's bar and showed N1's top in N0's bar; expansion took N1 back with `popToViewController`. These changes had no transition coordinator, and N0's delegate heard `willShow`/`didShow` of the list during expansion.
3. **A person's pops.** Collapsed with [Note 1, Note 40] in N1, the Back button and an edge drag popped Note 40 within N1, then N1 off N0 (`from` N1, `to` the list). A short held drag was cancelled, with the stacks unchanged. Each pop had a coordinator. With pop gates that allowed any pop with something beneath, each drag asked N0's recognizer, then N1's: N1's drove the pop within N1, and N0's the pop of N1 once N1 was one deep. A programmatic push of N1 onto N0 reached both delegates, each with a coordinator from the list to N1.
4. **The collapse proposal is always the detail,** even with nothing selected (the cover then showed the empty detail over the list, with Back) and after a Back on the cover (the next opening showed the detail the person had left).
5. **Showing and hiding.** With the list over the detail, setting the detail alone left the list there; `show(.secondary)` closed it (the SDK header says it is ignored for the secondary column when expanded), and was not remembered; `hide(.primary)` closed it and was remembered across rotation and relaunch until a reinstall; `showDetailViewController` replaced the app's N1 with a navigation controller UIKit made.
6. **Sheets** presented from the list or the detail stayed up through a fold; the split collapsed and expanded under them.
7. **Selection.** A table with UIKit's default `clearsSelectionOnViewWillAppear` lost its selected row when the split expanded again.
8. **A hidden detail bar** on the tall panel left no button in the accessibility tree; a drag from the leading edge showed the list.
9. **Widths.** A preferred width of 280 held open and gave way to 475.7 folded; the person's drag on the divider set 390, which UIKit kept across relaunch.
10. **Containers.** At the settled poses compared (open and 130° upright, closed, 130° and closed turned), the geometry was the same with the split as a child of a plain controller, as Exact holds its navigation; in a `UITabBarController` the tab bar stood in the side strip when wide and on the cover, and along the bottom when turned.

## 2. Decisions

### D1 — Nothing new to write

The Contract is the one that splits on the web and the Mac: a stack whose root route holds a `list` whose rows go down the page, one of whose rows pushes, as `Runner::stack_split` reads it. `navigationSplit` keeps its meanings (`"none"`; a width, which also splits a stack with no list; `"placeholder"` on a box). On iOS a width is `preferredPrimaryColumnWidth`, which the fold and the person's divider override (§1.9); unsaid, UIKit's own (320 on the Duo's wide panel). LLP 1116.004 D1.2's "a phone keeps the pushed stack" becomes "a compact width keeps the pushed stack".

### D2 — One split, collapsed while compact

Where the iOS host installs a stack's navigation controller today (`NavigationHost.installPrimary`, `NavigationTabsIOS.installTabs`), a stack the rule splits gets a `UISplitViewController(style: .doubleColumn)` whose columns are navigation controllers Exact makes (`makeNavigation`): N0 holds the list, N1 the detail. It is installed at every size, so opening, closing and turning the Duo are UIKit's collapse and expansion of the same controllers. Exact sets no display mode, split behavior, gesture or button visibility. The rule is asked after every batch; a stack that starts or stops splitting changes container between batches (D10). Under `--chrome agent` the stack pushes as before. tvOS keeps its stack. Q1 weighs the alternatives.

### D3 — Which column holds which route

Let P = [root, r1 … rk] be the stack's **nonmodal prefix**, its routes before the first modal route (`NavigationRules.segments`); modal routes are presented over the split as today.

**A detail selection** is the one event this split acts on. The runner reports it with a batch, naming the selected entry, when:
- a row of the list was activated (a press, a link's default navigation, an accessibility activation, the agent's tap);
- the activation's dispatch wrote the router without a refusal, whether or not the value changed (`go` to the shown location changes nothing);
- and the selected entry is then P's last entry, with P holding a route above its root.

P is found as `NavigationRules.segments` finds the first presentation segment: the routes before the first `modal` or `fullscreen` route. A push after a presentation belongs to that presentation's stack, not to P.

Anything else is no selection:
- an activation that writes no router, or whose write is refused;
- one that ends inside a presentation: a row that opens a sheet, or a sheet and a push within it;
- a dialog or a popover, which writes no router.

Routes that such an activation added to P are still placed by the table below and by D6, with no selection. The Mac's attribution likewise requires the detail to be the selected top below the first sheet (`SplitMac.read`).

Today a link with no `press` handler reaches the runner as the navigation root's `navigate` event, with a location only (`PresenterIOS.press` → `activateLink` → `Session.navigate`). That event gains the activating node's id, so the runner knows the row. A warm incoming link carries none.

**The association.** For each entry of P the runner keeps at most one row, the entry's association. It names the row by its plan site and its instance path within the list (`Tree.site`: each region crossed, with its row key or active arm). A static row is thus named, and rows of nested repeated regions are told apart. A row is in the list while its item is, mounted or not, so a row that a windowed list has scrolled out stays named. The association is set:
- by a detail selection, to the activated row;
- by a current location pair (a mounted row whose link names the entry's location), when the entry has no association, when its row is no longer in the list, or when its row was set by a pair and now links elsewhere (a refreshed list).

A row set by a selection is not displaced by another row's pair while it stays in the list, so a row whose press reselects the shown detail keeps it beside a link row that names the same location. Nothing else changes an association: not its row unmounting, and not a change no selection makes (a replace inside the detail, an `open` that keeps the entry; `replace` keeps the entry's id). It ends only when the entry leaves P. This follows the Mac's `rowOf`, which the pairs and the table's press set and which is pruned only when the route goes (`SplitMac.read`, `SplitMac.present`); there, a current pair replaces any association. `StackSplit.rows` reports the associations.

**A row's detail** is an entry of P with an association. **The base** b is the highest i in 1 … k whose r_i is a row's detail, else 1: the Mac's rule.

| P | N0 | N1 | Collapsed | Collapse answer | The list's selected row, expanded |
|---|---|---|---|---|---|
| [root] | root | the empty detail (D7) | [root] | primary | none |
| [root, r1 … rk] | root | [r_b … r_k] | [root, r_b … r_k] | secondary | r_b's associated row, whether mounted or not; none when r_b has no association |
| either, with modal routes above P | as P has it | as P has it | as P has it | as P has it | as P has it; a row that opened the sheet is no selection |

- Routes r1 … r_(b−1) are in neither column. They stay mounted, as a retained route that has left its native stack does (LLP 1123 D2).
- Each split answers its collapse from its own stack, the selected tab's or a retained one's. A sheet over the list collapses to the list, a sheet over a detail to the detail; a presentation pending or being dismissed does not count.
- The highlight follows the last column: it changes whenever the row that column names changes, is shown again when that row mounts (a windowed list, as the Mac retries), and is restored when the split expands (UIKit's default cleared it, §1.7). `RowCellsIOS.ran`, which lets a row go when its screen is on top, does not apply to a list beside its detail.

In the list-detail pattern, whose rows `go` to the first item and `replace` it with the next, each selection is the base and its row is highlighted. A deep link whose route a mounted row links to is the base from its first batch, and stays the base while that row scrolls out of the window and back. A detail that no row is associated with (a deep link to an unlisted item, a push from the list's header) is the base only when nothing below it in P is a row's detail. A link inside a detail to another row's detail makes that route the base, as on the Mac.

### D4 — One Back, and clearing the selection

- **One Back** is a completed pop within N1, from a route above the base: UIKit's Back button, an edge drag or the agent's `back`. It dispatches once through today's order: the route's authored control, else the root's `navigate` with the location beneath, else the runner's fallback (`NavigationHost.goBack`).
- **Clearing the selection** is a completed pop of N1 off N0 (only the base can leave so, D5), or the agent's `back` from the base of an expanded split, which shows no Back. It removes r_b … r1, one route per step, each by today's Back for the route then on top:
  - after each step's batch, it continues only if the stack is the previous one less its top entry;
  - it stops on anything else: the stack unchanged (a disabled control's refusal included), a presentation added (a confirmation), another entry on top (a redirect or a replace), another tab;
  - it stops after b steps at most;
  - if it stops after UIKit has already popped N1 off N0, N1 shows where the router stopped and is pushed again (D6).

  On the Mac the same is taking the selection off the row (`SplitMac.closeDetail`).
- **An authored Back control** does what its action does: one `back(nav)` from the base shows r_(b−1) if there is one, else the root.
- **A cancelled pop** dispatches nothing and returns the editor it put away, as today.

### D5 — Following two navigation controllers

1. **Contents.** Each route's `RouteController` is in at most one of N0 and N1. Exact writes their stacks from the router only; UIKit alone nests N1 in N0 and takes it back.
2. **Both followed.** For a split stack the host follows N0 and N1 where it follows one navigation controller today: their pop recognizers, their delegate callbacks, their transitions.
3. **Recognizers.** Exact's gate (`popMayBegin`) answers both, each from the selected route's Back as today:
   - N1's begins when N1 holds more than one route and its top route's Back is available;
   - N0's begins only while N1 is nested in it and holds just the base, and the base's Back is available. With a route above the base, N0's refuses, so a refused Back on that route is never bypassed by a drag that would remove the whole detail.
4. **A transition is its coordinator.** A transition is identified by its coordinator object from its first callback to its end, and handled once, whichever delegates report it; a controller can return an ancestor's coordinator, and §1.3 saw one push reach both delegates. Its `from` and `to` name the native operation:
   - from a route in N1 to the route beneath it: a pop within the detail;
   - from N1 to the list: the base leaving, whose source route, for the drag's snapshot and the editor it puts away, is N1's top;
   - anything that brings a newer controller in: a push.
5. **Whether a Back is owed** stays today's decision (`NavigationRules.dispatchesBack`, with its exception for a source replaced during a drag): a pop that Exact animated because the router had already moved (`setViewControllers`) owes nothing. A native pop within the detail owes one Back; the base leaving owes the clearing (D4); a push owes nothing.
6. **No coordinator, no Back.** UIKit's collapse and expansion reached N0 and N1 only as changes without a coordinator (§1.2); they never dispatch. Exact's own unanimated changes, a context menu's commit included, never did and do not.
7. **Ends.** A completed pop dispatches what item 5 says once; a cancelled one, nothing; a pop that a fold interrupts is judged by its coordinator's end.
8. **Settlement.** A split stack is in transition while a coordinator of N0, N1 or the split itself runs (the split's follows `show(_:)`, per the SDK header), and from UIKit's collapse or expansion question to its `didCollapse` or `didExpand`: focus waits, and `clock settle` waits (`nativeInFlight`).

### D6 — When Exact calls `show(_:)`

The columns follow P on every batch (D3), a presentation above P or not. `show(_:)` is called only for these:
- **A detail selection (D3):** after its batch, `show(.secondary)`, whether or not the router changed. Tapping the shown row, or a row whose `go` pops back to its detail, closes a list overlaid on the detail; collapsed, it pushes N1 when N1 is not on top and does nothing when it is (SDK header).
- **A new base that a router change made without a selection, or restoration after a stopped clearing** (D4, whether or not the base changed): N1 is set, then `show(.secondary)`. The new base may come from a deep link, an action elsewhere, or a route an activation added to P beneath the sheet it opened; in that last case the detail the router holds is the one under the sheet when it is dismissed, as on a phone today.
- **P back to its root:** N1 is set to the empty detail, then `show(.primary)`: collapsed, UIKit pops N1; with the detail alone, the list comes over it.
- **Anything else:** no call. That includes:
  - an activation that is no selection and leaves P as it was (a row's unrelated action, a refused destination, a row that opens a sheet over the list or over the shown detail);
  - a base that changes with the router unchanged, as when a row mounts and its link names a deep-linked entry: N1 follows the new base, and the person's view of the columns stays as it was;
  - a push or pop within the detail, and every other batch.

Exact calls neither `hide(_:)`, which UIKit remembers (§1.5), nor `showDetailViewController`, which replaces N1. `show(.secondary)` may close a list the person opened over the detail; that is UIKit's, and not remembered.

### D7 — Bars, titles and the empty detail

- **Bars.** Each column's bar is its top route's, projected from the route's header by today's rules; LLP 1075.003 §3.7's one bar per stack becomes one per column. Collapsed, N0's bar shows N1's top route's items. Where items stand is UIKit's: an item with a title and no symbol, or a custom view, stays out of the vertical bar (Preparing your app for iPhone Duo). Titles are UIKit's rendering of the heading's level.
- **A route in N1 counts as pushed** for the bar rule (LLP 1115.000 D5.2), the base included, though it is N1's first route: a route with no bar-shaped header still gets UIKit's bar, which holds Back when collapsed and UIKit's sidebar button when the list is hidden. Today that rule reads "pushed" from the route's place in its navigation controller (`makeNavigation`, `NavigationRules`), which would give N1's base no bar. Collapsed, the route on top decides N0's bar, under its own column's opt-outs.
- **The empty detail** keeps N1's bar too, with no title and no items. It is not a Contract route. Its content is `UIContentUnavailableConfiguration.empty()` saying "No Selection", in the strings' language as LLP 1116.006 D2 asks, which is English until the iOS host has a strings table for its own words, as on the web. The author's `navigationSplit="placeholder"` box takes its place in stage 3 (§4), with the Mac's.
- **The author's opt-outs stand, as today** (`bar(of:pushed:)`, `barShows`). A detail route has no UIKit bar when:
  - it says `navigationBar="none"` (LLP 1115.000 D5.2);
  - its first child is a shown `header` that says `appearance="none"`, which keeps that header authored, pushed or not (`HeaderRule.keepsAuthoredHeader`; LLP 1075.003 §9.10; LLP 1115.000 D5.2 as amended 2026-10-12);
  - the `navigation` hatch set its column's `showsBar` to false (LLP 1075.003 §3.7); the hatch receives N0 and N1 separately (D10).

  With the detail alone, such a route leaves no UIKit sidebar button (§1.8). The list is then reached by UIKit's leading-edge drag, or by a Back control the author drew: each press goes back one route (D4), and at the root D6 shows the list. A route with neither leaves the drag alone, as a hand-built split with a hidden detail bar does. An app that hides a column's bar through the hatch owns that column's chrome.

### D8 — One answer to what is visible

After every layout and every split transition, whether a batch caused it or the person did (the sidebar button, a drag, a tap beside the overlay, a fold), the host derives one visibility for each route of the split stack. The host's checks use it (`isInactiveRoute`, which menus' `eligible`, `flushPendingFocus` and the agent's checks ask), and so does the runner (`inactive`, `elementFromPoint`, the agent's aim), told with the owners' publication (LLP 1123 D2, `exact_host_covers`); the wire form is the implementer's.

| UIKit shows | Live: input, focus, menus, the agent |
|---|---|
| both columns | the root route; N1's top |
| the list over the detail | the root route |
| the detail alone | N1's top |
| collapsed | the nested stack's top |

Every route of the stack not live is covered, r1 … r_(b−1) always. A modal presentation and an unselected tab cover what is under them first, as today. The host reads the columns' actual bounds and safe areas: where both columns show, a point in the list's column reaches the list, though the detail's view may lie under it (§1) and document order puts the detail's routes later.

### D9 — Clearance, and how far the fold is handled

Each route in a column is a content owner laid out in its controller's view bounds (LLP 1123 D2, stage 1). Its local clearance (LLP 1123 D3) is the column's safe area: for the detail, the list beside it (320 open; 495.7 folded, the list's 475.7 and the fold's margin) and the vertical bar; for the list, the fold's 20-point margin. This needs LLP 1123 stage 2: until then a route's side edges are covered only while the root does not say `viewport-fit="cover"` (`NavigationBarIOS.routeEdges`), so under `cover` a detail would lie under the list (Q3).

That qualifies the upright inner panel, open and folded. On the tall panel folded the detail spans the hinge (§1), and edge clearance cannot protect an interior fold (LLP 1123 D6). Content that must keep clear of it can read the document's segments (LLP 1078), which are the window's and describe the column only where the column is the window, the detail alone. Owner-local segments stay deferred, with this as their consumer. The document's facts do not change while a split shows (LLP 1123 D1).

### D10 — Tabs, and the app's tab container

A tab whose stack splits holds the split in the tab's place. The router's selected route governs the shared tab bar: the authored tablist's `display` hides or shows it through the tab controller's `setTabBarHidden`, collapsed or expanded, and no route inside a split sets `hidesBottomBarWhenPushed`, which would follow UIKit's nesting rather than the router (`NavigationTitleIOS.followTablist`).

`ExactTab.controller`, public as a `UINavigationController` (`ExactNativeModule.swift`), becomes a `UIViewController`: a navigation controller or a split view controller. The module ABI's major goes from 3 to 4, so a module built against 3, which reads each tab's pointer as a navigation controller, is refused by name at load ("module ABI 3, host ABI 4"), as LLP 1075.003 §3.9 requires; the table's layout does not change. `apps/native-fixture` changes in the same commit, which says what to change. When a tab's stack starts or stops splitting, Exact retires the app's container and asks `tabContainer` again, as when the tabs change (`reshape`). The `navigation` hatch receives N0 and N1 as it receives any stack's navigation controller.

### D11 — The agent and the smoke

The split shows under `--chrome platform` (the default of an app's `exact.mjs agent ios` and `test ios`); under `--chrome agent` the stack pushes, so the new path is qualified under platform chrome only. `state window.split` reports UIKit's state (`collapsed`, `displayMode`, the list's width), D8's visibility per route, each column's routes and the controller hosting them now, and `base`, `row` and `empty` as on the Mac. `tree`, `tap` and `type` follow D8; `back` is D4's.

### D12 — Duo Lab keeps Fold and gains a split screen

Fold stays: it is LLP 1078's consumer of authored segments, and its two-down panes are the only device check of vertical segments (`smoke duo`'s rotate leg; `contract/corpus/segments.contract` branches only on two horizontal segments). A fifth tab (`tab split "/split"`, a child `item "/split/:id"`, rows that are links, a header on each route) is the split, and `smoke duo` checks it (§3).

### D13 — Other hosts

The web splits at 900 px as today, and does not follow a foldable browser's segments. The Mac keeps LLP 1115.002 D4 and can take the runner's detail selection in place of its `pressing` flag. tvOS, Android, Linux and Windows are unchanged.

## 3. Tests and qualification

**UIKit XCTests,** with Exact's N0 and N1:
- each row of D3's table;
- a detail selection by a link row, by a press row, by `replace` from another row (the list-detail pattern), and by reselecting the shown row with an action row's `go`, which highlights that row, also when a link row names the same location; no selection from a row's unrelated action or a refused destination;
- the association across a replace inside the detail, a warm `open` that keeps the entry, a selected windowed row unmounted and mounted, and a refreshed list whose row for the entry is replaced by a new one;
- a URL-only arrival with b > 1 (an `open` to [root, A, B], B's row mounted, no activation): b = 2; B's row scrolled out of the window and back: b stays 2, N1 stays [B], no `show`, and B's row highlighted again when it mounts;
- an explicit width with no list; a link inside a detail to another row's detail; a push from the list's header;
- the collapse answer with a sheet over the list, over a detail, pending and being dismissed, and in a retained tab;
- clearing over [root, A, B] with B's Back refused, opening a confirmation, and redirecting;
- clearing over [root, A] collapsed, with A's Back leaving the router unchanged at the first step: N1 restored by `show(.secondary)`;
- a fold while a detail shows dispatches nothing;
- the button and a drag dispatch once, within N1 and off N0; a cancelled drag of each kind keeps history and the editor;
- a `replace` during a drag; an authored Back that pops by `setViewControllers` dispatches nothing more; a context menu's commit;
- a drag over [base, child] where the child's Back is disabled and the base's is not: neither recognizer begins;
- a headerless detail base keeps UIKit's bar, Back and sidebar button;
- `navigationBar="none"`, a header that says `appearance="none"`, and N1's `showsBar` set false by the hatch each remove that bar, and the list is still reached by the leading drag and by an authored Back control;
- a row that opens a sheet, from [root] and from [root, A]: no selection, no `show`, the highlight unchanged;
- a row whose `open` yields [root, A, M], M a sheet: no selection; N1 set to [A] and `show(.secondary)` beneath the sheet, to be measured on the Duo before it is built; A highlighted only if a row is associated with it;
- a row whose action yields [root, M, C], M `fullscreen` and C pushed within it: no selection, P unchanged;
- D6's calls, and none on other batches; the highlight across expansion and a windowed remount;
- `navigationSplit="none"`; an authored width; a split in a tab; a module built against ABI 3 refused; the `tabContainer` hatch with a split tab and with a tab that starts splitting.

**The runner:** the detail selection and the associations (D3), the `navigate` event's source; `inactive` and `elementFromPoint` in each row of D8's table, with publication after a change no batch caused.

**`smoke duo`,** platform chrome, Duo Lab's split screen at 180°, 130°, 90° and 0°, upright and turned: both columns open and folded upright; folded, the list's edge at the centre of the division `layout.env` reports and each column's content clear of it; collapsed with the router's top shown; the detail alone turned; the selection and its highlight across every pose; a Back on the cover then opening shows the empty detail; a push inside the detail kept across a close and an open; menus and typing in either column; the tab bar where §1 saw it. Fold's checks unchanged.

**Bars:** a title-only item, a symbol with a title, a readout, header search and an overflow, each where UIKit puts it and reachable.

**With LLP 1123 stage 2:** a row's ground reaches its column's edge while its content stops at the clearance (475.7 and 455.7 folded).

**Qualification before the split is on by default:** a list and a detail with `navigationBar="none"`; relaunch with a remembered hidden list and column width; a regular-width landscape phone; VoiceOver traversal and focus restoration across the columns; a hardware keyboard in either column; the largest Dynamic Type; a right-to-left language; an iPad resized through compact and intermediate widths, in Stage Manager and in two windows; the cost on a compact phone (controllers at boot; drafts and scroll across a fold); eligibility changing while running, including rows that only open sheets (`QUEUE.md`, "The Mac's split, what is owed", item 1).

## 4. Order of work

1. LLP 1123 stage 2 (Q3).
2. The runner's detail selection, the associations and the `navigate` event's source; the split on iOS (D2–D8, D10, D11), with their tests; the list-detail pattern and `apps/shelf` under platform chrome on an iPhone and an iPad simulator; the qualification list.
3. The author's placeholder on iOS and the Mac together: the lowerer's `display: none` and a containing block in the detail's column.
4. Duo Lab's split screen and `smoke duo` (D12); the docs.

## 5. Amendments on acceptance

- **LLP 1116.004 D1.2:** "a phone keeps the pushed stack" becomes "a compact width keeps the pushed stack; on iOS the split is UIKit's (LLP 1127)".
- **LLP 1038 D12**'s 2026-10-11 amendment: the queued presenters are LLP 1115.002 D4 and this document.
- **LLP 1115.002 D4:** "the web and the iPad keep their width" becomes "the web keeps its width; on iOS UIKit decides (LLP 1127)".
- **LLP 1115.000 D5.2:** a route in a split's detail column counts as pushed for its bar.
- **LLP 1075.003:** §3.6 and §3.9, `ExactTab.controller` is a `UIViewController` and the module ABI major is 4; §3.7, a split's bars are one per column.
- **LLP 1078 D8:** Duo Lab's fifth screen.
- **`host/apple/include/exact.h`:** the `navigate` event (kind 14) carries its activating node beside its location, a changed input form, so `EXACT_ABI_VERSION` goes from 15 to 16 and enters the bake's compatibility identity as every C ABI change does (`bake/src/compat.rs`). This is the host's C ABI, apart from the native module ABI that D10 moves from 3 to 4.
- **`kernel/tables/schema.json`:** `navigationSplit`'s comment: unsaid, the list's width is 320 on the web and the Mac and UIKit's on iOS.
- **`docs/recipes/navigation-stack.md`, `docs/contract-for-agents.md`:** where a list and its detail split on iOS.
- **`QUEUE.md`,** once built: the iPad's split in "A wide window's sidebar and split on iPad and the Mac" and item 10 of "The Mac's split, what is owed" go; a line for the Mac to take the detail selection.

## 6. Not in this document

A root tablist as an iPad sidebar, and a three-column split joining it to the list; tvOS; the web's split on a foldable browser; owner-local segments (LLP 1123 D6); arrangement views and `navigationArrangement`, since Apple's guidance is to "keep navigation outside of arrangement views"; two apps side by side on the Duo's inner panel, which is unmeasured.

## 7. Questions for Charlie

- **Q1 — The compact form.** Proposed: (a) D2's one split at every size. Its costs:
  - every eligible stack on every iPhone, including ones the rule admits wrongly (rows that only open sheets), gains a split and a second navigation controller;
  - where a row's detail lies above an earlier one (two selections in the expanded list, or a link inside a detail to another row's detail), the collapsed Back from it clears the selection where a phone today returns to the earlier route.

  The alternatives: (b) swap a navigation controller and the split at the size-class change, keeping the route controllers as `reshape` does, which leaves compact phones exactly as today and makes each fold a container swap without UIKit's transition; (c) a `.compact` column holding today's single navigation controller with the whole prefix, which keeps today's compact Back chain but has Exact move route controllers between navigation controllers at every collapse and expansion, the rearrangement UIKit's nesting avoids.
- **Q2 — Nothing selected while UIKit shows the detail alone.** On the tall panel at launch UIKit shows the empty detail full screen with the list behind its sidebar button. Proposed: UIKit's, as a hand-built split does; D6 shows the list after a Back to the root. The alternative, `show(.primary)` whenever the stack is at its root, puts the list over the detail unasked. What Apple's own apps and an iPad in portrait do there was not measured.
- **Q3 — LLP 1123 stage 2 first.** Proposed: the split lands with stage 2 or after it (D9). The alternative covers a split column's sides in `routeEdges` first, which stage 2 then deletes.
- **Q4 — The working set** holds 15 of 15. Proposed: this document stays out until accepted, and then one leaves for it.
