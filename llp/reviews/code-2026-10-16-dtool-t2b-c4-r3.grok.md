# Code review: LLP 1122 T2b step 4 (inspect mode, wasm page), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `81cb38c91` (commit `85c72e54e` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `0314dcb7160cf684184ce91e370ca2ba4ea1fa13267677bf764fffae7f9e7031`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 5 medium, 2 low).
- **Disposition:** taken in part, applied without re-review (round 3 was the last): a transparent cover over the whole page now takes the pointer while inspect mode is on, so no view sees hover, enter or leave, a press on a guest frame, or the start of a drag, and a press is read through it; focus, blur, change, input, compositionend and a dialog's cancel are taken too, so entering (which blurs) commits nothing; a guest frame that takes focus itself sends focus back to the close control; the banner's own events never pass on into the app (the close control is handled in the gate, Escape first), and the chrome is made again when a new tree or a modal opened later took it away, inside the topmost modal; a tab that was inspecting starts inspecting again on a reload, with the command it was on by, until the server's replay says otherwise, and an exit the server may not have heard is kept across the reload and answers the replay; a selection that finishes after an exit (or another command) is dropped, and a refresh does nothing when off; a refresh that finds two copies with the same path selects neither; a node with no modes reports none rather than the window's; the test now asserts no control takes focus while inspecting, and after the exit a focused button's Enter swaps (the positive control), which caught an exit pressed before the reload's replay being undone, fixed here. Not taken: a popover's light dismiss happens in the browser before any event a page can stop (its toggle reaches the app); a gesture already running when inspect turns on is not cancelled; the outline is the element's bounding box, not a transformed quad.

---

## High

**Hover still reaches the app.** `host/web/dev.js:429`, `host/web/dev.js:520`, `host/web/glue.js:577`

`pointerenter` and `pointerleave` are on the window capture list, but those events are not dispatched to ancestors at all, in either phase. The listener that actually runs is the one `attach` puts on the view itself, and it sends hover (kind 2) and unhover (kind 3) straight to the runner. `pointerover` is swallowed, and that does not stop the separate `pointerenter` that follows.

Scenario: inspect is on, and the pointer moves onto a node with a hover handler. The outline appears, and the runner still receives the hover and can change state.

Fix: the listener has to be on the view, not the window. Set a flag in `setInspect` and have the glue hover handlers return before `exact_dispatch` while it is set, including views attached after inspect turns on.

**A light-dismiss still closes the app's popover.** `host/web/dev.js:429`, `host/web/glue.js:563`

The gate never listens for `beforetoggle`, `toggle`, `cancel`, or `close`. Stopping the pointer event does not stop the hide: for a `popover="auto"`, the hide fires a cancelable `beforetoggle`, and if that is not canceled the popover closes and `toggle` runs the view's handler (kind 50). A dialog closed by `closedby` does the same through `cancel` and `close` (kind 49, `host/web/glue.js:583`).

Scenario: inspect is on, a menu popover is open, and the click that was meant to select a node lands outside it. The click is swallowed, the menu still closes, and the runner handles the toggle.

Fix: on `beforetoggle` and `cancel` during inspect, `preventDefault` and `stopImmediatePropagation` so the hide does not happen. Swallow `toggle` and `close` as well, so a hide that already started is not delivered.

## Medium

**Focus, blur, change, and compositionend are delivered before the blur.** `host/web/dev.js:524`, `host/web/dev.js:535`, `host/web/glue.js:583`

The gate blurs on `focusin`. `focus` is dispatched to the view first (kind 4), and `focusin` only starts after that. `setInspect(true)` also calls `document.activeElement.blur()`, which runs the view's blur handler (kind 5). A field that was mid-composition then gets `compositionend`, and glue writes the value on a timeout (`host/web/glue.js:525`). `change` and `input` are not in the gate either. Enter and Space themselves are prevented; this is the focus transition reaching the runner, not a second activation.

Scenario: inspect turns on while a field is focused, or a later commit calls `focusAutofocus()` (`host/web/glue.js:142`, `host/web/glue.js:878`). The runner receives focus, then blur, then the composed text.

Fix: swallow `focus` on the window in the capture phase (it has one), and ignore the blur, change, input, and compositionend caused by the page's own blur.

**A drag that already started keeps driving the runner.** `host/web/dev.js:526`, `host/web/group-glue.js:113`, `host/web/motion-glue.js:1037`

`setInspect(true)` never cancels an active contact. New pointer events are swallowed, so the gesture also never receives `pointerup`. The reorder edge pump does not need those events: while `phase` stays `active` it scrolls on its own `requestAnimationFrame` and calls `reorder-preview-into` or `reorder-preview` every frame. A pan that already scheduled a flush can dispatch one more move (`host/web/input-glue.js:425`).

Scenario: a list reorder is in the edge band when the tool turns inspect on. The list keeps scrolling and the runner keeps taking preview ops until the scroll hits the end.

Fix: on entry, cancel pointer capture, dispatch `pointercancel` to the active contact, and stop those pumps before the next frame.

**The banner is mounted inside the dialog, and its events are not stopped.** `host/web/dev.js:503`, `host/web/dev.js:554`

If a modal is already open, the banner and the outline are appended to that dialog, which is inside `#exact-root`. `swallow` returns immediately for any event whose target is inside the banner, without `preventDefault` or `stopPropagation`. The close handler does not stop the event either. The event then continues through the dialog: root's capture `pointerdown` walks the path and calls `exactPointerDown` (`host/web/input-glue.js:96`), and the collection listener treats it as a contact and enqueues a report (`host/web/collection-glue.js:357`). The click then bubbles to any ancestor with a press handler.

Scenario: a modal is open, inspect is turned on, and the person clicks ×. Inspect turns off, and the dialog's subtree still receives the pointerdown and the click.

Fix: keep the close control activatable, and stop every banner event from leaving the banner. `preventDefault` the wheel there too, so the page does not scroll under the banner.

**A dialog opened later buries the close control.** `host/web/dev.js:554`

The modal is looked up once, at show time. A `showModal()` after that is a newer top-layer node and makes the banner popover inert. Escape still leaves, because the key listener is on the window. A touch device has no other local way out, which is the way out D4 requires.

Scenario: inspect is on with the banner on `document.body`, and the app opens a dialog from a timer or a generation that was already in flight. The × control cannot be pressed.

Fix: on each dialog open, move the banner into the current top modal and call `showPopover` again so it is the topmost top-layer node.

**A local exit does not survive the reload that aborts its report.** `host/web/dev.js:532`, `host/web/dev.js:340`

Escape and × set `inspecting.left` and queue one `selected: null` post. That flag lives only in the document. `report` swallows a failed post and does not retry. `location.reload()` (`host/web/dev.js:589`, or the person reloading) aborts the in-flight fetch. The new document has `left === null`. If the server never accepted the null, register replays `on: true` and inspect turns back on.

Scenario: the person presses Escape, a program change reloads the page before the exit post is accepted, and the next tap is swallowed again. A reload after the post has landed stays off. A new stream in the same document is fine: `left` is still set, and the same `seq` is answered with the exit again.

Fix: remember the left `seq` in `sessionStorage`, and answer the replayed `on: true` with the exit until a register comes back with inspect off.

**A selection that resolves after Escape is published anyway.** `host/web/dev.js:480`

`select` awaits the runner and the design digest, then stores `inspecting.selected` and posts it. It does not look at `inspecting.on` again. `setInspect(false)` clears `selected` and queues the null report first; when `select` resumes it writes `selected` back and posts the node after the null. `refresh` (`host/web/dev.js:485`) does not check `inspecting.on` either, so a later refresh reports that node while the banner is gone. The owner stays cleared, because a normal `selected` report does not restore it.

Scenario: pointerup starts a selection, Escape lands while `selectedReport` is still awaiting, and the tool shows a selected node after the page has left inspect.

Fix: capture `on` and `seq` before the await, and drop the result if inspect turned off or the command number changed.

**The page test does not prove the claims in its name.** `scripts/tamber-server.test.mjs:417`

The button is focused after the banner is up, and the `focusin` handler has already blurred it before Enter and Space (`scripts/tamber-server.test.mjs:442`). The color staying put shows that an unfocused key did nothing, not that a focused control's key was blocked. The hover check only asserts that some outline is displayed (`scripts/tamber-server.test.mjs:422`). The hit comparison is `elementsFromPoint` plus the nearest `data-view` (`scripts/tamber-server.test.mjs:430`), which skips `nodeAt`'s root, exiting, and `views`-identity checks. Nothing in the test covers a guest frame, a popover toggle, a hover handler, an in-flight drag, or an exit whose post is aborted by a reload.

Fix: focus the control before inspect turns on and assert it is still focused when the key is pressed; assert the runner did not receive hover; and cover the popover, the banner-inside-dialog click, and an exit lost across reload.

## Low

**The outline is the axis-aligned box.** `host/web/dev.js:452`

`getBoundingClientRect` includes corners a transformed quad does not. A press in that corner correctly selects nothing, while the outline said the pointer was inside the node.

Fix: outline the transformed quad, or document the box as the bounds.

**A missing mode tuple is replaced with the window's.** `host/web/dev.js:471`

`node.modes ?? await modes()` calls `exact_set_mode("?")`, which is the window tuple (`host/web/src/host/mode.rs:32`). The runner omits `modes` when the view has no tuple (`runner/src/agent.rs:514`). The report then says that node is in the window's modes.

Fix: leave `tuple` null when the runner omitted `modes`.

Verdict: request changes — hover and popover light-dismiss still reach the app, and the close control, a reload during exit, or a selection that finishes after Escape undoes the exit.
