# Code review: the design tool's half of stage 11 D, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI (--max-turns 400 --no-plan), reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `c3c3062230`.
- **Method:** one brief (sha256 `570a41fd1c866516ee6f8d1c2669d83a7f145be5829602d58ee8d3b0651dd775`), the plan inline, shared by both reviewers, blind to the other review; commit c3c3062230 (round 1's fixes) on the three before it. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (2 SHOULD-FIX, 2 NIT).
- **Disposition:** post-round, taken in `6e3e8952ba`: an overridden base name is edited as the app's token only when explain names its branches (`whens`, asked of theme2 for 1.14 so the override's when lines can be read back); a base reply with the override's text alone shows it read only with why (Astra blocker 1, Grok 2); the re-scope reader offers none for a base explain (Astra 2); an unoverridden base value shows the first column the app ships (Astra 4, Grok 1); Set for this app is now set-token (theme2's 17:09Z ruling), which carries no kind, the server's to keep (Astra 3); the stub moves a package's row on set and back, and the test asserts an enabled editor holding the override, and the package's set and back (Astra 5, Grok 3); the stub's removal guards a missing name (Grok 4).

Nothing blocks. The round-1 fixes that were taken are in place and do the right thing on the main paths. Two new gaps are worth fixing; neither breaks Set, Back, or the shipped-column grid.

1. **SHOULD-FIX** — An unoverridden base branch shows the first column key, not the first column this app ships. `apps/design-tool/app.ts:741`

   The comment on that line says the value is the first column the app ships. The expression is `Object.values(base.columns)[0]`, which follows the object's key order and never looks at `targets`. A Mac-only app (`targets: [{target: "macos", column: "mac"}]`) lists Mac `10px` first in the grid, and Set for this app seeds `10px`. Open the read-only branch — Back to the base while that cell is still selected, or the inspector's Open on a copy that uses `space.md` — and the editor shows iOS `16px`. The field is read-only, so it does not write that number, but it is the wrong base value.

2. **SHOULD-FIX** — "Only this mode" splits the one override line, and the base read cannot show the split. `apps/design-tool/app.ts:741`, `apps/design-tool/app.contract:1831`

   An override is one `{text}`, and explain forces `whens` to `[]`, so Apply of the value as it stands writes `set-token` with an empty `when`. In this design there is a `density` axis, so for a length (the override's kind) `branchOf` sets `own` to `density=regular` on the light default cell and `exact` is false. "Only this mode" stays enabled. Click it, change `radius.md` from `12px` to `4px`, and Apply sends `when: [{axis: "density", value: "regular"}]`. The next explain is still `kind: 'base'` with that one override text and empty `whens`, so the editor shows the single line again and the density when cannot be read back.

3. **NIT** — "Editable" is asserted by the absence of the read-only banner. `apps/design-tool/app.test.contract:1873`

   After Set, the test taps `cell-space.md-0-light` and expects `branch-readonly` to be missing. That node is also missing when explain fails and the branch panel never opens (`branch.ok` false, or `branch.token` empty). The row transitions themselves are real: Set removes `set-for-app-space.md` and shows the over-line; Back restores `set-for-app-radius.md` and removes the over-line. Asserting `branch-value` is enabled and holds the override text (`16px`) would lock the edit.

4. **NIT** — A remove of a base name that is not in the stub's token list deletes the last scale token. `apps/design-tool/stub/tamber-stub.ts:316`

   `findIndex` is `-1` when the name is not in `SCALE_TOKENS`, and `splice(-1, 1)` then removes `reading.measure`. Back on `radius.md` is safe because that name is in the list. A `remove-token` for `space.md` before Set is not offered in the grid, and it would drop the package row.

Taken, and right: `shippedOf` reads `{target, column}` and maps nothing (`base.ts:57`); with an override, explain is one editable line whose value is `override.text` and whose kind is the token's (`app.ts:740-744`); with none, the branch is read-only and the kind is `length`; Set seeds `tuples[0].value` for a token and the first shipped column for a base row, not `cellText`; `branchKey` includes `readOnly`, so that draft goes stale, and Apply, Scrub, and Remove's confirm refuse a read-only branch edit (`app.contract:827`, `1842`, `1900`); Set and Back set `file` to `""`, so the op is the app's design; a package row whose `file` is hello's design is the app's line with Back to the package (`base.ts:47`); the stub moves the base row on add and remove, and the test asserts those two transitions.

Not taken, and the code matches the reasons: platform lines are text, with Set for this app as the edit (`app.contract:1730`); Back's undo bytes are still the stub's count, left for the real server.
