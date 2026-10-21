# Code review: UIKit's zoom lands photo on photo (alignmentRectProvider) and the first-frame guard, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `bf88d8d9c401cbbab1ca379d0134aff9ef150a25d0c9d41acc85f11a1f4bf007`), shared with grok. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at `6fe5a33d4`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition (r2):** 1 taken: the alongside registration's result is checked (alpha restored at once if refused) and the present completion restores alpha unconditionally, without animation. 2 taken: the provider keeps its last answer and returns it when the route's nodes are gone (the zoom back after a Close). 3 taken: fitted and clipped in `contentBox()`. 4 taken: `zoomImage` searches authored descendants through `container`, depth first; the fallback natural size comes from `context.sourceView`. Tests: padding/border and a nested image added; DEFERRED: a presentation-level test (provider installed, coordinator refusal, retiring dismissal) and device validation of the first-frame mitigation. LLP addendum rewritten to separate the observation, the reading and the mitigation, with device validation outstanding.

---

**Verdict: NOT READY.** Reviewed the uncommitted diff and new test against `6fe5a33d4`. Four defects need changes:

1. **[P1] Handle rejected coordinator registration — [ModalIOS.swift:364](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:364).**  
   `animate(alongsideTransition:)` can return `false`; its completion **may** still run, but is not a guaranteed fallback. Ignoring that result leaves a path where alpha remains zero, potentially through the animation or indefinitely. The separate `present` completion never restores it. Restore immediately when registration fails, and add unconditional, nonanimated restoration to presentation completion or appearance cleanup. [Apple’s coordinator documentation](https://developer.apple.com/documentation/uikit/uiviewcontrollertransitioncoordinator/animate%28alongsidetransition%3Acompletion%3A%29).

2. **[P2] Explicit Close loses dismissal alignment — [ModalIOS.swift:313](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:313).**  
   When Close destroys the route, `modals.prepare` preserves its outgoing presentation, but `Presenter.release` removes its nodes from `presenter.views` before `drainRetired()` starts UIKit dismissal. This provider consequently returns `nil`, reverting to whole-controller alignment on zoom-out. Weak captures are not the problem: the retained hierarchy survives, but this lookup cannot find it. Preserve alignment for the retiring presentation before teardown; merely retaining the image node is insufficient because `NodeView.forget()` also clears its raster.

3. **[P2] Image alignment uses the wrong box — [ModalIOS.swift:272](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:272).**  
   Both image-rendering paths apply `object-fit` within the **content box**, subtracting border and padding, and clip there. This helper fits and clips against `image.bounds`. Any padded or bordered destination therefore gets a different alignment rectangle from its drawn photo. Use `image.contentBox()` for both fitting and intersection.

4. **[P2] Image-child lookup misses native containers — [ModalIOS.swift:262](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:262).**  
   Authored children live in `node.container`, which can be a scroll view, clipping wrapper, or material content view. Searching `node.subviews` misses the first image child in those cases and aligns to the holder instead. Use the authored-child container; this also affects source natural-size fallback.

The coordinate conversion itself is correct: converting the fitted image rectangle into `context.zoomedViewController.view` matches the SDK’s required coordinate space. Resolving inside the provider supports fresh geometry on each transition. Route scoping correctly separates the destination from its preceding source, and the weak captures avoid retention cycles. Own-raster natural size takes precedence correctly. For fallback, prefer `context.sourceView`: the current unsorted `.first` lookup does not necessarily select the same source as the source provider’s mounted, minimum-ID lookup.

Once alpha restoration executes without animation, I see no additional alpha defect specific to cancelled interactive dismissal, `isModalInPresentation`, or later dismissal. This implementation creates a fresh modal controller for each presentation, so it does not reuse a previously guarded controller. **Portal opacity and first-frame timing remain unverified**: public API guarantees do not establish whether UIKit samples or mirrors that opacity before the alongside callback. Normal simulator rendering does not resolve the device-only concern.

The [new test:20](/tmp/x20/host/apple/tests/ExactKitTests/ZoomAlignmentIOSTests.swift:20) is a useful geometry test, but never installs or invokes the provider or presents a controller. Add coverage for retiring-route dismissal, border/padding, container-held image children, and rejected coordinator registration. Use differing source/destination aspect ratios to verify fallback precedence.

The [LLP addendum:595](/tmp/x20/llp/1035.001-native-interaction-ownership.rfc.md:595) overstates “hidden until the transition’s first frame.” Describe the observed flash, the suspected early commit, and the proposed mitigation separately, explicitly retaining physical-device validation as outstanding.

`git diff --check` passed. UIKit tests and device verification were not run in this read-only environment.
