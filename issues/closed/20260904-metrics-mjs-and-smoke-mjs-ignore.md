# metrics.mjs and smoke.mjs ignore the outside app

**Status:** Closed
**Resolution:** Resolved app selection throughout diagnostics; private complete-source fixtures verify external edits, builds, runtime and signed deployment without changing live source.
**Progress:** The resolved app is measured and driven through a closed source capture, with private build/output/state directories and cleanup on failure. External edits, rebuilds, macOS measurements, and signed deploy smoke are verified.
**Systems:** Scripts, Dev loop
**Severity:** P3
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** scripts/metrics.mjs:42,124,163-168; scripts/smoke.mjs:21; scripts/app.mjs

metrics.mjs hardcodes -p caltrain-web for the native step and apps/caltrain/app.contract for the reload/rebuild steps; EXACT_APP_DIR only redirects the wasm step. So the 100ms edit-to-present budget is unverified for weird-castle, and a run under EXACT_APP_DIR edits caltrain's contract against weird-castle's dist. smoke.mjs has no --app selector (resolveApp() with no arg; argv[0] is the host), so the outside app is selectable only via EXACT_APP_DIR env — inconsistent with dev.mjs --app and build.mjs positional crate. Fix: thread the resolved app through both scripts.

Verified 2026-09-05 against a private capture of Weird Castle, using `EXACT_APP_DIR`
and `--app weird-castle`; its copied Cargo lock had already been refreshed, while
live source stayed untouched. Metrics changed visible text and verified the new
DOM: plan 9 ms, acceptance 300 ms (above the 100 ms budget; follow-up in QUEUE),
wasm rebuild 4.025 s. `--long` built and booted the selected macOS app and rebuilt
a touched captured host file. `--scaling` explicitly identifies its fixed Caltrain
workload. An incorrect inherited `EXACT_WEB_DIST` did not receive output.

External deploy smoke passed: signed heads, published browser boot, asset update,
concurrent publishers, retired stream, and unchanged source plus Git state. Repeat
publication compares the actual public cards, reads back the complete graph, and
permits writes only to that graph and its pointer; identical native bundles stay
current. Relocated external wasm can carry different absolute Rust source paths,
so byte reproducibility is a separate QUEUE item. Focused regressions: 47/47.
