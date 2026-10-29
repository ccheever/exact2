# Code review: the image error event on Linux, Android and Windows, round 2, re-check 10 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `506c874585`.
- **Method:** one brief (sha256 `5271edb5a3d2937ece2609e514145ef5afc0bb38299cbd81b5391bd5154bb1ad`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image/remote.rs:93` (stop at `remote.rs:100`) — `url(` is closed by the first `)`. A bundled stylesheet warning copies the at-rule text through that function (`host/linux/src/image/svg.rs:228`; the prelude is the dropped selector in `host/svg-file/src/css.rs:105`). For `@import url("https://h/a?file=(1).png&hm=secret");` the `)` in `(1)` ends the span after `?` has already been deleted, so the log line is `url("https://h/a).png&hm=secret")`. There is no `?` left, and `without_queries` keeps `&hm=secret`. The same input with only the word rule loses the whole query. An escaped quote does the same inside `"…"`: `"https://h/a?x=\"a &hm=secret"` stops at the `"` after `\`, and `&hm=secret` stays. Fix: run the cut to the real end of the span — an unescaped matching quote, and for `url(` a `)` that is outside quotes and nested parens — and delete from the first `?` or `#` through that end.

2. `host/linux/src/image/remote.rs:93` — `url(` is only the exact lowercase letters immediately before `(`. `@import URL(https://h/a?x=a &hm=secret);` and `@import url (https://h/a?x=a &hm=secret);` are not spans, the word rule stops at the space, and the warning still contains `&hm=secret`. `URL("…")` is covered by the quote rule. Fix: match `url` case-insensitively and allow whitespace between `url` and `(`.

**Non-blocking**

1. `host/linux/src/image/remote.rs:91` — A closing `"` or `'` is read again as a new opener, so a later `?` in the same warning drops the rest of the line (the fixed “is not read…” suffix has no `?`, so today’s stylesheet warning does not hit this). Fix: consume the closer in the inner loop and do not start a span on it.
