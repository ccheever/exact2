# Code review: the image error event on Linux, Android and Windows, round 2, re-check 4 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `f8e489d751`.
- **Method:** one brief (sha256 `12b4701cab8e055af26ed3171084353faeb857a78c219cb156efdcea3e1ca284`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `host/apple/Sources/ExactKit/RasterRetry.swift:75` — The comment on `redacted` still describes the old per-word cut (first `?` or `#` after `/` or `:`). `withoutQueries` below it is the behavior that actually runs. Drop that paragraph so the next edit does not put the old cut back.

2. `host/apple/tests/ExactKitTests/RasterRetryTests.swift:226` — Linux locks `fill=#fff stroke=#000`. The Swift tests never do. The Swift scan does keep that colour (`#` with no `/`, `:`, or `url(` in the word); the test just does not pin it.

A normal query does not survive a message or a log line. Both hosts use the same scan, and the callers pass the text through it: `workers.rs` logs the name with `without_query` (cut at the first `?` or `#`) and the failure text with `redacted`; `svg.rs` logs through `redacted` and `without_queries`; `RasterLoader.swift` stores `redacted(...)` and logs `withoutQueries(source)`. The two earlier holes are closed: there is no source substitution, so `url(https://h/a.svg?x=1&hm=secret)` loses `x=1&hm=secret` through the `&hm=`, and `url( a.css?hm=secret)` loses the query even with the space after `(`. A trailing `?` stays. A `)` `"` `'` `` ` `` `>` or `]` inside the query still ends the drop, so a tail such as `?q=O'Brien&hm=secret` would keep `Brien&hm=secret`. That is the stop rule this commit states, and Discord-style `ex` / `is` / `hm` values do not contain those marks.
