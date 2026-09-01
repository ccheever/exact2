# `fulfill` drops the ticket before parse and settle succeed

**Status:** Closed
**Resolution:** A refused asynchronous fulfillment now restores its ticket so the same answer can be retried.
**Systems:** Runner
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1016 D5 (forgotten vs failed accept)

`Runner::fulfill` removes the ticket and `sync_pending_flags()` *before* `parse` / shape / `settle`. A `DataError`, `Shape`, or settle `Cycle`/`Data` then rolls back slots/resources and the store but does not put the ticket back.

The resource is no longer pending, the value is the old one, and the host cannot retry that ticket. This is not D5 “forgotten” (args changed / assignment); it is a failed accept of a still-wanted reply.

Fix: keep the ticket in `pending` until `fulfill_inner` returns `Ok`. On `Err`, leave the request in flight (or re-insert) so `pending(x)` / `clock settle` / a retry still see it. Drop only when the commit stands or when a later newest-wins forget already replaced it.
