# Code review: LLP 1121 stage 3, commit 6, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `2f01c58bedd43c294fb1eca660c17b70bc71d18885be1de057b0d6d3f407904e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: two high, eight medium, three low; Grok: two high, six medium, three low.
- **Disposition:** fixed in round 1 (a CSS reader that reads strings and nesting, dark media, selector lists, colour references, collisions, when lines taken back, the review across a recompute); the accent-color offer stays a hand Contract edit until the server writes Contract

---

## High

**A dark `prefers-color-scheme` `:root` replaces the light palette — `tamber/src/import/shadcn.rs:38`**

`css_blocks` flattens a nested `:root`, and `theme_of` treats every `:root` as the light side. For

```css
:root { --background: oklch(1 0 0); }
@media (prefers-color-scheme: dark) { :root { --background: oklch(0.2 0 0); } }
```

both declarations land on the light side, the dark value wins, and the light value is reported as a later duplicate in the same block. The import checks, with `shadcn.background = oklch(0.2 0 0)`. That is the media-query dark mode shadcn documents. Carry the enclosing prelude into the block and treat `@media (prefers-color-scheme: dark)` as the dark side, or report that inner block unmapped and leave the outer `:root` as the light value.

**A block that also contains a nested rule drops its own declarations, and the count still balances — `tamber/src/import/mod.rs:245`**

If the body contains `{`, the reader only recurses. The enclosing declarations are never counted. `:root { --background: oklch(1 0 0); @media (…) { --background: oklch(0.2 0 0); } }` reports one source value (the inner one, as “not a theme block”). The light declaration is gone. Record a block’s own `--*` declarations and then enter nested rules.

## Medium

**The reader does not track strings or comments inside them — `tamber/src/import/mod.rs:199` and `tamber/src/import/mod.rs:228`**

Brace depth and `split(';')` are byte scans, and `strip_comments` deletes from `/*` to `*/` even inside a quote. `--font: "a { b }"` drops the rest of that `:root`, including a following `--background`, and the count never sees it (a later `.dark` becomes the only colour). `--font: "/*"` eats the rest of the file and the count is 0. `--font: "a; b"` keeps `--background` but stores the font value as `"a`. Scan strings and comments before braces, semicolons, and `/*`.

**A theme class named `default`, or a name that is not an identifier, makes the whole proposal fail at apply — `tamber/src/import/shadcn.rs:278`**

The axis is always `default` plus every class name, unchecked. `.theme-default` emits `theme = default | default`, and `apply` refuses with “an axis has values, each once”. `.theme-2` emits the value `2`, and `apply` refuses with “an axis and its values are identifiers”. Either refusal drops the colour tokens with the axis. A name with a space (`[data-theme="my theme"]`) is split on whitespace in `theme_of` (`shadcn.rs:29`) and reported as not a theme block. Reject these at import time, with a diagnostic. Keep `.theme-default` from colliding with the synthetic base (treat it as the base, or pick another value and say so). Run theme names through `is_ident` before `Ok`.

**`var(--name)` is mapped as a colour whenever the name was seen, including a length that never becomes a token — `tamber/src/import/shadcn.rs:89`**

`--gap: var(--radius)` with `--radius: 0.5rem` counts `--gap` as mapped and writes `shadcn.gap = shadcn.radius`. `--radius` is only in the unmapped report. `check` then refuses the whole proposal: `shadcn.radius` is no token. Resolve `var()` only when that variable was mapped to a colour token; otherwise report it. An unknown name is already reported, and `var(--background)` to a real colour checks.

**A `when` the design has and the source no longer has is left in place, and the next import proposes nothing — `tamber/src/import/mod.rs:146`**

`reconcile` adds or updates source `when` lines and never emits `unset-token-when`. After a theme class is imported, changing that class so its colours equal the default (or deleting the override) leaves `when theme=blue: …` on the token. `again.ops` is empty, so the folded theme stays on the old value. Emit an unset for each design `when` on an imported token that the source did not produce. The same gap leaves a `theme` axis value behind after the class disappears from the file.

**`theme_of` aborts the whole selector on any simple selector it does not recognise — `tamber/src/import/shadcn.rs:52`**

`class(p)?` returns `None` for the selector. `html.dark .theme-blue` is “not a theme block”: the dark colour is listed unmapped, and the token’s `when theme=blue` repeats the light value, so an accepted import never goes dark. `[data-theme="blue"], .theme-blue` is the same. `.theme-blue, [data-theme="blue"]` works only because the last part overwrites a name that had already kept the comma (`blue,`). Ignore unknown simple selectors, and take a theme class if one is present. Split selector lists on commas before classifying.

**A recomputed held proposal drops `review` — `scripts/tamber-server.mjs:416`**

`accept` calls `hold` without the review object. A conflict recomputes every operation, including the role overrides, and the replacement has no `offers`, `unmapped`, or `tokens`. The tool can no longer leave overrides off or list unmapped values. Pass the previous review through, and renumber `offers[].items` when the operation list was filtered.

## Low

**Only the exact list `:root, .light` is the light side — `tamber/src/import/shadcn.rs:38`**

That spelling maps to `light-dark(…)`. `.light, :root` and `:root,.light` are reported as not a theme block. Split on commas, trim, and treat either order as light.

**A variable that exists only on a theme class gets a `when` equal to its default — `tamber/src/import/shadcn.rs:238`**

The equality check runs before the fallback that copies the first theme value into the default. `--destructive-foreground` only under `.theme-blue` becomes `hsl(…)` plus `when theme=blue: hsl(…)` with the same value. Folding matches. Compare again after that fallback and omit the `when`.

**A role already written with the bare spelling is offered again — `tamber/src/import/shadcn.rs:317`**

The lookup is the declaration text `-exact-label`. A design that already has `label = shadcn.foreground` still gets an `add-token` for `-exact-label`, and accepting it fails with “declared twice”. Match with the same role key the checker uses.

## Checked, and not findings

D14’s names match the code: `shadcn.background` / `foreground`, `card` and `card-foreground` kept whole, `-exact-secondary-background` for `--card`, `-exact-separator` for `--border`, root `accent-color` naming `shadcn.primary` as a hand edit with no operations, `--chart-1` → `shadcn.chart.s1`, `--sidebar-*` → `shadcn.sidebar.*`. The pinned Neutral and Blue fixtures count (47 = 46 + 1, 39 = 38 + 1), and the four derived radius rows sit outside that sum.

Duplicates in one side are reported and the later value wins. Empty `--radius: ;` is “an empty value”. Non-theme blocks are reported. `var(--missing)` is reported. Slash alpha (`222.2 84% 4.9% / 0.5`) becomes `hsl(… / 0.5)` and checks. Bare `222.2deg …` and comma-separated `222.2, 84%, 4.9%` are reported unmapped. The RFC’s form is the space-separated channels, which is what the Blue fixture uses.

`add-axis` after an existing axes block, and a new `axes` block after the header, round-trips through `tamber fmt`, including a top-level comment between the header and `theme`. An axis named `theme` next to a token named `theme` checks. `set-token` keeps an existing token’s kind. A source path with spaces is one `spawnSync` argument, so the shell never splits it. The 64-tuple cap and the 1 MiB request cap are not what the pinned themes hit.

`@theme` is skipped and not counted. That matches this commit (Tailwind’s importer owns those blocks). A colour that exists only there would be absent while the check passes.

The tests never leave the three fixtures: no sidebar variable, no media query, no `.theme-default`, no `var()`, no `when` removed on re-import, no bare role spelling, and the wire test does not check that `review` survives a recompute.
