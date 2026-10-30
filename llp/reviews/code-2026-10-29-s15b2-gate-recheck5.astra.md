# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck5, 2026-10-29 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `fede0c3eca44c6e408c1f6f35b29d1bf12a668528a2f5f2cce09d115da94ebce`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium: P2).
- **Disposition:** taken in b679167cd9.

## Brief

Narrow re-check of ONE commit, HEAD (`git show HEAD`), on the s15b2 gate branch. Read-only: no checkout, restore, stash, reset or edits.
Earlier briefs: /tmp/iosfix-rv-g1..g6-brief.txt. At g6, both found textual-scan holes in declaresSlot (a contextual keyword ending the scan early; `slot` alone on a line as a value).
This commit REMOVES every source-text scan. The binding's shape now comes from the compiler's own parse:
- `contract lean <app> --components` prints each component as a Lean record: slot := true/false, and view := [(.use n "Name" [args] [children]), (.when n cond [then] [else]), (.element ...)]. See contract/cli/src (the lean printer) and the copy, e.g. run nothing, but the shape is in the test fixtures.
- componentIR(lean, component) reads the named record (`name := "<C>", props :=`) and parses its view with a small reader of lists, tuples, strings and words.
- irShape requires: no slot of its own; exactly one use of PersonRow; under it nothing, or one `when` with no else whose only child is one use of FollowButton with nothing under it.
- consumerPixels runs `exact.mjs contract lean app.contract --components --name <first binding>` in the copy once, and checks each binding. The map checks stay (bindingsCheck, bindingShape: package identities, slot placement, nothing else in the slot).
Check:
- the reader against the printer's real output: escapes in strings, nested lists in arguments, number words like 0x4044000000000000;
- that the record finder can't hit a use or a string (`name := "<C>", props :=` is the declaration's form);
- that the lean printer's view keeps if/else/for/match distinct (`.when` vs others), so an else, a for or a match fails;
- that a slot declaration always shows as slot := true;
- anything left.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [scripts/stage12-named.mjs:210](/tmp/iosfix-rv-g7/scripts/stage12-named.mjs:210): Field searches still match string contents.** The printer emits `provides` before `slot`, so `provide note = "slot := false"` masks a subsequent real `slot := true`; an otherwise valid row passes the no-slot check. Likewise, `state note = "view := []"` makes line 211 select that string’s empty list and reject a valid view. Read both fields structurally, outside strings and nested values.

- **P2 — [scripts/stage12-named.mjs:224](/tmp/iosfix-rv-g7/scripts/stage12-named.mjs:224): Lean strings are not JSON strings.** The real printer emits control characters as `\xNN` (`contract/cli/src/lean.rs:172`). A legal argument containing `"\u{7f}"` therefore becomes `"\x7f"`, and `JSON.parse` throws, aborting the gate. Decode the printer’s escape syntax.

Static review only; no tests or compiler runs.

VERDICT: CHANGES
