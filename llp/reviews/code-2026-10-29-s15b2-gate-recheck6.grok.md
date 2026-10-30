# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck6, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `ff487d56de64d83b7b267c34da246d171d04441af2e51b47685245e71c4f2bd0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (the same).
- **Disposition:** taken in abb4cd7a71.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g7-brief.txt. At g7, both found that componentIR read `slot := …` and `view := …` by text search, so a string (a provide's or a state's value) could stand in for them. They also found that Lean strings aren't JSON: the printer's \xHH escape (contract/cli/src/lean.rs `string`).
This commit:
- reads the whole record structurally: a reader of records `{ key := value, … }`, lists, tuples, strings and words. slot and view are taken from the record's own top-level fields.
- decodes strings by the printer's escapes: \" \\ \n \t \r \xHH.
- finds the record by the printer's own form, `{` then a newline and 6 spaces, then `name := "<C>", props :=`.
- adds tests for the provide string masking `slot`, a state string `view := []`, and the \xHH escape.
Check the reader against contract/cli/src/lean/components.rs's printer: every value form it can print (records nested in lists, words like `.str` or `0x…`, negative numbers, anything with `:=` or `{` inside values), the finder's form for root and components, and anything else left.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

I'll re-check only HEAD on the s15b2 gate branch, read-only, and compare the new structural reader with the printer it has to parse.The commit is a structural reader in `scripts/stage12-named.mjs`. Next I’ll compare it field-by-field with the Lean printer.[P2] The record reader crashes on the printer's two-token `.some` values — `scripts/stage12-named.mjs:220`

A field value is read as one bracketed term, one string, or one word (`scripts/stage12-named.mjs:230`). For the root component, `mutations_list` writes `andThen := .some "action"` (`contract/cli/src/lean.rs:486`) and `tasks_list` writes `gate := .some (expr)` and `key := .some (expr)` (`contract/cli/src/lean.rs:543`). After the word `.some`, the next token is a quote or `(`. The field loop treats that token as a key, the key pattern does not match, and `componentIR` throws `TypeError` on the `[0]` at line 220, before it can return `slot` and `view`. The finder does accept this record: the root is the same `component()` form, `root := {\n      name := "…", props :=`. A root with `andThen := .none` or `queue := true` and no gate parses. Child components are unaffected, because the printer fills their `mutations` and `tasks` with `[]` (`contract/cli/src/lean/components.rs:137`).

The rest of the printer's component record matches the reader. Records nested in lists (props, states, derives, resources, actions) stay nested, so a string cannot stand in for a top-level field. Printer-shaped `provides := [("note", (.str "slot := false"))]` before `slot := true` still yields `slot: true`, and `init := (.str "view := []")` leaves the real view. Words such as `.str`, `.neg`, and `0x…` parse; a negative number is `(.unary .neg (.num 0x…))`, one tuple. `\"`, `\\`, `\n`, `\t`, `\r`, and `\xHH` decode to the characters `string()` prints. The finder is `{\n      name := "<C>", props :=` with the six spaces from `component()` (`contract/cli/src/lean/components.rs:143`), for the root and for every child. A string cannot imitate that header: the printer writes a newline as `\n`, not a raw line break. In a view, a `use` name is its string, a `when`'s then/else are its last two lists, and an `element`'s children are the last of its four lists, including when a condition or an argument holds a nested list.

VERDICT: CHANGES
