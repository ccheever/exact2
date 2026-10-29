# Code review: runner keyed-row skip (x2/keyed-skip), round 1, 2026-10-28 (grok)

- **Family:** xAI. Grok CLI, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `01e35bcdea`.
- **Method:** one brief (sha256 `b3a9b6cc00e35571951304936a5edf8f3cbd1524fd19af22f17f91a1d24ef337`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole plain output, unedited except for the CLI's two context-compaction notices.
- **Verdict:** clean.
- **Disposition:** none needed

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

I'll review the one commit on this branch against `origin/main`, read-only, and check the position-audit list for any path a moved row can still depend on without reading `i`.This is a read-only review of the one commit against `origin/main`. I'll inspect the diff and the position-tracking paths the plan claims are covered.The diff is five runner files. Next I’ll read the scan, union, and both reposition paths against the audit cases.No findings.

VERDICT: CLEAN

A moved row's output depends on its position only through `LoadIndex`, and that bit survives every region boundary the body is folded through. `Reads::positions` is shifted with the same `out_of` as `frames`, and `reads_position` is bit 0 of the row body after those shifts. A direct `i`, an `i` inside an `if` or `match` arm, and an inner `for` reading the outer `i` all land on that bit. The nested-`for` test is that shift. An arm is a region frame, so the arm body's depth-1 `LoadIndex` is shifted once in `regions[]` (`deps.rs:360-363`) before the row body unions it at shift 0.

The other audit cases hold:

- **Keys.** `is_index_key` (`deps.rs:460`) is only the plain `for` fast path, and there the index never changes, so the moved-row path is not taken. A virtualized `key=i` still rekeys: `rekey_shared` bails on any `LoadIndex` (`rekey.rs:137-140`) and `rekey_full` evaluates the key at the new position unless the same object stayed at that same position. A key that actually changes makes a new identity. A key that reads `i` but still spells the same text is the same row, and the body does not display `i`.
- **`aria-posinset` / `aria-setsize`.** `settle_mounted` publishes them whenever `(position, count)` changes (`mod.rs:1034-1036`), whether or not the row re-evaluates.
- **Grouped first and last.** `Kernel::grouped_list` walks current children. `emit_children` reorders wrappers on every settle.
- **Selectors.** No `nth` / `:first-child` / `:last-child` in Contract, Tamber, the kernel, or the runner.
- **Outside state and `indexOf`.** A slot, derive, or resource bit still makes `Update::stale` true with `dirty == 0`.
- **Handlers.** `find` clones the live row frame (`find.rs:48`, `102`). `reposition` writes `frame.index` on every move, including when the dirty mask is not `!0`.
- **Stagger.** It is an `animation` template that loads `i` (LLP 1062 D7–D8), so it is case 1.
- **Changed fields.** A move with `reads_position == false` uses `changed_fields`, same as an unmoved row. Non-records that differ are already `!0`.

Memos, slots, measurements, and the mask edges:

- An equivalent item keeps its object on the collection path even when the return is `!0` (`mod.rs:1012-1021`). The plain `for` still replaces the object when dirty is `!0`; that is the old all-dirty behavior, and rows that do not read a position now keep the object too. Derives run with an empty frame list. `let` and `map` / `filter` / `sortBy` callbacks are inlined and bind locals, not frames, so an outer `i` stays a `LoadIndex` in the same body the scan walks. Row-slot initializers run only at create or rebind; state is kept with the key, and the old all-dirty path did not re-run them either.
- Heights and measurement tokens move with the key (`replace_keys_indexed` / `splice_keys`). A prepend takes the `invalidate_all` path because the length changed, independent of row dirty. The anchor is captured and restored around that, not from a re-evaluation.
- `positions` is a subset of `frames`: every `LoadIndex` sets both, and both go through `out_of`. `opaque` and `fields = !0` do not set a position bit, and they should not. `opaque` already forces `stale`. `fields = !0` means the item was read without field precision. OR is the right union. Bit 63 is the same sticky spread as `frames`: depth 62 shifts onto bit 0 exactly, and depth 63 spreads onto bit 0 after 63 shifts, so a real read of this row is not dropped. The only extra is a false all-dirty at 63-deep nesting.
- The two settle callers pass `reads_position=true` and ignore the mask. `reposition` still updates the index and the item. That matches the old behavior.
