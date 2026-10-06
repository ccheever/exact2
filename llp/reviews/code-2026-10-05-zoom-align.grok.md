# Code review: UIKit's zoom lands photo on photo (alignmentRectProvider) and the first-frame guard, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x20`, plain output.
- **Method:** one brief (sha256 `bf88d8d9c401cbbab1ca379d0134aff9ef150a25d0c9d41acc85f11a1f4bf007`), shared with astra. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at `6fe5a33d4`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition (r2):** Taken: fitted and clipped in `contentBox()`, with a padding/border test and the addendum sentence fixed. (Also taken from astra: the coordinator result, the cached answer for the zoom back, descendant search, `context.sourceView` fallback.)

---

The alignment rect is the one defect that should be fixed before this lands. The alpha guard, the id lookup, and the coordinate space of `alignmentRectProvider` hold up.

**[P2] The alignment rect is the border box, not the image as drawn — `host/apple/Sources/ExactKit/IOS/ModalIOS.swift:272`**

`zoomAlignment` feeds `image.bounds` to `RasterGeometry.rect` and intersects the result with `image.bounds`. The host paints `object-fit` in the content box, bounds inset by border and padding, and clips to that box (`NodeViewIOS.swift:1372-1386`, `BoxLayerIOS.swift:77-84`, `contentBox()` at `NodeViewIOS.swift:843`). With any border or padding, including default `fill`, the zoom targets a different rectangle than the pixels. `ZoomAlignmentIOSTests` only builds images with no border or padding, so it locks the wrong geometry in. The addendum at `llp/1035.001-native-interaction-ownership.rfc.md:589` says "the image as drawn," which this does not implement. Use `image.contentBox()` as the content rect and intersect that, the same pair `applyImageLayer` uses.

The rest of the change matches the request.

The alpha guard does not leave a presentation invisible on the paths that are actually in this code. `controller.view.alpha = 0` is only set for this zoom (`ModalIOS.swift:333`). It is set back to 1 in three places: the alongside block (`364`), that block's completion whether or not the transition is cancelled (`365`), and immediately when `transitionCoordinator` is nil (`366`), which is the non-animated `ExactEnv.agentFreezes` present. `isModalInPresentation` (`337`) does not touch alpha. Dismissal never takes this branch, and the view is already at alpha 1. Each `present` builds a new `ModalController` (`132`), so a later present does not reuse a view left at 0. `_UIPortalView` has `matchesAlpha` and can mirror opacity, but the alongside block writes model alpha to 1 with animations disabled on the same turn as `present`, before the next commit. A commit inside the zoom's own setup while alpha is still 0 would be one transparent portal frame, not a presentation that stays invisible. The return value of `animate(alongsideTransition:)` is ignored. Apple documents that the completion can still run when that returns false, and the completion sets alpha to 1. That is the backstop.

`alignmentRectProvider` is in the zoomed controller's view coordinates, which is what the header requires (`UIZoomTransitionOptions.h`: a frame in the zoomed view controller's view, `CGRectNull` for no preference). The closure converts into `context.zoomedViewController.viewIfLoaded` (`312`, `318`). UIKit calls it at the start and again on dismissal. Frames for the opening call are already applied: geometry runs before `navigation.sync` (`PresenterIOS.swift:894`, then `938`). Returning nil when no descendant has the id leaves UIKit on the whole route. The route filter is `isDescendant(of:)`, which includes the route itself, so the copy of that id in the preceding route is excluded. Captures are `[weak self, weak route, weak preceding]`. A strong `self` would cycle through the controller that owns the transition. `route` stays alive via `Presentation` while the zoom is up. `preceding` is only the natural-size fallback.

That fallback is the source element's own image (the node, or its first image child) via `raster?.image.naturalSize`, which is the file's oriented size, the same value drawing uses. It is applied with the destination's bounds and `object-fit`. If the destination already has a raster, the source size is ignored. If neither has a size, the result is the element's box.

The test covers the pure function: no size yet, fallback aspect, own raster, image-inside-a-holder, and a non-image box. It does not cover the id lookup, the window check, or border and padding. The addendum otherwise matches the code: no Contract change, id scoped to the presented route, nil meaning the whole route, and alpha held until `animate(alongsideTransition:)`.

**Verdict: READY WITH CHANGES** — fix the content-box rect (and the test and the one LLP sentence) before landing. The alpha guard and the provider's lookup are sound.
