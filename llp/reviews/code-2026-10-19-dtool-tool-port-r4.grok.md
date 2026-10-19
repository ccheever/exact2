# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 4, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `6da4a13619`.
- **Method:** one brief (sha256 `c2b08f80c0c663b0768744fb7b3f0a4e1f091bf189e7f0f9b422ce8f43fba5f7`), shared by both reviewers, blind to the other review; commit 40dfdd8a77 (round 3's fixes, on the squashed change rebased on main after stage 10 B). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 P2).
- **Disposition:** taken in `9416b2090c`: a web build is stale when an input it read is gone (agent-launch.mjs `webChanges`, for every app), and host/web/build.mjs records the inputs as the JS build starts, so a source changed or gone while it compiles, or a build published over it meanwhile, is a change to the next check; that replaces round 3's stage signature (`builtFrom`), which a build publishing over another could mislabel (Astra P2); test files and globs map into the stage by their real path, a link to the checkout too (Astra P2, Grok P2), and nothing else maps, so a flag's value stays the caller's (Astra P3); `--help` is not staged (Grok).

**Request changes.** One defect: a test file or glob named through a symlink is still the source file. The stub stays on the checkout port.

**[P2] Realpath the test path before mapping it into the stage — `scripts/tamber-tool-port.mjs:94`**

`stageToolTests` compares `resolve(arg)` with `from` and `realpathSync(from)`. Bun's `import.meta.url` is already the real path (`scripts/tamber-tool-port.mjs:8`), and `apps/design-tool` is not a symlink, so those two roots are the same string: `/private/tmp/rv-dt-port4/apps/design-tool`. `resolve` does not follow links, so the `/tmp/…` spelling never matches.

`EXACT_TOOL_PORT=17999 bun apps/design-tool/exact.mjs test web /tmp/rv-dt-port4/apps/design-tool/app.test.contract` stages the tool, points `EXACT_WEB_DIST` at `target/design-tool-test-web-17999`, and starts the stub on 17999. The compiler still receives the source contract. Its `pass fetch "http://127.0.0.1:7740"` (`apps/design-tool/app.test.contract:26`) misses that stub. The same miss happens for the glob `/tmp/rv-dt-port4/apps/design-tool/*.test.contract`. The real spelling `/private/tmp/…/app.test.contract` is rewritten, and so is a relative `app.test.contract`, because `process.cwd()` is the physical path.

Realpath the argument when it exists, and for a glob realpath the directory ahead of the first `*`, `?`, or `[`, then map from that real directory into the stage.

The other taken points hold.

`apps/design-tool/exact.mjs:8-9` calls `run`. `scripts/app-cli.mjs:94-97` stages on `test`, `agent`, and `verify` before the verb is split, then passes the stage directory and the rewritten arguments. `bun /tmp/rv-dt-port4/apps/design-tool/exact.mjs --help` reaches that help text.

Argument mapping for the spellings the unit test covers: `web` stays `web` (no such path). `--size` is kept because it starts with `-`, and `390x844` is kept because it is not a file and not a glob. A real-path glob becomes `target/design-tool-test-<port>/*.test.contract`, and `Bun.Glob` scans an absolute pattern. `tests/elsewhere.test.contract` stays as given. `EXACT_WEB_DIST` is assigned to the stage dist (`scripts/tamber-tool-port.mjs:93`), so a parent value cannot send this build into `target/design-tool-web`.

`builtFrom` matches the stage `buildTool` actually compiled. `scripts/tamber-server.mjs:1215` stages first and keeps that signature. The freshness check at `:1222` accepts the dist only when `builtFrom(TOOL_DIST)` is still that signature. The record is written at `:1226` only after `host/web/build.mjs` exits 0, and that process publishes `.exact-build.json` only on success (`host/web/build.mjs:79-83`). A build that captured `dials.ts`, while another stage has since dropped it and removed the marker, finishes by recording the old signature. The next `stageTool` hashes the tree without `dials.ts`, the signatures differ, and the republished marker is not treated as current. A failed build leaves `builtFrom` unchanged and, when the signature changed, leaves the marker deleted at `scripts/tamber-tool-port.mjs:71`, so the next launch builds again. On 7740, `staged` is null (`scripts/tamber-server.mjs:1215`), the signature is not consulted, and `builtFrom` is not written.

The stub prune removes every name that is no longer in `apps/design-tool/stub`, including `dials.tamber` and `regen.mjs` (`scripts/tamber-tool-port.mjs:54`). Port 8765 is refused before `7740` is rewritten (`:64`): `app.test.contract` already contains `:8765` (`:722`). `checkoutPort` hashes `realpathSync(root)` (`scripts/port-lock.mjs:21`), so `/tmp/rv-dt-port4` and `/private/tmp/rv-dt-port4` take the same port. `scripts/design-tool.test.mjs:22` and `scripts/app-cli.mjs:95` both go through that function.

The points left open are reasoned.

A five-digit checkout port turns `http://127.0.0.1:07740` into six digits (`017741`–`017940`). `pageOf` accepts only `\d{1,5}` (`apps/design-tool/app.ts:109`) and the test still expects `no-app` (`app.test.contract:765-768`). Checkout ports are all five digits. Two processes staging across a deletion can still publish a dirty alternate-port bundle and record the pruned signature; `QUEUE.md` describes that race, and `builtFrom` does not close it. The new unit test stores and compares signatures directly. That is the comparison `buildTool` uses; the in-flight and failed-build behavior is the order in `buildTool`, above. `bun scripts/agent.mjs web --app design-tool` and `bun scripts/verify.mjs --app design-tool` still resolve the source app (`scripts/verify.mjs:1043`) and start `stub/tamber-stub.ts` on 7740. That remains the queue item.

Stage 10 B's doors serve `TOOL_DIST`. `scripts/tamber-tool-files.mjs:7` takes it from `tamber-tool-port.mjs`. With `EXACT_TOOL_PORT` unset that is `target/design-tool-web`. With another port it is `target/design-tool-web-<port>` (`scripts/tamber-tool-port.mjs:23`). `serveToolFiles` defaults to that directory (`scripts/tamber-tool-files.mjs:29`). The local server does too (`scripts/tamber-server.mjs:153`). `openRemote` passes `toolDir` through unset (`scripts/tamber-server.mjs:1167`, `host/web/dev.mjs:1498`), and `remoteHandler`'s default is the same `TOOL_DIST` (`scripts/tamber-remote.mjs:135`). An undefined argument still takes the default, so both doors on 7740 and on another port serve the build that port just made.

A person's `tamber tool` on 7740 leaves `EXACT_TOOL_PORT` unset, skips `stageTool`, and keeps `target/design-tool-web` and the old freshness check. Another app's `bun exact.mjs` fails the `realpath(dir)` comparison in `scripts/app-cli.mjs:94` and is not staged. Generated launchers still call the same `run` (`game/new.mjs:1169`); only `apps/design-tool` enters this branch.

Smaller: `bun exact.mjs test --help` (and `agent` / `verify`) stages before help is recognized, because the stage runs at `scripts/app-cli.mjs:94` and help is chosen at `:117`. `EXACT_TOOL_PORT=8765 bun exact.mjs test --help` throws the page-port refusal instead of printing help.
