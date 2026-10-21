# Code review: a flight's arriver draws the leaver's image until its own lands (the photo flicker), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `c21d8135309c7eb72f750693d158b82aa5ad224a7ac7b06d6e74700c30683184`), shared with grok. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at `86b3d1818`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition (r2):** 1 taken: the interrupted branch takes `natural` from the raster it keeps, and `showFlight` sizes the fit from `view.raster ?? source.raster ?? source.natural`. 2 taken: `forgetFlight` and `resetFlights` clear `flightLook` (iOS and Mac). Tests taken: the arriver's own raster is delivered and drawn at its own aspect with the stand let go; a flight ended early drops the look. DEFERRED: a tinted image's `draw(_:)` path still needs its own raster; an animated leaver's stand shows its first frame.

---

**READY WITH CHANGES.** Two issues need small corrections:

1. **Interrupted flights can pair a raster with the wrong natural size.** [FlightsIOS.swift:74](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:74) and [FlightsMac.swift:73](/tmp/x20/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:73) prefer the original flight source’s dimensions, while the new assignment prefers the current leaver’s raster. After a thumbnail→full-image replacement, an interruption can therefore carry the full image with thumbnail dimensions. While the next arriver is unloaded, its fallback fit uses those mismatched dimensions. Derive `natural` from the selected `source.raster`; retain the captured `fit` for the starting rectangle.

2. **Destruction/reset does not explicitly release `FlightLook.stand`.** [FlightsIOS.swift:145](/tmp/x20/host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:145) and [FlightsMac.swift:128](/tmp/x20/host/apple/Sources/ExactKit/Mac/FlightsMac.swift:128), plus both `resetFlights` implementations, remove the flight without clearing the view’s look. `NodeView.forget()` also leaves it intact. If another owner retains the removed view, the lease stays pinned after the flight ends. Clear `flightLook` during these teardown paths. Normal landing already does this correctly; I found no reference cycle or accumulating per-frame copies.

The remaining checks:

- **AnimatedRasters:** safe with no own raster—the identity guard returns nil. The stand displays the raster’s **first animation frame**, not the leaver’s currently displayed animated frame.
- **iOS outside flights:** unchanged, provided `flightLook` is nil.
- **Mac drawing fallback:** ordinary untinted flights take the layer branch even when the box requires drawing. Tinted images still bypass it, and both platforms’ `draw(_:)` paths require `raster`; their loading gap remains. This is an uncovered existing case.
- **Different assets:** showing thumbnail pixels until the full image arrives is reasonable. Same-aspect `contain`/`cover` geometry remains consistent; different aspect ratios or `none`/`scale-down` can visibly change fit upon replacement. If loading outlasts the flight, landing still removes the stand.
- **Test:** [FlightsIOSTests.swift:146](/tmp/x20/host/apple/tests/ExactKitTests/FlightsIOSTests.swift:146) is a valid regression test for initial and mid-flight visibility. It never delivers the arriver’s raster or lands the flight. Extend it to check replacement, interruption, and weak-lease release while retaining the removed view.

Reviewed against `HEAD` **86b3d1818**; local `origin/main` has advanced. `git diff --check` passes. No files changed or tests rerun under the read-only restriction.
