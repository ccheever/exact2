# Code review: the macOS split-pane popover, narrow re-check 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `25a5a8eb3b`.
- **Method:** one brief (sha256 `ec3f1e8017fc66f628ea609941ccbcaec49b01df698a01030282a33d39f7924d`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 067907ce0b

**WRONG — the initial placement is fixed, but an open popover does not follow a container change.**

At [PopoversMac.swift:94](/tmp/iosfix-rv-pane3/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:94), the layer’s parent is chosen only during `paint`. [syncPresentation:116](/tmp/iosfix-rv-pane3/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:116) returns immediately unless `position_try_fallbacks` changes.

Concrete failure: open a painted popover in the detail pane, then make the list ineligible for splitting while retaining the selected detail—for example, hide the list through an action inside the popover. `SplitHost.present` returns the detail route to the page and collapses the pane. The popover remains logically open in the collapsed pane: its source and authored parent remain valid, and layout merely resizes the existing layer. It disappears; the next toggle closes that invisible presentation. The reverse transition leaves a page-attached popover clamped into the list column. Reconcile the layer’s parent when the split changes, or explicitly dismiss it.

The new assertions do **not** independently cover every fix:

- Removing pane attachment fails the ancestry/position assertions.
- Removing `holdsFocus` fails the Escape assertions.
- Removing the document-overflow change leaves them passing: the [fixture:840](/tmp/iosfix-rv-pane3/host/apple/tests/ExactKitTests/SplitMacTests.swift:840) specifies neither `position` nor `position-area`. Default placement returns before consulting the overflow rectangle.

Light-dismiss hit testing uses window content; frame conversion uses the actual hierarchy; active-split accessibility and capture include the split root. Restore uses the saved authored parent. [SheetsMac.swift:196](/tmp/iosfix-rv-pane3/host/apple/Sources/ExactKit/Mac/SheetsMac.swift:196) still assumes viewport ancestry for depth, returning `-1` for pane layers, but I found no independent concrete ordering failure in this scope.

Read-only source review; no files changed or tests executed. The XCTest fixture writes files.
