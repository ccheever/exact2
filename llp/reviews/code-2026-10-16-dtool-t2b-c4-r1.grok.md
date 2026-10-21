# Code review: LLP 1122 T2b step 4 (inspect mode, wasm page), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `85c72e54e`.
- **Method:** one brief (sha256 `523a2b1b151db020e825b746b2e6cc95bc83be9ec951a42751dc31c73cc5b638`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 high, 7 medium, 3 low).
- **Disposition:** taken but 8 (astra) and Grok's low items in part: inspect mode now takes every input itself (keys too, Escape and the banner aside; pointer over/out/enter/leave and moves; wheel; Safari gestures; drags), blurs the focused element on entry, keeps guest frames from taking pointer input (a press there is their view's), and the GPU surface's own window key listener stands down while it is on; a press released where it began (mouse, pen or finger) selects, never a click (so a keyboard click selects nothing, and touch selects); the hit is the topmost element, nothing when the host's chrome is on top, and the view the runner knows by that element; the node's own modes are reported (the runner's `layout` answer now says `modes`, the tuple the instance tree keeps per node), so a node in a pinned subtree reports its pin's; a refresh after another generation finds the selection by its site and instance path or says it is gone; the report is measured in bytes as the server does, and refused, named, when the identity alone does not fit; a build that registers again is told inspect mode as the server holds it (a reload keeps it; an owner gone while it was away turns it off); the page leaving inspect itself reports a selection of none, which drops its owner; the polled queue keeps only the latest inspect and one refresh; ownership goes with an evicted or replaced build; inspect needs write scope and names `on` or `refresh`; the e2e test now covers Enter and Space on a button focused before, a finger tap, a pinned preview's modes, a reload and the local exit. In part: the banner and outline go to the top layer (a popover) when shown; a modal opened after them is still above. Not tested: guest frames, the GPU keys, a body over 64 KiB, a generation change (no fixture for them here); ownership loss through the presence timer is the channel's `clear`, tested directly.

---

## High

**1. Keyboard still activates the app, and a keyboard click selects whatever is at the viewport origin.** `host/web/dev.js:423`, `host/web/dev.js:458–465`, `host/web/dev.js:487`.

`INSPECT_EVENTS` has no `keydown`/`keyup`. The only key listener is Escape. Everything else keeps going:

- Tab moves focus, because that `keydown` is not canceled. Enter or Space on a focused `<button>` then fires a trusted `click` with no pointer position, so `clientX`/`clientY` are 0 (`UI Events`). `swallow` stops the button’s `press` (`host/web/glue.js:545`) and then calls `nodeAt(0, 0)` and `select`. The report is the view at the top-left of the viewport, not the focused control.
- `key` handlers are `keydown` on the element (`host/web/glue.js:519`). They run after window capture. So does the document key path: shortcuts and `el.click()` for a non-button pressable (`host/web/input-glue.js:120–176`), and Enter `submit` (`host/web/document-glue.js:100–113`). Typing in a focused field fires `beforeinput`/`input` the same way.
- A GPU surface registers `window` capture `keydown`/`keyup` when it is created (`host/web/gpu-glue.js:632–633`), which is before inspect is turned on. Those listeners run first on the same target. Space, Enter, and arrows are sent into the canvas (`host/web/gpu-glue.js:619–654`) before the Escape-only listener can see them. Arrow, Space, and PageDown also scroll, and a `scroll` listener then reports to the app (`host/web/glue.js:552–554`).

Scenario: inspect is on, focus is on Swap pins (Tab, or focus from before inspect), press Enter. The fixture does not swap, and the tool receives a selection for the node at (0, 0). On a page with a canvas, Space still drives the GPU app.

Fix: on entering inspect, add a window capture `keydown`/`keyup` that is registered so it runs before the GPU listeners (or remove those listeners for the session). `preventDefault` and `stopImmediatePropagation` on every key except Escape and the banner controls. Select only from a primary pointer gesture (`pointerup`, not a drag). Ignore a `click` with `detail === 0` or an empty `pointerType`.

**2. A reload leaves the server owning inspect while the new document is live.** `scripts/tamber-builds.mjs:211–215` (pin replay), `scripts/tamber-builds.mjs:105–106` and `296–298` (inspect ownership), `host/web/dev.js:218` and `512`.

Register replays the pin frame and does not write `{kind:'inspect', on:true}` when `inspectors` still names this build. A program change and `message.fresh` both call `location.reload()`. `sessionStorage` keeps the build id (`host/web/dev.js:308–325`), so the new document registers as the same build, gets its pin, and does not get inspect. Page state is reset, so capture is off. The tool still shows inspect on and will not send `on: true` again if its toggle did not change. Taps reach the app. RFC D4 (`llp/1122-design-tool.rfc.md:337–338`) replays pins and holds on reconnect; inspect is the same owned session and is the one that must be on for the gate to hold.

Replacing a build id deletes `pins` and `sent` but not `inspectors` (`scripts/tamber-builds.mjs:204`, `209`), so a replaced or evicted id keeps its client in `owners()` after the build is gone.

Fix: on register, if `inspectors` has this id, `s.write({kind:'inspect', on:true})`. Delete the `inspectors` entry when that id is replaced or evicted.

**3. A tap inside an iframe or guest frame never meets the capture listeners.** `host/web/dev.js:485`, `host/web/glue.js:531–536`, `host/web/input-glue.js:533–549`.

Listeners are on the parent `window`. A pointer event whose target is a document inside an iframe does not propagate to that window. The wasm document mounts iframes (`host/web/src/document.rs:704–711`), and `guestTap` delivers pointer and click inside the guest document. Cross-origin content cannot be scripted from the parent; covering it can. While inspect is on, a tap in that frame activates the guest and selects nothing.

Fix: for the inspect session, put a viewport overlay (`pointer-events: auto`) above the app and below the banner, and select from that overlay’s `pointerup` with `elementsFromPoint`. Set `pointer-events: none` on same-origin iframes as well. The overlay is also the reliable way to keep hover and touch off the app.

## Medium

**4. A finger tap is swallowed and not selected.** `host/web/dev.js:423`, `host/web/dev.js:458–462`.

Selection runs only for `click`. The same listener `preventDefault`s `touchstart`. Touch Events tell the user agent not to dispatch the mouse events that follow a canceled `touchstart`, which in Chrome has included the synthetic `click`. Pointer Events Level 3 says canceling a pointer event must not suppress `click`; that covers the mouse path the test uses (`page.mouse.click` at `scripts/tamber-server.test.mjs:425`), not this touchstart cancel. A finger tap then neither reaches the app nor produces a `selected` report. `touch-action` panning is also not a pointer-event default; canceling `touchmove` is what stops it, and a non-cancelable `touchmove` can still scroll.

Fix: select on primary `pointerup` at the down point when the gesture was not a drag, for `pointerType` `touch` as well as `mouse`. Do not depend on `click`.

**5. Hover still reaches the app.** `host/web/dev.js:464`, `host/web/dev.js:486`, `host/web/glue.js:577–578`.

The `pointermove` listener only moves the outline. It does not `preventDefault` or stop propagation, and `pointerenter` / `pointerleave` are not in `INSPECT_EVENTS`. The app’s `hover` handler is `pointerenter` on the element. Canceling `pointerdown` does not stop `mouseenter` / `pointerenter`. Moving the pointer runs hover handlers (menus, highlights, state) while the outline is showing.

Fix: stop `pointermove`, `pointerover`, and `pointerenter` in the capture listener that also updates the outline, or let the overlay from finding 3 be the hit target so the app never sees the pointer.

**6. Escape and the banner close the page only.** `host/web/dev.js:465`, `host/web/dev.js:480`, `host/web/dev.js:496–498`, `scripts/tamber-builds.mjs:307–308`.

Both call `setInspect(false)` and post nothing. `inspectors` still names the client, and no event tells the tool. The page takes taps again (the test asserts this) while ownership still says inspect is on. A later `refresh` re-reports `inspecting.selected` even though inspect is off. RFC D4 requires the local way out; it also treats inspect as that client’s until the client is gone. After Escape those two disagree.

Fix: on local exit, post a report the channel handles by deleting that inspector and announcing it. Clear `inspecting.selected` when inspect turns off, so a refresh after exit does not re-report.

**7. The 64 KiB trim measures the wrong string, and an oversize report is destroyed with no event.** `host/web/dev.js:425`, `host/web/dev.js:452–455`, `scripts/tamber-builds.mjs:47`, `scripts/tamber-builds.mjs:327–329`.

`room()` is `JSON.stringify(selected).length`, UTF-16 code units of the nested object, against 60 KiB. The POST is `{nonce, kind, selected, plan, design, revision}` (`host/web/dev.js:329`, `455`), and `serveReport` counts UTF-8 bytes against 64 KiB. A row value with non-ASCII text (one code unit, up to three UTF-8 bytes) passes the check and the socket is destroyed. The selection is dropped: no `selected` event, no `unsupported`. The first shrink copies `{name, value, source, from, token}`, which is already the runner row (`runner/src/agent.rs:537–554`), and sets `trimmed: true` before anything was removed. If the rows are emptied and the wrapper is still over the room, that body is still returned.

Fix: measure `Buffer.byteLength` of the exact POST body, drop rows until it fits, and if it still does not fit, report `unsupported` instead of posting. Set `trimmed` only when a row was removed.

**8. `selected.tuple` is the window’s mode tuple, not the node’s.** `host/web/dev.js:348`, `host/web/dev.js:451`, `runner/src/instance/theme.rs:29–30`, `runner/src/instance/theme.rs:115–127`.

`tuple` is `exact_set_mode("?")`. The runner already stores a per-visit app tuple, the window’s overridden by pins above (`tuple_of`, `boundary` for an `-exact-mode` pin or a `color-scheme`). The agent `node` answer does not include it. This commit’s theme fixture is the counterexample: Light, Dim, and Black previews sit in one window with their own `color-scheme` and `-exact-mode` (`apps/theme-fixture/app.contract:34–41`). A click in Dim reports the window’s modes, so the branch D5 asks for (`llp/1122-design-tool.rfc.md:368`) is the window’s. A row’s `token.modes` is that token’s `when` clause (`runner/src/agent.rs:430–447`), not the node’s tuple. The protocol text was changed to say “the window’s mode tuple” (`docs/tamber-protocol.md:615`), which matches the code and diverges from D5. The commit message’s claim that the runner keeps no per-node tuple is wrong about the instance.

Fix: add the effective tuple the runner already computed to the `node` answer, and put that in `selected.tuple`.

**9. An in-place generation leaves the old selection and a detached outline.** `host/web/dev.js:233`, `host/web/dev.js:436–440`, `host/web/dev.js:457`.

`applied` replays a held pin and does not re-select. The outline updates only on `pointermove`, from `getBoundingClientRect` of the element captured at click time. After the DOM is rebuilt that element is detached (a zero rect) until the pointer moves. `refresh` asks the runner for the old numeric id, which answers `stale node` once the incarnation moves (`runner/src/agent.rs:463–467`). The tool keeps the previous report until it happens to ask.

Fix: in `applied`, if `inspecting.selected != null`, re-select that id, or clear the selection and hide the outline when the id is stale.

**10. The tests do not prove the gate they name.** `scripts/tamber-server.test.mjs:396–452`, `scripts/tamber-builds.test.mjs:387–401`.

The browser test moves the mouse, `page.mouse.click`s Swap pins, and checks that the dim preview color did not change, that `selected.node` equals a hit test, that rows have a source, that `tuple` is truthy, that refresh emits a second event, and that Escape removes the banner so a later click works. That hit test is `elementsFromPoint(…).find(e => e.closest('[data-view]'))`, which does not apply `nodeAt`’s `root.contains` or `data-exiting` filter (`host/web/dev.js:430–434`). The unit test calls `channel.clear('tool')` directly. It never goes through `absent()` (`scripts/tamber-server.mjs:208`). Neither test covers touch, Enter/Space on a focused button, wheel, an iframe, GPU keys, a link, a reload or a new generation, the presence timer, a body over 64 KiB, design digest or pin revision, or the preview’s `-exact-mode` against `tuple`. A green run still leaves findings 1–9 unexercised. I did not run the Playwright test (it needs Chrome and a wasm build, and it writes artifacts).

Fix: add a focused-button Enter case that expects no `press` and a selection of that button, a reload that expects `{kind:'inspect', on:true}` on the new hello, and a server test that expires presence and expects the off frame. Point the hit assertion at `nodeAt`’s filter.

## Low

**Hit-testing gaps on the same document.** `nodeAt` (`host/web/dev.js:430–434`) does not require `views.get(id) === el`, which is what a real tap’s geometry import requires (`host/web/glue.js:1432`). A stale `data-view` whose id was reused reports the runner’s current node while the tap’s listener no-ops. The “Open in native” bar (`host/web/dev.js:200–213`) sits outside `#exact-root` at `z-index` 2147483646; `swallow` does not exempt it, so a click there is eaten and `nodeAt` selects the app node underneath, which a real tap would not hit. The outline is the axis-aligned `getBoundingClientRect` (`host/web/dev.js:439`), larger than a rotated quad. Fix: walk to the ancestor whose id the views map confirms, ignore hits outside the app root that are host chrome, and accept the axis-aligned outline as approximate or outline the transformed quad.

**Ownership edges.** The last `on: true` overwrites `inspectors` (`scripts/tamber-builds.mjs:298`); the previous client’s departure does not turn it off. `on` omitted is off, because the server uses `request.on === true` (`scripts/tamber-server.mjs:494`), so `inspect {client}` turns every capable build off. `refresh: true` together with `on` ignores `on`, and any present client can refresh, including while the page has inspect off but `inspecting.selected` is set. `clear` deletes the inspector and then `frame()` no-ops when the stream is already gone (`scripts/tamber-builds.mjs:123`, `307–308`); a page that did not reload keeps capturing with no owner. The banner is still there, so this direction stays closed. `inspect` does not check write scope (`scripts/tamber-server.mjs:474`); `pin` has the same gap, and D4’s scope is the later remote work. The `selected` handler forwards `r.selected` and drops the POST’s `plan`, `design`, and `revision` (`scripts/tamber-builds.mjs:245`). That matches the event shape in `docs/tamber-protocol.md:500`. The join D5 describes will need those fields kept on the report before they are discarded.

**Stale comments.** `scripts/tamber-builds.mjs:19–23` and `host/web/dev.js:303–306` still say `inspect` is reserved and answered `unsupported`. The code handles it. `outline` and `hold` are still reserved, which matches this step.

Verdict: Inspect mode blocks same-document mouse clicks and selects from them, and it does not block keyboard, iframes, or hover; a keyboard click selects the node at (0, 0), a reload drops capture while the server still owns inspect, and the tests only prove the mouse path.
