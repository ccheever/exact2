# Code review: a table row's cells follow the engine's displacement (Taffy patch 69), round 2 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `1bb377e36c`.
- **Method:** one brief (sha256 `bcbdddfd8ba4335078c895800a9416d7241946df42ac6b1b9f2da177a6d4649f`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** the finding (shared by both reviewers) is addressed in `01efbc5155`; a confirmation round follows.

**MAJOR** — A flat row or group republishes every child whenever it is visited — `kernel/src/layout/publication.rs:203`

`descend_all` includes `tree.tables.flat(slot)` for every visit, not only when that part’s own shift changed. A `tbody`/`thead`/`tfoot` is flat, and any dirty descendant visits it, so one cell edit republishes every row and every cell in the group. A multi-cell `tr` does the same for its sibling cells. That is the ordinary table path; sparse publication already avoids it. `origin_moved` cannot see this case: a popover row’s block frame stays put while its grid origin moves, which is why the blanket exists, but the origin-change bit already marks that row.

Snapshot `arena.layout_dirty.contains(slot)` at the start of the visit, before `flags_mut` (line 100), which inserts every visited node. Add the flat term only when that snapshot is set. Direct children then move, and their own `origin_moved` walks the rest. A `tbody` entered only as an ancestor stays on the dirty path. The popover test still passes: the row is in `layout_dirty`, its frame is not, and the cells are not.

No second defect. The same `align_item_within_area` inputs, including the shim and safe alignment, go into the placed position and the origin, so a baseline shim or an RTL row wider than its area stays in the origin and cancels. Memo records `Event::Origin` and replays it; a node the trace has not read poisons the trace instead of dropping the write. A grid that stops placing a node leaves the old origin, and `remove` clears it; `display: none` drops `flat`, so displacement does not read it. Static items store origin equal to location, so they are not marked again. An absolute item whose containing block is not the grid can be marked when only its area moves; that is one visit, and it is the signal a flat popover needs.

VERDICT: CHANGES
