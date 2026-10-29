# Code review: the image error event on Linux, Android and Windows, round 2, re-check 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `d79b7210ab`.
- **Method:** one brief (sha256 `dff19173615cd456558f8fc23f7aa3aa334c20ff8720d437981b1dee8d274f2a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image/svg.rs:191` (with `host/linux/src/image/remote.rs:42`) — SVG decode used to redact the source name and then wrap it in backticks. It now wraps the raw name first (`exact: image `{name}`: …`) and redacts the whole line. `without_queries` treats a word as a path only when the raw word starts with `/`. The backtick makes `` `/a.svg?hm=1`: `` and `` `//cdn.example/a.svg?hm=1`: `` fail that test, and they have no `://`, so the query stays. Any warning, parse error, or non-UTF-8 log for a path or protocol-relative source prints the signature. That redaction used to happen. Fix: after the same leading-punctuation trim the scheme check uses, treat a word as a link when that remainder starts with `/` (Apple's `withoutQueries` at `RasterRetry.swift:86` should use the same trim, so `` `//host/a?hm=1` `` matches). Add those two strings next to the new backtick test.

2. `host/linux/src/image/remote.rs:31` and `host/apple/Sources/ExactKit/RasterRetry.swift:82` — A bare scheme is recognized only at the start of the word, after one run of leading punctuation. A quoted SVG value is one word, so `fill="url(https:h/x?hm=s)"` and `fill="url(//h/x?hm=s)"` are not links: the scheme or `/` sits after `url(`, and there is no `://`. `lower.rs` quotes that value when it is not valid for the row (`Paint::parse` rejects an external `url()`), and `svg.rs:199` logs the warning. The new test only locks the `https://` form, which `://` already catches anywhere in the word. Fix: scan the word for a scheme (`[A-Za-z][A-Za-z0-9+.-]*:`) or a `/` whose previous character is not a scheme character, and cut at the first `?` or `#`. Keep `why?` and `HTTP 404` unchanged. Cover `url(https:…?hm=)` and `url(//…?hm=)` on both hosts.

**Non-blocking**

The `rejected = None` resets on the symbol and package arms, and renew's keep when `view.source` is empty and `rejected == source_hash(kernel source)`, match the round-2 cases. Nothing in this commit tests them (a symbol or package between two identical over-long assignments must error again; renew of an unchanged over-long source must keep the view). The new tests cover only the sanitizer.
