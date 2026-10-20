# Code review: Apple MIDI's hold and tick-batch timers strict (MIDIMacTests' coalesced timers), round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `750819338`.
- **Method:** one brief (sha256 `7ad7e3f8aac99f74c734ee4de9b849a01786456528637702c8f510c41699b13e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking in the two commits; two should-fix in existing code: the hub's 50 ms tick batch is a coalescible main-queue asyncAfter, past a follower's 100 ms lookahead; the stale-drain test's sleeps are coalesced and can miss its race.
- **Disposition:** both taken in the hub commit (a strict timer; a lone-tick test that fails 9 of 9 on the old hub; the stale-drain test paced strictly).

**Nothing blocks these two commits.** I found two follow-ups in existing code:

1. **SHOULD-FIX — [MIDIModule.swift:175](/tmp/rv-ios-8/host/apple/Sources/ExactKit/MIDIModule.swift:175): clock batching remains coalescible.** Under the reported conditions, this 50 ms `main.asyncAfter` can exceed the follower’s [100 ms lookahead](/tmp/rv-ios-8/runner/src/transport/clock.rs:76), causing clock-followed notes to arrive late despite the corrected output timer. **Fix:** use a strict one-shot deadline, deliver the drain on main, preserve the `armed` generation check, and cancel when the batch drains. Add an isolated single-tick latency check without continuing strict sender wakeups. Those wakeups may help service ordinary timers, so the revised test’s success alone does not establish that this batching deadline withstands coalescing.

2. **SHOULD-FIX — [MIDIMacTests.swift:365](/tmp/rv-ios-8/host/apple/tests/ExactKitTests/MIDIMacTests.swift:365): the adjacent stale-drain test still uses coalescible sleeps.** Its intended sequence is tick → note at 10 ms → new tick at 30 ms → original drain at 50 ms. With the reported sleep delays, the new tick can arrive after the original drain has already run, letting the test pass without exercising the race. **Fix:** reuse `pause` for both sleeps and verify that the intended overlap actually occurred.

The arm change itself looks sound:

- **Thread safety and lifetime:** [the handler](/tmp/rv-ios-8/host/apple/midiarm/MIDIArm.swift:343) only calls `perform`; mutable scheduling state stays on the arm thread. The run loop is published before initialization returns. The nested closure preserves the weak capture.
- **Cancellation and close:** cancellation does not interrupt an executing handler or retract its already-posted run-loop block. That matches [libdispatch’s contract](https://github.com/swiftlang/swift-corelibs-libdispatch/blob/main/dispatch/source.h). A stale `schedule()` may cancel a replacement timer, but recomputes from the current queue, without duplicating sends. [Close](/tmp/rv-ios-8/host/apple/midiarm/MIDIArm.swift:378) clears `held` before disposing the client and stopping the loop; a trailing callback therefore cannot send or rearm. After loop exit, it cannot execute.
- **Energy:** the strict timer is one-shot and armed only while messages remain held. Keep the [three-second watchdog](/tmp/rv-ios-8/host/apple/midiarm/MIDIArm.swift:169) non-strict: it refreshes ports and does not release held sends.
- **Test validity:** strict pacing correctly models the intended clock source. Assertions remain unchanged, including the separate send-arrival test’s 2 ms bound. It does not weaken coverage of the output regression.

Read-only review; no builds, tests, or file/index changes.
