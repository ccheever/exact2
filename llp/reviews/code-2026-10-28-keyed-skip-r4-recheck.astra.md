# Code review: runner keyed-row skip (x2/keyed-skip), narrow re-check of the co-signer's conditions, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `ee01aa1421` (now ec60ada353 after a plain rebase).
- **Method:** one brief (sha256 `031666c11b274d4ecebe613b2ac1d90b6c5b89d9b9b92011f6cc6b8739686ab9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed. theme2's follow-up condition, a test of an opaque body re-evaluating on a move, is b80a714932.

## Brief

Narrow re-check of ONE commit, HEAD (ee01aa1421), on top of a reviewed branch: `git show HEAD`. Read-only: no checkout, restore, stash, reset or edits.
Context: the runner keyed-row skip. Its earlier commits had 3 rounds, the last two clean; the brief is at /tmp/iosfix-rv-keyed1-brief.txt. This commit takes the co-signer's conditions:
(a) a deps unit test that `positions` shifts exactly as `frames` does through union/out_of, with the sticky far bit 63;
(b) a body whose reads come from a scan that stopped at undecodable code (opaque) now counts as reading its position: reads_position() = positions bit 0 || opaque;
(c) a test that a `lit=(item == slot)` change in the SAME commit as a prepend shows on a kept, moved row (the old first row, no longer first) and on the newly first row, for both the virtualized list and the plain `for`.
Check: is (b) correct and sufficient? Does any other path build a body's Reads without a complete scan? Does making opaque bodies all-dirty on a move change anything else, for example perf or an opaque body that was never meant to be all-dirty? Is the far-bit expectation in the test the right semantics? Do the tests check what they claim?
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings in HEAD (`ee01aa1421`).

- **(b) is correct and sufficient for incomplete scans.** The only early exit from `scan` sets `opaque`; body construction preserves it through every union. Other construction paths use the same scanner. Inline callbacks and branches are scanned too. See [deps.rs:469](/tmp/iosfix-rv-keyed4/runner/src/instance/deps.rs:469).
- **The fallback is conservative, with possible extra work.** `opaque` also covers `SharedFiles`, parameters and effects. Their expressions already reevaluate unconditionally; marking a moved row dirty additionally reevaluates other frame-dependent bindings. This restores the previous move behavior for those rows. Fully scanned, nonopaque rows retain the optimization. See [deps.rs:167](/tmp/iosfix-rv-keyed4/runner/src/instance/deps.rs:167).
- **The far-bit expectation is correct.** Bit 63 represents depths ≥63. Moving outward by `s` therefore spreads it across depths ≥`63−s`; at ≥63 shifts, every bit must remain possible. The [unit test](/tmp/iosfix-rv-keyed4/runner/src/instance/deps.rs:619) checks that behavior and the opaque predicate, though it does not exercise malformed-code scanning end to end.
- **The prepend test checks its claim.** Both slots change before one update. It verifies the old first row retains its identity, moves to second and becomes lit; another simultaneous prepend/highlight change lights the new first row and clears the others, for both implementations. See [position_tests.rs:576](/tmp/iosfix-rv-keyed4/runner/src/instance/collection/position_tests.rs:576).

Static review only; tests were not rerun under the read-only constraint.

VERDICT: CLEAN
