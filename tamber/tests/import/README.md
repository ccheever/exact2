# Importer fixtures (LLP 1121 stage 3, commits 6–9)

Pinned copies of other systems' themes, as the importers' gates read them.

| File | Source | Version |
|---|---|---|
| `shadcn-neutral.css` | shadcn/ui's default theme (Neutral, v4 `oklch()`): `cssVarsTemplate` of <https://ui.shadcn.com/r/colors/neutral.json> | fetched 2026-10-15 |
| `shadcn-blue.css` | shadcn/ui's Blue colour theme (v3 HSL channels), as its themes page gives it: the `.theme-blue` and `.dark .theme-blue` blocks of `apps/v4/public/r/themes.css` with the selectors written `:root` and `.dark` | shadcn-ui/ui `1aa35048a5651206cfc7677a79dc2d1fafae0978` |
| `shadcn-neutral-themes.css` | `shadcn-neutral.css` followed by the same two Blue blocks verbatim (`.theme-blue`, `.dark .theme-blue`): a further theme class | as above |
| `tailwind-theme.css` | Tailwind CSS v4's default theme, `packages/tailwindcss/theme.css` | tailwindlabs/tailwindcss `e48c5e80473c25fe5a27fe267fbaf4f6512424a3` (tailwindcss 4.3.3 on npm, fetched 2026-10-15) |
| `tailwind-project.css` | Written for this test: a project's `@theme` with a colour reset, a palette, a reference, and a `.dark` block | — |
| `tokens-studio/` | Tokens Studio's sample, `tokens/` of tokens-studio/lion-example (multi-file sets, `$themes.json` with brand and colour groups, the legacy `value`/`type` form) | tokens-studio/lion-example `18030855cefc5b84ffb7f3b66ac8cdc8f6e15b83` |
| `figma/` | Figma's Variables export example, `tokens/` of figma/variables-github-action-example (`<Collection>.<Mode>.json`: Light and Dark, two themes, one default) | figma/variables-github-action-example `0bcdfcb99eddd5baaf3e18eb4d77a499e9ef1d6e` |
| `resolver.json` | Written for this test: a Resolver Module document with a set, a light/dark modifier and a brand modifier, in 2025.10's colour object form | — |
