# Code review: an iPad's floating tab bar hidden by a pushed route's hidden tablist, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fdc8ad486d`.
- **Method:** one brief (sha256 `28b223457c6d75d6d041a5c5021f2e526b3126d9a9ce1c4a66c22d72c4dfb3bb`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

## Blocker

**Tab switch does not re-hide the bar — `NavigationTitleIOS.swift:540` (call site `NavigationBarIOS.swift:331`)**

`followFloatingBar` runs only from `willShow`. A tab change does not push or pop: `syncTabs` sets `selectedIndex` (`NavigationTabsIOS.swift:230`) and calls `followTablist`, and `willShow` does not run.

`isTabBarHidden` is one value for the whole `UITabBarController`. Sequence: Home pushes Chat (tablist hidden) → `setTabBarHidden(true)`, `tablistHidBar = true`. Switch to Second, whose root shows the tablist. `followTablistAtRoot` treats that as an arrival (`tablistRoot` was cleared when Chat was selected) and shows the bar, clearing `tablistHidBar`. Switch back to Home, still on Chat. Chat's `tablistHidden` is already `true`, so the pushed-route loop in `followTablist` (`NavigationTitleIOS.swift:473`) does not enter, and the trailing `if !hidden` does not either. Nothing calls `setTabBarHidden(true)` again. The bar stays up over Chat.

The fixture can do this with `tab-second` and `tab-home` after opening Chat. `testARoutePushedWhileTheTablistIsHiddenHidesTheTabBar` never switches tabs.

Fix: when the selected tab changes, after that tab's `prepareRoutes`, call `followFloatingBar` for its `topViewController` (roots return at `at > 0`). Do not call it on every later batch, or a hatch's `setTabBarHidden(false)` while the tablist is unchanged gets undone. The hide branch does not need `tablistHidBar` to still be set. Add that switch-away-and-back to the test.

**The next at-rest `followTablist` shows the bar over a stack that should keep it hidden — `NavigationTitleIOS.swift:492`**

`followFloatingBar` hides when any controller in `viewControllers[1...at]` has `hidesBottomBarWhenPushed`, and it sets `tablistHidBar = true` (`NavigationTitleIOS.swift:546`). The trailing block then shows the bar whenever the selected route's tablist is shown and `tablistHidBar` is set, and it does not look at the stack.

Push Chat (`hidesBottomBarWhenPushed == true`, `tablistHidBar` now true), then push a route whose tablist is shown (Detail). Chat's flag is left true: the update loop only rewrites the selected route or a route whose `tablistHidden` is still nil. On that push, `prepareRoutes` runs first and the trailing block shows the bar and clears `tablistHidBar`; `willShow` hides it again. The next projection, with no transition and the shown route on top, takes the trailing block once more and leaves the bar up. On a phone the same stack stays hidden, which is the rule this commit states.

A root that hid the bar and then pushes Detail still works: the root never sets `hidesBottomBarWhenPushed`, so the stack walk is false and the trailing block should still show.

Fix: in that trailing `if`, and in the "on top already" `setTabBarHidden` (`NavigationTitleIOS.swift:482`), treat the bar as hidden when any controller from index 1 through the top has the flag, same walk as `followFloatingBar`. Only then clear `tablistHidBar` and show. Cover Chat-then-a-shown-route in the test.

## Should-fix

**Becoming regular width does not apply the hide — `NavigationTitleIOS.swift:543`**

The guard is right for where UIKit already honors `hidesBottomBarWhenPushed`: iPhone (including a regular-width landscape phone; idiom is `.phone`), iPad compact, and iOS 17 (the `#available(iOS 18.0, *)` returns before `setTabBarHidden`). iPadOS 18+ regular width is the floating bar, and that is the only place this runs.

It only runs from `willShow`. Push Chat in a compact split (UIKit hides the bottom bar; `tablistHidBar` stays false; `isTabBarHidden` stays false), then expand to regular. No `willShow`. The floating bar comes back over Chat. The trait callback at `NavigationTabsIOS.swift:89` only rebuilds item images.

Regular → compact while already hidden does not show the bar: `isTabBarHidden` sticks, and a later pop still restores through `followTablistAtRoot` or the trailing block, which are not size-class gated.

Fix: from that trait callback, call `followFloatingBar` for the selected stack's top (animated false). In compact the guard returns; in regular it hides if the stack says so.

## Nit

**The hide is not in the transition — `NavigationTitleIOS.swift:547`**

`setTabBarHidden(_:animated:)` uses `willShow`'s `animated` flag and is not added to the transition coordinator. An interactive pop to a shown intermediate starts a separate bar animation at gesture begin; cancel depends on `willShow` running again for the source (it does, and the second call puts the wish back). A pop to the root correctly does nothing here (`at > 0` fails) and lets `settleTablist` → `followTablistAtRoot` restore after the transition. Drive the change with `transitionCoordinator?.animate(alongsideTransition:)`, and call `setTabBarHidden` without animation when there is no coordinator, so a cancel reverses the bar with the gesture.

## What holds

Pop to root, `popToRoot`, and pop to an intermediate at the flag boundary restore. A cancelled pop to the root never shows the bar, so the source stays hidden. A root whose tablist is hidden, then a push of Detail, still shows on Detail and hides again on pop (`testTheRootsTablistHiddenHidesTheTabBarAndShownBringsItBack`). An in-place wish change on the top pushed route still goes through the existing `setTabBarHidden` path. iOS 17 and the phone are unchanged.

The test change does not weaken iPhone: `barInsets` is still true there, so the safe-area check remains. On iPad it now requires `barShows` false over Chat and true again on Home, which is the simple push and pop only.
