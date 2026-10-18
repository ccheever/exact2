# Code review: LLP 1122, stage 6 kinds in the tool, steps 1-2 (length dials px; each kind's cell), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `df6946315`.
- **Method:** one brief (sha256 `84ea16dd5d70493a14e39de94d105fc003f17c8ad9d2d67675a4f7d3861da62d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 1 low).
- **Disposition:** taken.
  - A shadow is previewed in its column's scheme (`color-scheme` from the column, on the system background), so a `light-dark()` shadow shows each side. The stub gets a box-shadow token; the contract test checks both columns' previews exist and the cell's text. Not tested: the pixels.
  - A length, a number and a duration show the server's spelling (`value`; a duration with a space before ms), never JavaScript's, so `0.0000004` stays as written. Tested.
  - A platform name a non-colour token reads is "a platform value", not "a platform colour" (the branch now carries the token's kind).
  - An import's swatch paints only a colour's cell, never another kind's text.
  - Not taken: more stub edge cases (the stub checks no values); the real-server drive applies a length.

---

- **Medium — [app.contract:1064](/tmp/rv-dt-k1r2/apps/design-tool/app.contract:1064): shadow previews use the tool’s scheme, not the column’s.** A valid shadow such as `0px 1px 3px light-dark(#000000ff, #ffffffff)` reaches both cells unchanged. Neither shadow preview sets `color-scheme`, so Light and Dark columns display the same side. This also affects text shadows and scheme-dependent platform colours. Carry the column’s scheme into the preview and render it with appropriate foreground/background colours. Add a rendered test distinguishing both columns; the current shadow test checks only text.

- **Low — [app.contract:1092](/tmp/rv-dt-k1r2/apps/design-tool/app.contract:1092): named lengths are described as platform colours.** Selecting `hairline: length = -exact-hairline` makes `inputsOf` classify the name as a role, and the branch editor says “a platform colour, set by the system.” Text-style lengths such as `-exact-body` have the same problem. Classify these dependencies by their actual kind, or use accurate neutral wording.

The original fixes otherwise hold: length sliders and steppers preserve `px` through apply, scrub and release; clearing a stepper stays empty; negative and decimal values retain their spelling. Selects preserve accepted literals. `rem` and named lengths are valid tokens but rejected dial values.

In-memory probes loaded every requested tuple kind, residuals and missing tuples without breaking the answer shape. Kind-based scopes are correct, and the source files meet the cap. Fixture comparison found no unrelated drift.

Tests now cover a real-server length-slider apply, but still lack length-stepper apply, length scrub/release and the requested edge cases. Full suites were not run under the read-only constraint; no files changed.

**Verdict: request changes for the incorrect shadow previews.**
