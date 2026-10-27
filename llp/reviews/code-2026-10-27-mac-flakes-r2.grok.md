# Code review: two Mac flakes' event waits (ScrollEnd, HoverFollowsLayout) and the one EventWaits helper, round 2 of 2, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `581768a5dc`.
- **Method:** one brief (sha256 `d2c482f7b3b5c5b6fa5c11d8cd3651d97381fff930ae08aec11f9abce4eeccbe`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

CLEAN.

The not-too-early checks take `Date` immediately before the move that schedules the work, and they compare after `waitFor` has seen it. In `ScrollEndMacTests.swift:31` that move is `move(sv, to: 80)`, which reaches `ScrollEnd.settle` (`asyncAfter(deadline: .now() + quiet)`) synchronously through the clip-view bounds observer. In `HoverFollowsLayoutMacTests.swift:275` it is `scroll(to:)`, which reaches `followPointerAfterScroll` the same way. `asyncAfter` arms a timer at that monotonic deadline, and `waitFor` returns only once the callback has run on the main run loop, so the wall interval from the earlier `Date` is at least `ScrollEnd.quiet` or `Presenter.scrollRestInterval`. Each scroll cancels the previous `DispatchWorkItem` before arming the next, and `last` is the final iteration, so the bound is the timer that actually fires.

`SmoothCollectionIOSTests.swift:43` shares one `sample()`. `giveUp` samples, then stops once link time is past the frame budget. The loop tests `done()` before `giveUp`, so the frame that arrived on the spin that finished the motion is read by the extra `sample()` in `facts()` (`:53`). That string is what a failure and a LATE-PASS record. A timestamp that did not move is not counted twice.

`interruptElementJump` (`ScrollEndMacTests.swift:133`) waits with `waitFor("interrupted scroll ends", late: 1)`. The end is the same `settle` from `cancelElementScroll`, after the handler is installed. `late: 1` is the lateness mark; the failure cap stays 30 seconds. The bounds and `ends == 1` asserts are unchanged.

The LATE-PASS line is `print` to stdout (`EventWaits.swift:35`), with the test `name` so a flat log can tell tests apart. `macTests` pipes the `xctest` process's stdout and stderr and writes them when the class exits, which is the log the Mac run keeps. The attachment is still added with `keepAlways` for the iOS result bundle. A pass slower than `late` records that line. A dropped work item still hits the 30 second cap and fails with the timing string.
