# RFC 0100: Interactive Navigation Transitions

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Router, Motion, Tooling, Layout
**Author:** Charlie Cheever / Claude
**Date:** 2026-03-28
**Revised:** 2026-07-13 (as-built reconciliation after ENG-24695: P1 and
P1.5 code paths have landed, including native publication, retained/leased
dual scenes, Rust controller + DecisionCell, presenter composition, unified
start → resolution → settled outcomes, tab-aware commits, capability
promotions, Acto projection, fixtures, and the registered automated harness.
Status remains Review because the physical ProMotion fidelity bundle and the
required shadow-readiness evidence set (the P1 stack-only artifact plus the
P1.5 tab-pop cohort artifact) are absent; P2–P5 stay gated.)
2026-07-05 (review round 1 — two GPT reviews, see
`llp/reviews/0100-interactive-navigation-transitions.gpt.md` —
incorporated: normative gesture readiness snapshot §4a; two-phase
logical-commit / visual-settle lifecycle with state machine §4b; three
progress channels §4c; operational P1 fidelity gate with numeric
thresholds and a runtime-thread-pressure test; retained-scene rule pinned
in D4; §5 reconciled to D1's clone-first; Apple arbitration subsection in
§9; reduced-motion reconciled with RFC 0046; §8 worklet API marked
illustrative/post-P1; pre-P3 nonvisual shared-element checklist in D1);
2026-07-05 (design closure: the five open questions are resolved
in the new Design Decisions log; a Phasing section names the P1 vertical
slice — iOS finger-tracking swipe-back — as the fidelity experiment gating
the rest; substrate reconciled to LLP 0297's two-domain model; status
Draft → Review); 2026-07-11 (review round 2 — fresh Fable 5 and Codex
gpt-5.6-sol reviews, see `llp/reviews/0100-…​.fable.md` / `.codex.md` —
incorporated: declared-amendments block naming the deltas this RFC makes
to RFC 0010 D6, RFC 0046, and RFC 0052; arbitration contention deferred
to LLP 0336 §6.1's arena (edge-back = the `RouterHistory` claim) and the
P1 slice rebased on the shipped 0336/0297 substrate; §4a snapshot split
into router-published BackDisposition + main-owned presenter readiness
with a scene lease; §4b gains the release-before-confirmation and
racing-navigation rows and pins scene retention through the rendered
route set (no detach/destroy during settling); one direction-aware
commit/cancel rule replacing three inconsistent statements; §3 collapsed
to a single velocity-continuous progress spring; §4 pseudocode rewritten
as the confirmation path; D4 reconciled to RFC 0010's lifecycle
(post-commit revalidation on the retained-scene path); §11 aligned to
the shipped RFC 0052 v2 wire surface; §10/D5 Android rows corrected to
LLP 0310 (system owns both edges; `OnBackInvokedCallback`); P1 gate
re-instrumented — signpost/display-link timing, replayed touch traces,
`EXACT_RUNTIME_THREAD=1` provenance, silent-fallback-fails-the-gate);
2026-07-11 (review round 3 — fresh Fable 5 and Codex gpt-5.6-sol reviews
on the round-2 revision — incorporated: RFC 0010/0046 registration edits
per 0010's Normative Home Rule (Invariant 7 transition-retention entry,
lifecycle carve-outs, handle settled signal, 0046 TransitionHandle
amendment note); §4a gains `handlerDisposition` + `dataDisposition`
(RFC 0053 pop semantics gate the fast path), an atomic target-bound
`acquireSceneLease`, cascade re-run at confirmation, and a bounded ack
timeout with tombstone; §4b racing-navigation row aligned to LLP 0336's
structural-invalidation rule (immediate cancel, not freeze-until-release)
and the arena InteractionLease distinguished from the presenter
SceneLease; Principle 3 scoped to Exact-owned recognizers with external
sources (Android predictive back) delivering terminal resolution; §3
sketch rewired so tracking and settle share one progress→properties path
with the reduced-motion branch; §7 blocked-attempt UX aligned to
CT-061-08/14 (intent-threshold blocker UI, superseding round 2's
silent-fail rule); §9's second arbitrator sketch removed in favor of the
0336 arena; §11 realigned to the shipped v2 wire (string from/to,
transitionInterrupted payload, agent-operations registry); P1 gains the
ambient scene-retention, slab-allocator, and arena-FFI build rows, a
Release-optimized benchmark configuration with in-process injection in
BOTH fixtures, an outcome-parity matrix, and a pinned fixture spec);
2026-07-12 (review round 4, Fable half — the Codex half is pending a
provider usage-limit reset and reviews this revision fresh when it runs;
incorporated: the `tab-pop` eligibility gap made explicit and owned —
§4a gating scoped to global stack-pop with the rationale, D2 qualified,
new P1.5 increment for the tab-aware commit transaction; an ε-band
boundary tolerance on the outcome-parity gate so a correct
implementation can pass it; a minimal single-claimant `InteractionLease`
build row (the shipped 0336 slice is types + decide() + receipts only);
§4/§4a blocker-check sentences reconciled with the round-3 cascade
re-run; turn-token/timeout precedent attributions corrected to
`ExactPagerMotion.swift`; gesture-during-settling pinned (recognizer
fails until settled); the blocked-attempt inert-stream mechanism named;
the racing-navigation SSE outcome assigned (`interrupted`); a D6
in-place amendment pointer added in RFC 0010; velocity estimator,
reduced-motion fade geometry, parked-scene freshness, lease pre-warm
order, presenter-layer binding path, timeout magnitude/scope, and
0099-milestone alignment pinned);
2026-07-12 (review round 5 — fresh Fable 5 and Codex gpt-5.6-sol reviews
on blob f85cec388, judged against the same-day 0099 r5 / 0336 r9
authority revisions — incorporated: transition properties adopt LLP 0336
r9's presenter-composited channel (ENG-24306) with the live-update
failure row the pinned fixture was blind to; a shared-atomic
`DecisionCell` (pending → confirmed | expired via CAS) as the single
cross-domain linearization point for held commits, closing the
wedged-runtime tombstone race, with the timeout starting at
`settled-visual` entry; confirmation upgraded from cascade re-run to the
full canonical MATCH authorization with the commit transaction
re-validating atomically; the progress slot becomes a root-lifetime,
runtime-allocated reservation (0099 M1 is append-only — Motion never
allocates), with per-property values Motion-local; one degrade
vocabulary — tab-pop/handler/unusable-data/publish-gap/non-retained →
RFC 0061's discrete pop, blocker/dismiss → inert-stream blocked-attempt,
root/disabled → clean failure — fixing the §4a wording whose literal
reading regressed the shipped discrete swipe; `dismissDisposition` moved
out of P1 stack-pop gating (P2, dismiss claimants); the shipped
`TransitionDirective.preset` (navigation intent) split from the §4a
`motionDescriptor` (visual preset); backTarget/lease bound to
history-entry identity with RAII pre-warm release; token-bearing
promotion commits exempted from ENG-24189 equality elision with the
no-delta release condition defined; settling interruption made
resolution-preserving (commit→1, cancel→0 — never converting a cancel);
per-layer reduced-motion fade geometry; ε-band capped with a held-out
matrix; stock-side sampler parity; fullScreenBackSwipe excluded from P1
pending 0336 OQ7; P1.5 dependencies named (native tabs engine, `tabs`
capability, tab-scoped scene identity); shadow-mode promoted to a
required pre-P1.5 input; §11 velocity/interactivePhase registered in
0010 Contract 3 + RFC 0052; structural vs policy revisions split so
Motion can distinguish immediate-cancel invalidation from
checkpoint-handled flips)
2026-07-13 (round-6 reconcile, set-loop round 0 — applies BOTH round-6
queues per the recorded Option-A resume protocol, against converged
authorities (0336 r13 committed; 0099 r10). Value-locality sweep
completed where round 5 left contradictions: the Architecture diagram,
data-flow step 8c, the §3 controller struct + `apply_progress`, and
§4c channel 1 now all say Motion-LOCAL per-property floats applied via
the presenter-composited channel, with the reserved progress slot as
the only slab write; the §7 "Failed state" comment, the §4a
publish-gap bullet (one recognizer, two modes), and the
"release-origin" timer wording corrected; §11's settled-visual
endpoint is 1.0 only. The DecisionCell adopts 0099 r9's adjudication:
a control-plane record beside the outcome log (NOT a §4.5 slab slot),
`(transitionGeneration, state)` packed word (ABA-proof re-arm), AcqRel
ordering, epoch-tombstone-on-reset, plus the post-confirm-wedge
posture (watchdog-owned recovery) and the expanded interleaving
traces. The confirmation path now carries BOTH revision stamps + the
pinned target on the start record, splits structural (immediate nack)
from policy (forced-cancel-at-release, tracking never revoked)
divergence, adds RFC 0061's self-invalidation exemption (a cascade
publication stamped with the owning transitionId is not divergence;
the 0061-side wording is queued through the router-set loop), and
captures the full-MATCH result as an authorization token; the §4
commit transaction's re-validation scope is PINNED (cell assert +
token revisions + synchronous dispatch enforcement of blockers'
`when()`/access/layer-policy/target-identity — 0336 checkpoint 3;
effectful middleware never re-runs). Commit release is
outcome-specific everywhere (step 11a-e says settled AND token
observed). P1 executability: the presenter-composited channel gets a
build row (ENG-24306 composer reuse); native `registerBackHandler`
registration + publication added; the pre-P1.5 tab-pop fixture is
web-scoped (native twin has no tabs engine) and the shadow cohort's
tab-share honesty is stated; the lease row maps 0336's three
checkpoint duties onto named P1 router-side mechanisms; the reference
model gains DecisionCell traces + a gate mutation-test; milestone
alignment adds the M2 transport seed, the velocity-estimator re-gate
trigger, and P4's M6 dependency. Gate: realized-cadence requirement
(≥110Hz median or rerun; matched regimes both fixtures);
direction-consistent defined (monotone in signed velocity/progress);
ε derivation names stock's own near-boundary flips; failure suite
gains the at-root-modal (b-passive) row, the parked-scene-update row
with bounded first-reveal staleness, AX focus/inertness rows, and the
stale-eligible data complement (failed revalidation with unexpired
cache must NOT degrade — D4 and the §4a data row sharpened;
`dataUsableUntil` pinned as the gcTime eligibility deadline). §7's
inert stream is specified as a genuinely passive observer (no
prevention relationships — a begun system edge recognizer can prevent
scrolling even without canceling touches) and outcome (b) splits into
(b-blocker) vs (b-passive). §9's OQ7 sentence de-circularized (0100
owns the schema half in §2; 0336 owns the contention half). Status
unchanged: Review.)
2026-07-13 (set-loop round-1 reconciliation, from a split round —
artifacts `llp/reviews/0099-…-set.{fable,codex}-round1.md`; the Codex
round's verified Blockings drove the substance. The DecisionCell's
confirmed state now carries the verdict IN the word
(`confirmedAllow`/`confirmedCancel` — forced cancel is linearized, the
commit transaction asserts `confirmedAllow` specifically, and a
forced-cancel wake against a held commit is processed as a nack with
its own trace); ack/nack become level-triggered wakes over the durable
cell (lost wake = latency by construction); the word packs
`(rootInstance, transitionGeneration, state)` and the control region
inherits the value plane's physical-lifetime rules (teardown
tombstones, never frees; release only at shutdown behind quiescence;
re-tenancy watermark-gated) — closing the root-destroy/recreate ABA;
dispatch enforcement extends to `handlerDisposition` and the
`dataUsableUntil` wall-clock deadline (deadline crossing bumps no
revision, so commit re-checks it synchronously); root back handlers
become reachable per RFC 0061's handler-observability rule
(`handlerMounted` keeps the discrete path eligible with a
no-pop-target dispatch; ENG-24664 tracks the missing wire field and
the shipped handler/blocker ordering divergence); 0336 OQ16/OQ17
become explicit P1-entry gates for §9's arbitration rows; the handle
extension is re-registered as orthogonal phase + resolution (a scalar
cannot stay readable past `settled`); the reference model gains the
verdict-bearing cell traces, a per-field checkpoint matrix, and the
composed-Loom pairing with 0099's reset barrier; the M2 transport
seed is digest-stamped from day one; the P1 report check asserts
artifact presence; the reserved-slot row states the
proceed-on-spike-under-M1-semantics precondition. Status unchanged:
Review.)
2026-07-13 (set-loop closing fix pass, round 3 — the loop stopped
after a third consecutive split (Fable all-six READY / Codex all-six
NOT READY; artifacts `…-set.{fable,codex}-round3.md`); this revision
applies both rounds' verified line items: the MATCH-divergence branch
now CASes `nacked` durably (the round-2 sweep had covered only the
revision-mismatch branch — the closed-machine rule holds on every
path); the P1 build row's verdict-state list gains `nacked`; the §4a
staleness bullet replaces its blanket mismatch rule with the r12
event-classification matrix (structural → nacked; policy → re-run
decides; own-publication → bumps neither; Back-mode changes are
policy-only for a live lease); the §9 authority note drops its stale
raw-`canGoBack` spelling and gains the discriminant-ownership note
(0100 derives + publishes; 0336 cites; ENG-24664 owns wire fields);
the blocked-attempt contract is stated per attempt-burst, matching
0099's latch row. Status unchanged: Review — author adjudication on
the set closes the loop, codex dissent recorded.)
2026-07-13 (set-loop round-2 reconciliation — both round-2 reviews'
verified findings applied (artifacts `…-set.{fable,codex}-round2.md`).
The decision cell's state machine is CLOSED: structural nacks are a
durable `nacked` cell state CASed by the runtime before waking (the
revision-mismatch and MATCH-divergence paths), epoch reset tombstones
every armed state (confirm-then-reset-before-commit → reconcile), and
main's progress is wake-independent (scheduled cell loads at release
processing, settled-visual entry, timeout — the expiry-vs-
confirmedCancel/nacked branches are defined). forcedCancel is the
MATCH re-run's policy verdict ONLY — a bare policy-revision mismatch
just disables published-intermediate reuse, so benign bumps commit
normally (new failure row), matching the document's own never-a-
substitute doctrine and 0336's checkpoint semantics; self-invalidation
is resolved structurally (the owning gesture's cascade re-publication
does not bump policyRevision — a reentrancy signal, no origin-stamp
compare; 0061 wording queued via the router-set loop). Activation is
pinned: controller construction, cell re-arm, credit-bundle
reservation, and the start record all happen AT arena activation
(slop/axis-lock lease grant) — never at Began (steps 4–5 reworded;
blocked attempts get 0099's new per-root latch record class). §9's
arena gate becomes the four-mode Back discriminant (none |
passive-intent | discrete-claim | interactive-claim; blocker =
passive-intent — raw canGoBack is true during a blocker in shipped
code and must not gate claims). The Architecture diagram's scene
boxes are swept (composited channel, not SharedValue); the pager
parenthetical stops over-blessing the shipped rule (composition
defect cited per 0336); lost-ack-then-token and own-publication+
external-blocker join the failure suite. Status unchanged: Review.)
**Related:** RFC 0010 (Router v2), RFC 0046 (View Transitions), RFC 0061 (Native Navigation Polish), RFC 0099 (Motion), RFC 0007 (Behavior Primitives), RFC 0041 (Safe Area), LLP 0297 (threading model — owns the execution substrate; the Motion domain is main-owned per §4.4), LLP 0310 (Android navigation — owns predictive back and Android system-gesture specifics), LLP 0336 (cross-surface interaction model — owns v1 gesture contention via its §6.1 arbitration arena; this RFC's edge-back gesture is its `RouterHistory` claimant), LLP 0322 (runtime-thread rollout status — iOS remains opt-in, which P1's evidence rules account for)

## Summary

Define the complete system for gesture-driven, interactive navigation transitions in Exact: swipe-back, interactive sheet dismiss, shared element hero animations, and custom interactive transitions. This RFC bridges RFC 0099 (Motion) and RFC 0046 (router transition orchestration) into a concrete, end-to-end architecture where a user's finger drives screen transitions at 120fps through the Rust kernel, with the router managing navigation state and the platform renderer committing frames.

> **Substrate and status note (2026-07-05).** This document was written in
> March 2026, before the LLP 0297 threading model was accepted. Where the
> prose and diagrams below say "kernel (Rust, UI thread)," read the
> two-domain model of LLP 0297 §4.4: gesture recognizers, the
> `NavigationTransitionController`, animation drivers, and animated bindings
> live in the **main-owned Motion domain** (a main-thread Rust structure
> ticked by the display link, *not* the `Kernel` struct), while the
> tree/layout domain lives on the dedicated runtime thread. Compositor-path
> properties apply same-frame through the presenter; layout-affecting
> bindings take one frame via the constraint buffer. LLP 0297 §4.4 names
> transform and opacity as the compositor class; the built-in presets
> additionally drive corner radius, shadow/overlay opacity, and scale —
> all layout-inert layer properties applied through the same presenter
> path. This RFC treats that as a deliberate extension of §4.4's named
> set (declared here rather than silently assumed; it needs no constraint
> buffer and no 0297 mechanism change). Gesture hit-testing runs against the
> main-side presenter snapshot, never the kernel tree. Code sketches passing
> `&mut Kernel` should be read as "the Motion domain's state," and
> SharedValue storage semantics are fixed by LLP 0297 §4.5.
>
> **Implementation and evidence status (2026-07-13).** The P1 and P1.5 code
> paths are implemented: native `BackDisposition` publication, retained and
> leased dual scenes, the Rust interactive controller and `DecisionCell`,
> presenter-composited progress, unified start → resolution → settled outcome
> delivery, tab-aware commit transactions, capability promotions, Acto
> projection, ReleaseBench/stock fixtures, and the registered automated
> fidelity harness. This is not a passed P1 gate. No qualifying physical
> ProMotion correctness/fidelity bundle or complete shadow-readiness evidence
> set (P1 stack-only plus P1.5 tab-pop) is present. Status therefore remains
> Review;
> P2–P5 remain gated.
>
> The 2026-07-11 inventory baseline was materially earlier: native had only
> the discrete router edge swipe plus a non-router Motion demo, the web router
> owned the only interactive lane, and the native twin still declared
> transitions, provisional navigation, and blockers unsupported. The P1 table
> in Phasing retains that provenance; the implementation-status paragraph
> above is the current truth.
>
> **Amendments this RFC makes to sibling authorities (declared
> 2026-07-11).** Where this document's model differs from an Accepted or
> sibling document, the difference is a deliberate amendment, named here
> so nothing is contradicted silently:
>
> 1. **RFC 0010 D6 / RFC 0046 (threshold ownership).** Both give the
>    commit/cancel threshold decision to the router, and RFC 0046's
>    `TransitionHandle.reportProgress()` is specified as a per-frame call
>    feeding the router's threshold check. For interactive gestures under
>    LLP 0297, Motion owns threshold resolution and kinematics; the
>    router retains semantic authorization (the full canonical MATCH at
>    confirmation, captured as an authorization token; synchronous
>    dispatch enforcement re-runs inside the commit transaction — §4a/§4)
>    and performs the logical commit (§4a–§4c).
>    `reportProgress` becomes coalesced observability — never a per-frame
>    cross-thread call — and the handle lifecycle gains a distinct
>    **settled** signal: resolution (`commit()`/`abort()`) and settlement
>    are different moments (§4b), with `navigation.transitionEnd` moving
>    to settlement. **Registered, not merely declared:** per RFC 0010's
>    Normative Home Rule these extensions are recorded in RFC 0010 itself
>    (Invariant 7's transition-retention entry and the interactive
>    lifecycle carve-outs, 2026-07-11) and noted in RFC 0046's
>    TransitionHandle section the same day.
> 2. **RFC 0046 (interruption during an active gesture).** 0046's
>    `finish()` fast-forwards and commits when a new navigation
>    interrupts a transition. That remains the rule for *settling*
>    phases (§4b `interrupted`). While a finger is down, a racing
>    navigation is **structural invalidation** under LLP 0336's lease
>    rules: the stream cancels with snapback immediately (spring back
>    from current progress, no dispatch — §4b racing-navigation row),
>    after which the racing navigation applies. A gesture never commits
>    from stale truth. (Ordinary eligibility flips — a blocker appearing
>    mid-gesture — are NOT structural invalidation; they stay
>    checkpoint-handled at release, per 0336 and §4a.)
> 3. **RFC 0046 Appendix A (shared elements).** D1 replaces the
>    appendix's raster-snapshot default with a presenter-level retained
>    clone; rasterization is the measured upgrade path. (Noted in 0046's
>    TransitionHandle amendment note.)
> 4. **RFC 0052 + the agent-operations registry (agent surface).** §11
>    is an additive v2 amendment proposal: bare `GET /agent/navigation`
>    stays frozen v1 per RFC 0010 Contract 3; interactive lifecycle
>    detail rides `?v=2` as new additive fields registered through
>    0052's Contract 3 extensions table. Agent *operations* (the
>    deterministic gesture op) register through the `agent-operations`
>    authority in `exact-contracts.json`, which owns operation
>    registration — 0052 owns the wire payloads.
> 5. **LLP 0336 §6.1 (gesture contention).** 0336's arbitration arena is
>    the authority for *who wins a contested gesture* and for lease
>    invalidation timing; this RFC's edge swipe participates as its
>    `RouterHistory` claimant, and §4b's racing-navigation rule is
>    0336's structural-invalidation rule applied to navigation. This RFC
>    owns the navigation-specific lifecycle (readiness, provisional
>    state, two-phase resolution). §9's arbitration content is
>    orientation, not a second authority.

## Motivation

### The defining moment of app quality

The single most visible quality signal in any mobile app is what happens when a user swipes back. In a native iOS app, the current screen tracks the finger exactly, the previous screen slides in from the left with a parallax offset, shadows shift, and if the user changes their mind and reverses direction, everything follows instantly. The entire interaction is 120fps, interruptible at any point, and velocity-aware — a fast fling commits, a slow release snaps back.

In React Native today, this is either:
- Delegated entirely to the native navigation controller (losing control over the transition)
- Implemented with react-native-screens + Reanimated (fragile, version-sensitive, hard to customize)
- Faked with JS-driven animation (drops frames, feels wrong)

None of these options give developers both full control and native performance. Exact can.

### Why this needs its own RFC

RFC 0046 defines the `TransitionDirective`/`TransitionHandle` contract — what the router emits and what adapters consume. RFC 0099 defines Motion's kernel primitives — SharedValues, gesture recognizers, animation drivers. But neither RFC specifies the concrete system that connects them: how an edge swipe gesture creates a provisional navigation, how two screens animate simultaneously driven by finger position, how velocity determines commit vs. cancel, how shared elements are snapshotted and interpolated, or how sheets follow a drag gesture to their detent positions.

This RFC is the wiring diagram.

### What this enables

1. **Swipe-back navigation** that feels identical to native iOS, running entirely in the Rust kernel
2. **Interactive sheet dismiss** with velocity-aware snap-to-detent physics
3. **Shared element transitions** where a thumbnail morphs into a full-screen image across routes
4. **Custom interactive transitions** where developers define their own gesture-to-navigation mapping
5. **Cross-platform consistency** — same gesture API, platform-appropriate feel

## Design Principles

### 1. The router owns navigation; the kernel owns motion

The router decides *what* navigation means (which route a gesture would pop to, whether it is allowed, and the logical commit itself). Motion decides *how* it moves (spring physics, gesture tracking, frame interpolation) **and resolves the gesture at release** by applying the router's pre-published policy (§4a) to locally measured kinematics. The precise split: Motion produces the commit/cancel *resolution* from frozen policy + measured velocity/progress; the router provides semantic *authorization* (the generation confirmation of §4a) and performs the logical commit (§4b). Neither reaches into the other's domain at frame cadence. The bridge is the `TransitionHandle` plus the readiness snapshot — the router publishes policy ahead of need; Motion drives progress. (This is a declared amendment of RFC 0010 D6 / RFC 0046, which placed the threshold decision in the router — see the amendments block above.)

### 2. Both screens are always live during interactive transitions

When a swipe-back gesture is active, the outgoing screen and the incoming screen are both rendered and both receiving style updates from the kernel on every display-link tick. There is no snapshot-and-animate — both screens are live **route scenes** (Contract or React; the retained scene is a runtime artifact, not a framework one — per LLP 0160 the Contract binding of any new capability here ships first, with React as the fully supported secondary tier). This means the incoming screen can show loading states, animations, or real-time data while the transition is in progress. "Live" means live for rendering and data: pointer input is owned by the gesture for its duration — neither scene receives touches mid-transition, matching stock behavior.

"Live" is two separable rules, not one: the **retained-scene rule** (both scene trees exist and receive property updates for the duration of the interaction — presenter-owned, see §4b) and the **data-loading rule** (what data work runs during the gesture — D4 pins the default to load-on-commit). For P1, interactive preview additionally requires the back target's scene to be retained and synchronously renderable at gesture start; if it is not, the gesture degrades to RFC 0061's discrete pop (see D4 and §4a).

### 3. Velocity determines intent

**The normative resolution rule (stated once; every other statement of it in this document defers here):** let `v` be the release velocity along the transition axis, signed so positive means *toward commit*, and `p` the current progress.

1. If `|v| ≥ v_threshold` (default 500 pt/s — a measured parameter, fitted against stock traces in P1, not an assumed constant): resolve **in the direction of `v`** — commit if `v > 0`, cancel if `v < 0` — regardless of distance. A fast reverse fling past the midpoint *cancels*, matching stock iOS.
2. Otherwise: commit if `p > 0.5`, cancel if `p ≤ 0.5`.

Motion evaluates this rule at gesture end (per Principle 1 and §4c); it never rides a JS round-trip. This amends RFC 0046's informative `shouldCommitGesture` sketch (`progress > 0.5`, else `velocity > 500 && progress > 0.2`): its distance arm was already midpoint-based; the deltas here are the **direction-aware** velocity arm and **velocity overriding distance** (the sketch's velocity arm was direction-blind and still required `progress > 0.2`).

**Scope: Exact-owned recognizers only** (iOS/macOS edge and pan recognizers, web drag). An externally resolved gesture source delivers its own terminal resolution and this rule never overrides it — Android predictive back is the canonical case: per LLP 0310 §1.2, `onBackInvoked` *is* commit and `onBackCancelled` *is* cancel; the system's progress stream drives presentation, and its estimated release velocity may seed settle continuity, but the commit/cancel outcome is the system's.

### 4. Everything composes with the behavior primitive stack

DismissableLayer, Presence, FocusScope, and blockers all participate in interactive transitions. A blocker can prevent a swipe-dismiss. A DismissableLayer's exit animation runs after the gesture commits. Focus restores to the trigger element after a modal is dismissed by gesture.

### 5. Transition progress is a first-class value

Interactive navigation should not hide progress inside an adapter callback. Progress must be continuously computable, seekable, and observable by the router, the kernel, platform renderers, and the agent/debugging surface. This is what lets a gesture drive a transition frame-by-frame and lets commit/cancel continue naturally from the current visual state.

### 6. Cancellation is a first-class lifecycle path

An interactive navigation is not "successful unless something goes wrong." Cancel is a primary outcome. The architecture must preserve both screens, gesture state, provisional route state, and cleanup semantics until the system knows whether the interaction committed or cancelled.

## Prior Art for Interactive Navigation

### UIKit interactive transitions

Apple's documented [`UIPercentDrivenInteractiveTransition`](https://developer.apple.com/library/archive/featuredarticles/ViewControllerPGforiPhoneOS/CustomizingtheTransitionAnimations.html) is the closest existing mental model for this RFC. UIKit's model is straightforward: compute a completion percentage from gesture events, update it as input changes, then finish or cancel explicitly. UIKit also treats cancellation cleanup as a core concern, not a detail.

Exact should copy that lifecycle shape almost directly:
- A progress value updated continuously from gesture input
- Explicit commit and cancel paths
- Cleanup keyed off whether the interaction was cancelled

Where Exact intentionally differs is architectural ownership: UIKit puts the interaction controller in platform code, while Exact puts progress computation and transition control in the kernel and keeps navigation state in the router.

### Jetpack Compose predictive back and anchored motion

[Jetpack Compose predictive back](https://developer.android.com/develop/ui/compose/system/predictive-back) reinforces that back-gesture progress should be an explicit, observable value rather than a hidden side effect. Compose's [AnchoredDraggable](https://developer.android.com/develop/ui/compose/touch-input/pointer-input/migrate-swipeable) model is also highly relevant to modal and sheet dismissal because it treats offset, progress, target value, positional thresholds, and velocity thresholds as one state machine.

The lesson for this RFC is that interactive back, sheet detents, and dismiss gestures should all be modeled as anchored, seekable state. This RFC therefore treats progress, thresholds, and velocity as first-class controller inputs rather than adapter-local heuristics.

### Flutter route and hero transitions

[Flutter Hero animations](https://docs.flutter.dev/ui/animations/hero-animations) show that shared-element transitions work best when they are defined in terms of route-local source and destination elements with a shared semantic identity. Flutter's broader animation model also reinforces that route transitions, gesture tracking, and spring completion should share the same motion vocabulary.

The lesson for this RFC is that shared elements should be matched semantically by tag across routes, and the transition system should own the temporary overlay/clone lifecycle instead of asking application code to wire it manually.

### MotionLayout and seekable transitions

[MotionLayout](https://developer.android.com/develop/ui/views/animations/motionlayout) is useful prior art for one reason above all others: it treats transitions as seekable by explicit progress, and it supports touch-driven progress updates through `OnSwipe`. Exact should adopt the seekable-progress idea while avoiding MotionLayout's layout-coupled authoring model.

The lesson for this RFC is that built-in presets and future custom transitions should all be expressible as `progress -> view properties` mappings. That is what makes edge-swipe back, modal drag-dismiss, and future custom transitions all fit the same controller.

## Architecture

### System overview

```
User's Finger
      │
      ▼
┌──────────────────────────────────────────────────────────┐
│ Platform Input Bridge                                     │
│ (UIKit touch → kernel, MotionEvent → kernel, PointerEvent)│
└─────────────────────┬────────────────────────────────────┘
                      │ raw pointer events
                      ▼
┌──────────────────────────────────────────────────────────┐
│ Kernel Gesture Recognizer (Rust, UI thread)               │
│                                                           │
│  EdgeSwipeRecognizer / PanRecognizer                      │
│  ├─ Activation: edge zone + directional threshold         │
│  ├─ Tracking: position, velocity (VelocityTracker)        │
│  └─ Resolution: commit / cancel based on velocity+distance│
└─────────────────────┬────────────────────────────────────┘
                      │ gesture state (progress 0→1, velocity)
                      ▼
┌──────────────────────────────────────────────────────────┐
│ Navigation Transition Controller (Motion domain, main)    │
│                                                           │
│  Writes ONE slab slot: the root's reserved progress slot. │
│  Every per-property value is Motion-LOCAL controller      │
│  state (plain floats — never SharedValue slots):          │
│  ├─ outgoingTranslateX: f32 (0 → screenWidth)            │
│  ├─ incomingTranslateX: f32 (-parallax → 0)              │
│  ├─ shadowOpacity: f32 (0.15 → 0)                        │
│  └─ overlayOpacity: f32 (0.1 → 0)                        │
│                                                           │
│  On gesture update:                                       │
│    progress = clamp(translationX / screenWidth, 0, 1)     │
│    → progress slot write + all per-property values        │
│      derived and applied via the presenter-composited     │
│      channel (step 4c)                                    │
│                                                           │
│  On gesture end:                                          │
│    ONE spring drives progress → 1.0 / 0.0; every          │
│    property continues to derive from progress             │
└──────────┬────────────────────────┬──────────────────────┘
           │                        │
           ▼                        ▼
┌────────────────────┐   ┌────────────────────┐
│ Outgoing Screen    │   │ Incoming Screen     │
│ (live route scene) │   │ (live route scene)  │
│ transform derived  │   │ transform derived   │
│ from progress via  │   │ from progress via   │
│ composited channel │   │ composited channel  │
└────────────────────┘   └────────────────────┘
           │                        │
           ▼                        ▼
┌──────────────────────────────────────────────────────────┐
│ Platform Renderer                                         │
│ Commits both screen transforms on display-link            │
│ (Core Animation / DOM style / Android property animator)  │
└──────────────────────────────────────────────────────────┘
```

### Data flow: interactive swipe-back (step by step)

```
 1. User touches left edge of screen
 2. Platform delivers pointer-down to kernel
 3. Kernel's EdgeSwipeRecognizer enters Began state
    (activation zone: leftmost 20pt, or full-screen if configured)
 4. AT ARENA ACTIVATION — the slop/axis-lock lease grant, the ONLY
    activation boundary (0099's admission rule; everything before this
    step is RAII SceneLease prewarm, and Began mints NOTHING) — Motion
    atomically: acquires the InteractionLease, reserves the transition's
    three-credit bundle (start + resolution + settle-driver), validates
    the gesture against the main-side readiness snapshot (§4a), acquires
    the presenter scene lease (§4a/§4b), mints/re-arms the DecisionCell,
    and constructs
    the NavigationTransitionController ENTIRELY from prepublished data:
    a. The disposition's motionDescriptor names the visual preset,
       parameters, and back target — no runtime-thread object is needed
    b. Motion mints the transition id main-side (structural revision +
       sequence); everything downstream — the handle, the SSE ids, the
       agent phase — adopts this id
    c. The controller writes the root's RESERVED transition-progress
       slot — a root-lifetime SharedValue allocated by the RUNTIME at
       root setup (0099 M1's registry is append-only and
       runtime-single-owner; Motion never allocates slots, at touch time
       or otherwise). Every per-property value is Motion-LOCAL controller
       state, applied to the outgoing scene root and the leased
       back-target scene root as a **presenter-composited channel** per
       LLP 0336 r9 (ENG-24306): final value = authored/static value ∘
       transition value, re-composed on EVERY presenter apply that
       touches a scene holding a live lease or retention entry — never a
       raw one-shot layer write that an unrelated commit can wipe. This
       stays outside the kernel binding tables, so LLP 0297's
       ring-ordered attach/detach invariant for tree bindings is
       untouched
 5. Motion appends the start record (at activation, after the arena
    lease grant — never at Began; its credit was reserved in step 4)
    notifying the router ("interactive back gesture started", carrying
    the minted id + both revision stamps) and begins tracking
    immediately — the first visual frame never waits for a reply
 6. Router (runtime thread, asynchronously) processes the notification,
    enters TRANSITION-PREP, and confirms the generation — ack adopts the
    minted id; a generation mismatch nacks, resolving the gesture as
    cancel-with-snapback (§4a):
    a. Creates TransitionDirective { kind: 'slide', direction: 'backward', interactive: true }
       around the minted id
    b. Creates the TransitionHandle bound to that id
    c. Sets state.provisional = { location: backTarget, matches: [...], active: true }
    d. Renders both current and provisional route trees (Invariant 7) —
       for the P1 retained-scene path this re-confirms what the lease
       already guarantees renderable
 7. User moves finger right → kernel updates EdgeSwipeRecognizer
 8. On each pointer-move (120fps):
    a. Recognizer computes translationX and velocity
    b. Controller computes progress = translationX / screenWidth
    c. Controller writes the reserved progress slot, derives the
       Motion-local per-property values (outgoing pos, incoming pos,
       shadow, overlay), and applies them via the presenter-composited
       channel (step 4c) — the progress slot is the only slab write
    d. Display-link: renderer commits new transforms for both screens
    e. Controller publishes progress to the latest-value plane;
       handle.reportProgress delivers coalesced updates to the router
       (observability channel, never the frame path — §4c)
 9. User lifts finger → EdgeSwipeRecognizer enters Ended state
10. Motion evaluates commit/cancel by the Design Principle 3 rule
    (direction-aware velocity arm at |v| ≥ threshold, else midpoint)
11a. COMMIT path (two-phase — logical commit, then visual settle; §4b):
    a. Motion springs the single progress value → 1.0, initialized with
       the release velocity (dp/dt = v / screenWidth); every visual
       property continues to derive from progress (§3) — velocity
       continuity is structural, not per-property
    b. Motion sends the resolution ("commit", id, generation) → lifecycle
       enters settling-commit
    c. Router (having acked, or on processing the resolution after a late
       ack) performs the LOGICAL commit as one atomic navigation-store
       transaction: state.provisional promotes to state.matches and the
       history pop applies together. If the release arrived before the
       router's ack, the visual settle proceeds and the logical commit is
       HELD pending confirmation (§4b `settled-visual` row); a nack
       instead reconciles to logical truth
    d. The rendered route set still retains BOTH scenes, generation-fenced
       — logical promotion emits no unmount for the outgoing scene
    e. Motion reports settled AND the commit token is observed
       (reconcile-applied substitutes on timeout — §4b's release rule;
       settle alone never releases a COMMIT's entry) → retention entry
       released, outgoing scene unmounts, cleanup
11b. CANCEL path (two-phase; §4b):
    a. Motion springs progress → 0.0, initialized with the same signed
       dp/dt (no negation — continuity through the turnaround comes from
       the spring, matching stock behavior)
    b. Motion sends the resolution ("cancel", id, generation) → lifecycle
       enters settling-cancel
    c. Router clears state.provisional (logical discard is unconditional —
       cancel needs no confirmation); the rendered route set retains the
       incoming scene until settle — clearing provisional state must not
       unmount a scene mid-snapback
    d. Motion reports settled → retention entry released, incoming scene
       unmounts, cleanup
```

JS is involved at step 6 (router confirmation and provisional state) and at the router lines of step 11 (logical commit / provisional discard). Steps 4–5 and 7–10 are pure Rust at display-link cadence — under LLP 0297 §4.4 they run in the main-owned Motion domain, and the router JS at steps 6/11 runs on the runtime thread, communicating over the lock-free planes.

## Detailed Design

### 1. Built-in Transition Presets

Each preset defines how normalized progress maps to per-scene visual properties, computed in the Motion domain and applied through the presenter-composited channel (data-flow step 4c). Developers can use these directly or build custom transitions.

#### Slide (Stack Push/Pop)

The iOS-standard horizontal slide with parallax.

```typescript
// What developers write (the shipped ScreenOptions surface):
export const screen: ScreenOptions = {
  transition: 'slide',
  gestureEnabled: true,          // edge-swipe back is the default gesture for stack routes
};
```

```
// What Motion computes per frame, given progress p ∈ [0, 1]:

Outgoing screen:
  translateX = p * screenWidth
  shadowOpacity = 0.15 * (1 - p)

Incoming screen (behind, with parallax):
  translateX = -0.3 * screenWidth * (1 - p)    // 30% parallax offset
  overlayOpacity = 0.1 * (1 - p)               // dim overlay fades out

Overlay (between screens):
  opacity = 0.1 * (1 - p)
```

This is the shape of iOS `UINavigationController` interactive pop behavior: the outgoing screen moves with the finger, the incoming screen has a subtle parallax shift from the left, and a dim overlay separates them. The constants shown here and throughout this section (parallax 0.3, shadow 0.15, dim 0.1, the spring configs in §10) are starting parameters to be fitted against measured stock traces during P1 — not assumed exact matches.

#### Slide Vertical (Modal Present/Dismiss)

```
// progress p ∈ [0, 1] where 0 = fully presented, 1 = fully dismissed

Modal screen:
  translateY = p * screenHeight
  borderRadius = interpolate(p, [0, 0.1], [modalRadius, 0])

Background screen:
  scale = interpolate(p, [0, 1], [0.92, 1])
  borderRadius = interpolate(p, [0, 1], [screenRadius, 0])
  opacity = interpolate(p, [0, 1], [0.85, 1])
```

This matches iOS modal presentation: the modal slides up from the bottom, and the background screen scales down slightly with rounded corners (the "card stack" effect).

#### Fade (Cross-Dissolve)

```
// progress p ∈ [0, 1]

Outgoing screen:
  opacity = 1 - p

Incoming screen:
  opacity = p
```

Used for tab switches and replace navigations.

#### Sheet Snap-to-Detent

```
// Sheets use a custom controller with multiple stable positions

Detent positions (from bottom):
  medium = screenHeight * 0.5
  large = screenHeight - topSafeArea
  collapsed = 0 (fully dismissed)

Physics:
  On gesture end, spring to nearest detent based on:
    1. Current position
    2. Gesture velocity (high velocity can skip to next detent)
    3. Directional intent (moving up → next larger detent)

Background dimming:
  overlayOpacity = interpolate(sheetY, [collapsed, large], [0, 0.4])
```

### 2. Edge Swipe Recognizer

A specialized gesture recognizer for navigation back gestures.

On Apple platforms, v1 should use a hybrid boundary consistent with RFC 0099 (Motion): Motion owns transition progress, controller state, thresholds, spring completion, and router integration, while the host bridge uses thin wrappers around platform recognizers such as `UIScreenEdgePanGestureRecognizer` and `UIPanGestureRecognizer` to preserve native edge-swipe feel and scroll-view coexistence. The kernel-side `EdgeSwipeRecognizer` remains the architectural model and the cross-platform abstraction; the Apple bridge may source its input from platform recognizers rather than raw touch interpretation in v1.

```typescript
// JS configuration. The first three fields are the SHIPPED ScreenOptions
// surface (packages/exact-router/src/types.ts; the 0046/0061-owned
// ScreenOptions gesture fields). The two
// marked PROPOSED are extensions this RFC adds to that surface — they land
// as a ScreenOptions amendment through RFC 0061/0010 when P1 implements
// them, not as a parallel vocabulary. (Earlier drafts of this RFC sketched
// a `gestureNavigation: { back: 'edge-swipe' }` shape; the shipped
// `gestureEnabled`/`gestureOptions` surface supersedes it. LLP 0336's
// references to "RFC 0100's gestureNavigation options" — its §5 adoption
// and OQ7 — resolve to this shipped surface.)
export const screen: ScreenOptions = {
  gestureEnabled: true,                 // shipped — master switch for this screen
  gestureDirection: 'horizontal',       // shipped — 'horizontal' | 'vertical'
  gestureOptions: {
    fullScreenBackSwipe: false,         // shipped — full-screen swipe (iOS Mail style);
                                        // iOS-only per LLP 0310; NOT in P1 (§9 / 0336 OQ7)
    dismissSwipe: true,                 // shipped — sheet/modal drag-dismiss (P2 consumer)
    edgeWidth: 20,                      // PROPOSED — edge zone width in pt (default: 20pt on iOS)
    activationThreshold: 10,            // PROPOSED — min horizontal distance before activation
  },
};
```

**Edge-zone geometry:** the activation zone is measured from the
**presented container's** leading edge, not the physical screen edge. For
a full-screen stack the two coincide; for a floating iPad sheet or a
form-sheet modal containing a stack, the zone hugs the container —
composing with D2: the zone belongs to the deepest container whose stack
can pop.

**Recognizer topology (P1):** ONE edge recognizer with two modes —
interactive when the §4a plan is eligible, recognize-then-discrete
otherwise — **superseding** the separate ENG-22454 host recognizer at
P1 rather than coexisting with it. The discrete path becomes a mode of
the same recognizer, which is what makes outcome (a) degradation
seamless and leaves exactly one owner for edge-swipe wiring.

```rust
// Kernel implementation
pub struct EdgeSwipeRecognizer {
    id: GestureId,
    state: GestureState,

    // Configuration
    edge: Edge,                    // Left, Right, Top, Bottom
    edge_width: f32,               // Activation zone width in points
    activation_threshold: f32,     // Min translation before activation
    full_screen: bool,             // If true, ignore edge_width

    // Tracking
    start_position: Vec2,
    current_position: Vec2,
    velocity_tracker: VelocityTracker,

    // Arbitration
    /// If the user moves more vertically than horizontally in the first
    /// activation_threshold distance, this recognizer fails.
    /// This prevents conflicts with vertical scroll.
    fail_if_vertical: bool,
}

impl EdgeSwipeRecognizer {
    fn on_pointer_move(&mut self, position: Vec2, timestamp: Instant) {
        self.velocity_tracker.add_sample(position, timestamp);
        self.current_position = position;

        match self.state {
            GestureState::Began => {
                let dx = (position.x - self.start_position.x).abs();
                let dy = (position.y - self.start_position.y).abs();

                if dx > self.activation_threshold {
                    if self.fail_if_vertical && dy > dx {
                        self.state = GestureState::Failed;
                    } else {
                        self.state = GestureState::Active;
                    }
                }
            }
            GestureState::Active => {
                // Continue tracking — controller reads position directly
            }
            _ => {}
        }
    }

    fn on_pointer_up(&mut self) {
        if self.state == GestureState::Active {
            self.state = GestureState::Ended;
        } else {
            self.state = GestureState::Failed;
        }
    }

    fn started_in_edge_zone(&self) -> bool {
        if self.full_screen { return true; }
        match self.edge {
            Edge::Left => self.start_position.x < self.edge_width,
            Edge::Right => self.start_position.x > self.screen_width - self.edge_width,
            Edge::Top => self.start_position.y < self.edge_width,
            Edge::Bottom => self.start_position.y > self.screen_height - self.edge_width,
        }
    }
}
```

### 3. Navigation Transition Controller

The central orchestrator that connects gestures to screen transforms. One controller exists per active interactive transition.

```rust
pub struct NavigationTransitionController {
    /// The transition preset (determines how progress maps to screen properties)
    preset: TransitionPreset,

    /// Per-property transition values are MOTION-LOCAL plain floats —
    /// controller state, never SharedValue slots (0099 M1's registry is
    /// runtime-single-owner; Motion never allocates). They reach glass
    /// through the presenter-composited channel (data-flow step 4c).
    outgoing_translate: f32,
    incoming_translate: f32,
    shadow_opacity: f32,
    overlay_opacity: f32,
    /// Optional additional values for modal/sheet presets
    background_scale: Option<f32>,
    background_radius: Option<f32>,

    /// Screen dimensions for computing absolute positions
    screen_width: f32,
    screen_height: f32,

    /// Current normalized progress [0, 1]. Progress is the SINGLE
    /// spring-driven value at settle; every visual property derives from
    /// it (see apply_progress below). The controller never springs
    /// per-property values independently.
    progress: f32,
    /// The root's RESERVED transition-progress slot (runtime-allocated,
    /// root-lifetime — §4a) — the ONLY slab slot this controller writes
    progress_value: SharedValueId,

    /// Handle ID for reporting progress to the router
    transition_handle_id: TransitionHandleId,

    /// Commit/cancel thresholds (Design Principle 3)
    velocity_threshold: f32,      // pt/s (logical points) — default 500
    distance_threshold: f32,      // fraction — default 0.5
}

impl NavigationTransitionController {
    /// Called on each gesture update. Tracking and settling share ONE
    /// progress→properties path: this method computes progress, writes the
    /// progress SharedValue, and applies the shared mapping — nothing else.
    pub fn update_from_gesture(&mut self, translation: f32, motion: &mut MotionDomain) {
        self.progress = (translation / self.screen_width).clamp(0.0, 1.0);
        motion.write_shared_value(self.progress_value, self.progress);
        self.apply_progress(self.progress, motion);

        // Publish to the latest-value plane; the router-facing handle update
        // is coalesced (§4c) — never a per-frame cross-thread call
        motion.report_transition_progress(self.transition_handle_id, self.progress);
    }

    /// The single progress→properties mapping. Both drivers call it:
    /// tracking (above) with finger-derived progress, and the settle spring
    /// per display-link tick with spring-driven progress. One mapping, two
    /// drivers — there is no second code path to drift.
    fn apply_progress(&mut self, p: f32, motion: &mut MotionDomain) {
        match self.preset {
            TransitionPreset::Slide => {
                // Motion-local derivation; publication is the
                // presenter-composited channel (step 4c), never slab writes
                self.outgoing_translate = p * self.screen_width;
                self.incoming_translate = -0.3 * self.screen_width * (1.0 - p);
                self.shadow_opacity = 0.15 * (1.0 - p);
                self.overlay_opacity = 0.1 * (1.0 - p);
                motion.compose_scene_properties(self.scene_channel(), self.property_set());
            }
            TransitionPreset::SlideVertical => { /* modal math */ }
            TransitionPreset::Fade => { /* cross-fade math */ }
            TransitionPreset::Sheet(ref detents) => { /* detent math */ }
            TransitionPreset::Custom(ref descriptor) => {
                // §8: descriptor-declared bindings derive from progress; a
                // restricted worklet computes SharedValue writes only —
                // never direct tree mutation (LLP 0297)
                motion.apply_binding_descriptor(descriptor, p);
            }
        }
    }

    /// Called when gesture ends. Applies the Design Principle 3 rule
    /// (the single normative statement) and starts the settling spring
    /// ON PROGRESS — not on per-property values. `velocity_pt_s` is
    /// normalized to logical points/second along the transition axis and
    /// signed so positive means toward commit.
    pub fn resolve_gesture(
        &mut self,
        velocity_pt_s: f32,
        motion: &mut MotionDomain,
    ) -> TransitionResolution {
        let should_commit = if velocity_pt_s.abs() >= self.velocity_threshold {
            velocity_pt_s > 0.0            // velocity arm: direction wins
        } else {
            self.progress > self.distance_threshold   // midpoint arm
        };

        // NORMAL-MOTION PATH ONLY — under reduced motion (§12 rule 3) the
        // settle is RFC 0046's 150ms fade to `target`, never this spring.
        //
        // ONE spring drives normalized progress. Velocity continuity is
        // structural: the spring's initial dp/dt is the release velocity
        // in progress units, WITH ITS REAL SIGN — a cancel that begins
        // while the finger was still moving toward commit decelerates
        // through the turnaround (no negation, no restart discontinuity).
        let spring = SpringConfig {
            damping: 28.0,
            stiffness: 320.0,
            mass: 1.0,
            initial_velocity: velocity_pt_s / self.screen_width,
            rest_velocity_threshold: 0.001,      // progress units
            rest_displacement_threshold: 0.001,
        };
        let target = if should_commit { 1.0 } else { 0.0 };
        motion.start_spring(self.progress_value, target, spring);

        if should_commit { TransitionResolution::Commit } else { TransitionResolution::Cancel }
    }
    // (apply_progress is defined above, with update_from_gesture — the
    // settle spring drives it per tick exactly as tracking does.)
}
```

Why one spring on progress rather than one spring per property: the
per-property values have different ranges, derivatives, and signs with
respect to progress (outgoing translate runs 0→width while shadow opacity
runs 0.15→0), so feeding them one raw gesture velocity cannot preserve
continuity for any of them except by accident, and unit errors (pt/s into
an opacity spring) are structural. Springing progress and deriving
properties makes velocity inheritance exact for every property at once.
This is also the shape the existing native prototype validated
(`MotionDemoModule.swift` tracks and settles normalized progress), and it
is why the P1 settle-fidelity gate can meaningfully demand "no restart
discontinuity."

### 4. Router Integration: The Provisional State Lifecycle

The router's role during interactive transitions.

```typescript
// The router's CONFIRMATION path (runtime thread). Illustrative of
// ordering and ownership, not of the navigation-store API. Three rules
// distinguish it from a March-era pull model:
//   - The router never answers a question at gesture time that was not
//     pre-published; the confirmation runs the FULL canonical MATCH
//     (RFC 0010's lifecycle authorization — matching, interceptors,
//     middleware, access, blockers' `when()` predicates, handlers, layer
//     policy) with full runtime-thread truth. A blocker registration
//     that raced the gesture is additionally a revision mismatch — but
//     the MATCH re-run, not the revision compare, is what catches an
//     unobserved predicate flip.
//   - The back target comes from the published disposition's pinned
//     identity, never from `matches` arithmetic (`matches` is the current
//     URL-derived hierarchy, not a history stack — RFC 0010 Contract 2;
//     the disposition was computed by the RFC 0061 cascade).
//   - Messages from Motion are processed in order: the start notification
//     (ack/nack) is always handled before a resolution for the same
//     transition id (§4b's pending-confirmation row exists because the
//     USER can be faster than this queue, not because order is lost).
//   - Resolutions for a nacked or unknown transition id are defined
//     no-ops — the tombstone rule, mirroring LLP 0297's stale-id
//     tolerance. The terminal SSE outcome for a nack-after-settle
//     reconcile is `interrupted` (§11).

function onGestureStartNotification(msg: {
  transitionId: string;      // minted main-side by Motion (data-flow step 4)
  structuralRevision: number; // BOTH stamps ride the start record —
  policyRevision: number;     // the §4a table's two revisions, never a
                              // fused "generation"
  pinnedTargetId: string;     // the disposition's pinned history-entry id
}): void {
  // The two revisions diverge deliberately (§4a's split):
  //  - STRUCTURAL mismatch (navigation, route change, target gone) →
  //    immediate nack → cancel-with-snapback. The finger cannot own a
  //    frame whose target no longer exists.
  //  - POLICY divergence (a blocker/access/handler flip the MATCH
  //    re-run FAILS on — never a bare revision mismatch) → the
  //    mid-gesture blocker rule (§4a): tracking is NEVER revoked;
  //    confirm the start as confirmedCancel — release
  //    resolves as cancel-with-snapback regardless of progress/velocity,
  //    and the blocker UI shows after settle.
  //  - SELF-INVALIDATION, structurally impossible (r12): the owning
  //    gesture's own cascade re-publication (`backAction: gesture`)
  //    does NOT bump policyRevision — it is a reentrancy signal that
  //    denies a SECOND stream, carried on the publication itself, not
  //    a policy change. No origin stamp, no exemption compare; an
  //    external policy change racing the gesture still bumps
  //    policyRevision normally. (0061-side wording queued through the
  //    router-set loop.)
  if (msg.structuralRevision !== publishedDisposition.structuralRevision) {
    // Structural invalidation is a DURABLE CELL STATE (r12: the state
    // machine is closed — every outcome is in the word): CAS pending →
    // nacked, then wake. Main's scheduled loads see `nacked` even if
    // the wake is lost.
    decisionCell(msg.transitionId).cas('pending', 'nacked');
    motionChannel.nack(msg.transitionId);   // wake → cancel-with-snapback
    return;
  }
  // Policy-revision mismatch alone is NOT an outcome (this document's
  // doctrine: revision equality is an optimization, NEVER a substitute
  // — and 0336's checkpoints re-evaluate rather than mismatch-cancel).
  // A bare mismatch only disables published-intermediate reuse; the
  // full MATCH re-run below decides. Benign bumps (a dataDisposition
  // recompute, a re-publish) authorize cleanly and commit normally.
  // SELF-INVALIDATION, resolved structurally (r12): the owning
  // gesture's own cascade re-publication (`backAction: gesture`) is a
  // REENTRANCY signal, not a policy change — it does NOT bump
  // policyRevision at all (it rides the publication's reentrancy
  // field, which denies a SECOND stream); so no origin-stamp compare
  // is needed and a simultaneous EXTERNAL policy change still bumps
  // policyRevision and is caught here. (The 0061-side wording of the
  // no-bump rule is queued through the router-set loop.)
  const intermediatesReusable =
    msg.policyRevision === publishedDisposition.policyRevision;

  // Semantic authorization runs the FULL canonical MATCH for the pop —
  // route matching, interceptors, registered middleware (which may
  // redirect or abort), access, blockers' `when()` predicates, handler
  // registrations, layer policy — with full runtime-thread truth, exactly
  // as RFC 0010's lifecycle requires of every navigation. Revision
  // equality is an optimization that lets the re-run reuse published
  // intermediates — never a substitute. A middleware redirect or target
  // divergence is STRUCTURAL and nacks (the gesture never follows a
  // redirect mid-flight); a blocker/access flip is POLICY and joins the
  // forced-cancel path — tracking continues either way until release.
  const authorized = runCanonicalMatchForPop(publishedDisposition.backTarget);
  if (!authorized.ok ||
      authorized.target.historyEntryId !==
        publishedDisposition.backTarget.historyEntryId) {
    // Structural divergence (redirect, target gone) nacks DURABLY —
    // same closed-machine rule as the revision-mismatch branch above:
    // CAS the cell, then wake (a lost wake still resolves at main's
    // scheduled loads). A pure policy failure (blocker predicate now
    // true) joins the forced-cancel path below instead — tracking is
    // never revoked.
    if (authorized.divergence === 'structural') {
      decisionCell(msg.transitionId).cas('pending', 'nacked');
      motionChannel.nack(msg.transitionId);
      return;
    }
  }
  // The confirmation's result is captured as an AUTHORIZATION TOKEN:
  // { structuralRevision, policyRevision, pinnedTargetId,
  //   middlewareOutcome } — effectful/async phases (interceptors,
  // registered middleware, which may await) run ONCE, here, and never
  // again; the token is invalidated by any later revision bump. This is
  // what the §4 commit transaction re-validates (its comment below).
  // forcedCancel = the RE-RUN's policy verdict ONLY (never a bare
  // revision compare): a blocker/access/handler flip the MATCH re-run
  // actually fails on.
  const forcedCancel = authorized.divergence === 'policy';
  // The cross-domain LINEARIZATION POINT: CAS the transition's
  // DecisionCell (§4a) — a CONTROL-PLANE record beside the outcome log
  // (0099 r9's adjudication; deliberately NOT a SharedValue slot — the
  // value plane is single-writer). The expected word packs
  // (rootInstance, transitionGeneration, state), so a late CAS for
  // transition N fails against the cell re-armed for N+1 and a
  // destroyed root's CAS fails against any successor. THE VERDICT IS
  // IN THE WORD: confirmedAllow when authorization holds,
  // confirmedCancel when policy diverged (forced cancel) — so the
  // allow/cancel decision is linearized with the confirmation itself,
  // never carried as a hint on a message. If main already expired the
  // cell — the bounded timeout fired while these messages sat in a
  // wedged runtime's queue — the CAS fails and this gesture nacks: a
  // tombstoned id can never be revived into a logical pop by late
  // queue drainage.
  const confirmedState = forcedCancel ? 'confirmedCancel' : 'confirmedAllow';
  if (!decisionCell(msg.transitionId).cas('pending', confirmedState)) {
    motionChannel.nack(msg.transitionId);   // expired; reconcile already ran
    return;
  }
  const target = publishedDisposition.backTarget;  // pinned identity, now re-validated

  // Shipped TransitionDirective shape (packages/exact-router/src/types.ts):
  // preset and durationMs are required; there is no `easing` field. NOTE:
  // the shipped `preset` is NAVIGATION INTENT ("push" | "pop" | …), not a
  // visual preset — the visual preset + parameters are §4a's
  // motionDescriptor, which never rides this type. from/to are
  // TransitionRouteDescriptors.
  const directive: TransitionDirective = {
    id: msg.transitionId,                   // adopts Motion's minted id
    kind: resolveTransitionKind('pop'),     // respects per-route, per-layout, reduced-motion
    direction: 'backward',
    preset: 'pop',                          // navigation intent (shipped semantics)
    interactive: true,
    durationMs: publishedDisposition.motionDescriptor.durationMs,
    reducedMotion: publishedDisposition.reducedMotion,
    from: describeRoute(state.matches.at(-1)),
    to: describeRoute(target),
  };
  const handle = createTransitionHandle(directive);

  // Provisional state — both routes now render (Invariant 7)
  state.provisional = {
    location: target.location,
    matches: computeMatchesForLocation(target.location),
    active: true,
    transitionId: directive.id,
  };
  // The ack is a level-triggered WAKE over the durable cell — main
  // re-reads the cell word on wake (confirmedCancel ⇒ Motion keeps
  // tracking, release resolves cancel-with-snapback, blocker UI after
  // settle — §4a's mid-gesture rule). A lost or duplicate wake is
  // presentation latency only; the decision is already linearized in
  // the cell.
  motionChannel.ack(msg.transitionId);
  emitSSE('navigation.transitionStart', {
    id: directive.id, kind: directive.kind,
    direction: directive.direction, interactive: true,
  });
}

// Motion resolved COMMIT (settling-commit is already running visually):
function onGestureResolution_Commit(handle: TransitionHandle) {
  handle.commit();
  // The LOGICAL pop is ONE atomic navigation-store transaction: match
  // promotion and the history pop apply together (never `state.matches =`
  // plus a separate `history.back()` — a pop CONSUMES a history entry and
  // must not double-apply or interleave). The transaction RE-VALIDATES
  // before mutating, with a PINNED scope (the confirmation→commit window
  // is the whole gesture, so this is load-bearing, not belt-and-braces):
  //   1. DecisionCell is `confirmedAllow` SPECIFICALLY (control-plane
  //      assert — `confirmedCancel` converts the resolution to the
  //      cancel path before any mutation; forced cancel is inside the
  //      linearized authorization, not an ack hint);
  //   2. the authorization token's structural + policy revisions are
  //      both unchanged;
  //   3. the SYNCHRONOUS pure predicates re-run atomically inside the
  //      transaction: blockers' `when()` predicates, access, layer
  //      policy, target-identity equality, `handlerDisposition` still
  //      clear, AND `dataUsableUntil` not yet crossed (wall-clock
  //      deadline crossing bumps no revision, so the deadline is
  //      re-checked here synchronously) — this is LLP 0336 checkpoint
  //      3's dispatch enforcement ("the router enforces blockers
  //      regardless"), covering EVERY §4a disposition field a pop
  //      depends on, so an unobserved flip that escaped dependency
  //      tracking still cannot commit a pop past an active blocker,
  //      a newly-registered handler, or expired data;
  //   4. effectful/async MATCH phases (interceptors, middleware) are
  //      deliberately NOT re-run — they ran once at confirmation and
  //      their outcome rides the token (middleware executes once per
  //      navigation, per RFC 0010's lifecycle; a redirect can only
  //      happen at confirmation, where it nacks).
  // Any check failing aborts to reconcile; nothing mutates on stale
  // authorization. The transaction carries the transition id as its
  // token, which is exempt from update-economy equality elision by
  // construction (§4b).
  commitNavigation({
    kind: 'pop', to: state.provisional.matches,
    transitionId: handle.directive.id,
    revalidate: {
      decisionCell: 'confirmedAllow',
      authorizationToken: handle.authorizationToken,
      dispatchEnforcement: ['blockers.when', 'access', 'layerPolicy',
                            'targetIdentity', 'handlerDisposition',
                            'dataUsableUntil'],
    },
  });
  state.provisional = null;
  // The rendered route set is NOT reduced here: the outgoing route stays
  // in it as a transition-retention entry (input-inert), so no unmount or
  // binding-detach is emitted while the settle spring runs (§4b).
  // `navigation.transitionEnd` is NOT emitted here — it fires at settled,
  // when presentation matches logical truth (§4c, §11).
}

// Motion resolved CANCEL (settling-cancel is already running visually):
function onGestureResolution_Cancel(handle: TransitionHandle) {
  handle.abort();
  // Logical discard is unconditional — nothing was committed. The incoming
  // scene stays rendered via its retention entry until Motion reports
  // settled; clearing provisional must not unmount it mid-snapback (§4b).
  state.provisional = null;
}
```

#### 4a. Gesture readiness: published disposition + presenter readiness (normative for P1)

Under LLP 0297 the Motion domain is main-owned and the router runs on the
runtime thread, so gesture start MUST NOT synchronously ask the router
anything. Everything Motion needs to decide "may this gesture begin, and
what would it do?" must already be main-readable when the finger touches
the edge. That truth has **two owners**, so it is published as two
generation-stamped halves — a single fused snapshot would make the router
claim authority over main-side facts it cannot know (current dimensions,
scene retention, presenter identity):

**Half 1 — the router-published `BackDisposition` (runtime thread → main):**

Pre-activation ineligibility resolves to exactly one of three outcomes,
used consistently by every row below and by §7: **(a) degrade to RFC
0061's discrete pop** — the shipped recognize-and-commit edge swipe
still runs, so the user always has *a* working swipe (tab-pop until
P1.5, registered handler, unusable data, non-retained scene, publish
gap); **(b) inert-stream blocked-attempt** — no tracking, blocker UI on
intent (blocked disposition, §7); **(c) clean recognizer failure, no
back path at all** (null target at root, `gestureEnabled: false`).

| Field | Meaning |
|---|---|
| `structuralRevision` | Bumped when the *world under the gesture* changes: committed navigations, route/scene-set changes, presenter swaps. A mid-gesture structural bump is LLP 0336 **structural invalidation** → immediate cancel-with-snapback (§4b) |
| `policyRevision` | Bumped on *eligibility* changes: blocker/handler/dismiss registration or predicate-dependency change, data-disposition recompute. A mid-gesture policy bump is an ordinary eligibility flip → checkpoint-handled at release (0336's no-mid-gesture-revocation rule). Splitting the two revisions is what lets Motion distinguish cancel-now from resolve-at-release without asking anyone |
| `backTarget` | The pinned pop target as a **history-entry identity** (global or tab-scoped entry id + match id) — repeated visits to the same route, or the same route in two tabs, are distinct entries, and the confirmation path and `SceneLease` bind to THIS identity. Never recomputed from `matches` arithmetic (the cascade computed it; `matches` is a URL-derived hierarchy, not a history stack). Null at root |
| `backAction` | The RFC 0061 cascade answer (level + target), as already computed for the agent API (RFC 0052). **Gesture gating rule (as built after P1.5):** both global `stack-pop` and per-tab-history `tab-pop` dispositions may enter the interactive lane when their readiness and lease checks pass. A tab-scoped commit uses the tab-aware transaction (global replace plus `tabNavigation` pop); readiness/publish/lease failure retains the shipped discrete edge-swipe fallback, so there is never a dead edge. Blocker and modal/sheet-dismiss levels use outcome **(b)**. Window levels and other members of the shipped union (`gesture`, `platform-default` — named so no future producer assumes eligibility by omission) use outcome **(a)**. The recognizer, readiness, lease, retention, and settle machinery is shared across stack and tab pops; only the logical commit transaction differs |
| `handlerDisposition` | `none` / `registered` — whether any custom `useBackHandler` is registered for the focused scope. The shipped `BackActionState` level union has **no handler level**, and the shipped `back()` runs handlers after blocker resolution without publishing them — so this field is the conservative, side-effect-free mirror: `registered` makes the interactive gesture ineligible in P1 → outcome **(a)**, the discrete pop, whose `back()` cascade runs the handler exactly as a button press would. Registration/removal bumps `policyRevision` |
| `dataDisposition` + `dataUsableUntil` | Back-target cache state under RFC 0053's pop semantics: `fresh` / `stale` (within `gcTime`) / `unusable` — where `unusable` means **expired, missing, access-changed, blocking `dataStrategy: 'external'`, or a whole-loader failure with no usable cached scene/data**; a failed REVALIDATION or a rejected deferred field WITH an unexpired cached value classifies `stale`, per RFC 0053's render-cached-while-revalidating rule and the shipped rejected-deferred-is-stale classification (`loader-cache-store.ts`) — plus **`dataUsableUntil`** — the **eligibility deadline** (gcTime-derived): past it the disposition treats data as unusable; freshness (`staleTime`) only selects serve-as-is vs SWR and never gates eligibility. The shipped cache evaluates validity at *read* time, so a deadline-less snapshot would silently rot: a gesture starting past `dataUsableUntil` treats the disposition as stale (revision-mismatch path, republish). The fast path requires `fresh` or `stale` within deadline — serve the retained scene, revalidate after commit; `unusable` → outcome **(a)**, the discrete pop (D4) |
| `gestureEnabled` | The screen's resolved `gestureEnabled`/`gestureOptions` |
| `blockerDisposition` | `none` / `blocked` — an active blocker fails the gesture pre-activation. Blocker predicates are re-evaluated at publish points (registration/removal, predicate-dependency change, committed navigation); a registration/removal additionally bumps `policyRevision`, while an unobserved predicate flip between publishes is caught by the confirmation's **full canonical MATCH re-run** (§4), not by the revision compare |
| `dismissDisposition` | Interactive-dismiss policy of the topmost dismissable layer / sheet, mirrored from the behavior-primitive registrations at publish time. **P2 scope, dismiss claimants only:** this field gates *dismiss-claiming* gestures (sheet/modal drag), never a P1 stack-pop gesture — a non-dismissable sheet containing a poppable inner stack still gets the inner-stack gesture (the cascade already chose the inner pop; D2). Its authoritative source (a behavior-registration schema — RFC 0007 deferred gesture dismissal) is a named P2 build item |
| `motionDescriptor` | The **visual** preset + parameters the controller will instantiate (slide, parallax factor, spring config, durationMs). Deliberately distinct from the shipped `TransitionDirective.preset`, which is *navigation intent* (`"push" \| "pop" \| …`) — the two never share a field. Together with `backTarget` this is a complete transition plan: Motion constructs the controller and mints the transition id from it with **no runtime-thread object** (data-flow step 4) |
| `reducedMotion` | Effective reduced-motion flag (§12) |

**Half 2 — main-owned presenter readiness (never crosses a thread):**

| Field | Meaning |
|---|---|
| `presenterGeneration` | Bumped on presenter replacement, root swap, HMR scene teardown, **and retained-scene attach/detach/replacement/eviction** — any change to what is synchronously renderable |
| `screenDims` | Current width/height for progress math — main-side truth; a live resize updates it with no router round-trip |
| `retainedSceneAvailable` | Advisory pre-signal: whether a retained, synchronously renderable scene for the current back target exists (§4b, D4). **Advisory only — this boolean never authorizes activation** |

Activation is authorized by an **atomic main-owned acquisition**, not by
reading flags: `acquireSceneLease(rootId, backTargetHistoryEntryId,
structuralRevision, presenterGeneration)` returns a **`SceneLease`
bound to that exact history entry and both freshness stamps**, or fails. A live lease
keeps both scene layers renderable until Motion releases it at settle (or
the confirmation timeout fires — see staleness rules). Because
acquisition names the disposition's `backTarget`, the two halves cannot
be joined stale: a retained scene for the *wrong* target, or a presenter
whose generation moved between read and acquire, fails acquisition and
the interaction degrades per D4. Two leases, two jobs: the arena's
`InteractionLease` (LLP 0336; P1 builds the minimal single-claimant form
— see the build table) resolves *contention* and is authoritative at
activation; the presenter's `SceneLease` guarantees *renderability*.
Order: the `SceneLease` MAY be acquired early (recognizer Began, before
slop/axis-lock — data-flow step 4) as **pre-warm**, and is revalidated
against both revisions and the presenter generation at activation, so
pre-warm never front-runs the arena; authoritative sequencing is
arena-first at activation, and release runs in reverse order at settle.
Every **pre-activation terminal** releases a pre-warm lease immediately,
RAII-style — sub-slop release, axis lock lost to another claimant, arena
denial, system takeover, disabled screen — and a pre-warm lease lives at
most one gesture stream. Memory pressure composes simply: eviction while
*parked* flips `retainedSceneAvailable` (degrade outcome (a)); a **live**
lease — acquired, gesture in flight — is an eviction barrier for its
duration.

Publisher and cadence: the router (runtime thread) writes the disposition
through on **every committed navigation; every blocker, back-handler, or
dismissable-layer registration/removal or predicate-dependency change; and
any other change to the cascade's answer** — the same eager-disposition
rule LLP 0310 imposes on Android, where the host must know the back
disposition before the system gesture begins (LLP 0310 §1.1 is this same
property expressed as a callback-registration state machine). iOS does not
need Android's system registration, but it needs the same invariant:
**back disposition is pushed ahead of need, never pulled at gesture
time.** Transport: the disposition rides an extension of the
committed-navigation host payload that already pushes per-root
`canGoBack`/`canGoForward` into main-side stores (the ENG-22437/22454
lineage) — a proven channel, not a new one.

Staleness rules:

- Motion MAY start the gesture optimistically from the halves it has,
  recording the structural revision, policy revision, and presenter
  generation on the in-flight transition. The first
  visual frame does not wait for the router.
- The router's asynchronous confirmation (data-flow step 6) carries the
  revisions it computed from — and the outcome follows the r12 §4a
  classification, never a blanket mismatch rule: a STRUCTURAL
  divergence (navigation, route change, target gone) resolves as
  **cancel-with-snapback** via the durable `nacked` cell state; a
  POLICY change is decided by the MATCH re-run (a re-run FAILURE —
  blocker/access/handler flip — confirms `confirmedCancel`; a benign
  bump that re-authorizes cleanly commits normally). A gesture never
  commits from stale truth — the §4 commit transaction's dispatch
  enforcement is the last line either way. (Event-classification
  matrix, one row per change class: navigation/route/target =
  structural → immediate nacked; blocker/access/handler/data-deadline
  = policy → re-run decides at confirmation, dispatch enforcement
  re-checks at commit, forced-cancel resolves at release; the owning
  gesture's own cascade re-publication = reentrancy signal → bumps
  NEITHER revision; a Back-mode eligibility change for an EXISTING
  lease is policy-only — mode changes never structurally invalidate a
  live stream, matching 0336's checkpoint model.)
- **Release before confirmation.** The user can release before the
  router's ack arrives (routine under runtime-thread pressure — the P1
  gate manufactures exactly this). Tracking and the settle spring never
  wait: the visual settle proceeds in the direction Motion resolved. A
  *cancel* needs no confirmation — logical discard of a never-confirmed
  candidate is unconditional. A *commit*'s logical promotion is **held
  pending the ack** (§4b `settled-visual` row): on ack the queued
  resolution applies normally; on nack — or a mismatch discovered at
  confirmation — the presentation **reconciles to logical truth** with
  the same jump-to-truth re-render as the HMR interruption row. A gesture
  never *logically* commits from stale truth; under pressure it may
  visually settle first and then reconcile. The wait is **bounded**: if
  no ack/nack arrives within the confirmation timeout (the new constant
  lands in `kernel/src/motion.rs`; the one-second precedent is the
  pager's host-side turn-token timeout in `ExactPagerMotion.swift`. P1
  starts at that 1.0s value, **per root**, recorded as a fitted
  parameter — a held `settled-visual` is input-inert and visually at
  rest, so the cost of waiting is delayed logical effects, not jank),
  the held commit is treated as **nacked** —
  jump-to-truth reconcile, lease release — and the transition id becomes
  a **tombstone**: late acks, late nacks, and duplicate resolutions for
  it are defined no-ops. The tombstone is not main-side bookkeeping
  alone — it is a **shared atomic `DecisionCell`**: one root-lifetime
  CAS word per root, re-armed per in-flight transition, living in the
  kernel's **cross-domain control plane beside the outcome log** (0099
  r9's adjudication and its record-class table row — deliberately NOT a
  §4.5 SharedValue slot: the value plane stays single-writer main-side,
  and the slab FFI exposes no runtime-side CAS accessor; decision
  records are the design's sole two-writer record class). The word
  packs `(rootInstance, transitionGeneration, state)` — Motion re-arms
  it at mint time under a fresh transition generation, so a late CAS
  for transition N fails against the cell re-armed for N+1, and a late
  CAS from a destroyed root fails against any successor root's cell
  (the control region follows the value plane's physical-lifetime
  rules: stable addresses, teardown TOMBSTONES under a
  root-instance-qualified generation and never frees, physical release
  only at engine shutdown behind the quiescence barrier, re-tenancy
  watermark-gated — ABA-proof across re-arm AND root
  destroy/recreate). Both CASes are AcqRel, and the logical commit is
  ordered after the winning CAS. **The state machine is CLOSED —
  every authorization outcome is a durable cell state (r12)**: the
  runtime CASes `pending → confirmedAllow` (authorization holds),
  `pending → confirmedCancel` (a policy flip the MATCH re-run failed
  on — forced cancel; tracking unaffected, release resolves cancel, no
  commit can ever be authorized), or `pending → nacked` (structural
  invalidation — the revision-mismatch and MATCH-divergence paths CAS
  this before waking), taken while processing the start notification,
  *before* any wake is sent; any of these races `pending → expired`
  (main CAS, taken when the timeout fires — and the timer **starts at
  entry to `settled-visual`**, never at gesture start). Exactly one
  writer wins
  the cell, and that CAS is THE cross-domain linearization point: a
  wedged runtime that drains its queue after main expired the cell
  finds the CAS failed and nacks — a tombstoned id can never be
  revived into a logical pop by late queue drainage — and the commit
  transaction asserts **`confirmedAllow` specifically** before
  mutating (§4): forced cancel is inside the linearized authorization,
  never a hint riding a message. **Ack/nack are level-triggered wakes
  over the durable cell, not the decision** — main re-reads the word
  on wake, so a lost or duplicated wake costs presentation latency
  only, by construction; an ack bearing forced-cancel against a held
  commit is processed as a nack (jump-to-truth reconcile; SSE terminal
  outcome `interrupted` — the `settled-visual` row names this path,
  and it is a P1 reference-model trace). Epoch reset tombstones
  EVERY armed state — pending AND the confirmed/nacked states — in the
  main-side sweep, so BOTH sides' subsequent CAS fail, and a
  `confirmedAllow` whose commit never ran resolves to reconcile, never
  a post-reset pop (the HMR row below; confirm-then-reset-before-commit
  is a named Loom trace). **Progress is wake-independent**: main loads
  the cell at release processing, at `settled-visual` entry, and at
  timeout — so a lost wake costs latency bounded by the next scheduled
  load, and main's expiry CAS failing against `confirmedCancel` or
  `nacked` routes to the cancel/nack path exactly as the wake would
  have. **Post-confirm wedge
  posture:** if the runtime confirms and THEN wedges (before the wake
  or the commit transaction), main's expiry CAS fails and the held
  `settled-visual` state has no transition-machinery bound — that state
  is input-inert and visually at rest, and a wedged runtime thread
  means the app is globally dead anyway, so recovery is delegated to
  the standard runtime-stall watchdog/diagnostics, not to transition
  timeouts. Until the cell is confirmed, Motion's resolution is a
  **kinematic intent**, not an authorized commit — 0099's terminology
  bridge states this exactly: exactly-one **authorization** (the cell)
  + at-least-once record transport with deduplicated effect. (The
  pager's one-second precedent starts after semantic dispatch; this
  timer's origin at `settled-visual` entry — after release, never
  during tracking — is deliberately its analogue.)
- If no snapshot halves are available (publish gap, cold start), the
  interactive mode is ineligible and the SAME recognizer runs its
  discrete mode (RFC 0061's behavior) — nothing "fails"; §2's
  one-recognizer-two-modes topology means eligibility selects the mode. **Separately:** if `gestureEnabled` is false, the recognizer
  fails and *no gesture path runs at all* — not the discrete fallback
  either, since RFC 0061's discrete edge swipe is equally gated by
  `gestureEnabled`. (Null `backTarget` — at root — makes the
  INTERACTIVE mode ineligible, but does NOT by itself end the story:
  per RFC 0061's handler-observability rule, **`handlerMounted: true`
  keeps the discrete path eligible with no pop target** — the discrete
  dispatch runs the mounted handlers exactly as a hardware back press
  would, and only when every handler declines does the dispatch-time
  fallback apply. Without that rule a root-level `useBackHandler`
  could never receive an edge swipe (the hole 0061 names;
  `BackActionState` lacks the field in shipped code — ENG-24664, which
  also aligns the shipped `startInteractiveBack` handler/blocker order
  to `back()`'s blocker-first order, the order this document's failure
  suite pins). Only null `backTarget` AND no eligible mounted handler
  fails the recognizer outright — nothing to pop, nothing to run, no
  dead-feeling drag.)
- A blocker that appears **mid-gesture** bumps `policyRevision` (an
  ordinary eligibility flip, never structural invalidation): tracking
  may continue (the finger owns the frame), but release resolves as
  cancel-with-snapback regardless of progress and velocity, and the
  blocker UI shows after settle.

#### 4b. Two-phase resolution: logical commit vs. visual settle

`state.provisional` means the **logical navigation candidate** — nothing
else. Scene retention is **presenter-owned** and generation-fenced.
Logical history mutates when the router processes the gesture resolution
(held pending confirmation when release outran the ack — §4a); the scene
layers stay alive until Motion reports the settling spring at rest.

**What retention means operationally (and why bindings survive
settling):** retention is expressed through the **rendered route set**
(RFC 0010 Invariant 7 / RFC 0046's both-trees rule). The resolved-away
route — outgoing on commit, incoming on cancel — remains in the rendered
set as a **transition-retention entry** (rendered, input-inert) from
resolution until Motion reports settled; only then is the entry removed
and the unmount emitted. Because the route never leaves the rendered set
mid-settle, **no node-destroy or binding-detach commits ride the ring
during settling** — LLP 0297's detach-before-destroy ordering is
respected trivially, and the settle spring always has live binding
targets. Retention entries are **render-only** (so registered in RFC
0010's Invariant 7): they keep pixels and bindings alive but are
excluded from loader execution, revalidation, focus, and scroll
restoration — a departing scene never loads. "Presenter-owned teardown"
means exactly this: the presenter's settled signal is what releases the
retention entry (and the §4a scene lease). The full lifecycle:

| State | Owner | Domain / thread | Retained scenes | Router state | Agent `interactivePhase` |
|---|---|---|---|---|---|
| `idle` | — | — | current | `matches` | — |
| `gesture-possible` | Motion | main | current (+ back target if leased) | `matches` | — |
| `gesture-active` | Motion | main | outgoing + incoming | `matches` + `provisional` | `gesture-active` |
| `settling-commit` | Motion | main | outgoing + incoming (fenced) | `matches` promoted; `provisional` cleared (or promotion pending — see next row) | `settling-commit` |
| `settled-visual` (commit pending confirmation) | Motion + presenter | main | outgoing + incoming (fenced, input-inert) | promotion HELD pending ack (§4a); ack → `settled`; nack or bounded timeout → jump-to-truth reconcile, id tombstoned | `settled-pending-confirmation` |
| `settling-cancel` | Motion | main | outgoing + incoming (fenced) | `provisional` cleared | `settling-cancel` |
| `settled` | presenter | main | survivor only | quiescent | `settled` |
| `failed-before-provisional` | Motion | main | current | untouched | — |
| `interrupted` | router + presenter | runtime + main | see below | logical truth | `interrupted` |

Interruption semantics, by phase:

- **During `gesture-active` (racing navigation = structural
  invalidation):** a navigation committed by the runtime thread while the
  finger is down — a `structuralRevision` bump — is **structural
  invalidation** under LLP 0336's lease rules ("before the semantic commit, structural invalidation … cancels
  the stream: spring back, no dispatch") — the scene set under the
  gesture changed. The stream cancels **immediately**: tracking ends, the
  settle spring runs cancel-with-snapback from current progress and
  velocity, and after settle the presentation reconciles to logical truth
  (the racing navigation's result). Deliberately *not*
  freeze-until-release: tracking frozen ghost layers and then snapping
  back to a screen that no longer exists reads worse than one clean
  snapback-then-apply, and 0336 owns invalidation timing. This differs
  from RFC 0046's `finish()`-commits rule for the finger-down case
  (declared in the amendments block; `finish()` still governs settling
  phases). Contrast the mid-gesture *blocker* (§4a): that is an ordinary
  eligibility flip — 0336 handles those at the commit checkpoint, never
  by yanking an active lease — so there tracking continues and release
  resolves cancel.
- **During settling:** a new navigation completes the settle instantly
  to its **resolved endpoint** — commit settles to 1, cancel settles to
  0; interruption never converts a cancel into a commit — and then
  applies. (This makes 0046's `finish()` **resolution-preserving** for
  interactive transitions; its fast-forward-and-commit wording continues
  to govern unresolved non-interactive transitions. Registered in the
  0046 amendment note.) In `settled-visual`, a racing navigation is
  processed after the held promotion resolves (ack/nack/expiry), in
  queue order.
- **HMR reset or route reload:** tears down both retained layers and
  re-renders logical truth (the promoted or restored `matches`) with no
  transition.
- **Host restart:** recovers to logical truth by construction.
- **A new gesture during settling or `settled-visual`:** the recognizer
  **fails until `settled`** — P1 does not support catching the snapback
  or chaining flicks mid-settle (stock iOS largely disallows it, and RFC
  0046's old evaluate-at-new-touch rule predates Motion-owned thresholds
  and does not carry over). The readiness halves report not-ready during
  these phases, and a `SceneLease` is never granted while a retention
  entry for the same root is live. Rapid chained back flicks therefore
  resolve as: settle completes (fast — the spring starts from rest),
  then the next gesture begins.

In every case logical state is already correct (or deterministically
pending exactly one queued resolution) at interruption time, because
resolution — not settle — is the logical boundary; only presentation is
interrupted.

Correlating "which runtime re-render belongs to this resolution" reuses
the pager's **turn-token shape** (`ExactPagerMotion.swift`'s
generation-aware token over the descriptor protocol — host-side today,
policy migrating into Motion at 0099 M4) rather than inventing a second
correlation vocabulary — adopting its **LLP 0336 r9 repair invariants,
not just its vocabulary** (the correlation shape shipped; its ack
inference shipped *defective* — an args-only hash whose counter never
increments, ENG-24305 — and r9 defines the fix):

1. The promotion transaction carrying the transition id as its token is
   **exempt from update-economy equality elision by construction** —
   ENG-24189's byte-equality cutoff never swallows a token-bearing
   transaction.
2. The **no-tree-delta case is defined:** on the retained-scene fast
   path the promotion can be render-light to the point of emptiness, so
   the token also rides the committed-navigation host payload (the §4a
   transport), which is never elided — the presenter always observes
   logical truth applying even when no presenter-visible tree delta
   exists.
3. **Release condition, exactly:** the retention entry releases on
   (**settled AND token observed**); on the §4a timeout/tombstone path,
   reconcile-applied substitutes for token-observed. Settle alone never
   releases a commit's retention entry.

The handle-side surface of this lifecycle is a **registered extension**
(RFC 0046 TransitionHandle note + RFC 0010 lifecycle carve-out,
2026-07-11; shape corrected 2026-07-13): a scalar status cannot both
reach a terminal `settled` and keep the outcome readable, so the
registered shape is **phase + resolution as orthogonal fields** —
`phase: running | settling | settled` and
`resolution: commit | cancel | interrupted | null` (resolution set at
resolution time, durable after `settled`; adapters synchronously read
commit-vs-cancel-vs-interrupted after settlement instead of inferring
it from a consumed scalar). `navigation.transitionEnd` fires on the
`settled` edge, not at resolution. The shipped scalar
`TransitionHandleStatus` remains for non-interactive handles;
interactive handles expose the pair, registered in RFC 0046 (handle
authority) and RFC 0010 (lifecycle).

#### 4c. Progress channels

Progress exists on three channels with different cadences:

1. **Motion hot path** (main, frame-rate): the controller's progress
   value and the Motion-local per-property values derived from it,
   applied through the presenter-composited channel (step 4c). No JS,
   no router, no per-property slab writes.
2. **Presenter writes** (main, same frame): compositor-path properties
   applied per LLP 0297 §4.4.
3. **Observability** (runtime thread + agent surface, coalesced): the
   latest-value plane — concretely, the transition's progress slot in the
   LLP 0297 §4.5 SharedValue slab, read through the slot's §4.5
   synchronization: a **per-slot atomic** for this single-word value
   (seqlocks are §4.5's multi-word-record primitive, not this) — is
   sampled/throttled into `handle.reportProgress` updates, router hooks,
   and the SSE `navigation.transitionProgress` events (§11). Coalescing
   follows the shipped rule: the server defaults to **10fps** and caps
   explicit `transitionProgressFps` requests at 60fps. A change in the closed
   interactive-phase union bypasses the sample throttle once so release
   velocity and settle classification cannot be hidden by a same-timestamp
   tracking sample; continuous same-phase progress is never one event per
   display frame. As built, the focused native router owns one identity-fenced
   100 ms async sample timer while the gesture or spring is active. Its host
   read samples the validated progress slot plus raw finger velocity during
   `gesture-active` or progress-spring velocity (converted back to points per
   second) while settling; the reduced-motion opacity fade has no geometric
   spring and therefore samples velocity `0`. It permits only one read in flight and rejects a
   response whose host phase no longer matches the router phase. The timer
   stops at `settled-pending-confirmation` (which restores the release
   velocity), `settled`, or `interrupted`; an older host's unknown-op response
   disables it for that session. (RFC 0046 specified `reportProgress` as a per-frame
   adapter call; this channel structure is the declared amendment — see
   the amendments block.)

Gesture tracking and compositor writes never depend on a JS callback at
frame cadence. Motion owns threshold resolution for the built-in presets
(Design Principle 3); a stalled runtime thread degrades observability and
delays logical confirmation — it never degrades tracking.

### 5. Shared Element Transitions

Shared element (hero) transitions animate a specific element from its position in the outgoing screen to its position in the incoming screen, while the background cross-fades.

#### Developer API

```tsx
import { SharedElement } from 'exact/router';

// In the list screen:
function PhotoGrid() {
  return photos.map(photo => (
    <Link href={`/photo/${photo.id}`} key={photo.id}>
      <SharedElement tag={`photo-${photo.id}`}>
        <Image src={photo.thumb} style={{ width: 100, height: 100 }} />
      </SharedElement>
    </Link>
  ));
}

// In the detail screen:
function PhotoDetail({ params }: { params: { id: string } }) {
  return (
    <SharedElement tag={`photo-${params.id}`}>
      <Image src={photo.full} style={{ width: '100%', aspectRatio: photo.ratio }} />
    </SharedElement>
  );
}
```

The `tag` is the matching key. The router finds elements with the same tag across the outgoing and incoming routes and animates between their measured frames. (Examples shown in React — the fully supported secondary tier. Per LLP 0160 §5.2 the Contract bindings of `SharedElement` and `defineTransition` ship first when P3/P4 land; D1's pre-P3 checklist includes the Contract spellings.)

#### How it works

```
 1. Navigation starts (push to photo detail)
 2. Router enters TRANSITION-PREP
 3. Kernel measures all SharedElement frames in the outgoing tree
    → { tag: "photo-42", frame: { x: 20, y: 340, w: 100, h: 100 } }
 4. Router renders incoming route tree (off-screen or at opacity 0)
 5. Kernel measures all SharedElement frames in the incoming tree
    → { tag: "photo-42", frame: { x: 0, y: 0, w: 390, h: 520 } }
 6. Router matches tags → SharedElementPair { tag, sourceFrame, targetFrame }
 7. Kernel creates shared element animation controller:
    a. Create the floating transition element per D1: a presenter-level
       retained clone of the source view subtree (rasterization is a
       measured upgrade path, not the default; web uses the View
       Transitions API)
    b. Position the floating clone at sourceFrame
    c. Hide both the source and target elements
    d. Animate the clone: sourceFrame → targetFrame (spring physics)
       - Position interpolation
       - Size interpolation
       - Border radius interpolation
       - Optional: content crossfade (thumb → full resolution)
 8. Background screens cross-fade simultaneously
 9. When animation settles:
    a. Remove the floating clone
    b. Unhide the target element
    c. Clean up
```

#### Kernel representation

```rust
pub struct SharedElementController {
    tag: String,
    source_frame: Rect,
    target_frame: Rect,
    /// The floating clone node — a presenter-level overlay owned by the
    /// transition system, not part of either app tree. Under LLP 0297 the
    /// main-owned Motion domain never clones or mutates the kernel tree;
    /// the clone lives in the presenter's layer space (see D1).
    clone_node: ViewId,
    /// Interpolated frame values — Motion-LOCAL floats, same rule as §3
    /// (the value plane holds only the reserved progress slot; the clone
    /// is presenter-space, so its frame applies through the presenter,
    /// never through slab slots)
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    /// Overall transition progress — derived from the SAME reserved
    /// progress slot §3 writes (one progress, one slot, shared)
    progress: f32,
}

impl SharedElementController {
    fn update(&mut self, progress: f32, motion: &mut MotionDomain) {
        self.x = lerp(self.source_frame.x, self.target_frame.x, progress);
        self.y = lerp(self.source_frame.y, self.target_frame.y, progress);
        self.width = lerp(self.source_frame.w, self.target_frame.w, progress);
        self.height = lerp(self.source_frame.h, self.target_frame.h, progress);
        self.radius = lerp(self.source_radius, self.target_radius, progress);
        motion.apply_clone_frame(self.clone_node, self.frame());
    }
}
```

#### Interactive shared element transitions

Shared element transitions can also be gesture-driven. The photo detail screen can support pinch-to-dismiss where the image shrinks back to its grid position:

```typescript
export const screen: ScreenOptions = {
  transition: 'shared',
  gestureEnabled: true,
  gestureOptions: {
    pinchToClose: true,   // PROPOSED (P5) extension to the shipped gestureOptions surface
  },
};
```

During the pinch gesture, the shared element clone tracks the gesture (scale and position) instead of following a spring. On commit, it springs to the source frame. On cancel, it springs back to the target frame.

### 6. Sheet Transitions with Detents

Sheets are a special case: they have multiple stable positions (detents) and the gesture can snap to any of them.

#### Developer API

```typescript
// The first three fields are the SHIPPED SheetOptions surface (RFC
// 0061-owned; packages/exact-router/src/types.ts). The four marked
// PROPOSED are P2 extensions that register through RFC 0061/0010's
// ScreenOptions extensions table when P2 implements them.
export const screen: ScreenOptions = {
  presentation: 'sheet',
  sheet: {
    detents: ['medium', 'large'],           // shipped — stable positions
    grabberVisible: true,                    // shipped — top grabber handle
    canDismissInteractively: true,           // shipped — swipe to fully dismiss
    initialDetent: 'medium',                 // PROPOSED (P2) — starting position
    largestUndimmedDetent: 'medium',         // PROPOSED (P2) — dimming threshold
    cornerRadius: 12,                        // PROPOSED (P2) — sheet corner radius
    preferredEdge: 'bottom',                 // PROPOSED (P2) — sheet origin edge
  },
};
```

#### Detent physics

```rust
pub struct SheetController {
    detents: Vec<DetentPosition>,        // Sorted from smallest to largest
    current_detent: usize,               // Index into detents
    sheet_translate: f32,                 // Current Y position (Motion-local,
    background_dimming: f32,              // §3's rule: presenter-composited
    background_scale: f32,                // channel, never slab slots)
    can_dismiss: bool,                    // Whether dragging past all detents dismisses

    // Rubber-banding when dragging past largest detent
    rubber_band_factor: f32,             // Default 0.55
}

impl SheetController {
    fn resolve_gesture(&self, position: f32, velocity: f32) -> SheetResolution {
        // Find nearest detent considering velocity
        let mut target_detent = self.nearest_detent(position);

        // Velocity bias: fast flings skip to the next detent in the fling direction
        if velocity.abs() > 500.0 {
            if velocity > 0.0 {
                // Flinging down → next smaller detent (or dismiss)
                target_detent = target_detent.saturating_sub(1);
            } else {
                // Flinging up → next larger detent
                target_detent = (target_detent + 1).min(self.detents.len() - 1);
            }
        }

        // If past all detents and dismiss is allowed
        if target_detent == 0 && self.can_dismiss && (position > dismiss_threshold || velocity > 1000.0) {
            return SheetResolution::Dismiss;
        }

        SheetResolution::SnapToDetent(target_detent)
    }

    fn nearest_detent(&self, position: f32) -> usize {
        self.detents
            .iter()
            .enumerate()
            .min_by_key(|(_, d)| ((d.y - position).abs() * 1000.0) as i32)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }
}
```

### 7. Blocker Integration

Navigation blockers (RFC 0010, RFC 0061) must intercept interactive gestures before they start.

```rust
// In the gesture activation path — decided ENTIRELY from the main-side
// published halves (§4a: BackDisposition + presenter readiness). Motion
// never queries the router synchronously (LLP 0297's
// no-sync-cross-runtime rule); the router pre-published everything this
// function reads, and every field here is in the §4a tables.
fn should_allow_interactive_navigation(
    disposition: &BackDisposition,
    readiness: &PresenterReadiness,
) -> bool {
    // Blocker disposition was mirrored in at publish time
    if disposition.blocker_disposition == BlockerDisposition::Blocked {
        return false; // no interactive stream forms — outcome (b) below
                      // (the recognizer does NOT enter a system Failed
                      // state; §7's inert-stream mechanism observes it)
    }

    // NOTE: `dismissDisposition` deliberately does NOT gate the stack-pop
    // gesture (§4a: P2 scope, dismiss claimants only) — a non-dismissable
    // sheet still gets its inner stack's pop gesture; the cascade already
    // chose the inner pop (D2).

    // P1 gating rule: the INTERACTIVE gesture drives only global
    // stack-pop dispositions (§4a `backAction`). "return false" here is
    // outcome (a) for tab-pop/window levels — the same recognizer's
    // discrete mode still commits the pop — and outcome (b) for
    // blocker/dismiss levels (inert-stream blocked-attempt below).
    if disposition.back_action.level != BackActionLevel::StackPop {
        return false;
    }

    // Custom back handlers make the INTERACTIVE gesture ineligible in P1
    // (§4a `handlerDisposition` — the shipped BackActionState level union
    // cannot represent them). Outcome (a): the discrete pop's back()
    // cascade runs the handler exactly as a button press would.
    // Note (RFC 0061 handler observability): handler_disposition derives
    // from 0061's `handlerMounted` — dispatch-time consumption cannot be
    // predicted eagerly, so "Registered" means "mounted and may consume";
    // the discrete path runs the handlers, with a no-pop-target dispatch
    // at root (ENG-24664 adds the missing wire field).
    if disposition.handler_disposition == HandlerDisposition::Registered {
        return false;
    }

    // Data disposition (RFC 0053 pop semantics): the fast path needs
    // fresh or stale-within-gcTime cache for the back target
    if disposition.data_disposition == DataDisposition::Unusable {
        return false; // degrade to the discrete pop (D4)
    }

    // Screen configuration
    if !disposition.gesture_enabled {
        return false;
    }

    // Scene availability (main-owned half; converts to a lease on start)
    if !readiness.retained_scene_available {
        return false; // degrade to RFC 0061's discrete pop (D4)
    }

    true
}
```

Outcome **(b)** has two sub-cases, split by whether a blocker exists:
**(b-blocker)** — an active blocker: the blocked-attempt notification
below, blocker UI on intent-crossing (CT-061-08/14). **(b-passive)** —
dismiss-level dispositions with no blocker (e.g. `sheet-dismiss` at a
modal's root, the shipped classifier's answer for a modal with an inner
stack at root): **nothing surfaces on intent** — a horizontal edge
swipe inside a sheet is a non-event on stock too, P2's vertical
claimant owns dismissal, and the passive observer's measurement feeds
diagnostics only. Surfacing blocker UI where no blocker exists would
be an invented interruption; the at-root-modal edge swipe is its own
failure-suite row so the distinction is tested, not asserted.

When a blocker prevents an interactive gesture **pre-activation**:
1. The recognizer never activates for tracking — no provisional state is
   created and nothing moves
2. **If the attempt crosses the intent threshold** (the same activation
   slop that would have started tracking), Motion emits an asynchronous
   `blocked-back-attempt` notification and the router surfaces the
   blocker UI, exactly once per attempt-BURST (crossings before the
   runtime consumes the outstanding blocked-attempt latch are one
   burst — one dialog for two rapid attempts is the correct UX; 0099's
   latch row states the same contract). This is what RFC 0061's
   CT-061-08/14 require — a gesture attempt against an active blocker
   surfaces the blocker UI; a silent dead drag reads as breakage. A
   sub-slop touch remains a non-event (no dialog for a graze).
   **Mechanism:** the intent threshold is measured by a **genuinely
   passive observer**, NOT by letting the system edge-pan recognizer
   begin: a begun `UIScreenEdgePanGestureRecognizer` can still PREVENT
   competing recognizers (scrolling) even with
   `cancelsTouchesInView = false` — prevention, not cancellation, is
   the leak. The inert stream is therefore a custom observer
   (a recognizer subclass that computes slop-crossing from raw touches
   and immediately transitions to `.failed`, or a non-recognizer touch
   observer) configured to (i) never delay or cancel touches, (ii)
   recognize simultaneously with everything, and (iii) sit in NO
   prevention relationship in either direction. "Fails pre-activation"
   means no controller, lease, provisional state, or visual tracking is
   created — and no competing recognizer is even momentarily
   disadvantaged: a sub-slop touch in the edge zone is a non-event *for
   the app too*, and a blocked edge drag over a horizontal scroller or
   a control scrolls/activates exactly as stock does when the pop
   gesture is unavailable (both are §9/failure-suite rows)
3. The user resolves through the blocker UI or a non-gesture back
   affordance

A blocker that registers **after** the gesture has started is a snapshot
generation bump, not a race to lose: per §4a, tracking continues while the
finger is down, release resolves as cancel-with-snapback regardless of
progress, and the blocker UI shows after settle.

### 8. Custom Transition API

Developers can define custom transitions using worklets from Motion (RFC 0099).

```typescript
import { defineTransition, SharedValue, interpolate, withSpring } from 'exact';

const cardFlip = defineTransition({
  // Called per frame during interactive gesture or animation
  apply(progress: SharedValue, screens: TransitionScreens) {
    'worklet';
    const p = progress.value;

    // Outgoing screen: rotate away
    screens.outgoing.style = {
      transform: [
        { perspective: 1200 },
        { rotateY: `${interpolate(p, [0, 0.5], [0, -90])}deg` },
      ],
      opacity: interpolate(p, [0, 0.5], [1, 0]),
    };

    // Incoming screen: rotate in
    screens.incoming.style = {
      transform: [
        { perspective: 1200 },
        { rotateY: `${interpolate(p, [0.5, 1], [90, 0])}deg` },
      ],
      opacity: interpolate(p, [0.5, 1], [0, 1]),
    };
  },

  // Spring configuration for animation (non-interactive) and gesture completion
  spring: { damping: 20, stiffness: 200 },

  // Duration hint for non-interactive transitions
  duration: 500,

  // Which gesture drives this transition interactively
  gesture: 'pan-horizontal',
});

// Usage:
export const screen: ScreenOptions = {
  transition: cardFlip,
  gestureEnabled: true,
};
```

> **Status (2026-07-05):** the object-style worklet API above is
> **illustrative and post-P1**. The implementation contract is a **typed
> binding descriptor** over supported compositor/layout properties — RFC
> 0099's binding-table model, running under LLP 0297's deliberately
> restricted UI-worklet runtime — not a general-purpose UI-thread JS
> object-mutation API. The sketch shows the developer-facing shape such
> descriptors may eventually compile from; nothing in P1 depends on it.

Under the actual contract, `defineTransition` compiles to a typed binding
descriptor plus (at most) a restricted worklet that computes SharedValue
writes on the display link; **declared bindings and the presenter apply
those values** per LLP 0297 — a worklet never mutates the view tree
directly, on any path. (The `screens.outgoing.style` spelling in the
sketch is developer-facing sugar the compiler lowers to descriptor
entries.) The descriptor schema itself — property enum, source slots,
attach/detach, compositor vs. layout-island routing, failure behavior —
is RFC 0099's deliverable — 0099's M2 (Motion graph transport) now
specifies the transport and record-shape plan in detail, but the frozen
schema lands *at* M2, not before — and it is a named **precondition for
P4**: P4 does not start until it exists (see Phasing).

### 9. Gesture Arbitration with Scroll Views

The hardest UX problem: distinguishing a back-swipe gesture from a horizontal scroll inside the screen content.

> **Authority note (2026-07-11).** Gesture *contention* — who wins a
> contested drag — is normatively owned by **LLP 0336 §6.1's v1
> arbitration arena**; this RFC's edge-back gesture participates as the
> arena's `RouterHistory` claimant, whose pairwise row ("reserved edge
> wins above scrolling, only while the Back-mode discriminant is
> claim-capable" — 0336 r15; raw `canGoBack` was the pre-r15 spelling
> and is unsafe as a gate, §9) is exactly the
> eligibility §4a's disposition fields supply.

> **Discriminant ownership (r13):** THIS document owns the four-mode
> Back discriminant's derivation and publication (it is computed from
> the §4a disposition fields this document owns — blocker present →
> `passive-intent`; `handlerMounted` with no pop target →
> `discrete-claim`; eligible stack-pop → `interactive-claim`; else
> `none` — and published with the disposition halves); 0336 cites it as
> the arena gate; ENG-24664 owns the wire fields. The native
> slice of that arena is already in `kernel/src/motion.rs`. This RFC owns
> the navigation-specific *lifecycle* (readiness, provisional state,
> two-phase resolution); everything below is the cross-platform
> orientation for how the arena's rules feel in navigation terms, plus
> the P1 test list — not a second contention authority.

#### Priority rules

```
1. System gestures always win (iOS home indicator, notification center)
2. Edge swipe recognizers have higher priority than content scroll
3. Full-screen swipe recognizers have LOWER priority than horizontal scroll
4. Vertical scroll and horizontal edge swipe race to the axis lock —
   both are candidates until slop resolves an axis, then exactly ONE
   holds the exclusive lease (LLP 0336: one lease per stream; there are
   no simultaneous winners, only fast axis resolution)
5. If content has a horizontal scroll view:
   a. Edge swipe still works (starts in edge zone, not in scroll content)
   b. Full-screen swipe is disabled for that screen
```

#### How the rules map to the 0336 arena (informative)

There is deliberately **no second arbitrator** — the rules above are what
LLP 0336 §6.1's pairwise table produces for the navigation claims. Rule 1
is the arena's `System` claim; rule 2 is the `RouterHistory` row,
gated on the router's published **Back-mode discriminant** (r12; 0336
r15): `none | passive-intent | discrete-claim | interactive-claim`,
derived from the full disposition — only the claim-capable modes enter
the arena. An active blocker maps to `passive-intent` (the §7 passive
observer + blocked-attempt record; NO arena claim, no prevention — raw
`canGoBack` is true during a blocker in shipped code, which is exactly
why the discriminant, not `canGoBack`, is the gate); `handlerMounted`
with no pop target maps to `discrete-claim` (RFC 0061); rules 3
and 5b are the same-axis-consumability eligibility read at activation (a
horizontally-scrollable ancestor that can consume the pan beats a
full-screen swipe) — stated as the **proposed** rule for LLP 0336's
open question 7, which still owns full-screen-swipe arbitration:
**`fullScreenBackSwipe` is NOT in P1 scope** and stays disabled until
0336 registers the full-screen contention rule under its §6.1 authority
(the arbitration half of its OQ7; the schema half is resolved HERE in
§2 — 0336 r10+ delegates the canonical gesture-option choice to this
document, so each owns one half and neither waits on the other); rule 4
is the axis lock. Arbitration outcomes are
observable as **arena receipts** (`kernel/src/motion.rs`), which is what
the P1 tests assert — including for failed activations, where no
`interactivePhase` ever appears and the receipt is the only evidence.

#### Apple-bridge arbitration (P1)

The rules above are the cross-platform orientation (LLP 0336 §6.1 is the
contention authority — see the note at the top of this section); on Apple
platforms in v1 they are enforced through the hybrid boundary (§2) —
platform recognizer delegates informed by main-side presenter data and
registered as the `RouterHistory` claim in the 0336 arena, not a pure
Rust recognizer graph. Concretely:

- The presenter snapshot exposes per-node **scroll metadata** to the
  native recognizer delegate: axis, content offset, scrollable extent,
  and whether the node sits at its leading edge. (The inspector already
  tracks observed scroll offsets — ENG-22790; this is the same data made
  main-readable for arbitration.)
- The edge recognizer (`UIScreenEdgePanGestureRecognizer`) wins over
  content scrolls by platform behavior — nothing to add. **Honesty
  gate on this seam:** where these rules depend on native
  interaction-state publication (live scroll capacity, modal/focus
  state, router claims), LLP 0336's OQ16/OQ17 — the publication
  channel and the snapshot publishers — are still UNDECIDED there, and
  0099 gates its M4 on the same two decisions. Those two decisions are
  therefore **explicit P1-entry gates for the arbitration rows of this
  section** (the pure platform-default rows stand on their own): P1
  does not start its arena-integration work until 0336 records
  OQ16/OQ17, and the P1 correctness report cites the recorded answers.
- A **full-screen** back swipe (`fullScreenBackSwipe`) may begin only if
  no horizontally-scrollable ancestor of the touch target can scroll
  toward its leading edge (the pan cannot be consumed as a scroll);
  otherwise the scroll wins, matching priority rule 3.
- D2's deepest-stack rule composes at activation time: the recognizer
  delegate consults the readiness snapshot's `backAction` (§4a) — the
  cascade has already chosen the target stack. Scroll arbitration decides
  only whether a *gesture* begins, never *what it pops*.

P1 arbitration test list (these ride the failure-mode suite in Phasing):
a horizontal carousel whose bounds touch the left edge zone; vertical
scroll vs. edge swipe racing to the axis lock — assert exactly one
winner and its arena receipt; a modal containing a navigation stack at
depth > 1 (inner stack pops — D2); the root screen with no pop target
(recognizer fails cleanly — asserted via the arena receipt, no
dead-feeling drag).

### 10. Platform-Specific Behavior

| Behavior | iOS | Android | Web | macOS |
|---|---|---|---|---|
| Back gesture | Left edge swipe | System predictive back only — the system owns *both* edges; Exact implements no Android edge recognizer (LLP 0310 §5) | Optional | Two-finger swipe |
| Edge zone | 20pt from left | System-owned (n/a) | N/A | Full trackpad |
| Parallax factor | 0.3 (Apple standard) | 0.0 (no parallax on Android) | 0.3 | 0.3 |
| Shadow | Drop shadow on outgoing | No shadow (elevation system) | Box shadow | Drop shadow |
| Modal dismiss | Swipe down on grabber area | Swipe down | Click backdrop | Swipe down |
| Sheet detents | UISheetPresentation-style | Bottom sheet pattern | CSS-driven | Popover or sheet |
| Shared elements | Kernel-driven FLIP | Kernel-driven FLIP | View Transitions API | Kernel-driven FLIP |
| Spring config | Damping: 28, Stiffness: 320 | Damping: 24, Stiffness: 280 | Damping: 28, Stiffness: 320 | Damping: 28, Stiffness: 320 |

Windows: deferred — deliberately no column yet; it follows macOS
trackpad semantics when scheduled (mirroring RFC 0099's explicit
Windows deferral gates).

### 11. Agent Observability

Interactive transitions expose rich state to the agent API. **This
section is an additive v2 amendment proposal registered through RFC
0052's process** (see the amendments block): bare `GET /agent/navigation`
stays frozen v1 per RFC 0010 Contract 3, so everything here rides the
existing `?v=2` surface; the shipped `TransitionSnapshot.phase` enum
(`idle | running | settled`) is untouched, with interactive detail landing
as a **new sub-state field** so existing consumers keep working.

```
GET /agent/navigation?v=2&rootId=0
{
  "transition": {
    "id": "tr_abc123",
    "type": "navigate",         // shipped required field
    "kind": "slide",
    "direction": "backward",
    "interactive": true,
    "progress": 0.42,
    "velocity": 312.5,          // NEW (v2 additive): pt/s, signed toward commit
    "from": "/settings",        // shipped v2 WIRE shape: string hrefs (the
    "to": "/home",              // router-internal TransitionSnapshot's route
                                // descriptors do not ride this wire)
    "durationMs": 350,          // shipped required field
    "reducedMotion": false,     // shipped required field
    "phase": "running",         // shipped enum, unchanged
    "interactivePhase": "gesture-active"
        // NEW (v2 additive) — §4b vocabulary: gesture-active |
        // settling-commit | settling-cancel |
        // settled-pending-confirmation | settled | interrupted;
        // null for non-interactive transitions
  },
  "provisional": {
    "path": "/home",
    "active": true
  }
}

// Driving gestures synthetically — two lanes, deliberately distinct:
//  - FIDELITY / ARBITRATION tests use the real input path:
//    exact_touch_sequence drives actual platform recognizers (the same
//    channel ENG-23094 validated), so edge activation, scroll
//    coexistence, and velocity handling are exercised for real.
//  - DETERMINISTIC LIFECYCLE tests (state-machine rows, §4b) may use a
//    registered navigation-gesture operation that forces a scripted
//    progress curve and resolution. Operation REGISTRATION goes through
//    the `agent-operations` authority in exact-contracts.json (that
//    boundary owns op registration; RFC 0052 owns the wire payloads) —
//    NOT a new bespoke /agent/interact endpoint, which does not exist.

POST /agent/wait
// Visual settle (Motion/presenter truth — §4b `settled`):
{ "condition": "animationSettled", "timeout": 5000 }
// Logical settle (router truth):
{ "condition": "navigationSettled", "timeout": 5000 }
// Both conditions already exist on the shipped wait surface; interactive
// transitions define animationSettled to become true at §4b `settled` —
// settle-spring rest AND retention release, not spring rest alone. No new
// wait condition is introduced. Web semantics are unchanged until the web
// lane adopts §4b: the existing web interactive lane keeps its current
// animationSettled behavior, and `interactivePhase` is null on web until
// that adoption (not just for non-interactive transitions).

// SSE events during interactive transitions. Snippets are PARTIAL — they
// show only the fields under discussion; the shipped envelopes carry
// their full registered field sets (rootId, kind, direction, durationMs,
// reducedMotion, from/to hrefs, requestId) unchanged.
event: navigation.transitionStart
data: { "id": "tr_abc123", "kind": "slide", "interactive": true, … }

event: navigation.transitionProgress
data: { "id": "tr_abc123", "progress": 0.42, … }   // coalesced (§4c): 10fps default.
    // During settled-visual, emits the pinned endpoint (always 1.0 —
    // only commits hold; a cancel never enters settled-visual) at the
    // coalesced cadence until settled — agents see the held state, not silence.

event: navigation.transitionInterrupted
data: { "interruptedId": "tr_abc123",              // shipped payload — NOT a bare
        "interruptedProgress": 0.42,               // "id": interruptedId/Progress +
        "newId": "tr_def456", "newKind": "slide", … }   // the replacing transition
    // Mapping rule: this event fires only when a REPLACING transition
    // exists (racing navigation, settling interruption) — its new* fields
    // are total. §4b interruptions with no replacement (HMR/reset
    // teardown, host restart) do NOT ride this event; the transition
    // terminates directly via transitionEnd with outcome "interrupted".

event: navigation.transitionEnd                    // fires at SETTLED (§4b), not at
data: { "id": "tr_abc123", "outcome": "completed", … }   // logical resolution
    // outcome uses the shipped vocabulary: "completed" | "interrupted" |
    // "cancelled" — a committed gesture is "completed"; a user cancel (or
    // §4a confirmation-mismatch cancel-with-snapback) is "cancelled"; the
    // nack/timeout jump-to-truth reconcile, no-replacement teardowns, AND
    // the §4b racing-navigation snapback (which has a replacing
    // transition and rides transitionInterrupted, per 0046's interruption
    // lineage) are "interrupted". There is no separate "committed"
    // boolean.
```

Failed activations (recognizer never activates — root, disabled,
blocked, unusable data) never appear in `interactivePhase`; their
assertion surface is the LLP 0336 **arena receipt** plus
`GET /agent/diagnostics`, which is what the §9 P1 arbitration tests
consume. The published `BackDisposition` halves (fields + revisions) are
also exposed through `GET /agent/diagnostics`, so revision-mismatch and
publish-gap rows assert directly against published state rather than
inferring it from behavior.

### 12. Accessibility

#### Reduced motion

Reduced motion is one policy with three rules. (This reconciles the March
text with RFC 0046's reduced-motion override, which remains normative for
non-interactive transitions — the two documents previously disagreed.)

1. **Non-interactive transitions** — button/link/programmatic navigation,
   and any transition not driven by a live finger — follow RFC 0046: the
   transition is replaced by its reduced-motion override (150ms fade by
   default).
2. **While the finger is down**, direct-manipulation tracking is allowed
   — motion caused by the user's own hand is not vestibular-hostile — but
   the decorative extras (parallax, shadow, dim interpolation) are
   minimized or disabled.
3. **On release**, commit/cancel resolves with RFC 0046's 150ms fade —
   matching its MUST for gesture-driven commit/cancel exactly — never a
   velocity-inheriting spring. The fade's geometry is **per-layer**: the
   **departing** layer (the resolution's loser) freezes at its release
   values and fades out; the **surviving** scene renders at its
   **final** values from fade start, its position reset covered by the
   departing layer's opacity. No layer animates translation during the
   fade (translation is the motion being reduced); "no jump-to-final"
   constrains the *departing* layer only — the survivor's reset at fade
   start is the sanctioned discontinuity, invisible behind the fade. Commit/cancel
   *thresholds* (velocity + distance) are unchanged.

Shared element transitions cross-fade at source/target positions (no FLIP
travel). Sheet detent-to-detent changes are instant snaps (a detent move
is not a navigation, so RFC 0046's fade rule does not govern it); a sheet
*dismiss* is a navigation and takes the 150ms fade on release like any
other commit.

Rules 2–3 compose unchanged with the §4a/§4b confirmation machinery:
under reduced motion the 150ms fade simply replaces the settle spring —
`settled-visual`, held promotion, the bounded timeout, and tombstone
semantics are identical.

#### VoiceOver / TalkBack

- During an interactive transition, the outgoing screen remains the accessibility focus owner
- On commit, focus transfers to the incoming screen's first accessible element
- On cancel, focus stays on the outgoing screen (no change)
- `accessibilityViewIsModal` on the topmost screen during transitions
  defers to LLP 0336's `accessibilityModal` severing marker — 0336 owns
  interaction-time AX severing semantics; this line applies its marker
  rather than defining a second rule

#### Switch Control / Keyboard

- All interactive gesture transitions have a non-gesture equivalent:
  - Swipe-back → back button / `Cmd+[` / hardware back
  - Sheet dismiss → close button / `Escape`
  - Shared element → standard push/pop (gesture is an enhancement, not the only path)

## Implementation Plan

### Phase 1: Edge Swipe Back (2 weeks)

*March dependency note, superseded: RFC 0099's Phase 1/2 breakdown no
longer exists as written (the SharedValue slab shipped with LLP 0297 W2;
raw-intake recognizers were superseded by the hybrid boundary). The
operative dependency story is the Phasing section's reuse/build table.*

- `EdgeSwipeRecognizer` in kernel with edge zone detection and vertical fail check
- `NavigationTransitionController` with slide preset
- Provisional state integration with router
- Commit/cancel with spring completion
- iOS input bridge (UIKit touch → kernel)
- **Milestone:** Swipe back from left edge, screen follows finger, spring to commit or cancel

### Phase 2: Modal and Sheet Dismiss (1 week)

- Vertical pan recognizer for modal drag-to-dismiss
- `SheetController` with detent snapping and rubber-band physics
- Blocker integration (gesture fails when blocker active)
- Background scale + dim animation for modal stack effect
- **Milestone:** Present modal, drag down to dismiss with physics

### Phase 3: Shared Element Transitions (2 weeks)

- `SharedElement` component with tag registration
- Kernel frame measurement for tag matching
- Floating-element creation on native (presenter-level retained clone per
  D1; rasterization is the measured upgrade path, not the default)
- `SharedElementController` with FLIP interpolation
- Web fallback via View Transitions API
- **Milestone:** Photo grid → detail with hero animation

### Phase 4: Custom Transitions and Polish (1 week)

- `defineTransition` API with worklet compilation
- Gesture arbitration with scroll views (edge zone priority)
- Platform-specific spring tuning (iOS, Android, web, macOS)
- Reduced motion and accessibility
- Agent observability extensions
- **Milestone:** Custom card-flip transition, full accessibility pass

### Phase 5: Interactive Shared Elements (1 week)

- Pinch-to-dismiss driving shared element reverse transition
- Gesture-driven shared element (finger controls the morph progress)
- Multi-element transitions (multiple shared elements in one navigation)
- **Milestone:** Photo viewer with pinch-to-close that morphs back to grid

## Phasing (2026-07)

The March implementation plan above remains the detailed task breakdown, but
its ordering and week-estimates predate the LLP 0297 substrate and the
current router reality. The operative phasing is:

**P1 — the vertical slice: iOS finger-tracking swipe-back.** One gesture,
one preset, end to end: edge swipe → provisional navigation
(`state.provisional`, both screens live) → per-frame progress driving both
screens' transforms in the Motion domain → velocity-aware commit/cancel with
spring completion. This subsumes the old Phase 1.

**P1 implementation baseline at plan approval (2026-07-11; historical, not
current status).** This replaced the earlier three-item "minimum slice of RFC
0099," which materially understated the work and predated the shipped
substrate. **One-Motion-stack rule:** P1 extends what has shipped; building a
parallel navigation-only motion stack is prohibited.

| Seam | 2026-07-11 baseline | Planned P1 action |
|---|---|---|
| SharedValue slab + restricted worklet runtime | shipped on iOS (LLP 0297 §4.5; ENG-22837 — ENG-22834's gesture-worklet driver demo is macOS-only precedent, not iOS substrate) | reuse |
| LLP 0336 arena native slice (`kernel/src/motion.rs`, `RouterHistory` claim kind) | shipped (2026-07-11) | extend — register the edge/back recognizer as the `RouterHistory` claimant (§9 authority note) |
| Normalized-progress tracking + commit/cancel spring shape | demo only (`MotionDemoModule.swift`) | productionize as the Motion progress-spring driver with velocity inheritance (§3); the demo is precedent to be superseded, not shipped substrate |
| Turn-token commit correlation | shipped for the pager (`ExactPagerMotion`) | reuse for resolution↔commit-batch correlation (§4b) |
| Velocity-inheriting settle spring on progress | missing | build (§3) |
| Readiness publication: `BackDisposition` + presenter readiness + scene lease | missing (the transport channel exists — the ENG-22437/22454 committed-navigation payload lineage) | build (§4a) |
| Rendered-route-set transition retention (dual-scene, generation-fenced, input-inert) | missing | build (§4b) |
| **Ambient back-target scene retention** across committed forward navigations — what makes `retainedSceneAvailable` ever true: the previous route's scene must stay renderable from push-commit until a later gesture | missing — today the outgoing scene unmounts when a push commits on the native twin | build — P1 scope is depth-1 retention for stack pops on the fixture profile; parked scenes are **frozen** (outside the rendered route set, no re-renders reach them; the §4a lease + step-6 provisional render refresh them, so first-frame staleness is bounded by confirmation latency; a post-P1 option — opportunistic parked-scene re-render on committed navigations — is priced by the required shadow-mode data); policy beyond depth-1 (memory pressure, eviction, rehydration) stays deferred per D4, owned by this RFC until a dedicated scene-lifecycle LLP takes it |
| Root-lifetime **reserved transition-progress slot** per root (a SharedValue slot) + the **`DecisionCell` control word** (NOT a slab slot — 0099's control-plane record class: `(rootInstance, transitionGeneration, state)` packed word beside the outcome log, with `confirmedAllow`/`confirmedCancel`/`nacked`/`expired` verdict states) — both allocated by the RUNTIME at root setup and published with the disposition; 0099 M1's registry is append-only and runtime-single-owner, so Motion never allocates (at touch time or otherwise). **Teardown TOMBSTONES, never frees**: the control region follows the value plane's physical-lifetime rules (stable addresses; release only at engine shutdown behind quiescence; re-tenancy watermark-gated), so a CAS paused across root destroy/recreate no-ops on a tombstone | missing | build — the slot rides M1's append-only registry; the control word rides the same registry commit machinery in the control region (0099 §Protocol and Transport decision-records row); per-property values are Motion-local controller state and need no slots. P1 may proceed on the shipped 0297 W2 spike slab under **M1-conformant semantics** (runtime-allocated, append-only, tombstone-at-teardown) as an interim shim, becoming an M1 tenant when M1 lands — the same seed-then-conform pattern as the spring and the transport |
| Arena FFI + host instantiation for the `RouterHistory` claim — `kernel/src/ffi.rs` exposes only `exact_motion_pager_*` today, and `ExactPagerMotion` does not instantiate the arena | missing | build — extend the 0336 FFI surface; receipts wired to diagnostics (§9/§11) |
| Minimal single-claimant `InteractionLease` — the shipped 0336 slice is types + `decide()` + a bounded receipt ring only; **no lease type, checkpoint machinery, or outcome lifecycle exists in `kernel/src/motion.rs`** (0336/0099 own the full §6.1 lifecycle at their M4) | missing | build — P1 needs only the single-claimant grant-at-activation / terminal-release pair that §4a sequences against the `SceneLease`, receipt-correlated. **0336's three checkpoint DUTIES are still discharged in P1, by named router-side mechanisms rather than arena machinery:** activation eligibility = the §4a gate + lease acquisition; commit re-check = the §4 authorization-token re-validation; dispatch enforcement = the §4 commit transaction's synchronous blocker/access re-run. The generalized multi-claimant checkpoint MACHINERY stays with M4 — the duties do not wait for it |
| **Presenter-composited transition channel** (step 4c: final value = authored ∘ transition, re-composed on every presenter apply) | missing — 0336 records even the pager's version as defective and open (ENG-24306: dedicated Motion wrapper view/layer preferred; a composed matrix needs one composer with a specified multiplication order) | build/extend — reuse ENG-24306's wrapper/composer shape for scene roots: ONE composer, one multiplication order, both consumers (pager + navigation), per the One-Motion-stack rule |
| Executable reference model of §4a/§4b with property-tested event traces (start → race → release → late ack → HMR interleavings), **including the DecisionCell interleavings** (confirm-vs-expire with BOTH confirmed verdicts, forced-cancel-wake vs held commit, late-CAS both directions, reset-during-pending, confirm-then-reset-before-commit, transition-generation ABA, CAS-paused-across-root-destroy/recreate — the 0099 M2 composed-Loom suite's 0100 half), a **checkpoint matrix** (every §4a disposition field × activation / commit-revalidation / dispatch-enforcement / structural-vs-policy classification / lease pinning — so no field's coverage is implicit; `dataUsableUntil` is the worked example, deadline-crossing tested), and a **gate mutation-test**: the harness must demonstrably FAIL a one-frame-delay variant, a wrong reverse-fling rule, and a raw one-shot layer write wiped by a live update | missing | build — the state machine is too big for prose review alone; the traces double as conformance fixtures, and a gate that cannot fail defective variants is not a gate |
| Native-twin capability promotions: `transitions`, `provisional-navigation`, `blockers` → true | all declared unsupported (`packages/exact-router/src/router-capabilities.ts`) | build — flipping these flags (with their snapshot assertions) is part of P1 completion, not an afterthought; the `blockers` flip includes a **minimal native blocker presentation surface** (CT-061-08's confirmation dialog), which does not exist natively today. **Post-flip, non-iOS native hosts (macOS/Windows) report `transitions: true` with no Motion lane feeding them** — the capability means "the runtime tracks transition state when a lane drives it," stated where the promotion registers |
| **Native `registerBackHandler` registration + `handlerDisposition` publication** — the failure suite's blocker+handler row needs a handler to exist natively, and the shipped native twin has no `registerBackHandler` | missing | build — registration surface + cascade publication only (handlers stay outcome (a): the discrete pop runs them); full handler semantics stay with RFC 0061's owners |
| Agent surface additions (§11) | v2 endpoint exists; interactive fields missing | build, registered through RFC 0052 |
| Callback-affinity rows for every new/modified callback | required by LLP 0297 standing rules | write (`docs/callback-affinity.md`) — unclassified callbacks block merge |

**As-built landing state (2026-07-13).** Every P1 implementation row above has
landed. The registered automated checks pass, but implementation completion is
not acceptance completion: the physical-device gate, retained P1 stack-only
shadow-readiness artifact, and P1.5 tab-pop cohort artifact remain open.

Milestone alignment (the One-Motion-stack rule, enforced against RFC
0099's plan as well as the 0336 slice): the progress-spring driver seeds
0099 **M3** (a pre-M3 form, conformed to M3's simulation contract —
analytic solver, timebase/gap semantics — when M3 lands); the
root-lifetime reserved slot rides 0099 **M1**'s append-only registry;
the arena FFI + single-claimant lease seed 0336/0099 **M4**; the
start/ack/nack + resolution transport is the ENG-22437/22454
committed-navigation lineage as an 0099 **M2 seed** — conformed to M2's
lifecycle-record model (start on the outcome log; ack/nack as wakes
over the decision cell) when M2 lands, the same seed-then-conform
pattern as the spring — and the seed's record shapes are
**digest-stamped from day one** with 0099 M2's schema-digest/
`layoutVersion` inputs, so the later conformance is mechanical and the
mixed-version startup matrix gets its first real fixture for free. Two explicit re-gate triggers: **the P1 velocity estimator
(UIKit `velocity(in:)`) is a P1 expedient** — when 0099 M4's estimator
replaces it, the fidelity gate RERUNS (a fitted-parameter input
changed); and **P4's worklet-backed custom presets additionally require
0099 M6's typed ABI/output-slot lowering**, not just M2's descriptor
schema. One stack, one owner per seam — neither document builds a
duplicate of the other's deliverable.

**Threading precondition.** The architecture under test is LLP 0297's
two-domain split, but LLP 0322 leaves the runtime thread **opt-in on
iOS** (ENG-23520 gate open). Authoritative P1 runs therefore set
`EXACT_RUNTIME_THREAD=1` and **record positive executor provenance** (the
dedicated-runtime executor active) in the run artifacts; a
main-thread-mode run is an informative control only. P1 evidence is
collected ahead of the platform default flip, not gated on it — and a P1
that never left the main-thread mode would not have tested the bet at
all.

<a id="p1-the-fidelity-experiment"></a>

P1 is explicitly the **experiment that can falsify the bet** this RFC — and
the router's native strategy — rests on: that gesture-driven transitions
routed through Exact's kernel can match native feel.

**The P1 gate is operational, not impressionistic.** The comparison
fixture is a two-screen stack app (list → detail, realistic content
density) built twice: once on Exact, once as a stock
`UINavigationController` baseline. Both run as **Release builds**. The
acceptance authority is a run on a physical ProMotion iPhone (120Hz,
iPhone 15 Pro class or newer, current major iOS); a simulator run is the
automatable proxy. The fixture spec is **pinned** so the gate cannot be
weakened by fixture choice: the list screen renders ≥50 rows (thumbnail
image + two text runs each), the detail screen a hero image + ≥6 text
paragraphs; ≥400 rendered nodes per screen; both screens data-warm. A
second fixture variant runs the same screens with **live updating
content** (a ticking clock row + a streaming image swap) for the
composited-channel and live-update failure rows.

**Input is reproducible — via a bench build configuration, not the DEBUG
agent stack.** The shipped `exact_touch_sequence` injector is compiled
out of non-DEBUG builds (LLP 0030's App Store exclusion) and resolves
only Exact-managed windows, so it cannot drive the stock fixture or a
Release build as-is. P1 therefore defines an internal **bench
configuration**: release optimization settings with the same LLP 0030
`UIWindow.sendEvent` synthesis harness compiled into **both** fixture
apps (never App-Store-shipped), so identical scripted traces enter at
the same layer on both sides. (Known risk, owned in P1: if
`UIWindow.sendEvent` injection proves unable to arm the *system* edge
recognizer faithfully, the fallback is an IOHID-level or XCUITest replay
lane driving both apps — the parity requirement is the invariant, not
the mechanism.) Runs record build settings, synthesis
mode, and the **realized** per-event timestamps and release velocities —
nominal scripted delays (`Task.sleep` cadence) are not evidence; the
recognizer-visible timestamps are. A free-hand pass on hardware is the
sanity check. Runs specify repetitions and warm-up (≥10 gestures per
velocity bucket after 3 discarded warm-up gestures, across at least
slow / mid / fling / reverse-fling buckets, with near-threshold and
near-midpoint cases included) on a thermally settled device.

**Instrumentation is per-metric; video is the visual-artifact oracle,
never the sole timing instrument** (screen recording cannot resolve 8.3ms
deltas, cannot see touch timestamps, and conflates encoder duplication
with app frame drops). The Exact side logs recognizer sample timestamps
(`UITouch.timestamp`), display-link and frame-presentation timestamps,
and the applied transform per frame via in-process signposts
(os_signpost / CADisplayLink logs; Instruments hitch analysis on device).
The **same in-process monotonic sampler is compiled into both bench
fixtures** — injected-event timestamps, display-link ticks,
presentation-layer transform/opacity per tick, signposts — so every
numeric gate reads identical instrumentation on both sides;
presentation-timestamp inference is validated once against a high-speed
camera, Instruments corroborates hitches, and video remains the
visual-artifact oracle only. **The P1 velocity estimator is
the platform recognizer's `velocity(in:)`** — the hybrid boundary's
natural, stock-matched source; outcome parity depends on estimator
parity as much as threshold parity, so runs record which estimator
produced each release velocity. **Definition — dropped frame:** a
missed vsync deadline at the active refresh rate (display-link delta
> 1.5× the nominal interval). **Realized-cadence requirement** (the
gate names a 120Hz device, so the budget must not quietly loosen to
whatever rate ProMotion happened to choose): authoritative traces must
record their realized display-link cadence, a trace spending more than
10% of tracking below 110Hz median is not authoritative (rerun it),
and both fixtures must exhibit the same cadence regime on compared
traces — a 60/80Hz run passing a looser frame budget is a fixture
artifact, not evidence. Numeric gates, measured on both apps and
compared:

- **Touch-to-transform latency:** recognizer sample timestamp → first
  presentation timestamp carrying the resulting transform; Exact within
  one frame (8.3ms at 120Hz) of the stock baseline's measured latency.
- **Frame time during tracking:** p95 within frame budget; **zero dropped
  frames at p95 across the run set; at most one dropped frame per
  5-second trace at p99**. Sampling unit: one 5-second tracking trace per
  gesture; quantiles are computed across the full run set (≥40 traces),
  never within a single trace.
- **Tracking error:** max |applied transform − trace finger position| per
  frame, no worse than the stock trace's own error envelope on the same
  replayed trace.
- **First-frame readiness:** recognizer activation to first moved
  presentation within one frame of stock.
- **Settle fidelity:** spring settle duration and overshoot within 10% of
  stock for matched (replayed) release velocities; velocity continuity
  verified as no dp/dt step at spring start beyond the trace noise floor
  — structural under §3's single progress spring, measured anyway.
- **Cancel correctness:** snapback with zero visual artifacts
  (CT-061-12); commit completes cleanly (CT-061-13); mid-flight reversal
  tracks the finger.
- **Outcome parity:** across a dense grid of release progress × signed
  velocity — concentrated near the velocity threshold, the midpoint, and
  the reverse-fling boundary, plus mid-flight reversals — replayed
  traces resolve commit/cancel **identically to the stock baseline**,
  with a **boundary tolerance**: strict identity is required only
  *outside* an ε-band around the fitted decision boundary (ε in velocity
  and progress, recorded with the constant fit); *inside* the band —
  where stock's unpublished rule may differ in functional form from
  Principle 3's two-parameter shape no matter how well constants are
  fitted — runs must be deterministic and **direction-consistent**
  (monotone in signed velocity and in progress: moving either input
  commit-ward never flips a commit to a cancel), and
  disagreements are **reported with the fit**, not failed on. The gate
  must be passable by a correct implementation and failable by a wrong
  rule; without the band it is unpassable by construction, and an
  unpassable gate gets waived by judgment — the exact failure mode this
  section exists to prevent. **Caps that keep the band honest:** ε is
  derived from repeated-trace measurement noise *before* constant
  fitting — where "noise" explicitly includes **stock's own
  near-boundary outcome flips on identical replayed traces** (oracle
  rule-inconsistency is direct evidence of the band's necessary width,
  not just timing jitter) — and capped (≤5% of the fitted velocity
  threshold; ≤0.02
  progress); the band may exclude at most 10% of the grid; constants fit
  on a calibration set, with strict parity evaluated on a **held-out**
  matrix outside the locked band; exceeding any cap fails the gate; and
  the band's width is itself a reported regression metric across
  re-runs — a growing band is a drifting rule. A fixture that tracks
  beautifully but commits when stock cancels *outside* the band fails
  here.
- **Runtime-thread pressure:** with app JS deliberately busy (a synthetic
  long task pinned to the runtime thread) **and** a valid disposition +
  leased scene, the gesture MUST begin within the latency gate and track
  at full fidelity — **a silent fallback to the discrete pop in this
  configuration fails the gate** (§4c: a stalled runtime thread may delay
  logical confirmation and degrade observability; it must never degrade
  tracking). The discrete fallback is legitimate only when the
  disposition or scene is genuinely unavailable (§4a publish-gap and D4
  non-retained rows — tested separately). This run also exercises
  release-before-ack: the visual settle must proceed and the logical
  commit apply on the late ack (§4b `settled-visual`).

The preset constants (velocity threshold 500 pt/s, parallax 0.3, shadow
0.15, dim 0.1, spring damping/stiffness) are **starting parameters to be
fitted to the stock traces during P1** — the gate is the comparison, not
the constants. Fitted values land in the shipped constants home —
`kernel/src/motion.rs`, where the LLP 0336 slice already carries
`COMMIT_VELOCITY_PER_MS = 0.5` (the same 500 pt/s this RFC started from)
— so both documents cite one authority rather than drifting. Constants
are shared per interaction class; **rules are not**: the pager's
magnitude-based resolution (`dx.abs() > COMMIT_DISTANCE ||
velocity.abs() > threshold` — the magnitude-OR shape is right for a pager, where both directions are commits; its direction/velocity COMPOSITION is separately defective and under repair per 0336's status row — a reversal trace commits toward displacement, ENG-24305/24312) deliberately differs from this RFC's direction-aware rule,
and colocating constants must not tempt a rule unification. Evidence accelerators:
**fit-by-optimization** (optional, recorded if used — least-squares fit
of spring/parallax constants against the stock trace's per-frame
transforms, making "fitted to stock" mechanical rather than
judgment-based), and the **shadow-mode readiness run — REQUIRED for P1.5
acceptance and still outstanding**: run §4a publication, lease acquisition,
and would-be resolutions in shadow in dogfood builds,
yielding real-world distributions (retention hit rate,
revision-mismatch and publish-gap rates, confirmation-latency
percentiles) that price P1.5's priority and D4's post-P1 `external`
relaxation on data instead of instinct. **Cohort honesty on the tab-pop
share:** the retained evidence set has two named parts: the P1 physical gate's
stack-only shadow-readiness artifact and P1.5's additional tab-pop cohort
artifact. They may be bundled together for archival convenience, but neither
cohort substitutes for the other and implementation landing substitutes for
neither measured distribution. In particular, the original native dogfood
cohort was stack-only, so tab-pop share cannot be inferred from it; the second
artifact must come from P1.5 dogfood or another qualifying tab-enabled cohort
(with web tab configurations useful only as additional context).

**Failure-mode suite** (must pass alongside the happy path): blocker
appears after the readiness snapshot (ordinary eligibility flip —
tracking continues, release resolves cancel, blocker UI after settle,
§4a); route change or new navigation during an active gesture (§4b
racing-navigation row: structural invalidation — immediate
cancel-with-snapback, then the navigation applies); **release before
router confirmation** — the ack-after-visual-settle path (held promotion
applies), the nack-after-visual-settle path (jump-to-truth reconcile,
§4a/§4b `settled-visual`), and the **never-ack** path (runtime wedged
past the bounded confirmation timeout → treated as nacked, tombstone
honored for the late messages that eventually arrive); duplicate or late
resolution delivery (the §4 queue-order and tombstone rules: start
notification before same-id resolution; nacked/unknown ids are no-ops);
runtime reset (HMR) during settling (§4b interruption rules); previous
scene not retained (degrade to discrete, D4); unusable data disposition
— expired cache, auth change, whole-loader failure with no usable
cache, blocking-external (degrade to discrete, D4/§4a) — AND its
stale-eligible complement: a failed revalidation or rejected deferred
field WITH an unexpired cached value classifies `stale` and **must NOT
degrade** (RFC 0053's render-cached-while-revalidating rule; both
directions tested); blocked attempt past the intent threshold
(blocker UI surfaces exactly once, §7 / CT-061-08/14); **the
at-root-modal edge swipe** (`sheet-dismiss` disposition, outcome
(b-passive)) — no blocker UI, no dead tracking, content receives the
touches; **the parked-scene update** — mutate the back target while
parked, then gesture: the step-6 provisional render must refresh it,
with first-reveal staleness bounded against stock on the same trace,
runtime pressure included; **accessibility rows** — AX focus follows
the §12/accessibility rules (focus preserved on cancel; the input-inert
retained scene excluded from content touches and the AX tree's
interactive elements); reduced-motion
enabled (§12 rules 2–3, including composition with `settled-visual`);
**the benign policy bump mid-gesture** — a dataDisposition recompute
bumps policyRevision while tracking; the MATCH re-run authorizes
cleanly and the commit PROCEEDS (no forced cancel — the doctrine row);
**own-publication plus external blocker** — the gesture's own cascade
re-publication (no policyRevision bump) coincides with an external
blocker registration (policyRevision bump): the external change is
caught, forced-cancel applies; **lost-ack-then-token** — the ack wake
is dropped, the commit token arrives via the committed-navigation
payload before the timeout: the scheduled cell load at
`settled-visual` entry reads `confirmedAllow` and the held promotion
applies without the wake;
a second edge swipe beginning during settling and during
`settled-visual` (recognizer fails until `settled`; the first
transition is unaffected — §4b); **live content updating on both scenes
during tracking and settling** — updates render AND transition values
survive every presenter apply (the step-4c composited channel; LLP 0336
r9's ENG-24306 wipe class); **the tab-pop path and fallback** — a ready
per-tab fixture commits through the interactive tab-aware transaction, while
readiness/publish/lease failure commits through the discrete fallback, with
the corresponding arena receipts asserted; **blocker + custom handler registered
together** — the blocker wins, matching the shipped `back()` order
(needs P1's native `registerBackHandler` registration row); **the
wedged-runtime `DecisionCell` trace** — runtime wedged past the
timeout with start + resolution queued → main expires the cell, the
queue drains, the CAS fails, and no logical pop occurs — plus the
**generation-ABA trace** (a late confirm for transition N fails
against the cell re-armed for N+1) and the **post-confirm-wedge
trace** (runtime confirmed then wedged: expiry CAS fails, the held
entry stays inert, the runtime-stall watchdog owns recovery — §4a);
the §9 Apple
arbitration test list (scroll coexistence, arena receipts asserted).

Verification housekeeping: runs publish two artifacts — a
**correctness report** (state-machine traces, outcome parity, failure
suite) and a **fidelity report** (latency/frame/settle metrics) — with
fitted constants, locked ε, build settings, executor provenance, and
raw traces bundled reproducibly. The automatable portion (simulator
traces, frame-extraction analysis, failure-mode suite) registers in
`exact-verify.json`, and the registered check **asserts the presence
of the retained report artifacts** (correctness + fidelity bundles) —
a run without its artifacts fails the check, the same
aspirational-protocol closure 0099 M3 registered; the device run is
**human-gated evidence** — video
and trace artifacts attached to the implementation issue. If the bar is
missed, the fallback posture is RFC 0061's v1-core discrete edge swipe
(commit a standard pop) plus platform-delegated transitions; **whether to
retreat, retune, or re-architect is the author's decision**, made on the
recorded measurements rather than on faith.

**P1.5 — tab-inner stack pops (implementation landed; acceptance inherits the
open P1 gate).** The tab-aware commit transaction, `tabs` capability,
tab-scoped scene identity, and per-tab fixtures have landed. `tab-pop` now uses
the interactive lane when readiness succeeds and retains the discrete path as
its fallback. P1.5 is not acceptance-complete because its required tab-pop
cohort artifact is absent; the P1 stack-only shadow-readiness artifact and P1
physical-device evidence also remain absent. Until a dedicated registry row
lands, ENG-24719 is the explicit human-gated owner for the tab-pop cohort
artifact; `motion-navigation-fidelity-physical` continues to validate only the
P1 stack-only artifact and is not claimed as P1.5 evidence.

**P2 — sheets and modal dismiss** (old Phase 2): M8 now supplies the shared
dismiss-layer and claim substrate for component-local layers. P2 still owns
sheet geometry, detents, the §4a router `dismissDisposition` publication,
router-owned native layer-stack integration, and native Presence/focus
restoration acceptance; component-local M8 does not silently satisfy those
router-owned obligations.
**P3 — shared elements** (old Phase 3, gated by D1's
measurement clause) — additionally requires frame measurement across
trees and presenter overlay retention, plus D1's pre-P3 nonvisual
checklist and content-class ladder. **P4 — custom transitions and polish**
(old Phase 4): the M2 descriptor schema and M6 typed worklet ABI have landed.
P4 remains gated by P1 and by the custom-transition work itself. The substrate
note's §4.4 compositor-set extension (corner radius, shadow/overlay opacity,
scale) registers in the M2 property enum as its durable home. **P5 —
interactive shared elements** (old Phase 5). P2–P5 do not start until P1
has passed its gate.

## Design Decisions (Resolved)

The five questions this RFC left open in March are resolved as of
2026-07-05. Residual work inside each decision is noted where it exists;
none of it reopens the decision. **Numbering crosswalk:** D1–D5 preserve
the March open-question order (OQ1 → D1 … OQ5 → D5), so external
citations of "RFC 0100 OQn" (e.g., LLP 0310's references to OQ1, OQ4,
and OQ5) resolve to the corresponding Dn below.

**D1 — Shared-element snapshot strategy: view-subtree clone first,
rasterization as a measured upgrade.** The **presenter** clones the
source view subtree for the floating transition element, in presenter
layer space (§5 — under LLP 0297 the Motion domain never clones or
mutates the kernel tree; this decision amends RFC 0046 Appendix A's
raster-snapshot default, per the amendments block). Clone-first has the
lower fidelity ceiling but lower memory and no texture lifecycle to
manage. Not every native subtree is cloneable with one rule: P3 defines a
**content-class ladder** — presenter re-projection/clone where safe
(static view content), raster snapshot where required (platform
controls, video, web content), cross-fade fallback otherwise — decided
per shared-element pair and recorded with the pre-P3 checklist below. Metal-texture rasterization is an
upgrade path to be taken only if P3 measurement shows clone fidelity is
insufficient for real content (text reflow during size interpolation is the
expected first offender). This is a decision with a fallback, not a research
question: the controller's interface (`SharedElementController` over
source/target frames) is identical under either strategy, so the choice is
swappable behind it. Web keeps the View Transitions API. Before P3 starts,
the clone strategy must also specify the nonvisual semantics (either
strategy needs them): event suppression on the clone, accessibility hiding
(the clone is decorative — source/target own the AX tree), focus behavior
across the swap, image decode state (the clone must not flash undecoded
content), text reflow during size interpolation, and z-order across
overlays.

**D2 — Nested stacks: the inner stack pops first, and arbitration follows
the back-policy cascade.** Per RFC 0061 §5 (levels 3 and 4 of the cascade):
a back gesture inside a modal that contains a navigation stack pops that
stack; the modal can only dismiss once its inner stack is at root. The
gesture-arbitration rule, concretely: **an edge swipe targets the deepest
(innermost) stack whose topmost route can pop; the modal/sheet dismiss
gesture (vertical drag) claims the gesture only when no inner stack can
pop.** The two gestures are axis-disjoint (horizontal edge swipe vs.
vertical drag), so arbitration is by axis and edge zone first, cascade level
second; the existing `GestureArbitrator` rules in §9 need no new mechanism,
only the cascade lookup at activation time. `backAction.level` (RFC
0061/0052) must report the same answer the arbitrator would give, so this is
agent-testable (CT-061-09/10 cover the discrete case; P1 extends them to the
gesture-driven case).

*As-built note (2026-07-13):* the deepest-stack rule applies to tab-inner
stacks. P1.5's tab-aware transaction now admits `tab-pop` to the interactive
lane when readiness succeeds; the discrete behavior remains its failure and
readiness fallback. The cascade's answer about *what back does* is unchanged.

**D3 — Cross-window transitions on macOS: descoped to a future revision.**
No cross-window shared-element transitions in v1 or v2 of this system.
Rationale: the transition controller and both live trees are scoped to one
window's presenter; a cross-window morph requires an overlay window or
window-server-level compositing, both of which fight AppKit window
management for marginal payoff. Cross-window navigation gets the
non-interactive treatment (the destination window opens/focuses per RFC
0048; `startInteractiveNavigation()` already routes managed-window targets
through the target window's own transition state, per RFC 0046's 2026-06-25
sync). Revisit only with a concrete product pull.

**D4 — Prefetch during gesture: configurable per-route; conservative
default.** Default: the gesture renders the provisional screen from the
retained scene plus cached data (back targets are usually warm — they
were just visited), and loader work runs as **post-commit SWR
revalidation** of the already-rendered scene. Squared with RFC 0010's
canonical lifecycle explicitly: 0010 says "LOAD blocks COMMIT" in
general, and for interactive gestures "LOAD may or may not have started";
on the P1 retained-scene path COMMIT is **not** blocked on LOAD because
the retained scene guarantees renderable content — the loader pass that
follows commit is revalidation, not first load. The fast path is
**data-gated, not scene-gated alone** (§4a `dataDisposition`, RFC 0053's
pop semantics): `fresh` data serves as-is; `stale` within `gcTime`
serves and SWR-revalidates after commit; **expired, missing,
access-changed, blocking-external, or whole-loader-failed-with-no-cache
data disqualifies the interactive
path** — in P1 the gesture degrades to the discrete pop rather than
committing a screen RFC 0053 would not let render from cache. The
boundary is deliberate in BOTH directions: a failed revalidation or a
rejected deferred field with an unexpired cached value is `stale`, not
`unusable` — RFC 0053 requires the cached value to render while
revalidation runs, so background-work failure alone never degrades the
gesture. P1's
failure suite covers the expired-cache, auth-change,
whole-loader-failure, and external-data rows AND the stale-eligible
complement (failed revalidation with unexpired cache stays
interactive). (The `dataStrategy: 'external'` exclusion may relax
post-P1 — a retained scene owns its component's live data, so "retained
scene present ⇒ eligible" is plausible once observed in practice; the
current conservatism is deliberate, not derived.) (The general LOAD-before-COMMIT rule stands for
*constructed* provisional routes — explicitly deferred past P1 below —
which will need a post-release LOAD-before-logical-COMMIT state with
defined slow/error behavior when they land.) Routes can opt into optimistic loading
(`prefetch: 'gesture-start'`) where loaders fire when the provisional
state is created, accepting wasted work on cancel; `prefetch` registers
in the canonical ScreenOptions contract (RFC 0061/0010 surface) when
implemented. This is consistent with RFC 0046's "provisional route's
loaders MAY run" and RFC 0061's stretch contract; this decision pins the
default. Cancelled optimistic loads must be abortable through the same
staleness machinery as any superseded navigation.

P1 additionally pins the **retained-scene rule**, which is separate from
data loading: interactive preview requires the back target's scene to be
retained and synchronously renderable at gesture start
(`retainedSceneAvailable`, §4a). If it is not — evicted under memory
pressure, or never rendered — the gesture degrades to RFC 0061's discrete
pop. On-demand provisional route construction, loader start at
gesture-begin, and evicted-scene rehydration are explicitly deferred past
P1.

**D5 — System-gesture exclusivity: defer by construction, not
negotiation.** The kernel never contends with system gestures it cannot
win. On Apple platforms this falls out of the v1 hybrid boundary already
specified in §2: the host bridge sources input from platform recognizers
(`UIScreenEdgePanGestureRecognizer` et al.), which lose to system gestures
(home indicator, notification/control center) under UIKit's own arbitration
— exactly the deference we want, for free. ENG-22454 validated the channel
shape: host-recognized gestures reach navigation through the shared host
gesture channel with zero per-app JS. The Motion domain treats a
system-gesture takeover as gesture failure (recognizer → `Failed`, no
provisional state, or cancel-with-snapback if already active). Android's
system back gesture (both edges, predictive back) is a platform contract,
not a preset tweak — it is owned by LLP 0310 (Android navigation), which
maps the RFC 0061 back-policy cascade onto the modern predictive-back
callbacks (`OnBackInvokedCallback` / `OnBackAnimationCallback`, per
0310's own naming) and their progress API; this RFC's controller consumes
that progress stream like any other gesture source (Tier 2 — whether
Tier 1 host-driven previews migrate to it is LLP 0310's open question 1).

## Open Questions

None remain open at the design level — the five March questions are
resolved in the Design Decisions log above (D1–D5), and the review round
of 2026-07-05 closed the lifecycle questions it raised: gesture start
reads only the §4a published halves (optimistic start,
cancel-with-snapback on generation mismatch); `state.provisional` means
the logical candidate only, with scene retention presenter-owned per §4b;
a non-retained previous scene degrades to the discrete pop (D4); a
mid-gesture blocker resolves as cancel-with-snapback at release (§4a/§7).
The rounds of 2026-07-11 closed the remainder: release-before-confirmation
resolves as visual-settle-then-reconcile with logical commit held pending
ack, bounded by a confirmation timeout with tombstoned ids (§4a/§4b
`settled-visual`); a navigation racing an active gesture is structural
invalidation under LLP 0336 — immediate cancel-with-snapback, then the
navigation applies (§4b); commit/cancel is one direction-aware rule
stated once, scoped to Exact-owned recognizers, with externally resolved
sources (Android predictive back) keeping their system outcome (Design
Principle 3, D5); gesture contention is LLP 0336 §6.1's arena with this
RFC's edge swipe as the `RouterHistory` claimant; and the deltas against
RFC 0010 D6, RFC 0046, and RFC 0052 are **registered in those documents**
per 0010's Normative Home Rule (see the amendments block), not merely
declared here.

What remains is measurement-contingent, not undecided: D1's
clone-fidelity clause is evaluated in P3, the preset constants are fitted
to stock traces in P1, and P1's fidelity gate (see Phasing) decides
whether the architecture proceeds past the vertical slice.
Android-specific gesture semantics are owned by LLP 0310, including its
open question of whether Tier 1 host-driven previews migrate to the
Motion gesture-driver path once Motion exists (LLP 0310 OQ1 — needs a
working Motion prototype to answer; this RFC only requires forward
compatibility of the event mapping).

## Alternatives Considered

### Delegate to native navigation controllers entirely

Let `UINavigationController` (iOS) and `FragmentManager` (Android) own all navigation transitions. This gives perfect platform fidelity but means Exact cannot customize transitions, cannot do shared element animations cross-platform, and loses the ability to use the same navigation primitives on web. The router becomes a thin wrapper with little value.

### Use the View Transitions API on all platforms

The web's View Transitions API (snapshot-based crossfade) could theoretically work on native via a polyfill. But snapshot-based transitions lose the "both screens are live" property. The incoming screen is frozen during the transition. This is fine for simple cross-fades but insufficient for interactive gestures where the user expects both screens to respond to input.

### Keep transitions non-interactive (animation-only)

Only animate between committed states — no gesture tracking, no provisional routes, no cancel. This is dramatically simpler but forfeits the most impactful UX capability. A swipe-back that doesn't follow the finger is worse than no swipe-back at all.

### Snapshot the outgoing screen, render only the incoming screen live

Take a raster snapshot of the outgoing screen and animate the snapshot while rendering only the incoming screen as a live React tree. This halves the rendering cost but means the outgoing screen is frozen — any real-time content (clocks, animations, video) freezes during the transition. More importantly, on cancel, the snapshot must be replaced by the live tree, causing a visible "pop" as the frozen image is swapped for live content.
