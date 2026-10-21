# Code review: LLP 1122 T7b, the narrow verification round of a51522a217 and c840f90d17 (review2's 00:39Z ask), 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `c840f90d17`.
- **Method:** one brief (sha256 `6c0eab4d83eccf2fada56927910043062464a90cf9c7b8b45d2a6ec62b411892`): round 2's findings verbatim, then the same question. Shared by both reviewers, each blind to the other's review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** round 2's findings closed but B4 (not closed) and the recorded B7; 3 new SHOULD-FIX, 1 NIT.
- **Disposition:** small fixes, in the commit named in the sign-off note:
  - A copy that has gone somewhere isn't sent there again, and a send not answered by a new place within five passes (about 5 s) counts as gone, so a rewrite or a Back doesn't strand or loop it (B4).
  - The larger size takes 4 to 64 px (new 1).
  - Mirror off or on clears what each copy last said, so a send that lands after the toggle is a first report, not a lead (new 2).
  - In States a copy that went to the selection's screen counts, wherever the router put it, so its hold is sent (new 3).
  - A new run clears the reload fallbacks (NIT).
- **Not taken:** B7, the stub's holds and remap, as recorded in round 2.

I'll verify round 2's findings against the two commits only: the post-round commit and the app-note commit. Read-only — no tree changes, builds, or test runs.The review notes claim most items closed. I'll read the grid, wire, and server paths those findings name and check the claims against the two commits.Nothing blocks. The three round-2 blockers are closed or explicitly left as recorded; one should-fix is still open.

Reviewer A (astra), against `a51522a217` and `c840f90d17`:

1. **PARTLY** — `apps/design-tool/grid.ts:120-122`. A pin is kept only when the listing’s `pinRevision` is still the revision the pin reply returned (`scripts/tamber-builds.mjs:533-536` bumps it on every pin, including an equal value); a newer revision drops the duty and sends nothing, and give-back reads `builds()` at `grid.ts:117`. Recorded not taken, and still true: the server’s `pin` replaces with no revision condition, so a `pin: null` already in flight still clears another client’s pin, and a hold is still matched only by state and instance (`grid.ts:124-126`) with no hold revision.
2. **CLOSED** — `apps/design-tool/grid.ts:111` passes `handle` as the wire’s `bound` (`apps/design-tool/wire.ts:111-127` will not take a restarted server’s token). `grid.ts:133` clears `handle` and `owned` together when a non-empty epoch changes, so an old cleanup is not replayed onto the new run.
3. **CLOSED** — `apps/design-tool/grid.ts:200`. The hold is not asked, and `asked` is not set, until `status.location` equals the selection’s screen; navigation is sent later in the same pass (`grid.ts:212`).
4. **CLOSED** — `apps/design-tool/grid.ts:165` and `grid.ts:169`. `keepHold` is false unless the tile’s state is wanted and `planDigest` matches, so a mismatch calls give-back.
5. **CLOSED** — `apps/design-tool/grid.ts:142` bumps `chosen` whenever `sel.e` is a new object (the same condition that refreshes `shown`), and that generation is in the key at `grid.ts:197`. A failed send deletes the key and says “Not held” (`grid.ts:204`); a real let-go still asks for another selection (`grid.ts:206`).
6. **CLOSED** — `apps/design-tool/app.ts:1081` counts a later `unsupported`/`navigate`, and `apps/design-tool/grid.ts:209-210` drops `sentTo` and latches `reloaded` before the next pass points the iframe at that place (`grid.ts:159`). The stub emits that late event (`apps/design-tool/stub/tamber-stub.ts:259-262`); the grid test expects copy 5’s reload line (`apps/design-tool/app.test.contract:1985-1986`).
7. **CLOSED** — `apps/design-tool/grid.ts:134-135`. Any view change or Mirror toggle clears `sentTo` before the mirror loop, so a pending `/a` cannot suppress `/b` after Mirror is turned back on.
8. **NOT TAKEN** — `apps/design-tool/grid.ts:51-55` and `grid.ts:159-160`. Registration is still origin plus the frame’s current `src`; there is no document generation. Same as the post-round note.
9. **CLOSED** — `apps/design-tool/grid.ts:110` and `grid.ts:235-237`. Each pass has an ask number; after a newer `loadGrid`, the next `send` returns `superseded` and does not write. The command already on the wire still finishes.
10. **CLOSED** — `apps/design-tool/grid.ts:133` (`if (epoch && epoch !== run)`) and `apps/design-tool/app.ts:623` (a failed hello returns `epoch: ''`). An empty epoch does not clear duties; a later hello with the same epoch does not clear them either.
11. **PARTLY** — `apps/design-tool/app.test.contract:1985-1986` and `apps/design-tool/stub/tamber-stub.ts:259-262` cover a later navigate refusal only, as the note says. No second selection, digest mismatch, other client’s pin, remapped hold, or exact hold payload.
12. **CLOSED** — `contract/cli/tests/it/vocab.rs:1187-1190`. The comment now says a native button returns earlier and predates this. `contract/lower/src/controls.rs:78-86` is unchanged.

Reviewer B (grok):

1. **CLOSED** — `apps/design-tool/grid.ts:165`, `grid.ts:169`, and `grid.ts:124-127`. Off the selection’s plan the hold is given back. Ownership is state plus instance, not site, so a correspond that moves site 3 to site 5 still matches and `state: null` is sent; `scripts/tamber-builds.mjs:599-600` clears that build’s hold whichever site it is on.
2. **CLOSED** — `apps/design-tool/grid.ts:142`, `grid.ts:197`, and `grid.ts:204-206`. A new selection generation changes the key; a failed send deletes it and the caption is “Not held”, not “Let go”.
3. **CLOSED** — `apps/design-tool/grid.ts:177-180`. `place` changes only when `seen` already had a location for that build, so a first report, including a late `/`, does not lead.
4. **NOT CLOSED** — `apps/design-tool/grid.ts:178-179` and `grid.ts:212-214`. Any new location clears `sentTo`, but it does not become `place`. The same pass sees `loc !== place` with `sentTo` empty and sends the exact target again. On the next poll the rewritten path (or Back) is stable, so `loc === was`, `sentTo` stays, and that copy is stranded for the rest of the session — the same failure, one pass later.
5. **CLOSED** — `apps/design-tool/grid.ts:117` and `grid.ts:120-122`. Give-back reads the builds at that call, after earlier awaits in the loop, and a higher `pinRevision` drops the duty without `pin: null`. A pin that lands during this call’s own await is the server gap recorded under A1.
6. **CLOSED** — `apps/design-tool/grid.ts:133` and `apps/design-tool/app.ts:623`. Same as A10: a failed hello’s empty epoch is not a new run.
7. **PARTLY** — `apps/design-tool/stub/tamber-stub.ts:243-249` still stores a hold with no plan and does not replay a remap; the new assertion is only copy 5’s later refusal (`apps/design-tool/app.test.contract:1985-1986`). Recorded in part.
8. **NOT TAKEN** — `apps/design-tool/grid.ts:51-55` and `grid.ts:159`. Same stale same-origin `exact-build` window as A8.
9. **CLOSED** — `contract/cli/tests/it/vocab.rs:1187-1190`. The comment matches `contract/lower/src/controls.rs:78-86`.

`c840f90d17` only splits the app sentence onto `GridTile.app` (`apps/design-tool/grid.ts:228-229`, `apps/design-tool/app.contract:1583-1584`). With no warning, the facts stay on `grid-note` and the app line is `grid-app`, which is what the test at `app.test.contract:1969-1970` checks. It does not touch the round-2 control flow.

New issues:

- **SHOULD-FIX** — `apps/design-tool/grid.ts:65`. `large` of 59–64 used to add that text size (`<= 64`). It is now treated as no large size, so the large row disappears. The field allows 4–64 (`apps/design-tool/app.contract:1560`). The `+6` row is why 58 is the top of the middle size; the size the person typed still has to be kept.
- **SHOULD-FIX** — `apps/design-tool/grid.ts:134-135` and `grid.ts:177-180`. Mirror off or on clears `sentTo` but leaves `seen`. A navigate sent before the toggle can arrive afterward and, because `sentTo` no longer marks it, the mirror loop treats it as a user move. In tile order a later copy’s stale arrival overwrites `place`.
- **SHOULD-FIX** — `apps/design-tool/grid.ts:172` and `grid.ts:200`. In States, `place` is the selection’s exact path and the hold waits for that exact path, but acknowledgements run only in the grid mirror branch (`grid.ts:173`). A router rewrite leaves `sentTo` set and the location unequal, so the copy stays on “Going to the selection’s screen” and the hold is never sent.
- **NIT** — `apps/design-tool/grid.ts:133` clears `sentTo` and `refusals` on a new run and does not clear `reloaded`. `grid.ts:159` and `grid.ts:212` then keep that tile on the first fallback URL, including after a later leader move, until the tile is hidden.
