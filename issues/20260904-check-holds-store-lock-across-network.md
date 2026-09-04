# The update check holds the store mutex across the whole fetch

**Status:** Open
**Systems:** Apple host, Linux host, Delivery
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11, `host/apple/src/update.rs`, `host/linux/src/update.rs`

`run_check` on both hosts locks `Client` for the entire download: head, then every missing file, then the entry write. Status is answered from a snapshot so the runner does not wait. `activate` uses `try_lock` and returns the same empty answer as "nothing is staged" (`host/apple/src/update.rs`; `host/linux/src/update.rs`). `deliveryActivate` during a check logs "nothing is staged" on Linux while the banner still shows `staged`.

Anything that needs the client lock on the main thread waits on the network: Apple `exact_update_select` / `selected_plan` (a new session's `exact_boot` during a check), Linux `selected_plan`. The comment that main-thread store calls hold the lock for microseconds is false for the check's duration.

Drop the lock during `fetch`, re-lock to write the entry. Distinct activate returns for "check in flight" vs "nothing staged", or retry once the check thread drops the lock.
