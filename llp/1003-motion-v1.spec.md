# LLP 1003: Motion v1 — what `exact-motion` is, as built

**Type:** Spec
**Status:** Draft
**Systems:** Motion, Kernel, Wire
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-28
**Implementer:** Claude (Fable 5), landing 2026-08-28 (this document transcribes the landing)
**Related:** LLP 1002 (the decision), LLP 1001 (kernel v1), CSS Transitions Level 1, CSS Easing Functions Level 1/2

## Summary

`motion/` is LLP 1002 built: CSS `transition` semantics over the four
compositor properties, one spring, and a seekable clock, in 1,461 lines with
`libm` as the only dependency. The kernel depends on it for the `transition`
row's type; it depends on nothing. Where this document and the code disagree,
the code and its tests are the authority and this document is stale.

## 1. The `transition` row (`kernel/tables/schema.json`, bit 82)

Codec `transitions`; type `exact_motion::Transitions` (re-exported as
`exact_kernel::Transitions`); no default (empty = CSS's initial value, which
starts nothing). Wire grammar (`kernel/src/wire/codec.rs`): count u8 (≤ 8),
then per declaration property u8 (0 `all`, 1 `translate`, 2 `scale`, 3
`rotate`, 4 `opacity`), duration f32 seconds, delay f32 seconds, easing u8:
0 `linear`, 1 `ease`, 2 `ease-in`, 3 `ease-out`, 4 `ease-in-out`,
5 `cubic-bezier` + 4×f32, 6 `steps` + u16 count + u8 position (0 `jump-start`,
1 `jump-end`, 2 `jump-none`, 3 `jump-both`), 7 `spring` + f32 stiffness,
damping, mass, 8 `linear()` + u8 count (≤ 64) + count × (f32 input, f32
output). Unknown discriminants are typed decode rejections. The row is
validated at decode (`DecodeError::InvalidTransition`) and at structured apply
(`ApplyError::InvalidTransition`); a frame or batch carrying an invalid row
applies nothing (`kernel/tests/motion.rs::an_invalid_row_is_refused_on_both_ingress_paths_and_applies_nothing`).

The animatable rows were renamed to CSS's individual transform properties in
the same change: `translate` (vec2, bit 66), `scale` (bit 67), `rotate`
(degrees, bit 68), joining `opacity` (bit 48). Bits 69–81 shifted down by one;
the schema digest moved with them.

## 2. Properties and values (`property.rs`)

`Property::{Translate, Scale, Rotate, Opacity}` with CSS names, wire order,
component counts (2, 1, 1, 1), and CSS initial values (`0 0`, `1`, `0`, `1`).
`Value { x, y }` is every value; scalars use `x`. Interpolation is componentwise
linear (CSS "by computed value"). Comparison is exact.

## 3. Easing (`easing.rs`)

`Easing::{Linear, Ease, EaseIn, EaseOut, EaseInOut, CubicBezier, Steps,
PiecewiseLinear}`. The keywords are their CSS Béziers bit for bit
(`keywords_are_their_cubic_beziers`). `cubic-bezier` is solved by Newton with a
bisection fallback to 1e-7, browser-style; `x1`/`x2` must lie in `[0, 1]`.
`steps()` follows CSS Easing §2.3.2's procedure for all four jump positions.
`linear()` interpolates between stops and holds the end stops outside them.
Outputs are pinned to browser values at the midpoint (`ease` ≈ 0.8024,
`ease-in` ≈ 0.3153, `ease-out` ≈ 0.6847) and at the ends, and an overshooting
Bézier leaves `[0, 1]` as CSS requires. `validate()` names every refusal.

## 4. Spring (`spring.rs`) — the declared deviation

`SpringConfig { stiffness, damping, mass }` (default 100 / 10 / 1). `sample
(displacement, velocity, t)` is the closed-form damped oscillator in the
under-, critically-, and over-damped regimes. Rest is `|x| < 1e-3 ∧ |v| < 1e-3`
in the property's own units; `settle_time` is the first at-rest sample on the
240 Hz grid, capped at 10 s, after which the engine snaps to the target.
`keyframes(config, from, velocity, to)` is the web lowering: absolute-value
frames on the same grid, the last exactly `to` at offset 1. A spring's
`transition-duration` must be `0` (`SpringDeclaresDuration`); its delay must be
nonnegative.

## 5. Transition semantics (`transition.rs`, `engine.rs`)

CSS Transitions §3 as the engine runs it, per `(node, property)`:

- **First seen → no transition.** A property the engine has never observed
  takes its value (no before-change style).
- **Matching.** `Transitions::matching` is the last declaration covering the
  property (`all` or its name); it starts only if its combined duration
  (`max(duration, 0) + delay`) is positive — a spring always starts.
- **Fresh change.** Start from the current presentation value with
  reversing-adjusted start = that value and shortening factor 1.
- **Change to the running end value → nothing.**
- **Interruption.** Sample the running transition at `now`; if that equals the
  new value, cancel and jump; otherwise start a new transition from the sampled
  value. An easing reversal (new value equals the running transition's
  reversing-adjusted start) uses the §3.2 factor `clamp(|p·f + (1 − f)|, 0, 1)`
  to scale duration and any negative delay, and records the old end value as the
  new reversing-adjusted start. A spring interruption inherits the sampled
  velocity (or the change's `velocity`, when a gesture supplied one).
- **Delay** holds the start value; a negative delay starts partway.

`Engine::hold` writes through with no transition (a drag under
`transition: none`). `Engine::advance(now)` is monotonic and samples every
running transition at `now` — a seek. `Engine::frame()` drains the changed
`(node, property)` set in key order. `settle_time()` is the last running end
time (a spring's from `settle_time` on the grid). `remove(node)` forgets a node.

## 6. The seam (`kernel/src/motion.rs`)

`Kernel::motion_sync(&receipt) -> MotionSync { removed, transitions, changes }`
restates a commit: destroyed keys to forget; for every created or touched node,
its `transition` row and its four targets (`motion::targets`). `MotionSync::
apply(&mut engine)` feeds them in that order — row before targets, so a commit
that changes both animates under the after-change row, as CSS does. Nodes are
`motion_node(key) = generation << 32 | index`. The engine deduplicates unchanged
targets, so syncing every touched node's four rows costs nothing when they did
not move. The kernel's own targets never change under animation; only the
engine's presentation does (`a_style_change_transitions_under_the_row_and_the_clock_seeks`).

## 7. Determinism (`math.rs`)

`exp`, `sin`, `cos` route through `libm` 0.2.16 (pinned; a test reads
`Cargo.lock`); `sqrt` is IEEE correctly rounded. All arithmetic is f64 with no
FMA contraction. The same inputs give the same bits on every target
(`the_same_inputs_give_the_same_bits`), and the value at `t` does not depend on
the path to `t` (`the_clock_is_a_seek`). Cross-architecture identity is a claim
about the same crate on the same `libm`; it is not a claim about the browser,
which is held to a 1e-3 band by fixture, not to bits.

## 8. Errors

`EasingError`, `SpringError`, `TransitionError`, `EngineError` — each a closed
enum naming the exact refusal; `Copy + Eq`, so the kernel embeds them in its
own `DecodeError`/`ApplyError` without translation.

## 9. Not in v1 (and where each is declared)

Layout-affecting transitions; `@keyframes`/`animation`; gesture recognition,
hit-testing, and arbitration (the platform's — LLP 1002 D4); a Core Animation
executor (LLP 1002 §4); reduced-motion policy in the engine (producer emits
`transition: none` — LLP 1002 §4). The browser-driven parity harness owed
with the web host (LLP 1002 §5) landed there the same day: LLP 1007 §5, a
recorded fixture of 105 browser samples the engine matches within 1e-3, held
by `host/web/tests/parity.rs`. Everything deleted from the ported crate is listed
in LLP 1002 D5 and in `rules/NOT-DOING.md` §Motion.

## 10. Checks that hold this

`cargo test --workspace`: 113 tests — `motion/tests/easing.rs` (7),
`motion/tests/spring.rs` (6), `motion/tests/engine.rs` (17), the `math` pin
tests (2), and `kernel/tests/motion.rs` (7) for the row's round trip through
EXWF bytes, the seam, refusal on both ingress paths with nothing applied, slot
reuse, and clearing the row. `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo fmt --all -- --check`, `cargo build --workspace --target
wasm32-unknown-unknown`, and `node scripts/caps.mjs` all green on 2026-08-28.
