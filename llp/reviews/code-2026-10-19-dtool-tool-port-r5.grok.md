# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 5, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `7b4db39f80`.
- **Method:** one brief (sha256 `53f233b9d82c68b6260697429f33aaad97ad368252b7b38b0ea457ff539c4fcc`), shared by both reviewers, blind to the other review; commit f5cee1c0e4 (round 4's fixes: two general web-build freshness changes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 P2).
- **Disposition:** taken in `caef755d8d`: host/web/build.mjs records when the JS build read its inputs (`inputsAt`) and webChanges nominates every input changed since then (less two seconds of clock grain), not since the marker, so an edit during the compile, or a build published over another, is compared with the digest read (both, P2); only test files and globs (`….test.contract`) map into the stage (Astra P2). Not taken: the wasm path and the JS dev loop's markers (no `inputs`; the dev loop watches its sources) keep their own freshness, as before (Astra, Grok note it, no regression); Expose's build-written local.ts is deterministic, so it reads as a change at most once after it moves (Astra).

**Request changes.** One defect blocks: a source edited while a JS build compiles can stay current. The new start-of-build digest is not what the next check reads.

**[P2] The start snapshot is ignored for any input still on disk — `scripts/agent-launch.mjs:834`**

`host/web/build.mjs:68` records `webInputDigests` before the compile, and the marker is written only after the compiler returns (`host/web/build.mjs:82`). `webChanges` then asks `webInputs` only for files newer than that marker's mtime (`scripts/agent-launch.mjs:834`) and compares digests only for those (`:833-835`). An edit made during the compile is older than the marker, so the stored digest is never read.

Two `tamber tool` builds on port 17999:

1. A stages `dials.ts` as S1 and the child records `hash(S1)`.
2. The source becomes S2. B's `stageTool` writes S2 into the same stage (`scripts/tamber-tool-port.mjs:46`, mtime Tb) and publishes a bundle of S2.
3. A then `replaceBuild`s its S1 bundle and its marker (mtime Tm > Tb) over B.

The next `buildTool` stages again. The stage already equals S2, so `put` does not write and the mtime stays Tb. `webChanges` sees nothing newer than Tm. `scripts/tamber-server.mjs:1222` returns 0. The bundle being served is A's compile of S1.

Round 3's `builtFrom` compared the stage signature with no mtime gate, so this rebuilt, except when a parent stamped the other build's signature. This commit removes that stamp and that compare together. Publishing the inputs with the bundle is the right half. The check still has to use them when the bytes changed and the mtime did not.

The same gate covers every non-staged app. Save `app.ts` while `bun host/web/build.mjs hello` compiles. The next `webChanges` in `scripts/agent.mjs:87`, `scripts/verify.mjs:278`, `scripts/agent-playwright.mjs:75`, `host/web-js/conform.mjs:307`, and the `host/web/tests` `refuseStale` calls accepts the pre-save bundle. `verify.mjs` and `buildTool` rebuild only when `.all` is non-empty. Playwright refuses on `.app` and warns on `.shared`.

A deletion is caught. The new loop (`scripts/agent-launch.mjs:837`) does not look at mtime: a key with a non-null digest whose path is gone is a change. Remove `dials.ts` while A compiles, B prunes it, A publishes a marker that still names it, and the next check rebuilds.

These do not force a rebuild on every launch.

- `.exact/backend-origin` is always listed (`scripts/agent-launch.mjs:813-817`). When it is absent, `contentDigest` stores null, and the new loop skips null (`:837`). The existing always-compare already notices create, edit, and delete. After a rebuild records null, the next check is empty. The new test's delete-then-bake is that case.
- `app.contract.d.ts` is `DECLARATIONS` and `notBuildInput` drops it before the snapshot (`:700`, `:746`). The JS build writes it beside `app.ts` during the child (`host/web-js/build.mjs:247-250`), after the snapshot. It is not a key, and a later run does not treat it as gone.
- Other writes from that child go to the output stage or to Cargo's `OUT_DIR` under `target/`. `newerThan` skips a directory named `target` (`:634-645`). Those paths are not keys. A file the compile rewrites and leaves in place is older than the marker, so it is not nominated. A file the compile creates is absent from the start snapshot; deleting it later is invisible, which is the reverse hole, and it does not repeat.
- Keys are `shown` paths (`:486`): relative under this checkout, absolute outside it (the freshness test's temp app, `backend-origin`, a game file outside the repo). `existsSync(resolve(ROOT, path))` is the same path `contentDigest` hashed. `resolveApp` realpaths `app.dir` (`scripts/app.mjs:477`), so a symlinked app dir is stored as the real path. A live file is not reported gone.
- A game's proof inputs (`:805-808`, `game/inputs.mjs` `proofInputFiles`) are files that existed at the snapshot. A tracked file that stays deleted is one rebuild; the next wasm snapshot no longer lists it. Gitignored generated assets are excluded there, so they are not keys. Proof files are not an absence sentinel.

Callers of `webInputDigests`: the JS build (before the compile), the wasm build (after, `host/web/build.mjs:499`), the two freshness tests, and `game/proof-snapshot.test.mjs`. `host/web-js/dev.mjs:139` still writes the marker after the compile and still omits `inputs`. Its watcher treats a source that disappeared as a reason to rebuild (`:119-124`). A later `webChanges` on a dev dist still cannot see a deletion, because there are no keys; with no `inputs`, only files newer than the marker count (`:833`). The wasm path has the same during-compile hole: a file removed before the end snapshot is not a key. Neither path undoes the JS deletion check.

The extra cost is one `existsSync` per recorded key on every `webChanges`, on top of the walk that already stats the live tree. In this checkout that shared JS set is a few hundred files (`host/web-js` 104, `contract` 224, `plan` 11, `kernel/tables` 1, the top of `host/web` 63, plus the app). A game stats its proof list instead. It does not re-read those files.

`stageToolTests` holds for the cases you named (`scripts/tamber-tool-port.mjs:81-83`).

- An existing file is `realpathSync`'d, so `/tmp/…/apps/design-tool/app.test.contract` and the relative `apps/design-tool/app.test.contract` (cwd at the checkout, as the unit test calls it) both become the stage file.
- A glob realpaths the directory before the first `*`, `?`, or `[`. A link whose target is the checkout, including a link only on the app directory, becomes `stage/*.test.contract`.
- A path outside the tool, such as `tests/elsewhere.test.contract`, stays as given.
- `--shots .` and `--storage app.ts` do not end in `.test.contract` and are not globs, and a token that starts with `-` is left alone. `verify` still resolves `--shots` from the caller's cwd (`scripts/app-cli.mjs:151` spawns with no `cwd`).

`--help` and `-h` skip staging (`scripts/app-cli.mjs:94`) before `tamber-tool-port.mjs` is imported. `EXACT_TOOL_PORT=8765 bun exact.mjs test --help` reaches `printHelp` (`:117-121`) and does not hit the page-port refusal. That matches the help test already used at `:117`.

Nothing else in this commit blocks. The in-flight content edit does.
