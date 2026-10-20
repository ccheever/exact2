# Code review: LLP 1122 T2b step 5 (the join, selection, outline), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `fbe0bfd39` (commit `2a10281de` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `1509f1d858c0df96d9495cc851e9d8dc85bc3dbc32f35bba8c153cad601ea750`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium, 2 low).
- **Disposition:** taken in part, applied without re-review (round 3 was the last): the design file's digest that certifies a served design is now taken when the compiler's design line arrives (the file it compiled), not when a design is pushed, so a scrub's restore keeps it; while a scrub previews, the served design certifies nothing; a design file gone from where the server looks is the map's entry, whose failed read is a reason; a client's `outline` looks a span up only when there are spans (an empty one clears, map or none), and a design edit no longer makes the map stale (it moves no span); outlined views are kept outlined as they change size (a ResizeObserver: a font-size pin, a commit); the browser test waits for the boxes to equal the views'. Not taken: locations carrying their source digest (a `uses` answer from before a reload can still land on the reloaded plan's node at that line), outline ownership changing only where the frame was applied, the race of a save landing between a compile and its design line, and a map per running build (the accepted partial, said in round 2).

---

## High

**The live-design bypass hashes the file at push time, not the bytes the design was compiled from** — `host/web/dev.mjs:273`, consumed at `scripts/tamber-inspect.mjs:60`

`pushDesign` sets `designSource` with a new `readFileSync(designPath(...))` after the compiler has already answered. The comment says those are the bytes the design was made from. They are the file at the moment the line is handled. `designMoved` then returns no reason when `served.source === now` and `build.design === served.digest`.

A scrub makes this deterministic. `Session::scrub` (`host/web/src/dev.rs:306`) compiles the scrub text via `with_design_text` and writes nothing. The reply still goes through `pushDesign` (`host/web/dev.mjs:327`). Save a design the compiler refuses, so the file is no longer the map's source, then scrub. The preview's digest is what the page runs, and `designSource` is the hash of the refused file. The file matches neither the map nor the scrub text, and the token rung is offered anyway. The same window exists on an ordinary value edit: the compiler read one version, a later save lands before the design line is handled, and the bypass treats that later save as the source of the design already applied.

Record the SHA-256 of the text that compile actually used (the file bytes at the start of that compile, or the scrub text) on the design and scrub replies, and store that. A scrub preview must not satisfy the live-edit bypass.

## Medium

**Deleting the design file skips the check** — `scripts/tamber-server.mjs:112`, `scripts/tamber-inspect.mjs:53`

`designPath` returns null when the file is gone. `designMoved` returns `''` on a null path before it reads anything, so the "gone" branch at line 58 never runs for the path the server passes. The map still names that design, and the page is still running it.

Delete or rename `app.tamber` and select an own token row. Currency still matches, `moved` does not look at the design file, and the token rung stays open.

If the map names a design source and `designPath` is null or the file cannot be read, return the gone reason.

**Outline boxes are not remeasured after a pin or a live reflow** — `host/web/dev.js:614`, updates only at `host/web/dev.js:526` and `host/web/dev.js:546`

`place()` runs when the outline arrives, on a window `scroll` (capture), and on window `resize`. A `pin` frame and a design batch change layout without either event. Inspect also calls `preventDefault` on wheel and touch, so the user cannot scroll the boxes back into place.

Select a node so the dashed boxes are drawn, then pin a mode or a root font size that moves it. The boxes stay at the old coordinates until the window resizes.

Call `place()` after a pin and after a generation's layout settles, or observe the outlined elements.

## Low

**An empty outline cannot clear while any compiled file has changed** — `scripts/tamber-server.mjs:541`

`staleMap` runs before `sitesAt`, including for `at: []`. An empty list looks up no span. Select a node, edit `app.contract`, then send `outline {at: []}`. The request is 409 and the old boxes stay until inspect turns off or the client disconnects (`clear` does not consult `staleMap`).

Resolve sites only when `at` is non-empty, and always clear on an empty list.

**The tests that describe this join never run the dev loop's design source** — `scripts/tamber-inspect.test.mjs:65`, `scripts/tamber-inspect.test.mjs:93`, `scripts/tamber-server.test.mjs:47`

The "later save" case passes an honest `{digest, source}` straight into `join`. Nothing calls `pushDesign` or a scrub, which is why the bypass above is green. The test titled "outlined on every build" registers one build and its own `selected` hook, whose refresh omits `status: true`. The server test is also one build, with source `exact:app` (skipped by `moved`) and no design file. The wasm path checks the inherited "Swap pins" link and box geometry, not an own token rung, a second generation, a moved file, a scrub, or a deleted design.

Drive `designServed` from `pushDesign` (a scrub, and a save the compiler did not read) and assert the rung stays closed. Register two inspect-capable builds on the real server hook.

Currency compares the same three fields the page reports and the build record stores (generation id, design digest, pin revision). A digest mismatch refuses the map. The refresh is latched once per `[plan, design, revision]`, status first, and does not spin. `view_sites_json` is one pass over remembered views. The accepted one-map outline stands: a page whose plan digest differs leaves its outlines.

Request changes: the design-file bypass hashes a later read of the file, so a scrub or a save the compiler did not take still offers the token rung, and a deleted design file skips the check.
