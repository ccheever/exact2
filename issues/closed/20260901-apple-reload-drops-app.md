# Apple reload drops the running app before the new plan boots

**Status:** Closed
**Resolution:** Apple reloads now build beside the live host and reset the presenter only after candidate boot succeeds.
**Systems:** Apple host
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1023.001 §2 (keep the running app on refusal), LLP 1007 §6 (a reload is a restart from a new plan, not a blank window)

`PlanURL.swift` documents a transactional boot: a candidate replaces the running app only after hash and boot; every failure keeps the last good app. Fetch and hash failures do. Boot does not.

`Bridge::boot_plan` (`host/apple/src/abi.rs`) does `self.host.take()` — carrying state out of the live `Host` — then `Host::boot_stored_after_decode`. On `Err`, `host` stays `None` and `exact_pump` returns `"not booted"`. The old executor is left running with no runner.

On the Swift side, `PlanURL.boot`, `DevMenu.reload`, and the `EXACT_DEV_PLAN` watcher all `presenter.reset()` (every view `forget()`'d) and *then* `Exact.bootPlan`. A decode/runner error, or a truncated `app.plan` the watcher reads mid-write, leaves an empty window. `PlanURL.fetchPlan` also sets `bootedDigest` before `PlanURL.boot?`, so a failed boot of those bytes will not retry.

A successful reload has the same hole on the data plane: `boot_plan`'s `Ok` arm assigns a new `Executor`, which drops the job `Sender` of the old thread. A `loginV2` (or any request) that was out across a dev-loop save is never `fulfill`'d; the new runner does not hold the ticket. Same class as `issues/20260901-web-fetch-reload-tickets.md`.

Fix: keep the old `Host` until `Ok`; only `reset()` after a batch with `error == nil`; set `bootedDigest` after apply succeeds. On success, drop or abort the old executor's in-flight jobs the same way a forgotten ticket is dropped — do not let them land on a runner that no longer holds the numbers. The file watcher already writes through tmp+rename; the consumer still must not throw away the tree first.
