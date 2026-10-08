# LLP 1113: Settled, and the two clocks

**Type:** RFC
**Status:** Draft r5 (2026-10-08). r5 was written from a throwaway prototype, `spike/settled-capture` (`ba1dee19a`, `f7786deda`; local, unmerged), that answered round 4's findings by running them, and it applies Charlie's rulings. Round 4 by Astra (max) and Grok 4.7 (xhigh), both NOT READY, is in `llp/reviews/1113-settled-and-the-two-clocks.{astra,grok}.md`; §7 lists r5's dispositions. Earlier rulings: r4 dropped mid-test sampling ("yeah sounds good do all that"); r3 split final and mid-test captures ("sure do both"); r2 made a capture a sample ("ok sounds reasonable, go ahead").
**Renumbered (2026-10-08):** drafted and reviewed as LLP 1107, a number origin had meanwhile given another document; its reviews keep the old numbers (1105→1111, 1106→1112, 1107→1113). r1–r3's code citations were checked at `2ce193976`; r4's and r5's at origin/main of 2026-10-08.
**Systems:** the runner (`runner/src/runner/commit.rs`, `queue.rs`, `settlement.rs`: timers off while settling, an invocation's work); the Linux host (`host/linux/src/host.rs`, `presenter*`, `image.rs`, `agent.rs`: the settle flag, the settle loop, the final capture); the agent driver (`scripts/agent.mjs`, `agent-test.mjs`: the final capture uses up the session); the Apple app owner (`ExactApp`: a new tool-call lock); the terminal host (cited, unchanged).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-07; r2–r5 2026-10-08
**Implementer:** none yet. This document proposes; it specifies nothing until it has one.
**Related:** LLP 1012 §2 (the agent's clock: `clock` moves both clocks to one instant; unamended), LLP 1012.000 (a wait that does not move the clock), LLP 1075.003.000.001 §2.4–2.5 (hatches), LLP 1101.003 D2 (print mode's fixpoint: the authority for the command-line column), LLP 1111 D4 (a tool call's outcomes), LLP 1112 (pictures; its D1a is what D3 replaces), LLP 1103 D3 (counted faults), LLP 1055 D10 (an infinite animation never settles), LLP 1092 (queues and `next`), LLP 1097 (background storage work).

## Summary

The runner clock fires timers and stamps actions; the presentation clock samples motion. Under the
agent's clock the two stay equal, as they are today, and nothing here moves them apart while
anything can still observe the session. **A final capture seeks until settled** (D1): `exact
render`'s picture, or a `--test` screenshot sent with `final: true`, runs one loop (D3) that pumps,
seeks, paints and waits until a pass is quiet, encodes that pass and uses up the session. **A
mid-test capture paints now** (D1): today's `screenshot` plus a `motion` field. D2: no timers fire
while settling. D3: the procedure (LLP 1112 will cite it). D4: what a tool call owns (LLP 1111). D5:
the modes (LLP 1101.003 D2 rules its own column).

The prototype (`spike/settled-capture`) ran D3 on the Linux host: settled captures took 0.5–18 ms,
and its PNGs matched today's `screenshot`, or `clock settle` + `screenshot` when motion was in
flight, to the pixel (D3).

## 0. Why this is its own document

Three review rounds of LLP 1101.003, 1111 and 1112 found one rule written three ways, each fix
leaving the others wrong. Four rounds of this one found that only a capture with nothing after it
can move the session safely; round 4's remaining questions were about how real functions interact,
so they were run.

## D1 — One clock; a final capture seeks until settled, a mid-test capture paints now

**The two roles.** The runner clock (`Runner::now_ms`, `runner.rs:1199`) drives timers, `then_due`,
a queue's `next`, an action's `now()` and every agent reply's `clock`. The presentation clock
(`Host::now_ms`, `host.rs:98`) is where the engine was last sought (`Host::tick`,
`host.rs:1189-1192`). They are roles, not always fields: the terminal host has no engine, and the JS
agent tags replies from `exact.clock.now` (`host/web-js/agent.js:83`). Under the agent's clock they
stay equal today: `clock` advances the runner and ticks the host to where it landed
(`presenter/clock.rs:8-31`), `Host::dispatch_at` moves both to `max(argument, now)` (`host.rs:833`,
`:846`), and `screenshot` paints without seeking (`presenter.rs:1430`).

***t_s*** is `settle().unwrap_or(host.now())` (`agent.rs:755-760`), in milliseconds: engine and
press settle (`host.rs:684-692`) and `group_settles_at`, the reorder ghost
(`presenter/group.rs:428`). An infinite-only scene has `settle()` `None`, so *t_s* is now.

**A final capture** is a `screenshot` sent with `final: true`: `exact render`'s picture, or
`screenshot <png> final [timeout]` in a `--test`. It runs D3 under the final-capture column (D5) and
seeks with `Presenter::tick(t_s)` (`presenter.rs:1416-1427`). `Host::tick` sets the host clock to
`max(t_s, now)` and advances the engine to that time over 1000 (`host.rs:1190-1191`), so a *t_s*
behind the engine samples at now and the backwards check (`motion/src/engine.rs:586-591`) cannot
trip. When height changed the presenter then clamps scroll, queues collections, refreshes transform
geometry and marks hatches stale; it always runs `tick_arrange` and sets `dirty`. The runner clock
does not move, so the clocks part (the prototype's ghost run ended with the host at 1532.5 ms and
the runner at 520), and nothing observes it, because **a final capture uses up the session**:

- **Unconditionally, once accepted.** Whatever the end (`complete`, `deadline`, `exhausted`,
  `error`, or a failed PNG encode or write), the host refuses every later operation ("the session
  ended with a final capture") and the driver closes. The prototype refused the next operation in
  every run.
- **The counted-fault check moves into the reply.** Today `unfired`
  (`scripts/agent-test.mjs:239-247`, called at `:377`) reads the fault table after the steps. The
  final capture can itself land the reply whose `then` makes the matching fetch, so a check before
  it would misreport. The host reads the fault table at the final observation point (D3) and returns
  it as `faults` (the shape of `state.faults`); the driver judges counted faults from that frozen
  copy and sends nothing more. New work; not prototyped.
- **Teardown after encoding.** `pointer_cancel` (`agent.rs:53`; it may deliver an authored
  `pointerup`, `presenter/contact.rs:791-806`) and `teardown::finish` (`:54`) run after the PNG is
  written and cannot change it. Teardown ran cleanly in every prototype run.

**A mid-test capture paints now.** A `--test` screenshot without `final` is today's `screenshot`: no
seek, no loop. Its reply adds `motion: "in-flight"` when `Host::motion()` (`host.rs:628-630`: the
engine is not quiescent, or a press is settling) or `group_needs_frame()` (`presenter/group.rs:443`)
is true, an infinite animation included, else `"none"`. It does not promise an untouched session:
today's prelude already pumps replies (`agent.rs:61-66`), and their commits can start motion. It
promises no seek and no clock movement beyond today's, so LLP 1012 §2 needs no amendment. Settled
pixels mid-test are `clock settle` then `screenshot`, which already fires the timers it crosses, as
LLP 1012 intends.

## D2 — No implicit timers while settling

With the clocks equal, `advance_timed(now)` still fires a timer due at exactly `now` (`next_ms <=
now_ms`, `commit.rs:328`), and its advance stops early when a timer refuses, leaving a sibling due
at that instant for the event to fire (`commit.rs:192-200`). So while a `settling` flag is set,
**`Host::dispatch_at` itself** (`host.rs:825-849`) calls a new `Runner::dispatch_settling`:
`land_then` (`commit.rs:289-291`), then the event at the runner's own clock, moving neither clock. A
due `then` and a queue's due `next` still run; they are due already, not timers. One rule covers
every caller: transform geometry (`transform_geometry.rs:183`), resize delivery (`:224`), authored
scroll events (`presenter/collection.rs:494`, `:709`), `focus`/`blur` (`presenter/events.rs:95`),
hover (`:267`), hatch dispatches (`presenter/hatch.rs:525`), surface messages (`surfaces.rs:865`,
`:946`) and host-command callbacks.

**The flag's span** is the final capture's own path through `agent::handle` (`agent.rs:59-95`): set
on entry, before `poll_images` (`:61`), `pump` (`:66`), `poll_update`, `poll_development`,
`run_commands`, `sync_surfaces` (`:69-72`) and the hatch turn (`:76-77`); the capture then replaces
`answer` and skips the epilogue (D3). Every other operation, a mid-test screenshot included, is
unchanged: a `tap` runs `dispatch_at` and fires what is due, as LLP 1012 says.

**Evidence and gap.** The prototype implemented this; with it off (`EXACT_SPIKE_NOFLAG=1`) the
height spring and the ghost gave the same clocks and pixels, because no app it drove dispatches
after a seek. Right but unexercised: a fixture with a `resize=` or `scroll=` callback beside an
`every(…)` timer asserts a final capture fires it zero times.

## D3 — The settle procedure

Steps by name: **activation**, **data pass**, **seek**, **frame**, **images**, **ends**. A mode (D5)
selects which run.

**Activation.** Where the host defers a data module past first pixel: paint, call `first_pixel`,
sleep 20 ms while `data_activating()` (`presenter/delivery.rs:119-121`); read `activation_failed`
(`presenter/display_frame.rs:289-308`) and end `error`, `step: "activation"`, if set; then paint one
frame.

**The loop** (the final-capture and picture columns; as prototyped in `agent.rs` `final_capture`,
`ba1dee19a`):

```text
inputs: deadline (D5); the settling flag (D2); budget = 16
loop:
  if past deadline: end deadline
  data pass: pump(host.now()); land_then; run_commands; sync_surfaces;
             settle_collections; hatch_moments;
             if hatch_in_flight() > 0: one hatch_turn                 (work)
  seek:      t = settle().unwrap_or(host.now()); if t > host.now(): Presenter::tick(t)   (work)
  frame:     if dirty: frame(); follow_pointer()                      hover inside the loop
  images:    reports = wait_images(min(50 ms, remaining))              (work if any)
             if reports: frame(); follow_pointer()
  work  := a seek, a hatch turn, an image report, or a change in
           (runner.seq(), kernel.epoch(), hatch acts, host clock); a paint alone is not work
  quiet := !requests_in_flight     has_pending less the background ticket (D5)
           && hatch_in_flight() == 0 && !images.pending()   symbols excluded
           && no node with accessibilityBusy                 a whole-tree walk; new
           && !(settle() > host.now()) && !group_needs_frame()
  if !work && quiet: end complete                            the final observation point
  if work: budget -= 1; if budget == 0: end exhausted
  else: sleep 20 ms                                          no work, not quiet; spends nothing
```

The seek follows the data pass, so a reply's motion is in *t_s*, and precedes the frame, so the
frame shows it. A seek that starts motion (`Host::tick` lays out height and `observe_layout` can
start a `Layout` curve) makes the next pass seek again, as `clock_within` does (`agent.rs:946-981`);
no prototype run needed a second seek, so a test covers it.

What the prototype settled. **Hatches:** one `hatch_turn` per pass, as `clock settle` caps its
drains (`agent.rs:888-903`); a one-shot hatch is drained by the prelude, and native-fixture's
looping presser ends `exhausted` after 16 turns in 31 ms, `still: ["hatches"]` (`clock settle`:
`reason: "hatches"`). **Symbols:** a `symbol:` view has a source and no `source_id`, so
`Images::pending()` (`image.rs:459`) is `true` forever; native-fixture hit the 5 s deadline (66 idle
passes, 67 frames) until `pending()` skipped views with `symbol_size` set (one line): `complete` in
2 ms, 0 pixels different. **Image waits are sliced:** `Images::wait` (`image.rs:528`) never pumps
replies, so a pass waits ≤50 ms and a reply is pumped next pass. **One loop, no busy poll:** a pass
neither working nor quiet sleeps 20 ms and reruns whole, so requests, a busy node and a slow image
are treated alike. **No ghost tick:** every `after_commit` calls `arrange_settled()`
(`presenter.rs:745`, `presenter/arrange.rs:371`) → `land_group(host.now())`
(`presenter/group.rs:399`); `clock +3000` then a final capture is `complete` with 0 seeks and
reference pixels; `group_needs_frame()` stays as a guard. **`dirty` is no quiet input:** a
forever-pending loader repainted every idle pass.

**The final observation point** is the end of the first quiet pass with no work: every callback the
capture caused has run and the last `frame()` reflects it. Its pixels are encoded directly; the
capture **does not call `answer`'s `screenshot()`** (which calls `frame()` again,
`presenter.rs:1430-1432`) and **skips `handle`'s epilogue** (`hatch_moments`, `frame`,
`follow_pointer` after `answer`, `agent.rs:79-92`). The prototype did both, with the 0-difference
results above.

**Errors stop the procedure.** These log and continue today; each returns its error to the loop
instead (new work): `wait_for_replies` (`agent.rs:1033-1044`) and the pump, `Presenter::tick`'s
geometry callback (`presenter.rs:1421-1423`), `frame()`'s refinement, `apply_reports`
(`presenter/images.rs:18`), `run_commands`, and nested callbacks such as `focus_command`'s
(`presenter/events.rs:63`). The prototype collected them in `errors`; the first ends the loop.

**Ends.** `complete`; `deadline`, with `still` listing what kept it from quiet in the order
`requests`, `hatches`, `busy`, `motion`, `images`; `exhausted` after sixteen work-producing passes
(a budget, not proof of a cycle: synchronous `then` chains do not spend it, because `land_then`
drains them in one call up to `TIMER_FIRE_LIMIT`, `runner.rs:503`; replies, hatch turns, seeks and
image reports arriving between passes do); and `error`, `{end: "error", step, message}`, including
`activation`, and `paint` when `last_frame_succeeded` is false
(`content_region/presenter.rs:153-180`, which otherwise returns retained or white pixels).

**Settled is not publishable.** The reply lists refused or failed images as `imagesRefused`, apart
from the end. motion-gallery's GIF and WebP tiles are `DecodeFailed` on desktop Linux: `complete`,
`still: []`, seven blank tiles. LLP 1112's publishable check fails on a non-empty `imagesRefused`.

**The reply** keeps the prototype's shape: `{screenshot, final: true, end, still, passes, idle,
seeks, hatchTurns, frames, ms, runnerClock, hostClock, errors, imagesRefused, faults}`. Caltrain `/`
after `tap sky-toggle` replied `end: "complete"`, `passes: 0`, `ms: 3.4`, `runnerClock: 0`,
`imagesRefused: []`; its `every(1000, tick)` never fired.

## D4 — What an invocation owns

A tool call (LLP 1111) does not settle the whole tree, because the person's own work would count. It
owns a set of work and **completes when the set is empty.**

**Root and ids.** The host stamps the tool action's event with the invocation's id (a new
`Runner::dispatch` form beside `dispatch_at`, `commit.rs:192`); that commit is the root. A commit's
id is copied at `enqueue` (`commit.rs:986`), `arm_then` (`:539`), `wait_turn` (`queue.rs:111`), the
`unsent` pushes (`commit.rs:661`, `:758`) and `force_refresh` (`:975`); firing a `then` stamps the
commit it runs with the `then`'s owner, and a queue entry gives its owner to the commit it runs.
**Deferred refreshes:** `refresh_next` (`Vec<usize>`, `commit.rs:26`, `:976`) becomes (resource,
owner) pairs; consuming one (`settlement.rs:771-779`) stamps the request with the refresh's owner,
whatever commit consumed it; an owned force adopts an unowned refresh, an unowned force leaves an
owned one owned, two owners cannot collide because calls are serialized, and rollback restores
owners (`commit.rs:63`, `:113`). **Fulfilment:** the id is read off the pending entry before
`remove` (`commit.rs:1236`) and stamps the fulfilment commit, so its re-asks (Fieldnotes' `library`
after `addNote`) are owned.

**Completion** is observed after a commit's `arm_then`/`arm_next` (`commit.rs:1242-1243`) **and**
after `take_requests` (`commit.rs:1072`), where background work is armed and requests handed off, so
a synchronous `Answer::Now` that starts background work is seen; never between `remove` and
`arm_then`. **Supersession:** an owned commit retargeting an unowned in-flight ticket
(`commit.rs:995-1013`) does not adopt it (LLP 1111's kept-ticket outcome); an unowned commit that
retargets an owned ticket, removes and replaces an owned entry (`:1015-1018`), `forget`s one
(`:953-963`) or overwrites an owned `then_due` (`:539-545`) ends the call `superseded`. **Streams**
are refused when scheduled (`runner.rs:301-304`). **Background work** cannot be attributed, so v1
needs none: a call is refused (`busy`) while a runner background ticket exists (`background.rs:65`)
and fails if one appears at a completion check. **Outside the set:** timers, frame tasks and
announcements the call arms.

**v1.** A call is refused (`busy`) until the data module is ready, so `unsent` is empty. Calls are
serialized by a new lock, one per `ExactApp` (`host/apple/Sources/ExactKit/Session.swift:130`),
across every session, consent included, held after a timeout while owned work survives, **for at
most 60 s** (Charlie's ruling); then it is released and the next call's certainty is `uncertain`
(LLP 1111's names). The runner holds ids only.

## D5 — The modes

| | Picture | Test, final | Test, mid-test | Command line | Tool call | `clock data` | `clock settle` |
|---|---|---|---|---|---|---|---|
| Authority | D3 | D3 | today's `screenshot` | LLP 1101.003 D2 | D4 | today (unchanged) | today (unchanged) |
| Loop | D3's | D3's | none | 1101.003 D2's fixpoint | none (the app runs) | `land_then` | advance to the settle time |
| Activation | yes | yes | no | per 1101.003 D2 | refused until ready | yes | no |
| Timers | none fire (D2) | none fire (D2) | as today | per 1101.003 D2 | the app's, live | `land_then` fires none; the prelude is unguarded | fire to the target |
| Motion | sought until settled | sought until settled | not sought; `motion` reported | none (no engine) | not waited | not sought | sought, and world `settleAt` (`agent.rs:950-957`) |
| Frame | in the loop, hover after | in the loop, hover after | today's `frame()` | per 1101.003 D2 | not waited | no | no |
| Images | waited, ≤50 ms slices | waited, ≤50 ms slices | as painted | per 1101.003 D2 | not waited | no | no |
| Background ticket | does not hold (ruling) | does not hold (ruling) | n/a | per 1101.003 D2 | refuses or fails the call | waits | waits |
| Budget | 16 work passes | 16 work passes | none | per 1101.003 D2 | none | 16 | 16, plus 16 hatch drains |
| Bound | caller's deadline (LLP 1112 D11) | the test's remaining `--timeout`, else 5 s | none | per 1101.003 D2 | 10 s from dispatch | 20 s, `settled: false` | 20 s; `reason` `"world"`, `"requests"`, `"hatches"`, `"device"`, or none after 16 rounds of engine, press or ghost time (`agent.rs:895-981`) |
| After it | process ends | session used up | session goes on | process ends | the app runs on | today | today |
| Reply | D3's | D3's, with `faults` | today's, plus `motion` | per 1101.003 D2 | LLP 1111 outcome | `{clock, settled}` | `{clock, settled, reason}` |
| Reply clock | runner | runner | runner (= presentation) | per 1101.003 D2 | n/a | runner | runner |

The prototype's `screenshot <png> final [timeout]` defaulted to 5 s; no settled run needed more than
18 ms.

## What changes, where

| Where | Change |
|---|---|
| `runner/src/runner/commit.rs`, `queue.rs`, `settlement.rs` | `dispatch_settling` (D2, as prototyped); invocation ids, the root dispatch, owned `refresh_next`, the fulfilment capture, completion after `take_requests`, supersession (D4) |
| `host/linux/src/host.rs`, `presenter*` | the `settling` flag in `Host::dispatch_at` (D2, as prototyped); helpers return errors; a whole-tree `accessibilityBusy` walk (D3) |
| `host/linux/src/image.rs` | `pending()` skips `symbol:` views (one line, as prototyped); `images_refused()` (D3) |
| `host/linux/src/agent.rs` | the final capture's own path through `handle`: the flag, the loop, encoding the quiet pass, skipping `screenshot()` and the epilogue, refusing later operations, `faults` (D1–D3); `motion` on the mid-test reply (D1); `clock data` and `clock settle` untouched |
| `scripts/agent.mjs`, `agent-test.mjs` | `screenshot <png> final [timeout]`; counted faults judged from `faults`; nothing after a final capture (D1) |
| `ExactKit` | the per-`ExactApp` tool-call lock with its 60 s cap (D4) |

## 6. Tests

From the prototype's runs: the 0-difference comparisons against `screenshot` and against `clock
settle` + `screenshot`; the looping hatch ends `exhausted` with `still: ["hatches"]`; a symbol-only
picture is `complete`; the ghost after `clock +3000` is `complete`; Caltrain's ticker does not fire;
the session is refused after a final capture; motion-gallery's refused tiles are in `imagesRefused`.
New: a `resize=` or `scroll=` callback beside an `every(…)` timer fires it zero times under a final
capture (D2's gap); a height change that starts a sibling's `Layout` curve is sought again; hover
over settled geometry that starts a request is waited for; a counted fault first exercised during
the final capture passes from `faults`; a failed encode still uses up the session; a `then` issuing
`focus` whose callback starts a request is waited for; 17 request round-trips end `exhausted`;
failed activation and paint end `error`; a mid-test screenshot during a transition reports `motion:
"in-flight"`; Fieldnotes' `addNote` completes after `library`; a synchronous action that starts
background work fails the call; an unowned `forget` of an owned request ends `superseded`; the lock
releases after 60 s.

## Not in this RFC

**Isolated sampling (later)**, a mid-test capture of settled motion that leaves the session
untouched, needs a scratch kernel and presenter or a capture-local painting context. Rounds 2–3
(Astra N1–N3, R3-3, R3-4; Grok's row-cache and paint-colour blockers) list what it must isolate: the
row cache, `frame()`'s post-paint writes, `present_paint`'s inheritance, `Layout`'s surface scale
and clipping, collection refinement, layout's writes and the loader's demand.

Also out: supersession outcome names (LLP 1111); publishable, fonts, strict refusal (LLP 1112);
print mode's loop and exits (LLP 1101.003); frame pacing (LLP 1073); the Apple and web settle
functions, built with their first consumer.

## Questions for Charlie, ruled

**Ruled (Charlie, 2026-10-08: "recs seem like an ok place to start"),** as starting points: (1) **a
16-pass work budget** for pictures and final captures, one named constant (only the looping hatch
reached it); (2) **one tool-call lock per `ExactApp`,** all windows, consent included, held past a
timeout while owned work drains, **capped at 60 s**, written into D4; (3) **a mid-test screenshot
with motion in flight reports it and does not fail** (D1); (4) **a background storage ticket holds
neither a picture nor a final capture;** the busy indicator still does, taken out of `quiet` in D3
and D5. **The prototype** ("yeah do it") was built and run as `spike/settled-capture`; its
`spike-findings.md` is r5's evidence.

## 7. Review dispositions

**r1–r3:** taken, moved or made moot; r4's table and the review files record each.

**r4:**

| Concern | Disposition |
|---|---|
| A N1, G B: the hatch drain is unbounded | **Taken, prototyped:** one `hatch_turn` per pass, counted; `hatch_in_flight()==0` in `quiet`; the looping presser ends `exhausted` in 31 ms (D3) |
| A N4, G: the observation point versus `handle`'s epilogue | **Taken, prototyped:** the capture's own path encodes the quiet pass and skips `screenshot()`'s `frame()` and the epilogue; 0 pixels differ (D3) |
| A N2: `symbol:` images hold `pending()` | **Confirmed and taken:** `pending()` skips symbols; 5,023 ms deadline before, 2 ms `complete` after (D3) |
| A N3, G: image waits starve replies; the busy poll; no sleep while requests are pending | **Taken:** ≤50 ms image slices; one loop with a 20 ms idle sleep spending no budget (D3) |
| G: a landing ghost whose rest has passed is reported settled | **Refuted by the prototype:** `after_commit` → `arrange_settled()` → `land_group(host.now())` lands it; `clock +3000` then a final capture equals the reference; `group_needs_frame()` kept as a guard |
| G: the image reframe skips hover | **Taken:** `follow_pointer` after every `frame()` in the loop (D3) |
| A N5: the counted-fault check before the capture | **Taken:** read at the observation point and returned as `faults` (D1); not prototyped |
| A N6: completion before the background ticket exists | **Taken:** completion is also observed after `take_requests` (D4) |
| A N7, G: deferred-refresh owners; unowned `forget` and replace | **Taken:** owned `refresh_next`, owner transfer and coalescing; unowned `forget` or replace ends `superseded`; the root stamp and `then` stamping named (D4) |
| A N8, G minors: "session unchanged", `clock data`'s guarantee, the passes sentence, `unwrap_or`, height-gated side effects, "LLP 1112 will cite it", no-reason exhaustion, reason order, the "fresh" deadline, consumption on every end | **Taken** (Summary, D1, D3, D5) |
| Prototype finding: the timers-off flag never changed a result | **Recorded:** a fixture is named (D2, §6) |
