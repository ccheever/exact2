# Code review: LLP 1122 T1 commit 7 (T1a, import review), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f42acb24d` (commit `cd635dade`, with later commits' fixes).
- **Method:** one brief (sha256 `4c3b030489ffa8aa55ed3101d8750497f2216f6f7b85b3b53440cbeeda99d0f0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (9 medium).
- **Disposition:** taken but 5 and 9 in part: an edit carries what was ticked and unticked to the new proposal (and a recomputed import's kept offers stay on); a stylesheet import takes one file (the picker allows several only for DTCG, and the tool refuses more); the review lists each operation by its kind (a token added or set, an axis added, a mode's value dropped); every mode's value is shown and editable; an import with nothing to propose shows its unmapped report and count; a recomputed import shows the notice and its diff; an edit goes to the run that holds the proposal, and a failed drop is said. 5 in part: a value that is a reference to a token is said, unpainted, not resolved. 9 in part: the real-server test edits a value and leaves a token out before accepting, and checks the held proposal, the partial accept writing nothing, the bytes written and undone, and Figma's axis row and mode values; rendered checkbox states are not inspected.

---

Reviewed `cd635dade` against HEAD `f42acb24d`. This was read-only; tests were not run.

1. **Medium — Editing resets the chosen items.** [app.ts:938](/tmp/rv-dt-7/apps/design-tool/app.ts:938)  
   Editing creates a new proposal, but checkbox choices remain keyed to the old proposal ID. Uncheck a token, tick AccentColor, then edit another value: the excluded token becomes included again and AccentColor becomes unchecked. Editing a recomputed proposal also loses its `replaces` state. **Fix:** transfer the explicit inclusion choices to the replacement proposal.

2. **Medium — Selecting several CSS files silently imports only one.** [app.contract:778](/tmp/rv-dt-7/apps/design-tool/app.contract:778), [tamber_import.rs:112](/tmp/rv-dt-7/contract/cli/src/tamber_import.rs:112)  
   The picker permits multiple files for every format, and the client sends them all. The server sorts them, then shadcn and Tailwind read only `files.first()`. Selecting two sheets therefore produces a successful review and count check that omit the second sheet entirely. **Fix:** allow multiple selection only for DTCG and reject extra CSS files server-side, or explicitly support all selected sheets.

3. **Medium — The review mistakes operations for tokens and loses their meaning.** [app.ts:483](/tmp/rv-dt-7/apps/design-tool/app.ts:483)  
   `review.tokens` counts operations, including axis additions and conditional updates/removals. Treating every operation as `{name, value, whens}` shows the Figma fixture’s axis as an empty-valued token. On re-import, `set-token` with `when` appears as a default-value edit, while `unset-token-when` appears as another empty value. **Fix:** distinguish operation kinds, preserve conditions, and explicitly describe axis changes and branch removals.

4. **Medium — Non-default mode values cannot be read or edited.** [app.contract:729](/tmp/rv-dt-7/apps/design-tool/app.contract:729)  
   Each additional mode renders swatches and a label, but its value is never displayed. The sole input selects `"default"`, and `editImport` only changes `op.value`. A themed shadcn import or Figma import therefore cannot edit an imported `when` value before acceptance. **Fix:** render an editable value for each mode and pass its branch identity through to the edit operation.

5. **Medium — Alias tokens produce invalid swatch colours.** [app.ts:485](/tmp/rv-dt-7/apps/design-tool/app.ts:485)  
   Swatches receive the authored expression directly. The Figma fixture preserves references such as `dtcg.color.gray.s900`; Tailwind’s project fixture likewise preserves `tw.brand.s600`. These are Tamber references, not CSS colours, so the review cannot show their actual colours. **Fix:** obtain resolved colours for each proposed mode from Tamber’s evaluator while retaining the authored expression for editing.

6. **Medium — Imports with no operations lose their entire unmapped report.** [app.ts:917](/tmp/rv-dt-7/apps/design-tool/app.ts:917)  
   The server returns `review` even when `proposal` is null, but the client discards it. Importing a sheet containing only `--radius: 0.5rem` shows “nothing to propose” instead of the unsupported-value report and count. Re-importing an unchanged theme also hides its remaining unmapped values. **Fix:** display the returned review independently of whether there are operations to accept.

7. **Medium — Recomputed imports hide the required diff.** [app.contract:719](/tmp/rv-dt-7/apps/design-tool/app.contract:719)  
   The replacement notice and `h.diff` exist only in the non-import branch. If another client changes an existing token before acceptance, the server correctly recomputes the import, but the UI’s instruction to “review its diff” points to nothing visible. This misses LLP 1122 D10’s review requirement. **Fix:** show the replacement notice and recomputed diff for imports too.

8. **Medium — Editing a held import can cross server sessions.** [app.ts:942](/tmp/rv-dt-7/apps/design-tool/app.ts:942)  
   Unlike `acceptHeld`, the edit’s `propose` and `drop` calls are not bound to `heldToken`. If another project takes port 7740 before the next poll, editing the stale card can renew authentication and recreate the old project’s proposal on the new server. **Fix:** capture and bind both calls to the proposal’s server token, check the session, and handle a failed drop.

9. **Medium — The tests do not establish several claimed review behaviours.** [app.test.contract:357](/tmp/rv-dt-7/apps/design-tool/app.test.contract:357), [design-tool.test.mjs:601](/tmp/rv-dt-7/scripts/design-tool.test.mjs:601)  
   The edit test checks a success message and replacement ID; removing the actual value assignment would still satisfy those assertions. Tailwind/Figma checks inspect resource counts, so missing mode values and invalid swatches escape. **Fix:** assert the replacement’s actual value and accepted bytes, inspect rendered review rows and checkbox states, exercise ticking then unticking, and assert the first partial accept leaves bytes unchanged before the final acceptance and single undo.

Verdict: request changes; the T1a review is not yet complete.
