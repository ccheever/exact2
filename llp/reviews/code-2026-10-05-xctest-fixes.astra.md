# Round 1
Found three defects:

1. **P2 — Hidden paragraphs still paint their box decorations.**  
   [NodeViewIOS.swift:70](/Users/admin/projects/exact2-wt-xct/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:70), mirrored at [NodeViewMac.swift:85](/Users/admin/projects/exact2-wt-xct/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:85).  
   The exemption hides glyphs through `Run.hidden`, but background, border, gradient and box-shadow painting never checks visibility. A decorated paragraph therefore leaves its box visible after `visibility: hidden`, contrary to [CSS visibility semantics](https://www.w3.org/TR/css-display-3/#visibility).  
   **Fix:** Keep the paragraph view available for visible inline descendants, while separately suppressing its own decoration painting and decoration layers. Test a decorated hidden paragraph, including a visible inline descendant.

2. **P2 — Hidden paragraphs remain interactive and exposed to accessibility.**  
   [NodeViewIOS.swift:70](/Users/admin/projects/exact2-wt-xct/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:70), with the same macOS change.  
   Native hit testing checks `isHidden`, while inline targeting and accessibility projection do not filter hidden runs. The exemption consequently lets an invisible paragraph intercept clicks over underlying content; its text and hidden links remain accessibility elements. macOS also permits paragraph focus through `acceptsFirstResponder`.  
   **Fix:** Apply computed visibility to hit testing, focus and accessibility, preserving participation only for explicitly visible inline descendants.

3. **P2 — The enlarged HDR adoption path does not enforce the resident reservation.**  
   [RasterImage.swift:388](/Users/admin/projects/exact2-wt-xct/host/apple/Sources/ExactKit/RasterImage.swift:388).  
   `scratchBytes` bounds temporary storage, but the retained image is still charged as `plan.outputBytes`; completion releases the scratch reservation. A 64-bit BT.2100 thumbnail of **64×17**, with 512-byte rows, passes the new guards for a **64×16** plan: **8,704 bytes retained versus 8,192 charged**. I verified this accepted state with an in-memory CGImage; it is a synthetic guard counterexample, not a reproduced fixture decode. The packed 32-bit HDR fixtures have enough reservation headroom.  
   **Fix:** Require retained `bytesPerRow × height <= outputBytes`, or reserve and charge the larger output before adoption. Preserve BT.2100 storage.

The paragraph exemption is necessary for visible inline runs, but insufficient for complete visibility semantics. I found no additional child-NodeView regression in normal paragraph projection: inline descendants are values, and paragraph child mounting is suppressed. The reorder ghost remains intact because its hidden wrapper is `NodeType::View`.

The SDR path is sound: an oversized thumbnail fails `isAdoptable` and redraws into the planned dimensions and stride. Downstream fitting uses `naturalSize`. The plan already uses exact integer ceiling, so a bounded decoder tolerance is more appropriate than changing its rounding—provided the resident-byte bound is enforced.

Validation used source inspection and in-memory ImageIO probes. No full native test suite was run; no files were modified.
# Round 2
No additional concrete defects found in the two newest commits.

- [RasterImage.swift:388](/Users/admin/projects/exact2-wt-xct/host/apple/Sources/ExactKit/RasterImage.swift:388): the byte bound closes (a), including row padding and the +1 tolerance. Oversized thumbnails fall through to the planned-size redraw.
- [QUEUE.md:6](/Users/admin/projects/exact2-wt-xct/QUEUE.md:6): accurately records the pre-existing painting, hit-testing, focus, and accessibility defect.

Static review only; tests were not rerun in the read-only sandbox. No files modified.