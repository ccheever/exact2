# Code review: a held proposal's choice, round 3 re-check 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `3a1aa2813d`.
- **Method:** one brief (sha256 `9aaf5d6cc72a8f4409608415f74f8b75fa29c47686dc2638ec8575358f7b9aa4`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The card's own `choiceGen` is what accept and edit compare. `fold` copies `heldChoice.gen` onto the held card in the same turn it builds that card's ticks (`app.ts:423-428`). Accept and edit pass that number through (`review.contract:60`, `review.contract:73`, `app.contract:922-927`). `inStep` compares it to the module's current generation (`choice.ts:87`). `takeShown` only clears the dirty bit (`choice.ts:82`); it no longer records the card as shown. A click on a card the page still has, after the module has rebuilt, fails the generation check and neither sends nor carries. The generation moves only when another client's choice replaces the base, and the proposal ids are `prop-${epoch}-…`, so a restart cannot make an old card's generation match a new base. The in-step check runs before the left-out check (`app.ts:1363-1365`), so a stale card is refused before `chosen` is consulted.

Edit closes the gap after `choose`. It records the selection the card showed, then refuses if the generation moved (`app.ts:1370-1371`). The `choiceAt` it sends is read in that same turn (`app.ts:1372`). `staleChoice` runs before `edited`, and `p.busy = true` is set before the first await (`tamber-edit-held.mjs:51-58`), so a `choose` cannot land between the check and the proposal going busy. Accept already did the same: `accepted` checks, then `proposals.delete` before any await (`tamber-server.mjs:1027-1034`). A mismatch is 409 and nothing is written. Omitting `choiceAt` skips the check (`tamber-held-choice.mjs:32`), which is the 1.18-compatible wire behavior for both accept and edit. This client sends it only when the server is 1.18 (`app.ts:1372`).
