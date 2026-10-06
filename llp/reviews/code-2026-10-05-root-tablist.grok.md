# Code review: a hidden tablist over a stack's root hides the tab bar (LLP 1075.003 §3.7 as amended), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x24`.
- **Method:** one brief (sha256 `5743a2dd11b3161fb8a42d26d28cd5260001706e9a2af5cd586e6b87512935ef`), shared with astra. Round 1, blind to the other review. Reviewed the staged diff in a worktree at 5111d86af. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r2):** r1 taken by a redesign. The root's wish (`tablistHidden`) is now recorded every batch, even when the bar cannot move yet; the bar moves when the wish changes or the root has just arrived on top: `didShow` calls a new `settleTablist` once a transition settles on a stack's root (Astra 1, Grok 1: a contract or UIKit pop back to a still-hidden root), and selecting another tab's root counts as arriving (`tablistRoot`, Grok 2). A shown tablist shows the bar only if Exact hid it (`tablistHidBar`), so a hook's own hide stands, and between those moments nothing is written, so a hook's own show stands too (Astra 2: the out-of-step repair on every batch is gone). Test (Astra 3, Grok 3): the native fixture gains a Choose button that hides its tablist at Home, and the test drives the real batch path: hide/show at the root, a pushed Detail and a pop back to the still-hidden root (UIKit's pop; the fixture's authored back is the bar's item), the second tab's root, and a hook's show and hide standing between those moments. LLP (Grok 4, 5): the "Not built" clause updated and the new test named; the pop write described as running when the transition settles; the census says `add` is a presented stack's root that the change does not reach. NOT TAKEN: a cancelled interactive pop's own test (both reviews traced it as staying on the pushed-route path, unchanged).

---

NOT READY

1. **A contract pop back to a root whose tablist is still hidden leaves the tab bar shown.** `host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:343` (must-fix)

`followTablistAtRoot` returns immediately unless that root is already `nav.topViewController` and `transitionCoordinator` is nil. On a contract pop, `prepareRoutes` calls `followTablist` before `setViewControllers` (`NavigationIOS.swift:338`, then `:347`), while the pushed controller is still on top, so this guard drops the write. Nothing calls `followTablist` again when the transition ends: `didShow` only syncs if `pendingSync` was set, and this batch already ran. UIKit then shows the bar, because the root's `hidesBottomBarWhenPushed` stays false (the new test asserts that). `outOfStep` never sees the restored bar.

The same early return skips recording `tablistHidden`. A root whose tablist became shown on that pop keeps `tablistHidden == true`. The next batch treats that as a change and calls `setTabBarHidden(false)`, which undoes a hook that hid the bar after the pop.

A push of a shown-tablist route over that hidden root does set `hidesBottomBarWhenPushed = false` on the pushed controller, but it does not call `setTabBarHidden(false)`: the destination is not top yet. The pop is what leaves the bar wrong either way.

**Fix:** Remember the root's desired visibility even when the bar cannot move yet. When the transition ends (`navigationController(_:didShow:)`, after the coordinator is nil), call `followTablist` on the settled stack. For a contract pop the selected key is already the root, so `outOfStep` hides the bar again. An interactive pop that is cancelled shows the pushed controller, so that call stays on the pushed-route path.

2. **Another tab's root, with the tablist shown, leaves the bar hidden.** `host/apple/Sources/ExactKit/IOS/NavigationTitleIOS.swift:346` (must-fix)

`isTabBarHidden` is one value for the whole `UITabBarController`. `changed` is per `RouteController`, and a root first seen with the tablist shown never stores `tablistHidden`. `outOfStep` only repairs "tablist hidden, bar visible".

Hide the tablist on Home (`tablistHidden = true`, bar hidden), then select another tab whose root is showing the tablist. `sync` does call `followTablist` with that root on top (`NavigationTabsIOS.swift:225` switches the tab, then `NavigationIOS.swift:338` prepares the selected stack). For that root, `tablistHidden` is nil and `hidden` is false, so `changed` is false and `outOfStep` is false. The bar stays hidden. The same hole exists if that root previously recorded `tablistHidden == false`: equal values write nothing, so a bar hidden by the other root is left hidden.

A hook that hides the bar while this root's tablist stays shown is a different state and should keep standing: `tablistHidden` is nil or already matches, and Exact did not hide the bar.

**Fix:** On the host, record whether `followTablist` / `followTablistAtRoot` last hid the bar (set the flag in both the root path and the pushed-route `setTabBarHidden`). When the root on top has a shown tablist, `isTabBarHidden` is true, and that flag is set, call `setTabBarHidden(false)` and clear the flag. When the flag is clear, write nothing, so the hook's hide stands.

3. **The new test never runs the pop or a tab switch.** `host/apple/tests/ExactKitTests/NavigationBasicsIOSTests.swift:195` (should-fix)

Hiding and showing while Home is already on top matches `prepareRoutes`: `sync` calls `followTablist` with that stack, and the function reads `style["display"]`. Mutating the node and calling `followTablist` is enough for that part.

The block at lines 195–201 is not a pop. It calls `setTabBarHidden(false)` and then `followTablist` with the root already on top, which is the only state in which `outOfStep` works. A contract pop does the opposite (finding 1). The final `chat()` check covers `hidesBottomBarWhenPushed` on a pushed route, not the root after a pop, and no second tab is selected.

**Fix:** Drive a real contract pop (authored Back, so `setViewControllers` runs) after the root tablist is hidden, and assert the safe-area inset after `transitionCoordinator` is nil. Select another tab whose tablist is shown and assert the bar is back. Keep the direct `followTablist` calls for the at-rest hook case.

4. **The plan still says this behavior is not built, and it states the pop write as if it already happens.** `llp/1075.003-native-platform-control-merged.plan.md:1309` (should-fix)

The amendment at lines 1246–1248 says a pop that brings the bar back is corrected on the next batch. That write does not happen (finding 1). The XCTest paragraph still lists "hiding the tab bar over a stack's root" under "Not built" and does not mention `testTheRootsTablistHiddenHidesTheTabBarAndShownBringsItBack`.

**Fix:** Delete that "Not built" clause, name the new test, and describe the write as running when the transition has settled (`didShow`), once finding 1 is fixed.

5. **The census calls `add` a pushed route.** `llp/1075.003-native-platform-control-merged.plan.md:1253` (nit)

In `contract/corpus/tabs.contract`, the tablist is hidden for `full` (pushed) and for `add`, whose `navigationPresentation` is `modal`. `add` is the root of the presented stack. The new root path does not hide the bar there: the presented navigation's `tabBarController` is not Exact's, so the guard at `NavigationTitleIOS.swift:343` fails. Say that explicitly.

Root detection holds. `routes.first?.key == navigationKey` is true only when the selected route is that stack's root. The selected prefix is then just that root, so the `return` does not skip a cold launch that pushes routes in the same batch: those batches name the deepest route, the first key differs, and the old loop is unchanged, including routes between the root and the named route that still have `tablistHidden == nil`. Another tab's full stack does not contain the selected key, so it still returns without writing, as before. The agent returns at `followTablist`'s first guard. iOS 17 returns inside `followTablistAtRoot` before any write. Sheets fail `nav.tabBarController === tabs`. An app-owned tab container fails `tabBarShows`. A cancelled interactive pop still has the pushed route selected, so it stays on the pushed-route path and `hidesBottomBarWhenPushed`.
