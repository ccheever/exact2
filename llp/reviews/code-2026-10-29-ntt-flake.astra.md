# Code review: NativeTaskTimersTests load flake (retry ask waited for), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `e1ca00033d`.
- **Method:** one brief (sha256 `c4b496a20174bd989626bfc64c57545c36888bcdaef1281613dd4a933cbfc4fd`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none.

**Non-blocking**

1. [NativeTaskTimersTests.swift:206](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:206): The 50 ms window can still miss a duplicate ask delayed by load; teardown then empties its queue. This permits a false pass, rather than recreating the reported missing-ask failure. For deterministic “once” coverage, use a controlled scheduler and drain the scheduled callbacks before asserting.

2. [NativeTaskTimersTests.swift:204](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:204): Pre-existing limitation: “not at once” meaningfully rejects synchronous delivery, but an immediate `DispatchQueue.main.async` would also pass. It does not establish the frame delay. A scheduler seam could assert the requested `1/60` delay while retaining the bounded wait for delivery.

3. [NativeTaskTimersTests.swift:208](host/apple/tests/ExactKitTests/NativeTaskTimersTests.swift:208): Pre-existing limitation: with only `a`, “at the front” proves presence, not ordering. Queue a distinct second row before the initial ask, then verify the retry yields `a`, the second row, and nothing else. The current final `take() == nil` still checks that the intervening firings added no deliveries.
