# Code review: the macOS split-pane popover, narrow re-check 3, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a9ab83f7b2`.
- **Method:** one brief (sha256 `46ad87d981e434e93a7b31538e96599303113ef851ed52397f9ea5f25ab58cf0`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 339fdffe37

**WRONG — one remaining paint-order bug.**

[MenusMac.swift:551](/tmp/iosfix-rv-pane5/host/apple/Sources/ExactKit/Mac/MenusMac.swift:551) changes subview order without recomputing the moved layer’s `CALayer.zPosition`. Those depths are calculated separately for each parent in [PaintOrder.swift:120](/tmp/iosfix-rv-pane5/host/apple/Sources/ExactKit/PaintOrder.swift:120); `PopoverLayer` has no reparent callback that updates them.

Concrete failing case:

1. Visit details in two split tabs, leaving the inactive tab’s positioned detail route retained in the pane.
2. In the active tab, open painted popover A, then manual page popover B. Arrange their boxes to overlap after collapse.
3. Collapse the split as the regression test does. Only the active stack’s routes return to the page; the inactive route remains in the pane.

The retained positioned route keeps A’s old depth at `0.002`; B’s viewport depth is `0.001`. Moving A below B in `subviews` therefore still paints **A above B**. `SheetsMac.openTopLayer` reports the subview order, disagreeing with the pixels.

An in-memory AppKit probe confirmed that `.below` preserves these unequal depths while producing subview indices `0 < 1`. Recompute the destination’s paint ranks after reparenting.

The assertion at [SplitMacTests.swift:893](/tmp/iosfix-rv-pane5/host/apple/tests/ExactKitTests/SplitMacTests.swift:893) catches the previous append bug but checks only subview indices, so it misses this failure. The other assertions distinguish the earlier ownership, Escape and migration defects by inspection.

No files changed. I did not run XCTest or mutation checks because the fixture writes files.
