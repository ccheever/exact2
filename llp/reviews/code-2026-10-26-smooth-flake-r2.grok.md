# Code review: SmoothCollectionIOSTests' frame-bounded waits, round 2 of 2, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `59a7b09de6`.
- **Method:** one brief (sha256 `2eca793005a61ba5a6943d417677851128ee2722cf3bbe61bf6f3ca8e5722921`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

CLEAN.

The frame budget bounds a stuck motion, and a motion that never ends still fails. `CADisplayLink.timestamp` changes only when that link’s callback runs, and the callback is `FrameClock.fire`, which ticks every registered user with the same link. `OffsetDriver` is registered in `start()` for the whole motion and advances from `link.targetTimestamp` on that same callback (`CollectionIOS.swift:419` and `CollectionIOS.swift:452`). While the driver is on the clock, timestamp time and driver frames move together: after the first new sample, `delivered = last - first` passes 0.6 s and `wait` fails (`SmoothCollectionIOSTests.swift:44` and `SmoothCollectionIOSTests.swift:51`). A stalled link leaves `delivered` at 0 (one frame sets `first == last`), and the 30 s cap fails it (`SmoothCollectionIOSTests.swift:45` and `SmoothCollectionIOSTests.swift:59`). Setting `first` to the first new timestamp, rather than `before`, drops a stale or zero baseline (a paused link reads 0) so one resumed frame does not instantly burn the budget.

No wait in this class is served by anything other than this run-loop pump. The two non-frame predicates, `moving` (`offsetDrivers` filled from `DispatchQueue.main.async`) and `the deferred start ran` (`startOwed` cleared at the top of that block, `CollectionIOS.swift:222`), are already queued before `wait` and run on the same `.default` turn as before. A live run returns on the first turns. Only a start that never runs, with the link paused, sits out the 30 s cap and then fails. That is the approved backstop.

Sampling matches the budget. `t > last` once per 5 ms turn, `seen` and `gap` are diagnostics, and the reported gap includes the open wait since the last new timestamp (`SmoothCollectionIOSTests.swift:58`). Coalesced callbacks still widen `last - first`. The final `<= 0.6` check exits only once link time is past 0.6 s, which is twice the 0.3 s ease.

`XCTAttachment(string:)` is `attachmentWithString:` (plain text), present in the iOS 17 XCTest this target builds with. `.keepAlways` is set before `add` (`SmoothCollectionIOSTests.swift:61`), which is XCTest’s contract for keeping the attachment in the xcresult when the test passes. The failure path puts the same text in `XCTFail`, so a red test keeps it as the assertion message.

No product check was loosened. Every `wait` predicate and every later `XCTAssert` is unchanged. `done() == false` still fails. The pass path returns as soon as the predicate is true, so the budget cannot turn a finished motion into a pass by waiting.

Nothing in this class calls `FrameClock.fire`. The `driver.drawn` waits are the real shared link: `drawn` is set in `OffsetDriver.frame`, the same callback that updates `timestamp`. A hand-fired `FrameClock.shared.fire` (only `FrameClockIOSTests`) would not move `timestamp`, but it would satisfy `done()` and this `wait` returns on the next turn anyway. A stuck motion still cannot pass.
