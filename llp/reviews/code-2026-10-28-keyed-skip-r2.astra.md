# Code review: runner keyed-row skip (x2/keyed-skip), round 2, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `17270cbd32`.
- **Method:** one brief (sha256 `eca57ce940a810879f9f98854e9b3ba954acdc49add3a4f750f523d17f9f7a24`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.
- **Disposition:** none needed

## Brief

Round 2 of 3 reviewing the branch: diff origin/main..HEAD, two commits. Read-only: no checkout, restore, stash, reset or edits.
It is a runner change (exact-runner), runtime level. Round 1's brief, with the change, the position audit and the measurements, is at /tmp/iosfix-rv-keyed1-brief.txt; read it first.
Round 1:
- Grok was CLEAN.
- Astra had one P2: the fresh-build oracle in after_a_prepend_the_rows_equal_a_fresh_build_of_the_new_list reused the incremental path. It is FIXED in 17270cbd32: the reference is now built by Harness::from_parts from the prepended list and the final slot, it has 21 rows, and forcing the moved rows never-dirty turns it red.
- Astra also qualified audit item 2: a key that mixes item and position can keep its key while the row moves, under a conditional. That's still safe, because shared-key reuse rejects LoadIndex and kept rows get the item comparison and the index update. Please confirm.
Review the whole diff afresh, not only the fix. Look in particular at the following.
- Any path where a kept row's OUTPUT depends on its position, its neighbours or its order without a LoadIndex in its body: separators, wrappers, the collection's emitted props, sticky headers or sections, focus, selection, swipe actions, the reorder or drag state, accessibility order.
- The plain `for`'s object reuse when dirty == 0 (`frame.item = r.frame.item.take()`), now reached by moved rows.
- update_row's handling of a dirty mask that isn't !0 for rows with row slots or memos.
- The tests' strength.
Checked: runner 762/0 and contract 2619/0 on the first commit; position tests 14/14 on the tip; fmt.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No P1/P2/P3 findings in the full two-commit diff.

- Confirmed the conditional mixed-key case is safe: [rekey.rs:137](/tmp/iosfix-rv-keyed2/runner/src/instance/collection/rekey.rs:137) rejects shared-key reuse for `LoadIndex`; surviving rows still receive item comparison and the new index.
- Plain `for` object reuse preserves an equivalent item without losing the incoming index or row slots. Partial dirty masks retain outer-input, row-slot, and nested-region invalidation.
- Found no missed dependency through separators, wrappers, collection props, sticky/section layout, focus, selection, swipe actions, reorder state, or accessibility order.
- The [fresh-build oracle:539](/tmp/iosfix-rv-keyed2/runner/src/instance/collection/position_tests.rs:539) now independently constructs the final state and asserts 21 rows. The other tests check retained identity, output changes, and evaluation counts.

Read-only review; tests were not rerun. `git diff --check` passed; the worktree is unchanged.

VERDICT: CLEAN
