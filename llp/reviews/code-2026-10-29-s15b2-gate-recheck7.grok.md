# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck7, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `af3e5f9d2e36d96ae4891427254f3368c04d77536a0abb66f745253cee875300`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** clean.
- **Disposition:** none needed.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g8-brief.txt. At g8, both found that a field's value can be an unparenthesized application (`gate := .some ((.bool true))`, `key := .some (…)`, `andThen := .some "done"` in a root's tasks and mutations), which threw the reader.
This commit reads a field's value, or a list item, as its terms up to the next comma or the closing bracket, grouped as {app: [...]}. Tuples still read space-separated terms. There's a test with those three forms. The copy's real root (Bluesky, with tasks) now parses.
Check the reader against every form contract/cli/src/lean/components.rs (and the expression printer) can print inside a record or list: applications in list items, a comma inside nested brackets or strings, `.none`, a trailing comma. Anything left?
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only `HEAD` on the s15b2 gate branch, read-only, against the g8 finding and every record/list form the printer can emit.No findings.

The reader stays aligned with every record and list form `contract/cli/src/lean/components.rs` and `Emitter::expr` print:

- An unparenthesized application is either a field value (`gate := .some (expr)`, `key := .some (expr)`, `andThen := .some "…"`) or, if it shows up as a list item, the same run of terms up to the comma or closing bracket.
- A comma inside nested `()`, `[]`, `{}`, or a string stays inside that term, including `]`, `}`, and `:=`.
- `.none` is one word as a field, a list item, or a tuple term.
- A trailing comma is skipped before the next field, the next item, or the closing bracket.
- Every expression, statement, and view node is already one parenthesized term or `.none`, so a view node stays a tuple. A gate expression whose commas sit inside those parens does not consume the following field.

VERDICT: CLEAN
