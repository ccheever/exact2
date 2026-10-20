# Code review: LLP 1122, stage 6 kinds in the tool, step 1 (length dials write px; non-colour tokens load), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ff4d6dc38`.
- **Method:** one brief (sha256 `0f9fb5120e7b098d119b0f5cebd32c2951a2563262f4df1ef8a5df2563668e4b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** approve (no findings).
- **Disposition:** taken; steps 1 and 2 (the grid by kind) are now one commit, which round 2 reviews.
  - The "only this mode" scope is decided by the token's kind (snapshots keep each token's kind): a non-colour token's own `when` names no under-dark axis, since it has no sides and may not vary there, and a duration or easing has none.
  - The line cap: the panel's controls and the grid's cells by kind moved to apps/design-tool/dials.ts (app.ts is 1,356 lines, and caps passes).
  - A length dial applied end to end through the real server: the dials test drives the Gap slider to 10, applies it, and the file has `gap: length = 10px`.
  - Not taken: more cases in the stub (it doesn't check values); the real-server drive covers what the stub can't.

---

I'll review this commit read-only: the design-tool length dials and non-colour token loading, against the Tamber stage 6 contract. First I'll read the review guidance and the commit itself.The commit is the design-tool stage 6 step: length dials write `px`, and non-colour tokens load. I'll read the diff and the dial and token paths it touches.The diff is small in the contract and `app.ts`; the risk is the other write paths and how every kind's `resolve` tuple is read. I'll trace those next.No High, Medium, or Low defects. The two fixes hold on every path that was checked.

**Dial writes.** A length is the only kind that gets `unit: "px"` (`apps/design-tool/app.ts:241`). The slider’s `dialNumber` stores `` `${n}${unit}` `` (`app.contract:597`) and the stepper’s `dialStepped` stores the trimmed text plus that unit, or `""` when the field is cleared (`app.contract:607`). A colour field and a select still go through `dialText`, which writes the text as given (`app.contract:624`). Scrub frames, Keep, and `dialReleased` send that same `editValue` (`app.ts:949`, `app.ts:974`, `app.contract:614`); release does not rebuild the number, so it does not drop the unit. A number dial’s unit is `""`, so those paths stay bare numbers (`-4`, `1.5`, `0`). A length becomes `-4px`, `1.5px`, `0px`, which is what `tamber/src/check.rs:1210` accepts (`Npx`, or a unitless `0`). A checked design cannot contain a length dial written in `rem` or as a named length: `describe` only runs after `check` (`contract/cli/src/tamber_serve.rs:690`), and the IR stores a length dial as `Npx` (`tamber/src/ir.rs:15`). A select option in `rem` or a named length fails `literalOf` and is dropped; an option that is `Npx` or `0` is kept and written unchanged by `dialText`.

**Grid.** For any token whose kind is not `color`, each cell is `resolve`’s `value` string, with an empty `srgb`, `residual: false`, and a transparent swatch (`app.ts:826`, `app.ts:833`). That covers length (including `rem` and a named length), number, both shadows, type, family, duration, and easing, all of which carry `value`. A missing `tuples` array becomes no cells with `css: ""`, so the answer stays inside the `Cell` shape. A null resolve entry still throws, as it did before this commit; a real `resolve` is an object. Colour residuals still take the fallback path, and the existing grid test still expects `text.system`’s fallback badge. Contrast uses `srgb`, which is empty for these kinds, so `contrast` returns `"not a colour"` (`app.ts:848`, `app.ts:1079`) and does not throw. The aria label is the value as written. Pins, the branch editor, and the inspector’s token links read `explain` and the column id, not `light`/`dark`.

**Fixture.** Against the parent, `stub/dials.json` matches aside from `gap`, `inset`, `space.gap`, and `space.inset`, the new file digests, and the `tone.warm` warning moving from line 32 to line 37 because those lines were inserted above it.

**Tests.** `app.test.contract:94` shows the slider and stepper states `10px` and `6px`. `scripts/design-tool.test.mjs` checks the Space group’s px bounds and `controlOf`’s `unit`. The older grid, contrast, residual, and branch tests load this stub, so a length token that still broke the shape would fail them. Nothing asserts a length scrub or release, an emptied length stepper, a negative or decimal length, a length select, a `rem` or named length cell, or a shadow, type, family, duration, or easing tuple.

The length dial writes `Npx` on every write path, and a non-colour token loads without breaking the grid or its other readers.

