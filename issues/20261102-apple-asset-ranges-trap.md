# A suffix range or an empty given file traps the Apple asset handler

**Status:** Closed
**Systems:** Apple web frame (`host/apple/webarm/WebArm.swift`, `host/apple/webarm/FrameFiles.swift`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A `Range: bytes=-N` header with `N` larger than the file traps the asset scheme handler. A `GET` of an empty given file traps it too. WebKit delivers that header from frame content, and Swift traps the subtraction, so the host process dies.

**Cause.** `AssetScheme.bytes` handles an empty start as a suffix and returns `max(0, count - suffix)`. The subtraction runs before `max`. When `suffix` is greater than `count`, the `Int` subtraction traps. `serveGiven` sets `to = size - 1` for a file whose `seekToEnd` is 0, before it applies a range, so an empty file traps on that line. A non-empty file with a suffix range uses the same `bytes` function.

**Reproduction.** Serve a given file of 10 bytes and request `Range: bytes=-11`. Request a zero-byte given file with no range. In both cases the handler traps instead of answering 416 or an empty 200.

**Fix.** Compute a suffix as `count.saturatingSubtraction`-style arithmetic: if `suffix >= count`, the range is the whole file, otherwise it starts at `count - suffix`. For an empty file, answer 200 with `Content-Length: 0` and no byte range.

**Acceptance.** `bytes=-11` on a 10-byte file returns the whole file, or 416, and does not trap. A zero-byte given file returns an empty body. A suffix shorter than the file is unchanged.

**Resolution.** Clamp oversized suffix ranges before subtraction and handle empty files explicitly. Refuse file sizes that cannot fit the handler’s Int representation.

**Verification after the fix.** The reported traps did not reproduce: the original Swift expressions produced signed negative values. A harness using the actual file handler passed empty-file GET, normal suffix, oversized suffix and Int.max suffix requests. Added both oversized suffix cases to the existing XCTest; a full WebKit app drive remains a native integration check.
