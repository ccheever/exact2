# Code review: ResetGate knownLoss follow-up, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `faa1260bed`.
- **Method:** one brief (sha256 `9a1b986fb5d8e6246a7455beb0af9b3eafb50b6893f1d87501f11954cb63784f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [ResetGate.swift:167](host/apple/Sources/ExactKit/ResetGate.swift:167), also [QUEUE.md:4](QUEUE.md:4): `regionComplete` is still misclassified. Restart increments the session generation before holding; [RegionController.receive:183](host/apple/Sources/ExactKit/RegionController.swift:183) rejects the old generation before calling the runtime. Reset then clears the controller, and replacement work starts after the hold. Like `fulfillSurface`, this is obsolete work, not lost input for the replacement. Move it to `dropped`, remove it from `KNOWN_LOSS`, and update the count to 16.

**Non-blocking**

none.

The current Swift table and Rust `KNOWN_LOSS` match exactly: 17 entries. The two moved entries and split `change` reason check out. No tests or builds run.
