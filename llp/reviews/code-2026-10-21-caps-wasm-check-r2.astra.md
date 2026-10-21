# Code review: caps' wasm32 check of the crates the web apps link, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `34f344fea29da9b49e22413f89410c7c1269cc3ebca132307b80e4c2a4389fea`), shared by both reviewers, each blind to the other's review; HEAD `b2b96ab693`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 5 SHOULD-FIX
- **Disposition:**
  Neither reviewer found anything blocking. Taken post-round, in their own commit:
  - Astra 1 (a separate Cargo build directory): CARGO_BUILD_BUILD_DIR is read for the warm test, and assertOwnTarget guards it too. A config file's build-dir is unstable Cargo's, as the code notes.
  - Astra 2 (siblings matched as warm): warm is exact-web's own `exact-web-<16 hex>` fingerprint under wasm32's debug profile.
  - Astra 3 (a build script's cause dropped; earlier results lost when a later check could not run): Cargo's own `error:` keeps its block, the cause beneath it, up to the next diagnostic. Each check runs on its own, and one that could not run is reported beside the others.
  - Astra 4, Grok 1 (the rerun omitted a failed binary): there is a rerun for each failed check. The library check names the crates that failed, build scripts' included; the binary's rerun is its own `-p exact-textflow --bin textflow-web`.
  - Astra 5, Grok 2 (warm untested; discovery repeated; not in the lane): discovery can be injected, so one test runs cargo tree and the rest use a fixture. New tests cover warm on fixtures (siblings and other profiles are not warm), warm with only docs changed, the report on a fixture with an E0425, a build script's failure and a binary's error, and a check that could not run. wasm-check is added to scripts/async.mjs's script tests.
  Verified: G2's break reintroduced still fails caps with `Rerun: … --lib -p contract`. A warm caps run takes 4-9 s.

Nothing blocks the intended shared-library wasm check. Five **SHOULD-FIX** findings remain.

1. **SHOULD-FIX — [scripts/wasm-check.mjs:64](/tmp/rv-dt-wck2-astra/scripts/wasm-check.mjs:64): Separate Cargo build directories bypass warmth and ownership checks.** Cargo supports `build.build-dir` / `CARGO_BUILD_BUILD_DIR` independently of `target_directory`. With that configured, fingerprints live elsewhere, so this check can remain “cold” after successful runs; `assertOwnTarget` also misses intermediate artifacts directed into another checkout. Inspect and guard the actual build directory too, or explicitly reject this configuration. [Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html#buildbuild-dir)

2. **SHOULD-FIX — [scripts/wasm-check.mjs:70](/tmp/rv-dt-wck2-astra/scripts/wasm-check.mjs:70): Other packages falsely establish warmth.** `startsWith('exact-web-')` also matches `exact-web-js-motion-*`, `exact-web-capabilities-*`, and `exact-web-geometry-*`. Checking only the small motion leaf can therefore make a documentation-only push compile the whole cold graph. Match `exact-web-<hash>` specifically. Stale fingerprints or another tool’s build can still cause extra compilation; this heuristic establishes neither successful completion nor a cheap next run.

3. **SHOULD-FIX — [scripts/wasm-check.mjs:98](/tmp/rv-dt-wck2-astra/scripts/wasm-check.mjs:98): Build-script failure causes still disappear.** A malformed `kernel/tables/schema.json` produces an `error: failed to run custom build command` header, followed by the actual panic and parsing error. The filter retains the header but discards that explanation; because an error matched, the fallback never runs. Preserve Cargo’s failure blocks. Also retain earlier diagnostics if a later invocation throws at line 93, rather than replacing everything with “nothing was checked.” Round 1’s reporting finding is only partially closed.

4. **SHOULD-FIX — [scripts/wasm-check.mjs:103](/tmp/rv-dt-wck2-astra/scripts/wasm-check.mjs:103): The rerun command omits failed binary targets.** An error confined to `textflow/src/web.rs` fails the `textflow-web` check, but the suggested `--lib -p exact-textflow` command excludes that code and can pass. Preserve each failed invocation’s target selection and print `--bin textflow-web` when appropriate. Build-script failures also need inclusion when `failed` contains only other crates’ compiler summaries.

5. **SHOULD-FIX — [scripts/wasm-check.test.mjs:18](/tmp/rv-dt-wck2-astra/scripts/wasm-check.test.mjs:18): The central warm behavior has no regression coverage, and these tests are absent from the lane.** Every trigger case forces `hot: false`; none exercises actual fingerprint discovery, warm documentation changes, command failures, or diagnostic retention. Meanwhile, the seven trigger cases each repeat real metadata/tree discovery. Keep one graph integration check and use isolated fixtures for these decisions and failures. Add the file to the explicit script-test list in [scripts/async.mjs:114](/tmp/rv-dt-wck2-astra/scripts/async.mjs:114).

The remaining round 1 fixes look sound: NUL-delimited Git paths, Windows path normalization, nonempty discovery, subprocess-error handling, and acceptance of status 101 only for compilation. `--offline --locked` correctly rejects missing downloads or required lockfile updates; a valid, already-fetched lockfile change is allowed. [Cargo’s flag semantics](https://doc.rust-lang.org/cargo/commands/cargo-tree.html#manifest-options)

Leaving GPU/app compilation to the lane is justified by the reported multi-minute cost. The cold exception for external embedded inputs is explicit; the named recipe inputs are also compiled by native `contract`. Release/custom-profile artifacts correctly do not establish warmth for this debug check.

The `app.mjs` import performs environment/Bun setup but has no unconditional build; synchronous loading is compatible with the repo’s Bun usage. `dry` skips compilation, though it still performs discovery. Four seconds warm is reasonable, but does not prove the complete 65-second gate budget or cold-path cost.

Static review only; no files changed, Cargo commands, builds, or test suites run.
astra exit 0
