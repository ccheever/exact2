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

## Literal match follow-up

**Reproduction:** Confirmed by Codex on 2026-11-02 against `b4714391738e6b4332988a16b9c124e812966022`.

The enum-preserving fix rejects a legal assignment when the value is a `match` whose arms are declared members. `accepts_expr` recursively checks literal ternary arms, lists and `some`, but has no corresponding `Expr::Match` case (`contract/types/src/enums.rs:31-37`). Inference joins the string literal arms as `string`, and the fallback rejects that widened type.

```contract
component App
  state mode: enum<"on", "off"> = "on"
  state opt = some(true)
  action go
    mode = match opt { case some(x) => "on", case none => "off" }
  view
    button mode press=go
```

The existing `target/debug/contract build <fixture> --json` exits 1 with `type-assign`: `mode` is `enum<"on", "off">`, cannot assign `string`. The retained fixture is `/Users/ccheever/.cache/exact-review/exact-release-20261102/contract-probes/enum-state-match.contract`.

Check each match arm in its lexical scope before admitting the assignment, preserving the declared enum type. Verify that this program compiles, a match arm returning `"sideways"` is rejected, and a bound `some` value with an unconstrained string type still cannot enter the enum. The resolution and verification above continue to describe the cases already fixed.

## Match resolution

Each match arm now checks enum membership in its lexical scope. The `some` binding is visible only in that arm; the `none` arm keeps the enclosing scope. Legal members preserve the declared enum slot, and invalid literal diagnostics name the rejected member.

All nine focused enum tests passed, including literal arms, enum bindings, nested matches, and both directions of shadowing an unconstrained string with an enum. The original audit fixture now builds. The quick semantics comparison passed against the upstream delivery base (`d165f8959fc6cc597a9985a62fe9895ed0fc5cc8`).
