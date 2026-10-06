# Round 1 (Grok 4.7, xhigh, on 04a2cac0e)

**READY WITH CHANGES.** The idle-clock fix is right for a transform or height release: the runner moves to the input time before the action, an `after` armed there counts from that time, and the release wire still says whether the event committed. Two holes remain.

## Findings

**[P2] A timer refusal marks a committed geometry observation as unadmitted — `host/web/src/transform_drag.rs:457`**

`deliver_at` returns the refusal and `committed` separately. The geometry path uses `committed` only to decide whether to lower current values, then folds the error into the batch:

```457:465:host/web/src/transform_drag.rs
            let (receipts, error, committed) = self.deliver_at(view, input.event(), input.now_ms);
            if !committed {
                Self::emit_lowered(&mut batch, self.springs.lower_current(self.runner.kernel()));
            }
            Ok(accepted(self.batch_from(
                batch,
                &receipts,
                error.as_deref(),
            )))
```

`accepted` (`host/web/src/transform_drag.rs:611`) is `{"accepted":true,"batch":…}` and carries no `committed` field. The page then treats any batch error as a failed geometry event:

```439:439:host/web/motion-glue.js
      if(transformLocal(b))b.admitted=reply.accepted===true&&!reply.batch?.error&&!!next&&next.dimensions.every(v=>v>0);
```

A timer due at the geometry sample that refuses, while the geometry action commits, sets `admitted` false. `flushGeometry` returns on unchanged facts (`host/web/motion-glue.js:421`), so admission stays false until the dimensions change, and a one-shot timer will not fire again. The release path is unaffected: it puts `committed` on the wire, and `host/web/motion-glue.js:724` cancels the pair only when `committed !== true`. Apple geometry keys admission off `reply.accepted` alone (`host/apple/Sources/ExactKit/TransformGeometry.swift:72`). Put `committed` on the geometry reply and admit from that, leaving the timer error on the batch.

**[P2] Two host-delivered events still run at the runner's last advance — `host/apple/src/resize.rs:26`, `host/linux/src/host.rs:937`**

These paths were already dispatching without `advance_timed`. They are the same stale-`now()` hole this commit closes for gesture release and geometry.

- Apple resize delivers with `self.runner.dispatch` and stamps the receipt at `self.now_ms`. `tick` (`host/apple/src/host.rs:1107`) seeks the engine and the host clock — its comment says nothing else moves — then `finish` runs `resize_rounds`. A `resize` handler's `now()` and any `after` it arms therefore use the runner's previous advance, which a long motion tick can leave seconds behind.
- Linux `dispatch_retained` runs `runner.dispatch_bound` and only then sets `self.now_ms`, stamping the receipt at the input time without moving the runner. Callers are a retained press (`host/linux/src/presenter/retained_action.rs:275`) and a retained swipe (`host/linux/src/holds.rs:45`).

Linux transform release and geometry, Linux height release, and web height release already go through `dispatch_at` (`host/linux/src/transform_binding.rs:259`, `host/linux/src/presenter/transform_geometry.rs:183`, `host/linux/src/height_binding.rs:185`, `host/web/src/height_drag.rs:177`). The terminal host advances the runner on every `tick` before key dispatch (`host/terminal/src/host.rs:425`), and the poll wait is bounded, so a key's `now()` is at most one poll behind. The Windows host does not dispatch events.

## Assessment

**Holds during the action.** Firing due timers before the release action keeps the paired-hold invariant at action time. `action_fired` and the host clock are set, and the view id is captured, before `deliver_at` (`host/apple/src/transform_drag.rs:251`, `host/web/src/transform_drag.rs:375`). Timer actions mutate the kernel only. Engine holds live in the motion engine; `reconcile_transform_drags` and height cancellation run later, per receipt, after `deliver_at` returns. `dispatch` targets the captured view and returns `UnknownView` if that view is gone, with `committed` false, so the event is not retargeted. While the action runs, both holds still own presentation.

**`committed` / `dispatched`.** `deliver_at` (`host/apple/src/transform_drag.rs:107`, `host/web/src/transform_drag.rs:231`) matches the comment. A timer refusal with a committed event keeps the timer error, returns `committed: true`, and includes both receipts, the event's last. An event refusal with successful timers reports the event error and `committed: false`. Both refusing keeps the timer error (`error.get_or_insert`) and returns `committed: false`. `dispatched` stays true once dispatch is reached. Release UI keys the hold end off `committed`: web at `motion-glue.js:724`, Apple at `TransformDragHold.swift:128`. A timer refusal with a committed release still releases the pair. Apple height throws `committed` away (`host/apple/src/height_drag.rs:236`); `HeightDragHold.finish` ends the hold from the pointer path (`HeightDragHold.swift:92`), so that batch error does not cancel the sheet. This differs from `Runner::dispatch_at` (`runner/src/runner/commit.rs:195`), which overwrites a timer error with the event error. The `deliver_at` comment states the first-refusal rule, and the release callers follow it.

**Web lowering when the event fails.** On `!committed`, geometry emits `lower_current` and then `batch_from` with the timer receipts and the error (`host/web/src/transform_drag.rs:458`). With no timer receipts that is the previous failure path. The success path's dirty-frame lowering is the event receipt. `lower_current` skips held properties.

**Tests.** `release_and_geometry_actions_run_at_the_inputs_time_after_an_idle_clock` (Apple `host/apple/src/transform_drag_tests.rs:912`, web `host/web/tests/it/transform_drag.rs:858`) and the height test (`host/apple/src/height_drag.rs:309`) prove the bug and the fix. Boot leaves the runner at 0, and begin/move do not advance it, so before this change `now()` in the geometry and release actions stays 0: `measuredAt == 4000` and `releasedAt == 5010` fail. `after(300, …)` armed at 0 would have fired by `advance(5100)`; the tests require `landed`/`closed` still 0 at 5100 and 5309 and 1 at 5310, which is the arming-at-5010 boundary. Times are integer milliseconds, exact in `f64`, with no wall clock. A timer already due, a dual refusal, web admission after a timer error, and hold survival when an earlier receipt reconciles are untested.

**Residuals, same shape as `dispatch_at`.** Receipts apply after every timer action and the release action have mutated the kernel, so an earlier receipt reconciles the final tree. If that tree fails `transform_active_valid`, the earlier receipt can retire the pair before the release receipt's motion sync. The photo case arms `after(300)` from the release and leaves the pair valid. Gesture code also seeks the engine to the input time before `deliver_at` (`update_transform_hold` / `update_hold` / geometry `synchronize_transform`), and commit seeks with `max(receipt_at, engine.now)`, so springs from an overdue timer are sampled at the gesture time. The runner clock inside those timer actions is still each timer's due time. The idle photo bug has no timer due, and its `now()` / `after` fix is the runner clock.

# Disposition (round 1)

1. Taken: geometry replies say `committed` (both hosts), and the web glue admits from it when present (`reply.committed ?? !reply.batch?.error`), so a timer's refusal in the batch no longer unadmits a committed observation.
2. Apple resize taken (advanced to the host's clock before its handlers). Linux `dispatch_retained` queued (QUEUE.md): its rule that a refused binding takes neither the sample nor an ordinary action needs its own care.
