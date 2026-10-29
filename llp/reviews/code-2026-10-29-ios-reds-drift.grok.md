# Code review: two iOS reds as test drift (main red, quick review) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `054ca690dc`.
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

**VERDICT: CLEAN**

The reds are test drift, and the rewrites still prove what those two classes are for. The diff is only `DueBeforeFrameIOSTests.swift` and `AgentCheckIOSTests.swift`.

**Due-before-frame.** Once `session.clock` becomes nil, `Session.swift:384` cancels the driver timers, calls `runtime.liveClock(true)`, and applies `runtime.advance`. An ordinary task (`due-now.contract`: `after(50, flip)`, not a frame task) is then a native row. `timer_execution` reports that row as infinity (`waits.rs:233`), so `timerDue` no longer names it, and `advanceIfDue` (`SessionClock.swift:63`) will not post it. A batch that still carried the old deadline posted an advance that never fired the task.

The two wall-clock cases follow the same stand-in as `AsyncAdvanceTests.deliverTask` (`AsyncAdvanceTests.swift:85`): wait until the row is due without turning the run loop (`usleep`), `timerForTests(row.index).fire()`, then `deliverNativeTimer()`. `Timer.fire()` runs the block in `NativeTaskTimers.swift:55`, which only queues a `Delivery` and `resume()`s. `resume` asks for delivery on the next main turn (`NativeTaskTimers.swift:73`); it does not run the wake. `deliverNativeTimer` (`NativeTaskTimers.swift:100`) is that ask. On iOS `asyncFills` is on, so it sets `advanceInFlight` and posts `fireNativeTimerAsync` instead of applying inline. Calling it on this turn, before the queued `async` runs, is the same substitution the Mac twin uses. The fixture has one row, so `rows.first` is `nextDue`. `fire_native_timer` accepts `cutoff >= nominal`, and `fire()` happens after the wait has already reached that deadline.

`testATimerAlreadyDueIsPostedAndLandsWithALaterTurn` still shows the §4 B shape: the batch does not take the queued wake (`DueBeforeFrameIOSTests.swift:77`), `deliverNativeTimer` posts it (`:79`) while the label is still `"Waiting"`, and `drainFill` lands `"Flipped"`. The false assertion is after `fireTask` and after the batch, so a batch that still posted via `timerDue`, or a landing that delivered the queue, would fail it. The later `XCTAssertTrue` fails if `deliverNativeTimer` does not post. `testABatchWithChangesPostsItsDueDeadlineAfterApplyingThem` reads `accessibilityHint == "landed"` and `"Waiting"` before delivery, then the flip only after `drainFill`.

The driver-owned return is still `session.timerDue` captured after the arm and before `session.clock = due` (`DueBeforeFrameIOSTests.swift:51`). That assignment stays on the virtual clock (`didSet` advances only when the clock becomes nil), so it does not change `timerDue`. The third case is otherwise untouched and still passes that `due` through.

**Frame-health occlusion.** `layoutSubviews` pins the card to `safeAreaLayoutGuide.layoutFrame` inset by 8 (`FrameHealthOverlay.swift:167`). On a notched phone that top is past y=42, and `hitTest` answers only for the handle, details, and close (`:212`). `(70, 42)` never lands on the card there. The test now converts the handle’s midpoint into the window (`AgentCheckIOSTests.swift:54`) and moves node 1 with a `frame` op so that point is the button’s center. `mounted` sets `viewport.frame = window.bounds` (`:34`); page margins stay zero because this harness never mounts an `ExactView`, and the root is at the viewport origin, so that window point is the node’s frame. `cover` hit-tests the node’s window midpoint (`AgentCheckIOS.swift:188`) and walks ancestors for the `exact.host.frame-health` prefix (`:195`). The overlay’s identifier is `exact.host.frame-health-overlay`, so the handle hit is not a finding; changing that identifier to `test.ordinary-overlay` makes the same hit a finding (`AgentCheckIOSTests.swift:62`).
