# Code review: stage 3 commit 9 follow-up: Astryx's AccentColor offer, round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `ce550aafd6eb24347f0378af86ec6a6e05be6cdef52c044b3d132bbf83011902`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both: an existing role a changed value could split is not named).
- **Disposition:** fixed in the first-round commit (split_roles, shared with shadcn)

---

I'll review the HEAD commit read-only against the Astryx role-offer checks you listed, starting with the diff and the importer code it touches.The commit is Astryx's `AccentColor` role offer. I'll read the diff and the reconcile path it depends on, then check the five points you named.One blocking hole: a kept `when` that currently folds to the same colour as the default becomes a real axis split when this import rewrites the default, and a role that already names that token is not named.

## Blocking

**Accepting the token transaction can make an existing role fail `tamber-role-axis`, with no offer naming it.** `tamber/src/import/astryx.rs:274`

The gate only looks at the two paired tokens. If `astryx.accent` or `astryx.surface.body` has any `when`, it pushes a note on `AccentColor` or `-exact-background` and leaves the role operations empty. It never looks at any other role. `reconcile` is called with no owned axis (`tamber/src/import/astryx.rs:243`), so a design `when` is kept (`tamber/src/import/mod.rs:159`) while a changed default is still written (`tamber/src/import/mod.rs:148`).

A `when` whose folded colour matches the default does not fail the build today. Lowering clears the slice list when every tuple's `css()` matches (`contract/lower/src/theme.rs:249`), and only then checks `tamber-role-axis` (`contract/lower/src/theme.rs:254`). `#111` and `#111111` both fold to `#111111ff`. So this design compiles:

```tamber
tamber app

axes
  density = default | compact

theme
  astryx.accent = #111111
    when density=compact: #111
  Canvas = astryx.accent
```

Import `--color-accent: #333333` on both sides. The offer list is only the "not offered" note for `AccentColor`. The token operations set the default to `#333333` and leave `when density=compact: #111`. `Canvas` is then `#333333ff` in one tuple and `#111111ff` in the other, and the build fails with `tamber-role-axis`. The same happens for `accentcolor`, `-exact-fill`, or a chain (`brand = astryx.accent`, `Canvas = brand`, including `label` and `-exact-label` as one token).

Shadcn's second pass exists for this, not only for a `when` or an axis the import adds (`tamber/src/import/shadcn.rs:409`). Astryx adds neither, and that is not sufficient: rewriting the default under a kept `when` is enough. After the loop, name every design role (any spelling, via `role_key`) whose value names such a token, directly or through another token, with empty operations and a note that cites `tamber-role-axis`. Skip a role an earlier offer rebinds with operations.

The tests miss it. `astryx_offers_no_accent_role_over_a_token_the_design_varies` (`tamber/tests/import.rs:1305`) only checks that the new accent offer has no operations. `an_astryx_import_with_every_offer_accepted_compiles` (`contract/cli/tests/it/tamber_cli.rs:202`) compiles the neutral fixture from an empty design, where nothing names the token yet.

## What holds

The variance gate is a sound over-approximation for the new binding. Each offered value is a colour literal, or `light-dark` of two: a side is mapped only after `colour_literal`, which requires `exact_color::parse`. That parser accepts hex, named CSS colours, and absolute `rgb` / `hsl` / `oklch` / `color()` with numeric components. A token reference does not parse, so a mapped token cannot name another token. `astryx.accent` is not a role. Plain tokens are keyed by the exact declared name (`tamber/src/check.rs:454`), so another spelling is a different token, and a role spelling cannot hide a `when` on this one. Counting every `when`, even one that still folds to a single colour, refuses more than lowering does. That is the safe direction for the new offer.

An existing role is updated in place under any spelling. `token_index` (`tamber/src/edit.rs:942`) resolves `accentcolor`, `AccentColor`, `background`, and `-exact-background` to one schema name, and the edit uses the design's own name (`tamber/src/import/astryx.rs:293`). An unchanged binding (`value` already the token, no `when`) adds no offer (`tamber/src/import/astryx.rs:291`). The neutral re-import locks that for the canonical spelling (`tamber/tests/import.rs:1297`). No test covers a second spelling; the lookup is the same one shadcn already uses.

Nothing else in this commit is blocking.
