# Code review: runner keyed-row skip (x2/keyed-skip), round 1, 2026-10-28 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `01e35bcdea`.
- **Method:** one brief (sha256 `b3a9b6cc00e35571951304936a5edf8f3cbd1524fd19af22f17f91a1d24ef337`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium: P2).
- **Disposition:** taken in 17270cbd32 (the fresh-build oracle is built from the new list at creation). The audit item 2 qualification is accepted; round 2 confirmed it.

## Brief

Round 1 review of the branch: diff origin/main..HEAD (one commit). Read-only: no checkout, restore, stash, reset or edits.
It is a runner change (exact-runner), runtime level, 3 rounds. The plan, approved by theme2, is quoted below.

THE CAUSE: a prepend moves every mounted row of a keyed list. Collection::reposition (runner/src/instance/collection/mod.rs) and the plain `for`'s RegionInst::update_item (runner/src/instance/region.rs) marked EVERY field dirty for any row that moved ("a row that moved reads its new one", LLP 1062 D8). So every kept row re-evaluated to the same result: in Excord, 1,173 of 3,178 evaluations over 3 history pages.
THE CHANGE: deps.rs Reads gains `positions: u64`, by relative frame depth like `frames`. scan sets bit k for a LoadIndex of frame k; union shifts it as it does `frames` (out_of). bodies[region].reads_position() (bit 0) says whether anything in the row's body, at any depth, reads the row's own position (`each item, i in list`), an inner `for` reading the outer `i` included. A moved row is all-dirty only when its body reads its position. Otherwise its dirty fields are compare::changed_fields of its item, as for an unmoved row. frame.index is still updated on every move. Whether a binding re-evaluates otherwise stays the existing tracking (Update::stale: slots, derives, resources, the clock, outside frames, row slots, opaque reads).
The two other reposition callers (settle paths) pass reads_position=true; they keep the old behaviour.

POSITION AUDIT (theme2's condition: every other way a row's output can depend on its position without reading `i`, each tracked or kept all-dirty, with a test where a prepend changes it). Please CHECK this list for completeness and correctness; finding a missed path is the most valuable thing you can do:
1. LoadIndex (`i`), at any depth, nested `for` included: tracked (positions bits). Tests: a_prepend_reevaluates_and_renumbers_every_kept_row_that_reads_its_position, a_plain_for_prepend_renumbers_every_kept_row_that_reads_its_position, a_row_whose_inner_for_reads_the_outer_position_renumbers_after_a_prepend (virtualized and plain).
2. Keys that read `i`: a row keyed by its position (`key=i`; is_index_key, region.rs:137) IS its position, so it never moves; a prepend changes its item, and compare::changed_fields dirties the readers of what changed. A key mixing `i` with the item gives every row a new key on a prepend, so all rows are new. Neither reaches the moved-row path. Please verify this for the virtualized collection's keying too (collection/mod.rs, rekey.rs).
3. aria-posinset / aria-setsize on the row wrapper: views::publish_position, on every settle, independent of dirty. Test: a_kept_row_that_reads_no_position_still_publishes_its_new_place.
4. A grouped list's first and last rows (corners, separators): the kernel's, from child order (Kernel::grouped_list). Test: a_grouped_lists_old_first_row_is_no_longer_first_after_a_prepend.
5. Selectors such as first, last or nth: none exist in Contract/Tamber (LLP 1121 §4).
6. Sibling-derived first/last (an app passing `first = m.key == rows[0].key`): reads outside state (a slot, derive or resource), so it's tracked by Update::stale. Test: a_row_reading_outside_state_updates_when_it_changes_and_its_item_does_not_moved_or_not (both kinds).
7. Event handlers reading `i`: dispatch reads the live frame (runner/src/runner/event.rs dispatch_inner -> find), and frame.index is updated on every move.
8. Stagger and animation indices read `i` (case 1).
9. A changed item on a moved row: only the changed fields' readers. Test: a_moved_row_whose_item_changed_reevaluates_only_its_changed_fields_readers. Equivalence to a fresh build: after_a_prepend_the_rows_equal_a_fresh_build_of_the_new_list.
Also check: memos and nested regions inside a row; derives/locals in a row body (row slots, LoadBound); measurement tokens and revision; anything that caches by position (heights, the anchor) that relied on a re-evaluation; whether `positions` being OR-ed in union is right for `opaque` and `fields = !0` paths; the far bit (63) in out_of.
Checked already: runner 762/0, fmt, clippy -D warnings. The contract suite is running. Measured (release, a plain `for` of 300 rows, one row prepended): with the change 150 µs and 2 bindings evaluated; with all-dirty 254 µs and 302. Rows reading `i`: 363 vs 404 µs (603 bindings, both), so the bit test costs nothing measurable.
Findings P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

- **P2 — [position_tests.rs:538](/tmp/iosfix-rv-keyed1/runner/src/instance/collection/position_tests.rs:538): The fresh-build oracle uses the same incremental prepend path.** `plain(...)` already creates the original 20 rows; replacing its slots and calling `update()` reuses those rows. Both sides can therefore retain identical stale output and pass. Construct the reference with `Harness::from_parts(...)` using the prepended list and final slot values from creation.

No runtime defect found in the audited paths, including nested dependencies, live event frames, measurement invalidation, and the conservative bit-63 shift.

Audit #2 needs qualification: a key mixing item and position does **not necessarily** change on every prepend. Conditional expressions can retain keys while rows move. This remains safe here: collection shared-key reuse rejects `LoadIndex`, and retained rows still receive item comparison and unconditional index updates.

Tests were not rerun: this read-only checkout has no build artifacts.

VERDICT: CHANGES
