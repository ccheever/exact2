# Code review: context-menu rows in the iOS node pool (LLP 1021 §5.1), round 2, 2026-10-05 (grok)

- **Family:** xAI. `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, `--cwd` a detached worktree at the reviewed commit; read-only by instruction.
- **Method:** one brief (sha256 `3ea3f70245bfa10deefda4ed0c65042a8a3ec6b8fda51c70549f3bfa779b8777`), shared with astra. Blind to astra's round-2 review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** nothing to take. Its note on the presenter's reset (the feedback generator left on a parked view released in the same reset) is astra's 2, taken.

---

**No findings.**

**Verdict: READY.** The three round-1 fixes hold, and the new cleanup does not open another path that carries the old node into the next one.

Taken findings:

1. **At rest is `.possible` and no touch.** `contextual` in `NodePoolIOS.swift:355` refuses a recognizer unless `state == .possible` and `numberOfTouches == 0`, and that guard runs before both the host's companion check and the `contextRecognizer` identity check. A long press waiting out its duration still has a touch, so the row is destroyed instead of parked. The same check applies to the recognizers UIKit added with the interaction.

2. **Set-aside focus stays with the node it was taken from.** `MenuFocus.restore` in `MenuFocusIOS.swift:89` keeps the incarnation captured when the responder was set aside and returns without calling `becomeFirstResponder` when that incarnation differs or the view is no longer `presenter.views[id]`. Parking zeroes the incarnation (`NodePoolIOS.swift:289`) and `rebind` issues a new one (`NodePoolIOS.swift:535`), so a lent view fails the check. A node that was not parked still matches and is focused. The bar path still holds `ExactView`, which is not a `NodeView`, so that restore is unchanged. `testASetAsideFocusIsNotGivenToTheNodeItsViewIsLentTo` drives this.

3. **Leaving the pool drops the interaction and the recorded companions.** `drop` (`NodePoolIOS.swift:149`) calls `dropped` for eviction, a list whose window is gone, and `pool.reset()` (the memory-warning path). `dropped` (`ContextMenusIOS.swift:171`) detaches every interaction this host still has on the view. `detach` (`ContextMenusIOS.swift:150`) removes the recorded companion interactions, including the click-presentation feedback generator, and deletes that interaction's record. `ContextMenuHost.reset` clears `recognizers` and `parkedViews`.

Nothing of the old node is left for the new one. `parked` takes the interaction out of the id map and leaves it on the hidden view; the delegate reads the node from the view and refuses a view that is not `presenter.views[id]`. The next `sync` adopts that same interaction only when the new node names a popover (`ContextMenusIOS.swift:104`); otherwise it detaches it and turns `contextRecognizer` back on. That recognizer's action reads `self.id` after `rebind`. `open` never points at a parked view: `recyclable` rejects `open.source`. A lend with no `contextmenu` handler removes the long press in `handlers`' `didSet` before `sync`. Nested and paired menu nodes are per view, and one ineligible view keeps the whole tree out.

The companion snapshot cannot admit a foreign recognizer on any path in this host. `sync` records only objects that appear during `addInteraction`, after the row's own recognizers exist, and `came` looks them up through a live interaction on that view. A recognizer added later fails `recyclable`. `detach` deletes the record when the interaction comes off, so a freed companion id is not reused as proof while that interaction is gone.

tvOS does not build `ContextMenuHost`. Interactions still refuse a park. Only a `contextRecognizer` at rest is new there, and it also reads the current id. Session teardown calls `menus.reset()` before `pool.reset()`, so `dropped` no longer has companion ids and does not itself remove the feedback generator; the tree is released in that same `pool.reset()`, and the view is not lent again.

The new tests cover lend, lend without a popover, lend with no `contextmenu` handler, `pool.reset()`, set-aside focus onto a lent view, and a row whose menu is up. They still call the delegate directly, so they do not show a real UIKit dismissal or a touch that is still being judged. The "eight recognizers" claim is not counted. No test here shows a foreign recognizer or interaction refusing a park; that remains the old `allSatisfy` rule.
