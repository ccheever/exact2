I'll review the tip commit against the grouped-fling review, starting with the project instructions and the change itself.The review names two padding defects. I'll read the tip commit and the surrounding list code those fixes touch.The diff is small. I'll read the mount path, offset tracking, and the test that claims the padding now shows at rest.The top-padding follow is the right fix, and nothing in it blocks landing. The new test does not lock the case that was broken.

## Should-fix

**The padding test still passes if the offset follow is deleted — `host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:416`**

`presenter` builds a 402×874 list (`GroupedListIOSTests.swift:35`, frame at line 55) of two sections and five rows. That content, plus the 12 and 83 point insets, is shorter than the port, so the only legal `contentOffset.y` is `-adjustedContentInset.top`. UIKit clamps to that inside `setContentInset` and again in `shown()`'s `layoutIfNeeded`. `shown() - top == 12` is therefore true for a short list with or without the new write at `GroupedListIOS.swift:454`. The case from the review is the other one: a list already taller than its port, resting at `contentOffset.y == 0`, where 0 stays legal after the minimum becomes `-padding_top` and the first section never moves.

Give the collection a content size taller than its bounds before applying padding (shorter frame, or enough sections). Assert `contentOffset.y + adjustedContentInset.top == 0` and that the first row's port position moved by 12. Then scroll down, remove the padding, and assert that row does not jump.

## Nit

**A settling fling is not treated as a drag — `host/apple/Sources/ExactGroupedLists/GroupedListIOS.swift:454`**

The guard is only `!collection.isDragging`. After the finger lifts, `isDragging` is false while `isDecelerating` is still true. A padding change that arrives during the top rubber-band (a rotation batch is enough), once the offset is within 0.5 points of `-adjustedContentInset.top`, assigns `contentOffset` and cuts the bounce. `restoreScrollPosition` already treats `isTracking || isDragging || isDecelerating` as the reader still moving. Use that same condition here.

## Checked, no defect

First mount, before layout, content size is still 0, so the list is short and the new write and UIKit's clamp agree on `-adjustedContentInset.top`. The later layout that grows a tall content size keeps that offset; it is already the top of the new range.

Keyboard lift writes the hidden scroll's `contentInset.bottom` only (`KeyboardToolbarIOS.swift:62`, `PresenterIOS.setKeyboardInset`). `moved` watches the top, so a mount while the keyboard is up does not pull a resting list. The refresh control can change the hidden scroll's top inset, but only after a pull, and the finger never reaches that scroll: it is hidden, and the collection is the view on top. `updateRefresh` never calls `beginRefreshing`. A mount that did copy a refresh top inset would treat it as padding and drop a resting list by the spinner height, with the spinner still on the hidden scroll. That path does not run.

`scrollIntoView` and a restored `scrollTop` run before `groupedLists.sync` and write the hidden scroll (`IntoView.swift:72`, `applyPendingScroll`). The wheel writes the collection (`AgentIOS.swift:610`) and does not set `isDragging`. A later mount rewrites the collection offset only when that offset is still the top and the top inset changed, which keeps a wheel that has already left the top. Neither path restores a kept position onto the collection; that desync is older than this commit.

Rotation changes the frame after the inset write (`GroupedListIOS.swift:467`). The top inset does not change unless the style's padding number changes. At the top, the follow moves to the new top; scrolled, the offset is left so the rows stay put. Removing padding while scrolled is the same: `atTop` is false, so the offset stays, and UIKit clamps only if the new range can no longer hold it.

Grok's alternative (padding in the first section's `headerTopPadding` or `contentInsets.top`, stacked with `spaceAbove`) is not better. Bottom padding has to stay a content inset so the last row can rest above a tab bar, and the indicator inset this commit adds is the same room. Putting only the top in the section layout splits one padding into two mechanisms, and a scrolled reader would jump when that section inset changed unless you also adjusted the offset — the same follow, inverted. Keying the follow off "at the top, and the top inset moved" is what keeps a reader among the rows still, which is the behavior the virtualized-list padding rule already wants.

The indicator change matches the review: `added` is the padding plus `spaceBelow` only when that space is on the content inset (last section has a footer). The section-inset `spaceBelow` stays inside `contentSize`, so the indicator already ends with the content.

Nothing blocks landing.
