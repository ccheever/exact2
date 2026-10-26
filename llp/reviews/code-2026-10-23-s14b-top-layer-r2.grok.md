# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), round 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `bf1127fd29`.
- **Method:** one brief (sha256 `e928f109e6b0ac9596588e4fe55e243f1a6b9fd91146dcee22b57a44dee4a6a6`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** Linux content_size taken in e2a0e96d7e; the native-region test's frame check added

[P2] Linux scroll extent treats a popover's block-space frame as page space — `host/linux/src/paint.rs:1398`

`content_size` still does `c.frame.x - node.frame.x + c.frame.width` for every direct child. Apple's copy skips `host_block` children (`host/apple/src/layout.rs:650`); this one does not. On a host with `navigation.top_layer == false` (headless/DRM), `project_bars` does not apply `HostCover::Whole` (`host/linux/src/bars.rs:120`), so the next layout's `imply_blocks` gives the popover a viewport `Fit` block and `block::apply` clears its insets. The published frame is in that block. `Presenter::scroll_range` (`host/linux/src/presenter.rs:976`) feeds `ScrollBounds::capture` (`host/linux/src/paint/region.rs:73`), which calls this function whenever the scroller is not already a displayed region picture. A scrolling column 200px wide whose direct child is `popover="auto"` with `width: 50%` on an 800px window gets a border box about 400px wide at block `x = 0`. The scroll range becomes 200px of empty horizontal travel. The kernel's own `content` stays the in-flow size (`a_popover_takes_no_room_and_no_extent`); the popover does not paint, because `navigation.rs:298` marks every popover hidden. The same mix hits `presenter/collection.rs:322`. A windowed shell is unaffected: Whole cover means no implied block.

Round 1, checked in the tree:

- A region owner, or any ancestor, that `blocks.holds` or is in `top_layer_slots` is refused (`kernel/src/region/state.rs:797`). Registration fails before `imply_blocks`.
- A dropped block is `{"op":"block","id":N}` with no size. iOS `OwnerBlocks.applied` and Mac `BlockHost.rects` forget a nil rect; iOS `reset` clears the map. The host-block test covers the sticky toggle.
- `measure_height_targets` calls `replace_env(with_viewport(offer))` before `imply_blocks` (`kernel/src/kernel.rs:623` area, same pattern as `compute_layout_presented`). The kernel test compares that measure with the following layout at the same offer.
- `block_clip` returns `None` when `host_block_implied` (`runner/src/geometry.rs:600`). The overflow child in `a_popovers_block_clips_nothing` would miss if the margin box still clipped.
- Under a registered region, a top-layer box under the owner gets no implied block and stays in the trial; shell popovers are laid out in `lay_out_blocks` after a successful `region.compute`. `publish` only writes listed frames, so a shell popover is not wiped before that pass.
- Terminal `sync_top_layer` reports `[-dx, -dy, used]` (`host/terminal/src/host.rs:1152`). `inset_at` places a length inset outside the start margin and centres when both insets and both margins are auto. Reveal stops on any `host_block`.
- `LayoutMotion::seed` records block membership (`kernel/src/motion/layout.rs:55`), so a transition added on a box that is already in a block does not snap. The two presence tests fail without that.
- `root_above` returns the block root, so `measure_auto_height` does not lay the page out again.

Not defects: `shown_at` scale when the host reports the used (margin) box, which the Apple batch does (`w:416,h:46,x:8,y:8` against a border box at `(8,8)`); the mirror `implied` flag, which only forces that resend and is not a Swift field; web, which does not read kernel frames; Apple `document_extent`, which already skips `host_block` and popovers. `compute_region_layout` implies blocks before `validate`, but a refusal does not publish, and clearing the popover drops the block on the next `imply_blocks`. iOS `blockRects` is not cleared in `release`; the id is only read for an open top-layer view, and the next block op overwrites it.

The agree, kernel percentage, cover, clip, unblock, presence, and terminal inset tests assert the new geometry. The Apple native-region test only checks that a publication id exists and `host_block` is `None`, not that the in-region popover is half the owner.
