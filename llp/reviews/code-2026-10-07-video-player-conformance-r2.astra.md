# Code review: Video Player conformance, round 2 (51eba24da), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `51eba24da`.
- **Method:** one brief (sha256 `9097aae60bf6ea715b1b7e924e3abdecdab2fe3e150273f78889fa16f5024392`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The brief's round number reads 1 (a label slip); it is round 2, of the change reduced to the glue fix.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: a waiting report is checked against what the host has written, not only what the glue has applied: the source in `props`, a queued `load` command, and the load generation. A replayed handler that changes the source drops the reports behind it (tested). 2 taken: a waiting time (`timeupdate`, `seeking`, `seeked`) is also dropped by a seek: a changed bound `currentTime`, a queued command, or a `fastSeek` run (`state.seeks`); a duration is not (tested). 3 taken: the source change, `emptied`, the load command and removal are separate tests, and each new guard fails its test when removed.

---

DO NOT LAND

1. **MATERIAL — Source changes during replay do not invalidate remaining reports soon enough.** [glue.js:30](host/web/glue.js:30) defers `update()` to another microtask, but [media-glue.js:108](host/web/media-glue.js:108) checks the generation immediately. Reproduced: queued `loadedmetadata` switches to a missing source and clears duration; queued `durationchange\n10` then restores the old duration before the counter advances. **Fix:** invalidate synchronously when accepting a source change or load command, including changes caused by replayed handlers.

2. **MATERIAL — A queued time report can undo a newer seek.** [media-glue.js:108](host/web/media-glue.js:108) validates only the load generation; [currentTime updates](host/web/media-glue.js:56) do not invalidate it. Reproduced: queue `timeupdate\n4`, apply an activation seek to 20, then open readiness. A handler mirroring time into the bound position rewinds both state and element to 4. **Fix:** invalidate superseded position reports when accepting a seek, independently of duration/metadata reports. Test activation seeks and `fastSeek`.

3. **MINOR — The source-replacement assertion is masked by `emptied`.** [media-glue.test.mjs:125](host/web/tests/media-glue.test.mjs:125) changes source, then fires `emptied` before resolving readiness. All eight tests still pass with the source-change increment deleted in memory. **Fix:** test source replacement, `emptied`, and explicit load separately; assert the replacement source’s report survives.

Eight tests pass; both defects reproduced using the actual glue with stand-in elements. Browser conformance was not run. Harness-hold and idle-tick dispositions remain intact; no presenter skip changed. All changed source files meet the 1,500-line cap.
