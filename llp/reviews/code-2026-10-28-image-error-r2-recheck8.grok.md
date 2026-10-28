# Code review: the image error event on Linux, Android and Windows, round 2, re-check 8 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `f8d71f47ae`.
- **Method:** one brief (sha256 `0a7b4bd16626bfaaec71d9f1baa55d8ec39cf65004578ef391172011c437af42`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image/svg.rs:191` (logs at 206 and 217). `is_remote` is only `http`/`https`, but the branch that still prints the parser error and each warning is everything else, including a page `data:` image and an `app:/` file (a picked file, not a bundled asset). Those strings include the file: a dropped `@import url(...)` is the whole rule, not a 40-character clip. The log then calls `redacted` with the name already passed through `without_query`, so the new whole-source replace never sees a `?` or a space. `without_queries` still ends a query at the first ASCII space, tab, newline, carriage return, or form feed, and the tail stays. `@import url(https://h/a?x=a &hm=secret)` logs as `&hm=secret`. The same line also prints a `data:` name in full except for a literal `?` or `#`, so a percent-encoded query (`%3F…hm%3Dsecret`) is still in the log. Quote parser text and warnings only for a package-relative asset. For `data:`, `app:/`, and `http(s)`, log a fixed reason and a warning count. For `data:`, log the media type only (`data:image/svg+xml,…`), not the body.

**Non-blocking**

- A package-relative SVG still logs warnings in full, with that same whitespace split, and `redacted` cannot see the original query because the name was already cut. That matches the comment for the app’s own file, and it still leaves a signature that sits after a space.
- `host/apple/Sources/ExactKit/RasterLoader.swift:257` does not run for a parser refusal. `RasterFailure.svg` is thrown from `RasterImage.decodeSVG` and dropped in `decode` (around line 336), which only fails the permit. The image error stays the core’s “decode failed”, which has no reason and no query.
- `redacted` only pulls a longer link’s tail along while that tail stays in the same word as the inserted `?…`. A space before the rest of the query leaves the rest. The new tests cover a glued tail, not that space.
