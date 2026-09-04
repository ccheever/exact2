# metrics.mjs and smoke.mjs ignore the outside app

**Status:** Open
**Progress:** App selection and authenticated web-build identity are threaded end to end. In-repo deploy smoke now runs in a disposable checkout, but external deploy smoke is still refused and external edit-to-present is still unmeasured.
**Systems:** Scripts, Dev loop
**Severity:** P3
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** scripts/metrics.mjs:42,124,163-168; scripts/smoke.mjs:21; scripts/app.mjs

metrics.mjs hardcodes -p caltrain-web for the native step and apps/caltrain/app.contract for the reload/rebuild steps; EXACT_APP_DIR only redirects the wasm step. So the 100ms edit-to-present budget is unverified for weird-castle, and a run under EXACT_APP_DIR edits caltrain's contract against weird-castle's dist. smoke.mjs has no --app selector (resolveApp() with no arg; argv[0] is the host), so the outside app is selectable only via EXACT_APP_DIR env — inconsistent with dev.mjs --app and build.mjs positional crate. Fix: thread the resolved app through both scripts.
