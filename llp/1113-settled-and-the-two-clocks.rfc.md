# LLP 1113: Settled, and the two clocks

**Type:** RFC
**Status:** Draft r3 (2026-10-08): round 2 by Astra (max) and Grok 4.7 (xhigh), both NOT READY; r3 seeks for a final capture and samples paint-only motion mid-test, refusing otherwise (Charlie, 2026-10-08: "sure do both"). r2 made a capture a sample (Charlie: "ok sounds reasonable, go ahead"); both reviewers accepted that for the engine and found the presenter is not isolated. Reviews: `llp/reviews/1113-settled-and-the-two-clocks.{astra,grok}.md`; dispositions in §7 and §8.
**Renumbered (2026-10-08):** drafted and reviewed as LLP 1107, a number origin had meanwhile given another document; its reviews keep the old numbers (1105→1111, 1106→1112, 1107→1113). Code citations were checked at `2ce193976`, 542 commits behind the commit that adds this file.
**Systems:** the motion engine (`motion/src/engine.rs`: a side-effect-free sample), the runner (`runner/src/runner/commit.rs`, `queue.rs`: an invocation's work), the Linux host (`host/linux/src/host.rs`, `presenter*`, `agent.rs`: the settle flag, the settle procedure, the two captures), the Apple app owner (`ExactApp`: a new tool-call lock), the terminal host (cited, unchanged), the agent driver (refuses a step after a final capture)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07; r2 and r3 2026-10-08
**Implementer:** none yet. This document proposes; it specifies nothing until it has one.
**Related:** LLP 1012 §2 (the agent's clock: `clock` moves both clocks to one instant; unamended here), LLP 1012.000 (a wait that does not move the clock), LLP 1101.003 D2 (print mode's fixpoint: the command-line column's authority), LLP 1111 D4 (a tool call's outcomes), LLP 1112 (pictures; its D1a is what D3 replaces), LLP 1055 D10 (an infinite animation never settles), LLP 1092 (queues and `next`), LLP 1097 (background storage work).

## Summary

The runner clock fires timers and stamps actions; the presentation clock
samples motion. Under the agent's clock the two stay equal, as they are today.
A capture shows motion at its settle time *t_s* in one of two ways (D1):

- **A final capture seeks.** `exact render`'s picture, or a `--test`
  screenshot that is the test's last step: the session is sought to *t_s*
  through today's presenter, settled again, and painted. Nothing observes the
  session afterwards, so the clocks may part.
- **A mid-test capture samples.** A `--test` screenshot with steps after it
  paints paint-only motion from a clone of the engine and changes nothing in
  the session. Motion that a sample cannot paint without touching the session
  refuses the capture by name.

While a settle or capture runs, `dispatch_at` fires no timers (D2). D3 is the
settle procedure, D4 what a tool call owns, D5 the modes. LLP 1101.003 cites
D5 (its own D2 is the authority), LLP 1112 cites D3, LLP 1111 cites D4.

## 0. Why this is its own document

Three review rounds of LLP 1101.003, 1111 and 1112 (Astra and Grok) found one
rule written three ways, each fix leaving the others wrong: 1112 D1a's capture
clock, 1111 D4's ownership, and 1112's summary of 1101.003. One definition,
cited, ends that.

## D1 — One clock in agent mode; a final capture seeks, a mid-test capture samples

**The two roles.** The runner clock (`Runner::now_ms`, `runner.rs:1255`)
drives timers, `then_due`, queue `next`, an action's `now()`, and the `clock`
of every agent reply. The presentation clock (`Host::now_ms`, `host.rs:90`)
is where the motion engine was last sought (`Host::tick`, `host.rs:1175-1178`).
These are roles, not always two fields: the terminal host has no presentation
clock (no engine), and the JS agent samples animation from `exact.clock.now`
(`host/web-js/agent.js:197`).

**Today they stay equal under the agent's clock**: `clock` advances the
runner then ticks the host (`presenter/clock.rs:7-31`); `dispatch_at` sets
`now_ms = max(argument, now)` (`host.rs:859`, `:873`); `screenshot` seeks
nothing (`presenter.rs:1412`).

***t_s*.** `settle()` (`host.rs:654-661`) in milliseconds: the engine's finite
settle time ×1000, press settle and `group_settles_at`. `None` means *t_s* is
now. The engine takes seconds: every seek or sample uses `t_s / 1000`, and a
*t_s* behind `engine.now` (a ghost whose rest has passed while it is still
`Landing`) samples at `engine.now`, so the engine's backwards check
(`engine.rs:553`) cannot trip.

**A final capture seeks.** A capture is final when nothing after it can
observe the session: `exact render`'s picture, or a `--test` screenshot with
no later step. It runs `Presenter::tick(t_s)` (`presenter.rs:1399-1409`),
which seeks the engine and the host clock, clamps scroll, queues collections,
refreshes transform geometry and runs `tick_arrange` (the reorder ghost). Press
feedback then reads `host.now_ms` (`host.rs:628-631`), and paint motion goes
through `present_paint`. So layout, press, the ghost, paint colours, image
visibility and collection refinement all follow the real presenter. The D3
loop then runs again until quiet, and the real `frame()` is the picture. The
runner clock does not move, so the clocks part; nothing follows, so nothing
observes it. The driver refuses any step after a final capture
(`refused: "after final capture"`); a test that needs one takes a mid-test
capture instead.

**A mid-test capture samples.** A `--test` screenshot with steps after it:

- `sample = engine.sample_at(t_s / 1000)`, a clone advanced to that time.
  `advance` is a seek whose result depends only on `t` (`engine.rs:520-547`).
  New work: `#[derive(Clone)]` on `Engine` (`engine.rs:239`), `Clocks` and
  `Joining` (`engine/clock.rs:24`, `:33`) and `Timelines`
  (`engine/timeline.rs:39`); `Slot`, `Transitions`, `AnimationPlay` and the
  velocity tracker already derive it. The clone is made once per sampled pass
  (D3 repeats it after image work), each a copy of the slot and animation maps.
- **Paint-only** is what `Host::present` applies without a layout
  (`host.rs:1343-1380`): `Translate`, `Scale`, `Rotate`, `Opacity`, `Layout`
  (drawn as a translate of the presented box, `paint.rs:891-892`), the SVG
  geometry (`R`, `StrokeDashoffset`, `Cx`, `Cy`, `X`, `Y`, `Rx`, `Ry`) and
  the eleven `Property::PAINT` colours (`motion/src/property.rs:128`). The
  overlay is a copy of `host.presented` with the sample's frame applied, plus
  a `PaintMotion` read from the clone, never `present_paint` on the live
  engine (`host.rs:1355-1357`, `paint_motion.rs:81-113`). Press feedback is
  sampled too: `PressFeedback::factor(t)` is pure (`press.rs:20-22`), so the
  overlay reads `factor(t_s)` and retires nothing.
- **Refused by name**, `{capture: "refused", reason}`, when the sample cannot
  paint without touching the session:
  - `height`: a `Height` transition in flight. Height reaches pixels only
    through `layout` (`host.rs:1350-1351`), which writes kernel geometry,
    damage, perf counts and the engine's `played` set (`height.rs:100-158`,
    `kernel/src/motion.rs:97-120`, `engine.rs:419-427`). It also covers a
    collection whose sampled geometry would differ, since only height changes
    a scrollport.
  - `ghost`: a reorder ghost not at rest. It moves only in `tick_arrange`,
    which a sample does not call.
  - `images`: the sampled frame shows an image not decoded for the live
    frame. A sample uses only images already decoded; `sync_visible` would
    cancel requests the live session holds (`image.rs:181`).
  The author adds a `clock settle` step before the screenshot, or makes it the
  test's last step.
- **The scratch frame.** A new `Presenter::frame_sampled(overlay)` paints the
  overlay into a capture buffer. It touches none of `take_row_dirty`,
  `sync_canvases`, `refine_collections`, `sync_images`, `last_frame_succeeded`,
  `boxes` or damage (`content_region/presenter.rs:49-231`, `presenter.rs:66`).
  Kept row pixmaps are bypassed for the sample (rows are invalidated in the
  capture's copy before the walk, `paint/rows.rs:119-163`); nothing is
  dirtied afterwards. It returns pixels and its own success.

**What stays the same after a capture.** After a mid-test capture, the clocks
and the session's engine are exactly as before, so the next `tap`,
`clock +N` or `clock data` behaves as today and LLP 1012 §2 needs no
amendment. The settle that preceded it may have changed the session in
ordinary ways: image intrinsics applied by `wait_images` stay
(`presenter/images.rs:18-23`, `host.rs:1022-1024`), as they would after any
`clock data`.

**Why the split.** r1 seeked every capture and left the runner behind; the
clocks never rejoined (`clock +100` refused as backwards, `agent.rs:814-815`).
r2 sampled every capture, and both round-2 reviews showed a full sampled frame
needs an isolated layout, image loader, press, ghost and paint motion. A final
capture has no "after", so it can seek; a mid-test capture keeps to what a
clone can paint and refuses the rest. The full sampled frame is in "Not in this
RFC".

## D2 — No implicit timers while settling

With the clocks equal, `advance_timed(now)` still fires a timer due at exactly
`now`: selection is `next_ms <= now_ms` (`commit.rs:319`), and a refusal stops
the advance at a due time with a sibling still due (`:468`). So while a
`settling` flag is set, **`Host::dispatch_at` itself** dispatches with timers
off: `land_then` (`advance_within(now, false, false)`, `commit.rs:285-287`),
then the event, the clock unmoved. It still runs a due `then` and a queue's due
`next` (`:339-345`); those are due already, not timers. Putting the rule in
`dispatch_at` covers every caller: transform geometry
(`transform_geometry.rs:183`), `deliver_resizes` (`:224`), collection scroll
events (`presenter/collection.rs:490`, `:705`), image reports' geometry
(`presenter/images.rs:29-31`), host-command callbacks, `focus`/`blur` through
`set_focus` (`presenter/events.rs:94`), and surface messages
(`surfaces.rs:775`, `:856`).

**The flag's span.** For an operation that settles or captures (a picture, a
test screenshot), `agent::handle` sets the flag before its prelude and clears it
after its epilogue. The prelude is `poll_images`, `pump` and the commands it
runs (`agent.rs:59-66`); `pump` delivers authored scroll events at once
(`presenter.rs:1289`), which reach `dispatch_at`, so r2's "the pre-op pump is
timer-free" was wrong. The epilogue is `sync_surfaces`, `run_commands`,
`frame()` and `follow_pointer`'s hover callbacks (`agent.rs:75-85`). An error
in any of them is the capture's error (D3). For every other operation nothing
changes: a `tap` runs `dispatch_at` and fires what is due, as LLP 1012 says.

## D3 — The settle procedure

Steps are cited by name: **activation**, **data pass**, **frame**, **images**,
**ends**. A mode (D5) selects which run.

**Activation.** Where the host defers a data module past first pixel: paint,
call `first_pixel`, and wait while `data_activating()` (`presenter/delivery.rs:117`).
That returns false on failure as well as success, so the step also reads
`activation_failed` (`presenter/display_frame.rs:293`); a failed activation
ends `error`, `step: "activation"`.

**One pass, specified:**

```text
inputs:   mode (D5), deadline, the session; the settling flag is set (D2)
clock:    unmoved, except a final capture's one seek (D1)
data pass:
  pump(host.now())           replies, buffered announcements, authored scroll events
  land_then                  due then, queue next
  run_commands               host commands and their callbacks
  refine and settle collections; sync_surfaces
frame:    the real frame() (final capture, or before it); frame_sampled (mid-test); read dirty after
images:   w = wait_images(remaining); frame again if w
work:     a session change from pump, land_then, commands, collections or an image report;
          a paint is never work
quiet:    !has_pending                (commit.rs:1129-1135; device holds stay outside)
          && no due then or queue next
          && no queued host command, scroll event or announcement
          && !collection.pending()
          && no node with accessibilityBusy   (a whole-tree walk; new)
          && w was false and the loader's pending() is false
```

Passes repeat until one is quiet with no work. A pass that is quiet except for
`accessibilityBusy` sleeps 20 ms and polls; polls do not spend the budget.
Errors stop the procedure. Today these log and continue: `wait_for_replies`
(`agent.rs:934-945`), `Presenter::tick`'s geometry callback, `frame()`'s
refinement (`content_region/presenter.rs:63`), `apply_reports`
(`presenter/images.rs:18-24`), `run_commands`, and nested callbacks such as
`focus_command`'s (`presenter/events.rs:61`, which prints and drops one). Each
returns its error to the pass instead (new work).

**Ends:**

- `complete`: a quiet pass with no work.
- `deadline`, with `reason` naming the step still working: `data`,
  `requests`, `busy` or `images`. An image held by `QueueFull` or `Budget`
  keeps the loader pending (`image.rs:432-436`) and ends here.
- `exhausted`: 16 passes that did work. It counts passes, so it is a work
  budget, not proof of a cycle. `land_then` drains a synchronous chain of
  `then`s within one call, up to `TIMER_FIRE_LIMIT` (`runner.rs:563`), so it
  takes 17 request round-trips, each answered on a later pump, to exhaust it.
- `error`: `{settled: "error", step, message}`, including `activation`, and
  `paint` when `last_frame_succeeded` is false
  (`content_region/presenter.rs:147-180`, which otherwise hands back retained
  or white pixels) or a sampled paint fails.
- A mid-test capture can also end `{capture: "refused", reason}` (D1).

*Publishable* stays LLP 1112's, judged on top of `complete`.

## D4 — What an invocation owns

A tool call (LLP 1111) does not settle the whole tree, because the person's
own work would count. It owns a set of work, and it **completes when that set
is empty**:

- **Ids.** A commit carries an invocation id. It is copied at `enqueue`
  (`commit.rs:1006`), `arm_then` (`:538`), `wait_turn` (`queue.rs:122`), the
  `unsent` pushes (`:657`, `:754`) and `force_refresh` (`:956`). A queue
  entry, when it runs, gives its owner to the commit it runs.
- **Deferred refreshes.** A forced refresh that settlement cannot ask yet is
  kept in `refresh_next` (`settlement.rs:768`). It keeps its owner and counts
  toward completion until it is asked.
- **Fulfilment.** The id is read off the pending entry before `remove`
  (`commit.rs:1217`) and stamps the fulfilment commit. So the resource
  re-asks it enqueues (`settlement.rs:755-764`), such as Fieldnotes'
  `library` after `addNote`, are owned.
- **Completion** is observed after a commit and its continuation-arming
  stages return, never in the gap between `remove` and `arm_then`.
- **A kept ticket.** An owned commit that retargets an unowned in-flight
  ticket (`commit.rs:980-995`) does not adopt it; the call ends with LLP
  1111's kept-ticket outcome. An unowned commit that retargets an owned
  ticket, or overwrites an owned `then_due` slot (`commit.rs:535-540`), ends
  the call `superseded`.
- **Streams** are refused when scheduled: a stream stays in `pending` after its
  first message (`runner.rs:301-305`).
- **Background work.** The runner cannot attribute it: `arm_background` polls
  module-wide work (`runner/src/runner/background.rs:27`), the source interface
  carries no invocation (`source.rs:252`), and the JS executor reports only
  aggregate state (`js/src/background.rs:35`). v1 is conservative and needs no
  attribution: a call is refused (`busy`) while any background work is armed,
  and fails if background work becomes armed during it.
- **Outside the set:** timers, frame tasks and announcements the call arms. A
  `then` continues an answer; a timer is the app's own schedule. (`after`
  takes at least 1 ms, `contract/lower/src/timers.rs:34`.)
- **v1:** a call is refused (`busy`) while the data module is not ready, so
  `unsent` is empty. Calls are serialized by a new lock, one per `ExactApp`
  (the class at `host/apple/Sources/ExactKit/Session.swift:139`), across every
  session in the process, consent included. The lock stays held after a
  timeout while owned work survives, until the owned set drains or the session
  closes. The runner holds ids only.

## D5 — The modes

Two kinds of pass. The **agent-clock pass** is D3's data pass (`land_then`,
timers off). The **wall-clock pass** is LLP 1101.003 D2's fixpoint. Each
column selects one.

| | Picture | Test screenshot, last step | Test screenshot, mid-test | Command line | Tool call | `clock data` | `clock settle` |
|---|---|---|---|---|---|---|---|
| Authority | D3 | D3 | D3 | LLP 1101.003 D2 | D4 | today (unchanged) | today (unchanged) |
| Pass | agent-clock | agent-clock | agent-clock | 1101.003 D2's fixpoint | none (the app's own) | `land_then` | advance to settle time |
| Activation | yes | yes | yes | none | refused until ready | yes | no |
| Timers | none fire | none fire | none fire | fire as due | the app's, live | none fire | fire to the target |
| Motion | sought to *t_s* | sought to *t_s* | sampled, paint-only; else refused | none (no engine) | not waited | not sought | sought, and world `settleAt` (`agent.rs:875-900`) |
| Frame | real `frame()` | real `frame()` | `frame_sampled` | per commit | not waited | no | no |
| Images | waited | waited | decoded already, else refused | decode synchronously | not waited | no | no |
| Budget | 16 | 16 | 16 per capture | per 1101.003 D2 | none | 16 | 16 |
| Bound and end | caller's deadline (LLP 1112 D11) → D3 ends | fresh per capture | fresh per capture | per 1101.003 D2 | 10 s from dispatch | 20 s, `settled: false` | 20 s; `reason` `"world"`, `"requests"` or `"device"` (`agent.rs:840-843`, `:884-893`) |
| After it | process ends | driver refuses a further step | session unchanged | process ends | the app runs on | today | today |
| Reply | D3 ends | D3 ends | D3 ends or `refused` | per 1101.003 D2 | LLP 1111 outcome | `{clock, settled}` | `{clock, settled, reason}` |
| Reply clock | runner | runner | runner (= presentation) | runner, elapsed since boot | n/a | runner | runner |

## What changes, where

| Where | Change |
|---|---|
| `motion/src/engine*.rs` | `Clone` on the engine and its parts; `sample_at`; `infinite()` (D1) |
| `runner/src/runner/commit.rs`, `queue.rs`, `settlement.rs` | the timers-off dispatch (D2); invocation ids, deferred-refresh owners, fulfilment capture, the outstanding-set query (D4) |
| `host/linux/src/host.rs`, `presenter*` | the `settling` flag in `dispatch_at` (D2); `frame_sampled`, the overlay with `PaintMotion` and sampled press, the refusal checks (D1); helpers return errors (D3) |
| `host/linux/src/agent.rs` | one settle function for the picture and test columns; the flag around prelude and epilogue (D2, D3); `clock data` and `clock settle` untouched |
| `scripts/agent.mjs`, `agent-test.mjs` | refuse a step after a final capture (D1) |
| `ExactKit` | the new per-`ExactApp` tool-call lock (D4) |
| Tests | a mid-test capture followed by `clock +100`, `tap` and `clock data` behaves as with no capture; a mid-test capture during a height transition is refused `height`, and after `clock settle` succeeds; press mid-fade samples at *t_s*; a final capture paints the settled height and ghost; a step after a final capture is refused; two timers due at one time with the first refusing fire nothing during a settle; a pre-op scroll event with a due timer fires nothing; a `then` issuing `focus` whose callback starts a request is waited for; 17 request round-trips end `exhausted`; a failed activation and a failed paint end `error`; Fieldnotes' `addNote` completes after `library`; a call while background work is armed is refused |

## Not in this RFC

**The full sampled frame**: a scratch kernel and presenter, so a mid-test
capture can paint height, ghost, newly visible images and newly materialized
collection rows without touching the session. Both round-2 reviews (Astra N1–N3,
Grok's two blockers) showed what it must isolate: layout's writes, the image
loader's demand, press and ghost state, paint motion and row caches. It is the
later general answer; v1 refuses those cases instead.

Also out: superseded-outcome names (LLP 1111); publishable, fonts, strict
refusal (LLP 1112); print mode's loop and exits (LLP 1101.003); frame pacing
(LLP 1073); the Apple and web settle functions, built with their first consumer.

## Questions for Charlie

1. A per-pass engine clone as v1's sample, a pure sampler later (D1)?
2. A work budget of 16 for pictures (D3)?
3. One new tool-call lock per `ExactApp`, all windows, consent included, held
   past a timeout until owned work drains (D4)?

## 7. Review dispositions (r1)

All taken in r2 and kept here (details in the review files): the clock split
(A C1, G B3) by D1; timers due at `now` (A C2) by D2; queued host work, one
predicate, the image step and helper errors (A C3–C5, G B1) by D3; ownership,
streams, background and the lock (A C6–C7) by D4; the mode adapters and
`clock settle`'s results (A C8, G B2) by D5; citations (A C9) throughout.

## 8. Review dispositions (r2)

| Concern | Disposition |
|---|---|
| A N1, G B1 — layout is not pure; the sampled height is undone before the pixels; image sync cancels live requests | **Taken by narrowing:** a mid-test sample paints no height and syncs no images, refusing `height` and `images`; a final capture seeks through the real presenter (D1) |
| A N2, G B2 — press, the ghost and paint motion are outside the copied overlay | **Taken:** the overlay carries `PaintMotion` from the clone and press at `factor(t_s)`; a ghost not at rest is refused; a final capture runs the real paths (D1) |
| G B2 — `frame_sampled`'s writes; rows dirtied afterwards | **Taken:** the scratch frame's exclusions listed; rows bypassed for the sample, nothing dirtied (D1) |
| A N3 — sampled height can expose unmaterialized collection rows | **Taken:** covered by the `height` refusal; a final capture refines collections for real (D1) |
| A N8, G M — *t_s* ms vs engine seconds; a *t_s* behind `engine.now` | **Taken** (D1) |
| A N4, G M — the pre-op pump is not timer-free; `focus` and surface messages bypass five callers; the flag's span | **Taken:** timers off inside `dispatch_at`; the flag spans prelude to epilogue (D2) |
| G M — `work` undefined; the 17-continuation test | **Taken:** `work` defined, a paint is not work; 17 request round-trips (D3) |
| A N5 — activation failure and nested callback errors | **Taken** (D3) |
| A N6 — deferred refresh intentions | **Taken:** owned and counted (D4) |
| A N7 — background attribution | **Taken conservatively:** refuse while any is armed; fail if armed during the call (D4) |
| A Q3 — the lock after a timeout | **Taken:** held until owned work drains or the session closes (D4) |
| G suggestion — narrow "the session after is the session before" | **Taken:** clocks and engine; intrinsics stay (D1) |
| G suggestions — command-line cell; `clock settle` reasons; the lock is new | **Taken** (D5, D4) |
| A N8 minor — one clone per capture; `after(0, …)` | **Taken:** per sampled pass; `after` needs ≥1 ms (D1, D4) |
| Charlie's split named `layout` as a refusal | **Not taken, with reason:** `Layout` motion is drawn as a translate of the presented box (`paint.rs:891-892`) and needs no layout, so it samples as paint-only (Grok round 2) |
| Charlie's split named `press` as a refusal | **Not taken, with reason:** `PressFeedback::factor(t)` is a pure function (`press.rs:20-22`), so the overlay samples press at *t_s* and retires nothing |
