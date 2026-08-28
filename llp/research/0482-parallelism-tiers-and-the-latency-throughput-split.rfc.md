# LLP 0482: Five Tiers of Parallelism — and the invariant that keeps them from destroying latency

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Contract (compiler/runtime), contract-native, Kernel, Web/SSR,
Exact Native, Expose, Protocol
**Author:** Charlie Cheever / Claude (Opus 5)
**Date:** 2026-08-18
**Revised:** 2026-08-20 (r6 — LLP 0505 r3 row 8 propagation: the v1
shape is DECIDED — tier 2 ships replay-only, the 0330 inbox amendment
is declined for v1 (0330 §4 synchronous dispatch stands), tier 5 is
parked, and the inbox/census/shadow-telemetry ROI program is dormant
until a workload demands the live tier. The refusal rows remain the
product; the adopted-branch text is retained as recorded design.)
2026-08-20 (r5 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision: §3.3 now carries Accepted 0485 §7.4's admission-order table
verbatim (fixed class order + composite source-id key + per-source
ordinals) instead of owing it; tier 2's dependency on the **unadopted
LLP 0330 inbox amendment** made explicit with the reject branch (tier 2
becomes replay-only; 0330 §4's synchronous host-event dispatch is the
standing rule until amended); the ROI gate prices the universal inbox
hop on width-one real input (shadow telemetry → singleton p50/p95/p99 +
energy non-regression; failure keeps synchronous live dispatch); tier 1's
unit corrected from static Region schemas to **active-instance jobs**
(guards, parent/template, row environments, cell readiness,
document-position deps — a nested `when → each` is not two unconditional
peer jobs); §3.1 gains the ready-prefix commit clarification; the
slot-grain conflict cap named; §8 item 2 restated Track-R-build-out; the
no-other-framework uniqueness sentence dropped.)
2026-08-20 (r4 — program super-refine round 1 (grok READY /
codex NOT READY): tier 2's execution model corrected to Accepted
LLP 0485 §13.3's stronger oracle — **per-run snapshots** (each successive
conflict-free run snapshots after all preceding runs commit; never one
whole-batch pre-snapshot), per-invocation target/fence revalidation
(stale-target suppression), and **per-invocation** merge → sweep →
commit → command release in enqueue order (serial observation; the r3
batch-then-commands shape deferred command timing past later commits and
is withdrawn — 0485 §13.3 holds tier-2 implementation on this document's
owner adopting exactly these deltas); §3.3 absorbs 0485's merged
cross-family ordering key and per-source firing ordinals as the owed
priority table's shape; a **tier-2 ROI census gate** added — a
frequency-weighted batch-shape census (batch width, conflict-free run
width, reducer fraction, p95/p99 benefit, energy) precedes any observable
machinery, with correct-but-dormant an acceptable outcome; "no
scheduling overhead" scoped to no *dynamic dependency-discovery*
overhead (queueing/dispatch/join stay real, charged); RFC 0490 cited
beside 0422 as the paint-GPU authority; tier 5 notes the registered
`exact-native-transport` boundary as the starting substrate candidate;
the Related line's 0479 quote corrected to "downstream of the plan
runner".) 2026-08-18 (r2 after super-refine round 1, 3/3 NOT READY:
tier 2 repartitioned on Bernstein R/W conflict freedom; `writes` re-priced
as sound over-approximation; §1 restated as a deterministic admission
rule; worker-affinity certificate added; tier 5's missing protocol named;
§5's extra GPU uses struck; arithmetic corrected). 2026-08-18 (r3 after
super-refine round 2, 3/3 NOT READY: Summary rewritten to match §8's
totality/contiguity split — "no path to parallelism without the flat
plan" retracted; the sequential oracle re-keyed from declaration order
to monotonic enqueue order with timer/completion sequencing and a
tie-break rule; batch scheduling upgraded from pairwise Bernstein to a
precedence DAG with a contiguous-run v1; the read-mask derivability gap
conceded against `semantic-graph.ts` and the fail-closed per-invocation
signature stated; §1's determinism invariant restated as output
determinism with pinned-or-serial strategy recorded as replay metadata;
LLP 0297's single-owner commit rule threaded through tiers 2/3)
**Related:** LLP 0480 (The Plan Is the Program — the umbrella; §8 splits
which tiers are downstream of its totality claim and which of its
representation claim), LLP 0485 (flat plan implementation spec, Accepted —
its §13.3 tier-2 execution model is adopted here in r4; it holds tier-2
implementation on this document's owner ruling), LLP 0481 (Semantic
tightenings — tiers 2 and 3 require its changes 1–3), LLP 0483 (Web on the
flat plan — its §5 consumes tier 1), LLP 0478 (Contract at Full Speed —
W6's annex is tiers 3/3b; §1.4 and P6 are the GPU rejection
retained here; the W6 go was given 2026-08-20), LLP 0479 (Two Lowerings
and One Plan — §3.1's "the annex is downstream of the plan runner"), LLP 0297 (threading model — binding on
every tier here), LLP 0186 (signal scheduler — invariants parallel dispatch
must preserve), LLP 0442 (determinism as a system property), LLP 0422 (GPU
paint composition — the separate, legitimate GPU track), LLP 0220 (GPUI
lessons — do not pivot), LLP 0351 (server-generation strategy), LLP 0331 /
LLP 0406 (Exact Native and Expose)

## Summary

The question this document answers: **how far can compiled parallelism be
pushed in a UI framework, on many-core phones, on GPUs, and across machines?**

Parallelism in a UI runtime is not one opportunity with one payoff; it is
**five distinct opportunities with wildly different value**, and the two
most exciting-sounding ones — GPU compute in the update path, and threading
a discrete update — are the two worst. Conflating them is how frameworks
acquire thread pools that make p99 worse.

| Tier | What | Honest value |
| --- | --- | --- |
| 1 | Build/server fan-out over independent regions | **Very high.** Expected near-linear across cores and machines; unmeasured |
| 2 | Conflict-free (R/W-declared) concurrent event dispatch | **High and, as far as we know, unique to Contract** |
| 3 | On-device multicore for bulk work | **Real but narrow.** ≈1.4–1.5× on a fling; large on bulk data |
| 3b | SIMD inside collection derives | **Modest, cheap once the representation is right** |
| 4 | GPU compute in the middle | **Negative.** Rejected on physics, again |
| 5 | Region-level placement (server / edge / device) | **High — but only the encoding falls out; the protocol must be built** |

Every tier here is downstream of one of LLP 0480's **two** claims — and
§8 splits which, because they gate differently. Static totality
(declared dataflow, static R/W masks, the region partition) is what
tiers 1, 2, and 5 need: concurrent dispatch needs conflict masks, not
`mmap`; SSR fan-out needs the region partition, not SoA columns. The
contiguous representation is what tier 3b needs (columns to vectorize)
and what makes tier 3 safe and worth having. LLP 0479 §3.1 states the
joint form in the negative — the organizing observation of this
document:

> You cannot safely partition an `Rc<RefCell<…>>` graph across threads or
> SIMD lanes: the aliasing is dynamic, the ownership is shared and
> non-`Send`, and the independence of two subtrees is not knowable without
> running them. You *can* partition a dense plan whose independence is
> statically known.

Parallelism is not a feature to add. It is a **consequence of declared
dataflow** — and, for the on-device tiers, of a contiguous
representation. r2's "we see no path to parallelism without the flat
plan" claimed too much and is retracted: without totality, tiers 1, 2,
and 5 do not exist; without contiguity, 3b does not exist and 3 is
unsafe. That split, not a blanket dependency, is why RFC 0478's W6
annex is correctly gated.

**Nothing here is measured.** Every figure is reasoned from a stated
mechanism, per RFC 0478 §P7.

---

## 1. The invariant

State it first, because every tier below is either an application of it or a
violation to be refused:

> **The latency path is CPU, single-threaded, and measured in microseconds.
> Throughput paths — builds, bulk data, speculative precomputation, very
> large collections — may be parallel, vectorized, or GPU-resident. The two
> never mix, and no throughput optimization may be introduced into the
> update path.**

The arithmetic behind it: a discrete update in the target design is p50
under 10 µs (LLP 0480 §6.3). Thread dispatch costs roughly 5–20 µs plus
cache-coherence effects. **Parallelizing a discrete update makes it slower**,
and makes its tail worse, which is the metric that actually matters.

Two of the tiers below touch user-visible work — tier 2 dispatches live
discrete events, tier 3 runs during flings and mounts — so the invariant
must be an **admission rule**, not a slogan (the first draft's
"compile-time thresholds, never runtime adaptation" was too loose to be
one). The rule: the discrete-update path has a serial, zero-dispatch fast
path that is always taken unless a deterministic predicate proves the work
is above a certified break-even. *Eligibility* (may this region or batch
ever parallelize?) and the *cutoff constant* are compile-time facts; the
predicate compares them against runtime N, because compile time cannot
know row counts.

r2 then overstated the invariant as "the same inputs take the same path
on every machine" — which this document's own thermal gating (§4.1) and
variable worker count contradict. The invariant that holds, and the one
worth having, is **output determinism**: state′, the op stream,
exception behavior, command order, and commit order are identical to
the sequential oracle (§3.3) on every machine under every admitted
strategy. *Strategy* — path choice, worker count — is either pinned by
a per-hardware-class certificate fixed at build/install time or **fails
closed to serial**, and whichever strategy ran is recorded as replay
metadata so a session replays under the same one. Within those bounds,
thermal and charge gating may vary strategy without touching
determinism, because strategy can never affect output. What stays
refused is *feedback-driven* adaptation — tuning cutoffs from observed
timings — because it makes performance itself unpredictable: variance
in latency, even with identical outputs, is still the thing a UI
runtime exists to eliminate.

LLP 0297's threading contract binds throughout: the runtime thread owns the
app runtime, reconciler, kernel tree, and Taffy; one persistent UI worklet
runtime lives on main; no code may assume "sync call ⇒ main thread." Every
tier here is work *dispatched from* the runtime thread, never work that
relocates its responsibilities.

---

## 2. Tier 1 — build and server fan-out

### 2.1 The Quora precedent

The motivating prior art: at Quora, the component tree was split so that
separate processes could generate parts of it across six to eight
machines. The recollection is "roughly a 60% speedup," and this document
should not launder that into a factor: if it meant 60% less wall-clock it
is 2.5×; if a 1.6× rate, 1.6×. The record does not disambiguate; the prior
art establishes the *shape* of the win — real, cross-machine, from a
runtime-discovered partition — not its magnitude.

The clean-slate version should beat whichever it was, for a structural
reason: **the partition is a compile-time fact rather than a runtime
discovery.**
LLP 0481's change 2 makes each region's input set known before anything
runs, so there is no **dynamic dependency-discovery** overhead and no
risk of discovering a cross-partition read mid-render. (Queueing, worker
dispatch, serialization, joins, and segment concatenation remain real,
charged costs — "no scheduling overhead" means no runtime partition
discovery, not free dispatch.)

### 2.2 The mechanism

The compiler emits, per route, a **partition** derived from the region
schema (LLP 0481 §5.1's region records) — but the unit of parallel work
is the **active-instance job**, not the static Region record. Region
schemas are conditional and keyed: which branch of a `when` is active,
how many rows an `each` holds, and whether a cell is ready are runtime
facts, so a nested `when → each` is *not* two unconditional peer jobs
whose output is concatenated. The renderer therefore expands the schema
against initial state/data into a **job DAG** whose nodes carry: the
governing guard state, the parent/template edge, the row environment
(for keyed instances), cell/fetch readiness dependencies, and the job's
document position. Jobs whose dependencies are satisfied evaluate in
parallel; a job's output is a contiguous op or HTML span placed by its
document-position token. (A simpler admissible v1: a **non-overlapping
partition cut** — split only at unconditional, non-keyed region
boundaries, evaluating conditional/keyed interiors serially inside their
job.)

Output is a **rope of segments**, not a tree to stitch. Concatenation is
O(jobs). Segment positions are known even though segment *lengths* are
not — exactly what makes this cheap.

### 2.3 What it is worth

- **Whole-site static generation should parallelize near-linearly** across
  cores and machines (unmeasured — §7): routes are independent, and regions
  within a route are independent. The ceiling is data-fetch serialization,
  not tree structure.
- **Request-time SSR** parallelizes within a single response, bounded by the
  same data dependencies.
- **This is the mechanism under streaming SSR** — with one honest fork the
  first draft blurred: document-order flush needs no placeholders but is
  head-of-line blocked by the slowest early region; out-of-order backfill
  needs placeholders *and a client receiver*. LLP 0483 §5 owns the
  web-facing trade. LLP 0351's server-generation map is the authority for
  how this composes with existing strategies.
- **Cold builds get faster**, which is an *agent* metric: build wall clock
  is a per-turn cost.

### 2.4 Honest boundaries

- Regions that share a data fetch must be grouped, or the fan-out multiplies
  the fetch. The compiler knows the input sets, so grouping is mechanical —
  but the work must exist.
- Machine-level fan-out only pays above a per-worker fixed cost of tens of
  milliseconds; below that, cores on one box win — a scheduler heuristic
  that belongs in the build tool, not the language.

---

## 3. Tier 2 — conflict-free concurrent event dispatch

### 3.1 The claim

Contract declares, for every action, the slots it may write
(`llp/contract/0085`; `analyze.ts` A3: absent clause = empty signature; an
undeclared body write is an error).
LLP 0481 change 2 makes the *read* set a static fact as well, and change 3
closes the effect signature. Together they give every action a
compile-time **conflict signature** — reads, writes, mutations, commands —
as bitmasks over the slot table. Frameworks that discover a handler's
effects by running it (the React/Solid/Svelte/SwiftUI/Compose model)
cannot construct this object at compile time; Contract's declared
effects can.

Therefore Contract can partition a pending batch by **Bernstein's
conditions**, not by write-sets alone:

> Invocations A and B may dispatch concurrently iff
> `W(A) ∩ (R(B) ∪ W(B)) = ∅` and `W(B) ∩ (R(A) ∪ W(A)) = ∅`.
> Conflicting invocations retain the sequential oracle's order —
> monotonic **enqueue** order (§3.3). Declaration order, r2's key,
> cannot even order two invocations of the same action.

Write-disjointness by itself — the rule this document's first draft
stated — is **unsound**: in the batch `[A2: b = 7, A1: a = b + 1]` the
write-sets are disjoint, yet A1 reads what A2 writes, so concurrent
dispatch diverges from the sequential order. Read/write conflict freedom
is the classical criterion, and with static read-sets the pairwise
test is two bitmask intersections.

Two further corrections keep this tier honest, both r3.

**Pairwise is necessary, not sufficient, for a batch.** Let A write `x`;
B read `x` and write `y`; C read `y`. A and C are pairwise
conflict-free yet transitively ordered through B — grouping A and C
around B in the wrong structure reorders observable effects. The batch
therefore needs a **precedence DAG**: nodes are invocations, edges are
pairwise conflicts directed by enqueue order; a schedule is valid iff
it linearizes to the oracle, and concurrency groups are antichains of
that DAG. The v1 simplification worth naming now: dispatch the maximal
*contiguous run* of mutually conflict-free invocations at the batch
head, barrier, repeat — strictly less parallel than DAG scheduling,
trivially correct, and probably sufficient at real batch sizes (§9.1).
One latency clarification inside a run: "barrier" bounds when the *next*
run may start, not when results may land — the runtime thread MAY commit
the maximal **ready enqueue-order prefix** of the current run as results
arrive (never committing past a missing earlier result), so an early
event does not wait on its slowest peer for observability; the run's
stragglers only gate the next run's snapshot.

**The read masks are not soundly derivable today, and this document must
say so.** Credit first: `semantic-graph.ts` already emits local action
read/write edges (`buildActionEdges`, line 1146). But its statement
walker visits an assignment's **value and not its target** (line 1486):
`rows[index] = 1` records no read of `index`. And a curried handler's
arguments (`press=press(dep.id)`) are evaluated **at event time from
live instance scope** (`makeHandler`, `runtime/view.ts:6394`) — those
reads live on the view binding, not in the action's declared R set.
What this tier actually requires is a **fail-closed per-invocation
signature**: R includes binding-expression and event-payload reads (the
curried prelude); R/W compose transitively through action calls and
callback values (LLP 0481 §4.1); external conflict domains — capability
targets, navigation — and emitted commands are part of the signature;
and every invocation carries its event-sequence ID. An invocation whose
signature cannot be closed dispatches serially. Execution model —
**adopted from Accepted LLP 0485 §13.3, whose oracle is strictly
stronger than this document's r3 shape**: workers read an arena
snapshot taken at *run* start (each successive conflict-free run
snapshots **after all preceding runs commit** — never one whole-batch
pre-snapshot, which would let later runs observe stale state) and
return write-journals plus command lists; the runtime thread — the
arena's single owner per LLP 0297 — then processes each invocation
**in enqueue order, one at a time**: re-check the invocation's target
stamp and task fence (a cancel or target disposal earlier in the same
batch suppresses it as stale-target), merge its journal, run the
ordinary sweep and op capture, handle its abort-or-commit, and
**release its commands, per invocation** — never batching command
submission past later transitions. Observable behavior (intermediate op
streams, per-transition command timing, exception ordering) is
identical to sequential dispatch by construction; only reducer
evaluation parallelizes, so this tier's speedup ceiling is the
reducer-evaluation fraction.

### 3.2 What it requires

- **LLP 0481 change 3 — the full seven-part closure of its §4.1, not
  the await ban alone.** An awaiting action's effects span an unbounded
  interval — no usable signature. Nested action calls, callback values,
  and capability effects fold transitively into the signature or the
  invocation is ineligible (dispatched serially). Commands (task
  starts, navigation) are released **per invocation, in enqueue order,
  as each invocation commits** (§3.1's adopted execution model — the
  r3 collect-then-submit-after-the-batch shape is withdrawn as an
  oracle weakening). An exception aborts its action
  atomically, leaving exactly the state sequential dispatch would —
  pending 0481 §4.1's rollback mechanism for in-place mutation. RNG
  and clock reads come from each event's environment snapshot, fixed at
  enqueue, so draws cannot race. Without change 3, this tier does not
  exist.
- **LLP 0481 changes 1–2 (slot-addressed state, static read-sets).**
  Conflict signatures must be slot bitmasks for the pairwise test to be
  machine-word ops. Name the grain honestly: at **slot** grain, writes to
  `obj.x` and reads of `obj.y` on the same slot serialize — a chosen,
  sound over-approximation that caps this tier's parallelism until (and
  unless) the field/column granularity 0481 §9.3 / Accepted 0485 propose
  is adopted, at which point the masks widen to field addresses.
- **Soundness, not exactness.** The first draft claimed `writes` is exact
  today and must stay exact; both were wrong; neither is needed.
  The safety requirement is that declared sets never
  *under*-approximate — which is what `analyze.ts` enforces: an
  under-declared write is an error, an unused declared write is
  `writes_unused`, a **warning**, documented as "a sound
  over-approximation of the action's effects." Over-declaration is legal
  and merely costs parallelism: a wider mask serializes more pairs.

### 3.3 Determinism is not optional

The output must be **identical to sequential dispatch in the oracle
order**, always, on every machine, under every scheduling. Anything less
produces heisenbugs, which are the worst possible failure class for both
agent authoring and OS shells.

The oracle order is **monotonic enqueue order**, not declaration order
(which cannot order two invocations of the same action): every event
receives a monotonic sequence ID when the runtime thread enqueues it.
Timer events are sequenced when the LLP 0330 §10 deadline queue services
them; task and capability completions when their completion is delivered
to the runtime thread; a same-instant tie — one servicing pass admitting
several sources — breaks deterministically. The table is **carried here
verbatim from Accepted LLP 0485 §7.4** (one statement, two documents
agreeing, 0485's text controlling on divergence), so it is no longer
owed:

- **Admission class order per servicing pass:** (1) due timers, under
  one merged cross-family key `(dueTime, familyPriority: 0330 §4 timers
  before §10 deadlines, registration ordinal within family)` — the
  intra-family halves are 0330's own rules verbatim; (2) host input, by
  `(stable source id, per-source ordinal)`; (3) task and capability
  completions, by `(stable source id, per-source ordinal)`.
- **One source identity** — the composite key
  `(source class, declaration id, canonical instance path, scope
  generation, start-attempt ordinal)` with per-class fills: host sources
  `(host, registry id, ∅, ∅, 0)`; task sources fill all five (the
  start-attempt ordinal allocated synchronously at task start, so two
  overlapping `manual` starts are distinct sources); capability attempts
  from `(EffectAddressV1, attempt ordinal)`; due timers
  `(timer, family, global registration ordinal, ∅, firing ordinal)`.
- Every source stamps a monotonic **per-source ordinal**; the admission
  record carries both halves, so the merged key is total without a
  global clock read per event.

**Standing-semantics caveat (adoption branch, made explicit):** this
enqueue/inbox sequencing is part of 0485 §1.2's **proposed LLP 0330
amendment** — the registered Contract semantics authority today says a
host event *fires the bound action synchronously* (0330 §4).
**DECIDED (Charlie, 2026-08-20, LLP 0505 r3 row 8): the rejected
branch is v1.** The 0330 inbox amendment is **declined for v1**; live
dispatch stays synchronous (0330 §4 stands unamended) and tier 2 ships
as **replay-time parallelism only** (log segments re-executed
concurrently under the same conflict test), which preserves the
determinism spec's value without touching live event semantics. The
adopted branch's text below is retained as the recorded design should
a workload later demand the live tier; the refusal rows remain this
document's product.

The specification obligation, owed before any implementation:

- merge order is the batch's enqueue order, never completion order;
- LLP 0186's batch atomicity, glitch-freedom, and quiesce-equivalence
  invariants are preserved exactly, since they are pinned at the invariant
  level and flush order is already implementation-defined (LLP 0328 D2);
- the conformance corpus gains a parallel-dispatch arm, and the existing
  chaos/determinism instrumentation is the natural harness;
- a `--sequential` mode exists permanently, and any divergence between it
  and the parallel mode is a stop-the-line bug, never a known difference.

LLP 0480 §13.5 ranks under-specified parallel determinism as a top-five way
this series fails. This section is the mitigation and it is load-bearing.

### 3.4 What it is worth

Modest in the common case — most batches contain one event — and large in
specific ones: multi-touch, an input stream arriving alongside a timer tick,
a shell surface fielding several independent system events per frame, and
event replay during deterministic testing — where log segments may replay
concurrently only under the same §3.1 conflict test, not mere
write-disjointness, and that is still directly an agent-loop wall-clock
win.

Its greater value may be as **a proof of the thesis**: it demonstrates that
declaration-over-discovery buys capabilities that no engine optimization can.

**ROI gate (binding on implementation, not on the specification —
dormant under the 2026-08-20 row-8 decision):** with tier 2 decided
replay-only for v1, the inbox/census/shadow-telemetry program below
does **not run now** — it is deleted from the active plan and revives
only if a workload demands the live tier (which would reopen the 0330
amendment as its own decision). Recorded for that day:
before any observable inbox/worker machinery ships, run a
frequency-weighted **batch-shape census** on the conformance corpus plus
at least one real app: batch width distribution, conflict-free run
width, reducer fraction of update cost, projected p95/p99 benefit, and
energy cost of the machinery itself. Most batches are expected to
contain one event; **correct-but-dormant is an explicitly acceptable
outcome** — the determinism specification (§3.3) retains its value as
the oracle for replay-time parallelism even if the live tier never
activates. And the gate must price the machinery's cost on the case
that dominates: **a width-one real input pays the inbox hop even when
no worker ever runs** (enqueue, admission, sequencing — costs the
serial fast path of §1 does not pay today). Sequence: first **shadow
admission telemetry** (run the admission machinery observationally,
committing nothing), then a **real-input singleton gate** — p50/p95/p99
input-to-commit latency and energy non-regression against synchronous
dispatch. If the singleton gate fails, live synchronous dispatch stays,
and tier 2 ships replay-only (§3.3's rejected branch). Only past both
does the contiguous-run v1 ship, and only if the census clears the
certified break-even; the precedence-DAG scheduler is a later upgrade
decided on the same evidence (§9.1).

---

## 4. Tier 3 — on-device multicore, and 3b — SIMD

### 4.1 Where multicore actually pays

Not on discrete updates (§1). The four workloads where it does:

1. **List rebinding during a fling.** N rows × M bindings, each row
   independent by construction once `each` is an SoA row arena
   (LLP 0481 §2.1). Work-stealing over row ranges, then a single ordered op
   emission. On eight cores expect roughly 4–5× *on the evaluation portion*;
   if evaluation is ~40% of the frame, Amdahl gives
   1/(0.6 + 0.4/4…5) ≈ **1.4–1.5× end to end** (the first draft's
   "1.5–1.7×" was wrong). Real, worth having, not transformative — and
   RFC 0478 §6.4 is
   right that steady scrolling of already-bound rows is presenter work that
   neither JS nor Rust is in.
2. **Bulk data operations** feeding an `each`: sort, filter, map, group over
   10K+ items. Parallel plus vectorized. The largest on-device win, and the
   one users perceive: it is what makes a list *appear* after a query.
3. **Initial mount of a large screen** — regions in parallel, using tier 1's
   partition on device.
4. **Speculative precomputation** (LLP 0480 §6.5) on idle cores: the next
   likely route, both branches of a toggle. A latency win that costs only
   idle capacity; it must be gated on thermal state and charge, because
   goal (c) counts battery, and it is eligible only for actions with an
   empty declared ambient-read set — a clock-reading action speculates
   wrongly (LLP 0480 §6.5's restriction).

### 4.2 SIMD inside derives (3b)

Struct-of-arrays columns plus monomorphic element types plus a pinned
stdlib means a collection derive vectorizes:
`derive totals = items.map(i => i.price * i.qty)` becomes a vectorized loop
over two contiguous `f64` columns.

Expect 4–8× on the arithmetic portion. It matters for tables, charts, and
aggregate rows; it is negligible for ordinary screens. It is worth doing
only because it is cheap once LLP 0481 change 1 lands — LLP 0479 §3.1's
point: the annex is a consequence of the representation, not a project.

RFC 0478 §10's note holds with its hedge intact: **CPU SIMD likely wins
over GPU compute below ~10⁶ elements.** For UI-shaped data that is almost
always.

### 4.3 Honest boundaries

- Low-end phones have many cores but shallow thermal headroom and slow
  memory. Parallel work that thrashes cache can lose outright. Cutoffs must
  be conservative, per §1's admission rule.
- The presenter is still main-thread-bound on Apple platforms; parallel
  evaluation that produces ops faster than the presenter can apply them buys
  nothing and may cost memory.
- `Send`-ness of the data is necessary but not sufficient, and the first
  draft implied it was automatic. Today's instance scopes are
  `Rc<RefCell<…>>`, and some callbacks are host-thread-affine — the
  `docs/callback-affinity.md` discipline exists because affinity is real.
  A region is worker-eligible only under an explicit compiled
  **worker-affinity certificate**: arena state with no shared interior
  mutability, no host-affine capability calls, no main-thread-affine
  callbacks in its effect signature. Deriving it takes LLP 0481 changes
  1–2 *plus* change 3's capability affinity/purity declarations
  (0481 §4.1, closure 5); none of them make it automatic. And LLP 0297
  holds throughout: the arena has one owner, so workers compute over
  snapshots and return deltas the runtime thread commits (§3.1's
  execution model), never mutating shared state in place.

---

## 5. Tier 4 — the GPU, and why the answer is still no

RFC 0478 §1.4's rejection stands unchanged, and this document does not
relitigate it. The provenance is worth preserving: the RFC began as a
question about
GPU-first UI architectures reporting 200+ fps, and concluded that those
numbers come **less from GPU silicon than from what GPU programming
forces** — precompiled pipelines, dense buffers, static dataflow, no
per-element dynamic dispatch.

That is the entire thesis of LLP 0480. **We are taking the programming
model and declining the silicon**, and that is the correct trade for this
workload:

- a compute dispatch carries ~0.1–1 ms of fixed dispatch-plus-readback
  latency — more than an entire update should take;
- framework evaluation is small-data and branchy, which is the profile GPUs
  are worst at;
- output must land back in CPU memory as native view mutations, so readback
  is unavoidable, not an implementation detail.

**Where the GPU legitimately belongs:** paint and composition — the
presenter's business. Accepted RFC 0490 (the GPU-first-class substrate:
one spine, one frame clock, uniform/scene sinks) and LLP 0422's
paint-composition tier model are the authorities there; 0490 is a
paint/substrate unification, not compute between an input event and an
op, so nothing in it conflicts with this tier's refusal. Both meet this
program only at the protocol boundary.

Two further uses circulated in this document's first draft — GPU layout of
very large uniform collections, and batch speculative plan evaluation —
and they must be **struck, not endorsed**: both are compute in the middle
or kernel, which RFC 0478 P6 ("no GPU compute in the middle") and LLP 0480
D4 rule out in this same cluster. They stand only as future hypotheses:
either would need its own measured case brought to RFC 0478's owner as a
P6 amendment. Nothing in this series assumes or schedules them.

Rule of record: **if a proposal puts the GPU between an input event and an
op, it is wrong.** LLP 0220's do-not-pivot conclusion holds.

---

## 6. Tier 5 — region-level placement as a compiler decision

> **PARKED (Charlie, 2026-08-20, LLP 0505 r3 row 8).** Tier 5 is
> parked: no protocol, authority, or implementation work proceeds until
> a workload demands it. The section stands as the recorded design.

Possibly the most differentiating tier — and where the first draft's
"falls out rather than being built" was most wrong: what falls out is the
*encoding*; the protocol does not (owed list below).

Given: regions are independently evaluable with statically known input sets
(LLP 0481 §§3, 5); state is slot-addressed (change 1); and the
middle→presenter wire format is already **a binary op stream over shared
memory** — the same bytes over a socket.

Therefore the boundary between server-rendered and device-rendered can be
chosen **per region, by the compiler, from data locality**. A region whose
inputs live in a database is evaluated where the database is, and ships
ops; a region whose inputs are local device state evaluates on device.
Nothing about the authoring changes.

It generalizes Quora's trick from build time to run time:
**server-driven UI stops being an architecture you adopt and becomes a
placement decision the compiler makes.** The historical cost of server-driven
UI — a bespoke payload format, a bespoke client interpreter, and a second
authoring model — is exactly what the plan deletes.

Owed before this is more than an observation:

- an authority for placement policy (compiler heuristic, author annotation,
  or deployment manifest);
- the security model, since a region evaluated server-side reads server data
  and must not be able to smuggle it into a device-side region's inputs —
  LLP 0333's boundary-schema and threat-model discipline is the right frame,
  and LLP 0476's authority-native-held pattern is the right shape;
- latency policy: a remote region's ops arrive late, so every placement
  needs a declared loading state, which is the async-cell seam again
  (LLP 0481 §4.1);
- and the part the first draft elided entirely: **a distributed protocol.**
  `contract-native/src/encoder.rs` is explicit that it leaves transport
  sequencing and authentication to the caller's connection epoch — the
  bytes travel, but everything above them is unwritten for the *remote*
  case: plan/state digests and epochs, stale-result rejection,
  idempotency and retry, reconnect and resume, authn/authz, cancellation,
  backpressure, offline behavior. Not everything starts from zero: the
  registered `exact-native-transport` boundary
  (`docs/schemas/exact-native/v0/transport.schema.json`) already carries
  authenticated bounded *local* external-producer transport — framing,
  grants, sequencing, flow control, reconnect — and is the starting
  substrate candidate to evaluate before designing a parallel stack; it
  is not a remote-placement protocol, but "everything above the bytes is
  unwritten" overstated the gap.

---

## 7. How far can this actually be pushed?

Collected, with mechanisms. The first draft attached "Confidence: high" to
unmeasured claims; r2 replaces that column. Every positive row is a
hypothesis pending LLP 0480 §12's matrix; only the two refusals rest on
dispatch physics rather than on measurements not yet taken.

| Workload | Ceiling (hypothesis) | Mechanism | Status |
| --- | --- | --- | --- |
| Whole-site build | near-linear in cores × machines | static region partition, rope concatenation | unmeasured; data-fetch serialization is the real ceiling |
| Streaming SSR TTFB | first segment, not first page | document-order flush | unmeasured; head-of-line blocked by slow early regions (§2.3) |
| Bulk data → list | ~cores × SIMD width on the arithmetic | SoA columns, work-stealing, vectorized ops | unmeasured; likely memory-bound in practice |
| 10K-row fling | ≈1.4–1.5× end to end (§4.1) | parallel row rebinding; presenter unchanged | unmeasured; presenter-bound above this |
| Discrete update | **1× — do not parallelize** | dispatch cost exceeds the work | refusal — holds on physics |
| Multi-event batch | up to the conflict-free-group count | precedence-DAG groups over R/W bitmasks (§3.1) | unmeasured; depends on real batch shapes (§9.1) |
| Perceived navigation | press-to-ops ≈ a buffer copy | speculative op precomputation | hypothesis; bounded by cache and thermals |
| GPU in the middle | **negative** | dispatch + readback latency | refusal — holds on physics |

The single most important row is the one that says **1×**. A framework that
parallelizes the update path has misunderstood its own workload, and the
discipline to refuse it is worth more than any of the wins above.

---

## 8. What is owed before any of this is built

1. **LLP 0480 §12.2** — the admission-certificate audit. Without static
   dependency resolution there is no partition, and tiers 1, 2, 3, and 5
   do not exist.
2. **The Track R contiguous-representation build-out** (the F2 spike has
   run and passed — 2026-08-20; what tiers 3/3b now wait on is the real
   flat runner, on Track R's clock). Without the contiguous
   representation, tier 3b does not exist and tier 3 is unsafe.
3. **A determinism specification for tier 2** (§3.3), before any
   implementation, with a conformance arm. The LLP 0330 inbox-amendment
   branch was **decided 2026-08-20 (LLP 0505 r3 row 8): rejected for
   v1** — tier 2 is replay-only; the adopted branch's text is retained
   as the recorded design should a workload later reopen it.
4. **Deterministic admission** for every parallel path (§1):
   compile-time eligibility and cutoff constants, a runtime
   N-comparison, output determinism under every admitted strategy,
   strategy pinned per hardware class or failed closed to serial and
   recorded as replay metadata, no feedback adaptation.
5. **A thermal and charge policy** for speculation and background work
   (§4.1.4). Goal (c) counts battery, and a framework that quietly consumes
   idle cores is not OS-grade.
6. **Placement authority, threat model, and the tier-5 protocol** (§6).

## 9. Open questions

1. What are real Contract event batch shapes? Tier 2's value is entirely a
   function of how often a batch contains conflict-free actions, and nobody
   has measured it.
2. Is there a work-stealing scheduler that is deterministic *and* fast
   enough for tier 3.1's frame budget, or does determinism force static
   range partitioning?
3. What is the correct speculation cache eviction policy, and how is its
   memory counted against the residency budgets of LLP 0480 §7?
4. What is the authority document for the tier-5 protocol (§6) — a new
   LLP, or an extension of `encoder.rs`'s connection-epoch model — and
   does the wire need a distinct plan encoding (versioning, partial trust)?
5. Does tier 3b's SIMD survive contact with real derive bodies, or do
   ternaries and string operations dominate collection derives in the actual
   corpus?
