# iOS device lane has no dev-loop wiring

**Status:** Closed
**Resolution:** `build.mjs --device --run` now passes a caller-set HTTP(S) `EXACT_DEV_PLAN` through `devicectl` as launch environment, and refuses local Mac paths that a phone cannot read. The remote envelope supplies the generation's assets, so no host filesystem path crosses to the device.
**Systems:** Apple host, Dev loop
**Severity:** P3
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/apple/build.mjs:430-434,464-466

build.mjs --ios --run passes SIMCTL_CHILD_EXACT_DEV_PLAN/ASSETS, but the --device --run path launches via devicectl with no EXACT_DEV_PLAN, EXACT_ASSETS, or dev-URL env, so the on-phone app boots baked with no dev connection (only a manually entered URL via the watchPlan http path works). Note: there is no host/ios dir; iOS is host/apple --ios sharing ExactKit Updates/Session with macOS. On-device verification was not possible here (no phone attached). Fix: pass the dev env/URL through the devicectl launch.
