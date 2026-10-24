# Code review: the macOS split-pane popover, round 1, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `9e15a8ef8b`.
- **Method:** one brief (sha256 `fc2b839044c788156a29282be74ac46046b80e4ea9df7f8262f7af7bd5bd4a8d`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 150a9a6460

`[P2]` A shown block inherits its home's hide — `host/apple/Sources/ExactKit/Mac/MenusMac.swift:183`

`hidden()` (`MenusMac.swift:208`) and `shown()` (`:889`) walk `parent(of:)`. For a block root that walk is `entry.home` (`BlockHostMac.swift:93`). `NodeView.inert` stops at an owned dialog before that jump (`NodeViewMac.swift:237`). These two do not.

Failing case: a `dialog` authored in a route, presented with `dialog.isHidden = false` (`DialogsMac.swift:141`) and `blocks.show` (`SheetsMac.swift:246`). `DialogHost.sync` (`DialogsMac.swift:231`) keeps the sheet as long as `entry.parent` is under `root`; it does not care that the route is hidden. From inside the sheet, hide that route (`NavigationMac.swift:181` on a push, or `display: none` on the home). The sheet stays on screen. `show()` then returns at `MenusMac.swift:314` because `hidden(source)` is true, `sync` closes an open popover (`:518`), and `context` returns at `:394`. `AlertsMac.choosable` (`AlertsMac.swift:98`) also walks this chain, so the same home's `routeInert` disables the alert's rows. Before this commit the walk stayed in the sheet window.

The detail pane does not hit this from navigation's own gate: an inactive panel sets `away`, which sets `isHidden` on the route itself (`NavigationMac.swift:179`). A home that is hidden while that route is not (author `display: none` on the tabpanel, the route still in the pane) is the same failure.

Elsewhere the new walk is fine.

- Stack nesting (`MenusMac.swift:320`, `404`) only treats a popover as the parent when that popover view is still an ancestor. A page popover has been moved to its layer, so a popover from the pane still closes it. A click in the pane and a click in the page both light-dismiss (`:652`): the pane button is not an ancestor of the page, and the open popover is not an ancestor of either hit once it has left the tree.
- `presented(within:)` (`:136`) and `following()` (`:199`) — focus scope does not use `parent`. Sheet content is still inside the dialog node before the jump, so a popover opened there stays inside the sheet. There is no `logicalParent` on Mac `MenuHost`.
- `connected` (`:204`) is the intended fix. Inert and disabled on the opener are unchanged for a sheet: inert stops at `dialogs.owns`, and disabled is the node's own flag (`:312`).
- AppKit popover block: `parent(of: pop)` still returns `authoredParent` (`:186`), captured in `Entry.init` (`:51`) before `blocks.show` (`PopoversMac.swift:180`). That show records the same superview as `home` (`BlockHostMac.swift:122`). Same view as before. A node that was already a block keeps its older home (`BlockHostMac.swift:129`); `parent(of: pop)` still returns the container, as it did before this commit.
- Route sheets: while the sheet is the current stack's, the route stays unhidden (`under`) and the selected panel is unhidden, so a popover there opens. `syncRoutes` drops it on a tab change.

`SplitMacTests.testAPopoverInTheDetailPaneOpens` (`SplitMacTests.swift:836`) fails without the fix: the opener is not under `root` and not in a dialog, so `connected` is false and `show` never sets `isOpen`. It only covers that open, with a visible home (`main` / the selected panel). It would stay green for the hide-the-home break above, and it never clicks the pane versus the page.
