# Code review: design-tool.test beside another run, follow-up round on 5e5435b866 (round 1 answered), b85aa54004 (per-dist JS dev stage) and a93c2f4d22 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** follow-up round on 5e5435b866 (round 1 answered), b85aa54004 (per-dist JS dev stage) and a93c2f4d22; HEAD `a93c2f4d22`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; its SHOULD-FIXes and NITs answered in 3c4a6c99f0, but for the GPU cache (stated out of scope).

I'll review those three commits read-only against the two round-1 findings and the staging-path questions, without changing the tree or running builds.The three commits are on top of `c1db7328af`. Next I’ll read each diff against the two round-1 findings and the stage-path questions.The diffs are small. I’ll read the port-lock contract, the T2b fixture path, and every use of the stage directory.The port turn and the stage directory are the two places a second copy can still collide. I’ll read those paths in the test and the dev loop.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is almost written. I’ll confirm the line numbers on the stage path, the new concurrency test, and the two round-1 fixes before filing findings.The concurrency test is the open question. I need to see how a temp app picks `app.target` when the loop’s working directory is the repo.Nothing blocks. Both round-1 fixes close their findings, and the per-dist stage is the right change. The new concurrency test does not pin the shared target it describes.

Round 1 (a) is closed. T2b takes `1000 + checkoutPort(apps/theme-fixture)` (`scripts/design-tool.test.mjs:1116`), a key in 18741–18940 whose turn listen is 28741–28940, clear of the tool band 17741–17940 and its turns 27741–27940. Two copies of one checkout wait on that turn. The design server is stopped in the test’s `finally` (`scripts/design-tool.test.mjs:1179`) and, on timeout, by synchronous `stopAll` in `afterEach` (`scripts/design-tool-harness.mjs:49`) before `onTestFinished` gives the turn back, so the flock is gone before the next writer.

Round 1 (b) is closed. `claimPort` starts at `checkoutPort(ROOT) + 1` (`scripts/design-tool-harness.mjs:17`). The contract stub binds that checkout port (`scripts/app-cli.mjs:95`) and holds it with `takePort` (`scripts/verify.mjs:149`), so it is the last of the 200 candidates and is skipped while the stub holds its turn.

`web-js-dev-stage` appears only at `host/web-js/dev.mjs:95`. `replaceBuild` (`host/web/serve.mjs:606`) renames that stage directory onto `dist`, so the extra hash directory stays behind as the parent. Two loops on one resolved `dist` still share that stage and that `dist`; the wipe stays inside the hash directory and the publish stays under `dist.lock`, the same fight over one `dist`. T2’s check (`scripts/design-tool.test.mjs:773`) sees a failed build: `host/web-js/dev.mjs:232` prints `does not build` before the URL line the wait returns on.

1. **SHOULD-FIX** `host/web-js/dev-builds.test.mjs:216` — The two-loop test never puts both apps on one `target/`. `devLoop` passes each temp dir as `--app` and only adds `EXACT_WEB_DIST` (`host/web-js/dev-builds.test.mjs:29`). For an outside app with no `Cargo.toml`, `resolveApp` runs `cargo locate-project` in that directory (`scripts/app.mjs:530`); from `os.tmpdir()` that fails, `knownWorkspace` is empty, and each app’s target is `<that dir>/target` (`scripts/app.mjs:545`) unless the environment already exports `CARGO_TARGET_DIR`. The old shared `<app.target>/web-js-dev-stage` is then two directories, so this test still passes on the code the commit says it catches. The hash at `host/web-js/dev.mjs:95` is the right fix for loops that do share a target. Give both loops one `CARGO_TARGET_DIR`.

2. **NIT** `host/web-js/dev.mjs:1015` — `web-dist-resident` is one directory per target. Two JS loops on one target that both forward a native client share it. That path is the resident wasm producer, reached only from a native-client forward.

3. **NIT** `host/web-js/dev.mjs:377` — `inspect-${digest}.json` is one file per target and plan digest. A second loop’s `rmSync` (`host/web-js/dev.mjs:386`) can drop the other’s table. The miss deletes the cache entry and the next ask retries.

4. **NIT** `host/web-js/build.mjs:725` — `web-js-gpu/<app name>` is removed and copied when the app has `gpu/src/lib.rs` (`host/web-js/build.mjs:729`). Two loops of one GPU app on one target can race there. The apps in these commits have no GPU crate.

The compiler copy at `host/web-js/dev.mjs:528` is per app name; a pair that does not match falls back to the debug compiler.
