# Code review: a held proposal's choice (tamber/1.18), round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `32fa11a433`.
- **Method:** one brief (sha256 `a42f2862fa74fe77d3404e02473bcfb28f3e052c00206dcd45aeadd6503c2338`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. `apps/design-tool/app.ts:1340` (edit at `apps/design-tool/app.ts:1358`; base moves in `apps/design-tool/choice.ts:49`) — After this page's `choose` answer lands, `settle` rebases immediately, but the checkboxes stay on the last `held` until `choicesAgain` runs `reheld` and that poll folds (`apps/design-tool/app.contract:314`, `apps/design-tool/app.ts:415`). In that interval `shownStale()` is true and `chosen()` already returns the other client's set (`choice.ts:76` ignores this page's flips), while `m.on` is still the old base (`apps/design-tool/review.contract:47`). `acceptImport` only refuses when `gen` changes during `flush`. A click that starts after settle sees the new `gen`, `flush()` is already `true`, and it sends that unseen set with the new `choiceAt`, so the server writes it (or recomputes it) instead of 409. `editImport` does not check `gen` or `shownStale` either: `send` records the other client's indices as this page's choice, then `edit-held` carries them. Fix: before reading `chosen()`, and again after the await, refuse accept and edit while `heldChoice.shownStale()` is true (per proposal is enough: remember the `gen` last built into `held`). Do not treat the post-rebase base as the card until `takeShown` rebuilds it.

**Non-blocking**

1. `apps/design-tool/choice.ts:20` — An offer checkbox is on only when every index is in the base (`app.ts:654`), but `chosenOf` still keeps a partial index. A `choose` of `[2]` on offer `[2,3]` shows the override off and Accept still sends `2`. The tool's own toggles are all-or-nothing; a partial record from another client is not. Show a mixed offer, or accept an offer only when it is fully in.
2. `apps/design-tool/app.ts:1363` — `edit-held` has no `choiceAt`. Another `choose` between this page's ack and the edit is what gets carried. The new card shows it before anything is written to the design; a matching `choiceAt` on `edit-held` would keep the edited proposal on the choice this page just recorded.
3. `scripts/design-tool-choice.test.mjs:71` — The unit test covers the state machine in scripted order, and the field drives finish one page before the next acts. Neither accepts while `shownStale()` is true and the rendered `on` flags are still the previous base, which is the gap above.

Scope matches `edit-held`: `propose` for one's own, `write` for another's (`scripts/tamber-pairing.mjs:83`), and a paired body client is replaced by the token before dispatch (`scripts/tamber-server.mjs:554`). The door test covers own, another's, a forged owner name, and a write client. No path there sets another client's choice without `write`.
