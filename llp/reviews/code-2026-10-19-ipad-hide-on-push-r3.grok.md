# Code review: an iPad's floating tab bar hidden by a pushed route's hidden tablist, round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `438f19b96d`.
- **Method:** one brief (sha256 `7a3324b39d786efd87c5a3c73efb1fbeb1d61e39e4d39be1027b8203808b61c7`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

Nothing blocks. The hide-on-push path on an iPad in a regular width holds for the cases that used to block: pop to root, `popToRoot`, pop to an intermediate, a settled tab switch, a tab switch while a push is still finishing, a cancelled interactive pop, an in-place wish change, and a hidden root followed by a push.

**should-fix — After the width leaves regular, the next stack update can show the bar over an ancestor that still hides it.** `NavigationTitleIOS.swift:494` (same hole at `:483`)

`followFloatingBar` sets `tablistHidBar` whenever it hides, including when the top route’s own tablist is shown and a route under it has `hidesBottomBarWhenPushed` (`:557`). That is right while the bar is floating: the trailing check and the “on top already” branch both consult `floatingStackHides`, which is true. Once the width is compact, `floatingStackHides` returns nil (`:574`), and `nil != true` is true. The next `followTablist` then takes

```494:496:host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift
        if !hidden, floatingStackHides(nav) != true, tablistHidBar, nav.transitionCoordinator == nil, tabBarShows, let tabs = tabController, nav.tabBarController === tabs {
            if #available(iOS 18.0, tvOS 18.0, *), tabs.isTabBarHidden { tabs.setTabBarHidden(false, animated: nav.view.window != nil) }
            tablistHidBar = false
```

and shows the bar. The resize itself does not: `refollowFloatingBar` no-ops in compact. The show happens on the next projection of that stack (any later batch, including a bump). Chat alone on top stays hidden, because `hidden` is true. The broken stack is Root → Chat (hides) → Detail (shows), then a split to compact, then any `followTablist`. UIKit would keep the bottom bar hidden for that stack.

The on-top branch has the same shape: `floatingStackHides(nav) ?? hidden` (`:483`) becomes “the top route’s own wish” in compact, so an in-place change of Detail back to shown calls `setTabBarHidden(false)` over Chat.

Fix: decide “may show” from the pushed controllers, not from `floatingStackHides != true`. In the trailing `if`, use `routes[1...at]` (the stack being applied; `viewControllers` still contains the route a pop has not removed yet) and require that none of them have `hidesBottomBarWhenPushed`. In the on-top branch, skip the show when `floatingStackHides` is nil and any controller above the root still has the flag. Leave the hide direction on phone and compact to UIKit’s flag, so this does not start calling `setTabBarHidden(true)` there.

**nit — The bar animation is not part of the navigation transition.** `NavigationTitleIOS.swift:558`

`setTabBarHidden(_:animated:)` uses `willShow`’s `animated` flag and is not added with `animate(alongsideTransition:)`. An interactive pop starts a separate bar animation at gesture begin; cancel depends on the later `willShow`, `didShow`, or the coordinator completion to put the settled wish back. The end state is right. The iPadOS 18 safe-area bug with this call during a push was fixed in the 18 beta 6 timeframe. There is no end-state failure in calling it from `willShow`.

**nit — A resize that refuses the alongside completion never retries.** `NavigationTitleIOS.swift:587`

`refollowFloatingBar` always returns once a coordinator exists, and it ignores `animate(alongsideTransition:)`’s `Bool`. If that returns false, the completion is never called and the floating wish is never applied. Apply immediately when it returns false.

**Tests.** `NavigationBasicsIOSTests.swift:352` does not weaken iPhone. `barInsets` is still true there, so the safe-area check remains (`(!insets || chat.view.safeAreaInsets.bottom < shown - 20)`), and the pop-back check is the same. On iPad it now requires the floating bar to leave over Chat and return on Home.

The tab switch you were most worried about is in the tree. The settled Home → Second → Home case is in `testARoutePushedWhileTheTablistIsHiddenHidesTheTabBar` (`:354`). The mid-push case is in `testTheTabBarFollowsTheStackThroughCancelledPopsAndTabSwitches` (`:957`): it switches after `viewControllers.count == 3`, waits `0.6s`, and requires the bar to be showing on Second. The code for that race is right — `followFloatingBar` returns unless `tabs.selectedViewController === nav` (`:545`), so a push that finishes on a tab that is no longer selected cannot write the shared bar. The test does not pin the switch to `transitionCoordinator != nil`. `tapTab` goes through `selectTab` → `act`, which queues the press, so on a slow run the push can settle first and the assertion still passes by the settled path.

Still untested, and the one that matches the should-fix: a route that shows its tablist pushed over one that hides it, then regular → compact, then another projection. Also untested: an inactive tab’s `setViewControllers` while Chat stays selected (the selection guard covers it), and regular → compact → regular while the top route itself hides the bar.
