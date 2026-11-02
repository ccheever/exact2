# Assigning an enum state erases its vocabulary

**Status:** Closed
**Systems:** Contract type checker (`contract/types/src/actions.rs`, `contract/types/src/lib.rs`), lowering (`contract/lower/src/lib.rs`)
**Severity:** P1
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A `state` declared `enum<…>` accepts a later assignment of any string, including a value outside the enum. After that assignment the plan stores the slot as `string`, so a runtime membership check has nothing left to enforce. A write of a legal member does the same.

**Cause.** The initializer is checked with `accepts_expr`, which tests a string literal against the members (`contract/types/src/enums.rs`). An assignment infers the right-hand side — `Expr::Str` is `Ty::String` (`contract/types/src/lib.rs`, the `infer` arm) — and stores `slots[si].unify(&t)` (`contract/types/src/actions.rs`, the `Stmt::Assign` arm). `Ty::unify` joins `enum<…>` with `string` into `string`. Lowering emits `root_types.slots[i]`. Props, record fields and `fn` arguments use `can_unify` or `accepts_expr` and keep the declared type.

**Reproduction.** `contract build` accepts this. The plan's string pool contains `string` and does not contain `enum`.

```
component App
  state mode: enum<"on", "off"> = "on"
  action go
    mode = "sideways"
  view
    text mode
```

A write of `"off"`, or `mode = other` where `other` is an unannotated string state, drops `enum` the same way.

**Fix.** When the slot type contains an enum, admit the right-hand side with `accepts_expr` and leave the slot type as declared. Use `unify` only while the slot is still unknown.

**Acceptance.** `mode = "sideways"` is `type-assign`. `mode = "off"` compiles and the plan slot stays `enum<"on", "off">`. An unannotated `state mode = "on"` may still widen when a later write requires it.

## Resolution

State assignments containing enums validate literal membership without widening the declared type. Non-enum state inference remains unchanged.

## Verification

`enum_assignments_preserve_declared_membership` passed for enum states, optional enums and enum lists, including invalid literals and assignment from an unconstrained string state.
