# Code review: an iPad's floating tab bar hidden by a pushed route's hidden tablist, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `805aed1da4`.
- **Method:** one brief (sha256 `fd7762685714e581950d0619df8f118dd98ca319dd67c6bb4c94d38c202d3205`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

The round-1 tab-switch repair is real for a settled stack, and it is undone if the tab you left is still in a transition. That still blocks.

**Blocker — A stack that is not the one on screen still writes the one shared bar.** `NavigationTitleIOS.swift:544` (calls at `NavigationBarIOS.swift:331` and `NavigationBarIOS.swift:354`)

`followFloatingBar` never checks that `nav` is `tabController.selectedViewController`. `isTabBarHidden` is one value for the whole controller. `syncTabs` runs before the `changing` guard (`NavigationIOS.swift:491`, guard at `:494`) and will switch tabs while a push on the old tab is still running.

Push Chat on tab A: `willShow` hides the bar and sets `tablistHidBar`. Select tab B’s root before that push settles. B’s `followTablistAtRoot` shows the bar, clears `tablistHidBar`, and records `tablistRoot`. A’s push then ends off-screen. The proxy’s `didShow` still calls `followFloatingBar(A, Chat)`, hides the bar, and sets `tablistHidBar` again. B’s later batches see neither a wish change nor a new arrival (`NavigationTitleIOS.swift:522`), so the bar stays hidden over B.

The same write happens without a tab change. `syncTabs` calls `setViewControllers` on every other tab whose stack differs (`NavigationTabsIOS.swift:224`). That calls `willShow` and `didShow` synchronously, so the other stack’s wish shows or hides the bar over the stack the user is on. The next `followTablist` on the selected stack does not put it back when that route’s `tablistHidden` is already current.

Do not drop the `didShow` call. A cancelled pop needs it. At the top of `followFloatingBar`, return unless `nav === tabs.selectedViewController`.

The new test does not catch this. It switches only after `until("the tab bar left")`, so Chat’s transition is already over. Drive the switch while `transitionCoordinator != nil` and assert the bar is still shown on Second after that coordinator goes nil. Also change the other tab’s stack with `setViewControllers` while Chat stays selected and assert the bar stays hidden.

Nothing else blocks. Checked against the current helpers:

- **Settled tab switch** is fixed. `refollowFloatingBar` after `selectedIndex` re-hides Chat, and the test’s `tapTab` goes through `shouldSelect` → the router → `syncTabs`, on both devices. iPhone is unchanged: `floatingStackHides` returns nil unless the idiom is pad and the width is regular, and the safe-area checks still run when `barInsets` is true.
- **`followTablist` no longer undoes an ancestor hide.** The on-top branch and the trailing clear both use `floatingStackHides` (`NavigationTitleIOS.swift:483` and `:494`). Root → Chat (hides) → a route whose own tablist shows stays hidden at rest, including an in-place wish change on the top route. A root that hid the bar, then a push of Detail, still shows: the root never sets `hidesBottomBarWhenPushed`, so the stack walk is false and the trailing clear runs.
- **Pop to root, `popToRoot`, and pop to an intermediate** restore. Root is `at == 0`, so `followFloatingBar` leaves it to `settleTablist` → `followTablistAtRoot`. An intermediate is handled in `willShow` from the prefix `stack[...at]`, which does not count a controller above the one being revealed.
- **Cancelled interactive pop** now counts a source UIKit has not put back in `viewControllers` (`NavigationTitleIOS.swift:546`), and `didShow` applies that source again with `animated: false`.
- **Width.** The pad-and-regular gate matches where UIKit ignores `hidesBottomBarWhenPushed`. iPhone (including a regular-width landscape phone), iPad compact, and iOS 17 never call `setTabBarHidden` from this path. Compact → regular at rest is `refollowFloatingBar` from the controller’s trait callback (`NavigationTabsIOS.swift:93`). Regular → compact leaves `isTabBarHidden` set; that matches UIKit, which also keeps the bar hidden while any controller above the root has the flag. A later pop still clears it through `followTablistAtRoot` or the trailing block, which are not size-class gated.
- **`setTabBarHidden` from `willShow`.** No end-state failure. The animation is not inside the transition coordinator, so an interactive pop does not track the finger; the second `willShow` plus `didShow` put the settled wish back. The iPadOS 18 safe-area bug with this call during a push was fixed in the 18 beta 6 timeframe. Not a blocker.

Still untested, and worth adding beside the during-push case: Chat then a shown route above it, a cancelled pop between different wishes, and regular ↔ compact while a pushed route hides the bar.
