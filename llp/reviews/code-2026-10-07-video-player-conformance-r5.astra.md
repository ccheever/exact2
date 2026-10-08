# Code review: Video Player conformance, round 5 (0033c999a), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `0033c999a`.
- **Method:** one brief (sha256 `787e992ca764bd96a906eaea4b5f5208f66f311b05ee2e87fa9c999f4791445d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** not taken; the branch is parked and the lead is asked to choose (five rounds, each finding another window in reporting the element's state after the fact).

---

DO NOT LAND

1. **MATERIAL — Replacement loads lose their own facts.** [media-glue.js:116](host/web/media-glue.js:116) merges subsequent reports into the original `dropped` record. After A reports, `emptied` advances the generation; B’s metadata/duration/canplay arriving before readiness join A’s record and are rejected together. Reproduced: fully loaded B delivers nothing, leaving duration zero permanently. **Fix:** replace invalidated pending records when the new load reports. Extend [the replacement test](host/web/tests/media-glue.test.mjs:150) to deliver B’s events before readiness.

2. **MATERIAL — Unresolved `app:/` loads still admit stale duration.** [glue.js:774](host/web/glue.js:774) queues `load` only after file lookup, so [the guard](host/web/media-glue.js:109) sees neither a pending command nor changed source. Reproduced with the actual host functions: replayed Reload receives the old duration while lookup remains unresolved. A first drop *after* requesting an unresolved replacement likewise stamps the new source onto the old element’s facts. **Fix:** record unresolved source/reload intent synchronously and associate recovery with the resource actually applied. Test both lookup windows.

3. **MATERIAL — Round 3’s error-ordering defect returns.** [Immediate delivery](host/web/media-glue.js:114) leaves the pending error intact; [recovery](host/web/media-glue.js:128) reads the older `el.error`. In the `inputReady`/`moduleReady` gap, an activation update delivers `invalid-value`, then recovery delivers `src-not-supported`, overwriting the newer failure. Reproduced through `syncMedia`. **Fix:** consume pending recovery when a newer report is delivered. Test readiness and promise resolution separately; [the helper](host/web/tests/media-glue.test.mjs:112) currently opens both together.

4. **MATERIAL — Recovered `ended` can undo a newer restart.** A recovered metadata handler queues `fastSeek(0)`; [the flush](host/web/media-glue.js:127) immediately sends `ended` before that command runs. Its guard checks only source/load changes. Reproduced: an ended handler advances tracks despite the preceding restart request. **Fix:** invalidate pending `ended` on accepted seek/restart intent, including writes from earlier recovered handlers. Add a queued-seek regression.

5. **MINOR — Early autoplay-policy errors remain permanently lost.** [`not-allowed`](host/web/media-glue.js:27) does not populate `el.error`, so [recovery](host/web/media-glue.js:128) emits nothing. The unchanged pause binding does not retry playback. This predates the diff but remains outside round 4’s claimed error fix. **Fix:** retain and invalidate the glue’s current failure separately from `MediaError`; test rejection before readiness.

All 10 unit tests pass. In-memory mutations deleting processed-load generation tracking or `ended` recovery also pass all 10. Browser conformance was unavailable without dependencies/build artifacts.

The failed-activation loop is fixed. No presenter skip changed; idle-tick dispositions remain intact. Changed source files satisfy the 1,500-line cap.
