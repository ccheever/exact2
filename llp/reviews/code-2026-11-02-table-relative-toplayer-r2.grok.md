# Code review: a popover table row's relative position, round 2 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `dc2d098ac5`.
- **Method:** one brief (sha256 `4e29dd3ecdcd7f43f46ad758c4d93670dba86e22eb1136a7c2b931a8fc0d729b`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** Astra's MAJOR (a popover row laid out before any viewport, with no block of its own) is put to the lead; see the landing note.

No findings.

`relative` (`kernel/src/layout/table.rs:192`) applies an offset only when `layout_position` is `Relative`. That accessor and `arena.style().position_type` diverge in two places: a popover whose author did not write `sticky` (forced `Absolute`), and a neutral route header’s platform start (`-exact-header-position` is `sticky`, `kernel/src/style/metrics.rs:336`). `position` is not inherited, and no other style start sets it. A sticky header row is not `Relative`, so an ordinary row or group keeps the same offset: `(0, 0)`, or the stored `left`/`top` lengths read at `table.rs:197`.

A popover row and a popover row group share the flat publication path (`publication.rs:237`). With a viewport the row is its own block and `block::apply` clears the insets, so the row box stays put. The old shift was a second offset on the cells. `(0, 0)` removes it. With no viewport the row is absolute inside the table and the cells stay on the grid. Clearing `popover` drops the block and restyles (`top_layer::prop_changed`); `block_changed` marks the parent `CHILDREN_DIRTY` and the row `ATTACHED`, so the next layout publishes the cells with the relative offset again. An anonymous table only moves the grid origin fed into that walk. RTL still prefers `left` over `right` inside `relative()`; a popover returns before that.

The new test sets `popover="manual"` on the row before layout, which `layout_position` treats as top layer. It passes, along with the ordinary relative row/group test (24 `table::` tests) and `scripts/position-reads.test.mjs`. A popover row group and removing the attribute use the same predicate. Swift already reads `NodeView.layoutPosition`. Cell frames come from this publication.

VERDICT: APPROVE
