# Code review: the image error event on Linux, Android and Windows, round 2, re-check 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `eab63da37b`.
- **Method:** one brief (sha256 `70ba80b029a3cf1d62becd98d68e9a1105407b816e52546c3c5ae35876366814`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image/remote.rs:32` and `host/apple/Sources/ExactKit/RasterRetry.swift:84` — `redacted` replaces every substring occurrence of the source with the source cut at the first `?` or `#`. When another link in the same line is longer and starts with that source, the replacement deletes the `?` and leaves the rest of the query. The per-word pass then sees no `?` or `#` and keeps the tail. The SVG log builds `exact: image `{name}`: {warning}` and runs it through `redacted` (`host/linux/src/image/svg.rs:189`, `host/linux/src/image/svg.rs:199`). A stylesheet warning quotes the raw at-rule (`host/svg-file/src/css.rs:105`, `host/svg-file/src/lower.rs:174`). An image loaded from `https://cdn.example/a.svg?hm=aaa` whose file contains `@import "https://cdn.example/a.svg?hm=aaa&is=bbb"` logs `https://cdn.example/a.svg&is=bbb`. The same helper builds the image error (`host/linux/src/image/workers.rs:379`, `host/apple/Sources/ExactKit/RasterLoader.swift:256`). On each match, emit the source cut at `?` or `#`, and if the next character continues that token, drop the glued tail through the next delimiter (whitespace, backtick, quote, parenthesis, comma, semicolon, or colon) before copying further. Then run the per-word pass. Add a test where the source is a proper prefix of another link on the same line and the whole query is gone.

**Non-blocking**

none
