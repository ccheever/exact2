# Code review: The design tool's direct drive and verify on the checkout's own port (scripts/tool-drive.mjs), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `dad72a284a`.
- **Method:** one brief (sha256 `0d2499b497e24339feac41eafd9b2d596e539de5207457989d1f6d34f66743d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (a path-valued --app undid the staging and ran the source tree on 7740), two should-fix (help staged; the test missed the entry points)
- **Disposition:** all taken in 6f08d91bbf: resolveApp's precedence, the path given way to the tool's name after staging, help stages nothing, agent.mjs and verify.mjs tested as entry points

Found **one blocking issue** and two should-fix items in HEAD (`dad72a284a`).

1. **Blocking — path-valued `--app` undoes staging.** [tool-drive.mjs:36](/tmp/rv-tooldrive-1/scripts/tool-drive.mjs:36) sets `EXACT_APP_DIR` but returns the original `--app` path. [resolveApp:476](/tmp/rv-tooldrive-1/scripts/app.mjs:476) then overwrites that environment variable with the path. Consequently, `bun scripts/verify.mjs --app apps/design-tool` and the equivalent agent drive resolve the original app, whose stub binds **7740**. Absolute paths and `--app .` inside the tool have the same problem. Also, `namedDir` gives the environment precedence, whereas `resolveApp` gives an explicit path precedence, so an unrelated `EXACT_APP_DIR` can bypass staging entirely. Match that precedence and rewrite or remove the path argument after staging.

2. **Should-fix — `--help` now performs staging.** [tool-drive.mjs:29](/tmp/rv-tooldrive-1/scripts/tool-drive.mjs:29) runs before either CLI’s help guard. Naming the design tool with `--help` or `-h` therefore writes/prunes staged files; an invalid `EXACT_TOOL_PORT` or staging failure prevents help from appearing. Mirror [app-cli.mjs:94](/tmp/rv-tooldrive-1/scripts/app-cli.mjs:94), which explicitly skips staging for help.

3. **Should-fix — the regression test misses the integration being fixed.** [tool-drive.test.mjs:18](/tmp/rv-tooldrive-1/scripts/tool-drive.test.mjs:18) invokes only `toolDrive`; removing either entry-point hook would leave these tests passing. Its path case checks `appServer(stage)` directly, missing finding 1’s subsequent resolution back to the source directory. Cover both entry points through argument parsing and app resolution, including path/environment precedence and help.

The remaining checks look sound by inspection:

- No static dependency captures `TOOL_PORT` or loads `tamber-tool-port`/`tamber-server` before the hook.
- Verify imports `open`/`runTests` in-process; it does not spawn `agent.mjs`. Existing staged runs therefore avoid repeated staging and test-path mapping.
- Crate suffix handling matches `resolveApp`; other apps, including games, remain untouched. Agent’s host-first argv fits `testFiles`; verify selects its default test from `app.dir`.
- `--app=` and `--test=` are recognized by the helper but remain rejected by the actual CLI parsers.
- Existing ESM importers skip the guarded entry blocks; I found no top-level-await breakage. Staging rejections reach the existing error handlers before `main`.

The README’s name-based example is accurate, but removing the QUEUE item is premature while finding 1 remains.

Read-only review; no builds or tests run.
