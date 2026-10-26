# Code review: the macOS split-pane popover, round 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `150a9a6460`.
- **Method:** one brief (sha256 `cc97cde16974259269440223a9597eb8720bdd18c2f08869d343ed04fad93a54`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 25a5a8eb3b (pane layer, Escape)

Two remaining problems affect the newly enabled painted popovers:

- **[P2] Popovers from the detail pane are positioned in the list column.** [PopoversMac.swift:93](/tmp/iosfix-rv-pane2/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:93) attaches their layer to `presenter.viewport`, which occupies the list column. [MenusMac.swift:548](/tmp/iosfix-rv-pane2/host/apple/Sources/ExactKit/Mac/MenusMac.swift:548) then uses that column’s bounds for placement. Concrete case: the new test’s painted branch, with its 100-point-wide popover and default placement. With the list width at 320, placement clamps the popover’s x-coordinate to 220, although its invoker is beyond the divider. It opens over the list instead of beside the detail control. This also affects normal platform presentation when authored `position-try-fallbacks` selects painting.

- **[P2] Escape cannot dismiss a painted popover while focus remains in the detail pane.** [MenusMac.swift:580](/tmp/iosfix-rv-pane2/host/apple/Sources/ExactKit/Mac/MenusMac.swift:580) requires the focus owner to descend from `viewport`; the detail pane does not. Concrete case: focus `field-a`, open `bookpop` without autofocus using `p.menus.painted = true` (or authored fallbacks), then press Escape. Focus remains in the detail field, the guard returns false, and the popover stays open. `ExactView.ownsShortcutFocus()` already recognizes detail-pane focus, but this dismissal guard does not.

The Round 1 ancestry regressions are resolved. `parent(of:)` and `contains()` retain their baseline behavior, preserving stack nesting, hidden/shown checks, modal containment, accessibility, context menus, and inert/disabled inheritance. For an open native popover, `authoredParent(of:)` still wins before block lookup. Sheets retain the presented-dialog connectivity fallback. Light dismissal inspects the whole window, so outside clicks in either pane reach it; event/logical ancestry is unchanged.

By inspection, [SplitMacTests.swift:853](/tmp/iosfix-rv-pane2/host/apple/tests/ExactKitTests/SplitMacTests.swift:853) **would fail without the fix**: it establishes that the invoker is outside `p.root`, and the old connectivity guard rejects opening. However, its assertions only establish open bookkeeping and native presentation count; they miss both failures above. The post-open loop also skips settling whenever opening succeeds synchronously.

Static review only; tests were not run because their fixture writes files.
