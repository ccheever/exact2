# Code review: Video Player conformance, round 3 (7cf34da0a), 2026-10-07 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, a detached worktree at `7cf34da0a`.
- **Method:** one brief (sha256 `09373ee0a8bfe2a2a6b639e678ac38502ba468f0b4a435dbc25908f435fb2610`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** Moot by redesign (80a6ea336): the third round of replay holes ended replaying. A waiting report is dropped, and only `loadedmetadata` and `durationchange` are reported once the host can, read from the element then, so no stale time, play, pause or error is sent and no echo or seek can be misjudged. 2 moot: the command guards are gone.

---

DO NOT LAND

1. **MATERIAL** — A deferred `play`, `playing`, or `pause` overwrites a newer `paused` value. `since` (`host/web/media-glue.js:108-112`) records the load generation, `props.src`, and a queued `load` for every report, and the seek tuple only for `timeupdate`, `seeking`, and `seeked` (`:10`). `play`, `playing`, and `pause` are not in that set. Video Player writes `paused` from those events (`apps/video-player/app.contract:44-47`) and binds `paused`, `play`, and `pause` on the same element (`:117`).

   On a built page the press is replayed from `inputOpened` (`host/web/document-glue.js:179-184`) after `setInputReady(true)` (`host/web/glue.js:1263-1264`). `resolveModuleReady()` runs in the `finally` after `await activateData()` (`:1473-1476`), so that replay microtask runs before the waiting reports flush. Autoplay’s `play`/`playing`, fired after `installMedia` and held by `() => inputReady ? null : moduleReady` (`glue.js:30`), is then delivered and sets `paused` false. `syncPlayback` calls `el.play()` (`media-glue.js:20-27`). Before this change the glue’s `send` dropped that report while `inputReady` was false, and the replayed Pause stayed.

   Concrete input: built Video Player, wasm still loading, the clip fires `play` after media-glue attaches, the user hits Pause. Activation applies the press, then the deferred `play` starts the clip again.

   **Fix:** put `props.paused` in the snapshot for `play`, `playing`, and `pause`, and drop the waiting report when it differs. Test it like the bound-seek case: fire `play`, set `props.paused` to `'true'` without relying on a later event, open readiness, assert the `play` was not sent; an unchanged `paused` still delivers.

2. **MINOR** — The pre-update command guards are not what the tests fail on. `command()` (`host/web/tests/media-glue.test.mjs:28`) pushes the command and calls `installMedia`, which runs `update`/`run` immediately, so `state.load` and `state.seeks` have already moved before `open()`. Removing `commands.some(([c]) => c === 'load')` (`media-glue.js:110`) or `commands.length` (`:111`) still passes the load and `fastSeek` tests. Those reads exist for the window the comment at `:106-107` describes: the host has queued the command, and `update` is still a `mediaModule.then` (`glue.js:30`).

   **Fix:** queue the `load` or `fastSeek` and do not call `installMedia` before `open()`. Assert the waiting duration or time is dropped, and that a duration survives a `fastSeek`.

Round 1’s harness hold is gone (no `HOLD_MEDIA`; clocks stay frozen in `scripts/agent-launch.mjs`). Round 2 holds: a waiting report is dropped when `props.src` or a queued `load` changes before `update` (`media-glue.test.mjs:150-158`), and a waiting time is dropped by a bound `currentTime` or a processed `fastSeek` while a waiting duration is kept (`:161-171`). `inputReady` is set before `moduleReady` resolves on the success path, including a built page (`glue.js:1484` calls the same `activate`). A reload never clears `inputReady`, so `later()` stays null. The first `update` bumps `load` only when `src` differs from `applied` (`media-glue.js:51`); the same string does not, and the opening emit is after that bump (`:133-136`). Retirement and a reused view id are still dropped (`:117` checks `state.retired`; `glue.js:30` checks `views.get(id) === el`). This does not skip a batch or touch the idle-tick presenter path. `glue.js` is 1491 lines, `media-glue.js` 266. Eleven unit tests passed.
