# LLP 1041: Graceful overload, proved by interactive workloads

**Type:** RFC
**Status:** Draft design; three stress examples and native resize diagnostics implemented. Host evidence and the first runner optimization are recorded below. This does not claim a new scheduler, worker placement, virtualization, or 120 Hz support has shipped.
**Systems:** Data execution, runner settlement, host completion pumps, presentation, workload diagnostics
**Author:** Tuft / Codex for Charlie Cheever
**Implementer:** Tuft / Codex with Astra workers; first examples start 2026-09-16. Runtime changes follow measured examples and the accepted worker-placement design.
**Date:** 2026-09-16
**Related:** [LLP 1027.002](1027.002-optional-worker-execution.rfc.md) (accepted module placement and Store ordering); [LLP 1016](1016-async-data-settlement.rfc.md) (async settlement); [LLP 1010](1010-scrolling-v1.spec.md) (scrolling and windowing); [LLP 1029.000](1029.000-rust-development-reload.rfc.md) (replacement); [RULES](../rules/RULES.md)

## 1. Outcome and scope

Charlie asked on 2026-09-16 for an LLP about a system that handles extreme
workloads gracefully, and to start the Messages and completion-storm examples.
The outcome is **bounded resource use and usable interaction under overload**,
with correct state and explicit refusal when a promise cannot be met. It is
not a promise to compute, retain, and draw arbitrarily much work in fixed time.

At a target refresh rate H, one frame is 1000/H milliseconds (8.33 at 120 Hz).
This is the entire frame interval, not a budget the runner may consume by
itself. A browser scrolling an old layer smoothly does not establish that
Contract state, layout, and fresh application content update at that rate.

The first delivery is two real Contract apps and reproducible correctness
drives on web, macOS and Linux, using existing hosts and diagnostics. Charlie
explicitly required native validation on 2026-09-16; browser results cannot stand
in for either native host. Synthetic data never touches a
user's Messages database, accounts, or production services. Large workloads
are opt-in, not new blocking CI gates. This RFC defines the direction; it does
not silently widen LLP 1027.002 to a generic pool or add an independent task
graph to the runner. Pools, parallel layout, and per-source scheduling remain
unimplemented candidates, earned by the measurements below.

## 2. Existing boundaries and where pressure enters

- `host/apple/src/executor.rs` runs requests and native continuations on one
  FIFO worker. This avoids renderer I/O, but unrelated work queues behind a
  slow request. The accepted module-worker RFC addresses computation ownership;
  moving work alone neither makes it faster nor resolves shared Store ordering.
- `Executor::drain` collects every ready outcome. `Host::fulfill_all` settles
  each on the runner before producing a host batch. One host batch does not
  mean one settlement, and a large ready queue can occupy one UI turn.
- `runner/src/runner/settlement.rs` stages resource effects and checks a
  fixpoint before publication. Transaction failure and declaration-dependent
  ordering cannot be replaced with arbitrary completion order for throughput.
- `host/apple/src/host.rs` computes root layout and walks the tree to emit
  changed frames. Taffy's cache is useful; emitting only changed frames is not
  proof that inspection work is proportional to the changed subtree.
- Existing `scripts/metrics.mjs` measures startup, first interaction, scaling,
  and list memory. Extend that diagnostic family as needed; do not create a
  second performance registry or add a sixth blocking gate.

These are inspected mechanisms, not measured bottleneck rankings.

## 3. Proposed runtime contract

### D1 — Ownership and dependency order precede parallelism

Keep one executor owner per live stateful module. VM objects, native pointers,
database handles, and module globals remain there. Arguments and outcomes cross
as values. The runner alone commits Contract/Store state; native UI remains on
its platform thread. Use LLP 1027.002's post-commit dispatch and committed Store
snapshot, admitted scope, read observations, and ordered writes.

Shared `secret.keep` names impose the ordering defined by that RFC. A second
thread cannot remove that dependency. A pure job pool, if later admitted, takes
owned inputs and produces owned outputs without capturing mutable module state.
The runtime must know which operations commute; source names, equal arguments,
and the absence of Rust data races do not establish semantic independence.

### D2 — Bound every queue by count and bytes

Admission counts queued, running, and completed-but-unconsumed work, including
payloads and retained snapshots. Account for the whole session across owners,
not just each worker separately. Bound decoded images and cached layout/data
separately from logical record count. A 100,000-record history does not require
100,000 materialized views.

Every bound has an overload policy. Replaceable previews may supersede queued
older previews; display notifications may coalesce. An admitted user mutation
must produce a terminal outcome or remain visibly pending. Reject new durable
work before executing an effect when capacity is unavailable, or use an
explicit durable queue owned by the application. Never silently drop writes,
truncate a supposedly complete result, or retry a non-idempotent mutation.

Initial queue/byte/turn limits are implementation choices to measure, not
universal constants invented here. Report the selected limits with each run.

### D3 — Fairness and yielding are explicit

Separate interaction-sensitive work from background throughput, while giving
background work a bounded opportunity to progress. Priority cannot skip a
dependency or reorder conflicting mutations. Do not sort an unbounded ready
list on every frame. Demonstrate progress under a continuous foreground load.

Drain a bounded amount of completion work per UI turn and schedule another wake
if work remains. Bound both result count and observed turn cost. Commit each
transaction atomically; yield between transactions or at an explicitly safe
continuation boundary. A time check after one long operation returns is only
diagnostic: it cannot undo a missed frame. Such operations must move off-thread,
become incremental, or refuse oversized input. A chunk must itself be bounded.

Do not add a frame of input lag merely to improve a throughput chart. Presentation
may retain the previous good view while background results are unavailable.

### D4 — Cancellation and retirement protect identity

Identify work by session, module incarnation, logical call, and runner ticket;
component-owned work also carries its owner lifetime. Superseding input, unmount,
or replacement invalidates interest before later replies can mutate live state.
Duplicate and stale completions must be harmless, including malformed replies.

Interest cancellation is not worker preemption and does not undo a remote write.
Cooperative work checks cancellation at safe boundaries. Native work that cannot
stop must have bounded retained ownership; repeated replacement cannot create
unlimited hung workers. Where an in-process call cannot safely be interrupted,
stop admission or visibly refuse replacement. No unsafe thread termination or
promise of crash isolation. Reuse LLP 1029.000's lifetime rules.

### D5 — Degrade optional work, preserve meaning

Under overload, defer offscreen work, coalesce intermediate visual states,
lower optional image/detail budgets, and keep the last complete result. Show
pending/stale/refused state where it affects a user's decision. Maintain focus,
text edits, selection, and scroll anchoring. Stop background production when
the consumer cannot accept more rather than growing memory without limit.

Differentiate logical history, materialized rows, decoded images, and delivered
updates in the examples. An explicit bounded-page baseline must say it is a
page; it cannot be reported as successful automatic virtualization.

## 4. Two first workloads

### Messages stress

`apps/messages-stress` is a deterministic synthetic conversation. Controls vary
history cardinality and update pressure while a user scrolls and edits a composer.
Mixed text lengths produce varied row heights; stable keys let us observe whether
updating a message recreates unrelated views. The intended larger workload adds
image decode/resize pressure, multiple live streams, and history prepend.

Start at a small functional case; increase through 1,000, 10,000 and 100,000
logical records. Report separately the number actually materialized. An eager
mode, when available, is opt-in and may fail visibly at large sizes; a bounded
baseline permits testing interaction without first constructing every record.
Do not label a generated history alone a sustained update workload.

Correctness: stable identity, deterministic data, finite input limits, composer
edits preserved during updates, pending versus completed updates distinguishable,
and no production data. Later windowing must preserve anchors and keyboard focus.

### Completion storm

`apps/completion-storm` sends independent async requests to a bounded local
fixture. Hold a cohort, then release it together; vary fan-out and controlled
failures. Type while completions arrive. Navigate away and start a new generation
before releasing old work. This must use real request/fulfillment/settlement paths,
not a loop that calls a synchronous fake and claims worker throughput.

Report requested, admitted, held, released, completed, and failed distinctly
where observed. A release button cannot claim that all work was concurrently
admitted; the native serial executor may admit only one at a time. Separate the
server's release barrier from client arrival rate. Use explicit controls and
bounded storage/timeouts; disconnected requests must not remain held forever.

Correctness: malformed/errors settle visibly, stale results do not overwrite a
new screen, duplicate completions have no extra effect, counts reconcile, and
all held requests reach release, disconnect, or expiry. No uncontrolled network.

## 5. Evidence and measurement

Functional drives use the existing agent operations and deterministic clock.
Performance runs use real elapsed time; advancing a virtual clock proves no FPS.
Use optimized builds on named hardware with a recorded source revision/dirty
state, host/browser version, workload controls, run length and sample counts.

Record idle baseline and repeated load runs. Capture p50/p95/p99/max for frame
callback gaps and input-to-visible-update proxies, plus raw samples or histograms,
queue age, execution/transfer/settlement costs where instrumented, live view count,
and peak/after-idle memory. DOM mutation, rAF, compositor presentation, process
RSS, and JS heap are different observations: name exactly what was measured.
Long tasks at 50 ms cannot establish an 8.33 ms budget. A 60 Hz/headless browser
cannot certify a 120 Hz physical display. Target rate is explicit; observed
callback cadence is evidence with caveats, not automatic display detection.

Run foreground/background competition, finite bursts, sustained production,
navigation/reload during work, cancellation, and recovery after load stops.
The overload result includes throughput sacrificed and any refused work, not
just a smooth animation. Pass correctness first. Responsiveness improvements
must not mask starvation, retained memory, or uncompleted durable actions.

The first sampler is `bun scripts/metrics.mjs --stress-url
http://127.0.0.1:PORT --seconds 10 --target-hz 120`. It requires an already-built
local demo and `CHROME`, then types into `stress-input` and watches the bound
`stress-echo`. Repeat `--tap <testId>` to select workload controls before typing;
`--cpu-throttle 4` is a browser CPU slowdown, not a physical low-end device.
It reports raw bounded rAF/echo samples and percentile summaries, DOM count,
optional JS heap, source checkout state, and errors as JSON. Unlike the normal
metrics build mode it samples a live URL: the checkout identity is not an
attestation of the artifacts that server serves. Queue spans, native frame
presentation, process memory, and pointer-to-display latency remain unmeasured.

## 6. Sequence and return triggers

1. Ship usable first slices of both examples and run their correctness tests.
   Drive actual AppKit and an actual Linux OS as well as the browser. Record
   missing scenarios openly; no performance claim from scaffolding or from
   compiling the Linux host on a Mac. Headless CPU rendering, GPU/display
   presentation, native state acknowledgment and OS input delivery are distinct.
2. Complete accepted LLP 1027.002 placement independently, and compare these
   workloads where relevant without changing their promised outputs.
3. Use measured completion bursts to implement bounded pumping; use sustained
   overload to select admission bounds/fairness. Prove recovery and progress.
4. Use Messages cardinality and layout traces to guide LLP 1010 windowing and
   image budgets. A large list is not a reason to parallelize every tree walk.
5. Admit additional owners/pools only after disjoint work is demonstrably waiting
   or a measured parallel throughput gain improves interaction at acceptable cost.

The immediate owner/date covers the examples and this design, not an unbounded
promise to implement all candidates in this turn. Update this document with
actual evidence and implementation status as each slice lands.

## 7. First-slice evidence, 2026-09-16

Messages uses existing root resources/tasks, stable keyed rows, six text-length
patterns, a bounded composer and local echo. It supports explicit 100-row pages
or eager 100/1,000/10,000/100,000 rows, and 1/8/32 tail rows revised at a requested
4 ticks/s for at most 120 ticks. Automatic windowing, images, history prepend,
multiple independent streams, and physical-display performance runs remain
follow-ups. Both examples have web, Apple and Linux adapters. Completion Storm
uses 128 explicit root resources.
Navigation explicitly disables their arguments as well as removing views: it
does not rely on child-owned effects absent from this main revision.

Exploratory optimized web runs on Apple M4, HeadlessChrome 153.0.8010.12,
1x CPU, 1200×850, base `5bbb307` plus this working change:

| Messages workload | Run | Input samples | Echo DOM p50 / p95 / max |
| --- | --- | --- | --- |
| 100 rows, paused | 5 s | 20 | 2.9 / 3.5 / 3.6 ms |
| 1,000 eager rows, 32 revised per requested tick | 5 s | 19 | 11.9 / 18.7 / 18.7 ms |

All sampled inputs matched their Contract echo, with zero reported page errors.
The loaded run reached revision 21 and 5,059 DOM elements (idle: 567). Median rAF
gaps were approximately 50 ms in **both** runs, making this environment unsuitable
for a claim of physical 120 Hz. These are single short diagnostic samples, not
statistically established regressions or an isolated CPU attribution. The loaded
DOM path exceeding 8.33 ms is already a useful reproduction to investigate.

Native Messages drives also passed exact typing echo, revision checks, scrolling,
local echo and reset on actual AppKit and actual Ubuntu Linux. Optimized builds,
980×820 requested viewport, 12 samples per profile, same working source:

| Native surface | Idle 100 rows: input ack p50 / p95 | Eager 1,000 rows: input ack p50 / p95 |
| --- | --- | --- |
| macOS, Apple M4, Darwin 25.2.0 | 4.3 / 111.8 ms | 43.3 / 59.0 ms |
| Ubuntu 24.04 ARM64 VM, Linux 6.8.0-134, CPU raster | 6.7 / 7.7 ms | 117.7 / 122.9 ms |

Linux ran in Lima/VZ on that M4 with 4 vCPUs and 4 GiB RAM, not as a Linux
renderer binary running on macOS. Its headless agent paints dirty frames before
acknowledgment; no desktop compositor or physical display participates. Each
loaded sample queues a 250 ms **virtual-clock advance** (32 tail rows updated)
then types immediately, so the timing includes queued synchronous work and IPC.
It is not a wall-clock stream or OS-input-to-presentation timing. The macOS
idle outlier and these small sample counts preclude a stable comparative benchmark.
Echo tree inspection adds additional cost; runner node counts are not native
live-view counts. Reproduce with each app's documented `native-smoke.mjs` driver.

Completion Storm passed 128-lane drives on web, AppKit and the Ubuntu VM:
128 fresh successes after navigation/remount, 128 stale replies discarded,
and a deterministic mixed wave with 64 valid and 64 failed/invalid results.
Typing echo remained correct during held and released phases. Native fixture
releases saw only **one held HTTP response**, reflecting the existing serial
request worker; 128 logical lanes do not mean 128 simultaneous native sockets.
The native driver releases from another process because an in-app release
request otherwise queues behind held data. This exposes head-of-line blocking,
not a demonstrated 128-way simultaneous native completion burst. The next
scheduler experiment must preserve this baseline and measure admitted versus
actually concurrent work separately.

Focused validation: 14 Rust tests, 9 JavaScript sampler/fixture tests, scoped
Clippy and formatting, optimized web/Apple/Linux builds, caps and boot passed.
The existing whole-workspace build/test currently fails on this machine because
the lean Hermes producer needed by unrelated TypeScript apps is not provisioned.
Whole-workspace formatting also reports changes in the sibling Snapback checkout;
those sources were not reformatted. Focused example checks and caps/boot are
recorded separately; no blanket workspace-green claim is made.

## 8. Implementation campaign: three workloads and live resize

Charlie authorized this campaign on 2026-09-16 and requested Astra xhigh.
Tuft owns integration; Astra xhigh workers implement disjoint changes. Add a
third, gigantic Markdown consumer using the existing parser/presenter, rather
than a substitute renderer. Evolve scheduling and variable-height viewport
collections together. The existing eager cases remain available as controls.

The target is responsive visible interaction at a 120 Hz frame interval under
explicitly recorded workload profiles on macOS and actual Linux, with web
measured separately. Background completion need not occur within one frame;
correctness, bounded admission/retention, ordered effects, and eventual progress
are mandatory. No universal workload or physical-display result is implied.

Initial work streams:

1. Gigantic Markdown: deterministic large documents, long paragraphs, fenced
   code, tables and many blocks; exercise opening, scrolling, typing, heading
   navigation and width changes against the existing Markdown semantics.
2. Native scheduling: remove independent HTTP head-of-line blocking only with
   explicit bounded admission and without reordering storage or module effects.
   Then bound completion pumping and retain one committed UI state owner.
3. Viewport work: variable-height rows/blocks, stable identity and scroll
   anchors, bounded visible/overscan work, width-sensitive measurement invalidation.
   Preserve document selection and search semantics; paging is not virtualization.
4. Measurement: separate queue wait, computation, settlement, layout/paint and
   presentation where instrumented. Exercise continuous actual window resizing,
   not only fixed-size launches. Record repeated idle/load runs, raw samples,
   percentiles, workload sizes, memory/queue bounds and recovery after load stops.

Use 8.33 ms as the target frame interval, not an allowance for every individual
stage. Report misses and the worst indivisible operation. Input acknowledgment,
CPU frame construction, GPU completion and display presentation remain separate
metrics. macOS display capability and a graphical Linux 120 Hz setup must be
verified before claiming physical 120 fps. The current Linux VM can establish
Linux correctness and CPU-path costs, not compositor/display performance.

Each implementation increment must retain the pathological controls, document
any quality/throughput sacrifice, pass relevant correctness checks, and include
before/after evidence. A missing display does not prevent software progress;
it does prevent calling physical presentation verified. Remaining bottlenecks
and the next discriminating experiment must be recorded explicitly.

### 8.1 First runner experiment: skip unchanged keyed content

On 2026-09-16, the instance evaluator gained conservative dependency memos for
outer keyed regions. Unrelated global changes skip region keying and bindings;
a changed list still validates every key but unchanged row values can retain
their subtree when its referenced globals did not change. Dynamic row-owned
state and contextual expressions fall back to the existing path. Resource
settlement, action execution, atomic kernel application and event identity are
unchanged. Deterministic `last_instance_work()` counters distinguish visited
nodes, evaluated row keys, retained row subtrees and skipped regions.

The first release-build experiment uses the opt-in test
`cargo test --release -p messages-stress-data --test interaction -- --ignored --nocapture`.
It exercises the actual Messages Contract and runner with a monospace measurer:
**no host layout, paint, native input or physical presentation is timed**. On the
Apple M4, three alternating before/after runs of 40 inputs per phase compared
commit `024ace7` with the working dependency-memo change:

| Eager rows / operation | Baseline p95 range | With memo p95 range |
| --- | --- | --- |
| 1,000 / typing | 2.17–4.99 ms | 0.053–0.135 ms |
| 1,000 / revise 32 tail rows | 2.43–5.77 ms | 0.746–1.77 ms |
| 10,000 / typing | 22.96–25.61 ms | 0.326–0.486 ms |
| 10,000 / revise 32 tail rows | 40.48–73.11 ms | 7.16–18.70 ms |

Raw samples and executable SHA-256 identities are in the local diagnostic
`/tmp/exact2-instance-memo-comparison-final.json`. Concurrent builds mean these are
exploratory measurements, not a controlled machine-wide performance guarantee.
The deterministic work assertions establish that typing keys zero rows rather
than 1,000, while changed global bindings, clock/pending values, row-owned state,
and inactive branch dependencies remain live. All 159 compiler/runner tests passed before the final signed-zero case; the
seven targeted memo tests also pass. Review caught that ordinary numeric
equality erases signed-zero changes; cache equivalence now preserves numeric
bits recursively. This final run still exceeds 8.33 ms for some streaming
samples; it is not evidence that the complete frame budget has been met. This is not virtualization: retained
views are still O(N), changed lists still key O(N) records, and native layout and
painting may dominate after this improvement. Those are the next measurements.

### 8.2 Markdown and native resize baselines

`apps/markdown-stress` uses the shipped Markdown parser, value conversion and
reader components. Its five deterministic fixtures range from 16 KiB to 4 MiB:
mixed prose, one huge paragraph, one huge code block, a table and many blocks.
Manual 40-block paging and eager mounting remain explicit controls. Paging does
not split a giant block or avoid parsing the full source. The native production
reader already parses files in a continuation; this synthetic baseline exposes
synchronous parsing separately. The app README records exact workloads, host
drives and raw-sample locations.

The initial M4 parser diagnostic measured a 4 MiB paragraph at 21.27–24.23 s
over three samples, versus a 1.15 ms median for a 4 MiB fenced code block.
Repeated suffix validation during bare-URL probing is the next parser experiment.
Moving this work off-thread would not itself remove that throughput cost.

`scripts/native-resize-metrics.mjs` queues bounded cohorts of resize, typing and
wheel input, optionally preceded by a workload clock advance, for all three apps.
It verifies geometry, exact echo, scroll movement and completion recovery;
records raw command-send-to-ack samples, binary SHA-256 and hardware identity;
and refuses invalid dimensions without changing geometry. The existing `tap`
operation carries the resize input, keeping eight agent operations.

AppKit changes the actual window content size through `NSWindow.setContentSize`.
Linux changes the presenter viewport and constructs a headless frame. Neither
is an OS window-border drag or a physical presentation measurement. Cohorts are
backpressured, not a claimed fixed-rate input stream. The M4's attached JetKVM v1
display is configured at 1920×1080 @ 60 Hz, independently observed through
`system_profiler`; physical 120 Hz cannot be certified on this display.

Native resize baselines include an older captured Rust archive on macOS while
the Swift diagnostic was being added. Source checkout identity alone does not
identify all build inputs. These reports expose resize cost but are not a
before/after measurement of §8.1; a complete rebuild is required for that comparison.
