# Code review: the Apple agent's bounded scroll track, round 3 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `1464cd9eee`.
- **Method:** one brief (sha256 `dc901ebeb9b9ad2d679ad5ec8efc2a27fe4d24c8db34333038a617b4de3e3545`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Round-2 items are fixed and covered by tests. The track still follows the #1141 limits: no new scheduler, one 4096-row ring, pages under 4000 bytes with the newline, and the same `enabled && !virtual` gate as capture.

- **Cursors per session label.** `trackCursors` in `scripts/agent-inspect.mjs` is a map per driver, keyed by `s.session`, cleared on a successful start, on stop, and when a read sees the track end. `a scroll track start by view id sends no tree, and cursors are kept per app session` switches A to B and back: B's first read has no cursor, and A's next read continues at `"5"`.
- **Numeric start sends no `tree`.** A `/^\d+$/` id is passed straight through; a testId still resolves with one `tree`, and the contract says to do that before the glide. The same test expects exactly one request, `{track: "start", node: 42}`. The host still skips `drainAll` for any track request in `Agent.handle`, and `testCarrierTrackRequestsDoNotDrainTheOwner` holds the owner across start, read, and stop.
- **Mac axis.** `FrameHealth.trackAxis` uses `scrollsX`/`scrollsY` when only one axis scrolls. `testAMacSidewaysScrollerIsReadAlongX` builds a sideways scroller whose content is taller than the clip (`scrollsX && !scrollsY`) and checks the recorded offset against `origin.x`.
- **Origin at start.** Start stores `trackAxis`'s offset in `last`, so the first bounds change that leaves it writes nothing. `testTheAgentReadsAMacScrollerMovedByAPlainWheel` calls `clipScrolled()` before any wheel and expects zero rows.
- **Mac phase at the handler.** `ChainingScrollView.scrollWheel` sets `t`/`d` only for `.began`/`.changed`, then clears it with `defer`. `testTheMacPhaseIsTheWheelEventsThatMovedIt` drives began, changed, momentum begin, momentum continue, momentum end, and a later layout move, and expects `t, t, d, d, -, -`, with `trackPhase` left at `"-"`. CG scroll phases 1 and 2 and momentum phases 1, 2, and 3 map to `NSEvent.Phase` began, changed, and ended, so those events are the ones the handler classifies.

VERDICT: APPROVE

No findings.
