# Code review: LLP 1122 T2b step 4 (inspect mode, wasm page), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `2472f2de5` (commit `85c72e54e` and its round-1 fixes).
- **Method:** one brief (sha256 `b86cd901b6713b69b027d0c8e24b3e836a8e46f82f1193d89a959a64b5e4efb6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 5 medium, 3 low).
- **Disposition:** taken but 1, 5, 6 (astra) in part: the input gate is on the window from the page's start (doing nothing until inspect is on), so it runs before any listener the host adds later (a gesture's, a GPU surface's), and the GPU glue change is gone (no inspect in any shipped file); scroll, scrollend and every drag event are taken too, and focus moving into a field or guest frame meanwhile is dropped; a guest frame is hit as itself (the shield lifted for the reading only); a refresh after another generation finds the node again only under the same plan digest, else says it is gone; the banner and outline go inside an open modal dialog when one is, so its close control is not inert; inspect commands are numbered (`seq`): a page's own exit names the one it left and drops its owner only if no newer command came, and a replay of a command the page left (a new stream before the server heard the exit) is the exit again; the outline hides on a new generation; tests: the button is focused after inspect is on, each key checked on its own; the refresh's baseline is taken before it is asked; numbered commands, a stale exit and the replay on registering are in the channel test. In part: the browser's light dismiss of an open popover and a scrollbar drag happen before any event a page can stop (scroll events are kept from the app, the scrolling itself is not); a modal opened after the banner is above it.

---

Static review only; no files changed and no builds or tests run.

1. **High — Browser light-dismiss bypasses capture.** [host/web/dev.js:494](/tmp/rv-dt-t2b4-r2/host/web/dev.js:494)  
   `preventDefault()` and `stopImmediatePropagation()` cannot prevent browser actions performed before dispatch. Open an app’s auto popover, enable inspection, then click outside: the browser still dismisses it, and its `toggle` handler can reach the runner. Chromium performs light-dismiss before dispatching the click. **Fix:** explicitly preserve app popover/dialog presentation during inspection and prevent dismissal-triggered app events; add an open-popover outside-click case. [Chromium dispatch implementation](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/input/mouse_event_manager.cc)

2. **High — Existing gestures can run before the inspector’s listeners.** [host/web/dev.js:539](/tmp/rv-dt-t2b4-r2/host/web/dev.js:539)  
   Capture listeners are installed only on entry. A pan started on a nested button already has window capture listeners registered by [input-glue.js:422](/tmp/rv-dt-t2b4-r2/host/web/input-glue.js:422). Enable inspection while holding that press: its next move reaches the earlier listener, schedules a pan callback, and dispatches into the app. Entry also leaves queued gesture work and reorder autoscrolling alive. **Fix:** install the dev input gate before app listeners, cancel active contacts and pending gesture work on entry, and guard deferred dispatch.

3. **Medium — Refresh still selects unrelated nodes after structural edits.** [host/web/dev.js:480](/tmp/rv-dt-t2b4-r2/host/web/dev.js:480)  
   `site` and the region IDs in `instance` are indices within a particular plan, not stable identities across plans. Select a node outside any region, insert an earlier sibling, and refresh after the generation changes: another node can now have the saved site and empty instance path. **Fix:** retain `planDigest` and invalidate selection when it changes, unless the server supplies a verified correspondence through the source maps.

4. **Medium — The iframe shield makes iframe selection impossible.** [host/web/dev.js:534](/tmp/rv-dt-t2b4-r2/host/web/dev.js:534)  
   Setting `pointer-events: none` removes the iframe from `elementsFromPoint()`. A press over Caltrain’s guest deck therefore selects its parent or an underlying view, rather than the iframe’s owning view as the comment promises. **Fix:** temporarily disable only the inspector’s shielding stylesheet during the synchronous hit-test, restoring it immediately afterward, or use a shield that preserves the underlying iframe identity.

5. **Medium — Native scrollbar input remains active.** [host/web/dev.js:426](/tmp/rv-dt-t2b4-r2/host/web/dev.js:426)  
   Capturing wheel and pointer events does not cover every scrolling path. Chromium handles touch/pen scrollbar dragging before DOM pointer dispatch. Such a drag can still scroll the app, whose `scroll` handler dispatches unconditionally at [glue.js:554](/tmp/rv-dt-t2b4-r2/host/web/glue.js:554). **Fix:** shield native scroll affordances and gate user-originated scroll/scrollend delivery while inspecting; cover an actual scrollbar drag. [Chromium scrollbar handling](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/core/input/pointer_event_manager.cc)

6. **Medium — The banner remains inert beside an open modal.** [host/web/dev.js:536](/tmp/rv-dt-t2b4-r2/host/web/dev.js:536)  
   Promoting a body sibling to a popover changes its painting layer, not modal inertness. With a dialog already open, the banner’s close button remains outside that dialog’s subtree and cannot receive input. A touch-only user loses the local exit required by D4. **Fix:** mount inspector controls within the active modal and maintain that relationship when the modal stack changes. [HTML modal inertness rules](https://html.spec.whatwg.org/multipage/interaction.html#modal-dialogs-and-inert-subtrees)

7. **Medium — A delayed local-exit report erases a newer owner.** [scripts/tamber-builds.mjs:254](/tmp/rv-dt-t2b4-r2/scripts/tamber-builds.mjs:254)  
   Every `selected: null` unconditionally deletes current ownership. Suppose Escape queues that report behind a slow POST, then another client enables inspection before it arrives. The delayed report deletes the new owner while the page remains inspecting; that client’s later departure cannot clear it. **Fix:** version inspect commands independently and accept a local exit only for the inspect revision it acknowledges.

8. **Low — The keyboard test does not exercise a focused app control.** [scripts/tamber-server.test.mjs:415](/tmp/rv-dt-t2b4-r2/scripts/tamber-server.test.mjs:415)  
   The test focuses Swap before enabling inspection, which immediately blurs it. Enter and Space consequently target an unfocused page. Furthermore, checking a toggled value only after both keys can conceal two activations. **Fix:** focus the control after inspection starts and assert after each key, preferably using an action counter.

9. **Low — The refresh assertion has a response-order race.** [scripts/tamber-server.test.mjs:454](/tmp/rv-dt-t2b4-r2/scripts/tamber-server.test.mjs:454)  
   The event-count baseline is captured after awaiting the refresh request. If the selection event reaches the polling loop first, the baseline already includes it and the test waits for a nonexistent additional event. **Fix:** capture the baseline before sending refresh, or correlate the expected response explicitly.

10. **Low — Inspect-specific behavior ships in production GPU glue.** [host/web/gpu-glue.js:621](/tmp/rv-dt-t2b4-r2/host/web/gpu-glue.js:621)  
    The new `exact.inspecting` branch is included in production GPU host files. The exclusion test checks only the string `exact-inspect`, so it passes despite this dependency, contrary to D13’s compiled-out requirement. **Fix:** establish the earlier capture gate entirely in dev code, or remove the inspect branch during production generation and verify the resulting artifact.

**Verdict: Request changes; input isolation, selection identity, and ownership cleanup remain incomplete.**
