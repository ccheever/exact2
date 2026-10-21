# Code review r3: iOS, a fling no longer pays the presenter's whole pass per batch (5b33306bc..8fca848db), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `8fca848db`.
- **Method:** one brief (sha256 `66f443c00ba867d9abc1d39403931a35fa4fe65ce35016915cdf515194259ce1`), shared with grok. Round 3, the last, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (landed):** 1 taken: a skipped scroll batch calls `scrolledWithoutPass`, which on iOS positions the context previews and then re-aims the fixed gradients, in the pass's order (macOS has no fixed-gradient layers). A test of that combination is DEFERRED with round 2's live-preview test. 2 taken: the tablist test squeezes the control to 1×1 before each trait change and asserts it was sized again (180 pt floor, a real height). 3 taken: `ControlHost.reset` clears the faces.

---

LAND WITH CHANGES

1. **MATERIAL — Context positioning leaves fixed gradients aimed at stale geometry.** [NodeViewIOS.swift:641](host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:641) re-aims gradients before delivering the scroll event. The skipped path then calls [positionContexts](host/apple/Sources/ExactKit/Session.swift:948), which translates source contents at [PresenterIOS.swift:1227](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1227). Concrete case: an open preview, a fixed-gradient bubble in its source contents, and a programmatic scroll with a no-op handler. The compensation moves the bubble after its gradient was aimed, leaving incorrect colours. Full apply re-aimed afterward at [line 741](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:741). **Fix:** re-aim after context positioning; test this combination through `applyUnlessEmpty(..., scrolled: true)`.

2. **MINOR — The segmented-title regression checks bookkeeping, not sizing.** [NavigationBarIOSTests.swift:178](host/apple/tests/ExactKitTests/NavigationBarIOSTests.swift:178) only checks `sized.traits`. Removing both sizing statements from [NavigationBarIOS.swift:281](host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:281) would leave it green. **Fix:** use long titles and compare actual dimensions against a freshly sized control after Dynamic Type and Bold Text changes, without another batch.

3. **MINOR — The face cache survives reset.** [ControlsIOS.swift:268](host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:268) clears controls but retains `faces`. Subsequent cleanup iterates `controls.keys` ([line 134](host/apple/Sources/ExactKit/IOS/ControlsIOS.swift:134)), so retired faces can remain retained after restarting into a button-free tree. **Fix:** clear `faces` in `reset()`.

Earlier clock, selection, appearance, GPU-drain and projection-order fixes remain intact. Changed sources satisfy the 1,500-line cap; `git diff --check` passes. Static review only; runtime tests were not run in this read-only checkout.