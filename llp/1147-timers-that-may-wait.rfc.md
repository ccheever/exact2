# LLP 1147: Timers that may wait for a frame, an update, a scroll's end or idle

**Type:** RFC
**Status:** Draft, 2026-10-27. Reviews: `llp/reviews/rfc-2026-10-27-1147-*`. The scope expansion in D9 needs Charlie's waiver under `rules/DEFERRED.md`.
**Systems:** Contract syntax and lowering (`every`, `after`: `wait=`, `max=`); Plan (`timers.wait`, `timers.max_ms`); Runner (held timers, `Runner::waits`, `Runner::release`, `Runner::frame`, takeover); Lean semantics and `contract-difftest`; JS web target (`host/web-js/rt.js`, `schedule.js`); Apple host (`SessionClock.swift`, `SessionClockTimer.swift`, `ScrollEnd.swift`, `IOS/FrameClockIOS.swift`); Android (`Owner.kt`, `ExactView.kt`, `Scrolling.kt`); Linux and Windows painted hosts (`display.rs`, `window.rs`); terminal host (`term/unix.rs`)
**Author:** Claude (Opus 5.5)
**Implementer:** none yet. This document proposes; it specifies nothing until it has one.
**Date:** 2026-10-27
**Related:** `issues/20261027-timers-cannot-wait-for-a-frame.md` (the evidence and platform facts this builds on); LLP 1073 (`every(frame, …)`, virtual frames); LLP 1092 D7–D10 (gated tasks); LLP 1043.000 ruling 5 (hosts advance near deadlines on the display frame); LLP 1072 (work off the main thread); LLP 1144 (measuring a fling)

## Summary

A Contract timer becomes due at its deadline and runs when the host next processes it.
During a scroll, the resulting batch of UI changes can consume time needed for a
frame. An app cannot currently permit the host to defer that work to a suitable point.

This RFC adds `wait=` to `every` and `after`, and `max=` to bound the wait for
`scrollend` or `idle`. On a live clock:

| `wait=` | Admission condition, once the deadline is reached | Waiting bound |
|---|---|---|
| none (the default) | ordinary timer processing | — |
| `frame` | the first frame callback at or after the deadline, before Exact's frame tasks | 100 ms |
| `update` | a frame callback when the host observes frame demand; immediate admission when it observes none | 100 ms |
| `scrollend` | no observed scroll activity in the session | `max=`, or no bound |
| `idle` | D2's condition: no observed activity or conflicting work, and no known work due soon | `max=`, or the larger of the timer's interval and 1,000 ms |

The bound is measured from the nominal deadline. Reaching it ends the intentional
wait; it does not guarantee execution by that time. D2 defines the admission
conditions, exceptions and limits. D3 defines the caps, and D5 defines behavior on the
seekable clock. Timers remain precise by default: they acquire no intentional wait
unless the author requests one.

## 1. The problem

Apple processes due timers from display-link callbacks, common-mode `Timer` callbacks,
and `advanceIfDue` after a batch lands. `SessionClockTimer.wake` normally uses the
display link when the nearest deadline is within eight 60 Hz frame periods (about 133
ms), and a main-thread `Timer` otherwise. A hidden session, or one whose recent timer
advances changed nothing shown, can use the `Timer` even for a near deadline. An armed
frame task keeps the display link running. These paths can process timers during
scrolling. The main thread waits synchronously for the runner in `Owner.sync` and
applies the returned batch.

On the JS web target, `drive()` schedules non-frame timers with `setTimeout`. An
active `paint()` frame loop also advances due timers before its frame tasks. There is
no near-deadline threshold in this target.

On Lexy's iPhone Duo at 120 Hz, a glide missed two frames in one app batch whose
main-thread turn took 18 ms. Whether a timer caused that batch is not known. In
Messages on an iPhone Duo simulator, `every(250, poll)` ran six times in each 1.5 s
sampling window after a fling, always in a display-link frame's batch. Four of the
five late frames in those windows immediately followed a timer beat's frame and
included a wait on the runtime (§5). The traces do not establish that the beat caused
the wait. An app has no way to request that a timer wait until scrolling ends or join
a frame being produced for another reason.

Waiting does not make a batch longer than a frame fit in one. That needs batches that
do not block the main thread, which LLP 1072 began for list rows.

## 2. Decisions

### D1. The form

```
task ticker mount
  every(5000, tick, wait=scrollend)
task autosave when draft != saved key=draft
  after(800, save, wait=idle, max=5000)
task progress when uploading
  every(250, sample, wait=update)
```

`wait=` takes a word, not an expression, as `every`'s `frame` does (LLP 1073 D1). Each
word uses a web name: `frame` refers to `requestAnimationFrame`, `update` to HTML's
"update the rendering", `scrollend` to CSSOM View's event of that name, and `idle` to
`requestIdleCallback`. `max=` is a literal whole number of milliseconds from 1 to
4,294,967,295, as an interval is (`contract/lower/src/timers.rs`). Both follow the
action, as `play(tracks, bpm=…)` takes its named arguments (LLP 1125 D2), in either
order, each at most once.

The compiler refuses (`lower-timer-wait`) an unknown word or an expression after
`wait=`; a `max=` that is not such a literal; a repeated argument; `wait=` on
`every(frame, …)`, which already runs at a frame; and `max=` without `wait=`, or with
`frame` or `update`, whose bound is fixed (D3). The plan decoder refuses the same rows
(`plan/src/lib.rs`, which already refuses a frame row with an interval), so a plan
from elsewhere cannot carry one. A timer waits for one thing: a combination, such as a
scroll's end and then a frame, is not proposed (§3).

### D2. What each wait is, on a live clock

A **firing** is one scheduled invocation of a timer's action. Its **nominal deadline**
is the time assigned by the timer's schedule. Its **execution time** is the time its
commit reads when it starts (D4 and D5). A **precise** timer has no intentional wait.

A waiting firing can also have a **cap**: an absolute time equal to its nominal
deadline plus D3's waiting bound. The bound is a duration; the cap is a time. An
uncapped `scrollend` firing has no cap.

On a live clock, an advance records a due waiting firing as **held** until the host
**admits** it: permits the runner to execute it. A **wait point** is the callback or
observed condition that permits admission for that wait kind. Reaching the cap instead
makes the firing ordinary due work. D4 defines admission order; D7 identifies the
signals each host observes.

**`frame`.** The host admits the firing at the first frame callback it runs whose
sampled time is at or after the deadline, before that frame's frame tasks (LLP 1073 D2
runs timers first, then frame tasks). When the host knows it runs no frames, because
the page is hidden, the app is in the background or there is no display, it admits the
firing at its deadline, as `SessionClockTimer.wake` already treats a hidden session. A
frame callback does not promise that a frame is presented.

**`update`.** **Frame demand** is the host's evidence that frames are being made for a
reason other than this wait: Exact's own frame work (motion, frame tasks, a held
`frame` timer), observed scroll activity (below), and, where a platform reports them,
its UI updates (D7). A callback the host asks for only to check waits is not demand.
At its first check at or after the deadline, the host samples demand. With demand, it
admits the firing in the first eligible frame callback, before Exact's own frame work.
A check at the start of a callback can admit the firing in that callback, including
when the cutoff equals the deadline. If the host observes demand ending before that
callback, it admits the firing immediately. Without demand, it admits the firing
immediately.

An `update` firing does not request a frame solely for itself; a `frame` firing can.
The `update` callback on iOS and tvOS 18 and later precedes that update's input
dispatch. On the web it follows the rendering update's scroll steps. On Android,
Exact's owner thread has no guaranteed ordering relative to main-thread input.

**`scrollend`.** **Scroll activity** is what the host observes of scrolling in the
session: **gesture holds**, a touch or pointer down on a scroller or a wheel's recent
events, and **movement**, a scroller seen moving, from its first notice until its
completion. Completion is the observed end of the gesture and its momentum or
deceleration, the end of a scroll animation, removal of the scroller, or the host's
quiet-period fallback (D7). Holds and movement are kept apart, so a finger's lift does
not end a deceleration and an old completion does not end a newer gesture; a finger
that holds a scroller still keeps its hold. The host admits the firing when activity
is empty. While the session is **reported off screen**, its scroll activity counts as
empty for admission. This means the app is in the background or, on the web, the page
is hidden or its viewport observation reports no intersection (D7). It does not mean
movement has stopped: the iOS probe observed deceleration continuing and ending in the
background while the app remained executable (§5).

**`idle`.** The host admits one firing when it samples all of these true at once,
immediately before admitting it:

1. scroll activity is empty;
2. there is no frame demand, and on the web no document animation runs (D7);
3. no batch is being made or applied;
4. no firing is due or held other than held `idle` firings;
5. no known deadline or frame is due at or before the sampled time plus 1000/60 ms.
   This is a fixed horizon, independent of the display's rate. `timer_due_ms` supplies
   the earliest runner deadline, including timers, caps, `then`, `next`, polls,
   presence and other runner wakes. A `None` result imposes no runner-deadline
   restriction.

"Due" and "known" refer to work visible to the session's scheduler. Platform work that
Exact cannot observe is outside the condition. The deadline, the end of scroll
activity or frame demand, completion of batch application, and execution of due work
each prompt a check. D7 specifies the callback or loop opportunity at which the host
samples the condition and admits a firing. After an `idle` firing's batch is applied,
it samples the condition again before admitting the next. A platform's idle callback
is an opportunity to check the condition, not the condition. The fifth term keeps an
`idle` batch from starting when known work is about to; a batch longer than a frame
can still delay input that arrives during it, as any batch can (§1).

**What the waits guarantee.** For a wait the host implements, a firing is admitted
only at a wait point of its kind or once its cap has been reached, and never before
its nominal deadline. Admission follows D4's order. A gate or key change can cancel
the firing, and a refused attempt is spent.

These guarantees concern admission based on host observations. They do not promise
physical stillness, presentation of a frame, execution by the cap, or a batch that
fits in a frame (§3). The following limits apply:

- **Notice.** A scroll that another thread starts, the browser's compositor or
  Android's main thread while Exact runs on `Owner`, counts from when its first notice
  reaches the host; a firing admitted before then, or just before a scroll begins, can
  run in that scroll's first frames, as a precise timer can.
- **Quiet on the web.** Browsers end some scrolls without any event (§5), so on the
  web a scroll that no gesture holds counts as complete after 1,000 ms without
  observed movement, or 100 ms after its last `scroll` where the browser has no
  `scrollend`. A scroll that pauses that long and then goes on can have an uncapped
  `scrollend` firing run in it. No length of pause proves a scroll over, since CSSOM
  View sets no bound on one; the rule accepts that so a scroll that ended silently
  does not hold a firing forever.
- **Reports arrive late.** The web's off-screen report arrives after the rendering
  update it describes, and Android's main-thread reports after the state they
  describe; admission uses the latest report the host has.
- **Android publication.** A release decides when `Owner` runs the action; the main
  thread applies what it publishes when it arrives, which can be after a scroll has
  begun. A reply the action awaits lands when it arrives, as today.
- **Caps.** A cap ends the waiting, not the latency: at its cap a firing becomes
  ordinary due work (D3), and runs as soon as the host can run timers, which a
  suspended, throttled or stopped executor delays (HTML's
  [timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timers)
  allow it; Android's `ExactActivity.onStop` stops the session's view). A capped
  firing, `idle` included, can run mid-scroll at its cap.
- **No bound.** An uncapped `scrollend` firing can wait indefinitely, either because
  observed activity continues or because the host receives no observation that ends
  it. For example, a browser can stop rendering without either off-screen signal
  reporting it. `max=` bounds the intentional wait, subject to the cap's latency
  limits above.
- **Unsupported waits.** A wait the host does not implement is processed as a precise
  timer (D6). It has ordinary timer behavior and none of the requested wait's
  admission guarantees.

### D3. Caps

A firing's cap is its nominal deadline plus the bound below, fixed when the firing is
armed; it does not move while the firing is held. At the cap the wait stops
restricting admission: the firing becomes ordinary due work, and the next advance runs
it with the ordinary work due then (D4). The host keeps a wake at the earliest cap
(D6). The bounds:

- **`frame` and `update`: 100 ms.** This spans six frame periods at 60 Hz. It limits
  intentional waiting when the expected callback is delayed or never arrives: for
  example, when rendering stops after the deadline or a passive `UIUpdateLink` reports
  no further update.
- **`scrollend`: `max=`, or none.** Without `max=` the firing waits for the scroll to
  end, however long it lasts, within D2's limits, since running mid-scroll is what the
  author opted out of. `max=` says how long it may wait before it runs mid-scroll
  anyway. A scroller that a frame task moves on every frame never ends; such an app
  names `max=`.
- **`idle`: `max=`, or the larger of the timer's interval and 1,000 ms.** Continuous
  frame demand, such as an armed frame task, prevents idle admission, so `idle` always
  has a cap. Armed at time 0, `after(800, save, wait=idle)` has a deadline of 800 ms
  and becomes ordinary due work at 1,800 ms. An `every(5000, tick, wait=idle)` firing
  has a default waiting bound of one interval. Shorter intervals can merge several
  beats: an `every(250, sample, wait=idle)` firing held from 250 ms until 1,250 ms,
  and completing at 1,250 ms, runs once and next becomes due at 1,500 ms. Execution
  delayed beyond the cap, or a longer callback, can merge further beats under D4.

### D4. The clock and the order

**`now()` on a live clock is when the firing ran.** Every commit on a live clock reads
the clock as it starts (`start_commit`), so a precise timer that runs late already
reads the later time, and a held firing is the same. A `then` or a queue `next` that
its action arms counts from that commit's start, not from the nominal deadline. Today
`scheduled(at)` gives a precise callback's armed work its scheduling instant
(`advance.rs`, `armed_at`); for an admitted firing that instant is its commit's start.
On the seekable clock `now()` reads the execution time D5 gives. An app that computes
from an end time (`endsAt - now`, `docs/recipes/timers.md`) needs no change.

**An `every` keeps its phase.** Its phase is the sequence of beats established when it
is armed, separated by its interval. After a waiting firing runs, the next nominal
deadline is the first beat strictly after callback completion on a live clock, or
strictly after its execution time on the seekable clock. The live-clock calculation
uses `beat_after`, as it does for a late precise timer.

Beats reached while the firing waits or its callback runs are coalesced into that one
invocation. They do not produce a backlog. A key change establishes a new schedule
under the gated-task rule below.

**Admission turns.** On a live clock the host admits held firings in **turns**. A turn
has one **cutoff**, the clock sampled when it starts, and runs in this order:

1. **Ordinary work first.** The runner runs the ordinary work due by the cutoff, as an
   advance does: precise timers, `then` and `next`, polls, and firings whose caps have
   passed. A gate one of them turns false drops a held firing (below). The host
   applies that batch before anything else in the turn.
2. **Then eligible waits.** Once no ordinary work due by the cutoff remains, the host
   determines which wait kinds are eligible from its observations after applying the
   ordinary batches. The runner processes held firings of those kinds whose nominal
   deadlines are at or before the cutoff, ordered by nominal deadline and then by plan
   row. Each firing is its own commit.
3. **`idle` alone.** An `idle` firing is admitted only after the others, one per turn,
   and only when the host's sample of D2's condition, taken after everything before it
   was applied, holds.

A frame callback is an admission turn whose cutoff is the clock sample taken when the
callback starts. Its frame tasks follow the admission turn (LLP 1073 D2). For the same
cutoff and eligible wait kinds, firing order follows the rules above, independently of
the order in which the host checks those kinds.

A `then` or `next` armed by an admitted firing belongs to a later turn, including when
its scheduling time equals the current cutoff. Different platform callbacks start
different turns.

**Refusals and the fire limit.** A timer firing whose commit is refused is spent.
Firings not yet reached remain held or due. Unless the runner is poisoned, a refusal
or `TimerFireLimit` returns control to the host, which resumes the unfinished turn
with the same cutoff and its remaining work. Completed work is not repeated, and work
due after the cutoff cannot overtake the turn.

The fire limit is checked before a firing (`advance.rs`) and does not consume that
firing. Poison ends processing; the host does not keep attempting admission on a
poisoned runner.

**Gated tasks (LLP 1092 D8).** A gate that turns false drops a held firing, which then
does not run. A key that another commit changes drops it and re-arms the task from
that commit's time. On a live clock, a repeat whose own action changes its key starts
its next interval when its callback completes (`advance.rs`; `live_timers.rs`,
`a_rekeyed_timer_arms_from_its_callbacks_completion`). On the seekable clock it starts
that interval at the firing's execution time.

**What stays precise.** `wait=` applies only to `every` and `after`. `then`, a queue's
`next`, polls, live `time` and `progress`, and sequence scheduling retain their
existing deadline rules.

### D5. The seekable clock

When the agent or a test owns the seekable clock, timer scheduling uses deterministic
rules independent of live scrolling and display callbacks. The runner, JS target and
Lean compute the same times. A firing keeps its nominal deadline *D* and is scheduled
at execution time *E*, which its commit reads:

| `wait=` | *E* |
|---|---|
| `frame` | the least virtual frame *F*(*k*) ≥ *D*, where *F*(*k*) is the binary64 value of *k*·1000/60, the product first (`virtual_frame(0, k)`, LLP 1073 D4) |
| `update` | *D*, since a seek has no frame demand |
| `scrollend` | *D*, since a driver's scroll lands at once |
| `idle` | *D*, after every other task due then (below) |

*k* is found by correction, not by `ceil(D·60/1000)` alone: for *D* = 1000/60, exactly
*F*(1), that expression gives 2. Take *k* = `ceil(D·60/1000)`, then lower it while
*F*(*k*−1) ≥ *D* and raise it while *F*(*k*) < *D*. *k* is a 64-bit integer, since the
clock's range exceeds what `virtual_frame`'s `u32` holds; the runner gains a 64-bit
form of it.

Task firings are selected by *E*. At the same *E*, non-`idle` task firings precede
`idle` task firings; within either group, the order is by *D* and then plan row. A
timer without `wait=` has *D* = *E*, so plans without waiting timers retain their
existing task order. The rest of that order stands: an armed `then` runs before a
timer due at or after its time and a queue's `next` between them, reconsidered after
each commit (LLP 1092 D3); a sequence's transport wake follows every task due at its
instant, `idle` included (LLP 1125 D3); a hatch's instant comes after them (`rt.js`,
LLP 1075.003.000.001). So a `frame` timer due at 49 ms runs at 50 ms before a precise
timer or a frame task due at 50 ms, and an `idle` timer runs after a precise timer due
at its time. This order is the seekable clock's own. It does not copy a live turn's
(D4), which runs ordinary work first: the two clocks place firings at different times,
and today a seek and a presented frame already order a frame's `then` differently. On
a seek, `update`, `scrollend` and `idle` firings run at their nominal deadlines,
`idle` in the order above, and `frame` firings at the next virtual frame. With no
intervening input or replies, and provided no refusal or fire-limit stop interrupts
processing, one seek and smaller seeks to the same final time produce the same timer
commits at the same times. A waiting repeat's next beat is the first of its phase
strictly after *E*, so `every(1, tick, wait=frame)` runs once per virtual frame. A
timer already held when the agent takes over runs at the takeover (D6). Under
`--timing platform` (LLP 1144 D1) the clock is the wall's, and D2 applies.

### D6. Plan and runner

**Plan.** `timers` gains `wait`, a `u8` (0 for none, then `frame`, `update`,
`scrollend` and `idle`), and `max_ms`, a `u32` that is 0 when there is no `max=`. The
format digest changes, so an older host refuses a newer plan by name. The decoder
refuses an unknown `wait`, a `max_ms` with no wait or with `frame` or `update`, and a
`wait` on a frame row (D1).

**A host says which waits it implements.** `Runner::hold_waits(kinds)` names the wait
kinds the host implements, before live scheduling starts; the set does not change for
the session. It names none by default. On a live clock, a timer whose wait the host
does not implement runs at its deadline, as a precise timer does. So a host that has
not implemented waits runs `wait=` timers as it runs timers today, and the seekable
clock's times (D5) do not depend on it.

**The runner holds; the host admits.** On a live clock, an advance holds a due firing
whose wait the host implements, unless its cap has already made it ordinary due work.

`timer_due_ms()` returns the earliest ordinary deadline or cap. Every armed firing
whose wait the host implements contributes its cap from arming onward, whether or not
it is held. An uncapped firing contributes no cap. Thus an uncapped held firing cannot
hide another firing's cap. The host schedules a wake for the reported deadline;
execution remains subject to D2's latency limits.

The host also schedules a check at a waiting firing's nominal deadline, from
`Runner::waits()` (below). A check that admits nothing creates no timer batch. The
host may omit this nominal-deadline wake while an observed condition prevents
admission and the end of that condition will trigger another check—for example, an
active scroll holding a `scrollend` firing. It still schedules the cap wake.

**`Runner::waits() -> Waits`.** For each wait kind it reports whether a firing of that
kind is held, and the earliest nominal deadline among those not yet held, or `None`. A
host reads it where it reads `timer_due_ms` and `wants_frames`, and the batch carries
it. A `frame` deadline tells the host to ask for one frame once it passes; the others
tell it which signal to watch.

**`Runner::release(kinds, cutoff) -> Admitted` and `Runner::frame(kinds, cutoff)`.**
These calls implement D4's admission turn.

For a turn with held firings to consider, ordinary work due by `cutoff` runs first. If
any runs, the call returns its batch with `continue` and admits no held firing. The
host applies the batch, recomputes the eligible kinds and resumes with the same
cutoff. Once that ordinary work is exhausted, the runner processes held firings of the
supplied kinds in D4's order. `Runner::frame` runs the frame tasks after the admission
turn.

An `idle` call supplies `idle` alone and admits at most one firing. The host makes it
last, after applying the preceding batches and sampling D2's condition.

A recoverable refusal or the fire limit returns `continue`. Continuation preserves the
turn's progress: completed firings and frame tasks do not repeat, and `then` or `next`
work created during held-firing admission remains for a later turn as D4 requires.

When no firing is held or becomes held for the turn, the existing path remains:
ordinary due work is returned without an extra admission round, and `Runner::frame`
runs ordinary due work and frame tasks in one call. A host implementing no waits
therefore retains its existing behavior.

The host samples the idle condition from its activity, demand and batch state.
`timer_due_ms()` supplies the runner-deadline part of the fifth term; the host also
accounts for its scheduled frames.

**Rollback.** Held firings, their deadlines and caps are runner state, saved in the
checkpoint a commit makes and restored on rollback, as timers already are
(`commit.rs`). A firing is spent before its commit runs, so a rolled-back commit does
not run it twice.

**The agent's takeover.** When the agent takes the clock (`set_live_clock(None)`), the
host first stops its turns, then the runner samples the live clock once, *R*, no
earlier than its current time. At the first seek, every outstanding firing of an armed
timer whose nominal deadline is at or before *R* has execution time *R*. This includes
held firings, precise firings and waiting firings whose nominal-deadline wakes were
skipped. The runner selects them in D5's order, retaining nominal deadlines and
repeating timers' phases. Gate or key changes can still cancel a firing before it is
reached, and a refused firing is spent. No commit after takeover reads a time before
*R*. The host advances a **clock generation** as it hands the clock over, and every
callback it set for waits (a frame, a timeout, an idle callback, an observer, an
animation promise, an Android report) does nothing when it runs under an older
generation, so a callback queued before the takeover cannot admit a firing. Firings
whose deadlines have not passed keep their waits, and D5 applies to them. The seekable
clock holds nothing, so handing the clock back holds nothing.

### D7. Hosts

This section identifies each host's observations and admission opportunities. Every
table entry is subject to D2's conditions and exceptions, D3's caps and D4's turn
order. A callback listed here does not by itself establish eligibility.

| | `frame` | `update`: frame demand, and the callback | `scrollend`: scroll activity | `idle`: where the host samples D2's condition |
|---|---|---|---|---|
| JS web target | `setTimeout` to the deadline, then one `requestAnimationFrame`; runs before `paint()`'s frame tasks | demand: a `requestAnimationFrame` Exact asked for its own work (motion, frame tasks, a held `frame` firing), or observed scroll activity; at the start of the next callback, before Exact's own work | gesture holds and moving scrollers (below) | in a `requestIdleCallback` callback (Chrome, Firefox); where it is missing (Safari ships it disabled through 27.2), in a `setTimeout(0)` posted from a `requestAnimationFrame` callback, or from a timeout while reported off screen |
| Apple, iOS and tvOS 18 and later | one callback requested by the session from the shared `CADisplayLink` (`FrameClock`) | demand: a passive `UIUpdateLink`'s callback within the last 1000/60 ms, Exact's own frame work, or observed scroll activity; in the next link callback's `afterUpdateScheduled`, the phase for "purely time driven" work, which runs before input events | touches on scrollers from a gesture recognizer that only watches (below), and the scrollers `ScrollEnd` holds open (`isDragging`, `isDecelerating`, scroll animations), whether or not a node listens | in a default-mode `beforeWaiting` observer on the main run loop (below); term 3 also counts a job posted to the owner thread (`Owner.post`, LLP 1072's collection fill) that is unfinished or whose publication is not yet applied |
| Apple, iOS and tvOS 17, and macOS | the display link's next callback | demand: Exact's own frame work or observed scroll activity; at the display link's next callback | on iOS and tvOS, the touch holds and `ScrollEnd` state described above; on macOS, `ScrollEnd.live` covers a trackpad's live scroll and momentum, and `ScrollEnd` tracks other movement and scroll animations | the default-mode main-run-loop observer described above |
| Android | `Owner`'s frame callback (`turn`), on Exact's own thread | demand: `Owner`'s `turn` posted for its own work (`Native.animating()`), or the main thread's latest report of scrolling or animation sampling; at the start of `Owner`'s `turn`, before `Native.displayFrame()` | the main thread's latest report: a touch down, and `Scrolling.kt` dragging or flinging | in a `MessageQueue.IdleHandler` on `Owner`'s looper that returns false after one call; the main thread's report supplies terms 1 to 3 (below) |
| Linux, Windows (painted) | the display loop's next frame (`display.rs`, `window.rs`) | demand: a frame turn due for the presenter's own work; at that turn | the presenter's scroll state | at the loop's point before it sleeps |
| Terminal | no display frames: at the deadline | no demand: at once | none: a key's scroll lands at once, and the terminal scrolls its own scrollback | in the `poll` loop with no input pending |

**The web's scroll activity.** The session's scrollers are the document's elements and
the document itself, seen through capture-phase `scroll` and `scrollend` listeners on
the document, and the visual viewport, seen through listeners on
`window.visualViewport`, whose events have it as their target (CSSOM View's [event
summary](https://drafts.csswg.org/cssom-view/#event-summary)). An embedded document's
scrollers belong to its own session, if it has one. Gesture holds, from passive
listeners that never delay scrolling: a touch from its `touchstart` to the `touchend`
or `touchcancel` for that touch, since a browser that takes over a pan cancels the
pointer while the finger stays down ([Pointer
Events](https://www.w3.org/TR/pointerevents3/#suppressing-a-pointer-event-stream)); a
mouse or pen pointer from `pointerdown` to `pointerup` or `pointercancel`; a `wheel`
until 100 ms after its last event. Movement tracking starts at a scroller's first
`scroll`, which establishes its initial observed position and movement time. Tracking
ends on `scrollend`, disconnection, or D2's quiet-period fallback. Removal during
movement produces no `scrollend` in the probes (§5).

Quiet completion applies only when no gesture holds that scroller. With `scrollend`
support, the host samples position in animation-frame callbacks and measures the quiet
period from the initial observation or the latest observed movement. Without support,
it measures from the last `scroll` event. D2 gives both durations. Support is
feature-detected with `'onscrollend' in window`; Safari before 26.2 uses the fallback
without `scrollend`. The bound is long because browsers end some scrolls silently, as
Chromium and WebKit do when an instant scroll to the same position replaces a smooth
one (§5; `host/web-js/list.test.mjs`), while Firefox holds one position for up to 68
ms in the middle of a smooth scroll on a loaded machine (§5). Whenever a gesture hold
or movement entry ends, the host checks held firings at the next available
animation-frame callback, which can occur later in the current rendering update. While
reported off screen it checks immediately. A gesture that ends without scrolling
therefore also prompts a check.

While rendering continues, these checks run after the rendering update's scroll steps.
A timeout alone does not declare movement complete. The blocked-main-thread probe
shows that a long gap between callbacks does not establish scroll completion (§5).

**The web's off-screen report.** The session is reported off screen while the page is
hidden (`visibilityState`) or the latest observation an `IntersectionObserver` has
delivered for an element covering the document's viewport (`position: fixed; inset: 0;
pointer-events: none`, empty, `aria-hidden` and outside the app's tree, so it takes no
space, no input and no place in the accessibility tree) says it does not intersect.
Before the first observation is delivered, a visible page is not reported off screen;
a hidden page still is. The observer uses its implicit root, the top-level viewport,
and the latest entry's `isIntersecting` value. This is a geometric report, including
edge-adjacent intersections, rather than proof that pixels are visible.

The host observes the covering element because the document root under `scroll
document` stays one viewport tall (`html { height: 100% }`, `host/web/index.html`) and
leaves the viewport as a long page scrolls. A corner pixel can leave view while part
of an iframe remains visible (§5).

In the offscreen cross-origin iframe probe, Chromium and WebKit delivered no
animation-frame callbacks and Firefox delivered two in two seconds. The child page
remained `visible`, while the observer reported no intersection (§5). An observation
is computed in a rendering update and delivered in a later task ([processing
model](https://w3c.github.io/IntersectionObserver/#processing-model)), so the report
is a policy for admission, not proof that rendering has stopped. While reported off
screen the host checks from timeouts, on `visibilitychange` and on the observer's
callbacks.

**The web's animations.** The browser runs the plan's CSS animations and transitions
(LLP 1055), not Exact's frame loop. For `idle`'s second term, the web host counts an
animation whose `playState` is `running` and whose `playbackRate` is not 0, found with
`document.getAnimations()`, which lists CSS animations, transitions and Web
Animations, keeping those on the document's timeline, as `agent.js` already does; a
scroll-driven animation follows its scroll, which scroll activity covers. The host
observes the animations' `finished` promises, using `Promise.allSettled` to handle
both completion and cancellation, and checks again when those promises have settled.
This includes animations created by `Element.animate()`.

Pausing a running animation or setting its playback rate to 0 does not settle its
`finished` promise ([Web
Animations](https://drafts.csswg.org/web-animations-1/#pausing-an-animation)). While
animations are the only remaining obstacle to idle admission, the host also schedules
checks every 100 ms. It calls `document.getAnimations()` again before admission. It
cannot tell which animations the compositor runs without the main thread, so it counts
all of them for `idle` and none as demand for `update`. An animation a native platform
runs without the main thread, such as a box's keyframes in Core Animation on iOS (LLP
1055.001), counts for neither: a batch does not hold it up.

**Apple's touches and updates.** A scroller counts from its touch down, since UIKit
runs the main run loop in its default mode until the scroll view's pan is recognized,
so the `idle` observer fires then (§5), and `ScrollEnd` learns of a scroller only once
its offset moves. The host adds an observing gesture recognizer to each scroller. It
never enters a recognized state, never cancels or delays touch delivery, allows
simultaneous recognition by other recognizers, and imposes no prevention or failure
dependency on them. It keeps its own set of the touches it saw begin, removes one when
it ends or is cancelled, and empties the set on `reset()`, when its view is removed
and when it is reused; each step is safe to repeat. When the set empties, the host
checks its held firings once UIKit has finished delivering the event, on the main
queue's next turn, whether or not a `scrollend` is owed, so a touch that lifts without
moving the scroller, a touch on a scroller that cannot scroll and a touch another
gesture wins each end with a check. Recognizers see touches before the hit-tested view
does (`UIGestureRecognizerSubclass.h`).

The `beforeWaiting` observer is installed only in the default run-loop mode. In the
iOS 27.1 probe it was silent during recognized dragging, the held drag and
deceleration; a common-mode observer ran about 450 times during deceleration (§5). The
host uses this callback as an opportunity to sample every idle term. Eligibility does
not follow from run-loop mode alone, and the cap's wake is independent of the
observer.

In the iOS 27.1 probe, a passive `UIUpdateLink` received callbacks throughout a drag,
a held drag and deceleration. Without XCUITest attached, it received none during the
initial still interval and two during each tested opacity fade, at the start and end.
The tested timers that changed nothing produced no updates. XCUITest added updates of
its own (§5). In every measured update containing touches or scroll callbacks,
`afterUpdateScheduled` preceded them. The host adds the link's action and enables it,
and leaves `requiresContinuousUpdates` at its default, false: setting it on a link
with no action crashes the app, even to false (§5). iOS and tvOS 17 have no passive
observer of UI updates; `UIUpdateLink` arrived in iOS and tvOS 18. Their demand is
Exact's own frame work and scrolling, so an update that only a gesture recognizer
causes on a still screen is not demand there.

**Android's reports.** Exact runs on its own thread (`Owner`) by default, and the main
thread draws, scrolls, samples the plan's animations (`ExactView`'s sampler; the
engine keeps no clock busy for an animation a host lowers,
`motion/src/engine/animate.rs`) and applies batches. The main thread sends `Owner` one
report each time any of these change, and `Owner` takes them in order: the reader that
sent it (the current `ExactView`), whether a touch is down, whether it scrolls,
whether it samples animations, and the number of the last publication it has fully
applied, after deferred moves and final placement, not when it took it from the queue
(`ExactView.takeOwned`). That number is new: each publication gets the next one, since
`Pub.seq` acknowledges scroll steps and repeats across publications. Session work
`Owner` posts to the main thread separately (`Owner.kt`, its `main.post` calls) counts
as unapplied until a report says it ran. `Owner` uses reports only from the current
reader. Until that reader first reports, including after replacement, the session does
not satisfy the idle condition. `Owner` decides from the latest applicable report,
which can already be stale (D2). With `owner=0`, a comparison mode that runs Exact on
the main thread, `ExactView`'s `turn` and the main looper stand in for `Owner`'s, and
no report is needed.

**Hosts awaiting implementation.** Until their implementation in §4's stage 3, the
wasm web host used by games and its text-flow executor declare no supported waits.
Their `wait=` timers therefore use precise scheduling (D6).

Android does not yet emit `scrollend`; its wait uses the scrolling state named in the
table. Linux already tracks and delivers scroll completion for nodes with `scrollend`
handlers (`host/linux/src/presenter/scroll_end.rs`; LLP 1005, "Linux, first step").
Wait admission observes session scroll activity regardless of whether an app installs
a handler.

Sources: for `UIUpdateLink`, `UIUpdateLink.h`, `UIUpdateActionPhase.h` and
`UIUpdateInfo.h` in the iOS 27.1 SDK and Apple's
[`UIUpdateLink`](https://developer.apple.com/documentation/uikit/uiupdatelink)
documentation; for gesture recognizers, `UIGestureRecognizerSubclass.h` and Apple's
[gesture recognizer state
machine](https://developer.apple.com/documentation/uikit/about-the-gesture-recognizer-state-machine);
for idle callbacks, the W3C's [Cooperative Scheduling of Background
Tasks](https://w3c.github.io/requestidlecallback/) and Android's
[`MessageQueue.IdleHandler`](https://developer.android.com/reference/android/os/MessageQueue.IdleHandler);
for rendering that stops on a hidden page, HTML's [event loop processing
model](https://html.spec.whatwg.org/multipage/webappapis.html#event-loop-processing-model),
and in an offscreen cross-origin iframe, Chromium's [rendering
throttling](https://chromium.googlesource.com/chromium/src/+/HEAD/third_party/blink/renderer/core/frame#throttling);
for a viewport out of view, the W3C's [Intersection
Observer](https://w3c.github.io/IntersectionObserver/); for a scroll replaced by
another, CSSOM View's [perform a
scroll](https://drafts.csswg.org/cssom-view/#perform-a-scroll); for document
animations, Web Animations'
[`Document.getAnimations()`](https://drafts.csswg.org/web-animations-1/#dom-documentorshadowroot-getanimations);
for browser support, caniuse's
[`requestIdleCallback`](https://caniuse.com/requestidlecallback) and
[`scrollend`](https://caniuse.com/mdn-api_element_scrollend_event) tables.

### D8. Docs and the agent

`docs/recipes/timers.md` gains a short section, "A timer that can wait". It recommends
`wait=scrollend` for a tick that refreshes what a list shows and `wait=idle` for saves
and housekeeping, says what D2's limits mean for each, and points a clock display to a
live `time`, which needs no task. `contract vocab every` lists the words. The agent's
`state.tasks` (LLP 1092 D10) reports whether a firing is held and its cap, if any.
`perf … during` counts timer batches by admission kind, including caps.

Apple currently suppresses automatic timer advancement in agent mode, including
`--timing platform`: `SessionClock.scheduleClock` arms no timer wake, and `fireClock`
returns. Explicit seeks still execute timers. Consequently, LLP 1144's fling report
does not currently measure live timer batches on Apple (§5). Before that report is
used for §4's comparison, platform timing must leave the app's timers running on the
live clock.

### D9. Scope

This expands DEFERRED's task forms (**Motion**, after LLP 1073's and LLP 1092's
expansions). It leaves computed intervals, component-scoped and action-started timers,
per-node callbacks, and gesture or motion callbacks outside scope. The existing root
`every(frame, …)` task remains available. It offers nothing in exchange (DEFERRED's
"Take": none offered).

Lexy's traces motivated the investigation, but its `every(5000, tick)` evaluates
nothing on Status at Lexy revision `8ec5ef8`. The candidate consumers are Excord's
`traffic` task (`every(2500, trafficTick)`) and Messages' `network` task (`every(250,
poll)`), whose work can affect a scrolled list.

The waiver request follows §4's device comparison of precise and waiting timers. The
Messages baseline in §5 shows timer work during flings and an association with later
frame delays on a loaded simulator. It does not yet establish the effect of changing
the admission point.

### D10. Tests

Each behavior is tested at the lowest level that exhibits it. Scripted clocks and
observations pin exact boundaries; platform tests pin the connection between real
events, host observations and runner commits.

A host test records which firing was admitted, whether a wait point or a cap permitted
it, and the activity, demand and reports available at that decision. Assertions use
that record rather than assuming physical stillness or requiring a callback to arrive
within a wall-clock interval. A capped firing is checked against the ordinary-work
rule, even if a loaded machine prevented earlier admission.

**Compiler and plan.** `contract/corpus/rejects.txt` gains `lower-timer-wait` fixtures
for D1's refusals: an unknown wait word; an expression after `wait=`; zero, negative,
fractional, overflowing or nonliteral `max=`; repetition of either named argument;
`wait=` on `every(frame, …)`; and `max=` without `wait=` or with `frame` or `update`.

Successful compiler tests cover both named-argument orders and the accepted `max=`
endpoints, 1 and 4,294,967,295. The plan decoder's tests construct each invalid
encoded row listed in D6 directly.

**Semantics, on the seekable clock.** `Contract/Runtime.lean` gives each firing its
wait, nominal deadline, execution time and cap, selects by execution time with D5's
order, and moves a waiting repeat to the first beat of its phase after its execution
time (`Runtime.lean`'s repeat today adds one interval). New corpus files under
`semantics/corpus/timers/`, which `contract-difftest` runs on the runner and the JS
target against Lean:

| File | What it pins |
|---|---|
| `wait-frame` | `frame` runs at the least virtual frame at or after its deadline; a deadline exactly on a frame (1000/60) runs at that frame, and the adjacent representable values on either side round as D5 says; several deadlines within one frame run at that frame by deadline |
| `wait-frame-every` | `every(1, tick, wait=frame)` runs once per virtual frame, at a fractional rearming time too |
| `wait-frame-then` | a `then` and a queue `next` the action arms count from the frame it ran at |
| `wait-same-time` | D5's task order: a gated `after(49, record, wait=frame)` runs at 50 ms before a precise `after(50, disable)` or an `every(frame, disable)` that clears its gate; equal execution and nominal times use plan row order; a `then` armed by the first action runs before another task at that execution time; `frame-and-after` retains its existing result |
| `wait-idle-order` | an `idle` timer on an earlier plan row does not run if a precise timer at the same execution time first clears its gate |
| `wait-every-phase` | a waiting repeat keeps its phase and skips beats at or before its rounded execution time |
| `wait-gated-drop` | a gate or key change before a pending `frame` firing's execution time cancels that firing; a key change arms a new deadline from the changing commit |
| `wait-split-seeks` | uninterrupted seeks to the same final time, with no intervening input or replies, produce the same commits at the same times |

Separate runtime tests cover a sequence wake following an `idle` task at the same
instant. Host clock tests cover a hatch instant following the tasks due then. These
cases do not claim comparison against Lean, which models neither transport wakes nor
host hatch callbacks. Live held-state cancellation is covered below.

**Runner, on a live clock.** `runner/tests/it/wait_timers.rs`, beside `live_timers.rs`
and with its scripted clock samples:

| Case | What it pins |
|---|---|
| holding and caps | a firing whose wait the host implements is held at its deadline and becomes ordinary work at its cap; `timer_due_ms` reports its cap from arming; an uncapped `scrollend` firing held at 100 ms does not hide a later one's cap (due at 200 ms, `max=100`: `timer_due_ms` is 300 before either deadline); a cap and a wait point at the same turn run the firing once |
| no host support | without `hold_waits`, every wait runs at its deadline |
| ordinary work first | a held `frame` firing due at 49 ms and a precise timer due at 50 ms that clears its gate, both reached by a turn at 51 ms, through `release` and through `frame`: the call returns `continue` with the precise commit, and the held firing is dropped |
| frame tasks last | at a frame callback at 50 ms, a held `after(49, record, wait=frame)` runs before an `every(frame, disable)` that clears its gate, as on the seekable clock |
| a refusing precise repeat | its refused firing is spent; the next call admits the held firings |
| a refusal among held firings | the refused firing is spent, the rest stay held, and the continued turn runs them |
| the fire limit | reached before the first held firing, it spends nothing, and continuing the turn with the same cutoff admits the held firings before work due after it |
| all candidates dropped | ordinary work drops every held firing and returns its batch with `continue`; after that batch is applied, the resumed call admits no held firing |
| `idle` alone | a call admits `idle` only with `kinds` = `idle`, and admits at most one firing; with two held `idle` firings, the second remains held after the first call |
| kinds together | with both kinds admitted, equal nominal deadlines use plan order: an earlier-row `scrollend` firing records before a later-row `update` firing clears its gate; in a second case, the later-row `update` firing has the earlier nominal deadline, runs first and drops the `scrollend` firing |
| `idle` merging | `every(250, sample, wait=idle)` held to its cap at 1,250 ms runs once, and its next beat is 1,500 ms |
| a self-rekeying repeat | a held `every` whose action changes its key starts its next interval at its callback's completion |
| `now()`, `then` and `next` | an admitted firing's commit reads the clock as it starts, and a `then` or queue `next` its action arms counts from there, not from the nominal deadline, including a callback whose start and completion differ |
| takeover | `set_live_clock(None)` with a waiting firing whose deadline passed without a wake, a held firing and an overdue precise timer: each runs once at *R* in D5's order, no later commit reads a time before *R*, phases are kept; a seek of 0 runs them; a takeover firing that refuses is spent; handing the clock back holds nothing |
| rollback | a commit that rolls back restores held firings, deadlines and caps, and does not run a spent firing again |

**Hosts, alone.** Each host's observation logic is also tested against scripted
events, so its edges do not depend on a browser or a device producing them:

- The web's scroll activity: a scroller that stands still for 1,200 ms of frames and
  then moves leaves at 1,000 ms and comes back with its next `scroll`; without
  `scrollend` support it leaves 100 ms after its last `scroll`; a `pointercancel`
  while the touch stays down keeps the hold until `touchend`; two touches, one
  lifting; a lift during deceleration keeps the movement; a `scrollend` from an
  earlier scroll does not end a newer hold; a nested scroller handing off to its
  parent; an instant replacement that moves nothing (left by quiet) and one that goes
  on (left by its `scrollend`).
- The web's off-screen report: before the first observation, a visible page is not
  reported off screen and a hidden page is; a delayed observation on return to view
  changes admission only when delivered; a scroll report delivered after an admission
  does not retroactively change that decision.
- The web's animations: pausing an animation or setting its rate to 0 removes that
  obstacle at the next scheduled 100 ms check; admission occurs only if the other idle
  terms also hold. Replacement of an animation, or creation of a frame task while an
  idle callback is queued, is reflected in the admission-time sample.
- `update` demand on each host: the first check uses the demand observed at that
  check; a deadline equal to an eligible callback's cutoff can run in that callback; a
  callback requested only to check waits is not demand; observed demand ending before
  the callback permits immediate admission.
- The idle condition: each of D2's five terms independently prevents admission; no
  runner deadline (`None`) imposes no deadline restriction; a deadline exactly 1000/60
  ms ahead prevents admission, while one just beyond the horizon does not. With two
  held idle firings, the host applies the first batch and resamples before attempting
  the second. Work armed by the first prevents the second's admission until the full
  condition holds again.
- The clock generation: callbacks queued before a takeover admit nothing.

**Hosts, against the real platform.** Each host's suite drives real input and reads
the runner's commits with what the host observed at each:

- The JS web target, in a Playwright suite (`suiteBrowser`) on Chromium, Firefox and
  WebKit: a `scrollend` firing is held through a smooth scroll and runs after its
  `scrollend`; it is held through a touch drag that holds still (Chromium's CDP touch
  input); it runs when a pointer pressed on a scroller lifts without scrolling it; an
  instant replacement of a smooth scroll does not itself end movement tracking: the
  firing runs after observed `scrollend` if movement continues, or after quiet
  completion if the browser stops silently; a scroller removed mid-scroll stops
  holding it; a cross-origin iframe scrolled out of view releases it, while one partly
  scrolled out of view, with a fling inside it, does not, and neither does a `scroll
  document` page flung more than a screen; `idle` waits for a running CSS animation's
  `finished` and ignores one at playback rate 0; `update` runs in Exact's frame loop
  and asks for no frame during a CSS animation alone.
- Apple, in UIKit XCTest classes with real touches through the touch runner: a
  `scrollend` firing is held through a drag, a drag held still and its deceleration,
  and runs at the end; a stationary finger holds the firing until it lifts, including
  on a scroller that cannot move; a touch won by a tap recognizer ends with a check;
  with two fingers down, lifting the first keeps the hold and lifting the last permits
  admission when movement has also ended; it runs when the app goes to the background
  mid-deceleration, which the test makes from the host (`simctl launch` of another
  app), since XCUITest's home button did not background the app on the iOS 27.1
  simulator (§5); a cap passes while a finger tracks; `idle` waits through a
  deceleration and for a job posted to the owner thread just before the observer runs;
  `update` uses a passive `UIUpdateLink` callback during scrolling on iOS 18 and
  later; it admits immediately when the link has reported no update within 1000/60 ms
  and neither Exact frame work nor observed scroll activity supplies demand; the iOS
  17 path through the display link. The touch runner is an XCUITest, whose agent in
  the app makes UI updates of its own on a still screen (§5), so the still-screen case
  asserts on what the link saw, not on the screen being still.
- Android, on the `exact_fold` emulator: a fling holds a `scrollend` firing through
  the main thread's reports and releases it at the fling's end; `idle` waits for the
  report that the last publication was fully applied, deferred moves included; a
  delayed report; a reader replaced mid-session counts as busy until it reports;
  animation sampling holds `idle`.
- The terminal and the painted Linux and Windows hosts: their wait points in their
  existing host tests, `scrollend` at the deadline on the terminal.

## 3. Not proposed

**Waiting by default.** An app may need a precise timer, and a default that moves an
action into a later frame would change what existing apps do.

**Predicting a timer's cost.** Running `update` only when the frame has room for the
timer's last batches needs a release per timer and a measurement per batch. When frame
demand exists, `update` admits work in a frame callback. Otherwise it admits
immediately. Neither case promises that the batch fits in a frame.

**Physical stillness.** A host cannot know that nothing moves on screen, only what it
has been told; D2 defines the waits by that and names the limits, rather than claiming
more.

**Tolerance for power.** Letting the system coalesce wakes, through Apple's
`Timer.tolerance`, Windows' `SetCoalescableTimer` or Linux's timer slack, is a
separate change with its own measurements. Mapping a waiting firing's deadline-to-cap
window to such an API is outside this RFC's implementation plan.

**Combined waits** (`wait=scrollend` then `frame`), **computed waits**, and **per-node
or per-scroller waits** (wait for this list's scroll only).

**Background wakeups.** A timer does not run while the platform suspends the app, and
nothing here changes that.

## 4. Plan

1. Contract syntax and lowering; plan fields and validation; runner holding, caps,
   turns and takeover; `Runner::hold_waits`, `Runner::waits`, `Runner::release` and
   the extended `Runner::frame`; and D10's compiler, plan, semantics and runner tests.
   Hosts still declare no supported waits, so live timers retain precise scheduling.
2. The JS web target and Apple, with D10's host tests. Implement `frame`, `scrollend`
   and `idle` on their supported versions. Implement `update` first through
   `UIUpdateLink` on iOS and tvOS 18 and later, then through D7's JS and older-Apple
   paths.
3. Android, Linux, Windows, the terminal and the wasm web host.
4. Docs, `state.tasks` and `perf`.

Each stage runs the six checks and, since it touches `runner/` and `contract/`,
`contract-difftest quick`.

Before stage 2 lands, the change is measured on an iPhone, in an app whose tick
changes the scrolled list: flings across a tick's deadline with the timer precise,
then with `wait=scrollend` and `wait=update`, comparing late frames, timer admission
points and the measured runtime and application work associated with those frames. LLP
1144's fling report can measure it once its platform timing runs the app's timers on
the wall's clock, which on Apple it does not (D8); until then the measurement runs
without the agent, as `experiments/timer-wait-points/compiled/fling-free.mjs` does.

## 5. Measured

The probes in `experiments/timer-wait-points/` measured the platform behavior used by
D2 and D7 on 2026-10-27, Pacific time. The directory's `README.md` gives the methods.
The browser and plain UIKit probes isolate platform behavior; the compiled Messages
probe measures Exact's existing timer scheduling. These experiments supply the
evidence below. D10 defines regression tests for the proposed implementation.

**Browsers**, through Playwright on Chromium 156.0.8078.4, Firefox 157.0 and WebKit
27.2 (`web/probe.mjs`, `web/results/<engine>.json`):

| What | Chromium | Firefox | WebKit | Used in |
|---|---|---|---|---|
| a smooth scroll's `scroll` event to the next animation frame | 0–0.1 ms | 0–1 ms | 0 ms | D7: checking scrolling in an animation frame |
| `scrollend` for a scroller removed at its first `scroll` | none | none | none | D2, D7: a removed scroller leaves the set |
| a CSS animation at playback rate 0 | `running` | `running` | `running` | D7: rate 0 does not count |
| a scroll-driven animation | `running`, on a `ScrollTimeline` | unsupported | `running`, on a `ScrollTimeline` | D7: the document timeline only |
| how an `Element.animate()` animation announces its end | its own events and `finished`, no document event | same | same | D7: `finished`, with `allSettled` for a cancel |
| `requestIdleCallback` while an animation-frame loop runs | about 60 a second, reported budgets 8.8–16.3 ms | about 61 a second, 4.1–16.1 ms | not available | D2: the callback alone does not establish Exact's idle condition |
| an offscreen cross-origin iframe, for 2 s: animation frames; 100 ms timeouts; an `IntersectionObserver` on its root, on a fixed 1 px corner element and on a fixed element covering the viewport; `visibilityState` | 0; about every 110 ms; none intersecting; `visible` | 2; about every 110 ms; none intersecting; `visible` | 0; up to 176 ms apart; none intersecting; `visible` | D7: intersection supplies the off-screen report when page visibility remains `visible` |
| the same iframe with its top 100 px of 300 scrolled out of view | 120 frames; root and covering element intersecting, corner element not | 121 frames; same | 87 frames; same | D7: observe the whole viewport |
| a page laid out as `scroll document`, scrolled 2,000 px, rendering on; the same in an on-screen iframe scrolled inside | root not intersecting; corner and covering elements intersecting | same | same | D7: not the root |
| a touch drag held still for 623 ms, then lifted | no `scrollend` while held; one 26 ms after the lift | (no touch input) | (no touch input) | D2: a held finger has not ended its gesture |
| a smooth scroll that an instant one to its current position replaces | stops; no `scrollend` | goes on; one `scrollend` | stops; no `scrollend` | D2, D7: quiet ends it; a host scroll command does not |
| the longest a smooth scroll held one position after its first `scroll` and before it moved again, in 10 runs (load average 391–425 on 18 cores) | 0 ms; a touch fling after the lift, 34–35 ms | 34–68 ms | 0–4 ms | D2, D7: the 1,000 ms quiet |
| the last movement of a smooth scroll or a touch fling to its `scrollend` | 29–50 ms | 76–101 ms | 0–23 ms | D7 |
| the main thread blocked 1.2 s at a smooth scroll's first event | the requested animation-frame callback ran after 1,200.2 ms; `scrollend` arrived 0.8 ms after the block | 1,268 ms; 1 ms after the block | 1,200 ms; 1 ms after the block | D7: a callback gap alone does not establish scroll completion |

**UIKit**, a plain app with no Exact code on an iPhone Duo simulator, iOS 27.1, Xcode
27.1, at 60 Hz, with gestures from XCUITest. Each scenario ran three times except
`scrollState`, which ran five times and attempted two background switches per run. The
update-link control also ran three times without XCUITest (`ios/`, `ios/results/`):

| What | Measured | Used in |
|---|---|---|
| a passive `UIUpdateLink`'s `afterUpdateScheduled` calls | none during the initial approximately 2.1 s still interval without XCUITest, 4 with it; 60 during a drag lasting 987–993 ms, 31 during the 517 ms held interval, and 146 during 2,433 ms of deceleration; without XCUITest, 2 during each tested fade, at its start and end; with XCUITest, 4 during `UIView.animate` and 2 during `UIViewPropertyAnimator`; none during each run's 30 no-op `Timer` and `asyncAfter` callbacks, or during the second after the fling ended | D2, D7: observed update demand |
| the order within an update | in each of the three runs, `afterUpdateScheduled` preceded the first touch in all 56 touch-bearing held-drag updates and preceded `scrollViewDidScroll` in all 140 scroll-bearing deceleration updates; no observed touch or scroll callback in these windows was outside an update | D7: the selected update phase precedes input dispatch |
| setting `requiresContinuousUpdates` on a link with no action | crashes the app, whether true or false | D7 |
| a `beforeWaiting` observer in the default mode | called 20–23 times before the drag's pan was recognized, then silent during recognized dragging, the held interval and deceleration; called again 0.3–0.7 ms after deceleration ended. During the three deceleration windows, the common-mode observer ran 459, 445 and 456 times; the common-mode 100 ms timer fired 25, 25 and 24 times. The default-mode timer fired once in the first window, about 4.9 ms after release before tracking mode resumed, and zero times in the other two | D7: the observer supplies an opportunity, while the full condition governs admission |
| scroll state across a fling, a held drag and a switch to another app | a held finger: `isTracking` and `isDragging` true, `isDecelerating` false, no scroll callbacks, and the lift gives no deceleration; `isDragging` stays true through a deceleration; in 5 of 10 switches the app reached the background mid-deceleration, which went on (up to 73 scroll callbacks there) and ended there, 2,446–2,617 ms after the release | D2: a held finger; off screen |
| a `CADisplayLink` at the default rate | 60 Hz still and during a fling; the simulator reports a 60 Hz maximum | D3 |

The simulator says nothing about ProMotion rates or a device's power states, and the
app held a background task for 2 s to keep sampling there, so a suspended app is
unmeasured.

**Exact's Messages**, compiled at `b0ce559698`, on the same simulator, flung through
the touch runner (`compiled/`, whose `README.md` has the method). Under the agent, the
runner's clock stood at 12,719 ms through 5.4 s that included a real fling, so LLP
1144's fling report sees no timer batch (D8). Without the agent, the three runs
supplied 22 flings and 1,970 sampled frames in the 1.5 s window following each touch
gesture. `every(250, poll)` ran six times in each window, 132 times in total. Every
sampled beat shared a display-link batch with the frame task's commit, and each beat's
own frame was on time.

Five frames in those windows were late. Four immediately followed a beat's frame and
waited 14.6–86.7 ms on the runtime. Across the sampled traces from the full runs,
including setup and rest but excluding intervals over 300 ms attributed to trace
saving, 30 of 46 late frames followed a beat's frame.

Each beat re-evaluated `bubble.contract:76` once per message: 153 messages in the
first run and 453 in the other two. Across the intervals between saved traces, 2 of
134,249 evaluations changed a value. The Mac's load average ranged from 150 to 700, so
these timings do not establish device performance or causation. The observed admission
locations and evaluation counts provide the baseline for §4.

## Open questions

1. **Measurements of `idle`'s default bound.** D3 selects the larger of the interval
   and 1,000 ms. This gives short timers a meaningful opportunity to wait. For a
   repeat of at least 1,000 ms, the default waiting bound spans one interval; for a
   shorter repeat it can span several beats. Delayed execution and callback duration
   can merge further beats under D4. The remaining question is whether consumer
   measurements justify revisiting this default or requiring an explicit `max=`.
2. **Device measurements of the 100 ms bound.** D3 selects 100 ms, equal to six frame
   periods at 60 Hz or three at 30 Hz. The probes do not establish how often eligible
   callbacks are more than 100 ms apart on a visible physical device, including under
   reduced refresh rates or power-saving modes. Such a delay ends waiting at the cap;
   it does not violate the admission rule.
