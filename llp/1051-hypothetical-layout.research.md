# LLP 1051: Hypothetical layout — where things would be, before they are there

**Type:** Research
**Status:** Draft. A record of the follow-up to Charlie's 2026-09-24 conversation with Jordan Walke (LLP 1052). It decides nothing and names no implementer; LLP 1051.000 outlines an implementation. Not linked into `llp/current/`, which is at its cap of 15 on origin/main.
**Systems:** Kernel (the layout engine; `measure_height_targets`, `compute_layout_presented`, the staged view in `txn`), Motion (`exact-motion`: closed-form timing, holds, the seekable clock), Runner (event handlers; timers), Web host (the browser lays out; DOM measurement), Apple and Linux hosts (the kernel lays out; CoreText and cosmic-text measure), Agent API (`layout`, `clock`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** LLP 1052 (the conversation notes); LLP 1001 §3 (`SetChildren` reparents with identity kept), §6 (text measurement); LLP 1002 D4 (follow and release), D7 (the numeric sheet), D8 (the reorder preview); LLP 1007 §9 (no text-measurement bridge for kernel layout on the web); LLP 1010 §6 (windowed lists estimate unmounted rows); LLP 1012 §2 (the clock); LLP 1013 (view transitions: snapshots around a commit); LLP 1039 D5 (no container queries); LLP 1041 §8.5 (the four interactions), §8.12 (presented heights, `measure_height_targets`); LLP 1043 (Pretext, measured); LLP 1043.000 (text around shapes); LLP 1046.002 F16 and LLP 1046.003 (the game add-on and its physics); LLP 1050 F6 (fling landing points); research LLP 0099 (`model | predicted | presented`), 0100 (off-screen route layout for shared elements), 0297 (layout islands), 0486–0488 (counterfactual `fits`; the container-size feedback channel); `rules/NOT-DOING.md` §Motion. Sub-document: LLP 1051.000.

## Summary

Charlie, 2026-09-24, relaying Jordan Walke: *"He had some system called
Preflex or something that was kind of like Cheng Lou's Pretext thing but
more general. I didn't get a great look at it but it seemed like you could
on the main thread, compute a hypothetical layout and then adjust your
gestures or animations or whatever accordingly based on where things would
be and how much space they would take up, etc. This seems broadly useful?
Does our stuff offer that sufficiently flexibly and powerfully to do
everything you would want to do?"* That day's answer, from a sweep of the
code, was no.

The follow-up, 2026-09-25: *"write up an LLP for a general hypothetical
layout system that would be useful … background and research on existing
systems and examples of use cases where it would be valuable. Maybe one
case we should think through is you fling a ball against a wall and you
need to think through the physics of when it might hit the wall and bounce
in between frames."* This is that record.

**A hypothetical layout** is a layout computed for inputs that are not the
committed ones — a different tree, a different offer or a different time.
Code reads it; it is never presented.

| | |
|---|---|
| Every engine already does it internally | CSS specifies the flex algorithm through a "hypothetical main size". Taffy runs `ComputeSize` passes and caches nine size answers per node. The only decision is whether app code may ask (§2.1) |
| Four shapes of exposure | Force a layout and read it (the web, UIKit). Ask a size question (SwiftUI, Flutter's dry layout, Compose intrinsics). Compute the next layout before committing it (Compose `LookaheadScope`, Texture, Fabric, exact1's layout islands). Project motion to where it stops (UIScrollView's `targetContentOffset`, `OverScroller.getFinalX`) (§2.2–§2.5) |
| Four ways it fails | A second implementation drifts from the real one (Flutter's dry layout). Re-measurement compounds with depth (so Compose bans measuring twice). Geometry that feeds render-time state loops (exact1's container-size channel). An answer computed off the thread that needs it arrives a frame late (§2) |
| exact2 today | No app-facing query. One what-if pass, native only, heights only, with the root offer as the only input that varies (`measure_height_targets`). Consumers compensate with per-feature code in each host, or with hard-coded numbers (§3) |
| The web is the hard part | The browser lays out. The exact answer there is a forced layout of a temporary change. The fast answer is a second engine that predicts the browser and can disagree with it: Pretext's trade, and by its description Preflex's (§2.7) |
| Time is the axis nobody unifies | For the ball, per-frame stepping rests it 19 pt apart at 60 Hz and 120 Hz. It tunnels through a thin divider on 15% of releases at 60 Hz, and it breaks the seekable clock. Event-driven closed-form paths against layout at time t are exact at any step size and know the resting point at release (§5) |

§4 lists the uses and the exact2 consumer each has today. §6 gives the
findings and §7 the questions. LLP 1051.000 covers what a useful system would
look like and how to build it in exact2.

## 1. The question, made precise

### 1.1 Three answers to "where is it?"

exact1 named them (LLP 0099, "geometry honesty during motion", taking LLP
0313's vocabulary): *model* (layout truth), *predicted* ("an island's
predicted next-commit layout") and *presented* ("what is on glass now").
This record calls them committed, hypothetical and presented.

- **Committed.** The kernel's published frames for the committed tree and
  offer, which is what `Kernel::row` reads. There is one answer per epoch.
- **Presented.** What a frame shows at its presentation time: committed
  layout composed with motion's current values. Under LLP 1041 §8.12 it also
  includes sampled heights, which are laid out again. This is UIKit's
  presentation layer, and what the agent's `layout` reports on the web
  (transforms and scroll offsets folded in, `host/web/glue.js:1160`).
- **Hypothetical.** Layout computed for inputs that differ from the committed
  ones. Code reads it and it is never presented. There can be any number per
  epoch.

### 1.2 Three axes

| Axis | The question | Typical uses |
|---|---|---|
| Tree | Where would X be if the tree were T′? | Reparenting; insertion and removal; reorder and drop previews; the next route's shared elements |
| Constraint | How big would X be under offer O? | Whether something fits; choosing among alternatives; a paragraph's height at a width; a rotation's target size |
| Time | Where will X be at t, and where will it stop? | Layout partway through a transition; a fling's landing point; the moment of a collision |

The axes compose. "Where will this card rest if I reparent it now, while the
keyboard is still rising?" uses all three. An answer is a value: frames and,
for time, velocities, stamped with the epoch they assume.

### 1.3 Two rules exact2 has already paid for

- **Read at handler time, never at render time.** A derive or view that
  reads geometry makes layout an input to the state that produces the
  layout.
  - exact1 built exactly that channel: "layout → measure → runtime read →
    (next frame) layout" (LLP 0488:763). Sizes oscillated across
    breakpoints. exact1 settled on detect-and-pin and deleted its look-ahead
    (LLP 0487:187-197).
  - exact2 declines container queries for the same reason (LLP 1039 D5).
  - A read inside an event handler is edge-triggered: one read, one decision,
    one commit. Everything Jordan described, adjusting gestures and
    animations to where things would be, happens at handler time.
- **Paint-only rows are the safe continuous channel.** In CSS, `translate`,
  `scale`, `rotate` and `opacity` do not affect layout. Geometry may drive
  them every frame, as a path or a spring toward a hypothetical target,
  without creating the loop. Geometry that continuously drives
  layout-affecting rows is the loop again.

## 2. How other systems answer

The systems are grouped by the shape of the answer, because that is what a
design chooses.

### 2.1 Inside every engine

- **CSS specifies layout through hypotheticals.**
  - The flexbox algorithm gives each item a flex base size and a
    "hypothetical main size", then a "hypothetical cross size", before it
    distributes free space.
  - Grid track sizing measures items under trial sizes.
  - Knuth–Plass line breaking (TeX) scores many candidate breaks.
    `text-wrap: balance` and `pretty` are bounded versions of the same
    search.
- **Taffy** (vendored 0.14.0) separates `RunMode::PerformLayout` from
  `RunMode::ComputeSize`. In `ComputeSize`, "layout steps that aren't
  necessary for determining the container size … can be skipped".
  - Each node caches one final layout plus nine size answers
    (`vendor/taffy/src/tree/cache.rs:11`), keyed on every layout input. Exact's
    patch 11 (2026-09-25) added three inputs the key had left out, after a
    cached answer to one hypothesis was returned for another.
  - The kernel keeps up to 16 measured offers per text leaf
    (`kernel/src/layout.rs:68`).

The machinery exists and is cached per hypothesis: the cache key *is* the
hypothesis, and an incomplete key gives wrong answers, as patch 11 found.
Whether app code may ask is a separate decision.

### 2.2 Force a layout and read it

- **The web.** Reading geometry after a DOM change (`getBoundingClientRect()`)
  forces a synchronous style and layout pass. Nothing paints until the task
  ends.
  - FLIP (Paul Lewis, 2015): record the First box, apply the change and
    measure the Last, Invert with a transform, then Play the transform back to
    identity. Motion (formerly Framer Motion) does the same for `layout` and
    `layoutId` changes, animating transforms with scale correction for
    children.
  - The View Transitions API does it across a DOM update. It snapshots the
    named elements, applies the update, captures the new boxes and animates
    between the two. Same-document transitions are in Chrome 111, Safari 18
    and Firefox 144.
  - The cost is a layout per read, plus layout thrashing when reads and writes
    interleave.
- **UIKit.** Call `layoutIfNeeded()` inside an animation block: the model
  layer takes the new frames at once, and Core Animation interpolates the
  presentation layer.
  - Under `UIView.animate`, hit testing uses the model layer, which is the
    destination, unless code asks for the layer's `presentation()`.
  - `UIViewPropertyAnimator` (iOS 10) hit-tests views where they appear, by
    default.
- **exact2 does this today, per feature and per host.**
  - `positionContexts` (`host/web/glue.js:233`) reads a dozen boxes to place a
    context preview, flip it inside the viewport and shift its siblings.
    Apple has its own version.
  - LLP 1013 proposes View Transitions for shared elements: commit, then
    animate snapshots. That suits discrete changes, but it cannot give an
    answer before the commit.

This shape is exact: there is one engine, so nothing can drift. The price is a
layout per read, and the temporary change must not disturb any live state. On
the web, making a temporary mutation, reading, and reverting inside one task
is the only exact answer. That makes it the oracle for anything exact2
computes instead.

### 2.3 Ask a size question

- **SwiftUI** (iOS 16).
  - The `Layout` protocol's `sizeThatFits(proposal:subviews:cache:)` may be
    called several times per pass with different `ProposedViewSize`s: zero,
    infinity, unspecified and concrete.
  - `ViewThatFits` tries its children in order and shows the first that fits.
  - `AnyLayout` switches layouts while keeping identity, so the switch
    animates.
- **UIKit.** `sizeThatFits(_:)`, and Auto Layout's
  `systemLayoutSizeFitting(_:withHorizontalFittingPriority:verticalFittingPriority:)`.
- **Flutter.** `getDryLayout(constraints)` (Flutter 2.0, 2021) is backed by
  each render object's `computeDryLayout`. It answers "what size would you
  be" without laying out.
  - It is a second code path in every render object, and it must agree with
    `performLayout` by hand. Dry baselines followed in 3.22.
  - The docs warn that intrinsic queries are "expensive as it can result in
    O(N^2) behavior".
- **Jetpack Compose** throws if a child is measured twice in one pass.
  - Intrinsic measurements (`IntrinsicSize.Min`, `.Max`) are the sanctioned
    question to ask before measuring.
  - `SubcomposeLayout` composes content only once constraints are known, as
    `BoxWithConstraints` does.
- **Yoga** calls a measure function with a mode per axis (undefined, exactly,
  at most), possibly several times per layout. It caches eight answers plus
  one layout per node.
- **Android views** may measure a child more than once per pass.
  `RelativeLayout` and nested `layout_weight` measure twice per level, so the
  cost compounds with depth.
- **The CSS Layout API** (Houdini) would let a custom layout lay out a child
  several times (`layoutNextFragment`). It is still behind a flag in
  Chromium.

Pure questions compose. But re-measurement compounds unless it is cached or
banned, and an answer computed by separate code drifts from the real one.

### 2.4 Compute the next layout before committing it

- **Compose `LookaheadScope`** (1.5, stable in 1.6; `LookaheadLayout` in
  1.3). A lookahead pass computes the destination layout, and each element
  approaches it over frames (`approachLayout`, 1.7).
  `SharedTransitionLayout` and `Modifier.sharedElement` (1.7, still
  experimental) are built on it.
- **Texture** (AsyncDisplayKit).
  `transitionLayoutWithAnimation:shouldMeasureAsync:measurementCompletion:`
  computes a pending layout, possibly off the main thread.
  `animateLayoutTransition:` then receives each node's `initialFrameForNode:`
  and `finalFrameForNode:`.
- **React Native's Fabric** lays out an immutable new revision of the shadow
  tree before mounting it. Reanimated's layout animations and shared
  transitions read before-and-after frames at that commit.
- **UICollectionView** has `initialLayoutAttributesForAppearingItem(at:)` and
  `finalLayoutAttributesForDisappearingItem(at:)`.
  `UICollectionViewTransitionLayout` interpolates between two whole layouts by
  `transitionProgress`, which a gesture can drive
  (`startInteractiveTransition(to:completion:)`).
- **Android MotionLayout** solves two `ConstraintSet`s and interpolates
  between them by progress. `OnSwipe` uses the release velocity to decide
  whether to finish.
- **exact1** tried two versions of this, and exact2 carried over neither.
  - RFC 0100 rendered the incoming route off-screen to measure shared
    elements (`llp/research/0100-interactive-navigation-transitions.rfc.md:1594-1620`).
    It never got past swipe-back.
  - Its layout islands were bounded UI-thread Taffy sub-passes (LLP 0297 OQ4),
    measured at 0.214 ms at 120 Hz (LLP 0099:487). They produced the
    *predicted* answer.
- **Preflex**, as Charlie saw it, is the same idea run synchronously on the
  main thread, so a gesture or animation can use the answer in the frame that
  needs it.

With both layouts in hand, reparenting, shared elements and layout animation
are straightforward. Where the lookahead runs off the thread that needs it,
the answer lands a frame late. Texture measures asynchronously, and
ComponentKit and Litho lay out in the background. A late answer is fine for a
transition the app starts, and wrong for a gesture that has to decide now.
That is Jordan's point about the main thread.

### 2.5 Project motion to where it stops

- **UIScrollView** hands its delegate the projected resting offset at release
  (`scrollViewWillEndDragging(_:withVelocity:targetContentOffset:)`) and lets
  the delegate change it. Paging and snapping work this way. The deceleration
  rate `.normal` is 0.998 per millisecond. WWDC 2018's "Designing Fluid
  Interfaces" gives the same projection for any value:
  `(v / 1000) · d / (1 − d)`.
- **Android** has `OverScroller.fling()` followed by
  `getFinalX()`/`getFinalY()`. `FlingAnimation` takes friction and min/max
  bounds; it stops at a bound rather than bouncing. On Windows,
  InteractionTracker exposes a `NaturalRestingPosition` (LLP 1050 F6).
- **CSS Scroll Snap** gives a fling, "interpreted with momentum", an intended
  end position, and chooses the snap position nearest to it.
- **The OS announces its own motion before it happens.**
  - iOS keyboard notifications carry the end frame, duration and curve.
  - `viewWillTransition(to:with:)` carries the rotated size.
  - Android goes further. For an inset animation (API 30) it lays the views
    out in the end state before `onStart`, and the app animates from the start
    state in `onProgress`.
  - In each case the app lays out for the destination and animates alongside.
- **Interruption keeps velocity.** `UIViewPropertyAnimator` scrubs
  `fractionComplete`, reverses, and continues with new timing. Since iOS 8,
  UIView animations of certain properties are additive. CSS transitions
  reverse using a "reversing shortening factor".
- **UIKit Dynamics** collides items with the reference view's bounds
  (`translatesReferenceBoundsIntoBoundary`). That is physics against layout,
  but as a second owner of `center` and `transform`, which fights Auto Layout.
- **exact2**, in apps and hosts:
  - `snapSheet` projects `height + velocity · 0.15` against hard-coded stops
    of 180, 360 and 640 (`apps/interaction-gallery/app.contract:77-78`,
    `apps/exact-live/app.contract:313-314`). On an AppKit drive the tallest
    stop lands at 562.79 pt, clamped by `max-height: 100%` (LLP 1041 §8.12).
    The projection cannot see geometry.
  - The photo viewer's `releasePhoto` clamps the release point to bounds taken
    from layout and ignores the velocity it receives
    (`apps/interaction-gallery/app.contract:289-291`).
  - The one projection against layout is in the iOS host. It snaps UIKit's
    projected offset to the nearest snap position taken from layout
    (`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:690-707`).
  - LLP 1050 F6 would build the rows at a fling's landing point first.

Knowing the destination at release lets an app commit it (the model) while the
path is presented. What no platform offers is a projection that sees
obstacles defined by layout. That is the ball.

### 2.6 Physics: the time of impact

- **Discrete stepping tunnels.** Anything that moves farther per step than an
  obstacle is thick can pass through it between two checks.
- **Continuous collision detection** solves for the time of impact instead.
  - Erin Catto's GDC 2013 talk surveys the methods, from conservative
    advancement (after Mirtich) to the local separating-axis search that
    Box2D's `b2TimeOfImpact` uses. That search finds "the largest time at
    which separation is maintained".
  - Box2D v3 (2024) runs it by default for dynamic-versus-static contact, and
    for bodies flagged as bullets.
  - Unity names the choices: `Discrete`, `Continuous`, `ContinuousDynamic`,
    `ContinuousSpeculative`.
  - Rapier has had it per body since 0.7. Since 0.35 it sweeps fast bodies
    against fixed colliders automatically.
- **Hypothetical movement is a query in every engine.** Godot has `test_move`
  and `move_and_collide(…, test_only)`. Shape casts such as Rapier's
  `cast_shape` and Unity's `SphereCast` answer "if I moved like this, what
  would I hit, and when?"
- **Fixed steps with interpolation** (Glenn Fiedler, "Fix Your Timestep!",
  2004): simulate at a fixed rate and render between the last two states.
  This is deterministic, at the cost of up to one step of latency.
- **Event-driven simulation** jumps from event to event.
  - Hard-sphere molecular dynamics (Alder and Wainwright, 1957 and 1959)
    predicts each next collision analytically.
  - Mirtich's Timewarp (SIGGRAPH 2000) advances bodies asynchronously and
    rolls back when a prediction breaks.
  - The bouncing ball is the textbook Zeno hybrid system (Johansson et al.,
    1999): with restitution below 1 it bounces infinitely often in finite
    time.
- **exact2's game add-on** runs vendored Rapier 0.35.3 at a fixed 60 Hz tick
  and renders one tick behind, interpolated.
  - Its wrapper never enables per-body CCD. Rapier's automatic sweep clamps a
    fast body to the earliest impact, "velocities untouched, no re-solve"
    (`vendor/rapier3d/src/dynamics/ccd/ccd_solver.rs:19`). The body then
    bounces on the next tick and loses the rest of that step's travel: the
    clamp row of §5.4.
  - Its `raycast`, `overlap` and `sweep` queries run on a separate query
    scene, and "reads never mutate saved solver state"
    (`game/physics/src/queries.rs:36`).
  - It keeps metres and ticks out of CSS layout on purpose. LLP 1046.002 F16
    says the two worlds "meet at exactly one point … a placement".
- **exact2's app-side physics** works around not being able to read layout.
  - Reflow's balls are closed-form triangle waves,
    `reflect(x0 + vx·elapsed, …)` (`apps/reflow/data/src/balls.rs:34-50`),
    which are exact at any sample time. Their walls, though, are arithmetic on
    `exactViewport()` plus a hard-coded height of 560 or 440
    (`apps/reflow/app.contract:167-168`), not layout. Their time advances 16 ms
    per timer fire (`:196`, `:209`).
  - The textflow orbs replay fixed 16 ms Euler steps from t = 0 on every query
    (`apps/textflow/data/src/geometry.rs:85-155`). They mirror off walls
    hard-coded to a 656 × 440 court, while the visible box is 450 or 480 pt
    tall (`apps/textflow/app.contract:35`). The physics floor is not the box's
    floor.

Correctness under large or uneven steps comes from solving for event times,
not from taking smaller steps. Every simulation needs a rest rule.

### 2.7 Heights before layout: Pretext, and what Preflex seems to be

- **Pretext** (Cheng Lou; LLP 1043) prepares text once, segmenting it and
  measuring the segments with canvas. It then lays the text out per width with
  arithmetic, because asking a browser for a height costs a reflow.
  - That takes 0.16–0.22 µs per paragraph, against 63–92 µs for CoreText to
    re-break the same paragraph (LLP 1043 F4).
  - It predicts the platform, and it can disagree. A naive breaker over
    separately measured segments gave CoreText a different line count in 22 of
    10,000 paragraph-widths (LLP 1043 F5).
- **exact2 has the idea only in app code and in estimates.**
  - Reflow's data crate measures with advances read at build time, and sets
    text 2 pt wider than it measured.
  - Windowed lists mount at most sixteen rows at a provisional 32 pt before
    host geometry arrives (LLP 1010 §6).
  - The web's equivalent is `content-visibility: auto` with
    `contain-intrinsic-size: auto`, which keeps the "last remembered size".
- **Preflex**, going by its name and Charlie's description, takes Pretext's
  approach from text to flex layout: a layout engine the app calls on the main
  thread to predict what the platform will do.
  - Nothing public goes by that name (searched 2026-09-25).
  - Jordan did write a flexbox engine, `jordwalke/flex`: a Reason port of Yoga
    from 2016 that also compiles to JavaScript, and the layout engine Revery
    uses. A layout engine that app code can call is familiar ground for him.
- **That is one side of a real trade.** A predictor is exact when it *is* the
  platform's engine, as on exact2's native hosts, where the kernel does the
  layout. It drifts when it is not, as on the web, where the browser lays out.
  We have not seen Preflex's API; asking Jordan is the cheapest design input
  available.

## 3. Where exact2 stands

This table was verified at origin/main `121d629e` (2026-09-25) and extends the
2026-09-24 sweep.

| Piece | What it does | Limits |
|---|---|---|
| `Kernel::measure_height_targets` (`kernel/src/kernel.rs:562`) | The only built what-if pass. It removes live height projections, lays out under an offer, reads border-box heights and restores the projections. It publishes nothing (`kernel/tests/it/presented_height.rs:1002`, `:1040`) | Heights only. The root's offer is the only input that varies. Host-only and native only. Refused while a content region is registered |
| `compute_layout_presented` and `PresentedHeight` (`:502`, `:39`) | Lays out with sampled heights as derived style, which gives layout at time t for registered heights. Frames, hit testing and collection feedback observe the sample | Numeric or border-box `auto` heights only. It publishes |
| `txn::Staged` (`kernel/src/txn.rs:71`) | "The batch's view of the tree during validation: the arena plus overrides", which makes it a structural overlay | Validation only; no layout |
| `Kernel::rehydrate` (`:910`) | Forks a kernel from its columns. It is the result-equality gate's oracle | Costs a whole tree per fork |
| `Region::projected_frames` (`kernel/src/region.rs:408`) | Moves already laid-out frames to a new origin without mutating anything | A translation, not a layout |
| `Kernel::on_demand` (`:246`) | The web's kernel builds no engine tree until one is asked for | On the web the browser lays out and the measurer is a placeholder, so kernel layout there would not match the page |
| Event payloads | `heightrelease(height, velocity)`, `transformgeometry(bw, bh, pw, ph)`, `transformrelease(x, y, scale, vx, vy, vscale)`, `reorderdrop(item, before)`, `scroll`, `pan` | The only geometry an app sees. On Apple, `transformgeometry` arrives a turn late |
| Host code per feature | The context preview (the web's `positionContexts`, and Apple's own); the reorder preview, which hit-tests "unpreviewed logical boxes … not moved presentation boxes" and returns `NeedsMeasurement` when geometry is unknown (LLP 1002 D8); list row feedback | Written once per feature per host |
| App-side prediction | Reflow's data crate; Reflow's balls; the textflow orbs | Disagrees with the platform at the margins. Walls cannot come from layout |
| `exact-motion` | Closed-form springs: `SpringConfig::sample` returns position *and* velocity at any t (`motion/src/spring.rs:102`). The engine's advance is a seek. `libm`'s software kernels are pinned so every host computes the same bits (`motion/src/math.rs`) | No decay or collision driver (NOT-DOING). The engine exposes no velocity read. A spring rests when displacement and speed are both under 1e-3, found on a 240 Hz grid capped at 10 s (`motion/src/spring.rs:53`, `:59`, `:145`) |
| The clock that samples motion | Apple: `CACurrentMediaTime()` when the display-link callback runs; its canvases use `targetTimestamp` (`host/apple/Sources/ExactKit/Session.swift:920`: "Motion keeps its existing sampling clock; canvas frames target presentation"). Linux: monotonic time in the poll loop. Web: the browser plays keyframes sampled at 240 Hz (`motion/src/transition.rs:329`) | UI motion samples when the callback runs, not when the frame will be shown |
| Release velocity | Apple differences the last two samples. The web fits a slope over up to eight samples in 80 ms. Linux uses `VelocityTracker`, a weighted least-squares fit over 100 ms (`motion/src/velocity.rs:19`) | Three estimators, so the same fling carries three velocities |
| Hit testing during motion | The web, iOS and Linux hit presented geometry. macOS writes transforms to layers but hit-tests with `NSView.hitTest`, which by inference ignores them | One host deviates, and no test covers it |
| The agent's clock | Motion jumps in one seek. Timers fire one by one, at most 4,096 per call (`runner/src/runner.rs:346`). Linux's reorder edge scroll integrates per frame (`host/linux/src/presenter/arrange.rs:408`) | Stepping is independent of step size for springs, not for timers or integrators |

What any design has to respect:

- `rules/NOT-DOING.md` §Motion:
  - no gesture arena ("Scroll always wins");
  - no second value graph ("The style row is the binding");
  - general layout transitions only where admitted (the sheet and the Shop
    accordions);
  - no "decay, sequence, and repeat drivers … A spring carries release
    velocity; nothing else needs a driver";
  - "speculative parallel layout" stays out.
- LLP 1035.001 §6 refuses "per-frame application callbacks".
- LLP 1007 §9: no text-measurement bridge for kernel layout on the web.
- exact1's LLP 0486 §13 put a "public `measure(constraints)` protocol later or
  never — … unimplementable atop CSS". That holds for code that takes part in
  layout, as in Houdini's model. It does not hold for code that only reads a
  hypothetical: a forced layout answers that, at a price.

## 4. Where it would be valuable

| Use | Axis | exact2 consumer | What happens today |
|---|---|---|---|
| Reparenting and shared elements | Tree | Photo zoom's return "by stable item identity, including an unmounted, moved or deleted source" (LLP 1041 §8.5, owed); LLP 1013 | Not built. LLP 1013 would snapshot around a commit |
| Layout transitions that retarget | Tree, time | Shop's overlapping accordions; the sheet | `measure_height_targets` supplies `auto` targets on Apple; the web uses CSS |
| Drop previews | Tree | Virtualized reorder (LLP 1041 §8.5) | A runner-owned preview, returning `NeedsMeasurement` for unmounted rows |
| Fitting and choosing | Constraint | Context previews and menus; toolbar overflow; label variants | Per-feature host code. Choice layout is not in v1 (LLP 1001) |
| Heights before layout | Constraint | Windowed lists; gigantic Markdown; text around shapes, which needs band heights "before a line is broken" (LLP 1043.000) | Estimates, replaced by measurement after mounting |
| Fling projection onto layout | Time, constraint | Sheet detents; pager; landing-first rows (LLP 1050 F6) | Hard-coded stops (§2.5) |
| Destinations the OS announces | Constraint, time | Keyboard, rotation, continuous native resize (LLP 1041): lay out once for the end state and animate alongside | Committed viewport facts (LLP 1039), laid out as they arrive |
| Physics against layout | All three | The ball (§5); a photo released with momentum inside its bounds, which is the ball in a box; Reflow's balls, with text flowing around them | Velocity ignored; walls from viewport arithmetic or hard-coded (§2.5, §2.6) |
| Agents and tests | Time | `clock`, `layout` | Committed or presented geometry at the current time only |

Five rows need more than a table cell:

- **Reparenting.** The kernel already moves a node to a new parent in one
  batch with its identity kept (LLP 1001 §3). What is missing is the node's
  frame under the new parent *before* the commit. With that frame, the move
  can be animated from where the node is to where it will be, or declined.
- **Retargeting.** Tapping a second accordion while the first is still
  opening moves the target of everything below it. The new target is a
  hypothetical layout, and the velocity at the moment of the tap is presented
  state. Both are needed at once.
- **Drop previews.** "Where would it land?" means the list with the item at
  index i. Hit-testing the moving presentation instead makes rows oscillate
  under the finger, which is why LLP 1002 D8 tests logical boxes. A
  virtualized list must also answer for rows that are not mounted.
- **Fitting.** CSS anchor positioning makes the browser try each fallback
  position in turn (`position-try-fallbacks`; Chrome 125, Safari 26, Firefox
  147). exact2 does the same by hand in `positionContexts`, once per host.
- **Agents.** An agent that could ask for layout at +300 ms, or at rest,
  without advancing the clock could test transitions and throws
  deterministically. The ball shows that the clock can be trusted only if
  motion gives the same answer however time is stepped.

## 5. Worked example: a ball flung against a wall

Charlie's case is a ball flung at a wall. Something has to decide when it hits
and how it bounces, and that will almost always happen between two frames. It
is worth working through carefully because it uses all three axes at once:

- the wall is layout, a tree fact;
- the court's size comes from an offer, a constraint;
- the question is *when*, which is time.

The numbers come from a short script written for this record (in the session
scratchpad, not the repo). They are arithmetic, not measurements of exact2.

### 5.1 Setup

- **The court.** A box whose content box is 390 pt wide, an iPhone's width.
  The ball is a 40 pt circle placed with `translate`, so its centre moves
  within [20, 370].
- **The fling.** The ball is released at x = 300, moving right at 3,000 pt/s.
  That is a hard fling; UIKit reports pan velocity in points per second.
- **Friction.** Exponential decay at UIScrollView's normal rate, 0.998 per
  millisecond: v(t) = v₀·e^(−kt), with k = −1000·ln 0.998 ≈ 2.002 s⁻¹.
  - Under this law, speed falls linearly with distance travelled:
    v = v₀ − k·s.
  - In open space the ball would coast 1,498.5 pt. UIKit's projection formula
    (§2.5) gives 1,497.
- **Restitution 0.7.** Each bounce keeps 70% of the speed.

### 5.2 The exact answer: three contacts and a rest

Each contact has a closed form. Across a gap d at speed v, the time to the
wall is t = −ln(1 − k·d/v)/k.

| Event | Time after release | Centre at | Speed in → out (pt/s) |
|---|---|---|---|
| Right wall | 23.90 ms | 370 | 2,859.9 → 2,001.9 |
| Left wall | 239.09 ms | 20 | 1,301.2 → 910.8 |
| Right wall | 971.65 ms | 370 | 210.1 → 147.1 |
| Rest (within 0.5 pt) | ≈ 3.46 s | 296.5 | — |

The whole future, including where the ball comes to rest, is known at release
for the cost of a few logarithms.

### 5.3 What the frames see

At 60 Hz, the frames either side of the first impact fall at 16.67 ms and
33.33 ms:

- In the first, the ball is drawn 20.8 pt short of the wall, still
  approaching.
- In the second, it is 18.7 pt back from the wall, receding.

No frame shows the ball touching the wall, and none should: the impact
happened 7.2 ms after one frame and 9.4 ms before the next. At 120 Hz, the
frame at 25.00 ms shows the ball 2.2 pt from the wall, already moving away.

Presenting motion means sampling a continuous path at presentation times. The
impact is an event on the path; it does not belong to any frame. Two things
follow.

- **Sample at the time the frame will be shown**, not at the time the
  callback runs. Otherwise jitter in callback timing becomes jitter in
  position. exact2's Apple host samples motion when its display-link callback
  runs, although its canvases already sample at `targetTimestamp` (§3).
- **Anything meant to coincide with the impact belongs to the event's time.**
  A haptic, a click, or a state change such as "the ball entered the goal"
  should happen at 23.90 ms. That time is known at release, so there is no
  reason to wait for the next frame. Audio can be scheduled sample-accurately
  (Web Audio's `start(when)`), whereas a frame boundary is up to 8–17 ms late
  by construction.

### 5.4 What per-frame stepping gets wrong

The usual implementation advances the ball one frame at a time and checks for
penetration afterwards. The table compares two such variants with the exact
answer (positions in pt):

| | At 50 ms | At rest |
|---|---|---|
| Exact | 319.1 | 296.5 |
| Clamp to the wall and reverse, 120 Hz | 321.3 | 298.3 |
| Same, 60 Hz | 337.8 | 315.1 |
| Same, 30 Hz | — | 316.9 |
| Same, one step per 100 ms | — | 349.6 (two bounces, not three) |
| Same, one 50 ms step (an agent's `clock +50`) | 370.0, stuck on the wall | — |
| Mirror the overshoot, scaled by restitution, any step size | 319.1 | 296.5 |

**Clamping loses the time after the impact.** At 60 Hz, the frame after the
impact puts the ball on the wall instead of 18.7 pt back from it. The lost
travel compounds, so where the ball rests depends on the frame rate: the
120 Hz and 60 Hz results are 19 pt apart. The same app would rest the ball in
different places on a 120 Hz phone and a 60 Hz one, and in a third place when
an agent advances the clock in one step. The game add-on's Rapier behaves like
the clamp rows for fast bodies (§2.6).

**Mirroring is exact here, and that is a trap.** The mirror rule reflects the
overshoot: x ← wall − e·(x − wall), v ← −e·v. It reproduces the exact answer
at any step size. That is because this motion is linear and homogeneous in
velocity (no force depends on position), so a scaled reflection commutes with
it. For constant velocity with no friction, repeated mirroring folds into a
triangle wave, which is what Reflow's balls compute (§2.6). This is why many
UI implementations get away with the mirror rule. It stops being exact as soon
as any of the following holds.

- **A force that is not friction-like.** Drop a ball from 300 pt under
  gravity of 1,000 pt/s² (UIKit Dynamics' unit gravity), with restitution 0.7.
  At 1.5 s the true height is 130.2 pt. Mirroring gives 140.9 at 60 Hz and
  161.0 at 30 Hz. In a single 1.5 s step it gives 577.5, higher than the
  drop.
- **A thin wall.** Put a 2 pt divider inside the court. An after-the-step check
  sees an overlap only while the centre is inside a 42 pt band: the divider
  plus the ball's diameter.
  - At 3,000 pt/s the ball moves 49 pt per 60 Hz frame, more than the band, so
    15% of release positions tunnel straight through. At 120 Hz none do.
  - At 6,000 pt/s, 57% tunnel at 60 Hz and 15% at 120 Hz.
  - A single `clock` jump tunnels almost always.
- **A wall that moves** (§5.5). Reflecting the overshoot needs the wall's
  position at the moment of contact, and that moment is not known.
- **An event that needs its time** (§5.3). The mirror says where the ball is,
  not when it hit.

### 5.5 When the wall moves

In an interface, walls move: a panel slides in, a sheet rises, the keyboard
pushes the court up, an accordion above it opens. Suppose the right wall
slides left at 600 pt/s from the moment of release.

- **Contact comes earlier.** It is now the root of x_ball(t) = W(t) − r, at
  19.77 ms rather than 23.90 ms, with the centre at 358.1.
- **The bounce is relative to the wall:** v′ = w − e·(v − w). The ball
  arrives at 2,883.6 pt/s and leaves at 3,038.5 pt/s. It leaves faster than it
  came, because the wall bats it. A static wall would send it back at 2,001.9.

So the physics needs the wall's **position and velocity at an arbitrary time
between frames**. In exact2's terms there are three cases.

1. **A paint-only row moves the wall**, such as `translate` under a
   transition. Its position and velocity at any t are closed-form from
   exact-motion. No layout is involved.
2. **Layout that is itself animating moves the wall.** For example, LLP 1041
   §8.12's presented heights push the content below them down.
   - The wall's position at t is the layout of the styles at t, so each sample
     is a hypothetical layout.
   - Finding the contact needs a bracketed root search. Conservative
     advancement steps by the gap divided by the largest possible closing
     speed, which takes a handful of samples per contact.
3. **The wall jumps.** A commit without a transition, or text that re-wraps,
   moves it discontinuously, and the ball can end up inside it. That needs a
   stated rule, such as pushing the ball out along the wall's normal, keeping
   its velocity and recording the push. It should not be left to whatever the
   stepper happens to do.

### 5.6 Rest, and Zeno

With gravity and restitution below 1, the times between bounces form a
geometric series. The dropped ball above bounces infinitely many times before
4.389 s, the Zeno limit.

- **With no rest rule**, an event-driven solver never finishes. After 102
  bounces the next interval is smaller than the resolution of a double near
  4.39 s, so time stops advancing while contacts keep arriving. The speed
  never reaches zero either: after about 2,100 bounces it sticks at the
  smallest subnormal.
- **With a rule** such as "at rest when the next bounce would rise less than
  0.5 pt", the ball stops after 9 bounces, the last at 4.18 s.

The rule is not a hack. Springs already need one (`settle_time()`), and it is
what gives `clock settle` a meaning for a ball.

### 5.7 What the ball asks of a layout system

1. **The walls' geometry at release**: committed layout, readable
   synchronously in the release handler. Today an app cannot read it at all.
2. **The walls' position and velocity at any later time**: layout at t, not
   only at frames.
3. **Notice when a wall changes**, as a receipt naming the node. An in-flight
   path then re-solves from its presented position and velocity at that
   moment, so the velocity stays continuous.
4. **A path value**: closed-form segments between events, evaluated at
   presentation time, with events that carry exact times.
5. **The same arithmetic on every host.** exact-motion already pins `libm`,
   so every host would find the same contacts.
6. **One release velocity.** With three estimators (§3), the same fling
   projects to three resting points.
7. **A rest rule.**
8. **Commit the destination, present the path.** The release handler knows
   the rest point at once (296.5 here). It can commit that point, or a
   decision derived from it, while the host plays the path.

## 6. Findings

- **F1. Hypothetical layout is ordinary inside engines.** CSS defines layout
  through hypothetical sizes, and Taffy caches them per hypothesis. What
  varies across systems is whether app code may ask, and in what shape.
- **F2. Four shapes rest on one mechanism.** Force-and-read, size questions,
  lookahead and motion projection differ in who computes and when, not in
  what is computed. Each serves a different use (§4), so a general system
  needs all four. Built as four separate mechanisms, they drift apart.
- **F3. The failures are consistent.**
  - Second implementations drift (Flutter's dry layout).
  - Re-measurement compounds with depth (Android's nested weights, Flutter's
    intrinsics), hence Compose's ban.
  - Geometry read at render time loops (exact1's container-size channel),
    hence LLP 1039 D5.
  - Answers computed off the thread that needs them arrive late
    (asynchronous layout toolkits).
- **F4. exact2 has the native mechanism in miniature, and no way to ask for
  it.** `measure_height_targets`, presented heights, the staged overlay and
  `rehydrate` are the parts. No app code can call them, so each consumer
  writes its own measurement per host or hard-codes numbers.
- **F5. On native hosts a what-if is exact by construction.** It uses the same
  engine and the same measurer, synchronously on the main thread (LLP 1001:
  "hosts lay out on their main thread"). Its cost is the dirty paths it lays
  out, plus re-breaking text at any new width (63–92 µs per long paragraph on
  CoreText).
- **F6. On the web the only exact answer is the browser's.** It comes from
  forcing a layout of a temporary change and reverting it within one task. A
  second engine (Taffy plus text flow in wasm, Pretext- or Preflex-style) is
  faster and can disagree with the page. Under exact2's rule that the web is
  the standard, the browser's answer is the oracle either way.
- **F7. Time is the least-served axis.** Per-frame stepping gives answers that
  depend on frame rate and phase. It tunnels, and it breaks a seekable clock.
  Event-driven closed-form paths with exact event times and pinned math are
  exact at any step and yield the resting point at release (§5). exact2's
  springs already work this way; its timers and integrators do not (§3).
- **F8. The ball's needs are specific:**
  - layout at an arbitrary time, velocities included;
  - notice when a wall changes;
  - one release velocity;
  - a rest rule;
  - a path whose end the release handler can commit.

  No surveyed UI system provides all of these together. Physics engines do,
  but for their own worlds rather than for layout.
- **F9. Handler-time reads plus paint-only continuous channels avoid the
  loop** that sank exact1's container-size channel, without giving up any of
  the uses in §4.

## 7. Questions

1. **For Jordan:** what Preflex takes and returns, and how it stays in
   agreement with the browser. Also his cases, from 2026-09-24, for excluding
   touches in subtrees during simultaneous recognition.
2. **For Charlie:** is physics against layout a consumer, or only an
   illustration? A consumer moves NOT-DOING's decay-driver line (LLP 1051.000
   §9).
3. **The web:** forced-layout what-ifs (exact, but a reflow each), or a second
   engine (fast, but it drifts)? LLP 1051.000 recommends the first and keeps
   the second as a fallback, to be adopted only if measurements call for it.
4. **The agent:** should `layout` answer at a given time, or at rest, without
   moving the clock? That would be a form of `layout`, not a ninth operation.

## Confidence

- **exact2 facts** were read at origin/main `3dc3a16f` and re-checked at
  `121d629e` (2026-09-25); line numbers are from `121d629e`. Two claims are
  inferences with no test behind them:
  macOS hit-testing untransformed frames, and the per-host release-velocity
  differences mattering in practice.
- **External facts** were checked on 2026-09-25 against official
  documentation, specifications, chromestatus and MDN compatibility data, and
  project source on GitHub. That check corrected three first drafts: Box2D's
  time-of-impact method, UIKit's hit testing under `UIViewPropertyAnimator`,
  and CSS Scroll Snap's wording. UIScrollView's 0.998 and 0.99 are not in
  Apple's documentation; a Mac Catalyst build printed them.
- **Physics** (§5) is arithmetic from a script written for this record. It is
  not a measurement of exact2 or of any device.
- **Preflex** was seen only briefly by Charlie, and nothing here depends on
  its details.
