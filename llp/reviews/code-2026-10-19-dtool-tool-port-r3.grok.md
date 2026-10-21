# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 3, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `a7f607ec78`.
- **Method:** one brief (sha256 `da9fcf2c31c36acfa284f7e4cbecfde5e57e55ab2e7c9b29cf481a057e96a0d5`), shared by both reviewers, blind to the other review; commit 9c9afd5737 (round 2's fixes and the contract tests' staging). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 P1).
- **Disposition:** taken in `da449db1c7` (rebased on main with stage 10 B, whose tamber-tool-files.mjs now takes `TOOL_DIST` from tamber-tool-port.mjs): `apps/design-tool/exact.mjs` calls app-cli `run`, so `bun exact.mjs test` (and `agent`, `verify`) in apps/design-tool reaches the staging (Grok P1); `stageToolTests` turns a test file or glob named in the tool's sources into the stage's (Astra P2, Grok) and always sets `EXACT_WEB_DIST` to the stage's dist; a build records the stage signature it was made from (`builtFrom`) and `buildTool` accepts it only while staging gives the same, so a build in flight while a source goes is not current (Astra P2); the stub's stage prunes any file gone (Grok); a port the tests name for a page of their own (8765) is refused (Astra); `checkoutPort` hashes the real path. Not taken: the zero-padded spelling on a five-digit port is six digits, which pageOf refuses by length, correctly (both); two processes staging while a source is deleted (Grok) is QUEUE'd (a lock would close it); the invalidation test drives the signature directly (the cross-process cases are covered by the new builtFrom test). A direct `scripts/agent.mjs`/`scripts/verify.mjs` of the tool still uses 7740 (QUEUE).

This blocks. `bun exact.mjs test` in `apps/design-tool` never enters the staging in `scripts/app-cli.mjs`, so the stub still binds 7740 on every command that actually runs.

**[P1] The advertised command has no launcher, and nothing else calls `run`.** `scripts/app-cli.mjs:86-89` is the whole of the new behavior: on `test`, if `dir` is this checkout's `apps/design-tool` and the port is not 7740, set `EXACT_TOOL_PORT` from `checkoutPort`, `stageTool({ tests: true })`, and point `EXACT_WEB_DIST` at that stage's dist. The only caller of `run` is the launcher `game/new.mjs` `commandsFor` writes (`game/new.mjs:1169`). This app has no `exact.mjs`, no `Cargo.toml`, and no `logic/src/lib.rs`. `updateApp` therefore throws `no app workspace here to update` (`game/new.mjs:1319`) and never writes one. The repo CLI rejects the verb: `scripts/exact.mjs:1212` prints `exact: no verb test` and exits 2. `scripts/app-cli.mjs` itself only runs `--lock`.

Scenario: `cd apps/design-tool && bun exact.mjs test` fails before any stage exists. `bun scripts/exact.mjs test` exits 2. The command that does run the contract tests, `bun scripts/agent.mjs web --app design-tool --test apps/design-tool/app.test.contract`, skips `app-cli` and starts `stub/tamber-stub.ts` from the source tree, which imports `API` from `apps/design-tool/app.ts:24` and listens on 7740 (`apps/design-tool/stub/tamber-stub.ts:115-116`). `QUEUE.md:1538` still records that, and `apps/design-tool/README.md:181-183` states the command that cannot start.

Below is what holds once `run()` is actually entered with a port other than 7740, then the smaller points.

### 1. Build invalidation

Both round-2 cases rebuild.

Generated `app.contract.d.ts` stays out of the inventory. `source()` excludes every `.d.ts` (`scripts/tamber-tool-port.mjs:44`), `prune` only removes names `source` accepts (`:46`), and the signature is updated only from `put` (`:49`), which the declaration file never reaches. `webChanges` also skips `app.contract.d.ts` (`scripts/agent-launch.mjs:701`, `:746`). A second launch with the same sources leaves `.exact-build.json` in place. The unit test covers that keep, and the removal when `.exact-stage` differs (`scripts/tamber-tool-port.test.mjs:32-38`).

A source deleted after an earlier `stageTool()` rebuilds too. The first call prunes the file, the hash changes, and line 67 removes the dist's `.exact-build.json` before writing the new signature. The next call sees the same signature and does not put the marker back. `buildTool` no longer consults a one-shot `changed` flag:

```1094:1094:scripts/tamber-server.mjs
  if (await builtAppMatches(TOOL_DIST, app) && jsTargetBuild(TOOL_DIST) && !webChanges(TOOL_DIST, app).all.length) return 0;
```

With the marker gone, `builtAppMatches` and `jsTargetBuild` are false (`host/web/serve.mjs:551-552`, `:683-685`). `webChanges` returns no changes when the marker is missing (`scripts/agent-launch.mjs:831`), so the deletion of the marker is what forces the rebuild. That is the sequential "something else staged first" case, including `scripts/design-tool.test.mjs:25` staging at import and `buildTool` staging again.

A failed build after that invalidation still rebuilds. The signature is committed and the marker removed before the build runs. The JS build writes `.exact-build.json` only into its temp stage, and only on success, then `replaceBuild` (`host/web/build.mjs:73-83`). Failure exits 1 (`:99`) and the exit handler removes the temp stage (`:65`). The dist marker stays absent, so the next `buildTool` fails the same freshness check. The contract-test path rebuilds either way: `webBuildQuiet` always runs `host/web/build.mjs` (`scripts/app-cli.mjs:207`).

Two processes staging the same tree with no deletion in between settle on one signature. A torn or overlapping signature write deletes the marker and causes an extra rebuild. A deletion that overlaps the two calls can stick, and only on the freshness path. `stageTool` has no lock around prune, `put`, and the signature write (`scripts/tamber-tool-port.mjs:48-67`). One process can hash the pruned tree, another can `put` the deleted file back from its earlier read, and the first can write the pruned signature last while the file is on disk. Its `buildTool`, already past `stageTool`, then builds that dirty stage and writes a new marker (`scripts/tamber-server.mjs:1094-1096`). The next `stageTool` prunes the file, sees a matching signature, and leaves the marker. `webChanges` only names files that still exist (`scripts/agent-launch.mjs:829-837`), so `buildTool` returns 0 and the bundle keeps the deleted source. The next `bun exact.mjs test` does not keep it, because that path always runs the build. A person's alternate-port `tamber tool` can.

The 7740 path is the old one. `buildTool` sets `staged` to null when `TOOL_PORT === 7740` (`scripts/tamber-server.mjs:1087`), uses `apps/design-tool`, and builds `target/design-tool-web` (`scripts/tamber-tool-port.mjs:23`). The freshness check is the three predicates above. `app-cli` does not call `stageTool` when the port is 7740 (`scripts/app-cli.mjs:89`).

### 2. Contract-test staging, once `run()` is entered

With `EXACT_TOOL_PORT` unset, `run` sets it to `checkoutPort(resolve(exact2))` before importing `tamber-tool-port.mjs` (`scripts/app-cli.mjs:87-88`). The generated launcher imports `scripts/bun.mjs` and `scripts/app-cli.mjs` only (`game/new.mjs:1166-1168`), so that assignment is the first time the port module loads. The port is one of 17741–17940 (`scripts/port-lock.mjs:20`). Its turn is that port plus 10000 (`:23`), 27741–27940. Neither range is 7740 or 7740's turn, 17740.

The staged app follows it. `stageTool({ tests: true })` copies into `target/design-tool-test-<port>`, rewrites the single `'http://127.0.0.1:7740'` in `app.ts` to `TOOL_ORIGIN` (`scripts/tamber-tool-port.mjs:57-60`), and `grants` is `` `net.fetch ${API}` `` (`apps/design-tool/app.ts:27`). Zero or two literals throw. `app.json` keeps `commands.server` on this path (`scripts/tamber-tool-port.mjs:63`).

The stub follows it. `stub/tamber-stub.ts` is copied beside the staged `app.ts`, so `import { API } from '../app.ts'` is the staged file. `../../../scripts/tamber-scrub.mjs` still resolves to the repo's `scripts/`: `target/design-tool-test-<port>/stub` is the same depth as `apps/design-tool/stub`. The stub listens on `new URL(API)` (`apps/design-tool/stub/tamber-stub.ts:115-116`) and reads `process.env.DB` (`:14`), so `appServer` has no refusal (`scripts/verify.mjs:104-107`).

The server start follows it. `serverUrl` takes the first loopback origin in the staged `app.ts` (`scripts/verify.mjs:58`, `:75-77`). `startServer` takes that port's turn, then spawns the stub with `cwd` the stage, `DB` set, and `PORT` that port (`scripts/verify.mjs:148`, `:160`). `withAppServer` waits until that URL answers before the tests run (`:265-266`). `agent.mjs --test` returns from `runTests` and does not call `driveServer` (`scripts/agent.mjs:1371-1381`). Nothing in that chain binds 7740.

The page follows it. `run` sets `EXACT_WEB_DIST` to `target/design-tool-test-web-<port>` (`scripts/app-cli.mjs:89`) and the child inherits it (`:131`). `webDist()` prefers that variable (`scripts/app.mjs:429-430`). `runTests` serves `webDist()` (`scripts/agent-test.mjs:14`, `:439`), and `resolveApp` prefers `EXACT_APP_DIR` over `--app design-tool` (`scripts/app.mjs:471-474`). The build is `host/web/build.mjs design-tool` with those two variables, so the bundle's `API` is the staged origin. The page process listens on the drive's own port (`scripts/agent.mjs:114`), not the tool port. Every import in `app.ts` is a relative sibling, and those files are copied.

Replacing every `7740` is right for six of the seven sites in `apps/design-tool/app.test.contract`. Lines 20, 25, 26, 757, 761, and 769 are the tool origin or the no-server sentence built from `API` (`apps/design-tool/app.ts:39`). `pageOf` compares `Number(port)` with `Number(new URL(API).port)` (`apps/design-tool/app.ts:108-111`), so those three host forms have to move with the port. The 8765 pages do not contain `7740` and stay.

Line 765 is the exception. `http://127.0.0.1:07740` is the leading-zero bypass: `Number("07740") === 7740` while the text is not `"7740"`. `replaceAll('7740', …)` (`scripts/tamber-tool-port.mjs:62`) turns it into `0` plus the port. Every checkout port is five digits, so the result is six (`017741`–`017940`). `pageOf`'s `\d{1,5}` rejects that before the numeric compare, and the test still expects `no-app`, so it passes for a URL that is the wrong shape. A four-digit `EXACT_TOOL_PORT` (8080) still yields a five-digit `08080` and still hits the numeric check. The unit test's `not.toContain('7740')` (`scripts/tamber-tool-port.test.mjs:42`) passes for `017999` as well. `EXACT_TOOL_PORT=7740` skips staging, so that run keeps `07740`.

On the default command, the test run does not execute `apps/design-tool` after the copy. No filename means the file is the staged `app.test.contract` (`scripts/app-cli.mjs:231`). `cargo run -p contract -- test` compiles that path (`scripts/agent-test.mjs:452`). `request` lines use `serverUrl` of the staged dir (`scripts/agent-test.mjs:469`). `stageTool` reads the source tree only to copy it.

A named file or glob does reach the source. From `apps/design-tool`, `bun exact.mjs test web app.test.contract` treats `app.test.contract` as already found in the current directory (`scripts/app-cli.mjs:155`, `:193`) and passes the source file, still full of 7740, to the compiler. The stub and the bundle are on the checkout port. `fail fetch "http://127.0.0.1:7740"` passes because nothing listens there, and `pass fetch` of that same URL fails the run. The default command has no filename, so it uses the staged file. The same split happens for a glob, which is scanned from the current directory (`:193`).

### 3. Regressions

A person's `tamber tool` leaves `EXACT_TOOL_PORT` unset, so `TOOL_PORT` is 7740, `stageTool` is not called, and the dist stays `target/design-tool-web`. The unit test asserts that origin, frame header, and dist (`scripts/tamber-tool-port.test.mjs:50-52`).

Another app's `bun exact.mjs test` fails the `realpath(dir)` comparison with `apps/design-tool` (`scripts/app-cli.mjs:86`) and is not staged. It does not set `EXACT_TOOL_PORT`. Its launcher, build, and server path are unchanged.

`scripts/design-tool.test.mjs` still sets the checkout port before importing the tool (`:22-25`) and drives `stageTool()` without tests, which strips `commands` so that drive starts no stub. It shares the checkout port with the contract tests and takes the turn (`scripts/port-lock.mjs:49`). It does not bind 7740 unless `EXACT_TOOL_PORT=7740`, which `QUEUE.md:1538` already leaves as remaining.

### Smaller

The leading-zero rewrite above is the one wrong substitution. It does not bind 7740. The three host forms of the real port still reach the numeric check in `pageOf`.

The named-file path above fails the suite. It does not start a stub on 7740, because `withAppServer` has already started the staged one.

The overlapping-stage deletion above can keep a warm alternate-port bundle. One process staging, then another, prunes and drops the marker on the call that sees the new signature.

`EXACT_WEB_DIST ??=` (`scripts/app-cli.mjs:89`) leaves a value already set. A parent that points it at `target/design-tool-web` makes this build write the checkout-port bundle into the 7740 dist. The stub still listens on the checkout port. Unset, the test dist is used.

`prune` only removes names `source()` accepts (`scripts/tamber-tool-port.mjs:44`, `:53`). `stub/dials.tamber` and `stub/regen.mjs` are copied and hashed while present, and left behind after a deletion. The stub runs `dials.json` and `tamber-scrub.mjs`; those two files are not on the test-server path.

The missing launcher is the blocker. Invalidation holds for both round-2 scenarios, a failed build after the marker is removed, and an unchanged 7740 build. The staged app, stub, turn, and page follow the checkout port when `run()` is entered, and that path binds no 7740.
