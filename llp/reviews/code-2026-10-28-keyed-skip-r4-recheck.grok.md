# Code review: runner keyed-row skip (x2/keyed-skip), narrow re-check of the co-signer's conditions, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `ee01aa1421` (now ec60ada353 after a plain rebase).
- **Method:** one brief (sha256 `031666c11b274d4ecebe613b2ac1d90b6c5b89d9b9b92011f6cc6b8739686ab9`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's context-compaction notices.
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

I'll re-check only HEAD (`ee01aa1421`) against the co-signer's three conditions, read-only. First I'll read the review brief and the commit itself.The commit only touches `deps.rs` and the position tests. I'll read how `Reads` is built and where `reads_position` is used so I can judge condition (b) against every scan path.`opaque` is set for more than an aborted scan. I'll trace every place a body's `Reads` is built and how `opaque` already forces re-evaluation.The predicate treats every `opaque` body as a position read, while the scan only stops early on undecodable code. I'll check the other `opaque` setters and whether the new tests actually hit the move path.`opaque` is also set for effects that the scan finishes. I'll check whether that makes ordinary rows all-dirty on a move, and whether the new lit test really exercises a kept row.No findings.

`(b)` is correct and sufficient. `scan` stops early only when `vm::instructions` yields a decode error, and that path sets `opaque` and returns (`runner/src/instance/deps.rs:473`). Every body `Reads` is a union of those scans (`deps.rs:356`), and `union` keeps both `opaque` and `positions` (`deps.rs:155`). A `LoadIndex` after the bad instruction is therefore covered by `reads_position()` (`deps.rs:167`). Jumps are not followed, but the byte stream is walked in order, so both arms of a branch are scanned. Handler and action bodies are not part of `deps.bodies`; they run later against the live frame, whose index is updated on every move.

`|| opaque` is wider than that early return. `LoadParam`, `StoreSlot`, `Command`, `Send`, `Refresh`, and `SharedFiles` set `opaque` and the scan continues (`deps.rs:502`, `deps.rs:521`), so their `positions` bits are already complete. A moved row whose template contains one of those is all-dirty. That does not change results: `changed_outside` already treats `reads.opaque` as stale (`runner/src/instance.rs:556`), so the row is entered on every update; the extra effect is `dirty = !0` on a move, which re-evaluates item-field readers. The item object is still replaced only when `changed_fields` is nonzero, so nested memo identity stays. Action bodies are not in the row body, so a row's click `send` does not do this. The compiled case that does is `sharedFiles(...)` (or a parameter load) inside a binding, surface argument, or descendant. That is the formula the commit was asked to implement, and it is conservative.

The far-bit expectation matches `out_of` (`deps.rs:178`). Bit 63 means "63 or farther", so a shift of `s` spreads it from bit `63 - min(s, 63)` upward. Bit 0, `reads_position()`, is set only for `s >= 63`. The test checks that mask literally (`deps.rs:636`), and the same function already shifts `frames`.

The lit test checks the claim. For both the virtualized list and the plain `for` (`position_tests.rs:579`), one update prepends and sets the highlight to the old first item. The kept row is the same kernel root, now at index 1, and its text goes from `["0", "false"]` to `["0", "true"]` (`position_tests.rs:603`). Every other mounted row, including the new first item, stays `"false"`. A second same-commit prepend sets the highlight to the new first item and requires `["-2", "true"]` there and `"false"` on every later row, which still includes the previously lit kept row. `ToString` of a bool is `"true"` / `"false"`, and `Eq` pushes a bool. The slot bit makes the binding stale on its own, so this is the interaction the move optimization has to preserve.

VERDICT: CLEAN
