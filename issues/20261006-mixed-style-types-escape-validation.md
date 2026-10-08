# Reject incompatible computed style branches before lowering

**Status:** Open
**Systems:** Contract type checking, native/web style parity
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** QUEUE.md mixed numeric/string styles; contract/lower/src/values.rs

A conditional style binding can evade the new string-versus-number validation when its arms have incompatible types.

This current compiler accepts:

```contract
component App
  state on = true
  state label = "0.75"
  view
    view opacity=(on ? 0.5 : label)
```

Executed `contract build` on the equivalent fixture (including a nonnumeric label): it produced a valid plan rather than a type diagnostic. `contract/lower/src/expr.rs` falls back to `Ty::Unknown` when the conditional arms do not unify. `values.rs:755–770` rejects a whole string expression on a number-only row but accepts Unknown; its branch walk validates literal strings, not the type of each computed arm. A numeric-looking string can be applied by the browser while native number-only binding code does not read it.

Retain scoped type information for each branch or refuse incompatible arm unification when checking the row. Respect match-bound locals and let expressions; do not merely reject all Unknown expressions if valid existing authoring depends on them.

Acceptance: a numeric/string opacity conditional and analogous number-only style rows produce a precise compile-time diagnostic identifying the incompatible arm; all-numeric branches compile, and rows deliberately admitting both forms keep working. Exercise variables, ternaries, match and let, on both targets.
