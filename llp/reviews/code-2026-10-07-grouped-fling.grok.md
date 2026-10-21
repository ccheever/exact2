I'll start with the repo instructions and the two tip commits, then check whether the safe-area inset fix and the header/footer padding change are complete.The review stays read-only. Next I’ll read the binding rules and the two tip commits, then check the safe-area fix and the header, footer, and padding changes against the surrounding list code.The diffs are small. I’ll read the list mount path, cell configuration, and the tests so the safe-area and padding changes can be checked against the real call sites.The safe-area change is the right fix and is complete. The new list padding is only half-applied.

## Should-fix

**Top padding is not room before the first section — `host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:441`**

`mount` adds `padding_top` to `contentInset` and leaves `contentOffset` alone. `update` applies the snapshot before that, so a list taller than its port already has a real `contentSize` and is resting at `contentOffset.y == 0`. That offset is still legal after the minimum becomes `-padding_top`, and UIKit only clamps an offset that falls outside the range. The first section stays at the top of the port. The padding exists as overscroll above `y = 0`. A settings list with `padding_top: 12`, or a header overlay of 92, shows no gap at rest. `padding_bottom` does show after a scroll to the end, because the maximum offset grows. That is the tab-bar case.

`testTheListsPaddingIsRoomAboveAndBelowItsSections` (`GroupedListIOSTests.swift:414`) compares inset numbers only, so it passes while the first section never moves. The agent's list offset is `contentOffset.y + adjustedContentInset.top` (`GroupedListIOS.swift:254`), so at rest it reports the top padding as a scroll position.

The hidden scroll's `contentInset` is the keyboard lift and, while refreshing, the refresh control. Padding is not in it. `mount` rebuilds from that value plus the current style on every sync, and a style batch replaces the whole style, so clearing `padding_top` / `padding_bottom` drops back to zero. `spaceBelow` stays its own term: the last section's bottom inset when that section has no footer, and `contentInset.bottom` when it does. `padding_bottom` adds beside it. Pull-to-refresh stays `PaddedRefreshControl` on the hidden scroll. This inset is on the collection, so the spinner's padding shift and the new inset do not stack, and the spinner still does not follow the visible list.

Put `padding_top` in the first section's content, stacked with `spaceAbove`: `headerTopPadding` when that section has a header, `contentInsets.top` when it does not. Invalidate the list layout when that padding changes. Assert the first section's origin, or `contentOffset.y + adjustedContentInset.top`, at rest.

**The scroll indicator draws through the new padding — `host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:444`**

`verticalScrollIndicatorInsets` is copied from the hidden scroll, which has no list padding. With `padding_bottom: 83` the last row can rest above a tab bar while the indicator runs to the collection's bottom edge, under the bar. `KeyboardToolbarIOS.apply` moves the indicator with the inset for this reason.

Add the same top and bottom deltas (the padding, and `spaceBelow` when it is on the content inset) to `verticalScrollIndicatorInsets`.

## Nit

**The safe-area test never lays a cell on an unsafe edge — `host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:104`**

It does lock the flag. `cell()` calls `layoutIfNeeded` first, then it checks two standard rows, a header's content view, and a custom row's content view after a standard-to-custom reconfigure. Dropping `marginsIgnoreSafeArea()` fails it. The footer, which is the supplementary that sits on the home indicator, uses that same call and is never read. A fitted height inside a non-zero bottom inset is never compared, so a loop that returned only while a footer overlapped the home indicator would pass.

Assert the visible footer's content view the same way. Where a window can provide a non-zero bottom inset, compare one cell's fitted height in and out of that inset.

## Checked, no defect

The cell flag is the right scope. `contentInsetAdjustmentBehavior` still lets a bounce carry a cell into the unsafe area, which is the crash. The list section's own `contentInsetsReference` (list sections keep `.safeArea`; they do not inherit the layout configuration's `.automatic`) is what keeps plain, grouped, and inset-grouped content off the notch and the iPad side inset. Setting that reference to `.none` would remove that side clearance. Sidebar is not a Contract style. The same `#if os(iOS) || os(tvOS)` file compiles the call on tvOS. macOS compiles only the empty `install()`.

Rows, headers, and footers all go through `marginsIgnoreSafeArea()` after `contentConfiguration`, which is when UIKit can replace the content view. Reused cells and custom-to-standard rows take the cell registration handler. Custom rows also return their explicit height from `preferredLayoutAttributesFitting`, so they do not self-size off the safe area. A standard row that sits fully inside the safe area keeps the list's margins, including landscape leading margins. A row that overlaps an unsafe edge, including a fling under the home indicator with adjustment `.never`, no longer grows padding into that overlap. That is the fix. Clearance there is the section inset or `padding_bottom`.

Nothing blocks landing.
