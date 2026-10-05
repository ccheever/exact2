# LLP 1099: UIKit's springs, everywhere

**Type:** RFC
**Status:** Draft (r6), for Charlie's decision. Design only; nothing is built.
- r1 (`9b974e323`) was reviewed blind by Astra (`gpt-6-astra`, reasoning effort max): NEEDS REWORK, `llp/reviews/llp-1099-uikit-springs.astra.md`. It was also reviewed by Grok 4.7 (xhigh): SOUND WITH CHANGES, `llp/reviews/llp-1099-uikit-springs.grok.md`.
- r2 (`0926e91f8`) took every round-1 finding, one of them in part. It added measured retargets (rows R) and edge cases (rows X).
- Round 2 reviewed r2. Astra: NEEDS REWORK, `llp/reviews/llp-1099-uikit-springs-r2.astra.md`. Grok: NEEDS REWORK, `llp/reviews/llp-1099-uikit-springs-r2.grok.md`.
- r3 (`16f12c461`) took every round-2 finding. It added opacity with a retarget velocity (rows R), and it restored the round-1 Grok review's body, which r2's commit had lost.
- Round 3 reviewed r3. Astra: NEEDS REWORK, `llp/reviews/llp-1099-uikit-springs-r3.astra.md`. Grok: NEEDS REWORK, `llp/reviews/llp-1099-uikit-springs-r3.grok.md`. Their findings are narrower implementation gaps.
- r4 (`e240893f4`) folded every round-3 finding.
- Charlie approved up to six more rounds (4–9). Round 4 reviewed r4: Astra NEEDS REWORK (`-r4.astra.md`), Grok NEEDS REWORK (`-r4.grok.md`).
- r5 (`5afd8bd8f`) took every round-4 finding and measured transform retargets (rows R `scale`, `scale-up`, `scale-to-zero`, `rotate`), which settled how `scale` and `rotate` compose. Round 5: Astra NEEDS REWORK, Grok NEEDS REWORK (`-r5.*.md`).
- r6 takes every round-5 finding (§11).
**Systems:** `exact-motion` (`motion/src/spring.rs`, `parse.rs`, `transition.rs`, `engine.rs`, `easing.rs`), the wire (`kernel/src/wire/codec.rs`: a new easing tag 9), the kernel's transition check (`kernel/build.rs`), the Contract compiler's literal check (`contract/lower/src/values.rs`), the web host (`host/web/src/motion.rs`, `batch.rs`, `css.rs`, `motion-glue.js`, `presence-glue.js`), the JS target (`host/web-js/motion`, `shared.js`), Apple and Linux hosts (the per-frame seek, unchanged), parity fixtures (`host/web/src/parity.rs`, `host/web/tests/fixtures/browser-motion.txt`), new measurement fixtures (`motion/tests/fixtures/uikit-springs/`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-04
**Revised:** 2026-10-04 (r2, r3); 2026-10-05 (r4, r5, r6)
**Amends:**
- LLP 1002 D2: `spring()` gains three spellings and a labelled velocity.
- LLP 1003 §1: `transition-duration` on a spring becomes an end time, and generated `linear()` keeps the stop limit.
- LLP 1003 §4: a spring may end at a time.
- LLP 1003 §5: springs in the UIKit spellings retarget additively.
- LLP 1003 §7: `ln` joins the pinned libm functions.
- LLP 1007 §3: the frames carry explicit offsets and steps. A property animated by a UIKit spelling plays the engine's presented value. The "an interruption carries velocity" sentence applies to the restart forms.
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
Core Animation ends it: exactly for the duration form, and to 1.8·10⁻⁴ of
the time for the bounce form. The exception is the band below, where the
stiffness can differ. Measured on the iOS 27.0 simulator across 9,600
UIKit calls (§4):

- **Duration form at zero velocity.** UIKit's rule has a closed form. It
  agrees with all 308 measured calls to 6·10⁻⁸ in W and 1.2·10⁻⁷ in
  stiffness, both relative.
- **Duration form with velocity.** UIKit solves its equation numerically.
  A reconstruction of that solver (Newton, twelve steps from a fixed start)
  agrees with UIKit's stiffness in 99.2 % of a dense sweep.
  - Outside one band of inputs, defined by a closed predicate, the curves
    differ by at most 1.4·10⁻⁷ of the move on the measured grid.
  - Inside the band, UIKit's solver is chaotic, so the reconstruction can
    pick another root or no root. A result that is not a root is refused.
    Otherwise a literal inside the band gets a warning, and a dynamic
    string there is refused. One of Signal's real
    calls is in the band (§4.3).
- **iOS 17 bounce form.** Stiffness and damping match in all 60 calls,
  within one ulp (56 are bit-identical). The end time agrees to 2·10⁻⁴.
- **Core Animation's curve.** Hand-built `CASpringAnimation`s match the
  closed form to 6.4·10⁻⁸. That includes a measured quirk in the overdamped
  branch (§4.5). The springs UIKit itself creates match to 4·10⁻⁵.
- **Retargets.** A mid-flight retarget runs independent springs that
  combine (rows R):
  - position and rotation add (0.007 pt over a 300 pt move; 0.003° over
    150°);
  - scale multiplies (3.3·10⁻⁵);
  - opacity is replaced instead.

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
| `UISpringTimingParameters(mass:stiffness:damping:initialVelocity:)` | Signal's `springResponse` helper: k = (2π/response)², c = 4πζ/response | that spring, overdamping clamped to critical. It ends at `settlingDuration` below critical (rows D); clamped from overdamped, it ends at the critical settle time, 0.9235 s against a `settlingDuration` of 1.0 (rows X) |
| `UIView.animate(springDuration:bounce:initialSpringVelocity:)` (iOS 17) | — | k = (2π/d)², c from the bounce, `allowsOverdamping` on, ending at a settle time (rows E) |
| SwiftUI `.spring(response:dampingFraction:)`, `Spring(duration:bounce:)` | — | SwiftUI's own evaluator, not measured; its fields are (rows F, G) |

In build 18 the clone treated "0.4 s, 0.8" as a response of 0.4 s. Its
tracker measured that as 0.168 of the move off UIKit's real curve, and
0.0023 off once it used the probe's numbers (DIARY, build 19). Recomputed
from the closed forms for that call (0.4 s, 0.8, v 1), the response
reading differs from UIKit's curve by up to 0.206 of the move. Across the
measured grid at v 0 it reaches 1.965 (ζ 0.01, 0.1 s). UIKit's rule is not
a response.

## 2. Design

### D1. Spellings

`spring()` keeps its place in the `transition` shorthand and in
`layout-transition`. The function token is `spring` until LLP 1081 lands
and `-exact-spring` after it, never both at once. The argument grammar
below is the same under either token.

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
  This matches `UIView.animate`'s single scalar.
- *A vector velocity is future work.* `UISpringTimingParameters` with a
  vector velocity (rows X) makes one animation per axis. The axes share one
  stiffness, the one solved for v 1 in both (1, 5) and (5, 1), and each
  axis has its own `initialVelocity`. One scalar cannot express that.
- *Physics properties only.* An authored velocity on a paint property is
  refused. Paint springs carry no velocity (LLP 1062 D3).

**Clamping ζ.** In the duration and response forms, ζ > 1 is clamped to 1.

- *Duration form.* UIKit clamps it (rows A, ζ 1.01–5 are identical to
  ζ 1).
- *Response form.* Core Animation clamps the damping of the timing
  parameters Signal's helper uses (rows X: `aod=false` for k 100, c 40).
- *Physical form.* It keeps true overdamped physics, as today.

**Refusals.** Each is a `BadEasing` with a reason:

- duration or response outside [1 ms, 60 s];
- ζ < 0.01 (unmeasured below; UIKit returns NaN at ζ 0, v 0, and k ≈ 10⁶ at ζ 0, v 1: rows X);
- bounce outside (−1, 1);
- a non-finite argument;
- `bounce` with `response`;
- a bare fourth number;
- a duration-form result that is not a root (D2);
- resolved coefficients that fail `SpringConfig::validate`
  (`motion/src/spring.rs:79`), or an initial velocity v·Δ that is not
  finite. Both are checked before any motion state changes;
- a dynamic (runtime) duration-form string inside the band (§4.3);
- a spring whose web lowering would exceed the frame bound (D6). Every
  host refuses it at parse, so no host plays a spelling another refuses;
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
- **Zero velocity.** At v = 0 the procedure reaches a closed form, to
  within the 10⁻⁵ gate for every ζ' ≤ 0.999999 (every measured row).
  Closer to 1, twelve steps no longer converge (0.027 off at
  ζ' = 1 − 10⁻⁹). The closed form is the definition, and implementations
  use it:
  - for ζ' < 1, ζ'W = ln(ζ' / (ε·√(1 − ζ'²)));
  - at ζ' = 1, the logarithm is singular, so W is the root of
    (1 + W)·e^(−W) = ε, W = 9.233413476451585.

  The logarithm needs ζ'/√(1 − ζ'²) > ε, which holds for every
  ζ' > 0.001. The probe never went below ζ 0.01, so ζ ∈ (0, 0.01) is
  refused as unmeasured.
  - *Examples.* ζ 0.8 gives W 8.99430, so 0.4 s gives k = 505.609 (UIKit:
    505.6086). ζ 0.5 gives W 12.7169.
- **Validation.** The result is accepted only when it is finite, lies in
  (0, 10⁴], and |g(W)| ≤ 0.1·ε. Otherwise the spelling is refused.
  - *Why the threshold.* Where the procedure matches UIKit, |g| is at most
    0.059·ε. That allows for UIKit's own unconverged answers, which the
    twelve-step cap reproduces.
  - *What it refuses.* It refuses 27 of the 9,474 measured inputs. All 27
    are in the band, and UIKit chose differently on every one.
- **Precision.** The authored numbers, including the end time, are f64 at
  every entry point:
  - literal text, as `parse.rs` keeps them today;
  - the new wire tag (D6), unlike tag 7's f32s and the row's f32 duration
    field.

  A cut at an authored `200ms` therefore happens at 0.2 s on every path.
  Through an f32 it would happen at 0.20000000298 s, where the icon is
  still 1.77 of the move away.

  Inside the band, rounding the inputs to f32 can change which root the
  procedure finds. At ζ = u = 0.05, for example, the result goes from
  W = 5 to 1.3·10⁹. That is why the inputs stay f64.
- **Determinism.** `exp` goes through `motion`'s pinned libm 0.2.16, and
  `sqrt` stays the IEEE intrinsic (`motion/src/math.rs`). `ln` is not
  pinned today. This RFC amends LLP 1003 §7 to add it from the same libm
  0.2.16. With that, the procedure, the closed forms and D3's end times are
  bit-identical on every host.

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
  - *ζ ≥ 1.* It ends at the last T with f(T) = |P(T)|·e^(−ωT) = ε, where
    P(T) = −1 + (v − ω)T. This is the critically damped settle time, and
    it applies even when the spring is overdamped. The equation can have
    three roots, so the last crossing is found by bracketing:
    - P is linear, so f has at most two lobes: one from T = 0 and, when
      v > ω, a second one after P's zero T₀ = 1/(v − ω), peaking at
      T₀ + 1/ω.
    - The end is on the falling side of the last lobe whose peak exceeds ε.
      The falling side is monotone, so it is found by bisection to f64
      resolution.
    - This avoids fixed-step scans, which miss a narrow late lobe. At
      ω = 1 and v = 1.2261769259289865 the last crossing is 5.421646815,
      not the first lobe's 4.142853. The lobe is that sensitive: at
      v = 1.2262 the answer is 5.454495022.
  - *Agreement.* UIKit's ends match to 1.8·10⁻⁴ relative (60 rows). This
    is a reconstruction of the time, not the time itself. Within that
    window around the end, the engine can have snapped while UIKit has not,
    or the reverse. At 0.4 s and b −0.5 the window is 0.1 ms, and the gap
    inside it is the snap, 0.0065 of the move. §8 tests the end-time
    reconstruction separately from the curve.
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
  after they start. A longer end is refused, not clamped. The frame bound
  (D6) is a separate check: a spring within 60 s can still exceed it, for
  example `spring(1s, bounce 0.9, velocity 20000)` (ends at 23.8 s; needs
  about 17,900 frames), and is then refused.
  - At d = 3 s and v 0, the bounce form crosses 60 s at b ≈ 0.9446 (b 0.95 ends at 66.43 s and is refused).
  - b 0.9 with d 1 s ends at 11.146 s, which is allowed.
- **Completion and the 10 s cap.** Once a component has an end time, the
  10 s `MAX_DURATION` snap (`transition.rs:354`) does not apply to it.
  A property's completion, and `clock settle`, come at the latest end among
  its live components.
- **Delays.** Each new component waits the transition's delay before it
  starts, as a new transition does today.
  - *During the delay* the component holds its starting residual, −Δ_new
    (or its starting factor, for `scale`), as UIKit's additive animation
    holds its `fromValue` with backwards fill. The presented value is
    therefore unchanged by the new target until the delay ends, and older
    components keep moving.
  - *At the start* the component's velocity begins. The value is
    continuous there and its derivative is not, so the web's frame grid
    breaks at every component start as well as at every end (D6).

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

**What Contract does.** On physics properties, the duration and bounce
forms, and any spring given an end time, keep a list of additive components
per property. Paint properties keep no list: a paint spring in any spelling
interrupts as CSS does, replaced from the presented colour with no carried
velocity (LLP 1062 D3).

- **How components combine.** Rows R measure `center`, `scale`, `rotate`
  and opacity; `height` and layout follow `center`'s rule:
  - *By addition* for `translate`, `height`, layout and `rotate`. A
    rotation is additive because rotations about one axis concatenate by
    adding angles. The measured `rotate` retarget (60° then 150°) matches
    the sum to 0.003°.
  - *By multiplication* for `scale`. UIKit's additive transform animations
    run from old·new⁻¹ to the identity (rows R: `from` is 2.0 for 1 → 0.5,
    and 0.667 for 2 → 3). Each component's factor therefore runs from
    old/new to 1 along its curve, linearly in the factor. Its velocity is
    v times the factor distance (1 − old/new). The presented scale is the
    target times the product of the live factors. It matches the measured
    retargets to 3.3·10⁻⁵.
  - *Not at all* for `opacity`. It is replaced, not combined (below).
- **Scale through zero.**
  - *A target of 0.* UIKit's presented scale is 0 from the retarget on,
    because the target multiplies every factor (rows R `scale-to-zero`).
    Contract does the same and collapses the live scale components.
    UIKit's would reappear on a later retarget within their lifetimes;
    that is a declared deviation.
  - *Leaving 0.* A retarget away from 0 has old/new = 0, so its factor runs
    from 0 to 1, which is defined.
  - *Negative scales.* They need no special case: factors keep their sign.

The components work like this:

- **A new target adds a component.** It starts at −Δ_new, where Δ_new is
  the new target minus the old one, with velocity v·Δ_new.
- **Old components keep running.** Each runs on its own curve and is
  dropped, with its snap, at its own end.
- **The presented value.** For the additive properties it is the target
  plus the sum of the components' residuals. For `scale` it is the target
  times the product of the factors.
- **Collapsing.** A component is collapsed into the target once it has
  ended. One without an end is collapsed once both its offset and its
  velocity are below 10⁻⁶ of its own Δ (a relative test).
- **A bound on the list.** At most 64 components live per property. A
  65th folds the oldest two into one component:
  - *Which two.* The oldest are the ones with the earliest begin time, ties
    broken by insertion order.
  - *Where it starts.* The replacement starts from their combined offset
    and combined actual derivative. Offsets add and derivatives add for the
    additive properties. For `scale`, the factor is f₁·f₂, so the
    displacement handed to `spring.rs` is f₁·f₂ − 1. Its derivative follows
    the product rule, f₁′f₂ + f₁f₂′. The presented value
    and velocity are exact at the fold.
  - *How it runs.* It runs on the textbook branch with the newer
    component's k, c, m and end time.
  - *Cost.* After the fold, the trajectory can differ from UIKit's. That
    is a declared deviation that only a 65-deep pile of retargets reaches,
    and §8.4 pins the sample at the fold and one frame after.

**Why not today's restart.** LLP 1003 §5 restarts from the presented value
and velocity, and that is exact for two components only under strict
conditions. The ODE is linear, so these conditions are sufficient for the
sum to equal a single restart:

1. the components have equal k/m and c/m;
2. the restart starts from the presented offset with the *sum of the
   components' actual derivatives*, including the new component's impulse.
   For the quirk, that impulse is (v + 2ζω)·Δ, not v·Δ. The quirk's curve
   still solves the same ODE, so a textbook restart from the summed
   derivative reproduces it too. For k 100, c 10, m 1, with Δ 100 then
   Δ 200 at v 1 and a retarget at 100 ms, the right initial derivative is
   746.13, against 546.13 from carrying only the old velocity;
3. the time is before the first component's end.

Different parameters or a component's cut break them, and then only the
sum reproduces UIKit, which is why the UIKit spellings use it.

**The physical and response forms** without an end time keep LLP 1003
§5's restart, velocity included.

**Velocity, in order of precedence:**

1. **Release velocity.** A hold or gesture that catches a property
   (LLP 1057.003) collapses its components into the presented value and
   velocity, as it catches a spring today. On release it starts one
   component from the presented value whose *actual* initial derivative is
   the release velocity, per axis, in property units.
   - *What stays authored.* k, c and the end time stay what the authored
     spelling gives: d for the duration form, and D3's end computed with
     the authored v for the bounce form. A release never re-solves
     anything, so unequal axes and zero distance need no special case.
   - *The overdamped quirk.* The quirk's branch cannot hold an arbitrary
     derivative at zero distance, and at nonzero distance it would add
     2ζω·Δ. A released component therefore uses the textbook branch with
     the same k and c, so the release velocity is what the user sees.
   - *`scale` releases are absolute.* A released `scale` component is an
     additive residual in scale units, not a factor. A factor cannot carry
     a velocity when the target is 0: target × factor is 0 whatever the
     factor does, so a release toward 0 at 1 /s would show nothing where
     the textbook curve moves 0.0188 by 50 ms.
     - *The presented scale* is then target × ∏(factors) + Σ(released
       residuals).
     - *Later retargets* add factor components as usual. The released
       residual keeps running and adds.
     - *A fold* only combines two components of the same kind.
   - *How this differs from UIKit.* A UIKit app passes the release
     velocity divided by Δ into the solve, which changes k as well. That
     difference is declared (Q7).
2. **Authored velocity.** v·Δ_new for a new component; v·Δ for a restart.
3. **Nothing authored.** An omitted velocity is 0 on a new UIKit-spelling
   component. The restart forms still carry the presented velocity, per
   LLP 1003 §5.

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
  call replaces the first.
  - *It drops the inherited velocity and keeps the authored one.* A second
    call at v 1 gets `initialVelocity` 1 and v 1's stiffness (497.54).
  - *Where it starts.* By default it starts from the *model* value, the old
    target, so the presented opacity jumps (0.40 → 0.20 at 100 ms). With
    `.beginFromCurrentState` it starts from the presented value.
- *What Contract does.* Contract has no per-call options. An opacity
  spring in a UIKit spelling restarts from the presented value. It drops
  the inherited velocity and starts with the authored v·Δ, where Δ is
  measured from the presented value to the new target, as Core Animation
  measures its from → to. That is UIKit with `.beginFromCurrentState`.
  UIKit's default jump is a declared deviation (Q6).

### D6. Every host

There is one implementation, in `exact-motion`:

| Host | How it plays | Agreement |
|---|---|---|
| iOS, macOS | per-frame seek of the engine (`CADisplayLink`), as today. No `CASpringAnimation`: a Core Animation executor stays "permitted, not built" (`rules/DEFERRED.md`). If one is built, the duration and bounce forms map onto `CASpringAnimation` (mass 1, k, c, v, `duration` = end, `isAdditive`, `allowsOverdamping` as in D4) | bit-identical (libm pinned, no FMA; LLP 1003 §7) |
| Linux | per-frame seek, as today | bit-identical |
| Web (wasm host), compositor properties (`translate`, `scale`, `rotate`, `opacity`) | one WAAPI animation per property with replace compositing, as today (`motion-glue.js:581`, LLP 1007 §3). Its frames are the engine's presented absolute value, components summed, re-lowered at every retarget as an interruption is today. There is no `composite: 'add'` here: CSS adds `scale` by multiplying | the band below |
| Web, layout (`layout-transition`) | residual translate frames, offset from the resting box with rest value 0, played with `composite: 'add'`, as LLP 1063 D6 already does for translation. The engine supplies the frames: `presence-glue.js`'s JS integrator, which uses unpinned `Math.exp`, is replaced | the band below |
| Web, `height` | absolute frames with replace compositing, like the compositor properties. A residual cannot work here: a negative height is invalid, and `motion-glue.js:98` clamps it to 0 | the band below |
| Web, paint properties | `linear()` from the curve (LLP 1062 D3). The end is two stops at 100 %: `linear(…, x(d⁻) 100%, 1 100%)`. CSS takes the later stop when two stops share an input, and so does `easing.rs`: past the last stop it returns the last output (`:334`), and for an equal-input pair it returns the later stop (`:340`). Paint has no interior steps (it keeps no components), so only the terminal pair occurs. Stops are placed by error, not evenly: each new stop goes where the piecewise-linear curve is furthest from the spring, until every point is within 10⁻² of the move. A paint spring that needs more than 64 stops (`easing.rs:61`) to get there is refused, at parse, on every host. Fast oscillating springs are refused on paint for this reason: 0.1 s at ζ 0.01 has 73 extrema | 10⁻² of the move |
| JS target | static springs lowered at build (LLP 1071) through the engine. Dynamic springs, including paint springs, are lowered at runtime by the same `exact-motion` code into the same frames and `linear()` text. Today `host/web-js/src/style.rs:534` strips a dynamic spring from CSS and the JS motion module observes only the compositor properties, so this is new work. Stage 4 deletes `shared.js`'s `springCurve` | as the web |

**Frames.**
- *Explicit offsets everywhere.* Frames carry explicit offsets through
  every consumer: the batch op, `motion-glue.js` (including
  `timelineEasing`, `motion-glue.js:78`, which rebuilds uniform offsets
  today and would turn a step into a ramp), and the presence glue.
- *Breaks at starts.* The grid has a frame exactly at every component
  start, including a delayed one, and spacing restarts there. The curvature
  bound never spans a derivative jump. Without this break, a 10 ms delayed
  start at velocity 100 is off by 0.055 units between frames.
- *Steps.* At every component end, the end of the whole animation and every
  interior end left by a retarget alike, there are two frames at the
  *same* offset: the summed value just before the end, then just after. A
  snap is therefore a step at every offset where UIKit has one. At 240 Hz
  without the step, Signal's icon would be off by 0.387 of the move in the
  last interval.
- *Error-controlled spacing.* There is one frame grid per track, spaced
  so that linear interpolation of the presented value stays within
  5·10⁻⁴·S of the engine.
  - *The scale S.* It is fixed when the track is lowered, per axis:
    - S = Σ|Δᵢ| over the live components when that sum is at least
      10⁻³ property units (the move);
    - otherwise max(Σ|v₀ᵢ|/ωᵢ, 10⁻³ units), where v₀ᵢ is a component's
      actual initial derivative. This covers a release at zero distance.

    For `scale`, Δᵢ is the component's factor distance times the target.
  - *The step size.* From each frame at t, the next step is the largest
    h ≤ 0.25 s with h ≤ √(8 · 5·10⁻⁴ · S / M(t, h)). M(t, h) = Σᵢ Mᵢ bounds
    |x''| of the summed track over [t, t + h]. Where M depends on h (the
    critical branch), the largest such h is found by bisection. An axis
    with M = 0 everywhere (no displacement, no velocity) is skipped. The
    step is the smallest over the other axes.
  - *The bound per branch.* Each Mᵢ comes from a closed-form envelope, with
    A and B the component's property-unit coefficients as in
    `spring.rs:110-111` and `:120-121`:
    - ω²·√(A² + B²)·e^(−ζωt) when underdamped;
    - (ω²(|A| + |B|(t + h)) + 2ω|B|)·e^(−ωt) when critical;
    - Σ|Cⱼ|sⱼ²e^(sⱼt) on both overdamped branches, the quirk included.

    For `scale`, the bound is on the second derivative of the product
    s = T·∏fᵢ, where T is the target:

      s'' = T·[Σᵢ fᵢ''·∏_{j≠i} fⱼ + 2·Σ_{i<j} fᵢ'·fⱼ'·∏_{k≠i,j} fₖ].

    Each factor is bounded on the step as follows:
    - |fᵢ| ≤ 1 + Eᵢ;
    - |fᵢ'| ≤ ωᵢ·Eᵢ·Kᵢ;
    - |fᵢ''| ≤ ωᵢ²·Eᵢ·Kᵢ².

    Here Eᵢ is the factor's displacement envelope |fᵢ − 1| from its branch,
    as above with A and B in factor units, and Kᵢ is 1 underdamped and 2
    otherwise. Released residuals add their own Mᵢ. Per-component envelopes
    are not enough here: on a 1 → 0.1 → 0.01 retarget, |s''| is 3.8 times
    their sum.
  - *Why not a fixed rate.* A fixed rate fails fast springs: 16 frames per
    oscillation leaves 0.019 of the move. It also over-samples slow ones:
    60 s at 240 Hz is 14,400 frames.
- *A frame bound on every host.* Lowering is bounded at 16,384 frames.
  - *At parse.* The count is computed from the normalised curve: Δ = 1,
    the authored velocity, and no floor. It is checked when the spring is
    parsed, on every host. A spring that would exceed it is refused
    everywhere, not only on the web. A move small enough for the 10⁻³
    floor to dominate takes fewer frames, so the parse check still bounds
    it.
  - *At run time.* Run-time states are not known at parse: a release
    velocity, or a pile of components. When one of those needs more than
    16,384 frames, the web lowers it at the bound.
    - *What stays exact.* Every component-end offset, with its pair of
      frames, and the final snap stay exactly where they are.
    - *What moves.* Only the open gaps between them grow, in proportion.
    - *The cost.* That track is not held to the 5·10⁻⁴·S band.
    - A user's gesture is never refused. Native hosts sample the engine
      directly and are unaffected.
  - *How far measured calls are from it.* The fastest measured call (0.1 s,
    ζ 0.01) needs about 2,200 frames, Signal's calls about 50, and a
    60 s critical spring about 240.
- *The parity test.* The new cases get their own comparator: at sample
  times taken *between* frames, |browser − engine| ≤ 10⁻³·S. Each case
  stores its S in the fixture. The existing `spring` case keeps
  `parity.rs`'s absolute 10⁻³ (`parity.rs:38`), which is in property units
  and suits its 0.5 scale move. The between-frames allowance, 5·10⁻⁴·S, is
  half the band, which leaves room for the browser's own timing.

**Displayed values are clamped; the engine's are not.**
- *The ranges.* A property's displayed value is clamped to its range on
  every host: opacity to [0, 1], `height` to ≥ 0. Native height already
  clamps each sample (`host/apple/src/height.rs:301`); the web clamps each
  frame (`motion-glue.js:96-98`). The engine keeps the unclamped value, so
  velocity and later components are unaffected.
- *Where the clamp kicks in.* Clamping frames is not the same as clamping
  the curve between them. So the web grid also places a frame at every
  crossing of a clamp bound: found by bisection on the engine curve, the
  value clamped there, and spacing restarting. Without that frame, a 100 →
  0 height undershoot is off by 0.37 px between frames.
- *What parity compares.* `getComputedStyle` returns clamped values
  (`parity.html:15`), so the parity cases compare displayed values:
  clamp(engine) against the browser.

**The wire.** A new easing tag, 9, carries:

- the form: physical, duration, bounce or response;
- the authored parameters, as f64;
- the velocity, if present;
- the end time of a physical or response spring, as f64, if present;
- the delay, as f64, if present.

With tag 9 the row's f32 duration and delay fields must be 0, so each time
has one home and one precision. A delayed cut (100 ms delay, 200 ms end)
then happens at 0.3 s on the text path and the wire path alike. Through
the f32 times it would happen at 0.30000001192 s if added in f32, or
0.30000000447 s if the two f32 values are added in f64. At either time
Signal's icon spring is still 1.77 of the move away. Tag 7 stays as it is for the zero- and three-argument
spellings.

## 3. Semantics worth stating

### 3.1. The snap

A duration-form spring ends at d, and UIKit's settle test drops the cosine
term. So the value at d is not within 10⁻³ of the target:

- **Zero velocity.** The residual at d is up to ε·√(1 + B²)/|B|: 1.25·10⁻³
  at ζ 0.8, but ε/ζ ≈ 1.7·10⁻² at ζ 0.06.
- **Velocity near ζω.** B → 0 there, so the solve picks a soft spring.
  - *Signal's reply icon* (0.2 s, 0.06, v 0.8). UIKit resolves it to
    k 184.95 and c 1.632. UIKit's left limit at 0.2 s is 1.773 of the move,
    and the last 10 ms sample before the end reads 1.724. Scaling 1 → 1.16,
    the icon swings to about 1.28 and snaps back to 1.16.
    - *This call is in the band (§4.3).* D2's procedure resolves it to
      k 171.05 and c 1.569 instead, with a left limit of 1.738, a curve gap
      of 0.06 of the move.
    - *The warning.* The literal therefore gets the band warning, and the
      way to UIKit's exact curve is the physical spelling with the probe's
      numbers.
  - *(1 s, 0.3, v 1).* UIKit and the procedure agree on k 10.9, and the
    spring is at 1.375 at its last sample.

That is UIKit's behaviour, and outside the band the duration form
reproduces it. A port that snaps is correct.

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
| R | retargets over time: `center` and opacity (with and without `.beginFromCurrentState`), `scale` down and up, `scale` to 0, `rotate` |
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
| v = 0, closed form (A, W, B, C) | 308 | 308; worst relative 6.0·10⁻⁸ in W, 1.2·10⁻⁷ in k | ≤ 1.1·10⁻⁷ |
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
0.6 ≤ u/ζ' ≤ 8.**

- *Coverage.* It holds all 100 misses among the 9,474 measured inputs
  (A, W, and the U rows), including Signal's icon: u = 0.16 and ζ = 0.06
  give u/ζ' = 2.7.
- *Margin.* The misses span u/ζ' from 1.0 (ζ 0.05, u 0.05) to 5.95
  (ζ 1). The bounds leave margin on both sides.
- *Cost.* It flags 1,916 of the 9,474 inputs (20 %). It never flags v ≤ 0.
- *Signal's menu call.* It is outside the band (u/ζ' = 0.5), which is why
  the lower bound is not lower.
- *Not sampled.* Below u = 0.05 the sweep has no samples. Stage 0 adds a
  dense sweep there for ζ ≤ 0.1, where u = 0.05 is already u/ζ' ≥ 0.5.

**What happens inside the band.**

- **Literals.** A literal is refused if validation fails (D2). Otherwise
  it is accepted with a warning, `lower-spring-uikit-band`.
  - *How it survives run time.* Literal transitions compile to strings that
    reach the kernel through `StyleProps::set_dynamic`: the compiler interns
    the string (`contract/lower/src/expr.rs:89`) and the runner converts it
    to a style value (`runner/src/bridge.rs:108`). The kernel cannot tell them from dynamic
    ones, so it would refuse them too. The compiler therefore rewrites an
    accepted in-band literal to its resolved physical spelling with the end
    time and velocity, for example `200ms spring(171.054…, 1.569…, 1,
    velocity 0.8)` in f64. That spelling is the same curve, and the band
    check never applies to it.
  - *Where the warning appears.* Today the compiler reports diagnostics
    only when compilation fails (`contract/cli/src/build.rs:59-93`).
    Stage 2 adds a warning channel: `contract build` prints warnings on
    success, and the agent's report carries them. If that channel is not
    wanted, the alternative is to refuse in-band literals (Q1).
  - *The refusal* says the procedure found no root and suggests the
    physical spelling with an end time. It quotes the probe's numbers only
    for a measured call (Signal's icon below). For any other input it says
    to measure with `probe.sh`.
  - *The warning* says UIKit's own solver is ill-conditioned there and
    that the result may not be UIKit's: it can be another root, an O(1)
    difference in the curve. It suggests pinning the spring with the
    physical spelling and an end time, taken from the probe. For Signal's
    icon that is `200ms spring(184.95, 1.632, 1, velocity 0.8)`.
- **Dynamic strings.** A dynamic string inside the band is refused
  (`BadTransition`). The kernel has no warning channel
  (`kernel/build.rs:1306`), and a silent O(1) difference is worse than a
  refusal the author can see in the agent's report. The literal and
  dynamic paths differ for that reason only. Q1 asks whether literals
  should be refused too.

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

Stage 0 tasks (§9):

- iOS versions other than 27.0, because no other runtime is installed. The
  probe needs availability guards to build below iOS 18.
- A physical device.
- A retarget between springs of different forms.
- The duration form below u = 0.05 at ζ ≤ 0.1 (§4.3).

Not needed by this RFC, so not scheduled:

- Core Animation's `settlingDuration` at ζ ≥ 1. No form uses it; it
  appears to step in 0.1 s.
- SwiftUI's rendered curves. Contract follows UIKit's springs, not
  SwiftUI's evaluator.

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
- **The band was not fixed by tuning.** No variant tried was exact (§4.3).
  That does not prove none exists, and the band is known only on the
  sampled grid of the reference iOS. Refusal, the warning and the escape
  hatch are the answer for now.
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
   - Bounce ends hold to 2·10⁻⁴, as a test of the end-time reconstruction
     on its own.
   - Last-crossing cases near a tangent lobe and at large velocity
     (ω 1: v 1.2261769259289865 → 5.421646815; v 1.2262 → 5.454495022).
   - Refusals of overflowing coefficients (`spring(1e-200s, 0.8)` is outside
     [1 ms, 60 s]; a d of 1 ms with ζ 0.01 still validates).
2. **Renderer (D4).** Every U and S curve, fed its measured coefficients
   *and its measured end time*, matches the engine:
   - to 10⁻⁴ for t < end;
   - exactly the target for t ≥ end;
   - checked at end − 1 µs, end and end + 1 µs.
3. **Authored text to pixels.** For the canonical spellings (both of
   Signal's, (1 s, 0.3, velocity 1), each bounce at d 0.4 s, and a
   physical spring with velocity and an end time), the path runs from
   string to wire tag 9 to engine to every host:
   - native hosts compare frames bit for bit;
   - `parity.rs` gains `uikit-duration`, `uikit-cut`, `uikit-bounce-overdamped`
     and `uikit-retarget`, recorded in Chrome. They are compared between
     frames with |browser − engine| ≤ 10⁻³·S, S stored per case (D6), with
     the step at the end time;
   - Signal's icon is expected to differ from UIKit by 0.06 (§3.1, §4.3)
     and is pinned as such;
   - the cut is tested at the authored end time exactly, on the text path
     and the wire path alike.
4. **Retargeting.**
   - Engine tests replay rows R to 10⁻⁴ of the move: the two-call `center`
     retarget at 100 ms and 250 ms, the opacity replacements with authored
     velocity 0 and 1, the `scale` and `scale-up` products, `scale-to-zero`
     and the `rotate` sum.
   - An in-band retarget whose first component snaps by more than half the
     move mid-animation checks that interior steps reach every host,
     `timelineEasing` included.
   - Web frames for an exact zero-distance release: it plays, it is not
     refused, and its S is max(|v₀|/ω, 10⁻³ units). Also a small-Δ,
     high-velocity release that trips the run-time frame bound. For the
     latter the assertions are: the track plays, it has at most 16,384
     frames, and its step offsets equal the engine's component ends. The
     5·10⁻⁴·S band may be exceeded.
   - The 65th retarget, with unequal coefficients and different ends,
     pins the sample at the fold and 1/240 s after, for an additive
     property and for `scale`. For `scale`, the folded component's
     displacement from its equilibrium is f₁·f₂ − 1.
   - A `scale` release toward 0 at 1 /s (0.0188 at 50 ms on
     `spring(400ms, 0.8)`), then a retarget while it runs.
   - A delayed start at high velocity, sampled between frames across the
     start.
   - Displayed values: a `height` shrink that crosses 0, and an opacity
     curve that overshoots 1, compared after clamping.
   - A `uikit-bounce-overdamped` midpoint regression at b −0.5, held to
     10⁻³·S.
   - A delayed retarget: the second component waits the delay.
   - A `scale` retarget through 0 and away from it, and a negative scale.
   - `height` growing and shrinking on the absolute track.
   - A delayed cut, tested exactly at delay + end on the text and wire
     paths.
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
   - A dynamic string inside the band is refused. The same text as a
     literal is accepted, rewritten and warned, and it plays the same
     curve end to end.
   - A paint spring needing more than 64 stops is refused; a paint
     duration-form retarget interrupts as CSS does.
   - A spring over the frame bound is refused on every host:
     `spring(1s, bounce 0.9, velocity 20000)`.
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
   - A dense duration-form sweep below u = 0.05 at ζ ≤ 0.1.

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
   - `contract/lower` diagnostics, including the band warning, the rewrite
     of in-band literals, and a warning channel on successful builds.
   - LLP 1003 §7's amendment: `ln` from libm 0.2.16.
   - Docs: `docs/contract-for-agents.md` and `-for-humans.md`.
   - If LLP 1081 has landed, the function is `-exact-spring`.
3. **Retargeting.** Additive components and the velocity precedence (D5),
   with the §8.4 tests.
4. **Hosts.**
   - Web frames with explicit offsets, error-controlled spacing and steps
     at every component end, through `motion-glue.js` (`timelineEasing`
     included) and the presence glue.
   - Paint `linear()` with the double 100 % stop.
   - The JS target: engine frames in place of its integrators, and runtime
     lowering for dynamic springs, paint included.
   - The Chrome parity fixture re-recorded (§8.3, §8.6).
5. **The Signal clone.** Replace its hand-sampled `linear()` curves with
   the spellings, and film both against UIKit.

## 10. Open questions for Charlie

- **Q1. The band.**
  - Recommended: accept the reconstruction; refuse non-roots; inside the
    predicate, warn on literals (compiled to their resolved physical
    spelling) and refuse dynamic strings. This needs the new warning
    channel.
  - Or authorise reverse-engineering UIKit's solver to make the band
    exact. That is a legal and licence call.
  - Or refuse every duration-form literal inside the predicate. That
    refuses 1,916 of the 9,474 measured inputs (20 %), never one at zero
    velocity, but it does refuse one of Signal's two spelled calls: the
    reply icon (u/ζ' = 2.7). That call would then have to be written in the
    physical spelling, `200ms spring(184.95, 1.632, 1, velocity 0.8)`. The
    menu call (u/ζ' = 0.5) stays accepted.
- **Q2. The overdamped branch.** Should the bounce form below b 0 copy Core
  Animation's overdamped branch (recommended: it is what
  `animate(springDuration:bounce:)` renders on iOS 27.0)? The alternative
  is ideal physics, declared as a deviation.
- **Q3. Where the physical and response forms end.**
  - Recommended: keep exact2's absolute rest, so existing springs do not
    move.
  - Or adopt Core Animation's end, so completion times match UIKit, at the
    cost of a second rest rule. That end is UIKit's animation duration: the
    `settlingDuration` getter below critical damping, and the critical
    settle time when overdamping is clamped (rows X: 0.9235 s against the
    getter's 1.0).
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

## 11. Review dispositions

### Round 1 (r1 → r2)

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

### Round 2 (r2 → r3)

Astra (A) and Grok (G), round 2; both NEEDS REWORK. Taken, unless a
finding says otherwise.

| Finding | Disposition |
|---|---|
| A1, G2 A fixed 16-per-oscillation rate still misses 10⁻³, and the 8,192-frame cap refuses legal springs | Taken. Error-controlled spacing from closed-form bounds on \|x''\| on every branch, at 5·10⁻⁴ of the move; a 16,384-frame bound checked at parse on every host. The fastest measured call needs about 2,200 frames (D6). |
| A2 The end time still travels as f32 | Taken. Tag 9 carries the end as f64, and the row's duration must be 0 with it; the cut is tested at the authored time on both paths (D2, D6, §8.3). |
| A3 A fixed-step scan misses the last crossing | Taken. Analytic lobe bracketing, then bisection on the monotone falling side (D3). `fit.mjs` implements it and returns 5.4216 for the counterexample. |
| A4 Release velocity versus the quirk's 2ζω; per-axis release; Δ = 0 | Taken. A release sets the *actual* derivative and uses the textbook branch with the same k and c. Its end is the authored spelling's end, so nothing is re-solved per axis (D5). |
| A5 The opacity probe passed velocity 0 on both calls | Taken. Rows R add a second call at v 1: UIKit drops the inherited velocity and keeps the authored one, with v 1's stiffness. D5 now says so. |
| A6, G3 Steps only at the final offset; `timelineEasing` rebuilds uniform offsets | Taken. Two frames at the same offset at every component end; offsets carried through every consumer, `timelineEasing` included; an in-band snap test (D6, §8.4). |
| A7 Dynamic paint springs on the JS target | Taken. Runtime lowering through `exact-motion` for dynamic springs, paint included (D6, stage 4). |
| A8 Coefficients can overflow | Taken. Durations limited to [1 ms, 60 s], and resolved coefficients and v·Δ validated through `SpringConfig::validate` before any state changes (D1). |
| A9 The bounce end is approximate, so a renderer test using it is unfair | Taken. Renderer tests use measured end times; the end-time reconstruction is tested on its own, and its window is stated (D3, §8). The summary's "ends where Core Animation ends it" is qualified. |
| A10 Linearity conditions | Taken. k/m and c/m, and the sum of actual derivatives including the new impulse, with the worked number (D5). |
| A11, G7 Over-claims; the ζ' = 1 logarithm | Taken. "Was not fixed by tuning"; errors labelled relative; the critical constant is separate from the logarithm (D2, §7). |
| A12 The round-1 Grok review lost its body | Taken. Restored from the run's output. |
| G1 §3.1 credited the procedure with UIKit's icon spring | Taken. §3.1 gives UIKit's k 184.95 and the procedure's k 171.05, with left limits 1.773 and 1.738. |
| G4 Layout springs need residual frames and `composite: 'add'` | Taken. Compositor properties: one replace track of the absolute sum. Layout and height: residual frames with `composite: 'add'`, from the engine, in place of `presence-glue.js`'s `Math.exp` integrator (D6). |
| G5 The predicate has no margin; dynamic strings accepted silently | Taken. The predicate widens to 0.6 ≤ u/ζ' ≤ 8, flagging 20 %; the menu call (0.5) stays outside. Dynamic strings inside it are refused. A dense sweep below u = 0.05 is added to stage 0 (§4.3). |
| G6 Rows X are not per-axis scaling | Taken. The axes share the v 1 stiffness and take separate initial velocities; the vector form stays future work (D1). |
| G8 The 0.206 figure's domain | Taken. It is the menu call's figure; the grid maximum at v 0 is 1.93 (§1). |
| G9 The token under LLP 1081 | Taken. `spring` until 1081 lands, `-exact-spring` after it, never both (D1). |
| G10 §4.6 and stage 0 differ | Taken. §4.6 splits stage 0 tasks from items not needed. |
| G11 6·10⁻⁸ against 6.4·10⁻⁸; the step counts are not printed | Taken. Now 6.4·10⁻⁸; `fit.mjs` prints the 11-, 12- and 13-step counts. |

### Round 3 (r3 → r4)

Astra (A) and Grok (G), round 3; both NEEDS REWORK, on narrower gaps.
r4 took every finding. Round 3 was the last round of the first approval.
Round 4, under Charlie's extension, reviewed these changes.

| Finding | Disposition |
|---|---|
| A1, G1 The step size is zero at zero distance; release velocity is unknown at parse | Taken. The scale is S = max(\|Δ\|, \|v₀\|/ω, 10⁻³ units) per axis. The parse-time bound covers authored springs; at run time an over-bound track is lowered at the bound and leaves the browser band, and is never refused (D6). |
| A2 Accumulated components defeat a parse-time bound | Taken. Run-time lowering is bounded as above, and at most 64 components live per property, the oldest two folding into an exact restart (D5, D6). |
| A3 The delay is still f32 | Taken. Tag 9 carries the delay as f64 too, and the row's delay field must be 0 (D6). |
| A4, G2 A warned literal has no run-time path, and a successful build has no warning channel | Taken. The compiler rewrites an accepted in-band literal to its resolved physical spelling; stage 2 adds warnings on successful builds; Q1's alternative is to refuse (§4.3). |
| A5 `scale` retargets in UIKit compose by matrix concatenation | Taken. `scale` components multiply, provisionally; stage 0 measures rendered transform retargets (D5). |
| A6, G3 A residual `height` cannot be negative | Taken. `height` plays on the absolute replace track (D6). |
| A7 Thinning to 64 stops ruins fast paint springs | Taken. Stops are placed by error to 10⁻² of the move, and a paint spring that needs more than 64 is refused on every host (D6). |
| A8, G5 The counterexample's input was rounded | Taken. The full input appears in D3 and §8 beside both answers. |
| A9 Condition 2 of the linearity argument was unnecessary | Taken. The conditions are stated as sufficient, and the quirk is covered by the summed derivative (D5). |
| A10 The getter is not UIKit's end for clamped overdamping | Taken. The idiom table and Q3 say the end is the animation duration (rows X: 0.9235 s against 1.0). |
| G4 Paint retargets were specified twice | Taken. Paint keeps no component list and interrupts as CSS does (D5); §8 adds the case. |
| G6 `ln` is not pinned | Taken. LLP 1003 §7 is amended to add `ln` from libm 0.2.16; `sqrt` stays the IEEE intrinsic (D2). |
| G7 Twelve steps miss the closed form as ζ → 1 | Taken. The 10⁻⁵ claim is limited to ζ' ≤ 0.999999, and the closed form is the definition (D2). |

### Round 4 (r4 → r5)

Astra (A) and Grok (G), round 4; both NEEDS REWORK. Taken, unless a finding
says otherwise.

| Finding | Disposition |
|---|---|
| A1, G4 The sampler's tolerance and the parity band used different normalisations; the quirk's scale exceeded the move; per-component spacing does not bound the sum | Taken. One grid per track on the envelope of the summed track. S is the move Σ\|Δᵢ\| (velocity only at zero distance). The new parity cases use \|browser − engine\| ≤ 10⁻³·S with S in the fixture, and the existing case keeps its absolute check. A negative-bounce midpoint regression is added (D6, §8.4). |
| A2, G2 The 65-component fold had no defined trajectory | Taken. Oldest by begin time, then insertion. Combined offset and derivative, with the product rule for `scale`; the newer component's k, c, m and end; textbook branch. A §8.4 fixture pins the fold (D5). |
| A3, G1 `scale` at 0 is undefined; the composition list mixed rules | Taken, and measured. Rows R show `scale` multiplies (to 3.3·10⁻⁵) and `rotate` adds (to 0.003°); a target of 0 presents 0 from the retarget on. D5 now lists addition (translate, height, layout, rotate), multiplication (scale) and replacement (opacity), with the zero and negative cases defined. Stage 0's transform task is done and removed. |
| G3 Q1's figures | Taken. 1,916 of 9,474 (20 %), and the refuse-literals alternative refuses Signal's reply-icon spelling (Q1). |
| G5 The run-time over-bound path; M = 0; the critical step | Taken. Component ends and the final snap stay exact and only open gaps grow; M = 0 axes are skipped; the critical step is the largest h found by bisection; §8.4 states the assertions (D6). |
| G6 Precedence item 3; LLP 1007 §3's velocity sentence | Taken. Item 3 distinguishes UIKit components (0) from restart forms (carried); the 1007 amendment limits its sentence to restart forms (D5, header). |
| G7 Undefined A and B; the `easing.rs` citation; the refusal's probe spelling; the literal path | Taken. A and B are `spring.rs`'s property-unit coefficients. `easing.rs:334` is cited, and only the terminal pair is used. The refusal quotes probe numbers only for a measured call. The literal path is `StyleProps::set_dynamic` (D6, §4.3). |
| G8 Completion, the 10 s snap, delays on retarget | Taken. Timed components are exempt from the 10 s snap; completion and `clock settle` come at the latest end; each new component waits the delay (D3, §8.4). |
| G9 The parse-time frame count | Taken. It is computed on the normalised curve: Δ = 1, the authored v, no floor (D6). |
| G10 Status line | Taken. |
| G11 The 60 s crossing | Taken. b ≈ 0.9446 at d 3 s; 11.146 s at b 0.9 and d 1 s (D3). |

### Round 5 (r5 → r6)

Astra (A) and Grok (G), round 5; both NEEDS REWORK. All taken.

| Finding | Disposition |
|---|---|
| A1 A `scale` release toward a target of 0 cannot be a factor | Taken. Released `scale` components are additive residuals in scale units; the presented scale is target × ∏factors + Σresiduals; folds combine like kinds only; §8.4 adds the 0.0188-at-50 ms release (D5). |
| A2 Delayed starts break the curvature bound | Taken. A delayed component holds its starting residual, as UIKit's backwards fill does; the grid breaks at every component start; §8.4 adds the case (D3, D6). |
| A3, G2 Clamping of `height` and opacity on the web, and what parity can read | Taken. Displayed values are clamped on every host and the engine's are not; the web grid adds frames at clamp crossings; parity compares clamped values (D6, §8.4). |
| G1 §8.3's comparator | Taken. 10⁻³·S with S in the fixture (§8.3). |
| G3 The frame bound is reachable within 60 s | Taken. The sizing sentence is gone; `spring(1s, bounce 0.9, velocity 20000)` is the parse-time refusal example (D3, §8.5). |
| G4 The `easing.rs` lines | Taken. `:334` for past-the-end, `:340` for equal inputs (D6). |
| G5 The `scale` envelope and the fold's displacement | Taken. The product's second derivative is written out, with per-factor bounds; the folded displacement is f₁·f₂ − 1; the post-fold sample is at 1/240 s (D5, D6, §8.4). |
| G6 Which rules rows R measured | Taken. `center`, `scale`, `rotate` and opacity; `height` and layout follow `center` (D5). |
| G7 The zero-distance release's assertions | Taken (§8.4). |
| G8 Stale numbers | Taken. The f32 delayed-cut times; 1.965 at ζ 0.01; the round-3 heading; the status line. |
