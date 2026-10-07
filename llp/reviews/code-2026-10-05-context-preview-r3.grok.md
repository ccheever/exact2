# Code review: context menus with a preview and a commit (LLP 1021 §5.1), round 3, 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd /private/tmp/x2-peek-review`; read-only by instruction.
- **Method:** one brief (sha256 `60b0644156dbab0114d1e321ab604dec2a0ae014d0210061ffd08a86505f0865`), shared with astra. Blind to astra's round-3 review. Reviewed the uncommitted diff in a detached worktree at `a51d6ecab`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Round 3, the last (rules/RULES.md: fix loops get three rounds); the fixes below were verified by the UIKit and AppKit tests, a simulator recording and both web listeners' syntax, not by a fourth review. 1 taken: a commit keeps its interaction until it completes, and a navigating commit returns the row in its animator's completion, then syncs. 2 taken on both web targets: a disabled or inert node opens nothing. 3 taken: the opener stops the event, as macOS consumes the click. 4 taken: `agentContext` answers whether it opened, and a node with only a popover that cannot open it lets the walk continue. 5 taken: §5.1 states the commit exception. 6 taken in the text: §5.1 and QUEUE now say iOS logs both, macOS a missing popover, the web neither; web logging is not added. 7 taken: an empty context menu no longer starts a presentation.

---

[P2] A navigating commit that drops its source removes the interaction and never puts the preview back — host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:82

`willPerformPreviewActionForMenuWith` sets `committed` before `presenter.press` (`ContextMenusIOS.swift:239`). That press applies its batch before it returns (`Session.swift` assigns `onPress` to `apply(runtime.press(...))`), and `apply` ends in `menus.sync()` → `context.sync()`. The guard at `ContextMenusIOS.swift:73` skips dismiss-and-restore while `committed` is set, which is the round-2 hold for the `.pop` morph. The removal loop just below does not share that hold: if the press unmounts the source or clears `contextPopover`, the source is absent from `wanted` and `removeInteraction` runs inside the commit callback. The restore after the press (`ContextMenusIOS.swift:248`) runs only when `activeKey` did not change, so a press that both navigates and drops the source leaves the row in the preview controller. The same function's comment says `willEndFor` may never arrive once the source is gone, and that callback is now the only path that returns the row. A synchronous `willEndFor` from `removeInteraction` instead restores the row before `.pop` is chosen, so the morph commits an empty controller.

[P2] The web opener shows the popover for a disabled or inert node — host/web-js/rt.js:727

`cp` and the wasm listener (`host/web/glue.js:431`) return only for a missing attribute, a field, or an opener that already ran. They then `preventDefault` and `showPopover`. iOS `configurationForMenuAtLocation` returns nil unless `menus.eligible` (live, enabled, not inert, not hidden, active route). macOS `MenuHost.context` returns unless the source is enabled and not inert. On the wasm host the node's own `contextmenu` already refuses that same node (`host/web/input-glue.js:215`, `:disabled` or an inert ancestor) and returns without stopping the event, so the opener still runs: the action does not fire and the popover opens. Registration order does not save it. At create, `applyProps` installs the opener before `attach` installs the action, and a later prop write installs the opener after; both orders reach `showPopover`.

[P2] A contextPopover node with no contextmenu handler still delivers an ancestor's contextmenu — host/web-js/rt.js:727

The opener never calls `stopPropagation`. Listeners on the same element still run either way, which is what lets the node's own action fire after the opener. Ancestors are different. A row that only names `contextPopover` — the case §5.1 allows, including under the agent — schedules `showPopover` and lets the event continue. The ancestor's `contextmenu` listener then runs synchronously (`rt.js:858` stops only at that ancestor), so `menuFor` on the ancestor can rewrite the shared popover before the timeout reads it, and the timeout anchors that popover to the inner node. macOS consumes the click: `NodeViewMac.rightMouseDown` (`NodeViewMac.swift:1482`) returns without `super` when the node names a popover, and a `super.rightMouseDown` is what reaches the ancestor.

[P2] An ineligible contextPopover tap is reported as delivered and swallows the walk — host/apple/Sources/ExactKit/IOS/AgentIOS.swift:545

The walk matches any non-disabled node that has a `contextmenu` handler or a non-empty `contextPopover`, then always returns `injected: true`. `MenuHost.agentContext` (`MenusIOS.swift:418`) rechecks `eligible` after the action and returns without opening when the node is inert, hidden, in an inactive route, or under a disabled ancestor. A node that only names `contextPopover` and fails that check used to miss the walk and produce `no contextmenu handler`. It now stops the walk, fires nothing, opens nothing, and answers success. A node higher up that would have opened is not tried. `eligible` is also what refuses the popover after the action disables or renames the source; that refusal is the same silent success.

[P3] §5.1 still says the preview returns the moment the source drops, including during a commit — llp/1021-menus.rfc.md:721

The paragraph says a source that is unmounted or stops naming the popover ends the menu with `dismissMenu` and the row goes back at once, without waiting for `willEndFor`. The host holds the row when `committed` is set (`ContextMenusIOS.swift:71`), and restores inside `willPerform` only when the route did not change (`ContextMenusIOS.swift:248`). A navigating commit is the exception the code and the round-2 disposition both describe, and the section does not.

[P3] A missing popover and a second preview are not logged on every host — host/web-js/rt.js:727

§5.1's deferred note, and the new `QUEUE.md` bullet, say the hosts log a `contextPopover` that names no popover and a second `contextPreview` row. iOS logs both (`ContextMenusIOS.swift:148` and `:181`). macOS logs a missing popover (`MenusMac.swift:195`) and skips every preview row with no count (`MenusMac.swift:374`). Both web listeners swallow a missing id in `catch {}` and never look at `data-context-preview`.

[P3] An empty macOS context menu still retires in-flight picks of that popover — host/apple/Sources/ExactKit/Mac/MenusMac.swift:200

`context` increments `presentations[pop.id]` and calls `revalidate()` before it knows the menu has an item. `valid` for a pick requires that same generation. A popover whose only row is the preview, or whose rows are separators, then hits the empty-menu return (`MenusMac.swift:203`) and shows nothing, after cancelling a pick that was waiting for the next turn.

## Assessment

**NOT READY.** The preview's home, reparent, and drop while the menu is up match §5.1, and so do the commit's eligibility walk, `.pop` only when `activeKey` changes, the dismissal target only after that change, `unanimated` across a deferred push and `reset`, and `agentContext` rechecking eligibility after the action. The two recorded deferrals still stand: the node pool refuses these rows, and a web timeout can open the pre-action popover when the page defers its commit.

The holes above are in the paths the earlier fixes left open. A commit whose press also drops the source is not covered by `ContextMenuLifetimeIOSTests` (that test clears `contextPopover` while the menu is merely open). Nothing asserts a disabled or inert opener, an ancestor `contextmenu` beside a `contextPopover`-only child, or an agent tap that `agentContext` refuses. `MenuHost.context` and `rightMouseDown` remain untested because `popUp` is modal, which §5.1 already says.
