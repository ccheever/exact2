# Code review: caps' wasm32 check of the crates the web apps link, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `ae47c3752ee01b2334681854ca7e5d64ed1a59ca3a7e5993fb48c5dc540add91`), shared by both reviewers, each blind to the other's review; HEAD `468dec75e1`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 4 BLOCKER, 3 SHOULD-FIX
- **Disposition:**
  Taken in the round 1 commit:
  - Astra 1 (BLOCKER), Grok 1 (the trigger missed .cargo/config.toml, build scripts' non-Rust inputs and include_str! files): a target directory that already holds this check always runs it, and Cargo's fingerprints decide what changed, include_str! and build-script inputs included. A warm run with nothing to do takes about a second. A cold directory runs it on a Rust, Cargo, toolchain or .cargo/ change, or on any non-test file inside a checked crate's directory (kernel/tables/schema.json, host/web/index.html). An include_str! input outside the crates (docs/recipes) waits, on a cold directory, for the next check or the lane's; the module comment says so.
  - Astra 2, Grok 2 (renames and quoted names): `git diff --no-renames -z` and `ls-files -z`.
  - Astra 3 (BLOCKER), Grok 4 (absolute paths, Windows separators, an empty set passing): paths are workspace-relative with backslashes normalised, and crates come from workspace members. A set without exact-web, or a named leaf that is gone, fails.
  - Astra 4 (BLOCKER), Grok 5 (`.error` ignored): every command throws on a spawn error or ENOBUFS and on an unexpected status, and its message is kept. cargo tree and cargo check run with `--offline --locked`, so caps never rewrites Cargo.lock or waits on the network.
  - Astra 5 (BLOCKER: not every violation reported; locationless errors missed; advice not reproducible): `--keep-going`; every diagnostic, deduplicated, at its file or not, minus Cargo's per-crate summaries. A failure with no diagnostic reports stderr's tail. The rerun command names the crates that failed. The cfg advice shows only for E0425, E0433, E0412 and E0432.
  - Astra 6, Grok 3 (wasm leaves outside the apps' graph): exact-markdown-editor, exact-web-js-motion and textflow-web (`--bin`) are added, about a second together. The GPU leaves (wgpu, about 2m40s cold) and the app crates (about 2 minutes cold) are left to the lane, as the comment says.
  - Astra 7 (shared target directory): assertOwnTarget is applied before compiling.
  Verified: reintroducing G2's break fails caps with the E0425 at its line and `Rerun: … -p contract`. A warm caps run takes about 4 s.

Blocking findings remain.

1. **BLOCKER — [scripts/wasm-check.mjs:11](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:11): Relevant compilation inputs can bypass the gate.** The filter excludes `.cargo/config.toml`, `kernel/tables/schema.json`, `plan/tables/format.json`, and non-Rust `include_str!` inputs. For example, deleting `contract/cli/src/vocab/card.md` breaks compilation but produces “no Rust changed.” Include configuration and build/compiler inputs, with a conservative fallback when dependencies are unknown.

2. **SHOULD-FIX — [scripts/wasm-check.mjs:19](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:19): Renames and quoted filenames lose triggers.** With rename detection, `--name-only` reports the destination: renaming a required `.rs` file to `.txt` can hide its deletion. Git also quotes some filenames, defeating the suffix match. Use NUL-delimited output and either disable rename detection or inspect both names. Ordinary `.rs` deletions are covered.

3. **BLOCKER — [scripts/wasm-check.mjs:28](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:28): Discovery can silently select nothing.** The manifest/tree patterns assume Unix paths. Windows paths fail discovery; additionally, a Unix checkout beneath an ancestor named `apps` or `vendor` has every local crate filtered out at line 33. Both paths return successful “no web app crates.” Normalize workspace-relative paths, identify workspace members through metadata, and reject unexpectedly empty discovery.

4. **BLOCKER — [scripts/wasm-check.mjs:49](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:49): Subprocess errors can be treated as success.** Every invocation checks `status` without checking `.error`. Existing caps code explicitly documents that `ENOBUFS` can accompany status zero and truncated output. A truncated tree can select incomplete coverage; a truncated check can report success. Require both `!result.error` and status zero, and preserve the error message.

5. **BLOCKER — [scripts/wasm-check.mjs:47](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:47): It does not report every violation.** Cargo lacks `--keep-going`, and line 53 discards diagnostics after the first six. The regex also misses locationless compiler errors and Cargo/build-script failures; these can misleadingly become “0 errors,” or disappear when another diagnostic matches. Continue independent compilation, retain all failures, and report infrastructure failures distinctly. The unconditional cfg advice and `<crate>` placeholder do not provide a reproducible command.

6. **SHOULD-FIX — [scripts/wasm-check.mjs:30](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:30): The graph omits shipped wasm targets.** `exact-web-js-motion`, `exact-markdown-editor`, and `exact-gpu` are built separately from `apps/*/web`; `--lib` also omits the shipped `textflow-web` binary. Their wasm-only code can break while caps passes. Account for these targets explicitly. `EXACT_WEB_LINK=all` selects generated registrations, not Cargo features: capability implementations are covered, but excluding app crates leaves their generated entry and exported macro expansions unchecked.

7. **SHOULD-FIX — [scripts/wasm-check.mjs:47](/tmp/rv-dt-wck1-astra/scripts/wasm-check.mjs:47): Compilation bypasses target-directory ownership checks.** An inherited `CARGO_TARGET_DIR` or symlinked `target/` can point into another checkout. The repository already rejects this through `assertOwnTarget` because shared targets have produced stale generated inputs. Apply that guard before compiling.

The selected graph **would catch G2**: `contract` is a normal dependency of `caltrain-web`, and the offending function belongs to its library. Missing Cargo and offline dependency-resolution failures ordinarily fail closed, though startup diagnostics lose `.error` details. The fake caps repositories remain unaffected because they lack the new script. Both subprocess opt-outs are isolated, and the synchronous in-process `try/finally` restores the environment correctly.

The reported 50 seconds fits the individual 60-second limit but leaves only 15 seconds for the rest of the 65-second gate. Total cost remains unverified, and subprocess waits are unbounded. Static review only; no files changed, builds, Cargo commands, or tests run.
astra exit 0
