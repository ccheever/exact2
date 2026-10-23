# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), round 3, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `e2a0e96d7e`.
- **Method:** one brief (sha256 `15ecb68756d77b6defa29a6f82ba3bf733b46233069e0ef185265c1ce432a363`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** Linux scroll anchoring taken in 7df0ba99f8

[P1] Linux scroll anchoring treats a popover's block-space frame as page space — host/linux/src/scroll_anchor.rs:175

`bounds` maps `node.frame` and compares it with the scroller's port. `excluded` (line 149) drops an absolute box from a nested scroller, but for the document scroller (`PAGE`) an absolute box is kept. A headless popover is not `HostCover::Whole` (that cover is only applied when a shell sets `navigation.top_layer`), so `imply_blocks` lays it out in the viewport block and its frame origin is its margin, near `(0, 0)`, not its old static position. `content_size` was updated to skip that child; this walk was not. Anchoring runs only once the page is already scrolled (`capture` returns on `offset.y <= 0`), and a short scroll still overlaps that box: after 20px, the port starts at y=20 and a popover whose border box is 80px tall still intersects it. If that popover is the first intersecting branch (a tall root whose first child is the popover), `select` anchors to it and records y≈0. Inserting 100px above the real rows does not move the block frame, so `Anchor::shift` (line 318) returns 0 and the viewport jumps. Before this change the same popover's frame moved with the document, and the shift kept the rows still.

Round 1 and round 2 fixes are in place and hold: region owners under a popover are refused on every compute, a block op with no size drops the Swift rect (and iOS `reset` clears `blockRects`; view ids are not reused within a runner), `measure_height_targets` sets the viewport before `imply_blocks`, implied blocks do not clip, shell popovers rebuild and count provisional chrome once, a block buried under `HostCover::Whole` is restyled when it is reparented out, one auto margin uses the room left, and Linux `content_size` skips block children. Agree frames, terminal insets, and the layout-motion snap match the tests that pin them.
