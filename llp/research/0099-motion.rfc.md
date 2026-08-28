# RFC 0099: Motion

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Motion, Kernel, Protocol, Router, Contract, Facet, Accessibility, Tooling, Layout
**Author:** Charlie Cheever / Claude
**Date:** 2026-03-28
**Revised:** 2026-08-25 (registers the second consumer of the
registered-sampler class, discharging the GPU-sampler-class registration
this RFC owed per RFC 0115 §3 Tier 1 ratification (b) and Accepted
RFC 0490 §6 (its RFC 0099 row), landed with RFC 0490 M3: the **GPU
uniform sampler** — main-pinned sampling at the generated `gpu` clock
phase under the GPU-uniform binding authority
`tests/gpu/gpu-uniform-binding-v1.json`, over the same
`exact_shared_value_sample` accessor, admission counters, and retirement
fences as LLP 0313's island change-detector; ratified in lockstep with
LLP 0297 §4.5's r26 fourth-reader amendment. The accessor remains
restricted to the registered-sampler class and is still not a general
read API; no storage or ordering semantics change.)
2026-08-25 (cross-reference correction only: ENG-23520 — the
iOS runtime-thread gate — resolved 2026-07-23 per the LLP 0322 revision
(iOS default-on; all W4b device gates recorded on hardware). The iOS-promotion
lines now state that, with ENG-23516's island producer/clock/host-op remainder
plus iOS acceptance evidence as the still-open gate content. No normative
change; Status unchanged); 2026-08-01 (records the LLP 0420 §7 finding as a new subsection in
§Architecture: the gap is a build-baked, presenter-played timeline *format*
inside the existing three tiers, not a missing tier; the honest first move is a
compiler lowering into today's `Sequence`/`Repeat` drivers, with any
presenter-native timeline artifact gated on a static-choreography
driver-graph overhead measurement that does not yet exist. No tier-model change); 2026-07-27 (r23 — progress-class truth reconciliation from
the LLP 0411 §2.1/OQ5 audit (the
issues/closed/20260727-llp0099-progress-class-claims-diverge-from-source.md
ticket, closed by this revision). Two divergences, opposite remedies:
§Registry's capacity bullet over-claimed "growth is wait-free" — an
erroneous description, corrected to the load-bearing reader-side
property (slot lookup is a bounded wait-free load chain; growth
heap-allocates and CAS-publishes). The plane table's applied-watermarks
row is the opposite case — an unimplemented normative requirement: its
control-word representation with wait-free runtime reads stands as
design, and the row now carries an explicit deployment-status note (the
landed readers take the management mutex or are test-only;
implementation tracked in
issues/20260727-sharedvalue-applied-watermark-control-words.md). The
try_read-class wait-free claims (§SharedValue app-JS read, the 0297
alignment note) were re-audited as correct and are unchanged. r23 also
executes the issues/20260727-llp0099-line1001-wait-free-loose-usage.md
residual filed from the kernel SharedBuffer ticket: the M1-summary
"wait-free reads" parenthetical is qualified to the try_read class
only, keeping its actual point (reads are not callbacks, so no
callback-affinity entries). Status remains Review.)
2026-07-20 (r22 — adds §Velocity Tracking's "Sample-stream
fidelity and input-cadence robustness" guardrails: raw per-axis
(position, timestamp) streams are never collapsed to derived scalars
before their consumers, and estimators/thresholds must be
timestamp-based and validated across input event cadences — a distinct
axis from the simulation contract's display-refresh invariance.
Externally corroborated by the Inertia-1 wearable-motion
foundation-model study (full triaxial time-domain input beats collapsed
magnitude summaries; rate robustness as a design axis). Corpus-side
counterpart added to LLP 0336 §Acceptance. Status remains Review.)
2026-07-15 (r21 — closure truth reconciliation after the
ENG-24960–ENG-24978 remediation pass. M2, M6, and M8 are implementation- and
registered-acceptance-complete on the macOS/web v1 paths. M4's native arena and
pager-policy absorption, web wheel/RouterHistory/profile publication, and
token-bearing pager re-center acknowledgement have landed with shared
Rust/web and Apple-host acceptance coverage; only the manual named-browser
edge evidence remains open for M4 acceptance. Status remains Review because
that browser evidence and RFC 0100's physical ProMotion, P1 stack-only shadow,
and P1.5 tab-pop cohort evidence are absent. Android/Windows acceptance and the
iOS runtime-thread promotion remain explicitly deferred, not macOS/web v1
blockers.)
2026-07-14 (r20 — ENG-25009: recorded the v1 animated-style
exposure decision in §Animated Style Bindings — transform/opacity direct
sinks are the shipped authoring contract; `WebAnimatedStyle` is
renderer-private; `useAnimatedStyle` exposure is deliberate future work.
Prior r19 note follows.)
2026-07-13 (r19 — as-built reconciliation after ENG-24695:
substantial M1–M8 slices have landed on the Apple and web paved paths,
including the durable SharedValue registry, graph and command transport,
drivers and per-surface clock, native/web recognition and arbitration,
composition automata, typed worklet ABI, scroll bindings, and
navigation/dismiss/reduced-motion/Acto integrations. This is not an M1–M8
closeout: M4 still lacks the native pager-policy absorption, web
wheel/RouterHistory/profile publication, token-bearing pager re-center
acknowledgement, and named-browser edge evidence listed in its milestone row;
M6 retains the authoring gaps recorded in r15. Status remains Review. RFC
0100's physical ProMotion fidelity evidence and required stack-only
shadow-readiness artifact also remain outstanding; Android and Windows are
outside the v1 acceptance claim.)
2026-07-13 (r18 — M2 retires layout-island registration from
the production path: inventory kind 14 carries one strictly validated
SharedValue dependency plus island-root/closure/property scalars; renderer
registration lowers it into the whole-root sidecar; and the Apple graph
reconciles complete per-island binding sets through the real SharedValue
registry tenant before/after presenter apply. The old `island.register` /
`island.unregister` operations remain deprecated debug compatibility only.
Status unchanged: Review.)
2026-07-13 (r17 — the bounded web-pointer leg of M4 lands:
one document intake and per-root `WebMotionStreamArena` freeze composed-path
topology, arbitrate nested Motion/pager/scroll/browser-system contenders behind
one external lease, re-check activation/commit/dispatch, retain bounded
receipts, join multipointer membership, and cancel on structural/reset/browser
takeover. React `DismissableLayer` now registers with that arena instead of
installing a duplicate controller. This is deliberately not the whole M4 web
claim: wheel still uses the pager-specific intake, no web RouterHistory
publisher feeds the arena, and the named-browser inset-edge evidence remains
open; pager settle still releases at semantic dispatch rather than a
token-bearing re-center acknowledgement. Status unchanged: Review.)
2026-07-13 (r16 — M8 transfers the reduced-motion class authority
to LLP 0074 as planned: one JSON profile now generates TypeScript and Swift
projections, one resolver covers Motion/presenters/web, and
`motion-conformance` rejects drift. This document retains Motion's obligation
to consume that authority, not a duplicate table. The agent-observability
slice also lands typed gesture/driver/retention state, a bounded receipt ring,
tree/dense/diff/diagnostics projection, and event-sourced waits. Status
unchanged: Review.)
2026-07-13 (r15 — ENG-24704 lands the M6 native worklet/runtime
seams without claiming the authoring half complete: the stable content+capture
identity and typed fixed-slot invoke ABI, validated scalar/boolean/SharedValue
captures with stale-handle shadows, bounded generation-fenced `runOnJS` drain,
and runtime-owned rated-publish mailbox/pacer/provider are implemented. The
conditional 0297 amendment is decided in favor of explicit UTF-8
function-expression source install: 64 distinct warm artifacts measured p50
0.081ms / p95 0.182ms against the 5ms mount-time ceiling, and the current
pipeline has no self-contained restricted-worklet HBC + capture artifact to
consume. Nested/inline extraction, capture diagnostics, output-slot authoring
lowering, and bundle-installed app callback registries remain M6 authoring
work. Status unchanged: Review.)
2026-07-13 (r14 — author adjudication after the set loop:
Draft → Review. The third-round Fable verdict found all six documents
ready and the set coherent; the recorded Codex dissent is retained as
implementation and gate work, not hidden or treated as acceptance.
The residual design decisions have explicit owners: ENG-24698 closes
0336 OQ16/OQ17 before M4, M2 retains and bounds the typed commit-output
crossing, and the M1–M8/P1 acceptance gates must close the cited
as-built gaps before this RFC may become Implemented. Review means the
program is approved for implementation; it does not waive any gate.)
2026-07-13 (r13 — the set-loop's closing fix pass after a
THIRD consecutive split round (Fable: all six READY / set COHERENT,
7 minors, no materials; Codex: all six NOT READY — artifacts
`llp/reviews/0099-…-set.{fable,codex}-round3.md`; loop stopped per the
pre-declared asymptote rule, adjudication to the author). Applies both
rounds' verified line-level findings: the sampler publication recipe
gains its writer-side release fence (torn-matched-pair = named M1 Loom
trace); the reset-barrier step (1) says every-armed-cell (not
"pending"); the tombstone bit is defined (the sampler verdict's
mechanism); E3 writes get the explicit E4 catch-up rule; native
rated-publish ownership is assigned (runtime-side pacer/evaluator/
provider; Motion publishes dirty generation + wake; scalar authoring =
the Contract-motion authority, not 0281); the blocked-attempt latch's
UX contract is stated per-burst; the adjudication bullet's stale
"Relaxed" vocabulary and the ledger's (a)/(b) revision attribution are
corrected; table pipes escaped; the 0336 pointer and step-7b identity
vocabulary refreshed. The revision stopped in Draft pending the author
adjudication recorded in r14, with the Codex dissent preserved per the
0041/0336 precedent.)
2026-07-13 (r12 — set-loop round-2 reconciliation, from a
second split round (Fable: 0099/0100 NOT READY narrowly, four READY,
set coherent; Codex: all six NOT READY — artifacts
`llp/reviews/0099-…-set.{fable,codex}-round2.md`; both families
converged on the same fixes). The decision cell becomes a CLOSED state
machine: structural nacks are now a durable `nacked` cell state (the
runtime CASes it — every authorization outcome is in the word, so the
wake gloss is literally true on every path), epoch reset tombstones
EVERY armed state (confirm-then-reset-before-commit resolves to
reconcile), main's progress is wake-independent (scheduled cell loads
at release processing, settled-visual entry, timeout), the atomic
representation is pinned (one AtomicU64, rootInstance:24 |
transitionGeneration:32 | state:8, wrap-escalation not in-place wrap),
the teardown tombstone's writer and carrier are named (MAIN, in the
teardown transaction's sidecar detach phase; the teardown watermark IS
that transaction's applied-sidecar watermark), and the
expiry-vs-confirmedCancel/nacked branches are defined. Blocked-back-
attempts get a record class (per-root single-slot latch on the log,
identity (rootId, attemptSeq), coalesce-while-outstanding,
epoch-fenced, permanent reservation — not an activation credit). The
sampler accessor's memory order is pinned (Release publication /
Acquire retry — 'Relaxed' vocabulary retired from §Reclaim), it
returns a tombstone-capable verdict (0313's detach contract
preserved), and write generations are u32 with wrap-escalation.
E4 gains the consumed-generation provenance rule (records carry
main-captured {slot, lifetimeGen, writeGen, value}; commits echo the
vector; missing driving slot ⇒ freshness FAIL) — the sentence that
makes 0313's ENG-24663 retirement structural. The 0100 ledger entry is
refreshed (applied + upgraded, nothing queued); the outcome-log sizing
note lands. Full history: §Revision history. Status: Draft.)
**Related:** RFC 0007 (Behavior Primitives — defers gesture composition to
a future gesture RFC, i.e. this document; the behavior-primitive
integrations below are proposed amendments to it), RFC 0010 (Router v2 —
owns screen-option spelling), RFC 0041 (Safe Area), RFC 0046 (View
Transitions — owns the no-opt-out reduced-motion navigation fade),
RFC 0074 (Accessibility — owns preference signals and the
accessibility-action schema), RFC 0089 (Contract Animation —
`llp/contract/`; its four motion categories map onto the tiers below),
RFC 0097 (Facet Quality Grind), RFC 0100 (Interactive Navigation
Transitions — Review; bridges this document and RFC 0046 and owns how an
interactive navigation transition moves; Motion supplies its substrate),
LLP 0281 (Contract Transient Reactivity Lane — Implemented, stage 0;
draws the §1 seam with this document and answers OQ6; its stage-0 JS/web
channel semantics are normative for this document's native channels;
stage 1 was declined per its §3.1 gate), LLP 0297 (threading model —
Implemented; owns the execution substrate: two-domain kernel residency
§4.4, UI worklet runtime §4.3/§4.6, SharedValue storage §4.5, generation
fencing §4.8; its OQ2 — slot lifecycle — is received by this document's
registry section), LLP 0313 (same-frame layout islands — Implemented on
the Apple paved path; M2's inventory-owned binding table and true write
generations supersede the interim host-op/change-counting path; owns the OQ2
answer here / 0297 OQ4), LLP 0322 (runtime thread default-on for
macOS 2026-07-07 and iOS 2026-07-23; Windows opt-in pending its
gate), LLP 0336 (Cross-Surface
Interaction Model — Review; P1 web recognizer and P2 native Motion slice
implemented 2026-07-11 as landed slices — see the implementation-status and
historical-baseline sections for the acceptance gaps this document tracks;
its §6 is this document's
minimum v1 slice, §6.1 is the normative v1 arbitration arena contract the
composition API compiles onto, and its P3 (ENG-24183) is the absorption
of that slice by this document's full substrate)

## Summary

Define Motion, Exact's internal subsystem for gesture recognition,
SharedValues, continuous animation, and animated bindings. Motion is a
**main-owned Rust domain** (LLP 0297 §4.4) plus the persistent UI worklet
runtime (LLP 0297 §4.3): gesture recognition and arbitration run in
main-side Rust fed by thin platform adapters, spring/timing/decay
interpolation runs in main-side Rust on the display link, animated style
worklets run in the restricted Hermes runtime on main, and app JS is never
in the hot path for touch tracking or frame-driven motion. Compositor-path
properties (transform, opacity) apply to the presenter in the same frame;
layout-affecting values route through LLP 0297's constraint buffer or,
where eligible, LLP 0313's bounded same-frame layout islands. On web, the
same API runs on a main-thread TypeScript implementation (see §Web) — the
main thread is the UI thread there, and the shipped recognizer is
permanent. This completes Exact's three-tier animation architecture by
building the general Tier 2 (Motion) on the substrate that has shipped
since March: the SharedValue slab, the UI worklet runtime, layout islands,
the permanent web recognizer, and the minimum native Motion slice
(LLP 0336 §6). The implementation status section below distinguishes landed
code from the remaining acceptance evidence and platform-coverage work.

## Motivation

### The gap in Exact today

Exact has a real Tier 1 transition system: declarative `style.transition`
that maps to renderer-native animation (Core Animation on Apple platforms,
CSS transitions on web; Android and Windows do not yet apply transitions —
see "Implementation status"). Tier 1 handles state-driven fades, scales, and
color changes well, including spring-kind transitions
(`TransitionKind::Spring`, `kernel/src/transition.rs`).

The examples below were the continuous, input-driven motion gap this RFC set
out to close. As of 2026-07-13, the general subsystem exists on the Apple and
web paved paths; the remaining question is whether the native path satisfies
the physical fidelity gate and broader host-coverage requirements:

- A user drags a card and it follows their finger at 120fps
- A swipe-back gesture reveals the previous screen, interruptible at any point
- Pull-to-refresh stretches with rubber-band physics
- A bottom sheet snaps to detents based on velocity
- Pinch-to-zoom on a photo with momentum
- A parallax header that tracks scroll position
- A dismissable modal that follows a downward swipe and either commits or springs back

These interactions define the quality gap between "works" and "feels
native." Every one of them requires per-frame input processing, physics
simulation, and style updates — none of which can run through app JS
without dropping frames.

Those slices now compose through one general subsystem rather than remaining
pager- and proof-specific mechanisms.

### Why this matters architecturally

React Native solves this with Reanimated + Gesture Handler — two large,
complex libraries that work around the framework's architecture rather
than with it. They use a secondary JS runtime (worklets on the UI thread),
a shared-value bridge, and custom native gesture recognizer composition.
The result works but is fragile: shared-value threading bugs, gesture
handler version conflicts, and an impedance mismatch between the "React
world" and the "Reanimated world."

Exact's architecture is positioned to do this correctly, and the pieces
Motion needs are already in place on the main thread and in shared memory:

- The main thread owns the presenters and can write compositor-path
  properties same-frame (LLP 0297 §4.4 latency classes)
- SharedValue storage exists as kernel-owned shared memory with per-slot
  atomics (LLP 0297 §4.5; `kernel/src/ffi.rs` `SharedValueSlab`)
- A persistent restricted worklet runtime is resident on main
  (LLP 0297 §4.3; measured invoke p95 1.07µs)
- Gesture hit-testing has a defined geometry source: the main-side
  presenter snapshot — never the kernel tree (LLP 0297 §4.4)
- The typed arbitration seam has a shipped seed (`kernel/src/motion.rs`)
  and a normative contract (LLP 0336 §6.1)
- The binary protocol has a working precedent for Motion identities: the
  generation-fenced pager binding descriptor and its semantic commit
  event, allocated inventory-first (LLP 0336 §6)

Building Motion is not a bolt-on — it is the natural completion of this
substrate. The main thread already knows where views are (presenter
snapshot). Motion makes it compute where they are *going*.

### The competitive case

If Exact nails gesture-driven animation at this level, it becomes the
first cross-platform framework where:

1. Touch tracking never leaves the platform's input thread (main)
2. Physics simulation runs in compiled Rust, not interpreted JS
3. Gesture arbitration is a framework contract with receipts, not a
   library convention
4. Compositor-path motion — gesture → physics → style → render — runs
   with zero thread hops and no app-JS involvement
5. Layout-affecting motion has an explicit, bounded cost model:
   constraint route by default (next runtime commit), same-frame only
   through eligible layout islands (LLP 0313)

Point 5 is deliberately weaker than the March draft's "the entire
pipeline has zero thread hops": under the two-domain model, arbitrary
layout crosses to the runtime thread. That honesty is what makes the rest
of the table credible.

## Design Principles

### 1. Gestures are Motion-domain objects, not JS event streams

A gesture recognizer is a state machine that lives in the main-owned
Motion domain, receives input samples (from thin platform-recognizer
adapters in v1, raw pointer intake where no platform recognizer fits),
and emits structured gesture state (position, velocity, phase) to the
binding graph and — as semantic, coalesced events — to app JS. JS declares
what recognizers exist and how they compose; Motion runs them.

### 2. Animation values are shared-memory state with a durable registry

A `SharedValue` is a slot in kernel-owned shared memory (LLP 0297 §4.5)
plus a registry entry owned by this document. It can be:
- Written by a gesture recognizer (finger position → value)
- Written by an animation driver (spring simulation → value)
- Written by app JS (imperative state change → value, delivered async)
- Written by a worklet (derived values, gesture math)
- Read by an animated binding (value → view property)
- Read by app JS (a validated atomic read of the latest published value)

All reads and writes avoid serialization. The Motion domain updates
animated bindings on the display link without waking app JS.

### 3. Composition is declarative and arena-resolved

Simultaneous gestures (pan + pinch on the same view), exclusive gestures
(swipe-back vs. horizontal scroll), and gesture-to-gesture handoff are
declared in JS but resolved by the arbitration arena (LLP 0336 §6.1) —
one policy-and-receipt authority per surface, fed by adapters, with
exclusive leases, defined checkpoints, and bounded receipts. The
composition API compiles onto arena claims; it does not invent a second
conflict-resolution mechanism.

### 4. Springs are the default physics

Timing curves are for transitions (Tier 1). Springs are for gestures and
interactive motion (Tier 2). Spring physics produces natural-feeling
motion that responds correctly to velocity — when a user flings a card,
the spring inherits the gesture velocity and decelerates naturally. Tier 1
ships a spring *kind* too (`TransitionKind::Spring`); Tier 2's distinction
is not spring availability but **velocity inheritance from live gestures
and mid-flight retargeting**, which state-driven transitions cannot
express.

### 5. The system degrades gracefully on web

On web, gesture recognition runs in TypeScript (pointer events are already
on the main thread — there is no thread-hop problem to solve) and
animation uses `requestAnimationFrame` as the primary driver, with WAAPI
as an optimization for fire-and-forget curves (velocity-inheriting springs
and decay cannot be expressed as WAAPI effects directly; see §Web). The
API is identical; the execution model adapts to the platform. The shipped
web recognizer (LLP 0336 P1) is **permanent**, not interim.

### 6. High-level APIs preserve semantics by default

Motion should expose semantic, high-level APIs first and low-level
recognizer APIs second. High-level APIs are where accessibility, focus
behavior, keyboard affordances, hover state, and platform feedback are
preserved by default — and where LLP 0336's witnessed-signifier law ("no
signifier, no accelerator") is enforced for command-shaped gestures.
Lower-level gesture APIs still exist for custom interactions, but they
should not be the only ergonomic path.

### 7. Motion is one subsystem, not two loosely-coupled libraries

React Native's Gesture Handler and Reanimated prove the value of native
recognizers, shared values, and UI-thread execution. They also show the
cost of splitting the model into separate mental worlds. Exact learns
from the primitives without copying the packaging: Motion is one
subsystem inside the Exact runtime, not a gesture library bolted to an
animation library.

## Prior Art and Design Lessons

### React Native: RNGH and Reanimated

[React Native Gesture Handler](https://docs.swmansion.com/react-native-gesture-handler/docs/gesture-handlers/about-handlers/) validates the need for native recognizers and explicit composition relationships such as `waitFor`. [Reanimated](https://docs.swmansion.com/react-native-reanimated/docs/fundamentals/glossary/) validates the `SharedValue` model, a constrained worklet environment, and per-frame animation objects that can drive view properties without routing through React on every update.

The key lesson is positive on primitives and negative on packaging. Exact should absolutely copy the good parts: recognizer composition, shared values, spring/timing/decay drivers, and a small UI-thread execution model. It should not reproduce the split architecture where developers must constantly think about "React world" versus "animation world." Motion should feel native to Exact's runtime model and protocol.

### Flutter

[Flutter's gesture system](https://docs.flutter.dev/ui/interactivity/gestures) and its gesture-disambiguation model show the value of framework-owned arbitration. Flutter's gesture arena makes conflict resolution a first-class system concern rather than an ad hoc convention. [Flutter's animation stack](https://docs.flutter.dev/ui/animations/overview) also demonstrates the power of scheduler-owned ticking through `Ticker` and `AnimationController`, with direct support for physical simulations such as springs and flings. [Hero animations](https://docs.flutter.dev/ui/animations/hero-animations) show that shared elements work best as a route-level concept rather than a library trick.

The lesson for Motion is that touch tracking, frame stepping, shared element orchestration, and animation ownership should all feel like one coherent runtime capability. Arbitration belongs to the framework. Frame cadence belongs to the framework. Shared-element overlays should be part of the navigation/motion stack, not an afterthought.

### Jetpack Compose

[Jetpack Compose's gesture guidance](https://developer.android.com/develop/ui/compose/touch-input/pointer-input/understand-gestures) is especially instructive because it explicitly defines multiple levels of abstraction and recommends preferring higher-level gesture modifiers when possible. That recommendation matters: the high-level path preserves semantics, keyboard support, focus behavior, and visual affordances. Compose's [value-based animation APIs](https://developer.android.com/develop/ui/compose/animation/value-based) and `Animatable` model reinforce the idea that the core motion primitive should be a value object with direct, retargetable control. [AnchoredDraggable](https://developer.android.com/develop/ui/compose/touch-input/pointer-input/migrate-swipeable), [nested scroll](https://developer.android.com/develop/ui/compose/touch-input/scroll/nested-scroll-modifiers), [shared elements](https://developer.android.com/develop/ui/compose/animation/shared-elements), and [predictive back](https://developer.android.com/develop/ui/compose/system/predictive-back) all push in the same direction: anchored motion, seekable progress, and scroll/gesture interop are core architecture, not polish.

The lesson for Motion is that it should expose three layers clearly:
- Semantic/high-level motion APIs
- Recognizer/shared-value APIs
- Raw pointer/input APIs

It should also treat anchored motion and progress observability as first-class. Sheets, drawers, swipe-dismiss, and interactive navigation should all be built on the same seekable state model.

### Apple: SwiftUI and UIKit

SwiftUI reinforces the idea that gesture and animation APIs should feel built into the UI framework rather than bolted on from the side. UIKit remains the clearest analog for Motion's interactive-navigation behavior. Apple's documented [`UIPercentDrivenInteractiveTransition`](https://developer.apple.com/library/archive/featuredarticles/ViewControllerPGforiPhoneOS/CustomizingtheTransitionAnimations.html) model is almost exactly the lifecycle Exact wants: compute a completion percentage from gesture events, update progress continuously, then finish or cancel explicitly, with cleanup driven by cancellation state.

The lesson for Motion is that cancellation is not an error case. It is a primary lifecycle path. Motion should keep transient gesture state separate from committed application state, and every interactive operation should define the cancel path as carefully as the commit path. (RFC 0100's two-phase resolution — logical commit vs. visual settle — and LLP 0336's lease lifecycle are this lesson, made normative.)

### Android Views: MotionLayout

[MotionLayout](https://developer.android.com/develop/ui/views/animations/motionlayout) is worth studying even though Exact should not copy its XML authoring model. Its important ideas are seekable transitions, touch-driven `OnSwipe`, explicit progress, and room for richer keyframes. MotionLayout makes it obvious that interactive motion works best when progress is a first-class value rather than an opaque side effect.

The lesson for Motion is that transitions should be seekable by design. Presets, sheets, and custom transitions should all be writable in terms of explicit progress. Richer keyframe support can come later, but the progress model has to be right from v1.

### Implications for Motion v1

1. Motion is one subsystem, not a gesture package plus an animation package.
2. The API surface should have clear layers: high-level first, low-level second, raw input last.
3. `SharedValue` remains the universal currency for gesture state, animation state, transition progress, and scroll-linked effects.
4. Anchored motion is a core primitive, not a specialty API.
5. Scroll arbitration and nested gesture interop are foundational design work (now normative in LLP 0336 §6.1).
6. Interactive motion must be seekable by progress and have explicit finish/cancel semantics.
7. High-level APIs must preserve semantics and accessibility by default.
8. The worklet runtime remains a constrained math-and-binding environment (LLP 0297 §4.3/§4.6), not a second general-purpose programming environment.

## Implementation status (2026-07-13, ENG-24695 landing)

The implementation program has landed the durable SharedValue registry and
lifetime model, whole-root graph transport and command plane, acknowledged
lifecycle outcomes, analytic drivers and per-surface MotionClock, substantial
native/web recognition and arbitration, all four composition automata, the
typed worklet ABI, scroll bindings, and M8
navigation/dismiss/command/reduced-motion/Acto integrations on the Apple and
web paved paths.

Status remains Review, not Implemented, because the retained physical evidence
gates are not satisfied. M2, M6, and M8 are implementation- and
registered-acceptance-complete on the macOS/web v1 paths. M4's implementation
is complete on those paths, but its named-browser edge evidence is still
absent. Separately, RFC 0100 P1's physical ProMotion fidelity bundle and
stack-only shadow-readiness artifact, plus P1.5's tab-pop cohort artifact, have
not been supplied. Android and Windows remain outside Motion v1 acceptance;
iOS runtime-thread promotion was owned by ENG-23520, which resolved
2026-07-23 (LLP 0322 revision: iOS runtime thread default-on, all W4b
device gates recorded on hardware); the still-open iOS Motion remainder
is ENG-23516's island producer/clock/host-op integration plus iOS
acceptance evidence.

### Historical pre-M1 baseline

The table below is retained as the implementation program's starting snapshot,
not as current status. Each row's State cell named its **category** explicitly:
**production-shipped** (load-bearing as described — several rows are
deliberately narrow, and the description is the claim),
**DEBUG-gated proof** (real code, compiled out of Release),
**governing design** (a sibling authority this document consumed, not
shipped implementation), or **proposed** (the final row — this RFC's original
scope). Landed implementation slices supersede the corresponding baseline
gaps; the milestone rows remain authoritative for work that is still open.

| Piece | State | Where |
|---|---|---|
| SharedValue storage (single-word f32 slots, per-slot atomics) | Shipped spike (LLP 0297 §4.5/W2) — **no per-slot write generations**; the setter (`exact_shared_value_set`) takes no generation argument, worklet writes bypass it entirely (raw-pointer binding via `ex_worklet_bind_shared_values`), and the resident runtime binds a fixed 64-slot slab | `kernel/src/ffi.rs` `SharedValueSlab`, `exact_shared_value_*` |
| UI worklet runtime (persistent restricted Hermes on main) | Shipped **on the Apple hosts** (LLP 0297 §4.3; invoke p95 1.07µs). The shipped install is **source-text**: `install(id: String, source: String, generation: UInt64)` over `ex_worklet_install(handle, id, source, len, generation, out_error)`; the build plugin (`createWorkletSourcePlugin`) extracts **top-level** `'worklet'` functions as source strings. The invoke ABI is **JSON-in/JSON-out** (`ex_worklet_invoke(…, args_json, out_result_json)`, allocating). HBC install, nested/inline extraction, captured-constant serialization, and a typed no-JSON ABI are **not built** — they are this RFC's M6 (0297 §4.3's `installWorklet(id, bytecode, capturedConsts, generation)` is contract language, not shipped code). `scheduleOnAppRuntime` buffers main-side in a **capped drop-oldest queue** — best-effort by construction — and the production drain into the app runtime is M6 wiring | `ExactWorkletRuntime.swift:133`; `vendor/ibex/src/engine/hermes_runtime_worklet.cc`; `packages/exact-devtools/src/worklet-source-plugin.ts` |
| Gesture worklet proof slice | **DEBUG-gated proof** (LLP 0297 W5) — **macOS-only AND `#if DEBUG`**: every entry point rides the `worklet.installGestureDriver`/burst host ops inside `#if DEBUG && os(macOS)` (`ExactRuntimeEngine.swift`), so nothing of this slice exists in a Release build. Platform pan recognizer → worklet invoke (JSON args/result) → **direct layer transform**. Its measured number is **elapsed hot-path time, not observed presentation**: a simulated-120Hz burst through that path, elapsed-time p95 0.248ms/sample, 0/600 samples over the 8.333ms budget. It does **not** exercise a SharedValue-slab→binding pipeline — that, with presentation-timestamp evidence, is M3/M4's measurement | `ExactWorkletGestureDriver.swift` |
| Same-frame layout islands | Production-shipped (LLP 0313; max layout+apply 0.214ms @120Hz hardware) on macOS and iOS presenters. M2's inventory-owned `layout-island-binding` kind is now the production registration authority: the renderer emits whole-root bindings, Apple resolves their SharedValue dependencies to real registry tenants, and graph detach/attach unregisters/re-registers complete island sets. True write-generation sampling is live. The old `island.register`/`island.unregister` host operations remain deprecated compatibility/debug fallback only | `kernel/src/island.rs`, `packages/exact-renderer/src/host-ops.ts`, `ExactMotionGraph.swift`, `ExactIslandRuntimeProducer.swift` |
| Web recognizer (**permanent**, not interim) | Production pointer path now includes the bounded M4 arena leg: one document capture intake, per-root frozen topology, nested Motion/pager/scroll/system arbitration, one external lease, three checkpoints, multipointer membership, loser suppression, and structural/reset/external-owner cancellation with bounded receipts. Gesture composition stays internal to one arena contender. **Still partial, not “M4 complete”:** wheel remains on the pager-specific recognizer path; RouterHistory has no web arena publisher; pager settle releases at semantic dispatch instead of a token-bearing re-center acknowledgement; reduced-motion pager settle and mobile-browser inset-edge/named-browser proof remain open; and Motion's transform write still overwrites authored transforms | `packages/exact-renderer/src/motion-gesture-recognizer.ts`, `dom-mirror.ts` |
| Native minimum Motion slice | Shipped (LLP 0336 P2), precisely: a main-owned Rust **pager recognizer** plus the **typed arbitration seed** — `InteractionProfile`/`InteractionClaim`/`InteractionArena`/`ArbitrationDecision` + a bounded receipt ring and `decide()`, exercised by unit tests; **no `InteractionLease` type, checkpoint machinery, or outcome lifecycle yet** — and current LLP 0336 (r15) records that machinery as a **normative P3-entry contract outside v1 acceptance, owned jointly with this document**, not unfinished P2 state. Its absorption is exactly this document's **M4 work on both the native and web implementations**, with conformance tests. `ExactPagerMotion.swift` is today a **host-side controller** — it owns descriptor decoding (with hand-coded opcode/entry-size identities), recognizers, macOS wheel candidate selection, commit/settle, reduced-motion settlement, semantic dispatch, and timeout — whose policy ownership migrates into Motion at M4. The pager binding descriptor + `PagerCommit` event are allocated inventory-first | `kernel/src/motion.rs`, `ios/…/Engine/ExactPagerMotion.swift` |
| Contract transient value channels (stage 0, web) | Shipped (LLP 0281; **stage 1 declined** per its §3.1 gate) — **pointer** channels (rich `PointerSample` records with sealed edge copies; phase/element-box ride the envelope), semantics **normative for this RFC's native channels** | `packages/exact-contract/src/runtime/channels.ts`, `capabilities.ts` |
| Tier 1 transitions | Shipped on Apple + web; **Android renderer skips `SetTransition`; Windows scene projection carries no transition state**. `TransitionKind::Spring` is a physical `CASpringAnimation` on Apple; web lowers spring-kind to a **fixed easing approximation** (`css-motion.ts`) | `kernel/src/transition.rs`; `ExactUIKitPresenterTree.swift` / `ExactAppKitPresenterTree.swift` (`CABasicAnimation`/`CASpringAnimation`); `packages/exact-renderer/src/style/css-motion.ts` |
| Scroll events | Shipped (`EventType::Scroll = 13`, host-ops channel by design) | `kernel/src/tree.rs` |
| Display-link plumbing | Shipped, but **not yet a per-surface Motion clock** — today's ticker inventory: macOS view-scoped `NSView.displayLink(target:selector:)`; the **production** iOS link is the per-host demand-started `CADisplayLink` in `ExactSurfaceHost` (only when native views need it; 120Hz `CAFrameRateRange` requested) — the `ExactSurfaceRenderer` link belongs to the **DEBUG-only emergency SwiftUI renderer stack, compiled out of Release**, and generic secondary-surface presenter mounts have **no display-link lifecycle at all** (a production secondary-surface ticker does not exist yet — an M3/M8 gate); **on the iOS flag-off (non-dedicated) path only**, an engine event-loop pump link pinned to 60Hz (`CAFrameRateRange(60,60,60)`) — dedicated mode is wake-driven with no main-side tick at all, and macOS flag-off uses a dispatch timer. The island pass is **one per engine instance**, shared across that engine's windows. The unified MotionClock registry is M3 | `DisplayLinkCoordinator.swift`, `ExactSurfaceHost.swift`, `ExactSurfaceRenderer.swift` (DEBUG-only), `ExactRuntimeEngine.swift`, `ExactIslandRuntimeProducer.swift` |
| Interactive navigation design | **Governing design, not shipped implementation** — RFC 0100 (Review): readiness published as two generation-stamped halves (router `BackDisposition` + main-owned presenter readiness) combined through the presenter's SceneLease; two-phase resolution; three progress channels | `llp/0100-interactive-navigation-transitions.rfc.md` |
| Threading substrate | LLP 0297 Implemented; runtime thread **default-on for macOS (2026-07-07) and iOS (2026-07-23)** (LLP 0322; `EXACT_RUNTIME_THREAD=0` kill switch), opt-in on Windows pending ENG-23494 | — |
| General Motion: durable registry + lifetime (M1), graph transport (M2), driver family (M3), recognizer generalization + arena completion (M4), composition API (M5), authoring surfaces + worklet ABI (M6), scroll bindings (M7), surface integrations (M8) | **Not built — this RFC** | — |

## Architecture

### Where this fits in the three-tier model

```
Tier 1: Transitions (LANDED on Apple + web; Android/Windows pending)
  Declarative style.transition → renderer-native animation
  For: state-driven fades, scales, color changes (incl. spring-kind)
  Runs on: renderer's animation system (Core Animation, CSS, etc.)

Tier 2: Motion (THIS RFC)
  SharedValue + gesture recognizers + animation drivers + animated bindings
  For: gesture-driven motion, scroll-linked effects, physics-based animation
  Runs on: main-owned Motion domain (Rust) + UI worklet runtime, ticked by
  the display link; layout-affecting values via constraint route or
  eligible layout islands (see execution classes). On web: TypeScript on
  the main thread (§Web).

Tier 3: Layout Transitions (FUTURE)
  Snapshot-based animated relayout
  For: list reorder, text reflow, grid rearrangement
  Runs on: kernel + renderer cooperation (LLP 0313's islands are the
  bounded, gesture-linked precursor, not the general Tier 3)
```

### The missing piece inside this model: a baked timeline format

Recorded 2026-08-01 from LLP 0420 §7 (PocketJS study). The three tiers above are
the right decomposition and nothing here proposes a fourth. What Exact lacks is
an *authoring and execution format* inside them: a named, **build-baked,
presenter-played** timeline — per-property segment data with CSS-like delays,
fill modes (`forwards`/`backwards`/`both`), `reverse`, `infinite`, and
multi-property synchronization — that runs with **zero per-frame JS**. PocketJS
compiles every CSS `animation` shorthand into frame-precise per-prop segment
timelines in `styles.bin` and plays them in its Rust core at fixed dt.

Two things this is *not*, both of which LLP 0420 rev 2 got wrong before its
review rounds corrected it. It is not a missing tier: Tier 2 already composes
`sequence` and `repeat`, so a pulse, a spinner, and a staged entrance are
expressible today on the shipped Contract path. And it does not cost gesture-worklet discipline: routing here
is by capability, not trigger ("programmatic drivers are Tier 2 by definition"),
and the worklet-eligibility rules bind gesture-edge callbacks, not all
programmatic choreography.

**Sequencing, if this is taken up.** Do the cheap thing first: treat baked
choreography as a **compiler lowering over the existing Motion graph** — a
"static choreography" certificate proving no dynamic dependencies, lowering
keyframes into the current `Sequence`/`Repeat` drivers. Introduce a
presenter-native timeline artifact only if measurement shows the per-frame driver
overhead is real, or if delay/fill/multi-property semantics genuinely cannot be
expressed by the existing graph. **That measurement was taken on 2026-08-01 and
the gate did not open**: a staged entrance over 32 elements costs ~61 µs/frame
(0.37% of a 60 fps frame) on web, and on native this class of choreography is
already played by `kernel/src/motion.rs` at zero per-frame JS — binding a
SharedValue into a native motion root calls `acquireNativeDriverAuthority()`,
which sets `running = null` and deactivates the web driver. A presenter-native
baked timeline would re-solve a solved problem on the tier that matters most.
What survives is the compiler-lowering half as an authoring convenience. See
`issues/20260801-motion-baked-timeline-measurement.md`. Adopt PocketJS's soundness constraints either way: build-time absolute
keyframe values, 0% and 100% pinned per animated property, anything dynamic
staying on the ordinary Tier 2 path. Keep the semantic-timeline claim separate
from any pixel claim — the latter holds only on a qualifying deterministic paint
profile, which no native Exact host is.

RFC 0089's four Contract motion categories map onto this model rather
than onto one universal path: **state change** → Tier 1 by default;
**enter/exit** → behavior primitives/presence over Tier 1/2;
**collection reorder** → Tier 3 (future); **route transition** →
router + RFC 0100 on Tier 2 substrate.

**Routing is by capability, not by trigger.** "Gesture ⇒ Tier 2" is the
common case, not the rule: the routing truth is what the motion *needs*
— velocity inheritance, mid-flight retargeting, physics, seekable
progress, or per-frame input put a value on Tier 2 **even when
state-triggered** (programmatic drivers are Tier 2 by definition), while
a two-known-endpoint property change with none of those needs is Tier 1.
Tier 1's shipped property set (opacity, transform, background color,
border radius — `kernel/src/transition.rs`) also bounds what it can
carry at all: a state-triggered expanding card (RFC 0089's own example)
that needs interruption or layout participation routes to Tier 2/E3, not
to a Tier-1 transition that cannot express it.

### Execution classes (authoritative for every latency claim below)

| Class | Path | Latency | Owner |
|---|---|---|---|
| E1 Renderer-native transition | Tier 1 `SetTransition` metadata → platform animation | platform-scheduled | Tier 1 |
| E2 Compositor projection | Motion domain writes transform/opacity through the presenter | same frame | LLP 0297 §4.4 |
| E3 Same-frame layout island | eligible feed-backed, layout-closed subtree; dynamic inputs = SharedValues + current host constraints + frozen feed data (LLP 0313's eligibility set); input-matched settlement | same frame, bounded | LLP 0313 |
| E4 Constraint-route layout | main → constraint buffer → runtime-thread Taffy → next commit | **next runtime commit** — nominally one frame when the runtime thread is schedulable; longer behind a busy app-JS task or GC (LLP 0297 §4.4's honesty note) | LLP 0297 §4.4 |
| E5 Coalesced observability | Motion → app JS / agent surface, latest-wins | ≥1 frame, throttled | this RFC + LLP 0336 |

Compositor projection (E2) is the first choice whenever true relayout is
not required. Widening LLP 0297's single sanctioned servicing wait (live
resize) is **not** on the table for Motion (0313's recorded answer).

### System diagram

```
main thread                                        runtime thread
─────────────────────────────────────────────      ──────────────────────
Platform input (UIKit/AppKit recognizers,          App JS (Hermes)
NSEvent scroll phases, WM_POINTER planned,          │ declare gestures,
Pointer Events on web)                              │ values, bindings
    │ samples (thin adapters)                       ▼
    ▼                                              Tree/layout kernel
┌───────────────────────────────┐                  (`Kernel`, Taffy,
│ Motion domain (main-owned     │                  protocol dispatch)
│ Rust; LLP 0297 §4.4)          │                       │
│  recognizers · arbitration    │   descriptor sidecar  │
│  arena (LLP 0336 §6.1) ·      │◄──(ring, transaction──┘
│  drivers · binding tables ·   │    scoped)
│  geometry mirror (presenter   │──semantic events / coalesced──►
│  snapshot + feed frames)      │        observability (to app JS)
└─────┬───────────┬─────────────┘
      │           │ SharedValue slab (kernel-owned shared memory,
      │           │ tagged per-slot atomics; seqlock records are design
      │           ▼ for multi-word values — the shipped slab is the
      │      ┌────────────────────────┐            single-word spike)
      │      │ UI worklet runtime     │  restricted Hermes, resident
      │      │ (LLP 0297 §4.3)        │  math-class worklets
      │      └───────────┬────────────┘
      ▼                  ▼
┌──────────────────────────────────┐
│ Presenter writes                 │  E2 same-frame (transform/opacity)
│ E3 layout islands (eligible)     │  E4 constraint buffer → runtime
└──────────────────────────────────┘  thread for arbitrary layout
```

Layout-driving bindings do **not** run Taffy on the display link in
general: eligible subtrees use layout islands (E3); everything else takes
the constraint route (E4). This corrects the March diagram, which
predated the two-domain split.

### Clock ownership

Today's clocks are **not yet the model this document needs**: macOS
presenter hosts tick via view-scoped `NSView.displayLink`, iOS creates
per-host `CADisplayLink`s on demand (two creation sites — surface host
and surface renderer), the iOS flag-off path additionally runs a
60Hz-pinned event-loop pump link (dedicated mode is wake-driven with no
main-side tick; macOS flag-off uses a dispatch timer), and the
layout-island pass is **one per engine instance**, shared across that
engine's windows. Motion's target — stood up at M3, absorbing those
tickers without duplicate links — is a per-surface **MotionClock
registry**: one scheduler per surface (window), with consumers (drivers,
bindings, the island pass, native-view ticks) registered against the
surface hosting their root. Ordering within a tick is fixed: input
sampling → arena checkpoints → driver advance → worklet evaluation →
binding application (E2/E3/E4 routing) → observability.

Rules:

- Drivers and bindings tick on their root's surface clock. (A value
  consumed by bindings on two surfaces would sample at each consumer's
  cadence with the slab as shared truth — but that is **post-v1**: v1
  values are single-root (§Registry), so the case arises only when
  OQ13 lifts the restriction.)
- Window migration across displays re-anchors the clock (macOS
  view-scoped links track their view's display; iOS display association
  and migration are M3 design work).
- **Time advances with the world, not the clock.** A hidden or suspended
  surface stops *presenting*, but driver time is true elapsed time:
  analytic drivers evaluate at the real elapsed point on the next tick
  (they may settle across a gap; terminal events fire at the next tick
  opportunity), and quantum-integrated drivers cap catch-up steps and
  re-anchor beyond the cap. The clamp is a numerical-integration rule,
  never a time-stop.
- **Idle clocks stop.** A surface with no active drivers, gestures, or
  registered tickers stops its clock (the demand-started pattern the iOS
  host already uses). An infinite `withRepeat` driver pins its surface's
  clock by design — per-driver frame-rate preferences and ambient-motion
  power policy are OQ13.
- **An in-flight gesture on a suspending surface is a structural
  cancellation** (the arena's invalidation class — spring back, no
  dispatch): unlike drivers, a gesture cannot fast-forward through a
  gap, because its input source is gone.

Cross-surface values, mid-gesture display migration, and the power
policy are OQ13.

### Data flow for a swipe-to-dismiss gesture

```
1. JS declares the gesture, its SharedValues, bindings, and thresholds
   (descriptors ride the commit sidecar, generation-fenced, ahead of any
   touch)
2. User touches; the platform adapter (v1: a thin wrapper over
   UIPanGestureRecognizer et al.) forwards samples into the Motion domain
3. The arena freezes stream topology; candidates race slop/axis-lock;
   the pan claims its exclusive lease (LLP 0336 §6.1)
4. Recognizer transitions Began → Active; hit context comes from the
   main-side presenter snapshot (never the kernel tree)
5. Per sample: recognizer writes translation into a SharedValue slot;
   dependent worklets run in the UI worklet runtime; bindings write
   transform/opacity through the presenter — same frame (E2)
6. User lifts. Commit checkpoint re-checks the lease holder; velocity +
   displacement decide commit vs cancel
7a. Cancel: a spring driver retargets the value home (velocity
    inherited); no app JS involved
7b. Commit: the driver springs offscreen; Motion emits ONE semantic
    event to app JS (async; outcome-log reliable — at-least-once +
    idempotent consumption), carrying phase,
    velocity, and its resolution identity
8. App JS receives the terminal record and reacts. THIS walkthrough is
   the component-local lifecycle: the framework dispatches the bound
   CommandRef (the dismissal); retention is component-local Presence
   (retain-through-settle per RFC 0007's usePresence amendment) — the
   unmount is emitted when the settle spring rests. Terminal records:
   gesture resolution (verdict + velocity), driver settled. No router
   or pager machinery is involved.
```

Steps 1–7 are the shared kinematic core; **step 8 is
lifecycle-specific, and the three completion lifecycles do not
compose** — each has its own authoritative command, retention owner,
terminal records, settle condition, and acknowledgment:

- **Component-local (this walkthrough).** Command: the gesture's bound
  `CommandRef`, framework-dispatched on committed resolution.
  Retention: Presence (component-local). Terminal records: gesture
  resolution + driver settled on the outcome log. Settle: the spring
  at rest releases the retained subtree. Acknowledgment: none beyond
  the log's consumed cursor — nothing re-renders under the gesture.
- **Router-owned (edge-back, routed swipe-to-dismiss — RFC 0100).**
  Command: the router pop, dispatched through 0100's confirmation
  path (DecisionCell authorization; ack/nack records —
  §Protocol and Transport). Retention: the rendered-route-set
  transition-retention entry (0100 §4b; RFC 0010 Invariant 7), owned
  by the presenter's settled signal. Terminal records: gesture
  resolution; driver settled. Settle: spring at rest AND (for commits)
  the commit token observed. Acknowledgment: 0100's ack/nack plus the
  scene-lease release.
- **Pager turn (LLP 0336 §6).** Command: the semantic `PagerCommit`
  through the ordinary action path. Retention: the presenter pins the
  turned transform (no scene teardown). Terminal record: turn
  resolution carrying the turn token. Settle: immediate on
  reduced-motion, spring otherwise. Acknowledgment: the **token-bearing
  descriptor update the re-centered re-render produces** (0336 §6's
  commit → re-center handshake) releases the pinned transform — the
  runtime→main changed descriptor IS the re-center acknowledgment;
  there is no separate main→runtime ack record.

App JS is involved at step 1 (declaration) and step 8 (semantic
reaction). Steps 2–7 are main-thread Rust + worklets at display-link
cadence. If app JS is busy, tracking and settle are unaffected; only the
semantic reaction (and observability) waits. The semantic terminal event
in step 7b is the **reliable** completion path — the JS API below teaches
it; `runOnJS` never carries commits.

## Detailed Design

### SharedValue

```typescript
// JS API (React tier; the Contract projection is LLP 0281's
// transient bindings/value channels — same substrate, one per-frame
// value system)
import { useSharedValue, useDerivedValue } from 'exact';

// Create a shared value; storage is a slab slot, identity is a typed
// handle (slot index + generation + engine epoch)
const translateY = useSharedValue(0);

// Derived values are computed in the UI worklet runtime when
// dependencies change
const opacity = useDerivedValue(() => {
  'worklet';
  return interpolate(translateY.value, [0, 300], [1, 0], Extrapolation.CLAMP);
});

// Read from app JS: synchronous and WAIT-FREE — a generation-validated
// atomic load from the slab, one of LLP 0297 §4.5's named access points.
// It is never a cross-thread servicing wait; this sentence is the
// LLP-logged classification of this read against the standing
// async-first module rule. Reads see the handle's own pending write
// (below); otherwise they return the latest published value. Per-slot
// only — no cross-slot consistency.
console.log(translateY.value);

// Write from app JS: applied to the handle's local shadow immediately
// (read-your-own-write within the app runtime) and delivered to the
// Motion domain asynchronously (microtask flush). Writes carry ONE
// PER-ROOT MONOTONIC COMMAND SEQUENCE (motionSeq, allocated at write
// issue, applied in issue order — not per-handle, which would make a
// per-root watermark incomparable), and the shadow clears against an
// acknowledgment, not a guess: Motion publishes the highest APPLIED
// command sequence per root (the applied-watermark control atomic),
// and the shadow lifts only when appliedSeq >= this write's
// sequence — a write-generation bump alone cannot clear it, because
// another writer (gesture, worklet) can advance the generation before
// this command lands. Ordering: the applied-sequence publication is
// Release-ordered after the command's tagged-word CAS; the runtime
// reads it Acquire. Stale-generation writes are no-ops (counted in
// dev). An imperative app write behaves like a gesture write toward
// drivers: it CANCELS an active driver on that value (author intent
// wins — the same rule as "finger wins").
translateY.value = 100;
```

Supported value shapes: `f32` (the per-frame class — tagged per-slot
atomics), `bool` (encoded as f32 0/1), and multi-word `[f32; 2]` /
`[f32; 4]` records (positions, colors) using the seqlock protocol
LLP 0297 §4.5 names for multi-word — **design, not shipped: the existing
slab is the single-word spike**. More complex structures (full
transforms) are composed from multiple values; Contract pointer channels
use a dedicated record-bundle layout (see §Pointer-channel records).

#### Storage vs. registry (two layers, two owners)

- **Storage** is LLP 0297 §4.5's `SharedValueSlab`: kernel-owned shared
  memory with per-slot atomics. This shipped as the single-word spike;
  the tagged-word protocol below is M1.
- **The registry** — allocation, identity, lifetime, and the binding graph
  — is this document's normative content (it receives LLP 0297 OQ2). The
  March draft's single `SharedValue` struct conflated the two; they are
  separate, with per-item ownership: **identity, generation, epoch, and
  owner label** live in the registry (runtime thread); **active driver
  and dependents** live in the Motion-domain binding tables — that is
  the truth (worklet-initiated driver commands are main-local and never
  cross domains), and the registry may mirror them eventually-
  consistently for diagnostics only.

#### Registry and lifetime (normative)

- **Handles are typed indices plus generations plus an epoch.** A
  SharedValue handle is `{slot: u32, generation: u32, epoch: u32}`.
  Binding lookups and slab writes validate generation and epoch; **stale
  handles are defined no-ops, never errors** (LLP 0297 §4.4's
  tolerated-not-raced rule). A stale handle *read* returns the handle's
  last-observed value (its local shadow) — it never reads another
  tenant's slot; slab-side stale reads return 0.0 with a dev diagnostic.
  The shadow's population rule: it is written by the handle's own
  writes **and refreshed on every successful validated read** — it is
  the stale-fallback cache (and the pending-write overlay), explicitly
  distinct from the withdrawn primary-read mirror: live reads still go
  to the slab.
- **The slot is one tagged atomic word — validate-and-write is a CAS.**
  Each single-word slot is one `AtomicU64` packing `{generation: u32
  (high), value: f32-as-u32 (low)}`. A write loads the word (Acquire),
  verifies the generation against the handle (and the handle's epoch
  against the slab epoch), and publishes with a single
  `compare_exchange` of the full word — same generation, new value;
  Release on success. **The successful CAS is the linearization point.**
  A generation mismatch at load or CAS is a counted no-op; retry only on
  value races within the same generation. Reads are one atomic load
  split into (generation, value); latest-value-only consumers may use
  Relaxed loads — each slot remains an independent latest-value
  register. Freeing CASes the generation to a tombstoned next
  generation — retrying past concurrent same-generation value CASes
  until it lands — which atomically strands every outstanding writer:
  the ABA window between "validate" and "publish" is closed because both
  live in the same word. The per-slot **write generation** is a separate
  monotonic counter bumped after a successful publish (Release) —
  advisory for samplers (LLP 0313's requirement), not part of the
  linearization. **Generation-sensitive sampling is made coherent**
  where it matters: a slot with registered samplers (island inputs)
  publishes under a per-slot odd/even **publication sequence**
  (seq→odd, value CAS, write-generation store, seq→even; readers retry
  on odd or changed), so a sampler's `(value, writeGeneration)` read is
  a matched pair — LLP 0313's prediction inputs require exactly this.
  Plain slots skip the sequence (one CAS, no extra ordering), and
  latest-value-only consumers keep Relaxed loads everywhere — they
  never read generations. The read ABI is validating:
  `exact_shared_value_try_read(slab, slot, generation, epoch)` returns
  the tagged word or a stale verdict — callers do not wrap a raw read
  in their own epoch check. Multi-word records use a seqlock whose sequence word
  embeds the lifetime generation (odd = writing); writers verify the
  generation before entering, readers retry on sequence change — and a
  seqlock record has **exactly one writer**: writer exclusion is a
  precondition declared per record class (pointer intake: main), not
  something the seqlock provides.
- **Cross-language access is a C ABI, not a pointer cast.** The shipped
  spike binds the raw slot array into the worklet runtime
  (`ex_worklet_bind_shared_values`) and reinterprets it as C++ atomics —
  serviceable for a spike, not a portable atomic-object contract. M1
  replaces both the generation-less `exact_shared_value_set` and the raw
  binding with exported accessors
  (`exact_shared_value_try_write(slab, slot, generation, epoch, value)`
  for every writer — app-runtime commands, drivers, recognizers,
  worklets; `exact_shared_value_try_read(slab, slot, generation, epoch)`
  for every handle-based consumer read). There is exactly ONE
  non-validating export, deliberately restricted and named for what it
  is: `exact_shared_value_sample(slab, slot)` returns the matched
  **`(value, writeGeneration, verdict)` triple** — the
  publication-sequence retry is INTERNAL to the accessor, one wide
  return, so the sampler class carries no retry-loop contract across
  the C boundary. **Memory order, pinned (r12; writer-side ordering completed r13):**
  publication is a seq→odd store, a **release fence** (ordering the
  field stores after the odd store — on weakly-ordered targets plain
  stores may otherwise reorder ahead of it, letting a sampler read new
  fields under an even-and-equal sequence: a torn pair accepted as
  matched), the value + write-generation stores, then a **Release**
  seq→even store; the sampler **Acquire**-loads the sequence on both
  sides of its field loads and retries on odd-or-changed — the
  torn-matched-pair interleaving is a named M1 Loom trace — matched
  pairs are a release/acquire property, not a Relaxed one. The
  **write generation is u32 per slot**, wrap-escalated exactly like
  lifetime generations (approach escalates to slot re-tenancy under a
  fresh lifetime generation — never an in-place wrap). The `verdict`
  distinguishes live from **tombstoned/stale-lifetime** slots — 0313's
  shipped sampler contract (nil-on-tombstone → retire the descriptor)
  is preserved, not weakened; what the accessor still deliberately
  omits is any epoch validation (the sampler class is
  reuse-protected by the §Reclaim watermark, and lifetime generation
  vs write generation are SEPARATE fields with separate jobs — r12
  states the split once). It exists **only** for the
  registered-sampler class — whose registered consumers are two:
  LLP 0313's island change-detector (whose
  prediction inputs are exactly this triple), and, since the
  2026-08-25 RFC 0490 M3 registration (see the Revised entry), the
  GPU uniform sampler (main, generated `gpu` phase, authority
  `tests/gpu/gpu-uniform-binding-v1.json`); it is not a general
  read API, and handle-based callers using it instead of `try_read`
  are a review error. (The raw tagged word — lifetime generation +
  value — is `try_read`'s return; no export hands it out unvalidated.)
  If profiling ever justifies a raw fast path beyond that, the
  slot layout is declared as a C struct with a documented atomics
  contract.
- **Capacity is segmented and addresses are stable — and growth never
  obstructs readers.** (r23 truth correction from the LLP 0411 OQ5
  audit: this bullet previously claimed "growth is wait-free ...
  appending a segment is a Release store." Neither matched the
  mechanism's own mechanics — growth heap-allocates the new segment
  and publishes it with a release-ordered compare-exchange
  (`ensure_segment`, `kernel/src/ffi.rs`), so the grow path is
  allocator-bound and makes no progress-class promise. The
  load-bearing property was always reader-side, and is what this
  bullet now claims.) The slab grows by appending fixed-size segments
  to a **fixed-capacity directory of atomic segment pointers**
  (preallocated at slab creation): the new segment's pointer is
  published into its directory slot with release ordering; readers
  Acquire-load the pointer — slot lookup is a bounded load chain,
  wait-free regardless of concurrent growth; the directory itself
  never moves or reallocates, so concurrent main/runtime readers are
  never invalidated or obstructed (the shipped fixed 64-slot binding
  is the spike this replaces). Segment lifetime
  has ONE rule: segment memory is never freed mid-session — epoch reset
  **recycles segments in place** (§Reclaim) and memory is released only
  at engine shutdown behind the cross-domain quiescence barrier. The
  **slab epoch lives in a
  slab-header atomic** (the slot word carries only the generation),
  which is what the exported accessors check the handle's epoch
  against. Directory exhaustion is a hard dev error with a diagnostic,
  like node-id exhaustion; directory capacity, default segment size,
  and counts are M1 constants recorded with the implementation.
- **Allocation is single-owner.** The registry lives with the tree/layout
  domain on the runtime thread, because lifetime is component lifetime:
  `useSharedValue` (mount) allocates; unmount frees. Allocation, attach,
  detach, and free records travel with the commits that create/destroy
  their owners (attach-before-use, detach-before-destroy) — at M1 on the
  interim host-op path (the `island.register` precedent), replaced at M2
  by the descriptor sidecar.
- **Reclaim is watermarked — consume-before-reuse is a mechanism — and
  M1 is append-only.** A freed slot is quarantined (generation
  tombstoned — concretely, the tagged word's generation field carries a
  dedicated **tombstone bit** (r13): live tenants publish it clear,
  frees CAS it set under the incremented generation, and the sampler's
  `verdict` reads exactly this bit within its sequence-mediated read —
  a tombstoned generation is distinguishable from any live tenant by
  one load, and re-tenancy publishes clear-bit words only after the
  watermark gate) until the Motion domain's **applied-sidecar watermark** —
  the highest sidecar `motionSeq` it has applied, published per root as
  an applied-watermark control atomic (§Protocol and Transport; a
  latest-wins control word beside the consumed cursor, which is the
  opposite-direction acknowledgment in the same control-record
  family) — passes the freeing transaction's
  sequence; where a value is consumed by more than one root, reuse
  waits for the **minimum** watermark across all consuming roots. Only then may the slot be re-tenanted under a new generation. A
  delayed writer holding the old handle CASes against a dead generation
  and no-ops — never the new tenant. Dev builds assert no write lands in
  the tombstone window. Sequencing honesty: the watermark transport
  arrives with M2, so **M1 ships append-only** — targeted frees
  quarantine indefinitely and no index is reused until M2 activates
  watermark-gated re-tenancy — with one bounded exception that keeps
  dev sessions from growing without limit: **epoch-scoped whole-slab
  reuse**. After a reset's generation sweep and teardown complete, the
  entire slab may be recycled under the new epoch (every possible
  straggler is already stranded by the sweep; validating lookups no-op
  on stale handles), so repeated HMR reloads do not accumulate
  quarantined segments. The recycle invariants that make the stranding
  argument airtight: **per-slot generation words are never reset** —
  whole-slab recycling re-tenants under strictly increasing generations
  with the sweep's tombstone as the floor, so a stale writer's expected
  `(generation, value)` word can never recur (u32 generation wrap, if
  ever approached, escalates to a slab-header-epoch re-handshake rather
  than wrapping); **epoch reset recycles in place, never frees** —
  segment memory is released only at engine shutdown behind a
  cross-domain quiescence barrier (both accessor-capable threads park),
  so an accessor in flight between its directory-pointer load and its
  CAS can never touch freed memory; **the sweep and teardown execute on
  main** (the Motion domain), and since ALL slab writes are main-side —
  imperative app writes ride the command path, and registry-allocated
  initial values do too — the runtime thread is never a slab writer at
  all, only a validating/sampling reader. (The slab is the **value
  plane**, and this rule holds without exception: the one word in the
  whole design whose semantics require two writers — RFC 0100's
  commit-authorization `DecisionCell` — is deliberately *not* a slab
  slot; it is a decision-record control word beside the outcome log,
  per the r9 adjudication recorded in §Protocol and Transport.) Scope
  honesty: **v1 values
  are single-root**
  (one owning root per value; cross-root sharing is prohibited until
  OQ13's multi-window design lands), which keeps the consumer set
  well-defined. And why the watermark is load-bearing rather than
  defense-in-depth: **per-tick binding reads validate the slot
  generation** (the binding path is a validating consumer), but pure
  samplers — LLP 0313's island change-detector reading write
  generations — use non-validating **sequence-mediated reads** by
  design (the accessor's internal Release/Acquire publication
  sequence, with no lifetime-generation/epoch verdict beyond the
  tombstone flag; "Relaxed" was the pre-r11 model); the
  watermark is what makes reuse safe for that read class. Watermark
  sequence comparisons are **epoch-qualified** (an epoch bump resets
  the sequence space; M2's tests pin the cross-epoch comparison).
- **Per-slot write generations.** Every successful publish bumps the
  per-slot write generation readable by samplers. This is a hard
  requirement recorded by LLP 0313: its change-counting sampler is
  interim precisely because worklet writes currently bypass
  `exact_shared_value_set`, and its islands (and any consumer needing
  "did this change since I looked?") switch to true write generations
  when this ships.
- **Engine epochs, fence-first — and the fence is a generation sweep.**
  HMR reset, engine restart, and worklet-runtime generation changes bump
  an epoch. Reset order is **fence-first**, matching LLP 0297 §4.8 and
  the shipped worklet-runtime reset (which bumps `workletGeneration`
  before anything else) — but an epoch counter alone cannot fence
  writers, because the epoch is validated *outside* the slot's atomic
  word: a writer paused between its epoch check and its CAS would still
  publish successfully. The fence is therefore realized **in the slots
  themselves**: reset first sweeps every live slot, CASing its tagged
  word to a tombstoned next generation (each CAS retrying past
  concurrent same-generation value writes until it lands) — per-slot
  linearizable, so the paused writer's CAS now fails — and only after
  the sweep completes are drivers cancelled, bindings detached, and
  worklets dropped. The handle's epoch field is **advisory** (fast-path
  rejection and diagnostics); the slot generation is the only fence a
  writer can trust. Reclaiming before fencing would let a straggler
  write into a recycled slot.
  **The reset barrier is an explicit asynchronous state machine** — the
  full reset is ordered, acknowledged, and never a synchronous
  cross-thread wait. Its states, in order, each entered only when the
  prior completes: **(1) sweep** (the per-slot generation fence above,
  plus tombstoning EVERY armed decision cell, all states — the decision-records row's rule — control plane);
  **(2) teardown** (drivers cancelled, bindings detached, worklets
  dropped, all main-side); **(3) epoch publish** (the slab-header epoch
  atomic advances — from here every validating access fails fast);
  **(4) transport sub-barrier.** Main first closes outcome admission and
  invalidates queued delivery fences. The retiring app-runtime FIFO then
  advances the semantic-dedup generation after every handler already entered
  on that executor. Only its asynchronous main completion clears the Rust
  logs, cursors, and reservations and permits successor admission. No thread
  waits synchronously, and predecessor closures cannot alias successor
  storage;
  **(5) recycle-enable** (whole-slab re-tenancy under strictly
  increasing generations — §Reclaim). Records produced against the old epoch
  are whole-transaction no-ops per the sidecar rules, while the explicit
  asynchronous FIFO completion above prevents reuse from outrunning an
  already-entered predecessor callback. In the landed M1 path, the barrier
  supplies this ordering structurally: install records arriving in states
  (1)–(4) are stale-epoch no-ops, and the post-barrier reinstall is the only
  install that lands. This replaces the older reset/boot race in which
  `workletGeneration` advanced before a successor runtime could reinstall
  (`ExactRuntimeEngine.swift`). The adversarial suite covers a writer paused
  between validation and publish across reset, plus an install, descriptor,
  command, and sampler read arriving in every barrier state.
- **Driver exclusivity and write arbitration.** At most one driver
  targets a value at a time; starting a driver retargets or replaces the
  incumbent (inheriting position and velocity). Gesture writes during an
  active driver cancel the driver (finger wins), and **imperative app-JS
  writes do the same** (author wins — the Reanimated convention). The
  total order per value is arrival order at the Motion domain on main:
  commands consumed from the ring, gesture/driver/worklet writes at
  their tick position — one writer domain, one serialization point.
  Exclusivity also pins terminal-event ordering across clock gaps: at
  most one driver per value is live across a suspension, and its settle
  event is delivered once, ordered before any later driver's events on
  that value. **Admission credits follow the same exclusivity:** every
  activated terminal-producing entity either appends exactly ONE
  terminal record (`settled` or `cancelled` — a driver cancelled by
  finger-wins/author-wins replacement appends `cancelled`, consuming
  its credit) or, where activation itself failed, releases its
  reservation — credits never leak across replacement, and M2's tests
  cover the replacement chains. A deliberately infinite driver
  (`withRepeat`) holds its credit for its whole life **by design** —
  that is one outstanding record slot, not a leak.
- **Leak diagnostics.** Dev builds track live slot count per root and per
  owning component identity — the registry allocates on the runtime
  thread beside the reconciler, so the owner label is available at
  allocation time without a new debug record class. A root teardown that
  leaves live slots logs the owners; the agent surface exposes the same
  counts.

**M1 verification for all of the above:** kernel unit tests for the
tagged-word CAS (stale generation, tombstone window), adversarial
interleavings — including a writer paused between validation and publish
across a full reset sweep (a Loom or equivalent model-checked
interleaving suite is the intended vehicle), free-CAS vs. concurrent
same-generation writer contention, reader during re-tenant, and
multi-word seqlock torn-read stress — append-only quarantine behavior
(no reuse at M1), fence-first sweep-then-teardown order, stale-handle
read semantics (shadow + slab 0.0 + diagnostic), and ABA reuse tests
staged for M2's watermark activation; LLP 0313's sampler switched from
change-counting to write generations with its island checks green; the
dev leak report exercised. (No new runtime callbacks — these reads are
not callbacks, so no `docs/callback-affinity.md` entries at M1;
progress-class honesty, r23: only the handle `try_read` class is
wait-free — the registered `sample()` path is a retrying seqlock read
whose progress depends on writer completion.)
Purpose, said out loud to preempt "why model interleavings that
cannot occur": under the all-writes-main-side rule every native slab
writer is serialized on main TODAY, so the tagged-word CAS machinery
is **topology insurance** — it buys atomic `(generation, value)` read
pairs for the runtime/sampler side now, and correctness headroom if
the write topology ever changes (OQ13 multi-window) — the
interleaving suite pins the insurance, not a live write-write race.

### Gesture Recognizers

#### Recognizer types

| Recognizer | State | Output |
|-----------|-------|--------|
| `Tap` | idle → began → ended / failed | tap count, position |
| `LongPress` | idle → began → active → ended | position, duration |
| `Pan` | idle → began → active → ended | translation, velocity |
| `Pinch` | idle → began → active → ended | scale, focal point, velocity |
| `Rotation` | idle → began → active → ended | rotation (radians), velocity |
| `Fling` | idle → began → ended | velocity, direction |

#### The v1 recognition boundary (hybrid, by design)

Motion owns the gesture state model, shared values, drivers, arbitration
policy, and transition progress. **Recognition intake is hybrid in v1**:

- **Apple platforms:** thin wrappers around platform recognizers
  (`UITapGestureRecognizer`, `UILongPressGestureRecognizer`,
  `UIPanGestureRecognizer`, `UIPinchGestureRecognizer`,
  `UIRotationGestureRecognizer`, `UIScreenEdgePanGestureRecognizer`;
  AppKit equivalents and `NSEvent` scroll phases) act as **sample
  adapters** feeding recognized state into Motion-domain controllers,
  enforcing arena decisions where the platform decides first and
  reporting outcomes (`won` / `failed` / `cancelled` / `external-owner`).
  The LLP 0336 P2 slice approximates this shape today — but honestly:
  `ExactPagerMotion.swift` still owns recognition policy, candidate
  selection, and settlement host-side, with `kernel/src/motion.rs`
  owning the numeric pager model. **M4 migrates policy ownership into
  Motion proper**; "thin adapter" describes the end state, not today's
  controller.
- **Web:** the permanent TypeScript recognizer (LLP 0336 P1) — pointer
  events + wheel accumulation with `touch-action` intersection and
  `pointercancel`-as-loss. See §Web.
- **Windows:** `WM_POINTER`-family intake is planned work (the current
  host handles `WM_LBUTTONDOWN`/`WM_MOUSEWHEEL` only); see §Windows.

This keeps Motion the architectural owner of composition and animation
while borrowing the platforms' tuned recognition, thresholding, and
scroll-view coexistence. Motion-native recognition remains the long-term
abstraction boundary where a platform recognizer doesn't fit (custom
gestures, Fling-as-primitive), and OQ3 tracks how far to push it.

#### JS API

```typescript
import { Gesture, GestureDetector } from 'exact';

// This example shows the M8-COMPLETE, COMMAND-BOUND form (a dismissal
// is a command-shaped gesture): .command() takes a TYPED, REGISTERED
// CommandRef — the same action reference the visible signifier (the
// close button) binds — which is what makes 0336's R2 witness
// correlation and R3 one-action-path rule enforceable rather than
// stringly asserted. On committed resolution, THE FRAMEWORK dispatches
// the CommandRef (with a dispatch receipt); onGestureEnd is
// observational only and carries no semantic authority. Until M8 wires
// command identities, the shipped subset is the visual-only form (no
// .command(), no state change at rest).
const dismissCard = useCommand(onDismiss); // one action, all modalities
const pan = Gesture.pan()
  .command(dismissCard)
  .onStart((e) => {
    'worklet';
    startY.value = translateY.value;
  })
  .onUpdate((e) => {
    'worklet';
    translateY.value = startY.value + e.translationY;
  })
  .onEnd((e) => {
    'worklet';
    if (Math.abs(e.velocityY) > 500 || Math.abs(translateY.value) > THRESHOLD) {
      // Commit: spring offscreen inheriting gesture velocity, and RETURN
      // the semantic verdict — this is the resolution contract. The
      // dismissal COMMAND does not ride this worklet — correctness-
      // bearing completions use the reliable semantic end event below.
      translateY.value = withSpring(OFFSCREEN_Y, { velocity: e.velocityY });
      return { resolution: 'commit' };
    }
    // Snap back — cancellation is a primary path, not an error.
    // Absent return = cancel.
    translateY.value = withSpring(0, { velocity: e.velocityY });
  })
  .activeOffsetY([-10, 10]); // Only activate after 10px vertical movement

return (
  // The semantic dispatch is FRAMEWORK-OWNED: on committed resolution,
  // Motion's terminal record (outcome log — at-least-once, idempotent
  // by identity) drives dispatch of the bound CommandRef, producing a
  // dispatch receipt that names the command, the gesture, and its
  // witness. onGestureEnd is OBSERVATIONAL (logging, analytics,
  // focus management) — it carries no semantic authority, so a
  // divergent callback cannot violate 0336's one-action-path rule.
  <GestureDetector
    gesture={pan}
    onGestureEnd={(e) => {
      logDismissGesture(e.committed, e.velocityY);
    }}
  >
    <View style={useAnimatedStyle(() => {
      'worklet';
      return {
        transform: [{ translateY: translateY.value }],
        opacity: opacity.value,
      };
    })}>
      {/* The visible WITNESS: the same CommandRef the gesture declares —
          gesture, button, and dispatched action are provably one action
          path (0336 R2); M8's witness fixtures assert this pairing. */}
      <CloseButton command={dismissCard} label="Dismiss" />
      {cardContent}
    </View>
  </GestureDetector>
);
```

**The resolution contract:** a gesture's semantic verdict is explicit —
the `onEnd` worklet returns it (`{resolution: 'commit' | 'cancel', …}`;
an absent return is `cancel`), and the verdict is **final when `onEnd`
returns** from the author's side (one-shot; anything later is a no-op
with a diagnostic) — system checkpoints may still force cancel after a
`commit` verdict (commit re-check, dispatch blockers — see the outcome
table). Motion carries the verdict on the terminal event — that is
where `e.committed` comes from; it is never inferred from which spring
the worklet happened to start. A terminal-edge return like this is
**edge allocation, not steady-state allocation** — it is legal under
the no-allocation rule, which governs the per-frame path; the M6 typed
ABI encodes the verdict as an enum, not a string.

**Command-shaped gestures declare their command — as a typed
reference.** A gesture that accelerates a command (swipe-to-dismiss,
flick-to-commit) is declared *with* its command binding: a
**registered `CommandRef`** — the same reference the visible signifier
binds — not a free string, so the gesture, the visible control, and
the dispatched action are provably one action path (0336 R2's
symbol-level witness identity and R3's every-modality-same-action
rule become enforceable). **The framework dispatches the CommandRef
on committed resolution** and emits a dispatch receipt naming command,
gesture, and witness; `onGestureEnd` is observational only. Motion
carries the identity on claims, receipts, and terminal events. The
bare low-level API (no command binding) is for **non-semantic visual
motion only**, and M6 ships with exactly that restriction until M8
wires command identities through the behavior/router surfaces —
including the M8 fixtures for missing, disabled, stale, and
conditionally-visible witnesses on both tiers. **"Registered" is
specific:** a `CommandRef` is minted by `useCommand` at component
mount and registered under **LLP 0336's command-instance identity
family** (action symbol + binding generation — the same records its
witness correlation reads; NOT a new registry, and unification with
the manifest/`declareIntent` surface remains 0336's open item, not
duplicated here); its lifetime is its component's (dispatch against an
unmounted ref is a no-op with a diagnostic); its identity rides
claims, receipts, and terminal events unchanged; its witness binding
is the visible control holding the SAME ref; and router/platform
commands are exempt exactly as wide as the platform-signifier
exemption (§Accessibility) — no wider.

(`runOnJS` remains available inside worklets and lowers to LLP 0297
§4.3's `scheduleOnAppRuntime` — async-only, never blocking, ordered per
source worklet, **bounded drop-oldest and therefore best-effort by
construction** (the shipped queue shape). It is for observations and
side effects that tolerate loss; it **must not carry correctness-bearing
commands**. Completions that change application state ride the semantic
terminal event, as above; built-in surfaces additionally correlate it
with the turn token — the LLP 0336 §6 handshake.)

**Three completion lifecycles, one substrate.** Who retains what through
settle differs by surface, and conflating them was the March draft's
error: (1) **router presentations** — RFC 0100's SceneLease retains both
scene layers until Motion reports rest (0100 §4b); (2) **the pager** —
the turn-token descriptor handshake pins the transform until the
re-centered tree applies (LLP 0336 §6); (3) **component-local
dismissal** — the behavior primitive owns retention: `<Presence>` keeps
the exiting subtree mounted until Motion's settle event (the M8
amendment to RFC 0007), and if reconciliation unmounts the target early
anyway, remaining motion on it cancels via stale-tolerance (defined
no-ops), never crashes. Until M8 lands that retention, component-local
*semantic* dismissal stays on the existing non-gestural paths — which is
the M6 restriction above.

#### Motion-domain representation

Recognizer state is main-owned Rust (LLP 0297 §4.4) — **not** the
`Kernel` struct, which main never touches. `kernel/src/motion.rs` (the
shipped pager slice) is the seed this generalizes.

```rust
pub struct PanRecognizer {
    id: GestureId,
    state: GestureState,
    /// Activation thresholds
    active_offset_x: Option<(f32, f32)>,
    active_offset_y: Option<(f32, f32)>,
    fail_offset_x: Option<(f32, f32)>,
    fail_offset_y: Option<(f32, f32)>,
    min_pointers: u8,
    max_pointers: u8,
    /// Tracking state
    start_position: Vec2,
    current_position: Vec2,
    velocity_tracker: VelocityTracker,
    /// Worklet callbacks — ids installed in the UI worklet runtime
    /// (LLP 0297 §4.3), invoked by the Motion domain
    on_start: Option<WorkletId>,
    on_update: Option<WorkletId>,
    on_end: Option<WorkletId>,
}

pub enum GestureState {
    Idle,
    Began,       // Touch received, not yet activated
    Active,      // Activation threshold met (arena lease granted)
    Ended,       // Touch released while active; resolution recorded
    Cancelled,   // Post-activation loss (see the outcome table)
    Failed,      // Pre-activation loss (see the outcome table)
}
```

**Terminal outcomes are phase-dependent** (aligning with LLP 0336 §6.1's
lease lifecycle — a "race loss" is not one thing):

| Phase × cause | Terminal state |
|---|---|
| Pre-activation: slop/eligibility loss, another candidate activates | `Failed` |
| Pre-activation: one-way standdown to scrolling | `Failed` |
| Active: structural invalidation (descriptor/reset generation, modal/profile change) | `Cancelled` (reason: structural) |
| Active: system/browser takeover | `Cancelled` (reason: external-owner) |
| Commit checkpoint: holder re-check fails | `Cancelled` (resolution forced to cancel) |
| Dispatch checkpoint: router/action blocker | `Cancelled` (reason: blocked; snapback) |
| Engine reset / epoch bump | `Cancelled` (reason: reset; no events after the fence) |
| Ordinary completion | `Ended` (with the worklet's resolution) |

Hit context and container bounds come from the main-side presenter
snapshot via `measure(nodeId)` (LLP 0297 §4.3) — never from the kernel
tree (`hit_test` there is a runtime-thread facility, not Motion's
geometry source).

### Gesture Composition

Gestures compose declaratively; **conflicts resolve in the arbitration
arena** (LLP 0336 §6.1 — the normative *contract*; the shipped code is
its typed seed, and the full lease lifecycle is an M4 obligation). The
arena is the policy-and-receipt authority — main-owned Motion on native,
the permanent recognizer on web — with typed, versioned records
(`InteractionProfile`, `InteractionClaim`, `InteractionArena`,
`InteractionLease`, `ArbitrationDecision`), stream topology frozen at
start, slop → axis-lock racing, **one exclusive lease per stream**, three
checkpoints (activation eligibility, commit re-check, dispatch
enforcement), **no mid-gesture revocation**, structural invalidation as
cancellation, and a bounded arbitration-receipt ring for diagnostics.

```typescript
// Authoring intent — combinator lowering is M5 design (OQ7):
const panAndPinch = Gesture.simultaneous(pan, pinch);
const swipeOrScroll = Gesture.exclusive(horizontalSwipe, verticalScroll);
const doubleTap = Gesture.sequence(firstTap, secondTap);
const swipeBackOrPanContent = Gesture.race(edgeSwipe, contentPan);
```

The composition API **compiles onto arena claims and leases**; it does
not add a second arbitration mechanism. **The record shape is decided
now, de-risking M2: a composition compiles to one externally-exclusive
compound claim.** Toward the arena, a composed gesture is a single
claim/lease holder; internally, a deterministic automaton owns the child
recognizers — `simultaneous` activates multiple children behind the one
external lease, `sequence` advances phases, `race` selects first
activation, `exclusive` applies ordered eligibility. This preserves
0336's one-lease invariant and gives receipts a stable shape.

The claim record distinguishes **compound identity from its per-stream
lease set**: one compound id (the externally visible holder) over a
stream set whose entries each carry their own lease — LLP 0336 grants
one exclusive lease *per stream*, so a compound over N streams holds N
leases behind one identity, which is how multi-stream acquisition
(`simultaneous(pan, pinch)`), partial acquisition/failure, and
sequence-phase membership changes fit without reinterpretation. Wire
discipline: **M2 reserves the compound identity and an extensible
layout only; the field-level stream-set schema lands with M5's
combinator automata** (matching LLP 0336's assignment of field schemas
to joint P3 work) — identity-reserved-then-proven, never frozen blind.
The **authoring surface follows the same gate**: no combinator ships
to authors before its automaton is proven against the four state
machines at M5 (and no compound claim reaches the arena before the
first-non-pager-use gate) — until then the public composition surface
is exactly the pairwise contract below, so reserving the identity
freezes a wire envelope, not semantics.

Honest scope: what is settled today is the arena's **pairwise contract**
— the v1 table's named contenders (router edge, pager regions, scroll
containers, system) with exclusive-lease semantics, which LLP 0336
describes as reducing this document's relationship model to
`Exclusive`/`Race` *over those named pairs*. **No general authorable
combinator has settled operational semantics yet** — not
`Simultaneous`/`Sequence`, and not general `Exclusive`/`Race` either
(activation priority among arbitrary siblings, declaration-order ties,
loser callbacks, and nesting are undesigned; the shipped arena's
`decide()` takes no composition relation today). The compound-claim
automaton's detailed semantics are OQ7 and land at M5. Gesture-to-gesture
handoff in v1 is the arena's own rule — **pre-lock standdown only, no
handoff after visible motion** — and the compound claim inherits it
unless OQ7 explicitly relaxes it internally. Same-axis nesting resolves
to the deepest candidate able to consume at activation; v1 diagnoses
(rather than silently accepts) a pager inside a pager and a same-axis
scroller between a pager and its claim region — a design commitment,
not shipped behavior: today's native slice diagnoses nested pagers
only, and the scroller case lands with the M4 arena completion.

### Animation Drivers

Animation drivers produce values over time on the display link. They
target SharedValues and run in the Motion domain.

```typescript
// Spring (default for gestures)
translateY.value = withSpring(targetValue, {
  damping: 15,
  stiffness: 150,
  mass: 1,
  velocity: gestureVelocity, // inherited from gesture, pt/s
});

// Timing (for non-interactive motion)
opacity.value = withTiming(0, {
  duration: 300,
  easing: Easing.bezier(0.25, 0.1, 0.25, 1),
});

// Decay (momentum after fling)
scrollOffset.value = withDecay({
  velocity: flingVelocity,          // pt/s
  deceleration: 0.998,              // per-millisecond decay base (see below)
  clamp: [0, maxScroll],
  rubberBandEffect: true,
  rubberBandFactor: 0.55,
});

// Sequence
translateY.value = withSequence(
  withTiming(20, { duration: 100 }),
  withSpring(0),
);

// Repeat
rotation.value = withRepeat(
  withTiming(2 * Math.PI, { duration: 1000, easing: Easing.linear }),
  -1, // infinite
  false, // don't reverse
);
```

#### Simulation contract (normative)

- **Timebase:** a monotonic clock; `dt` in seconds derived from
  display-link timestamps (never wall time). Units: points and seconds
  (`pt`, `pt/s`); decay's `deceleration` is the per-millisecond base
  (`v(t) = v₀ · deceleration^(t·1000)`) — a convention this document
  defines, chosen to match the number's common usage, not a documented
  platform contract.
- **Refresh-rate invariance by construction, not by dt-capping.** Springs
  and decay use **closed-form analytic solutions** where they exist
  (underdamped/critically damped/overdamped springs and exponential
  decay all have them) — the trajectory is then a pure function of
  elapsed time and cannot vary with refresh rate. Composed or
  non-analytic cases integrate on a **fixed simulation quantum with an
  accumulator** (e.g. 1/240s steps; each display frame consumes whole
  quanta and carries the remainder), so 60/80/90/120Hz displays sample
  the *same* trajectory at different points. A mere per-frame dt cap is
  not sufficient — 60Hz would integrate 1/60 steps while 120Hz integrates
  1/120 steps and diverges.
- **Gaps and suspensions:** driver time is true elapsed time (see §Clock
  ownership). Analytic drivers evaluate at the real elapsed point after
  any gap — safe by construction, no integration error — and may settle
  across it. Quantum-integrated drivers cap catch-up steps and re-anchor
  beyond the cap. The clamp is a numerical rule, never a time-stop;
  presentation stops while hidden but the trajectory does not.
- **Invariance is a registered check (M3, future):** identical config +
  initial velocity must produce position traces within tolerance across
  60/80/90/120Hz sampling and across hosts — the cross-platform driver
  conformance fixture, to be registered in `exact-verify.json` at M3.
  (The registry's existing `gesture-motion-fixtures` check is a
  marker-level fixtures check, not this.)
- **Rest:** a spring settles when |velocity| and |displacement| are under
  their rest thresholds; on settle the value snaps exactly to target and
  the driver emits its terminal event.

```rust
pub enum AnimationDriver {
    Spring(SpringDriver),
    Timing(TimingDriver),
    Decay(DecayDriver),
    Sequence(Vec<AnimationDriver>),
    Repeat {
        inner: Box<AnimationDriver>,
        count: i32,        // -1 = infinite
        reverse: bool,
    },
}

pub struct SpringDriver {
    target: f32,
    damping: f32,
    stiffness: f32,
    mass: f32,
    /// Simulation state (analytic solver: anchor time + initial
    /// conditions; the fields below are the sampled state)
    position: f32,
    velocity: f32,
    /// Settled when |velocity| < threshold and |position - target| < threshold
    rest_velocity_threshold: f32,
    rest_displacement_threshold: f32,
}

impl SpringDriver {
    /// Sample the closed-form spring solution at now = anchor + elapsed.
    /// (Illustrative; the underdamped case is
    /// x(t) = target + e^(-ζω t)·(A cos(ω_d t) + B sin(ω_d t)) with A, B
    /// from initial displacement/velocity. Retarget/velocity-inherit =
    /// re-anchor with current state as the new initial conditions.)
    pub fn sample(&mut self, elapsed: f32) -> bool {
        // …evaluate analytic form; update position/velocity…
        let settled = self.velocity.abs() < self.rest_velocity_threshold
            && (self.position - self.target).abs() < self.rest_displacement_threshold;
        if settled {
            self.position = self.target;
            self.velocity = 0.0;
        }
        settled
    }
}
```

### Animated Style Bindings

Animated styles map SharedValues to view properties without app JS
involvement.

```typescript
const animatedStyle = useAnimatedStyle(() => {
  'worklet';
  return {
    transform: [
      { translateY: translateY.value },
      { scale: interpolate(translateY.value, [0, 300], [1, 0.8], Extrapolation.CLAMP) },
    ],
    opacity: interpolate(translateY.value, [0, 300], [1, 0], Extrapolation.CLAMP),
  };
});
```

The object-literal form is the **authoring surface, not the runtime
representation**: at install time the style worklet's leaves are lowered
to per-property **output slots**, and steady-state evaluation writes
those slots — it does not construct objects or arrays per frame (the
no-allocation rule below). This lowering is M6 work and its design —
along with the typed invoke ABI — is a prerequisite for calling the
binding path allocation-free.

**v1 exposure status (2026-07-14, ENG-25009 decision):** the shipped
authoring contract is **direct sink bindings on `transform` and
`opacity` only** — Contract motion expressions bind straight onto those
two view attrs (`motion_presentation_sink_required` rejects everything
else), and no multi-property animated-style object is authorable on any
tier yet. The web engine's internal `WebAnimatedStyle` is
**renderer-private** plumbing for that lowering, not a public surface.
`useAnimatedStyle`'s multi-property object form (this section's example)
remains the design target for the React tier; exposing it — and widening
the sink set (backgroundColor and borderRadius, the Tier-1 four, are the
candidates) — is deliberate future work to be re-opened against a
concrete demand, not an accidental omission.

#### Execution (per display-link tick, in the Motion domain)

1. Ingest recognizer samples; run arena checkpoints as needed
2. Advance all active animation drivers (analytic sample or quantum
   accumulator)
3. Publish changed SharedValues to the slab (tagged-word CAS; write
   generations bump)
4. For each changed value, invoke dependent worklets in the UI worklet
   runtime (math-class; measured invoke p95 1.07µs)
5. Route resulting property writes by execution class: E2 presenter
   writes (transform/opacity, same frame), E3 island inputs (eligible
   subtrees), E4 constraint-buffer writes (arbitrary layout, consumed by
   the runtime thread at its next wake)
6. Emit coalesced observability records (E5) and any terminal semantic
   events

   **Consumed-generation provenance (r12 — the rule that makes 0313's
   defect retirement structural):** every E4 constraint record carries
   the matched `{slot, lifetimeGeneration, writeGeneration, value}`
   captured by MAIN at write time (main is the value plane's only
   writer, so the pair is exact by construction — no reconstruction,
   no value-matching), and the authoritative commit **echoes the
   consumed vector verbatim**; a prediction-driving slot missing from
   a commit's vector FAILS freshness for that commit (never
   skip-if-absent). The runtime layout pass never samples the slab
   for generations — provenance travels WITH the value. **E3 writes get
   the same authoritative catch-up (r13):** an island (E3) write is
   prediction-only on main; the SAME write's provenance-carrying E4
   constraint record is emitted alongside it, so the runtime's next
   ordinary pass recomputes the island authoritatively (0313's
   catch-up promise) with the echoed vector — E3 is a latency
   optimization over E4, never a substitute channel. M1 ships the
   record shape; M2's watermarks govern reuse on top of it; the
   ENG-24663 test list (A→B→A, commit-after-newer-write, absent
   slots, publication races, generation wrap) lands with M1.

**Propagation is topological and glitch-free per tick.** Dependencies
form a DAG captured at install time; each tick evaluates dirty worklets
in topological order, each at most once (a diamond does not double-run
its join). Derived-writes-derived cascades resolve within the same tick;
**where chain depth is statically derivable it is checked at install
time** (rejection with a diagnostic naming the chain), and the runtime
depth cap exists only for dynamically-formed chains (diagnostic beyond
it — a weaker guarantee, which is why install-time rejection is
preferred); cycles are rejected at install with a diagnostic naming the
cycle. (Reanimated's mapper-ordering history is the cautionary
precedent.) **Worklet failure policy, aligned with the threading
authority:** an exception, a non-finite output, an invalid verdict, or
a failed driver creation each cancel the affected binding/gesture
deterministically with a named diagnostic — never a silent skip. Budget
**overrun is diagnosed, not preempted**: per LLP 0297's failure
classes, `worklet_overrun` means the frame janks but everything keeps
running, and a truly wedged (non-terminating) worklet is a
watchdog-class **main-thread hang** — the shipped invoke path has no
interruption mechanism, and Release-mode interruption is gated on
LLP 0297 OQ1, which Motion inherits rather than overrides. This
propagation contract is normative; M6 implements it.

This is a tight main-thread loop with no allocations in the steady state.
The legacy JSON-in/JSON-out invoke path does not meet that target; the landed
M6 typed fixed-slot ABI does. Binding
attach/detach arrives via sidecar records with their owning commits
(attach-before-use, detach-before-destroy); presentation writes are
overrides on the presenter — the authoritative model tree is not mutated
per-frame, and settlement reconciles model truth through the ordinary
commit path with generation/token fencing so catch-up never fires a
Tier-1 transition.

<a id="typed-worklet-abi"></a>

### Worklets (compilation and runtime)

**Legacy authoring path** (LLP 0297 §4.3): the build plugin
(`createWorkletSourcePlugin`) extracts **top-level** functions carrying a
`'worklet'` directive as **source strings**; the host installs them into
the persistent UI worklet runtime via `install(id: String, source:
String, generation: UInt64)` → `ex_worklet_install(handle, id, source,
len, generation, out_error)`. Installs are generation-fenced (stale
generations are dropped atomically on reset — the fence-first rule). The
invoke ABI is JSON-in/JSON-out (`ex_worklet_invoke`), which allocates.

**M6 native seam (ENG-24704):** the parallel typed installer admits explicit
UTF-8 function-expression source plus finite scalar/boolean constants and
complete SharedValue handles, computes stable identity from content + capture
set, preserves each captured SharedValue's last live shadow after a stale
verdict, and invokes through caller-owned fixed f32 input/output slots. Typed
`runOnJS` uses a 256-record drop-oldest ring carrying source identity,
per-source sequence, and generation; the resident main-owned runtime drains
immutable copies and the production bridge enqueues callback execution on the
app-runtime executor with a reset-generation fence. This is the native/runtime
half delivered in r15. The r19 closeout subsequently landed the nested/inline
classification and output-slot authoring path; the top-level plugin remains a
compatibility route rather than the normative M6 path.

**M6 acceptance obligations (landed; retained as the implementation record):**

- **Nested/inline extraction with capture classification** — worklets
  written inline in hooks/gesture builders (the API examples above), with
  captures classified at build time: SharedValue handles and constants
  are legal (serialized as install-time captures); anything else is a
  compile error with a fix-it. The legacy plugin handles neither nested
  functions nor captures; the M6 compiler path does.
- **Stable callback identities** — re-renders must not reinstall or
  re-identify unchanged worklets (identity = content + capture set).
- **Install format** — **M6 decision: retain explicit UTF-8 source
  install.** The shipped plugin already emits the required function
  expression, while no restricted-worklet HBC + captured-constant artifact
  pipeline exists; 64 distinct warm installs measured p50 0.081ms / p95
  0.182ms against a 5ms mount-time ceiling. The ABI uses an explicit format
  enum and rejects unknown values, so HBC can be added later as a deliberate
  format if cold-mount evidence changes the trade.
- **Typed, allocation-free invoke ABI** — packed f32 argument/result
  records (or direct output-slot writes) replacing JSON both ways; the
  no-strings/no-allocation steady-state rule becomes enforceable only
  then.
- **`runOnJS` delivery contract** — generation-fenced, bounded queue with
  defined overflow behavior (drop-oldest + diagnostic), ordered per
  source worklet; plus the production **drain** wiring into the app
  runtime (**the M6 native production drain is now wired**; app-bundle
  callback registry generation remains authoring work). `runOnJS` is
  best-effort by construction and must
  not carry correctness-bearing commands — those ride the semantic
  terminal-event path (M2 transport, consumed by the M6 authoring
  surface).

Supported in worklets (the math class, LLP 0297 §4.6):
- Arithmetic, comparisons, boolean logic, conditionals, local variables
- `Math.*` functions
- `interpolate()`, `clamp()`, `Extrapolation`
- SharedValue reads/writes (`.value`) through validated handles
- Driver invocations (`withSpring`, `withTiming`, …) — main-local calls
- `runOnJS()` — the async best-effort escape (lowers to
  `scheduleOnAppRuntime`)
- `measure(nodeId)` — synchronous geometry from the presenter snapshot

Not supported (use `runOnJS` or the semantic event path):
- Object/string allocation in the steady state (no strings in the hot
  path; capture constants at install time)
- Closures capturing non-SharedValue mutable state
- Async/await, network, I/O, DOM/tree access

Input-filter worklets (the filter class — keyboard/text filtering with
accept/reject/replace verdicts) share the runtime with their own
eligibility rules (LLP 0297 §4.6). Contract's compiled projections are
*intended* to carry provable eligibility — that compilation design is
open work under LLP 0297 OQ6 / LLP 0281, not shipped. This RFC adds no
third class.

### Velocity Tracking

Accurate velocity is critical for natural-feeling gesture completion.
Motion uses a least-squares estimator over recent samples, weighted
toward recency — the same family as Android's documented least-squares
(LSQ2) `VelocityTracker` strategy; the exact algorithm is chosen at M4
against recorded traces. iOS's estimator is not public, so parity there
is validated against the LLP 0336 gesture-trace corpus, not claimed by
construction.

```rust
pub struct VelocityTracker {
    /// Recent pointer samples (position in pt, timestamp from the
    /// monotonic timebase). Illustrative sketch — a ring buffer of
    /// ~20 samples / ~100ms, not a container-crate commitment.
    samples: RingBuffer<Sample>,
}

impl VelocityTracker {
    pub fn add_sample(&mut self, position: Vec2, timestamp: Instant) { /* … */ }

    /// Least-squares fit over the last ~100ms, recency-weighted,
    /// output in pt/s.
    pub fn velocity(&self) -> Vec2 { /* … */ }
}
```

The shipped pager slice uses a simpler estimator; when the general
tracker absorbs it (LLP 0336 P3), commit thresholds are re-validated
against the recorded trace corpus so feel does not drift (OQ11).

#### Sample-stream fidelity and input-cadence robustness (r22)

Two guardrails on everything that consumes raw gesture samples —
velocity estimation, commit/fling thresholds, direction classification,
and the LLP 0336 trace corpus:

- **No early collapse of per-axis streams.** The pointer sample stream
  keeps full per-axis positions with monotonic timestamps end to end;
  derived scalars (vector magnitude, axis verdicts, direction) are
  computed by the consumer and never replace the sample stream anywhere
  in the pipeline or in recorded traces. Information collapsed at
  ingestion cannot be recovered downstream. (External corroboration
  from an adjacent field: [Inertia-1](https://yang-ai-lab.github.io/Inertia-1/),
  a 2026 wearable-motion foundation-model study, found full triaxial
  time-domain input consistently outperforms collapsed
  vector-magnitude summaries — same lesson, different domain.)
- **Input-cadence robustness is distinct from display-refresh
  invariance.** The simulation contract above makes *driver output* a
  pure function of elapsed time; this guardrail covers the *input*
  side. Estimators and thresholds are functions of (position,
  timestamp) samples — never sample counts or an assumed event rate —
  and must hold across real cadences: 60/120Hz touch, coalesced
  trackpad wheel deltas, browser-throttled pointermoves, and synthetic
  Acto sequences. The trace corpus therefore records input device
  class and event cadence per trace and spans them (the corpus-side
  requirement lives in LLP 0336 §Acceptance); the M3 invariance check
  covers display Hz, not this.

### Rubber-Banding and Boundary Physics

Pull-to-refresh, scroll bounce, and overscroll effects use Motion-level
rubber-band physics:

```rust
/// Sign-safe rubber-band: operates on |offset| and reapplies the sign,
/// so overscroll behaves identically in both directions.
pub fn rubber_band(offset: f32, limit: f32, factor: f32) -> f32 {
    let sign = offset.signum();
    let x = offset.abs();
    let c = factor; // 0.55 default — a trace-fitted profile, not magic
    let d = limit.max(f32::EPSILON);
    sign * (1.0 - (1.0 / (x * c / d + 1.0))) * d
}
```

This is the widely circulated rubber-band curve associated with
UIScrollView — associated, not documented: Apple publishes neither the
formula nor the constants. Platform *feel* is also more than the curve —
sampling cadence, deceleration handoff, and arbitration all contribute.
So cross-platform parity is validated by replaying recorded traces
through the conformance fixtures, and the constants ship as **versioned,
trace-fitted profiles** (recording device class, OS, input class, and
fitting error) rather than one hardcoded truth.

### Integration with Existing Systems

#### Tier 1 Transitions

Tier 1 (`style.transition`) and Tier 2 (animated values) operate on the
same style properties through different mechanisms. Rules:

- If a property has both a Tier 1 transition and a Tier 2 animated
  binding, **Tier 2 wins while the binding is installed**: the binding
  writes the presentation value directly and the presenter suppresses
  the transition for that property (the property is motion-owned).
- When a Tier 2 driver settles, control returns to Tier 1. Settlement
  reconciles the model value through the ordinary commit path with
  generation/token fencing, so the catch-up commit does **not** fire a
  Tier-1 transition (no double animation, no visual jump). The remaining
  design nuance — exact presenter ownership at the handoff frame across
  the AppKit/UIKit presenter trees — is OQ5.
- Continuously tracked or velocity-bearing gesture motion is always
  Tier 2 (a discrete tap that merely toggles state is not "gesture
  motion"); state-driven motion defaults to Tier 1 and escalates to
  Tier 2 by capability (see §"Routing is by capability" above), never
  silently.

#### Behavior Primitives (RFC 0007 — proposed amendments)

RFC 0007 deliberately defers gesture composition to a future gesture RFC
— this document — and its implemented `usePresence(present: boolean)`
contract has no gesture parameter, so these are **proposed amendments to
0007**, to be applied there when Motion lands, not descriptions of
current behavior.

Dismissal has **two regimes** (LLP 0336 §5), and the amendment targets
the second:

- **Router-owned presentations** (routes, router-presented sheets):
  back/dismiss is a router command bound once per surface — interactive
  swipe-dismiss of these is RFC 0100 ground, not a component prop.
- **Component-local layers** (`DismissableLayer`, and Contract's
  `dismissable` semantic primitives arriving with 0336 P4):
  `dismissOnSwipeDown` here means a Motion pan recognizer whose commit
  fires the layer's own dismiss command — with a witnessed signifier per
  0336 R1/R2 — after which the `<Presence>` exit runs under the
  two-phase settle rule. The amendment must also define the
  **topmost-layer claim**: today each React `DismissableLayer` instance
  installs its own handlers with no shared stack, so "which layer owns
  the swipe" needs an owner before the gesture does. **That owner is
  named: RFC 0007's M8 amendment (under LLP 0336 §5's dismissal
  governance) defines the per-root layer-stack schema**, and its
  acceptance list is enumerated here so the amendment cannot
  under-specify: stable layer identity, visual/topmost ordering, modal
  and portal semantics, teardown fencing (early unmount = structural
  cancellation), Contract/React framework parity, projection of the
  topmost claim into the arena, and receipt coverage — with
  nested-layer and early-unmount tests.
- **usePresence**: gains a gesture-progress input for gesture-driven
  enter/exit (interactive sheet presentation where the sheet follows the
  finger), defined so that cancellation returns cleanly to the previous
  presence phase.

#### Interactive navigation (RFC 0100)

RFC 0100 owns interactive navigation transitions end-to-end — gesture
readiness published **ahead of need as two generation-stamped halves**
(the router-published `BackDisposition` and the main-owned
presenter-readiness half, plus the presenter's scene lease — deliberately
unfused, because the two halves have different owners; 0100 §4a),
two-phase resolution (logical commit vs. visual settle, with presenters
retaining both scene layers until rest; 0100 §4b), the three progress
channels (Motion hot path / presenter writes / coalesced observability;
0100 §4c), and the screen-level configuration knobs (whose spelling
belongs to RFC 0010's `ScreenOptions`). This document supplies 0100's
substrate and nothing above it:

- edge-swipe recognition as arena claims (router edge zones are never
  claimable by app regions)
- SharedValue-backed progress the presets consume
- velocity-inheriting springs for commit/cancel settling, with settle
  reporting the presenter needs to release retained layers
- generation-fenced cancellation (cancel-with-snapback on stale
  readiness, structural invalidation mid-gesture)

Shared-element transitions are 0100's design ground (with RFC 0046);
Motion contributes value channels and drivers, not the orchestration.

#### Agent API

Gesture and animation state are observable through the existing agent
surface. Today: synthetic input rides `exact_gesture` /
`exact_touch_sequence` / `exact_pointer_sequence`, and `POST /agent/wait`
exists for condition waits. Proposed Motion additions (illustrative
shapes, allocated/named through the agent-operations authority at
implementation time):

- gesture/animation state in tree and diagnostics payloads
  (`node.gestures: [{type, state, translation…}]`,
  `node.animations: [{property, driver, settled}]`)
- wait conditions: `animationSettled`, `gestureState`
- the arbitration-receipt ring (LLP 0336 §6.1 diagnostics): "what
  happened to that gesture?" answered with contenders, eligibility,
  winner, rule codes, and terminal outcome
- **geometry honesty during motion**: agent geometry queries declare
  their source. LLP 0313 shipped `geometry: predicted | model` (an
  island's *predicted next-commit layout* vs. model truth); Motion adds
  `presented` (*what is on glass now*, Motion overrides folded in) —
  three distinct answers, unified at implementation time as one enum
  (`model | predicted | presented`) on the agent surface so consumers
  see a single vocabulary. This receives LLP 0297 OQ8 (mid-gesture
  observability: presented frames for targeting, model frames for layout
  truth; accessibility frames stay model-only during motion per 0313's
  conservative rule).

Two postures, per LLP 0336: agents normally exercise UI through
**witnessed semantic commands** (the same affordances users see);
synthetic pointer sequences are the *mechanics-test* path for the
recognizers themselves — the Motion domain processes synthetic samples
identically to real ones.

#### Accessibility (RFC 0074, LLP 0336)

Two distinct rules, matching LLP 0336's scoping — conflating them
produces requirements that are silly for one class or too weak for the
other:

- **Command-shaped gestures** (accelerators: flick-to-commit,
  swipe-to-dismiss, edge-back, swipe actions) follow 0336 R1/R2: the
  command must also be invocable through a **visible, witnessed
  signifier**, and app-authored gesture bindings carry their witness.
  Router/platform-owned gestures satisfy the rule through
  **platform-owned signifiers** — host or browser chrome — per 0336's
  own exemption class, not through a same-component witness; but the
  exemption is exactly as wide as 0336 draws it: the mounted
  interaction snapshot is *evidence about* a signifier, not a
  signifier, and where platform chrome is absent (standalone PWA, iOS
  sheets) an **app-declared visible back/close control is still
  required**. RFC 0074's custom accessibility
  actions are the assistive-tech supplement (rotor/menu operations),
  not a substitute for the visible signifier.
- **Continuous direct manipulation** (dragging content, pinch-zoom,
  scrubbing) is not a command and is not covered by the witness rule; it
  requires an **accessible, path-free alternative** to reach the same
  end state (single-pointer/assistive route — e.g. accessibility actions
  or increment/decrement semantics), per 0336's carve-out and standard
  pointer-accessibility practice.

**Reduced motion is per interaction class.** LLP 0074's
§Reduced-motion-interaction-class-profile is now the single normative class
table and data authority, adopted at M8 per OQ9. Motion consumes
`resolveMotionReducedMotion` from `@exact/core/motion/reduced-motion`;
presenters use its generated Swift projection; the permanent web recognizer
uses the same TypeScript resolver. No consumer may recreate the class switch
or replace it with a duration multiplier. RFC 0046/0100 remain the owners of
the mandatory 150ms navigation fade semantics cited by that profile.
Activation-threshold scaling for motor accessibility is **open design
work with RFC 0074**, not a v1 claim: platform-level accommodations
(e.g. iOS touch accommodations) apply upstream of app recognizers
already, and no cross-platform "activation distance" API exists to bind
to today.

### Scroll Integration

Scroll position becomes a SharedValue, enabling scroll-linked animations
without app JS:

```typescript
const scrollY = useScrollSharedValue(scrollViewRef);

const headerStyle = useAnimatedStyle(() => {
  'worklet';
  return {
    transform: [
      { translateY: interpolate(scrollY.value, [0, 200], [0, -100], Extrapolation.CLAMP) },
    ],
    opacity: interpolate(scrollY.value, [0, 200], [1, 0], Extrapolation.CLAMP),
  };
});
```

**Pre-M7 baseline (retained for provenance):** scroll *events* existed
(`EventType::Scroll = 13`, riding the host-ops channel by design), Contract
had pointer channels (LLP 0281 stage 0), and the web recognizer handled wheel
internally (LLP 0336 P1), but no host had a generalized
scroll→SharedValue binding. M7 landed the design below: the presenter-side
scroll owner publishes the offset into a
slab slot every frame during active scrolls (a scroll binding descriptor
names the container and the slot, generation-fenced like any binding);
dependent bindings update in the same display-link tick on main. No
app-JS round-trip. Scroll-driven layout obeys the execution classes like
everything else (E2 first; E3 islands where eligible — LLP 0313's
feed-backed islands were built for exactly this shape; E4 otherwise).

The reserved-but-unlowered event spellings `scrollend` and `refresh`
(ENG-23128) acquire their semantics here, with scroll bindings (M7) —
`scrollend` as the settle edge of the scroll value, `refresh` as the
committed pull-to-refresh gesture — so no parallel design happens
elsewhere.

### Pointer-channel records

The OQ6 seam says a Contract transient binding is the language projection
of the Motion value substrate — but a stage-0 **pointer channel does not
lower to one scalar slot**, and its native record must preserve the
shipped stage-0 semantics exactly (`channels.ts`, normative):

- **The sealed sample** is `PointerSample` alone — element-local x/y,
  `pointerId`, `pointerType`, `buttons`, optional `pressure`. Edge
  sampling seals exactly this record (one immutable final-generation
  copy), nothing more.
- **The envelope** carries `phase` and the element box, held beside the
  sample — the shipped implementation stores `lastPhase`/`lastElement`
  separately and discards the envelope timestamp. The element box is
  **not** static descriptor metadata: it refreshes with each delivered
  envelope. And the shipped semantics have **two cadences the native
  record must preserve**: envelope fields update at *raw intake*, while
  the channel `generation` advances only when the *coalesced sample
  applies* — so the envelope can be newer than the applied sample
  between intake and frame flush. The native form therefore carries two
  sequences: an **envelope-delivery sequence** (intake cadence) and the
  **applied-sample generation** (apply cadence); `seal()` and
  programmatic publication observe *applied* state. M6 ports the
  characterization tests for this skew and adds cross-thread torn-read
  fixtures. The intern table for unknown pointer kinds is
  **epoch-scoped** (cleared on reset), so long dev sessions do not leak
  interned strings.

The native form mirrors this split: a fixed-layout multi-word **sample
record** (numeric fields plus a `pointerType` encoding that **preserves
unknown kinds** — the shipped TS type is an open `string`, so the
record carries a kind enum for the known vocabulary plus an interned
raw-string index when the kind is `other`, round-tripping the original
value rather than losing it; `pressure` carries an explicit **presence
bit**, preserving stage-0 optionality rather than smuggling absence as
a number) under one
record-level sequence/generation providing the seqlock read and the
sealed edge, with envelope fields (phase enum, element box) as adjacent
per-delivery words outside the sealed unit. One channel = one record
bundle (+ its descriptor). The author-bindable vocabulary covers all six
shipped pointer attrs (`pointerdown/move/up/cancel/enter/leave`). M6
owns this layout and must pass conformance against `channels.ts` stage-0
behavior (cadence, edge sampling, confinement, disposal, intake guard)
by porting its characterization tests.

## Platform Execution

### Apple (iOS / macOS) — the reference implementation

- **Clock:** view-scoped `CADisplayLink` on macOS (via
  `NSView.displayLink(target:selector:)`); iOS today creates a per-host
  `CADisplayLink` on demand and requests a 120Hz `CAFrameRateRange`; the
  M3 MotionClock registry unifies these (§Clock ownership). High-refresh
  presentation on iPhone additionally requires the ProMotion plist
  opt-in (`CADisableMinimumFrameDurationOnPhone`) — **not yet set** in
  the iOS host or `exact new` templates; adding it is part of M3's
  acceptance.
- **Intake:** platform recognizers as sample adapters (the hybrid
  boundary above); `NSEvent` scroll phases on macOS.
- **Apply:** presenter-tree writes; the AppKit/UIKit presenter trees
  already animate Tier-1 transitions via
  `CABasicAnimation`/`CASpringAnimation`, so the "CALayer-backed
  application" target is partially met — Motion's per-frame writes use
  the same layer-backed path without implicit animations.
- **Threading:** macOS is the runtime-thread default-on host (LLP 0322)
  and therefore the first native conformance target; iOS promotes when
  its 0322 gate (ENG-23520 — met 2026-07-23, iOS default-on) and the
  island **producer/clock/host-op integration** (ENG-23516's remainder —
  the presenter adapter itself has landed) land.

### Web — permanent TypeScript implementation

Web is a v1 target, not a deferral. The main thread is the UI thread, so
Motion's roles collapse onto one thread with the same semantics:

- **Value store:** a `Float32Array` slab (f32, matching native precision
  bit-for-bit at solver boundaries) owned by the web Motion runtime,
  with the same handle/generation/epoch validation and write-generation
  rules as the native slab (the same M1 lifetime tests run against it).
- **Recognition:** the shipped permanent recognizer (LLP 0336 P1) —
  Pointer Events with `touch-action` intersection,
  `pointercancel`-as-loss, wheel with capacity checks. The arena
  contract is the same; the recognizer is its web authority — and the
  §6.1 lifecycle it must implement is M4's web leg, not assumed done.
- **Drivers:** the same **analytic solver math, shared by specification**
  — closed-form solutions make TS/Rust parity a float-tolerance
  question, checked by running the same trace corpus on both (M3's
  cross-host leg). rAF is the tick.
- **Worklets:** plain functions on main — no restricted runtime is
  needed where app JS already shares the thread; the `'worklet'`
  directive is a portability boundary (the same code must stay in the
  math class so it can run natively), enforced by the same build plugin,
  not a separate execution model.
- **Apply:** direct style/transform writes in the rAF tick (the
  dom-mirror precedent). **WAAPI** is an optimization for
  fire-and-forget timing curves on compositor properties; springs/decay
  are either rAF-driven or pre-sampled to keyframes — WAAPI has no
  velocity-continuous retarget primitive (an inference from the
  standardized timing/keyframe surface, not an explicit spec statement
  — labeled as such), so interactive/retargetable values stay on rAF.
  Eligibility matrix detail is OQ4.
- **Degradation:** under main-thread contention web tracking degrades
  with the page (there is no second thread to hide in); the framework's
  job is to keep its own tick cheap. OffscreenCanvas does not help —
  real-DOM mutation cannot leave the main thread.
- **Drift control:** shared-by-spec analytic math + shared trace corpus;
  the WASM recognizer/driver core stays the recorded fallback
  (§Alternatives) if drift becomes a real maintenance cost.

### Android — deferred behind gates (OQ10)

Tier-1 parity first (the renderer currently skips `SetTransition`);
then `MotionEvent` intake and `Choreographer`-driven ticking (its
callback runs on the Looper the Choreographer is attached to and
supplies the frame time), with
direct property writes for Rust-stepped values (`ViewPropertyAnimator`
is for renderer-owned effects, not per-frame external writes). Motion's
Rust core is platform-neutral by construction; this is host-adapter
work.

### Windows — deferred behind gates (OQ10)

The current host handles `WM_LBUTTONDOWN`/`WM_MOUSEWHEEL` only and the
scene projection carries no transition state. The gate list:
`WM_POINTER`-family intake (touch/pen/precision-trackpad — noting that
precision-touchpad pointer input additionally requires Windows 11's
touchpad-capable window/thread registration, without which trackpad
input arrives wheel-shaped, and Microsoft's guidance points at
Interaction Context or explicit preservation of system touchpad
gestures), Tier-1 transition support in the scene projection, and a
compositor-clock tick source. Until then Windows is explicitly out of
Motion acceptance.

## Protocol and Transport

The March draft proposed concrete opcode and event numbers; those tables
are **withdrawn** — their opcode block and first event values collided
with values since assigned (`0x20–0x23` are
`SetOpacity`/`SetTransform`/`SetBgColor`/`SetPagerBindings`; event
values 13/14 are `Scroll`/`PagerCommit`; the March table's events 15–17
remain unassigned — the tables were stale, not fully consumed), and
LLP 0336 already records the collision. The normative posture:

- **Inventory-first, always.** Every Motion identity (opcodes, event
  types, prop ids) is allocated through
  `tests/protocol/protocol-inventory.json` at implementation time, per
  the `protocol-abi` row in `exact-contracts.json`. This document names
  record classes, directions, and semantics — never numbers.
- **Pre-1.0 ABI fluidity (author directive, Charlie, 2026-07-11).**
  Exact has **no external protocol consumers yet**, so shipped numeric
  values are renumberable at will: the inventory is the **allocation
  authority for coordination** — its real job is preventing concurrent
  internal workstreams from double-claiming values, which is exactly how
  this document's March tables went stale — not a compatibility freeze.
  If Motion's implementation wants a coherent contiguous block (for
  example, formalizing the de-facto motion/patch neighborhood at
  `0x20+`, where the compositor-patch ops and the first Motion
  descriptor already sit, or grouping records by *consuming domain* now
  that the two-domain split makes that meaningful for dispatch), **M2
  may perform a coordinated renumber**: change the inventory, regenerate
  every consumer (`opcodes.generated.ts`, kernel enums, host decoders,
  fixtures) in one change. Internal version skew is still real without
  external consumers — a native host, dev server, cached bundle, or
  stale HBC can disagree about meanings — and a bare version bump is
  **not sufficient**: today's decoder uses one global,
  version-independent opcode map (the opcode decodes before any
  version-specific semantics apply), so a stale v3 frame would decode
  successfully with new meanings if v1–v3 remained accepted. The
  concrete guard: **a schema digest generated from the inventory,
  bound to every producer, checked at handshake before dispatch;
  renumbered sessions reject pre-renumber producers outright; cached
  bundles/HBC are invalidated with the digest; and M2 ships a
  mixed-version startup test** (old host + new dev server, and vice
  versa). Digest inputs are normalized — field order fixed,
  prose/description fields excluded — so "generated from the inventory"
  is deterministic across regenerations. Prose exclusion would leave
  payload LAYOUT unguarded (today's inventory describes payloads in
  prose), so every correctness-bearing Motion record class carries an
  explicit **`layoutVersion`** — a structured inventory field, a
  digest input, bumped on any byte-shape change (a same-opcode field
  insertion like 0336's pending `centerRevision` is exactly the
  guarded case). M2's startup matrix therefore includes
  **mixed-layout tests** (old host + new producer differing only in a
  layout bump, and vice versa), not just mixed-numbering; and
  byte-level golden fixtures plus schema assertions **generated from
  the layout-version table** guard the hand-authored codecs. Now is the cheapest this
  will ever be; the decision belongs to M2 with implementation facts
  (record count, OQ8's snapshot-vs-patch outcome), not to this document
  by default conservatism. This fluidity expires at first external
  release.
- **Record classes and their planes** (the two-domain transport rule —
  which plane a record rides is part of its design, not an
  implementation detail):

| Record class | Direction | Plane | Semantics |
|---|---|---|---|
| Motion graph descriptors (recognizer attachments, compound claims, bindings, scroll bindings, driver configs) | runtime → main | **commit sidecar — protocol-ingress records assembled runtime-side, crossing runtime→main on the typed commit-output publisher** (two transports, one rule: see the two-transports bullet below) — transaction-scoped `{epoch, sequence, rootId}` (the sequence is the per-root `motionSeq` clock — see the one-clock bullet), generation-fenced. **Validation is runtime-side, before emission:** the runtime thread produces both the tree ops and the sidecar, so structural + semantic validation of the descriptor set runs there, *before the transaction becomes authoritative* — an invalid sidecar rejects the whole tree+Motion transaction at the only point where atomic rejection is implementable (main cannot retroactively reject runtime-side tree mutation). Main-side decode failure is therefore always transport corruption (fail closed, whole frame). Application on main is **two-phase around the presenter mutation** — the only mutation Motion can be ordered against, since the authoritative kernel tree has already moved on the runtime thread: the **detach phase** (ids-only records for nodes the transaction destroys or re-targets) applies *before* main's presenter application, the **attach phase** (new bindings/claims/drivers) applies *after* presenter output — the shipped pager precedent is the attach phase. **Staged sidecars are epoch-fenced as whole no-ops** (an epoch bump between staging and presenter publication drops the sidecar entirely; the affected gestures and drivers transition to Cancelled(reset) with **no post-fence records** — reset cancellation is observed as the epoch bump, never as delivered cancel records, since the log/cursor/dedup clear together) — deliberately *stricter* than the shipped pipeline's completions-run-on-stale-discard behavior, which Motion descriptors do not inherit | The favored shape is a **whole-root snapshot record** in the `SetPagerBindings` mold — descriptor-set replace is atomic under reset/HMR — with small imperative patches only if snapshot size measurably hurts (OQ8). Snapshots respect the protocol's u16 op-payload ceiling: a set that exceeds it ships as sequence-numbered chunks under a transaction envelope (snapshot id, chunk index/count, total size, digest) — **staged and reassembled runtime-side, pre-emission**, so the emitted frame always carries one complete validated set — or the M2 size fixture proves real apps never approach the ceiling and a hard cap + diagnostic is recorded instead (OQ8) |
| Value commands (imperative app-JS writes, app-JS-initiated driver start/cancel) | runtime → main | command records on the ingress + typed crossing (**one per-root monotonic `motionSeq`** — allocated at write issue, applied in issue order) → tagged-word slab publish **by the Motion domain (main) at crossing-consume** — main is the single writer for command-path publishes | Stale-generation targets are no-ops. **App-runtime-initiated only** — worklet-initiated driver commands (`withSpring` in a worklet) are main-local Motion-domain calls and never cross domains |
| Per-frame values | written by main ONLY; read by both domains (the value plane's single-writer rule — §Registry) | SharedValue slab (tagged atomics; seqlock record bundles for multi-word, single writer per record) | Never events, never the crossing |
| Semantic lifecycle events (reliable gesture activation + resolution, pager turns, driver settled) | main → runtime | A per-root **acknowledged outcome log** (SPSC: main is the single producer, the app runtime the single consumer). A gesture atomically reserves two credits before platform recognition can activate. After the arena's real lease grant, main appends semantic `MotionGestureStart` `(streamId, 0)`; exactly one `MotionGestureEnd`/resolution `(streamId, 1)` follows. The log enforces start-before-resolution. This semantic start is not a raw recognizer sample; raw down/move/up/cancel phases remain Motion-internal and loss-tolerant. A driver reserves one terminal credit. The runtime applies a record and advances its runtime→main consumed cursor; main reclaims only below that cursor. The main→runtime event queue carries level-triggered wakes, not record payloads | **At-least-once delivery + idempotent consumption.** A replay between apply and cursor advance is suppressed by the epoch-scoped dedup table. Identities per class are gesture `(streamId, phaseSeq)`, driver `(valueHandle, driverSeq)`, and pager `(rootId, turnToken)`. Reset uses the asynchronous transport sub-barrier above: predecessor delivery is fenced before logs, cursors, reservations, and dedup state are cleared. The pager re-center acknowledgment remains a token-bearing changed descriptor on the runtime→main commit sidecar, not a duplicate outcome record. Correctness-bearing lifecycles never ride `runOnJS`. Capacity covers ambient driver credits, one navigation three-credit bundle, gesture two-credit bundles, the permanent blocked-attempt latch, and headroom; admission fails before activation rather than dropping a record |
| **Interactive-navigation lifecycle records** (RFC 0100: start notification; ack/nack wakes) | start: main → runtime; ack/nack: runtime → main | Start rides the **outcome log as a non-terminal lifecycle record** — the log's per-root order makes start-before-resolution **structural**. **Admission is a per-transition credit BUNDLE reserved atomically at arena activation** (slop/axis-lock lease grant — the ONLY activation boundary; everything earlier is RAII prewarm): start + gesture resolution + the settle driver's terminal record — the transition's settle spring IS an ordinary logged driver whose `settled` record is what releases retention, so its credit is reserved up front with the others (three slots, never discovered short mid-flight). Ack/nack are **command records on the ingress + typed crossing**, ordered under the same per-root `motionSeq` as every other command | Identities: start `(rootId, transitionId)`; ack/nack `(rootId, transitionId, verdict)`, deduplicated by identity. **Authorization never rides these records** — they are level-triggered WAKES over the durable DecisionCell (decision-records row below), whose state machine is closed (every outcome incl. structural nacks is a cell state, r12): main re-reads the cell word on wake AND at its scheduled loads (release, `settled-visual` entry, timeout), so a lost or duplicated wake is latency by construction, never correctness — and accepted command records on the typed crossing are **structurally lossless** anyway (the staging queue never drops; reset clears it whole under the reset-cancel rules, which is cancellation, not loss). Terminology bridge for 0100's "exactly-once": exactly-one **authorization** (the cell) + at-least-once record transport with deduplicated effect |
| **Blocked-attempt records** (RFC 0100 outcome (b-blocker): the intent-threshold crossing that must surface blocker UI exactly once — CT-061-08/14) | main → runtime | the outcome log, as a **per-root single-slot latch**: at most ONE outstanding blocked-attempt record per root — repeat crossings while one is outstanding coalesce into it — the UX contract is **exactly one surfaced dialog per attempt-BURST** (crossings before the runtime consumes the latch are one burst; two rapid attempts produce one dialog, which is the correct UX — CT-061-08/14's "exactly once" reads per-burst, and 0100 words it identically), never a count, and the latch re-arms when the runtime consumes the record. No arena lease and no activation exist for a blocked attempt, so its slot is NOT an activation credit: the latch is a permanent per-root reservation, made at root setup with the reserved progress slot | Identity `(rootId, attemptSeq)` — attemptSeq is per-root monotonic, minted at latch set; deduplicated by identity; epoch-fenced with the log (reset clears the latch; no post-fence records, matching the reset-cancel rules). Passive by construction: the record reports the crossing; it claims nothing and cancels nothing |
| **Applied watermarks** (distinct from the consumed cursor: opposite direction, latest-wins) | main → runtime | **two per-root latest-wins atomics in the control-record family** beside the consumed cursor and the DecisionCell (single writer: main; the runtime reads wait-free) — deliberately NOT records on the log: latest-wins mutation and an immutable, cursor-reclaimed log don't mix, so mutable watermarks get control words and the log stays append-only. **Deployment status (r23, from the LLP 0411 OQ5 audit): this row is the mechanism record, not the deployment record.** The control words — and the wait-free runtime read they exist for — are normative and **not built yet**: as landed, both watermarks are per-root `HashMap` entries in the slab's writer-owned management state, the one exported reader (`exact_shared_value_applied_command_watermark`, `kernel/src/ffi.rs`) acquires the management mutex and has no non-test caller, and the `EpochSequenceWatermark` seqlock pair (`kernel/src/motion_transport.rs`) is test-only. Until `issues/20260727-sharedvalue-applied-watermark-control-words.md` lands (it owns the packed-word representation question), no reader may assume watermark loads are wait-free — LLP 0411 §5.1's liveness-probe hazard is exactly this mistake | Values: highest applied sidecar `motionSeq`; highest applied app-command `motionSeq` — **epoch-qualified** comparisons (an epoch bump resets the sequence space). The sidecar watermark gates slot reuse (§Registry); the applied-command watermark is the app-write shadow's clearing observable (§SharedValue) |
| **Decision records** (RFC 0100's `DecisionCell`: one root-lifetime commit-authorization word per root, re-armed per in-flight transition) | both — **the sole record class whose semantics require two writers racing on one word**: the state machine is CLOSED — every authorization outcome is a durable cell state: the runtime CASes `pending → confirmedAllow` (authorized), `pending → confirmedCancel` (policy forced-cancel), or `pending → nacked` (structural invalidation — revision mismatch, MATCH divergence; r12: nacks are cell states, not message-only outcomes), any of which races main's `pending → expired` (timeout); exactly one writer wins, and the winning CAS is THE cross-domain linearization point. The logical commit transaction asserts `confirmedAllow` specifically. **The cell is the durable decision; ack/nack records are level-triggered wakes over it, never the decision** — and the gloss is now literally true for every path, because every path writes the cell. Progress is additionally **wake-independent**: main loads the cell at release processing, at `settled-visual` entry, and at timeout, so a lost or duplicated wake costs only latency bounded by the next scheduled load, by construction (RFC 0100 §4a owns the state machine, timer origin, and lease semantics; this row is the plane assignment and writer topology 0100 consumes) | a **control word beside the outcome log — deliberately NOT a §4.5 slab slot** (the value plane stays single-writer main-side; the slab FFI exposes no runtime-side CAS accessor and gains none). Runtime-allocated at root setup, root-lifetime — and the **control region follows the value plane's physical-lifetime rules exactly**: stable addresses; root teardown TOMBSTONES the cell under a root-instance-qualified generation (never frees); physical release only at engine shutdown behind the same cross-domain quiescence barrier; and re-tenancy of a control word by a NEW root is gated on the **teardown watermark — the applied-sidecar watermark of the teardown transaction** (the tombstone rides the teardown commit's sidecar detach phase and is CASed by MAIN when that phase applies, the same writer/ordering as every detach) — so a runtime CAS paused across root destroy/recreate lands on a tombstone and no-ops, never on a successor root's cell | The word packs `(rootInstance, transitionGeneration, state)` so a late CAS from transition N fails against the cell re-armed for transition N+1, AND a late CAS from a destroyed root fails against any successor — ABA-proof across both re-arm and root recreate. All CASes are AcqRel, and the logical commit is ordered after the winning CAS. **Atomic representation (r12):** one `AtomicU64` (a hard lock-free requirement on every tier-1 target — the control region asserts 8-byte lock-free atomics at slab-header handshake), packed `rootInstance:24 \| transitionGeneration:32 \| state:8`; transitionGeneration approaching wrap escalates to a fresh rootInstance exactly as slab lifetime generations escalate to a header-epoch re-handshake — no field ever wraps in place. **Epoch reset tombstones EVERY armed state — pending AND all confirmed/nacked states — in the main-side sweep**, so BOTH sides' subsequent CAS fail and a `confirmedAllow` whose commit never ran is structurally invalidated (confirm-then-reset-before-commit resolves to reconcile, never a post-reset pop). Losing-side outcomes are defined no-ops: late confirm after expiry ⇒ the runtime reads `expired` and reconciles; late expiry after `confirmedAllow` ⇒ no-op (wedge posture); late expiry after `confirmedCancel` or `nacked` ⇒ no-op (the cancel/nack path already ran or runs at the next scheduled load). M2 ships the adversarial interleaving suite as **one composed Loom harness with the slab reset-barrier interleavings** (they share the epoch-sweep actor): confirm-vs-expire races for all three runtime verdicts, late-CAS no-ops in every direction, reset-during-pending, reset-after-each-armed-state, confirm-then-reset-before-commit, structural-nack-vs-timeout, lost-wake-then-scheduled-load, transition-generation ABA, CAS-paused-across-root-destroy/recreate, and engine shutdown |
| Coalesced observability | main → runtime | event queue, latest-wins, opt-in | Diagnostics and JS-side mirrors; loss-tolerant by design |
| Worklet install/update | runtime → main | LLP 0297 §4.3's shipped install path | No new identity needed |

**Landed M2 authority:** `tests/motion/record-classes.json` is the
machine-readable record-class manifest for kind id, layout version,
direction, transport plane, writer, and retention. Rust and TypeScript keep
small runtime projections of that authority, and the registered
`motion-conformance` check joins them to the retained cross-language fixtures
and schema digest. This prevents 0297/0099 transport drift in the same way the
protocol inventory prevents opcode drift; identity, credit, and epoch rules
that are richer than the closed manifest schema remain normative in the table
above and its referenced LLP sections.

- **Why the decision record is its own class — the author adjudication
  (2026-07-13) of round 7's blocking finding.** Codex r7 found the
  three-document contradiction: RFC 0100 specified the `DecisionCell`
  as a §4.5 slab word with competing runtime/main CAS, this document
  makes all slab writes main-side, and the integration list still said
  0100 was consumed as-is. Its resolution menu: (a) a separate
  two-writer control record, (b) a narrowly defined exception to
  main-only slab writes, (c) a replacement confirmation mechanism
  preserving exactly-one authorization. **Option (a) is adopted.** The
  invariant this document actually crystallized was never "the runtime
  writes nothing shared" — the consumed cursor is already a
  runtime-owned atomic — but "**every shared record has exactly one
  declared writer**" on the value and log planes. The decision record
  is the one word whose *semantics* require two writers: exactly-one-
  winner between two threads is a consensus problem, so both parties
  must perform an atomic RMW on the same word. No single-writer variant
  exists — a runtime that reads `pending` and then commits has a fatal
  window where main expires the cell and jump-to-truth reconciles while
  the runtime is mid-mutation, precisely the split brain the cell
  prevents. So the class is named, enumerated, and model-tested rather
  than smuggled into the value plane. **Why not (b):** the single-writer
  slab rule is load-bearing for the §Registry reclaim chain — the epoch
  sweep linearizes with every writer by program order because both run
  on main, whole-slab recycling is provable because no runtime-side
  writer can be in flight, and LLP 0313's sequence-mediated samplers are
  safe because single-writer + watermark covers that read class. A carve-out
  forces a case split through each of those arguments and requires the
  slab FFI to grow a runtime-side CAS accessor — a general-purpose hole
  punched to serve one word (0100's own review record flagged that no
  such accessor exists in the slab; that absence is a feature). **Why
  not (c):** the only coherent single-writer replacement routes
  authorization through main as messages (confirm-request → main
  linearizes against its own timeout → verdict → the runtime commits on
  verdict). It preserves exactly-one authorization but inserts a
  main-thread round trip into every held commit under exactly the
  runtime pressure that created the hold, adds two record classes plus
  admission rules to this table, adds parked states to 0100 §4b's
  already-heavy machine, and weakens the wedged-runtime fence — 0100's
  P1 wedged-runtime fixture is decisive precisely because the fence is
  one atomic word the late queue drain must lose. The two-writer CAS
  costs one root-lifetime word per root and an interleaving test.
  **Landing:** this table row is normative for the plane assignment and
  writer topology; RFC 0100 re-homes the cell and keeps the state
  machine (its edit is queued for the 0100 loop's resume reconcile);
  LLP 0297's §4.4 plane-inventory amendment (ledger, §Amendments)
  enumerates the class.
- **Two transports, named once — the sidecar precedent's actual
  shape.** *Protocol ingress*: producer→kernel command records under
  wire rules — the u16 per-op payload ceiling, chunk records, and
  byte-level validation at decode (on the dedicated runtime thread the
  producer and the kernel share a thread, but the wire contract is
  unchanged). *The runtime→main crossing*: the **typed commit-output
  publisher** — the shipped `ExactEngineCommitOutput` staging-queue
  precedent — which this document extends with ordered pre-presenter
  detach and post-presenter attach slots. Where a row above says
  "ingress," wire rules apply; the cross-domain leg is ALWAYS the typed
  publisher — **Motion introduces no second raw cross-domain ring**.
  Chunked snapshots are assembled and validated **runtime-side,
  pre-emission**, into one complete logical set (each leg
  byte-validates its own transport; the crossing carries typed
  records, never raw chunks), so no staging timeout or partial set
  ever crosses the authoritative boundary.
- **One record clock per root, named: `motionSeq`.** A per-root u64
  allocated at write issue on the runtime thread, carried by sidecar
  transactions and value/ack command records alike; it totally orders
  a root's records across drained frames by issue order. Comparisons
  are **epoch-qualified** (a bump resets the space). Root
  destroy/recreate starts a fresh root instance whose records never
  compare against the old (the root instance id scopes the space).
  u64 headroom makes wrap unreachable (debug-asserted, not handled).
  The wire header's u32 `sequence_id` is **transport framing on the
  ingress** — frame identity, not a record clock — and the publisher's
  internal u64 counter orders crossings, not records: `motionSeq` is
  the only clock records and watermarks compare on.
- **Raw per-frame gesture samples stay Motion-internal and
  loss-tolerant.** Do not confuse them with inventory event
  `MotionGestureStart`, which is the reliable semantic activation record
  emitted only after lease grant. Any future raw exposure remains opt-in,
  coalesced latest-wins, and carries stream + generation identity; raw input
  is never load-bearing for app correctness.
- **Three failure classes, three behaviors — one model.** *Byte-valid
  but semantically invalid* descriptor sets — unknown enums, non-finite
  parameters, duplicate identities, dangling references, cycles,
  impossible bindings — are caught by the **runtime-side pre-emission
  validation** (the sidecar table row above): the complete tree+sidecar
  transaction validates as a unit *before kernel mutation*, and ANY
  invalid record — detach or attach — **rejects the whole transaction
  there, loudly**. Descriptors are framework-produced, so an invalid
  one is a framework bug, not an app input: dev builds fail the commit
  with a naming diagnostic; the last valid tree and Motion graph simply
  persist because nothing was emitted. **Release rejects identically**
  — the transaction never becomes authoritative in any build mode;
  only loudness differs (dev fails the commit at the callsite with the
  naming diagnostic; Release rejects with a rate-limited diagnostic
  and the last valid state persists). There is no
  prune-then-retain path — an invalid attach set never coexists with an
  applied tree commit. Consequently, *main-side* byte-valid
  disagreement can only mean **version skew, corruption, or
  codec/validator implementation divergence** (hand-authored codecs
  make divergence a real class — the pager's three disagreeing
  axis-decode sites below are its live fixture): it fails
  closed (whole frame) with the schema-digest check as the diagnosis
  path — the shipped precedent already rejects a bad
  `SetPagerBindings` payload with a `ProtocolError` (`dispatch.rs`),
  and the Swift decoder fails malformed snapshots closed. Today's
  shipped pager decode is the motivating fixture for the validation
  corpus: its decode sites disagree — the Swift direction-math site
  reads any non-zero axis byte as vertical, the Swift transform-apply
  sites test `== 0`/`== 1` exactly (axis ≥ 2 gets direction math but
  **no transform**), and the Rust FFI maps every non-`1` byte to
  *horizontal* — the same byte meaning different things in three
  places, exactly the incoherence runtime-side validation eliminates
  (the first entry in M2's test corpus). Structural prevention, not
  vigilance: the runtime-side semantic validator and the main-side
  consumer share **one implementation home** — kernel-crate logic
  linked into both domains — so validator/consumer drift cannot recur
  by construction. *Well-formed but stale*
  records (stale generations, detached or unknown identities) are
  defined no-ops with dev diagnostics — "the tolerated-not-raced rule"
  means this class only. Chunked snapshots do not weaken the model:
  **chunk staging is runtime-side, pre-emission** — an emitted frame
  always carries a complete, validated descriptor set, so no staging
  timeout or partial set ever crosses the authoritative boundary.
- The shipped pager pair — binding descriptor + `PagerCommit` — is the
  precedent instance of this table. Motion's general records now supersede
  the interim `island.register`/`island.unregister` host ops in production:
  descriptor kind 14 groups validated layout-property edges by island and
  resolves each SharedValue dependency through its real registry tenant
  (LLP 0313's recorded expectation). M2
  also replaces `ExactPagerMotion.swift`'s hand-coded identities (it
  hardcodes the descriptor opcode and 18-byte entry layout today):
  **identities and tables are generated** from the inventory (the
  existing generator's scope); **codecs are hand-authored** against
  byte-level golden fixtures plus parity checks — the inventory today
  describes payloads in prose, so codec *generation* would need a typed
  payload schema (IDL), which is recorded as the follow-up if Motion's
  record count grows past a handful.

## Performance

### Measured today (the substrate this design inherits)

| Measurement | Value | Scope / provenance |
|---|---|---|
| Worklet invoke, p95 | 1.07µs (~230× headroom) | LLP 0297 r9 — invoke only, via the current JSON ABI |
| Gesture sample → worklet → layer write, **elapsed-time p95 per sample (simulated-120Hz burst)** | 0.248ms (0/600 samples over the 8.333ms budget) | LLP 0297 r9 — the macOS proof slice: elapsed time around `driveGestureSample` in a `Timer`-driven burst harness; excludes the platform recognizer callback and is **not an observed presentation timestamp** (same-frame-on-glass evidence arrives with M3/M4 instrumentation) |
| Island layout+apply, max @120Hz hardware | 0.214ms (2.6% of the 8.333ms budget) | LLP 0313 rev 8 — measured inside the real AppKit display-link path on 120Hz hardware, on a **Debug host** (conservative; the recorded run's own provenance) |

These are prose-recorded results from their source documents' harnesses;
raw distributions were not retained for them. They are inherited context,
not Review-stage baselines — any figure used as an acceptance baseline is
**re-run in Release with retained distribution artifacts at M3** (see the
measurement protocol below).

### Budgets (CPU work per display-link tick, Release builds)

| Metric | Budget | Notes |
|--------|--------|-------|
| Driver advance | < 25µs per driver | analytic sample or quantum steps |
| Binding evaluation | < 50µs per binding | worklet invocation dominates; measured invoke p95 is ~1µs, so the ceiling is generous |
| Reference-workload tick total | < 2ms for 20 drivers + 20 bindings | worst case at the per-item ceilings is 20×25µs + 20×50µs = 1.5ms, leaving arena/scheduler margin inside 2ms — the ceilings **compose**; expected actuals are far lower |
| Recognizer sample processing | < 20µs per sample | state machine + velocity sample + arena checkpoint |
| Memory | 12–16 bytes/slot (tagged word + write-generation counter) + ~64 bytes/value split across the two domains (registry entry on the runtime thread + binding-table entry on main) + ~256 bytes/recognizer | steady-state allocation-free tick (gated on the M6 typed ABI) |

**Units and the reference DAG.** Budgets are per-element ceilings over a
named reference graph — **DAG-A: 20 values, 20 worklet nodes with
fan-out ≤ 2, 20 drivers, 30 property sinks** — encoded as a checked-in
fixture at M3 (the same artifact the trace format and conformance
harness consume). "Binding evaluation" means worklet invoke **plus its
edge propagation**; sink writes, arena work, and scheduling are costed
separately. The complete worst-case subtotal for DAG-A: 20 drivers ×
25µs + 20 binding evaluations × 50µs + 30 sink writes × 5µs + arena/
checkpoint work ≤ 100µs + scheduling/publication ≤ 100µs ≈ **1.85ms
< 2ms** — the margin is demonstrated, not asserted. Budgets do not
claim linear composition beyond the reference shape: bigger or denser
graphs get measured, not extrapolated, and the memory line excludes
allocator/side-table overhead until M1 pins the real structures.

Budgets are per-tick CPU work, **not** end-to-end latency — "touch to
glass" folds in input delivery, compositor scheduling, and presentation,
which Motion does not control. The end-to-end commitments are: (a)
compositor-path bindings apply in the **same display-link tick** as the
input sample that changed them (E2), and (b) Motion adds **≤2ms to
touch-to-glass** versus a hand-written platform equivalent (LLP 0297's
inherited delta metric — the honest end-to-end formulation).

### Measurement protocol (adopting RFC 0100's pattern)

Acceptance numbers are reported as p50/p95/p99 from Release builds on
named physical hardware (120Hz-capable for the same-frame claims),
against a named workload (DAG-A above, plus island counts), with refresh
rate and thermal state recorded, and — where a stock-platform equivalent
exists — side-by-side with that baseline. **Result artifacts
(distributions with hardware, OS, build configuration, harness revision,
and sample counts — not only prose summaries) are retained in-repo for
every number quoted as acceptance evidence from M3 onward**; the
inherited numbers above are labeled with their actual provenance rather
than re-blessed. One versioned **Motion trace format** (clock samples,
input samples, arena checkpoints, value publications, driver changes,
property writes, presentation receipts, cost measurements) serves both
the cross-host parity fixtures and benchmark artifact retention. The
ProMotion plist opt-in lands with M3 (see §Apple). The driver conformance fixture (refresh-rate and
cross-host invariance) and the tick-budget check **will register** in
`exact-verify.json` at M3 per the verification authority — the existing
`gesture-motion-fixtures` entry is a marker-level fixtures check, not
this.

## Implementation Plan

Dependency-gated increments, not calendar weeks. Each increment names
its verification; work that already shipped is absorbed, not rebuilt.
**v1 acceptance scope: macOS + web** (macOS is the runtime-thread
default-on host and the native proof substrate; web is the permanent
TypeScript implementation). iOS promotes when its LLP 0322 gate
(ENG-23520 — met 2026-07-23, iOS default-on) and the island
producer/clock/host-op integration
(ENG-23516's remainder — the presenter adapter has landed) land;
Android/Windows are deferred behind their §Platform gates (OQ10). (The
March plan's Phases 1–6 are superseded: Phase 1's slab shipped with 0297
W2, Phase 4's register VM was retired by 0297 §4.3, Phase 6's web
runtime shipped as 0336 P1, and Phase 2's raw-intake recognizers
contradicted this document's own hybrid v1 boundary.)

- **M0 — Reconciliation (this revision).** The document matches the
  shipped substrate; protocol numbers withdrawn for inventory-first
  language; the ABI-fluidity option recorded. *Verification:* review
  artifacts; no code.
<a id="M1"></a>

- **M1 — SharedValue registry + lifetime (append-only).** The
  tagged-word slot protocol (single-`AtomicU64` generation+value CAS),
  the exported C accessors replacing `exact_shared_value_set` and the
  raw worklet binding, the segmented stable-address slab, ring/host-op
  lifecycle records (interim path — the `island.register` precedent —
  replaced at M2), the generation-sweep reset fence, **per-slot write
  generations on every write path** (closing the bypass LLP 0313
  records), and leak diagnostics. **No slot reuse at M1**: freed slots
  quarantine indefinitely until M2's watermark transport activates
  re-tenancy (v1 values are single-root, so the consumer set is
  well-defined when it does). Applies to the native slab and the web
  `Float32Array` store alike. *Verification:* the M1 test list in
  §Registry and lifetime (tagged-word CAS, model-checked adversarial
  interleavings incl. writer-paused-across-reset, free-CAS contention,
  append-only quarantine, sweep-then-teardown order, stale-read
  semantics); LLP 0313's sampler switched to write generations with its
  island checks green; dev leak report exercised. *Fallback:* islands
  stay on the change-counting sampler (shipped today) if generations
  slip.
<a id="M2"></a>

- **M2 — Motion graph transport.** The descriptor **commit sidecar**
  (transaction-scoped `{epoch, sequence, rootId}`) with **runtime-side
  pre-emission validation** and its **two-phase main-side apply**
  (detach phase before main's presenter application; attach phase after
  presenter output — the shipped pager precedent generalized), the
  whole-root snapshot record with runtime-side chunk staging (or a
  fixture-proven cap), value/driver command records, the **acknowledged
  outcome log** with its consumed cursor, admission-credit reservation,
  and applied-watermark control atomics (the reliable completion path:
  at-least-once + idempotent consumption) — the applied-sidecar
  watermark **activates the reuse M1 deferred**; the RFC 0100
  **lifecycle records** (start on the log; ack/nack command records)
  and the **decision-record control words** with their adversarial
  interleaving suite (confirm-vs-expire, late-CAS no-ops both
  directions, reset-during-pending, generation ABA); identities
  allocated inventory-first with **per-record `layoutVersion`
  fields**; **semantic validation** (runtime-side
  whole-transaction rejection) with its test corpus (invalid enums,
  non-finite values, duplicate ids, dangling refs, cycles — seeded by
  the shipped pager's three disagreeing axis-decode sites); the
  compound-claim **identity reserved** (extensible layout; the
  field-level stream-set schema lands with M5's automata, matching
  0336's assignment of field schemas to joint P3 work; composition
  *semantics* remain M5/OQ7); the pager descriptor
  becomes an instance of the general record;
  `island.register`/`island.unregister` retire into the binding table;
  pager identities/tables generated from the inventory, codecs
  hand-authored against byte-golden fixtures (an inventory payload
  schema/IDL is the recorded follow-up if record count grows). Pager
  absorption distinguishes **compatibility fixtures from
  0336-recorded defects repaired here**: `centerRevision` adoption,
  transform composition + per-commit reassertion, real generation
  fencing (not args-derived), recognizer competition/delegates, and
  busy-turn exclusion are M2/M4 **acceptance items**, and the
  characterization fixtures covering today's defective behaviors are
  marked expected-to-change, not treated as parity gates.
  Direct (worklet-less) slot→property bindings are part of this
  descriptor model, so M3's end-to-end measurement is self-contained
  before M6's style-worklet lowering exists. **M2 decides the pre-1.0
  renumber option** (coherent Motion block vs. allocate-in-free-space;
  either way, version/digest-guarded). *Verification:* protocol
  round-trip tests against the inventory; byte-golden codec fixtures;
  the semantic-validation corpus; two-phase apply tests (sidecar invalid
  + targets destroyed in the same commit); a named snapshot-size fixture
  on real apps decides OQ8; watermark-gated reuse (the M1-staged ABA
  tests activate here); the **mixed-version AND mixed-layout startup
  matrix** (old host + new producer differing only in a layout bump,
  and vice versa); pager fixture unchanged through the migration
  (characterization tests); `verify-registry-consistency` green; new
  callbacks (terminal events, watermark) classified in
  `docs/callback-affinity.md`.
  *Current landing state at r21 (2026-07-15):* complete on the macOS/web v1
  paths. Inventory-owned descriptors and commands, strict whole-transaction
  validation, two-phase Apple application, acknowledged outcomes, decision
  cells, watermarks, mixed-version rejection, and layout-island bindings are
  all exercised by `motion-conformance` and the focused Apple suite. The
  ENG-24960–ENG-24978 pass closed the remaining accepted-record loss,
  cancellation, boundedness, and lifecycle-fence defects.
<a id="M3"></a>

- **M3 — Driver family (native Rust + web TS) + MotionClock.**
  Spring/timing/decay/sequence/repeat with the simulation contract
  (analytic solvers / quantum accumulator, timebase, gap semantics);
  driver→value exclusivity; settle events; the web leg implements the
  same solvers by specification. The **MotionClock registry** stands up
  per-surface scheduling, absorbing the island pass and native-view
  ticking without duplicate links; iOS display association defined
  here. *Verification:* the cross-platform/refresh-rate conformance
  fixture registered in `exact-verify.json` (60/80/90/120Hz trace
  tolerance, OQ11's number decided here; the registered check also
  asserts **presence of the retained result artifacts**, so the
  measurement-protocol rule is machine-checked, not aspirational); the
  DAG-A reference graph checked in as the shared budget/trace fixture;
  ProMotion plist opt-in lands; a SharedValue driven by a spring
  animates a view at 120Hz with zero app-JS involvement — measured
  end-to-end through the slab→binding→presenter pipeline **with
  presentation-timestamp instrumentation** (replacing the CPU-budget
  proof-slice number).
<a id="M4"></a>

- **M4 — Recognizer generalization + arena completion (LLP 0336 P3,
  ENG-24183).** Pan/Tap first, then Pinch/Rotation/LongPress/Fling;
  **the full §6.1 lifecycle lands on BOTH implementations** — `motion.rs`
  (lease type, topology freeze, three checkpoints, outcome lifecycle,
  profile validation) and the web recognizer (the same contract in TS)
  — with one shared conformance suite driving both, including
  adapter-reported external outcomes; `ExactPagerMotion`'s recognition
  policy, candidate selection, and settlement migrate into Motion (the
  Swift side becomes the thin sample adapter); velocity tracker chosen
  and validated against the trace corpus. **Explicit prerequisites
  from LLP 0336's own open questions:** OQ16/OQ17 (the native
  interaction-state publication channel and the router/host snapshot
  publishers) were decided by ENG-24698 before this implementation landed;
  M4 consumes those decisions rather than silently assuming them.
  *Verification:*
  recorded-trace replay (same commits, same receipts before/after);
  pager characterization suite; the shared arena conformance suite;
  swipe-to-dismiss demo driven entirely by Motion.
  *Current landing state at r21 (2026-07-15):* implementation-complete on the
  macOS/web v1 paths. The native Rust arena owns the lease/checkpoint/outcome
  lifecycle and pager policy behind thin Apple adapters; the web pointer and
  normalized-wheel paths share the arena with pager, scroll, browser-system,
  and RouterHistory contenders; profile publication and token-bearing
  re-center acknowledgement are live. The shared Rust/web trace corpus,
  renderer acceptance, and Apple host suites are green. M4 acceptance remains
  open only for the manual named-browser edge evidence registered as
  `motion-web-profile-evidence`.
<a id="M5"></a>

- **M5 — Composition API over the arena.** Operational semantics for the
  compound-claim automaton (OQ7): activation priority, ordering, loser
  callbacks, nesting, multi-pointer membership, and lowering onto
  claims/leases — `exclusive`/`race` first, `simultaneous`/`sequence`
  after their lease-model extension is decided; receipts extended with
  composition decisions. *Verification:* arena-receipt fixtures per
  combinator; the unsupported-nesting diagnostics stay diagnostic.
<a id="M6"></a>

- **M6 — Authoring surfaces + worklet ABI (Contract-first).** Per
  LLP 0160 §5.2, the Contract binding is not an afterthought:
  **native rated-publish ownership (r13):** the pacer, payload
  evaluation, and provider call run on the RUNTIME side exactly as
  the shipped web channel does — Motion publishes samples plus a
  per-channel dirty generation with a level-triggered wake, and the
  runtime-side pacer samples on its cadence (the final write-driven
  sample is guaranteed by the dirty generation, not a heartbeat);
  provider code never executes on main. Scalar-channel AUTHORING
  belongs to the Contract-motion grammar authority (the M6-named new
  Contract RFC or 0089 revision) — NOT to 0281, which shipped pointer
  channels only and declined its stage 1; the
  **Contract motion grammar** (recognizer/composition/driver/binding
  declarations in `.contract`) lands **no later than** the React tier.
  Ownership is explicit: **the grammar is owned Contract-side — a new
  Contract RFC or an RFC 0089 revision, as the 0089 owners choose**
  (0089 deliberately left user-facing motion syntax open); this
  document owns the lowering, host contract, and requirements, and
  this is a **new motion-specific language decision, not a reopening
  of LLP 0281's declined stage 1**. The owning Contract document is
  drafted and accepted before M6 freezes any syntax. React tier: `useSharedValue`, `useDerivedValue`,
  `useAnimatedStyle`, `GestureDetector` (including the reliable
  `onGestureEnd` semantic event, consuming M2's transport). The worklet
  obligations from §Worklets: nested/inline extraction with capture
  classification and serialization, stable callback identities, the
  install-format decision (HBC vs. source, recorded), the **typed
  allocation-free invoke ABI**, output-slot lowering for style worklets,
  the propagation contract (topological, once-per-tick, cycles
  rejected), and the `runOnJS` bounded queue + app-runtime drain.
  LLP 0281's transient bindings/value channels lower onto the Motion
  substrate — scalar channels to slots, **pointer channels to the
  record-bundle layout** (§Pointer-channel records) — with native
  semantics conforming to `channels.ts` stage-0 behavior via its ported
  characterization tests. **Scope restriction:** M6's low-level gesture
  surface is limited to non-semantic visual motion; command-shaped
  gestures wait for M8's command-identity wiring (§JS API). **Declared
  split line if M6 strains:** M6a = worklet ABI/extraction; M6b =
  authoring-tier lowering + conformance. *Verification:* both tiers
  drive the same binding graph in a twin fixture; allocation counters
  flat in steady state; Contract dataflow inspector shows channel/value
  unification; the drain callback classified in
  `docs/callback-affinity.md`.
  *Current landing state at r21 (2026-07-15):* complete on the macOS/web v1
  paths. Contract and React authoring twins compile nested/inline worklets,
  classify captures, lower typed output slots and callback identities, install
  whole-root artifact planes, and reconcile the app-runtime callback registry.
  `motion-m6-acceptance` proves the shared six-node graph and the steady-state
  semantic-excess allocation bound.
<a id="M7"></a>

- **M7 — Scroll bindings.** `useScrollSharedValue` + the scroll binding
  descriptor; presenter-side offset publication (classified in
  `docs/callback-affinity.md`); `scrollend`/`refresh` semantics land
  here (ENG-23128 spellings); parallax-header milestone. *Verification:*
  scroll-linked binding updates in the same tick as the scroll delta on
  main; agent-observable offsets stay consistent with ENG-22790's
  scroll-tracking rules.
<a id="M8"></a>

- **M8 — Surface integrations.** RFC 0100's substrate hooks (edge
  claims, progress values, settle reporting) as 0100 implements;
  RFC 0007 amendments (`dismissOnSwipeDown` with the topmost-layer
  claim, presence retain-through-settle, presence gesture progress)
  applied in 0007 under the two dismissal regimes; **command-identity
  wiring** (gesture command bindings → claims/receipts/terminal events —
  lifting M6's non-semantic restriction); RFC 0074 adopts the
  reduced-motion table (OQ9 default); agent API additions (receipts,
  waits, the unified geometry enum). *Verification:* 0100's own
  acceptance gates; behavior-primitive contract tests (incl.
  early-unmount cancellation); agent fixtures.
  *Current landing state at r21 (2026-07-15):* complete on the macOS/web v1
  paths. The shared dismiss authority, Presence retention, semantic command
  witnesses, generated reduced-motion profile, and Acto projection are covered
  by `motion-m8-surface-integrations`; RFC 0100's physical navigation evidence
  remains owned by its separate P1/P1.5 gates and does not make M8 incomplete.

## Amendments this document proposes to sibling authorities

One auditable ledger (the RFC 0100 house pattern) for every place this
document touches ground another document owns — each lands *in the
owning document* at the named milestone:

- **LLP 0297 §4.5 (declared refinements — two, not one):** (a) the
  single-word slot shape is refined from the named
  `AtomicF32`/`AtomicU32` spike to the **tagged `AtomicU64`**
  (generation+value in one word), and the raw-pointer worklet binding
  (`ex_worklet_bind_shared_values`) is replaced by exported C
  accessors; (b) the **access topology changes**: §4.5 permits the app
  runtime/tree domain to write storage directly, while this document
  serializes imperative app writes as **commands applied by the Motion
  domain on main** (wait-free synchronous reads are preserved
  unchanged). **APPLIED in-set 2026-07-13 — (b) in 0297 r16; (a) in 0297 r17** — including the
  §4.4/§Summary-diagram sweep of the old "both directions" write
  phrasing (reads remain bidirectional) and the cross-reference to the
  decision-record carve-out so the slab rule reads as exception-free
  by construction. (M1 stamps the storage topology; the
  value-command transport it serializes arrives at M2 and the
  app-facing write API at M6 — no app-JS write path exists before
  those, so the early stamp is deliberate, not off-by-one.) (M1)
- **LLP 0297 §4.3/§4.6 (conditional amendment):** 0297's contract
  language compiles React/TS worklets to HBC
  (`installWorklet(id, bytecode, …)`); the shipped path is source-text
  install. M6 either **confirms HBC** (closing the gap toward 0297) or
  records an explicit 0297 amendment adopting source-install with the
  measured evidence — the two documents end with one normative answer.
  **APPLIED 2026-07-13 (0297 r19): explicit UTF-8 function-expression
  source is normative; 64-artifact warm p50 0.081ms / p95 0.182ms, 5ms
  ceiling; future HBC is a new explicit format, not inferred bytes.** (M6)
- **LLP 0313 (agent-surface extension):** the shipped
  `geometry: predicted | model` vocabulary is unified into one
  three-value enum (`model | predicted | presented`) on the agent
  surface; the agent-operations authority owns the enum, and 0313's
  surface is extended, not contradicted. (M8) *Also recorded in-set
  2026-07-13 (0313 rev 9): two verified interim defects in 0313's
  shipped machinery were named. The no-SVG live-resize stale-descriptor
  resume (ENG-24662) is **RESOLVED 2026-07-13**: islands remain suspended
  until the post-resize descriptor sync and the engine forces a full
  descriptor walk when a no-SVG span ends, with a changed-envelope
  regression. The equal-value generation-reconstruction ABA +
  skip-if-absent freshness defect (ENG-24663) is retired structurally by
  this document's M1 write generations + M2 watermarks; 0313 carries its
  remaining provenance and regression spec.*
- **LLP 0297 §4.4 (plane-inventory amendment):** the acknowledged
  outcome log (main→runtime records), its consumed cursor
  (runtime→main atomic), and the **decision-record control words**
  (RFC 0100's `DecisionCell` — enumerated as the sole two-writer
  record class, per the r9 adjudication: the value plane's
  single-writer rule is unchanged, and two-writer shared state is
  legal *only* as a record class enumerated in this plane inventory)
  are a **new communication plane** joining
  §4.4's enumerated set (slab, constraint buffer, ring + feed — the
  input-event queue is B4/§Summary-diagram vocabulary rather than part
  of §4.4's enumerated list, and the note cites it accordingly).
  **APPLIED in-set 2026-07-13 (0297 r16)**, alongside the §4.3 note
  recording `runOnJS`'s delivery contract (bounded drop-oldest,
  observational only, never correctness-bearing) and the
  shipped-install honesty note (source-text today; HBC is the M6
  target). (M2/M6 for the mechanisms; the contract language landed
  now)
- **RFC 0007:** `DismissableLayer.dismissOnSwipeDown` + the
  topmost-layer claim, and `usePresence` gesture progress +
  retain-through-settle. (M8)
- **RFC 0074:** **APPLIED 2026-07-13** — owns the reduced-motion class
  profile, JSON authority, generated projections, and shared resolver (OQ9
  default). Motion retains enforcement, not a second class table. (M8)
- **ENG-23128 reserved spellings:** `scrollend`/`refresh` acquire their
  semantics with scroll bindings. (M7)
- **LLP 0336 (§6 sweep — APPLIED in-set 2026-07-13, 0336 r14):** the
  lands-second cross-reference rule fired on this revision: 0336's §6
  protocol paragraph and Related line described this document's
  numeric tables as "stale sketches" with the `GestureStart = 13`
  collision — those tables are now withdrawn for inventory-first
  language, and 0336's text says so (the historical collision note
  stays as history). Its P3-authoring boundary is also fixed in the
  same revision (arena lifecycle = P3/M4; authorable composition = M5),
  closing the ambiguity this entry previously recorded: §6.1
  associated P3 with the authorable composition API while its phasing
  table says no authoring changes; this document treats combinator
  authoring as M5/OQ7 and does **not** read 0336 as having settled it —
  reconciled with 0336's owners at M5. The web recognizer's §6.1
  lifecycle completion and reduced-motion settle branch are M4/0336-P1
  acceptance work tracked in "What exists today."
- **RFC 0089 / new Contract RFC:** the Contract motion grammar is a
  new Contract-side document (or an 0089 revision — the 0089 owners
  choose), drafted before M6 freezes syntax; explicitly not a
  reopening of LLP 0281's declined stage 1. (M6)
- **RFC 0100 (r9 adjudication; APPLIED and upgraded in-set — r12):**
  0100 adopted the control-plane re-home in the set-loop round-0
  reconcile and now carries the r12 closed state machine
  (`confirmedAllow | confirmedCancel | nacked | expired`, the
  `(rootInstance, transitionGeneration, state)` AtomicU64 packing, the
  wake-independent load points, and the reset-covers-armed-states
  rule) — nothing remains queued on its side. Historically: consumed for interactive-navigation
  semantics — gesture arbitration handoff, two-phase resolution, scene
  leases, and the `DecisionCell` state machine, timer origin, and
  tombstone behavior are 0100's unchanged — with ONE amendment queued
  for its side: the `DecisionCell` re-homes from "living in the §4.5
  slab" to the decision-record control word defined in this document's
  record-class table, and its spec adopts that row's
  `(transitionGeneration, state)` word layout, AcqRel ordering,
  epoch-tombstone behavior, and adversarial interleaving test. The
  0100 loop applies this on resume per its recorded protocol. (M2 for
  the record class; the 0100 doc edit lands at its loop's resume
  reconcile)
- **LLP 0281 (citation amendment, applied in-set 2026-07-13):** its
  gesture-recognition pointers ("RFC 0007's host gesture composition
  remains the recognizer layer") predate LLP 0336/this document owning
  recognition and arbitration — 0281's Related line and §"what this
  RFC does not solve" bullet now cite Motion (recognition/arbitration
  per 0336 §6.1) with RFC 0007 as the behavior-primitive integration
  surface. Channel/binding semantics are unchanged — 0281 stays the
  language projection this document consumes.
- **RFC 0046 / RFC 0010:** consumed as-is; no
  amendments proposed.

## Open Questions

1. **Worklet compiler residue — resolved for v1.** M6 landed the restricted
   Hermes math/filter classes, nested and inline extraction, capture
   validation/serialization, output-slot lowering, stable content+capture
   identities, and the typed fixed-slot no-JSON invoke ABI. Explicit UTF-8
   function-expression source install is the normative v1 format, backed by
   the measured mount-time ceiling recorded in LLP 0297; a self-contained HBC
   artifact remains an optional future optimization, not unfinished v1 work.
   Contract provable eligibility remains governed by LLP 0297's rules in the
   Contract-motion authority, not by LLP 0281's declined stage-1 analyzer.

2. **Layout-driving animations (recorded answer).** Should animated
   SharedValues drive layout properties (width, height, flex) in v1?

   **Accepted v0 answer (2026-07-06, LLP 0313 — now Implemented):**
   LLP 0313 owns this question together with LLP 0297 OQ4. Default
   behavior remains the 0297 constraint-buffer route (runtime-thread
   layout, next commit). The accepted same-frame upgrade is a bounded
   **layout island** over a feed-backed, layout-closed subtree with
   SharedValue-only dynamic inputs, input-matched settlement, and
   conservative model-geometry-only accessibility frames during motion.
   Compositor projection remains the first choice when true relayout is
   not required, and widening 0297's servicing-wait carve-out is not
   part of this answer.

3. **Recognizer end-state.** The v1 boundary is hybrid (platform
   recognizers as sample adapters). Is that boundary permanent — as the
   web recognizer's permanence suggests by analogy — or do Motion-native
   recognizers replace platform ones as they mature? What evidence would
   justify replacement (trace-corpus parity? composition needs platform
   recognizers can't express? a platform breaking its recognizer
   contract)?

4. **Web driver eligibility detail.** §Web sets the posture (rAF
   primary; WAAPI for fire-and-forget compositor curves; springs/decay
   rAF-driven or pre-sampled). Remaining: the concrete eligibility
   matrix (which property × driver combinations take WAAPI), the
   contention-degradation measurement, and whether drift between the TS
   and Rust solvers ever justifies revisiting the recorded WASM-core
   option (§Alternatives).

5. **Tier 1 ↔ Tier 2 handoff, concretely.** When a Tier-2 binding
   settles on a property that also has a Tier-1 transition configured,
   who owns the presentation value at the handoff frame in each
   presenter tree (AppKit/UIKit/web), and how is "no visual jump"
   asserted in tests? The suppression + fenced-reconciliation rule above
   is the design; the per-presenter mechanics need a written contract.

6. **Contract framework integration (recorded answer, refined).**
   RFC 0089 defines motion categories for Contract. Should Motion be the
   execution target for Contract's declarative motion model, or should
   Contract have its own motion runtime?

   **Answered 2026-07-02 (LLP 0281 §1, recorded here per its seam; ENG-22525):**
   yes — Motion is the execution target; Contract does NOT grow a second
   motion runtime. A Contract transient binding (and its stage-0 concrete
   form, the value channel) is the **Contract-language projection of the
   Motion value substrate**: there is one per-frame value system in
   Exact, and this document owns its execution (registry, native intake,
   host channels, driver model). Refinement (r3): "projection of a
   SharedValue" means *onto the substrate*, not one-scalar-slot-per-
   channel — pointer channels lower to record bundles
   (§Pointer-channel records). Contract owns the *authoring surface* —
   how a `.contract` file declares, confines, samples, and publishes
   such values. The seam extends to input: the author-bindable
   pointer-event vocabulary and its payload shape
   (`pointerdown/move/up/cancel/enter/leave`, element-local
   `{x, y, pointerId, pointerType, buttons, pressure?}`) are Contract
   surface (LLP 0281, per LLP 0275 A1); raw input intake and per-host
   delivery plumbing are Motion substrate — one vocabulary, one intake,
   two documents. Escalation unifies too: requests to animate
   layout-affecting properties route to OQ2's answer, not a parallel
   path. LLP 0281 stage 0 shipped on the JS frame-coalesced reference
   implementation plus the web host channel whose semantics are
   normative for the native channels this document's M6 builds;
   **stage 1 was declined** per 0281's §3.1 gate, so the native lane
   arrives with Motion, not before.

7. **Composition semantics — resolved for v1.** M5 landed the
   `simultaneous`, `sequence`, `exclusive`, and `race` automata behind one
   externally exclusive compound claim, including declaration-order ties,
   loser outcomes, nesting, multipointer membership, and the no-mid-gesture-
   revocation rule.

8. **Transport shape — resolved for v1.** M2 selected atomic whole-root
   descriptor snapshots with bounded, digest-checked chunking for payloads
   above the wire ceiling. Inventory-assigned record kinds and schema digests
   preserve one coherent ABI without a competing incremental registry.

9. **Reduced-motion table ownership — resolved.** RFC 0074 adopted the
   per-class table at M8 on 2026-07-13 and is now the single class authority;
   its existing preference signal/platform mappings stay co-located there.
   This document owns Motion-side enforcement only.

10. **Per-host acceptance.** What are the Android and Windows entry
    gates (Tier-1 transition parity first? `WM_POINTER` intake +
    touchpad registration? which conformance fixtures must pass on which
    hardware), and what promotes iOS from substrate host to conformance
    host (ENG-23520 — met 2026-07-23 — + ENG-23516's
    producer/clock/host-op remainder are the known gates — the island
    presenter adapter itself has landed)?

11. **Tuning profiles and trace tolerances — partially resolved.** M3's
    deterministic solver and refresh-rate conformance profiles now pin the
    automatic tolerances, and M4's estimator runs against the shared trace
    corpus. RFC 0100 still owns the stock-vs-Exact physical-device fitting and
    ProMotion evidence; those retained artifacts are the remaining answer.

12. **Raw gesture event ABI.** Which real consumers (devtools? analytics?
    app-level gesture mirroring?) justify public raw `GestureStart/
    Update/End` events, and with what coalescing, backpressure, and
    generation semantics? (Deferred by LLP 0336 until a consumer exists;
    P3 amends the identity set here if one appears.)

13. **Clock edge cases, power, and cross-root values.** Per-surface
    clocks are the model (§Clock ownership); remaining: display
    migration mid-gesture (re-anchor rules for in-flight drivers),
    whether any driver ever needs a process-global clock, the
    ambient-motion power policy (per-driver frame-rate-range
    preferences; what an infinite `withRepeat` may pin and when the
    system may demote it), and — since **v1 prohibits cross-root
    values** (§Registry) — the multi-window sharing design that would
    lift that prohibition: consumer registry, minimum-watermark reuse
    across roots, sampling-skew bounds for mixed refresh rates, and
    which host owns a shared value's lifetime.

## Alternatives Considered

### JS-side gesture handling (status quo for web frameworks)

Process gestures in app JS and apply styles via React state updates. This
is what React does on web and what React Native does without Reanimated.
Rejected for native because it cannot achieve 120fps — every touch event
requires a JS round-trip, and batched rendering adds latency. (On web,
main-thread TypeScript recognition **is** the design — the main thread is
the UI thread there, and the shipped recognizer is permanent; see §Web.)

### Reanimated-style secondary JS runtime — adopted in restricted form

The March draft rejected running a second JS runtime on the UI thread as
complexity Exact's "kernel already on the UI thread" made unnecessary.
The threading work proved the opposite trade: LLP 0297 §4.3 **adopted**
a persistent, restricted Hermes instance on main as the UI worklet
runtime — because the tree/layout kernel moved *off* main, and because a
real JS engine (with generation fencing and a confined environment) beat
a bespoke register-machine VM on correctness, tooling, and
Hermes-semantics fidelity, while the kernel-owned SharedValue slab
eliminates the dual-heap serialization that makes Reanimated fragile.
What remains rejected is the *unrestricted* version: a general-purpose
second app runtime with its own module graph and object protocol. The
restriction philosophy of the March VM sketch survives as the math-class
eligibility rules (LLP 0297 §4.6).

### Platform-native gesture recognizers only

Delegate entirely to UIGestureRecognizer (iOS), GestureDetector
(Android), etc. This maximizes platform fidelity but makes cross-platform
composition impossible and puts gesture *policy* in platform-specific
code. Motion's hybrid keeps platform recognizers as **sample adapters**
(fidelity where platforms are good) while owning arbitration, values,
and drivers centrally (the part that must be one model). Full delegation
remains rejected; full replacement is OQ3.

### A WASM recognizer/driver core on web

Compile one Rust core to WASM for web, leaving only DOM event intake in
TypeScript — recorded by LLP 0336 as a post-P3 consolidation option,
**not chosen**: it would erase the dual implementation at the cost of
departing from the working, debuggable TypeScript recognizer and its
characterization suite. §Web's shared-by-spec analytic solvers shrink
the drift surface this would address. Revisit only if solver or
recognizer drift becomes a real maintenance cost (OQ4).

### A portable Motion-expression IR

Define a compiler-neutral, allocation-free expression DAG for the common
binding subset; React worklets and Contract expressions compile to it;
Rust evaluates it natively and TypeScript (or WASM) on web, with Hermes
as the escape hatch for irregular logic. Recorded as a **candidate
consolidation, not chosen for v1**: it would improve deterministic
replay, web parity, and capture diagnostics, but it adds a second
compilation target and a new IR surface before M6 shows how much of the
real worklet corpus is expression-shaped. Re-evaluate after M6 with
corpus data.

### CSS-only animation on all platforms

Expand Tier 1 transitions to cover all cases. Rejected because CSS
transitions are fundamentally state-driven — they animate between two
known values. Gesture-driven animation is value-driven — the target
changes every frame based on input. These are different problems
requiring different solutions (and Tier 1's spring kind does not change
this: it cannot inherit live gesture velocity or retarget mid-flight).

## Revision history

Newest first. Each entry is the note recorded when that revision
was applied (moved here from the header per the 0336 convention;
the **Revised:** line above carries only the newest summary).

2026-07-13 (r13 — set-loop round-3 reconciliation; the previous
**Revised:** line was this revision's full note. Round 3 split for the
third consecutive time: Fable found all six READY and the set COHERENT;
Codex found all six NOT READY and the set INCOHERENT. Every verified
mechanical finding from both rounds was applied in the closing fix
pass. The remaining Codex concerns were preserved as author-tier
status/gate/transport decisions and implementation obligations. The
loop stopped under its pre-declared asymptote rule and left the RFC in
Draft for the r14 adjudication.)
2026-07-13 (r12 — set-loop round-2 reconciliation; the **Revised:**
line above is this revision's full note. Round 2 split again — Fable
0099/0100-only NOT READY with three Materials, Codex all-six NOT READY
with five Blockings — but the two families' findings CONVERGED on the
same defects in the round-1 text (the nack/wake gloss, the
benign-policy-bump forced cancel, the unspecified consumed-generation
transport), plus Codex-only finds the orchestrator verified
(blocked-attempt record class missing; canGoBack-true-during-blocker;
the 0046 second Interruption section; 0281's measured-zero nonfinite
leak; 0297 W4b/AtomicF32/affinity drift). The Codex round's first
attempt exhausted its session and is recorded as uncounted in the
artifact's provenance.)
2026-07-13 (r11 — set-loop round-1 reconciliation; the **Revised:**
line above is this revision's full note. Round 1 split: Fable READY
×6/set-coherent with 6 minors; Codex NOT-READY ×5 with 3 Blocking +
10 Material, of which the orchestrator verified every decisive claim
against the repo before applying — see the round-1 artifacts'
orchestrator notes. The convergent cross-family finding (the sampler
accessor's return shape) and Codex's verified Blockings (control-word
teardown lifetime; forcedCancel outside the linearized authorization)
drove this revision's substance.)
2026-07-13 (r10 — set-loop round 0 reconciliation, applying the
round-7 punch lists both reviewers left queued (Codex C2–C10
materials; Fable minors 3–8 plus suggestions), on top of r9's
DecisionCell adjudication. Transport: the two legs are named once
(protocol ingress vs the typed runtime→main commit-output publisher —
no second raw cross-domain ring; chunk assembly runtime-side,
pre-emission); the record clock is named (per-root u64 `motionSeq`,
epoch-qualified, root-instance-scoped; the wire u32 `sequence_id` is
framing, not a clock); applied watermarks move OFF the log into
per-root latest-wins control atomics beside the consumed cursor; RFC
0100's lifecycle records enter the inventory (start rides the log as a
non-terminal record so start-before-resolution is structural; ack/nack
are command records; authorization stays the DecisionCell CAS;
"exactly-once" bridged as exactly-one authorization + at-least-once
transport); the pager re-center ack is single-homed as 0336's changed
descriptor (the duplicate main→runtime representation is removed).
Schema: every correctness-bearing record carries a `layoutVersion`
digest input; M2 gains the mixed-layout startup matrix and generated
byte-schema assertions. Registry: the reset barrier is an explicit
five-state asynchronous machine (sweep → teardown → epoch publish →
transport clear → recycle-enable) with runtime acknowledgment by
construction and shipped install-race honesty; segment lifetime is
stated once (recycle-in-place; memory freed only at shutdown — the r5
"segments tear down at epoch reset" remnant deleted); the only
non-validating read is the renamed `exact_shared_value_sample_relaxed`
with a restricted sampler-class contract; admission credits get a
replacement/cancellation lifecycle (exactly one terminal record or a
released reservation; infinite drivers hold by design); the CAS
machinery is labeled topology insurance. JS API: the swipe-to-dismiss
walkthrough splits into the three completion lifecycles
(component-local / router-owned / pager), each naming its command,
retention owner, terminal records, settle condition, and
acknowledgment; CommandRef "registered" is pinned (0336's
command-instance identity family, component lifetime, witness = same
ref, router exemption no wider than the platform-signifier exemption);
the composition authoring gate is explicit (no combinator ships before
its M5 automaton — M2 freezes an envelope, not semantics); the M8
example renders its witness. Failure model: Release rejects
identically (only loudness differs); main-side disagreement classes
gain codec/validator implementation divergence; the runtime validator
and main consumer share one kernel-crate home; reset cancellation
produces no post-fence records. Current state: What-exists reissued
2026-07-13 with explicit categories (production-shipped / DEBUG-gated
proof / governing design / proposed); the gesture-worklet slice
disclosed as `#if DEBUG && os(macOS)`; the LANDED UIKit island
presenter adapter recorded and ENG-23516 narrowed to its
producer/clock/host-op remainder (plan rows and OQ updated); 0336's
lease/arena machinery recharacterized as P3-entry work jointly owned,
not unfinished P2. Ledger: 0281's stale "RFC 0007 is the recognizer
layer" citations amended in-set (recorded entry); OQ1's
Contract-eligibility owner corrected to the Contract-motion grammar
authority under 0297 OQ6 (not 0281); the §4.4 plane note's event-queue
attribution fixed; the M1/M6 stamp rationale added; M4 gains explicit
0336 OQ16/OQ17 prerequisites. Header: per-round revision history moved
to this section per the 0336 convention. Status: Draft.)
2026-07-13 (r9 — **author adjudication of the round-7
blocking finding**: the author chose option (a) of Codex r7's
resolution menu. RFC 0100's `DecisionCell` re-homes off the §4.5
SharedValue slab into a **decision-record control word** beside the
outcome log: the value plane stays single-writer main-side without
exception, and the decision record becomes the design's sole record
class whose semantics require two writers racing on one word (runtime
CAS `pending → confirmed` vs main CAS `pending → expired`;
generation-tagged against ABA, epoch-fenced). The record-class table
gains the decision-records row; a rationale bullet under the table
records why (a) beat the slab-exception (b) and replacement-mechanism
(c) alternatives; the 0297 §4.4 plane-inventory ledger row is extended
to enumerate the class; and RFC 0100 moves out of the consumed-as-is
list into its own ledger entry (its side of the edit — re-homing the
cell plus the word layout/ordering/epoch spec — is queued for the 0100
loop's resume reconcile). Scope honesty: this revision lands ONLY the
adjudication; Codex r7's other material items and both reviewers'
minors remain queued exactly as r8 left them. Status remains Draft.)
2026-07-12 (r8 — **post-loop sweep, UNREVIEWED**: after seven
dual-model review rounds the /llp-super-refine loop stopped per its
final-round rule with both reviewers NOT READY on r7; this revision
applies ONLY the two stale-phrase corrections both round-7 reviews
converged on — the plane table's Value-commands row now says one
per-root monotonic sequence (the r7 model; "per-handle sequenced" was an
incomplete sweep) and the What-exists gesture-proof row now says elapsed
hot-path time (not "CPU"). Everything else in both round-7 reviews —
notably Codex's blocking finding that RFC 0100's two-writer
`DecisionCell` in the §4.5 slab contradicts this document's
all-slab-writes-main-side rule (a 0099/0100/0297 three-document
decision), its transport/schema/lifecycle material items, and both
reviewers' remaining minors — is deliberately left for the author's
adjudication; the round-7 artifacts in
`llp/reviews/0099-motion.{fable,codex}.md` carry the full punch lists.
Status remains Draft.)
2026-07-12 (r7: round-6 convergent fixes — both reviewers
NOT-READY on r6, agreeing on the same defects in the r5/r6 rework. ONE
sidecar failure model: semantic validation runs runtime-side on the
complete tree+sidecar transaction before kernel mutation; ANY invalid
record (detach or attach) rejects the whole transaction there, loudly
(descriptors are framework-produced — an invalid one is a framework
bug); the last valid tree+graph simply persists because nothing was
emitted, so prune-then-retain is DELETED (main-side byte-valid
disagreement can only be version skew/corruption ⇒ fail closed + digest
check); chunk staging is runtime-side pre-emission, so an emitted frame
always carries a complete validated set. The outcome log is completed:
the runtime writes a **consumed cursor** (its own runtime→main atomic —
distinct from main's applied-watermark record ON the log); capacity is
**admission-credit reservation** (a terminal-producing gesture/driver/
turn reserves its outcome slot at activation; no slot ⇒ activation
fails with a diagnostic — never-drop becomes structural and the release
cap behavior is admission refusal); delivery restated honestly as
at-least-once + idempotent tokenized consumption (epoch-scoped dedup);
stale pre-r6 "event queue carries the records" phrasings swept from
§Registry and M2. Generation-sensitive sampling becomes coherent: slots
with registered samplers publish under a per-slot odd/even sequence so
(value, writeGeneration) reads are matched pairs (0313's requirement);
latest-value-only consumers keep Relaxed loads. Slab recycling gets its
invariants: per-slot generations are NEVER reset (recycling re-tenants
under strictly increasing generations; u32 wrap escalates to a
header-epoch re-handshake), epoch reset **recycles in place** (segment
memory is freed only at engine shutdown behind a cross-domain
quiescence barrier), the sweep/teardown run on main, ALL slab writes
are main-side (initial values ride the command path), and the read ABI
becomes a validating `try_read(slab, slot, generation, epoch)`. The
command sequence is per-root monotonic (per-handle dropped). The
watchdog claim is retracted to 0297's reality: overrun = jank +
diagnostic (`worklet_overrun`), a wedged worklet is a main-thread hang,
and Release interruption is gated on 0297 OQ1. `.command()` takes a
typed/registered CommandRef shared with the visible control; the
framework dispatches it on committed resolution and `onGestureEnd`
becomes observational; the router/platform signifier exemption is
tightened (the mounted snapshot is evidence, not a signifier; an
app-declared visible control is required where platform chrome is
absent). Current-state corrections: `ExactSurfaceRenderer` is the
DEBUG-only emergency renderer (compiled out of Release) — the
production secondary-surface ticker does not exist yet (M3/M8 gate);
the web-recognizer row inherits 0336's fuller implementation delta
(unproven mobile-browser edge ownership; transform overwrite);
"elapsed hot-path p95"; resident-Hermes scoped Apple-host; the
current-state preamble excludes the final proposed-scope row.
Pointer-channel records gain the pending/applied split (envelope
delivery sequence distinct from applied-sample generation; seal()
observes applied state; skew tests ported; intern table epoch-scoped).
The component-local topmost-layer authority is assigned (RFC 0007's M8
amendment under 0336 §5) with its schema requirements enumerated. The
ledger gains the 0297 §4.4 plane-inventory row (outcome log + consumed
cursor) and the §4.3 runOnJS-contract note. The shadow's population
rule is stated (writes + successful reads; the stale-fallback cache,
distinct from the withdrawn primary-read mirror). The compound-claim
record softens to identity-reserved-at-M2 with the field-level
stream-set schema landing with M5's automata (0336 assigns field
schemas to joint P3 work). In-flight gestures across surface
suspension are structural cancellations. Digest inputs are normalized
(field order fixed, prose excluded). Status unchanged: Draft.)
2026-07-12 (r6: round-5 dual-review fixes. The sidecar
transaction protocol is restated where it is actually implementable:
**semantic validation runs on the runtime thread before the transaction
is emitted** (whole-transaction rejection happens there, before anything
is authoritative — main cannot retroactively reject runtime-side tree
mutation); the two phases order detach/attach around **main's presenter
application** (the only mutation Motion can be ordered against — the
authoritative kernel tree has already moved); staged sidecars are
epoch-fenced as whole no-ops, explicitly diverging from the shipped
completions-run-on-stale-discard behavior. Terminal-event delivery is
redesigned as a per-root **acknowledged outcome log** (main appends
immutable terminal records; the runtime acknowledges a watermark;
records reclaim only below it; the event queue is just a wake) — a
bounded-FIFO-with-overflow cannot honestly promise at-least-once; each
completion class gets a named identity (gesture: stream+resolution seq;
driver settle: valueHandle+driverSeq; recenter: root+turnToken). The
app-write shadow now clears against an **applied-command-sequence
acknowledgment**, not a write-generation observable another writer can
advance. Segment-directory growth is pinned (fixed-capacity atomic
pointer array; slab-header epoch atomic named); after a reset's sweep
and teardown complete, **epoch-scoped whole-slab reuse** bounds M1
dev-session growth; per-tick binding reads validate generations (the
watermark's read-side rationale stated). Pager absorption names the
0336-recorded defects repaired at M2/M4 (centerRevision, transform
composition/reassertion, generation fencing, recognizer competition,
busy-turn) vs compatibility fixtures. The JS example becomes the
M8-complete command-bound form. The compound-claim record distinguishes
compound identity from its per-stream lease set and is proven against
the four combinator state machines at M5 before first non-pager use.
The amendments ledger gains the 0297 §4.5 access-topology change
(app writes become main-applied commands), the §4.3/§4.6 HBC-vs-source
decision, and the 0313 geometry-enum extension; the Contract motion
grammar's owner is named (a new Contract-side RFC or an 0089 revision —
their owners choose; explicitly not a 0281 stage-1 reopening). The
renumber guard is made concrete (schema-digest handshake bound to every
producer; pre-v4 producers rejected for renumbered sessions; cached
bundle/HBC invalidation; mixed-version startup test). DAG-A gets a
complete worst-case subtotal (~1.85ms < 2ms). The pointer record
preserves unknown kinds (interned raw string) and pressure presence.
Factual tightenings: events 15–17 from the March table were never
assigned; the axis defect is cross-domain (Swift non-0→vertical vs Rust
non-1→horizontal); the 60Hz pump is the iOS flag-off path; the two
routing sentences reconciled; cross-surface sampling labeled post-v1;
Choreographer runs on its attached Looper; platform-owned signifiers
(host/browser chrome) preserved in the witness rule; terminal-edge
worklet returns classified vs the no-allocation rule (typed verdict
enum at M6); worklet failure policy (exception/timeout/non-finite/
invalid verdict → deterministic cancel + diagnostic); depth-cap
rejection moved to install time where derivable; artifact-presence
checked by the verification registry at M3. Status unchanged: Draft.)
2026-07-12 (r5: round-4 dual-review fixes. The epoch fence is
made linearizable as a **generation sweep** (reset CASes every live slot
to a tombstoned generation before any teardown; the handle epoch field
becomes advisory — the slot generation is the only fence a writer can
trust). The descriptor sidecar becomes **two-phase** (detach records
validate independently and apply before the transaction's tree/presenter
mutation; attach records apply after presenter output), which is what
makes detach-before-destroy real; reject-and-retain becomes
**prune-then-retain** (the retained graph is pruned of bindings whose
targets the accompanying commit destroyed). M1 is **append-only** (no
slot reuse until M2's watermark transport exists; reuse then gates on the
minimum watermark across all consuming roots, and **v1 values are
single-root**). The gesture API gains an explicit **resolution
contract** (the `onEnd` worklet returns the commit/cancel verdict; final
at return; absent = cancel) and the three completion lifecycles
(router/SceneLease, pager/turn-token, component-local/Presence) are
separated; M6's low-level surface is restricted to non-semantic visual
motion until M8 wires command identities, and command-shaped gestures
declare their command binding (the 0336 witness correlation).
App-JS write ordering is pinned (per-handle sequence; shadow clears
against the observed write-generation, not "any publish"; an imperative
app write cancels an active driver — the same author-wins rule as
gestures). The main→runtime plane is corrected to the **event queue**
(the data-plane feed is runtime→main per 0297 §4.4) with honest
delivery semantics: at-least-once transport, exactly-once effect keyed
by (streamId, turnToken). Tier routing becomes capability-based rather
than trigger-based (state-triggered motion may use Tier 2). M6 gains an
explicit Contract-first leg (grammar owned by the 0281/0089 side, joint
deliverable; declared M6a/M6b split line). The pager reduced-motion cell
is tightened to **immediate** (0336 acceptance); other direct
manipulation gets numeric caps (immediate or opacity-only ≤150ms).
Evidence claims are made honest (0313's hardware record is a Debug-host
run; 0.248ms is elapsed-time around `driveGestureSample`; artifacts are
retained going forward, and Review-stage baselines re-run in Release at
M3). Budgets gain units + a named reference DAG. M2 codecs are narrowed
to generated identities + hand-authored codecs with byte-golden
fixtures. A phase × cause → terminal-state table aligns gesture outcomes
with 0336. A consolidated "Amendments to sibling authorities" section
is added (including the declared refinement of 0297 §4.5's slot shape to
the tagged word). The axis-decode parenthetical is corrected to the
actual shipped behavior (two disagreeing decode sites; the named M2
validation fixture). Seqlock records get the single-writer precondition;
the value-command publisher is named (Motion domain, main). The 60Hz
event-loop display-link fallback joins the clock inventory. Status
unchanged: Draft.)
2026-07-12 (r4: round-3 dual-review fixes. The SharedValue
write protocol is made linearizable — each single-word slot is one tagged
`AtomicU64` (generation+value) whose successful CAS is the linearization
point; cross-language access moves from raw-pointer casts to exported C
accessors; capacity is a segmented, stable-address directory. Transport is
reframed on the shipped commit-sidecar precedent (descriptor application
ordered after presenter output; a per-root consumption watermark makes
consume-before-reuse a mechanism; the u16 op-payload ceiling forces
chunked snapshots or a proven cap). A third protocol failure class is
added (byte-valid but semantically invalid → transactional
reject-and-retain), and any pre-1.0 renumber must bump the protocol
version or a schema digest (internal skew is real even without external
consumers). LLP 0336 P1/P2 are described as landed slices with named
acceptance gaps — the §6.1 lease lifecycle is M4 work on BOTH the native
and web implementations. The composition record shape is decided (one
externally-exclusive compound claim; semantics remain OQ7). Pointer
records are realigned to the shipped sample/envelope split
(`channels.ts`). Clock ownership now describes today's real tickers
(per-engine island pass; demand-started host links) and makes the
per-surface MotionClock registry an M3 deliverable, with
time-advances-with-the-world suspension semantics and idle-stop.
Correctness-bearing completions move off `runOnJS` (bounded, drop-oldest,
best-effort by construction) onto the reliable semantic terminal-event
path, and the API example teaches that shape. The reduced-motion table
separates RFC 0046's no-opt-out navigation fade from per-entry
`respectsReducedMotion` property transitions. The 0.248ms figure is
relabeled as simulated-cadence CPU p95 (not observed presentation).
Binding propagation gains a normative rule (topological, once-per-tick,
cycles rejected). App-JS read/write coherence is defined (pending-write
overlay = read-your-own-write; the wait-free read is logged against the
async-first rule). Web uses `Float32Array`; the Windows gate adds
precision-touchpad registration; assorted minor fixes from the Fable
round. Status unchanged: Draft.)
2026-07-11 (r3: dual-review round 2 fixes + an author directive.
Current-state honesty tightened: the shipped worklet install is source-text
(`install(id:source:generation:)` / `ex_worklet_install`), not the 0297
contract spelling; the 0297 W5 gesture proof is a macOS-only
JSON-args→direct-layer-write slice, not a slab→binding pipeline; the
`kernel/src/motion.rs` arbitration seam is a typed seed (no lease
type/checkpoints yet) and `ExactPagerMotion.swift` is a host-side
controller whose policy ownership migrates at M4; islands are
macOS-production with the iOS presenter adapter open (ENG-23516).
SharedValue lifetime gains a versioned validate-and-write primitive,
consume-before-reuse ordering, and a **fence-first** reset order (the r2
text had the epoch bump last — backwards per LLP 0297 §4.8 and the shipped
engine); app-JS reads are defined as direct validated atomic slab reads
(the r2 "mirror, ≤1 frame lag" model is withdrawn). A normative Platform
Execution section restores §Web (TS value store + analytic drivers on rAF,
WAAPI as optimization) and §Windows/§Android deferral gates, fixing two
dangling references. Composition stops implying `Exclusive`/`Race` lowering
is settled (OQ7 widened to all four combinators); dismissal is split into
router-owned vs component-local regimes; the accessibility rule
distinguishes command-shaped accelerators (witnessed signifiers, 0336
R1/R2) from continuous direct manipulation (accessible path-free
alternatives), and navigation release under reduced motion is RFC 0100's
150ms fade verbatim. Simulation moves to analytic solvers / fixed-quantum
accumulator (a dt cap alone is not rate-invariant); tick budgets are made
composable; the driver conformance fixture is explicitly future (M3) —
the existing `gesture-motion-fixtures` check is a marker check. Structural
protocol corruption fails closed (ProtocolError precedent) vs.
stale-but-well-formed no-ops. Pointer channels lower to record bundles,
not scalar slots. Per-surface clock ownership added. **Author directive
recorded (Charlie, 2026-07-11): Exact has no external protocol consumers
yet — the opcode set may be overhauled now if there is a real reason; a
pre-1.0 ABI-fluidity note is added and M2 holds the coordinated
Motion-block renumber option.** Status unchanged: Draft.)
2026-07-11 (r2: reconciliation revision. The March body predated
LLP 0297/0313/0322/0336 and RFC 0100; the July header notes patched the
seams but the body still specified the superseded all-on-main
kernel-register-VM design, proposed protocol numbers that now collide with
shipped ABI values, and planned work that has since shipped. This revision
rewrites the body onto the two-domain substrate (LLP 0297 §4.4): Motion is
a main-owned Rust domain plus the UI worklet runtime; the tree/layout
kernel lives on the runtime thread. Concrete opcode/event numbers are
withdrawn in favor of inventory-first allocation (`protocol-abi`
authority). A normative SharedValue registry-and-lifetime section receives
LLP 0297 OQ2 (the 2026-07-04 note's parenthetical "(OQ2 here = LLP 0297
OQ2)" misspoke — slot lifecycle is 0297's OQ2 and had no OQ number here;
it is now §"Registry and lifetime", distinct from this document's OQ2 on
layout-driving animations). Gesture arbitration adopts LLP 0336 §6.1's v1
arena as the core the composition API compiles onto. The implementation
plan is rewritten as absorption of the shipped substrate (0297 worklets +
slab, 0313 islands, 0336 P1 web recognizer + P2 native Motion slice, 0281
stage-0 channels) rather than greenfield phases. Interactive navigation is
delegated to RFC 0100. Reduced-motion policy is rewritten as a
per-interaction-class table aligned with RFC 0074/0100/0336. Status
unchanged: Draft; the general subsystem remains unbuilt — see "What exists
today").)
2026-07-06 (OQ2 layout-driving animations now route to LLP
0313's Accepted same-frame layout-islands answer for the bounded fast path;
ordinary layout-affecting bindings still use LLP 0297's constraint route.
Status unchanged: Draft, unbuilt.)
2026-07-05 (OQ2 layout-driving animations now route to LLP
0313's Draft same-frame layout-islands proposal. Status unchanged: Draft,
unbuilt.)
2026-07-04 (substrate note per LLP 0297 §6, upon its
acceptance: "kernel (Rust) on the UI thread" is refined to the two-domain
model of LLP 0297 §4.4 — the tree/layout domain (the `Kernel` struct) lives
on the dedicated runtime thread, while the Motion domain (gesture
recognizers, drivers, binding tables, geometry mirror) is a **main-owned
Rust structure**, not the `Kernel` struct. Tier-2 semantics in this
document are unchanged. "The worklet VM" (design constraint 8, §Worklet VM)
is realized as LLP 0297's UI worklet runtime — a persistent restricted
Hermes instance on main, superseding this document's kernel
register-machine sketch (its no-strings rule survives re-scoped to
math-class worklets, LLP 0297 §4.6). SharedValue **storage and access**
(slab, per-slot atomics, seqlock records) are fixed by LLP 0297 §4.5; this
document still owns the durable registry, driver, and binding model on top,
including slot allocation/reclaim (OQ2 here = LLP 0297 OQ2). Status
unchanged: Draft, unbuilt.)
2026-07-02 (dated OQ6 answer recorded per LLP 0281 §1 — Contract transient bindings/value channels are the language projection of SharedValues; pointer vocabulary is Contract surface, intake/per-host delivery is this document's substrate; escalation paths unified. Status unchanged: Draft, unbuilt.)

## Amendment A1 (2026-07-20) — Gesture-trace ingress identity (LLP 0374 W3, Phase-3a entry)

This document owns the gesture-trace surface and its vocabulary
(`GestureTraceEventStatus = ok|denied|failed|dropped|info`, phase records
`ok|failed|denied|unobserved|skipped`, and the derived
`GestureTraceReceiptLevel`). Two commitments charged by LLP 0374:

1. **Ingress identity acceptance and propagation** — not only vocabulary
   reconciliation: gesture-trace records accept the ingress-minted
   action-root identity (`interactionId`, LLP 0227 A1 / RFC 0102 A1) at
   dispatch and stamp it on every event/phase record, so the causal
   composite joins gesture evidence by propagated ID, never
   timing inference.
2. **Crosswalk participation.** Every current producer status at every
   record level — including the event-level `dropped` and `info` — maps
   into the LLP 0374 status crosswalk (Phase-3a entry deliverable),
   ratified by this owner; producer-native statuses are preserved beside
   the normalized axes. `GestureTraceReceiptLevel` remains a permitted
   derived per-surface summary (explicitly not condemned by the
   no-scalar-receiptLevel rule, which governs the composite only).
   Shipped wire vocabulary unchanged until this owner amends further;
   read-time mapping applies meanwhile.
