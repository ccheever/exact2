# Code review: #1144 native timer retry, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c183367a4e`.
- **Method:** one brief (sha256 `ee36599369ebe0c5ec411cd662b5a25ab002857e19d46bdf2aee767e9a266622`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [NativeTaskTimersTests.swift:155](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:155): The tests never prove the queued delivery received busy/held. One run-loop pass is not an acknowledgement; if the ask has not run, `hasReady` is already true and `advanceInFlight` false. The gate can then clear and ordinary ticking satisfy every assertion—even without the fix. Keep the gate active until the specific refused timer call is observed, then await its requeue. For held, assert the corresponding `heldCalls` entry.

2. [NativeTaskTimersTests.swift:149](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:149): `periods + 2` permits the duplicate being tested against. A retry plus one duplicate gives two increments; the next legitimate firing gives `fired = 3`, `periods = 1`, and `3 <= 3` passes. It also ends before two further periods occur. Assert exactly one retry and one firing in each of two subsequent controlled periods, including a Foundation callback while the retry remains pending. Keep delivery automatic.

**Non-blocking**

1. [NativeTaskTimersTests.swift:250](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:250): The owner-queue test still has the original notification queued by `timer.fire()` when the manual delivery hits `ownerInFlight`. That original notification can deliver after the fill lands, so the test does not establish its comment’s claim that the fill resumes delivery. Let the queue’s own ask encounter the guard before publishing the fill.

No separate production-code blocker found: pending entries coalesce Foundation callbacks, persistent refusals produce bounded polling, and cancellation plus entry/epoch checks prevent stale entries from returning. A delayed notification postpones an immediate `update()` request but retains an outstanding ask.

Static review only; no tests or builds run.
