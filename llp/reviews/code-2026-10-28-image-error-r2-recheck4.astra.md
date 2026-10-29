# Code review: the image error event on Linux, Android and Windows, round 2, re-check 4 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f8e489d751`.
- **Method:** one brief (sha256 `12b4701cab8e055af26ed3171084353faeb857a78c219cb156efdcea3e1ca284`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Query contents still leak** — [remote.rs:41](host/linux/src/image/remote.rs:41), [RasterRetry.swift:91](host/apple/Sources/ExactKit/RasterRetry.swift:91). Both transform `https://h/a.svg?x=)&hm=secret` into `https://h/a.svg)&hm=secret`. A literal `)` is valid query content. Linux’s SVG logger passes the raw source through this helper, so this can expose a signature in logs. Redact complete URL spans before formatting, or distinguish enclosing delimiters from query characters; add this regression on both hosts.

2. **Quadratic scanning allows image-worker starvation** — [remote.rs:46](host/linux/src/image/remote.rs:46), [RasterRetry.swift:95](host/apple/Sources/ExactKit/RasterRetry.swift:95). Every character rescans the growing word for link markers. A remote SVG containing a very long malformed tag generates a diagnostic quoting that entire tag; Linux then performs O(n²) redaction on its fixed raster-worker pool. Track link-marker state incrementally, with bounded lookbehind for `url(`, instead of repeatedly searching the accumulated word.

**Non-blocking**

none
