# Code review: a grouped-list switch that "does not flip" (d82c5dd55), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `d82c5dd55`.
- **Method:** one brief (sha256 `8fdf2ae19f73806d768314bee7671cb905d189c6a706e5c0f8531c1fa1bc0c65`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition (r2):** 1 taken, and taken further on grok's finding 1: the test now lays the hidden row's control out under the accessory, asserts the two overlap, and that the hit still reaches the accessory.

---

1. **nit — [GroupedListIOSTests.swift:127](/tmp/x15-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:127): clarify the coverage claim.** The fixture creates and mounts a separate ControlHost UISwitch for node 13, but only nodes 1 and 21 receive frames (lines 45–54). The authored toggle therefore does **not** overlap the accessory at its middle. The assertion correctly converts coordinates and accepts UIKit’s private switch descendants; it would catch an intercepting view or disabled interaction along the exercised hierarchy. It does not specifically exercise overlapping switches, navigation-container ranks, or touch recognition. **Fix:** narrow the comment to “The accessory’s center is hit-testable through the window,” or add overlapping authored-control geometry if that specific regression needs coverage.

No blocker or should-fix findings. Given the supplied runtime evidence, the diagnosis is sound:

- The bare UIKit reproduction removes exact2 entirely; successful physical taps on the same accessory, with corresponding state changes, establish that its wiring works.
- [GroupedListIOS.swift:335](/tmp/x15-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:335) hides the authored scroll. The accessory is a separate UISwitch with its own action, created at [line 429](/tmp/x15-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:429). Finding ControlHost targets on the hidden controls is expected.
- [NodeViewIOS.swift:743](/tmp/x15-review/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:743) rejects hidden or interaction-disabled branches; [PaintOrder.swift:141](/tmp/x15-review/host/apple/Sources/ExactKit/PaintOrder.swift:141) orders hits by `zPosition`. Nothing here implicates 7a2ecf2ef in the reported failure. The grouped collection adds no custom touch-delay behavior, and the bare-switch failure shows collection-view delay is not necessary to reproduce it.
- Agent activation directly calls `setOn` and `sendActions` ([GroupedListIOS.swift:483](/tmp/x15-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:483)); its success alone would not establish working touch recognition.

The [pitfall entry](/tmp/x15-review/docs/agent-pitfalls.md:177) is accurate against the supplied evidence and follows the surrounding symptom → cause → fix → provenance style. AXe’s [tap implementation](https://github.com/cameroncooke/AXe/blob/main/Sources/AXe/Commands/Tap.swift) also confirms that automatic coordinate taps select simulator delivery, while recognized switches select physical delivery.

Static review only; no files changed or tests run.

Verdict: LAND