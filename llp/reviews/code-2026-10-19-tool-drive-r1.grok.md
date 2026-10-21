# Code review: The design tool's direct drive and verify on the checkout's own port (scripts/tool-drive.mjs), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `dad72a284a`.
- **Method:** one brief (sha256 `0d2499b497e24339feac41eafd9b2d596e539de5207457989d1f6d34f66743d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (the same path hole), two should-fix (the same), one nit (--app= handled only here)
- **Disposition:** taken in 6f08d91bbf; --app= dropped, as the CLIs refuse it

One blocking issue: a path-shaped `--app` still starts the design tool's stub on 7740. The name form (`--app design-tool`) is fixed.

1. **Blocking** — A path-shaped `--app` still binds 7740. `scripts/tool-drive.mjs:36` and `scripts/app.mjs:476`.

`namedDir` treats a path the way `resolveApp` does (`/`, `\`, `.`, `..`), and for `apps/design-tool`, `./apps/design-tool`, or `--app .` from that directory it does stage and set `EXACT_APP_DIR` to `target/design-tool-test-<port>`. It then returns the original argv, so the value is still that path. `resolveApp` overwrites `EXACT_APP_DIR` with the source tree whenever its argument matches that same pattern, and both entry points pass the raw value through: `parseFlags` in `scripts/agent.mjs:337` into `resolveApp(flags.app)`, and `value('--app')` in `scripts/verify.mjs:1043`. The stub is `app.json`'s `stub/tamber-stub.ts`, and `appServer` takes the port from `app.ts`. The source file still has `http://127.0.0.1:7740`, so the server listens there. `EXACT_WEB_DIST` stays on the checkout-port dist from `stageToolTests`, so the build directory and the server can disagree about the origin.

The same split shows up when `EXACT_APP_DIR` is already set to some other app and `--app` is a path to this checkout's design tool. `namedDir` (`scripts/tool-drive.mjs:17`) keeps the environment variable and does not stage; `resolveApp` lets the path replace it and runs the source tree. A bare name still works: `--app design-tool` and `--app design-tool-web` are not paths, so the stage directory wins and the crate name stays `design-tool`.

2. **Should-fix** — `--help` and `-h` stage before help is printed. `scripts/tool-drive.mjs:28` and `scripts/agent.mjs:1498`.

`app-cli.mjs` skips staging when the arguments include `--help` or `-h`. These entry points call `toolDrive` first and `guardFlags` only inside `main`. `bun scripts/agent.mjs web --app design-tool --help` (and the same with `EXACT_APP_DIR` pointed at the tool and no `--app`) copies the stage, and a throw from `stageTool` is printed as the command's error instead of the help text. The rejection handler itself is fine for a real drive: the message is printed and the process exits 1, and `main` does not run.

3. **Should-fix** — The test never runs the handoff that finding 1 breaks. `scripts/tool-drive.test.mjs:27`.

`drive()` calls `toolDrive` in a child and checks the environment it sets. The verify case uses `--app apps/design-tool` and then `appServer(stage)` on that directory directly. That stays green while `verify`'s `main` and `agent`'s `main` still resolve the source tree. Nothing here loads `agent.mjs` or `verify.mjs` as the entry script, so `--help`, the entry-line `await`, and `resolveApp` after staging are untested.

4. **Nit** — `--app=` is only handled in `namedDir`. `scripts/tool-drive.mjs:18`.

`parseFlags` and verify's `value('--app')` only see a separate `--app` argument (`help.mjs` documents `--app <name>`). `bun scripts/agent.mjs web --app=design-tool tree` stages, then `guardFlags` exits 2 on an unknown flag. No server starts.

What holds:

- Crate suffixes match `resolveApp`: only a trailing `-web|-apple|-ios|-macos|-linux|-windows|-gpu` is stripped, and a path is not stripped. `design-tool-web` stages and the crate name stays `design-tool`.
- Game apps are left alone. `namedDir` looks only under `apps/<name>`. A name that exists only in `game/games` fails `existsSync` and `toolDrive` returns the original argv; `resolveApp` then applies its `game/games` fallback.
- For a non-path `--app`, `EXACT_APP_DIR` wins in both places. Through `exact.mjs`, the child already has `EXACT_APP_DIR` set to the stage, the real paths differ, and `toolDrive` returns immediately. That is the no-op.
- Neither entry's static graph loads `tamber-tool-port.mjs` or `tamber-server.mjs`. Across those imports (57 modules), `TOOL_PORT` and `EXACT_TOOL_PORT` appear only in comments and inside `app-cli.mjs`'s `run()`. `toolDrive` sets `EXACT_TOOL_PORT` before its dynamic import, which is what `TOOL_PORT` reads.
- `verify` does not spawn `agent.mjs`. It calls `runTests` and `open` in-process, and the test file is `app.dir/app.test.contract` after `resolveApp`. `app-cli`'s child already has the stage in `EXACT_APP_DIR` and already-mapped arguments, so the child returns before `testFiles` and does not map twice.
- `testFiles` gets the same shape `app-cli` uses: host first, no verb. For `agent` and `verify` only `--test` values are files, which is what those verbs pass. Verify's own run does not use `--test` on argv.
- The new `await` sits on the entry condition (`import.meta.main` in verify, the `argv[1]` path check in agent). Importers do not take that branch. `agent.mjs` and `verify.mjs` are imported widely; nothing in those modules reads the port at load.
- `EXACT_TOOL_PORT=7740` returns before `EXACT_APP_DIR` is changed. Other apps return before the port is set.
- The README sentence matches the name form and the 7740 opt-in. Dropping the QUEUE line matches that same name-form gap; the path hole above was not what that line described.
