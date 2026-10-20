# Code review: Apple MIDI's hold and tick-batch timers strict (MIDIMacTests' coalesced timers), round 2, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ed37c024f`.
- **Method:** one brief (sha256 `024954a373e0b3a36d008a48709e9667312e760bee49f113f4e770e9e23265fc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; one should-fix: the lone-tick test's 75 ms ceiling risks load flakes.
- **Disposition:** taken: the clock test's 100 ms ceiling, which still rejects the old hub's 133-202 ms.

Nothing is blocking. One nonblocking finding:

1. **SHOULD-FIX — [MIDIMacTests.swift:403](/tmp/rv-ios-9/host/apple/tests/ExactKitTests/MIDIMacTests.swift:403): the 75 ms ceiling risks load-related flakes.** A scheduling delay exceeding 25 ms after the 50 ms deadline fails correct code; `.strict` reduces timer coalescing but provides only best-effort timing. The neighboring test already documents 83 ms delivery under load and allows 100 ms. **Fix:** use a 100 ms upper bound, retaining the 40 ms lower bound and idle run-loop wait. This still rejects all reported 133–202 ms regression samples. [Apple’s timing contract](https://developer.apple.com/documentation/dispatch/dispatchsource/timerflags)

The production changes look sound on inspection:

- **`.strict` applies with `.main`.** Timer strictness is independent of the handler’s target queue; main-thread congestion can still delay execution. [libdispatch implementation](https://github.com/apple-oss-distributions/libdispatch/blob/main/src/event/event.c)
- **The self-capture does not permanently leak.** Cancellation cleanup releases the event handler, breaking its reference back to the timer after execution. [Cancellation implementation](https://github.com/apple-oss-distributions/libdispatch/blob/main/src/source.c)
- **One timer per batch generation:** [MIDIModule.swift:166](/tmp/rv-ios-9/host/apple/Sources/ExactKit/MIDIModule.swift:166) sets `ticking` under the lock. Early drains can leave older timers pending, but those cancel themselves and fail the unchanged generation check.
- No additional findings in the arm’s strict hold timer or `pause()` changes. Leaving the three-second watchdog non-strict is appropriate.

Read-only review of all three diffs and surrounding code; no builds or tests run.
