# Code review: caps' wasm32 check of the crates the web apps link, round 2 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `34f344fea29da9b49e22413f89410c7c1269cc3ebca132307b80e4c2a4389fea`), shared by both reviewers, each blind to the other's review; HEAD `b2b96ab693`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks; 1 SHOULD-FIX, 1 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Taken post-round, in their own commit:
  - Astra 1 (a separate Cargo build directory): CARGO_BUILD_BUILD_DIR is read for the warm test, and assertOwnTarget guards it too. A config file's build-dir is unstable Cargo's, as the code notes.
  - Astra 2 (siblings matched as warm): warm is exact-web's own `exact-web-<16 hex>` fingerprint under wasm32's debug profile.
  - Astra 3 (a build script's cause dropped; earlier results lost when a later check could not run): Cargo's own `error:` keeps its block, the cause beneath it, up to the next diagnostic. Each check runs on its own, and one that could not run is reported beside the others.
  - Astra 4, Grok 1 (the rerun omitted a failed binary): there is a rerun for each failed check. The library check names the crates that failed, build scripts' included; the binary's rerun is its own `-p exact-textflow --bin textflow-web`.
  - Astra 5, Grok 2 (warm untested; discovery repeated; not in the lane): discovery can be injected, so one test runs cargo tree and the rest use a fixture. New tests cover warm on fixtures (siblings and other profiles are not warm), warm with only docs changed, the report on a fixture with an E0425, a build script's failure and a binary's error, and a check that could not run. wasm-check is added to scripts/async.mjs's script tests.
  Verified: G2's break reintroduced still fails caps with `Rerun: … --lib -p contract`. A warm caps run takes 4-9 s.

1. **SHOULD-FIX** — `scripts/wasm-check.mjs:103`
   The printed rerun is always `cargo check --target wasm32-unknown-unknown --lib -p …`. A failure in `textflow-web` is reported (the diagnostic lines are kept), but the command checks `exact-textflow`'s library. That library can be clean while `textflow/src/bin/textflow-web.rs` is the unit that failed, so the suggested command can pass. Include `--bin textflow-web` when that unit failed.

2. **NIT** — `scripts/wasm-check.test.mjs:18`
   Every case passes `hot: false`, so the warm branch (fingerprints present, run even when nothing Rust changed) is never asserted. The cold cases each call `wasmCrates`, which is a full `cargo metadata` plus `cargo tree`, seven times inside a 60s timeout. The file is not one of the six blocking checks. One discovery, then a pure filter check, and one `hot: true, dry: true` case with an empty change list, would lock the branch this gate actually runs.

The rest holds.

Round 1 is closed. Renames use `git diff --no-renames -z` and untracked files use `ls-files -z`. Crate paths are workspace-relative with backslashes normalized, an empty app list or a missing `exact-web` or leaf throws, and `assertOwnTarget` runs before `cargo check`. `run` throws on `error` (including `ENOBUFS` with status 0). Status 101 is accepted only for the two `cargo check` calls, so a compile failure is parsed and a metadata or tree failure still throws. `--offline --locked` on `cargo tree` and `cargo check` means a lock that does not match the manifests, or a crate that is not already downloaded, fails the push immediately and does not rewrite `Cargo.lock` or wait on the network. `--keep-going` is on the library check. The diagnostic regex keeps file-positioned and locationless `error` / `error[E####]` lines and drops Cargo's `could not compile` / `aborting` / `build failed` summaries. The cfg note is limited to E0425, E0433, E0412, and E0432. `exact-markdown-editor`, `exact-web-js-motion`, and `textflow-web` are in the set.

GPU leaves and app crates stay with the lane, and that is the right cut. `webGpuArtifacts` builds an app's GPU crate as its own wasm under `--profile web`; app crates sit under `apps/` and are filtered out of this graph. The stated cold costs (about two minutes for the app crates, about 2m40s for wgpu) cannot sit inside a 65s gate. The async lane's web build and `conform` are what compile those.

`include_str!` inputs outside a checked crate directory are a cold-only skip, and that is sound. On a warm tree, Cargo's fingerprints still rebuild when those files change. On a cold tree, `docs/recipes`, `contract/lib/*.contract`, and `docs/contract-grammar.md` do not by themselves start this check. The ones compiled into a library are also compiled by the blocking host `cargo check --workspace --all-targets`, and a missing or non-UTF-8 include fails there. `docs/recipes` is included from the `contract` binary (`main.rs`), which this wasm check does not build.

Warm means `target/wasm32-unknown-unknown/debug/.fingerprint/exact-web-*` exists, and then this check always runs. That matches `cargo check`'s dev profile, whose output directory is `debug`. A `--profile web` or `--release` build writes elsewhere, so the lane's wasm build does not count as warm; the next Rust change still runs this check once, writes the debug fingerprints, and later pushes stay warm. A stale `exact-web-*` directory, or one left by another tool in this checkout's target, only forces `cargo check` to run. Cargo's own fingerprints decide what is out of date, so a stale directory does not turn a broken crate into a pass. `exact-web-js-motion-*` also matches the prefix, which only makes the check run sooner.

A cold directory runs when any `.rs`, manifest, lock, build script, toolchain file, or `.cargo/` path changed, or any non-test file inside a checked crate directory (`kernel/tables/schema.json`, `host/web/index.html`). It skips a docs-only or scripts-only push, and an outside `include_str!` as above. An app-crate or GPU `.rs` change does start the cold library check once, which does not compile those crates. That is the "any Rust change" rule, and it happens once per target directory.

`assertOwnTarget` comes in through `import` of `scripts/app.mjs`. Caps loads that module with Bun's `require`, the same way it already loads `help-parity.mjs`, and the top-level work is `cargoOnPath()` plus `HERMES_LEAN_SYS_OFFLINE=1`, which `.cargo/config.toml` already forces. `wasm()` is the last step in `runCaps`, so the PATH repair does not change the earlier metadata call. `EXACT_CAPS_WASM=0` returns before discovery and compile; the two real-checkout spawns (`help-parity.test.mjs`, `windows.test.mjs`) set it, and the throwaway caps fixtures have no `scripts/wasm-check.mjs`. `dry` returns after discovery and before `cargo check`, and caps never passes it.

A warm caps run at about 4s fits the 65s gate. The cold compile, about a minute, is the first Rust push in a target directory that has no debug wasm fingerprints yet.
grok exit 0
