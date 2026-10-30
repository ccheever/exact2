# Code review: the iOS smooth-correction read-back livelock (SmoothCollectionIOSTests' flake), round 1 of 2, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `02cc0bd218`.
- **Method:** one brief (sha256 `bbe5093a51a44b529a24943cd5ed10bc34adeb6071b4c154f4be93af9ac18cc9`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**SHOULD — [ReorderPartsIOS.swift:107](host/apple/Sources/ExactKit/IOS/ReorderPartsIOS.swift:107): pre-existing audit miss.** At an upper limit of `5/3`, the supplied UIKit read-back makes `r.now < r.high` true despite already occupying the last pixel. The guard at line 112 also passes; writing that pixel again leaves the read-back unchanged, so `scroll(by:)` returns false. [ReorderGroup.swift:352](host/apple/Sources/ExactKit/ReorderGroup.swift:352) then stops autoscroll instead of selecting an eligible ancestor. Exclude offsets within `PixelGrid.same` of the boundary in `canScroll`. Line 116 itself compares two read-backs; the missed decision is upstream.

Additional safe snapped/read-back comparisons: [CollectionIOS.swift:173](host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:173) and 177, [NodeViewIOS.swift:1348](host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1348), [SegmentsIOS.swift:441](host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:441), and [DialogSheetIOS.swift:165](host/apple/Sources/ExactKit/IOS/DialogSheetIOS.swift:165) and 173 only skip idempotent geometry writes.

No blocker found in the changed motion or anchor guards. The tolerance preserves genuine pixel-scale reclamps and drag cancellation. Read-only review; UIKit tests were not rerun.
