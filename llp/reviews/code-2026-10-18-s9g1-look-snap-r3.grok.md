# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the change and rounds 1-2.
- **Method:** one brief (sha256 `d8837278ed2e15e6cbc72eea86aae7c07e88e570e261b1fb9c6b7979e732d5de`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: rt.js stubbed writeIn before a GPU surface; the dedupe cleared too early; a media query per event).
- **Disposition:** fixed in round 3's commit.

---

The round-2 fixes hold in Chrome: the snap attribute is set before any style read, `matches()` does not start a transition, and the three `focusSeen` cases stay consistent with the events the browser actually fires. One regression is blocking.

1. **Blocking** — `host/web-js/rt.js:1149` (called from `rt.js:620`, before the adopt return). `input()` installs `exact.writeIn ??= () => 0` synchronously while the tree is still being built. `gs()` then does `x.writeIn ??= …` (`rt.js:767`) and only the first assignment wins. A looked node is created in `h()` before a later canvas’s `gs()` runs, and a canvas that itself has a look calls `input()` at the start of its own `h()`, before `gs()`. The GPU writer never attaches. `exact_surface_record` still closes over an empty string, so published surface records are dropped. `canvas2d.js` assigns `writeIn` with `=` later and survives; GPU does not. This is new for pages that have a look and no shortcut or press, which used to not call `input()` at all. Putting the call before the adopt return is required (an adopted node returns from `h()` immediately), but the stub must not be taken on that call. Move `writeIn ??= () => 0` into the import `then`, immediately before `createInputHandlers`, so it runs only if `gs()` has not already installed the real writer. The route-match stub can stay.

2. **Should-fix** — `host/web/input-glue.js:478`. `edge` is cleared with `queueMicrotask`. In Chrome the compat `mouseover`/`mouseout` is dispatched in the same task only after that microtask checkpoint, so the mouse handler always sees `edge === null`. A mouse-only edge (Firefox scroll under a still pointer) is not skipped; the dedupe never matches, and `stillThrough` forces style twice on every pointer boundary. Clear the flag in a macrotask (`setTimeout(…, 0)`), or ignore a mouse event only when it shares the pointer event’s `timeStamp` and target. A later mouse event must still run.

3. **Nit** — `host/web/input-glue.js:480`. `matchMedia("(hover: hover)")` builds a new list on every pointer and mouse boundary. Hoist one `MediaQueryList` next to `lookStates` and read `.matches`. It stays live across hover-capability changes.

`focusSeen` is not stale in the cases that matter. Removing a focused node (or a parent that contains it) fires `focusout` while the node is still inside `exact-root`, so the map is cleared before a re-add. `focus()` on an element that is already focused does not fire focus events and does not clear `:focus-visible`, so a true entry still matches the screen. A `focusout` that never had a `focusin` leaves the map at false and does not snap; that is the documented “edge before the input piece has loaded” gap, not a later miss. Click-then-key is already `:focus-visible` inside Chrome’s `keydown` listener, and `stillThrough` then shows the focus color with no transition left running. The `requestAnimationFrame` retry is the other order (flag set in the key’s default action), and one frame is enough because that default action finishes before the frame callback. `inPage` walks `getRootNode().host` and stops: a host’s root is the outer tree, not its own shadow, and a `Document` has no `host`.

The looks test does check the snap (final color, no transition still running, delay-only, click-then-key, pointer focus and hover on a focus-only look leave the attribute unset, a base change still starts one transition). It never loads `rt.js`, so it cannot see finding 1.
