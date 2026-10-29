# Code review: two iOS reds as test drift (main red, quick review) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `054ca690dc`.
- **Method:** one brief (sha256 `94920b4b8e6796dd561127bc563f5a8d083597fa01f9f7ce1c731495907568d5`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.

## Brief

Quick review (mechanical main-red fix, test-only) of ONE commit, `git show HEAD` (054ca690dc) in this worktree: two iOS XCTest classes red on main since #1141/#1144. Read-only: no checkout, restore, stash, reset or edits. Compare rt1's accepted Mac twin of this kind, f6eb28a492 (AsyncAdvanceTests' deliverTask pattern).

The commit message explains each change. Check:
- DueBeforeFrameIOSTests: that the red was drift (since #1144, NativeTaskTimers owns an ordinary task's timer once session.clock is nil: Session.swift `clock` didSet, NativeTaskTimers.swift update/resume/deliverNativeTimer), and that the rewritten cases still prove the class's point (LLP 1072.000 §4 B, the doc comment): a wake already due when a batch lands is POSTED to the owner (advanceInFlight), main does not wait for it, and its commit lands with a later turn (drainFill); the batch's own change applies first. Is firing the Foundation timer by hand (timerForTests(...).fire()) and calling deliverNativeTimer() a faithful stand-in for the run loop, as in f6eb28a492? Does anything vacuously pass (e.g. asserting advanceInFlight false before delivery)? Is `due` returned from armedAndDue still right for the driver-owned case?
- AgentCheckIOSTests.testFrameHealthControlsAreExcludedFromAppOcclusionFindings: that the fixed point (70, 42) could never hit the card on a phone (FrameHealthOverlay.layoutSubviews places it at the safe area + 8; hitTest only answers on handle/details/close), and that moving node 1 under the drag handle by a `frame` op keeps the test's meaning: the host's card is excluded from `cover` findings (AgentCheckIOS.swift ~195, the `exact.host.frame-health` identifier prefix) and an ordinary covering view still is. Coordinates: the viewport's frame is the window's bounds.
- nothing else changed; test-only.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings.

- [DueBeforeFrameIOSTests.swift:59](/tmp/iosfix2-rv-red-astra/host/apple/tests/ExactKitTests/DueBeforeFrameIOSTests.swift:59): Confirmed #1144 timer-ownership drift. Manual `fire()` followed by `deliverNativeTimer()` faithfully exercises the callback and delivery paths, matching f6eb28a492. The assertions cover a queued wake, post-delivery `advanceInFlight`, unchanged presentation before drain, and “Flipped” afterward. The batch’s own change applies first. The driver-owned branch still returns the correct runtime `due`.
- [AgentCheckIOSTests.swift:54](/tmp/iosfix2-rv-red-astra/host/apple/tests/ExactKitTests/AgentCheckIOSTests.swift:54): The handle-derived point fixes the safe-area miss. Window and viewport coordinates align in this fixture, so the frame operation centers node 1 beneath the handle. The hit assertion and identifier-change control preserve meaningful coverage of both host-overlay exclusion and ordinary occlusion.

Only the two test files changed. Read-only review; XCTests were not rerun.

VERDICT: CLEAN
