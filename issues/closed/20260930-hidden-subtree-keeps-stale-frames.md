# A box re-parented under a hidden subtree keeps its last frame where a fresh layout has none

**Status:** Closed
**Resolution:** Clear frames and scroll extents throughout inherited hidden subtrees during publication; restored display:none differential draws reproduce three failures before the fix and all 14 layout equality tests pass afterward.
**Systems:** kernel, vendored Taffy
**Severity:** P3
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** kernel/tests/it/layout_equality.rs (the note in `random_style`), vendor/taffy/src/compute/mod.rs `compute_hidden_layout`

Found by extending the incremental-versus-fresh differential with a `display: none` draw (LLP 1074's review round); it fails at origin/main before LLP 1074 as well, so it is not that change.

Seed 5, round 26 of `run` with `rng.side(12) == 0` setting `display: none`: node 6 (`Pressable`, hidden) → 11 (`Pressable`, flex) → 9 (`Pressable`) → 32 (`Text`). The round's ops restyle two other nodes and `SetChildren 11 [37, 9, 34]`, moving 9 under the hidden subtree. The incremental layout keeps 9 at (30.5, −46, 51.5×116) and 32 with it; a rehydrated or replayed kernel has both at zero, as every other box under 6 is. Seeds 10, 405, 1412 and 2423214 show the same shape.

Cause, from reading: when a hidden box's parent is laid out again, Taffy's `compute_hidden_layout` zeroes the hidden box and recurses over its children, but a child that arrived under it after the last hidden pass and whose own layout was cached is not re-zeroed (its cache says clean). Publication then copies the stale layout into the frame; `HIDDEN` is set only for a node's own `display`. On Linux nothing is painted (the painter returns at the hidden box), but the box's scroll extent and any reader of frames see it; AppKit does not hide a subview for an ancestor's `display: none`.

Fix in the kernel or the vendored tree: on `set_children` under a hidden ancestor (or on any hidden pass) clear the moved subtree's layouts, or have publication zero frames under a hidden ancestor. Then put the `display: none` draw back into `random_style` (the note there says where).
