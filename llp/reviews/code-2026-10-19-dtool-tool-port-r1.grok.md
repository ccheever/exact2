# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `2b4bae29e7`.
- **Method:** one brief (sha256 `919e13e6d6ebfe76a7073a18acb2f15f9d825d6c15b64014403fa39b812f331e`), shared by both reviewers, blind to the other review; commits f0192030bf 3b42c0fc18 2b4bae29e7. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (smaller points).
- **Disposition:** taken in `14e6c27147` (with rounds 1 and 2 and the contract tests' staging, squashed): the tests' agent drives name the staged tool (`EXACT_APP_DIR`), whose app.json names no server, so no drive starts the stub on 7740; staging that moves or removes any file makes the build again (`changed`), so a deleted source can't leave a warm bundle accepted; the API literal must appear exactly once or staging refuses; an `EXACT_TOOL_PORT` that is not a port from 1024 to 65535 is refused, never 7740 by default (and port 80 no longer listens on 0: the listener takes `TOOL_PORT`); port-lock's note updated; the checkout-port comment admits a hash collision. Not taken: an actual browser frame refusal from 7740 (the CSP string is the whole mechanism, asserted exactly).

Nothing blocks. The three commits keep 7740 for a person and for the contract stub, and a test run on this checkout serves, grants, and frames one other loopback origin.

The plan file at `/Users/admin/x2bench/to-dtool2.md` is not on this machine. The conditions checked are the ones in the request: rebuild the staged tool when its sources change, `FRAME_ANCESTORS` names exactly the chosen origin, and the queue entry for the contract stub stays.

**Staged build and the default build.** With `EXACT_TOOL_PORT` unset, `TOOL_PORT` is 7740 (`scripts/tamber-tool-port.mjs:11`), the origin string is `http://127.0.0.1:7740` (`:18`), and the dist is `target/design-tool-web` (`:15`). `buildTool` then points `EXACT_APP_DIR` at `apps/design-tool` and passes that directory plus `EXACT_WEB_DIST` of that same dist (`scripts/tamber-server.mjs:1087`, `:1095`). `stageTool` is not called, so `apps/design-tool` is never rewritten. A person’s `tamber tool` still compiles the same tree into the same directory.

With another port, `stageTool` copies the top-level `.ts`, `.contract`, and `.json` files into `target/design-tool-src-<port>` and rewrites only `app.ts`’s API literal (`scripts/tamber-tool-port.mjs:22-31`). That literal is the single quoted `http://127.0.0.1:7740` at `apps/design-tool/app.ts:24`, which `staticFacts` folds into the grant. `resolveApp` takes `EXACT_APP_DIR` over the `design-tool` argument (`scripts/app.mjs:472-474`), and `webDist()` follows `EXACT_WEB_DIST` (`scripts/app.mjs:429-430`), so the bundle lands in `target/design-tool-web-<port>`.

An edit is visible to the freshness check. `stageTool` rewrites a staged file when its text differs, which updates its mtime; `webChanges` nominates files newer than the dist marker and then compares digests (`scripts/agent-launch.mjs:645`, `:829-837`). `buildTool` rebuilds when that list is non-empty (`scripts/tamber-server.mjs:1093`). A removed source is deleted from the stage before that check (`scripts/tamber-tool-port.mjs:26`), so the next build compiles the directory as it is. A deletion by itself does not bump another file’s mtime, and `webChanges` does not treat a missing path as a change — the same rule as a person’s 7740 build. Deleting `holds.ts` and fixing the import in `app.ts` still rebuilds, because `app.ts` changed. The port dist and `target/design-tool-web` stay separate directories.

**Origin, grants, and frame-ancestors.** `FRAME_ANCESTORS` is `frame-ancestors 'self' ${TOOL_ORIGIN}` (`scripts/tamber-server.mjs:1050`). Both dev loops call `frameOnlyByTool` on every response (`host/web-js/dev.mjs:858`, `host/web/dev.mjs:1333`), and that header is fixed when the process loads `tamber-server.mjs`. `scripts/design-tool.test.mjs:21` sets `EXACT_TOOL_PORT` before the import on `:22`. Spawns either inherit the environment or pass `...process.env` (the dev loops at `:850`, `:957`, `:1107`, and `:1200`). The tool process, the design server, and those dev loops therefore name the same origin. The glue step runs `bun test --parallel`, which isolates files, so this env does not leak into `tamber-server.test.mjs`.

**Two checkouts, a hash collision, a leftover listener.** The port is `17741 + (sha256(checkout) mod 200)`, so it sits above 17740, the turn port for 7740 (`scripts/port-lock.mjs:15`). Two checkouts normally bind different ports and different `target/` trees (`AGENTS.md` refuses a shared `target/`). A hash collision is 1 in 200: both call `takePort` on the same port, and the second waits on a listen on `port + 10000`, which the kernel releases when the holder exits. They do not both bind the tool port, and they do not fall back to 7740.

A listener left on the port fails the next run. `beforeEach` takes the turn, and if `lsof` sees a listener it releases the turn and throws (`scripts/design-tool.test.mjs:34-38`). If that probe misses, `toolCommand` still refuses a server whose `index.html`, `app.js`, or `app.plan` differs from this checkout’s dist (`scripts/tamber-server.mjs:1154-1156`).

**Security.** `TOOL_PORT` is `Number(EXACT_TOOL_PORT) || 7740`, and the origin is always `http://127.0.0.1:` plus that number (`scripts/tamber-tool-port.mjs:11`, `:18`). A non-numeric value becomes 7740. The number’s decimal form cannot add a second host or a `*`. The header names `'self'` and that one origin.

The queue entry is present (`QUEUE.md:1538`): `scripts/design-tool.test.mjs` moves, and `test web` in `apps/design-tool` still starts the stub on 7740 because that app is compiled against the literal.

Smaller points, none of them blocking:

- `scripts/port-lock.mjs:6-7` still says `scripts/design-tool.test.mjs` holds 7740. After this change only the contract stub does.
- The replace in `stageTool` (`scripts/tamber-tool-port.mjs:29`) does not check that it matched. It matches `app.ts:24` today. If that literal’s quotes change, staging still succeeds and the grant stays on 7740 while the server listens elsewhere.
- `scripts/tamber-tool-port.test.mjs` checks the staged `app.ts`, the header string, and the dist path. It does not run the web build, so the rebuild path is the existing `webChanges` behavior above, not a direct assertion.
