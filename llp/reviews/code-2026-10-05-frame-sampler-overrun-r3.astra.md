# Code review: the frame sampler sees missed render deadlines, round 3, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `5ee4ebccb`.
- **Method:** one brief (sha256 `9d139a2699a05d20bb52663d8b7db2a8cb13b467fa57b93c91b4a28e23255e0f`), shared with grok. Round 3, blind to the other review. The authors are not reviewers.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition:** 1 taken in part: the held-turn test takes its baseline after the warm-up. Not isolated from the callback path, and no nested or tracking-mode drive.

---

LAND

1. **MINOR — The real-turn test does not isolate the new observer behavior.** [FrameSamplerIOSTests.swift:37](host/apple/tests/ExactKitTests/FrameSamplerIOSTests.swift:37) takes its baseline before warm-up, so an unrelated warm-up overrun can satisfy the assertion. The 60 ms stall can also be counted by [observe’s callback fallback](host/apple/Sources/ExactKit/FrameSampler.swift:164) with an empty `beforeWaiting` handler. **Fix:** baseline after warm-up and drive a controlled turn whose observer records an overrun while callbacks remain punctual; assert that record’s `overrun > 0` and `missed == 0`. First-install and explicit nested/tracking-mode cases remain unpinned.

Earlier correctness fixes hold: [stop clears pending timing](host/apple/Sources/ExactKit/FrameSampler.swift:109), callbacks check the previous target before replacement, and [clock takeover stops sampling](host/apple/Sources/ExactKit/Agent.swift:468). Both platforms share weak observer captures and constant per-turn work; production creation remains gated.

No presentation skip changed, so no new batch, trait, recycling, nested-apply or in-flight-fill behavior difference was found. The idle-tick recovery fixes remain intact. Changed sources satisfy the 1,500-line limit.

Formatter assertions and `git diff --check` passed. XCTest was not run: this read-only checkout has no build artifacts.