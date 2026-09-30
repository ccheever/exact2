# The projected iOS tab bar reports its height to layout instead of drawing outside its box

**Status:** Closed
**Resolution:** Fixed: projected tab bars report intrinsic height to kernel layout and fill its box; kernel regressions, 53 UIKit tests, and an iOS simulator drive pass (root build/test/Clippy remain blocked by unrelated Contract test compile errors). Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** iOS host (`SegmentsIOS.swift`), kernel measure seam
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1059 (the 2026-09-27 ruling), LLP 1035.001

`SegmentsIOS.swift:173-179` lets a projected `UITabBar` grow above its owner when `sizeThatFits` exceeds the authored height. Charlie ruled that the projection stays automatic and its height goes back to layout.

**Do:**
- Report the bar's intrinsic height as the tablist's measured size, through the host measure seam native controls already use, so the kernel reserves it.
- Remove LLP 1059 D2a's overflow.
- Add the three projection shapes to LLP 1035.001's table.
- XCTest: the bar's frame equals the kernel's box.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: projected tab bars report intrinsic height to kernel layout and fill its box; kernel regressions, 53 UIKit tests, and an iOS simulator drive pass (root build/test/Clippy remain blocked by unrelated Contract test compile errors).

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.

## Verification

- Reproduced before the fix: XCTest saw the bar at `(0, -63, 400, 83)`
  inside a `(0, 0, 400, 20)` kernel box, with no intrinsic report; the kernel
  regression rejected a tablist measurement with `NotAnImage`.
- The host coalesces control measurements through the existing intrinsic seam,
  remeasures on resize, and clears them when projection ends. The native height
  supplies the automatic minimum; explicit CSS `min-height` remains authoritative.
  LLP 1059 removes the overflow and LLP 1035.001 lists all three projection shapes.
- `cargo test -p exact-kernel --lib --tests --no-fail-fast`: 106 unit tests and
  297 integration tests passed; two ignored. Added regressions cover reservation,
  short authored height, rehydration, resize, clearing, sibling placement in block
  and flex layout, and explicit CSS minimums.
- `bun host/apple/build.mjs --ios --test`: 53 tests, 0 failures. The added UIKit
  test checks box equality before and after measurement, no repeated report for
  an unchanged size, resize, and returning to authored views.
- `bun host/apple/build.mjs --ios` and `bun scripts/agent.mjs ios` with a tab-bar
  fixture: native selection commits; width changes 390 → 300; projection changes
  height 83 → 20 → 83; the following sibling stays at the box's lower edge.
  Screenshot inspected; no host errors. Evidence: `/tmp/exact-tabbar-drive.json`
  and `/tmp/exact-tabbar-fixed.png`.
- Required root build, test, and Clippy each stop on the same two existing E0061
  errors in unchanged `contract/cli/tests/it/strings.rs:221,223`: `set_place`
  requires its third `Option<f64>` argument. Kept outside this ticket's scope.
  Formatting, caps and boot pass. No design ruling remains for Charlie.
