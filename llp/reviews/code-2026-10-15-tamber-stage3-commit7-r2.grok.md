# Code review: LLP 1121 stage 3, commit 7, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `d19ae85ffd4b3bd47f352b8ff7bd6f1918bec4c07dbe675c4d184b99624d4a92`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: seven medium, three low; Grok: three medium, three low.
- **Disposition:** fixed in round 2

---

## Medium

**`@theme default` single-key `initial` wipes a non-default value** — `tamber/src/import/tailwind.rs:283`

Tailwind's `Theme.add` (`packages/tailwindcss/src/theme.ts`) returns before the delete when the new declaration is `default` and the stored value is not. A namespace reset (`--color-*: initial`) still clears first; a single key does not. This importer clears on every `initial` and never looks at `e.default`.

```css
@theme { --color-brand: red; }
@theme default {
  --color-brand: initial;
  --color-brand: blue;
}
```

Tailwind keeps `red` (`initial` yields, then `blue` yields). Here the reset drops `red`, `keys` is empty, and the later default is stored, so the token is `blue`. A non-default reset followed by a later redefinition is fine: the later key is not in `keys` yet and is added after the clear.

Fix: if `e.default` and the name is not a `-*` reset and the stored value is non-default, report this declaration as a default that yielded and do not clear.

**A light root redefinition leaves the `@theme` value on the dark side** — `tamber/src/import/tailwind.rs:351`

Every surviving colour key is written onto both `(None, false)` and `(None, true)`. A `:root` / `html` / `body` / `.light` rule then writes only the light target (`tailwind.rs:404`). With no `.dark` rule the dark winner is still the synthetic `@theme` entry.

```css
@theme { --color-brand: red; }
:root { --color-brand: blue; }
```

The token is `light-dark(blue, red)`. In CSS that variable is `blue` in both schemes until a dark rule sets it. `.dark`, dark `prefers-color-scheme`, and `@variant dark` still override only the dark side, which is right. Inline keys are unaffected: their redefinitions are reported and never enter `decls`.

Fix: a non-dark own-block assignment of a theme colour should take both sides (the root variable is what dark inherits). Own-blocks stay in source order, so a later `.dark` rule still wins the dark side.

**A side that fails to resolve is published as the other side's colour** — `tamber/src/import/engine.rs:243`

`pair` treats a missing side as "use the other one". That is right when the side has no declaration. It is wrong when the side has a winner and `colour_of` returns `None` (missing `var()`, or `visiting` hitting a cycle at `engine.rs:148`). The theme key is still `Fate::Mapped` if either side resolved (`engine.rs:233`), and an aux row is always reported as read, not as failed (`tailwind.rs:430`).

```css
:root { --ink: white; }
.dark { --ink: var(--missing); }
@theme inline { --color-ink: var(--ink); }
```

Dark does not resolve. The token is `white` in both schemes, and the report does not say the dark value failed. Cycles do stop; they do not loop. They fall into this same hole.

Fix: remember a winner that did not resolve, separate from "no winner". Inherit only when there is no winner. If a targeted side has a winner and no colour, do not emit a token that wears the other side, and report that declaration as unresolved there.

## Low

**`none` is rewritten only when it is its own word** — `tamber/src/import/mod.rs:226`

`oklch(98.5% 0 none)` becomes `oklch(98.5% 0 0)`, which matches CSS's used value and is what the 288 colours need. A first component is glued to the function name (`oklch(none 0 0)`, `rgb(none, 0, 0)`), so the word is `oklch(none` or `rgb(none,` and it is left in place. shadcn calls this same helper (`shadcn.rs:92`). Ordinary v3 triples (`222.2 84% 4.9%`) never contain `none` and still go through `hsl(…)` only when the raw value does not parse. A bare triple whose later channel is `none` (`0 0% none` → `hsl(0 0% 0)`) now imports, which is the spec's zero. A bare triple whose hue is `none` becomes `hsl(none …)` and still does not import. No v3 numeric channel changes meaning.

Fix: replace a component that is `none` after splitting arguments, including the first and comma-separated forms, not only whitespace words equal to `none`.

**An over-cap selector list is imported as the root** — `tamber/src/import/engine.rs:312`

`nested` returns an empty `Vec` past 1024 selectors. shadcn treats an empty list as "not a theme block". Tailwind treats "every selector is blank" as the light root (`tailwind.rs:240`), and an empty list satisfies that, so those custom properties are imported as theme colours (or as the dark side, if a dark media query or `@variant dark` wraps them).

Fix: an empty list from the cap should be reported, the same way shadcn's `targets` already refuses it.

**Two Tailwind reset exceptions are missing** — `tamber/src/import/tailwind.rs:138`

`clearNamespace` also keeps `--grid-column-start` / `--grid-column-end` under `--grid-column-*`, and the same for `--grid-row`. `resets` only special-cases `--font`, `--inset`, and `--text`. Those grid keys are not mapped to tokens, so the theme output does not change; a `--grid-column-*: initial` reports them as cleared where Tailwind would keep them.

Fix: add those two entries to `kept`.

## Checked, not findings

The cache is sound. `unfolded` probes one fixed design (`probe = <value>`, empty vocab), so the same string cannot mean two things, and the `thread_local` map only repeats that result per thread. `colour_literal` consults it only after `exact_color::parse` succeeds, so a string that folds as a number is not taken as a colour.

`@custom-variant dark` is ignored, and that does not mis-parse the file: the project fixture's `@custom-variant dark (&:where(.dark, .dark *));` is not a custom property. `.dark`, `[data-theme=dark]`, dark `prefers-color-scheme`, and `@variant dark` still mark the dark side. A project that points `dark` at some other selector is not followed; that matches the selectors round 1 named.

A non-default reset does not erase a later redefinition of that key. `@theme default` still yields to a non-default value on either side of it, except the single-key `initial` case above. Pending rows are cleared; `--font-*` keeps `--font-weight-*` and `--font-size-*`. `@media` around `@theme` does not condition the value; a rule inside `@theme` is reported. `--font-size-*` is type, and it is matched before `--font-*`. Named steps such as `button-foreground` stay one segment. Tracking stays a length. Files are under the 1,500-line cap (`tailwind.rs` 467, `engine.rs` 362, `import.rs` 576).
