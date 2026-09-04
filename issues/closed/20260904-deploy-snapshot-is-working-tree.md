# exact deploy snapshots HEAD of the app dir, then bakes the live workspace

**Status:** Closed
**Resolution:** Deploy freezes each full Git and local Cargo repository into one content-identified tree, materializes only that tree outside the live repositories and behind a Git-discovery boundary, and refuses source links or Cargo paths that escape it. Tracked, untracked, and ignored inputs require the same explicit dirty snapshot; only named build-output roots are excluded. Bake, classification, signing, and publication re-execute from the captured Exact tree.
**Systems:** exact deploy, Build
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030.000 D3 item 1, LLP 1030.001, `scripts/deploy.mjs`

D3 item 1 requires an immutable snapshot — a commit or tree — baked into a run-specific directory, never the editor's mutable working state.

`snapshotOf` records `HEAD` and runs `git status --porcelain -- .` with `cwd: app.dir` (`apps/caltrain` in-repo). It never extracts that commit (`git archive` / worktree / checkout). `bake()` then runs `host/web/build.mjs` against the live workspace, which compiles kernel, plan, runner, host/web, glue, and the app.

Uncommitted changes under `kernel/`, `host/`, `contract/`, `runner/`, `glue.js` are baked and published while the table still prints a clean `HEAD` sha. `--snapshot <sha>` only checks `HEAD.startsWith(sha)`; it does not materialize that sha. `--dirty` is honest about the *app* tree and silent about everything else. For `EXACT_APP_DIR` the app repo's status similarly cannot see a dirty exact2 path-dep.

Status the repository that `bake` actually reads (workspace root, or `HEAD` + exact2 when outside), or extract the commit into the run directory and bake that. Keep `--dirty` as the named escape, covering the same tree.

## Progress

A correction now derives dirty policy, identity, and materialized bytes from the same frozen Git tree; captures deletions, ignored inputs, filtered worktree bytes, and whole local Cargo repositories (including workspace roots); relocates safe internal links; and refuses escaping manifests, targets, or links. The source stage lives outside mutable Cargo ancestors, and the captured publisher owns app resolution through publication. Focused race, workspace-inheritance, filter, absolute-path, symlink, Cargo-config, and ancestor-Git fixtures pass, as does a real Caltrain staged bake. Independent review covers the exact integrated commit.
