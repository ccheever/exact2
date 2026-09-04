# dev.mjs serves a stale dist across --app switches

**Status:** Closed
**Resolution:** Web builds write a private completion marker last, binding the requested manifest identity and every public artifact's digest and size. Dev startup rebuilds unless that marker, the named plan, the envelope, and the exact public inventory agree; deploy uses the same public allowlist and never publishes the marker.
**Systems:** Web host, Dev loop
**Severity:** P2
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/web/dev.mjs:43-46

Startup only builds when dist/app.wasm is missing and never checks the baked app matches the resolved one. Observed: dev.mjs --app weird-castle after a caltrain build serves caltrain's wasm/plan while the resident compiler watches weird-castle's contract (dist/exact.json names whichever app built last). Fix: record the baked app in dist at build time and rebuild when it differs from the resolved --app.
