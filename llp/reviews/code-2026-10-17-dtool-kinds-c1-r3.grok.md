# Code review: LLP 1122, stage 6 kinds in the tool, steps 1-2, round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `c2e932f72`.
- **Method:** one brief (sha256 `115161407f192f12c36f9d796f98cc5f612dabeae5855a2ac0a4117235b5e061`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** approve (no findings).
- **Disposition:** taken (the last round). A box shadow's preview is a box inside a padded box of the column's scheme and ground, so the shadow falls on its own ground, not the tool's page. Not taken: a pixel assertion. The contract test checks the previews' nodes and text, and the gate checks them by eye.

---

I'll review this commit read-only: the two dial/grid fixes, whether they cover every write path and stage-6 kind, and whether the regenerated stub and tests actually prove them.Round 2's fixes are in the tree. Next I'll trace every dial write path and how each stage-6 `resolve` tuple is read.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is down to confirming the write paths and the grid against the code, then writing the round-3 report. I'll re-read the dial writer, the cell builder, and the checker before signing off.No High, Medium, or Low findings.

Both fixes in this commit are right and complete, and nothing new on the write paths or the grid fails the bar.

**Length dials write `Npx`.** `controlOf` sets `unit` to `px` only when the kind is `length` (`apps/design-tool/dials.ts:125`). The slider (`dialNumber`) and the stepper (`dialStepped`) append that unit once (`apps/design-tool/app.contract:604`, `:614`). A colour field and a select go through `dialText` and keep the text (`:632`). A number dial's unit is empty, so its slider and stepper still write a bare number, including negatives and decimals. A cleared or whitespace-only stepper stays `""` (`:614`); `proposeEdit` refuses an empty value (`apps/design-tool/app.ts:782`), and `releaseScrub` refuses an empty draft (`:843`), so it is never sent as `px`. Scrub frames and `dialReleased` send the already-formatted `editValue` (`app.contract:621`), so letting go of the slider does not strip the unit. A `rem`, `em`, or named length cannot be a dial value: `check` runs before `describe` (`contract/cli/src/tamber_serve.rs`), and `literalOf` drops those spellings from a length select (`dials.ts:89`), leaving them out of the options with the existing help. `0` and `Npx` stay and are written unchanged.

**A non-colour token loads.** For any kind other than `color`, the cell is built from `cellText` before any light/dark/srgb read (`app.ts:684`). A missing `tuples` array becomes no cells inside the cell shape (`:678`, `:690`). Length, number, shadow, and easing use the server's `value` spelling (`dials.ts:154`, `:158`); duration only inserts a space before `ms` (`:157`); type and family read the longhand fields the protocol writes (`:155–156`). Colour residuals are unchanged. Contrast on an empty `srgb` returns `not a colour` (`app.ts:1354`) and does not throw. The aria label is that cell text (`:693`). Pins compare `Stored.value`, which is `r.value` (`:612`). The branch editor gets the token's kind (`:765`); duration and easing offer no "Only this mode", and other non-colours name only the non-under-dark axes (`dials.ts` is not the branch — `app.ts:173–175`). Inspector links are still token name, column id, and place (`apps/design-tool/inspector.ts:68–69`). The import swatch paints only a cell with a residual or an `srgb` (`app.ts:516`). A shadow preview sets `color-scheme` from the column (`app.contract:1068–1071`).

**`stub/dials.json` matches the new design.** Compared by dial and token name with the parent, the only additions are `gap` and `inset`, the tokens `space.gap`, `space.inset`, and `shadow.card`, their explain/resolve entries, the two digests, and the contrast warning moving from line 32 to line 38 — the same `tone.warm` message, which is line 38 of `dials.tamber` after the comment wrap, the two dials, and the three tokens.

**What the tests lock.** The contract test types the Gap slider to `10` and the inset stepper to `6` and expects `10px` and `6px`, both gap cells as `8px`, and both shadow preview nodes (`apps/design-tool/app.test.contract:94–125`). The driver applies Gap against a real server and expects `gap: length = 10px`, and the `cellText` / `branchOf` cases match the resolve shapes (`scripts/design-tool.test.mjs`). These suites were not re-run in this review. Still untested, and not defects: an emptied stepper, a negative or decimal length, a length select, a scrub release, and the shadow preview's pixels.

Verdict: approve — the length-dial writes and the non-colour grid are correct for every stage 6 kind, and this commit adds no further defect.

