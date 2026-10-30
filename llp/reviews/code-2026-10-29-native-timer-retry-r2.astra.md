# Code review: #1144 native timer retry, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `aa621854e8`.
- **Method:** one brief (sha256 `be1d9083d4b641994018f89941e2fb7b394876661cf416db76406dbe5deb58fd`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [NativeTaskTimersTests.swift:125](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:125): **The shared test target cannot compile for iOS.** Foundation `Process` is unavailable there, and this fixture has no platform guard. Compile the ticker through the existing `xctest.mjs` fixture setup and load it with `fixturePlan`, preserving both platforms’ coverage.

2. [NativeTaskTimersTests.swift:175](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:175): **The timing assertions can flake on correct behavior.** Foundation retains the original repeating phase. With deadlines at 1 s and 2 s, a delayed retry at 1.7 s legitimately leaves only 300 ms until the next firing, failing the `>500` assertion. A retry near 1.9 s also fails the 200 ms unchanged-count assertion. Use controlled time and explicit firings for exact coalescing assertions; let the live-timer check establish continued progress without a minimum gap measured from the retry.

**Non-blocking**

1. [NativeTaskTimersTests.swift:169](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:169): **Tick counts do not count every host delivery.** These back-to-back firings have cutoffs within the same period. Even if incorrectly queued, extra deliveries can produce no ticks because `fire_native_timer` rejects `cutoff < nominal`. Extend the queue unit test to fire twice after `complete(retry: true)`, take the retained delivery, and assert that no second delivery exists.
