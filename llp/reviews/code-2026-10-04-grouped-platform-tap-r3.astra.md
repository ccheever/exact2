# Code review: a real touch aims at what a grouped list draws, round 3 (5da0e24df), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `5da0e24df`.
- **Method:** one brief (sha256 `5603e9a78a5b04eab1b80f684cf0a54b2dbef00fc2d87cb6f668ebce00696398`), shared with grok. Round 3, the last; blind to the other round-3 review; it read the earlier artifacts. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** 1 DEFERRED, recorded in LLP 1084 §7. A `when` replacing a row's control between the aim and the touch leaves the same switch under the finger, and the touch flips the row's current control, as a person's would. The cost is a reply that names the old id. Carrying the target id needs the host's model in `TouchLog.landing`, which is a static walk of the view today. This was the last round, so the change waits for its own review.

---

1. **Should-fix — Replacing an accessory’s target within the same row still passes delivery verification.** [GroupedListIOS.swift:169](/tmp/x15p-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:169), [touches.mjs:351](/tmp/x15p-review/host/apple/touches.mjs:351)

   `{row, part}` distinguishes neighboring rows, but omits the accessory’s authored control ID. If row 12 replaces control 13 with control 14 between the final aim and touch-down, both records remain `{row: 12, part: "switch"}`. The list node also matches, so `realTap` reports successful delivery for control 13 although the switch now represents control 14. This replacement is explicitly supported by [GroupedListIOSTests.swift:214](/tmp/x15p-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:214).

   **Fix:** Include the current authored target ID in projected accessory identity and compare it at touch-down. Extend the existing replacement test to assert that this identity changes. Round 2’s identity finding is therefore only partially closed.

The specified cross-row reuse case is closed: `configure` updates `cell.row`, which landing reads. No static false-refusal issue found for unchanged custom rows or private switch/accessory subviews; both paths classify the hit through the same ancestry. The tvOS branch avoids `UISwitch`.

The wording correction and LLP note accurately describe the implemented row/part check. The race test is explicitly deferred. No builds or tests run; files unchanged.

Verdict: LAND WITH FIXES