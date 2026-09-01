# Smoke under `EXACT_APP_DIR` still requires Caltrain nodes

**Status:** Closed
**Resolution:** Smoke now discovers the resolved app and gates Caltrain-specific fixtures while retaining generic host coverage.
**Systems:** Agent API, Tooling
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** QUEUE.md (the smoke under EXACT_APP_DIR), LLP 1012

`open({ host })` goes through `resolveApp(undefined)`. With `EXACT_APP_DIR` set that boots the foreign app. Step 1 then requires `caltrain-main`, `station-name`, `board-north`, `board-south`, `change-station` and the Mountain View copy. Failures are collected; the run stays red.

The GPU deck step requires `deck-toggle` / `card-*` with no presence guard. The last step always runs `apps/caltrain/app.test.contract` from this repo root, not `$EXACT_APP_DIR/app.test.contract`.

QUEUE already names this. `check()` collecting failures does not save it: the process still exits 1. “Smoke web with an app dir” does not mean “the steps that apply plus the app’s tests.”

Fix: guard each Caltrain-shaped step on the nodes it needs (the way the iframe step used to), and point the last step at the app dir’s `app.test.contract` when `EXACT_APP_DIR` is set. Or declare the full smoke caltrain-only and point app repos at `agent.mjs --test`.
