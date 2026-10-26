# Code review: a re-held import keeps its role overrides (third pass #6), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `55f60f8531`.
- **Method:** one brief (sha256 `cfdb7ec7700080345086fe02578d13f7e53f503a589499e345813c5d6652eb4c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

`startsChosen` matches every held shape the server and the stub actually produce. A fresh import and a re-import have no `replaces`, no `editedBy`, and no `kept`, so the card starts with tokens in and overrides off. An accept recompute sets `replaces` and does not set `from` to that id (an edited one's `from` stays the pre-edit id, and `editedBy` is only copied), so everything still on the proposal starts chosen. An edit-held result is the one case with `editedBy`, `replaces`, and `review.from` all equal to the proposal it replaced, so it starts fresh. The older propose path sends the whole operation list with `kept: false` and no `replaces`, which is the same fresh start. Carrying the page's flipped ids onto that new id preserves a tick or a left-out token across a second edit, an edit of a recomputed import, and an accept-recompute of an edited one. A reload drops the page's choice on purpose: an edited card comes back with overrides off and tokens in, and a recomputed card comes back with only the operations that were accepted, all on.

The new tests are deterministic and check the claim they state. The stub cases fix the ids (`imp`, then `edited-1` after restart) and prove tick-then-edit, edit-then-tick, and left-unticked by the checkbox and by whether accept writes or recomputes. The real-server drive uses one page session, both orders, and compares the diff, the mapped values, the offer roles, and the choice; the write contains the ticked override and the edit, and not the left-out token or an unticked override. The reload reads `kept` and `on`, which do not depend on storage, so a reloaded page shows no override on.
