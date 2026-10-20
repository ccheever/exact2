# Code review: LLP 1122, stage 6 kinds in the tool, steps 1-2, round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `c2e932f72`.
- **Method:** one brief (sha256 `115161407f192f12c36f9d796f98cc5f612dabeae5855a2ac0a4117235b5e061`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium).
- **Disposition:** taken (the last round). A box shadow's preview is a box inside a padded box of the column's scheme and ground, so the shadow falls on its own ground, not the tool's page. Not taken: a pixel assertion. The contract test checks the previews' nodes and text, and the gate checks them by eye.

---

- **Medium — [apps/design-tool/app.contract:1068](/tmp/rv-dt-k1r3/apps/design-tool/app.contract:1068): outer box shadows still paint against the tool’s scheme.** With the tool in light mode, the fixture’s Dark column selects the white shadow correctly, but paints it outside the dark box onto the white page, making it disappear. The background on the shadow-casting box covers only its interior. This leaves round 2’s preview fix incomplete. Put the sample inside a padded container with the column’s `color-scheme` and background, containing the shadow’s overflow. Add a rendered assertion: the current test checks only node existence and text.

The other taken fixes look correct:

- Length sliders and steppers append `px` once; negatives and decimals retain their values, and clearing a stepper leaves `""`. Number controls remain unitless. Selects and colour fields preserve their text. Apply, `scrubFrame`, `releaseScrub`, and `dialReleased` carry the formatted draft unchanged. `rem` and named lengths are rejected dial values; valid length-select options already carry their units.
- Every requested non-colour tuple kind produces valid cell fields; missing tuples become empty cells. Colour residuals, contrast handling, aria labels, pins, branch editing and inspector links remain intact. Kind-based scopes exclude under-dark axes for non-colours and all overrides for duration/easing.
- Server numeric spelling, neutral platform-value wording, import-swatch filtering and the source-file line cap are fixed.
- Fixture comparison found no unrelated drift: only the two dials, their tokens, `shadow.card`, updated digests and the warning moving from line 32 to 38. The source digest matches.

Tests cover draft formatting, a real-server length-slider apply, representative cell formatting and branch scopes. They still do not prove length-stepper apply, length scrub/release, length-select and empty/negative/decimal cases, or shadow pixels. I inspected tests without running write-producing suites; the working tree is unchanged.

**Verdict: request changes for the remaining box-shadow preview defect.**
