# Code review: runner keyed-row skip (x2/keyed-skip), round 2, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `17270cbd32`.
- **Method:** one brief (sha256 `eca57ce940a810879f9f98854e9b3ba954acdc49add3a4f750f523d17f9f7a24`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's two context-compaction notices.
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

I'll review the branch diff read-only, starting with the round 1 brief and the two commits against `origin/main`.The brief is in. I'll use the defect-first review process and inspect the full two-commit diff, including the oracle fix and the position-dependent output paths.The production change is small. Next I’ll read the dirty-mask, keying, and row-update paths the new `reads_position` gate now reaches.Keying and the dirty mask look conservative so far. I’ll check outputs that follow a row’s place without a `LoadIndex` in the body.No findings.

Astra's note on audit item 2 holds. A conditional key such as `flag ? i : item.id` can keep its identity while the row moves. That row still gets the item comparison and a live index.

- `bodies` does not include the key. `Deps::new` folds the key into `regions` at shift 1, and `reads_position` is read from `bodies` (`deps.rs:354-364`, `deps.rs:165-167`). A `LoadIndex` that exists only in the key does not all-dirty the row.
- The virtualized shared-key path bails out on any `LoadIndex` in the key, taken or not (`rekey.rs:137-140`), so the full rekey runs. Reuse there is positional: the key is copied only when the item at that same index is `same()` (`rekey.rs:54-63`). A prepend fails that check, and the key is evaluated at the new index. If the conditional still returns the old key, `reposition` compares the item and writes `frame.index` either way (`mod.rs:1011-1021`).
- The plain `for` has no shared-key shortcut. On a length change it always evaluates the key (`region.rs:177-186`) and then matches by identity.

Checked the other output paths. Separators, grouped corners, swipe content, and accessibility order come from child order at read time (`Kernel::grouped_list`, `Kernel::row_separator`, `row_of`). A moved plain row sets `roots` because `found != Some(position)` (`region.rs:265`), and the parent emits `SetChildren` (`instance.rs:1169-1177`). The collection reorders in `emit_children` after `sort_by_key` (`mod.rs:997-999`, `views.rs:184-219`). Wrapper `aria-posinset` / `aria-setsize` are published on settle whenever `(position, count)` changes (`views.rs:133-143`, `mod.rs:1034-1036`). Focus and reorder pins are view id and key (`mod.rs:729-743`); a data change ends a reorder preview before rows move (`mod.rs:555-557`). Sticky placement is layout, not a binding. None of these wait on a `LoadIndex` in the row body.

The plain `for` reuse `frame.item = r.frame.item.take()` when `dirty == 0` (`region.rs:318-320`) is the same rule the collection already used: an equivalent item keeps its object. It now applies to a moved row whose body does not read its position. `equivalent` is the comparison, and the new index is still written onto the frame. A body that does read its position still gets `!0`, so the new object is installed and the row re-evaluates.

`update_row` with a partial mask is the unmoved-row path. `enter` records that mask as `dirty_fields` (`instance.rs:573-581`). A binding is stale when a field it reads is in the mask, an outside input changed, it is opaque, or a row slot it reads was written (`instance.rs:551-568`). A row slot is not re-initialized on move. Initializers run only when the row is created (`region.rs:426-441`), which was already true when a move forced `!0`. An inner `for` that reads the outer index is stale through the shifted dirty frame, not through its own `reads_position` bit. That is what `a_row_whose_inner_for_reads_the_outer_position_renumbers_after_a_prepend` covers.

The tests would catch the failures that matter. The fresh oracle is a real `Tree::create` from the prepended list (`position_tests.rs:539-542`, `tests.rs:186-192`), 21 rows, and the index and nested variants go red if moved rows stay clean. The field test expects exactly 3 evaluations (`position_tests.rs:520`). The two `reposition(..., true)` callers ignore the returned mask. On a data update those rows are updated or rebound with `!0`. On a scroll fill the index does not change.

VERDICT: CLEAN
