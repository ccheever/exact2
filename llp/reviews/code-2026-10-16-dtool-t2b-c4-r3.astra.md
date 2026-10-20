# Code review: LLP 1122 T2b step 4 (inspect mode, wasm page), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `81cb38c91` (commit `85c72e54e` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `0314dcb7160cf684184ce91e370ca2ba4ea1fa13267677bf764fffae7f9e7031`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 high, 5 medium, 1 low).
- **Disposition:** taken in part, applied without re-review (round 3 was the last): a transparent cover over the whole page now takes the pointer while inspect mode is on, so no view sees hover, enter or leave, a press on a guest frame, or the start of a drag, and a press is read through it; focus, blur, change, input, compositionend and a dialog's cancel are taken too, so entering (which blurs) commits nothing; a guest frame that takes focus itself sends focus back to the close control; the banner's own events never pass on into the app (the close control is handled in the gate, Escape first), and the chrome is made again when a new tree or a modal opened later took it away, inside the topmost modal; a tab that was inspecting starts inspecting again on a reload, with the command it was on by, until the server's replay says otherwise, and an exit the server may not have heard is kept across the reload and answers the replay; a selection that finishes after an exit (or another command) is dropped, and a refresh does nothing when off; a refresh that finds two copies with the same path selects neither; a node with no modes reports none rather than the window's; the test now asserts no control takes focus while inspecting, and after the exit a focused button's Enter swaps (the positive control), which caught an exit pressed before the reload's replay being undone, fixed here. Not taken: a popover's light dismiss happens in the browser before any event a page can stop (its toggle reaches the app); a gesture already running when inspect turns on is not cancelled; the outline is the element's bounding box, not a transformed quad.

---

Static review of `81cb38c91`; no files changed and no builds or tests run. Input isolation still has blockers.

1. **High — Guest frames can regain keyboard input.** [host/web/dev.js:524](/tmp/rv-dt-t2b4-r3/host/web/dev.js:524)  
   The parent’s `focusin` listener cannot observe focus events inside a guest document. A guest that focuses an input after inspection starts can receive typing, Enter, and Escape; its messages still reach the runner. Chromium changes the focused frame through window focus/blur events, without dispatching `focusin` on the parent iframe element. [Chromium focus handling](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/page/focus_controller.cc).  
   **Fix:** establish a frame-aware focus barrier throughout inspection, including newly loaded guests, and return focus to inspector chrome when focus enters a guest.

2. **High — Active gestures and queued input work survive entry.** [host/web/dev.js:520](/tmp/rv-dt-t2b4-r3/host/web/dev.js:520)  
   Installing capture early fixes listener ordering, but entry does not cancel existing contacts or callbacks. Start a captured pan, enable inspection, then release: `pointerup` is swallowed, but the browser’s `lostpointercapture` is not. [input-glue.js:474](/tmp/rv-dt-t2b4-r3/host/web/input-glue.js:474) then cancels the pan and dispatches `panrelease`. Already queued pointer/pan callbacks and reorder autoscrolling also bypass the gate.  
   **Fix:** cancel active contacts and pending input work on entry without dispatching application actions, and prevent deferred delivery throughout inspection.

3. **High — Browser light-dismiss remains an application-input path.** [host/web/dev.js:429](/tmp/rv-dt-t2b4-r3/host/web/dev.js:429)  
   This round-2 finding remains open. With an app auto-popover open, clicking outside during inspection still dismisses it. Chromium performs dismissal before dispatching the click, and the resulting `toggle` reaches [glue.js:563](/tmp/rv-dt-t2b4-r3/host/web/glue.js:563). [Chromium click dispatch](https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/core/input/mouse_event_manager.cc).  
   **Fix:** preserve open presentation and disable user-driven dismissal during inspection; suppress the resulting application notifications during any necessary presentation changes.

4. **Medium — Inspector controls pass input into application ancestors.** [host/web/dev.js:503](/tmp/rv-dt-t2b4-r3/host/web/dev.js:503)  
   Every event targeting the banner bypasses capture. When the banner is mounted inside an app dialog, its pointer and keyboard events traverse application handlers. Clicking × can bubble into an ancestor’s `press` handler even after the banner removes itself. Escape on the focused close button also bypasses the inspector’s Escape handling.  
   **Fix:** handle inspector controls in the early capture gate without delivering their events through the app, and process Escape before the banner exemption.

5. **Medium — Entering inspection can commit an edited field.** [host/web/dev.js:535](/tmp/rv-dt-t2b4-r3/host/web/dev.js:535)  
   Entry calls `blur()` after setting `inspecting.on`, but `change`, `blur`, and `focusout` are not swallowed. Edit a field whose `change` saves data, then enable inspection: withdrawing focus commits that edit and runs its application handler. This follows the browser’s [focus-update rules](https://html.spec.whatwg.org/multipage/interaction.html#focus-update-steps).  
   **Fix:** suppress application delivery of focus and control-completion events while withdrawing focus, while preserving required host cleanup.

6. **Medium — A hot reload can remove the banner while inspection remains active.** [host/web/dev.js:557](/tmp/rv-dt-t2b4-r3/host/web/dev.js:557)  
   With a modal open, the banner and outline become children of the application tree. A generation replacement calls `root.replaceChildren()` in [glue.js:1353](/tmp/rv-dt-t2b4-r3/host/web/glue.js:1353), removing both. `inspecting.on` remains true, and another `on:true` returns without repairing them. Touch users lose the required local exit. Later modal presentation can likewise make existing chrome inaccessible.  
   **Fix:** maintain inspector chrome across tree replacement and modal-stack changes, and recreate or rehost missing chrome even when inspection is already on.

7. **Medium — Full reloads temporarily enable application input.** [host/web/dev.js:430](/tmp/rv-dt-t2b4-r3/host/web/dev.js:430)  
   The build ID survives reload, but inspection starts as `false`. Registration waits for application readiness, and only the server’s subsequent replay reinstates inspection. A tap during that network interval reaches the app despite the server retaining inspect ownership. The reload test waits for the banner and misses this interval.  
   **Fix:** preserve inspection state across reloads and keep input gated until registration reconciles ownership; retain pending local exits until acknowledged.

8. **Medium — Refresh can select the wrong repeated instance.** [host/web/dev.js:495](/tmp/rv-dt-t2b4-r3/host/web/dev.js:495)  
   Refresh accepts the first matching site and instance path. The runner supports duplicate row keys with distinct duplicate ordinals, but reported instance paths contain only the raw key. Select the second duplicate, apply an asset-only generation with the same plan, and refresh: the first duplicate matches instead.  
   **Fix:** include the duplicate ordinal in identity, or require exactly one matching node and refuse ambiguous refreshes.

9. **Low — The focused-keyboard test still exercises an unfocused page.** [scripts/tamber-server.test.mjs:417](/tmp/rv-dt-t2b4-r3/scripts/tamber-server.test.mjs:417)  
   Calling `focus()` after inspection starts immediately triggers the new listener’s `blur()`. Enter and Space therefore do not test activation of a focused button; removing keyboard capture could leave these assertions green.  
   **Fix:** assert the actual focus state, test focus prevention explicitly, and separately verify keyboard capture with an observable handler and an off-mode positive control.

**Verdict: Request changes — input isolation and reload behavior remain incomplete.**
