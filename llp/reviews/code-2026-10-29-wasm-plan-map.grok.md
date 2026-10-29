# Code review: a wasm development build writes its plan's source map (main red, quick review) (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree at `793756ad3b`.
- **Method:** one brief (sha256 `ff5fd2c0188ab41aa3c80df1d54849f1e0e9103c8754dd0f09a60b7e617c8626`), shared by both reviewers, blind to the other review. The author (Claude, iosfix2) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** clean.

## Brief

Quick review (mechanical fix under the main-red standing permission) of ONE commit, `git show HEAD` in this worktree (x2/wasm-map on origin/main). Read-only: no checkout, restore, stash, reset or edits.

What it does: host/web/build.mjs --wasm now writes app.plan.map.json beside app.plan in a DEVELOPMENT build, copying the bake's web-wasm32-unknown-unknown.plan.map.json from EXACT_BAKE_OUTPUT only when the map's `digest` equals sha256 of the plan bytes the build just wrote; never in a production build (EXACT_UPDATE_TRUST=production). The JS target already does the same (host/web-js/build.mjs ~785). Without it, the agent (scripts/agent.mjs mapLocator -> the dist's app.plan and its map) attached no source map to any node of a wasm dist. A new slow-lane test (host/web/wasm-plan-map.test.mjs, EXACT_SLOW_TESTS=1) builds apps/library-gate --wasm and checks the map's digest against the dist's app.plan.

Evidence from the author: library-gate built with the old and the new build.mjs gives the same app.plan sha256 (5c8cb05b...); only the new one has app.plan.map.json; the slow test passes.

Check:
- the map is written only in development, only when its digest matches, into the stage (a fresh directory per build, so no stale map survives), and is published with the stage;
- app.plan bytes and every other output are unchanged;
- `--bake` (bakeOnly) still works (planBytes from the bake; the map from the same bake);
- the map becomes a public file card in .exact-build.json as the JS target's does; whether that matters;
- the test is sound and not vacuous; the 1500-line cap on build.mjs;
- nothing else changed.
Give findings as P1/P2/P3 with file:line, then VERDICT: CLEAN or VERDICT: CHANGES.

## Review

No findings.

The commit is `793756ad` on `origin/main` (`0d8d0d15`), and it changes only `host/web/build.mjs` and `host/web/wasm-plan-map.test.mjs`. A development wasm build copies `web-wasm32-unknown-unknown.plan.map.json` from `EXACT_BAKE_OUTPUT` into the fresh stage as `app.plan.map.json` when `EXACT_UPDATE_TRUST` is not `production` and the map's `digest` is the sha256 of the `app.plan` bytes just written (`host/web/build.mjs:198`, `host/web/build.mjs:245`, `host/web/build.mjs:250`). That digest is the compiler's lowercase hex of those same bytes (`contract/cli/src/map.rs:19`). The stage is a new directory each build (`host/web/build.mjs:147`) and `replaceBuild` publishes the whole directory (`host/web/build.mjs:523`), so a later production build leaves no map behind. `app.plan` is written before the copy and is not written again. Assets, `exact.json`, and the `.exact-build.json` cards are built from the plan bytes and the public allowlist, which does not name this file.

`--bake` still reads `planBytes` from that same bake output (`host/web/build.mjs:231`). Deploy sets `EXACT_UPDATE_TRUST=production` for both `--bake` and a game `--wasm` (`scripts/deploy.mjs:647`). The map stays off the public file cards: `publicFileCards` follows `PUBLIC_FILES` (`host/web/serve.mjs:24`, `host/web/serve.mjs:508`), while the JS target lists every non-dot file and deploy then drops `*.plan.map.json`. The agent does not need the card. `mapLocator` opens `<dist>/app.plan.map.json` on disk (`scripts/agent.mjs:743`, `scripts/agent-inspect.mjs:85`), and the dev server reads that same path. `host/web/build.mjs` is 529 lines by the source cap's count, under 1,500.

The slow test is not vacuous. It forces a development build of `apps/library-gate` into a fresh dist and fails if the build fails, the map is missing, the digest differs, or no node names a component (`host/web/wasm-plan-map.test.mjs:21`). Library-gate's contract is real components (`PostCard` and the rest). It does not also assert the production skip. That skip is the same `!production` guard.

VERDICT: CLEAN
