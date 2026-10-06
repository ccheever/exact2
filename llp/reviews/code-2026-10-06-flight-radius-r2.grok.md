# Code review r2: a landed flight's view takes its clip and corners from its style (LLP 1013.000 D4/D4.4), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x2fr-rv`.
- **Method:** one brief (sha256 `1296861248e77db8d2e09d67decb28d4e2111e42dc42b4fee87ffe237640a148`), shared with the other reviewer. Round 2, blind to the other's round-2 review; both round-1 reviews and dispositions available. Reviewed commit 1badc497c in a detached worktree. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r3):** no findings. After this round the macOS `overflowClips` was rewritten shorter, with the same predicate, to keep NodeViewMac.swift under the 1,500-line cap; round 3 confirms that rewrite only.

---

I'll review the amended flight-landing commit against the round-1 findings and the new tests, without editing anything. First I'll read the review notes and the commit itself.This is a targeted correctness review of commit `1badc497c`, not a branch-wide review. I'll read the round-1 notes, the spec, and the commit diff next.Round 1 left a gradient-display gap, a clip-snapshot restore, and an iOS box pass that ran before geometry. I'll check the amended commit against those paths.The landing change and the `overflowClips` extraction both check out. Round 1's gradient, geometry-order, and lift-snapshot clip bugs are fixed, and the style pass on each host computes the same clip it did before.

**READY**

Round 1 is resolved:

- On iOS, `restore` runs `applyBoxLayer()` and then `setNeedsDisplay()`. `NodeLayer.display` runs the box pass with `flightLook` already cleared, then `applyGradientLayer` copies that layer radius. Both landing branches go through `restore`.
- The normal iOS landing calls `restore` after `applyGeometry`, so a radius that CSS reduces is computed at the landed size. The roots-changed branch still frames the view before `restore`.
- Neither host saves a radius or a clip at lift. Both set the clip from the current style (`overflowClips && clipBox == nil`) before the box pass. On macOS the box pass writes a positive backing-layer radius only when `clipsToBounds` is already true, so that write cannot turn clipping back on. The hidden-to-visible test locks this: after landing, the layer radius is 0 and `masksToBounds` is false.

The extraction does not change the style pass. `NodeStyle` is a dictionary copied into `style` at the start of the pass, and nothing writes it again before the clip decision. On iOS, `overflowClips` is the old `syncScroll` expression (`hidden` on either axis, or a waiting scroll), read at the same point. On macOS it is the old `clips || line-clamp` expression, including a paragraph's non-`visible` overflow; `syncClipBox` still requires `kind != "text"`, so a line-clamp cannot create a clip box. `applyStyle` and `syncScroll` still feed the scroll view from the same overflow values as before.

No findings.
