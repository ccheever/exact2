# Run shared ExactKit UIKit tests on the iOS test path

**Status:** Closed
**Resolution:** build.mjs --test --ios now selects every class compiled into the iOS bundle; a serial run of the shared classes (the `IOSTests` classes skipped) executed 393 shared tests and a deliberately failing canary, and after removing it all 11 SurfaceControl tests pass; eight HDR failures are tracked separately. The two-worker run of the full command is not yet shown green (it previously stalled in ShareIOSTests).
**Systems:** Apple build, UIKit verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** QUEUE.md shared ExactKit tests; host/apple/build.mjs

`host/apple/build.mjs:1474–1484` selects only filenames ending in `IOSTests.swift` and supplies each class as `-only-testing`. Shared classes with UIKit branches compile into the iOS test bundle but never execute.

For example, `SurfaceControlTests.swift` imports AppKit on macOS and UIKit otherwise and defines platform-specific behavior in one shared test class. It does not end in IOSTests and is absent from the invocation. `NavigationRulesTests.swift`, `SegmentsTests.swift`, `ProfileColorTests.swift` and `TextPaintTests.swift` also contain UIKit/iOS code and fail that filename filter. Enumerating the current selector and those files confirms the omission. QUEUE already records a consumer-reported failure missed this way.

Select all tests actually admitted for the destination, with platform-specific test availability handled in Swift or explicit destination metadata. Do not indiscriminately include macOS-only classes.

Acceptance: `bun host/apple/build.mjs --test --ios` lists and runs the shared UIKit methods, including SurfaceControlTests; a deliberate failing shared UIKit assertion makes the command fail. Record the previously skipped tests' real simulator results and fix or file any failures without relaxing assertions.

## Verification, 2026-10-13

The existing runner has removed the filename selector and its `-only-testing`
arguments. Xcode runs the destination's compiled tests; Swift's platform guards
remain the authority. No runner change is needed in this lane.

On iPhone 18 Pro Max, iOS 27.0, the shared-class run completed 393 tests,
with three skips. A temporary serial invocation excluded the `IOSTests`
classes with `-skip-testing`; it did not explicitly select shared classes.
`SurfaceControlTests.testSharedUIKitSelectionCanary` deliberately failed and
`build.mjs --test --ios` exited 1. All other failures were the eight methods
already recorded in `20261013-shared-uikit-hdr-tests-fail-on.md`; the remaining
shared tests passed, including ProfileColorTests, TextPaintTests and
SegmentsTests. (`NavigationRulesTests` is compiled for macOS only and is not
in the iOS bundle.)

The canary and temporary filter were removed. The final focused run passed
all 11 SurfaceControlTests, alongside 18 NavigationBasicsIOSTests, the
scroll-settle test and ShareIOSTests (31 tests, zero failures). The runner
retains its original two workers.

Evidence: `target/uikittests-verification/shared-canary.log` and `fixed.log`.
