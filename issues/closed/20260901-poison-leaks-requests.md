# A poisoned runner clears commands but not in-flight requests

**Status:** Closed
**Resolution:** Poisoning now clears requests, pending work, commands, and other effects from the refused commit.
**Systems:** Runner
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1005 §6 (poison), `issues/20260901-mutation-forget-on-rollback.md`, `issues/20260901-fulfill-drops-failed-ticket.md`

Mutation `Later` answers are `enqueue`d after a successful settle and *before* `update()` (`runner/src/runner.rs`). `update` poisons on instance or kernel failure and clears `commands` but does not clear `requests` or `pending`.

Hosts drain `take_requests()` on error batches. A duplicate `each` key (the remaining poison path) can therefore emit `request` ops for a commit that never applied, while commands are suppressed. `take_store_writes` is the same shape: store ops ride the error batch after a published settle and a poisoned tree.

Fix: on poison, `requests.clear()`, drop `pending`, `sync_pending_flags`, matching `commands.clear()`. Optionally restore the store checkpoint taken at the start of `run_action`/`fulfill` when `update` fails, so a poisoned runner persists nothing.
