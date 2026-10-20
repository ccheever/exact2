# Code review: LLP 1121 stage 9 G2, the reader's track taken back, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the reclaim commit.
- **Method:** one brief (sha256 `db92b2ba01e4842571bcbf407588efbefae4049fbd058c868613778688abb58f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (the same test gap).
- **Disposition:** the test fixed in round 1's commit.

---

Nothing blocking. The reclaim path matches the engine’s rules, and the Apple claim holds. One test gap is worth fixing.

1. **should-fix** — The new test never reclaims mid-track, and it never checks the sample `reclaim_played` presents. `host/linux/src/lower_tests.rs:220`

   `dispatch_at(..., 0)` leaves the host clock at 0. `commit` then seeks with `at_ms.min(now_ms)` (`host/linux/src/host.rs:1321`), so the `SetLooks` at 250 ms is applied and reclaimed at t=0, where the linear curve is still at its start (`1.0`). `tick(250)` runs `advance` before `present`, and `advance` overwrites `presented` from the curve still sitting in `running`. `shown ≈ 0.8` is therefore just “the original curve kept running,” which is also what a reclaim that stored the wrong sample would show after that tick. Hover is never turned on, so `presented()` does not apply the look’s `0.5` either. Removing `reclaim_played` still fails the test (`is_played` stays true); a wrong sample at the reclaim instant does not.

   Fix: `tick(250)` first (so `now_ms` is 250), commit the look at 250, and assert opacity `≈ 0.8` on that commit’s `presented()` before any further tick. Tick to 500 ms and assert `≈ 0.6`, so a restart from the sample with a fresh 1 s duration cannot pass.

What I checked and did not find a defect in:

- **Curve, clock, pending, hold.** `play_transition_full` leaves the `Running` curve on the slot, rewrites `start` to `start_at`, and clears `pending` (`motion/src/engine/played.rs:79`). `reclaim_played` does not build a new curve: it samples that same curve at `sample_time()`, writes the sample into `presented` when it is not done, and puts the key back in `running`. A spring stays a spring (frames exist only on the host’s `PlayedTransition`, which the retain drops). Linux/Android never turn on `start_on_frame`, and a played curve is not pending anyway. `quiescent` is `running` / `animating` / clock-pending, so the reinserted key wakes the next frame through `Host::motion`. A hold cannot own the slot while it is in `engine.played`: `begin_hold` removes `played` and clears the curve before `play_transitions` runs, and `reclaim_played` returns immediately when the key is already gone.
- **Re-handover.** The same `play_transitions` pass then walks `running` and calls `playable`, which returns false when `paints_over` is true (`host/linux/src/lower.rs:565`). The track is not handed back.
- **Transform.** `paints_over` is only opacity, background, and color (`host/linux/src/press.rs:120`). Look rows cannot name translate, scale, or rotate. A transform track staying with the reader is consistent with that.
- **Borrow.** The loop borrows the host’s `played` map and calls `engine.reclaim_played`, which only mutates the engine. The host map is updated afterward by `retain`. No aliasing and no iterator invalidation.
- **Cost.** The extra work is one node resolve and a mask/look scan per track the reader already owns, on a `present` that already walks that map. After a reclaim the key leaves the map, so it is not scanned again.
- **Apple.** The only `play_transition` in `host/apple` is `Property::D` (`svg_lower.rs:489`). Opacity and color transitions are not given to Core Animation. Keyframe eligibility, including `paints_over`, runs for every created or touched node on the commit (`svg_lower.rs:374` from `host.rs:1334`). `SetLooks` touches the node, and `SvgState::element` queues a spec rebuild that emits `[]` once `node_sampled` is set.
- **Reader.** Dropping the host entry and bumping `lowered_changed` is the same signal a hold already uses. `layer_tracks` then omits the property. If that was the layer’s only track, `lowered_mask` becomes 0, the row is dirty, the new recording has no layer, and `LAYER_FREE` follows. If a transform track remains, op 31 replaces the track list and `Layers.sample` sets alpha from the layer base (`1`, opacity no longer lowered) while the recording carries the engine sample via `push_opacity`. `tracks()` replaces the whole array; it does not keep the old opacity track.
