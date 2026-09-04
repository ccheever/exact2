# default exact deploy dry run crashes on unreachable origin

**Status:** Open
**Systems:** Delivery, Deploy
**Severity:** P2
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** scripts/deploy.mjs classify; scripts/origin.mjs HttpsOrigin.get

node scripts/deploy.mjs caltrain --json (manifest https origin https://caltrain.exact.invalid, no --origin flag) exits 1 with a bare 'TypeError: fetch failed ... ENOTFOUND' and no table, because HttpsOrigin.get throws on DNS/network failure and classify's origin-row loop does not catch. A dry run must classify, not crash: catch per-origin fetch failures and render them as rows in the table.
