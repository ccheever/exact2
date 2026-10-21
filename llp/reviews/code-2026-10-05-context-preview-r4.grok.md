# Code review: context menus with a preview and a commit (LLP 1021 §5.1), round 4, confirmation of the landed 7a1ac6826, 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at `7a1ac6826`; read-only by instruction.
- **Method:** one brief (sha256 `a67041dbdcdd20c848a1d620553b0f2cf78246739c259f99d09878770a53b926`), shared with astra. Blind to astra's round-4 review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 4 was asked for by the lead after landing, to confirm the unreviewed round-3 fixes; the fixes below land with the focus fix (MenuFocusIOS) and were verified by the UIKit tests and a simulator recording, not by a fifth review. 1 taken (astra's 1). 2 taken: `unanimated` is held across a deferred sync only when the press changed the route.

---

[P2] A non-navigating commit that drops its source leaves the interaction installed and the long-press action disabled

`host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:251` sets `committed` before `presenter.press`. That press applies its batch before it returns, and the batch ends in `menus.sync()` → `context.sync()`. The removal loop at `ContextMenusIOS.swift:83-90` now skips this interaction while `committed` is set, so it neither calls `removeInteraction` nor re-enables `contextRecognizer` (`:88`, disabled at `:100` when the interaction was installed).

When the route did not change and the source is gone or no longer names that popover, `ContextMenusIOS.swift:260-263` puts the row back, sets `open = nil`, and returns. It does not call `sync()`. The navigating path does, in the commit animator's completion (`:275-279`). `willEndFor` (`:301-309`) cannot finish the job either: it returns immediately unless `open` is still this owner, and this branch has already cleared `open`. UIKit delivering the dismissal callback does not remove the interaction.

A source that survives with `contextPopover` cleared keeps a `UIContextMenuInteraction` whose configuration returns nil (`:157`) and a disabled `contextRecognizer` (`NodeViewIOS.swift:188-196`). The plain `contextmenu` action does not fire, and the menu does not open, until some later batch happens to call `sync()`. A source that was destroyed leaves its interaction in the `interactions` dictionary until that same later sync. Renaming the popover still works, because the surviving interaction reads the new id at event time. Clearing it, which this branch is written to handle, does not.

This is the round-3 hold (grok r3 finding 1) applied to the non-navigating invalidation path that round 2 already restored the row for. Before that hold, the sync inside `press` removed the interaction. `ContextMenuLifetimeIOSTests` clears `contextPopover` only while the menu is open, not from inside the commit.

### 2. [P3] A preview press that does not navigate, overlapping a transition, suppresses the next stack animation

`ContextMenusIOS.swift:255-257` sets `NavigationHost.unanimated` before every preview press and clears it afterward only when `inTransition` is false. `inTransition` is `changing || syncing || mounting || pendingSync` (`NavigationIOS.swift:517-518`). A sync that bails because a navigation or modal transition is already running sets `pendingSync` and returns at `NavigationIOS.swift:326`, before `unanimated = false` at `:352`.

If that press left `activeKey` unchanged, there is no commit push to protect. `didShow` then flushes the deferred sync (`NavigationIOS.swift:717-719`) and `setViewControllers` runs with `animated: false` (`:347`). The waiting stack update loses its animation. The navigating hold and the `reset` clear (`:762`) are intact.

## Taken fixes

Round 3, the fixes that had not been reviewed:

- Astra 1, kept. `inPopover` (`ContextMenusIOS.swift:131`, restore at `:213`) sends a row that stayed in its popover back to that popover's container after glass replaces it. Lift, add glass, children op that still lists the row, remove glass: the content view is released and the weak container goes nil, and restore still inserts into `popover.container` as it is then.
- Astra 2, kept. `place` sets `center` (`:120`), and `home` is the untransformed layout box (`:112-114`, recorded from `applyGeometry` after the transform is put back, `PresenterIOS.swift:1032-1043`).
- Astra 3, kept. `committable` ignores the popover's own `isHidden` and refuses an authored `display: none` (`ContextMenusIOS.swift:294-295`). The lifetime test covers the row, the popover, and a staying press.
- Astra 4, kept. The macOS guard checks `hidden(source)` after the action (`MenusMac.swift:192-193`), including `display: none` (`:82-85`).
- Grok 1, only on the navigating path. See finding 1. The completion restores the row and then syncs.
- Grok 2, kept. Both listeners return for `:disabled` and an inert ancestor before `preventDefault` (`rt.js:727`, `glue.js:431`).
- Grok 3, kept. The same listeners `stopPropagation` on the path that opens the popover, so an ancestor does not hear it. The node's own listener is on the same element and still runs.
- Grok 4, kept. `agentContext` returns whether it opened (`MenusIOS.swift:418-427`). The walk continues only when that is false and the node has no `contextmenu` handler (`AgentIOS.swift:545-551`).
- Grok 5 and 6, kept in the text. §5.1 states the commit exception (`llp/1021-menus.rfc.md:721-726`) and the logging split. The web still does not log.
- Grok 7, kept. An empty macOS menu returns before `presentations` is incremented (`MenusMac.swift:201-202`).

Earlier taken fixes still hold on inspection: configuration reported only after a provider runs, `willEndFor` matched by configuration object, immediate restore when a non-committed source drops, dismissal target only after `navigated`, `unanimated` cleared when a sync applies and in `reset`, frame ops recorded including an origin-zero box of the same size, and `agentContext` rechecked after the action.

The two deferrals stand. The pool still refuses a view that has an interaction, and the web timeout can still open the pre-action popover when a bound `contextPopover` changes and the page defers its commit.

**Verdict: NOT READY.**
