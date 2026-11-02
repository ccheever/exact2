# `contract rust` emits structs Rust will not compile

**Status:** Closed
**Systems:** `contract rust` and the module Rust printer (`contract/cli/src/rust.rs`, `contract/cli/src/module_rust.rs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A legal Contract shape can make `contract rust` print a struct a data crate cannot `include!`. The generator still returns success. Two fields that fold to one Rust name are emitted twice, so the `ContractValue` arms would not round-trip even if the struct parsed.

**Cause.** The struct name is the Contract shape name with no sanitizing (`pub struct {name}` in `rust` and the same spelling in `module_rust`). Contract reserves sixteen words (`contract/syntax/src/parser/names.rs`); `type` is not one of them, and it is a Rust keyword. Field names go through `field_name`, which maps `-` and `.` to `_` and escapes Rust keywords, and does not check that two fields land on one identifier.

**Reproduction.** `contract rust` on the first program prints `pub struct type`. On the second it prints `pub struct Row` with two `pub a_b: f64` fields.

```
shape type
  value: number
component App
  state row = type(value=1)
  view
    text "x"
```

```
shape Row
  a-b: number
  a_b: number
  type: number
component App
  state row = Row(a-b=1, a_b=2, type=3)
  view
    text "x"
```

**Fix.** Sanitize type names with the same rules as `field_name`, and prefix a result that is empty or starts with a digit. After mapping, refuse the plan when two fields of one record share a Rust name. Apply the same rule in `module_rust`.

**Acceptance.** Both programs are refused with a diagnostic that names the shape or the colliding fields, or they emit structs that `rustc` accepts and whose `from_value` distinguishes `a-b` from `a_b`. A shape whose name is already a Rust identifier is unchanged.

## Resolution

Both Rust printers escape type keywords and sanitize invalid type names. They reject fields or distinct shape names that map to the same Rust identifier within a generated scope.

## Verification

All three Rust shape regressions passed, including both printers refusing `A-B`/`A_B` collisions within one scope and allowing the mapped name in separate modules. The generated struct for `shape type` also compiled with `rustc --crate-type lib`, linked against the runner; ordinary shape generation remains covered by the existing tests.
