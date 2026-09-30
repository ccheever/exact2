# LLP 1073: `every(frame, action)` — a task that fires once per presented frame

**Type:** RFC
**Status:** Accepted (Charlie, 2026-09-29: "Yeah add the every frame task"); implementing
**Systems:** Contract syntax and lowering (`task … every(frame, a)`), Plan (`timers.frame`), Runner (`Runner::frame`, virtual frames on a seek), JS web target (`host/web-js/rt.js`, a `requestAnimationFrame` loop), wasm web, Apple and Linux hosts (their display links call `frame`), Agent API (the seekable clock's virtual display)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-29
**Related:** LLP 1006 §4 (tasks: `every`, `after`); LLP 1005 §6 ("The clock": `advance` is a seek); LLP 1043.000 D8 and ruling 5 (hosts wake on the display frame near a timer's due time; "a display-linked source is its own document, written only if judder or cost shows" — this is that document); LLP 1027.000 and 1027.000.000 (`now()` is elapsed time; the date is `exactTime.epochAtZero + now()`); LLP 1012 §2 (the agent's clock); LLP 1056 D5 (Canvas 2D's frame request, the nearest precedent); `rules/DEFERRED.md` §Motion ("no per-frame callbacks", amended here); the web-framework bench's grid ticker (`web-framework-bench/apps/grid`, EXACT2-FINDINGS.md)

## Summary

Contract has one kind of clock: `task t mount / every(ms, action)`, whose
missed intervals all fire on the next `advance`, each at its own due time. A
host can't run that in step with the display: a 60 Hz ticker written as
`every(16, step)` drifts against a 60 Hz display, fires twice in some frames
and none in others, and after a stall fires every tick it missed back to back
and paints fewer frames. The grid bench's 1,000-row ticker painted 258 frames
with 33 dropped where the React, Solid and Svelte builds, each ticking on
`requestAnimationFrame`, painted about 304 with 1.

This adds one task form, `every(frame, action)`. It fires the action once
for each frame the host presents, at that frame's time, and never catches
up a frame it missed. Where no display presents frames (the agent's
seekable clock, tests), the display is virtual: a frame every 1000/60 ms
after the task last fired, each fired at its own time. So a seek of
+1000 ms fires sixty frames on every host.

## Design

### D1 — The form

```
task ticker mount
  every(frame, step)
```

`frame` in `every`'s first position is a word, not an expression (a slot
named `frame` doesn't change it). `after(frame, …)` is refused
(`contract-task-body`): "after the next frame" is not something an app has
asked for, and the one-shot timer covers "soon". One entry per task and
root-only tasks are unchanged (LLP 1006 §4; `type-child-resource`).

### D2 — What a frame is

- **Presented frames (a host with a display).** The action runs once per
  frame the host is about to present, before that frame's paint, while the
  root is mounted:
  - the web: a `requestAnimationFrame` callback;
  - Apple: the session's `CADisplayLink` tick;
  - Linux: the display loop's frame.
  A frame runs at the display's rate. On a 120 Hz display that is 120 times
  a second, so an app that must not depend on the rate reads `now()`.
- **The clock.** The commit's `now()` is the frame's time, on the same
  elapsed clock as every other timer, where the host has one (the web's
  `requestAnimationFrame` timestamp and Apple's `targetTimestamp`, less the
  clock's start), never earlier than the clock already reads.
  `exactTime.epochAtZero + now()` is still the date.
- **No catch-up.** A frame the host could not present (a long commit, a
  hidden tab, a backgrounded app) is not fired later. After a stall the task
  fires once, at the next frame, and `now()` shows how long it was. This is
  the difference from `every(16, …)`, whose missed intervals all fire
  (LLP 1005 §6: `advance` is a seek).
- **Order.** At a frame at time *t* the host first advances timers to *t*
  (each due timer at its own time, as before), then fires each frame task at
  *t*, in plan order. Every task of the frame is its own commit, as a
  timer's is. The frame then paints.
- **Nothing to present, nothing fired.** A frame task keeps the host's frame
  source running while the root holds one. A task that must stop checks a
  slot (the grid's `step` does nothing while the ticker is off), as a timer's
  action does. A task isn't paused by writing state.

### D3 — The seekable clock: a virtual display

Under the agent (LLP 1012) and in tests the clock is moved by seeks, not by
a display. There the display is virtual. It presents a frame every
P = 1000/60 ms after the task last fired, or after mount, and never drops
one:

- A seek to *T* fires each virtual frame due at or before *T* at its own
  time, interleaved with timers in time order. Ties go to the plan's
  earlier row, frame tasks among timers. The next frame is at the time the
  task fired plus P, as an f64 sum, so every host computes the same times
  bit for bit.
- `clock +1000` fires sixty frames on every host, and `now()` reads the
  same values on each. The agent sees the same thing a 60 Hz display that
  never drops a frame would show.
- A presented frame at *t* moves the task's next virtual frame to *t* + P.
  An agent that attaches to a running app therefore continues from the last
  painted frame and has no backlog to replay.
- The runner's per-seek limit (4,096 commits, `TimerFireLimit`) counts
  frames as it counts timers.
- A render (LLP 1048.000) fires no frame task. The completion rule already
  ignores timers.

### D4 — The runner's API

- `Runner::frame(now_ms) -> Advanced`: a presented frame. It advances timers
  to `now_ms` as `advance_timed` does, then fires every frame task once at
  `now_ms` and sets each task's next virtual frame to `now_ms` + P. Like
  `advance_timed`, it returns the commits with their times and any refusal,
  so hosts handle it the same way.
- `advance_timed(now_ms)` is the wall-clock timeout's path, and fires no
  frame task: display frames are `frame`'s job. `advance` (tests) and
  `advance_until_request` (the agent's jump) are seeks, and fire virtual
  frames (D3).
- `timer_due_ms()` leaves frame tasks out: a host wakes for them by its
  frame source, not by a deadline. The new `wants_frames()` is true while
  the plan has a frame task, and a host keeps its frame source running
  while it holds.
- **Plan:** `timers` gains `frame: bool`. A frame row has `interval_ms` 0
  and `once` false (`PlanError::FrameTimer`). Every other row still needs
  `interval_ms` ≥ 1 (`ZeroInterval`). The format digest changes, so an older
  host refuses a newer plan by name.

### D5 — Hosts

| host | presented frames | seeks |
|---|---|---|
| JS web target | `rt.js`: a `requestAnimationFrame` loop while a frame task exists and the clock isn't the agent's, calling `frame` (timers, then frame tasks) | `rt.js` `advance` and `agent.js`'s stepped jump, virtual frames as D3 |
| wasm web | `timer-glue.js`: while the batch says `frames`, `exact_frame(now)` each animation frame | the runner (`advance_until_request`) |
| Apple | `Frames` (`CADisplayLink`) runs while the batch says `frames`; each tick calls `frame(now)` | the runner |
| Linux | the display loop asks for a frame while `wants_frames()` and calls `frame(now)` | the runner |

The batch each Rust host sends its view layer carries `"frames": true`
while `wants_frames()` holds, next to `timer_due_ms`.

## What it doesn't do

- **No frame argument.** The action takes no parameters, as a timer's
  doesn't. The time is `now()`, and the time since the previous frame is
  something an app keeps in a slot if it wants it.
- **No rate choice.** `every(frame)` is the display's rate. A fixed-rate
  loop is still `every(ms)`.
- **No per-node callbacks.** One task at the root, like any task, dispatching
  an action that commits. DEFERRED's "no per-frame callbacks" stays true of
  gestures and motion. This admits exactly one root-level task form.

## Measured

To be filled in as it lands: the grid bench's ticker scenario on
`every(16, step)` against `every(frame, step)`, headful desktop and mobile,
with the SSR frameworks as same-session controls.
