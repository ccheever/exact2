# Rolling back a mutation assignment still forgets the in-flight ticket

**Status:** Closed
**Resolution:** Mutation ticket forgetting now commits only after settlement succeeds and rolls back atomically on refusal.
**Systems:** Runner
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1016 D5 (a forgotten ticket is not unsent on the wire; the runner's pending set is a different fact)

An assignment to a mutation slot calls `forget` *before* `settle` (`runner/src/runner.rs`). The comment says the forget is intentional because nothing was undone on the wire. That confuses wire policy (D5: do not unsend) with runner bookkeeping.

If settle then refuses, slot writes and commands roll back but the previous ticket is already gone from `pending`. Rollback restores `pending_mut`, not `self.pending`. `pending(mutation)` can then be true while `pending()` / `has_pending()` / `fulfill` see no ticket. `clock settle` will not wait; a later reply is `dropped: no such request`. The assignment never committed, so the previous request is still what the view means.

Fix: checkpoint `self.pending` with the slots. Apply `forget` only after settle succeeds, or restore `pending` on `Err`. Keep the journal/wire behaviour: do not pull an already-taken `RequestOut` back from the host.
