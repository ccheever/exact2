# A height `aspect-ratio` derives is a floor, so a child's `height: 100%` is `auto` where Chrome resolves it

**Status:** Closed
**Resolution:** Resolve percentages against the ratio height while content may enlarge the used height; removed all five owed cases and added overflowing-percentage fixtures against live Chrome.
**Systems:** vendored Taffy, kernel
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1053 G1, LLP 1074 §4, vendor/taffy/EXACT-PATCHES.md patch 12, kernel/tests/it/browser_position.rs `OWED`

Patch 12 makes a ratio-derived height a floor the content can pass (CSS Box Sizing 4 §5.2's automatic minimum): the box's height stays `auto` with a minimum. Under a block or grid parent that means the height is not definite, and a child's `height: 100%` resolves to `auto`. Chrome resolves the percentage against the ratio's height and lets content beyond it extend the box.

Five cases in `browser_position.tsv` are declared owed (`OWED` in `browser_position.rs`): `block in block: width + ratio, a child that fills` and its `block in grid`, `flex in block`, `flex in grid` and `grid item` forms; each expects the child 100 × 50 and gets 100 × 0. `min-height: 0` on the box is the way round it today.

Fix: two passes at the parent's item path (block items, grid items; flex passes a definite cross size already): measure the content, and when it fits give the box the ratio's height as a definite known dimension; otherwise lay it out at `auto` with the floor. The `OWED` list makes the test fail the day it lands.
