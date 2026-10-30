# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck6, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `ff487d56de64d83b7b267c34da246d171d04441af2e51b47685245e71c4f2bd0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium: P2).
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

- **P2 — [scripts/stage12-named.mjs:225](/tmp/iosfix-rv-g8/scripts/stage12-named.mjs:225): Record values can contain unparenthesized applications.** The printer emits `gate := .some ((.bool true))`, `key := .some (...)`, and `andThen := .some "done"` for root tasks and mutations. `read()` consumes only `.some`; the record loop then treats its argument as another field and throws at line 220. Reproduced all three failures with otherwise valid root records. Consume the complete field value before advancing to the next key.

The new regression test and additional probes for nested records, strings, expressions, and root/component discovery passed.

VERDICT: CHANGES
