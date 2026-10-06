# Code review r2: a flight's arriver draws the leaver's image until its own lands, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `9f9276ddea7c9087a3d3b318d1d6ac8092bb379d65e63f17fbd3c829b4303e86`), shared with astra. Round 2, a delta review of the round-1 dispositions, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. The deferred cases (a tinted image's `draw(_:)` path, an animated leaver's first frame, no test locking the interrupted path) stay DEFERRED.

---

**READY.** No findings.

The round-1 fixes hold on both hosts. An interrupted flight keeps the raster it will draw (`leaver.raster ?? look.stand`) and takes `natural` from that same image, and `showFlight` sizes the fit from `view.raster`, then that raster, then `source.natural`. While the stand is what is on screen, those are the same image, so the end rectangle matches the pixels. The captured `fit` is still the on-screen rectangle, so the next flight starts where the previous one was. `forgetFlight` and `resetFlights` clear `flightLook` before the `Flight` is dropped, so a view held past the flight no longer retains the lease. `NativeRasterLease` is a class; `CALayer.contents` retains the `CGImage`, not the lease.

`testAnArriverDrawsTheLeaversImageUntilItsOwnLands` now separates the two aspects: a 200×150 stand contained in 400×800 would be 300 pt tall, and the assertion requires 100 pt after the 400×100 raster is delivered. The early-end test keeps the arriver alive across `destroy` and checks the look is gone.

Still the deferred cases: a tinted image only paints in `draw(_:)`, which still requires the arriver's own raster (`NodeViewIOS.swift:1366`, `NodeViewMac.swift:1317`), and an animated leaver's stand is `RasterImage.image`, the first frame (`AnimatedRasters.frame(for:)` returns nil unless the player’s image is that view’s own raster). The interrupt path is the same pairing and is not locked by a test. This pass did not re-run the suite.
