# Code review: LLP 1122, stage 6 kinds in the tool, steps 3-4, round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, a throwaway worktree at `056c17479`.
- **Method:** one brief (sha256 `60505da833a30f12c05271431589bec154647b939fdfe8381fc1873e58689f17`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 1 medium, 1 low).
- **Disposition:** taken, in `3a3e08e27`, as the Astra record of this round says.
  - The reread: the cut is keyed by the branch's value. A reread that gives the same value back keeps the fields and the draft, and one that moves it marks the draft stale.
  - Another edit replacing the draft resets the fields. The fields read the draft only while it is this cell's token edit, so no action needs its own clear.
  - A scrub cancelled is now tested.

I'll review this commit read-only: the diff, the round-1 findings, and the stage-6 grammars, then check whether field edits stay consistent with `editValue` and whether `proposeHere` is correct.HEAD is the kinds commit. Next I’ll read the round-1 reviews and the editor, parser, and propose paths those findings touched.Round 1 flagged typing that dropped fields, wrong shadow and type splits, and a stale cut. I’ll read the current editor and grammars against those cases.Round 1’s grammar and typing bugs are fixed, and `proposeHere` matches the checklist. A design reread still drops the typed-field overlay while the draft remains.

**High — A design reread puts the branch value back in the fields while `editValue` stays the draft.** `apps/design-tool/app.contract:594`

`branchKey` includes `branch.answer`, and `loadDesign` bumps that number on every read (`apps/design-tool/app.ts:666`). `partFor` is that key, so the overlay dies. `cancelScrub` (`app.contract:812`), `undo` (`app.contract:835`), and a failed Apply all run `written`, which refreshes the design and does not clear the draft. Cancel a scrub of `space.edge` after the amount field changed `12px` to `16px`: the amount field shows `12`, the value field and `editValue` stay `16px`, and Apply still writes `16px`. The next field edit composes from the reloaded cut (`typed.of == partsBase` passes, `fieldTexts` is the branch parts) and throws the draft away. Undo that changes the token does the same once `loadParts` settles.

Fix: key the cut and the overlay by token and column, not the design read. On `partEdit`, store that cut (`rawBase`/`rawFor`) so a later `branch.value` does not retarget it. Drop the overlay only when the draft is abandoned (the clears already at `app.contract:702`, `723`, `800`, and `829`).

**Medium — Switching away from the token edit leaves the overlay up.** `apps/design-tool/app.contract:646`

`dialNumber`, `dialStepped`, `dialText`, `addToken` (`764`), `renameToken` (`775`), and `removeToken` (`784`) set `editKind` and never clear `rawFor`/`partFor`. Change the amount from `12` to `16`, then move a dial: the amount field still shows `16`, the value field shows `12px` (`shownValue` is the branch value because `editKind` is no longer `set-token`), and `editValue` is the dial. Apply writes the dial. The next part edit, or Only this mode (it copies `branch.value` only when the open edit is not already `set-token`), composes `16rem` from the overlay and drops the dial.

Fix: clear `rawFor` and `partFor` in those actions, as `selectCell` does.

**Low — The tests never reload the design with a typed draft open.** `apps/design-tool/app.test.contract:129`

They type with `clock data` between edits, empty a required part, use Only this mode, and Keep a scrub. They do not cancel a scrub, undo, or fail Apply while a field overlay is up, so they cannot see the fields leave `editValue`. The join and one-line `set-token` checks in `scripts/design-tool.test.mjs:1089` and `:1142` do prove the two binding conditions.

Round 1 is fixed: an incomplete part keeps its cut, a required hole drafts nothing, `+`/exponent lengths and `600.0` land in the right field, `italic` only before a weight, `normal`/`oblique` and a text-shadow list get no fields, and a shadow that still names a token says “Names”, not “Pinned”. Nothing the server accepts is filed in the wrong part. `proposeHere` strips `texts` unless asked, leaves a held propose and the scrub session’s own `ask` alone, keeps the session class when `inline` fails, and runs only when the JS loop passes `inline` (the wasm loop does not). `inlineWhy` compares against `scrubbed ?? design`, which is what the pages are running, and only after `toolFresh()`.

Verdict: request changes — the typed draft and the fields diverge after any design reread that keeps the draft.
