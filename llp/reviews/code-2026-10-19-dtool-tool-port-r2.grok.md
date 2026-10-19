# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ab563ed6e2`.
- **Method:** one brief (sha256 `7b5c668b0ac28482f0e3435acc1a53fce62a7a5d36a62e572a9a0cfac0472ec2`), shared by both reviewers, blind to the other review; commit 486ae7b8cf against round 1. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 P1).
- **Disposition:** taken in `14e6c27147` (with rounds 1 and 2 and the contract tests' staging, squashed): staging keeps what the build writes there (`app.contract.d.ts`, any `.d.ts`), so an unchanged launch builds nothing; it hashes the staged sources (names and contents, sorted) into `.exact-stage` and, when that signature differs from the last staging's, removes the dist's build marker, so whichever process staged first, the next build is made again (a deleted source included); the unit test now checks the marker kept when nothing changed, gone when the sources changed, and the declarations kept. Also in that commit, the lead's top priority: `bun exact.mjs test` in apps/design-tool runs a stage with its stub and tests on the checkout's port. Not taken: a browser frame refusal from 7740 (both reviewers judge the exact CSP assertion enough).

This blocks. The rebuild signal `changed` does not mean "the staged sources differ from the build."

**[P1] A later `tamber tool` in the same run rebuilds because the build's own declaration file counts as a removed source — `scripts/tamber-tool-port.mjs:38`**

`stageTool` deletes every staged file that is not in `apps/design-tool` and sets `changed`. The design-tool web build then writes `app.contract.d.ts` beside `app.ts` (`host/web-js/build.mjs:247-250`; `apps/*/app.contract.d.ts` is gitignored, so the source tree does not have it). `buildTool` treats `staged.changed` as "rebuild" (`scripts/tamber-server.mjs:1094`).

`scripts/design-tool.test.mjs` starts a tool per test (`:72`, `:169`, `:289` via `proxied`, and the later `--via` launches). On a cold dist the first launch builds and leaves `target/design-tool-src-<port>/app.contract.d.ts`. The next launch's `stageTool` deletes that file, `changed` is true, and the tool is built again. The freshness check used to ignore that file (`scripts/agent-launch.mjs:701`). A suite run now pays a full design-tool build for every tool start after the first.

The same cycle hits a direct `EXACT_TOOL_PORT=17999 bun exact.mjs tamber tool`: the second launch deletes the declaration file and rebuilds with no source edit.

**[P1] A deleted source still leaves the warm bundle when anything stages first — `scripts/design-tool.test.mjs:25`**

`changed` is only true on the call that writes or deletes. The test file calls `stageTool()` at import and keeps `.dir`. The tool process calls it again inside `buildTool` (`scripts/tamber-server.mjs:1087`).

Delete `apps/design-tool/dials.ts` (`app.ts:12` imports it) after a warm `target/design-tool-web-<port>`. Import removes the staged copy and drops `changed`. The tool's `stageTool` finds the stage already matching, so `changed` is false. `webChanges` only names files that are still there and newer than the marker (`scripts/agent-launch.mjs:644-646`, `:829-837`). `builtAppMatches` still accepts the old manifest and outputs. `buildTool` returns 0. The bundle still contains `dials.ts`, and the missing import is never reported. That is the round-1 deletion case, on the path this commit added.

The direct one-process `tamber tool` path does set `changed` for a real deletion. The suite never uses that path: import stages first.

What holds:

- **Drives.** With the suite's port (17741–17940), each agent spawn passes `EXACT_APP_DIR` of the stage and no `--app design-tool` (`scripts/design-tool.test.mjs:25`). The staged `app.json` has `commands` removed (`scripts/tamber-tool-port.mjs:46`). `driveServer` (`scripts/agent.mjs:1397`) only starts `app.json`'s `commands.server`, else `server.ts` (`scripts/verify.mjs:97-101`). The stage has neither, and `stub/` is not copied, so a drive does not start `stub/tamber-stub.ts` on 7740.
- **One API literal.** `app.ts:24` is the only `'http://127.0.0.1:7740'`. `grants` uses `${API}`. Zero or two matches throw before replace (`scripts/tamber-tool-port.mjs:41-44`). `app.test.contract` is copied unchanged, which is what keeps the contract tests on 7740.
- **Port.** Unset stays 7740. `''`, `0`, `x`, `80`, and `70000` throw (`scripts/tamber-tool-port.mjs:12-17`, asserted at `scripts/tamber-tool-port.test.mjs:27`). The listener binds `TOOL_PORT` (`scripts/tamber-server.mjs:1162`).
- **A person's `tamber tool` on 7740.** `stageTool` is not called, the app directory stays `apps/design-tool`, the dist stays `target/design-tool-web`, and `!staged?.changed` leaves the old freshness check. `apps/design-tool/app.json` is untouched.
- **Contract tests.** They still start `stub/tamber-stub.ts`, which imports `API` from the source `app.ts` (`apps/design-tool/stub/tamber-stub.ts:10`, `app.json:16-18`). `QUEUE.md:1538` is unchanged.
- **Not taken.** An actual browser frame refusal from 7740 stays untested. The header is the whole mechanism (`FRAME_ANCESTORS`, applied in `frameOnlyByTool`), and `scripts/tamber-tool-port.test.mjs:16` asserts that string exactly. That deferral is sound.
- The port-lock note and the hash-collision comment match the code. Both are comments only.

Smaller: `scripts/tamber-tool-port.test.mjs` checks that `gone.ts` disappears and that `commands` is absent. It never reads `changed` or a warm dist, so neither failure above is caught. If `EXACT_TOOL_PORT` is explicitly `7740`, `DRIVEN` still passes `--app design-tool` and the stub starts again. The suite's own port is never 7740.
