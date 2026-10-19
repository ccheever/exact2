# LLP 1138: A list row's inset

**Type:** RFC
**Status:** Draft, 2026-10-19; built for iOS, reviewed (Astra, `~/audits/exact2-no-tells-2026-10-11/llp1138/`) and not landed: the owner's decision (QUEUE.md). The code is in that directory's `code-1.diff`.
**Systems:** the kernel's list margins (`Kernel::set_list_margins`, `kernel/src/rows.rs` `apply_margins`), the Apple host's covers channel (kinds 10 and 11, `host/apple/src/abi_collections.rs`, `covers.rs`, `Bridge.swift`), iOS's painted rows (`host/apple/Sources/ExactKit/IOS/RowCellsIOS.swift`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Related:** LLP 1115.000 D16 (rows of a vertical `list` are cells, each host's row its platform's), the grouped list's margins (`Kernel::set_grouped_margins`, LLP 1116.019), QUEUE.md's "A plain `list`'s rows have no inset of their own".

## Summary

UIKit's plain list starts a cell's text at the cell's layout margin: 16 on an iPhone
upright, 20 in landscape and on the iPhone Duo. Exact's painted rows (D16) are laid out
from the row's own box, so a row whose author wrote no padding put its text at the list's
edge, under separators that UIKit's rule had inset. The host now reports the margin its
platform's cell has, and the kernel gives a cells list's rows that inset where their
author wrote none, as a grouped list's custom rows take the platform's cell margins.

## Decisions

### D1 — the margin is the host's

A host whose platform's list cell has a margin reports it per cells list (`listRows`), at
its left and right: on iOS, the list's view controller's `systemMinimumLayoutMargins`,
which a `UICollectionView` plain list's cell matches (text at 16 on an iPhone 18 Pro
upright, measured on iOS 27, `~/audits/exact2-no-tells-2026-10-11/list-inset-probe/`), so
it follows the orientation and the device as UIKit's does. A host that reports none (the
web's neutral look, whose rows run full-bleed under full-width lines as a web list's do;
the Mac, Linux and Windows for now) lays the rows out as written.

### D2 — what takes it

The list's child, the row inside a windowed list's wrapper, or the content a row's swipe
names (a `delete=` row's), whose scroll takes none: its padding at each end its author
wrote none of, in layout only. Such a row is `border-box` unless its author wrote
`box-sizing`, as a grouped row is (the sheet's), so a row written `width: 100%` still spans
its list with the margins inside it. iOS's separators end at the trailing margin and start,
in a row with no text, at the leading one.

## What landing it needs

The iOS half alone doubles an inset an app wrote for itself: Shelf's `BookRow` keeps a
16-point inset on an inner row so its web rows, which the neutral look draws full-bleed,
have one, and on iOS it would stand at 32. So it lands with: the web's sheet giving the
same inset (`data-exact-said` marking a row's written padding, a `neutral.css` rule for
`[data-listrows]`'s rows and windowed wrappers, and the neutral look's choice of 16), the
Mac's and Android's hosts reporting theirs, and the repository's apps dropping their
hand-written insets in the same commit, as D16 dropped hand-drawn separators. The review
found, besides: a windowed `delete=` row whose scroll is padded beside its content (UIKit's
swipe then refused); no restyle when a row's `swipeContent`, `id` or `listItemKey` changes;
swipe content under an authored wrapper not reached; a content region's publication kept
when the margins change; iOS's separators drawn before the new margin is recorded.

## The gate

`list_rows.rs` holds the inset, an end the author wrote, a windowed row and a full-width
row; Unsaid's list on the iOS 27 simulator puts each row's text 16 in from the row; the
UIKit XCTests, the iOS smoke and the six checks.
