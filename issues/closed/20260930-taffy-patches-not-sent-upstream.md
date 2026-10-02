# Complete the remaining vendored Taffy aspect-ratio upstream submission

**Status:** Closed (2026-10-02; submissions and coverage audit complete)
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

Charlie authorized preparing and submitting the remaining fixes. The first two focused, reviewed PRs were submitted from fresh branches based on upstream main
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

Existing equivalent submissions were retained and credited rather than duplicated.
The submission work does not modify the vendored runtime.

## Completed ratio submission, 2026-10-02

Patch 12 is submitted as [DioxusLabs/taffy#1213](https://github.com/DioxusLabs/taffy/pull/1213),
reviewed commit `da1fac00a19f2fb366c198f7ab16aa6ceac14785`, based on upstream
main `22f941e19e05d1b29621703b4f3a9d96d7839886`. It includes #1210's identical
cache-input implementation as a prerequisite for intrinsic-content probes.
The PR explicitly credits and maps its overlap with #1184 (`l7aromeo`) and
#1081/#1098/#1157 (`nicoburns`). Natural-ratio selection remains the host's
responsibility; the new default-false `aspect_ratio_content_box` flag carries
its sizing-box semantics independently of authored `box-sizing`.

The two earlier bounded passes left 12 failing variants. Charlie authorized
up to seven additional rounds, completed with all shapes passing: grid text's
preferred measurement offer, absolute automatic inline minima, intrinsic
column flex width, percentage bases and stretch, automatic content floors,
content-box ratios with padding, and invalid flex-item ratios.

The final committed suite passes **6,978 tests, four ignored**: 6,693 XML,
153 unit, 127 handwritten and five doctests. It adds 500 Chrome-derived XML
variants from 125 HTML fixtures. The broader audit also passed 7,925 XML cases,
including 1,252 preserved supplemental probes. Formatting, all-features and
no-default-features checks, gentest compilation and clippy passed (existing
warnings only). Independent exact-commit review found no material issues.

Upstream merged #1209 as `8391a347` during validation. #1210 and #1213 are
submitted, not yet merged; the other patch equivalents remain mapped in
`vendor/taffy/EXACT-PATCHES.md`. This closes the submission task. Updating Exact's
vendored dependency when those changes land is separate maintenance.

The earlier experiments remain recoverable on `exact/20261002-aspect-ratio`
through `267074ad`; the submitted branch is
`exact/20261002-aspect-ratio-submission`. Both belong to the isolated worktree
`/Users/ccheever/projects/taffy-exact-upstream-20261002`; the original upstream
checkout's unrelated unpublished work was preserved.
