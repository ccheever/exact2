# Code review r2: iOS, a fling no longer pays the presenter's whole pass per batch (5b33306bc..434e5ea32), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `434e5ea32`.
- **Method:** one brief (sha256 `1e769677616c0749d24d9985b2434376ac9d5b8a5755adbd652d4e0e0c70ec6d`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken: a skipped scroll batch positions the context previews (`positionContexts`, now internal on both presenters), the one step of the pass a scroll moves. A scroller with no `scroll=` handler never sent a batch, so its previews did not follow before either; a test with a live preview is DEFERRED (it needs a session with a context target in a scroller). 2 taken: the test removes the gradient and puts it back, asserts a new layer, clears it, and checks the re-aim. 3 taken: an ordinary empty `apply` is asserted to reach the presenter.

---

LAND WITH CHANGES

1. **MATERIAL — Empty scroll batches leave context previews stale.** The shared [scroll callback](host/apple/Sources/ExactKit/Session.swift:734) now skips positioning. Concrete case: a `contextTarget` preview outside its source’s scroller, with a no-op `scroll=` handler. On macOS, scrolling the source 40 points leaves the preview behind: [positioning](host/apple/Sources/ExactKit/Mac/PresenterMac.swift:1133) runs only from full apply. iOS likewise loses its [source-position compensation](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1226). **Fix:** refresh context positioning from scroll callbacks; test an empty scroll batch with a visible preview.

2. **MINOR — The “layer made again” test never replaces the layer.** [FixedGradientIOSTests.swift:94](host/apple/tests/ExactKitTests/FixedGradientIOSTests.swift:94) toggles fixed attachment, but this plain gradient remains layered and [reuses `boxGradient`](host/apple/Sources/ExactKit/IOS/BoxLayerIOS.swift:135). The identity guard could be wrong without failing this case. **Fix:** actually replace the layer, assert identity changed, and verify re-aiming with the previous cache populated.

3. **MINOR — The corrected skip boundary lacks a regression assertion.** [IdleTickTests.swift:41](host/apple/tests/ExactKitTests/IdleTickTests.swift:41) calls the helper directly. Restoring the round‑1 global skip would still pass these assertions. **Fix:** assert ordinary empty `apply` still presents, and cover native selection rejection or agent-clock animation seeking.

Earlier clock, selection, appearance, GPU-drain and projection-order fixes remain present. Changed sources satisfy the 1,500-line cap; `git diff --check` passes. Static review only; runtime tests were not run in this read-only checkout.