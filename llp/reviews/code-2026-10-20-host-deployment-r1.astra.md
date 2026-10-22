# Quick review: one host deployment target (520efb1700), Astra

**RIGHT WITH CHANGES**

1. **BLOCKING — the selected target still diverges.** [host/apple/build.mjs:678](/tmp/rv-hostenv-astra/host/apple/build.mjs:678) uses an app’s macOS minimum, but [scripts/linked-plan.mjs:33](/tmp/rv-hostenv-astra/scripts/linked-plan.mjs:33) constructs the bake tool’s environment independently. Repro: build a linkable JS app with `host.macos.minimumOS="15.0"` twice: main Cargo gets 15.0; the bake tool gets 14.0, retaining the rebuild trigger. The filesystem helper likewise inherits 15.0 inside that bake but defaults to 14.0 outside it ([scripts/filesystem.mjs:50](/tmp/rv-hostenv-astra/scripts/filesystem.mjs:50)). Conversely, exporting `MACOSX_DEPLOYMENT_TARGET=15.0` makes iOS/device/tvOS main builds use 14.0 while the tool preserves 15.0. Resolve and propagate one environment per build.

2. **BLOCKING — the Linux development entrypoint bypasses normalization.** [scripts/build-linux.mjs:16](/tmp/rv-hostenv-astra/scripts/build-linux.mjs:16) passes raw `process.env`. Repro on a Mac with the variable unset: alternate `bun scripts/build-linux.mjs weatherlight` with its Apple build. The Linux host’s `host-dev` Cargo invocation remains unset while the bake tool uses 14.0 in the shared target directory.

3. **BLOCKING — valid warm web receipts are rejected on Macs.** Web builds now record 14.0 in [scripts/app.mjs:1150](/tmp/rv-hostenv-astra/scripts/app.mjs:1150). However, [game/proof.mjs:1072](/tmp/rv-hostenv-astra/game/proof.mjs:1072) passes raw `process.env` to reuse validation; [host/web-js/module.mjs:422](/tmp/rv-hostenv-astra/host/web-js/module.mjs:422) compares that unset value against 14.0. Repro: produce a warm `gpu-dev` web build, then start a proof eligible to reuse it with the variable unset: reuse fails and rebuilds. Also, direct `buildBake` callers can still record null while Cargo receives 14.0: normalization happens only inside `buildCommand` ([scripts/app.mjs:886](/tmp/rv-hostenv-astra/scripts/app.mjs:886)), leaving the receipt’s environment unchanged.

4. **NIT — “every cargo call” overstates coverage.** Additional raw calls remain: deploy’s compiler ([scripts/deploy.mjs:197](/tmp/rv-hostenv-astra/scripts/deploy.mjs:197)), deployment metadata (:166, :524), Apple test compiler calls ([host/apple/build.mjs:1355](/tmp/rv-hostenv-astra/host/apple/build.mjs:1355), :1374), and error diagnosis ([scripts/contract-diagnosis.mjs:29](/tmp/rv-hostenv-astra/scripts/contract-diagnosis.mjs:29)). Repro: invoke those paths with the variable unset. Metadata itself does not compile anything; its mismatch alone causes no rebuild.

With an unset environment and default minimum, the ordinary iOS/device/tvOS main, bake-tool, filesystem-helper, metadata, Hermes and GPU paths agree. Deploy’s Mac/Linux-target bakes also use 14.0.

Explicit macOS product minimums remain authoritative. Actual Linux/wasm targets gain no macOS deployment floor; a Linux-host executable targeting Darwin can change. Non-Mac machines receive no default. No new import cycle or initialization-order issue found.

The permitted test passed: **1 test, 4 assertions**. Reproduction paths above were traced, not built or launched.
