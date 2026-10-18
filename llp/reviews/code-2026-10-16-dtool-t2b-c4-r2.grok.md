# Code review: LLP 1122 T2b step 4 (inspect mode, wasm page), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `2472f2de5` (commit `85c72e54e` and its round-1 fixes).
- **Method:** one brief (sha256 `b86cd901b6713b69b027d0c8e24b3e836a8e46f82f1193d89a959a64b5e4efb6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 2 medium, 1 low).
- **Disposition:** taken but 1, 5, 6 (astra) in part: the input gate is on the window from the page's start (doing nothing until inspect is on), so it runs before any listener the host adds later (a gesture's, a GPU surface's), and the GPU glue change is gone (no inspect in any shipped file); scroll, scrollend and every drag event are taken too, and focus moving into a field or guest frame meanwhile is dropped; a guest frame is hit as itself (the shield lifted for the reading only); a refresh after another generation finds the node again only under the same plan digest, else says it is gone; the banner and outline go inside an open modal dialog when one is, so its close control is not inert; inspect commands are numbered (`seq`): a page's own exit names the one it left and drops its owner only if no newer command came, and a replay of a command the page left (a new stream before the server heard the exit) is the exit again; the outline hides on a new generation; tests: the button is focused after inspect is on, each key checked on its own; the refresh's baseline is taken before it is asked; numbered commands, a stale exit and the replay on registering are in the channel test. In part: the browser's light dismiss of an open popover and a scrollbar drag happen before any event a page can stop (scroll events are kept from the app, the scrolling itself is not); a modal opened after the banner is above it.

---

Two correctness bugs are still open: a press on a guest frame selects the node behind it, and a local exit does not survive a new dev stream.

## High

**A press on a guest frame selects the ancestor, and keys inside that frame never meet the capture.** `host/web/dev.js:532`

`setInspect` injects `iframe, frame, embed, object { pointer-events: none !important }`. A `WebView` is that iframe (`host/web/src/element.rs:539`) and it is the `[data-view]` element. `elementsFromPoint` skips `pointer-events: none`, so `nodeAt` (`host/web/dev.js:433`) returns the ancestor under the iframe. The comment on the shield says the press is that view's; the hit test returns the parent. Hover outlines the parent too.

Keys are a second hole on the same path. `swallow` listens on the parent window (`host/web/dev.js:426`). A key whose target is inside the guest document never reaches that window. Entry blurs once (`host/web/dev.js:517`) and nothing blurs again: there is no `focusin` capture. A guest that focuses an editable after that (autofocus when its document loads, a blur handler that refocuses, the runner focusing the webview) takes the typing.

Scenario: inspect a screen with a document or deck `WebView`, click the middle of it. The report is the parent column. Then the guest focuses its field and characters land in the guest.

Fix: drop the shield. Capture pointers with a viewport overlay under the banner. On `pointerup`, ignore the overlay and hit-test so the iframe element is included. For the whole session, blur any focus that enters the app root, including the iframe.

**A new `/__dev` stream turns inspect back on after Escape.** `host/web/dev.js:337`

Every EventSource connection, including the browser's reconnect, mints a new nonce (`host/web/dev.mjs:1309`) and `close()` drops the stream without clearing `inspectors` (`scripts/tamber-builds.mjs:118`). Escape and the banner call `setInspect(false, true)`, which removes the capture immediately and queues `{kind:'selected', selected:null}` (`host/web/dev.js:514`). `report` posts on the nonce it captured, drops the body if `channel.nonce` has changed, and swallows a 403. The server deletes the owner only when that POST arrives (`scripts/tamber-builds.mjs:254`). The next register then writes `{kind:'inspect', on:true}` because `inspectors` still has the build (`scripts/tamber-builds.mjs:222`), and `setInspect(true)` arms the capture again.

Scenario: inspect is on, the dev connection drops, the user presses Escape (the banner goes, and a click in that window reaches the app), the EventSource reconnects, the banner returns, and taps are taken again. The same drop happens if the new hello arrives before the queued null report is posted.

Fix: remember a local exit. Re-post `selected: null` on the new nonce, and ignore a replayed `on: true` until that post is accepted. Folding the local-off into register, so the server clears the owner before it writes the replay, is the same fix.

## Medium

**`scroll` and `dragover` still reach the app.** `host/web/dev.js:426`

Wheel, touchmove, and keydown are cancelled, so a new scroll gesture in the parent document does not start. `scroll` is not one of the captured events, and it is what the app hears: a view with a scroll handler dispatches kind 13 on every `scroll` (`host/web/glue.js:552`). An inertial scroll already running when inspect turns on keeps delivering those events. A scrollbar-thumb drag is the same hole when the browser starts it after the cancelled `pointerdown`.

`dragstart` and `drop` are captured. `dragover` is not, and a node with a drop handler forwards `dragover` to the app (`host/web/glue.js:572`). Dragging a file across that node during inspect runs the handler.

Fix: capture `scroll` and `scrollend` and stop them before the view's listener; pin scroll offsets while inspect is on. Add `drag`, `dragenter`, `dragover`, `dragleave`, and `dragend` next to `dragstart` and `drop`.

**The new tests do not cover the paths they are the evidence for.** `scripts/tamber-server.test.mjs:414`

The wasm test focuses Swap, then turns inspect on (which blurs it), then presses Enter and Space (`scripts/tamber-server.test.mjs:442`). That shows those keys did not activate an unfocused button. It does not show a control that is focused during inspect, or a key delivered inside a guest document. The hit it compares (`scripts/tamber-server.test.mjs:430`) is `elementsFromPoint` plus the nearest `[data-view]`, without `root`, `[data-exiting]`, or `views.get(id) === el`, and the fixture has no iframe, so the ancestor-behind-a-WebView case passes vacuously. Reload only waits for `#exact-inspect`. Nothing covers a `scroll` or `dragover` dispatch, a report over 64 KiB, a generation remap, or Escape while the dev stream is down. `scripts/tamber-builds.test.mjs:399` calls `channel.clear('tool')` directly; the presence timer that calls `clear` is untested.

Fix: add a guest iframe and assert the reported node is that view; focus a button after the banner is up and press Enter; drop the dev socket, press Escape, reconnect, and assert the banner stays gone and a click reaches the app.

## Low

**The outline keeps the last rectangle across a generation.** `host/web/dev.js:233`

`applied` replays a held pin and sends status. It does not move or hide the outline. The outline is a fixed box at the last measured rect (`host/web/dev.js:444`). A hot reload clears the views and leaves that box painted on the old coordinates until the next `pointermove`. Refresh still remaps a stored selection by site and instance when the client asks.

Fix: hide the outline in `applied`, and send the selection again (or clear it) for the new generation.

Verdict: request changes — a guest-frame press selects the node behind the frame, and Escape does not stick across a new dev stream.
