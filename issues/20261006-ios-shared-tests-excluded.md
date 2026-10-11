# Run shared ExactKit UIKit tests on the iOS test path

**Status:** Open
**Systems:** Apple build, UIKit verification
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** QUEUE.md shared ExactKit tests; host/apple/build.mjs

`host/apple/build.mjs:1474–1484` selects only filenames ending in `IOSTests.swift` and supplies each class as `-only-testing`. Shared classes with UIKit branches compile into the iOS test bundle but never execute.

For example, `SurfaceControlTests.swift` imports AppKit on macOS and UIKit otherwise and defines platform-specific behavior in one shared test class. It does not end in IOSTests and is absent from the invocation. `NavigationRulesTests.swift`, `SegmentsTests.swift`, `ProfileColorTests.swift` and `TextPaintTests.swift` also contain UIKit/iOS code and fail that filename filter. Enumerating the current selector and those files confirms the omission. QUEUE already records a consumer-reported failure missed this way.

Select all tests actually admitted for the destination, with platform-specific test availability handled in Swift or explicit destination metadata. Do not indiscriminately include macOS-only classes.

Acceptance: `bun host/apple/build.mjs --test --ios` lists and runs the shared UIKit methods, including SurfaceControlTests; a deliberate failing shared UIKit assertion makes the command fail. Record the previously skipped tests' real simulator results and fix or file any failures without relaxing assertions.
