# Code review r2: UIKit's zoom lands photo on photo, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `e650c1aba59bbe9acff3e2ae56ac140f569876af93fbca700f12ec3ba164f237`), shared with astra. Round 2, a delta review of the round-1 dispositions, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY (P1 finding, no explicit verdict line).
- **Disposition (r3):** Taken: `prepare` samples the landing (the same resolution the provider runs) while the route's nodes are still mapped, just before `freeze` and `release`, so the zoom back starts from the photo as it is then. DEFERRED: a presentation-level regression (open, transform, Close through route destruction, read the dismissal answer); it needs a session, a navigation projection and a live UIKit zoom in the test host.

---

The Close-path cache is the opening rect, so a zoom or pan before Close zooms back to the wrong place. The other round-1 fixes hold.

[P1] Zoom-back reuses the opening alignment rect — `host/apple/Sources/ExactKit/IOS/ModalIOS.swift:330`

`alignmentRectProvider` writes `landing.rect` only on a call that still finds the source id in `presenter.views` (line 335). UIKit asks at the start of the zoom, then again when the dismissal starts. A pinch, pan, or rotation while the viewer is up changes that element's transform or frame (`applyTransform` in `PressFeedback.swift`) and does not ask again. Close then destroys the route: `modals.prepare` runs, `Presenter.release` drops the nodes from `presenter.views`, and only afterwards does `drainRetired` start the UIKit dismissal. The lookup fails and the provider returns the opening rect. The snapshot taken in `prepare` (`freeze`) shows the zoomed photo. Sample the rect in `prepare`, while the node is still mapped and its raster is still set, and return that from the provider once the nodes have left the map.

The rest of the delta matches the round-1 fixes. Alpha is restored when `animate(alongsideTransition:completion:)` returns false or there is no coordinator, again in that animation and its completion, and unconditionally without animation in the `present` completion. `zoomAlignment` fits and clips in `contentBox()`, which is the same inset the image is drawn with. `zoomImage` walks `container` depth-first. The fallback natural size comes from `context.sourceView`. A nil `viewIfLoaded` is not a live failure: `controller.view` is loaded before `present`, and on dismissal the presented controller's view is still loaded, so the decisive calls convert into a real view. `ZoomAlignmentIOSTests` covers padding, border, and a nested image only; it never installs the provider or runs a retiring dismissal, so it does not catch this cache.
