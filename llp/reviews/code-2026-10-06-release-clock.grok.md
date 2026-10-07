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

# Round 2 (on 789a66c18)

**READY WITH CHANGES.** One defect: overdue timers fired by this change do not get their motion at the due time.

## Findings

[P2] Overdue timers fired for a gesture start their transitions at the gesture time — `host/apple/src/transform_drag.rs:339`

`advance_for_input` does move the runner, so a timer action's `now()` is its due time. The motion engine is already at the input time before those receipts are applied, and it will not rewind. `Engine::observe` starts a transition at `self.now` (`motion/src/engine.rs:436`), and both hosts then sample with `max(receipt time, engine.now())` (`host/apple/src/host.rs:1264`, `host/web/src/motion.rs:539`).

That seek happens first on every path this change arms:

- Apple geometry calls `engine.advance(input.now_ms / 1000)` and only then `advance_for_input` (`host/apple/src/transform_drag.rs:339`).
- Apple and web release call `update_transform_hold`, which seeks, before `advance_for_input` (`host/apple/src/transform_drag.rs:250`, `host/web/src/transform_drag.rs:377`). `update_transform_hold` seeks in `motion/src/engine/hold.rs:178`.
- Web geometry advances the runner first, then `synchronize_transform` seeks with an empty receipt list (`host/web/src/transform_drag.rs:465`) before `batch_from` applies the timer receipts.
- Apple height release seeks in `update_hold` (`host/apple/src/height_drag.rs:203`) before `advance_for_input` (`host/apple/src/height_drag.rs:236`).
- Apple resize advances the runner at `host/apple/src/resize.rs:27` (as of `789a66c18`), but `tick` has already seeked the engine to the host time (`host/apple/src/host.rs:1110`) and `finish` runs the rounds after that.

An idle app is the case this change is for. A timer due at 1.0 s that sets a 200 ms transition, with a fling at 5.0 s, used to be caught up by `Host::advance` / `dispatch_at` when the engine was still behind: the curve started at 1.0 s and the later seek sampled it forward. Here the curve starts at 5.0 s. The hold sample and the post-timer authoring can still be applied at the gesture time; the timer receipts have to be committed while the engine is on its old clock, and only then should the hold or the geometry seek run.

## What holds

The runner half matches `Runner::dispatch_at`. `deliver_after` keeps the timer commits, skips the event when the kernel binding changed, and reports that with `committed: false`. A refused timer rolls back, so `still = early.is_empty() || binding` does not run the event against a rolled-back change, and a successful timer always has a receipt so the binding is rechecked. The event then runs at the stopped clock. `committed: false` cancels the transform hold on both hosts (`TransformDragHold.swift:128`, `motion-glue.js:724`). A height binding change is cancelled inside the commit by `cancel_invalid_height_drag`, so dropping `committed` on that reply does not release the hold as a fling. Geometry bumps the sequence before adopting authoring; a binding or sequence mismatch retires the pair from `reconcile_transform_drags` when the timer receipts are committed. The web glue admits geometry from `committed`, and falls back to `!batch.error` when the field is absent.

The new tests fail on the old control flow: without the advance, `now()` stays at boot or at the press, the 300 ms `after` is due immediately, and the unbind timer never runs before the release. The times are integer milliseconds and the clock is the host's, so they are deterministic. I did not execute them. They do not cover a timer transition's start time, a resize round, a timer refusal followed by the event at the stopped clock, or a height binding changing under the release.

Linux `dispatch_retained` (`host/linux/src/host.rs:937`) and terminal keys (`host/terminal/src/term.rs:654`) are the same stale `now()` and are described accurately in `QUEUE.md`. Web height release and the web ABI already go through `dispatch_at`. Collection feedback still stamps the host clock and does not advance timers; that comment is older than this change and is a different choice.

# Disposition (round 2)

Queued (QUEUE.md): an overdue timer at a gesture's event starts its motion at the event's time. Only a timer the host's own wake missed is overdue at an input, so the shift is normally under a frame; the fix is to commit the timers' receipts before the hold or geometry seeks the engine. The resize round's advance was reverted on Astra's r2 finding and the path queued. A timer refusal followed by the event is now tested. Not re-reviewed.
