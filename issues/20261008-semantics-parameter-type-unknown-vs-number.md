# The Lean semantics types three corpus parameters `unknown` where the compiler infers `number`

**Status:** Open
**Systems:** semantics (Lean typing), contract/types, contract-difftest types
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P2
**Related:** semantics/README.md; first in the async lane's be15451e..cb762285 range

The async lane's semantics step exits 1 since be15451e..cb762285. `contract-difftest types` agrees on all 580 generated programs, but three corpus programs differ in a parameter's inferred type:

- `corpus/actions/calls/inject.contract`: `inject` parameter types[1]: Lean `unknown`, Rust `number`;
- `corpus/actions/calls/prop-row.contract`: a prop call in a row, parameter types[1]: Lean `[number, string, unknown]`, Rust `[number, string, number]`;
- `corpus/components/curried-capture.contract`: a non-finite curried argument, parameter types[1]: Lean `unknown`, Rust `number`.

The only compiler change in that range under contract/types, contract/analyze or semantics is ff62431f7 ("Contract: name the web's words the app farm reached for"), which does not obviously touch parameter inference, so the cause may be elsewhere in the range (e.g. an event payload's type). Find the Rust change that started inferring `number` for these parameters and either port it to the Lean checker or, if it is wrong, revert it; then the semantics step passes.
