# Code review: scrub frames' status deferred and transitions off through a drag, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** `6cb6f9402`.
- **Method:** one brief (sha256 `e131df4bc7d2d761af4d0bdef2d220ca4302ccdcf4c239889058d8fdecf7c415`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (1 blocking: the commit hook still reports each frame; a generation's timer; the per-frame flush).
- **Disposition:** fixed in `9cc1fa38f` (see its message).

---

I'll review the HEAD commit read-only, starting with the diff and the design-status path it changes.The diff is small. Next I’ll trace every status caller, how scrub frames are marked, and whether a deferred report can land late or stale.The wasm page still posts a build-channel status on every scrub frame. The new 150 ms timer does not replace that report.

1. **Blocking** — `host/web/dev.js:285` (the send at `host/web/dev.js:282`, the hook at `host/web/dev.js:405`). A scrub frame that applies still calls `x.send`, and `applyBatch` always ends in `exact.committed`. That schedules `facts()` on the next turn: `getComputedStyle`, three `matchMedia` checks, then a post whenever the body differs from `channel.told`. `design` is part of that body (`host/web/dev.js:397`), and the digest was just updated, so the channel still gets one status per scrub frame. The 150 ms timer only adds a second post after the drag. This is the wasm page the commit profiled, so the build channel does not learn the design once the gesture rests.
   **Fix:** Arm `statusLater` before `send`. In `committed`, if `statusLater` is set, clear `due` and return without calling `facts()`. The deferred `status()` already sends scheme, contrast, mode, font size, and design. A non-scrub frame still calls `status()` immediately and still clears the timer.

2. **Nit** — `host/web/dev.js:236`. A new generation deletes `designDigest` and calls `status()` without `clearTimeout(statusLater)`. The late callback does not put the scrub digest back: the cache is gone, and `status()` re-reads via `designNow()`. It does run `facts()` again and post a second status after the generation's own report.
   **Fix:** `clearTimeout(statusLater)` and set `statusLater = null` at the start of `applied`, before the immediate `status()`. A full navigation already drops the timer with the document. The JS page reloads that way (`location.replace`), so `liveStatus` does not need the same clear.

3. **Nit** — `host/web/dev.js:301`, and the JS handler at `host/web-js/dev.mjs:695` via `still()` in `host/web-js/modes.js:241`. Every scrub frame still forces layout with `offsetWidth` so transitions stay off for that frame. That cost is independent of the status report, and the protocol wants it. The status deferral does not remove it.

Nothing else in the deferral path fails the checks you listed.

Scrub ends are scrub-marked, and their status still arrives. Cancel, idle, disconnect, mismatch, a competing write, and a refused Keep all call `restore(mark(...))` in `scripts/tamber-scrub.mjs`. Keep's release build carries the same kind of mark (`releaseMark` in `host/web/dev.mjs`, `releasedMark` in `host/web-js/dev.mjs`). The page treats those like drag frames, so the status waits 150 ms rather than going out immediately. If the digest changes, the new timer is the one that fires. If Keep does not change the digest (the preview is already the written design), the timer from the last scrub frame is left in place and reports that same digest. A selection during the lag fails the server's "latest status" check and is retried with `inspect {refresh, status: true}`, which calls `status()` at once.

A deferred status cannot report a stale design or overwrite a newer immediate one. The timer calls `status()` with no saved digest; both pages read `designDigest` when the callback runs. Any later frame that changes the digest hits `clearTimeout` before it either reschedules or calls `status()`. `clearTimeout` also drops a callback that is queued but not started. On the wasm page, `facts()` only awaits already-resolved promises, so the read and `report()` finish before the next stream event. `report` posts in order. The server drops a status only when its pin revision is older; an equal revision does not rewind a design that was posted later.

No temporal-dead-zone issue. `statusLater` and `liveStatus` are initialized before the closures that assign them. `status` is declared later in the same synchronous script, and both `apply` and `live` run only from later events (the generation callback, the dev stream), after that initialization.
