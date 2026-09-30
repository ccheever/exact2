# `exact new` writes dependency paths one level short for an app beside the exact2 checkout

**Status:** Closed
**Resolution:** Already fixed: sibling manifest-relative paths and failed-create cleanup regressions pass on 2026-09-30 (game/new-app.test.mjs).
**Systems:** Outside apps (`game/new.mjs`, `game/new-app.test.mjs`)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1036.001

`dep()` computes each path dependency relative to the app's root directory (`pathFrom(dir, …)`, `game/new.mjs:69`), but Cargo resolves a path from the directory of the manifest that names it (`apple/Cargo.toml`, the web manifest, …).

**Reproduced:** `createApp('<checkout>/.review/field-log')` wrote `exact-logic = { path = "../../logic" }` into `apple/Cargo.toml`, one level short, and `cargo metadata` then failed ("failed to load manifest for dependency `exact-apple`"). It also left the half-written app directory behind.

The test passes only because `tmpdir()` is outside `/Users`, so every path comes out absolute (`game/new-app.test.mjs:10`).

**Fix:**
- Compute each path relative to the manifest that contains it.
- Add a test with the app as a sibling of the checkout.
- Remove the directory when creation fails.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed: Host dependencies are relative to their containing manifests, and failed creation removes the partial app; sibling-app Cargo metadata and forced-failure cleanup regressions pass.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max; reproduced by the verifier. Verification: reproduced.
