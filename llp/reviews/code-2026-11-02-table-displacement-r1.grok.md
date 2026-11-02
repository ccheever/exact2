# Code review: a table row's cells follow the engine's displacement (Taffy patch 69), round 1 (grok)

- **Family:** xAI. Grok, `grok -p … --reasoning-effort xhigh --max-turns 400 --no-plan --no-subagents --always-approve --deny Edit --deny Write`, a throwaway worktree at `7f19bbd629`.
- **Method:** one brief (sha256 `5b67aebc5bc24bd6584904dba4c303f1e34bf5744fa45d05ff5ffd07052ae13d`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** both findings are addressed in `1bb377e36c`.

**MAJOR** — A relative row’s cell shift includes its alignment — `kernel/src/layout/table.rs:206`

`displacement` is the engine location minus the origin stored by patch 69. For an in-flow item that origin is the grid-area corner (`vendor/taffy/src/compute/grid/alignment.rs:419`), while `location` is that corner plus alignment and only then the relative inset (`alignment.rs:559`). With zero margins and stretch, the extra term is 0, which is why the new tests pass (`cargo test -p exact-kernel --test it table::`: 27 passed). It is not 0 when the border box does not start on the area: `justify-items` / `align-items` other than stretch, a baseline shim, or RTL when `min-width` makes the row wider than its area (`offset = area − size` in `align_item_within_area`). Cells then move by that alignment as well as the inset, so the gap they had to the row changes. Store the border-box origin from before the relative inset for in-flow items. The absolute pass (`vendor/taffy/src/compute/grid/mod.rs:818`) should keep storing the grid-area origin, so a popover’s cells still move from that area onto the border box. `location − origin` is then the inset for a relative row and the absolute shift for a popover. The direct Taffy test’s margined item should report origin `(29, 3)` (area `(25, 3)` plus the 4px margin); its position stays `(34, 3)`.

The area write is not read by any algorithm. `Event::Area` is recorded after `Visit` and restored on replay. A node a grid stops placing can keep an old area; a `display: none` row leaves `flat`, so `displacement` does not read it. Margins on rows and groups are forced to 0. A popover with a block is placed again as that block’s root after the grid has stored the area, so the cell’s grid location plus `block location − area` lands on the row. Row-group and rowspan shifts stack in publication because cells are not engine children of the row.

VERDICT: CHANGES
