# Code review: the painted popover's default placement and AppKit live margins, narrow re-check 4, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f4525ec737`.
- **Method:** one brief (sha256 `904218ab1b4bfc2e7a402c4eede2d563e5ebe01c2eb383ec5113555ea3f8129e`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** two old cases accepted as stated limits (theme2 22:25), QUEUE'd in ad5713949a

**WRONG — resize fix (a) looks correct; gap fix (b) is incomplete.**

- **Toolbar, including 0→nonzero:** [PopoversMac.swift:229](/tmp/iosfix-rv-flip6/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:229) clears `native.positioning`. Concrete case: open a manual popover from a toolbar item with zero margins and no `position-area`, then set `margin-top=12`. `reanchor` records the margins but cannot update `positioningRect`; the gap stays unchanged. There is no transition from the toolbar-item anchor to the rectangle anchor.

- **Unflipped source view:** [PopoversMac.swift:215](/tmp/iosfix-rv-flip6/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:215) forces `gap=0` whenever `view.isFlipped` is false. For an unflipped window-content anchor, changing the facing margin from 12 to 24 produces the same rectangle. Converting the preferred edge does not restore that gap. This exclusion predates the commit but remains outside the claimed fix. A flipped source converted into an unflipped destination is handled correctly.

The ordinary flipped-view path retains its closure even when opening at gap zero, so subsequent changes reach the [documented repositioning mechanism](https://developer.apple.com/documentation/appkit/nspopover/positioningrect). The new test does not cover that direction.

[BlockHostMac.swift:173](/tmp/iosfix-rv-flip6/host/apple/Sources/ExactKit/Mac/BlockHostMac.swift:173) correctly notices margin changes despite an unchanged total. Route sheets and dialog blocks remain safe: their callbacks pass the full used size to [SheetsMac.swift:79](/tmp/iosfix-rv-flip6/host/apple/Sources/ExactKit/Mac/SheetsMac.swift:79), whose final-size guard avoids redundant window resizing. `Margins.gap(edge)` maps the facing sides correctly.

**Test sensitivity, by inspection:** [PopoverNativeMacTests.swift:470](/tmp/iosfix-rv-flip6/host/apple/tests/ExactKitTests/PopoverNativeMacTests.swift:470) should fail without (a): the unchanged total suppresses the callback, leaving content height unchanged. The following position assertion should fail without (b): content grows, but the anchor retains its original 12-point gap. Thus the test distinguishes both fixes for its flipped-view, 12→0 case, while missing the cases above.

No files changed. XCTest/mutation runs were not executed; the fixture creates files, contrary to this read-only review.
