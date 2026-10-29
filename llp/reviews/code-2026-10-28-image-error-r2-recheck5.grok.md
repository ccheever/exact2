# Code review: the image error event on Linux, Android and Windows, round 2, re-check 5 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `75086ffa79`.
- **Method:** one brief (sha256 `237606eed6793c2eb24af32c7cd79f4115990a3e31db237a0487a4a89d9d338a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `host/linux/src/image/remote.rs:45` and `host/apple/Sources/ExactKit/RasterRetry.swift:88` — A closing mark still ends the drop when the next character is `,`, `;`, `:`, or `.`. Those are legal inside a query, so the tail is logged. `` `https://h/a.svg?file=(1).png&hm=secret`: x `` becomes `` `https://h/a.svg).png&hm=secret`: x ``. Same for `url(a.css?file=(1).png&hm=secret)` → `url(a.css).png&hm=secret)`. The `)&` test stays safe only because `&` is not in that set. End the drop on whitespace, end of string, or a closer that matches a delimiter opened before the `?` or `#` (carry an unmatched `(` across one space so `url( a.css?hm=secret);` still keeps its `);`). Do not treat `,;:.` as ending an unmatched `)`. Add that `(1).png&hm=secret` case on both hosts.

2. `host/linux/src/image/remote.rs:52` and `host/apple/Sources/ExactKit/RasterRetry.swift:94` — On a link, a `?` or `#` whose next character is `)` `"` `'` `` ` `` `>` or `]` never starts a drop, so the whole query stays. `` `https://h/a.png?)ex=1&is=2&hm=secret`: x `` is unchanged, signature included. `why?)` is not a link and should stay. When the word is already a link, start the drop even if the next character is a closing mark, then use the end rule above. Keep the current start rule only for a word that is not a link.

**Non-blocking**

none
