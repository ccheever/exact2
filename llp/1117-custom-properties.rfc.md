# LLP 1117: Custom properties — design tokens that keep their names

**Type:** RFC
**Status:** Draft r1, 2026-10-09, for review. Nothing here is built.
**Systems:**
- Contract (`contract/syntax`: `--` attribute names and the `theme` block's entries; `contract/lower`: declaration, type and cycle checks; `contract/cli/src/sources.rs`: `use theme`)
- Kernel (`kernel/tables/schema.json`: the plan's custom-property table and a pending-substitution row value; `kernel/src/style/`: substitution, per-subtree resolution and invalidation beside `env.rs` and `roles.rs`)
- Runner (bound declarations; the runtime check of a computed value)
- Web host and the web JS target (declarations and `var()` written as CSS text)
- Apple and Linux hosts (nothing new: they receive resolved rows)
- Agent API (`layout` reports a row's `var()` chain)
**Author:** Claude (Opus 5.5) for James Ide
**Implementer:** unassigned (RULES: a spec needs an implementer and a date before it is built)
**Date:** 2026-10-09
**Related:**
- LLP 1095 D4 (`theme`, amended to declare root custom properties), D10, D11 (the web role sheet).
- LLP 1116 D9 (`--exact-safe-area-inset-*`, on its own branch): the first host-registered custom properties. Its per-subtree resolution and invalidation are this RFC's, applied to four names the host sets.
- LLP 1069.011 D8 (`--exact-accent`) and LLP 1062 (`--exact-tint`): host-private custom properties on the web.
- LLP 1081 D5, D7 (amended: author custom property names and the lexer).
- LLP 1118 (class lists): a `style` can carry custom property declarations.
- LLP 1034 §8 (per-subtree `color-scheme`), LLP 1047 (pay for what you use), LLP 1001 (declared deviations land there).
- LLP 1119 (the Expo styleguide plan): the first consumer.
- [CSS Custom Properties for Cascading Variables 1](https://www.w3.org/TR/css-variables-1/); [CSS Properties and Values API 1](https://drafts.css-houdini.org/css-properties-values-api/) (`@property`).

## Summary

A design system is a graph of names: `--expo-theme-text-default` is `var(--slate-12)`, which is a pair of hex colours. Today Contract can only flatten it. A `style` block holds literals, there are no file-scope constants, and `provide`/`inject` carries values to components but not into a `style`. So a converted design system loses every relationship the moment it compiles.

CSS already has the answer, and the web is the standard: **custom properties**. This RFC admits them with three narrowings, each declared in LLP 1001:

1. **Every author custom property is declared**, in the app's `theme` (LLP 1095 D4, amended). That is where its root value lives, as `:root { … }` is on the web. The declaration also gives the property a type, as `@property` would.
2. **Substitution is checked at compile time** wherever the value is known. A misspelt name, a cycle, or a colour substituted into a length is a compile error, not a silent `unset`.
3. **Values cascade by inheritance only.** There are no selectors. A node, or a `style` it uses, declares a value, and the node and its descendants see it.

The kernel resolves `var()` per subtree on native hosts, and re-resolves exactly the rows that read a property when its value changes. The web writes the declarations and the `var()` text as they are, so the browser's devtools show the same chain the author wrote.

## 1. Motivation

**The consumer.** LLP 1119 converts Expo's styleguide (`@expo/styleguide`, `@expo/styleguide-base`) to Contract. Its `expo-theme.css` is 151 custom properties in three layers:
- Radix's colour scales (`--slate-1` … `--slate-12`, eight hues, light and dark)
- semantic tokens over them (`--expo-theme-text-secondary: var(--slate-11)`)
- component tokens over those (`--expo-theme-button-secondary-background: var(--slate-3)`)

Its Tailwind preset then refers to the semantic layer (`bg-default` is `var(--expo-theme-background-default)`, `shadow-md` is `var(--expo-theme-shadows-md)`). The owner's requirement (2026-10-09): convert it so the semantic relationships between styles are kept, not precomputed.

**What breaks without the relationships:**
- A retune of `--slate-11` should move every secondary label. Flattened, it is a search-and-replace across generated hex.
- A subtree that wants a different value for one token (a destructive section, a marketing hero) has to restate every style that used it.
- An agent reading a screen sees `#60646c` and cannot say which token it is, or whether two greys are meant to match.

**Why not something of Contract's own:**
- **`provide`/`inject` token records** (what the Markdown readers do, LLP 1033) carry values to components, but a `style` block is literal-only (LLP 1017.000 P6). Every use becomes a binding at the call site plus an `inject` line, and nothing reaches the web as a name.
- **File-scope constants** would keep names in the source, but lowering flattens them. Nothing can override a subtree at run time, and the web output shows hex.

Custom properties keep the names in the source, the plan, the kernel, the browser and the agent's report.

## 2. Decisions

### D1 — Declared in the `theme`, typed by their value

```
theme
  --slate-11 = light-dark(#60646c, #b0b4ba)
  --slate-12 = light-dark(#1c2024, #edeef0)
  --expo-theme-text-secondary = var(--slate-11)
  --radius-md = 6px
  --expo-theme-shadows-xs = 0 1px 3px var(--shadow-a1), 0 1px 2px var(--shadow-a2)
  --duration-default = 150ms
```

- **An entry whose name starts `--` is a custom property.** Its value is its root value, as a `:root` rule's would be. Entries without `--` keep LLP 1095 D4's meaning (a role override).
- **The type is the value's**, inferred at compile time as `@property`'s `syntax` would be declared:
  - `<color>`, if the value parses as a colour (a role, a `platform-color()`, a pair, a relative colour, or a `var()` of a `<color>` entry)
  - `<length-percentage>`, `<number>`, `<time>` or `<angle>`, if it parses as one of those
  - otherwise `*`, CSS's universal syntax: a token sequence checked at each place it is used (D4)
- **A typed property inherits its computed value**, as a registered property does. A colour stays a reference (LLP 1095 D1), so a theme entry naming `secondary-label` still resolves per view, per trait set. A `*` property inherits its tokens.
- **A `var()` chain keeps its links.** `--expo-theme-text-secondary = var(--slate-11)` is stored as a reference to `--slate-11`, not as its value. A node that overrides `--slate-11` changes `--expo-theme-text-secondary` below it too, as in CSS, because substitution happens where the value is used.
- **Every `--` name an author uses must be declared**, in the app's theme or a used one (D7). An undeclared one is `lower-var-undeclared`. This is the first narrowing. CSS lets any element invent a property; a typo then silently falls back. Contract's rule is no quiet failures (LLP 1017.000 P1).
- **Names** are CSS's `<dashed-ident>`, case-sensitive as in CSS. `--exact-*` is reserved for the host (LLP 1081 D5, amended): an author reads one only where its spec admits it, and declares one only where its spec admits an override (LLP 1116 D9).

### D2 — A node or a style may override a value for its subtree

```
column --expo-theme-text-default="var(--slate-11)" --radius-md=4px
  …

style DangerZone
  --expo-theme-button-primary-background="var(--red-10)"
```

- **An attribute spelled `--name`** sets the property on that node, and its descendants inherit it. It is a style row, so a `style` can carry it and `class=` applies it (LLP 1118). Expo's `.dark-theme` scope is not this: a forced scheme is `color-scheme="dark"` (LLP 1034 §8), and `light-dark()` tokens follow it.
- **The value must fit the declared type** (`lower-var-type`). A `<color>` property takes a colour, a `<length-percentage>` one a length.
- **Bound values are allowed**: `--accent-wash=(danger ? "var(--red-3)" : "var(--blue-3)")`. A computed string is checked at run time. If it doesn't fit the type, it is invalid at computed-value time and behaves as `unset`: the node inherits its parent's value, and the runner journals one line, as LLP 1095 D3 does for a computed colour.
- **A cycle** (`--a: var(--b)` on one node and `--b: var(--a)` on another, reached by inheritance) is refused by the compiler when it can see both declarations (`lower-var-cycle`). Otherwise it is invalid at computed-value time, as CSS Variables 1 §3.1 says.

### D3 — `var()` is admitted wherever a style row takes a value

```
text color="var(--expo-theme-text-secondary)"
column padding="var(--space-4)" border-radius="var(--radius-md)"
column box-shadow="var(--expo-theme-shadows-md)"
column width="calc(100% - var(--space-8))"
```

- **Positions:** `var(--name)` and `var(--name, <fallback>)` in any style row's value, including inside `calc()`, `min()`, `max()` and `clamp()`, inside colour functions (`rgb(from var(--red-8) …)`, LLP 1095 D10) and inside lists (`box-shadow`, `background-image` stops, `transition`).
- **In a `style` block:** a `var()` is literal text, so styles can use tokens. This is the main point: Tailwind's `bg-default` becomes `style BgDefault background-color="var(--expo-theme-background-default)"`.
- **Refused in r1:**
  - inside `keyframes` (`lower-var-keyframes`; CSS allows it, and LLP 1095 D9's "known when the animation starts" is the likely rule, §5 Q3)
  - in a prop that is not a style row (`buttonStyle`, `aria-*`): props are not CSS values
  - as a whole `transition` property name
- **Values the host decides stay the host's.** `var()` can only reach the author's own rows. A native button's chrome is still refused (LLP 1069.011.001 D10), whether written as a literal or a `var()`.

### D4 — Substitution, then the row's own parser

- **Order:** CSS's. The row's specified value has its `var()`s replaced by the referenced property's value (or the fallback), recursively, and then the row's parser reads the result. The kernel's parser for that row (`box_shadow`, `color`, a `dimension`) is the same one a literal goes through, so a literal, a `style`, a computed string and a substituted value are refused with the same text.
- **At compile time:** the compiler knows every literal declaration of every property (the theme entry and each literal override). For each use it substitutes each value the property can have there and checks the row. It refuses a combination that can't parse (`lower-var-type`, naming the property, the declaration and the row). This is where a `*` property is checked.
- **At run time:** a substitution that doesn't parse makes the row invalid at computed-value time: the row takes its inherited value if it is inherited, else its initial value, as CSS does. It is never a crash, and it is journalled once.
- **`light-dark()` and references:** substitution happens first, so LLP 1095 D1's rules apply to the result. `light-dark(var(--a), var(--b))` where `--a` is a role is refused there, as a literal pair of references is.
- **A pair substituted into a pair's half** resolves to that half, as CSS resolves a nested `light-dark()` against the same scheme. So `light-dark(var(--slate-11), hsl(from var(--red-8) …))`, with `--red-8` itself a pair, is the pair of `--slate-11`'s light value and the relative colour of `--red-8`'s dark value. Design systems that keep separate light and dark sheets convert to this form (LLP 1119 §4).

### D5 — The kernel resolves per subtree, and re-resolves only readers

- **Storage.** The plan carries a custom-property table: id, name, type, root value. A node's declarations are a small sorted map from id to value (empty for almost every node). A row whose specified value contains `var()` is stored as a **pending-substitution value**: the row's id and its text, interned. It is a new discriminant in the style codec.
- **Resolution** happens during style computation. The kernel walks to the nearest ancestor-or-self declaring the property, else the theme's root value. LLP 1116 D9 specifies this walk for its four properties, and `roles.rs` resolves `var(--exact-<role>)` the same way. The resolved value goes into the row as any other value would, so layout, paint, motion and every host see an ordinary row.
- **Invalidation.** A node's declaration change dirties the nodes in its subtree whose pending rows read that property, directly or through a chain, and nothing else. `set_env` and `uses_env` (`kernel/src/style/env.rs:567`) already invalidate `env()` readers this way. A theme entry is a root declaration, so an over-the-air plan update that retunes `--slate-11` re-resolves every reader.
- **Motion.** A change in a row's resolved value is an ordinary value change, so the row's `transition` applies (LLP 1062). The custom property itself does not animate, as an unregistered one doesn't in CSS. Animating a token is a later decision (§5 Q2).
- **Cost.** A plan with no `var()` has an empty table and no pending rows, so it pays nothing (LLP 1047). The boot path executes nothing new: the table is plan data.

### D6 — The web writes CSS, and the browser resolves it

- **The theme** is a `:root { --slate-11: light-dark(#60646c, #b0b4ba); … }` rule at the head of `app.css`. It is emitted in declaration order with chains as `var()`, never resolved values. Entries no row can reach are dropped by the build, as LLP 1095 D11 drops unused roles.
- **A node's declaration** is the same declaration on its element: in its class rule if it came from a `style` (LLP 1118 D4), else inline. A bound one is written through the JS target's row maps (`host/web-js/src/rows.rs`), as a bound colour is.
- **A pending row** is written as its author text. The browser substitutes, so the web is the oracle for the kernel's resolution, and the conformance run compares them (`host/web-js/conform.mjs`).
- **Typed properties** are registered with `@property { syntax; inherits: true; initial-value }` where CSS can express the type. That makes the browser inherit computed values, as the kernel does, and lets a colour transition. `*` properties are not registered.
- **Colour references** inside a value are written as LLP 1095 D11 writes them (`var(--exact-<role>, <fallback>)`), so the role sheet still decides them.

### D7 — A theme can be imported, and the app's entries win

```
use theme from "./expo/theme.contract"

theme
  --expo-theme-text-link = var(--blue-10)
```

- **`use theme from "<file>"`** merges that file's theme entries into the app's, after the imported file's own `use theme`s. It follows LLP 1017.000 P8's resolution: by path, transitive, `contract-use-cycle` and `contract-use-unreadable` as before.
- **Precedence.** The using file's own entries override imported ones of the same name. This is how an app adjusts a library's token, and the chain through it follows. Two imported files that declare one entry differently are `contract-use-duplicate`, unless the using file declares it too, which settles the conflict.
- **`use Name from`** of a style or component that reads a theme entry does not import the theme. The compiler names the missing declaration (`lower-var-undeclared`) and the `use theme` that would fix it.

### D8 — The agent sees the chain

`layout <node>` reports, for each row with a pending value: the author's text, the chain (`--expo-theme-text-secondary → --slate-11 → light-dark(#60646c, #b0b4ba)`), the node that declared each link, and the resolved value. `tree` is unchanged. This is how an agent answers "which token is this grey?" without the source.

## 3. Declared deviations (land in LLP 1001)

- **Declaration is required.** CSS allows undeclared properties; Contract refuses them (D1).
- **The type is inferred from the root value**, not written. CSS's `@property` states it.
- **Inheritance is the only cascade.** There are no selectors, so a declaration reaches exactly its subtree, as an inline `style` attribute does on the web.
- **`var()` in keyframes is refused in r1** (D3).

## 4. What this deliberately does not do

- **No selectors, no `:root` rule as text, no stylesheets.** The theme is the root rule; a node or `style` is the rest.
- **No registration syntax.** `@property` exists to tell a browser a type. The compiler infers it from the theme, which every property already needs.
- **No custom properties in props or logic.** A breakpoint used in a `when` is a number in Contract, not a CSS value. Named constants for logic are a separate question (LLP 1119 §6).
- **No `env()` changes.** `env()` stays the host's (LLP 1001 §2, LLP 1078 D3); `var()` is the author's.

## 5. Open questions

1. **Inferred or written types?** Inference keeps a theme to one line per token. The risk is a value that parses as two types (`0` is a `<number>` and a `<length>`). Recommendation: infer, and resolve the ambiguity in the narrower type's favour (`<length>` for `0`). An author who needs `*` writes the value so it can't be a single type, or the compiler grows an explicit form when an app asks.
2. **Should a token animate?** CSS animates only registered properties. The kernel could interpolate a typed property and re-resolve its readers per frame. Recommendation: not in r1. Rows already transition when a token's value changes, which covers a theme switch.
3. **`var()` inside `keyframes`.** Resolve when the animation starts, as LLP 1095 D9 proposes for references? Recommendation: yes, with that RFC's rule, once a consumer needs it.
4. **Does `theme` stay the name?** LLP 1095 D4 coined it for colours. With lengths and shadows in it, it is a design system's token table, which "theme" still describes. Recommendation: keep it.

## 6. Landing order

1. **Contract:** `--` attribute names (LLP 1081 D7, amended), `theme` entries with `--` names, `use theme`, the declaration, type and cycle checks. Corpus: `contract/corpus/custom-properties.contract` and reject fixtures per diagnostic.
2. **Kernel:** the plan's table, the pending-substitution value, resolution and invalidation, with unit tests beside `env.rs`'s.
3. **Web and the JS target:** the root rule, declarations, `@property` registrations, and the pending text. Conformance fixtures: a chain, a subtree override, a bound override, an invalid substitution, and a colour reference inside a token. Chrome is the oracle.
4. **Apple and Linux:** nothing new to build. Each host's smoke drives the fixture, compared with the web.
5. **The agent's chain report (D8).**
6. **LLP 1116 D9** re-expressed as four host-registered properties over this machinery, when both have landed.
