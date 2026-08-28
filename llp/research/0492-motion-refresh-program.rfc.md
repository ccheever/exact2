# RFC 0492: Motion Refresh Program — one graph, one authoring path, motion as plan data

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Motion, Contract (compiler/runtime), kernel, GPU, Renderer, Apple Host, Windows Host, Android, Web, Rust Native, Agent API (Acto), Design Mode, Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-20
**Revised:** 2026-08-26 (§4.1 gains a **third view-replacement case** — RFC 0540 §1.6 virtual visibility: a hidden row keeps its SharedValues and drivers evaluating on true elapsed time while only presenter sinks detach, re-attaching on `prerender` with an attachment receipt; in-flight recognizers survive, so visibility is not a cancellation boundary. A case added to existing rules, not a new mechanism — OQ9 vocabulary unchanged, sinks still transform/opacity. Executes 0504 §3 row 64; delivery post-window per 0540 §8.6. 0506 D1(d) burn-down lane.) 2026-08-25 (OQ3 answered by citation at RFC 0518's adoption — runtime graph construction = Ruling B Lane 2 over the LLP 0485 §3.3.14 Class-S artifact + the retained bounded admission; dated annotation only, per 0518 ask 3, discharging the shared 0504 §3 row 26; decision Charlie Cheever 2026-08-25 via exact-9e.) 2026-08-23 (amendment `A-0492/0510-MOTION-EVALUATOR-EPOCH` appended to §8's joint rulings — the four-member `motionEvaluatorEpoch` preimage, build-emits/host-recomputes comparison at LLP 0510 admission, evaluator-only changes MUST move it and wire-only changes MUST NOT; ratified by Charlie Cheever 2026-08-23 as drafted in LLP 0491.001 §7, decision relayed via orchestration session exact-9e. The epoch generator and golden fixtures land with the RFC 0491 Phase-4 horizon, riding A-0510's single tuple major.) 2026-08-20 (clock-join sync per the 2026-08-20 holistic corpus reviews — five residual pre-docket delegation/runOnJS sites synced to the row-7 decision (Summary, the rejected-in-pre-draft paragraph, M-B's abort gate inverted to the v1 ruling, M-G no-shim, §9's bet).) 2026-08-20 (r5 — LLP 0505 r3 row 7 applied, DECIDED by
Charlie: the delegation default inverts — v1 ships evaluator-everywhere
on all platforms (the recorded M-B abort-fallback is the v1
configuration); per-platform CA/CSS handback leaves the critical path
and returns only on measured power need at a dated Apple checkpoint
(2026-10-01 placeholder, owner may move); `motion.runOnJS` deleted
outright with in-repo migration to `motion.command` — no shim, OQ5
resolved.)
2026-08-20 (r4 — lockstep amendment with RFC 0491 r15 —
performance-claim ownership transfer, per 0491 §7.4: the ≤1 ms/≤64-node
incremental-relayout number, the gesture classification (which animations
may claim the ≤64 budget), and M-D's gate are **owned by this program**;
RFC 0491 supplies the dirty-set instrument and result equality as WS-H
exit obligations. The Summary, §2's "number attached" paragraph, the §3
consumed-rulings row, §5 M-D's gate, §7's sequencing line, and §8's 0491
entry are restated accordingly, and §8's "adds no new asks of the kernel
program" is corrected — the instrument is a standing ask, and a target
miss commissions kernel skip-work as scheduled 0491 work.) 2026-08-20 (r3 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision: M-F's exit rewritten onto M-A's oracle (deterministic-math
prerequisite or 0487 semantic equality — the M-F/M-A contradiction
resolved; the M-D-customer leg names its dependency); §4.2's headline
aligned with M-A (pinned evaluator/capture; no cross-machine screenshot
identity); §4.6's dangling OQ7 pointer replaced with the 0500 D2 close;
§8 gains the named narrow 0485 §3.3.14 amendment (evaluator retarget
worklet-runtime → `exact-motion` + the S-code→S-data paved path);
"closed by construction" scoped to the handoff ticket — the arming-race
and mid-gesture tickets are presenter-lifecycle defects repaired by
explicit M-B/M-C attachment work (binding identity, generation-fenced
activation, reattachment, in-flight recognizer survival, receipts;
verified against both tickets); OQ8 notes the shared 0483 runner-budget
object) 2026-08-20 (r2 — program super-refine round 1, both families NOT READY: motion-as-plan-data restated as 0485-conformant — motion stays a §3.3.14 Class S *referenced* program whose referenced artifact becomes compiled `exact-motion` data, with in-container graph bytes named as an explicit 0485 amendment decision, never a silent joint ruling; OQ7 closed by frozen LLP 0500 D2 (wasm evaluator in the fixed runner, dev and prod; the TS evaluator is deletion-listed, not a dev shim); M-A's exit repaired — bit-identical *value sequences* on a pinned evaluator with a deterministic-math obligation for cross-architecture claims, presentation evidence per LLP 0354 pinned capture, layout-affecting cases via the 0487 instrument — cross-machine screenshot identity withdrawn; bounded runtime admission retained for escape-hatch graphs; the Rust-root motion API restated as a producer-side builder behind the App-ABI; the 0490 M3 unblock named as a ruling in §8; handback failure → evaluator-everywhere promoted to a named M-B abort gate; Design Mode knobs bound to 0500 D1 overlay patches; §4.4 budget claims split symbolic vs measured) 2026-08-20 (r1 — initial draft; incorporates pre-draft review: the `transition=` ruling is reframed as unify-representation / delegate-execution-with-declared-handback rather than "compiler sugar dissolves Tier 1", and the layout-motion workstream is explicitly gated on RFC 0491 WS-H's number being **demonstrated on the LLP 0487 corpus instrument**, not merely specified)
**Related:** LLP 0492.001 (**M-D's layout-budget classification ruling**, 2026-08-26 — the §4.4/§5 adjudication of the ≤1 ms/≤64-node number against the RFC 0491 WS-H instrument's first measurement; agent-proposed pending author ratification, landed enforcing), RFC 0099 (Motion — the Tier-2 substrate and the three-tier model this program re-founds as lowerings; amendment owed at acceptance, §8), RFC 0491 (Kernel Refresh Program — §7 rulings 1–2 and the r8 amendments this program consumes: evaluation ownership, tick-source agnosticism, wasm-compilable `exact-motion`, per-binding budget claims, deletion-bound runtime motion validation; its §7.4 r15 transfer makes the ≤1 ms/≤64-node claim this program's), RFC 0490 (GPU-First-Class Substrate — one frame clock §3.3, SharedValue GPU endpoints §3.4; this program is a consumer, never a second decider), LLP 0396 (Interactive Motion State Machines, Accepted — the authorable model M-E implements), RFC 0480 / LLP 0481 / LLP 0485 (the flat plan — motion as a §3.3.14 Class S referenced compiled artifact, §4.3/§8; the in-container question is a named 0485 amendment decision), LLP 0483 (Web on the Flat Plan — the fixed runner the wasm evaluator ships inside), LLP 0449 (Contract sink families — the `layoutIsland`/paint-scalar families M-D completes; its embedded-release slot-sampler stub is a named defect this program deletes structurally), LLP 0313 / LLP 0349 (layout islands / programmable layout — OQ10 mechanism authorities), LLP 0297 / LLP 0322 (threading contract — main-owned motion domain §4.4, SharedValue reader rules §4.5), LLP 0411 (cross-runtime shared data — the 32-bit scalar constraint), LLP 0354 (frame pacing — the presentation-evidence model M-A extends), LLP 0442 (PocketJS lessons — the virtual-rate/frame-transaction proposal M-A operationalizes; settle timing as the agent loop's live flake source), LLP 0100 (interactive navigation transitions — velocity resolve + two-phase settle, a consumer of M-B's handback vocabulary), LLP 0448 (motion drag presentation — out of scope here, consumer of the same graph), LLP 0331 / LLP 0332 / LLP 0333 (Rust Native roots / App ABI — the no-JS consumer M-F serves), LLP 0406 (Expose/DRM milestone — the no-JS display path that makes Rust-owned motion non-optional), LLP 0486 (one layout language — source of the "doctrine with no instrument" failure mode M-D's gate is written against), LLP 0487 (choice-layout conformance corpus — the measurement substrate for this program's ≤1 ms number), `issues/20260717-motion-tier1-tier2-handoff.md`, `issues/20260716-motion-graph-midgesture-state-stability.md`, `issues/20260726-motion-gated-on-ios.md`, `issues/20260717-motion-v1-physical-acceptance-evidence.md`, `issues/20260801-motion-baked-timeline-measurement.md`, `issues/20260803-motion-bindings-need-a-testid.md`

## Summary

Motion becomes the third leg of the Rust substrate: **one value graph, evaluated
by one Rust crate (`exact-motion`), ticked by one clock, compiled to a
plan-referenced artifact (0485 §3.3.14)**, sitting between the kernel (layout sinks, RFC 0491) and the GPU spine
(paint and uniform sinks, RFC 0490). Hosts are sink appliers. Contract, Rust
Native roots, and Design Mode are frontends that compile motion to plan data;
JS appears as an authoring surface and a fenced escape hatch, never on the
frame path.

Three headline moves, in the order they pay off:

1. **Determinism as substrate.** The frame clock's tick source is
   interchangeable (display link / rAF / virtual — RFC 0491 §7.2 as amended
   r8), so every animation and gesture becomes seekable, replayable, and
   agent-deterministic. This retires the settle-timing flake class LLP 0442
   names as the agent loop's largest remaining nondeterminism source, and it
   is the cheapest workstream — it does not wait for the rest of the program.
2. **One representation, lowered execution.** Everything motion-shaped —
   `transition=`, the SharedValue graph, LLP 0396 state machines — compiles to
   one compiled, plan-referenced graph. **v1 executes everything in the
   evaluator on every platform (DECIDED 2026-08-20, LLP 0505 r3 row 7)**;
   delegating a pure two-endpoint composited animation down to Core
   Animation / CSS — as a lowering with **declared handback semantics** —
   returns only at the dated Apple checkpoint if measured power demands
   it. What dies is the second source of truth; the platform's free power
   win is the checkpoint's question, not v1's. The Tier-1/Tier-2 *seam* — where the
   open motion ticket cluster lives — becomes structurally impossible while
   the "lowest sufficient tier" discipline (RFC 0422's for paint, RFC 0491
   OQ10's for layout) is preserved for execution.
3. **Motion for every tier.** A no-JS Rust Native root, the Expose/DRM path,
   and the web fixed runner (same evaluator, compiled to wasm — RFC 0491 WS-G
   requires the crate wasm-compilable from day one) all run the identical
   evaluator. Layout joins the sink families once — and only once — **this
   program's own ≤1 ms/≤64-node target** (transferred here by RFC 0491
   §7.4; 0491 supplies the dirty-set instrument and result equality) is
   demonstrated on the WS-H instrument over the LLP 0487 corpus.

This RFC is a **program document in the RFC 0490/0491 mold**: it consumes
their rulings rather than restating them (§3), owns the authoring path, the
determinism/evidence story, the sink-family completion, the state-machine
implementation, and the no-JS/web legs (§5), and specifies the
plan-referenced artifact inside 0485 §3.3.14's existing shape (§4.3/§8).

## 1. Motivation

**The ticket cluster is seam-shaped, not bug-shaped.** Twenty open motion
tickets cluster on the boundaries between motion's coexisting
implementations, not inside any one of them:
`20260717-motion-tier1-tier2-handoff.md` (the ownership handoff between the
kernel transition system and the SharedValue graph is unimplemented and
untested), `20260716-motion-graph-midgesture-state-stability.md` (ordinary
Contract state writes destabilize a mid-gesture graph),
`20260728-native-motion-arming-boot-race.md` (arming races the boot
sequence). Each is a defect *between* evaluators. RFC 0099's honest
implementation-status table shows the same fault line spatially: Tier-1
transitions are landed on Apple + web, but **Android skips `SetTransition`
entirely, Windows scene projection carries no transition state, and web
lowers spring-kind to a fixed easing approximation** — three platforms, three
different answers to the same authored line.

**No-JS apps currently get nothing.** Motion's authoring and half its
evaluation assume Hermes. A Rust Native root (LLP 0331) or the Expose DRM/KMS
path (LLP 0406) has no motion story at all. RFC 0491 §7's ruling — Rust owns
motion-graph evaluation; hosts become sink appliers; the 8.4K-line Swift
graph mirror collapses — makes a single cross-tier evaluator possible; this
program makes it *reachable from every authoring surface*.

**The agent story leaks determinism exactly here.** LLP 0442's finding:
"Acto is Exact's differentiator and the agent loop is where its remaining
flake lives" — and the flake is settle timing, i.e. motion. A pure
closed-form graph under a virtualizable clock is the structural fix, and
RFC 0491 r8 made the kernel-side prerequisite (tick-source agnosticism) a
requirement rather than a hope.

**Layout motion is the one frontier with a number attached — and the
number is this program's.** Per RFC 0491 §7.4 (r15 lockstep), WS-H's own
exits are result-equality and the dirty-set instrument; the quantitative
target — dirty-scoped incremental relayout, ≤64-node affected set in a
1K-node tree, ≤1 ms on reference hardware, measured on the LLP 0487
corpus — is **owned here**, demonstrated on that instrument, with Motion
as the named customer. Shared-element/FLIP, reorder animation, container-size
motion, and keyboard avoidance all reduce to that capability. Meanwhile
LLP 0449 records that the embedded release build **stubs the slot sampler,
so a shipped embedded app cannot use layout sinks at all** — a defect this
program deletes structurally by carrying the motion program in the plan
rather than in a JS sampler.

**And the polish platform is gated.** `20260726-motion-gated-on-ios.md`:
motion is off on iOS, the platform where feel matters most, with
`20260717-motion-v1-physical-acceptance-evidence.md` still open. A program
that ends with motion ungated on iOS under real-input physical evidence is
the honest finish line.

## 2. Constraints (non-negotiables this program preserves)

1. **The graph is a pure function of (clock, inputs).** Drivers stay
   closed-form in `t` (RFC 0491 lists the closed-form motion math among its
   protected constraints); every value is derivable from clock time, the
   gesture input log, and graph topology. Purity is what buys seek, replay,
   virtual rate, and journey-capsule replay — no workstream may trade it away.
2. **LLP 0297 threading.** The motion domain stays main-owned (§4.4, per
   RFC 0491 §7.1 — not the runtime-thread `Kernel` struct); SharedValue
   reader/writer rules (§4.5) hold; GPU completions reach the slab only as
   generation-fenced commands applied by the main Motion writer (RFC 0490
   §3.4.4). No new cross-thread sync waits.
3. **LLP 0411's scalar constraint.** SharedValues remain writable 32-bit
   scalar cells; frame time is a read-only frame signal, not a SharedValue
   (RFC 0490 §3.4.3); uniform *blocks* are the sanctioned aggregation.
4. **The real-input principal.** Agent-path synthetic input never
   masquerades as real input; "motion works" claims on physical platforms
   require the real-input evidence tier. Correctness-bearing outcomes exit
   the graph only via `motion.command` receipts — never best-effort
   callbacks.
5. **The Sandwich Model.** Hosts as sink appliers still apply through
   platform-natural machinery (CATransaction batches, DOM style/transform
   writes, Direct2D properties). Rust owns *evaluation*, not presentation.
6. **Lowest sufficient tier, as an execution discipline.** RFC 0099's tier
   instinct survives — re-founded (§4.1): tiers stop being authoring
   surfaces and become *lowerings* chosen by measured cost. Taking a higher
   execution tier than needed remains a defect (RFC 0422's discipline;
   RFC 0491 OQ10's for layout).
7. **Graceful degradation.** A sink family a host does not implement leaves
   bindings dormant with a diagnosable reason — never a crash, never silent
   divergence between what the plan claims and what the host applies.

## 3. What this program consumes (decided elsewhere — not re-litigated here)

| Ruling | Owner | What 0492 does with it |
| --- | --- | --- |
| Rust owns motion-graph evaluation, in the main-owned motion domain; hosts become sink appliers; Swift graph mirror + MSCH fencing collapse | RFC 0491 §7.1 | Builds every workstream on the single evaluator |
| Clock phases are a Rust-side generated authority; tick source host-owned but **interchangeable** (display link / rAF / virtual) | RFC 0491 §7.2 (r8), RFC 0490 §3.3 | M-A ships the virtual mode and the agent surface |
| `exact-motion` crate (value graph + drivers + recognizers + transport + navigation model), **wasm-compilable from day one**; gpu-service consumes it without `exact-kernel` | RFC 0491 WS-G (r8) | M-F ships the web leg as the same evaluator; no parallel TS implementation |
| The dirty-set instrument (per-binding affected-set sizes, changed-geometry receipts, patch→receipt timing endpoints) and incremental-relayout **result equality** — the substrate for the budget claim | RFC 0491 WS-H (r14/r15 §7.4 — the ≤1 ms/≤64-node **number itself is owned by this program**, not consumed) | This program owns the number, the gesture classification (which animations may claim ≤64), and M-D's gate: M-D starts when the instrument exists and this program's target is demonstrated on it (§5); a miss commissions kernel skip-work as scheduled 0491 work, never a 0491 freeze blocker |
| SharedValue → uniform blocks; GPU → SharedValue command-shaped; frame time a read-only signal; transient-lane values excluded from GPU bindings in v1 | RFC 0490 §3.4 | Binding-target families adopted as-is into the sink taxonomy (§4.4) |
| The animated-layout mechanism ruling (islands vs constraint-buffer relayout vs baked timelines) is empirical, post-WS-H; layout motion routes only through declared sink families and clock phases | RFC 0491 OQ10 | M-D executes the ruling with evidence; this RFC never promises general layout animation ahead of it |
| The runtime motion-validation tier is deletion-bound under plan-compiled motion — memoize, don't invest | RFC 0491 r8 (joint with RFC 0480/LLP 0485) | M-C performs the deletion |
| Interactive-motion state machines: named states/inputs, interruptible transitions, deterministic resolution, Rive as importable source | LLP 0396 (Accepted) | M-E implements it on the Rust evaluator, plan-encoded |
| Frame-pacing presentation evidence | LLP 0354 | M-A extends it with virtual-clock receipts |

The division of labor is deliberate: **0490 owns where pixels and uniforms
come from, 0491 owns where evaluation and relayout live, 0492 owns how motion
is authored, carried, claimed, and proven.** Where a boundary is genuinely
shared, it appears in §8 as a joint ruling, not silently in two documents.

## 4. Design: the target state

### 4.1 One representation, lowered execution — the `transition=` ruling

Today `transition=` and the SharedValue graph are two sources of truth with
an unowned seam (`kernel/src/transition.rs` + host transition machinery on
one side; the graph on the other; the handoff ticket in between). The naive
unification — "everything runs in the evaluator" — was considered and
rejected in pre-draft review *(that ruling was **inverted on 2026-08-20**:
LLP 0505 r3 row 7 makes evaluator-everywhere the v1 shipping shape, and
the power argument below moves to the dated checkpoint)*: RFC 0099's Tier 1 hands a whole animation to
Core Animation / CSS at **zero per-frame cost**, and an ambient theme-color
fade should not wake an evaluator every frame on battery. The ruling that
gets both:

- **Representation unifies.** `transition=` compiles to a two-endpoint
  driver *in the one graph*. There is exactly one description of every
  animation in the system — compiled, plan-referenced, inspectable, seekable. The
  Tier-1/Tier-2 *handoff* defect class dies here, because there is no
  second representation to hand off to or race against. Honesty about the
  other two named tickets: representation unification alone does **not**
  fix them — the arming-boot race and mid-gesture instability are
  *presenter-lifecycle* defects (bindings and recognizers attached to
  views that boot or state writes replace; an in-flight pointer stream
  that must survive the replacement). Their repair is the explicit
  attachment work M-B/M-C carry: **stable per-instance binding
  identity, generation-fenced prepare/commit activation, reattachment
  rules on view replacement, in-flight recognizer survival, and
  attachment receipts** (binding identity vocabulary is OQ9).
  > **Third replacement case, recorded 2026-08-26 (RFC 0540 §1.6 /
  > §8.5 row 64).** Virtual visibility adds a view-replacement cause
  > beyond boot and state-write replacement: a row entering `hidden`
  > has its native subtree and host view **released** while its logical
  > owner persists. Two rules follow, both decided by Accepted RFC
  > 0540 and recorded here as this program's fold; **delivery is
  > post-window** (0540 §8.6, D2) and no M-milestone gate moves.
  > **(a) The sink detach rule.** SharedValues and drivers owned by a
  > hidden row **persist and keep evaluating on true elapsed time**
  > (RFC 0099's "time advances with the world"; constraint 1 here) —
  > hiding is not a pause. Only the **presenter sinks** detach, and
  > only while the row has no host view; they re-attach on `prerender`
  > under the §4.1 rules above (per-instance binding identity,
  > generation-fenced activation). A hidden row therefore resumes at
  > the value the world reached, never at the value it left.
  > **(b) The hidden reattachment receipt.** `hidden → prerender`
  > re-attachment emits an **attachment receipt** like every other
  > reattachment in this section; it is not a silent rebind. In-flight
  > recognizers **survive** the hide (0540 OQ2 closed on exactly this
  > program's §4.1 guarantee), so **visibility is not a cancellation
  > boundary** — a gesture that starts on a row and scrolls it out of
  > the band is still that gesture. The receipt is what lets a test
  > distinguish "survived" from "silently re-armed", which is the
  > whole point of the attachment-receipt vocabulary.
  > Scope: this adds a *case* to the existing rules, not a new
  > mechanism, so the OQ9 binding-identity vocabulary is unchanged and
  > sinks stay `transform`/`opacity` per RFC 0099 v1.
- **Execution may delegate, as a lowering — but not in v1 (DECIDED,
  LLP 0505 r3 row 7, Charlie 2026-08-20).** The delegation default is
  inverted: **v1 ships evaluator-everywhere on all platforms** (the
  recorded M-B abort-fallback is the v1 shipping configuration, not an
  abort outcome), and per-platform CA/CSS handback leaves the critical
  path — it returns only on measured power need, evaluated at a dated
  Apple checkpoint (**2026-10-01**, a placeholder date the owner may
  move). The paragraphs below define what delegation means *when it
  returns*; nothing in them is v1 work. A pure two-endpoint
  composited-sink animation (transform/opacity/paint scalar, no live graph
  edge feeding it) may be lowered to CA/CSS execution. Delegation carries
  **declared handback semantics**: any retarget, interruption, velocity
  inheritance, or composition with a live graph edge pulls execution back
  into the evaluator at the delegated animation's *current presentation
  value* (platform mechanics in OQ1). Delegation is an optimization the
  compiler/evaluator chooses by measured cost — never something an author
  spells, and never observable in semantics (a delegated and an evaluated
  run of the same animation must be claim-indistinguishable).
- **The ruling instrument is power, not architecture.** RFC 0491 r8 records
  that WS-F's easing/driver unification supports either shape, so the
  delegate-vs-evaluate boundary is decided by a power/wakeup measurement on
  reference hardware (OQ2), starting from the ambient-fade case that
  motivated it.
- **The parity holes close as a side effect.** Platforms with no native
  transition machinery (Android's skipped `SetTransition`, Windows) simply
  never delegate — they run the evaluator. One representation plus an
  optional lowering closes RFC 0099's three-platform divergence without a
  third implementation; web's spring-kind approximation is retired because
  spring execution belongs to the evaluator wherever handback fidelity
  requires it (OQ1).

RFC 0099's three tiers are thereby **re-founded as lowerings from one
representation**: its routing table ("state change → Tier 1 by default…")
survives as compiler lowering policy, not as authoring guidance. The
amendment to 0099 lands at acceptance (§8).

### 4.2 Determinism: the graph under a virtualizable clock

With constraint 1 (purity) and the §7.2 tick-source ruling, motion becomes
deterministic in the strong sense: a recorded (clock schedule, input log)
pair replays to bit-identical value sequences **on a pinned evaluator
build** (cross-architecture identity additionally needs M-A's
deterministic-math obligation; visual evidence is pinned-capture, never
cross-machine — the M-A exit is the normative statement). The program
surfaces this three ways:

- **Virtual-rate sessions for Acto** (LLP 0442's proposal, operationalized):
  an agent session option that drives the clock virtually — fixed-step
  ticks, seek-to-t, run-to-quiescence — so "settle" becomes a computed
  property, not a timing heuristic. Armed-bracket-style waiting is replaced
  by clock-driven receipts.
- **Seekable evidence**: `exact_screenshot` (and the LLP 0354 presentation
  evidence) can be taken *at* `t` of a transition, identically across runs
  on a pinned capture configuration.
- **Replay**: the (schedule, input-log) pair is the motion slice of a
  journey capsule; divergence on replay is a detected defect, not noise.

Delegated executions (§4.1) participate: under a virtual clock, delegation
is disabled and everything evaluates — one more reason delegation must be
semantically invisible.

### 4.3 Motion is plan data — inside Accepted 0485's own shape

Accepted LLP 0485 already decided the v1 seam this section must respect:
the plan **references** motion programs and does not subsume them
(§3.3.14), the Motion segment's v1 payload is binding rows, and
**plan-encoded motion is explicitly out of v1** (§3.8). This program does
not silently reverse an Accepted exclusion. The conformant restatement:
the compiled motion graph — topology, driver parameters, gesture
bindings, state machines, sink bindings with their budget claims — is a
**compiled `exact-motion` artifact that the plan references as a Class S
sibling program** (0485 §3.3.14's own pattern, the same shape the
graphics scene sub-programs use). The artifact is data; nothing walks
it; the evaluator runs it. What changes relative to today is *what the
referenced artifact is* (compiled data for the Rust evaluator, not a JS
worklet graph) — not where it lives.

**Open decision (0485 amendment, Charlie):** if implementation shows the
graph bytes must live *inside* the plan container (single-file delivery,
content-addressed sharing of motion programs), that is an explicit,
named amendment to 0485 §3.3.14/§3.8 with a rationale for reopening a
v1 exclusion — proposed through 0485's format authority, never enacted
by this document. Recommended default: keep the Class S reference; it
already satisfies every consequence below.

Consequences, in order of importance:

1. **The no-JS tier is first-class by construction** (§4.6) — the plan
   carries the program; no Hermes required.
2. **LLP 0449's embedded stub is deleted structurally**: there is no JS
   slot sampler to stub, so a shipped embedded app runs the same layout
   sinks as everything else (once M-D opens them).
3. **The runtime motion-validation tier is deleted for plan-compiled
   graphs only** (consuming RFC 0491 r8): a compiled graph is validated
   at compile time; the interim memoization (`Validated<T>`) is
   scaffolding, not investment. **Escape-hatch graphs keep bounded
   runtime admission** — a runtime-constructed graph cannot be
   compile-validated, so it is admitted through a typed builder whose
   invalid structure, types, lifetimes, and budget violations are
   unrepresentable or rejected at construction; "delete the validation
   tier" never means "accept unvalidated dynamic graphs."
4. **Runtime graph construction becomes the fenced escape hatch** — an
   explicit capability for genuinely dynamic cases (agent-generated UI,
   Design Mode live editing), with a declared ceiling (OQ3, the motion
   analogue of LLP 0481's dynamic-indexing question). The paved road is
   compiled.

### 4.4 The sink taxonomy, completed

Binding-target families of the one graph, each with a registered authority
and a cost class:

| Family | Cost class | Authority | Status under this program |
| --- | --- | --- | --- |
| Composited (transform, opacity) | apply-on-main, delegable §4.1 | RFC 0099 v1 | shipped; gains handback vocabulary |
| Paint scalars (color, radius, border) | paint-dirty, delegable | LLP 0449 | completed by M-B |
| Uniform blocks / scene-node properties | GPU phase | RFC 0490 §3.4 + RFC 0115 | consumed as decided there |
| Playheads (Rive/Lottie progress, video time) | clock consumers | LLP 0394/0396/0393 | regularized by M-E |
| **Layout** (island properties; constraint-buffer relayout) | **per-binding ≤1 ms claim** | LLP 0313/0349/0449 + RFC 0491 WS-H/OQ10 | **opened by M-D, gated §5** |

The new discipline is the **per-binding budget claim** (RFC 0491 r8): a
layout-targeting binding carries a compiler-checkable affected-set bound; the
kernel supplies the affected-set measurement; the claim is verified the same
three ways a contract block is — statically, in tests against the LLP 0487
corpus, and live through the agent API. Two semantics are deliberately
separate (per 0485's cost-claim discipline): the **static bound** may be
symbolic for dynamic collections (`base + rows × perRow`-shaped — a
scalar bound over an unbounded `each` is either wrong or vacuous), with a
declared runtime-violation behavior when the live affected set exceeds
it; and the **timing claim** (≤1 ms) is always a separately *measured*
gate — a node-count bound never proves elapsed time. "This animation stays in budget"
becomes a machine-checked sentence, which is the lesson LLP 0486 §-instrument
taught: a budget nobody measures against is doctrine, not engineering.

**Adjudicated 2026-08-26 (LLP 0492.001).** The instrument landed and the first
measurement over the pinned 1K fixture selected §4.4's own symbolic-bound
sentence: `reorder` measures **k=999 of n=1000** and is classified
`collection-extent` — a named M-D customer, exempt from the *scalar* ≤64 and
owing a sound/tight/parametric symbolic bound instead; the four point patches
(`keyboard-avoidance` k=1, `single-leaf-width` k=3, `container-size` k=4,
`single-leaf-height` k=5) claim the scalar bound. The measurement also shows the
two halves are not proxies for each other: the patch→receipt endpoint is O(n) by
construction today, so the ≤1 ms half passes every case and discriminates
nothing on k (`issues/20260826-ws-h-patch-to-receipt-cost-flat-in-k.md`,
commissioned back to 0491 per §7.4). Gates: `motion-md-affected-set-budget`
(every push) and `motion-md-relayout-timing-budget` (milestone).

Bindings also acquire identity (`issues/20260803-motion-bindings-need-a-testid.md`):
every binding is addressable by the same testId vocabulary the rest of Acto
uses, so claims, receipts, and Design Mode knobs name the same thing.

### 4.5 State machines on top, commands out the side

M-E implements LLP 0396 — named states, named inputs, interruptible
transitions with its Accepted deterministic-resolution semantics — **on the
Rust evaluator, plan-encoded**, with `.riv` files as one importable source.
States and inputs are agent-legible by name through the existing registry
(no new wire names). Design Mode's H8 dial edits ride frozen LLP 0500
D1's spine: a knob edit is an **overlay patch on the same plan-patch
channel** as file-save HMR, and bake persists to source — the escape
hatch (§4.3) is the runtime mechanism underneath, never a
motion-private second HMR path.

The effect boundary hardens: gesture→graph edges are pure data;
correctness-bearing consequences exit only via `motion.command`
(generation-fenced, receipted). `motion.runOnJS` is **DELETED outright
(DECIDED, LLP 0505 r3 row 7): no compatibility shim, no deprecation
window — the existing in-repo corpus migrates to `motion.command` (the
correctness path) in the same change** — in a world with no-JS
apps, a frame-path callback into Hermes cannot be a load-bearing primitive.

### 4.6 Every tier, one evaluator

- **Rust Native roots** (LLP 0331/0333): a **producer-side motion
  builder** behind the generated App-ABI — the app crate declares
  graphs, drivers, recognizers, and claims as data/commands through the
  ABI, and the **host-tier evaluator executes them** in the main-owned
  motion domain. App-tier crates never link the evaluator or the kernel
  (LLP 0331's producer/host layering, restated by RFC 0491 §2): the
  same authored surface as Contract, no Hermes in the process, both
  embedded and external placements served by one declaration. Expose/DRM
  (LLP 0406) inherits this for free.
- **Web**: the wasm build of the same crate inside LLP 0483's fixed runner
  (decided by RFC 0491 WS-G r8). The rAF scatter collapses into RFC 0490's
  `ExactFrameClock`; ambient-motion background-tab policy becomes a clock
  policy, not a per-callsite discipline. Dev and production run the same
  wasm evaluator in the fixed runner (frozen LLP 0500 D2 — OQ7 is closed);
  the React tier consumes it through the runner it embeds.
- **Apple/Windows/Android**: hosts as sink appliers over the §7.1 evaluator;
  Android and Windows gain transitions for the first time via §4.1's
  evaluator-execution fallback.

## 5. Workstreams

Each workstream names its exit evidence; none claims completion on
specification alone.

**M-A — Deterministic clock and agent evidence.** Virtual-rate clock mode
(consumes §7.2), seek/run-to-quiescence, the Acto session option, LLP 0354
receipt integration, and the **motion conformance corpus** — driver math,
interruption/retarget, handback, state machines — run against the native
evaluator and (later) the wasm build, in the LLP 0487 corpus mold.
*Exit:* a scripted gesture replays to **bit-identical value sequences on
a pinned evaluator build** across two runs and two machines — with a
named deterministic-math obligation before any cross-architecture
(native vs wasm) bit-identity claim, since closed-form springs use
platform `sqrt`/`exp`/`sin`/`cos` today (pin deterministic kernels or
quantize at the comparison boundary); **visual evidence** is LLP 0354
presentation evidence at a seekable `t` on a **pinned capture
configuration**, and layout-affecting cases are compared through the
LLP 0487 corpus instrument's semantic equality — cross-machine
screenshot identity is not claimed (text measurement, fonts, and DPI
are not virtualized by a clock; that is why 0486/0487 exist). The
settle-timing flake class has a regression test; corpus green on the
native evaluator.
*Starts now* — it depends only on the §7.2 ruling and the existing
`ExactMotionClock` phases.

**M-B — One representation, lowered execution.** *(Re-scoped by LLP
0505 r3 row 7: v1 is evaluator-everywhere on every platform; the
handback protocol (OQ1) and power instrument (OQ2) leave the critical
path and are taken up only at the dated Apple checkpoint if measured
power demands delegation.)* `transition=` recompiled
into the graph; ALL platform transitions via evaluator execution in v1;
the handback protocol per platform (OQ1) and the
power instrument with its delegation ruling
(OQ2) deferred to the checkpoint.
*Exit:* the tier-handoff, mid-gesture-stability, and arming-race tickets are
closed — the handoff ticket by construction (the seam no longer exists,
demonstrated by corpus cases that previously straddled it), the
arming-race and mid-gesture tickets by the §4.1 attachment work landing
(binding identity, generation-fenced prepare/commit activation,
reattachment on view replacement, in-flight recognizer survival, each
with attachment receipts) — never by representation unification alone;
the delegation ruling is
recorded with its measurements; no authoring surface names a tier.
**Historical abort gate (now the v1 ruling):** the "abort" branch
below — evaluator-everywhere at a measured, accepted power cost — **is
the decided v1 configuration** (2026-08-20, LLP 0505 r3 row 7), not a
contingency. If the dated checkpoint later attempts delegation and
handback fidelity fails in practice — presentation-value reads that lie,
platforms that cannot interrupt cleanly, spring velocity unrecoverable —
delegation aborts back to this ruling with its own receipt.
*After* RFC 0491 WS-F/WS-G land (Phase 4).

**M-C — Plan-compiled motion.** The referenced-artifact format under
0485 §3.3.14's Class S pattern (§4.3, with the in-container question as
its recorded open decision); compile paths from Contract motion and
`transition=`; the escape-hatch API with its ceiling (OQ3) and typed
runtime admission (§4.3 item 3); deletion of the runtime validation tier
for compiled graphs.
*Exit:* the Caltrain app and Motion Lab run their motion entirely from plan
segments with zero per-frame JS; an embedded release build executes compiled
motion with no sampler shim.
*Grammar work starts early* (freeze risk with 0485); execution lands with
M-B.

**M-D — Layout-motion customers.** **Gated: this workstream does not start
until RFC 0491 WS-H's dirty-set instrument exists and this program's own
≤1 ms/≤64-node target is demonstrated on it over the LLP 0487 corpus —
demonstrated, not specified. The number, the classification of which
gestures may claim the ≤64 budget, and this gate are 0492's rulings
(RFC 0491 §7.4).** (LLP 0486's lesson, named: a
budget without a measuring instrument is the failure mode this program
refuses to inherit.) **The gate is discharged as of 2026-08-26**: the instrument
exists (`kernel-p5-dirty-set-instrument`), the target is measured on it, and the
classification is ruled in **LLP 0492.001** — read that before assuming `reorder`
owes the scalar ≤64 (it does not; it is `collection-extent`) or that the ≤1 ms
half currently discriminates (it does not). Then: execute the OQ10 mechanism ruling with evidence,
open the layout sink family with per-binding claims (§4.4), and ship the
four named customers — shared-element/FLIP (consuming RFC 0491 WS-C commit
receipts as before/after geometry), reorder animation, container-size
motion, keyboard avoidance.
*Exit:* each customer demo runs on iOS, macOS, web, and a Rust Native root,
inside its claimed budget, measured per binding.

**M-E — Interactive state machines.** LLP 0396 on the Rust evaluator,
plan-encoded; Rive import; Design Mode H8 knobs over the escape hatch;
agent-legible states.
*Exit:* the 0396 acceptance gates pass on the shared evaluator; a state
machine authored in Design Mode round-trips edit → bake → plan; an agent
reads and drives states by name; composed with RFC 0490's jelly-slider
workload (gesture → state machine → shader uniform, zero per-frame JS).
*After* M-B/M-C; parallel with M-D.

**M-F — The no-JS and web legs.** The Rust-root motion API under
LLP 0331/0333 governance; the wasm evaluator in the fixed runner with a
size/load budget (OQ8); background-tab clock policy.
*Exit:* a Rust Native root runs the M-A conformance corpus with no Hermes
in the process (plus one M-D customer **where M-D has landed** — that leg
explicitly depends on M-D and detaches from evaluator parity if M-D
slips); the wasm build passes the same corpus **under M-A's oracle** —
bit-identical value sequences only under the named deterministic-math
obligation (pinned deterministic kernels or quantization at the
comparison boundary), otherwise the LLP 0487-mold semantic-equality
comparison. Native-vs-wasm byte-for-byte identity is never claimed
without that prerequisite (platform `sqrt`/`exp`/`sin`/`cos` differ
across architectures).
*After* RFC 0491 WS-G.

**M-G — Deletion sweep and the iOS finish line.** Delete: the kernel/host
transition machinery as a source of truth (its execution path survives only
as M-B's lowering), the web TS evaluator, the arming apparatus, the runtime
validation tier, `runOnJS` (**deleted outright — no shim**; in-repo
migration to `motion.command`, LLP 0505 r3 row 7). Then ungate motion on iOS.
*Exit:* motion default-on on iOS with the
`20260717-motion-v1-physical-acceptance-evidence.md` physical evidence
retained under the real-input rule; the deletion list verified
population-zero in the RFC 0491 WS-F mold.

## 6. What this deletes or simplifies

Via RFC 0491 (consumed, listed for the complete picture): the 8.4K-line
Swift graph mirror, per-frame MSCH fencing, the dead `MotionClockRegistry`,
O(N²) JSON motion outcomes and the per-frame schema-envelope revalidation.
Owned here: the second motion representation (`transition=` as an
independent system), the web TS evaluator, the Tier-1/Tier-2 handoff and
arming machinery, the runtime motion-validation tier, frame-path `runOnJS`,
LLP 0449's embedded sampler stub, and — as a consequence — the seam-shaped
majority of the open motion ticket cluster.

## 7. Sequencing

Against the sibling programs: **M-A immediately** (needs only §7.2 and the
existing Apple clock; web determinism completes when RFC 0490 M2's
`ExactFrameClock` lands). **M-C's grammar early** (joint with LLP 0485
before the plan format hardens). **M-B and the rest of M-C after RFC 0491
Phase 4** (WS-F/WS-G). **M-D strictly after the WS-H instrument exists and
this program's target is demonstrated on it.**
**M-E/M-F** follow their stated dependencies. **M-G last.** The program has
standalone value even if the siblings slip: M-A's determinism and corpus pay
for themselves against today's evaluator.

## 8. Joint rulings and amendments

- **With LLP 0485 (flat plan):** conformance, not amendment, by default
  (§4.3): the compiled motion artifact is a Class S referenced program
  under 0485 §3.3.14; the v1 Motion segment keeps its binding-row
  payload; the referenced-artifact format (topology, driver params, sink
  bindings with budget claims, state machines) is this program's to
  specify with 0485's format authority consulted on the reference
  encoding. **One named narrow amendment is owed even on the default
  path:** 0485 §3.3.14 currently names the referenced evaluator as "the
  separate LLP 0099/0297 worklet runtime" and classifies today's motion
  programs as S-code — this program retargets that one sentence
  (evaluator = `exact-motion`) and adds the S-code→S-data paved path for
  compiled artifacts; a one-sentence, explicitly proposed 0485
  amendment, not silent conformance. Moving graph bytes *into* the plan
  container is the §4.3
  open decision — an explicit 0485 §3.3.14/§3.8 amendment if ever taken.
  The escape-hatch ceiling (OQ3) is decided jointly with LLP 0481.
- **With RFC 0490 (the M3 unblock, named):** §4.1/§4.6 *are* the atomic
  evaluator decision the 0099↔0491 amendment expected in this document —
  one Rust `exact-motion` evaluator on every tier, the web leg as the
  wasm build in the fixed runner, CA/CSS surviving only as M-B's
  delegated lowering. RFC 0490 M3 (both legs) unblocks on this ruling's
  acceptance; 0490/0491 need only cite it.
- **With RFC 0490:** binding-schema unification for uniforms vs scene
  `shaderUniforms` is 0490 OQ10 — this program is a consumer of the outcome;
  presentation-evidence extensions ride LLP 0354.
- **With RFC 0491:** already recorded as its r8 amendments (tick-source
  agnosticism; wasm-compilable `exact-motion`; per-binding budget claims;
  deletion-bound validation tier), plus §7.4's r15 transfer: **this
  program owns the ≤1 ms/≤64-node claim and its gesture classification**;
  the standing asks of the kernel program are the WS-H dirty-set
  instrument and result equality (already 0491 exit obligations) — and if
  the instrument shows the target missed, further kernel skip-work is
  commissioned as scheduled 0491 work with this program as the customer.
- **RFC 0099 amendment (at acceptance):** the three tiers re-founded as
  lowerings from one representation (§4.1); 0099 remains the authority for
  driver semantics and the Tier-2 substrate; its implementation-status
  table gains the M-B parity closure.
- **LLP 0449 amendment (at acceptance):** the sink-family registry becomes
  this program's §4.4 taxonomy; the embedded-stub defect row closes with
  M-C.
- **`motionEvaluatorEpoch` (with RFC 0491 Phase 4 / LLP 0510
  admission).** *(Amendment `A-0492/0510-MOTION-EVALUATOR-EPOCH`,
  RFC 0491 §8 — drafted in LLP 0491.001 §7, ratified by Charlie
  Cheever 2026-08-23, decision relayed via orchestration session
  exact-9e.)* The canonical evaluator identity is a domain-separated
  SHA-256 (proposed tag `EXACT.MOTION.EVALUATOR-EPOCH.SHA256.V1\0`)
  over a generated canonical package of exactly four members:
  1. **canonical evaluator sources** — the `exact-motion` evaluator
     module set (closed-form driver math, phase consumer, recognizer
     state machines), enumerated by path + content digest by the
     generator, never a whole-crate hash (docs and tests do not move
     the epoch);
  2. **the deterministic-math profile** — M-A's pinned kernels /
     quantization boundary declarations;
  3. **the feature set** — the resolved cargo feature selection
     affecting evaluation;
  4. **the exported evaluator ABI** — the generated evaluator
     interface surface (the phase-consumer API signature set).

  The build emits the epoch beside the artifact; a host computes its
  **own local epoch from the same generator** and artifact admission
  compares them (LLP 0510's tuple carries the artifact side — 0510
  §13). An evaluator-only change MUST move the epoch even when every
  Motion wire schema/layout byte is unchanged; a wire-only change
  MUST NOT move it; neither the epoch nor the wire `motion` object
  may be derived from or substituted for the other. RFC 0491 Phase 4
  and the §7.1 Swift-evaluator deletion wait for this detector to be
  live. Ratified with the draft's positions on the owner decision
  points: toolchain identity stays excluded (it lives in the tuple's
  `toolchain` object — a rustc bump alone does not move the epoch; a
  rustc change that changes evaluator *behavior* is a
  deterministic-math-profile defect and surfaces in member 2); member
  1's boundary is the evaluator modules only (transport/FFI are
  wire-dimension territory); and the epoch is required in the tuple
  for every artifact, motion declarations or not (one answer shared
  with 0510 §13's decision point (a)).

## 9. Honest costs and tensions

- **Delegation would keep two executors alive — which is why v1 does
  not take it.** (DECIDED 2026-08-20, LLP 0505 r3 row 7: v1 is
  evaluator-everywhere at a measured, accepted power cost.) If the dated
  checkpoint later takes delegation, the seam moves from representation
  (everywhere, unowned) to handback (one narrow, declared protocol
  point) — and if handback proves leaky, the recorded fallback is this
  v1 ruling. The checkpoint's measurements are where the delegation bet
  is taken or declined.
- **A wasm evaluator has a size and load cost** on the tier where bytes are
  scrutinized most (LLP 0483's caching argument). The budget is named in
  OQ8 and enforced in M-F's exit, not hoped about.
- **Plan-compiled motion trades authoring fluidity for totality.** The
  escape hatch exists precisely so Design Mode and agent-generated UI stay
  live; its ceiling (OQ3) is the same knife-edge LLP 0481 walks for
  dynamic indexing, and it should be decided with 0481, not independently.
- **This program is mostly a consumer of two unlanded programs.** If 0490
  or 0491 slip or reshape, M-B through M-F move with them. Mitigation:
  M-A's standalone value, and §3's consumption table making every
  dependency explicit enough to re-plan.
- **Cross-thread choreography for layout sinks** depends on OQ10's chosen
  mechanism; the main-owned domain plus runtime-thread relayout implies
  one dirty-set message per frame in the constraint-buffer arm. The
  structural rule (declared sink families and clock phases only, no ad-hoc
  channel) is already fixed by 0491; the latency accounting lands with
  M-D's evidence.

## 10. Open questions

- **OQ1 — Handback mechanics per platform.** Reading the current
  presentation value mid-delegation (CA presentation layer; web
  `getComputedStyle`/WAAPI state) with enough fidelity to hand velocity to
  a spring. Where a platform cannot answer (CSS transitions carry no
  velocity), does spring-kind simply never delegate there, or does
  delegation carry a shadow evaluation for handback state only?
- **OQ2 — The delegation ruling instrument.** Which power/wakeup
  measurement, which reference devices, what threshold separates "delegate"
  from "evaluate"; the ambient theme-fade and jelly-slider as the two ends
  of the spectrum.
- **OQ3 — The dynamic-graph ceiling.** How much runtime graph mutation the
  plan admits before an escape-hatch fence (motion's analogue of LLP 0481's
  dynamic-indexing question; decide jointly). *(**ANSWERED by citation,
  2026-08-25** — RFC 0518 §4 as adopted at the W2 venue (Charlie Cheever
  via exact-9e, the LLP 0508 §14 "Dynamic Ceiling" amendment): runtime
  graph construction is Ruling B's Lane 2 — the island's output is a
  motion-graph artifact in the same referenced-artifact class compiled
  motion uses (LLP 0485 §3.3.14), admitted through the bounded validator
  this program already retains (§4.3 item 4), patches riding the
  LLP 0500 D1 overlay spine; nothing motion-private is added.
  One-sentence fold per 0518 ask 3.)*
- **OQ4 — State-machine encoding for agent-generated UI.** Plan-encoded
  machines cover authored surfaces; does genui-class dynamic UI get
  runtime-registered machines under the OQ3 ceiling, or a restricted
  template vocabulary?
- **OQ5 — resolved (DECIDED, LLP 0505 r3 row 7).** No deprecation
  timeline: `runOnJS` is deleted outright with the in-repo corpus
  migrated to `motion.command` in the same change; no shim ever ships.
- **OQ6 — Baked timelines.** RFC 0491 OQ10's third mechanism; the
  `20260801-motion-baked-timeline-measurement.md` ticket's measurement
  decides whether it exists at all (per RFC 0099's recorded stance: not a
  missing tier; measure first).
- **OQ7 — closed (r2) by frozen LLP 0500 D2.** Dev runs the production
  runner — the wasm evaluator serves the web dev loop and production
  alike; there is no TS dev shim (the accretion risk this OQ feared is
  retired by decision, not vigilance). The TS web evaluator is on M-G's
  deletion list. The React tier consumes the same evaluator through the
  runner it embeds.
- **OQ8 — The wasm size/load budget** for the fixed runner, and whether the
  evaluator ships in the base runner or as a demand-loaded segment. The
  budget row is one shared 0483 runner-budget object — `exact-motion`,
  `exact-router-core`, and `exact-aquifer` all wasm-compile into the same
  fixed runner, so LLP 0483 owns the combined byte/load budget and this
  OQ contributes motion's line to it.
- **OQ9 — Binding identity vocabulary.** The testId-for-bindings shape
  (`20260803-motion-bindings-need-a-testid.md`): per-binding, per-instance
  under `each`, and how claims and Design Mode knobs address the same id.

## Revision history

- **r1 (2026-08-20)** — initial draft, from the 2026-08-20 motion
  assessment against RFC 0490/0491. Incorporates pre-draft cross-agent
  review: (1) the `transition=` ruling is
  unify-representation / delegate-execution-with-declared-handback —
  execution delegation to CA/CSS survives as a lowering decided by power
  measurement, preserving RFC 0099's Tier-1 zero-per-frame-cost property
  while killing the representation seam (the reviewer's middle path,
  verified against 0099's three-tier model and 0491 r8's recorded
  either-ruling support); (2) M-D is explicitly gated on WS-H's number
  being demonstrated on the LLP 0487 corpus instrument, not merely
  specified — refusing the "doctrine with no instrument" failure mode
  LLP 0486 named.
