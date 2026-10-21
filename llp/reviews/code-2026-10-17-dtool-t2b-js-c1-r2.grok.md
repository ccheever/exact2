# Code review: LLP 1122 T2b, the JS target, step 1 (the page's plan digest and its map), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `244e62f8d`.
- **Method:** one brief (sha256 `3fc662431c107d854b2e38e5632dc46f3e33fcddc8d59f873e7e55a58ce428e4`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** approve (no findings).
- **Disposition:** nothing to take.

---

I'll review this commit read-only: the round-1 notes first, then whether the plan digest, source map, and `designServed` behavior hold for every build the JS dev loop serves.The commit touches the JS dev page, a test, and the round-1 notes. I'll read those notes and the map/digest path next, including the loop code this step depends on.Round 1 asked for three fixes. I'll read the current loop, the wasm `designServed` meaning, and how the map digest is named.Auto-compacting conversation (82% full)...
Conversation compacted.
The round-1 fixes are in the tree; I’m checking the served-plan path, `designServed`, and the test against the current line numbers before writing the verdict.The three round-1 fixes hold, and the plan the page reports is the plan the loop can answer a map for. There is no new High or Medium defect.

`exact.plan` is the digest the map names for every plan this loop serves. The compiler writes one `app.plan` and `map.json` of those same bytes (`host/web-js/src/main.rs:318-329`, `contract/cli/src/map.rs:19-20`). The dev build embeds SHA-256 of that file and copies the file and the map through unchanged (`host/web-js/build.mjs:400`, `692-696`). The loop does not pass `--plan` or `--production`. `keepPageMap` stores the map only when `map.digest` equals SHA-256 of the `dist/app.plan` just published (`host/web-js/dev.mjs:125-134`).

- A kept or fast value-only edit never publishes a plan. The stage is deleted (`dev.mjs:84`) and `exact.plan` stays the plan already in `pageMaps`.
- A failed build deletes the stage and leaves `dist`. The first build calls `keepPageMap()` whether it failed or not (`dev.mjs:134`), so a restart that fails still answers the last good page. A later failure does not drop that entry.
- A publish, including one that lands while a later edit was already requested (`dev.mjs:328-333`) and one that ends a scrub (`dev.mjs:372-375`), calls `keepPageMap` synchronously after `replaceBuild` and before the reload event. The exit handler does not yield in between, so a request cannot see the new page without the map.
- An unpublished plan is not kept. The only extra loss is the same eight-deep cap the wasm loop uses: a page still open on a plan older than the last eight is mapless until it reloads.

With no digest, `sourceMap` returns only the resident loop’s current plan (`dev.mjs:542`). A selection that names no digest is joined with no map and stays read-only (`scripts/tamber-server.mjs:218`). A web-js page that has registered a digest is not given the resident map in that fallback.

`designServed` matches the wasm loop (`host/web/dev.mjs:390`). It stays null for the plan’s own design, so the join uses the map. A fast edit sets it from the string passed to `designOf` (`dev.mjs:277-284`); a kept build sets it from the candidate map’s design hash, read before the stage is removed (`dev.mjs:83-84`, `355`). A publish clears it. While `scrubbed` is set it returns null, which drops the live certificate and does not by itself make the row editable. `designMoved` (`scripts/tamber-inspect.mjs:97-99`) still requires the file’s bytes to equal that source and the build’s design digest to equal it. A save during `designOf` or during a compile certifies the bytes that were compiled; the newer file fails closed. The same is true across a scrub and after a kept build: a moved file stays read-only, and a whitespace-only save whose token digest did not change stays editable.

The map bytes come from `dist` after the compiler, or from the resident pipe after `pullMap` checks the card. Checkpoints stay in memory. `saveTrace` writes `traces/`. No request body is stored as a map.

The new assertion is meaningful for what this step took. While the drive still has the page open it reads that build’s `planDigest`, then requires it to equal SHA-256 of `dist/app.plan` and the on-disk map’s `digest` (`scripts/design-tool.test.mjs:736-785`). A loop-level `designServed` check was explicitly not taken; editability on this loop is step 4.

No High, Medium, or Low findings.

Verdict: round-1 fixes are right and complete, and this step keeps the map of the plan the page runs without certifying a design the page did not take.

