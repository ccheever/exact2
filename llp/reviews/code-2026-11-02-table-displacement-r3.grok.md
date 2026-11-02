# Code review: a table row's cells follow the engine's displacement (Taffy patch 69), round 3, confirmation (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `01efbc5155`.
- **Method:** one brief (sha256 `5bff77518b70d0c5ea2db87e666268fbfa26f28697d60c3f96a2eb3f23271672`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** nothing to address.

No new findings. The two earlier fixes hold.

Alignment stays out of the displacement: an in-flow item's origin is its place in the area with `position: static` and no insets, so `location − origin` is only the relative offset (`alignment.rs:420`). An absolute child's origin is the grid-area corner (`mod.rs:818`), including when a later containing block writes the location, so a second-row or second-column cell still cancels back onto the row box. A changed origin is recorded as a layout change and replayed as `Event::Origin`; a cache hit does not rerun placement, and the stored origin remains the one from the run that filled the cache. Nothing clears `grid_origins` without also dropping the node or the tree.

A popover row with a block is still a grid item, so the page pass records that area origin, and the block pass overwrites only the location. Publication then places cells at `block location − area origin + cell location`, which is the cell's offset within the row. The page pass leaves the row in `layout_dirty` (it is not detached), so the block pass still sees an origin change and republishes.

The visit cut is consistent: child origin is `frame origin − grid origin`, so it changes only when the frame moves or the area origin moves, and both of those set `displaced` or `origin_moved` and walk the whole flat part (`publication.rs:209`). A one-cell edit does not.

VERDICT: APPROVE
