# Code review: the macOS split-pane popover, round 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9e15a8ef8b`.
- **Method:** one brief (sha256 `fc2b839044c788156a29282be74ac46046b80e4ea9df7f8262f7af7bd5bd4a8d`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 150a9a6460 (parent(of:) unchanged; connected() alone walks the block home)

Two regressions from [MenusMac.swift:183](/tmp/iosfix-rv-pane1/host/apple/Sources/ExactKit/Mac/MenusMac.swift:183):

- **[P2] A projected toolbar can prevent popovers inside its native dialog from opening.** Declare a dialog inside a `role="toolbar" toolbarPlacement="window"` container and open it through a toolbar action. Closed dialogs are excluded from the toolbar’s visible declarations, so this shape can project; projection sets the container’s `isHidden` at [ToolbarMac.swift:715](/tmp/iosfix-rv-pane1/host/apple/Sources/ExactKit/Mac/ToolbarMac.swift:715). The dialog remains visible in its sheet. Its popover button now walks through the dialog’s block home to that hidden toolbar, making `hidden(source)` true and failing [MenusMac.swift:314](/tmp/iosfix-rv-pane1/host/apple/Sources/ExactKit/Mac/MenusMac.swift:314). Previously the visibility walk ended in the sheet. This concerns platform projection hiding; an authored `display:none` ancestor legitimately suppresses top-layer rendering under the [CSS rule](https://drafts.csswg.org/css-position-4/#top-layer-styling).

- **[P2] Alert actions inherit inertness past an enclosing modal sheet.** Open a native dialog, set `inert=true` on its authored wrapper, then open a one-field `alertdialog` declared inside that dialog, with an enabled Save action and Cancel. Ordinary dialog controls correctly escape the wrapper’s inertness because [NodeViewMac.swift:237](/tmp/iosfix-rv-pane1/host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:237) stops at the presented dialog. However, [AlertsMac.swift:98](/tmp/iosfix-rv-pane1/host/apple/Sources/ExactKit/Mac/AlertsMac.swift:98) walks `menus.parent` without that boundary. It now reaches the inert wrapper, so the native prompt’s Save button is disabled. Previously this walk ended at the enclosing sheet’s window.

The remaining traced paths showed no concrete regression: stack nesting, modal containment, `presented(within:)`, focus scope, and light-dismiss containment retain ancestor direction. A click elsewhere in the pane does not become a descendant of its popover. An open native popover still takes `authoredParent` first, retaining its existing saved parent. `logicalParent` already uses `eventParent`.

The [new test](/tmp/iosfix-rv-pane1/host/apple/tests/ExactKitTests/SplitMacTests.swift:835) **would fail without the fix**: neither the popover nor its opener reaches `presenter.root`, and no dialog satisfies the alternative connection check. Testing both presentations and asserting `windowed.count == 1` prevents a painted fallback from satisfying the native case. It covers opening, but neither regression above; its fixed settling loop at line 847 is also timing-dependent.

Source review only; no tests run or files changed.
