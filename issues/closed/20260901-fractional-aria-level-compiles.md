# Fractional aria level compiles then fails boot

**Status:** Closed
**Resolution:** Integer-valued props now reject fractional, nonfinite, saturating, and out-of-range values in lowering and at runtime.
**Systems:** Contract compiler, Runner
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017.000 P1a

`aria-level` is an integer property. Lowering accepts any expression typed as
number without checking a numeric literal's integral value
(`contract/lower/src/lib.rs:1155-1172`). `aria-level=1.5` therefore builds a
plan that the runner bridge rejects at boot because `fract() != 0`.

LLP 1017.000 P1a requires literal errors to be caught by the compiler. Reject
fractional and out-of-range numeric literals in lowering, retain the runtime
check for dynamic values, and cover the compile-time and dynamic refusal paths.
