# Code review: a held proposal's choice (tamber/1.18), round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c1878b9444`.
- **Method:** one brief (sha256 `bde369eb6793f6c73c0f4ed408926dbe78a71c0af6c2f1d5c5b31dd2c27c3bb9`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Rebasing can publish a choice nobody made.** [app.ts:1382](apps/design-tool/app.ts:1382) sends before removing obsolete flips. After another client replaces the base, it sends `new base XOR old flips`: for example, base `[1,2]` plus an obsolete flip of `1` publishes `[2]`. Clearing flips later cannot retract that observable choice. **Fix:** invalidate the old flips atomically with the rebase, before rendering, sending or accepting.

2. **A newer choice received during a pending choose is discarded.** [choice.ts:46](apps/design-tool/choice.ts:46) ignores updates while `pending.has(id)` and never revisits them. If A’s reply for revision 1 is delayed while B’s revision 2 reaches A’s poll, A retains revision 1 afterward. The poll has already consumed revision 2, so subsequent accepts can repeatedly return 409 without updating the card. **Fix:** retain the latest observed record and reconcile it against acknowledged own revisions when the request finishes.

3. **Undoing the first tick can leave the earlier selection saved—and accepted.** [choice.ts:76](apps/design-tool/choice.ts:76) considers only acknowledged sends. Untick an item, then recheck it before the first reply: `picked` becomes empty and `sentFor` remains false, so no restoring choose is queued. With the event arriving before the reply, nothing subsequently repairs this. Moreover, [app.ts:1349](apps/design-tool/app.ts:1349) omits `items` when everything is selected, allowing accept to take that earlier saved subset. **Fix:** track pending/touched cards immediately, queue their latest selection even with zero flips, and send explicit items for full selections too.

4. **Editing continues after the prerequisite choose fails.** [app.ts:676](apps/design-tool/app.ts:676) converts choose errors to `null`; `heldChoice.send` resolves successfully regardless. Consequently, [app.ts:1368](apps/design-tool/app.ts:1368) proceeds to `edit-held` after a failed request. If editing succeeds, the server carries its older choice and the tool discards local carry information, losing ticks while reporting success. **Fix:** return and check the choose result; preserve the pending selection and stop the edit on failure.

5. **Restart handling clears the new run’s base after displaying it.** [app.ts:429](apps/design-tool/app.ts:429) seeds choices and builds cards before the epoch-change branch calls `forget`, which now clears them at [app.ts:294](apps/design-tool/app.ts:294). If the replacement server already has chosen proposals, displayed checkboxes reflect those choices while subsequent operations use `startsChosen` defaults and revision 0. A tick before the next refresh can therefore send a different selection from the displayed one. **Fix:** reset the old run before ingesting the new snapshot.

6. **Partially chosen grouped offers disagree with their checkbox.** [app.ts:666](apps/design-tool/app.ts:666) displays an offer as unchecked unless every member is chosen, but [choice.ts:17](apps/design-tool/choice.ts:17) flips each member independently. For offer `[1,2]` and saved choice `[1]`, checking the box sends `[2]`, although the UI shows the whole offer selected. Such groups exist when an override also removes `when` branches. **Fix:** represent partial groups explicitly and make checking/unchecking set or clear every member.

**Non-blocking**

1. **The stub differs materially from 1.17.** [tamber-stub.ts:394](apps/design-tool/stub/tamber-stub.ts:394) neither emits an event nor advances the held revision. Its accept path ignores saved choices when `items` is absent, and recomputation spreads the old choice into renumbered operations. Mirror the server’s behavior, including clearing choice on recompute.

2. **The tests do not establish the race claims.** [app.test.contract:527](apps/design-tool/app.test.contract:527) settles before accepting; [design-tool-fields.test.mjs:207](scripts/design-tool-fields.test.mjs:207) opens the second page after the first finishes. Add controlled request/event barriers for both arrival orders, simultaneous pages, rapid reversal, choose failure and restart. Fixed settling sequences do not establish those orderings.

No scope, pinned-name or paired-door bypass found in the inspected paths. Both permitted test commands were blocked during setup by sandbox `EPERM`: temporary-directory creation and socket binding, respectively.
