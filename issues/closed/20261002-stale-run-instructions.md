# Stale run instructions: Textflow's README sets EXACT_APP, which nothing reads, and Caltrain's test file says node and eight operations

**Status:** Closed
**Resolution:** Fixed by 28d617918.
**Systems:** apps/textflow, apps/caltrain, docs
**Severity:** P4
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-02
**Related:** apps/textflow/README.md:26, apps/caltrain/app.test.contract:1-4

Two sets of instructions point readers, and agents, at commands that don't do what they
say:

- `apps/textflow/README.md:26` says `EXACT_APP=textflow bun host/web/dev.mjs`. Nothing
  reads `EXACT_APP` (`dev.mjs` takes `--app`, and `resolveApp` without one is
  Caltrain), so the command starts Caltrain. It should be
  `bun host/web/dev.mjs --app textflow`. Its Apple lines (`build.mjs --app textflow`) work,
  because `build.mjs` takes the first bare argument as the app.
- `apps/caltrain/app.test.contract:1-4` describes the tests as "the agent API's eight
  operations" and the command as `node scripts/agent.mjs <host> --test …`. There are
  nine operations (`prefer` was added 2026-09-27, `rules/DEFERRED.md` §Agent API), and
  the tooling runs on Bun (`bun scripts/agent.mjs`). The repository doesn't need Node,
  and the Caltrain file is the first test file most readers open.

Both are one-line edits.
