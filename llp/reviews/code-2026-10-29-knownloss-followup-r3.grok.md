# Code review: the knownLoss follow-up (ResetGate tables, ResetHoldTests), round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `826308a57a`.
- **Method:** one brief (sha256 `fbfa80b153a5b41ebc2f253b12ccc6c81fc9679a232f593957d6453278bcc4a5`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `host/apple/tests/ExactKitTests/ResetHoldTests.swift:15`, `host/apple/Sources/ExactKit/ResetGate.swift:155`, `QUEUE.md:4` — The probes only observe the main queue, common modes, and `.modalPanel` / `.eventTracking`. Those last two are already common modes, so the extra `perform` does not widen coverage. A nested loop in any other mode would run none of this work, and the test would stay green. The comments say the teardown runs no nested run loop and that any nested loop fails the test. That is stronger than the probes. It does not weaken the known-loss claim: the same private mode would not deliver main-queue arrivals either, and that is how these losses are scheduled. Narrow the sentences to a loop in the default, modal-panel, or event-tracking mode, and drop "runs no nested run loop."

2. `host/apple/tests/ExactKitTests/ResetHoldTests.swift:115` — `testTwoWindowsRestartedTogetherTurnNoRunLoopWhileHeld` spins the default mode for a full second before it checks that both sheets are gone, and it never checks that `endSheet` took over 0.15 s or that the probes ran afterwards. `SheetsMac.end` calls `orderOut` after `endSheet`, and `RouteSheetPushMacTests` already has to spin the default mode to finish clearing `attachedSheet`. This test can pass with the sheets still up when `apply` returns, and with the probes never armed. Sample `attachedSheet`, visibility, and duration before that spin, and assert `ran` is non-empty after it, as the one-window test does. On iOS, `after` at line 66 is only interpolated into the failure string, so a restart that leaves the sheet up still passes; assert it is false there too.
