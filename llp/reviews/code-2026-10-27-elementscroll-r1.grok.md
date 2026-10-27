# Code review: ElementScrollTests' motions observed by bounds changes (a Mac flake), round 1 of 2, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `6c83a7dc2b`.
- **Method:** one brief (sha256 `a444373b7278368f98c1de4cd80b97259d256c2291ca0ff8a298cbf1cd3018a0`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**CLEAN.** Commit `6c83a7dc` does what the approved test change describes. Both files parse for the macOS and iOS Simulator targets.

`NSView.boundsDidChangeNotification` is posted for each bounds step of `NSClipView.animator().setBoundsOrigin`, not only at the end. A 0.25 s animation on this machine produced 28 notifications, 26 of them strictly between the start and the target, including with a layer-backed parent. They arrive in pairs, about two per run-loop turn. `setBoundsOrigin` posts when the origin changes and `postsBoundsChangedNotifications` is set; the helper sets that flag before the motion starts.

`giveUp` runs in `waitFor`'s loop condition, so each call records the previous turn and the completing turn is not recorded. For this animator the completion handler runs two turns after the last bounds notification (last note on turn 18, completion on turn 20 in a 0.25 s run), so the turn that drew the frame is one `giveUp` does measure. A live run's slowest `giveUp` interval matched the slowest `run` (0.138 s).

The element-animation mark is set synchronously in `IntoView.swift` before `runAnimationGroup`, and the test reads it after `scrollElementIntoView` returns and before the wait. The completion handler clears it and then calls `endScroll`, which is what fires `scrollend` and ends the wait. The "gone after its end" check runs after that wait.

The observer token is removed with `defer` on every exit from `observeMotion`. The only softer intermediate check is the three-part late pass (one recorded bounds change, a turn slower than 0.25 s, and the mark). The hand-built scroll view does not pass `marked`, so `animated` stays false and that pass does not apply. The synchronous-land, final-offset, and single-`scrollend` checks are unchanged.

`latePass` is a straight extract of the old attachment-and-log line in `EventWaits.swift`, which both hosts compile. The new helper sits in the macOS section of `ElementScrollTests.swift`; the iOS section is untouched.
