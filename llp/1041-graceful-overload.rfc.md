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
sources after removing the test hooks. No native workload, latency comparison
or physical presentation run has tested this optimization yet. The misses and
tails in §8.48 remain the latest measured result.

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
