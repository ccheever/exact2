# LLP 1099: UIKit's springs, everywhere

**Type:** RFC
**Status:** Draft (r1), for Charlie's decision. Design only; nothing is built.
**Systems:** `exact-motion` (`motion/src/spring.rs`, `parse.rs`, `transition.rs`, `engine.rs`), the wire (`kernel/src/wire/codec.rs`, spring tag 7), the Contract compiler's literal check (`contract/lower/src/values.rs`), the web host (`host/web/src/motion.rs`, `css.rs`, `motion-glue.js`, `presence-glue.js`), the JS target (`host/web-js/motion`, `shared.js`), Apple and Linux hosts (the per-frame seek, unchanged), parity fixtures (`host/web/src/parity.rs`, `host/web/tests/fixtures/browser-motion.txt`), new measurement fixtures (`motion/tests/fixtures/uikit-springs/`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Amends:** LLP 1002 D2 (`spring()` gains three spellings and a fourth argument); LLP 1003 §4 (a spring may end at a time) and §5 (springs in the UIKit spellings retarget additively)
**Related:** LLP 1002 D2–D4 and LLP 1003 §4–§7 (the spring, interruption, determinism); LLP 1007 §3 (springs on the web); LLP 1062 D3 (paint springs as `linear()`); LLP 1063 D6 (additive layout motion on the web); LLP 1057.002 (velocity handoff); LLP 1081 (`-exact-` names: `spring` may become `-exact-spring`); LLP 1053.000.000.001 D3 and its reviews (the last attempt to convert a UIKit spring by hand). The Signal clone: `~/.tuft/projects/signal-exact2/DIARY.md` (build 19), `reference/animations.md`, `tools/uikit-spring-probe/` (the first probe). External, read 2026-10-04: UIKit `animate(withDuration:delay:usingSpringWithDamping:initialSpringVelocity:)`, `UIViewPropertyAnimator(duration:dampingRatio:)`, `UISpringTimingParameters` (all four initialisers), `animate(springDuration:bounce:initialSpringVelocity:)` (iOS 17); Core Animation `CASpringAnimation` (`settlingDuration`, `allowsOverdamping`); SwiftUI `Spring`; WWDC23 "Animate with springs".

## Summary

Apps ported from UIKit describe springs the way UIKit does: "0.4 s, damping
0.8, velocity 1". Contract's `spring(stiffness, damping, mass)` cannot say
that. UIKit turns those three numbers into a physical spring by an
undocumented rule, then Core Animation cuts the spring off at 0.4 s whether
or not it has settled. The Signal clone measured the real values from UIKit
and pasted hand-sampled `linear()` curves into its source. Those curves are
correct, but they are not springs: they carry no velocity into a retarget,
and they say nothing about where the numbers came from.

This RFC measures the rule and proposes four spellings inside `spring()`:

```
transition="scale spring(400ms, 0.8, 1)"           // UIKit: duration, damping ratio, velocity
transition="scale spring(500ms, bounce 0.3)"        // UIKit, iOS 17: duration, bounce
transition="scale spring(response 250ms, 0.645)"    // SwiftUI: response, damping fraction
transition="scale 200ms spring(185, 1.63, 1, 0.8)"  // physical, with a velocity and an end time
```

Each spelling lowers to the physical spring that UIKit or SwiftUI builds,
and it ends where Core Animation ends it. Measured on iOS 27.0 across 9,600
UIKit calls (§4):

- **Duration form at zero velocity.** UIKit's rule has a closed form. It
  agrees with all 308 measured calls to 3·10⁻⁸ in stiffness.
- **Duration form with velocity.** UIKit solves its equation numerically. A
  reconstruction of that solver (Newton, twelve steps from a fixed start)
  agrees with UIKit's stiffness in 99.2 % of a dense sweep. Outside one
  narrow band, the curves differ by at most 1.4·10⁻⁷ of the move.
  Inside the band, UIKit's solver is chaotic and the reconstruction can pick
  a different root. That band contains one of Signal's real calls (§4.3).
- **iOS 17 bounce form.** Stiffness and damping are exact in all 60
  calls. The end time agrees to 2·10⁻⁴.
- **Core Animation's curve.** The renderer's samples match the closed form
  to 6·10⁻⁸, including a measured quirk in its overdamped branch (§4.5).

The forms run from one implementation in `exact-motion`. They are therefore
bit-identical on iOS, macOS and Linux. The browser plays the engine's frames
and is held to the same 10⁻³ band as today. This is delicate because the
reference is undocumented Apple behaviour (§7). The fixtures and the probe
that measured it land with this RFC, so any later iOS can be measured and
compared.

## 1. What a port meets

Signal-iOS uses five spring idioms (`reference/animations.md`):

| Idiom | Example in Signal | What UIKit builds |
|---|---|---|
| `UIView.animate(withDuration:delay:usingSpringWithDamping:initialSpringVelocity:)` | long-press menu: 0.4 s, 0.8, v 1; reply icon pop: 0.2 s, 0.06, v 0.8 | a `CASpringAnimation`, mass 1, with a solved stiffness; it ends at the duration |
| `UIViewPropertyAnimator(duration:dampingRatio:)`, `UISpringTimingParameters(dampingRatio:initialVelocity:)` | — | the same solve, the same end (§4.2, rows B and C) |
| `UISpringTimingParameters(mass:stiffness:damping:initialVelocity:)` | Signal's `springResponse` helper: k = (2π/response)², c = 4πζ/response | that spring, ending at Core Animation's `settlingDuration` |
| `UIView.animate(springDuration:bounce:initialSpringVelocity:)` (iOS 17) | — | k = (2π/d)², c from the bounce, ending at a settle time (§4.4) |
| SwiftUI `.spring(response:dampingFraction:)`, `Spring(duration:bounce:)` | — | SwiftUI's own evaluator; the fields are measurable (§4.6) |

In build 18 the clone treated "0.4 s, 0.8" as a response of 0.4 s. Against
UIKit's real curve that was off by 0.168 of the move; using the probe's
numbers it is off by 0.0023 (DIARY build 19). The difference is visible
because UIKit's rule is not a response.

## 2. Design

### D1. Spellings

`spring()` keeps its place in the `transition` shorthand and in
`layout-transition`. The first argument picks the form:

| Spelling | Form | Lowers to | Ends |
|---|---|---|---|
| `spring(k, c, m)` | physical, unchanged | itself | at rest, by today's rule (LLP 1003 §4) |
| `spring(k, c, m, v)` | physical with a velocity (new) | itself, initial velocity `v` moves per second | at rest |
| `spring(<time>, ζ)`, `spring(<time>, ζ, v)` | **duration** (UIKit's `withDuration:usingSpringWithDamping:initialSpringVelocity:` and the property animator's `duration:dampingRatio:`) | mass 1, ζ' = min(ζ, 1), k = (W/d)², c = 2ζ'√k, v (D2) | **at d**, snapping to the target |
| `spring(<time>, bounce b)`, `spring(<time>, bounce b, v)` | **bounce** (iOS 17 `springDuration:bounce:`) | mass 1, k = (2π/d)², c = 4π(1−b)/d for b ≥ 0, 4π/(d(1+b)) for b < 0 | at UIKit's settle time (D3) |
| `spring(response <time>, ζ)` | **response** (SwiftUI, and Signal's helper) | mass 1, k = (2π/r)², c = 4πζ/r | at rest |

Two more rules:

- **An end time on any spring.** A `transition-duration` on a spring is
  allowed and means "end here". Today it is refused as
  `SpringDeclaresDuration`. With an end time, the physical form can say
  exactly what UIKit ran, in UIKit's own numbers: `200ms spring(185, 1.63,
  1, 0.8)`. The duration form already carries its end. Giving it a second
  end is refused, as is a negative delay (both as today).
- **Velocity units.** Velocity is in moves per second. It is UIKit's unit
  for `initialSpringVelocity` and Core Animation's for `initialVelocity`:
  1 means the whole distance in a second. Each component is scaled by its
  own distance. A `translate` of (100, 0) at v 1 starts at 100 pt/s in x
  and 0 in y. This is what one `CASpringAnimation` on `position` does with
  its single scalar.

Refusals, each an existing-style `BadEasing` with a reason:

- duration ≤ 0, response ≤ 0;
- ζ ≤ 0;
- bounce outside (−1, 1);
- a non-finite argument;
- `bounce` with `response`;
- any spelling not in the table.

ζ > 1 is accepted and clamped to 1, as UIKit clamps it (rows A, ζ 1.01–5:
identical to ζ 1). The 0- and 3-argument physical spellings parse exactly
as today, so no existing source changes meaning.

Why not new syntax:

- The `transition` grammar already reserves the easing slot, and these
  spellings are timing functions.
- A leading `<time>` cannot be confused with a stiffness, because Contract
  numbers carry units.
- The keyword arguments follow CSS's own precedent (`steps(4, jump-end)`).
- LLP 1081 may rename `spring` to `-exact-spring`. The forms carry over
  unchanged.

### D2. The duration form's mapping

UIKit's rule, recovered from the measurements (§4.1), uses dimensionless
variables W = ω·d (ω = √k) and u = v·d. It solves

- ζ' < 1: |B|·e^(−ζ'W) = ε, where B = (u/W − ζ')/√(1 − ζ'²);
- ζ' = 1: |−1 + (u/W − 1)·W|·e^(−W) = ε;

with ε = 10⁻³. B is the sine coefficient of the spring's displacement from
its target. UIKit's settle test drops the cosine term. That omission is why
the spring is not settled at d, and why the cut-off can snap (§3.1).

- **Zero velocity.** This is the common case: Signal's menu call has v 1,
  but most calls pass 0. The rule has a closed form:
  ζ'W = ln(ζ' / (ε·√(1 − ζ'²))), and W = 9.233413476 at ζ' = 1. The
  result depends only on ζ, so d scales out. Examples: ζ 0.8 → W 8.99430,
  so 0.4 s gives k = 505.609 (UIKit 505.6086); ζ 0.5 → W 12.7169.
- **Nonzero velocity.** The equation can have three roots, and UIKit picks
  one numerically. The reconstruction:
  - start at W₀ = 5 and take exactly twelve Newton steps on
    g(W) = |B|·e^(−ζ'W) − ε (at ζ' = 1, the critical form), using the
    analytic derivative, with sign(0) taken as +1;
  - a step that lands at or below 0 halves W instead.

  Contract defines the duration form by this procedure, not by "the root
  UIKit picks". The procedure is deterministic, so every host computes the
  same k. It equals UIKit's choice everywhere measured except a narrow band
  (§4.3).

### D3. Where each form ends

- **Duration form.** It ends at d exactly, whatever the spring's state. The
  value snaps to the target, completion fires, and `clock settle` treats d
  as the settle time. This is Core Animation's `duration` (rows U and S:
  presentation equals the closed form up to d and the model value after
  it).
- **Bounce form.**
  - ζ < 1: it ends at Core Animation's `settlingDuration`, which the
    measurements give in closed form: T = ln((1 + |B|)/ε)/(ζω), with B as in
    D2 but in physical units, (v − ζω)/ω_d. This matches all 1,817 rows to
    machine precision.
  - ζ ≥ 1: UIKit ends it at the critically damped settle time,
    |−1 + (v − ω)T|·e^(−ωT) = ε, even when the spring is overdamped.
  - Measured agreement: 2·10⁻⁴ relative.
- **Physical and response forms.** They end at rest by today's rule unless
  given an end time. Core Animation's own `settlingDuration` is relative,
  while exact2's rest threshold is absolute (10⁻³ in property units), so the
  two can end at slightly different times. The value at that end is within
  10⁻³ of the move either way. The difference is in when completion fires,
  not in what is drawn. Q3 asks whether to adopt Core Animation's relative
  end for these forms too.

The forms that end at a time are capped at 60 s rather than
`MAX_DURATION` (10 s). The bounce form at b 0.8 and d 1 s legitimately
runs 5.6 s, and a duration form ends when the author says.

### D4. The curve between start and end

The forms evaluate Core Animation's curve, not ideal physics:

- **Underdamped and critical springs.** These are the textbook closed form
  already in `spring.rs`, now with an authored initial velocity.
- **The bounce form below b 0.** This is the only route by which UIKit sets
  `allowsOverdamping` (rows E: `aod=true`). Core Animation's overdamped
  branch does not follow the overdamped solution:

  x(t) − 1 = a·e^(s₂t) + (−1 − a)·e^(s₁t), with a = (s₂ − v)/(s₁ − s₂),

  where s₁,₂ = −ζω ± ω√(ζ² − 1). Its initial velocity is v + 2ζω, not v,
  so an "overdamped" iOS 17 spring overshoots. At 0.4 s and b −0.2, UIKit
  peaks at 1.115 of the move. The formula matches UIKit's rendered samples
  to 2·10⁻⁵, and hand-built `CASpringAnimation`s to 6·10⁻⁸.
- **The physical form.** It keeps true overdamped physics, as today.
  UIKit's `UISpringTimingParameters(mass:stiffness:damping:)` clamps
  overdamping to critical (rows D, `aod=false`). A port that relies on that
  writes critical damping.

### D5. Retargeting

UIKit retargets additively, and the probe shows how (rows H). Animating
`center` 100 pt and then, before the first finishes, 200 pt further adds a
second animation. The result is two `CASpringAnimation`s on `position`,
`position` and `position-2`, each additive, from −Δ to 0, with its own k
and its own end. The first is not removed or replaced: its residual keeps
playing to its own cut-off, and the new one plays from the new
displacement.

The spring is linear, so for two springs with the same k and c this sum
equals one spring that restarts from the presented value and velocity.
That restart is LLP 1003 §5's rule today. The sum differs from it in three
ways:

1. **Each part ends separately.** Each component ends at its own cut-off,
   and its residual snaps away then.
2. **The new part can differ.** It may have different parameters.
3. **Authored velocity adds.** The new component's velocity v·Δ_new is
   added on top of the motion already carried.

So the duration and bounce forms, and any spring given an end time, keep a
list of additive components per property, as UIKit does. The engine
presents their sum and drops each one at its end. A component that has
ended or is within 10⁻⁶ of the move at rest is collapsed into the base
value, so the list stays short. The web already plays layout motion this
way (LLP 1063 D6, `composite: 'add'`).

The physical and response forms keep LLP 1003 §5's restart. That restart is
SwiftUI's retarget too (SwiftUI springs preserve velocity), and for a
spring without an end it is identical to the additive sum.

- **Opacity is not additive.** UIKit makes its opacity springs
  non-additive (rows A1b: `additive=false`). The probe has not yet measured
  what a second opacity call does mid-flight: whether it replaces the first
  from the presented value or from the model value, and whether it keeps
  velocity. That is stage 0's first job (§9). Until it is measured, an
  opacity spring in a UIKit spelling is specified as a restart from the
  presented value with zero carried velocity, which is CSS's interruption
  rule.
- **Holds and gestures.** A hold or gesture (LLP 1057.003) that catches a
  property collapses its components into the presented value and velocity,
  as it catches a spring today.

### D6. Every host

The forms are resolved and evaluated in `exact-motion`, so there is one
implementation:

| Host | How it plays | Equality |
|---|---|---|
| iOS, macOS | per-frame seek of the engine (`CADisplayLink`), as today. No `CASpringAnimation`: a Core Animation executor stays "permitted, not built" (`rules/DEFERRED.md`). If one is built, these forms map 1:1 onto `CASpringAnimation` (mass 1, k, c, v, `duration` = end, `isAdditive`), because D4 is Core Animation's own curve | bit-identical (libm pinned, no FMA; LLP 1003 §7) |
| Linux | per-frame seek, as today | bit-identical |
| Web (wasm host), physics properties | engine frames at 240 Hz played through WAAPI (LLP 1007 §3). The frames stop at the end time, and the last frame is the target, so the snap is a step. Each additive component is its own animation with `composite: 'add'` | 10⁻³ band against Chrome by fixture, as today |
| Web, paint properties | `linear()` sampled from the curve (LLP 1062 D3), with a repeated stop at 100 % so it jumps to 1 at the end: `linear(…, 1.0006 99.99%, 1 100%)` | 10⁻³ band |
| JS target | static springs lowered at build (LLP 1071). `shared.js`'s separate `springCurve` and `presence-glue.js`'s closed form either route through the engine's frames or gain these forms with shared tests (stage 4) | 10⁻³ band |

**The wire.** A spring today is tag 7 plus three f32s
(`kernel/src/wire/codec.rs`). Dynamic `transition` strings need the new
forms on the wire, so they get a new tag. It carries the form, the authored
parameters (f32) and an optional end time, and the engine resolves them in
f64.

- **Resolve in one place.** Resolving from the authored numbers, rather than
  shipping a solved k, keeps the solve in the engine.
- **f32 does not matter here.** f32 rounding of 0.4 s moves k by 3·10⁻⁸
  relative, which is below anything measured.

## 3. Semantics worth stating

### 3.1. The snap

A duration-form spring ends at d, and UIKit's settle test ignores the
cosine term (D2), so the value at d is not within 10⁻³ of the target:

- **Zero velocity.** At v 0 the residual at d is up to ε·√(1 + B²)/|B|.
  That is 1.25·10⁻³ at ζ 0.8, but ε/ζ ≈ 1.7·10⁻² at ζ 0.06.
- **Velocity near ζω.** When the velocity is close to ζω, B → 0 and the
  solve picks a soft spring. Measured in rows U:
  - Signal's reply icon (0.2 s, 0.06, v 0.8) resolves to k 185 and c 1.63.
    It is at 1.724 of the move when it is cut. Scaling 1 → 1.16, the icon
    swings to about 1.28 and snaps back to 1.16.
  - (1 s, 0.3, v 1) resolves to k 10.9 and is at 1.375 when it is cut.

  Both curves are measured, not inferred.

That is UIKit's behaviour, and the duration form reproduces it. A spring
that looks broken in a port because it snaps is correct.

### 3.2. Zero distance

If the target does not change, UIKit adds no animation, and Contract adds
no component. A velocity of v moves per second times a zero move is 0, so
the duration form has no way to start a property moving from rest toward
its own value. That is the same as UIKit. LLP 1003 §5's carried velocity is
the one exception: a zero-distance interruption still carries motion, and
it does so through the existing restart rule.

### 3.3. Reduced motion

Unchanged: LLP 1061's preference applies to these forms as to any spring.

## 4. Measurements

**Method.** `motion/tests/fixtures/uikit-springs/probe/` is a one-scene
simulator app (`probe.sh <udid>`).

- **Reading back the spring.** For each call it runs the UIKit API on a
  `UIView`, reads back the `CASpringAnimation` UIKit adds to the layer, and
  prints its mass, stiffness, damping, initial velocity, duration,
  `settlingDuration`, `isAdditive` and `allowsOverdamping`.
- **SwiftUI.** It prints `Spring`'s fields.
- **Rendered curves.** It copies selected animations onto a paused layer
  (`speed = 0`), steps `timeOffset` in 10 ms increments, flushes the
  transaction and reads `presentation()`.
- **Where it ran.** It ran on a simulator created for this RFC (iPhone 18
  Pro, iOS 27.0, 24A434). No other simulator was touched.
- **Files.** The report is `ios-27.0.txt` (9,646 lines).
  `fit.mjs` (`bun motion/tests/fixtures/uikit-springs/fit.mjs`) recomputes
  every number in this section from the report.

Line tags:

| Tag | What it holds |
|---|---|
| A | duration grid: 13 durations (0.1–3 s) × 21 ratios (0.01–5) × 9 velocities (−5…20) |
| A1b | other keys and distances |
| B, C, D | property animator and timing-parameter initialisers |
| E, E2 | bounce: 4 durations × 7 bounces × 2 velocities, plus the timing-parameter initialiser |
| F, G, G2 | SwiftUI `Spring` |
| H | retargeting |
| W | dense sweep at d 1 s: 14 ratios × u ∈ [−5, 20] in steps of 0.05, 7,014 calls |
| U | curves of springs UIKit made |
| S | curves of springs built by hand |

### 4.1. The duration form's structure

Every A, B and C row has mass 1, `initialVelocity` = v, `duration` = d,
and c = 2·min(ζ, 1)·√k exactly. Only k varies, and √k·d depends only on
(ζ, v·d):

- (0.1 s, v 2), (0.2 s, v 1) and (0.4 s, v 0.5) give identical W, even
  where UIKit's solver had not converged.
- The distance and key path do not matter (rows A1b: `center` by 100 or
  1,000 pt and `alpha` resolve identically).

### 4.2. Accuracy of D2

| Set | Calls | W within 10⁻⁵ | Curve error over [0, d] (fraction of the move) |
|---|---|---|---|
| A, v = 0 (closed form) | 273 | 273 (closed form within 10⁻⁷; worst 2.7·10⁻⁸) | ≤ 1.1·10⁻⁷ |
| A, all | 2,457 | 2,412 (98.2 %) | median 3.6·10⁻⁹; p99 1.6·10⁻²; max 1.85 |
| W sweep | 7,014 | 6,960 (99.2 %) | median 2.7·10⁻⁹; p99 9.4·10⁻⁸; max 1.85 |
| B, C | 39 | 38 | max 1.6·10⁻² |

- **UIKit's own precision.** UIKit converges to about 10⁻⁸ relative in W.
  Its residual |B|e^(−ζW) − ε is about 10⁻¹¹.
- **SwiftUI agrees.** SwiftUI's `Spring(settlingDuration:dampingRatio:)`
  gives the same stiffness as UIKit's duration form at v 0, to 4.7·10⁻¹⁰.
  Apple's own documented API therefore calls this rule a settling duration,
  which corroborates the reading.

### 4.3. The band

Every miss falls in a narrow band of u for each ζ. Outside the band the
curve error is at most 1.4·10⁻⁷:

| ζ | u band holding all misses | worst curve error inside |
|---|---|---|
| 0.05 | 0.05–0.10 | 1.85 |
| 0.1 | 0.15–0.25 | 1.71 |
| 0.3 | 0.85–1.05 | 0.62 |
| 0.5 | 1.70–1.90 | 0.98 |
| 0.8 | 3.10–3.30 | 0.99 |
| 1.0 | 4.85–5.95 | 0.05 |

(All 14 ratios are in `fit.mjs`'s output.)

- **Where the band sits.** It lies where the smaller pair of roots crosses
  the solver's start (W ≈ 5). Newton's basins interleave there: for Signal's call, a start of
  5.4–6.0 converges to one root and 5.0–5.2 to the other.
- **What the twelve steps reproduce.** The step cap, not convergence, is
  what makes the twelve-step reconstruction match UIKit's unconverged
  answers outside the band. Eleven or thirteen steps match fewer.
- **What was tried.** Variants of the step guard, the start, the variable
  (ω, k, ln W, 1/W) and the function (linear, logarithmic, settle time)
  each move a few points in the band and lose others. None was exact.
- **Signal's case.** Signal's reply-icon call is in the band: u 0.16 at
  ζ 0.06. UIKit picks k 184.95 and the reconstruction picks k 171.05. That
  is a 0.06 curve difference, or 0.01 in scale for the 1 → 1.16 pop.

A band warning is part of the proposal. When a literal duration-form
spring falls in the measured band (a small per-ζ table with margins,
generated from the fixtures), the compiler warns
`lower-spring-uikit-band`. The warning says UIKit's own solver is
ill-conditioned there and suggests the physical spelling with an end time,
taken from the probe. For Signal's icon that is
`200ms spring(184.95, 1.632, 1, 0.8)`. A dynamic string gets the same
check at runtime through the existing `BadTransition` reporting, as a
warning.

### 4.4. The bounce form

- **Stiffness and damping.** All 60 E and E2 calls match k = (2π/d)² and
  the damping in D1 exactly, to the last bit.
- **End time.** The end time matches D3 to 1.8·10⁻⁴.
- **Velocity.** Velocity changes only the end time.
- **SwiftUI disagrees below b 0.** For b < 0, SwiftUI's
  `Spring(duration:bounce:)` reports a different spring from UIKit's: its
  stiffness is (2ζ² − 1)·(2π/d)², where ζ = 1/(1 + b). Its
  `Spring(response:dampingRatio:)` does the same above ζ 1 (rows F, G:
  ×3.5 at ζ 1.5 and ×7 at ζ 2). Contract follows UIKit for `bounce`, and
  the formula in D1 for `response` at every ζ, which is Signal's helper.
  SwiftUI's own evaluator was not measured.

### 4.5. The renderer

The presentation layer's samples:

- **Hand-built springs (S).** They match D4's formulas to at most
  6.4·10⁻⁸, which is f32 precision.
- **Springs UIKit made (U).** They match to at most 4·10⁻⁵ of a
  1,000 pt move.
- **After the end.** Every sample past the end is the target exactly.

Core Animation clamps ζ to 1 unless `allowsOverdamping` is set. Its
overdamped branch is D4's formula. Under the textbook overdamped solution
the error would be 0.87.

### 4.6. What was not measured

Each of these is a stage 0 task (§9):

- iOS versions other than 27.0, because no other runtime is installed.
- A physical device.
- A mid-flight retarget sampled over time. Rows H show the structure but
  not the curve.
- Non-additive (opacity) retargets.
- `UISpringTimingParameters`' `CGVector` velocity with dx ≠ dy.
- Core Animation's `settlingDuration` at ζ ≥ 1 (it appears to step in
  0.1 s).
- SwiftUI's rendered curves.

## 5. Interaction with today's spring

`spring(k, c, m)` keeps its meaning, its rest rule and its retarget. The
new fourth argument and the end time are additions. Paint springs keep
LLP 1062 D3: they start from rest and interrupt as CSS does. An authored
velocity on a paint spring is refused, because paint springs carry no
velocity.

## 6. Alternatives considered

- **Use UIKit's `animate` on iOS only.**
  - For: exact by construction on iOS.
  - Against: it breaks parity. macOS, Linux and the web would still need
    this mapping, and the agent's clock seeks the engine, which cannot seek
    a UIKit animation. The Core Animation executor (D6) is the version of
    this that keeps parity.
- **A measured lookup table** of W over (ζ, u).
  - For: it would hold UIKit's exact choices at the grid points.
  - Against: in the band the function is discontinuous at every scale, so
    no table interpolates it. Outside the band the closed form and the
    reconstruction are already at 10⁻⁷, so a table adds bulk without
    accuracy.
- **Do nothing and document the conversion.**
  - For: authors would write `200ms spring(k, c, 1, v)` from the probe.
    That still needs D1's fourth argument and end time.
  - Against: every port would need a simulator to translate
    "0.4 s, 0.8", and the source would lose the author's intent.
  - This RFC keeps that path as the escape hatch for the band.
- **Treat duration as response** (k = (2π/d)²), which is what build 18 and
  LLP 1053.000.000.001 did.
  - For: it is simple.
  - Against: the measured error was 0.168 of the move.
- **Disassemble UIKit's solver** to make the band exact.
  - For: it is the only route to an exact band.
  - Against: it means reverse-engineering Apple's binary, and Apple's
    licence restricts that. This RFC does not do it. Q1 asks.
- **Ideal physics in the bounce form below b 0.**
  - For: it is what the parameters claim.
  - Against: it is not what UIKit renders (0.87 error). Q2 asks.

## 7. Why this is delicate

- **The reference is undocumented.** Nothing Apple publishes says "sine
  coefficient, ε 10⁻³, Newton from 5, twelve steps". It was read off
  measurements.
  - The v = 0 rule is exact.
  - The bounce form matches WWDC23's published formulas, and the
    measurements pin which branch applies when.
  - The velocity solve is a reconstruction.
- **Apple can change it.** A UIKit update could change the start, the
  steps or ε. That would move every velocity spring slightly and reshuffle
  the band. A Core Animation fix to the overdamped branch would change
  every negative-bounce curve. If that happens:
  - The fixtures are per iOS version (`ios-27.0.txt`). `probe.sh` plus
    `fit.mjs` on a new runtime reports the drift in one run.
  - Contract pins one reference: the newest measured iOS, today 27.0.
    Following a change is a deliberate fixture update with a status line
    on this LLP. An app on an older iOS that differs is a documented
    deviation, not a bug.
  - Per-iOS-version behaviour is not proposed. Contract output is the same
    on every platform, so it cannot branch on the iOS version a user
    happens to run.
- **The band cannot be fixed by tuning.** It is inherently chaotic, so the
  warning and the escape hatch are the answer, not a better guess.
- **Drift is caught by golden curves.** The curves for the canonical calls
  live in the engine's tests and in the web parity fixture, so drift in our
  own implementation fails a test. Drift in Apple's shows up only when the
  probe is re-run. Doing that after each Xcode update is in the test plan.

## 8. Test plan

1. **Mapping (motion unit tests, in Rust).**
   - Port `fit.mjs`'s checks to tests that read `ios-27.0.txt` directly.
   - Every A, B, C, E, E2 and W row must match within 10⁻⁵ in W, except
     the listed band misses, which are pinned so that a change in the
     solver shows.
   - The v = 0 closed form must hold to 10⁻⁷.
   - Every U and S curve must match the engine to 10⁻⁴ before the end and
     equal the target after it.
2. **Golden curves.** For the canonical calls (both of Signal's, (1 s,
   0.3, v 1), each E row at d 0.4 s, and one physical spring with an end
   time), store the engine's 240 Hz frames:
   - native hosts compare bit for bit;
   - `parity.rs` gains `uikit-duration`, `uikit-cut` (the snap) and
     `uikit-bounce-overdamped`, recorded in Chrome and held to 10⁻³, with
     the step at the end time checked to one frame.
3. **Retargeting.** The stage 0 probe records UIKit's two-call `center`
   retarget, at 100 ms and 250 ms into the first spring, sampled on a
   paused layer, plus the opacity case. Engine tests replay the same
   sequence and must match to 10⁻⁴. Per-host tests on Linux, macOS and the
   web check that the components end at their own times.
4. **Parser.**
   - Each spelling and each refusal.
   - The 0- and 3-argument spellings are byte-identical to today.
   - The duration form with a `transition-duration` is refused.
   - The band warning fires on Signal's literal and not on the menu's.
5. **Cross-host.** The existing presence and timeline cross-host tests
   gain one duration-form case with a retarget.
6. **Drift.** After each Xcode or iOS update, run `probe.sh` and
   `fit.mjs` on the new runtime. Any change in the A, E or U lines lands
   as a new `ios-<version>.txt` and a status note here. This is a manual
   step in `docs/`, since CI has no simulator.

## 9. Stages

0. **Close the measurement gaps** (§4.6).
   - Retargets over time, including opacity.
   - `CGVector` velocity.
   - A device recording of one duration-form and one negative-bounce
     spring, filmed and tracked with the clone's `motion-trace.mjs`.
   - Older iOS runtimes, if any can be installed.

   Fold the findings into this LLP before code.
1. **`exact-motion`.**
   - The forms in `SpringConfig`, the resolver (D2) and Core Animation's
     curve (D4).
   - End times (D3) and the 60 s cap.
   - The unit tests in §8.1 and §8.2 (native).
2. **Parse, wire, compiler.**
   - The spellings in `parse.rs`.
   - An end time on a spring in `transition.rs`.
   - The new wire tag.
   - `contract/lower` diagnostics, including the band warning.
   - Docs: `docs/contract-for-agents.md` and `-for-humans.md`.
3. **Retargeting.** Additive components in the engine (D5), with the
   §8.3 tests.
4. **Hosts.**
   - Web frames with end steps and `composite: 'add'`.
   - Paint `linear()` with the end jump.
   - The JS target's `shared.js` and `presence-glue.js`.
   - The Chrome parity fixture re-recorded.
5. **The Signal clone.** Replace its hand-sampled `linear()` curves with
   the spellings, and film both against UIKit.

## 10. Open questions for Charlie

- **Q1. The band.**
  - Accept the reconstruction plus a warning (recommended).
  - Or authorise reverse-engineering UIKit's solver to make it exact. That
    is a legal and licence call, not a technical one.
  - Or refuse duration-form literals inside the band.
- **Q2. The overdamped branch.** Should the bounce form below b 0 copy
  Core Animation's overdamped branch (recommended: it is what users see on
  iOS 17 and later)? The alternative is ideal physics, declared as a
  deviation.
- **Q3. Where the physical and response forms end.** Core Animation ends
  them at its relative `settlingDuration`; exact2 ends them at its absolute
  rest. Adopting Core Animation's end would make completion times match
  UIKit, at the cost of one more rule. The recommendation is to keep
  exact2's rest, because the drawn difference is ≤ 10⁻³.
- **Q4. Syntax.** The recommendation is `spring(<time>, ζ[, v])`,
  `spring(<time>, bounce b[, v])` and `spring(response <time>, ζ)`.
  - The alternative is fully labelled arguments:
    `spring(duration 400ms, damping-ratio 0.8, velocity 1)`.
  - Should it follow LLP 1081's `-exact-` rename when that lands?
- **Q5. A reference iOS.** Pin the newest measured iOS (recommended), or
  the oldest that a target app supports? Signal supports iOS 15, which
  could not be measured here.
