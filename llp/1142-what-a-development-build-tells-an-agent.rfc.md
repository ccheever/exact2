# LLP 1142: What a development build tells an agent — late frames, portable traces, failed resources

**Type:** RFC
**Status:** Draft r4, 2026-10-25. §12 records what r3 built; §13 proposes the remaining Excord development-loop work and is unreviewed and unimplemented. The r1 and r2 blind reviews by GPT-6 Astra and Grok are in `llp/reviews/rfc-2026-10-25-1142-r{1,2}.{astra,grok}.md`; §R and §12 record their disposition. §13.8 lists the new decisions requested.
**Systems:**
- Apple host:
  - the frame sampler and its trace (`host/apple/Sources/ExactKit/FrameSampler.swift`);
  - a phase recorder beside the presenters' signposts (`Mac/PresenterMac.swift`, `IOS/PresenterIOS.swift`, `Mac/CollectionMac.swift`, `IOS/ScrollPumpIOS.swift`, `IOS/NodeViewIOS.swift`, `IOS/TextRasterIOS.swift`, `Mac/RegionReaderMac.swift`);
  - the agent's wheel and its routing (`Mac/AgentMac.swift`, `Mac/AgentMouseMac.swift`, `Mac/ChainingScrollView.swift`);
  - the Develop menu's App Info (`Mac/DevMenuMac.swift`);
  - signals (`ExactMac/main.swift`, `ExactIOS/main.swift`);
  - the batch reader (`ExactCore/BatchShapes.swift`).
- Runner:
  - failure counts and alerts (`runner/src/runner.rs`, `runner/src/runner/admission.rs`, `settlement.rs`, `runner/src/agent.rs`);
  - handler counts in `perf` (`runner/src/perf.rs`, `runner/src/runner/event.rs`).
- The Apple and web hosts' batch builders: alerts in the batch (`host/apple/src/layout.rs`, `host/web/src/host.rs`). Linux reads alerts in process.
- Contract lowering: `lower-key-position` (`contract/lower/`).
- Tooling:
  - `scripts/agent.mjs` and `scripts/agent-inspect.mjs` (`trace --pid`, `trace --plan`, `trace --out`, the late-frame text);
  - `scripts/async.mjs` (macOS XCTests);
  - `scripts/agent-launch.mjs` (a fixture port per checkout).
- Proposed in §13: resource freshness inspection; stream mapper execution in `js/` and `host/web-js/`; collection edge events and scroll reports; Excord's existing fixture and chat recipe.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Revision author:** Codex for Charlie Cheever (§13 and r4 metadata), 2026-10-25, at his request to write an RFC from the Excord development findings.
**Date:** 2026-10-25
**Implementer:** Claude (Opus 5.5), from 2026-10-25, for §11's work, as recorded in §12. Charlie asked for that review and build ("have the panel review this then go ahead and implement when you're happy with it"). §13 is a requested proposal; implementation is not yet assigned or scheduled, and the earlier authorization does not cover its additions.
**Related:**
- LLP 1079 (`perf`, the frame sampler, the trace, "juxtaposition, never a cause"). This RFC extends it most.
- LLP 1075.003.000.001 §3.1 (hatch calls charged by overlap with a late frame's window, the model D1 follows).
- LLP 1012 (the operations); LLP 1035.002 D6 (the development source map, keyed by plan digest).
- LLP 1070 G1–G2 (wheel latching and routing); LLP 1115.000 D8.2 (AppKit scrolls a `scroll` node for a person).
- LLP 1050.000 (fill policy; responsive scrolling checked only by XCTest).
- LLP 1141 (porting real apps). Excord is the app whose session this comes from.
- [LLP 1016.000](1016.000-answers-that-keep-coming.rfc.md) D7 (stream ordering, sends and request lifetime); [LLP 1027.005](1027.005-resource-identity-and-request-context.rfc.md) (persisted answer identity, not a live freshness predicate); [LLP 1113](1113-settled-and-the-two-clocks.rfc.md) D1 (capture and clock semantics).
- `rules/DEFERRED.md` §Agent API (ten operations; "a new measurement is a form of `perf`"; "a wheel is a `tap`"; never a causal trace, a flame chart, a devtools UI or production telemetry; the source map is "read by the driver and never by a host, never in a bake") and §Tooling (no devtools UI).
- `rules/RULES.md` §Agents.
- W3C [Long Animation Frames](https://w3c.github.io/long-animation-frames/). Apple: [`OSSignposter`](https://developer.apple.com/documentation/os/ossignposter), [`Logger`](https://developer.apple.com/documentation/os/logger), [`CGEventField.scrollWheelEventMomentumPhase`](https://developer.apple.com/documentation/coregraphics/cgeventfield/scrollwheeleventmomentumphase).

## Summary

Revision 4 extends this RFC with [§13](#13-the-remaining-excord-development-loop-work).
The proposal closes the native-input and inspection gaps left by §12, then removes stream
scheduling and list-prefetch workarounds from Excord. It changes no implementation or admission
rule by being written. §12 remains the account of what shipped.

An agent working on Excord on macOS (2026-10-24 and 25) lost most of its time to things a
development build knew but did not say. Its report listed seven gaps. This RFC checks each against
the code and proposes:

| | Gap | Proposal |
|---|---|---|
| D1 | A late frame names no cause | The time each named main-thread host phase spent inside the late frame's interval, charged to the innermost phase, with the rest stated as unattributed |
| D2 | A trace from another machine reads as bare site numbers | The trace names the build it came from (commit, host, bundle). `trace --plan` names a map, and `trace --out` writes a copy with the map joined in by the driver |
| D3 | The macOS driver cannot fling or hold a rubber band, and has no live frames | `wheel … fling` and `wheel … hold`, routed to AppKit and paced on the main run loop; `perf frames live` on Apple. Each starts with a probe |
| D4 | A failed resource is visible only in the journal | Counts beside `state.failed`; failure lines carried to the host in the batch and written to the unified log and stderr; a line in App Info |
| D5 | Only a person can save a macOS trace | `SIGUSR1` on macOS and the iOS simulator; `agent.mjs trace --pid` finds the file it asked for |
| D6 | Nothing flags work at frame rate | Dispatches and commits by handler site in `perf`; a lowering warning for a key that reads the row's position |
| D7 | Smaller gaps | macOS XCTests in the async lane once their cost is measured; a fixture port per checkout; build exit codes |

Two of the report's premises do not hold:

- A shape error already names the record's index (`runner/src/conform.rs:103`).
- `symbol:warning` is refused for every target by one lowering check (`contract/lower/src/symbols.rs:236`). It is not refused by the Apple bake alone.

## R. What r2 changed

| Change | Raised by | Where |
|---|---|---|
| D1 charges each main-thread phase its exclusive overlap with the late frame's sample interval, as hatches are charged. It reports the rest as `unattributed`, not as overrun minus a sum, which could go negative, and never as AppKit's work | both (BLOCKING in Astra's) | §3 |
| D1 names the function each phase wraps and drops phases that run off the main thread (raster decode, `text-raster-worker`). Kernel layout gets a new interval | Grok | §3 |
| The web's `loaf` fields keep their own names. They exist only for frames over 50 ms and are not batch apply | both | §3 |
| "0 transactions" means no recorded `seq` range, not "the app did nothing". The late-frame line also prints `batches` and the hatch overlap the record already holds | both | §3 |
| D2 takes provenance from what the build stamped (`ExactCommit`, `ExactCommitDirty`, `ExactBuildHost` in Info.plist), not from git at export. A portable trace is one the driver writes with the map joined in (`trace --out`), so no host reads a map | both | §4 |
| D3 is rewritten against `ChainingScrollView.appliesAtOnce`. Under the agent, every wheel the view takes is applied by hand, so a fling or hold must route to AppKit for its gesture | Grok (BLOCKING) | §5 |
| D3 specifies pacing, its end, the clock across a live window, and the second gate in `FrameSampler.activity()`. `perf frames … during` is new orchestration in the driver | both | §5 |
| D4 keeps `state.failed` a map of strings, which `agent-test.mjs` prints, and adds counts under a new key. Failure lines reach a host as data in the batch, not by searching text. Counting, reset and rollback rules are stated | both | §6 |
| D5 names `ExactIOS/main.swift` and how `--pid` finds the file (a per-process file in the process's temporary directory). Before signalling, it checks that the process installed the handler | both | §7 |
| D6 counts in `perf` by plan site, joined to source by the driver. It drops the 30-per-second threshold, the "while a scroller moved" clause and the runtime key-churn warning | both | §8 |
| D7 runs macOS XCTests on the same paths as the iOS ones and keeps their failures across skipped runs. The fixture port is chosen by the launcher, not written into `app.json`, and a collision fails loudly | both | §9 |
| "Development builds only" becomes a named predicate (§2) | Astra | §2 |
| Not adopted from r1: a stack sample, an in-app badge, a map carried by a host, a `QUEUE.md` "building" marker | both | §10 |

## 1. Motivation

These are the session's findings, with what the code says about each.

**Late frames.** The trace printed frames "17 ms past the target · 0 transactions · 0 journal
lines". "0 transactions" means only that the frame recorded no `seq` range (`scripts/agent-inspect.mjs:628`). A batch with no range still counts in `batches` (`FrameSampler.swift:105-110`), which the line does not print. The reader's comment calls the journal lines "juxtaposition, never a cause" (`:611`). A late frame's record holds:

- **Apple:** `interval`, `missed`, `seq`, `batches`, `apply` and `overrun`, plus any hatch calls that overlapped it (`FrameSampler.swift:44-57`, `:224`). The reader prints the hatches nowhere.
- **Linux:** the paint time (`frames.rs:26-32`).
- **Web:** the long-animation-frame split of script time against style and layout (`host/web/frames.js:90-91`), for frames over 50 ms only.

The work that fills a late native frame is the host's own. The Mac and iOS presenters already mark several of these with `OSSignposter` intervals:

- Mac: `apply` (`PresenterMac.swift:1011`), `scrolled` (`:444`), `pump-collection` (`:546`), `pump-text` (`:551`), `reader-demand`, `reader-publish`, `reader-draw` (`RegionReaderMac.swift:162`, `:275`, `:404`).
- iOS: `apply` (`PresenterIOS.swift:727`), `scrolled` and `text-draw` (`NodeViewIOS.swift:769`, `:1446`), `pump-collection` and `pump-text` (`ScrollPumpIOS.swift:243`, `:248`), `text-raster-*` (`TextRasterIOS.swift:168-250`).

Those intervals reach Instruments only. Kernel layout, and the collection's anchor fixes (`CollectionMac.swift:297-301`, inside `apply`), have no interval at all.

**Traces from elsewhere.** The trace from the M5 worktree printed "source unavailable: source
map does not match the running plan" (`agent-inspect.mjs:93`, `:567`), so every site was a bare
number:

- The reader joins sites to source by plan digest (`:101`). It looks for a map in a few local places: `EXACT_DEV_PLAN`, the web build, this checkout's bakes (`agent-launch.mjs:745-757`).
- An Apple or Linux trace names its host, app, trust and OS, but not the build it came from (`FrameSampler.swift:286-288`, `frames.rs:243-244`).
- The web dev server adds git's state at export time and the map when the digest matches (`host/web/serve.mjs:1128-1134`).
- `agent.mjs trace` ignores the `--plan` flag it parses for launches (`agent.mjs:1291`, `agent-launch.mjs:449`).

**Scrolling on macOS.** The bottom jitter was found from a screen recording, not by the driver:

- Under the agent, `ChainingScrollView.appliesAtOnce` is true (`ChainingScrollView.swift:108`). A wheel the view takes is applied by hand and clamped (`.here`, `:302-310`), so AppKit's momentum and rubber band never run, except on a contained axis (`:195-200`).
- `tap … wheel dx dy gesture` sends began, changed and ended back to back from one call, with no momentum phase (`AgentMac.swift:712-715`, `AgentMouseMac.swift:80-89`).
- `perf frames live` refuses: "the macos host has no live window" (`agent.mjs:795`). The Apple agent answers `virtual` whenever the session runs on the agent clock (`Agent.swift:383`). The sampler opens no segment then either (`FrameSampler.swift:115`).
- A stretched band cannot be built in a test, because "`NSClipView` clamps an origin set through its own API while AppKit's rubber band puts one out of range" (`AgentMac.swift:215-218`).

**A silent failed resource.** One channel's `lastRead` came back as a number where the shape
declares a string. Both the channel list and the DM list failed, and the DM list failed 67 times
in the session:

- The runner refused each answer with a message that names the path and index (`conform.rs:87-158`, `failure.rs:129-132`), kept the last value (`admission.rs:181-193`), and wrote one journal line.
- `state.failed` holds only the latest message per resource, with no count (`runner/src/agent.rs:845-860`).
- The Apple host sends the journal nowhere else (`Session.swift:1385`), so `log show` found nothing. App Info (`DevMenuMac.swift:438-457`) does not mention failed resources.
- On screen, the app showed yesterday's list.

**Saving a trace.** Only a person can save a trace from a running macOS app:

- Develop → Save Trace (⌥⌘T, `DevMenuMac.swift:295-296`) ends in a modal alert (`:488-501`).
- Neither `ExactMac/main.swift` nor `ExactIOS/main.swift` handles `SIGUSR1`.
- Linux saves on `SIGUSR1` in its display loop (`display.rs:380-384`, `:517-523`).
- `AGENTS.md` and `CLAUDE.md` list "⌥⇧T, Save Trace or `SIGUSR1`" as if each worked on every host. ⌥⇧T is the web's shortcut.

**Churn.** A hover handler committed once per row crossed during a fling. It showed up only
because the agent happened to profile a wheel gesture:

- `perf` counts work by plan node (`perf.rs:25-46`), but not dispatches or commits by handler.
- The member list's rows were rebuilt about twice over, probably because their keys include the row's index. Nothing warns when a `for`'s `key=` reads the position (LLP 1062 D8).

## 2. Constraints

- **Ten operations.** A new measurement is a form of `perf`, and a new input is a form of `tap`. `trace` is a driver verb, not an operation on the session. `trace --pid` asks a process for an export by signal, which adds no operation to the agent API.
- **Never a causal trace or a flame chart.** D1 records durations of a fixed list of named host phases beside a frame, with coverage stated. It records no stacks and claims no cause.
- **No devtools UI.** D4 adds a line to the existing App Info panel and writes to logs the platform already has.
- **Source maps** are read by the driver only. D2's `--out` is the driver joining a map it read into a file it writes. No host or bake reads or carries one.
- **The predicate.** Everything here is on wherever the frame sampler and trace may run: every binary that is not a production bake (Apple `FrameSampler.measured`, `!ExactEnv.productionBake`; Linux `!delivery::production`). A non-production release build starts measuring at the first ask, as today (`FrameSampler.swift:22-25`). Specifically:
  - The signal handler, the phase recorder, the platform-log writes and the App Info line follow this predicate.
  - The runner's counts and alert queue are bounded data a production host never drains.
  - The lowering warning is a compiler diagnostic, so it has no runtime predicate.

  (r1 said the sampler is absent from release builds. It is absent from production bakes.)

## 3. D1: what the main thread did in a late frame

**Recording.** A host keeps a ring of *self segments*: `(phase, start, end)` on the media clock, main thread only. `Phases.measure(.rows) { … }` does three things:

- It opens the existing signpost interval, so Instruments and the trace agree.
- It closes the enclosing phase's current segment when it starts, and reopens one for the enclosing phase when it ends.
- Its segments never overlap. Time is charged to the innermost phase.

**Attribution.** For a late frame, the sampler charges each phase its overlap with the sample's interval `(now − interval, now]`, which is the window hatches use (`HatchDiagnostics.swift:97-108`). It records:

- `phases`: each phase's milliseconds inside the window, and how many segments it had.
- `unattributed`: the interval minus the phase sum. Segments are disjoint and clipped to the window, so this is never negative. It covers idle time, AppKit's own work and anything not wrapped, and the text says so.
- `coverage: partial` when the ring's oldest segment starts after the window opened.

This is an observation interval with an exclusive partition, not a share of the overrun.

**The phases** (Apple; wrapped where the signpost is or where named):

| Phase | Mac | iOS |
|---|---|---|
| `apply` | `Presenter.apply` (`PresenterMac.swift:1011`) | `Presenter.apply` (`PresenterIOS.swift:727`) |
| `layout` | the host's kernel layout calls (new interval) | the same |
| `rows` | `pump-collection` (`:546`) | `pump-collection` (`ScrollPumpIOS.swift:243`) |
| `anchor` | the collection's anchor fix (`CollectionMac.swift:297-301`, new interval) | — |
| `text` | `pump-text` (`:551`) | `pump-text` (`:248`), `text-draw` (`NodeViewIOS.swift:1446`), `text-raster-urgent` (`TextRasterIOS.swift:168`, `:220`) |
| `scroll` | `scrolled` (`:444`), less what nests inside it | `scrolled` (`NodeViewIOS.swift:769`) |
| `draw` | `reader-draw` (`RegionReaderMac.swift:404`) | — |

Raster decode and `text-raster-worker` run off the main thread and are not phases.

**Other hosts.**
- Linux keeps `paint` and gains `layout` and `text` the same way once Apple's shape is settled.
- The web keeps its own `loaf` fields under their own names: they are a browser report for frames over 50 ms, not these phases.

**The text.**

```text
late at 41210: 1 missed (16.7 ms) · main 17 ms past the target · no transaction range · 2 batches
  of 16.7 ms: rows 9.4 (3) · text 4.1 (12) · anchor 1.8 · apply 0.3 · unattributed 1.1
  hatches: haptics 0.4
```

**Cost.** Two clock reads per phase entry, development only. The ring is fixed (2,048 segments).

**First probe.** A unit test of the recorder covering four cases: nested phases, a segment crossing the window's edge, an on-time frame, and a late frame with no phase. Then a late frame from a real fling in Excord.

## 4. D2: a trace that reads anywhere

1. **The build the trace came from.** An Apple trace's `identity` gains what `build.mjs` stamped into Info.plist: `commit` and `dirty` (`ExactCommit`, `ExactCommitDirty`), `buildHost` (`ExactBuildHost`), `appCommit` where the app has its own repository, and the bundle's path, `bundle`. It also gains `pid` and `savedAt` (D5). A development bundle's `receipt.json` names the linked plan the build used. The reader follows it when the bundle is on this machine. Linux gains the same fields from its build's environment.
2. **`trace <file> --plan <path>`** names a plan, its `.map.json`, or a directory. A directory is searched one level deep for `*.plan` files with a map beside them. The join stays by digest: an explicit `--plan` is tried first, then a map the trace carries, then the defaults.
3. **`trace <file> --out <file>`** writes the trace back with `map` set to the map whose digest matched. `trace --pid` (D5) does this by default into the app's `target/traces/`. A trace written that way reads on any machine. The map was joined by the driver, so no host reads or carries one.
4. **When nothing matches,** the reader says what to pass: "plan `<digest>` from commit `<sha>` (dirty) built on `<host>`: pass `--plan` a build of it."

A matching digest establishes that a map fits the plan, not that its source revision is the one the agent has. The reader's existing caveat (`agent-inspect.mjs:29-32`) stands.

## 5. D3: real scrolling in the macOS driver

All three are forms of `tap … wheel` and `perf`. None is a new operation.

**Routing.** A `fling` or `hold` gesture sets `ChainingScrollView.appliesAtOnce` false from its `began` until its end, so the latched owner routes it `.appKit` and AppKit runs its scrolling, momentum and band. Plain `wheel` and `wheel … gesture` are unchanged.

**Pacing.** The events are posted with `NSApp.postEvent(_:atStart: false)` from a timer on the main run loop, one per display period. They are not sent from one call, because AppKit applies one event of a gesture handed over whole (`ChainingScrollView.swift:103-105`).

- **`tap <s> wheel dx dy fling`** takes `(dx, dy)` as the finger's total travel. The driver sends:
  - began;
  - changed events carrying the travel over 100 ms;
  - ended with zero delta;
  - a momentum stream: `scrollWheelEventMomentumPhase` began, then changed events whose deltas decay by 0.95 per period from the last finger velocity, then ended once a delta is under 0.5 points or after 2 s.

  The operation returns after the momentum ends. Its reply carries the elapsed real time and the final offset.
- **`tap <s> wheel dx dy hold`** sends began and changed and leaves the gesture open. AppKit then stretches the band as a held trackpad does. `layout` reports the out-of-range origin (`ox`/`oy`, `AgentMac.swift:266-268`) without settling, and `screenshot` shows it. `tap <s> wheel 0 0 release` sends the zero-delta ended event. The driver sends it itself if the run ends with a hold open.

**The clock.** AppKit's scrolling runs on the real clock, so a fling's events and AppKit's animation run in real time. The session's virtual clock is not advanced meanwhile. Motion the runner owns stays where it was until the next `clock` operation.

**`perf frames live <ms> [during "<op>" …]` on Apple.** For the window, the sampler's gate in `activity()` (`FrameSampler.swift:115`) admits segments even though the session has an agent clock. The display link samples on the real clock, the `during` operations run, and the reply is the window's frames with `live.from/to` as LLP 1079 returns them. The driver runs `during` inside the window. Today the `perf frames` branch returns before reading `during` (`agent-inspect.mjs:445`).

**Probes first, in this order.** The lane stops and records the result at the first that fails:

1. In agent mode, a `hold` routed `.appKit` leaves the band stretched across a separate `layout` and `screenshot`.
2. A paced momentum stream moves a `ChainingScrollView` in agent mode as a trackpad's would. Apple does not document that synthesized momentum events drive `NSScrollView`. QUEUE.md's fill-policy entry notes that responsive scrolling takes window-server events.
3. `perf frames live 1500 during "tap list wheel 0 -2000 fling"` records the fling's frames.

## 6. D4: a failed resource you can see

**Counts.** `state.failed` keeps its shape, a map from resource name to message, which `scripts/agent-test.mjs:433` and `:633` print. A new key, `failures`, gives each resource that has failed since boot `{count, first, last}`:

- `count` counts refused answers since boot. It is not reset by a success and it is not rolled back with a refused commit. It is diagnostic, kept outside the commit's snapshot (`commit.rs:25-26`, `:86-87`).
- `first` and `last` are the runner's clock (`now_ms`) at the first and latest refusal.
- A stale ticket's reply is counted only if the runner refuses it.

**Alerts.** The runner gains `Runner::alert(line)`. It writes the journal line as `log` does and also queues it in a bounded list of 64 lines, with a count of lines dropped. The failure sites call it:

- `release_failed` (`admission.rs:190-192`);
- a resource failing during reconcile (`settlement.rs:625-628`);
- `failed_now` (`lines.rs:229-230`).

Hosts take the queue as data:

- **Apple and web wasm:** the batch gains `alerts: [String]` beside `seq` (`host/apple/src/layout.rs:49`, `host/web/src/host.rs:1045`).
- **Apple's Swift side:** `Logger(subsystem: "com.exact.app", category: "journal").error(…)` and stderr.
- **Linux:** reads the queue in process and writes stderr.
- **Web:** `console.error`.

`log show --predicate 'subsystem == "com.exact.app"'` then finds them, and a native app launched by `build.mjs --run` prints them in its terminal.

**App Info.** The Develop menu's App Info (⌘D) gains one line per failing resource: its name, count and message.

**First probe.** A runner test covering four sequences: failure, failure, success, failure; a stale reply; a refused commit; and the queue's bound. Then Excord's `lastRead` case with the fixture answering a number.

## 7. D5: a trace without a person

**The handler.** `ExactMac/main.swift` and `ExactIOS/main.swift` install a `SIGUSR1` source as `SIGTERM`'s is installed (`ExactMac/main.swift:703-717`): the signal is caught, then handled on the main queue. Only builds under §2's predicate install it.

**The save.** It calls `saveTrace()` with no alert. Besides today's two files, it writes `trace-pid-<pid>.json` atomically in the process's temporary directory.

**The marker.** When it installs the handler, the process writes `exact-trace-<pid>.ready` in the same directory.

**`agent.mjs trace --pid <pid>`:**

1. **It finds the directory.**
   - For a Mac app, which is unsandboxed here (no `app-sandbox` entitlement outside widgets), this is `getconf DARWIN_USER_TEMP_DIR`.
   - For a simulator app, `ps -o comm=` gives the executable inside the simulator's container. The device and the bundle's Info.plist follow from that path, and `simctl get_app_container <udid> <bundle> data` gives `tmp/`.
2. **It refuses to signal** a process with no `.ready` marker newer than the process's start time (`ps -o lstart=`). An unhandled `SIGUSR1` terminates a process.
3. **It signals, then waits** up to 10 s for `trace-pid-<pid>.json` with an mtime after the signal.
4. **It reads the trace** with D2's join, writes it with its map into the app's `target/traces/`, and prints it.

A phone keeps `trace --phone`, where a person saves and the driver copies.

**The docs.** The line in `AGENTS.md` and `CLAUDE.md` becomes "⌥⌘T on macOS, ⌥⇧T on the web, the dev menu's Save Trace on iOS, `SIGUSR1` on macOS, the iOS simulator and Linux (`trace --pid`)".

**Not covered.** Linux's signal is read only in the DRM display loop (`display.rs:517-523`), so its headless agent build has no trace path. This RFC leaves that alone.

## 8. D6: work at frame rate

**Handler counts in `perf`.** `dispatch` (`runner/src/runner/event.rs:1085`) has the view and the event. The runner counts, per `(plan site, event name)`, how many dispatches ran and how many committed a change. `perf` replies with them as `handlers: [{site, event, dispatched, committed}]`, and the driver joins `site` to source as it does for work. `perf … during "tap list wheel 0 -2000 gesture"` then shows "`hover in` at app.contract:412: 58 dispatched, 58 committed" as a difference.

There is no threshold and no runtime warning. Rates depend on the display (8.3 ms on ProMotion), and a handler that should run every frame (an animation) looks the same. The driver prints the counts, and the agent judges.

**`lower-key-position`.** Lowering warns when a `for`'s `key=` reads its position variable (`contract/syntax/src/ast.rs:1085-1100`, `contract/lower/src/lib.rs:1053-1065`):

```text
warning[lower-key-position]: this key reads `i`, the row's position: if a row is inserted or removed before others, they get new keys and are rebuilt; key by something the record keeps
```

It is a warning, not an error. An append-only list keyed by position is correct.

**Not proposed.** A runtime warning that a collection replaced most of its keys. Filtering or replacing a dataset does that legitimately, and its denominator and false positives are unmeasured.

## 9. D7: smaller things

- **macOS XCTests in the async lane.** The lane runs only `build.mjs --test --ios` (`scripts/async.mjs:174`), so a macOS class that fails on main is never reported. The session found six.
  - First, time `bun host/apple/build.mjs --test` on the Studio.
  - If it fits the lane, add it beside the iOS step under the same `appleTouched` predicate (`:121`, `:215-223`), and add its name to `retainSkippedFailures` (`:257`) so a skipped run keeps an outstanding failure.
  - `QUEUE.md` on main at 5f3080d45c has no entry for the six classes. The lane adds one if they still fail.
- **A fixture port per checkout.** Excord's fixture takes `--port` or `PORT` (`apps/excord/fixture/server.ts:1072-1075`), and the app's origin is baked from `EXACT_BACKEND_ORIGIN` (`apps/excord/app.ts:14-20`), which is a build input (`agent-launch.mjs:954`; `js/bake/src/lib.rs:566`). Choosing a port per session would rebuild on every launch, so `agent-launch.mjs` chooses one per checkout:
  - An explicit `--backend` or `PORT` wins.
  - Otherwise, a hash of the checkout's path picks a port in 4790–5789.
  - The launcher passes it to the server it starts from `app.json`'s fixture command, and to the bake as `EXACT_BACKEND_ORIGIN`.
  - If the port is taken by something other than this checkout's fixture, the launch fails and names the port and how to choose another.
  - `app.json` stays unchanged. A server a person starts by hand prints the port the launcher would choose (`bun scripts/agent-launch.mjs --fixture-port`).
- **Build exit codes.** The `symbol:warning` refusal comes from one lowering check, so the targets agree. The session's web check grepped the output for "error" instead of reading the exit status. `host/web/build.mjs` exits 1 on a failed build (`:104`). The lane confirms `host/apple/build.mjs` does likewise and adds a line to `docs/agent-pitfalls.md`: judge a build by its exit status.

## 10. Questions for Charlie

Both reviewers answered r1's six questions the same way, and r2 builds those answers:

- phases yes, no stack sample;
- maps stay out of hosts;
- no badge;
- the lowering warning, and handler counts in `perf` with no runtime threshold;
- macOS XCTests once measured;
- no `QUEUE.md` marker.

Charlie can overrule any of them. Two remain open:

- **Q1.** Is a main-thread stack sample on overrun ever wanted? Both reviewers place it under the flame chart DEFERRED refuses, and r2 leaves it out.
- **Q2.** The duplicated nucbox work was a worktree that had not rebased, not a missing marker. Is anything wanted there beyond the existing memory note to check main for parallel lanes?

## 11. Order

1. D5 and D2: the signal, `trace --pid`, provenance, `--plan` and `--out`.
2. D4: counts, alerts, logs and App Info.
3. D1: the recorder, the Apple phases and the reader's text.
4. D6: `lower-key-position`, then handler counts.
5. D7: time the macOS XCTests, set the port per checkout, add the pitfall line.
6. D3: the three probes, then what they allow.

## 12. As built (r3)

Built on 2026-10-25 by Claude (Opus 5.5). Where the build departs from §3–§9, this section
is the authority. The r2 reviews' points are folded where named.

**D5, the signal.** `TraceSignal` (`host/apple/Sources/ExactKit/FrameSampler.swift`) is
installed by `ExactMac/main.swift` and `ExactIOS/main.swift` where `FrameSampler.measured`
holds:

- It catches `SIGUSR1` and saves on the main queue with no alert.
- A signalled save also writes `trace-pid-<pid>.json`. The handler writes `exact-trace-<pid>.ready` when installed and removes it at exit.
- A Mac app saves its front window's session (`DevMenu.session`), and the trace names it (`identity.session`). This answers Astra's r2 point that a process can hold several sessions.

`agent.mjs trace --pid <pid>` (`pidTrace`, `scripts/agent-inspect.mjs`) works as follows:

- It finds the directory from `ps -o comm=`. A simulator app's path holds the device and bundle, and `simctl get_app_container` gives `tmp/`; a Mac app uses `getconf DARWIN_USER_TEMP_DIR`.
- It signals only a process whose marker is no more than a second older than its start time. `ps -o lstart=` gives whole seconds (Grok's r2 point).
- It waits up to 10 s for the file and copies it into the app's `target/traces/` with its map joined in.

On a hand-launched development Caltrain, the whole round took 0.13 s, and a process without the marker is refused by name. The simulator path is unexercised. The `AGENTS.md` line now names each host's way.

**D2, provenance.** The trace's `identity.hostBuild` holds the host binary's Info.plist stamps (`commit`, `dirty`, `machine`, `at`, `appCommit`, `appDirty`, `bundlePath`) and is not presented as the plan's: a development host may run another plan (Astra's r2 point).

- When no map fits, the reader says "no map here fits plan …: pass --plan …; the host was built from …".
- `--plan` takes a plan, a map, or a directory searched four levels deep.
- `--out` writes the trace with its map, and says so when none matched.
- The receipt does not name the map, so the reader keeps finding a map through this checkout's bake for the app, not through the receipt.
- Linux traces gain `pid` and `savedAt`, but no build stamps.

**D4, failures.** Everything new lives in the measured `Work` (`runner/src/perf.rs`), so a runner that measures nothing collects nothing (Astra's r2 point):

- `state.failures` gives `{count, first, last}` per resource; `failed` keeps its strings.
- The runner counts a refused answer at four sites: `release_failed`, `give_up`, and the two reconcile failures (`settlement.rs`, both).
- `Runner::alert` queues the same lines and the failed sends (`queue.rs`, three sites).
- `take_alerts` drains the queue, so each line is delivered once. Past 64 lines it says how many were dropped. The runner tests cover failure, failure, success, failure; a production runner; and the bound.

The hosts deliver alerts this way:

- **Apple:** the batch carries `alerts`. The session writes each line once, at the batch's one accounting point, to `Logger(subsystem: "com.exact.app", category: "journal")` at `.error` and to stderr.
- **Linux:** stderr, from `service_commit`.
- **Web JS target:** `console.error` for a failed request in a development build (`host/web-js/rt.js`). It has no counts.
- **Web wasm target:** not covered.
- **App Info:** a `failing:` line per resource.

**D1, phases.** `HostPhases` (`host/apple/Sources/ExactCore/HostPhases.swift`) keeps exclusive self-segments in a ring of 2,048 on the main thread. It is turned on with the frame sampler. Each late record gains `phases`, `unattributed` and, when recording does not reach the window's start, `phasesCoverage`.

Kernel layout runs in Rust, on the runtime's owner thread, and main waits for each call. So the `layout` phase became `runtime`: main's wait in `Owner.sync`, which covers the runner, the kernel and its layout (Grok's r2 point). The other wraps are the signposted sites of §3's table, plus `CollectionMac.shift` for `anchor`.

Coverage is partial when recording began after the window opened, or when the ring has let go of a segment ending inside it. An open phase is charged when it closes (Astra's r2 point). `HostPhasesTests` covers nesting, clipping, an idle window with an open phase, both kinds of partial coverage, and off.

On a hand-launched Caltrain, the late frames read, for example:

```text
late at 1170.24: 1 missed (19.39 ms) · main 11.07 ms past the target · 1 transaction (seq 4..4) · 1 batch · apply 10.55
  of its 19.39 ms: apply 10.5 (2) · runtime 0.37 (3) · unattributed 8.52
```

The reader prints the record's hatches too. Linux phases are not built.

**D6.** `lower-key-position` sits beside `lower-row-lookup` (`contract/lower/src/warnings/rows.rs`). It warns when a key reads the position, with an arrow parameter, `let` or `match` arm that rebinds the name excepted. Its text says that what a row holds follows the position and that rows may be built again, not that they are (Astra's r2 point).

`perf` replies with `handlers: [{site, event, dispatched, committed}]`, counted in `dispatch_event`. "Committed" means the receipt created, destroyed or touched a node. The driver joins sites to source, subtracts across `during`, and prints the handlers under the sites.

**D7.**
- **The pitfall line** is in `docs/agent-pitfalls.md`. The web build refuses `symbol:warning` with exit 1, and its message carries no word "error".
- **macOS XCTests.** On an M5 Max, `bun host/apple/build.mjs --test` took 6 min 3 s with its compile: 274 classes in 254 processes. Three classes failed: `BorderParityMacTests` (a comparison with Chrome on screen), `NativeButtonFidelityMacTests` (a pixel difference of 0.028 against 0.02) and `TableFeedKeyMacTests`, which `QUEUE.md` already lists as flaky. None touches what this RFC changed. They are not yet added to the async lane, because the lane would start red. `QUEUE.md` has the entry.
- **The fixture port per checkout** is not built. A development bake moves a loopback grants key only to the same port on another address (`sameServer` in `js/bake/src/typescript.mjs`), so the change reaches into the grants rule. `QUEUE.md` has the entry.

**D3: the first probe failed, and nothing of D3 was kept.** In agent mode on Caltrain, the gesture was routed `.appKit` for its length, and its phased events were made with `CGEvent` and paced a display period apart from a common-mode timer, with the event queue served by `NSApp.nextEvent`. Whether the events were posted to the queue or sent with `NSApp.sendEvent`:

- AppKit applied only the gesture's first event. A held pull of 1,000 points in 12 events moved the offset 83 points and never past the end.
- A 123-event fling with momentum moved nothing.

This matches what `ChainingScrollView`'s comment and the fill-policy entry in `QUEUE.md` record: AppKit's tracking of a trackpad gesture reads the window server's events, not the application's. An agent cannot hold a rubber band or fling this way. What would is an event the window server delivers: `CGEvent.post` to the app's process, which needs the session's Accessibility permission (see the `session-tcc-limits` memory). `QUEUE.md` has the entry. `perf frames live` on Apple was not built, since it would measure a fling the driver cannot make.

## 13. The remaining Excord development-loop work

### 13.1 Problem and evidence

Exact should let an author answer three questions without reading host internals: is the screen
showing the answer I asked for, what work made this interaction slow, and did my test exercise
the behavior a person uses? Excord exposed gaps in all three. It also carried scheduling and
paging machinery that belongs below the app's protocol logic.

The evidence is Excord's October 22–25 development sessions on `100.107.191.6`, the fixes they
produced, and the reproductions linked below. The session records are in
`~/.claude/projects/-Users-ccheever-projects-exact2/`, sessions
`042c91d9-2368-4510-82e6-369d192e97be` and `571cc685-f344-4cbd-99f1-ae47a2aae8b6`.
They are historical observations, not fresh measurements of this revision. No account data or
raw service payload is copied here.

| Observed cost | Framework lesson | Durable evidence |
|---|---|---|
| A malformed read position failed the DM resource 67 times while old rows stayed visible | Failure and retained-answer identity must be visible together | §1; fix `c7405ae111` |
| The agent repeatedly waited for Charlie to scroll while a CPU sampler was armed | The driver must exercise native scrolling and measure it on real time | §12 D3 |
| Each symbol size caused another Mac layout; hover work followed rows under a still pointer | Report repeated work within a frame, not only a late-frame total | Fixes `a6ec5a0f7e`, `4d404187a7` |
| Opening about 3,000 DMs built every row; member keys included their positions | Show construction counts and identity churn before choosing an app limit or framework fix | Fixes `268396d49f`, `f3af880a77`; §12 D6 |
| A 3 MB READY took 129 ms against a 100 ms turn budget | A supported stream needs bounded work across turns | [Stream mapper issue](../issues/20261023-a-stream-mapper-cannot-yield.md) |
| A fling outran the fixed one-screen page lead | An author needs to choose how early to fetch | [Collection edge issue](../issues/20261022-reach-lead-is-fixed-at-one-screen.md) |

The profile reports compare samples of busy time, not elapsed-time benchmarks. They establish
where to investigate; they do not establish a frame-rate improvement. A frame with no transaction
range likewise does not prove that app work was absent (§3).

### 13.2 D8: exercise native input and observe its presentation

Continue D3 from its failed probe. First try window-server delivery on a development Mac with
the required Accessibility permission already granted. Prove that a held pull crosses the
scroll limit and that momentum continues after release. A driver that only changes an offset
does not pass. If the environment lacks permission or AppKit does not accept the events, report
the exact unsupported condition; never silently substitute the synthetic path.

Extend existing `tap`, `layout`, `screenshot` and `perf` forms:

- Input receipts identify the path actually used: platform delivery or synthetic delivery.
  A platform event posted without observable delivery is not reported as a successful gesture.
- `perf frames live` on Apple can observe a bounded interval of human input independently of
  whether the driver can synthesize it. That removes §12's dependency between recording and
  automatic flinging. The interval records actual frame timestamps, display cadence, input path
  when known, and whether the session is on real or virtual time.
- A live capture does not seek or settle. Frames carry timestamps and dropped-capture counts;
  screenshots alone are not a display-cadence measurement. Keep LLP 1113's existing final-capture
  behavior separate and unchanged. Restore the prior clock mode on success, failure or timeout,
  without replaying elapsed timers or moving the session clock backward.
- `layout` labels target and presented geometry separately. Where the host cannot read the
  presented value, it says unavailable rather than returning the target under that name.
- On iOS, a platform touch can cross from the transcript over the software keyboard while
  preserving its contact identity. Verify interactive dismissal, cancellation and return to
  typing with the platform's input path.

**Acceptance:** exercise the Mac bottom-bounce gesture with native input; capture its settling and
read its presented offset; film an iOS keyboard dismissal while observing real frame timing.
The same gesture on a small hand-built native scroller is the reference. Keep deterministic
virtual-clock tests for state and exact motion samples. Neither substitutes for this drive.

### 13.3 D9: show why a resource's displayed answer is retained

D4's failure counts and logs are built. Extend `state` to join them with a resource's source
site, current request generation, last successful generation, pending status, and whether the
displayed value is a fallback, a current answer, or an answer retained from an earlier request.
For a retained answer, distinguish equal arguments from changed arguments using the runner's
existing equality rules. Publish opaque generation identifiers and equality results, not raw
arguments, tokens, answer bodies, or hashes of secrets. The driver joins source locations.

Give the web JS target the same diagnostic meaning; §12 currently gives it console errors but
no failure counts. Bring the web wasm target into the same failure reporting. Keep collection
bounded and development-only under §2's predicate.

This does not change retention semantics. LLP 1027.005's `with` governs admission of a persisted
answer at boot; it does not establish that a live value belongs to the latest request. Update
the existing fetch/loading/error recipe with the server-switch case: an answer names its owner,
the view checks it before showing rows, and failure shows an actionable state. Account generation
checks remain app authorization logic, including the A → B → A case.

**Acceptance:** load A, switch to B, then make B return a shape error. One inspection identifies
the failed field, the resource's declaration, its repeat count, and the retained answer's older
request. The recipe never labels A's rows as B's. Repeat with a failed refresh of A: A's own rows
may remain visible with its failure. Test overlapping completions and recovery on both runtimes.

**Deferred decision:** a new author-visible freshness predicate or a change to default retention
needs a separate semantic decision. Start with accurate inspection and the existing owner-gating
pattern; do not introduce a second resource identity system here.

### 13.4 D10: count the work inside a slow interaction

Extend the existing measured `perf` data with a fixed set of counts: kernel layout invocations,
intrinsic-size reports and their resulting layout requests, native control measurements, row
construction/reuse/destruction, and synchronous owner calls. Record counts at the operation
itself. Attribute them to a collection or plan site where the caller has one; otherwise name
the host phase and mark the site unknown. Do not infer attribution from the nearest event.

Use `perf … during` to report differences for one drive. Phase timing keeps D1's exclusive
accounting; counts are not added to milliseconds, and parent and child timings are not summed.
Keep bounded aggregates, not a history of every node or a new causal trace. The report states
when measurement began and what coverage the host provides.

**Acceptance:** the 150-channel fixture exposes per-icon layout multiplication, a 3,000-DM
fixture exposes eager construction, and a member insertion exposes unnecessary row replacement.
After the corresponding fixes, the same drives show the reduced counts. Measure instrumentation
overhead with it enabled and disabled on the same build and device. Ordinary production builds
collect none of these additions. The existing key warning remains; no new warning threshold or
performance check is added by this proposal.

### 13.5 D11: let a stream mapper finish across bounded turns

Amend LLP 1016.000 D7 to permit a mapper to return a promise, with the same per-turn budget and
yield mechanism as an asynchronous answer. The executor owns delivery, queue bounds and lifetime:

1. At most one mapper invocation is active for a stream generation. Later frames remain ordered
   behind it, including while it yields. An empty waiting queue does not mean the active mapper
   has finished. Normal close delivery follows preceding admitted frames.
2. Pending frame bytes stay charged against a finite receive budget while a mapper is suspended.
   Reuse the transport's documented limits and overflow policy. Overflow fails visibly and retires
   the generation; it never silently drops a protocol frame or grows an unbounded queue.
3. Refresh, forget and transport failure retire the generation. Its continuations cannot publish
   another snapshot or issue another socket send. Already accepted sends retain D7's existing
   semantics; cancellation cannot undo them.
4. Socket sends after a yield are authorized against the active mapper's captured request and
   generation on both native and web JS executors. An unrelated source turn cannot lend it
   authority. Completion uses the existing stream settlement and snapshot coalescing rules.
5. Framework cancellation does not roll back arbitrary mutations already made to module state.
   Excord must still guard shared cache writes with its account and connection generations. The
   framework removes delivery machinery, not the app's ownership of its protocol state.

Native `DecompressionStream("deflate")` is a second, separable improvement, loaded through an
optional artifact rather than a core Cargo feature. Require bounded output and retained decoder
state across packets. Its API and capability placement need an amendment to the data API's
governing LLP before implementation. It does not make a large synchronous `JSON.parse` preemptible;
Excord's bounded parsing stays until a measured replacement exists.

**Acceptance:** run the existing 3, 8 and 32 MB READY fixtures on Hermes and the web, including
highly compressible and fragmented input. Exercise a heartbeat behind READY, disconnect during
yield, a replaced connection, sign-out, overflow and a stale continuation. Assert frame order,
bounded queued bytes, no sends or publication after retirement, and no reconnect caused by
starving the fixture's heartbeat deadline. Measure turn duration and peak memory on an iPhone,
not only Bun. Remove Excord's digest mutation and its competing snapshot path only when the same
drives pass; retain the protocol parser and any necessary bounded scan.

### 13.6 D12: make paging and row-position reports sufficient for a transcript

Add `reach-margin` to virtualized lists: one nonnegative CSS length or percentage, default `100%`.
A percentage is relative to the scrollport's extent along the list's scrolling axis. It says how
far before either content edge to fire `reachstart` or `reachend`; zero fires at the edge. This
keeps today's one-screen default and permits an author to choose a longer lead, as the existing
issue requests. Rearm after leaving the region, or after that edge's content range grows, with
at most one delivery per edge per committed geometry change. An unchanged range does not rearm
merely because a frame passes. The app still guards a pending page request; the list does not
know its network state. Pin these rules in the collection tests before updating the recipe.

After a jump and row measurements settle, the scroll report's first-visible key and offset must
describe the measured viewport even if the physical offset did not change. Deliver a correction
only when those facts change. A completed jump belongs to the collection identity that accepted
it. Switching to another channel's collection cancels that jump; replacing the keyed window
inside the same collection may fulfill it, including in the commit that brings its target in.

Extend the existing chat recipe or its linked example with channel identity, early paging,
restoration and jump completion. Preserve the recipe byte budget. Delete Excord's row-distance
prefetch workaround when the edge event covers it. Keep Discord's history gaps, unread rules,
optimistic message IDs and permission logic in Excord.

**Acceptance:** fling while history replies are delayed; jump to an unmeasured key while replacing
the window; prepend and resize rows above the viewport; switch channels during a pending jump;
restore after a measurement correction. Assert the visible key and offset as well as the screen.
A deliberately inadequate margin may reach the edge; a sufficient margin must avoid it without
fetching the entire history. Run on web and native hosts and report any unsupported cell.

### 13.7 D13: verify the app's scale and the host actually being shipped

Use Excord's existing fixture for the large READY, 3,000 DMs, 150-channel server, numeric-zero read
position, delayed history and concurrent incoming messages. Include representative PNG, JPEG,
WebP and GIF responses and a failed decode. Fixture data remains synthetic; automated drives
never use the real account. Shared rendering code alone does not qualify Windows or Android.

Complete D7's fixture isolation: one port per checkout, explicit overrides winning, and an
ownership check that refuses another checkout's server. Development loopback-grant relocation
must cover the selected port without broadening production grants. Report the app, host build,
plan digest, fixture origin and delivery path in the drive's existing output. Select a compatible
test archive, and mark an incompatible fixture as not run rather than misdiagnosing a product
failure or silently counting a skip as a pass.

Run macOS XCTests in the existing async lane with retained failure attribution, after resolving
or explicitly classifying the baseline failures in §12. Exercise the supported Android emulator
ABI through the existing build and driver rather than relying on an unlanded patch. A compiled
APK, an installed APK and a driven app are separate results. Keep the six blocking checks and
their budgets unchanged.

**Acceptance:** two checkouts drive isolated fixture worlds concurrently; a wrong archive is
identified before its unrelated tests run; a native failure names the tested build; the same
chat scenario is driven on each claimed host. Tests wait on observable completion, following
[docs/testing.md](../docs/testing.md), rather than adding fixed startup sleeps.

### 13.8 Sequence and decisions requested

| Order | Deliverable | Completion evidence |
|---|---|---|
| 1 | D8 real input probe and independent live observation; D9 resource inspection | A captured native bounce and one inspection of the stale-list failure |
| 2 | D10 work counts; D13 fixture isolation and correct host verification | A measured server switch and two independent fixture drives |
| 3 | D11 ordered asynchronous mappers, then the separable inflater if warranted | Large READY and retirement cases pass; app scheduling code is deleted |
| 4 | D12 edge lead and measured row reports | Paging/jump drives pass; app prefetch workaround is deleted |

Each implementation change updates the governing subsystem LLP and author guide in the same
change. A regression belongs at the lowest level that exhibits it, with an app drive where
native behavior is the subject. Apply the three-round fix limit; report an unsupported probe
instead of changing what the test claims to prove. These are proposed stages, not scheduled work.

Requested decisions:

1. **Accept D8–D10 and D13 as the next development-tooling scope?** Recommendation: yes, within
   the existing operations and scripts. Explicitly include independent live observation even if
   automatic native input remains blocked by the environment.
2. **Admit promise-returning stream mappers and optional native decompression?** Recommendation:
   accept ordered mappers first, with Excord's scheduling workaround as the deletion target;
   qualify the inflater separately. This requires amending D7, not treating an RFC draft as
   runtime authority.
3. **Admit `reach-margin` and the corrected list reports?** Recommendation: yes, with the existing
   collection as owner and Excord's scroll-handler prefetch as the deletion target.
4. **Change live resource retention semantics now?** Recommendation: no. First ship D9 and the
   owner-gated example, then decide from a concrete failure whether a new predicate earns its
   place. No implicit reuse of `with` as a live freshness guarantee.

No causal trace, general provenance explorer, session replay, production inspector, new blocking
check or general job scheduler is proposed. Where an accepted change crosses `rules/DEFERRED.md`,
record its admission and take there in the implementation change. Deleting app workaround code
is a measurable benefit, but does not itself replace the doing-list trade that file requires;
the acceptance must name a take or waive it explicitly.
