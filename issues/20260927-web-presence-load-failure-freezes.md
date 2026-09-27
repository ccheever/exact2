# If the presence module fails to load, the web holds every later batch and retries the load forever

**Status:** Open
**Systems:** Web host (`host/web/navigation.js` `presenceLoader`, `host/web/glue.js`)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1063

When an exit arrives, `presence.hold(batch)` defers the batch until `presence-glue.js` loads. If that load fails, the `.catch` clears `loading` and releases the held batches through `applyBatch` (`host/web/navigation.js:281-291`). `applyBatch` calls `hold` again (`host/web/glue.js:805`), which sees the same exit, restarts the load, and holds the batch again.

**Result:** after one failed fetch of the module (offline, a deploy that removed the old hash), every later UI update queues behind it and the load retries in a loop. The page stops updating.

**Fix:** add a terminal "unavailable" state. Apply held and later batches without presence motion (exits leave at once, layout snaps), and journal it once.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max. Verification: confirmed by reading.
