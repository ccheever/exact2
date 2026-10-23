# Code review: stage 14 B, the popover's top layer (a kernel-implied viewport block), round 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `52b1bec7c8`.
- **Method:** one brief (sha256 `95302a25108100b87510c21f8b68fec838b1b126b4f2e0a90c66086209a78017`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** 7 findings: 1-4, 6, 7 taken in a3bf371b56; 5 (native region) ruled by theme2 09:54 fallback (b), bf1127fd29

Found seven correctness issues:

1. **[P1] A content-region owner inside a popover can panic.** [kernel/src/region/tree.rs:327](/tmp/iosfix-rv-s14b1/kernel/src/region/tree.rs:327) skips every block subtree, including one containing the registered owner. Registration permits this, but [region/state.rs:299](/tmp/iosfix-rv-s14b1/kernel/src/region/state.rs:299) unconditionally unwraps that owner’s shell frame. Repro: give the existing region fixture’s owner `popover="manual"` and compute a definite layout. The owner leaves the shell, and computation panics before `lay_out_blocks` runs.

2. **[P2] Removing a block leaves stale rectangles in Swift.** [host/apple/src/layout.rs:400](/tmp/iosfix-rv-s14b1/host/apple/src/layout.rs:400) updates the mirror to `None` without emitting anything that clears `blockRects`/`rects`. Both presenters continue reporting the cached rectangle. Repro: open a `width="50%"` popover under a 100-point parent in an 800-point viewport, then make it sticky. Its width becomes 50, but Swift still reports the previous 400-point rectangle; `shown_at` consequently scales geometry and hit coordinates incorrectly. The iOS cache also survives destruction and runtime reset.

3. **[P2] Height-target measurement uses the previous viewport.** [kernel/src/kernel.rs:689](/tmp/iosfix-rv-s14b1/kernel/src/kernel.rs:689) calls `imply_blocks()` without updating the environment from the supplied `offer`. After laying out at width 400, measuring at width 800 still measures a 50%-wide popover at 200 rather than 400. Wrapping content therefore returns the wrong automatic height. This reaches Apple’s transition path because `sync_height_transitions()` runs before the ordinary layout updates the viewport; an expanding popover resized mid-transition targets the wrong height.

4. **[P2] Implied blocks introduce an unintended overflow clip.** [runner/src/geometry.rs:596](/tmp/iosfix-rv-s14b1/runner/src/geometry.rs:596) treats every block’s used size as a clipping rectangle. For an implied block, that size is the popover’s margin box, not the viewport. Repro: an open 100×40 popover with explicit `overflow="visible"` and an absolutely positioned button at `top=50`. The painted button remains visible, but `elementFromPoint` rejects it at the block boundary and intersection reports it outside. The implied block needs different clipping treatment from a native pane or sheet.

5. **[P2] Apple native-region publication rejects the newly excluded popover.** [kernel/src/region/tree.rs:51](/tmp/iosfix-rv-s14b1/kernel/src/region/tree.rs:51) removes block subtrees from publication, but Apple’s native candidate still captures every descendant at [content_region/host.rs:400](/tmp/iosfix-rv-s14b1/host/apple/src/content_region/host.rs:400). Its membership check then returns `"native publication membership mismatch"` at line 502. Repro: boot a native region with a popover beneath its content branch. The first accepted publication excludes nodes that the candidate still contains. Native capture and protection must follow the same exclusion rule.

6. **[P2] Terminal placement loses authored automatic margins.** [host/terminal/src/paint.rs:160](/tmp/iosfix-rv-s14b1/host/terminal/src/paint.rs:160) always chooses the start inset when supplied. Block lowering has already replaced automatic margins with zero, so the placement calculation cannot distribute remaining space. In a 40-column viewport, a popover with `width=80`, `left=0`, `right=0`, and automatic horizontal margins lands at column 0 instead of column 15. The added tests cover numeric insets and centering with no insets, not this case.

7. **[P2] Adding a layout transition inside an existing block incorrectly snaps.** [kernel/src/motion/layout.rs:132](/tmp/iosfix-rv-s14b1/kernel/src/motion/layout.rs:132) interprets first insertion into `blocked` as crossing a boundary. But `seed()` initializes the previous layout value without initializing block membership. Repro: an already open popover initially has no layout transition; one commit adds `-exact-layout-transition="1s linear"` and changes its width. Observation removes the seeded value and snaps, although the box remained in the same block.

The new inline terminal test exercises the unchanged inline-placement branch, so it does not establish that the block-placement fix works.

Static review only: no files edited or builds/tests run.
