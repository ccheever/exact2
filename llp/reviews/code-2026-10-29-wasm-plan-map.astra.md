# Code review: a wasm development build writes its plan's source map (main red, quick review) (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `793756ad3b`.
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

No P1/P2/P3 findings.

- [build.mjs:250](/tmp/iosfix2-rv-wm-astra/host/web/build.mjs:250) gates copying on development trust and a matching plan digest. The fresh stage prevents stale maps and is published whole. Plan bytes remain untouched; `--bake` reads plan and map from the same bake output.
- **Public-card correction:** wasm’s [allowlist](/tmp/iosfix2-rv-wm-astra/host/web/serve.mjs:24) excludes `app.plan.map.json`, so it does **not** enter `.exact-build.json`, unlike JS. This is fine for the requested local-dist fix: the agent reads the adjacent map directly.
- The [test](/tmp/iosfix2-rv-wm-astra/host/web/wasm-plan-map.test.mjs:18) is meaningful: fresh dist, successful wasm build, matching digest, and component metadata. It requires `EXACT_SLOW_TESTS=1`.
- Only the two stated files changed. `build.mjs` is 528 lines, below 1,500. Syntax and diff-whitespace checks passed.

Read-only review; build/test execution and byte-for-byte artifact comparison were not rerun.

VERDICT: CLEAN
