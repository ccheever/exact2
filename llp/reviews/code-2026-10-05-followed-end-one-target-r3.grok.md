# Code review: a followed end's measured row is its motion's one target; a port at a device pixel is at its target, round 3 (00899a8f4), 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `00899a8f4`, plain output.
- **Method:** one brief (sha256 `f37606037bbbcc4551c13263e00ae22e333c44ec014ac9947e58e59c74aae568`), shared with astra. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition:** 1 taken as proposed (900.49 and 0.49, asserting the move).

---

LAND

1. **MINOR** — The fold assertions still pass if the offset is never written. `host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:220` and `:234`. The fold starts at 900 and retargets to 900.2; the deferred start goes from 0 to 0.2. Both expect the new value with accuracy 0.34, and `|900 − 900.2|` and `|0 − 0.2|` are 0.2. Dropping `scroll.contentOffset = target` in `OffsetDriver.retarget` (`CollectionIOS.swift:384`) or the `setContentOffset` in the deferred start (`:194`), while still calling `done` / `animationEnded`, leaves `folded`, the display link, and `animating` as the test wants. Round 2 asked for these asserts; the accuracy is wider than the move. Fix: retarget to 900.49 (still under the 0.5 fold) and assert `contentOffset.y > 900.2`; use 0.49 and `> 0.2` for the deferred start.

Round 1 and 2 dispositions otherwise hold. `at_target` (`start.rs:99`) widens to half a point only when the anchor follows the end and `corrected` is within 0.01 of `end_sent`; a row anchor stays at 0.01, and an end that moved by more than that is sent and `end_sent` is updated in `restore` (`mod.rs:735`). `list.js` matches (`atTarget`, `endSent`). `a_row_anchors_small_moves_still_correct` moves 0.4 twice; `an_end_moved_by_less_than_half_a_point_is_still_followed` sends 32.4 then 32.8 on both axes and then follows an append. A pre-frame retarget keeps `from` only while `contentOffset == from` (`CollectionIOS.swift:381`); a clamped port takes a fresh ease, and the shrink test pins `began` and a frame that stays in `[800, 900]`.

A half point here does not drop a real move. `restore_anchor` returns the end on every followed-end call, so a gap in `(0.01, 0.5]` cannot accumulate: the next end more than 0.01 from `end_sent` is sent again. Inserts above, measurements, and restore go through that path; `scrollIntoView` still settles at its own half point after `restore`. The fast path (`mod.rs:1437`) still bails on a remeasure, `at_end`, a target, a pending row, an outstanding correction, a preview, or a different window, so the idle-tick holes (fills, controls, rasters, regions on a skipped `Session.apply`) stay closed. Linux stores the correction as `f32`, whose ulp at a transcript offset is well under 0.5; the wasm web host uses this same runner. `mod.rs` is 1499 lines. `QUEUE.md` still records the host-resolved end.
