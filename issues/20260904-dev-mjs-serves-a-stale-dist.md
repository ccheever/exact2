# dev.mjs serves a stale dist across --app switches

**Status:** Open
**Systems:** Web host, Dev loop
**Severity:** P2
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/web/dev.mjs:43-46

Startup only builds when dist/app.wasm is missing and never checks the baked app matches the resolved one. Observed: dev.mjs --app weird-castle after a caltrain build serves caltrain's wasm/plan while the resident compiler watches weird-castle's contract (dist/exact.json names whichever app built last). Fix: record the baked app in dist at build time and rebuild when it differs from the resolved --app.
