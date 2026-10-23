# Code review: the macOS split-pane popover, round 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `150a9a6460`.
- **Method:** one brief (sha256 `cc97cde16974259269440223a9597eb8720bdd18c2f08869d343ed04fad93a54`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 25a5a8eb3b

[P2] A painted popover from the detail pane is laid out in the list column — `host/apple/Sources/ExactKit/Mac/MenusMac.swift:204`

`connected()` now lets `show()` proceed for a node whose only path to `presenter.root` is `BlockHost`'s home. `paint()` still does `(window === presenter.viewport.window ? presenter.viewport : window.contentView).addSubview(entry.layer)` (`MenusMac.swift:93`). The detail pane is another split item in that same window (`SidebarViewsMac.swift:74`), so the layer is parented to the list column's scroll view. With no `position-area`, `layout()` clamps the box into that layer (`MenusMac.swift:554`, `PositionArea.swift:81`). The 100×40 `bookpop` from the test sits on the library column's right edge, not beside "More". A click in the detail (not on the button) is outside that view, so light dismiss closes it; a click on that clamped box in the list is inside it and does not. AppKit's popover is unaffected: `present()` anchors to the source view (`PopoversMac.swift:167`).

The other callers are unchanged. `parent(of:)` is `authoredParent ?? superview` again (`MenusMac.swift:180`), so `contains()` does not walk a block home.

- Stack nesting, `presented(within:)`, focus scope, and context-menu nesting use `contains()`. A pane click and a page click are not the same ancestor chain.
- `hidden()` (`MenusMac.swift:212`) and `shown()` stay on that walk. A hidden home does not hide the pane's route. `NavigationMac.gate` writes `isHidden` and `routeInert` onto the route itself (`NavigationMac.swift:179`).
- Alert rows (`AlertsMac.swift:98`), `NodeView.inert` (`NodeViewMac.swift:238`), and Back's inert check (`BackMac.swift:147`) likewise stop at the sheet. A dialog block does not pick up its route's hide, a projected toolbar's `isHidden`, or the page wrapper's `inert`. `NodeView.inert` also breaks when it reaches a dialog it owns.
- An AppKit popover becomes a block root in `blocks.show` (`PopoversMac.swift:180`) only after `Entry` stores `parent = popover.superview` (`MenusMac.swift:51`). `blocks.home` is that same superview. `parent(of: pop)` still returns `authoredParent`, and `connected()` tries that before the block home, so the new block root does not replace it.
- A popover already inside a sheet stays connected via `dialogs.presented` and `contains()`, which does not need the home. Its layer goes on the sheet's content view because that window is not `viewport.window`. The new walk only adds the pane case.

`testAPopoverInTheDetailPaneOpens` fails without the fix: `press` → `command` → `show` returns before `entries.append`, so `isOpen` stays false for both iterations. The assertions that the route is held and the opener is not under `root` (`SplitMacTests.swift:853`) keep it from passing if the route never leaves the page. It does not catch the placement bug: it asserts `isOpen` only, and for AppKit merely `windowed.count == 1`. `show()` appends synchronously, so the 3s wait never checks a later `sync()` or the popover's frame.
