# Code review: SmoothCollectionIOSTests' frame-bounded waits, round 1 of 2, 2026-10-26 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `59a7b09de6`.
- **Method:** one brief (sha256 `381c588dc38e981cd656b767e17825c5b5a520c5f55e43bcf6cc685ded97c616`), blind to the other reviewer; 2 quick rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

CLEAN.

The frame budget bounds a stuck in-flight motion. `CADisplayLink.timestamp` is the time of the most recent target invocation, so it advances only when `FrameClock.fire` runs (`FrameClockIOS.swift:101`). A registered `OffsetDriver` is ticked in that same invocation (`CollectionIOS.swift:418` registers it, `frame` at `CollectionIOS.swift:452`). While the driver is registered, a new timestamp means that driver got the frame, and a timestamp that stays put means it got none. Progress uses the same clock (`link.targetTimestamp` against a 0.3 s duration), so a stall that jumps the timestamp also finishes the motion on the callback that reveals the jump, and `done()` is checked before the budget (`SmoothCollectionIOSTests.swift:45`, `:59`). A motion that keeps receiving callbacks and is still running once `last - first` exceeds 0.6 s fails. One that receives none is bounded by the 30 s cap.

The three waits whose event is a main-queue async start are acceptable. `wait("the port animates to it, from the next turn")` (`:87`), `wait("moving")` (`:98`), and `wait("the deferred start ran")` (`:261`) observe `offsetDrivers` / `startOwed`, which `animateOffset` updates inside `DispatchQueue.main.async` (`CollectionIOS.swift:222`) before any frame. The success path still completes on an early `run(mode: .default)` turn: that mode drains the main queue, and `run` returns after one source, so the block is handled within a turn or two. The 30 s cap is only the failure path where that block never runs and the link produces no new timestamps (nothing has called `want` yet). A false fail at 0.6 s would need the link to deliver 0.6 s of frames before that already-queued block runs.

Sampling matches the clock. `last` starts at the timestamp from before the wait (`:42`), and `first` is the first newer value, so pre-wait time is excluded and a single frame leaves `delivered` at 0 (`:44`, `:51`). `RunLoop.run(mode:before:)` returns after one source, and the link is one source on `.common`, so each 5 ms turn observes at most one callback. `seen` counts those steps. The gap is wall time between observed steps, and the failure text also includes the open interval since the last one (`:58`).

The attachment API matches this SDK. The test package is iOS 17, and Xcode 27's iOS XCTest headers expose `+attachmentWithString:` (Swift `XCTAttachment(string:)`), `lifetime`, and `XCTAttachmentLifetimeKeepAlways`. The default lifetime is delete-on-success. Setting `.keepAlways` before `add` (`:60–64`) is the documented way to keep an attachment on a passing test. `XCTestCase` conforms to `XCTActivity`, so `add(_:)` is the right call.

No check was weakened. The diff only replaces the give-up condition inside `wait`. Every call site still uses the default, and every later `XCTAssert` is unchanged. A failure still requires `done()` to be false.
