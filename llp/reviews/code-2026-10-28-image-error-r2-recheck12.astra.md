# Code review: the image error event on Linux, Android and Windows, round 2, re-check 12 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `14c8880ace`.
- **Method:** one brief (sha256 `7bd8acbf21c6df4ee2baa2892b0c883a807534eb9360cc3218541125162ca7cf`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. Both reported leaks are resolved: all three changed logs truncate the whole source at its first `?` or `#`. The added test covers the whitespace-containing query.

I found no remaining query leak in the reviewed image logs/error paths. `RasterFailure.svg(why)` is discarded by the [decode catch](host/apple/Sources/ExactKit/RasterLoader.swift:331); [reconcile](host/apple/Sources/ExactKit/RasterLoader.swift:1000) reports `decode failed`. No path exposing `why` was found.

**Non-blocking**

none.

Static review only; no files changed, network access, tests, or builds.
