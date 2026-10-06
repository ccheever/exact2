# Code review: a landed flight's view takes its corners from its style (LLP 1013.000 D4/D4.4), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x2fr-rv`.
- **Method:** one brief (sha256 `c9f6d585c0f441ab17b36fe575b6b7547e31b29683d98ff7e49bd031e56cf778`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit b0a52a0dc in a detached worktree. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition (r2):** 1 taken: iOS landing now calls `setNeedsDisplay()` so `NodeLayer.display` runs `applyGradientLayer` with `flightLook` cleared; test `testALandedGradientTakesItsCornersToo` displays the gradient in flight, then lands. 2 taken: overflow changed in flight both ways, and an image landing unclipped, on both hosts (`testALandedArriverTakesTheClipItsStyleGivesIt`); the clip now comes from the style (`overflowClips`) rather than the lift snapshot.

---

**READY WITH CHANGES**

1. **should-fix — [FlightsIOS.swift:190](/tmp/x2fr-rv/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:190): gradient-backed boxes can still land square.** `applyGradientLayer()` copies the backing layer’s radius ([BoxLayerIOS.swift:150](/tmp/x2fr-rv/host/apple/Sources/ExactKit/IOS/BoxLayerIOS.swift:150)). If a display during flight copied the stale radius, this new box pass updates only the parent layer. A clip flight keeps its bounds size, and landing’s `setNeedsLayout()` does not guarantee another display. With visible overflow, the gradient can retain square corners. Refresh the gradient after restoring the box, or explicitly invalidate display on both landing branches. Add a regression that displays the gradient during flight before landing.

2. **nit — [FlightScaleIOSTests.swift:167](/tmp/x2fr-rv/host/apple/tests/ExactKitTests/FlightScaleIOSTests.swift:167), [FlightScaleMacTests.swift:97](/tmp/x2fr-rv/host/apple/tests/ExactKitTests/FlightScaleMacTests.swift:97): the clipping-policy change is untested.** These tests genuinely reproduce the stale-radius bug, including landing without a slot, but keep overflow unchanged and never assert `masksToBounds`. Add hidden→visible and visible→hidden changes during clip flights, asserting clipping after landing on both hosts.

The root cause is correct. Reapplying the box pass safely reconciles borders, shadows, vibrancy and clip masks; material views retain their separate style/layout handling. Normal image landing subsequently calls `applyImageLayer()`. Press animations are not removed by the box pass, and interruption captures the source before landing the old flight, using the clip’s radius for clip flights.

Leaving clip flights’ current `masksToBounds` intact is correct on both hosts. On macOS, the box pass assigns a positive backing radius only when clipping is already enabled. Both surviving-view landing branches receive the corner fix; forget/reset discard views, and reduced motion never lifts them.

I ran small AppKit/Core Animation probes, but did not rerun XCTest in this read-only worktree.
