# exact deploy snapshots HEAD of the app dir, then bakes the live workspace

**Status:** Open
**Systems:** exact deploy, Build
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030.000 D3 item 1, LLP 1030.001, `scripts/deploy.mjs`

D3 item 1 requires an immutable snapshot — a commit or tree — baked into a run-specific directory, never the editor's mutable working state.

`snapshotOf` records `HEAD` and runs `git status --porcelain -- .` with `cwd: app.dir` (`apps/caltrain` in-repo). It never extracts that commit (`git archive` / worktree / checkout). `bake()` then runs `host/web/build.mjs` against the live workspace, which compiles kernel, plan, runner, host/web, glue, and the app.

Uncommitted changes under `kernel/`, `host/`, `contract/`, `runner/`, `glue.js` are baked and published while the table still prints a clean `HEAD` sha. `--snapshot <sha>` only checks `HEAD.startsWith(sha)`; it does not materialize that sha. `--dirty` is honest about the *app* tree and silent about everything else. For `EXACT_APP_DIR` the app repo's status similarly cannot see a dirty exact2 path-dep.

Status the repository that `bake` actually reads (workspace root, or `HEAD` + exact2 when outside), or extract the commit into the run directory and bake that. Keep `--dirty` as the named escape, covering the same tree.
