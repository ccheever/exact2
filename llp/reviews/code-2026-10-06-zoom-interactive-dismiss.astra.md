# Code review: UIKit's zoom interactive dismissal lands unaligned (LLP 1035.001, the zoom's alignment), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x2zoom-rv`.
- **Method:** one brief (sha256 `0333dd0135c70ca0f5f5d3e4d427608dc0aee9e8d34bdd89ecaf95662e795504`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit 10fe88442 in a detached worktree. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition (r2):** 1 taken in part: the flag's transitions are `ZoomLanding.dismissal(begins:)` and `appeared()`, the closures pass straight through, and the test drives refusal, acceptance, cancellation then Close, and a second drag, with two rects. The live wiring (UIKit's own interactive dismissal and its cancellation) cannot be driven from a unit test; it is verified by real-touch simulator drives (the recordings in the commit message). 2: proposed LLP 1035.001 text, sent for Charlie's approval; not landed with the code.

---

READY WITH CHANGES

1. **[should-fix] Test the lifecycle wiring, not just the Boolean.** [ZoomAlignmentIOSTests.swift:82](/tmp/x2zoom-rv/host/apple/tests/ExactKitTests/ZoomAlignmentIOSTests.swift:82). This test still passes if either `interactiveDismissShouldBegin`’s assignment or `controller.appeared` is removed. Add a native cancellation → programmatic Close case and cancellation → second interactive dismissal case, observing the actual alignment answers. Include a drag interrupting the opening zoom: the relative ordering of `shouldBegin`, the opening `viewDidAppear`, and the alignment query is not established by this test. Also use two different rectangles to verify that resolving a new rect while interactive preserves it for the next aligned answer.

2. **[nit] Document the interactive exception in the governing design.** [1035.001-native-interaction-ownership.rfc.md:587](/tmp/x2zoom-rv/llp/1035.001-native-interaction-ownership.rfc.md:587). This paragraph still describes photo-to-photo alignment without distinguishing interactive dismissal. Record the intentional whole-route cross-fade during interactive dismissal and restoration after cancellation; no Contract change is needed.

I found no demonstrated implementation defect in the ordinary lifecycle:

- **Completed, cancelled and repeated dismissals:** each presentation creates its own `ZoomLanding`. Completion can leave the retiring controller’s flag true; a replacement presentation gets fresh state. Cancellation resets before the backdrop-recognizer early return, and the next accepted dismissal sets it again.
- **Refused attempts and reentrancy:** `willBegin == false` and permission refusal leave the flag untouched. Repeated accepted callbacks are idempotent. The changed closures do not synchronously dispatch app actions.
- **Replacement and `sampleLanding`:** old callbacks capture only the old landing. Sampling directly into `rect` preserves the latest geometry without overriding interactive suppression. After cancellation resets the flag, a programmatic close can use that sampled rectangle.
- **Retention:** `controller → appeared → landing` has no return edge. `landing` contains only value state; the resolver captures host/route weakly, and sampling captures the controller weakly.

`viewDidAppear` is a defensible cancellation-completion signal here. Apple explicitly recommends appearance callbacks for clearing temporary zoom-transition state and describes cancelled transitions returning through appearance at completion. A coordinator is therefore **not inherently better**. If used, register a completion on the actual dismissal coordinator and check cancellation and transition identity; do not reset in `notifyWhenInteractionChanges`, which can fire while settling continues. [Apple’s zoom lifecycle guidance](https://developer.apple.com/videos/play/wwdc2024/10145/?time=255)

Returning **nil is the appropriate lever**: the SDK defines the underlying null rectangle as “no preference.” A different concrete rectangle still requests alignment and lacks the supporting evidence supplied for nil.

This was a static review against the code, SDK headers and Apple guidance. I did not run XCTest or reproduce the recordings in the read-only environment.
