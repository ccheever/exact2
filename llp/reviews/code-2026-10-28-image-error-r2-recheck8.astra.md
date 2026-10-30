# Code review: the image error event on Linux, Android and Windows, round 2, re-check 8 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f8d71f47ae`.
- **Method:** one brief (sha256 `0a7b4bd16626bfaaec71d9f1baa55d8ec39cf65004578ef391172011c437af42`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none. No remaining query leak found in the reviewed downloaded-image error and logging paths.

**Non-blocking**

1. [RasterLoader.swift:257](host/apple/Sources/ExactKit/RasterLoader.swift:257): The new `.svg` branch is in metadata inspection, but SVG parser failures occur during decoding and are swallowed by the catch at line 336. The existing generic error remains safe; the new SVG-specific message is unused. Remove the unreachable branch, or handle that message in the decode path.

Static review only; no tests or builds run.
