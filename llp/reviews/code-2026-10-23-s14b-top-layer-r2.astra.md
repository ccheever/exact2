# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), round 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `bf1127fd29`.
- **Method:** one brief (sha256 `e928f109e6b0ac9596588e4fe55e243f1a6b9fd91146dcee22b57a44dee4a6a6`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all 5 taken in e2a0e96d7e

Five issues remain:

1. **[P2] Reparenting can leave a popover permanently zero-sized** — [kernel/src/kernel/block.rs:350](/tmp/iosfix-rv-s14b2/kernel/src/kernel/block.rs:350).  
   An unchanged implied block skips restyling, but its lowered style depends on whether an ancestor has `HostCover::Whole`. Lay out `covered ancestor → ordinary wrapper → popover`, then move the wrapper under an uncovered sibling with identical inherited styles. The popover retains the `display: none` installed by `block::apply`: topology handling restyles direct positioned children, not this nested descendant. It remains zero-sized until another change forces restyling.

2. **[P2] Shell popovers bypass provisional-measurement handling with a registered region** — [kernel/src/kernel.rs:551](/tmp/iosfix-rv-s14b2/kernel/src/kernel.rs:551).  
   `provisional_layouts` is updated before `lay_out_blocks` runs. A native field inside a shell popover can therefore return provisional chrome without incrementing the counter. For example, with the region settled, a newly opened popover’s field returns provisional height 34 and final height 44. Apple’s `layout_pass` sees no counter change, skips its settlement retry, and publishes the guessed geometry. Include the block pass’s provisional status in the region layout’s result.

3. **[P2] Shell-block failures bypass region cache recovery** — [kernel/src/kernel.rs:551](/tmp/iosfix-rv-s14b2/kernel/src/kernel.rs:551).  
   The rebuild on error only covers `region.compute`; an error from the newly appended block pass escapes through `?`. With a registered region and a shell popover containing text, return invalid text metrics for one layout, then valid metrics on the next unchanged offer. Taffy has cached the containment zero from the failed measurement, and no rebuild invalidates it. The retry can succeed with zero-sized text without calling the corrected measurer. Recovery must cover errors from both passes.

4. **[P2] Mac restart retains stale block rectangles across reused IDs** — [host/apple/Sources/ExactKit/Mac/BlockHostMac.swift:165](/tmp/iosfix-rv-s14b2/host/apple/Sources/ExactKit/Mac/BlockHostMac.swift:165).  
   The new `rects` cache is cleared on destroy/unblock, but `Presenter.reset()` neither clears it nor calls `blocks.destroyed` for discarded views. Open a painted popover, restart with a plan that reuses its ID for an author-sticky popover, then open that popover. The fresh Rust mirror emits no unblock because it never held a block. `SheetsMac.shown()` consequently reports the previous popover’s rectangle, corrupting runner geometry and hit testing. The iOS reset fix needs its Mac counterpart.

5. **[P2] Terminal placement ignores a single automatic margin** — [host/terminal/src/paint.rs:170](/tmp/iosfix-rv-s14b2/host/terminal/src/paint.rs:170).  
   The special case handles only *both* margins being automatic. In a 40-column grid, an 80-point popover with `left=0 right=0 margin-left="auto" margin-right=0 top=0` takes the `start + at` branch and paints at column 0. Its automatic left margin should consume the remaining 240 points, placing it at column 30. The vertical equivalent also fails. The added test covers only two automatic margins.

Coverage note: [host_top_layer_tests.rs:96](/tmp/iosfix-rv-s14b2/host/terminal/src/host_top_layer_tests.rs:96) exercises the unchanged inline placement branch and would pass before this fix. Reveal stopping is tested only in fullscreen mode.

Static review only; no files changed and no builds or tests run.
