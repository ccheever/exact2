# Code review r3: UIKit's zoom lands photo on photo, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `36c4e5cc4c435aa2d076746de0cba50f5afec3b79dcd517bbd5cccbe7b91dab3`), shared with astra. Round 3 (the last), a delta review of the round-2 disposition, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. DEFERRED, as stated: a presentation-level regression test (open, transform, Close, read the dismissal answer) and device validation of the first-frame guard.

---

No findings.

**READY** — `host/apple/Sources/ExactKit/IOS/ModalIOS.swift:209`

The Close path now samples the photo as it is shown, and the three checks hold.

`prepare` calls `sampleLanding` at line 209, before `freeze()` and before this batch's `.destroy` ops reach `Presenter.release`. A pinch or pan is already on the view: each sample is applied in its own `session.apply` (`TransformDragHold.move`), which sets `translate` and `scale` through `.present` and returns before the later Close batch exists. `zoomAlignment` then `convert`s that transform into the zoomed controller's view. Frame and present ops in the Close batch run after this sample, and also after `freeze()`, so the cached rect matches the snapshot UIKit dismisses. `applyGeometry` clears a transform only while setting a frame, then `applyTransform()` puts it back, under that snapshot.

`sampleLanding` (`:346`) and `alignmentRectProvider` (`:340`) share one `resolve` and one `ZoomLanding` class instance, so the write in `prepare` is the rect the provider returns once the lookup fails. The closure captures `[weak controller]`; `resolve` captures `[weak self, weak route]`. The provider keeps the transition, and the transition does not keep the presentation. Nothing in that graph retains `Presentation` or `ModalHost`.

An interactive swipe never takes this path. The route stays in `presenter.views` until `presentationControllerDidDismiss`, which only then starts the destroy batch, after UIKit has already asked for the rect. While the nodes are mapped, `resolve` succeeds and the provider returns that live rect, updating the cache as a side effect. `sampleLanding` does not run, and a later sample cannot replace an answer UIKit already used.

Residual risk, as deferred: `ZoomAlignmentIOSTests` still exercises only `zoomAlignment` and `zoomImage`. Nothing opens a zoom, changes the transform, destroys the route, and reads the dismissal answer. I did not re-run the 277 tests.
