# If the presence module fails to load, the web holds every later batch and retries the load forever

**Status:** Closed
**Resolution:** Already fixed: unavailable presence releases held and future batches and logs once; both loader regressions pass on 2026-09-30.
**Systems:** Web host (`host/web/navigation.js` `presenceLoader`, `host/web/glue.js`)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063

When an exit arrives, `presence.hold(batch)` defers the batch until `presence-glue.js` loads. If that load fails, the `.catch` clears `loading` and releases the held batches through `applyBatch` (`host/web/navigation.js:281-291`). `applyBatch` calls `hold` again (`host/web/glue.js:805`), which sees the same exit, restarts the load, and holds the batch again.

**Result:** after one failed fetch of the module (offline, a deploy that removed the old hash), every later UI update queues behind it and the load retries in a loop. The page stops updating.

**Fix:** add a terminal "unavailable" state. Apply held and later batches without presence motion (exits leave at once, layout snaps), and journal it once.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: failed presence loads release updates without motion and journal once; loader regressions and a Chrome agent drive pass. Supplemental web smoke reports Chrome keychain/encryption and paint-timing diagnostics.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max. Verification: confirmed by reading.

Fixed in `host/web/navigation.js`, with the runner's journal passed from
`host/web/glue.js`. The failed load marks presence unavailable before releasing
the queue; held and future batches apply normally, so destroys remove exits
immediately and layout takes its target box.

`host/web/tests/presence-loader.test.mjs` reproduces the failure by rejecting
the first load and replaying through `hold`: before the fix no batch applied;
after it, held and later batches apply in order, with one attempt and one
journal entry. The successful-load ordering test also passes. A built Chrome
page driven through `scripts/agent.mjs`'s carrier, with the presence script
load failed deliberately, applied three toggles of the presence fixture with
one load attempt and one `presence module: unavailable` journal entry.

Required build, test, clippy, fmt, caps and boot checks pass. The supplemental
`bun scripts/smoke.mjs web` completed its functional scenarios but reports one
failure from headless Chrome's macOS keychain access (`-25308`), unavailable
password-store encryption and `Invalid first_paint` diagnostics; no page
exception or failed functional scenario was reported.
