# Code review: LLP 1122 T7b (several live copies in the design tool) and the iframe's `name`, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `aaaa9d32d1`.
- **Method:** one brief (sha256 `f20c20063b0ffce52e5ba1f12b7a1565d2d7023390565c958a665aac05670e16`), the plan inline, shared by both reviewers, each blind to the other's review; commits d1898dde89 and aaaa9d32d1. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (6 SHOULD-FIX, 4 NIT).
- **Disposition:** all taken in the round's fix commit (named in round 2's records), with grid.ts rewritten as one reconciling pass at a time:
  - States' holds are given back on the grid (1), and the grid's pins with no selection (2).
  - Mirror off sends nothing; a failed navigate is not recorded (3).
  - No skipped send counts as done, and a give-back is forgotten only once acknowledged (4).
  - `copies` is tied to each tile's page and dropped with it (5).
  - No hold without matching plan digests, and a mismatch gives back a hold already sent (6).
  - Tests: the reload fallback's re-registration, holds gone on the grid, Mirror off (7).
  - Two copies moved in one pass: the last in the list leads, by design. Mirror navigation has one place (8).
  - The compiler test now refuses `name` on several tags (9).
  - The pass each second is cheap when idle, and passes no longer overlap (10).

Nothing blocks. A tile binds only to a `message` whose `origin` is the copies' origin and whose `source` is this page's own `exact-copy-<n>` child. A popup, a frame inside a copy, another origin, and the unnamed single canvas frame do not match. Below write, `loadGrid` sends no pin or hold. The tool leaving still clears that client's pins and holds in the server's existing client-gone path. Remote copies load the gateway page only after `#g=` is gone, and the entry frame is the unnamed one. `name` is now legal on `iframe` and still refused on every other non-input tag in that check; radios keep their own path.

## Findings

1. **SHOULD-FIX** — Holds from States stay on builds the grid still shows. `apps/design-tool/grid.ts:121-125` and `apps/design-tool/grid.ts:148-155`. Hand-back runs only for a build id that is no longer in `mine`. Switching from States to Grid of modes keeps `exact-copy-1`…`4`, so those ids stay in `mine` and the hold block never runs on the grid. Open States, let exact-copy-2 hold pressed, then choose Grid of modes. That tile is still pressed (the caption keeps "holding pressed") until One copy, which does call `handBack`.

2. **SHOULD-FIX** — States with nothing snapshotted returns before any hand-back. `apps/design-tool/grid.ts:109`. Pin six copies on the grid, then choose States before a selection exists (or while the only selection is already stale, so `shown` is still null). `loadGrid` returns the "Select a view…" note and empty tiles, and the six pins stay on the server. The iframes unmount, so those builds are no longer on the canvas. One copy and a later States pass that does get a snapshot do hand the extras back; sitting on that prompt does not.

3. **SHOULD-FIX** — Mirror-off still navigates, and a failed navigate is recorded as done. `apps/design-tool/grid.ts:127-136` and `apps/design-tool/grid.ts:157-164`. `place` is sticky, and the navigate loop does not read `mirror`. `sentTo` is set before `send` returns. Uncheck Mirror after any copy has moved, or leave States (which sets `place` to the selection's screen) for the grid with Mirror off. A copy that is still elsewhere, including one that then comes back to `/`, is sent `navigate` to that old place. A navigate that fails for a reason other than `unsupported`, or one skipped because that build's command is already in flight, leaves `sentTo` set, so later polls do not retry and do not reload the frame.

4. **SHOULD-FIX** — An in-flight command is treated as success, and hand-back forgets the record first. `apps/design-tool/grid.ts:78-87` and `apps/design-tool/grid.ts:84-87` and `apps/design-tool/grid.ts:122-124`. `send` returns `''` when `sending` already has that build, and every caller treats `''` as applied. `pinned` / `holding` are deleted before the unpin or release is known to have landed. Cap is 6 and the pin for copy-6 is in flight; set the cap to 4 so a second `loadGrid` starts (the resource re-runs while the first call is awaiting `call`). The second pass deletes copy-6 from `pinned` and skips the unpin. The first pass's pin then lands, and no later poll retries the hand-back.

5. **SHOULD-FIX** — `copies` is never dropped. `apps/design-tool/grid.ts:34-44`, `apps/design-tool/grid.ts:91-92`, and `apps/design-tool/grid.ts:119`. One copy clears `origin` and the navigation maps, not `copies`. A frame that loads again under a new id (sessionStorage unavailable, or the other tab answered `taken` on `exact.dev.build`) stays mapped to the old id until the new document posts. While the server still lists the old id, each pass pins that build, which is no longer the document in the frame. The same window exists when `gridPage` changes origin: the first polls pin the previous origin's builds.

6. **SHOULD-FIX** — The plan check does not run when either digest is missing, and a mismatch does not drop a hold already sent. `apps/design-tool/grid.ts:150-154`. The guard is `b.planDigest && e.planDigest &&` they differ. A states copy listed with `planDigest` null is held at the selection's site anyway. When the digest arrives and differs, the note says to reload, and the `if` does not fall through to `hold` with `state: null`, so the hold stays on the other plan's site.

7. **NIT** — The two contract tests lock the end state and miss the claims in their titles. `apps/design-tool/app.test.contract:1910-1968` and `apps/design-tool/app.test.contract:1970-2028`. After four seconds, copy-1 is pinned light and copy-3 dim, which is the right final pairing. Nothing asserts that, during exact-copy-1's 1.5s delay, that build had no pin and no other tile's pin. The move checks locations on copy-1 and copy-6 and the reload sentence on copy-3. It never reads `/__stub/navs`, never checks the reloaded frame's `src`, and the stub only serves `/` (`apps/design-tool/stub/tamber-stub.ts:578-579`), so `/post/3` cannot load. The states test's "holding pressed" text is the state word only (`grid.ts:27`); a hold of the right state on the wrong site or instance still passes. The freeze against copy-5's pin being handed back is what the later note assertion actually covers. A second selection and a digest mismatch are not exercised.

8. **NIT** — Two copies moving in one pass: the later tile in the list becomes the leader and the earlier move is navigated away. `apps/design-tool/grid.ts:128-132`. Copy 2 goes to `/a` and copy 6 goes to `/b` before the next poll. `place` ends as `/b`, and copy 2 is sent there.

9. **NIT** — The new compiler test only rejects `name` on `text`. `contract/cli/tests/it/vocab.rs:1191-1196`. `contract/lower/src/controls.rs:129-149` exempts `iframe` alone inside the non-input check; `select` still refuses `name` earlier, and a non-radio `input` still refuses it later. The test's comment says every other tag but a radio still refuses it; one `text` refusal would stay green if that condition widened.

10. **NIT** — While the grid section is up, `watchGrid` runs `loadGrid` every second (`apps/design-tool/app.contract:889-890`), and `changes.answer` runs it again on every builds event. An idle pass does not write. The cost is a host call per second plus a writing resource that can overlap itself, which is the race in finding 4. The "Select a view…" prompt polls the same way.
