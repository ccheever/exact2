# Code review r2: UIKit's zoom lands photo on photo, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `e650c1aba59bbe9acff3e2ae56ac140f569876af93fbca700f12ec3ba164f237`), shared with grok. Round 2, a delta review of the round-1 dispositions, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r3):** Taken: `prepare` samples the landing (the same resolution the provider runs) while the route's nodes are still mapped, just before `freeze` and `release`, so the zoom back starts from the photo as it is then. DEFERRED: a presentation-level regression (open, transform, Close through route destruction, read the dismissal answer); it needs a session, a navigation projection and a live UIKit zoom in the test host.

---

**NOT READY — one remaining correctness defect.**

**[P2] Close can reuse stale opening geometry — [ModalIOS.swift:330](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:330).** `landing.rect` is refreshed only when the alignment provider successfully resolves the target. After opening, zooming, panning, scrolling, or rotating can change the photo’s rectangle without updating that cache. Close then freezes the current pixels in `prepare`, and `release` removes the destination nodes before UIKit’s dismissal callback. The provider returns the old rectangle, so zoom-out aligns the thumbnail against the opening geometry rather than the displayed photo.

Capture the current alignment **before teardown**, while geometry and raster metadata remain available, and retain it with the retiring presentation. Add a regression that opens, changes the photo’s transform, closes through route destruction, and checks the dismissal provider’s answer. The [current test:20](/tmp/x20/host/apple/tests/ExactKitTests/ZoomAlignmentIOSTests.swift:20) exercises only the geometry helper and cannot catch this lifecycle defect.

The other round-one fixes look correct by inspection: content-box fitting/clipping, recursive container traversal, source-context fallback, and nonanimated alpha restoration on refusal and completion. No separate nil-`viewIfLoaded` defect: the guard returns the optional cached answer without attempting coordinate conversion.

The [LLP:597](/tmp/x20/llp/1035.001-native-interaction-ownership.rfc.md:597) appropriately leaves device validation outstanding. `git diff --check` passed. I did not rerun XCTest in this read-only review; reading the existing result bundle also failed because `xcresulttool` requires temporary-file writes.
