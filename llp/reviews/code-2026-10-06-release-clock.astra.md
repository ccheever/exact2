# Round 1 (gpt-6-astra, xhigh, read-only, on 04a2cac0e)

**Verdict: NOT READY.** The idle-clock fix works in the straightforward case, but I found three timer-interleaving problems.

1. **P2 — A refusing timer leaves the event running at a stale clock.**  
   [Apple transform_drag.rs:113](/Users/admin/.tuft/projects/exact2-bsky/host/apple/src/transform_drag.rs:113), [web transform_drag.rs:237](/Users/admin/.tuft/projects/exact2-bsky/host/web/src/transform_drag.rs:237).  
   `advance_timed` stops at the refusing timer’s deadline. The helper then dispatches without reaching the input time. For example, a non-poisoning refusal at 4500 followed by release at 5010 makes `now()` return 4500 and arms `after(300)` for 4800. Stamping the receipt 5010 does not fix that. This reproduces the early-close bug under refusal. `Runner::dispatch_at` shares this existing defect; copying its sequence does not satisfy the helper’s timing promise.

2. **P2 — Timer commits can invalidate the gesture binding after its only validation.**  
   [Apple transform_drag.rs:269](/Users/admin/.tuft/projects/exact2-bsky/host/apple/src/transform_drag.rs:269), [web transform_drag.rs:393](/Users/admin/.tuft/projects/exact2-bsky/host/web/src/transform_drag.rs:393), [Apple height_drag.rs:236](/Users/admin/.tuft/projects/exact2-bsky/host/apple/src/height_drag.rs:236).  
   A due timer can change `transformDragFor` to `"absent"` or hide the target while leaving the handle alive. The helper still dispatches its release action and reports `committed:true`; binding reconciliation happens afterward. The same gap affects geometry and Apple height release. Revalidate the original binding after advancing, retaining and committing the timer receipts even when the gesture has become invalid.

3. **P2 — Geometry cancels holds before applying the timers that logically precede it.**  
   [Apple transform_drag.rs:345](/Users/admin/.tuft/projects/exact2-bsky/host/apple/src/transform_drag.rs:345), [web transform_drag.rs:454](/Users/admin/.tuft/projects/exact2-bsky/host/web/src/transform_drag.rs:454).  
   Consider a held transform, a due timer changing only `transition` to `"none"`, and then changed geometry. Cancellation starts a spring using the old declaration before `deliver_at` runs that timer. Later synchronization does not stop it: `Engine::set_transitions` preserves running transitions, and `observe` preserves a curve whose target is unchanged. The gesture therefore animates when it should snap. Synchronize preceding timer authoring while held, before geometry cancellation. Web’s failure fallback at line 459 also lowers before synchronizing those receipts, potentially emitting an obsolete animation before its correction.

The requested host sweep found these **pre-existing P2 follow-ups**:

- [Apple resize.rs:26](/Users/admin/.tuft/projects/exact2-bsky/host/apple/src/resize.rs:26): resize callbacks dispatch directly. Height-hold layout and motion ticks can reach this with the host clock ahead of the runner.
- [Linux host.rs:937](/Users/admin/.tuft/projects/exact2-bsky/host/linux/src/host.rs:937): retained press/swipe delivery calls `dispatch_bound` without advancing; assigning `self.now_ms` afterward cannot fix the action’s clock.
- [Terminal host.rs:439](/Users/admin/.tuft/projects/exact2-bsky/host/terminal/src/host.rs:439): direct dispatch follows a poll of up to 250 ms; the clock was advanced **before** that wait ([term.rs:622](/Users/admin/.tuft/projects/exact2-bsky/host/terminal/src/term.rs:622)). Input can arm timers early by that interval.

Windows advances before ordinary input; web height release and Linux’s ordinary drag releases already use `dispatch_at`. The JS target refreshes time inside its commit.

The ownership/result handling is otherwise sound: buffering receipts keeps both engine holds owned during release dispatch. Destroying the handle produces `UnknownView`; view IDs are never reused, so it cannot redirect to a replacement view. `committed` correctly distinguishes timer-refusal/event-success from event failure, and receipts remain timer-first/event-last. If both refuse, the helper reports the timer error, unlike `Runner::dispatch_at`, which reports the event error.

The added tests are deterministic and would fail on the parent by inspection. They cover idle timing and the 300 ms deadline, but no timer is due during delivery, so they miss all three findings above. Add those interleavings and both refusal combinations.

I modified no files. Cached binaries passed 39 transform tests and failed the height clock test, but they predate this revision; I did not treat those runs as validation of this commit.

# Disposition (round 1)

1. Taken as documentation: a timer's refusal stops the runner's clock at that timer (the runner's rule, LLP 1012 §2), and the event runs there, as `Runner::dispatch_at` runs ordinary input. Changing that rule is outside this fix; `advance_for_input`'s comment says it.
2. Taken: the advance and the event are split (`advance_for_input`, `deliver_after`). After the timers, the event runs only if the kernel's binding is still the one the input was validated against (`transform_drag_binding(handle) == input.binding`; for a height drag, `height_drag_target(handle) == target`); otherwise the timers' commits are kept and the event is refused (`committed:false`, "the gesture's binding changed before its event"). Test: `a_timer_that_unbinds_the_photo_before_its_release_ends_the_gesture` (Apple and web).
3. Taken: geometry advances the runner before it adopts the held pair's authoring and cancels it, so the cancellation meets what the timers left. Test: `a_timer_that_drops_the_transition_before_new_geometry_snaps_the_cancel` (Apple and web). The web's failure fallback still lowers current values before the receipts, in the same batch, which the presenter applies in order.
Pre-existing follow-ups: Apple resize is taken (advanced to the host's clock before its handlers; a view a timer removed is skipped); Linux retained press/swipe and the terminal are queued (QUEUE.md).
