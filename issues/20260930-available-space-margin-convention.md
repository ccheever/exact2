# Taffy's available space is a margin box to leaf and flex, a border box to block and grid

**Status:** Open
**Systems:** vendored Taffy, kernel
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** vendor/taffy/EXACT-PATCHES.md patch 18, vendor/taffy/src/compute/{leaf.rs,flexbox.rs,block.rs,grid/mod.rs}

Upstream Taffy's algorithms disagree on what the `available_space` a box is given means. `compute_leaf_layout` and the flex container entry subtract the box's own margins from it (a margin-box space); the block and grid container entries do not (a border-box space); and the block algorithm pre-subtracts a child's margins before measuring it for its content-based width, so a text leaf with margins inside a shrink-to-fit block is measured too narrow by its margin sum. The absolute solver (patch 18, 24) passes the space its insets, static position and margins leave, which is right for a block or grid box and too narrow by the margins for a text leaf or a flex container.

Case: a positioned block 400 wide; a static block `margin-left: 100px`; an absolute *text* child with `margin-left: 20px; margin-right: 30px` and wrappable text. Chrome measures it in 250; the kernel in 200 (and wraps a line early). The same box as a block holding the text measures in 250 (`browser_containing_block.tsv`, "shrink-to-fit from a static inset, margins", which uses that shape).

Fix in Taffy: one convention. Either every container entry subtracts its own margins (and the block algorithm stops pre-subtracting a child's), or none does and the parents pass border-box space; the leaf and flex paths, the block item measure and the solver change together, with the differential and the Chrome fixtures as the gate. Low priority: an absolute text leaf with horizontal margins and no width is rare in the apps.
