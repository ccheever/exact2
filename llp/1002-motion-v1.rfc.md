# LLP 1002: Motion v1 — one representation, two executors

**Type:** RFC
**Status:** Accepted (D2 and D6 confirmed by Charlie Cheever 2026-08-28; built the same day, LLP 1003)
**Systems:** Motion, Kernel, Wire, Web, Apple, Linux, Agent API
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Related:** LLP 1003 (the spec of what this built), LLP 1001 (kernel v1 — the rows motion targets), RFC 0492 (the exact1 motion program this supersedes as authority; research), RFC 0099 (exact1's motion substrate; research), LLP 0486 (one layout language, two engines — the pattern this applies to motion), LLP 0559 F1 (the Flutter warning this heeds), `rules/NOT-DOING.md` §Motion

## Summary

The motion crate exact2 inherited was the one subsystem that was **ported rather
than built**, and — the same fact in different clothes — the one subsystem where
**the web was not the standard**. RFC 0492 decided "one evaluator everywhere,"
including the web, where the browser already *is* the evaluator and beats a
per-frame wasm loop on every axis this repository budgets. This RFC reverses
that for exact2 and rebuilds motion on the rule everything else here follows:

> Motion is CSS's `transition` model. A property's target is its style row in
> the kernel; a `transition` row on the node says how it gets there; the value
> painted this frame is presentation state. On the web the browser executes it.
> Everywhere else, `exact-motion` executes it — and is held to the browser.

That is "one layout language, two engines" (LLP 0486) applied to motion, which
is exactly where the old corpus stopped short of applying it. The crate went from
15,021 lines to 1,461; the gesture arena, interactive-navigation model,
shared-value plane, and second value graph are gone; the seekable clock and the
closed-form spring — the parts worth keeping — stayed.

## 1. What was wrong

Found by reading, confirmed by counting (2026-08-28):

1. **Anti-web by decision.** `rules/NOT-DOING.md` carried "no delegation to
   Core Animation or CSS — one evaluator everywhere at a measured power cost."
   On the web that is a wasm evaluator writing `transform`/`opacity` from the
   main thread every frame, which a bare `<div style="transition: …">` beats on
   boot bytes (zero), thread (compositor), jank (survives a busy main thread),
   `prefers-reduced-motion`, and interruptibility. RFC 0492 §4.1 records that
   its own pre-draft *rejected* evaluator-everywhere for the power reason and
   that a docket inverted it for "one source of truth" — conflating one
   *representation* with one *executor*. The NOT-DOING line said this was "the
   same call the old repo made"; per RFC 0099's status table the old repo shipped
   CSS/CA delegation and only *decided* to invert, never shipped it.
2. **The stated reason for owning the web evaluator did not hold.** The
   virtual clock is "why motion is in v1 at all" — seekable, agent advances time.
   WAAPI's `Animation.currentTime` is that; `CALayer.timeOffset` is that. The
   genuinely non-native property was bit-identical *value sequences* for replay,
   and 0492 §4.2/M-A had already withdrawn the cross-machine identity claim.
3. **Nobody owned the frame.** LLP 1001 §3: "layout is a host call, because the
   host owns the frame clock." `motion/src/tick.rs`: an eight-phase order every
   tick source "must run," including `Present`, with no layout phase. Two frame
   models, no document saying what one frame is.
4. **No seam.** Motion bound sinks to a `node_id: u32` with no relation to the
   kernel's `NodeKey`; the kernel had `opacity` and `transform_*` rows the motion
   crate never read; neither crate referenced the other; nothing hit-tested.
5. **Size.** Kernel, fresh-built: 4.6K lines. Motion, ported: 15.1K, of which
   11K was a Flutter-style gesture arena (compound claims, leases, receipts v2,
   three checkpoints) and a CAS decision cell for drag-to-go-back — platform
   behaviors the sandwich model says the platform owns (LLP 0559 F1), sized for
   nothing the v1 app needs.

## 2. Decisions

**D1 — The representation is CSS's.** Targets are kernel style rows, renamed
to CSS's individual transform properties: `translate` (vec2, points), `scale`
(number), `rotate` (degrees), `opacity`. A new style row `transition` carries
CSS `transition` declarations (`transition-property`, `-duration`,
`-timing-function`, `-delay`; up to 8; `all` admitted; last matching wins).
There is no motion-private value graph, binding table, or shared-value plane:
the style row *is* the binding. A `SetStyle` that changes an animatable row on a
node with a matching `transition` row *is* the animation — the same sentence a
browser executes.

**D2 — Two executors; the web is the oracle.** A web host emits the rows as
CSS and does nothing per frame. Every other host runs `exact-motion`, which
implements CSS Transitions §3 — start from the before-change value, interrupt
from the current value, the reversing-adjusted start value and reversing
shortening factor (§3.2), delay and negative delay, combined-duration zero
starts nothing — and CSS Easing Level 1/2 (`linear`, the four keywords,
`cubic-bezier()`, `steps()` with all four jump positions, `linear()`), with
fixtures pinning its outputs to a browser's (`motion/tests/easing.rs`). One
declared deviation: **`spring(stiffness, damping, mass)`**, which CSS lacks. On
the web a spring is *lowered*, not evaluated: `exact_motion::spring::keyframes`
samples the closed form once (240 Hz, until rest) into keyframes the host hands
to `Element.animate` with `linear` easing; interior frames are bit-identical to
the native sample and midpoints are within ω²·A/(8·240²) — sub-pixel. The
evaluator runs on the web once per release as a compiler, never per frame.

**D3 — The clock is a seek.** `Engine::advance(t)` samples the clock;
accepted hold inputs seek it before changing presentation. Every running
curve is a closed-form function of `t`, so one call and sixty
give the same bits (`the_clock_is_a_seek`). `Engine::settle_time()` is what an
agent's `clock` operation advances to. On the web the same operation is
`Animation.currentTime`. No virtual-clock type, no tick-phase enum, no timer.
Engine time is seconds; host bridges convert milliseconds. If a gesture has
advanced presentation past an overdue timer, receipts retain Runner due-time
order while motion samples at `max(receipt_time, engine.now())`. The final
requested presentation seek remains monotonic; timer dispatch is not reordered.

**D4 — Gestures: the platform recognizes; the engine follows.** Recognition,
hit-testing, and scroll-vs-pan arbitration belong to `touch-action`/pointer
events on the web and the native platform's recognizers and scroll views.
Scroll always wins. Follow/release uses temporary ownership of an existing
node/property's presentation; its authored style remains the target:

- `begin_hold(node, property, now_s, presented)` returns an optional
  `HoldStart { token, value }`. Native hosts pass `None` to capture the current
  curve; the browser supplies computed presentation at recognition, before
  cancelling playback. This value is the displacement origin, including on
  rebegin; it is not the authored target or a pointer-down sample.
- `update_hold(token, now_s, value)` writes absolute presentation. Commits
  during the hold update the latest target and transition without repainting
  the held property. A hold alone is quiescent; other properties keep moving.
- `end_hold(token, now_s, HoldEnd::Release { velocity })` returns to the newest
  target under the newest transition. Velocity is in property units/second
  after drag resistance; `VelocityTracker` can estimate it from presentation
  samples. `Cancel` uses zero velocity. Easings ignore velocity; an absent or
  non-starting transition snaps. A spring with zero displacement still inherits
  nonzero velocity. Apply the final sample and any authored action while held,
  then end once; an action that destroys the row makes that end stale.

`HoldToken` is opaque and contains node, property and a checked, non-wrapping
u64 serial. Serials are unique across Engines sharing the process's linked
evaluator, not across independent Wasm instances or process reloads. Bridges
carry all 64 bits and check their runtime incarnation and live view identity.
Rebegin, release and removal invalidate old tokens; only live holds are retained,
with no history cache. `has_hold` lets hosts reject stale callbacks before their
own clock or batch mutations; `is_held` informs lowering. Stale updates/ends are
inert before time/value validation. Invalid live inputs leave ownership and time
unchanged; values and velocities must be finite, with `y = 0` for scalar rows.

D2 remains unchanged. Native hosts drain Engine presentation; browser hosts
preserve a held-property overlay across style commits, cancel only its playback
at takeover, and restore the latest authored declaration on release, including
`transition: none` authored while held. Release drains lowering even without a
kernel receipt, including other properties dirtied by the seek. Delays preserve
the release presentation. The web compares `spring_descriptor` (start, origin,
target, velocity, parameters) before compiling keyframes; unchanged curves do
not rebuild on pointer moves. CSS/WAAPI still execute motion without a per-frame
Wasm evaluator. These semantics and regressions do not establish physical 120 Hz
presentation.

**D5 — Deleted.** The gesture arena, claims, compound claims, leases,
arbitration receipts, recognizer state machines, compositions, interaction-state
store, publications, the interactive-navigation model and its decision cell, the
shared-value slab, derived values, property bindings, the plan node graph, the
decay driver, sequence/repeat drivers, the virtual-clock type and its input
events, the tick-phase order, and the reduced-motion action enum. No shims.

**D6 — The NOT-DOING trade** (written into `rules/NOT-DOING.md` §Motion).
Off the not-doing list: delegation to CSS on the web — it unblocks a web host
that ships zero motion bytes and a corpus with the browser as oracle. Onto it:
the gesture arena and interactive-navigation model (platform-owned), a second
value graph, layout transitions, decay/sequence/repeat, and reduced-motion
actions in the engine.

## 3. The frame

The host owns the clock (LLP 1001 §3 stands). One frame on a native host:
input → ops → `Kernel::apply` → `Kernel::compute_layout` → `Kernel::motion_sync
(&receipt).apply(&mut engine)` → `engine.advance(now)` → `engine.frame()` → the
host applies geometry and presentation values → present. On the web the motion
steps do not exist; the browser does them. The seam is one call and carries
exactly two things a browser reads from computed style: each created or touched
node's `transition` row and its four animatable targets, plus destroyed nodes to
forget. Nodes are keyed by the generation-checked `NodeKey` packed to `u64`, so a
reused slot never inherits motion (`a_destroyed_node_is_forgotten_and_its_slot_never_inherits`).

## 4. Not decided here

- **An Apple executor other than `exact-motion`.** D2 permits a host to lower a
  transition to Core Animation the way the web lowers to CSS; v1 Apple runs the
  evaluator because that is what exists. Whether CA delegation earns its place is
  a measured question for the Apple host lane, held to the same fixtures.
- **`prefers-reduced-motion`.** CSS handles it in the author's stylesheet with a
  media query; here the producer does the same — emits `transition: none` (or a
  shorter row) when the host reports the preference. The engine has no opinion.
- **Layout-affecting transitions** (`width`, `height`, insets). Not in v1; gated,
  as before, on an incremental-relayout number that has not been demonstrated.
- **The producer's authoring surface** for `transition` in Contract. This RFC
  fixes the row; the compiler that emits it is the Contract lane's.

## 5. Costs

- Two executors means the corpus is load-bearing: a browser value the fixtures
  do not pin is a divergence the rule cannot see. The fixtures pin the keyword
  midpoints, the step table, endpoints, clamping, and the reversing rule; a real
  browser-driven harness (0486's shape) is owed now that the web host exists
  (LLP 1007 §6) — the host emits `transition` as CSS and the smoke renders it,
  but nothing yet compares the browser's interpolation to the evaluator's.
- A spring on the web is 240 samples per release. Cheap; not free.
- Times and control points ride the wire as f32 like every other style row; a
  producer writing `0.1s` reads back `0.100000001s`. Harmless, and consistent.

## Ratification note

Built and verified 2026-08-28 (LLP 1003 §10): 113 tests across both crates,
clippy `-D warnings` and fmt clean, both crates on `wasm32-unknown-unknown`,
`caps` green. D2 and D6 reversed a line Charlie wrote in `rules/NOT-DOING.md`;
he confirmed both on 2026-08-28 and this document became Accepted. The spring
`duration: 0` rule (LLP 1003 §4) stands as built; he expressed no view on it.
