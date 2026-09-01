# A parse-time store write does not re-answer store-reading resources

**Status:** Closed
**Resolution:** Store revisions now propagate across parse, resource, and mutation answers so dependent resources re-answer.
**Systems:** Runner
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1018 D4, LLP 1005 §8, QUEUE.md (the remember() stale-until-refresh line)

Settlement reuses a resource whenever `s.args == args && !forced` (`runner/src/runner.rs`). `store_readers[i] = true` is recorded when `query` consults the store and is never consulted as a dirty bit.

`fulfill_inner` runs `parse(&mut self.store, …)` then `settle(false)`. A login `parse` that `store.set`s a token therefore leaves a `remember()`-shaped resource on its previous answer until an action says `refresh` or its argument expressions change.

QUEUE already names this. Weird Castle dodges it (the menu's opening refreshes `accounts`; the mutation slot shadows `remembered`). The general rule — a parse-time store write marks store-reading resources dirty — is still missing.

Fix: after a store write in `parse`/`answer` (or whenever `Store::writes` grew), mark every `store_readers[i]` resource forced for that settle pass — the same path as `refresh_next`. A deps table (LLP 1005 §8) is the larger form; a boolean dirty set is enough for v1.
