# UIKit navigation, scroll-settle and share verification fail in the iOS 27 test host

**Status:** Closed
**Resolution:** Use the screen-sized navigation fixture and wait for native scroll settlement; the new screen-edge regression and all 31 focused UIKit tests pass.
**Systems:** Apple host, UIKit verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-13
**Related:** 20261006-ios-shared-tests-excluded

An iostooling verification run at base `9db708c47` on iPhone 18 Pro Max,
iOS 27.0 (24A434), Xcode 27 exposed these existing `*IOSTests` failures:

- `NavigationBasicsIOSTests.testAnInlineTitlesScrollerGoesUnderTheBar`: the
  home-indicator safe-area inset is 0, expected greater than 0 (line 377).
- `NavigationBasicsIOSTests.testAScrollerUIKitInsetsLaysItsContentOutBetweenTheInsets`:
  the bottom inset is 0, expected greater than 0 (line 631).
- `SettleScrollAnimationIOSTests.testASmoothCorrectionIsNativeWorkUntilItEndsOrIsInterrupted`:
  native work remains after landing and interruption (lines 46 and 53).
- `ShareIOSTests.testAnchor` did not complete in over two minutes; the run was
  interrupted at that test. No successful sample was obtained before interruption.

Reproduction: `bun host/apple/build.mjs --test --ios --sim
793D239B-0A6E-48CB-9C12-E87C729207EB`, temporarily setting parallel testing to NO.
This run also included the newly admitted shared classes. These pre-existing
IOSTests are separate from the shared HDR failures recorded in
`20261013-shared-uikit-hdr-tests-fail-on.md`; no baseline isolation or device proof
is claimed. Log: `target/iostooling-verification/ios-canary.log`.

Keep their assertions and qualify the test-host/window setup and native work on
a simulator and device. The normal runner retains its two workers.

## Resolution evidence, 2026-10-13

The new `testTheNavigationFixtureReachesTheScreensBottomEdge` failed with a
402×874 window on the 440×956 iPhone 18 Pro Max simulator. The fixture now
uses its screen's bounds and boots the session at that size. Both original
home-indicator assertions and the new regression pass.

The scroll test passed at baseline; its fixed 0.6-second delay is replaced
with a bounded wait on `nativeInFlight()`. It additionally checks the landed
offset (1700) and the wheel-interrupted offset (1600). Share's unchanged
anchor test passed alone in 1.1 seconds and in the final run in 0.58 seconds;
its reported stall was not reproduced.

All 31 tests in NavigationBasicsIOSTests, SettleScrollAnimationIOSTests,
ShareIOSTests and SurfaceControlTests passed on iOS 27.0. No production code
changed and no physical-device run is claimed. Evidence:
`target/uikittests-verification/baseline.log`, `share-baseline.log` and
`fixed.log`.
