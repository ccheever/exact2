# Code review: LLP 1122 T2b step 4 (inspect mode, wasm page), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `85c72e54e`.
- **Method:** one brief (sha256 `523a2b1b151db020e825b746b2e6cc95bc83be9ec951a42751dc31c73cc5b638`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 9 medium, 2 low).
- **Disposition:** taken but 8 (astra) and Grok's low items in part: inspect mode now takes every input itself (keys too, Escape and the banner aside; pointer over/out/enter/leave and moves; wheel; Safari gestures; drags), blurs the focused element on entry, keeps guest frames from taking pointer input (a press there is their view's), and the GPU surface's own window key listener stands down while it is on; a press released where it began (mouse, pen or finger) selects, never a click (so a keyboard click selects nothing, and touch selects); the hit is the topmost element, nothing when the host's chrome is on top, and the view the runner knows by that element; the node's own modes are reported (the runner's `layout` answer now says `modes`, the tuple the instance tree keeps per node), so a node in a pinned subtree reports its pin's; a refresh after another generation finds the selection by its site and instance path or says it is gone; the report is measured in bytes as the server does, and refused, named, when the identity alone does not fit; a build that registers again is told inspect mode as the server holds it (a reload keeps it; an owner gone while it was away turns it off); the page leaving inspect itself reports a selection of none, which drops its owner; the polled queue keeps only the latest inspect and one refresh; ownership goes with an evicted or replaced build; inspect needs write scope and names `on` or `refresh`; the e2e test now covers Enter and Space on a button focused before, a finger tap, a pinned preview's modes, a reload and the local exit. In part: the banner and outline go to the top layer (a popover) when shown; a modal opened after them is still above. Not tested: guest frames, the GPU keys, a body over 64 KiB, a generation change (no fixture for them here); ownership loss through the presence timer is the channel's `clear`, tested directly.

---

Static review only; no files changed and no builds or tests run.

1. **High — App input still reaches keyboard, movement, and gesture handlers.** [host/web/dev.js:423](/tmp/rv-dt-t2b4/host/web/dev.js:423)  
   Only Escape is intercepted on the keyboard; `pointermove` merely draws an outline, and pointer enter/leave and Safari gesture events remain unblocked. Enter in a focused `submit` field still dispatches its action; GPU controls receive Enter/Space and pointer movement; Safari pinch handlers still transform content. Keyboard scrolling and editing also remain active. **Fix:** gate these host input paths while inspecting, preserve the inspector’s controls, and cancel existing contacts and pending gesture work on entry.

2. **High — Guest frames bypass the input capture entirely.** [host/web/dev.js:485](/tmp/rv-dt-t2b4/host/web/dev.js:485)  
   Events inside an iframe do not propagate through the parent window. For example, tapping Caltrain’s deck still executes its guest `pointerdown` handler, posts `deck-tapped`, and reaches the parent’s Contract message handler. No selection is reported. **Fix:** shield guest frames from input while inspecting, move focus out of them, and select the iframe’s owning view when its shield is tapped.

3. **Medium — Touch taps can never produce a selection.** [host/web/dev.js:460](/tmp/rv-dt-t2b4/host/web/dev.js:460)  
   Selection happens only on `click`, but the same handler prevents `touchstart`. Touch browsers suppress the resulting compatibility mouse events, including click, after that cancellation. A finger tap is therefore swallowed without selecting anything. [Touch Events §7](https://www.w3.org/TR/touch-events/#mouse-events) describes this behavior. **Fix:** recognize selection from the captured pointer/touch release, with movement/cancellation checks and deduplication.

4. **Medium — Refresh can silently select a different node after a rebuild.** [host/web/dev.js:498](/tmp/rv-dt-t2b4/host/web/dev.js:498)  
   The stored selection is only a numeric view ID. A new runner reuses IDs, while inspect state survives an in-place generation change. Select a button, insert an earlier node in the Contract, then refresh: the old ID can now identify another node, which is reported under the new digests. **Fix:** retain generation and node identity together; invalidate on replacement, or remap through a uniquely matching site/instance path. Guard pending reports against generation and stream changes too.

5. **Medium — Disconnect cleanup can be lost permanently.** [scripts/tamber-builds.mjs:308](/tmp/rv-dt-t2b4/scripts/tamber-builds.mjs:308)  
   If the build’s stream disconnects before its inspecting client disappears, `clear()` deletes ownership but cannot deliver `on:false`. Reconnecting the existing page preserves its local inspect state, and registration replays only pins. Inspection remains active without an owner. **Fix:** reconcile inspect on/off during registration, or clear local inspect state on stream loss. Cover cleanup while disconnected and server restart.

6. **Medium — Report trimming does not enforce the 64 KiB limit.** [host/web/dev.js:452](/tmp/rv-dt-t2b4/host/web/dev.js:452)  
   `JSON.stringify(...).length` counts UTF-16 code units, whereas the server limits transmitted bytes. A selection containing 30,000 CJK characters in an instance key can pass this check yet exceed 64 KiB. Furthermore, removing every row still leaves an oversized `testId` or `instance` untouched. The server destroys the request, and the page silently loses the report. **Fix:** bound the complete UTF-8 envelope and return a bounded refusal when essential identity alone cannot fit.

7. **Medium — Hit testing selects through opaque host overlays.** [host/web/dev.js:432](/tmp/rv-dt-t2b4/host/web/dev.js:432)  
   Searching for the first element *inside the root* skips an actual hit outside it. Clicking the dev error overlay or “Open in native…” bar can select an obscured app node underneath, although a real click would hit that overlay. **Fix:** determine the actual foremost target first and select nothing when it belongs to host chrome; exclude only the inspector’s own transparent decoration.

8. **Medium — An app modal makes the local close control inaccessible.** [host/web/dev.js:484](/tmp/rv-dt-t2b4/host/web/dev.js:484)  
   The banner and outline are ordinary body children. An open `showModal()` dialog occupies the browser’s top layer and makes those siblings inert; maximum `z-index` does not fix this. A touch user inspecting an open dialog cannot use the banner’s close button, and outlines appear beneath the modal. **Fix:** provide inspector chrome that remains visible and interactive above app modal presentation.

9. **Medium — Inspect commands bypass the polled queue’s bounds.** [scripts/tamber-builds.mjs:297](/tmp/rv-dt-t2b4/scripts/tamber-builds.mjs:297)  
   `openPolled()` coalesces pins and collapses live-table frames, but never removes inspect frames. Sending more than 64 inspect/refresh requests to a registered polled inspector leaves the queue above its advertised cap; repeated unacknowledged polls can keep it alive while it grows. **Fix:** coalesce inspect state and refresh requests and enforce a hard bound after every enqueue.

10. **Medium — The selection reports the wrong mode tuple inside pinned subtrees.** [host/web/dev.js:451](/tmp/rv-dt-t2b4/host/web/dev.js:451)  
    `modes()` returns the window’s tuple. Selecting the fixture’s Dim preview while the window uses Black therefore reports Black alongside rows resolved in Dim. This violates D5 and can identify the wrong branch for an edit. The runner already retains `NodeInst.tuple`; token rows alone are not a complete substitute. **Fix:** expose the selected node’s effective tuple in the synchronous node explanation and use it here.

11. **Medium — Inspect ignores the server’s supplied authorization scope.** [scripts/tamber-server.mjs:474](/tmp/rv-dt-t2b4/scripts/tamber-server.mjs:474)  
    `dispatch()` receives `scopeOf(req)` but passes no scope to `inspect()`. With the existing hook configured for `propose`, a present client can still disable application input. D4/D13 require write scope for control commands. **Fix:** reject inspect requests without write scope, as scrubbing already does.

12. **Low — Inspector ownership survives build eviction.** [scripts/tamber-builds.mjs:106](/tmp/rv-dt-t2b4/scripts/tamber-builds.mjs:106)  
    Registration’s build replacement and cap-eviction paths remove `builds`, `pins`, and `sent`, but leave `inspectors`. A continuously present client inspecting successive build IDs accumulates entries beyond the 256-build cap. **Fix:** remove inspector ownership wherever the corresponding build is forgotten.

13. **Low — The tests do not establish the claimed input and selection guarantees.** [scripts/tamber-server.test.mjs:428](/tmp/rv-dt-t2b4/scripts/tamber-server.test.mjs:428), [scripts/tamber-builds.test.mjs:399](/tmp/rv-dt-t2b4/scripts/tamber-builds.test.mjs:399)  
    The hit-test assertion largely repeats the implementation, tuple validation checks only truthiness, and “client goes” directly calls `clear()` without exercising presence loss. These tests pass despite the failures above. **Fix:** use independently known targets and exact provenance values; add touch, focused keyboard/GPU controls, guest frames, overlays, generation changes, disconnected cleanup, and byte-limit cases.

**Verdict: Request changes; input isolation, selection identity, and lifecycle cleanup are not yet reliable.**
