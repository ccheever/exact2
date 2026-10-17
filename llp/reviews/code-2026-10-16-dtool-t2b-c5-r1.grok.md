# Code review: LLP 1122 T2b step 5 (the join, selection, outline), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `2a10281de`.
- **Method:** one brief (sha256 `5c2b96e05920b6e9d5c51fc0b6e29bfc700a63d49dc140c9590332dcefa4b614`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (8 medium, 1 low).
- **Disposition:** taken but 3 (astra) and Grok's "every build is sent one plan's sites" in part: the token rung fails closed (only for a row the node itself sets, `authored` and `own` in the map, whose whole value is one token; an expression over tokens, a row the map does not place, a conditional class, a style's or the tag's row are read-only with their reasons, and every non-editable row still links its token, a boundary's included); an inherited row now carries the token its ancestor's own row read (the runner's `layout` answer), so it links; the design file is checked too, unless the build runs the design the dev loop serves now (`designServed`); a stale selection asks its build again once per status, its status first (`refresh: true, status: true`); `outline` carries the source locations as well as the sites, so a page on another plan finds its own sites in its generation's map, and a frame that is none of its own leaves its outlines; the page reads every view's site in one call (a new `sites` agent op) instead of a node explanation per view, scrolls before measuring, keeps the outlines on their views as it scrolls and resizes, and moves them with the chrome; `outline` needs write scope and is journaled; the outline frame's locations are trimmed to file, line and column. Tests: the fail-closed cases, the design's currency, a stale selection asked again once per status through `tamberServer` itself, the inherited row's link on the real page. In part: the server still keeps one map (the current generation's, else the resident compiler's beside the plan); a build on another plan resolves its own sites from its generation's map, and a selection from a plan with no map here is read-only, said so.

---

## Medium

**A mode-boundary row publishes no token link** — `scripts/tamber-inspect.mjs:48`

`editable` keeps a token name only when `row.token.implied` is false, then the boundary branch sets `link` to that name. An own implied row still carries `token.names` (the runner attaches `token` for every own row, `runner/src/agent.rs:555`, and `row_token` returns the names with `implied`, `runner/src/runner/mode.rs:222`). The link is therefore always null. The unit test only asserts `rung: null` (`scripts/tamber-inspect.test.mjs:56`).

Click the node that sets a mode-boundary color. `selection.rows` for that row has `editable.rung: null` and `editable.link: null`, so the inspector cannot open the token D5 says a boundary row links to.

Take `names[0]` whether or not the row is implied. Use `implied` only to refuse the token rung, and set `link` to that name. Assert `link` in the implied case.

**An inherited row on the real page has no link either** — `scripts/tamber-inspect.mjs:49`

The inherited branch does put `link: token`, but the page never sends a token on that row. `token_json` runs only when the row is the node's own (`runner/src/agent.rs:555-556`); an inherited row is `source` and `from` only (`runner/src/agent.rs:562-565`). `selectedReport` forwards those rows unchanged (`host/web/dev.js:473`). The join does not look up the ancestor. The unit fixture invents a token the page does not send (`scripts/tamber-inspect.test.mjs:29`), and the wasm test only checks that every row has a rung or some reason string (`scripts/tamber-server.test.mjs:447`).

Click a text node whose color is inherited from a token. That selection row is `source: "inherited"` with no `token` and no `link`.

Have the runner include the ancestor's token on inherited rows, and assert a link from a report shaped like the page's. A dynamic row is fine: it is own, so its token is already on the report.

**The token rung is offered when the map does not name the row** — `scripts/tamber-inspect.mjs:56`

After the class and tag checks, any non-implied token becomes `{rung: "token"}`. `join` passes `origin: null` when `node.bindings` has no entry for that row (`scripts/tamber-inspect.mjs:74`). D5's fail-closed rule is that an authored row the map does not name is read-only. `origin === "own"` is never required.

A node reports an authored color whose theme index names a token, and the map has no binding for that row (a compiler-derived row, or a row name the bindings do not use). The selection offers `{rung: "token"}`.

Require `origin === "own"` before the token rung. Any other origin, including null, stays read-only with a reason that the map does not name the row.

**The design file is never part of the moved-file check** — `scripts/tamber-inspect.mjs:68`

`moved` runs on `node.file` and the chain files only. The design file is a source (`contract/cli/src/sources.rs:206-208`) and is the file a token edit writes, and it is not a view node and not a call site, so it is never hashed. `exact:` modules correctly skip the disk check (`scripts/tamber-inspect.mjs:40`): they are compiled in and have no path to move. Currency compares the applied design digest, which stays the running design until the compiler republishes.

Replace `app.tamber` on disk before the next plan. The page's plan, design digest, and pin revision still match, `readOnly` stays null, and an own token row is still `{rung: "token"}`.

When a row would be offered the token rung, run `moved` on the design entry in `map.sources` and fold a mismatch into `readOnly`.

**Outline drops existing marks, then draws boxes at the pre-scroll position** — `host/web/dev.js:607`

`outlineSites` calls `unmark()` first. The first view whose `planDigest` differs from `frame.plan` then `return`s (`host/web/dev.js:615`), so a frame for another plan clears the selection outlines and draws none. Marks that do get drawn are `position: fixed` from `getBoundingClientRect`, and only then does `found[0]` scroll (`host/web/dev.js:618-625`). Nothing listens for scroll or resize. `chrome()` (`host/web/dev.js:553`) also removes the banner without `unmark()`, and every `pointermove` in inspect mode calls `chrome()` (`host/web/dev.js:534`), so a dialog that takes the chrome leaves the dashed boxes on the old parent.

A second build, or this page still on the previous generation, receives `outline` for the current map: its dashed boxes vanish. Separately, select a copy below the fold: the box stays at the old viewport coordinates while the element scrolls away. Open a dialog afterward and the boxes stay on the previous parent.

If the frame's plan is not this page's, return before `unmark()`. Scroll the first match, then measure, and reposition on scroll and resize. Retarget or clear marks inside `chrome()`.

**Outline walks every view with a full node read** — `host/web/dev.js:611`

For each connected view the page calls `ask({op: "node", id, plan: true})`, which returns that node's style, props, and frames, on the main thread, and it does this for every outline frame. The runner already has the smaller read: `ask({op: "tree", plan: true})` returns each node's `id` and `site` plus one `planDigest` (`runner/src/agent/tree.rs:78-81` and `runner/src/agent/tree.rs:108-110`). One plan per page makes the per-view digest check redundant once that single digest is known.

A selection whose node is instantiated many times (an `each` body, a component expanded on a long screen) blocks the page once per view per outline.

One tree read, then match `site` to `exact.views`. If that `planDigest` differs, return before removing marks.

**Every build is sent one plan's site indexes** — `scripts/tamber-server.mjs:193` and `scripts/tamber-server.mjs:528`

Both the selection hook and `outline()` compute sites from the single `sourceMap()` (the current generation's map, or `dist/app.plan.map.json` when the generation has none, `host/web/dev.mjs:359`) and `channel.outline` sends that same list and the same digest to every inspect-capable build (`scripts/tamber-builds.mjs:323-326`). The build record stores the generation id, not the plan digest, and there is no map kept per digest. D4 and D5 say `outline` carries sites in the receiving build's plan, computed through that build's map (`llp/1122-design-tool.rfc.md:331` and `llp/1122-design-tool.rfc.md:381`). The protocol text added here describes the narrower behavior (`docs/tamber-protocol.md:640-641` and `docs/tamber-protocol.md:657-659`). The client `outline` path also `JSON.parse`s the map with no `mapFor` check (`scripts/tamber-server.mjs:524-528`), so a stale disk map still produces site indexes labeled with its own digest. The selection join does use `mapFor` (`scripts/tamber-server.mjs:190`), so a digest mismatch there stays read-only rather than joining the wrong file.

Two pages on two generations: both are told the current map's site indexes. The page still on the previous plan ignores the frame and, with the bug above, clears its outlines. A `uses` span is translated only through the current map, ignoring the plan `uses` was compiled from.

Keep each admitted map keyed by plan digest. Give each build the sites from `mapFor` against the plan that build is running, and refuse the outline when that plan has no map.

**`outline` does not require write scope** — `scripts/tamber-server.mjs:491`

`inspect` returns 403 unless `scope === "write"` (`scripts/tamber-server.mjs:508-509`). Dispatch calls `outline(request)` without `scope`. D4 says commands need write scope and are journaled (`llp/1122-design-tool.rfc.md:350`). `outline` is also not logged.

A paired client whose `scopeOf` is not `write` posts `{method: "outline", client, at: [...]}` and every inspect build draws those sites.

Pass `scope` through and refuse with the same 403 as `inspect`. Journal the client with the command.

**A refused selection retries forever after a dropped status** — `host/web/dev.js:366` and `scripts/tamber-server.mjs:189`

On the steady path the check is the right three fields: the page's `plan` is the generation id, `design` is the applied design digest, and `revision` is the applied pin revision (`host/web/dev.js:364` and `host/web/dev.js:472`), compared with `build.plan`, `build.design`, and `build.status.revision` (`scripts/tamber-inspect.mjs:88-90`). A late report is one 409, then `refresh()` reports again under the new facts. There is no retry cap. `status()` sets `channel.told` before the POST, and `report()` swallows a failed POST (`host/web/dev.js:347`). `committed()` resends only when the facts differ from `told`, so a lost status is not retried. `refresh()` posts `selected` again and does not post `status`.

The status POST after a design or pin change times out. The server still holds the previous plan, design, or revision. The next click 409s, the page refreshes, the same envelope 409s, and that pair continues for as long as inspect mode stays on.

Set `channel.told` only after the status POST succeeds. Cap `inspect {refresh: true}`, and do not refresh when the report just refused already carries the page's current plan, design, and revision; resend `status` instead.

## Low

**A conditional class is described as template editing** — `scripts/tamber-inspect.mjs:53`

`origin.startsWith("class:")` catches `class:A/B` (the compiler's label for a ternary class, `contract/lower/src/class.rs`). The row is read-only, which is what D5 requires, and the reason says it comes with template editing in T5. D5 says a conditional `class:A/B` is refused (`llp/1122-design-tool.rfc.md:406`).

Say that a conditional class is refused, and keep the T5 wording for a single `class:<Name>`.

## Tests

`scripts/tamber-inspect.test.mjs` covers a fabricated inherited token, a boundary row's rung but not its link, and a stale report only through a reimplemented hook (`scripts/tamber-inspect.test.mjs:68-74`), not `tamberServer`'s `selected` callback. `scripts/tamber-server.test.mjs` waits for the log line `plan ready` (`scripts/tamber-server.test.mjs:412`), then checks a file line, `readOnly: null`, some reason string, and an outline count. It does not check links, a moved design file, a second build's sites, or that refresh stops. `contract/cli/tests/it/source_map.rs` checks the app, a child module, and the design digest only.

What holds: with a successful status, currency is generation id, applied design digest, and applied pin revision on both sides. `mapFor` refuses a map whose digest is not the page's `planDigest`, including another generation's map and a stale disk file. `exact:` paths skip the disk check, and the chain files are hashed.

Verdict: the join is safe for one current plan, and the selection links, the token rung's fail-closed checks, and outline (one shared map, no write scope, and a wipe-and-full-node search on the page) do not match D4–D6.
