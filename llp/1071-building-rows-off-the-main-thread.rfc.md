# LLP 1071: Building list rows off the main thread

**Type:** RFC
**Status:** Draft (r1, for a directional review before any build)
**Systems:** Apple host (`Bridge.swift`'s `Runtime`, `Session.swift`'s `wire`/`apply`/`Frames`, `Collection.swift`, `IOS/ScrollPumpIOS.swift`, `Mac/PresenterMac.swift`'s pump, `Agent.swift`, `Text.swift`, `NativeModule.swift`), Apple Rust library (`abi.rs`'s registry and `with_runtime`, `abi/exports.rs`, `markup.rs`, `textflow.rs`; no runner or kernel change), Agent API (LLP 1012: no new operation), Web host (none), Linux host (none)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** unassigned until the review; the `perf/node-cost` lane is the natural one
**Date:** 2026-09-28
**Related:**
- Charlie's ruling, 2026-09-28 (through the coordinator): "move the mount off the main thread … leaving only the UIKit/CA apply on main. Resumable rows are not chosen."
- `QUEUE.md` "A heavy list row still mounts as one lump" and its 2026-09-28 diagnosis (commit `ee0f8b70`): the evidence in §1.
- LLP 1022 (the serial runtime owner, measured and parked 2026-08-30): the precedent this revives in a narrower form; its findings are this RFC's acceptance tests (§9).
- LLP 1044 F4/F5/§3 (every piece of list work on main, synchronously; "ours has one thread, by design") and 1044.000 §5 item 8 ("the engine stays on the main thread until a physical 120 Hz run under load shows main-thread misses"), §5 item 2 ("CoreText objects stay with the worker that made them"), §8 ("no runtime-owner thread"). This RFC meets item 8's bar (§1) and replaces §8's line.
- LLP 1050.000 D1 (never blank), D3 (a costly row is never built mid-fling; resumable rows a separate project), §2.4 ("closing it needs owed rows to be built off the frame, either by resumable rows or by moving work off the main thread"), §6 ("the runner stays deterministic and the host owns time").
- LLP 1068 §4.9 (incarnation tokens on async callbacks), §6.2 (the remaining lump: runner report, host commit, batch decode).
- LLP 1010 §6.5 (feedback is versioned; stale revisions, sequences and epochs cannot overwrite newer geometry), §6.2 ("AppKit and UIKit settle windows before paint").
- LLP 1012 (the agent's operations; "the call returned, therefore it settled"), LLP 1016 D2 (replies wake the runner through the host), LLP 1027.002 (worker-placed data sources), LLP 1031 D2 (the `u32` handle ABI).

## Summary

On the iPhone 13 Pro Max's live-only feed, every late frame is one list row's
mount: 14.7 ms of main-thread work in one runloop turn, of which 8.9 ms is
Rust and decoding (the runner's report, the host commit with layout, the batch
decode) and 5.9 ms is the Swift apply. SwiftUI spends more on the main thread
but never more than 16.7 ms in a turn.

This RFC moves the 8.9 ms to one **owner thread** that holds the whole Rust
runtime for its life. The main thread keeps UIKit, Core Animation and the
presenter.

- **Every call stays synchronous except one.** Events, timers, replies, boot,
  the agent and every read block main until the owner answers, exactly as
  they return today. The runner is never touched from two threads, needs no
  `Send`, and gains no `unsafe`.
- **The exception is the collection fill.** While a list moves, the scroll
  pump posts a fill to the owner and returns. The owner runs the report, the
  commit, layout and the decode. The decoded batch comes back to main and is
  applied, in the owner's order, at most one fill per frame.
- **Rows are asked for earlier.** The lead covers the owner's build time plus
  one frame. A row the scrollport would show before its batch arrives is
  **owed**. Main then waits for it, as it does today (LLP 1050.000 D1).

The web and Linux hosts keep today's single-threaded path. They are the
synchronous oracle the Apple host must equal under the agent. macOS shares the
Swift and adopts async fills after iOS (stage 5).

**Recommended decisions** (§10 asks the questions):

| # | Decision |
|---|---|
| T1 | One owner thread per process holds every Rust runtime from creation to destruction. The registry, the runner, the kernel, the host, the data source and every Rust thread-local live there. No `Send`, no `unsafe impl` |
| T2 | Every C call is a synchronous request from main to the owner, except the collection fill (T3) and a display-link tick that meets a fill in flight (T6). Main blocks on the reply and services callbacks the owner sends it while it waits (T7) |
| T3 | The collection fill is asynchronous during motion. At most one fill is in flight per session. Its decoded batch is published to main |
| T4 | Main applies batches in the order the owner produced them. A synchronous call first applies every fill batch published before it. Fill batches wait for the pump, one per frame |
| T5 | Owed rows are never late. A fill whose rows the scrollport needs by the next frame is awaited on main (T4's drain). A hole rescue in the scroll callback is a synchronous fill, as today |
| T6 | A display-link tick never waits on a fill. If a fill is in flight, the tick is posted, coalesced to one, and its frame applied when it arrives |
| T7 | Nothing on the owner waits on main while main waits on the owner. Callbacks that need main (`native.call`) are run by main's wait loop |
| T8 | Batch decode runs on the owner. Main receives a decoded `Batch` |
| T9 | Under the agent, every carrier request starts with a drain: in-flight fills finish and apply before the operation reads or acts. `clock settle` drains to a fixed point |
| T10 | Text measurement runs on the owner (CoreText, `UIFont`). `TextEngine`'s measurement side belongs to the owner. What main paints from it crosses as plain values under one lock. macOS's `NSFontManager` conversion is resolved on main ahead of time (§6) |
| T11 | Web and Linux are unchanged. macOS adopts T3 in stage 5, behind 1022's smoke invariants |
| T12 | `EXACT_FILL_SYNC=1` runs fills synchronously on the owner. It is the A/B and differential-test switch, not a user setting |

## 1. The evidence (why now)

LLP 1044.000 §5 item 8 kept the engine on main "until a physical 120 Hz run
under load shows main-thread misses". The run was taken on 2026-09-28: System
Trace on the iPhone 13 Pro Max (120 Hz), live-only feed, fling at 1k–6k pt/s,
on origin/main `558a5bd8`. Data and scripts are in
`~/bench/xheavy/results/late/iphone/`.

| | exact2 | SwiftUI |
|---|---|---|
| main-thread busy, ms/s | 350 | 408 |
| runloop turns > 8.33 ms, per s | 11.0 | 5.4 |
| turns > 16.7 ms, per s | 5.4 | 0 |
| median long turn | 15.9 ms | 11.5 ms |
| what long turns are | 94% one `ScrollPump` slice mounting about one row | CA commit 61%, `TimelineView` display link 36% |
| probe late frames per s (fling, 3–9 rounds) | 8–9 | 1.1 |

One live row's mount on the main thread (average over 132 long turns):

| part | ms | moves under this RFC |
|---|---|---|
| runner report (`apply_document` 1.5, instance creation 1.1) | 2.9 | yes |
| host commit (layout 1.5, create 0.8, motion 0.9, SVG emit 0.5) | 4.6 | yes |
| batch decode (`BatchReader`) | 1.4 | yes (T8) |
| Swift apply (flat leaves 2.1, SVG scene + animations 1.2, pool 0.6, …) | 5.9 | no (§8 stage 4 trims it) |

The live clock is off in the fling, in both apps. With `BENCH_LIVE=1`,
exact2's late frames do not change (201–205 in 24 s), while SwiftUI's rise to
50–62, locked to its tick. Keyframes are set up once per mount (0.6 ms) and
never restarted. The cause is the lump. The 19% per-node cut
(`perf/node-cost`, landed `558a5bd8`) lowered the lump but did not move the
late frames.

What moving 8.9 ms buys: the row's main-thread cost falls to about 6 ms plus
CA commit. That is under the 8.33 ms turn in the common case. Stage 4 targets
the Swift apply for the rest.

## 2. What runs where

### 2.1 The owner thread (T1)

There is one `Thread` named `exact.owner`, at QoS `userInteractive`. It is
created at the first `exact_create` and lives for the process. Its loop
drains a FIFO of jobs. It must be a dedicated thread, not a serial
`DispatchQueue`. A serial queue may run on different pool threads, and the
registry (`abi/exports.rs:25-27`) and five other thread-locals are keyed by
thread.

The owner holds:
- **The registry and everything in it:** `Bridge<D>`, `Host<D>` with its
  `Runner` and `Kernel`, the motion `Engine`, `SvgState`, the executor's
  sender, the in/out buffers.
- **The data source `D`.** It stays on the thread it was created on, so
  `DataSource` needs no `Send` bound. Worker-placed sources (LLP 1027.002)
  already hand envelopes back to "the runner's thread"; that is now the
  owner.
- **The Rust thread-locals:** `REFUSAL` (`abi.rs:1384`), `markup.rs:59`,
  `textflow.rs:107`, `stdlib.rs:280`'s scratch, and the collection
  `PinEpoch` (`nest.rs:378`). Each one keeps its meaning because every
  access is on the owner. §6.4 lists the Swift calls into `markup_*` and
  `textflow_*` exports that must become owner requests.

**Why not the runtime moving between threads (a lease).** An exclusive
hand-off with a `Mutex` would let the fill run on a worker and everything
else on main. It needs an `unsafe impl Send` over a graph of `Rc`s, and an
audit that nothing outside the runtime holds a clone. It also needs every
thread-local above moved into the runtime; `PinEpoch`'s counter would
otherwise go backwards across threads and the pins cache would hit falsely.
It buys nothing the owner does not: main is blocked during either model's
synchronous calls. Rejected.

**Why not a second runtime for rows.** A row is built against the session's
state; a copy would fork it. Rejected, as 1044.000 §8 rejects "a second state
graph".

### 2.2 Main (T2)

Main keeps:
- UIKit and AppKit.
- The presenter, `CollectionHost` (its facts, cursor, snapshots), the scroll
  pump, `Frames`, the raster loader, the text rasterizer's publication.
- The agent carrier's execution (it already runs each request on main,
  `Agent.swift:62-90`).

Every Swift→Rust call in `Bridge.swift`'s `Runtime` becomes
`owner.sync { … }`. That call does three things:
1. It enqueues the closure.
2. It waits on a condition that wakes for the reply *or* for a callback the
   owner posted for main (T7).
3. It returns the decoded result.

The call sites in `Session.wire` (`Session.swift:514-560`) do not change
shape. `apply(runtime.press(id, now: now()))` still reads as a synchronous
call and still is one.

**The hop costs about two thread wakes.** It is paid about 120 times a
second while a display-link tick runs, and once per event otherwise. Stage 1
measures it (§8).

### 2.3 The asynchronous fill (T3)

`CollectionHost.flush` (`Collection.swift:362`) gathers facts on main:
- the scrollport geometry;
- the focus and interaction owners;
- the measured sizes of mounted rows, read from applied views (`:390-406`).

These are plain bytes (`CollectionFacts.encode`). In a *fill* (the scroll
pump's `fillSlice` during motion) they are posted to the owner with
`owner.async`. The owner then runs `exact_collection_feedback`: the report,
the commit, layout with text measurement, `present`, `finish`, the decode.
It publishes the `Batch`, tagged with:
- the session's generation (a reboot or destroy bumps it; LLP 1068 §4.9's
  rule);
- the owner's sequence number.

Only one fill is in flight per session. The pump does not post another until
the batch is applied. A second list's dirty report waits for the same slot.

What stays synchronous:
- Reports that are not fills: at rest, after a batch, a resize, pins
  changing, `dataReady`.
- The scroll callback's hole rescue (`Collection.swift:285-290`).

Those are rarely large. Keeping them synchronous keeps LLP 1010 §6.2's
"settle windows before paint" true. Stage 2 counts how many large reports
remain synchronous.

## 3. Handing a built row to main (T4)

**Published batches.** A batch is an immutable value: `Batch` and its
`BatchOp`s. Publication is a mutex-guarded queue on the owner side, plus one
`DispatchQueue.main.async` wake. The payload dictionaries are immutable after
decode, so the hand-off is safe under the queue's lock. They cross in an
`@unchecked Sendable` box whose one invariant is "written once before
publish". `StyleCache` (`BatchReader.swift`) already has its lock; it becomes
owner-only.

**Order.** Batches are applied in owner sequence, always.
- Every synchronous call first applies any published but unapplied batches,
  then its own.
- A fill batch is therefore never applied after a batch the owner produced
  later.
- This is the only ordering rule the presenter needs. Every batch it applies
  is one the runner committed, in commit order, as today.

**Budget.** The pump (`ScrollPumpIOS.swift:187`) applies at most one fill
batch per display frame, inside `pump()`. It applies it at the start of the
slice, before it posts the next fill, so one frame's main-thread work is one
row's apply. When the owner has published several (a slow main), the rest
wait for the next frames unless one is owed (T5).

**What main does not do.** It never partially applies a batch. A batch is one
runner commit: ids, children and styles that refer to each other. Splitting a
row's apply across frames is resumable rows by another name, and that is not
chosen.

## 4. Consistency

The owner is the only place the runtime's state changes, and it changes in
FIFO order. Every question below is about wall time, not state.

**An event arrives while a fill is building.** A tap, a key, a text change:
the event's `owner.sync` queues behind the fill. Main waits for the rest of
the build (at most one row's 8.9 ms, usually less), applies the fill batch,
then the event's batch. The runner saw fill-then-event. That is an order the
synchronous host could also have produced; the fill would have run first had
the pump's slice come first. Input latency is bounded by one build; today it
is bounded by one whole mount (14.7 ms).

**The row being built cannot be tapped.** It has no views on main until its
batch applies. A tap on a mounted row names a view id that the fill cannot
retire. Retirement happens in the owner's commit and reaches main in the
batch. If a fill's commit retires the tapped row's view, the owner handles
the event after the fill, against the new tree. The runner
handles an event on a retired view as it does today, when a
scroll-triggered report retires a row a moment before a tap. Stage 2's tests
include that case.

**Data updates, replies and timers.** The executor's wake
(`Session.swift:503-509`) posts `apply(runtime.pump)` to main, which becomes
`owner.sync(pump)`. It queues behind a fill like any event. A reply that
changes a row's data therefore commits after the fill that built the row,
and its batch updates the row just built. The same holds for timers
(`SessionClockTimer`, `Session.swift:1075-1080`).

**Geometry that moved while the row was built.** The facts in a fill are the
scroll state at post time. By apply time, UIKit has scrolled one or two
frames further. The runner's window was computed with a lead (T5) that
covers the build time, so the rows are still ahead of the viewport.

**The next report uses the newer state.** The next report carries the new
offset and the sizes measured from applied views. LLP 1010 §6.5's revision,
sequence and epoch checks (`prepare_feedback`, `collection/mod.rs:858-893`)
already drop a report whose revision a batch superseded. Main never posts a
report while a fill is in flight (§2.3), so a report never races a batch it
has not seen.

**Pins.** Focus and interaction owners are read on main at post time and
sent in the facts, as today. A focus change during a build dirties the list.
The next report, after the fill applies, carries it. Pins that must move
immediately use a synchronous report (`pinsChanged`, `Collection.swift:326`).

**Non-list updates.** Nothing else is asynchronous (T2), so a non-list
update's batch is always applied in its call. That includes an action's
batch, a navigation, and a style change from an event.

## 5. The agent, the clock and tests (T9)

LLP 1012's contract is "the call returned, therefore it settled". LLP 1022
failed it because boot, ticks and events all became asynchronous. Its smoke
flaked on the motion seek across the timer and on canvas captures.

Here, every operation except the fill stays synchronous. Fills are drained at
the carrier boundary:

- **Before each carrier request** (`Agent.swift:62-90`), main calls
  `owner.drain()`. That call:
  1. waits until no fill is in flight;
  2. applies every published batch in order;
  3. repeats if an applied batch scheduled another fill (bounded, as
     `ScrollPump.settle` is: 8 rounds).
- **`clock settle`** already runs `presenter.settlePump()`
  (`IOS/PresenterIOS.swift:472-476`), whose `ScrollPump.settle` fills with an
  unlimited `fillSlice` (`ScrollPumpIOS.swift:210-218`). Under this RFC
  `settle` fills are synchronous (T5's owed path). `Agent.clock`'s loop
  (`Agent.swift:327-406`) gains the drain at the top of each round.
- **The clock stays virtual.** A fill carries `now` from main
  (`collectionFeedback(bytes, now:)`), and under the agent `now()` is the
  agent's clock (`Session.swift:512`). The runner neither presents frames nor
  reads a clock (LLP 1050.000 §6). A fill built later in wall time is the
  same fill.

With the drains, an agent sees exactly the synchronous host's states at
every request boundary. The web and Linux hosts, which never go async (T11),
are the oracle for that claim.

**Tests:**
- **Runner and kernel:** unchanged. The runner is not threaded. The
  incremental differential test (`runner/tests/it/incremental.rs`) keeps
  covering feedback ordering, including stale reports.
- **Swift (XCTest), new:**
  - An `Owner` unit test: FIFO order, sync-after-async ordering (a sync
    call's batch is applied after every earlier fill's), the callback
    service under a synchronous wait (a `native.call`-shaped callback during
    a sync request does not deadlock), and drain.
  - A host test that runs a scripted scroll over a collection fixture twice,
    with `EXACT_FILL_SYNC=1` and without. It asserts the same agent `tree`
    and the same collection `state` at each settle.
  - The existing collection tests (`CollectionTests.swift`,
    `CollectionMacTests.swift`, `CollectionFillMacTests.swift`,
    `NodePoolIOSTests`, `FlatLeavesIOSTests`) run on the owner model. The
    ones the survey marks as order-sensitive stay green unmodified:
    `testCorrectionLandsBeforeDeferredAuthoredEvent`,
    `testFeedbackMembershipCommitsContinueOnLaterTurnsWithoutScroll`,
    `testCancelledPostScrollSliceCannotConsumeNewWork`.
- **Smokes:**
  - `smoke.mjs ios`, `macos`, `host`, `host-ios`, `svg`, `canvas`, `motion`.
  - 1022's two failure modes are explicit acceptance tests: the motion seek
    across the timer, and canvas capture counts, each 8 of 8 against a
    baseline worktree's 8 of 8.
  - `smoke.mjs` has no virtualized-list step today. The Extra Heavy feed's
    probe (out of repo) and the differential XCTest cover lists.

## 6. What changes for each subsystem

### 6.1 Text measurement (T10)

`TextEngine.measure` (`Text.swift:1078-1150`) is CoreText:
`CTTypesetterCreateWithAttributedString`, `SuggestLineBreak`,
`CreateLine`. It uses `CTFont`s from `TextEngine.font` (`:561-615`). CoreText
objects are thread-safe to create and use from one thread. `UIFont` objects
are immutable and usable off main.

`TextEngine`'s state is unsynchronized: `fonts`, `residency`, the counters.
It is read on main by painting:
- `measuredBreaks` (`:715-720`);
- `TextRaster.render(lines:)`, which asserts main when it reuses cached lines
  (`TextRaster.swift:53`).

The split:
- Measurement and its caches (the font table, `residency`) are owner-only.
- What painting needs crosses as plain values, following 1044.000 §5 item 2.
  These are line ranges and baselines (`measuredBreaks` already returns
  them) through one small lock around the break cache.
- Main never touches an owner `CTLine`. The reuse path in `TextRaster`
  re-shapes from the plain breaks on main, or on the existing text raster
  workers.
- Stage 1 measures what this costs painting. If re-shaping is visible, the
  cached lines are copied into the published batch for the rows it created
  instead.

**macOS:** `NSFontManager.shared.convert` (`:613`) is not documented as
thread-safe. The plan's font variants are a finite set, known at boot. Main
resolves them into a table the owner reads. An unseen variant is resolved by
a callback to main (T7).

Canvas text (`exact_set_canvas_text`, `Canvas2DText.swift:31-103`) is
CoreText and "called on the runtime's thread" (`abi/exports.rs:49-57`):
unchanged, now on the owner.

### 6.2 SVG, motion, images

- **SVG:**
  - `SvgState::emit` (`svg.rs:184`, from `present`) is Rust: owner.
  - Building the `CALayer`/`CAShapeLayer` tree and adding
    `CAKeyframeAnimation`s (`SvgScene.swift:75-121,195`) is the presenter
    apply: main.
  - Stage 4 may build `CGPath`s and the animation values on the owner (they
    are Core Graphics values, not layers) and hand them over in the batch.
- **Motion:**
  - The Rust `Engine` (`host.rs:123`) is owner-only. It advances in every
    commit, fills included.
  - `Frames.tick` (`Session.swift:1142-1164`) is a synchronous request unless
    a fill is in flight. Then it is posted, at most one outstanding (T6), and
    its frame ops apply when published. A Rust-sampled value can therefore
    lag one frame while a row builds. Today the whole frame is lost.
  - CA-lowered keyframes are unaffected.
- **Images:**
  - Decode is already off main (`RasterWorkers`, `RasterLoader.swift:143-172`)
    over a process-global `Mutex` core (`raster.rs:111-113`). That is
    independent of the registry: unchanged.
  - Intrinsic sizes flow main→Rust after a batch (`Session.swift:539`) as a
    synchronous request.

### 6.3 Native modules and other callbacks (T7)

`nativeCallCallback` runs inline on main or does `DispatchQueue.main.sync`
from another thread (`NativeModule.swift:232-244`).
- **During a fill (async):** main is free, and `main.sync` is safe.
- **During a synchronous request:** main is blocked in `owner.sync`, and
  `main.sync` would deadlock.

The rule: the owner never calls `DispatchQueue.main.sync`. It calls
`Owner.callMain { … }`. That enqueues the closure for main and waits. Main
runs it either:
- in its wait loop, if main is inside `owner.sync`; or
- from a `main.async` hop, if main is free.

`native.call` is changed to use it. The same helper serves macOS's unseen
font variant (§6.1).

A debug build asserts that the owner never calls `main.sync`. It does this
with `dispatchPrecondition(.notOnQueue(.main))` in the owner's callbacks, and
a check in `callMain` that the caller is the owner.

`later` and `changed` already hop with `main.async` (`:217-227,246-253`):
unchanged.

### 6.4 The other ABI entry points

- **`markup_*` and `textflow_*`:** their thread-local tables
  (`markup.rs:59`, `textflow.rs:107`) are filled by the host's commit and
  read by Swift. Every Swift call into them becomes an owner request.
- **The raster ABI:** global `Mutex`: unchanged.
- **The macOS regions ABI** (`text_ready`, `region_complete`,
  `abi_collections.rs:17-49`): owner requests.
- **The executor's `WakeFn`:** already "must enqueue asynchronously"
  (`executor.rs:9-11`). It keeps waking main, which requests `pump` on the
  owner.

### 6.5 Boot, reload, destroy

Boot is a synchronous request: main waits, as today. The first frame is the
whole frame, not a shell. That answers 1022 finding 2, which measured the
async shell at 11 views where the synchronous boot delivered 275.
- **Fonts:** installed inside boot, on the owner (`Text.swift:1155`).
- **Destroy:** a synchronous request queued after any in-flight fill.
- **A published batch for a dead generation** is dropped on arrival.
- **The dev loop's reload** keeps its order: boot the new plan, then reset
  the presenter.

1022 finding 3's pre-`UIApplicationMain` prepare overlap becomes possible
(a one-shot async `prepare` on the owner). It is not part of this RFC.

## 7. Failure modes and detection

| failure | how it would show | detection |
|---|---|---|
| deadlock: the owner waits on main while main waits on the owner | the app freezes | T7's rule and debug assertion. A watchdog in debug builds logs any `owner.sync` wait over 250 ms with the owner's current job name |
| a batch applied out of owner order | a missing parent, a stale style, a view reused under the wrong id | batches carry the owner sequence. Main asserts it is increasing (debug) and logs a gap (release journal) |
| a stale batch after reload or destroy | views from the old plan appear | the generation tag drops it. The dev-menu reload smoke |
| main starved of fills (the owner behind) | blank bands or late owed rows | the probe's blank count (must stay 0 under `complete`). A new counter: owed waits per second and their total ms |
| main waiting on fills (sync behind async) | input latency, late frames on events | a new counter: `owner.sync` wait ms/s on main, split by whether a fill was in flight |
| an owner-only object touched on main | a crash, or a rare corruption | `dispatchPrecondition(.onQueue)` isn't available for a thread, so `Owner.assertOwner()` compares `pthread_self` in debug builds. A lint (`grep`-level, in `caps.mjs`'s style) that `exact_*` calls appear only in `Owner`-wrapped code in `Bridge.swift` |
| text measured differently on the owner | layout parity diffs | the SVG, canvas and motion parity smokes, and the web parity oracle (T11) |
| the hop tax regresses non-list apps | main ms/s up in the Caltrain and Markdown scroll benches | stage 1's gate (§8) |
| agent flake (1022's) | the seek or capture smokes fail | 8 of 8 A/B against a baseline worktree before any landing |

Regression watch after landing: `bun scripts/metrics.mjs --long` gains the
hop count and cost from a scripted Caltrain run, if the review wants it in
the repo (it is apparatus: a human's word). The Extra Heavy probe runs stay
out of repo, as today.

## 8. Staging and the measurement that proves each stage

All device numbers come from the Extra Heavy probe (`~/bench/xheavy`), 3+
rounds, iPhone 13 Pro Max and M1 iPad Pro, against SwiftUI in the same
batch. Each stage lands alone, on its gate.

| stage | what | proves it |
|---|---|---|
| 1 | The owner thread with everything synchronous (T1, T2, T7, T8, T10, §6.4). No async fill yet | Behaviour is unchanged: every check and smoke green, 1022's seek/capture 8/8 A/B. The hop's cost is measured: calls/s and main ms/s on the Caltrain and live feeds, iPhone fling within noise of main. Boot time-to-settled-frame within noise (1022's metric). Decode is now off main, so main ms/s should fall by about 1.4 ms per row |
| 2 | Async fills on iOS (T3–T6, T9, T12), the lead widened by the measured build time | iPhone live fling: late frames per s from 8–9 toward SwiftUI's 1.1 (gate: ≤ 3), main ms/s down by about 8.9 ms per row, blank count 0, owed waits counted. The 19-kind feed and the iPad no worse. The differential XCTest identical. Smokes 8/8 |
| 3 | 1050.000 D3's cost memo measures the row's main-thread apply, not its build. A row whose apply fits the frame is built mid-fling | Heavy kinds (video, web, map) keep D3's deferral only where their *apply* exceeds the frame. iPad 19-kind ladder and fling |
| 4 | The Swift apply's movable parts on the owner: SVG `CGPath`s and animation values, flat-leaf geometry (values, not layers) | Main per-row apply from 5.9 ms down; late frames on the 19-kind feed |
| 5 | macOS adopts async fills in `PresenterMac`'s pump | The Markdown scroll bench of LLP 1044 (hitches/s), macOS smokes 8/8 A/B |

Linux and the web are not staged. §10 Q4 asks whether Linux should follow.

What would stop the lane:
- Stage 1's hop tax exceeds 3 ms/s on the Caltrain scroll, or boot regresses
  by more than noise.
- Stage 2 cannot hold blank 0 at 6k pt/s on the iPhone.
- 1022's flake reappears and cannot be traced within three fix rounds.

## 9. What LLP 1022 found, and how this differs

| 1022 finding | here |
|---|---|
| 1. The macOS smoke failed about 6 of 8 (seek, capture) | Events, ticks at rest, boot and the agent stay synchronous. Only fills go async, and the carrier drains them. Acceptance: those two smokes 8 of 8 |
| 2. Boot became an 11-view shell | Boot is synchronous (§6.5) |
| 3. The iOS win was separable | Not claimed. The win here is the fling's late frames, measured (§1) |
| 5. The wasm is synchronous forever; the agent's contract becomes a tax | The web stays the oracle (T11). The tax is confined to one operation, the fill, and one rule, the drain |
| 6. "The problem it solves has not arrived" | It has (§1) |

The parked branch (`parked/runtime-owner`, `2583be3`) is a reference for the
FIFO and `barrier()`. It is not rebased. Its keyboard half is independent and
stays parked.

## 10. Questions for the review

1. **One owner per process or per session?** This RFC says per process:
   simple, and the brownfield host (LLP 1031) embeds two sessions that would
   then share one thread. Per session gives two sessions parallelism and
   costs a thread each.
2. **Is `EXACT_FILL_SYNC` wanted in the repo** (T12)? It is the differential
   test's switch and the A/B lever. It is apparatus by the rules' reading.
3. **Should stage 1 land alone**, a pure refactor with a small cost, or only
   together with stage 2's win?
4. **Linux:** stay synchronous (the agent's reference host), or follow
   stage 5 for parity of behaviour under load?
5. **1050.000 D3** (stage 3): is "build mid-fling when the apply fits" the
   right reading of "never mid-fling" once the build is off main?
