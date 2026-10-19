# Code review: MIDIMacTests' latency bounds and the main queue's own lag, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e1833ee20`.
- **Method:** one brief (sha256 `4b4c27fea6f482383666dab4b42d3ae5428adbbdc07cfb0adaf6eb3781b12039`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; two should-fix: lag seen anywhere in the window widens the bounds, so one spike could admit a coalesced drain (and an unmeasured stall in the idle window still fails correct code); MainLag loses its timer's phase after missed beats.
- **Disposition:** both taken in `20141b7db` (the clock test adds only the lag while each batch waited; the lone tick is timed on a fixed 40-100 ms in quiet attempts only, skipped if none is quiet; the phase kept with the timer's expiration count). The old hub still fails 3 of 3.

Nothing blocking in HEAD. Two nonblocking findings:

1. **SHOULD-FIX — [MIDIMacTests.swift:359](/tmp/rv-ios-10/host/apple/tests/ExactKitTests/MIDIMacTests.swift:359), [432](/tmp/rv-ios-10/host/apple/tests/ExactKitTests/MIDIMacTests.swift:432): unrelated lag can excuse a regression.** A 150 ms spike during either calibration raises the lone-tick ceiling to 250 ms, admitting the reported 193 ms old-hub result even if its idle window was otherwise quiet. Similarly, one clock-test spike—including during the trailing wait—can excuse an isolated second batch hold; the batch-count assertions need not catch that. Conversely, a stall confined to the idle window remains unmeasured and can still fail correct code.
   
   **Fix:** use contemporaneous, per-batch lag for the clock assertion, with a nominal 50 ms budget plus measurement margin below another 50 ms hold. For the idle test, qualify timing on sufficiently quiet attempts; report overloaded attempts as inconclusive rather than enlarging the ceiling into the known regression range. The reported 193-versus-101 result establishes discrimination for that sample, not under large allowances.

2. **SHOULD-FIX — [MIDIMacTests.swift:81](/tmp/rv-ios-10/host/apple/tests/ExactKitTests/MIDIMacTests.swift:81): `due` loses the timer’s actual phase after missed beats.** If the beat due at 10 ms runs at 115 ms, this sets `due` to 105 ms; the next scheduled beat is at 120 ms. An on-time callback then reports 15 ms of nonexistent lateness, and subsequent stalls can inflate `most`.
   
   **Fix:** preserve the scheduled phase and advance `due` by `Double(timer.data) * 0.010`, using the same initial deadline for scheduling and measurement. Dispatch supplies the number of expirations represented by each callback. [Dispatch timer contract](https://github.com/apple-oss-distributions/libdispatch/blob/main/dispatch/source.h)

The clock test’s 10 ms wakeups **can mask a coalesced drain**. That division of coverage is acceptable with the separate idle lone-tick test, subject to finding 1; the helper’s comment currently overstates the guarantee.

`unowned self` and cancellation look safe at the current call sites: locals remain alive through `stop()`, execution is on main, and cancellation prevents subsequent handler invocations. There is no concurrently executing handler to outlive the object here. [Dispatch cancellation contract](https://github.com/apple-oss-distributions/libdispatch/blob/main/dispatch/source.h)

Reviewed HEAD and four preceding commits. No builds, tests, or file/index changes.
