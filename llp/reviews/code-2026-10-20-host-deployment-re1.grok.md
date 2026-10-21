# Narrow re-check: one host deployment target (7446768dd1), Grok

**RIGHT.** The three blocking findings are fixed. Cargo calls that share a target directory agree inside an iOS, device, tvOS, macOS, and Linux-on-Mac dev build.

`scripts/build-linux.mjs` passes `cargoEnvironment` into `app.target`. `androidToolchainEnv` does the same, and both the APK cargo and `androidBuild` use that environment. Unset, they write `14.0` (`HOST_MACOS`); an exported value is kept. An iOS build uses that same choice (`host/apple/build.mjs:678`). `ios` covers `--device`, `--tvos`, and `--ios`.

`buildBake` calls `withHostDeployment` before cargo and before the receipt. `buildCommand` fills `MACOSX_DEPLOYMENT_TARGET` only when it is missing, so the recorded flag is the value cargo sees. `warmGameEnvironment` applies the same default, and `takeWarmGame` compares through it, so a warm web proof accepts a receipt of `14.0` from an unset shell.

`bakeOutside` passes the build environment to `bakeTool.options`. For an in-repo app the tool and the app share `target/`. A macOS build puts `targets.macos` on both, including a minimum above `14.0`. The filesystem helper forces `HOST_MACOS` and builds only in `target/exact-filesystem-tool`. GPU modules take the build's value in `target/apple-modules`.

A macOS minimum other than `14.0` and a later iOS or Linux dev build are different values in that shared directory. Each build is consistent on its own. Every in-tree macOS minimum is `14.0`, so the ordinary loop stays on one value. `bun test scripts/host-deployment.test.mjs`: 2 pass, 7 assertions.

1. **NIT.** `host/apple/build.mjs --test` uses its own value in `app.target`. The iOS test cargo hardcodes `14.0` (`:1320`), while a dev iOS build keeps an export. A macOS test follows `developmentBuildEnv` (`:1318`), so a higher `targets.macos` stays on the dev build alone. `--test` is outside the dev loop. Deploy's compiler and `scripts/contract-diagnosis.mjs` are unchanged; the contract crate does not build ring.
