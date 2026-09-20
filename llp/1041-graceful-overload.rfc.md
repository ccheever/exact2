# LLP 1041: Graceful overload, proved by interactive workloads

**Type:** RFC
**Status:** Campaign in progress. Three stress examples, bounded native HTTP/pump work, viewport collections and interruptible Messages gestures are implemented. Native image admission, the three continuous gallery gestures, measured worker placement and physical 120 Hz remain incomplete; evidence and limitations are recorded below.
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

### 8.3 Remove repeated Markdown suffix validation

The next parser experiment confirmed the §8.2 candidate. Keeping the original
validated string and slicing it at the existing character-boundary cursor
avoids revalidating the remaining source at every possible bare-URL prefix.
Three paired runs of frozen before/after diagnostic binaries on the M4 reduced
the same 4,194,181-byte paragraph's median parse time from **20,991.428 ms to
30.458 ms**. The 1 MiB paragraph fell from 1,339.868 ms to 7.351 ms. Controls,
raw samples and build identities are in the Markdown stress README.

All twenty profile/size combinations produced exactly equal titles and block
values against the old parser, including text, styles, cells and links. Sixteen
parser tests and the opt-in 4 MiB integrity test pass. The change uses checked
string slicing, keeps the complete input, and does not alter worker placement.
Other potentially expensive Markdown algorithms are unchanged. Thirty
milliseconds still exceeds the entire 8.33 ms target interval before layout;
this throughput improvement does not establish smooth giant-block rendering.

### 8.4 Bounded native HTTP admission and completion pumping

Native requests now stay ordered by default. A source can explicitly promise
that an HTTP operation **and its settlement** may overlap/reorder, with a declared
response ceiling. Storage and native continuations cannot opt in. Two independent
HTTP owners use separate transports; the original ordered owner retains control
capacity. Completion Storm opts its held data requests in and keeps release
controls ordered. No independence is inferred from GET or matching origins.

Independent admission counts queued, running and undrained outcomes against
128 requests / 32 MiB. The ordered lane has 16 requests / 512 MiB; reserving its
64 MiB response ceiling conservatively normally admits three calls. Request
buffers are capped at 4 MiB; response limits apply during HTTP reads. Reservations
last until consumption. One completion or admission refusal settles per pump,
with alternating opportunities for ready lanes/refusals and coalesced wakes.
Ordered refusals wait for prior admitted work and prevent later ordered effects
from bypassing their settlement. Refusals occupy existing current runner tickets,
not a new unbounded failure queue.

Retirement clears interest, aborts HTTP and drops queued work on its executor
owner. It never joins arbitrary native closures on the UI thread. Each native
host implementation limits live and retiring executor workers to 48 until exit.
Opaque Rust closure captures and transient allocations remain **count-bounded
only**, outside the transport byte reservation. D2 is not fully satisfied for
arbitrary native work; this is no absolute heap bound. Nor does one transaction
per pump bound its duration or prove that AppKit's run loop presents between
consecutive main-queue blocks. Those are remaining measurement/scheduling work.

The macOS and actual Ubuntu Linux 128-lane drives both observed two held data
responses while an **in-app** release control succeeded; the old baseline needed
an external releaser. Each drive discarded 128 stale replies, accepted 128 fresh
ones, reconciled 64 valid / 64 failed mixed outcomes, preserved exact input echo
and reported no host exceptions. The rebuilt web drive passed the same outcome
checks; browser connection admission remains browser-owned.

The final Linux build used frozen Git tree
`8c7320d3f1146a756b175da9cfe503750fda650c`, excluding concurrent collection/painter
edits. Its executable SHA-256 is
`ca38139adab8a7f30929fb9add7ff0962235abd1ba50dd078d345b1fd019bb16`;
reports are in `target/scheduler-final-linux/storm-smoke/` and
`target/scheduler-final-web/`. The macOS report is
`/tmp/exact2-http-macos-review/macos-check.json`. The focused suite passed 134
Rust tests, including admission retention, illegal annotations, ordered refusal
recovery, response limits, retirement and stale-cohort recovery; scoped Clippy,
formatting, caps and boot also passed. These are correctness and bounded-work
results, not a 120 Hz frame-budget pass. Linux headless agent pumping occurs
between commands and does not exercise a desktop event loop or compositor.

Final review also found that a direct, unwrapped Rust web source could attach
independent HTTP metadata to storage/continuations. Web validation now precedes
token conversion and effect serialization, and defers a terminal Refused outcome
until the enclosing batch is applied. Five web request-path Rust tests and three
JavaScript response/refusal tests pass, alongside scoped Clippy and formatting.
The rebuilt 128-lane browser drive passes again in
`target/scheduler-final-web-refusal/`; the native executor sources are unchanged.

### 8.5 Continuously interactive collections: expanded campaign

Charlie authorized all four demonstrations on 2026-09-16, including additional
Astra xhigh agents, independent sessions and machines where useful. Tuft owns
implementation and integration, starting now. This expands the active campaign;
it does not replace Messages, Completion Storm, giant Markdown or their eager
controls. First finish the common viewport and scheduler foundations, then use
these interactions to drive the next measured changes. Shared primitives belong
in Exact2; example-specific content and visual design belong in the consumers.

1. **Live Messages.** Swipe a variable-height message toward reply, release its
   spring, grab it while moving, reverse and cancel. Repeat during streaming
   updates, history prepend, delayed attachment sizing and continuous resize.
   Keep the manipulated logical message and reading anchor stable even as rows
   mount/unmount. Finish the interaction once, including when its record is
   deleted or navigation removes the collection.
2. **Photo collection and interactive zoom.** Expand a thumbnail, immediately
   drag the enlarged image, cancel or dismiss, and switch images before returning.
   Reflow the grid and load distinct image assets during the interaction. Resolve
   return geometry by stable item identity, including an unmounted, moved or
   deleted source. Define those outcomes explicitly. Any temporary visual retained
   above the collection has bounded lifetime and byte accounting; no indefinitely
   pinned source view or decoded-image cache. LLP 1013 is a starting design, not
   evidence that snapshot transitions already provide interactive takeover.
3. **Reorder with edge scrolling.** Lift a card, move through a large virtualized
   collection while nearby cards spring aside, auto-scroll in both directions,
   then drop or cancel. Exercise concurrent insert/delete/reorder and resize.
   The dragged identity stays attached to the pointer; preview geometry cannot
   silently mutate durable ordering. A committed drop applies once to the current
   data or is explicitly cancelled when its identity no longer exists.
4. **Sheet and nested collection.** Drag among sheet heights, scroll the list
   inside, reverse at its boundary and catch an unfinished settling spring.
   Preserve position and appropriate velocity through each transfer of control.
   The sheet's changing scrollport drives virtualization without repeated full
   list construction. Test pointer/trackpad behavior on desktop and actual touch
   recognition where available; synthesized deltas do not prove touch arbitration.

For every scenario, test the ordinary interaction first, then combine it with
completion pressure, changed row geometry, navigation and window resize. Keep
input, committed application state and current presentation geometry distinct.
One owner commits application state; a small host presentation state can follow
input and animate independently. Stale worker results, measurements and animation
callbacks must not overwrite a newer interaction. Sample current presentation
when taking over a moving object, preserving position continuity and carrying
velocity into release where appropriate. Hit testing must follow what is drawn.

Acceptance includes interruption in both directions, cancellation, no duplicate
actions, correct focus/selection, stable anchors and bounded retained rows,
images and transition visuals after repeated traversals and navigation. Background
work must recover and make progress during sustained interaction. Respect reduced
motion and retain keyboard alternatives. Record visible placeholder/quality choices
and throughput costs under overload rather than concealing missing content.

Record input delivery, first matching presentation opportunity, frame intervals,
collection work, retained memory and background throughput separately. Compare
repeated normal and loaded runs with identical fixtures on named hardware; retain
raw results and videos for visual review. Compare appropriate platform reference
interactions on the same available hardware where possible. The aesthetic target
is excellent direct manipulation; neither a subjective superlative nor a headless
timing can establish superiority to every framework or physical 120 Hz. macOS and
actual Linux remain required, web is a measured target, and touch-only claims need
touch-device evidence. These demonstrations are not yet implemented or verified.

### 8.6 Shared viewport foundation and next motion slice

The first shared collection is implemented in LLP 1010 §6.5–6.6. Messages and
Markdown can supply their complete histories/documents while retaining only the
viewport, overscan and two globally bounded pins. Eager/manual controls remain.
Compiler/runtime shape checks, zero-height runs, typography invalidation,
stale geometry, pin transfer and native rounding have explicit regressions.
This does not bound O(N) input generation, changed-list key validation or giant
indivisible text layout; image admission and unmounted selection remain open.

Eighteen paired fresh-process M4 measurements cover 25/1,000/25,000 records and
twenty complete traversals. At 25,000, synchronous input-path p50/p95 falls from
29.1057/30.9080 ms eager to 0.2462/0.2543 ms virtualized; requested retained heap
falls from 105.944 to 7.429 MiB. Live kernel and arena bounds match at 1,000 and
25,000. LLP 1010 records exact identities, raw artifacts and accounting limits.
These diagnostics contain no presenter and make no frame-rate claim. Separate
web/AppKit/Ubuntu drives exercise the real collection adapters and rich text;
their app READMEs distinguish endpoint, anchor and functional input checks from
physical presentation.

Interruptible presentation ownership is now implemented in LLP 1002 D3–D4 and
1003. The latest authored target survives while a token holds sampled
presentation; release uses the newest declaration and displayed velocity.
Generation/interaction checks reject stale callbacks before clock mutation.
Holding alone schedules no frames. Web retains CSS/WAAPI execution and samples
computed presentation at recognition; native hosts sample the shared engine.
Tuft integrates, Zeno owns the engine, Newton kernel regressions, and the
web/Apple/Linux owners their adapters, starting 2026-09-16. This completes a
shared primitive and the first Messages interaction slice, not all of §8.5.

### 8.7 Interrupted gestures: implementation evidence, 2026-09-17

Messages now supports catching a right-displaced returning row by moving
immediately left, reversing through its origin, and releasing or cancelling.
Inverse resistance preserves the caught position even beyond the resistance
threshold. Final input precedes the authored action; token release and collection
pin retirement follow it. Commits while held may change the target or transition
without moving the held row. Deletion, replacement, inactive routes, disabled
ancestors and carrier loss retire ownership without a late semantic action.
Native recognizer cancellation during a presentation batch is deferred beyond
that batch, with identity checked again on delivery. Overdue timer receipts
keep Runner order while presentation time remains monotonic.

Review exposed work that output-only tests had missed: Web regenerated the
entire keyframe array before checking whether a spring was already playing.
An allocation-free curve descriptor now precedes lowering. With 64 unrelated
springs and 100 hold moves, the instrumented regression falls from 6,528 curve
compilations to 64; subsequent releases correctly add one each. This is a work
count, not an elapsed-time or physical-frame result. Original failing logs and
passing browser checks remain under `/tmp/exact-web-motion-*`; shared checks
are in `target/motion-integration-final/`. The aggregate run passes 304 Rust
tests plus 13 kernel motion tests, with three opt-in browser tests separately
driven by the web owner. Scoped strict Clippy passes. Full workspace build/test/
lint still require the unrelated lean Hermes producer; external formatting
failures remain separately recorded rather than rewritten in this increment.
The final native Linux source independently passes 91 tests and strict all-target
Clippy on Ubuntu ARM64, including real TCP disconnect and evdev report grouping
that the macOS-hosted Linux package run cannot exercise.

AppKit drives use real native event dispatch over a 10,000-record virtualized
transcript; linked native probes cover batch reentry and loss of input eligibility.
Evidence and binary/source identities are in
`target/messages-stress-native/holds/`. UIKit source is migrated but has no
SDK/device validation on this machine; standalone Swift assertions are not
XCTest or a touch-device result.

The actual Ubuntu VM now also runs the normal DRM/KMS display loop using the
kernel's VKMS virtual 1024×768/60 Hz connector and CPU renderer. Existing VNC
input drives the 10,000-record transcript, typing while held, spring release,
immediate-left catch and reversal, without headless observation requests to
advance the host. Captured bubble edges are x=12 at rest/recognition, x=84 while
held and after typing, x=67 at catch, x=47 after reversal and x=12 after settling.
Disconnecting a held VNC contact also releases it. Exact captures, integer-pixel
checks, source identity and executable are in
`target/messages-stress-native/linux-holds-vkms/`. Its report distinguishes the
captured binary from the subsequent removal of the unused legacy hold API.
This is virtual display-loop and remote-input evidence, not a physical display,
desktop-compositor comparison, input-latency measurement or a 120 Hz claim.

An exploratory AppKit resize/typing/scroll drive of the preserved final Messages
binary supplies 10,000 records, revises 32 tail records per virtual producer tick
and resizes between 800×640 and 1120×860. All 72 cohorts pass echo, viewport,
scroll and recovery checks (three repetitions of 24). The three typing-ACK p95s
are 22.19 / 22.89 / 23.08 ms; resize-ACK p95s are 15.08 / 16.35 / 14.03 ms.
ACK includes IPC and earlier queued clock/resize work, not a stage's CPU cost
or physical presentation. The virtual producer advances once per backpressured
cohort; this is not a real-time 4 Hz throughput result. Raw samples and executable
identity are in `target/motion-integration-final/resize-10k-virtualized/`.
The earlier `resize-10k-windowed-final/` directory actually exercised the manual
100-row control, as its recorded mode says; its misleading directory name is
not evidence of virtualization. The preceding PATH-only startup failure is
retained separately. No compiler quiet-window claim follows from these runs.

**Normal-clock AppKit progress, Tuft and Newton, 2026-09-17:** a separate
optimized `7a6fde2` build adds only three ignored observer files. It boots with
`EXACT_AGENT=0`, no clock override, and the unchanged production 250-ms,
default-run-loop-mode timer. The observer starts the existing stdio carrier and
rejects clock/settle commands; it records bounded scalar timer/wake events.
With 10,000 supplied records, virtualization and batch32 selected, one
2,002.111-ms interval sends no app commands. Eight distinct timer/advance/apply
chains finish before the following state-handler begins, and committed history
revision advances from zero to eight. This establishes autonomous progress in
that interval, not sustained loaded throughput, tracking-mode progress or 120 Hz.

The executable SHA-256 is
`f7d304ec3f2446e2870e82bbc32c9d0cf9b3f1d43ea5792ffe94979617c4a40e`;
source, receipt, raw records and the independently checked replay are in
`target/normal-clock-carrier-native/messages-silent-02/` and its sibling build
capture. The first drive's oracle failure is preserved: equal JSON records had
different object-key order. A named regression and structural comparison fix
pass on the unchanged binary. This is one nonquiet instrumented correctness
sample, with no input/resize latency or physical-presentation measurement.

The matching normal-clock Storm build then completes two sequential 128-lane
waves. During a 2,002.050-ms interval with no app commands, an external release
of that run's wave produces 129 complete coalesced wake/pump/apply chains and
128 valid current outcomes before the next state handler begins. The second
wave receives 25 fixed-schedule offers: eight typing, eight responder-path wheel,
eight programmatic resize and one authored ordered release. All are sent, with
zero skips and a cap of four outstanding requests. Four novel widths precede
revisits to primed 800/960 widths. The final echo matches, actual scroll moves
0→120, and 130 coalesced pump chains occur during the offered-load interval.
These chain counts are not per-ticket ready counts. Both waves reconcile to
256 received/issued responses, with no held, rejected or abandoned response.

One nonquiet sample's transport/native ACK min/median/max is 0.504/7.634/50.592ms;
resize's eight-sample median is 26.108ms. These are not robust tail estimates or
8.33ms frame acceptance. The separate external Quartz window-edge input occurs
after HTTP recovery. AppKit records will-start, 11 `inLiveResize=true` changes
and did-end, but the observer's current-mode value is nil throughout. The
required tracking-mode witness is therefore **NOT ESTABLISHED**. No ordinary
timer callback appears inside that 1,274.146-ms span; callbacks resume afterward.
This observation does not establish loaded background progress during tracking
or identify a cause. No notifications or tracking loop were synthesized.
The raw input, unchanged observer, owned fixture lifecycle and exact captures
are under `target/normal-clock-carrier-native/`; the Storm executable SHA-256 is
`371a8455ac0b83f3a4b1a9cd807e72ab082430ba555a5f3aa260be664b086064`.

The matching 72-cohort drive also passes on actual Ubuntu ARM64 with final
binary `d5c89044…`; all 10,000 records are supplied there too. Typing-ACK p95s
are 17.80 / 15.92 / 15.92 ms and resize-ACK p95s 15.05 / 13.45 / 13.43 ms.
Raw samples/screenshots are in `target/motion-integration-final/resize-10k-linux/`.
This uses the headless CPU presenter, separately from the VKMS gesture drive;
the OS, fonts, painting and viewport differ from AppKit, so these are independent
workload observations rather than a platform speed comparison.

The native raster implementation and acceptance are recorded in LLP 1010 §6.3.
Actual Ubuntu ARM64 now completes twenty full 25,000-row traversals with typing
and resizing: 232,603 wheels, 3,634 exact Unicode input ACKs and 3,635 resize ACKs.
All twenty coverage bitmaps contain every logical row. Observed rows/images stay
at 6–12 and kernel/painted boxes at 121–200; managed raster peak is 6.01 MiB,
maximum sampled RSS and kernel VmHWM are 104.12 MiB. The source and executable
remain frozen through the run. Evidence and review limits are in
`target/native-raster-20260917/linux-accept25k-20/`. This CPU Presenter sweep
measures coverage and observed lifetime bounds, not frame cadence or a complete
native/GPU allocation ledger. Six images are reused; unique-source churn remains
a separate loader test. AppKit's full 25k sweep and Linux focus parity remain open.

The remaining campaign includes sustained loaded/resize interaction measurements,
the three gallery gestures and indivisible giant Markdown layout. The gallery's separate collection slice
supplies all 25,000 Arrange/Read records while preserving manual/eager controls;
metadata-only actions reuse row values and do no record keying. Photos remains
manually paged. Discrete buttons, static screenshots and bounded mounted rows
do not establish the continuous interactions or physical presentation rate.

### 8.8 Giant Apple paragraph: measured warm-path changes, 2026-09-17

Profiling the unchanged 4,194,181-byte Markdown fixture identified two warm
costs: canonical Unicode hashing during cache lookup/LRU updates, and computing
glyph ink bounds for all 48,932 lines before rejecting off-viewport lines. The
suspected repeated UTF16-length calculation was not a dominant cost in this
optimized fixture; no optimization based on that hypothesis was made.

Apple paragraphs now lazily retain a conservative vertical ink interval tree,
visited in logical paint order. Repeated dirty viewport paints select intersecting
lines before drawing; overlapping/zero-height lines still all contribute. The
first dirty paint constructs the index and retains that cost. Cache hits update
LRU through the found dictionary index, preserving checkpoint copy-on-write.
Run text uses matching exact UTF8 equality and hashing: canonical String equality
previously aliased NFC/NFD text whose CoreText UTF16 ranges differ. No pointer
identity, text truncation or alternate renderer is introduced.

Three fresh processes per size and stage compare baseline, paint-only,
dictionary-index and final changes on Apple M4/16 GiB/macOS 26.2. Timed measurement
includes the C request's UTF8-to-String conversion; paint targets a 640×820 bitmap.
The entire paragraph's contiguous UTF16 coverage is checked at widths 640 and 641.
Four-MiB medians, in milliseconds:

| Phase | Baseline | Final |
|---|---:|---:|
| Cold measurement at 640 | 293.078 | 252.571 |
| Warm measurement at 640 | 27.033 | 6.010 |
| First dirty viewport paint | 269.646 | 272.853 |
| Repeated dirty viewport paint | 266.886 | 0.200 |
| First measurement at 641 | 228.123 | 164.570 |
| Return to cached 640 | 26.990 | 6.231 |

The 4 MiB index adds 2,097,152 bytes of array payload per painted paragraph/width,
excluding headers and allocator slack; 256 KiB/1 MiB fixtures add 131,072/524,288
bytes. Unpainted width variants have no ink index. Existing paragraph/typesetter
caches still cap entries at 4,096, not bytes. Warm comparison and bridge ingress
remain O(source bytes); first paint and fresh-width reflow remain far beyond an
8.33 ms budget. Byte-bounded snapshots and asynchronous reflow are not implemented
by this increment.

Seventeen actual engine test methods pass 255 assertions, including exhaustive
bitmap comparisons, aligned overflow, backwards baselines, Unicode source ranges
and checkpoint eviction independence. The old paint cull and canonical-source
cache both have recorded failing regressions. This machine lacks XCTest, so
standalone optimized Swift runs the methods using temporary assertion functions;
it excludes session/view tests and is not a full-host or UIKit validation.

Artifacts, four preserved source/binary stages, raw samples and profiles are in
`target/apple-text-20260917/`; its `report.md` and `manifest.json` identify every
preserved artifact. Final Text.swift
SHA-256 is `1979567fce5dd591e51279e3e51f1dbef3f01c44fe5334bdc00772adf5f503c7`;
the timing executable is `1c2f17ccd75ad69eb7689a86fda6b95e8c7e5f63cb8b91c0ea180cdf6412e7a3`.
These are exploratory same-machine CPU observations without an exclusive quiet
window, Linux text result, whole-app input latency or physical presentation claim.

### 8.9 Actual Linux giant-text failures and residency work, 2026-09-17

Nine fresh-process probes use the preserved Linux rich-text executable
`a2f9a246e6ebff0ce8971a47428665e2a0b1e4a43540da1443656afc11372fc1`
on Ubuntu ARM64, four vCPUs/4 GiB, CPU presenter and DejaVu Sans. This is an
identified earlier binary, not the current image-adapter working tree. The
unchanged paragraph consumer supplies both document blocks and pauses production.
Three fixed-width edits/scrolls precede alternating window widths and eight new
widths, each queued immediately before a Unicode text edit. Timings include IPC,
Runner work, shaping and painting; they are not isolated engine or display times.

The 256 KiB arm completes all three runs: median warm typing ACKs are
47.22/47.69/49.67 ms; eight fresh widths finish at about 838 MiB RSS, versus
244 MiB after the alternating-width phase. All three 1 MiB arms abort on their
fourth fresh width under the diagnostic's 2.5 GiB virtual-address-space cap;
their previous acknowledged RSS is about 1,782 MiB. All three 4 MiB arms finish
the size-change command in 1.55–1.57 s, then abort during wide-column setup.
These are retained failures under an explicit experiment limit, not claims
about unrestricted OS OOM behavior. The cap protects the shared VM and is not
a product policy. Full samples, partial geometry, stderr, limits and provenance
qualifications are in `target/linux-giant-text-20260917/report.md`.

This exposes a prerequisite to asynchronous reflow: width-specific full buffers
must not accumulate behind an entry-only cache limit. Newton owns a synchronous
Linux residency increment, starting 2026-09-17 in an isolated worktree while the
native image integration finishes. Retain each live paragraph owner's accepted
snapshot and current working result; identical content may have several visible
owners at different widths. Retire unpinned obsolete widths before allocation,
retain scalar intrinsic metrics instead of full probe buffers, and replace
formatted full-text width keys with collision-checked content/style/catalog
identity. A proposed 64 MiB soft cold-cache target is an eviction policy, not a
process cap: pinned/working overage, known owned storage, estimated library
storage and RSS must remain distinct. Tests and repeated width sweeps must prove
bounded history and unchanged complete layout before any improvement is claimed.
Cold work remains synchronous in that increment. Moving it to a worker must
later preserve a responsive published shell and explicitly pending content,
coherent accepted geometry/text/selection, stale-result rejection and bounded
running/pending work; returning invented final metrics is not an implementation.

### 8.10 Reuse validated resources and cover growing web ports, 2026-09-17

Carson's Runner change removes repeated shape validation only when the existing
resource-reuse predicate succeeds. Each retained value has already crossed the
current immutable plan/index's validation boundary; fresh answers, baked values,
carry/kept inputs, asynchronous replies and forced refreshes still validate.
Seven regressions failed before the change and pass afterwards, with 87 Runner
tests and strict scoped Clippy passing. The unchanged 25,000-item fixture enters
shape validation zero times during 120 scalar actions, versus 120 before;
a forced fresh answer at the same pointer still enters it once.

Twelve alternating fresh processes compare preserved before/after executables
on the same Apple M4, with native builders/drives paused. Three repetitions per
arm use 1,000 or 25,000 gallery records and 120 authored height changes from
180 to 420 at fixed width. At 25,000 records, per-process action p50 falls from
438.542/442.333/465.125 to 45.750/42.084/39.750 microseconds; p95 falls from
459.958/476.000/513.833 to 49.209/50.625/44.625. All 1,440 samples pass and
deterministic outputs match: 2–5 mounted rows, 69–93 live nodes, 254 arena slots,
135 text measurements per process and no added source queries or key evaluation.
The 1,000-record arm has an after-run outlier (p95 120.666 microseconds versus
61.375 before); its other after p95s are 47.875/45.333 versus 70.250/74.167.
It is retained, not discarded or described as an across-the-board speedup.

The before executable SHA-256 is
`b0305f8403952c40adaed835435399cc9cae1d96f23d68632ebf20543eb7e25d`;
after is `0c9e7fa2ba10ca1b0ecfff8fb7db7679d1d34f453d864e9fed0b69b397811536`.
Source/binary identities match before and after the run; all 13 point-in-time
compiler observations are empty. Full raw samples and limits are under
`target/runner-shape-reuse/paired-2026-09-17T08-17-58.950293+00-00/`.
This probe uses deterministic monospace kernel measurement and ordinary authored
actions. It does not include native fonts, paint, IPC, held-height projection or
physical presentation; the tiny layout brackets do not prove a full frame budget.

Leibniz's browser fix lets actual scrollport ResizeObserver changes spend the
remaining collection-feedback budget before paint. It shares four reports with
the scheduled rAF, preserves the two-pass stimulus budget and leaves row-only
observation deferred. A growing port previously exposed an eight-pixel spacer
strip for one rendered frame. The real 25,000-record gallery regression pauses
a WAAPI height animation and grows its nested port from 105 to 256 pixels:
two 124-pixel rows become five before the first resized frame is painted.

The same test fails against the old JavaScript. In the fixed test, delaying the
actual observer by one rAF still paints the uncovered strip, while ordinary
delivery paints full coverage. Screencast PNGs must contain the marker for the
first rendering opportunity; an eventual screenshot cannot satisfy the test.
Both the 1,000-row Wasm fixture and gallery pass, as do 22 DOM tests. The lead
independently reran the gallery and DOM tests. Artifacts are preserved in
`target/web-resize-coverage-20260917/` and `target/web-resize-coverage-lead/`.
This combines current JavaScript with an identified prior Wasm/plan, colors
wrappers/spacers to isolate coverage, and makes no typography, physical gesture
or physical 120 Hz claim. Native image and text-residency work remains separate.

### 8.11 Native paragraph residency, 2026-09-17

Newton implements the Linux cache/painter ownership change; Zeno implements
Apple's cache and acceptance hook. Tuft integrates and measures both. Exact
UTF8, metric styles and font-catalog identity select paragraphs, with typed
width keys and weak lookup for retained owners. A new width retires obsolete
unpinned full paragraphs before shaping. Intrinsic min/max questions retain
scalar answers. Each actual accepted view/frame pins its own width: identical
content can be visible at multiple widths, and a failed paint cannot retire
the preceding accepted frame. Apple checkpoints preserve independent value
semantics; colored text retains its authored attributes and source ranges.

Both cold policies use a named 64 MiB **soft** target. Maintenance/acceptance
evicts cold ownership; last-caller drops and hits need not trim an oversized
measurement-to-paint handoff immediately. Overage is explicitly reported. Linux
reports accessible capacities, private text-length estimates and separately
deduplicated retiring accepted owners outside the current catalog. Apple reports
current cold logical payload and named CoreText estimates. Accepted owners,
checkpoints, opaque font/shaping storage and allocator/RSS costs are not silently
counted as cold or refunded on cache eviction. Neither policy is a total-memory
ceiling, a bound on arbitrary paragraph size, or a global two-width limit.

**Actual Linux:** fourteen residency and seven existing text tests pass with
strict all-targets Clippy on the frozen Ubuntu ARM64 source. Eighteen fresh
processes compare the raster foundation with the four Linux residency files,
using identical fonts, inputs and the §8.9 protocol. All 256 KiB cells finish;
last acknowledged RSS falls from about 838 MiB to 205 MiB. All three current
1 MiB baseline cells abort on the third new width under the 2.5 GiB address-space
limit, after two fresh-width ACKs. All three after cells finish eight widths at
762–769 MiB RSS. Exact document digests and all nine available common
phase-boundary node/viewport geometries match.

The trade is visible: 1 MiB alternating-width queued-input medians rise from
388–406 ms to 737–742 ms, and fresh-width medians from 583–603 ms over the two
acknowledged baseline widths to 729–732 ms over all eight after widths. This
does not establish the cause of the additional cost; repeated probe shaping,
synchronous destruction and capacity bookkeeping need discrimination. All six
4 MiB cells still abort during wide-column setup, before warm measurements.
Their last acknowledged RSS is about 1,239–1,241 MiB, not peak RSS at failure.
The address-space limit and preserved allocation failures remain part of the
result. This fixes retained width history, not synchronous giant reflow.

Source captures are `8d498d5546ce28a562dedfa46e83280a536a332d863bd76e6535bf469ae4f4d7`
and `13a1a542effe4b097d3c2faebd301d6defd4a79a68cf7b07095db564d0cee287`;
binary/provenance records are in `target/linux-giant-text-20260917/paired-build/`.
The eighteen raw runs and report are in `target/linux-text-residency-20260917/`.
The 36 guest boundary observations contain no competing compiler/gallery/
Markdown process. Host background processes are recorded separately.

A later diagnostic clone isolates two costs without replacing those paired
results. In the first fresh 1 MiB resize, both arms receive one giant paragraph
measurement offer at width 633. The baseline builds it once and paint reuses it;
the residency arm destroys the measured snapshot while still in layout, then
builds the identical paragraph/width again during paint. This cell demonstrates
measurement-to-paint eviction, not repeated giant width probes. The instrumented
extra build costs about 169 ms, versus 0.367 ms for both capacity walks and
19.335 ms for two field destructions. Inclusive trace stages overlap and are not
summed as independent wall time. In a settled warm repaint the updated arm
builds no paragraph but visits 978,600 glyphs in about 194 ms; Spec/key work is
about 0.231 ms. Both 4 MiB traces abort inside cosmic shaping/layout with one
accepted giant Buffer and one replacement being built. This identifies the
failing stage and overlap, not the private allocation responsible for the limit.

**Measured handoff correction, Newton/Tuft, 2026-09-17:** at most 64 exact
paragraph identities retain their latest definite-width result until the next
paint attempt finishes. Unrelated control measurement and cold-cache trimming
cannot evict that result. A replacement width retires the previous handoff
before allocation; an intrinsic miss releases its identity's handoff before
shaping scratch, while a scalar hit preserves it. Successful painting installs
generational accepted-owner leases; failed painting preserves the previous
accepted set. Both paths drain the transient handoffs. This is a count bound,
not a byte/process ceiling or a promise to retain every exploratory width.
Separate diagnostics report its cost without summing overlapping accepted and
cache categories. Twenty-two residency tests and seven existing text tests pass,
including the real Taffy giant-then-small-control regression and failure cleanup.

A new actual-Linux instrumented 1 MiB pair uses the same frozen baseline,
fonts and driver. Each arm offers width 633 once. The baseline builds twice
(measurement then paint); the correction builds once and paint reuses that exact
backing. Document hash and complete measured geometry match. The one-pair
first-fresh request is 550.956→364.133 ms, and the queued input acknowledgement
is 752.051→555.498 ms. This is nonquiet CPU/IPC evidence, not a repeated latency
benchmark or physical presentation result. `target/linux-text-trace/handoff-20260917/`
retains raw traces, source identities and binaries; the corrected executable
SHA-256 is `f4bb846c9f4ba5d13915d1182d67c1cacde5a3201ba8f18221319ed33b6746b9`.
That preserved trace still visits 978,600 glyphs per warm paint. The subsequent
CPU ink increment (Epicurus/Newton/Tuft, 2026-09-17) selects conservative visible
ink intervals in original line order, including overlapping/backwards baselines.
It retains the full shaped Buffer and leaves the GPU stream unchanged. Unknown
or ill-conditioned bounds fall back to exhaustive painting. One lazy index per
paragraph/catalog/scale has an 8 MiB array-capacity limit; old arrays drop before
replacement, and bounded envelope scratch does not retain extra glyph rasters.
First paint still builds the complete index. A 1,200-line pixel oracle visits under five
percent of glyphs at top/middle/end while matching exhaustive RGBA output.

Current ink array capacities enter catalog, measured-handoff, retiring-owner and
cold-policy diagnostics/maintenance in O(1) per paragraph. Categories overlap;
the soft cold target and the 64-identity handoff are not total-memory limits.
Three failing accounting regressions now prove shared-owner deduplication,
lazy cold growth followed by reclamation, and scale/refusal replacement without
history. The combined Linux package passes 173 tests with one existing GPU-only
test ignored, strict all-targets Clippy, scoped formatting, caps and boot on
macOS. This is not execution on Linux or physical presentation evidence.
Sources, pixel oracles, raw failures and integration logs are retained under
`target/linux-visible-text-validation/`. The 4 MiB accepted-plus-working peak,
shared shaping and asynchronous presentation remain unresolved.

The subsequent actual Ubuntu ARM64 comparison uses three fresh-process pairs
of the unchanged 1 MiB document, identical fonts and matching diagnostic
instrumentation. All six cells complete within the retained 2.5 GiB address-space
and 60-second process guards. Forty-eight checkpoints match full shaped source,
tree geometry and RGBA pixels. Across 45 settled typing/scrolling samples per arm,
acknowledgement p50/p95 falls from 196.809/202.802 ms to 3.123/4.024 ms. The giant
paragraph's inclusive draw median falls from 195.141 ms to 0.683 ms; median glyph
visits fall from 978,600 to 1,127, with 51 index nodes visited. These warm samples
perform no shaping or index construction and no index fallback.

Cold work remains separate: each arm builds 21 full paragraphs across setup and
width changes. The new arm builds 21 ink indices, at median 13.280 ms and at most
1,272,272 array-capacity bytes each. Combined cold-request acknowledgement median
is 662.985→496.942 ms, still far above an interactive frame budget. This is a
nonquiet, instrumented CPU/IPC comparison, not physical presentation or a 4 MiB
peak-memory result. Exact sources, six raw traces, pixels, commands and repeated
comparisons are retained in `target/linux-text-trace/ink-20260917/`. The corrected
executable SHA-256 is
`8e67ca398830bf13fcf76ab40b165636f642b78d3d7bf8e16c239917b7eede2a`.
Full-source synchronous wrapping and accepted-plus-working storage remain the
next bottlenecks; the warm paint improvement does not resolve them.

**Identified Linux inputs, Tuft and Leibniz, 2026-09-17:** independent paragraph
measurement and painting now resolve the kernel's paragraph stamp before owning
or hashing the full source. At most 256 catalog-local shortcuts retain a stamp
and checked canonical handle, without owning text or a paragraph. Metric changes,
catalog changes, eviction and unrelated kernel domains fall back to exact UTF8
identity. Paint-only changes still rebuild the current palette/source mapping;
TextInput's separate shown-value paint path is unchanged. New widths borrow the
canonical source but still shape synchronously. Existing accepted owner leases,
64 measured handoffs and soft cold-cache policy remain intact.

Nine tests pass on actual Ubuntu ARM64 as well as in the 203-test Linux-crate
suite on macOS (one existing GPU test ignored). A real giant-paragraph/sibling
typing regression reduces Linux-owned copied/hashed source bytes from
12,582,912/12,582,912 to zero/zero, with zero new giant shaping in both arms.
The tests separately check exact geometry and RGBA, catalog/owner lifetime,
paint-only changes, new widths and bounded shortcut metadata. The kernel still
constructs borrowed runs; this is not a zero-work or asynchronous layout claim.

Three fresh-process pairs then compare uninstrumented CPU-presenter release
binaries on the same four-vCPU/4-GiB Ubuntu ARM64 VM with DejaVu Sans. The document
contains 1,048,531 source bytes in two unchanged blocks. All six warm cells and
one additional fresh-width pair pass, with matching document hashes, complete
geometry and PNG bytes. Across nine typing and nine scroll ACKs per arm, pooled
medians are 2.604→2.439 ms and 1.476→1.250 ms respectively. This small sample is
not a robust tail estimate. Cold-size medians remain 209.07→208.33 ms; the single
fresh-width control is 197.66→205.31 ms, not evidence of faster reflow. The
2.5-GiB address-space and 60-second guards remain; no 4-MiB run was performed.

These ACKs include IPC and host work, not physical display cadence. Raw tests
and the reviewed four-file source are in `target/linux-paragraph-stamps-validation/`;
source/font/binary identities, eight cells and pixels are in
`target/linux-paragraph-stamps-native/`. The after release executable SHA-256 is
`51e5535ebedc13c6effb565dbf082374a4be4939afcaf07357cf4d13222732e1`.
Cold full-source wrapping and the earlier 4-MiB accepted-plus-working allocation
failure remain open.

The wider parallel suite also exposed an image-worker scheduling edge: a decode
obtained through `wait_decode` did not give the next turn to metadata. A
controlled arrival test now fails on two consecutive decodes and passes with
metadata between them. The former shared-pool test's late resolver counter could
also exceed its constant bound after prompt admission, so deterministic turns
prove ordering while the shared-pool check retains actual completion. All three
new scheduling regressions and the final parallel suite pass; the original
9/12-counter failures remain preserved, rather than treated as passing retries.

**Linux reserve-only diagnostic, Tuft, Leibniz and Zeno, 2026-09-17:** eight
fresh-process cells compare the same instrumented `42267a3` host and font catalog,
with only cosmic-text 0.19's UTF8-byte-count `ShapeSpan` reservation deleted in
the treatment. Two repeats per arm and document size retain the 2.5-GiB
address-space and 60-second process limits; the second repeat reverses arm order.
Both binaries and all captured source/dependency hashes remain unchanged across
the runs. This experiment changes no production dependency or host source.

All four 1-MiB cells complete. Each has seven complete paragraph builds and
matching source/glyph geometry; cold, wide and fresh viewport PNGs also match.
All four 4-MiB cells still abort on the first 600-to-632-point width change.
They first accept the full 4,194,149-byte paragraph in the unchanged two-block,
4,194,181-byte document: 55,923 lines, complete final-cluster coverage, identical
glyph digests and cold viewport PNGs. Subsequent width geometry/pixels are
unavailable, not passing comparisons. The diagnostic's successful 1-MiB oracle
result must not be read as eight completed workloads.

In both repeats, peak tracked outer-span capacity falls from 117,432,000 to
448 bytes at 1 MiB, and from 469,744,800 to 448 bytes at 4 MiB. Completed accessible
paragraph-capacity checkpoints fall from 769,304,320 to 651,872,768 bytes and
from 1,532,773,520 to 1,297,901,344 bytes respectively. These are vector-capacity
subsets, not total allocation peaks or resident memory. The latter counter omits
the building replacement, lazy ink growth and private text/font/scratch storage;
shape subtotals overlap it. Cold 4-MiB process address-space peaks fall from
about 1,914.5 to 1,689.5 MiB while RSS remains about 1,241 MiB, illustrating why
unused reserved capacity cannot be labeled resident bytes saved.

At failure, the old accepted paragraph remains pinned while a distinct
replacement is building. Baseline aborts before its second shape completes;
treatment completes that shape but aborts before host shaping/layout returns.
Both reach the address-space cap. Treatment's higher abort-time RSS reflects
farther progress, not equal completed work. Even without the oversized outer
reservation, one completed 4-MiB shape owns 663,013,536 bytes of visible capacities
and its width layout another 634,624,352 bytes. This supports testing immutable
shaping shared between accepted and replacement widths; it does not prove that
sharing will fit the workload or solve synchronous reflow.

Raw cells, binaries, capacity/lifetime evidence and independent review are in
`target/linux-shape-reserve-v2-execution/`; the treatment binary SHA-256 is
`276c051747e0db021a3d7f288f088891cb6caa7cfd5c6a08b0b40422caff23c5`.
All aborts and harness/build setup failures remain preserved. Sampled process
memory can miss transients; synchronous traces and whole-source/glyph hashing
also preclude uninstrumented latency or physical 120-Hz claims.

**Apple engine:** twenty-five actual engine methods pass 437 assertions in the
optimized standalone harness, including complete Unicode ranges, independent
accepted widths, last-owner release, checkpoint/catalog isolation and exhaustive
bitmap geometry. This remains distinct from full-host/XCTest/device validation.
Eighteen paired phase cells complete: 4 MiB cold measurement is 249.740→252.235 ms,
first dirty paint 270.366→272.138 ms and fresh-width measurement
168.525→150.157 ms. Warm measurement remains about 6 ms and warm dirty paint
about 0.2 ms. Whole source and UTF16 coverage remain intact.

Fifty-four separate ownership sweeps exercise 0/8/100 unseen widths with two
accepted paragraph owners, one resizing and one fixed at 900 units. All 27 after
cells finish. In the 100-width case, the 256 KiB median final Mach physical
footprint falls from 597.88 to 33.92 MiB. The 1 MiB and 4 MiB baseline cells
cross the 2 GiB diagnostic footprint guard after 87 and 18 widths respectively;
these six runs are incomplete, not 100-width successes. All after runs reach
100 widths, with median final footprint 103.53 and 357.74 MiB respectively.
Two accepted paragraphs remain live at the end; dropping those owners leaves
zero live historical Paragraphs, which does not imply zero process memory.
Full source/fixture/binary hashes, raw guard stops and samples are under
`target/apple-text-residency-20260917/paired/`. These standalone measurements
do not establish whole-app latency, asynchronous reflow or physical 120 Hz.

The integrated optimized AppKit Markdown build also passes a real-window drive:
4,194,181 source bytes, two unsplit blocks, revision zero, exact input echoes,
scrolling and eight fresh window widths. The paragraph's 4,194,149 UTF8 bytes
and 3,998,422 UTF16 units retain their source digest. The agent observes source
and rectangles, not every glyph's hit range; the bitmap/range oracles above
cover that distinction. Nonquiet ACKs still expose 311–746 ms fresh-width work
and roughly 0.6 s for some first inputs following reflow. Preserved binary
SHA-256 is `74de0e08174483914bccacb9d01c927a940b0ab3caff19e08993f38c2f06d18d`;
build logs, exact samples and screenshots are in `target/apple-text-integration-final/`.
Full macOS compilation and this drive validate the acceptance hook; Swift
Package XCTest and UIKit remain unverified on this Command Line Tools machine.

**Identified Apple scalar metrics, Tuft and Darwin, 2026-09-17:** the Rust
callback measurer now checks the paragraph stamp before constructing C runs or
calling Swift. Each measurer retains at most 256 generational owners, four exact
typed width/height offer pairs per owner, scalar metrics and payload-free stamps.
Metric/domain changes miss; paint-only revisions may reuse metrics. Catalog
lifetime is the measurer's lifetime: current boot/candidate paths construct a
fresh measurer before installing fonts. Invalid raw callback results retain the
previous sanitized return behavior but are never memoized; valid zeros remain
cacheable. Anonymous requests still use the original foreign path.

The meaningful 4-MiB paragraph/sibling-typing regression changes four additional
foreign callbacks to zero after the initial call. Ten new Rust tests also cover
exact offers, owner bounds, source/namespace revisions, malformed-result retry
and candidate/catalog lifetimes. All 118 Apple-package tests pass on integrated
source, with strict Clippy, formatting, caps and boot. The optimized standalone
Swift suite passes 32 methods/517 assertions, including accepted source, links,
catalog restoration and bitmap geometry; it is not full XCTest. NodeText's
accepted paragraph and paint/source/selection ownership are unchanged.

Frozen source, meaningful RED/green logs, independent review and main integration
checks are in `target/apple-identified-measure-validation/`.

The subsequent controlled AppKit comparison uses baseline `8f3eec6` versus only
the six memo files, with identical bounded diagnostics in both optimized builds.
Three alternating fresh-process pairs pass on the same M4/macOS machine. Each
retains the full 4,194,181-byte, two-block document and verifies exact Unicode
echoes, source digest, viewport and block rectangles. Accepted giant-paragraph
identity and geometry remain unchanged during warm input/height samples; both
arms produce the same new geometry at the fresh width.

After priming heights 820 and 821 at window width 980, the nine return-to-820
ACKs per arm fall from median 394.635 to 2.349 ms (p95 398.069 to 2.661 ms).
Their giant foreign callbacks fall from 18, supplying 75,494,682 bytes, to zero.
The other nine height samples were already fast: medians 1.431 to 1.363 ms.
Across 36 warm input ACKs per arm, medians are 3.167 to 3.151 ms; neither arm
calls the giant foreign measurer for those inputs. This is a height-offer reuse
result, not evidence that ordinary typing previously copied the giant source.

Cold document-load medians remain 284.611 to 264.943 ms, including generation
and parsing. The three fresh-width controls remain slow: 671.474 to 485.585 ms,
followed by first-input ACK medians 591.302 to 547.257 ms. The latter interval
has no giant foreign callback; native paragraph acceptance can occur after the
resize reply. These observations do not attribute all remaining time to shaping
or establish faster continuous live resize. Cold reflow and paint publication
still require separate work.

Raw six-cell evidence is in
`target/apple-identified-measure-validation/native-comparison-preparation/paired-20260917-064910/`.
The after executable SHA-256 is
`40704293cbcd9c5f681d2b5a47a2dcdd6e4068552ab23d80f8cb881d5cc7d5b2`.
The initial six aborted cells are preserved separately: a driver looked for
test IDs in layout replies instead of joining their numeric IDs to the tree;
correcting only that oracle enabled the unchanged workload. Timings include
instrumentation and agent IPC under a seekable clock; programmatic NSWindow
resize is not physical tracking. Tree/layout reads and screenshots are outside
input ACK timing. No RSS bound, autonomous background progress, UIKit or
physical 120-Hz result is claimed.

### 8.12 Numeric sheet-height presentation trial, 2026-09-17

The kernel trial separates sampled layout height from authored state.
Leibniz owns the kernel prototype and tests in an isolated worktree; Tuft owns
integration and the subsequent native/DOM experiment. The first consumer is one
numeric-pixel-height panel and its virtualized nested collection. This does not
introduce a general parallel layout engine or a second application value graph.

Kernel input is `PresentedHeight { node: NodeKey, epoch: u64, px: f32 }`
on `compute_layout_presented(root, offer, Option<PresentedHeight>)`; ordinary
layout passes `None`. Height remains a CSS height subject to current box sizing,
min/max constraints and aspect ratio, not a forced border-box rectangle. One
cached derived projection applies at a time. Equal samples reuse layout state;
switching or clearing restores current authored lowering, never a saved target.
Central style writes preserve the sample while refreshing other authored fields.
Arena styles, masks, commit receipts and authored epoch remain unchanged; exported
frames, hit testing and collection feedback intentionally observe presentation.

Preflight validates offer/root, current epoch, live generational key, root
membership, eligible box style and finite nonnegative height before changing a
previous projection. Stale/foreign samples cannot revoke a valid one. Rebuild
reapplies the validated sample; failed text/layout work preserves published frames
and invalidates derived cache state so recovery really remeasures. A later motion
adapter must release ownership when authored height becomes unsupported instead
of preserving an obsolete pixel target.

Tests cover CSS box constraints and sibling flow, repeated clean samples,
authored/env/intrinsic changes, switch/clear, stale/detached/reused keys, fault
rebuild and invalid-measurement recovery, and unchanged authored export columns.
Then price the actual projection plus collection feedback against the existing
25k authored-update control. Its earlier microsecond layout result does not
price native fonts, paint, projection or gesture delivery. Motion property and
host recognition changes follow this proof; no continuous sheet implementation
or physical-frame result is claimed by writing this plan.

The kernel implementation passes all160 kernel tests, including20 projection
cases, and strict all-target Clippy. Independent review added cross-root failure,
percentage max-height during same-epoch resizing, border/content sizing, min>max
and the existing auto-width-root border-box exception. One global projection is
intentional: switching/clearing restores derived state for the old root but only
the addressed root publishes in that call. Native adapters must account for this
when they iterate multiple roots; no multi-root motion policy is implemented.

The same-binary gallery comparison runs three fresh processes per arm/count,
120 expanding heights each: all12 cells and1,440 samples pass. At25k records,
authored action+layout+feedback p50 is45.250–50.833 µs and p95 is56.667–61.458 µs;
projection+feedback p50 is4.542–4.875 µs and p95 is6.583–7.542 µs. The first1k
authored repetition is slower than the other two and remains in the raw data.
Paired geometry, coverage endpoints and lifetime counts match exactly; both arms
perform135 monospace text measures per120 changing samples, and repeated equal
samples perform none. No resources are queried or full inputs keyed after setup.
The projected arm keeps the authored target178px; collection refinement refreshes
the sample epoch after its own commits. Two to five rows stay mounted while all
25,000 logical records remain supplied.

`target/presented-height-metrics/` retains raw cells, full source/binary hashes,
controller, compiler observations and readable report. Binary SHA-256 is
`76526657a914ad1a652df61c734fef5e50a96ebf9113e969183ab2dd3b557f35`.
This is M4 Runner/kernel CPU evidence using deterministic monospace, fixed width
and top-of-list expansion. It admits the next narrow motion/adapter experiment;
it does not establish native text/paint cost, continuous gestures or physical
120Hz. Max-height takeover must still sample displayed CSS height, and switching
to auto/%/env must retire Height alone while other properties continue.

**Programmatic motion/host increment, Tuft, Zeno, Darwin and Leibniz,
2026-09-17:** the shared engine appends Height without assigning `auto` a numeric
identity. One explicit host registration synchronizes its latest numeric target
and declaration; ordinary boot/receipt targets remain the four compositor
properties. A retirement tombstone clears only Height, including host projection
or DOM overlay/playback. Native adapters require one root and an explicit
border-box owner. Catch uses published constrained geometry, while each layout
entry revalidates the owner and carries a fresh projection epoch through actual
collection feedback. Cached successful samples keep unchanged held Height from
relayout on unrelated Translate ticks; a failed layout preserves published frames
and forces the next attempt to retry. Repeated live registration is idempotent
while hidden/unsupported, and destroyed keys cannot regain ownership.

External held positions use finite nonnegative f32 layout range with scalar y=0;
stale callbacks return before validation or clocks. Internal negative spring
samples and signed release velocity remain raw in the engine. Native projection
and the DOM formatter clamp only the displayed length to zero, preserving every
keyframe, delayed start and rebound. CSS/WAAPI still execute web animation.

Before integration the core passes 57 motion and 167 kernel tests; Apple passes
91 Rust tests including 14 Height cases, Linux passes 124 scoped tests including
21 new Height cases (one existing GPU test ignored), and web passes 72 Rust
tests plus 26 real-DOM tests/76 assertions. Three existing web browser-carrier
tests remain ignored; the DOM tests use a mock bridge, while separate Rust tests
exercise Host/Engine and the actual binary bridge. Independent review reproduced
and fixed extra Linux layout calls under held Height and non-idempotent hidden
registration. Evidence and exact frozen source hashes are in
`target/height-{motion,apple,linux,web}-validation/`; coherent main validation is
recorded in `target/height-integration-validation/`. The combined main tree
passes all 538 tests across motion, kernel and the three host crates, with four
existing GPU/browser-carrier tests ignored. Strict all-targets Clippy for those
five crates, wasm32 checks for motion/kernel/web, scoped formatting, caps and
boot pass using an isolated Cargo target. Existing whole-workspace Hermes and
sibling-format limitations remain; these are scoped integration results.

These are programmatic adapter tests. Linux-crate validation here executes on
macOS, and Apple has no Swift registration/vertical recognizer in this increment.
No physical sheet drive, asynchronous layout or frame-rate result is claimed.
The next consumer slice is an explicit authored header handle referencing its
panel ID, a typed height/velocity release event, and a synchronous snap action
before the token ends. It will deliberately replace the gallery's percentage
stops with numeric pixel stops; inner-list boundary arbitration remains separate.

**Authored header increment, Tuft, Zeno, Newton, Darwin, Leibniz and Epicurus,
2026-09-17:** `heightDragFor` resolves one strict ancestor's authored ID through
the kernel. Physical admission also requires a `heightrelease` handler. One
target supports multiple handles; a property-only node cannot take its place.
Hosts retain live handler declarations across absent/set/clear IDREFs and check
handle generation, resolved target and token before clock or action. Explicit
replacement retains programmatic ownership; same-key registration remains a
no-op. Cancellation or removal does not invoke the release action.

The gallery replaces its percentage/data-module height with one Contract
`sheetPx` value, initially 360, and local stops 180/360/640. Release projects the
displayed height by 0.15 times signed velocity and synchronously chooses a stop
while the token is held. The final pointer sample precedes that action, then
release and collection-pin cleanup follow. Header recognition leaves buttons
and inner-list scrolling independent. Native samples use constrained displayed
height; the browser continues to execute CSS/WAAPI. The existing agent `tap`
operation now carries Linux contact phases through the same Presenter methods,
explicitly labeled synthesized/headless rather than physical input.

Review regressions cover cancellation at an accepted receipt's current clock
and newest target/transition, without draining unrelated pending motion. Browser
cleanup handles Rust-first cancellation with a second surviving handle and
synchronous binding retirement inside a begin reply. Native tests cover explicit
owner replacement, handler admission and dormant IDREF reuse. A late Linux
up/cancel after another input retires contact acknowledges `contact:false`
without another action or clock step, allowing the next contact to start.

The corrected real-Wasm Chromium gallery passes 48 trace assertions and 18 actual
inner-List geometry samples at 1,000 logical rows, with at most 9 mounted rows.
Recognition catches 517.078px rather than pointer-down 443.266px; reverse and
typing preserve the held sample while the latest target remains 640. The final
437.062px/-1250px/s action publishes 180 before the same token ends. Narrow-window
constraints, live width changes, actual wheel coverage and removal/remount stale
callbacks pass. Geometry checks wait for ResizeObserver delivery; they do not
claim every intervening frame has no gap. A separate normal browser without the
agent clock passes drag/reverse/typing/release and ordinary wall-clock WAAPI
settling. Corrected Wasm SHA-256 is
`993886301f5f74c04d194e5b2625491814a04cf1d5b4a3e5a65d18f372acd42c`;
raw commands, observer records, screenshots and the distinctly labeled unfixed
baseline remain in `target/gallery-height-browser/`.

The final registration correction is rebuilt as V3, Wasm SHA-256
`bbb9184c08c9ad1d81128285ec55c27b71c1108a4551fddbc17e42afa54da3c3`.
Its setter is reachable through motion ABI operations 6/7, so the changed binary
receives another ordinary-browser drive: all 10 checks pass, including actual
header drag, typing while held, reversal and wall-clock release/settlement.
The earlier 48-assertion trace belongs to V2 and was not repeated on V3. Both
builds, exact source overlays and raw drives remain preserved separately.

The AppKit gallery's real NSEvent drive passes catch/reverse/snap, typing and
constrained-height checks, with actual sheet/inner-port dimensions following
360/273→540/453→562.79/475.79. Eight separate AppKit lifetime cases cover
disable/rebind/remove/hide/inert/detach/reset/destroy and late callbacks. Those
drives use the retained baseline18 binary. The subsequent Rust-only ownership
and handler-admission correction passes 108 Rust tests and an optimized AppKit
rebuild; unchanged Swift and earlier drive evidence are identified separately,
not presented as a rerun. Artifacts are in `target/height-drag-apple-validation/`
and `target/height-drag-apple-revision19/`. UIKit paths remain uncompiled on this
Command Line Tools machine; standalone Swift checks are not XCTest.

The frozen Linux revision4 capture passes 198 tests with one existing GPU test
ignored, strict all-target Clippy, and both display-input tests on Ubuntu ARM64.
The shared agent's actual stdio drive proves contact phases, typing, constrained
catch/reverse, cancellation and stale-terminal acknowledgement. At 25k records,
the final logical rows cover the real 273px inner port before and after resize,
with four to five mounted rows. A separate non-agent VNC input drive through the
normal VKMS display loop shows constrained Full at 516px, held shrink to 366px,
typing during the hold, Escape returning to Full without a release action,
recatch/reverse and release to Read at 360px. Inner-list wheel scrolling leaves
the header at Read. The unpaced attempts that did not establish completed
actions remain labeled inconclusive; accepted framebuffer observations use
paced input/capture. This software 60Hz display has no evdev hardware device.
Evidence is `target/height-drag-linux-native/revision4/`, retained executable
SHA-256 `e71067087af2f68b0620b7d1294a5bb5b4035563a3d777043372675f66c8fd9f`.
None of these checks establishes physical 120Hz or inner-list-to-header gesture
arbitration.

A separate revision19 AppKit 25k-by-20 traversal attempt stops for runtime cost
after 986 distinct rows of the first traversal: zero complete traversals, not an
acceptance pass. The one-second main-thread profile repeatedly reaches
cache-wide `TextResidency` pruning, cold-byte accounting and width retirement
from collection layout. This supports a bookkeeping hotspot, not attribution
of all elapsed time to one function. The partial scan retains 8–12 mounted rows,
147–200 mapped nodes and six RSS samples of 178.72–229.78 MiB; these do not prove
full-traversal bounds. Raw journals, partial coverage, exact source/binary
identity and the stop report are under
`target/native-raster-20260917/macos-accept25k-revision19-20260917-043443/`.
**Cache maintenance increment, Tuft, Darwin and Epicurus, 2026-09-17:**
width retirement now uses per-identity indexes, while value-semantic LRU links
and incremental source/shaper charges replace cache-wide pruning, sorting and
byte recounts. Each operation sweeps at most four lookup and four identity
records. Cold entries, weak lookups and identity metadata have explicit caps;
forgetting a lookup may require later remeasurement but cannot invalidate a
view's accepted paragraph. Checkpoints retain their independent COW state.
The 64 MiB cold target remains soft, with conservative lazy-ink admission and
observable overage; external accepted owners and opaque CoreText memory are
not a total-residency ceiling.

With eight accepted paragraphs and 10/100/1000 prior short identities, a warm
hit, unrelated miss and acceptance previously visited 257/1247/11147 metadata
entries; the new path visits 99/97/97. This discriminator excludes the old
extra byte-accounting scans, statistics enumeration and COW costs. All 31
standalone engine methods/501 assertions pass, including existing Unicode and
bitmap oracles; the optimized full AppKit build also passes. This machine's
standalone test extraction is not XCTest.

Three fresh native processes per arm then complete the same 100-wheel prefix
on 25k logical records. All six cells expose 217 contiguous records and match
normalized geometry at every observation, with five exact Unicode inputs and
five queued resizes each. Prefix wall time falls from 45.44–46.27 s to
4.34–4.79 s. Ordinary wheel ACK p50 falls from 146.98–152.77 ms to
12.21–13.17 ms; p95 falls from 452.03–462.66 ms to 31.81–34.41 ms.
Each cell has only five queued typing samples: their medians fall from
277.28–289.89 ms to 25.59–26.16 ms, not a robust typing-tail estimate.
ACK includes host settlement and IPC; prefix wall includes diagnostic reads.
Runs were nonquiet. Both arms retain 8–12 mounted rows and 147–200 mapped
nodes; sampled RSS does not show a memory reduction. These short prefixes
complete no whole-document traversal and do not replace the earlier incomplete
25k-by-20 attempt or establish physical frame rate.

Exact source/build identities, six raw journals and separated query/ACK timings
are in `target/apple-text-maintenance-validation/`, with native evidence under
`native-comparison-20260917-051731/`. The preserved after executable SHA-256 is
`8048a7e115363edf8cf0f943b0fd1c1286ecfe996ae80dcea840703117242c0d`.

Coherent main validation passes 891 tests across the compiler, plan, runner,
motion, three hosts and gallery data. A final two-case web registration
correction then passes all 92 web tests: same-owner registration preserves
automatic lifetime, and explicit clearing does not re-adopt before returning.
Four existing opt-in GPU/browser-carrier tests remain ignored. Strict scoped
all-target Clippy, core/web Wasm checks, formatting, caps and boot pass; the
real-DOM suite passes 33 tests/98 assertions and the agent protocol fixture
passes three tests/13 assertions. These are scoped results, with the previously
recorded workspace Hermes and sibling-format limitations unchanged. Exact
sources, failed reproductions, commands and logs are retained under
`target/height-binding-integration-validation/`.

### 8.13 Paired photo dragging on Web and native hosts, 2026-09-17

Tuft, Carson, Newton and Zeno implement and validate the first photo drag
consumer. A keyed viewer owns local pan/zoom state; accepted source replacement
creates new target/handle keys, even for equally sized images. The handle's
authored `transformDragFor` resolves one coherent handle/target/clip binding.
The wrapper fills its direct zero-inset clip and contains the image. Fit/2× and
reset remain synchronous Contract actions; release clamps pan against natural
image dimensions and the newest authored zoom, without a data request.

The Web adapter catches Translate and Scale together through the existing
atomic motion pair. Both tokens, all three generational keys, runtime and
geometry identity must remain live before movement or release changes time.
Geometry includes coordinate mapping, independently of dimension-event equality:
ancestor scrolling can cancel a pair without reporting changed dimensions.
Accepted receipts synchronize current targets/declarations while held before
cancellation. Release applies its final sample, dispatches the authored action
while both holds are live, then independently ends surviving original tokens.
The browser continues to execute springs; no Rust frame loop is introduced.

The integrated Web crate passes 110 tests, with three existing browser-carrier
tests ignored, plus strict all-target Clippy, native build and Wasm check.
Eighteen new Rust cases cover admission, lifetime and bounded current-pair
metadata. The 44-test Chromium DOM suite passes 149 assertions using mocked
Rust replies; it is separate from the actual-consumer drive below. Frozen
source, meaningful failures and independent review are in
`target/photo-web-validation/`.

The real baked gallery then passes 22 correctness checks and 91 replay commands
in ordinary-clock Chromium, without `exact.agent` or `exact.now`. CDP mouse
events exercise zero-displacement catch, parent-space pan, regrab and release.
Space activates Fit while both properties remain held; focus is placed with
DOM `focus()`, not keyboard traversal. Typing preserves presentation, and the
release action commits the latest Fit target before both original token ends.
The observed journal interval contains no resource queries. Contain bounds,
actual viewport resize, ancestor wheel scrolling and same-sized keyed source
replacement pass. A separately labeled real-ABI property takeover preserves its
successor and cleans the old pair; it is not a physical input case.

All 17 observed tokens are dead at completion, with no browser errors or trace
overflow. The running-pair catch is near the spring tails and exactly matches
the sampled CSS presentation; it does not prove every curve point. Exploratory
driver-oracle failures are preserved, with no product edits or rebuilt binary
between runs. Exact 24-file source identity, raw replays, screenshots and the
runnable bundle are in `target/photo-gallery-browser/`. Its Wasm SHA-256 is
`b7d9ea51b1621deb756d02b829539c29b1157001dfb18bfa0622a0e3d248eefd`.
This is instrumented correctness evidence, not latency, native photo parity,
physical pinch, shared-element return or physical 120-Hz presentation.

**Linux adapter, Tuft, Leibniz and Epicurus, 2026-09-17:** the existing Presenter
pointer path now recognizes the same authored photo binding. It catches the
currently published Translate/Scale pair, preserves parent-coordinate pan and
velocity, dispatches the terminal action while both tokens remain live, and
cleans only surviving original tokens. Actual kernel frames and accumulated
scroll offsets certify fill, origin and mapping lifetime. Unsupported ancestor
presentation refuses admission; same-size mapping changes cancel without a
duplicate dimension action. Feedback allows two synchronous passes and leaves
further changes for an external turn, refusing stale contact in the meantime.

All 221 integrated Linux-crate tests pass on macOS, with one existing GPU test
ignored, plus strict all-target Clippy, build, formatting, caps and boot.
Eighteen new tests include real Presenter pointer delivery, full-tuple refusal,
current receipt declaration/time, geometry changes and callback destruction.
A retained failure caught unrelated Height motion advancing during a pair update
without projected layout; pair lowering now uses the ordinary complete hold path.
Exact eight-file sources, failures, independent review and integration logs are
in `target/photo-linux-validation/`. This macOS package run supplies no actual
Linux, display or GPU result.

**Actual Linux acceptance, Tuft, Leibniz and Zeno, 2026-09-17:** an immutable
`e565158` capture passes all 18 binding/contact tests on Ubuntu ARM64 and builds
the optimized gallery against registry cosmic-text, without the reserve/trace
experiment. The real shared-agent stdio replay passes 54 recorded observations:
zero-displacement catch, 96×48 parent-space pan at scale 2, typing, latest Fit
while held, release, spring recatch/reversal, resize cancellation and keyed
source replacement. Fit during a hold is programmatic control dispatch, not a
second physical click. An initial driver-only resource-path error is preserved;
the corrected replay uses the identical executable without a rebuild.

A separate non-agent run uses VNC input through the ordinary VKMS display loop
and CPU painter at 1024×768. Real decoded North shore and Ochre dunes images
appear in the framebuffer. Comparing recognition and held frames after a
96×48 translation gives mean absolute RGB channel error 0.000619; incorrectly
dividing movement by scale gives 49.2445 over the same 58,176 channels. The
centered Fit image spans 546 pixels, consistent with the expected 546.67 inside
a 984-pixel clip. These checks support containment and parent-space movement.

The exploratory two-channel-level return-frame error bound **fails** for Escape,
late-up and disconnect captures (2.9172, 2.0677 and 2.2827). The threshold and raw
failure remain unchanged; these live spring/image-resolution frames are not an
exact settled-image oracle. They show approximate visual return; precise stale
action/cancellation assertions come from the separate native tests and stdio
drive. The display journal also retains an existing unsupported `focus` command.
No all-VNC-checks-pass, GPU, hardware-input, latency or physical 120Hz claim is
made. VKMS advertises software 60Hz and has no attached evdev device.

All 1,174 captured source entries, fonts and executable/bake identities match
after execution, and all owned processes are gone. Evidence, both agent attempts,
pixel failure and framebuffer sequences are in `target/photo-linux-native-e565158/`.
The retained executable SHA-256 is
`7f3b7989f7c74d04bc2209ceac6ba9db8a9927ccee32b404e71d2c9c77d10d9d`.
Six asset files were hash-verified, but only two rendered studies were inspected;
this is not exhaustive decode, memory-bound, pinch or shared-element evidence.

**Apple adapter, Tuft, Darwin and Zeno, 2026-09-17:** the reviewed 21-file
implementation shares the atomic pair protocol and uses actual AppKit layer
presentation for catch. Both tokens are installed locally before synchronous
batch reentry. Geometry feedback coalesces and permits two refinement turns;
cancellation retires original token survivors before releasing the captured
collection pin. Rust validation passes 139 tests and strict all-target Clippy;
six standalone Swift methods/65 assertions and boundary-double probes are
separate from the full optimized AppKit product build, which also passes.

The actual NSWindow NSEvent gallery replay passes 55 operations and 12 geometry
observations at 100 logical records, using a seekable clock. Fit pan reaches
(120,55), exact Unicode typing preserves it, and a released spring is recaught
at (106.39,48.76) then reversed. A programmatic Space-key control action changes
the authored zoom to 2 while both properties remain held at scale 1; release
settles at 2. Reset, resize after contact ends, keyed source replacement and
deletion with stale lift also pass. Screenshots contain decoded imagery.

The first immediate-catch oracle failed because a virtual-clock layout report
and the current layer presentation differed. The corrected replay adds a 60ms
real held-contact phase before recognition, allowing the layer transaction to
reach presentation; exact continuity assertions remain. This establishes neither
immediate model/presentation equality nor normal-clock photo latency. The public
agent refuses resize during contact, so this drive proves no held-resize or
held-scroll invalidation. A supplemental runtime probe for those cases was
compiled but **not run**. The viewer is outside a collection, so this drive also
does not exercise a live collection interaction pin. UIKit is uncompiled and
standalone Swift checks are not the complete XCTest suite.

The first native link reused a stale shared Rust archive; refreshing only
`host/apple/src/abi.rs`'s mtime forced a rebuild with unchanged source bytes. That failure
and both gallery attempts are preserved in `target/photo-apple-validation/final/`.
The accepted executable SHA-256 is
`c67a15c2a631495bc609156bf4a7da23d9735570a88cddf1b983b63724670449`.
No physical pinch, shared-element return, latency or 120Hz claim is made.

### 8.14 Pending content-region kernel foundation, 2026-09-17

Tuft and Epicurus implement, with Zeno reviewing, one explicitly registered
content region inside an independently sized, clipped owner. Its first cold
publication uses a real authored placeholder; subsequent pending work retains
the last accepted geometry and exact text artifacts. The surrounding shell can
publish independently. This is a kernel foundation, not a native worker or
paint implementation, and ordinary text measurement remains synchronous.

The UI owner discovers exact text offers and exposes one missing request with
owned immutable source, paragraph stamp, font catalog identity and an opaque candidate
ticket. Completing an obsolete request cannot publish. Ready metrics retain an
artifact from that exact source and offer; final paint widths are included in
offer discovery. Internal missing-measurement probes never become published
ready geometry. Errors preserve the previous publication. Switching registered
owners restores current authored topology, and catalog changes invalidate shell
text caches before layout. Effective baseline participation under Flex or Grid
is conservatively refused because cutting descendants could alter the shell.

Collection provenance remains separate from paragraph identity. Retained old
geometry supplies no current row measurement; a new consumer revision must
publish with its own row epochs before collection feedback can accept it.
Contract actions, collection state and Taffy remain serial. The trial bounds
each branch to 4,096 mounted nodes and each candidate/publication to 64 exact
offers and 16MiB captured UTF-8. Source allocations are shared across widths;
opaque native artifacts, host-retained publications, spare capacities and font
storage are separate costs, not a process-memory ceiling.

The final private revision passes 225 kernel tests, including 28 region tests.
Integrated kernel/runner tests, strict all-target Clippy across the kernel,
runner and three hosts, native build, Wasm checks and scoped formatting pass.
Meaningful failures cover owner replacement, catalog invalidation and both Flex
and Grid baseline dependence. Two parsed 1MiB Markdown kernel fixtures passed
revision 2; they were not rerun for revision 3's Grid-only eligibility fix.
Exact sources, all revisions, failures and review are retained in
`target/cold-region-validation/`.

Native integration must still capture paint, links and selection metadata at
the same full source stamp, isolate font/shaping ownership, and make the painter
consume only the accepted publication. No host cold-shaping fallback, worker
latency, autonomous background reflow or physical 120Hz result is established
by these kernel tests.

### 8.15 Shared immutable Linux paragraph shaping, 2026-09-17

Tuft and Carson implement, with Zeno reviewing, one canonical shaped source per
exact text/metric/catalog identity. Each width owns independent wrapped glyph
arrays, CSS baselines and lazy ink data. Width changes reuse the public cosmic
`ShapeLine::layout_to_buffer` operation instead of rebuilding a full Buffer's
shaped source. Request-local layout scratch is dropped before publication.
The production cosmic-text 0.19 dependency and its reserve behavior are unchanged.

Accepted paragraph leases keep their original font/raster catalog alive through
CPU painting and GPU font extraction, including after catalog replacement. The
catalog has no back-reference to source, cache or engine. Existing per-NodeKey
accepted-frame leases and success/failure publication behavior remain unchanged.
Current, handoff, cold and retiring diagnostics deduplicate shared source and
width owners; source/layout subtotals overlap and must not be added twice.
The cold target remains soft, and private shaping scratch, font storage and RSS
are separate from accessible vector/string capacity accounting.

A retained identified rich-Unicode paragraph regression first showed a new
width shaping again despite zero canonical source copy/hash. It now reuses the
shape while preserving the previous width's glyphs, metrics and baselines.
The legacy full-Buffer oracle compares glyph fields, source clusters, selection
ranges and baseline bits across rich runs, bidi text, combining/emoji, blank
lines, spacing, alignment, wrapping and fractional offers. Catalog replacement
tests retain exact CPU pixels and GPU font owners, then prove old owners release.

All 226 integrated Linux-package tests pass on macOS, with one existing native
GPU test ignored; strict all-target Clippy, build, scoped formatting, caps and
boot pass. Five new tests and the retained identified-width behavioral RED, exact ten-file sources,
independent review and integration logs are in `target/immutable-shape-validation/`.

The subsequent actual Ubuntu ARM64 comparison runs four instrumented CPU/headless
cells once: 1MiB before/after, then 4MiB after/before, under the unchanged 2.5GiB
address-space guard. Both arms retain stock cosmic-text's reserve behavior. The
1MiB pair completes with 13 exact source/glyph/geometry/pixel/identity checks;
full shaping falls from seven builds to one across seven width layouts. The
4MiB treatment also completes all seven layouts from one shape, while baseline
aborts during its first 600-to-632px replacement. Initial 4MiB cold output matches;
later paired oracles remain UNAVAILABLE because baseline never reaches them.

Treatment 4MiB sampled RSS/HWM reaches 1,952,404KiB and VmPeak 2,588,816KiB, only
31.859375MiB below the guard. The sampled peak is distinct from the final ACK's
1,952,264KiB. This is an observed fit for one fixture/font set, not a robust memory
ceiling. Completed capacity checkpoints deduplicate canonical keys, shared shape,
each width and post-paint lazy ink; unfinished allocations, private scratch,
fonts and allocator overhead remain separate. Comparing completed treatment RSS
with an aborted baseline is not a like-for-like memory comparison.

This increment remains synchronous. Width layout is O(glyphs/lines), and accepted
plus working widths still require separate arrays. Instrumented 4MiB treatment
cold ACK is 1710.383ms, first width change 1000.441ms, and subsequent resize ACKs
766.723–796.302ms. These include IPC, paint and diagnostic hashing, not isolated
layout or production latency. No physical presentation or 120Hz claim follows.
Worker transfer and asynchronous publication remain separate work. Exact binaries,
all four cells including the abort, available oracles and independently reviewed
capacity reconstruction are in `target/linux-immutable-shape-native-v4-execution/`;
source/harness revisions and their retained failures are in the sibling V1–V4
captures. No native rerun or cap increase was used to obtain completion.

### 8.16 Apple resource clock during native tracking, 2026-09-17

Tuft implements the timer correction; Newton owns the native comparison and
Darwin reviews the source. The existing repeating 250ms session resource timer
now registers in the main run loop's common modes. Its callback, weak session
capture, agent-mode gate and destruction invalidation are unchanged. Contract
actions, Runner commits and layout remain serial on the UI owner; no worker,
second clock, timer-frequency change or completion-pump policy is added.

The actual Foundation regression first passed default-mode delivery and failed
both tracking-delivery cases. Common-mode registration passes all three methods,
including invalidation. A fourth original-versus-new timer test confirms the
initial 250ms deadline on Darwin. These actual methods compile with an assertion
shim under optimized strict-concurrency/warnings-as-errors Swift; they are not
a full XCTest/package run. The optimized native Messages treatment also builds.

Frozen normal-clock V3 baselines use `7a6fde2` plus a diagnostic-only observer,
real AppKit callbacks and externally posted Quartz window-edge input (synthetic
OS input). With 10,000 supplied Messages rows, windowing and batch32, the baseline
has zero
ordinary timer callbacks inside an 1850.332ms edge. The separate Storm128
baseline also has zero timer callbacks, while 129 coalesced completion-pump
chains and the echo handler run during its edge; these are not 129 requests.

Rebuilding that Messages source with only the timer delta produces seven
complete timer/advance/apply chains inside a 1794.829ms edge with 18 live changes.
The first echo enters the native queue 843.809ms after the edge starts; its handler
and ACK both arrive during the edge. The observation-window revision delta11 is
not a callback count. Three timer chains complete in the first command-free 750ms.
Source, driver, oracle and poster identities are retained. The passive
mode quota omits 321 records in the treatment, so its predeclared mode endpoint
remains NOTESTABLISHED despite positive retained tracking-mode samples; the
primary callback trace has no omissions. No endpoint or oracle was relaxed.

The saved starting window sizes differ (baseline 960x932, treatment 876x884), so
this is not a matched layout-cost comparison. This is one instrumented, nonquiet
native correctness comparison, not a latency,
all-frame, UIKit or physical 120Hz result. Storm was not rerun with the treatment.
The full baselines, failures, compiler-only correction and original archives stay
under `target/normal-clock-resize-v3-native/`; the isolated treatment and cleanup
proof are under `target/normal-clock-resize-v3-treatment-native/`. Focused RED/GREEN
tests, exact final sources and independent raw-chain reconstruction are under
`target/session-clock-validation/`.

### 8.17 Linux font and shaped-text transfer foundation, 2026-09-17

Leibniz implements and Zeno reviews worker-owned font generation and immutable
text transfer. The UI snapshots catalog metadata and source references. A worker
captures font bytes once per backing, preserves constructor-stage face identities
through the final database, and constructs separate shaping and raster systems.
The raster system moves to the UI; adoption wraps it without a UI constructor,
reshaping or shared mutable FontSystem. This creates a new catalog generation;
accepted paragraphs keep their old catalog rather than freezing old lazy files
retrospectively. Missing, changed or nonregular font inputs refuse publication.

Jobs retain exact source/stamp, private job/catalog identity and both offer axes.
Widths share immutable shape data and retain independent layout arrays. Intrinsic
probes release their layout arrays; stale adoption refuses before mutation. The
controller still owns final kernel liveness, request/completion limits and atomic
paint/source publication; these transfer helpers do not enable asynchronous UI
behavior by themselves.

All 243 integrated Linux-package tests pass on Darwin, with one existing native
GPU ignore, including 17 transfer tests using real threads and exact glyph,
baseline and CPU-pixel comparisons. Strict all-target Clippy and scoped formatting
pass. The system-font fixture captures 811,618,692 bytes (about 774MiB): shared
owned font content, not RSS, a peak bound or a claim that the new generation fits
the previous 4MiB process cap. Ordinary TextEngine startup does not eagerly perform
this capture. Native controller acceptance remains separate.
Exact seven-file sources, behavioral failures, independent review and integration
logs are retained in `target/text-transfer-validation/`.

A frozen `6b03bf4` actual Ubuntu ARM64 run then passes 42 scoped tests: 17 transfer,
four sharing, 14 CPU ink oracles and seven text integration tests. The existing
GPU-device test is unselected; this is not GPU rendering or cross-platform font
pixel equality. That VM's 39 faces use 13 captured font files totaling 107,296,644
bytes (about 102MiB), separately from the Mac's catalog. All four test processes
complete under the retained 2.5GiB address-space guard, but they are not a giant
controller drive. One staging-permission failure before Cargo is preserved;
source, dependency, symlink and font identities remain unchanged. Exact binaries,
raw outputs and cleanup are in `target/linux-text-transfer-native-execution-v2/`.

That initial transfer adopted an empty lazy ink cache: first UI paint still
scanned the full glyph layout to build its index. A subsequent four-file change
by Leibniz prepares the existing CPU ink index on the worker. Each private job
binds exact scale bits alongside source, both offers and catalog identity.
Definite results own the numeric index arrays or explicitly refuse; intrinsic
results retain neither an index nor width arrays. UI adoption moves the arrays
and attaches the matching raster catalog token without a scan. The existing
8MiB index-array limit excludes font caches, shape/layout storage and scratch.

The first-paint regression fails on the original empty-cache transfer and passes
with preparation. A 77,824-byte, 53,248-glyph paragraph builds its index on a real
worker thread; first, middle and end CPU paints match the full-glyph pixel oracle
with zero UI index builds. Visible glyph-cache calls are 156/156/110, not cache
misses or latency measurements. Scale changes, stale and foreign-catalog results,
intrinsic probes and refusal lifetimes are covered. The private Darwin run passes
248 Linux-package tests with one existing GPU ignore. Integration initially
passes 247 but hits an image-worker progress timeout. A deterministic reproduction
shows the test's setup poll draining the delivery cells it subsequently waits
to observe; the test also passes alone. A test-only decode-completion barrier
establishes the intended undrained state, preserving the cross-session progress
oracle without changing production code or timeouts. The corrected integrated
run passes all 248 tests with the existing GPU ignore and strict all-target
Clippy. Initial failure, isolated pass and reproduction remain preserved under
`target/image-delivery-test-validation/`; integrated results are under
`target/image-delivery-test-integration/`.
Sources, failures and integration evidence are in `target/text-transfer-ink-validation/`.
The preceding actual-Linux 42-test capture does not include this follow-up.
A subsequent immutable `7fe1b7e` Ubuntu ARM64 build passes all 47 selected tests:
22 transfer (including five prepared-ink tests), four sharing, 14 CPU ink oracles
and seven text integration tests. The 53,248-glyph fixture builds its index on
the worker; top, middle and final CPU paints match exact RGBA with zero UI index
builds. One build and four test processes complete without retries under the
existing 2.5GiB address-space and 60-second process guards. The font files remain
unchanged from the preceding Linux run. Sources, ELFs, raw results and terminal
cleanup are in `target/linux-text-transfer-ink-native-execution/`. This is worker
ink correctness on Linux, not a giant controller drive or latency measurement.

Controller admission must still reject mismatched paint context. A crate-private
paragraph predicate now checks the existing index, catalog token, exact scale
and conservative viewport calculation without building, walking glyphs or
allocating. Conflicting borrows and unsupported queries refuse. Three focused
tests cover missing/stale indexes, uncertain placement and repeated probes;
supported fractional-scale paints remain exact, while ordinary lazy building
and full-glyph fallback are unchanged. Integrated tests and strict Clippy pass;
the initial RED is a missing-API compile failure, not a behavioral comparison.
Evidence is in `target/prepared-ink-query-validation/` and
`target/prepared-ink-query-integration/`. The controller still must call this
predicate before accepted text reaches the fallback-capable painter.
Ordinary painting at another scale can rebuild the cache, conservative queries
can fall back to all glyphs, and GPU painting still constructs full position vectors.
Visible glyph raster work also remains on the UI. These helpers therefore do not
prove a complete responsive publication path or physical 120Hz. The private
pre-parsed native fixture also excludes Markdown Stress's cold source generation
and parse cost, addressed separately by the opt-in below. The actual Markdown
reader already reads and parses files in its native continuation, but constructs
Runner Values and enumerates sibling files when settling on the UI owner.

### 8.18 Native Markdown Stress parse continuation, 2026-09-17

Newton implements an explicit `NativeMarkdownStress` data source on the existing
ordered native continuation lane. Cold runtime requests generate and parse the
fixture on that worker, retaining the baked or previous document while pending.
The original `MarkdownStress` stays synchronous for bake, web and the pathological
control. Host runtime selection is a separate integration step; adding this data
source alone does not change the shipped native entry or move Contract execution.

One accepted parsed document and one latest result cell are retained. Queued
closures hold weak references, so superseded generations skip work; a running
obsolete parse drops its result without requiring a Runner parse callback.
Same-source page requests share one parse and get distinct checked process-wide
serials. Completion carries only an eight-byte acknowledgement; adoption checks
the current serial, claim and all request arguments. Invalid input or exhaustion
preserves the current request. Admission failure retains accepted content and
reports an error, with no automatic retry or synchronous fallback. Running work
is not preempted, and the executor's queued closures are accounted separately.

The original synchronous source fails eight behavioral tests. Final private and
integrated data suites pass 26 tests, then two explicitly selected giant tests
compare complete 1MiB and 4MiB paragraph/code Values against that source. This is
28 distinct passes (21 new), with the preexisting ignored giant parser test still
unrun. Real threads prove stale payload release, one active parse through 100-key
churn and one shared parse for 20 page tickets. A real baked Runner keeps its old
document while pending; typing and width changes issue no new data work.
Strict all-target Clippy, scoped formatting and the integrated wasm32 data-crate
check pass. Sources, behavioral and setup failures are preserved in
`target/markdown-continuation-validation/`; integrated results are in
`target/markdown-continuation-integration/`.

Value/Rc string construction, table-row counting, previous-document destruction,
Runner validation/realization and layout remain on the UI owner. Forty blocks
per manual page is not a byte bound: the giant single block stays intact.
This app-specific continuation does not implement general module placement,
generic resource cancellation, total-memory bounds or native responsiveness.

### 8.19 Linux content-region controller and cold launch, 2026-09-17

Epicurus implements the native controller; Zeno reviews thread/source/adoption
and Tuft reviews paint, display readiness and integration. One explicitly
registered region discovers exact offers on the UI owner and sends owned work
to one process-lifetime font service. One active job, one replaceable pending
job and one undrained result are permitted. Retiring work keeps admission until
its actual worker-owned allocations drop. There is no UI join, per-request
thread or parallel Contract/Taffy execution.

The first successful authored placeholder frame precedes font-recipe capture.
Worker-prepared final paragraphs carry the exact source, stamp, both offers,
catalog and raster scale. Stale results cannot adopt. The painter admits only
prepared CPU indexes compatible with the current query; unsupported queries
cannot silently invoke full-paragraph fallback. Flat retained paint commands
pin source, palette, paragraphs and hit metadata. Successful backend completion
publishes them together; failed B can preserve displayed A while C is pending.
Collection feedback additionally requires the painted publication, origin and
current row epochs. Read-only retained boxes cannot dispatch current actions;
general selection and link activation remain outside this trial.

The real Linux display loop watches the region completion FD alongside existing
executor/image readiness. The stdio agent is still command-pumped. Source
inspection and Host tests do not establish autonomous native presentation;
the next actual Linux acceptance uses the existing display/VKMS path.

Markdown Stress opts in with explicit CPU painting and
`--content-region=1048576` or `--content-region=4194304`. An authored action
selects the paragraph and full budget before first layout. Bake and ordinary
native/web controls retain synchronous `MarkdownStress`; this runtime uses
`NativeMarkdownStress`. The trial's independently sized 400px region leaves
typing and controls outside. A reproduced delayed default mixed launch returned
`InvalidTextMetrics(298)` and permanently refused its registration. Bare,
malformed and duplicate trial arguments now refuse before boot. The existing
ordinary mixed/code nonfinite-width failure was preserved separately; §8.20
records its font-admission repair. Selecting the paragraph workload did not fix it.

Two source-inclusive tests on macOS park the actual ordered continuation before
generation and let font work finish first. They distinguish a ready loading
label from the requested document, exercise typing, then require full unsplit
1MiB/4MiB paragraph source and prepared ink after release, with zero giant UI
measurement calls. Raw source sizes are 1,048,531 and 4,194,181 bytes; paragraph
sizes are 1,048,499 and 4,194,149 bytes. Both pass. Seven generated-executable
argument checks also pass. The canonical controller package records 272 passes
and one existing GPU ignore; strict Clippy passes. An older 1MiB complete CPU
pixel oracle predates canonical query-helper consolidation, as its archived
source identity states. No actual Linux, capped-memory or latency conclusion
follows from these Mac-hosted tests.

While later source parsing is pending, the authored loading branch can replace
the old document with “Generating…”. Retaining the DataSource Value does not
mean retaining its displayed pixels. Width-only reflow does retain the accepted
picture. Fixed DPR, explicit CPU and the read-only paragraph consumer are trial
constraints. UI source/range capture, one native Spec source copy, Value/Rc
conversion, shell layout, visible glyph raster work and destruction remain.
The 64KiB link limit is per paragraph; displayed A, kernel-accepted B and working
C may coexist. Count/source/index limits and viewport surfaces are separate
categories, not a total resident-memory bound.

Frozen sources, behavioral failures, private harnesses and independent reviews
are retained in `target/content-region-controller-validation/`; integration
checks are in `target/content-region-controller-integration/`. Actual Linux
cold-to-correct publication, continuous input/resize, measured memory and Apple
consumer integration remain in progress. No physical 120Hz claim is made.

### 8.20 Refuse unusable font metrics before shaping, 2026-09-17

Leibniz implements the admission repair; Tuft integrates and checks the ordinary
Markdown consumer. The installed GB18030Bitmap face has `bhed` but no `head`.
Pinned skrifa reports zero units per em; cosmic-text 0.19.0 previously admitted
that face and divided glyph advances by zero before wrapping. The ordinary
16KiB mixed Markdown entry also reproduced a paint-time `SubpixelBin` overflow.

The existing Cargo patch mechanism selects the complete pinned cosmic-text
0.19.0 archive with one three-line change: `Font::new` returns `None` for zero
units per em. Existing failed-admission caching and fallback select another
eligible face. There is no font-name blacklist, width clamp, dependency upgrade
or per-shape catalog mutation. The archive identity, licenses and exact source
delta are recorded in `vendor/cosmic-text/EXACT-PATCHES.md`.

Portable missing/zero-head fixtures fail against stock admission and pass after
the repair. Tests cover fallback, finite ordinary code at three widths,
unclamped overwide words, and real worker transfer with matching geometry,
glyphs and CPU pixels. The valid-font stock/treatment comparison is byte-identical
for 12,677 bytes of glyph/geometry signatures and 1,280,000 bytes of RGBA.
The private scope records 95 distinct passes. Integrated Linux-package tests
record 279 passes and one existing GPU ignore; strict Clippy and the Markdown
consumer build pass.

The same ordinary 16KiB mixed entry, CPU painter and 1024×768 headless viewport
changed from exit101 with the overflow to exit0 with 276 nodes and no boot error.
Both runs execute the Linux crate on macOS with the installed font catalog.
They establish a correctness repair, not an actual Linux display or latency
comparison. The separate ac86627 Linux cold-publication capture retains its
stock dependency identity. Mixed/code content-region support, giant-memory
bounds and physical 120Hz are not established by this increment.

Sources, stock failures and scoped checks are in `target/font-admission-validation/`;
integrated checks and the ordinary-entry before/after are in
`target/font-admission-integration/`.

### 8.21 Arrange collection and app foundation, 2026-09-17

Leibniz implements the collection seam and Newton implements the gallery
consumer; Zeno reviews both. The windowed Arrange List has an authored ID and
`reorderdrop(item, before)` handler. A dedicated non-button grip uses
`reorderFor`; manual Move disables it, and eager/manual controls remain available.
The typed terminal event preserves string keys and an optional destination.
Host physical delivery is a separate implementation, not implied by authoring
or synthesized event tests.

The Runner retains one preview descriptor and the existing source interaction
pin, alongside at most the existing focus pin. It does not reorder the resource
or export all keys during preview. Gap lookup and current-measurement proof are
logarithmic; absolute Translate targets touch mounted wrappers. The additional
minimum-epoch index occupies 512KiB for 25,000 entries. Source exclusion precedes
right-biased zero-height boundary certification. An unproved latest sample keeps
the displayed preview but clears permission to drop at the previous gap.

Terminal admission consumes action eligibility before dispatch. A still-owned
source pin survives the structural move and replacement of its grip; transferring
the pin away prevents an old finish from clearing a successor. The host must
capture presentation, apply the action and coherent layout, rebase wrapper
coordinates, end motion and release the pin when safe. These common operations
do not themselves implement that physical ordering or edge scrolling.

The app calls synchronous `galleryReorder` with the latest structural revision.
It checks stable source/destination identities and manual-move exclusion before
mutation. Self, current-next and already-at-end moves are unchanged; a real move
reserves the next revision before changing order. Refusals return a transient
notice while preserving the model and cached rows. Other structural mutations
also refuse exhaustion before consuming tokens or state. Terminal identity
lookup and rebuilding changed rows remain O(N); preview does no per-pointer
data query. The existing 26-field state and eight-field Photo wire are unchanged.

The common freeze records 497 passing tests, including 25 new cases. Retained
behavioral failures cover source-excluded zero gaps, stale final certification
and premature source retirement after grip replacement. MAIN integration passes
83 focused tests, strict all-target Clippy for all three hosts and Web wasm checks.
The app passes 60 tests, including 19 new cases, both privately and on MAIN;
model, data and old-app/new-common behavioral failures are retained. Scoped
formatting, caps and strict app Clippy pass. Evidence is in
`target/arrange-common-validation/` and `target/gallery-arrange-validation/`.

Physical Web/Apple/Linux adapters, same-clip paint and hit elevation, actual
nested-port edge scrolling and continuous-position terminal rebase remain work
in progress. No physical drag, velocity-continuous neighbor rebase, latency or
120Hz result follows from this foundation.

### 8.22 Actual Linux cold publication: partial acceptance, 2026-09-17

Carson runs the captured ac86627 controller and diagnostic observer on ARM Linux,
with stock cosmic-text 0.19, VKMS 1024×768, CPU painting at scale 1 and the fixed
DejaVu/system font catalog. Both independent synchronous references pass under
the unchanged 2,684,354,560-byte address-space cap and 60-second process guard.
The native coverage certificate verifies omitted ASCII spaces only at actual
adjacent layout-run boundaries. The 1MiB/4MiB references retain respectively
1,048,499/4,194,149 canonical bytes and 13,981/55,923 lines. Earlier guard failures
remain failed; these new certificates do not rewrite their verdicts.

The 1MiB candidate includes real cold source generation and parsing. Pending
frame 5 and accepted frame 15 share input sequence 18, proving publication
without another input, agent query or settle command. Continued typing appears
in frame 17. All 26 source, glyph, geometry, font and boundary fields match the
independent reference, and the accepted content RGB crop is byte-identical.
Cumulative giant UI shape calls and UI ink-index builds remain zero. The worker
records one giant shape and two completed layouts/index builds; this is not a
one-index claim. Observed running and pending work each peak at one.

The complete candidate cell nevertheless fails: RFB wheel input arrives, but
the reported document offset remains zero until the unchanged deadline. Cleanup
retires the owned process; no successful scrolled frame or normal shutdown is
invented. The later regression in §8.24 distinguishes stale replay metadata from
actual scroll state: this trace alone does not prove the pixels stayed still.
The initial shell-only extent diagnosis was incomplete because accepted region
layout does reach the live kernel before native painting succeeds. Sampled
candidate VmPeak is 1,282,868KiB and VmHWM/RSS
589,724KiB for this one instrumented cell, not a general memory bound.

The 4MiB candidate fails loopback-port preflight before native launch. It has no
worker completion or memory result. The synchronous 4MiB reference's memory is
not a substitute. No unowned listener was killed, cap raised or automatic retry
performed. Full candidate acceptance, continuous resizing, latency and physical
120Hz remain unproved.

Exact sources and paired binaries are in
`target/linux-content-region-native-observer-v2/` and
`target/linux-content-region-native-execution-v3/`. Raw references, partial
candidate evidence, both failures and cleanup receipts are retained in
`target/linux-content-region-native-runtime-v2/` (manifest
`662a2b82c9755a2bf5a209d0f35728b1fca8d4b3f9a89a5ca8582cbe70ab3445`).

### 8.23 AppKit contained paragraph and publication repairs, 2026-09-17

Darwin implements the opt-in AppKit consumer; Zeno independently reviews its
source and two repair deltas. `EXACT_CONTENT_REGION=1048576` or `4194304` selects
the full paragraph before first layout. Other values and region use on iOS
refuse. The generated Apple runtime uses NativeMarkdownStress even without the
flag; bake retains MarkdownStress. The flag controls contained layout and paint,
not whether source parsing uses the native continuation. Ordinary layout and
Value construction remain on the UI owner.

One serial CoreText service prepares immutable text, width layout, numeric hit
metadata and profiled viewport pixels. Reset retires epochs without releasing
an occupied worker early or joining it on the UI thread. Main-thread adoption
checks request, generation, publication and actual native phase before exposing
pixels and matching metadata. Source/font capture still performs UI work, and
source strings, CoreText/font heaps and other opaque storage prevent a strict
total-memory claim. Per-paragraph selection/copy is not general Markdown parity.

Review found and fixed accepted-A → pending-B → latest-A supersession: a cache
hit now retires B's desired state, and delivery rechecks the actual viewport,
selection and background even before another display pass. Cleanup is serial
qualified so reentrant successor C survives B's completion. Effective appearance
is fixed per registration; change terminally refuses pixels, hits and pending
answers until reset, rather than pairing an old foreground with a new background.

Selection-aware pixel invalidation initially cleared its own drag anchor. The
second repair retains only the exact generation/publication/artifact and
geometry/palette-qualified interaction while highlight pixels are pending.
Fresh hits remain blocked on hidden pixels. Mouseup, reset, refusal, source or
phase changes retire that anchor. Production NSEvent handlers and select methods
exercise begin → drag → selected-raster acceptance → further drag, including
begin over an existing selection and a blocked highlight worker.

Recorded Rust coverage is 144 passing tests, also passing on the integrated tree.
Combined-tree ExactKit typecheck, strict host/consumer all-targets Clippy, consumer
check and scoped formatting pass. Consumer Clippy first found a redundant factory
closure in the generated entry; replacing it with the same default function fixes
that lint, and the original failure is retained. Revision1 retains meaningful
62-assertion/16-failure and successor 69/1 failures, then 69/0 controller checks
and 65/0 native-surface checks. Revision2 retains selection RED 107/10 and GREEN
125/0, plus the unchanged 20,821-assertion strict Swift6 actual-engine suite and
complete ExactKit Swift5 typecheck. The native fixtures use real NSWindow,
RegionSurface and CoreText work with documented runtime/presenter composition
doubles; they are not full-package XCTest or physical-input acceptance.

Earlier optimized native binaries exercised full 1MiB/4MiB source identity,
functional typing, resize and plain wheel. They predate the final hit delta and
both publication repairs. A fresh optimized full-host build at 6663356 then
passes a bounded 1MiB smoke with executable SHA256
`e785870236c16932d618501b4693967228f91e6b2e9982d2686cf6a491cdbad5`.
It independently checks the complete 1,048,531-byte source and 1,048,499-byte
unsplit paragraph against published source identities, typing during pending
and accepted states, width changes, and plain-wheel movement whose raster offset
matches the actual clip. All 71 recorded commands finish, the binary remains
unchanged, and the owned driver/app exit. This is an actual AppKit functional
smoke; state polling can pump the existing agent boundary. It does not repeat
4MiB or the deterministic worker-barrier fixtures. A separate six-command default
launch of the same binary also passes Unicode input/echo with region registration
absent and the original mixed 16KiB baked document. It does not force fresh parsing.

External WindowServer capture was refused; native view-cache screenshots do not
replace that missing live-pixel comparison. Phased synthetic wheel and real
trackpad behavior, autonomous full-host progress, continuous-resize latency,
UIKit and physical 120Hz remain unproved. Viewport mismatch may hide the old image
while preparing replacement pixels. Current build and smoke evidence lives under
MAIN's `target/apple-content-region-integrated-6663356/` and the separate
`target/apple-content-region-default-6663356/` appendix; copied artifact hashes
match the immutable private captures.

The immutable original and two revisions are under
`target/apple-content-region-validation/darwin-{final,revision1,revision2}-20260917/`.
Revision2's complete 29-file patch is
`131e0183ae867b87321768d66249d92ed8e8c5a9c005d909f82e1b83848c5e50`;
integration preserves MAIN's Arrange event mapping and removes one trailing blank
line, plus the equivalent generated factory correction. Combined-tree validation is recorded separately in
`target/apple-content-region-integration/`.

### 8.24 Keep Linux scroll geometry with the painted publication, 2026-09-17

Carson reproduces four failures with naturally overflowing text in the real CPU
presenter. Wheel input changes state and pixels while replayed PaintedBox scroll
metadata stays zero. Independently, a ready but unpainted shorter document clamps
the still-visible taller document; the reverse permits scrolling against an
unpainted taller document. A recreated key can also borrow replacement geometry
before its pixels are published. The original native failed cell remains failed.

Each retained picture now owns numeric scroll limits and overflow axes keyed by
full NodeKey. Wheel and clamp use that picture's region incarnation; missing or
recycled region keys cannot fall back to a live candidate. Replay uses one
clamped offset for text queries, drawing and hit metadata. Successful painting
adopts those offsets; failed painting retains the previous picture and limits.
Existing collection logical-end limits are captured rather than replaced with
mounted-row bounds. Ordinary opt-out behavior is unchanged.

The four meaningful behavioral REDs become GREEN. The frozen package run records
283 passing tests and one existing GPU ignore on macOS ARM64, with strict
all-targets Clippy, formatting and caps passing. The same 283/1 package result,
strict Clippy, scoped formatting, caps and boot checks pass on the integrated tree.
These are CPU correctness tests,
not a fixed actual-Linux display or latency result. A separate exploratory region
collection end-follow fixture stopped after three attempts; its two authoring
errors and missing-follow-end expectation are retained outside the patch, with
no new end-follow claim or ignored test.

The six exact source files, full patch, raw failures and validation are archived
under `target/linux-region-scroll-validation/freeze-v1/`, source manifest
`f6b1ee333ee162690a0b4ee3d9e7b289f29a95eefca1fde511d1df8fae20ede3`.
Fresh Linux display evidence must retain its own source, reference and binary
identities and the existing memory/time guards.

### 8.25 Web Arrange contact, rebase and return ownership, 2026-09-17

Epicurus implements the browser adapter over the existing collection preview
and atomic gallery drop. Recognition catches the current wrapper Translate,
uses actual nested-port geometry and retains the authored grip rather than its
label child. Runtime, generational keys, preview token and geometry are checked
before motion time or terminal action. An unproved final sample cancels rather
than dispatching an earlier gap. Only the final typed List event reaches app data.

Mounted wrappers are sampled before the synchronous action, held through layout
and rebased from old to new displayed origins. Source velocity is preserved;
neighbours restart at zero velocity, so this is positional continuity, not a
continuous-velocity claim. The source stays inside the existing List clip and
uses its existing interaction pin through return settlement. Edge scrolling
uses 720 CSS pixels/second with at most 32ms elapsed-time catch-up, consumes
actual clamped scroll movement, and stops scheduling at the end or cancellation.
The logical extent excludes overflow caused solely by the preview transform.

Independent review found that finishing a returning pinned-only source before
replacement admission could unmount it. A same-row grip → wrapper → grip transfer
now preserves the pin without a null interval. A further prethreshold probe found
that tap or horizontal refusal still finished the original return too early.
The final repair leaves that terminal owner intact until vertical recognition;
pointerup, pointercancel or horizontal refusal only retire provisional listeners.
Natural settlement remains authoritative, and a stale returning descriptor cannot
reacquire a successor. No extra pin, timer, animation loop or owner registry is added.

The broader v4 real-Wasm headless CDP drive passes 17 checks on 25,000 logical rows
with at most nine mounted. Its 28 common-wrapper position comparisons have maximum
dy 0.0000152588 CSS pixels; this is sampled geometry, not an all-frame proof.
It covers edge clamp/idle, held-source deletion and final pin cleanup. Final v5
actual-Wasm replays pass recognized regrab 8/8, prethreshold tap 9/9 and horizontal
abort 9/9. Their meaningful earlier failures remain recorded. The same Wasm and
plan are used; final JavaScript carries the ownership repair. The broader v4 drive
was not repeated after that narrow delta.

On the integrated tree, all 119 Web Rust tests pass with three existing opt-in
ignores; all 61 DOM tests pass with 206 assertions. DOM fixtures use mock Rust
replies and remain separate from the actual-Wasm drives. Strict all-targets Clippy,
Wasm check, scoped formatting, caps and boot pass. An initial unscoped Bun filter
selected archived test copies and failed during Chrome setup; the failure is
preserved and the explicit single-file run passes. No production change was made
for that setup correction.

Frozen v4/v5 sources, dependencies, raw evidence and independent reviews are under
`target/arrange-web-validation/`. Final source manifest is
`09760f3f996767b04646efa0d8ce53d07ae7338e3b6de5749e817e8c376b7535`.
This is bounded browser functional evidence, not native Arrange parity, complete
25k traversals, quiet latency or physical 120Hz presentation. Public preview
serving remains unchanged.

### 8.26 Linux Arrange contact and owned edge feedback, 2026-09-17

Leibniz implements the Linux adapter using the existing primary contact owner,
common collection preview and single interaction pin. Recognition samples the
current source Translate; movement uses parent coordinates and actual clamped
List scroll. Runtime, generational binding, preview/hold tokens and current
geometry are checked before clock or terminal action. Unproved final geometry
cancels. Unsupported transforms, mapping reflow and content-region picture
replay are refused rather than borrowing stale collection facts.

The final typed action runs while the source remains held. Surviving mounted
wrappers rebase from their sampled viewport positions after synchronous layout;
source velocity is retained and neighbours restart with zero velocity. This is
positional continuity, not velocity continuity. The source paints last within
its own List clip, and reverse painted-box hit testing follows that same order.
The existing pin survives return settlement, a removed grip and same-row
replacement admission. No additional pin, portal, executor or timer is added.

Edge scrolling uses the existing display pump, at most 50ms elapsed time per
step and 300px/second, and compensates from actual clamped movement. Review found
that an ordinary List scroll handler could cancel the first owned step: native
offset/sequence had advanced, but contact retirement still saw old Runner facts.
The repaired owned path dispatches the handler normally, then prioritizes that
List in the existing two-pass feedback budget before retirement. External scroll
keeps its stale-first order. A harmless counter handler preserves the hold and
stationary viewport position through one terminal drop; deletion and width
reflow still retire before malformed late delivery. No guard-suppression flag or
unbounded refinement was introduced.

Private revision2 records 297 passing tests and one existing native-GPU ignore,
including eighteen new Arrange tests. The integrated tree also preserves §8.24's
four painted-scroll regressions and passes **301 tests, one GPU ignore**, strict
all-targets Clippy, scoped formatting, caps and boot. CPU tests cover paint/hit
priority, clipping, held typing, C0 samples, edge compensation, stale facts, pin
transfer and idle shutdown. The initial integrated build missed the shared
`effective_overflow` import after combining the two independently frozen changes;
that compile failure is retained, and restoring the import requires no behavior
change. The harmless-handler behavioral RED and the intermediate test's unproved
edge-gap refusal are also preserved.

Frozen eleven-file revision2 source identity is
`c4f7fbf6a0bdc38043e07b513386846f569e0bb7c42923f74adfe6d4c463dd82`.
Exact source snapshots, raw checks, independent reviews and the two composite
integration files are under `target/arrange-linux-validation/`. Tests ran on
macOS ARM64 with the native Rust Presenter and CPU painter. Actual Linux
display/stdio input, GPU order, full 25k traversal, timing and physical 120Hz remain
separate validation; no current native display claim is inferred from these tests.

A subsequent actual Linux run at1f48f28 passes all eighteen focused tests and
the gallery build, then fails the first100-row stdio cell: typing is acknowledged
during an admitted drag, but the next hold reports no contact. No drop or C0
assertion was reached, and no retry replaces the failure. The source trace finds
that unchanged `when` frames prevent collection body memoization, ending the
preview on unrelated draft writes. Exact binaries, raw replies, images and terminal ownership are
retained under `target/arrange-linux-native-1f48f28/`; this is synthesized
Presenter contact, not physical evdev delivery or a successful25k traversal.

The focused repair permits the existing body dependency memo through enclosing
frames only when every item, match binding, region and row-local state field is
absent. Contextual frames still use normal evaluation; no Presenter retirement
guard is bypassed. Common and native regressions first lose the preview/hold on
unrelated typing. A separate harness using the exact gallery plan and real data
source proves unchanged list-to-root geometry, scroll and presentation before
that failure. After the repair it retains the same row epochs, rows allocation,
hold and pin, then performs exactly one reorder and releases the pin on settlement.
Changed dependencies, match bindings and destroyed arms still invalidate.

Private validation records 74 passing tests: 20 common, 42 existing collection,
11 native Arrange and one actual-gallery harness, plus strict scoped Clippy.
Four tracked regressions and the separate gallery proof are new. Preserved
fixture syntax and revision-field mistakes are identified separately from the
behavioral REDs. The integrated tree repeats the 73 common/collection/native
tests and passes strict all-targets Clippy for runner, Contract and Linux, plus
scoped formatting, caps and boot. These tests ran on macOS with the Rust CPU Presenter, not in a
new Linux VM cell. The original Linux failure remains. Frozen sources and
raw results are under `target/arrange-empty-frames-validation/freeze-v1/`.

The first 898dd74 Linux gallery capture reused an older optimized Runner from
the warm target, although its test-profile Runner was fresh. That failed replay
cannot establish the repair's outcome. A byte-preserving source-mtime refresh
and one rebuild recompile both Runner profiles and downstream host/gallery;
the retained ELF changes from `6008d3a4…` to `77eaa2e3…`, with the same app plan.

One unchanged 100-row driver replay on the coherent binary retains contact
through typing, swaps exactly the first two items with revision+1, and passes
nine same-clock positional-continuity comparisons. It later attempts a second
catch at y=137.5 above the List's clipped viewport top=250 and fails. This is an
invalid driver coordinate, not evidence of another product contact-loss bug.
The overall run remains FAIL; cancel/resize checks are unexecuted, with no retry.
Build linkage, raw partial success/failure and terminal cleanup are preserved at
`target/arrange-linux-native-898dd74/coherent-rebuild-v2/`. Complete 25k traversal,
physical input and frame timing remain unproved.

### 8.27 Actual Linux scroll repair and remaining 4MiB allocation failure, 2026-09-17

Carson captures merged d6b3431 with the unchanged fourteen-path diagnostic
observer, current vendored cosmic-text and the same 39 faces from thirteen font
files. Each fresh reference/candidate keeps the 2,684,354,560-byte address-space
cap and 60s deadline. This is instrumented, nonquiet CPU/VKMS plus RFB functional
validation; no physical input, frame-cadence or comparative speed claim follows.

The full 1MiB display cell passes. Pending frame5 and accepted frame15 both carry
input18, with no intervening input: the display loop delivers the worker result
autonomously. Continued typing reaches frame18/input21; wheel reaches frame20/
input25 and scroll0→40. The complete canonical paragraph is 1,048,499 bytes,
978,599 glyphs and 13,981 lines. All26 available final proof fields match the fresh
reference, and the accepted 1,228,800-byte RGB content crop matches exactly. The
scrolled strip [16,259,480,328] equals the previous [16,299,480,328] strip across
472,320 bytes and differs from the unshifted strip. This proves overlapping visible
content movement, not newly exposed bottom pixels. UI giant shape/index counters
remain zero; worker shape1/index2 and completed layouts2 are recorded. Observed
running, pending and completed occupancy peaks are each one.

The 4MiB synchronous reference passes, but the display candidate **fails before
publication**. Its one completed worker layout has all24 available fields equal
to the reference: 4,194,149 canonical bytes, 3,914,539 glyphs and 55,923 lines.
Further worker work then aborts with `memory allocation of 11264 bytes failed`.
Exit is SIGABRT/−6 without a cleanup signal; every frame remains non-current and
published ticket remains null. Sampled VmPeak/VmSize reaches exactly 2,621,440KiB,
the retained address-space cap; sampled RSS/HWM is 1,735,924KiB. The 1MiB candidate
peaks at VmPeak1,282,884KiB and RSS/HWM589,348KiB. These samples and the failed
allocation identify a capacity limit, not its exact allocation site or all live
memory categories. Correct pending input and one worker answer are not complete
4MiB success. No cell was retried or workload reduced.

Candidate ELF is `4486525afcff99e96029641b9d2b343624747d4d314abc74b7fb7503fa5e9ea5`;
reference ELF is `e89a3f53307e89eef4282571a97c0d225992bd2766469fec78567a6f962b4f0a`.
The archive at `target/linux-region-scroll-native-execution-d6b3431-v1/` has
1,497 verified entries/120,309,593 bytes, manifest
`d98d5cd5d97894d16ef31b2f220642185f4479da2e9988235d54844f5148f89c`.
Raw events, exact images, memory samples, source/link identities and owned-group
cleanup are retained. The first oracle build stopped at the disk guard; a recorded
downloaded-OS-package-cache cleanup preceded its one successful build retry, with
candidate bytes unchanged. Original setup failures and §8.22's failed cells keep
their original verdicts. Independent review recomputes the proofs and pixel crops.

The remaining work is 4MiB allocation ownership, repeated same-source reflow and
UI publication cost, followed by actual continuous-resize/input measurements.
This single 1MiB run's roughly1.95s cold-publication envelope is not an 8.33ms
frame result or a robust latency distribution.

### 8.28 Share identical worker layout payloads across exact requests, 2026-09-17

A real kernel regression reproduces two requests for the same paragraph:
`(Definite(600), MinContent)` followed by `(Definite(600), MaxContent)`. Linux
wraps both at the same width, but previously built two complete layout vectors
and two ink indexes. The regression preserves both request identities and their
sampled pixels; the new implementation builds one layout and one index.

Each prepared source has one weak slot keyed by exact width and paint-scale
bits. Its immutable backing contains layout lines, baselines, metrics and the
numeric ink index. Adoption retains that backing in the paragraph's local ink
cache, so the slot remains reusable after the completed worker result is consumed.
Every answer still has a fresh private job and the complete two-axis request;
sharing never admits a stale answer. The backing retains no source, job, ticket
or UI catalog, and the weak slot retains no payload after the last owner releases
it. Width/scale changes replace the slot, with no visited-width history.

Construction and indexing remain outside the slot lock. Failed and intrinsic
answers cannot seed it. Resetting one paragraph's ink cache leaves siblings
intact. Capacity diagnostics deduplicate source, layout, baseline and index
allocations separately across current, handoff and retiring wrappers. These are
accessible vector capacities, excluding Arc headers, private font storage,
allocator reservation and RSS; wrapper counts remain separate.

The frozen nine-file change has 92 passing text tests, including four new
regressions for actual offers, adopted-owner reuse, stale siblings, slot misses,
independent reset and allocation lifetime. Strict all-targets Clippy passes.
The latest full-library invocation is **208 pass, one unchanged image-test
failure, one GPU ignore**, not a whole-package pass. The image test assumes zero
reserved delivery cells immediately after enabling workers, although admission
can reserve a cell before its held completion hook. Its failure, the initial
compile errors and updated ownership expectations are retained without image
edits or retries. Evidence is under `target/shared-width-validation/`, source
manifest `947af46afa2dfc94ef319091ac1689be73785c9986bbbe21623b8b5cfd74f460`.

The approximately610MiB duplicate layout/index payload observed in the earlier
4MiB fixture motivated this repair. The source tests alone did not establish
native peak-memory savings or complete display admission; the subsequent native
replay is recorded below. Repeated resizing and physical120Hz remain unproved.

### 8.29 Actual Linux shared-layout publication within the retained cap, 2026-09-17

The ef12f06 replay runs fresh synchronous references and display candidates for
both nominal1MiB and4MiB paragraphs. All four cells pass under the unchanged
2.5GiB address-space cap,60s process guard,1024x768 viewport,CPU painter and
fixed scale1. The font fixture remains39 faces from13 sources/107,296,644 bytes.
The4MiB candidate previously aborted before publication on d6 (§8.27); that
negative result and its exact sources remain intact.

Each candidate prepares and adopts two distinct private jobs with different
height offers. One immutable layout and numeric ink index serve both jobs;
there is one cache hit, one actual shape, one layout and one index construction.
Private job, full request, source, catalog and final publication identities remain
checked independently of the shared backing. Cumulative giant UI shape and ink
index construction counters stay zero.

The full raw sources are1,048,531 and4,194,181 bytes; their unsplit paragraphs
are1,048,499 and4,194,149 bytes. Each final proof matches the fresh independent
reference's full-source, glyph, ordered coverage, geometry and font oracles.
The larger paragraph contains55,923 lines and3,914,539 glyphs. Publication
occurs without intervening input after the recorded idle frame. Continued
typing is then visible, followed by actual wheel scrolling0→40px.
The1,228,800-byte accepted RGB content crop matches its reference exactly.
A472,320-byte overlapping strip shifts by exactly40px in each candidate;
newly exposed pixels and whole-window presentation are outside that comparison.

Sampled VmPeak/RSS-HWM are1,148,028/449,028KiB for1MiB and
2,303,612/1,384,404KiB for4MiB. The4MiB VmPeak is317,828KiB below this run's
address-space cap. Accessible K/shape/line/baseline/index capacity checkpoints
deduplicate backing identities; they are not RSS, complete allocations or a
general peak-memory bound. In-progress work, font storage and allocator overhead
remain outside that ledger. The earlier failed process and this successful one
are not a matched peak-allocation experiment at an identical completion stage.

Successful display processes are deliberately stopped only after the observations
complete; their recorded SIGTERM exits are intended cleanup, not hidden crashes.
References exit0, all owned groups are absent, and no cell is retried. These are
single instrumented functional cells, labeled nonquiet by the retained harness,
not latency distributions, continuous-reflow or physical120Hz evidence.
The first outer coordinator stops between pairs because its supplemental
preflight checks both ports, including the just-used1MiB port5937 in TIME_WAIT.
That failure is preserved. The narrow outer continuation checks only the
upcoming, unchanged4MiB port5938; no receiver edit, port substitution, socket
option change or completed native-cell rerun is used.
Build and runtime evidence is retained under
`target/shared-width-native-execution-ef12f06-v1/`; the candidate binary is
`5bc095b6098ab068468f9e4ad17864a913d3a5119c8865225a094ff2afcd6a4d`.
The runtime archive contains80 verified entries/29,048,549 bytes, manifest
`ce4e93f29b5c1c76d1c5516dfaf8ef7d8b080d2fe9790c7976066c49419ba082`.
The final build/runtime/report archive has126 entries/92,334,840 bytes, manifest
`ed06f46dfb571aad55b7aa4b2c7f43ef986ef4a5e14ae1c2946392ae7dc3ac22`.

### 8.30 Immutable Messages row reuse, 2026-09-17

An explicit `ReusableMessagesStress` data source retains one latest immutable
history result. It preserves the stateless source's complete values and bytes,
including changed-tail rollback, manual pages, local echo and invalid-input
refusal. With 10,000 supplied rows and 32 revised bodies, 9,968 record owners are
reused; unchanged string fields within the 32 replacement records also survive.
Accepted older results remain immutable, and neither old revisions nor visited
pages are retained after their last owner releases them.

Same arguments reuse the complete result. Other same-range requests copy an
O(N) vector of row handles and use the existing generator's temporary 100-row
tail page for changed content. This is not only 32 temporary allocations or
O(batch) total settlement. Cold/count/range changes use the original generator.
The real Contract test still observes 10,000 key evaluations per tick; fresh
answer validation, reconciliation and layout remain synchronous. Typing and
width changes issue no history query in either source.

The original source produces 9 behavioral failures and 4 passes in the 13-test
reuse suite; the candidate passes all 13. Integrated validation runs the complete
data package: 26 pass, one existing opt-in timing test ignored, strict all-targets
Clippy pass. Frozen three-file source manifest is
`0151ac1924345c8189387768caffc2b1b51a295ccb45a5451c42da45a50ec908`,
with raw RED/GREEN evidence under `target/messages-row-reuse-validation/`.
All bake, Web and native entry selections remain the stateless control.
No host activation, measured speedup, worker placement or 120 Hz result follows
from these allocation and correctness tests.

### 8.31 Share live Apple worker layouts across height offers, 2026-09-17

Three requests with the same captured source and definite width, but different
height offers, now share one immutable CoreText layout. The existing shaper
depends on width, not height. Each result still carries a fresh artifact and
request ID; the kernel's full-offer and publication checks remain unchanged.
Lookup requires identical source object, source ID, generation and offered
width, and searches only the existing live request table. All aliases count
against its 64-entry cap. Intrinsic offers do not seed reuse, and retired or
paint-only owners are not a searchable history.

The serial queue, latest-request mailbox, reset/close behavior and original
admission barrier are unchanged. A separate construction hook distinguishes
actual shaping from reuse. Retiring one artifact preserves its surviving alias;
retiring the last table binding prevents later lookup even if an older paint
owner legitimately remains alive. CoreText objects stay confined to the worker.

The preserved baseline executes nine methods/64 assertions with 12 behavioral
failures, including three constructions where one is expected. The candidate
passes 16 methods/20,885 assertions under strict optimized Swift 6. Nine new
methods cover identity misses, admission, reset, last-owner release and alias
painting; seven existing methods retain ordinary TextEngine geometry, exact
fractional-viewport pixels and Unicode hit oracles. These are actual production
worker sources with an assertion shim, not full-package XCTest or a GUI drive.
An earlier test-only compile failure is retained separately.

The four-file patch is
`36f5a79c035a56e3b032fc8783d2add4624cdc697f5f837f4c71ec6560de7519`;
84 copied evidence files are under `target/apple-region-width-reuse-validation/`.
The earlier carrier did not record enough source identity to attribute its
suspected duplicate spans. Those tests alone establish reuse, not a measured
giant-paragraph speedup, memory bound or physical 120 Hz result.

A separate one-pair backend experiment now exercises the complete saved
1,048,499-byte paragraph (999,569 UTF16 units) through actual `RegionService`.
Both arms retain the same source object and three fresh artifact IDs, at offered
width 940 and height offers MinContent, MaxContent and 400. Three off-main
admissions produce three constructions before the change and one afterward.
All six full-text copies and streamed geometry digests match: 8,155 lines,
212,030 height, natural measured width 945. The offered box remains 940; no
fixture or geometry expectation is retuned.

The final artifact's 940×160 bitmap at scale 1 and internal scroll 0.375 matches
in every one of its 601,600 RGBA bytes, with identical ICC profile, fixed
selection and sampled hit metadata. This uses a source-derived fixed style and
palette; it excludes heading, parsing, controller and WindowServer presentation.
Source/font capture takes 12.088/15.752 ms before/after. Submit-to-UI delivery
takes 2737.366/2674.675 ms for the first offer, 2642.066/8.749 ms for the second,
and 2654.142/1.985 ms for the third. These are one pair's wall intervals, not
isolated CoreText CPU or a latency distribution. Full geometry checking occurs
between offers, outside those intervals. First construction remains expensive.

Both native helpers and the exact comparator exit successfully, with recorded
processes/groups retired. The initial launch attempt remains a setup failure
with zero helpers started: copied binaries lacked execute permission. Separate
exact-byte executable copies correct only that permission; no rebuild or source
change is involved. The earlier three full-host carrier failures remain
separate. This backend pair does not establish native interaction, novel-width
reflow or physical 120 Hz. Its 23 artifacts/1,230,199 bytes are under
`target/apple-region-width-reuse-1m-runtime/pair-2/`, manifest
`d1dcc3cf3df9bc1b71b6d0e2428d25d74de7faa092d4adc1c4322778cef58ff4`.

### 8.32 Retain the height index when ordered keys are identical, 2026-09-17

`HeightIndex::replace_keys` now returns immediately when the incoming ordered
keys equal the already validated order. This retains the positions map, row
measurements, sum tree and generations instead of reconstructing them for a
fresh immutable list with unchanged membership. Changed order, insertion,
deletion and duplicate refusal still use the existing transactional path.
Caller content/width invalidation and row realization remain unchanged.

The behavioral RED observes an unnecessary rebuild. Its regression now retains
allocations and measurements for empty, three-row and 25,000-row orders. Controls
cover structural changes, stale measurements, and equal keys with changed text:
all keys are evaluated, changed content lays out, and old feedback is refused.
The private collection suite passes 45 tests; integration passes all 64 Runner
library tests plus the 13 Messages reuse tests, with scoped strict Clippy.
The three-file source manifest is
`18b037d1c95731f6fb2f6425f6b7c6795147ed9cacbc062c4155ac42f0236eda`;
raw evidence is under `target/collection-identical-keys-validation/`.

Equality still compares O(N) strings. The caller still evaluates keys, allocates
key strings, checks uniqueness and validates fresh values; this is not constant
time settlement. The source tests establish avoided reconstruction, with no
host timing result or change to the pinned Linux Arrange replay.

### 8.33 Ordinary-clock Web baselines and finite Storm interleaving, 2026-09-17

Six fresh browser cells on captured 898dd74 run three repetitions per app, with
fixed two-second idle, loaded and recovery windows. Messages supplies 10,000 rows
in windowed mode with 32 changed rows observed after positive revision. Storm
reconciles all 128 exact lane payloads. All 432 scheduled offers and 144 input/echo
observations complete without skips, errors or trace overflow. This is the
stateless Messages source before the later index fast path, not a measurement
of §§8.30–8.32. The browser is headless Chrome 153 on macOS/Apple M4.

Messages' fully scored loaded `exact_advance` calls have per-run medians
32.9, 24.3 and 31.9 ms, maximum 36.3 ms. All 23 exceed 8.33 ms; a late eighth call in
one repetition remains outside its fixed window. Idle medians are 0.1–0.6 ms.
The export span includes synchronous Rust settlement, not subsequent JavaScript
batch application or paint. It identifies work to reduce without attributing
the whole cost to data generation, keys or layout individually.
The observer does not wrap `exact_resize`, whose host entry also advances due
timers. These `exact_advance` samples therefore do not account for all timer
settlement during resize; comparing that export alone can miss work performed
by another entry point.

Storm's 128 fulfillments occupy 123.8–127.7 ms envelopes, with 109.8–113.8 ms inside
the direct Wasm calls. A small per-call median of 0.8 ms does not make the aggregate
free. Only six responses were held at release; later requests complete within
the released wave. No captured input overlaps these original envelopes, so
typing-under-completion-pressure remains NOT_ESTABLISHED for these three cells.
Actual rAF callback execution occurs between completions in two runs; the whole
wave cannot be described as one uninterrupted task. The HTTP glue has no explicit
completion drain budget or deliberate task yield; browser scheduling supplies
the observed opportunities to interleave work.

Scored rAF gaps are coarse even while idle. They are not physical presentation
intervals, and their cause is not isolated. Generic scrolling is observed, but
simultaneous resize and anchoring prevent a wheel-causality claim. These small
instrumented cohorts establish neither robust latency tails nor 120 Hz.
All six processes terminate normally. Raw data and independent review cover 57
artifacts/1,796,529 bytes under `target/web-normal-clock-cells/898dd74-v2/`,
manifest `08128e4890c0819c6c1b259f9a2540f1297fb30b0e01501bb610cf2081754217`.

A separate, predeclared three-cell Storm arm moves only external release from
500 to 400 ms, retaining the scheduled 500 ms typed offer, binary, 128 lanes,
two-second windows and input quotas. All three complete once. The entire input
offer clock bracket and captured input now lie inside each fulfillment envelope:
after 79/71/89 fulfillments and before the remaining 49/57/39. Offer-to-capture
bounds are 1.335–1.433, 1.377–1.465 and 1.402–1.454 ms. Capture-to-echo is 0.8 ms;
echo-to-following-rAF callback varies from 3.7 to 44.5 ms. These are three selected
samples, not latency tails or physical presentation measurements.

This closes the finite-burst input-overlap discriminator without changing
production scheduling. It does not establish sustained pressure or a general
fairness bound. All 384 exact wave fulfillments and 216 scheduled offers complete;
only six transports are held at release. The original three Storm overlap
results remain NOT_ESTABLISHED. Source/runtime identities and cleanup cover 32
artifacts/1,231,542 bytes under
`target/web-normal-clock-cells/898dd74-release400-v1/`, manifest
`f6dc90eb720586c81899b436b11c8834e7e275f07e44964682c9d5ae61449c02`.

### 8.34 Messages during real AppKit resize: three baseline repetitions, 2026-09-17

Three fresh processes use the unchanged d6 Messages binary `f75d9e98…` and
ordinary-clock observer, with the corrected V3 current-tail setup policy. The
10,000-row, 32-changed-row windowed workload stays intact. All nine scored
idle/load/recovery phases establish genuine AppKit resize plus six typed inputs
and six direction-matching 40px wheels, with all 108 ACKs inside their edges.
The external resize input is synthetic OS input; typing/wheels use the existing
stdio carrier. This is not physical-human or display-frame timing.

All 23 complete loaded `Runtime.advance` intervals exceed 8.33 ms. Their per-run
medians are 17.619, 16.630 and 15.076 ms, maximum 27.717 ms; Swift apply medians
are 1.026, 1.041 and 0.948 ms. Advance includes bridge decoding, and these spans
do not isolate Rust query, validation, keys or layout. Loaded main-queue wait
maxima are 17.828–21.472 ms. Idle also includes a 30.476 ms ACK, so the small
cohort does not establish a general loaded-versus-idle latency bound.

Separate two-second command-free intervals contain 8/8/7 complete autonomous
timer→advance→apply chains before the next read. Mode coverage remains
NOTESTABLISHED despite 108–111 positive tracking samples per phase: passive
trace quotas overflow. The primary trace does not overflow. Setup preserves
the viewport but allows the tail's absolute scroll offset to change as rows
grow; no measured-edge reanchoring or polling is added. These are old-source
baselines, not measurements of the newer index or row-reuse changes.

The planned six-cell suite stops before any Storm native launch: a coordinator
preflight queries `/api/stats` on data port 4319 instead of control port 4320.
That preserved 404 is a setup error; three Storm cells remain unrun in this
archive. All recorded 23 PIDs and 16 groups retire and both ports are free.
The partial result covers 563 artifacts/208,058,993 bytes in
`target/normal-clock-responsiveness-native-d6-v3/`, manifest
`4f97f8d6ff60302af8eb8cf81c67f5b548231c060dce97c5e808869b8ac77472`.
The original failed V2 run is unchanged. No complete six-cell PASS or physical
120 Hz claim follows.

A separate corrected Storm continuation runs three fresh processes once, with
the same d6 binary `8a74d4f6…`, observer and V3 driver. Only the coordinator's
initial stats request moves to the correct control port. All three reconcile
128 exact lane payloads; the final fixture records 384 received/issued and zero
held, rejected, abandoned or outstanding waves. Each release reply reports only
two held responses, not 128 simultaneously ready transports.

Each loaded phase contains 129 complete coalesced wake/pump/apply chains that
begin after the conservative release-send clock bound and end inside genuine
AppKit resize. Their first-to-last envelopes are 289.505, 315.341 and 296.452 ms;
3/3/2 actual typing or direction-matching wheel handlers intersect them. These
are callback chains and finite-envelope interleaving, not per-request readiness
timestamps, CPU concurrency or sustained pressure. All nine scored phases
complete their 108 total ACKs and 54 actual wheel changes. Full run-loop mode
coverage remains NOTESTABLISHED because passive trace quotas overflow.

Loaded pump-core medians are 0.687–0.694 ms and apply medians 0.906–0.915 ms,
but wake-to-pump waits reach 19.468–28.107 ms and resize-core calls reach
18.628–23.264 ms. The latter still exceed an 8.33 ms frame budget. Measured
UI-work unions deduplicate nested spans; these stage costs cannot simply be
added. Small instrumented samples establish neither latency tails nor physical
120 Hz. All recorded 23 PIDs and 16 groups retire, and both ports are free.
The separate continuation preserves the earlier 404/incomplete archive and
covers 492 artifacts/24,482,235 bytes under
`target/normal-clock-responsiveness-storm-d6-v3/`, manifest
`939cbf9e2abaf31127ce6325d329784218645b83987c0afc0b51b28ca97e94d2`.

### 8.35 Messages row reuse with both timer entry points observed, 2026-09-17

Six fresh browser cells compare the stateless and reusable sources on the same
70668a3 base, ordered A/B, B/A, A/B. Only the private Web runtime factory differs;
bake, plan, glue and workload remain equal. The V3 observer wraps `exact_resize`
as well as `exact_advance`, dispatch, fulfillment and collection feedback.
Earlier advance-only comparison evidence remains inconclusive and preserved.

All six cells pass their functional checks with 10,000 supplied rows, 32 changed
rows observed, fixed two-second phases and no trace overflow. All 432 scheduled
offers and 144 scored input echoes complete; 474 rAF gaps are scored separately.
Per-export inclusive durations retain zero-length calls. The union below counts
only fully contained observed-call intervals, deduplicating any overlap; it
excludes JavaScript batch application, DOM layout and painting outside them.

| Pair | Comparator revision snapshots, control / reuse | Observed call union, control / reuse | Comparison eligibility |
| --- | --- | --- | --- |
| 1 | 8→16 / 8→16 | 188.4 / 107.3 ms | Equal boundary progress |
| 2 | 8→16 / 8→17 | 176.5 / 123.6 ms | NOT_ESTABLISHED |
| 3 | 7→16 / 8→16 | 147.8 / 96.4 ms | NOT_ESTABLISHED |

Only pair 1 satisfies the predeclared equal-progress rule. Pair 3's control
already reaches revision 8 at the later loaded-phase snapshot; the comparator
still refuses its earlier workload snapshot of 7. These reads are not atomic
with the score endpoints. They do not count exact commits inside the window,
so the totals measure observed coverage, not repeated speedup or per-update
cost. In pair 2,
reuse performs its expensive work predominantly inside `exact_resize`; ignoring
that export would again misattribute the cost. Across all six cells, 46 timer
entry calls exceed 8.33 ms. The reusable source therefore does not yet establish
the target frame budget.

Generic scrolling is observed, but concurrent resize and anchoring still prevent
wheel attribution. Coarse headless-Chrome rAF gaps are not physical presentation
intervals. No default entry activation, native gain or 120 Hz claim follows.
All six children and the supervisor exit successfully; all 13 recorded PIDs,
six process groups and six private listeners retire. Raw data, separate boundary
and unscored summaries, comparison decisions and cleanup receipts are under
`target/messages-row-reuse-web/70668a3/cells-resize-v3/`: 56 artifacts/2,158,143
bytes, manifest `b19d8b75edcf123c7a92f430b830c980d70b55f37d972ed04fb85020522d7853`.
The original six-cell archive and the supervisor's reproduced cleanup failure
remain intact.

### 8.36 Linux novel-width reflow: 1 MiB succeeds, 4 MiB exhausts the cap, 2026-09-17

The unchanged ef12f06 candidate changes authored column width 640→1200,
producing paragraph widths 600→984. Actual Linux uses the VKMS/display path
and RFB input at a fixed 1024×768, CPU, scale 1. This is content reflow, not
continuous window resizing. Each independent reference and candidate retains
the same 2.5 GiB address-space cap and 60-second process guard.

The first 1 MiB run completes its actions but fails the exact B crop: its
software cursor is absent from the reference. That whole-run FAIL remains
preserved. A separate reference-only correction specifies the independently
derived document-center pointer, [512,443], with identical buttons and scale.
It changes no candidate bytes, tolerance, crop or pixel mask. Both references
run afresh before each corrected candidate.

| Corrected workload | Completed candidate result | Sampled VmPeak / RSS |
| --- | --- | --- |
| 1 MiB | Full workload PASS; A and B match fresh references | 1,282,176 / 546,500 KiB |
| 4 MiB | FAIL: SIGABRT while waiting for B publication | 2,621,432 / 1,767,596 KiB |

At 1 MiB, all 26 proof fields and both entire 1,228,800-byte accepted crops
match. A remains painted through typing and scrolling while B is built; B
publishes without another input, then accepts further typing and scrolling.
Four private jobs use one shape and two layouts/indexes, with same-width reuse;
A retires after B adoption. Both original 40 px overlap comparisons remain
exact. The known completed-backing checkpoint peaks at 487,403,394 bytes;
it excludes in-progress arrays and private/process allocations.

At 4 MiB, both independent references pass, and accepted A matches all 26
fields and its full crop. Retained-A typing and scrolling also succeed while B
runs. The candidate then reports `memory allocation of 11264 bytes failed` and
exits -6. Sampled address space is only 8 KiB below the unchanged cap; no stack
identifies the allocation site. Its final frame still paints A at scroll Y=80,
with B pending publication. One shape, two layout starts and one index are
recorded: the second start is not a completed B layout. No B picture, autonomous
B publication or completed whole workload is claimed. The last known completed
K/S/L/baseline/index subtotal is 1,546,250,942 bytes, excluding in-progress B
and font/process costs. It is neither RSS nor the whole peak.

Corrected 1 MiB evidence is under `target/novel-width-pointer-runtime-1m-v2/`,
79 artifacts/31,344,261 bytes, manifest
`a0692b2f0fdedb49bbebfe99fa0cc10dab742c07ce61bcb1f16bbf8fe3ebd97f`.
The separate failed 4 MiB run is under
`target/novel-width-pointer-runtime-4m-v2/`, 79 artifacts/23,651,831 bytes,
manifest `dee0a82ce8c27c8ceeb2e3b3fb700626cf2db779ed5e163c1b501a107f545371`.
All owned processes retire; planned display shutdown after the 1 MiB proofs is
distinct from the unsignaled 4 MiB allocation abort. No cap increase or retry
follows the 4 MiB failure. These instrumented, nonquiet cases establish neither
an isolated memory saving, latency bound nor physical 120 Hz.

The next production change compacts private glyph vectors before their layout
is shared. It runs after shaping scratch is dropped and before Arc publication,
baselines and ink-index construction. A paragraph with less than 64 KiB spare
glyph capacity keeps its allocations; otherwise each oversized vector is moved
through a boxed slice to discard spare slots. This is an allocation optimization,
not admission or a memory ceiling. Source, glyph values, layout geometry, job
identities and the one Weak reuse slot stay intact.

The independent Buffer oracle records 96,096→64,073 glyph slots at width 600
and 76,288→64,477 at width 984 for the larger wrapped test, saving 2,818,024 and
1,039,368 capacity bytes. The unwrapped test saves 487,168 bytes; the ordinary
mixed-text test retains its 104 glyph slots. All four new tests are included in
96 passing text tests, independently repeated after integration, with strict
all-targets Clippy. Exact glyph/geometry/selection/cursor, CPU pixels, GPU batch
data, adopted-A sharing and last-owner retirement are checked. The initial
ordinary-control outer-array comparison error is preserved separately from the
three meaningful capacity failures.

Tight capacity does not prove allocator AS/RSS release. Reallocation may retain
the old block plus one replacement line; the unwrapped test's replacement is
5,280,000 bytes, not viewport-bounded scratch. Ordinary layout adds a line-header
scan; no overhead timings were collected. Full uncompacted output still exists
before this step. The purpose is to reduce A retained during later B construction.
The source/test
freeze is `target/layout-capacity-validation/freeze-v1/`, manifest
`f0cc711d4df9dc16246fb253812e31e6b6800d6d8ce3086aff1ea17f394971d0`;
integrated logs are in the adjacent `integration/` directory.

Separate compaction replays now retain the same workload, cap, guard, fonts,
pointer policy and receiver. Both candidate and references were rebuilt from
frozen ef12f06 plus the reviewed compaction, rather than a moving MAIN checkout.
At 1 MiB the whole recipe passes again: both 26-field proofs, full accepted
crops, retained-A typing/scrolling, input-free B publication and A retirement.
Four jobs use one shape and two layouts/indexes. Known completed backing peaks
at 404,207,930 bytes, versus 487,403,394 in the earlier run; sampled VmPeak/RSS
is 1,279,952/544,064 KiB. These different accounting categories do not establish
an isolated process-memory saving. All 303 events retain zero giant UI
shape/layout/index construction counters.

At 4 MiB, both fresh references pass but the candidate still aborts before B
publication, now reporting a 3,145,728-byte allocation failure. Sampled VmPeak
is again 2,621,432 KiB and RSS/HWM is 1,772,192 KiB. Accepted A still matches
all 26 fields and its full crop, handles typing and scrolls 40→80 while B runs;
the original 532,224-byte retained overlap remains exact. All 403 raw events
retain zero giant UI construction counters. The last frame still paints A.
B reaches the index-start marker after layout returns, but no completed B
proof/index or picture follows. The marker and allocation size do not identify
the failed allocation site. The completed-owner subtotal is 1,260,824,614 bytes,
including compacted A lines at 349,198,048 bytes; in-progress B, font catalogs,
allocator state, shrink transients and observer scratch remain excluded.

The new 1 MiB archive is `target/novel-width-compaction-runtime-1m-v1/`,
77 artifacts/31,145,130 bytes, manifest
`f41425072cbb85d31e8ba0c16b60319f6089baae03848bbd6325be7c264d9c19`.
The new failed 4 MiB archive is `target/novel-width-compaction-runtime-4m-v1/`,
72 artifacts/23,722,850 bytes, manifest
`79913d3078fcb63151f1708093d225335ec5b943152817877a0e891f252c9949`.
Original runs remain untouched. All owned groups terminate; planned 1 MiB
display shutdown remains distinct from the unsignaled 4 MiB SIGABRT. No cap
increase or retry follows. The next discriminator is the production index and
subsequent diagnostic proof allocation/lifetime path, before assigning a site
or choosing another change. Continuous resizing and physical 120 Hz remain
unproved.

### 8.37 Messages per-update stages identify remaining key work, 2026-09-17

Private diagnostic builds add four stage hooks to both 70668a3 source variants:
query, shape validation, key evaluation/uniqueness and index replacement. Actual
InstanceWork is captured at each tree update, under its owning Wasm call. Both
`exact_advance` and `exact_resize` are observed. There is no per-row clock or new
application scheduling. The two generated runtime factories remain the only
application difference; diagnostic hooks and the full 10,000-row/32-change
workload are equal. This instrumentation is not integrated into production.

The first control run fails its full functional oracle: an idle typing offer
arrives 113.461 ms late and is skipped under the existing 100 ms policy. The
original pair stops before reuse. No observed export spans that delay; an
overlapping 200 ms rAF gap does not identify its cause. All loaded offers still
complete and eight complete stage-bearing parents lie inside the loaded window.
The failed run remains failed.

A separately released reuse-only continuation passes once without a rebuild,
retune or skip-policy change. It has seven fully contained loaded stage-bearing
parents. An eighth parent crosses the window end and remains wholly unscored;
the revision snapshots 8→16 must not replace that actual interval count.

| Stage | Control median, 8 contained records | Reuse median, 7 contained records |
| --- | ---: | ---: |
| Query | 10.75 ms | 0.6 ms |
| Shape validation | 0.75 ms | 1.5 ms |
| Key evaluation and uniqueness | 6.6 ms | 8.8 ms |
| Index replacement | 0.2 ms | 0.5 ms |

Each of these records visits 109 nodes and evaluates 10,000 keys. Parent calls
span 18.9–24.2 ms in the control and 11.0–14.6 ms in reuse. Both captures observe
all hooks, 328 packets and 174 work records, without collector errors or overflow.
Silent, boundary and zero-stage update records remain separate. Tree-ok describes
the local tree result, not kernel or physical presentation. Callback/clock costs
are retained; unmarked work is not assigned to these four stages.

These single samples identify query construction and then all-row key work as
concrete costs. They establish neither a passing paired cohort, repeated gain,
default-entry activation nor physical 120 Hz. All owned processes/listeners retire.
The failed control is preserved under `target/collection-stage-web-pair-706/runtime-v1/`,
21 artifacts/607,596 bytes, manifest
`33ddcb51ec6f153f0bc470d2e03f9b084fee130335f923b0bd96fc8337d4cf06`.
The separate continuation is under `runtime-b1-continuation-v1/`, 21 artifacts/
848,495 bytes, manifest `d6fbbef39bdcd44a5ad9b9f7885656bc44c490fb831b215f4c6980bc745ebaa9`.

The follow-up Runner change reuses keys for same-position immutable items only
when a separate key-environment certificate is unchanged. It tracks actual
slot/derive/resource values, pending flags, clock and outer-frame structure;
unsupported dependencies decline reuse. Changed records still execute the key
VM. If every ordered key matches, the prior keys, uniqueness proof and index
remain. A mismatch restores ordinary canonicalization and duplicate checks in
original order, including duplicate-before-later-trap behavior. Body updates,
height invalidation and collection feedback remain active.

There is no extra per-row history or cross-position cache. The certificate is
bounded by dependencies and frame depth, and O(N) positional item scanning
remains. Fresh record identities still require all-N key evaluation; changed
keys or length use normal replacement work. Exact scalar bits distinguish item
identity; numeric key canonicalization, including signed-zero normalization,
is unchanged.

Integrated validation passes 117 Runner tests (15 new) and 13 existing Messages
reuse tests, plus strict all-targets Clippy for both crates. The actual Messages
Contract now performs 32 key evaluations per 10,000-row/32-change tick, while
the stateless control performs 10,000. Three ticks retain full result equality,
9,968 shared records and matching layout; typing/width changes still issue no
history query. The old test's explicit 10,000-key expectation failed with the
observed value 32 and is preserved before updating that expectation and adding
the explicit control assertion. The private Runner baseline has eight behavioral
failures after a separately preserved fixture construction correction.

These are counter/correctness results, not new browser or native timing samples.
Query/shape validation, scanning, body/layout and host work remain; shipped
Messages entry defaults are unchanged. The exact three-file Runner freeze and
raw evidence are at `target/collection-key-reuse-validation/freeze-v1/`, source
manifest `b545aa098845048334e07170292930188b73add19b90282c37747f7f33e4bfa5`.
Adjacent integration logs retain the additional consumer-test adjustment.

### 8.38 Reserve span storage by actual bidi runs, 2026-09-17

The pinned cosmic-text copy previously reserved a ShapeSpan header for every
UTF-8 byte in a paragraph, even when the paragraph contained one bidi run.
Removing that single reservation lets existing pushes grow storage by actual
span count. Shaping, layout, caches, worker admission and the zero-unit-font
guard remain unchanged. No prescan or shrink is added. Warm scratch still
retains its high-water allocation; many alternating runs may require more
vector growth, so this is not a universal latency improvement.

Three capacity regressions first fail against the unchanged vendor, while two
semantic controls pass. A fresh 65,552-byte ASCII source changes outer span
capacity from 65,553 headers to four for its one actual span. A multibyte source
and warm one-span source growth reproduce the same unnecessary reservation.
The alternating-bidi control retains its allocation across empty/small/repeated
inputs and preserves exact shape and width-layout output.

Integrated validation passes all 101 scoped Linux text tests, including five
new tests, plus strict all-targets Clippy and scoped formatting. The treatment
also compares all 225 immutable pre-change records exactly: source and font
identities, numeric shapes/layouts, sampled cursor/selection data, GPU batch
payloads and 90 CPU RGBA crops. Large cursor probes sample representative
boundaries; GPU payload equality does not claim a GPU-device run. These tests
run on macOS and establish neither actual-Linux memory savings nor 120 Hz.

Sources, original failures, both test binaries and before-data are archived at
`target/span-reservation-validation/`; the four-path source manifest is
`b0435fd62b7a0861e4c5e8daf2e0d60bd4f802e95fdaec1f3f107732b3a9e809`.
The subsequent native experiment in §8.41 combines this change with shared
and compacted layouts under the unchanged 1 MiB/4 MiB workloads, retained
painted A and 2.5 GiB address-space cap. The earlier 4 MiB novel-width SIGABRT
remains a failure; the later experiment supplies separate A+B evidence.

### 8.39 Repeated browser key-reuse measurements, 2026-09-17

Three fresh Before/After pairs pass in AB, BA, AB order on Apple M4 macOS ARM64
with Chrome 153.0.8010.48. Both arms use ReusableMessagesStress and supply all
10,000 windowed records, with 32 changed rows. The frozen 70668a3 composites
share the plan, generated factory, assets and diagnostic stage hooks; After
adds the Runner key-reuse change. Optimized Wasm identities are
`f18b940a5b1700cf31668cacd1bf002e08e06255a691fb5fd781cee55ab8a90c`
and `834d4993bb37fb821af4e213f3f2a04be17883ae1571646115277b11a1260bbe`.

Each cell retains fixed two-second idle, loaded and recovery windows, ordinary
clock progress, typing, wheel offers and viewport changes. Every pair has equal
boundary revision progress and the required observed exports. Each loaded
window contains eight key-bearing records whose entire parent call is inside
the window; no key-bearing parent crosses its boundary.

| Pair | Before parent median | After parent median | Before key median | After key median |
| --- | ---: | ---: | ---: | ---: |
| 1, AB | 14.20 ms | 3.55 ms | 8.85 ms | 0.15 ms |
| 2, BA | 14.05 ms | 3.90 ms | 8.80 ms | 0.20 ms |
| 3, AB | 14.35 ms | 4.05 ms | 8.90 ms | 0.20 ms |

All 24 Before records evaluate 10,000 keys; all 24 After records evaluate 32.
Each visits 109 nodes. Index replacement runs once per Before record and never
in the selected After records: an absent call is not a measured zero-cost call.
Quantized zero durations remain in the data. All Before parent calls exceed
8.33 ms; the largest selected After parent is 4.70 ms. Parent time includes
the stage intervals and surrounding work, so their totals must not be added.
O(N) positional scanning, list copies and Value shape validation remain.

Parent export mixes differ: advance/resize counts are 8/0 versus 7/1 in pair1,
6/2 versus 6/2 in pair2, and 5/3 versus 8/0 in pair3. These repeated workload
observations are not an identical per-export comparison or end-to-end latency.
Observed export coverage excludes other JavaScript, DOM, painting and physical
presentation. Generic scroll movement is present, but wheel causality remains
unestablished under the retained oracle. The six cells do not prove 120 Hz.

The original attempt stopped before After's scored windows because two clock
calibration brackets were disjoint by 1.334 microseconds. That failure remains.
A separately frozen correction uses conservative fixed 0.2 ms uncertainty per
sample, derived from pinned Chromium clock quantization, with outward bounds
and refusal of unknown browser versions. It changes no workload, export hook,
cadence or product. Its behavioral failures and 18 passing checks are retained.
The corrected cohort contains no retries or rebuilds, and all owned processes
and listeners terminate.

Exact raw cells and per-export/window records are linked from
`target/collection-key-reuse-stage-706/clock-three-pairs-v1/`, manifest
`0bcee4f96da032960b8dfce0c96a43ab485cf870a7278865128eb58bd63d73e9`.
The first matched native Messages pair follows in §8.40. Shipped Messages
factory defaults remain unchanged.

### 8.40 First matched native Messages row-reuse pair, 2026-09-17

One fresh Before/After pair completes on macOS with the full 10,000-row,
32-change workload, an ordinary 250 ms timer and external Quartz window-edge
input (synthetic OS events). Both builds use 630bc22 with the same Runner key
memo and three-file Swift observer. Only the generated native runtime factory changes
from MessagesStress to ReusableMessagesStress; bake, plan, compatibility and
host sources match. Executables are
`b83032e6aa74e58b0754e4e03346784712938c001f33c02ebe4892433b37c9a3`
and `7170cca3098e28dce2145162f42d7220e9728fab9f226290b0195e1135913c90`.
Older d6 measurements are motivation, not the control for this pair.

Both cells retain idle/load/recovery, 36 scored offers, no skips, and a separate
two-second command-free interval with eight complete timer chains. All six
resize/input endpoints are established: 72 ACKs, 36 typing actions and 36
directionally observed wheels. Actual initial window, port, backing scale and
Quartz geometry match. Loaded state boundaries are revision 8→18 in both arms;
seven complete timer chains lie wholly inside each genuine resize edge.
Full History values match at shared argument tuples for revisions 0, 8 and 18,
including ordered keys and every row field. Final revisions 19 versus 18 remain
different: clocks and content were not forced to match after pausing.

| Loaded observation | Before | After |
| --- | ---: | ---: |
| Advance/decode median, 7 complete chains | 10.337 ms | 3.882 ms |
| Advance/decode maximum | 23.307 ms | 10.358 ms |
| Whole timer median / maximum, including apply | 11.144 / 25.586 ms | 4.982 / 14.386 ms |
| Whole resize median / maximum, 35 complete spans | 5.945 / 13.895 ms | 6.200 / 12.025 ms |
| Typing ACK median / maximum, 6 samples | 6.619 / 13.927 ms | 7.074 / 28.748 ms |

Advance/decode includes Runtime.advance, FFI and batch JSON decoding; it is not
isolated query or Rust CPU time. After still has one advance/decode span and two
whole timer spans above 1000/120 ms. Its resize and typing observations do not
show a general latency improvement. Thirty-six resize envelopes surround each
edge, but one occurs entirely afterward; only 35 enter the table. The measured UI interval
unions are 342.354/316.509 ms inside edges of 1772.710/1820.125 ms. Nested work is
not added as independent CPU cost.

Mode coverage remains unestablished in all phases despite positive tracking
samples, because the passive quota overflows; primary records do not overflow.
This is one instrumented pair, not repeatability, robust tail latency or physical
120 Hz evidence. Two alternating repeat pairs use the same products and recipe
in §8.42. No shipped entry is activated by this experiment.

The frozen raw pair, exact geometry/content cohorts, build receipts and cleanup
proofs are at `target/messages-row-reuse-native-630-v1/`, runtime manifest
`c5bfe81ed0be6f76a4f261eb1d89c0b743cd2ad57d073eafaaac76b75472a2a4`,
922 files/236,583,306 bytes. Both cells end successfully once, all 12 recorded
PIDs and ten groups retire, and eight posters issue their owned button-up.
Original bootstrap and offline comparison errors remain separate from these
successful runtime cells; no source fix, rebuild or runtime retry occurred.

### 8.41 Capped Linux reflow after span reservation removal, 2026-09-17

The complete retained-A → novel-B recipe now passes at both 1 MiB and 4 MiB.
Each budget runs two fresh synchronous references and one native CPU/VKMS
candidate under the unchanged 2.5 GiB address-space cap and 60 s process limit.
The authored column changes effective text width from 600 to 984 px inside a
fixed 1024×768 display. This is content reflow, not continuous physical window
resizing. Both runs are recorded as instrumented, nonquiet correctness work.

The frozen ef12f06 pointer-v2/shared-layout composite already contains glyph
compaction; the only new source change is §8.38's span-reservation deletion.
It is not a clean current-MAIN snapshot. Observer, receiver, oracle, font guard,
external dependencies, workload and quotas remain unchanged. Both fresh ELF
builds select the pinned local cosmic-text copy. Candidate identity is
`a04a1f6df78eabf2a35b55bd6eb1667465216d93a21d6a8cb7e2d1cdbe7affb8`;
reference identity is
`058a4b2f3d2b3f668cc2862be95062f2beb513c90830f3e3b17fa9f9e7ba3c47`.
The font capture retains 39 faces from 13 reads and 107,296,644 owned bytes.

Raw source/canonical paragraph sizes are 1,048,531/1,048,499 bytes and
4,194,181/4,194,149 bytes. Both widths match all 26 fresh-reference proof fields,
including complete source, glyph, geometry and classified wrap-space coverage.
Each accepted 1,228,800-byte viewport crop matches exactly. A has no pointer;
B's independent reference includes the same document-center pointer. Both
532,224-byte scroll-overlap strips also match exactly, with no mask or tolerance.
Newly exposed pixels and entire continued-input pictures are outside those
pixel comparisons; numeric full-paragraph proof is separate from viewport ink.

A remains painted during B work and accepts typing and scroll. A and B each
publish with no intervening input, at unchanged input sequences 18 and 41;
subsequent B typing and scroll also complete. Four private jobs preserve full
two-axis request identities. Two same-width hits share backing identities;
the total is one shape, two layouts and two ink indexes. Ledger checkpoints
show A+B coexistence followed by A numeric-backing retirement. Cumulative giant
UI shape/layout/index construction counters stay zero, which does not exclude
other UI work, source copies or destruction.

| Budget | Sampled VmPeak | Sampled RSS/HWM | Accessible backing capacity peak |
| --- | ---: | ---: | ---: |
| 1 MiB | 1,223,884 KiB | 545,360 KiB | 345,492,154 B |
| 4 MiB | 2,411,644 KiB | 1,774,156 KiB | 1,382,011,614 B |

The 4 MiB observed AS margin is 209,796 KiB, about 204.879 MiB. The K/S/L/B/I
ledger deduplicates categories and backing identities; it excludes in-progress
arrays, fonts/catalogs, scratch, allocator overhead and RSS. Its shared S is
667,208,977 B at 4 MiB, 234,872,176 B below the prior compaction run's S ledger.
That category difference is not measured RSS savings or allocation-site proof.
Do not add the ledger to RSS or compare the earlier A-only checkpoint subtotal
as though it were a completed A+B peak. No general width/memory bound follows.

All six cells complete once: references and coordinators exit 0; each display
candidate receives its intended owned SIGTERM only after the passing observations.
Recorded processes and groups retire. The 4 MiB post-terminal bind probe returns
Errno 98 with only TIME_WAIT and no listener; its diagnostic exit 1 is preserved
separately. There is no retry, port substitution, source reduction or cap increase.
Earlier reserve-only, shared-layout and compaction failures keep their verdicts.
This clears their tested 4 MiB content-reflow barrier, not a latency or 120 Hz bar.

Final archives are `target/novel-width-span-reserve-runtime-1m-v1/` (84 files,
31,270,472 B; manifest
`2674ca7b055147158164538aa3cce43a708cebdb1c73ce585244fae1d602ae30`)
and `target/novel-width-span-reserve-runtime-4m-v1/` (88 files, 31,472,007 B;
manifest `11dcb637f09ee3faf749fa4c772df573f0cf177df22f9062ae822d063dd253ac`).
The separate build archive proves source/package/binary selection; raw journals,
fresh references, memory samples, pictures and ownership receipts remain available.

### 8.42 Two alternating native Messages repeat pairs, 2026-09-17

Four further fresh cells complete once in BA then AB order using §8.40's exact
two binaries, plan, recipe and 10,000-row/32-change workload. No source change,
rebuild, runtime retry or factory activation occurs. Before remains the stateless
source; After uses row reuse, with the same Runner key memo in both arms.

The lower loaded advance/decode median recurs in each order. These are complete
linked timer chains wholly inside genuine AppKit resize edges, not isolated
query CPU time or display frames. The first pair is retained for comparison:

| Pair/order | Before/After inside chains | Before/After loaded revisions | Before/After core median | Before/After whole-chain maximum |
| --- | ---: | --- | ---: | ---: |
| 1, AB | 7 / 7 | 8→18 / 8→18 | 10.337 / 3.882 ms | 25.586 / 14.386 ms |
| 2, BA | 6 / 7 | 9→19 / 9→18 | 10.181 / 3.959 ms | 19.621 / 11.059 ms |
| 3, AB | 7 / 7 | 8→18 / 9→19 | 10.822 / 3.277 ms | 14.774 / 10.245 ms |

Callback counts and revision intervals differ naturally; the two new pairs are
not matched at every timed payload or progress boundary. All 60 saved full
History states validate ordered keys, five row fields and body-byte totals,
and whole values match across every equal six-argument tuple. Shared tuples
also match the original pair. Final revisions 19/19/19/20 are preserved.
These small repeated cohorts support the observed core-cost reduction, not a
general speedup or robust latency distribution.

Whole timer chains still exceed 1000/120 ms in every After cell. New After
loaded resize-whole medians are 6.406/6.284 ms versus 5.975/6.221 ms Before;
After maxima are 15.645/15.597 ms. Each uses 35 complete inside-edge resize
chains; the 36th occurs entirely afterward. There is no resize improvement in
these pairs. The original After core maximum 10.358 ms, whole maximum 14.386 ms
and loaded typing ACK maximum 28.748 ms remain in the record. Core includes
FFI/JSON decoding, apply includes feedback, and ACK is not presentation.

All twelve new resize/input endpoints are established: 144 ACKs, 72 typing
actions and 72 actual directional ±40 px wheels, without skipped offers.
Each edge has 18 genuine AppKit live changes. Actual window, port, backing
scale and Quartz trajectories match between arms and the retained first pair;
absolute scroll/revision/clock equality is not forced. Each new cell also has
eight complete timer chains in its separate two-second command-free interval.
Mode coverage stays unestablished despite positive tracking samples because
the passive quota overflows; primary trace records do not overflow.

All four cells exit 0, all 28 recorded PIDs and twenty groups retire, and all
sixteen posters issue their owned button-up. Two offline schema-assumption
failures remain separate; neither changes raw data, oracle or runtime results.
The original 922-file archive stays immutable. New reports, raw states, exact
geometry/content cohorts and cleanup evidence are under
`target/messages-row-reuse-native-630-followup/`, 735 files/278,227,700 bytes,
manifest `6ffbe73c511a12d0d7205e79ed1be3f81af489f2b1544ab6282421a2aae17930`.
Remaining work is the UI/resize cost split and actual presentation evidence;
neither repeated lower core medians nor these functional endpoints prove 120 Hz.

### 8.43 Native Messages resize boundary measurements, 2026-09-17

One further private 630 After capture/run separates the remaining synchronous
costs. Its Reusable factory, baked plan and compatibility bytes match §8.40's
After product. Only diagnostic Bridge/Session markers are added to the existing
normal-clock observer: after native return, after the owned output copy, and
around collection feedback through returned-batch application. Workload remains
10,000 rows/batch32, ordinary 250ms timers, three genuine resize edges with fixed
12 typing/wheel offers each, and a separate command-free interval. This more
instrumented cell is a cost discriminator, not another speed A/B.

Each edge contains 35 complete resize chains and seven timer chains. Resize
measurements in milliseconds are:

| Phase | Native/entry median | Copy median | Decode/Batch median | Apply median | Whole median / maximum |
| --- | ---: | ---: | ---: | ---: | ---: |
| Idle | 4.638 | 0.003 | 0.170 | 1.340 | 6.269 / 13.690 |
| Loaded | 4.790 | 0.003 | 0.164 | 1.345 | 6.563 / 13.790 |
| Recovery | 4.636 | 0.003 | 0.165 | 1.744 | 6.340 / 15.863 |

These are per-column medians, not additive components of a median call. Native
and entry includes synchronous platform callbacks, wrapper/probe overhead,
locks and descheduling; it is not isolated Rust CPU or pure layout time. Its
loaded maximum is 9.580ms, recovery maximum 10.799ms. Output copying and JSON
decoding are small in this cell; the next useful boundary is within native
resize/layout and its text-measurement callbacks, not a transport rewrite.

Apply includes reentrant collection feedback. The 77/79/75 complete feedback
chains inside the three edges remain individually available; parent overlap
is an interval union, never a sum of nested calls. Loaded resize apply
has median feedback overlap 0.403ms and unclassified residual 1.304ms. These
separate medians do not sum to the apply median. Feedback's final interval
combines decode and apply because no internal boundary was measured.

Whole resize calls exceed 1000/120ms in 12/9/11 cases. Seven loaded timer chains
have whole median 3.677ms and maximum 7.724ms, but that subset does not establish
the deadline: five of eight chains in the separate zero-command producer interval
exceed it, including whole spans of 13.889ms and 13.825ms. That interval's whole
median is 8.955ms. Its revision advances 0→8; loaded boundaries are 9→19.
Current-source full History values match the retained equal-argument cohorts.
Mode coverage remains unestablished because the passive quota overflows;
primary trace records do not overflow.

The one build and one runtime finish successfully, with all seven runtime PIDs,
five groups and four owned poster button-ups accounted for. Added probes keep
the 4096/512 quotas and add two record attempts per observed advance/resize,
four per feedback, plus serialization and synchronous parent bookkeeping.
No overhead subtraction, default activation, physical frame deadline or 120Hz
claim follows. Raw states, complete nested split rows, source/binary receipts
and terminal proofs are retained under `target/native-630-bridge-split-native-v1/`,
464 files/123,721,283 bytes, manifest
`99409ba5835eca0910f4f0da23993ac0fea35503b53f022416aef7dbaf352627`.

### 8.44 Native resize layout and measurement callbacks, 2026-09-17

One further private 630 After build and cell refine §8.43's native boundary.
The Reusable factory, plan, compatibility, full 10,000-row/batch32 workload,
ordinary timers, three resize edges and 36 fixed input offers are unchanged.
A fixed 96-byte C reply carries viewport/layout elapsed times and measurement
counts; five scalar rows follow each resize's existing markers. The actual
compiled header is bound through the Bridge object dependency file, alongside
the source receipt and Rust/C/Swift ABI checks. This is another instrumented
cost discriminator, not a speed A/B or a production hot-path change.

All 105 complete interior resize parents have matching scalar rows, nonnegative
residuals and one kernel call each. No primary records are omitted. In milliseconds:

| Phase | Native/entry median | Kernel inclusive median | Swift callback median | Kernel minus callback median | Whole median / maximum |
| --- | ---: | ---: | ---: | ---: | ---: |
| Idle | 4.992 | 4.915 | 4.609 | 0.318 | 6.977 / 12.827 |
| Loaded | 4.580 | 4.519 | 4.227 | 0.288 | 5.836 / 14.593 |
| Recovery | 4.514 | 4.405 | 4.053 | 0.345 | 6.400 / 13.415 |

Callback time is included in kernel time; separate medians are not additive.
The full synchronous Swift entry includes string decoding, Spec construction,
cache work and CoreText through return. It accounts for 93.090% of summed loaded
kernel wall time, not 93% of CPU or pure CoreText shaping. The 35 loaded resizes
make 9,685 identified memo calls, with 4,833 hits and 4,852 foreign callbacks
(128–146 per resize); plain calls are zero. Every parent satisfies
`foreign = memoCalls - memoHits + plainCalls`. Loaded `set_viewport` median is
0.000792ms; the other native/entry residual is 0.076416ms and remains unclassified.
These rows do not partition timer settlement, resource construction or keys.

Whole resize calls exceed 1000/120ms in 11/7/9 cases. One of seven loaded timer
chains exceeds it (maximum 8.562ms). Three of eight chains in the separate
2,002.064ms zero-command producer interval exceed it, with whole spans of
14.148ms, 14.017ms and 10.834ms. The producer advances revision 0→8; the loaded
edge is 9→19. All 36 input offers are acknowledged, including 18 actual alternating
40px wheel movements. Passive quota omissions still leave mode coverage
unestablished. No frame deadline or physical 120Hz conclusion follows.

Each edge has 18 actual AppKit size changes but 35 distinct interior viewport
offers, including 17 adjacent pairs differing by +17 points in both axes.
These are different sizes, not duplicate identical-viewport calls. A separate
small AppKit control reproduces transient legacy scrollbar gutters when an old
fitting document is tiled before it is resized. Its fitting and genuine-overflow
cases guide a candidate pre-fit fix; this does not yet establish correctness or
a workload speedup for that candidate, nor justify skipping necessary layouts.

One build and one runtime finish successfully without retries; seven recorded
runtime PIDs, five groups and four poster button-ups are accounted for. Raw
records, all per-parent cuts, prior bridge/timer boundaries, exact source/header
and binary receipts remain under `target/apple-resize-native-cut-native-v1/`,
508 files/126,951,165 bytes, manifest
`0678dd2aea4222a3f187b3c0f8e0f4ffc2062e393b78cffcb9a99f62c1294d5d`.
The separate AppKit control is under `target/native-630-resize-tile-price/`.

A subsequent small production cleanup passes the already-resolved text identity
from definite-width measurement into the private paragraph helper. This removes
a second identity lookup on that miss path; intrinsic measurement, public
paragraph lookup, source/paint keys, width offers and ownership policy are
unchanged. The exact changed source passes 32 existing standalone TextGeometry
methods (517 assertions) and full ExactKit Swift typechecking. Four
Session/NodeView-dependent methods are excluded from that standalone harness;
this is not a full XCTest or native performance run. No saving is measured yet.
The source and checks are retained under `target/apple-text-known-identity/`,
manifest `4743d8edadd3e91adce1872a0025415a6836cae3b25fdc3caec2857bdc89f90d`.

### 8.45 Fitting documents before AppKit resize tiling, 2026-09-17

The page scroll view now remembers a completed document fit and shrinks only
previously fitting wrapper axes before AppKit tiles a smaller outer viewport.
This prevents the old wrapper extent from temporarily introducing unnecessary
scrollbars. Authored root frames and genuinely overflowing axes stay owned by
normal layout. The fit is consumed before callbacks and refused after changes
to the document, geometry, scrollbar configuration or insets, and during batch
application/reset. No fixed gutter width, timer or extra layout loop is added.

The small AppKit control retains its behavioral baseline failure. Across its
28 resize steps, candidate viewport offers fall from 40 to 30; the stable legacy
fitting case uses one offer per shrink. Eight cases over the full compiled
ExactKit module pass 38 assertions, covering overflow, scroll origin, successive
shrinks, invalidation and reentrant batch handling. The standalone runner uses
the unchanged test bodies because this CLT installation lacks XCTest; the
failed SwiftPM attempt is retained.

A separate real ExactView/Session/Runtime fixture checks viewport-derived width
and height at four natural drawing boundaries without corrective fit/layout
or pump calls. All four agree, including after vertical overflow appears. Its
first 956-point outer width nevertheless has a 939-point clip, so that fixture
does not prove every transient gutter is removed. The earlier control's final
manual-layout height discrepancy remains separate. Missing-view-section and
string-valued sizing fixture failures are preserved before the successful
numeric fixture. These are correctness and callback-count results, not a
Messages latency comparison or a physical 120Hz claim.

Exact production/test patch `e9c5902487d17c48f4b40994e643d12c972889bc51f2c4fd07c13fcbc0fa86d3`
and the control, eight-case and natural-draw evidence are retained in the private
`exact2-appkit-prefit-document/target/appkit-prefit-validation/` archive.

### 8.46 Linux UI work and presentation waiting, 2026-09-18

One instrumented actual-Linux 1MiB run now completes the cold source and the
600→984 content-width transition on a fixed 1024×768, 60Hz VKMS display. Two
fresh reference processes and the candidate pass: both accepted RGB crops,
all 26 reference fields, both 40px overlapping scroll strips, retained-A input,
autonomous B publication, shared-width reuse and old-backing retirement. Four
jobs build one shape and two layouts/indexes; giant UI construction counters
remain zero. This is one transition, not continuous display resizing or 120Hz.

The saved journal closes 129 UI turns, 3,226 numeric spans and 12 worker-port
lifecycles through the final picture at frame31/input48. Seven ordinary text
submissions have typed null proof IDs; four giant submissions retain their
distinct job identities. The original diagnostic boot assertion on the 46-byte
placeholder remains a failed run. The correction has five exact-source,
standalone Mac Rust tests; the separate Cargo attempt refused before launch
because warm test dependencies were absent. Neither is a Linux test-suite pass.

Only display submission and page-flip waiting contain individual recorded spans
over 8.333ms. Frame building reaches 4.141ms, painting 3.741ms and input dispatch
1.921ms. The largest nonboot turn after excluding display wait, observer work
and VNC publication is 6.640479ms; it still includes display copy/submission and
unattributed work. Boot retains a 9.915537ms remainder even after excluding all
those display and observer intervals. These are synchronous wall measurements,
including scheduling, not isolated CPU costs. Observer work reaches 4.753ms and
display wait 20.419ms. Excluding them does not make the actual loop a 120Hz loop.

The four giant slot-store→take intervals are 0.057792, 14.777387, 0.041667 and
8.460530ms; the two long cases are cached completions. Production `present`
currently waits for the page-flip event before the loop returns to its input and
worker wake descriptors. That is a concrete next scheduling investigation, not
proof that every measured delay has that cause or a reason to add a worker pool.
The two long intervals overlap 9.134/2.041ms of display wait and 4.140/4.024ms
of diagnostic work respectively; removing the wait would not remove all of them.

All processes retired; the candidate's SIGTERM followed completed observations.
The original failure, bounded recorder, censored final observer flush and
`nonquiet=true` label are retained. No individual-keystroke visibility, CPU,
memory-saving or physical-frame claim follows. Evidence is under
`target/linux-ui-critical-runtime-1m-v4/`, manifest
`18eedd010b432164e3b839fe85d3d51da051b52d98b703f578b69c599a9f9bbe`.

### 8.47 Native Messages prefit comparison, 2026-09-18

The first fresh A→B pair isolates the page-wrapper prefitting change on exact630
with the same reusable data factory and native-cut observer. B changes only
PresenterMac; the text-identity cleanup is excluded. Both arms keep all 10,000
rows, batch32, ordinary timers, typing, wheels and the same 18 external window
changes per phase. Actual window/display/port geometry and all 18 accepted
viewport offers match. Backing scale is not separately recorded by this carrier.

Each baseline step produces a transient viewport followed by the accepted one;
B goes directly to the accepted dimensions. Across the complete delivery of
those 18 steps, layout calls fall from 36 to 18 in every phase. Measured whole
resize-interval unions in idle/load/recovery are 258.513→185.692,
248.244→163.241 and 219.843→145.352ms. Full synchronous Swift measurement counts
are 5,956→2,998, 4,992→2,496 and 4,992→2,496. Nested feedback is not added again.
These totals exclude AppKit work outside the observed entries, including the
prefit operation itself; they are not total main-thread occupancy or CPU costs.

The strictly inside-edge cohort is different: A has 35 calls and its final
accepted viewport arrives after didEnd; B has all 18 inside. Both views are
retained. Removing calls changes the mix: per-call medians are higher in B,
and 14/11/9 inside-edge resize calls still exceed 8.333ms. Loaded timer whole
medians worsen 5.059→7.502ms and typing ACK maxima 4.179→16.960ms. The separate
command-free periods have eight versus nine timer chains, with four versus
seven deadline misses. Lower aggregate resize work is not a uniform latency win.

Both cells pass their resize/input endpoints, all 72 inputs receive replies,
and shared-argument full-history values match. Mode coverage remains incomplete;
recovery revisions differ. All owned processes retired. This first pair alone
does not establish a repeated gain, robust tail latency or physical 120Hz.
Evidence is under `target/native-630-prefit-pair-v1/`, manifest
`1f9b3fd3bf1ee26c99136c66f454721792dad56ca6147e33114ec89616c63dfb`.

A separate fresh B→A pair reuses those exact products and recipe without a
rebuild. The 36→18 calls and accepted-viewport sequence repeat in all phases.
Full-delivery resize unions are 267.725→195.084, 244.875→165.948 and
249.732→165.419ms: 27.1%, 32.2% and 33.8% lower, versus 28.2%, 34.2% and
33.9% in the first pair. This confirms the direction of measured aggregate
resize-work reduction across both run orders, with the same probe exclusions.

The latency limits repeat too: B resize maxima are 17.022/14.325/13.916ms;
loaded timer medians are 4.051→7.649ms (seven versus eight callbacks) and typing
ACK maxima 9.295→17.090ms. Other tails improve, so neither uniform improvement
nor regression of all latency follows. Loaded revisions are 8→18 in this pair,
versus 9→19 in the first; no cross-pair timed-progress equality is imposed.
Both cells pass, all 72 offered inputs reply, selected full histories agree and
all owned processes retire. No further run or generalized frame-rate claim is
implied. Reverse-pair evidence: `target/native-630-prefit-reverse-pair-v1/`,
manifest `5456d96efee7993ce86496e1f3c9f6d3aa5d33a68ebdc16a149079804930da29`.

### 8.48 Native resize measurement cache hits, 2026-09-18

One further prefit-B diagnostic adds two counters in Text and two trace rows per
resize in Session. The captured product differs from the preceding B only in
those two Swift files; no cache, Rust memo, timer or worker policy changes.
All 54 inside-edge resize parents have complete, ordered count pairs matching
the original C96 callback total. Each phase still has 18 distinct viewport
changes, all 10,000 rows and batch32.

| Phase | Swift callbacks | Intrinsic | Definite | Scalar hits | Geometry hits | Without either hit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Idle | 2,998 | 360 | 2,638 | 360 | 2,004 | 634 |
| Load | 2,496 | 360 | 2,136 | 360 | 1,622 | 514 |
| Recovery | 2,496 | 360 | 2,136 | 360 | 1,622 | 514 |

Every parent has 20 intrinsic requests and 20 scalar hits. Thus the geometry
hits in this sample are definite-width requests; that is not a general rule
about the counter. During load, 1,982 of 2,496 callbacks (79.4%) return through
one of these Swift caches. They still cross the foreign-call boundary and the
existing entry constructs strings and measurement specifications before lookup.
Avoiding that construction on an exact cached request is the next source-level
candidate. No such optimization is included in this diagnostic.

These are counts, not cost shares or proof of Rust memo eviction. Changed
height, source, owner and cache lifetime remain possible reasons for entry.
The 514 loaded requests without a recorded hit are not a typesetter-construction
count. The results do not by themselves justify reserving intrinsic memo slots,
changing definite-width retention or adding parallel measurement. Counters end
before apply and exclude its feedback; inclusive nested spans are not summed.

The original functional endpoints pass and all 36 inputs receive replies.
Primary trace omissions are zero; passive mode omissions still prevent a mode
coverage claim. This is one instrumented cell, not a performance comparison.
Whole resize maxima remain 15.919/12.530/13.508ms, with 11/10/14 of 18 calls
over 8.333ms. Two of seven loaded timer chains miss that budget. The separate
command-free interval retains five misses in eight timer chains, median
10.740ms and maximum 15.422ms. Probe-excluded AppKit work, physical presentation,
per-kind CPU cost and a 120Hz guarantee remain unmeasured.

One build and one cell completed without retry; all owned processes retired.
The 20 pure counter/oracle checks are retained separately from native evidence.
Raw count identities and timer tails were independently reconstructed. Evidence:
`target/native-630-prefit-text-mix-native-v1/`, manifest
`fa7a794fcd4fbcf498751fd62e39e74585ececcd91a6981f964f8b6e506490d7`.

### 8.49 Borrowed Apple text measurement lookup, 2026-09-18

The synchronous Swift callback now checks the existing metric identity index
before decoding its borrowed C request into Strings, Runs and a Spec. A shared
metric hash selects candidates; exact original UTF8, run boundaries and converted
metric fields decide equality. Only an existing intrinsic scalar or exact-width
geometry answer returns early. Paint remains separate, catalog replacement keeps
its existing namespace, and no C pointer or request escapes the callback. There
is no additional cache, worker or retained source history.

Cold sources, new widths and malformed UTF8 retain the decoder path. The lookup
still scans source bytes, and a miss can scan them again after decoding; avoiding
construction is neither constant-time nor a zero-allocation claim. Existing
Swift height handling and Rust's two-axis memo checks remain unchanged. Cache
maintenance runs once for identity lookup/admission. The measureSeconds counter
includes the borrowed lookup and existing cache/layout work, but still excludes
fallback Run/Spec decoding; it is not interchangeable with the full foreign-call
wall interval measured in §8.44.

A test-only marker at the actual decoder entry fails three cached-repeat checks
on the original callback, while cold/new-width controls pass. The candidate
passes 38 methods and 7,027 assertions in a strict Swift 6 optimized CoreText
runner, including existing bitmap, Unicode, lease and cache-bound tests. Forced
hash collisions pass another 68 assertions. Controls cover changed run/strut
fields, exact Unicode bytes, signed zero, malformed input, independent freed C
buffers, catalog restore and retained paint. The reference callback body is the
original body running against the candidate cache, not a separately linked old
engine. A test-only compile warning and its correction remain in the evidence.

The complete current ExactKit module typechecks on macOS. Four Session/view
methods are excluded from the standalone assertion runner; this is not full
XCTest or an iOS build. Compiled copies reconstruct to the exact production
sources after removing the test hooks. The first native comparison in §8.51
does not establish a consistent speedup. Neither validation establishes a
physical presentation rate.

Evidence: `target/apple-text-borrowed-validation/freeze-v1/`, manifest
`251ea3da4b96a9bb6692f40c0fc20db8171c188b816529ff882a0b2c1e509d3c`.

### 8.50 Linux display completion without a blocking wait, 2026-09-18

The DRM host now submits at most one pending flip and waits for its descriptor
in the existing input/executor/region/timer poll. The first set_crtc remains
synchronous; subsequent page flips return to that loop instead of blocking in
receive_events. Each readiness turn performs one bounded nonblocking read.
WouldBlock, interruption, empty batches and unrelated events leave the pending
frame intact. Only a matching CRTC and forward sequence retire it. Two display
buffers remain in use, and an occupied slot refuses a second copy/submission.
Pending animation alone does not spin the loop; real timer deadlines remain.

Input geometry follows the acknowledged picture. If A is displayed, B is
submitted and C becomes live, B's paint cannot prematurely replace A's hit
boxes, text source or scroll extent. A receipt carries B's publication and
ownership witness until acknowledgement, with current eligibility checks able
to refuse deleted or replaced targets. The attached display guard also prevents
implicit painting through boxes() between an acknowledgement and the next
submission. A successful old-origin completion can release its receipt without
publishing it into a replacement presenter. Headless eager-frame behavior stays
unchanged.

The natural-text regression starts with tall A at scroll60, holds short B's
flip and accepts same-key C. Wheel input queues scroll90 while A's painted
scroll/source remain60. B's acknowledgement installs B's zero extent/source;
box, hit and wheel queries still use B before C is submitted. The original
failure clamped A to zero before acknowledgement. Receipt ownership, reload,
failed painting, executor/timer progress and unchanged headless controls pass
alongside this case. Pixel aliases do not retain receipt metadata histories.
The witness costs O(painted nodes); two display buffers are not a bound on all
live kernel, worker, cache or image memory.

The exact eight-file change passes 22 integrated tests and strict all-target
Clippy on macOS. Actual aarch64 Linux additionally compiles and runs the 13
display tests: 35 distinct scoped tests pass, plus strict release all-target
Clippy. Readiness tests use real libc poll over UnixStream descriptors, while
event/ioctl transitions use deterministic callbacks. This is not a DRM device
run, a full-package pass or measured responsiveness improvement. All owned
validation processes retired; the unchanged disk reserve remained satisfied.

A future native comparison must distinguish CPU paint, submission and actual
acknowledgement across turns. The old observer's same-turn paint/presentation
assumption cannot be reused as a timing baseline for this change. Source and
Mac evidence: `target/linux-pending-flip-validation/freeze-v1/`, manifest
`0968f5f30ce76b25d1f7d46b89aa3e8c91a73d1a4f37192091897540e5f6214b`.
Actual Linux evidence: `target/pending-flip-linux-execution-v1/`, manifest
`de35e367855adc14c8e1ebd42f4bd0b38d21ee6de05650d86006c24c0604ae1b`.

### 8.51 Borrowed text lookup: first native comparison, 2026-09-18

Two fresh optimized Messages products and one control→treatment pair test
§8.49 with the same ReusableMessagesStress factory, prefit behavior, common
known-identity cleanup and diagnostic hooks. Only Text and TextResidency differ
among 258 captured source inputs per arm. Historical prefit products are not
the timing control. All six phases accept the same 18 viewport changes, wholly
inside their genuine resize edges; window, port and external point sequences
agree. There are no additional post-edge resize parents in this pair.

| Phase | Whole resize union ms, control→treatment | Full Swift callback sum ms | Callback entries per arm |
| --- | ---: | ---: | ---: |
| Idle | 171.622→189.784 | 106.301→114.754 | 2,998 |
| Load | 169.928→155.909 | 105.796→95.283 | 2,496 |
| Recovery | 163.994→171.623 | 102.281→103.473 | 2,496 |

Across 54 resize calls per arm, whole work totals 505.544→517.315ms,
native-and-entry 348.020→349.797ms and full Swift callback wall
314.378→313.510ms. All 108 parents have the five C96 rows, the exact callback
count identity and one kernel layout. The intervals do not overlap, so these
whole unions equal sums. Nested feedback is not added again. These mixed phase
directions and nearly equal pooled callback costs establish neither a consistent
native speedup nor a reliable regression estimate from one fixed-order pair.
The full callback includes lookup, decoding and cache/CoreText work; this is
not decoder CPU. The changed measureSeconds interval is not used for comparison.

Whole resize calls above 8.333ms total 35/54→37/54. Loaded complete timer
chains number 7→6, with revision progress 9→18 versus 8→18; their populations
are not matched. Their whole medians are 6.088→6.576ms and maxima
11.853→9.305ms. Loaded typing ACK maxima are 13.472→24.115ms and wheel ACK
maxima 2.481→16.680ms. Separate command-free intervals both advance revision
0→8, but retain eight versus seven complete pre-read timer chains: five versus
six miss 8.333ms, with maxima 15.092→15.170ms. Revision deltas are not callback
counts, and clock overrides remain nil.

All 72 offered inputs reply, including 36 directionally verified wheel moves.
Selected full 10,000-row histories agree whenever all resource arguments agree.
Interaction endpoints pass; passive quota omissions retain mode NOTESTABLISHED
despite positive tracking samples. Primary omissions are zero. All owned
processes retire after exactly two cells, without retry. The current hardware
record identifies a 60Hz display; software spans do not establish physical
120Hz. Unobserved AppKit work and presentation costs remain outside these hooks.

The source tests establish that cached repeats can avoid decoding; this native
pair does not show a workload speedup. Further work should target the remaining
callback and display-loop costs rather than treating this result as a gain.
Evidence: `target/native-630-borrowed-text-pair-v1/`, manifest
`a5979826072793ce269cb9d51f238cb784920f03e70387e1d145d203a13ed571`.

### 8.52 Browser resize tasks beyond the Wasm calls, 2026-09-18

One fresh instrumented After cell reuses the immutable `834d4993` Messages
runtime from §8.39, with all 10,000 rows, actual batch32 and unchanged fixed
two-second idle/load/recovery windows. All 72 offers and 24 scored input echoes
pass. Browser tracing adds no page commands or sampling; its overhead is
nonzero and uncalibrated. This is a discriminator, not another speed comparison.

The first capture failed in the diagnostic reader: the completion event names
its format `traceFormat`, while the reader and its mocks expected the request
field `streamFormat`. Functional checks passed, but no stream bytes were read;
the handle and browser were closed and that failure remains sealed. The literal
saved reply supplies two behavioral RED checks. A separate one-field correction
passes 29 pure tests and three ownership checks before the new cell. No category,
quota, timing tolerance or workload change accompanies the correction.

The successful capture retains 14,010,791 bytes, explicit no-loss/EOF/close,
61,289 events and 1,787 recognized renderer tasks. Navigation metadata selects
the renderer and main thread; two clock brackets give a 0.752046ms-wide
trace-to-page offset interval. Fixed 200us page endpoint uncertainty remains in
containment and export joins. Missing or ambiguous joins are not assigned to
the nearest task: only 85/196 full-capture joins are established, including
17/39 loaded joins.

| Fixed window | Fully contained tasks | Task wall union ms | Largest task ms | Tasks above 8.333ms |
| --- | ---: | ---: | ---: | ---: |
| Idle | 340 | 115.404 | 6.297 | 0 |
| Load | 297 | 125.486 | 8.718 | 5 |
| Recovery | 316 | 83.577 | 3.879 | 0 |

The five loaded misses take 8.426–8.718ms. Each contains a resize event and
joins an observed `exact_resize` call of 3.8–4.3ms plus collection feedback of
0.1–0.2ms. The trace locates the handler in the frozen glue.js resize listener,
which performs resize, output decoding and DOM application. Layout spans of
1.097–1.173ms, paint of 0.519–0.562ms and the collection-flush animation callback
also occur in these tasks. These intervals overlap; they are not additive CPU
costs. Exact JSON parsing and DOM application costs remain inseparable here.

Conservative interval subtraction leaves 1.819–2.365ms lower bounds and
5.570–5.921ms upper bounds outside observed exports for those five tasks.
All potentially overlapping exports and clock uncertainty participate in these
bounds. They do not identify pure JavaScript or establish a cause for every
frame gap. The full-capture maximum task is 24.237ms outside the scored windows;
cold/setup/primer/silent records remain separate. The next source investigation
is the remaining resize/DOM path, without inferring that more worker threads
would remove its cost.

All owned processes and the private server retire after the single corrected
cell. Wheel causality remains NOT_ESTABLISHED. The known display is 60Hz;
neither renderer tasks, rAF nor paint events establish physical 120Hz.
Successful evidence: `target/web-after-trace-cell-v2/`, manifest
`497a037625a5964bb4cd1e279a7c5970067e5b697b82ad326ac96ff045aa2d22`.
The original failure remains in `target/web-after-trace-cell-v1/`, manifest
`dc997ad466ddb723a6902526792f305613d21ef9b25f161fdf5991d15c9e9308`.

### 8.53 Web collection geometry read reuse, 2026-09-18

Following the resize/DOM trace in §8.52, collection commits now read correction
geometry only after the correction passes the cheap revision, scroll-sequence
and requested-top checks. Actual current dimensions still decide whether to
apply it. Observer baselines remain fresh on each valid commit. In the real DOM
fixture, a correction-free commit reads the list and port rectangles once each
instead of twice; an eligible correction still reads its current nested origin.

Within a single feedback pass, each mounted row's measured rectangle supplies
both its reported height and its observer width/height baseline. A temporary
map bounded by mounted rows is cleared before synchronous feedback can replace
nodes or change dimensions. Later passes sample again. Presence checks remain
separate: zero-height attached rows can be measured; hidden or detached rows
cannot supply invented geometry. Epochs, focus/interaction pins, correction
eligibility and the existing two-pass/four-report scheduling are unchanged.

Three tests fail on the original source because of duplicate rectangle reads,
after checking exact feedback bytes and pin identities. Three new controls pass
there. The first test run also exposed two scrollbar-width assumptions in test
expectations; those failures are preserved separately, and the corrected tests
are identical between the confirmed baseline and candidate. The candidate
passes all 68 existing/new collection, motion and Arrange tests, 242 assertions,
including six new tests. Coverage includes fractional wrapping, stale user-scroll
priority, synchronous replacement with the same view ID/new epoch, and fresh
dimensions on the next pass. Exact source identities and process cleanup are
recorded; no app or Rust build is involved.

This proves fewer DOM geometry calls with equivalent fixture feedback, not a
timing gain or fewer forced layouts. The necessary observer reads can still
force layout, and the temporary map/string work has a cost. The first fresh
runtime comparison follows in §8.55. Evidence:
`target/web-collection-read-reuse-validation/freeze-v1/`, artifact manifest
`d87d34a2a8488619299f1f16a6dd6d23c0ebe80ef246b49d415825e14632dbfb`.

### 8.54 Linux pending-flip control recorder refusal, 2026-09-18

The first paired 1MiB display experiment for §8.50 stopped in its BEFORE arm.
Both fresh references passed, but the control reached the final passive timing
fence with an incomplete observer batch. The AFTER arm was not launched. This
is an overall failed control and no paired performance result.

The raw journal has 1,000 contiguous events and 155 UI-turn batches containing
exactly 4,096 spans. Earlier batches report no recorder failure. Turn154/input48
reaches the cumulative bound with fourteen rows; its LoopTail span then attempts
an Images child before flushing. That ordinary call meets the exhausted total
limit. Maximum recorded per-turn use is 217/256 and nesting is 6/32. The observer
refuses and panics; missing spans cannot be reconstructed as zero-cost work.

Picture33 has already received an installed, live-clean ACK with zero queued
work, but that does not make its incomplete timing batch valid. The saved final
fence remains failed, even though image files and earlier proofs are available.
Failure cleanup sends SIGTERM then SIGKILL; exit -9 is not evidence of OOM or a
production pending-flip crash. All recorded guest/Mac processes and groups retire.
The fixed VKMS display is 60Hz; no physical 120Hz or treatment gain follows.

A separate source-only correction is being tested: 8,192 cumulative spans in
both arms and analyzers, retaining the same 256-row native buffer, depth32,
line/event/byte limits and complete final-picture fence. No timing filter or
workload reduction is proposed. A latent AFTER binding mismatch also needs its
exact 1MiB receiver port guard to agree with the already assigned port5940;
BEFORE remains5939. Neither correction changes this failed record.
The corrected, separately built pair is recorded in §8.56.
Evidence: `target/pending-flip-observer-runtime-before-1m-v1/`, manifest
`d6c54d21c4273cde3aa8637db3ebe23c390ee30b538f2bab9ed78388702c7b25`.

### 8.55 First fresh Web collection-read A/B trace, 2026-09-18

Two fresh cells compare §8.53 with the same immutable `834d4993` Wasm, plan,
Reusable factory and §8.52 trace/driver. Only served `navigation.js` differs;
the unserved build marker updates that file's hash/size. Both pass functional
checks, lossless trace capture and offline attribution, with all 10,000 rows,
actual batch32, silent revision0→8, loaded8→16 and recovery16. There is no retry
or reuse of the historical trace as a timing arm.

| Fixed window | Before tasks / wall union ms / max ms | After tasks / wall union ms / max ms |
| --- | ---: | ---: |
| Idle | 339 / 106.139 / 5.873 | 324 / 96.937 / 6.172 |
| Load | 313 / 130.677 / 8.045 | 333 / 137.478 / 5.925 |
| Recovery | 298 / 75.387 / 3.902 | 283 / 82.786 / 4.278 |

Every cohort has zero fully contained tasks above8.333ms, including the fresh
control. The five historical misses in §8.52 therefore cannot be used to claim
that the change removed them. Total observed task time is mixed: lower at idle,
higher during load and recovery. One pair does not establish a gain or regression.

Each loaded arm records eight key-bearing updates, 109nodes/32keys each, but
six advance/two resize parents become eight advance/zero resize parents. The
Web host's resize entry drains due timers. The control's two heavy resize exports take
3.5/3.7ms; every treatment resize export is at most0.2ms, with producer work in
advance callbacks instead. Loaded resize-containing task union37.543→27.071ms
and maximum8.045→3.979ms consequently do not isolate the geometry-read change.
The full task cohorts above retain that work instead of selecting only resize.

Both cells retain all72 offers/24 input echoes. Loaded boundary geometry agrees;
idle ending scrollTop differs by24px, and idle/recovery resize-task counts are
8→7. Existing snapshots are not full row/glyph/pin equality. Export/task joins
remain partial:110/201 versus85/202 overall. Overlapping script/layout/paint
families and clock uncertainty are not additive CPU or JSON/application costs.
Both loaded traces contain32 layout spans; fewer DOM API reads do not imply fewer
layouts. Crossing, quantized-zero and unscored/cold/primer/silent records remain.

The captures contain13,932,071/13,438,490 bytes,55/53 bounded stream reads,
explicit no loss/EOF/close, and60,910/58,759 events. All five recorded processes,
both groups and both private ports retire. The source change is supported by
its correctness/read-count tests; this pair supplies no demonstrated speedup,
physical120Hz or latency-tail guarantee. Trace overhead remains uncalibrated,
and the known display remains60Hz. Evidence:
`target/web-collection-read-reuse-trace-cell-v1/`, manifest
`6f45517f54c8e62a49e81497a9994c8fe10691488470f612c5dc14474d45ccac`.

### 8.56 First complete Linux pending-flip pair, 2026-09-18

One fresh 1MiB control/treatment pair exercises §8.50 on actual Linux. Both
arms pass two independent reference renders, the candidate workload and the
final publication fence. Each candidate and its bound oracle was rebuilt from
the corrected observer source. The 8,192-span limit applies to both arms;
the 256-row native buffer, depth32, other trace limits, input sequence, 2.5GiB
address-space cap and time limits are unchanged. Boundary tests include actual
old-limit failures, 171 Python passes per arm and seven standalone Rust cases;
these are distinct from the four successful Linux artifact builds. §8.54's
failed 4,096-span control remains immutable.

The treatment demonstrably services input while a display update is pending.
Ten input records occur between submit and readiness for two treatment
pictures: inputs23–25 during picture17, and31–37 during picture21. The control
has no input records inside its blocking submit-to-ready intervals. Thirty
treatment acknowledgments arrive in later UI turns than their paints; the
control's acknowledgments all occur in the painting turn. Matching stamps
join paint, submit, readiness and actual publication without substituting live
state for the submitted picture.

| Recorded work | Control | Treatment |
| --- | ---: | ---: |
| Blocking KMS waits | 31; 288.499ms total; 18.605ms max | None |
| KMS readiness handler | No separate handler | 30; 0.056ms total; 0.003375ms max |
| Publication spans | 32; 0.494ms total; 0.025ms max | 31; 5.432ms total; 1.089ms max |
| Complete UI turns / spans / pictures | 83 / 3,017 / 32 | 178 / 4,250 / 31 |

Readiness and publication remain UI work. The treatment moves display waiting
into the existing event loop; it does not make the display finish sooner or
make acknowledgment handling free. The initial synchronous modeset remains:
its submit takes18.670ms in the treatment. Whole-turn maxima are41.243→34.451ms,
and treatment turn1 still takes8.447ms. Raw turns above8.333ms number31/83→2/178,
but the loop structure and counts differ, so this is not a matched per-turn
speedup estimate or a physical frame-rate measurement.

The two same-width giant reuse jobs have store-to-take intervals15.056/15.122ms
in the control and5.240/0.024ms in the treatment. The control intervals overlap
9.573/8.748ms of KMS waiting and about4ms of observer work each. The remaining
5.240ms treatment interval still overlaps3.966ms of observer work. These are
finite instrumented handoffs including scheduling/unlock, not pure queue or
CPU costs; overlapping frame/observer spans cannot be added together.

All26 proof fields match each fresh width reference. Each accepted A/B crop
matches1,228,800 RGB bytes exactly; both532,224-byte40px overlap checks also
pass in each arm. Four private jobs share two numeric layouts/indexes with
one shape; giant UI construction counters remain zero. Retained-A interaction,
autonomous B publication, coexistence and A retirement pass. Accessible payload
capacity peaks at345,492,154 bytes in both runs, excluding in-progress work,
fonts and allocator costs. Sampled AS/RSS maxima are1,222,612/544,204KiB versus
1,221,116/544,304KiB; these do not establish an isolated memory saving or peak bound.

The raw stderr reproduces all772/1,027 journal records. Final frame32/input48
joins turn82/seq772 in the control; frame31/input48 joins turn177/seq1027 in the
treatment. Both end with the same picture hash, clean live state and no pending
work or unacknowledged later picture. Final trace-flush cost remains censored.
Both runtime wrappers exit0; each display is intentionally stopped with SIGTERM
after observations, and all recorded processes/groups retire. No retry or4MiB
run is included. The display remains fixed1024×768 at60Hz;600→984 is authored
content width. This pair supports removal of blocking display waits while
preserving tested correctness, not sustained120Hz, general latency tails or
an isolated CPU gain.

Evidence: `target/pending-flip-span-budget-runtime-before-1m-v1/`, manifest
`5ebe63eb69c72c788f0f708013044406f0cbac34514b09a21a892f2b87f83b91`;
`target/pending-flip-span-budget-runtime-after-1m-v1/`, manifest
`db3194ff997555c2571dddc7491f5cca23b432e6c25d9c0bb45d1703720f44fc`.

### 8.57 Linux pending-flip confirmation with reversed order, 2026-09-18

A second fresh 1MiB pair runs treatment before control, reusing §8.56's four
immutable binaries and receivers. Only output namespaces and the prerequisite
between arms change: the second control requires this new treatment's full
pass and completed cleanup. Both arms again pass two independent reference
renders and the cold candidate, once, with the same address-space, time and
observer limits. The first pair and §8.54's failed recorder remain unchanged.

The treatment again handles actual input during pending flips: nine input
records, inputs31–37 during picture21 and43–44 during picture29. The control
again has none. All30 treatment page-flip acknowledgments arrive in later UI
turns; the initial modeset is synchronous. Each acknowledgment retains its
submitted picture's stamp, separately identifying later live state. All32
control and31 treatment paint→submit→ready→publication joins close, including
the final desired picture and no later pending C.

| Reversed pair, semantic arm | Control | Treatment |
| --- | ---: | ---: |
| Blocking KMS waits | 31; 306.894ms total; 20.064ms max | None |
| KMS readiness handler | No separate handler | 30; 0.066ms total; 0.006334ms max |
| Publication spans | 32; 0.541ms total | 31; 5.474ms total |
| Complete UI turns / spans / pictures | 67 / 2,777 / 32 | 172 / 4,160 / 31 |
| Whole turns above8.333ms | 32/67 | 1/172 |
| Initial whole turn | 34.382ms | 37.546ms |
| Largest later whole turn | 26.416ms | 8.135ms |

The same-width giant reuse handoffs are16.083/10.002ms in the control and
5.247/6.199ms in the treatment. They include scheduling and overlapping
observer work; they are not pure queue or CPU costs. Readiness and publication
remain counted UI work. The loop structure, turn counts and small-job
admissions differ, so the table is not a matched-turn latency distribution.
The treatment's initial submit still takes21.095ms. Its first-pair8.447ms
post-startup miss remains part of the evidence; this second pair does not
establish a general steady-state bound.

Independent reconstruction matches all724/1,009 raw journal records, all26
A/B reference fields, each1,228,800-byte accepted crop and both532,224-byte40px
overlaps per arm. Final control frame32/input48 joins turn66/seq724; treatment
frame31/input48 joins turn171/seq1009. Both end with the same final pixel hash
as the first pair. Four private jobs, two numeric layouts/indexes, one shape,
zero giant UI construction, retained-A interaction and A retirement remain
verified. Accessible capacity again peaks at345,492,154 bytes; sampled AS/RSS
are1,224,856/546,456KiB versus1,221,116/542,816KiB, with no isolated saving or
strict peak claim.

Both wrappers exit0, reference processes exit0 and candidate SIGTERM occurs
intentionally after observations. All recorded runtime and copy owners retire.
Across two orders, this confirms removal of blocking display waits while
preserving the tested pixels and interaction ownership. Both runs retain
`nonquiet:true`, fixed60Hz VKMS, authored600→984 content width and censored final
trace-flush cost. No4MiB confirmation, physical120Hz, broad latency-tail or
individual-input visibility claim follows.

Evidence: `target/pending-flip-span-budget-runtime-reverse-before-1m-v1/`,
manifest `dd38c02be40113a301cdbc4ee700e89373ce73da3cbc4c66d3723fcb515ae6ed`;
`target/pending-flip-span-budget-runtime-reverse-after-1m-v1/`, manifest
`03eb94b89b3f49b4ee21ef3ef9a0922c6b2a92e0795d7740700d81cfd60d4bf1`;
paired summary manifest `94e2f44c534c3d1f40923f3cfdd4549f75b3959f3b67603609c0108fe4c027d4`.

### 8.58 Pending-flip 4MiB correctness and retained-cap confirmation, 2026-09-18

One current Linux treatment run extends §8.57 to4MiB. It reuses the same
candidate and oracle binaries, with no rebuild or receiver change. Two fresh
600/984-width references and one cold candidate pass under the unchanged
2.5GiB address-space cap,60s process limits and220s outer limit. The binding
selects only4MiB and explicitly checks the sealed successful1MiB prerequisite;
it neither copies a substitute result into the new output nor reruns1MiB.

All26 reference fields match at both widths, including complete4,194,149-byte
canonical coverage. Each accepted viewport crop matches1,228,800 RGB bytes;
both532,224-byte40px interior overlap checks pass. These checks cover the
accepted crops and specified overlaps, not every displayed pixel at every
input. Four distinct jobs share one shape and two layouts/indexes with two
cache hits. Giant UI construction remains zero; retained-A interaction,
autonomous B publication, coexistence and A retirement pass.

Independent raw reconstruction reproduces1,132 events,190 turns and4,733 spans,
below the unchanged8,192-span limit, with no recorder failures. All38 submitted
pictures reach matching publication;37 acknowledgments cross UI turns. Nine
inputs occur during pending flips: input1 during picture2,31–37 during
picture25, and45 during picture37. Final picture38/input48 joins turn189/seq1132
with no later pending C. Trailing trace-flush cost remains censored.

Sampled AS peaks at2,408,984KiB and RSS at1,771,596KiB, leaving212,456KiB observed
AS headroom. Accessible deduplicated payload capacity peaks at1,382,011,614
bytes; in-progress work, fonts and allocator overhead are excluded. These
numbers establish fit for this fixture/run, not a strict peak or memory saving.

Blocking KMS waits remain absent; the37 readiness handlers total0.113ms and
remain counted UI work. Whole turns still include34.431ms startup and9.830ms
on the next turn:2/190 exceed8.333ms. This single treatment cell is correctness
and cap evidence, not an A/B speed comparison. Instrumentation remains
`nonquiet:true`; the fixed60Hz VKMS display and authored content-width change
do not establish physical120Hz or continuous physical-resize performance.
Both references and the outer wrapper exit0; the display's SIGTERM follows
completed observations. All recorded runtime and copy owners retire, with no
watchdog action or retry. Earlier failures and both1MiB pairs remain unchanged.

Evidence: `target/pending-flip-span-budget-runtime-after-4m-v1/`, manifest
`1b46d9e363667c87b2f0a1a669b79f46569d86e0282cf83a2dea7ef09f1840c9`;
report `aeaf30fa44f6cc20fb3de5ee6dd32c3ab3d2524e323c424ad607cdff30d8fb99`.

### 8.59 Native Messages memo-miss diagnostic, 2026-09-18

One fresh optimized Mac capture and one native cell extend the borrowed-text
treatment with bounded cache witnesses. This is an ignored diagnostic overlay,
not a production cache-policy change or A/B comparison. The existing256-owner,
four-offer FIFO, exact two-axis keys, metrics and lifetime rules remain unchanged.
The10,000-row/batch32 workload, ordinary timer, resize/type/wheel recipe and
4,096/512 observer limits are unchanged. The original96-byte cost reply is
preserved inside a224-byte reply carrying16 additional scalars in eight rows.
Actual compiler inputs include the new Rust helper and checked C header; the
generated Reusable factory, plan and compatibility bytes match the prior treatment.

Independent raw reconstruction agrees with all54 complete scored resize parents,
18 per phase. Each has all eight ordered, correctly owned diagnostic rows, valid
integer partitions and no diagnostic flags or primary trace omissions.

| Scored resize calls | Idle | Load | Recovery |
| --- | ---: | ---: | ---: |
| Identified memo lookups | 5,604 | 4,980 | 5,016 |
| Memo hits | 2,605 | 2,484 | 2,520 |
| Stable-owner offer misses / Swift callbacks | 2,999 | 2,496 | 2,496 |
| Exact evicted-offer recurrence witnesses | 1,367 | 1,188 | 1,188 |
| Not previously admitted within covered interval | 36 | 285 | 48 |
| Unknown beyond retained history | 1,596 | 1,023 | 1,260 |

All7,991 scored misses belong to still-valid resident owners whose exact offer
is absent. These parents record zero absent owners, metric invalidations, invalid
raw results, owner evictions or plain callbacks. That scope does not include
every producer/timer call. There are7,938 offer evictions and7,663 witness
overwrites. The3,743 positive exact recurrence witnesses are46.84% of scored
misses; they support testing offer retention. The remaining3,879 unknowns are
not evidence of novelty. The369 covered misses mean not previously *admitted*
within a fully covered current metric interval, not first-ever requests.

Observed exact-witness gaps reach340/596/304 global identified-get ordinals.
They include other owners and accesses outside scored parents, and are neither
per-owner working-set sizes nor FIFO/LRU distances. They cannot select a cache
capacity or predict the number of hits a different policy would achieve.
Counts are not time shares. The fixed73,776-byte diagnostic bookkeeping bound
excludes existing cache payload and allocator overhead; its comparisons and
recording add work, so these timings are not a speed comparison.

Whole resize maxima remain15.617/15.686/13.700ms, with38/54 above8.333ms. Three
of seven loaded timer chains also exceed that budget, maximum13.088ms. The
command-free interval has nine complete producer chains and revision+9;
four exceed8.333ms, maximum13.430ms. All36 input handlers complete inside their
edges, including18 directional40px wheels, but typing ACK reaches32.295ms and
wheel ACK22.022ms. Passive observations overflow, so mode remains
`NOTESTABLISHED`. The display is still60Hz; no physical120Hz or tail bound follows.

The build and cell each exit0 once, without watchdog action or retry. All four
external posters release their owned input; all recorded processes/groups retire.
Reader validation retains116 distinct pure checks and meaningful refusal REDs.
The dependency-symlink and shebang setup corrections are preserved separately;
neither changed native source or the workload. A modest bounded offer-retention
experiment is the next candidate, with exact metrics and both axes retained.

Evidence: `target/native-630-memo-causes-native-v1/`,579 files/125,751,386 bytes;
artifact manifest `23126504c7ec16c158ce390579c1b2b1a46db1b43cf53d380fd682647a1fa612`;
report `d7b738472b2c5449e022fd72ee1538e071c91d3716ced6a00f37bd870ea2409c`.
The immutable build capture is
`ca5858a2bf6edfc032e9c1d91fb60f96ffbbd9c85e1faa3d8ce5f5a72aae283d`;
source/binding archive `b1ce06f0e5d1005e3c67d4dda384b0286c3e2dd6104accc43ca433bdb004cfc6`.

### 8.60 Eight exact text offers per native cache owner, 2026-09-18

The Apple identified-measurement memo now retains eight exact width/height
pairs per owner instead of four. The256-owner bound, FIFO insertion policy,
metric/source stamps, raw-result validation and exact two-axis bit keys are
unchanged. It retains scalar metrics, not text/font payload or worker jobs.
This is the bounded retention experiment motivated by §8.59; the diagnostic
witness book and224-byte ABI are not part of the production change.

Corrected baseline tests record four behavioral failures and one occupancy
assertion after the lifetime control passes; the eight-offer candidate passes
all14 scoped tests and strict all-targets Apple Clippy. A real Host/Runner/Taffy
resize fixture reduces actual foreign callbacks17→11, with all17 request
width/height tuples, full metric answers, frames and output batches equal to
an uncached oracle. That fixture uses a deterministic callback, not CoreText.
Tests retain exact axis bits, intrinsic offers, FIFO ninth-offer eviction,
invalid-result retry, namespace/owner lifetime and existing source controls.

The allocation fixture records requested live capacity92,680→158,216 bytes,
a65,536-byte increase; occupied owner payload increases32,768 bytes. The hash
map's448 usable entries are not its512 inferred buckets. These are observed
allocator requests for that fixture, not RSS, allocator size classes or a
future churn peak. There is no new production allocation type or worker.

One optimized candidate build changes only the constant against the retained
borrowed/prefit control. Actual258 source/header cards, compiler/SDK/settings,
77 configuration units, generated Reusable entry, plan and compatibility bytes
match apart from that file. Both arms use the original C96 observer, full10k
history/batch32, ordinary timer, fixed resize/type/wheel recipe and4,096/512
quotas. Two fresh pairs run in opposite orders; no old cell supplies timings.

| Comparison, control → eight offers | First A→B pair | Reversed B→A pair |
| --- | ---: | ---: |
| Scored resize parents | 54→54 | 54→54 |
| Identified lookups | 15,600→15,600 | 15,600→15,600 |
| Swift measurement callbacks | 7,991→5,066 | 7,990→5,060 |
| Full Swift callback wall sum, ms | 313.870→268.369 | 331.263→264.246 |
| Native+entry wall sum, ms | 349.820→305.415 | 369.166→299.756 |
| Whole resize union, ms | 515.306→475.561 | 543.167→483.031 |
| Whole resize calls above8.333ms | 35/54→33/54 | 36/54→33/54 |

Each phase delivers the same18 viewport dimensions. Full accepted-step and
strict AppKit-inside resize cohorts coincide; a queued last delivery can start
after QuartzUP while still inside AppKitdidEnd. Full equal-argument histories
match. Clock/cadence and timed revision progress are observed, not forced.
The count reduction repeats at about37%; summed whole resize work is7.7% and
11.1% lower in these two pairs. Callback spans are inclusive full Swift wall,
not CoreText CPU, and are not added to the parent native/whole spans.

This is a useful reduction in repeated measurement, not a120Hz result or a
uniform latency improvement. First-pair loaded whole median worsens8.872→9.236ms,
loaded typing ACK maximum10.257→29.312ms and wheel2.524→12.333ms. Both first-pair
silent intervals have five of eight producer chains over8.333ms; their median
worsens9.430→9.848ms. Reversed-pair recovery resize median worsens9.251→10.064ms
and loaded timer maximum10.172→12.574ms. All these negatives remain beside
the lower totals. The instrumented display remains60Hz, passive observations
overflow and mode stays `NOTESTABLISHED`; no physical120 or broad tail bound.

The reversed pair also retains a worse loaded typing ACK maximum12.751→19.942ms;
its command-free samples differ, eight control chains versus nine candidate.
All four cells exit0 without retry or watchdog action, all16 external posters
release input, and their28 recorded PIDs/20 groups retire. Integrated validation
passes14 distinct measurement tests and strict Apple all-targets Clippy; no
whole-workspace or new native-demo build is claimed.

Evidence: `target/apple-memo-offers-validation/freeze-v1/`, source/test manifest
`06e7362b9c678a7590d5c2dca77e1d7c4ab8d75a9eae282dfebff12f29ab30bc`;
first native pair `target/native-630-offers-pair-v1/`,1,052 files/200,556,632 bytes,
manifest `f3cd8dbd3aca03a9b873ceb6982bd19b6cc88c6f47732c8976829c76d3e89614`;
reversed pair `target/native-630-offers-reverse-pair-v1/`,379 files/145,723,390 bytes,
manifest `e6e32fbcb9b58821ce09b66041ff71fc844dcf68c055b68d20544cca998084fb`.
The original test authoring, pre-launch SDK-variable check and offline setup
failures remain in their archives; none is a native retry or a retuned workload.

### 8.61 Reuse collection feedback for observer baselines, 2026-09-18

The Web collection adapter previously read every mounted row's rectangle during
commit to establish observer baselines, then read those rows again in its queued
measurement pass. Eligible commits now defer only row baselines to that existing
pass. List and port baselines remain immediate. The final dependent commit also
retains immediate row reads: its feedback budget is exhausted, so no later pass
can safely supply them. No new timer, queue, frame cache or scheduler is added.

The same real rectangle supplies the typed row height and observer width/height.
Even unchanged feedback refreshes width-only baselines. Samples stay local to the
DOM pass and are cleared before synchronous reporting can replace Elements or
epochs. Deferred own notifications coalesce without replenishing the two-pass
budget; external changes still replenish it, and port growth retains the existing
before-paint flush and four-report bound. An unmeasured baseline is not a guessed
row height. Workloads and application data remain unchanged.

The corrected old-source test passes exact wire, fractional height, large epoch,
focus/interaction pin and real observer-idle checks before failing its read-count
assertion. Each row has one read at commit, two cumulatively by feedback and three
by observer delivery; the candidate has zero, one and two. Four baseline controls
pass. An initial observer-constructor setup failure is retained separately.

Six new tests bring the exact source suite to 74 passing tests/271 assertions.
They cover callback ordering, own-only termination, external growth, width-only
feedback, replaced Elements and hidden restoration. A separate negative control
that always defers row baselines loses external growth coalesced after the last
own commit: the observer sees 97.25px while accepted feedback remains 41.5px.
The integrated fallback preserves 97.25px with the exact wire and returns to idle.
All owned test processes terminated; the final tested source bytes match the
integration, without another test execution being counted as additional evidence.

This proves fewer rectangle API calls in eligible commits, not fewer browser
layouts or a frame-time gain. Other style, list, port and scroll reads remain;
observer delivery still samples current bounds. A fresh application trace pair
is separate work. Physical120, Storm scheduling and latency tails are unproved.

Evidence: `target/web-observer-baseline-validation/freeze-v1/`, 33 artifacts /
1,381,354 bytes, artifact manifest
`7e365f0ec8484463ae0713a7267f8a5f46720a4f7b9475b1e60a68ae7093799f`;
source manifest `2616287b945b5d55d7ed54f7849737118415a77ded679d3bf6fce596f6a8cb62`,
two-file patch `100aa303c9c3a88ec7c200e36c37dc06195bdd1eb8f64a307861a3bb7a62a0ef`.

### 8.62 Fresh browser observer-baseline comparison, 2026-09-18

One fresh control then candidate pair measures §8.61's navigation change with
the same instrumented Wasm, plan, Reusable factory, glue and driver. Both cells
pass functional checks and capture complete traces. Only served navigation and
its unserved build-identity card differ; the server verifies the exact Buffer
it serves. This is not an independent browser-body digest or a rebuilt Wasm.

Both arms supply all10,000 records with32 revised records per loaded update.
Silent progress is0→8, loaded8→16 and recovery16→16. All144 scheduled offers and
48 input echoes complete across six fixed2s phases;162 fully contained rAF gaps
are recorded. Loaded/recovery boundary geometry matches. Idle starts at different
scroll offsets,328319 versus328343, then ends at matching port facts. Sparse
snapshots do not establish all-frame geometry. Wheel-caused movement remains
`NOT_ESTABLISHED` because resize and anchoring also change scroll offsets.

Selected renderer tasks wholly contained within conservative clock bounds:

| Phase | Control count / union / max ms | Candidate count / union / max ms |
|---|---:|---:|
| idle |349 /120.331 /5.505|313 /102.417 /5.301|
| loaded |276 /117.220 /6.319|331 /137.818 /6.431|
| recovery |306 /85.001 /4.252|330 /87.580 /4.161|

None of these contained tasks exceeds8.333ms in either arm. Crossing tasks
remain separate:2/2/3 control and2/3/2 candidate. Loaded key evaluation has eight
`exact_advance` parents per arm, each109nodes/32keys. But loaded resize tasks
number7 versus8, collection-feedback exports15 versus21, and total task counts
differ. Matching revision progress and key-parent ownership do not make the
complete browser work identical. The single pair establishes neither a timing
gain nor a causal regression; idle decreases while load/recovery increase.

Loaded Script/style/layout/paint unions are77.936/1.348/15.510/10.320ms versus
85.442/1.324/16.995/16.086ms. These intervals overlap each other and whole tasks;
they are not additive CPU costs. No named selected-thread event isolates the
collection ResizeObserver callback. Observer-specific cost and layouts saved
remain unavailable despite the source tests proving fewer rectangle reads.
Setup retains an unscored13.2ms control dispatch. rAF and echo timings are not
physical presentation; the known display remains60Hz and120Hz is unproved.

Independent raw reconstruction matches both target-frame renderer/thread choices,
clock brackets,9,943/10,402 normalized spans and1,691/1,791 complete task spans.
All six scored task/family unions and crossing counts match. Conservative export
joins identify80/198 and104/201 calls; unavailable joins are not assigned by
nearest timestamp. Both cells and their supervisor exit0 once, without retry or
cleanup action; all five recorded PIDs/three groups and both private ports retire.
The unchanged trace overhead is uncalibrated. No additional browser run or new
instrumentation follows from this comparison.

Evidence: `target/web-observer-baseline-trace-cell-v1/`,30 artifacts/34,422,503
bytes, manifest `d760378de413c0fe7bdc10e8e4ae3f28150040cc10b0e8dea7ef1cedee55c7c5`;
report `8d0cb401189da027e521327b80cf5914d2ba1f32fe35a19742d9c5b6ea600016`.
The earlier §8.55 pair and §8.61 source-test failures remain unchanged.

### 8.63 Unicode boundary reuse: mixed first native pair, 2026-09-18

One fresh native Messages control then candidate pair tests reuse of immutable
Unicode line endpoints across widths. Both retain Reusable Messages, prefitting,
borrowed text and §8.60's eight offers. Only Text.swift and TextResidency.swift
differ. The candidate adds bounded, residency-charged endpoint storage and locale
qualification; the nil-tokenizer policy stays unchanged. Locale identifiers,
preferred languages and two CFPreferences reads qualify reuse, with another
qualification during construction. Their cost is not separately measured.

The new candidate build/capture and both fresh processes pass. Each arm delivers
all18 accepted viewport dimensions in each of three AppKit resize intervals;
all54 complete C96 parents are inside those intervals. Each final resize begins
after Quartz UP but finishes before AppKit didEnd. Window/port geometry, accepted
dimensions and external points match; backing scale is not separately recorded.
Full10k/batch32, normal clock and all72 input offers/ACKs remain intact, including
36 directional40px wheels. All16 equal-argument full History comparisons match.
Loaded progress is8→18 in both arms; recovery/final19 versus18 is retained.

| Phase | Whole resize union, control→candidate ms | Swift callback sum ms | Whole >8.333ms |
|---|---:|---:|---:|
| Idle |108.527→158.482|60.908→81.499|4/18→11/18|
| Load |151.307→147.839|83.795→77.338|10/18→10/18|
| Recovery |156.674→142.526|90.464→74.170|9/18→10/18|

Pooled whole resize work rises416.508→448.847ms (+7.8%); native+entry is
266.130→269.986ms and callback wall235.167→233.007ms. Idle callback counts differ
1970→1955; load/recovery match1536/1560. These are inclusive wall spans, not CPU
or tokenizer-only costs, and exclude surrounding prefit work outside resize.
The native observer does not count tokenizer construction or boundary-cache hits;
the separate source fixture's3→1 construction result is not native reuse evidence.

Candidate recovery resize max14.530ms, loaded timer max12.824ms and idle wheel
ACK max17.969ms remain. Command-free intervals contain8 complete timer chains per
arm; whole misses rise3/8→4/8 and medians7.396→8.247ms. Loaded timer misses rise
1/7→2/7. ACK includes transport/queue/handler, not frame latency. Primary records
have zero omissions; passive quota saturation keeps mode NOTESTABLISHED.

The fixed-order pair establishes no aggregate benefit or cause for the mixed
phases. The candidate remains private, with no production activation or automatic
repeat. Recorded60Hz hardware does not prove physical120Hz. Both cells exit0
once; all14 owned PIDs/10 groups retire, with eight poster UPs and ports free.

Evidence: `target/native-630-text-boundaries-pair-v1/`,921 artifacts/201,166,864
bytes, manifest `2aaf18c381f83a6da7785fcd7bf6028bd673e633c6094068c8ddb7f23275cdf4`;
report `a5d563fc02f331aeefa05f4d548646707d4c0113c2367fc9846ba56e247377b0`.
Independent reconstruction of all108 raw parent/C96 joins and feedback unions
agrees. Previous control captures, source-test failures and timing cells stay sealed.

### 8.64 First Mac giant-paragraph live resize: input works, output is late, 2026-09-18

One fresh e494 Markdown process exercises the complete1MiB fixture during an
actual Quartz/AppKit resize. The document is1,048,531UTF8 bytes; its unsplit
paragraph is1,048,499UTF8 bytes/999,569UTF16 units. The final source identity is
unchanged. Paragraph width940→856 produces8155→8988 lines and height212030→233688.
The app stays in the full-size, wide, manual, paused paragraph configuration.
This is not a4MiB, mixed-Markdown or code-block result.

The native live-resize interval lasts1800.118ms with18 changes. All six typing
handlers and six ordinary wheel handlers, plus their ACKs, are inside it. Wheels
move the actual RegionSurface alternately40/-40px; final actual/raster/layer
scroll offsets agree at0 and the final typed echo matches. The external sequence
is down,20 drag events and up. Inputs use the normal agent route, not physical
keyboard/trackpad events; there is no clock-settle pump or in-edge state polling.

No accepted raster plus current model-layer assignment is established inside the
resize interval. Two successful completion-return markers do not prove accepted
output. The first post-edge checkpoint has publication/candidate16 but
visiblePublication0 and displayable=false; retained visible paragraph pixels
through resize are therefore not established. Final publication41/artifact40
matches the current source and geometry, but raster acceptance is3660.260ms
after the native edge and model-layer assignment4122.117ms after it. These are
model-layer timestamps, not screen presentation evidence.

Two complete worker shape spans take2668.770ms and2548.628ms. The first overlaps
the edge; the second is entirely afterward. Final indexing takes77.576ms. Clipped
instrumented unions within the edge are8.939ms on UI and1653.490ms on the worker;
these cover selected hooks, not all UI work or isolated CoreText/CPU cost. Nested
stage durations are not additive. Input responsiveness and eventual correctness
do not establish continuous reflow or120Hz. The result remains
FUNCTIONAL_IN_EDGE_PUBLICATION_NOTESTABLISHED.

All36 positive tracking witnesses are in the primary trace. Passive quota
saturation keeps mode NOTESTABLISHED; primary179/4096, UI468/8192 and worker101/8192
have no omissions or nesting mismatches. Saved replies15,174,032B stay below64MiB.
One build/capture and one runtime exit0 without retry; four owned PIDs/two groups
retire with poster UP. The initial missing-header receipt assertion is preserved:
an added record of the actual Bridge.swift.o dependency rule, canonical header
and module map, and dependency mtime within the owned build supplies provenance
without rewriting the receipt or rebuilding.

Evidence: `target/apple-giant-live-e494-execution-v1/`, runtime manifest
`6058cd8fa7c965bc09192868809c443676b527d0824cf001d5308778858602dd`
(48 files/16,866,842B), report
`b36e777391842ce58f774cbdfdbd8c1955f373fb884bc4d891b6a9b53908e983`, and
offline closure `f70889298a7c765542c005ca3164429c47ece70994ed2dc3a3831527f0bb1bd3`.
Independent raw reconstruction agrees on the clipped stages and late publication.
The next source investigation targets full worker shaping and stale-job delay;
the existing ordered-worker boundary and full paragraph remain the workload.

### 8.65 Linux viewport witnesses and idle resize correction, 2026-09-18

The Linux presenter now retains viewport size and scale with each acknowledged
picture's existing source, bounds and scroll witness. While B is submitted and C
is live, root scroll limits and pointer admission follow acknowledged A; after
B's acknowledgment they follow B until C is presented. Direct DRM pointer
mapping refuses mismatched extents/scales. A mismatched physical release cancels
the held interaction rather than dispatching a successful drop; keys and cancel
remain available. Headless presentation keeps immediate behavior.

Review found a second idle-resize case: A at scroll60 can submit B using that
offset, then B's acknowledgment clamps the offset to0. With no C, timer or input,
clearing dirty left B's old-offset pixels indefinitely. Clamp now reports actual
offset changes/removals and acknowledgment preserves dirty when needed. A plain
root regression proves one correction reaches rootY0 and clean state, retaining
A60 while pending and immutable B pixels. The original failed version is sealed.

Validation is29 scoped Mac tests plus4 actual Ubuntu AArch64 pointer tests
(two new viewport/scale/cancel cases and two existing controls). The reviewed
five-file patch also passes strict all-target package Clippy on both platforms.
Original4 behavioral failures/two controls and the later idle-clamp failure are
preserved. Actual Linux uses one fresh release test ELF with exact staged source
and unchanged dependency/tool identities; it is not a display or compositor run.
No additional whole-package test or performance result is claimed.

MAIN's five source hashes exactly match the tested freeze. Package formatting,
caps and boot checks pass. Workspace-wide formatting fails in61 external
Snapback files, with no differences under MAIN; that output is retained, not
reported as a passing workspace check. A real Linux window configure/resize path
remains separate from this presentation and interaction foundation.

Source manifest `b39638ede05d34f0f97a3d460cd18e96f75a6578ae30069f8a9bcf2569170350`,
patch `6514a26298a41c33ad17819ed7f4230480ec15745f9e22cbdbe2af12c891317b`, archived
under `target/linux-presented-viewport-validation/freeze-v2/`. Actual Linux
evidence is `target/presented-viewport-linux-execution-v2/`,68 files/18,990,611B,
manifest `a70930b8961f30fe2b207ef2c6da41981d9f5c3d8f5ecaa27601430b5cf8b286` and
report `aee7213b0ee337fcca2b1c36488893636a9cc5853087f148ff0e037ac3c1838f`.

### 8.66 Reuse captured authored ranges during Apple worker shaping, 2026-09-18

The worker's glyph-run/authored-run intersection used to rebuild each authored
run's cumulative UTF16 interval with `NSString.length` on every visit. The source
already owns those exact immutable ranges. The worker now compares their stored
endpoints, preserving strict intersection, complete loop order, coalesced runs,
explicit line heights and normal/fallback metrics. No cache, worker, cancellation
or publication policy changes; source capture still performs its original work.

Both baseline and candidate pass19 standalone methods/28,104 assertions against
actual production Swift sources. Three new methods cover empty and touching
intervals, combining/astral/RTL/replacement text, coalesced authored line heights,
wrap/clamp and fractional widths. Exact glyphs, geometry, carets, hits, source copy
and bitmap comparisons agree with the unchanged ordinary text engine. The initial
fixture did not actually coalesce its runs; its failed guard is preserved, and the
corrected fixture requires coalescing rather than weakening the oracle.

An ignored test wrapper at the actual old conversion site counts3,417 evaluations
in the baseline and0 in the candidate. Both are semantic passes; this is removed
work, not a manufactured correctness failure, allocation count or CPU measurement.
Strict Swift6 optimized compilation passes. The standalone assertion shim is not
full XCTest, and these checks establish no full-document native speedup. A fresh
native comparison uses the same complete1MiB paragraph and resize/input recipe;
the previous cell's timing is not its paired control.

Exact two-file patch `da171d0726fa570ddb3102d40aa25ea84b109806094d8603df007231acad5d20`,
production-only patch `60a488c447e6b354e00f812e2159e361bf0b5371ea78e4c0d956bbfe72ce8ef7`,
and90 artifacts/2,790,302B are preserved under
`target/apple-region-range-validation/freeze-v1/`, manifest
`4f3acfa47a9e3d30310a46bb68700627e5e4f2126b88492e9bfaad2fe886ad16`.
MAIN matches the tested production and test bytes; unrelated compiled dependency
identities, caps and boot checks are retained. The multi-second publication delay
and absent in-edge paragraph layer in§8.64 remain uncorrected measurements until
the new native experiment establishes otherwise.

### 8.67 First range-reuse native comparison has unmatched geometry, 2026-09-18

One new optimized candidate build and two fresh full1MiB cells complete with
typing, directional wheel movement and eventual correct source/publication.
The candidate differs only in the worker range change from§8.66; generated entry,
plan, compatibility and existing diagnostic sources match the control. Neither
cell establishes a new accepted raster and layer assignment inside live resize.

The intended timing comparison is not valid: macOS restored the prior saved
window after processing requested size overrides. Actual paragraph widths are
856→772 for control and772→688 for candidate, with different line counts. Each
cell's existing oracle correctly checks its actual window but does not require
equal geometry between cells. This setup failure was found offline; both records
remain intact, with no retry or inferred speedup. Future comparisons must check
actual starting geometry before admitting either measured edge.

The candidate still spends2,241.554ms in the full shape crossing the edge and
2,131.356ms in the final-width shape afterward, followed by79.058ms of index work.
Its final layer arrives3,101.454ms after resize; the control's arrives4,205.302ms
after its different-width edge. Those are separate observations, not a gain.
Both first post-edge samples have visible publication0 and displayable=false.
Each has six typing and six actual±40 wheel handlers/ACKs inside18 native resize
changes. Primary/cost recorders have no omissions; passive mode remains unknown.
No physical presentation, destination pixel comparison or120Hz result follows.

All eight recorded PIDs/four groups are retired with both posted mouse-ups saved.
Evidence is `target/apple-giant-range-pair-e494-v1/`. Independent selected-raw
reconstruction agrees on source, widths, worker spans and publication delays;
parent and nested worker costs are not added. The next cost discriminator separates
typesetter setup, Unicode boundaries, line construction and numeric caret/hit
metadata, before choosing another optimization or scheduling change.
Manifest `bfab2998a2cc124bbfaf7a0fa54fdad6375945837e38979bfad0ccb398ec6a7c`
records190 files/85,284,262B; report
`f95ce87b41a3d318d1eafc3d4cac58ab9c5c80a75785bfe6bd7b660ff4629412`.

### 8.68 Giant worker time is concentrated in interaction metadata, 2026-09-18

One standalone optimized worker diagnostic separates four consecutive phases on
the complete canonical1MiB paragraph. It uses production d9f928d sources plus
ignored scalar probes: four child rows/eight clock reads per full construction,
with no per-line or per-caret clock. The inclusive service shape takes2712.364ms:
attributed source/typesetter45.071ms, Unicode boundaries9.586ms, line breaking and
metrics16.697ms, numeric caret/hit metadata2639.431ms, and remainder1.581ms.
The children are disjoint and contained in the same request/generation/offer;
the inclusive parent is not added to them. Metadata occupies97.311% of this
fixture's parent wall, including any deferred framework work inside its constructor.
It does not isolate individual CoreText calls, sorting, allocation or CPU cost.

The body remains1048499 UTF8 bytes/999569 UTF16 units at width940, with8155 lines
and212030 height. Three distinct height offers admit one full construction and
two shared-backing hits with fresh request/artifact wrappers. Full source copy,
streamed geometry/caret digest, font/Spec, sampled hits and selection oracle agree
exactly with the retained reference;601600 bitmap bytes and3144 ICC bytes match.
Source capture separately takes12.347ms, outside the shape parent. The fixed plain
Spec is source-derived, not recovered native wire; there is no controller, window,
live resize, adaptive palette or physical-presentation measurement in this helper.

Twenty-two synthetic attribution checks pass. One strict Swift6 optimized compile
and one guarded process exit0, with unchanged inputs and both owned groups gone.
The original missing dependency-file result stays recorded as false. A separate
proof verifies this build's retained CExact compiled module, its explicit header
and module-map inputs, and creation time within the owned build; no recompilation
or oracle change occurred. Independent raw reconstruction verifies all seven spans,
source/output equality and that provenance correction.

This result directs the next change toward bounded viewport interaction metadata
while keeping full source, exact layout, worker-confined CoreText and qualified
pixel/hit publication. It is neither an implemented optimization nor a native
speedup/120Hz result. Source and evidence are archived at
`target/apple-region-shape-stages-validation/run-1/`:93 artifacts/5,283,884B,
manifest `d559ff375d551200fb6449f69b58b795e8141b9f299b256f1cbb0c3fb0ae2d8e`,
report `dd6137da3127cfd6e43f2b2f154bec24e66d60f36431fc6b91f4a9ac67b6d36e`.

### 8.69 Messages runtime entries use immutable row reuse by default, 2026-09-18

The Web, Apple and Linux Messages stress entries now select the already tested
`ReusableMessagesStress`. The app-local build selector
`EXACT_MESSAGES_SOURCE=reuse|stateless` preserves the original stateless control;
unset selects reuse. Empty, unknown and non-Unicode values fail before Contract
compilation or bake. Web dev consumes the same compile-selected factory and does
not consult the selector at runtime. Switching a reused Web dist still requires
rebuilding its Wasm with the same selector, as documented in the app README.

The embedded default-page bake and grants remain stateless. Actual generated
products in both modes have identical plan, compatibility and artifact records
on each checked target/platform; only their runtime factory changes. The full
10,000-row/32-change canonical values agree, with9,968 unchanged record owners
shared by reuse and none shared by the control. Cardinalities, Contract, finite
producer cadence, eager/windowed controls and Exact Live are unchanged. This
activates the previously compared composition; it does not remove O(N) handle
copying or positional traversal, add a worker, or establish a new timing gain.

The final default data/Web suite passes35 unique tests with one existing opt-in
timing ignore, including nine new tests. The stateless factory/dev repeat passes
three tests separately. Actual old-default selection fails the new expectation;
three old-script malformed-selector cases write plans, while all nine final
platform/case combinations refuse before outputs. An earlier missing-Cargo-PATH
setup failure remains separately recorded. The built dev executable also writes
the same baked plan under an invalid runtime selector in each compiled mode.

Both modes pass scoped all-targets Rust checks/strict Clippy and Web wasm32 checks.
Apple/Linux entry checks here run on the Mac host; there is no new full native
build, iOS/Linux OS run, GUI, performance comparison or physical120Hz claim.
The exact nine app paths and143 artifacts/1,539,074B are archived at
`target/messages-runtime-activation-validation/freeze-v1/`, source manifest
`e2e7e6210ba92e13fb98c613bf2f6aeb9008f6a62df07259e091477791598636`,
patch `6904f5dc8c8b53ce81a798b35c769d84b1cd712bd3a186e928af23e4f6a76725`,
artifact manifest `28647b960fe3081c9540054476c6563b6de071bb811f4f8b7943c29ae12ea22d`.

### 8.70 Apple worker hit metadata follows the viewport, 2026-09-18

The worker now retains complete source, CoreText lines and numeric line geometry
without eagerly constructing every UTF16 caret table. Each raster may carry an
immutable viewport hit slab; an uncovered point is answered exactly by the same
serial worker. Missing coverage or admission refusal does not invent an index or
reject otherwise valid pixels. Full layout, source copy, wrap and extent remain
intact, including the single unwrapped1MiB line control.

Each slab reserves4MiB before allocation, with at most two owners/8MiB per service
lifetime. Last backing ownership, including aliases, releases the charge. The
directory, endpoint/answer arrays and enumeration/sort storage fit inside that
slab; admission considers at most512 logical hit-selected lines and8192 UTF16
caret units. Oversized lines bypass dense construction. Independent review found
that the first version's Swift buffer sort allocated temporary arrays outside
the slab. The final version uses in-place heapsort/deduplication with scalar
scratch. Platform/CoreText internals, full source/line heaps, pixel and index
accounts remain separate; this is not a process-memory or allocation-peak bound.

Pixels, hit coverage and any point reply form one qualified output. Contacts keep
their initial and latest/terminal points until an anchor resolves, then one
latest point. Quick down/drag/up while the worker is blocked completes once;
stale or replaced gestures cannot adopt its answer. Fresh hits require current
displayable pixels. Selection-only pending pixels preserve their qualified
contact, while phase, size, palette, publication, refusal and reset retire it.
No additional worker, queue or application state channel is introduced.

The original production baseline reaches the eager-expansion regression and
fails one of three assertions. Final strict Swift6 optimized helper compilation
and31 methods/28,246 assertions pass, including existing exact geometry, caret,
selection and bitmap references, sparse/out-of-view lookup, ownership limits and
98,339-element sorting controls. A test-only synchronous reference initially ran
on the main thread and trapped; its async correction and original failure remain
recorded. Final ExactKit typecheck passes with70 captured sources. These helpers
use an assertion shim, not the full XCTest runner or an iOS SDK build.

A separately compiled AppKit fixture passes110 assertions through actual
RegionInkView NSEvent handlers, a window, controller and worker. It covers cached
multi-move selection, beginning over selection, blocked sparse terminal input,
out-of-view continuation, gesture ABA and seven cancellation conditions. Runtime,
Presenter and session boundaries are doubles; there is no OS input injection or
full-app/physical-pixel claim. That binary predates only the final sort correction
and is retained as such, rather than attributed to the final source. The final
helper suite rechecks hit/pixel equivalence after the sort change.

Exact seven production and three test paths match the frozen compiled inputs.
Evidence is `target/apple-viewport-hits-validation/freeze-v2/`,448 artifacts/
9,124,106B, manifest
`01e1d7676154e579e35e39f182ed621eda6e11a196b0f28b522e847b0aac0f40`,
production patch `706e099efee0bcd43f81eae761e31656ae83207eb0b023ab61d01aedf5e64a3c`,
complete patch `72b30a649ed2b339d44373554091c81e965bd6b481a6d9e724d7b8b113e7e84f`.
The original freeze and sort-review correction are preserved. Long-line exact
CoreText calls and full shaping remain indivisible worker work, and a point can
wait behind shaping. Existing resize visibility restrictions still apply. Native
speedup, in-edge layer publication and120Hz remain unmeasured for this change;
the next comparison admits both fresh cells only after actual starting geometry
and source identity match.

### 8.71 First geometry-matched viewport-hit native comparison, 2026-09-18

Two fresh optimized products and one control→treatment pair keep the complete
canonical1MiB paragraph and existing18-step external resize, six typing offers
and six directional40px wheel offers. Both cells admit actual980×852 windows,
980×820 content and backing scale1 before starting the edge. Paragraph geometry
matches at width940/8155 lines initially and856/8988 lines finally; the full
1048499-byte/999569-UTF16 body hash agrees before and afterward. All18 actual
resize dimensions match between cells. Both input handlers and ACKs remain
inside their respective edges. Runtime IDs, coalesced work counts and timing
progress are not forced to match.

The expensive full-width shape calls are much shorter in this first comparison:

| Paragraph width | Control shape wall, ms | Viewport-hit shape wall, ms |
| --- | ---: | ---: |
| 940, initial | 2734.693 | 122.948 |
| 927, first resized width | 2659.889 | 126.735 |
| 856, final | 2512.432 | 121.833 |

These are inclusive worker shape calls, not CPU or end-to-end frame times. The
treatment executes many more intermediate-width jobs, taking115–128ms each.
Worker wall clipped to the live edge remains busy:1626.618→1668.295ms. Nested
shape/index/raster spans are not added to their worker parents. Final index
construction remains76.719→77.316ms. The source/geometry behavior and comparison
are preserved without shrinking or splitting the paragraph. Raster-scope total
wall increases5.729→27.973ms, with different call counts19→7; the treatment
raster includes viewport-hit construction. It is not a uniform reduction in work.

Neither arm establishes an accepted new-width raster **and** model-layer
assignment inside live resize. Both first post-edge snapshots have
displayable=false and visible publication0. The final raster is accepted
3642.321→281.279ms after the edge; its **first qualified** model-layer assignment
follows at3642.739→281.716ms, only0.418→0.437ms after raster acceptance.
The original report selected the latest repeated assignment of that raster,
at3961.678→1078.265ms after the edge. Those endpoints certify the final saved
state but do not measure first delivery; the earlier interpretation of a
319.357→796.986ms delivery gap is withdrawn. Faster shaping has not established
continuous visible reflow, and layer assignment does not prove physical presentation.

All18 measured resize calls per arm are under8.333ms, but their whole-call
maximum rises3.507→4.091ms and their totals17.122→17.439ms. Typing ACK maxima
are4.426→4.013ms; wheel ACK maxima rise0.277→0.352ms. These bounded offers are
not a latency-tail guarantee. Primary/UI/worker probes have no omissions;
passive mode still overflows and remains NOTESTABLISHED. The actual display is
60Hz. One run order proves neither repeated benefit nor physical120Hz.

The two immutable captures contain92 and93 files,98,134,318B total. Actual
compiled Swift inputs differ only in the seven production paths from§8.70;
header/module/Bridge dependency rules fall within each owned build. Generated
NativeMarkdownStress entry, plan, compatibility, toolchain and configuration
match. Control was captured before the treatment build. Both builds and both
cells exit0 without retries; all eight recorded PIDs/four groups are absent,
with both external mouse-ups retained. Evidence is
`target/apple-giant-viewport-hit-pair-execution-v1/`; control binary
`1e10d131bc78b037c52bdd7352b23ddada7a5eb28583f8ccbf15989dc9564660`,
treatment `579581d86cca1216add89c9265fcc36fcb068bba009002746f65b6f631989253`.
Independent selected-raw reconstruction agrees on shape/index spans, complete
source, geometry, input counts and first-versus-latest layer endpoints. No screenshot/crop or
general memory-bound claim is added by this experiment. The next work separates
remaining index construction from retaining valid visible content and prompt
publication; a reversed pair must use these same products and fresh cells.

The owner runtime freeze records310 files/125,064,209B, manifest
`9268499f87f8ab02c2336aa84cba4aa327ad07f49e306ae4c868916bd07b92ce`,
report `6cf2b15e3140510a4666aeda50b8943a1cb29d66d78fc9eb406d6b5fdea5d13f`.
A retained offline summary-script parse failure was corrected without changing
the diagnostic reader, oracle, source or native runs.
The frozen report's endpoint error remains preserved; the correction is in
`target/apple-giant-viewport-hit-first-assignment-addendum-v1/`, manifest
`8a9475350384d68cb532844d0837b189b792dcd6f6f09015b46d8d29e77c782e`.
Independent raw joins qualify first layer440 after accepted raster434 in control,
and first layer605 after accepted raster599 in treatment, with the same generation,
publication, raster serial, frame/source/artifact and scroll coordinates as their
latest certificates. A repeated-assignment behavioral RED and seven pure checks
cover the offline correction. No runtime rerun or historical artifact rewrite occurred.

### 8.72 Reuse captured line metrics when building the Apple paint index, 2026-09-18

RegionPaintIndex now reads the immutable line ink bounds, ascent, descent and
leading already captured during shaping. Its span closure previously queried
the same CTLine again. Operand order, null-ink handling, nonnegative leading,
rounded baseline, row origin, two-point padding and global paragraph/line
ordinals remain unchanged. No array, cache, worker or publication policy is added.

Two tests were authored before changing production. The baseline executes14
assertions and reaches two meaningful failures:36 indexed lines make36 redundant
glyph-bound calls and36 typographic calls where zero are expected. The candidate
records zero at both index callsites. Validation-only wrappers instrument the
original calls; counters reset after shaping and independent reference work.
Neither the wrappers nor counters are in production.

The exact old CoreText formula matches every candidate span and all endpoint,
adjacent-representable, interval and whole-range overlap queries, preserving
paint order. Controls include zero-height/coincident spans, empty/intrinsic
boundaries, fractional phases, clamp, bidi, emoji and combining text. Existing
selection, bitmap, reuse, viewport capacity and lifetime controls remain green.
Strict Swift6 optimized compilation and33 standalone methods/28,260 assertions
pass using the existing assertion shim, including the conditional work-counter
method. This is not a fullhost, full XCTest or native timing result.

The exact two source/test paths and69 artifacts/2,358,280B are archived under
`target/apple-region-index-numeric-validation/freeze-v1/`, manifest
`cb255a1f19f27f5c2e47e780660b2cf9b5f06cb07ce94d00b5970af3508aad9f`,
complete patch `f21071157eff482da030e2e950fd1f9af82d7f56e11c4c2b93c565ca67a86b85`.
This change was made after the immutable native products in§8.71. Their measured
76–77ms index spans do not measure this change: traversal, index allocation,
construction and sorting remain. No native speedup or120Hz result is inferred
from removing the duplicate calls.

### 8.73 Reversed viewport-hit native comparison, 2026-09-18

Two fresh cells run treatment→control using the same immutable products as§8.71;
there is no rebuild or index-reuse change. All four cells have the same actual
980×852 window,980×820 content, backing scale1, complete1MiB source and paragraph
940→856/8155→8988 lines. The18 accepted resize dimensions match across both
orders. Each new cell handles six type and six directional40px wheel offers
inside its edge. Edge duration and completed work remain unequal.

Semantically control→treatment, the reversed run records:

| Recorded wall scope or boundary | Control, ms | Viewport hits, ms |
| --- | ---: | ---: |
| Full shape, width940 | 2684.753 | 126.756 |
| Full shape, width927 | 2702.543 | 124.932 |
| Full shape, width856 | 2502.093 | 119.002 |
| Final index construction | 77.443 | 74.919 |
| First qualified layer assignment after edge | 3643.744 | 295.347 |
| Accepted raster to first qualified assignment | 5.288500 | 0.299000 |

First assignment joins acceptance612→layer618 in control and641→647 in treatment,
with exact generation/publication/serial/frame/source/artifact/scroll identity.
Later repeats631/667 remain current-state witnesses, not first-delivery endpoints.
Both run orders show shorter full shape scopes and earlier first post-edge layers.
Neither produces a new-width raster and layer inside resize; the first post-edge
snapshots again show displayable=false/visible publication0. This does not measure
a continuous blank interval or physical presentation.

Unequal work and negatives remain explicit. Complete shape scopes are16→60;
source captures4→5 and cumulative captured UTF8 bytes1,048,617→2,097,116 differ
despite equal accepted documents. Raster scopes80→6 have summed wall16.584→27.178ms
and maximum1.936→8.393ms; raster completions remain three each. Region-UI clipped
union rises7.936→15.384ms and worker union1671.221→1756.375ms. These inclusive
and overlapping scopes are not additive CPU. Resize envelopes improve in this
order, but the first order's increased total and wheel ACK maximum remain.
Typing ACK maximum rises3.870→3.949ms; six samples do not establish robust tails.
Primary and reflow probes have no omissions, while passive mode remains
NOTESTABLISHED. The display is60Hz, and continuous visible reflow/120Hz remain open.

Both cells exit0 once, without retries or builds. Eight recorded PIDs/four groups
are absent, with two owned mouse-ups, before the quiet lane is released. The
separate reverse archive is `target/apple-giant-viewport-hit-reverse-execution-v1/`,
106files/28,593,965B, manifest
`6de0a6464270ecb2724aa663dd14c37b6a297789ce4a162cc50221bd306a2c12`, report
`60ed1d335149173580916e4705c857b298fb63c0460bfdf5c127fd215d2ad639`.
The first310-file archive and its separate first-assignment correction remain
unchanged. Independent selected raw joins reproduce the new source, resize,
shape/index and first-layer endpoints; no screenshot or memory-bound claim is added.

### 8.74 Retire a rejected raster's own pending desire, 2026-09-18

RegionController previously cleared the submitted serial when a raster returned,
then rejected its old publication and scheduled again while the same desired
raster remained. This could resubmit the identical stale raster until an AppKit
surface update supplied a new intent. The rejection branch now clears that desire
only when serial, generation and publication all match the rejected answer.
A newer intent keeps its scheduling ownership; accepted pixels, appearance and
phase checks, batch deferral and the serial worker are unchanged.

The focused actual-controller/service/surface fixture blocks raster B, advances
the candidate publication while holding display updates, then releases B. A
submit-site counter and second worker barrier prove the recurrence without an
unbounded failure loop. Baseline has96 assertions/two expected behavioral failures;
candidate has99/zero, including three successor-recovery assertions reached only
after successful retirement. Different-serial successors, accepted-A cache return,
deferred answers, wrong ownership tuples, appearance/refusal and reset controls pass.
Both copied-source builds use Swift5 optimized compilation with warnings as errors.
Runtime/Presenter/session are fixture doubles; the worker, raster paths and small
AppKit windows are real. This is no fullhost, OS-input or native speedup result.

The one production path and115 artifacts/2,596,657B are preserved in
`target/apple-stale-raster-validation/freeze-v1/`, manifest
`e93a4ae6ca41ab93844b853c35dd8e632c53242d5c80bb0c0e2e92a7e76cac06`, patch
`13fe372d1a5e5acc12ae630b2b126a5fac0d9d0b7fbafa4e2620fc8ddce0012f`.
Independent inverse checks recover exact baseline/candidate production from all
13 copied production inputs; both compiler/run pairs are terminal and all four
recorded process groups are absent. Neither prior native pair nor the separately
prepared numeric-index experiment includes this fix. Visible continuity and the
remaining full-shape/index costs require their own evidence.

### 8.75 First native comparison of numeric paint-index reuse, 2026-09-18

A fresh viewport-hit control579581d8 followed by numeric-index candidate3f8f5741
isolates §8.72's RegionRaster change. The candidate's one optimized build exits0;
all1405 source cards and1471 actual compiled inputs differ only in that path.
The75 Swift inputs,78 normalized configuration units, toolchain/SDK, actual
Bridge dependency/header proof and generated entry/plan/compat remain matched.
Neither arm includes §8.74's stale-raster fix or retained-image work. Old native
timing cells are not substituted for the fresh control.

Both cells admit actual980×852 windows/content980×820, backing1, and the same
18 resize dimensions. The unchanged1MiB workload contains a1,048,499-byte,
999,569-UTF16-unit paragraph. Its accepted width/line count changes940/8155 to
856/8988, with equal full source before, first after and final. Each cell has
six typing and six alternating40-unit wheel inputs; handlers and ACK brackets
are inside the real resize edge. Timed progress and edge durations differ.

| Recorded worker index scope | Control ms | Candidate ms |
|---|---:|---:|
| Initial full-publication index |74.033792|0.706542|
| Final full-publication index |76.837125|0.731792|
| All stage101 scopes, including short attempts |150.873125|1.441876|

There are3→4 index scopes, including1→2 tiny attempts; these counts are not
counts of full constructions. Identical stage boundaries include traversal,
index storage and sorting, not just the removed CoreText calls. Retained index
bytes719,120 and sampled index charge peak1,371,600 match; no RSS reduction or
total allocation bound follows. Full shape work remains: largest scopes at
940/927/856 are124.373/138.368/120.927→125.122/123.590/118.439ms.

First qualified layer assignment after the edge is273.808792→183.802208ms.
The exact joins are control frame601/source602/acceptance616→layer622 and
candidate645/646/659→665, with generation, publication, serial, artifact and
scroll matched. Acceptance-to-first-assignment worsens0.365375→16.387750ms;
latest repeats649/692 occur1150.033500/1091.613500ms after the edge and are
reported separately. Both in-edge/new-width layer outcomes remain
NOTESTABLISHED. Sparse snapshots do not establish a continuous blank interval.

Unequal work and negative observations remain: source captures4→5 and captured
UTF8 bytes1,048,617→2,097,116; shape scopes57→61 and summed wall1951.868→2059.693ms;
raster scopes4→7 and summed wall16.211→30.169ms. Region-UI clipped union rises
16.062→16.673ms and worker union1683.419→1693.514ms. Inclusive scopes overlap and
are not additive CPU. All18 resize envelopes per arm are below8.333ms, but
this does not establish frame pacing without in-edge publication. Typing ACK
maximum rises4.051→4.093ms and wheel ACK maximum0.334→2.690ms. Passive mode
remains NOTESTABLISHED; primary/reflow probes have no omissions. One run order
on the60Hz display proves neither repeated latency benefit nor physical120Hz.

Both cells exit0 once without retries; eight recorded PIDs/four groups are
absent with two owned mouse-ups before release. The archive at
`target/apple-giant-viewport-index-execution-v1/` contains209files/71,335,760B,
manifest `c75bd47a389ba1eb41c48f491bcbaadf6578159e3ae01c456b751c61e5e94905`.
Independent raw reconstruction reproduces the index scopes, first/current
publication joins, source equality,18 resize chains and clipped unions. A
reversed confirmation and continuous visible reflow remain separate work.

### 8.76 Keep accepted Apple pixels while geometry changes, 2026-09-18

An accepted viewport image can now remain visible while its replacement is being
prepared. A fixed-size flipped image child retains the same CGImage/provider,
logical size and backing scale. Its translation is accepted scroll minus actual
NSClipView scroll; only exact integer device-pixel translations with the same
fractional phase are admitted. The current viewport clips that image and paints
the unchanged background in uncovered strips. No bitmap copy, image history,
extra worker or new pixel budget is introduced. Empty overlap hides the image.

This is display-only continuity. A separate retained witness describes coverage;
the current-pixel witness and visible-publication field remain invalid there.
Fresh text hits, copy and links are refused, and entering retention cancels the
old contact. Ordinary selection-only highlight replacement keeps its existing
qualified anchor behavior. Only accepted replacement pixels can change the
scroll extent or reenable exact current input. The same pixel/profile owners
remain charged through their last provider alias; opaque compositor storage is
outside that accounting.

Before applying a batch, the controller protects old and incoming region members,
owner/content, current ancestors and the page-background provider. Source/style,
tree, presentation or unknown/global operations invalidate retention before
callbacks can observe partial changes. Disjoint sibling typing and neutral
frame/extent changes may retain it. Invalidation retires the desired raster
without releasing the active worker slot; returning to an old source or palette
cannot simply redisplay the old image. Fresh accepted pixels must rearm it.
Conservative refusals are intentional; this is not a general interactive subtree
fallback. The separately tested §8.74 receive correction is preserved unchanged.

The actual AppKit fixture changes viewport size while B's worker is blocked,
then performs six actual NSClipView ±40 scroll steps. It verifies fixed image
size/placement, clipping, provider bytes/scale, unchanged A extent/publication,
fresh-input refusal and eventual B takeover. Source/style/background ABA and
existing cached/sparse/terminal selection controls are included. Old production
executes192 assertions with18 intended failures; candidate executes198 with0,
including six byte comparisons reached only when A remains visible. Runtime
and Presenter are fixture doubles; the NSWindow, surface, controller, worker and
constructed NSEvent handlers are real. This is model-layer evidence, not OS
input injection or a WindowServer screenshot.

Eight new helper tests plus eight existing controls pass16 methods/222 assertions,
including exact RGBA crop/background comparisons at scales1/2, fractional phase,
empty/reverse/end-clamp cases, caps and last-alias retirement. An earlier218-assertion
run fails two ownership assertions because test-owned CFData/provider temporaries
remain alive; explicit scope closure and weak-owner/drop checks correct the test,
without changing production ownership. Three compiler setup/warning failures
remain separate from the behavioral RED. All70 private module sources typecheck
with warnings as errors. No fullhost or native performance run is claimed.

The three production paths and one test are preserved with276 artifacts/7,074,184B
in `target/region-retained-validation/freeze-v2/`, manifest
`5464b6283e2e86178e4fddfe669434671b501457f5f8d91b9be6bd8d7c20ec46`, patch
`891145ddfe3615eef3d25e263e11361d577769edfde41ab7423e6c5770f15932`.
Independent inverse checks recover all13 actual compiled production inputs per
AppKit arm. All12 recorded processes/groups are absent before lane release.
The numeric-index timing products do not contain this change. Continuous fresh
reflow, retained coverage during the giant workload and physical120Hz still need
their own evidence.

### 8.77 Reversed index comparison stops at control geometry, 2026-09-18

The reversed numeric-index comparison is **FAILED/INCOMPLETE**. Candidate3f8f5741
completes once; the following control579581d8 refuses before poster creation with
`canonical region clipSize differs`. Its actual clip is980×400 rather than the
sealed963×400. Outer980×852/content980×820, backing1, the940-wide/8155-line
paragraph, full source, document extent and scroll0 match the other three starts.
Only three cells have completed resize edges. The discrepancy's cause and any
product defect remain unestablished. No retry, replacement control, relaxed
geometry check or paired reverse-order speedup is claimed.

The completed candidate records initial/final full-publication index scopes of
0.721583/0.746125ms, plus a0.003125ms short attempt. Shape scopes still reach
127.198708ms. All18 actual resize dimensions match §8.75. The qualified final
frame625/source626/acceptance640→layer646 join puts first layer assignment
265.053375ms after the edge and12.120584ms after acceptance; layer673 is a later
repeat at1150.151042ms after the edge. Both in-edge/new-width publication remain
NOTESTABLISHED. Neither a sub-millisecond index nor all18 resize envelopes below
8.333ms proves fresh frames during the edge. Typing ACK maximum10.945ms and
passive-mode omissions remain; the output is60Hz.

The new archive `target/apple-giant-viewport-index-reverse-execution-v1/` contains
75files/16,473,352B, manifest
`775f3513e94ea694bba754db73d70bbe2e696833f4c2b395c8e937b8affae851`.
Independent raw reconstruction verifies the index scopes, complete first/current
publication join,18 resize chains, clipped unions and four starting geometries.
The previous209-file first-pair archive remains unchanged. Two native apps but
only one external resize edge ran; eight recorded PIDs/three groups are absent
with one owned mouse-up and no control poster. The next discriminator separates
remaining full-shape costs using the current compact metadata; it does not
replace this failed comparison. These products omit §8.74/8.76's later fixes.

### 8.78 Remaining compact-worker costs at two widths, 2026-09-18

A standalone optimized Swift6 fixture pins dd810d1's compact metadata and numeric
paint index, before retained-image changes. One captured full1MiB source is shaped
at940 then856 while A remains owned, followed by a fresh856/height400 request.
Three admissions produce two layouts and one exact-width reuse. Existing worker
phases partition each full construction without per-line clocks or a new queue:

| Wall scope |940px ms|856px ms|
|---|---:|---:|
| Inclusive shape |142.717750|119.885208|
| Attributed string and typesetter |47.426542|24.500458|
| Nil-locale boundaries |9.958958|7.181834|
| Breaking, line construction and metrics |16.477375|17.354875|
| Compact line metadata |67.323041|69.487084|
| Remaining parent wall |1.531834|1.360957|

The height-only reuse takes0.002209ms and has no child phases. Metadata is the
largest measured child, but includes range, typographic bounds, glyph-path ink,
flush and allocations; these results do not isolate one CoreText call. Preparation
repeats24.500458ms at the second width. No preparation-sharing implementation or
gain is established. Full glyph hashing runs after the parent and between offers;
submit-to-delivery includes that diagnostic work and is not production latency.
Source capture14.931ms is separate. These are one ordered fixture's wall spans,
not CPU, repeated native A/B or frame timing.

Both complete layouts equal ordinary TextEngine for every line range, ink,
metrics, baseline, bottom and full glyph/run digest. All six top/middle/end
fractional-scroll/selection RGBA comparisons pass, retaining3,448,320 bytes.
A retirement leaves B's fresh alias usable; reset refuses its old worker backing
while external immutable metadata still copies the source. Seven raster scopes
each prove weak pixel-owner disappearance, owners/bytes0 and exactly one drop
before the next request. Successful numeric indexes take0.648/0.732/0.723ms;
seven renders take1.704–5.396ms. The reset's failed index is reported separately.

The first strict compile stops on a weak-variable warning; its helper-only
correction compiles, but the next run refuses a third image because temporary
provider aliases remain alive. The final helper gives each raster an explicit
autorelease scope plus weak/account checks. Production caps, instrumentation,
source and oracles are unchanged. Final compile/run/strict-reader exit0; actual
13 dependency files prove the header/module inputs. Earlier failures remain
failed. No further retry, GUI/fullhost or controller validation is implied.

`target/apple-region-current-phases-dd810d1/execution-v3/manifest.json` binds
109 retained files/7,848,531B, SHA
`7c0ca263284960e316b2d5bd4c0055948048077d7fe0f027d85568d78586cccb`.
Independent reconstruction checks complete child identities/containment/order,
pixel hashes and all seven release records. Owned compile/runtime groups are
absent. These results focus the next source investigation on metadata and repeated
preparation while full-app retained-image continuity remains separately unproven.

### 8.79 Share live worker preparation across widths, 2026-09-18

New definite-width requests can reuse the attributed string and CTTypesetter of
an existing live layout for the exact captured source object, source ID and
generation. The existing scan of at most64 request bindings prioritizes an
equal-width layout; otherwise it shares only preparation and builds new lines
and metadata. Every request retains its own artifact ID and full offer.
Intrinsic requests bypass this lookup. Nil-locale boundaries, ellipsis, glyph
ink, source capture, admission and cancellation remain unchanged. There is no
new worker, history cache or lookup through paint-only owners.

The prepared object is not Sendable and stays on the serial worker. This trades
longer attributed-string/typesetter residency for less repeated construction:
the last layout, including legitimate paint ownership, releases it. Those opaque
CoreText heaps are not measured here or included in pixel/index/hit budgets.
The64-binding limit is not a source-byte or RSS bound.

Tests first run the old implementation with a sentinel immediately after its
actual non-ellipsis typesetter constructor. Two distinct widths while A remains
live construct two preparations against an expected one; exact-key controls
construct seven against an expected six. These are the only failures among
36 methods/28,399 assertions. Candidate39 methods/33,986 assertions pass, including
two distinct layouts from one preparation, fresh height-request identity,
source/font/palette/generation misses, intrinsic and cap controls, reset/close,
paint-only exclusion and weak last-owner release. Glyphs, metrics, selection,
hits and selected pixels match unchanged ordinary or fresh-preparation references.
The candidate's three additional new-API methods are omitted from the baseline;
global suite constructor totals and execution times are not comparable gains.

Both standalone arms compile with Swift6 strict concurrency and warnings as
errors. All70 uninstrumented module sources typecheck in the host's Swift5 mode;
their bytes match the integrated tree. The harness uses an assertion shim rather
than full XCTest. No fullhost/iOS build, new native timing or physical120Hz result
is claimed. The full1MiB/two-width oracle was pending at this checkpoint;
§8.80 records that separate validation.

The exact two production files and two existing test files are archived at
`target/region-prepared-source-validation/freeze-v1/`, with185 artifacts/4,254,710B,
manifest `8b122289a636156fa9a1b57c05d05eefee570b415d0b7b7966e1b3bc337fd8bc`
and complete patch `e634e23e36f08e845074c608b445d470b1be6d1964ea8d8ba610f401774f5970`.
Independent constructor-instrumentation inverses recover both production arms;
all10 recorded processes/five groups are absent. The measured repeated
preparation in §8.78 motivates this change but does not quantify its saving.

### 8.80 Full-body validation of shared preparation, 2026-09-18

The §8.79 candidate passes the full1MiB backend oracle in one fresh optimized
Swift6 process. The same captured1,048,499UTF8/999,569UTF16 source is offered at
940 and856 while A remains owned, then at856 with a different height. Two layouts
and three distinct request artifacts preserve exact source identity; the third
shares B's layout. Every line's range, ink, metrics, baseline and full glyph/run
digest agrees with ordinary TextEngine. Six top/middle/end fractional-scroll
selection pictures match exactly, totaling3,448,320 RGBA bytes; their bytes and
3144-byte color profile also equal the retained §8.78 outputs.

| Wall scope |940px ms|856px ms|
|---|---:|---:|
| Inclusive shape |140.158541|93.117083|
| Preparation construction/acquisition |46.221292|0.000375|
| Nil-locale boundaries |8.756542|7.170125|
| Breaking, line construction and metrics |16.225583|17.009083|
| Compact line metadata |68.865541|68.928958|
| Remaining parent wall |0.089583|0.008542|

The second preparation span acquires the existing object; it is not a fresh
constructor taking0.000375ms. Separate scoped tests establish the constructor
count reduction. The height-only shape hit takes0.002458ms. Full glyph hashing
runs after the shape parent and between offers; submit-to-delivery remains
327.654/281.604/184.586ms including that diagnostic work. These are one ordered
backend fixture's wall spans, not a fresh matched A/B, CPU attribution, UI latency
or physical120Hz evidence. Metadata still takes about69ms and is not isolated to
one CoreText call. Longer opaque preparation residency remains outside the
pixel/index/hit budgets; no RSS bound is measured here.

All seven raster scopes release their weak pixel owner, return accounted owners
and bytes to zero, and record exactly one drop. Retiring A and B request IDs
leaves B's fresh alias painting identical pixels; reset refuses its old worker
backing while external metadata still copies the full source. Strict compile,
backend run and unchanged reader exit0 without retries. Actual13 dependency
files establish header/module inputs; the11 production inputs match integrated
ff0119e after reversing the two existing observer overlays. No controller,
full-app resize or iPhone execution is covered.

`target/region-prepared-fullbody-validation/execution-v1/manifest.json` binds
105 artifacts/7,843,496B, SHA
`4962555f404b9a16577ed654cab9da0cc02b70e12a17c830616e0365a426a046`.
Independent raw reconstruction checks all three shape parents and children,
full geometry/glyph identities, pixel bytes and seven release records. Both
owned execution groups are absent. The next investigation is the remaining
metadata work; retaining correct visible output during it is a separate target.

### 8.81 Full-app continuity run stops at startup geometry, 2026-09-18

One optimized Markdown Stress build combines the retained-image change with
MAIN's raster-rejection handling and numeric index. Its actual receipt comparison
against the immutable numeric-index foundation differs only in the controller,
surface, raster and ignored observer; generated app entry, plan, compatibility
and workload are unchanged. This product does not contain §8.79's preparation
sharing, and its old foundation is not a runtime comparison arm.

The sole fresh cell exits1 before the resize poster launches. Its region clip
is980×400 where the frozen admission requires963×400. Window980×852,
content980×820, backing scale1, paragraph940×212030 with8155 lines, zero scroll,
and accepted document985×212193.67 otherwise match. The complete paragraph is
present with a current, displayable publication. The recorded17px difference
does not establish its cause or a product scroll/rendering defect.

Eight setup commands ran; the18-resize/6-type/6-wheel workload did not. There is
no A-to-B transition, retained-placement observation, replacement-B latency or
continuity conclusion. No poster/down existed, so no release-up is applicable.
The four recorded processes and sole group are absent, with no watchdog action,
retry, reanchor or geometry relaxation. The earlier successful build remains
successful; the runtime remains failed.

`target/apple-giant-retained-continuity-execution-v1/runtime-manifest.json`
binds24 artifacts/6,038,217B, SHA
`6658adb06a3abdb447d0cd81bbcb8e14941a2a91728505bc001e603964fb1de3`.
Build capture `1277f38262ae380b168e6df7afe590340a710d77dc7bb14e1d6cea23d1999d86`
and binary `25e98b0a2439583d6b20aaba1d8976847442613502d5bd3dfef5a5631cefa598`
remain immutable. Startup geometry must be explained before a separately bound
continuity experiment; this failed admission is not replaced by earlier results.

A separately frozen980 binding then also exits1 before its poster starts: this
time the actual clip and current-image certificate are963×400. Empty-document
snapshots start at980; the full document's two later snapshots both show963.
All eight commands are setup only. The actual scroller style, visibility and
insets were not recorded, so the opposite geometry is not explained by these
receipts. Neither accepting both sizes nor another blind retry follows from it.
The three recorded processes and sole group are absent without watchdog action.
`target/apple-giant-retained-measured980-execution-v1/runtime-manifest.json`
binds25 artifacts/5,008,934B, SHA
`438ba2824a64beb75acaf1babf5ecca03165991d533e57c73e708bafa67af117`.

This failed cell does contain one narrower startup observation: the original
980×400 image stays owned and is cropped to963×400 at zero translation, scale1,
with matching publication/source/artifact and all2047 retention flags. A second
raster's acceptance86 joins first model-layer assignment92,31.634250ms later;
99/106 are repeats. This is a viewport recrop at the same940px paragraph width,
not the unrun new-width B transition. No clear row is observed, but neither
uninterrupted coverage nor physical presentation follows from that absence.
Both failed archives and the original product remain immutable.

### 8.82 Glyph-path bounds dominate compact metadata, 2026-09-18

One diagnostic over the unchanged §8.80 full-body fixture brackets the four
CoreText calls inside each compact line's construction. The exact operation
`CTLineGetBoundsWithOptions(.useGlyphPathBounds)` accounts for almost all of the
observed metadata wall time. Range, duplicate typographic metrics and flush
offset together account for less than1ms at either width.

| Width / lines | Shape ms | Metadata ms | Range ms | Typographic ms | Glyph-path bounds ms | Flush ms |
|---|---:|---:|---:|---:|---:|---:|
|940 /8155|140.112124|68.466166|0.074366|0.069839|67.466807|0.309553|
|856 /8988|101.170291|76.853125|0.094166|0.083346|75.397943|0.688017|

Each of the four call counts equals the line count. Exactly two aggregate
records join108 scopes5/10 to100 parents1/6 with the same request, generation,
owner and offered axes. The same-width height hit takes0.002333ms and emits no
metadata aggregate. No aggregate or primary row is omitted or mismatched.

The diagnostic adds137,144 clock reads, an88-byte local accumulator and480 bytes
for three summary slots per existing lane; both lanes reserve960 bytes in total.
There are no added per-line trace rows or
locks. These are scalar layout sizes, not allocator/RSS or zero-overhead proof.
The brackets include probe effects and any waiting within the calls; no CPU
attribution, overhead subtraction, array-only residual or performance A/B is
claimed. Parent and child spans overlap and cannot be added.

Seventeen pure reader controls, one strict optimized Swift6 compile, one backend
run and both readers pass without retry. Every line's geometry and full glyph
digest still matches ordinary TextEngine; all six RGBA outputs and ICC bytes
equal §8.80, and all seven pixel-owner releases, alias retirement and reset
checks pass. Actual13 compiler dependency files bind the header/module inputs.
Compiler and backend groups are absent. Production is unchanged; these three
observer overlays exist only in the ignored fixture.

`target/region-metadata-calls-validation/execution-v1/manifest.json` binds110
artifacts/7,797,932B, SHA
`6f72d5b9732435dc6861e96e9459f6b381ab15d2b0eed8dec59f6366231b0455`.
Independent raw arithmetic verifies both aggregates and their containing spans.
The next source investigation targets exact ink-bound work while preserving
conservative paint coverage and the pixel oracle. Replacing ink with ordinary
typographic bounds is not established as correct. This fixture does not measure
full-app continuity, UI responsiveness or physical120Hz.

A subsequent helper-only discriminator rejects reuse by exact previous line
range for this workload: A's8155 and B's8988 output ranges have zero exact
matches. A constant-scratch merge first validates positive, ordered, disjoint
in-source ranges. Equal output ranges would only be an upper bound on safe
creation-range reuse; zero matches therefore leaves that proposal no coverage
for these widths. No certificate cache or associated memory account is added.
One strict compile, backend and unchanged reader pass, retaining all full
geometry/glyph/copy checks, six exact pictures and seven owner-release checks.
`target/region-range-overlap-validation/execution-v1/manifest.json` binds87
artifacts/6,902,939B, SHA
`f687b13626fb03035a70ee44ca2bdaf7363df7058581eeeb300dde082aa24bd4`.
This counts coverage in the frozen pre-§8.83 fixture; it is not a timing comparison
or proof that every other form of exact ink reuse is impossible.

### 8.83 Compute accepted-source digests on the existing worker, 2026-09-18

Giant-source capture previously hashed the full joined UTF8 and allocated its
temporary Data on the UI thread. That diagnostic SHA now belongs to the existing
worker-only preparation. New widths sharing that preparation reuse its digest;
accepted metadata carries only the immutable string. The controller's existing
`acceptedSources[].sha256` field reads that metadata with the same value and
publication ownership. Its receive, retained-image and delivery code is unchanged.

No queue, lock, lazy UI fallback, layout history or additional CoreText owner is
introduced. Same-width reuse still takes precedence; reset, source replacement
and non-retained intrinsic preparations compute fresh worker digests. Metadata
keeps its digest and source usable after the last preparation drops. Hashes are
diagnostic values, not cache identity keys. Exact source/object/generation and
request guards remain unchanged.

A validation-only wrapper around the real SHA call records one UI hash and zero
worker hashes on the old source: the API-compatible eight-assertion method fails
three intended assertions. The candidate passes43 methods/34,009 assertions,
including zero UI hashes, one hash across two live widths and a height hit,
exact decomposed Unicode/run boundaries, empty input, reset/intrinsic behavior,
old accepted diagnostic values and last-owner release. Strict Swift6 compilation
and the complete70-source module typecheck pass. The extracted diagnostic closure
test is not a full controller/publication integration test; the full controller
is separately typechecked and its sole source change is the field read.

This relocates work; it does not remove hashing or all linear capture work.
UI capture still resolves fonts/appearance, traverses runs and ranges, joins text
and counts bytes. The earlier14.3ms capture measurement includes those costs and
cannot be credited entirely to this change. Worker preparation now includes the
hash; previous frozen diagnostic recipes retain their original scope and must
be explicitly rebound before new measurements. No native speedup or120Hz result
is claimed from the scoped tests.

`target/region-worker-digest-validation/freeze-v1/manifest.json` binds262
artifacts/5,314,451B, SHA
`784c229584b577e3e60957665d3c44915dc2dbf394accabf430d9cb6205cb43b`.
The six integrated source/test files equal the compiled and frozen inputs.

### 8.84 Wayland window attempt refuses device identity before launch, 2026-09-18

The observed Linux client and independent reference compile successfully from
the corrected source, producing ELFs7f0c783a and1c481467. The first window-run
binding stages successfully, then its guest preflight exits1 at the requirement
for exactly one DRM card whose `device/driver` resolves to `vkms`. The failed
cardinality assertion does not record the selected count or sysfs inventory;
it does not distinguish absent hardware from a selector mismatch.

Only a read-only `systemctl show seatd.service` child ran and reported inactive.
Neither reference, app, compositor, seatd server nor input device launched.
There are no window, configure, pixel, input or timing results. All recorded Mac
and guest processes/groups are absent, without signals or forced cleanup.
Successful build evidence remains unchanged. Device identity must be explained
before another explicitly bound attempt; no backend or mode fallback occurred.

`target/wayland-window-validation/attempt-v1/artifact-manifest.json`, SHA
`2216f495a3c712db0cf5080190599631ad0af10a3a9f4682552d36d2a8df2d5e`,
preserves the original failed run, staging and cleanup receipts. The corrected
retained-A input/B-worker reader remains a pure-test result, not native acceptance.

Subsequent read-only diagnosis finds zero cards under `/sys/class/drm` and no
`/dev/dri` entries. The running kernel is6.8.0-139; neither VKMS nor virtio_gpu is
loaded, and `modinfo` cannot find a matching VKMS module. The139 modules-extra
package is not installed; uinput is built into this kernel. Older successful
captures record a134 VKMS module, which is not a compatible substitute. These
facts explain zero selector matches without establishing when device state was
lost. No driver load, package install, device open or native retry is covered by
this diagnosis. Restoring the matching virtual device is separate setup work.

A separate restoration downloads the exact139 extras archive, verifies its
SHA256, and loads only its signed17,317-byte VKMS module for this boot.
The one `insmod` exits0; the module has matching139 vermagic and no additional
module dependencies. No package is installed, no package scripts run, and no
boot, service or module-parameter policy changes. The module SHA is
`661bd13d446a3ad7d4cc3d8b9f307ada50aeb8d9a1be92cfdc0b5baa595c3cee`.

Post-load metadata identifies character device226:0 at `/dev/dri/card0`, backed
by `/sys/devices/platform/vkms`. Virtual-1 is connected and lists1024×768 first;
that list alone does not prove refresh rate or the preferred-mode flag. The
actual defaults are cursor/writeback enabled and overlay disabled. This device
has no `device/driver` symlink, so the original matcher would still refuse it.
The next binding must qualify the actual platform/module/device identity and
query the DRM identity before starting the compositor. No compositor, reference,
client, input or DRM-device open occurs in this restoration. It establishes
neither Wayland acceptance nor a performance result.

All restoration processes/groups retire without cleanup signals. The41-file,
105,054B archive is
`target/wayland-device-restoration-validation/current139-v1/`, manifest
`1ccb7da95a8a5ef8ed9f4bf28ab14a9c80b007b00a8127da770e57d70e824d69`.
The original failed attempt remains unchanged.

A separately bound second attempt accepts the actual platform/module/card
identity and passes package, source, toolchain, font and product preflights.
It then fails in the first reference's output-directory setup: the supervisor
creates and chowns the directory, but the unchanged oracle calls `create_dir`
on that same path. Reference A aborts with EEXIST after0.061793s, before its
trace, source construction, shaping or pixels. This is a harness error, not a
product, compositor or memory-cap failure.

Reference B, the client, libdrm identity query, seatd server, Sway and uinput
remain unrun. The2.5GiB reference limit was applied; no watchdog or cleanup
signal fired. All recorded guest and Mac processes/groups are absent. The
module remains loaded without package installation; the first failure remains
unchanged. The56-file,114,138B failed-attempt archive is
`target/wayland-window-validation/attempt-v2/`, manifest
`3a1656555e374fc239aab63476004eb3d8f8e9f14823fa7a1c6029cf2751b5b4`.
No Wayland window acceptance follows from either attempt.

The third and final bounded attempt fixes only that output path: the unchanged
oracle creates a fresh child of its owned container. The old caller produces an
actual EEXIST failure with one ownership control passing; the corrected caller
passes six local filesystem tests. An earlier test-only umask assumption failed
before the intended assertion and is preserved separately.

Both native references now exit0, in1.666160s and1.617747s, and the actual DRM
identity query passes. The run then exits1 when starting `/usr/bin/seatd`:
Popen reports ENOENT before the server starts. Sway, the client and input/window
acceptance remain unrun. This is another launch-harness failure, not a product
crash; reference results are not candidate results. All recorded processes and
groups retire without signals. Wayland runtime validation is deferred after
these three attempts; no fourth automatic retry or backend fallback follows.

### 8.85 Full-app retained-image placements during resize, 2026-09-18

One diagnostic-only scroll-geometry observer resolves startup admission without
choosing a963/980px allowlist. It records AppKit's actual scroll classes, border,
insets, scroller style, layout dirtiness and independent content-size prediction.
In this run the standard legacy vertical scroller occupies17px: Cocoa predicts
a963×400 clip inside the980×400 scroll view, matching the actual clip, ink view
and accepted raster certificate. Unknown configurations or inconsistent geometry
still refuse. This does not explain the historical cause of both earlier startup
failures, which remain unchanged.

The one optimized build and one full-app cell exit0. All18 actual resize changes,
six typing handlers/ACKs and six signed40px wheel handlers/ACKs occur inside the
native resize interval. The canonical paragraph remains1,048,499UTF8 bytes and
999,569UTF16 units, changing from940px/8,155 lines to856px/8,988 lines. No workload,
quota, input cadence or production presentation policy changes.

There are24 qualified retained-image placements,23 inside the interval and one
just after it. They reuse the963×400 accepted image at scale1 with translations
(0,0) or(0,-40). Fourteen placements cover their viewport; ten cover only360 of
400 vertical pixels after scrolling. Those placements deliberately leave the
newly exposed strip to the background policy. Discrete model-layer records do
not prove continuous coverage, WindowServer pixels or usable fresh text hits.

The retained episode closes with a fresh final-width assignment. Its frame/source
records973/974 join raster acceptance989 and the **first** matching layer record995:
12.254500ms after acceptance and231.130375ms after resize ends. Record1023 is a
later repeat, not the first assignment. Fresh new-width publication inside the
resize interval remains not established. The captured observer uses retained
placement stage34, children35–43 and clear44; newer MAIN stage numbers must not
be applied to these raw records.

This binary is the earlier8410 fixture plus retention and the geometry observer;
it does not contain the subsequent prepared-source or worker-digest changes.
It is one functional observation, not an A/B speedup or current-MAIN timing.
UI/worker records have no omissions, but passive-mode omissions keep mode
classification not established. The Mac display remains60Hz. All four recorded
processes and two groups retire, the owned external pointer releases, and no
retry follows.

The raw report is
`target/apple-giant-scroll-geometry-observation-execution-v1/candidate/cell-01/report.json`,
SHA `a0e2c376467b6e53b4347dc1594f2625fb88cfa2fabb95574801e6e0956de618`.
Its separate retention evidence identifies the first assignment; the summary's
`geometry.after` field identifies the latest assignment. Both must be retained.

### 8.86 Reconciliation with bounded answers and current main, 2026-09-18

The responsiveness branch at569f966 is reconciled with main atf61ff1c in an
isolated checkout. Main's bounded Messages answers and collection edge delivery
remain alongside immutable full-history row reuse, Arrange ownership, height
index reuse and half-unit end following. Host feedback still applies committed
work when an edge action refuses; first data activation refreshes collections.
Linux keeps its presented-picture and pending-flip guards.

Reusable Messages accepts the optional cursor and returns the canonical
nine-field History in both modes. Manual/full requests retain row sharing,
including equivalence between an omitted cursor and explicit none. Cursor
requests use the existing bounded generator; both modes share one latest result,
validate before publishing and preserve accepted old answers. Four new tests
cover canonical fields, cursor controls, invalid-input atomicity, old-owner
release and real Runner mode transitions. The full10k/batch32 stress control
remains available; no historical full-history timing is relabeled as bounded
answer performance.

Exact Live's curated and synthetic answers now use the same nine-field shape.
The existing producer/navigation test first fails with `Shape { resource:
"history" }` when entering full load, then passes after this correction. An
earlier curated-only boot probe passed and was insufficient to expose the
transition; its saved log is not behavioral RED evidence.

Scoped verification passes529 Rust tests:83 Runner library,334 across Apple,
Linux and Web host libraries,69 Messages/Exact Live data and43 Contract
collection/edge/reorder tests. Two existing tests remain ignored: the Linux GPU
case and the opt-in timing probe. Web collection DOM tests pass77 tests with274
assertions. Strict all-targets Clippy passes for these seven packages; scoped
formatting passes after a test-only array-format correction. All70 current
ExactKit Swift sources typecheck with warnings as errors against the merged
header and module map. UIKit branches, native relinking, physical interaction
and new performance measurements are not covered by that typecheck.

Staged caps pass706 source files and boot remains two modules,155,453 bytes.
The raw validation and preserved failure logs are under
`target/responsiveness-main-integration-validation/`. These are scoped merge
checks, not a whole-workspace, Linux GUI, Wayland or120Hz result. Earlier frozen
runtime archives retain their original source identities and limitations.

### 8.87 Current-source Markdown cells with unmatched initial clips, 2026-09-18

Two fresh optimized builds use the merged dafa9d6 base and the full1MiB
NativeMarkdownStress paragraph. The control reverses only the five prepared-source
and worker-digest production paths; treatment keeps current production. Both
retain the same seven observer hunks and current Session, collection, ABI and
Runner behavior. The complete source and actual build receipts differ only at
those five paths. Generated entry, plan, compatibility data, header, toolchain
and configuration match. The earlier8410/42bad products are not timed controls.

Both single cells exit0 with18 actual resize changes, six typing inputs and six
signed40px wheel inputs. They have equal outer window geometry and the same full
paragraph:1,048,499UTF8 bytes and999,569UTF16 units, reflowing940→856px. However,
independent Cocoa admission records a980×400 initial clip for control and963×400
for treatment: overlay versus legacy scroller policy, already different at boot.
Both match their actual Cocoa sizing; the policy transition's cause is not
observed. The initial accepted raster serials also differ. These are
individual functional observations, not a geometry-matched A/B speedup.

| Recorded endpoint | Control | Treatment |
|---|---:|---:|
| Native resize interval | 1797.039ms | 1808.895ms |
| Whole resize-call median / maximum,18 calls | 0.791 /1.507ms | 0.795 /3.624ms |
| Typing ACK maximum,6 offers | 5.098ms | 3.613ms |
| Wheel ACK maximum,6 offers | 4.165ms | 0.433ms |
| Recorded UI interval union inside resize | 23.240ms | 26.454ms |
| Recorded worker interval union inside resize | 1670.009ms | 1586.093ms |
| First final-width layer assignment after resize ends | 180.055ms | 157.509ms |

All recorded resize calls and input ACKs in these cells are below8.33ms. This
does not bound every UI operation, input-to-photon delay or presentation frame.
UI/worker unions are overlapping diagnostic wall intervals, not additive CPU
costs. Both fresh-width publication-inside-resize endpoints remain not
established; faster ACKs alone do not establish visible120Hz progress.

Control frame/source941/942 joins acceptance958 and first layer966,
2.997499ms after acceptance. Layer987 is a later repeat,1003.136792ms after
resize ends. Treatment1031/1032 joins1047 and first/latest1053,
12.283458ms after acceptance. Treatment is already ready in its first post-edge
snapshot; no subsequent `final-full` snapshot is required by the runtime.
The original paired analyzer assumes that filename and fails. A separate
descriptive reader preserves that failure and validates the already-ready
snapshot against the unchanged publication join. Its first attempt also rejects
control's legitimately pending first-after snapshot; that setup failure remains
saved. The corrected offline description keeps the geometry mismatch and uses
pending snapshots only for source-content comparisons. No runtime is repeated.

Frozen retention attribution is also not established. Control records25 A
placements,24 inside resize; its clear occurs179.728ms after resize ends,
following B acceptance, with the next model-layer assignment about0.326ms later.
That transition is not proof of an onscreen blank. Treatment's reader collects21
A placements then refuses the preceding assignment identity for intermediate
B publication62/raster24. Source and raw records show three B placements through
the retained-image path, two inside resize, without a prior exact-current
stage18 assignment. The reader requires that prior assignment and cannot join
this lineage. These display-only placements do not establish fresh-current
output or hit eligibility. Both original refusals remain unchanged.

Next work is an offline acceptance-to-retained-placement lineage check and a
source-level examination of the replacement clear. A future causal pair must
match measured scroller policy and geometry; neither an allowlist nor forcing a
preferred result can repair this pair. Continuous coverage remains unproved.

All eight recorded processes and four groups retire, with both owned external
pointers released and no cleanup signals, retry or third cell. Passive-mode
classification remains not established and the display remains60Hz. No physical
120fps or repeated performance gain is claimed. Raw cells and separate capture
receipts are under `target/apple-current-region-pair-execution-v1/`; the lead's
independent per-cell reconstruction is `lead-independent-per-cell.json` there.

### 8.88 Assign qualified replacement pixels before clearing, 2026-09-18

The replacement transition observed in8.87 has a narrow production cause.
`publish` can install accepted B in the surface's stored raster/image before
AppKit calls `updateLayer`. An intervening phase invalidation rejects A's old
visible witness, but the retained-image branch also rejects exact-current B.
It therefore clears the layer until the ordinary display callback assigns B.

The surface now shares its existing exact-current assignment between ordinary
display and phase invalidation. It validates the accepted raster against the
actual current phase and image/profile/scale, assigns the image, frame and
background, then publishes B's visible witness. Invalidation issues no raster
request and dispatches no pending link; link completion stays in ordinary
display. Source/appearance/refusal and geometry guards remain intact, as do
selection ownership, display-only retained pixels and existing pixel budgets.

The existing AppKit fixture adds the actual interleaving: display A, retain it
during resize, accept B while ordinary display is blocked, then invalidate the
phase. Old production fails seven of221 assertions, exposing nil contents and
one clear before B can be shown. The candidate passes all221: B is directly
assigned with its exact frame/provider and fresh hit identity, zero clears and
zero additional raster requests. The same fixture retains multi-move selection,
blocked sparse input, ABA, refusal and context-cancellation controls.

The scoped retention/raster assertion runner passes17 methods/230 assertions;
it is a direct assertion shim, not a full XCTest run. All70 current ExactKit
sources typecheck with warnings as errors against the current header. Both
compiled fixture variants invert exactly to their claimed production inputs and
use identical test-main bytes. Integrated production matches the compiled
candidate. These are real AppKit/model-layer fixtures with explicit runtime and
presenter doubles, not a relinked full app, physical blanking test, UIKit result
or performance A/B. The earlier runtime archives remain unchanged.

Source and execution evidence is preserved under
`target/region-atomic-publication-validation/`. No worker, queue, image history
or wider budget is added. The change closes the tested model-layer gap; it does
not establish continuous physical120Hz presentation.

### 8.89 Guarded reversed Markdown pair, 2026-09-18

Two new cells run treatment then control using the same immutable dafa9d6
products as8.87, without rebuilding. Both precede the atomic Surface fix1e1cf3b;
these are not current-MAIN timings. A prospective guard compares the first
existing fully-ready snapshot with the existing pre-edge snapshot, using the
unchanged independent Cocoa admission. Control must also match treatment's
sampled tuple before its external resize input starts. The old admission passes
nine controls and misses eight intended refusal cases; the new guard passes all
17. It adds no native read, poll, forced style, width allowlist or retry.

Both cells pass with legacy scrollers and963×400 initial clips. Their18 actual
resize offers, full six document Values, twelve input requests, window geometry
and backing scale match. Final clips are879×400; the unchanged full paragraph
reflows940→856px. This closes the sampled scroller mismatch for this pair only;
the original unmatched cells and their classifications remain unchanged.

| Semantic control → treatment; execution treatment → control | Control | Treatment |
|---|---:|---:|
| Whole resize sum / maximum,18 calls | 17.647 /4.112ms | 16.488 /2.775ms |
| Typing ACK maximum / wheel ACK maximum | 3.956 /0.535ms | 3.506 /0.360ms |
| UI interval union inside resize | 23.304ms | 26.761ms |
| Worker interval union inside resize | 1660.839ms | 1646.156ms |
| First final-width layer after resize ends | 210.608ms | 168.654ms |

Control frame/source935/936 joins acceptance951 and first layer957; treatment
1020/1021 joins1036 and first1042. Acceptance-to-first assignment is12.174875
and12.348875ms. Latest985/1070 are fifth repetitions, not first delivery. All18
measured resize calls and all12 input ACKs per cell are below8.33ms, but both
fresh-publication-inside-resize endpoints remain not established.

The unchanged retention reader establishes24 discrete A placements per cell,
23 inside resize, with no clear44 and each episode ending at fresh B. Coverage
is14full/10partial for control and15full/9partial for treatment. The captured
enum is still34/35–43/44. These placements do not prove uninterrupted coverage,
fresh hits or physical display, and do not test the later Surface fix.

Costs remain mixed: treatment's UI union and acceptance-to-first delay are
higher. Final shape completions are38 versus44; recorded worker spans cover
different progress and horizons. The clipped unions are overlapping wall
intervals, not additive CPU costs or equal-work comparisons. One matched pair
does not establish a uniform or repeated gain. Passive-mode classification
remains not established and the recorded display is60Hz; physical120fps remains
unmeasured.

Both cells exit0, with eight recorded processes/four groups gone and both owned
pointers released. No build, retry, third cell or source change occurred.
The102-file runtime archive is under
`target/apple-current-region-guarded-reverse-execution-v1/`; independent lead
raw reconstruction and final-geometry checks are separate `lead-*` sidecars.
The next source discriminator targets obsolete shape work before metadata
construction, preserving complete results, bounded progress and accepted A.

### 8.90 Stop obsolete definite-width work before metadata, 2026-09-18

A busy Apple region worker previously finished compact metadata even when the
controller had already received a newer shape request. The controller's busy
guard prevented that newer request from reaching the worker's latest-job slot.
Accepted region receipts now separately publish the desired request ID and
generation to the existing service. This signal carries no source or job and
does not release the occupied worker slot.

One checkpoint after line construction and before `RegionParagraph` can abandon
an obsolete definite-width request. Its temporary CoreText owners unwind on the
same serial worker inside the existing autorelease pool. No partial metrics,
layout binding or completion reaches the kernel. A typed abandoned terminal
contains only request ID and generation; the controller checks both against its
active owner before touching busy state or the deferred-answer slot, then
schedules the latest request. Accepted pixels and their owners remain intact.

After one voluntary abandonment, a valid complete shape must finish before
another is allowed. Same-width aliases count as complete answers, and intrinsic
width requests retain their complete-answer behavior. A negative height alone
does not exempt a definite width. Reset and close invalidate old epochs without
rearming a successor from an old completion. The live-ID cap, one serial worker,
one replaceable pending job and one completion mailbox remain unchanged.

The existing AppKit/controller fixture pauses actual shaping at this checkpoint,
supersedes it through `prepare`, and counts real metadata construction. Old
production fails three of269 assertions: obsolete work, a rearmed abandonment,
and reset all still construct metadata. The candidate passes271 assertions,
including two additional old-owner/old-generation terminal controls. Protected
completion, latest-request completion, exact retained A pixels and the existing
selection/retention/atomic-publication controls pass. Both fixture compilations
invert to their claimed production inputs and share the same test main.

All70 current ExactKit Swift sources typecheck with warnings as errors. The
broader55-method assertion harness remains unrun after three preserved compiler
setup failures: a missing extracted helper, a strict weak-binding warning in a
new test, and a stale generated runner entry. The equivalent weak-binding test
correction is retained; the four new unit methods are authored coverage, not
additional passing tests. There was no fourth compile attempt or suppression.

This verifies the checkpoint's work avoidance and ownership in an actual
AppKit/CoreText worker fixture with explicit Runtime/Session/Presenter doubles.
It is not a relinked full app, UIKit result, measured speedup or physical120Hz
result. Work before the checkpoint is still indivisible; supersession after it
can still complete metadata. A protected complete result can itself be obsolete,
so this policy does not promise current publication during endless changes.
Source and execution evidence is retained under
`target/region-abandonment-validation/freeze-v1/`. Application timing remains a
separate experiment against fresh products that include the atomic Surface fix.

### 8.91 First full-app abandonment pair observes no abandonment, 2026-09-18

Two fresh optimized products share base1e1cf3b, including the atomic Surface
assignment fix. Treatment adds only the three production paths from8.90. Both
use the same NativeMarkdownStress factory, full1MiB document, generated plan,
toolchain and observer. Actual source receipts differ only in those three paths;
the control is captured before building treatment. Both build and runtime cells
exit0 without retries or watchdog actions.

The guarded control→treatment pair matches the sampled963px starting clip and
879px final clip, all18 resize dimensions, twelve input payloads and six full
document Values. The paragraph reflows940→856px and8155→8988 lines. Input timing
and worker progress remain unequal; sampled geometry does not establish equal
work. Each cell has six typing and six signed40px wheel inputs, with handlers
and conservative ACK brackets inside its AppKit resize edge.

| Saved scope | Control | Treatment |
|---|---:|---:|
| Returned shapes, including aliases | 66 | 66 |
| Returned rasters | 4 | 6 |
| Typed abandoned / refused outcomes | 0 /0 | 0 /0 |
| Completion returns without error, stage29 | 46 | 47 |
| Resize whole sum / maximum,18 calls | 17.294 /3.802ms | 16.974 /3.570ms |
| UI / worker wall unions inside resize | 30.857 /1680.244ms | 31.340 /1658.145ms |
| First final-width layer after resize ends | 163.890ms | 198.543ms |
| Typing / wheel ACK maxima | 3.681 /0.605ms | 3.904 /0.530ms |

The new checkpoint does not fire in either cell. These results establish no
avoided metadata construction or application benefit. Stage29 means only that
the completion call returned without error; it is not proof of current kernel
publication. Stage100 includes layout aliases, and stage103 includes empty or
retirement turns. Worker104 outcome and UI27 delivery joins identify returned
answers, but do not establish the time their last owners are released.

Control frame/source1110/1111 joins acceptance1128 and first layer1131;
treatment1126/1127 joins1143 and first1149. Acceptance-to-first delays are
3.834583/12.067458ms. Latest1165/1177 are later repetitions. Both final-width
publication-inside-resize endpoints remain not established. Source captures
differ5→4, as do raster and completion progress. The overlapping interval unions
are wall observations, not additive CPU costs or comparable construction totals.

The frozen retention reader establishes control's24 discrete placements,
23 inside, but refuses treatment at an accepted-assignment identity join.
Independent raw reconstruction sees24 treatment placements across two accepted
publications, not one fixed A. This does not revise the reader's not-established
classification or prove continuous coverage, fresh hits or physical display.

Primary/UI/worker trace omissions are zero. Passive-mode classification remains
not established, and the recorded display is60Hz. All recorded resize and input
ACK scopes are below8.33ms, while fresh final-width content still arrives after
resizing ends. This is one instrumented pair, not a latency distribution or
physical120Hz result. Both pointers are released and all nine recorded processes
and four groups are absent. The116-file runtime archive is under
`target/apple-region-abandonment-pair-execution-v1/`; immutable products have
separate manifests and the independent lead reconstruction is a `lead-*` sidecar.

The checkpoint precedes the entire metadata loop. The existing trace cannot
locate supersession relative to that checkpoint. The next controlled fixture
tests bounded checks during metadata construction; no pressure retuning or
repeat of this pair is used to manufacture abandonment.

### 8.92 Check for supersession during compact metadata construction, 2026-09-18

The entry-only checkpoint cannot stop a request superseded while its numeric
line metadata is being built. `RegionParagraph` now builds the same ordered
array locally and calls the existing checkpoint before each new128-line chunk
and after the final nonempty tail. `RegionWorkerLayout` retains its entry check
and forwards that same closure. The existing nonthrowing initializer delegates
with a no-op closure. Only these two production files change.

The controlled fixture pauses after129 actual `RegionLine` constructors, then
supersedes the request through the real controller's `prepare` path. Baseline
constructs all600 lines. Candidate stops at256:344 actual constructors are
avoided. The next superseded job is protected and still constructs all600;
its valid completion rearms one later abandonment, which again stops at256.
Reset during metadata also stops the old job at256 while preserving one serial
worker and the new generation. Partial metadata never supplies runtime metrics
or a layout binding, and the old accepted provider remains intact.

The baseline has three intended failures among1516 assertions. Candidate passes
1532, including16 additional assertions for0/1/127/128/129 line boundaries,
unchanged no-op output, a source-free terminal, last-source release after an
actual serial destruction fence, and zero partial raster/index/hit charges.
Both completed600-line successors match ordinary full metrics, every baseline,
line range and glyph-ink bound, source copy and128,000 exact RGBA bytes. Existing
atomic assignment, retention, multi-move selection, current-request, stale
terminal and same-width height-alias controls remain in these actual runs.

Two test-only actor-annotation setup failures precede the successful baseline
compile; both are preserved. The corrected baseline and candidate use identical
test-main bytes and unchanged warning flags. Their compiled production copies
invert exactly to the claimed sources. All70 current ExactKit sources typecheck
with warnings as errors, and those inputs match the integration. The earlier
55-method unit runner remains unrun; it was not repaired or counted here.

This is a real worker/CoreText/AppKit fixture with Runtime/Session/Presenter
doubles, not a full-app performance result. The recorded count reduction does
not establish saved CPU or latency. An8155-line shape adds64 checkpoint calls;
8988 lines adds71, with unmeasured branch/lock overhead. A single CoreText call
remains indivisible, and supersession after the final check can still produce
stale complete output. Existing one-abandonment/required-completion fairness,
intrinsic-width behavior, epochs, accounts and accepted-image ownership remain.

The319-file source/fixture archive is under
`target/region-metadata-checkpoint-validation/freeze-v1/`. All seven execution
groups retired. The zero-abandonment application pair in8.91 is unchanged; this
fixture does not retroactively turn it into a gain or physical120Hz result.

### 8.93 First metadata-checkpoint application pair is incomplete, 2026-09-18

The new candidate builds successfully in95.658s. Its captured1418 source cards
and1471 receipt inputs differ from the entry-only control only in the two
metadata-checkpoint files from8.92. The generated application entry, plan,
compatibility bytes, observer and toolchain remain equal. The93-file candidate
capture hashes exactly; this is build evidence, not a runtime result.

A fresh control cell exits0, but the candidate driver exits1 after24ms because
the copied harness omitted `reuse/compiled-inputs.mjs`. No candidate application
or poster launches. Syntax parsing had not checked transitive import resolution.
The first-failure stop is preserved: no retry, replacement timing, source repair
or candidate performance result is included in this pair.

The control completes18 resize changes and twelve input requests, including six
actual alternating40px wheel moves. All twelve handlers and conservative sender
ACK brackets lie inside the AppKit edge. Three complete document Values remain
equal, with the1,048,499-byte giant paragraph unchanged. One intermediate869px
layout is accepted and assigned during resize: frame/source920/921 joins
accept937 and layer943,175.466500ms before the edge ends. This supports the saved
driver's in-edge new-width endpoint; it does not describe the final856px layout.
That final layout first joins1133/1134 through1150 to1156,191.293709ms after the
edge and10.264626ms after acceptance. Layer1184 is its fifth assignment, not the
first. Retention remains the frozen `NOTESTABLISHED` result; discrete placements
span two publications and do not establish continuous coverage.

Unlike the earlier pair in8.91, this control records one actual entry-checkpoint
abandonment: request56 at873px, inside the edge. All67 shape attempts join66
shape returns, including aliases, plus that abandonment. Six raster returns and
one raster refusal are separate outcomes. Each observed terminal joins its
worker parent and UI delivery;49 successful completion returns are not49 fresh
publications. This is evidence that the old checkpoint can fire, not a comparison
of old and new checkpoints or a measurement of saved metadata work.

The61-file failed-runtime archive is under
`target/apple-region-metadata-pair-execution-v1/`, separate from the successful
candidate capture. Its original report and raw endpoint remain unchanged; the
lead's independent raw joins qualify intermediate and final widths separately.
All six recorded PIDs and three groups retired, with exactly one actual control
poster release and no invented candidate cleanup. The next step is a separate
source-only import-closure correction. No full-app benefit, physical120Hz or
robust latency-tail result follows from this incomplete pair.

### 8.94 Current Web Messages separates short tasks from long frame waits, 2026-09-18

One unmodified00272ea Messages build and one headless Chrome cell use the current
nine-field History and default ReusableMessagesStress. The workload supplies all
10,000 records, observes32 changed records after a positive revision, and keeps
viewport mounting enabled. Silent revisions advance0→8, loaded8→16, and recovery
stays16. All72 scheduled offers and24 exact input echoes pass across the three
fixed2s phases. The existing cadence, four-outstanding limit and trace caps are
unchanged. No diagnostic Rust hooks or smaller workload are introduced.

The build exits0 in25.218s. An initial capture assertion incorrectly compares raw
Git index bytes across the planned mtime/stat-cache refresh. That failure remains
preserved. Offline finalization verifies all1417 stage0 path/mode/blob entries
against the pinned tree, exact source bytes and the first build's619 saved input
files, linked products and generated factory. No producer or build is rerun.
The actual optimized Wasm is3f6a00d7, with plan392a1006; older834d timing is not a
control for this product.

The59,650-event trace has no reported loss, reaches EOF and closes its stream.
Independent frame/process/thread and outward clock reconstruction agrees with
all9678 selected-thread spans. Tasks fully contained in idle/load/recovery number
360/306/267, with wall unions114.947/92.826/66.571ms and maxima7.992/8.199/3.805ms.
None of those tasks exceeds8.333ms; two crossing tasks per phase remain separate.
The largest unscored whole task is21.702ms. Loaded exports include eight advances
with maximum2.2ms and eight resizes with maximum3.5ms. These are inclusive wall
observations, not CPU, isolated layout cost or one export per revision. Only80
of205 exports have conservative whole-task joins; missing joins stay unavailable.
Absent EST1 packets make query/shape/key/index work counts `NOT_ESTABLISHED`.

Input-capture-to-echo maxima are1.2/1.3/1.0ms, but the90 scored frame-callback gaps
have maxima116.7/216.7/250ms. Input-to-next-callback maxima are78.7/99.4/96.6ms.
The actual frame/callback232 request43317 and fire43782 are229.752ms apart; the
recorded main-thread task union within that interval is3.366ms across22 tasks.
This wait is not explained by one equally long observed application task. It
does not prove CPU idleness or identify a throttle, compositor or OS cause.

The carrier is Chrome153 headless, with target focus emulation and no explicit
frame-rate override. Visibility/occlusion and a complete frame-scheduler chain
were not captured. Existing AnimationFrame presentation markers are browser
breadcrumbs, not physical scanout receipts. Generic scrolling is observed but
wheel causality remains unestablished. There is no paired optimization result,
robust tail guarantee or physical120Hz proof on the known60Hz display. More Web
threading is not justified by these task costs alone; a separate visible-browser
discriminator is prepared without changing application work.

The18-file runtime archive is under
`target/web-current-messages-00272ea/cell-v1/`. The sole execution exits0, all
three recorded processes and the owned group retire, and its port is free.
The independent raw reconstruction and carrier note are separate from the seal.

### 8.95 Corrected metadata pair stops at native startup admission, 2026-09-18

The missing module from8.93 is restored byte-for-byte from the earlier frozen
driver. Both entry points' source-only import closure resolves42 modules and220
edges. Only the treatment's shared-driver import and new control-cohort path
change; existing app products, readers, geometry policy and workload remain.
The original missing-module attempt is unchanged.

One fresh control completes. The candidate now launches, but its fifth command,
the second initial full-state read, fails `pending/unknown native layout` before
any poster, resize edge or measured input workload. It has a current accepted
940px paragraph and image at publication15/generation2/artifact14/source11, with
a963×400 clip and matching current certificate. Only the RegionSurfaceMac node
reports `needsLayout=true`; its constraint flag and other sampled layout flags
are false. Current publication and Cocoa-layout admission are distinct checks.
The saved sample does not establish why that flag remains set, later settlement,
incorrect pixels or causation by the metadata checkpoint.

The control's first final-width assignment is178.222ms after its edge; it records
64 shape attempts/returns, four raster returns and no abandonment or refusal.
Those control-only facts cannot replace missing candidate timing. All seven
recorded PIDs and three groups retire, with one actual control poster release
and no invented candidate release. The75-file archive is under
`target/apple-region-metadata-pair-corrected-execution-v1/`. This second attempt
is also failed/incomplete. No third run, added wait, relaxed layout admission or
performance claim follows; the next action is source diagnosis of the flag.

### 8.96 Visible-browser discriminator completes the workload but caps its trace, 2026-09-18

One headed Chrome cell uses the exact00272ea/3f6a00d7 product from8.94. The only
executable change removes `--headless=new`; the fresh profile, target focus,
fixed2s phases, full10,000/actual32 workload, deadlines and observation caps stay
unchanged. All72 offers and24 echoes pass. Silent revisions advance0→8, loaded
8→16, and recovery stays16, with no application/observer errors or drops.

The overall execution nevertheless exits1: trace draining reaches the unchanged
16,777,216-byte limit on read65. The original partial JSON is retained. Reported
no-loss completion and successful stream closure do not make it complete.
Whole-task and script/style/layout/paint attribution are unavailable; the trace
analyzer is not run and the partial stream is not repaired. No retry or larger
capture follows. The generic supervisor failure label does not override the
separate functional PASS and trace FAILED results.

Independent reconstruction of the complete page-observer log counts107/106/105
scored frame-callback intervals in idle/load/recovery. Their medians are16.7ms
and maxima35.1/33.4/33.5ms. Echo maxima are1.4/1.5/1.3ms; input-to-next-callback
maxima are2.9/22/29ms. Loaded exports comprise eight dispatches, eight advances,
eight resizes and32 feedback calls, with maxima0.9/4.5/0.2/0.4ms. Export interval
unions27.600/47.000/13.600ms exclude other browser work and are not whole-task or
CPU totals. An unscored13.3ms dispatch remains separate. Missing EST1 packets and
wheel-causal evidence remain unavailable.

The historical headless gaps up to250ms are absent from these scored page
observations. This is not a fresh matched comparison: the headed idle port
height changes547→532, and no independent OS-front/occlusion or physical scanout
receipt is captured. No application optimization occurred. The result narrows
the relevance of the test carrier without identifying a throttle or proving
that threading will improve it. Callback spacing near16.7ms on the known60Hz
machine does not establish physical120Hz or a robust latency bound.

The15-file failed-trace archive is under
`target/web-current-messages-00272ea/cell-headed-v1/`. All three recorded PIDs,
the owned group and its private listener retire before the next native lane.
Source/product identities and the original headless archive remain unchanged.

### 8.97 Final metadata comparison refuses unequal scrollbar geometry, 2026-09-18

Source diagnosis of8.95 finds that RegionController.flush marks the Surface for
layout even when its outer frame is unchanged; image assignment can complete
independently. This does not identify the saved sample's last setter or establish
a product defect. No production invalidation guard is added: document extent,
scrollbar policy and other geometry can change without changing that outer frame.

A separate three-module reader correction includes strict Cocoa admission in
initial readiness. Known boolean layout/constraint dirtiness may consume the
existing12-check/15s/1s cadence only after all other source, geometry, topology,
scale and certificate checks pass. Unknown flags and invalid geometry still
throw immediately. The shared28-read cap,60s driver,75s watchdog and separate
strict pre-edge/cross-arm guard remain. Both actual caller extracts pass14 pure
tests after the old readers produce eight passes and six behavioral failures;
setup failures remain separate. Synthetic pending-to-settled samples test reader
logic, not actual AppKit settlement. No native source or product changes.

The third and final fresh C→T attempt passes each arm's strict ready and pre-edge
admissions, then refuses the candidate before its poster. The control's legacy
style0 has a963×400 clip; the candidate's overlay style1 has a980×400 clip. Each
matches its own independent Cocoa sizing and current pixel certificate, but
the sampled cross-arm tuples differ. The underlying policy difference is not
explained. Both apps already report clean layout flags at their full ready
sample, so this run does not exercise the new known-pending wait branch.

The control completes one edge; the candidate has no measured resize/input
edge, final-width result or performance comparison. All seven recorded PIDs and
three groups retire, with one actual control poster release and none invented
for the candidate. The82-file final archive is under
`target/apple-region-metadata-startup-execution-v1/`. The original missing-module
and pending-layout failures remain sealed. This measurement campaign is stopped
after three attempts: no fourth retry, forced scrollbar policy, wider admission
or replacement timing is selected. The controlled metadata constructor savings
in8.92 remain valid; full-app savings and physical120Hz remain unproved.

### 8.98 Current Mac Messages baseline exposes remaining resize costs, 2026-09-18

One fresh8d9858c build uses the unchanged default Reusable factory and current
seven-argument/History9 data contract. Earlier630 native products have six
arguments/History5 and cannot supply a current comparison. Only the three
Session/Agent/main observers are ported; current collection dataReady, common-mode
timer and cleanup stay intact. The94.364s optimized build and one15.896s cell
exit0. The actual receipt, generated factory and plan match the current app.

The full10,000-row, actual32-change workload remains unbounded at the data seam:
boundedfalse/cursorNone, windowed display, no eager or smaller-answer substitute.
All36 offers receive acknowledgments inside the three AppKit edges, with18
typing inputs and18 alternating measured40-unit wheel movements. Each phase has
18 complete resize chains strictly inside its edge. Current-tail admission
allows the loaded offset to change by324.8; it does not restore an obsolete
absolute scroll offset. Full outside-edge History checks retain all rows, keys,
UTF8 bodyBytes, revised suffix and cursor metadata. Loaded state progresses8→19.

Resize whole-wall medians are8.663/8.796/9.851ms in idle/load/recovery, with
maxima15.400/13.872/12.181ms. Respectively9/18,12/18 and13/18 exceed1000/120ms:
34 of54 total. The Runtime.resize+decode medians are5.654/5.687/6.230ms and
separate Swift apply medians2.582/2.832/3.390ms. These include synchronous Swift
measurement callbacks and other native work, not isolated Rust or CoreText CPU.
There is no current C96 or Bridge-split observation to assign finer causes.

The seven strictly-inside loaded timer chains have a4.942ms whole median and
7.994ms maximum, but that subset is insufficient to claim the budget is met.
The broader loaded state bracket has11 chains, two above8.333ms and a9.511ms
maximum. Eight command-free producer chains over2001.664ms advance revision0→8;
four exceed8.333ms, with13.462ms whole and9.588ms core maxima. Loaded typing ACK
reaches23.334ms, and recovery wheel ACK15.995ms. No revision count substitutes
for linked callback counts, and these small samples are not robust tail bounds.

Primary records have no omissions. Passive tracking omissions retain mode
NOTESTABLISHED despite positive samples. The instrumented cell establishes
resize/input endpoints and autonomous progress, not an old-versus-new gain or
physical120Hz on the known60Hz display. All seven recorded PIDs and five groups
retire, including four owned poster releases. The187-file runtime archive is
under `target/messages-current-native-execution-v1/`, separate from its immutable
build and source seals. Next source investigation targets work repeated during
distinct-width resize; no new threading or layout policy is selected here.

### 8.99 Full browser paragraph fails the fixed offer schedule, 2026-09-18

One unmodified8d9858c Web build and one ordinary headed Chrome cell exercise the
full1MiB paragraph. Historical browser1MiB evidence used many small blocks;
unsplit browser evidence was only16KiB. The default synchronous MarkdownStress
factory and small bake stay unchanged. Actual controls select paragraph, wide
column and1MiB; the initial complete DOM paragraph is1,048,499 UTF8 bytes with
the canonical digest, within1,048,531 source bytes and two supplied blocks.
Revision stays0 and reparsing is paused. The actual reading column changes
1185↔1105px in all three fixed2s windows; no truncation or splitting is used.

The cell exits1 and remains FAILED/functional NOT_ESTABLISHED. Its fixed72 offers
include58 sent requests and14 capacity skips, distributed5/5/4 across windows;
four outstanding requests trigger the unchanged cap. No late-offer skips,
page errors or observer drops occur. All19 sent typing requests eventually echo,
but only16 input handlers begin within the fixed windows; three arrive after
their endpoints. Successful partial delivery does not satisfy72-offer acceptance.

Independent reconstruction counts24/23/24 fully-contained frame-callback gaps,
with maxima349.9/333.4/350.1ms. Request ACK maxima are610.470/611.087/602.809ms.
Conservative clock brackets place the slowest sent typing request's DOM capture
roughly264/265/256ms later in each window, separately from fast capture-to-echo
handling. Wrapped Wasm interval unions are only1.4/1.5/1.4ms per window; their
coverage within each contained gap over50ms is at most0.501ms. This locates much
of the delay outside those exports, without identifying browser layout, paint,
compositor, transport or CPU causation. No Chrome task trace or EST1 is collected.

Cold1MiB selection takes469.804ms to ACK and471.971ms to DOM readiness. A37.4ms
document dispatch and468ms boot longtask have different, overlapping boundaries;
they do not isolate parsing or prove input responsiveness during cold work.
The final full-text digest is unavailable: after offer assessment fails, the
catch path replaces the computed full snapshot with a metadata-only snapshot.
The initial digest and final two-block/revision metadata remain, but no retained
before/after text equality or end-to-end PASS is claimed. The original failure
and raw records remain unchanged; no retry, wider cap or smaller workload follows.

The15-file runtime archive is under
`target/web-giant-markdown-8d9858c/cell-v1/`. All three recorded PIDs, the owned
group and private listener retire. Callback timing on the known60Hz carrier is
not physical presentation or120Hz proof. Next work is source-only pricing of
failure-proof evidence retention and a bounded observation of the missing
browser work, before selecting a production optimization.

### 8.100 Reuse the stored Swift text length, 2026-09-18

Ordinary text layout now reads `shape.identity.utf16Count` instead of recounting
each run through NSString. The immutable identity already stores that count;
applying paint preserves the run texts and order. This removes repeated work
without changing wrapping, widths, cache policy or ownership. Its cost saving
has not been measured and may be negligible for short Messages text.

The one-expression change passes the existing standalone geometry fixture:
38 methods,7,022 assertions, zero failures, compiled with strict Swift6 and
warnings as errors. Unicode, viewport pixel, cache and lifetime controls remain.
The lightweight assertion shim excludes four Session/View-dependent methods;
this is not full XCTest. A current70-source Mac module typecheck also exits0.
All73 captured source/header/test inputs match the integrated composition,
with only Text.swift changed from5a7ed29. No native timing comparison, allocation
measurement, UIKit validation or120Hz improvement follows from these checks.
The99-file source and validation archive remains in
`exact2-apple-text-length/target/apple-text-length-validation/freeze-v1/`.

### 8.101 Browser trace localizes giant-width stalls to layout, 2026-09-18

One further diagnostic cell keeps the immutable8d9858c Web product, full unsplit
1MiB paragraph and72-offer recipe from8.99. A separately tested ignored driver
preserves the full final document proof on assessment failure and traces only
the third2s window, after the second window drains. It reuses the existing
four-category,16MiB trace carrier; no rebuild, larger cap or runtime retry occurs.

The workload still fails:57 requests are sent and15 skipped at the four-request
capacity limit, five per window. All17 sent typing requests eventually echo,
but only15 handlers begin within the fixed windows. Before and final DOM proofs
now both retain the canonical1,048,499-byte paragraph digest; this separate source
check does not turn failed offer acceptance into a pass. The original8.99
failure and unavailable final digest remain unchanged.

The complete2,180,224-byte trace contains9,476 events with no loss or truncation.
Frame/process/thread identity and conservative clock brackets join the selected
renderer. In the third fixed window,178 fully-contained tasks occupy1377.535ms
of wall time;14 Layout intervals occupy1335.558ms. Four long tasks contain
326.379,326.087,357.005 and325.708ms Layout intervals. Each names the selected
frame and full document root at alternating1120/1200px viewport widths. Script,
style and paint unions are8.926,0.189 and22.490ms; these nested wall families are
not additive CPU costs. One324.274ms task crosses the endpoint and a333.505ms
task follows it, both excluded from the four contained misses.

This identifies browser layout as the long measured renderer work, without
isolating a text subroutine or demonstrating a fix. It supplies no evidence
that moving parsing to another worker would remove these layout intervals.
The first two windows have no trace attribution. Frame-callback gaps still
reach335ms, and tracing overhead is uncalibrated; no matched gain, physical
presentation or120Hz claim follows. All recorded processes and the private
listener retire. The20-file frozen archive is under
`target/web-giant-markdown-8d9858c/cell-trace-v1/`. A bounded source check finds
no supported duplicate-layout patch: all six long Layout intervals across the
whole trace correspond in order to actual width changes, while a repeated1200px
offer adds no giant layout. The four contained long tasks have no recognized
script interval. Inactive context, collection and gesture paths do not supply a
demonstrated cause. Observer reads remain a possible perturbation, not the
identified trigger of these four FrameWidget IPC tasks. No further browser run
or production change is selected from this evidence; full content and actual
wrapping remain required.

### 8.102 First current Mac stack-sampling attempt produces no report, 2026-09-18

One diagnostic reuses the immutable8d9858c Messages product from8.98, its
full10k/batch32 workload and unchanged ordinary-clock driver. An external
`sample` process attaches to the freshly verified app PID at the existing load
marker, requesting3s at2ms intervals with a10s sampler deadline. The app's75s
watchdog and workload remain independent of the sampler.

Profiling fails. After5.151s of sampler process lifetime, the coordinator's exact
target-identity check no longer matches and sends TERM only to the sampler.
Its only output is the sampling-start line; no stack report exists. The rejected
process snapshot was not saved, so target absence and an identity change cannot
be distinguished retrospectively. This is not evidence of a tool permission
refusal, completed3s sampling, symbolication cost or a particular app hotspot.
The proposed repeated visible-text scan remains an unmeasured source hypothesis.

The native driver and watcher exit0 without watchdog actions. All three phases
retain18 resize changes and12 input handlers/ACKs, including six actual signed
40px wheel movements. Full10,000 rows and actual32 changed rows remain; tracking
mode is still not established because its observations overflow. The silent
interval contains eight timer chains, four exceeding8.333ms, with an11.8345ms
maximum. Attempted sampling adds uncalibrated disturbance, so these observations
are neither a matched timing comparison nor120Hz evidence. All nine recorded
PIDs, seven groups and four owned poster releases close. The184-file failed
profile/native-capture archive remains under
`target/messages-current-sample-execution-v1/`.

A separate earlier-attachment variant is being prepared, keeping the same
duration, interval and deadlines. Its possible coverage includes startup and
setup; it cannot be labeled load-only or full10k-only. The failed first attempt
is preserved, and no production optimization is selected from absent stacks.

### 8.103 Earlier Mac attachment obtains sparse, inclusive stacks, 2026-09-18

The second profiler attempt attaches once the existing watcher records the fresh
app identity, keeping the same8d9858c product, native recipe and3s/2ms request.
Eleven focused pure controls cover the changed trigger, ownership lag and refusal;
the two old-trigger cases fail as intended. Sampling exits0 and writes a516,615-byte
report within its10s deadline. Native execution also exits0, and all nine recorded
PIDs, seven groups and four poster releases close. The failed first attempt stays
unchanged; no third sample is selected.

The report contains1,305 main-thread samples. Independent reconstruction of its
1,578 main-thread tree nodes preserves every parent/child count and the total
leaf weight. Of those samples,1,174 end at `mach_msg2_trap`; two include
`Presenter.refreshVisibleText`, seventeen include `Presenter.apply`, and
twenty-three include `TextEngine.measure`. These inclusive families overlap.
They are neither invocation counts nor CPU time, and absence of an optimized
function name does not establish absence of its work.

The two visible-text scan branches establish that apply-end refresh and a
wheel/clip-notification/collection-feedback route execute. They do not establish
duplicate scans within one batch or a dominant cost, so this evidence does not
select refresh coalescing. The sampler process lives5.835s including preparation
and symbol processing, ending before the load poster starts. Its early trigger
can include setup and the default state; the condensed report has no per-sample
timestamps for exact phase attribution. It cannot explain the loaded resize
tails or establish a full10k-only profile. The unchanged native run still
completes all36 input offers and the full10k/batch32 workload, with tracking mode
unestablished. Sampling overhead is uncalibrated and the display remains60Hz.
No timing gain or120Hz result follows. The184-file archive is under
`target/messages-current-early-sample-execution-v1/`.

### 8.104 Reuse exact strut extents within one text layout, 2026-09-18

Text layout already computes its paragraph minimum line box from the strut.
Intersecting authored runs with identical size, weight, family, italic and
explicit line height now reuse those two extents. Floating fields must be finite
and bit-identical; mismatches, signed-zero differences and nonfinite values keep
the original calculation. Width-specific shaping, authored-range intersections,
each extent contribution, normal/fallback font metrics, clamping and rounding
remain unchanged. This adds no cache, retained owner, callback or worker.

The exact d85a2a1 baseline and candidate compile against the same test body.
A test-only counter at the actual extent computation records five calculations
for four uniform lines before the change and one afterward. The baseline has
one intended assertion failure among41 methods/7,422 assertions; the candidate
passes all7,422. The ordinary, uninstrumented candidate passes41 methods/7,063
assertions, and the complete70-source Mac module typechecks. This is the existing
assertion shim, with four Session/View-dependent methods excluded, not full
XCTest or UIKit SDK execution. Compiled reference/counter injections reverse to
the exact production sources; neither is present in production.

Controls retain exact geometry, source ranges, point hits and viewport pixels
against the original layout, including mixed fonts, coalesced spans, fractional
heights, emoji fallback and hidden clamped suffixes. Nonfinite-height layouts
are compared without rasterizing them; nonfinite font sizes are not executed.
The original reference shares the engine/font infrastructure. The285-file
freeze is preserved under `target/apple-strut-extents-validation/freeze-v1/`.

This proves fewer repeated calculations in that fixture. It does not price the
new comparisons or establish fewer allocations, faster callbacks or native
frames. A fresh current-source native comparison remains unrun; the older
86ef control lacks the already-landed stored-length cleanup and cannot isolate
this change. No120Hz claim follows.

### 8.105 Guarded strut reuse: first native pair is mixed, 2026-09-18

Two fresh optimized d85a2a1 builds and one ordered control→treatment pair complete
without retries or watchdog actions. Both include the stored-length cleanup,
default Reusable producer and identical observation code; the258 captured source
inputs differ only in §8.104's Text.swift change. Generated entry, plan, compatibility
record, toolchain and build configuration match. Each immutable capture has272
files; binaries are a455d481 and2eb53927. The old86ef run is not a control.

All six phases retain the full10,000-row History9 response, actual32-row changes,
18 accepted resize dimensions and12 input handlers/ACKs each. Window, port,
Quartz points and wheel paths match. Each phase's18 resize chains is wholly
inside its AppKit edge. Full History values match at equal arguments; loaded
progress is8→18 versus9→19, so equal geometry does not establish equal timed work.
Both arms reach revision19 in recovery.

| Strict resize cohort | Control | Treatment |
|---|---:|---:|
| Idle whole sum, ms | 167.528 | 157.495 |
| Load whole sum, ms | 144.985 | 147.692 |
| Recovery whole sum, ms | 169.453 | 165.334 |
| All54 whole sum, ms | 481.966 | 470.521 |
| Whole spans exceeding8.333ms | 33/54 | 33/54 |

The pooled whole span is2.37% lower in this pair, while the loaded phase worsens;
this does not establish a consistent native gain. Loaded timer medians improve
7.361→4.212ms, but maxima worsen8.195→8.853ms. Both command-free2s intervals have
eight complete timer chains and five budget misses; their maxima worsen
11.519→13.101ms. Loaded typing ACK maximum worsens21.010→26.793ms, loaded wheel
7.285→10.897ms and recovery wheel2.661→15.056ms. Six inputs per type per phase
are not a robust tail distribution. Resize maxima still reach13.442ms in the
treatment. No reverse pair or additional sampler is selected from this result.

Independent raw reconstruction covers all108 resize chains, phase timers,
silent chains and72 input joins, plus selected full History and geometry checks.
Primary omissions are zero; tracking mode remains unestablished because its
separate observations overflow. The core interval includes Runtime work,
synchronous Swift measurement and decode, not isolated strut or CoreText CPU
time. Prefit work outside the resize probe is not measured. Prior display
metadata is60Hz; backing scale is not independently captured. These results
neither establish physical120Hz nor turn §8.104's calculation count into a
latency claim.

All14 recorded PIDs, ten groups and eight owned pointer releases close before
the final report. The370-file runtime archive at
`target/messages-strut-native-execution-v1/` is sealed by manifest9e9468aa;
report42e70c2a preserves the full tables, progress differences and negative tails.
The separate569-file build archive and source preparation remain unchanged.

### 8.106 Linux Messages reaches display but refuses tail setup, 2026-09-18

One optimized7384567 Linux Messages build completes in40.769s. The actual
generated entry uses ReusableMessagesStress; source, plan, compatibility and
fonts are pinned. This fixed1024×768 VKMS diagnostic is intended to establish
the full10,000-row/32-change workload with its changing suffix actually visible.
It does not exercise continuous window resizing. Four ignored observation paths
retain CPU-picture ownership and bounded scalar row witnesses; production is
unchanged. The20-file build capture identifies ELF c4ecfff1.

Reader checks distinguish painted History from newer live state, seal the silent
2s prefix before draining, close later dirty/timing work at the final fence, and
check setup time and records at the actual handoff. The separate v5 diagnostic
budget permits49,152 setup spans/16,384 events/32MiB, reserving4,096 spans,
4,096 events and8MiB for later work. Total bounds are53,248 spans/20,480 events/
40MiB. The native recorder remains256 fixed rows per turn with depth32; setup20s,
native60s, outer220s, transport250s and2.5GiB address space remain unchanged.
These are diagnostic capacities, not a receiver-memory bound or proof that
natural tail traversal fits. Saved focused validation passes28 pure tests;
earlier reader failures and setup errors remain preserved.

The first attempt exits before driver or app launch: the ordinary VM user lacks
read/write access to the root:video0660 DRM node. A requested Runas-group route
also refuses noninteractive authentication. A separately qualified sudo→setpriv
launch instead drops all real/effective/saved user IDs to501 and group IDs to44,
retains the original home/lock, and enables NoNewPrivs before running Python.
Only the recorded sudo monitor retains root privileges. No account, device,
ACL, module or persistent permission changes are made. The original refusals
remain separate from the corrected attempt.

The corrected attempt reaches actual native display and accepts the windowed
and batch32 controls, followed by four real+40 wheel inputs while paused at100
rows. It exits1 during that first setup batch. Independently reconstructed raw
records show frame5/input11→frame7/input23, scroll0→160 and an unchanged
`[0,211,1024,471]` transcript rectangle. The picture-state comparison differs
only in kernel commit epoch5→6. The reader's cross-picture equality guard rejects this
as changed geometry/state; neither a production defect nor a weakened identity
rule follows from that rejection.

All91 journal records,16 timing turns and258 spans are retained, with seven CPU
pictures, no recorder failures and no timer events. No tail checkpoint, full10k
transition, History snapshot, RGB capture, silent interval or scored phase
completes. Thus setup remains unestablished; this is not a successful baseline,
pixel acceptance, performance result or120Hz claim. The native process exits-15
after the reader's owned cleanup SIGTERM, not an application crash. All six
recorded Mac/guest PIDs and owned groups retire; the listener is absent.

The corrected archive is
`target/messages-linux-vkms-tail-runtime-v5-setpriv-v1/`, report ad8cf857 and
manifest51bf3464. The original prelaunch failure remains under
`target/messages-linux-vkms-tail-runtime-v5/`. Next work is a source-only
investigation of cross-picture setup qualification while preserving each
picture's exact paint/ACK/current-state guards; no further runtime is selected.

### 8.107 Setup epoch correction advances, then a newer anchor stops the final attempt, 2026-09-18

The setup reader now distinguishes the kernel's global commit epoch from the
History revision. Collection realization can change kernel nodes during a
wheel without changing message data. Only comparisons between setup pictures
permit a nondecreasing positive epoch; all other sampled identity/workload keys,
incarnation, paused full/windowed100-or10000/batch32 state and target geometry
remain qualified. Intermediate input/timer/snapshot states cannot change and
then hide a reset or data mutation. Each picture's own paint/ACK epoch equality,
latest current coordinates, closed turn, final fence, silent prefix and scored
painted-History rules remain unchanged.

Replaying the actual91-event failure produces one intended old-reader failure
and three passing controls. The isolated candidate passes11 focused tests,
including stale ACK, dirty C, unacknowledged picture, incomplete timing turn,
geometry, data and incarnation controls. The batch mark is reconstructed from
the saved initial-mask boundary and exact driver fields; the original driver
did not save the refused batch. This is reader evidence, not a retroactive PASS
for §8.106. Only acceptance.py changes behavior; the other four readers, native
binary, recorder, fixed capacities and timing limits stay identical. The new
reader adds bounded prefix scanning, whose runtime overhead is not isolated.

One final third command in this attempt sequence uses the qualified nonroot
graphics route and ELF c4ecfff1. It exits1 after2.152s during paused100-row setup.
The epoch correction accepts51 four-wheel batches,204 notches, reaching
scroll8160. Before the next batch, the driver's cached frame74 is superseded by
an independently acknowledged frame75, so its existing exact frame-ID guard
refuses `setup anchor superseded`. No further input batch is sent.

The last accepted batch closes at seq1916. Paused timer1917 changes the clock
and marks the state dirty; picture75 then submits and ACKs clean before turn1924
closes. Input623, epoch89, incarnation, sampled workload, scroll8160 and transcript
`[0,211,1024,471]` remain equal between pictures74/75. Only sampled clock changes
1501.568→1752.265ms. Their recorded CPU-pixel hashes also match; no RGB capture
or independent pixel oracle ran. The trace establishes why the reader stopped,
not a production correctness defect or a successful performance baseline.

The preserved setup journal has1,924 records,370 turns,5,340 spans,75 complete
picture chains and seven timers. Recorder failures are zero and the largest
turn has59 of256 slots. No budget, deadline or disk-floor refusal occurs. No
tail checkpoint,10k transition, full changed-suffix workload, silent interval
or scored phase completes. Native exit-15 follows owned failure-cleanup SIGTERM;
all six recorded Mac/guest PIDs and groups retire, and port5941 has no entries.
The archive at `target/messages-linux-vkms-tail-runtime-v5-setpriv-epoch-v1/`
has34 files, manifest9ed40e2b and report69debf03.

The three-command attempt sequence is stopped; no fourth run, wider limit or
easier workload is selected. Source-only follow-up examines unnecessary repaint
after a provably unchanged timer while preserving effects and independent
invalidations. Separately, the complete Messages viewport worker path still
needs faithful collection, decoration and action ownership; a text-only stand-in
would not satisfy this workload. Neither investigation establishes a speedup,
physical120Hz or a completed Linux baseline.

### 8.108 Unchanged Linux timers no longer force a repaint, 2026-09-18

The ordinary Linux timer path now requests painting from actual commit effects
instead of marking every advance dirty. It still executes every due action,
preserves receipt order, performs the same layout and motion work, and runs all
post-commit services. Node/layout changes, navigation, the final animation sample,
errors and registered content regions conservatively demand painting. Existing
dirty state is only ORed with new demand; a pending picture or later mutation is
never cleared. Focus retirement, scroll clamping, images and collection feedback
retain their own invalidations. Nonvisual commands wake the existing executor
FD without requiring a paint; an appearance command marks its visual change.
Explicit agent clock operations and other commit callers stay conservative.

Twelve tests use the real Presenter and counted CPU backend on macOS. With old
production, nine controls pass and three assertions fail: paused due and
not-yet-due timers each cause one extra paint through the ordinary submission
gate, and a command-only timer does not independently wake the loop. The exact
same test bytes pass with the change. All29 display tests plus18 existing
collection/height controls pass, as do strict exact-linux all-target Clippy and
scoped formatting. Two earlier test-authoring failures are preserved separately.
Whole-workspace formatting still fails in external Snapback sources, untouched
here; this is not a blanket workspace-green result.

The three-path freeze is in the private `exact2-linux-noop-timer` worktree at
`target/noop-timer-validation/freeze-v1/`, manifest7e2e0371, patcha77d5e37.
MAIN received those exact tested sources; its integration receipt is at
`target/linux-noop-timer-integration/`. Full layout and synchronous post-commit
scans remain. These checks establish avoided paints in the tested cases, not
native timing, a completed Messages baseline or120Hz. Linux-only display code
was not exercised by the Mac tests, and the stopped native attempt sequence
remains stopped.

### 8.109 Admit an independent flex-height region shell, 2026-09-18

The complete Messages transcript cannot use a fixed height without changing
its composer allocation. Region eligibility now also permits a narrowly
qualified auto-height owner: a direct child of an explicitly sized, unwrapped
root column, with zero flex basis, grow/shrink1, explicit minimum height,
content-independent size limits, definite width and hidden overflow. Baseline
dependencies, nonzero padding/borders, aspect transfer and unsupported parents
remain refused. Both outer offers must be finite, nonnegative and definite
before shell work or request/catalog changes. Existing explicit-size behavior
and the64-offer budget are unchanged.

An actual Messages Contract fixture preserves the entire List block, all four
paragraph kinds per row, Reply controls and the composer. It uses the full
10,000-row/batch32 source with a test-only region envelope. Its admission test
fails on old production and passes after the eligibility change. Eight
width/draft/Reply combinations preserve ordinary owner and complete composer
geometry; the negative content-sized control still changes when its children
are cut. Six new kernel cases cover unsupported styles, intrinsic outer offers,
lost eligibility, explicit limits and complete A retained while a newer request
and composer change proceed. The latest small B matches ordinary geometry;
stale malformed delivery is refused before validation and reset retires it.

All34 kernel content-region tests and24 Messages reuse tests pass. Strict
all-target Clippy for both affected packages and scoped formatting also pass.
Earlier
fixture failures are retained: ScrollView was corrected to the actual List,
and the cut owner's deliberately empty shell content extent was distinguished
from exact descendant extents. Production was unchanged by those corrections.
The35-file freeze is in `exact2-messages-region-consumer/target/messages-region-consumer-prep/eligibility-freeze-v1/`,
manifest594be7ca, patchdd19c303; MAIN's exact-source verification is in
`target/messages-flex-region-integration/`.

This only removes the independent-size admission obstacle. A cold ordinary
Monospace layout plus final paint needs108 exact offers at980px and90 at896px
after a real32-body step, across six/five mounted rows. Those are ordinary
inventories, not measured native or complete region replay counts. The actual
full Messages candidate still refuses at64 with no partial publication. Native
source/artifact capacity, coherent collection/decorations/actions and full
worker publication remain unfinished; no app activation, native timing gain
or120Hz result follows from this kernel increment.

### 8.110 Complete Messages replay exposes capacity and coordinate gaps, 2026-09-18

A separate CPU diagnostic temporarily raises only the private exact-offer
ceiling to1,024, retaining the4,096-node and16MiB source limits. The actual
full10k/batch32 Contract uses the same test envelope and all16 bootstrap rows,
including sender, body, metadata and Reply text. Each candidate is discovered
through the real first-missing-request API and exact Monospace answers, followed
by all final paint offers. No ordinary layout supplies missing region metrics.
An independent ordinary kernel supplies the comparison and feedback inputs;
preceding completed-stage comparisons check the corresponding heights. This
does not exercise native presented-picture feedback or payload memory.

| Complete stage | Rows / paragraphs | New replies / accepted offers |
| --- | --- | --- |
| Bootstrap |16 /64|288 /288|
| First tail feedback |33 /132|594 /594|
| Settled tail |6 /24|0 /108|
| Narrower width |6 /24|78 /108|
| One batch32 step |6 /24|39 /108|
| Feedback after that step |5 /20|0 /90|

The first tail refinement naturally mounts33 rows; none are omitted to fit.
Old A plus complete B reaches882 exact artifact slots/unique tuples, while
deduplicated canonical source UTF8 peaks at12,711B. These are logical tuple and
source counts with marker payloads, not native allocation sizes. The real step
advances revision0→1 and changes32 data rows; six mounted body sources change,
and one unchanged sender needs additional exact offers. Ten stages complete
discovery with999 new replies and1,009 compute passes. Quiet completion says
nothing about keeping up with a continuing4Hz producer.

The strict comparison **fails**: ten bootstrap descendant Y coordinates differ
in floating-point bits, for example499.8 versus499.80002. All later stages,
widths/heights/X, extents and composer checks match. Different addition order
between local projection and ordinary root-first flattening is a hypothesis,
not a verified repair. The first failed assertion and the second diagnostic's
aggregate failure are both retained; no tolerance or third run was introduced.
All original private bytes/modes were restored, including the production64
limit; a distinct diagnostic Cargo configuration isolates its artifacts.

The28-file archive is `exact2-messages-region-consumer/target/messages-region-consumer-prep/replay-inventory-v1/`,
manifest17b0f588. Neither128 nor512 is justified by this inventory, and1,024 is
not a selected shipping policy. Native capacity, exact projection, interactive
publication and progress under sustained updates remain unfinished.

### 8.111 Parent-first region projection, 2026-09-18

A default64 regression isolates the coordinate failure without repeating the
full replay. One paragraph under two0.1-point offsets at origin213.6 produces
213.8 (`0x4355cccd`) with flattened-local translation, versus ordinary layout's
213.80002 (`0x4355ccce`). The original assertion fails on that difference.

Accepted region geometry now retains bounded parent ordinals and local offsets.
`projected_frames(origin)` accumulates the same parent-first f32 additions as
ordinary layout, validating the entire result before shell/content publication
or accepted-artifact replacement. Single-key lookup follows the same arithmetic.
Retained A keeps its original dimensions; pending placeholders, inline zero
frames and overflow refusal remain qualified. `frames()` still means immutable
origin-zero geometry, not an exactly translatable world-coordinate array.

The three production paths add coordinate metadata, not retained layout-engine
state. Estimated metadata is16B/frame and a temporary projected array32B/frame,
bounded by the unchanged4,096-node ceiling. Single-key lookup also uses a bounded
ancestor vector. These are representation costs, not allocation or speed results.
The64-offer and16MiB source ceilings remain unchanged.

All40 kernel region tests and24 Messages reuse tests pass, including six exact-bit
projection controls; strict two-package all-target Clippy and scoped formatting
pass. MAIN repeats the64 scoped tests and Clippy on the exact six integrated
source/test files. The owner preserves an initial stale shared-library compile
failure; byte-identical entry mtime refresh forced the actual candidate rebuild,
without changing code or assertions. The54-file validation freeze is
`exact2-messages-region-consumer/target/messages-region-consumer-prep/geometry-projection-v1/validation-freeze-v1/`,
manifestf89fa212, production patch4891179d. The original full-replay failure is
unchanged and was not rerun.

Apple's cached local frame wire and Linux's translated picture commands still
need origin-qualified mapping and rendering checks. Kernel geometry correctness
does not establish those adapters, full Messages worker publication, capacity
policy, native performance or120Hz.

### 8.112 Shipped no-op timer: native progress and setup refusal, 2026-09-18

One separate actual Linux build/run applies the shipped no-op-timer change to
the preserved738 source plus unchanged observation overlay and readers. The
ARM64 ELF is40abccdb;9addbd18 identifies its product record, not the binary.
This is not another command in the stopped three-attempt reader campaign or a
matched timing comparison.

The run reaches and captures the100-row endpoint, including real wheel/back/clamp
checks, then loads the full10,000-row resource. Snapshot4048 contains every
sequential key,2,342,860 body UTF8 bytes and the unchanged seven-argument/nine-field
schema. Batch32 is selected, but revision/changed remain0 because updating phases
never start. First full picture132 retains scroll16211.400390625; after320 total
setup batches, picture396 reaches50451.3984375 without a final-tail witness.

The unchanged20s setup allowance reserves1s before another offer. At19.021440572s,
37,115 spans/12,590 events/6,332,199B, that time cushion refuses another wheel
batch. Recorder capacities and the60s watchdog were not exhausted. The whole
run remains **FAILED / setup NOTESTABLISHED**: no full10k-tail admission, measured
phases, silent2s proof or final fence. The saved image is only the100-row checkpoint.

Independent stderr parsing exactly reproduces12,590 journal records and396
ordered picture/submit/ready/installed-ACK chains. All75 paused timers preserve
state apart from clock:68 clean→clean and7 already-dirty→dirty, with no clean→dirty
transition. The previous paused-anchor refusal does not recur. This supports the
observed no-op behavior, not a general timing or frame-rate gain.

The37-file runtime freeze is `target/messages-linux-vkms-noop-timer-runtime-v1/`,
manifest6c1e1528/reporte7d26c51. Recorded cleanup retires all7 owned PIDs/four groups
and clears the port; the native process receives owned SIGTERM and exits−15.
No retry or reader/capacity retuning follows. The observer lacks List identity,
index extent, feedback sequence and correction disposition, so an end-follow
correction being lost to acknowledged-picture clamping is only a source hypothesis.
A small existing presenter fixture is the next discriminator; the failed native
evidence is not relabeled.

### 8.113 Retained action qualification in Runner, 2026-09-18

A retained picture can name a live keyed row whose curried action argument has
changed. Generic dispatch correctly uses the new value; using it directly for
the old picture can therefore deliver the wrong target. The existing test target
reproduces that mismatch with one intended failure and six passing controls.
An earlier typed-record fixture setup failure remains preserved.

Runner now offers explicit capture, validation and bound dispatch for Press and
Swiperight. The opaque witness includes a fresh per-boot origin, full NodeKey,
handler/action identity and at most eight bounded scalars. Strings are checked
before copying:1,024B each,4,096B total. Arguments admit only literal or direct
slot/item/bound field projections; closed actions admit bounded literals,
parameters and scalar root stores. Ambient reads, payload events, calls, effects,
control flow and rich retained values refuse. Ordinary dispatch is unchanged.
Deletion/remount, foreign Runner, reload or changed arguments refuse before
action state, journal, effects or time change; unchanged bound IDs survive
body-only updates. Existing mounted-tree lookup cost remains explicit.

The three-path candidate passes nine integration and seven boundary tests plus
strict Runner all-target Clippy. MAIN integrates exact bytes and passes all24
now_screen tests and90 library tests, with Clippy/format/caps/boot checks. The
47-file freeze is `exact2-runner-retained-action-binding/target/retained-action-binding/freeze-v1/`,
manifestbbbde311/patche25015a7. Actual generated Messages handler compatibility
was checked in the baseline only, not by the new candidate API in an app.

Hosts still must qualify displayed geometry, live eligibility and binding before
their own clock advance, then use bound dispatch's second validation. No host
adapter or Messages worker activation is included; these are correctness
prerequisites, not native latency or120Hz results.

### 8.114 Stage native subtree publication before replacing retained A, 2026-09-18

The explicit Rust native-region producer now keeps selected A's native properties,
children, geometry and collection facts while replacement B is pending. It captures
one replaceable candidate, validates its publication identity, stages the complete
node diff, then replaces the selected mirror only after admission succeeds. Live B
cannot leak body/children/destroy updates into A; ordinary outside controls continue.
The existing opaque region boot and public ABI are unchanged. No Swift subtree
consumer or app entry is activated by this increment.

Capture admission prices borrowed properties, styles, topology and collection rows
before copying. Default ceilings are4,096 nodes/edges/rows,64 collections,8MiB logical
wire per selected/candidate packet and16MiB staged diff. These bounds do not measure
total heap, source/font/layout storage or RSS. Bounded snapshots and staging still
allocate and traverse; per-node compatibility frame lookup remains to be replaced
by the separately planned bulk projection.

Refused B must also revoke the client's previous current-state advertisement. An
actual public dispatch test sets a13,520-byte diff allowance: A fits, A+B does not.
Old production retains A correctly but returns no native-region state; the intended
assertion fails. The fix emits retained A with current:false before returning the
staging error. The identical test passes with unchanged A, no partial B operations,
outside typing and later complete-B recovery. It is a deterministic capacity refusal,
not an allocator/OOM reproduction. Earlier capture/compute/observe failures are
outside this narrow error-path correction.

The final owner round passes8 Host,3 library and38 collection tests, strict affected
all-target Clippy and scoped formatting. The library checks include72 exact ordinary
batch-byte combinations. Prior fixture syntax and Clippy-placement failures remain
preserved; no fourth candidate correction was needed. Source seal4e1e3123 and raw
execution are under `exact2-apple-native-publication/target/native-publication-candidate-*`.
Independent review closes the public-error issue at source level.

MAIN integrates the exact nine files. A shared-target audit found older kernel
artifacts could survive switching worktrees, so byte-identical kernel/Runner/Apple
entry refreshes force a current-source rebuild. The composed run passes40 kernel
region tests plus the49 scoped producer/collection tests, Clippy and formatting.
The initial49-pass run and a mistyped test-target refusal are preserved separately;
only the fresh composed run qualifies integration. Evidence is in
`target/native-publication-integration/`.

Native coordinate/pixel parity, retained-action host eligibility/clock routing,
full10k Messages capacity and native consumer activation remain open. These are
publication correctness results, not a responsiveness or120Hz measurement.

### 8.115 Keep collection end intent until its picture is acknowledged, 2026-09-19

A real Linux Presenter fixture now reproduces a lost follow-end correction:
acknowledge a20-row List at its end, then grow the same List to200 through its
ordinary action. Runner emits the new tail position, but feedback clamped against
the still-displayed A bounds can consume that correction and leave the reader at
the old offset. The headless control reaches the new end. This is an executed
CPU-path regression, not proof that the earlier full10k native setup failure had
this cause; that capture lacks the necessary List/correction identities.

The adapter now keeps one pending model position per collection cursor. Feedback
and the next submitted picture use it; A's input, hits and scroll bounds keep
using the acknowledged position. A successful matching B acknowledgement adopts
only a target that B actually painted, with current node generation and input
sequence. Newer real scrolling, changed geometry and runtime replacement defeat
old B. A newer model target on the same sequence survives B's acknowledgement.
The headless path retains its immediate-offset behavior.

Review also found that an identical-size resize advanced the input sequence and
discarded the future target. The same real test fails against the first candidate
with ten controls passing. Advancing only for changed dimensions makes all eleven
pass; Host resize validation/layout and post-commit processing still run. Changed
dimensions and newer wheel input remain explicit supersession controls.

The original baseline preserves one behavioral failure, two incorrect fixture
assumptions about absent corrections and three controls. The first candidate
passes eight cases but fails a reload fixture expecting20 instead of the correctly
carried200 rows. Its corrected assertion retains the fresh zero scroll position
and old-receipt refusal checks. These setup failures remain separate from the
same-size behavioral RED. Final owner checks and fresh MAIN integration each pass
40 display,14 collection and8 height tests, strict library Clippy and scoped fmt.
MAIN forces byte-identical kernel/Runner/Linux entry refreshes to avoid stale
cross-worktree artifacts. Evidence: `target/linux-count-follow-end-integration/`;
owner source/raw: `exact2-linux-count-follow-end/target/linux-count-follow-end-*`.

The pending picture carries scalar targets; no source/image history is added.
Painting with a pending target clones the live scroll map once, an explicit
allocation/traversal cost. No new native/VM run, full10k baseline, timing gain or
physical120Hz result is established by these tests.

### 8.116 Project native subtree coordinates once, 2026-09-19

The explicit Apple native-region producer now checks exact paint-order membership
and projects the complete bounded publication once before staging its node diff.
It uses ordinary f32 subtraction against current parent frames for native-local
coordinates. It no longer scans publication membership and reconstructs ancestor
coordinates separately for every node. Current receipt/candidate identity checks,
diff admission and immutable retained A remain unchanged.

The same ordinary/native public-batch regression passes on old and new code:
first complete A, origin-only movement, width B pending while A remains selected,
another origin change, complete B and invalid-viewport refusal. It compares exact
four-f32 frames and wire bytes for all protected descendants, overflow extents and
two real zero-frame inline runs. The213.6+0.1+0.1 fixture verifies nonassociative
f32 arithmetic; no epsilon or alternate arithmetic is introduced. This baseline
is a PASS, not a claimed semantic RED.

Owner and fresh MAIN checks each pass9 Host and3 library tests, strict Apple
all-target Clippy and scoped formatting; the library tests retain72 ordinary batch
byte combinations. Actual verbose kernel/Runner/Apple compilation and unchanged
entry-byte refreshes qualify both owner arms and MAIN. The owner's initial empty
dependency-file collection is preserved; corrected copies are explicitly
post-check evidence. Source/execution freeze: `exact2-apple-native-projection/target/`;
MAIN receipt: `target/native-projection-integration/`.

The projection adds one transient Vec bounded by the admitted4,096-node ceiling.
Other capture/header/diff scans remain; the complete selection is not claimed
linear or allocation-free. No Swift subtree consumer, app entry, public ABI,
native performance result or120Hz claim is added.

### 8.117 Preserve ordinary raw coordinates during Linux region painting, 2026-09-19

Correct kernel frames alone did not make retained region pixels equal ordinary
painting. The old region recorder constructed local geometry and translated it
later. Text glyph placement occurs before that later transform, and shape/hit
arithmetic also changes when translation is regrouped. A fresh-kernel CPU fixture
has equal projected frames but55,145/54,813 differing RGBA bytes at initial/scroll
checkpoints and50,578 when retained A moves while B's real worker is blocked.
The earlier stale-kernel baseline remains separate.

The painter now freezes immutable box/text/image operands and flat enter/leave
order. Replay projects once, resolves geometry using the ordinary f32 operation
order and validates source, finite geometry and prepared text queries before
backend emission. Ordinary and region paths share the same box emitter and feed
the same raw rectangle, text origin and parent transform to the backend. No
Raster, font, text-cache or kernel policy changes are involved. A's source and
image owners remain retained until its existing picture owners release them.

Tests written before this change forward actual raw backend arguments and feed
the same Paragraph to both placement paths. Old production fails all three tests;
the integer control already has equal pixels but differs in raw/physical inputs.
The candidate passes all five asserted scenes: integer initial/scroll, fractional
initial/scroll and retained A moved with B pending. All twelve backend calls,
complete400x600 CPU RGBA, hit/source/stamp geometry and same-Paragraph placement
inputs/pixels match exactly. The post-B diagnostic still compares new short-source
placement against a stale A oracle, printing60 glyph/12,745 byte differences;
it is unasserted and is not a new-B pixel-parity result.

Owner validation passes15 region tests including the three focused cases,9 paint
tests, strict all-target Clippy and formatting. The formatted source is composed
with the collection fix in§8.115 on MAIN:15 region,9 paint,40 display,14 collection
and8 height tests pass,86 distinct in total, plus Clippy/fmt/caps/boot. Entry-byte
refreshes force current kernel/Runner/Linux compilation. Owner freezee097fb1f is
under `exact2-linux-region-projection/target/linux-region-projection-validation/`;
MAIN receipt is `target/linux-region-paint-integration/`.

Default64 offers,4,096 nodes and49,152 expanded paint operations remain unchanged.
Replay preparation holds both projected and resolved O(window) arrays; this is
not an allocation-free or memory-reduction result. Evidence is macOS execution
of the Linux CPU painter at scale1, not Linux/VM/GPU presentation. Full interactive
Messages capacity/action routing and native performance remain open; no120Hz
claim follows from pixel parity.

### 8.118 Current Linux Messages completes the full fixed workload, 2026-09-19

One fresh CPU/VKMS run at6956be1 now reaches the real10,000-row tail and completes
idle/load/recovery, the separate command-free producer interval and the final
acknowledged-image fence. Setup takes4.233593s under the unchanged20s allowance:
the100-row endpoint is frame128/scroll16211.400391, the10,000-row endpoint is
frame133/337108.906250, then frame135 establishes the existing80px margin.
This is a current-source correctness result. It does not retroactively establish
the cause of the old failed native capture or provide a matched performance gain.

The ordinary ReusableMessagesStress workload stays full10,000/batch32, windowed,
unbounded data, cursorNone and eagerfalse at1024x768/scale1. The two complete
History9 snapshots have10,000 exact row keys and checked UTF8 body totals:
revision0/changed0/2,342,860B and revision16/changed32/2,344,844B. Each fixed2s
phase delivers six typed keys and six alternating actual40px wheels. All36
handlers and18 wheel movements join inside their conservative collector prefixes.
The idle final picture's later drain remains separate. Eight silent timers advance
revision0 to8 with zero input; eight loaded timers advance8 to16. Twenty installed
loaded pictures carry their own newer History and visible changed-suffix geometry,
not merely the revision of a newer live state at acknowledgement.

| Phase | Enclosed turns | Whole wall sum ms | Whole max ms | Above8.333333ms |
|---|---:|---:|---:|---:|
| idle | 157 | 156.030309 | 13.408172 | 12 |
| loaded | 234 | 240.453173 | 18.428273 | 16 |
| recovery | 53 | 148.148812 | 12.125376 | 13 |

These include cheap no-work turns and nested observer work; they are neither CPU
time nor a frame-rate distribution. Subtracting only the recorded union of observer,
KMS-wait and VNC intervals leaves maxima4.948351/10.373411/4.340807ms and0/5/0
misses. That residual is observed wall time, not a prediction of an uninstrumented
app. Native input to first covering ACK maxima remain23.474/30.431/22.784ms;
coalescing does not prove individual physical visibility. Loaded and silent timer
calls alone max3.803/3.873ms, a narrower scope than the whole turn.

Raw stderr exactly reconstructs5,001 events,1,358 closed turns,15,147 spans and196
complete paint/submit/ready/ACK chains. Final frame196/input1403 is acknowledged
at sequence4945, with2,359,296 captured RGB bytes exactly matching its painted
hash. A later clean, unchanged timer4999 needs no new pixels; its timing turn
closes before the final5001 fence. The last observer flush remains censored.
The app is deliberately SIGTERMed after observations, native exit-15; driver and
outer exit0. Six recorded PIDs/four groups are absent and port5941 is free.

The fresh release ELF is d4d72b5b. Actual generated entry/plan match the intended
workload; compat930f8a9b differs from the historical expectation because the
data-crate digest includes the changed tests/reuse.rs. Other data inputs match.
The first launcher refused an obsolete738 source-pin literal before native Popen;
its failed archive remains intact. The corrected binding changes only that pin,
with old-config behavioral refusal and nine passing pure controls. There is no
rebuild, cap change, retry of a failed native cell or observer-policy change.

Build evidence is `target/messages-linux-vkms-current-6956-build-v1/`; the final
44-file/48,760,945B runtime freeze is `target/messages-linux-vkms-current-6956-runtime-v2/`,
manifest5d6154a0/report140caf75. The lead independently reconstructs raw counts,
phase costs, nested exclusions, inputs, histories and final RGB; its initial
overstrict assertion forbidding any trailing timer is preserved and corrected
to the existing clean-state/closed-turn rule. The observer's nonquiet=true label
remains. This is one virtual-display cell with no measured resize and no physical
120Hz proof. The ordinary path is still synchronous; full-viewport region capacity,
retained row actions and native worker-backed Messages activation remain open.

### 8.119 Swift native-publication consumer remains deferred, 2026-09-19

The proposed Swift consumer keeps one selected publication and incoming weak
view metadata, validates native authority before applying protected operations,
and qualifies deferred callbacks by publication token and view identity. It is
not integrated. The final allowed standalone test round still fails four
nested-pending/reset assertions; source review alone does not establish those
behaviors.

A real Rust producer exports44 returned batches covering complete A, pending B,
outside progress, diff refusal/current:false and recovery. The first fixture's
fixed diff allowance also refused its recovery; a preserved test-only correction
derives215,556 bytes from the actual small endpoint prices. Small A+B costs215,304
and large A+B1,579,544, so the public refusal and recovery remain discriminated.
The corrected producer test and ten existing producer controls pass separately.

After a generated-runner actor-isolation compile failure, the final runner-only
correction wraps both identical standalone test bodies in a main-thread check
and MainActor.assumeIsolated. Both arms compile and run eight methods with677
assertions over the same exported batches. Baseline has14 failures; candidate
has4. Direct pending/refusal/recovery and malformed/error-envelope methods pass
228 and222 candidate assertions respectively. Five existing controls pass27.
The nested/reset method executes200 assertions but fails four, including object
replacement and deferred-authority checks. Synthetic UInt32 operation IDs versus
the presenter's Int decoding are a source-traced fixture concern, not a proven
cause or grounds to ignore the failure.

The target stops at round three: no fourth correction, integration or subsequent
full-module check. The fixture uses actual AppKit/production source with a nil
Presenter session and dynamic-lookup linking; it does not execute a real Session
Runtime, worker-backed text realization or an app entry. No XCTest, full native
build, input-latency or performance result follows. Final evidence is
`exact2-apple-native-consumer/target/native-consumer-checks-v3/`, manifest7d430e7e,
36files/7,782,499B, report208532e3. All eight recorded PIDs/groups are absent;
earlier fixture and compilation failures remain preserved. Kernel capacity and
Linux retained-action work continue independently.

### 8.120 Separate measurement facts from final text owners, 2026-09-19

The optional SplitFacts kernel profile now completes the unchanged full Messages
viewport without retaining a native artifact for every intermediate measurement.
The default PinnedOffers profile still admits64 retained exact offers. This is
an explicit kernel policy, not a host entry change or native Messages activation.

SplitFacts stores at most768 exact scalar measurement facts and192 sources/final
paint owners per generation. Measurement completion validates its private request
then drops the supplied opaque payload. Once geometry is complete, each final
paint tuple receives a fresh request and must return bit-identical width, height
and optional baseline before any new publication becomes current. Source/catalog
identity and both typed offer axes remain exact; stale identity is checked first.
Fact reuse imports scalar information without carrying an old artifact wrapper.
The common layout, shell and parent-first f32 projection remain shared with the
default path; final ownership does not require repeating that completed layout.

Two payload-free reservation tokens limit live split generations, including
external request, artifact and publication aliases. The weak slots survive
unregister/reset on the same Kernel. While both are occupied, a third candidate
allocates no facts or request: retained A and outside shell progress remain,
current is false, and later explicit computation can resume after release. There
is no new queue, worker, poll or wake source. A new Kernel has its own domain;
host-extracted payload aliases and native memory admission remain host obligations.

Tests using only the old API first reproduce two failures: an actual resolved
measurement keeps its payload alive, and the real10,000/batch32 Contract stops
bootstrap at exactly64 deliveries with an offer-budget refusal. With only the
registration statement changed, both tests pass. Bootstrap completes288
measurements plus64 final requests; the first tail refinement completes594 plus
132, preserving all33 mounted rows and132 paragraphs. This is more total delivery
work than retaining every measured artifact, and is not a measured speedup.
Full-source and ordinary-geometry comparisons, batch32 updates, width changes,
selected row heights and live focus/interaction pins pass without an epsilon,
smaller viewport or altered Contract.

The actual scalar record is36 bytes, with768 slots reserved exactly;192 source
pointer slots are separately bounded. Existing4,096-node and16MiB captured UTF8
limits per generation remain. These counts exclude opaque native shapes/layouts,
font caches, temporary layout storage and allocator/RSS costs. A native adapter
must separately admit aggregate payload ownership and arrange progress when a
display acknowledgement releases a parked generation. Count fit alone does not
authorize192 native indexes under the old per-index byte ceiling.

Owner validation passes84 canonical tests plus two diagnostic baseline controls
(86 distinct,88 executions), strict Clippy and scoped formatting. The formatter's
brace changes caused the initial token-equality proof to fail; that false result
remains, followed by explicit hunk qualification and exact inverse verification.
Final owner freeze is `exact2-region-facts-final-owners/target/region-facts-final-owners-validation/freeze-v1/`,
manifestc5fb5274/source29319de8. Fresh MAIN compilation passes57 kernel-region,
26 Messages, one scalar-storage,15 Linux-region, nine Apple-region and86 Apple
library tests,194 total, plus strict all-target Clippy for the four affected
packages, formatting, caps and boot. The Apple library command selected the full
library rather than only three batch tests; all86 actual results are retained.
MAIN evidence is `target/split-facts-integration/`; all46 recorded PIDs/seven
groups retire. A lead preflight JSON-schema mismatch stopped before application
or compilation and remains recorded separately. Native bytes, retained actions,
worker-backed app activation and physical120Hz remain unproved by this increment.

### 8.121 Retained Linux row actions and owned return motion, 2026-09-19

The CPU region picture now captures bounded Press and right-swipe bindings, so
an acknowledged A can keep its own row actions while B is prepared. Live state
may deny a captured target; it cannot redirect that action to B or a recycled
row. Current curry, ancestry, visibility and eligibility are checked before
advancing input time, and Runner validates again before dispatch. Unsupported
bindings remain barriers. Capture admits the whole picture or refuses, with256
event entries and65,536 captured UTF8 bytes per picture; A+B can retain twice
those amounts, plus one4,096-byte temporary capture. Contacts retain weak
identities rather than pictures, paragraphs or resource values.

A qualified single-row Translate hold moves retained pixels and hits together.
Cancellation can paint its ordinary nonzero return even when its action is no
longer valid. Motion preserves the process-unique hold serial as Returning until
completion or replacement, without granting it hold-mutation authority. A
successful distinct-picture ACK immediately retires the old contact and pin and
snaps only its still-owned Translate motion. An identical-looking newer return
or hold is untouched. Wrong-origin and duplicate acknowledgements confer no
retirement authority. This is not general retained transform support.

The original candidate passed10/11 tests but refused retained UP. Two added
tests then reproduced ACK-only retirement and cancelled-return paint failures.
The correction passes all14 retained tests, including actual moved RGBA/row-hit
agreement with ordinary rendering and a foreign replacement-curve control.
Full Motion84 and Linux27 distinct interaction tests pass on the owner checkout.
Mac non-test Clippy initially rejected five ACK-only helpers as unused; matching
their cfg to the existing display caller closes that final correction. Formatting
changes no bytes. The optional identity remains16 bytes on this compiler, with
Slot160 and HoldToken24; this is not a general memory or overhead measurement.

Fresh MAIN compilation passes135 distinct tests:84 Motion,24 Linux swipe,
18 Linux region and nine Runner binding tests. Strict all-target Clippy for the
three affected packages, all15-file formatting, caps and boot pass. Evidence is
`target/retained-actions-integration/`;40 recorded PIDs/eight groups retire.
Owner source and execution evidence remains in
`exact2-linux-retained-actions/target/retained-actions-validation/`.
These are Mac-hosted CPU/model tests with a real worker and small row fixture,
not an actual Linux display run, full10,000-message worker activation, latency
improvement or physical120Hz proof. Native SplitFacts capacity and ACK progress
remain a separate unintegrated candidate.

### 8.122 Native SplitFacts full-viewport adapter remains deferred, 2026-09-19

The proposed Linux adapter adds explicit pre-first-paint SplitFacts activation,
known retained native-capacity admission and progress when a matching display
acknowledgement releases an old generation. It remains private and unintegrated.
The final allowed test round compiles and runs five tests: one passes and four
fail at the full-workload admission assertion, with seven mounted tail rows where
33 are required. The tests do not reach their132-final-owner, ordinary-geometry,
pixel-refusal or parked-generation ACK assertions. This is neither a smaller
accepted workload nor proof that the native capacity policy is insufficient;
the cause of the fixture's row-count mismatch is unproved.

Fifteen earlier native-budget, payload-lifetime and measurement-purpose tests
pass. An old-API baseline separately reproduces the unwanted measurement index
construction. Those results do not establish the missing full-viewport behavior.
The first candidate attempt had test compilation errors; the next full-workload
attempt was stopped by a forbidden unsafe poll in the fixture. The final
fixture-only correction uses the existing safe test wait and real completion
drain. It does not prove OS readiness or an actual Linux display path.

Execution stops after correction round three: no fourth fix, reduced row count,
cap increase, native activation or integration. Final command exit101 has no
watchdog action; all ten recorded PIDs and its process group are absent. Evidence
is `exact2-linux-split-facts-native/target/split-facts-native-validation/cpu03/`;
the source-v4 seal and both earlier attempts remain preserved. The kernel profile
in§8.120 and verified retained gestures in§8.121 remain separate verified changes
on the feature branch. No native-memory, latency or120Hz result follows here.

A subsequent source-only postmortem finds that33/132 was copied from the kernel
Monospace test immediately after one feedback, before further settlement. The
native fixture settles before scrolling and after every wheel. Neither a native
font count nor a settled-tail invariant established33. This invalidates that
test premise without proving the native adapter correct or explaining the seven
rows. An unapplied ordinary-renderer diagnostic is prepared separately; no
exception to the three-round execution limit has been granted.

### 8.123 Avoid a temporary array for one native text run, 2026-09-19

The Apple callback adapter now passes one local CRun through a borrowed singleton
slice when a measurement contains exactly one run. Empty and multi-run requests
keep the original vector path. Field conversion, synchronous callback lifetime,
raw result validation, metric memo admission and the C ABI are unchanged. No new
retained state, text copy, cache entry or pointer escape is added.

Identical tests on the old adapter observe one40-byte allocation/free, then fail
the intended zero-allocation assertion after all request/metric/lifetime checks
pass. The other two storage tests pass. The candidate passes all16 measurement
tests and observes zero request-storage allocations for one run. Twelve request
variants compare exact fields, both offer axes, Unicode bytes and source pointers;
empty/multiple runs and existing invalid-result, catalog, frame and baseline
controls remain. Strict Apple all-target Clippy and scoped formatting pass on the
same-base private checkout; the exact two tested source snapshots are integrated.
No duplicate integration run is claimed. Caps passes after staging.

Evidence is `exact2-apple-measure-request/target/measure-request-source-v1/` and
`target/measure-request-execution-v1/`; the MAIN source receipt is
`target/measure-request-integration/`. This proves a removed allocation with no
additional retained payload, not native callback frequency, whole-frame savings
or120Hz. Swift/CoreText, full native builds and runtime timing were not rerun.

### 8.124 Reuse one unchanged CPU clip mask, 2026-09-19

The Linux CPU rasterizer retains one immutable mask for the first parentless
clip in a frame. Reuse requires exact shape, transform, device dimensions and
scale bits. A changed key drops the old backing before allocating its replacement.
An invalid or oversized first clip uses the existing rendering path and consumes
the slot; later root clips, images and nested masks do not replace it. Nested
operations copy before mutation, and a frame without clips has no active mask.

Retained mask payload is capped at 1 MiB, or 786,432 bytes at 1024 by 768.
Scalar keys, reference-counting and allocator overhead are additional, as are
existing temporary masks and layers. No source, node or picture is retained.
This trades bounded retention for avoided repeated mask construction; it does
not impose a total renderer-memory or RSS bound.

The same 21 tests run against both arms. The old implementation passes all 14
existing controls and fails the seven new allocation/ownership assertions.
In the scrolling-text probe, both complete pixel comparisons pass before the
old path reports two mask allocations instead of one. The candidate passes all
21 tests, including independent full-RGBA, nested/pop, fractional/rotated,
changed-key, invalid/oversized, no-clip and last-owner controls. Strict Linux
all-target Clippy and scoped formatting pass without a correction or retry.
The exact tested two-file snapshot is integrated; the intervening MAIN change
only affected Apple measurement and documentation, so no duplicate CPU run is
claimed. Caps and boot pass after integration.

Source and raw checks are in
`exact2-linux-clip-mask/target/clip-mask-validation/`, source manifest
`a9b85387`, with the integration receipt in `target/clip-mask-integration/`.
All recorded check processes retire. These are Mac-hosted CPU checks. Saved
Linux Messages frames establish repeated clip inputs but not their cost; no
fresh application timing, actual Linux comparison or 120Hz gain is claimed.

### 8.125 Fresh Linux clip-mask comparison: no demonstrated timing gain, 2026-09-19

One fresh before→after pair at `f6eded3` completes the full ordinary Messages
workload: 10,000 materialized rows, batch32, three fixed two-second input phases
and a separate two-second command-free producer interval. The two freshly built
aarch64 Linux products differ only in `raster.rs`; generated entry, plan,
compatibility inputs and fonts are byte-identical. Both use the existing VKMS
1024×768, scale1 CPU recipe and DejaVu Sans, with unchanged time, address-space,
disk and observer bounds. This is an instrumented, fixed-viewport VM comparison,
not continuous resizing or physical presentation measurement.

Both cells pass: 36 scored inputs each, including18 actual signed40-unit wheels;
eight advancing timers in each fixed silent prefix; updated painted suffixes
inside the loaded prefix; and a complete current final-picture fence. Full
10,000-row History values match between arms at revisions0 and17, including all
nine fields. Setup-margin and final RGB buffers are also byte-identical between
arms. These selected-state comparisons do not replace the independent CPU pixel
controls in§8.124 or prove every intermediate picture equal.

| Phase | Before FrameBuild total/count | After FrameBuild total/count |
| --- | ---: | ---: |
| Idle | 131.482ms /12 | 131.760ms /12 |
| Loaded | 186.431ms /20 | 186.641ms /19 |
| Recovery | 112.324ms /12 | 119.042ms /13 |

The table uses turns wholly enclosed by native timestamps at the fixed collector
prefix boundaries. Their observed whole-turn totals are512.218→515.847ms,
with39→41 turns exceeding8.333ms. Loaded whole-turn maxima
remain20.998→20.351ms. Loaded advancing timers differ8→7, painted entry revisions
are9 versus8, and recovery builds differ12→13. After's boundary-crossing loaded
turn1540 is separately retained:10.083ms whole /7.644ms FrameBuild. Including it
gives the broader collected-prefix totals525.930ms whole /445.088ms FrameBuild;
the strictly enclosed FrameBuild total is437.444ms versus before430.237ms.
Before's last loaded ACK drains beyond the scored prefix and remains excluded.
Counts and progress are not
normalized after the fact. No whole-workload timing benefit is established;
neither these nested FrameBuild spans nor whole turns isolate raster CPU cost.
The allocation/ownership proof remains valid, but no repeated gain or120Hz claim
follows from this pair.

Setup history remains separate: v1 could not open a helper outside the VM share;
v2 compiled successfully but its capture rejected an expected compatibility digest
computed with string ordering instead of Rust path-component ordering. The final
third setup uses the source-derived component order; both builds and full captures
pass. No application source was changed to repair either setup failure. Both
runtime coordinators exit0; native SIGTERM is deliberate after observations, and
all recorded Mac/guest processes and groups retire before the next cell. No
runtime retry or additional cell ran.

Evidence is `exact2-linux-clip-mask/target/messages-clip-mask-{before,after}-runtime-v3/`:
copied raw manifests `3b0d521c` and `8f9aea2e` cover46 files /50,956,273B, verified
by the lead. Build captures are `9cd6ac05` / `1258b732`; source preparation
`14ba6f1b` and runtime binding `be02145c` remain sealed. Independent lead arithmetic
is `target/clip-mask-integration/first-pair-selected-raw.json`. No native worker
activation, general memory bound, quiet-observer estimate or physical120 proof
is added.

### 8.126 Linux Messages frame split: painting remains the next locus, 2026-09-19

One fresh optimized diagnostic build and one full10,000/32 runtime cell at
`69676b8` pass the unchanged ordinary Messages recipe. The existing four observer
overlays gain only result-aware brackets around frame-entry collection refinement
and each existing paint invocation, including the fallback. These use existing
Refine/Paint phase IDs; the reader algorithms, workload, source semantics and
limits stay unchanged. This is a cost discriminator, not an optimization or an
old-product/new-product timing comparison.

Independent raw reconstruction joins one successful Refine and Paint child to
each of52 wholly enclosed FrameBuild parents:12 idle,20 loaded,12 recovery and
eight in the separate command-free interval. Parent/attempt/input identities and
ordering match. Observer intervals are unioned before subtraction; scene setup,
witness construction and other frame work remain an unclassified remainder.

| Cohort | Paint median / maximum | Tick median / maximum |
| --- | ---: | ---: |
| Idle | 2.584 /3.340ms | 0.149 /0.184ms |
| Loaded | 2.885 /6.365ms | 2.298 /2.739ms |
| Recovery | 2.817 /3.326ms | 0.111 /0.143ms |
| Command-free producer interval | 5.268 /7.989ms | 2.830 /3.840ms |

Frame-entry Refine medians are below0.0001ms, with a maximum0.0174ms;
unclassified frame remainder medians are0.037–0.042ms. This does not measure
refinement already performed inside commits or Tick. Painting explains nearly
all of the frame residual in this sample, but includes traversal, text lookup,
miss work, Raster and frame completion. No glyph, shaping or Raster-only cause
is established. Tick and Paint distributions are separate; their medians must
not be added as a measured end-to-end latency.

Recorded observer work remains substantial: its per-frame median is6.746ms in
load and7.229ms in the silent interval. Removing recorded intervals cannot undo
observer scheduling or cache effects. Whole-turn maxima remain17.502ms loaded
and21.419ms silent, with19 and8 budget misses respectively. This is neither
observer-free timing nor CPU or physical120 evidence.

All36 scheduled inputs complete inside their fixed prefixes, including18 actual
signed40-unit wheels. The silent prefix has eight advancing timers and no late
drain credit. Updated painted suffixes and the final current clean frame195 pass;
final History has10,000 rows,32 changed, revision16. The raw journal has5236
events and16,719 timing rows, maximum65 per turn, without recorder overflow.
Native SIGTERM follows completed observations; the runtime coordinator exits0,
all six recorded Mac/guest PIDs and four groups retire, and port5941 is free.
No retry or second cell ran.

The diagnostic remains outside production. Preparation `80257e2f`, compiled
capture `d46a2668` and runtime binding `3370f2c7` identify the exact inputs.
Raw manifest `affbacff` covers23 files /24,819,126B, verified by the lead, under
`exact2-linux-messages-frame-split/target/messages-frame-split-runtime-v1/`.
Independent reconstruction is `target/messages-linux-frame-split-lead/selected-raw.json`
(`4d17c3f5`). These results select investigation within painting; they do not
justify removing collection feedback, activating a worker, or claiming a gain.

### 8.127 Reuse cached glyph placement when building Linux ink bounds, 2026-09-19

Ink-index construction previously rasterized all four X phases uncached even
when ordinary drawing had already cached the exact glyph image. It now reads
only the placement scalars from an existing full `CacheKey` entry. Cached
`None` remains no ink; an absent entry takes the original uncached path. No
extra phase is inserted, image cloned, owner retained or cache limit changed.
The four-phase union, numeric bounds, query order and index budget stay intact.
Cold misses add a map lookup; the opportunity depends on existing cache entries.

The same seven tests run against the original formula and the candidate. The
baseline has four controls pass and three intended call-count failures: ordinary
rendered A followed by different-source B makes40 uncached calls where28 are
needed; a partially warm cache makes60 instead of45; cached no-ink glyphs make
four instead of zero. These are fixture work counts, not a measured Messages
cache-hit rate or timing gain. Assertions after each baseline failure did not run.

All seven candidate tests pass, including exact index bounds, query selections,
paint order and RGBA comparisons against the copied original formula and existing
full-glyph renderer. Controls cover cold/partial/full caches, fractional geometry
and scale, full font keys, cached no ink and zero-size images, catalog replacement,
accepted old owners, index refusal and warm repaint. The existing real Painter
failed-frame lease test separately checks accepted-owner preservation.

Final validation is28 ink tests plus that one Painter test:29 unique passes,
zero ignored, strict `exact-linux --all-targets` Clippy and scoped formatting.
The initial Clippy failure was test-helper ordering; moving the unchanged helper
and formatting both files passed the same checks. Original failures and the
exact relocation inverse remain recorded. Tests ran on macOS against the Linux
crate, with private Motion/kernel/Runner/Linux compilation verified; they are
not actual Linux workload or physical120 evidence.

The two-file donor is
`exact2-linux-ink-cached-placement/target/ink-cached-placement-validation/`:
final source card `74d6de26`, patch `e5b40c3e`, original source card `44a999ad`.
No native comparison has yet run for this shortcut. Section8.126's single
diagnostic remains historical context, not a matched baseline or attribution
of its Paint time to this function.

### 8.128 First actual Linux cached-placement pair: lower Paint, mixed whole work, 2026-09-19

One fresh A→B pair uses the existing frame-split control and one new optimized
candidate containing exactly the two tested ink files. The five observer overlays,
readers, fonts, generated Reusable entry/plan/compat and full10,000/32 recipe match.
Both cells pass once; the earlier diagnostic cell is not substituted for A.
This pair is fixed-viewport VKMS, not continuous resizing or physical120 evidence.

Independent reconstruction matches stderr to all5223/4774 events and closes all
192/199 four-field picture→submit→ACK chains. Strict phase prefixes contain one
Refine and Paint child per FrameBuild, with matching owner/input/attempt and no
boundary-straddling turn in these samples. Paint elapsed spans are:

| Cohort | A→B Paint count | A→B median | A→B maximum |
| --- | ---: | ---: | ---: |
| Idle | 12→12 | 2.831→2.783ms | 3.409→3.297ms |
| Loaded | 19→20 | 3.098→2.673ms | 6.359→4.776ms |
| Recovery | 12→12 | 2.702→2.654ms | 3.310→3.171ms |
| Separate command-free interval | 8→8 | 6.259→4.287ms | 7.282→5.117ms |

Loaded producer progress is8→16 in both cells, but input/paint interleavings and
picture counts differ. Both silent prefixes advance0→8 without commands or drain
credit. Their Paint totals fall47.240→34.868ms while whole-turn totals rise
140.579→145.049ms: recorded observer union rises60.637→71.741ms and Tick total
rises22.077→26.971ms. All eight producer turns still exceed8.333ms in each cell;
silent whole maxima are20.376→19.618ms.

Loaded Tick medians also rise2.453→3.216ms, and whole maxima are18.000→18.169ms.
Across the three input phases, FrameBuild totals are439.502→429.924ms and whole
totals521.593→516.258ms, with308 versus205 turns and40 versus39 misses. These
natural samples do not establish an overall latency improvement. Lower Paint
time is a first-pair observation, not isolated Swash CPU attribution or repeated
benefit. Subtracting recorded observer intervals does not reproduce an unobserved
execution or undo scheduling/cache effects.

All36 offered inputs per cell complete in-window, including18 actual signed40-unit
wheels. Initial and final full History9 values match all10,000 records at revisions
0 and16, including UTF-8 body-byte sums; whole snapshots differ in clock/state
metadata. All four saved RGB buffers match across arms at2,359,296B each, with
matching final boxes/scroll and the same offer schedule. Final frames192/199 are
current and clean through their closing timing turn.

B build exits0 in41.302s; capture `f3f8e456` proves the fresh private compilation
and unchanged generated inputs. Runtime coordinators exit0; native SIGTERM is
deliberate after observations. All12 recorded runtime PIDs/eight groups retire
and port5941 is free. No retry, extra cell or rebuild ran. Runtime binding is
`451ac84c`; copied raw manifests `8be64926` / `1d3a208d` cover46 files /48,762,292B.
Lead reconstruction and full-state/pixel comparisons are under
`target/ink-placement-integration/` (`82d8071a`, `7119a34d`, `ec15f6f8`).

### 8.129 Reversed Linux cached-placement pair: command-free Paint benefit repeats, 2026-09-19

Two fresh cells run B→A with the same binaries, observers, full10,000/32 workload,
fixed viewport and limits as §8.128. Both pass once, with no rebuild, retry or
third cell. Tables retain semantic A→B order. Independent stderr reconstruction
matches6251/5668 events and closes all199/195 picture→submit→ACK chains.

| Cohort | A→B Paint count | A→B median | A→B maximum |
| --- | ---: | ---: | ---: |
| Idle | 12→12 | 2.672→2.546ms | 3.181→2.997ms |
| Loaded | 20→19 | 2.577→2.849ms | 7.029→3.970ms |
| Recovery | 12→12 | 2.553→2.504ms | 2.938→2.623ms |
| Separate command-free interval | 8→8 | 5.890→3.905ms | 7.587→4.771ms |

The command-free Paint reduction repeats in both run orders. Loaded per-call
medians change direction; this is not a consistent loaded latency gain. Loaded
progress again advances8→16 in both cells with different paint interleavings.
Here Paint totals fall62.147→54.697ms, but observer union rises115.093→132.580ms
and FrameBuild totals rise177.978→188.025ms. Loaded whole totals rise218.874→
231.532ms; maxima fall20.166→15.330ms while misses rise15→19. Counts and tails
remain separate from aggregate work.

Silent prefixes each advance0→8 without commands or drain credit. Paint totals
fall48.391→31.927ms, FrameBuild109.940→101.077ms and whole141.670→135.105ms,
but all eight producer turns still exceed8.333ms. Whole maxima are21.804→18.127ms.
Loaded Tick medians rise2.128→2.338ms; silent Tick medians rise2.489→2.869ms,
consistent in direction with the first pair. Tick includes settlement and native
commit work, not just the data query. The next diagnostic separates those stages
before choosing an optimization; no query-validation shortcut is justified.

Across the three input phases, FrameBuild totals are410.947→400.284ms and whole
totals488.840→479.791ms, with702 versus462 turns and39 misses each. Observer
union totals284.355→286.603ms remain material. These two orders establish the
observed command-free Paint result, not isolated rasterizer CPU savings, overall
responsiveness, observer-free costs or physical120Hz.

All72 semantic handlers are inside their fixed prefixes, with36 actual signed
40-unit wheels. Initial/final full History9 values match all10,000 rows at0/16;
all four2,359,296B RGB buffers, final boxes and scroll match across arms and the
first pair. Whole snapshots still differ in clock/state metadata. Final frames
199/195 close cleanly through6251/5668. All12 recorded PIDs/eight groups retire,
port5941 is free, and native SIGTERM is intentional after acceptance.

Reverse binding `e6acb355` preserves the first archive. Copied raw manifests
`87b12651` / `0f8e1919` verify46 files /52,316,027B. Independent lead reconstructions
`f28d8a4f` / `c8cbf470` and cohort proof `8265a352` are under
`target/ink-placement-integration/`. No new source or policy change accompanies
this comparison.

### 8.130 Linux timer split: native commit outweighs Runner advancement, 2026-09-19

One optimized diagnostic build and one fresh functional cell preserve the
§8.129 candidate and add only three existing observer scopes: RunnerAdvance30
around `advance_timed`, HostCommit20 around `commit_effects`, and AfterCommit14
around the presenter's `finish_commit`. Error forwarding, paint decisions,
motion-only ticks, workload, caps and driver remain unchanged. This adds three
records/six clock reads per normal advancement; it is not a performance A/B.

The eighteen pure reader controls pass once. Old B's31 timer witnesses correctly
produce unavailable/null split costs. The fresh native cell passes the full
10,000/32 workload; independent reconstruction matches all5312 stderr events,
1689 turns/17269 spans and191 closed picture chains. Each scored timer has one
ordered30/20/14 child set with matching input/attempt, successful results and
nonnegative remainder. The loaded split is:

| Stage | Eight loaded calls, median | Loaded total | Seven strict silent calls, median |
| --- | ---: | ---: | ---: |
| Whole Tick | 2.429ms | 17.995ms | 3.449ms |
| RunnerAdvance | 0.663ms | 4.992ms | 1.176ms |
| HostCommit | 1.693ms | 12.420ms | 2.190ms |
| AfterCommit | 0.071ms | 0.578ms | 0.133ms |
| Unclassified remainder | 0.000646ms | 0.005792ms | 0.001500ms |

HostCommit dominates this timer cohort, shifting the next investigation away
from query/key work toward native commit/layout. RunnerAdvance still includes
query settlement, shape validation, instance work and kernel application; it is
not a query-only cost. HostCommit includes receipt processing, navigation,
Motion, layout and presentation values. These spans do not identify shaping,
layout-engine CPU or a safely removable layout. Inclusive parents are not added
to their children, and the remainder includes observer bookkeeping.

The silent fixed prefix proves eight advances0→8 and zero commands. Timer4638 is
inside that prefix, but its owning turn1323 closes at4641 after the cutoff;
its complete timing is excluded. Seven enclosed silent producer turns still
exceed8.333ms, with20.438ms whole maximum and5.138ms Tick maximum. Loaded progress
is8→16; idle/loaded/recovery whole maxima are14.291/15.013/12.052ms with12/14/12
misses. The primary recorder reports no failure, with at most61 rows per turn.
Final observer flush remains censored. This diagnostic proves no speedup or120Hz.

All36 scheduled semantic handlers are in-window, including18 actual signed
40-unit wheels. Full initial/final History9 at0/16 and all four saved RGB buffers
match prior B; this is functional equivalence, not a timing control. Final
frame191 closes cleanly through5312. All six runtime PIDs/four groups retire and
port5941 is free; native SIGTERM follows successful observation deliberately.

The41.423s build captures21 files /12,849,610B (`bad36c1e`, ELF `e88d221d`), with
fresh private compilation and unchanged generated entry/plan/compat, fonts and
external inputs. Exactly two source contents differ from B; an added
`baselineB_SHA256` provenance field on the unchanged observer is kept separately
from file identity. Runtime binding `630268db` and copied raw manifest `0dcac37d`
cover23 files /24,969,118B. Independent lead timer/frame reconstructions
`55002c8c` / `2845db37` live under `target/ink-placement-integration/`.

### 8.131 Linux advancing timers: layout accounts for nearly all native commit, 2026-09-19

One further diagnostic build and one fresh full-workload cell wrap the existing
`layout_motion`/`layout` expression in Geometry17, directly inside HostCommit20.
Only that source content differs from the prior Timer capture. It adds one
record/two clock reads per commit, preserving the expression, returned result,
workload, caps and driver. The eighteen prior reader controls plus sixteen new
geometry controls pass once; no production instrumentation is integrated.

The cell passes full10,000/32 at fixed1024×768 with the ordinary reusable
producer and default Region64. Independent reconstruction matches5064 journal
events,1429 turns/16377 spans and194 closed picture chains. All32 strictly
enclosed timers, eight per cohort, have the exact ordered Timer/Runner/Host/After
join and one direct Geometry child. Layout reports CHANGED for all loaded and
silent timers, UNCHANGED for idle and recovery; no error or unavailable result
is observed in those cohorts.

| Stage | Eight loaded calls, median | Loaded total | Eight silent calls, median | Silent total |
| --- | ---: | ---: | ---: | ---: |
| Whole Tick | 2.622ms | 20.942ms | 3.249ms | 24.276ms |
| RunnerAdvance | 0.694ms | 6.251ms | 0.974ms | 7.510ms |
| HostCommit | 1.765ms | 13.991ms | 2.044ms | 15.876ms |
| Geometry, inside HostCommit | 1.741ms | 13.755ms | 2.004ms | 15.561ms |
| HostCommit excluding Geometry | 0.028ms | 0.236ms | 0.040ms | 0.315ms |
| AfterCommit | 0.082ms | 0.693ms | 0.116ms | 0.883ms |

Geometry accounts for98.31%/98.01% of summed loaded/silent HostCommit intervals.
This narrows the investigation to the existing layout path; it does not isolate
text measurement, shaping or layout-engine CPU, or prove that layout can safely
be skipped. Idle/recovery Geometry medians are both about0.007ms. Inclusive
parents are not added to children, independent medians are not additive, and
the remainder includes observer overhead. All50 raw timer witnesses join,
including18 outside the scored cohorts;313 other Geometry scopes are not
reassigned to timer costs. This is one instrumented cell, not a performance A/B.

Whole-turn maxima remain13.705/18.637/11.728ms for idle/loaded/recovery, with
12/15/11 misses above8.333333ms. All eight silent producer turns miss, with
20.419ms maximum. Loaded/silent FrameBuild observer unions total121.779/63.705ms;
their costs remain explicit. All32 scored Tick spans and52 scored Paint spans
fit individually, which does not make their enclosing turns fit. Startup is
separate at42.655ms; final recorder flush remains censored. No overall latency,
CPU or physical120Hz improvement is established.

All36 scheduled semantic handlers and18 actual signed40-unit wheels fall inside
the fixed prefixes. Silence has eight advances0→8 and no commands; loaded
progress is8→16. Unlike §8.130, this cell excludes no boundary timer and credits
no drain. Full History9 at0/16, including all10,000 rows and body-byte totals,
and all four saved RGB buffers equal prior Timer states. This establishes
selected functional equality, not a historical timing control. Finalframe194
is current and clean through5064. The six recorded runtime PIDs/four groups
retire, port5941 is free, and native SIGTERM deliberately follows acceptance.

The41.090s build captures21 files /12,849,957B (`eaeaa386`, ELF `9118c958`), with
fresh private compilation and unchanged generated entry/plan/compat and other
inputs. Binding `0cbccc1d` retains the existing60s native/20s setup/220s outer
guards, quota and2.5GiB address-space cap. Raw `a991c30f` covers23 files
/24,578,563B; final report `aca8b592` and manifest `92305c29` seal56 files
/49,659,444B. Lead timer/frame/cohort reconstructions `460d8cd2` / `354e6d0e` /
`32a7880c` independently match all32 component arrays, history, pixels and input
joins. A console-only formatting SyntaxError is preserved separately; it caused
no reader, runtime or source retry.

### 8.132 Linux timer layout: host text callbacks dominate, 2026-09-19

One fresh diagnostic build and one functional cell add scalar count/elapsed
aggregates around the actual `Measurer::measure_identified` and `measure`
callbacks. Only `text.rs` and the ignored observer differ from §8.131. Existing
Timer/Host/Geometry spans and eight-word records stay unchanged. The aggregate
requires exact active Geometry17→Host20→Timer32 ownership and a closed scope;
missing metadata is unavailable, distinct from a measured zero. Nested calls,
unwind, overflow and identity/interval mismatches refuse attribution. Fifty-six
pure reader controls pass once, including the old real Geometry witness with
missing metadata; those tests do not execute the Rust callback failure paths.

The full10,000/32 cell passes. Independent reconstruction matches5217 journal
events,1522 turns/17067 spans,209 closed picture chains and all50 timer/aggregate
witnesses. All32 strictly enclosed timers have complete attribution, with eight
in each cohort and no excluded boundary timer. Every loaded/silent timer records
82 callbacks; all idle/recovery timers explicitly record zero. The advancing
cohorts are:

| Cohort | Timers / callbacks | Geometry total | Callback total / median per timer | Geometry residual total / median |
| --- | ---: | ---: | ---: | ---: |
| Loaded | 8 / 656 | 12.659ms | 11.931ms / 1.413ms | 0.728ms / 0.085ms |
| Silent producer | 8 / 656 | 14.704ms | 13.731ms / 1.693ms | 0.973ms / 0.123ms |

Callback sums account for94.25%/93.39% of these Geometry totals. The scope
includes host request/spec identity work, cache access, shaping and width layout;
it does not separate their costs. Borrowed-run construction, inherited style,
the layout engine and frame publication outside the callback remain in the
residual, alongside observer bookkeeping. These are elapsed intervals, not
CPU or font-lookup-only costs. Aggregate non-overlap is qualified by the Rust
busy/closed/flags mechanism; the raw data does not contain leaf intervals from
which an independent callback union could be reconstructed.

Source inspection confirms ordinary measurement builds no ink index or glyph
raster: those remain lazy paint work. Definite measurements retain their useful
width layout for paint, and intrinsic probes already keep scalar answers.
The next narrow candidate is local reuse of consecutive identical font/weight
metric lookups in the width pass, preserving per-glyph size, fallback, explicit
line-height and floating-point arithmetic. This diagnostic justifies examining
text work; it does not measure that candidate's share or establish a gain.

Whole-turn maxima remain13.974/17.909/12.650ms for idle/loaded/recovery, with
12/17/12 misses above8.333333ms. All eight silent producer turns miss, with
19.852ms maximum. Loaded/silent Tick medians are2.233/3.065ms. Observer costs,
startup and final-flush censoring remain separate; no unobserved timing is
inferred by subtracting the recorder. No latency improvement or120Hz is proved.

Full History9 at0/16 and all four saved RGB buffers match §8.131's corresponding
states, not its timings. All36 semantic input handlers are in-prefix, including
18 exact character/draft matches and18 actual signed40-unit wheels. The silent
prefix contains eight advances0→8, no commands and no credited drain; loaded
progress is8→16. Finalframe209 is current and clean through5217. All six runtime
PIDs/four groups retire and port5941 is free after acceptance.

The native process exits−15 after the driver's deliberate post-acceptance
SIGTERM; the driver and outer transport exit0. The frozen owner report's
“Native/driver/outer terminal0” and “No cleanup signals” wording is incorrect
for the native process. Its raw cleanup receipt preserves the signal and exit;
the functional result and completed cleanup are unchanged.

The observer adds12,296B of fixed scalar field payload and26,624B of flush-array
payload on the pinned64-bit target, excluding padding, compiler stack and serde
allocations. Each qualified callback adds two clock reads; there are no leaf
records. The additional wire field is39B empty and conservatively38,182B at full
capacity. Existing65,500B line/40MiB journal and all other limits can refuse;
none is enlarged. The41.119s optimized build captures21 files /12,849,934B
(`dd3fb1df`, ELF `d36545c5`), preserving all1423 source entries except the exact
two-file delta and matching prior generated entry/plan/compat, fonts and external
inputs. Runtime binding `da21ec0c` and raw manifest `f619cd1d` cover the released
cell and23 copied files /25,035,241B. Lead reconstruction `92c5b3d5` and functional
comparison `cff35bbb` match all32 component arrays and input/history/pixel checks.
The final owner report `69abc3ea` / manifest `ddc3c998` preserves58 files
/50,598,258B, including the original cleanup wording noted above.

### 8.133 Linux width layout: reuse consecutive font metrics, 2026-09-19

The width-layout pass now keeps one local last-font/weight slot containing only
units-per-em, ascent, descent and leading. Consecutive identical keys reuse
these unscaled values, including a missing-font result. A different key uses
the original `get_font` path. The slot dies with the width pass under its
existing exclusive catalog borrow; it retains no font, paragraph or catalog
owner and adds no cross-call cache or policy. Per-glyph size, metadata,
fallback font, explicit/normal line-height, floating-point operation order,
comparisons and equal-value flags remain unchanged.

Seven tests were authored before the candidate. An instrumented original loop
fails four lookup-count assertions while three controls pass. Actual width
passes record481→1,75→5 and23→1 lookups; a direct same-ID/different-weight and
missing-font sequence records8→5. The candidate passes all seven. Its repeated
width fixture records1 lookup for481/485/481 glyphs across three independent
passes. These counts measure calls to the existing font cache, not font
allocations or elapsed-time savings.

An exact test-only copy of the original layout loop provides the metrics,
baseline, glyph/source/selection and RGBA oracle. Controls exercise fallback
faces, weights, sizes, metadata, explicit/normal equal ties, zero/fractional
line-height, empty text, catalog replacement and last-owner release. Pixel
comparisons cover three scales, fractional origins, clipping and cold/warm
paths. The full sharing and ink suites pass14+28 distinct tests; the initial
seven are repeated within those14, not additional distinct coverage. Strict
`exact-linux --all-targets` Clippy and scoped formatting pass. Actual compiler
commands use the private Motion/Kernel/Runner/Linux sources. This is scoped
Mac-hosted CPU validation, not a whole-workspace or actual-Linux runtime sweep.

One test-fixture correction is preserved: the bundled Inter face is regular
only, so the requested-bold witness initially failed before its comparison.
Selecting the already loaded DejaVu bold family for that run preserves the
fallback and every assertion; production code required no correction. The
corrected baseline's four failures are lookup counts, not missing APIs or
pixel/metric differences. Source formatting follows the successful checks;
the original source seal and both baseline logs remain retained separately.

The integrated files are `shaping.rs` (`7ccbc339`,517 lines) and
`sharing_tests.rs` (`78366f3c`,991 lines), based on unchanged `aa26f2a4` /
`4c099d21`. Private evidence is under
`exact2-linux-last-font-metrics/target/last-font-metrics-validation`, with
source-v1 `298670f5`, corrected baseline log `662503fc`, candidate log
`8668c928`, sharing `7b5a5c89`, ink `fdf69e0a` and Clippy `7e6ca178`.
The existing callback diagnostic motivates examining this code, but does not
show that font lookup dominates callback cost. Native timing, a net workload
gain and120Hz remain unestablished; the next comparison needs fresh control
and candidate cells with the same full10,000/32 workload and recorder.

### 8.134 Font-metric comparison: control refused, candidate unrun, 2026-09-19

The first fresh native comparison stops at the control's acceptance refusal;
the candidate is not launched. A timer installs a newer picture between the
driver recording its wheel target and the actual wheel input. The frozen
reader requires those picture numbers to be identical, even when target
geometry and scroll offset agree. This attempt establishes no timing gain or
product regression and remains failed.

Independent raw reconstruction finds one mismatch among18 wheels. Loaded
input1347/event4309 records offered frame175 (ACK4295, History revision11),
but the actual preceding installed picture is176 (ACK4303, revision12).
Both are current, at scroll337069.5, with transcript rectangle
[0,211,1024,457]. The next picture177 moves exactly−40 to337029.5. The
reader refuses the frame-number mismatch at final phase validation; the
saved report's earlier `tail-setup` label does not locate that failure.

Partial checks preserve36 scheduled in-window handlers,18 exact typing
echoes,18 actual signed40-unit wheel movements and eight command-free
advances0→8 inside the silent prefix. All four saved RGB buffers match the
prior diagnostic's corresponding states. These facts do not turn the cell
into a pass. The driver computes its local final fence before phase
validation raises, but never reaches the report update that stores the fence,
input proof and workload-completed flag. No success-only timing analysis or
substitute historical control is used.

The candidate's one41.315s build remains valid: ELF `e3a9930d`, capture
`1383cd30` (21 files /12,850,572B). Its1423 source records differ from control
`d36545c5` only in production `shaping.rs`; the ten observer overlays,
generated app, fonts and external inputs match. The fresh pair binding
`0c4877e8` preserves full10,000/32, all caps and fixed schedules. No rebuild,
retry, alternate port or candidate cell follows the refusal.

Native cleanup deliberately sends SIGTERM and reaps−15; driver, outer and
Mac wrapper exit1. All six runtime PIDs/four groups retire and port5941 is
free. Raw manifest `4bd5a36c` seals24 files /22,173,798B; final report
`7768a733` / manifest `83153f9f` preserves42 files /44,392,807B under
`exact2-linux-messages-ink-pair/target/messages-last-font-metrics-before-runtime-v1`.
Lead mismatch reconstruction `bb6a2f03` and partial checks `ff0abb76` remain
separate. The next reader change must qualify both the offered and actual
installed target, retain refusal for incompatible geometry or ownership,
and measure movement from the actual picture. It must preserve this failed
archive and all fixed-prefix, fresh-History and final-publication guards.

### 8.135 Sort cold text entries only when eviction is needed, 2026-09-19

Linux paragraph maintenance now skips sorting cold widths when their byte
budget already fits. After the unchanged width-eviction loop, it sorts cold
identities only if the original keep-inclusive count or remaining byte cost
still exceeds its limit. The full scan, dead-weak pruning, temporary vectors,
cost accounting, pin checks, eviction loops and final binding pruning remain.
When eviction is necessary, the original comparator and order apply. This
removes unnecessary sort calls; it does not remove the linear scan or change
cache policy, ownership, the256-identity limit or the64-entry handoff limit.

Seven tests compare independently owned catalogs against the original trim
body. They cover below/exact/over-budget costs, width reclamation before the
identity decision,256/257 identities, absent and pinned keep values, tied
identity ages, dead widths and stale bindings, handoffs and lazy ink growth.
Test-only counters sit at the two actual sort sites. The corrected baseline
has six count failures and one passing control; the candidate passes7/7.
Full residency32 and identified-cache10 tests pass:42 distinct tests across49
executions, with strict all-targets Clippy and scoped formatting passing.
Actual compiler commands use the private Motion/Kernel/Runner/Linux sources.

The earlier supervisor argument failure and one fixture failure are retained.
The fixture had assigned both bindings the same node owner, so the second
correctly replaced the first before trim. Distinct nodes2/3 in one kernel fix
the setup without changing production, the original reference or assertions.
The third attempt supplies the meaningful baseline and passing candidate;
formatting afterward changes only presentation of the tested source.

The integrated files are `cache.rs` (`5b026302`,795 lines) and
`residency_tests.rs` (`889350d0`,1316 lines). Evidence is under
`exact2-linux-trim-sort/target/trim-sort-validation`: source seal `8bf4b9d5`,
production-only patch `3b20b2f0`, corrected baseline log `29e4962b` and
`cpu03-candidate` logs. All36 recorded candidate PIDs/six groups are gone.
This is scoped Mac-hosted CPU validation; no native timing improvement is
established, and the font-metric comparison in§8.134 remains failed with its
candidate runtime unrun. The diagnostic's callback share does not attribute
time to cache sorting, and no120Hz claim follows from these call counts.

### 8.136 Fresh font-metric pair: functional pass, mixed timing, 2026-09-19

A separate fresh control→candidate pair passes the full10,000-row/32-change
workload with the qualified wheel reader. Both products and the recorder are
unchanged from§8.134: only production `shaping.rs` differs between them;
neither includes§8.135's later trim-sort change. This first successful pair
establishes no consistent overall performance gain. The original failed
control and unrun candidate archive remain unchanged.

The reader accepts a newer installed picture only after complete
CPU/submit/ready/ACK joins, fixed-source target qualification, compatible
geometry/scroll, ordered input and actual predecessor movement. Raw boxes
alone do not prove generic List identity. Treatment input1362 exercises this:
offered173 is replaced by174 after timer5008, then picture175 moves exactly+40.
The selected reader bytes passed their original24 checks. A later26-check
run passed25 and failed one exact-message assertion: the changed input was
refused by an earlier guard. That failure remains recorded; no fourth
correction or26/26 claim is made.

Independent reconstruction verifies full nine-field Histories at revisions0
and16, all10,000 five-field rows and UTF-8 body-byte counts. All four saved
2,359,296-byte RGB buffers match between arms. Each cell delivers36 scheduled
in-window handlers,18 typing echoes and18 actual signed40-unit wheels.
Loaded phases install20 pictures carrying newer History revisions9–16 above
entry8, with changed32 and a visible suffix. Those pictures retain their own
painted History even when newer live state is dirty:17 control and19 candidate
ACKs are also live-current. Both silent prefixes contain eight advances0→8
and zero input commands. Final fences close at frame194/sequence4813 and
frame189/sequence5249; control's three recovery drain events are excluded.

Strictly enclosed loaded and silent timer cohorts contain eight timers and
656 text callbacks per arm. Idle has eight timers each; recovery has seven
versus eight. Times below are instrumented elapsed milliseconds, not isolated
font lookup or CPU costs:

| Scope | Control | Candidate |
| --- | ---: | ---: |
| Loaded callback median / sum | 1.503 / 12.480 | 1.117 / 10.375 |
| Loaded Tick median / maximum | 2.321 / 3.039 | 1.782 / 3.436 |
| Silent callback sum | 14.280 | 14.247 |
| Silent Tick median / maximum | 3.091 / 4.524 | 3.125 / 5.600 |
| Loaded whole-turn maximum | 17.224 | 18.251 |
| Silent whole-turn maximum | 19.195 | 20.821 |

All eight silent producer turns exceed8.333ms in both arms. Loaded whole-turn
counts differ60→267, so their aggregate246.157→211.772ms does not establish
equal-work savings. Silent totals rise121.207→127.359ms. Loaded Paint medians
fall2.904→2.458ms, while silent Paint sums remain29.822→29.961ms; observer
costs and natural scheduling remain in the measurements. This is one run
order, without a repeated gain, resize or physical120Hz claim.

A separate selected reconstruction of§8.132's existing diagnostic explains
why its whole-turn tail needs caution: the19.852ms silent turn includes
9.979ms inside diagnostic picture observation,5.105ms Paint,3.275ms Tick and
1.097ms native pixel copy. Picture observation hashes786,432 RGB triplets
through individual SHA updates and also samples geometry/state; hashing alone
was not timed. Its nested journal write is only a small part of that scope.
Full History exports occur at two paused checkpoints, not on every silent
turn. Subtracting observed overhead would not establish a faster executable;
Tick plus Paint alone already exceeds8.333ms in that selected turn. A future
batched observer must preserve exact bytes and be identical in both arms of
any subsequent comparison.

Both native processes receive deliberate post-observation SIGTERM (−15);
drivers, outer wrappers and Mac launchers exit0 without watchdog actions.
All12 recorded PIDs/eight groups retire and port5941 is free. The source
binding is `534aeccb`. Under
`exact2-linux-messages-ink-pair/target`, control root
`messages-last-font-metrics-before-runtime-v2` seals52 files/48,913,306B
(report `3f8354c2`, manifest `a82793eb`); candidate root
`messages-last-font-metrics-after-runtime-v2` seals60 files/50,705,951B
(report `35173b26`, manifest `396f8921`, cohort `300dbdb3`). Lead raw and cohort
checks remain under `target/ink-placement-integration`; cohort `231a62c8`
records corrected local inspection assumptions without changing the reader,
raw evidence or either runtime. No further runtime is implied by this closure.

### 8.137 Three approved native-reference diagnostics, 2026-09-19

Charlie explicitly approved up to three additional bounded reference attempts
after§8.122, then requested merging the verified progress. These diagnostics
change only an appended private test; production, the original five tests,
10,000 rows and batch32 remain unchanged. They use the ordinary native text
engine and CPU painter without registering a content region, on macOS rather
than an actual Linux display. Each runs once under the existing240s guard.

All three attempts fail. The first reuses a helper requiring changed32 at
revision0, where changed0 is correct, and never reaches geometry. A local
diagnostic-only extraction corrects that premise for the second attempt; after
eight wheel calls it still sees rows0–6 instead of row9999. The final attempt
adds bounded before/after wheel and hit-ancestry records, preserving the point,
movement policy and tail assertions. It fails the same6-versus9999 assertion.

The final raw records explain the failed scrolling in this ordinary reference:
each wheel at(10,228) hits `split-pending`, the fixture's absolute
“Preparing messages…” label. Its acknowledged ancestors are `split-owner`
and the root, all with zero scroll extent; the List is a sibling, not an
ancestor. The List itself has a positive323,784.1875 maximum offset, but all
eight before/immediate/settled observations retain offset0, sequence1 and
range0–6. Thus seven mounted rows here are the head, not a measured native
tail invariant. Full nine-field History bytes still match the canonical
10,000-row answer and remain unchanged across the wheel attempts. The saved
980×820 picture is acknowledged, but row9999 has no viewport intersection.

This identifies the ordinary diagnostic's target obstruction; it neither
establishes33 native tail rows nor proves the worker candidate's original
failure has the same cause. Its132-owner, byte-admission and ACK-progress
claims remain unproved. No fourth attempt, fixture workaround or production
integration follows. The unvalidated Linux native adapter and Swift native
consumer remain excluded from the verified feature branch.

All three filtered tests exit101 without watchdog actions. Final execution
`ordinary-tail-diagnostic-execution-03` records0PASS/1FAIL in1.97s; its ten
recorded PIDs and one group are absent and the Cargo lock is released.
Evidence remains under
`exact2-linux-split-facts-native/target/split-facts-native-validation/`:
source03 `804981fb`, hit addendum `ad36c9bd`, test bytes `66351f83`, final log
`02395023` and release `6a352327`. The earlier failures and§8.122's candidate
archive remain preserved; this is diagnostic closure, not native readiness
or a performance result.

### 8.138 Move eligible owned text into a new cache identity, 2026-09-19

A cold identified text request previously constructed an owned `Spec`, cloned
its runs and strings into the canonical cache identity, then discarded the
temporary. The Linux text engine now moves that temporary into the canonical
`Arc` on a new identity. Borrowed and owned inputs share the original lookup;
fingerprint, exact equality, clock/serial updates and trim-before-insert order
are preserved. An equal-content hit retains the existing canonical identity.

The move requires the runs vector and every run/strut string to have capacity
equal to length. Spare-capacity inputs keep the old clone path because capacity
is charged by the cache: moving arbitrary spare storage would change eviction
accounting. Borrowed callers also retain their original clone behavior. No
cache limit, cold eviction policy, stamp qualification, shaping, layout, font
work or accepted-owner lifetime changes.

Six new tests run against identical fixtures with old and new production.
The baseline passes four controls and fails two source-copy counts. A real
Kernel-derived Unicode request copies82 bytes instead of the required41;
the candidate copies41. A separate styled, owned builder copies37 bytes on
baseline and zero on candidate. These counters cover host `Run::from_style`
and `Run::clone` UTF-8 bytes only, not all text-library allocations or memory.
Independent borrowed engines match metrics, baselines, glyph data and RGBA
before those count assertions. Tests also check actual old-clone capacities,
spare-capacity fallback, eviction accounting, equal-content identities, empty
inputs and independently retained old/new paragraph owners.

Candidate focused6, complete identified16 and residency32 tests pass:48
distinct tests across54 candidate executions. Strict all-targets Linux-package
Clippy and scoped formatting pass. Both production files are byte-identical
to their compiled versions; subsequent test formatting changes only whitespace
and optional trailing commas. A lifetime fixture was corrected before execution:
`clear()` intentionally preserves pinned identities, so the test must also drop
the engine/cache owner before asserting retirement at the last external owner.
The earlier source-only fixture remains preserved, with no executed failure
claimed for it. All63 recorded PIDs and seven groups are absent.

Integration uses exactly `text.rs`, `text/cache.rs` and
`text/identified_tests.rs`, from base `5300a42`. Evidence is under
`exact2-linux-owned-text-spec/target/owned-text-spec-validation/freeze-final`:
source `55c6e09c`, full patch `506937d2`, production patch `9c29bdef` and
report `a70b1e67`. This removes a demonstrated copy on an eligible cold path;
the earlier82-callback observations do not quantify its frequency or cost.
Native latency, throughput and120Hz benefit remain unmeasured.

### 8.139 Fresh font pair with batched picture hashing, 2026-09-19

The ignored picture recorder now feeds the same RGB byte stream to SHA in
768-byte chunks using a fixed stack buffer. A1024×768 picture still hashes
2,359,296 bytes, through3,072 updates instead of786,432. The old implementation
fails two update-count assertions while five controls pass; all seven pass
with batching. This establishes byte-stream and call-count preservation, not
an isolated hashing-time measurement. Both new native products use the same
recorder; earlier runs with the old recorder are not matched timing controls.

Two optimized Linux captures differ only in the already integrated last-font
lookup from§8.136. They retain the older matched source basis, without§8.138's
owned-Spec move or the later trim-sort change. Source materialization first
stopped on archive permissions before any build: raw Git archive mode0664 did
not satisfy the0644 source card. A separately preserved preparation uses
explicit `tar.umask=0022` and passes the original mode check. Both builds then
exit0 once, followed by one fresh control and one treatment runtime. No runtime
retry, workload reduction, reader change or additional reference test occurs.

Both runs pass the frozen functional acceptance at1024×768 under VKMS. Full
nine-field History values match at revisions0 and16, including all10,000 rows,
UTF-8 body-byte counts and32 changed rows after advancement. All four saved
RGB pictures match exactly. Each fixed2s interaction phase contains12 handlers:
36 per arm, including18 actual signed40-pixel wheel movements. Loaded entry
revision8 advances through16 in eight timers; each arm installs20 in-window
pictures carrying newer visible History, of which17/18 respectively are also
current against live state at ACK. Silent windows contain eight0→8 advances
and no input. Final picture chains204/193 close through journal5141/4531,
with no later dirty or unflushed work admitted by the fence.

The performance result is mixed and does not repeat the earlier loaded
callback improvement. The table uses eight complete timer turns per cohort;
callback values aggregate82 callbacks per timer, not individual-call medians.

| Wall time, ms | Control | Treatment |
|---|---:|---:|
| Loaded callback aggregate median |1.403816|1.571504|
| Loaded callback aggregate sum |11.836748|13.165089|
| Loaded whole timer-turn median |12.658818|13.281696|
| Loaded whole timer-turn maximum |15.198141|16.676230|
| Silent callback aggregate sum |14.325365|15.160808|
| Silent whole timer-turn median |17.448713|17.387316|
| Silent whole timer-turn maximum |20.145993|20.441452|

Loaded timer turns exceed8.333ms in6/8 control and8/8 treatment observations;
all eight silent timer turns exceed it in both arms.
Whole loaded-phase sums decrease236.191612→215.943862ms, but idle and recovery
sums increase146.673143→160.978233 and120.504548→137.066480ms. Natural turn
counts differ99/51,132/58 and50/60; equal inputs and pixels do not make their
execution cohorts identical. The complete captures contain1,466/901 turns.

The recorder remains substantial: silent picture-observer medians are
8.989910/8.905972ms, alongside Paint3.958307/4.089120ms and
Tick3.368325/3.279721ms. These are separate medians, not additive components
of one synthetic turn. The observer includes sampling, hashing and journal
work; hashing alone is not timed. Subtracting recorded observer/wait scopes
does not establish performance of an unobserved executable. No isolated font
CPU, consistent net speedup, resize result or physical120Hz claim follows.

The original v2 reader qualification remains24 prior passes and a final25/1
exact-refusal-message assertion result, not26GREEN. Both native processes
receive intentional post-observation SIGTERM(−15); driver, outer and Mac
wrappers exit0. The saved release receipts show all12 recorded PIDs/eight
groups absent and port5941 free, with control cleanup before treatment launch.

Products are `05f810a4`/`d3b3d438`, common recorder `17537a6f`, source-ready
manifest `1084c094`, and runtime plan `476c1f5e`. Copied raw captures under
`exact2-linux-messages-ink-pair/target/` are
`messages-last-font-batched-observer-before-runtime-v1` (23 files/24,895,916B,
actual manifest `979ccaa1`) and `messages-last-font-batched-observer-after-runtime-v1`
(23 files/23,680,206B, actual manifest `bcd00bd5`). Lead independent raw,
cohort and selected-release proofs are `087bc4da`, `29395724`, `ae2c7534` and
`a6fb2eb2` under `target/observer-batching-integration`. These are new captures;
all earlier failures and evidence remain unchanged. Final owner archives seal
52 files/50,337,095B (`da485454`) and64 files/48,004,483B (`49fb9402`), with
pair report `e3b02a9b`. An initial report-only lookup error is preserved; no
analyzer or native rerun follows that prose correction.

### 8.140 Reuse the validated Spec on a warm lookup, 2026-09-19

`Cache::identified` previously looked up and cloned an `Arc<Spec>` to validate
a warm stamp, discarded it, then returned the key so its caller could repeat
the same lookup and clone. It now returns that validated owner with the key.
There is no intervening eviction or callback. Stamp qualification, stale
binding removal, recency, clock and `used` updates retain their original order;
width lookup, cold insertion, trimming and ownership budgets are unchanged.
This adds no persistent owner and does not skip layout at a new width.

A counter at the real `Cache::spec` entry makes the old production fail one
warm-Painter assertion,2 calls instead of1, with two controls passing. Pixels,
paragraph identity and absence of new host copy/hash/shape work are checked
before that assertion. The candidate passes all three new tests, the complete
19 identified tests and32 residency tests:51 distinct tests/54 executions.
Independent engines using the original lookup sequence match cache state,
geometry, glyphs, pixels and owner retirement. Baseline keeps the old two
residency assertion forms; only the candidate adapts them to its private
return type. The meaningful new tests are identical across arms.

Strict all-targets Linux-package Clippy and scoped formatting pass. Both
production files match their compiled bytes; later test formatting is limited
to whitespace and optional trailing commas. All41 recorded PIDs/seven groups
are absent. Integration takes four exact files from
`exact2-linux-owned-text-spec/target/warm-identified-spec-validation/freeze-final`:
source `20f1c173`, full patch `e7fb9ba3`, production-only patch `8c8b570c`.
This proves one fewer validated-Spec search/clone/drop on the warm path,
not the number of such hits in§8.139 or a native timing/120Hz improvement.


### 8.141 Direct glyph blit: exact pixels, broad candidate not selected, 2026-09-19

A private Linux CPU text experiment replaces each eligible glyph's generic
`draw_pixmap` setup with a direct premultiplied SourceOver blit. Eligibility
requires the exact identity transform, bounded integer placement and matching
mask dimensions; other cases use the existing renderer. The implementation
retains tiny-skia0.12's f32 load, mask, blend and ties-to-even store order.
It adds no glyph cache, allocation, retained owner or workload reduction.

Tests first run the original generic draw through a counted wrapper. Six
controls pass and the ordinary Painter test fails on32 generic glyph calls
for both cold and warm paints, versus the expected zero. The first candidate
removes those calls and passes the real text scene, but fails a cropped-edge
pixel comparison. That failure is retained. Matching the existing scanner's
`Rect::round` and padded source-edge behavior fixes the mismatch without a
pixel tolerance or mask exclusion. A later test-only Clippy correction is
also preserved. The final candidate passes35 ink tests and nine paint
integration tests:44 distinct tests,86 passing candidate executions including
repeats. Strict all-targets package Clippy and scoped formatting pass. These
are CPU tests on the Mac, not an actual Linux display or physical frame test.

A separate optimized rejection probe copies the complete production helper
byte for byte (`c64c3b22`). Both implementations run in one executable against
the repository-pinned tiny-skia dependencies, using Rust1.97 on
`aarch64-apple-darwin`, opt-level3, thin LTO and one codegen unit. It compares
patterned premultiplied glyphs on a1024x768 destination with signed/cropped
placements, overlapping draws and no/full/partial masks. Each case retains
eight alternating-order samples of1,024 draws. Allocations precede timing;
all96 paired final RGBA buffers match exactly.

| Glyph size | No mask ratio | Full mask ratio | Partial mask ratio |
|---|---:|---:|---:|
|8x16|0.791|0.705|0.755|
|16x24|1.004|0.975|0.990|
|32x48|1.211|1.124|1.141|
|96x96|1.280|1.172|1.165|

Ratios are candidate/reference median elapsed time within each synthetic
case; above1 is slower. Every retained32x48 and96x96 candidate sample is
slower than its paired reference. Thus removing generic pipeline setup does
not justify this unconditional eligible-glyph replacement: scalar blend work
can outweigh that saving. The production candidate is **not integrated**.
No size threshold is tuned to these samples, and no full-app/native gain,
individual glyph distribution, observer-free performance or120Hz result is
claimed. Actual cached font glyphs may have a different coverage distribution;
these patterned images are a rejection probe, not a Messages benchmark.

The lead probe preserves two setup refusals before any compilation or timing:
the system Python lacks `tomllib`, and an unconstrained offline lock resolution
selects a cached dependency newer than the repository pin. The executed attempt
seeds the exact repository lock, resolves offline, checks every registry
version/checksum, then builds locked. One optimized build and one probe exit0;
all recorded processes/groups are absent. No native application is launched.

Correctness sources/raw logs remain under
`exact2-linux-owned-text-spec/target/glyph-blit-validation`; the complete probe,
raw timings, selected dependency proof and release are under MAIN
`target/glyph-blit-performance-probe`, including `mac-result-v2`,
`mac-summary.json`, `source-binding.json` and `mac-release.json`.
The earlier native captures and merged production remain unchanged.

### 8.142 Merged text-cache bundle: fresh Linux pair, mixed whole-turn result, 2026-09-19

One fresh optimized build evaluates the combined trim-sort, owned-Spec and
validated warm-Spec changes. The candidate is Git`6be906af`, tree-identical to
the then-merged`eb516d4`; subsequent`b232ec4` integration is **not measured**.
The control reuses the immutable§8.139 treatment ELF`d3b3d438`, with a new
runtime cell. Its old timings are not a control. The new ELF is`f7b8f75b`;
one build exits0 in41.936s, with all five private Cargo artifacts non-fresh,
exact source receipts and identical generated Reusable entry, plan, compat,
fonts and external dependencies. The production differences are text/cache;
four test-bearing files and the experiment document also differ physically.
The shaping body and batched observer`17537a6f` remain byte-identical.

Both fresh cells pass once, in control→candidate order, with the unchanged
Linux VM/VKMS/RFB recipe:1024x768, full10,000 rows/32 updates, windowed=true,
bounded=false, ordinary Reusable path and no Region worker. No resize is
measured. Each retains36 in-window input handlers and18 exact±40 wheels.
Initial/final complete History9 at revisions0/16 match across arms, including
all10,000 five-field rows and UTF8 body-byte totals. All four2,359,296B RGB
captures match exactly. Loaded prefixes contain20→19 fresh installed suffix
pictures above entry revision8;17 in each arm are also current at ACK. Each
silent2s prefix advances0→8 with no input. Final pictures192/195 close through
journal sequences5450/5079; later dirty or unacknowledged work is not hidden.

| Strict timer cohort, control→candidate | Loaded | Silent |
|---|---:|---:|
| Complete timers / callbacks in each arm |8 /656|8 /656|
| Callback aggregate median, ms |1.367549→1.224611|1.701345→1.550135|
| Callback aggregate sum, ms |11.348758→10.198171|12.749358→12.327219|
| Whole enclosing-turn median, ms |12.000869→13.787021|15.769366→17.145767|
| Whole enclosing-turn maximum, ms |15.448406→17.763831|18.735713→18.220834|
| Whole enclosing-turn misses above8.333ms |5/8→7/8|8/8→8/8|

Each callback median is over eight per-timer aggregates of82 calls, not656
individual measurements. Loaded revisions8→16 match. Recovery has7→8 timer
calls; idle/loaded/recovery whole-turn populations are212/130/49 versus
243/55/153. Those different loop populations are not equal-work frame rates.
A later reconstruction of the same parent/child spans finds a FrameBuild in
only5/8 loaded timer turns for the control versus7/8 for the candidate; all8
silent timer turns contain a frame in both arms. The loaded whole-turn medians
therefore also mix different in-turn painting work. An absent frame in a timer
turn does not mean eventual painting is missing: the installed-picture counts
above remain unchanged. The control's three recovery drain events remain outside
its fixed prefix.
All-phase whole-turn maxima are13.578/15.448/13.016 versus
14.974/17.764/12.174ms. The silent observer median is8.005→8.892ms and paint
median3.836→3.953ms. Instrumentation remains inside whole-turn costs; its
subtraction does not establish uninstrumented performance, and medians are
not additive. This single ordered pair shows lower callback aggregates but
**no consistent whole-turn gain**, no isolated attribution among the three
changes, no CPU-only result and no physical120Hz claim.

Both drivers/outer wrappers exit0; native SIGTERM follows completed
observations. Saved release records show all12 owned PIDs/eight groups absent
and port5941 free. No retry, new baseline build or acceptance change occurs.
The source package's v1 archive-header provenance concern is preserved:
extraction already normalized Git modes; v2 explicitly supplies
`tar.umask=0022`, without a failed staging run or changed production bytes.
Frozen build/runtime evidence is in the private ink-pair checkout under
`target/messages-current-integrated-{build,control-runtime,candidate-runtime}-v1`;
lead raw reconstruction and equality checks are in MAIN
`target/integrated-text-cache-evaluation`. The earlier archives remain intact.


### 8.143 Linux observer hashing costs a substantial part of a frame interval, 2026-09-19

A separate optimized primitive probe measures the exact13-line RGB batching
helper from observer`17537a6f`, using the unchanged§8.142 candidate's final
1024x768 RGB capture. Its2,359,296 bytes have SHA-256`e080ceb8`; opaque RGBA
reconstruction happens before sampling. Each of24 alternating-order pairs
hashes both that contiguous RGB buffer and the reconstructed RGBA through the
original extraction/helper. All48 results equal the saved full digest.
Initialization, updates and finalization are timed; input construction, digest
comparison and output formatting are outside the samples. No application or
GUI runs, no workload is reduced and no production source changes.

| Standalone wall time,24 samples each | Minimum | Median | Maximum |
|---|---:|---:|---:|
| Exact RGB extraction plus SHA-256, ms |3.927266|3.958974|4.172725|
| Contiguous RGB SHA-256, ms |3.709098|3.726827|3.783265|

The actual AArch64 Linux ELF`3d7092dd` is freshly compiled with the app's opt3,
thin LTO, one codegen unit and panic-abort release settings, debug0 and no
incremental compilation. Actual sha2`0.10.9` features are default/std, without
asm; its captured dispatcher selects the software backend on AArch64 despite
the VM detecting the SHA2 hardware feature. This identifies an available
investigation, not a measured hardware-backend benefit. All10 dependency
archives match the repository lock checksums and their actual extracted source
members; compilation uses the existing offline cache, with no new dependency.

The original prebuild wrapper incorrectly expected a vendor checksum file in
Cargo's registry cache; v1 stops before any compilation or probe. V2 preserves
that failure and checks the cached archives directly. Its single build exits0
in1.556s and its single probe exits0, without retry or cleanup signals.
The fixed120s/30s guards and disk floor remain intact; owned process/group
absence and Cargo-lock release are recorded separately.

The primitive's roughly4ms median supports reducing recorder overhead before
another whole-application comparison. It is not a CPU profile, a full observer
measurement, an application gain or a quantity to subtract from old timings.
The existing silent timer turns contain observer medians8.005/8.892ms; nested
journal work accounts for only0.013521/0.015771ms at the median. Sampling,
geometry witnesses and other recorder work remain in the observer interval,
and medians are not additive. No uninstrumented or physical120Hz result follows.
The private ink-pair checkout retains`target/observer-rgb-cost-v1` and`-v2`;
MAIN`target/observer-cost-investigation` holds the independent48-sample arithmetic
and the existing-span reconstruction, including§8.142's5/8 versus7/8 loaded
painting qualification. Earlier captures and timings remain unchanged.

### 8.144 A standard SHA-256 backend reduces the standalone observer primitive, 2026-09-19

A fresh optimized Linux probe keeps§8.143's exact RGB extraction helper and
full1024x768 input. Twenty-four alternating-order pairs compare sha2`0.10.9`
default/std with ring`0.17.14`'s standard SHA-256 Context in the same binary.
All48 full digests equal the captured2,359,296-byte RGB digest`e080ceb8`.
Nine untimed pixel-count boundary controls check cross-backend equality,
alpha-only invariance and changed-RGB detection; mutation is inapplicable for
the empty input. Initialization, updates and finalization are timed. Input
construction, comparisons and digest formatting happen outside sampling.

| Extraction plus SHA-256 wall time,24 samples each | Minimum | Median | Maximum |
|---|---:|---:|---:|
| sha2 default/std, ms |3.926808|3.946725|4.635561|
| ring standard backend, ms |0.885213|0.893879|1.020129|

The AArch64 ELF`a043595f` uses the app's opt3, thin-LTO, one-codegen-unit,
panic-abort release profile, debug0 and no incremental compilation. Actual
ring features are alloc/default/dev_urandom_fallback. The VM reports SHA2
support, and the captured standard dispatcher selects hardware support at
runtime with a fallback. This is not an instruction trace or CPU attribution.
The27 pinned dependency archives come from the existing cache and match their
checksums; no dependency version, custom compression, forced CPU target or
sha2 asm feature is introduced.

One optimized build exits0 in1.972s and one primitive run exits0, with no retry
or cleanup signal. The120s/30s guards and disk floor are unchanged; saved
cleanup proves all10 recorded PIDs/eight groups absent and the Cargo lock
released. Private`target/observer-rgb-backend-v1` preserves report`6eaefc05`
and execution manifest`aa087425`. MAIN`target/observer-cost-investigation/`
holds independent all48-sample/control arithmetic in`backend-independent.json`
(`4449a067`) and selected final identity checks.

This supports using the cheaper standard backend for subsequent recording.
It establishes neither a whole-observer nor application speedup, and its
difference must not be subtracted from earlier app timings. The prospective
common observer changes pixel hashing and lowercase digest formatting only;
snapshot hashing, workload, acceptance and quotas remain unchanged. A fresh
application comparison is still required. No physical120Hz claim follows.

### 8.145 Kernel measurement cache: fewer callbacks, mixed first Linux pair, 2026-09-19

Two fresh optimized products share f72c142, the lighter§8.144 pixel recorder,
the current Reusable factory and identical generated entry/plan/compat and
fonts. Only`kernel/src/layout.rs` differs: control removes the already merged
four-offer measurement cache while retaining current Video and invalidation
behavior; candidate is current production. Actual2,005 source paths/23 links
and five private `fresh:false` artifacts are checked for each capture.
Control ELF`cf532a9b` builds in45.171s; candidate`5276fbf7` in42.736s.
This experiment changes no production defaults.

The first build stops before compilation because the workspace lock records
ureq3.4.2 while the unchanged external Ibex manifest requires exactly3.4.0.
One isolated offline metadata resolution changes only that version/checksum.
Both fresh build inputs use the same explicit correction and retain
`--locked --offline`; MAIN's lock and the original failed root stay unchanged.

Each actual Linux cell passes once with full10,000 rows/32 updates at1024x768,
three fixed2s phases,36 in-window handlers and18 exact ±40 wheel movements.
Full nine-field History values match between arms at revisions0 and16,
including all10,000 rows and UTF-8 body-byte totals. Four full RGB captures
are byte-identical. Loaded prefixes install21/20 fresh pictures at revisions
9–16; both command-free2s prefixes advance0→8 with eight timers and no input.
Independent raw stderr/journal joins close all200/205 picture chains and
the final clean fences at4862/4779. Drain observations receive no phase credit.

| Complete advancing-timer cohort | Control | Current cache |
|---|---:|---:|
| Loaded timers / callbacks |8 /656|8 /552|
| Loaded callback-sum median, ms |1.431738|1.536442|
| Loaded geometry median, ms |1.530610|1.634882|
| Loaded whole-turn median / max, ms |9.324122 /10.594503|9.893604 /10.829878|
| Loaded whole turns above8.333ms |5/8|8/8|
| Silent timers / callbacks |8 /656|8 /552|
| Silent callback-sum median, ms |1.611929|1.573923|
| Silent whole-turn median / max, ms |11.157025 /12.087592|10.918295 /12.125175|
| Silent whole turns above8.333ms |7/8|8/8|

Each advancing timer makes82→69 callbacks, but fewer calls do not establish
lower cost. Loaded callback sums total11.430726→11.826135ms; silent sums
12.472895→12.945632ms. These are per-timer aggregates, not individual callback
medians. Only6/8 control loaded timer turns paint, versus8/8 candidate turns;
restricting to those gives whole medians10.037875→9.893604ms, still unequal
cohorts. All eight silent turns paint in both arms, with whole totals
86.599525→88.178489ms. Idle has8/7 complete timers and recovery8/8; both have
zero text callbacks. Their whole-turn maxima remain below8.333ms.

Silent Paint medians are4.282414→4.123496ms and observer medians
2.290364→2.298344ms. Paint remains a substantial application cost; observer
wall time still includes work beyond hashing. Neither this one fixed-order
pair nor comparison with older, differently instrumented products proves a
consistent speedup, CPU attribution, repeatable gain or physical120Hz.

After the control's successful native run/copy/cleanup, a local finalization
helper reads an obsolete report path and stops before candidate launch. The
failure remains frozen. Reading the actual copied path independently passes
the original digest/acceptance assertions and all raw joins. A two-literal
local helper correction then permits the candidate's first run,186.033s
after control termination. Neither native cell is retried, and no runtime
reader, source, quota or port changes. Both driver/outer exits are0; native−15
is deliberate post-observation cleanup. All12 runtime PIDs/eight groups are
absent and5941 is free; auxiliary copy/check children are separately retired.

The private ink-pair checkout retains`messages-kernel-cache-ring-*-build-v2`
and`-runtime-v2`, control seal`a530df9a`, candidate seal`6ec9a38c`, and
`kernel-cache-ring-pair-closure-v1` manifest`9dfd7617`. MAIN ignored
`target/kernel-cache-ring-evaluation` holds independent build, functional,
timer and painting-cohort reconstructions. Build sources match before/after;
runtime checks2028 identities before launch, with no post-runtime source
rehash claimed. Earlier failures, archives and timings remain intact.

### 8.146 Linux CPU Paint: text and shape fills dominate the recorded span, 2026-09-19

One fresh optimized diagnostic keeps§8.145's current kernel-cache product,
f72 source, Reusable factory, ring pixel recorder, fonts and normalized lock.
Only ignored observer code changes: eleven outer Raster method guards and a
fixed scalar accumulator. No production rendering changes or new phase IDs.
Each Paint26 parent carries counts and wall-time sums for begin, fill, stroke,
image, text, clip push/pop, opacity push/pop, pointer and finish. Exact parent,
input and frame-attempt identity, closed status, flags, count and elapsed bounds
are checked. Missing, nested, overflowing or incomplete data refuses attribution.
There are two clock reads and TLS bookkeeping per admitted method call; no
per-glyph clocks, per-call allocations or retained rendering objects. Fixed
recorder caps remain unchanged, and extra instrumentation cost is not zero or
measured separately. Eight actual-recorder-core Rust checks and ten focused
reader checks pass before the full build.

The aarch64 ELF`3126cbc6` passes one optimized build and the original actual
Linux workload once: full10,000 rows/32 updates at1024x768, three fixed2s input
phases plus command-free2s. All36 handlers and18 exact ±40 wheel movements are
inside their prefixes. Initial full History9 and three setup RGB buffers match
the prior candidate. Final history naturally reaches17 rather than16: all
10,000 rows and body-byte totals validate, the first9,968 rows remain exact,
and the last32 preserve their other four fields. Final pixels close their own
197/4744 picture/fence; they are not an equal-revision comparison with the old
final image. No prior timing is paired with this instrumented diagnostic.

Raw stderr equals the4,744-event journal. Independent reconstruction closes
197 complete picture/Paint chains,1,099 turns and14,757 phase spans, with at
most72 rows per turn and no missing/flagged Paint aggregates. Strict whole-turn
containment selects12/19/12 Paints in idle/load/recovery. Loaded has seven
complete timer turns, five containing Paint; timer4511 straddles the prefix
boundary and is excluded. All eight silent timer turns contain Paint and
advance0→8 without input or drain credit.

| Silent cohort: eight Paints | Per-Paint aggregate median, ms | Sum, ms | Share of summed Paint |
|---|---:|---:|---:|
| Whole Paint |3.740035|28.276935|100%|
| Text,392 calls |1.932441|14.459548|51.135%|
| Fill,200 calls |1.520194|11.367376|40.200%|
| Begin,8 calls |0.156188|1.175630|4.158%|
| Unclassified remainder |0.144481|1.113424|3.938%|

These medians describe each Paint's aggregate, not individual method calls;
medians are not additive. Non-overlapping method sums are bounded by their
own parent. Clip push totals0.007749ms and pop0.000500ms; pointer0.152332ms,
finish0.000376ms. Stroke, image and opacity methods have zero calls in this
cohort. Text includes ink-index construction/query, glyph-cache/raster work
and drawing. Fill includes path construction and rasterization. Neither is
isolated CPU, and the remainder is not a specific named algorithm. Across all
19 strict loaded Paints, text/fill account for44.98%/45.39% of summed Paint.
This supports investigating actual shape fills alongside text before selecting
another generic glyph-blitter change; it does not establish a particular fill
shape, cache policy or replacement rasterizer as the cause.

Whole-turn misses remain. The eight silent timer turns have median9.266950ms,
maximum12.887005ms and6/8 above8.333ms. The seven loaded timer turns have
median9.250075ms, maximum10.109620ms and5/7 misses; only five paint. All-phase
idle/load/recovery maxima are7.637528/10.109620/6.825983ms. This is one
diagnostic, not repeated benefit, uninstrumented performance or physical120Hz.

All2,006 actual source paths/23 links match before and after building; the
three diagnostic source differences are explicit. Generated entry/plan/compat
remain byte-identical to the current candidate. An initial local binding copy
refuses a newly copied read-only destination; the owned destination-mode fix
is preserved before any build. A later independent arithmetic script's exact
wheel-input/ACK-input assumption fails because subsequent pointer packets can
share the batch. Its corrected join uses the complete post-handler picture,
forbids intervening key/wheel input, and still requires exact predecessor
movement. Neither issue changes runtime acceptance or causes a native retry.
Driver/outer exits are0; native−15 is intentional post-observation cleanup.
Saved release`54798269` proves six runtime PIDs/four groups absent and5941 free.

MAIN ignored`target/linux-paint-cost-source-v1` seal`67019dce` preserves source
and pure checks;`target/paint-cost-evaluation` holds independent source, raw,
aggregate, cohort and functional proofs. The private ink-pair checkout keeps
`messages-paint-cost-build-v1` capture`5118f90e` and
`messages-paint-cost-runtime-v1` capture`c0c085b4`. Historical archives remain
unchanged. No new native SplitFacts reference is consumed or authorized.

### 8.147 Fill-mask shortcut rejected; actual fill categories

An isolated tiny-skia0.12 prototype first checks actual clip-mask bytes for
full255 coverage before omitting the mask. The broad version changes rounded
AA edge pixels: masked SourceOver and mask-free Source use different lowp
rounding. This is an executed pixel mismatch, not only a source concern.
The broad optimization is rejected without production integration.

The third/final prototype round retains only opaque integer rectangles with
identity device transform;1,440 full-buffer controls pass, with only15 positive
optimized cases, plus4,000 randomized rounded fallback controls. Those4,000
are not positive optimization coverage. Eight alternating synthetic pairs of
32 full1024x768 all255-mask fills have median58.901→7.724ms on this Mac;
rounded and partial-mask cases retain the original path. This isolated win
has no demonstrated Messages eligibility, Linux or whole-app benefit. The
narrow prototype also remains unselected. The first setup failure (cached
LLVM22 bitcode sent to Apple's LLVM21 linker without rustc LTO) and subsequent
actual AA mismatch are preserved; thin LTO fixed only the link setup.

A separate diagnostic keeps the actual Linux full10,000/32 workload, current
rendering and acceptance unchanged. The original outer Fill timer is divided
into five exclusive geometry/clip categories. Original method indices stay;
legacy Fill must be zero, and the version2 reader requires all16 fields.
Integer geometry means opaque, identity-transform, finite positive exact integer
endpoints/limited dimensions; it does not certify mask bytes or fast-path
eligibility. Five extra count/time pairs cost20,480 fixed TLS bytes; category
calculation lies before the method clock and remains in the parent remainder.
Existing caps, two clocks per outer call and zero per-glyph records remain.
12 Python and10 actual-recorder/scalar Rust checks pass before the build.

The single optimized actual-Linux product`0c6e0ea4` builds in42.050s and
passes the original workload once. Raw stderr equals4,779 journal events;
1,138 turns/14,937 spans close196 full picture/Paint chains, maximum66 span
rows/turn, no missing or flagged fill aggregates. All196 Paints contain
833 unmasked rectangle,2,940 unmasked rounded and1,774 masked rounded calls;
**zero masked rectangles of either geometry class**. Thus the only exact-pixel
shortcut retained by the isolated prototype has no eligible calls in this run
and is not integrated.

All36 handlers and18 exact±40 wheels are inside the three fixed2s prefixes.
Eight command-free timers advance0→8 without drain credit. Both full10,000-row
History9 values (initial0/final17) and allfour2,359,296-byte RGB buffers equal
§8.146's diagnostic exactly; compared with§8.145's older final16, only setup
RGBs are an equal-revision comparison. The final picture196/fence4779 is
complete. This functional agreement does not pair the two diagnostics' timing.

| Silent eight Paints: fill category | Calls | Per-Paint aggregate median, ms | Sum, ms | Share of summed Fill |
|---|---:|---:|---:|---:|
| Masked rounded |48|0.955191|7.628028|63.647%|
| Unmasked rectangle |32|0.435127|3.400098|28.370%|
| Unmasked rounded |120|0.128481|0.956798|7.983%|
| Masked rectangle, both classes |0|0|0|0%|

Whole Fill median is1.506111ms; whole Paint median3.760954ms and maximum
4.775770ms. Text aggregate median1.883362ms remains separate. Category medians
are not additive and describe per-Paint aggregates, not individual shape calls.
Six of eight loaded complete timer turns paint; their masked-rounded sum is
6.201359ms,64.065% of summed Fill. One idle timer paints; recovery timers do not.
Whole silent timer median9.803542ms/max12.382469ms and6/8 misses remain;
loaded whole median9.368477ms/max11.983176ms also has6/8 misses. All-phase
idle/load/recovery whole maxima are8.823996/11.983176/7.748492ms. No A/B gain,
CPU attribution, observer-free cost or physical120Hz is established.

The measured fill priority is now masked rounded geometry, not the synthetic
integer-rectangle win. The next source-sized candidate is conservative rejection
of fully clipped fills using the existing coverage bounds, preserving the exact
masked AA path for every visible shape. The present data do not count how many
rounded fills are fully clipped, and establish no culling benefit or cache policy.

Source/binding/capture preserve2,006 source paths (including23 links), the Reusable factory,
unchanged entry/plan/compat and exact fonts/external inputs. The first local
launch setup omits creation of the caller-owned output directory and fails
writing preflight/terminal metadata before transport/native Popen. Creating that
exact directory permits the unchanged sealed command; there is one actual
runtime attempt, no native retry or source correction. All driver/outer exits
are0; native−15 is deliberate post-observation cleanup. Release`bc85f47e` proves
six runtime PIDs/four groups absent and5941 free.

The rejected prototype is frozen at MAIN`target/fill-mask-probe-v1`
(`2238d45b`). Diagnostic source`target/linux-fill-category-source-v1`
(`ed1b1445`) and build binding`target/messages-fill-category-ready-v1`
(`9a22ddca`) remain separate. Private ink-pair
`messages-fill-category-build-v1` capture`2b2aca04` and
`messages-fill-category-runtime-v1` capture`7706a7dd` preserve actual products
and raw evidence. MAIN`target/fill-category-evaluation` holds the independent
reconstruction, full equality checks and category tables. No rendering
optimization is shipped in this increment; the native SplitFacts reference
budget remains exhausted at3/3.

### 8.148 Invisible-fill candidate: exact pixels, mixed native timing

A bounded Raster candidate keeps the original rounded path and masked AA draw
for every visible or uncertain fill. With an identity device transform, limited
finite coordinates and matching mask/target dimensions, it rejects only a path
whose control-point bounds, padded by two pixels, are strictly outside the
existing conservative clip bound. It introduces no cache or picture/input owner.
This candidate remains **unselected** after two fresh actual-Linux pairs; the
production renderer is restored unchanged.

Six tests compare every RGBA byte with the original masked `fill_path`, including
fractional rounded edges, all four invisible sides, touching bounds, cached and
nested clips, empty/invalid clips, opacity, transforms and large-domain fallbacks.
The identical baseline tests have three intended call-count failures and three
passing controls; the candidate passes all six. Across their129 calls,60 skip
tiny-skia and69 preserve the original draw. These are fixture counts, not native
workload eligibility. The actual diagnostic still counts outer Fill invocations;
it does not record which calls the new predicate rejects.

One optimized treatment build takes42.660s. Its2,006 source paths, including23
links, differ from control only in Raster. Entry/plan/compat, Reusable factory,
fonts/external inputs, observer, driver, workload and caps remain equal. Reuse of
control product`0c6e0ea4` is paired with **fresh** cells, never old timings;
treatment is`4ca20f1a`. Run order is control→treatment, then treatment→control,
with no rebuild or retry. Each retains full10,000 records,32 changed rows,
three fixed2s input windows and the existing command-free interval.

| Loaded interval; values control→treatment | First order | Reverse order |
|---|---:|---:|
| Complete timer turns |8→8|8→8|
| Timer turns with Paint |7→7|7→6|
| Masked-rounded per-Paint aggregate median, ms |0.973171→0.994169|1.163423→0.719315|
| Whole timer median, ms |9.572787→9.177139|10.234998→6.454421|
| Whole timer maximum, ms |9.920496→12.042129|12.255547→10.206956|
| Whole timer misses over8.33ms |7→7|7→3|

Every selected loaded Paint invokes six masked-rounded fills and49 text calls,
but identical invocation counts do not prove identical pixel work or history
progress. The reverse-only loaded result does not establish a repeatable effect.
Silent whole medians are12.264505→11.132751ms and11.582211→11.152960ms;
misses are7/7→8/8 and8/8→7/8. The reverse treatment maximum worsens from
12.994175 to13.926304ms. The first control has seven in-prefix advancing timers
at revisions1→8; all other cells have eight at0→8. Drain is excluded. These
instrumented wall measurements establish neither CPU cost nor physical120Hz.

Allfour cells pass the original functional acceptance:144 in-window handlers,
72 actual signed40-unit wheels, complete final fences and unchanged first9,968
rows. All initial fullHistory9 values and three setup RGB buffers match. The
reverse pair's full finalHistory9 at revision16 and allfour2,359,296-byte RGB
buffers are exact. First-pair final revisions17/16 differ; those final pixels
are not presented as an equal-revision comparison. All24 recorded runtime PIDs
and16 groups retire; native−15 is intentional post-observation cleanup, separate
from driver/outer0. No runtime is repeated to improve a result.

The original broader host test run has322PASS/1FAIL/1ignored. Its unrelated
image-worker test confuses reserved delivery cells with completed answers.
The selected **test-only** correction waits for actual asynchronous admission,
asserts ready0/reserved2 while A is blocked, then ready2/running0 before B starts,
and preserves both undrained answers while B loads its two images. The first
correction's premature reserved2 assertion fails0vs2 and remains recorded;
waiting for admission fixes that race without changing production or workload.
The corrected test passes; strict host all-targets Clippy, scoped format, caps
and boot pass. No single all-green full-suite run is claimed for this experiment.

Two pre-compilation locked-dependency refusals are retained: the sibling ibex
pins ureq3.4.0 while MAIN locks3.4.2, and the historical diagnostic lock also has
a ring edge absent from the ordinary host. Only the isolated validation lock is
normalized; MAIN's lock is unchanged. A local build-release receipt schema error
is corrected after the successful build. Offline reader setup errors (an extra
parenthesis and a hardcoded silent-start revision0) are preserved and corrected
without touching raw data, fixed prefixes, runtime acceptance or source policy.

Source/tests and the rejected candidate are preserved under MAIN
`target/invisible-fill-validation` (`7aa7bc21`); the treatment source binding is
`target/messages-invisible-fill-pair-ready-v1/treatment-build` (`c762193f`).
Private ink-pair build capture`2cb3ed5b`, first runtime captures`a29f661f`/
`2e037138`, and reverse captures`1fc08fea`/`41b01f0f` retain actual products and
raw data. MAIN`target/invisible-fill-evaluation` holds independent reconstruction
and selection. Masked rounded fills remain a measured cost; the next discriminator
is actual eligibility and work avoided, including partially visible fills, before
selecting further clipping changes. The NativeSplit reference budget remains3/3.

### 8.149 Clipped-fill attribution: little cost in wholly invisible calls

One new diagnostic keeps the original Raster draw path and the same full10,000
records/32 changes. It classifies actual masked path bounds using the exact
§8.148 culling predicate: wouldSkip, partialBounds, containedBounds or unknown.
These are conservative geometric classes, not actual mask coverage; contained
does not prove all255 and partial does not prove visible pixels. Twenty-four
checked u64 counts/time sums add48KiB fixed TLS. The same method clock endpoints
feed both original and class totals; class time is not added twice. Classification
and serialization remain observer overhead. No mask scan, new renderer owner,
extra per-call clock, area estimate or production change is introduced.

The extracted actual recorder/classifier/fill passes15 Rust checks, including
360 path/transform comparisons with the frozen predicate,10 refused contexts,
45 complete RGBA comparisons and identity/overflow controls. The supplementary
reader passes24 checks. Its exact old version has four meaningful assertion
failures plus two controls; that baseline was executed **after** candidate
validation, not an executed tests-first sequence. Source inverses are exact.
The integrated optimized Linux build passes once in42.365s. Its2,006 physical
source entries differ from control only in the three observed files; generated
Reusable entry/plan/compat, fonts and external inputs remain equal.

The single actual run passes original functional acceptance. Raw stderr equals
4,578 journal events,892 turns and13,880 spans; all207 picture/Paint chains close,
maximum66 rows/turn. Every class partition equals its original masked category.
The full supplementary reader correctly refuses attribution:94 unknown calls
occur in two early Paint parents at journal6/11, totaling0.032998ms. The refusal
and raw data remain preserved. All fixed scoring cohorts have zero unknown;
the predeclared complete-timer subsets below are independently qualified. This
does not relabel the full capture as attribution PASS or identify the unknown
calls' cause.

| Masked-rounded class | Loaded7 Paints: calls / sum ms / share | Silent8 Paints: calls / sum ms / share |
|---|---:|---:|
| wouldSkip |14 /0.079665 /1.143%|16 /0.028708 /0.311%|
| partialBounds |14 /4.144309 /59.460%|16 /5.715230 /61.967%|
| containedBounds |14 /2.745927 /39.397%|16 /3.479054 /37.722%|

Each selected Paint has two calls in each known class. Per-Paint class medians
are0.002999/0.569710/0.382252ms loaded and0.003459/0.730690/0.442585ms silent,
in table order. These are inclusive aggregate wall times, not individual-call
medians, avoided CPU or predicted gains. The small wouldSkip share makes
whole-fill culling low priority for this workload. Most measured masked time
belongs to calls whose padded bounds overlap or lie within the clip bound.

Eight loaded complete timer turns have median8.910013ms/max11.830420ms and6/8
misses over8.33ms; the seven with Paint form the table's loaded subset. Eight
command-free turns advance0→8 with median11.601044ms/max15.955185ms and8/8
misses. All36 handlers and18 signed40-unit wheels remain inside the fixed2s
windows. Initial fullHistory9 and three setup RGB buffers match the prior run;
final revision17 versus prior16 is unequal and not a matched final-pixel claim.
The first9,968 rows are unchanged and the32 updated rows preserve their other
four fields. Final picture207/fence4578 is complete. Allsix recorded runtime
PIDs/four groups retire,5941 is free, and native−15 is deliberate cleanup after
acceptance, separate from outer0. No runtime retry, A/B gain, continuous resize,
CPU or physical120Hz claim follows.

A separate public-API vertical-band prototype is rejected before timing. It
keeps the original mask blending but draws through a shorter destination view
and copied mask. Case48 changes three RGBA bytes with crop top0: even without
path translation, altered scan clipping changes a rounded edge. The remaining
random and timing cases are UNRUN. Production stays unchanged. Future partial
fill work must preserve original scan conversion as well as masked AA.

Private ink-pair `fill-eligibility-source-v1` seal`d59a95da`, build capture
`4fca7e4e`/ELF`13ad71e0`, runtime capture`5e73d627`/seal`a1898565` and separate
cohort addendum`b63922ef` retain the exact sources, products and raw limits.
MAIN`target/fill-eligibility-evaluation` holds independent reconstruction and
`target/fill-band-probe-v1` (`b64b8d65`) the rejected prototype. A source-priced
opaque full-coverage span shortcut would require an isolated dependency patch
and exact-pixel/performance evidence; these bounds counts do not establish its
all255-mask eligibility. No dependency fork is selected. NativeSplit remains3/3.

### 8.150 Opaque masked spans: exact library controls, application benefit unknown

An isolated tiny-skia0.12 prototype adds49 lines to one internal blitter file.
Only masked SolidColor/alpha1/SourceOver/Linear/lowp rectangles qualify. The
entire existing rectangle must contain mask255 before any destination write;
parent strides and subview origins are retained. It writes the same quantized
opaque color. All AA routines, scan conversion, mask extents and original
fallback pipelines remain unchanged. Forty-four other Rust files match stock.
This is an ignored library experiment, not a selected dependency fork.

The92 Rust source files supplied to these builds match the freeze. Stock6 and candidate8
unit executions plus4 differential methods pass:12 distinct methods/18 executions.
Controls cover all source bytes and destination alpha, SIMD tails/subview strides,
all AA values, mixed/late masks, HQ/gamma/blend/shader fallbacks and float colors.
The earlier rejected AA128 mask-removal example still distinguishes2 from1.
No production/test-source correction was needed. These are library pixel controls,
not a complete application raster or physical presentation proof.

One bounded microbenchmark uses320×128 targets, four warmups and six alternating
rounds of24 draws/arm for each case/mode. Allocation and mask/path setup are outside
both clocks; mask inspection, original scanning and drawing are inside. Reset mode
also includes the full RGBA copy. Pixel equality is checked before timing and after
each round. All72 rows are retained; ratios are medians of paired round ratios,
candidate/stock, not ratios of separate medians or individual-pixel CPU times.

| Synthetic case | Reused target ratio | Reset-copy plus paint ratio |
|---|---:|---:|
| rounded path, mask255 |0.3076|0.3130|
| mask254 early refusal |0.9963|1.0081|
| last byte of each row254 |1.1179|1.1176|
| existing all-zero skip |1.0116|1.0090|
| alternating255/128 |1.0019|1.0066|
| HQ fallback |1.0026|1.0017|

The eligible case is lower in all six rounds in each mode. The late-mask refusal
is higher in all six, about11.8%; smaller fallback costs and variation remain.
This qualifies measuring actual application eligibility, not an application
speedup. §8.149's bounds classes do not identify opaque/all255 spans, and a private
dependency patch has a distribution/maintenance cost. No app gain, avoided CPU,
new120Hz evidence or production dependency change is selected.

Setup history is preserved. Python3.9 lacked the first manifest helper's digest
API. Compile round1 stopped on Bash3 empty-array/nounset expansion; round2 built
both libraries but Apple's linker rejected newer LLVM bitcode. Round3 uses equal
Rust thin-LTO flags and compiles all six outputs in11.672s. Its two unit suites
pass, then a busy-process preflight stops before the reference/benchmark launch.
A separate fresh-preflight continuation runs only those two previously unstarted
binaries once, unchanged. There is no fourth compilation, source fix or repeated
passed test; original stopped receipts remain intact. All owned groups retire.

MAIN`target/masked-opaque-span-probe-v1` preserves source seal`d588b4e3`, patch
`5a5954e7` and all three compilation attempts. `target/fill-eligibility-evaluation`
holds independent source/product readback and the separate unstarted-execution
seal`9c851c81`/raw CSV`4c823149`. NativeSplit's unrelated reference allowance stays3/3.

### 8.151 Opaque spans in actual Messages: eligible and equivalent, speed unmeasured

One new optimized Linux diagnostic binds the published tiny-skia0.12 package,
§8.150's shortcut and bounded counters to the original full10,000/32 Messages
workload. Each eligible rectangle first runs the original pipeline against its
actual incoming destination, checks every resulting RGBA pixel, writes the
candidate span and checks again. A mismatch refuses success. Eight u64 values per
Paint count rectangle calls/visits, hits, writes, misses, verified visits and
refusals. They add16KiB fixed turn storage and96B active TLS. The existing
functional reader, clocks, quotas, Raster path and workload remain unchanged.

Nine library checks, one host bridge check and16 reader checks pass after one
preserved test-only import setup failure. The optimized app build passes once
in43.230s; actual compiler artifacts bind the owned dependency and45 source
dependencies. The corrected v2 build helper ran; the later v2-final assertion
addition did not. One subsequent native cell passes functional acceptance.
Independent raw reconstruction finds4,614 events,985 turns,14,126 spans and193
complete picture/Paint chains, at most72 rows per turn. Every Paint's
owner/input/attempt and all eight counters close without flags or mismatches.

| Cohort | Paints | Eligible writes | Verified pixel visits | Mask refusals |
|---|---:|---:|---:|---:|
| Entire capture |193|62,319|45,776,418|42,689|
| Loaded fixed prefix |20|6,280|4,213,625|5,220|
| Command-free fixed prefix |8|2,512|1,620,605|2,064|

The entire capture offers105,008 rectangles/80,626,066 pixel visits. Counts include
overdraw, not unique pixels or inspected mask bytes. This is compositional proof
for actual replaced spans, not an independent full-frame replay. The separate
fill classifier still has94 unknown calls at early events6/11, totaling0.031880ms;
its full-capture attribution remains unavailable. Scored prefixes have none.

All36 input handlers and18 signed40-unit wheels remain inside fixed2s prefixes.
The qualified newer-picture wheel1342 uses actual predecessor163, not offered162.
Eight silent timers advance0→8 without input or drain credit. Initial/final full
History has10,000 rows, final revision16/changed32. Final picture193 and fence4614
close; full RGB binds that picture. Eight loaded timer turns have median9.693248ms,
maximum12.843718ms and8/8 over8.33ms; silent median11.466400ms, maximum15.014976ms
and7/8 misses. Reference rendering and verification deliberately add work: these
timings show neither gain nor regression of the uninstrumented shortcut.

All six recorded PIDs/four groups retire and port5941 is free. Native−15 is
intentional post-acceptance cleanup, separate from outer0. The preserved plan's
descriptive lock label is stale; operative source cards, compiler metadata and
compat bind actual lock`cae4ca0b`. No retry, source repair, production dependency
selection, CPU or physical120Hz claim follows. Private ink-pair build capture
`fecf5e83`/ELF`b283f5df`, runtime raw`77bc65dc`, report`fb7c4967` and seal`1a2f4339`
retain the evidence. MAIN`target/fill-eligibility-evaluation` holds independent
arithmetic. Actual application benefit remains the next required discriminator.

### 8.152 Pixel-copy expression: faster ordinary-buffer primitive, app trial pending

The existing Linux XRGB copy already compiles to vector instructions. An isolated
safe Rust expression treats each four-byte input chunk as a little-endian word,
reverses bytes, shifts by8, sets alpha255 and writes little-endian bytes. This
produces the same B/G/R/255 bytes, including nonopaque inputs. All cropping,
row strides, destination pitch, slicing and row order remain byte-identical.
There is no unsafe code, alignment assumption or architecture-specific source.

One actual AArch64 compile/check/benchmark passes. Six correctness groups cover
12,950 cases and6,447,368 aggregate compared byte pairs, including all channel
values, vector tails, source/destination guards, odd pitches, unaligned offsets,
zero extents and overlapping in-bounds destination rows. A deliberately incorrect
swizzle is detected. Invalid-length panic cases are not executed; their outer
source is unchanged. Both functions have the same probe-only noinline annotation.

| Prewarmed ordinary-buffer case | Original median µs/copy | Candidate median µs/copy | Median paired ratio |
|---|---:|---:|---:|
|1024×768|163.943344|81.055000|0.494506|
|1920×1080|268.132594|132.788250|0.495421|
|Cropped/padded/unaligned|283.633938|159.830063|0.565627|
|33 pixels|0.035480|0.030844|0.869316|
|1 pixel|0.002370|0.002024|0.850087|

Each case has six alternating-order pairs; all30 rows/52,800 timed calls remain.
Every candidate sample is lower, including the retained one-pixel last-round
ratio0.326350 outlier. Small copies include call/timing overhead. Ratios are
medians of paired totals, not quotients of separate medians. Allocation and first
touch are outside timing; these are ordinary buffers, not DRM mappings or scanout.
Do not apply these ratios to recorded application KmsCopy times.

The actual distinct symbols show a34-instruction/16-pixel original loop and a
12-instruction/8-pixel candidate loop using word reversal/shift/OR/stores. This is
code shape, not measured cycles. All owned groups/locks release without guard or
cleanup signals. No production change is selected by this primitive alone.
MAIN`target/kms-copy-expression-probe-v1` preserves input`467b575b`, ELF`50e4222d`,
raw CSV`27069f7f`, report`d80d9947` and result seal`3fe0fe69`; lead arithmetic is in
`target/fill-eligibility-evaluation`. The next trial reuses only the original app
product for a fresh baseline cell and changes one candidate copy path. Existing
RGB captures observe the source Pixmap/VNC image, not a DRM XRGB readback; byte
tests establish the swizzle contract. NativeSplit's separate allowance stays3/3.

### 8.153 Packed KMS copy: byte contract preserved, app comparison refused

The selected candidate changes only the inner expression in Linux `copy_xrgb`;
source cropping, strides, destination pitch, row order and bounds checks stay
unchanged. It writes the same B/G/R/255 bytes without unsafe or alignment-specific
access. Five permanent fixed-byte controls cover transparency, odd pitch and
unaligned guards, a17-pixel tail, zero/larger requested extents and overlapping
in-bounds destination rows. The isolated ordinary-buffer result is §8.152;
application benefit remains unestablished.

A single optimized candidate app build takes42.212s. Its2,006 source paths differ
from the original control only at the copy expression; stock tiny-skia, generated
entry/plan/compat and fonts remain equal. Fresh control13ad then candidate4eef
run once. Control passes; candidate fails the unchanged wheel reader after
observations. Offered picture167 is superseded by timer4312/ACK168, followed by
pointer completions4321/4323 with dirty=true before wheel1337/4324. The reader
refuses that uncertified live state. Actual predecessor168→169 scrolls−40, but
this partial fact does not replace acceptance. Candidate final-fence success is
not persisted. No reader weakening, retry or successful paired-gain claim follows.

Separate saved-trace arithmetic retains mixed copy wall spans. Idle/loaded/
recovery copy sums are10.135→12.148 /20.390→18.738 /11.646→11.845ms. Loaded complete
timer copy medians are1.019108→1.027880ms, despite the lower aggregate loaded-copy
median. Loaded timer whole maxima are12.273426→10.561794ms with7/8→6/8 misses;
silent cohorts contain7→8 complete turns, and full final History revisions differ
17→16. All three setup RGBs match, but final RGBs are not a matched-state comparison.
RGB observes the source Pixmap/VNC, not the DRM XRGB destination. These failed-pair
diagnostics establish neither application gain, CPU savings nor physical120Hz.

Validation compiles the complete current Linux host production library on actual
AArch64 Linux and links five unchanged test bodies against its real copy function.
All five pass; formatting and both strict Clippy checks pass. This is direct Rust
library/dispatcher execution, not Cargo/libtest or the full cfg(test) suite.
The first attempt stopped before tests on an omitted shared executor source;
the second adds only that unchanged file and passes without a production/test fix.
Nineteen pinned warm externs are reused without rebuilding dependencies. Sources,
products and recorded warm identities remain unchanged, with all owned groups
released. MAIN`target/kms-copy-existing-module-validation-v2` preserves input
`8900ecec`, library`b5c066cd` and test executable`768704a1`; v1 failure remains.
This selects the small byte-equivalent expression for its isolated throughput
result, with no application speedup claim.

Private ink-pair failure seal`928bc286`/report`9210fb8c` and separate diagnostic
seal`4f6ebb33`/report`d2aa3867` preserve both raw runs and the refusal. All12 recorded
PIDs/eight groups retire, port5941 is free; native−15 is intentional cleanup.
MAIN`target/fill-eligibility-evaluation` retains independent source/identity reads.
NativeSplit's separate reference allowance remains3/3.

### 8.154 Opaque-span application pair: correct pixels, mixed cost, fork unselected

One fresh actual-Linux control→candidate pair at `1bc430f` passes the unchanged
functional reader. Both use the landed packed copy and identical instrumentation;
only §8.150's49-line tiny-skia shortcut differs. Timed products do no shadow
rendering or per-span pixel verification. Full10,000 rows/32 updates, fixed1024×768,
three2s input phases,2s command-free production, deadlines and caps stay unchanged.
This pin precedes origin`b3d12c3`'s layout, text-flow and timer changes; it does not
measure that newer source or native resizing.

Initial/final full History9 values match across arms at revisions0/16, including
all records and UTF-8 byte counts. All four saved2,359,296-byte RGBs match exactly;
they observe CPU/Pixmap/VNC output, not DRM readback. Each arm has36 in-window
handlers/18 signed40-unit wheels with matching offers and final geometry. Loaded
fresh installed History pictures number20/21, of which18/20 are live-current at
ACK. Installed B and a newer dirty C remain separate. Both loaded cohorts advance
8→16 and both silent cohorts0→8, with69 text callbacks per advancing timer.

| Fixed phase | Paint count C/T | Paint median ms C/T | Paint sum ms C/T | Whole-turn sum ms C/T | Whole misses >8.333333ms C/T |
|---|---:|---:|---:|---:|---:|
|idle|12/12|3.046387/2.213154|36.496854/27.079232|86.265133/70.448368|0/0|
|loaded|20/21|2.891532/2.424677|53.185417/55.065635|126.334706/142.272308|3/6|
|recovery|12/12|2.838719/2.377634|33.295171/26.157310|74.328005/66.026056|0/0|

All eight loaded timer turns remain in the complete cohort: whole median
6.470213→9.224953ms, sum52.282455→70.368277ms, misses3/8→6/8. Only six control
timer turns paint, versus eight candidate turns. That separate Paint-bearing
subset has whole median8.088866→9.224953ms and Paint median3.053386→3.254033ms;
the other two control timers still make valid producer progress. Do not compare
only the lower all-Paint median as evidence of equal-work application gain.

Silence has eight Paint-bearing timer turns per arm: whole median
11.494025→9.403016ms, maximum15.825270→11.787213ms, misses8/8→5/8; Paint median
4.396517→3.177012ms and sum35.337515→24.077137ms. Idle software input-to-ACK
median worsens12.682175→18.361156ms, loaded12.258257→13.796075ms; recovery improves
16.002125→14.828246ms. Loaded ACK maxima26.747647→23.704260ms still exceed one
frame interval. These are instrumented wall observations, not CPU or physical
display latency. Whole turns, Paint and callback/copy/observer spans are nested,
not additive. Unequal turn/Paint counts and every tail remain in the raw report.

The mixed-result rule keeps the dependency fork **unselected**; no reverse is
prepared. Lower fill/Paint cost in several cohorts does not establish consistent
application benefit. The11.8% late-mask refusal regression and dependency
distribution cost from §8.150 remain. Full-capture fill attribution also retains
94 unknown early calls per arm; selected measured/silent Paints have none.
There is no robust gain, identical per-frame-work, CPU or physical120 claim.

Control immutable capture and owned-process retirement precede candidate launch.
Both outer processes exit0; native−15 is deliberate post-acceptance cleanup.
All12 recorded PIDs/eight groups retire and port5941 is free. The two23-file raw
captures total49,914,668B. Private ink-pair
`target/messages-opaque-span-current-pair-runtime-result-v1` preserves report
`4f228e71`, comparison`7a873fa3` and24-file result seal`2511834c`; original capture
seals`ada3aaae`/`56032809` remain unchanged. MAIN`target/fill-eligibility-evaluation`
holds independent native-boundary, whole-turn, complete-timer and Paint arithmetic.
Next is a fresh stock baseline on current origin, preserving its new scheduling
and complete text callback, before selecting another production optimization.

### 8.155 Pinned Mac baseline: setup actions did not establish the workload

One previously captured `1bc430f` Mac product launches under the unchanged75s
NormalClock driver and stops before primer or measured phases. Bootstrap is
100 rows/batch8/windowed=false. Three named setup taps return platform-delivery
replies for history10,000, windowing and batch32, but subsequent trees/state remain
at epoch1 with the original100/8/false configuration. The fixed full10k32/None
guard refuses the run. A delivery reply is not proof that the action handler
ran; the failure does not yet distinguish event routing from application state.
No resize poster, measured input cohort or performance result is established.

The watcher exits1 after4.037s and the app closes; all three recorded owned PIDs
and both groups retire, with no cleanup escalation. Source remains unchanged.
An earlier prelaunch attempt launched nothing: its index guard compared saved
NUL-delimited records with newline-delimited output. The corrected owner check
uses `git ls-files --stage -z` and matches the saved bytes exactly, without
normalization. That prelaunch failure and this one native failure are separate;
neither is rewritten as success or retried. Host free space stays above77.289GB.

MAIN`target/mac-messages-current-execution-v1` retains capture`c27b618e`, binary
`4f00d271`, failed runtime report`1250060e`, terminal/cleanup`5aca3c36` and index
correction`64dd2fdd`. This is the pinned product, not later origin`b3d12c3`.
The next discriminator is the setup delivery path; no smaller workload or
historical timing substitutes for the failed baseline.

### 8.156 Current stock Linux baseline: qualified offline pointer-turn closure

One fresh optimized actual-Linux build at `b3d12c3` uses registry tiny-skia0.12,
the current layout/text-flow/deadline changes and the existing diagnostic observer.
The private lock changes only ureq3.4.2 to3.4.0, as required by the actual local
ibex dependency; MAIN's lock is unchanged. Locked offline metadata passes after
that explicit compatibility correction. Build time is103.717s, exit0. The captured
12,392,408-byte ELF is`8180d113`, in28 files/16,098,089B, manifest`80bdc8fd`.
The receipt's crate-name map selected an opt0 build-dependency Runner; actual
verbose exact_linux rustc arguments select the separate opt3 runtime Runner.
The retained profile addendum records that distinction without rebuilding.

The one native run captures all three fixed2s phases, full10,000 rows/batch32,
36 input offers and a2s command-free producer interval, then fails acceptance
with `pointer interleaved work`. Its stale `tail-setup` report label does not
describe the actual failure stage. The original outer/driver exit1, failed report
and24-file/23,606,229B capture remain unchanged. Native SIGTERM/−15 is deliberate
post-observation cleanup; all recorded Mac/guest groups retire and5941 is free.

Raw input1347 explains the refusal. Offered picture167 is superseded by a real
producer timer and fully installed picture168 before the pointer packets. The
first Absolute input/done lands in turn760, followed by its closed ui_turn; the
second Absolute and wheel land in turn761. There is no intervening timer, paint
or state change. Both pointer states exactly equal clean ACK168, including Host
clock. Picture169 subsequently moves−40. The reader incorrectly assumed that
these separate transport packets must always occupy one input batch.

A private reader correction admits only this two-adjacent-turn pattern while
preserving the old same-turn path. Both turns, dispatch parents, flush linkage,
coordinates, clean state, complete current picture, fixed fixture certificate,
packet order and causal wheel movement remain required. Unexpected pre-wheel
work, a third turn, pending content, changed state or clock still refuse. The
same49 tests produce48 controls plus one intended old-reader behavioral RED,
then49 candidate PASS. All34 new adversarial controls assert their intended
refusal, alongside the actual saved positive and14 unchanged controls.

One separate offline replay passes full acceptance, including the original
deferred setup validation, three phase proofs, fixed-prefix silence, full
History9 snapshots, RGB checkpoints, final fence and recorded cleanup. This is
a versioned reinterpretation, not a replacement native PASS or a runtime rerun.
All36 handlers/18 signed40 wheels qualify. Eight silent timers advance without
drain credit; full histories are revision0/16 with10,000 rows and final32 changes.
Final frame193 closes through event4755 with923 UI turns/193 picture chains and
no pending picture or unfinished input. The final observer flush is unmeasured.

| Fixed phase | Complete UI turns | Whole maximum, ms | Turns above8.333333ms |
|---|---:|---:|---:|
|Idle|94|7.794487|0|
|Loaded|99|12.357211|6|
|Recovery|92|7.533652|0|

These are instrumented elapsed baseline observations, not an A/B improvement,
CPU time or physical120Hz. Loaded timers include8 advancing and26 no-change
callbacks; counting all34 as producer progress or using all-turn medians as
frame cost would be wrong. Workload and caps are unchanged. Build evidence is
MAIN`target/messages-stock-observer-b3d12c3-build-v1`; private stock-observer
worktree runtime manifest`7679d9d9` and cleanup`8b514e42` remain immutable.
The separate `target/messages-stock-observer-b3d12c3-split-reader-v1` seal
`a418a48a` binds the corrected reader`97b74663`, pure tests and full offline replay.

### 8.157 Deliver the agent's own queued AppKit mouse release

AppKit can return a queued NSEvent through a different Swift object wrapper.
Agent.tap previously posted its release, sent mouseDown, then required the
peeked release to be object-identical before sending mouseUp. A standalone real
queue probe observes different posted/peeked/dequeued objects with matching
event fields; Double timestamps also differ by sub-nanosecond representation.

The agent now allocates a negative event number and qualifies its release by
type, number, window and preserved CG nanoseconds. It peeks before taking and
revalidates the dequeued event, restoring a foreign event without dispatching
it. If tracking already consumed this click's release, no second release is
fabricated. Only synthetic agent input changes; no ordinary pointer, keyboard,
Runner, app or performance scheduling policy changes.

Three standalone real-AppKit controls pass: queued release delivered once,
foreign release preserved, and consumed release not duplicated. Six XCTest
methods are added to the existing target but remain UNRUN on this CLT-only
machine: the current ExactKit module compiles, then the test target stops at
`no such module 'XCTest'`. The failed test build and unused later commands are
retained. This is not a six-test pass or whole-workspace validation.

A separate coherent1bc/ABI6 Swift app links the unchanged captured Rust archive;
it is not the current ABI8 app. Compile and capture exit0. Its first diagnostic
stops after7 requests because a new guard omitted the existing `t=0` log prefix,
although windowing had already succeeded. That failure remains intact. A narrow
predicate correction passes20 offline controls, then one separately authorized
29-request diagnostic completes. The first click synchronously accepts windowing
at epoch2. Subsequent Count/Batch mouse and Enter inputs all reach completed
Runner dispatch, but return`Poisoned`; count100/batch8 remain. Those four refusals
are an unresolved runtime failure, not missing input delivery or a full10k32
setup pass. The poison trigger and current-ABI behavior remain to be diagnosed.

The helper and integrated source are byte-identical to the validated candidate.
The new app binary is`d82e2c6d`; the Rust archive remains`8906e5e6`. Original
evidence is under MAIN`target/mac-messages-current-execution-v1`, including
`owned-mouse-up-source-v1`, `owned-mouse-up-module-validation-v1`,
`owned-mouse-up-app-control-v1` and separate `owned-mouse-up-app-control-runtime2-v1`.
All owned groups retire, no retry or recompile occurs in the corrected runtime
relay, and host free space stays above76.37GB. No measured resize phase, native
performance benefit or120Hz claim follows from this setup repair.
