# Code review: the iOS smooth-correction read-back livelock (SmoothCollectionIOSTests' flake), round 1 of 2, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `02cc0bd218`.
- **Method:** one brief (sha256 `bbe5093a51a44b529a24943cd5ed10bc34adeb6071b4c154f4be93af9ac18cc9`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**CLEAN**

The hundredth-of-a-pixel tolerance is one named constant, the reclamp path still treats a real move as a move, and the Apple-host audit does not leave an exact snapped-versus-read-back compare that can stall a motion.

`PixelGrid.readBackSlop` is `0.01` device pixels, used only inside `PixelGrid.same`, which divides by `traitCollection.displayScale` (the same scale `PixelGrid.offset` snaps with). The reclamp test, the retarget’s “port still at `from`” test, and both reading-anchor compares go through that function. A content shrink like the existing case (target 1500, edge 1300) is hundreds of points outside that slop, so the branch still resets `began`, retargets, and stops when `here == reached` (both `reachable()` results, `CollectionIOS.swift:461`). A drag still stops in `scrollViewWillBeginDragging`. The new test finds a grid offset UIKit reads back off-grid, requires that read-back to sit inside the slop, and requires the correction to land.

The listed sites match the tree (the pixel-grid write is `PixelGridIOS.swift:240` after the insertion; the two-snapped compare is `CollectionIOS.swift:461`). Each one either skips or performs one idempotent write, or stops a bounded loop (`GroupedListIOS.swift:996` runs at most three times). Other exact compares against a scroll read-back are outside that class: `CollectionMac.swift:121`, `:132`, `:393`, `:405` and `ScrollAnchoringMac.swift:50` compare an unsapped computed point and scroll once; `NodeViewMac.swift:693` compares two read-backs; `AgentMac.swift:433` and `IntoView.swift:87` are one reveal scroll; `IntoView.swift:333` compares two snapped targets the host stored; `TextAreaIOS.swift:401` is the property’s own old and new value. None of them restart a clock from a one-ULP gap.
