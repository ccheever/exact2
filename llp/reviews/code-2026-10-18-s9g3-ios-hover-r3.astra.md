# Code review: LLP 1121 stage 9 G3, a pointer's :hover look on iOS, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 3: high), sandbox read-only, a throwaway worktree.
- **Scope:** round 2's commit.
- **Method:** one brief (sha256 `b1cf56c436f000b31f1e08c362657611f3effb3f231ef0c2784f41b399099504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (tests for pooling and a cancel while over).
- **Disposition:** fixed in round 3's commit.

---

Nothing blocking found. The crossing baseline, paint-before-release ordering, and descendant hold cleanup look correct by inspection.

1. **should-fix** — [HeldStateTests.swift:750](/tmp/rv-g3-3/host/apple/tests/ExactKitTests/HeldStateTests.swift:750): The tests cover crossing and recognizer removal, but never exercise row pooling. Add a regression that destroys a collection row with a hover-held descendant, asserting one `"gone"` notification and no hold surviving reuse.

2. **should-fix** — [HeldStateTests.swift:762](/tmp/rv-g3-3/host/apple/tests/ExactKitTests/HeldStateTests.swift:762): Cancellation is tested only with real hover already false. Add cancellation while hovered, asserting paint and hold survive, a subsequent `.began` preserves the hold, and `.ended` releases it.

Read-only review; no builds or tests run.
