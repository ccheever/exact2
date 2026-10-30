# Code review: Excord's plain-text fast path, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `03fdc47869`.
- **Method:** one brief (sha256 `815ff5b318a288b16a6291bfc670408ea4ef72f6d2667525ef4613bd582155f4`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The fast path matches the parser for every string it admits. Line-start syntax, inline Discord markup, trailing CR/LF, and the emoji cases that stay on this path all agree, and both Markdown test files pass (333 tests).

`isPlain` only accepts a string when `PLAIN` matches and `trim()` is non-empty. The character class drops every inline or block rule's opening character except `h`, and the `url` rule still needs `:`, which is dropped. The start lookahead rejects every list, subtext, or indented `-`/`+`/`digit` marker this parser actually builds (`*` is excluded everywhere). With no `\n` or `\r` left, `parseMarkdown` keeps a single line and merges it into one text run, which is the AST returned at `markdown.ts:370`. On Bun and Node 26, `$` does not match before a final line break, so `"hello\n"` and `"hello\r"` are not admitted. U+2028/U+2029 are admitted and kept as text on both paths. Keycaps such as `1️⃣`, flags, skin tones, ZWJ sequences, and 27 emoji stay one text run, and `jumbo` is still true. Bare `www.`/`foo.com` text is admitted and is not autolinked.

`markdown-plain.test.ts` pins the cases the plan required: fixture text (both heading modes), line-start and list forms asserted not plain, emoji-only `jumbo`, blank and Unicode whitespace, long strings, and 6,000 mixed strings. A rejected string makes `parse` call `parseMarkdown`, so equality there is trivial. An admitted string is a real deep-equal. The suite also fails if the gate admits nothing.

VERDICT: APPROVE

1. NIT — `apps/excord/discord/markdown.ts:365` — The comment says `markdown.test.ts` holds the fast path equal to the parser. That check is in `markdown-plain.test.ts`. `markdown.test.ts` never calls `parseMarkdown`.
