# Metrics: the wasm core of realworld and caltrain fails to build in the async lane's captured tree

**Status:** Closed
**Resolution:** Not reproducible on current main: Caltrain's failure was fixed by #538; at c874a53cd the captured-tree wasm core of RealWorld and Caltrain (normal and names builds, via withAppFixture as metrics.mjs does) built 12 of 12 times across three captures with identical hashes, on Linux; the 2026-10-08 Mac lane logs are no longer reachable.
**Systems:** scripts/metrics.mjs (`--long`), scripts/app.mjs (captured snapshot), host/web/build.mjs `--wasm`
**Author:** Claude (Opus 5.5), for Charlie Cheever
**Date:** 2026-10-08
**Severity:** P3
**Related:** the async lane on the mini, checks 342a6eb2 through 5f5f5aed (`target/async/<sha>/metrics.log`)

Since 342a6eb2 every tier 1 check's `metrics.mjs --long` reports `wasm core: realworld  FAILED` (six checks), and caltrain twice, with only a stack as the reason (`at contractLast (…/exact-capture-…/source/exact2-async/scripts/app.mjs:853:16) / at …/host/web/build.mjs:115:22 / Bun v1.4.2`). Its run is from the captured source tree under `$TMPDIR/exact-capture-*`. The same builds succeed by hand in a worktree on the mini at 784eac0dd: `bun host/web/build.mjs realworld-web --wasm`, plain and with the names environment metrics uses (`EXACT_WEB_NAMES=1`, `CARGO_TARGET_DIR=target/metrics-names`). So the difference is the captured tree or the lane's environment, not the code.

`failure()` in scripts/metrics.mjs kept the last three lines of output, which for a thrown error are its stack frames and Bun's banner. It now drops those (the triage commit that files this issue), so the next async check's row names the error. Read that row, then fix the capture (a file the snapshot omits) or the build.

2026-10-14 (#538): reproduced at b171c9663 with `metrics.mjs --long` on an M5 Pro. The row read `wasm core: caltrain FAILED … compiler input has no captured source identity: …/wasm32-unknown-unknown/web/build/caltrain-gpu-<hash>/out/shaders.rs`. The JS target builds the GPU crate on stable into the same profile directory where the bake builds it on the web nightly. `unitDepInfo` took the stable `deps/caltrain_gpu.d` without checking its product's bytes. Its OUT_DIR lies outside the captured ROOT, so `nameOf` threw. A checkout named it `exact/target/…` and passed. With the fix, the same run reports all three wasm cores.

2026-10-14 (#538), what remains: realworld. #538 does not explain its lane failures. They start at 342a6eb26 (2026-10-08 03:22), before 355177011 (14:01) made the web GPU crate build on stable, and apps/realworld has no GPU crate. Realworld's wasm core does not reproduce at b171c9663: it built in the local captured run before the fix too. The lane's 2026-10-08 realworld logs stay on the mini, and #538 did not read its newer rows. After #538 merges, read the lane's next `metrics.mjs --long` rows. Close this issue when both wasm core rows pass; if realworld still fails, fix the error its row names. Keep it open until then: the lane files only failures its previous check did not have, so it will not report a realworld failure that continues.
