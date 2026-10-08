# Code review: Video Player conformance, round 4 (0b03f1ca9), 2026-10-07 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `0b03f1ca9`.
- **Method:** one brief (sha256 `82dfb055271a77d35a14552365960fc5fde4754401578e196c5bb072d69b15ee`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. Round 4 was run past the three-round limit with the lead's go-ahead, on the redesign that replays nothing.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: the catch-up sends directly and gives up if the host is still not ready (tested). 2 taken: the load generation, the source written and a queued `load` are recorded at the first drop and checked before each report (tested, including a handler that asks for a load and an `app:/` source). 3 taken: `emptied` bumps the load, so A's facts are not reported for B (tested). The early `error`, `canplay` and `ended` note: taken with Grok 3.

---

DO NOT LAND

1. **MATERIAL — Failed activation causes an infinite microtask loop.** [glue.js:1474](host/web/glue.js:1474) resolves `moduleReady` even when activation fails and `inputReady` remains false. [media-glue.js:111](host/web/media-glue.js:111) then calls `emit`, which schedules another callback on that same resolved promise indefinitely. Reproduced with loaded media and failed readiness. **Fix:** stop retrying when activation settles unsuccessfully; test promise resolution without input readiness.

2. **MATERIAL — Source/load changes can still admit stale duration.** The [source guard](host/web/media-glue.js:113) runs only before dispatching metadata. Reproduced: its handler clears duration and queues `load`; the callback immediately sends `durationchange\n10`, leaving app duration 10 after the element resets to NaN. Commands execute later through [syncMedia](host/web/glue.js:30). An unresolved `app:/` replacement also leaves the old element loaded even after `state.applied` accepts the new props ([glue.js:10](host/web/glue.js:10)). **Fix:** invalidate pending recovery synchronously on source/load intent and recheck between dispatches. Test queued commands and unresolved replacements. Round 2’s disposition does not fully hold.

3. **MATERIAL — Pending facts survive `emptied` and duplicate the replacement’s events.** [media-glue.js:125](host/web/media-glue.js:125) clears `early`, but retains `dropped`. Reproduced: A’s reports arrive before readiness; A empties; B acquires metadata; readiness samples B; B’s queued events subsequently deliver the same metadata and duration again. [HTML permits this state-before-event ordering](https://html.spec.whatwg.org/multipage/media.html#ready-states). **Fix:** invalidate pending facts across load changes and test readiness between B’s state update and event delivery.

Nine unit tests pass; these defects reproduced using the actual glue with stand-in elements. The [readiness helper](host/web/tests/media-glue.test.mjs:108) always attaches at `readyState=0` and opens readiness atomically, missing these cases. Browser verification was blocked by missing `playwright-core`.

Early `error`, `canplay`, and `ended` still disappear at [line 110](host/web/media-glue.js:110), potentially leaving failure, readiness, or completion state permanently wrong. These losses predate this diff.

Seek/play replay fixes hold; no presenter skip changed, and idle-tick dispositions remain intact. Changed source files are within the 1,500-line cap.
