# Metrics: the wasm core of realworld and caltrain fails to build in the async lane's captured tree

**Status:** Open
**Systems:** scripts/metrics.mjs (`--long`), scripts/app.mjs (captured snapshot), host/web/build.mjs `--wasm`
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P3
**Related:** the async lane on the mini, checks 342a6eb2 through 5f5f5aed (`target/async/<sha>/metrics.log`)

Since 342a6eb2 every tier 1 check's `metrics.mjs --long` reports `wasm core: realworld  FAILED` (six checks), and caltrain twice, with only a stack as the reason (`at contractLast (…/exact-capture-…/source/exact2-async/scripts/app.mjs:853:16) / at …/host/web/build.mjs:115:22 / Bun v1.4.2`). Its run is from the captured source tree under `$TMPDIR/exact-capture-*`. The same builds succeed by hand in a worktree on the mini at 784eac0dd: `bun host/web/build.mjs realworld-web --wasm`, plain and with the names environment metrics uses (`EXACT_WEB_NAMES=1`, `CARGO_TARGET_DIR=target/metrics-names`). So the difference is the captured tree or the lane's environment, not the code.

`failure()` in scripts/metrics.mjs kept the last three lines of output, which for a thrown error are its stack frames and Bun's banner. It now drops those (the triage commit that files this issue), so the next async check's row names the error. Read that row, then fix the capture (a file the snapshot omits) or the build.
