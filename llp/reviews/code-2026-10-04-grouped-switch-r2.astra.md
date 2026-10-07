# Code review: a grouped-list switch that "does not flip", round 2 (4cc252edb), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `4cc252edb`.
- **Method:** one brief (sha256 `74eee858ffbf59e27ce7bead1e52efa374a2203fff8c2e317b54b6b96f73a5bf`), shared with grok. Round 2, blind to the other round-2 review; it read the round-1 artifacts. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition:** nothing to take. The last round.

---

No blocker, should-fix, or nit findings. The round-1 findings are correctly addressed.

The arithmetic in [GroupedListIOSTests.swift:132](/tmp/x15-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:132) is correct. The presenter assigns frame coordinates directly relative to each superview. Section 2 and column 4 contribute zero offsets; row 12 contributes `at.y − 26`; control 13 contributes `at.x − 31.5`. `ControlHost.sync()` centers the native switch in control 13’s unpadded `63 × 52` box, placing its center at `(at.x, at.y)` in scroll coordinates. The window-to-scroll conversion accounts for scroll offsets. Those dimensions describe the fixture’s box, without assuming the native switch’s size.

The [assertions at lines 137–140](/tmp/x15-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:137) establish distinct switches, actual overlap at the tested point, and a hidden authored scroll. They fail if the authored switch or an unrelated view wins the hit, or if no view answers. Accepting accessory descendants accommodates UIKit’s internal views. The frame batch leaves the standard-row model unchanged, so it does not reconfigure the accessory or invalidate the captured midpoint.

The [pitfall wording and commands](/tmp/x15-review/docs/agent-pitfalls.md:177) address both documentation findings. Both review artifacts accurately preserve their round-1 verdicts and describe the round-2 dispositions.

Static review only; no edits or tests run. The test verifies hit-testing and action wiring; touch-recognition evidence comes from the supplied simulator results.

Verdict: LAND