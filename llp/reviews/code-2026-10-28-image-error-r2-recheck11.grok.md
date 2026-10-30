# Code review: the image error event on Linux, Android and Windows, round 2, re-check 11 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `ba008c577f`.
- **Method:** one brief (sha256 `2a08382eca35efcfa7f128ee501a4554e90396888f874d61f6ef9ed47bad8f45`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/apple/Sources/ExactKit/RasterImage.swift:29` — An SVG the parser refuses becomes the image `error` as `not an SVG this host reads: \(why)`, and `why` is the parser’s own text (`host/apple/src/svg_image.rs:22`). An end tag’s body is copied in whole up to `>` (`host/svg-file/src/xml.rs:166`), so a file can carry `</x https://cdn.example/a.png?ex=1 &hm=secret>`. `RasterRetry.withoutQueries` (`host/apple/Sources/ExactKit/RasterRetry.swift:103`) drops only through the next ASCII space, so `&hm=secret` stays in the `error` payload (`RasterLoader.swift:1262`) and in `image deferred:` (`RasterLoader.swift:1006`). Linux no longer includes that text (`host/linux/src/image/svg.rs:207`). Fix: return a fixed reason with no file text, the same sentence Linux logs, and do not pass `e.to_string()` or the scene buffer into `RasterFailure.svg`.

2. `host/apple/Sources/ExactKit/RasterLoader.swift:622` — The refusal log runs `withoutQueries` on the raw source. For `https://cdn.example/a.png?ex=1 &hm=secret` the tail `&hm=secret` is a new word and is logged. Linux cuts the source at the first `?` or `#` in the whole string (`without_query` in `host/linux/src/image/remote.rs:22`), which removes that tail. Fix: cut this source the same way before logging, or pass it through `redacted(_:source:)` so the whole source, spaces included, is replaced before the word scan.

**Non-blocking**

1. `host/linux/src/image/svg.rs:207` — Nothing asserts that a warning or parse error stays off stderr. The old quoted-span test was removed with the helper (`host/linux/src/image/remote_tests.rs`). A later edit can quote the file again and no test fails. Add one decode log assertion: a warning that contains `url("https://h/a?x=a &hm=secret")` logs only the count and the source cut at `?`.
