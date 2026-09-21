# LLP 1002: Motion v1 — one representation, two executors

**Type:** RFC
**Status:** Accepted (D2 and D6 confirmed by Charlie Cheever 2026-08-28; built the same day, LLP 1003)
**Systems:** Motion, Kernel, Wire, Web, Apple, Linux, Agent API
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Numeric-height trial:** Tuft / Zeno (Astra), implementing 2026-09-17; LLP 1041 §8.12.
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
  `HoldStart { token, value }`. Native compositor hosts pass `None` to capture the current
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

**Photo takeover foundation (Tuft / Newton, 2026-09-17).**
`begin_transform_hold(node, now_s, presented)` captures Translate and Scale at
one time and returns an opaque `TransformHold` containing their ordinary
`HoldStart`s. `update_transform_hold(hold, now_s, values)` updates both or
neither. Arrays use Translate/Scale order. Begin preflights both adopted slots,
samples and time, then reserves two serials together before mutation; either
stale token rejects a paired update before validation or clock movement.
Authored targets, existing scalar semantics and unrelated motion are unchanged.
There is no pair registry or paired end operation. Hosts validate the entire
terminal payload/time before an action or first end, act only while both old
tokens are live, then end each surviving old token independently. A successor
must never be cancelled by cleanup of the old pair. This provides atomic engine
takeover for the photo consumer; it does not add physical pan/pinch recognition,
photo bounds, shared-element return or another motion executor.

**Authored photo trial (Tuft / Zeno, 2026-09-17).** The common seam is
`transformDragFor="photo"` plus `transformgeometry(bw, bh, pw, ph)` and
`transformrelease(x, y, scale, vx, vy, vscale)`. Both handlers are required by
physical adapters. The kernel returns coherent handle/target/direct-clip
generations in O(depth): unique strict ancestor `id`, attached enabled path,
full-size border-box View, zero effective border/padding/margins/displacement,
positive uniform scale, no rotation, and identity transforms elsewhere on the
path. The direct View parent clips both axes with zero border/padding. Hosts
must additionally prove current centered fill, untransformed dimensions and
coordinate mapping; authored eligibility is not presentation proof.

Geometry is changed-only feedback for this one binding, not general measurement.
Zero dimensions suspend physical admission. Geometry/source/lifetime changes
invalidate the active pair before feedback; equal feedback does not loop or
cancel on unrelated typing. Release writes local Contract targets synchronously
while both tokens remain held, then follows the latest declarations. Controls
may change zoom during a hold; release must not overwrite it with caught scale.
The consumer owns contain/letterbox bounds and a 0/1 viewer list keyed by its
checked viewer token; state resets when that accepted source lifetime changes.
This common increment supplies no physical adapter, pinch or thumbnail return,
and establishes no frame-rate result.

Contract can author `translate` through its existing text path, including
`${panX}px ${panY}px`: one or two finite pixel lengths, with unitless zero and
one-axis y=0. Percent, calc, third-axis, and `none` remain unsupported; `none`
cannot be collapsed into identity translation without losing CSS semantics.
Explicit Rust `StyleValue::Vec2` remains available; no new vector expression or
generic bridge coercion is added.

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

**D7 — One numeric-height owner, measured trial (2026-09-17).** LLP 1041 §8.12
prices the kernel projection plus collection feedback at 4.542–4.875 µs p50
and 6.583–7.542 µs p95 for 25k logical records, against authored updates at
45.25–50.833 µs p50. These are same-binary Monospace CPU measurements, not
native fonts, paint, gesture delivery or 120 Hz presentation. They admit the
sheet trial, not general layout animation or a second application graph.

`Property::Height` is scalar logical pixels (discriminant 4); its CSS initial
`auto` has no numeric identity: `identity()` returns `None`. Ordinary
`targets(style)` and `motion_sync(receipt)` keep their four compositor rows.
The host explicitly registers one live `NodeKey`; at registration/boot and
every commit or layout entry, `height_motion_sync(owner)` checks that owner
in O(depth), even if the receipt did not touch it. `height_target(owner)`
accepts only finite nonnegative `Dimension::Points` on a non-inline box under
an attached root with no `display:none` ancestor. Unsupported, detached or
removed input retires Height only; percentages, env and auto are never resolved
into invented numeric targets. Replacing the registered owner retires its
previous Height first. There is no automatic adoption of all numeric heights.

**Authored handle increment (Tuft / Zeno and host owners, 2026-09-17).**
`heightDragFor="reading-sheet"` names one strict ancestor's authored `id`.
`Kernel::height_drag_target(handle: NodeKey)` resolves it in O(depth), requiring
one matching ancestor, numeric border-box height, and an attached, displayed,
enabled, non-inert path through the root. Duplicate matching ancestors refuse;
`testId` and `nativeId` do not participate. Hosts track authored handles during
existing tree updates and reconcile them after coherent receipts, not on every
motion tick. One target may have several handles; a second target refuses
without replacing the registered owner. Retire authored registration when its
last valid handle disappears; unrelated programmatic registration stays intact.

`heightrelease=snapSheet` supplies two trailing number arguments: displayed
height in logical pixels and signed logical pixels/second. The app chooses its
snap stop by synchronously writing its existing numeric state while Height is
held, before the host calls `end_hold` once. Physical delivery retains the handle
key, target key and live Height token; revalidate all three before clock, final
sample or action and after every receipt, including handle/ancestor-only changes.
Invalidation cancels without invoking the action. Generic synthesized release
events validate the finite pair and height domain `[0, f32::MAX]`; they are not
proof of a live pointer gesture. Native recognition and CSS/WAAPI remain the two
existing executor paths. The common authoring seam is not physical-input or
120 Hz evidence.

Physical admission requires both the IDREF and a release handler. A property-only
node cannot reserve the sole owner. Hosts retain known live handler declarations
across absent/empty/set/clear IDREFs and revalidate resolution on receipts. An
accepted different programmatic owner replaces automatic ownership; repeating
the same live registration is a no-op, including its ownership provenance.
Explicit clearing publishes unbound handles immediately; a later ordinary
receipt can admit still-authored handles again.

`remove_property(node, property)` removes its value, curve, dirty frame and
hold without changing time, other properties or the node's declaration.
`is_active` means held or running, including delay and a zero-distance spring
with velocity; `quiescent` still ignores held-only state. D3–D4 retain their
target, delay, release velocity and stale-token rules. Hosts also retire their
owned projection, held overlay and playback; a missing engine frame is not
a browser cancellation command.

Native adapters must use `PresentedHeight` for the registered owner and feed
the resulting actual scrollport to the collection protocol. CSS box sizing,
min/max constraints and the existing root-lowering exception still apply.
Takeover supplies the displayed CSS-height value, not an unconstrained engine
target or a content-box node's outer rectangle. The web keeps CSS easings and
WAAPI spring playback, using actual DOM geometry for collection feedback; no
per-frame Rust evaluator is added. Programmatic Apple/Linux host adapters and
the web hold/lowering path now implement this seam. Native trial registration
requires a single root and explicit border-box owner; content-box capture is
refused until resolved padding/border conversion is implemented. Repeating a
live registration is idempotent even while hidden/unsupported; a destroyed key
cannot re-adopt. An unchanged held Height does not add layout work to unrelated
compositor ticks. Mandatory commits, resize and feedback still refresh layout.

External Height positions must be finite in `[0, f32::MAX]`, with scalar y=0.
Unknown/unadopted/stale callbacks return before validation; malformed live
positions refuse before clock or hold mutation. Signed release velocities and
internal curve values remain unchanged. Only displayed Height clamps a negative
sample to zero, retaining its timing and later rebound. These Rust host and DOM
regressions alone do not establish native frame performance. The authored handle
and real-input results are recorded separately in LLP 1041 §8.12.

**D8 — One collection-owned Arrange preview.** Tuft / Leibniz, 2026-09-17.
The dedicated non-button grip authors `reorderFor="list-id"`; its strict ancestor
virtual List authors `reorderdrop(item: string, before: option<string>)`. Runner
joins that IDREF and handler to a current positive measured private wrapper and
string each key. Matching ancestor IDs must be unique, including ineligible
matches; the handle-to-root path must be attached, displayed, enabled and
non-inert. Height/Photo props cannot also claim the grip. Other path transforms
remain identity except the Runner-owned wrapper Translate; hosts separately
validate actual presentation, mapping and runtime incarnation. The resolver costs
O(depth + mounted tree), never a scan/export of all logical keys per move.

Hosts first acquire the existing interaction pin, then begin one opaque
process-unique token. No destination pin or second items graph exists. Gap hits
use unpreviewed logical boxes and current measured local boundaries, not moved
presentation boxes or a far pinned island. True end is logical end inside the
actual scrollport. Unknown geometry returns NeedsMeasurement, preserving displayed targets but
invalidating terminal eligibility until a new accepted final sample. After excluding the source, zero-height
runs choose the rightmost coincident boundary; a minimum-measurement-epoch sum-tree
summary proves it without scanning the run (512KiB additional numeric storage at
25k entries). The ordinary items Rc, source order, placeholders and extent stay
unchanged during preview. Mounted wrappers receive absolute Translate targets
with `translate spring(300,30,1)`; newly mounted rows derive from the same single
descriptor. A bounded current-output scalar avoids repeated equal style writes;
there is no per-row preview history. Active preview suppresses follow-end.

A terminal drop checks the exact latest scroll facts, live binding/token and
certified gap before consuming action eligibility. It changes Active to Terminal
in the same descriptor, resets preview targets, and dispatches exact private keys
once while the source pin and host hold remain live. Own structural reconciliation
keeps the source row pinned by its retained key if it survives, including
replacement of the authored grip before a second reconciliation. Ordinary app refusal is a successful
current-state result, not a poisoning data error. The gallery's synchronous
terminal action supplies its current checked revision; arbitrary asynchronous
reorder completion is not certified by this slice.

The host samples O(W) current viewport presentations before dispatch, applies the
receipt/layout/bounded feedback, and rebases surviving generational wrappers from
old viewport position minus the new untransformed base (including actual anchor
scroll). Release the source with its observed parent-space velocity; temporarily
catch/release surviving neighbors with zero velocity. This is C0 position
continuity, not C1 velocity preservation, and adds no Engine API. Common
`reorder_frame(token)` exposes only surviving wrapper/root NodeKeys and logical
tops/targets, never keys or claimed presentation samples. Elevation, clipping,
actual input and before-paint lowering remain separate host obligations.

`finish_reorder(token)` retires the terminal descriptor and releases only its
still-owned interaction pin. Defer finish through source return settling when
needed, or explicitly clean the old owner before rapid regrab. No new begin is
admitted while that descriptor remains. Cancel, body/width/typography/structural
invalidation, and accepted pin transfer retire action eligibility without an
action; stale feedback is inert. Pin transfer marks the old owner unowned, so
late finish cannot clear a successor, even if it uses the same handle. Deletion
requires no source to survive. These common tests do not claim a physical drag,
Linux elevation, host C0 implementation or frame performance.

## 3. The frame

The host owns the clock (LLP 1001 §3 stands). An ordinary compositor frame on a native host:
input → ops → `Kernel::apply` → `Kernel::compute_layout` → `Kernel::motion_sync
(&receipt).apply(&mut engine)` → `engine.advance(now)` → `engine.frame()` → the
host applies geometry and presentation values → present. On the web the motion
steps do not exist; the browser does them. The seam is one call and carries
exactly two things a browser reads from computed style: each created or touched
node's `transition` row and its four animatable targets, plus destroyed nodes to
forget. Nodes are keyed by the generation-checked `NodeKey` packed to `u64`, so a
reused slot never inherits motion (`a_destroyed_node_is_forgotten_and_its_slot_never_inherits`).

For the D7 trial, the native adapter reconciles its Height owner and seeks
motion before `compute_layout_presented`, then reports actual collection
geometry before presentation. Every subsequent layout entry retains that
validated sample until retirement; an ordinary layout call would clear it.

## 4. Not decided here

- **An Apple executor other than `exact-motion`.** D2 permits a host to lower a
  transition to Core Animation the way the web lowers to CSS; v1 Apple runs the
  evaluator because that is what exists. Whether CA delegation earns its place is
  a measured question for the Apple host lane, held to the same fixtures.
- **`prefers-reduced-motion`.** CSS handles it in the author's stylesheet with a
  media query; here the producer does the same — emits `transition: none` (or a
  shorter row) when the host reports the preference. The engine has no opinion.
- **General layout-affecting transitions** (`width`, insets, arbitrary heights).
  Only the explicitly registered numeric-height trial in D7 is admitted; broader
  ownership and layout policies remain gated on their own measured consumers.
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

**Shop gallery coalesced pan (Codex, 2026-09-19).** UIKit translation already measures from touch-down. The photo adapter previously subtracted the entire translation received at `.began`; a coalesced began/ended pair at the same position therefore applied zero displacement despite valid geometry and an accepted hold. The adapter now preserves that movement. The formerly failing Mac-hosted iOS drag, repeated bounded panning and double-tap return to fit pass in the Shop clone. This is a native mouse-delivered iOS gesture check, not physical-iPhone, pinch or frame-pacing evidence.

**Web swipe capture transfer (Codex, 2026-09-20).** Touch starts on a Messages bubble’s text child, which receives implicit pointer capture. On horizontal recognition the bubble takes capture; the child’s bubbling `lostpointercapture` previously cancelled that same swipe. `attachSwipe` now responds only to capture loss targeted at its own element. Collection feedback preserves the row’s interaction pin while an ancestor has taken capture, then releases it on actual cancellation or pointer-up. Real headless Chrome touch input opens the reply thread; the existing browser motion suite checks both descendant transfer and actual owner capture loss, with pointer cancellation coverage retained. This repairs gesture delivery, with no frame-pacing claim.
