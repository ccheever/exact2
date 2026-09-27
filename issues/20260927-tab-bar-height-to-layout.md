# The projected iOS tab bar reports its height to layout instead of drawing outside its box

**Status:** Open
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

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
