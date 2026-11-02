# Code review: a table row's cells follow the engine's displacement (Taffy patch 69), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `7f19bbd629`.
- **Method:** one brief (sha256 `5b67aebc5bc24bd6584904dba4c303f1e34bf5744fa45d05ff5ffd07052ae13d`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** both findings are addressed in `1bb377e36c`.

- **MAJOR — Alignment is mistaken for relative displacement.** [kernel/src/layout/table.rs:206](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/layout/table.rs:206). `location − area` includes alignment within the area, not just relative insets. Row/group lowering clears `align_self`, allowing the table’s `align-items` to apply. With `align-items: center`, a 12px cell and an empty engine row, the row centers 6px below its area’s origin. Merely making that row `position: relative`, with no insets, now shifts its cell another 6px; the parent commit leaves it unchanged. Relative groups can compound this. **Fix:** record the engine-resolved relative translation separately, or subtract the aligned position before relative offsets; retain area subtraction for absolute placement. Add inset-free relative row/group tests under centered alignment.

- **MAJOR — Changed displacement can leave published cells stale.** [kernel/src/layout/publication.rs:238](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/layout/publication.rs:238). Publication now depends on the area origin, but `descend_all` still checks the row’s frame origin. [taffy_tree.rs:930](/Users/admin/x2scratch/fb1-rv-astra/vendor/taffy/src/tree/taffy_tree.rs:930) also changes area metadata without reporting a layout change. Concrete case: a first-row cell spans two rows in column one; a popover second row owns the column-two cell and has its viewport block. Reduce column-one content width by 10px while increasing table padding-left by 10px. The second cell’s engine rectangle stays unchanged, but the row’s area moves right 10px, changing its required block-local displacement. The block root remains at `(0,0)`, so sparse publication skips that unchanged cell and retains its previous frame; a fresh layout differs. **Fix:** surface area changes to publication and republish all affected descendants when the flat-child origin changes, independently of the row’s frame. Add this incremental-versus-fresh regression.

These findings are source-traced. The sandbox rejected `cargo test -p exact-kernel --test it table::` with “Operation not permitted”; tests could not run.

VERDICT: CHANGES
