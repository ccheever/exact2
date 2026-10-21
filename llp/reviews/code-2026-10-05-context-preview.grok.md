# Code review: context menus with a preview and a commit (LLP 1021 §5.1), 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd /private/tmp/x2-peek-review`. `--always-approve` because plan mode cancels shell reads; the brief instructs read-only, and the worktree's diff was unchanged afterwards. Two earlier `-p` runs printed only their opening sentences and are not reviews.
- **Method:** one brief (sha256 `8c822ce06355327fa9ad6e2c71113dd7be34eafb6e88b22e3a65ee3e010eda9f`), shared with astra. Round 1, blind to the other review. Reviewed the uncommitted diff in a detached worktree at `a51d6ecab`. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken (astra's 5). 2 taken (astra's 7). 3 taken: a configuration is reported only once a provider has run, and `willEndFor` matches its configuration object; a source gone while the menu shows returns the row (astra's 1). 4 DEFERRED, to QUEUE and §5.1: the pool already refused these rows for their `contextmenu` recognizer. 5 taken: `unanimated` is held while the stack defers the push and cleared when it applies. 6 taken: a children op that moves the row to another parent makes that its home (test). 7 taken (astra's 6).

---

**NOT READY.** The long-press path that navigates and restores is in place, but the lifted row's home, a commit that stays, and a menu that never reaches `willEnd` all disagree with `llp/1021-menus.rfc.md` §5.1, and the new tests do not catch them.

## Defects

### 1. A frame op at the origin is not the row's home

`host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:94`

`place` records a new home only when `preview.frame.origin != .zero`, then snaps the on-screen frame to the origin. That is how a later `sync` avoids saving the snapped rect. A kernel frame whose origin is `(0, 0)` is indistinguishable from that snapped rect, so `owner.home` keeps the old size and position. `restore` (`:177`) writes that stale rect back. The kernel will not send an unchanged frame again, so the row stays wrong in the popover after the menu.

§5.1 (`llp/1021-menus.rfc.md:713`) says a batch's frame op is the row's new home in the popover. The section's own example is the first child of a column with no padding, so its origin is `(0, 0)`. `applyGeometry` (`PresenterIOS.swift:1033`) writes that frame onto the lifted view before `menus.sync` calls `place`. The fixture popover has `padding=8` (`apps/native-fixture/app.contract:339`), and `ContextMenuIOSTests` only asserts the snapped on-screen origin (`ContextMenuIOSTests.swift:79`), so a resize of an unpadded preview never runs.

### 2. A preview press that does not navigate still has no dismissal target

`ContextMenusIOS.swift:198` and `:208`

`previewForDismissingMenuWithConfiguration` returns nil whenever `open.committed` is true. `willPerformPreviewActionForMenuWith` sets `committed = true` before it compares `activeKey`, for every press that passes the eligibility guard. The style is then `.dismiss` when the route did not change (`:216`), but the dismissal target is already suppressed.

§5.1 (`:722`) reserves a nil dismissal target for a commit that navigated, because the source is under the new screen. A press that stays is `.dismiss` (`:734`) and should target the node. The comment on `:197` says the same thing; the condition does not. `ContextMenuIOSTests.swift:131` replaces `onPress` with `{ _ in }` and asserts `.dismiss` and `animated == 0`. It never calls `previewForDismissingMenuWithConfiguration` on that commit. The navigating test (`:95`) does, and that case is fine.

`committed` also drives `observation`'s `phase` (`:247`). A non-navigating press is reported as `commit`. That part matches "phase `commit` after a commit" (`:738`). The dismissal target does not.

### 3. `open` survives a gesture that never becomes a menu

`ContextMenusIOS.swift:129` and `:222`, `MenusIOS.swift:441`

`configurationForMenuAtLocation` assigns `open` before either provider runs. The comment on `:129` records that a press released before the menu showed never reaches `willEnd`. Nothing else clears that `Open`. `willEnd` (`:223`) returns unless the configuration id still equals `owner.source?.id`, and its completion (`:225`) clears `open` only when it is still that owner. `observation` (`:242`) reports `{kind: contextmenu, phase: open}` whenever `open` is set. `MenuHost.observation` returns that first (`MenusIOS.swift:443`), ahead of an agent popover or a confirmation.

§5.1 (`:695`, `:738`) says configuration reads nothing and must not fire `contextmenu` because UIKit asks before the press has become a long press, and `state.navigation.popover` is reported while the menu is up. A tap that asked for a configuration and then ended leaves the agent and `state.navigation.popover` describing a context menu that is not up, until the next configuration or `reset`. The same guard drops the preview on the floor if the source view is already gone when the menu does end: `source` is weak, `id == owner.source?.id` fails, `restore` does not run, and the row stays in `owner.controller`. `sync` (`:84`) may call `dismissMenu` when the source dies, and that `willEnd` then fails the same check.

### 4. A `contextPopover` row cannot be parked

`host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:326`

`recyclable` requires `v.interactions.isEmpty`. `ContextMenuHost.sync` (`ContextMenusIOS.swift:73`) installs a `UIContextMenuInteraction` on every non-empty `contextPopover` outside agent mode. `park` (`NodePoolIOS.swift:263`) refuses the whole subtree when any view fails that check. The chat list §5.1 is written for is the consumer that virtualizes rows, and every such row now misses the pool. `sync`'s comment on `:75` ("a recycled view may still hold one") describes a reuse path the pool gate makes unreachable. Behavior is safe: ids are not reused, and there is no cross-row menu. The cost is rebuilding every context-menu row.

### 5. A deferred push ignores `unanimated`

`ContextMenusIOS.swift:210`, `NavigationIOS.swift:326` and `:713`

The commit sets `NavigationHost.unanimated` around `presenter.press`, and `onPress` applies the batch before returning (`Session.swift:688`). On a settled screen, `setViewControllers` runs inside that call with `animated:` false (`NavigationIOS.swift:347`). If `changing` or a modal transition is already set, `sync` stores `pendingSync` and returns before `setViewControllers` (`:326`). `didShow` flushes that later (`:713`) with `unanimated` already false, so the stack animates. `activeKey` is the `navigationKey` prop, so the style is already `.pop` and an animation was added. §5.1 (`:727`) turns the stack animation off for that batch because `.pop` plus the stack's own push is the combination the section says was measured as wrong.

### 6. Dropping or moving the preview while it is lifted loses its parent

`ContextMenusIOS.swift:102` and `:175`

A children op whose parent is still `owner.parent` updates `index` when the row is in the new list. That matches §5.1 (`:714`): the row stays out, and `placeChildren` skips `menus.lifted` (`PresenterIOS.swift:990`). If the op omits the row, `owner.parent` becomes nil. `restore` then `removeFromSuperview`s it and does not insert it. A reparent in the same batch hits the same hole from the other side: the new parent skips the lifted view, and the old parent has cleared `home`. No later children op is guaranteed. The row is still live and has no superview.

A same-parent op that keeps the row, and a destroy of the row itself (`preview` is weak, `restore` no-ops), behave as the section describes.

### 7. Clearing `contextpopover` leaves the listener that swallows the browser menu

`host/web/glue.js:420` and `:435`, `host/web-js/rt.js:632` and `:726`

Both listeners call `preventDefault` before resolving the popover, and both read the attribute at event time. The wasm clear path is `removeAttribute` (`glue.js:420`); it never clears `exactContext` or removes the listener. On the JS target, `P` removes the attribute when the binding goes null (`rt.js:632`) and `cp` is registered once at element construction (`emit.rs:1176`, `rows.rs:523`), not inside the effect, and never removed.

After the prop is cleared, a later `contextmenu` still prevents the browser menu and opens nothing. While the attribute is present, ordering matches §5.1 (`:748`): the popover listener is registered before the node's handler (`glue.js:627` `applyProps` then `attach`; `emit.rs:1043` `element_extras` before the handler loop at `:1196`), `stopPropagation` does not skip the other listener on the same node, and `setTimeout(showPopover)` runs after the synchronous action flush (`Session`-equivalent `send` → `applyBatch` in `glue.js`, the JS runtime's synchronous apply). `ev.$cp` keeps only the innermost opener. A field target returns before `preventDefault` in both (`rt.js:727` and `:858`).

## Elsewhere the change matches the section

- Schema ids 238 and 239 (`kernel/tables/schema.json:1376`), `tags.rs`, and the web DOM names `contextpopover` / `data-context-preview` (`element.rs:871`) line up. `ChromeIndex` carries `contextPopover`.
- `sync` removes a stale interaction and re-enables `contextRecognizer`, then adds one and disables the recognizer (`ContextMenusIOS.swift:66`). Agent mode installs nothing, so the recognizer stays on and `openContext` (`NodeViewIOS.swift:216`) reaches `agentContext`. The `AgentIOS.swift:545` condition is one `if` clause: disabled is still required, and a node with only `contextPopover` opens the painted popover.
- `fire` is the first call in both providers (`:144`, `:153`) and runs once through `contextmenu(_:line:)` → clipboard kind 10, which applies before returning (`Session.swift:745`). A nested `sync` during that batch sees no lifted preview yet.
- The navigating commit presses by view id, wraps `unanimated` on the settled path, sets `.pop`, and adds `layoutIfNeeded`. `eligible` runs before the press. macOS `rightMouseDown` (`NodeViewMac.swift:1480`) fires `contextmenu` and then `MenuHost.context`, which builds the `NSMenu` on the next turn with `source: nil` (`MenusMac.swift:189`), so a pick is not rechecked against an invoker. Under the agent that path calls `show`.
- `items(of:)` and `menu(of:)` skip `contextPreview`. Two preview rows are logged and left out of the menu (`ContextMenusIOS.swift:156`).

## Tests and the LLP

`ContextMenuIOSTests` calls the delegate directly. It shows that configuration does not increment `peeks`, that `lift` then `menu` fires once, that the padded fixture's preview is 300×180 at the controller origin, that the menu is Bump | Clear with destructive on Clear, that a real `detail` press answers `.pop` with one animation and a nil dismissal target, and that `willEnd(animator: nil)` puts the row back in the hidden popover. It does not drive UIKit's provider closures, a frame op, a children op, unmount, recycling, a cancelled configuration, or the dismissal target after a press that stays.

`ContextMenuMacTests` calls `menu(of:)` and `pick` on a synthetic popover. It never calls `context` or `rightMouseDown`, so it does not cover event order, the popup point, the async guards, or the agent `show`.

§5.1's Evidence paragraph (`:772`) says the iOS tests prove `contextmenu` fires once before either provider reads, and that a press that stays answers `.dismiss`. The first is the order of two direct calls, not of UIKit's closures. The second is the style only. The design text at `:713` and `:722` does not match `place` or `previewForDismissingMenuWithConfiguration`, as above.

## Verdict

**NOT READY.** Fix the origin-zero home, key the nil dismissal target off the route change, and clear `open` when a configuration never becomes a menu (including when the source is already gone). The pool gate, the deferred `unanimated` push, a children op that drops the lifted row, and the web listener that outlives the attribute should land with that.
