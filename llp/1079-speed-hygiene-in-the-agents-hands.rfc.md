# LLP 1079: Speed hygiene in the agent's hands — `perf`, the tenth operation

**Type:** RFC
**Status:** Accepted with rulings (Charlie, 2026-10-03: "sure" to §Rulings Q1–Q4); r2. Charlie asked for this document on 2026-10-02 ("yes write up an LLP"). He waived the nine-operation limit for this op the same day: *"it's probably worth a waiver to a 10th op I think"* (§7).
Astra reviewed r1 (`llp/reviews/1079-speed-hygiene-in-the-agents-hands.astra.md`, NOT READY, 13 findings). r2 folds all thirteen; the dispositions are in §9.
**Systems:**
- Runner: per-site work totals that survive node lifetimes; a per-node changed-receipt tally (authored and inherited); a commit sequence number; the runner's half of `perf`.
- Kernel: nothing new. `CommitReceipt` and the equality skips are read as they are.
- Hosts:
  - the `moved` tally from the layout receipts they already consume;
  - a frame sampler with defined active segments, and a frame ring;
  - late-frame records joined to commits by sequence range;
  - the host's half of `perf`;
  - a development-only trace export.
- Batch trailer (web, Apple): gains the commit sequence range.
- JS web target: the same totals over `rt.js`, and its journal bounded to the runner's ring size.
- Tooling: `scripts/agent.mjs` (`perf`, `trace <file>`, the source-map join, two-read diffs); `scripts/agent-inspect.mjs`.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** Claude (Opus 5.5). Stage 1 (§8) on the web starts once Charlie accepts r2.
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
- the runner's deterministic work counters (`InstanceWork`, `runner/src/instance.rs:313-331`);
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

**Per site, across lifetimes** (runner; `rt.js` on the JS target). These
survive node retirement until the incarnation ends:

| counter | unit | source |
|---|---|---|
| `created` | instances realized at this site | the runner's realization of an instance, not the receipt. A receipt omits nodes created and destroyed within one batch (`kernel/src/txn.rs:58`, `:856`); the runner sees them |
| `retired` | instances retired at this site | the same place, on retirement |
| `evaluated` | binding expressions evaluated for instances of this site | the `bindings_evaluated` increments (`instance.rs:785,792`), attributed to the site being visited |
| `unchanged` | evaluations whose result equalled the last one, so no op was emitted | the equality skip right after (`instance.rs:795-798`) |
| `authored` | kernel transactions in which a node of this site is in the receipt's `touched` set because an op in that transaction set its own prop, style or children | the receipt and the op that touched it. Each transaction counts a node at most once (`touched` is deduplicated, `txn.rs:862`); a node touched both ways counts here. The reorder clean-up transaction (`runner.rs:1292-1295`) is a second transaction and counts as one |
| `inherited` | kernel transactions in which a node of this site is in `touched` only because an inherited value changed above it | the kernel's inherited path (`txn.rs:1138-1141`), which already knows it is that path. `authored + inherited` is the receipt membership r1 called `changed` |
| `moved` | layout passes in which a node of this site is in `LayoutReceipt.changed` | **the host**, where layout is consumed: Apple `host/apple/src/layout.rs:58`, Linux `host/linux/src/height.rs:129`. This includes layouts with no runner commit (resize, projection, region). Absent, never zero, on the web: the kernel does not lay out there (LLP 1007 §9) |

**What `authored` and `inherited` are not.** Together they count membership
in a changed-node receipt. It does not count binding evaluations: those are `evaluated`.
- The kernel already skips a set that leaves a prop, style or children equal
  (`txn.rs:595`, `:669`, `:716`).
- The runner already skips an evaluation whose result equals the last one
  (`instance.rs:795`).
- A node can be in `touched` because an inherited value changed above it,
  with no binding of its own going stale (`txn.rs:1138-1141`).

The split is ruled (Q3): "Row changed 732 times" must say whether the rows'
own bindings did it or one parent style dragged every descendant along. The
**hygiene number** is `unchanged / evaluated`: work the runner did that
produced nothing. It is read from branches that already exist. No new
comparison is added.

**On the JS target** each counter is defined in the same unit:
- `authored` is one per node per commit. A per-element stamp holding the commit
  sequence makes a second write in the same commit a no-op for the count.
  The `P`, `S` and `css` helpers receive the element (`rt.js:718` is one
  update doing a remove and a set; it counts once).
- `created` and `retired` count where `rt.js` builds and drops a site's
  nodes. Adopting server-rendered DOM (LLP 1048.001) counts as `adopted`, not
  `created`.
- `inherited` and `moved` are absent: the browser cascades and lays out.

**Live-node detail** for the target subtree is the same counters per node,
reset when the node's slot is destroyed. It is opt-in in the reply (D2). The
site totals are the default view.

**Cost and gating.**
- **Production trust: collection off.** The host knows the trust it was baked
  with (`Session.swift:59`; the web's `AGENT_ADMITTED`). It tells the runner
  once at boot through a private ABI flag (ruled, Q2), never through a field
  the app can read: an app has no business branching on whether it is
  measured. When off, every counter path is
  one predictable branch. This is not a cargo feature, so the ban on optional
  capability in core crates is untouched.
- **Development and agent builds: on.** The memory is a fixed table per plan
  site, sized by the plan, plus a per-slot table when the node detail is
  wanted. The acceptance target is ≤1% on each of:
  - `metrics.mjs --scaling` at 10000 rows (the runner);
  - the JS target's grid ticker (`web-framework-bench/apps/grid`);
  - the `--stress-url` sampler's p95 frame gap with collection on and off.

  That is an acceptance target, measured in stage 1, not a claim made here.

### D2 — `perf <target>`: the work read

```
{"op":"perf","target":V}                    // sites under V, totals
{"op":"perf","target":V,"nodes":true}       // plus per-node counters, bounded
```

The reply is tagged like every other read (LLP 1035.002 D3), with one extra
tag:
- `epoch`, `incarnation`, `clock` as usual;
- **`seq`**, the runner's count of kernel transactions applied in this
  incarnation. Epoch is not enough: layout can leave the epoch where the
  commit put it (`kernel.rs:509`).

The body:

```
{ "seq": 412, "incarnation": 1, "plan": "<digest>",
  "sites": [ { "site": 58, "instances": 12, "created": 12, "retired": 0,
               "evaluated": 5904, "unchanged": 5172, "authored": 732, "inherited": 0, "moved": 0 }, … ],
  "walked": 37, "truncated": false }
```

- **One pass.** The runner walks the target's instances once. It attributes
  each to its site as it goes, from the tree it is walking, so it does not call
  `site_of` per node (that call searches recursively, `instance.rs:1098`).
  It reads no props and builds no child arrays: this is not `tree`'s
  materializer (`agent.rs:279`).
- **Bounded.** It walks at most 20000 instances and returns at most 500 sites.
  Past either bound the reply carries `truncated: true` and `walked`, and the
  driver labels every aggregate partial. With `nodes`, at most 2000 node
  records. Reply size is measured in `metrics.mjs --inspection`, as LLP
  1035.002 D7 does for `layout <target>`.
- **Retired instances count.** A site's totals include instances that are
  gone. A site whose instances have all retired still appears when it sits
  statically under the target.
- **Deltas belong to the driver.** There is no `since` and no mark: a read
  changes nothing. `perf <target>`, then a drive, then `perf <target>` again,
  and the driver subtracts, provided the two reads share an `incarnation`. A
  different incarnation means the driver refuses the diff by name. The `perf`
  CLI form `perf <target> during "<op>" "<op>" …` does exactly this.
- **Source.** The driver joins `plan` and `site` through the source map
  (LLP 1035.002 D6), as `agent-inspect.mjs` does. No host reads the map.

```
$ bun scripts/agent.mjs web "perf feed during \"tap start-ticker\" \"clock +1000\""
feed (List, app.contract:41) — seq 12..73, clock 0..1000, one incarnation
  component     site                instances  created  retired  evaluated  unchanged  authored  inherited
  Row           app.contract:58     12         0        0        5904       5172       732       0
  Row › Price   app.contract:63     12         0        0        732        0          732       0
  Header        app.contract:44     1          0        0        3          2          1         0
```

The numbers are illustrative, not measured. The finding is the first row:
`Row` re-evaluates eight bindings per instance per virtual frame and seven
come out unchanged. The tick belongs on `Price` alone.

### D3 — Frames: active segments, a named period, and a join by sequence

Each host samples frames inside **active segments** and keeps a ring of the
last 600 samples.

**Segments.**
- A segment starts at the first frame callback after any of these:
  - the frame source starting or resuming from a park (LLP 1061 D4);
  - the page or app becoming visible (`visibilitychange`; the scene going
    active);
  - a sampler start.
- The first callback of a segment sets the baseline and is **not a sample**.
  The idle gap before a newly applied batch never counts as late.
- A segment ends when the source parks, the page hides or the app leaves the
  foreground.

**Keeping the source running while it matters.** The frame source parks when
nothing wants it. So in development builds the host keeps the source running
while there is visible work it would not otherwise see:
- input within the last 500 ms;
- an active scroll;
- a running CSS or Core Animation animation the host started (the web:
  `document.getAnimations()` not empty);
- a canvas wanting frames;
- a batch applied in the last 100 ms.

Each host lists the sources it covers in `perf frames`' `covers[]`. Stage 2
names the web's exactly: `timer-glue.js`'s rAF, `gpu-glue.js`'s own loop
(`host/web/gpu-glue.js:350`), and `rt.js`'s `paint()` loop, which today runs
only for frame tasks (`rt.js:292`).

**The period and where it comes from** (`period{ms, source}`):

| host | `source` | how |
|---|---|---|
| Apple | `target` | `targetTimestamp − timestamp`, as the canvas code already does because `duration` is wrong under ProMotion (`Session.swift:1345-1349`) |
| Linux | `refresh` | the mode's refresh rate. `missed` is the flip-sequence gap (`display.rs:241`), not a division |
| web | `floor` | the smallest interval sustained across 8 consecutive samples in the current segment, never raised by a slower phase inside the segment. A display change (`matchMedia('(resolution …)')`, `screen.onchange`) re-estimates it. Not `pace.js`'s fitted `period_ms`: that starts at zero and adopts a sustained double-slot cadence as the new period (`pace.js:20`, `:79`), which would normalize sustained jank |

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
- **`seq`** is the range of kernel transactions whose batches this frame
  applied, or null when it applied none. This needs the batch trailer to
  carry the transaction sequence range on the web and Apple: today it carries
  the clock and no epoch (`host/web/src/batch.rs:744`,
  `host/apple/src/batch.rs:576`). The trailer gains `seq` (two numbers). Linux
  applies in-process and reads the runner directly.
- **`apply` and `layout`** are host-timed: the host applying batches, and
  kernel layout on Apple and Linux. **`paint`** is on Linux (`paint.rs:800`).
- **`loaf`**, web only, comes from Chrome's Long Animation Frames API. It
  exists only for a frame that produced a `long-animation-frame` entry, which
  are frames over 50 ms. Its fields:
  - `script` is the sum of `scripts[].duration`;
  - `styleLayout` is `startTime + duration − styleAndLayoutStart`, the
    reported style-and-layout tail, present only when `styleAndLayoutStart`
    is non-zero.

  The observer delivers late. An entry is joined to the sample whose `t` lies
  in `[startTime, startTime + duration]` and updates that record once. An
  entry that matches no retained sample is dropped and counted in
  `loafUnmatched`. Shorter late frames carry no `loaf`, and no value is
  invented for them.

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

**The join is by `seq`, not by adjacency in the journal.**
- Commit lines do not cover every commit: a frame task's commits are not
  journaled individually (`commit.rs:173` journals only failures).
- The JS target journals no successful commit (`rt.js:178`).

So the driver joins a late frame to the actions behind it this way:
1. The late record's `seq` range is the authority.
2. The journal lines inside that range name the actions, where there are lines.
3. Commits in range with no line are reported as `n unjournaled (frame tasks
   or JS-target commits)`, never omitted.

This is juxtaposition with stated coverage, not a causal trace (§7).

**The JS target's journal** is an unbounded array (`rt.js:127`). Stage 1
bounds it to `JOURNAL_RING` with the same `from`/`dropped` cursor, because a
human session (D5) would otherwise grow it without limit.

### D5 — A person's session, handed to the agent as a file

James's case is a person using the app normally. That run is not in agent
mode: no carrier is open and the clock is the wall's. LLP 1012.000's attach
is not landed. The hand-off is a file.

- **Development builds only.** Collection is off in production trust (D1). The
  export is removed by the same gate that drops `EXACT_AGENT*` and ships
  `AGENT_ADMITTED=false` (LLP 1069.007 §4).
- **Writing it.**
  - Apple: the dev menu gains **Save Trace** (the iOS sheet; macOS Develop
    menu ⌥⌘T). It writes `trace-<wallclock>.json` to the app's temporary
    directory. On a phone the driver copies it back with the app-container
    file service the iOS carrier uses for screenshots (LLP 1012, physical
    carrier).
  - Web: `dev.mjs` serves `POST /__exact/trace`. The page posts on ⌥⇧T, and the
    file lands under the app's `target/`.
  - Linux: `SIGUSR1` writes it beside the binary.
- **The file holds:**
  - **`identity`**, in the shape `stress-metrics.mjs` already writes (commit,
    dirty tree, platform, arch, CPU, browser or OS version), plus host, app,
    build trust, incarnation, boot wall time and the clock origin;
  - **`plan`**, the digest, and **`map`**, the source map itself, copied in at
    export. A digest alone cannot locate a map (`agent-inspect.mjs:9`), and a
    trace read a week later must not depend on a build directory;
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

1. **Web, work only.**
   - Runner: site totals, the `seq` tag, the reply.
   - `rt.js`: the same counters, with per-commit stamps.
   - The driver's `perf` and `during`, and the source-map join.
   - The JS journal bounded.
   - The production-trust switch.
   - `DEFERRED.md` amended.
   - Measure D1's cost.
   - Exit: on `apps/caltrain`, `perf` shows a known needless evaluation, and
     the fix is shown by D6's diff.
2. **Web frames.**
   - Segments, the `floor` period, the trailer's `seq`, LoAF, `perf frames`.
   - The late line in the journal.
   - `POST /__exact/trace` and `agent.mjs trace`.
3. **Apple.** `moved`, the trailer's `seq`, the `target` period, Save Trace.
   Delete `EXACT_FPS` and `EXACT_TIMER_TRACE`.
4. **Linux.** `moved`, the flip-sequence `missed`, `SIGUSR1`.

Stages 2–4 depend only on stage 1.

## Rulings (Charlie, 2026-10-03)

Charlie leaned `perf` on Q1 and asked for a recommendation on Q2–Q4; he
accepted the four recommendations below ("sure").

- **Q1 — The name: `perf`.** It covers frames as well as work, and it is
  DEFERRED's own word for this.
- **Q2 — The trust switch: a private ABI flag.** The host tells the runner at
  boot. It is never a reserved field the app can read: an app has no business
  branching on whether it is measured (D1).
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
