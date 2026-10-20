# Code review: LLP 1121 D3, pair= (what a token is drawn on, and its contrast), round 1, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `905b0a69b` (commit `6d9d4796e`).
- **Method:** one brief (sha256 `0715214d3029752f7873bd75d59773be385b5598aa6f02e1133152f19f605f7c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 2 low).
- **Disposition:** all taken: a rename rewrites the old name in every `pair=` list (bare when every name is plain, quoted as metadata is otherwise), and its undo gives the bytes back; the badges read a Map, so a token named `constructor` is no inherited property; a cell's accessible name adds each badge, a low one said "low contrast"; an empty `pair=` list or name is refused (`tamber-pair`); ratios are cut to two places, never rounded up past 4.5, in the warning and the badge alike; tests for each, a role's ratio at its fallback stated, and the low badge reached by its name.

---

1. **Medium — Renaming a paired background fails.** [tamber/src/check.rs:239](/tmp/rv-dt-p1/tamber/src/check.rs:239) introduces token references in metadata, but [tamber/src/edit.rs:776](/tmp/rv-dt-p1/tamber/src/edit.rs:776) excludes metadata from renaming. Renaming the fixture’s `surface.page` leaves `pair=surface.page` behind, so validation rejects the rename. Update `pair` entries during renames and test a paired background, including undo.

2. **Medium — Valid token names can prevent the entire grid from loading.** [apps/design-tool/app.ts:650](/tmp/rv-dt-p1/apps/design-tool/app.ts:650) reads `declared[row.name]` without checking ownership. An unpaired token named `constructor`, `toString`, or `__proto__` retrieves an inherited object property; the subsequent iteration throws. I confirmed this lookup failure in memory. Use a `Map` or an own-property check and test these legal names.

3. **Medium — Contrast badges are missing from the cell’s accessible name.** [apps/design-tool/app.contract:657](/tmp/rv-dt-p1/apps/design-tool/app.contract:657) places badges inside a button whose explicit `aria-label` contains only the token, column and colour. Focusing `tone.warm` therefore does not announce its `2.78:1` result, background, or low-contrast status; background fallback information is likewise omitted. Include these details in the accessible label or description, with a textual low-contrast indication.

4. **Low — Empty pair declarations silently disable checking.** [tamber/src/check.rs:243](/tmp/rv-dt-p1/tamber/src/check.rs:243) discards empty entries. Consequently, `pair=""` and `pair=,` pass with no pairs or warnings, despite the declared grammar requiring a token. Reject empty lists and empty comma-separated entries with `tamber-pair`.

5. **Low — The badge test does not prove its stated behaviour.** [apps/design-tool/app.test.contract:109](/tmp/rv-dt-p1/apps/design-tool/app.test.contract:109) claims coverage of each cell and low marking, but asserts only two default-light text strings. Setting every badge’s `low` to `false` would still pass. Assert the low state or rendered colour, dark-mode badges and fallback labels; add arithmetic cases covering partial foreground alpha and translucent backgrounds against both implementations.

The sRGB calculations, role normalization and overrides, tuple ordering, and nonfatal warning path otherwise look consistent. Repository tests were not run under the read-only constraint. Line references above use current HEAD.

Verdict: Request changes.
