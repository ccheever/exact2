# Baked store dependency is not transitive

**Status:** Closed
**Resolution:** Bake dependency analysis now propagates store taint transitively through resource arguments.
**Systems:** Contract compiler, Runner
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1005 §8, LLP 1018

Bake marks only a resource whose own query directly reads the store
(`runner/src/runner.rs:1421-1428`). If resource B reads resource A while
evaluating its arguments, A's store dependence is not propagated through the
resource read (`runner/src/runner.rs:1380-1395`).

The compiler omits baked initials only for directly marked resources. In an
`A(store) -> B(A)` graph, A is recomputed on device but B retains its
build-time value; boot then associates that stale B value with A's new
device-time arguments until another change happens.

Track store dependence transitively through resource reads during bake, or
compute the dependency closure before deciding which initials are portable.
Add a two-resource fixture whose build and device stores differ.
