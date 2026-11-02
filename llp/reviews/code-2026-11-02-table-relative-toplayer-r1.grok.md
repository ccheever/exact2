# Code review: a popover table row's relative position, round 1 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `dc2d098ac5`.
- **Method:** one brief (sha256 `047cabb4791e63e49a31444b94e8f15254d2d97b919d81b9cdc74370c747b2db`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** nothing to address.

`table::relative` (`kernel/src/layout/table.rs:192`) now takes `layout_position`. For an ordinary row that matches `arena.style().position_type`. `position` is not inherited (schema bit 23). The only platform start that sets it is the neutral route header (`kernel/src/arena/defaults.rs:19`, metric `header-position` = sticky). Sticky and the stored initial static both fail the `Relative` check, so the offset stays zero. An authored `relative` row carries the bit on its own style, so `computed_source` returns that style, and the `left`/`top` read from `arena.style()` is the same value as before. An authored sticky popover stays sticky in `of` and still gets no relative offset.

A popover row should not pick up that offset on another path. `top_layer::apply` already forces the engine box absolute, and `block::apply` clears the insets so the host places the box (`kernel/src/kernel/block.rs:258`). Cells are grid items, so publication was the only place their frames gained the author's `left`/`top` (`kernel/src/layout/publication.rs:237`). Returning zero puts the cells on the row. A relative row group's offset still reaches a child published inside it. A row that has taken its viewport block is skipped (`publication.rs:248`).

No other direct `position_type` read remains in `table.rs` or `publication.rs`. The engine mapping in `kernel/src/style.rs:1268` is on the allowlist and is overwritten by `top_layer::apply` before layout.

The new test sets `popover=manual` on `tr` id 4, then `position: relative`. Both kernels share that popover; only `left`/`top` differ. Those insets move the cell only while `layout_position` is still `Relative`, which is why the test failed before this commit. `cargo test -p exact-kernel --test it table::` passed here (24 tests, including this one and `a_relatively_moved_row_or_group_moves_its_cells`). `bun test scripts/position-reads.test.mjs` passed (3 tests).

Swift already implements the same rule on `NodeView.layoutPosition`. Apple and Linux place from the published frames and do not recompute this offset. The web hosts emit the authored row and sit outside that scan on purpose.

No findings.

VERDICT: APPROVE
