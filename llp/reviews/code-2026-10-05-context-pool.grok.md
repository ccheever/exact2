# Code review: context-menu rows in the iOS node pool (LLP 1021 §5.1), 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at `bc885b72e`; read-only by instruction.
- **Method:** one brief (sha256 `d1e1aaeaa90e36845c4323188aaeed9f8d2ab77315a974480b10176f8eed44e1`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken (astra's 2). 2 taken (astra's 3). The test remarks taken in part: the lend to a node with no `contextmenu` handler and the drop are tested now, and so is that nothing UIKit added is left on the view; a foreign recognizer refusing the park is the pool's existing rule, tested in NodePoolIOSTests. Fixing these found one more, also taken: UIKit leaves a `_UIClickPresentationFeedbackGenerator` interaction on a view when only the menu's interaction is removed (measured), which kept such a view out of the pool for good; the host now removes the interactions recorded beside its own (`detach`), tested.

---

[P1] A set-aside focus is restored onto the row that reused the view — `host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift:361`

`contextual` only refuses the view that is `open.source`. `idle` only refuses a current first responder. `MenuFocus.setAside` has already resigned that responder and kept the view in `held` (`MenuFocusIOS.swift:63`). A parked row stays in the window, so `MenuFocus.restore` (`MenuFocusIOS.swift:86`) passes its `window != nil` check and calls `becomeFirstResponder` on it. That callback does not look at incarnation (zeroed at park, replaced at take), and it runs with `quiet` set, so the new row becomes first responder without a `focus` event. Enter, Space, and Tab then hit the new row.

This shows up on the lists this change pools. Previously every context-menu row was destroyed, `removeFromSuperview` cleared `window`, and restore did nothing. Now: tap row A (a button becomes first responder in `touchesEnded`), long-press row B, and B's menu calls `setAside`, which holds A. A batch while B's menu is up — the `contextmenu` action, or any other list update — retires A. A is not `open.source`, so it parks and is lent to row C. When B's menu ends, restore focuses C. The new tests never set a first responder or call `setAside`.

[P2] Evicting or dropping a parked menu row never releases the interaction or its companion set — `host/apple/Sources/ExactKit/IOS/ContextMenusIOS.swift:157`

`parked()` takes the interaction out of the id map and leaves it on the view, recorded in `recognizers` and `parkedViews`. `NodePool.drop` (`NodePoolIOS.swift:149`), which eviction, a list leaving the window, and the memory-warning `reset` all use, only `forget()`s the views and removes the root. It does not tell the host. `ContextMenuHost.reset` (`ContextMenusIOS.swift:367`) removes interactions still in the map and clears `recognizers`, but parked interactions are already out of the map, and `parkedViews` is left as it is. Session teardown hides this because `menus.reset()` runs and then `pool.reset()` drops the views. A memory warning and ordinary eviction do not: each evicted context-menu row keeps a `recognizers` entry for the life of the process. Those entries are `ObjectIdentifier`s of objects that have been freed, which is what `came` treats as proof that a recognizer belongs to the menu.

The companion snapshot itself does not mis-label a host recognizer. `sync` records only objects that appear during `addInteraction`, after handlers have already installed the long-press, pointer, swipe, and reorder recognizers, so those stay in the "before" set and still fail `recyclable`. Anything UIKit adds inside that call is admitted for the life of the interaction. Nothing in the host adds a non-menu recognizer there. A recognizer added later is refused, which fails closed.

tvOS is consistent with that split. `ContextMenuHost` is iOS-only, so interactions still block parking. The long-press is allowed only in `.possible`, and `openContext` reads `self.id` when it fires, after rebind. Nested and paired menu nodes are per view: a child whose menu is up fails `recyclable` for the whole tree; on the next sync each parked view keeps the interaction only if that view is in `wanted`. A lend to a node with no `contextmenu` handler removes the recognizer in `handlers`' `didSet` before sync enables whatever is left.

The new tests do not prove those claims. `testARowWithAContextMenuIsLentToTheNextRow` checks that one `UIContextMenuInteraction` object survived; the message about eight recognizers is not asserted, and a foreign recognizer or interaction is never shown to refuse parking. The lend with `menu: false` still passes `handlers: ["press", "contextmenu"]`, so it never covers a node with no `contextmenu` handler. `testARowWhoseMenuIsUpIsDestroyedAndItsMenuEnds` calls the delegate directly, never presents a menu, and treats `open == nil` as the menu having ended. Eviction, memory warning, `reset`, nested menus, two menu nodes, focus set-aside, and tvOS are untested. The tests are iOS-only.

**NOT READY**
