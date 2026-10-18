# Code review: LLP 1122, stage 6 kinds in the tool, steps 1-2 (length dials px; each kind's cell), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `df6946315`.
- **Method:** one brief (sha256 `84ea16dd5d70493a14e39de94d105fc003f17c8ad9d2d67675a4f7d3861da62d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the whole output, unedited.
- **Verdict:** request changes (1 medium, 1 low).
- **Disposition:** taken.
  - A shadow is previewed in its column's scheme (`color-scheme` from the column, on the system background), so a `light-dark()` shadow shows each side. The stub gets a box-shadow token; the contract test checks both columns' previews exist and the cell's text. Not tested: the pixels.
  - A length, a number and a duration show the server's spelling (`value`; a duration with a space before ms), never JavaScript's, so `0.0000004` stays as written. Tested.
  - A platform name a non-colour token reads is "a platform value", not "a platform colour" (the branch now carries the token's kind).
  - An import's swatch paints only a colour's cell, never another kind's text.
  - Not taken: more stub edge cases (the stub checks no values); the real-server drive applies a length.

---

I'll review this commit read-only: the round-1 dispositions, the dial write paths, and how the grid handles every stage 6 resolve tuple.Round 1 asked for kind-scoped branches, a line-cap split, and a real-server length apply. I'll read those paths and the grid cell logic next.**Medium — `apps/design-tool/dials.ts:153`: a px length, a number, and a duration are reprinted with JavaScript’s number spelling, so the cell is not the text `resolve` stored.**

`cellText` builds a px length as `` `${r.amount}px` ``, a number as `String(r.amount)`, and a duration as `` `${r.ms} ms` ``. `resolve` already puts the canonical spelling in `value` (`num()` in `tamber/src/measure.rs`: decimal, no exponent). JavaScript’s `String` switches to exponent form below `1e-6` and can differ in the last digit of a 16-digit tie. Load `tamber/tests/corpus/measures.tamber`’s `tiny` (`number = 0.0000004`). The tuple is `{amount: 0.0000004, value: "0.0000004"}` and the cell and its aria label (`app.ts:692`) read `4e-7`. A length `0.0000004px` reads `4e-7px`; a duration of that many milliseconds reads `4e-7 ms`. Ordinary values such as `8px`, `0.5`, and `400 ms` still match, which is why the new tests pass. Use `r.value` for a length and a number. For a duration, insert the space into that spelling (`0.0000004ms` → `0.0000004 ms`) instead of interpolating `r.ms`.

**Low — `apps/design-tool/app.ts:683`: an import swatch paints a non-colour token as a background.**

A non-colour cell stores its display text in `css` (`8px`, a shadow, a type line). The import swatch returns that `css` whenever the named design token has a stored cell (`app.ts:514`). Import a token whose value is `space.gap`. The review draws a chip with `background-color` `8px` (`app.contract:1210`), which the contract treats as “no swatch” only when the side is `""`. Return `""` unless the stored cell is a colour (a colour’s side or its residual `srgb`).

Dial writes are right on the paths that were checked. A length slider and stepper append `px` once (`10px`, `-4px`, `1.5px`, `0px`); a cleared stepper stays `""` and is not sent as `px`. A number’s unit is empty, so those same actions stay bare numbers. Colour fields and selects go through `dialText` and keep the text. Scrub, Keep, and `dialReleased` send that `editValue`, so release does not rebuild the number or drop the unit. A checked design cannot hold a length dial in `rem` or as a named length (`describe` runs after `check`). A select option in `rem` or a named length fails `literalOf` and is dropped; `0` and `Npx` are kept and written unchanged. The grid keeps every listed kind inside the `Cell` shape, including a missing `tuples` array. Colour residuals, contrast (`""` srgb → `not a colour`), kind-scoped “Only this mode” (no under-dark axis; a duration or easing has none), pins, the branch editor, and the inspector’s links stay on `explain` and the column id. `stub/dials.json` matches the parent except `gap`, `inset`, `space.gap`, `space.inset`, the new digests, and the `tone.warm` warning moving from line 32 to 37. `app.ts` is 1,362 lines. The tests prove the `10px`/`6px` drafts, a real-server apply of `gap: length = 10px`, happy-path `cellText`, and `branchOf` for a length and a duration. They do not run an emptied stepper, a negative or decimal length, a length select, a scrub release, or an `em` or named length through the grid.

**Verdict: request changes — the grid’s px, number, and duration text is JavaScript’s spelling, not `resolve`’s `value`.**

