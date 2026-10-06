# Code review: the frame sampler sees missed render deadlines, round 2 (d9f97e21a..36c588d1a), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `36c588d1a`.
- **Method:** one brief (sha256 `893230920610039befa5359651c2b38d6efaa27add5402b84022164a568ae45c`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken: the agent's clock taking over stops the sampler in the same call (`Agent.clock`), its turn observers with it; `testTheAgentsClockStopsTheWatch`. 2 taken in part: `testATurnThatRunsPastItsTargetIsSeen` holds one real main-queue turn 60 ms with the link running and reads the overrun the observers alone recorded; the observers' activities and orders are asserted. No nested-loop or tracking-mode drive. 3 taken (it had been missed): the amendment says iOS and macOS.

---

LAND WITH CHANGES

1. **MATERIAL — New turn observers survive virtual-clock takeover.** [Agent.swift:458](host/apple/Sources/ExactKit/Agent.swift:458) assigns `session.clock` without stopping an active sampler. The new [observer callbacks](host/apple/Sources/ExactKit/FrameSampler.swift:131) and [tick](host/apple/Sources/ExactKit/FrameSampler.swift:152) never check it. Start with platform timing, take the clock during an active segment, then perform a slow virtual seek: its work can increment `overruns` and enter the journal/trace. The existing takeover gap now includes turn measurements. **Fix:** synchronously stop sampling when the clock becomes virtual; test active → takeover → seek, asserting observer removal and unchanged counters.

2. **MINOR — Observer behavior remains untested.** [FrameSamplerIOSTests.swift:17](host/apple/tests/ExactKitTests/FrameSamplerIOSTests.swift:17) checks registration only. Empty callback bodies or incorrect activity masks would pass; [FrameSamplerTests.swift:27](host/apple/tests/ExactKitTests/FrameSamplerTests.swift:27) supplies boundaries manually. **Fix:** drive actual observers through first-turn, nested-loop and tracking-mode cases, asserting the resulting records. The round-1 request for an observer-driven regression is only partially addressed.

3. **NIT — The platform correction was not made.** The [amendment:708](llp/1079-speed-hygiene-in-the-agents-hands.rfc.md:708) still says “On iOS,” despite the disposition claiming both platforms. **Fix:** say “On iOS and macOS.”

The callback-replacement and stopped-segment fixes hold. No presentation skip changed; idle-tick dispositions remain intact. Production creation stays gated, captures are weak, observer work is constant, and changed sources satisfy the line cap.

Formatter assertions and `git diff --check` passed. XCTest was not run: this read-only checkout has no build artifacts.