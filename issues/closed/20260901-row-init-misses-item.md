# P4c row-slot initializers cannot read the row item

**Status:** Closed
**Resolution:** Row-owned state initializers now compile and evaluate in the owning item scope.
**Systems:** Contract compiler, Runner
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017.000 P4c

LLP 1017.000 P4c and the runner agree: a new row evaluates slot init *in that row's frames*, so an initializer may read the item (`runner/src/instance.rs`). The compiler does not. Child state is subst'd (a prop `station` becomes the enclosing `each` var) then compiled with `component_scope` only — no `Item` binding.

`state hot = station.id` on a row child is `type-unknown-name` (or it captures a root name). The corpus (`contract/corpus/instance.contract`) only initializes with literals (`n = 0`, `hot = false`), so this never failed a test. Derives used in the view work, because they lower inside the `each`.

Fix: compile owned-slot init with the owning `each`'s item in scope, matching `u.eval(s.init, &inner)`. Fixture: `StationRow` with `state label = station.name` under `each s in stations`.
