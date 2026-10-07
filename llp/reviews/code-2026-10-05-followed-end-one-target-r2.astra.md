# Code review: a followed end's measured row is its motion's one target; a port at a device pixel is at its target, round 2 (07ec03707), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `07ec03707`.
- **Method:** one brief (sha256 `780488b2ed58b0902719262fa0797acfc9e4af26eed03eb8ae7319c14fb98552`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r3):** 1 taken: half a point now covers only a followed end the runner has already sent (`start::at_target`, `end_sent`); an end that moved, by any amount, is sent once, so a row list's re-measurement that lands before the anchor is taken still moves the port. `an_end_moved_by_less_than_half_a_point_is_still_followed` measures the last row 32.4 then 32.8 on both axes and follows a new row (it fails without the sent-end test). `list.js` mirrors it (`atTarget`, `endSent`); it has no unit harness, so conformance is its check. 2 taken: the shrink case drives a frame and asserts the port moves from 900 toward 800; `testADeferredStartFoldedOntoThePortRunsNoFrames` covers the deferred start.

---

LAND WITH CHANGES

1. **MATERIAL — Fractional horizontal remeasurements lose end-follow.** [mod.rs:732](runner/src/instance/collection/mod.rs:732) skips a 0.4-point correction, but horizontal remeasurements precede anchor capture ([mod.rs:1274](runner/src/instance/collection/mod.rs:1274)). With 100×32-point rows, a 320-point port at 2880, and the last row measured successively at 32.4 and 32.8, the second report sees a 0.8-point gap and stops following. A subsequent append receives no correction; the base targets 2912.8. Reproduced using both versions’ JS methods; [list.js:520](host/web-js/list.js:520) mirrors the runner ordering. **Fix:** preserve end-follow across horizontal remeasurement using its premeasurement state, while retaining horizontal row-anchor semantics. Add the two-report-plus-append regression to runner and JS tests.

2. **MINOR — The shrink regression does not assert the corrected origin.** [SmoothCollectionIOSTests.swift:202](host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:202) changes only the offset, asserts that `began` changed, then cancels. Restarting the clock while incorrectly retaining `from = 1000` still passes. The deferred-start folding branch also remains untested. **Fix:** shrink the content, drive a controlled frame, and assert motion starts from the clamped 900-point port; separately test folding before the deferred start runs.

The earlier idle-tick fixes remain intact. Whitespace and source-size checks pass. Rust/UIKit suites were not run in this read-only checkout.