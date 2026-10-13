# LLP 1121: Facet — a design system in any language, edited through one API

**Type:** RFC
**Status:** Draft r5, 2026-10-13. r1–r4 went through three blind review rounds by GPT-6 Astra (max) and Grok 4.7 (xhigh) (`llp/reviews/rfc-2026-10-13-design-system-r{1,2,3}.{astra,grok}.md`); every verdict after round 1 was RIGHT WITH CHANGES. r5 writes in Charlie's rulings on r4's questions (2026-10-13): the system is **Facet**, with `.facet` files; a design changes through a public, versioned edit API whose main client is Facet's own design tool (D13, D14); components declare their tweakable params in Contract (D15); a component library and a base scale are later stages. r5 is reviewed once (round 4) and r6 takes its findings (§0). Nothing built. Builds on LLP 1095 D4 (`theme`) and its reach (Charlie, 2026-10-12; LLP 1095 §5 item 6), stage 1 here, not redesigned. Stages 3 and 4 need DEFERRED admissions (§9 Q4).
**Systems:**
- A new leaf crate, `facet/` (`exact-facet`): the `.facet` grammar, its checker and the build's evaluator.
- Contract: token names in rows, the `params` section (D15), state parts (D8), `setMode` and `-exact-mode` (D5).
- The Facet protocol server, `facet/1` (D13), and Facet's design tool, an Exact app (D14).
- The plan and the kernel: the reference table the reach is adding, keyed by app modes (D7).
- The hosts: per-mode selection, their existing colour-refresh paths extended to tokens, control roles (D9), the live table (D13).
- The web's JS target and the wasm host: CSS custom properties per mode boundary (D7).
- Generators for TypeScript, Rust, Swift, Kotlin and DTCG (D12, D14).
- The agent API: `prefer` pins an app mode; `tree` names the token a row came from (D13).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** Claude (Opus 5.5), stage 2, starting the day it is accepted (it waits on stage 1's landing). Stages 3–12 are not accepted until each has an implementer and a date (`rules/RULES.md` §Scope).
**Related:**
- LLP 1095 (platform colours): D1 the interned reference and its fallback pair, D4 `theme`, D5 per-view resolution, D6 trait changes transition, D9 keyframes, D10 `color-mix()` and relative colour, D11 the web role sheet, §3 the wire, §5 item 6 (the theme port and the reach ruling).
- LLP 1115 (write the web, ship the platform), LLP 1115.000 D6 (press and disabled feedback; the 2026-10-13 ruling, built as `contract/lower/src/disabled_look.rs`), D10.3 (the hairline), D12.3 (Bold Text), D16 (a list row's highlight), D17 and the schema's `_metrics` (looks as data).
- LLP 1034 (scheme-aware colour: D2 the host resolves `light-dark()`, §8 `color-scheme` on a subtree); LLP 1061 D1 (`press-scale` and the rejected `active` block); LLP 1069.000 D3 (`rem`, `em`, the root font size); LLP 1100 (colour spaces, the wide row).
- `QUEUE.md`: "The app's root font size, what #102 left" (`setRootFontSize("125%")`, the first frame) and "`rem`/`em` where the kernel still takes only `px`".
- LLP 1114 (the Bluesky clone); LLP 1116.012 (round 14: "the lever is the app `theme`").
- `rules/DEFERRED.md` §Tooling ("no Design Mode … no devtools UI") and §Runtime ("hot revision surfaces").
- exact1's Facet, as research: LLP 0008, 0269 Part III, 0304, 0451, 0497, 0497.000, 0514 WP5T, `docs/facet-figma-sync-constraints.md` in `~/projects/exact1`. The survey behind this RFC: `~/.tuft/projects/exact2-theming/facet1-research.md`.
- External: CSS Color 5, CSS Fonts 4, the Design Tokens Community Group's Format and Resolver Module (2025.10), Figma Variables.

## Summary

**Facet** is Exact's design system. An app names its look once, in a `.facet` file that
reads like Contract and whose values are written in CSS's syntax. It declares **dials** (a few typed inputs: an accent, a
roundness), **app axes** (named modes beside light and dark: Bluesky's Dim, a density) and
**tokens** (named colours, lengths, type, shadows and icons). A token's value is a CSS
value or a CSS expression over dials, roles and other tokens.

**The build folds; the hosts resolve as they do today.** One Rust evaluator, at build,
checks the file and folds every expression it can. What it cannot fold, because only the
device knows it, is left in a form the kernel and the hosts already resolve: a role, a
`light-dark()` pair, a `platform-color`, a mix over those (LLP 1095), a `rem`, `em`, text
style or hairline length (LLP 1069.000 D3, LLP 1115.000 D10.3). Nothing new is evaluated on
a device.

**The plan carries references.** A row names a token; the plan's reference table holds,
per token, one value for each combination of app modes that changes it. The kernel selects
the slice for a node's app modes; scheme, contrast and the rest stay inside the value, for
the host, as now. So a Dim switch, a rebrand shipped over the air, and a designer dragging a
dial in development are all one operation: replace rows in one table and refresh what reads
them.

**A design changes through one API.** `facet/1` is a public, versioned edit protocol: reads,
subscriptions, and checked, atomic writes whose durable storage is the files. Facet's own
design tool, the "living Figma", is its main client; agents, Figma, Paper, Pencil and other
tools are clients of the same API, with no side channel (D13, D14). A write reaches every
running development build at once through the live table. **Components declare what may be
tweaked**: a `params` section in a Contract component gives each param a type, a range, an
editor control, a group and a priority, so the tool knows what to surface first (D15).

**The default design is the platform** (LLP 1115). A design names only what a brand
changes; a design's tokens are the author's values, written once. Nothing here replaces a
platform control with a look-alike (the reach ruling).

**No language owns the truth.** The `.facet` file is the source; a JSON IR is generated
from it; Contract checks names against the IR; TypeScript, Rust, Swift and Kotlin get
generated names and accessors over the same live table; other tools reach it through the
protocol, or as DTCG.
Only the build evaluates, in Rust: exact1 wrote its resolver in TypeScript and then ported
it to Rust by hand.

## 0. Review rounds

**Charlie's rulings on r4** (r4 → r5, 2026-10-13):
- **An API is how a design changes.** D13 is now the Facet protocol, `facet/1`: public,
  versioned, reads, subscriptions, atomic checked writes, `propose`, inverse transactions,
  with the files as its storage. The live table is what a write does to running development
  builds. D14 makes Facet's design tool the primary client and Figma, Paper and Pencil
  clients like any other; DTCG is one client's format. The local token stays; remote and
  collaborative clients get a pairing proposal, left open (§9 Q5).
- **The name** is Facet, with `.facet` files (`exact-facet`, `bun exact.mjs facet`,
  diagnostics `facet-*`); alternatives in §9 Q1. exact1's system is called "exact1's Facet"
  throughout.
- **A component library and a base scale** are wanted later: stages 11 and 12, no longer
  non-goals.
- **Component params** (D15): a `params` section in the Contract component, with types,
  ranges, controls, labels, groups and priorities; the same block in a `.facet` file for a
  component that is not Contract. App dials take the same metadata (D4).
- **The DSL in files** stands; the extension is `.facet`.

**Round 1** (r1 → r2). Astra: RIGHT WITH CHANGES (2 BLOCKER, 12 MAJOR, 1 MINOR). Grok:
WRONG (3 BLOCKER, 9 MAJOR, 4 MINOR). They agreed on the core (named tokens as plan data,
the platform as default, Dim as an app axis) and found the resolution pipeline wrong. Each
finding is taken:
- **Where modes resolve** (Grok B1, Astra B2). r1 put the scheme in a kernel mode vector,
  against LLP 1034 D2 and LLP 1095 D1/D5. Now the kernel selects only the **app-axis
  slice**; scheme, contrast, level and material stay inside the value for the host. On the
  web, a pinned subtree gets its whole resolved slice as properties on its own element, so
  nested pins work and no derived variable is left resolved at the wrong level (D5, D7).
- **What may stay unresolved** (Astra B1, Grok B2/B3). The residuals are now a closed list
  of forms the kernel and hosts already resolve, and r1's extensions are gone: no new
  colour variant, no wire mixes beyond LLP 1095 D10's subset, `calc(rem)` waits on its
  queue item, and type tokens lower to the four `font` longhands plus `font-family` (D6,
  D7; r3 corrects how weight and Bold Text follow).
- **Precedence** (Grok M4). No extra tier: a design supplies the app's authored values
  (D1).
- **`setMode` and the root size** (Grok M5). `setMode` selects the app slice; `setScheme`
  is unchanged; the scale is the queued `setRootFontSize("125%")` (D5, D6).
- **The mode key** (Grok M6): disjoint rows keyed by the app-axis tuple, specificity
  compiled away (D5).
- **Invalidation** (Grok M7, Astra M3/M4/M5): per use, with the domain from the row the
  token is used in; inherited colour and `currentcolor`; text caches; transitions; the
  hosts' existing refresh paths named; transitive recomputation and one atomic batch;
  index maintenance; a JS-target path (D7, D13).
- **Kinds against row codecs** (Astra M4): a token is checked against each row that uses it
  (`box-shadow` and `text-shadow` are separate kinds); r1's `Copy + Eq` claim is dropped
  (D3).
- **Dynamic names and pruning** (Astra M8): the reachable closure, or the whole table (D7).
- **State parts** (Astra M6/M7, Grok M8): precedence among states, base restoration,
  `:active` as the host's existing press (its curve, Reduce Motion, the agent clock),
  native controls and list rows excluded, the existing `disabledStyled` mark kept and
  existing authored forms still counting, `:checked` defined (D8).
- **Controls** (Grok M9, Astra M10): extend the `accent-color` path that already reaches
  switches, ranges and progress; a control role only where `accent-color` cannot name the
  parameter; a column for every shipping host; Android's real reader (Views and Canvas), no
  Compose (D9).
- **Icons** (Grok M10): the 76 symbol roles stay; icon tokens are the checked form for
  other names (D10).
- **The base** (Grok M11, Astra M12): roles, text styles and the schema's existing named
  lengths and metrics; no exact1 Facet spacing scale; package merge rules (D11).
- **Bindings** (Astra M9/M10, Grok m14): stable names to session-local handles,
  subscriptions, per-session tables, trait context; Android Views first; the right command
  (D12). Bindings no longer wait for the editor (§5).
- **The editor** (Astra M11, Grok m15): the dev gate does not authenticate a LAN client, so
  a project-scoped write capability, digest preconditions and atomic publish are specified;
  the channel is called what it is, a development-only hot revision (D13).
- **DTCG** (Astra M13): the 2025.10 Resolver Module has modes; versions pinned; conversions
  and losses defined; the round-trip gate restated (D14).
- **Names** (Astra M14): LLP 1095 D4's undotted app names (`brand`, `card`) stay legal (D3).
- **Staging** (Grok M12, Astra M12): stage 2 is colour tokens and app axes on the reach's
  table after it lands; each stage lists its schema-digest break and every host that must
  read it (§5).
- **Minors:** the grammar has the header and the kind annotation and current spellings
  (`-exact-press-scale`, `-exact-platform-color`); "values are CSS" is restated as CSS
  syntax with token names, rewritten at emission (§3); the no-design gate is unchanged
  behaviour and no design payload (§5); gradients take colour tokens in their stops, and
  the hairline stays the named length (D3); keyframes refuse references until LLP 1095 D9's
  keyframes land (D7).

**Round 2** (r2 → r3). Astra: RIGHT WITH CHANGES (1 BLOCKER, 10 MAJOR, 1 MINOR). Grok:
RIGHT WITH CHANGES (1 BLOCKER, 7 MAJOR, 3 MINOR). Both confirmed the round-1 fixes to
resolution, residuals, precedence, invalidation, pruning, controls, icons and the base, and
found the same blocker. Each finding is taken:
- **Nested pins** (both B). r2 emitted only what differs from the default, so a default-tuple
  pin under a non-default one inherited its parent's values. Now every mode boundary
  declares its **whole** slice; the properties stay unregistered and their values stay
  `light-dark()` and role variables; a boundary re-specifies the inherited rows whose source
  is a token, on the web and in the kernel, so inherited text re-selects under a pin (D7).
- **`under scheme=dark` for non-colours** (Grok M2): refused (D5).
- **Type and Bold Text** (Grok M3, Astra M3): a type token on a text style goes through the
  `font` shorthand's existing path, which supplies the style's weight and leading; Bold Text
  is the system font's accessibility pass whatever the weight (D6).
- **Role-override keys** (Grok M4): the author-facing names (`-exact-secondary-label`),
  exempt from the keyword refusal; switches take the root `accent-color`; the inline `theme`
  block stays LLP 1095 D4's colour entries (D2, D3, D9).
- **State composition** (Grok M5, Astra M6/M7): a complete order (node over class;
  `:disabled` over `:checked` over the base; host overlays over both); `checked` means true;
  the web's `:active` is its press glue; host overlays take only non-inherited rows (D8).
- **Durations and easings** (Grok M6): a component substituted inside `transition` and
  `animation` at build; they may not vary by app axis (D3).
- **Bindings** (Grok M7, Astra M4/M5): a handle bound to a node or view, for its app tuple;
  observation of traits, the root size and pins; tokens a binding can reach are kept (D7,
  D12).
- **The editor** (Grok M8, Astra M8/M9/M10): the capability only on a verified-local
  response, as the install token is; runner-known state edits rebuild; a batch names its
  base and new design digests, a session gets a full snapshot on connect or mismatch, and the
  server's own write does not trigger the watcher's reload (D13).
- **Folding and colour spaces** (Astra M2): a folded colour keeps the form CSS would
  serialize, under LLP 1100's rules (D7).
- **DTCG** (Astra M11, Grok m10): one modifier over the whole tuple, every tuple checked
  against the folded table; icons outside the token set; `em` lossy and marked (D14).
- **Minors:** Android draws an icon's SVG and `material` is an annotation (D10); the
  example's default is Black (`black | dim`), and `setMode` lays out only when a selected
  value is in a layout row (D5); stage 2 has an implementer (header).

**Round 3** (r3 → r4, written in without another round). Astra: RIGHT WITH CHANGES, no
BLOCKER (9 MAJOR). Grok: RIGHT WITH CHANGES (1 BLOCKER, 10 MAJOR, 4 MINOR). Both confirmed
the round-2 fixes to the web's boundaries, state order, overlays, binding context,
retention, the capability and the DTCG tuple export. Each finding is taken:
- **Inherited colour under a pin in the kernel and the wasm host** (Grok B1). The kernel's
  inheritance walks past ancestors without the row, so comparing a node's tuple with its
  parent's missed the case. Now one rule on every target: **a mode boundary re-specifies
  each inherited token row** with its value for the boundary's tuple, as an implied row on
  the boundary node, which the wasm host emits and descendants inherit (D7).
- **Re-specified fonts would compound `em`** (Astra M4): re-selection is for colour rows; an
  inherited non-colour row may not name a token that varies by app axis
  (`facet-inherited-axis`) (D7).
- **Role variables resolve where a custom property is declared** (Astra M1): only the accent
  is element-contextual on the web, so an element with an authored `accent-color`
  redeclares the token properties whose values name it (D7).
- **Lookup keys** (Grok M3): every full tuple has an entry, by exact match (D5, D7).
- **Folding** (Astra M2, Grok M10): `Fixed` only for what CSS serializes as legacy `rgb()`;
  a `light-dark()` with a modern half is one `Wide` with its dark side (D7, §3).
- **Typography** (Astra M3, Grok M2): LLP 1115.000 D12.2 (unsaid weight and leading resolved
  per platform) is stage 4's prerequisite, and a type token sets only the longhands it writes
  (D6).
- **Previews** (Grok M4): a mode pin does not change the scheme; a Light preview is
  `color-scheme="light"`, Dim and Black are `color-scheme="dark"` plus the pin (D5).
- **Swift colours and app modes** (Grok M5): the provider reads its place's tuple, and the
  binding invalidates the view on `setMode`, a pin and a swap (D12).
- **A token in a row expression** (Grok M6) becomes a synthetic token folded per tuple at
  build (D3).
- **Role-override keys** (Grok M7): D4's bare key and the spelled role are the same override
  in both files (D3).
- **The lexer** (Grok M8): the design lexer is the Contract lexer extended with `|`, `..` and
  CSS value text (§3).
- **Retention** (Grok M9, Astra M5): one rule, in D7, with role and control-role overrides as
  unconditional roots (D7, D11).
- **Live edits** (Astra M6/M7/M8, Grok M11): state-part and motion-token edits rebuild and
  reload; the design endpoint lives in the dev-loop process, so its own write is skipped
  in-process; reclaimable live colour storage is stage 7's prerequisite, since `Wide`
  entries are global, immutable and capped at 4,096 (D13).
- **DTCG import** (Astra M9): values are compared with the exported baseline per tuple (D14).
- **Minors** (Grok m12–m15): roles and `platform-color` on an `under` axis; `border-color`
  expanded before the state order; `facet-motion-axis`; the implementer's start (header).

## 1. Motivation

### What the Bluesky clone hit

The clone (`~/projects/bluesky-exact2`, LLP 1114) is the first app here with a brand, three
appearance variants and a font-size setting:

| Gap | What the clone does today | Decision |
|---|---|---|
| Light, Dim and Dark (true black); `light-dark()` has two sides | 244 inline `light-dark(…)` literals (`screens.contract` 112, `post.contract` 70, `sheets.contract` 33, `chat.contract` 24, `app.contract` 5); the palette is a comment (`post.contract:4-7`); Dim needs a palette threaded from the root, about 500 colour reads | D3, D5 |
| The app cannot scale the person's text size | Smaller and Larger replace Dynamic Type through `setRootFontSize(px)` | D6 |
| State looks are the host's | the press is the host's; a disabled look the app styles was double-dimmed until the 2026-10-13 ruling | D8 |
| Native control styling leaks through | the disabled Post button is a grey prominent-glass pill, not faded blue; switches were the app's black accent until fixed by hand | D8, D9 |
| Icons | about 100 `symbol:sf/…` uses draw nothing on the web | D10 |

### What the rest of the repository says

- **Five apps hand-roll a `Theme` record**, and Expose wrote one colour pair 26 times
  (LLP 1095 §5 item 6). The D4 port took Expose from 274 scattered hex colours to 61 in one
  block with screens identical, and listed what D4 leaves out: typefaces and sizes (Expose
  writes `font-family` 48 times), radii, gradients, strengths of one colour, and "a palette
  the person chooses at run time". Those are D3, D5 and D6.
- **T3 Code's theme editor** outlines and counts the nodes each colour role paints. The
  clone needed a `data-paint` word on every themed node and a new `querySelectorAll` read
  (`issues/closed/20261010-paint-role-node-list-read.md`). With references in rows, "where
  is this token used" is a lookup (D13).
- **Round 14** graded the web's neutral default plain; "the lever is the app `theme`"
  (LLP 1116.012).

### What exact1's Facet taught

| exact1 Facet | Lesson | Here |
|---|---|---|
| Dials → a deterministic resolver → semantic tokens; runtime inputs (scheme, contrast, text scale) kept apart from the design | Right | D4, D5, D6 |
| Purpose-named tokens, identifier-safe paths (`s4`, `xxs`), on-colours by the fill's luminance, density as a table of control sizes | Right | D3, D11 |
| Tokens as TypeScript objects "by decree" (0008); the resolver in TS; a second Rust resolver held to it by a parity corpus that reproduced JS rounding (0514 WP5T) | The root failure | D2: data, one build evaluator, in Rust |
| Recipes as TS ternaries, a sidecar descriptor and a generated table, unchecked against each other (0497) | One representation | D8: recipes are `style` blocks |
| `type.ratio` and the status hues went inert once Figma pinned values (0269) | Every dial must move a token | D4 |
| Dials and Figma-as-authority never reconciled | An authority per class | D14 |
| Apps kept private palettes and raw px; adoption "C+" (0304) | App tokens first-class | D11 |
| A theme flip was O(tree) on native; 0497's answer was swapping data, with dependency domains | Swap rows; refresh by use | D7 |
| Design Mode was DOM-only, since provenance came from the DOM mirror | Provenance from references | D13 |
| ~35 LLPs, 6 authorities, ~30 checks, three generators | One file, one command | D2, D12 |

Exact2 has refused what exact1's Facet was built from: a second value graph, JS above the data seam,
package imports of theme code (`contract/corpus/rejects.txt` keeps
`use theme from "@exact/facet-contract"`), and selectors. "The tokens are data" survived
(`llp/1033-markdown-viewer.plan.md:493-511`). This RFC builds that sentence out.

## 2. Decisions

### D1 — The platform is the default; a design is the author's values, written once

LLP 1115's order is unchanged: **the person's accessibility settings where a hand-built app
applies them, then the author, then the platform, then CSS's default.** A design adds no
tier. A token is an authored value that the author wrote once instead of at every use: a
row that names `surface.card` is as written as a row that names `#f1f3f5`, and carries the
written bit (LLP 1115 §2). A node's own literal still beats a class or a default as today.
Accessibility beats a token exactly where it beats a literal: Increase Contrast moves a role,
not an authored colour, and Bold Text weights the system font (LLP 1115.000 D12.3).

- **Semantic colours stay the platform's roles** (LLP 1095 D2). A design does not invent a
  second set: an app's secondary text is `-exact-secondary-label` unless its design
  overrides that role, and an override reaches each surface the platform lets it reach, no
  further (the reach ruling). The compiler reports, per target, what an override cannot
  reach (D9).
- **An app with no design behaves exactly as today**, carries no design payload, and no host
  does any work for one.
- **A design never draws a control.** Where a brand wants a look a native control cannot
  take, the author writes their own control, a styled pressable (D8, D9).

*Rejected:* a neutral brand like exact1's Facet as the default. LLP 1115 exists to stop a look that
"was not made here".

### D2 — One source, the `.facet` file; a generated IR; one build evaluator, in Rust

**The source is `app.facet`** beside `app.contract` (or named in `app.json` as `"facet"`).
Its grammar is Contract's lexical grammar, extended (§3): indentation blocks, `name =
value` lines, `//` comments, plus `|` and `..`, with each value read as CSS value text. Its values are written in CSS's syntax, with token names where CSS would write
`var()` (§3). LLP 1095 D4's `theme` block in a `.contract` file stays legal and stays what D4
says: colour entries only, with no `when` lines and no other kinds. An app that wants more
moves it to `app.facet`, whose `theme` block is a superset. An app has one or the other
(`facet-two-sources`).

**`exact-facet` is a leaf crate.** It parses, checks (names, kinds against each using row,
ranges, cycles, the completeness rule of D4) and folds. It is the **build's** evaluator,
used by:
- the Contract compiler, which writes the folded table into the plan (D7);
- `bun exact.mjs facet check | fmt | eval --mode dark-style=dim | gen <target> | export dtcg`;
- the dev server, for the editor (D13). The editor's own web page loads it as wasm; a
  shipped build never carries it.

**Run-time resolution stays where it is.** The kernel resolves `rem`, `em`, text styles and
the hairline at the end of each commit (`kernel/src/style/relative.rs`); the hosts resolve
roles, pairs, platform colours and mixes per view (LLP 1095 D5); the web's JS target leaves
both to the browser. A design adds rows these paths read; it adds nothing for them to
evaluate.

**The IR is generated, never committed** (DEFERRED §Process): `facet.json`, with a format
number and the design's digest. Every generator and the editor read it; none parses the DSL.

**`facet fmt` owns the file's layout**, so an editor edit is a one-line diff in a file a
person can still edit, comments kept.

*Rejected:*
- **TOML or JSON as the source.** Per-mode values and expressions read badly; the source must
  be hand-editable and round-trip through the editor.
- **DTCG as the source.** It has no dials and no expressions; it is the export (D14).
- **Contract alone.** Rust, Swift and Kotlin projects that embed Exact (the sample host, LLP
  1031; native modules, LLP 1024; LLP 1075.003's hatches) need the design without a view in
  Contract. The grammar is shared, so nothing is lost.
- **A TypeScript or Rust API as the source.** exact1 Facet's root failure, in either language.

### D3 — Tokens: names, kinds, and the rows that take them

**Names.** An identifier, or identifiers joined by dots: `brand`, `card`, `surface.card`,
`space.row`, `type.body`, `icon.reply`. A segment is an identifier (`s4`, not `4`). A name
spelled as a role is written (`-exact-secondary-label`, `CanvasText`: what
`kernel/src/style/roles.rs` reads as a colour) overrides that role, within the reach ruling;
role names are the one exception to the keyword refusal. LLP 1095 D4's bare key
(`secondary-label`) and the spelled role (`-exact-secondary-label`) are the same override, in
an inline `theme` and in `app.facet`. Any other name equal to a CSS
keyword or named colour is refused, as D4 refuses it. LLP 1095 D4's
undotted app names (`brand`, `card`, `defrost-on`) stay legal; dots are a convention for
grouping, which the editor and DTCG use.

**Kinds**, each checked against every row that uses the token (`facet-kind` names the row
and why):

| Kind | Value | Rows | Lowers to |
|---|---|---|---|
| `color` | any CSS colour, `light-dark()`, a role, `-exact-platform-color()`, a mix (§3) | every colour row and embedded colour LLP 1095 D1 lists, gradient and mask stops included | an existing `ColorValue` form; a reference row once the reach lands |
| `length` | px; one `rem`, `em`, text style or `-exact-hairline`; `calc()`/`min()`/`max()`/`clamp()` folded to px | each length row whose codec takes the form (the kernel's relative side table today; `calc()` with `rem` waits on its queue item) | a dimension, or the relative side table's entry |
| `number` | a number | `opacity`, `line-height`, `flex-grow`, `font-weight`, `-exact-press-scale` | the row's number |
| `type` | CSS `font` syntax: style, weight, size (px, `rem`, or a text style), `/line-height`, family | `font` | the four longhands `font` sets today (`contract/lower/src/shorthands.rs`) plus `font-family` |
| `family` | a font-family stack | `font-family` | the row |
| `box-shadow`, `text-shadow` | each row's own syntax (`text-shadow` has no spread) | that row | the row's codec |
| `duration`, `easing` | CSS time; `cubic-bezier()`, a keyword | a component of a `transition` or `animation` value (`transition="background-color duration.fast easing.standard"`) | substituted into the value at build, before the row's codec reads it; so a `when` on one is refused (`facet-motion-axis`), and an edit to one rebuilds (D13) |
| `icon` | `icon(…)` (D10) | `symbol` | a symbol source |

A gradient is colour tokens in its stops; there is no gradient kind. The hairline stays the
named length `-exact-hairline`, usable as a length token's value.

**A row names a token as a string**, as LLP 1095 D4 does: `background-color="surface.card"`,
`padding="space.row"`, `font="type.body"`. Inside an expression in a row, a token is a bare
name: `color="color-mix(in srgb, brand 60%, transparent)"`. The compiler checks every literal
name (`facet-unknown-token`). A row expression that names a token is lowered to a
synthetic token holding the whole expression, folded per tuple at build like any other, so
the row still holds one reference and no device evaluates the mix. A name computed at run
time (from data) is resolved by the
runtime parser against the plan's table, as D4 resolves `"brand"`; an unknown name is an
invalid value in the log.

*Considered:* CSS's own `var(--surface-card)`. Rejected for r2: it brings custom-property
inheritance, a cascade by another name, which Contract has refused; D4's bare names are
built and ported. §9 Q3 asks.

### D4 — Dials: typed inputs that tokens are folded from

```
dials
  accent: color = #1083fe
  roundness: number = 1  in 0..2
```

- **A dial is typed and ranged**: `color`, `length` or `number`, a default, and for numbers
  and lengths a closed range. The design tool draws its control from the type, or from the
  editor metadata a component param takes (`step=`, `control=`, `label=`, `help=`,
  `group=`, `priority=`, D15); an agent sets it by name. An out-of-range value is refused,
  not clamped.
- **Tokens fold from dials**, roles and other tokens: `radius.card = calc(12px * roundness)`,
  `accent.on-fill = contrast-color(accent)`.
- **An enumerated choice is an axis**, not a dial (D5): one way for a token to vary by a
  choice.
- **Completeness.** Every dial moves at least one token, or `facet check` refuses it
  (`facet-dial-inert`). The IR lists, per token, the dials it depends on.
- **A dial is a design-time input.** A shipped app's dials are fixed and folded at build.
  Changing one at run time is the editor's (D13); a person's run-time choice is an app axis.
- **No generative palette yet.** exact1 Facet's `ramp()` is left until an app wants a palette made
  from one colour; real brands (Bluesky's) are hand-tuned, written as literals, with a dial
  only where the brand is meant to move.

### D5 — App axes: named modes beside light and dark

```
axes
  dark-style = black | dim  under scheme=dark
  density = default | compact | comfortable
```

- **Scheme and contrast are not app axes.** They are the host's traits, as now: a token
  varies by scheme with `light-dark()` or a role inside its value, and by contrast through a
  role, and the host resolves both per view (LLP 1034 D2, LLP 1095 D5). Dim is dark to the
  platform, so the platform's controls are dark on a Dim page.
- **An app axis is declared in the design**, its first value the default. `under scheme=dark`
  says the axis matters only where the host shows dark.
- **A token varies by app axis with `when` lines**, which name app axes only:

```
theme
  surface.page = light-dark(#ffffff, #000000)
    when dark-style=dim: light-dark(#ffffff, #161e27)
  space.row = 12px
    when density=compact: 8px
```

- **The mode key.** The compiler resolves each token for every full tuple of app-axis
  values: the table holds, per token that varies, an entry for each tuple (pointing at one
  of its distinct values), and the kernel looks a node's tuple up by exact match. A token
  that varies by no axis has one value and no entries. A `when` that names more axes wins
  over one that names fewer; two that both hold and neither names more are refused
  (`facet-when-ambiguous`). An axis declared `under scheme=dark` is resolved by the host
  side: its values are `light-dark(<the default's light side>, <the axis value's dark
  side>)`, so the light side never varies by it. A `when` on it may write one colour, its dark
  side, or a `light-dark()` whose light side is the default's; another light side is refused
  (`facet-under-light`). A role or `platform-color` has no light side to keep, so as such a
  `when`'s value, or as the default of a token that has one, it is refused unless written
  inside a `light-dark()` (`facet-under-role`). Only colours may vary by such an
  axis: the kernel picks a length's slice without knowing the scheme, so a non-colour `when`
  on it is refused (`facet-under-kind`). A design whose axes multiply past 64 tuples is
  refused with the axes named.
- **`setMode("dark-style", "dim")`** sets the window's app slice: a runner command beside
  `setScheme`, which stays the host's appearance command. The new slice applies in the
  action's own commit, refreshed by D7's per-use domains: repainted, and laid out only where
  a selected value is in a layout row. Undeclared axes and values are refused at compile.
- **A subtree pins app modes** with `-exact-mode="dark-style:dim"` (several, comma-separated),
  as `color-scheme` pins the scheme (LLP 1034 §8). A pin inherits the axes it does not name
  from above. A mode pin never changes the scheme: a settings screen previews the three
  appearances side by side as `color-scheme="light"`, and `color-scheme="dark"` with
  `-exact-mode="dark-style:dim"` or `"dark-style:black"`. The editor shows one screen in
  every mode the same way.
- **Bluesky:** its three-way appearance is `setScheme` for light, dark or system, and
  `setMode("dark-style", …)` for Dim or Black. No palette is threaded; the 244 literals
  become names.
- **An authored contrast variant** for an app colour (`when contrast=more`) is not in r2. A
  host would need a value form carrying four colours; LLP 1095 D11's high-contrast columns
  are the place to design it. §9 Q6.

### D6 — Text scale is the host's; type is relative to it

- **Type tokens take the platform's text styles and `rem`.** `type.body = -exact-body/1.35
  "InterVariable"` takes the body style's size and weight, with `line-height: 1.35` and
  `font-family: "InterVariable"`; it follows Dynamic Type and the browser's setting (LLP 1115 D3). A px
  size is the author's choice not to scale, and `facet check` warns (`facet-type-fixed`).
- **A type token sets only the longhands it writes.** `font="type.body"` writes
  `font-size: -exact-body`, `line-height: 1.35` and `font-family: "InterVariable"`, each a
  written row; the weight it omits stays unsaid, so the text style's own weight applies.
  Today's `font` shorthand cannot do that: it writes a numeric weight and `line-height:
  normal` and marks all four written (`contract/lower/src/shorthands.rs`). LLP 1115.000
  D12.2, an unsaid weight and leading resolved per platform with per-longhand provenance, is
  therefore stage 4's prerequisite. When a token's value changes which longhands it writes
  (an update, a dev reload), the ones it no longer writes are cleared.
- **Bold Text is not a consequence of an unsaid weight.** It is the accessibility pass on the
  system font, which weights it even under an authored weight and leaves a bundled font
  alone (LLP 1115.000 D12.3). A token's weight is an authored weight like any other.
- **The scale is the queued command**, `setRootFontSize("125%")`: CSS's percentage of the
  host's `medium`, so Smaller and Larger scale Dynamic Type instead of replacing it, and a
  Dynamic Type change under a stored percentage re-resolves (today an app size makes later
  Dynamic Type updates wait, `runner/src/runner/root_font.rs`). It is `QUEUE.md`'s "The
  app's root font size" item, and this RFC is the app that asks for it. Persistence and a
  stored size at the first frame are that item's second half; `setMode` asks the same
  question (§9 Q6).

### D7 — The plan carries references; the kernel picks the app slice; hosts resolve the rest

This widens LLP 1095 D1 and D4 ("the theme ships in the plan as part of the reference
table") from colours to the kinds in D3, keyed by app modes.

**The table.** One row per token the build keeps: `{ kind, values, entries: [value index
per full app tuple], fallback }` (D5's mode key).
- **A value is an existing form**, not a new one. A colour is a `ColorValue` the kernel
  already has (`Fixed`, `LightDark`, `Role`, `Platform`, `Wide`, `Profiled`; a mix once LLP
  1095 D10's table lands). A length is a dimension or an entry of the relative side table.
  The rest are their rows' own codecs.
- **The build folds everything else.** An expression over literals and dials folds to a
  constant: any CSS Color 4/5 function (`oklch()`, relative colour with channel `calc()`,
  `contrast-color()`). The folded colour keeps the form the author would get by writing the
  result as CSS Color 5 serializes it, under LLP 1100's rules: `Fixed` only when CSS
  serializes the result in legacy `rgb()` form (a hex, `rgb()`, `hsl()`, `hwb()` or named
  colour, or a token that is one); every other result, a `color-mix()` in any space and a
  relative colour included, serializes as `color()` or its own space and is `Wide`, even
  inside sRGB, so its precision and its interpolation in a transition
  (`kernel/src/motion.rs`) are what the literal's would be. A `light-dark()` whose halves
  are both legacy is `LightDark`; one with a modern half is one `Wide` with its dark side
  (`kernel/src/style/wide.rs`), as the parser makes it. An expression over a host value (a role, a pair's side, a `platform-color`) may only
  be one LLP 1095 D10 admits (`color-mix(in srgb|oklab)`, `rgb(from X r g b / a)`, depth 2),
  and only once D10 is built; any other is refused (`facet-residual`, naming the host value
  that stops the fold). `contrast-color()` of a role is refused for that reason.
- **The fallback is a pair**, D1's rule: light and dark, for the default app tuple, with
  roles at their fallback pairs. The kernel, Linux and headless runs always have a value.
- **Retention, the one rule** (packages included, D11). The build keeps the closure of:
  every token a literal names; every role and control-role override, unconditionally, since
  a kernel default (`CanvasText` for unsaid text) or a native control may read it with no
  row naming it; every token of a kind that a computed string may name (a string from data
  in a colour row), as LLP 1095 D11 keeps the whole role sheet for computed roles; and, in
  an app with generated bindings (D12), every token, unless its design lists an `exports`
  block, which then bounds what the bindings may name.

**The kernel picks the slice, per node.** A node's app tuple is the window's (`setMode`),
overridden axis by axis by the nearest `-exact-mode` above it. The kernel stores the
selected value in the row, so every reader sees an ordinary value, and keeps the token beside
it.

**A mode boundary re-specifies inherited colour tokens.** Inheritance copies the nearest
ancestor's row past nodes that lack it (`kernel/src/arena.rs`), so comparing a node with its
parent cannot catch a pin. Instead, at each node with `-exact-mode`, the kernel writes every
inherited colour row (`color`, `fill`, `stroke`, `-exact-tint-color` …) whose inherited
value came from a token as an implied row on that node, holding the token's value for the
node's tuple. Descendants inherit it as any row; the wasm host emits it in the boundary's
CSS (`host/web/src/element.rs`); the JS target does the same (below). So text inside a Black
preview on a Dim page is Black's colour, though its row was written on the page. **Only
colours re-select.** Re-specifying a font would compound an `em` size at the boundary
(`kernel/src/style/relative.rs`), so an inherited non-colour row may not name a token that
varies by an app axis (`facet-inherited-axis`).

**What refreshes when a slice or a row changes.** The kernel keeps, per token, the nodes
whose rows hold it, maintained when a node mounts, retires, moves under another pin, or has
a row rewritten by a binding. A change marks those nodes by the **row** they use it in, from
the schema's existing flags (a `number` in `opacity` is paint, in `line-height` layout; an
icon whose source changes size is layout). Then the existing paths run:
- **inherited colour and `currentcolor`** follow the parent's computed value as today
  (`kernel/src/txn.rs`'s propagation), re-specified at a boundary as above; the index
  records the node that holds the row and every boundary under it;
- **text** colours and fonts invalidate the host's text caches, as an appearance change does
  (LLP 1034 D2);
- **a `transition`** on a row eases the change, as a trait change does (LLP 1095 D6); without
  one it snaps;
- **keyframes** refuse a token reference until LLP 1095 D9's keyframe references land.

**Native hosts.** A colour value that varies only by scheme and contrast stays a dynamic
`UIColor`/`NSColor`, so the OS re-resolves it per view. That is not all of the work, and r1
was wrong to say so: the Apple host already re-resolves `CGColor` layers, gradients, filters,
SVG paint and transition targets on an appearance change (`host/apple/src/colors.rs`, LLP
1095 D5, D9). The same paths run for a token row change and an app-slice change. Linux and
Android's reader repaint from the kernel's resolved rows (`host/linux/src/paint.rs`).

**The web.** The JS target keeps no kernel at run time, so the browser resolves:
- every kept token is a custom property, `--x-<name>`, **unregistered**, so its value is a
  token stream the browser substitutes where a row uses it, not a colour computed on the
  element that declares it. Its value is the token's row value: `light-dark()` and role
  variables (`var(--exact-<role>, …)`, LLP 1095 D11) inside, so scheme and contrast resolve
  at the using element against its inherited `color-scheme` and `prefers-contrast`;
- **every mode boundary declares its whole slice.** The runtime gives the root and each
  element with `-exact-mode` a `data-x-modes` attribute holding its full app tuple (its pins
  over the inherited ones), in one canonical order (axes as declared, `axis:value`
  joined by commas). The page has one rule per tuple the app can reach (at most 64),
  selected by exact match, and each rule declares **every** token that varies by any app
  axis, default values included. So a Black pin inside a Dim page restores Black, and no
  property is inherited across a boundary. No token's value is a `var()` of another token;
- **a boundary re-specifies inherited colour tokens**, the kernel's rule: CSS inherits
  `color` as a computed colour, so the runtime, which knows for each inherited row whether
  its nearest specified value is a token, writes that row again (`color:
  var(--x-text-primary)`) on the boundary element;
- **the accent is the one element-contextual role.** A `var()` inside a custom property is
  substituted on the element that declares it, so a token whose value names
  `var(--exact-accent, …)` would keep the root's accent under a child with its own
  `accent-color` (`kernel/src/style/roles.rs`'s nearest-accent rule). Every other role
  variable is declared once at the root and its value is a `light-dark()` pair or a
  keyword, which survive substitution and resolve at use. So an element with an authored
  `accent-color` redeclares, for its own tuple, every token property whose value names the
  accent;
- `setMode` and a changed pin rewrite `data-x-modes` on the boundary and recompute the tuples
  of the boundaries beneath it, which inherit the axes they do not pin;
- the wasm host computes its CSS from the kernel's rows as today, the boundary's implied
  rows included.

**Wire and compatibility.** The reference table's widening to every kind, and the app tuple
on the row, change the kernel schema digest once, so every native host needs a new binary
then (LLP 1030 D3a). Other stages break it again, each listed in §5. Between breaks, a
token, a value or a new axis value is plan data that an update replaces over the air (LLP
1095 §3).

### D8 — State looks: a state part, paint rows only, the host's own press

The Bluesky gaps are a pressed look and a disabled look the author wants to own. Today
`-exact-press-scale` is a row (LLP 1061 D1), the press and disabled feedback are the host's
(LLP 1115.000 D6), and a styled disabled look suppresses the dim by a compile-time heuristic
(`disabledStyled`, `contract/lower/src/disabled_look.rs`).

**A state part is a block under a node or in a `style`:**

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

- **States:** `:active`, `:hover`, `:focus-visible`, `:disabled`, `:checked`. `:checked`
  holds when the node's `checked` is true, or its `aria-checked`, `aria-selected` or
  `aria-pressed` is `"true"`: one name for a toggle and a selected row, declared as a
  deviation in LLP 1001. No combinations, no nesting, no attribute selectors, never another
  node.
- **Rows:** in a runner-known state, `background-color`, `color`, `border-color`, `opacity`,
  `box-shadow`, `-exact-tint-color`; in a host-known state, only the rows that do not
  inherit: `background-color`, `border-color`, `opacity`, `box-shadow`,
  `-exact-press-scale`. Paint only, so a state never relayouts. A host overlay never changes
  an inherited colour, so it never has to recolour descendant text, inline runs, SVG or
  `currentcolor` outside the runner's commit; a pressed text colour is a later step with
  host-side inheritance designed for it. Another row is `lower-state-row`.
- **Runner-known states** (`:disabled`, `:checked`) lower to the conditional rows an author
  can write today. **Host-known states** (`:active`, `:hover`, `:focus-visible`) lower to a
  **state look**: an interned entry `{ state, rows }` the node points to, applied by the
  host on its own tracking, with no round trip (LLP 1061 D1's reason).
- **`:active` is the host's existing press**, not CSS's: the innermost node with a `press`
  handler (LLP 1061), eased on the curve the host uses for `-exact-press-scale` today
  (`cubic-bezier(.16, 1, .3, 1)` on Apple), held under Reduce Motion with colour changes
  snapping, and unsmoothed on the agent's clock. On the web it is the runtime's existing press
  glue (`data-pressed`), never a stylesheet `:active` rule, which would match ancestors and
  nodes with no `press` handler. A written `-exact-press-scale="0.97"` is that state's
  scale.
- **Composition, per row**, after shorthands are expanded (`border-color` to its four
  sides, so a state's `border-color` overrides each side the base wrote, longhand or not),
  from lowest to highest:
  1. the class's rows, then the node's own rows (the node wins, as today);
  2. `:checked`, the class's then the node's;
  3. `:disabled`, the class's then the node's;
  4. the host overlays, `:hover`, then `:focus-visible`, then `:active`, each the class's then
     the node's.

  Steps 1–3 are the base, folded by the runner into conditional rows. A disabled node takes
  no press, hover or focus, as today. An overlay's values are absolute; when the base or the
  class changes under an active overlay, the host applies the overlay over the new base, and
  rows the overlay does not name show the new base. The host's ease owns overlay changes; an
  authored `transition` owns base changes.
- **A declared state replaces the host's default for that state only.** `:disabled` sets the
  same `disabledStyled` mark the heuristic sets, and the heuristic's existing forms (an
  authored `opacity`; a colour that reads what `disabled` reads) still count. `:active`
  turns off the host's unsaid press look on that node (iOS's dim, the Mac's ink wash, the
  web's 65%).
- **Excluded:** native controls (their states are the platform's, D9) and a `list`'s rows
  (their press is the cell highlight, LLP 1115.000 D16). A state part on either is
  `lower-state-target`.
- **This amends LLP 1061 D1**, which rejected an `active` block as "a selector system for one
  property". It is now seven rows and five states over tracking the host already does, and
  it never selects another node. `::placeholder` (LLP 1104 D3) is the precedent for a part as
  a block, not for these rows. §9 Q7 asks.

**Recipes are `style` blocks**: tokens and state parts in a `style`, chosen by `class`, which
merges without cascading. No component library or anatomy schema (§8).

### D9 — Native controls: `accent-color` first, control roles only where it cannot reach

A native control takes the platform's supported parameters, never Exact paint over it.

- **`accent-color` already reaches** switches, ranges and progress on iOS and the web, and
  Linux's painted controls (`GroupedListIOS.swift:901`, `RangeIOS.swift:23`,
  `host/linux/src/paint/control.rs:155`), and `AccentColor` follows it
  (`follows_accent`, `kernel/src/style/roles.rs`). A root `accent-color` naming a token
  (`accent-color="accent.fill"`) colours them. Bluesky's switches need nothing else.
- **A control role** is added to the schema only for a parameter `accent-color` cannot name:
  for instance a field's text and background, or a switch's off track where a platform takes
  one. Each is curated (LLP 1095 §3: a new role is a schema change and a new binary), with a
  default role and a column for every host Exact ships: iOS, macOS, the web, Linux, Windows,
  the terminal, and Android's reader (its Views and the Rust painter, `host/android/README.md`;
  not Compose). A node's own row wins over the role.
- **Fidelity is declared per role and host:** `exact`, `best-effort` (the platform alters
  it), or `none`. `NSSwitch` and AppKit's progress indicator take no tint
  (`ControlsMac.swift:158`, `ProgressMac.swift`): `none`. For each target an app ships, the
  compiler lists every override that is `none` there (`facet-unreachable: accent-color on
  macOS switches: NSSwitch draws the system accent`). A lint, not an error: the platform's
  look is the answer there.
- **A state the platform owns stays the platform's.** iOS draws a disabled prominent glass
  button grey, and no parameter changes that. The Post button's faded blue is the author's
  control: a styled pressable (`PostButton`, D8). The lint says so where a design colours
  prominent buttons.

The control-role table is the schema's, beside `_metrics` (looks as data, LLP 1115.000 D17);
its rows are measured before they land, as LLP 1095's were.

### D10 — Icons: the symbol roles stay; an icon token is the checked form for other names

- **The 76 symbol roles stay** (`kernel/tables/schema.json`, an SF name and an SVG path
  each), and `symbol="back"` draws everywhere, as now.
- **An icon token names a symbol that is not a role**, with a source per platform:

```
icons
  icon.reply = icon(sf "bubble.left", material "chat_bubble_outline", svg "icons/reply.svg")
```

  An SF Symbol on Apple and an SVG under `assets/` everywhere else, Android included: the
  reader paints SVG paths (`host/android/README.md`). A `material` name is an annotation
  kept in the IR for a host that draws Material Symbols one day; nothing resolves it now.
- **Missing sources fail at build.** An icon token with no source for a target the app ships
  (`app.json`) and no `svg` fallback is refused (`facet-icon-missing: icon.share has no
  source for web`). A bare `symbol:sf/…` in an app that ships a non-Apple target becomes a
  lint naming the target and the icon token to write: an error would break existing apps.
- **Rendering stays the existing row**, `-exact-symbol-rendering` (monochrome, hierarchical,
  palette, multicolor), with colour from `-exact-tint-color` and `color`, so colour tokens
  tint icons. An SVG source is drawn as a template (its alpha as a mask) unless the row asks
  for `multicolor`.

### D11 — App and package tokens are first-class; Exact's base is what exists

**One table.** Exact's names, an app's tokens and a package's tokens are rows in the same
reference table, checked by the same compiler, shown by the same editor. exact1 Facet's adoption
failure came from app tokens being second-class.

**Exact's base is what the schema already names**, every value the platform's:
- colours: the roles (LLP 1095 D2), plus any control roles (D9);
- type: the text styles (`-exact-body` …);
- lengths: the named lengths (`-exact-hairline`) and the metrics of `_metrics` that an author
  may name.

No spacing or radius scale ships in the base yet: the platform's metrics are already schema
rows. A base scale, with platform values rather than a brand's 4 px grid, is wanted later
(Charlie, 2026-10-13) and is stage 11.

**Packages.** A package (`packages/reading`) may ship `<package>.facet` with tokens named
under its package (`reading.measure`, `reading.body`), which its views use. The app's design
may set any of them by full name. Two definitions of one name from different files are a
compile error, unless the app's design sets it. Dials and axes are the app's alone: a package
may not declare them, and a package token's `when` lines may name only the app's axes, which
the app must declare or the package's token is refused. What a build keeps of a package's
tokens is D7's one retention rule.

### D12 — Every language gets generated names over the one table

`bun exact.mjs facet gen <target>` writes bindings from the IR, generated and never
committed.

| Target | What it gets | For |
|---|---|---|
| Contract | nothing generated: the compiler checks names against the IR | views |
| TypeScript | a `Token` string-literal union per kind in the types `bun exact.mjs contract types` writes, so a data module returning `"surface.card"` is checked; `facet.css` and `facet.ts` (resolved constants per tuple) for code outside a plan | `app.ts` data modules; web pages outside Exact |
| Rust | `pub const SURFACE_CARD: Token<Color>` and `session.facet().get(token, node)` | data crates returning names; the terminal host; a game's Canvas 2D; Rust native modules |
| Swift | `ExactFacet(view).color(.surfaceCard)` as a dynamic `UIColor`/`NSColor`, `font(.typeBody)`, `symbol(.iconReply)`, and `observe` | native modules (LLP 1024), LLP 1075.003's hatches, the sample host's screens (LLP 1031) |
| Kotlin | `ExactFacet(view).color("surface.card")` as an Android `Color` int, and `observe`; a Compose adapter is optional, outside the host | Android native views |
| DTCG | D14 | other tools |

- **Identity is the name, not the index.** A generated constant holds the token's name and
  kind; a session maps it to its own table's row when the plan loads, and again after an
  update or a dev reload. A name the new plan lacks resolves to its fallback and logs once.
- **Per session.** Each session has its own table and modes, so two embedded sessions with
  different designs or modes answer differently (the colour report table is process-global
  today, `kernel/src/style/roles.rs`; token rows are not).
- **Context.** A handle is bound to a place: a native module's view, a hatch's UIKit object,
  or a session's root for a host screen outside any node. It resolves for that place's app
  tuple, so two views in one session under different `-exact-mode` pins answer differently.
  A Swift colour is a dynamic provider over the view's traits, so scheme and contrast resolve
  as LLP 1095 D5 says, and it reads its place's app tuple each time it resolves; since the
  app tuple is not a trait, the binding also invalidates the view on `setMode`, a pin change
  above it and an editor swap. A font, a symbol and a Kotlin colour int are values for now, so
  `observe` fires on every input they depend on: `setMode`, a pin above the place, scheme,
  contrast, the root font size and Dynamic Type, Bold Text, an update, a dev reload and an
  editor swap.
- **Mapping names.** Dots and hyphens become camelCase in Swift, Kotlin and TypeScript
  properties, and `SCREAMING_SNAKE` in Rust; a design whose names collide after mapping is
  refused.

### D13 — The Facet protocol: one versioned edit API, with the files as its storage

**Ruled (Charlie, 2026-10-13):** a design is changed through an API, not only by hand or
through a development-only editor. The API is public and versioned. External tools (Figma,
Paper, Pencil and the like) connect through it, and Facet's own design tool (D14) is built on
it and is the main way designs are edited. Files are its durable storage.

**The server.** `bun exact.mjs facet serve` owns one project's design: its `.facet` files and
the `params` sections and state parts in its `.contract` files (D15, D8). The dev loop runs
it in its own process (`host/web-js/dev.mjs`, and the native loop's), so it can coordinate
with the file watcher; it also runs alone, with no app running, for a tool that edits tokens
and params without previews. It keeps no state of its own beyond what the files say.

**The protocol, `facet/1`.** JSON messages over a WebSocket, and the same messages as one-shot
HTTP POSTs for scripts.
- **`hello`** negotiates the version. A minor version adds operations or fields; a client
  ignores fields it does not know. A major version the server does not speak is refused. The
  IR's format number (D2) is reported with it.
- **Reads:** `describe` (the IR: kinds, axes, dials and component params with their editor
  metadata, tokens and their `when`s, which dials each token depends on); `resolve` (a
  token's value for a tuple and a scheme); `explain` (a token's chain to its dials);
  `uses` (where a token is used, from the connected development builds, D7's index).
- **`subscribe`** streams every change, whoever made it: another client's operation, a hand
  edit (the server watches the files), a `git checkout`. Each event carries the new digests.
- **Writes** are the operations below, alone or as a **transaction** applied atomically:

| Operation | Edits | Connected development builds |
|---|---|---|
| `set-dial name value` | a dial | the live table (below) for every token it reaches |
| `set-token name [when …] value` | a token's default or a `when` | the live table for that token and all folded from it |
| `set-param component.name value` | a component param's app value (D15) | the live table |
| `set-state style state row value` | a state part (D8), through `contract fmt` | rebuild and reload: state parts compile into rows and state looks |
| `set-token` on a `duration` or `easing`, or `set-dial` on a dial one folds from | the design | rebuild and reload: substituted at build (D3) |
| `add-token`, `rename-token`, `remove-token`, `add-axis-value`, `add-param` | structure | rebuild and reload, which carries state (LLP 1007 §6) |
| `preview axis=value` | nothing | pins an app mode on one build, as `prefer` does |

- **Each write**, or transaction:
  1. carries the digest of every file it edits as the client last saw it; a different digest
     on disk is a conflict and nothing is written (optimistic concurrency, so two clients
     editing at once never overwrite each other silently);
  2. is checked by the evaluator; a refused write changes nothing and returns the
     diagnostics;
  3. is written through `facet fmt` and `contract fmt` (one line per value), so the files
     stay hand-editable, diffable and reviewable, and git is the history;
  4. returns the new digests and the **inverse** transaction, which is how a client undoes.
- **`propose`** runs a transaction without writing: the diff, the diagnostics and the tokens
  it would change. Importers use it so a person accepts what another tool suggests (D14).

**The live table** is what a write does to running development builds. The evaluator re-folds
every token that depends on the change and sends one batch of changed rows, tagged with the
plan's **structure digest** (the plan without its design table), the design digest it
applies to, and the new one. A build applies it only if its design digest is the batch's
base; otherwise, and on every connect, it gets a full snapshot. A different structure digest
gets the ordinary reload. The kernel replaces the rows in one commit and runs D7's refresh;
the JS target rewrites, in one style update, every boundary rule that declares a changed
token. The server skips the watcher's reload for its own write, by digest, in-process, and
rebuilds the plan in the background so the next reload carries the values.

**Authentication.**
- **Local clients** (the design tool, the agent, a plugin's local bridge) use the
  **project-scoped write token**: random, in `.exact/`, mode `0600`, written into a response
  only for a verified-local request (loopback host and loopback peer, the `access.local`
  test the install page uses, `host/web/local-install.mjs`), never into a body a `--lan` or
  `--allow-host` client can load. The dev gate itself authenticates no one
  (`scripts/install-page.mjs:274-294`). Reads of a local server need the token too, since a
  design may be unreleased.
- **Remote and collaborative clients** are open (§9 Q5). The proposal: **pairing**. The
  local design tool shows a short code; the remote client presents it once and receives its
  own token, scoped `read`, `propose` or `write`, listed and revocable in `.exact/`, with
  every write attributed to it in the event stream. Several people editing one design are
  several clients of one server, serialized by step 1's digests. A hosted relay for a team
  that shares no machine is not in r5.
- **Release builds** have no protocol endpoint, no live table and no token. A shipped app's
  design changes by an update (D7).

**What it is, in DEFERRED's terms.** The server and its file writes are tooling, against
§Tooling's "no Design Mode … no devtools UI"; the live table patches a running development
session outside the dev reload, a hot revision of values only, against §Runtime. Both wait on
Charlie's admission (§9 Q4).

**Provenance is a lookup**, through the protocol and the agent alike:
- **Where a token is used:** D7's index lists every mounted node holding it, on every kernel
  host; the JS target's runtime keeps the same map. T3 Code's "Accent color · 4 uses" needs
  no `data-paint` word.
- **Explain this pixel:** `tree` and `explain` report, for a row holding a reference, the
  token, the `when` that won, and its dial chain (`background-color: surface.card ← neutral
  (dial)`).
- **The agent pins app modes** with `prefer mode dark-style=dim`, as it pins scheme and
  contrast (LLP 1095 D7).

### D14 — Clients: Facet's design tool first; Figma, Paper and Pencil are clients too

**Facet's design tool is the primary client**, the "living Figma". It is an Exact app,
connected over `facet/1` like any other client:
- **dials and params**, grouped and ordered by their declared `group` and `priority` (D4,
  D15), each drawn with its declared control;
- **a token × mode grid**, like Figma Variables: resolved swatches per tuple and scheme,
  contrast badges between declared on-pairs, which dials each token follows;
- **a component workbench:** each component's params, its state parts, and previews in every
  mode side by side (`color-scheme` and `-exact-mode`, D5);
- **the app's own screens**, as embedded sessions (as the sample host embeds two, LLP 1031)
  or as connected development builds on a phone and the Mac, all changing together through
  the live table.

**Agents** use the same operations: `bun exact.mjs facet op set-token surface.card …` is a
protocol client with the local token.

**External tools are clients, not peers.** Each connects through `facet/1`, with the same
checks and the same authority: the files.
- **Authority.** Names, kinds, axes and structure change only through structure operations
  that pass the checks; values change only through writes. No tool has a side channel.
- **Importers propose.** A Figma plugin (through a local bridge, since a plugin cannot open a
  socket to an arbitrary host), a Paper or Pencil integration, or a DTCG file turns its
  changes into `propose` transactions; a person or agent accepts them. A change to a token
  folded from a dial is offered as a dial change where one dial explains it, or as a pinned
  override the tool marks ("no longer follows `accent`"), so a dial never goes inert unseen.
  exact1's Facet never made this choice, and `type.ratio` died of it.
- **DTCG** is one client's format: `facet export dtcg` writes DTCG Format 2025.10 with the
  Resolver Module 2025.10, one modifier `mode` whose contexts are the full tuples (every
  app-axis tuple × `light` and `dark`), each a complete set, checked against the folded table
  before writing. Conversions are named in the export's header: a value with its `$type`; a
  role as its fallback for the context with `$extensions["exact"].role`; `rem`, `em` and text
  styles as px at the default root size, marked lossy, with the source in `$extensions`; an
  expression as its folded value with its source; dials in root `$extensions`; icons outside
  the token set. Re-import compares each incoming `$value` with the exported baseline (kept in
  `.exact/`): unchanged values keep their source, changed ones are proposed. Tokens Studio's
  dialect is a separate exporter.
- **No live two-way canvas sync** with Figma: its canvas cannot be written headlessly and its
  scene graph carries no semantics (exact1 LLP 0389, 0395). Figma's Variables can be kept in
  step through the API (an export after each accepted write, an import as proposals); its
  canvas stays Figma's.

### D15 — Component params: a component declares what the tool may tweak, and how

**Ruled (Charlie, 2026-10-13):** a Contract component declares what is tweakable and how, as
part of its definition: typed params with ranges, editor controls and a priority or group, so
the design tool knows what to surface first. This is exact1 Facet's `params` and `design`
blocks (button.contract), made one section, without its TypeScript recipe tables.

**A `params` section in the component:**

```contract
component PostButton
  props
    label: string
    enabled: bool
  params
    fill: color = "accent.fill"  control=color  group="Colour"  priority=1
    radius: length = "radius.full"  in 0..40  step=1  control=slider  group="Shape"  priority=1  label="Corner radius"
    pressed: color = "color-mix(in oklab, param.fill 88%, text.primary)"  control=none
    padding: length = 12px  in 8..32  step=2  control=slider  group="Shape"  priority=2
  view
    button press=post disabled=(!enabled) background-color="param.fill" border-radius="param.radius" padding="param.padding"
      :active
        background-color = "param.pressed"
      text label
```

- **A param is a token owned by its component**, named `PostButton.radius` (a package's,
  `reading.Article.measure`). Its default is any token value: a literal, another token, or an
  expression over tokens, dials and the component's other params. Inside the component a row
  names it as `"param.radius"`, which the compiler rewrites to the full name. It is in the
  reference table like any token, so it follows modes and the live table.
- **The app's value.** The app's `.facet` file may set it by full name, with `when` lines
  like any token (`PostButton.radius = 12px`); `set-param` writes there. The component's file
  keeps its default. One value per design: every instance of the component shares it.
- **Params are not props.** A prop is per instance and runtime data; a param is the design's,
  the same for every instance, and never read by the runner. A per-instance variant is a prop
  that chooses between params (`background-color=(primary ? "param.fill" : "param.quiet")`).
- **Editor metadata**, all optional: `in a..b` and `step=` (for numbers and lengths, the
  range is enforced, as a dial's is); `control=` (`slider`, `color`, `token` (a picker of
  tokens of the kind), `toggle`, `select` for a param whose value is one of listed tokens, or
  `none` for a derived param the tool does not surface); `label=`; `help=`; `group=` (groups
  in the order they first appear); `priority=` 1, 2 or 3 (1 is surfaced first, 3 behind
  "more"; the default is 2). App dials (D4) take the same metadata.
- **Checks.** A param no row in its component reads is refused (`facet-param-unused`, D4's
  completeness rule for params). A param's kind is checked against each row that uses it
  (D3). A default outside its range is refused.
- **Language-neutral.** The `params` section lowers into the IR's `components` block. A
  component that is not Contract (a native module's view in Swift or Kotlin, a Rust canvas
  component) declares the same block in a `.facet` file:

```
component MapCard
  params
    pin: color = "accent.fill"  control=color  priority=1
```

  and reads its params through the bindings (D12): `ExactFacet(view).color(.mapCardPin)`.
- **Grammar.** `section = … | "params" block(param)`; `param = FIELD ":" kind "=" value
  [ "in" value ".." value ] { IDENT "=" literal } NL`. One `params` section per component.

## 3. The `.facet` grammar

```ebnf
facet-file  = "facet" IDENT NL { block } ;
block       = "axes" NL block(axis) | "dials" NL block(dial)
                        | "theme" NL block(token) | "icons" NL block(token)
            | "exports" NL block(NAME NL)
            | "component" IDENT NL INDENT "params" NL block(param) DEDENT ;
param       = IDENT ":" kind "=" value [ "in" value ".." value ] { meta } NL ;
meta        = IDENT "=" ( STRING | NUMBER | IDENT ) ;
axis        = IDENT "=" IDENT { "|" IDENT } [ "under" "scheme" "=" "dark" ] NL ;
dial        = IDENT ":" ( "color" | "length" | "number" ) "=" value [ "in" value ".." value ] { meta } NL ;
token       = NAME [ ":" kind ] "=" value NL [ INDENT { when } DEDENT ] ;
kind        = "color" | "length" | "number" | "type" | "family" | "box-shadow"
            | "text-shadow" | "duration" | "easing" | "icon" ;
when        = "when" cond { cond } ":" value NL ;
cond        = IDENT "=" IDENT ;
NAME        = IDENT { "." IDENT } ;
value       = CSS value syntax, with dial and token names where CSS writes var() ;
```

**Values.** CSS's syntax for colours, lengths, times, `cubic-bezier()`, `font` and the
shadows, with `calc()`, `min()`, `max()`, `clamp()` and `round()` over numbers and lengths,
CSS Color 4/5's colour functions, `light-dark()`, and `icon()`. A token or dial name stands
where CSS would write `var(--name)`. So the file is not CSS a browser can read: the build
rewrites each value, folding it or emitting it as `var(--x-…)` (D7). A kind is inferred from
the value where it can be and written where it cannot.

**The lexer** is Contract's (`contract/syntax/src/lexer.rs`) for the structure (indentation,
identifiers, `=`, `:`, comments), extended with `|` and `..`. After a token's or a `when`'s
`=` or `:`, the rest of the line is **CSS value text**, read by the crate's CSS value
parser, so `-exact-body/1.35` is a text style and a line height, not a negation and a
division as Contract's expression lexer would read it. On a dial or param line, the value
ends, outside any parentheses, at the word `in` that starts a range or at the first
`name=` of the editor metadata; so `color-mix(in oklab, …)` keeps its `in`. In a Contract
`params` section (D15) the value is a string, as a row's token name is (`"accent.fill"`),
and the metadata are ordinary attributes.

**Folding** is in f64. Only a legacy result (D7) is quantised, once, to 8-bit sRGB; a `Wide`
result keeps LLP 1100's storage. Every host gets the same bits: there is one evaluator.

**Example: the Bluesky clone, in part** (values illustrative; the port takes the clone's
own):

```
facet bluesky

axes
  dark-style = black | dim  under scheme=dark

dials
  accent: color = #1083fe

theme
  accent.fill = light-dark(accent, #208bfe)  // the app root writes accent-color="accent.fill"
  surface.page = light-dark(#ffffff, #000000)
    when dark-style=dim: light-dark(#ffffff, #161e27)
  surface.raised = light-dark(#f1f3f5, #0f1419)
    when dark-style=dim: light-dark(#f1f3f5, #1e2936)
  text.primary = light-dark(#0b0f14, #f1f3f5)
  border.hairline = light-dark(#d4dbe2, #2e4052)
  accent.fill-pressed = light-dark(color-mix(in oklab, accent 88%, #0b0f14), color-mix(in oklab, #208bfe 88%, #f1f3f5))
  accent.fill-faded = light-dark(color-mix(in srgb, accent 50%, #ffffff), color-mix(in srgb, #208bfe 50%, #000000))
  accent.on-fill = #ffffff
  type.body = -exact-body/1.35 "InterVariable"
  radius.full = 999px

icons
  icon.reply = icon(sf "bubble.left", svg "icons/reply.svg")
```

## 4. What this deliberately does not do

- **No runtime theme library, no JS above the data seam, no package import of theme code.**
- **No evaluation on a device.** The build folds; the existing resolution paths read rows.
- **No new colour value form.** Tokens lower to the forms the kernel has.
- **No selectors and no cascade.** A state part is the node's own state; a mode pin works as
  `color-scheme` does.
- **No look-alike controls** (D9).
- **No third scheme.** Dim is an app axis under dark.
- **No dial or param changes at run time in a shipped app.** A person's choice is an app axis.
- **No side channel for any tool.** Every client, ours included, edits through `facet/1`.
- **No live two-way Figma canvas sync** (D14).

## 5. Staging and gates

Each stage needs its implementer and its predecessor's gate. Each names its schema-digest
break (every native host binary moves on one, LLP 1030 D3a) and the hosts that must read
what it adds: the web's JS target, the wasm host, iOS, macOS, tvOS, Linux, Windows,
Android's reader and the terminal.

1. **LLP 1095 D4 and its reach** (its branch). Gate: LLP 1095 §5 item 6's measurement and
   Charlie's word.
2. **Colour tokens and app axes.** The crate; `app.facet` or inline `theme`; dials; app axes
   with `setMode`, `-exact-mode` and `prefer mode`; colour tokens on the reach's table, keyed
   by app tuple; the web's mode boundaries; the retention rule. Digest break: the app tuple
   on reference rows. Hosts: all. **Gate:** the Bluesky clone's 244 literals become names,
   with Light, Dim and Black from the root and no palette threaded; every compared screen
   identical on the web, iOS and the Mac in all three; a settings screen with three previews
   (Light by `color-scheme`, Dim and Black by `color-scheme="dark"` and a pin) renders each
   correctly on the web, the wasm host and iOS, including a Black pin inside a Dim page with
   text whose colour is inherited from the page, and a token naming the accent under a child
   with its own `accent-color`; a Dim switch on a 1,000-row list does not relayout (`perf`);
   Expose and the readers unchanged; an app without a design unchanged in behaviour, with no
   design payload. **Alongside, independent:** `setRootFontSize("125%")` (QUEUE's item).
   Gate: the clone's font-size setting scales Dynamic Type at two Dynamic Type sizes.
3. **The Facet protocol and the design tool, over files** (D13 without the live table, D14's
   own tool). Needs Charlie's §Tooling admission. No digest break. **Gate:** the design tool
   edits the clone's dials and tokens through `facet/1`; every write is a one-line diff;
   two clients writing the same token conflict instead of overwriting; a hand edit and a
   `git checkout` reach a subscribed client; `propose` changes nothing on disk; a write or
   read without the token is refused, and a `--lan` client cannot obtain it; the inverse
   transaction undoes.
4. **The live table** (D13). Needs Charlie's §Runtime admission, and reclaimable live colour
   storage first: `Wide` entries are process-global, immutable and capped at 4,096
   (`kernel/src/style/wide.rs`); the policy says what a running transition keeps alive. No
   digest break beyond stage 2. **Gate:** a dial drag in the design tool reaches the clone on
   the iOS simulator, the Mac and the web at once, with no state lost (a scrolled list stays
   scrolled, a draft stays typed) and no reload; a build that reconnects after missing a batch
   gets the snapshot; more than 4,096 distinct colour edits drop none and leave storage
   bounded; `uses` and `explain` are right on every host.
5. **Bindings and the DTCG client** (D12, D14). No digest break. **Gate:** a native module in
   the sample host draws with `ExactFacet(view).color(…)` and follows `setMode`, a pin above
   it, a Dynamic Type change and the live table; two embedded sessions with different
   designs answer differently; a TypeScript data module returning an unknown token fails
   `tsc`; a DTCG export re-imported unedited proposes no operations.
6. **Length, number, type, shadow and motion tokens.** Prerequisite: LLP 1115.000 D12.2 (D6).
   Digest break: the new reference forms on those rows. Hosts: all. **Gate:** Expose's 48
   `font-family` writes and its radii become tokens with screens identical; under Bold Text a
   system-font type token is bolded, one with an authored weight too, and a bundled family is
   not. `calc()` with `rem` in a token waits on QUEUE's `rem`/`em` item.
7. **Component params** (D15). Colour params may land with stage 2's kinds, the rest with
   stage 6's. No digest break beyond those stages (a param is a token). **Gate:** the clone's
   post action row and Post button declare params; the design tool lists them by group and
   priority with their controls; a `set-param` reaches every instance live; a Swift native
   view's `.facet` params appear beside them; an unused param is refused.
8. **State parts and control roles** (D8, D9). Digest break: state looks; the control-role
   table. Hosts: all that paint a press. **Gate:** the clone's Post button and switches are
   right on every host with no hand fix and no double dim; a checked, disabled toggle with
   both parts shows `:disabled`'s; a class change and a base change while pressed keep the
   overlay over the new base; existing `disabledStyled` apps unchanged; LLP 1061's press
   corpus unchanged; `facet-unreachable` names the macOS switch and iOS's disabled glass
   button.
9. **Icon tokens** (D10). Digest break: none if the symbol wire already takes an SVG source
   per node, else one. **Gate:** the clone's ~100 `symbol:sf/…` uses draw on the web from
   icon tokens; a missing source fails the build with the target named.
10. **External clients and remote authentication** (D13, D14). A Figma plugin bridge, a Paper
    or Pencil client, pairing. Waits on §9 Q5. **Gate:** a changed accent in Figma Variables
    is proposed as a dial change and a changed folded value as a pinned override; a paired
    client with `propose` scope cannot write; revoking a client cuts it off at once.
11. **A base spacing and radius scale** (D11), when packages need a shared vocabulary:
    `space.*` and `radius.*` with values taken from each platform's metrics, not a brand's
    4 px grid. **Gate:** two packages restyle together from one app value.
12. **A component library**: `.contract` packages over Facet, with params (D15) as their
    tweakable surface and anatomy as checked data, keeping exact1 Facet's rule, "a recipe
    styles parts; it never forecloses slots". Its own RFC. **Gate:** set by that RFC; its
    first consumer is named before it starts.

**Checks that stay from stage 2:** `facet-dial-inert` and `facet-param-unused`; golden IR and
folded tables for the corpus designs (one evaluator, so no cross-language parity corpus); a
plan-size budget (the clone's design adds under 4 KB); first frame unchanged within noise for
an app with a design.

## 6. Cost

- One crate; the compiler's design pass and `params` section; the app tuple on reference
  rows and the kernel's slice selection and token index; the web's mode-boundary generator
  (one generator with LLP 1095 D11's role sheet); the hosts' existing refresh paths called
  for row changes; the protocol server and the design tool; later, the stages' own pieces.
- Docs: a section in `docs/contract-for-agents.md`, a recipe in `docs/recipes/`, and the
  protocol's reference (`docs/facet-protocol.md`), versioned with it. The required read grows
  by one line: "a brand is an `app.facet`; a colour or size names a role or a token".
- It removes per-app `Theme` records, threaded palettes, repeated `light-dark` pairs, the
  clone's hand-fixed switches, and the `data-paint` workaround.

## 7. Risks

- **A second way to write a colour.** The no-tells lint is taught that a token name is not a
  literal, and the docs give one rule: a role if the platform has one, a token if it is the
  brand.
- **The reach is unfinished.** D1's claims about overrides inherit stage 1's measurement.
- **A public protocol is a compatibility promise.** `facet/1` is versioned from its first
  release, minor versions only add, and the IR's format number is reported, so a client
  knows what it is talking to; a major version is an RFC.
- **The server writes source on a developer's disk from the network.** The token, verified
  local delivery, digest preconditions and atomic writes (D13) are reviewed as a new writer
  in stage 3; remote clients wait on pairing (stage 10).
- **Host press paths** (stage 8) are tuned and measured; they change one host at a time, with
  the press corpus as the gate.
- **The web's tuple rules** grow with app axes; the 64-tuple cap and retention bound them.

## 8. Not in this RFC

- The component library's design (stage 12 is its own RFC), Code Connect.
- A generative palette (`ramp()`), responsive tokens by size class, and an authored contrast
  variant (D5): each waits for an app that needs it.
- A hosted relay for teams that share no machine (D13).
- exact1 LLP 0269's other "design intent" layers (structure, behaviour, content).

## 9. Open questions for Charlie

**Ruled on r4 (2026-10-13):** an API is the way a design changes, public and versioned, with
Facet's own design tool its main client and the files its storage (D13, D14); the name is
Facet, with `.facet` files (alternatives below); a component library and a base scale are
wanted later (stages 11 and 12); components declare their params in Contract (D15); a DSL in
files is right.

1. **The name.** r5 uses **Facet** (`app.facet`, `exact-facet`, `bun exact.mjs facet`,
   `facet/1`, `ExactFacet`). It reuses exact1's name for the same idea, which helps anyone
   who knew it, and "a facet of a design" reads well. Alternatives, if a fresh name is
   wanted:
   - **Livery**: the colours and marks a brand puts on everything it owns; distinctive, and
     `.livery` is unclaimed, but a less familiar word.
   - **Swatch**: what a designer holds up to compare; friendly and concrete, but it says
     colour and this is more than colour.
   - **Mien**: a thing's look and bearing; short and exact, but obscure.
   - **Guise**: the form in which something appears; apt for modes and variants, though it
     hints at disguise.
   - **Atelier**: the studio; better as the name of the design tool (D14) than of the system.
2. **How generative?** D4 keeps dials but expects a brand to be hand-tuned literals with a
   few dials, and defers `ramp()`. More (a brand from three dials) or less?
3. **References.** Token references as bare names (as LLP 1095 D4 built) or CSS's
   `var(--name)` with custom-property inheritance (subtree overrides for free, but a
   cascade)?
4. **DEFERRED admissions.** The protocol server and design tool (stage 3, against §Tooling)
   and the live table (stage 4, a development-only hot revision of values, against
   §Runtime). Admit both? What take?
5. **Remote and collaborative clients.** D13 proposes pairing with scoped, revocable,
   attributed tokens, and leaves a hosted relay out. Is that the model, and does a team
   relay belong in this program?
6. **Persistence and contrast.** Should `setMode` and the root-size percentage persist and
   apply before the first frame (so Dim never flashes Black)? Is an authored contrast
   variant for app colours wanted (D5)?
7. **State parts** amend LLP 1061 D1. Acceptable as parts with paint rows only, or named rows
   (`-exact-pressed-background-color` and the like) and no block?
8. **Figma.** Is a Variables writer wanted beside the plugin bridge, and is Enterprise access
   (which Variables writes need) available?
