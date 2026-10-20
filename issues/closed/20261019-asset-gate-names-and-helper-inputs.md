# Asset gate: @2x names refused, one offender per build; unimported helper scripts are build inputs

**Status:** Closed
**Resolution:** fixed by bbd846b41: @ admitted, every bad name reported at once, unimported scripts are no driver input
**Systems:** update envelope asset names, bake receipt, js/bake sources, scripts/agent-launch.mjs inputs
**Severity:** P3
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptoms

1. `photo@2x.png` and `two words.png` under `assets/` fail the bake, and the bake names one offender per run.
2. A helper script the app never imports (`scripts/export.mjs`) makes the driver call the build stale and re-runs the bake when it changes.

## Causes

1. `safe_name` (`update/src/envelope.rs:480-498`) admits `A-Z a-z 0-9 . _ -` only, and `check_asset_names` returns at its first error (`:504-535`); the bake calls it at `bake/src/receipt.rs:248`. `scripts/deploy.mjs:782-809` is the publisher's copy, also first-failure. `@` is neither folded nor normalized by any filesystem; a space is a URL hazard and stays refused.
2. The bake's `sources()` walk captures every `ts|js|mjs|cjs|json|contract|ttf|otf` file in the app (`js/bake/src/lib.rs:886-891`) and emits `rerun-if-changed` for each; the driver's `notBuildInput` (`scripts/agent-launch.mjs:848-867`) skips unimported `.ts`/`.json` through the module graph but keeps every `.mjs`/`.js` outside `tools/`.

## Fix

Admit `@`; report every bad name in one run, in both copies. Treat an unimported `.js`/`.mjs`/`.cjs` as the driver treats an unimported `.ts`.

## Fixed (2026-10-19)

`safe_name` admits `@`, and `check_asset_names` (and the publisher's `portableAssetNames`) report every refused name at once, one per line; a space stays refused. The driver reads `.js`/`.mjs`/`.cjs` files through the module graph as it reads `.ts`: one app.ts imports is an input, one it never imports is none (`scripts/agent-build.test.mjs`). The bake's own `rerun-if-changed` capture still names every script file, so an edit to an unimported helper re-runs the bake (cheap) though the driver no longer calls the build stale.
