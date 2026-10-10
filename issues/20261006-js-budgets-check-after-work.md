# Enforce JS evaluation limits before allocating or scanning

**Status:** Open
**Systems:** web JS runtime, runner parity
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P1
**Related:** LLP 1090; LLP 1041; host/web-js/budget.js

The JS budget helpers can perform work far beyond the promised limit before reporting a trap. This defeats the responsiveness and allocation bound even when the eventual diagnostic looks correct.

In `host/web-js/budget.js:101`, list `slice` allocates before `step`; `x_split` at line 124 allocates all pieces before checking the 65,536-step ceiling. `x_includes` and `x_indexOf` scan before accounting for the remaining budget. `x_join` at line 133 builds the entire string before checking its 64 MiB UTF-8 limit. The native VM checks split's piece count before allocation (`runner/src/vm.rs`); native join stops while appending (`runner/src/stdlib.rs`).

Executed the current, unchanged budget module in Chrome. `x_join(Array(600).fill(""), "x".repeat(1 << 20), 123)` threw `RangeError: Invalid string length`, with no Exact trap kind or PC. Each input is within the value/string limits; their separator expansion is about 599 MiB. Instrumenting the real `String.prototype.split` for `x_split(",".repeat(65536), ",", 0, 456)` showed 65,537 pieces allocated before `IterationLimit`.

Check remaining steps before list copying and during scans. Count split pieces with bounded work, and preflight or incrementally bound string expansion before building the result. Apply the same review to other string builders.

Acceptance: these fixtures produce the native trap kind and PC in Chrome, Firefox and WebKit; instrumentation shows no allocation or scan proportional to the rejected result beyond the admitted budget. Preserve existing UTF-8 and surrogate semantics.
