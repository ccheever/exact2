# Reject spring configurations whose derived coefficients produce non-finite samples

**Status:** Open
**Systems:** Motion, Kernel style admission, Web motion lowering
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1003 §4, motion/src/spring.rs, motion/src/parse.rs

Checking that the three physical parameters are finite does not ensure that the spring's derived coefficients are finite. `validate` admits finite positive stiffness/mass and finite nonnegative damping; `sample` then computes `alpha * alpha`, ratios and overdamped roots without guarding overflow.

Verified through the public authored shorthand parser:
```rust
Transitions::parse("opacity spring(100, 1e200, 1)") // Ok
SpringConfig { stiffness: 100., damping: 1e200, mass: 1. }
    .sample(1., 0., 0.1)
// displacement: NaN, velocity: NaN
```
The same configuration's `validate()` returns `Ok(())`. Overflow makes the roots infinite and subsequent arithmetic produces NaN. Tiny positive masses can create analogous failures.

An admitted transition can consequently publish non-finite native property samples or invalid web keyframes rather than receiving a named ingress refusal. Snapping at the ten-second duration cap does not protect frames before it.

Validate the derived quantities at admission or use a numerically stable formulation over the admitted parameter domain. The decoder and shorthand parser must agree. Preserve a clear diagnostic instead of silently treating invalid arithmetic as an ordinary spring.

Acceptance: the shown declaration is refused or samples finitely for its entire duration. Cover extreme finite damping, stiffness and positive mass, including retarget velocity and the web keyframe lowering. Existing under-, critical- and overdamped fixtures remain unchanged.
