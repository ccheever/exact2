# Code review: a held proposal's choice (tamber/1.18), round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `07a949d4db`.
- **Method:** one brief (sha256 `c303fcc2fbd6bc0135f0288c3925d45f0091dbb879cfc231863f52e156a1bb46`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Accept can change its selection while waiting.** [apps/design-tool/app.ts:1342](apps/design-tool/app.ts:1342): A presses Accept with `[0]` selected while its choose is pending. B’s `[0,1]` arrives before A’s answer. `flush` settles that rebase, then `chosen` returns `[0,1]` and accept sends B’s current `choiceAt`. The whole proposal can be written although A approved `[0]`. **Fix:** capture the selection and base generation at the action; after awaiting own acknowledgements, require another review if a foreign choice changed that base.

2. **Settling a rebase does not reliably update the visible card.** [apps/design-tool/app.ts:1381](apps/design-tool/app.ts:1381): when B’s record arrives during A’s choose, `fold` builds `held` using the old base. `settle` later changes the internal base, but `again` merely reopens polling with the already-consumed held revision. The real server omits `proposals` at that revision, so `fold` never rebuilds the checkbox values. Accept uses the new base while the card still shows the old one. The stub’s unconditional proposal list masks this. **Fix:** rebuild cards whenever their bases change, independently of server-list revisions, and synchronize flip removal with that update.

3. **Obsolete-flip removal can be lost when a resource answer is superseded.** [apps/design-tool/choice.ts:82](apps/design-tool/choice.ts:82): `drop` clears `rebased` immediately. If that `sendChoices` invocation waits on another card and newer arguments replace its resource ticket, its returned drop list is discarded. Calling `drop` again with the same `picked` returns `[]`, although those flips remain obsolete. The checkbox continues displaying a flip that `chosenOf` ignores. **Fix:** return every obsolete flip still present in `picked` until its removal is observed; test repeated calls and a superseded resource answer.

4. **An uncertain choose failure leaves an invalid deduplication cache.** [apps/design-tool/choice.ts:95](apps/design-tool/choice.ts:95): suppose selection A succeeded, then B reached the server but its response was lost. The failure leaves `sent=A`. Reverting to A therefore returns success without sending anything, leaving B recorded. An edit can also pass this false prerequisite and carry B. **Fix:** invalidate the cached selection when a subsequent request has an uncertain outcome; restoring A must actually send or verify A.

5. **A temporarily absent proposal reapplies its flips over its saved choice.** [apps/design-tool/choice.ts:68](apps/design-tool/choice.ts:68): accept removes a proposal while writing and can restore the same ID on failure. A poll during that interval deletes its base and own-send history, but leaves `picked`. On restoration, the saved choice becomes a new base beneath the old flips. For example, base `[0,1]` plus flip `1` saves `[0]`; restoration seeds `[0]`, and that same flip now produces—and can automatically save—`[0,1]`. **Fix:** preserve the base across temporary absence, or invalidate its associated flips atomically before reseeding.

**Non-blocking**

1. **Empty-choice acceptance still differs in the stub.** [apps/design-tool/stub/tamber-stub.ts:422](apps/design-tool/stub/tamber-stub.ts:422) turns an itemless accept of `choice: []` into a successful empty recompute and removes the original. The server returns 400 and retains it. Mirror that refusal.

2. **Partial groups remain visually ambiguous.** [apps/design-tool/app.ts:661](apps/design-tool/app.ts:661) shows offer `[2,3]` unchecked when only `2` is saved, while `chosenOf` still includes `2`. Group toggling is fixed, but the saved partial selection needs a mixed-state indication or an explicit operation listing.

3. **The tests still do not establish the integration race claims.** [apps/design-tool/app.test.contract:555](apps/design-tool/app.test.contract:555) settles before accepting; [scripts/design-tool-fields.test.mjs:210](scripts/design-tool-fields.test.mjs:210) finishes A’s ticks before opening B. The deferred unit tests inspect internal maps, not rendered cards or superseded resource answers. Add controlled request/event barriers for the scenarios above.

No scope, pinned-name, or paired-door bypass found. Both permitted test commands stopped before assertions with sandbox `EPERM`: temporary-directory creation and TLS-listener binding, respectively.
