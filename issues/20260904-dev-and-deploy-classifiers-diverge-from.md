# Deploy and dev compute compatibility ids that disagree with the bake

**Status:** Open
**Systems:** Delivery, Dev loop
**Severity:** P1
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/web/dev.mjs:204,212,229; scripts/deploy.mjs:210 compatOf; contract/cli/src/main.rs:44-47; contract/cli/src/compat.rs:309,311; llp/1030.000-dev-server-as-deployer.rfc.md

Two divergences, both verified. (1) `dev.mjs` carries its own conservative classifier plus `classifyRebuild` instead of importing the deploy classifier (RFC 1030.000 owed “dev.mjs importing this classifier”); two classifiers can drift, and `dev.mjs` never publishes so divergence is silent. (2) Both script classifiers call `contract compat` with no `--target` and grants `None` (`grantCeiling: null`), while bake uses the real `TARGET` plus the data crate's grants. A rebuilt Linux binary's `OUT_DIR/compat.json` was `030646b901a2bd78a9fcdf21d86ffd8b` (`aarch64`, `grantCeiling: ""`); the CLI used by deploy returned `2f2aee5a5b0a397f5a799e9b3cc1fcd` (`aarch64`, `grantCeiling: null`). For web the second computation also says the publisher's architecture instead of `wasm32`.

Effect: deploy publishes streams under ids the built binary's baked `head_url`
never checks, and a grants-only change moves the baked cohort while both script
classifiers say it did not. `host/apple/build.mjs --embed` has the same second
computation: it supplies a target but no grants, so the `compat.json` shipped
beside the archive can disagree with the string compiled into that archive
(`host/apple/build.mjs:282-294`). Its receipt does not carry either id.

Do not make another caller reconstruct grants and targets. The binary-producing
bake must emit the compatibility document and build receipt that deploy,
`--embed`, and the dev classifier consume as their sole authority; assert that
the emitted id equals what the binary exposes. Then move both scripts onto one
classifier. Cover wasm32, macOS, iOS device and simulator, Linux, a non-empty
grant set, and an external path-dependent app.
