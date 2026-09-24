# A deep view aborts the compiler before SiteTooDeep

**Status:** Closed
**Resolution:** Parser refuses more than 256 nested view sites with syntax-view-depth before recursive compiler passes; CLI regression covers 256 accepted and 400/800 refused without abort.
**Systems:** Contract compiler
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1006

Expressions are capped, so a deep expression is a refusal. View nodes are not. Plan validation is iterative and refuses depth 256 (`plan/src/lib.rs`, `MAX_SITE_DEPTH`) because realization recurses once per level. Parse, view checking, and lower walk the same tree recursively first (`contract/syntax/src/parser.rs` `node`, `contract/lower/src/lib.rs` `nodes`).

Reproduced with `target/debug/contract`. A view of 400 nested `column`s and a `text` is refused: `lower-invalid-plan` / `SiteTooDeep { table: "nodes", row: 256 }`. The same file with 800 columns aborts: `thread 'main' has overflowed its stack` (exit 134). The plan check that would refuse it never runs.

Count depth while parsing or lowering and refuse at the plan's limit before the stack grows with the tree. Done when 800 nested columns is the same typed `SiteTooDeep` as 400, and a view of 256 still lowers.
