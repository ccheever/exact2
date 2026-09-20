# X1 results — world as an external DataSource

Result: the requested nonspatial Tally runs with **zero exact2 source changes**.
The only exact2 commit is `7369b48d`, the permitted ureq 3.4.0 → 3.4.2 lock repair.
There are no `HOOK:` commits. Baseline: `a4e78e42` (origin/main when this began).
The 48 kernel files and Tally logic match `d88e08b6` byte for byte.
The generic adapter is 246 production lines, including comments and blank lines.

All three hosts passed **13/13** checks: web/Chrome, native AppKit, and the Linux
host's headless renderer **on this Mac**. This is not a claim of execution on a
Linux operating system. iOS was not attempted. The commands are in [README.md](README.md).
Machine-readable samples and checks are in [evidence/summary.json](evidence/summary.json).

| Question | Zero-change result |
|---|---|
| a. Time | Yes: +1000 ms gives exactly 60 ticks on each host; live web and AppKit sustain approximately 60 simulation ticks/s. The Mac-runnable Linux headless entry needs an app-owned live pump. |
| b. Actions | Yes: send → ordered simulation input → tick → release. Repeated same-time actions survive and retain ordering. |
| c. Inspection | Yes through ordinary typed values; no special world agent API or automatic merging into runner logs. |
| d. Save/restore | Yes via export/import on all three, including queued inputs. Automatic runner-store persistence works on web and macOS; **fails on the Linux host**. |
| e. Failure | Yes: returned tick Err remains visible, app stays alive, and Reset recovers. |
| f. Startup/size | Yes, one app.wasm. It is larger than the supplied old two-module total; this is a different main/runtime revision. Ten cold process runs per host measured below. |

## Time and cost

Contract owns `state nowMs`, sets it to `now()` in `every(10, tick)`, and queries
`world(nowMs)`. The adapter advances an absolute clock, with one initial world
tick at 16.667 ms. The initial tick is bootstrap work; clock +1000 adds 60 more.
No host-specific simulation clock, GPU frame, wall-clock read, or JavaScript is
used to drive the world. Queries with an older timer argument after a send are
reads of the latest simulation state, not clock retreats.

Both supplied pins reproduce on all three hosts with seed 7:

- Tick 1: `0x32a9f7776ceb5052`.
- Tick 85 after the branch proof's script: `0x3d943e409a0493be`.

Timer intervals on main are **whole milliseconds**. A 16 ms timer is 62.5 Hz,
not exactly 60 Hz, and a seek ending between deadlines does not query that
remainder. Using 10 or 100 ms makes a +1000 seek exact. At 100 ms, the simulation
still executes every one of its 60 ticks/s, while Contract sees six-tick batches.
A permanently ticking world cannot meaningfully use `clock settle` to stop time.
The existing seek bound is 4,096 timer commits per advance; a 10 ms timer reaches
that limit on seeks beyond 40.96 seconds. Chunking long seeks needs no core change.

Each timer fires an ordinary action, resource settlement, and kernel commit.
The runner does not coalesce the simulation's intermediate work out of that
transaction path. Empty/no-change commits need not advance the kernel epoch:
100 timer receipts/s produce about 60 changed-tree epochs/s in this UI.

Measured runner-only cost, release build, one simulated second after warm-up:

| Timer | Commits/s | Allocations/s | Requested bytes/s | CPU µs/s |
|---|---:|---:|---:|---:|
| 10 ms | 100 | 22,434 | 4,499,050 | 1,390 |
| 16 ms | 63 in sampled second | 14,300 | 2,845,213 | 646 |
| 20 ms | 50 | 11,363 | 2,263,899 | 492 |
| 100 ms | 10 | 2,282 | 468,222 | 113 |

The allocator counts alloc, alloc_zeroed, and realloc; bytes are cumulative
requests, **not resident memory**. This includes the runner, source, and game.
Hashing/inspection is on demand and is excluded from ordinary timer work.

Live samples (short diagnostic windows, not a sustained-frame-rate guarantee):

| Host / timer | Simulation ticks/s | Rust allocations/s | Observation |
|---|---:|---:|---|
| Web / 10 ms | 59.6 | 24,111 | 60 host advances/s, 55.3 observed DOM text publications/s |
| Web / 100 ms | 60.0 | 2,593 | 10 updates/s |
| macOS / 10 ms | 59.9 | 28,412 | 99.8 resource queries/s; 5.89 MB requested/s |
| Linux presenter / 10 ms | 60.4 | 92,652 | App pump paints every timer wake; 266 MB requested/s |

Web numbers exclude JS/DOM allocation; macOS numbers exclude Swift/AppKit
allocation. The Linux number includes CPU raster output allocation and is not a
measurement of the native Linux DRM display loop. The stock headless entry on a
Mac serves the controlled agent clock; `tally-linux-live` adds a 32-line app-owned
Instant/deadline loop over `boot_presenter`, `timer_due_ms`, `clock`, and `frame`.
No exact2 code is changed for that embedding.

## Actions, inspection, and failure

Each `send` queues an Action-down and a later Action-up in `Sim`. Actions arriving
inside the same host tick are serialized across consecutive simulation ticks;
this deliberately trades up to one tick of latency per preceding action for no
lost presses. The proof sends Hold, Draw, Draw without advancing time, then
checks two cards and no banked score. There is a 500-tick backlog bound. The
checkpoint includes Sim's queued input plus the adapter's next-press timestamp.

Healthy Reset queues the original game's reset action, preserving its clock and
RNG behavior. Reset after an error reconstructs the world. Original game logic is
unchanged; the app's `Probe` delegates it and adds one explicit failure action.
The adapter catches a returned tick error and publishes the message as data.
A Rust panic/abort is outside this result; sharing the app module does not isolate
panic failures from the runner.

`state.resources.world` contains the published Hud, tick, and error. `Inspect`
fills `state.slots.inspection` with tick, hash, up to 64 named entities, component
JSON, and the kernel's bounded journal. `tree` sees the rendered text and controls;
it is still the UI tree, not an ECS tree. Runner `logs` contains sends, timers,
store names, and commit outcomes; world journal text appears there only if the
app explicitly arranges such a channel. Here it is in the inspection value and
in text visible through `tree`. No ninth agent operation is introduced.

## Durable state and continuation

The source saves complete EXSIM bytes, not only publications. Save/load/save
bytes and continued hashes match after process relaunch on all three hosts.
A further check saves several queued presses before they run and resumes those
same inputs after relaunch. Kernel error state refuses saves, and Reset restores
healthy operation.

Web live localStorage preserves the checkpoint across reload, with the score,
hand, and tick restored. On macOS the live secret store also restores exact
checkpoint bytes. The existing `EXACT_STORE=real` override permits a controlled
agent proof against it: save at tick 73, kill/relaunch, automatically restore,
continue to tick 85, and reproduce `0x3d943e409a0493be`. The proof forgets its saved
checkpoint afterward. Default agent mode uses a disposable store; web agent
mode ignores durable writes.

Linux is red for **automatic runner-store persistence**: a live run saves at
61, but relaunch starts at 1. `host/linux/src/host.rs::boot_at_with_region` passes
an empty snapshot, and the host does not persist `Runner::take_store_writes`.
Its update/bundle store is a different facility. Ordinary text export/import
already provides correct save/restore without a hook on this host.

Source-private state is also outside the runner's transaction rollback. The
negative control advances the simulation and then deliberately supplies a
wrong-shaped resource answer: the runner refuses and restores its visible
resource, but the source stays advanced. The standard successful Tally path does
not trigger this; a general stateful source must account for it. Hot reload of
private source state is likewise not automatic; saves must explicitly carry it.

## Startup and size

Ten fresh processes per host; Chrome uses a new profile each time. OS file caches
were not flushed. Measurements are from the instrumented experiment app.

| Host | First tick median | Reference point |
|---|---:|---|
| Web | During 80.30–88.95 ms | Navigation start; bracket around the actual Wasm host call containing first tick |
| Web FCP | 102.00 ms | First tick completed a median 13.50 ms before FCP |
| macOS | 130.48 ms | App-owned pre-main initialization → first tick |
| Linux host on Mac | 0.414 ms | Rust main entry → first tick |

The browser instrumentation gives a bracket, not an invented precise in-Wasm
wall-clock timestamp. The upper endpoint includes the rest of that host call.
Each of the ten web runs fetched exactly one Wasm. Mac pre-main initialization
excludes dyld work before the initializer; Linux main excludes pre-main work.
Host-reported boot-to-ready medians were 68.23 ms (macOS) and 68.86 ms (Linux),
with different origins from the process-entry measurements.

| Artifact | Raw bytes | gzip -9 bytes |
|---|---:|---:|
| Tally-as-source app.wasm | 1,262,330 | 518,510 |
| Main's Video Player app.wasm | 1,074,841 | 450,600 |
| Supplied branch Tally modules | approximately 661 KB + 366 KB | not supplied |

Video Player is main's smallest existing app by authored `app.contract` size
(2,173 bytes), built with the same web profile and wasm-opt. This experiment did
not exhaustively build every app to prove the absolute smallest Wasm artifact.
Tally adds 187,489 raw bytes and 67,910 gzip bytes over that baseline. The old
branch's modules and current main use different runtime revisions; eliminating
a second module does not by itself make this current artifact smaller. Optional
instrumentation includes allocation counters and two small measurement exports.

## Smallest hooks for limitations (proposals only)

No hooks were installed. Line counts below are implementation estimates, not
measured patches; they exclude tests and generated output.

| Limitation | Smallest proposed hook | Approximate lines | Ordinary-app use |
|---|---|---:|---|
| Linux automatic durable store | Add a secret-store adapter modeled on Apple `store.rs`; supply snapshot in `host/linux/src/host.rs::boot_at_with_region`; drain committed writes in the host/presenter effect path. | 70–100 | Login sessions, preferences, offline drafts |
| Stateful source rollback | Optional DataSource begin/commit/abort transaction callbacks in `runner/src/runner/source.rs`, called around `Runner::run_action`, resource invalidation, fulfill, and boot settlement. Adapter snapshots its own state. | 60–100 | Stateful caches, local editing engines, devices |
| Clock-to-source notification without resource re-query per tick | Optional source deadline and advance callback; fold deadline into `Runner::timer_due_ms` / `has_timers`, advance in `advance_timed`, publish only when changed. | 70–120 | Polling engines, playback, live dashboards |
| Exactly 60 Hz authored timer rather than integer-ms approximation | Fractional/rational timer period in `plan/tables/format.json` and builder; lower it in `contract/lower/src/lib.rs`; compute deadlines from origin plus count in runner to avoid accumulated drift. | 25–50 | Media/UI schedules, sampling |

The last two are efficiency/precision improvements, not prerequisites for this
experiment's deterministic 60 Hz simulation. The app-owned headless Linux loop,
manual checkpoint transport, and typed inspection require no core hook. A direct
DataSource→agent journal append API would be convenient but is unnecessary for
inspection and failure visibility. A CLI `--web-dist`/EXACT_WEB_DIST preference
in `scripts/agent.mjs` would be a roughly five-line external-app convenience;
the add-on wrapper already uses the existing `--url` path instead.

## Validation and execution notes

194 kernel, derive, app, adapter, and runner tests passed. Add-on clippy with
warnings denied and formatting passed; exact2 caps and boot checks passed.
Each host proof passed 13/13. All launches in the proof/measurement harness are
recorded, killed by PID, and reaped. No pushes or stashes were used.

Early harness failures are retained in the logs: Bun did not honor the spawn
patch, so the first custom browser measurement left children alive until its
recorded parent/profile inventory was collected and all 11 owned browser groups
were SIGKILLed and reaped. Measurements were discarded and rerun under Node.
The first two AppKit startup measurement attempts dispatched an inspect button
too early (ready had been announced); the third added a 250 ms host-readiness
wait and passed all ten runs. That wait does not change recorded first-tick times.
The Linux live helper was once invoked before its build completed (ENOENT), then
passed after the build. No failing measurement is represented as a passing run.

The source seam is sufficient to install this nonspatial world separately.
Its costs are ordinary Contract settlement, explicit inspection/serialization,
and shared failure/transaction boundaries. It is a poor fit for large,
high-frequency render state or logic that needs isolation and transactionally
rolled-back private state without an additional adapter protocol.
