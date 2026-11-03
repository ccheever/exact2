# An enum mutation rejects a write of its own member

**Status:** Closed
**Systems:** Contract type checker (`contract/types/src/actions.rs`, `contract/types/src/checks.rs`, `contract/types/src/enums.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A mutation declared with an enum shape cannot be set from a written member. A member and a non-member fail with the same `type-assign`. An action parameter, a prop and a record field of that enum accept the member.

**Cause.** The assignment arm looks the name up as a mutation and admits it with `can_unify` (`contract/types/src/actions.rs`, the mutation branch of `Stmt::Assign`). A string literal infers as `string`. `can_unify` allows `string` to accept an enum, and does not allow an enum to accept `string`, and it never reads the literal. `accepts_expr` does: `enum<…>` against `Expr::Str` tests membership, and `option<T>` against `some(…)` recurses (`contract/types/src/enums.rs`).

**Reproduction.** `contract build` rejects both programs with `` `session` is `option<enum<"on", "off">>`, cannot assign `option<string>` ``.

```
component App
  mutation session as shape enum<"on", "off">
  action go
    session = some("on")
  view
    text "x"
```

`session = some("sideways")` produces the same diagnostic.

**Fix.** Admit mutation assignments with `accepts_expr`, as a prop ascription does, and leave the declared shape unchanged.

**Acceptance.** `session = some("on")` compiles. `session = some("sideways")` is `type-assign` and names the member.

## Resolution

Mutation assignments validate literal enum membership. Lowering wraps the existing mutation type in its option slot, preserving the required type identity. Rejected literals are named in the diagnostic.

## Verification

`enum_assignments_preserve_declared_membership` passed: a declared member compiles, and a non-member fails with `type-assign` naming the rejected value.
