# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `bd89cc2f2`.
- **Method:** one brief (sha256 `3d33bbac1b487ea94f4b82bb5cb3ee265ed23e1a4b18c4ed5e1863364f9a4f7c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** two blockers (a measured lag added to a ceiling can admit a second hold or a coalesced send), two should-fix (MainLag's unowned self off main; the hub's observer unsynchronized).
- **Disposition:** all taken in `ce32fa972`: a lag qualifies a check, never widens it; weak self; a locked observer.

**Two blockers remain in the timing assertions.**

1. **BLOCKER — [MIDIMacTests.swift:388](/tmp/rv-ios-13/host/apple/tests/ExactKitTests/MIDIMacTests.swift:388): measured lag can still hide a second hold.**  
   Scenario: handoff = 80 ms, delivery = 130 ms, main lag = 10 ms. The extra 50 ms hold passes: `130 < 80 + 45 + 10`. That lag can occur entirely before the hub hears the tick, because the measurement starts at its stamp. Likewise, with zero handoff and 100 ms lag, delivery at 190 ms passes a 195 ms ceiling—admitting 140 ms beyond the intended deadline.  
   **Fix:** exclude lag before the actual drain deadline, and treat attempts whose allowance erases the remaining 5 ms separation from a second hold as inconclusive, with bounded retries/skip. Preserve a controlled or quiet timing check for the single-hold invariant.

2. **BLOCKER — [MIDIMacTests.swift:135](/tmp/rv-ios-13/host/apple/tests/ExactKitTests/MIDIMacTests.swift:135): unrelated probe lag can excuse the coalesced send regression.**  
   Scenario: the note is due at 200 ms and arrives at 300 ms; a probe beat subsequently stalls 110 ms during the remaining wait. `busy.most` includes that later stall, so the 100 ms late send passes the 112 ms ceiling. Matching `.userInteractive` QoS is defensible as a load signal, but establishes no bound on the arm thread and CoreMIDI’s delays; it can overestimate or underestimate them.  
   **Fix:** exclude post-arrival samples and use the probe to qualify attempts against a fixed regression-rejecting ceiling, or measure the relevant send stages directly. An unbounded maximum from another queue cannot establish timely delivery.

3. **SHOULD-FIX — [MIDIMacTests.swift:85](/tmp/rv-ios-13/host/apple/tests/ExactKitTests/MIDIMacTests.swift:85): background sampling makes `[unowned self]` unsafe.**  
   Scenario: the handler starts on `send lag`, is descheduled, and resumes after `stop()` and release of `busy`. Dispatch cancellation does not wait for an executing handler; a subsequent unowned access can trap. Locking `beats` protects the collection, not the owner’s lifetime.  
   **Fix:** capture weakly and retain `self` locally for the handler’s duration. Synchronize cancellation completion if `stop()` must also provide a final, stable sample set. `due` itself remains confined to the serial handler queue.

4. **SHOULD-FIX — [MIDIModule.swift:157](/tmp/rv-ios-13/host/apple/Sources/ExactKit/MIDIModule.swift:157): the observer closure has unsynchronized cross-thread access.**  
   Scenario: main clears `observeHeard` while a delayed arm callback reads it. Shutdown queues input disconnection asynchronously, so it does not guarantee callback quiescence. The test’s lock protects `handed`, but neither the closure’s storage nor its retention; `nonisolated(unsafe)` supplies no synchronization.  
   **Fix:** protect installation, removal, and copying of the observer with the same lock, then invoke the retained copy outside that lock.

The lone-tick test retains its fixed 100 ms ceiling, so the batch finding does not imply that its separate coalescing check passes. The CoreMIDI change only clarifies an existing skip reason.

Read-only static review; no edits, builds, or test runs.
