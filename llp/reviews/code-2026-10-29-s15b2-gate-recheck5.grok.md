# Code review: the s15b2 gate profile (x2/s15b2-gate), recheck5, 2026-10-29 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree of the branch's commit under review.
- **Method:** one brief (sha256 `fede0c3eca44c6e408c1f6f35b29d1bf12a668528a2f5f2cce09d115da94ebce`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
- **Verdict:** request changes (1 medium, 1 low).
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

I'll re-check only HEAD on the s15b2 gate branch, read-only, against the g6 scan holes and the new compiler-parse shape. First I'll read the earlier briefs and the commit itself.The first shell call was blocked. I'll read the earlier briefs and retry a strictly read-only look at HEAD.The commit drops the source scan and reads shape from `contract lean --components`. Next I’ll compare the reader with the printer’s real output.The printer and the reader are next to each other now. I'll run the reader against printer-shaped records, including escapes, nested lists, and a slot flag hidden in an earlier string.The slot flag is read as text, and a string the printer emits earlier in the same record can hide a real `slot` declaration.

**P2** `scripts/stage12-named.mjs:210`
`componentIR` takes the first `slot := true` or `slot := false` in the record. The lean printer writes `provides` before that field (`contract/cli/src/lean/components.rs:143`), and a provide value is any expression. `provide note = "slot := false"` is legal Contract (same form as `accent = "#c0392b"`) and prints as `(.str "slot := false")` ahead of the real `slot := true`. The regex matches inside the string, so `ir.slot` is false and `irShape` accepts the component when the view is otherwise one `PersonRow` and the `if` around one `FollowButton`. A `children` node stays in the parsed view, and `irShape` rejects that view (`PersonRow, a children`, or `a children` in the row's slot). A binding that only declares `slot` passes.

**P3** `scripts/stage12-named.mjs:224`
View strings go through `JSON.parse`. The printer's `\xHH` form for other controls (`contract/cli/src/lean.rs:172`) is not JSON, so `componentIR` throws and the gate line fails closed. `\"`, `\\`, `\n`, `\t`, `\r`, nested argument lists, and number words such as `0x4044000000000000` parse, and a use's last list stays its children.

Also checked, and holding: the record finder looks for `name := "<C>", props :=` with raw quotes, which is only the declaration. A use is `(.use n "C" …)`, and a string that contains those words has `\"`. `if` prints as `.when` (the else is its last list), `for` as `.each`, `match` as `.matchN`, so an else, a `for`, or a `match` fails `irShape`. A slot declaration prints `slot := true` (`c.slot` as a bool). The map checks are unchanged.

VERDICT: CHANGES
