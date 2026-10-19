# Code review: the iPad-only Swift tests and the PressFocus sheet flake, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `bfb42b8a7e (before the rebase; the same diff as e93c9cadbe..940ebde15f)`.
- **Method:** one brief (sha256 `79d86a78fb5b143c2a48c452ca762f76f8602f005c062eee2f6ae71c264fbc6b`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Nothing blocks. The simple push gives the bar its touches back, and on this controller the floating tab's cell maps to the right tab. Both are what the iPad run covered.

## Findings

**should-fix** — A dropped restore never retries, so the bar can stay dead — `host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:350`

The true-to-false edge is the right shape. `didShow` snapshots `isUserInteractionEnabled`, forces layout, and schedules a restore only when that layout turned it off. A bar UIKit had already disabled (an interactive pop still down, a hide `showBar` started in `willShow`) is left alone. `setNavigationBarHidden` is not the setter the swizzle named; `_setItems:transition:reset:` is.

The guard then throws the restore away. It runs on the next main-queue turn and returns without doing anything when `transitionCoordinator != nil`. `coordinator == nil` is also not "the item animation finished": that crossfade is about 0.35s, and this block is one turn later. The same `didShow` calls the host before the block runs, and `transitionEnded` (`NavigationIOS.swift:1049`) applies a `pendingSync` batch with `setViewControllers(animated:)`. A batch that arrived during the push starts a new animated transition, the guard returns, and the flag stays false. The next `didShow` snapshots `touches == false` and does not schedule again, because UIKit never puts the flag back after the delegate. The same drop happens if the app delegate's `didShow` starts a transition, or an interactive pop begins before the block. An early `true` on the simple path is the fix (that completion never runs; the new test holds the flag for 0.3s after the coordinator clears). The hole is the one-shot return.

Keep `layoutIfNeeded` in this callback. `coversChanged` reports covers synchronously from the host `didShow`, and `afterBatch` runs immediately when nothing is applying, so skipping layout whenever a coordinator exists would report a stale safe area. There is no public "deferred transition pending" check. Reschedule while `transitionCoordinator != nil` (bounded) and while the bar still has an in-flight animation; set `isUserInteractionEnabled = true` only if it is still false.

The same code runs on iPhone and schedules only if `layoutIfNeeded` flips the flag. The deferred item application is the iPadOS path the commit names, and the iPhone suite passed, including the new flag assert. If a phone path starts deferring items, this same one-shot drop applies.

**should-fix** — Selecting a floating tab outlines nothing — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:303`

The cell-to-index mapping is right for Exact's `UITabBarController` (classic `setViewControllers`, not `UITab`). `indexPath.item` matches tab order, the iPad test counts exactly two cells for two tabs, and a hit on the second cell returns that tab's id. `indexPath.item` is per section; this bar has one section of item cells.

The view handed back is `session.presenter.views[id] ?? cell`. The authored tab is hidden while the native bar shows it (`SegmentsIOS.swift:385`), and it is still in the window. `press(where:)` (`InspectIOS.swift:454`) and `outline` (`:103`) then prefer `place(of:)`. `place` (`:147`) still requires `tabBar.window != nil` and returns a closure that yields nil, so `follow()` hides the press box and the outline. Returning the cell as the view would not help: the hidden node exists, so `place` wins.

In `place`, when `tabBar.window == nil`, walk `tabs.view` for the `UICollectionViewCell` whose `indexPath.item` is that tab index (the same mapping as `floatingTab`) and return the cell's window frame.

**should-fix** — The push that hides the tab bar is no longer observed on iPad — `host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:338`

`barInsets` (`:319`, `tabBar.window != nil`) is the right device split. A compact iPad still has the bar in the window and still checks insets. `testTheRootsTablistHiddenHidesTheTabBarAndShownBringsItBack` still reads `isTabBarHidden` on every device and only gates the inset magnitudes.

`testARoutePushedWhileTheTablistIsHiddenHidesTheTabBar` does not. On a floating bar both "the tab bar left" and "the tab bar came back" sit inside `if insets`, so the test only checks `hidesBottomBarWhenPushed`, the flag Exact writes. That push uses `hidesBottomBarWhenPushed`, which the root-tablist test does not cover (its Chat assert at `:429` is the same flag). The comment at `:316` says the bar's own state is read on iPad; this test never reads it.

Around the Chat push and the pop, assert `tabs.isTabBarHidden` on iOS 18 on every device (hidden over Chat, shown again over Home). Keep the inset waits inside `if insets`.

**nit** — Any non-cell sibling of the selected tab is reported as the tab bar — `host/apple/Sources/ExactKit/IOS/InspectIOS.swift:307`

The comment says nil where no floating bar is at the point. Content is excluded: the hit must be under `tabs.view` and outside the selected controller's view. Sheets are presented from `owner` (`NavigationIOS.swift:644`, the top sheet or the parent of the tab controller), so they are not descendants of `tabs.view`. A popover inside the selected controller is excluded by the same check; one attached above the tab controller never enters `floatingTab`. No `tabController` returns nil. A compact width has `tabBar.window != nil`, so this path does not run. The container background is not what `hitTest` returns while the selected view fills the screen.

The fallback still fires for every other sibling: a `tabs` hatch subview added on the controller (a replacement container nils `tabController` and is fine), a future bottom accessory, a sidebar. The normal walk would have found a hatch view that is a node. Claim the tab-bar message only after the walk has passed through the item `UICollectionView` or a container of it. Otherwise return nil.

**nit** — The status-bar-tap count ignores the floating list — `host/apple/tests/ExactKitTests/InspectIOSTests.swift:154`

`syncScrollsToTop` (`ScrollsToTopIOS.swift:17`) only toggles the viewport and known node scroll views. The file's own rule is that UIKit scrolls on a status-bar tap only when exactly one scroll view has `scrollsToTop`. The test now drops tab chrome from that count. The floating list is a short top bar, so UIKit is unlikely to treat it as the vertical scroller, and the exclusion matches that. If that collection view ever scrolls vertically, the gesture breaks and this test still passes. Set `scrollsToTop = false` on that collection view in `syncScrollsToTop`, and assert the flag.

**nit** — Forty rows is a fixed guess at "any screen" — `scripts/fixtures/grouped-list-reveal.contract:36`

The iOS reveal test still requires the inserted row to be unrealized before reveal and realized after. Forty was enough for the iPad Pro 11-inch run. The comment says any screen; a 13-inch portrait is taller and this number is not taken from the window. macOS `GroupedRevealTests` and `GroupedMarginsIOSTests` share the fixture. The macOS window stays 402×874, and margins still reveals the remove button explicitly, so those two are unchanged. Derive the row count from the window height if a taller pad should be covered.

## Checked, and holding

Press focus. `testADismissedSheetHandsNothingOn` takes the agent clock before the tap. `noteLoading` becomes true in `openFocusedNote`, and `after(1000, noteLoaded)` arms only then (`app.contract:275`). With `session.clock` set, `scheduleClock` and `advanceIfDue` do not advance that timer, so the answer stays unanswered however long the sheet takes. The sheet's animation is `animated: !ExactEnv.agentFreezes` (`ModalIOS.swift:647`), independent of the clock. The handoff rules are unchanged: the test now asserts no textarea yet and a live handoff, then dismisses with `animated: false` and expects the handoff cleared. Taking the clock also runs `startOnFrame(false)` and `syncProgress` (`Session.swift:402`, `Agent.swift:653`). That can start motion that was waiting for a frame and hold a progress indicator. The note timer is not armed yet, and the test never reads that indicator. The sibling test still waits for the textarea and does not take the clock. This is a sound fix for the flake.

Face symbols and the tab-bar-hidden helper. `FaceSymbol.tabBarPoints` is 17 at a regular horizontal size class and 23 otherwise. The tests compare against the bar's own trait, then force compact, regular, and compact through `traitOverrides`. Both classes are still exercised. `NavigationTabsIOSTests.tabBarHidden` reads `isTabBarHidden` on iOS 18, which is how the host hides the floating bar.

Elsewhere that still assumes `tabBar` is in the window, all of it already true before these commits: `AgentRevealIOS.bars(over:)` drops a tab bar whose window differs from the scroll view's (`AgentRevealIOS.swift:131`), so reveal does not treat the floating bar as a cover. `AgentCheckIOS.isBar` matches `UITabBar` only. `buttonBoxes` is only reached from `place`, which is the finding above. `Agent.tabItem` already walks `controller.view` for the item's accessibility id, including the iPad bar. `registerForTraitChanges` is attached to the off-window `UITabBar` (`NavigationTabsIOS.swift:89`); `syncItems` already substitutes the page's traits when `tabBar.window == nil`, and a live size-class change may not resync until something else calls `resyncTabItems`. `QUEUE.md` still has `elementFromPoint` clipping a tab panel under the floating bar.
