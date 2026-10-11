---
name: 20261011-x83-repeating-hatch-across-list-rows
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X83: A hatch cannot run at a fixed period across a virtualized list's rows, so split view's empty side repeats every 10 pt where T3 Code's repeats every 8 px

Moved to main `issues/20261011-repeating-hatch-across-list-rows.md` (2026-10-11), where it is tracked. Main PR
[#430](https://github.com/ccheever/exact2/pull/430) filed it (merged `2d04f681a`).

## Summary

Contract refuses `repeating-linear-gradient()` (`kernel/src/gradient.rs` `REFUSED`), length stop positions and
`background-attachment: local` (LLP 1066 D2, D7). An SVG `pattern` per row builds, but each row's pattern starts at its
own top, so the stripes break at every row edge unless the period divides every row's height. In Chrome one
`repeating-linear-gradient` on the scroller with `background-attachment: local` runs unbroken across rows of any height.

## Why it arose

Task [files-gutter-parity](../../tasks/closed/20261011-files-gutter-parity.md) FG-3 (T3 PR #427): T3 Code (`1e2ecbd975`,
@pierre/diffs 1.3.0-beta.10) hatches the empty side of a split run with one element over the whole run, in 8 px tiles. The
clone draws a virtualized row at a time.

## Clone workaround

`DiffCell` (`diff-rows.contract`) draws the stripes as an SVG `pattern` per row at a 10 pt period, which divides the 20 pt
line (wrapped lines included), with the reference's colours, share and phase; declared in `EXACT2-GAPS.md` ("Split view's
hatched empty side"). It goes when Contract paints the reference's gradient across a list's rows.
