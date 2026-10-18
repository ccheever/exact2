# Code review: LLP 1121 D3, pair= (what a token is drawn on, and its contrast), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `905b0a69b` (commit `6d9d4796e`).
- **Method:** one brief (sha256 `0715214d3029752f7873bd75d59773be385b5598aa6f02e1133152f19f605f7c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix the rename with it (1 medium, 2 low).
- **Disposition:** all taken: a rename rewrites the old name in every `pair=` list (bare when every name is plain, quoted as metadata is otherwise), and its undo gives the bytes back; the badges read a Map, so a token named `constructor` is no inherited property; a cell's accessible name adds each badge, a low one said "low contrast"; an empty `pair=` list or name is refused (`tamber-pair`); ratios are cut to two places, never rounded up past 4.5, in the warning and the badge alike; tests for each, a role's ratio at its fallback stated, and the low badge reached by its name.

---

The contrast check, the `describe` payload, and the grid badges agree on names, clips, and the ratio. Two defects are worth fixing.

### Medium

**Renaming a token that any `pair=` names is refused, and the error does not say why** — `tamber/src/edit.rs:764`

`RenameToken` rewrites names in theme values and exports, then cuts the line at the first trailing `name=value` word and leaves that metadata untouched. That was harmless when metadata was `scheme=fixed`. This commit stores token and role names there (`tamber/src/check.rs:234`). After the rename, `pair=` still spells the old name, `check` returns `tamber-pair`, and the rename gate at `tamber/src/edit.rs:216` replaces that diagnostic with "a rename needs a design that checks before and after it". The file is not written.

In the fixture this commit adds, `text.primary` is `pair=surface.page` (`apps/design-tool/stub/dials.tamber:36`). Renaming `surface.page` to `surface.ground` fails that way. The same happens for a quoted value: `meta_word` (`tamber/src/edit.rs:993`) quotes a comma-separated list, and `rename_words` copies quoted text through unchanged, so merely running the renamer over the metadata is not enough.

Rewrite each comma-separated name inside every `pair=` value, quoted or not, with the same whole-name rule as a value reference. A rename of a paired background should check and fold as it does today.

### Low

**A ratio in [4.495, 4.5) is shown as 4.50:1 and still flagged below 4.5** — `tamber/src/lib.rs:185`

The warning compares the full ratio with 4.5 (`tamber/src/lib.rs:165`) and then formats it with `{r:.2}`. The badge does the same (`apps/design-tool/app.ts:659`): `toFixed(2)` and `low: r < 4.5`. `#737874` on `#ffffff` is 4.49896, so both say `4.50:1` and mark it as failing WCAG's 4.5:1.

Format at least three decimals, or warn from the rounded value that the message and the badge show.

### Low

**The new tests do not lock the claims they are named for** — `tamber/tests/pair.rs:85`

`a_role_side_is_read_at_its_fallback_and_the_warning_says_so` only checks that the message contains "read at its fallback". A warning that appended that sentence and used some other colour would pass. `#eeeeee` on Canvas's light fallback is the ratio that belongs in the assertion.

The grid test's comment says "a low one marked" (`apps/design-tool/app.test.contract:109`) and the commit says the low badge is red. The assertions check the strings `18.51:1 on surface.page` and `2.78:1 on surface.page` only. `low` drives the red at `apps/design-tool/app.contract:657`, and a badge that never sets `low` still passes. Nothing renames a token named by `pair=`, contrasts a role at its overriding token rather than the vocab fallback, or checks that exactly 4.5 does not warn.

Verdict: the pair check and the badges are consistent, and the one fix that should land with them is teaching rename to rewrite `pair=`.
