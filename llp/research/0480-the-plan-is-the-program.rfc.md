# LLP 0480: The Plan Is the Program — a clean-slate derivation of Exact's ideal middle

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Contract (compiler/IR/runtime), contract-native, Web, Exact
Native, Expose, Verification, Agent API
**Author:** Charlie Cheever / Claude (Opus 5)
**Date:** 2026-08-18
**Revised:** 2026-08-20 (clock-join sync per the 2026-08-20 holistic corpus reviews — §5 and §14 D2/D4 synced to the decided state (D14(4) taken — gated one-engine ruling; D11 pause taken); no open-decision language remains for decided rows.) 2026-08-20 (r5 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision. **D1 split** (§14): D1-package (runner package) adopted under
Track R; D1-bytes (the contiguous representation as cause) gated on
§12.1's 2×2 ablation, now **blocking** for any §6.3 promotion; the spike's
Vec-not-mmap encoding added to the attribution record. **§12.2's
adjudication record corrected against the receipts**: decision-time
INTERIM figures (32/38/0, residue 70 across 2 classes, 0/2) separated
from the later FINAL certificates (306/158/30, cluster 494, 0/12,
report-only, governing rule NOT PROJECTED-SATISFIED, dependency closure
excluded) — the finals' disposition joins 0478 D12; the going-forward
gate adopts 0485's stricter predicate **in full** (named delta incl.
Class S; 0485-stricter verdict governs) plus digest-addressed transitive
import closure. §14 gains the **W6 scope-of-record paragraph** (identical
to 0478 §W6; receipt-verified: tiers-3/3b annex authorized inside
Track R; the direct-kernel-call fast path is not) and the **two proposed
pointed 0485 amendments** (Track R start-gate as-exercised; Mode-1
collapse onto Mode 2 + the 0500 patch channel). §5's Run row stops
deciding the web default (D14(4) open; 0485 §15.5's experimental-web
pin respected). §6.5 speculation re-keyed on 0485's full identity
(instance/generation, payload fingerprint, state epoch, plan
generation, build) with unconditional mount-precompute retracted. §9.4
replay gains 0442's privacy-projection-before-export boundary with
opt-in evaluation use. §12's F1/F2 gate sentences marked historical;
§13.3 mode naming clarified; §15 OQs 1/3/10/11/12 marked
shape-resolved-downstream with residuals kept.) 2026-08-20 (r4 —
program super-refine round 1 (LLP 0478–0504
loop; codex gpt-5.6-sol@ultra + grok-4.6@xhigh, both NOT READY on r3,
convergent). Brought forward to the adjudicated program state: §14
records the decisions actually taken (F2 PASS; F1 accepted as
instrument; **D1 adopted + the 0478 W6 go, 2026-08-20 "Track R go"** —
`docs/reports/llp0485-0480-adjudications-20260819.md`); §12.2 records
the rule the adjudicator actually applied (instrument acceptance + F2 +
explicit author decision, residue dispositions drafted-awaiting-
acceptance) and adopts frozen 0485's stricter certificate predicate as
the going-forward gate; §12.1/F2 gains the representation-vs-indexed-
reconciliation attribution caveat and a 2×2 ablation obligation plus
the fixture-provenance amendment; F3/F2 gain diagnostic rows for the §6
ceilings, which are demoted to labeled hypotheses; §5's Interpret mode
collapses onto frozen 0500 D1/D2 (dev = production runner + patch
channel + introspection layer) and OQ 2 closes by pointer to 0500; §10
drops "does not contradict RFC 0478" — its asks are now dispositioned
in 0478 §8 D14; §8's material invariant is restated as
default-with-priced-escape (matching the shipped Guide policy and
0490's promoted subtrees) and the E19 quote updated to r10's
proof-shaped phrasing; F5 leaves the plan-claim matrix; §4 change 1's
byte-width claim takes 0485's slot+arena-handle formulation; §11's
current-state notes refreshed against the now-registered checks.)
2026-08-18 (r2 after super-refine round 1, 3/3 NOT READY:
thesis split into two gated claims; §12 rewritten as a registered
falsifier matrix with pre-registered thresholds and a D1 verdict rule;
§10 re-enumerated, naming the W3.1↔change-3 collision;
schema-total/instance-parametric framing; sixth closure decision;
LLP 0307 Phase B credited; inspection notes corrected; timer coalescing
demoted to open question). 2026-08-18 (r3 after super-refine round 2,
3/3 NOT READY: §12.2 rebuilt around a per-context
`PlanAdmissionCertificate` reported per route and per application,
retiring the pooled 85% rule; §12.1 pre-registers RFC 0478's
keyed-rebind workload and margin and drops the decides-everything claim;
F3/F5/F6/F7/F8 rows made fireable; §10 gains the web-speed ask against
D6 and drops the stale W6-finality claim; replay-certificate citations
corrected to `compiler/replay-certificate.ts` with its real taxonomy;
§6.3's no-GC-anywhere claim narrowed to what survives the
keyed-instance path; §6.5 gains the empty-ambient-read speculation
restriction; AX/semantics projection, localization, and shared-page
provenance named as open questions)
**Related:** LLP 0478 (Contract at Full Speed — the program this series
re-derives from scratch; 0478 remains the decision owner for W1–W6, and
§10 enumerates what this cluster asks of it), LLP 0479 (Two Lowerings and
One Plan — the plan-runner inversion this document generalizes), LLP 0481
(Contract semantic tightenings — subordinate), LLP 0482 (Parallelism
tiers — subordinate), LLP 0483 (Web on the flat plan — subordinate),
LLP 0328 (native execution tiers — the v1 "interpreter, not AOT codegen"
choice revisited), LLP 0330 (Contract semantics spec — Draft), LLP 0185
(compile to closures), LLP 0186 (signal scheduler), LLP 0188 (static
semantic graph), LLP 0381 (closed-admission emission precedent), LLP 0307
(precomputed first frame — Phase B v0 shipped), LLP 0288 (Contract is the
web production target), LLP 0331 / LLP 0332 / LLP 0333 (Exact Native
product, rollout, boundary schemas), LLP 0406 (Expose Linux pixels),
LLP 0477 (accidental reactivity — §4.2 scopes what change 2 closes),
LLP 0220 (GPUI lessons — "do not pivot"), LLP 0422 (GPU paint
composition), LLP 0442 (determinism as a system property), LLP 0080 /
`llp/contract/0082` / `0085` / `0087` (the Contract umbrella and members)

## Summary

This RFC asks a question RFC 0478 deliberately does not: **if nothing
existed, what would we build?** It is a derivation, not a proposal to
rewrite anything. Its conclusion is that the current architecture is
mostly right — the Sandwich Model, native presenters, one statically
analyzable declarative language, contract blocks, the binary protocol,
real-DOM web — and that a clean slate would change exactly **one
representation** and **five language semantics**, from which every other
answer follows.

The representation change:

> **Today the deployable artifact is an IR tree that engines walk. In the
> ideal design the deployable artifact is a flat, position-independent,
> mmap-able plan buffer that engines index into. The IR stops being a tree
> of nodes and becomes a set of relational tables — nodes, bindings,
> dependency edges, structural regions, state slots. Nothing walks it.
> Precompiled loops run over it.**

LLP 0479 §2 already proposes this, framed as the structural remedy for the
2× lowering tax, and — as of r4's record — no longer parked: the W6 go
was given 2026-08-20 and the work runs inside Accepted 0485's Track R. This
document's claim is that **the tax collapse is a side effect, not the
point.** Stated precisely, the thesis is **two separable claims with two
gates**, tested separately in §12:

- **Static totality** — every schematic fact resolved at build time — buys
  replay, impact analysis, fan-out as a number, cost budgets, and the
  region partition. Gate: §12.2.
- **The contiguous representation** — those facts as flat tables — buys
  `mmap` sharing, density, SoA/SIMD, and mechanical lowering. Gate: §12.1.

The second is worthless without the first; the first is valuable without
the second (several §9 ergonomics wins need only totality). Everything in
this series is downstream of one claim or the other, and conflating the
two was the first draft's central overclaim.

**Program state (r4/r5).** This document is no longer purely prospective:
its two gate falsifiers have adjudicated results — F2 PASS and F1
accepted as the instrument — and on that evidence the author adopted D1
and gave RFC 0478's W6 go on 2026-08-20 ("Track R go"; receipt:
`docs/reports/llp0485-0480-adjudications-20260819.md`). r5 splits what
that adoption can honestly cover (§14 D1): the **runner package**
(D1-package — the flat runner with indexed reconciliation) is adopted
and running under Track R; the **contiguous-representation claim**
(D1-bytes — that the flat, mmap-able tables are *why* it is fast)
remains gated on §12.1's 2×2 ablation, which is **blocking for any
§6.3 promotion** while non-blocking for Track R continuing behind its
flags. The §6–§9
*ceilings* remain reasoned hypotheses held to RFC 0478 §P7 — no number
here is promotable — and §12 keeps the falsifier discipline for what
remains unproven. §11 separates what the repository actually contains
from what this series proposes; frozen LLP 0485 is now the
implementation authority the destination is being built under.

---

## 1. The question and the method

The prompt: knowing what we now know, and refusing to internalize sunk
cost, what is the idealized version of Exact, given four simultaneous
goals?

- **(a)** Serve each native platform *natively* — including OS-provided
  materials like liquid glass, not imitations of them.
- **(b)** Be a zero-compromise way to build a website — as good as or
  better than Next.js, TanStack Start, Vue, Svelte.
- **(c)** Be the foundation of an operating system (Expose), where every
  kilobyte, millisecond, megabyte of RSS, and milliwatt is accounted for.
- **(d)** Have best-in-class *agentic* authoring ergonomics — better than
  React-on-web — measured in authoring tokens, wall-clock time, and
  **failures that cost another turn to correct**.

Two standing allowances: Rust authoring is acceptable where it earns its
place; something Contract-shaped (expressive, fast to edit,
live-reloadable) is preferred where it can carry the weight.

The method is deliberately adversarial to the existing codebase: derive
the design from the four goals alone, then diff against what exists. The
diff is the result — unusually small, concentrated in one place.

---

## 2. The inversion: the artifact stops being a tree

### 2.1 What is there now

`packages/exact-contract/src/runtime/interpreter.ts` states the v0 strategy
in its own header: *"the IR is the artifact and the runtime interprets it
(no codegen), so the same ComponentIR that travels as the wire format also
runs."* That was a good decision: it bought one artifact, one wire format,
sub-second hot reload, an editable IR — and why `contract-native` could
be an interpreter (LLP 0328's v1 choice) rather than a code generator.

Its cost: the artifact is a **pointer graph**. An IR tree is walked by a
`switch` per node; reactive dependencies are discovered by read-tracking
at run time (`signals.ts:732–737`); instances are `Rc<RefCell<…>>` graphs
on the native side, mirroring the TS shapes 1:1 by design.

Every ceiling in this series is a property of that representation, not of any
engine that implements it.

### 2.2 What replaces it

A **plan**: a contiguous, position-independent buffer of typed tables.

| Table | Contents |
| --- | --- |
| Slots | every state slot: stable id, type, byte offset, initial value |
| Nodes | every view node: stable id, tag, parent, attr-binding range |
| Bindings | every attr/text binding: target node+attr, expression id |
| Deps | dependency edges, topologically ordered at build time |
| Regions | structural records, one kind per construct (LLP 0481 §5.2's inventory; enumerated in §4.4) |
| Actions | opcode ranges; effect signature (reads/writes/commands) as slot bitmasks |
| Exprs | opcodes over a shared evaluator, referencing slots by index |
| Strings | interned, offset-addressed |

Three properties matter — the ones LLP 0479 §3 identifies as what GPU
programming *forces* rather than what GPU silicon *gives*:

1. **It is data, not code.** No closures at the leaves. LLP 0479 §5.1 finds
   that backend B — the compact-plan backend, the closest thing shipping to
   this idea — went the *other* way at the leaves (per-site compiled
   closures), so it is not a test of the hypothesis. A plan's leaves are
   opcodes over one evaluator.
2. **It is contiguous and position-independent.** It can be `mmap`ed
   read-only, shared between processes, and content-addressed (§7.2).
3. **It is schema-total and instance-parametric.** Every *schematic*
   fact — opcodes, templates, node sites, dependency edges, region kinds,
   effect signatures — is resolved at build time; what remains
   runtime-parametric is *instances*: active branches, keys,
   cardinalities, payload sizes. Nothing schematic is discovered by
   running.

Property 3 is what Contract's grammar already almost gives, for reasons
that had nothing to do with performance: `writes` lists, declared
`derive`s, and a closed structural-form inventory in the view grammar
(LLP 0080, 0085, 0188). LLP 0479 §2.2 makes this argument — though its
third bullet still carries the "exactly two forms" claim LLP 0481 §5.2
corrects, and the correction applies to 0479 (Draft) as much as to this
cluster. The five changes in LLP 0481 close the gap from *almost total*
to *total*.

The tables above close state, structure, and dataflow. They do **not**
close the world boundary: imports, capabilities, resources/queries/
mutations, tasks, motion, timers, window lifecycle, errors, and hydration
state have no column here. The shipped replay certificate
(`packages/exact-contract/src/compiler/replay-certificate.ts`) already
polices an overlapping boundary — its `ReplayRejectionReasonV1` taxonomy
covers imports, host inputs, async effects, live sources, ephemerals,
form state, opaque behaviors, key commands, and window lifecycle — and
it rejects imported execution outright (`IMPORT_NOT_ADMITTED_M0`). Nor
is the residue hypothetical: **141 of the 338 corpus files carry 1,079
explicit `.ts`/`.js` `use` imports** (scan of 2026-08-18). So the design
carries a **sixth closure decision**: plan modules are closed, and the
world is reached only through a typed, versioned **capability/effect
ABI** (§5's boundary role for Rust). A fixed app-independent runner
cannot run app-specific TypeScript merely because state types are
static; every behavior either lowers into the plan or is declared
through the ABI — otherwise "total plan" names an **admitted subset**,
whose per-route, per-application coverage §12.2's admission certificate
must report.

### 2.3 What "beyond the IR" actually means

The natural reading of "AOT-compile Contract further than the IR" —
*generate Rust from the IR* — is the weaker idea: codegen from a
tree-shaped IR reproduces the tree's costs in another language, and
(LLP 0479 §5.3, backend A) can be slower for reasons unrelated to
compilation being a bad idea.

The stronger idea is to make the IR into something that **does not need to
be walked**. Then interpreting it is fast (linear iteration over a flat
encoding, a `u8`-tag `match` that compiles to a jump table); compiling it
is *mechanical* (a total plan lowers without analysis); and the two cannot
diverge (the code generator is generated from the runner's opcode table,
§5).

An update becomes: evaluate dirty predicates → toggle regions → evaluate
dirty bindings → emit ops. Index arithmetic over flat tables. **No tree
walking and no tree diffing.** Keyed `each` keeps its instance
bookkeeping — create, reuse, dispose, reorder, exactly what
`contract-native/src/view.rs` does today — but against one closed template
with statically known bindings: keyed-instance management, never
reconciliation of trees (LLP 0481 §5.1).

---

## 3. What a clean slate would keep

The honest result of the derivation is that most of the architecture
survives it. Kept without change:

- **The Sandwich Model.** Platform constraints → Rust kernel + Taffy →
  platform rendering. Goal (a) requires it and goal (c) does not contradict
  it. LLP 0220's "do not pivot to GPUI" holds.
- **Native presenters own pixels.** See §8.
- **One declarative language whose grammar is statically analyzable**, with
  indentation as structure and no JSX resemblance. LLP 0477's most
  actionable finding — agents write bad Solid because JSX-that-is-not-React
  sits in an uncanny valley — makes Contract's *distinctness* an asset, not
  a barrier to sand down.
- **Contract blocks** and the three-way verification loop.
- **`writes` declarations.** Under-appreciated: LLP 0482 §3 shows they
  seed a static conflict-detection mechanism that, as far as we are aware,
  no other UI framework has.
- **The binary op protocol** — what makes LLP 0482 §6's server/device
  region placement a compiler decision rather than an architecture.
- **Real DOM on web** (LLP 0288). Do not paint the web. See LLP 0483.
- **Acto**, the conformance-corpus-as-oracle, the verification registry, the
  authority map, Facet, and the Guide-first workflow (LLP 0365).

This is not a rewrite of the vision: a change of representation plus five
semantic tightenings makes more of the existing vision reachable than the
current representation does.

---

## 4. The five semantic changes

Owned in full by **LLP 0481**; summarized here because the rest of the
document depends on them.

1. **Static types, inferred, with sum types, and no dynamic containers.**
   Every state slot has a known type and a fixed-width slot descriptor —
   scalars inline; strings, arrays, and nested objects as arena handles
   with known handle width, per Accepted 0485's slot+arena formulation
   (the *descriptor* is fixed-width, not every inline value). Instances
   become structs in an arena. `each` becomes a struct-of-arrays row arena. Authors write no
   annotations. *Already half-landed:* `native-state-schema.ts` defines a
   `ClosedStateType` as an author-time preflight for Contract Native
   admission (LLP 0333 §5.1; inventory in §11).
   The change: promote it from an *admission gate on one tier* to *the
   language's type system on every tier*.

2. **Dependency graphs are computed statically; runtime read-tracking is
   deleted.** A derive whose dependencies cannot be resolved statically is a
   compile error. The graph becomes a build-time DAG, topologically sorted
   once. *Already further landed than its header admits:*
   `semantic-graph.ts` (LLP 0188) builds import, composition, and route
   edges and implements `impactSet` (§11); `signals.ts` still tracks by
   read at run time and the runtime graph still decides. This
   change closes LLP 0477's *discovery* hazards outright and makes its
   *width* hazards visible as numbers — full scoping, including the
   slot-vs-field granularity question, in LLP 0481 §3.3.

3. **Actions become reducers: `(state, event, envSnapshot) → (state′,
   Command[])`, with a closed effect signature. No `await` in an action,
   ever.** Async re-enters as typed events dispatching other actions.
   Banning `await` alone is not enough — a synchronous body can still call
   actions, invoke capabilities, throw, or read the clock — so the change
   closes the whole effect surface, and it is a **breaking amendment of
   `llp/contract/0082` and of LLP 0330's open clause A4** (async action
   segmentation), not a promotion of an existing rule (A10 is the closed
   thenable-derive rule). Largest blast radius, largest payoff: LLP 0479
   §5.4 finds 100% of benchmarked action invocations traverse the
   await-continuation path, so the program's one committed action-latency
   signal largely measures machinery this change deletes. LLP 0481 §4 owns
   the details.

4. **View structure is fully resolved at compile time** into the node/region
   tables of §2.2 — one region kind per construct in the real inventory
   (`when`/`match`/`match-size`, `each`, `resource`/`async`/`boundary`,
   `pager`, snippet calls), not the first draft's "exactly two forms";
   sum-type matching lowers to the existing `match` (LLP 0184). LLP 0481
   §5.2 owns the corrected pricing.

5. **Contract blocks become compile-time obligations plus zero-cost runtime
   assertions, and gain a cost clause.** `derive x depends only on a, b`
   exists; add fan-out and per-update op budgets — symbolic
   (`base + rows × perRow`), per LLP 0481 §6. A screen exceeding its
   budget fails the build. **Performance becomes a contract** — precisely
   the property agents cannot self-check today, the property that costs
   correction turns.

These are the same five changes across all four goals — but all four were
evaluated through the same static-totality lens, so the lens, not the
world, may be doing the selecting. §12, not the convergence, carries the
evidentiary weight.

---

## 5. Two deployment modes, one plan (r4 — the former Interpret mode collapsed into Run per frozen LLP 0500)

| Mode | What runs | Primary use |
| --- | --- | --- |
| **Run** | one fixed runner, no codegen, no JIT | default production on native platforms — **web deliberately excepted**: production web speed is 0478 D14(4), **DECIDED 2026-08-20 (LLP 0505 r3 row 1)**: the wasm plan runner owns destination web speed, with the flip gated on corpus green + the TS-seam cutover gate (RFC 0483 §2(b)); frozen 0485 §15.5's experimental-for-F6-only framing is annotated superseded. Also **the dev loop**: per frozen LLP 0500 D1/D2, dev is this same runner plus a plan-patch channel (HMR = patch under the HotRevision spine) and an introspection layer; there is no separate dev renderer. The IR interpreter survives as a reference/oracle engine only |
| **Compile** | generated code through `rustc`, LTO'd | OS shells, widgets, watch complications, embedded |

Compile mode is the literal reading of "AOT beyond the IR," and it is a **build
flag, not a language decision**, because a total plan lowers mechanically.
Must Contract's semantics change to enable Rust AOT? Yes — but the changes
are §4's, the same ones everything else wants.

**The invariant that keeps this from recreating the 2× tax:** the
Compile-mode code generator is *generated from* the Run-mode runner's
opcode table, and the conformance corpus gates every mode. LLP 0479 §1.1
diagnoses the current tax — two hand-written walkers, one spec, drift
caught after the fact. Two modes over one plan (plus the retained oracle
interpreter) is one lowering with distinct execution strategies. If the
invariant is ever relaxed, the tax returns with an extra engine
attached, and this design is worse than the status quo.

**On Rust as an authoring language.** Rust owns the runner, presenters,
kernel, and genuinely special surfaces (compositor internals, IME
engines) — LLP 0331's governed roots. It should *not* become a second way
to write ordinary screens: two authoring languages means two ecosystems,
two agent training targets, and AOT/parallelism properties holding for
only one. Rust's boundary role is to expose **capabilities**: a typed,
versioned ABI of things Contract can call.

---

## 6. Performance ceilings

**Reasoned, not measured — and demoted to labeled hypotheses (r4).**
Each row states its mechanism so it can be attacked there. The numeric
ceilings below (sub-5 ms framework cold start, §6.3's p50 < 10 µs /
p99 < 50 µs, §6.2's ~1 MB marginal RSS) are **hypotheses, not
falsifiable targets of the §12 matrix as previously implied**: F2's bar
is beating the interpreter beyond variance and F3's is bytes/faults/
coverage, so both can pass while these numbers are false. §12's F2/F3
rows now carry non-gating *diagnostic* sub-rows (pinned wall-clock
first-frame; absolute update-latency histograms) so the hypotheses
accumulate evidence without being quotable as commitments (RFC 0478
§P7). Keep them out of any Summary sentence.

### 6.1 Cold start

The first frame becomes a **build artifact**: the bytes of the initial op
stream are emitted at build time and applied by the presenter. Credit
where due — this is not the plan's unique buy: **LLP 0307 Phase B v0 is
shipped**, already emitting the same container from the Contract IR at
build time. The plan makes that path cheaper and more complete: emission
becomes a table scan, and coverage stops depending on per-construct
lowering. One carve-out the umbrella must state (LLP 0481 §5.3 conditions
on it correctly): the frame is a pure build artifact only where initial
state is a build-time fact; the shipped corpus contains
`state nowMs = now()`, and such slots get a baked static frame plus a
first-evaluation patch — **bake-and-patch, not pure bake**.

Framework-attributable cold start → **under 5 ms**. Time-to-first-pixel
becomes dominated by OS process launch — order tens of milliseconds for
a small static binary; an unsourced placement figure, stated to name the
bottleneck, not to quantify it — which is not ours to optimize. Against
today's committed receipts (114 ms lab p50, ~224 ms release Hermes;
RFC 0478 §1.2, caveats of record), the claim is not "2× better" — it is
**"the framework leaves the critical path."**

For an always-resident Expose surface, first pixel after wake approaches the
compositor's own latency, because there is no boot to perform.

### 6.2 Memory

The metric split RFC 0478 §6.6 insists on — at-paint footprint versus steady
RSS — survives, but the *shape* changes. The plan is read-only and
`mmap`-able, so it is shared, not per-instance. Per-surface incremental cost
becomes the state arena (single-digit KB for a typical screen) plus the
platform's own view objects.

Reasoned target: **~1 MB incremental RSS per resident surface**, with runner
text and shared component plans amortized across all of them. Today's
79.8 MiB steady RSS is a *process* figure — allocator, Rust std, presenter,
per-node overhead; the clean-slate claim is about the *marginal* surface,
the figure an OS actually budgets (§7).

### 6.3 Update path

A 10,000-node tree with a five-binding dirty set is five expression
evaluations and five op encodes: hundreds of nanoseconds of arithmetic. The
floor is the shared-memory handoff and the presenter apply, not the
evaluation.

Reasoned target for the middle: **p50 < 10 µs, p99 < 50 µs**, with p99/p50
approaching 1 — stated for the path that can earn it. The precise claim,
consistent with §2.3's keyed-instance concession: **dirty-binding
evaluation over slots and columns is allocation-free**; keyed-instance
create/reuse/dispose is a separate, metered allocation path that
LLP 0481 §6's budgets must count; native modes 2 and 3 run with no
tracing GC; and the web tier is a JS runner under a collector
(LLP 0483 §2), where the claim is fewer allocations, never "no GC."
r2's "no allocator in the loop and no GC anywhere" did not survive its
own `each` correction. RFC 0478 §2.4's targets (p50 ≤ 100 µs,
p99 ≤ 1 ms) are roughly an order of magnitude conservative *for this
representation* — the right targets for an interpreter over a pointer
graph.

### 6.4 Power

Power is wakeups, touched bytes, and display. Static dependency graphs
bound an update's traffic to its declared dirty set — but the first
draft's "provably the minimum memory traffic" is withdrawn: a source-use
graph describes *uses*, not update cardinality (LLP 0477 says exactly
this of LLP 0188's `whatUses`), and dynamic `each` instances turn one
static edge into N live consumers. Traffic claims need symbolic cost
forms plus runtime metering (LLP 0481 §6).

Two honesty notes on the first draft's wakeup rule ("no wakeup not tied to
a display refresh or a real external event; idle means idle"). First, it
is **scheduler policy, not a representation property** — nothing in
today's IR forbids it, and LLP 0330 §10 already coalesces
all deadlines into one ordered queue behind one physical timeout. Second,
the stronger form — snapping timers into aligned buckets — would weaken
LLP 0330 §10's exact-deadline guarantee ("first clock millisecond at
which the consumer's predicate may differ"): an **uncounted sixth
semantic change**, demoted to an open question (§15.7), not proposed.
Power remains unmeasured (RFC 0478 Q5) and needs its own instrument.

### 6.5 Speculative op precomputation

Because a reducer transition is a pure function of `(state, event,
envSnapshot)` (§4.3), the state′ and op stream resulting from a
*hypothetical* event can be computed before the event occurs — state and
ops only; speculated commands never execute until the event is real. A
toggle can precompute its branches; a navigation its destination's
ops. The press becomes a `memcpy` of a precomputed buffer — **when the
cached entry is provably the one the real event would produce.** The r5
correction (round-2 fold): empty ambient reads are necessary but not
sufficient — a speculated buffer is reusable only under the **full
identity key frozen 0485 §13.4-class machinery supplies**: target
instance identity and generation, event payload fingerprint, state
epoch (any intervening committed transition invalidates), plan
generation/HotRevision, and runner build — with coherent snapshots,
explicit invalidation, and an atomic transition artifact on apply.
"Precompute both branches at mount" is retracted as an unconditional
rule: precomputation is a policy decision over that keyed cache, never
an identity-free memoization.

This is the mechanism behind "feels instant" rather than "is fast"; it is
sound only under change 3's reducer discipline; it costs idle cores and a
bounded cache; and it must be gated on thermal and charge state. One
restriction the first two drafts omitted: an action whose declared
ambient-read set is non-empty — the shipped corpus's `tick` reads
`now()` — speculates *wrongly*, because the environment snapshot taken
at precompute time is not the one the real event would capture. Change
3's closed effect signature is exactly what makes speculability
checkable: eligible means an empty (or provably snapshot-invariant)
declared ambient-read set; everything else waits for the real event. It
is the clearest example of the series' pattern: **a semantic tightening
buys a capability that no amount of engine optimization can.**

---

## 7. Where the value concentrates: Expose

RFC 0478 §6.6 argues the no-JS tier is close to an entry requirement for
OS-class UI. The plan sharpens that from a per-surface argument to a
*system* argument, in two ways.

### 7.1 Residency is marginal, not absolute

Lock screen, status bar, launcher, notification shade, quick settings, and
IME are always resident and numerous. The budget question is not "what does
one surface cost" but "what does the twentieth cost." §6.2's ~1 MB marginal
figure, if it holds, is the difference between a Contract-authored shell
and one hand-written in C++ or Rust.

### 7.2 Content-addressed plans

If plans are dense position-independent data and the runner is fixed, then
common components compile to **identical bytes** across applications: the 50
`.contract` component files in `packages/exact-facet-contract/src/components/`
produce the same plan pages in every app using them.

An OS can therefore hold **one** copy of those pages, `mmap`ed shared into
every surface that references them. Twenty resident surfaces cost twenty
state arenas, not twenty frameworks. This is structural, not an
optimization, with no analogue in any JS-hosted design: bytecode can be
shared; the heap objects it builds cannot.

Open tension, posed not resolved: monomorphization (LLP 0481 §2.5) and
build-time theme-token resolution both specialize plan bytes per app or
theme, pulling against identical-bytes sharing; which constructs
specialize is undecided (§15.8, LLP 0481 §9.8).

A second omission, named rather than resolved: sharing plan pages across
applications shares bytes **across trust domains**, and this document
carries no integrity/provenance model for it — who signs a shared page,
how a surface verifies the page it maps is the one its app was built
against, and what a malicious or stale shared page can reach. LLP 0482
§6 sketches a threat model for the *easier* single-app placement case;
the cross-app case is strictly harder and is owed one before §7.2 is
more than an architecture note (§15.13).

It also reshapes LLP 0331's Rust-root story as RFC 0478 §6.6 anticipates:
hand-authored Rust roots shrink to genuinely special surfaces; ordinary
shell UI keeps HMR, contract blocks, and the agent loop.

Honest boundary, inherited from RFC 0478 §D7: this document claims no
ownership of the Linux presenter and imposes no dependency on it. The tracks
meet at the protocol boundary.

---

## 8. Native presenters and the material contract

Goal (a) — real liquid glass, not an imitation — is a *language* constraint
before a presenter constraint. The Sandwich Model already puts pixels in
native hands; what is missing is that the language can still express a
pixel that contradicts the platform.

Proposed invariant — restated r4 as **default-and-priced-escape**, not
an absolute prohibition (the absolute form contradicted the shipped
Guide policy, Contract's deliberately exposed styling attrs, and frozen
0490's promoted GPU/graphics subtrees):

> **The framework never draws a control the OS provides by default, and
> contradicting the platform is always an explicit, per-surface,
> recorded opt-out — never an accident of styling.** (The shipped shape
> already exists: the control-chrome dial defaults to `system` on
> native-presenting hosts with `presentation="composed"` as the priced
> opt-out, `guide/styling/native-surfaces.md`; 0490's promoted subtrees
> are the same pattern at the paint tier.)

Concretely: a node declares **semantic role plus material intent** rather
than an appearance. Each presenter maps that to the platform's real
mechanism — `UIVisualEffectView` and liquid glass on Apple platforms, Mica
on Windows, Material You on Android, `backdrop-filter` on web. The
contract is *the OS decides*; lowest-common-denominator is the failure
mode designed against, and Facet is already the right shape.

Second consequence of §2: a screen's static structure being a build-time
fact, the presenter can create the whole view hierarchy in one batch — in
principle before app code runs at all.

One projection this cluster has not designed and must name: the
**semantics/accessibility tree**. RFC 0478 (r10) holds AX/IME as a
release-gating row — its E19, proof-shaped: the pipeline exists (native
AX props, producer-neutral presenters) but is unproven under
VoiceOver/IME on native-tier roots — and the semantics tree is also
Acto's substrate, so goals (c) and (d) both run through it. §2.2's tables carry
no AX column; where the semantics projection lives in the plan (a table
beside Nodes, an annotation on it, or presenter-derived) is open
(§15.11).

---

## 9. Ergonomics ceiling

Goal (d) has a specific objective function, worth stating plainly because
framework design does not usually optimize it:

> The dominant cost in agentic authoring is not tokens typed. It is **turns
> spent correcting.** A turn costs tens of seconds of wall clock and
> thousands of tokens. Therefore the design objective is to maximize the
> fraction of errors caught at compile time *with a message that names the
> fix*.

Six moves, in descending leverage:

1. **Every diagnostic carries a machine-applicable edit.** The agent applies
   it without spending a turn. *Partly landed:* `DiagnosticFixIt` exists in
   the IR, with ~6 emission sites, all in `analyze.ts` (the first draft's
   "18 sites" counted substring tokens). The target: a fix-it mandatory for
   every diagnostic class with a determinate repair; "no fix-it" is a
   reviewed exception.
2. **One command, one JSON output**: errors, contract verdicts, native
   eligibility, fan-out counts, estimated update cost. One tool call, whole
   picture. Today that information is spread across the compiler, vitest,
   Acto, and verify profiles — three or four turns to assemble.
3. **Contract-block-first authoring.** The agent writes the claims — short,
   dense, high-signal — and the compiler generates a view skeleton that
   satisfies them. The token asymmetry inverts: agent writes spec, machine
   writes boilerplate.
4. **Deterministic replay as the debugging primitive.** Reducer actions
   (§4.3) make a session an event log plus per-event environment
   snapshots — the hazard classes `compiler/replay-certificate.ts`
   enumerates today (§2.2) shrink structurally rather than vanish. Replay,
   bisect, emit a regression fixture. This deletes the "reproduce it on
   device" class of multi-turn failures; the same logs feed the
   conformance corpus, crash reports, and evaluation data — LLP 0442's
   thesis applied to the authoring loop — **under 0442's own privacy
   boundary (r5): privacy projection/vaulting is applied *before* any
   log is persisted or exported** (event payloads and environment
   snapshots can carry user content and secrets), crash-report and
   corpus feeds carry the privacy-projected artifact only, and
   evaluation-data use requires explicit opt-in. Admission, redaction,
   retention, and secret-handling ride the same rules as every other
   exported artifact; a replay primitive without them is a data
   exfiltration primitive.
5. **Hot reload becomes a plan patch (frozen 0500 D1) plus a computed
   state migration.** Slots
   are named and typed, so keep what matches, reset the rest, *report* what
   was reset. A reasoned target like everything in this section, not a
   commitment: sub-100 ms including native. The reset report is itself an
   agent affordance.
6. **Keep the language maximally un-React-shaped** (LLP 0477 §2.1 claim 4).

Reasoned targets: a typical screen in 40–80 lines (the 225-line
`js/src/caltrain-contract/app.contract` is a full application — already in
this class); first-try compile success high enough that the modal edit is
one turn; and **no *Contract admission or semantics* failure class that
appears only at run time on a device** (scoped r4: host/input-principal
classes like the real-input carrier split are device realities this
language cannot legislate away — the promise covers what static
admission can actually prevent).

---

## 10. Relationship to RFC 0478 — larger than a sequencing amendment

The first draft called this "one sequencing amendment"; that undersold
it, and r3's "does not contradict RFC 0478" was not true of the asks —
several were amendments. r4 states it plainly: **this section is an ask
list whose dispositions now live in RFC 0478 §8 D14** (0478 remains
decision owner for W1–W6, the ergonomics matrix, and the evidence lane).
Current dispositions of record: ask 1 **taken** (D1 adopted + W6 go,
2026-08-20); ask 3 **overtaken** (F2 ran and passed; the residual W4a
outputs are 0478 D12 risk controls); ask 4 **taken** (W3.1 pause =
0478 D11, DECIDED 2026-08-20 per LLP 0505 r3 row 2(d)); ask 6 **taken**
(web speed mechanism = 0478 D14(4), DECIDED 2026-08-20 per LLP 0505 r3
row 1 — the gated one-engine ruling); ask 2 (§10.2 timing) **open**.
The enumeration, kept for the record:

1. **D1 promotes W6.** 0478 §7's first cost item names the shared
   executable plan as "a W6 candidate, not a commitment" (there is no
   §7.1; the first draft miscited one). D1 restates that candidate as the
   architectural destination — still subordinate and gated, no longer a
   garnish. LLP 0479 §3.1 argues the annex is downstream of the plan
   runner; this document extends that across §6–§9.

2. **LLP 0481 is a language program running against W2.** W2 *is* the
   ratification of LLP 0330, and 0481 proposes five amendments to that
   document — one (change 3) breaking `llp/contract/0082` and 0330's open
   clause A4 — while W2 ratifies it. A real coordination cost; the timing
   is an open question (§15.9).

3. **D3 reaches into the pre-investment gate.** Asking for the §12.1 spike
   as a W4a precondition modifies a gate 0478 designed. The case: two of
   the three 0478 review families reached this independently (0479 §6) —
   grok, promote the shared compact-plan runner earlier as the tax
   collapse; codex, decide the shared executable plan earlier, after a
   bounded spike, not leave the only structural remedy in optional W6.
   The ratchet: the tax is paid continuously; every month accumulates
   hand-written second lowering that raises switching cost.
   0478's adjudicator decides.

4. **D2 collides with W3.1, and no document in this cluster named it.**
   0478 W3.1 promotes **async actions** and general resource/query cells
   into the native matrix; 0481 change 3 **deletes async actions from the
   language**. Every week of W3.1 async-action lowering is hand-written
   work change 3 would strand — 0479 §6's ratchet argument pointed at this
   cluster's own proposal. The recommendation is explicit: **D2 as
   proposed pauses W3.1's async-action lowering specifically** until the
   §12.2 admission audit reports; general resource/query cell work and
   the rest of W3 continue. A recommendation to 0478's owner, not a decision
   taken here.

5. **A caution about W4a's design** (revised: the first draft's "fails
   toward paying the tax forever" is stale — 0478 now lets a W6 no-go
   reopen on a contradicting capture or by author decision).
   LLP 0479 §5 argues the committed `actionBatchP95Ms` inversion is close
   to silent on the plan question: its three candidate causes are all
   JS-engine and harness effects that do not transfer to a Rust plan
   runner. If W4a answers a JavaScript question and a mixed verdict
   defaults to no-go, contamination still biases toward the tax —
   recoverably, given the reopen rule, but a reopened no-go costs the
   months a clean measurement would have saved. §12.1 is the direct
   remedy.

6. **The web ask, previously unenumerated.** §5's mode 2 is "default
   production on every platform," and LLP 0483 §2 replaces
   compiled-per-app JS with a fixed runner plus a plan — but 0478 D6's
   retained posture is real DOM **and** "web speed is TS-AOT (W1)" (its
   E14). This cluster keeps the real-DOM half of D6 untouched and asks
   0478 to treat the *speed mechanism* — W1's per-app compiled closures —
   as up for replacement by the fixed-runner-plus-plan story, gated on
   F6. r2's inventory omitted this ask entirely, and 0483's Related line
   claimed to be "the clean-slate version of that same commitment,"
   which was half true; both are corrected in r3.

---

## 11. What is true today

Holding RFC 0478's evidence discipline: repository claims below are
**inspected** (2026-08-18); everything in §2–§10 is **reasoning**.

**Inspected, and closer to this design than expected:**

- `packages/exact-contract/src/compiler/native-state-schema.ts` defines a
  `ClosedStateType` — string / number / boolean / null / never /
  string-enum / array / object with `additionalProperties: false`, plus a
  `value-kind` arm (dimension / color / duration) and an ephemeral
  action-parameter arm — as an author-time preflight for Contract Native,
  with Rust independently re-deriving it as the admission authority. The
  governing section is LLP 0333 §5.1 (state identity and fingerprint),
  which the file's `@ref` header already cites; the header's *other*
  pointer — LLP 0331 §§6.2–6.4 (capsule/compatibility/reload) — is the
  stray half to drop at source. A closed state type system already
  exists on one tier.
- `packages/exact-contract/src/compiler/semantic-graph.ts` derives a
  semantic graph (LLP 0188) over `state`/`derive`/`action`/`view` and
  related kinds. Its "later phases" header is **stale**: the file already
  builds import, composition, and route edges and implements `impactSet` —
  what is missing is authority, not construction.
- **LLP 0307 Phase B v0 is shipped**: the precomputed first frame is
  emitted as the same container *from the Contract IR at build time*.
  First-frame-from-artifact is not a plan-only capability (§6.1).
- `packages/exact-contract/src/compiler/production-program.ts` implements
  *closed admission and direct JavaScript emission* for the LLP 0381 P3/P4
  CSR programs ("without the runtime view tree," "no per-node or
  per-expression legacy fallback") — the closed-admission emission
  precedent exists.
- `packages/exact-contract/src/compiler/slots.ts` (LLP 0166 §4.2) is a
  *derived, never stored* catalog — the right pattern for every table in §2.2.
- `DiagnosticFixIt` exists, with ~6 emission sites, all in `analyze.ts`
  (the first draft's "18 sites" counted substring tokens).

**Inspected, and what the design would remove:**

- `packages/exact-contract/src/runtime/signals.ts:104, 722–737` — the
  `currentComputation` / `track()` read-tracking machinery (`observers`,
  `deps`, `readHeight`) — the runtime discovery §4.2 replaces with a
  build-time DAG.
- The two hand-written IR walkers of LLP 0479 §1.1 (`interpreter.ts`,
  `contract-native/src/interp.rs`), and the `Rc<RefCell<…>>` mirroring that
  RFC 0478 §1.3 records as an honesty note against its own dense-memory
  argument.

**Corpus for §12:** 339 `.contract` files
excluding `node_modules` (338 of them under `js/`, `packages/`,
`examples/`, `tests/`, and `scripts/`);
`js/src/caltrain-contract/app.contract` at ~226 lines; 50 `.contract`
component files in `packages/exact-facet-contract/src/components/`.

**Measured since r3:** one plan-runner figure exists — the F2 spike
receipt (plan 8.80 ms p50 / 10.02 ms p99 vs interpreter 163.0/171.8 ms
on the pinned Linux class; attribution caveat in §12.1). **Still not
measured, and load-bearing:** cross-engine update-path evidence with
tail distributions on target hardware (RFC 0478 §1.3/W4b); the 2×2
ablation isolating representation from reconciliation strategy (§12.1);
power (RFC 0478 Q5); marginal per-surface residency (§7.1).

---

## 12. The falsifiers — a registered matrix

The first draft named two experiments with no pass/fail criterion, no
adjudicator, no registered check; as written, neither could fire. r2
replaces them with **independently registered falsifiers**. Rules common
to every row: each lands as a check in
`exact-verify.json` when it first runs (r4 status: F1 and F2 are
registered and adjudicated — `plan-admission-certificate` and the
flat-plan spike bench; the rest remain unregistered); each commits a
receipt JSON under `docs/reports/`; each pins
its capture configuration; **Charlie adjudicates every verdict**; none
runs against a fixture authored *to flatter* the falsifier. Fixture
rule, amended r4 so the recorded F2 pass is in-procedure: a fixture
must implement a workload **specified by RFC 0478 W4 before this
cluster existed** (the WP-A family — small/1K/10K nodes; leaf-poke and
keyed-collection-rebind dirty classes); a purpose-built clone of that
pre-registered shape (the committed `flat_plan_spike.contract` is one)
qualifies, because the anti-gaming property lives in the workload's
provenance, not the file's authorship date. Shipped corpus and real
apps qualify a fortiori.

| # | Claim | Falsifier | Threshold / oracle | On failure |
| --- | --- | --- | --- | --- |
| F1 | Static totality / admission | `PlanAdmissionCertificate` audit (§12.2) — **ADJUDICATED 2026-08-19: accepted as instrument** (not a pass of the written rule; §12.2 records what was applied) | per-axis verdict rule below; going forward, frozen 0485's stricter predicate | changes 2/3 rejected; D1 dies |
| F2 | Update-path ceiling | Rust flat-plan spike (§12.1) — **ADJUDICATED PASS 2026-08-19/20**; attribution caveat + 2×2 ablation owed (§12.1); diagnostic sub-row: absolute update-latency histograms on the pinned class (non-gating; evidence for the §6.3 hypothesis) | plan beats interpreter p50 *and* p99 on the pre-registered keyed-rebind workload, by more than the measured variance band | representation dies; totality may survive |
| F3 | Startup (§6.1) | first-frame bytes + cold page faults vs shipped Phase B, same fixtures; diagnostic sub-row: pinned wall-clock first-frame (non-gating; evidence for the §6.1 hypothesis) | construct coverage strictly ≥ Phase B's — no parity escape on the completeness half; faults/bytes beat Phase B or parity declared | §6.1 rewritten |
| F4 | Residency (§6.2, §7.1) | marginal PSS/USS per added resident surface, ≥3 surfaces, smaps-verified sharing | ~1 MB; >3 MB kills §7.1 | §7 rewritten |
| F5 | Power (§6.4) | wakeups/sec + idle CPU under the LLP 0330 §10 queue — **scheduler check only, removed from the plan-claim matrix (r4)**: §6.4 calls it representation-independent, so it can neither confirm nor kill the plan | zero non-deadline wakeups at idle | §6.4 demoted |
| F6 | Web (LLP 0483) | one real app built four ways: plan, SvelteKit, Qwik, Next (protocol: 0483 §8.1–8.2) | pass: compressed first-load bytes ≤ the SvelteKit build AND first successful interaction no later than the best hydrated comparator; Qwik reported as the resumability reference, no bar | 0483 §2–§4 rewritten; payload claims withdrawn |
| F7 | Parallel dispatch (LLP 0482) | serializability vs the sequential oracle — monotonic enqueue order, LLP 0482 §3.3 — corpus + chaos arm; measured break-even | any divergence = stop-the-line; and at least one real-corpus workload must clear the measured break-even on a reference device, else the tier is vacuous | tiers 2/3 rejected or dormant |
| F8 | Agent turns (§9) | correction-turn count, task battery pre-registered before any run | fewer correction turns than the same battery on the current Contract stack (the comparator), at equal task success | §9 demoted |

F1 and F2 alone gated D1 (historical — both adjudicated 2026-08-19/20;
§12.2 records how, and §14 D1 records what the adoption covers and what
stays gated on the §12.1 ablation). F3–F8 fire as their
subsystems exist; registering them now makes the §6–§9 claims falsifiable
rather than rhetorical. Where a row states a numeric bar, it is a
proposal to the adjudicator, registered before the run — the same
discipline as §12.2's rule.

### 12.1 The Rust-tier flat-plan spike (F2)

A bounded spike in `contract-native`: a data-only plan — opcodes over a
shared expression evaluator, no per-site closures, no `await` path, a flat
typed encoding — measured against the existing interpreter on the update
path, tails included. This is codex's recommendation (LLP 0479 §5.6: *"a
Rust-tier question ultimately needs a Rust-tier measurement"*), and the
highest-leverage single measurement available — but r2 overstated its
reach: F2 informed the W6 go (now given, 2026-08-20) and this
document's update-path
ceiling (§6.3), and decides nothing about startup (F3), residency (F4),
power (F5), or speculation.

The workload must be pre-registered, because F2's verdict ("the
representation dies") is a *scale* verdict and the conformance corpus is
23 correctness fixtures plus a 225-line app — a leaf-poke pass over
those cannot carry it. The registered workload is therefore RFC 0478
W4's own normative shape: the WP-A family's keyed-collection-rebind
dirty class at the 1K/10K-node sizes ("a leaf poke cannot fire it"),
admissible under §12's fixture rule because 0478 specified it for W4a
before this cluster existed. Pre-registered margin, proposed with this
revision: the plan must beat the interpreter on p50 *and* p99 by more
than the measured run-to-run variance band (captured first, committed in
the receipt); a win inside the band is inconclusive, not a pass. The
conformance fixtures still run — as the equivalence oracle, and as a
kill instrument (a plan that loses even on small fixtures dies) — but
only the keyed-rebind workload can *confirm*. (The oracle's 2026-08-18
red was re-greened 2026-08-19, I3 receipt.)

**Adjudicated result and attribution caveat (r4).** F2 ran and PASSED
(receipt: `llp0485-i4-flat-plan-spike-bench.json`; adjudicated in
`llp0485-0480-adjudications-20260819.md`) — but the review record
stands: the measured delta is dominated by the old interpreter's
re-evaluated per-pass keyed reconciliation (`contract-native/src/
view.rs`, `evaluate_each_identity_rows` re-evaluating rows with
duplicate-key/position scans) against the flat runner's prepared
indexed key table (`flatplan/runtime.rs`, `seen_keys`), and a
pointer-tree interpreter could also adopt indexed reconciliation — so
the receipt demonstrates a winning *runner package*, not causally the
contiguous representation. A second attribution fact (r5, verified):
the spike's plan is a collection of `Vec`s including `Vec<String>`
(`contract-native/src/flatplan/plan.rs:292–323`) — a flat *logical*
encoding, not yet the mmap-able contiguous buffer §2.2 describes — so
the receipt is even one step further from the bytes claim. **Owed
under Track R: a 2×2 ablation** — tree/flat storage ×
linear/indexed reconciliation, otherwise-identical evaluator and
output path, plus a same-algorithm encoded-buffer comparison —
**blocking for D1-bytes and for any §6.3 promotion from this line of
evidence** (§14 D1's split; r5 upgrades it from a non-blocking
follow-up), while Track R itself continues behind its flags. Dated
with D12's risk controls.

### 12.2 The admission-certificate audit (F1) — the kill gate

r2's version of this gate measured static resolution of
derives+bindings+regions plus an await census, against a pooled 85%
threshold. That audit could not test what D1 needs. It omitted
state/value-layout admission, import and executable-context
lowerability, capability-ABI coverage, task and resource state
transitions, motion/timer/lifecycle lowering, action effect signatures,
and the full region inventory — and its numeric rule was doubly weak: a
pooled percentage over easy bindings and hard derives measures nothing,
and "the residue is concentrated" says nothing about whether the
concentrated classes are *tractable*. Both are retired.

What replaces them: one instrumented build pass emitting a versioned
**`PlanAdmissionCertificate`** per executable context — every site that
runs: derive bodies, binding and event-payload expressions, region
predicates, action and callback bodies, task bodies, resource
initializers. Each context is classified into exactly one of:

1. **plan opcode** — lowers fully into §2.2's tables;
2. **typed capability ABI** — reaches the world only through the sixth
   closure's declared surface;
3. **server-only** — never lowered on device; admissible for LLP 0482
   §6 placement and LLP 0483 without native lowering;
4. **explicit JS-or-mixed escape** — named, counted, priced; never
   silent;
5. **rejected** — no lowering and no admitted escape.

The certificate is reported **per route and per real application**
(`js/src/caltrain-contract` at minimum, plus each shipping example
app), never as one aggregate percent — an app ships routes, not
averages. Its axes are the ones r2's audit skipped: state/value-layout
admission (`ClosedStateType`, change 1), dependency resolution (change
2), import lowerability across the corpus's 1,079 `.ts`/`.js` imports
in 141 files (§2.2), capability/effect-ABI coverage, task and resource
state transitions (LLP 0481 §4.1's full closure), motion/timer/
lifecycle lowering, action invocation read/effect signatures (LLP 0482
§3.1's fail-closed shape), and the complete structural-region inventory
(LLP 0481
§5.2). The **action-await census** (LLP 0481 §4.3) rides in the same
pass and is explicitly **report-only, with no threshold**, for a stated
reason: an awaiting action is always mechanically expressible in the
event form, so no count of awaits can kill change 3 — the census prices
migration and shapes the paved form. What kills is class 5.

**Pre-registered verdict rule — set before the audit runs, so the known
hard case cannot become an escape hatch.** Proposed to the adjudicator
with this revision:

- **Pass:** at least one real shipping application classifies **every**
  context on **every** route into classes 1–4 with zero class 5; and
  each cluster-wide class-5 residue class carries a written
  disposition — lower it, ABI it, pave an alternative, or
  reject-with-fix-it — that the adjudicator accepts as tractable, with
  reasons in the receipt.
- **Fail:** some admission axis shows a residue class with no accepted
  disposition, or no real application reaches zero class 5 even as a
  costed plan.
- **Dynamic indexing counts inside the residue like any other class.**
  Being anticipated does not exempt it.

**Verdict rule for D1** (the first draft had none): F1 + F2 pass → D1
adopted as the stated destination. F1 fail → D1 dies regardless of F2, and
the series with it — totality is the load-bearing claim. F1 pass, F2
fail → the representation dies; the language changes may still proceed on
the TS tier where each pays its own way (LLP 0481 §9.7). Marginal or
mixed → D1 deferred; re-run once the residue classes have paved answers.

**Record of the adjudication actually taken (r4, corrected r5 against
the receipts; receipt:
`docs/reports/llp0485-0480-adjudications-20260819.md`).** The written
Pass rule above did not fire, and the evidence must be kept in two
dated layers rather than blended:

- **Decision-time evidence (the INTERIM certificates, before the
  adjudicator at the 2026-08-19 F1 ruling and the 2026-08-20 go):**
  hello-world clean in-scope (0 governing class-5 requirements),
  caltrain **32**, contract-app-shell **38**; cluster-wide governing
  class-5 residue **70 across 2 residue classes**; disposition
  coverage **0/2** at ruling time. The adjudicator **accepted the
  certificates as the F1 instrument** ("i think accept"), took the F2
  pass, and on 2026-08-20 **explicitly adopted D1 and gave the 0478 W6
  go ("Track R go")** — i.e. the rule applied was instrument-acceptance
  + F2 + explicit author decision, an author override of this
  section's kill-gate as written, recorded here as such rather than
  laundered into a pass. Twelve cluster residue dispositions were
  drafted at the adjudicator's request the same day
  (`llp0485-f1-residue-dispositions.md`), awaiting acceptance.
- **Final evidence (the FINAL certificates, landed after the ruling
  and materially larger):** caltrain **306** governing class-5
  requirements, contract-app-shell **158**, hello-world **30**;
  cluster-wide residue **494** across **12** residue classes,
  disposition coverage **0/12**; every final receipt is stamped
  report-only, "governing 0480-as-written rule NOT PROJECTED-
  SATISFIED," and **excludes the imported Contract library dependency
  closure** ("excluded and explicitly deferred"). The registered
  `plan-admission-certificate` check validates the interim receipts
  only — final receipts stay disabled until the I5+I6 proof
  authorities exist. The receipt's own framing says the finals
  "inherit this ruling's framing"; whether the ~7× larger final
  residue changes anything about the go is **not this document's to
  decide — it is part of 0478 D12's open disposition**, and these
  figures are recorded so that decision is taken on the final
  evidence, not the interim snapshot.

**Going forward,** the gate of record for Track L admission claims is
frozen 0485's stricter certificate predicate, adopted here **in
full** (not by "etc."): zero class-4 and class-5 contexts; 100%
executable-context coverage; deterministic app computation admitted in
class 1; class-2 capability implementations constrained per 0485 §10;
bounded, visible widening; **plus Class S (server-placed) as 0485
defines it**, and — required by the final receipts' named exclusion —
**digest-addressed transitive closure over imported Contract
dependencies** (an app's certificate is not whole-app until its
imported plans are certified by digest or independently certified and
referenced). While 0485 emits dual verdicts, **the 0485-stricter
verdict governs**; residue dispositions are a Track L *entry
condition*, not a retrospective D1 condition.

---

## 13. Costs, and what would kill this

Ranked by probability of being what actually goes wrong.

1. **Static admission is too strict** — dependency resolution above
   all. §12.2 is the gate — first to test, most likely to be fatal.
2. **Type inference is not good enough.** Closed types with poor inference
   give an agent errors it cannot fix — worse than the dynamism they
   replaced. LLP 0481 §2 owns the requirements; "authors write no
   annotations" is a hard constraint, not an aspiration.
3. **Run and Compile modes diverge** (0485's modes 2 and 3 — this
   document's §5 table keeps 0485's numbering for those two after
   Mode 1's collapse), recreating the 2× tax with an extra engine.
   Prevented only by generating the code generator from the runner's opcode
   table and gating both with the corpus (§5) — an invariant, not a
   practice.
4. **Reducer actions break too many real patterns.** Elm-shaped, so the
   prior is decent and familiarity high, but the await census inside
   §12.2's merged pass is the evidence, not the prior.
5. **Parallel dispatch determinism is under-specified**, producing
   heisenbugs — the worst failure class for agents and OS shells alike.
   LLP 0482 §3 owns the specification; it is not optional.
6. **The migration cost is real and unpriced here.** A clean-slate
   derivation is direction, not schedule. RFC 0478's W1–W5 remain the
   shipping program; nothing here authorizes a rewrite.

---

## 14. Decisions (r4 — brought to the adjudicated record; r5 — D1 split)

- **D1 — TAKEN (Charlie, 2026-08-20, "Track R go") — split r5 into what
  the evidence can carry.** The F2 receipt's own attribution (§12.1)
  says the measured win is the runner package, not isolatedly the
  bytes, so the adoption divides:
  - **D1-package — ADOPTED.** The flat *runner package* (data-only
    plan, shared evaluator, indexed keyed reconciliation, no per-site
    closures) is the architectural destination for the Contract
    middle; execution runs under Accepted LLP 0485 Track R,
    subordinate to RFC 0478's program. §12.2 records how the gate was
    actually applied.
  - **D1-bytes — GATED.** The claim that the *contiguous, mmap-able
    representation* is the causal mechanism (and everything §6.3/§7.2
    prices off it — density numbers, SoA/SIMD, identical-bytes
    sharing) stays gated on §12.1's **blocking** 2×2 ablation.
    Track R continues behind its flags either way; no §6.3 number
    promotes from the F2 line of evidence before the ablation reports.

  **W6 scope of record (2026-08-20 go — one paragraph, stated
  identically here and in RFC 0478 §W6).** The "Track R go"
  (`docs/reports/llp0485-0480-adjudications-20260819.md`) authorized:
  plan format v1 (the format-schema authority with generated readers
  and golden fixtures), IR→plan lowering, the mode-2 flat-plan runner
  behind `EXACT_PLAN_RUNNER=1`, dual-run equivalence per 0485 §14.3,
  **and the W6 research annex — tiers 3/3b per LLP 0482 (SIMD
  collection derives, parallel independent-subtree evaluation) —
  inside Track R behind its own flags and receipts** (the receipt's
  words; this is the separate annex decision 0485 §1.3 required, taken
  by the author in the same ruling). **Not authorized by that go:**
  the direct in-process kernel-call fast path (0328 D1's flagged
  optimization — skipping protocol encode/decode remains its own
  future decision, carrying 0478 Q3's evidence-stream question and
  0478 §W4's transport-neutrality rule), and the two maturity-marked
  proposal-stage seams (the PE replay machine and tier-2 dispatch),
  which the receipt keeps as non-inputs until their owner adoptions.

  **Proposed pointed LLP 0485 amendments (carried by this document —
  Accepted 0485 cannot be run past silently):**
  1. *Track R start-gate as-exercised.* 0485 §1.3 reads "F1 + F2 pass
     … A track MUST NOT begin while its gate is open." Track R began
     on instrument-acceptance + F2 + the explicit author go, with F1's
     written pass rule unfired (§12.2's record). The amendment records
     that exercised gate in 0485's own text (F1-pass is a Track L
     entry condition under the stricter predicate, not a Track R
     start condition), so the Accepted spec and the executing program
     tell one story.
  2. *Mode 1 collapsed.* 0485 §12.1's Mode-1 dev runner (plan-swap
     HMR; Design Mode on source/IR) is superseded by frozen 0500
     D1/D2 (production runner + plan-*patch* channel; overlay patches
     for Design Mode); the amendment collapses Mode 1 onto Mode 2 +
     the 0500 patch channel, matching §5 here and 0478 P3.
- **D2 — the audit ran; the pause half is TAKEN.** The certificate audit
  exists and was accepted as instrument (§12.2). Its companion
  recommendation — pause 0478 W3.1's async-action lowering — is
  RFC 0478 §8 **D11**, **DECIDED 2026-08-20 (LLP 0505 r3 row 2(d)):
  the pause is taken**.
- **D3 — overtaken by events.** The §12.1 spike ran and passed; the
  W4a-precondition question dissolved into 0478 §8 **D12** (disposition
  of the bypassed W4a outputs as Track R risk controls).
- **D4 — holds.** No change to LLP 0288's web posture, LLP 0220's
  no-pivot conclusion, RFC 0478 §1.4's GPU rejection, or LLP 0331's
  governed-root story is implied or requested. (The web *speed
  mechanism* question, 0478 §8 D14(4), is **DECIDED 2026-08-20** — the
  gated one-engine ruling, LLP 0505 r3 row 1.)

## 15. Open questions

1. *Shape resolved downstream (r5):* frozen 0485 specifies the
   dynamic-indexing escape as **cursor plus budget-enforced visible
   widening** (its §6-class rules) — pending only the W2/Track L
   adoption decision. The residual policy question is whether that
   pairing survives author contact, not what the form is.
2. *Closed r4 by frozen LLP 0500 D1/D2:* the plan is edited through
   **plan patches** (HMR unit; Design Mode live edits are overlay
   patches on the same channel, bake persisting to source); the
   LLP 0387 sidecar/slot-catalog relation rides that spine. Remaining
   detail is 0500's program RFC, not this question.
3. *Shape resolved downstream (r5):* frozen 0485 §3.3.14 answers it —
   the plan **references** motion programs as Class S separate-evaluator
   artifacts (plan-encoded motion is out of v1); RFC 0492 owns the
   evaluator's own refresh. Only an explicit future 0485 amendment
   reopens in-container motion bytes.
4. What is the plan's versioning story across a runner upgrade —
   particularly for content-addressed shared pages (§7.2) when surfaces
   update independently?
5. Is there a defensible marginal-residency measurement (§7.1) that does
   not require the full Expose shell?
6. Does mode 3 (compile) earn its place at all, or does mode 2 (run) get
   close enough that a third artifact is unjustified?
7. Should timer bucket-alignment be proposed at all (§6.4)? It would amend
   LLP 0330 §10's exact-deadline guarantee — semantic change six — and
   must be priced as one, or dropped.
8. Which constructs monomorphize and which stay polymorphic-but-shared
   (§7.2)? Both monomorphization and theme-token resolution pull against
   identical-bytes sharing.
9. When do LLP 0481's five amendments land relative to W2's ratification
   of LLP 0330 (§10.2) — before v1, as v1.x, or as a v2 track?
10. *Shape resolved downstream (r5):* frozen 0485 carries loci as an
    auxiliary-class **loci sidecar segment** (UTF-16 units, total row
    set) — spans survive the flat encoding by design. Residual: the
    fix-it/diagnostic ergonomics on top (§9), not the encoding.
11. *Shape resolved downstream (r5):* frozen 0485 §3.3.13 defines the
    **Semantics table** (the AX projection beside Nodes) — the plan
    carries the projection §2.2's r2 tables lacked. Residual: the
    VoiceOver/IME proof legs (0478 E19) and per-host mapping, not the
    plan's schema.
12. *Shape resolved downstream (r5):* frozen 0485 splits localization
    into **`localizable-strings` auxiliary catalogs** (id→bytes per
    locale, outside `planDigest`, addressed by the §3.4 manifest) with
    a localized-string invalidation channel. Residual: baked-first-
    frame and shared-page interaction policy under locale switch, not
    the string carrier.
13. What is the **integrity/provenance model for cross-app shared plan
    pages** (§7.2)? Sharing bytes across trust domains needs a signer, a
    verification story, and a threat model; LLP 0482 §6 sketches one
    only for the easier single-app placement case.
