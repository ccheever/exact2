# Narrow re-check: one host deployment target (7446768dd1), Astra

**RIGHT**

No remaining BLOCKING findings or regressions found in the requested scope.

- **Linux and Android:** [build-linux.mjs:16](/tmp/rv-hostenv-astra/scripts/build-linux.mjs:16) and [androidToolchainEnv:67](/tmp/rv-hostenv-astra/scripts/agent-android.mjs:67) now pass through `cargoEnvironment`, preserving an explicit deployment target and supplying the Mac default when absent.

- **Receipts and warm reuse:** [buildBake:1235](/tmp/rv-hostenv-astra/scripts/app.mjs:1235) normalizes before preparing dependencies, invoking Cargo, or recording configuration. The recorded deployment target therefore matches Cargo’s. [warmGameEnvironment:409](/tmp/rv-hostenv-astra/host/web-js/module.mjs:409) applies the same default, resolving the unset-versus-`14.0` reuse mismatch.

- **Apple target propagation:** [bakeOutside:127](/tmp/rv-hostenv-astra/scripts/linked-plan.mjs:127) passes its build environment to `bakeTool.options(env)`, which retains the build’s selected target. [Apple build:678](/tmp/rv-hostenv-astra/host/apple/build.mjs:678) honors an exported value for iOS, device, and tvOS; macOS retains its manifest minimum, including a later minimum. Main Cargo, its host dependencies, and the bake-tool build consequently agree within each reviewed build. GPU builds receive the same selected value. The [filesystem helper:51](/tmp/rv-hostenv-astra/scripts/filesystem.mjs:51) consistently uses `HOST_MACOS` in its separate `target/exact-filesystem-tool` directory.

Linux-on-Mac also agrees with the default or exported value. Switching between builds that deliberately select different minima can still invalidate Cargo artifacts; the unintended disagreement within one build is fixed.

`bun test scripts/host-deployment.test.mjs` passed: **2 tests, 7 assertions**. Build paths were inspected, not executed; no files were edited.
