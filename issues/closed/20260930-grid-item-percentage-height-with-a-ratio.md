# A grid item with a percentage height and a ratio, under a ratio container, resolves the percentage against the container's floor

**Status:** Closed
**Resolution:** Grid containers now apply the ratio to parent-assigned dimensions during intrinsic sizing; Chrome 154 and three browser_position.tsv cases agree at 100x100 for the item and 50x50 for its child, with the existing position/ratio and containing-block suite passing.
**Systems:** vendored Taffy, kernel
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** vendor/taffy/EXACT-PATCHES.md patch 12, vendor/taffy/src/compute/grid/types/grid_item.rs

Case (Chrome 154): a `display: grid; width: 100px; aspect-ratio: 2` container (floor 50) holding one item `display: grid; height: 100%; aspect-ratio: 1` with a child `width: 50%; height: 50%`. Chrome: the item's percentage is of an indefinite row, so `auto`; the item stretches to 100 wide and the ratio makes it 100 tall; the container grows to 100×100; the child is 50×50. The kernel gives the container 100×50 and the item 50×50: the item's `height: 100%` resolved to the container's floor (50) during track sizing, as if the row were definite, and the ratio then narrowed the item. It is the grid item's percentage resolution during intrinsic track sizing, not the ratio floor (checked with the floor's definite pass disabled on the follow-up branch).

Found measuring patch 22's cases; the case is kept out of `browser_position.tsv` (the other five nested-ratio and content-taller cases for block, flex and grid pass). Fix in `GridItem::known_dimensions` or the row-sizing estimate: a percentage height of an item in an auto row is indefinite until the row is sized, whatever the container's own min height.
