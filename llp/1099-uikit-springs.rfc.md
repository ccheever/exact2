# LLP 1099: UIKit's springs, everywhere

**Type:** RFC
**Status:** Draft (r2), for Charlie's decision. Design only; nothing is built.
- r1 (`9b974e323`) was reviewed blind by Astra (`gpt-6-astra`, reasoning effort max): NEEDS REWORK, `llp/reviews/llp-1099-uikit-springs.astra.md`. It was also reviewed by Grok 4.7 (xhigh): SOUND WITH CHANGES, `llp/reviews/llp-1099-uikit-springs.grok.md`.
- r2 takes every finding except one, which is taken in part (§11). It adds measured retargets (rows R) and edge cases (rows X).
**Systems:** `exact-motion` (`motion/src/spring.rs`, `parse.rs`, `transition.rs`, `engine.rs`, `easing.rs`), the wire (`kernel/src/wire/codec.rs`: a new easing tag 9), the kernel's transition check (`kernel/build.rs`), the Contract compiler's literal check (`contract/lower/src/values.rs`), the web host (`host/web/src/motion.rs`, `batch.rs`, `css.rs`, `motion-glue.js`, `presence-glue.js`), the JS target (`host/web-js/motion`, `shared.js`), Apple and Linux hosts (the per-frame seek, unchanged), parity fixtures (`host/web/src/parity.rs`, `host/web/tests/fixtures/browser-motion.txt`), new measurement fixtures (`motion/tests/fixtures/uikit-springs/`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2)
**Amends:**
- LLP 1002 D2: `spring()` gains three spellings and a labelled velocity.
- LLP 1003 §1: `transition-duration` on a spring becomes an end time, and generated `linear()` keeps the stop limit.
- LLP 1003 §4: a spring may end at a time.
- LLP 1003 §5: springs in the UIKit spellings retarget additively.
- LLP 1007 §3: the frames carry explicit offsets and a step at the end.
**Related:**
- Motion: LLP 1002 D2–D4 and LLP 1003 §4–§7 (the spring, interruption, determinism); LLP 1007 §3 (springs on the web); LLP 1062 D3 (paint springs as `linear()`); LLP 1063 D6 (additive layout motion on the web); LLP 1057.002 and 1057.003 (velocity handoff, holds).
- Naming: LLP 1081 (`-exact-` names; its recommendations are taken, so `spring` becomes `-exact-spring`).
- Prior conversion: LLP 1053.000.000.001 D3 and its reviews (the last attempt to convert a UIKit spring by hand).
- The Signal clone: `~/.tuft/projects/signal-exact2/DIARY.md` (build 19), `reference/animations.md`, `tools/uikit-spring-probe/` (the first probe).
- External, read 2026-10-04:
  - UIKit: `animate(withDuration:delay:usingSpringWithDamping:initialSpringVelocity:)`, `UIViewPropertyAnimator(duration:dampingRatio:)`, `UISpringTimingParameters` (all four initialisers), `animate(springDuration:bounce:initialSpringVelocity:)` (iOS 17).
  - Core Animation: `CASpringAnimation` (`settlingDuration`, `allowsOverdamping`).
  - SwiftUI: `Spring`.
  - WWDC23, "Animate with springs".
  - CSS Easing 2, `linear()` output.

## Summary

Apps ported from UIKit describe springs the way UIKit does: "0.4 s, damping
0.8, velocity 1". Contract's `spring(stiffness, damping, mass)` cannot say
that. UIKit turns those numbers into a physical spring by an undocumented
rule, then Core Animation cuts the spring off at 0.4 s whether or not it
has settled. The Signal clone read the real values out of UIKit and pasted
hand-sampled `linear()` curves into its source. Those curves are correct,
but they are not springs: they carry no velocity into a retarget, and they
hide the author's intent.

This RFC measures the rule and proposes three spellings inside `spring()`,
plus a labelled velocity and an end time for the physical spring:

```
transition="scale spring(400ms, 0.8, velocity 1)"         // UIKit: duration, damping ratio, velocity
transition="scale spring(500ms, bounce 0.3)"              // UIKit, iOS 17: duration, bounce
transition="scale spring(response 250ms, 0.645)"          // response, damping fraction (Signal's helper)
transition="scale 200ms spring(185, 1.63, 1, velocity 0.8)" // physical, with a velocity and an end time
```

Under LLP 1081, whose recommendations are taken, the function is spelled
`-exact-spring`. The arguments are the same.

Each spelling lowers to the physical spring UIKit builds, and it ends where
Core Animation ends it. Measured on the iOS 27.0 simulator across 9,600
UIKit calls (§4):

- **Duration form at zero velocity.** UIKit's rule has a closed form. It
  agrees with all 308 measured calls to 6·10⁻⁸ in W and 1.2·10⁻⁷ in
  stiffness.
- **Duration form with velocity.** UIKit solves its equation numerically.
  A reconstruction of that solver (Newton, twelve steps from a fixed start)
  agrees with UIKit's stiffness in 99.2 % of a dense sweep.
  - Outside one band of inputs, defined by a closed predicate, the curves
    differ by at most 1.4·10⁻⁷ of the move.
  - Inside the band, UIKit's solver is chaotic, so the reconstruction can
    pick another root or no root. A result that is not a root is refused,
    and a literal inside the band gets a warning. One of Signal's real
    calls is in the band (§4.3).
- **iOS 17 bounce form.** Stiffness and damping match in all 60 calls,
  within one ulp (56 are bit-identical). The end time agrees to 2·10⁻⁴.
- **Core Animation's curve.** Hand-built `CASpringAnimation`s match the
  closed form to 6·10⁻⁸. That includes a measured quirk in the overdamped
  branch (§4.5). The springs UIKit itself creates match to 4·10⁻⁵.
- **Retargets.** A mid-flight retarget is the sum of independent additive
  springs, to 0.007 pt over a 300 pt move. Opacity is replaced instead
  (rows R).

The forms are resolved and evaluated in `exact-motion`, so iOS, macOS and
Linux are bit-identical. The browser plays the engine's frames, held to a
stated band. The reference is undocumented Apple behaviour (§7), so the
probe and fixtures land with this RFC. Any later iOS can then be measured
and compared in one run.

## 1. What a port meets

Signal-iOS uses three of these idioms (`reference/animations.md`); the
other two complete what UIKit and SwiftUI offer:

| Idiom | Example in Signal | What UIKit builds (measured) |
|---|---|---|
| `UIView.animate(withDuration:delay:usingSpringWithDamping:initialSpringVelocity:)` | long-press menu: 0.4 s, 0.8, v 1; reply icon pop: 0.2 s, 0.06, v 0.8 | a `CASpringAnimation`, mass 1, with a solved stiffness; it ends at the duration (rows A) |
| `UIViewPropertyAnimator(duration:dampingRatio:)`, `UISpringTimingParameters(dampingRatio:initialVelocity:)` | — | the same solve, the same end (rows B, C) |
| `UISpringTimingParameters(mass:stiffness:damping:initialVelocity:)` | Signal's `springResponse` helper: k = (2π/response)², c = 4πζ/response | that spring, overdamping clamped to critical, ending at Core Animation's `settlingDuration` (rows D, X) |
| `UIView.animate(springDuration:bounce:initialSpringVelocity:)` (iOS 17) | — | k = (2π/d)², c from the bounce, `allowsOverdamping` on, ending at a settle time (rows E) |
| SwiftUI `.spring(response:dampingFraction:)`, `Spring(duration:bounce:)` | — | SwiftUI's own evaluator, not measured; its fields are (rows F, G) |

In build 18 the clone treated "0.4 s, 0.8" as a response of 0.4 s. Its
tracker measured that as 0.168 of the move off UIKit's real curve, and
0.0023 off once it used the probe's numbers (DIARY, build 19). Recomputed
from the closed forms, the response reading differs from UIKit's curve by
up to 0.206. UIKit's rule is not a response.

## 2. Design

### D1. Spellings

`spring()` keeps its place in the `transition` shorthand and in
`layout-transition`:

| Spelling | Form | Lowers to | Ends |
|---|---|---|---|
| `spring()`, `spring(k, c, m)` | physical, unchanged | itself | at rest, by today's rule (LLP 1003 §4) |
| `spring(k, c, m, velocity v)` | physical with a velocity (new) | itself; initial velocity v moves per second | at rest, or at an end time |
| `spring(<time>, ζ)`, `spring(<time>, ζ, velocity v)` | **duration**: UIKit's `withDuration:usingSpringWithDamping:initialSpringVelocity:` and `duration:dampingRatio:` | mass 1, ζ' = min(ζ, 1), k = (W/d)², c = 2ζ'√k, v (D2) | **at d**, snapping to the target |
| `spring(<time>, bounce b)`, `spring(<time>, bounce b, velocity v)` | **bounce**: UIKit's `animate(springDuration:bounce:initialSpringVelocity:)` | mass 1, k = (2π/d)², c = 4π(1−b)/d for b ≥ 0, 4π/(d(1+b)) for b < 0 | at UIKit's settle time (D3) |
| `spring(response <time>, ζ)`, `spring(response <time>, ζ, velocity v)` | **response**: Signal's helper on `UISpringTimingParameters(mass:stiffness:damping:)` | mass 1, k = (2π/r)², c = 4π·min(ζ, 1)/r | at rest, or at an end time |

**Telling the forms apart.** The first argument decides:

- a number: physical;
- a `<time>`: duration or bounce, decided by whether the second argument
  starts with `bounce`;
- the keyword `response`: response.

**Velocity is always labelled.** A bare fourth number is refused, as it is
today. That keeps a WebKit-order `spring(mass stiffness damping velocity)`
typo from parsing as a wrong spring (LLP 1081:33). It also means the third
argument never changes meaning between `spring(400, 0.8, 1)` (a mass) and
`spring(400ms, 0.8, velocity 1)`.

**An end time on any spring.**
- *Allowed where today it is refused.* A nonzero `transition-duration` on
  a spring now means "end here"; today it is refused as
  `SpringDeclaresDuration` (`motion/src/transition.rs:132`). With an end
  time, the physical form can say what UIKit ran, in UIKit's own numbers:
  `200ms spring(185, 1.63, 1, velocity 0.8)`.
- *Zero means no end.* `0s` stays the "no end time" placeholder. Delayed
  springs keep their spelling, `0s spring(…) 100ms`
  (`host/web/tests/it/holds.rs:222`).
- *Only one end.* The duration and bounce forms carry their own end, so a
  nonzero `transition-duration` with them is refused.

**Velocity units.**
- *Moves per second.* This is UIKit's unit for `initialSpringVelocity` and
  Core Animation's for `initialVelocity`: 1 means the whole distance in a
  second.
- *Per component.* Each component is scaled by its own distance. A
  `translate` of (100, 0) at v 1 starts at 100 pt/s in x and 0 in y.
  `UISpringTimingParameters` with a vector velocity does the same per
  component: rows X show two springs with dx and dy as their velocities.
  Contract takes one scalar; a vector is future work.
- *Physics properties only.* An authored velocity on a paint property is
  refused. Paint springs carry no velocity (LLP 1062 D3).

**Clamping ζ.** In the duration and response forms, ζ > 1 is clamped to 1.

- *Duration form.* UIKit clamps it (rows A, ζ 1.01–5 are identical to
  ζ 1).
- *Response form.* Core Animation clamps the damping of the timing
  parameters Signal's helper uses (rows X: `aod=false` for k 100, c 40).
- *Physical form.* It keeps true overdamped physics, as today.

**Refusals.** Each is a `BadEasing` with a reason:

- duration or response not in (0 s, 60 s];
- ζ < 0.01 (unmeasured below; UIKit returns NaN at ζ 0, v 0, and k ≈ 10⁶ at ζ 0, v 1: rows X);
- bounce outside (−1, 1);
- a non-finite argument;
- `bounce` with `response`;
- a bare fourth number;
- a duration-form result that is not a root (D2);
- a duration or bounce spring whose end would pass 60 s (D3);
- any spelling not in the table.

The zero- and three-argument spellings parse exactly as today.

### D2. The duration form's mapping

UIKit's rule, recovered from the measurements (§4.1), uses dimensionless
variables: W = ω·d (ω = √k) and u = v·d. It solves

- ζ' < 1: |B|·e^(−ζ'W) = ε, where B = (u/W − ζ')/√(1 − ζ'²);
- ζ' = 1: |−1 + (u/W − 1)·W|·e^(−W) = ε;

with ε = 10⁻³. B is the sine coefficient of the spring's displacement from
its target. UIKit's settle test drops the cosine term. That is why the
spring is not settled at d, and why the cut-off can snap (§3.1).

- **The procedure.** Contract defines the form by a procedure, not by
  "the root UIKit picks":
  - start at W₀ = 5;
  - take Newton steps on g(W) = |B|·e^(−ζ'W) − ε (at ζ' = 1, the critical
    form), using the analytic derivative and taking sign(0) as +1, for
    twelve steps or until the derivative is exactly 0;
  - when a step lands at or below 0, halve W instead.
- **Zero velocity.** At v = 0 the procedure reaches the closed form
  ζ'W = ln(ζ' / (ε·√(1 − ζ'²))), which gives W = 9.233413476 at ζ' = 1,
  to within the 10⁻⁵ gate. Implementations use the closed form there. The
  closed form needs ζ'/√(1 − ζ'²) > ε, which holds for every ζ' > 0.001.
  The probe never went below ζ 0.01, so ζ ∈ (0, 0.01) is refused as
  unmeasured.
  - *Examples.* ζ 0.8 gives W 8.99430, so 0.4 s gives k = 505.609 (UIKit:
    505.6086). ζ 0.5 gives W 12.7169.
- **Validation.** The result is accepted only when it is finite, lies in
  (0, 10⁴], and |g(W)| ≤ 0.1·ε. Otherwise the spelling is refused.
  - *Why the threshold.* Where the procedure matches UIKit, |g| is at most
    0.059·ε. That allows for UIKit's own unconverged answers, which the
    twelve-step cap reproduces.
  - *What it refuses.* It refuses 27 of the 9,474 measured inputs. All 27
    are in the band, and UIKit chose differently on every one.
- **Precision.** The authored numbers are f64 at every entry point:
  - literal text, as `parse.rs` keeps them today;
  - the new wire tag (D6), unlike tag 7's f32s.

  Inside the band, rounding the inputs to f32 can change which root the
  procedure finds. At ζ = u = 0.05, for example, the result goes from
  W = 5 to 1.3·10⁹. That is why the inputs stay f64.
- **Determinism.** `exp`, `ln` and `sqrt` go through `motion`'s pinned
  libm (LLP 1003 §7), so the procedure is bit-identical on every host.

### D3. Where each form ends

- **Duration form.** It ends at d exactly, whatever the spring's state.
  - For t < d the value is the curve; for t ≥ d it is the target.
  - Completion fires at d, and `clock settle` treats d as the settle time.
  - This is Core Animation's `duration` (rows U and S: the presentation
    equals the curve before d and the target after it).
- **Bounce form.** It ends at UIKit's animation `duration`, which can
  differ from `settlingDuration` (row E d 0.4, b 0: 0.58792 against
  0.6 s).
  - *ζ < 1.* It ends at Core Animation's `settlingDuration`, which the
    measurements give in closed form: T = ln((1 + |B|)/ε)/(ζω), with
    B = (v − ζω)/ω_d. This matches all 1,817 measured rows to 2.2·10⁻¹⁶.
  - *ζ ≥ 1.* It ends at the last T with |−1 + (v − ω)T|·e^(−ωT) = ε, the
    critically damped settle time, even when the spring is overdamped. The
    last crossing is used because the equation can have three roots.
  - *Agreement.* UIKit's ends match to 1.8·10⁻⁴ relative (60 rows).
- **Physical and response forms.** They end at rest, by today's absolute
  rule (10⁻³ in property units, capped at 10 s), unless they are given an
  end time.
  - *How this differs from UIKit.* Core Animation ends a physical spring
    at its relative `settlingDuration`. The two rules end at different
    times, and the drawn difference depends on the move.
    - On a large move, exact2 runs longer and ends closer to the target.
    - On a move smaller than 10⁻³, exact2 is "at rest" at once.
    - A spring still oscillating at the 10 s cap snaps.
  - *Pinning UIKit's end.* An author who needs it writes the end time.
  - Q3 asks whether to adopt Core Animation's end instead.
- **The 60 s bound.** The forms that end at a time may end at most 60 s
  after they start. A longer end is refused, not clamped.
  - The bounce form reaches 60 s at b ≈ 0.95 with d = 3 s.
  - b 0.9 with d 1 s ends at 11.2 s, which is allowed.

### D4. The curve between start and end

The forms evaluate Core Animation's curve, which is not always ideal
physics:

- **Underdamped and critical springs.** These use the textbook closed form
  already in `spring.rs`, with the initial velocity v·Δ.
- **The bounce form below b 0.** `animate(springDuration:bounce:)` sets
  `allowsOverdamping` on every call (all 56 rows E). The timing-parameter
  initialiser `UISpringTimingParameters(duration:bounce:)` sets it on none
  (rows E2, X). At b < 0 the two therefore render differently: the animate
  API runs Core Animation's overdamped branch, and the timing parameters
  clamp to critical.
  - *The spelling follows the animate API.* `spring(<time>, bounce b)`
    follows `animate(springDuration:bounce:)`, the common call. Porting
    the timing-parameter call at b < 0 means writing the critical spring:
    `spring(<time>, bounce 0)` with the same d gives the same k and a
    critical c.
  - *Core Animation's overdamped branch* does not follow the overdamped
    solution:

    x(t) − 1 = a·e^(s₂t) + (−1 − a)·e^(s₁t), with a = (s₂ − v)/(s₁ − s₂),

    where s₁,₂ = −ζω ± ω√(ζ² − 1).
  - *Its actual initial velocity* is v + 2ζω moves per second, not v. So
    an "overdamped" iOS 17 spring overshoots: at 0.4 s and b −0.2, UIKit
    peaks at 1.115 of the move.
  - *Agreement.* The formula matches UIKit's rendered samples to 2·10⁻⁵
    and hand-built springs to 6·10⁻⁸. The textbook solution would be off
    by 0.87.
- **The physical form** keeps true physics at every ζ, as today.

### D5. Retargeting

**What UIKit does** (rows H and R). The probe animated `center` 50 → 150
at v 0, then → 350 at v 1, the second call made 100 ms or 250 ms into the
first.

- *Two animations.* UIKit adds a second animation, `position-2`, and does
  not touch the first. Each is additive, from −Δ to 0, with its own k,
  begin time and end.
- *A measured sum.* The presented position equals the sum of the two
  components to 0.007 pt over the 300 pt move. The first component's snap
  at its own 0.4 s end is in the data.
- *No option changes it.* `.beginFromCurrentState` makes no difference for
  an additive key.

**What Contract does.** The duration and bounce forms, and any spring
given an end time, keep a list of additive components per property:

- **A new target adds a component.** It starts at −Δ_new, where Δ_new is
  the new target minus the old one, with velocity v·Δ_new.
- **Old components keep running.** Each runs on its own curve and is
  dropped, with its snap, at its own end.
- **The value is the sum.** The presented value is the target plus the sum
  of the components' residuals.
- **Collapsing.** A component is collapsed into the target once it has
  ended. One without an end is collapsed once both its offset and its
  velocity are below 10⁻⁶ of its own Δ (a relative test). The list stays
  short.

**Why not today's restart.** LLP 1003 §5 restarts from the presented value
and velocity, and that is exact for two components only under strict
conditions. The ODE is linear, so the sum equals a single restart only
when:

1. the components have equal k and c;
2. both are on the textbook branch: not the overdamped quirk, whose
   component starts at (v + 2ζω)·Δ, not v·Δ;
3. the time is before the first component's end.

Outside those conditions only the sum reproduces UIKit, which is why the
UIKit spellings use it.

**The physical and response forms** without an end time keep LLP 1003
§5's restart, velocity included.

**Velocity, in order of precedence:**

1. **Release velocity.** A hold or gesture that catches a property
   (LLP 1057.003) collapses its components into the presented value and
   velocity, as it catches a spring today. On release it starts one
   component from the presented value with the release velocity, in
   property units, in place of v·Δ. A duration-form component still ends
   at d, and k still comes from the authored v.
   - *How this differs from UIKit.* A UIKit app passes the release
     velocity divided by Δ into the solve, which changes k as well. That
     difference is declared.
2. **Authored velocity.** v·Δ_new for a new component; v·Δ for a restart.
3. **None.**

**Zero distance and unchanged targets.**

- *No animation.* If the target does not change, UIKit adds no animation
  (rows X: an unchanged `center` gives none), and Contract adds no
  component.
- *Held motion.* Components that are already running continue.
- *Release velocity.* A release velocity at zero distance still starts a
  component, as the hold release does today
  (`motion/src/engine/hold.rs:326`).
- *Today's cancel.* The ordinary interruption that cancels when the
  presented value equals the new target (`motion/src/engine.rs:444`)
  applies only to the restart forms.

**Opacity is replaced, not added** (rows R).

- *What UIKit does.* UIKit's opacity springs are non-additive. A second
  call replaces the first, with velocity 0. By default it starts from the
  *model* value, the old target, so the presented opacity jumps
  (0.40 → 0.20 at 100 ms). With `.beginFromCurrentState` it starts from
  the presented value.
- *What Contract does.* Contract has no per-call options. An opacity
  spring in a UIKit spelling restarts from the presented value with zero
  velocity, which is UIKit's `.beginFromCurrentState` and CSS's
  interruption rule. UIKit's default jump is a declared deviation (Q6).

### D6. Every host

There is one implementation, in `exact-motion`:

| Host | How it plays | Agreement |
|---|---|---|
| iOS, macOS | per-frame seek of the engine (`CADisplayLink`), as today. No `CASpringAnimation`: a Core Animation executor stays "permitted, not built" (`rules/DEFERRED.md`). If one is built, the duration and bounce forms map onto `CASpringAnimation` (mass 1, k, c, v, `duration` = end, `isAdditive`, `allowsOverdamping` as in D4) | bit-identical (libm pinned, no FMA; LLP 1003 §7) |
| Linux | per-frame seek, as today | bit-identical |
| Web (wasm host), physics properties | one WAAPI animation per property with replace compositing, as today (`motion-glue.js:581`, LLP 1007 §3). Its frames are the engine's presented value, components summed, re-lowered at every retarget as an interruption is today. The frames carry explicit offsets (see the band entry below). There is no `composite: 'add'`: CSS adds `scale` by multiplying, and `transform` lists by appending | within the stated band |
| Web, paint properties | `linear()` from the curve (LLP 1062 D3). The end is two stops at 100 %: `linear(…, x(d⁻) 100%, 1 100%)`. CSS takes the later stop when two share an input, and so does `easing.rs:340`. Generated curves keep the 64-stop limit (`easing.rs:61`) by thinning the interior stops evenly | the `linear()` approximation band, as today |
| JS target | static springs lowered at build (LLP 1071) through the engine. Stage 4 deletes `shared.js`'s `springCurve` and `presence-glue.js`'s JS integrator: both carry engine frames instead | as the web |

**The browser's band.**
- *Explicit frame offsets.* Frames come at a rate of max(240 Hz, 16 per
  damped oscillation), up to 8,192 frames per lowering. A spring that
  would need more is refused at lowering, which no measured UIKit call
  needs. Frames at the end time hold the left limit x(d⁻) and then the
  target at the *same* offset, so the snap is a step, not a 4 ms ramp. At
  240 Hz without that step, Signal's icon would be off by 0.387 of the
  move in the last interval.
- *The parity test.* It is held to 10⁻³ of the move at sample times taken
  *between* frames, as the existing `spring` parity case does.

**The wire.** A new easing tag, 9, carries:

- the form: physical, duration, bounce or response;
- the authored parameters, as f64;
- the velocity, if present.

The end time of a physical or response spring travels in the row's
existing duration field, which today must be 0. The tag carries no second
end. Tag 7 stays as it is for the zero- and three-argument spellings.

## 3. Semantics worth stating

### 3.1. The snap

A duration-form spring ends at d, and UIKit's settle test drops the cosine
term. So the value at d is not within 10⁻³ of the target:

- **Zero velocity.** The residual at d is up to ε·√(1 + B²)/|B|: 1.25·10⁻³
  at ζ 0.8, but ε/ζ ≈ 1.7·10⁻² at ζ 0.06.
- **Velocity near ζω.** B → 0 there, so the solve picks a soft spring.
  - Signal's reply icon (0.2 s, 0.06, v 0.8) resolves to k 185 and c 1.63.
    Its left limit at 0.2 s is 1.773 of the move; the last 10 ms sample
    before the end reads 1.724. Scaling 1 → 1.16, the icon swings to about
    1.28 and snaps back to 1.16.
  - (1 s, 0.3, v 1) resolves to k 10.9 and is at 1.375 at its last sample.

That is UIKit's behaviour, and the duration form reproduces it. A port
that snaps is correct.

### 3.2. Reduced motion

Unchanged: LLP 1061's preference applies to these forms as to any spring.

## 4. Measurements

**Method.** `motion/tests/fixtures/uikit-springs/probe/` is a one-scene
simulator app. Running `probe.sh <udid>` builds it for iOS 18 or later and
prints its report.

- **Reading back the spring.** Each call runs the UIKit API on a `UIView`
  and reads back the `CASpringAnimation` UIKit adds to the layer: its mass,
  stiffness, damping, initial velocity, duration, `settlingDuration`,
  `isAdditive` and `allowsOverdamping`.
- **SwiftUI.** It prints `Spring`'s fields.
- **Rendered curves.** It copies selected animations onto a paused layer
  (`speed = 0`), steps `timeOffset` in 10 ms increments, flushes, and reads
  `presentation()`.
- **Retargets.** It pauses a container layer, makes the first call, moves
  `timeOffset` to the retarget time, makes the second call, then sweeps.
- **Where it ran.** It ran on a simulator created for this RFC (iPhone 18
  Pro, iOS 27.0, 24A434). No other simulator was touched.
- **The report.** It is `ios-27.0.txt`. Re-runs reproduced every line byte
  for byte.
- **The fit.** `fit.mjs` (`bun motion/tests/fixtures/uikit-springs/fit.mjs`)
  recomputes every number in this section from the report.

| Tag | What it holds |
|---|---|
| A | duration grid: 13 durations (0.1–3 s) × 21 ratios (0.01–5) × 9 velocities (−5…20) |
| A1b | other keys and distances |
| B, C, D | property animator and timing-parameter initialisers |
| E, E2 | bounce: 4 durations × 7 bounces × 2 velocities, plus the timing-parameter initialiser |
| F, G, G2 | SwiftUI `Spring` fields |
| H | the animations a second same-turn call adds |
| R | retargets over time (`center`, opacity; with and without `.beginFromCurrentState`) |
| X | edge cases: overdamped physical timing parameters, E2 below b 0, ζ 0, an unchanged value, a vector velocity |
| W | dense sweep at d 1 s: 14 ratios × u ∈ [−5, 20] in steps of 0.05 (7,014 calls) |
| U, S | curves of springs UIKit made, and of springs built by hand |

### 4.1. The duration form's structure

Every A, B and C row has mass 1, `initialVelocity` = v and `duration` = d,
with c = 2·min(ζ, 1)·√k to rounding. Only k varies, and √k·d depends only
on (ζ, v·d):

- (0.1 s, v 2), (0.2 s, v 1) and (0.4 s, v 0.5) give identical W, even
  where UIKit's solver has not converged.
- Distance and key path do not matter. In rows A1b, `center` by 100 or
  1,000 pt and `alpha` resolve identically.

This reduction holds for velocity normalised by distance, which is how
UIKit defines it.

### 4.2. Accuracy of D2

| Set | Calls | W within 10⁻⁵ | Curve error over [0, d), fraction of the move |
|---|---|---|---|
| v = 0, closed form (A, W, B, C) | 308 | 308; worst 6.0·10⁻⁸ in W, 1.2·10⁻⁷ in k | ≤ 1.1·10⁻⁷ |
| A, all | 2,457 | 2,412 (98.2 %) | median 3.6·10⁻⁹; p99 1.6·10⁻²; max 1.85 |
| W sweep | 7,014 | 6,960 (99.2 %) | median 2.7·10⁻⁹; p99 9.4·10⁻⁸; max 1.85 |
| B, C | 39 | 38 | max 1.6·10⁻² |

- **UIKit's own precision.** UIKit converges to about 10⁻⁸ relative in W,
  with |g| about 10⁻¹¹.
- **SwiftUI agrees.** SwiftUI's `Spring(settlingDuration:dampingRatio:)`
  gives UIKit's duration-form stiffness at v 0 to 5.4·10⁻⁸. Apple's own
  documented API therefore names this rule a settling duration.
- **Two kinds of test.** These numbers test the mapping (D2). The curve
  tests in §4.5 feed the measured k into the renderer model (D4) and test
  that separately (§8).

### 4.3. The band

**Where the misses are.** Every miss lies in a narrow interval of u for its
ζ:

| ζ | misses | u interval | worst curve error inside |
|---|---|---|---|
| 0.05 | 2 | 0.05–0.10 | 1.85 |
| 0.1 | 3 | 0.15–0.25 | 1.71 |
| 0.2 | 1 | 0.50 | 1.00 |
| 0.3 | 3 | 0.85–1.05 | 0.62 |
| 0.4 | 3 | 1.30–1.40 | 1.06 |
| 0.5 | 4 | 1.70–1.90 | 0.98 |
| 0.6 | 3 | 2.15–2.30 | 0.50 |
| 0.7 | 3 | 2.65–2.75 | 0.30 |
| 0.8 | 5 | 3.10–3.30 | 0.99 |
| 0.9 | 4 | 3.60–3.75 | 0.70 |
| 0.95 | 5 | 3.80–4.00 | 0.11 |
| 0.99 | 5 | 4.00–4.20 | 0.40 |
| 1.0 | 13 | 4.85–5.95 | 0.05 |

Outside these intervals the worst curve error is 1.4·10⁻⁷. ζ 0.01 has no
misses.

**What causes them.** The intervals lie where the smaller pair of roots
crosses the solver's start (W ≈ 5). Newton's basins interleave there: for
Signal's call, a start of 5.4–6.0 converges to one root and 5.0–5.2 to the
other. The step cap, not convergence, is why twelve steps reproduce UIKit's
unconverged answers outside the band. On the sweep, 11 steps match 6,466
calls, 12 match 6,960 and 13 match 6,819.

**What was tried.** Variants of each of these moved a few points in the
band and lost others:

- the step guard;
- the start;
- the variable (ω, k, ln W, 1/W);
- the function (linear, logarithmic, settle time).

None was exact. That does not prove a better reconstruction cannot exist.
It does mean one was not found from behaviour alone.

**The predicate.** It is closed and reproducible: **u > 0 and
1 ≤ u/ζ' ≤ 7.**

- *Coverage.* It holds all 100 misses among the 9,474 measured inputs
  (A, W, and the U rows), including Signal's: u = 0.16 and ζ = 0.06 give
  u/ζ' = 2.7.
- *Cost.* It flags 1,550 of them (16 %).
- *Signal's menu call.* It is outside the band (u/ζ' = 0.5).

**What happens inside the band.**

- **Refused** if validation fails (D2), with a message that names the
  physical spelling from the probe.
- **Warned** otherwise, for a literal: `lower-spring-uikit-band`. The
  warning says UIKit's own solver is ill-conditioned there and that the
  result may not be UIKit's. It suggests pinning the spring with the
  physical spelling and an end time, taken from the probe. For Signal's
  icon that is `200ms spring(184.95, 1.632, 1, velocity 0.8)`.
- **Accepted silently** for a dynamic string. The kernel's
  `BadTransition` path has no warnings (`kernel/build.rs:1306`), so a
  refusal there is the only runtime diagnostic.

### 4.4. The bounce form

- **Stiffness and damping.** All 60 rows E and E2 match k = (2π/d)² and
  D1's damping to 2.2·10⁻¹⁶ relative; 56 are bit-identical.
- **End time.** The end time matches D3 to 1.8·10⁻⁴.
- **Velocity.** Velocity changes only the end time.
- **SwiftUI disagrees below b 0.** For b < 0, SwiftUI's
  `Spring(duration:bounce:)` reports a different spring from UIKit's: its
  stiffness is (2ζ² − 1)·(2π/d)², where ζ = 1/(1 + b). Its
  `Spring(response:dampingRatio:)` does the same above ζ 1 (rows F, G:
  ×3.5 at ζ 1.5, ×7 at ζ 2). Contract follows UIKit.

### 4.5. The renderer

The presentation layer's samples, fed the measured coefficients:

- **Hand-built springs (S)** match D4 to 6.4·10⁻⁸, including the
  overdamped branch.
- **Springs UIKit made (U)** match to 4.0·10⁻⁵ of a 1,000 pt move.
- **After the end,** every sample is the target exactly.

Core Animation clamps ζ to 1 unless `allowsOverdamping` is set (S: an
overdamped spring with the flag off matches the critical curve to
2.8·10⁻⁸).

### 4.6. What was not measured

Each of these is a stage 0 task (§9):

- iOS versions other than 27.0, because no other runtime is installed. The
  probe needs availability guards to build below iOS 18.
- A physical device.
- Core Animation's `settlingDuration` at ζ ≥ 1. It is not used by any form
  in this RFC, and it appears to step in 0.1 s.
- SwiftUI's rendered curves.
- A retarget between springs of different forms.

## 5. Interaction with today's spring

`spring()` and `spring(k, c, m)` keep their meaning, their rest rule and
their retarget. The labelled velocity and the end time are additions. Paint
springs keep LLP 1062 D3: they start from rest and interrupt as CSS does.

## 6. Alternatives considered

- **Use UIKit's `animate` on iOS only.**
  - For: exact by construction on iOS.
  - Against: macOS, Linux and the web would still need this mapping, and
    the agent's clock seeks the engine, which cannot seek a UIKit
    animation. The Core Animation executor (D6) is the parity-keeping form
    of this idea.
- **A measured lookup table** of W over (ζ, u).
  - For: it holds UIKit's exact choices at the grid points.
  - Against: in the band the map is discontinuous at every scale, so no
    table interpolates it. Outside the band the closed form and the
    reconstruction are already at 10⁻⁷.
- **Do nothing and document the conversion.**
  - For: authors would write the physical spelling from the probe.
  - Against: every port would need a simulator to translate
    "0.4 s, 0.8", and the source would lose the author's intent.
  - This RFC keeps that path as the escape hatch for the band.
- **Treat duration as response.**
  - For: it is simple. Build 18 and LLP 1053.000.000.001 did it.
  - Against: it is off by up to 0.206 of the move.
- **Disassemble UIKit's solver** to make the band exact.
  - For: it is the only route to an exact band.
  - Against: it means reverse-engineering Apple's binary, which Apple's
    licence restricts. Not done. Q1 asks.
- **Ideal physics in the bounce form below b 0.**
  - For: it is what the parameters claim.
  - Against: it is not what UIKit renders (0.87 off). Q2 asks.
- **Fully labelled arguments**, as in
  `spring(duration 400ms, damping-ratio 0.8, velocity 1)`.
  - For: unambiguous.
  - Against: longer than needed, since units and keywords already
    distinguish the forms. Labelling only the velocity removes the one
    real ambiguity, the fourth number. Q4 asks.

## 7. Why this is delicate

- **The reference is undocumented.** Nothing Apple publishes says "sine
  coefficient, ε 10⁻³, Newton from 5, twelve steps". It was read off
  measurements.
  - The v = 0 rule matches to 10⁻⁷.
  - The bounce coefficients match WWDC23's published formulas, and the
    measurements pin when each branch applies.
  - The velocity solve is a reconstruction with a known band.
- **Apple can change it.** A UIKit update could change the start, the
  steps or ε, moving every velocity spring slightly and reshuffling the
  band. A Core Animation fix to the overdamped branch would change every
  negative-bounce curve. If that happens:
  - Fixtures are per iOS version (`ios-27.0.txt`). `probe.sh` plus
    `fit.mjs` on a new runtime reports the drift in one run.
  - Contract pins one reference, iOS 27.0, the only one measured.
    Following a change is a deliberate fixture update with a status line
    here. Apps on other iOS versions may differ, as a documented deviation.
  - Contract output cannot branch on the iOS version a user runs.
- **The band cannot be fixed by tuning.** Refusal, the warning and the
  escape hatch are the answer.
- **Drift is caught on both sides.** Golden curves catch drift in exact2's
  own implementation. Re-running the probe after each Xcode update (§8)
  catches drift in Apple's.

## 8. Test plan

1. **Mapping (D2, D3; Rust unit tests reading `ios-27.0.txt`).**
   - Every A, B, C, E, E2 and W row matches within 10⁻⁵ in W, except the
     listed misses. The misses are pinned, so a change in the procedure
     shows.
   - The 27 refusals are pinned.
   - Every miss satisfies the band predicate.
   - v = 0 holds to 10⁻⁷.
   - Bounce ends hold to 2·10⁻⁴.
2. **Renderer (D4).** Every U and S curve, fed its measured coefficients,
   matches the engine:
   - to 10⁻⁴ for t < end;
   - exactly the target for t ≥ end;
   - checked at end − 1 µs, end and end + 1 µs.
3. **Authored text to pixels.** For the canonical spellings (both of
   Signal's, (1 s, 0.3, velocity 1), each bounce at d 0.4 s, and a
   physical spring with velocity and an end time), the path runs from
   string to wire tag 9 to engine to every host:
   - native hosts compare frames bit for bit;
   - `parity.rs` gains `uikit-duration`, `uikit-cut`, `uikit-bounce-overdamped`
     and `uikit-retarget`, recorded in Chrome and held to 10⁻³ between
     frames, with the step at the end time;
   - Signal's icon is expected to differ from UIKit by 0.06 (§4.3) and is
     pinned as such.
4. **Retargeting.**
   - Engine tests replay rows R (the two-call `center` retarget at 100 ms
     and 250 ms, and the opacity replacement) to 10⁻⁴ of the move.
   - Further cases: an unequal-axis `translate`, a zero-distance call with
     and without a release velocity, a hold catching two components, and
     a negative-bounce retarget.
   - Per-host tests on Linux, macOS and the web check that each component
     ends at its own time.
5. **Parser.**
   - Each spelling and each refusal, including a bare fourth number and
     `0s spring(400ms, 0.8) 100ms` (accepted: a delay).
   - The zero- and three-argument spellings are byte-identical to today.
   - The band warning fires on Signal's literal and not on the menu's.
   - The 60 s bound refuses b 0.95 at 3 s.
6. **Cross-host.** The presence and timeline cross-host tests gain a
   duration-form retarget and a negative-bounce case.
7. **Drift.** After each Xcode or iOS update, run `probe.sh` and `fit.mjs`
   on the new runtime. Any change in the A, E, R or U lines lands as a new
   `ios-<version>.txt` and a status note here. The steps live in `docs/`,
   since CI has no simulator.

## 9. Stages

0. **Close the measurement gaps** (§4.6).
   - A device recording of one duration-form spring and one
     negative-bounce spring, filmed and tracked with the clone's
     `motion-trace.mjs`.
   - Older iOS runtimes, if any can be installed, with availability guards
     in the probe.
   - Mixed-form retargets.

   Fold the findings in before code.
1. **`exact-motion`.**
   - The forms in `SpringConfig`, the procedure and its validation (D2),
     and Core Animation's curve (D4).
   - End times and the 60 s bound (D3).
   - The tests in §8.1 and §8.2.
2. **Parse, wire, compiler.**
   - The spellings in `parse.rs`.
   - End times in `transition.rs`.
   - Tag 9.
   - `contract/lower` diagnostics, including the band warning.
   - Docs: `docs/contract-for-agents.md` and `-for-humans.md`.
   - If LLP 1081 has landed, the function is `-exact-spring`.
3. **Retargeting.** Additive components and the velocity precedence (D5),
   with the §8.4 tests.
4. **Hosts.**
   - Web frames with offsets and the end step.
   - Paint `linear()` with the double 100 % stop.
   - The JS target's integrators replaced by engine frames.
   - The Chrome parity fixture re-recorded (§8.3, §8.6).
5. **The Signal clone.** Replace its hand-sampled `linear()` curves with
   the spellings, and film both against UIKit.

## 10. Open questions for Charlie

- **Q1. The band.**
  - Recommended: accept the reconstruction, refuse non-roots, and warn
    inside the predicate.
  - Or authorise reverse-engineering UIKit's solver to make the band
    exact. That is a legal and licence call.
  - Or refuse every duration-form literal inside the predicate (16 % of
    the sweep, but few real calls: zero velocity is never inside it).
- **Q2. The overdamped branch.** Should the bounce form below b 0 copy Core
  Animation's overdamped branch (recommended: it is what
  `animate(springDuration:bounce:)` renders on iOS 27.0)? The alternative
  is ideal physics, declared as a deviation.
- **Q3. Where the physical and response forms end.**
  - Recommended: keep exact2's absolute rest, so existing springs do not
    move.
  - Or adopt Core Animation's relative `settlingDuration`, so completion
    times match UIKit, at the cost of a second rest rule.
- **Q4. Syntax.**
  - Recommended: positional time and ratio, with a labelled `velocity`.
  - Or fully labelled arguments.
- **Q5. The reference iOS.** Recommended: pin iOS 27.0, the only one
  measured, and move deliberately. Signal supports iOS 15, which could not
  be measured here.
- **Q6. Opacity retargets.** UIKit's default restarts an opacity spring
  from the old target, a visible jump; `.beginFromCurrentState` restarts
  from the presented value. Copy the jump, or restart from the presented
  value (recommended)?
- **Q7. Release velocity.** When a hold releases into a duration-form
  spring, should the release velocity also re-solve k, as a UIKit app
  passing it as `initialSpringVelocity` would? The recommendation is no:
  keep k from the authored velocity, so the spring a reader sees in the
  source is the one that runs.

## 11. Review dispositions (r1 → r2)

Astra (A) and Grok (G), round 1. Taken, unless a finding says otherwise.

| Finding | Disposition |
|---|---|
| A1 D2 undefined for some inputs; zero derivative | Taken. D2 defines the domain (ζ ≥ 0.01), the zero-derivative stop, validation and pinned libm. |
| A2 f32 rounding changes roots | Taken. Authored parameters are f64 at every entry point, including tag 9 (D2, D6). |
| A3, G10 The final frame ramps; `99.99%` is not a repeated stop | Taken. The frames carry explicit offsets, with the left limit and the target at the same offset; `linear()` uses two 100 % stops (D6). |
| A4, G2 `composite: 'add'` multiplies `scale` and doubles absolute frames | Taken. One replace-mode animation of the engine's summed value (D6). |
| A5 240 Hz cannot follow fast springs | Taken. The rate adapts to max(240 Hz, 16 per oscillation) with an 8,192-frame bound, tested between frames (D6). |
| A6 The linearity claim ignores the overdamped quirk's injected velocity | Taken. Three stated conditions (D5). |
| A7, G6 Velocity handoff and zero distance | Taken. Precedence, release, the unchanged target and today's cancel are defined (D5). Rows R now measure mid-flight retargets and opacity. Rows X measure the unchanged value. |
| A8 The critical end has three roots | Taken. The last crossing; the animation `duration`, not `settlingDuration` (D3). |
| A9 Delayed springs; the ζ clamp's scope; LLP 1081 | Taken. `0s` means no end. The clamp applies to the duration and response forms, measured for both. 1081's name is used when it lands. |
| A10, G1 No reproducible band predicate; non-roots shipped | Taken. A closed predicate covers all 100 misses, and validation refuses non-roots (§4.3, D2). The kernel's runtime path has no warnings, so a dynamic string is refused or accepted. |
| A11 Separate mapping tests from renderer tests | Taken (§8.1–§8.3). The ζ ≥ 1 `settlingDuration` is dropped from stage 0: no form uses it. The probe needs availability guards for old runtimes (§4.6). |
| A12, G3 Metrics overstated | Taken. 6.0·10⁻⁸ in W and 1.2·10⁻⁷ in k at v 0; S and U are split; SwiftUI is compared with UIKit (5.4·10⁻⁸); 56 of 60 bit-identical; the icon's left limit is 1.773; 0.168 is attributed to the diary's tracker; "iOS 17 and later" is now "iOS 27.0". |
| A13 Q3's rationale | Taken (D3, Q3). |
| G4 60 s cap and the `linear()` stop limit | Taken. Over 60 s is refused; generated `linear()` keeps 64 stops by thinning (D3, D6). |
| G5 The overloaded third argument and the WebKit fourth argument | Taken in part. Velocity is labelled and a bare fourth number is refused. The positional time and ratio stay; Q4 holds the fully labelled alternative. |
| G7 `response` above critical; the clamp evidence | Taken. Rows X measure the overdamped physical timing parameters (`aod=false`, so they clamp), and `response` clamps (D1). |
| G8 One implementation; tag number; libm | Taken. Tag 9; the end time travels in the duration field; the JS integrators are replaced by engine frames; libm is pinned (D2, D6). |
| G9 `allowsOverdamping` is on every animate call and off for E2 | Taken. Rows X measure E2 below b 0; D4 states the split. |
| G11 ζ 0; the collapse threshold; f32 versus the claim | Taken. ζ 0 is refused (rows X show UIKit returns NaN); the collapse test is relative; the precision note is replaced (D2). |
| G12 Open questions | Taken. Q1's recommendation refuses non-roots; Q7 is added; overruns, `response` above ζ 1, the vector velocity and `.beginFromCurrentState` are now decided in the text. |
