# Code review: LLP 1122 T3 step 1, the param rung in the selection join, round 3, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, the same throwaway worktree at `5c7138e94` (`2d59d8578` after the rebase).
- **Method:** one brief (sha256 `a006ec781c9a3f315c2fa6f7d99e3718497953995b6ba8ecc97871b66d3263f7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (the run compacted its context once).
- **Verdict:** nothing blocks; smaller points.
- **Disposition:** taken in `ff449500e`: a describe whose files are not those its read was for is not kept (and is tried again); a write's files read the params again (the server's publish); two params whose page property would be one are named by neither in a page's text, so the suffixed one never takes the other's name; a held state's rows (stage 8 E, which the rebase brought) are scrubbed as the selection's. The stale comment is corrected. The param tests moved to scripts/tamber-param-rung.test.mjs (`8238fc96d`, the line cap), with a test of the files mismatch.

Nothing blocks. The four round-2 defects are fixed in this tree, and the fixes do not bring the alias or the intermediate name back.

## Round 2

**A new plan no longer offers the previous alias.** `learnParams(null, planDigest)` in `scripts/tamber-server.mjs:368-372` copies the map and sets `token` to null before `join` runs. Select `Card.fill` after a package rebuild moves the plan from P1 to P2: the row stays `{rung: "param", param: "app.contract#Card.fill", short: "Card.fill"}` with no `token`, so re-scope cannot write `ink` while `describe` is still out. When that `describe` returns, `params.map` is replaced from the new intermediate representation (`:378`). The server test locks the in-flight shape at `scripts/tamber-server.test.mjs:113-116`.

**A failed re-read is not sticky.** On an error, a down server, or the 120s timeout (`ask` resolves `{status: 504}` at `scripts/tamber-server.mjs:335`, it does not reject), `failed` clears both `params.key` and `params.plan` (`:375-379`) only if that read is still current. The next selection sees `planDigest !== params.plan` (`:221`) and calls `learnParams` again. A late success after the timeout does not land: the promise has already settled, and a newer `reading` ignores it.

**The JavaScript custom property is named.** `CSS_PARAM` (`scripts/tamber-inspect.mjs:114`) matches `var(--x-param-f` + eight hex digits + `-…)`, and `paramsOf` builds the same string modes.rs does by turning every non-identifier character into `-` (`:132`). `color-mix(in oklab, var(--x-param-f1a2b3c4d-PostButton-fill) 50%, black)` becomes the full name (`scripts/tamber-inspect.test.mjs:140-144`). A single `var(--x-param-…)` never reaches this path: the page replaces that text with the token name first, and the dotted-name scrub covers it.

**The dotted name is a whole word of the real shape.** `IR_PARAM` (`scripts/tamber-inspect.mjs:113`) is `(?<![\w.-])param\.f[0-9a-f]{8}\.[A-Za-z_][\w-]*\.[A-Za-z_][\w-]*(?![\w.-])`, which is what `token_name` emits (`contract/lower/src/params.rs:25-36`: `f` plus eight lowercase hex digits, then component and param). `ui.param.face.fill` stays a token rung (`scripts/tamber-inspect.test.mjs:142-146`).

**Role casing matches the checker.** `roleKey` (`scripts/tamber-inspect.mjs:116`) lowercases and strips one leading `-exact-`, then looks the schema name up. `label`, `Label`, `-exact-label`, and `-Exact-Label` all alias the row spelling `-exact-label`. An ordinary token is still an exact `names.has` hit before that, so `Primary_500` does not alias `primary_500`. The `-Exact-Label` case is in the inspect test (`:112`).

The protocol text matches this: both events, the custom property, names kept and no `token` while a plan read is out, a failed read tried again, a row whose one name is a param linking that full name, and a client with no param rung offering no edit. This checkout's inspector does that last part (`apps/design-tool/app.contract:1152-1163`): `rung == "param"` is taken before the "Open …" link.

## Still fixed from round 1

The published `selected` event is `joined.publish` (`scripts/tamber-builds.mjs:327-329`), the same scrub as `selection` (`scripts/tamber-server.mjs:239`). A synthetic `color-mix(… param.f… …)` is rewritten inside the name. A file-key change empties the map until `describe` answers (`:371`). `set.whens > 0` stores no alias. An inherited or file-moved row whose one name is the full param links it. `unread` is stripped before the row is sent. Callers that do not return `{publish}` still publish the raw report.

An icon param still gets no alias. That matches the tool: nothing writes an icon (`apps/design-tool/app.contract:325`).

## Smaller

**Two params in one file can share one custom property, and compound text can then name the wrong one.** `A.b-c` and `A-b.c` in the same file both dash to `--x-param-f<hash>-A-b-c`. modes.rs keeps the first and suffixes the second with `-{index}` (`host/web-js/src/modes.rs:123-128`). The join's key is only the unsuffixed `var(--x-…)` (`scripts/tamber-inspect.mjs:132`), and the css map keeps one of them (`:142`). In a gradient or shadow, the suffixed `var()` becomes the words "a component param", and the unsuffixed one can be rewritten to the other param's full name. `named` also drops the unread flag when it is set only while rewriting `text` (`scripts/tamber-inspect.mjs:161`). `token.names` still comes from the dotted intermediate name via the page's property table, so the rung and the link stay on the right param, and the hash does not leak. A direct row is unaffected: its text is the dotted name, or a single `var()` already replaced with that name.

**The server test does not lock the failure that round 2 reproduced.** `scripts/tamber-server.test.mjs:65-120` fails the first `describe`, then succeeds, then changes the plan while every answer is still `ink`. It does not fail a `describe` after a populated map, and it does not change the alias between P1 and P2. The code path above does both; the test would not catch a regression that cleared only `params.key`.

**The selection comment is stale.** `scripts/tamber-server.mjs:219-220` still says the map is kept across a plan read. The tokens are cleared; the comment above `learnParams` (`:365-366`) is the one that matches.

`~/x2voice/SIGNOFFS.md` is not on this machine. The checks above are against D6/D7 and the T3 gate in `llp/1122-design-tool.rfc.md`, the stage 7 naming rule (`param.f` + eight hex digits, full name on anything an author reads), and the round 1 and round 2 conditions. I did not run the suites.
