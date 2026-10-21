# Code review r3: UIKit's zoom lands photo on photo, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `36c4e5cc4c435aa2d076746de0cba50f5afec3b79dcd517bbd5cccbe7b91dab3`), shared with grok. Round 3 (the last), a delta review of the round-2 disposition, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. DEFERRED, as stated: a presentation-level regression test (open, transform, Close, read the dismissal answer) and device validation of the first-frame guard.

---

**READY — no new defects found in the round-3 delta.**

- **Close captures the current geometry:** [ModalIOS.swift:209](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:209) samples immediately before `freeze()`. [PresenterIOS.swift:717](/tmp/x20/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:717) runs this before processing the closing batch’s geometry and destroys. Previously applied zoom/pan transforms are included; the sample matches the state being frozen.
- **No new retain cycle:** [ModalIOS.swift:330](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:330) captures the host and route weakly; `sampleLanding` captures the controller weakly. The shared cache holds only a rectangle.
- **Interactive dismissal remains live:** [ModalIOS.swift:340](/tmp/x20/host/apple/Sources/ExactKit/IOS/ModalIOS.swift:340) resolves on every provider call while the target remains mapped. After Close removes it, the provider returns the newly sampled cache.

`git diff --check` passed. This was a read-only inspection; I did not rerun the reported 277 passing XCTests. The deferred presentation-level regression remains a coverage gap, but I found no remaining defect requiring another change.
