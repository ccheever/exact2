# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), round 1, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `52b1bec7c8`.
- **Method:** one brief (sha256 `95302a25108100b87510c21f8b68fec838b1b126b4f2e0a90c66086209a78017`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** 1, 3, 4 taken in a3bf371b56; 2 (measure_auto_height) rejected: root_above returns the block root, pinned by a kernel test

Three blocking bugs. The implied-block lifecycle itself (destroy, slot reuse, host-given wins, `HostCover::Whole` and roots and inline runs cleared on the next `imply_blocks`, region trial skipped, agree frames in block space, motion snap on the block root) holds.

**[P1] A dropped implied block leaves the Apple host reporting the old viewport rect — `host/apple/src/layout.rs:399`**

When the in-force block becomes `None`, the mirror stores that and sends no op. `OwnersIOS.applied` (`OwnersIOS.swift:220`) and `BlockHostMac.applied` (`BlockHostMac.swift:165`) only write `blockRects` / `rects`; Mac clears them in `destroyed` (`BlockHostMac.swift:182`), iOS never does. `openTopLayer` still prefers that rect (`OwnersIOS.swift:397`, `SheetsMac.swift:202`).

Open a popover at `width: 50%` in an 800-wide window (block rect 400 wide). Set `position: sticky` so the next layout drops the block and the border box becomes the parent's 50%. The popover stays presented. The runner is still told the 400-point block, so hit testing and `open` geometry cover the old viewport box.

**[P1] `measure_auto_height` never lays out a block subtree — `kernel/src/kernel/geometry.rs:60`**

It only `compute`s the page root. A popover is no longer in that tree (`order::laid_out` drops block roots), and this path does not call `lay_out_blocks`. `runner/src/geometry.rs:433` (`measure`) therefore reads the last cached size. The following `compute` of the page root also leaves the popover dirty, so the engine no longer matches the published tree.

A popover with `height: 200px` whose content is 40px at `height: auto`. `measure` returns 200. Before this change the popover was still in the page tree and the same call returned 40.

**[P1] Height measurement implies the block against the previous viewport — `kernel/src/kernel.rs:689`**

`compute_layout_presented` installs the offer with `replace_env` before `imply_blocks` (`kernel.rs:622`). `measure_height_targets` does not, then lays the popover out with `lay_out_blocks_unpublished`. The host samples heights before layout (`host/apple/src/layout.rs:114`) using the new window size. The first time a popover gains a block here, `block_changed` also sets `ATTACHED` and `CHILDREN_DIRTY` and detaches it, which this function documents as leaving flags untouched.

Resize 800×600 to 400×800 with an auto-height target inside a `width: 100%` popover. The sample wraps at 800. The layout that follows wraps at 400. The height transition aims at the wrong height.

**[P2] Terminal geometry still uses the kernel frame as the viewport position — `host/terminal/src/host.rs:1144`**

`sync_top_layer` records the popover with no rect. `shown_in` then treats the kernel frame as viewport space (`runner/src/geometry.rs:564`). `block::apply` clears insets on a `Fit` block, so that frame stays at the static position, usually `(0, 0)`. Paint places the same box by insets (`host/terminal/src/paint.rs:128`).

Fullscreen, `top=32 left=16`. The cells land at column 2, row 2, which the new test checks. `frame` and `elementFromPoint` still resolve the popover at the origin, so a hit on the painted cells misses.

Apple `content_size` / `document_extent` already skip a block root and a popover. Linux covers popovers whole, so they never keep an implied block. The web host leaves the popover to the browser. The inline terminal test never reads `host_block`; inline's `MaxContent` height is what actually withholds the block.
