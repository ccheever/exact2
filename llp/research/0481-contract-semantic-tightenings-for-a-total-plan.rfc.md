# LLP 0481: Five Semantic Tightenings — what Contract must give up to become a total plan

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Contract (grammar/compiler/semantics/runtime), contract-native,
Verification, Agent API
**Author:** Charlie Cheever / Claude (Opus 5)
**Date:** 2026-08-18
**Revised:** 2026-08-25 (W2 decision annotations — §3.5's blocking clause and §9 Q1 marked DECIDED at the W2 venue: RFC 0518 Rulings A + B adopted verbatim by Charlie Cheever, 2026-08-25 (via exact-9e), through the LLP 0508 §14 "Dynamic Ceiling" amendment; dated record edits only, no normative change here — the ruling text lives in 0518/0508.) 2026-08-20 (r5 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision: §8 unchained from F2 (Track L runs on F1 alone per Accepted
0485 §1.3 — the spike has run and passed and was never a language gate);
§3.5's pure-derive conditional-union framing corrected — unioning can
create **false SCC cycles**, not merely extra recomputation
(`issues/20260820-llp0485-conditional-union-false-scc-erratum.md`), so
change 2 must pick guarded/predicate-labelled edges or
rejection-with-pricing for mutually conditional derives; the §4.3
line-count criterion demoted to a diagnostic with migration-evidence
gates; dynamic indexing sharpened as blocking for coherent totality;
Summary's stale 0479 accusation and Draft status fixed; §5.2/§5.4
aligned with 0501's six Region kinds; §9.2 gains the prop-annotation
plurality cost.) 2026-08-20 (r4 — program super-refine round 1 (grok READY /
codex NOT READY): §2.3's allocation-free claim narrowed to Accepted
LLP 0485 §8.2's formulation (scalar/handle dirty-binding evaluation that
constructs no new values allocates nothing — a reasoned target, not a
conformance MUST; keyed-instance and value-constructing bindings are the
metered paths); §6.1's derive fan-out split into schema-edge width (a
build-time number) vs live fan-out (symbolic, `each`-cardinality-shaped);
§2.4 now prices undefined-elimination inside change 1 (0485 §5.1 bound
it there: `??`/optional access/interpolation/OOB indexing lose their
`undefined` arms; the operators survive restated); §4.4's rollback
question now cites 0485 §7.5's A-VAL + ordered (address, before, after)
journal as the proposed resolution; the change-3 ↔ 0478 W3.1
async-action collision now carries 0480 §10.4's pause recommendation
explicitly; §9 items 1/3/9 restated as "0485 proposes an answer, gated
on W2 adoption" (cursor + budget-enforced widening; field/column
granularity; guarded resource edges) rather than unchosen forks; §4.3
gains the visually-distinct paved-form recommendation. Core owner
decisions — dynamic indexing, dependency granularity, generics — remain
explicitly W2's, with 0485's candidates named.) 2026-08-18 (r2 after super-refine round 1, 3/3 NOT READY:
view-form inventory corrected against `ir.ts`; change 3 restated as a
reducer with a closed effect signature (breaking amendment of
`llp/contract/0082` and LLP 0330 A4); `each` reframed schema-total /
instance-parametric; LLP 0477 closure scoped; inspection notes fixed;
corpus audits merged). 2026-08-18 (r3 after super-refine round 2, 3/3
NOT READY: change 3's closure inventory completed — loops (r2's "no
loop construct" was false against `ir.ts`), tasks, resource settlement,
callbacks, capability affinity, exception rollback, imported calls —
and its amendment surface widened beyond the async clause; §3.5 prices
conditional-dependency union as a resource-identity amendment, not a
precision loss; §2.3's no-GC claim narrowed to survive §5.1's own
keyed-instance concession; §3.2 records the semantic-graph walker's
read-mask gap alongside its real credit; the kill gate re-pointed at
LLP 0480 §12.2's admission certificate; replay-certificate citations
corrected to `compiler/replay-certificate.ts`)
**Related:** LLP 0480 (The Plan Is the Program — the umbrella; this
document is subordinate), LLP 0330 (Contract semantics spec v0 — Draft;
the document every change here would amend), LLP 0482 (Parallelism tiers —
consumes changes 1, 2, and 3), LLP 0483 (Web on the flat plan — consumes
change 3), LLP 0478 (Contract at Full Speed), LLP 0479 (Two Lowerings and
One Plan), LLP 0477 (Accidental reactivity — change 2 closes its discovery
hazards, §3.3), LLP 0184 (`match`), LLP 0080 (Contract umbrella),
`llp/contract/0082` (runtime and reactivity), `llp/contract/0085`
(verification and component contracts), `llp/contract/0087` (expression
language), `llp/contract/0083` (composition and shared state), LLP 0185
(compile to closures), LLP 0186 (signal scheduler), LLP 0188 (static
semantic graph), LLP 0206 (purity inference), LLP 0183 (semantic import
boundaries), LLP 0259 / LLP 0263 (async cells vs thenable derive),
LLP 0331 / LLP 0333 (Exact Native state schema and boundary threat model),
LLP 0166 (editability substrate), LLP 0442 (determinism as a system
property)

## Summary

LLP 0480 argues that Exact's ideal middle is a flat, total, `mmap`-able
**plan** rather than an IR tree — as **two separable claims**: static
totality buys replay, impact analysis, fan-out as a number, and the
region partition; the contiguous representation buys sharing, density,
and SIMD (LLP 0480 Summary; LLP 0482 §8 splits which tiers hang off
which). A plan can only be total if the language is total. This document
names the five semantic changes that close the gap and **prices each one
honestly**, because each narrows what an author may write.

The five:

| # | Change | Primary purchase |
| --- | --- | --- |
| 1 | Closed static types, inferred, with sum types; no dynamic containers | dense memory, allocation-free updates (§2.3), SIMD, better diagnostics |
| 2 | Static dependency graphs; delete runtime read-tracking | partitionability, fan-out as a build number, LLP 0477 closure |
| 3 | Actions become reducers: synchronous, closed effect signature | replay, speculation, safe concurrent dispatch |
| 4 | View structure fully resolved at build time | closed region schema; instance bookkeeping, not tree diffing |
| 5 | Contract blocks as compile-time obligations, plus cost claims | performance becomes machine-checked |

Two framing points.

**First: Contract is already most of the way here, for reasons unrelated
to performance.** `writes` lists, declared `derive`s, and a closed
structural-form inventory in the view grammar (§5.2) were chosen for agent
legibility and static analyzability (LLP 0080, 0085, 0188). LLP 0479 §2.2
makes the crux observation — *the shape of an update is already close to
a compile-time fact* — and (as of its r2, Review) now carries the
corrected closed-inventory form of its third bullet, matching §5.2 here.
These five changes are a completion, not a
redesign; two are partly implemented on one tier already (§2.1, §3.1).

**Second: every change here is a narrowing, and narrowings have victims.**
Each change carries a "What this costs" subsection that is not decorative.
A language that rejects more programs is only better if the rejected
programs have a paved alternative at least as short to write; where that
does not exist yet, this document says so.

**Status of the argument:** reasoning, not measurement. LLP 0480 §12.2 —
the **plan-admission certificate**: one instrumented pass classifying
every executable context in the corpus (scope stated in LLP 0480 §11) as
plan-opcode, capability-ABI, server-only, explicit escape, or rejected,
reported per route and per application, with the action-await census
riding along as a report-only diagnostic — is the kill gate for changes
2 and 3 and, transitively, for this document.

---

## 1. The property being purchased

A plan is **total** when every structural fact about an update is resolved
before the program runs:

- which state exists, and how many bytes it occupies;
- which derives exist, and exactly what each depends on;
- which view nodes exist, under which conditions;
- which slots each action may write.

Totality is not a performance property directly. It is the property that
makes performance properties *available*: you cannot lay out state densely
without knowing its width (→ 1); topologically sort a graph you discover by
running (→ 2); partition work whose independence is unknown before it
starts (→ 2, 3); speculate on an event whose handler might do anything
(→ 3); replace tree reconciliation with index arithmetic over a structure
you must walk to know (→ 4); or make a performance claim a contract when
cost is discovered at run time (→ 5).

One dimension is absent from that enumeration and must be named: **the
world boundary**. Imports, capabilities, resources/queries/mutations, tasks,
motion, timers, window lifecycle, errors, and hydration state are not
closed by changes 1–5. The shipped replay certificate
(`packages/exact-contract/src/compiler/replay-certificate.ts`) polices an
overlapping boundary — its `ReplayRejectionReasonV1` taxonomy covers
imports, host inputs, async effects, live sources, ephemerals, form
state, opaque behaviors, key commands, and window lifecycle — and it
already rejects imported execution outright (`IMPORT_NOT_ADMITTED_M0`);
the corpus carries 1,079 explicit `.ts`/`.js` `use` imports across 141
of 338 files. LLP 0480 §2.2 states the sixth closure decision (closed
plan modules plus a typed, versioned capability/effect ABI); until it
holds, "total plan" names an **admitted subset**, whose per-route
coverage the §3.6 gate's admission certificate must report.

Each change below is therefore justified twice: once by what it enables, and
once by what it forbids.

---

## 2. Change 1 — closed static types, inferred, with sum types

### 2.1 What it is

Every state slot has a known type and a known byte width, resolved at
compile time. Component instances become structs in an arena rather than
`Rc<RefCell<…>>` graphs. `each` over a keyed collection becomes a
**struct-of-arrays row arena**: one contiguous column per bound attribute,
not an array of row objects.

Sum types become a first-class construct, because the alternative —
encoding variants as loose strings or nullable field clusters — defeats
dense layout and exhaustiveness checking.

**Authors write no annotations for state.** Contract already infers state
types from initializers (`state expanded = false`); props already carry
declared types (`name: string`). The change is that inference becomes
*total and enforced* rather than best-effort, and that unresolvable cases
are compile errors with fix-its rather than dynamic values.

### 2.2 What is already landed

`packages/exact-contract/src/compiler/native-state-schema.ts` defines
`ClosedStateType` over `string | number | boolean | null | never`,
`string-enum`, `array`, `object` with `additionalProperties: false`, a
`value-kind` arm (dimension / color / duration), and an ephemeral
action-parameter arm, as an author-time preflight for Contract Native
admission, with Rust independently re-deriving the descriptor as the
admission authority. The governing section is LLP 0333 §5.1 (state
identity and fingerprint), which the file's `@ref` header already cites;
the header's *other* pointer — LLP 0331 §§6.2–6.4
(capsule/compatibility/reload) — is the stray half to drop at source.

So a closed state type system **already exists**, fingerprinted across an
ABI boundary and dual-derived. The change is one of *scope*: promote it
from an admission gate on one tier to the language's type system on every
tier — materially smaller than "add types to Contract," and the corpus
Contract Native admits today is a lower bound on what survives it.

`string-enum` is also already a closed sum type in embryo; first-class sum
types generalize it.

### 2.3 What it enables

- **Dense state arenas** — the end-state property RFC 0478 §1.3 flags as
  *not* holding today, since `contract-native` mirrors the TS `Rc` graph 1:1.
- **Allocation-free dirty-binding evaluation; no tracing GC on the
  native tiers.** Stated at the width Accepted LLP 0485 §8.2 pins:
  evaluating dirty bindings over **scalar and handle columns that
  construct no new string or collection values** allocates nothing — a
  reasoned target, not a conformance MUST; a template-string or
  array-constructing binding allocates, correctly, and those
  allocations are metered like the keyed-instance path
  (create/reuse/dispose remains the separate metered allocation path
  change 5's budgets must count). Native modes 2/3 run without a
  tracing GC; the web tier is a runner under a collector (LLP 0483 §2)
  — fewer allocations there, never "no GC, anywhere."
- **SIMD over collection derives** (LLP 0482 §4): vectorization needs
  monomorphic element types in contiguous columns — exactly what SoA row
  arenas provide.
- **Diagnostics that name the fix.** Closed-type errors are determinate —
  the precondition for LLP 0480 §9.1's mandatory-fix-it rule.
- **Cheap serialization** for replay, resumability (LLP 0483 §3), and state
  capsules across hot reload.

### 2.4 What this costs

- **No heterogeneous arrays and no arbitrary JSON as state.** This is the
  real bite. Applications that hold a network response verbatim and read
  into it lazily cannot do that.
- **Observable `undefined` leaves with it — priced here, not implied.**
  Accepted LLP 0485 §5.1 binds undefined-elimination to change 1:
  closed objects have no absent members; `??`, optional member access,
  template interpolation, and out-of-bounds indexing all lose their
  `undefined` arms (the operators survive, restated over `null` and
  option sums — `??` becomes null-coalescing; optional access returns
  an option matched exhaustively; OOB indexing is option-shaped). Every
  undefined-observing site is a migration site; the census rides 0485
  §11.3's channel, report-only. LLP 0489 carries the amendment text —
  and where its shipped checker is stricter than 0485's restatement,
  that divergence is a 0489 adoption gate, not a change to this
  pricing.
- **The mitigation must be genuinely paved, or this change fails.** Network
  data is parsed into a declared shape at the cell boundary — where
  validation belongs, and where LLP 0183 / LLP 0206 already operate. But
  the paved path must be *shorter to write* than holding the blob, or
  authors and agents route around it. Today it is not obviously shorter.
  **Owed design work, not a solved problem.**
- **Inference must be excellent.** LLP 0480 §13.2 ranks weak inference as
  the second most likely way this series fails. A closed type system with
  mediocre inference gives an agent errors it cannot fix, which is
  strictly worse than the dynamism it replaced.
- **Generic components** need a story. `snippet` and polymorphic props
  exist today; per-use-site monomorphization is the standard answer and
  costs plan size.

### 2.5 Open design problems

- The parse-at-the-boundary ergonomics (§2.4).
- Sum-type view matching lowers to the existing `match` (LLP 0184; §5.2);
  the open question is surface syntax and exhaustiveness diagnostics, not
  region semantics.
- Monomorphization strategy and plan-size cost — in tension with LLP 0480
  §7.2's identical-bytes sharing (§9.8).

---

## 3. Change 2 — static dependency graphs; delete runtime read-tracking

### 3.1 What it is

**A derive whose dependencies cannot be resolved statically is a compile
error.** The dependency graph becomes a build-time DAG, topologically
ordered once, emitted into the plan's dep table (LLP 0480 §2.2), and never
discovered at run time.

Runtime read-tracking is deleted, not merely bypassed.

### 3.2 What is there now

`packages/exact-contract/src/runtime/signals.ts` implements Solid-style
tracking: `track(core)` (lines 732–737) adds the computation to
`core.observers` and the core to `computation.deps`, records
`seenVersions`, raises `readHeight`. Reads subscribe; `untrack` (line 722)
is the escape.

`packages/exact-contract/src/compiler/semantic-graph.ts` already derives a
semantic graph (LLP 0188) over `state`, `derive`, `action`, `view`,
`query`, `mutation`, and related node kinds — and its "later phases" header
is **stale**: the file already builds import, composition, and route edges
and implements `impactSet`. More is landed than the file admits; what is
missing is not construction but *authority*.

Two boundary facts keep "already landed" honest. The graph builder
already emits local action read/write edges (`buildActionEdges`,
`semantic-graph.ts:1146`) — real credit. And the same file's statement
walker visits an assignment's **value but not its target**
(`semantic-graph.ts:1486`): `rows[index] = 1` today records no read of
`index`. Sound read masks — what LLP 0482 §3 consumes — are therefore
new work on this walker, not a re-labeling of its output; the
fail-closed per-invocation signature LLP 0482 §3.1 specifies is the
target shape.

The change: **the static graph becomes the authority, and the absence of a
static answer becomes a diagnostic.**

### 3.3 Why this is the right answer to LLP 0477

LLP 0477 identifies accidental fan-out as the live Solid-class hazard that
survives in Contract, and correctly declines to recommend either dependency
arrays or a retreat toward React. Both are wrong for the same reason, and
Dev Agrawal's framing in LLP 0477 §2.2 says it best: *a dependency array is
a note to yourself; the graph is a fact.* The right move is neither to
write the note nor to abandon the graph. It is to **make the graph a
build-time fact and print it**.

That converts fan-out from a latent runtime property into a number the
compiler emits per derive — what LLP 0477 §2.4 records Robbie Speed asking
for independently, and what change 5 turns into a budget.

Scope the closure claim carefully: LLP 0477's F3 names five failure modes
of two kinds. The *discovery* hazards — F3.4 auto-tracked `resource` reads,
F3.5 naive store/capability reads — lose their mechanism outright: there is
no accidental subscription if subscription is not a runtime event. The
*width* hazards — F3.1 whole-slot objects, F3.2 table-level query notify,
F3.3 wide derives — are granularity problems, and a static graph at
**slot** granularity reproduces the same wide edges; it makes them visible
and budgetable (change 5), not absent. Whether change 2 also fixes width
turns on a question this series must pose: **is dependency resolution
slot-granular or field-granular?** Change 1's closed object types make
field-granular edges possible for the first time; with them F3.1/F3.3
narrow structurally and change 5's budgets measure real width — LLP 0477
itself warns that LLP 0188's `whatUses` is source-level uses, not update
cardinality.

### 3.4 What it enables

- **Topological order computed once**, at build time, rather than
  maintained by `readHeight` bookkeeping on every read.
- **Partitionability.** LLP 0479 §3.1: an `Rc<RefCell<…>>` graph cannot be
  safely partitioned — aliasing is dynamic, ownership non-`Send`,
  independence unknowable without running. A static DAG is partitionable by
  inspection. Every tier in LLP 0482 depends on this change.
- **Trustworthy impact analysis** — `impactSet` already exists in
  `semantic-graph.ts` (§3.2); authority is what makes its answers safe to
  act on ("what breaks if I change this?").
- **Deleting `observers`/`deps` sets** removes per-signal allocation and
  hash-set traffic from the hot path — a memory-traffic win (LLP 0480 §6.4)
  independent of the parallelism story.

### 3.5 What this costs

- **Dynamic indexing is the hard case.** `items[i].x` where `i` is state has
  no static answer at element granularity. Three candidate resolutions, none
  chosen: (a) *collection* granularity — correct, conservative,
  reintroducing exactly the coarse fan-out LLP 0477 warns about; (b) an
  explicit cursor construct making the selected element a first-class
  reactive position; (c) reject and pave an alternative. **The single most
  important unresolved question in the series** (LLP 0480 §15.1) — and it
  is **blocking**: until W2 selects the rule (including option/OOB
  behavior and the resource-identity implications), "static totality" is
  not yet a coherent language decision, only a direction. *(**W2 has
  selected — DECIDED 2026-08-25**: RFC 0518 Ruling A adopted at the W2
  venue via the LLP 0508 §14 "Dynamic Ceiling" amendment — resolution
  (b), the explicit cursor (0485 §6.2's cursor + budget-enforced visible
  widening, option-shaped OOB), with computation escapes ruled separately
  as 0518 Ruling B's three-lane islands. Record edit under the author's
  decision, Charlie Cheever via exact-9e.)*
- **Cross-module totality is required.** A derive reading a capability or
  an imported shape needs a static answer across the boundary; LLP 0183 /
  LLP 0206 provide the machinery, totality is new work on top.
- **Conditional dependencies** (`a ? b : c`) cannot simply
  over-approximate to the union — the earlier "small, acceptable
  precision loss (extra recomputation, same value)" framing for pure
  derives was **wrong**: unioning both arms can create **false SCC
  cycles**. `a = flag ? b : 0` with `b = flag ? 0 : a` is branchwise
  acyclic under every valuation of `flag`, but the union produces an
  `a ↔ b` cycle with *no static topological order at all* — the
  build-time DAG cannot be constructed and a legal program is rejected
  (or ordered undefinedly). The current runtime is branch-sensitive
  (`signals.ts:871–886`); the static walker visits both ternary arms
  (`semantic-graph.ts:426–499`). Change 2 must therefore pick, for pure
  derives too: guarded/predicate-labelled edges (branch-aware SCC
  analysis), or explicit rejection of mutually conditional derive pairs
  with a diagnostic and priced migration. (Accepted 0485 repeats the
  erroneous same-value claim; the erratum is filed —
  `issues/20260820-llp0485-conditional-union-false-scc-erratum.md`.)
  For a **`resource` the union is additionally a semantic change**:
  resource identity is the declaration instance plus the dependency
  tuple the initializer read (`llp/contract/0082:81–84`), and a
  dependency change is an identity change — so widening an
  initializer's edges to the union lets a write on the *inactive*
  branch restart/cancel/refetch a resource that today would not blink,
  changing observable status transitions and real network activity.
  Change 2 must pick one of: guarded edges/branch masks for resource
  initializers (precision kept, analysis cost paid), authored resource
  dependency lists (the one place a dependency annotation earns its
  keep), or an amended identity rule priced as a breaking change with
  migration (§9.9).
- **`untrack` loses its meaning** and must be either removed or redefined.

### 3.6 The kill gate

LLP 0480 §12.2 — the plan-admission certificate, under a verdict rule
pre-registered there before it runs. Dependency resolution is one axis
of a per-context classification (plan-opcode / capability-ABI /
server-only / explicit escape / rejected), reported per route and per
real application rather than as a pooled percentage — r2's "85% of
derives+bindings+regions" rule is retired there, with reasons. If any
axis shows a rejected-class residue with no tractable disposition, this
change does not work, and with it the series. The audit runs against
the shipped corpus, never a fixture written for it.

---

## 4. Change 3 — actions are synchronous and total

### 4.1 What it is

An action becomes a **reducer**:

> `(state, event, envSnapshot) → (state′, Command[])`

It may not `await`. It reads state plus a captured environment snapshot
(clock, RNG draw, other declared ambient inputs), performs declared writes,
and returns commands — task starts, capability invocations, navigation —
which the runtime executes *after* the transition commits. Task and
capability completions re-enter as typed events dispatching other actions.

Forbidding `await` alone does not buy this (the first draft claimed it
did). A synchronous action can still call another action or invoke an
imported capability mid-body (`llp/contract/0082:135–136` allows both),
mutate through `mutates`, throw, or read the clock. So the change is
two-part: the reducer shape, plus a **closed effect signature** — `writes`
and `mutates` (already separate `ActionIR` fields, alongside `effectSites`)
extended to declare task starts, capability calls, and navigation, composed
transitively through action-to-action calls.

And the closure is wider than r2 stated, in seven ways this revision
enumerates rather than gestures at:

1. **Loops exist and must be priced — r2's "action bodies have no loop
   construct" was false.** `Stmt` in `compiler/ir.ts` carries a `for`
   kind (over an iterable, alongside `if`, member/index assignment
   targets, and `await-assign`), and LLP 0479 §5.1 quotes its lowering.
   Termination is therefore not free from the grammar: the
   non-termination vectors are action-call cycles (rejectable from the
   call graph) *and* loops, which are bounded only when the iteration
   domain is a finite collection the body cannot grow — a rule the
   compiler must enforce, with imported iterables excluded by closure 7.
   Loops also thread through everything downstream: change 5's cost
   bounds must price loop bodies symbolically (`perRow`-shaped, never
   scalar), rollback must survive a throw mid-loop, and LLP 0482's
   conflict masks must count loop reads/writes transitively.
2. **Tasks stop writing state directly.** `llp/contract/0082` permits
   state writes from actions, tasks, and resource settlement — a
   reducer closure over actions alone is empty if the other two writers
   stay open. `TaskIR` today carries `mutates` and effect sites but
   **no state-write signature**; under change 3 a task body performs
   I/O and *dispatches typed events*; only actions transition state.
3. **Resource settlement becomes an event.** A resource's
   status/value/error transitions enter the log as typed events driving
   §5.2's state-machine regions, not as out-of-band writes. This is
   what lets replay cover the async world instead of exempting it.
4. **Callbacks compose into the signature.** An action-typed prop
   (`press: (id) => void`) is an effect-carrying value with **no effect
   row today**; transitive composition must run through callback
   values, so a component's callback props become part of its declared
   interface — otherwise the signature closes over direct calls and
   leaks through indirection.
5. **Capability calls carry affinity and purity.** The signature
   records not just *that* a capability is invoked but its declared
   thread affinity (`docs/callback-affinity.md` exists because affinity
   is real) and purity class (LLP 0206); LLP 0482 §4.3's
   worker-affinity certificate reads both.
6. **Exception rollback needs a mechanism, not a sentence.** "Aborts
   atomically: no partial writes, no commands" is cheap for slot
   assignment and unresolved for in-place member/index mutation, where
   atomicity hangs on alias semantics this cluster has not settled. The
   owed design — journaled writes or copy-on-write per transition — is
   priced in §4.4, not assumed.
7. **Imported opaque calls are the sixth closure's case.** An imported
   helper inside an action body either lowers, is a declared ABI call,
   or the action lands in the rejected class of LLP 0480 §12.2's
   certificate — the 1,079-import residue (§1) concentrates here.

**This is a breaking amendment and must be named as one — and it amends
more of 0082/0087 than the async-action clause alone.** It rewrites
`llp/contract/0082`'s action model *and* its write-source rule
(actions/tasks/resource settlement — closures 2–3), constrains
`llp/contract/0087`'s statement classes (loops — closure 1) and its
treatment of action-typed values (closure 4), and answers LLP 0330's
**open clause A4** (async action segmentation) by deleting the construct
A4 exists to specify. It is *not* a promotion of A10 — the
thenable-derive rule, closed 2026-08-13. The async-cell seam
(LLP 0259/0263) is the paved pattern this generalizes; RFC 0478 §P2's
loop invariant — *the loop reads cells; it never blocks on their
producers* — becomes a property of the language rather than of the loop.

The reference model is Elm's update function returning `(Model, Cmd)`, with
Contract's fine-grained declared slots in place of a monolithic model and
the closed effect signature in place of an unconstrained `Cmd`.

**Scheduling collision, carried explicitly:** RFC 0478 W3.1 promotes
native *async-action* lowering while this change deletes async actions
from the language. LLP 0480 §10.4 already recommends pausing exactly
that lowering (the rest of W3 continues) until the admission audit
reports, and Accepted 0485 records the two strands as mutually
destructive if both proceed. The pause recommendation stands here too;
the decision is 0478's owner's.

### 4.2 Why this is worth the disruption

Three payoffs, and they are unusually large relative to the cost.

**Deterministic replay.** A session becomes an ordered event log plus each
event's environment snapshot. The shipped certificate
(`packages/exact-contract/src/compiler/replay-certificate.ts`) already
enumerates today's hazard classes (imports, host inputs, async effects,
live sources, ephemerals, form state, opaque behaviors, key commands,
window lifecycle); the reducer moves async effects and ambient reads into
the log, shrinking that list structurally rather than erasing it. The
yield: replay debugging, regression fixtures, reproducible crash reports,
deterministic conformance, agent-facing bisection — LLP 0442's thesis
landing here rather than retrofitted. LLP 0480 §9.4 argues this deletes
the "reproduce it on device" class of multi-turn agent failures — on the
correction-turn metric, possibly the largest single win in the series.

**Safe speculation.** A reducer's transition is a pure function of
`(state, event, envSnapshot)`, so the state′ and ops resulting from a
*hypothetical* event can be computed before it occurs (LLP 0480 §6.5) —
state and ops only; speculated commands never execute until the event is
real. A toggle precomputes both branches; a navigation its destination.
Unavailable in any design where a handler may await or carry undeclared
effects.

**Safe concurrent dispatch.** LLP 0482 §3's conflict partition requires an
action's effects to be complete at its return and its reads and writes to
be statically known. An awaiting action's effects span an unbounded
interval; it has no usable conflict signature.

And a fourth, evidential: LLP 0479 §5.4 finds that **100% of the measured
invocations** in the committed statement-backend benchmark traverse the
await-continuation path. The program's one committed action-latency signal
is substantially measuring machinery this change deletes.

### 4.3 What this costs

- **The `await` idiom in action bodies goes away.** This is a visible,
  daily ergonomic change and the most likely source of author complaint.
- **Sequential async flows become explicit state machines.** "Submit, then
  on success navigate" becomes two actions and a cell transition rather than
  one procedure. This is more honest and more verifiable; it is also more
  lines. The mitigation is a paved multi-step form — a `task` naming its
  success and failure actions, **visually distinct from `action` at a
  glance (a keyword, not a convention)** — and **it must exist before the
  narrowing lands**, or every application invents its own. The adoption
  bar is **migration evidence, not source length**: line count is a
  gameable proxy and is demoted to a reported diagnostic; the gate runs
  on representative migrations measuring completion rate, correction
  turns, time-to-fix, diagnostic quality, and opaque-escape routing. If
  that evidence shows authors and agents routing around the paved form,
  delay change 3 rather than shipping the narrowing on a promise.
- **Agent familiarity cuts both ways.** Elm/Redux shapes are well
  represented in training data; `async function` handlers are far more
  common. LLP 0477's uncanny-valley finding suggests the deciding factor
  is *regularity* — a rule with no exceptions is learnable in one turn.
- **The action-await census rides in LLP 0480 §12.2's certificate
  pass**, explicitly report-only: an awaiting action is always
  mechanically expressible in the event form, so no count of awaits can
  kill this change — the census prices the migration and shapes the
  paved form (§4.4). What kills is a context the certificate must
  reject outright.

### 4.4 Open design problems

- The paved multi-step form (§4.3).
- Whether `task` bodies may await (probably yes — they are outside the
  interactive loop and, under §4.1 closure 2, no longer write state —
  but then the `action`/`task` boundary must be unmistakable at a
  glance).
- Cancellation semantics for in-flight tasks when their owner unmounts.
- The rollback mechanism (§4.1 closure 6): journaled writes vs
  copy-on-write, and the alias-semantics question in-place mutation
  hangs on. **Proposed resolution of record:** Accepted LLP 0485 §7.5
  answers both — A-VAL value semantics (aliases unobservable) plus an
  ordered `(address, before, after)` write journal per transition;
  adopt that here rather than keeping a second design live, unless W2
  supersedes it by name.
- The loop-boundedness rule (§4.1 closure 1): what exactly makes a
  `for` admissible, and the diagnostic when it is not.

---

## 5. Change 4 — view structure fully resolved at build time

### 5.1 What it is

Every view node gets a stable slot id, and every structural construct
becomes a typed **region record** with statically known inputs and a known
output span. An update is then predicate evaluation → region toggle → dirty
binding evaluation → op emission, index arithmetic over the flat tables of
LLP 0480 §2.2.

The honest name for the resulting property is that the artifact is
**schema-total and instance-parametric**. Everything *schematic* is closed
at build time: opcodes, templates, node sites, dependency edges, region
kinds, effect signatures. What remains runtime-parametric is *instances*:
which branches are active, which keys exist, cardinalities, variable
payload sizes. Keyed `each` is therefore not "a permutation over an index
array," as the first draft claimed: `contract-native/src/view.rs` today
creates, reuses, disposes, and reorders keyed instances, and
insert/remove/reorder remain instance reconciliation in any design. What
the plan changes is what that bookkeeping runs against — one closed
template with statically known bindings. Keyed-instance management, never
tree diffing.

### 5.2 The real structural inventory

The premise must be stated correctly, because the first draft got it wrong:
Contract's view grammar does **not** have "exactly two structural
control-flow forms." The view IR (`compiler/ir.ts`) today carries `when`,
`match` (LLP 0184 — parsed and tested), `match-size`, `each`, `resource`,
`async`, and `boundary` structural kinds, plus `pager`, snippet calls, and
the graphics scene kinds — and `native-eligibility.ts` already admits
`match` and `async` into the native v1 view-kind set. Change 4 therefore
owes a region-record kind per construct: exclusive-branch regions for
`when`/`match`/`match-size` (one predicate or scrutinee, N known
templates); keyed row regions for `each` (§5.1); state-machine regions for
`resource`/`async`/`boundary`, whose pending/value/error arms are known
templates driven by cell state — coupling this change to change 3; and
composition regions for `pager` and snippet calls.
Sum-type matching from change 1 adds **no new region kind**: it lowers to
the existing `match`. LLP 0501 pins this inventory as exactly **six
kinds** — `exclusive`, `keyed`, `cell`, `composed`, `graphics`, `root` —
which is the authoritative count (many constructs, six kinds).

The change is still the cheapest of the five: `CONTRACT_TAG_LOWERING`
closes the tag set, `assignStructuralIds` in `compiler/identity.ts` already
assigns structural identity, `compiler/slots.ts` demonstrates the *derived,
never stored* table pattern, and LLP 0185's `aot-views.ts` already collects
view nodes into a flat list. The semantics barely move; what moves is when
structure is known. But "nearly free" was the wrong price: the region
vocabulary must cover the full inventory, not two forms.

### 5.3 What it enables

- Deleting the runtime view tree — LLP 0381's P3/P4 CSR programs
  (`compiler/production-program.ts`, "closed admission … no per-node or
  per-expression legacy fallback") are already a working precedent on one
  path.
- Regions as index ranges make **partial rendering and streaming**
  (LLP 0483 §5) and **server/device region placement** (LLP 0482 §6 — the
  placement tier lives there; LLP 0483 has no placement section)
  expressible, because a region is a nameable, independently evaluable unit
  with known inputs and a known output span.
- **First frame as a build artifact** (LLP 0480 §6.1): where structure is
  known *and initial state is known*, the initial op stream is computable
  at build time. LLP 0307 Phase B v0 already ships the IR-emitted form;
  LLP 0480 §6.1 owns the bake-and-patch carve-out for env-dependent
  initializers.

### 5.4 What this costs

- **A wider region vocabulary than advertised** — the constructs are
  many, and LLP 0501 pins them onto **six Region kinds** (`exclusive`,
  `keyed`, `cell`, `composed`, `graphics`, `root`), not the first
  draft's two forms (§5.2) — with the `cell` state-machine regions
  co-designed with change 3.
- **Plan size grows with static structure.** A view with many mutually
  exclusive branches encodes all of them. Acceptable for a `mmap`ed
  read-only artifact; less so over a cold network, where LLP 0483 §4 owns
  the route-splitting answer.
- **Dynamic component composition** — choosing a component by value at run
  time — must be expressible as a bounded region choice, or rejected.

---

## 6. Change 5 — contract blocks as obligations, with cost claims

### 6.1 What it is

Two moves.

First, contract clauses compile **into the plan** as assertions that are
zero-cost when disabled, and are discharged **at compile time** wherever
possible. Today `action ... writes` is checked statically *and* per
invocation, and `derive ... depends only on` is compiler-static
(`llp/contract/0085`). Be precise about how far changes 1–4 move this: the
*structural* clauses — `has` / `missing` / `count` over unconditional nodes
and statically sized regions — become compile-time facts. Most of the
clause vocabulary does not: guarded visibility, state-dependent assertions,
interaction outcomes, and navigation claims are properties of a live
surface and remain runtime / live-agent checks under `llp/contract/0085` —
with cheaper harnesses, not compiler proofs.

Second — the new construct — **cost claims**:

- a fan-out bound per derive — split into two quantities, because one
  scalar cannot carry both: **schema-edge width** (a build-time number
  under change 2; whether it measures real width is §3.3's granularity
  question) and **live fan-out**, which is symbolic wherever a keyed
  `each` turns one schema edge into N live consumers (`base +
  rows × perRow`-shaped, like the op bound below; a hard budget on live
  fan-out requires a declared cardinality cap);
- a per-update op-emission bound per action — **symbolic, not scalar**
  (`base + rows × perRow`): dynamic `each` instances turn one static edge
  into N live consumers, so a scalar bound is either wrong or vacuous;
- optionally, a plan-size bound per route.

A screen exceeding its declared budget **fails the build.**

### 6.2 Why this is the agent-facing keystone

An agent can already check correctness three ways: the compiler, tests, and
live Acto (LLP 0365's loop). It cannot check *cost*. Performance regressions
are therefore the residual class that reaches a human, or production, or
costs a correction turn after a benchmark the agent cannot locally
reproduce.

Cost claims close that gap using machinery the other four changes create.
They are the reason change 2 is worth its narrowing even for authors who do
not care about parallelism: **fan-out stops being a thing you discover and
becomes a thing you declare.**

### 6.3 What this costs

- **Budgets need defaults, or nobody writes them.** A build failure on an
  unwritten budget is unacceptable; a budget nobody writes is inert. The
  likely shape is an inferred budget recorded on first build and thereafter
  ratcheted, with explicit override — which is a governance design, not a
  language design, and is owed.
- **Op-count is a proxy for cost, not cost.** It is stable, deterministic,
  and machine-checkable, which is why it is the right *claim*; it is not
  the same as time, and the relationship must be measured before op budgets
  are presented as performance guarantees (RFC 0478 §P7).
- **Assertion compilation must be genuinely zero-cost when off**, or the
  verification surface taxes the production path it was meant to protect.

---

## 7. What is not proposed

Explicitly out of scope, to prevent scope drift in review:

- **No new authoring language, dialect, or "fast mode."** RFC 0478 §P1's
  one-language invariant is retained without qualification.
- **No change to the indent grammar.** These are semantic changes; the
  surface syntax moves only where a construct is added (sum types, cost
  claims, possibly a cursor).
- **No retreat toward React's model.** LLP 0477's conclusion holds; every
  change here moves *away* from prediction and toward declaration.
- **No GPU compute in the middle.** RFC 0478 §1.4 stands (LLP 0482 §5).
- **No presenter changes.** LLP 0480 §8's material contract is separate
  and independent.

---

## 8. Sequencing, if adopted

Strictly gated, cheapest-and-most-falsifying first — and **unchained from
the representation track**: per Accepted LLP 0485 §1.3, Track L (this
language program) runs on **F1 alone**; F2 was never a language gate, and
chaining language work behind it would silently repeal that rule. (F2 has
in any case run and passed — 2026-08-20 adjudication — so the point is
now also moot in fact; it is recorded because the order below previously
implied the chain.)

1. **LLP 0480 §12.2 — the admission-certificate audit** (per-context
   classification across every admission axis, the await census riding
   along report-only). Kill gate for changes 2 and 3, and transitively
   the series'. Days of work, not a day; nothing else in this program
   starts first.
2. **Change 1 scope promotion** — `ClosedStateType` from Contract Native
   admission gate to language type system, behind a compiler flag, measured
   against the corpus for rejection rate.
3. **Change 4** — structure resolution and region records: mostly compiler
   work, co-designed with change 3 where state-machine regions meet event
   re-entry (§5.4).
4. **Changes 2, 3** — the two real narrowings, each landing only after its
   paved alternative (§3.5's cursor decision, §4.3's multi-step form) exists.
5. **Change 5** — last, because it depends on the numbers changes 1–4 make
   available.

*(Informational, not a step: LLP 0480 §12.1's Rust-tier flat-plan spike —
the representation direction's own gate — ran and passed on 2026-08-20;
Track R proceeds on its own clock and this sequence neither waits for nor
feeds it.)*

Every step lands as an amendment to LLP 0330, not as a fork of it. LLP 0330
is still Draft, which is the most favorable moment this program will ever
have for semantic tightening — and, as LLP 0479 §6 argues about the tax
generally, the cost of waiting is a ratchet. The same fact cuts the other
way: RFC 0478 W2 *is* the ratification of LLP 0330, so this proposes five
amendments — one breaking (§4.1) — to a spec mid-ratification. LLP 0480 §10
enumerates that cost and holds the open timing question.

## 9. Open questions

1. Dynamic indexing (§3.5) — cursor, collection granularity, or rejection?
   Everything else waits on this. **0485 proposes an answer, gated on W2
   adoption:** an explicit `cursor` construct plus budget-enforced
   visible widening for the residue — treat as the leading candidate,
   not an unchosen fork. *(**DECIDED at the W2 venue, 2026-08-25** —
   Charlie Cheever, via exact-9e: RFC 0518 Rulings A + B adopted verbatim
   through the LLP 0508 §14 ratification (0508 §13 item 19). The answer
   is the cursor: 0485 §6.2 is the shared data-access rule, the escape
   hatch is unavailable for data access, and un-plannable computation
   takes 0518's three-lane island treaty with an empty initial
   whitelist. Record edit under the author's decision.)*
2. Does parse-at-the-boundary (§2.4) have a form shorter than holding the
   blob? If not, change 1 fails on ergonomics rather than on capability.
   (LLP 0489 §5.1's census — 60.9% file rejection, 90 `shape` sites —
   says this is not yet shown. Price the plurality class too: 120 of the
   278 rejection sites are `closed-types/prop-annotation`, so change 1's
   author-facing cost today is **prop discipline** more than state
   inference — the migration story must cover it, not just `shape`.)
3. Is dependency resolution slot-granular or field-granular (§3.3)? Change
   1 makes field-granular possible; the answer decides whether change 5's
   budgets measure real width and whether change 2 touches LLP 0477's
   width hazards at all. **0485 proposes field/column granularity for
   closed objects and keyed-row columns, gated on W2 adoption.**
4. What is the paved multi-step async form (§4.3), and does it survive the
   migration-evidence bar (completion rate, correction turns, time-to-fix,
   diagnostic quality, escape routing — line count reported as a
   diagnostic only)?
5. How are cost budgets defaulted and ratcheted without becoming either
   inert or hostile (§6.3)?
6. What is the measured op-count↔time relationship, before op budgets are
   called performance guarantees (§6.3)?
7. Which of these five can land on the TS tier alone as a compatible
   tightening, and which require the flat plan to be worth doing?
8. Which constructs monomorphize (§2.5) and which stay polymorphic-but-
   shared? Monomorphization and build-time theme-token resolution both pull
   against LLP 0480 §7.2's content-addressed identical-bytes sharing.
9. Which resolution does §3.5's resource case take — guarded edges,
   authored resource dependencies, or an amended identity rule with
   migration? Unlike the pure-derive union, this one changes observable
   status transitions and network behavior, so it cannot default
   silently to the union. **0485 proposes guarded resource edges
   (precision kept), gated on W2 adoption.**
