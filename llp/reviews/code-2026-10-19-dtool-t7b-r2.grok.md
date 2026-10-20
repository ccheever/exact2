# Code review: LLP 1122 T7b (several live copies in the design tool), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `b5206c4066`.
- **Method:** one brief (sha256 `6c50ead7fb11129e435dba17c0dfb3b372e4938754ce75f1d1be090ef96f2ee6`), the plan and rulings inline, shared by both reviewers, each blind to the other's review; commits 07c386ffaf, 0ae425a70b and b5206c4066, with round 1's records. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (after the CLI's own compaction notice).
- **Verdict:** request changes (1 BLOCKER, 5 SHOULD-FIX, 3 NIT).
- **Disposition:** post-round, in the commit named in the sign-off request:
  - A hold is the grid's by its state and instance, whatever site the server moved it to. Off the selection's plan it is given back (1).
  - Each choice of a view asks again, and a failed send asks again and says "Not held" (2).
  - A copy's first report doesn't lead; only a move after it does (3).
  - A copy sent somewhere has arrived once it reports any new place, so a router's rewrite or a Back doesn't strand it (4).
  - Give-back reads the builds as they are then, and a pin's revision decides whose it is (5).
  - A failed hello isn't another run (6).
  - Tests: a later navigate refusal (7, in part).
- **Not taken:** a stale same-origin word (8): the same build id, as Astra's 8. The native button predates T7b; the comment says so (9).

The remaining open question is whether a reload fallback deep link is actually the app document. I'll confirm that in the dev server, then check the give-back and binding lines against the round-1 records.One blocker: a hold the server rewrites onto a corresponding site is never given back, and leaving States then forgets the duty. Round 1's serialization, same-plan give-back, Mirror off, navigation acknowledgements for an exact arrival, null `planDigest`, selection identity, frame binding, the gateway cap of 4, the F2 caption, the storage note, and `iframe` `name` are in place in this tree.

The reload fallback's `src` is that tile's own origin plus the location (`grid.ts:148`). A browser navigation to that path is the app document: the dev loop's `buildTreeFile` serves `index.html` for an HTML deep link (`host/web/serve.mjs:703`), and the gateway proxies that path (`scripts/tamber-canvas.mjs:105-114`, `528`).

1. **BLOCKER** — A hold already sent is kept across a plan change, and a remapped hold is then dropped without `state: null`. `apps/design-tool/grid.ts:156`, `grid.ts:179-182`, `grid.ts:116-118`. The server rewrites the same client's hold in `scripts/tamber-builds.mjs:254-272` and replays it on register at `tamber-builds.mjs:404`.

   Open States and hold Pressed on copy 2, site 3, plan D1. The grid's duty is `{"site":3,"instance":[],"state":"active"}`. Copy 2 reports plan D2 and the server's `correspond` returns site 5, so the live hold is now site 5. `keepHold` is true because that tile's state is `active`, and the mismatch branch only sets the caption "This copy runs another plan than the selection (or says none): reload it." Reloading the frame re-registers, and the server puts the hold back on site 5. Choose Grid of modes. `keepHold` is now false, the listed hold's site is 5, and the owned JSON still says site 3, so `giveBack` deletes the duty and sends nothing. Copy 2 stays pressed on the mode grid until this client disconnects. A correspond that returns no site is already cleared on the server; a correspond that keeps site 3 is released. The stuck case is a corresponding site with a different index.

2. **SHOULD-FIX** — "Select the view again" does not ask again, and a failed hold is labeled as a let-go. `apps/design-tool/grid.ts:181-188`. `asked` is cleared only when `epoch` changes (`grid.ts:124`).

   The key is build, site, instance, plan digest, and state. It is stored before `send` returns. Hold Pressed on a control the build lets go. The caption says to select the view again. Click that same view again: `shown` updates (`grid.ts:131`), the key does not, and the next pass takes the final branch and sends no hold. The same stick happens when the first `hold` fails on the wire: the next caption says "Let go" and a click on that view does not retry. Choosing a different view and then this one does send, because that replaces the key.

3. **SHOULD-FIX** — The first location a copy reports becomes the mirror leader, including a copy that registers late. `apps/design-tool/grid.ts:159-166`. `seen` starts empty and is cleared on an epoch change (`grid.ts:124`). `place` is kept across passes and is overwritten by that first report.

   Mirror is on by default (`app.contract:752`). Move copy 2 to `/post/3`. Before the next pass, exact-copy-1 finishes registering and reports `/` (the contract test's own 4s delay is this shape). It has no `seen` entry, so the loop sets `place` back to `/`, and the navigate loop sends copy 2 to `/`. The same loop runs when the grid first opens: the last copy to report wins, so one copy restoring `/settings` from its own storage pulls the others there. Two copies the person moves in one pass still resolve to the later tile; this is a copy that has never reported.

4. **SHOULD-FIX** — A navigate acknowledgement is only the exact location that was sent, and any other report leaves that send in place. `apps/design-tool/grid.ts:163-164` and `grid.ts:192-193`.

   Mirror sends copy 2 to `/post/3`, so `sentTo` is `/post/3`. The page lands on `/post/3/edit` (the router rewrites the path) or the person hits Back before the next poll. The reported location is not `/post/3`, so the ack does not clear `sentTo`, and `sentTo` being set means `/post/3/edit` is not a new leader. The navigate loop will not retry, because `sentTo` already equals `place`. Copy 2 stays off the others' location for the rest of the session. An arrival at exactly `/post/3` does clear `sentTo`. Mirror off still sets `place` to `''` (`grid.ts:167`) and sends nothing.

5. **SHOULD-FIX** — Give-back judges another client's pin from the snapshot taken before the pass's awaits, then `pin: null` deletes whatever the server has. `apps/design-tool/grid.ts:125` and `grid.ts:112-114`. `scripts/tamber-builds.mjs:525-536` replaces the current pin with no revision check.

   Tool A starts leaving the grid. Its snapshot still shows A's pin on copy 2. During the await of an earlier tile's `pin: null`, tool B pins copy 2. A's `giveBack` still sees a match with what A set, sends `pin: null`, and B's pin is gone. A replacement that is already in that snapshot is left alone, and a build that is absent from the list keeps the duty (`grid.ts:111`).

6. **SHOULD-FIX** — A failed hello is treated as another server run and drops give-back duties the real server still has. `apps/design-tool/app.ts:623`, `apps/design-tool/app.contract:897-898`, `apps/design-tool/grid.ts:124` and `grid.ts:174-176`.

   The grid has pinned six copies. The events poll fails, `advance` refreshes hello, and hello fails. That answer is a normal `Hello` with `epoch: ''`, so the next pass clears `owned` before `giveBack`. The pins still match the tiles, and a matching pin is not written back into `owned`. Hello then succeeds with the original epoch, which clears `owned` again. Switch to one copy. There is no duty left to release, and the six pins stay until this client disconnects. A real epoch from a restarted server is the case that should clear them: the new process no longer has those pins.

7. **SHOULD-FIX** — The new tests stay green over findings 1 and 2, and the stub cannot represent them. `apps/design-tool/app.test.contract:2062-2069`, `apps/design-tool/stub/tamber-stub.ts:243-246` and `tamber-stub.ts:587-589`.

   The states test expects the captions to lose the word "holding" after returning to the grid. The stub stores a hold with no plan and, on a second registration, keeps the pin and drops the hold. The real server replays a hold, including one it has remapped. Nothing asserts a second selection, a digest mismatch, or another client's pin. The grid test does prove, against this stub, the F2 sentence, Mirror off, cap 4, and that a `noNavigate` copy re-registers at `/post/3` because every path serves the copy page (`tamber-stub.ts:579-580`).

8. **NIT** — A same-origin `exact-build` posted after `src` changes and before the iframe navigates is recorded as the new page. `apps/design-tool/grid.ts:51-55` and `grid.ts:149`.

   Copy 3 cannot navigate, so the pass sets its `src` to `http://exact-copy-3.localhost:<port>/post/3` and deletes its registration. The document still in the frame posts again (the dev client's stream reconnects). The origin matches, and the listener stores that id under the new `src`. The next pass treats it as the reloaded page. With a stable id the pin hits the live build one pass early and the server replays it onto the reloaded page. A hidden tile and a real origin change do drop the registration, and a sibling frame on the gateway's shared origin does not match `frames[name]`.

9. **NIT** — A native button still accepts `name`, and the new test comment says otherwise. `contract/lower/src/controls.rs:78-86` returns before the iframe check at `controls.rs:129-150`. `contract/cli/tests/it/vocab.rs:1187-1198`.

   `button appearance="auto" name="x"` compiles. The comment on `an_iframe_takes_a_frame_name` says any tag but a radio is refused. The test now refuses `text`, `view`, `image`, and checkbox, and an `iframe name="exact-copy-1"` is in the plan. `select` and a non-radio `input` still refuse `name`.
