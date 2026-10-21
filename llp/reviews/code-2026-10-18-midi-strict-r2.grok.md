# Code review: Apple MIDI's hold and tick-batch timers strict (MIDIMacTests' coalesced timers), round 2, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ed37c024f`.
- **Method:** one brief (sha256 `024954a373e0b3a36d008a48709e9667312e760bee49f113f4e770e9e23265fc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; the main-queue strict timer, its self-release and one timer per batch hold.
- **Disposition:** nothing to take.

Nothing blocking. No findings.

The main-queue strict timer does what round 1 asked. `.strict` is a flag on the timer source (`DISPATCH_TIMER_STRICT`), set at creation in `MIDIModule.swift:178`, and `dispatch_source_set_timer` observes that leeway even when UI visibility would otherwise widen the lower bound. The target queue only chooses where the handler runs. On `.main`, the strict kevent still wakes the main run loop through libdispatch’s mach port; the handler then waits only for that queue. With the loop in the default mode — the test’s `RunLoop.main.run`, and the app’s normal loop — that wake is the delivery. The lone-tick run is the measurement: 40–75 ms with the flag, 133–202 ms on the old `asyncAfter`.

The handler’s capture of `timer` at `MIDIModule.swift:180` is the keep-alive a one-shot needs after `heard()` returns. `timer.cancel()` at the start of that handler is the release: once the handler returns, libdispatch drops the block, and the block drops the source. The hub is already process-lifetime, and the cycle ends at the fire.

One batch has one timer. `later` is `tick && !draining && !ticking` (`MIDIModule.swift:166`), and `ticking` is set under that same lock before the source is created. The arm calls `heard()` synchronously from its ring drain, so the next tick in a burst sees `ticking` and only appends. A note in that window takes the `now` path; the pending timer’s `drain(armed:)` then no-ops because `drains` has moved. A superseded timer can still be in flight for up to ~50 ms. It does not open a second batch.

The 40–75 ms window is the right width for this regression. 40 ms is 10 ms under the 50 ms hold, so an immediate drain (a few milliseconds) fails and a strict fire near 50 ms does not. 75 ms is 25 ms over the hold, still under the coalesced band that failed 9/9 (133–202 ms) and under the follower’s 100 ms lookahead. Time spent before `heard()` shrinks `wait` (`max(0, 0.05 - elapsed)` at `MIDIModule.swift:177`), so load adds to the measured interval only after that 50 ms budget is already used. The reported runs stayed inside the band at load ~45.

Also checked, and sound: the arm’s timer (`MIDIArm.swift:341–345`) is strict on a user-interactive global queue and wakes its run loop with `CFRunLoopPerformBlock` plus `CFRunLoopWakeUp` (a mach wakeup, so the arm thread is not waiting on a coalesced run-loop timer). `schedule()` runs only on that thread, and a stale fire just schedules again from `held`. The 3 s watchdog (`MIDIArm.swift:169`) staying a plain run-loop timer is right: 100–150 ms on a 3 s re-enumerate does not miss the poll. `pause()` (`MIDIMacTests.swift:57–65`) holds its source on the stack across `semaphore.wait()`, so the clock test and the stale-drain test keep real 20.8 ms / 10 ms / 20 ms gaps.
