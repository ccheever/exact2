# Code review: Video Player conformance, round 2 (51eba24da), 2026-10-07 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, a detached worktree at `51eba24da`.
- **Method:** one brief (sha256 `9097aae60bf6ea715b1b7e924e3abdecdab2fe3e150273f78889fa16f5024392`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. The brief's round number reads 1 (a label slip); it is round 2, of the change reduced to the glue fix.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND.
- **Disposition:** Nothing to take. Its note that a src change with no `emptied` is not tested separately is taken with Astra 3.

---

LAND

No defects. Round 1 holds: the harness hold is gone (`host/web-js/conform.mjs:26-29`, `scripts/agent-launch.mjs:168-193` keeps `ratechange`), and a waiting report is tied to its load.

1. **Correctness.** `emit` snapshots the payload and `state.load`, and flushes in attachment order only if that load is unchanged and the element is not retired (`host/web/media-glue.js:104-108`). A later report is a later `then` on the same `moduleReady` promise. `inputReady` is set in `activateData` (`host/web/glue.js:1263-1264`) and `resolveModuleReady()` runs only after that returns (`host/web/glue.js:1473-1476`), on the success path, the reload path (`inputReady` stays true, so `later()` is null), and a built page (`host/web/document-glue.js:255-267` loads `glue.js` on first input; the same `activate` order). The glue `send` still drops a view id that was reused (`host/web/glue.js:30`). The first `update` bumps `load` when `applied` is `{}` (`host/web/media-glue.js:50`), before any `emit` (`:124-127`), so the opening report keeps that generation. The same string does not bump (`props[name] !== state.applied[name]`). A real src change also queues `emptied` before the new `durationchange` (HTML load algorithm), and `emptied` bumps again (`:116`); events of the new load run after that bump. `early` still swallows only the duplicate already queued (`:119`, `:126-127`).

2. **Regressions.** This does not widen a per-batch skip. Reports that used to be dropped are delivered once, before `exact.ready` (it awaits `moduleReady`, `host/web/glue.js:1408`). The idle-tick hole (carried-row `prepare` outside a batch) is iOS presenter code and is untouched.

3. **Tests.** `host/web/tests/media-glue.test.mjs:105-133` fails if an early report is dropped, reordered, duplicated, kept across `emptied`, or kept after `removeMedia`. 8 tests passed. A src change with no `emptied` is not separate; on a spec-following element that case has no prior duration (`NETWORK_EMPTY` never has metadata).

4. **Quality.** Same glue style. `glue.js` is 1491 lines, `media-glue.js` 257. No dead code. No `RULES.md` breach.
