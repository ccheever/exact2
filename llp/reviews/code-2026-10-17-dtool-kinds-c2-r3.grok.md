# Code review: LLP 1122, stage 6 kinds in the tool, steps 3-4, round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, a throwaway worktree at `7342e01d0`.
- **Method:** one brief (sha256 `3990c87597229cfdf623e1cd32f1e0a17cb2ae0ccf951b5bde51d98b516f7405`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 2 medium, 2 low).
- **Disposition:** taken, in `025dad11f`:
  - Only this mode;
  - the stale draft for every typing path, which a stale draft keeps out of Keep too;
  - the cut's key includes its parts;
  - `normal` in the size's place;
  - the e2e's snapshot.
  - Not taken as suggested: a clear in each dial, add, rename and remove action. The fields read the draft only while it is this cell's token edit, and Only this mode now clears it, so there is no path back to the hidden texts.

I'll review the round 2 fixes against the earlier findings, read-only, starting with the review notes and the diff since `HEAD~2`.Round 2 notes are on `b4115c75b`, after the fix commit at HEAD. I'll read those findings and the diff they were meant to close.The round 2 notes name eight defects. I'll read the fix diff and the editor paths those notes called out.Round 2's same-value cut, blur-before-spread, `2px#000` refusal, empty unit, `16px / normal`, inset casing, and `inlineWhy` after the await are fixed. Two edits still leave the typed fields and `editValue` apart, and Apply or Keep writes the one the fields do not show.

**High** — `apps/design-tool/app.contract:763`. Only this mode puts a hidden field draft back on a different value. Change `space.edge` from `12px` to `16rem` in the amount and unit fields, then move the gap dial. `fieldDraft` (`app.contract:606`) hides that overlay while `editKind` is `set-dial`, and the fields show `12` and `px`. Only this mode is not already this cell's `set-token`, so it sets `editValue` to the branch value `12px` and `editKind` back to `set-token`, without clearing `partFor`. `partFor` still equals `partsKey`, so the amount and unit fields show `16` and `rem` again. Apply and Scrub live are enabled (`partsStale` is false) and write `12px`. The next unit edit composes `16rem` from the resurrected texts. The same return happens after Add, Rename, or Remove. Clear `partFor` and `rawFor` in `onlyThisMode` when it copies `branch.value`, and in `dialNumber`, `dialStepped`, `dialText`, `addToken`, `renameToken`, and `removeToken`.

**High** — `apps/design-tool/app.contract:614`. A value-field draft is not stale when the branch value moves. `cellText` (`app.contract:749`) stores the cut in `rawFor` and clears `partFor`. `partsStale` is only `partFor != ""` and `partFor != partsKey`, so it stays false. Type `20px` into the value field of `space.edge` (`12px`), then Undo a write that changes that token to `8px`. `rawFor` no longer matches `branchKey`, so the amount field shows `8` while the value field and `editValue` stay `20px`. Apply and Scrub live write `20px`. There is no stale banner. Treat `rawFor != ""` and `rawFor != branchKey` as `partsStale`, and while that holds keep `partsBase` on `rawBase` so the fields stay on the draft the value field shows until Cancel.

**Medium** — `apps/design-tool/app.contract:819`. Keep ignores `partsStale`. Start a scrub after editing the blur to `5px`, then Undo (or another client) so the token's committed value changes. The fields drop the overlay, the banner says the branch changed, and Apply is disabled, but Keep's guard is only `scrub.live`, `scrub.ready`, and not pending (`app.contract:1277`). Keep writes the scrubbed draft. Disable Keep on `partsStale`, the same way Apply and Scrub live are disabled.

**Medium** — `apps/design-tool/app.contract:603`. `partsKey` is the value string plus `typed.of`, but the cut depends on the kind map from `design.answer`. Edit the x field of `0px 1px 2px surface.inset` to `4px` while `surface.inset` is a colour. A reread that leaves that shadow text unchanged and makes `surface.inset` a length re-parses the word as a length, so the part list gains a spread and the old `partTexts` no longer line up with `typed.parts`. `partFor` still equals `partsKey`, `partsStale` is false, and Apply writes `4px 1px 2px surface.inset` against fields that show another part's text. Put the part keys in `partsKey` (`typed.parts.map(p => p.key)`). A same-value reread keeps the shape; a reclassification does not, and `partsStale` then blocks the write.

**Low** — `apps/design-tool/parts.ts:169`. `normal` and `oblique` are refused only in the first two words. `typeface.rs` refuses them at any word before the size, including after a style and a weight. `italic 600 normal` gets a size field of `normal`; Tamber rejects the value. `16px / normal` is fine. Keep refusing `normal` and `oblique` until the size word, not only while `i < 2`.

**Low** — `scripts/design-tool.test.mjs:1010`. `kindsOnTheJsLoop`'s `look` returns on the first sample when the design snapshot is missing (`frame && !snapshot`). The `16px` check is then skipped (`:1051`), so a frame digest change that is not the scrubbed length still passes. Wait for a snapshot whose digest equals the frame, as the earlier scrub test does, and always assert the snapshot text.

Verdict: request changes — Only this mode and a value-field edit after the committed value moves still show one thing in the fields and write another.
