# Code review: LLP 1121 stage 3, commit 9, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `72f5881aceca10a510eb28435dda4ed93eff9a02205788a329bca5b1fbb69c97`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two medium, one low; Grok: none.
- **Disposition:** fixed in round 1

---

**Mapping.** `--color-background-*` becomes `surface.*`, with `-exact-background` offered only for `surface.body`. Text and icon stay in their groups. Accent and the status colours stay whole (`astryx.success`, `astryx.success-muted`, `astryx.on-success`, and the same for error, warning, and accent). Overlay is `astryx.overlay`, `astryx.overlay.hover`, and `astryx.overlay.pressed` as colours. Spacing, radius, shadows (inset rings under `ring`), durations, and easing are pending until stage 6, which is what this stage requires. Min and max duration tiers are reported with no token; the three nominal durations are pending as `astryx.duration.fast|medium|slow`.

**Count.** Light and dark each have the same 258 string keys. All 150 colours map, and the other 108 are reported. `counted` is `258 = 150 + 108`.

**Colours.** Nine colours are 8-digit hex (overlay, border, neutral, shadow, and the three muted status colours). Alpha stays in the literal; none of those pairs are equal, so they become `light-dark(...)`. Fifty-three colours are identical on both sides, including `--color-data-blue-5` (`#02165E`) and `--color-on-warning` (`#111111`), and those are stored as the single literal. No pair differs only by case. `colour_literal` is the same path that already accepts `#11223380`.

**Names.** The 150 colour names are unique and are legal token names. The only shared names are the 14 text styles: size, weight, and leading of one style all name `astryx.type.<style>`, which is the compound type token the commit describes. Nothing else collapses two variables onto one token.

**Script.** The install is in `mkdtemp` under the system temp directory and removed in `finally`. `bun add --ignore-scripts` is a real flag; current Bun does not run dependency lifecycle scripts unless the package is trusted, and this script does not pass `--trust`. `@astryxdesign/theme-neutral@0.7.0` peers `@astryxdesign/core` at exactly `0.7.0`, and the script installs core at that same version. The package exports `neutralTheme` (the only export with `tokens`), `neutralPalettes`, and `neutralIconRegistry`, so the `tokens` lookup selects the theme. `resolveThemeTokens` is the public `@astryxdesign/core/theme/tokens` entry. A failed `bun add` or resolve throws, which exits non-zero.

**Probe and test.** The commit message's buckets match the fixture exactly:

| Gap | Count |
|---|---|
| Duration min/max tiers | 6 |
| Nominal durations (pending) | 3 |
| Text styles (14 × size, weight, leading) | 42 |
| Box shadows | 3 |
| Inset rings | 5 |
| Lengths (15 spacing, 7 radii, 3 element sizes, 12 font sizes, border width, focus width, focus offset) | 40 |
| Weights / families / easing | 4 / 3 / 1 |
| Line style (`--focus-outline-style: solid`) | 1 |
| **Reported** | **108** |

The test locks `258 / 150 / 108`, the six tier names, `astryx.duration.fast`, one inset ring, the accent and body `light-dark` pairs, the equal `astryx.data.blue.s5`, the background offer, and a second import that proposes nothing. I did not run the tests or `scripts/astryx-tokens.mjs`.
