# Code review: web clock settle waits for a glide, round 2, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `fa9691b7cf`.
- **Method:** one brief (sha256 `ea477371267f65efe72c0568331b17320ea4f9d391be868cfc8b2f219c643d62`), blind to the other reviewer; 2 rounds (agent tooling on the web hosts, theme2's 03:42 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

**1 BLOCKER, 1 NIT.** Round 1's two fixes hold: the 500 ms fast path is gone, and a missing frame cannot hang settle past the deadline. A later round of the same settle can still fail a resting scroller.

## BLOCKER

`host/web/input-glue.js:1104` — Once the shared 4 s deadline has already passed, `glidesSettled` returns false on a page that is still producing frames, and both hosts answer `{settled: false, reason: 'scroll'}` while nothing is scrolling.

`frame` arms `setTimeout(…, Math.max(0, deadline - now))` together with a `requestAnimationFrame`. The loop returns false as soon as either wait loses, and only then looks at `scrolls === before` (line 1105). The second wait is scheduled from the first frame's continuation, so its timer is a 0 ms task and its animation frame is the next frame. The timer task runs in the gap, the promise resolves false, and the late animation-frame callback cannot change that.

Both loops capture one `glideEnd` for the whole settle and call `glidesSettled(glideEnd)` at every round (`host/web-js/agent.js:462`, `host/web/glue.js:1232`). Round 0 is allowed to spend well past 4 s on in-flight work, up to the 20 s request bound. If it then goes around again — owed scroll events after rows are built, a new request, or a timer or animation still ahead — the next call is already past `glideEnd` and fails the settle as `scroll`. Native `awaitScrollRest` returns as soon as nothing is decelerating; its deadline only stops a scroll view that is still moving.

Keep the timer so a frame that never arrives still answers false, and keep answering false when a scroll event was seen and the deadline has passed. A call that starts past the deadline and sees no scroll event should answer true.

## NIT

`scripts/glide-fixture.mjs:81` — `toBeLessThan(8000)` does not fail on the pre-fix loop in this fixture. Frames are delivered, and that loop already stopped after a pair of frames once the deadline had passed, so the auto case still finishes in about 4 s without the per-frame timer. The `> 3500` and `reason: 'scroll'` checks do lock a scroller that keeps stepping. The quiet-spell comment at line 60 does not isolate the removed fast path either: the scroller is armed, then `clock settle` is a later round trip, so the first event has usually already fired before the sample. The two-frame rule is closed in the source.

## Round-1 fixes

- Two animation frames are scheduled one after the other, so a `scrollTop` write in the first frame's callbacks shows up at the next frame's scroll steps. A fling's own events are fired at those scroll steps, so settle cannot return while they continue.
- Each frame wait races a timer to the one settle-wide deadline. `clock data` and a jump do not wait, same as native. A follow-up `clock settle` is a new settle with its own bound.
- `watchGlides` runs from the agent import (`clock.agent` only) and from `agentInputReady`, and the guard makes a second call a no-op. App `createInputHandlers` does not start it. The glide listener does not stop owed-scroll delivery.
- `glue.js` is still 1499 lines. `y` and `scrollTop` are both rounded; a glide that keeps moving by hundreds of pixels still fails the later check. The reason text says scroll events go on.

Full notes: `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-9aa02748.md` and `/var/folders/mv/pmcqbrn10cs3bpj4mxnm6xvc0000gn/T/grok-admin/grok-review-summary-9aa02748.md`. Browser tests were not re-run.
