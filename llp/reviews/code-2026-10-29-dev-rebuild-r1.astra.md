# Code review: the dev loop's -dev bake driver rebuild, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a6ca0a6e32`.
- **Method:** one brief (sha256 `9949f107597a5c9c81944b5ba70e6f87a716a0a12460410659d1ecbd435b6d48`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/web/build.mjs:407](host/web/build.mjs:407) — The renderer still runs Cargo without either variable, and it **is on the dev-loop path**: startup/rebuild invokes this builder with `--wasm` or `--bake`; both can reach the renderer. Weatherlight’s renderer uses `exact_js_bake`, so an absent backend-origin file remains perpetually dirty. Merge `bakeWatchEnv(app)` into `renderEnv` and correct the commit’s “outside the dev loop” classification.

2. [host/web/dev-bin-run.test.mjs:40](host/web/dev-bin-run.test.mjs:40) — The test mutates Sparkline’s real directory and shares a fixed target across invocations. Two runs can interleave their named/unnamed builds, invalidating freshness assertions. Worse, if another run creates `deck/` first, `mkdirSync` throws and this run’s `finally` deletes the other run’s directory. Use an owned fixture with app-local outputs, or serialize the entire sequence and remove only directories successfully created by this run.

3. [scripts/async.mjs:141](scripts/async.mjs:141) — Condition (1)’s inventory is incomplete. Additional callers without the variables include the ignored-test and Clippy workspace invocations here, [document.test.mjs:72](host/web/tests/document.test.mjs:72), and [zero-viewport.test.mjs:142](host/web/tests/zero-viewport.test.mjs:142). Report these remaining callers in the commit; they need not be fixed within this scope.

**Non-blocking**

1. The extraction preserves `buildBake`’s computations and refresh timing. `devBinRun` selects the correct workspace/profile for all four cases: repository app, outside app with injected profiles, generated game workspace, and generic `exact-dev`. I found no remaining consumer requiring this driver’s release binary path; the dev menus reload plans/connections.

2. [host/web/dev-bin-run.test.mjs:25](host/web/dev-bin-run.test.mjs:25) — Cargo’s `fresh` field is an appropriate signal, and the unset-env control demonstrates the original failure. However, filtering to Sparkline packages proves only those units stayed fresh, not “nothing compiled.” Remove that filter for the warm assertion or narrow the claim. The declared cold-build cost runs in the async glue lane, outside the blocking checks.

3. The commit’s evidence needs qualification: [tables.test.mjs:153](host/web/tests/tables.test.mjs:153) allows **300 seconds**, not 60. Also, Caltrain declares a portable Rust module and returns before this driver branch at [dev.mjs:528](host/web/dev.mjs:528); label its numbers as direct driver benchmarks. The timing and output-equality measurements were not independently reproduced, per your restrictions.
