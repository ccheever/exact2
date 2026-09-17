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
this capture. Actual-Linux transfer/controller acceptance remains separate.
Exact seven-file sources, behavioral failures, independent review and integration
logs are retained in `target/text-transfer-validation/`.
