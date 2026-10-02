# Complete the remaining vendored Taffy aspect-ratio upstream submission

**Status:** Open
**Systems:** vendored Taffy
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** vendor/taffy/EXACT-PATCHES.md (patches 10, 11, 12, 14, 17, 18, 20), LLP 1074 §0.1

`EXACT-PATCHES.md` marks patches 10 (percentage padding basis), 11 (the cache keyed on every input), 12 (`aspect-ratio` as CSS), 14 (an item's contribution clamps before its margin), 17 (replaced elements never stretch to a grid area or insets), 18 (one absolute-box solver) and 20 (`Position::Static` and the containing block) as upstream material. Each is written against upstream 0.14.0 with Chrome-differential cases the upstream test suite could take. At filing, none had been prepared as a pull request or sent (DioxusLabs/taffy). Both reviewers of LLP 1074 said the same: worth doing, as separate maintenance work, one patch per PR, with fixtures. Until then every Taffy upgrade re-applies them by hand.

## Triage, 2026-09-30

Still valid maintenance work. The local fixes and Chrome fixtures are retained
in `vendor/taffy/EXACT-PATCHES.md` and the kernel tests. Sending upstream pull
requests or messages is an external communication not authorized by this repo
cleanup request, so no submission was attempted and this issue remains open.

## Submission and coverage audit, 2026-10-02

Charlie authorized preparing and submitting the remaining fixes. Two focused,
reviewed PRs are now open, each from a fresh branch based on upstream main
`fb461a7826e49f488f31220744bf12227ffb580e`:

- Patch 10's remaining flex percentage-padding paths:
  [DioxusLabs/taffy#1209](https://github.com/DioxusLabs/taffy/pull/1209),
  commit `ff48fe36d091e4ccec4fab4ddbdb33b59f74f128`.
  Eight of 16 new Chrome-generated variants fail before; all pass after.
  6,468 tests pass after (four ignored), including doctests.
- Patch 11's missing sizing-mode and margin-collapse cache inputs:
  [DioxusLabs/taffy#1210](https://github.com/DioxusLabs/taffy/pull/1210),
  commit `151f2cbaa7793d07cec016f9501e7e39efd59109`.
  Three restyle regressions fail before; all pass after. The percentage-parent-
  height case is already green on current main and is retained as a control.
  6,458 tests pass after (four ignored), including doctests.

Both passed formatting, feature checks and clippy (only pre-existing warnings).
The complete seven-patch mapping is now in `vendor/taffy/EXACT-PATCHES.md`:
patches 14, 17, 18 and 20 already have merged or submitted upstream equivalents.
Fresh probes verify the relevant behavior rather than relying on PR titles:

- Patch 14: 32 row/column margin cases, 24 pass on current main and all 32 pass
  at [#1166](https://github.com/DioxusLabs/taffy/pull/1166) head `6c6fcb15`
  (which includes [#1165](https://github.com/DioxusLabs/taffy/pull/1165)).
- Patch 17: three natural-size grid cases fail current main and pass at
  [#1158](https://github.com/DioxusLabs/taffy/pull/1158) head `d66b6e6c`.
  The non-ratio absolute-inset cases already pass main after merged #1203.
- Patches 18/20: 528 non-ratio Chrome XML cases cover margins, absolute boxes
  and containing blocks. Main passes 480; applying existing
  [#1206](https://github.com/DioxusLabs/taffy/pull/1206)'s source at `b0a6a1b3`
  passes 520, with only the eight patch-14 cases above failing. The existing
  PR covers negative block-axis auto margins, inset/static shrink-to-fit and
  the two RTL static-position failures. Shared out-of-flow hoisting and static
  positioning are already merged in #1194/#1140.

No duplicate PRs, upstream comments or modifications to the existing authors'
work were made. The vendored runtime remains unchanged.

## Remaining blocker: patch 12

Patch 12 is not wholly represented upstream. Existing #1184 (`l7aromeo`) covers
block content-height minima/percentage children; #1081, #1098 and #1157
(`nicoburns`) cover portions of flex, grid and replaced-element ratio sizing.
Those overlaps do not establish equivalence for the entire vendor patch.
Still needing coordinated implementation/fixtures are min/max transfer only
into unsized axes across block/flex/grid/absolute/root paths, preservation of
definite dimensions, content-box natural ratios, and automatic inline minima
with height-derived widths. Percentage resolution and content-height floors
must be reconciled with #1184 rather than duplicated.

A narrowed block/leaf constraint attempt fixes 12 newly added Chrome variants
(both dimensions authored, width plus max-height, height plus max-width), but
regresses the four existing `block_aspect_ratio_fill_max_height` variants:
`border_box_ltr`, `border_box_rtl`, `content_box_ltr`, `content_box_rtl`.
A text box expected at 40×60 becomes 40×20. It needs the content-height-minimum
integration above. The repo's three-round fix limit was reached, so this
attempt was **not submitted** and this ticket stays open.

Recovery: upstream worktree
`/Users/ccheever/projects/taffy-exact-upstream-20261002`, branch
`exact/20261002-aspect-ratio`, unpublished WIP commit
`b434834f6cb408f15f1276b3e7bdea1f08d37eb3`. Switch to that branch and reproduce
with `cargo test -p taffy --test xml block_aspect_ratio_fill_max_height
--no-fail-fast`; the full suite at that commit has 6,460 passing tests, four
failing and four ignored. The untouched pre-existing unpublished work remains
in `/Users/ccheever/projects/taffy-upstream`; do not overwrite it.
A renewed scope/decision is needed before another ratio fix loop.
