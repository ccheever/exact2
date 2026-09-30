# LLP 1072: Building list rows off the main thread

**Type:** RFC
**Status:** Draft r7 (r4: stages 1+2 as built; r5: the revert and the text split, §0.4; r6: canvas draws, §8.5; r7: travel inside the built window, built and parked, §8.6). Direction accepted: Charlie ruled 2026-09-28 to move the mount off the main thread, and a directional review (Astra, max) said "go with named changes". r2 folded every P1 and P2 of that review (§0) and recorded the rulings on r1's questions (§0.1). r3 answers a focused second review of r2's retirement handshake by replacing it: asynchronous fills only create, and every destruction stays synchronous (§0.2, §5). r4 records what was built on `perf/owner-thread` and where it departs from r3 (§0.3): Charlie's 2026-09-29 direction merged stages 1 and 2.
**Systems:**
- Apple host: `Bridge.swift`'s `Runtime`, `Session.swift` (`wire`, `apply`, `Frames`, boot, pressure), `Collection.swift`, `IOS/ScrollPumpIOS.swift`, `IOS/CollectionIOS.swift`, `Mac/PresenterMac.swift`, `Mac/RegionReaderMac.swift`, `Agent.swift`, `Text.swift`, `NodeText.swift`, `Canvas2D*.swift`, `NativeModule.swift`.
- Apple Rust library: `abi.rs`'s registry and `with_runtime`, `abi/exports.rs`, `abi_collections.rs`, `app_module.rs`, `markup.rs`, `textflow.rs`.
- Runner: a create-only mode for collection feedback (`CollectionFeedback` v4 adds one `mode` word; v3 stays accepted), §5 only.
- Agent API: LLP 1012's `clock` result names `owner` as an unsettled reason.
- Web host and Linux host: none.

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** the `perf/node-cost` lane (Claude, Opus 5.5), stage 1 first; heavy builds on the M5 mini
**Date:** 2026-09-28 (r1 and r2)
**Related:**
- Charlie's ruling, 2026-09-28: "move the mount off the main thread … leaving only the UIKit/CA apply on main. Resumable rows are not chosen."
- The reviews: `llp/reviews/1072-building-rows-off-the-main-thread.astra.md` (r1) and `…astra-r2.md` (r2's retirement handshake).
- `QUEUE.md` "A heavy list row still mounts as one lump" and its diagnosis (commit `ee0f8b70`): the evidence in §1.
- LLP 1022, the serial runtime owner, parked 2026-08-30. This RFC revives it in narrower form. Its findings are acceptance tests here (§10, §11).
- LLP 1044: F4, F5 and §3 put all list work on main, synchronously.
- LLP 1044.000: §5 item 8's bar is met (§1) and its §8 "no runtime-owner thread" is replaced. §5 item 2: CoreText objects stay with the thread that made them.
- LLP 1050.000: D1 (never blank); D3, amended by §9; §2.4 names off-main building as one way to close the gap; §6: the runner stays deterministic and the host owns time.
- LLP 1068: §4.9 (incarnations), §5.1 (heavy-leaf hold), §6.2 (the remaining lump).
- LLP 1010 §6.5: stale revisions, sequences and epochs cannot overwrite newer geometry.
- LLP 1012: the agent's operations and the settle contract.
- LLP 1016 D2 (replies wake the runner through the host), LLP 1027.002 (worker-placed sources), LLP 1031 D2 (the `u32` handle ABI).

## Summary

On the iPhone 13 Pro Max's live-only feed, every late frame is one list row's
mount. It is 14.7 ms of main-thread work in one runloop turn: 8.9 ms of Rust
and decoding, and 5.9 ms of Swift apply (§1). SwiftUI spends more on the main
thread in total but never more than 16.7 ms in one turn.

This RFC moves the Rust and decoding to one **owner thread**. That thread
holds every Rust runtime in the process for its whole life. Main keeps UIKit,
Core Animation, the presenter and one **apply coordinator**.

- **Almost every call is synchronous.** The owner runs it, and main returns
  only after the call's batch has applied. Events, timers, replies, boot, the
  agent and reads all work this way.
- **The runner is touched from one thread only.** It needs no `Send` and
  gains no `unsafe`.
- **There are exactly two asynchronous operations.**
  1. **The collection fill during motion (§3).** Its batch is published to
     main and applied by the coordinator in owner order.
  2. **The display-link frame job when a fill is in flight (§7.1).**
- **Three handshakes keep the asynchrony honest:**
  - A fill only creates. Every destruction stays synchronous: eviction,
    timer commits and edge actions (§5).
  - Main re-checks coverage against current geometry before it presents
    (§6).
  - The agent's `clock` drains every kind of pending work to a fixed point,
    or says it did not (§7.2).
- **The web and Linux stay synchronous.** They are the scheduling-free
  reference that the Apple host must equal at every agent boundary.

**Decisions:**

| # | Decision |
|---|---|
| T1 | One owner thread per process holds every Rust runtime from `exact_create` to `exact_destroy`. The registry, runners, kernels, hosts, data sources and Rust thread-locals live there. No `Send`, no `unsafe impl` |
| T2 | Every C entry point has a class in §2.3's table. Owner-class calls run as one indivisible owner job: input write, operation, output copy and decode. That job is synchronous from main unless it is one of T3's two asynchronous operations |
| T3 | Asynchronous: the collection fill during motion, and the display-link frame job while a fill is in flight. Nothing else |
| T4 | One non-reentrant apply coordinator on main applies batches in owner sequence. A synchronous call returns after its own batch has applied. Reports that an apply triggers are deferred until the coordinator is empty. Geometry is gathered after the drain, and a report the runner rejects as stale stays dirty and is retried |
| T5 | Callback service is one-way. The owner reaches main only through `Owner.callMain`. A main callback cannot synchronously re-enter a runtime: that is refused by name. Notifications and logging are deferred as owner jobs. Each callback is claimed once, and locks are released before it runs |
| T6 | An asynchronous fill only creates. It evicts nothing and dispatches no edge action; it says whether eviction or an edge is owed. After applying its batch, main runs a synchronous report with fresh pins that evicts and dispatches, exactly as today. Timer advances stay synchronous |
| T7 | Coverage is verified before presentation. After a drain, main re-reads current geometry, rescues uncovered owed rows synchronously, and checks that the viewport is covered, apart from D3's pending rows and 1068's held leaves |
| T8 | Text measurement and text painting have separate mutable caches. Measurement publishes immutable line geometry keyed by content, width and font generation. No `CTLine` crosses threads |
| T9 | Agent settlement is a fixed point over fills, frame jobs, publications, apply-generated reports and the existing conditions. It is re-checked after the request and after native completion turns. When work remains after the bound, it answers `settled: false` with reason `owner` |
| T10 | Lifecycle: boot and plan swap are owner transactions. Destroy stops admission first. Pressure trims each cache on its own thread. Every job has an autorelease pool. Backgrounding stops speculative admission; foregrounding drains and re-checks coverage before resuming |
| T11 | Web and Linux are unchanged. macOS adopts asynchronous fills last (stage 5), but runs on the owner from stage 1 |
| T12 | `EXACT_FILL_SYNC=1` runs fills synchronously through the same owner and coordinator: the A/B and differential-test switch (approved, §0.1) |

## 0. What r2 changed (the review)

The review's verdict: "go with named changes. I would choose the
whole-runtime owner thread." Each finding and where r2 answers it:

| review | finding | r2 |
|---|---|---|
| #1 (P2) | Owner thread right; asynchronous ticks are a second exception, not a detail of fill | T3; §7.1 |
| #2 (P1) | Callback reentrancy: native init logs through `session.log → runtime.log` while the owner waits on main; `.notOnQueue(.main)` does not catch an owner calling `main.sync` | T5; §4 |
| #3 (P1) | An explicit ABI ownership table. `exact_app_changed` and `installAppModule` enter Rust on main. `exact_app_answer` must stay callable from a serviced callback. Input → op → output is one job | T2; §2.3 |
| #4 (P1) | FIFO execution is not FIFO application: `Session.apply` re-enters Rust. Geometry must come after the drain; rejected geometry must be retried; committed batches are never discarded | T4; §3.2–3.3 |
| #5 (P1) | A fill can retire a row UIKit still shows (`collection/mod.rs:708`). A tap in between gets `UnknownView` (`event.rs:789`) | T6; §5 |
| #6 (P1) | Awaiting the old fill does not prove never-blank. Re-read geometry, rescue, verify. The lead must cover queue, build, publication and apply. Measure uncovered rows and missing area × time | T7; §6 |
| #7 (P1) | Text: separate measurement and painting caches with immutable publications. Drop the "copy `CTLine`s" fallback. macOS `RegionReaderTiming.begin` asserts main (`RegionReaderMac.swift:59`), reached from `measure`: fix it in stage 1 | T8; §8.1 |
| #8 (P1) | Ticks and `clock settle` need a settlement protocol. Rust `tick` settles layout, heights and Canvas 2D (`host.rs:1065`). `Frames.tick` advances timers before `tick` (`Session.swift:1157`). Coalesce only unexecuted work across no barrier. Eight rounds must answer "unsettled" | T9; §7 |
| #9 (P2) | "One fill" is neither one row nor an 8.9 ms bound. Measure distributions and throughput. Bound admission across sessions | §3.4; §11 |
| #10 (P2) | Lifecycle, pressure and background policy | T10; §8.4 |
| #11 (P2) | Stage order 1 → 2 → 4 (if profiled) → 3 → 5, with its gates, including 25 µs per round trip in stage 1 | §11 |

**What changed in the architecture.** The owner thread, synchronous calls
and published batches stand as reviewed. r2's retirement handshake was
reviewed on its own and replaced in r3 (§0.2).

## 0.2 What r3 changed (the second review)

The focused review of r2's retirement handshake
(`llp/reviews/1072-building-rows-off-the-main-thread.astra-r2.md`) said "go
with named changes" but "not complete enough for stage 2". Its P1s were:

1. An acknowledged row could be destroyed while a touch still targets it.
2. Retiring rows needed a topology contract: kernel parentage, native
   attachment, `Tree::find`, `emit_children`/`placeChildren`.
3. Revival needed a transition table covering reversal, `scrollIntoView`,
   deletion and key reuse, and revival broke parity with the synchronous
   path.
4. A historical touched set is not a pin model.
5. Nested retirement had to be settled as a hierarchy.
6. v4 needed an explicit synchronous mode and coordinated ABI delivery.
7. Asynchronous timer advances and edge actions made destructive commits
   that bypassed the handshake.

Its P2 asked for a residency bound on retiring rows.

The coordinator asked for the simpler shape to be weighed with numbers:
**asynchronous fills only create**. Eviction, meaning the destruction of rows
the window no longer wants, stays synchronous.

**The number.** In the iPhone live-feed trace (§1, 1k–6k pt/s, 12 s), the
Rust side of eviction is 6.5 ms/s of main time, about 0.6 ms per mounted
row, or 2.7% of the mount. That covers:
- the runner dropping retired rows;
- `Detach::flush` and the layout-tree removals;
- the motion engine's `remove` and the host mirror's removals;
- the runner's `Mounted` map removals.

The Swift side (`NodePool.retire`, `FlatLeaves.destroy`) is 12 ms/s. It runs
on main in either design. Keeping eviction synchronous leaves 0.6 ms of
blocked main time per row where r2 would have moved it.

**Chosen: create-only** (§5, T6). It keeps destruction where it is today,
so every P1 above either becomes moot or is answered:
- **#1:** the eviction is a synchronous report issued after a fresh
  main-side interaction check. Its destroy batch applies before main does
  anything else.
- **#2–#5:** there is no retiring state, so no topology, revival, pin or
  hierarchy rules are needed. Pins are LLP 1010's and 1070's current ones.
- **#6:** v4 adds one `mode` word. Synchronous callers (v3, `EXACT_FILL_SYNC`,
  web, Linux) evict in the same operation, as today, with no observable
  extra phase.
- **#7:** timer advances and edge actions stay synchronous (§5, §7.1).
- **P2:** there is nothing to bound. A row lives in the window, or it is
  destroyed by the next synchronous report.

The residual cost is 0.6 ms per row of blocked main plus one hop. Stage 2
measures it, and it is the first thing to revisit if the late-frame gate is
missed.

## 0.3 What r4 changed (as built, 2026-09-29)

Charlie, 2026-09-29: "focus on optimizing and ideal end states rather than a
smooth journey there." Stages 1 and 2 were built as one lane on
`perf/owner-thread`, each commit verified (the five checks and every smoke on
the M5 mini; the Apple XCTests and the macOS smokes locally; device runs on
the iPhone 13 Pro Max). Where the build departs from r3:

1. **The wire.** v3's flags word carries the two modes: bit 1 is build-only
   and bit 2 is retire-only; setting both is refused. There is no `mode`
   word and no ABI bump. An older runner refuses flags above 1 as malformed
   feedback rather than misreading them. Web and Linux never set either bit.
2. **While a slice is in flight, work waits on main, not on the owner.**
   r2 and r3 had main block behind a fill, or post a coalesced frame job.
   As built, every report, every timer, every data-source reply and every
   intrinsic size is held on main while a slice builds, and runs in order
   after it lands. Events and reads that return values still block, behind
   at most one slice.
3. **A frame's tick is always asynchronous under off-main slices.** It is
   posted, not waited for, and its batch joins the slices' ordered
   publication queue. That removes one round trip per frame. A frame's timer
   advance stays synchronous (§5), and is skipped while a slice is in
   flight.
4. **A landed slice retires on the next main-queue turn.** Applying a slice
   and running its retire-only report are two turns' work. The next slice
   waits for the retirement.
5. **Under the agent, and with `EXACT_FILL_SYNC=1`, slices are
   synchronous.** The agent's operations therefore match the web and Linux
   by construction; the carrier still drains any slice in flight before each
   request. The build-only/retire-only split is proven equal to one
   immediate report by the runner's test
   (`a_build_only_report_then_a_retire_only_one_equal_one_immediate_report`).
6. **macOS keeps synchronous slices** until physical scrolling there is
   measured (stage 5). It runs on the owner thread like iOS.
7. **The macOS reader** (`RegionReaderTiming`, T8): its paragraphs hold
   layers, so they stay UI-owned. A measure request the reader may answer,
   or one naming a paragraph it holds, goes through the owner's one door to
   main. Every other measurement runs on the owner, and the timing
   assertion stays true.
8. **The round trip is a semaphore per waiter, not polling.** Polling (the
   owner 100 µs, main 50 µs to 1 ms, with or without the lock) made a round
   trip worse on the iPhone 13 Pro Max and the M1 iPad Pro: 6–15 ms/s of
   main's blocked time on the live fling, against 2–5 ms/s for plain waits.
   Both have two performance cores, and a spinning thread pushes the other
   off one.

**Measured on the iPhone 13 Pro Max** (medians of 3 rounds, rounds r10–r12,
`~/bench/xheavy/results/node/iphone`). n7 is origin/main before the lane,
n13 has off-main slices, n14 adds asynchronous ticks:

| feed, scenario | n7 | n13 | n14 | SwiftUI |
|---|---|---|---|---|
| live fling: fps / late per s / main ms/s | 107.0 / 8.4 / 380 | 113.3 / 5.2 / 226 | 113.6 / 5.1 / 226 | 118.1 / 1.1 / 423 |
| live ladder | 89.0 / 17.4 / 515 | 99.7 / 10.8 / 311 | 100.2 / 10.6 / 313 | 103.6 / 8.9 / 543 |
| 19-kind fling | 108.7 / 6.5 / 356 | 114.2 / 3.5 / 246 | 113.2 / 4.2 / 305 | 102.2 / 10.2 / 363 |
| 19-kind ladder | 81.5 / 13.5 / 494 | 112.8 / 4.5 / 281 | 102.2 / 6.7 / 323 | 79.4 / 15.3 / 505 |

n15 (a landed slice's retirement on the next turn, rounds r13–r15) against
n14 in the same batch: live fling 114.3 fps / 4.7 late per s against 114.2 /
4.7; the 19-kind fling 113.8 / 3.4 against 114.9 / 2.9. No gain, and no
loss.

**On the M1 iPad Pro** (n15, fling, 3 rounds, r13–r15,
`~/bench/xheavy/results/node`):

| feed | n7 | n15 | SwiftUI |
|---|---|---|---|
| live: fps / late per s / main ms/s | 119.4 / 0.6 / 379 | 120.0 / 0.0 / 241 | 119.4 / 0.5 / 543 |
| 19-kind | 112.5 / 4.1 / 392 | 115.8 / 2.2 / 352 | 106.2 / 8.4 / 423 |

No run on either device showed a blank band. The iPhone's live fling
(4.7 late per s against SwiftUI's 1.1) is the one gate not yet met.

**The round trip itself** (a temporary per-call counter, not committed): on
the live fling, 100–380 synchronous calls a second cost 2–5.5 ms/s of main's
blocked time beyond the work, p50 10–35 µs and p99 50–200 µs. That is over
the 3 ms/s budget in the slow segments. Asynchronous ticks remove the
per-frame call; the rest is events, reports and replies.

**What the late frames are now.** A System Trace of the n15 build
(`~/bench/xheavy/results/late/iphone/n15`) shows almost no main-thread turn
over 8.33 ms (0.2 a second). The late frames are frames whose several turns
together exceed the frame. Main's work over a 6k pt/s fling (ms/s):

| part | ms/s |
|---|---|
| presenter apply of landed slices | 96 |
| CA commit | 68 |
| reports on main, the retire-only report's destroy apply among them | 45 |
| display link | 10 |

Inside the apply, per node:

| part | ms/s |
|---|---|
| SVG animations and scenes | 23 |
| flat-leaf create, frame and flush | 42 |
| node-pool take and retire | 20 |

That is stage 4's target. The Rust half of a mount is off main.

## 0.4 The revert and the text split (r5, 2026-09-29)

**What happened.** Stages 1 and 2 landed (496ab5c35) and were reverted
(951c0677f) the same morning. exact2 crashed on any scroll, on the iPad and
the iPhone: five crash reports in twelve minutes
(`~/bench/xheavy/results/main-19-ipad-496-hang/`).

**The cause** was one `TextEngine` shared by two threads. Its residency,
fonts and break cache have no lock:
- the owner measured (`measureText` → `paragraph` → `TextResidency.retireWidths`);
- main painted the same text at the same time (`refreshVisibleText` →
  `paragraphLayout` → `paragraph` → `trim`, `evict`, `charge`).

T8 (§8.1) had been deferred to "stage 2's concern" and then not built with
stage 2.

**Why nothing caught it.**
- The agent, the smokes and every XCTest ran slices synchronously, so the
  two threads never ran at once there.
- The iPhone runs that built the lane did not crash. The race was rare
  enough there and common enough on the iPad.

**The fix is T8 as written.** A session's text is two engines of one kind
that share nothing:
- **The painter** paints on main.
- **The measurer** is touched only by the owner. The kernel's measure hook,
  font hook and Canvas 2D text hook all get it.
- **Separate state:** each has its own fonts, shaped text, residency and
  caches.
- **What crosses** from a measurement to painting is immutable
  `LineGeometry` (the nested-list lane's type, `perf/clamp-raster`) in a
  locked table, with the metrics measured there (`TextAnswers`, §8.1 "As
  built"; first a 512-entry board of breaks). It is keyed by the
  paragraph's metric content and the width offered, and a font install
  empties it.
- **Fonts.** The font hook installs the catalog in the measurer on the
  owner, and in the painter on main while the owner waits.
- **Owner-side work.** Checkpoints, font commits and cache trims reach the
  measurer on the owner.
- **The macOS reader.** Its held views are readable off main under a lock.
- **The font registry** is locked.

**The other shared state the owner touches, audited:**
- **The batch style cache:** already locked.
- **Markup handles:** per call.
- **The text-flow table:** process-wide under a lock since stage 1.
- **Raster core:** a global mutex.
- **Canvas 2D fonts:** now the measurer's own.
- **Pools, the SVG host and flat leaves:** main only.
- **`Runtime.destroyed`:** set on main before the owner's queued work reads
  it.

**What now catches it:**
- **A stress test** (`OwnerTextIOSTests`, macOS and the iOS simulator)
  measures on the owner while main paints the same paragraphs. Under the
  Thread Sanitizer it reports 16 races when one engine does both (the
  crash's shape) and none with the pair. The two lay out the same lines.
- **A scrolling test** (`OwnerScrollMacTests`) turns asynchronous slices on
  and scrolls a real list. It is clean under the Thread Sanitizer, as is
  the whole macOS suite (478 tests) and the stress test on the iOS
  simulator. It did not reproduce the old race by
  itself, so the stress test is the guard.
- **The device gate:** three rounds of the 19-kind fling and ladder and the
  live fling and ladder on the iPhone 13 Pro Max (rounds r20–r22, build
  `rl1`), with no crash report in the device's crash logs. (The lane's own
  iPhone runs on 2026-09-29 had crashed four times, 05:20–05:43, in
  `TextResidency` and in `measureText` on the owner. Those were not counted,
  because the probe's result files were complete. A device gate now reads
  the crash logs as well.)

**What the split costs** (medians of 3, the iPhone, fling / ladder):

| feed | `rl1` fps | late per s | main ms/s | before the split (n14–n15) |
|---|---|---|---|---|
| 19-kind fling | 115.1 | 2.8 | 237 | 113.8 fps, 3.4 late, 282 ms/s |
| live fling | 113.0 | 6.1 | 235 | 114.3 fps, 4.7 late, 207 ms/s |
| live ladder | 93.5 | 12.5 | 334 | 100.2 fps, 10.6 late, 313 ms/s |

The live feed pays for the split: painting no longer finds the measurer's
shaped paragraphs, so main shapes its own lines from the published breaks.
It costs about 25 ms/s of main on the live fling. Stage 4 recovers some of
it; building the painter's lines off main from `LineGeometry` (the
nested-list lane's raster path) recovers the rest.

## 0.1 Rulings on r1's questions (Charlie, 2026-09-28, through the coordinator)

- **Q1: one owner per process.** It stands only if the two-session latency
  gate holds (§11 stage 1). Per-session owners come only if measured
  contention needs isolation.
- **Q2: `EXACT_FILL_SYNC` is approved** (T12). It is a narrowly scoped
  diagnostic and test switch on the same implementation, not a second path.
- **Q3: stage 1 lands alone** once its gates pass.
- **Q4: Linux stays synchronous.** Semantic parity does not require
  identical scheduling.
- **Q5: amend LLP 1050.000 D3 explicitly** (§9). Building mid-fling off main
  is allowed when the row's main-thread work fits the remaining frame budget.

## 1. The evidence

LLP 1044.000 §5 item 8 kept the engine on main "until a physical 120 Hz run
under load shows main-thread misses". That run was taken on 2026-09-28:
- System Trace on the iPhone 13 Pro Max, live-only feed, fling at 1k–6k
  pt/s, origin/main `558a5bd8`.
- Data and scripts: `~/bench/xheavy/results/late/iphone/`.

| | exact2 | SwiftUI |
|---|---|---|
| main-thread busy, ms/s | 350 | 408 |
| runloop turns > 8.33 ms, per s | 11.0 | 5.4 |
| turns > 16.7 ms, per s | 5.4 | 0 |
| median long turn | 15.9 ms | 11.5 ms |
| what long turns are | 94% one `ScrollPump` slice mounting about one row | CA commit 61%, `TimelineView` display link 36% |
| probe late frames per s (fling, 3–9 rounds) | 8–9 | 1.1 |

The main-thread cost of one mount, averaged over 132 long turns:

| part | ms | moves |
|---|---|---|
| runner report (`apply_document` 1.5, instance creation 1.1) | 2.9 | yes |
| host commit (layout 1.5, create 0.8, motion 0.9, SVG emit 0.5) | 4.6 | yes |
| batch decode | 1.4 | yes |
| Swift apply (flat leaves 2.1, SVG scene and animations 1.2, pool 0.6, …) | 5.9 | stage 4, if profiled |

These are means over turns that mostly hold one row. A fill asks for
`max(1, fits, needed)` rows. A change in pins or geometry lifts the limit,
and a data change realizes the whole window. So neither "one fill" nor
"8.9 ms" is a bound. §11 replaces both with measured distributions.

The live clock is off in the fling, in both apps. With `BENCH_LIVE=1`,
exact2's late frames do not change (201–205 in 24 s). SwiftUI's rise to
50–62, locked to its tick. The cause is the lump.

## 2. What runs where

### 2.1 The owner thread (T1)

There is one `Thread`, `exact.owner`, at QoS `userInteractive`. It is created
at the first `exact_create` and lives for the process. It must be a dedicated
thread, not a serial queue: the registry (`abi/exports.rs:25-27`) and the
Rust thread-locals are keyed by thread.

The owner holds:
- each `Bridge<D>` with its `Host<D>`: `Runner`, `Kernel`, motion `Engine`,
  `SvgState`, the executor sender and the in/out buffers;
- each data source `D`. Worker-placed sources (LLP 1027.002) already hand
  envelopes to "the runner's thread", which is now the owner;
- the Rust thread-locals: `REFUSAL` (`abi.rs:1384`), `markup.rs:59`,
  `textflow.rs:107`, `stdlib.rs:280`'s scratch, and the collection
  `PinEpoch` (`nest.rs:378`).

Its loop pops jobs from two queues:
1. **Synchronous requests.**
2. **Admitted asynchronous work** (fills and frame jobs).

A synchronous request overtakes asynchronous work that has not *started*. An
unstarted fill has mutated nothing, so running the request first is the same
as the request arriving first. Started work is never preempted. Each job runs
inside an autorelease pool.

**Why not lend the runtime to a worker per fill.** The review agrees. A
hand-off would move `Rc` graphs and thread-local state between threads.
Detached subtree construction would need new rules for ids, local state,
dependencies, effects and invalidation. Rejected.

### 2.2 Main (T2, T4)

Main keeps:
- UIKit and AppKit, and the presenter;
- `CollectionHost` (facts, cursor, snapshots) and the scroll pump;
- `Frames`, the raster loader, and the publication side of text painting;
- the agent carrier's execution;
- the apply coordinator (§3.2).

`Runtime` in `Bridge.swift` becomes a façade. Each method submits one owner
job and waits. The waiting is a loop: it wakes for its reply *or* for a
callback the owner posted (§4). It returns the decoded `Batch` to the
coordinator, which applies it and any earlier batches. Call sites in
`Session.wire` keep their shape: `apply(runtime.press(id, now: now()))`.

### 2.3 The ABI, entry by entry

Every export of `exact.h` is in one of four classes. The façade enforces the
class, and a debug build asserts it (§10).

| class | where it runs | entries |
|---|---|---|
| **owner job** (registry, per runtime; input, op and output in one job) | the owner. Synchronous from main, except T3 | `exact_create`, `exact_destroy`, `exact_boot`, `exact_boot_plan`, `exact_prepare_plan`, `exact_commit_plan`, `exact_discard_plan`, `exact_prepare_module`, `exact_dispatch`, `exact_collection_feedback`, `exact_tick`, `exact_advance`, `exact_pump`, `exact_agent`, `exact_resize`, `exact_insets`, `exact_set_time`, `exact_set_root_font_size`, `exact_set_preferences`, `exact_set_place`, `exact_set_page`, `exact_set_launch_location`, `exact_set_measure`, `exact_set_fonts`, `exact_set_canvas_text`, `exact_set_wake`, `exact_set_app_module`, `exact_intrinsics`, `exact_into_view`, `exact_list_index`, `exact_list_text`, `exact_select_options`, `exact_reorder_*`, `exact_hold_*`, `exact_has_hold`, `exact_height_drag_*`, `exact_transform_motion`, `exact_surface_record`, `exact_fulfill_surface`, `exact_canvas_image`, `exact_canvas_display`, `exact_command`, `exact_auth`, `exact_delivery_sync`, `exact_data_ready`, `exact_request_active`, `exact_region_request`, `exact_region_complete`, `exact_text_ready`, `exact_location_of`, `exact_scheme`, `exact_view_scheme`, `exact_baked_compat`, `exact_trim`, `exact_app_changed`, `exact_in`/`exact_out` (only inside a job) |
| **owner-deferred** (a notification: no result used) | an asynchronous owner job, after the current one | `exact_log`. Also `exact_app_changed` when called from a serviced callback |
| **owner-local helpers** (thread-local tables, by handle) | directly when already on the owner (from a measure callback, say). As an owner job from main | `exact_markup_*`, `exact_textflow_*` (`prepare`, `flow`, `free`), `exact_text_collapse` |
| **callback context** (valid only during the call it answers) | on whichever thread services the callback. It never touches the registry or enqueues | `exact_app_answer`: it writes the waiting caller's stack slot (`app_module.rs:55`) |
| **thread-free** (process-global, locked, or pure) | any thread | `exact_raster_*` (a global `Mutex`, `raster.rs:111`), `exact_app_reply` (a `Send` reply), `exact_gesture_constant`, `exact_material_platform`, `exact_native_abi`, `exact_delivery_api` |

**What moves.**
- `nativeChangedCallback` called `exact_app_changed` on main
  (`NativeModule.swift:246`). That becomes an owner job: deferred if it
  arrives while main services a callback, synchronous otherwise.
- `installAppModule` called `exact_set_app_module` directly
  (`NativeModule.swift:378`). That becomes an owner job at session setup.
- Every other direct `exact_*` call outside `Bridge.swift` is found and
  classified by the same audit, which is a stage 1 deliverable.

## 3. The asynchronous fill and the apply coordinator

### 3.1 Posting a fill (T3)

During motion the scroll pump calls `fillSlice`, and the fill goes to the
owner.

1. **Drain first.** Main drains the coordinator (§3.2).
2. **Gather facts.** It reads the scrollport geometry, the focus and
   interaction owners, and the measured sizes of applied rows.
3. **Post.** It posts `exact_collection_feedback`, in create-only mode
   (§5), as an asynchronous job,
   with main's `now` and the session generation.
4. **Owner side.** The owner runs the report, the commit, layout with text
   measurement, `present`, `finish` and the decode. It publishes the
   `Batch`, stamped with the owner sequence and the generation, then wakes
   main with one `main.async`.

Admission is one fill in flight *per process*, round-robin over sessions and
lists. A second list's report waits for the slot.

What stays synchronous:
- reports at rest;
- reports after a resize or a pin change;
- `dataReady`;
- the hole rescue (§6);
- every report under `EXACT_FILL_SYNC=1`.

### 3.2 The apply coordinator (T4)

One object on main owns application. It holds a queue of batches ordered by
owner sequence, and one flag, `applying`.

- **Order.**
  - A synchronous call's batch goes into the queue.
  - The coordinator applies everything published before it, then that batch.
  - The call returns only after its own batch has applied.
  - A fill's batch waits for the pump, which applies at most one fill batch
    per frame, at the start of its slice, unless a synchronous call or an
    owed rescue drains it sooner.
- **No reentry.** Today `Session.apply` re-enters Rust from inside an apply:
  it flushes collections (`Session.swift:753`), and the presenter reports
  intrinsics. Under the coordinator, anything an apply triggers only marks
  work dirty. That work runs after the queue is empty, as new owner calls,
  so batch 3 can never overtake batch 2.
- **A batch is never partial.** It is one runner commit whose ids, children
  and styles refer to each other.
- **Discard only dead batches.** A batch is dropped only when its generation
  is dead (reload, destroy). A committed batch for a live runtime is always
  applied, even when newer data exists: the runner's state already includes
  it.

### 3.3 Fresh geometry, and retrying rejected geometry (T4)

Today feedback is encoded from the applied snapshot and marked sent
(`Collection.swift:390`, `:415-416`) before the Rust call. If a fill or a
data update lands in between, the runner rejects the report wholesale as
stale: `prepare_feedback` returns `Ok(None)` (`collection/mod.rs:858-893`).

Under r2:
- facts are gathered only after a drain;
- the Rust host tells a stale rejection apart from an empty success, with a
  new batch flag `stale`;
- on `stale`, `CollectionHost` restores the list's dirty bit and clears
  `lastFacts`/`lastSequence`, so the next turn re-gathers and re-reports.

LLP 1010 §6.5's revision, sequence and epoch checks stay the arbiter.

### 3.4 Throughput and cross-session admission

Waiting for apply before posting the next fill serializes build and apply.
At about 9 ms of build and 6 ms of apply, that caps a list near 66 rows/s.
A 6k pt/s fling of the live feed needs about 18. Faster flings build
nothing mid-fling (D3, `limit`).

Stage 2 measures sustained production against demand. If production falls
short, stage 2b allows one more fill to be posted while the previous batch
awaits apply. Its facts reuse the last applied geometry and carry no new
measurements; those come with the next report.

With one owner per process, a heavy session's fills must not starve another
session's synchronous calls. Synchronous requests overtake unstarted fills
(§2.1), and stage 1 has a two-session latency gate (§11).

## 4. Callback service and reentrancy (T5)

The owner sometimes needs main during a job:
- `native.call` does `DispatchQueue.main.sync` when off main
  (`NativeModule.swift:232-244`);
- macOS resolves an unseen font variant through `NSFontManager` (§8.1).

The contract:

1. **One door.** The owner reaches main only through
   `Owner.callMain(name, body)`. It posts the body to a mailbox and waits
   for its completion. Main runs the body from:
   - its wait loop, when main is blocked in a synchronous request; or
   - a `main.async` hop, when main is free.
   
   Each body is claimed once: an atomic `posted → claimed → done` state. The
   mailbox lock is released before the body runs.
2. **No synchronous reentry from a serviced body.** While main runs a body,
   a thread-local depth on main is non-zero. A synchronous owner request
   made then cannot be served: the owner is suspended inside the job that
   asked. Such a request answers today's `busy` refusal (`abi.rs:1446`) by
   name and journals it. Rust also holds a mutable borrow of the runtime
   across the callback (`abi.rs:1442`), so running the request inline would
   be unsound too.
3. **Notifications are deferred.**
   - `log` (`Session.swift:969`, reached from native init at
     `NativeModule.swift:318`), `app_changed`, and any other entry whose
     result is unused become owner-deferred jobs.
   - They run after the current job, in submission order.
4. **Answers are direct.** `exact_app_answer` is callback-context (§2.3). It
   writes the waiting caller's slot from the serviced body without touching
   the owner.
5. **Other threads.**
   - Worker-placed sources and the executor never wait on the owner.
   - They reach main with `main.async`, or with `Owner.callMain`'s mailbox
     when the owner is the one asking.
   - The owner never waits on any thread except through `callMain`.
6. **Detection.** `dispatchPrecondition(.notOnQueue(.main))` does not catch
   an owner calling `main.sync`, so r2 does three things instead:
   - ExactKit's `DispatchQueue.main.sync` sites go through one helper,
     `Main.sync`. It traps in debug builds when the current thread is the
     owner, and routes through `callMain` in release.
   - A source check fails the build on a raw `DispatchQueue.main.sync` in
     ExactKit.
   - A debug watchdog dumps both threads' current job names when main has
     waited more than 250 ms on the owner while the owner is inside
     `callMain`.

## 5. Asynchronous fills only create (T6)

Today one report both builds the rows the window wants and destroys the rows
it no longer wants (`collection/mod.rs:708`). It also dispatches
`reachstart`/`reachend` edge actions (`runner/collection.rs:124`), whose data
updates can destroy anything. r2's review showed that any destruction
committed asynchronously opens an interval in which UIKit still shows, and
hit-tests, views the runner has already forgotten. An event in that interval
fails as `UnknownView` (`runner/event.rs:789`).

r3 therefore splits a moving list's report in two:

1. **The asynchronous fill creates.** Its feedback carries
   `mode = create-only` (`CollectionFeedback` v4, one `u32` after v3's
   fields). The runner:
   - realizes the rows the window wants;
   - neither evicts rows outside the window nor dispatches edge actions;
   - sets two flags in the snapshot the batch carries: `evict_owed` and
     `edge_owed`.
   
   The window may briefly hold more rows than the retention cap, by at most
   one fill's worth.
2. **Main evicts synchronously, after the apply.** When the coordinator
   applies a create-only batch whose snapshot says `evict_owed` or
   `edge_owed`, the next turn's first collection work is a synchronous
   report for that list. It runs after the drain, gathers fresh facts, and
   uses mode `immediate` with `limit = 0` so it builds nothing. The facts
   carry the *current* focus and interaction owners (`Collection.swift:390`),
   read on main at that moment. The runner evicts and dispatches exactly as
   today's reports do, respecting current pins. Its batch, with the destroy
   ops, applies before the call returns.
3. **Nothing else destroys asynchronously.**
   - Timer advances (`host.rs:837`) are always synchronous (§7.1).
   - Frame jobs never advance timers.
   - Edge actions run only in immediate reports.

**What this preserves.** Between a fill's post and its apply, nothing the
user can see or touch loses its runner instance: the fill only adds. From
the eviction report's post to its apply, main is blocked, so no event can
arrive in between. Pins, the nested traversal, `keep_positions` and data
deletion are untouched; they run in immediate reports as today.

**Parity.** A create-only fill followed by an immediate `limit = 0` report
builds the same rows and destroys the same rows as one immediate report with
the fill's limit, given the same facts. The runner's differential test gains
exactly that pairing. `EXACT_FILL_SYNC=1` sends immediate reports only.
Under the agent, every boundary drains both halves (§7.2).

**Wire and ABI** (the review's #6):
- **v4 = v3 + `mode: u32`.** 0 is immediate and 1 is create-only; any other
  value is refused before mutation. The decoder accepts v3 as immediate.
  Web and Linux keep writing v3 and are unchanged.
- **Snapshot fields.** The Apple snapshot JSON gains `evictOwed` and
  `edgeOwed`, absent when false.
- **ABI version.** `EXACT_ABI_VERSION` is bumped (LLP 1031 D2). ExactKit
  and the archive must match, and a mixed-version pair is refused in both
  directions, as today's compatibility cohort does.

**Cost.** Eviction's Rust side stays on main as blocked time: 6.5 ms/s on
the live fling, about 0.6 ms a row (§0.2). Stage 2 reports it separately. If
stage 2 misses its late-frame gate, the first thing to revisit is running
the `limit = 0` eviction report asynchronously while the pump defers input
to the list. That needs its own design and review.

**Tests, with publication held.** A test hook holds a create-only fill:
- before commit;
- before publication;
- before apply;
- between the fill's apply and the eviction report.

At each hold point these must deliver their event with no `UnknownView`:
- touch-down then click on a row the window no longer wants;
- focus transfer into it;
- VoiceOver focus;
- a selection drag;
- IME composition;
- a timer and an edge action removing the displayed target.

The tree and state at the next settle must equal the `EXACT_FILL_SYNC=1`
run.

## 6. Never blank (T7)

Awaiting the old fill does not prove coverage. Its geometry describes the
viewport at post time. By completion, the scroll may have:
- reversed;
- jumped;
- met a keyboard or a width change;
- remeasured rows.

Another list may need rows too.

1. **Before presentation.** The rescue runs in the scroll callback
   (`Collection.swift:285-290`) and the pump's slice:
   1. drain the coordinator;
   2. re-read current geometry;
   3. if the applied rows do not cover the scrollport, run a *synchronous*
      report limited to the owed rows and apply it;
   4. verify coverage (§10 counts failures).
2. **Keep what is coherent.** During a resize, content that the runner
   accepted stays until its replacement applies. Correction sequence checks
   (`Collection.swift:43`) and UIKit's momentum (`CollectionIOS.swift:89`)
   are unchanged.
3. **The lead covers the whole path.** It is velocity × (p95 queue wait +
   p95 build + publication delay + p95 apply + one frame). Every term is
   measured per list, not assumed.
4. **What "never blank" means here.** No row box in the viewport is
   missing, except where the rules already allow it: D3's pending rows, and
   1068's held heavy leaves, whose row box is present and whose leaf content
   is absent.
5. **Metrics.** Uncovered rows per second, and missing-content area × time,
   each split into:
   - allowed (D3, 1068);
   - unintended. The gate for this part is zero.

## 7. Frames, ticks and settlement

### 7.1 The display-link frame (T3's second exception)

`Frames.tick` (`Session.swift:1142-1164`) does three things in order:
1. if a timer is due, `advance`;
2. if motion or a 2D canvas wants a frame, `tick`;
3. render native canvases with `canvases.tick`.

Rust `tick` is not a replaceable sample. It settles layout, runs height
layout and advances Canvas 2D (`host.rs:1065`).

The frame job:
- **No fill in flight:** steps 1 and 2 run as one synchronous owner job, as
  today.
- **A fill in flight, no timer due:** main does not wait. It submits one
  asynchronous frame job carrying the frame's captured virtual `now`, which
  runs step 2 only.
- **A fill in flight and a timer due:** step 1 stays synchronous (§5, r3).
  A timer's commit can destroy what the user touches, so main waits for the
  fill, then runs steps 1 and 2 as one synchronous job. A late timer frame
  during a fling is the price; stage 2 counts these waits.
- **Coalescing:**
  - A frame job that has not started may be replaced by a newer one, only
    if no event or timer job was submitted after it. Those are barriers.
  - A frame job that has run is never discarded. Its batch applies in
    order.
- **Native canvases** (step 3) render on main after the coordinator has
  applied everything published by that point. So they draw the latest
  applied state, never a state newer than what the presenter shows. When
  the frame job is still pending, they render the previous state with this
  frame's `frameNow`, as a dropped frame would today.

Under the agent (virtual clock), frame jobs are always synchronous.

### 7.2 `clock settle` and every agent request (T9)

The carrier runs each request on main (`Agent.swift:62-90`).

**Before and after every request, and after each native completion turn,**
it runs the fixed point:
- drain the coordinator;
- wait for fills in flight;
- run any queued frame job;
- run the reports that applies generated;
- then the existing conditions:
  - `fillPending` empty;
  - no pending replies (`pendingCount`);
  - text refreshed;
  - native work not in flight;
  - holds, and images.

**`clock settle`** (`Agent.swift:327-406`) runs that fixed point at the top
of each round, and again after `settlePump`. Its fills are synchronous (the
owed path), so the result does not depend on scheduling.

**When the bound is reached,** 16 rounds or 20 s as today, and any
condition is still true, the reply is `settled: false`. The reason list
includes `owner`, naming what remained:
- fills;
- frame jobs;
- publications;
- reports.

LLP 1012's diagnostics shape gains that one reason. Eight or sixteen rounds
are a bound, never proof of settlement.

**The clock stays virtual.** Fills and frame jobs carry main's captured
`now`, which is the agent's clock under the agent (`Session.swift:512`). The
runner reads no clock (LLP 1050.000 §6).

## 8. Subsystems

### 8.1 Text (T8)

Ordinary measurement is CoreText (`Text.swift:855-873`). It uses font
descriptors, literal colours and locally built attributed strings. It makes
no view or TextKit calls (`:573`). The hazard is shared mutable state:
- `measuredBreaks` reads `residency` (`:715-720`);
- painting calls `paragraph` and `accepted` (`NodeText.swift:24`);
- Canvas 2D mutates font caches (`Canvas2DImage.swift:99`).

r2 splits `TextEngine`:
- **Measurement (owner).** Its own font table, `residency`, and the
  `CTTypesetter`s and `CTLine`s it makes. It *publishes* an immutable
  `LineGeometry`: line ranges, baselines, width, extents, keyed by (content
  identity, width, font generation). Publication goes into a map under one
  lock; the lock is held only to insert or look up, never while shaping.
- **Painting (main and the text raster workers).** Its own font table and
  its own `CTLine`s, shaped from published `LineGeometry`. Measurement's
  objects are never read. r1's fallback, copying cached lines into a batch,
  is dropped: copying an array does not make independently owned `CTLine`s.
- **Canvas 2D text:** measured by `exact_set_canvas_text` on the owner, with
  measurement's fonts. `Canvas2DImage`'s drawing-side font cache is
  painting's.
- **Font generation:** bumped by an `exact_set_fonts` install or a catalog
  change. A publication with an old generation is not used.

**As built, 2026-09-30: measured answers.** What measurement publishes is
each paragraph's answer at each size the kernel offers it: the metrics, and
at a definite width the line breaks (`TextAnswers.swift`). Both engines read
it, under its one lock:
- **The measurer answers from it first.** The kernel offers a new row's
  paragraph about ten sizes (thirty calls for an inbox row's three
  paragraphs). A paragraph measured before is answered with no identity,
  shape or typesetter made, whatever the measurer's residency still holds.
  Before, the residency's 4,096 cold entries were the memory: a
  thousand-message list's travel evicted them, and each row coming back was
  typeset again (8–9 of its 30 calls; the owner's cost a row 0.22 → 0.45 ms
  on an iPad Pro M1). The measurer's residency now keeps shaped text only:
  the table replaces its list of answers, it is not a second one.
- **The painter takes its breaks from it.** Before, a paragraph the
  measurer answered from its own cache published nothing, and the board
  held 512: main typeset half of what a fast list painted a second time, to
  tell the raster job where its lines end.
- **The key is the paragraph's metric content as bytes**: its runs' UTF-8
  and the fields that move glyphs, the strut, the wrapping. It is compared
  exactly, and built the same from the kernel's request (before any
  `String` is made of it) and from the painter's `Spec`. A definite width
  is keyed at the kernel's `f32`: the painter's width came through the
  batch's decimal text, and unless it is a quarter of a point it is not the
  double of the kernel's `f32`, so as an exact double it never matched (a
  flexed label beside an intrinsic one).
- **Bounded by bytes**: 2.5 MB, least recently used first out, about 3,500
  paragraphs of a sentence or two at two or three offers each. The 19-kind
  fling ends with 1,400 paragraphs in it, 0.7 MB.
- **An answer that is gone is only typeset again.** What is measured and
  what is painted are what they are without the table
  (`OwnerTextIOSTests`: 160 paragraphs through a 16 KB table, each one's
  metrics and breaks against an engine that has none; the owner publishing
  while main reads, under the Thread Sanitizer). A font catalog change
  empties it from each engine's reset, in either order.
- **Measured** (iPad Pro M1, 3 rounds interleaved, the inbox's fling):
  48k pt/s 106.9/102.0 → 119.9/117.0 fps, 96k 101.0/102.0 → 106.5/106.0;
  the owner's cost for a 9-row rescue 3.5–4.0 → 1.85 ms; main typesets
  none of what it paints (6–9 paragraphs a frame before). The 19-kind
  fling's fps is unchanged, its main thread 7–23 ms/s lower from 6k to 24k.

**macOS, in stage 1:**
- `readerMeasure` (`RegionReaderMac.swift:428`) runs inside `measure`, now
  on the owner. `RegionReaderTiming.begin` asserts main (`:59`).
- The reader's measurement state becomes owner-owned: `readerParagraphs`
  and `RegionReaderTiming`'s intervals. The assertion becomes an owner
  assertion.
- Main-side consumers reach that state through owner jobs:
  `exact_region_complete`, `exact_text_ready`, and the timing readout.
- `NSFontManager.shared.convert` (`Text.swift:613`) is resolved on main into
  a table at font install. An unseen variant goes through `callMain`.

### 8.2 SVG, motion, images

- **SVG.** `SvgState::emit` is owner. Building layers and adding animations
  (`SvgScene.swift:75-121`, `:195`) is main. Stage 4, if profiled, moves
  `CGPath` and animation-value construction to the owner as values.
- **Motion.** The Rust `Engine` is owner. Frames follow §7.1.
- **Images.** Decode is already off main (`RasterWorkers`) over the
  thread-free raster core. Intrinsic sizes flow main → owner as synchronous
  requests after a batch, deferred by the coordinator's no-reentry rule.

### 8.3 Native modules

`native.call` follows §4. `later` and `changed` hop with `main.async` today
(`NativeModule.swift:217-227`, `:246-253`); `changed` then calls the owner as
§2.3 says. Native-module views are created by the presenter on main, as
today.

### 8.4 Lifecycle, pressure, background (T10)

- **Boot and plan swap are owner transactions.**
  - The font checkpoint, hook installation (measure, fonts, canvas text,
    wake, app module), candidate preparation (`exact_prepare_plan`,
    `exact_prepare_module`) and commit or discard
    (`Session.swift:590`, `:637`) run as one uninterrupted owner job, or as
    a sequence with no other job interleaved.
  - Boot stays synchronous: the first frame is the whole frame, never 1022's
    shell.
- **Callback contexts** (measure, fonts, wake, canvas text, app module) are
  retained by the owner facade from installation until the destroy job
  completes.
- **Destroy:**
  1. stop admitting the session's work, and discard its unstarted fills and
     frame jobs;
  2. resolve any pending `callMain` of the session with a refusal;
  3. run `exact_destroy` on the owner;
  4. release the contexts.
  
  Published batches of the dead generation are dropped.
- **Memory pressure:** the handler (`Session.swift:473`) posts an owner job
  that trims measurement caches, and trims painting caches on main. The same
  split applies to rest trimming (`ScrollPump.restDelay`).
- **Accounting:** `metrics` reports queued batches, bytes, and duplicate
  text storage (measurement's and painting's).
- **Background:** on resign-active, admission of fills and frame jobs stops,
  and in-flight work finishes and is published. On become-active, main
  drains, re-reads geometry, verifies coverage (§6), then resumes
  admission.
- **Cancellation** may discard unstarted work only. It never discards a
  committed batch of a live runtime.

### 8.5 Canvas draws (r6, 2026-09-30)

**What was wrong.** A 2D canvas's due draws ran at the end of whatever turn
came first in a frame (`Host::canvas_turn`, after layout). On a frame with a
due timer that is `exact_advance`, which main waits on (§2.3), so main was
awake for the whole draw: 150–170 ms/s of the feature bench's F2 on an iPhone,
a draw of 1.75–2.9 ms a frame (QUEUE, the canvas lane's trace), with main
doing almost nothing else meanwhile.

**The change.** Where fills are asynchronous (iOS, off the agent), the session
defers canvas draws (`exact_canvas_defer`):

- Every turn still lays canvases out (their geometry goes with the batch that
  moved them), but draws nothing; its batch says `canvasOwed` when
  `Runner::canvas_owed` finds a due draw (a frame request only while the
  canvas is not held), a fresh bitmap to announce, or lists and retired
  generations waiting.
- The last batch applied on main says what is owed (batches apply in the
  owner's order). Once per main-queue turn, after that turn's calls, the
  session posts `exact_canvas_draw` to the owner: geometry, the due draws,
  their lists. Its batch comes back by its own publication (§3.2), like a
  slice's. One is in flight at a time; a draw owed meanwhile is asked for
  when it lands.
- A frame's tick is never waited for here (§7.1), so it draws in its own turn,
  as before. While a tick is in flight no draw is posted: the tick's batch
  says whether one is still owed. An animating canvas on F2 is then drawn by
  the tick each frame, and a canvas that changes for any other reason (an
  argument, a size, an image loaded, a reply) by the draw turn.
- Main is awake for neither. A canvas's pixels land a frame later at most:
  the draw's lists land with the next main-queue turn and replay off main
  (LLP 1056 §8.3), which already put its pixels a frame behind its batch.

**Safety.** The draw already ran on the owner (inside the turn main waited
on); what is new is main running while it runs. The draw's Swift callback is
the Canvas 2D text measurer, which uses the measuring engine's `CanvasText`,
never the painter's (§8.1). Lists cross as memory ranges into Rust
(`Batch::canvas2d`), which `canvas_kept` keeps for two batches; with draws
deferred only the draw turn and the tick produce lists, and neither is posted
again before the last one's batch has landed and been read, so a list main
reads is never the one the owner frees. `OwnerCanvasMacTests` scrolls a list
of canvas rows down and back with slices and draws on the owner, retiring
canvases and making new ones while main replays, under the Thread Sanitizer.
The agent's clock and macOS keep synchronous draws (fills are synchronous
there), so every agent read and parity capture is unchanged.

**Prediction** (F2 on the iPhone 13 Pro Max, canvas lane's harness): main
busy 167–174 ms/s → 25–40 ms/s (its own CPU, 22–29 ms/s, plus the draw's
landing and the tick hop); process CPU within ±5 ms/s (the draw runs on the
owner as before; the frame's tick was already a hop, and a draw turn adds one
only when something other than a frame request is owed); 120 fps unchanged.
The 19-kind Extra Heavy fling: its canvas rows' first draws leave the retire
and measure reports main waits on, a few ms/s of main.

### 8.6 Travel inside the built window (r7, 2026-09-30: built, measured, parked)

**What the report log showed.** With slices on the owner, a list moving inside
its built window made about one report a frame that the runner answered with
nothing: the pump's build-only slice (`travel_within` recognizes it), the
retire-only report every landing forced, and the moved offset any batch's
flush reported again. On the M1 iPad Pro's 19-kind fling: 190 reports/s, 92
of them empty (110/s at 1k pt/s, 80/s at 24k). The empties were cheap where
the log could time them: 4.2 ms/s of owner time and 3.8 of main's wait over
the run (8.7 and 6.5 at 1k); the other 98 reports/s cost 90 ms/s of owner
time and 102 of main's apply. Row work at 1k costs more than twice per row
what it costs at 24k (a non-empty report 1.6 ms of owner time against 0.7):
the cores run slower when the app is nearly idle, so "machinery" that looks
fixed per frame is mostly rows at low clock.

**What was built** (`parked/travel-quiet`, 256f0b694 on eb898ad64). The
runner exports its own rule as a pure function, `travel_stays(offset, port,
velocity, total, first row, last row)`: the led window's two ends fall inside
the mounted run's first and last rows, a point from their edges. The snapshot
says `steady` (exactly `travel_within`'s state: no restored or sought
position, nothing pending, no preview, no correction) and Swift reads each
row's `start`, `size`, `measured`. `CollectionHost.flush` reports nothing for
a moving list that is steady, whose rows are one measured run touching
neither end, whose boxes are what the index holds (gathered once per batch,
not per report), whose port, width and pins are as last reported, when
`exact_collection_stays` says the window it leads to is the rows mounted.
Instead it leaves the offset and its scroll sequence in the runtime façade
and the owner takes them (`exact_collection_travel`, the effect
`travel_within` would have had) at the start of every job for that runtime,
so a data change's anchor, a kept inner position, an into-view step and the
agent's reads see where the list is; at rest nothing is owed and no timer
runs. A landed slice owes a retire-only report only when its snapshot says
`pending`. The iOS pump skips such a list before its `rowsToCover` walks.
`EXACT_TRAVEL_REPORTS=1` reports every frame. Tests: the runner's
differential (two harnesses over 1,200 frames with reversals, an item
arriving at the front of the data and a row growing above the port
mid-travel: rows, extent and corrections equal every frame; dropping the note
or widening the rule fails it), the host fixtures, and a real runtime with
slices on the owner where a press gives every row a paragraph mid-travel and
the row at the port's top stays put.

**Measured** (iPad, 3 interleaved rounds each, `results/cpu/ipad/ab3-travel*`):
reports 190 → 124/s, empties 92 → 26/s. The 19-kind fling: process CPU 636.7 →
623.8 ms/s, main 277.3 → 274.5, fps 117.5 → 117.6, late 1.1 → 1.0/s, blanks
0; by speed −19/−32 at ±1k, −32/−11 at ±3k, 0 ± 5 from 6k up (the skip takes
there too, reports 169 → 110/s at 6k, but those empties cost nothing to
speak of). The live fling: 660.3 → 649.6, main 293.3 → 289.5. The inbox
innerfling: 567 → 564. **Parked** under the program's threshold (−15 ms/s
of CPU or −8 of main): the empties were never the fixed cost; rows are.
What stands from it: the log (a diagnostic build, never landed) and the
finding that `CollectionHost`'s per-report cost is the rows' boxes gathered
again for each report, which the parked branch fixes and a later change can
take on its own.

**macOS, measured by the mac lane (2026-09-30, bones M4 at 60 Hz, 19-kind feed,
three rounds with the order alternated, machine-state gate, table at
`~/bench/xheavy-mac/results/ab1-bones/abtable.md`).** Fling: process CPU 815
with the skip, 815 reporting every frame, 819 on main; main thread 393 / 393 /
392; 58.9 fps all; the inbox fling 436/314 vs 437/316 vs 436/314 — nothing
within noise on macOS's synchronous fills, so the unpark threshold is not
reached there either. **At rest** (10 s, nothing moving) the branch costs +14
ms/s of process CPU and +14 of main (178/81 against main's 164/67; 167/71 with
reports on), in all three rounds and both orders: something in it runs at
rest that main does not. The likely cause, from the diff and unverified: a
travelling list whose report is skipped stays in `dirty`, and the branch's
added `if !dirty.isEmpty { schedule() }` reserves a main-queue turn every
turn, so at rest a continuation runs each turn and does nothing — the shape
of the in-flight spin the spin fix (f7aa53049) removed. Whoever reopens the
branch verifies that first.

## 9. Amending LLP 1050.000 D3 (ruled, Q5)

D3 today: "a row may take longer than a frame, but never mid-fling." Amended:

> During user motion a row may be built when the main-thread work it
> leaves fits the remaining frame budget: its apply, its CA commit share and
> any callbacks it makes. Where the build runs off the main thread
> (LLP 1072), the build's own time does not count against the frame; owner
> occupancy is admission's concern (1072 §3.4), not D3's. A row whose
> main-thread work does not fit is pending until motion slows, as before.

**Scope, before stage 3 builds anything.**
- The collection API has no whole-row cost memo today
  (`runner/src/instance/collection/api.rs:43`). D3's "known cost" is not
  implemented, so stage 3 introduces it: a per-row-key memo of main-thread
  apply cost, measured by the coordinator.
- It is separate from 1068 §5.1's heavy-leaf hold, which stays as ruled
  (`1068:698`).

The amendment is written into 1050.000's D3 as a dated note pointing here.

## 10. Failure modes and detection

| failure | shows as | detection |
|---|---|---|
| deadlock between owner and main | a frozen app | §4's one door. `Main.sync`'s debug trap and the build-time source check. The 250 ms watchdog with both job names |
| synchronous reentry from a serviced callback | a refused call | the named `busy` refusal and a journal line. Stage 1 runs native-module apps (the map module) and asserts zero |
| out-of-order application | a missing parent, a stale style | owner sequence stamped on every batch. The coordinator asserts it increases (debug) and journals a gap (release) |
| stale geometry dropped for good | a list stops refining | the `stale` flag and dirty retry (§3.3). A counter of stale rejections and retries |
| an event on a row an asynchronous commit destroyed | `UnknownView` | §5: asynchronous fills only create, and every destruction is synchronous. A counter of `UnknownView` refusals from list rows; the gate is zero |
| an unintended gap | a blank band | §6's verification counters. The probe's blank count |
| a stale batch after reload or destroy | old-plan views | the generation tag. The reload smoke |
| owner-only state touched on main | a crash or rare corruption | `Owner.assertOwner()` (pthread identity) on owner-only entry points in debug builds. The §2.3 audit |
| text measured and painted differently | parity diffs | the SVG, canvas and motion parity smokes; the web oracle |
| hop tax on non-list apps | main ms/s up | stage 1's gate |
| 1022's flake | seek or capture smokes fail | 8 of 8 A/B against a baseline worktree before landing |

## 11. Staging and gates

Order: **1 → 2 → 4 (if profiling warrants) → 3 → 5**.

- Device numbers come from the Extra Heavy probe: 3+ rounds on the iPhone 13
  Pro Max and the M1 iPad Pro, against SwiftUI in the same batch.
- Builds and full checks run on the M5 mini (`mini-verify`). Device runs and
  local macOS parity run on the main Mac.
- Each stage lands alone, on its gates.

**Stage 1: the owner, everything synchronous.** It covers T1, T2, T4, T5,
T8, T10, §8.1's macOS fix and the §2.3 audit.
- **Paths:** all Apple paths green:
  - the five checks, the Apple XCTests and `smoke.mjs ios`, `macos`, `host`,
    `host-ios`, `svg`, `canvas`, `motion`;
  - the large Markdown reader on macOS (LLP 1044's bench);
  - a native-module app (the map);
  - dev-loop reload;
  - two sessions in the sample host.
- **1022's invariants:** the motion seek across the timer and the canvas
  capture counts, 8 of 8 A/B against a baseline worktree.
- **Boot:** settled first-frame time (1022's metric) within noise on macOS
  and iOS.
- **Main-thread CPU and blocked wall, reported separately.** CPU falls,
  because Rust and decode move. Blocked wall is what the hop costs.
  - Round trips: count per second, p50 and p99.
  - Budget: at most 3 ms/s of added blocked wall on the Caltrain scroll and
    the live feed at rest. At 120 calls/s that is **25 µs per round trip**.
  - Measured counts and tails decide.
- **Two-session latency gate** (Q1): in the sample host, one session
  scrolling a heavy list beside an interactive one. The interactive
  session's input-to-apply p95 must be within 2 ms of it running alone.

**Stage 2: asynchronous fills on iOS.** It covers T3, T6, T7, T9, T12,
`CollectionFeedback` v4's create-only mode, the runner's `evict_owed` and
`edge_owed`, and the ABI version bump (§5). Eviction's blocked main time is
reported on its own line.
- iPhone live fling: at most 3 late frames per second (from 8–9; SwiftUI
  1.1).
- Unintended gaps (§6): 0.
- The 19-kind feed and the iPad no worse on any column.
- **Distributions reported:**
  - queue wait, build, publication delay and apply;
  - owed-wait count and duration;
  - input-to-visible latency, tap to changed pixels, p50 and p95, no worse
    than stage 1;
  - presentation cadence: frame interval histogram;
  - sustained rows/s produced against rows/s needed (§3.4, stage 2b if
    short).
- **Forced interleavings, with publication held** (§5's hook) at each hold
  point: an event, a data reply, a timer, a resize, a pin change and a
  reload, each against a fill. The tree and state must equal the
  `EXACT_FILL_SYNC=1` run at the next settle.
- The differential XCTest (a scripted scroll with and without
  `EXACT_FILL_SYNC`) identical at every settle.
- §5's interaction tests pass.

**Stage 4, if profiling warrants: the Swift apply's movable parts.** SVG
`CGPath`s and animation values, and flat-leaf geometry, as values built on
the owner. The gate is a measured fall in main apply plus CA commit per row,
with pixel parity (the svg and canvas smokes) and motion parity.

**Stage 3: D3 as amended (§9).** Scope first: the per-row-key main-apply
memo is new. Gate: heavy kinds keep their deferral only where their
main-thread work exceeds the remaining budget, measured on the iPad 19-kind
ladder and fling.

**Stage 5: macOS asynchronous fills** in `PresenterMac`'s pump, checked on
physical Mac scrolling:
- trackpad and wheel;
- reversal, jumps, resize, anchors;
- text input in rows;
- LLP 1044's hitch counts and the macOS smokes 8 of 8 A/B.

**What stops the lane:**
- stage 1's blocked-wall budget or the two-session gate is missed and
  cannot be met in three rounds;
- stage 2 cannot hold zero unintended gaps at 6k pt/s on the iPhone;
- 1022's flake returns and cannot be traced in three rounds.

## 12. What LLP 1022 found, and how this differs

| 1022 finding | here |
|---|---|
| 1. The macOS smoke failed about 6 of 8 (seek, capture) | Almost everything stays synchronous. The two asynchronous operations are drained at every agent boundary, frame jobs are synchronous under the agent, and settlement answers `settled: false` rather than pretend. Those smokes are the acceptance test, 8 of 8 |
| 2. Boot became an 11-view shell | Boot is a synchronous owner transaction (§8.4) |
| 3. The iOS startup win was separable | Not claimed |
| 5. The wasm is synchronous forever; the agent's contract becomes a tax | The web and Linux stay the oracle. The tax is confined to two named operations, the coordinator and the fixed point |
| 6. "The problem it solves has not arrived" | It has (§1) |

The parked branch (`parked/runtime-owner`, `2583be3`) is a reference for its
FIFO and `barrier()`, not a base. Its keyboard half stays parked.
