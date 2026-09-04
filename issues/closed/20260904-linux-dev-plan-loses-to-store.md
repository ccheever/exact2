# Linux EXACT_DEV_PLAN loses to the update store's selection

**Status:** Closed
**Resolution:** Linux now treats every EXACT_DEV_PLAN locator as explicit, boots a ready local dev plan immediately, and never selects or crash-counts a persisted production entry while the dev compiler is starting.
**Systems:** Linux host, Web dev loop
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030.000 §4 stage 4, LLP 1007 §6, `host/linux/src/app.rs`

1030.000 §4 stage 4: "`EXACT_PLAN` and the dev plan win over the selection, and neither is counted." The module comment at `host/linux/src/app.rs` says the same: selection only "with neither `EXACT_PLAN` nor `EXACT_DEV_PLAN`."

`explicit` is `from_url || named`, and `named` is only a file `EXACT_PLAN`. A file `EXACT_DEV_PLAN` is stored on `config.dev_plan` for the display watcher and does not set `explicit`. `select_update` then, when a store entry exists, replaces `plan` with that entry and calls `boot_started`.

Apple boots `EXACT_PLAN ?? devPlanPath` through `boot_plan` and never counts it. On Linux display, a leftover selected entry therefore boots instead of the baked/dev plan, is crash-counted, and is not displaced until the watched file's mtime moves.

Treat a set `EXACT_DEV_PLAN` like `explicit`: do not take `selected_plan`, do not call `boot_started`. Optionally load the file for the first boot, matching Apple.
