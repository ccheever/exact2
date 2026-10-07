# Code review: Video Player conformance, round 3 (7cf34da0a), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `7cf34da0a`.
- **Method:** one brief (sha256 `09373ee0a8bfe2a2a6b639e678ac38502ba468f0b4a435dbc25908f435fb2610`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** Moot by redesign (80a6ea336): the third round of replay holes ended replaying. A waiting report is dropped, and only `loadedmetadata` and `durationchange` are reported once the host can, read from the element then, so no stale time, play, pause or error is sent and no echo or seek can be misjudged. 4 moot: the command guards are gone.

---

DO NOT LAND

1. **MATERIAL — Time echoes discard newer reports.** [media-glue.js:111](host/web/media-glue.js:111) treats any changed `props.currentTime` as a seek. Reproduced: queue times 4 and 8; a handler mirrors each into `currentTime`. Replaying 4 invalidates 8, then the element rewinds to 4. A following `seeked` is likewise discarded, potentially leaving a seeking indicator stuck. **Fix:** distinguish report echoes from independent seeks; test multiple queued times and completion.

2. **MATERIAL — Bound seeks that return to the original value admit stale reports.** The fingerprint compares the current binding, while `state.seeks` advances only for [fastSeek](host/web/media-glue.js:88), excluding [bound seeks](host/web/media-glue.js:60). Reproduced: hold `timeupdate\n4` with binding 0, apply seeks to 20 then 0, release readiness. The stale 4 survives and a mirroring handler rewinds playback. **Fix:** track accepted bound seeks monotonically, including writes preceding `update()`, while preserving echoes; add this regression.

3. **MATERIAL — Activation can deliver newer errors before older queued errors.** [syncMedia](host/web/glue.js:30) stops deferring when [inputReady becomes true](host/web/glue.js:1264), before the [await continuation resolves moduleReady](host/web/glue.js:1473). Reproduced: an activation update setting volume to −1 sends `invalid-value` immediately; the previously queued `src-not-supported` then overwrites it. **Fix:** preserve queue order through activation and backlog drainage; test this microtask gap.

4. **MINOR — Pending-command guards remain untested.** [The tests](host/web/tests/media-glue.test.mjs:161) execute commands immediately. Removing either queued-load detection or the command-length fingerprint still passes all 11 tests. **Fix:** add replay handlers that enqueue `load` and `fastSeek` before the glue applies them.

Reproductions used the actual glue and `syncMedia` with stand-in elements. All 11 unit tests pass; browser verification was blocked by missing Playwright/read-only restrictions. Earlier source/removal fixes hold, no presenter skip changed, and source files meet the line cap.
