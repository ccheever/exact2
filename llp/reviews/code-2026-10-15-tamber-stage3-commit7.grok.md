# Code review: LLP 1121 stage 3, commit 7, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `f93c9248efae2e8c28dbc5d7375c310a14c2ba0553dc68019dd116958e5ad139`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: four medium, two low; Grok: two high, four medium, one low.
- **Disposition:** fixed in round 1 (the theme built as Tailwind builds it, whole blocks only, backing variables and inline keys, resets, defaults, none components); named palette steps stay whole names and tracking stays a length, for theme1's RFC wording

---

## High

### Dark detection is a substring, so the wrong rules become the theme's dark side
`tamber/src/import/tailwind.rs:109-117`, applied at `:154-161` and `:216`.

`dark_selector` treats any prelude that contains `.dark` (next character not an ident) or the substring `[data-theme=dark]` as the global dark side. `@variant dark` anywhere in the context does the same.

- `:root:not(.dark)` and `:not(.dark *)` are stored as dark. A light override becomes the dark side of `light-dark()`.
- `[data-theme=darker]` and `[data-theme=dark-blue]` match `[data-theme=dark]`.
- `.card { @variant dark { --color-primary: navy } }` and `.dark .card { --color-x: … }` are component-scoped, but they overwrite `tw.primary`'s dark side. shadcn refuses that shape (`shadcn.rs` `target`, around `:112-118`).

`:root.dark`, a bare `.dark`, a bare `[data-theme=dark]`, and a top-level `@media (prefers-color-scheme: dark)` are the idioms that should match. `@custom-variant dark` is never read. The project fixture's `@custom-variant dark (&:where(.dark, .dark *))` (`tailwind-project.css:3`) is ignored; `.dark` is hardcoded, which happens to match that one variant and also matches `.dark` when the project never redefined `dark`.

Fix: parse compound selectors the way shadcn does. Accept `.dark` and `[data-theme=dark]` only as a whole simple selector, and do not treat `:not(.dark)` as dark. Accept `@variant dark` only when it is not nested under a component selector. If `@custom-variant dark` names a selector, use that selector.

### `@theme inline` leaves `var(--background)` unresolved, and a dark literal then becomes the whole token
`tamber/src/import/tailwind.rs:153`, `:182-190`, `:206-216`, `:283-286`; `tamber/src/import/engine.rs:226-230`.

Every `@theme` prelude matches `starts_with("@theme")`. `inline`, `static`, `reference`, and `default` are not parsed.

`inline` means the utility gets the raw value. For `@theme inline { --color-foreground: var(--foreground) }` that raw value is `var(--foreground)`, not a nested `var(--color-foreground)`. In a shadcn v4 sheet, `--foreground` lives on `:root` and `.dark`, which this importer reports as "not an `@theme` block" (`:182-190`) and never puts in the engine. The theme key is then "`--foreground` is no colour where this block reads it" and contributes no light side.

A later `.dark { --color-foreground: black }` is still a `--color-*` literal, so it is a dark side. `pair` turns a dark-only side into the token's default (`engine.rs:229`: `(None, Some(x)) => Some(x)`). `tw.foreground` becomes `black`, with no light value. Tailwind utilities never read that dark override; they read `var(--foreground)`.

`var(--color-brand-600)` inside `@theme` does become `tw.brand.s600` (the project fixture). `static` and `reference` do not change the theme value (`static` only forces the variable to be emitted; `reference` only stops it being emitted). Those two options are fine to ignore for token values.

Fix: parse the `@theme` options. For `var(--x)`, resolve `--x` from custom properties in the same stylesheet (`:root`, `.dark`, and other theme keys) and build `light-dark()` from those. On an `inline` key, a `.dark` override of `--color-*` is not the dark side; the referenced variable's dark value is.

## Medium

### Resets drop only earlier light `@theme` colours
`tamber/src/import/tailwind.rs:119-125`, `:192-205`, `:233-247`.

`--color-*: initial` in a non-dark `@theme` does clear earlier `--color-*` declarations. The project test's 288 defaults depend on that, and a colour declared later in a light `@theme` is kept. That part matches Tailwind (`Theme.add`: a `-*` / `initial` clears keys already in the map, then later keys are added).

The rest does not:

- `--spacing-*: initial`, `--font-*: initial`, and `--*: initial` are reported as resets, but spacing, type, shadow, and easing rows were already reported as `kind pending` with token names (`:217-228`). They stay in the report. The module comment says those names are not imported.
- `initial` inside `.dark` or a dark `@media` is not recorded (`:194` requires `in_theme && !dark`). An earlier `.dark { --color-surface: black }` survives, and the report still says "what it names before it is not imported".
- The prefix is `strip_suffix('*')`, so `--color-*` is `--color-`. Tailwind clears `key.slice(0, -2)` (`--color`), and `ignoredThemeKeyMap` keeps `--font-weight` / `--font-size` out of `--font-*` and `--text-shadow` out of `--text-*`. A naive "also reset pending rows" pass would delete those.

Fix: one ordered theme map, same as `Theme.add`. On `initial`, delete that key; on a `-*` key, clear the namespace with those ignored prefixes. Apply that to dark entries too. Report the removed rows as reset, not as pending tokens.

### `@theme default` loses to source order, and `@media` around `@theme` flips the side
`tamber/src/import/tailwind.rs:153-169`, `:216`; `tamber/src/import/engine.rs:156-162`.

Tailwind skips a `@theme default` declaration when the key already has a non-default value. The engine keeps the last declaration for the block (`winners.insert`), so a later `@theme default` overwrites an earlier user value.

`@theme` nested in `@media (prefers-color-scheme: dark)` is still an unconditional theme value in Tailwind (`theme.add` ignores the enclosing media). Here `dark` is true whenever that media query is in the context, so those colours become the dark side only.

`@media` inside `@theme` is illegal in Tailwind 4.3 (`@theme` may only contain custom properties and `@keyframes`; the pinned commit `e48c5e80` throws). A nested `@media (prefers-color-scheme: dark)` is imported as `light-dark()`.

Fix: record the `default` bit and keep the existing non-default value. Treat `@theme` keys as light theme values even when an outer media query is dark. Report a nested rule inside `@theme` instead of mapping it.

### The neutral scale is omitted because the hue is `none`
`tamber/tests/import/tailwind-theme.css:238`, `:250-260`, `:274`; `tamber/src/import/tailwind.rs:97-105`; `tamber/src/color_fn.rs:120-124`.

The 13 reported colours are `--color-zinc-50`, all 11 `--color-neutral-*` steps, and `--color-mauve-50`, each `oklch(… 0 none)`. `exact_color` reads `none` as `0`. Chroma is already `0`, so the hue is powerless and `0` is the value CSS conversion uses. The folder rejects the keyword (`"a missing component (none) is not folded yet; write its value"`). The importer drops the token. Accepting the import checks only because `neutral` was never proposed. The counted test pins that omission (`import.rs:417-428`).

Fix: when the folder rejects `none` and `exact_color` parsed the colour, import the numeric form (`oklch(98.5% 0 0)`), which is what "write its value" asks for.

### `unfolded` runs a full design check per literal, with no roles
`tamber/src/import/mod.rs:192-203`, called from `tailwind.rs:99-104` and `:288-291`, and from `shadcn.rs:169-171` and `:331-339`.

Every colour `exact_color` accepts is wrapped in a probe design and passed to `check` with `Vocab { roles: Vec::new() }`. The default theme does that for all 288 colours. A `none` colour pays again on the report path (shadcn calls it twice there: the `is_some()` test and the message). The tailwind importer has no vocab argument, so the probe cannot see the app's roles. There is no cache.

The engine move itself matches commit 6: winners, `var()` resolution, the dark chain, and cycle rewriting in `engine.rs:43-271` are the old shadcn logic. The behaviour change is this predicate. shadcn's `literal` now refuses a colour the folder will not take, which is what the currentcolor report change is.

Fix: call the colour folder (or a small `none` / `currentcolor` check) instead of `check` on a synthetic file. Cache by the collapsed literal. Pass the real vocab from shadcn; thread it into tailwind if a literal can name a role.

## Low

### The gate tests lock the counts, not these rules
`tamber/tests/import.rs:413-517`.

`419 = 275 mapped + 144 reported`, the 13 `none` rows, `tw.red.s500`, `tw.black`, and the project `.dark` / `--color-*: initial` case all pass by asserting those totals. Re-import of those two sheets proposes nothing, and `counted` (`tailwind.rs:298-300`) only checks `mapped + reported == source`. Mis-classified rows still add up. `initial` is matched case-sensitively (`:193`); CSS-wide keywords are not. `--font-size-*` matches `--font-` and is reported as a family (`:64`).

Fix: add fixtures for `:not(.dark)`, `@variant dark` under a component selector, `@theme inline { --color-foreground: var(--foreground) }` with `:root` and `.dark`, `@theme default` after a user value, `--*: initial` and a dark-block `initial`, and a `none` hue that must land as a token.

## Checked, no defect

Numeric steps and non-numeric tails match D14. `--color-red-500` is `tw.red.s500`. A multi-dash palette keeps the dashes in the name (`--color-warm-gray-500` → `tw.warm-gray.s500`). A non-numeric tail is not a step (`--color-button-foreground` → `tw.button-foreground`, not `tw.button.sforeground`). Digit segments take `s`, so `--spacing-4` is `tw.space.s4`, `--text-2xl` is `tw.type.s2xl`, and `--radius-2xl` is `tw.radius.s2xl`; those names are identifiers. `--spacing` is `tw.space.base`. Weights, leading, tracking, the three shadow kinds, and easings get the `tw.weight` / `tw.leading` / `tw.tracking` / `tw.shadow` / `tw.inset-shadow` / `tw.text-shadow` / `tw.ease` tokens and are pending. Breakpoints, containers, blur, drop-shadow, perspective, aspect, animate, and `--default-*` are reported. shadcn's winner, reference, and cycle behaviour is unchanged aside from the `unfolded` filter above.
