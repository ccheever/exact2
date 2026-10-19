# Code review: LLP 1121 stage 9 G3, a pointer's :hover look on iOS, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 3: high), sandbox read-only, a throwaway worktree.
- **Scope:** the iOS hover-look commit.
- **Method:** one brief (sha256 `bec521430f16d0e42179248119da196a3fb3c214fc01d3474076f56606912bab`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (rows unrecyclable; a hold on a node with no look never let go; a cancel taken as leaving; the test bypassed the callback; a dead tvOS test).
- **Disposition:** fixed in round 1's commit.

---

Nothing blocking found in this read-only review of `6171c52`. No builds or tests were run.

1. **Should-fix — Hover styling disables row recycling.** [LookHoverIOS.swift:27](/tmp/rv-g3-1/host/apple/Sources/ExactKit/IOS/LookHoverIOS.swift:27)  
   `NodePool.recyclable` accepts only idle pointer recognizers and recognized context-menu recognizers. Adding this recognizer therefore makes an otherwise recyclable row’s entire subtree ineligible—even on an iPhone without hover input. Scrolling now destroys and recreates those views. **Fix:** admit the owned hover recognizer when idle, with explicit cleanup/reset during parking and rebinding.

2. **Should-fix — A hover hold without a hover look never releases on pointer crossing.** [PressPaint.swift:289](/tmp/rv-g3-1/host/apple/Sources/ExactKit/PressPaint.swift:289), [HeldState.swift:169](/tmp/rv-g3-1/host/apple/Sources/ExactKit/HeldState.swift:169)  
   Holding hover on the fixture’s plain node 9 succeeds, but that node has no look recognizer. Moving the pointer onto or off it cannot reach `HeldState.lookHovered`; the existing `hover` handler path does not notify HeldState either. Mac explicitly supports this case. **Fix:** track pointer occupancy for held nodes independently of whether their styles contain a hover look, including initialization of `over`.

3. **Should-fix — Programmatic cancellation is reported as real input.** [LookHoverIOS.swift:39](/tmp/rv-g3-1/host/apple/Sources/ExactKit/IOS/LookHoverIOS.swift:39)  
   Hold hover while the pointer is inside, then enable Inspect. [InspectIOS.swift:69](/tmp/rv-g3-1/host/apple/Sources/ExactKit/IOS/InspectIOS.swift:69) disables and reenables every recognizer. UIKit documents that disabling an active recognizer cancels it; this callback consequently drops the hold with `"input"` despite no pointer crossing. [UIKit cancellation behavior](https://developer.apple.com/documentation/uikit/uigesturerecognizer/isenabled). **Fix:** separate lifecycle cancellation from genuine pointer entry/exit; clear transient paint without incorrectly ending the hold, and reconcile occupancy when tracking resumes.

4. **Should-fix — The new test bypasses the feature’s event path.** [HeldStateTests.swift:740](/tmp/rv-g3-1/host/apple/tests/ExactKitTests/HeldStateTests.swift:740)  
   The test calls `HeldState.lookHovered` and `lookState` directly. It would still pass if `lookHovering` were empty or incorrectly wired, and its final enter does not verify that real hover paint remains after releasing the hold. **Fix:** exercise the installed recognizer’s callback through begin/change/end/cancel, verify the resulting paint and hold state, and cover removal/reinstallation plus a node without a look.

5. **Nit — The tvOS test cannot compile into any test run.** [HeldStateTests.swift:752](/tmp/rv-g3-1/host/apple/tests/ExactKitTests/HeldStateTests.swift:752)  
   The file’s outer guard permits only iOS and macOS. Inside the non-macOS branch, the new non-iOS branch is therefore unreachable. **Fix:** put this assertion in a tvOS-enabled UIKit fixture or update the file’s guards and platform-specific setup consistently.
