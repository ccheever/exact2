# Code review: LLP 1121 stage 3, commit 1 (tamber fmt), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `73a51d422820424a80b473a9d92b29b76e55a189c74473e84cfc20b2da646358`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, two low; Grok: one high, three medium.
- **Disposition:** all fixed in round 1

---

## Summary

`tamber fmt` prints a parsed design in the layout this commit describes: block order kept, two-space indents, value text kept, metadata and dial ranges after the value, blank runs inside a block collapsed to one. Quoted metadata, `=` inside double quotes and parentheses, multi-condition `when` lines, exports, role-spelled names, CRLF (rewritten to LF), and trailing whitespace round-trip. Tab-indented input is refused with `tamber-indent` and is not rewritten. The CLI `--check` path does not write, and `bun exact.mjs tamber fmt` forwards the same flag. Two high bugs: canonical spacing turns a token named `when`, and an axis value named `under`, into text the parser rejects, and the CLI writes that text and exits 0. Two medium bugs move comments (a blank between a comment and the block it precedes, and a comment at the end of a block). No low issues. Single quotes are not string delimiters in this lexer or in Contract, so `//` inside `'…'` was not filed.

## Issues

### Issue 1 -- Severity: bug
- File: tamber/src/fmt.rs:183
- Description: HIGH — A token whose name is `when` parses (`when` is an identifier and is not in `check::reserved`), but `token` always reprints `name = value` with spaces around `=`. The parser then rejects that line at entry indent because it `starts_with("when ")` (`syntax.rs:447` treats every such line as a misplaced when-line). `format` returns `Ok` without re-parsing, so `contract tamber fmt` writes the file and exits 0. `format` of the result errors, so the printer is not idempotent and the IR is gone. Input `tamber t\n\ntheme\n  when=#fff\n` (also `when= #fff` and `when=#fff scheme=fixed`) prints `tamber t\n\ntheme\n  when = #fff\n`, and parsing that yields `tamber-syntax`: `a when is indented under its token`. `when: color = #fff` is fine, because the line does not start with `when `.
- Suggestion: At entry indent, accept `when = <value>` as a token named `when` (the left of `=` is only `when`), and keep rejecting a real when-line (`when <axis>=<value>…: value`) that is not indented under its token. Have `format` return the parse errors if the printed text does not parse, so a bug like this cannot be written. The fixed-point test never uses this name; add `when=#fff` and `when=#fff scheme=fixed`.
- Status: open

### Issue 2 -- Severity: bug
- File: tamber/src/fmt.rs:122
- Description: HIGH — An axis value whose identifier is `under` parses when it is not written with spaces around that word (`tone=under|soft`, `tone=under|soft under scheme=dark`, `tone=a|under|b`). `axis` then joins values with ` | ` (`fmt.rs:127`) and detects the clause with the first substring ` under `, the same detector as `syntax.rs:289`. The spaced line is a different parse: `tone = under | soft` is read as an `under` clause whose right side is `| soft`, which is not `scheme=dark`, so the formatted file does not parse. `tone = under | soft  under scheme=dark` hits the value `under` before the real clause. `format` still returns `Ok`, and `contract tamber fmt` writes it and exits 0. The second format errors. A last value `under` with no following value and no clause (`tone=soft|under` → `tone = soft | under`) does not contain ` under ` and stays stable. `under` as an axis or token name is fine.
- Suggestion: Treat `under scheme=dark` as a trailing clause only (the right side of the match must tight-equal `scheme=dark`; if several ` under ` occur, take the last such match), in both `fmt::axis` and `syntax::axis`, so a value named `under` survives ` | ` spacing. Re-parse the printed text before returning `Ok`. Add a fixed-point case for `under|soft` and for `under|soft` plus `under scheme=dark`.
- Status: open

### Issue 3 -- Severity: bug
- File: tamber/src/fmt.rs:86
- Description: MEDIUM — A comment's level is the next code line, skipping blanks, but `opens` stops at the first non-comment, including a blank. A top-level comment that precedes a block with a blank between them therefore does not open that block. If the previous emit was a level-0 header, `opened` is still true, so the blank before the comment is dropped and the comment is glued to that header. Input `tamber t\n\ntheme\n\n// c\n\nexports\n  ink\n` prints `tamber t\n\ntheme\n// c\n\nexports\n  ink\n`. The same move happens between the header and the first block: `tamber t\n\n// about axes\n\naxes\n  tone = a | b\n` becomes `tamber t\n// about axes\n\naxes\n  tone = a | b\n`. Values and the folded IR stay put; the comment does not stay in place, and inserting that one blank is a multi-line diff. A comment already hard against the next header (`// c\nexports`) is left alone, so the result depends on a blank the printer otherwise treats as transparent.
- Suggestion: Skip blanks in the `opens` lookahead the same way the reverse level walk does, and when deciding whether a level-0 header is already attached to a preceding level-0 comment, look through blanks too, so the comment stays directly above the block it precedes with a single blank before the group. Cover both inputs in `tamber/tests/fmt.rs`; the corpus test only counts `//` and will not see a move.
- Status: open

### Issue 4 -- Severity: bug
- File: tamber/src/fmt.rs:66
- Description: MEDIUM — `next_level` starts at 0 and the reverse walk assigns a comment before it has seen the code above it. A comment with no following code line (end of a block, end of the file) is therefore level 0, and a level-0 comment opens a group, which inserts a blank and dedents it out of the block. Input `tamber t\n\ntheme\n  ink = #fff\n    when tone=hard: #000\n    // why\n` prints the `// why` at column 0 with a blank line above it, not under the `when`. The same happens for `  // end` after the last token or export. File-start comments are fine (`// The brand.\n` stays on the header, with or without a blank before `tamber`). This contradicts "comments kept in place": adding an indented comment at the end of an already formatted block changes the indent and inserts a blank, so the edit is no longer one line.
- Suggestion: A comment with only blanks after it should take the level of the code line above it (the block entry, or the `when` line), and should not open a new top-level group. Add that case next to `the_canonical_layout`, which only checks comments that precede a code line.
- Status: open
