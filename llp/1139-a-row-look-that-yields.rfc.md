# LLP 1139: A row look that yields

**Type:** RFC
**Status:** Accepted as built, 2026-10-20 (the owner's ruling: a poll's options are the native cell with a checkmark on every host; grouped rows otherwise draw their author's edge).
**Systems:** the grouped sheet's row (`contract/lower/src/grouped.rs` `row`), the kernel's standard-cell reading (`kernel/src/grouped.rs` `grouped_row`), Exact Content's `AnswerTile` (`contract/lib/content.contract`).
**Related:** LLP 1084 D7 (the sheet under the author's rows), LLP 1116.019 (UIKit's content configuration), QUEUE.md's "iOS: a standard grouped row's authored bottom edge is not drawn".

## Summary

A grouped row the kernel read as a standard cell was UIKit's content configuration on
iOS, which draws none of the row's box: an author's border, background image or "no
separator here" was lost there and kept on every other host. Carrying such rows fixes
it, but Exact Content's `AnswerTile`, a bordered tile, is meant to be a poll's cell with a
checkmark accessory in a grouped list, and the web drew its tile inside the card.

## Decisions

### D1 — an authored edge is the author's

A grouped row whose author wrote an edge of it (a border's width or style on any side,
`none` included) or its background image is its own views, carried, on iOS as on every
other host.

### D2 — a look that yields

`-exact-row-look="platform"` on an element: where it is a grouped list's row, its
`background`, `border`, `padding` and `box-shadow` rows are dropped before the sheet is
applied, so the list draws the row and the kernel reads its parts as it would a row's;
anywhere else the attribute is dropped and the element is as written. `AnswerTile`
declares it: a tile in a quiz, a list's cell in a poll, on every host.

## The gate

`packages.rs` holds a poll's tiles as standard cells with the checkmark accessory and no
tile edge; `host/apple/tests/it/grouped_rows.rs` holds D1; a poll in a grouped list on the
web and the iOS 27 simulator draws native cells with the checkmark.
