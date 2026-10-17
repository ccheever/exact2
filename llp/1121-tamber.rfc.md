# LLP 1121: Tamber — a design system in any language, edited through one API

**Type:** RFC
**Status:** **Accepted; stages 1–3 landed** (Charlie, 2026-10-13: "Can we start building tamber"; ".tamber is probably fine"). §9's open questions stand, each with the RFC's recommendation as the provisional default until he rules. §10 records what was built where it differs. Stage 1 landed 2026-10-14 (`7a95dcce4`..`13f49af39`). Stage 2 closed 2026-10-14 with its gate met on the Bluesky clone (§10). The `scheme=fixed` follow-up landed (§10). Stage 3 (the protocol over files, `tamber fmt`, the importers) closed 2026-10-15 with its gate met (§10). LLP 1122's T1 and T1a can start; Tamber stage 4 (the live table) is next. Draft history: r7 (2026-10-13) is r6 with the system named **Tamber (provisional)** (Charlie, 2026-10-13; earlier Saturi, and Facet in r5–r6); nothing else changed. r1–r4 went through three blind review rounds by GPT-6 Astra (max) and Grok 4.7 (xhigh); every verdict after round 1 was RIGHT WITH CHANGES. r5 wrote in Charlie's rulings on r4's questions (2026-10-13): the system was named Facet, which r7 replaces; a design changes through a public, versioned edit API whose main client is the design tool (D13, D14); components declare their tweakable params in Contract (D15); a component library and a base scale are later stages. Round 4 reviewed r5 (both RIGHT WITH CHANGES; Grok found one BLOCKER, how the live table reaches a running build), and r6 takes its findings without another round (§0). Reviews: `llp/reviews/rfc-2026-10-13-design-system-r{1,2,3,4}.{astra,grok}.md`; they cite this file by its earlier name, `llp/1121-design-system.rfc.md` (it was then `llp/1121-facet.rfc.md`, then `llp/1121-saturi.rfc.md`). Nothing built. Builds on LLP 1095 D4 (`theme`) and its reach (Charlie, 2026-10-12; LLP 1095 §5 item 6), stage 1 here, not redesigned. Stages 3 and 4 need DEFERRED admissions (§9 Q3).
**Systems:**
- A new leaf crate, `tamber/` (`exact-tamber`): the `.tamber` grammar, its checker and the build's evaluator.
- Contract: token names in rows, the `params` section (D15), state parts (D8), `setMode` and `-exact-mode` (D5).
- The Tamber protocol server, `tamber/1` (D13), and the design tool, an Exact app (D14).
- The plan and the kernel: the reference table the reach is adding, keyed by app modes (D7).
- The hosts: per-mode selection, their existing colour-refresh paths extended to tokens, control roles (D9), the live table (D13).
- The web's JS target and the wasm host: CSS custom properties per mode boundary (D7).
- Generators for TypeScript, Rust, Swift, Kotlin and DTCG (D12, D14).
- The agent API: `prefer` pins an app mode; `tree` names the token a row came from (D13).

**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-13
**Implementer:** x2-perf3 (Claude, Opus 5.5), stages 1 and 2, from 2026-10-13; the RFC's owner (theme1, Claude, Opus 5.5) reviews each stage's commits against it and amends it as built. Stages 3–12 are accepted in direction but not scheduled until each has an implementer and a date (`rules/RULES.md` §Scope).
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

**Tamber** is Exact's design system. An app names its look once, in a `.tamber` file that
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

**A design changes through one API.** `tamber/1` is a public, versioned edit protocol: reads,
subscriptions, and checked, atomic writes whose durable storage is the files. Tamber's own
design tool, the "living Figma", is its main client; agents, Figma, Paper, Pencil and other
tools are clients of the same API, with no side channel (D13, D14). A write reaches every
running development build at once through the live table. **Components declare what may be
tweaked**: a `params` section in a Contract component gives each param a type, a range, an
editor control, a group and a priority, so the tool knows what to surface first (D15).

**The default design is the platform** (LLP 1115). A design names only what a brand
changes; a design's tokens are the author's values, written once. Nothing here replaces a
platform control with a look-alike (the reach ruling).

**No language owns the truth.** The `.tamber` file is the source; a JSON IR is generated
from it; Contract checks names against the IR; TypeScript, Rust, Swift and Kotlin get
generated names and accessors over the same live table; other tools reach it through the
protocol, or as DTCG.
Only the build evaluates, in Rust: exact1 wrote its resolver in TypeScript and then ported
it to Rust by hand.

## 0. Review rounds

**Round 4** (r5 → r6, focused on r5's changes; written in without another round). Astra:
RIGHT WITH CHANGES, no BLOCKER (7 MAJOR, 2 MINOR). Grok: RIGHT WITH CHANGES (1 BLOCKER, 6
MAJOR, 2 MINOR). Both confirmed round 3's blocker fixed: a boundary's implied row stops the
kernel's ancestor walk and gives the wasm emitter a row. Each finding is taken:
- **The live table never reached a running build** (Grok B1). Publishing a plan is what
  restarts the page and the Apple hosts, and a mark inside one process did not stop the
  others. Now a value-only change publishes no plan, decided by content (the structure
  digest) in both loops' watchers and `Session`; the batch travels on the dev event stream
  the builds already hold, and a new kernel entry applies it in one commit (D13).
- **Colour ids across processes** (Grok M2): the batch carries CSS text; each process
  interns; `Wide` entries are reference-counted, and a full table refuses with a diagnostic
  (D13).
- **The token** (Grok M3): the install predicate whole, `!forwarded` included, and the same
  not-found; held in memory, never in `.exact/`; presented as the first frame or a header.
  The design tool builds for the web's JS target, since only a TypeScript data module there
  can send frames; native and wasm sessions do not speak the protocol. Pairing has a stated
  code strength and attempt limit (D13, D14).
- **Transactions** (Astra M1, Grok m9): one writer per project under a lock, serialized
  checks, a journaled set of renames recovered on restart, publication after the whole
  commit; inverses carry the new digests (D13).
- **Inverses** (Astra M2): `unset-*` and `remove-*` operations, so undoing a first
  `set-param` restores the component's default (D13).
- **`resolve`** (Astra M3): the stored value or residual with labelled fallbacks; concrete
  values from `resolve-on`, naming a build and a node (D13).
- **Live or rebuild** (Astra M6): judged by the dependency closure, so a param a motion token
  folds from rebuilds (D13, D15).
- **Params** (Grok M4, Astra M4/M5/M7): one Contract form (string values, `range="0..40"`
  and other attributes; no new lexer token); stable identities (`post.contract#PostButton`),
  ambiguous short names refused, aliases resolved to the declaration; controls by kind,
  `select` with `options`, no `toggle`; ranged lengths in px only, defaults checked after
  folding, a scrub over a token default a pinned override; reachability from rows, state
  parts and native declarations; the IR block's fields; `length` and `number` bindings; props
  are not params (D15, D12).
- **A Light preview** (Grok M5): a `color-scheme` node is a boundary too (D7).
- **Staging** (Grok M6, Astra m9): `tamber-param-unused` from stage 7; the tool's panels gated
  in the stages that add them; embedded sessions are later; D6's prerequisite points at the
  type-token stage (§5).
- **`prefer mode`** (Grok M7): a fourth `prefer` group (D13).
- **Minors:** the re-specified rows are the schema's inherited colour rows, not
  `-exact-tint-color` (Grok m8); `preview` names a build (Grok m9); a Figma plugin may open
  `ws://localhost`, so a bridge is optional (Astra m8).

**Charlie's naming** (r6 → r7, 2026-10-13): the system (tokens, dials, modes, params, the
evaluator and the protocol) is **Tamber (provisional)**: `.tamber` files (`app.tamber`),
`tamber/1`, the `tamber/` crate (`exact-tamber`), `bun exact.mjs tamber`, `ExactTamber`,
`tamber-*` diagnostics, the `"tamber"` key in `app.json`, `docs/tamber-protocol.md`. It was
Saturi, with `.sat` files, before that, and Facet in r5 and r6. The design tool on top gets
its own name later and is called "the design tool" here. Earlier entries below are written
in the current names, except where they record an old one.

**Charlie's rulings on r4** (r4 → r5, 2026-10-13):
- **An API is how a design changes.** D13 is now the Tamber protocol, `tamber/1`: public,
  versioned, reads, subscriptions, atomic checked writes, `propose`, inverse transactions,
  with the files as its storage. The live table is what a write does to running development
  builds. D14 makes the design tool the primary client and Figma, Paper and Pencil
  clients like any other; DTCG is one client's format. The local token stays; remote and
  collaborative clients get a pairing proposal, left open (§9 Q4).
- **The name** was Facet, with `.facet` files (`exact-facet`, `bun exact.mjs facet`,
  diagnostics `facet-*`) in r5; r7 renames it Tamber (above). exact1's system is called
  "exact1's Facet" throughout.
- **A component library and a base scale** are wanted later: stages 11 and 12, no longer
  non-goals.
- **Component params** (D15): a `params` section in the Contract component, with types,
  ranges, controls, labels, groups and priorities; the same block in a `.tamber` file for a
  component that is not Contract. App dials take the same metadata (D4).
- **The DSL in files** stands; the extension is `.tamber`.

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
  (`tamber-inherited-axis`) (D7).
- **Role variables resolve where a custom property is declared** (Astra M1): only the accent
  is element-contextual on the web, so an element with an authored `accent-color`
  redeclares the token properties whose values name it (D7).
- **Lookup keys** (Grok M3): every full tuple has an entry, by exact match (D5, D7).
- **Folding** (Astra M2, Grok M10): `Fixed` only for what CSS serializes as legacy `rgb()`;
  a `light-dark()` with a modern half is one `Wide` with its dark side (D7, §3).
- **Typography** (Astra M3, Grok M2): LLP 1115.000 D12.2 (unsaid weight and leading resolved
    per platform) is a prerequisite of the stage that adds type tokens, and a type token sets only the longhands it writes
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
  expanded before the state order; `tamber-motion-axis`; the implementer's start (header).

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

### D2 — One source, the `.tamber` file; a generated IR; one build evaluator, in Rust

**The source is `app.tamber`** beside `app.contract` (or named in `app.json` as `"tamber"`).
Its grammar is Contract's lexical grammar, extended (§3): indentation blocks, `name =
value` lines, `//` comments, plus `|` and `..`, with each value read as CSS value text. Its values are written in CSS's syntax, with token names where CSS would write
`var()` (§3). LLP 1095 D4's `theme` block in a `.contract` file stays legal and stays what D4
says: colour entries only, with no `when` lines and no other kinds. An app that wants more
moves it to `app.tamber`, whose `theme` block is a superset. An app has one or the other
(`tamber-two-sources`).

**`exact-tamber` is a leaf crate.** It parses, checks (names, kinds against each using row,
ranges, cycles, the completeness rule of D4) and folds. It is the **build's** evaluator,
used by:
- the Contract compiler, which writes the folded table into the plan (D7);
- `bun exact.mjs tamber check | fmt | eval --mode dark-style=dim | gen <target> | export dtcg`;
- the dev server, for the editor (D13). The editor's own web page loads it as wasm; a
  shipped build never carries it.

**Run-time resolution stays where it is.** The kernel resolves `rem`, `em`, text styles and
the hairline at the end of each commit (`kernel/src/style/relative.rs`); the hosts resolve
roles, pairs, platform colours and mixes per view (LLP 1095 D5); the web's JS target leaves
both to the browser. A design adds rows these paths read; it adds nothing for them to
evaluate.

**The IR is generated, never committed** (DEFERRED §Process): `tamber.json`, with a format
number and the design's digest. Every generator and the editor read it; none parses the DSL.

**`tamber fmt` owns the file's layout**, so an editor edit is a one-line diff in a file a
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
an inline `theme` and in `app.tamber`. Any other name equal to a CSS
keyword or named colour is refused, as D4 refuses it. LLP 1095 D4's
undotted app names (`brand`, `card`, `defrost-on`) stay legal; dots are a convention for
grouping, which the editor and DTCG use.

**Kinds**, each checked against every row that uses the token (`tamber-kind` names the row
and why):

| Kind | Value | Rows | Lowers to |
|---|---|---|---|
| `color` | any CSS colour, `light-dark()`, a role, `-exact-platform-color()`, a mix (§3) | every colour row and embedded colour LLP 1095 D1 lists, gradient and mask stops included | an existing `ColorValue` form; a reference row once the reach lands |
| `length` | px; one `rem`, `em`, text style or `-exact-hairline`; `calc()`/`min()`/`max()`/`clamp()` folded to px | each length row whose codec takes the form (the kernel's relative side table today; `calc()` with `rem` waits on its queue item) | a dimension, or the relative side table's entry |
| `number` | a number | `opacity`, `line-height`, `flex-grow`, `font-weight`, `-exact-press-scale` | the row's number |
| `type` | CSS `font` syntax: style, weight, size (px, `rem`, or a text style), `/line-height`, family | `font` | the four longhands `font` sets today (`contract/lower/src/shorthands.rs`) plus `font-family` |
| `family` | a font-family stack | `font-family` | the row |
| `box-shadow`, `text-shadow` | each row's own syntax (`text-shadow` has no spread) | that row | the row's codec |
| `duration`, `easing` | CSS time; `cubic-bezier()`, a keyword | a component of a `transition` or `animation` value (`transition="background-color duration.fast easing.standard"`) | substituted into the value at build, before the row's codec reads it; so a `when` on one is refused (`tamber-motion-axis`), and an edit to one rebuilds (D13) |
| `icon` | `icon(…)` (D10) | `symbol` | a symbol source |

A gradient is colour tokens in its stops; there is no gradient kind. The hairline stays the
named length `-exact-hairline`, usable as a length token's value.

**A token may be marked `scheme=fixed`** (token-line metadata, as a dial takes):
`white_100 = #ffffff  scheme=fixed` says the token is deliberately one colour in every scheme,
so `lower-colour-one-scheme` doesn't report it. The mark is refused where light and dark
differ (`tamber-scheme-fixed`). *Added 2026-10-14 from the Bluesky port; §10.*

**A token may declare what it is drawn on** with `pair=<token>` (token-line metadata; several,
comma-separated: `pair=surface.page,surface.raised`). Each named token must be a colour token
or a role, and not the token itself (`tamber-pair`).
- **What a pair is for.** It says the two are meant to be read together, a foreground on a
  background. So the design tool's grid shows a contrast badge per tuple and scheme between
  them (LLP 1122 D7).
- **The check.** `contract tamber check` warns when, in any tuple and scheme, the pair's
  folded colours fall below 4.5:1 (WCAG 2's ratio for text), naming the token, the tuple and
  the ratio (`tamber-contrast`). A residual side (a role, a platform colour) is checked at its
  fallback and the warning says so. The warning is the author's information, never a build
  failure.
- **Importers may propose pairs** where the source states them: shadcn's `x-foreground` is
  `pair=x`, and Astryx's `on-` variants likewise.

*Added 2026-10-15 (lead-approved), for the design tool's contrast badges; to be built by Tamber's
implementer.*

**A row names a token as a string**, as LLP 1095 D4 does: `background-color="surface.card"`,
`padding="space.row"`, `font="type.body"`. Inside an expression in a row, a token is a bare
name: `color="color-mix(in srgb, brand 60%, transparent)"`. The compiler checks every literal
name (`tamber-unknown-token`). A row expression that names a token is lowered to a
synthetic token holding the whole expression, folded per tuple at build like any other, so
the row still holds one reference and no device evaluates the mix. A name computed at run
time (from data) is resolved by the
runtime parser against the plan's table, as D4 resolves `"brand"`; an unknown name is an
invalid value in the log.

*Considered:* CSS's own `var(--surface-card)`. Rejected for r2: it brings custom-property
inheritance, a cascade by another name, which Contract has refused; D4's bare names are
built and ported. §9 Q2 asks.

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
- **Completeness.** Every dial moves at least one token, or `tamber check` refuses it
  (`tamber-dial-inert`). The IR lists, per token, the dials it depends on.
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
  (`tamber-when-ambiguous`). An axis declared `under scheme=dark` is resolved by the host
  side: its values are `light-dark(<the default's light side>, <the axis value's dark
  side>)`, so the light side never varies by it. A `when` on it may write one colour, its dark
  side, or a `light-dark()` whose light side is the default's; another light side is refused
  (`tamber-under-light`). A role or `platform-color` has no light side to keep, so as such a
  `when`'s value, or as the default of a token that has one, it is refused unless written
  inside a `light-dark()` (`tamber-under-role`; as built, that `light-dark()` is itself refused until LLP 1095 D10, §10). Only colours may vary by such an
  axis: the kernel picks a length's slice without knowing the scheme, so a non-colour `when`
  on it is refused (`tamber-under-kind`). A design whose axes multiply past 64 tuples is
  refused with the axes named.
- **`setMode("dark-style", "dim")`** sets the window's app slice: a runner command beside
  `setScheme`, which stays the host's appearance command. The new slice applies in the
  action's own commit, refreshed by D7's per-use domains: repainted, and laid out only where
  a selected value is in a layout row. Undeclared axes and values are refused at compile.
  A dev reload keeps the window's slice (runner `Carried::modes`, the JS target's checkpoint);
  an axis or value the edit removed starts again at its first value.
- **A subtree pins app modes** with `-exact-mode="dark-style=dim"` (several, space- or comma-separated; `axis=value` is the one spelling, as in a `when`),
  as `color-scheme` pins the scheme (LLP 1034 §8). A pin inherits the axes it does not name
  from above. A mode pin never changes the scheme: a settings screen previews the three
  appearances side by side as `color-scheme="light"`, and `color-scheme="dark"` with
  `-exact-mode="dark-style=dim"` or `"dark-style=black"`. The editor shows one screen in
  every mode the same way.
- **Bluesky:** its three-way appearance is `setScheme` for light, dark or system, and
  `setMode("dark-style", …)` for Dim or Black. No palette is threaded; the 244 literals
  become names.
- **An authored contrast variant** for an app colour (`when contrast=more`) is not in r2. A
  host would need a value form carrying four colours; LLP 1095 D11's high-contrast columns
  are the place to design it. §9 Q5.

### D6 — Text scale is the host's; type is relative to it

- **Type tokens take the platform's text styles and `rem`.** `type.body = -exact-body/1.35
  "InterVariable"` takes the body style's size and weight, with `line-height: 1.35` and
  `font-family: "InterVariable"`; it follows Dynamic Type and the browser's setting (LLP 1115 D3). A px
  size is the author's choice not to scale, and `tamber check` warns (`tamber-type-fixed`).
- **A type token sets only the longhands it writes.** `font="type.body"` writes
  `font-size: -exact-body`, `line-height: 1.35` and `font-family: "InterVariable"`, each a
  written row; the weight it omits stays unsaid, so the text style's own weight applies.
  Today's `font` shorthand cannot do that: it writes a numeric weight and `line-height:
  normal` and marks all four written (`contract/lower/src/shorthands.rs`). LLP 1115.000
  D12.2, an unsaid weight and leading resolved per platform with per-longhand provenance, is
    therefore stage 6's prerequisite. When a token's value changes which longhands it writes
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
  question (§9 Q5).

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
  and only once D10 is built; any other is refused (`tamber-residual`, naming the host value
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
parent cannot catch a pin. Instead, at each **boundary** (a node with `-exact-mode` or with `color-scheme`), the kernel
writes every inherited colour row (the schema's inherited colour rows: `color`,
`caret-color`, `fill`, `stroke`) whose inherited value came from a token as an implied row
on that node, holding the token's value for the node's tuple. A `color-scheme` boundary needs
it too: on the web a `light-dark()` resolves on the element that specifies it and
descendants inherit the used colour (LLP 1034), so a Light card on a dark page would keep
dark text without it. Descendants inherit it as any row; the wasm host emits it in the boundary's
CSS (`host/web/src/element.rs`); the JS target does the same (below). So text inside a Black
preview on a Dim page is Black's colour, though its row was written on the page. **Only
colours re-select.** Re-specifying a font would compound an `em` size at the boundary
(`kernel/src/style/relative.rs`), so an inherited non-colour row may not name a token that
varies by an app axis (`tamber-inherited-axis`).

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
  over the inherited ones), in one canonical order (axes as declared, `axis=value`
  joined by commas). The page has one rule per tuple the app can reach (at most 64),
  selected by exact match, and each rule declares **every** token that varies by any app
  axis, default values included. So a Black pin inside a Dim page restores Black, and no
  property is inherited across a boundary. No token's value is a `var()` of another token;
- **a boundary (a mode pin or a `color-scheme`) re-specifies inherited colour tokens**, the kernel's rule: CSS inherits
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
  a block, not for these rows. §9 Q6 asks.

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
  compiler lists every override that is `none` there (`tamber-unreachable: accent-color on
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
  (`app.json`) and no `svg` fallback is refused (`tamber-icon-missing: icon.share has no
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

**Packages.** A package (`packages/reading`) may ship `<package>.tamber` with tokens named
under its package (`reading.measure`, `reading.body`), which its views use. The app's design
may set any of them by full name. Two definitions of one name from different files are a
compile error, unless the app's design sets it. Dials and axes are the app's alone: a package
may not declare them, and a package token's `when` lines may name only the app's axes, which
the app must declare or the package's token is refused. What a build keeps of a package's
tokens is D7's one retention rule.

### D12 — Every language gets generated names over the one table

`bun exact.mjs tamber gen <target>` writes bindings from the IR, generated and never
committed.

| Target | What it gets | For |
|---|---|---|
| Contract | nothing generated: the compiler checks names against the IR | views |
| TypeScript | a `Token` string-literal union per kind in the types `bun exact.mjs contract types` writes, so a data module returning `"surface.card"` is checked; `tamber.css` and `tamber.ts` (resolved constants per tuple) for code outside a plan | `app.ts` data modules; web pages outside Exact |
| Rust | `pub const SURFACE_CARD: Token<Color>` (and `Token<Length>`, `Token<Number>` …) and `session.tamber().get(token, node)` | data crates returning names; the terminal host; a game's Canvas 2D; Rust native modules |
| Swift | `ExactTamber(view).color(.surfaceCard)` as a dynamic `UIColor`/`NSColor`, `length(…)` and `number(…)` in points, `font(.typeBody)`, `symbol(.iconReply)`, and `observe` | native modules (LLP 1024), LLP 1075.003's hatches, the sample host's screens (LLP 1031) |
| Kotlin | `ExactTamber(view).color("surface.card")` as an Android `Color` int, `length(…)` and `number(…)`, and `observe`; a Compose adapter is optional, outside the host | Android native views |
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

### D13 — The Tamber protocol: one versioned edit API, with the files as its storage

**Ruled (Charlie, 2026-10-13):** a design is changed through an API, not only by hand or
through a development-only editor. The API is public and versioned. External tools (Figma,
Paper, Pencil and the like) connect through it, and the design tool (D14) is built on
it and is the main way designs are edited. Files are its durable storage.

**The server.** `bun exact.mjs tamber serve` owns one project's design: its `.tamber` files and
the `params` sections and state parts in its `.contract` files (D15, D8). It runs inside
whichever dev loop is running (`host/web/dev.mjs`, whose `exact_web::dev::Session` compiles
the plan, or `host/web-js/dev.mjs`), or alone, with no app running, for a tool that edits
without previews. **One writer per project:** the server holds an exclusive lock
(`.exact/tamber.lock`), and a second server for the same project refuses to start. It keeps no
state of its own beyond what the files say.

**The protocol, `tamber/1`.** JSON messages over a WebSocket, and the same messages as one-shot
HTTP POSTs for scripts. Its reference is `docs/tamber-protocol.md`, versioned with it.
- **`hello`** negotiates the version. A minor version adds operations or fields; a client
  ignores fields it does not know. A major version the server does not speak is refused. The
  IR's format number (D2) is reported with it, and the connected development builds are
  listed, each with an id.
- **Reads:**
  - `describe`: the IR (kinds, axes, dials and component params with their editor metadata,
    tokens and their `when`s, the dials and params each depends on);
  - `resolve`: a token's value for a tuple and a scheme, **as the build stores it**: a
    constant, or the residual (a role, a `platform-color`, a mix, a `rem` length) with its
    fallback, labelled as a fallback. A concrete value for a residual (what iOS shows in
    Increased Contrast, a `rem` at this Dynamic Type size) comes from `resolve-on`, which
    names a connected build and a node, and says which build and traits it resolved under.
    Contrast badges say which of the two they assessed;
  - `explain`: a token's chain to its dials and params;
  - `uses`: where a token is used, from the connected builds (D7's index).
- **`subscribe`** streams every change, whoever made it: another client's write, a hand edit
  (the server watches the files), a `git checkout`. Each event carries the new digests and,
  for a client's write, the client that made it.
- **Writes** are the operations below, alone or as a **transaction**:

| Operation | Edits |
|---|---|
| `set-dial name value` | a dial's value |
| `set-token name [when …] value`, `unset-token-when name when…` | a token's default or one `when` line; unset removes the line |
| `set-param component.name value`, `unset-param component.name` | the app's value for a component param (D15); unset removes it, so the component's default applies again |
| `set-state style state row value`, `unset-state style state row` | a row of a state part (D8), through `contract fmt` |
| `add-token`, `rename-token`, `remove-token`, `add-axis-value`, `remove-axis-value`, `add-param`, `remove-param` | structure |
| `preview build axis=value` | nothing on disk: pins an app mode on one connected build, by its id, as `prefer mode` does |

- **Each transaction:**
  1. carries the digest of every file it edits as the client last saw it; the server, the
     one writer, checks them under its lock, so two clients editing at once conflict instead
     of overwriting;
  2. is checked by the evaluator; a refused transaction changes nothing and returns the
     diagnostics;
  3. is **committed as a set**: every changed file is formatted (`tamber fmt`, `contract
     fmt`) into a temporary file and fsynced, an intent record listing the renames is written
     to `.exact/tamber.journal`, the files are renamed into place, and the record is removed.
     A server that starts and finds a record completes its renames, so a crash never leaves a
     transaction half-applied. Files stay hand-editable, one line per value, and git is the
     history;
  4. is published (the event, the live table below) only after the whole commit;
  5. returns the new digests and the **inverse** transaction, whose preconditions are those
     digests. An inverse restores absence where the write created something
     (`unset-param` undoes the first `set-param`, so the component's default keeps working).
- **`propose`** runs a transaction without writing: the diff, the diagnostics and the tokens
  it would change. Importers use it so a person accepts what another tool suggests (D14).

**What a commit does to running development builds** depends on what changed, judged by the
whole dependency closure, not the operation's name:
- **Only values in the token table** (the structure digest, the plan without its design
  table, is unchanged, and no affected token is substituted at build): the **live table**.
- **Anything else** (a structure change; a state part, which compiles into rows and state
  looks; a `duration` or `easing` token, or any dial or param they fold from, which D3
  substitutes at build): the ordinary rebuild and reload, which carries state (LLP 1007 §6).

The same rule applies to a hand edit: saving a value change in `app.tamber` is a live-table
edit too.

**The live table.**
- **No plan is published for a value-only change.** Today a saved source becomes a new plan
  file, and publishing it is what restarts the page (`exact_boot_plan`, LLP 1007 §6) and the
  Apple hosts (`EXACT_DEV_PLAN`, LLP 1008), losing scroll, focus and drafts; the JS loop
  reloads the page on any change in the app directory (`host/web-js/dev.mjs`). So both
  loops' watchers, and `Session`, ask the evaluator whether a change to a `.tamber` file left
  the structure digest unchanged; if so they publish no plan and do not reload. The decision
  is by content, so it holds across processes and for a hand edit.
- **The batch travels on the channel the builds already hold to the dev server** (the dev
  event stream that tells them a plan changed), as a new message: the changed tokens, each
  value as its wire text (a colour as CSS text, as LLP 1095's wire carries colours; a length
  as its dimension), tagged with the structure digest, the design digest it applies to, and
  the new one. A build applies it only if its design digest is the batch's base; otherwise,
  and on every connect, it asks for a full snapshot.
- **A build applies it in one commit** through a new kernel entry (`exact_design_rows` in each
  host's ABI): the kernel replaces the rows and runs D7's refresh. The JS target's runtime
  rewrites, in one style update, every boundary rule that declares a changed token.
- **Each process interns colours itself.** A `Wide` id is an index into a process-global
  table capped at 4,096 entries (`kernel/src/style/wide.rs`), so ids never cross processes.
  An entry is reference-counted: it lives while a row, a fallback, a token value or a
  transition's endpoint holds it, and is freed after. A full table refuses the batch's new
  colour with a diagnostic to the tool, and the build keeps its old value: nothing is
  dropped silently.

**Authentication.**
- **Local clients** use the **write token**: random, held in the server's memory, never
  written to disk (`.exact/` is not ignored by git). It is given only in answer to a
  request that passes the install page's test whole: admitted by the dev gate, no
  forwarding header, loopback host and loopback peer (`access.local`,
  `scripts/install-page.mjs:269-292`); any other request for it gets the not-found the
  install endpoint gives. A client presents it as the first WebSocket message, or as an
  `x-exact-tamber-token` header on a POST, never in a URL. Reads need it too, since a design
  may be unreleased.
- **Who can speak the protocol.** A native or wasm Exact session cannot set headers or send
  frames on a socket (LLP 1016.000); a TypeScript data module on the web's JS target can.
  So the design tool (D14) builds for the web's JS target, and its data module is the
  protocol client. `bun exact.mjs tamber op …` is a client over HTTP.
- **Remote and collaborative clients** wait on §9 Q4. The proposal: **pairing**. The local
  design tool shows a code (at least 40 bits, valid two minutes, five attempts); the remote
  client presents it once, on the pairing endpoint, which is the only thing a non-local
  request can reach, and receives its own token, scoped `read`, `propose` or `write`, listed
  and revocable in the tool, every write attributed to it in the event stream. Several people
  editing one design are several clients of one server, serialized by the digests. A hosted
  relay for a team that shares no machine is not in r6.
- **Release builds** have no protocol endpoint, no live table and no token. A shipped app's
  design changes by an update (D7).

**What it is, in DEFERRED's terms.** The server and its file writes are tooling, against
§Tooling's "no Design Mode … no devtools UI"; the live table patches a running development
session outside the dev reload, a hot revision of values only, against §Runtime. Both wait on
Charlie's admission (§9 Q3).

**Provenance is a lookup**, through the protocol and the agent alike:
- **Where a token is used:** D7's index lists every mounted node holding it, on every kernel
  host; the JS target's runtime keeps the same map. T3 Code's "Accent color · 4 uses" needs
  no `data-paint` word.
- **Explain this pixel:** `tree` and `explain` report, for a row holding a reference, the
  token, the `when` that won, and its chain (`background-color: surface.card ← neutral
  (dial)`).
- **The agent pins app modes** with a fourth `prefer` group, `mode` (`prefer mode
  dark-style=dim`), beside `media`, `page` and `fold` (`scripts/agent-prefer.mjs`,
  `host/linux/src/agent.rs`). It runs `setMode` and leaves `prefers-color-scheme` alone.

### D14 — Clients: the design tool first; Figma, Paper and Pencil are clients too

**The design tool is the primary client**, the "living Figma". It is an Exact app built
for the web's JS target, whose TypeScript data module speaks `tamber/1` with the local token
(D13), like any other client:
- **dials and params**, grouped and ordered by their declared `group` and `priority` (D4,
  D15), each drawn with its declared control;
- **a token × mode grid**, like Figma Variables: resolved swatches per tuple and scheme,
  contrast badges between declared on-pairs, which dials each token follows;
- **a component workbench:** each component's params with their controls, its state parts,
  and its previews in every mode, drawn by a connected build of the app pinned per mode
  (`preview`, and `color-scheme` with `-exact-mode` inside the app's own preview screens,
  D5);
- **the app's own screens**, as its connected development builds (a phone, the Mac, a
  browser tab), listed by `hello` and all changing together through the live table.
  The tool embeds the app's web build in an `iframe` (LLP 1020): a second document at the
  dev loop's URL, not a second session in the tool's runtime, which a web JS-target app
  could not host. *Amended by LLP 1122 D3 (2026-10-13).*

**Agents** use the same operations: `bun exact.mjs tamber op set-token surface.card …` is a
protocol client with the local token.

**External tools are clients, not peers.** Each connects through `tamber/1`, with the same
checks and the same authority: the files.
- **Authority.** Names, kinds, axes and structure change only through structure operations
  that pass the checks; values change only through writes. No tool has a side channel.
- **Importers propose.** A Figma plugin (its manifest may allow `ws://localhost`; a bridge is
  needed only where a plugin should not hold the token), a Paper or Pencil integration, or a
  DTCG file turns its
  changes into `propose` transactions; a person or agent accepts them. A change to a token
  folded from a dial is offered as a dial change where one dial explains it, or as a pinned
  override the tool marks ("no longer follows `accent`"), so a dial never goes inert unseen.
  exact1's Facet never made this choice, and `type.ratio` died of it.
- **DTCG** is one client's format: `tamber export dtcg` writes DTCG Format 2025.10 with the
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

**Importers** (Charlie, 2026-10-13). Each importer is a `tamber/1` client that only
**proposes** (D13 `propose`). It reads another system's theme and writes a transaction of
`add-token` and `set-token` operations, with an **unmapped report** listing every source
value it did not map and why. A person or agent accepts the proposal. Nothing is ever
dropped silently: the source values number exactly the mapped ones plus the reported ones,
and the importer checks that before it proposes.

What every importer shares:
- **Names.** A source's names become dotted Tamber names under a prefix the person picks
  (`shadcn.` by default). A segment that starts with a digit takes `s`
  (`blue-500` → `blue.s500`, D3).
- **Role mappings are proposed separately.** Where a source token means a platform role
  (`--background` is the page), the importer proposes the role override as a separate
  item the person can decline, never folded into the token itself. A design stays the
  platform's unless the person chooses otherwise (D1).
- **Kinds not built yet are reported, not dropped.** Before stage 6, length, type, shadow
  and motion values were reported as `kind pending (stage 6)`, with the token each would
  become. **As built (stage 6 H, 2026-10-17):** they are proposed, each as its kind's token
  (`add-token` writes the kind), when it folds as that kind beside the import's other
  tokens; a `var()` in one is the token it names. One that does not fold, or that differs
  between light and dark (only a colour has a dark side), is reported with why. shadcn's
  derived `radius.sm/md/xl` (`calc()` over a `rem` radius) are reported, outside the count,
  until QUEUE's "`rem`/`em` where the kernel still takes only `px`".
- **Components and utility classes from these libraries are out of scope** (§4).

**1. shadcn/ui** (first: the most common). Source: the `:root` and `.dark` blocks of a
`globals.css`. The values are either v4's `oklch()` or v3's bare HSL channels
(`222.2 84% 4.9%`, read as `hsl()`).

| Source | Tamber | Modes |
|---|---|---|
| `--background`, `--foreground` | `shadcn.background`, `shadcn.foreground`; proposed role overrides `-exact-background`, `-exact-label` | `:root` is the light side and `.dark` the dark side: one `light-dark(a, b)` |
| `--card`, `--popover` and their `-foreground` | `shadcn.card`, `shadcn.card-foreground` … ; proposed `-exact-secondary-background` for `--card` | same |
| `--primary`, `--primary-foreground` | `shadcn.primary` …; proposed: `AccentColor = shadcn.primary`, a role override every host makes the app's tint (amended 2026-10-15, §10) | same |
| `--secondary`, `--muted`, `--accent` and their `-foreground`; `--destructive` | `shadcn.*` | same |
| `--border`, `--input`, `--ring` | `shadcn.*`; proposed `-exact-separator` for `--border` | same |
| `--chart-1…5`, `--sidebar-*` | `shadcn.chart.s1…`, `shadcn.sidebar.*` | same |
| `--radius` | `shadcn.radius` (length), with `radius.sm/md/lg/xl` as shadcn derives them (`calc(shadcn.radius - 4px)` …) | none |
| a further theme class (`.theme-blue`, `[data-theme=x]`) | an app axis, proposed (`theme = default \| blue`) | `when theme=blue` |

Gate: shadcn's default theme and one colour theme from its themes page import with every
variable mapped or reported. Accepting the proposal and exporting gives back the same
light and dark colours. Re-importing the unchanged source proposes nothing.

**2. Tailwind CSS v4 `@theme`.** Source: the `@theme` block (and `@theme inline`), plus
dark overrides wherever the stylesheet redefines the variables (`.dark { … }`,
`@media (prefers-color-scheme: dark)`, `@variant dark`).

| Source | Tamber | Modes |
|---|---|---|
| `--color-<name>`, `--color-<name>-<step>` | `tw.<name>`, `tw.<name>.s<step>` (colour) | a dark redefinition is the dark side of a `light-dark()` |
| `--spacing`, `--spacing-<n>` | `tw.space.base`, `tw.space.<n>` (length) | — |
| `--radius-<n>` | `tw.radius.<n>` | — |
| `--font-<n>` | `tw.family.<n>` | — |
| `--text-<n>`, `--text-<n>--line-height`, `--text-<n>--font-weight` | `tw.type.<n>` (size, line height and weight) | — |
| `--font-weight-<n>`, `--leading-<n>` | numbers (`tw.weight.<n>`, `tw.leading.<n>`) | — |
| `--tracking-<n>` | `tw.tracking.<n>` (length, as built: an `em`) | — |
| `--shadow-<n>`, `--inset-shadow-<n>`, `--text-shadow-<n>` | `box-shadow`, `box-shadow` (inset), `text-shadow` | as built (stage 6 H): only a colour has a dark side, so a redefinition outside `@theme` is reported |
| `--ease-<n>` | `tw.ease.<n>` (easing) | — |
| `--breakpoint-*`, `--container-*`, `--blur-*`, `--drop-shadow-*`, `--perspective-*`, `--aspect-*`, `--animate-*` | not mapped, reported (no Tamber kind; breakpoints are layout, CSS's own) | — |
| `--*: initial`, `--color-*: initial` | resets, honoured: the reset names are not imported | — |

Utility classes and `@utility` are out of scope. Gate: Tailwind v4's default theme imports
(its palette, spacing, radii, fonts, shadows and easings) with every variable mapped or
reported. A project with a `.dark` block maps its colours to `light-dark()`. Re-importing
the unchanged source proposes nothing.

**3. DTCG JSON** (Format 2025.10, with the Resolver Module where present), from Tokens Studio
and from Figma Variables exports. This replaces stage 5's "DTCG import"; export stays in
stage 5.

| Source `$type` | Tamber kind |
|---|---|
| `color` | `color` |
| `dimension` | `length` |
| `fontFamily` | `family` |
| `fontWeight`, `number` | `number` |
| `duration`, `cubicBezier` | `duration`, `easing` |
| `shadow` | `box-shadow` |
| `typography` | `type` |
| `border`, `transition`, `gradient`, `strokeStyle` | reported (a gradient's stops become colour tokens when they are references) |

- **Names:** a group path becomes a dotted name, with the segments sanitised and
  collisions reported.
- **Aliases:** `{a.b}` becomes a reference to the token, for a colour; a value of another
  kind is written out (as built, stage 6 H), inside a composite too.
- **Modes:** a Resolver modifier, Tokens Studio theme set, or Figma mode named `light` and
  `dark` (or `Light` and `Dark`) is the scheme, and becomes `light-dark()`. Any other
  modifier becomes an app axis, proposed, with its contexts as values and `when` lines for
  their values.
- **Our own export:** a file `tamber export dtcg` wrote carries
  `$extensions["exact"]`, and re-import restores the source expressions from it (D14).

Gate: a Tokens Studio sample set and a Figma Variables export with light and dark modes
import with every token mapped or reported. Our own export round-trips with no proposed
operations.

**4. Astryx** (per exact1 LLP 0267). It is also a **probe for gaps in Tamber's token kinds**:
its unmapped report is the evidence for new kinds, as it was for exact1's Proposal I.
Source: a published theme package (`@astryxdesign/theme-neutral`, pinned to a version) or a
`defineTheme()` result, read as resolved light and dark values (`resolveThemeTokens()`).

| Source | Tamber | Modes |
|---|---|---|
| `--color-background-body/surface/card/popover/inverted` | `astryx.surface.*`; proposed role overrides (`-exact-background` for body) | light and dark values become `light-dark()` |
| `--color-text-*`, `--color-icon-*` | `astryx.text.*`, `astryx.icon.*` | same |
| `--color-accent`, `--color-on-accent`, status colours with muted and on variants | `astryx.*` | same |
| `--color-overlay`, `--color-overlay-hover/pressed` | `astryx.overlay.*` (as colours; the pressed look is the author's state part, D8) | same |
| `--spacing-*` | `astryx.space.*` | — |
| `--radius-inner/element/container/full` | `astryx.radius.*` | — |
| `--shadow-low/med/high`, inset rings | `box-shadow` (rings as `inset` shadows) | same |
| `--duration-fast/medium/slow` | `duration`; their **min / nominal / max tiers** are reported (Tamber has one value per duration token) | — |
| easing | `easing` | — |

Gate: the pinned `theme-neutral` imports with every token mapped or reported. The unmapped
report is attached to the stage's commit as the kinds probe; each gap it finds (the motion
tiers, for one) becomes a §9 question or a later kind, never a silent omission.

### D15 — Component params: a component declares what the tool may tweak, and how

**Ruled (Charlie, 2026-10-13):** a Contract component declares what is tweakable and how, as
part of its definition: typed params with ranges, editor controls and a priority or group, so
the design tool knows what to surface first. This is exact1 Facet's `params` and `design`
blocks (`button.contract`), made one section, without its TypeScript recipe tables.

**A `params` section in the component.** In Contract every value is a string, as a row's token
name is, and the editor metadata are ordinary attributes:

```contract
component PostButton
  props
    label: string
    enabled: bool
  params
    fill: color = "accent.fill"  control="color"  group="Colour"  priority=1
    radius: length = "radius.md"  range="0..40"  step=1  control="slider"  group="Shape"  priority=1  label="Corner radius"
    padding: length = "12px"  range="8..32"  step=2  control="slider"  group="Shape"  priority=2
    pressed: color = "color-mix(in oklab, param.fill 88%, text.primary)"  control="none"
  view
    button press=post disabled=(!enabled) background-color="param.fill" border-radius="param.radius" padding="param.padding"
      :active
        background-color = "param.pressed"
      text label
```

- **A param is a token owned by its component.** Its default is any token value: a literal, a
  token, or an expression over tokens, dials and the component's other params. Inside the
  component, a row and a default expression name it as `param.radius`, which the compiler
  rewrites to its full name. It is in the reference table like any token, so it follows
  modes, and a change to it re-folds every param and token that reads it.
- **Its identity is stable.** A component's identity is its declaring file, relative to its
  app or package root, and its name: `post.contract#PostButton`, or
  `@scope/reading/article.contract#Article` for a package's. A param's full name is that
  identity and the param (`post.contract#PostButton.radius`). The short form
  `PostButton.radius` is accepted wherever it is unambiguous and refused where two
  components share the name (`tamber-param-ambiguous`); an import alias (`use X as Y`)
  resolves to the declaring identity, so renaming an import or reordering imports never
  redirects an override.
- **The app's value.** The app's `.tamber` file may set a param by name, with `when` lines like
  any token; `set-param` writes there and `unset-param` removes it. The component's file keeps
  its default. One value per design: every instance shares it.
- **Params are not props.** A prop is per instance and runtime data; a param is the design's,
  the same for every instance, never read by the runner. A per-instance variant is a prop
  that chooses between params (`background-color=(primary ? "param.fill" : "param.quiet")`).
  The tool surfaces params, not props; showing a component's variant props in the workbench
  (exact1's `control prop variant as select`) comes with the component library (stage 12).
- **Controls, by kind:**

| Kind | `control=` | Notes |
|---|---|---|
| `color` | `color` (a colour well), `token` (a picker of colour tokens), `select`, `none` | |
| `length` | `slider`, `stepper`, `token`, `select`, `none` | `range` and `step` in px |
| `number` | `slider`, `stepper`, `select`, `none` | `range` and `step` unitless |
| `type`, `family`, `box-shadow`, `text-shadow`, `icon` | `token`, `select`, `none` | |
| `duration`, `easing` | `slider` (duration, in ms), `token`, `select`, `none` | an edit rebuilds (D13) |

  `select` takes its choices as `options="radius.sm|radius.md|radius.lg"`: tokens of the
  param's kind, shown by name. `none` is a derived param the tool does not surface. The
  default control is `token`. `label=`, `help=`, `group=` (groups in the order they first
  appear) and `priority=` (1 surfaced first, 3 behind "more", default 2) apply to all. App
  dials (D4) take the same metadata.
- **What a range checks.** A ranged length takes px literals only, so it can be checked at
  build; a `rem`, `em` or text-style value there is refused (`tamber-range-unit`). The default,
  folded for every tuple, must be inside the range (`radius.md` is 12 px in the example).
  An override that is a literal is checked against it; one that names a token is checked
  after folding. A scrub in the tool that replaces a token default with a literal is a pinned
  override, marked as D14 marks a dial's ("no longer follows `radius.md`").
- **Checks.** A param must be reached from a root: a row or a state part of its component,
  through any chain of params (`pressed` above is read by `:active` only, so it lands with
  state parts, stage 8), or, for a component that is not Contract, its declaration in a
  `.tamber` file, which makes all its params roots. Otherwise it is refused
  (`tamber-param-unused`). A param's kind is checked against every row that uses it (D3).
- **Language-neutral.** The `params` section lowers into the IR's `components` block: per
  component its identity, and per param its name, kind, default expression, range, step,
  control, options, label, help, group and priority. A component that is not Contract (a
  native module's view in Swift or Kotlin, a Rust canvas component) declares the same block
  in a `.tamber` file, in the `.tamber` grammar (bare CSS values, `in a..b`):

```
component MapCard
  params
    pin: color = accent.fill  control=color  priority=1
    inset: length = 8px  in 0..24  control=slider  priority=2
```

  and reads them through the bindings (D12): `ExactTamber(view).color(.mapCardPin)`,
  `.length(.mapCardInset)`.
- **Contract grammar.** `section = … | "params" block(param)`; `param = FIELD ":" kind "="
  STRING { attribute } NL`, with `range`, `step`, `control`, `options`, `label`, `help`,
  `group` and `priority` the admitted attributes. One `params` section per component. No new
  token in the Contract lexer: the range is a string.

## 3. The `.tamber` grammar

```ebnf
tamber-file  = "tamber" IDENT NL { block } ;
block       = "axes" NL block(axis) | "dials" NL block(dial)
                        | "theme" NL block(token) | "icons" NL block(token)
            | "exports" NL block(NAME NL)
            | "component" IDENT NL INDENT "params" NL block(param) DEDENT ;
param       = IDENT ":" kind "=" value [ "in" value ".." value ] { meta } NL ;
meta        = IDENT "=" ( STRING | NUMBER | IDENT ) ;
axis        = IDENT "=" IDENT { "|" IDENT } [ "under" "scheme" "=" "dark" ] NL ;
dial        = IDENT ":" ( "color" | "length" | "number" ) "=" value [ "in" value ".." value ] { meta } NL ;
token       = NAME [ ":" kind ] "=" value { meta } NL [ INDENT { when } DEDENT ] ;
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
tamber bluesky

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
  radius.md = 12px
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
- **No side channel for any tool.** Every client, ours included, edits through `tamber/1`.
- **No live two-way Figma canvas sync** (D14).
- **No components or utility classes from imported libraries.** shadcn's components,
  Tailwind's utilities and Astryx's components are not imported; importers bring tokens only
  (D14).

## 5. Staging and gates

Each stage needs its implementer and its predecessor's gate. Each names its schema-digest
break (every native host binary moves on one, LLP 1030 D3a) and the hosts that must read
what it adds: the web's JS target, the wasm host, iOS, macOS, tvOS, Linux, Windows,
Android's reader and the terminal.

1. **LLP 1095 D4 and its reach**, built from main (the earlier branch never reached origin).
   Gate: LLP 1095 §5 item 6's measurement, and Charlie's word, which is given: "Land it"
   (2026-10-13). It lands when the RFC owner's review, the external reviews and the checks
   pass. A reach report follows the landing, so he can see what reached what.
2. **Colour tokens and app axes.** The crate; `app.tamber` or inline `theme`; dials; app axes
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
3. **The Tamber protocol and the design tool, over files** (D13 without the live table, D14's
   own tool: the dials panel and the token grid). Needs Charlie's §Tooling admission. No
   digest break. **Gate:** the design tool, a web JS-target app, edits the clone's dials and
   tokens through `tamber/1`; every write is a one-line diff; two clients writing the same
   token conflict instead of overwriting; a second server for the project refuses to start;
   a transaction across two files killed between its renames is completed on restart; a hand
   edit and a `git checkout` reach a subscribed client; `propose` changes nothing on disk; a
   request without the token is refused, and a `--lan` client or a forwarded request cannot
   obtain it; each inverse transaction undoes, including an `unset-param` restoring the
   component's default.
   - **3a–3d. Importers** (D14, Charlie 2026-10-13). Each is a `tamber/1` client that only
     proposes, after stage 3, in this order: **3a** shadcn, **3b** Tailwind v4 `@theme`,
     **3c** DTCG (Tokens Studio, Figma Variables export), **3d** Astryx (also the
     token-kinds probe). No digest break. Before stage 6, values of other kinds are reported
     as pending. **Gate, each:** the known theme D14 names for it imports; every source
     value is mapped or listed in the unmapped report with a reason, and a test counts
     them; the accepted proposal, exported again, gives back the source's values;
     re-importing the unchanged source proposes nothing.
4. **The live table** (D13). Needs Charlie's §Runtime admission, and reclaimable live colour
   storage first: `Wide` entries are process-global, immutable and capped at 4,096
   (`kernel/src/style/wide.rs`); the policy says what a running transition keeps alive. No
   digest break beyond stage 2; the new kernel entry `exact_design_rows` on every host.
   **Gate:** a dial drag in the design tool, and a hand edit of a value in `app.tamber`,
   reach the clone on the iOS simulator, the Mac and the web at once, with no plan
   published, no reload and no state lost (a scrolled list stays scrolled, a draft stays
   typed); a structure edit still reloads; a build that reconnects after missing a batch gets
   the snapshot; more than 4,096 distinct colour edits leave storage bounded, and a full
   table refuses with a diagnostic rather than dropping; `preview` pins one named build;
   `uses`, `explain` and `resolve-on` are right on every host.
5. **Bindings and the DTCG client** (D12, D14). No digest break. **Gate:** a native module in
   the sample host draws with `ExactTamber(view).color(…)` and follows `setMode`, a pin above
   it, a Dynamic Type change and the live table; two embedded sessions with different
   designs answer differently; a TypeScript data module returning an unknown token fails
   `tsc`; a DTCG export re-imported unedited proposes no operations (DTCG import itself is
   stage 3c).
6. **Length, number, type, shadow and motion tokens.** Prerequisite: LLP 1115.000 D12.2 (D6).
   Digest break: the new reference forms on those rows. Hosts: all. **Gate:** Expose's 48
   `font-family` writes and its radii become tokens with screens identical; under Bold Text a
   system-font type token is bolded, one with an authored weight too, and a bundled family is
   not. `calc()` with `rem` in a token waits on QUEUE's `rem`/`em` item.
7. **Component params** (D15) and the workbench's params panel. Colour params may land with
   stage 2's kinds, the rest with stage 6's; a param whose only reader is a state part lands
   with stage 8. No digest break beyond those stages (a param is a token). `tamber-param-unused`
   starts here. **Gate:** the clone's post action row and Post button declare params; the
   design tool lists them by group and priority with their controls and `select` options; a
   `set-param` reaches every instance live, and one on a duration param rebuilds instead; a
   Swift native view's `.tamber` params appear beside them and draw through
   `ExactTamber(view)`; an unused param, an ambiguous short name and a `rem` value on a ranged
   length are refused; renaming an import keeps an override on its component.
8. **State parts and control roles** (D8, D9). Digest break: state looks; the control-role
   table. Hosts: all that paint a press. **Gate:** the clone's Post button and switches are
   right on every host with no hand fix and no double dim; a checked, disabled toggle with
   both parts shows `:disabled`'s; a class change and a base change while pressed keep the
   overlay over the new base; existing `disabledStyled` apps unchanged; LLP 1061's press
      corpus unchanged; `tamber-unreachable` names the macOS switch and iOS's disabled glass
   button; the workbench shows each component's state parts, editable through `set-state`
   (which rebuilds).
9. **Icon tokens** (D10). Digest break: none if the symbol wire already takes an SVG source
   per node, else one. **Gate:** the clone's ~100 `symbol:sf/…` uses draw on the web from
   icon tokens; a missing source fails the build with the target named.
10. **External clients and remote authentication** (D13, D14). A Figma plugin bridge, a Paper
    or Pencil client, pairing. Waits on §9 Q4. **Gate:** a changed accent in Figma Variables
    is proposed as a dial change and a changed folded value as a pinned override; a paired
    client with `propose` scope cannot write; revoking a client cuts it off at once.
11. **A base spacing and radius scale** (D11), when packages need a shared vocabulary:
    `space.*` and `radius.*` with values taken from each platform's metrics, not a brand's
    4 px grid. **Gate:** two packages restyle together from one app value.
12. **A component library**: `.contract` packages over Tamber, with params (D15) as their
    tweakable surface and anatomy as checked data, keeping exact1 Facet's rule, "a recipe
    styles parts; it never forecloses slots". Its own RFC. **Gate:** set by that RFC; its
    first consumer is named before it starts.

**Checks that stay from stage 2:** `tamber-dial-inert` (and `tamber-param-unused` from stage
7); golden IR and
folded tables for the corpus designs (one evaluator, so no cross-language parity corpus); a
plan-size budget (the clone's design adds under 4 KB); first frame unchanged within noise for
an app with a design.

## 6. Cost

- One crate; the compiler's design pass and `params` section; the app tuple on reference
  rows and the kernel's slice selection and token index; the web's mode-boundary generator
  (one generator with LLP 1095 D11's role sheet); the hosts' existing refresh paths called
  for row changes; the protocol server and the design tool; later, the stages' own pieces.
- Docs: a section in `docs/contract-for-agents.md`, a recipe in `docs/recipes/`, and the
  protocol's reference (`docs/tamber-protocol.md`), versioned with it. The required read grows
  by one line: "a brand is an `app.tamber`; a colour or size names a role or a token".
- It removes per-app `Theme` records, threaded palettes, repeated `light-dark` pairs, the
  clone's hand-fixed switches, and the `data-paint` workaround.

## 7. Risks

- **A second way to write a colour.** The no-tells lint is taught that a token name is not a
  literal, and the docs give one rule: a role if the platform has one, a token if it is the
  brand.
- **The reach is unfinished.** D1's claims about overrides inherit stage 1's measurement.
- **A public protocol is a compatibility promise.** `tamber/1` is versioned from its first
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

**Ruled (2026-10-13):** an API is the way a design changes, public and versioned, with the
design tool its main client and the files its storage (D13, D14); the system is named
**Tamber** (provisional), with `.tamber` files ("probably fine"), and the design tool will
be named separately; a component library and a base scale are wanted later (stages 11 and
12); components declare their params in Contract (D15); a DSL in files is right; build it
("Can we start building tamber").

Each question below has a **provisional default**, the RFC's current recommendation, which
the implementation follows until Charlie rules otherwise.

1. **How generative?** D4 keeps dials but expects a brand to be hand-tuned literals with a
   few dials, and defers `ramp()`. More (a brand from three dials) or less?
   *Provisional default:* as D4 says: dials, literals, no `ramp()` until an app asks.
2. **References.** Token references as bare names (as LLP 1095 D4 built) or CSS's
   `var(--name)` with custom-property inheritance (subtree overrides for free, but a
   cascade)? *Provisional default:* bare names (D3).
3. **DEFERRED admissions.** The protocol server and design tool (stage 3, against §Tooling)
   and the live table (stage 4, a development-only hot revision of values, against
   §Runtime). **Ruled (Charlie, 2026-10-13): admitted, for development builds only,
   together with LLP 1122 §9 Q2.** This is his waiver, so no take is named. Release builds
   keep both refusals. The `rules/DEFERRED.md` text lands with LLP 1122.
4. **Remote and collaborative clients.** D13 proposes pairing with scoped, revocable,
   attributed tokens, and leaves a hosted relay out. Is that the model, and does a team
   relay belong in this program? *Provisional default:* local clients only; pairing as D13
   proposes when stage 10 comes; no relay.
5. **Persistence and contrast.** Should `setMode` and the root-size percentage persist and
   apply before the first frame (so Dim never flashes Black)? Is an authored contrast
   variant for app colours wanted (D5)? *Provisional default:* neither persists, as
   `setRootFontSize` does not today: the app keeps the choice and sets it from a mount task
   (LLP 1069.000 D3); no authored contrast variant.
6. **State parts** amend LLP 1061 D1. Acceptable as parts with paint rows only, or named rows
   (`-exact-pressed-background-color` and the like) and no block? *Provisional default:*
   parts with paint rows only (D8).
7. **Figma.** Is a Variables writer wanted beside the plugin bridge, and is Enterprise access
   (which Variables writes need) available? *Provisional default:* DTCG export and
   import-as-proposals only; no Variables writer.

## 10. As built

Notes on what was built where it differs from, or makes precise, the decisions above. Each
is the implementer's (x2-perf3), reviewed by the RFC's owner.

### Stage 2, commit 9: `setRootFontSize("125%")` (`b00aad973`, reviews `178ae7237`)

Built as D6 says, with no difference:
- The grammar is CSS's number syntax with a `%`, above 0. `exact_num::css_percent` reads it
  for the compiler and the runner, and the JS target uses the same pattern.
- The runner's app size is either px, which parks the host's size, or a ratio, which
  re-resolves in one commit at each new host size (`runner/src/runner/root_font.rs`).
- The web writes `calc(var(--exact-host-font-size, 100%) * ratio)`; the agent's
  `prefer root-font-size` sets that variable.
- Apple and Linux needed no change.
- Neither form persists across a launch (§9 Q5's default).

### Stage 2, commit 1: the `exact-tamber` crate (`8dc5953af`, reviews `67aa250d3`)

The crate's grammar, checker and colour folder follow D2–D5, D7 and §3, except:
- **The lexer is the crate's own.** `contract/syntax`'s lexer makes tokens, but a value
  must be kept as CSS text (§3). So `tamber/src/lex.rs` reimplements Contract's layout
  rules over lines: space indentation, `//` comments, blank lines, and strings with
  escapes. §3's "the Contract lexer, extended" holds for the rules, not the code.
- **The role vocabulary comes from the caller.** The list of roles and each role's
  fallback pair are passed in (a `Vocab` read from `kernel/tables/schema.json`), so the
  crate stays a leaf.
- **A `light-dark()` with a role on one side is `tamber-residual`.** `ColorValue::LightDark`
  holds two fixed colours, so that pair has no kernel form. D5's "refused unless written
  inside a `light-dark()`" (`tamber-under-role`) therefore has no accepted case until LLP
  1095 D10, and the diagnostic says so.
- **"The default's light side" under an axis `under scheme=dark`** is the light side of the
  value the tuple selects when every `when` naming such an axis is ignored. So a `when` on
  another axis (`density=compact`) still sets the light side.
- **A token that names another token** takes that token's value for the same tuple.
- **A `color-mix()` in `hsl` or `hwb`** is stored as `Wide` in srgb. The kernel has no `Wide`
  form in those spaces, and serializing as `hsl()` would quantise to `Fixed`, against D7's
  "every mix is Wide".
- **Reserved names (D3).** A token or dial may not be named:
  - a CSS colour keyword;
  - a CSS-wide keyword (`inherit`, `initial`, `unset`, `revert`, `revert-layer`);
  - `none`, `from`, `in`, `alpha`, `transparent` or `currentcolor`.

  `scheme` and `contrast` may not be app axes (`tamber-axis-host`). A platform colour's
  names follow the kernel's `native_name_ok` rule, and its fallback must be a legacy colour
  or a pair of them, as the kernel requires. The token's fallback pair is the pair the
  kernel reads back.
- **Refused, not folded yet:**
  - `currentcolor` as a token's value (no `ColorValue` holds it);
  - a missing component (`none`, or a powerless hue that a relative colour names, by CSS
    Color 4's thresholds);
  - a `color-mix()` without `in <space>`, or with more than two colours.

  Both `color-mix()` forms landed afterwards, in `bd83193ea` (reviews `e173b07c0`). The
  method is optional (oklab), and the mix takes one or more colours with CSS Color 5's
  normalisation, run premultiplied, one colour after another. In a polar space each step
  takes its own hue arc, which can differ slightly from a single n-way average; the code
  says so. Rec. 2100 `color()` came with them, absolute and relative.
- **`tamber-dial-inert` decides by influence, not mention.** Each dial is set to probe values
  and every token refolded; a dial that changes no token is inert. A dial that moved a token
  only at values no probe takes would be wrongly called inert. A probe the fold refuses is
  not counted as influence.
- **§3's example** indented `radius.md` four spaces, which reads as a malformed `when` under
  `type.body`. It is corrected above. The crate's corpus keeps the uncorrected text as a
  refusal case (`tamber/tests/corpus/bluesky-rfc.err`).

### Stage 1: LLP 1095 D4 and its reach (`7a95dcce4`..`13f49af39`, landed 2026-10-14)

Built from main; the earlier branch never reached origin. Charlie pre-approved the landing
("Land it", 2026-10-13). It landed on the RFC owner's review, three rounds of external review
for the core and three for the ports, and the checks.
- **Commits:**
  - `7a95dcce4`: named colours;
  - `cd77df1e8`: overrides where the kernel resolves;
  - `72b1e154f`: the table bound by pointer;
  - `9f8933fe8`: the reach;
  - `edff3d727` … `df717737d`: review rounds;
  - `bfde5ff10`: minors;
  - `59a4020df`: no-tells;
  - `8aa7bf43b`: Expose;
  - `f63541d57`: the readers, and the Rust `Theme` record deleted;
  - `856c17ee4` … `13f49af39`: review rounds.
- **What Charlie saw:** the reach report, `~/x2theme/stage1-reach-report.md`.

**Built differently from LLP 1095 D1: no `ColorValue::Ref`** (approved by the RFC's owner and
the lead, 2026-10-13; Charlie told).
- **The table and its scope.** The plan carries a `theme` table. The kernel's colour parser
  resolves a theme name to the entry's value, which is an existing form (`Fixed`,
  `LightDark`, `Role`, `Platform`, `Wide`, `Profiled`), through a scope set from the owning
  plan for each parse, as `color(--profile …)` resolves. The scope is an `Arc` table built
  once per plan and bound by pointer: a themed 1,000-row list allocates exactly what its
  literal twin does. Rows hold ordinary values. There is no wire discriminant and no
  schema-digest break.
- **Role overrides** are a role → value table.
  - The kernel applies it to its own defaults (unsaid text's `CanvasText`), to its fallback
    and resolution (Linux's painter), and to what crosses to a host.
  - Each Apple host is sent it once at boot (`{"op":"roles"}`), and the surfaces it draws
    itself read it: the iOS page ground, grouped list grounds, cards, row text, headers and
    footers, bar titles, sheet and dialog grounds, placeholders.
  - Those colours resolve per trait: dark, Increase Contrast and the elevated level on iOS,
    contrast on the Mac. Wide colours keep their components.
  - The Mac's grouped cards and separators read overrides of
    `-exact-secondary-grouped-background` and `-exact-separator`; without an override they
    keep their measured form colours. A native multiline editor's text view and scroll
    view read `Field`, keeping their separate AppKit defaults without it. Author-drawn
    and Markdown editors keep transparent native views. Light/dark app captures and
    editing across a scheme change qualify these surfaces; a changed table still takes
    effect through boot/reload, not a new live-table notification.
  - The web gets the overrides as `:root` custom properties that the host's own sheet
    reads.
- **What this costs stage 4's live table.** With a `Ref`, changing an entry's value would
  swap one row and every row pointing at it would follow. Without one, a value change
  re-resolves each row holding the token, found through stage 2's token-beside-value index
  (D7). **Stage 2's gate measures it:** a one-token colour change on the 1,000-row fixture,
  where every row reads the token, must touch only those nodes, do no layout, and cost no
  more than the same change made by a `setMode` slice switch. If it does not, `Ref` comes
  back before stage 4. The plan's `theme` table is the same in both designs, so that would
  change the kernel and the hosts, not the plan or the language.

**Rules settled in review:**
- **The tint is the app's, set only through `AccentColor`.** Its value must be a colour, not
  another tint role. Overriding a role drawn in the tint (`LinkText`, `ButtonText`,
  `-exact-link`, `-exact-symbol`) is refused (`lower-theme-tint`). Tint roles stay roles in
  rows, on the wire and in the JS target's map. Each host makes the theme's colour the app's
  tint (iOS the presenter root's `tintColor`, the Mac the accent default, the web `:root
  --exact-accent`), so an authored `accent-color` still wins its subtree (D1).
- **Names are exact.** `Brand` and `brand` are two entries; only a role's spelling matches
  without case.
- **A name spelled like a role is that role's override**, as LLP 1095 D4 says. So an entry
  named `field` replaces CSS `Field`; the readers renamed theirs `fieldBg`. A warning for a
  probably-unintended override is a candidate diagnostic.
- **Refused:** two spellings of one role (`lower-theme-duplicate`); CSS keywords as names;
  the words gradients and shadows write beside colours (`to`, `top`, `inset` …).
- **An entry may be any colour a row takes** (`ColorValue::parse_light_dark` is the general
  parser), including a declared profile's colour.
- **Entries are written as strings** (`brand="#c8102e"`), since Contract attributes are
  strings, where LLP 1095 D4's example wrote bare CSS.
- **A name is read as a whole component** (a row, a shadow's colour, a gradient stop, a
  palette entry), never inside a `light-dark()` half or another function (LLP 1095 D1).
  Names inside expressions are this RFC's synthetic tokens (D3).
- **`lower-colour-one-scheme` and `scripts/no-tells.mjs` read the theme:** the root `theme`
  block is the app's palette; a row naming a one-scheme entry and an entry replacing a role
  the page paints are flagged.
- **A boot the runner refuses** keeps the thread's previous overrides; clearing another
  plan's set is journaled.
- **Text on the web under a scheme.**
  - The root carries `--exact-scheme-color`.
  - One rule re-reads it on any element whose own style sets a `color-scheme`:
    `[style*=color-scheme]` for inline and bound schemes, and the JS target's generated
    scheme classes. So unsaid text on a dark card under a light page takes the dark side.
  - It loses to the look's control colours and to any authored colour. An authored colour,
    static or bound, writes `--exact-scheme-color: initial` for its subtree.
  - Nothing is written without a theme that replaces `CanvasText`.
  - The substring selector and the class list are accepted as built. A compiler-emitted
    marker attribute replaces them if they ever misfire (a `color-scheme` mentioned inside
    another property's value).

**The ports.**
- **Expose** is on a 43-entry theme: no hex left in its rows, no-tells colour findings from
  124 to 0, and 22/22 screens identical on the web, iOS and the Mac.
- **The readers** (`apps/llp`, `apps/markdown`, `apps/markdown-stress`, `packages/reading`)
  use root themes: 104/104 screens identical, and the Rust `Theme` record is gone.
- **Each of the three reader apps carries the same 20-entry palette**, because a theme
  belongs to the app root. One Rust constant used to serve them. That duplication is the
  case for D11's package tokens: once `packages/reading` ships its own `.tamber`, the apps
  share one palette.

**Known limits:**
- **Overrides are per thread, not per session.** A plan whose overrides replace another
  plan's different set on the same thread (a second embedded session, a reload) writes one
  journal line. Per-session tables are owed by stage 5 (D12).
- **Vibrancy:** a themed role under a material isn't drawn vibrant.
- **Mac single-line field backgrounds:** on macOS 27.0.1 (26A434), offscreen captures of
  bezeled `NSTextField`s keep their native background after `backgroundColor` is set;
  the same borderless control paints the assigned colour. This includes the current
  default square bezel. Apple's [rounded-bezel restriction](https://developer.apple.com/documentation/appkit/nstextfield/drawsbackground)
  explains the rounded case only. Focused and onscreen native probes remain unmeasured;
  no replacement control is introduced. Evidence: `~/audits/exact2-no-tells-2026-10-11/theme-reach/current-main-mac-gaps-2026-10-14/field-native-probe-independent-runtime-review.txt`.
- **Bound authored colours:** on the web, an ancestor's *bound* authored colour that
  `unset`s writes nothing, as `text_unsets` does.
- **Still the platform's, by the reach ruling:**
  - the tab bar's material and unselected glyphs;
  - search field, segmented control and progress chrome;
  - the Mac's sidebar, toolbar and selection highlight;
  - alerts and system menus everywhere.
- **The Mac's own rendering, not a theme gap:**
  - its stars are drawn in `ButtonText`, which is `controlTextColor` there;
  - its links keep `linkColor`;
  - `color` doesn't tint a symbol image on the Mac, with or without a theme (in QUEUE).

### Stage 2, commit 2: the compiler reads designs (`1861578bc`..`d15a3f8ed`; core `280ddc710`)

Built as D2, D3 and D7 say, except:
- **Inline `theme` goes through the crate.** It is built into a design, one token per entry,
  and checked by `exact_tamber::check_design` like `app.tamber`, so its refusals use the
  crate's ids (`tamber-cycle`, `tamber-duplicate`, `tamber-unknown-token`,
  `tamber-name-reserved`). Lowering keeps what the crate doesn't know: `lower-theme-tint`,
  `lower-theme-name` for gradient, shadow and colour-space words, and `lower-theme-value`
  for profile misuse.
- **The inline block's syntax is unchanged:** `name="colour"` lines only, with no `when` and
  no other kinds. Its values are now the crate's, so a name inside an entry's
  `light-dark()` is read and folded. That widens LLP 1095 D4 ("a name inside light-dark()
  … is not read") for entries; rows still read names only whole.
- **Synthetic tokens are named by their expression text** in the plan's table. The kernel's
  scope matches that text exactly, and no other text with `(` is ever a name. The JS
  target's map reads them the same way, and treats a dotted name as one name.
- **Only colour rows must fold.** A literal in a colour row's value (through `?:`, `match`
  and `let`) must fold. A literal elsewhere (a `fn`, a state, a derive, an action, a call
  argument, a condition) becomes a synthetic token if it folds and is dropped if it
  doesn't, so a label that only looks like a colour never fails a build.
- **Plan values use the crate's serialization** (`#c8102eff`, `light-dark(#ffffffff,
  #17181bff)`). Every kept entry in Expose and the readers was checked to read in the
  kernel exactly as its authored text did.
- **Profile colours are carried as written.** The crate is told `transparent` for
  `color(--profile …)` entries and whole-name references to them; their names are still
  checked. Any other use, including a `when` that holds at the default modes, is
  `lower-theme-value`. So a profile entry can't take part in a folded expression yet.
- **App axes are folded, but until commit 3 the plan carries the default tuple only.**

### Stage 2, commit 3: app axes, the slice, boundaries (`c609fb23d`..`9c8b4df46`; core `2ef82a085`)

**D7 is built in the runner, not the kernel.** The runner is where a name already becomes
a value, since it evaluates each style binding and parses it with the theme bound. So:
- the runner picks a node's slice by binding the theme table for the node's tuple, built
  once per tuple and swapped in by pointer;
- the runner keeps the token beside the row, in its own per-node index;
- the kernel receives an ordinary selected value, as D7 says.

The index is per runner, which means per session. A token value change, a pin change or
`setMode` re-runs the indexed bindings through the normal binding path. The kernel compares
the re-parse's result, not the source text, and does its usual work for the changed row
(paint only for colours).

**The plan.** An `axes` table (each axis's name and values, the default first). The `theme`
rows hold the default tuple's values. A `theme_slices` table gives each varying token one row
per full tuple; the tuple index is mixed-radix in axis order.

**`-exact-mode`** is an inherited row. At every pin, the runner writes the node's full
canonical tuple as an implied row, so hosts, the wasm CSS and the web's `data-x-modes` read a
complete tuple. The author's partial text stays the authored value `layout` shows.

**Boundary implied rows** (at `-exact-mode` and `color-scheme` nodes, for `color`,
`caret-color`, `fill`, `stroke`) are written by the runner and indexed under their token, so
edits refresh them too.

**`layout` reports the token beside each row**, from the runner's index (for LLP 1122 D5's
inspect).

**Further details:**
- **The digest breaks once.** `-exact-mode` is bit 211.
- **A pin is stored as the full tuple** it makes over the tuple around the node, as the tuple
  index plus one, so hosts and the kernel read a complete tuple.
- **One spelling for a mode: `axis=value`**, everywhere a mode is written: a `.tamber`
  `when` condition, an `-exact-mode` pin (space- or comma-separated, each axis once),
  `prefer mode`, and the CLI's `--mode`. D5's pin example (`dark-style:dim`) is amended to
  match. `axis:value` is refused, with `tamber-mode` naming the right spelling; nothing had
  shipped with it, so no deprecation period is needed. A bad pin is refused at compile time
  and unsets the row at run time.
- **Boundary rows cover all eight inherited colour rows** in the schema: `color`,
  `caret-color`, `fill`, `stroke`, `accent-color`, `text-shadow`,
  `-webkit-text-stroke-color` and `-exact-symbol-palette`. An ancestor's unset row passes on
  the token above it, and a boundary's own colour coming and going restores its implied
  row.
- **The index** keeps each themed row's text, tuple, written flag and a 64-bit bloom of the
  token names. It is maintained at mount, binding rewrite and unset, and pruned when a view
  retires. A collection remembers the modes around it, for rows it mounts later and for
  reused rows.
- **The gate measurement holds, so the no-`Ref` design stands.** `Runner::set_token` on the
  1,000-row fixture sets exactly the rows that read the token: no tree walk, no binding
  evaluation, no layout. In release builds it took 0.27–0.77 ms, against 1.08–2.32 ms for a
  pin flip over the same list (test `a_token_edit_sets_only_its_rows_and_paints`).
- **Role overrides may not vary by axis** (`tamber-role-axis`): a role crosses to the hosts
  once, for the surfaces they draw.
- **`set_token`'s limits.** It refuses a role override (an edit does not yet reach the
  kernel's defaults or the hosts' roles); edits belong to the plan; a bloom collision re-sets
  extra rows harmlessly.
- **Detached renders fall back.** The document fold refuses a design with axes or a themed
  `color-scheme` boundary, and the render runs again with a kernel.
- **The web** writes nothing for `-exact-mode` until commit 6; the JS target shows the default
  tuple until then.
- **A known cost** (not changed): a pin change or mode switch re-evaluates a virtualized
  list's keys once. They compare equal, so nothing remounts.


### Stage 2, commit 4: `setMode` and `prefer mode` (`de4ed34e5`..`b9b0a25c0`; core `b28f6bf38`, the gate test `44a19d999`, the spelling `11996b734`)

Built as D5 and D13 say:
- **`setMode(axis, value)`** is a runner command.
  - It sets the window's tuple in the action's own commit and re-sets every themed row, laying
    out only where a selected value is in a layout row.
  - No host is told: hosts learn the result through the rows. A refused commit restores the
    tuple.
  - An undeclared literal is refused at compile time (`tamber-mode`); a computed one is
    journaled.
  - It takes one axis and one value. Nothing persists across a launch (§9 Q5's default).
  - **The gate line holds:** a colour-only `setMode` on 1,000 rows does no relayout
    (`44a19d999`).
- **`prefer mode axis=value`** is the fourth `prefer` group. It runs `setMode`, leaves
  `prefers-color-scheme` alone, checks every pin before setting any, and answers with the
  window's modes.
  - It works on Linux, the wasm web host, Apple, and the browser carriers.
  - The JS target refuses it until commit 6.
- **A known gap:** the web glue's own page group still accepts a root font size the kernel
  cannot store, when the request doesn't come through the agent. The agent refuses it.

### Stage 2, commit 5: the Apple hosts (`b9b0a25c0`..`3bd831dfb`; core `a82d688e4`)

**No host code was needed.** Because the runner picks the slice (commit 3), an Apple host
receives ordinary rows:
- a token that varies only by scheme or contrast stays a dynamic `UIColor`/`NSColor`;
- a token that varies by app axis arrives as the node's slice;
- `setMode` and pin changes re-set the rows, which the existing refresh paths show.

Both reviewers traced those paths (CGColor layers, gradients, filters and shadows, SVG paint,
transition targets, text caches) and found nothing that holds a stale value. `ThemeRoles` stays
one table, which `tamber-role-axis` guarantees.

**The evidence is `apps/theme-fixture`:**
- one axis;
- a card whose border, shadow, gradient, `drop-shadow()` filter and SVG paint all read
  tokens, with a transition;
- a token whose value is a role;
- Light, Dim and Black previews with text inherited from the page;
- a pin swap.

Driven on the Mac and the iOS simulator (`~/x2p2/tf-shots/`). Everything changes to Dim, the
previews keep their own pins (a Black preview on a Dim page and the reverse), the card
transitions between the two dark values, and light is unchanged.

### Stage 2, commit 6: the web (`644818d98`..`42abcd182`; core `0addea0dc`)

Built as D7's "The web" says, with these as-built details:
- **Only tokens that vary by an app axis become custom properties** (`--x-<name>`, synthetic
  ones `--x-s<n>`, suffixed until unique). Every other token stays folded inline, which
  resolves the same at the using element. That is D11's pay-for-what-you-use applied to
  D7.
- **Finding a row's `var()`.** The rows are the template kernel's, projected twice: once as
  the plan says, and once with each varying token bound to a sentinel colour no row writes.
  The sentinel's position is each token's `var()`.
- **One rule per full tuple**, `[data-x-modes="axis=value,…"]`; the default tuple's rule is
  also `:root`'s, so the first paint is right.
- **Boundaries carry `data-x-b`:** an `-exact-mode`, a `color-scheme`, or an `accent-color`
  where a varying token names the accent. `modes.js` gives each its full tuple and writes
  every inherited colour row it doesn't set from the nearest ancestor's row text when that
  row read a token. The rows are `color`, `caret-color`, `fill`, `stroke`, `accent-color`,
  `text-shadow` and `-webkit-text-stroke-color`; `-exact-symbol-palette` has no CSS. An
  accent-only boundary redeclares tokens and writes no inherited rows, as the wasm host
  does.
- **An element whose own accent names an accent-naming varying token** takes its
  `--exact-accent` from per-tuple `[data-x-accent]` rules on pin and scheme boundaries,
  avoiding the self-reference `css.rs` also avoids.
- **Modes are on** with axes, and also for a theme plus any bound `color-scheme`, since a
  scheme boundary is a boundary.
- **It sits beside stage 1's `--exact-scheme-color`.** That rule covers unsaid text under a
  `CanvasText` override; `modes.js` covers rows that read a token. The two are disjoint:
  `tamber-modes` has dark cards under a light section holding unsaid text, token text, text
  inherited from a token-coloured ancestor and a Dim pin, each taking its own side on both
  web targets (`42abcd182`).
- **`setMode` and `prefer mode` work on the JS target.**
- **Pages rendered ahead:** `render.mjs` resolves boundaries on its DOM, and a Rust-rendered
  page's baked token values give way to the class's `var()` on adoption. Known gap:
  `render.mjs` has no `CSS.supports`, so a bound value the property refuses counts as
  setting the row until the page runs.
- **Evidence:** conformance `tamber-modes` covers Black in Dim, inherited text, a Light card
  on a dark page, five accent cases, a bound pin and a cleared pin colour, over two axes,
  with no failures against the wasm host in Chrome and in WebKit.

### Stage 2, commit 7: the CLI, `layout`'s tokens, no-tells, docs (`b7647aff8`..`f9c97d3ee`; core `6a95834c2`)

- **The CLI (D2):** `contract tamber check | eval [--mode axis=value,…] | ir [-o tamber.json]`,
  also run as `bun exact.mjs tamber …`. `check` runs the crate's check and then the
  compiler's admission (the design compiled beside an empty view), so it refuses what a build
  would. Deferred: `tamber fmt` to stage 3, where the protocol writes through it; `gen
  <target>`, `export dtcg` and the TypeScript Token union to stage 5 (D12, D14).
- **`layout <id>` reports the token beside each row** (D7; LLP 1122 D5).
  - Each own row that read a token gets `token: {names, text, modes, implied}` from the
    runner's index. The names are exact: the kernel records the entries a parse reads
    (`theme::recorded`), and a role override read by its spelling maps back to the token's
    name.
  - `implied` marks a boundary's implied row.
  - The JS target reports its axis-varying tokens; folded ones aren't properties there.
- **no-tells (§7):** a token name is not a literal. A relative colour isn't a tell when its
  origin is a token, a role or an expression over them and its channels are its keywords;
  `rgb(from brand 255 0 0)` still is.
- **Docs (§6):** a Tamber section in `docs/contract-for-agents.md`, `docs/recipes/tamber.md`, and
  `docs/start-here.md`'s line: "a brand is an `app.tamber`; a colour names a role or one of
  its tokens". The "colour or size" wording waits for stage 6's size tokens. A guide's
  `tamber` fence is checked, by the crate only.
- **Left open:** `contract tamber` doesn't resolve a profile colour (`color(--…)`), which
  needs the app's Contract declarations; `contract build` checks it.

### Stage 2's gate: met (2026-10-14; the Bluesky clone at `7166baf` on exact2 `774575681`, build b40)

The evidence is in `~/.tuft/projects/bluesky-clone-builds/b40/gate/SUMMARY.md`; the port is
bsky5's.

| Gate line (§5 stage 2) | Result |
|---|---|
| The clone's colour literals become names | **Met.** 1,808 replaced (1,587 pairs, 176 hex, 31 `rgba`, 14 palette reads); 0 left outside `app.tamber` (no-tells and grep). |
| Light, Dim and Black from the root, no palette threaded | **Met.** `setMode("dark-style", …)` from Appearance and at launch; the root-provided palette and 213 `inject pal` lines are gone. The design has 70 tokens and the axis `dark-style = dim \| black under scheme=dark`, Dim being Bluesky's default. |
| Every compared screen identical on the web, iOS and the Mac in all three | **Met.** Web: 45 captures pixel-identical apart from live data. iOS: SSIM 0.997–0.999 on thread, profile, settings and notifications, differing only in the status-bar clock and live counts; headers and bars 1.000. Mac: 1.000 on appearance, settings and the home header (0.999994 for Black's appearance). |
| Three previews, a Black pin inside Dim with inherited text, a token naming the accent under a child `accent-color` | **Met by fixture**, since Bluesky has none of these screens: `apps/theme-fixture` on iOS and the Mac (commit 5) and conformance `tamber-modes` on both web targets (commit 6). In the clone itself, the video feed's Black pin inside Dim measures (0,0,0) over the page's (21,29,40). |
| A Dim switch on a 1,000-row list does not relayout | **Met.** The test of `44a19d999`; in the clone, a Dim → Black switch on Home journals no node added or removed and no resize. |
| Expose and the readers unchanged; an app without a design unchanged, with no design payload | **Met.** Stage 1's ports (22/22 and 104/104) and every unthemed app's tests. |
| `tamber-dial-inert` live; golden IR and folded tables | **Met** (commit 1). |
| The clone's design adds under 4 KB to its plan | **Met:** the plan shrank by 19,116 bytes (3,888,684 → 3,869,568); `app.tamber` is 6,881 bytes. |
| First frame unchanged within noise | **Met.** Cold launch on the iOS simulator in a quiet window, the pre-port `e61eaf7` against b40 on the same exact2, 4 interleaved blocks of 3 runs (n = 12 each). Process start to first post: median 1722 ms against 1676 ms; runtime zero to first post: 594 ms against 568 ms. The −46 ms difference is inside a spread of about 200 ms (`…/b40/gate/firstframe/`). |
| `setRootFontSize("125%")`: the font-size setting scales Dynamic Type at two sizes | **Met.** The clone's Smaller and Larger use `93.75%` and `106.25%`. |

**One gap the port found**, fixed as a stage 2 follow-up: deliberately one-colour tokens
(white on a primary fill, media grounds) drew a `lower-colour-one-scheme` warning at each of
168 uses. A token line now takes `scheme=fixed`, and an unmarked one-scheme token is reported
once, at its declaration (see that follow-up's note).

**Stage 2 is closed (2026-10-14).** Every gate line is met. The `scheme=fixed` follow-up is a refinement, not a gate item.

### Stage 2 follow-up: `scheme=fixed` (`70c39ffec`..`f3b11e0a6`; core `73edf648a`, opaque-only `f3b11e0a6`)

The Bluesky port found that deliberately one-colour tokens (white on a primary fill, media
grounds) drew a `lower-colour-one-scheme` warning at every use: 168 on the clone.
- **A token line takes metadata after its value, as a dial's does:**
  `white_100 = #ffffff  scheme=fixed` (D3 and §3 are amended). `tamber-scheme-fixed` refuses
  the mark where the folded light and dark sides differ in any tuple, on a role or a platform
  colour, with any word other than `fixed`, or said twice. The IR carries `"scheme":
  "fixed"`.
- **The lint no longer warns at rows that name a token.** A token is reported once, at its
  declaration, when its folded value is one opaque colour in light and dark in some tuple,
  unless it is marked or is a whole alias of a marked token (a role's spellings count as one
  name).
  - A translucent single colour is not reported; an overlay works in both schemes.
  - A role override replacing a page role is reported the same way.
  - The message offers a `light-dark()` pair, or the mark. A token that is one colour only in
    some modes is offered the pair alone.
  - An inline theme can't carry the mark, so its message says to move the token to
    `app.tamber`.
- **Literals in `.contract` rows are unchanged:** one warning each.
- **Measured:** the clone went from 168 warnings to 9: eight token lines (`white_100`,
  `black_100`, `negative`, `over_limit`, `like`, `attie`, `mute_banner`, `contrast`) for the
  port to mark where deliberate, plus the closing summary.
- **Known:**
  - an empty `scheme=` is a CSS value error rather than `tamber-scheme-fixed`;
  - a row's own inline expression over a fixed token isn't folded by the lint;
  - with both a design and an inline theme (already refused as `tamber-two-sources`), the
    warning reads the inline entries.

### Stage 3, commit 1: `tamber fmt` (`0804b40ea`..`9853ec7dc`)

- **One canonical layout:**
  - block order kept;
  - two spaces per level (entries under a block, `when` lines under a token);
  - `name = value`, `name: kind = value`, dials with `in low..high` and metadata after two
    spaces, axes `a | b  under scheme=dark`, `when a=b c=d: value`;
  - one blank line between the header and each block;
  - each value's text exact.
- **Comments.** A trailing comment sits two spaces after its line. A comment line keeps its
  level, so removing the line after it moves nothing else.
- **`fmt` reparses what it prints** and refuses (`tamber-fmt`) rather than write a broken
  design. `contract tamber fmt [--check]` and `bun exact.mjs tamber fmt`.
- **Grammar settled along the way:**
  - single-quoted strings open only at a token start, so an apostrophe in a word opens
    nothing;
  - `when = value` at entry indent is a token named `when`;
  - an axis's `under` clause is the last `under` with no `|` after it.
- **Gate:** every corpus design, the theme fixture, the modes conformance design and the
  Bluesky clone's design reach a fixed point in one step, with the same IR and every comment.

### Stage 3, commit 2: edit operations and inverses (`43f74174b`..`423153516`)

- **The operations:** `set-dial`, `set-token` (the default, or one `when`, added if absent),
  `unset-token-when`, `add-token`, `rename-token`, `remove-token`, `add-axis-value`,
  `remove-axis-value`. Each splices only the lines it touches, in `fmt`'s layout, so a
  formatted design gets a one-line diff; removing a token with its `when`s removes those
  lines. An edit the checker refuses is refused with its diagnostics.
- **The inverse is a `restore`**, not a sequence of semantic operations. It carries exactly
  the changed lines as they were: comments, blank lines, CR line endings, a block an add
  created, the final newline. Re-serialised inverses could not give the bytes back, as LLP
  1122 r4's reviews found.
  - `undo` puts back the preimage even when that design did not check.
  - A restore is never a client write. The server accepts it only as an inverse it issued for
    the file's current digest.
- **Rename** changes token names in values and exports only, never in `when` conditions,
  metadata, hex digits, function names or strings. It is proved by the design checking before
  and after with every folded value unchanged, and it refuses names that colour syntax reads
  (channels, spaces, hue methods). A role override keeps its name.
- **Fields are validated by kind:** a value stands alone; metadata is one `name=value` word;
  names, axis values and conditions are identifiers; a `when` names each axis once. On the
  wire, fields are named and operations are kebab-case.

### Stage 3, commit 3: the session core (`a48085ada`..`2608a642c`)

`contract tamber serve --stdio <app>`, with the logic in `tamber/src/session.rs`.
- **One writer:** `.exact/tamber.lock`. Its device and inode are pinned and re-checked on
  every commit.
- **A commit:**
  - temp files are created with `create_new` and `O_NOFOLLOW` and fsynced;
  - a journal is written and renamed into place, each entry `{tmp, file, digest, before}`;
  - each target is re-read, and a file that moved withdraws the journal and returns a
    conflict;
  - then the renames, directory fsyncs throughout, and the journal removed.
  - A failure part way completes or the server exits.
- **Recovery** judges every entry before any rename and renames a temp only over its `before`
  text. A set none of whose files landed is withdrawn when a file changed since; otherwise the
  journal is kept and the server refuses to start.
- **Undo:** a client sends only an inverse id this run issued (the last 256 kept). Its
  precondition is each restored file's postimage digest, and a client-sent `restore` is
  refused.
- **Reads:**
  - `resolve` labels residuals and gives their fallback;
  - `explain` gives the `when` values and the checker's resolved names;
  - `describe`'s metadata is keyed by IR names;
  - `propose` checks the digests it is given and returns its baseline.
- **Limits:**
  - **live or rebuild is judged from the design's shape only.** Stage 4 must extend it through
    Contract's synthetic consumers before the live table relies on it, and stage 6 to kinds
    substituted at build.
  - **An editor that ignores the lock** and saves between the final read and the rename is
    overwritten (documented).
  - **The server runs on macOS and Linux only.**

### Stage 3, commit 4: the wire (`37f11c1ee`..`95733f1cf`, the async-lane line `fd7db6fd2`)

`scripts/tamber-server.mjs` talks to `contract tamber serve --stdio` and writes no design file
itself.
- **Where it runs:** both dev loops mount it when the app has a design; the resident
  `--serve-as` loop does not (stage 4 revisits that for the build channel). `bun exact.mjs
  tamber serve <app>` runs it alone, and `tamber op <url> <json> | subscribe` is the scripted
  client.
- **The token:**
  - `GET /__tamber/token` answers only under `access.local`; anything else gets the
    installer's not-found;
  - every POST carries `x-exact-tamber-token`;
  - `op` follows no redirects.
- **Requests:** `POST /__tamber`. A write must carry `client`, since clients share one server.
- **The change stream:** `POST /__tamber/subscribe`, server-sent events. Each event carries
  `seq`, `epoch`, the files and tokens changed, and the `client` (null for a hand edit or a
  checkout).
  - A write is announced once, from its reply. A hand edit made just before a write is
    announced first, separately.
  - Files that appear or disappear are announced.
- **The long poll:** `POST /__tamber/events {after, epoch}`. **This is how a JS-target data
  module, and so LLP 1122's tool, follows changes**: `fetch` there reads a whole body, and
  `exactStream` coalesces a server-sent stream (LLP 1016 D4). An `after` from another run, or
  older than the 256 events kept, is answered `missed` with the current files.
- **Transport, against D13's text:** POST plus a streamed response plus the long poll; no
  WebSocket.

### Stage 3, commit 5: `commit`, held proposals, attribution (`f63b7c658`..`f224c16ec`)

LLP 1122 D9 and D10, on the server.
- **`commit`** (`scripts/tamber-commit.mjs`):
  - **Baseline:** before the session first writes a file, it records HEAD, the file's index
    entry and the working tree against HEAD (content and mode). The write must have read
    HEAD's text.
  - **Refusals,** each naming the file and why: dirty or staged at the baseline; HEAD moved;
    any change the server did not make, even one a later write overwrote; the file differing
    from the server's last write.
  - **The commit itself:** a temporary index of HEAD plus exactly those files, then
    commit-tree. The real index is updated for those paths only, in one `--index-info`.
    `update-ref` is guarded on the checked HEAD, and the index is restored if it fails.
  - It never pushes, and it refuses while a write is in flight.
- **Held proposals:**
  - `propose {hold: true}` keeps a proposal for 15 minutes (64 at most) with its base
    digests, announced with its operations by index.
  - `accept` applies it against those bases. A moved file or a partial accept is recomputed
    as a new proposal (`replaces`/`replacedBy`) for a fresh accept.
  - `drop` forgets one.
  - Every end is announced. The open list rides `subscribed` and each long poll.
- **Attribution: every write carries its `client`**, not a hello, since clients share one
  server. LLP 1122 D10 is amended accordingly. An accepted write is the proposer's, with
  `acceptedBy`. A conflict names, per stale file, the client that last changed it, or
  "changed on disk".
- **Not yet watched:** `.contract` files, since nothing writes them before `set-row` (LLP 1122
  T5). That stage adds them to the watcher and to `commit`'s guard.

### Stage 3, commit 6: the import frame and shadcn/ui (`906c448a6`..`a05ba6d28`; the `AccentColor` follow-up `adb6808e8`..`93f49a3a8`)

- **The frame** (`tamber/src/import`): an import is
  - a transaction (`add-token`, `set-token`, a new `add-axis`);
  - role overrides offered apart;
  - an unmapped report;
  - a count check that refuses a mismatch (`tamber-import-count`).

  It is planned against the design as it stands, so re-importing an unchanged source
  proposes nothing. Reconciling takes back only `when` lines on the importer's own axes; a
  `when` on another axis is the author's.
- **shadcn:**
  - **Sources read:** `:root` and `.dark` in v4 `oklch()` or v3 HSL channels; a `:root`
    inside `@media (prefers-color-scheme: dark)` as the dark side; selector lists, nesting and
    `[data-theme=x]`.
  - **Theme classes:** a further theme class becomes an axis `theme = default | x`, with
    CSS's fallback for a missing side.
  - **References:** a `var()` that is a colour where it's read stays a reference; a reference
    cycle is written out as colours.
  - **Reported, never dropped:** `--radius` and its derived sizes as pending stage 6;
    restricted selectors; other media; collisions; superseded and empty values;
    `currentColor`.
  - **Refused whole:** an axis past 64 tuples.
- **Offers,** each off until ticked: `-exact-background`, `-exact-label`,
  `-exact-secondary-background`, `-exact-separator`, and **`AccentColor` for the accent**. That
  last is a role override in the design, which every host makes the app's tint (stage 1's
  rule), not a Contract `accent-color` edit (follow-up `adb6808e8`..`93f49a3a8`). D14's shadcn
  table is amended to match.
- **A role override can't vary by an app axis** (`tamber-role-axis`, D5). So a role offer that
  an imported theme class would make vary is offered with no operations and a note citing D5.
  An existing design role the import would make vary is named the same way, and no removal is
  offered, since removing a role can break expressions over it. A Contract-level test compiles
  each fixture's whole transaction with every offer accepted.
- **On the wire:** `contract tamber import shadcn <css>` prints the proposal; `bun exact.mjs
  tamber import shadcn <css> <url> --app <app>` holds it, with a `review` for the tool.
- **Gate fixtures** (pinned, `tamber/tests/import/README.md`): shadcn's Neutral theme (v4,
  47 = 46 mapped + 1 reported), the Blue theme (v3, 39 = 38 + 1), and Neutral with the Blue
  class making the axis. Each applies to a design that checks and re-imports to nothing.

### Stage 3, commit 7: Tailwind v4 `@theme` (`67982770c`..`3b5185888`)

- **The CSS importers share an engine** (`tamber/src/import/engine.rs`).
  - A variable that is no colour where it's set makes no token.
  - A value naming a dropped token is dropped and reported, so a proposal never names a missing
    token.
  - `none` components are written as the zero CSS reads them as.
- **The theme is built in Tailwind's own `Theme.add` order.**
  - Namespace resets follow Tailwind's rules and also clear pending rows.
  - A default key yields to a value the key already has.
  - A media query around `@theme` doesn't condition it; a rule inside `@theme` is reported.
- **The stylesheet's own blocks count only when whole:** the root, and its dark side (`.dark`,
  `[data-theme=dark]`, the dark media query, `@variant dark`, a whole-block `@custom-variant
  dark`). Narrower selectors are reported.
  - A root declaration sets both sides unless a dark declaration holds the dark side.
  - `initial` outside `@theme` invalidates its side.
- **Backing variables** (`:root { --ink }`, or a name in no namespace) make no token; a key that
  names one reads its colour on each side. So `@theme inline { --color-ink: var(--ink) }`
  gives a `light-dark()`. An inline key's redefinition, and everything reading it, is
  reported.
- **Mapping:**
  - colours become `tw.<name>[.s<step>]`; only a numeric tail is a step, so a named step stays
    a whole name;
  - spacing, radius, fonts, type and its sub-properties, weights, leading, tracking (a length,
    in em), the three shadow kinds and easings are pending stage 6, each with its token;
  - breakpoints, containers, blur, drop shadows, perspective, aspect, animations and
    `--default-*` are reported.
  - Selector specificity beyond whole root and dark blocks is not modelled.
- **Gate fixtures:** Tailwind v4's default `theme.css` (tailwindcss 4.3.3), 419 = 288 mapped +
  131 reported; a project sheet whose colours map to `light-dark()`. Both re-import to
  nothing.

### Stage 3, commit 8: DTCG, Tokens Studio, Figma Variables and the Resolver (`ad2fd0cb5`..`5033045a0`)

Each source is read as layers that are active in some modes.
- **Figma** `<Collection>.<Mode>.json`: Light/Dark is the scheme, one mode is the base, and
  other modes are an axis named for the collection, its first mode the default.
- **Tokens Studio:**
  - each set is active in the modes of every theme that enables it, with `tokenSetOrder`
    deciding precedence;
  - a one-theme group is the base, a light/dark group is the scheme, and any other group is an
    axis;
  - sets no theme enables, and `modify` values, are reported.
- **The Resolver:** references resolve in their own document, and sibling overrides and nested
  sets apply. A light/dark modifier is the scheme; any other is an axis, default first.
- **Resolution:**
  - each mode tuple is resolved on its own, with aliases followed there;
  - a token must be a colour in every tuple or it makes no token and is reported, since nothing
    is invented for a missing mode;
  - the default tuple gives the `light-dark()` default, and `when` lines are written only where
    needed;
  - an alias to an unmade token, or round a cycle, is written as its colour.
- **Format:** `$root` becomes `root`. `$extends` merges deeply (a cycle refuses the import).
  A colour object is read by its space first.
- **Refused whole:** axis names or values that collide after sanitising, host traits as axes,
  an existing axis with another default, and anything past 64 tuples.
- **Gate fixtures** (pinned): Tokens Studio's lion example, 49 = 44 + 5, brand and colour groups
  as axes; Figma's variables example, 104 = 40 + 64, Light/Dark plus a Brutal/Modern axis; a
  Resolver document. Each re-imports to nothing.
- **Known:**
  - a gradient's reference stops are reported, not made into tokens (D14 named this; later);
  - re-importing our own export waits for stage 5's export;
  - two themes with the same name, or a nameless theme, in one group of several were refused afterwards (`451015da5`).

### Stage 3, commit 9: Astryx and the token-kinds probe (`cf25fb322`..`af1c5454e`; Astryx's `AccentColor` offer `319904b77`..`04f6875c9`)

- **How it reads Astryx:** `scripts/astryx-tokens.mjs` resolves a pinned Astryx theme with its
  core's `resolveThemeTokens()`, installing packages in a temporary directory, never the repo.
  `tamber/src/import/astryx.rs` maps the result by name: surfaces, text, icon, border, overlay,
  syntax and data colours to their groups; the accent and status colours whole. The body
  surface is offered as `-exact-background`, and the accent as `AccentColor`.
- **Gate fixture:** `@astryxdesign/theme-neutral@0.7.0`, 258 = 150 mapped + 108 reported. The
  design checks after applying, and re-importing proposes nothing.
- **The kinds probe**, the unmapped report as evidence for later kinds (exact1's Proposal I,
  here):
  - **duration tiers:** fast, medium and slow each have a min and a max, and Tamber has one
    value per duration token (§9);
  - **text styles:** each wants one compound `type` token, already stage 6's;
  - **shadows with scheme-varying colours** (low, medium and high, and inset rings), so stage
    6's `box-shadow` kind must allow a `light-dark()` colour inside;
  - **a line style** (`--focus-outline-style: solid`), which no kind names and which stays
    out.

**Also from the importers' review** (`319904b77`..`04f6875c9`): a changed value can make an
existing role override vary by axis even when an import adds no `when`. Two cases: a kept
`when` that now folds to the new default, and a role chained to a rebound accent. A shared
pass (`split_roles`) names such roles with no operations and a note quoting
`tamber-role-axis`. A rebinding that would leave a dial unused (`tamber-dial-inert`) is
withheld, with a note.

### Stage 3's gate: met (2026-10-15; the run is `~/x2theme/stage3-gate-run.txt`)

Run with the scripted client, `bun exact.mjs tamber op`, against `tamber serve`, as the
brief adapted LLP 1121 §5's lines.

| Gate line | Result |
|---|---|
| Design edits through `tamber/1`, each a one-line diff | **Met** on the Bluesky clone's design (`set-token`, and a `when` set). The clone has no dials, so `set-dial` was run on §3's example design. |
| Two clients writing the same token conflict | **Met.** `conflict: true`, `by` naming the first client, the file unchanged. |
| A second server for the project refuses | **Met.** Its standalone wire stays up and answers with the lock error rather than exiting. |
| A transaction killed between renames completes on restart | **Met**, by the server's fault-injection test, not by the scripted client. |
| A hand edit and a `git checkout` reach a subscriber | **Met:** `changed` events with `client: null`. |
| `propose` changes nothing on disk | **Met.** |
| Requests without the token refused; `--lan` and forwarded requests can't obtain it | **Met:** 404 for each. |
| Each inverse undoes | **Met.** `unset-token-when` restores the default (standing in for stage 7's `unset-param`); undoing every write, and the dial edit, restores the file byte for byte. |
| The four importers | **Met.** shadcn neutral 47 = 46 + 1 and blue 39 = 38 + 1 (and through the wire as a held proposal with its five offers); Tailwind's default theme 419 = 288 + 131 and a project sheet 10 = 6 + 4; Tokens Studio 49 = 44 + 5; Figma 104 = 40 + 64; Astryx neutral 268 = 160 + 108. Re-importing proposes nothing (32 import tests). |

**Stage 3 is closed.** Its server additions for LLP 1122 T1 and T1a are in place, so the
design tool's first stages can start. The one item owed outside the gate,
refusing nameless or repeated themes in one DTCG group, landed in `451015da5`.

(Also: after `scheme=fixed`, the Bluesky clone marked 12 tokens and builds with 0
`lower-colour-one-scheme` warnings, down from 168: b41, clone `b8a7dc8`.)

### The protocol's reference, `docs/tamber-protocol.md` (2026-10-15)

D13 named this reference, and stage 3 didn't write it. It now describes `tamber/1.1` as built:
- the transport (POST, the streamed response and the long poll; no WebSocket);
- the token;
- every request and its reply;
- the operations;
- the change stream's events;
- what the later stages add.

Each stage that changes the wire changes this page in the same commit. Stage 4 adds the live
table and the build channel (LLP 1122 D4).

### Stage 4, commit 1: reclaimable `Wide` storage (`5624bb911`, reviews `3e06b2776`)

LLP 1121 §5 stage 4 and D13 said "reference-counted". **It is built as mark and sweep instead.**
`ColorValue` is `Copy`, and `Wide` ids are copied into rows, composite rows, transitions,
keyframes and the runner's token values, so a count would have meant un-`Copy`ing it
everywhere.
- **Ids carry a generation.** Each u16 holds the slot in its low 12 bits (the cap is 4,096) and
  the slot's generation in the top 4. A stale id reads as nothing, never as another colour. In a
  debug build a stale read asserts and writes a journal line, since it means a missed holder.
  Generations wrap after 16 sweeps of one slot, which is acceptable only because of that
  assertion.
- **Holders publish marks.** Only kernels and runners hold ids.
  - **A kernel** publishes a 4,096-bit mark at the end of each commit that changed it, from the
    shared style table's cached union. It costs 0.16 µs, and nothing while the table is empty.
    Measured on a 1,000-row P3 list, the Dim switch, a one-text commit and a window shift are all
    within noise of main.
  - **A `Keep`** stands beside anything else that keeps ids across a commit: role overrides, a
    runner's saved overrides and queued reorders, its kernel-free document, a decoded export.
  - **An idle holder** answers from its last mark, so it never holds a sweep up.
  - **A painting host's accent scope** is not a holder. It lasts only within a paint of a
    borrowed kernel, whose mark covers it.
- **Ids in flight** stand in for theme1's "ids minted after the sweep's request are kept",
  because the sweep runs at once in `intern`, from the marks already published.
  - A kernel thread's batch protects everything interned since its last publish, up to `SPAN` =
    3,072 interns. Export decode and the document fold open a `wide::Batch` explicitly.
  - Anything else is protected by recency: nothing interned within the last `GRACE` = 1,024 is
    freed.
  - An id in flight past both is a missed holder. **Invariant:** outside an open batch, no path
    holds an id unmarked across more than `GRACE` interns.
- **The sweep** runs when fewer than a quarter of the slots are free. With no holder registered
  (a compiler, a tool), nothing is swept.
- **A full table** of held colours refuses the new colour: the row keeps its value, and the
  refusal is counted (`wide::refusals()`, for commit 4's `unsupported` report) and journaled.
  A refusal ages the table as an intern does.
- **No host holds an id.** Apple and the web take text. The Linux host converts what it retains
  at capture:
  - svg scenes become 8-bit colours or `light-dark()` pairs;
  - paragraph sources keep both appearances resolved;
  - lookahead scratch ops drop their colours;
  - gradients keep their stops' values (`wide::Values`), read through `placed_with` and
    `resolved_with`, rather than 8-bit stops, which would move a polar ramp's hue.

### Stage 4, commit 2: live or rebuild by the whole closure (`bfa479e88`..`a99331d5f`, reviews `c180f8c04`, reference `687419133`)

- **`propose` and transactions now re-fold the app's synthetic tokens** with the design. The
  compiler records every candidate, and whether a colour row writes it
  (`contract_lower::synthetic_candidates`); the server caches that list per Contract sources,
  import resolution and design token names.
- **An edit is `live`** when the shape is unchanged, no role override is reached, and the same
  synthetic tokens fold on both sides. The synthetic tokens whose values change are returned
  per tuple (`synthetic`) for the batch.
- **An edit is `rebuild`** when it changes a role override or anything one reaches (hosts take
  role tables at boot), gains or loses a synthetic token, leaves a strict one unfoldable,
  involves a profile colour, has an unknown closure (the app's build stops before its design),
  or changes structure.
- `propose`'s answer is now the closure's, not the design's shape alone.

### Stage 4, commits 3 and 4: the live table (`ad6c46d1b`, review rounds `f133acb13`..`e27941bf9`, records `a7483ba86`)

- **The plan's structure** (`Plan::structure`) is its canonical bytes without the design's
  values. Role-override and profile-colour values stay in it, because hosts take them at boot,
  and so do the axes, since an axis change renumbers the tuples a batch's slices index (the
  brief had excluded the axes). The builder interns design values last, so a value-only edit
  moves no other string's id.
- **No plan is published** when only `.tamber` files changed and the structure is unchanged. This
  is decided by content in the wasm loop's `Session`, in the TypeScript producer and in the JS
  loop, so a hand edit and a server write are one path (the server's writes are not skipped
  in-process). The wasm loop still writes the plan file, so a host watching it
  (`EXACT_DEV_PLAN`) restarts as before; only builds connected to the dev URL are live. A
  Rust-data app's wasm loop says `live: false`.
- **The batch** is `{kind: "tamber-batch", structure, base, design, tokens: [{name, value,
  slices}]}`, where a synthetic token is named by its expression.
  - A snapshot (`GET /__dev/design`) carries every token.
  - A notice (no tokens) goes out on connect, and in place of a batch over 32 KiB, which native
    event frames can't carry.
  - A stale base, or a notice for another design, fetches the snapshot.
- **`Runner::apply_live`** sets every changed token in one commit with no layout, and refuses in a
  production build (`admit_live`). It skips role overrides. A colour refused by a full wide table
  keeps its old value and is reported as `unsupported`.
- **The hosts:** `exact_design_rows` on the web and Apple ABIs, and `Presenter::apply_live` on
  Linux (by polling, beside its envelope). Apple re-fetches the snapshot after every generation
  it applies.
- **The JS target's development build** makes every non-role token a custom property, and
  `modes.js` rewrites each tuple's rule in one style update. Its release build was compared
  byte for byte with the build before, on theme-fixture and the tamber-modes conformance app. The
  generated JS and CSS (`.gen/app.css`, `.gen/app.js`, `paint.js`, `names.js`, `pages.json`) are
  identical, and the plans differ only in string pool order, so the plan digest moves. The one
  code difference is `.gen/modes.js`: it carries the live-table runtime, inert in release because
  no development design is passed. `8a4f25b53` leaves it out of release builds.
- **The server:** `hello.live`, writes held until answered (never a 504 and then applied), and a
  held propose checks the `requires` its client names.

### Stage 4, commit 5: the build channel, LLP 1122 D4 (`e44ee2f97`..`d55792423`, records `2689b877e`)

- **Registration.** Each dev stream's hello carries a nonce (128 random bits). A build answers
  with one `POST /__dev/report` carrying its stable `id`, host, capabilities, and plan and design
  digests.
  - The id is per tab (in `sessionStorage`) on the web, and per process session on Apple and
    Linux. A duplicated tab whose id is already open takes a fresh one.
  - A report without a live nonce is refused.
  - `hello` lists the open builds.
- **Reports up:** `status` (the effective scheme, contrast, modes and root font size, the digests,
  the pin revision), `unsupported` and `selected` (its route only, until T2b). Each is an event to
  the server's clients. Typed, bounded fields; at most 256 streams.
- **Commands down:** `pin` (scheme, contrast, a mode, the root font size, or none; each pin
  replaces the whole set). `preview build axis=value` is pin's spelling for one build and one axis.
  `inspect`, `outline` and `hold` are reserved and answered `unsupported`. Every command is a frame
  a host dispatches before any generation handling.
- **Ownership and presence.** A pin belongs to its client while that client is present: a
  subscription naming it, or a long poll naming it that is open or was answered within 35 s. (The
  design tool can only long-poll.) A pin from a client that isn't present is 409. When a client
  stops being present, its pins are handed back on every build. A build that registers again
  under its id gets its pin, or none, replayed. Pins are journaled with their client.
- **One source of batches.** The loop that mounts the design server makes every batch and
  snapshot. Batches name the design's **shape** (axes, and each token's name, role, role value and
  slice count) rather than the plan structure, so the JS target and a native build of the same
  sources agree. The plan structure stays the dev loop's own value-only test.
  - The JS loop's resident `--serve-as` producer mounts no server and makes no batches. It
    relays the main loop's commands and batches onto its devices' streams, which take their
    nonces from the main loop.
- **Linux has no stream.** It polls `GET /__dev/hello`, then `/__dev/commands?nonce&ack`
  (acknowledged, so a lost answer is resent) beside its envelope, and it is disconnected after 5 s
  without a poll.
- **The web pin** is the system layer beneath the app's `setScheme`: a `:root` rule plus
  `exact.systemScheme`. Status reports the scheme that actually paints, and the app's own
  `setScheme`, `setMode` and `setRootFontSize` send a status. A contrast pin is answered
  `unsupported`, since a page can't override the browser's `prefers-contrast`.
- **Release builds.** The native hosts run no channel under a production compat; that is a runtime
  gate, not compiled out. The release JS target's `modes.js` still carried the inert live-table
  runtime here; `8a4f25b53` strips it (`// dev-only` regions, dropped from a build without `--dev`), with a test.
- **`tamber/1.2`**, with `docs/tamber-protocol.md` listing what 1.2 added.

### D3's `pair=` (`ae6dc3d78`, `cca68e081`; built by LLP 1122's implementer)

- `pair=<token>[,…]` names colour tokens or roles a token is drawn on. Naming itself, nothing of
  the design, or an empty name is refused (`tamber-pair`).
- **`tamber-contrast`** warns once per pair, at its lowest tuple and scheme below 4.5:1, naming the
  tuple, the scheme and the ratio.
  - The ratio is cut to two places, never rounded up past 4.5.
  - A residual side is read at its fallback, and the warning says so.
  - A background that isn't opaque is passed over, since it has no single ratio.
  - It is a warning, never a refusal.
- **A rename** rewrites the name inside `pair=` lists, and undo restores the bytes.
- `describe` gives each token's `pairs` and the design's `warnings`, and the design tool badges
  each declared pair's ratio per cell.
- **Importers** proposing pairs (shadcn's `x-foreground`, Astryx's `on-`) are left for later.

### Stage 4, commit 6: `uses`, `resolve` and scrub sessions (`ddaa887e1`..`6d904f998`, records `01ed90b6f`, wording `393a1e9c0`)

- **`uses`** is answered statically, from the plan compiled from the sources and its source map.
  - Each site is `{site, row, file, line, col, end_col, component, via?}`, where `via` names a
    token, a synthetic token, a slot or a derive the read goes through.
  - Only colour rows count. A value that is only compared, and an `#id` reference, is no read.
  - The JS target's plan and a native bake of the same sources give the same sites (tested), so
    one answer serves both of the JS loop's plans.
  - There is no mounted-instance count; that needs a report the channel doesn't carry (a §9
    candidate).
  - The clone's `primary_500` has 587 sites, all direct.
- **`resolve`** is unchanged: the stored form or residual with its fallback, labelled.
  `resolve-on` comes with LLP 1122 T2b, from a build's `selected` report.
- **Scrub sessions** (`scrub`, `scrub-release`, `scrub-cancel`, the `scrub` event):
  - Each update is planned on the scrub's opening files (`propose` with `texts`). The dev loop
    that mounts the wire compiles the texts as an in-memory overlay of the design, through its
    own compiler for each target, because the JS target's CSS and profile colours need the real
    compile. It sends every build the batch from its running design, and JS-loop natives get it
    by relay.
  - Updates are coalesced: one preview in flight, and the latest wins.
  - Opening needs write scope and a present client. A second scrub is 409, naming the opener. A
    rebuild-class edit has no scrub (409).
  - Release writes once against the opening digests; a conflict names who moved the file and
    restores.
  - A scrub ends without a write on a cancel, any other write or a hand edit (which ends it
    before it lands), its client no longer present, the server's loss, or 30 s with no update.
    Each end sends every build the latest committed design. A value-only build landing while a
    preview still shows is only an ordering case, and it changes only what the end restores.
- `scopeOf()` grants write to the local token, until pairing (stage 10).

**Stage 4's commits are complete.** The gate run follows.

### Stage 4's gate: met (2026-10-16; the run is `~/x2theme/stage4-gate-run.txt`)

The run was on the Bluesky clone (the v2 clone, TypeScript and Rust data, at `de5128c`) and exact2
`393a1e9c0` plus `82a0ca14d`. It covered iOS (its own simulator), the Mac and the web, on the JS
loop with its resident producer and on the `--wasm` loop. The lines are the brief's, as LLP 1122
amended §5's: `resolve-on` moved to LLP 1122 T2b, and `preview` is the build channel's `pin`.

| Gate line | Result |
|---|---|
| A dial or token change through `tamber/1`, and a hand edit, reach the clone at once on iOS, the Mac and the web, with no plan, no reload and no state lost | **Met** on both loops: a batch with no generation, plan digests unchanged, a scrolled feed and a typed draft kept. About 1 s on the wasm loop and about 10 s on the JS loop (its JS build). |
| A structure edit still reloads | **Met.** A Contract edit is a new generation that carries state. |
| A token read by a Contract synthetic token updates live, synthetic token included | **Met.** `color-mix(in srgb, primary_500 60%, black)` on Create account is `live` with its synthetic value, and repaints with no reload. |
| A role-override edit is `rebuild` and reloads | **Met.** An `AccentColor` override set is `rebuild`; the natives reload and the wasm loop makes a generation. |
| A build that reconnects after missing a batch gets the snapshot | **Met.** An app relaunched on its old generation after an edit took the snapshot's design. |
| More than 4,096 distinct colour edits leave storage bounded; a full table refuses with a diagnostic | **Met by the kernel tests** (6,000 colours set and replaced, a full table refusing and recovering, batches, grace, stale reads). Not driven live on the clone. |
| `pin` changes one named build, and the server reports its effective mode | **Met.** Dark plus `dark-style=black` on the iOS build only, and its status reports both. |
| `uses` is right for the clone's `primary_500`, through synthetic tokens | **Met.** 587 sites in 112 spans, all colour rows, and the synthetic row reported with `via`. |
| The JS target's development build changes a non-varying token live, and its release build is unchanged | **Met on theme-fixture** (live by hand and by commit 6's scrub test; release bytes in `ad6c46d1b`, release `modes.js` in `8a4f25b53`). The clone's own JS-target dev page never settles in headless Chrome. That was already so at `8f1e88689`, before commits 3–6, and is queued. |
| All of it works with the JS loop and its resident `--serve-as` producer | **Met** for native builds (each line above). The JS loop's own page: see the line above. |

**Scrubs on the clone:**
- JS loop, iOS: previews about 2.2 s each, the file untouched, and cancel restores.
- wasm loop, iOS and the Mac: previews about 0.5 s, and release wrote `app.tamber` once.

**Found and fixed during the run (`82a0ca14d`).** On a TypeScript app, the wasm loop's module
watcher ignored `.tamber` files, so no design edit reached the producer. Once it did, the design,
being a Rust build input, also rebuilt the wasm and reloaded the page. A design edit is now the
TypeScript producer's alone.

**Queued** (not gate lines):
- the clone's JS-target dev page busy-loops in headless Chrome;
- an Apple re-registration reports `plan` null;
- an iOS build doesn't reconnect by itself after the dev loop restarts;
- the JS loop takes about 10 s per value edit, against about 1 s on the wasm loop.

**Stage 4 is closed.** LLP 1122 T2 is under way on it.

### After stage 4's gate: the JS loop's speed and iOS reconnecting (`c5bbaf311`..`328b26e78`; `90e778787`..`b260e1486`)

- **The JS loop's value edit, about 10.5 s → about 0.4 s** on the clone.
  - A design-only edit to the app's own design now reads the edited design straight from the
    compiler (`exact-web-js design <app.contract> -`) and sends the batch at once. It qualifies only
    while the design's shape, the plan structure and the accent flags hold, and the reader is
    fresh against its depfile.
  - The full development build follows in the background as a check, and reloads or shows an
    error exactly when the old path would, one build later.
  - Scrub previews use the same reader (they took about 2.2 s each before).
- **An Apple host whose native code the server rebuilt stays on the live table.** A restarted loop
  that rebuilt the native code used to make `PlanURL` terminal, which cancelled its stream and
  dropped it off the build channel.
  - It now refuses that program's generations and still says "rebuild and relaunch", but it keeps
    its stream, stays registered, applies value batches, reconnects across restarts, and takes
    the next generation of its own program.
  - A stub-server test (`ddd0c1dbb`) covers a dropped stream and a restarted server
    (re-subscribed and re-registered under the same id, the held pin re-asserted), and a
    rebuilt-program frame keeping the host registered.

### Stage 5, commit 1: per-session role tables (`de05c0b2b`..`2ccbc448a`)

Stage 1's known limit, "overrides are per thread, not per session", is lifted.
- **Each runner owns its role overrides** (`roles::Roles`), and the wide colours they hold.
- **Hosts bind their session** around every call that resolves a role. Apple binds per call, a
  host's boot binds its new runner, and Android hands the roles from the boot thread to the
  running one. A resolve with no bind falls back to the newest live session installed on the
  thread. A session takes back only its own entry when it ends, so a refused candidate leaves the
  live one. Commit 2 makes an unbound resolve with two live sessions on a thread a development
  assertion.
- **Swift's `ThemeRoles` is per presenter** (by root). Sheets, popovers and split panes adopt their
  presenter's, and a sheet takes its presenter's tint. Boxed gradients carry the session's
  overrides as colours.
- **The colour report table stays process-global** (the platform's). Each session paints its own
  overrides over it, and `generation()` mixes in the bound session's.
- The "role overrides" journal line is gone. Two sessions on one thread, with different overrides,
  answer differently both ways round (tested).

### Stage 5, commit 2: role overrides go live (`55f7c4d33`..`3acbeceba`)

**Stage 4's "role overrides are `rebuild`" rule is lifted.** A role override's value is now a
live-table edit.
- **The design's shape and the plan structure** leave an override's value out. Gaining or losing an
  override is still structure, and so are profile colours. `propose` classes a change to an
  override's value, or to a token one reads, as `live`.
- **On the wire,** an override rides in the batch's and snapshot's `tokens` under its role's name
  (`AccentColor`, `-exact-label`), its value the colour the role now takes. It needs no field of
  its own: in a design an override is a token line named by its role, and a build knows its roles
  from its plan.
- **The runner:** `Roles::set` replaces the session's table in place, so every scope bound to it
  reads the new one, and the rows naming the role (by any spelling) are set again in the same
  commit.
- **The hosts:**
  - Apple re-presents the tint, sheets' tint and ground, bar titles, subtitles, field
    placeholders and grouped lists, and the Mac retints rows naming the tint;
  - the web page rewrites its role sheet;
  - Linux re-targets paint motion and drops kept paint;
  - the JS target's development build makes each override a sheet property (`--exact-label`, the
    tint in the theme's `:root` rule). Its release output is byte-identical to before.
- **Binding is checked.** A role resolved with no session bound while another session is live on the
  thread is a development assertion and one journal line per call site, and every host binds
  where it resolves.
- Apple's and Linux's `unsupported` for a refused pin name its revision.

### Stage 5, commit 3: the binding core (`3e1603cef`..`49c4df257`)

D12's runtime side, under every language's generated names.
- **`session.tamber()`** (`Runner::tamber`): `color(name, place)` reads what a row naming that token
  reads at the place. That takes in its app tuple, the live table's edits and the session's role
  overrides.
  - Identity is the name. A name the plan lacks, or a value that isn't a colour, is `None` and one
    journal line.
  - `Place::{Root, Node(view)}`: a node's tuple is the window's under the nearest `-exact-mode`
    pin, and its scheme is the nearest `color-scheme`'s, else the window's.
- **`inputs(place)`** is one number that moves when anything a value at that place depends on moves:
  its tuple, a token's value (an edit, a live batch, a role override), the platform's reported
  colours, the scheme, the contrast, and the root font size. Hosts compare it after each batch for
  `observe`. It may fire when nothing a binding reads changed, but it never misses a change. An
  update or a dev reload is a new runner. Bold Text and an editor swap come with commit 5.
- **Apple** has one ABI entry, `exact_tamber(rt, len)`: `{view, names}` → `{inputs, colors}`.
  Linux and Android's entry comes with commit 5.
- Two sessions with different designs answer differently, in the runner and through the export.

### Stage 5, commit 4: `tamber gen`, the TypeScript `ColorToken` union, Rust constants, and D7's retention for bindings (`875ad5bc9`..`ce6aef7b7`)

- **`bun exact.mjs tamber gen ts|rust <app> [-o <dir>]`** writes into the app's `.exact/tamber/`.
  The files are generated and never committed, and an unchanged design isn't rewritten.
  - `tamber.ts`: the `ColorToken` union, and `tokens.<camelName> = {name, css per tuple}`.
  - `tamber.css`: `--<camelName>` per tuple, with `:root` the first tuple and `[data-tamber="…"]` each
    other. Exact roles and platform colours are written as their fallback pair, for pages outside
    Exact.
  - `tamber.rs`: `pub const SURFACE_CARD: Token<Color>`, resolved by `session.tamber().get(TOKEN,
    place)`.
  - Names map as D12 says. Names that collide after mapping are refused, naming both, and so is a
    name with no letter, a leading digit, or `__proto__`. Role overrides are named by their role
    spelling.
- **The builds' TypeScript declarations** (`contract types`) carry the `ColorToken` union, bounded by
  `exports`, so a data module returning an unknown token fails `tsc`.
- **D7's retention for bindings** is one test, `contract_lower::names_tokens_outside_views`. An app
  that can name tokens outside its views keeps every token, or its `exports` plus what its views
  name. That means an app with:
  - `app.ts`, or a Rust data crate;
  - native modules or hatches;
  - a seam source the runner doesn't own;
  - a custom element in its views.

  Other apps prune as before. The clone's plan doesn't change size, since it already kept every
  token through computed colour rows. Release JS is byte-identical, apart from the declarations
  gaining the union.

### Stage 5, commit 6: DTCG export and the own-export round trip (`196500b02`..`a7591811c`; built by reload1)

- **`bun exact.mjs tamber export dtcg <app> [-o <file>]`** (`exact_tamber::export::dtcg`) writes
  DTCG Format 2025.10 with the Resolver Module 2025.10.
  - There is one modifier, `mode`, whose contexts are the full tuples × `light` and `dark`. Each is
    complete, and checked to read back as the colour a host renders.
  - Each value is a 2025.10 colour object in its own space where DTCG has it, else Oklab. An HDR
    colour is clipped and marked `lossy`.
  - Validated against DTCG's own 2025.10 schemas, pinned in the repo.
- **The header** (the as-built form of D14's "dials in root `$extensions`"): the resolver document's
  root takes no `$extensions`, so the header is a set `exact`, first in `resolutionOrder`, whose
  token document carries root `$extensions["exact"]`. It holds the format marker, the conversions,
  axes, dials, the context→tuple map and the design digest. The root `description` states the
  conversions in prose.
- **`$extensions["exact"]`** on a token carries `source`, plus `role` or `platform` for a token whose
  value is a host value (exported as its fallback). A role override is a token under its row name,
  with `overrides` naming the role. Icons aren't exported, and lengths and type wait for stage 6.
- **The baseline:** each export keeps what it wrote in `.exact/tamber-dtcg/<digest>.json` (the newest
  16), in a directory whose own `.gitignore` is `*`. A re-import is compared with the export it came
  from, so a stale file can't propose reverts.
- **Re-import:** unchanged values propose nothing, so expressions and roles keep their source. A
  changed value follows D6's target branch:
  - it edits the line that won for its tuple, as `explain` says;
  - a side changed across all of that line's contexts takes the new value, and the other side is
    kept as written;
  - only when some of the line's contexts didn't change is the narrowest separating `when` added,
    never one that ties.

  An unedited export re-imports with no operations, on theme-fixture and on the clone (70 tokens).
  One changed `$value` gives exactly one `set-token`.
- **`export {kind: "dtcg"}`** (*1.4*) returns `{kind, text, baseline}`, and `hello` reports
  `tamber/1.4`.

### Stage 5, commit 5: Swift and Kotlin bindings, and the sample host (`b134894b5`..`fb2fd04fc`; Kotlin `650416fc0`)

- **Swift** (`host/apple/Sources/ExactKit/Tamber.swift`): `ExactTamber(view)` and
  `ExactTamber(session:)`.
  - `.color(token, fallback:)` is a dynamic provider over scheme, contrast and elevation, and
    `observe` notifies.
  - Answers are kept per place under a lock. After each batch, places whose inputs moved ask again,
    and their views redraw (on UIKit through a trait that affects colour appearance). So do views
    moved to another node, and observers follow their view.
  - The tint is resolved per side on the main thread.
- **The native-module library** (`host/apple/modules/ExactTamber.swift`) reaches the same core
  through the hatch host table and a module entry `tamber`, with one place per module view.
  Colours are extended sRGB.
- **Kotlin** (`host/android/canvas/.../Tamber.kt`) goes through JNI `Native.tamber` to the Linux
  host's `Host::tamber_json`. It is compiled with Android's toolchain (the APK builds and the
  `.so` exports the entry), but not yet run: no emulator.
- **The sample host,** `apps/tamber-host` with `scripts/smoke-tamber.mjs`, passes on the Mac and the
  iOS simulator.
- **Observe** doesn't follow Bold Text yet: colours don't depend on it. It comes with stage 6's
  `font` accessor.

### After stage 5's commits: one retention question for `propose` and the dev loops (`8f1445777`, records `2e27c920b`)

`propose` judged its own design shape, which counts every token and ignores `when` lines, so it
classed every `add-token` as `rebuild` and a first `when` as `live`. The dev loops already decided
from the compiled plan's structure, which follows retention. There is now one predicate,
`exact_tamber::Retention::keeps` (D7). The compiler keeps tokens by it, and `propose`'s class
compares the retained shape: axes, dials, and each kept token's name, kind, role, fixedness,
variance, per-tuple accent flags, and any value carrying a platform `var`.
- An app with generated bindings (bindable) rebuilds on `add-token` and on a first `when`.
- A plain app's unnamed `add-token` is `live`, since nothing changes.
- `keeps` takes the token's kind: since stage 6 B, a computed length or number row keeps every
  token of those kinds (`Retention::computed_measure`).

### Stage 5's gate: met (2026-10-17; the run is `~/x2theme/stage5-gate-run.txt`)

| Gate line | Result |
|---|---|
| A native module in the sample host draws with `ExactTamber(view).color(…)` and follows `setMode`, a pin above it, a Dynamic Type change and the live table | **Met** on the iOS simulator and the Mac, live table included. Kotlin on the `x2-android` emulator follows `setMode`, the pin and the scheme. |
| Two embedded sessions with different designs answer differently, role overrides included, on one thread, with no journal line | **Met** by tests. |
| A role-override value edit reaches the clone live on iOS, the Mac and the web, on both loops | **Met**, the JS loop's page included (re-run with the clone's busy-loop fix). |
| A TypeScript data module returning an unknown token fails `tsc` | **Met.** |
| A DTCG export re-imported unedited proposes no operations, on theme-fixture and the clone | **Met:** 0 operations on the real clone (72 tokens), and one changed value gives one `set-token`. |
| Expose, the readers and an app with no design are unchanged | **Met** by plan contents. |

**Known limit:** the Android Canvas host joins no dev loop, so it has no live table and no build
channel (queued). *Lifted 2026-10-17:* the reader polls a dev loop's URL (`x_dev_plan`, a drive's
`--url`) on its turns and wakes, as the Linux display does, registers as `android`, and takes the
live batches, pins and `prefer`s; a plan the loop compiles replaces the running one, and a Rust
change needs a new APK (`host/android/README.md`). Checked on the x2-android emulator with
tamber-host: the Kotlin swatch follows a live batch (about 1.5 s from save to pixels) and a pin
sent to that build only, and a contrast pin is applied.

**Stage 5 is closed.**
### Stage 6, commit B: length and number tokens (B1, B2 the JS target, three review rounds)

- **The crate** folds three kinds (`tamber/src/measure.rs`). A length is px, a bare `0`, one `rem`
  or `em`, a named length the kernel reads (a text style, `-exact-hairline`, by the schema's
  spellings: `Vocab::named_lengths`), or `calc()`, `min()`, `max()` or `clamp()` over px and numbers
  folded to px. A number is a number or such a function over numbers. A kind is written or inferred
  from the value (§3); a lone name takes its token's or dial's kind; a bare `0` infers a number, so
  a zero length writes `: length`.
  - Refused: `rem`/`em` inside a function (`tamber-kind`, naming QUEUE's `rem`/`em` item), a
    percentage, a unit other than px/rem/em, a length and a number mixed, a length times a length,
    a colour named where a length is wanted and the reverse. `when` lines vary them per tuple; an
    axis `under scheme=dark` (`tamber-under-kind`), `scheme=fixed`, `pair=` and a role override stay
    colours'.
- **The digest break, stage 6's one** (D7):
  - the plan's `theme` row gains `kind` (0 colour, 1 length, 2 number; later kinds take later
    codes), so `FORMAT_DIGEST` moves;
  - the kernel's `SCHEMA_DIGEST` moves with it, since `set_dynamic`'s generator (`kernel/build.rs`, a
    codec source the digest hashes) now reads a whole-string length or number token name as its
    slice's value before the row's codec (`style::theme::measured`). A `rem` token is thus the
    relative side table's entry, and the runner's `ThemeIndex` keeps the row as it keeps a colour's.
  - Hosts that move: Apple (iOS, macOS), the wasm web host, Linux, Android's reader, the bake's
    compat record. The JS target recompiles.
- **The token beside the row is the runner's index**, as for colours (`runner/src/instance/theme.rs`),
  not a kernel field; D7's "keeps the token beside it" is met there.
- **Which rows take which kind** is the kernel's one rule (`style::theme::admits`), read at run time
  and by the compiler: a number in `opacity`, `line-height`, `flex-grow`, `font-weight` and
  `-exact-press-scale`; a length in no colour row and no number-only row (`line-height` takes
  both). At run time a name of another kind is not resolved, and the row's codec refuses it.
- **The compiler** (`contract/lower/src/theme.rs`, `measure_row`):
  - a literal naming a token a row does not take is `tamber-kind`, naming the row;
  - every tuple's value is read as the row's codec reads it, not the default's alone (a text style
    off `font-size` in one app mode is `tamber-kind`, with the kernel's reason);
  - a varying token in an inherited row is `tamber-inherited-axis` (only colours re-select at a
    mode boundary);
  - a computed string may name a number token in a number row when the design has one, and then
    every number token must suit the row in every app mode (a press scale positive);
  - at run time an inherited row reads no token that varies (`Entry::varies`), as the compiler
    refuses a literal naming one, so a computed name of one there is refused as an invalid value.
  Retention keeps every length and number token when a row of either kind is computed and may be a
  string.
- **Live:** a value edit is a live batch; a kind change (written or inferred) is structure, in both
  the session's shape and the runner's. The kernel lays out for a length: on a 1,000-row list, a
  `padding` token edit sets 4,000 rows and lays out in 12.9 ms; an `opacity` number edit sets 1,000
  and only paints, 3.1 ms (debug builds, `contract/cli/tests/it/tamber_measures.rs`).
- **The JS target** (`host/web-js/src/measures.rs`): a varying length or number token, and in a
  development build every one, is a custom property whose value per tuple is the literal's CSS (a
  `rem` stays `rem`, a text style its `rem`, the hairline the sheet's property). It is found by a
  sentinel the plain projection never writes: px for a length, a fraction in (0, 1) for a number.
  Every sentinel of a kind is as long as every other, so none is another's substring; a length's
  bare number is replaced too, since an SVG stroke's CSS is unitless. A number token a
  `font-weight` names stays inline in a development build, since no fraction is a weight; its
  design entry says `inline`, so a change to it (or a scrub of it) builds. So does a token an SVG
  attribute row names (`x`, `width`, `r` …), since an attribute reads no property; one that varies
  by an app axis there refuses the JS build, naming it. A token-backed `-exact-press-scale` reaches
  the press animation through the computed property (`input-glue.js`). A bound row's value naming a
  token maps through a module constant per mix of the kinds the row takes (`$tl`, `$tn`, `$tln`, an
  SVG attribute's `…a` writing values, never `var()`), so a name of a kind the row does not take
  stands, as the kernel leaves it. A bound `opacity`'s layer test reads a constant token's value (a
  varying one may be below 1). A grid track's `var()` of a token passes the target's grid check
  (each tuple's value was checked at build).
- **What the JS target does not read:** a varying token's name computed (from state or data) into
  an SVG attribute row stands there, since no tuple's rule reaches an attribute; and in a
  development build a token computed into one is written as its value, so a live edit of it waits
  for the rebuild. A literal name in an SVG attribute row is refused when it varies, and inline
  otherwise (above). In a static row a release writes a token that does not vary byte for byte as
  its literal; a plan that keeps no length or number token is byte for byte as before (the clone's
  `app.js` and `app.css` are identical).
- **Measured on theme-fixture with the JS loop, by hand:** a length edit arrives in ~0.2 s, a number
  edit in ~0.05 s, with no reload; a length scrub previews and cancels, with the file untouched.
- **The wire** (`tamber/1.5`): each IR token's `kind`; `resolve`'s length `{unit, amount}` and number
  `{amount}`; `uses` over rows of the token's kind. The concrete px of a `rem`, `em` or named
  length is the build's (`resolve-on`, LLP 1122 T2b).

### Stage 6, commit D: shadow tokens

- **The crate** (`tamber/src/shadow.rs`) folds `box-shadow` and `text-shadow` tokens, each its row's
  own syntax: a list (at most 8) of `inset`? and two to four lengths with one colour for a box
  shadow; one shadow of two or three lengths with a colour or none (`currentcolor`) for a text
  shadow, which has no spread and is never `inset`. A length is px, unitless zero, a length token
  or dial; `rem`/`em` inside one is refused naming QUEUE's `rem`/`em` item. A colour is a literal, a
  colour token, a role or a colour expression over tokens, folded per tuple as a colour token is,
  so a shadow's value per tuple is its row's text with its colours folded. Its kind is always
  written: a value that reads as a shadow with none is refused, asking for it.
- **The plan** numbers the kinds 3 and 4; no new break (B's `kind` byte holds them). The kernel
  reads one only in its own row (`admits`).
- **A colour token inside a shadow follows its own live edit**: the session refolds every token, so
  an edit of the colour moves the shadow's value too, and the batch carries both (live; tested at
  the session). A shadow edit paints: it lays nothing out.
- **A text shadow is an inherited row**, so a text-shadow token that varies by an app axis is
  `tamber-inherited-axis` (only colours re-select at a mode boundary), as for lengths and numbers.
- **The JS target:** a varying shadow token, and in a development build every one, is a property
  whose value per tuple is the row's CSS as `css.rs` writes the literal's; its sentinel is a shadow
  whose spread (a text shadow's offset) is a marker length, and the whole declaration holding the
  marker becomes the `var()`. A bound shadow row maps a name through its own constant (`$tb`,
  `$tt`).
- **The wire** (`tamber/1.8`): the kinds, and `resolve`'s shadow tuples.

### Stage 6, commit E: duration and easing tokens

- **The crate** (`tamber/src/motion.rs`) folds a `duration` (a CSS time, in ms, or a duration token)
  and an `easing` (a keyword, `cubic-bezier()` with its x in [0, 1], `steps()` with a whole count,
  `linear()`, `-exact-spring()`, or an easing token). A time, an easing keyword and an easing function
  infer their kinds. A `when` on one, written or inferred, is `tamber-motion-axis`.
- **Dotted names:** a duration's or an easing's name is dotted (`duration.fast`); another is
  `tamber-motion-name`. No CSS property, keyframes name or easing keyword has a dot, so a dotted
  word of a motion row that is a token's name is that token wherever it stands in the shorthand
  (`transition="1s opacity"` stays as written, whatever tokens exist). An inline `theme`'s names
  are undotted, so it declares none.
- **Substituted at build:** the loader writes each duration and easing token a literal of a
  `transition`, `animation`, exit animation, layout transition, timing longhand or keyframe's
  timing names (a template's text and holes, a `+`'s strings, a keyframe timing a function of literal
  arguments returns), as a whole word (not inside quotes, nor touching a template's `${…}`), in as its
  value before scoping, the longhands' composing or the row's codec read it
  (`contract_lower::motion`); lowering does it again, before the longhands compose, for a value an
  expansion carried in (a component's prop, to a shorthand or a longhand). So the plan holds the
  values and keeps no motion token. Named as a whole row of another kind, one is `tamber-kind`.
  Its tokens are checked by the motion crate, as the transition reads them (the crate depends on
  `exact-motion`). A profile's colour in the design is stood in for when the loader folds the
  motion tokens, so it does not stop their substitution. `uses` reports the literals the loader
  wrote a token into, in every source the app loads (a package's too), and the component
  arguments that name it (a site has no plan node); an app that does not load answers the build's
  error, not an empty list. A timing longhand's list splits at commas outside parentheses, so an easing
  function is one item.
- **An edit rebuilds** (D13): the session classes a change to a motion token, or to what it folds
  from, as `rebuild`, so `propose` says so and a scrub is refused; the plan's structure moves, so the
  dev loops' retained-shape checks see a structure change and rebuild.
- **Dials:** D15's duration control (a slider in ms) is a param's (stage 7); a dial stays a colour,
  a length or a number.
- **The wire** (`tamber/1.9`): the kinds, and `resolve`'s `{ms}` and easing tuples.

### Stage 6, commit A: the prerequisite, LLP 1115.000 D12.2 and D12.3 (A1 `e3e263603`..`df377a133`, A2 `60ef5581e`..`4a55f98ab`)

Built and recorded in full in LLP 1115.000's as-built table (D12.2 and D12.3). For Tamber:
- **A text style carries its weight and leading** per platform column, measured on the iOS simulator
  and the Mac: the Mac's headline is Bold with its own leadings, and iOS uses the face's natural
  line height.
- **Precedence per longhand** (the lead's ruling):
  1. written on the node;
  2. a style named through `font` or a type token;
  3. inherited;
  4. a style named by a bare `font-size`;
  5. CSS's default.

  So the `font` shorthand lowers to only `font-style` and `font-size`. That is what lets C's type
  tokens write only the longhands they name.
- **`layout` and inspect** name a row's source `style` or `heading`.
- **Headings** keep an implied bold through one switch, `HEADING_BOLD`, pending Charlie.
- **Bold Text** weights the system font by UIKit's measured step (400→600, 600→800, 700 and above
  →900), and leaves a bundled family alone. iOS and Android read the setting; the Mac and the web
  have none. Android's toggle is compiled and unit-tested but not observed on the emulator.
- **A known web-target difference:** a style named by a bare `font-size` fills nothing on the JS
  target, which keeps no kernel to see an ancestor's weight (queued).
- **The kernel schema digest moved again** (the `textStyles` columns and prop 270,
  `fontShorthand`), within stage 6's break window after B's. No host binary shipped between them.
### Stage 6, commit C: type and family tokens

- **The crate** (`tamber/src/typeface.rs`) folds a `type` token, CSS `font` syntax: `italic` (the
  kernel's `font-style` holds `normal` and `italic`, so `oblique` is refused), a weight (a whole number from 1 to 1000, or `bold`), a size (px, `rem`, `em`, a text
  style, or a length token naming one of those), an optional `/line-height` (a number, a length,
  `normal`, or a number or length token), and an optional family (a stack, or a family token). It
  names only what it writes, so `normal` is refused (it says nothing). A `family` token is a stack:
  each member a quoted name or identifiers, joined `, `; or another family token. A value with more
  than a size (a line height, a family, a style or weight before it) infers `type`; a stack with a
  quote or a comma infers `family`; otherwise the kind is written. A type token whose size is px in
  some tuple is warned of, never refused (`tamber-type-fixed`).
- **Rows name the token; the kernel reads each one's part.** `font="type.body"` lowers to one row a
  longhand the token writes, each naming the token, and `fontShorthand` (D12.2), so a text style's
  weight and leading apply where it writes none and the rows it writes are written rows
  (`contract/lower/src/type_tokens.rs`). The kernel reads each row's part
  (`kernel/src/style/theme/typeface.rs`). `font-family="family.mono"` is one row naming the token. The
  plan numbers the kinds 5 and 6, no new break. A type token in another row, or a family token
  outside `font-family`, is `tamber-kind`. A choice between tokens (`font=(c ? "type.a" : "type.b")`)
  lowers the same way. Every token and tuple it may be writes the same longhands, else it is
  `tamber-type-longhands`.
- **A family is a stack the plan declares**, as a literal `font-family`'s is. The compiler declares
  one for each tuple's family. A bound table holds its id (`#12`): the compiler's from its own
  stacks, the runner's from the plan's by their members as the compiler reads them
  (`typeface::family_key`: a quoted name, a generic one only unquoted, `-apple-system` as
  `system-ui`). The runner passes a `font-family` string naming a token, dotted or not, to the
  kernel rather than interning it as a runtime family. A one-word family (`textFace`) names a family
  token where the design has one. A type token's written style and weight are checked against a
  family the app ships, as `font-weight` and `font-style` beside `font-family` are (`lower-font-face`).
- **The axis (theme2's ruling):** a type or family token that varies by an app axis is refused on a
  node with element children, which would inherit its rows (`tamber-inherited-axis`, "write it on the
  text, not its container"). On a text with none it is read again per tuple, as a colour is: the
  kernel lets a varying type or family token reach an inherited row. A name computed at run time
  is held to the same rule by the runner, which refuses it as an invalid value there (a child is
  the node's own, or a choice's or a list's under it). A mode can't rescale a whole
  subtree's type through a container; boundary re-specification is a later stage's, if an app needs
  it.
- **Live and rebuild (the lead's ruling):** an edit that changes only a type token's style, weight,
  size or line height is live, and each row that names it reads it again. One that changes the
  longhands it writes, its family, or a family token's stack rebuilds: the session's shape carries
  them.
- **Bold Text** applies through D12.3's rule: a type token naming a system family is stepped, an
  authored weight included, and a family the app names is not.
- **The JS target** writes a type or family token as the kernel resolves each longhand, a literal,
  never a property (`measures::inline_tokens`); a bound row maps a name to that row's part (`$ty<row>`):
  a family as its plan stack's CSS (a bundled font's `ExactPlanStack<n>` alias), a text style as its
  name, which the size's own map turns into its `rem` and the style's weight and leading.
  So one that varies by an app axis is refused there (`QUEUE.md`); a development build rebuilds on an
  edit to one.
- **The wire** (under `tamber/1.9`, the window's one version): the kinds, and `resolve`'s type and
  family tuples.
- **Tests:** `contract/cli/tests/it/tamber_type.rs` (only the named longhands, the style's weight,
  a family token's stack, a tuple's leading under a pin, Bold Text on a system and a named family, a
  live value edit, and the refusals); `tamber/tests/it/edit.rs` (live versus rebuild);
  `tamber/tests/it/pair.rs` (`tamber-type-fixed`); the crate's corpus `type`, `refuse-type`.

### Stage 6, commit F1: the icons block (D10; stage 9's, brought forward)

- **The crate** (`tamber/src/icon.rs`) reads an `icons` block's tokens, each
  `icon(sf "…", material "…", svg "…")`: an `sf` name (lowercase, digits, dots), a `material`
  name (lowercase, digits, underscores), an `svg` path under the app's `assets/` (no `..`, ending
  `.svg`), each at most once, an `sf` or an `svg` among them (a `material` name alone draws
  nothing); commas inside the quotes are a name's, and a path with a `\` or a `:` is refused (it
  leaves `assets/` on Windows). Its name is dotted (`tamber-icon-name`), so no dial, role or CSS word
  is spelled as one, and unique among the design's tokens and icons (`tamber-duplicate`); a kind
  written is `icon`; no `when` (`tamber-icon-axis`); an icon under `theme` is refused, pointing at
  `icons`, and a value naming an icon says it is one. `exports` may list an icon (D12's bindings
  name it). `fmt` lays an icon's line out as a token's; `history` reports an icon's change (no
  operation writes one). What the SVG holds and which shipped target needs which source are the build's (F2).
- **The IR** carries `icons`, when the design has any, each its sources; `resolve` answers an
  icon's, the same in every tuple. An operation writes no icon (the design tool edits none yet);
  an icon's change is retained shape, so `propose` classes it `rebuild`.
- **The wire** (`tamber/1.9`): the IR's `icons` and `resolve`'s icon tuples.

### Stage 6, commit F2: icons at build (D10)

- **The SVG** (`contract/lower/src/icon_svg.rs`) is read as XML (comments, CDATA and
  declarations skipped, entities decoded, attributes apart) for its viewBox (its own, or its width
  and height) and its `<path>`s' `d` outside `defs`, each filled on its own (nonzero), joined by
  `;`, which no path data holds. Fill and stroke follow SVG's cascade (a `style` declaration over
  the attribute, inherited from groups). One whose paths fill in more than one colour, or are
  stroked or unfilled, or with what a filled template would lose (a transform, an even-odd
  `fill-rule`, an opacity below 1, a clip, a mask, a filter), or with anything but paths and groups
  (a gradient, an image, text, a `<circle>`, a `<use>`) is no template; a `d` that is not path data
  is refused.
- **Per target** (`contract/lower/src/icons.rs`): the targets an app ships are its manifest's
  (`.shells/app.json` first, as the build reads it, else `app.json`) `host`, `deploy.store` and
  `deploy.binaries` keys, and its target crates (`android/`, `linux/`, `web/`, theme2's ruling); an app with no manifest is checked for none. The source graph names each
  icon's SVG, so a watcher rebuilds on its edit. Apple's draw an `sf` (their hosts draw no path yet, so an SVG is no
  source there: a known limit, narrowing D10's "svg fallback", accepted by theme2; QUEUE "Apple
  hosts draw icon paths"); any other a template SVG; the web also a file that is no template, as written. Else `tamber-icon-missing: <icon> has no source
  for <target>`, or `tamber-icon-svg` naming why the file is no one-colour shape and the target.
- **The source form:** a literal `symbol:icon.reply` is written as
  `symbol:icon|<sf>|<view_box>|<path>|<svg>`, which hosts read (`exact_kernel::Icon`,
  `symbol_path`, `apple_symbol_name`), so the plan keeps no icon token for a literal.
- **The plan's `icons` table** keeps what a computed source may name: every icon when a `symbol:`
  source is computed, else those a literal named and, in an app that names tokens outside its
  views, those `exports` keeps. The runner writes a computed `symbol:icon.reply` or
  `symbol:sf/<name>` (the icon whose `sf` it is) in the same form as it sets the prop.
- **`symbol:sf/<name>`** that is an icon's `sf` is that icon; else one that is a role's own Apple
  name is that role, which every host draws (theme2's addition); either is
  `lower-symbol-alias`, naming what to write. A bare one in an app that ships a target other than
  Apple's is `lower-symbol-platform`, naming the targets and the icon token to declare.
- **The break:** stage 6's second, stage 9's brought forward: the plan format digest moves
  (`icons`, and the `theme.kind` comment for kinds 3 and 4 that D left out), pinned in
  `plan/tests/format.rs`. Every host needs a new binary; an old one refuses a new plan by the
  format digest before reading a table (`FormatDigestMismatch`).

### Stage 6, commit F3: hosts draw icons (D10)

- **One reading:** every host reads a symbol through the kernel: `apple_symbol_name` (Apple's SF
  Symbol, an icon's `sf` included) and `symbol_path` (`SymbolPath`: a role's path, stroked or
  filled even-odd in 24 units; an icon's paths under its viewBox, each filled nonzero; or `sf/` a
  role's Apple name, which draws that role).
- **Linux and Android's reader** (`paint/svg.rs`, `presenter/top/look.rs`) draw the path scaled by
  its viewBox, a role's stroke kept at its 24-unit width; paths are parsed once by their text.
- **The web** (`element.rs`, `glue.js`) writes `data-symbol-viewbox` beside the path; an icon whose
  SVG is no template is the file (`src="assets/…"`), shown as any image is.
- **The JS target** (`symbols.js`, `src/symbols.rs`) carries the plan's icons in its table (by name
  and by `sf/`, before the roles), reads the build's own form, and loads every role (with `sf/`
  keys for their Apple names) for a computed `sf/` it does not hold; a role's entry keeps its two
  fields, so an app without icons emits what it did.
- **Apple** reads the SF Symbol as before, through the kernel (`content_region.rs`,
  `kernel/src/grouped.rs`).
### Stage 6, commit G: bindings and the DTCG export of every kind

- **The DTCG export** (`tamber/src/export_kinds.rs`) writes every kind as its DTCG type, one value in
  light and dark: a length a `dimension` (px or `rem`; an `em`, a text style or the hairline as px at
  the default root size, `lossy` its source); a number; a family a `fontFamily`; a type a
  `typography` with every property, what it leaves unsaid named (`unsaid`); a shadow; a duration; an
  easing a `cubicBezier` (a keyword as its curve, a `steps()`, `linear()` or spring `lossy`). Icons
  stay outside it. Re-import reads each back as the design writes it (numbers by their value, so a
  tool's reprint changes nothing; the file's own `$extensions` where it kept them; a part the export
  left unsaid becomes said once edited; a shadow whose colour differs by scheme as one value with
  `light-dark()` colours), proposing per tuple the line that wins where every tuple it wins in
  moved, else a `when` of the tuple's axes, and checks each changed tuple folds to the file's value
  (`0.2s` is `200ms`) and each other keeps the design's (so an edit to one side of an axis under
  scheme=dark, which no kind but colour varies by, is refused); an edit the design already has
  proposes nothing. A placeholder the file moved (a type's size, a line height in a unit, a
  `currentcolor` shadow's colour) is taken as its value. A shadow's colour reference is written
  back as its folded colour once the shadow is edited. An export and re-import of a design with every kind proposes
  nothing. The vocabulary carries each named length's px (`Vocab::sizes`).
- **Bindings.** Rust: `Token<Length>`, `Token<Number>`, `Token<Type>` beside `Token<Color>`
  (`tamber gen rust`), resolved per place (`Tamber::length`, `number`, `font`: a `Font` of points,
  weight, style, line height and stack). A type token's weight is its text style's on this platform
  where it writes none, stepped by Bold Text on a system family (`Kernel::bold_text_weights`).
  `inputs` moves with Bold Text and the person's text size, so `observe` hears both. Swift:
  `ExactTamber(view).length(_:)`, `number(_:)`, `font(_:fallback:)` (a `UIFont`/`NSFont` the host's
  text engine resolves), in the process and in a native module's library (over the same host
  table's JSON: a bundled face by its name at its weight, a variable file's `wght` set; a system
  stack, which no name opens, as the system font at the weight in its design, serif, monospaced
  or rounded). Kotlin: `length` and `number`. TypeScript: a union per
  kind (`LengthToken`, `NumberToken`, `TypeToken`, `FamilyToken`, the shadows), bounded by
  `exports`; a motion token has none, as it is written into its rows at build and no module names it
  (D13). `tamber gen` writes each kind; `tamber.css` writes a text style, an `em` and the
  hairline as px at the default root size, saying so. Icons' `symbol(.icon…)` follows F.

### Stage 6, commit H: the importers propose every kind

- **What changed.** shadcn, Tailwind, DTCG and Astryx no longer report lengths, numbers, type,
  families, shadows and motion as `kind pending (stage 6)`. Each is proposed as its kind's token,
  and `add-token` writes the kind (`tw.radius.lg: length = 0.5rem`). A value is proposed when it
  folds as its kind beside the import's other tokens. The check is one design of them, plus the
  colours they name and the axes their `when` lines name (`import/kinds.rs`). It also says why a
  value does not fold, and that reason goes in the report: `not a <kind> Tamber folds: <why>`. A
  token the design already has of another kind is reported, not set (its line keeps its kind);
  so is a value that would change the kind of a design token with no kind written (`set-token`
  leaves it unsaid), and one the edit grammar would not take on its own line (an unquoted `in`).
  The import is not folded into the destination design itself: a design token that reads an
  imported one is not checked (QUEUE).
- **References.** A `var(--x)` is the token `--x` becomes, named bare (D3), through a chain of
  them; one to a variable that makes no token (another's name, a redefined key) is reported. A
  string (a quoted family) is kept as written. A Tailwind shadow over `var(--color-ring)` names
  `tw.ring`. A DTCG alias is written out as its value, inside a
  composite too (a shadow's `{size.sm}`, a typography's `{font.body}`).
- **One value on both sides.** A value of these kinds that differs between light and dark is
  reported: only a colour has a dark side. A theme class (shadcn) or a mode (DTCG) still gives a
  `when` line.
- **Each importer:**
  - **Tailwind.** `--text-<n>` with its `--line-height` and `--font-weight` is one type token.
    Its size, leading and weight are written out, their `var()`s as what the keys hold (a
    sub-property of another type too), and a `calc()` leading is folded to its number. A type
    key whose size is `var(--text-<m>)` takes that size alone, through a chain, not `<m>`'s
    leading and weight. A key whose value reads an `inline` key the stylesheet redefines,
    anywhere in it, is reported.
    Reported:
    - a sub-property a token holds no part of (`--letter-spacing`, a family's
      `--font-feature-settings`);
    - a sub-property whose key the theme lacks;
    - a key the stylesheet redefines outside `@theme`, or a sub-property of it (its utilities
      follow the redefinition);
    - a text shadow list.

    The default theme is now 419 = 364 mapped + 55 reported.
  - **shadcn.** A declaration that is no colour is a length by its value (a bare `0` too). It is a shadow or a
    family by its name (`--shadow-*`, `--font-*`), or a reference the kind of what it names (its
    last declaration, through a chain). A variable with one kind in every block it wins is a
    token; one that is a colour, or a value of no kind, in another block is reported. A colour
    role is offered only over a colour token. Neutral is 47 = 47 + 0.
    - `--radius`'s derived sizes are proposed when they fold: `lg` is `shadcn.radius`.
    - `sm`, `md` and `xl` are `calc()` over a `rem` radius. They are reported outside the
      count until QUEUE's "`rem`/`em` where the kernel still takes only `px`".
    - A px radius derives all four.
  - **DTCG.** These `$type`s map (`import/dtcg/kinds.rs`):
    - `dimension` and Tokens Studio's length types (a bare number is px);
    - `fontFamily` (a string is one family, commas and all; Tokens Studio's `fontFamilies`
      text is a stack);
    - `fontWeight` (a weight's name too, DTCG's list: `book` 400, `extra-black` 950) and `number`
      (a line height's percentage, as text or a dimension, is a ratio);
    - `duration`, `cubicBezier`, `shadow` (an `innerShadow` is inset);
    - `typography`, a `normal` leading kept. A typography whose letter spacing, paragraph
      spacing, case or decoration says anything (a zero in any unit says nothing) is refused: a
      type token holds none. A Tokens Studio `fontFamilies` alias inside it keeps its members.

    Figma's fixture is 104 = 101 + 3; Tokens Studio's sample is 49 = 49 + 0.
  - **Astryx.** A text style's size, weight and leading is one type token. Font sizes are lengths
    and weights are numbers. A value must be on both sides. A second variable to one token's
    name is reported, a second text style's too. 268 = 261 + 7: the six duration tiers and the line style.
- **Counts.** Every importer's count still holds (source = mapped + reported), and re-importing
  proposes nothing (`tamber/tests/it/import.rs`, `import_kinds.rs`).

### Stage 6's gate: met (2026-10-17; the run is `~/x2theme/stage6-gate-run.txt`)

The lines are §5 stage 6's, plus stage 9's for icons, brought forward.

| Gate line | Result |
|---|---|
| Expose's 48 `font-family` writes and its radii become tokens, with screens identical on the web, iOS and the Mac | **Met:** 4 of 4 screens with 0 pixels different on each host, an identical native dump, and byte-identical JS CSS. |
| Under Bold Text, a system-font type token is bolded, one with an authored weight too, and a bundled family isn't | **Met** on the Mac and the iOS simulator: system and authored-600 tokens bolded, and "Exposure Sans" unchanged. |
| A length token's value edit is live on the clone and lays out; a shadow edit is live; a motion token's edit rebuilds | **Met** on the wasm loop. A length edit took about 1 s with no generation, the padding and width moved, and a typed draft was kept. A shadow edit was the same. A duration edit was `rebuild`, restarting with state carried. |
| The sample host draws a `font(…)` and a `length(…)` that follow Dynamic Type and Bold Text | **Met** on the Mac and the iOS simulator. |
| An export with every kind re-imports with no operations | **Met:** 13 of 13, with 0 operations. |
| Stage 9's: the clone's `symbol:sf/…` uses draw on the web from icon tokens; a missing source fails the build | **Met:** every SVG on five routes drew, and a missing icon file fails naming the token and the file. F4 is merged into the clone's main. |
| Expose, the readers and an app with no design are unchanged | **Met** by plan contents against the commit before stage 6. The only new rows are A's `fontShorthand` markers. Conform is 12 of 12 on Expose and 15 of 15 on tamber-modes. |

**Stage 6's breaks:**
- B moved the plan format and kernel schema digests (theme rows gain `kind`).
- A moved the kernel schema digest (text style columns, `fontShorthand`).
- F moved the plan format digest (the `icons` table).

No host binary shipped between them, so users see one break. Both digests are now pinned by tests.

**Known limits carried:**
- the JS target writes type and family tokens, and inline font weights and SVG attributes, as literals, so they rebuild there and refuse to vary by axis;
- a bare `font-size` style fills nothing on the JS target;
- Apple draws only an icon's SF Symbol;
- a varying type or family token is refused on a container's inherited rows (`tamber-inherited-axis`);
- `calc()` with `rem` waits on QUEUE's item.

**D3, amended by this stage:** `duration`, `easing` and `icon` token names must be dotted
(`tamber-motion-name`, `tamber-icon-name`), so a transition's words or a symbol's name can't be
mistaken for one. There are 88 symbol roles plus 58 aliases, not 76.

**Stage 6 is closed** (with stage 9's icons).
