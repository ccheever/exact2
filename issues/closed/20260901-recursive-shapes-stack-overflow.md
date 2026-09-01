# Recursive shapes overflow the compiler stack

**Status:** Closed
**Resolution:** Direct and indirect recursive shape graphs now reject before recursive plan allocation.
**Systems:** Contract compiler
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1006

The type pass accepts self- and mutually recursive shape graphs
(`contract/types/src/lib.rs:693-708`). Lowering recursively lowers every field
before caching the record id (`contract/lower/src/lib.rs:604-616`), so a shape
such as `Link { next: option<Link> }` overflows the process stack and aborts
with exit 134 instead of returning a diagnostic.

Choose and enforce the language rule: either reject recursive shapes with a
stable typed error, or allocate/cache a record identity before lowering its
fields and prove the plan value representation supports cycles. Add direct and
mutual recursion fixtures; compilation must never abort.
