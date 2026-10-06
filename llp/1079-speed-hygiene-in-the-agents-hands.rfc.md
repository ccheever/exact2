# LLP 1079: Speed hygiene in the agent's hands — `perf`, the tenth operation

**Type:** RFC
**Status:** Accepted with rulings (Charlie, 2026-10-03: "sure" to §Rulings Q1–Q4); r3, as built (2026-10-03). Charlie asked for this document on 2026-10-02 ("yes write up an LLP"). He waived the nine-operation limit for this op the same day: *"it's probably worth a waiver to a 10th op I think"* (§7).
Astra reviewed r1 (`llp/reviews/1079-speed-hygiene-in-the-agents-hands.astra.md`, NOT READY, 13 findings). r2 folds all thirteen; the dispositions are in §9. r3 is the implementation's review of r2 against the code: what r2 got wrong or left unbuildable, and what changed, is in §10. All four stages landed with it (§As built).
**Systems:**
- Runner (`runner/src/perf.rs`, `runner/src/runner/perf.rs`): per-site work totals that survive node lifetimes, kept beside the instance ids; the receipt tally (authored and inherited) against the runner's own ops; `seq`; the switch a host turns (`Runner::measure`); the runner's half of `perf`.
- Kernel: nothing new. `CommitReceipt`, `LayoutReceipt` and the equality skips are read as they are.
- Hosts:
  - the switch, from the baked trust they already hold;
  - the `moved` tally from the layout receipts they already consume (Apple, Linux);
  - a frame sampler of their own, with active segments and a ring (web `host/web/frames.js`, Apple `FrameSampler.swift`, Linux `host/linux/src/frames.rs`);
  - the host's half of `perf` (`perf frames`);
  - a development-only trace export.
- Batch trailer (web wasm, Apple): gains `seq`, the transactions it carries, while measuring.
- JS web target: the same counters in `host/web-js/perf.js`, a module only a development build references; its journal bounded to the runner's ring size.
- Tooling: `scripts/agent.mjs` (`perf`, `trace <file>`); `scripts/agent-inspect.mjs` (the source-map join, two-read differences, the transcript); the dev servers' `POST /__exact/trace` (`host/web/serve.mjs`).
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5). All four stages landed on 2026-10-03.
**Date:** 2026-10-02
**Related:**
- LLP 1001 §5: `CommitReceipt`, `LayoutReceipt`, the receipt ring.
- LLP 1005 §6: commits, `advance`, the clock.
- LLP 1012 §1–§3: the operations, the tags, the journal and its cursor.
- LLP 1012.000: attached sessions. Not landed; §5 does not depend on it.
- LLP 1035.002: `layout <target>`; D3 (tags); D6 (the source map); D7 (bounded reads, `truncated`).
- LLP 1041 §5: frame-gap proxies, named.
- LLP 1050.000 §6: "the runner neither presents frames nor reads a clock".
- LLP 1054 L10: `component_costs()`.
- LLP 1061 D4: the frame source runs only while something wants it.
- LLP 1073: `every(frame, …)`, the virtual display.
- `rules/DEFERRED.md` §Agent API: the nine; "A tenth operation replaces one of the nine"; "Not shipping: … causal trace, … perf". §Tooling: "no devtools UI".

## Summary

James, 2026-10-02:

> This is a very rough idea. What if Exact had a way to programmatically
> enable instrumentation surgically, like tracking re-renders of specific
> subtrees? Or measuring frame drops as I test the app, and then being able to
> give the agent the trace so it can catch any little speed hygiene issues?

Most of what this needs already exists:
- the kernel's per-commit receipts;
- the runner's deterministic work counters (`InstanceWork`, `runner/src/instance.rs`);
- the runner's site and instance identity;
- the compiler's source map;
- each host's own frame loop.

This RFC adds one operation, `perf`, under Charlie's waiver. It has two forms:

1. **`perf <target>` — work on a subtree** (D1–D2). The reply groups by
   **plan site**. For each site it gives:
   - the instances created and retired, totalled across node lifetimes, so
     churn is visible after the nodes are gone;
   - how often those nodes were in a commit's changed set, split into their own
     changes and changes inherited from above;
   - how often their bindings were evaluated;
   - on Apple and Linux, how often their frames moved.

   The driver joins each site to its component and source line. Two reads
   tagged with the commit sequence give the work done between them; the
   driver subtracts.
2. **`perf frames` — presented frames** (D3–D4). Each host samples frames only
   inside defined *active segments*, using a period whose source is named. Every
   record carries the commit-sequence range it presented. The reply gives:
   - lifetime counters (presented, late, missed);
   - percentiles over a window that states its range;
   - the late frames themselves.

   A late frame is also one journal line, so it reads in order with the
   commits around it.

A person's session reaches the agent as a **trace file** (D5), exported from a
development build and read by `agent.mjs trace <file>`. In production
collection is off.

The agent checks a hygiene fix by **work counts**, which repeat under
controlled inputs. It reports times as observations with their proxy named,
never as a gate (D6).

## Motivation

- **The facts exist and are discarded.**
  - The kernel keeps the last 64 `CommitReceipt`s (`RECEIPT_RING`,
    `kernel/src/kernel.rs:37`), and nothing outside tests reads them
    (`Kernel::receipts`, `kernel.rs:876`).
  - The runner counts its own work per commit (`InstanceWork`: nodes visited,
    rows keyed, reused and scanned, bindings and derives evaluated). Only
    tests read it (`Runner::last_instance_work`, `runner.rs:1237`;
    `contract/cli/tests/it/lists.rs:155-172`).
  - The journal records a commit as counts only,
    `what → epoch E (+created −destroyed ~touched)`
    (`runner/src/runner/lines.rs:20-35`). It does not say which nodes or which
    sites, so "this list re-commits every row on every tick" cannot be seen
    without a debugger.
- **Frame timing exists, but as three shapes, two of them stderr only.**
  - iOS `EXACT_FPS=1` prints a once-a-second line and a corner label
    (`Session.swift:1373-1385`).
  - Apple `EXACT_TIMER_TRACE=1` prints per-second apply counts
    (`SessionTimerTrace`, `Session.swift:1280-1299`).
  - Linux puts one paint time in `state.paint` (`host/linux/src/agent.rs:181-184`).
  - The web's `metrics.mjs --stress-url` sampler writes JSON with an identity
    block (`scripts/stress-metrics.mjs:136`). It is headless Chrome and makes
    no physical-display claim (`stress-metrics.mjs:1`).

  None of them can say which commits a late frame carried.
- **Agents are good at hygiene, given evidence.** Each of these becomes a
  mechanical finding once the work and the late frames are in front of the
  agent:
  - "A row re-evaluates eight bindings per tick, and one changes."
  - "A timer drives layout faster than the display."
  - "A list retires and recreates rows it could keep."

  From a screenshot they are invisible.

## Design

### D1 — What is counted, and in what unit

Every counter is defined in a unit that already exists. That keeps the web
wasm target, the JS target and the native hosts comparable.

**Per site, across lifetimes** (the runner; `perf.js` on the JS target).
The counters survive node retirement until the runner ends. They live beside
the instance ids (`runner/src/instance.rs`, `Ids`), because every instance is
realized there, and the runner's half is `runner/src/perf.rs` and
`runner/src/runner/perf.rs`.

| counter | unit | source |
|---|---|---|
| `created` | instances realized at this site | `NodeInst::create`, where the view's site is recorded. Every realization counts, including a view created and destroyed in one batch, which no receipt shows (`kernel/src/txn.rs:58`) |
| `retired` | instances of this site no longer live | the views the runner remembers (`Ids`) whose kernel node is gone, counted at the read, plus those it has forgotten (`Ids::retain` keeps the count). A destroyed view's descendants are never visited by the runner, so retirement is read from liveness, not from a destroy call |
| `evaluated` | binding expressions evaluated for instances of this site | `emit_bindings`, where `bindings_evaluated` increments. A constant binding's cached value is not an evaluation |
| `unchanged` | evaluations whose result equalled the last one, so no op was built | the equality skip right after |
| `authored` | kernel transactions in which an instance of this site is in the receipt's `touched` set and an op of that transaction names it | the receipt, classified by the runner against the targets of its own ops (`SetProp`, `ClearProp`, `SetStyle`, `ClearStyle`, `SetChildren`). `touched` is deduplicated, so each transaction counts a node at most once. The reorder clean-up transaction is a second transaction and counts as one |
| `inherited` | kernel transactions in which an instance of this site is in `touched` and no op names it | the same classification: an inherited value from above, the document's language or direction, a resolved relative unit. The kernel is unchanged: it already writes `touched`, and the runner already holds its ops |
| `moved` | layout passes in which an instance of this site is in `LayoutReceipt.changed` | **the host**, where layout is consumed (Apple `layout.rs` and the content region's layout, Linux `height.rs`), through `Runner::moved`. Layouts with no commit count: resize, projection, region. A created node's first layout counts. Absent, never zero, on the web: the kernel does not lay out there (LLP 1007 §9) |

**What `authored` and `inherited` are not.** Together they count membership
in a changed-node receipt, not binding evaluations, which are `evaluated`.
- The kernel already skips a set that leaves a prop, style or children equal
  (`txn.rs:595`, `:669`, `:716`).
- The runner already skips an evaluation whose result equals the last one.
- A node can be in `touched` because an inherited value changed above it,
  with no binding of its own going stale (`txn.rs:1138-1141`).

The split is ruled (Q3): "Row changed 732 times" must say whether the rows'
own bindings did it or one parent style dragged every descendant along. The
**hygiene number** is `unchanged / evaluated`: work the runner did that
produced nothing. It is read from branches that already exist. No new
comparison is added.

**On the JS target** the counters live in `host/web-js/perf.js`, a module only
a development build references. `rt.js` carries none of them; it gains only
the re-export and the bounded journal (D4).
- `evaluated` and `unchanged`: the emitter wraps each dynamic binding of a
  development build in `pf(site, f)` (`emit.rs`, beside `data-site`). One
  expression can feed several effects (a shorthand's longhands, a paint fact),
  and each effect that runs it is an evaluation, as the page really does that
  work. `unchanged` compares against what that same effect last got (keyed by
  `owner()`). Literal rows are compiled into classes, so they are never
  evaluated: the JS target's `evaluated` is smaller than the runner's for the
  same plan, by design.
- `authored`, `created`, `retired`: the document's own mutation records,
  taken at each commit's `Before` and `After` hooks. An element named by a
  commit's records is authored once per commit. An element inserted into the
  document is created, and one removed and still detached at the end of the
  commit is retired. A moved element is neither. Records made between commits
  (the motion piece's frames) count structure, never authorship.
- `inherited` and `moved` are absent: the browser cascades and lays out.

**Cost and gating.**
- **Production trust: collection off.** The host tells the runner once,
  after boot: `Runner::measure(on, moves)` (ruled, Q2: never a field the app
  can read). The Apple, Linux and web wasm hosts already hold the baked
  `compat.json` and decide from its `inputs.trust` (`delivery::production`),
  so no new export is needed. Off, each counter is one branch on an empty
  table. A production JS build references neither `pf` nor `perf.js`. This is
  not a cargo feature, so the ban on optional capability in core crates is
  untouched.
- **Development and agent builds: on.** Measuring begins after boot. Instances
  already live count as created then, and boot's evaluations are not counted,
  since they are the plan's static cost, not hygiene. The memory is one row of
  seven counters per plan node. The acceptance target is ≤1% on each of:
  - `metrics.mjs --scaling` at 10000 rows (the runner);
  - the JS target's grid ticker (`web-framework-bench/apps/grid`);
  - the `--stress-url` sampler's p95 frame gap with collection on and off.

  Stage 1 measures these. The results are in §As built.

### D2 — `perf <target>`: the work read

```
{"op":"perf"}                 // every root's sites
{"op":"perf","target":V}      // the sites under V (a view id or testId, as `tree`)
```

The reply is tagged like every other read (LLP 1035.002 D3), with these
fields:
- `epoch`, `incarnation`, `clock` as usual.
- **`seq`**: the runner's count of kernel transactions applied (on the JS
  target, its committed epoch: a refused action is no transaction)
  (`Runner::seq`, the producer batch number it already keeps). Epoch is not
  enough, because layout can leave the epoch where the commit put it
  (`kernel.rs:509`).
- **`plan`**: the plan digest.

The body:

```
{ "seq": 412, "incarnation": 1, "plan": "<digest>",
  "sites": [ { "site": 58, "instances": 12, "live": 12, "created": 12, "retired": 0,
               "evaluated": 5904, "unchanged": 5172, "authored": 732, "inherited": 0, "moved": 0 }, … ],
  "walked": 37, "truncated": false }
```

- **Site-wide totals, target-scoped instances.** The counters are each
  site's, across every instance of it anywhere. `instances` is how many of
  them are live under the target; `live` is how many are live anywhere, so
  `created − retired = live`. A target inside a repeated region (one row of a
  list) shows the whole site's totals; the driver prints `instances` beside
  them.
- **One pass.** The runner walks the target's kernel subtree once. It
  attributes each view through the site table (`Ids::site`, a map lookup). It
  reads no props and builds no rows: this is not `tree`'s materializer.
- **Bounded.** It walks at most 20000 instances and returns at most 500 sites.
  Past either bound the reply carries `truncated: true` and `walked`, and the
  driver labels the read partial.
- **Retired instances count.** A site whose instances have all gone still
  appears when it sits statically under the target's site, and in a read of
  every root whenever it did any work. The runner follows the plan's node and
  arm parents; `perf.js` follows the site each site was first realized under.
- **Deltas belong to the driver.** There is no `since` and no mark: a read
  changes nothing. `perf <target>`, then a drive, then `perf <target>` again,
  and the driver subtracts. It refuses the difference by name when a read
  names no plan, when the reads name different plans or incarnations, when
  `seq` or any counter went back, or when the first read was partial (a site
  it left out would read its whole lifetime as new work). A runner restarted in place starts a new kernel whose incarnation is
  1 again, so the incarnation alone does not identify a run. The CLI form
  `perf <target> during "<op>" "<op>" …` does exactly this.
- **Source.** The driver joins `plan` and `site` through the source map
  (LLP 1035.002 D6), as `layout <target>` does. On the JS target the page
  names its own plan: a development build writes the served plan's digest
  into its entry, so a page from another build cannot borrow a newer map. No host reads the map. A component used twice
  is two sites; the driver names each by its nearest call site.

From caltrain's station list before its fix (§As built), on Linux:

```
$ bun scripts/agent.mjs linux "tap change-station" "perf stations-screen during \"tap station-mv hover\" \"tap station-paloalto hover\""
stations-screen — seq 2..5 · clock 0 ms · incarnation 1
  component   site                          instances  created  retired  evaluated  unchanged  authored  inherited  moved
  StationRow  app.contract:343 (from :306)  1          2        1        16         0          0         0          2
  StationRow  app.contract:349 (from :306)  8          1        2        0          0          0         0          1
  …
```

Each hover retired one row's five views and realized five more, because the
highlight was a `when hot` arm. After the fix, one row with a bound
background, the same drive creates and retires nothing.

### D3 — Frames: active segments, a named period, and a join by sequence

Each development host samples frames inside **active segments** and keeps a
ring of the last 600 samples.

**The sampler is its own observer.** It is a callback of its own on the
display (a `requestAnimationFrame` loop on the web, a `CADisplayLink` on
Apple). It never keeps the app's frame source running: r2 had development
builds hold the source open while there was visible work, which would make
the measured build schedule frames differently from the shipped one. The
sampler only watches. On Linux the display loop's own page flips are the
samples, since there is no compositor to observe beside it.

**Segments.**
- A segment starts on **activity**: input (pointer, touch, key, wheel), a
  scroll, a commit or applied batch, the app's frame source running, or the
  page or app becoming visible. Each host lists the activity it hears in
  `covers[]`.
- The first callback of a segment sets the baseline and is **not a sample**.
  The idle gap before newly applied work never counts as late.
- A segment ends 500 ms after the last activity, unless an animation is still
  running (the web: `document.getAnimations()` has one running), or when the
  page hides or the app leaves the foreground.

**The period and where it comes from** (`period{ms, source}`):

| host | `source` | how |
|---|---|---|
| Apple | `target` | `targetTimestamp − timestamp`, as the canvas code already does because `duration` is wrong under ProMotion (`Session.swift:1345-1349`) |
| Linux | `refresh` | the mode's refresh rate. A frame wanted before the previous flip completed: `missed` is the flip-sequence gap − 1, not a division. A frame wanted after an idle gap: measured from when it was wanted, `floor((flip − wanted) / period) − 1`, so a slow first frame is late and idleness never is |
| web | `floor` | the lowest median of 8 consecutive intervals in the current segment, never raised by a slower phase inside the segment, estimated afresh each segment (the display's rate may change between). The segment's samples before its floor exists are classified once it does; a segment too short for one uses the previous segment's, or stays unclassified (`missed: null`). Not `pace.js`'s fitted `period_ms`: that starts at zero and adopts a sustained double-slot cadence as the new period (`pace.js:20`, `:79`), which would normalize sustained jank |

`missed = round(interval / period) − 1` (Linux: the sequence gap − 1). A
sample is **late** when `missed ≥ 1`. VRR and ProMotion are covered by the
`target` source on Apple. On the web a variable-rate display shows as a
`floor` source, and its lateness is approximate. The reply says so through
`source`.

**The record**, one per sample:

```
{ t, interval, missed, seq: [first, last] | null, batches, apply, layout?, loaf? }
```

- **`t` and `interval`** are the host's observation times, on its own wall
  clock converted to the runner's elapsed clock. They are not the runner's
  `now`: `Runner::log` stamps the runner's last logical time
  (`runner.rs:965`), which is not when a frame was seen.
- **`seq`** is the range of kernel transactions whose batches were applied
  since the previous sample, or null when none was.
  - The web wasm and Apple hosts apply batches the runner produced elsewhere.
    iOS applies an asynchronous fill on a later main-thread turn
    (`Session.swift` `landFill`). So the batch trailer gains `"seq":[a,b]`,
    the runner's `seq` before and after producing it (`host/web/src/batch.rs`,
    `host/apple/src/batch.rs`).
  - Linux applies in-process and reads `Runner::seq` at each flip.
  - The JS target counts its commits (`perf.js`).
- **`apply`** is host-timed, the time spent applying batches since the
  previous sample: the JS target's commits, the web glue's `apply`, Apple's
  `Session.apply`. **`layout`** and **`paint`** are Linux's (`Host::layout`,
  `paint.rs`). Apple's layout runs inside the runtime call that produced the
  batch, and the record leaves it absent rather than guessing.
- **`loaf`**, web only, comes from Chrome's Long Animation Frames API. It
  exists only for a frame that produced a `long-animation-frame` entry, which
  are frames over 50 ms. Its fields:
  - `script` is the sum of `scripts[].duration`;
  - `styleLayout` is `startTime + duration − styleAndLayoutStart`, the
    reported style-and-layout tail, present only when `styleAndLayoutStart`
    is non-zero.

  The observer delivers late. An entry is joined, once, to the retained
  sample whose interval `(t − interval, t]` overlaps `[startTime, startTime +
  duration]` most. The sample's `t` is the callback of the frame *after* the
  long one, so it lies past the entry's end. An entry that overlaps no
  retained sample is dropped and counted in `loafUnmatched`. Shorter late
  frames carry no `loaf`, and no value is invented for them.

### D4 — `perf frames` and the journal

```
{"op":"perf","frames":true[,"late":N]}
```

Reply:

```
{ "period": {"ms": 8.33, "source": "target"}, "covers": [...],
  "lifetime": {"presented": 18210, "late": 41, "missed": 97, "segments": 33},
  "window": {"from": 4721.0, "to": 9712.4, "samples": 600, "dropped": 17610,
             "p50": 8.3, "p95": 9.1, "p99": 25.0, "max": 41.7},
  "late": [ {t, interval, missed, seq, apply, layout?, loaf?}, … ] }
```

- **Lifetime counters** run since boot or incarnation. They are never
  windowed and never reset by a read.
- **Percentiles are over the ring only.** `window` names its range, its
  sample count and how many samples it no longer holds, as the journal's
  cursor does (`from`, `dropped`; LLP 1012 §3).
- **`late`** holds the most recent N late records (default 20, at most 100).
- **Under the agent's clock** the reply is `{"virtual": true}` and nothing
  else. Several frame tasks can commit at one virtual instant
  (`commit.rs:164`), so a "virtual frame count" would be ill-defined, and no
  frame was presented.
- **A live window** (2026-10-04, the platformer's diary R11: a game's 60 fps
  had no measure under the driver): `{"op":"perf","frames":true,"live":ms}`
  lends the page's clock to the wall for `ms`. Each animation frame advances
  the runner to the wall's time, a game world draws on its own frame loop with
  its perf armed, and a sampler of the window's own measures what was
  presented; the reply is the one above plus `live: {ms, from, to}` (the
  clock it moved, which the driver's follows) and each world's `perf`. The
  web's wasm target has it (`frames.js` `liveFrames`, `gpu-glue.js` `live`);
  the JS target, Apple and Linux still answer `virtual`, and the driver says
  so. The world ticks on the wall inside the window, so its hash afterwards is
  not a seeked run's, and headless Chrome's frames are its own clock's, not a
  display's (`game/bench/feel.mjs` measures a headed one).
- A host that cannot observe presentation answers `{"unavailable": true}`.

**A late frame is also one journal line**, appended by the host through
`exact_log`:

```
t=4812.4 frame late: 4 missed (41.7 ms / 8.33 target) seq 812..814 apply 22.1 layout 9.4
```

The journal's existing `t=` is the runner's logical time when the line was
appended. The line's own figures are the host's observations. Only late
frames are journaled: at 120 Hz a line per frame would turn the 4096-line
ring (`JOURNAL_RING`, `runner.rs:431`) over in 34 s.

**The join is a count and a window, not adjacency in the journal.** Journal
lines carry no `seq`, and commit lines do not cover every commit: a frame
task's commits are not journaled individually (`commit.rs:173` journals only
failures), and the JS target journals no successful commit. Giving every
journal line a `seq` would change the line format every journal test pins.
So the driver joins a late frame to what was happening around it this way:
1. The record's `seq` range says how many transactions the frame carried.
   That count is the authority.
2. The journal lines appended just before the frame's own line are shown
   beside it: at most five, and none from before the previous late frame.
   Position, not the `t=` stamp, decides, because `t=` is the runner's
   logical clock, which need not be the frame's. A wasm page adopted from a
   checkpoint keeps the render's clock until its first advance.
3. The count is printed beside the lines, so transactions no line accounts
   for (frame tasks, the JS target's commits) are visible, never omitted.

This is juxtaposition with stated coverage, not a causal trace (§7).

**The JS target's journal** was an unbounded array. Stage 1 bounds it to the
runner's 4096 lines (`journal.start` is the oldest line's index; `logs`
reports `from` as the runner's does), because a human session (D5) would
otherwise grow it without limit.

### D5 — A person's session, handed to the agent as a file

James's case is a person using the app normally. That run is not in agent
mode: no carrier is open and the clock is the wall's. LLP 1012.000's attach
is not landed. The hand-off is a file.

- **Development builds only.** Collection is off in production trust (D1). The
  export is removed by the same gate that drops `EXACT_AGENT*` and ships
  `AGENT_ADMITTED=false` (LLP 1069.007 §4).
- **Writing it.**
  - Apple: **Save Trace** is its own item (ruled, Q4): in the iOS dev sheet
    beside Copy Info, and in macOS's Develop menu beside App Info… as ⌥⌘T. It
    writes `trace-<wallclock>.json` to the app's temporary directory and
    journals the path. On a simulator that directory is on the Mac's disk. On
    a phone it is copied off the device as any app-container file is.
  - Web: `dev.mjs` serves `POST /__exact/trace`. A development page posts on
    ⌥⇧T, and the file lands in the app's `target/traces/`. `dev.mjs` adds the
    identity and the source map, which it has.
  - Linux: `SIGUSR1` writes it to the temporary directory and journals the
    path.
- **The file holds:**
  - **`identity`**, in the shape `stress-metrics.mjs` already writes (commit,
    dirty tree, platform, arch, CPU, browser or OS version), plus host, app,
    build trust, incarnation, boot wall time and the clock origin;
  - **`plan`**, the digest, and **`map`**, the source map itself, where the
    exporter has it: `dev.mjs` does. A native app does not carry its map, so
    its trace names the digest, and `agent.mjs trace` finds the map the way
    the live driver does (the development plan, the bake's outputs). A trace
    whose map is in neither place prints sites by number and says why;
  - **`proxies`**, naming each timing's provenance (`period.source`,
    `covers[]`, `loaf` availability), as LLP 1041 §5 requires;
  - the journal with its `from`/`dropped`; `perf frames` with up to 600
    records; and `perf <root>` with sites only (no per-node records, so the
    size is bounded by the plan).
- **Reading it.** `bun scripts/agent.mjs trace <file>` renders the same views
  as the live reads: late frames joined by `seq`, the frame summary, and the
  site table with source lines. No app needs to run.

### D6 — What the agent checks after a fix

A hygiene fix is shown by **work counts**:

1. `perf <target> during …` drives fixed steps under the agent's clock.
2. Apply the fix and drive the same steps.
3. Compare `evaluated`, `unchanged`, `authored`, `inherited`, `created` and `retired` per
   site.

The counts repeat **when the inputs are controlled**: the same steps, the
same resource answers, in the same completion order. Requests answer
independently of seeks (LLP 1012, "Requests in flight"). Image
sizes that arrive asynchronously can relayout, so `moved` repeats only when
images are settled. Under `EXACT_AGENT_TIMING=platform`, UIKit's transitions
run in real time (`Session.swift:65`), so `moved` is not compared under that
timing. The driver prints these conditions beside any diff it shows.

Times are observations. Each is printed with its proxy ("p95 interval 16.9
→ 8.4 ms, web `floor` period, rAF-callback gap, this Mac"). None is a check:
`RULES.md` makes a flaky check worse than none.

### D7 — What is ruled, and what stays refused

- **Ruled (Charlie, 2026-10-02):** a tenth operation is worth a waiver
  (*"it's probably worth a waiver to a 10th op I think"*), as `prefer` was
  (2026-09-27). `perf` is that operation, and it takes no operation away.
  `rules/DEFERRED.md` changes in stage 1's PR, when the operation lands:
  - the list becomes ten;
  - the waiver is recorded beside `prefer`'s;
  - "perf" leaves "Not shipping", narrowed to this RFC.
- **The take.** `EXACT_FPS`, its corner label, and `EXACT_TIMER_TRACE` are
  deleted in stage 3: their numbers are `perf frames`, one shape on four
  hosts. `EXACT_CANVAS_STATS` stays. It splits the canvas's GPU passes, which
  a frame record does not.
- **Still refused:**
  - **Causal trace.** No dataflow, no "which write caused this commit". The
    join is by sequence range with stated coverage.
  - Record/replay.
  - A devtools UI, flame charts, the Chrome trace format. Instruments and
    Chrome's Performance panel run the host's code unchanged.
  - Production telemetry, which is EAS Observe's.
  - A per-frame journal line for every frame.
- **Not split into forms of `tree` and `state`.** r1 did that. Counts are not
  a property of the tree's shape, and a frame window is not the app's state;
  `state` "marks" would make a read mutate. One honest operation was
  preferred.

## §8 Stages

All four landed on 2026-10-03, in one change; §As built has what each showed.

1. **Web, work only.** The runner's site totals, `seq` and reply; `perf.js`
   and the emitter's `pf`; the driver's `perf` and `during` and the
   source-map join; the JS journal bounded; the production-trust switch;
   `DEFERRED.md` amended; D1's cost measured. Exit: on `apps/caltrain`, `perf`
   shows a known needless churn, and D6's difference shows the fix.
2. **Web frames.** Segments, the `floor` period, the wasm trailer's `seq`,
   LoAF, `perf frames`, the late line in the journal, `POST /__exact/trace`
   on both dev servers, and `agent.mjs trace`.
3. **Apple.** `moved`, the trailer's `seq`, the sampler and its `target`
   period, Save Trace in both dev menus. `EXACT_FPS`, its corner label and
   counters, and `EXACT_TIMER_TRACE` are deleted.
4. **Linux.** `moved`, the flip-sequence `missed` on the display loop,
   `SIGUSR1`; the headless agent's `perf frames` is virtual.

## As built (2026-10-03)

**Stage 1, the exit.** On `apps/caltrain`, `perf stations-screen during "tap
station-mv hover" "tap station-paloalto hover"` showed each hover retiring
one `StationRow`'s five views and realizing five more: its highlight was a
`when hot` arm. After the fix, one row whose background and testId are
bound, the same drive creates and retires nothing. Each hover now re-evaluates
every row's `hot` (54 evaluations, 48 unchanged, over three commits), which
is the binding's honest cost. The runner hosts (Linux, macOS, iOS on a
simulator, web wasm) agree on every counter they share. The JS target's
`evaluated` is smaller (literal rows are classes) and its `seq` is the same.
`contract/cli/tests/it/instance_work.rs` pins the counters: churn surviving
retirement, `unchanged/evaluated`, `inherited` against `authored`, a read of
every root, and production trust answering no counts.

**What D1's switch costs.**
- The runner: `metrics.mjs --scaling`'s runner loop (300, 3000 and 10000 rows;
  bump, reorder, topology; p50 of 40), measuring off against on, moved
  −2.7% to +1.8%. That is run-to-run noise, with no direction. The full
  `--scaling` run did not finish in 30 minutes on this Mac: its web-host half
  re-folds a parent once per created row (`host/web/src/host.rs`
  `emit_receipts`), which is quadratic at 10000 rows. That predates this
  change and is in `QUEUE.md`. The runner half was measured with that half
  left out.
- The JS target's development page: 800 hover commits on caltrain's station
  list took 9.7 ms against 6.7 ms with `perf.js` stubbed out, about 3.7 µs a
  commit (+45% of an 8 µs commit). Two-thirds is the mutation observer, the
  rest `pf`; neither got cheaper by small changes. **The ≤1% target is not
  met on the JS target's development page.** A production JS build was
  checked to carry none of `perf.js`, `frames.js` or `pf`. The grid ticker
  r2 named (`web-framework-bench/apps/grid`) is not in this checkout, and
  the `--stress-url` p95 comparison was not run.

**Stage 2.** A headless Chrome session on the dev server (JS target and
`--wasm`), not in agent mode, with a 120 ms script-blocked frame. The sampler
recorded it late (6 missed against a 16.7 ms floor), joined Chrome's LoAF
entry (`script 120`) and journaled it. ⌥⇧T wrote
`target/traces/trace-….json` with identity, proxies, journal, frames, `perf`
and the embedded map, and `agent.mjs trace` read it back with no app running.

**Stage 3.**
- `FrameSamplerTests` (macOS) pins: the trailer's `seq`; a late frame
  counted against the link's target period, journaled and saved by
  `saveTrace`.
- On the iOS simulator outside the agent, the sampler ticked continuously
  while caltrain's sky animated.
- Under `EXACT_AGENT_TIMING=platform` it read real frames, `seq` joined
  through the trailer.
- The Save Trace menu items were built on both platforms but not clicked by
  hand. (2026-10-05: on iOS it was, on a simulator. A phone's trace leaves
  it two ways: the saved alert's **Share…** (AirDrop to the Mac, or Files),
  and `bun scripts/agent.mjs trace --phone <name>` over the cable, which
  copies `tmp/trace-latest.json`, the last trace under a fixed name, with
  `devicectl` as a phone's screenshots are copied, into the app's
  `target/traces/` and reads it (a simulator's from its container, which
  `devicectl` cannot copy). Neither has been run on a phone.)
- `BorderParityMacTests.testEveryCaseMatchesChromeOnScreen` fails the same
  way at the base commit; it is not this change's.

**Stage 4.** `host/linux/src/frames.rs`'s arithmetic is unit-tested: vblank
gap, slow first frame, idle, reload. The display loop and `SIGUSR1` were
type-checked for `aarch64-unknown-linux-gnu`, not run on a DRM display. That
needs a Linux machine with a VT. The headless agent answers `perf frames` as
virtual.

**Review.** Astra (gpt-6-astra, xhigh) reviewed the implementation twice
(`llp/reviews/code-2026-10-03-perf.astra.md`).
- The first pass, NOT READY with 14 findings. Thirteen were changed in code.
  The fourteenth, the LoAF join, was fixed in D3's text, which still said
  r2's rule.
- The second pass, NOT READY with 10 findings. Four of those changes were
  incomplete: a Linux reload with an unchanged `seq`, a reset that kept its
  callback, Linux demand recorded after the work, and a GPU canvas's loop
  unwatched. One introduced a double count of a child added under an added
  parent. It also found `pf` built per evaluation inside mapped writes, an
  unmeasured Linux update activation and content-region boot, and
  production clock reads on Linux. All ten were fixed.
- The third and last pass confirmed those fixes and found two Linux items: a
  flip awaited across a host replacement, and a clock read in production.
  Both were fixed.

## Rulings (Charlie, 2026-10-03)

Charlie leaned `perf` on Q1 and asked for a recommendation on Q2–Q4; he
accepted the four recommendations below ("sure").

- **Q1 — The name: `perf`.** It covers frames as well as work, and it is
  DEFERRED's own word for this.
- **Q2 — The trust switch: a private ABI flag.** The host tells the runner at
  boot. It is never a reserved field the app can read: an app has no business
  branching on whether it is measured (D1). As built, the switch is
  `Runner::measure`, which only the host calls. Each host decides from the
  baked `compat.json` it already holds, so no new export was needed (§10).
- **Q3 — Split `changed` into `authored` and `inherited`.** One counter more,
  on a path the kernel already distinguishes (`txn.rs:1138-1141`). Without
  it, a parent style flipping reads the same as every row being wasteful
  (D1).
- **Q4 — Export: a separate Save Trace item, not Copy Info.** One copies text
  and the other writes a file. The keys and paths are as D5 has them: the dev
  menu's Save Trace on iOS, ⌥⌘T in macOS's Develop menu, ⌥⇧T on the web
  through `dev.mjs`, `SIGUSR1` on Linux.

## §9 Review dispositions (Astra, r1)

| # | finding | r2 |
|---|---|---|
| 1 | period could manufacture or hide drops | D3: Apple `targetTimestamp − timestamp`; web `floor`, not `pace.js`; `source` named; example corrected to 4 missed |
| 2 | segment boundaries and uncovered loops | D3: segments, the baseline sample, sources kept running in dev, `covers[]`, the three web loops named |
| 3 | retirement erased churn | D1: per-site `created`/`retired` across lifetimes, counted where the runner realizes, including within-batch churn |
| 4 | `since E` and `mark` | D2: removed; two reads tagged `seq` + `incarnation`, the driver subtracts |
| 5 | no frame-to-commit join | D3/D4: `seq` in the trailer and every record; join by range with unjournaled commits counted; JS journal bounded |
| 6 | kernel already compares; touches ≠ evaluations | D1: `changed` defined as receipt membership; `evaluated`/`unchanged` from existing branches; inherited touches split (ruled, Q3) |
| 7 | JS units differ | D1: one per node per commit via stamps; `adopted` separate |
| 8 | runner does not consume layout receipts | D1: `moved` counted by the host at its layout consumers, including layouts without commits; reorder clean-up defined |
| 9 | no identity in `tree`, unbounded work | D2: its own op, site-grouped, one pass, walk and site bounds, `truncated` |
| 10 | LoAF limits | D3: long frames only, `startTime + duration − styleAndLayoutStart`, late delivery joined once, `loafUnmatched` |
| 11 | ring vs "since boot" | D4: lifetime counters plus a window that states its range and drops |
| 12 | virtual clock ≠ repeatable by itself | D6: conditions stated; D4: virtual reply is `virtual: true` only |
| 13 | production cost, file identity | D1: off in production, ≤1% as an acceptance target on three measurements; D5: identity, embedded map, proxies; stress-metrics credited |

## §10 r3: what the implementation's review of r2 changed

r2 was right in shape. Measured against the code, it had these gaps, each
of which would have stopped an implementation or made it measure the wrong
thing.

| # | r2 | the code | r3 |
|---|---|---|---|
| 1 | `retired` counted "the same place, on retirement" | `NodeInst::destroy` emits one `DestroyView` and drops the subtree unvisited; a receipt's destroyed keys no longer resolve to view ids | D1: read from liveness. The views the runner remembers (`Ids`) whose kernel node is gone, plus a count kept when it forgets them. Within-batch churn is covered, since every realization is remembered |
| 2 | `inherited` from "the kernel's inherited path", with "Kernel: nothing new" | `touched` does not say why a node is in it; telling would change the kernel | D1: the runner classifies each touched node against the targets of its own ops in that transaction. The kernel is unchanged, and `inherited` is named for what it is: touched with no op naming the node |
| 3 | "a private ABI flag" for the trust switch | the Apple, Linux and web wasm hosts already hold the baked `compat.json` | D1: `Runner::measure(on, moves)`, decided by each host from `inputs.trust`; no new export. Measuring starts after boot, live instances seed `created` |
| 4 | the JS target's counters in `rt.js` | `rt.js` is at the 1,500-line cap; `P`/`S` stamps would ship in every production bundle | D1: `perf.js`, referenced only by a development build. The emitter wraps each dynamic binding in `pf(site, f)`, and mutation records give `authored`, `created`, `retired`. A production bundle was checked to carry none of it |
| 5 | site totals "for the target subtree" | a target inside a list is one instance of sites that have many | D2: counters are site-wide; `instances` is live under the target, `live` site-wide, so `created − retired = live` |
| 6 | `nodes: true` per-node counters | a per-slot table and a second reply shape, with no finding in the RFC that needs them | cut. A site-wide total plus `instances` answered every case met here |
| 7 | the driver refuses a diff across incarnations | a runner restarted in place has a new kernel whose incarnation is 1 again | D2: also refused when the plan differs or `seq` or any counter goes back |
| 8 | development builds keep the frame source running while there is visible work | that makes the measured build schedule frames unlike the shipped one | D3: the sampler is its own observer (a rAF loop, a display link); Linux samples its own flips |
| 9 | a late frame joined to journal lines "inside that range" of `seq` | journal lines carry no `seq`, and adding it changes every pinned line; their `t=` is the runner's logical clock (a page adopted from a checkpoint keeps the render's) | D4: the `seq` count is the authority; the lines appended just before the frame's own line are shown beside it, by position |
| 10 | the web period before 8 samples | a cold page's first long frame would set its own period and never be late | D3: until the floor is established nothing is classified (`missed` null) |
| 11 | the source map "copied in at export" on every host | a native app carries no map | D5: the web dev servers embed it; a native trace names its digest and `agent.mjs trace` looks where the live driver looks |
| 12 | Apple's corner label at `Session.swift`; Copy Info on macOS; Save Trace keys | the label is `ExactIOS/main.swift`; macOS has App Info… ⌘D | D5 and D7 name the real places |

## Amendment: an idle timer tick only moves the clock (2026-10-04)

An app timer that writes nothing still commits a batch: Signal Clone's 250 ms
poll, which finds no news, committed one with no ops and only its next
deadline. The Apple session applied each through `Presenter.apply`'s whole
pass: navigation, controls (a kernel face query per native button), menus,
scroll capture and accessibility. That cost 6.5 ms median on the simulator,
four times a second, a 7 ms main-thread stall under any gesture.
- List fills and list feedback already skipped such batches.
  `ExactSession.changesNothing` now names that predicate once: no ops, no
  error, no `controls`, no canvas images, and motion, spatial, frame tasks,
  the canvas and the owed draw unchanged.
- The timer path (`scheduleClock` → `applyUnlessEmpty`) uses it too. An idle
  tick only records its deadline and arms the next timer. So does a scroll
  event (2026-10-05): a `scroll=` handler whose writes show nowhere committed
  an empty batch every frame of a fling, each a ~4.5 ms pass. Other events
  still apply an empty batch, which reconciles a native control that changed
  itself before its handler ran, and seeks native animations to the agent's
  clock (`code-2026-10-05-ios-fling-pass.astra.md`).
- `controls` (LLP 1069.011 §9) is a batch flag the Apple projection sets when
  it suppresses a control's viewless contents: a native button's face, a
  select's options. Such a change puts no op on any view, so without the flag
  a fill or feedback batch that only changed an option was dropped, an
  existing bug this change fixes. `Presenter.applySnapshots` also takes no
  batch that carries it.
- Tests: `IdleTickTests`; `host/apple/tests/it/controls.rs` covers the flag
  from real Contract batches.
- Measured on an iPhone 17 Pro simulator, Signal Clone with a 250 ms poll,
  about 70 s each with six chat pushes and pops, with temporary
  instrumentation around `presenter.apply`:
  - batches applied went from 352 to 89;
  - empty ones went from 313 (median 6.47 ms, 1627 ms in all) to 46
    (88 ms in all);
  - main-thread apply time went from 28.6 to 7.7 ms per second.
- Some things change outside any batch, and an idle tick used to refresh
  them by accident: a subtree's appearance or text size under a control,
  and geometry a sheet replays as it finishes dismissing. Each now asks for
  a projection sync on the next turn (`Presenter.requestProjectionSync`: tab
  bars, controls, grouped lists), so a
  timer app no longer depends on its own ticks. Apps with no timer gain the
  same.
- A deferred GPU module drains its queued surface work when it loads. A
  skipped tick still reports its transactions to the frame sampler.

## Amendment: a turn past its frame's target is late (2026-10-05)

Missed callbacks only catch the main thread when it delays the display link
itself. A main-thread turn that began before a frame's target and ended after
it still commits too late for that frame: the render server shows the last
frame again, while the next callback can arrive on time. On iOS the sampler
now watches the main run loop's turns while it runs (`afterWaiting` first,
`beforeWaiting` last). It records how far the latest such turn ran past the
target as each record's `overrun` (ms). A record with an overrun is late even
when `missed` is 0, and its journal line ends with "main X ms past the
target". `lifetime.overruns` counts them, `covers[]` adds `turns`, and Save
Trace keeps `overrun` beside the other proxies. `agent.mjs perf frames` and
`trace` print both. Turns are a proxy: the deadline that counts is the render
server's, a little after the target, so an overrun of under a millisecond may
still have made the frame. Test: `FrameSamplerTests`.
