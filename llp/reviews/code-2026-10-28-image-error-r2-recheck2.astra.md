# Code review: the image error event on Linux, Android and Windows, round 2, re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `547856edcb`.
- **Method:** one brief (sha256 `9d96f14f1cb378c6e360e0eadffaf69d72522d241cd853b96d12fcb3936deb96`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [remote.rs:32](host/linux/src/image/remote.rs:32), [RasterRetry.swift:83](host/apple/Sources/ExactKit/RasterRetry.swift:83): **The reported forms are fixed, but queries can still leak.** Both helpers leave these unchanged:
   - `url(a.css?hm=secret)`: no slash or colon. An SVG containing `<style>@import url(a.css?hm=secret);</style>` produces a warning that [svg.rs:198](host/linux/src/image/svg.rs:198) logs with the query intact.
   - `https://h/a b.png?hm=secret`: whitespace separates the query from the scheme/slash. Linux’s URL parser encodes the space for fetching, but the failure logger sanitizes the original source, preserving `hm=secret`.

   Fix by stripping queries from complete known URL values before formatting diagnostics, and handling relative `url(...)` references without requiring a slash or colon. Add regression coverage for both cases.

**Non-blocking**

none.

Static review only; no files edited, network used, or tests/builds run.
