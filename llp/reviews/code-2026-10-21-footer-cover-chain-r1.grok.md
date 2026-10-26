# Code review: an absolute footer over a route's bottom-edge scroller rests on the tab bar (kernel cover chain), round 1 of 3, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `9aa0361b4a`.
- **Method:** one brief (sha256 `bd73c8bfd3fb75db9274cd1abeab4d25654cebe8a659a899b60d8a363dc1f9f7`), blind to the other reviewer; 3 rounds (kernel, theme2's plan approval) then a narrow check. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** changes (0 BLOCKER, 3 SHOULD-FIX, 3 NIT mentions).
- **Disposition:** taken (the round 1 commit).

## SHOULD-FIX

**SHOULD-FIX — A later restyle of an outer `Edges` box zeroes its padding through an inner one — `kernel/src/kernel/cover.rs:275`**

`scroller_insets_bottom` treats a descendant `Edges` box as just another column. `children_changed` (`cover.rs:361`) starts at the parent, so when the slot that changed is itself `Edges` it restyles the next `Edges` ancestor too. On the Q2 tree (sheet `Edges` 34, route `Edges` 83, inset scroller under the route), `SetChildren` or destroy on the inner route restyles the sheet, the walk finds the inset, and the sheet's bottom padding becomes 0. Content under the sheet's bar. The inset path itself is fine: `covering()` (`cover.rs:300`) stops at the route, which is why `an_inset_three_boxes_down_restyles_its_route_and_nothing_above` stays green. iOS and Linux put `Edges` on sibling routes, so this does not hit the navigators.

Fix: in `scroller_insets_bottom`, if `last` has `HostCover::Edges`, return false before descending. In `children_changed`, if `slot` itself has `Edges`, skip the upward walk (the self-restyle at `cover.rs:354` is already the nearest ancestor) and still call `rows::children_changed`. Add a test on that Q2 tree: after the inset, `SetChildren` on the inner route leaves the scroller ending at `844 - 34`.

**SHOULD-FIX — Display, position, or overflow on a chain node leaves the route's padding at 0 — `kernel/src/kernel/cover.rs:266`**

The new walk skips `display: none` and `absolute`, and stops at a vertical scroller. `style_changed` (`kernel/src/txn.rs:1368`) restyles that node only (and, for `Whole`, its parent at `txn.rs:1389`). It does not restyle the `Edges` ancestor. Destroy and `SetChildren` do (`txn.rs:1210`, `txn.rs:923`). Hiding the inset scroller, or positioning it `absolute`, keeps the route's bottom padding at 0 while the inset is still stored: the in-flow content sits under the bar. On iOS the next `reportCovers` drops `edgeScroll` and the inset, and `covering()` restyles the route one frame later. The kernel does not recover on its own. Revealing a scroller by hiding a trailing sibling stays padded for that frame, which is the safe direction.

Fix: from `style_changed`, when the mask has `Display`, `PositionType`, or `OverflowY`, walk from the parent the way `children_changed` does and restyle the nearest `Edges`. Stop on entering an ancestor that `scrolls`. Do not treat `slot` itself as that stop — `children_changed` sets `inside = scrolls(slot)` first, which would skip the route when the edge scroller's own overflow changes. Add a test from `nested()`: inset on node 6 (column height 792), then `display: none` on 6 without clearing the inset, and the column height returns to 709.

## NIT

**NIT — Several new assertions pass if this commit is reverted — `kernel/tests/it/cover.rs:642`**

- Q1's off-chain sibling (`cover.rs:642`) and the inset inside the edge scroller (`cover.rs:684`) pass on the pre-change kernel, which never looked past a direct child. The assertion that fails on revert is the middle one: removing node 8 grows the column to 792 (`cover.rs:660`). That is the on-chain walk. The off-chain lines are still the right guard against an any-descendant walk.
- Q2's `!restyled.contains(&sheet)` (`cover_tests.rs:108`) also passes on the old code. `restyled.contains(&route)` and the scroller ending at `844 - 34` are the ones that fail without `covering()` and the deep walk.
- `a_scroller_two_boxes_down_reaches_under_the_bar_and_the_footer_still_rests_on_it` fails without the change, including `[116, 0]` then `[116, 83]` (presence stays, the bottom flag flips) and the footer staying at y 721 (`added_padding`, not the zeroed padding).
- The Linux footer test (`bars_tests.rs:327`) does not fail if only the deep walk is reverted. Linux never reports `Inset`; it locks the 4df50 absolute shift with the route's padding kept.
- `testABarPinnedOverTheEdgeScrollerRestsOnTheTabBar` fails without the host restoring the route's bottom cover (the bar's `maxY` misses the tab bar) and without `bottomClearance` (after the keyboard, `safeAreaInsets.bottom` is 0 and the scroll stays above the bar). It does not lock the deep walk. In `keyboard-form.contract:88` `pinned-scroll` is a direct child of the route; the comment at `KeyboardFormIOSTests.swift:181` calls it two boxes down.

**NIT — The pinned-room test resigns the field before the keyboard hides — `host/apple/tests/ExactKitTests/KeyboardFormIOSTests.swift:172`**

The guard at `KeyboardAvoidanceIOS.swift:160` is right. `applyKeyboard` sets `keyboardTop` (`PresenterIOS.swift:308`) before the closure, `PinnedRoom.release()` runs when `top` is nil (`PresenterIOS.swift:322`), then `revealEditorAsTheKeysMove()`. A field still focused would otherwise grow the room again after it was given back. Focus reveal still grows room while `keyboardInset > 0` (`FieldEditingIOS.swift`). The test resigns at line 172 before `applyKeyboard(top: nil)`, so a regression of that guard still passes. Dismiss the keyboard with the field focused and assert `contentInset.bottom` is back to `base`.

## Checks

**(1) Chain walk.** `bottomScroller` (`NavigationBarIOS.swift:1139`) and `scroller_insets_bottom` (`cover.rs:263`) both follow the last in-flow child and stop at a vertical scroller. The kernel returns true only when that node has `Inset` bottom > 0. The host writes that inset only on `collapseScroll` (a direct child of the route) and `edgeScroll` (a node `bottomScroller` already accepted). Where they diverge, the route keeps its padding:

- The kernel has no `alone` or reach-the-foot test (`NavigationBarIOS.swift:1146`, `:1157`). A scroller the host rejects has no bottom inset, so the kernel does not zero.
- The host returns nil at a horizontal `scrollView` (`:1160`). The kernel descends only when `scrolls` is false, and an inner vertical scroller there is not `edgeScroll`, so it has no inset.
- The host's `inFlow` (`:1174`) also drops `fixed` and a closed dialog or popover. The kernel has no `fixed` (`PositionType` is relative, absolute, static, sticky) and drops `display: none`, `absolute`, and `Whole`. A dialog the host skips and the kernel stops on has no inset; padding stays.
- `viewport-fit=cover` forces `edgeScroll = nil` (`NavigationBarIOS.swift:1106`). A nested scroller gets no inset. Cover-page padding stays.

The footer shift reads `added_padding` (`cover.rs:240`), the full edges, so a `bottom: 0` footer stays on the bar while the route's padding is zero. The inset bottom is `max(safe.bottom, bottomClearance)` (`NavigationCoversIOS.swift:117`), so it does not clear when the route's padding lifts the scroller off its own safe area.

**(2) Restyle.** Destroy and replacing the scroller in place recompute the chain (`txn.rs:1210`, `:923`), and the freed inset node is already gone. The two holes are the findings above. Magnitude-only inset changes (83 to 90) correctly do not restyle: padding is all or nothing.

**(3) Cost.** The walk is a parent chase, stops at the first scroller or `Edges`, and restyles one node. A list's own `children_changed` sets `inside` immediately and does not walk. A children change on an earlier sibling still restyles the route; the chain answer is unchanged and the restyle is cheap. Acceptable.

**(4) `bottomClearance`.** It is set from the route's own edges before the inset loop (`NavigationCoversIOS.swift:98`), including to 0. The unsettled branch copies the previous edges and does not read clearance; the inset loop also requires settled and a window.

- Keyboard: `ownerFrame` (`KeyboardAvoidanceIOS.swift:341`) lengthens the tab owner by `keyboardInset`, the route ends at the keys, `reached` is 0, clearance is 0, the inset bottom clears, and the route's edges bottom is 0 too. When the keyboard goes, clearance returns even if `safe.bottom` is still 0, the inset sticks, and the kernel zeroes padding. No oscillation. The iOS test covers at rest and after show/hide.
- Presented routes use the same inset, with `env` zero and `whole` true (`NavigationCoversIOS.swift:31`, `:57`). Same mechanism.
- tvOS compiles this file. `setContentScrollView(for: .bottom)` is skipped (`NavigationBarIOS.swift:1123`). The tab bar is at the top; a zero bottom safe area makes the new inset a no-op.
- Mac `HostCovers` reports `Whole` only (`CoversMac.swift`). It does not use this inset or this edges path.

**(5) `keyboardTop != nil`.** The guard matches the comment above it. Room is added only while the keyboard is up, and it is not regrown by the reveal that runs after `release()`.

**(6) Tests.** The kernel nested-scroller test and the Q2 route-restyle assertion fail without this commit. The Q1 off-chain lines, the Linux footer test, and the iOS pinned-bar test do not lock the deep walk; the iOS test does lock the cover restore and `bottomClearance`. The Q2 test does not lock a later restyle of the sheet, which is the first finding.
