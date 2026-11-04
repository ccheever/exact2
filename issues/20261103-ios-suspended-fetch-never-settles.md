# A fetch outstanding at iOS suspension can strand later module answers

**Status:** Open
**Systems:** JavaScript data executor, Apple host, iOS lifecycle
**Author:** Codex (Lane A4, reported by Charlie Cheever)
**Date:** 2026-11-03
**Related:** LLP 1097; LLP 1027.003.000 §13; js/src/turns.rs

The Snapback chat demo at `~/projects/snapback-chat-exact/app`, running on an
iPhone on 2026-11-03, stopped answering `inbox`, `progress`, and `synced` after
suspension with a sync fetch in flight. Moving the UI on so the host forgets
the call can expose the same failure. Every later answer chained behind that
round failed until the app was relaunched:

```text
exact: request 186 (inbox) failed and is no longer pending: it keeps its last value: the answer is pending on nothing: it awaits a promise that no fetch or storage step in flight in this module will settle
```

The same diagnostic repeated for `progress` and `synced`. The app retained
its previous values, and further syncs could not recover.

To reproduce, start a chat sync, suspend the iOS app while its fetch is
outstanding, then resume and request inbox or progress again. A delayed
`/sync` response makes the window easier to reach. Also exercise navigation
that discards the originating resource while another resource awaits its
shared round promise. The physical-device failure is reported field evidence;
Lane A4's adapter regression injects an unresolved fetch and does not reproduce
the iOS lifecycle itself.

`js/src/lib.rs` describes module-wide liveness: a pending answer may await
another call only while fetch or storage work remains outstanding. Modules
have no timers or ambient clock. `Module::forget_calls` in `js/src/turns.rs`
is intended to reject forgotten fetches and drain their continuations;
`Module::wake` notices when the module has no outstanding work. The observed
failure is consistent with a fetch losing its host ticket without settling
its JavaScript promise. Trace ticket ownership and completion across iOS
suspension/resume and forgotten calls; the exact host path is not yet isolated.

Expected: every outstanding fetch resolves or rejects, including when its
originating answer is forgotten or the app resumes. Continuations release
shared queues so future calls can issue fresh host work without relaunch.

The Snapback adapter now abandons stale rounds when a later call supplies
Contract time. This bounds the adapter's wait; it does not repair host fetch
settlement or cover other modules with shared promise chains.
