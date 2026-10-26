# LLP 1139: A component library over Tamber

**Type:** RFC
**Status:** **Accepted (2026-10-20); stage 12 closed 2026-10-21 and the second wave (LLP 1121 stage 13 C) on 2026-10-23, each with its gate met (§10).** Charlie ruled §9, "Go with recs for now" (LEAD-DECISIONS 14:50Z); each answer is for now and revisitable. r6 with r6.1–r6.3's wording. Stage 12 C (anatomy), D2 and D4 are built and signed off, landing in this order: C1–C3, D2 and D4, the packages with `apps/library-gate` and the clone's adoption, D3 after B2-levels, the design tool's half, then the gate. Reviews: four blind full rounds and a narrow re-check by GPT-6 Astra (max) and Grok 4.7 (xhigh), every verdict RIGHT WITH CHANGES and every finding taken (`llp/reviews/rfc-2026-10-19-1139-component-library-r{1..5}.{astra,grok}.md`). This is LLP 1121 §5 stage
12, which §5 makes its own RFC. The lead accepted its brief (`~/x2theme/stage12-brief.md`) with answers 1–7 on
2026-10-19; those answers are fixed here, not reopened. §9 holds the questions for Charlie.
r6.3 (2026-10-20, theme2, wording only, from stage 12 C's build): a control's label requirement is `aria-label` or
`aria-labelledby` (Contract has no `label` element); the `displayMode` rule covers reads from the viewport, tracked
through aliases, provide and inject, and fns, never a prop of that name.
r6.2 (2026-10-20, theme2, wording only, from stage 12 C's build): `lower-library-literal` allows `transparent`,
`currentcolor`, `auto`, `none`, the CSS-wide keywords and `0`/`0px`; D6 states two limits (an inject provider from
outside the package, and per-platform host entries in a library package, which Contract has no syntax for), both
QUEUE'd; D3's component-owned styles are one rule for every component, amending LLP 1121 D15.
r6.1 (2026-10-20, theme2, wording only): the per-column range refusal is the existing `tamber-param-range`
naming the column, as stage 12 B2 built it, not a new `tamber-range-column`.
r6 changes from r5 (no review round, wording only):
- The native-control predicate names every native element, including `textarea`, `progress` and text inputs.
- D2's "no group" sentence excepts the `control` group's native-button rows.
- `exactTransport` is dropped from the platform list.
- A short param name for a library component (`Card.pad`) is an error.
- `apps/library-gate` places every exported component, a component with no variant props once.
- The native button's content rule follows `check_native_button`.
- `role="radio"` on a native button is native anywhere; the radiogroup rule is presentation.
- The server's scrub check is preview-then-verify, in the serve process.
- One list of required hosts, used by every gate line.
- The transform deny entry covers matrix and SVG transforms that collapse area; box `scale` is one number.
- `slot: true` is set only when the `children` holder is a part.
- Also from the re-check: `radius` on a native button needs `appearance="auto"`; a full cover fails the rendered
  check; the package design's file name; the `font-size` 0 wording; the 17:09 ruling's record cited as it is.
r5 changes from r4:
- Scope (D0): the library rules apply to components exported from a library package (its entry), not to its
  internal helpers or to apps.
- Slot: the deny-list covers every node on the slot's ancestor chain, `part=` or not; adds opacity ≤ 0, transforms
  that scale to zero, a zero font-size and line-height 0; the rendered check renders twice with two marker contents.
- Gallery: `gallery.contract` leaves v1's gate; a variant gallery is an optional tool feature (part D); the gate's
  rendered check runs on a fixture app, `apps/library-gate` (part E); no gallery retention.
- Facts: real files (`contract/lower`, `scripts/tamber-server.mjs`); `part` reserved and stripped; the selection
  report's new field is `library`, since `component` exists; the `control` group admits `NATIVE_ROWS` and D9's field
  roles; a per-column range refusal is `tamber-param-range` naming the column (built in stage 12 B2); A2 part 2's
  `set-token` and the `tamber-package-block`
  lift cited as dependencies, not facts.
r4 changes from r3:
- The slot promise narrows to a fixed, checkable line: one unconditional, unforwarded `children`; a short deny-list
  on the slot's chain (rows and attributes), shorthands expanded, on literal and folded values; one checker function
  shared by the compiler and the server; the allow-list dropped; a rendered gallery check as the gate; the rest
  stated as a limit.
- Ranges are checked per column on the resolved px value; the terminal's `1ch` is 8 px, before cell rounding.
- Native first is enforced: a hand-drawn control role is refused, and each native control's required attributes
  are checked.
- `gallery.contract` is a retention root for the package's own check and gallery build, not for an app's release.
- A name under a package's namespace that the package doesn't define is an error, `tamber-package-unknown`.
r3 changes from r2: expressions over base names moved to an LLP 1121 D11 amendment; `lower-library-platform` listed by
real names; component-owned styles may read params; precedence restated from D8; host-state looks don't vary by
variant; the `control` group enforced; the gallery as `gallery.contract`; `set-token` depends on A2 part 2.

**Systems:**
- Contract (the compiler): `parts` on a component, `part=` on its nodes, anatomy and slot checks,
  `lower-library-literal`, `lower-library-platform`.
- Tamber (LLP 1121): package components' params (D15 in packages); the server's slot check on scrubs and writes.
- The Tamber server: `describe` lists parts and variant props; the selection report names a node's component and
  part.
- The design tool (LLP 1122): a component panel and an optional variant gallery.
- Two private packages, `@exact/cards` and `@exact/actions` (names in §9 Q4), and the Bluesky clone as the first
  consumer.

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-19
**Implementer:** per part (§5): x2-perf4 (B), reload1 (C), rt1 and dtool2 (D), iosfix with x2-perf4 (E).
**Related:**
- LLP 1121 (Tamber): §4 (no selectors), D3 (`pair=` and contrast), D7 (the plan carries references), D8 (state
  parts and their composition), D9 (native controls first, and the Post button's exception), D11 (package designs;
  the base scale), D15 (component params; `tamber-param-scope`), §5 stages 11 and 12, §10 stage 11 as built (B1's
  scale table, B2's `--exact-*` variables, C2's scale conformance and C's `Env::cell_spaces`; A2 part 2's package
  fields and override op, and D's base rows, as signed off).
- LLP 1122 (the design tool): D3 (the canvas), D5 (inspect and the selection report), D6 (the scope ladder), D7
  (panels and the contrast badge), T7 (several live copies).
- LLP 1101 (terminal apps): D1, a host entry (`app.json` `host.terminal.entry`).
- `docs/contract-for-agents.md`: `slot` and `children`; `class=Style` and conditional classes, node attributes win;
  platform facts as reserved sources.
- `docs/contract-grammar.md`: `enum<…>`.
- exact1's Facet, as research: its rule "a recipe styles parts; it never forecloses slots", and its recipe tables,
  which this RFC does not bring back.

## Summary

A component library is a set of Contract components that ship in packages and that an app restyles without editing
them. This RFC builds the three things that needs and nothing more.

1. **Tokens and params are the only knobs.** A library component's look is D15 params whose defaults name the
   package's tokens, which alias the base scale (`space.*`, `radius.*`) and roles. The app sets a package token, a
   base name, a role or a param, by full name, in its own design. It never edits the package, and in v1 it writes no
   recipe for a library component.
2. **Anatomy is checked data.** A component names its **parts** and the row groups each part takes. The library's
   own recipes (`style` blocks in the component's file, chosen by variant props) are checked against them. The one
   slot is placed once, unconditionally, and nothing on its chain can remove, hide or zero it.
3. **The tool shows it.** The server lists parts and variant props, and the selection report names a node's
   component and part. The design tool gets a component panel and, optionally, a variant gallery.

The first consumer is the Bluesky clone. Its post card, AtCard, action row and Post button become library components
in two packages, and the clone's own copies go.

## 0. Review rounds

- **Round 1** (Astra Max, Grok 4.7 xhigh, blind, on r1): both RIGHT WITH CHANGES; r2 took the lead's decisions.
- **Round 2** (the same, on r2): both RIGHT WITH CHANGES; r3 takes theme2's decisions (the change list above).
- **Round 3** (the same, on r3): both RIGHT WITH CHANGES; r4 takes theme2's decisions (the change list above).
- **Round 4** (the same, on r4): both RIGHT WITH CHANGES; r5 takes theme2's decisions (the change list above).
- **Re-check of r4→r5:** pending.

## 1. Motivation

- **Packages can ship tokens now, and nothing to restyle with them.** Stage 11 gave packages designs and a base scale
  that is each platform's own. Its gate showed two fixture packages restyling together from one app value. But a
  package's components still hard-code what an app would want to change.
- **The clone repeats itself.** The post card, the AtCard, the action row and the Post button are written out in the
  clone, with hand-written spacing. They are state-parted and token-driven already (LLP 1121 stage 8), so they are
  the right first library.
- **A component's styleable surface has no contract.** Nothing says which nodes inside a component its variants may
  restyle, what rows they may change, or what they must leave alone. exact1's Facet had that contract as recipe
  tables in TypeScript; exact2 needs it as checked data in the files it already has.

## 2. Decisions

### D0 — Scope: exported library components

- A **library package** is a package under `packages/` whose `package.json` is `"private": true` and that ships a
  design: the file its `package.json` `"tamber"` names, else `<namespace>.tamber` beside its entry, the namespace
  being the last segment of the package's name (`package_design`, `contract/cli/src/resolve.rs`; stage 11 A1).
- Its **entry** is the file its `package.json` `exports` maps `.` to, else `index.contract` (LLP 1121 D11, as stage 11
  A1 finds a package's design).
- An **exported library component** is a component the entry declares, or one it re-exports from another file of
  the same package (Contract's re-export). Every other component of the package is an **internal helper**.
- **The library rules** of this RFC (D1's knobs and lints, D2's anatomy, D4's slot, D6's native-first rules) apply to
  exported library components. Internal helpers and apps' own components are not checked by them, with two
  exceptions: D4's deny-list follows the slot's chain through every node of the exported component's own view, and
  D6's platform rule covers the whole package (a helper can't read the host on an exported component's behalf).
- An app that renders a library component is checked as an app: nothing here limits what the app writes outside the
  component.

### D1 — A library component is a Contract component in a package; tokens and params are the knobs

- A library component is an ordinary `.contract` component in a package under `packages/`. Its identity is D15's:
  `@exact/cards/card.contract#Card`.
- **Its look is its params** (D15). Every value an app may change is a param or a token a param reads.
- **Defaults name package tokens**, never literals:

  ```contract
  component Card
    params
      pad: length = "cards.pad"  control="token"  group="Shape"  priority=1
      corner: length = "cards.corner"  control="token"  group="Shape"  priority=1
      fill: color = "cards.surface"  control="token"  group="Colour"  priority=1
  ```

  ```tamber
  tamber cards

  theme
    cards.pad = space.md
    cards.corner = radius.lg
    cards.surface = -exact-secondary-background
    cards.ink = -exact-secondary-label
  ```

- **The cascade, highest first**, for one param, per tuple:
  1. **the app's param value**: the app's design sets `@exact/cards/card.contract#Card.pad` (D15, `set-param`);
  2. else **the param's default**, the package token `cards.pad`, whose value is:
     - **the app's line for the token**, if the app's design sets `cards.pad` by full name (stage 11 A1). An app line
       replaces the package's token whole, all its `when` lines (D11). The tool writes it with `set-token`; today
       `set-token` refuses a name the app's design lacks (`tamber/src/edit.rs`), so creating the app's first line for a
       package token **depends on stage 11 A2 part 2** (in review; theme2's 17:09 ruling as recorded in the A2 part 2
       and stage 11 D reviews: "Set for this app" is `set-token`, which carries no kind; whether `add-token` also
       refuses package names is A2 part 2's to settle);
     - else **the package's line**, here `space.md`;
  3. a base name or role that line names resolves to **the app's line for it**, if any, else the base scale's cell
     for the host's column (stage 11 B1) or the role's platform value.

  The level that won is reported per tuple (D5).
- **Packages are private and in the repo** (`packages/`, `"private": true`) until the gate is met (the lead's answer
  5). Nothing publishes them.
- **`lower-library-literal`** (an error in a component of a package that ships a design; the app's own components
  are not checked):
  - a spacing row (`padding*`, `margin*`, `gap`, `row-gap`, `column-gap`), a radius row (`border-radius`,
    `border-*-radius`) or a colour row (`color`, `background-color`, `border-color`, `-exact-tint-color`,
    `accent-color`) whose value is a literal length or colour value, not a param, a token, a role or a base name;
  - a param default that is a literal.

  `0` and `0px` are the only literal lengths allowed. Keywords with no design value and no per-host variance are
  not literals here: `transparent`, `currentcolor`, `auto`, `none` and the CSS-wide keywords (`inherit`, `initial`,
  `unset`).
- **Ranges hold in every column (built in LLP 1121 stage 12 B2):** Today D15 folds a ranged
  param's default and the app's value per tuple, and refuses any length that isn't px (`tamber-range-unit`,
  `contract/lower/src/params.rs`), which includes a reference to an unset base name. This RFC changes one case: when
  the folded value is a reference to an unset base name, the check no longer emits `tamber-range-unit`; it reads that
  name's cell in every scale column (ios, mac, neutral, material, terminal), resolves each to px as the host's length
  does, and checks each against the range, refusing one outside it with the existing `tamber-param-range`, which
  names the column and the px. A `rem`, `em` or text style stays `tamber-range-unit`. The terminal's cells are `n ch`, and `1ch` is 8 px
  on either axis (`kernel/src/style/cells.rs`), so terminal `space.md` is checked as 8 px, before the kernel's
  whole-cell rounding of padding, margin and gap (`Env::cell_spaces`), which happens at layout, per axis, and is not a
  range question. The same rule applies to the app's override and to a token edit that reaches the param.
  `radius.full` is never a ranged default.

### D2 — Parts: named nodes and the row groups they take

A component declares its parts in a `parts` section, and names nodes of its own view with `part=`:

```contract
component Card
  props
    title: string
    tone: enum<"plain", "quiet">
  slot
  params
    pad: length = "cards.pad"  control="token"
    corner: length = "cards.corner"  control="token"
    fill: color = "cards.surface"  control="token"
    quiet: color = "cards.surface-quiet"  control="token"
  parts
    surface: paint shape space
    header: text space
    body: space
  view
    column part="surface" class=(tone == "quiet" ? QuietSurface : PlainSurface) border-radius="param.corner" padding="param.pad"
      text part="header" font-size="-exact-headline" color="cards.ink" title
      column part="body" gap="space.sm"
        children

style PlainSurface
  background-color = "param.fill"

style QuietSurface
  background-color = "param.quiet"
  box-shadow = "none"
```

- **A part is a node of the component's own view** with `part="name"`. Names are lower-case words, unique in the
  component. A node in a `for` or `match` may be a part; the groups then apply to each instance.
- **`part` is a reserved attribute**, as `testId` is: the compiler admits it on any element of a component's own view
  (not `lower-unknown-attr`), records it in the Tamber IR and the source map (D5), and strips it in lowering, so it
  never reaches a plan, the DOM or a host. It is unrelated to `::placeholder` (LLP 1104), which stays the one
  pseudo-element block.
- **Groups**, over the schema's longhand rows (a shorthand's longhands each checked):

  | Group | Rows | What it may change |
  |---|---|---|
  | `paint` | `background-color`, `border-*-color`, `border-*-width`, `box-shadow`, `opacity`, `-exact-tint-color`, `-exact-press-scale` | the part's fill, border and shadow; a border width moves its content by the width |
  | `text` | `color`, `font-family`, `font-size`, `font-weight`, `font-style`, `font-variant-numeric`, `line-height`, `letter-spacing`, `text-align`, `text-transform`, `text-decoration-*`, `white-space` | the part's type, which inherits into its subtree and can change its size |
  | `shape` | `border-*-radius` | the part's corners |
  | `space` | `padding-*`, `row-gap`, `column-gap`, `align-items`, `justify-content` | the space and alignment inside the part, which can change its size |
  | `control` | `accent-color`; on a native `button`, the rows `NATIVE_ROWS` admits (`contract/lower/src/controls.rs`: control size and corner style, gaps and alignment, font size and weight, text colour, corner radii, padding, sizing, margins, position, opacity, transforms, `appearance`, transitions) | a native control's supported parameters (LLP 1121 D9). A field's and text area's colours come from D9's control roles `Field`, `FieldText` and `placeholder` (`kernel/tables/schema.json`'s control-role table), set as role values in the design, not as rows |

  Rows in no group (`display`, `position`, `width`, `height`, `min-*`, `max-*`, `flex*`, `overflow`, `margin-*`,
  `visibility`, `transform`, `scale`) are never brought to a part by a class, except where the `control` group
  admits them on a native `button`'s part (its `NATIVE_ROWS`). State blocks keep D8's own row rules on
  top of the groups: a state row must be in the part's groups **and** D8's list for its state (so `border-*-width`,
  in `paint`, is still `lower-state-row` in a state).
- **What the groups govern.** The library's own recipes: every row a `class` (plain or conditional) brings to a
  part's node must be in that part's groups. A row outside them is `lower-anatomy-row`, naming the part and its
  groups. The component's own node attributes are not limited by groups, except on the slot's chain (D4) and on a
  native control's part (D6).
- **Declaration checks** (errors): a `part=` on a node outside the component's own view, two nodes with one part
  name, a `parts` entry with no node, or a `part=` with no entry (`lower-anatomy-part`); a group name not in the table
  (`lower-anatomy-group`); a `control` group on a node that isn't a native control, or a native control's part with
  any other group (`lower-anatomy-control`).

### D3 — Recipes are the library's own; precedence is LLP 1121 D8's

- **A recipe is a `style` block in the component's own file**, chosen by `class` on a part's node, plain or
  conditional on a variant prop (`class=(tone == "quiet" ? QuietSurface : PlainSurface)`). That is today's `class`
  model: same-file styles, conditional classes by a condition.
- **A recipe may read its component's params** when the compiler sees the style applied only by that component's
  view (every `class` naming it is in that component). Then `param.*` in the style resolves to that component's
  params. Otherwise it stays `tamber-param-scope`, as today (LLP 1121 §10 stage 7: "a param named where no component
  owns it is refused … in a `style` or a `keyframes`, which any component may apply";
  `contract/lower/src/params/rewrite.rs`). This is an amendment to that rule, built in part C.
- **Precedence is LLP 1121 D8's, unchanged.** Per row, after shorthands are expanded, lowest to highest:
  1. the class's base rows, then the node's own base rows (the node wins);
  2. `:checked`, the class's then the node's;
  3. `:disabled`, the class's then the node's;
  4. the host overlays, `:hover` then `:focus-visible`, each the class's then the node's;
  5. the press paint from `:active`, the class's then the node's.

  So the node beats its class **within each layer**, and every state layer beats the base: a class's `:disabled`
  row overrides the node's own base row. Steps 1–3 fold in the runner into conditional rows; 4–5 are the host's.
- **Host-state looks don't vary by variant in v1.** A host-known state's row (`:hover`, `:focus-visible`, `:active`)
  must be a literal; a token's or param's name is one (`lower-state-literal`, theme2's ruling (b)). A conditional
  class lowers differing rows to a `?:`, which a host-state row refuses. So a library component's host-state look
  can read a param (`:active background-color = "param.pressed"`), one look for every instance, but two variants
  can't have different `:active` colours. Runner-known states (`:disabled`, `:checked`) fold and may differ by
  variant. §9 Q8 asks whether v2 should let a host state choose by a variant prop.
- **No app-authored recipes in v1.** An app restyles a library component only through tokens (the package's, base
  names, roles) and params. A part-addressed style from another file would be a selector, which LLP 1121 §4 rules
  out. Whether apps should restyle parts directly is §9 Q6.
- **No recipe tables.** exact1 Facet's TypeScript recipe tables are not brought back (the lead's answer 7).

### D4 — The slot: a fixed, checkable line

Contract gives a component one content `slot`, which its view places with `children`. Contract itself lets the fill
be placed in an `if` arm, a `for` or more than once. An exported library component that declares `slot` promises
less and checks all of it. One without `slot` has no slot rules.

**Statically enforced** (errors, `lower-anatomy-slot` unless named):

1. **One unconditional placement.** `children` appears exactly once in the component's view, not inside an `if`,
   `match` or `for` at any depth, and not passed into another component's slot (no forwarding in v1):
   `lower-anatomy-slot-placement`. The node holding `children` and **every node above it to the component's root**,
   with or without `part=`, are the **slot's chain**.
2. **A deny-list on every node of the chain.** After shorthands expand (`contract/lower/src/shorthands.rs`) and the
   class and node rows merge (every branch of a conditional class, and each state layer), no node of the chain may
   carry:
   - `display: none`; `visibility: hidden`;
   - `opacity` of 0 or less;
   - `width`, `height`, `max-width` or `max-height` of 0;
   - a box `scale` of 0 or less (box `scale` is one number, `kernel/tables/schema.json`; a box `transform` is already
     refused, `lower-attr-tag`); on an SVG element of the chain, a `transform` whose matrix has a determinant of 0
     (`scale(0)`, `scale(1, 0)`, `matrix(…)` that collapses area, even when neither axis is 0);
   - `font-size` of 0 on any node of the chain (even if a descendant sets another size), and `line-height` of 0;
   - the attributes `hidden`, `inert`, `aria-hidden` (true) or `popover` (any value).
3. **Literal and folded values.** A row's literal value is checked, and so is a token's or param's folded value in
   **every tuple** of the design, and in every scale column when it reaches an unset base name (resolved to px as in
   D1). A value the checker can't fold is refused, never assumed safe.
4. **One checker.** The compiler and the server run **the same check**. It lives in `contract/lower`, beside the
   anatomy pass, takes the chain's merged rows and a candidate fold, and is exposed through `contract tamber`, the CLI
   the server already runs. The compiler runs it at build. The server runs it in the `contract tamber serve --stdio`
   process that `scripts/tamber-server.mjs` spawns, where its write checks already run: on every transaction's
   candidate fold before it commits. A scrub is preview-then-verify, as today (`scripts/tamber-scrub.mjs`): its frames
   are previewed on the running copies as the gesture moves, and when it rests, `scrub-fold` verifies it; a scrub
   whose resting value fails the check is refused there and its preview reverted, never committed. A rebuild-class
   edit has no scrub frame and is checked as a transaction. A failing one is refused with the
   checker's reason, naming the component, node, row and tuple (no clamping). The Tamber IR records each chain's rows
   and the tokens their values reach, so the server knows which writes to check.

**The gate's rendered check** (§5): on the fixture app `apps/library-gate`, on each required host (§5), for every
variant, the slot's sample content is rendered twice, with two different marker contents (two glyphs of different
shape) in a marker node of fixed size. Each time, the marker node has a laid-out rect of non-zero width and height
inside the viewport, and the two renders' pixels in that rect (the same rect, since the node's size is fixed)
differ. A full cover over the marker makes the two renders match and fails the check; a partial cover is the stated
limit below.

**Stated as a limit, not checked:** anything else that could hide a fill — a sibling drawn over it, an inset shadow,
a text colour equal to its background, clipping by a parent outside the component, a size squeezed by the caller's
layout. Contrast stays LLP 1121 D3's `pair=` and `tamber-contrast`, and the tool's contrast badge (LLP 1122 D7).
Inherited text styles reaching the caller's children (a card's body colour and type) are allowed and intended; the
fill keeps its caller's own rows, variables and providers (Contract's rule).

### D5 — Variants, inspection and the tool

- **A variant prop** is a prop of type `enum<…>` or `bool` that a `class` condition, or a row's `?:` or `match`, in
  the component's own view reads. The compiler marks it in the Tamber IR.
- **A variant gallery** in the design tool is an optional part D feature (dtool2): the tool renders an exported
  component once per variant value in its copies. It is not in v1's gate. Nothing is pinned on the app's own
  instances; per-instance prop pins are §9 Q5.
- **The Tamber IR** gains, per exported library component (with or without params): `parts: [{name, groups,
  slot}]` (`slot: true` on a part only when that part's node is the one holding `children`; a `children` holder that
  isn't a part sets no `slot` anywhere), `variants: [{prop, options}]`, and the slot chain's merged rows and the
  tokens their values reach (D4), recorded whether or not the chain's nodes are parts.
- **`describe`** (*1.15*) lists those per component, and each param's winning cascade level per tuple (`level`:
  `app-param`, `app-token`, `package`, `base`, `role`) with the token or name at that level.
- **The selection report** (LLP 1122 D5): its `source` already has `component`, the use-site name of the call site
  (`contract/cli/src/map.rs`), which stays as it is. It gains **`library`**, `{identity, part}` (the exported library
  component's declaring identity, and the part or null), on the source and on each row with a map origin. The
  server joins it from the source map for every host (`part=` never reaches a host), through inlining. It is absent
  outside an exported library component. `explain`, which takes a token, is unchanged.
- **The tool** (LLP 1122):
  - a **component panel**: params by group and priority, each with its level per tuple and two distinct resets:
    - **"Back to the default"** removes the app's param value (`unset-param`): the param then follows its default;
    - **"Back to the package"** removes the app's line for the package token (`remove-token`): the token then
      follows the package's line. It is offered only when the app has that line, and the panel says when an app
      param still wins over it;
    - each reset removes the whole line, all its `when` branches, and says so before it writes (LLP 1122 D6);
  - "Set for this app" writes `set-param` by default; editing the package token instead is an explicit choice;
  - library recipe rows are **read-only** in the tool: package files are read-only to the server (stage 11 A's guard),
    so no part-qualified write exists in v1;
  - a **part outline** when inspecting a library node, the slot's part marked; the optional variant gallery.

### D6 — Native controls first; buttons by D9's exception; no platform reads

- **Native control roles use the native control element.** An exported library component that draws a control's
  role by hand is refused (`lower-library-control`): a node with `role="switch"`, `"checkbox"`, `"slider"`,
  `"progressbar"`, `"textbox"`, `"spinbutton"`, `"combobox"` or `"radio"` that is not a native control element. The
  native control elements are those the "Write / iOS draws" table maps to a platform control: `input` of every
  `type` (text kinds, `checkbox`, `checkbox switch`, `radio`, `range`, `date`, `time`, `datetime-local`), `textarea`,
  `select`, `progress`, and a native `button` (`controls.rs` `control` classifies the form controls and buttons; the
  check adds `textarea`, `progress` and text inputs, which it leaves unclassified). A `role="radio"` on a native
  `button` is native anywhere `check_native_button` accepts it; whether a `role="radiogroup"` of them draws as the
  segmented control (not with `appearance="none"`) is presentation, not this check. A native element carrying a
  redundant matching role is allowed. It writes the native element instead, from the list Contract maps to platform controls
  (`docs/contract-for-agents.md`, the "Write / iOS draws" table; LLP 1121 D9 for their parameters).
- **Each native control's required attributes are checked** (`lower-library-control`, naming the missing one):

  | Control | Required |
  |---|---|
  | `input type="checkbox" switch`, `input type="checkbox"`, `input type="radio"` | `checked`, and `aria-label` or `aria-labelledby` (Contract has no `label` element) |
  | `input type="range"` | `value`, `min`, `max`, and `aria-label` |
  | `input` (text kinds), `textarea` | `value`, and `aria-label` or `placeholder` |
  | `input type="date"`, `"time"`, `"datetime-local"` | `value`, and `aria-label` |
  | `select` | `value`, its `option`s, and `aria-label` |
  | `progress` | `aria-label` (and `value` with `max` when it shows progress) |
  | `button` (native) | a text title, or one `symbol:` image with a non-empty `aria-label` (`check_native_button`; a face with neither is `lower-button-content`) |

- **The `control` group.** A native control's node is a part with the `control` group only. Its style rows are those
  the group admits (D2's table: `accent-color`, and on a native `button` the rows `NATIVE_ROWS` admits), from its own
  attributes or any class; another style row is `lower-anatomy-control`. A corner radius from `NATIVE_ROWS` keeps a
  native button native only with `appearance="auto"` (otherwise `buttons.rs` `button_appearance` demotes a default
  button carrying one), so on a library button a radius without `appearance="auto"` is `lower-anatomy-control` too:
  a library control is refused wherever an app's own button would be demoted to a bare box. Its non-style attributes (`type`,
  `value`, `checked`, `disabled`, `aria-label`, `name`, event handlers) are unaffected. A row the platform ignores
  (`tamber-unreachable`, D9) is reported as today and doesn't count as a passing check. No state parts on it
  (`lower-state-target`, as today).
- **Buttons follow D9's existing exception.** The Post button stays an authored pressable with the press paint,
  because the platform's prominent button can't show its faded-blue disabled look (LLP 1121 D9). Its parts take
  `paint`, `text`, `shape`, `space`, with D8's state rules. A library button that wants the platform's look is a
  native `button` with the `control` group, and takes no paint.
- **No per-OS forks: `lower-library-platform`.** A library component, and every component in its package, may not
  read the host it runs on. The compiler refuses, in a package that ships a library:
  - a resource of the reserved sources **`exactDelivery`** or **`exactSurface`** (any name), and the field
    **`displayMode`** of **`exactViewport`** (resources are root-only in Contract, so this applies to a package's
    root component and to anything it provides); a read is tracked through aliases, provide and inject, and fns;
    a prop that happens to be named `displayMode` is the app's data and passes;
  - an `inject` whose provider in the package reads one of those;
  - a host entry: a package file named as an `app.json` `host.<platform>.entry` (LLP 1101 D1), or a package shipping
    per-platform variants of one component.

  Dimensions, `exactViewport().pointer` and `.hover`, media preferences and `exactPage` facts are capabilities, not
  platforms, and stay allowed (`docs/contract-for-agents.md`: "use dimensions, media preferences, page facts, and
  capability state rather than suffixing files by platform"). Props are the app's data: the rule covers what the
  library reads itself, not what an app chooses to pass in.

  **Limits, QUEUE'd (stage 12 C):** an `inject` whose provider sits OUTSIDE the package and reads a platform input
  is not traced (a provider inside the package is held to the rule above); and per-platform host entries in a
  library package aren't checked, since Contract has no syntax for a package host entry yet.

### D7 — Expressions over base names

Library packages may use expressions over base names (`calc(space.md * 2)`) once stage 12 part B2 lands; until then
they alias base names. Their format, evaluation and any schema change are an amendment to LLP 1121 D11, written with
B2's plan and reviewed as a kernel change, not here.

### D8 — Names are API; a rename fails the build

- A package token's full name (`cards.pad`) and a param's full name (`@exact/cards/card.contract#Card.pad`) are the
  library's API.
- **A name under a library package's namespace that the package doesn't define is an error,
  `tamber-package-unknown`** (new; no such diagnostic exists today), failing the build. It covers: an app design line
  setting `<name>.<x>` (`<name>` from the package design's header, `tamber <name>`) when that design has no
  `<name>.<x>`; a param line naming a full identity under the package (`@exact/cards/…#Card.pad`) with no such
  component or param; and a Contract row naming such a token. A short param name (`Card.pad`) that names a library
  component's param, or that names no param while matching a name a library package exports or exported, is an
  error too (`tamber-param-short-library`, naming the full identity where there is one), so apps write full
  identities for library params and a rename can't fall back to `tamber-param-unknown`'s warning. A namespace whose package is gone (no linked package has `tamber <name>`)
  leaves a plain app token, as today. The nine base names are not a package namespace. Today a token line the app writes for a name nobody else defines is the app's own token, and a line
  naming no param is the warning `tamber-param-unknown` (LLP 1121 §10, stage 7: "a moved or renamed declaring file
  must not break the build"). That warning stays for the app's own components; under a package's namespace, the
  package owns the names (stage 11 A1: a package's tokens are all `<name>.…`), so the stale line is an error.
- **In v1, packages are private and in the repo**, so a rename updates every in-repo consumer in the same change, and
  `tamber-package-unknown` catches any it missed.
- A deprecation policy for published packages, with version ranges, is §9 Q1.

## 3. What it needs

| Need | Where | Status |
|---|---|---|
| package components' params, identity, the app's value, retention, `uses`; package `.tamber` components | LLP 1121 D11, D15 → the compiler, the server | proposed (B) |
| `set-token` creating the app's line for a package or base name | LLP 1121 stage 11 A2 part 2 | dependency, in review |
| lifting `tamber-package-block` for package `.tamber` components | stage 12 B1 (x2-perf4) | dependency, proposed |
| expressions over base names | an LLP 1121 D11 amendment with B2's plan | proposed (B2) |
| D0's scope, `parts`, the reserved `part`, groups, the slot checker, a component-owned style's params, `lower-library-literal`, `lower-library-platform`, `lower-library-control`, `tamber-package-unknown` | this RFC D1–D4, D6 → the compiler | proposed (C) |
| the slot chain in the IR; the same checker run by the server on writes and scrubs | this RFC D4 → the server | proposed (C, D) |
| parts, variants, levels in the IR and `describe`; the selection report's `library` | this RFC D5 → the server, the source maps | proposed (D) |
| the component panel, the part outline, the optional variant gallery | this RFC D5 → LLP 1122 | proposed (D) |
| two packages, `apps/library-gate`, and the clone's adoption | this RFC §5 | proposed (E) |

## 4. Protocol

**`tamber/1.15`**, opened when LLP 1121 stage 11 closes. B's and D's additions are optional fields under it. C has
no wire. Nothing here lands on the wire before stage 11 closes; B may be built before then and lands after.

## 5. Stages and gates

Owners as they free up (the lead's order).

1. **A, this RFC** (theme2), then Charlie's rulings on §9.
2. **B, package components' params** (x2-perf4; starts before this RFC is accepted, by the lead's answer 2). Package
   Contract components' params finish D15 as decided. Package `.tamber` components need LLP 1121's
   `tamber-package-block` lifted ("a package's components come with stage 12"): that is B1's change and a dependency
   of this RFC, not a decided fact. Verify what a package's Contract components list in `describe` and whether `set-param` by full
   name works for them; build what's missing. Package `.tamber` components (`component … params` in a package
   design, now `tamber-package-block`): identity `<package>/<file>#Name`, collected with their declaring identity,
   collisions refused, params retained with their dependency closures (D15), a package `params` override block
   still refused. Wire *1.15*.
   **Gate:** a package Contract component's param set by the app by full name moves every instance; a package
   `.tamber` component's param reaches a native component through its generated binding; unset returns each; the
   package files' bytes are unchanged (hashes before and after).
3. **B2, expressions over base names** (reload1, after stage 11 A2 part 2): its own plan and LLP 1121 D11 amendment.
4. **C, anatomy** (reload1, after B2). D1's lint and ranges, D2, D3's component-owned styles, D4, D6.
   **Gate:** a compiler test per refusal, each with its span: `lower-anatomy-part`, `-group`, `-row`, `-control`,
   `-slot` (each deny-listed row and attribute, by shorthand and by longhand), `-slot-placement` (an `if`, a
   `match`, a `for`, twice, forwarded), `lower-library-literal`, `lower-library-platform` (each listed input),
   `lower-library-control` (each hand-drawn role, each missing attribute), `tamber-param-range` naming a column (a terminal column
   included), `tamber-package-unknown`, `tamber-param-scope` for a style a second component applies; a fold test that
   a token folding to 0 in one tuple on a slot chain is refused; every recipe the packages ship passes with
   `lower-library-literal` clean.
5. **D, server and tool** (rt1 and dtool2, as they free up). D4's server check and D5; the variant gallery is
   optional in v1. It depends on C's IR and B's fields, not on LLP 1121 10c or LLP 1122 T7's grid.
   **Gate:** a `set-token` and a scrub that would give a slot chain's node `opacity: 0` in one tuple are each refused
   with the same reason the compiler gives for that value (one checker); `describe`'s parts, variants and levels on
   the fixture packages; the selection report's `library` on a library node, from every host's inspect mode.
6. **E, the first library, the fixture app and the consumer** (iosfix with x2-perf4). `@exact/cards` (the post card,
   the AtCard) and `@exact/actions` (the action row, the Post button), from the clone's own pieces;
   **`apps/library-gate`**, an ordinary app that places **every** exported library component of both packages: once
   per value of each enum and bool variant prop (one varied at a time, the others at their first value; a bool's
   first is `false`), and a component with no variant props once; a test checks that its components are exactly the
   packages' exports, with
   fixed sample props, `testId`s on part nodes, and in each slot a marker content switchable between two glyphs;
   normal app retention applies to it; and the clone's copies replaced, on a signed-out copy of the clone with no
   account writes.
   **The exports (as of LLP 1121 stage 15 B, 2026-10-26):** `@exact/cards` exports PostCard, AtCard, ProfileHeader
   (with its ProfileAlert shape), **Avatar** `(src, size)` and **VerifiedBadge** `(badge, size, ink, mark)`;
   `@exact/actions` ActionRow, PostButton, ComposeFrame, BarItem and HeaderButton; `@exact/lists` ListRow and
   SwitchRow. Avatar and VerifiedBadge were the cards' shared pieces until stage 15 B exported them with their params
   unchanged and no defaults: a consumer passes `size`, and VerifiedBadge's `ink` and `mark`, as token or role names
   (`size="cards.avatar"`, `ink="cards.badge"`, `mark="cards.badge-mark"`, or its own), not literal colours, as
   PostCard does, so an app's design restyles them by the tokens' full names. `apps/library-gate` holds each in a
   box named for its cell (neither has an `id` prop), VerifiedBadge's `""` an empty box.

**The gate** (`~/x2theme/stage12-gate-run.txt`), each line with its oracle. **The required hosts**, for every line
that runs per host: the web (the JS target and wasm), iOS, macOS, Linux, Android's reader and the terminal. A
required host missing from a run fails that line. Windows and tvOS are checked where a runner is available, else
recorded as skipped; they don't fail a line.

- **The consumer, no visual change.** The clone's post card, AtCard, action row and Post button come from the two
  packages. Oracle: conform on the clone clean, and a pixel diff on the clone's own targets among the required hosts
  (the web, iOS, macOS, Linux) of a **fixture build**: the clone's feed, thread and composer screens fed a checked-in recorded data set (no network), the agent
  clock fixed, fixed viewport, bundled fonts, default text size, light and dark, on named OS and simulator versions.
  Baselines are taken before the change and recorded with their provenance (commit, OS, device). Zero differing
  pixels outside a checked-in allowlist of rectangles; each rectangle has a reason and may not cover more than 10%
  of any library component's box.
- **One app value restyles the library.** Setting `space.md`, `cards.pad`, or a param in the app's design moves
  every library component that reads it; removing the line returns it. Oracle: per host, the measured box of
  each library component's `testId`'d nodes (`part=` is stripped, so `apps/library-gate` gives part nodes a `testId`) before, after and after removal (scale-check's method), expected values from `scale::column_for` per
  target (web, Windows and Linux read neutral; tvOS reads iOS; Android's reader Material; the terminal in whole cells
  by `Env::cell_spaces`), on the required hosts below; Windows and tvOS checked where a runner is available, else
  recorded as skipped. **Live versus rebuild**, per case,
  recorded from the server's live-or-rebuild answer (LLP 1121 D13): a change of a value the app already sets is live
  where the live table carries it; the first app line for a name, or its removal, may change the table's shape and
  rebuild.
- **Anatomy.** C's compiler tests.
- **The slot, rendered.** On `apps/library-gate`, on every required host, for every variant cell: rendered once with
  each of the two marker glyphs in its fixed-size marker node, that node has a laid-out rect of non-zero width and
  height inside the viewport (from the agent's layout), and the two renders' pixels in that rect differ (for the
  terminal, its cells in the text screenshot).
- **No per-OS forks.** `lower-library-platform` clean over the packages.
- **No regressions.** Release output byte-identical for apps that use no library component (the 14-app comparison
  stage 11 uses); conform clean; `@exact/reading`'s users unchanged.

## 6. Risks

- **Parts leak structure.** A part named today constrains the component's view tomorrow, since its variants and the
  tool rely on it. In v1 the library and its consumers move together (D8).
- **Groups too coarse or too fine.** If a real component needs a row no group has, the table grows by a reviewed
  change to this RFC, not per component.
- **The slot promise is narrow on purpose.** It catches the ways a recipe or token would remove or zero the fill,
  and `apps/library-gate` shows each variant rendering it. Overlays, shadows, colours and outside clipping are a
  stated limit (D4): the rendered check fails a full cover, and review catches the rest.

## 7. Cost

B: a week of one agent. C: a week. D: a week each for the server and the tool. E: a week, mostly the clone's
adoption and its fixture build and baselines. B2 is costed in its own plan.

## 8. Not in this RFC

- App-authored recipes for library components, and any cross-file part styling (§9 Q6).
- Named slots, render props (the lead's answer 3). A row with leading, content and trailing areas takes **props**
  for leading and trailing (an icon name, a label, an optional count), and its one slot for the content.
- Expressions over base names (an LLP 1121 D11 amendment with B2).
- Per-instance prop pins in the tool (§9 Q5).
- Publishing packages, version ranges, deprecation aliases, a registry (§9 Q1).
- Recipe tables.
- Parts inside native controls (§9 Q3).
- Code Connect and Figma components.

## 9. Questions for Charlie, ruled

**Ruled (Charlie, 2026-10-20: "Go with recs for now"; LEAD-DECISIONS 14:50Z).** Each answer is the recommendation,
for now, and revisitable when the library has users or is published.

1. **Published packages: version ranges and deprecation.** Not until a package is published. In-repo, a rename
   updates every consumer in one change (D8); version ranges and alias lines with a warning come with publishing.
2. **Variant coverage.** One prop varied at a time (`apps/library-gate`, and the tool's gallery when it comes), not
   the cross product.
3. **Parts inside a native control** (a switch's track and thumb, a slider's fill). Not in v1: a native control is
   one part with the `control` group.
4. **The package names.** `@exact/cards` and `@exact/actions`, from the clone's pieces, private. (Stage 11's gate
   fixtures were renamed out of the way first.)
5. **Per-instance variant pins.** The gallery first; pins on a live instance (a new *1.15* preview command) later if
   designers ask.
6. **Apps restyling a library component's parts directly.** No in v1: tokens and params only, and recipes stay the
   library's own `style` blocks chosen by variant props. Ask again once the first library has users; a yes needs a
   cross-file style rule that isn't a selector.
7. **Anatomy syntax.** r6's spelling, as built: a `parts` section with row groups (D2) and `part="name"` on nodes.
8. **Host-state looks per variant.** Not in v1: a hover, focus or press look can read a param but not differ between
   variants. A host state choosing by a variant prop is a v2 question.

## 10. As built

Stage 12 was built and reviewed part by part under LLP 1121's process, each part signed off by theme2 (the design
tool's code by review2), on `tamber/1.15`. Where a part differs from the text above, the text above was amended in
r6.1–r6.3 and the rulings are in `~/x2theme/SIGNOFFS.md`.

- **B1, package components' params** (`bec1cbb7a4` and after; x2-perf4): a package design's components declare params,
  named `<package>/<file>#Name`; the retention key that keeps it fast went through six re-checks to a verbatim key,
  and the follow-up's retention build and background whole build (reload1, through `dce4be7e40`) bring a name-moving
  edit from about 2.8 s to about 0.6 s on the clone.
- **B2, expressions over base names** (`8e71dd32d3`; reload1): kept as written and evaluated in each host's column
  (LLP 1121 D11's 2026-10-20 amendment); **B2-levels** (`0fc2845866` and the per-side follow-up; x2-perf4): a param's
  levels on explain and resolve, each tuple's winning level and name per scheme side; **B3** (`2d07317bd7`..`22de44dd62`;
  x2-perf4): shadows over base names and length tokens, per column.
- **C, anatomy in the checker** (`750aca50ac`, `ab9f60e07a`, `cc5265fea1`, records `4abdbbc13f`; reload1): parts with their
  groups, the slot's fixed line (D4), the library lints and D3's component-owned styles, one checker shared with the
  server; 45 plans and 47 sources' diagnostics byte-identical through every round.
- **D, the server** (rt1): D1 per-column values (`1b1d067ee3`, opening `tamber/1.15`), D2 the slot check on writes and
  scrubs (`344c9c2aa6`..), D3 describe's library, parts and variants (`e83c7570cc`), D4 the selection's `library`
  (`9d38af028f`..`4a2f1f1705`); **the tool** (dtool2): the levels with two resets (T1), variant selects (G1),
  `contract gallery` (G2), the gallery page with `page: "gallery"` (G3) and the Gallery view (G4).
- **E, the first library and the consumer** (`4749237d1d` and after; iosfix with x2-perf4): `@exact/cards` (PostCard,
  AtCard) and `@exact/actions` (PostButton, ActionRow), `apps/library-gate`, and the Bluesky clone's adoption on a
  signed-out copy with synthetic data.

### Stage 12's gate: met (2026-10-21; runs `~/x2theme/stage12-gate-run.txt` and `stage12-gate-run-noreg.txt`)

| Gate line | Result |
|---|---|
| The consumer: the clone's post card, AtCard, action row and Post button from the packages with no visual change | **Met**: 50 shots on web (JS and wasm), iOS, macOS and Linux, light and dark, identical to the baseline built on the same SDK; all four components placed and rendered; conform on the clone no failure beyond the baseline's. Allowlisted: the composer's caret strip and iOS's system keyboard, each with its reason; the web's anti-aliased clip edges at most 2 levels, flat areas strict. |
| One app value restyles the library | **Met** on 7 hosts: `space.md` and a param live, `cards.pad-start` by rebuild; removing the line restores each. |
| Anatomy is checked | **Met**: C's compiler tests, every refusal with its span. |
| The slot renders | **Met**: every variant cell of `apps/library-gate` shows its marker in both glyphs on every required host (Windows and tvOS recorded as skipped). The slot check takes 0.026 ms alone and 0.0002 ms with the packages (bar 10 ms). |
| No per-OS forks | **Met**: `lower-library-platform` clean over both packages. |
| No regressions | **Met** (the rerun, differing from the full run's tree only by a QUEUE line): conform 0 failures over 112 targets, as the base; release output identical over 392 comparisons in 8 runs except `apps/library-gate`'s own and `apps/expose` failing alike; `@exact/reading`'s users unchanged. |

**Found and fixed on the way:** a JS boot race in the agent's settle (a list's owed scroll event) and the clone's
tab-width race; a server that wrote a library param short; a nameless package outside the app leaking its path
(now `contract-package-unnamed`). **QUEUE:** the intersect and controls boot flakes; per-instance variant pins
(§9 Q5); apps restyling parts (§9 Q6).

**Stage 12 is closed.**

### The second wave (LLP 1121 stage 13 C): built, and its gate met (2026-10-23; run `~/x2theme/s13c-gate-run.txt`)

iosfix built it with x2-perf4, and theme2 signed off each piece:
- **`@exact/lists`**, a new private package: ListRow and SwitchRow, for the settings and profile rows
  (`21a1a836b3`).
- **ProfileHeader** in `@exact/cards`, with 8 library-gate cells (`b821a5f4d0`..`9aa14743f2`).
- **ComposeFrame** in `@exact/actions` (`f926a60524`..`304de7b893`).
- **BarItem and HeaderButton** in `@exact/actions`, under D9's exception for an app's own bar, with
  lower-library-control's button rules: every library button is named, and a look that reads a state says it
  (`ba2d017788`..`4f29a65443`).

The clone adopts all six on its signed-out, synthetic copy. Package icon tokens followed (LLP 1121 D11's 2026-10-22
amendment). Moving the packages' own path icons to them is in review.

| Gate line | Result |
|---|---|
| The consumer: the clone's settings rows, profile header, composer frame, bar items and header buttons from the packages with no visual change | **Met**: 100 shots on web (JS and wasm), iOS, macOS and Linux, light and dark, identical to the baseline on the same SDK. All 10 library components placed and shown on the shot screens on every host; the six new ones on none before. The signed-in home is signed in through the copy's fixture form only. |
| One app value restyles the library | **Met** on 7 hosts: stage 12's three cases unchanged, with web-wasm on library-gate's own artifact. Ten new cases, per component for its tokens and for its params, read all 104 new tokens and 23 params exactly on their nodes and restore each on removal. A swapped token is caught by name. |
| Anatomy is checked | **Met**: 51 compiler tests, `library_rules::` with the button rules among them. |
| The slot renders | **Met**: library-gate's 28 slotted cells show both marker glyphs on every required host. |
| No per-OS forks | **Met**: `lower-library-platform` clean over all three packages. |
| No regressions | **Met**: conform 0 failures over 114 targets. Release identity: 195 of 203 comparisons identical; 4 are library-gate's own, and 4 are `apps/expose` failing alike. `@exact/reading`'s users are unchanged. |

**QUEUE:** web semantics for a selected bar item (`role="tab"` needs a tablist); the pressed look of a custom popover
invoker; even-odd and two-colour package icons; the Android slot-marker flake. §9 Q5 (per-instance variant pins)
and Q6 (apps restyling parts) stay "for now": the design tool's second pass showed no need.

**Stage 14 (2026-10-26):** the QUEUE's even-odd package icons are done. Every nonzero and even-odd path icon in
`@exact/cards` and `@exact/actions` is a package icon token, drawn at its node's exact frame on every host
(LLP 1121 §10, stage 14 C: `5b2bd17345`..`0e796f5bc8`). Two-colour package icons stay in QUEUE. The third wave is
stage 15 B.
