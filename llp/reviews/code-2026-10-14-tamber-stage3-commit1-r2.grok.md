# Code review: LLP 1121 stage 3, commit 1, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `716a0ea8af3aacadfaafff611468e5c91012d9a797150c23dfd20f0c288bc9b8`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one medium, one low (main moved); Grok: one high (main moved), one medium.
- **Disposition:** fixed in round 2; the RFC text was main having moved

---

## High

**Landed `scheme=fixed` text is gone from the RFC — `llp/1121-tamber.rfc.md:1236`**

This commit deletes token metadata from the grammar, the `scheme=fixed` rules, and the stage 2 follow-up note, and the status line now says that follow-up is still next. The gate text at line 1901 still says "see that follow-up's note," which is no longer in the file. The checker, the formatter, and the canonical-layout test all still implement `ink = #ffffff  scheme=fixed`. Restore `{ meta }` on the `token` production, the follow-up note, and a status line that records `scheme=fixed` as landed and `tamber fmt` as this stage.

## Medium

**An apostrophe in a value swallows a later comment — `tamber/src/lex.rs:69`**

`strip_comment` now treats `'` as a string quote, and that function is what `lines()` uses for every parse, not only for fmt. A `'` after `//` is fine: the scan stops at the slashes, so `// the page's ground` and `// Don't` stay comments. A `'` in the code before `//` opens a string for the rest of the line, and while that string is open a `)` does not close a parenthesis group.

```tamber
brand = -exact-platform-color(web People's, #336699) // brand
```

Before this change the comment was stripped and the line checked (`web` names may contain `'`). Now the value keeps `// brand`, syntax parse still succeeds, the reparse guard only calls `parse`, and `contract tamber fmt` will write the file. `check` then fails. The same cut hits any unmatched apostrophe, as in `name = People's // face`.

Treat `'` as a string only at a token boundary (after whitespace or `= ( , :`), not after an ident character. `People's` and `it's.svg` stay apostrophes; `'http://x'` and `url('http://x')` stay strings.

`words`, `split_top`, and the CSS reader still only know `"`. That is why `font.family = 'a//b'` formats and reparses but does not check. The new fmt test never calls `check` on it.

## Low

None. The round 1 comment, header, `when =`, CRLF, and corpus-check fixes behave as described: a comment's level comes from its own indent, a comment before the first entry takes that entry's level, trailing blanks are dropped before grouping, and a top-level comment opens its own group. A one-line value edit or deleting a `when` under a deeper comment does not move any other line. File sizes are inside the caps (`tamber/src/fmt.rs` is 252 lines; `docs/recipes/tamber.md` is 2378 bytes).
