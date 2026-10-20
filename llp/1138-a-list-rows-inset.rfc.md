# LLP 1138: A list row's inset

**Type:** RFC
**Status:** Accepted, 2026-10-20 (the owner's ruling: on every host); built, reviewed (Astra, `~/audits/exact2-no-tells-2026-10-11/llp1138/`).
**Systems:** the kernel's list margins (`Kernel::set_list_margins`, `kernel/src/rows.rs` `apply_margins`, the `list-row-inset` metric), the web's sheet (`host/web/neutral.css`), the Apple host's covers channel (kinds 10 and 11, `host/apple/src/abi_collections.rs`, `covers.rs`, `Bridge.swift`), iOS's painted rows (`host/apple/Sources/ExactKit/IOS/RowCellsIOS.swift`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Related:** LLP 1115.000 D16 (rows of a vertical `list` are cells, each host's row its platform's), the grouped list's margins (`Kernel::set_grouped_margins`, LLP 1116.019), QUEUE.md's "A plain `list`'s rows have no inset of their own".

## Summary

UIKit's plain list starts a cell's text at the cell's layout margin: 16 on an iPhone
upright, 20 in landscape and on the iPhone Duo; AppKit's inset table starts it at 16.
Exact's painted rows (D16) were laid out from the row's own box, so a row whose author
wrote no padding put its text at the list's edge, and authors padded each row 16
themselves. Every look now gives a cells list's rows the cell's inset where their author
wrote none, and a host whose platform's margin varies reports it, as a grouped list's
custom rows take the platform's cell margins.

## Decisions

### D1 — every look insets a row

A cells list's row (`listRows`) whose author wrote no padding at an end has the cell's
margin there, and its box is the cell's (`border-box`, unless its author wrote
`box-sizing`), so a row written `width: 100%` still spans its list with the margins inside
it. The margin is the one a host reports for the list (`Kernel::set_list_margins`), else
the look's (`list-row-inset`: 16 in every column; a terminal's rows take none). iOS reports
the list's view controller's `systemMinimumLayoutMargins`, which a `UICollectionView` plain
list's cell matches (text at 16 on an iPhone 18 Pro upright, measured on iOS 27,
`~/audits/exact2-no-tells-2026-10-11/list-inset-probe/`), so it follows the orientation and
the device as UIKit's does. AppKit's inset table, the default since macOS 11, starts a
cell at 16 (the same probe directory's `mac.swift`); Material's list item has 16 at each
end. The JS target, which has no kernel, says the same in its sheet (`neutral.css`, at
zero specificity, so what the author writes wins).

### D2 — what takes it

The list's child, the row inside a windowed list's wrapper, or a swipe's content: its
scroll's first child, where the scroll names no leading actions (a `delete=` row's), the
scroll taking none, as UIKit's swipe needs the scroll and its content one width. That is
what the web's sheet can select; a swipe with leading actions, or whose content is wrapped,
is its author's to pad. A form control (a field, a switch, a select, a progress bar) keeps
its own padding; a button is a row like any box, and the Apple host places a native button's
title by the inset sides (`Kernel::takes_row_inset`). Layout only, as a grouped list's margins are: the row's own rows are
the author's. A box is styled again as it arrives within a list's rows or leaves them, as a
swipe's children change by any path (a child moved away or destroyed), as a list's `listRows`, a wrapper's `listItemKey` or a swipe's
`swipeContent` or `swipeLeading` changes, and as a host reports a new margin; each of these lays out again a
content region a restyled box owns. The web's rule outranks the controls' own
padding (a row may be a button); what the author writes is inline there and wins. iOS's
separators end at the trailing margin and start, in a row with no text, at the leading one,
drawn after the margin is sampled; the kernel is sent the latest sample.

### D3 — the apps

An inset an app wrote for itself goes in the same change, as D16 dropped hand-drawn
separators: Shelf's `BookRow` and `WorkRow`. The agents guide says the row's content
stands at the cell's margin unless its author writes a padding.

## The gate

`list_rows.rs` holds the look's inset, a reported one, an end the author wrote, a windowed
row, a windowed `delete=` row and a full-width row; Shelf's Library and Discover on the iOS
27 simulator keep their text at 16 with their own insets gone; the web's conformance run,
the UIKit XCTests, the iOS smoke and the six checks.
