# The update check holds the store mutex across the whole fetch

**Status:** Closed
**Resolution:** fixed by 3519b794
**Systems:** Apple host, Linux host, Delivery
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11, `host/apple/src/update.rs`, `host/linux/src/update.rs`

`run_check` on both hosts locks `Client` for the entire download: head, then every missing file, then the entry write. Status is answered from a snapshot so the runner does not wait. `activate` uses `try_lock` and returns the same empty answer as "nothing is staged" (`host/apple/src/update.rs`; `host/linux/src/update.rs`). `deliveryActivate` during a check logs "nothing is staged" on Linux while the banner still shows `staged`.

Anything that needs the client lock on the main thread waits on the network: Apple `exact_update_select` / `selected_plan` (a new session's `exact_boot` during a check), Linux `selected_plan`. The comment that main-thread store calls hold the lock for microseconds is false for the check's duration.

Drop the lock during `fetch`, re-lock to write the entry. Distinct activate returns for "check in flight" vs "nothing staged", or retry once the check thread drops the lock.


**Resolution (2026-09-04):** The client snapshots a check's inputs, downloads
and verifies whole entries without holding the host mutex, and then rechecks
admission against the live record while committing the result. Activation and
boot state changed during the download survive; an older completed download
cannot roll back a newer accepted sequence. Both hosts now wait only for local
store operations when activating, so a concurrent check cannot report an
existing staged bundle as absent. Apple's inverted snapshot/client lock order
is removed.

**Verification:** Apple and Linux regressions stall both the head and a missing
plan response, then select, boot a Caltrain session, activate, mark first pixel,
and read status before releasing the fetch. A client regression rejects an
older downloaded result after a newer check and activation without changing the
record. The update suite and full Apple/Linux tests pass; focused clippy passes.
Rebuilt apps pass Linux and macOS smoke. Real HTTP-stalled app drives answer
state, station selection, typing, and the changed tree before the update responds
(22.2 ms Linux, 72.9 ms macOS).
