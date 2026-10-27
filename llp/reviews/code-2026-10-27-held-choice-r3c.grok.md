# Code review: a held proposal's choice, round 3 re-check (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `f0ae7eacae`.
- **Method:** one brief (sha256 `51ab30a08751406f329c964b43feb479a3c39811ea23e8419f4e75da8fe27adc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `apps/design-tool/app.ts:1366` and `apps/design-tool/choice.ts:104`. `editImport` treats a successful `send` as proof the server still has the selection just computed, then `edit-held` carries whatever choice is on the proposal when that request is applied (`scripts/tamber-edit-held.mjs:25`, `scripts/tamber-held-choice.mjs:43`). Two cases skip that proof. If this page already sent the same indices, `send` returns true on the cached key and never talks to the server, so a choice another client has already recorded is what the edit carries. If this page's `choose` did just land, the other client can still choose before `edit-held` is processed, and the carry is theirs. Accept does not have this hole: it sends the indices and `choiceAt`, and the server answers 409 when that is no longer current. The card still shows the pre-edit ticks either way. Pass `heldChoice.at(proposal)` on `edit-held` after `send` returns, and reject the edit on the server when `choiceAt` is not the proposal's, the same rule as `accept`. Do not pass `items` for this; that narrows the held proposal to the chosen ops instead of carrying the choice on the whole import.

**Non-blocking**

1. `apps/design-tool/app.ts:1360`. The "tick it to edit" check runs on `chosen` before `inStep`. While a rebase is not on the card yet, a row the card still shows as on is reported as left out. The edit is not sent.
2. `apps/design-tool/choice.ts:25`. A partial override is removed even when one of its indices is also a mapped token row (`tokens` greater than that index). Real imports append override ops after the tokens, and renumbering keeps that, so the box and the selection stay aligned. A review that overlaps them, which the server will store, turns that token's checkbox on and then drops it from the selection.
