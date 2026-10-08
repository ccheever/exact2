# LLP 1113: Settled, and the two clocks

**Type:** RFC
**Status:** Draft r4 (2026-10-08): round 3 by Astra (max) and Grok 4.7 (xhigh), both NOT READY; r4 drops mid-test sampling and seeks a final capture until settled (Charlie, 2026-10-08: "yeah sounds good do all that"). r3 split final and mid-test captures ("sure do both"); r2 made a capture a sample ("ok sounds reasonable, go ahead"). Reviews: `llp/reviews/1113-settled-and-the-two-clocks.{astra,grok}.md`; dispositions in §7.
**Renumbered (2026-10-08):** drafted and reviewed as LLP 1107, a number origin had meanwhile given another document; its reviews keep the old numbers (1105→1111, 1106→1112, 1107→1113). r1–r3's code citations were checked at `2ce193976`; r4's are re-checked at `ce31a47b6`, 542 commits later.
**Systems:** the runner (`runner/src/runner/commit.rs`, `queue.rs`, `settlement.rs`: timers off while settling, an invocation's work), the Linux host (`host/linux/src/host.rs`, `presenter*`, `agent.rs`: the settle flag, the settle procedure, the final capture), the agent driver (`scripts/agent-test.mjs`: the final capture uses up the session), the Apple app owner (`ExactApp`: a new tool-call lock), the terminal host (cited, unchanged)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07; r2–r4 2026-10-08
**Implementer:** none yet. This document proposes; it specifies nothing until it has one.
**Related:** LLP 1012 §2 (the agent's clock: `clock` moves both clocks to one instant; unamended), LLP 1012.000 (a wait that does not move the clock), LLP 1075.003.000.001 §2.4–2.5 (hatches: drained inside `clock settle` since `2ce193976`), LLP 1101.003 D2 (print mode's fixpoint: the command-line column's authority), LLP 1111 D4 (a tool call's outcomes), LLP 1112 (pictures; its D1a is what D3 replaces), LLP 1055 D10 (an infinite animation never settles), LLP 1092 (queues and `next`), LLP 1097 (background storage work).

## Summary

The runner clock fires timers and stamps actions; the presentation clock
samples motion. Under the agent's clock the two stay equal, as they are
today, and nothing in this RFC parts them while anything can still look.

- **A final capture seeks until settled** (D1): `exact render`'s picture, or
  a `--test` screenshot that is the test's last step. The session is sought
  through today's presenter to its settle time, settled again, sought again
  while motion lies ahead, and painted. It is the session's last observer.
- **A mid-test capture paints now** (D1): a `--test` screenshot with steps
  after it is today's `screenshot`, plus a `motion` field. An author who
  wants settled pixels mid-test puts `clock settle` before it.

D2: no timers fire while settling. D3: the procedure (LLP 1112 cites it). D4:
what a tool call owns (LLP 1111). D5: the modes (LLP 1101.003 D2 rules its own).

## 0. Why this is its own document

Three review rounds of LLP 1101.003, 1111 and 1112 found one rule written three
ways, each fix leaving the others wrong. Three rounds of this one found that
only a capture with no "after" can move the session safely.

## D1 — One clock; a final capture seeks until settled, a mid-test capture paints now

**The two roles.** The runner clock (`Runner::now_ms`, `runner.rs:1199`)
drives timers, `then_due`, queue `next`, an action's `now()` and every agent
reply's `clock`. The presentation clock (`Host::now_ms`, `host.rs:98`) is
where the engine was last sought (`Host::tick`, `host.rs:1189-1192`). They are
roles, not always fields: the terminal host has no engine, and the JS agent
tags replies from `exact.clock.now` (`host/web-js/agent.js:83`). Under the
agent's clock they stay equal today: `clock` advances the runner and ticks the
host to where it landed (`presenter/clock.rs:8-31`), `Host::dispatch_at`
moves both to `max(argument, now)` (`host.rs:833`, `:846`), and `screenshot`
paints without seeking (`presenter.rs:1430`).

***t_s*** is `settle()` (`host/linux/src/agent.rs:755-760`), milliseconds:
the host's `settle` op (engine settle ×1000 and press settle,
`host.rs:684-692`) and `group_settles_at` (the ghost, `presenter/group.rs:428`).
`None` means nothing finite is in flight, and *t_s* is now.

**A final capture** is one nothing after it can observe: `exact render`'s
picture, or a `--test` screenshot that is the test's last step. It runs D3
under the final-capture column (D5), which seeks with `Presenter::tick(t_s)`
(`presenter.rs:1416-1427`). `Host::tick` sets the host clock to
`max(t_s, now)` and advances the engine to that over 1000
(`host.rs:1190-1191`), so a *t_s* behind `engine.now` samples at now and the
backwards check (`motion/src/engine.rs:586-591`) cannot trip. The real
presenter then clamps scroll, queues collections, refreshes transform
geometry, marks hatches stale and runs `tick_arrange`; press reads
`host.now_ms`, and paint motion goes through `present_paint`. The runner clock
does not move, so the clocks part; nothing observes it, because **a final
capture uses up the session**:

- **Last observer.** For `--test` the driver runs every post-step read first
  (the counted-fault check, `unfired`, `scripts/agent-test.mjs:206-214`,
  called at `:344`), then sends `screenshot` with `final: true`.
- **Nothing after.** The driver refuses every later operation (`refused:
  "after final capture"`) and closes. A failure after the first real seek
  still uses up the session; the reply is D3's `error`.
- **Teardown after encoding.** `pointer_cancel` (`agent.rs:53`; a cancel is
  an up and may deliver an authored `pointerup`, `presenter/contact.rs:791-806`)
  and `teardown::finish` (`agent.rs:54`, `teardown.rs:14`) run after the PNG
  is written and cannot change it.

**A mid-test capture paints now.** A `--test` screenshot with steps after it
is today's `screenshot`: no seek, no sample, no settle. Its reply adds
`motion: "in-flight"` when `Host::motion()` (`host.rs:628-630`: the engine is
not quiescent, or a press is settling) or `group_needs_frame`
(`presenter/group.rs:443`) is true (an infinite animation included), else
`"none"`. The clocks and engine are untouched, so LLP 1012 §2 needs no
amendment. Settled pixels mid-test are `clock settle` then `screenshot`, which
already works and fires the timers it crosses, as LLP 1012 intends.

## D2 — No implicit timers while settling

With the clocks equal, `advance_timed(now)` still fires a timer due at exactly
`now` (`next_ms <= now_ms`, `commit.rs:328`; a refusal leaves a sibling due,
`:204-212`). So while a `settling` flag is set, **`Host::dispatch_at`
itself** (`host.rs:825-849`) dispatches with timers off: `land_then`
(`advance_within(now, false, false)`, `commit.rs:289-291`), then the event,
the runner clock unmoved. A due `then` and a queue's due `next` still run;
they are due already, not timers. One rule in `dispatch_at` covers every
caller: transform geometry (`transform_geometry.rs:183`), resize delivery
(`:224`), authored scroll events (`presenter/collection.rs:494`, `:709`),
`focus`/`blur` (`presenter/events.rs:95`), hover (`:267`), hatch dispatches
(`presenter/hatch.rs:525`), surface messages (`surfaces.rs:865`, `:946`) and
host-command callbacks.

**The flag's span** is `agent::handle` (`agent.rs:59-95`), for a final
capture only: set on entry and cleared after `first_pixel` returns (`:93`). It
therefore covers `poll_images` (`:61`), `pump` (`:66`, which delivers authored
scroll events at once, `presenter.rs:1306`), `poll_update`,
`poll_development`, `run_commands`, `sync_surfaces` (`:69-72`), the hatch
turn (`:76-77`), `answer` (`:78`), the epilogue (`:79-92`) and `first_pixel`.
For every other operation, a mid-test screenshot included, nothing changes: a
`tap` runs `dispatch_at` and fires what is due, as LLP 1012 says.

## D3 — The settle procedure

Steps are cited by name: **activation**, **data pass**, **seek**,
**frame**, **images**, **ends**. A mode (D5) selects which run.

**Activation.** Where the host defers a data module past first pixel: paint,
call `first_pixel`, and wait while `data_activating()`
(`presenter/delivery.rs:119-121`, now false once `activation_failed` is set).
The step reads `activation_failed` (`presenter/display_frame.rs:289-308`)
itself; a failed activation ends `error`, `step: "activation"`.

**One pass** (the final-capture column; LLP 1112 cites it):

```text
inputs:   deadline; the session; the settling flag is set (D2)
data pass:
  pump(host.now())           replies, announcements, authored scroll events
  land_then                  due then, queue next
  run_commands               host commands and their callbacks
  settle_collections; sync_surfaces
  hatch_moments; while hatch_in_flight() > 0: hatch_turn   (as clock settle, agent.rs:888-903)
seek:     t = settle(); if t > host.now(): Presenter::tick(t)   (work)
frame:    if dirty: frame(); follow_pointer()     hover, inside the loop
images:   w = wait_images(remaining); if w: frame() again
work:     a session change from pump, land_then, commands, collections, hatches,
          a seek, hover, or an image report; a paint is never work
quiet:    !has_pending                (commit.rs:1148-1154; device holds stay outside)
          && no due then or queue next
          && no queued host command, scroll event or announcement
          && !collection.pending()
          && no node with accessibilityBusy   (a whole-tree walk; new)
          && !w and the loader's pending() is false   (image.rs:459)
          && !(settle() > host.now())         no finite target ahead
```

Passes repeat until one is quiet with no work. The seek comes after the data
pass so a reply's motion is in *t_s*, and before the frame so the frame shows
it; a seek that starts motion (`Host::tick` lays out height, and
`observe_layout` can start a `Layout` curve at that instant) makes the next
pass seek again, as `clock_within` does (`agent.rs:946-981`). A pass that is
quiet except for `accessibilityBusy` sleeps 20 ms, pumps announcements and
polls again; polls do not spend the budget. Every pass that did work spends
one, and the deadline is checked before each step.

**The final observation point** is the end of the first quiet pass: every
callback the capture caused has run, the last `frame()` reflects them, and
`last_frame_succeeded` (`content_region/presenter.rs:153`) is true. Those
pixels are encoded; nothing paints after them.

**Errors stop the procedure.** These log and continue today, and each returns
its error to the pass instead (new work): `wait_for_replies`
(`agent.rs:1033-1044`), `Presenter::tick`'s geometry callback
(`presenter.rs:1421-1423`), `frame()`'s refinement, `apply_reports`
(`presenter/images.rs:18`), `run_commands`, and nested callbacks such as
`focus_command`'s (`presenter/events.rs:63`).

**Ends:** `complete` (the final observation point); `deadline`, with
`reason` the step still working (`data`, `requests`, `hatches`, `motion`,
`busy`, `images`; an image held by `QueueFull` or `Budget` keeps the loader
pending, `image.rs:482`); `exhausted`, after sixteen work-producing passes (a
budget, not proof of a cycle; `land_then` drains synchronous `then` chains in
one call up to `TIMER_FIRE_LIMIT`, `runner.rs:503`, so only work awaiting a
later pump spends passes); and `error`, `{settled: "error", step, message}`,
including `activation`, and `paint` when `last_frame_succeeded` is false
(`content_region/presenter.rs:153-180`, which otherwise returns retained or
white pixels).

*Publishable* stays LLP 1112's, judged on top of `complete`.

## D4 — What an invocation owns

A tool call (LLP 1111) does not settle the whole tree, because the person's
own work would count. It owns a set of work and **completes when the set is
empty**:

- **Ids.** A commit carries an invocation id, copied at `enqueue`
  (`commit.rs:986`), `arm_then` (`:539`), `wait_turn` (`queue.rs:111`), the
  `unsent` pushes (`commit.rs:661`, `:758`) and `force_refresh` (`:975`); a
  queue entry gives its owner to the commit it runs, and a forced refresh kept
  in `refresh_next` (`settlement.rs:787`) keeps its owner and counts.
- **Fulfilment.** The id is read off the pending entry before `remove`
  (`commit.rs:1236`) and stamps the fulfilment commit, so the resource re-asks
  it enqueues (`settlement.rs:771-779`), such as Fieldnotes' `library` after
  `addNote`, are owned.
- **Completion** is observed after a commit and its `arm_then`/`arm_next`
  stages return (`commit.rs:1242-1243`), never between `remove` and `arm_then`.
- **A kept ticket.** An owned commit that retargets an unowned in-flight
  ticket (`commit.rs:995-1013`) does not adopt it; the call ends with LLP
  1111's kept-ticket outcome. An unowned commit that retargets an owned ticket,
  or overwrites an owned `then_due` slot (`commit.rs:539-545`), ends the call
  `superseded`.
- **Streams** are refused when scheduled. A stream stays in `pending` after
  its first message, no longer in flight (`runner.rs:301-304`).
- **Background work.** The runner cannot attribute it: `arm_background`
  (`background.rs:27`, called from `take_requests`, `commit.rs:1072-1074`)
  polls module-wide work, the source method carries no invocation
  (`source.rs:293`), and the JS executor reads only aggregate state
  (`js/src/background.rs:30-35`). v1 needs no attribution. "Armed" means an
  outstanding runner background ticket (`background_ticket()`,
  `background.rs:65`): a call is refused (`busy`) while one exists, and fails
  if one appears during it.
- **Outside the set:** timers, frame tasks and announcements the call arms.
  A `then` continues an answer; a timer is the app's own schedule (`after`
  takes at least 1 ms, `contract/lower/src/timers.rs:34`).
- **v1:** a call is refused (`busy`) while the data module is not ready, so
  `unsent` is empty. Calls are serialized by a new lock, one per `ExactApp`
  (the class at `host/apple/Sources/ExactKit/Session.swift:130`), across every
  session in the process, consent included. It stays held after a timeout
  while owned work survives, until the set drains or the session closes. The
  runner holds ids only.

## D5 — The modes

| | Picture | Test, final capture | Test, mid-test capture | Command line | Tool call | `clock data` | `clock settle` |
|---|---|---|---|---|---|---|---|
| Authority | D3 | D3 | today's `screenshot` | LLP 1101.003 D2 | D4 | today (unchanged) | today (unchanged) |
| Pass | D3's | D3's | none | 1101.003 D2's fixpoint | none (the app's own) | `land_then` | advance to settle time |
| Activation | yes | yes | no | per 1101.003 D2 | refused until ready | yes | no |
| Timers | none fire | none fire | as today | per 1101.003 D2 | the app's, live | none fire | fire to the target |
| Motion | sought until settled | sought until settled | not sought; `motion` reported | none (no engine) | not waited | not sought | sought, and world `settleAt` (`agent.rs:950-957`) |
| Frame | in the loop, hover after | in the loop, hover after | today's `frame()` | per 1101.003 D2 (lays out per commit; no frame step) | not waited | no | no |
| Images | waited | waited | as painted | per 1101.003 D2 | not waited | no | no |
| Budget | 16 work passes | 16 work passes | none | per 1101.003 D2 | none | 16 | 16, plus 16 hatch drains |
| Bound and end | caller's deadline (LLP 1112 D11) → D3 ends | fresh, then the session is used up | none | per 1101.003 D2 | 10 s from dispatch | 20 s, `settled: false` | 20 s; `reason` `"world"`, `"requests"`, `"hatches"` or `"device"` (`agent.rs:895-981`) |
| After it | process ends | driver refuses every operation | session unchanged | process ends | the app runs on | today | today |
| Reply | D3 ends | D3 ends | today's, plus `motion` | per 1101.003 D2 | LLP 1111 outcome | `{clock, settled}` | `{clock, settled, reason}` |
| Reply clock | runner | runner | runner (= presentation) | per 1101.003 D2 | n/a | runner | runner |

## What changes, where

| Where | Change |
|---|---|
| `runner/src/runner/commit.rs`, `queue.rs`, `settlement.rs` | timers-off dispatch (D2); invocation ids, deferred-refresh owners, fulfilment capture, the outstanding-set query (D4) |
| `host/linux/src/host.rs`, `presenter*` | the `settling` flag in `Host::dispatch_at` (D2); helpers return errors (D3); a whole-tree `accessibilityBusy` walk (D3) |
| `host/linux/src/agent.rs` | one settle function for the picture and final-capture columns, with the seek loop and hover inside (D3); the flag over `handle` (D2); `motion` on the mid-test reply (D1); `clock data` and `clock settle` untouched |
| `scripts/agent-test.mjs`, `agent.mjs` | post-step reads before a final screenshot, `final: true`, and refusal of every later operation (D1) |
| `ExactKit` | the per-`ExactApp` tool-call lock (D4) |
| Tests | a seek that starts a sibling's `Layout` curve is sought again before the picture; hover over settled geometry that starts a request is waited for; a final screenshot runs the counted-fault check first and refuses a later step; a failure after the first seek still refuses later operations; a mid-test screenshot during a transition reports `motion: "in-flight"`, and `clock +100`, `tap` and `clock data` after it behave as with no capture; two timers due at one time with the first refusing fire nothing during a settle; a pre-op scroll event with a due timer fires nothing; a `then` issuing `focus` whose callback starts a request is waited for; 17 request round-trips end `exhausted`; a failed activation and a failed paint end `error`; Fieldnotes' `addNote` completes after `library`; a call while a background ticket exists is refused |

No motion-engine change: r3's `sample_at`, `Clone` derives and `infinite()`
are gone; an infinite-only scene has `settle()` `None`, so *t_s* is now.

## Not in this RFC

**Isolated sampling (later).** A mid-test capture that shows settled motion
without touching the session needs a scratch kernel and presenter, or at least
a capture-local painting context. Rounds 2 and 3 (Astra N1–N3, R3-3–R3-4; Grok
round 2's blockers and round 3's "the scratch paint has nowhere to put row
pixels" and "paint colours are not a read of the engine clone") showed what it
must isolate: the row cache (`row_valid` does not check opacity, colour or
offset, so a sampled row would be replayed by the next real frame); `frame()`'s
post-paint writes (scroll, `flow_damage`, `dirty`, `queue_collections`, action
capture); `present_paint`'s inheritance and `currentcolor` walk; `Layout`'s
surface scale and clipping; `symbol:` images; collection refinement for sampled
geometry; layout's writes and the image loader's demand.

Also out: superseded-outcome names (LLP 1111); publishable, fonts, strict
refusal (LLP 1112); print mode's loop and exits (LLP 1101.003); frame pacing
(LLP 1073); the Apple and web settle functions, built with their first consumer.

## Questions for Charlie

1. A work budget of 16 passes for pictures and final captures (D3)?
2. One new tool-call lock per `ExactApp`, all windows, consent included, held
   past a timeout until owned work drains (D4)?
3. Should a mid-test screenshot with `motion` in flight fail the test (D1)? r4 only reports it.

## 7. Review dispositions

**r1 and r2:** all taken; the review files record each.

**r3:**

| Concern | Disposition |
|---|---|
| A R3-1, G B1: one final seek is not a fixed point; *t_s* cited at the host helper, missing the ghost | **Taken:** seek inside the pass until no finite target lies ahead, each seek spending the budget; *t_s* is `settle()` at `agent.rs:755-760` (D1, D3) |
| G B2, A R3-3/R3-4: the scratch paint, row cache, `PaintMotion`, the live-to-isolated handoff | **Taken by removal:** mid-test sampling is gone; "Isolated sampling (later)" records what it needs |
| A R3-2: epilogue hover is outside completion | **Taken:** `frame()` and `follow_pointer` run inside the loop; one final observation point (D3) |
| A R3-5: the last step is not the last observer; teardown can deliver `pointerup` | **Taken:** post-step reads before the final screenshot; teardown after encoding; failure still uses up the session (D1) |
| G M: the flag's span misses `run_commands`, `sync_surfaces`, `first_pixel` | **Taken:** the whole of `handle`, re-verified at `ce31a47b6` (D2) |
| G M: a mid-test settle can change the engine; refusal predicates undefined | **Moot:** a mid-test capture neither settles nor refuses |
| A Q2: what "armed" means for background work | **Taken:** an outstanding runner background ticket (D4) |
| G minor: `infinite()` in the change table only; "read dirty after" | **Taken:** `infinite()` removed; the frame step runs `frame()` when dirty (D3) |
| A R3-6: `Layout` is not only translation; `PAINT` is eleven properties; the 16/17 wording | **Moot or taken:** the paint-only list is gone; "sixteen work-producing passes exhaust it" (D3) |
| G suggestions: busy poll pumps; citations | **Taken** (D3, D4) |

**Changed since `2ce193976`:** hatches (LLP 1075.003.000.001) are new —
`handle` runs a hatch turn and drains moments (`agent.rs:76-77`, `:85`), and
`clock settle` drains them and can end `reason: "hatches"` (`:888-903`), so D3
drains them and D5 lists the reason. `data_activating()` now checks
`activation_failed`. Most cited lines drifted by 10–60; all are corrected.
