# Code review: caps' wasm32 check of the crates the web apps link, round 1 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `ae47c3752ee01b2334681854ca7e5d64ed1a59ca3a7e5993fb48c5dc540add91`), shared by both reviewers, each blind to the other's review; HEAD `468dec75e1`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks; 4 SHOULD-FIX, 1 NIT
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

1. **SHOULD-FIX** `scripts/wasm-check.mjs:11`
The trigger does not match `.cargo/config.toml` or the non-Rust inputs the build scripts actually read. `kernel/build.rs` regenerates the kernel from `kernel/tables/schema.json`, and `plan/build.rs` regenerates the plan from `plan/tables/format.json`. The comment above the regex says those inputs are covered. A rustflags edit in `.cargo/config.toml`, or a schema edit that makes the wasm32 codegen fail, leaves this gate idle. `include_str!` / `include_bytes!` files that are not `.rs` have the same hole (`bake/src/lib.rs` embeds `package.json`).

2. **NIT** `scripts/wasm-check.mjs:19`
`git diff --name-only` lists a rename's new path only (the move into `issues/closed/` in `e54ee0ea4d` appears once). Renaming `foo.rs` to a non-Rust name does not trigger. Names are split on newlines, while the rest of caps uses `git ls-files -z`, so a quoted path misses the regex.

3. **SHOULD-FIX** `scripts/wasm-check.mjs:47`
`--lib` skips wasm artifacts the web build compiles on their own. `exact-textflow` is in the graph, but its lib does not include `textflow/src/web.rs`; the JS target builds `textflow/src/bin/textflow-web.rs` (`host/web-js/module.mjs`). The same gap covers `exact-markdown-editor`, `exact-web-js-motion`, and `exact-gpu` plus `apps/*/gpu` (`scripts/app.mjs`, `webGpuArtifacts`). No `apps/*/web` manifest depends on those, and the `/apps/` filter drops the app GPU crates. A `cfg(unix)` hole there passes this gate. `exact-game-derive` is a proc-macro and is outside this graph, so the check does not try to build it for wasm32.

4. **SHOULD-FIX** `scripts/wasm-check.mjs:33`
The crate path is the absolute directory. A checkout under a directory named `apps` or `vendor` drops every crate, and an empty list is a successful "no web app crates" (`scripts/wasm-check.mjs:46`). Windows `cargo tree` output does the same: the capture requires `(/…)` and the web-app test requires `/apps/…/Cargo.toml`, while `harnesses` in `scripts/caps.mjs` already rewrites backslashes. A parse that sees web apps and then zero libraries should fail the run.

5. **SHOULD-FIX** `scripts/wasm-check.mjs:49`
Success is `status === 0` alone. The metadata check just above this hook treats `result.error` as failure because Bun can report status 0 with `ENOBUFS`. This check never reads `error`. A missing `cargo` still fails (status is null), but the message says "0 errors" and tells the reader to add a `cfg` arm: lines such as `error: failed to download` do not match `: error[E####]:`. `cargo tree` and `cargo check` are not `--locked`, so a stale lock can rewrite `Cargo.lock` from the pre-push hook. The check itself is not `--offline` and has no timeout, so a lock that needs the network can sit there. Metadata and tree already use `--offline` and do fail closed when they exit non-zero.
grok exit 0
