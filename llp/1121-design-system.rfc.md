# LLP 1121: A design system in any language — tokens, dials and modes as plan data

**Type:** RFC
**Status:** Draft r1, 2026-10-13. Not reviewed. Nothing built. Builds on LLP 1095 D4 (`theme`), which is being built on a branch with "the reach" (Charlie, 2026-10-12, LLP 1095 §5 item 6); stage 1 below is that work and is not redesigned here. Stage 5 (the editor) needs DEFERRED admissions (§9 Q7).
**Systems:**
- A new leaf crate, `design/` (`exact-design`): the `.design` grammar, its checker and its one evaluator.
- Contract: names in rows, the state parts (D8), `setMode` and `-exact-mode` (D5).
- The plan and the kernel: the reference table of LLP 1095 D1, widened from colours to every token kind (D7).
- The hosts: per-mode selection, the control roles (D9), icon sources (D10), the live table (D13).
- The web host and the JS target: CSS custom properties per mode.
- Generators for TypeScript, Rust, Swift, Kotlin and DTCG (D12).
- The agent API: `prefer` pins a mode; `tree` names the token a row came from (D13).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** none yet (§9)
**Related:**
- LLP 1095 (platform colours): D1 the interned reference and its fallback pair, D4 `theme`, D10 `color-mix()` and relative colour, D11 the web role sheet, §5 item 6 (what the theme port found, and the reach ruling).
- LLP 1115 (write the web, ship the platform) and LLP 1115.000 D6 (press and disabled feedback; Charlie's 2026-10-13 ruling on a styled disabled state).
- LLP 1034 (scheme-aware colour, `color-scheme` on a subtree); LLP 1061 D1 (`press-scale`, and the rejected `active` block); LLP 1069.011.001 and LLP 1104 (native buttons and fields); LLP 1100 (colour spaces).
- LLP 1114 (the Bluesky clone); LLP 1116.012 (round 14: the web's neutral default reads as plain, "the lever is the app `theme`").
- `rules/DEFERRED.md` §Tooling ("no Design Mode … no devtools UI") and §Runtime ("hot revision surfaces").
- exact1's Facet, as research (`llp/research/` and `~/projects/exact1`): LLP 0008, 0269 Part III, 0304, 0451, 0497, 0497.000, 0514 WP5T, `docs/facet-figma-sync-constraints.md`. The survey behind this RFC is `~/.tuft/projects/exact2-theming/facet1-research.md`.
- External: CSS Color 5 (`color-mix()`, relative colour, `contrast-color()`), CSS Fonts (`font` shorthand), the W3C Design Tokens Community Group format (DTCG), Figma Variables (collections × modes).

## Summary

An app names its look once, in a small file that reads like Contract and whose values are
CSS. The file declares **dials** (a few typed inputs: an accent, a neutral, a roundness),
**axes** (named modes beyond light and dark: Bluesky's Dim, a density), and **tokens**
(named colours, lengths, type, shadows, motion and icons, each a CSS value or a CSS
expression over dials, roles and other tokens). One Rust evaluator checks it and resolves
everything that does not depend on the device. What it cannot resolve at build (a platform
role, the person's text size) stays a reference the host resolves, as LLP 1095 already does
for colours.

The plan carries **references, never values**. A row names a token, the plan's reference
table holds the token's value for each mode, and the host picks the row for the modes it is
showing. So a mode switch, a rebrand shipped over the air, and a designer dragging a dial
are all the same operation: replace rows in one table, and repaint (or relayout) only what
reads them. That operation is what powers the editor, the "living Figma" (D13).

**The default design is the platform.** A design names only what a brand changes; what it
leaves unsaid stays the platform's, under LLP 1115's precedence. Nothing here replaces a
platform control with a look-alike (the reach ruling, 2026-10-12).

**No language owns the truth.** The `.design` file is the source; a canonical JSON IR is
generated from it; Contract reads names from the IR; TypeScript, Rust, Swift and Kotlin get
generated typed names and accessors over the same live table; Figma and other tools get DTCG
and Variables. Only Rust evaluates. That avoids exact1's TypeScript resolver and its
hand-ported Rust twin.

## 1. Motivation

### What the Bluesky clone hit

The clone (`~/projects/bluesky-exact2`, LLP 1114) is the first app here with a brand, three
appearance variants and a font-size setting. Each gap below is one a design system closes:

| Gap | What the clone does today | Decision |
|---|---|---|
| Three variants: Light, Dim, Dark (true black). `light-dark()` has two sides. | 244 inline `light-dark(…)` literals (`screens.contract` 112, `post.contract` 70, `sheets.contract` 33, `chat.contract` 24, `app.contract` 5); the palette is a comment (`post.contract:4-7`); Dim and Dark need a palette threaded from the root, about 500 colour reads. | D4, D5 |
| The app cannot scale the person's text size. | Appearance → font size Smaller and Larger replace Dynamic Type through `setRootFontSize(px)` instead of scaling it. | D6 |
| State looks are the host's. | LLP 1115.000 D6 dims a disabled custom pressable on every host, which double-dims a button that styles its own disabled look (ruled 2026-10-13: skip the dim when the app styles it). Pressed is likewise the host's. | D8 |
| Native control styling leaks through. | The disabled Post button draws as a grey prominent-glass pill, not Bluesky's faded blue; the switches were the app's black accent until fixed by hand. | D9 |
| Icons. | About 100 `symbol:sf/…` uses draw nothing on the web; the clone is replacing them with Bluesky's SVGs one by one. | D10 |

### What the rest of the repository says

- **Five apps hand-roll a `Theme` record**, and Expose wrote one colour pair 26 times
  (LLP 1095 §5 item 6). The D4 port took Expose from 274 scattered hex colours to 61 in one
  block, screens identical. It also found what D4 leaves out: typefaces and sizes (Expose
  writes `font-family` 48 times), radii, strengths of one colour, and "a palette the person
  chooses at run time, which a declared-once block carries only as state choosing between
  names at each use". Those are D3, D4 and D5 here.
- **T3 Code's theme editor** outlines every node a colour role paints and counts its uses
  (`issues/closed/20261010-paint-role-node-list-read.md`). The clone needed an app-wide
  `data-paint` word on every themed node and a new `querySelectorAll` read to approximate
  it. When rows hold references, "where is this token used" is a lookup (D13).
- **Round 14** graded the web's neutral default plain; "the lever is the app `theme`"
  (LLP 1116.012).

### What exact1's Facet taught

Facet resolved a few dials into ~200 semantic tokens, fed recipes, and fed components in
React, Contract and Rust. What was right and what failed, by the survey:

| Facet | Lesson | Here |
|---|---|---|
| Definition (dials) → deterministic resolver → semantic tokens; runtime inputs (scheme, contrast, text scale, platform) kept apart from the design | Right | D2, D4, D6 |
| v2's purpose-named taxonomy, identifier-safe paths (`s4`, `xxs`), on-colours picked by the fill's luminance, concentric radius as an apply-time helper, density as a table of control sizes | Right | D3, D11 |
| Tokens as TypeScript objects "by decree" (0008); the resolver in TS; a second, hand-written Rust resolver held to TS by a parity corpus that even reproduced JS rounding (0514 WP5T) | The root failure | D2: data, one Rust evaluator |
| Recipes as TS ternary trees, a sidecar descriptor and a generated table, with nothing making them agree (0497) | One representation | D8: state looks are rows |
| `type.ratio` and the status hues went inert once Figma pinned values; a fixture had to catch dials that move nothing (0269) | Every dial must move a token | D4's completeness check |
| Dials and Figma-as-authority never reconciled | Decide the authority per class | D14 |
| Apps kept private palettes and raw px; "real-app adoption C+" (0304) | App tokens first-class, and tokens the easy path | D11 |
| A theme flip was O(tree) on native; the kernel had no token concept. 0497's answer: swap data, not code, with dependency domains | Swap rows | D7 |
| Design Mode was DOM-only, because provenance came from the DOM mirror | Provenance from references | D13 |
| ~35 LLPs, 6 authorities, ~30 checks, three generators and a closure ritual | One file, one command | D2, D12 |

Exact2 has also already refused what Facet was built from: a second value graph, JS above
the data seam, package imports of theme code (`contract/corpus/rejects.txt` keeps
`use theme from "@exact/facet-contract"` as a refusal), and selectors. "The tokens are
data" is what survived (`llp/1033-markdown-viewer.plan.md:493-511`). This RFC is that sentence, built out.

## 2. Decisions

### D1 — The platform is the default design; a design names only what it changes

LLP 1115's precedence stands and gains one layer: **the person's accessibility settings >
the author's row > the design > the platform > CSS's default.** A design is the app's
declared brand; a row on a node still wins over it; what neither says is the platform's.

- **Colour semantics stay the platform's roles** (LLP 1095 D2). A design does not invent a
  parallel set of semantic colours: an app's "secondary text" is `-exact-secondary-label`
  unless its design overrides that role, and overriding a role reaches every surface the
  platform lets it reach and stops where the platform offers no supported way (the reach
  ruling; an alert or a system menu stays the platform's). The compiler reports what an
  override cannot reach on each target, as a lint naming the surface (D9).
- **An app with no `.design` file is exactly today's app.** Every table is empty, the plan
  is byte-identical, and no host does any work.
- **A design never draws a control itself.** Where a brand wants a look the native control
  cannot take (a faded-blue disabled glass button), the author writes their own control: a
  styled pressable, which is the author's row and wins (D9).

*Alternative rejected:* a neutral Facet-like brand as the default. It is what LLP 1115 was
written to stop: a look that "was not made here" on every platform.

### D2 — One source, the `.design` file; a canonical IR; one evaluator, in Rust

**The source is a `.design` file** (`app.design` beside `app.contract`, or named in
`app.json` as `"design"`). Its grammar is Contract's lexical grammar: indentation blocks,
`name = value` lines, `//` comments. Its **values are CSS** (§3): any CSS colour, length,
time, `cubic-bezier()`, a `font` or `box-shadow` shorthand, and a subset of CSS's
functions as the expression language. An author who knows Contract and CSS reads it on
first sight; one who writes only Rust, TypeScript, Swift or Kotlin reads it as a
configuration file with CSS values.

LLP 1095 D4's `theme` block, written in a `.contract` file, stays legal: it is a `.design`
file's `theme` block written inline, with the same grammar and the same checks. An app
uses one or the other; both is refused (`design-two-sources`).

**`exact-design` is a leaf crate with one evaluator.** It parses, checks (names, types,
ranges, cycles, the completeness rule of D4) and resolves. It is used by:
- the Contract compiler, which links it and writes the result into the plan (D7);
- `exact design` (a CLI verb of `scripts/exact.mjs` over the same crate): `check`, `fmt`,
  `eval --mode dark-style=dim`, `gen <target>` (D12), `export dtcg` (D14);
- the dev server, for the editor's live table (D13), natively;
- the web, only inside the dev server's editor page, as wasm. A shipped web build carries
  resolved tables and CSS, never the evaluator.

Hosts (Swift, Kotlin, the web runtime, Linux) **never evaluate an expression**. They select
a resolved row per mode and resolve the host references LLP 1095 already resolves (roles,
platform colours, `color-mix()` over them, the root font size).

**The IR is generated, never committed** (DEFERRED §Process): `design.json`, versioned by a
format number and the schema digest. It is the input to every generator and to the
editor's panels, so no generator parses the DSL.

**`exact design fmt` owns the file's layout.** The editor writes through it (D13), so an
edit made by dragging a slider is a one-line diff in a file a person can still edit by
hand, with comments kept.

*Alternatives rejected:*
- **TOML or JSON as the source.** Per-mode values and expressions read badly in both, and
  a hand-editable source with comments is what the editor round-trips through.
- **DTCG as the source.** It has no dials, no expressions and no modes beyond a vendor
  extension; it is an export (D14).
- **Contract alone.** Rust, Swift and Kotlin projects that embed Exact (the sample host, LLP
  1031; native modules, LLP 1024; the hatches of LLP 1075.003) need the design without
  writing a view in Contract. The grammar is shared, so nothing is lost.
- **A TypeScript or Rust API as the source.** That is Facet's root failure in either
  language.

### D3 — Token kinds and names

A token has a **name**, a **kind** and a **value**. The kinds, and the kernel row codecs
they feed:

| Kind | Value (CSS) | Rows that take it | Domain |
|---|---|---|---|
| `color` | any colour, `light-dark()`, a role, `color-mix()`, relative colour, `contrast-color()` | every colour row and embedded colour (LLP 1095 D1's list) | paint |
| `length` | px, `rem`, `em`, `calc()`/`min()`/`max()`/`clamp()` of these | length rows: padding, gap, size, `border-radius`, `border-width`, insets | layout |
| `number` | a number, `calc()` | `opacity`, `line-height`, `flex-grow`, `press-scale`, `font-weight` | paint or layout, by row |
| `type` | the CSS `font` shorthand: style, weight, size, `/line-height`, family; size may be a text style (`-exact-body`) | `font`, which sets the five rows as `font="-exact-footnote"` sets size today | text |
| `family` | a font-family stack | `font-family` | text |
| `shadow` | `box-shadow` syntax, layers allowed | `box-shadow`, `text-shadow` | paint |
| `duration`, `easing` | CSS time; `cubic-bezier()`, a keyword, or Exact's spring | `transition` parts, `animation-duration` | paint |
| `icon` | `icon(…)` (D10) | `symbol`, `src` of an image used as an icon | paint |

**Names are dotted identifiers**: `surface.card`, `text.link`, `space.s4`, `radius.md`,
`type.body`, `icon.reply`. A segment is an identifier, so `s4` not `4` and `xxs` not `2xs`
(Facet's rule, which Figma's names had to follow too). The dot is what tells a token from
a role or a CSS keyword: roles are undotted (`-exact-secondary-label`, `AccentColor`). A
token with an undotted name is a **role override** (LLP 1095 D4: `AccentColor`,
`secondary-label`), and only an existing role's name may be undotted.

**A row names a token as a string**, as LLP 1095 D4 names a theme colour:
`background-color="surface.card"`, `padding="space.s4"`, `font="type.body"`,
`border-radius="radius.lg"`. Inside an expression a token is a bare name:
`padding="calc(space.s4 * 2)"`, `color="color-mix(in srgb, text.link 60%, transparent)"`.
The compiler checks every literal name against the design and the row's kind
(`design-unknown-token`, `design-kind`); a name computed at run time (a string from data) is
resolved by the runtime parser against the plan's table, as LLP 1095 D4 does for `"brand"`,
and an unknown one is an invalid value in the log.

*Alternative considered:* CSS's own spelling, `var(--surface-card)`. It is what a web
author knows, and custom properties inherit, which would give subtree theming for free.
Rejected for r1: it imports CSS's cascade semantics (a custom property set on any node
overrides it for the subtree), which Contract has refused (`style` blocks are literal and
`class` merges without cascading), and D4's bare names are already built and ported.
§9 asks.

### D4 — Dials: typed inputs that tokens are computed from

```
dials
  accent: color = #1083fe
  neutral: color = #6f869f
  roundness: number = 1  in 0..2
  spacing: number = 1  in 0.75..1.5
```

- **A dial is a typed, ranged input**: `color`, `length` or `number`, with a default and,
  for numbers and lengths, a closed range. The editor draws a control from the type (a
  colour well, a slider); an agent sets it by name. Out-of-range values are refused, not
  clamped.
- **Tokens are expressions over dials**, roles and other tokens:
  `radius.md = calc(8px * roundness)`; `space.s4 = round(calc(16px * spacing), 1px)`;
  `accent.fill = accent`; `accent.on-fill = contrast-color(accent.fill)`.
- **An enumerated choice is an axis, not a dial** (D5). Facet's `density` and
  `radius.scale` enums become an axis (`density = compact | default | comfortable`) and a
  number (`roundness`), so a token has one way to vary by a choice.
- **`ramp(color, steps…)`** makes a palette from one colour: OKLCH lightness at fixed
  stops, chroma scaled toward the ends, specified as a table in the crate with golden
  tests. It is the one generative function beyond CSS's, and it arrives in stage 3, not
  before, because a designer's palette (Bluesky's) is usually hand-tuned, and those values
  are written as literals with a dial only where the brand is meant to move.
- **Completeness:** every dial must move at least one token, and every token that a dial
  reaches lists the dial in the IR. `exact design check` refuses a dial that moves nothing
  (`design-dial-inert`). This is the check Facet had to add after `type.ratio` went inert.
- **A dial is a design-time input.** In a shipped app its value is fixed and its tokens are
  resolved at build; changing a dial at run time is the editor's (D13), not an app's. An
  app that lets the person pick (Bluesky's Dim) uses an axis.

### D5 — Axes: named modes beyond light and dark

```
axes
  dark-style = dim | black  under scheme=dark
  density = compact | default | comfortable
```

- **`scheme` (light | dark) and `contrast` (standard | more) are built-in axes**, set by the
  host as today: appearance, Increase Contrast, `prefers-contrast`. They stay binary
  because the platforms are: UIKit's trait collection, AppKit's appearance and the
  platform's own controls know only light and dark, and a Dim page must still get dark
  platform controls.
- **An app axis is declared in the design** with its values, the first the default.
  `under scheme=dark` says the axis only matters there; a `when` on it implies the scheme.
- **A token varies by axis with `when` lines** under its default value; the most specific
  `when` that holds wins, and two equally specific ones that both hold are refused
  (`design-when-ambiguous`):

```
theme
  surface.page = light-dark(#ffffff, #000000)
    when dark-style=dim: #161e27
  text.primary = light-dark(#0b0f14, #f1f3f5)
  space.row = 12px
    when density=compact: 8px
    when density=comfortable: 16px
```

- **The app picks a value with a host command**, `setMode("dark-style", "dim")`, beside
  `setScheme` and with the same reach: the whole window, re-resolved in the action's own
  commit. A literal axis or value that the design does not declare is refused at compile.
- **A subtree pins a mode** with `-exact-mode="dark-style:dim"`, as `color-scheme="dark"`
  pins the scheme on a subtree (LLP 1034 §8). That is how a settings screen previews all
  three appearances side by side, and how the editor shows a component in every mode.
- **The mode vector is finite and small.** The evaluator resolves every token for every
  combination that changes its value, and the plan stores only the distinct values per
  token (a token that does not vary by `density` has one row for it). A design whose axes
  multiply past a cap (64 combinations in r1) is refused, which keeps the table bounded.
- **Bluesky, written out:** the root's three-way setting becomes
  `setScheme("light"|"dark"|"system")` plus `setMode("dark-style", "dim"|"black")`; no
  palette is threaded, and the 244 literals become names.

### D6 — Text scale is an input, and type is relative to the platform's

- **Type tokens are relative to the platform's text styles and the root size.**
  `type.body = 400 -exact-body/1.375 brand-sans` takes the platform's body size, which
  follows Dynamic Type on iOS and the browser's setting on the web (LLP 1115 D3).
  `calc(1rem * 0.9375)` is legal too. A px size is legal and is the author's choice not to
  scale; `exact design check` warns on it (`design-type-fixed`), since a fixed type size is
  a tell under Dynamic Type.
- **`setRootFontSize` gains a scale form**: `setRootFontSize(scale=1.125)` multiplies the
  host's own size instead of replacing it, and keeps following it (a Dynamic Type change
  under a scale of 1.125 is still scaled by 1.125). `"medium"` still hands the size back.
  This is the Bluesky font-size setting: Smaller and Larger become scales over the person's
  setting. It amends the command in `docs/contract-grammar.md` (§Host commands); it is a
  small change in each host's root-size path, and it ships in stage 2 whether or not the
  rest does.
- **Whether the scale persists across a launch** is the same question as `setMode`'s
  (§9 Q6).

### D7 — The plan carries references, never values

This generalises LLP 1095 D1 and D4 ("the theme ships in the plan as part of the reference
table") from colours to every kind.

- **One reference table in the plan**, one row per token, built from the IR:
  `{ kind, domain, values: [(mode mask, value)], fallback }`. A value is either a constant
  (a resolved colour, length, shadow …) or a **residual**: an expression the build cannot
  finish because it reads something only the host knows. The residuals are exactly the
  forms the hosts already resolve or that LLP 1095 designs: a role, `platform-color()`,
  `color-mix()` and relative colour over a role (D10, the `Mix` table), and `rem`/text-style
  lengths. Every other expression is folded at build.
- **Row values point into the table.** `ColorValue` gains no variant beyond D1's `Ref(RefId)`,
  which the reach work is adding; length, number, shadow and font rows gain one `Token(u16)`
  form each. `ColorValue` stays `Copy + Eq` (D1's constraint), and so do the others.
- **Every row has a fallback** (D1's rule): the value for the default modes, with roles
  resolved to their fallback pair. So the kernel, Linux, the agent's headless runs and the
  first frame always have a deterministic value without asking a host.
- **The kernel resolves a reference against the node's mode vector**: the window's modes,
  overridden by the nearest `color-scheme` or `-exact-mode` above the node. It keeps, per
  token, the nodes whose rows read it, built at mount, so a row swap or a mode change marks
  only those nodes, and only for the token's domain:
  - **paint** (colours, shadows, opacity, icons): repaint, no layout;
  - **layout** (lengths, layout numbers): relayout of the nodes' subtrees;
  - **text** (type, families): reshape and relayout.
  This is RFC 0497 §4.2's dependency domains; exact1 needed them because a flip re-ran the
  tree.
- **Native hosts keep their dynamic colours.** A colour token whose values differ only by
  `scheme` and `contrast` is handed to UIKit and AppKit as one dynamic colour, as roles are
  (LLP 1095 D5), so the OS re-resolves it on an appearance change with no Exact work. A
  token that also varies by an app axis is resolved by the kernel per mode vector and sent
  as that dynamic colour for the current app modes; a `setMode` replaces it.
- **The web gets CSS custom properties.** Each token used in the build is a property,
  `--x-surface-card`, defined in the page's head: one rule per app-axis combination that
  changes it (`:root[data-x-dark-style="dim"]`), `light-dark()` inside for the scheme, and
  `@media (prefers-contrast: more)` for contrast. Residual mixes are emitted as CSS's own
  `color-mix()` over the properties, which the browser resolves. `setMode` sets the root's
  data attribute; `-exact-mode` sets it on the subtree's element. Pay for what you use
  (LLP 1047): only tokens a build references are emitted. This and LLP 1095 D11's role sheet
  are one generator.
- **Wire and compatibility.** The new row forms change the kernel schema digest once, so
  every native host needs a new binary then (LLP 1030 D3a), as D1 did. After that a token,
  a value or a new axis value is plan data: a bundle carries it and an update replaces it
  over the air (LLP 1095 §3).

### D8 — State looks are rows: a state part admits paint rows only

Facet kept states in TS recipes; exact2 has `press-scale` (a row, LLP 1061 D1) and the
host's default press and disabled feedback (LLP 1115.000 D6). The Bluesky gaps are a
pressed look and a disabled look that the author wants to own.

**A state part is a block under a node or in a `style`, as `::placeholder` is:**

```contract
style PostButton
  background-color = "accent.fill"
  color = "accent.on-fill"
  border-radius = "radius.full"
  :active
    background-color = "accent.fill-pressed"
  :disabled
    background-color = "accent.fill-faded"
```

- **The states are CSS's pseudo-class names:** `:active` (pressed), `:hover`,
  `:focus-visible`, `:disabled`, and `:checked` / `[aria-selected]` for a toggle and a
  selected row. No combinations, no nesting, no other selectors.
- **A state part admits paint rows only:** `background-color`, `color`, `border-color`,
  `opacity`, `box-shadow`, `-exact-tint-color` and `press-scale` (its scale). No layout row,
  so a state never relayouts. Another row is `lower-state-row`.
- **Two lowerings, by who knows the state:**
  - **Runner-known** (`:disabled`, `:checked`, `[aria-selected]`): the runner already has the
    bound attribute, so the part lowers to the conditional row values an author could write
    today (`background-color=(disabled ? … : …)`), with no new host work.
  - **Host-known** (`:active`, `:hover`, `:focus-visible`): the press, the hover and the
    focus are the host's, with no round trip (LLP 1061 D1's reason). The part lowers to a
    **state look**: an interned table entry `{ state, rows: [(row, value)] }` the node points
    to, which the host applies on its own press, hover and focus tracking, with the same
    120 ms ease it gives `press-scale` today. A state look's values may be token references,
    so they follow the modes too.
- **A declared state part replaces the host's default for that state**, and only that one:
  a node with `:disabled` is not dimmed (Charlie's 2026-10-13 ruling, made a rule of the
  grammar rather than a guess from the rows); a node with `:active` gets no platform press
  dim; a node with neither keeps both defaults. `press-scale="0.97"` is still written as a
  row and means `:active { press-scale: 0.97 }`.
- **This amends LLP 1061 D1**, which rejected "an `active` pseudo-state block in `style` (a
  selector system for one property)". The cost-benefit has changed: the block now carries
  six rows and four states, and the host already tracks the states. It remains a part, not
  a selector: it names the node's own state, never another node. §9 asks.

**Recipes are `style` blocks.** A token-referencing `style` with state parts, chosen by
`class` (which merges without cascading), is a recipe: one representation, data, editable.
No component library or anatomy schema is proposed here (§8).

### D9 — Native controls: a control reads named roles, with fidelity declared per host

A native control takes its colours from the platform's own parameters, never from Exact
drawing over it. The design reaches a control by **control roles**: role names Exact defines
in the schema, each with a default of an existing role, which each host maps to the one
parameter the platform supports for it. Overriding one is a role override (D3, D1).

| Control role | Default | iOS | macOS | Web | Android |
|---|---|---|---|---|---|
| `switch-on` | `AccentColor` | `UISwitch.onTintColor` (exact) | `NSSwitch` takes the accent only (none) | `accent-color` (exact) | `SwitchColors.checkedTrackColor` (exact) |
| `slider-fill` | `AccentColor` | `minimumTrackTintColor` (exact) | `NSSlider` tint (best effort) | `accent-color` (exact) | `SliderColors.activeTrackColor` (exact) |
| `progress-fill` | `AccentColor` | `progressTintColor` (exact) | none | `accent-color` (exact) | exact |
| `control-tint` | `AccentColor` | the button's tint; a prominent glass button's tint (exact when enabled) | `bezelColor` (best effort) | the designed default's accent (exact) | exact |
| `field-text`, `field-background` | the field roles | the field's `textColor`, background (exact) | exact | exact | exact |

(The full list is a schema table, `controlRoles`, with a column per host; this table is its
shape. Values above are from the hosts' current code paths and LLP 1069.011.001; each is
to be measured before it lands, as LLP 1095's were.)

- **Fidelity is declared, per role and host:** `exact`, `best-effort` (the platform takes it
  but alters it, as AppKit's bezel does), or `none`. The compiler lists, for an app's design
  on each target it ships, every override that is `none` there
  (`design-unreachable: switch-on on macOS: NSSwitch takes only the system accent`). That
  is a lint, not an error; the platform's look on that host is the answer, as the reach
  ruling says.
- **A state the platform owns stays the platform's.** iOS draws a disabled prominent glass
  button grey; no supported parameter changes that. So the Bluesky Post button's faded blue
  is not a control role: the clone writes the Post button as its own styled pressable
  (`PostButton` above, D8), which is the author's control. The lint for a design that sets
  `control-tint` says, on iOS, "a disabled prominent button is drawn by the platform".
- **Switches follow the design without a hand fix.** `switch-on = accent.fill` in the
  design, or nothing at all where the brand's accent is the root's `accent-color`, colours
  every switch.

### D10 — Icons are tokens with a source per platform

```
icons
  icon.reply = icon(sf "bubble.left", material "chat_bubble_outline", svg "icons/reply.svg")
  icon.like = icon(sf "heart", svg "icons/heart.svg")
  icon.like-on = icon(sf "heart.fill", svg "icons/heart-fill.svg")
```

- **An icon token maps one name to a source per platform:** an SF Symbol on Apple, a
  Material Symbol on Android, an SVG under the app's `assets/` everywhere else and as the
  fallback. The schema's 76 `symbols` (an SF name and an SVG path each) become the base set
  of icon tokens, so `symbol="back"` keeps working.
- **A use is checked against every target the app ships** (`app.json`): an icon with no
  source for a target and no `svg` fallback is refused at compile
  (`design-icon-missing: icon.share has no source for web`). That is the Bluesky clone's
  100 blank `symbol:sf/…` uses, caught at build instead of drawing nothing.
- **An icon takes its colour from the colour rows** (`-exact-tint-color`, `color`) as a
  symbol does, so a colour token tints it. An SVG icon is drawn as a template (its alpha as
  a mask), as an SF Symbol is, unless its token says `multicolor`.
- **An icon's size follows its type** (`font`), as SF Symbols scale with text, unless a
  length is written.

### D11 — App tokens and package tokens are first-class; Exact ships a small base

**There is one table.** Exact's base names, an app's tokens and a package's tokens are
rows in the same reference table, checked by the same compiler and shown in the same
editor. Facet's adoption failure (apps kept private palettes and raw px) came from app
tokens being second-class; there is no second class here.

**Exact's base design is small, and every value in it is the platform's:**
- colours: the roles (LLP 1095 D2), plus the control roles (D9). No new colour names.
- type: the platform's text styles as `type.*` (`type.body` is `-exact-body`, and so on).
- spacing, radius, motion: `space.s1…s24` (4 px steps, Facet's set), `radius.sm…xl` and
  `radius.full` at the platform's own corner radii where it has them (LLP 1115.000's
  measured metrics), `motion.fast/normal/slow` and `easing.standard/enter/exit` at the
  platform's durations and curves.
- density: `control.height.sm/md/lg`, `row.height`, `hit.min` (Facet's table, which already
  had the platform minimum hit targets).

That base exists so a **package** can be themed by any app: `packages/reading` writes
`padding="space.s4"` and `font="type.body"`, and an app's design changes them for every
package it uses. A package may declare its own tokens with defaults in its own `.design`
file, named under its package (`reading.measure`); the app's design may set them. That is
Facet's component params, without a component system.

**Precedence among designs:** the app's design over a package's over Exact's base. A name
two packages both declare is refused unless the app sets it.

### D12 — Every language gets generated names over the one table

`exact design gen <target>` writes bindings from the IR. Generated, never committed; the
build regenerates them, as `contract build` already rewrites the types `app.ts` imports.

| Target | What it gets | Who uses it |
|---|---|---|
| Contract | nothing generated: the compiler reads the IR and checks names (D3) | views |
| TypeScript | a `Token` string-literal union per kind, so a data module that returns a token name (`"surface.card"`) is checked by `tsc`; for code outside Exact (a deck, a React page), `design.css` with the custom properties and `design.ts` with resolved constants per mode | `app.ts` data modules; the web outside a plan |
| Rust | `pub const SURFACE_CARD: Token<Color>` and the like, and `Design::resolve(token, &modes)` over the plan's table at run time | data crates that return names; the terminal host; a game's Canvas 2D; a Rust native module |
| Swift | `ExactDesign.color(.surfaceCard)` returning a dynamic `UIColor`/`NSColor` over the session's live table; `font(.typeBody)`; `image(.iconReply)` | native modules (LLP 1024), the hatches of LLP 1075.003, the sample host's own screens (LLP 1031) |
| Kotlin | `ExactDesign.colors.surfaceCard` as Compose `Color`s over a `CompositionLocal` fed from the live table | Android native modules and host screens |
| DTCG JSON | every token, per mode, with `$type`; dials as an extension | Figma via Tokens Studio, Style Dictionary (D14) |

- **Swift and Kotlin read the session's table, not a copy**, so native code beside Exact
  changes with `setMode`, with the editor and with an over-the-air design, and matches the
  Exact views next to it.
- **Names are mapped by one rule per language** (dots and hyphens to camelCase in Swift,
  Kotlin and TypeScript property names; to `SCREAMING_SNAKE` in Rust), and a design whose
  names collide after mapping is refused.

### D13 — The editor and the live table

The "living Figma" is a design editor that edits the `.design` file through structured
operations, and every running build of the app on the dev server shows each edit at once,
on every host, without a reload.

**The operations** (one vocabulary for the editor's panels and for agents):

| Operation | Edits | Effect on running apps |
|---|---|---|
| `set-dial name value` | a dial's default | rows for every token it reaches |
| `set-token name [when …] value` | a token's value, for a mode or the default | that token's rows |
| `set-state name part state row value` | a state part in a `style` (D8), in a `.contract` file through `contract fmt` | that state look |
| `add-token`, `rename-token`, `add-axis-value` | structure | the normal dev reload (below) |
| `preview mode=value` | nothing in the file | the editor pins a mode on a connected app, as `prefer` does |

**Each operation:** validates against the design (a refused op changes nothing and says
why), writes through `exact design fmt` (a one-line diff), re-evaluates in the dev server's
evaluator, and, for a value operation, sends the changed table rows to every session
connected to the dev server. The kernel replaces the rows and invalidates by domain (D7). No
plan is recompiled, no state is lost, nothing restarts. A structural operation goes through
the dev loop's ordinary rebuild and reload, which carries state (LLP 1007 §6).

**Why this is a value swap, not a hot revision.** DEFERRED §Runtime defers "hot revision
surfaces": no patch format, no generations, no identity matching. A row swap changes no
code, no slot and no tree: it is `setMode`'s mechanism with new values in the rows, and it
exists in development builds only. A shipped app's design changes by an update (D7).
Even so, it is a new development surface, and §Tooling defers Design Mode and devtools UI,
so stage 5 waits on Charlie's admission (§9 Q7).

**Provenance is a lookup.** Because rows hold references:
- **Where is this token used:** the kernel's token → nodes index (D7) answers it for every
  mounted node, on every host: T3 Code's "Accent color · 4 uses" outline with no `data-paint`
  word and no selector read.
- **Explain this pixel:** the agent's `tree` reports, for a row that holds a reference, the
  token's name, the `when` that won and the dial chain (`background-color: surface.card ←
  neutral (dial)`). The editor draws the same.
- **The agent pins modes** with `prefer`, as it pins scheme and contrast (LLP 1095 D7):
  `prefer mode dark-style=dim`. Tests and `verify` screenshots name their modes.

**The editor is an Exact app**, served by the dev server and connected like any client: a
dials panel (controls from the dial types), a token × mode grid (Figma Variables' layout,
with resolved swatches and contrast badges between declared on-pairs), and a preview pane
of the app's own screens pinned per mode with `-exact-mode` (D5). Its web build runs the
evaluator in wasm; its native builds use the dev server's.

**Security.** The swap channel exists only between a development build and the dev server
that built it, over the connection the dev loop already authenticates. A release build has
no swap channel and no editor. Designers on shared builds are out of r1 (§9 Q7).

### D14 — Figma and other tools: the design file is the authority; Figma is a peer

- **The authority, per class** (RFC 0497's open precedence, decided):
  - **names, kinds, axes and structure:** the `.design` file. A tool cannot add a name.
  - **values:** the `.design` file, edited by people, the editor, agents or an import. An
    import proposes; a person or agent accepts.
  - **dials:** the `.design` file. A value derived from a dial is never overwritten silently.
- **Export.** `exact design export dtcg` writes DTCG JSON: one group per kind, modes as
  Tokens Studio's theme sets, primitives (literal values with no dial) apart from semantic
  tokens. A Figma Variables writer (one collection per kind, modes = the axis combinations)
  is a later step and needs Enterprise access (§9 Q8).
- **Import is a proposal.** A changed value in DTCG or Variables becomes a list of
  `set-token` operations with a diff. A change to a token that a dial computes is offered
  two ways: as a dial change, where one dial explains it (an accent's hue), or as a leaf
  override that the editor marks as pinned ("no longer follows `accent`"), so a dial can
  never go inert unseen. Facet never made this choice explicit; that is how `type.ratio`
  died.
- **No live two-way canvas sync.** Figma's canvas cannot be written headlessly and its
  scene graph carries no semantics (exact1 LLP 0389, 0395). The editor of D13 is the living
  Figma; Figma is an import and export peer. Penpot, which is open, is the better second
  provider if one is wanted.

## 3. The `.design` grammar, in brief

```ebnf
design-file = { block } ;
block       = "axes" NL block(axis) | "dials" NL block(dial) | "theme" NL block(token)
            | "icons" NL block(token) ;
axis        = IDENT "=" IDENT { "|" IDENT } [ "under" IDENT "=" IDENT ] NL ;
dial        = IDENT ":" ( "color" | "length" | "number" ) "=" value [ "in" value ".." value ] NL ;
token       = NAME "=" value NL [ INDENT { when } DEDENT ] ;
when        = "when" cond { cond } ":" value NL ;
cond        = IDENT "=" IDENT ;
NAME        = IDENT { "." IDENT } ;
value       = CSS value text, read by the crate's CSS value parser ;
```

**The expression subset** is CSS's: `calc()`, `min()`, `max()`, `clamp()`, `round()` over
numbers and lengths; `color-mix(in srgb|oklab|oklch, …)`; relative colour (`rgb(from x r g
b / a)`, `oklch(from x l c h)` with `calc()` on channels); `contrast-color()` (white or
black against a colour, by CSS Color 5); `light-dark()`; plus Exact's `ramp()` (D4) and
`icon()` (D10). Colour arithmetic is in f64 and quantised once, at the end, to the row's
storage (8-bit sRGB today, LLP 1100's wide row where the value is wide), so every host
gets the same bits because only one evaluator produces them.

A token's kind is inferred from its value where it can be (a colour literal) and written
where it cannot (`space.row: length = 12px` is allowed; `space.row = 12px` infers it).

**Example: the Bluesky clone's design, in part.**

```
design bluesky

axes
  dark-style = dim | black  under scheme=dark

dials
  accent: color = #1083fe

theme
  AccentColor = accent                        // the role override: the root accent, the tint, switches (D9)
  surface.page = light-dark(#ffffff, #000000)
    when dark-style=dim: #161e27
  surface.raised = light-dark(#f1f3f5, #0f1419)
    when dark-style=dim: #1e2936
  text.primary = light-dark(#0b0f14, #f1f3f5)
  text.secondary = light-dark(#42576c, #8c9eb2)
    when dark-style=dim: #aebbc9
  border.hairline = light-dark(#d4dbe2, #2e4052)
  accent.fill = light-dark(accent, #208bfe)
  accent.fill-pressed = color-mix(in oklab, accent.fill 88%, text.primary)
  accent.fill-faded = color-mix(in srgb, accent.fill 50%, surface.page)
  accent.on-fill = contrast-color(accent.fill)
  type.body = 400 -exact-body/1.35 "InterVariable", -exact-system-ui
  radius.full = 999px

icons
  icon.reply = icon(sf "bubble.left", svg "icons/reply.svg")
```

(The values are illustrative; the port takes them from the clone's own palette.)

## 4. What this deliberately does not do

- **No runtime theme library, no JS above the data seam, no package import of theme
  code.** A design is plan data.
- **No selectors and no cascade.** A state part names a node's own state (D8); a mode is
  pinned on a subtree as `color-scheme` is (D5). Neither selects other nodes.
- **No look-alike controls.** A design reaches a native control through the platform's
  supported parameters or not at all (D9).
- **No component library, no anatomy schema.** `style` blocks with tokens and state parts
  are recipes (D8). Facet's 47 components × 3 bindings were its most expensive part; a
  library waits for a consumer (§9 Q5).
- **No evaluation on a host.** Swift, Kotlin and the web runtime select rows.
- **No live two-way Figma sync** (D14).
- **No per-element `color-scheme` beyond LLP 1034's**, and no third value of `scheme`: Dim is
  an app axis under dark (D5).
- **No dial changes at run time in a shipped app.** The person's choices are axes (D4, D5).

## 5. Staging and gates

Each stage lands on its own gate. No stage starts before the previous one's gate passes,
except stage 2's `setRootFontSize` scale, which is independent.

1. **LLP 1095 D4 and the reach** (in progress on its branch, not redesigned here). The plan's
   colour reference table, role overrides reaching every supported surface. Gate: LLP 1095
   §5 item 6's measurement and Charlie's word.
2. **The `.design` file, every token kind, app axes.** The crate; `theme` blocks inline or as
   a file; length, number, type, shadow, motion tokens as plan references (D7); `setMode`,
   `-exact-mode`, `prefer mode`; the web's custom properties; `setRootFontSize(scale=)`.
   **Gate:** the Bluesky clone's 244 `light-dark` literals become names with Light, Dim and
   Dark switched from the root and no palette threaded; every compared screen identical on
   web, iOS and Mac in all three; its font-size setting scales Dynamic Type (checked at two
   Dynamic Type sizes); Expose and the readers stay identical; an app without a design has a
   byte-identical plan; a mode switch on a 1,000-row list repaints and does not relayout
   (paint domain, measured with `perf`).
3. **State parts and control roles** (D8, D9), and `ramp()`. **Gate:** the clone's Post
   button and switches are right on every host with no hand fix and no double dim; the
   `design-unreachable` lint names the disabled glass button on iOS; LLP 1061's
   `press-scale` corpus is unchanged.
4. **Icons** (D10). **Gate:** the clone's ~100 `symbol:sf/…` uses draw on the web from icon
   tokens; a missing source fails the build with the target named.
5. **The editor and the live table** (D13). **Needs a DEFERRED admission (§Tooling,
   §Runtime)**. **Gate:** a dial drag in the editor reaches the clone running on the iOS
   simulator, the Mac and the web at once, within one frame of the dev server's evaluation
   plus the network, with no state lost (a scrolled list stays scrolled, a draft stays
   typed); the `tree` provenance and the uses count are right on every host.
6. **Bindings and export** (D12, D14). Swift and Kotlin accessors, TS types, Rust consts,
   DTCG. **Gate:** a native module in the sample host draws with `ExactDesign.color(…)` and
   follows `setMode` and the editor; a DTCG export round-trips through Tokens Studio with no
   diff; a Figma import of a changed accent is proposed as a dial change.

**Checks that ship with stage 2 and stay:** `design-dial-inert` (D4); determinism, by golden
IR and resolved tables for the corpus designs (one evaluator, so no cross-language parity
corpus is needed); the plan-size budget (each token costs its distinct values; the clone's
design must add less than 4 KB to its plan); first frame unchanged within noise for an app
with a design.

## 6. Cost

- One crate, the Contract compiler's design pass, the new row forms (one schema digest
  change), the kernel's token index and mode vector, the hosts' per-mode selection, state
  looks on each host's press path, the control-role table, the web generator.
- Docs: one section in `docs/contract-for-agents.md` (tokens, axes, state parts) and one
  recipe in `docs/recipes/`. The required read (`docs/start-here.md`) grows by a line: "a
  brand is an `app.design`; a colour or size names a role or a token".
- What it removes: per-app `Theme` records, threaded palettes, inline `light-dark` pairs
  repeated per file, the clone's hand-fixed switches, the `data-paint` workaround for usage
  counts.

## 7. Risks

- **A second way to write a colour.** A role, a theme colour and a literal already exist.
  The no-tells lint (`scripts/no-tells.mjs`) is taught that a token name is not a literal,
  and the docs say one rule: a role if the platform has one, a token if it is the brand.
- **The reach is not finished.** Every claim here about a role override reaching a surface
  inherits stage 1's measurement; where the reach stops, so does D1.
- **State looks on the host's press path** touch code that is tuned and measured (LLP 1061,
  LLP 1115.000 D6). Stage 3 changes it on one host at a time, with the press corpus as the
  gate.
- **App axes multiply.** The 64-combination cap and per-token distinct values bound the
  table; a design that hits the cap is refused with the axes named.
- **The editor is a surface that writes source on a developer's disk** from a network
  connection. D13 restricts it to the dev server's authenticated connection; it is still a
  new writer and is reviewed as one in stage 5.

## 8. Not in this RFC

- A component library, an anatomy schema (parts, slots, waivers), or Code Connect mapping.
  If a library comes, it is `.contract` packages over this design, with anatomy as checked
  data, Facet's rule that "a recipe styles parts; it never forecloses slots", and the
  `params` block as package tokens (D11).
- Generative layout ("design intent models", exact1 LLP 0269's structure, behaviour and
  content layers).
- Responsive tokens by size class. A size class could become a host-set axis like `scheme`
  later; r1 leaves layout breakpoints to CSS's own media and container features as they are.

## 9. Open questions for Charlie

1. **Authority.** D14 makes the `.design` file the authority for everything and Figma an
   import that proposes. Is that right, or should a designer's Figma file own the values
   for some class (as exact1's v2 did, which is what made its dials inert)?
2. **How generative?** D4 keeps dials and `ramp()` but expects a real brand to be hand-tuned
   literals with a few dials where it is meant to move. Do you want more (a brand from three
   dials, Facet v1's ambition) or less (no dials, tokens only)?
3. **The DSL.** A `.design` file in Contract's grammar with CSS values, the IR generated from
   it. Acceptable to Rust, TypeScript, Swift and Kotlin users, or do you want the source in
   one of their languages? And token references as bare names (D3, as D4 built) or CSS's
   `var(--name)` with custom-property inheritance (a subtree override for free, but a
   cascade)?
4. **The base names (D11).** Ship `space.*`, `radius.*`, `type.*`, `motion.*`, `control.*`
   with platform values so packages can be themed, or only roles and text styles, with
   every other name an app's?
5. **A component library.** None here (§8). Is that the right call for now?
6. **Modes that persist.** Should `setMode` and `setRootFontSize(scale=)` persist across a
   launch (the host applies them before the first frame, so Bluesky's Dim never flashes
   Dark), or stay app state set from a mount task, as `setRootFontSize` is today?
7. **The editor (D13)** needs DEFERRED admissions: a development-only design editor and its
   row-swap channel, against "no Design Mode, no devtools UI" and "hot revision surfaces".
   Admit it for development builds only, with designers on shared builds later? What take?
8. **State parts (D8)** amend LLP 1061 D1's rejection of an `active` block. Acceptable as
   parts with paint rows only, or would you rather named rows (`-exact-pressed-background-color`
   and the like) and no block?
9. **Figma investment.** DTCG export and import-as-proposals in stage 6. Is a Variables
   writer wanted, and is Enterprise access (which writes need) available?
10. **The name.** "Facet" collides with Bluesky's rich-text facets (LLP 1114.007). This RFC
    says "design" (`app.design`, `exact design`). Something else?
