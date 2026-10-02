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

## Renewed three-attempt pass, 2026-10-02

Charlie explicitly authorized another three attempts. The first attempt reconciled
block/leaf constraint transfer with #1184's authored `min-height: auto` distinction,
content-height minimum, and preferred ratio height for percentage children. It
fixes the four `block_aspect_ratio_fill_max_height` regressions from the first pass.

The green checkpoint is `1dd031567648743b58e0da0a27bf2a76e26eb619`:
6,600 tests pass, four ignored, including all 32 unchanged #1184 variants and
116 other added ratio variants. Fresh Chrome matches all 148 added XML variants
(37 HTML fixtures). Formatting, no-default-features, all-features and clippy pass;
only the pre-existing clippy/no-default warnings remain. This checkpoint covers
block/leaf and container/root sizing, not the full vendor patch. It overlaps
#1184, so it was not submitted as a competing partial PR.

The second and third attempts extended the helper through flex items, grid items
and absolute positioning, and incorporated the cache-key fix already submitted
in #1210 as a prerequisite for intrinsic-content probes. The additional audit
contains 1,252 preserved Chrome variants. Across the three attempts its failures
fell from 288 to 136 to eight. The final integration also regresses four existing
upstream grid variants. The complete final run is 7,842 passing, 12 failing and
four ignored, including doctests. No expected geometry was changed to make a
failure pass; fresh Chrome reconfirmed the existing regression expectations.

The three remaining shapes, each in border/content-box and LTR/RTL variants, are:

| Test | Expected | Final WIP |
| --- | --- | --- |
| `grid_aspect_ratio_fill_child_max_height` (existing upstream suite) | Vertical-writing grid text item 40×20 | 40×100 |
| `zc_ratio_inline_absolute` (additional audit) | Height-derived absolute box takes its content-based minimum width: 100×50 | 50×50 |
| `aspect_ratio_flex_column_min_height_auto_transferred_min_width` (existing #1081 probe) | Container 34×1, child 30×30 | Container 30×1; child is correct |

The renewed three-round limit is reached. This ticket remains open for the
remaining ratio integration and a coordinated upstream follow-up to #1184,
#1081 and #1098; no ratio PR or upstream comment was published. The two reviewed
submissions, #1209 and #1210, remain the completed upstream deliverables.
The vendored runtime is unchanged.

Recovery worktree: `/Users/ccheever/projects/taffy-exact-upstream-20261002`,
branch `exact/20261002-aspect-ratio`, clean at unpublished WIP
`beddc94a86220e0264121c15d0a1bf07a0f3cf31`. The earlier unpublished WIP
`b434834f6cb408f15f1276b3e7bdea1f08d37eb3` and green checkpoint above remain
in its history. Reproduce the four existing failures with
`cargo test -p taffy --test xml grid_aspect_ratio_fill_child_max_height --no-fail-fast`.
The additional audit sources remain in the untouched original upstream checkout,
`/Users/ccheever/projects/taffy-upstream/.tools/zz-xml/zzcb/zc_ratio_inline_absolute__*.xml`
and `.tools/zz6-xml/zz1081/aspect_ratio_flex_column_min_height_auto_transferred_min_width__*.xml`.
Run logs are `/tmp/exact-taffy-ratio-renew{1,2,3}.log`; the first expanded audit is
`/tmp/exact-taffy-ratio-renew1-audit.log`. Preserve the original checkout's unrelated
unpublished work. Another semantic fix loop requires renewed authorization.
