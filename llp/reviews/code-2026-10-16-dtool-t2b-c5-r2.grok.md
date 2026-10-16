# Code review: LLP 1122 T2b step 5 (the join, selection, outline), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ba6bf276c` (commit `2a10281de` and its round-1 fixes).
- **Method:** one brief (sha256 `8cff50c2683fe78175e19ae3da7090713baf835a2d55be33f53b33cf6a35e2c0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium, 3 low).
- **Disposition:** taken but 4 (astra) and Grok's first two in part: a synthetic token (named by its whole expression) gets no rung and no link; a role's spelling for one token is that token; a read-only join keeps each row's link; the app's design is the one the server serves (not found by extension; a map that does not name it is a reason), and a changed design file passes only when the build runs the design the dev loop serves now and the file's bytes are the ones that design was made from (the dev loop records them when it takes a design), so a later save it kept the last good design over is read-only; a host never reads a source map (rules/DEFERRED.md): the page no longer fetches its generation's map, so an outline is drawn only by a build running the plan whose sites the server sent, and leaves the others' outlines as they are; an outline of no sites clears them, and `{clear}` does; outlines are their client's, cleared on every build when it stops being present (a selection's are its inspector's); a client's `outline` is refused while a file the running plan compiled has changed (a fresh span would land on another node); the e2e test now checks one box per view at the sites, on that view's box, and that an empty outline clears them; the channel test checks the outline's owner clearing it. In part: the server keeps one map, the running generation's (else the resident compiler's beside the plan); a build on another plan, or the first baked plan with no map, gets no outline and a read-only selection, said so.

---

## Medium

**A page that has only first-booted never records its generation's map, so another plan's outline cannot be resolved** — `host/web/dev.js:258`

`channel.map` is set only in `applied` (`host/web/dev.js:236`). The first boot fetches that generation, whose envelope already carries `sourceMap.url`, then `client.adopt` (`host/web/dev.js:261`) marks it committed and returns. `receive` of the same generation then returns before `applied` (`host/web/dev.js:169`). `ownSites` (`host/web/dev.js:620`) bails out while `channel.map` is null.

Load the dev page and select a node: the plan digests match, `frame.sites` is drawn, and the missing URL is invisible. Publish a later generation that this page has not applied, or that the host refused. The server's `sourceMap()` is that generation's map, and `outline` labels `plan` with its digest (`scripts/tamber-server.mjs:541`). This page's `views.planDigest` is still the booted plan, so it calls `ownSites`, gets null, and leaves the previous boxes up (`host/web/dev.js:631`). The URL was in the envelope this page already fetched. That is the whole of the accepted partial fix for D5's "each build's own sites" (D4 also says the server computes those sites and a host never reads a map).

In `provide`, after the fetch succeeds, set `channel.map` from `candidate.sourceMap?.url` the same way `applied` does. Resolve `at` on the server from the map retained for the plan that build's status names.

**`outline` looks up a fresh compile's spans in the running generation's map** — `scripts/tamber-server.mjs:535`

`uses` compiles the sources as they are now and returns those spans (`contract/cli/src/tamber_serve.rs:822`). `outline` parses `sourceMap()` — the current generation's `app.plan.map.json`, or `dist/app.plan.map.json` when the generation has none (`host/web/dev.mjs:359`) — with no `mapFor` check and no per-source digest check, then sends that map's site indexes and digest. The page draws `frame.sites` whenever its runner's `planDigest` equals that digest (`host/web/dev.js:631`).

Insert a line above a token use and click that token's uses before the generation republishes. `uses` returns the new line. `sitesAt` finds whatever node the running plan's map still has at that line. The page is still on that plan, so the digests match and it outlines that other node. A miss (`sites` empty) keeps the previous boxes, below.

Refuse the outline when any source digest in the map differs from the file `uses` just compiled, and look up spans only in the map of the plan the build is running.

**An outline that resolves to no sites leaves the previous boxes up** — `host/web/dev.js:633`

`outlineSites` returns without `unmark()` when `sites` is missing or empty. An unresolved foreign plan (`ownSites` returns null) should keep the current marks. A resolved empty list is this page's answer: D4's `outline` of no sites outlines no nodes.

Select a node, then outline a token whose locations are not in this plan (`sitesAt` returns `[]`, or `ownSites` finds none). The dashed boxes of the node you selected stay, and they read as the token's copies. The wasm test allows that: it waits until `[data-exact-outline]` count is at least `outlined.sites.length` (`scripts/tamber-server.test.mjs:485`), which the leftovers already satisfy.

`unmark()` when `sites` is an array of length 0. Keep the early return only for a non-array.

**An outline has no owning client, so disconnect does not clear it** — `scripts/tamber-builds.mjs:324`

`outline` broadcasts `{sites, plan, at}` and records no client. `clear` (`scripts/tamber-builds.mjs:334`) turns off that client's inspect mode and drops its pins. Marks disappear only because inspect-off calls `unmark`. D4 says a disconnect clears that client's outlines on every build.

Client A is inspecting. Client B sends `outline` for a token's uses, then its poll ends. A's inspect stays on, and B's boxes stay. Remember the issuing client on the outline frame, and on `clear` send that client an empty outline (or drop those frames) the same way its pins are dropped.

## Low

**A read-only join drops the token link** — `scripts/tamber-inspect.mjs:97`

When `readOnly` is set, the row's `editable` is replaced with `{rung: null, reason: readOnly}`. `editable()` had already computed `link`. D5 still wants that link on a row that is not an edit.

Change the design file and click before the page reloads. The selection is read-only, and the inherited color row has a reason and no `link`, so the inspector cannot open the token. Keep `link` from `editable(r, origin)` and null only the rung.

**A single token written as its role spelling is refused as an expression** — `scripts/tamber-inspect.mjs:77`

The token rung requires `token.text.trim() === names[0]`. The runner resolves a role spelling to the token's canonical name (`runner/src/runner/mode.rs:207`): a row whose text is `-exact-secondary-label` and whose theme entry is the bare key `secondary-label` reports `names: ["secondary-label"]` and `text: "-exact-secondary-label"`. The join then says "an expression over tokens". Contracts in `docs/patterns` and `docs/contract-for-humans.md` write that role spelling. The theme fixture writes the token's own name (`text.label`), so the wasm test never sees the split.

Treat one name as the token rung when the trimmed text equals that name or the role spelling the plan records for it.

**The tests cited for this step do not exercise the outline they describe** — `scripts/tamber-inspect.test.mjs:85`

The "every build" test installs its own `selected` hook: refresh without `status: true`, `outline` called with no `at` (the frame arrives as `at: []`), and one build. `scripts/tamber-server.test.mjs:45` does cover once-per-status through `tamberServer`, still on one build and an `exact:` module. The wasm test waits for the compiler log `plan ready` (`scripts/tamber-server.test.mjs:437`), then asserts outline count `>= 1` and `>= outlined.sites.length` (`scripts/tamber-server.test.mjs:477`, `:485`). It never checks which element was marked, that a second plan translated `at`, or that an empty outline cleared the marks.

Assert the mark count equals the connected copies of that site, and drive `outline` through `tamberServer` with two generations whose digests differ.

The currency check is the page's `plan` (generation id), `design`, and `revision` against `build.plan`, `build.design`, and `build.status.revision`. `planDigest` is admitted only by `mapFor`. The moved-file check hashes absolute paths as raw bytes, skips compiled-in `exact:` modules, and includes the design file unless the build is running the design the dev loop is serving. A second stale report under the same status key is refused with no further refresh. One dropped `status: true` POST leaves later clicks at 409 until that key changes; the handler already notes that, and the latch is set before the POST is known to have landed.

Verdict: request changes — the join's currency and token-rung rules hold, but a first-booted page cannot resolve another plan's outline, and `uses` spans are applied to the plan still on screen.
