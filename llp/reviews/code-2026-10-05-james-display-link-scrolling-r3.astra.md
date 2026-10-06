# Code review: James's display-link scrolling, round 3 (04af82601..6e476ab9d), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `6e476ab9d`.
- **Method:** one brief (sha256 `ec610dede83602fbbafbced39e201df11c222ae5b6809b00b1818257527cc2c6`), shared with grok. Round 3, blind to the other review. The authors are not reviewers.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken, and it shows round 2's grok 2 was not a defect: the additive value is composed before the model, so a scale about the origin lands inside the rotation, where it commutes with the model's own (s, s, 1) scale. The keyframes are gone; every press uses the one exact ease again, so a re-aim is continuous. `testAPressInSpaceEasesInsideItsRotation` now pins that algebra at touch-down and part way. 2 not taken: the pump's owed-work condition is James's, unchanged by review; both round-3 reviewers find the idle wakes correct on inspection.

---

LAND WITH CHANGES

1. **MINOR — The 3D fix introduces a re-aim discontinuity.** [PressFeedback.swift:164](host/apple/Sources/ExactKit/PressFeedback.swift:164) linearly interpolates 13 sampled matrices, but re-aiming samples the exact Bézier. At 25 ms, a `.5` press has factors `.62209` versus `.61686`: releasing a 200-point box introduces about a one-point jump. **Fix:** use the exact timing function with an additive scale about the origin; QuartzCore matrix checks confirm that composition works for 3D rotations too. Test intermediate presentation and re-aim continuity—the current test checks endpoints.

2. **MINOR — Idle-pump wakeups remain unprotected by tests.** [ScrollPumpIOSTests.swift:49](host/apple/tests/ExactKitTests/ScrollPumpIOSTests.swift:49) explicitly scrolls after invalidation. It cannot catch lost wakeups from text becoming stale or a fill landing after scrolling stops. **Fix:** let the pump become idle, independently trigger text invalidation and landed-fill retirement, and assert completion without another scroll.

The remaining prior fixes and idle-tick dispositions hold on inspection. No additional concrete defect found in clock ordering, rate voting, pausing, background or frozen-agent paths. Changed sources meet the 1,500-line limit; `git diff --check` passes. UIKit tests were not run in this read-only checkout.