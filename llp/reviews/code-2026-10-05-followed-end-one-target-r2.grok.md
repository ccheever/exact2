# Code review: a followed end's measured row is its motion's one target; a port at a device pixel is at its target, round 2 (07ec03707), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `07ec03707`, plain output.
- **Method:** one brief (sha256 `780488b2ed58b0902719262fa0797acfc9e4af26eed03eb8ae7319c14fb98552`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition (r3):** 1 taken: the pre-frame retarget asserts the port has not moved, and the fold asserts the offset it set.

---

LAND

Round 1's dispositions hold. A half point applies only while the anchor follows the end, in both `restore` and the feedback fast path, in the runner and in `list.js`. A row anchor stays at 0.01, and `a_row_anchors_small_moves_still_correct` moves the port 0.4 pt twice. A retarget before the first frame keeps its start only while `contentOffset == from`; a port that moved takes a fresh ease. The driver test is unconditional, the half-point correction asserts the offset moved, and the opening list settles then follows smoothly. The `OffsetDriver` type comment matches §6.8.

No new visible result for a row anchor, an insert above, a measurement, a restore, `scrollIntoView`, a horizontal list, or a Linux or web report. `restore_anchor` returns the end on every followed-end call, so a gap in `(0.01, 0.5]` cannot accumulate: the next change past half a point still corrects, and `capture_anchor` already uses that same half point to decide `follows_end`. Restored positions and `scrollIntoView` keep their own rules (`anchor_at` is never a followed end; `settle_target` is already half a point). The idle-tick holes stay closed: this does not widen `Session.apply`, and `travel_within` still bails on a measurement, `at_end`, a target, a pending row, an outstanding correction, or a window that differs.

1. **MINOR** — The pre-frame retarget test does not pin the offset. `host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:186` and `:211`. After the retarget to 1690 the test checks `to`, `began`, and `drawn`, not `contentOffset`. An implementation that jumps the port to 1690 and leaves `began` alone still passes. The fold asserts `folded == true` and that the link was dropped, not that the offset became 900.2. Dropping the assignment and still calling `done` passes. Fix: after the 1690 retarget assert `contentOffset.y == 1000`, and after the fold assert `contentOffset.y == 900.2`.
