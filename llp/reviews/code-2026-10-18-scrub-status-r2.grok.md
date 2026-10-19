# Code review: scrub frames' status deferred and transitions off through a drag, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `6cb6f9402`, `9cc1fa38f`.
- **Method:** one brief (sha256 `6a8425d76f02523979ee1c54c5c91cbd385797b36a1501add1597efa71b405d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nits: the JS status's timer, a dropped frame, a throwing apply).
- **Disposition:** fixed in the next commit (see its message).

---

1. **Nit** — `host/web-js/dev.mjs:697`  
   The wasm page's `status()` cancels the deferred timer before it reads facts (`host/web/dev.js:407`). The JS page's `status()` does not. A pin, a scheme or contrast change, hello, or inspect refresh can report immediately while `liveStatus` is still armed. When that timer fires it calls `status()` again and posts a second time. The body is read then, not captured when the timer was armed, so the design is not an old one and the server does not rewind `build.design` (a later post has the same or a newer revision). It is still an extra `build-status` after the immediate one, which is the case the wasm clear was added for.  
   **Fix:** at the start of the JS `status()`, `clearTimeout(liveStatus); liveStatus = null`.

2. **Nit** — `host/web/dev.js:285` (drop is `runner/src/runner/live.rs:110`)  
   A scrub frame the runner drops (an older rev of the same run) still returns `live: "applied"`. This handler treats every applied scrub frame as a new rest: it restarts the 150 ms status timer and calls `unmoving`, which keeps `data-x-scrub` up and `x.send`s the empty batch. A snapshot that arrives just as the drag ends pushes the channel's design report out by another 150 ms and leaves transitions off that long. The report itself is current (`designDigest` is unchanged; `facts()` reads it when the timer fires). The JS page does not do this: an older frame returns without changing the digest, so `live` leaves the existing timer alone (`host/web-js/modes.js:219`, `host/web-js/dev.mjs:695`).  
   **Fix:** arm the timer and call `unmoving` only when this frame actually changed the design (digest or rows), not when `live` is `"applied"` merely because an older frame was ignored.

3. **Nit** — `host/web/dev.js:305` and `host/web-js/modes.js:243`  
   The attribute is set, then `apply()` / `x.send()` runs, and only then is the removal timer armed. The old `try/finally` always cleared `data-x-scrub`. If `x.send` throws, the timer is never scheduled and `[data-x-scrub] * { transition: none }` stays for the rest of the page.  
   **Fix:** arm the 150 ms removal in a `finally`, including when `apply()` throws.

Checked, not findings:

- **Ends still report.** Cancel, idle, mismatch, competing write, disconnect, and a refused Keep all `restore(mark(id))` (`scripts/tamber-scrub.mjs:102`, `:230`). A successful Keep does not restore; `released(mark(id))` puts that same mark on the build that lands (`host/web/dev.mjs:580`, `host/web-js/dev.mjs:477`). The protocol's "ends included" mark is intact. Those frames take the deferred path, not the immediate one. On the wasm page every applied scrub frame arms the timer, even when the digest did not change, so the end still posts. On the JS page a timer is armed only when the digest changes; if the end changes nothing, the timer from the last change is left to fire, or the design was already reported. `inspect {refresh, status: true}` calls `status()` at once (`host/web/dev.js:701`), which on wasm also cancels the deferred timer. For about 150 ms after the end, `build.design` can still be the previous status (`scripts/tamber-inspect.mjs` `current` / `designMoved`); a selection in that window is refused and asked again. That is the deferral, not a dropped report.

- **No stale overwrite.** The timer does not close over the design. Wasm `status()` clears `statusLater` synchronously before `await facts()`, and `design` is the last field read, in the same turn as `report()` (`host/web/dev.js:401`). A call that finishes later read the design later, and posts are queued in that order. JS `facts()` is synchronous and reads `designDigest` at call time. An immediate non-scrub frame goes through `live`, which `clearTimeout`s before it decides (`host/web-js/dev.mjs:695`).

- **Reloads and generations.** A wasm generation with no held pin calls `status()`, which clears `statusLater` (`host/web/dev.js:236`). A held pin clears it when `pin()` reaches `status()`. `stillUntil` is not cleared; transitions come back on the timer already armed. `location.reload` drops both timers with the document. The JS reload does not clear `liveStatus` before the checkpoint await (`host/web-js/dev.mjs:702`). If the timer fires, it posts the design that dying page is still showing; the new document registers on a new nonce and replaces the build record. It does not stick the scrub design on the new page.

- **TDZ.** `statusLater` is initialized before `apply` can run. `status` is only read when a frame arrives, after `const status`. `stillUntil` / `StillUntil` are initialized before any call; the functions close over them and do not read them during evaluation. `liveStatus` is initialized in the same `let` as `liveQ`, before `live` runs. `slot.ready()` only starts `provide()`, whose first await is the generation fetch (`host/web/navigation.js:743`).

- **No second per-frame status.** After the first scrub frame, `setAttribute("data-x-scrub", "")` is a no-op, and the forced layout moved to the one removal timer. The JS `moved` observer (`host/web-js/dev.mjs:700`) is the analogue of `committed` and does not look at `liveStatus`, but a colour scrub does not mutate what it watches: token writes are CSSOM `setProperty`, and `data-x-scrub` is not in the attribute filter. The first frame's `append` of `#x-scrub-still` can post one status; later frames do not. `committed` is not entered while `statusLater` is set. A callback already queued before the drag can still post once (`host/web/dev.js:417` does not re-check the flag). That is one report, not one per frame. `repaintPress` in `setTokens` still calls `getComputedStyle` per frame while a press is held; that call was already there and is outside this diff.
