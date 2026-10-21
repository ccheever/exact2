# Code review: LLP 1122 T2b, the JS target, step 1 (the page's plan digest and its map), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `358c51bdb`.
- **Method:** one brief (sha256 `9a537e85e39b420ca91dbcde92db7f59c9f043cf343cd7cccfb2b440ceee32f2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (1 high, 1 medium, 1 low).
- **Disposition:** taken.
  - `designServed` is the wasm loop's now. It is the design the pages run after value-only edits since the plan they booted, or null for the plan's own design, which the join checks against the map. A plan publish clears it. Its source is the SHA-256 of the bytes compiled: the text a value-only edit passed to `designOf`, or, for a kept build, the design file's digest as the candidate's own map records it (`mapDesignSource`, read before the stage goes). The file is never hashed again afterwards.
  - The served page's map is kept at start whether the first build passed or not. With a failed build, dist still serves the last good page and its map.
  - The test reads the page's `planDigest` while the drive has it open (a poll beside the drive), and checks that the map names the same digest.
  - Not taken: a loop-level `designServed` test. Editability on the JS loop is step 4's test, end to end.

---

I'll review this commit as a read-only pass: first the diff and the plan-digest path through the JS dev loop, then whether every served build keeps the map the page actually runs.The diff is small, so the risk is in how the digest is produced and when the loop keeps or answers the map. I'll trace that against the wasm loop and the map contract.`exact.plan` is the digest the map names for every plan the loop actually publishes. `designServed` is not the wasm loop's check, and a failed first build can leave the running page with no map.

## High

`host/web-js/dev.mjs:529` (`designServed`), fed by `designSource` at `129`, `272`, `342`, and `361`.

`designServed` returns `{ digest: design.digest, source: designSource }` whenever a design exists and no scrub is previewing. `designSource` is a fresh hash of the design file at that moment, not the bytes that design was compiled from. On the wasm loop (`host/web/dev.mjs:390`) the same callback stays null until a live value edit is sent, and a new plan clears it.

The dev loop does not watch the app until after the first build (`407`). Save `app.tamber` while that build runs: the compiler baked the old file, the page boots that design and reports its digest, and `designSource` is the new file. `designMoved` (`scripts/tamber-inspect.mjs:98`) then treats every build still reporting that digest, including iOS and Mac, as having taken the new file. The token rung is offered, and a commit writes the file the page is not showing. The same pairing happens on a later publish (`361`) and on a kept build (`342`) when the file moves after the compiler reads it. The fast path (`266–272`) re-reads the file after `designOf` instead of hashing the text it passed in.

Fix: keep a live snapshot only, set it when a value edit is sent, and clear it on every plan publish (the reload path and the publish inside the `fastSeq` early return). Hash the bytes that were compiled (the string given to `designOf`; for a kept build, the file hash from before `build()`, accepted only if it is unchanged). Leave `designServed` null for a plan's own baked design.

## Medium

`host/web-js/dev.mjs:124`.

`keepPageMap()` runs only when the first build succeeds. A failed build deletes the stage and leaves `dist` in place (`72`), and the server still serves that tree with the dev client injected (`607–625`). After a restart whose first compile fails, the previous page comes up, reports `exact.plan`, and the map beside that plan (`dist/app.plan.map.json`) is never loaded. `pageMaps` stays empty until a later successful publish, so every selection on the plan the page is running is "no source map".

Later failures are fine: the last good map is already cached, and kept or value-only builds do not change the plan. Successful publishes (first build, reload at `362`, and a build that lands while another was requested at `317`) key the map by `sha256(dist/app.plan)` and store it only when the map names that digest.

Fix: call `keepPageMap()` after the first build whether or not it failed. An empty or map-less `dist` already no-ops in the `catch`.

## Low

`scripts/design-tool.test.mjs:770–772`.

The new assertion checks that the framed page's `planDigest` equals `sha256` of `dist/app.plan` on the initial build. It never reads `app.plan.map.json`, never asks the loop for that digest's map, and never touches `designServed`, a kept build, a failed restart, or a scrub. Dropping the map or certifying a stale file still passes.

Fix: expect the map's `digest` field to equal that `planDigest`, and add a loop-level check that `designServed` is null for a baked plan whose file has moved.

No security finding: the page map is read from `dist` after the build, and a resident map still arrives on the map pipe and is kept only after `pullMap` checks the card. No request body is taken as a map. No ordering finding: `keepPageMap` runs synchronously after `replaceBuild` and before the reload event, and the build's `exit` handler does not yield.

Verdict: the page's digest matches the map for every plan the loop publishes, but `designServed` will mark a stale design editable, and a failed first build serves a plan whose map it never kept.

