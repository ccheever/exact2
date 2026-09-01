# Row actions cannot read row props

**Status:** Closed
**Resolution:** Row action captures now evaluate at the exact lexical handler site across nested rows and conditional frames.
**Systems:** Contract compiler, Runner
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017.000 P4c

LLP 1017.000 P4c says row-owned initializers may read the row item and row
actions execute with its frames. Child declarations are instead lifted to
global tables during inlining, retaining only slot ownership
(`contract/syntax/src/inline.rs:149-193`). Types and lowering compile those
actions from the component/root scope, without the owning row's item or child
prop bindings.

A `Row(item=item)` child with `action choose { selected = item.id }` is refused
as `type-unknown-name`; if a root name collides, it can capture the wrong value.
The existing row-init ticket covers the same missing scope for initializers,
but not dispatched actions.

Represent the lexical environment carried by each lifted row declaration and
use it in types, lowering, and runtime dispatch. Test both initializer and
action reads, nested rows, and a colliding root name.
