# Cross-browser conformance has never been calibrated for most apps

**Status:** Open
**Systems:** host/web-js/conform.mjs, host/web-js/conformance/known-firefox.json, host/web-js/conformance/known-webkit.json, scripts/async.mjs
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

The async lane's first full run (M5 mini, main at 931f9fb7d) failed the Firefox step 566 times and the WebKit step 156 times. Chrome is the oracle; the browsers are exactly Playwright 1.63.0's (firefox-1543, webkit-2359), so this is not a version mismatch.

**Why so many.** The Firefox and WebKit steps were added on 2026-10-02 (6a0354e41). The lane last ran on 2026-09-24/25, so they have never had a full run. `known-firefox.json` names differences for 5 apps (caltrain, interaction-gallery, fieldnotes, markdown, realworld). The lane compares 24 apps plus the synthetic fixtures.

**What the failures are.**
- **Most: layout drift that `line-height: normal` permits.** Firefox's rows are 24 px where Chrome's are 22, and the difference accumulates down a list (synthetic-router, synthetic-fields, carousel, markdown-stress, duo-lab, native-fixture, …). These belong in the known lists as the existing entries do.
- **`resources.viewport.pointer`: Chrome `fine`, Firefox and WebKit `coarse`** (weatherlight and others). Headless Firefox and WebKit report a coarse pointer under Playwright. Either launch them with a fine pointer or name it as known.
- **Tree differences that may be real**, each worth a look before it is named known:
  - Firefox, carousel `wheel strip 0 700`: rows in a different order after the wheel;
  - WebKit, video-player `clock +60000`: `0:10` / `-0:00` against `0:00` / `-0:10` (playback time);
  - WebKit, synthetic-startend `tap say`: a virtualized window 400 rows apart (`line-389` against `line-789`).

**To do.** Calibrate each app: name the permitted layout differences, fix or name the pointer, and inspect every tree or state difference before naming it known. Logs: `~/exact2-verify/exact2-async/target/async/931f9fb7de91/conform-{firefox,webkit}.log` on the mini.
