# LLP 1127: A list and its detail on iOS are UIKit's split view

**Type:** RFC (a decision brief: each decision carries a proposed answer; nothing here is built until Charlie has read it)
**Status:** Draft r30, 2026-10-25. Not in `llp/current/` (Q4). Accepting it accepts the edits made with it to LLP 1116.004 (D1.2), LLP 1115.002 (D4), LLP 1115.000 (D5.2) and LLP 1038 (D12).
**Systems:** the iOS host's navigation; the runner (the detail selection, `inactive`, point reads); the C ABI (`navigate`'s source) and the native module surface (`ExactTab.controller`); the agent; `apps/duo-lab` and `smoke duo`; docs; what every other host does (D13). Contract: `navigationInspector` (D14)
**Author:** Claude (Opus 5.5) for ide
**Implementer:** to be assigned on acceptance
**Date:** 2026-10-16
**Revised:** 2026-10-25 (r30)
**Related:**
- **Exact LLPs.** 1116.004 D1.2 (the shared rule; amended here); 1115.002 D4 (the Mac's split); 1038 (the router; D12, one value, two presentations); 1116.006 D2 (the empty detail) and D4 (`back`); 1115 D5 and 1115.000 D5 (Back, and the bar of a route without a header); 1075.003 §3.6, §3.7, §3.9 (the tab container hatch, one bar per stack, the kept surface); 1078 (`device-posture` and viewport segments; `apps/duo-lab`; `smoke duo`); 1123 (content owners and local clearance; D9, an owner's segments in its own coordinates).
- **Apple.** [HIG: Designing for iPhone Duo](https://developer.apple.com/design/human-interface-guidelines/designing-for-iphone-duo); [HIG: Split views](https://developer.apple.com/design/human-interface-guidelines/split-views); [Preparing your app for iPhone Duo](https://developer.apple.com/documentation/technologyoverviews/preparing-your-app-for-iphone-duo); [`UISplitViewController`](https://developer.apple.com/documentation/uikit/uisplitviewcontroller), its delegate, [`show(_:)`](https://developer.apple.com/documentation/uikit/uisplitviewcontroller/show(_:)), [`Column.compact`](https://developer.apple.com/documentation/uikit/uisplitviewcontroller/column/compact); [`transitionCoordinator`](https://developer.apple.com/documentation/uikit/uiviewcontroller/transitioncoordinator); Tech Talk [Strike a pose with adaptive layouts on iPhone Duo](https://developer.apple.com/videos/play/tech-talks/111463/). iPhone Duo material is iOS 27.1 SDK and simulator evidence (Xcode 27.1), not hardware: §1's table and item 10 were measured with its iOS 27.1 runtime (24A94425) on 2026-10-18, its other items with an earlier iOS 27.1 runtime.
- **Evidence.** §1 states every observation this document relies on. The raw record (`MEASUREMENTS.md`, the probe app, its UI tests, logs and captures) is on the author's machine, in `~/.config/tuft/sessions/1791430083.031489/notes/duo/split/`; ask ide for it.

## Summary

A stack whose root route is a list whose rows push a detail shows the two side by side on the web at 900 px and up and on the Mac at every width (LLP 1116.004 D1.2, LLP 1115.002 D4). On iOS it pushes at every width: the iOS host has no `UISplitViewController`. On the iPhone Duo that is a tell. Apple's guidance is that "a split view expands on the inner display and collapses to a single pane on the outer display", and that "when built with standard components, split views adapt to reserved regions automatically, adjusting width and margins to adapt to the fold" (HIG, Designing for iPhone Duo).

This document projects the stacks the shared rule splits as a two-column `UISplitViewController`, with nothing new for the author to write. UIKit decides how many columns show, their widths and where the bars stand, and on the upright inner display it keeps both columns clear of the fold. The router decides what each column holds and which is on top when UIKit collapses the split. On the tall inner display partially folded, the detail spans the hinge, which stays content's to handle (D9). tvOS takes the same projection once it is measured (§4). Every other host keeps what it has today, the Mac's split, the web's split in CSS and the pushed stack elsewhere; whether and how each of those splits is a document of its own (D13).

## 1. What a hand-built UIKit split does on the Duo

These are observations of a plain UIKit app on the iPhone Duo simulator, iOS 27.1, at the default text size. The app is a `.doubleColumn` split: a table list, and a detail with two bar items; each column's bar prefers large titles. Its columns' navigation controllers were first the ones UIKit makes, then ones the app makes, as Exact would (N0 for the list, N1 for the detail), with the same geometry. Back buttons and edge drags were real touches from XCUITest. Frames are in points; UIKit reports the 130° pose as 127.8°.

| Pose | Window, safe area | The split | Content |
|---|---|---|---|
| Fully open, upright | 951 × 669; right 84, bottom 34 | both columns (`oneBesideSecondary`); the list 0–320 | the detail's view spans 0–951, its safe left 320; detail text 340–867 |
| Partially folded (130° and 90°), upright | the same; a division 455.5–495.5 (margins 20 and 20) | both columns; the list 0–475.7, the fold's centre | the list's cells reach 475.7, their content 455.7; the detail's safe left 495.7 (475.7 and the margin), text from 515.7 |
| Closed (the outer display) | 466 × 678, safe right 84; turned, 678 × 466, safe left 84 | collapsed | the detail over the list, Back in the side bar |
| Fully open, turned | 669 × 951; top 82, bottom 34 | the detail alone (`secondaryOnly`), the list behind UIKit's sidebar button, or over the detail on request (`oneOverSecondary`) | detail text 20–649 |
| Partially folded (130° and 90°), turned | the same; the division across the display | the detail alone | the detail runs across the fold |

1. **Bars.** Upright and open, the list's bar runs along its column's top and the detail's items stand in the vertical bar, as Apple says ("the system shows bars horizontally for the sidebar or content view, and vertically for the detail view", Preparing your app for iPhone Duo). Turned, every bar on the inner display runs along the top; on the outer display turned, the bar's items and Back stand in the leading vertical bar.
2. **Collapse nests N1 in N0.** UIKit pushed N1 onto N0 (`pushViewController`, unanimated), hid N1's bar and showed N1's top in N0's bar; expansion took N1 back with `popToViewController`. These changes had no transition coordinator, and N0's delegate heard `willShow`/`didShow` of the list during expansion.
3. **A person's pops.** Collapsed with [Note 1, Note 40] in N1, the Back button and an edge drag popped Note 40 within N1, then N1 off N0 (`from` N1, `to` the list). A short held drag was cancelled, with the stacks unchanged. Each pop had a coordinator. With pop gates that allowed any pop with something beneath, each drag asked N0's recognizer, then N1's: N1's drove the pop within N1, and N0's the pop of N1 once N1 was one deep. A programmatic push of N1 onto N0 reached both delegates, each with a coordinator from the list to N1.
4. **The collapse proposal is always the detail,** even with nothing selected (the outer display then showed the empty detail over the list, with Back) and after a Back on the outer display (the next opening showed the detail the person had left).
5. **Showing and hiding.** With the list over the detail, setting the detail alone left the list there; `show(.secondary)` closed it, though the SDK header says that while expanded it ignores the secondary column, and this was not remembered; `hide(.primary)` closed it and was remembered across rotation and relaunch until a reinstall; `showDetailViewController` replaced the app's N1 with a navigation controller UIKit made.
6. **Sheets** presented from the list or the detail stayed up through a fold; the split collapsed and expanded under them.
7. **Selection.** A table with UIKit's default `clearsSelectionOnViewWillAppear` lost its selected row when the split expanded again.
8. **A hidden detail bar** on the tall inner display left no button in the accessibility tree; a drag from the leading edge showed the list.
9. **Widths.** A preferred width of 280 held fully open and gave way to 475.7 partially folded; the person's drag on the divider set 390, which UIKit kept across relaunch.
10. **Containers.** At the settled poses compared (fully open and partially folded upright, closed, partially folded and closed turned; partially folded measured at 130°), the geometry was the same with the split as a child of a plain controller, as Exact holds its navigation; in a `UITabBarController` the tab bar stood in the side strip when wide and on the outer display (the leading strip turned), and along the bottom on the inner display turned.

## 2. Decisions

### D1 — Nothing new to write

The Contract is the one that splits on the web and the Mac: a stack whose root route holds a `list` whose rows go down the page, one of whose rows pushes, as `Runner::stack_split` reads it. `navigationSplit` keeps its meanings (`"none"`; a width, which also splits a stack with no list; `"placeholder"` on a box). On iOS a width is `preferredPrimaryColumnWidth`, which the fold and the person's divider override (§1.9); unsaid, UIKit's own (320 on the Duo's wide inner display). LLP 1116.004 D1.2's "a phone keeps the pushed stack" becomes "a compact width keeps the pushed stack".

### D2 — One split, collapsed while compact

Where the iOS host installs a stack's navigation controller today (`NavigationHost.installPrimary`, `NavigationTabsIOS.installTabs`), a stack the rule splits gets a `UISplitViewController(style: .doubleColumn)` whose columns are navigation controllers Exact makes (`makeNavigation`): N0 holds the list, N1 the detail. It is installed at every size, so opening, closing and turning the Duo are UIKit's collapse and expansion of the same controllers. Exact sets no display mode, split behavior, gesture or button visibility. The rule is asked after every batch; a stack that starts or stops splitting changes container between batches (D10). Under `--chrome agent` the stack pushes as before. tvOS keeps its stack until §4's last step. Q1 weighs the alternatives.

### D3 — Which column holds which route

Let P = [root, r1 … rk] be the stack's **nonmodal prefix**, its routes before the first modal route (`NavigationRules.segments`); modal routes are presented over the split as today.

**A detail selection** is the one event this split acts on. The runner reports it with a batch, naming the selected entry (below), when:
- a row of the list was activated (a press, a link's default navigation, an accessibility activation, the agent's tap);
- the activation's dispatch wrote the router without a refusal, whether or not the value changed (`go` to the shown location changes nothing);
- and the router's top is then P's last entry, with P holding a route above its root.

P is found as `NavigationRules.segments` finds the first presentation segment: the routes before the first `modal` or `fullscreen` route. A push after a presentation belongs to that presentation's stack, not to P.

Anything else is no selection:
- an activation that writes no router, or whose write is refused;
- one that ends inside a presentation: a row that opens a sheet, or a sheet and a push within it;
- a dialog or a popover, which writes no router.

Routes that such an activation added to P are still placed by the table below and by D6, with no selection. The Mac's attribution likewise requires the detail to be the selected top below the first sheet (`SplitMac.read`).

Today a link with no `press` handler reaches the runner as the navigation root's `navigate` event, with a location only (`PresenterIOS.press` → `activateLink` → `Session.navigate`). That event gains the activating node's id, so the runner knows the row. A warm incoming link carries none.

**The association.** For each entry of P the runner keeps at most one row, the entry's association, which the list highlights (below). It names the row by its plan site and its instance path within the list (`Tree.site`: each region crossed, with its row key or active arm). A static row is thus named, and rows of nested repeated regions are told apart. A row is in the list while its item is, mounted or not, so a row that a windowed list has scrolled out stays named. The association is set:
- by a detail selection, to the activated row;
- by a current location pair (a mounted row whose link names the entry's location), when the entry has no association, when its row is no longer in the list, or when its row was set by a pair and now links elsewhere (a refreshed list).

Each association records its entry's location when it was set. When the entry's location changes without a selection (a replace inside the detail to another item; `replace` keeps the entry's id), the association is the current pair for the new location, else none until a row's link names it. A row set by a selection is not displaced by another row's pair while it stays in the list and its entry keeps its location, so a row whose press reselects the shown detail keeps it beside a link row that names the same location. An association ends when its entry leaves P, or when a pair set it and its row now links elsewhere with no current pair to replace it. Nothing else changes an association: not its row unmounting, and not a change that keeps the entry's location. The Mac's `rowOf` differs: there a current pair replaces any association (`SplitMac.read`).

**A selected entry** is the entry a detail selection names, the first of these that exists:
- the lowest entry above the root that the activation's batch added to P or gave a new location (`replace` keeps the entry's id);
- when the batch only removed entries or changed nothing, the highest entry of P that is still selected and whose association the activated row holds from a selection, not from a location pair;
- P's last entry.

A row whose `open` yields [root, A, B] selects A, with B above it in the detail's column, as a hand-built split sets its detail navigation controller to both; the same row activated again changes nothing and selects A again, though its link may also pair it with B. A selection unselects every entry above its own in P; otherwise an entry stays selected while it is in P, whatever its location and its association do.

**The base** b is the highest i in 1 … k whose r_i is a selected entry, else 1. Pairs and associations only name the highlighted row; they never move the base, so a link inside a detail pushes within the detail's column, as a hand-built split's detail navigation controller does, and a row that mounts later changes no column. On a compact iPhone the list shows only while P is [root], so a selection's entry is r1, b is 1 and the Back chain is today's. The Mac counts any route with a row as a base (`SplitMac.read`), and keeps that rule here (D13).

The runner reports the selected entries and the associations in `StackSplit` beside `rows`, which keeps reporting the current pairs that the Mac reads.

| P | N0 | N1 | Collapsed | Collapse answer | The list's selected row, expanded |
|---|---|---|---|---|---|
| [root] | root | the empty detail (D7) | [root] | primary | none |
| [root, r1 … rk] | root | [r_b … r_k] | [root, r_b … r_k] | secondary | r_b's associated row, whether mounted or not; none when r_b has no association |
| either, with modal routes above P | as P has it | as P has it | as P has it | as P has it | as P has it; a row that opened the sheet is no selection |

- Routes r1 … r_(b−1) are in neither column. They stay mounted, as a retained route that has left its native stack does (LLP 1123 D2).
- Each split answers its collapse from its own stack, the selected tab's or a retained one's. A presentation does not change the answer: it is primary for [root] and secondary otherwise, wherever the presentation was opened from, and one pending or being dismissed does not count.
- The highlight is the table's last column: it changes whenever the row that column names changes, is shown again when that row mounts (a windowed list, as the Mac retries), and is restored when the split expands (UIKit's default cleared it, §1.7). `RowCellsIOS.ran`, which lets a row go when its screen is on top, does not apply to a list beside its detail.

In the list-detail pattern, whose rows `go` to the first item and `replace` it with the next, each selection is the base and its row is highlighted. Two selections made while the split is expanded leave the earlier in neither column; collapsed, Back from the later clears both, as a hand-built split that replaced its detail shows the list. A deep link into a stack with no selected entry has b = 1: its first route above the root is the base, highlighted when a row is associated with it, and N1 holds the rest of P, whatever rows are mounted. A detail's control that replaces its item with the next moves the highlight to the next item's row.

### D4 — One Back, and clearing the selection

A completed native pop is classified by how many visits the router steps back from the source to the route it lands on, whatever started it (the Back button, an edge drag, the Back button's history menu or the agent's `back`), as today's pushed stack classifies it (`NavigationIOS.completeTransition`, LLP 1115.000 D4.A.3):

- **One Back** is a pop of one visit. It dispatches once through today's order: the route's authored control, else the root's `navigate` with the location beneath, else the runner's fallback (`NavigationHost.goBack`). That includes choosing, from the history menu, the route right beneath the top, within N1 or the list from a base at b = 1.
- **A history selection** is a pop of more than one visit: one the history menu makes within N1 to a route r_j at or above the base, or of N1 off N0 to the list, and a clearing over routes in neither column (below), which the Back button or a drag makes too. It follows LLP 1115.000 D4.A.3: the navigation root's `navigate` receives the chosen visit's canonical URL, or, without a handler, the router returns from the source visit to the chosen visit in one commit, repeated URLs keeping their own visits; no authored Back control is pressed. N1 then shows where the router stands (D6); a refused selection restores the declared columns, and navigation the app makes during UIKit's animation wins.
- **Clearing the selection** is a completed pop of N1 off N0, which lands on the list, or the agent's `back` from the base of an expanded split, which shows no Back. By the rule above it is one Back from the base when N1 holds only the base and the root is right beneath it (b = 1), and a history selection of the root otherwise (N1 holds more than the base, or routes in neither column lie between the root and the base), so none of their Back controls is pressed. On the Mac the same is taking the selection off the row (`SplitMac.closeDetail`).
- **An authored Back control** does what its action does: one `back(nav)` from the base shows r_(b−1) if there is one, else the root.
- **A cancelled pop** dispatches nothing and returns the editor it put away, as today.

### D5 — Following two navigation controllers

1. **Contents.** Each route's `RouteController` is in at most one of N0 and N1. Exact writes their stacks from the router only; UIKit alone nests N1 in N0 and takes it back.
2. **Both followed.** For a split stack the host follows N0 and N1 where it follows one navigation controller today: their pop recognizers, their delegate callbacks, their transitions.
3. **Recognizers.** Exact's gate (`popMayBegin`) answers both, each from the selected route's Back as today:
   - N1's begins when N1 holds more than one route and its top route's Back is available;
   - N0's begins only while N1 is nested in it and holds just the base, and the base's Back is available. With a route above the base, N0's refuses, so a refused Back on that route is never bypassed by a drag that would remove the whole detail.
4. **A transition is its coordinator.** A transition is identified by its coordinator object from its first callback to its end, and handled once, whichever delegates report it; a controller can return an ancestor's coordinator, and §1.3 saw one push reach both delegates. Its `from` and `to` name the native operation:
   - from a route in N1 to a route beneath it in N1: a pop within the detail, of one visit or, from the history menu, several;
   - from N1 to the list: N1 leaving, whose source route, for the drag's snapshot and the editor it puts away, is N1's top;
   - anything that brings a newer controller in: a push.
5. **Whether a Back is owed** stays today's decision (`NavigationRules.dispatchesBack`, with its exception for a source replaced during a drag): a pop that Exact animated because the router had already moved (`setViewControllers`) owes nothing. A native pop owes what D4's classification by visits says: one Back for one visit, a history selection for more, and the clearing when N1 leaves; a push owes nothing.
6. **No coordinator, no Back.** UIKit's collapse and expansion reached N0 and N1 only as changes without a coordinator (§1.2); they never dispatch. Exact's own unanimated changes, a context menu's commit included, never did and do not.
7. **Ends.** A completed pop dispatches what item 5 says, and only once; a cancelled one, nothing; a pop that a fold interrupts is judged by its coordinator's end.
8. **Settlement.** A split stack is in transition while a coordinator of N0, N1 or the split itself runs (the split's follows `show(_:)`, per the SDK header), and from UIKit's collapse or expansion question to its `didCollapse` or `didExpand`: focus waits, and `clock settle` waits (`nativeInFlight`).

### D6 — When Exact calls `show(_:)`

The columns follow P on every batch (D3), a presentation above P or not. `show(_:)` is called only for these:
- **A detail selection (D3):** after its batch, `show(.secondary)`, whether or not the router changed. Tapping the shown row, or a row whose `go` pops back to its detail, closes a list overlaid on the detail; collapsed, it pushes N1 when N1 is not on top and does nothing when it is (SDK header).
- **A new base that a router change made without a selection, or a refused or redirected clearing that left the router above the root** (D4, whether or not the base changed): N1 is set, then `show(.secondary)`. The new base may come from a deep link, an action elsewhere, or a route an activation added to P beneath the sheet it opened; in that last case the detail the router holds is the one under the sheet when it is dismissed, as on a phone today.
- **Navigation from the list's column or an incoming link, without a selection:** an activation inside the root route (a control in the list's header, a row's other control) or a link arriving while the app runs, whose write to the router was not refused, changed or not, and leaves the router's top in N1 with the base unchanged (a push, a replace, a pop to a route still in N1, or the detail already shown): `show(.secondary)`, so the route is not left under a list overlaid on the detail, where D8 makes only the list live.
- **P back to its root:** N1 is set to the empty detail, then `show(.primary)`: collapsed, UIKit pops N1; with the detail alone, the list comes over it.
- **Anything else:** no call. That includes:
  - an activation that writes no router, or whose write is refused, or that ends in a presentation (a row's unrelated action, a refused destination, a row that opens a sheet over the list or over the shown detail);
  - a push or pop that the detail's column made, and every other batch.

Exact calls neither `hide(_:)`, which UIKit remembers (§1.5), nor `showDetailViewController`, which replaces N1. `show(.secondary)` may close a list the person opened over the detail; that is UIKit's, and not remembered.

### D7 — Bars, titles and the empty detail

- **Bars.** Each column's bar is its top route's, projected from the route's header by today's rules; LLP 1075.003 §3.7's one bar per stack becomes one per column. Collapsed, N0's bar shows N1's top route's items. Where items stand is UIKit's: an item with a title and no symbol, or a custom view, stays out of the vertical bar (Preparing your app for iPhone Duo). Titles are UIKit's rendering of the heading's level.
- **A route in N1 counts as pushed** for the bar rule (LLP 1115.000 D5.2), the base included, though it is N1's first route: a route with no bar-shaped header still gets UIKit's bar, which holds Back when collapsed and UIKit's sidebar button when the list is hidden. Today that rule reads "pushed" from the route's place in its navigation controller (`makeNavigation`, `NavigationRules`), which would give N1's base no bar. Collapsed, the route on top decides N0's bar, under its own column's opt-outs.
- **The list's look.** Beside the detail, the list is UIKit's sidebar list (`UICollectionLayoutListConfiguration.Appearance.sidebar`): no separators and no disclosure chevrons, and the row whose detail shows (the runner's `StackSplit.rows`) stays selected. The grounds the platform or the compiler gave the list and its route are left to the split's sidebar, which UIKit draws; a ground the author wrote stays. Collapsed, the list is the authored style again.
- **The empty detail** keeps N1's bar too, with no title and no items. It is not a Contract route. Its content is `UIContentUnavailableConfiguration.empty()` saying "No Selection", in the strings' language as LLP 1116.006 D2 asks, which is English until the iOS host has a strings table for its own words, as on the web. The author's `navigationSplit="placeholder"` box takes its place in stage 3 (§4), with the Mac's.
- **The author's opt-outs stand, as today** (`bar(of:pushed:)`, `barShows`). A detail route has no UIKit bar when:
  - it says `navigationBar="none"` (LLP 1115.000 D5.2);
  - its first child is a shown `header` that says `appearance="none"`, which keeps that header authored, pushed or not (`HeaderRule.keepsAuthoredHeader`; LLP 1075.003 §9.10; LLP 1115.000 D5.2);
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

Each route in a column is a content owner laid out in its controller's view bounds (LLP 1123 D2, stage 1). Its local clearance (LLP 1123 D3) is the column's safe area: for the detail, the list beside it (320 fully open; 495.7 partially folded, the list's 475.7 and the fold's margin) and the vertical bar; for the list, the fold's 20-point margin. This needs LLP 1123 stage 2: until then a route's side edges are covered only while the root does not say `viewport-fit="cover"` (`NavigationBarIOS.routeEdges`), so under `cover` a detail would lie under the list (Q3).

That qualifies the upright inner display, fully open and partially folded. On the tall inner display partially folded the detail spans the hinge (§1), and edge clearance cannot protect an interior fold. Content that must keep clear of it reads its route's segments (LLP 1123 D9, `--exact-viewport-segment-*`), which are in the route's own coordinates, so they describe the column whether or not the column is the window. The document's facts do not change while a split shows (LLP 1123 D1).

### D10 — Tabs, and the app's tab container

A tab whose stack splits holds the split in the tab's place. The router's selected route governs the shared tab bar: the authored tablist's `display` hides or shows it through the tab controller's `setTabBarHidden`, collapsed or expanded, and no route inside a split sets `hidesBottomBarWhenPushed`, which would follow UIKit's nesting rather than the router (`NavigationTitleIOS.followTablist`).

`ExactTab.controller`, public as a `UINavigationController` (`ExactNativeModule.swift`), becomes a `UIViewController`: a navigation controller or a split view controller. The module ABI's major goes from 3 to 4, so a module built against 3, which reads each tab's pointer as a navigation controller, is refused by name at load ("module ABI 3, host ABI 4"), as LLP 1075.003 §3.9 requires; the table's layout does not change. `apps/native-fixture` changes in the same commit, which says what to change. When a tab's stack starts or stops splitting, Exact retires the app's container and asks `tabContainer` again, as when the tabs change (`reshape`). The `navigation` hatch receives N0 and N1 as it receives any stack's navigation controller.

### D11 — The agent and the smoke

The split shows under `--chrome platform` (the default of an app's `exact.mjs agent ios` and `test ios`); under `--chrome agent` the stack pushes, so the new path is qualified under platform chrome only. `state window.split` reports UIKit's state (`collapsed`, `displayMode`, the list's width), D8's visibility per route, each column's routes and the controller hosting them now, `base`, `row` and `empty` as on the Mac, and the selected entries (D3). `tree`, `tap` and `type` follow D8; `back` is D4's.

### D12 — Duo Lab keeps Fold and gains a split screen

Fold stays: it is LLP 1078's consumer of authored segments, and its two-down panes are the only device check of vertical segments (`smoke duo`'s rotate, resize and web legs; `contract/corpus/segments.contract` branches only on two horizontal segments). A fifth tab (`tab split "/split"`, a child `item "/split/:id"`, rows that are links, a header on each route) is the split, and `smoke duo` checks it (§3).

### D13 — Every host

This document changes the iOS host and plans tvOS's step (§4). Every other host keeps what it has today, and nothing here endorses that as its platform's answer: whether and how each splits is a document of its own, judged by LLP 1115's test of what someone who knows the platform would notice, against a hand-built app on it at that size. The rule (`Runner::stack_split`) stays one, read by each host that splits.

| Host | Today | The platform's list and detail | Here |
|---|---|---|---|
| iOS, iPadOS | pushes at every width | `UISplitViewController` | this document |
| tvOS | pushes | `UISplitViewController` | the same projection, after a hand-built split is measured with the focus engine (§4) |
| macOS | splits at every width (LLP 1115.002 D4), reading the rule through the C ABI | `NSSplitViewController` | unchanged: it reads `StackSplit.rows`, the current pairs, as today. Taking the runner's detail selection, and with it D3's base and associations, is a change of its own |
| The web | splits at 900 px and up with CSS (LLP 1116.004 D1.2), its projection reading the same plan facts | none: the page is the platform | unchanged; it does not follow a foldable browser's segments |
| Android | pushes; the reader paints its top app bar and navigation bar from Material's metrics, with no Material library (`Bars.kt`), and links Compose Material 3 only for the date and time pickers an app's plan uses (`-Pcompose=1`) | Material's [list-detail canonical layout](https://m3.material.io/foundations/layout/canonical-examples/list-detail), which AndroidX builds as [`SlidingPaneLayout`](https://developer.android.com/develop/ui/views/layout/twopane) for views and [`ListDetailPaneScaffold`](https://developer.android.com/develop/ui/compose/layouts/adaptive/list-detail) for Compose, each showing the list and the detail side by side where the window has room | unchanged. Its own document weighs painting Material's layout, as the reader paints its bars, against projecting onto `SlidingPaneLayout` from a host whose navigation is Android views (the experimental Views host, PR #245) |
| Linux | pushes; draws the kernel's tree to DRM/KMS buffers with evdev input, or to a buffer with no display (`host/linux/src/lib.rs`) | none: the host has no platform view tree | unchanged |
| Windows | pushes; a winit window over Exact's presenter, which paints the kernel's tree (`host/windows/src/window.rs`) | Windows's [list/details pattern](https://learn.microsoft.com/en-us/windows/apps/design/controls/list-details) | unchanged |

### D14 — An inspector is a sheet a wide split shows beside its detail

A sheet route (`navigationPresentation="modal"`) that says `navigationInspector=true` is an inspector: details beside the content they describe, SwiftUI's `inspector(isPresented:)`. While the split is expanded and the stack's one presentation is that route alone, iOS 26 shows it in the split's inspector column (`UISplitViewController.Column.inspector`) instead of presenting it; collapsed, and on every other host, it is the sheet it says. UIKit decides the column's width and whether the list stays beside it (on the Duo it hides the list), and its ground is UIKit's, as the sidebar's is (D7).

- **The router is unchanged.** The inspector opens with a push and closes with a Back, as the sheet does. The person closing the column (UIKit's `didHide`) is a Back; Exact's own hide is not.
- **Collapse and expansion move it.** At each, a stack whose selected route says `navigationInspector` is synced again: the route leaves the column for a sheet as the split collapses, and the sheet for the column as it expands.
- **The detail stays live** beside the inspector, as the list does beside the detail (D8): the host says so of both (`Kernel::host_block_beside`), and the runner keeps any route the host shows beside the stack's top active.
- **Before iOS 26** there is no inspector column, and the route is a sheet.

## 3. Tests and qualification

**UIKit XCTests,** with Exact's N0 and N1, by the decision each qualifies:
- **D3, selections and the base.** Each row of D3's table. A detail selection by a link row, by a press row, by `replace` from another row (the list-detail pattern), and by reselecting the shown row with an action row's `go`, which highlights that row, also when a link row names the same location. No selection from a row's unrelated action, a refused destination, a row that opens a sheet (from [root] and from [root, A]: no `show`, the highlight unchanged), or a row whose action yields [root, M, C], M `fullscreen` and C pushed within it (P unchanged). A row whose `open` yields [root, A, B] from [root], as a press row and as a default link whose location pairs it with B: A selected, N1 = [A, B], collapsed Back to A and then to the list, and the same row activated again with nothing changed selects A again. The same from [root, X, B] with B selected, where `open` keeps B's id: A selected and B not, N1 = [A, B], A's row highlighted, and once more with nothing changed, A again, not the unselected B. A row R that pushes A₁, a push within the detail, R pushing A₂ at A₁'s location, then R pressed with nothing changed: A₂ stays selected and the base. A link inside a detail to another row's detail, which pushes within N1 with Back to the first, collapsed and expanded. An explicit width with no list.
- **D3, associations and the highlight.** A replace inside the detail that keeps the location, and one to another row's location (the highlight moves to that row, or to none until it mounts; the base stays); a warm `open` that keeps the entry; a selected windowed row unmounted and mounted; a refreshed list whose row for the entry is replaced by a new one. A URL-only arrival (an `open` to [root, A, B], A's and B's rows in the list, no activation): b = 1 and N1 = [A, B] whether or not B's row is mounted, revealed by D6's `show(.secondary)`; A's row highlighted, its association known from its pair; scrolling A's and B's rows out of the window and back changes no column and calls no `show`, and A's row is highlighted again when it mounts. The highlight across expansion.
- **D3, the collapse answer.** With a sheet up, opened from the list and from a detail, pending and being dismissed, and in a retained tab: primary for [root], secondary otherwise.
- **D4 and D5, Back.** The button and a drag dispatch once, within N1 and off N0; a cancelled drag of each kind keeps history and the editor; a drag over [base, child] where the child's Back is disabled and the base's is not: neither recognizer begins; a `replace` during a drag; an authored Back that pops by `setViewControllers` dispatches nothing more; a context menu's commit; a fold while a detail shows dispatches nothing. The Back button's history menu choosing the route right beneath the top within N1, and the list from [root, A] collapsed: each one Back, A's authored control pressed. Choosing a route two below the top within N1, and the list from [root, A, B] collapsed: each one history selection (LLP 1115.000 D4.A.3), `navigate` with the chosen URL or, without a handler, one commit to the chosen visit, a repeated URL included; a refused selection restores the columns; navigation during UIKit's animation wins. Clearing over [root, A] collapsed: one Back from A, and with A's Back refused, opening a confirmation, redirecting or leaving the router unchanged, N1 restored by `show(.secondary)`; two selections made expanded and then a collapse: Back from the later is one history selection of the root, with no `show` and no push or pop of N1 in between, and a refused one restores the columns; a clearing whose authored Back goes straight to the root shows the list (`show(.primary)`), not N1.
- **D6, `show(_:)`.** D6's calls, and none on other batches. A push from the list's header over [root], and over [root, A] with the list overlaid on the detail; a link arriving while the app runs that pushes above the base, one that pops to the base, and one to the detail already shown, each with the list overlaid: each revealed by `show(.secondary)`, with the base unchanged where it was set. A row whose `open` yields [root, A, M], M a sheet: no selection; N1 set to [A] and `show(.secondary)` beneath the sheet, to be measured on the Duo before it is built; A highlighted only if a row is associated with it.
- **D7, bars.** A headerless detail base keeps UIKit's bar, Back and sidebar button; `navigationBar="none"`, a header that says `appearance="none"`, and N1's `showsBar` set false by the hatch each remove that bar, and the list is still reached by the leading drag and by an authored Back control.
- **D1 and D10, containers.** `navigationSplit="none"`; an authored width; a split in a tab; a module built against ABI 3 refused; a bake whose compatibility identity names the preceding C ABI version not offered to the new host (`bake/src/compat.rs`); the `tabContainer` hatch with a split tab and with a tab that starts splitting.

**The runner:** the detail selection and the associations (D3), the `navigate` event's source; `inactive` and `elementFromPoint` in each row of D8's table, with publication after a change no batch caused. D3's rules also get a model-based test: random sequences of row activations (press rows and default links, by `go`, `open` and `replace`, including refused ones and ones that write no router), router writes from elsewhere, incoming links, presentations, replaces inside the detail, list refreshes and windowing, expansion and collapse, and D4's Back, history selections and clearings, each step checked against D3's invariants: with a route above the root, the base is the highest selected entry, else r1; after a selection's batch, its entry is the base; the association branch of a selection's fallback chooses only an entry still selected; an association names a row in the list, or none; and in a history made wholly at a compact width, b is 1, the collapsed stack is P and its Back chain is today's. It finds by search the sequences that reviews found one at a time.

**`smoke duo`,** platform chrome, Duo Lab's split screen fully open, partially folded and closed, upright and turned, and across a fold within the partially folded state: both columns fully open and partially folded upright; partially folded, the list's edge at the centre of the division `layout.env` reports and each column's content clear of it; collapsed with the router's top shown; the detail alone turned; the selection and its highlight across every pose; a Back on the outer display then opening shows the empty detail; a push inside the detail kept across a close and an open; menus and typing in either column; the tab bar where §1 saw it. Fold's checks unchanged.

**Bars:** a title-only item, a symbol with a title, a readout, header search and an overflow, each where UIKit puts it and reachable.

**With LLP 1123 stage 2:** a row's ground reaches its column's edge while its content stops at the clearance (475.7 and 455.7 partially folded).

**Qualification before the split is on by default:** a list and a detail with `navigationBar="none"`; relaunch with a remembered hidden list and column width; a regular-width landscape phone; VoiceOver traversal and focus restoration across the columns; a hardware keyboard in either column; the largest Dynamic Type; a right-to-left language; an iPad resized through compact and intermediate widths, in Stage Manager and in two windows; the cost on a compact phone (controllers at boot; drafts and scroll across a fold); eligibility changing while running, including rows that only open sheets (`QUEUE.md`, "The Mac's split, what is owed", item 1).

## 4. Order of work

1. LLP 1123 stage 2 (Q3).
2. The runner's detail selection, the associations and the `navigate` event's source; the split on iOS (D2–D8, D10, D11), with their tests; the list-detail pattern and `apps/shelf` under platform chrome on an iPhone and an iPad simulator; the qualification list.
3. The author's placeholder on iOS and the Mac together: the lowerer's `display: none` and a containing block in the detail's column.
4. Duo Lab's split screen and `smoke duo` (D12); the docs.
5. tvOS: a hand-built `UISplitViewController` measured with the focus engine, then the same projection, qualified against it for focus movement, focus restoration across the columns and the remote's Back.
6. The milestone (§5), after steps 2 and 4 and the decisions §7 names for it.

## 5. Milestone: Landmarks, twice

Apple's sample [Landmarks: Building an app with Liquid Glass](https://developer.apple.com/documentation/swiftui/landmarks-building-an-app-with-liquid-glass) (SwiftUI; iOS, iPadOS and macOS 26) is Apple's example of a split view app with Liquid Glass. The milestone builds it twice and compares the builds: once from Apple's Xcode project, unchanged, and once with Exact, as `apps/landmarks`. The port takes the sample's data and images from Apple's download, which the repository does not hold. The milestone completes this document's work and qualifies LLP 1123's under a split's columns.

**What Apple's build shows at launch.** Built from the download for the simulator, unchanged, on the iPhone Duo simulator, iOS 27.1 (2026-10-25):
- Fully open, upright (951 × 669): the sidebar is 320 wide. It lists Landmarks, Map and Collections, with the search field at its foot and the Hide Sidebar button at its top. The detail shows the Landmarks page. Its featured image is laid out from x = 0, under the sidebar, and its picture shows from the sidebar's edge (`backgroundExtensionEffect()`). The badges' glass button floats near the detail's bottom trailing corner.
- Partially folded at 130°, upright: the sidebar ends at the fold's centre (476), as §1's list column does, and the detail is on the trailing segment.
- Fully open and at 130°, turned (669 × 951): the detail alone, with the Show Sidebar button.
- Closed (466 × 678): collapsed to the detail, with Back in the side bar. The featured image runs under the side strip.

Taps reach both builds through XCTest's event synthesizer given points in the display's own orientation: on the inner display the upright interface is turned a quarter to the left and the turned one half a turn. Info then shows Apple's inspector as a trailing column beside the detail, with the sidebar hidden, fully open; over the detail, turned; and as a sheet, closed. UIKit remembers the hidden sidebar and the shown inspector across launches, so each comparison starts from a fresh install.

**What the sample uses, and where Exact decides it.** The sample's sidebar is a list whose rows are links to three pages, so the port is a stack whose root route lists them, the shared rule's split (D2). At launch it is `[root, landmarks]`, which a compact width shows as the detail with Back to the list, as `preferredCompactColumn: .detail` does.

| What Landmarks does | SwiftUI | In Exact |
|---|---|---|
| Three pages in the sidebar, each opening in the detail column (`LandmarksSplitView`) | `NavigationSplitView(preferredCompactColumn:)`, a `List` of `NavigationLink(value:)` | this document: a selection by a link row (D3), the collapse answer (D3), `show(_:)` (D6) |
| A card, or the featured landmark's Learn More, pushes its detail within the detail column | `NavigationStack(path:)`, `navigationDestination(for:)` | D3: a push within the detail |
| The featured image and the detail's header image reach under the sidebar and the bars, and stretch as the scroller is pulled down (`FlexibleHeader.swift`) | `backgroundExtensionEffect()`, `ignoresSafeArea(edges: .top)`, `onScrollGeometryChange` | LLP 1123 D11 and D4; the stretch is the app's, from the scroll offset |
| Horizontal lists of cards scroll under the sidebar | `ScrollView(.horizontal)` | LLP 1123 D3's reach |
| Each horizontal list is a fraction of the height its scroller shows, with a minimum | `containerRelativeFrame(.vertical)` | LLP 1123 D12's container units |
| The detail's toolbar: Share; Favorite and a Collections menu of toggles, as one group; Info; fixed spacers between; no title | `ToolbarItem`, `ToolbarItemGroup`, `ToolbarSpacer`, `ShareLink`, `Menu`, `toolbar(removing: .title)` | header buttons are bar items (LLP 1075.003); groups and spacers are not decided (§7) |
| Info shows an inspector beside the detail, to which the sample adds a Close button on an iPhone (`LandmarkDetailInspectorView`) | `.inspector(isPresented:)` on the split | D14 |
| One search field for the split | `.searchable` on the split (nothing in the sample reads its text) | not decided (§7) |
| Everything else: the map, the badges' glass, the collections' grid and editing, a confirmation, a sheet that chooses landmarks | MapKit's `Map`, `GlassEffectContainer`, `LazyVGrid`, `confirmationDialog`, `.sheet`, `TextField`, `TextEditor` | the port lists what Exact lacks, each as a `QUEUE.md` line or a document of its own |

**The comparison.** Both builds run on the same simulators: the iPhone Duo in §1's poses and closed, an iPhone 18 Pro and an iPad, on one iOS version. One XCUITest sequence drives each build: launch; open a landmark; open the inspector and each of its segments; open Collections, a collection and its editor; focus the search field; fold and unfold with a detail open. At each step the test records a screenshot and the accessibility tree (labels, traits and frames). Each difference is fixed or recorded with its reason. The milestone passes when no difference remains that someone who knows the platform would notice (LLP 1115's test).

## 6. Other documents

Each passage below describes what is built, so the commit that builds this document changes it:

- **LLP 1038 D12:** "iOS's presenter is LLP 1127" becomes "and on iOS (LLP 1127)".
- **LLP 1075.003:** §3.6 and §3.9, `ExactTab.controller` is a `UIViewController` and the module ABI major is 4; §3.7, a split's bars are one per column; §9.1, iOS's split list column answers selectors beside its detail; LLP 1038 D6's projection, the same column live on iOS.
- **LLP 1078 D8:** Duo Lab's fifth screen.
- **`host/apple/include/exact.h`:** the `navigate` event (kind 14) carries its activating node beside its location, a changed input form, so `EXACT_ABI_VERSION` goes up by one and enters the bake's compatibility identity as every C ABI change does (`bake/src/compat.rs`). This is the host's C ABI, apart from the native module ABI that D10 moves from 3 to 4.
- **`kernel/tables/schema.json`:** `navigationSplit`'s comment: unsaid, the list's width is 320 on the web and the Mac and UIKit's on iOS.
- **`docs/recipes/navigation-stack.md`, `docs/contract-for-agents.md`, `docs/start-here.md` and the `rest` and `list-detail` patterns' opening comments:** where a list and its detail split on iOS, and that a narrow window, not a phone, gets one column (LLP 1116.011 D4).
- **`QUEUE.md`:** the iPad's split in "A wide window's sidebar and split on iPad and the Mac" and item 10 of "The Mac's split, what is owed" go; a line for the Mac to take the runner's detail selection, and with it D3's base and associations.

## 7. Not in this document

- **What the milestone needs and nothing here decides** (§5): toolbar item groups and spacers, and one search across the whole split. A document of their own decides them before §4's step 6, measured against Apple's build.
- A root tablist as an iPad sidebar, and a three-column split joining it to the list; the web's split on a foldable browser; arrangement views (LLP 1123.001), since Apple's guidance is to "keep navigation outside of arrangement views"; two apps side by side on the Duo's inner display, which is unmeasured.

## 8. Questions for Charlie

- **Q1 — The compact form.** Proposed: (a) D2's one split at every size. Its costs: every eligible stack on every iPhone, including ones the rule admits wrongly (rows that only open sheets), gains a split and a second navigation controller; and where the split expands (an iPad, the Duo's inner display, a large iPhone in landscape), two selections made while expanded leave the earlier in neither column, so after a collapse Back from the later clears both, where today's pushed stack returns to the earlier, as a hand-built split that replaced its detail does. On a compact iPhone the Back chain stays today's (D3). The alternatives: (b) swap a navigation controller and the split at the size-class change, keeping the route controllers as `reshape` does, which leaves compact phones exactly as today and makes each fold a container swap without UIKit's transition; (c) a `.compact` column holding today's single navigation controller with the whole prefix, which has Exact move route controllers between navigation controllers at every collapse and expansion, the rearrangement UIKit's nesting avoids.
- **Q2 — Nothing selected while UIKit shows the detail alone.** On the tall inner display at launch UIKit shows the empty detail full screen with the list behind its sidebar button. Proposed: UIKit's, as a hand-built split does; D6 shows the list after a Back to the root. The alternative, `show(.primary)` whenever the stack is at its root, puts the list over the detail unasked. What Apple's own apps and an iPad in portrait do there was not measured.
- **Q3 — LLP 1123 stage 2 first.** Proposed: the split lands with stage 2 or after it (D9). The alternative covers a split column's sides in `routeEdges` first, which stage 2 then deletes. A side cover is padding added to the route's box, so a route whose `content-box` width fills its column runs past it by the cover, where one with `box-sizing="border-box"` does not (QUEUE, "A covered route column with a width of its own runs past the screen"); stage 2's wrapper shrinks the containing block instead, which a column needs.
- **Q4 — The working set** holds 15 of 15. Proposed: this document stays out until accepted, and then one leaves for it.
