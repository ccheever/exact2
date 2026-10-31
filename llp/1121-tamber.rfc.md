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
  refused with the axes named. *(Amended 2026-10-23, the checker's own bound, theme2's
  approval:)* a value past 4096 bytes or 32 nested functions (`tamber-value-size`,
  `tamber-value-depth`), a length over base names past 256 terms (`tamber-length-size`), and a
  chain of names past 512 deep (`tamber-reference-depth`) are refused in the build and the
  design server's check alike, before anything reads the value; the importers and a reimport
  refuse such a source value with the same id.
  *(Amended 2026-10-26, stage 14's sweep, theme2's approval:)* a design declares at most 32
  axes (`tamber-axes`) and a token or param at most 64 `when` lines (`tamber-whens`). Both are
  design-format limits, refused in the preflight before anything walks the design, so the build,
  `check` and the design server refuse alike and in the same words; an importer that would pass
  either refuses with the same id (Pencil past its own 16 themes, too). The 64 tuples remain the
  cap on what the axes make; the 32 bounds the axes themselves, since an axis of one value adds no
  tuple. No real design comes near either (the corpus's most is 7 axes and 4 `when` lines). The
  wire's own limits (`tamber-ops-size`, 1024 operations a client's request; the 1 MiB body, 16 MiB
  for the local token's `import`) are `docs/tamber-protocol.md`'s, under `tamber/1.17`.
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
  `currentcolor` outside the runner's commit. `:active` is the exception, by way of the press
  paint (below): it takes `color` (the host's pressed colour already reaches the subtree, LLP
  1061 D7) and, until the press paint can draw them, refuses `border-color` and `box-shadow`.
  Another row is `lower-state-row`.
- **Runner-known states** (`:disabled`, `:checked`) lower to the conditional rows an author
  can write today. **`:hover` and `:focus-visible`** lower to a **state look**: a `looks` row
  `{ node, state, rows }`, keyed by node and state, applied by the host on its own tracking,
  with no round trip (LLP 1061 D1's reason). **`:active` lowers to the press paint** (LLP 1061
  D7: `-exact-press-background-color`, `-exact-press-color`, the press's opacity and
  `-exact-press-scale`), so each host has one press and one runtime for it, and no `:active`
  look.
- **`:active` is the host's existing press**, not CSS's: the innermost node with a `press`
  handler (LLP 1061), eased on the curve the host uses for `-exact-press-scale` today
  (`cubic-bezier(.16, 1, .3, 1)` on Apple), held under Reduce Motion with its colours still
  easing (the press is feedback, not motion: LLP 1061's 2026-09-27 ruling and D7), and
  unsmoothed on the agent's clock. On the web it is the runtime's existing press
  glue (`data-pressed`), never a stylesheet `:active` rule, which would match ancestors and
  nodes with no `press` handler. A written `-exact-press-scale="0.97"` is that state's
  scale.
- **Composition, per row**, after shorthands are expanded (`border-color` to its four
  sides, so a state's `border-color` overrides each side the base wrote, longhand or not),
  from lowest to highest:
  1. the class's rows, then the node's own rows (the node wins, as today);
  2. `:checked`, the class's then the node's;
  3. `:disabled`, the class's then the node's;
  4. the host overlays, `:hover`, then `:focus-visible`, each the class's then the node's;
  5. the press paint from `:active`, the class's then the node's.

  Steps 1–3 are the base, folded by the runner into conditional rows. A disabled node takes
  no press, hover or focus, as today. An overlay's values are absolute; when the base or the
  class changes under an active overlay, the host applies the overlay over the new base, and
  rows the overlay does not name show the new base. The host's ease owns overlay changes; an
  authored `transition` owns base changes. An `:active` `opacity` is CSS's, an absolute
  target, eased from the opacity shown at the press (a hover or focus look's included), not
  D7's share of the node's own; the press paint carries the target, replacing D7's clamped
  share for it. A `:hover` or `:focus-visible` look's colours snap under Reduce Motion.
- **One spelling a node.** A node or its class writing a D7 row and the same paint in `:active`
  (`-exact-press-background-color` and `:active`'s `background-color`, say) is refused,
  naming both. The design tool's `hold active` drives the host's real press, and inspect gives
  an `:active` row's source span, though it lowered to the press paint.
- **A declared state replaces the host's default for that state only.** `:disabled` sets the
  same `disabledStyled` mark the heuristic sets, and the heuristic's existing forms (an
  authored `opacity`; a colour that reads what `disabled` reads) still count. `:active`
  turns off the host's unsaid press look on that node (iOS's dim, the Mac's ink wash, the
  web's 65%).
- **Excluded:** native controls (their states are the platform's, D9) and a `list`'s rows
  (their press is the cell highlight, LLP 1115.000 D16). A state part on either is
  `lower-state-target`.
- **Ruled 2026-10-18 (Charlie, on a panel of Astra, Grok and Fable; option B):** `:active` is
  the authoring form and D7's press paint its one runtime, extended until it paints what
  `:active` says; this replaced looks for `:active` (stage 8 A's table keeps `:hover` and
  `:focus-visible`).
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

*Amended 2026-10-30 (the bake-off program lead's ruling, LLP 1116 round r34):*
`tamber-unreachable` is information, not a problem. It stays reported, because the author
should know where the platform's look stands, but `bun exact.mjs verify` and `test` say it
apart, as a note, and leave it out of their problem count and verdict: no author can change
the Mac's switch, and 12 of r34's 24 builds spent fix rounds on these lines. The overrides one
declaration sets are one line naming each host and control (`accent-color on iOS, macOS and
the web: the platform's look stands on macOS switches (NSSwitch draws the system accent);
…`), where one `accent-color` had made six.

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

  An SF Symbol on Apple where one is given, and an SVG under `assets/` everywhere else, Android
  included (the reader paints SVG paths, `host/android/README.md`), Apple's too where no SF Symbol
  is given (a one-colour SVG's path, stage 9 A). A `material` name is an annotation
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

*Amended 2026-10-19 (stage 11 B; theme2):* the base scale ships as nine base names (`space.xs`
to `space.xl`, `radius.sm` to `radius.full`), each a length in each host's column of the
kernel's table. A design value or a `when` arm may name one bare (`card.pad: length = space.md`,
an alias), and what it means depends on the app's design:
- **the base name is unset:** the alias is a per-host reference, kept as written and never
  folded, so each host reads its own column; an expression over it is kept too (below);
- **the app's design sets it:** it is the app's own token, and every reader folds through it as
  through any token reference, so the app's value wins everywhere, an edit of it re-folds the
  alias, and a ranged param naming it keeps its px.

*Amended 2026-10-20 (stage 12 B2; theme2's 00:42 ruling):* an expression over an unset base
name (`calc(space.md * 2)`, `min(space.lg, 20px)`; lengths only, with px and numbers beside it)
is kept as written, its token references inlined and its constant parts folded, so one
expression has one text (`calc(space.md + 2px + 2px)` is `calc(space.md + 4px)`). The scale
table carries no arithmetic, and the schema, its digest and the wire don't move: the theme
row's value is the expression.
- **Each host evaluates it in its own column at run time.** The kernel substitutes each
  base's length (the design's own definition first, as an alias reads it; the reads recorded)
  and folds the `calc()`, `min()`, `max()` or `clamp()` with its own calc code, so Apple,
  Linux, Android and the runner's bindings read px. The JS target writes
  `calc(var(--exact-space-md) * 2)`, and an SVG attribute the build's column's px. The
  terminal evaluates it in its column (cells as px), and its spaces then round as every space
  does there (LLP 1101 §4).
- **The compiler checks the expression in every column** and names the column it fails in
  (`card.pad` is the length `-2px` on mac (`calc(space.md - 12px)`), which `padding` does not
  take), a param's range included. A bare unset base name in a ranged param is checked the
  same way, per column, where it was refused before.
- **What is refused:** `rem`, `em` or `%` beside a base name (as in any `calc()`), a base name
  times or over a length (a base name's or px), and a length added to a number. A base name the
  app's design sets folds through as before.
- *Amended 2026-10-20 (stage 12 B3; theme2's sign-off):* **a shadow token's lengths** may be any
  of these: a bare base name, a length token, or a `calc()`, `min()`, `max()` or `clamp()` over
  length tokens and base names. One length path for every shadow length: over a base the design
  leaves unset it is kept, each host folding it in its own column (each base read recorded, so an
  edit of it sets the row again); over px tokens and set bases it folds to px, so a shadow over
  `space.md` keeps working when the app sets `space.md`. The compiler checks every column, its
  blur never negative and its lengths within the limits. A literal `box-shadow` row's lengths
  stay px (QUEUE).

**Packages.** A package (`packages/reading`) may ship `<package>.tamber` with tokens named
under its package (`reading.measure`, `reading.body`), which its views use. The app's design
may set any of them by full name. Two definitions of one name from different files are a
compile error, unless the app's design sets it. Dials and axes are the app's alone: a package
may not declare them, and a package token's `when` lines may name only the app's axes, which
the app must declare or the package's token is refused. What a build keeps of a package's
tokens is D7's one retention rule.

As built (stage 11 A1, theme2's rulings Q1–Q8 and 13:06): the file is `<name>.tamber` beside
the package's entry (what its `package.json` `exports` maps `.` to, else `index.contract`), or
the file its `package.json` `"tamber"` names inside the package (missing, or leading out of it:
`tamber-unreadable`), for every package the app's sources link, transitively (a package vendored
inside one and reached by a relative `use` included). `<name>` is the last part of the package's
own name (`@exact/reading` → `reading`, whatever name a `use` installed it under); the file's
header is `tamber <name>` and every token is `<name>.…` (`tamber-package-name`). Two linked
packages taking one namespace that both ship a design are refused, naming both: no line of the
app's can tell their tokens apart (the one-name rule above is then the app's and a package's: the
app's definition wins). A token keeps its name in the one table; its source is
`<package>/<file>` (`@exact/reading/reading.tamber`), and the design server names it
`<package>/<file>#<token>` (stage 11 A2), as D15 names components. A package's design holds a
`theme` block and its components' params (stage 12 B1: `component … params`, named
`<package>/<file>#Name`): axes, dials, `params` lines, icon tokens and `exports` are refused
(`tamber-package-block`; icons QUEUE; *superseded for icons by stage 13's amendment below*: a
package's design declares icons). Its tokens join the
app's design in the one fold; a token the app's design defines is the app's, whole (all its
`when` lines). What a package token names, as the fold resolves it (its values and its `pair=`),
is a token its own design declares, a role or one of the base scale's nine names (`space.xs`,
`space.sm`, `space.md`, `space.lg`, `space.xl`, `radius.sm`, `radius.md`, `radius.lg`,
`radius.full`, whose values are the app's until stage 11 B's; a package may not declare them),
and no dial its values read (`tamber-unknown-token`; references across packages are QUEUE).
A refusal in a package's design points at its line there (a token's, a `when`'s, a refused
block's); one about the file (unreadable, a namespace taken twice) names the file.

*Amended 2026-10-22 (stage 13, package icon tokens; theme2's 03:22 ruling):* **an SVG path is
relative to the `assets/` of the root whose file writes it**: the app's directory for the app's
lines, the package's root for a package's (its design wherever its `"tamber"` puts it, or its
Contract; a vendored package is the named package its identity names). A package's design may declare
`icons`, each named under its namespace (`reading.star`; else `tamber-package-name`) and naming
only its own package's icons (an app's or another package's is `tamber-unknown-token`, as for its
tokens). The app's design overrides one by its full name under its own `icons`, whole, and D7
keeps one as it keeps a package token. A package's `svg "star.svg"` reads
`<package root>/assets/star.svg`, the app's `<app>/assets/`; so does a package component's icon
param default written as a literal `icon(svg …)`, in the package's design or its Contract (Q1: a
package is self-contained wherever it is used), while an app's `params` line setting that param is the app's and reads the app's. An icon
naming another reads the file beside the line that writes the `icon(…)`. In v1 a package's SVG
must be a one-colour template (Q2 (a); `tamber-icon-svg` otherwise): the web, which alone draws a
file as written, stages the app's `assets/` only (QUEUE).

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
  `@scope/reading/article.contract#Article` for a package's. A package outside the app needs a
  name in its `package.json` (`contract-package-unnamed`, the path fix): its place from the app
  would spell this machine's directories. A root file linked into the app's tree from elsewhere (a symlink, as
  conform's corpus fixtures are) in no named package is the app's own, named where the link sits (one in a named package keeps its name); a file a `use` reaches is its target's (the path fix's follow-up). A param's full name is that
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
  build; a `rem`, `em` or text-style value there is refused (`tamber-range-unit`). A base
  name the design leaves unset, bare or in an expression (D11, stage 12 B2), is checked in
  every column of the scale instead, the refusal naming the first column outside the range
  (`tamber-param-range`; theme2's 01:13 ruling). The default,
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
            | "exports" NL block(NAME NL) | "params" NL block(setting)
            | "component" IDENT NL INDENT "params" NL block(param) DEDENT ;
setting     = PARAM "=" value NL [ INDENT { when } DEDENT ] ;   (as built, stage 7 B)
PARAM       = [ FILE "#" ] IDENT "." IDENT ;   (a full or short param name)
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
9. **Icons and native controls, finished; the design server's row operations** (D9, D10, D13).
   Re-scoped 2026-10-18 (theme2's brief, accepted by the lead, Charlie informed): the icon tokens
   this stage first held were built and met in stage 6 F (their gate: the clone's `symbol:sf/…`
   uses draw on the web from icon tokens, and a missing source fails the build with the target
   named). What it holds now: Apple draws icon paths (an svg-only icon, tinted by a colour
   token); Mac symbol images take their colour; `explain` and `uses` answer icons and params;
   `set-row`, `add-param` and `remove-param` (stage 7 C's, on stage 8 D's `.contract`
   ownership), the server half of LLP 1122 T5; held rows name their tokens (LLP 1122 T4);
   D9's and D8's follow-ups (an authored accent past an `AccentColor` role on the natives, the
   web's authored transition off a look's edges, a row arriving during a delegated track, the
   iPad pointer's `:hover`). Wire: the next window, `tamber/1.12`. Digest break: name it if A
   or B moves the kernel schema. **Gate:** an svg-only icon draws on iOS and the Mac, tinted by
   a colour token, through a live retheme; Mac symbols take `color`; `explain` answers an icon
   and a param; LLP 1122 T5's gate lines through `set-row` (one-line template edit with hot
   reload, a stale digest refused, a failing candidate writes nothing, promote to param and to
   token each one transaction, a `class:` row edits its style's own file); inspect on a held
   node names its rows' tokens; an authored transition does not time a look on the web; an
   iPad pointer shows a `:hover` look; no regressions (release unchanged where unused, conform
   clean, LLP 1061's press corpus unchanged).
10. **Remote authentication: pairing** (D13; with LLP 1122 T6, D12). Split 2026-10-19 (the
    lead's acceptance of theme2's stage 10 brief): pairing codes and scoped, in-memory tokens
    (12 h, revocable at once; `inspect` needs `read`, `hold`/`pin`/`outline` need `write`), a
    TLS-only remote listener with a per-run self-signed certificate whose fingerprint the paired
    client pins (or an explicit `--trust-tunnel`), and the canvas gateway; local and LAN only, no
    hosted relay (§9 Q4's default). Wire: `tamber/1.13`. **Gate:** a paired `propose` client
    cannot write or send commands; revocation cuts off its socket and, separately, its canvas
    through the gateway; an unencrypted remote connection is refused.
    **10b. External clients** (D14): a Figma Variables bridge, a Paper or Pencil client, as
    clients of stage 10. Wire: its additions land after stage 10 closed (2026-10-19), so they
    are the first of `tamber/1.14`, the window stage 11 shares. **Gate:** a changed accent in
    Figma Variables is proposed as a dial change and a changed folded value as a pinned override.
    **10c. A hosted relay** (Charlie, 2026-10-19: "We will prob need one yes"; design to
    follow, its own plan): remote pairing for a team that shares no machine or LAN. What
    stage 10 already fixes, and the relay must keep (theme2's check, 2026-10-19):
    - **End to end, not terminated.** The relay forwards the client's TLS to the developer's
      server unopened (a blind pass-through), so the per-run certificate's fingerprint, shown
      with the pairing code and pinned by the client, still authenticates the developer's
      server, and the relay never sees a token, a design or a canvas. A relay that terminated
      TLS would break pinning and see every secret: ruled out.
    - **The developer's server dials out.** Stage 10's listener only accepts; behind a NAT the
      server must hold an outbound connection to the relay that the relay hands client streams
      down. A new connector beside `--remote`, with the same routing and the same
      `access.remote`.
    - **Peers are the relay's.** The socket's peer is the relay for every relayed client, so
      stage 10's per-peer pairing limit would lump them together: relayed pairing leans on the
      per-code burn rule and a per-relay-session limit; a peer address the relay reports is
      never trusted for anything but display.
    - **Origins.** The relay's public host names become the tool's and the canvas's origins
      (two, as stage 10's two ports; the canvas stays a different origin from the tool and the
      cookie stays server-bound), and the Origin check and `remote.info()` name them.
    - **Tokens unchanged.** Pairing codes, scoped in-memory tokens, revoke and the method table
      work as in stage 10; the relay adds no credential of its own to the protocol (it may
      have an account for the developer's server to connect, outside `tamber/1`).
    **Gate:** set by its plan; at least stage 10's three lines through the relay, and the relay
    shown unable to read a token or a design.
11. **A base spacing and radius scale** (D11), when packages need a shared vocabulary:
    `space.*` and `radius.*` with values taken from each platform's metrics, not a brand's
    4 px grid. **Gate:** two packages restyle together from one app value.
    *As accepted (the lead, 2026-10-19, theme2's stage 11 brief):* started now, with two fixture
    packages as the consumer; it first builds D11's package designs (no package shipped a
    `.tamber`, and only the app's design was read); names `space.xs…xl` and
    `radius.sm/md/lg/full`, each base value from an existing metric or measured, never a
    multiplier, in px/points with no Dynamic Type scaling; an app may override a base name, a
    package may not; only the listed names are reserved; package components stay stage 12's.
12. **A component library** (*closed 2026-10-21; LLP 1139, its gate met, §10 there*): `.contract` packages over Tamber, with params (D15) as their
    tweakable surface and anatomy as checked data, keeping exact1 Facet's rule, "a recipe
    styles parts; it never forecloses slots". Its own RFC. **Gate:** set by that RFC; its
    first consumer is named before it starts.
13. **Finishing the open threads, and the library's second wave** (*closed 2026-10-23, its parts gated
    separately; C, D and E met, A's R4 and B's Figma run and `pen` importer carried; §10 "Stage 13: as built"*):
    the hosted relay's service (R3), external clients, LLP 1139's second wave of library components and
    package icons, the write check's open gaps, and the design tool after its hands-on pass, on `tamber/1.16`.

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
   **Answered (Charlie, 2026-10-19):** pairing as D13 is the model (stage 10, local and LAN);
   a hosted relay is wanted ("We will prob need one yes"), design to follow: §5 stage 10c,
   end to end through the relay, the developer's server dialing out.
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
- **Headings** keep their platform text style's weight on native hosts. Web, neutral and terminal
  looks emphasize title headings through the schema's metrics: 700 for levels 1 and 2,
  600 for level 3. An authored weight wins.
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
  icon's SVG, so a watcher rebuilds on its edit. Every target but a watch draws a template SVG (Apple's page and widget hosts too since stage 9 A,
  where no `sf` is given; a watch still needs an `sf`: the limit stage 6 F had, an SVG no source on Apple, is gone), Apple's an `sf`, and the web
  also a file that is no template, as written; a multicolour (non-template) SVG needs an `sf` for Apple, which draws
  no SVG file (theme2's 17:03 (a); QUEUE: rasterise it into the asset catalog). Else `tamber-icon-missing: <icon> has no source
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
  Symbol, an icon's `sf` included; Apple drew only that until stage 9 A, which draws an icon with no `sf` from its
  path) and `symbol_path` (`SymbolPath`: a role's path, stroked or
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
  `kernel/src/grouped.rs`); since stage 9 A an icon with no `sf` draws its path (`symbolPath` under
  `symbolViewBox`), each of its SVG's paths filled nonzero on its own.
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
  **The import is checked where it lands** (the follow-up, 2026-10-17, `import/landing.rs`):
  each token it adds or sets is laid into the design as `reconcile` writes it (a `when` on the
  importer's own axes it no longer gives taken out), and the design folded against the app's
  vocabulary (`tailwind::import_with`, `dtcg::import_with`; the CLI and the server pass it).
  - A token the design has of another kind is reported first, a colour too ("the design's `x` is
    a length token, not a colour"): its kind as the design folds, or, where it does not, as what
    folds once the lines it refuses are left out. So is a line the edit grammar would not take
    (a colour named `--café`). A token naming a refused one goes with it, a quoted name aside.
  - The import's axes are laid first: one that would leave the design unfolded refuses the import
    whole (`tamber-import-axis`).
  - Each refusal the fold makes that the design did not make before is charged to its cause: a
    laid token's own line; a design token's failing value (a `when` with its default), through
    the tokens it reads as laid (kept `when` lines too), to the laid tokens whose absence clears
    it, or, where none alone does, to those it holds with all the others out ("the design's
    `derived` would not fold once it lands: …"); a dial left unused to the token that no longer
    reads it; anything else to every token laid. Until the design folds.
  - The refused tokens' source values move from mapped to reported, so the count holds; a role
    offer over a refused token is not made.
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
- Apple drew only an icon's SF Symbol (stage 9 A draws its one-colour SVG's path where no `sf` is given; a multicolour file still needs an `sf` there);
- a varying type or family token is refused on a container's inherited rows (`tamber-inherited-axis`);
- `calc()` with `rem` waits on QUEUE's item.

**D3, amended by this stage:** `duration`, `easing` and `icon` token names must be dotted
(`tamber-motion-name`, `tamber-icon-name`), so a transition's words or a symbol's name can't be
mistaken for one. There are 88 symbol roles plus 58 aliases, not 76.

**Stage 6 is closed** (with stage 9's icons).

### Stage 7, commit A: the `params` section (`be8ae002c`..`08dfc82c3`)

- **Grammar.** A component takes one `params` section. Each line is `name: kind = "default"`
  followed by attributes `range`, `step`, `control`, `options`, `label`, `help`, `group` and
  `priority` (a string or a number; any other is `syntax-param-attribute`). The AST holds it as
  `Component.params` (`contract_syntax::params`). `contract fmt` keeps it as written.
- **Identity and naming** (theme2's ruling, 2026-10-17). The loader gives each component its
  identity: its file relative to the app's directory, or `<package>/<file>`, and its name
  (`post.contract#PostButton`). A param's full name adds the param
  (`post.contract#PostButton.radius`); its short name is `Component.param`.
  - Below the IR (the reference table, the fold, the live table, `uses`, the JS target), a param is
    the dotted token `param.f<hash>.<Component>.<param>`. The hash is FNV-1a of the file's path, as
    eight hex digits after an `f`, since a name's segment starts with a letter.
  - The loader rewrites `param.<name>` to that token before anything reads it, where a
    component's row takes its value (a style, shorthand or motion row, an image's `symbol:`
    source, named or positional): a string or a template, through `?:` branches, `match` arms and
    `+`, the places a motion token is written into. An expansion's `let` (a child's shared derive)
    is read by name, a comparison too, so a token in its value is refused as a derive's. Also in its params' defaults and
    `select` options. A quoted word (the quote carried across a template's holes), one after `#`
    (`url(#…)`) and one touching a template's `${…}` are left as written, as is text that is no
    row (a `text`'s content, a `testId`, an image's URL, a `?:` branch of an image's source that is
    no symbol's; an image's other rows take params as any element's do). So a param is read, folded, retained
    (D7) and bound as any token is, and an import alias never redirects it.
  - An argument to another component passes the param: the child's row reads it as its value
    (round 2's change; round 1 refused it). After inlining, a param's token anywhere but a row's
    value (a text, an attribute that is no row, a function's argument, a condition, a state, a
    derive, an action) is refused (`tamber-param-scope`), naming the param.
  - A param named where no component owns it is refused (`tamber-param-scope`): in a `style` or a
    `keyframes`, which any component may apply; in a state's or a derive's value. *Amended
    2026-10-20 (LLP 1139 D3, stage 12 C3; theme2's ruling, one rule for every component):* a
    `style` one component's view alone applies is that component's, and its `param.*` are that
    component's params; one another component applies too is no one's and stays refused. So is one in
    what a row computes (a function's argument, a condition, a comparison, a callback): what it
    computes is data the param's value never reaches, and a duration there would never be written
    in. A package's file is named by the package's declared name: the nearest directory above the
    file, below the app's own, whose `package.json` has a name (under `node_modules`, linked from
    elsewhere, or vendored). It is read from where the file is, never the route a `use` took, so
    two routes, aliases or links to one package are one identity. A package with no name is named
    by its place from the app (`../libs/a/index.contract` outside it). A source text with no path is `app.contract`.
  - Every refusal names the full name, never the hash: the lowering's errors pass through
    `params::shown`, every param's (a bad kind's too). `contract types` names no param, and
    `describe`, `contract tamber ir` and the design's checks fold the params with the design but
    list them only under `components` (`eval` lists none; `check` counts them apart, and admits
    the design by building the app itself when it has params). A refusal or a warning on a param
    is charged to its default, in its declaring file. An option names a param in full.
- **Folded with the design.** The compiler adds each param to the design it checks, as a token
  of its kind (an icon param among the icons), on lines past the design's own. A refusal on one is
  charged to its default's span. An app with params and no design folds them alone. A duration or
  easing param is written into its rows at build, as a motion token is. An icon param may default
  to another icon (a param's or the design's), and its SVG is a source of the build.
- **Checks:**
  - `tamber-param-unknown`: a `param.<name>` the component doesn't declare.
  - `tamber-param-unused`: a param no row of its component reaches, directly or through another
    param. The message says a param read only by a state part lands with stage 8, since state
    parts don't parse yet.
  - `tamber-param-kind`: no kind. A param's kind against its rows is D3's `tamber-kind`.
  - `tamber-param-control`: a control the kind doesn't take, `select` without `options`, or
    `options` without `select`.
  - `tamber-param-range`: a range on a kind without one, a range that isn't `low..high`, or a
    default outside its range in some tuple.
  - `tamber-range-unit`: a ranged length that isn't px.
  - `tamber-param-options`: an option that is no token of the kind.
  - `tamber-param-step` and `tamber-param-priority`.
  - `tamber-param-name`: a design token named `param.…`. The check is the compiler's, since the
    crate checks the param tokens themselves.
  - `tamber-param-collision`: two declaring files whose hashes match, naming both.
- **The IR's `components` block.** `describe` and `contract tamber ir` return it, at
  *tamber/1.10*, the stage's protocol version. Per component it gives the identity; per param, the
  full and short names, token, kind, default (params named in full), values per tuple, range,
  step, control (default `token`), options, label, help, group and priority (default 2).
  `params::resolve` reads a full or short name, and refuses a short name of a component name two
  components share, whichever has the param or neither (`tamber-param-ambiguous`), naming both. B's
  `set-param` and `resolve` take it. A param's name is a field's, so a reserved word may be one.
- **No digest moves.** A param is a token in the reference table.

### Stage 7, commit D: components that aren't Contract (ending `0d2e94e3b`)

- **Grammar.** A design may declare `component Name` over one indented `params` block. Each line
  reads as a dial's, for any kind: `name: kind = value [in low..high] [meta=…]`
  (`exact_tamber::Component`, `Param`).
  - Refused (`tamber-component`): a component with no `params`, or anything else under it; a
    component named twice; a param named twice; a `when` under a param (the app's design sets a
    param per mode, by name).
  - `fmt` lays a param line out as a dial's.
- **Identity.** The design file relative to the app's directory, and the name
  (`app.tamber#MapCard`). `DesignSource.home` carries the path.
  - `params::design_tokens` makes each param a `ParamToken`: `in low..high` becomes its `range`,
    and its metadata become a Contract param's attributes (`range=` is refused here, and so is any
    other unknown attribute, as `syntax-param-attribute`).
  - `param.<name>` in a default names the component's own params (`tamber-param-unknown`).
  - `params::tokens(file)` returns them after the Contract ones, so the checks (`declared`,
    `folded`), `add_to`, `resolve`, the collision check and the `components` block are A's, unchanged.
- **Roots.** `unused` never sees them. Retention keeps every one, and what its default reads
  through any chain of tokens: the code that reads them is outside the views.
- **Bindings** (`tamber gen`). A param is named from `<Component>.<param>`, never the hash
  (`swatchRing`, `SWATCH_RING`); its value is the token name, which the runtimes resolve as any
  token's. Contract params get the same.
  - Swift members now cover colour, length, number and type tokens, not colour alone.
  - `gen kotlin` writes `TamberTokens.kt` in package `exact.app`: `TamberColor`, `TamberLength`
    and `TamberNumber` objects of `const val`s (theme2, 14:17).
  - A collision across names that map alike is refused, naming both.
  - The Apple build writes `tamber.swift`, and the Android reader writes `TamberTokens.kt`
    (compiled beside the hatches), before the app's modules compile (`scripts/tamber-names.mjs`).
- **The sample.** `apps/tamber-host` declares `component Swatch` (`ring`, `ringWidth`). The Swift
  module and the Kotlin hatch stroke a ring through `.swatchRing` / `TamberColor.SWATCH_RING`. The
  smoke reads the ring as it follows `setMode` and the pin, on the Mac and the iOS simulator; the
  ring also draws on x2-android.
- **Not yet:** a live `set-param` reaching them waits for B's server folding (the server's own
  fold adds no params today). Packages' `.tamber` components are QUEUEd.
- **A param's token name may appear in generated code as a value, never as an identifier**
  (theme2's ruling, amending A's): the TypeScript kind unions (`contract types`, `tamber.ts`) and
  the constants in TypeScript, Kotlin and Rust carry it, so a data module returns
  `tokens.<name>.name` as a `ColorToken`. Diagnostics, `describe`, inspect, `set-param` text and the
  DTCG export use the full or short name. The docs say: return the constant, never type the
  literal.

### Stage 7, commit B: the app's value, `set-param` and `unset-param` (`1530e9671`..`ff89cbae9`)

- **The `params` block** (theme2's ruling, option 1). A design may have a `params` block beside
  `theme`. Each line is a param's full or short name, `= value`, with `when` lines. `#` and `/`
  are allowed in names there only, and a kind is not written (it is the component's). `tamber fmt`
  lays it out as `theme`'s.
  - Each line goes in place of its param's default and `when` lines, so every instance shares it.
    One layout does it everywhere (`tamber::params::laid`): the compiler, the motion rows written
    at load, the source graph, the CLI's checks, `plan`, the edits' check, `resolve` and
    `describe`. An icon naming another icon takes its sources after the lines are laid, so an
    alias follows the value set. A range is checked on each value written, the line's own and
    each `when` line's, folded on its own in its param's place (a token's, a `calc()`, a `when`
    no tuple picks), at its own line.
  - A line naming no param of the app is a warning, `tamber-param-unknown`: a moved or renamed
    declaring file must not break the build, and the value no longer applies.
  - A short name two components share is `tamber-param-ambiguous`, whichever has the param or
    neither: one rule (`tamber::params::named`), which `contract_lower::params::resolve` delegates
    to. A second line naming the same param is `tamber-param-twice`. The compiler and the design
    server refuse both alike (`params::entry_errors`): the server's reads refuse such a design, and
    its writes refuse to leave one, except `unset-param` by the line's own spelling, which repairs
    it.
- **`set-param` and `unset-param`** (`tamber::edit`, *tamber/1.10*):
  - `set-param` writes the param's line, making the block at the end when there is none, or sets
    one `when` line. A `when` needs the line first.
  - `unset-param` removes the line and its `when` lines, or one `when`. An emptied block goes.
  - The inverse is the preimage, as every edit's is, so undoing the first `set-param` restores
    absence.
  - The server refuses a name no param answers, and an ambiguous short name. It writes on the line
    the design already has, by that line's name, or by the name an earlier op of the transaction
    used. After every write to an app with params, not only a `set-param`, it refuses a design
    whose params do not fold or fall out of range (`tamber_ops::param_values`), each at its line.
  - The block is found as the parser finds it (a trailing comment allowed), a new line takes the
    block's indent and goes after its last line, comments included; the last line removed takes
    the block with it, comments too. A token's rename renames the values naming it, a param's name
    never, and is proved folded with the params, so a literal word it would change is refused.
  - An icon param's SVG must be the app's file when a write sets it (`tamber-icon-svg`); the
    build's per-target check (`tamber-icon-missing`) stays the build's.
- **The closure** (`tamber::params`). `plan` folds the design with the app's params, each in its
  `params` line's place (`Synthetic::params`; the server reads the app's params by its Contract
  sources' key), and checks an edit laid out with them (`apply_all_with`), so a dial only a param
  reads is read. So a param's value is `live`, and `rebuild` for a duration or easing param, even
  one no instance uses (the planner does not see where motion rows were written). A live one is
  scrubbed as a token is: the wire's scrub plans with `propose` and the dev loop compiles the
  texts.
- **Reads.** `resolve`, `explain` and `uses` take a param's full or short name and answer by its
  full name, their errors too, folded with every param (so a design token's `uses` follows it
  through a param). An ambiguous short name is refused, never read as a design token. `uses` gives
  every instance's row; a motion param's sites are its source literals, as a motion token's are.
  An icon param, like an icon, is read by `describe` and `resolve` (QUEUE: an icon's `explain` and
  image sites in `uses`). `describe`'s `components` gives `set`, the app's value. It gives
  `pinned`, each token or icon the default names that some value set (the line's own or a `when`
  line's) no longer reads, directly or through another token, an icon's alias or a param: D14's
  mark, "no longer follows `radius.md`".

### Stage 6 C's follow-up: type and family tokens as properties on the JS target (draft for theme2)

- **What changed.** The JS target wrote a type or family token as the kernel resolves each longhand,
  a literal, so one that varied by an app axis was refused there and an edit to one rebuilt. Now each
  row such a token writes is a property per tuple, as stage 6 B made lengths and numbers: a type
  token's `--x-<name>-style`, `-weight`, `-size`, `-leading` and `-family`, as it writes them; a
  family token's `--x-<name>`. One that varies has them always; in a development build every one
  does (`measures::type_properties`).
- **Each tuple's rule declares each part** as the template writes the literal's: a size, or a line
  height that is a length, as its CSS (a text style its `rem`); a family as its plan stack's CSS
  (`measures::part_css`).
- **No sentinel marks a part.** A style is `normal` or `italic`, a weight a whole number, a family a
  stack the plan declares, so none can be bound to a value no app writes. The declaration a row
  writes is found by its property instead (`Modes::type_rows`): a varying token reaches only a text
  with no element children (`tamber-inherited-axis`), whose own rows these are.
- **A bound row** (`font=(c ? "type.a" : "type.b")`) maps the name to its part's `var()`.
- **Live.** The design entry lists each row's property and its CSS per tuple (`parts`), and the page
  sets each one; a type token is no longer `inline`. A change to its style, weight, size or line
  height is live and scrubbable on the JS loop; one that changes the rows it writes or a family
  rebuilds, by the design's shape, as everywhere.
- **Release** output is byte for byte the same where no type or family token varies.
- Out of scope: Bold Text, and the weight and leading a text style implies where the token writes
  none (they stay the default tuple's literal).

### Stage 7, commit E: the clone declares params (clone `d2406b4`, on its main)

The Bluesky clone's `PostButton` declares `fill` and `height`, and its post action row
(`Controls`) declares `gap`, `tint`, hit padding and `radius`, with groups, priorities, controls and
`select` options. `Controls` passes `tint` to its glyph as the child's row value, which commit A
admits. The defaults equal the literals they replaced, so the screens are unchanged.

### Stage 7's gate: met (2026-10-17; the run is `~/x2theme/stage7-gate-run.txt`)

The clone at `d2406b4` was run on exact2 `272f117e3`.

| Gate line | Result |
|---|---|
| The clone's post action row and Post button declare params | **Met.** |
| The design tool lists them by group and priority with their controls and `select` options | **Met through `describe`**, which gives group, priority, control and options per param. The tool's own panel is LLP 1122 T3, not yet landed. |
| A `set-param` reaches every instance live, and one on a duration param rebuilds instead | **Met** on the wasm loop, the JS loop, the iOS simulator and the Mac. `tint` and `gap` reached all six action rows live, and the natives kept their plan ("2 tokens to 2 builds, no generation"). `Drawer.fade` (a duration) is `rebuild`, and the page and natives took a new plan. |
| A Swift native view's `.tamber` params appear beside them and draw through `ExactTamber(view)` | **Met** on the Mac and the iOS simulator: tamber-host's Swatch ring follows the mode. At the gate run the iOS smoke's relaunch never opened its agent socket; that was the gate simulator's own state. reload1's rerun on main `78b93c82a`, on a freshly made simulator, passes in full: the relaunch, the ring's colour change, the pin, the scheme and the font. |
| An unused param, an ambiguous short name and a `rem` value on a ranged length are refused | **Met.** |
| Renaming an import keeps an override on its component | **Met.** |
| No regressions | **Met.** Plans of apps without params are byte-identical before and after stage 7, apart from a format-digest header another lane moved. |

**Deferred:** commit C (`add-param` and `remove-param`, the server owning `.contract` params
sections) lands with LLP 1122 T5's `set-row`. A refuses an unread param, so either op only works
paired with a row rewrite. Its `.contract` ownership infrastructure arrives with stage 8 D.

**Queued, then closed:** a stale duration seen once on the JS loop after writes queued behind a
slow first verifying build (not reproduced in three replays). Closed by rt1's JS-loop builds
(`7a1c8e84f..bf2e295c2`): a build that started before a newer edit is stopped or set aside, never
published; the clone's sequence (`Drawer.fade` 400ms then 250ms behind slow builds) now reads 0.25s
on a fresh page.

**Stage 7 is closed.**


### Stage 8, commit A: state parts, grammar and lowering (draft for theme2)

- **Grammar** (`contract_syntax::parser::states`). `:active`, `:hover`, `:focus-visible`, `:disabled`
  or `:checked` alone on a line, then its rows indented, in a `style` (literal rows) or under a node
  before its children (computed rows, as the node's own). Each row is carried as an attribute named
  for the state and the row (`:disabled background-color`, `contract_syntax::state`), so a class's
  and the node's merge as any row does and the node's wins. `syntax-state`: an unknown state, a
  combination, a state twice at one place, a block inside a state, a state after the node's
  children. `contract fmt` keeps the block, its `:` against the name.
- **Rows and targets** (`contract/lower/src/states.rs`). A runner-known state takes
  `background-color`, `color`, `border-color` (each side's longhand too), `opacity`, `box-shadow` and
  `-exact-tint-color`; a host-known one `background-color`, `border-color`, `opacity`, `box-shadow`
  and `-exact-press-scale`. Another is `lower-state-row`. A host state's row is a literal
  (`lower-state-literal`; a token's name is one, theme2's ruling (b)). A part on a native control
  (`controls::control`, or a `Control` node) or on a cell list's row (through its `for`, `if` and
  `match` regions) is `lower-state-target`.
- **Runner-known states fold** into the node's rows, per longhand: `disabled ? D : (checked ? C :
  B)`. B is the base the class and the node compose, a `border-color` split into its sides and a
  side a `border` or `border-top` writes as its component, so a state's `border-color` overrides
  each side the base wrote, longhand or not. `disabled` is the node's own `disabled`; `checked` is
  `checked || aria-checked || aria-selected || aria-pressed`, the last compared with `"true"` unless
  it is a bool. `:disabled` sets `DisabledStyled`, beside the heuristic.
- **`lower-state-dead`** (a lint, theme2's ruling (a)): a node's own `:disabled` where it writes no
  `disabled`, or its own `:checked` where it writes none of the four. A class's part is not judged.
- **Host-known states are recorded** (the plan break, ruling (c), shape agreed with x2-perf4): a
  `looks` table, `{node, state, bindings}` (0 `:active`, 1 `:hover`, 2 `:focus-visible`) in node
  order, one look a state a node with the class's and the node's rows composed by longhand, each a
  literal style row as a static row's (a token's name stays a name, for a live batch and a mode). A
  node with no look has no row, so a plan without state parts pays 4 bytes (a nodes column cost the
  clone's plan 159,596 bytes, 4%: theme2 asked to key the table by node). No host reads it until B.
  FORMAT_DIGEST moves to `0x73dd_d2b7_978c_aec0`: every host takes a new binary (plan decode is
  generated), and none applies looks yet.
- **LLP 1001** declares the `:checked` deviation.
- **Tests:** `contract/cli/tests/it/state_parts.rs`: `:checked` under `:disabled` on a checked,
  disabled toggle (the gate line); a state's `border-color` over a `border` and a side's longhand;
  the node's part over its class's; the disabled-look mark; every refusal; the lint; looks one a
  state, composed; fmt.

### Stage 8, commit F: params read by state parts (draft for theme2)

- **Stage 7 A's refusal is lifted.** A state part's row is a row where a param may stand
  (`params::rewrite`'s `row` reads `:active background-color` as `background-color`), so
  `param.x` there is rewritten to its token and reaches the param: D15's `PostButton.pressed`,
  read by `:active` alone, compiles and folds. `tamber-param-unused`'s message no longer points at
  stage 8.
- **What it does at run time** is A's and B's: a `:disabled` or `:checked` row reading a param
  folds into the node's row, which reads the param's token, so a live `set-param` reaches it as
  any token row's; a host look's row keeps the token's name as a static row does, which B's hosts
  resolve per node tuple and follow live (theme2's ruling (b), 17:37).
- **Tests:** `state_parts.rs`'s `a_param_read_only_by_a_state_part_is_reached` (refused as unused
  before, `tamber-param-unused`).

### Stage 8, commit D: the server owns `.contract` state parts (draft for theme2)

- **Owned files.** The server's files are the design's, then the app's own Contract sources
  (`tamber_contract::owned`: the source graph's `App` units inside the app, never a package's or
  `node_modules`). `hello`, the snapshot, the journal and its recovery, a transaction's digests and
  `commit`'s set take them as they take a design file; the JS wire's watcher announces a hand edit
  of one with `client: null`, and a `set-state` without a `file` holds every Contract source for
  `commit`'s guard. This is stage 7 C's infrastructure (its plan's points 1, 4 and 5), which
  LLP 1122 T5's `add-param`, `remove-param` and `set-row` will use.
- **`set-state {style, state, row, value, file?}`** writes `row = value` in the `style`'s `:state`
  block (the block and the row made when absent; the row's value replaced where it is written);
  **`unset-state`** removes the row, and the block's header with its last row. The file is the one
  named, or the owned file declaring the style, refused where none or two do. `contract fmt` then
  lays the file out, and the whole app is compiled in memory with it (`with_contract_texts`, beside
  `with_design_text`) before anything is written: a refusal answers the build's diagnostics and
  writes nothing. The class is `rebuild`, so there is no scrub.
- **The inverse** is a line restore of the preimage (`exact_tamber::Op::Restore`, applied to the raw
  text by `tamber_contract::restore`, bypassing fmt), so an unformatted file comes back byte for
  byte (LLP 1122 D6).
- **Deviation from LLP 1122 D6's one-line diff:** a `.contract` write runs `contract fmt` over the
  whole file (as the approved plan said), so in an unformatted file a one-row `set-state` lays out
  its other lines too; in a formatted file it is one line. The inverse still gives the unformatted
  bytes back.
- **A node's own state part** is `set-row`'s (LLP 1122 T5): a `set-state` naming a `node` is
  refused, and the "no file declares `style`" refusal says so.
- **History and revert** (the lead's 06:04 ruling, a follow-up): a `.contract` file's commits
  read as its state rows' edits (`what: "state"`), `code` where other lines changed, which a
  revert leaves alone; a change with no state row is the app's code and not listed.
- **Wire:** `tamber/1.11`, the window stage 7's close opened (theme2, 20:30).
- **Tests:** `contract/cli/tests/it/tamber_serve_contract.rs` (set, replace, undo to the
  unformatted bytes; unset with its block; a refused compile writes nothing; an unknown style;
  `propose` says `rebuild`); `scripts/tamber-server.test.mjs` (a hand edit announced, `commit`
  takes a written `.contract`, one dirty at its baseline refused).

### Stage 8, commit B1: `:active` as the press paint; looks on the runner and the web (draft for theme2)

Built to D8 as amended 2026-10-18 (Charlie's option B; theme2's 06:14 ruling on the replan).

- **`:active` lowers to the press paint** (`contract/lower/src/states.rs`). Its rows are written on the node, the class's then the node's (the node's wins), each keeping its `:active` span for inspect and the map:
  - `background-color` → `-exact-press-background-color`;
  - `color` → `-exact-press-color`;
  - `-exact-press-scale` → the node's own;
  - `opacity` → `-exact-press-opacity-to`, a new row, bit 215, lowering's alone. An author's is `lower-unknown-attr`, naming `:active opacity`, and `contract vocab` doesn't list it.

  Tokens stay names, as in any row.

  Refused:
  - `border-color` and `box-shadow` in `:active` (`lower-state-row`: the press can't paint them yet);
  - `-exact-press-scale` in `:hover` or `:focus-visible` (`lower-state-row`, pointing at `:active`);
  - a node, or its class, saying a press paint both ways (`lower-state-press-paint`): `-exact-press-background-color`, `-exact-press-color`, `-exact-press-opacity` or `-exact-press-scale` beside the same `:active` row.
- **Where an `:active` row is written** (theme2's 06:14 condition). The map's binding for a lowered row carries `"state": "active"` and `at`, the `:active` row's file and span (`sites::Origin::State`; its `origin` is still the node's or class's). `uses`, and so explain, name that line, not the node's. (Since LLP 1122's queue item (1), on theme2's 2026-10-20 ruling: the part is the press row binding's `states` entry, `{state: "active", origin, at}`, and the binding has no base fields; `:hover` and `:focus-visible` rows are entries of their rows' `states` too.)
- **The absolute press opacity.** The press eases from the opacity shown at the press, a hover or focus look's included, to the target. For that node it replaces D7's clamped share. A hand-written `-exact-press-opacity` keeps the share.
  - Web: `--exact-press-opacity-to`. The glue's `PAINT` list takes it, and index.html computes `base + (to − base) × paint`.
  - Apple, Linux and Android: B2 and B3.
- **Looks are `:hover` and `:focus-visible` only.**
  - The looks table's state 0 is retired: the reader refuses it, and codes 1 and 2 are kept. `format.json`'s comment says so, so FORMAT_DIGEST moves to `0x973e_e759_65ad_8001` (over main's at the push).
  - The runner resolves a node's looks rows in its tuple (`set_plan_style`) and sends `Op::SetLooks` (op 9: at most 2 looks, states 1 and 2, ascending; look rows only, set or cleared). It sends them at mount and when its tuple moves (a mode, a pin). `ThemeIndex::looks` keeps those that read a token, and `set_tokens` (a live batch, a role's value) sends them again.
  - SCHEMA_DIGEST moves to `0x8b10_52ba_0e04_2f0b` (op 9 and bit 215, over main's at the push), in stage 8's window. Every host takes a new binary.
- **The web, wasm** (`host/web/src/host/looks.rs`). A node with looks gets a `looks` batch op: one rule per state in the page's `#exact-looks` sheet, written as follows.
  - `:hover` goes under `@media (hover:hover)`, then `:focus-visible`; none applies on a disabled node (`:disabled`, or a box's `disabled` of any value but "false").
  - The rows are `!important` over the inline base, and below the shell's pressed-paint rules, which are `#exact-root`-scoped. So the press paint shows over a hover.
  - They have no transition: a look snaps, under Reduce Motion too.
  - The sheet goes with the last plan at a boot. An explicit clear (`unset`) crosses written with no value, as a static row's.
- **Round 2:**
  - A text with a look never folds into its parent's box (`element::folded`, `holds_folded`).
  - The glue takes no press on a box whose `disabled` is any value but "false".
  - The press paint re-reads what's beneath when a sheet or the root changes under a hold (a look's rule, the theme, the JS target's modes).
  - `-exact-press-opacity-to` is refused in a `style` block, not suggested for a typo, and not described by `contract vocab`.
  - `uses` lists a token a `:hover` or `:focus-visible` row reads, at its node, with its state.
  - Round 3 (the last):
    - A disabled box takes no press by key either.
    - The design tool's inspect join and the agent's `layout` carry an `:active` row's `state` and `at`, and the inspector names it `:active opacity` and so on. Its edit is `set-state`'s; a token rung stands.
    - `outline` finds a node from a `uses` location that is a part's row.
    - The JS target's CSSOM token and tuple writes call the glue's `repaintPress`.
    - The wasm looks sheet is written once per batch (a microtask).
  - theme2's 07:06 question: a node's own `:active` where it has no `press` never holds, and is `lower-state-dead` (a lint, as `:disabled`'s); a class's part is not judged. A pressable link (`link`, `button`) with an `href` and no `press` is linted too, as unreliable: only Apple presses it, and the web by touch (theme2's 11:48 ruling; the hosts' disagreement is LLP 1061's, on QUEUE); a `text` link never holds it.
  - **Deviation (QUEUE):** on the web, a node's own `transition` on a property a look sets times the look's change. The look rule can't keep it out.
- **The JS target** (`host/web-js`):
  - The static tree applies each node's looks rows (`SetLooks`), and the template carries their declarations. A varying token's sentinel becomes its `var()`, as in a class, and look declarations join the sentinel scan.
  - Each distinct set of looks is one rule group, `[data-exact-looks="N"]`, after the classes, as unscoped as wasm's, so the shell's pressed paint wins over it; the node takes `data-exact-looks`.
  - `:active` reaches the page as D7's custom properties through the template.
- **Tests:**
  - `state_parts.rs` and `state_looks.rs`: each lowering, the refusals, one spelling, the internal row, retired state 0, looks re-sent by a live token (and not by one only the base reads), `unset`.
  - `host/web/tests/it/looks.rs`: the press rows, the rules in order, a live batch re-sending them.
  - `host/web/tests/looks.test.mjs` (Chrome):
    - a hover at once;
    - the press easing from the hovered 0.8 to its absolute 0.6, its paint over the hover;
    - a base change while pressed;
    - Reduce Motion (press colours ease, hover snaps);
    - a disabled box taking none.
  - The kernel's op round trip and its refusals; `modes_tests.rs`, the JS target's rule group.
  - LLP 1061's press tests unchanged.

### Stage 8, commit B2: Apple (iOS and Mac) (draft for theme2)

- **The absolute press opacity** (`PressPaint.swift`). `press_opacity_to` is a pressed paint row: it makes the node `authorsPress`, so there is no dim and no ink wash. A full press takes away `shown − to`, where "shown" is a hover or focus look's opacity, else the node's own, through the same additive `pressOpacity` animation. That depth is recomputed when the base moves live or a look comes or goes mid-press. A hand-written `press_opacity` keeps D7's share.
- **Looks** (`host/apple/src/looks.rs`). The node's style dictionary carries `"looks": {"hover": {…}, "focus_visible": {…}}`. Each look's rows are written as the style's own (colour pairs, roles), and its opacity is a number.
- **How Swift shows them.** Swift keeps the engine's style (`presented` → `looked` → `pressPainted`).
  - While the pointer hovers the node (the Mac) or its focus is visible, the look's background, border colours and shadow stand in, at once. The focus's go over the hover's, and the press's paint goes over both.
  - The look's opacity is an additive `lookOpacity` layer animation that snaps.
  - None of this applies on a disabled node.
- **Mac hover** (`Mac/HoverMac.swift`). `setHoverPath` passes its whole path to `hoverLooks`. So a node shows its `:hover` look whether or not it hears `hover`, and only nodes with a handler get the runner's hover event. The native-module `hover(_:_:)` moved to that file too (PresenterMac.swift is at its cap).
- **iOS** has no hover (an iPad pointer's hover is a later step), and no focus-visible yet.
- **Round 1:**
  - A pressed share is of what shows, a look's opacity included, as on the web and Linux.
  - A change of `disabled` shows or drops the looks at once (`relook`).
  - A depth re-aimed mid-ease re-aims from what shows (`segment`).
  - A look's `currentcolor` is the node's colour, and a row a look clears crosses null (an opacity as 1), so the base's goes.
  - A re-entrant hover publishes only the newer path's looks, for views still attached.
- **Round 2:**
  - The press's opacity fade is always `-depth × progress` of the press's own ease, held or easing either way. So a look's opacity that comes or goes mid-press moves what shows at once, as on the web and Linux, and the tracked segment is gone.
  - A side a look clears is the node's colour (`currentcolor`).
  - An opacity or background track a press row or a look names stays sampled by the engine, not Core Animation, so the additive layers compose on the rendered value.
  - A sheet's or a popover's own window shows `:hover` looks through its pointer tracking.
  - QUEUE: looks on an inline run, which have no view; and a look that comes while a track already plays.
- **Round 3 (the last):**
  - An opacity or background animation that a press row or a look names is engine-sampled on both platforms, in the common eligibility, not only for box-lowered properties.
  - The press's clock moves on every press and release, a depth of 0 included.
  - The opacity fade is the press's own ease, the same curve and start (`beginTime`), so what shows is `-depth × progress` at every frame.
  - A press easing out reads its depth from a new style too.
  - A sheet's or a popover's window keeps its hovered path on a child's exit.
- **Tests:**
  - `StateLooksTests` (Mac and iOS):
    - `:active`'s paint with its absolute 0.6 (depth 0.4) and no dim;
    - a base change while pressed;
    - on the Mac, hover and focus looks at once, with the press easing from the hover's 0.8 to 0.6, then back;
    - a disabled node taking none.
  - `looks.rs`'s dictionary test. `PressPaintTests` and `PressFeedback*Tests` are unchanged.


### Stage 8, commit B3: Linux, and Android through it (draft for theme2)

- **The absolute press opacity** (`press.rs`, `press::present`). After the hover and focus looks' opacity, the press moves `shown.opacity` toward `press_opacity_to` by its progress. `press_opacity` keeps the share. `press_opacity_to` counts as pressed paint (`paints`), so a list row's wash stands aside.
- **Looks** (`paint/presented.rs`, `BoxPaint::states`). While its `:hover` and then its `:focus-visible` look hold (`Presented.looks`), their background, border colours and shadow replace the box's own, at once. The press's background goes over them.
- **When a look holds** (`presenter/feedback.rs`, `refresh_looks`):
  - `:hover`: the mouse's hit and its ancestors that have one, as CSS matches. A finger hovers nothing.
  - `:focus-visible`: the focus where its ring shows.
  - A node whose looks change is marked for kept rows, and none applies on a disabled node.
- **Round 1:**
  - The engine's presentation cache starts from the committed style (`unpresented`), so a look's or a press's opacity is never kept in it.
  - The hover looks follow a resting pointer through `hover_at`, as hover handlers do.
  - A focus with no `:focus-visible` look marks nothing.
  - A look's `currentcolor` is the node's colour, and a cleared opacity is 1.
  - On Android's Canvas reader, an opacity, background or colour track the host paints over (a press row, a look naming it) is not lowered: the engine samples it, and the composed value is drawn.
- **Round 2:**
  - A reader layer takes the opacity as its alpha only when it plays the opacity. Otherwise the painter draws it, so a lowered scale under a look's or a press's opacity fades once. Before this, a style opacity under 1 with a lowered transform faded twice.
  - The hover looks follow the point while a contact is held (`follow_pointer`).
  - A row a look clears is CSS's initial: transparent, `currentcolor` or no shadow. `paints_over` counts a cleared row.
  - A look's `currentcolor` follows a colour transition.
  - A disabled node shows no press, its release ease included.
  - The shown opacity is clamped before the press composes on it.
  - QUEUE: a look that comes while a track already plays on the reader.
- **Round 3 (the last):**
  - An engine-sampled animation is never also a reader track, beside a transition that keeps the node's layer open.
  - A colour transition stays engine-sampled where a look reads `currentcolor`.
  - The hover looks follow a display's acknowledged frame (`follow_pointer` after `display_complete`).
  - A `background-clip: text` background shows the states' colour.
  - A look painted or taken off, and a focus with a look, take the full painter rather than flow damage.
  - Not done (nit): an inset border's shading of a look's side colour.
- **Android** replays the display list Linux records, so it needs no Kotlin change.
- **A follow-up for every host** (theme2's 08:13 ruling): a `:hover`, `:focus-visible` or `:active` part on a run inside a paragraph is `lower-state-target`: a run's press, hover and focus are its paragraph's. A run has no view on Apple and no box on Linux, and D7's press rows paint no run there either, so `:active` is refused with them. Runner-known `:disabled` and `:checked` on a run fold as before. Android forwards no mouse hover, so it shows none.
- **Tests:** `presenter/looks_tests.rs`:
  - the hover at once with its 0.8;
  - the press easing to its absolute 0.5 with the hover's border beneath;
  - the hover again after the lift;
  - hovering a child;
  - a disabled node, and a finger leaving no hover;
  - `press_paint_tests.rs` unchanged.

### Stage 8, commit C1: the control-role table, measured (draft for theme2)

- **`controls`** in `kernel/tables/schema.json`, beside `_metrics`: one row a (control, parameter),
  `[control, parameter, ios, macos, web, linux, windows, terminal, android]`. A control is `switch`,
  `checkbox`, `radio`, `range`, `progress` (with a value), `spinner` (`progress` without),
  `select`, `button-prominent`, `field` or `textarea`, with a state
  part's state after a `:` where the row is that state's (`button-prominent:disabled`); a parameter
  is `accent-color` or a colour role (`Field`, `FieldText`, `placeholder`). Each host cell is
  `exact`, `best-effort` or `none`, with `: why`. `build.rs` validates every part (the state too)
  and emits `generated::CONTROLS`. No new role (theme2's ruling (1)); the off track waits for a
  design that needs it.
- **SCHEMA_DIGEST moves** to `0x278f_080f_af7d_307f` (every host a new binary; no host reads the
  table). It lands in stage 8's break window with B's move, one new binary per host.
- **Measured before it lands:** `scripts/measure-controls.mjs` over `scripts/fixtures/control-roles`
  (the authored build against one with neither), on the web, the iOS simulator, macOS, the Linux
  painter and the Android emulator. `docs/control-roles-measured.md` records every cell with its
  ΔE00, pixels and provenance; `scripts/measure-controls.test.mjs` holds the table to it. Windows
  is the Linux painter's row (inferred, not measured); the terminal is `none` by construction.
- **What it found:** `accent-color` reaches every control on iOS but its spinner, and every one but
  `select` and the spinner on the web, the painters and the Android reader; a disabled switch,
  checkbox, radio or range is best-effort (dimmed) everywhere it is honoured. A disabled prominent button is the
  platform's on iOS, the web and the painters, the author's on the reader. `Field` reaches only the
  reader and the Mac's textarea, `FieldText` Apple's fields and the reader, `placeholder` every host
  but the reader. Each `none` that is Exact's own gap says so (theme2's 07:32 ruling) and has a QUEUE
  line. The Mac's accent cells are read from AppKit's API: its agent capture here is an inactive
  window, which draws every control grey (QUEUE: measure in a key window).
- **Round 1 (Astra, Grok):** Android's column is the Canvas reader (`--reader`, the fixture as an APK
  in a scratch checkout), not the Linux host under adb; best-effort is theme2's rule; `progress`
  is split from `spinner` and `field` from `textarea`; disabled switches, checkboxes, radios and ranges are rows;
  a frame too small to read is `unmeasured` and fails the run.

### Stage 8, commit C2: `tamber-unreachable` (draft for theme2)

- **A lint** (`contract/lower/src/control_roles.rs`): for each target the app ships (app.json
  `host`, `deploy.store`, `deploy.binaries` and its target crates, as icons read them), each
  control the app draws, and each parameter it sets there, a `none` cell of `CONTROLS` warns once a
  (target, control, parameter), at the override: `accent-color on macOS switches: NSSwitch draws
  the system accent (the first at line N; LLP 1121 D9)`. `best-effort` says nothing (theme2's ruling (2)).
- **Set** means an `accent-color` row on the control, an element around it or a class of either,
  or the design's `AccentColor`; a field's parameter, the design's `Field`, `FieldText` or
  `placeholder`, unless the field writes its own `background-color`, `color` or `::placeholder
  color` (a node's own row wins over the role).
- **The platform's own state** is a row: `button-prominent:disabled` (a prominent button that
  writes `disabled`) names iOS's grey disabled glass, its why pointing at a styled pressable (D8).
- **Tests:** `contract/cli/tests/it/control_roles.rs` (the Mac's switches warned, never the web's;
  best-effort silent; nothing set, nothing judged; a field's own row exempting its role; the iOS
  disabled glass line).

### Stage 8, commit E1: held states: the `hold` command, the server, the web and the agent's `holdState` (draft for theme2)

Built to the approved plan (`~/x2theme/x2p4-s8e-plan.md`; theme2's 07:48 ruling and its conditions), under the one 1.11 bullet.

- **The server** (`scripts/tamber-builds.mjs`, `scripts/tamber-server.mjs`):
  - `hold {client, build, site, instance, state}` needs write scope and a present client. It names a build that holds states; another is told (409, `unsupported: "hold"`).
  - One hold a build, its client's. A new one replaces the old; `state: null` lets it go; the client going lets its holds go.
  - It's sent as the build channel's `hold {revision, hold}`.
  - **Replay.** A build that registers again gets the hold under the same plan. A build that runs another plan (a `planDigest` that moved, at a status or a registration) gets it at the one site of the new plan's map at the old site's source location (`correspond`, through `sitesAt`). Otherwise the hold is cleared with `hold-cleared` to every client.
  - A build's `held` report is every client's event (07:48 (c)). One that lets a hold go for `input`, `disabled` or `gone` ends it at the server.
  - Holds don't survive a server restart, as pins don't.
- **The web, both targets.** The input glue's `exact.hold(el, state, dropped)` holds the real state, handlers never told:
  - `active`: the press of the node the press resolves to (`[data-exact-on~=press]` up from the site's element, as a real press), via `showPress`, so the press paint and scale ease as a real press's, at once on the agent's clock.
  - `hover` and `focus-visible`: `data-exact-held`, which `looks::rules` now writes beside `:hover` and `:focus-visible` under the same disabled guard (07:48 (b)).
  - A disabled node refuses one. Real input on the node lets it go: a press, a key, the pointer over it for a hover, the focus for a focus. So does its disabling or going (a mutation observer, only while held).
  - The wasm page's `dev.js` and the JS target's `inspect-dev.js` resolve a frame's site and instance path to exactly one view, as a refresh does, and report `held`. Both register `hold`.
- **The agent's form** (LLP 1012 amended): `{"op":"tap","id":V,"holdState":…}`, and `tap <target> hold-state active|hover|focus-visible|none` in the CLI. It calls the same glue and replies `{tapped, holdState, delivery: "recognized"}`.
- **What is held** (LLP 1122 D8):
  - `layout`'s node answer, and a selection made in inspect mode, carry `held {state, rows}` by wire name: the press paint's rows for `active`, the look's held rule's rows otherwise.
  - The server's join gives each row its `state` and its `source`: an `:active` row's part's place (the map's `at`), a look's row its node's.
- **Tests:**
  - `tamber-builds.test.mjs`: one a build, refusals, unsupported, `held` to every client, replay under the same plan, at the corresponding site, cleared otherwise, input letting go, the client going.
  - `tamber-server.test.mjs`: through the method, a present client, the next plan's site.
  - `host/web/tests/hold.test.mjs` (Chrome): `active` resolved to the press node with its paint and rows; no press there is `gone`; `hover` by its held rule with its rows; real input, disabling and going letting go; a disabled node refusing.
  - `tamber-inspect.test.mjs` (held rows joined), `agent-keys.test.mjs` (the CLI word), `looks.rs` (the held rules).
- **After review round 1 (Astra, Grok):**
  - Revisions: every hold frame, a replay included, has a new one, and only a report naming the last one sent counts (`held` requires it). A late echo or a superseded frame's `replaced` is dropped. A let-go at that revision ends the hold, an agent's hold in its place included.
  - A registration with no hold is sent `hold: null`; a replay that finds no single site sends `hold: null` as well as `hold-cleared`. A hold or its end announces `builds`.
  - A hold `unsupported` is checked against the hold revision, not the pin's; it ends the hold and goes to its owner alone (07:48 (c); an event's `only`, which the subscription and the long poll honour).
  - The wasm dev loop's new generation lets the hold go unsaid (`exact.holdQuiet`, before the tree goes) and takes it again itself under the same plan; under another, the server's replay does once the status names it.
  - The JS target: hold frames run one at a time, and one a newer frame replaced while it waited is dropped; the runtime is imported from beside `app.js`; `layout` and the selection carry `held`.
  - No single view there lets the previous hold go too. Instance paths compare with sorted keys.
  - `active` resolves to the innermost press handler only (07:48), not a bare styled button. A real press on the node as it is held ends there: its release leaves the held paint.
  - Looks held snap, on and off, an authored transition or not. The pointer or the focus leaving the node lets a hover or a focus go.
  - Held rows a token or a role writes carry `computed`, and the held answer says `provenance: "incomplete"`.
- **After review round 2 (Astra, Grok):**
  - The wasm loop: only the latest hold frame applies (a same-plan replay queued before a newer clear does not), and a frame a generation swap overtook while it waited is left unsaid to `rehold` (same plan) or the server's replay; a clear always applies. Taking the same frame again (resent, or `rehold`'s) reports no `replaced`; the JS target likewise.
  - The JS target: an instance path that does not resolve matches nothing (not the empty path); an unreadable inspect table lets the previous hold go and refuses this one by name.
  - The server: a let-go closes its revision (a late applied report of it is dropped); an applied report with no hold kept applies nothing; a hold `unsupported` clears `held`; a hold taken before the build named a plan takes the first plan it names; the owner-only field is `only` (`to` stays the inspect handoff's).
  - `active` resolves to the innermost press handler only, as the ruling says; `heldOf` answers for the element named too (a label inside the press), with the press's rows.
  - The wasm agent's `holdState` waits for the glue as the JS agent's does. `hello`'s builds name `hold` and `held`.
- **After review round 3 (Astra, Grok):**
  - The wasm loop binds each hold frame, as it arrives, to the generation it addresses and that generation's plan digest. One a generation swap overtook applies only once the swap is done and under that same plan; otherwise it is left unsaid to the server's replay (no `gone` ahead of the new plan's status). `rehold` binds the frame afresh.
  - A revision the page let go of itself (input, disabled, gone) is not taken again when a poll resends its frame (both targets).
  - The JS target: a table of a rebuilt plan (the page reloads to it) refuses nothing; the new page's registration replays the hold. A leaving view (`data-exiting`) is no match on either target, and a held node that starts to leave is gone.
- **After the re-check (Astra):** a frame bound at arrival could still address the old plan once the new runner was in. The server now names the plan a hold's site addresses (`hold {revision, hold, plan}`), and every host takes a frame of another plan as nothing, unsaid, for the replay to correspond. Ended revisions are forgotten at a lower revision (a new server's), and a refusal ends its revision too.
- **After the second re-check (Astra):** a hold from before a new stream (a server restart, a reconnect) neither ends a revision nor reports a let-go on the new one (ended revisions and reports are scoped to the stream and the server's run), so a new revision of the same number still applies and the new server's hold is not ended by the old one's. The protocol's `hold` bullet names `plan`.

### Stage 8, commit E2: held states on Apple (iOS and Mac) (draft for theme2)

- **`HeldState.swift`.** One hold an app, on the host's own tracking, handlers never told:
  - `active`: `pressed` on the node the press resolves to (up to the innermost node with a `press` handler, 07:48), through the same `pressChanged`, so D7's press paint and scale ease as a real press's (at once on the agent's clock). A held press plays no haptic. A real release on the node lets it go (`input`).
  - `hover` (the Mac): `lookState(hovered: true)`. The pointer coming over the node or leaving it lets it go; moving within it does not. iOS answers `hover` unsupported (D8; the iPad pointer is later).
  - `focus-visible`: `lookState(focus: true)`; the focus ring's change lets it go.
  - A disabled node refuses one; its disabling (`disabled`) and its going (`gone`) let it go.
- **The build channel** (`BuildChannel.swift`, `Inspect.swift`): `hold` among the capabilities; `hold {revision, hold}` resolves the site and instance path to exactly one view (structural instance compare), as `refresh` does, and reports `held`/`unsupported` with the frame's revision. No single view lets the previous hold go too; the same frame taken again replaces nothing.
- **A dev loop's new generation** (`ExactApp.applyTogether`): a held node going with the old tree is let go unsaid, and the hold frame is taken again after the swap under the same plan (`rehold`); under another plan the server's replay does, as on the web.
- **The agent**: `{"op":"tap","id":V,"holdState":…}` on both platforms; `layout`'s node and the inspect selection carry `held {state, rows}` (the press rows by wire name for `active`, the look's rows otherwise).
- **Tests** (`HeldStateTests`, Mac and iOS): a held press is the real press with its paint and rows until a real release; a disabled node refuses and its disabling and going let go; a new generation lets a hold go unsaid; the Mac's held hover goes when the pointer comes or leaves, not as it moves within; iOS holds no hover.
- **After review round 1 (Astra, Grok):**
  - The hold is kept apart from the real state and shown beside it: `active` shows the press (`showPress`, eased as a real press, at once on the agent's clock) without touching `pressed`; `hover` and `focus-visible` show the look beside the pointer's and the focus's (`looked`). Letting go leaves what the real state shows: a press really down and inside its box keeps its paint, and its release still clicks. A press already down as an active hold lands is taken over: its release or its leaving the box shows nothing and is no input.
  - Real input lets every state go: a press (the Mac's window monitor and `mouseDown`; iOS's `ExactWindow`, a node's `touchesBegan`, the agent's finger) or a key (`keyEvent`, both platforms) on the held node or inside it, through authored parents. A hover goes when the pointer's path comes onto the node or leaves it (an over bit of its own, a node with no hover look included); moving within keeps it. A focus goes when the focus comes into the node or leaves it, on every focus change (`Presenter.focus` and `blur`, both platforms).
  - Disabled: resolving `active` stops at a disabled node and answers `disabled`; a node under a disabled one refuses; a props change disabling the held node or an ancestor lets it go.
  - `held.rows` are CSS values: the press rows from the runner's `node` op, the look's rows written as the runner writes them (`#rrggbb`, `light-dark(…)`). They answer for the node named too, with its press's rows.
  - A new generation lets the hold go unsaid before the presenters reset (`HeldState.swap`, the frame kept), takes no input while swapping, and holds again after under the same plan: one `held` at the same revision, nothing `gone`; under another plan nothing is said.
  - A state no host knows is answered `unsupported` by name, unpainted.
  - The site and instance resolve through the logical tree: a flat leaf's or an inline run's site (no view of its own) presses its press ancestor; a hover or a focus needs the node's own view (else `gone`).
  - Tests: a press down after the hold lets it go at its down; one down before it is taken over (no input at its release, the paint kept, and let go with the press still down it still clicks); a key; the focus coming and going; a hover on a child; a disabled ancestor; row values; a child with no view; an unknown state; the presenter reset; and on the Mac `applyTogether` under the same plan and another.
- **After review round 2 (Astra, Grok):**
  - `active` resolves to the host's real press, a `press` handler or a button's default link (theme2's 11:48), as `platformPressDims` takes it.
  - The iOS agent lets a hold go at its press's down, whichever way the tap is delivered (a host activation included), on the node it names or that node's nearest view, and where a pan's finger lands.
  - Keys let go at keyboard input's entry, before shortcuts and composition: the Mac's `routeKey`, UIKit's key commands (`performShortcut`), the software keyboard's text (`insertText`, `setMarkedText`), and the iOS agent's key before its shortcut.
  - A held node that starts to leave (an exit) is `gone` at once, and one taken out of the map (`forget`) or under a presenter that resets (its session ending) too: one `gone` at the held revision; still unsaid in a generation swap.
  - A held press shows on a native button as its highlight, beside the platform's own (`NativeButtonIOS`, `NativeButtonCellMac`), and on a press with no press rows as the platform's dim or tint, taken over the same way.
  - A Mac hover takes `over` from a live hit test of the resting pointer, so after a same-plan swap a move within the new node keeps the hold.
  - The iOS agent's `layout` of a flat leaf answers as one (its box in its parent's view, `native.flatLeaf`), with the press's held rows.
  - Tests: a shortcut through `routeKey` and a UIKit key command; a native button and a plain press, down then held then up; a default link; an exit then a destroy, a reset, and on the build channel an exit and a session's end each `gone` once at the held revision; the iOS agent's tap on the press and on its flat child, and `layout` of that child; on the Mac a held hover through a same-plan swap, moved within. The fixture plan is `host/apple/tests/fixtures/held-state.contract`.
- **After review round 3 (Astra, Grok):**
  - Holds are their stream's, as the web's are: a hold frame is bound on arrival to the stream it came on (the connection's nonce). A new stream forgets the revisions ended on the one before (as a lower revision does), and what a hold of an old stream says later (its let-go, its refusal) is said to none and ends nothing, so a restarted server that numbers from where the old one did is never answered by an old hold. `rehold` takes only a frame of the stream it is on.
  - A hold frame for another plan than the running one leaves both the hold held and the frame kept for `rehold` as they are.
  - A Mac click's hit and a held hover's `over` are what the hover path sees (`TableRowCell.whatIsThere`): a pressable table row's own views, where AppKit's hit test gives the table.
  - iOS's text view and field let a hold go at a backspace, one that deletes nothing included.
  - The iOS agent lets a hold go where its press is delivered, never at a tap it then refuses: at a finger's checked hit (its long press or double tap too), at a host activation once it took, at a pan's, a pinch's, an iframe tap's, and at a scroll drag's accepted start.
  - Tests: two reconnects reusing revision 5 (an old hold's late release says nothing; ended revisions are the old stream's), and a frame for another plan; a real click on a pressable table row and a move within a held row; a refused tap, a tap on the press at its flat child's point (the press runs), a checkbox's host activation, and a backspace.
  - After the re-check: a canvas lets a hold go only at an accepted down or one-shot press (not its hover, wheel or a contact's later phase), and a long press or a double tap only once a handler heard it or it selected; one refused (`no … handler`) keeps the hold (tested).

### Stage 8, commit E3: held states on Linux and Android (draft for theme2)

Built to the same plan and E1's wire. Android's reader runs this presenter, so it holds what Linux holds.

- **The build channel** (`host/linux/src/build_channel.rs`, `presenter/build_channel.rs`): `hold` is parsed (`Frame::Hold`), no longer reserved. The registration says `hold` among its capabilities. The registration and `status` say `planDigest`, so the server replays a hold under a new plan.
- **Where the hold lives.** One hold an app, on the host's own tracking (`Host::looks_shown`, `press::Shown`), shared by the build channel's frames and the agent's `holdState`. A new generation's host holds none, so nothing is said.
- **Resolving.** The runner's `sites`, then `node` with `plan: true` for each view at the frame's site; the instance path compares as a JSON value (keys in any order). No single view there lets the one held before go (`replaced`) and reports `gone`.
- **The states**, handlers never told (`presenter/held.rs`):
  - `active`: the press of the innermost node at or above it with a `press` handler (07:48), via `press_feedback(key, true)`, so D7's press paint and scale ease as a real press's (landed at once on the agent's clock). While held, a real press's release or its moving off leaves the press: a press already down when the hold comes is taken over.
  - `hover` and `focus-visible`: the held node keeps that look beside the pointer's and the focus's (`Shown::on`); `refresh_looks` paints it on and off at once.
- **Letting go.** A disabled node (or one under a disabled node) refuses one, and disabling lets it go (`disabled`). The node going, or mounted again, lets it go (`gone`). Real input lets it go (`input`): a press on it or inside it (`pointer_pressed_with`, so the agent's taps too), a key with the focus in it, the pointer coming over it or leaving it for a hover (not moving within it), the focus coming or going for a focus. A held press let go while a real contact still presses the node stays until that contact's release. A newer hold, a null one or the agent's `holdState` lets it go (`replaced`); the same frame taken again (resent, or `rehold`'s) replaces nothing. Every report names its hold frame's revision; a state the host doesn't know is `unsupported` by name.
- **A new generation** lets the hold go unsaid, and `rehold` takes the last hold frame again under the same plan digest. Under another, the server's replay does.
- **The agent:** `{"op":"tap","id":V,"holdState":…}` answers `{tapped, holdState, delivery: "recognized"}`, or an error for a disabled node or no press there, or `malformed` for another state. `layout`'s node answer carries `held {state, rows}`: the node's press rows for `active` (`press_scale`, `press_opacity`, `press_opacity_to`, `press_background_color`, `press_color`, the ones it has), the look's rows otherwise, by wire name. It gives no tokens, and the server's join adds the sources.
- **Tests** (`presenter/held_tests.rs`): `active` as the press paint, eased, its rows, and let go by a real press that then goes on; a real press taken over; `hover` and `focus-visible` as their looks, their rows, and the pointer and the focus crossing; a disabled node refusing and disabling letting go; no press there, no single view, an unknown state, a reordered instance, and going; a new generation unsaid and held again under the same plan only; the agent's `holdState` and `layout`'s held rows. `build_channel.rs`: the frame parse and the capability.
- **After review round 1 (Astra, Grok):** a held `active` named on a node inside the press answers `layout` for that node too, with the press's rows; a held press let go keeps its paint only while a live contact presses the node inside the box it had when pressed (one dragged off unpaints). A frame addressing another plan is nothing, unsaid; a revision ended here is not taken again from a resent frame, and a lower revision (a new server's) forgets them.
- **After review round 2 (Astra, Grok):** a disabled node stops a held `active`'s walk to its press (`disabled`), as a real press does; an `unsupported` hold ends its revision; a held `active` shows the painted control's pressed look (a neutral control's wash) as a contact's does.
- **After review round 3 (Astra, Grok):** a held `active` named inside its press climbs the kernel's parents (the display's are none until a new generation's first picture is acknowledged), so a same-plan generation takes it again with no `gone`; a frame of another plan leaves the frame kept and what is held alone; holds are scoped to the build channel's stream (a let-go of an earlier stream's says nothing, a queued hold report of an earlier nonce, or of none while the stream is lost, is dropped, a kept frame is taken again only on its own stream, a hold command still waiting from the old stream is dropped, and a new or lost stream forgets the ended revisions); Android's reader tells exact2 of a finger down, a key down or any IME call (a code-point deletion included) in a hosted field (`Native.fieldInput`), so real input that changes nothing else (a backspace in an empty field) still lets a hold go; Android's hosted fields (a finger on one, an edit, composing, a selection, the action key) let a hold of the field go; a held press and a contact's each show their own painted control pressed.

### Stage 8, commit G: the clone takes state parts (clone `5ca7be8`, `382201d`, `95d7d41`, on its main)

- **The Post button** is its own control (D8): `style PostButton`, a solid pill on `param.fill`
  (primary_500), `:active` primary_600 (lowering to D7's press paint) and `:disabled`
  primary_300. It was iOS prominent glass with an `opacity=(dim ? 0.5 : 1)` hand fix, and iOS
  draws a disabled prominent glass button grey (D9); the hand fix and the `dim` prop are gone.
- **The action row's six buttons** take `style ControlButton`: `:hover` and `:focus-visible`
  show contrast_25, and `:disabled` is opacity 0.6, replacing the reply column's opacity hand fix.
  The switches already took `accent-color="primary_500"`.
- **Spinners** (`382201d`): after the spinner rule (C's gap, `4418182a9`) the clone's root
  `accent-color="text"` reached its 14 spinners; its 7 bare ones take their own colours
  (contrast_500, white_100 inside filled buttons), as Bluesky's grey `ActivityIndicator`s are.
- **Format** (`95d7d41`): one `contract fmt` over its 22 `.contract` files, format only; the plan
  is byte-identical (cmp), so a `set-state` write is a one-line diff.
- The clone's web tests pass, 9/9.

### Stage 8's gate: met, with two deviations (2026-10-18; the run is `~/x2theme/stage8-gate-run.txt`)

The clone at `95d7d41` (G `5ca7be8`) on exact2 main: the host lines at `91ad9421f` (the Android
reader at `55a2c10fc`), the hold lines at `b301a914a`. Stage 8's commits: A `e53b427a3`..`d17651ba0`,
F `7be3690cf`..`78b93c82a`, D `5ddec9cc4`..`dd5a46579`, B1 `4b1f450fe`..`6da7f6fc2`, B2 and B3
`b5f14324e`..`8e9ac2349`, C `c57e36a3d`..`84b24fafb`, E1 `9d8d22d29`..`740af62e9`, E2 and E3
`e27145459`..`a0ff804f2`.

| Gate line | Result |
|---|---|
| The clone's Post button and switches are right on every host with no hand fix and no double dim | **Met** on the web, macOS, the iOS simulator, Linux and the Android reader: rest #006aff (primary_500), pressed #0059d6 (the `:active` part), disabled #75afff (primary_300, nothing over it), switch #006aff. The Mac's `NSSwitch` draws the system accent, which `tamber-unreachable` names. |
| A checked, disabled toggle with both parts shows `:disabled`'s | **Met** on the same hosts (#ff0000), and `state_parts.rs`. |
| A class change and a base change while pressed keep the overlay over the new base | **Met**: the web's `looks.test.mjs` in Chrome, Linux's look tests, Apple's `StateLooksTests`. |
| Existing `disabledStyled` apps unchanged; LLP 1061's press corpus unchanged | **Met**: caltrain, theme-fixture, shelf, realworld and tamber-host, and the press corpus (`press.contract`, `press-paint.contract`, `motion-feel.contract`), built before and after each run of stage 8 commits: only the format and schema digests, and A's empty looks table (4 bytes). Conform `--synthetic --strict`: 0 failures in 97 targets. |
| `tamber-unreachable` names the macOS switch and iOS's disabled glass button | **Met**: the clone's settings and moderation switches (after `91ad9421f` placed it past the warning cap), and `control_roles.rs` for iOS's disabled prominent glass (the clone has none now). |
| The workbench shows each component's state parts, editable through `set-state` (which rebuilds) | **Deviation**: the server half is met (on a copy of the clone, `set-state PostButton :active background-color` proposes `rebuild`, writes the row as a one-line diff, and undo gives the bytes back). The workbench is the design tool's, LLP 1122 T3 and T4, and moves to T4's gate. |
| Hold (LLP 1122 T4): a held state shows without a touch; real input clears it; a disabled node refuses | **Met** on the web and the Mac. |
| Hold: inspect on a held node shows the state part's rows | **Deviation**: the rows and computed values show, but by the lowered press row (`press_background_color`) with provenance `incomplete`, not as the author wrote them (`background-color` under `:active`, its span, its tokens). Moved to stage 9 E. |
| No regressions; release output unchanged where no state parts are used | **Met** (above). |

**Also in stage 8:** C's Exact-side gaps, each its own reviewed commit, re-measured, flipping its
cells: the reader's placeholder (`682def423`), one spinner rule on every host (`4418182a9`), the
painters' closed select's why (`d4e62c645`), FieldText (`e5ebb1163`, which also ends Apple's
inherited colour on a field, LLP 1115 D4) and Field (`6ec337216`; the Mac's input stays `none`,
its bezel drawing its own ground). The protocol's tamber/1.11 window closes with the stage; the
next wire change takes tamber/1.12.

**Stage 8 is closed.**

### Stage 9, commit D1: the map says where a row is written (draft for theme2)

`set-row`'s address (LLP 1122 D6) is the attribute's place and its file's digest, never a plan
digest. The map carried each source's `sha256` and a state part's row's place; now every row an
author wrote says where.
- **The compiler** (`contract-lower`'s `Origin`): `Own` and `Class` carry the attribute the row
  came from (`Written {span, name}`), its name as written, so an expansion names its shorthand. A
  row the compiler derived, a tag's row, an implied one, a conditional class's (`class=(c ? A :
  B)`) and a folded `:disabled` or `:checked` row carry none. A written row's origin is its place's
  (a class's row is one of its style's attributes), so a rewrite that moves rows (a class's
  animation composed, a grouped list's rows inserted) moves none across its origin. The map gives
  the address only where the source as compiled (kept with its digest) says that name at that
  place, so a row the compiler composed or renamed (an animation's longhands, a symbol's tint, a
  grouped field's `flex` made `flex-grow`) says nowhere (fail closed, recorded).
- **The map** (`map.rs`): such a binding gives `at` (the attribute's place, in the style's own file
  for a class's row) and `attr`, its name as written. The map is not the plan: no format digest
  moves.
- **The selection join** (`scripts/tamber-inspect.mjs`): each such row carries `at` with that
  file's `sha256` as compiled, and `attr`. A state part's row keeps its `state` and `at` as before,
  and only a state part's place counts as a node's in `sitesAt`. *tamber/1.12* opens here.
- **Tests:** `source_map.rs` (an own row, an expansion, a class's row in another file, a tag's row
  saying nowhere); `tamber-inspect.test.mjs` (the join's `at`, `sha256` and `attr`; an attribute's
  place is no node's site).

### Stage 9, commit E: held rows name their tokens (draft for theme2)

Built to the approved plan (`~/x2theme/x2p4-s9e-plan.md`; theme2's 16:13 ruling and its conditions), under `tamber/1.12`.

- **The runner** (`runner/src/agent.rs` `node`): the answer gains `looks {hover?, focus_visible?}`, each look row as a style row is, its value and the token it read (`Runner::look_token`, from the theme index's look rows under the node's tuple). So every host reads a look's rows, and their tokens, from one place.
- **The held answer, every host**: `held {state, node, site, instance, rows: [{name, value, token?}]}`, the node being the one the state is held on (the press it resolved to, named on a node inside it). Rows: `active` the node's own D7 rows from the runner's answer; a look's from its `looks`.
  - The wasm page: `heldOf(el, ask)` reads the runner (no more CSS text, `computed` or `provenance`).
  - The JS target (no runner): the page's CSS, a varying token named by modes.js; `provenance: "incomplete"` only for a value it can't name (16:13 (b)).
  - Apple and Linux: their runner's `node` answer (Apple's batch conversion stays only for a presenter with no session).
- **The server's join** places held rows at the held node's site (its map node and bindings), and names an `:active` row as its author wrote it under the part (`background-color`), the press row it lowered to as `row` (16:13 (2)).
- **Docs:** the protocol's `tamber/1.12` bullet and the selection's `held`. QUEUE's two entries (held token names and the press's site; Apple's non-colour look rows) are closed.
- **Tests:** `state_looks.rs` (a node's layout says its looks' rows and tokens: a token row, a literal row, only the states it has); `hold.test.mjs` (the runner's rows and tokens for a held press named on its label, and for a held hover); `HeldStateTests` (a held hover's rows name their tokens on the Mac; a press named inside names the press's node and site); `held_tests.rs` (the held answer's node and site); `tamber-inspect.test.mjs` (a held press named inside joins at the press's site, named as written, tokens kept).

- **After review round 1 (Astra, Grok):**
  - A look's clear (`unset`) is said in the runner's `looks` (`{"value": "unset"}`), not dropped.
  - The JS target: a link's empty `data-view` marker names no node 0 (the runtime's id is the agent's and the selection's); the held node's instance path comes from the inspect table where the page has one (`exact.instanceOf`); `provenance: "incomplete"` only for a token reference (`var(--x-…)`) modes.js can't name, never for a literal pair or a host's own property.
  - The join checks the held node's own files (its file, chain and parts) for freshness, as the selected node's.
  - Tests: the clear said; a JS link's held node; the held node's moved file read-only; Linux's held answer names the card from the child's layout too.
- **After review round 2 (Astra, Grok):**
  - (At landing: `tamber/1.12` was opened on main by D1 and rt1's held revision; E's items joined that one bullet, theme2's 18:44.)
  - The JS target names a role's override too (`var(--exact-canvas-text)` is `CanvasText`, modes.js's table), a host's own property still none.
  - A site the inspect table lacks gives the held answer no instance path (not the root's `[]`).
  - Tests: the runtime's id names a JS link; a role override named; `cargo fmt` over the new tests.
- **After review round 3 (Astra RIGHT WITH CHANGES, Grok RIGHT):** the JS target checks each token reference on its own, so a role it names beside a token it can't name still leaves the answer `provenance: "incomplete"` (a test with both).

### Stage 9, commit D2: `set-row` (draft for theme2)

LLP 1122 D6's checked edit of one Contract row (`contract/cli/src/tamber_rows.rs`), on stage 8 D's
`.contract` ownership.
- **At a place:** `set-row {file, at {line, col}, sha256, row, state?, value}`, the
  selection's row's `at`, `attr` and digest from D1's join. The file must be the one the tool saw
  (its digest), the attribute there the one named (`row` is the attribute's name as written; with
  `state`, a node's own state part's row, which `set-state` refuses), and a paint row written as one
  value (a string, a token's or param's name as one, `"param.fill"`, or a number). Only the value is spliced; several places in one
  file go together, from the last up, before the transaction's other writes to it.
- **On a style:** `set-row {style, row, state?, value, file?}` edits a row the style writes (the
  last that writes it, as lowering keeps), in its
  own file (`file` named where two files declare it); with `state` it is `set-state`.
- **Refused, each saying why and where instead:** a stale digest; another attribute at the place
  (select again) or none (a tag's or derived row: write the row on the node); a longhand of a
  shorthand written there (the shorthand and its current text named, so the tool composes the new
  value); a layout row (D14) or a prop; a value written as an expression; a style row the style
  doesn't write. Then `contract fmt`, the whole app compiled in memory (a failing candidate writes
  nothing), the journal; the class is `rebuild`, and the inverse restores the preimage byte for
  byte.
- **Tests** (`tamber_serve_rows.rs`): one line in a formatted file and undo; a node's own `:active`
  row; an unformatted file restored exactly; a stale digest and a failing candidate writing
  nothing; each refusal; a class's row in its style's own file with two files declaring it.
- **History:** a style's own row joins `history` and `revert` by (style, row), its value put back
  with `set-row`; a row the commit added or removed is refused. A node's own attribute stays code:
  it has no key across versions (QUEUE; theme2 17:18).

### Stage 9, commit D3: `add-param` and `remove-param`; promote and detach (draft for theme2)

Stage 7 C's operations (approved, then deferred until `set-row`), `contract/cli/src/tamber_param_lines.rs`.
- **`add-param {component, name, kind, default, attrs?, file?}`** writes the component's param
  line (its attributes in D15's order), making the `params` section before the component's `view`
  where it has none. **`remove-param {component, name, file?}`** takes the line out, and the
  section with its last. The file is the one declaring the component unless named.
- D15 refuses a param no row reads, so each compiles only beside the `set-row` that starts or stops
  reading it, in one transaction: alone it is refused by the compile, the refusal carrying a
  `hint` that says so.
- **Promote** (LLP 1122 T5's gate): to param, `add-param` with `set-row` naming `param.<name>`; to
  token, `add-token` (or `set-token`) with `set-row` naming the token: the design and the
  `.contract` file in one transaction with one inverse, which the commit guard refuses after a
  hand edit to either file, writing neither. **Detach** sends a param's `remove-param` with the
  `set-row` that gives its last reader its own value.
- **Tests** (`tamber_serve_rows.rs`): promote to param then detach, each undone in one step, byte
  for byte; add or remove alone refused with the hint; promote to token across two files, its
  undo refused after a hand edit to the design and accepted once put back; the section in the
  named component before its own `view` (a comment on the header allowed), the last param taking a
  commented header and its comments; the hint only for an unread param.
- **History:** a `params` line is code, as the template row that reads it is (QUEUE): its revert
  needs that row.

### Stage 9, commit A1: Apple draws icon paths (draft for theme2)

Built to the approved plan (`~/x2theme/x2p4-s9a-plan.md`; theme2's 17:03 ruling and its conditions). No plan or schema change: the path is the plan's already, and `symbolPath` is Apple's own native prop.

- **Rust** (`host/apple/src/icon_path.rs`): an icon with no `sf` and a template path sends `symbolPath`, the kernel's parse (`svg::parse_d`, quadratics and arcs already cubics) as absolute `M`/`L`/`C`/`Z` commands, the SVG's paths together, and `symbolViewBox` (`paragraph.rs` `props_for`, priced in `content_region.rs`). An `sf` wins: nothing is sent.
- **Swift** (`IconPath.swift`): the commands as a `CGPath`, fitted into an em square in its aspect (xMidYMid meet, as Linux's `fitted`), filled nonzero, as a template image. Page images (`NodeViewIOS.updateSymbol`, `NodeSymbolMac`) show it where no SF Symbol is named, with no "unknown symbol role" log.
  - Its tint (D10): `-exact-tint-color` where written (the accent where it names one), else the node's `color`. Path icons only: SF Symbols keep their tint as before (17:03).
  - Its measure is an em square, as the kernel already measures a name Apple has no glyph for.
- **The lint** (`icons.rs`): a template SVG is a source for Apple targets; a non-template one needs an `sf` there, the message saying so.
- **Docs**: D10's per-target note and stage 6 F's §10 note (the symbols recipe, at its byte cap, already says an icon token draws everywhere). QUEUE: "Apple hosts draw icon paths" closed; a non-template SVG's rasterising into the asset catalog added.
- **Tests**: `icon_path.rs` (an svg-only icon's commands, a quadratic and an arc as cubics, two paths; none with an `sf`, for a non-template file, for a role); `tamber_icons.rs` (an svg-only icon builds for ios; a multicolour file is refused naming the SF Symbol); `IconPathTests` (Mac and iOS: the path drawn as an em-square template image, tinted by its tint, else its colour, tinted again by a new tint; an `sf` icon still its symbol; the path's fit).
- **After review round 1 (Astra, Grok):**
  - Each of an icon's SVG paths is filled nonzero on its own (`;` between them in `symbolPath`), as Linux and the web fill them: two paths of opposite winding that overlap stay solid.
  - Coordinates are the shortest text that reads back as the `f32` (a tiny viewBox keeps its path).
  - iOS draws the path again at the box it fills (UIKit scales a bitmap); the Mac's image is vector already. The widget snapshot draws the path too. The core (`IconPath`) moved to ExactCore for both.
  - A path icon with no tint row takes its text colour, else the platform's text colour (live), never the SF Symbol's unsaid grey.
  - The lint keeps requiring an `sf` for a watch (no watch host draws the path yet).
  - Tests: an asymmetric bar's pixels (right side up), two opposite-winding paths solid, an accent tint; docs (D10, the known-limits line, stage 6 F3's Apple bullet).
- **After review round 2 (Astra, Grok):** the image helper is left out of watchOS (it has no image renderer); a centred path icon on iOS shows its em square again whatever the box was before; the widget snapshot places the em square by `object-fit` and meets the viewBox in it, as the page hosts do, and its tint shows through one mask (overlapping paths don't compound a translucent tint); a watch target names its missing `sf` (`tamber-icon-missing`) instead of calling a template no one-colour shape.


### Stage 9, commit G1: a look snaps on the web (draft for theme2)

D8 says a look snaps; on both web targets a look is a page rule (`looks::rules`), so a node's own
`transition` on a property its look sets timed the look both ways.
- **The input glue** (`host/web/input-glue.js`, both targets): the browser has already moved
  `:hover` and `:focus-visible` when `pointerover`/`pointerout` (and `mouseover`/`mouseout`, which
  Firefox sends alone for a scroll under a still pointer; one after its pointer event is the same
  edge) and `focusin`/`focusout` come. An enabled node with a look for the state whose edge it is
  (the pointer entering or leaving it, not moving within it, where the pointer can hover; a focus
  look's node's own `:focus-visible` turning on or off, inside a shadow root too, not a pointer's
  focus) takes that one style change with its transitions off (`data-exact-snap`, set before any
  style is read, which the press paint's observer doesn't watch; one forced style for every such
  node). Focusing reads style before its focus events come, so a focus look's transition can start
  there and is ended at once by the same still change: what shows is the look, with no frame of the
  transition. A key that turns `:focus-visible` on for
  the node already focused (a click, then a key) is held still too, at the window's capture before a
  shortcut can take the key: Blink and WebKit move it before the key comes, others in its default
  action. A base change eases as authored. The JS target loads
  the input piece for a node with a look, as for a key shortcut (after first paint).
- **Deviation (theme2's 19:55 (b)):** a transition of that node's running across the edge, on any
  property, ends there (CSS can't hold one property still). QUEUE only if seen in practice.
- **Not heard:** hover that moves with no event (a node laid out or inserted under a still pointer;
  WebKit's hover timer), a touch screen's hover that sticks after its up, and an edge before the
  JS target's input piece has loaded; the look then eases as before.
- **Tests:** `host/web/tests/looks.test.mjs`: a wasm view and a JS-target node with transitions on
  what their looks set show the look and the base right after the edge with no transition started;
  moving onto a child touches nothing; a delay alone snaps; a focus look comes and goes at once, a
  click then a key included, and a pointer's focus or a hover over a focus-only look touches
  nothing; a base change still starts its transition.

### Stage 9, commit A2: Apple's control faces draw icon paths (draft for theme2)

Built to the approved plan's point 5 (`~/x2theme/x2p4-s9a-plan.md`; theme2's 17:03 and 17:32 rulings). No plan, schema or wire change: the press face's and a grouped row's JSON gain `symbolPath`/`symbolViewBox` beside a "" `symbol`, as page images carry them since A1.

- **Where a face's symbol travels**: a button face (`button.rs` `face_json`, the kernel's `PressFace.symbol` already the full source) and a grouped row (the kernel's `GroupedRow.icon`, the token form of an svg-only icon, which now makes a standard cell where it made a custom row; `abi/commands.rs`) write the path through one helper (`icon_path::push_json`). Bar, toolbar, tab, segment and title-line faces read the image node, which carries `symbolPath` since A1. An `sf` wins: no path is sent.
- **Swift** (`FaceSymbol.swift`): an svg-only icon's path travels as a token where a face carries a symbol's name (its equality, caches and drawn source unchanged) and draws as ExactCore's `IconPath` template image, an em square at the point size the site gives an SF Symbol (the configuration's, else the body's; a tab bar's 25, a Mac toolbar's 17). On a face the control tints it as it tints a symbol (D9; LLP 1115 D4); a symbol tint its author wrote on a Mac button face draws it in that colour, as a palette colour draws a symbol. A1's page-image tint-else-colour rule is not applied to faces. Sites: iOS bar items, badge items, readouts and bar faces, the navigation title's glyph line (seated on the cap height), root tabs, segments and their tab bar, button configurations, menu rows, swipe actions, grouped rows; the Mac's toolbar, segments, sidebar rows, button configurations, chooser and shortcut menu items. Snapshot leaves were A1's. A multicolour SVG still needs an `sf` on Apple.
- **Tests**: `icon_path.rs` and `button.rs` (the JSON carries the path for an svg-only icon, none for an `sf` icon or a role); `tamber_icons.rs` (a grouped row led by an svg-only icon is a standard cell with its icon, an `sf` icon's row its name and no icon); `FaceSymbolTests` (Mac and iOS: a bar or toolbar item, a tab, a segment, a button face, and on iOS a grouped row show a template image for an svg-only icon; an `sf` icon unchanged).
- **After review round 1 (Astra, Grok):**
  - An unsaid path on an iOS button takes the title's foreground as an untinted symbol does (UIKit gives a configuration's foreground to symbol images only): the colour UIKit proposes, white on a filled style, greyed when disabled or dimmed; on glass it is drawn in the glass's ink (white on a prominent glass, else the label, grey when disabled) through the button's ink, never the accent. Pixels match an SF Symbol's in the same button, light and dark, enabled and disabled.
  - A symbol alone on an iOS button: the path takes the size UIKit resolves for a symbol in that configuration (its `buttonSize` too), not the body's.
  - The Mac's symbol tint on a path keeps the tint's alpha (`sourceIn`): a translucent tint stays translucent, a transparent one draws nothing.
  - A grouped standard cell gives the path the standard symbol space (`reservedLayoutSize`), so its title aligns with an SF Symbol row's; untinted, the path is the label colour as a symbol is (a destructive row's red, a disabled row's grey as before); its size is the cell's Dynamic Type size, drawn again when that changes.
  - The agent names a path by the icon token its author wrote (`icon.reply`) in a native button's `symbol` (iOS and Mac) and a sidebar row's: the press face's JSON carries it as `symbolIcon`, which the Apple host reads from the plan's icons table by the icon's source form (no plan or format change); "" where no row names it. Caches keep the token.
  - Theme2's ruling: where a site gives a symbol no configuration, the path's size is measured, not guessed. An SF Symbol ("square") and the path were drawn side by side at each site and their inked boxes compared, the symbol's point size read against the same symbol drawn at 100 points. iOS 27 simulator (in a scene's window): a tab bar's symbol is 23.3 points at a compact width and 17.2 at a regular width (inline items), so the tab bar's path is 23 or 17 by the size class; a navigation bar item's 21.7 (22); a segment's 16.7 (the body's 17); a symbol-only button's what UIKit resolves (22 regular and large, 19 small). macOS 27: the toolbar's 16.7 (17), measured from a screen capture since AppKit draws a toolbar button through SwiftUI, which no in-process capture reaches; a segment's 13.3 (the system font's 13). The tests keep each measure, the toolbar's by hand (`EXACT_MEASURE_FACE_SYMBOLS=1`).
  - Tests mount the real faces: the native fixture's bar item and root tab with a path written on their image, native tabs' tab bar and segments, the Mac's toolbar and segments, and the Mac's sidebar over a compiled app with an svg-only icon token (the plan's path and the token's name through the Rust side); a button's pixels beside an SF Symbol's; the Mac's opaque, translucent and transparent tints; the grouped cell's alignment, tint and Dynamic Type; the agent's output.
- **After review round 2 (Astra, Grok):**
  - A grouped path whose tint follows the accent (`AccentColor`) keeps the cell's tint, as a symbol's does, not the label colour: an absent tint and one that follows the accent are told apart. Tested with an accent that changes (FaceSymbolGroupedTests; GroupedListIOSTests is at its cap).
  - Tab bars draw a path again when the size class changes: the in-page bar's items are keyed on the glyph's size and the bar watches the size class; the root tab bar watches it too and reads its own traits once in its window. Tested compact, regular, compact with the faces unchanged, in both.
  - A symbol segment's path is drawn at the control's body size and again as the text size changes (keyed on that size; the control watches the content size).
  - The agent's icon name: the source form carries none (the compiler and the runner's bridge expand the name away), so two tokens on one SVG (`icon.reply`, `icon.respond`) give one form and one plan string, and the host reports the first declared, the best it knows. theme2 ruled (21:49) to keep that and QUEUE it, with a build lint for two icon tokens with identical sources as the cheaper real fix.

### Stage 9, commit C1: `explain` answers an icon and a param (draft for theme2)

QUEUE (stage 7 B, round 2): an icon and a param were read through `describe` and `resolve` only.
Under `tamber/1.12`, optional fields (theme2's 19:25 ruling (a): one computation, two readers).
- **`describe`'s icon entry** gains `draws`, `{target: source}` for each target the app ships: the
  source that target draws, by D10's rule (`contract_lower::icons::draws`, the same function
  `tamber-icon-missing` now checks with), or null. It reads stage 9 A's rule: Apple draws an
  icon's SF Symbol, else a template SVG's path (not yet on a watch).
- **`explain` of an icon** answers `kind: "icon"`, its value as its design writes it, its chain
  (the icon an alias, `icon.alias = icon.home`, names; empty for an `icon(…)`), and its `describe`
  entry. The server folds a design as the build does with or without params, so an alias reads in
  an app that has none. An icon param (it was
  refused, "an icon") answers the same by its full name, its chain the icon or param its default
  names, with its `param`.
- **`explain` of a param** adds `param`: its component and identity, kind, the place of its line,
  its default as written, and `set`, the design's line that sets the app's value, or null.
- **Tests** (`tamber_serve_explain.rs`): an sf-only, an svg-only and a three-source icon's `draws`
  in `explain` and `describe`; a param before and after the app sets its value.

### Stage 9, commit C2: `uses` of an icon; a motion token's sites by instance (draft for theme2)

The rest of the QUEUE entry C1 began, under `tamber/1.12` (theme2 19:25 (b)). Both join the app's
elements (every source it loads, walked from each view) to the source map's nodes by span, so a
site is located as the map has it (`sitesAt` outlines by it) and a component used twice is two.
- **An icon** (`contract::tamber_uses::icon_sites`, `tamber_explain::icon_uses`): each plan node
  of an `image` whose source names it (its string, either arm of a condition, or `src`), directly
  or through an icon param of its component, by identity, whose value (the design's, else its
  default as decoded) resolves to it, to a fixed point (`via: {token: <param>}`); `row` is `symbol`.
  A text, another attribute or a comment that spells `symbol:…` is no site. Asked about an icon
  param, `uses` answers by its full name with the images that read it.
- **A motion token by instance** (`motion_instances`): each plan node declared at the element
  whose attribute holds a literal is a site, the literal kept as `literal` (an animation's
  longhands, which the compiler composes, included). A literal in no element of the views (an
  unused component, a component argument's) keeps `site: null`. `site` was always null before
  1.12; the answer's `plan` is the digest.
- The QUEUE entry closes but for the motion rebuild class (a motion value's change stays
  `rebuild`), kept as its own entry.
- **Tests** (`tamber_serve_explain.rs`): an icon drawn directly and through a param in a component
  used twice; a text, an attribute and a comment spelling it are no sites, a parenthesized source
  over two lines is; an icon param asked about; a motion literal in a component used twice (two
  sites, the literal kept), an animation longhand's instance at its node, one in an unused
  component (`site: null`); a design the app doesn't build has no sites.

### Stage 9, commit B: symbol images take their written colour (draft for theme2)

theme2's ruling (2026-10-18 17:32) on every host but Linux and the reader, which keep their own
rule (the tint, else the text colour). An untinted symbol **image** draws in:

1. its `-exact-tint-color`, where written;
2. else a `color` its author wrote on it, or on the nearest ancestor that sets one. A token or a role counts. The inherited default label colour and a sheet's colour don't.
3. else the platform's unsaid symbol colour: the accent on the web and iOS, `secondaryLabelColor` on the Mac.

This is D10's "colour from `-exact-tint-color` and `color`". It applies to page images only: a symbol
on a native control's face (a bar item, a tab, a segment, a button) keeps the control's tint (D9,
LLP 1115 D4).

- **The kernel** (`NodeRef::symbol_color_said`, kernel/src/node.rs). An image with a `symbol:`
  source, not a native button's face (a `Control`'s child), for which `says(TextColor)` is true.
  `says` walks up to the first node that sets `color`: true where its author wrote it, false where only
  a sheet did. A platform start (the neutral sheet's route header) stops the walk. A change of an
  ancestor's colour, or of whether it was written, already touches every descendant that inherits it
  (`propagate_inherited`), so each host re-sends the image. `NodeFacts` carries the answer
  (`symbol_color`, false for any other node), so `props_of` writes it for live batches and server
  documents alike. `DocTree::facts` computes it the same way for a document written without a kernel,
  and also stops at the neutral route header.
- **The web.** The image is marked `data-symbol-color` (element.rs). The glue's symbol sheet adds
  `:where(img[data-symbol-path][data-symbol-color]){--exact-tint:currentcolor}`. That rule has zero
  specificity, so an authored tint still wins. In Chrome, the registered `<color>` `--exact-tint`
  resolves `currentcolor` on the image to the image's own `color`: a `light-dark()` colour by its
  scheme, and a later colour at once.
- **The JS target.** The same mark and the same sheet (symbols.js). `style::symbol_marks` marks what
  the static tree can't decide alone:
  - any bound source, whatever names its role (a state's, a data's): the sheet asks for a symbol's path, so a photo's mark draws nothing;
  - under a bound `color` or a mode boundary, the image carries `data-symbol-ink` (how many such nodes are above it, and the decision past them), and each such node `data-exact-ink` (a bound colour written or not, `b` a boundary). After each commit symbols.js (`inked`) takes the nearest that sets the row: a boundary's implied row (modes.js `$xs`), no author's, which stands where the boundary's own binding clears or is refused, as the runner installs it; else a bound colour while its binding has a value, a clear included (`$ink`, which rows.rs records from the binding itself, not from what a boundary leaves inline). A `none` takes a bound colour away, as the runner clears the written bit, and the next one up decides.

  Not marked: a colour that only a state look sets (`:hover`).
- **Apple.** `props_for` sends `symbolColorWritten: "1"`, priced in `content_region.rs` as
  `symbolName` is. `NodeView.symbolInk` (Affordances.swift) is the node's `text_color` where nobody
  tinted it and the prop is set:
  - on iOS, `tintColor = symbolTint ?? symbolInk`. A colour that names the accent is `nil`, so UIKit's live tint is inherited as before;
  - on the Mac, `contentTintColor = symbolTint ?? symbolInk ?? unsaidSymbolTint`. The accent is `inheritedTint`;
  - a hierarchical configuration takes the same colour, and `symbolLookKey` holds it when the glyph bakes it.

  Only the page image's `updateSymbol` reads it; bar items, tab faces, segments and button faces
  don't. Where iOS or the Mac shows a tab's or a radio's one image as a segment, the segment's
  image is made at that point in the control's tint (`NodeView.faceImage`), never in the page's ink. A props change of the word re-runs `updateSymbol`. An unwritten symbol is unchanged, and
  `PlatformDefaultsMacTests` is unchanged.
- **Movers** (condition 1). Each app's static tree (the JS target's, every site with its arms
  flattened, a component per use) was walked for symbol images with no tint and a written colour:
  - **Shelf: none.** Its stars write no colour ("in the tint"), so they stay the accent on the web and iOS and `secondaryLabelColor` on the Mac. Its tab and header symbols are control faces.
  - **The clone (`95d7d41`): 8 sites** of 1,968 symbol and photo images (683 symbols):
    - the chat message menu's Translate, Copy, Delete and Report (chat.contract:1272, 1287, 1302, 1318);
    - the lightbox's Share (viewer.contract:453, used 4 times).

    Their colour and their accent both come from the root's `color="text"` and `accent-color="text"`. So on the web and iOS they look the same as before (the accent, now the colour: both `text`). On the Mac they move from `secondaryLabelColor` to `text`, the colour of the menu's labels. Every other clone symbol writes its own tint or is a control face.
- **Tests:**
  - the kernel's `symbol_color.rs`:
    - no colour;
    - an ancestor's written colour, its token form, and its clear touching the image;
    - a nearer colour no author wrote;
    - a nearer tint not deciding it;
    - its own colour;
    - not on a native button's face;
  - the web's `tint.rs`: batch, server document and kernel-free document agree;
  - Chrome's `a symbol draws in its written colour` (`document.test.mjs`, both sheets): the written colour, `light-dark()`, a live change, an authored tint winning, an unmarked symbol staying the accent;
  - the JS target's `a_symbol_under_a_written_colour_is_marked`;
  - Apple's `inherited.rs` (the prop, the colour beside it);
  - `SymbolColorMacTests`: Shelf's star under `color="AccentColor"` is the accent, a token colour, unwritten unchanged, a written tint wins, the word and the colour followed live, a hierarchical glyph remade;
  - `SymbolColorIOSTests`: the same on iOS.
- **After review round 1 (Astra, Grok):**
  - The JS target marks a symbol under a written colour whatever names its source, and decides a bound `color` after each commit (`data-symbol-ink`, `data-exact-ink`, symbols.js `inked`): a conditional colour that goes gives the symbol back to what is past it, and one that comes back takes it again (Astra 1, 2; `emit_tests.rs`, `js-symbols.test.mjs` in Chrome).
  - A document written without a kernel keeps an authored CSS-wide clear's written bit (`color="inherit"`), `none` unwritten, as a kernel's `ClearStyle` does (Astra 3).
  - Its walk stops where `says` does: at the neutral route header, a neutral alert and its children but its buttons; a field holds no image (Grok 1; `tint.rs` adds the clear and an alert, and fails without either).
  - A tab's whole face takes no page ink on Apple, so a projected segment's image keeps the control's tint (Astra 4; `inherited.rs`). Replaced in round 2.
  - A move that changes whether the colour is said, or takes an image off a native button's face, with the colour equal, sends the moved subtree's symbol images again (`DefaultContext.ink_said`; Astra 5, Grok 4; `symbol_color.rs`, both ways).
  - A standard grouped row on iOS tints its symbol with a written colour where nobody tinted it, destructive and disabled still over it, and reconfigures when it changes (Grok 2; `SymbolColorGroupedIOSTests`).
  - The Mac's emphasized table row keeps a fixed written colour, as it keeps a fixed tint (Grok 3; `SymbolColorMacTests`).
- **After review round 2 (Astra, Grok):**
  - The kernel's tab-face guess is gone (`is_tab_face`, its props gate and what it would have needed invalidated): it missed tabs kept as authored (`appearance="none"`, one tab, a face a segment can't show), radio segments and sibling changes. A segment's image is made where iOS and the Mac project it, in the control's tint (`faceImage`); the page image keeps the written colour wherever it is shown as one, an `aria-hidden` tab's glyph included (Astra 1, 4; Grok 1, 2, 4; `SymbolColorIOSTests`, `SymbolColorMacTests`: two image tabs and two image radios as segments with no written ink, a one-tab tablist and a page symbol beside them in it).
  - The JS target reads a bound colour's provenance from its binding (`$ink`), not from the inline colour, and a mode boundary as an implied setter, as the runner's (`theme.rs`): a symbol under a written token and a boundary is the accent, as on the kernel (Astra 2, 3; Grok 3; `modes_tests.rs` against the kernel, and `js-symbols.test.mjs` in Chrome with `Sx` on `data-x-b`: a clear said, `none`, a value).
  - The Mac's symbol cell is drawn `.normal` whenever a fixed tint or written colour keeps its ink, set again with each look, so a fixed colour that comes while the row is selected keeps its ink at once (Astra 5).
  - A document written without a kernel keeps a refused bound colour's written bit, as the runner's `ClearStyle` does (Astra 6; `tint.rs`, which fails without it).
  - After the re-check: on a boundary under a token colour, an implied row the boundary writes because its own bound colour clears or is refused stands over that binding, as the kernel's unwritten implied row does (`inked` reads `$xs` first; `modes_tests.rs` against the kernel, a clear and then a written value; `js-symbols.test.mjs`, a clear and a refused value).
  - After re-check 2: a boundary's bound colour that clears or is refused again, with nothing implied changed (`inherit` then `unset`), had its implied row cleared and not set back, so the kernel said written where the JS target did not; the runner now sets it again, unwritten, after each such clear (`NodeInst::unimply`, theme.rs). `modes_tests.rs` steps the kernel through inherit, unset, currentcolor, two refused values, a value, inherit, a value and unset against symbols.js's rule, and `js-symbols.test.mjs` the same in Chrome; iOS's segment test reads the image the installed segment was drawn from (`SegmentHost.drawn`).
  - On main with stage 9 A (the rebase): a path icon keeps A's rule (its tint, else its colour) and an SF Symbol takes B's; a path icon never takes the hierarchical face image (`faceImage`). So an untinted path icon with no written colour draws in its inherited colour where an untinted SF Symbol takes the platform's unsaid colour: both rulings, not a defect (theme2).

### Stage 9's gate: met, with one deviation (2026-10-19; the run is `~/x2theme/stage9-gate-run.txt`)

The clone at `3d3a4db` (H) on exact2 main: the server and web lines at `aa5cb16b1e`, H at
`02dd4316e2`, the icon lines and the regression reruns at `188517d645` (B landed). Stage 9's commits: D1 `f1ac0f4df2`, D2 `12846f3be3`, D3 `08475ffeb5`, E
`d731252e8d`..`7b4c2e4b2a`, A1 `6f5dd9004f`..`70f2a42327`, A2 `fcea1ec266`..`65d06ab882`, G1
`1d7e20015`..`60fccd36b`, G2 `697ac7206`..`69c770245`, G3 `ad166145d`..`6fb914c35`, G4
`c13fcdcde`..`b28c0c1a6`, F1 `44e834bfd`..`c11ac9f66`, F2 `419f6220f`..`f1ae334a2`, C
`e8089e1e86`..`558b8a0638`, the icon-alias fix `abb270eb5e`, H's empty `symbol:` name `3f3d5459d0`, B
`f7ed121e6b`..`188517d645`.

| Gate line | Result |
|---|---|
| An svg-only icon token draws on iOS and the Mac, tinted by a colour token, and follows a live retheme; `tamber-icon-missing` no longer asks Apple targets for an `sf` | **Met** (x2-perf4, on `188517d645`): an svg-only icon with no `sf` builds for iOS and macOS with no `tamber-icon-missing`, draws on both, tinted by the token through `-exact-tint-color` or `color`, and follows a live mode change of the token (#0a7f2e → #1a4fd6) with no rebuild. |
| Mac symbol images take their `color` and colour tokens | **Met** (x2-perf4): a Mac SF Symbol image takes its written `color` and the colour token, and follows the retheme; an unwritten one keeps the platform's colour. |
| `explain` answers an icon and a param; `uses` lists an icon's image sites | **Met** on a copy of the clone: an icon's sources and `draws` per shipped target, a param's place and default; `uses` of an icon written twice, once in a component used twice, gives three sites, each with its chain. |
| `set-row` edits a template colour in one line and hot-reloads; a stale digest is refused; a failing candidate writes nothing; promote to param and to token are each one transaction with one undo; a `class:` row's edit lands in the style's own file (T5) | **Met** on a copy of the clone, by the bytes: a one-line diff and undo gives them back; both stale digests (the op's and the files') refused; an unknown token and a wrong-kind value write nothing; each promotion is one transaction with one undo (the token's across two files); a style edit lands in its own file, and names the file to use where three files declare the style. Hot reload: on the JS dev loop, a `set-row` through the loop's own design server shows on the open page 303 ms later. |
| Inspect on a held node names its rows' tokens and the state part's spans (T4) | **Met**: LLP 1122 T4's run (`~/x2theme/t4-gate-run.txt`) after E: the held row reads `:active background-color var(--x-primary_600)` at the part's span. |
| On the web, an authored transition doesn't time a look; an iPad pointer shows a `:hover` look | **Met**: `looks.test.mjs` (a look snaps through an authored transition, entering and leaving); on an iPad simulator, `HeldStateTests.testIOSHoverLookAndHold`. **Deviation**: the iPad line is shown by that test, which drives the hover recognizer's callback; no real pointer was moved. |
| No regressions: release output unchanged where none of this is used; conform clean; LLP 1061's press corpus unchanged | **Met**, through B. Release output: caltrain, theme-fixture, shelf, realworld, tamber-host and the press corpus, built before and after each of 22 runs of stage 9 commits (176 comparisons), are byte-identical, but for F2's re-measured control-role cells moving the schema digest (8 header bytes, every plan). Conform (rt1, post-B): `--synthetic --strict` 0 failures in 98 targets; of the apps, native-fixture and interaction-gallery fail as before stage 9, line for line (QUEUE; accepted as unchanged). The press corpus's plans are unchanged by B. |

**H, the clone's icons:** the clone wrote no bare `symbol:sf/…`. Its settings rows' five computed
sources (`symbol:sf/${symbol}`) take icon tokens and role names now: 34 of their 55 SF names are
the same icon and 17 the same role, by the runner's rule. Four notification rows, whose names come
from data at run time, were no icon and no role and drew nothing off Apple; they are icon tokens
now. That needed one exact2 fix: an empty `symbol:` name is no symbol, as an empty `sf/` one is
(the principled fix, checking no value in an `if` arm that folds false, is in QUEUE).
**Also found and fixed in stage 9:** an icon naming another was refused by every fold but the
build's; in an app with params that let a write's role-axis check skip the design (`abb270eb5e`).
**B's movers:** B changes how hosts render, not any plan. On the Mac the clone's 8 menu and lightbox
icons under `color="text"` now draw in the text colour where they drew the unsaid symbol grey
(accepted by theme2).
**Not gate lines:** 8 Swift tests fail on an iPad simulator only, and one sheet-timing test is flaky
on iPhone (QUEUE). Also in stage 9: F1 and F2 (rt1), the native control follow-ups.

**Stage 9 is closed.**

### Stage 10, commit A1: one method→scope table (draft for theme2)

`scripts/tamber-pairing.mjs`'s `METHOD_SCOPE` gives every method of the wire one scope, checked in the
dispatcher before any handler. A method with no entry is refused (fail closed), and the change stream's
two paths go through the same table.
- `read`: hello, describe, resolve, explain, uses, history, export, inspect, and the streams.
- `propose`: propose (held or not), import, revert, and drop of the client's own held proposal.
- `write`: transaction, undo, commit, accept, drop of another client's proposal, pin, preview, outline,
  hold, and the three scrub methods (ruling 5).
- `local`, the local token only: pair-code, clients, revoke.
A request above its scope is 403 `{message, scope, needs}`. Opens `tamber/1.13`.
- **Read-only, below write:**
  - `inspect` `refresh` reads each build's last published selection and asks no build (a build's answer
    outlines on the developer's screens).
  - `export` keeps no DTCG baseline (keeping one prunes another client's).
  - `history`'s git runs with `--no-textconv`, `--no-show-signature` and `GIT_NO_LAZY_FETCH=1`.
  - The JS loop's live `propose` class reads the design with the built reader only: no build and no
    `cargo run`.
- **The held-proposal cap:** a client below write may not drop another's proposal, so at the 64-proposal
  cap its own oldest goes, else its hold is 409. This is decided in the same turn as the insert.
- **Review:** 3 rounds with security passes and three quick re-checks; the last found nothing blocking.

### Stage 10, commit A2: pairing, scoped tokens, clients and revoke (draft for theme2)

To the brief and theme2's approval: (a) drop-own is propose; (b) a revoked client's held proposals
stay, marked `revoked`; (c) 8 live codes, and 5 attempts a minute per peer.
- **Codes:** 50 CSPRNG bits (`XXXXX-XXXXX`), 2 minutes, single use, compared in constant time.
  - A failure counts against every live code, burned at 5.
  - A peer gets a burst of 5 attempts, refilled at 5 a minute, limited before its body is read.
  - Each burn and limit hit is logged with the peer, never a code.
- **Tokens:** 32 bytes, in memory only, kept by SHA-256, 12 h (`--pair-ttl`).
- **Clients:** named `remote:<name>#n` by the server, on every request and stream. A local client may not
  take a `remote:` name.
- **Local tool only:** `pair-code`, `clients`, `revoke`, and `bun exact.mjs tamber pair`.
- **No cookie** is ever read.
- **The local token** works only on a request the gate calls local. Through B's remote listener or a
  trusted tunnel nothing is local, and only `POST /__tamber/pair` answers without a paired token.
- **A revoke or an expiry:**
  - ends the client's subscriptions, long polls (401), scrub (restored), pins, inspect, outlines and
    holds, and the gateway's streams (`onEnd`);
  - makes a request still sending its body 401, and a scrub release mid-preview writes nothing;
  - lets a write already sent to the design server complete, as a local edit in flight would.
- **Deadlines are facts at once:** every read treats a client past its deadline as gone. Its end, and
  the callbacks, come from its timer or a sweep at each request and build report, never from inside
  another event's send.
- **The interface for C** (`lookup`, `onEnd`, `live`, `limiter`, `allows`, the table) **and for B**
  (`access.remote`, `tunnel`, `peer`) is the module's doc comment.
- **Review:** 3 rounds with security passes and six quick re-checks. Round 3's Grok and the last re-check
  found nothing.
- **Known limit (accepted, theme2):** five unauthenticated misses burn every live code, by the burn rule:
  a LAN peer can deny pairing, never gain it (rate-limited per peer; pairing is LAN-only, ruling 1).


### Stage 10, commit C1: the canvas gateway (draft for theme2)

Built to the approved plan (`~/x2theme/x2p4-s10c-plan.md` and its additions after approval) as a module on its own,
`scripts/tamber-canvas.mjs`, with A's interface and B's listener injected; under `tamber/1.13`.

- **The origin (theme2's ruling on C (a), the lead agreeing).** The canvas has its own origin: a second TLS port on
  B's remote listener, the same per-run certificate. It is the same site as the tool's port and a different origin,
  as the local tool (`127.0.0.1:7740`) and the framed page (`localhost:<port>`) are today, so `pageOf`'s guard
  holds: the framed page can't lift its sandbox or reach the tool's DOM, storage or token. D12's "same-origin" reads
  "served by the Tamber server under the paired token, never the app's dev URL handed out" (recorded in D12 by
  theme2). `/canvas/<client>/` is only the entry; the page is then served at its own root, so no path in the dev
  glue is rewritten and history URLs stay on the canvas.
- **The `canvas` method** (`read`; A's METHOD_SCOPE takes `canvas: 'read'`): `{url, grant, expires}`.
  - The grant: 256 bits from the CSPRNG, single use, valid 30 s, bound to the client. It is rate-limited per client
    by one A `limiter({rate: 6, per: 60 s})` the gateway makes, `take('canvas:<client>')` (a burst of 6, refilled at
    6 a minute).
  - The url: `<canvas origin>/canvas/<client>/`, never a loopback URL.
- **The entry**, `POST /canvas/<client>/`, with `grant` in a form or JSON body:
  - A query string, a GET, no Origin or one other than the canvas's or the tool's (`null` included), a wrong, used
    or expired grant, or another client's: not-found.
  - The grant is compared timing-safe (by its SHA-256) and burned on any try.
  - A session id (256 bits) is bound server-side to the client, its token and that grant.
  - The answer is a 303 to `/`.
- **The cookie rules:**
  - `__Host-exact-canvas=<session>; Secure; HttpOnly; SameSite=Strict; Path=/`, with no Domain.
  - A session is valid only by the gateway's map: a value it didn't mint is an unknown session.
  - A token is never read from a cookie, and no tool or design-server path reads `Cookie` (A's rule).
  - Cookies aren't port-scoped, and the canvas's page code is untrusted. Page code can set a cookie of that name on
    the host (the `__Host-` prefix only makes it Secure, host-only and `Path=/`), and it grants nothing on either
    port (a test sends one). At worst it overwrites or deletes the real cookie, which forces a fresh grant from the
    tool. A page-set value sent beside the real one doesn't hide it: each value is tried.
- **Every other request:**
  - It needs the session, then its client's token through A's `lookup` (a revoked or expired token fails at once,
    as does one whose `expires` has passed), then the route's scope through A's `allows(scope, needs, false)`: a
    canvas request is never local.
  - An unknown, missing, revoked or expired session: not-found, the same answer for each. A scope refused: 403
    `{message, scope, needs}`.
  - An allowed POST also needs `Origin` to be the canvas's, on top of SameSite=Strict.
  - No CORS header is ever sent.
- **The scope table** (everything else is not-found):

  | scope | routes |
  |---|---|
  | `read` | GET/HEAD of the page and its assets (any path outside `/__…`, `/.exact/` and the entry); `/__dev` and `/__dev/page` (SSE), `/__dev/hello`, `/__dev/commands` (its query kept), `/__dev/generation/*`, `/__dev/map/*`, `/__dev/inspect.js`, `/dev-inspect.json`, `/exact.json`, `/modules/*`; GET `/__dev/design`; the page's beacons POST `/__dev/reloaded`, `/__dev/painted`, `/__dev/gpu` (the wasm page's), `/__dev/report` |
  | `write` (theme2's ruling) | POST `/__dev/design`, `/__dev/checkpoint`, `/__dev/open`; GET `/__dev/open` (the opening page) |
  | never | `/__tamber` and `/__tamber/*` (through the proxy the loop would see a local request and hand out its write token), `/__dev/install/ios`, `/__exact/*`, `/.exact/*` |

  `/__dev/report` carries hold and held reports. Holding is a `write` command sent down the build channel, so a
  `read` or `propose` client's page reports only holds the server asked for (A's table, ruling 5).
- **Upstream:**
  - The request goes over loopback to the loop as the gateway itself, with `redirect: 'manual'` and the query kept.
  - Host is the loop's, and an allowed POST's Origin is the loop's origin.
  - Cookie, `x-exact-tamber-token`, Referer, the client's `forwarded` and `x-forwarded-*`, and hop-by-hop headers
    (and any the Connection header names) are dropped.
  - The gateway adds one marker of its own, fixed and naming no peer: `forwarded: for="_exact-canvas"`. Why: the
    gateway reaches the loop from loopback with a loopback Host, which the dev gate (`access.local`) would call
    local, and a local request is given the install token (the install pages) and the tamber token
    (`/__tamber/token`). With the marker the gate, unchanged, admits the request but never calls it local, so no
    route the allow list passes, now or later, can hand either token to a remote client. The gate never sees a
    paired credential.
  - The body limit is `forward()`'s (1 MiB less 64 bytes), but for `/__dev/checkpoint` (`write` only), which takes
    the loop's own cap (16 MiB, the JS loop's `CHECKPOINT_BYTES`, mirrored and checked by a test). Over it is a 413,
    then the connection closed.
- **Downstream:**
  - Set-Cookie, CORS, HSTS and hop-by-hop headers are dropped, with `cache-control: no-store`.
  - Loopback addresses are rewritten (theme2's ruling: D12's "no loopback URL handed out" is functional, not a
    security boundary). A candidate is a web or scheme-relative URL, or a bare `host:port` standing alone.
    - A URL's authority runs to the first `/`, `?`, `#`, `\`, whitespace, quote, `<` or `>`, so userinfo punctuation
      (`;`, `,`, `(`, `)`) stays in it. Only an authority that doesn't parse at all with a trailing `;`, `,`, `)` or
      `}` (a CSP's or a list's separator, a sentence's bracket) is read again without that punctuation, which stays
      in the text.
    - WHATWG URL's parse of it decides, nothing decoded first: a host that parses as loopback (127/8, localhost and
      `*.localhost`, [::1], one trailing dot dropped) at any port, the default included.
    - The loop's own origin (a loopback host at its port) maps to the canvas origin, in headers and in every body,
      text ones included. theme2's ruling point 2 blanks only OTHER loopback URLs: any other loopback authority
      becomes `loopback.invalid`, which never resolves.
    - No other host is ever touched: `http://127.0.0.1%2F@evil.test/a` and `http://127.0.0.1;@evil.test/a` are
      evil.test's and pass byte for byte.
    - Headers (Location, Link, CSP) and text bodies (HTML, JS, CSS, plain) by those candidates; JSON bodies (an
      envelope's type too) by parsing and rewriting each string, keys included, the bytes unchanged when nothing
      is; SSE a line at a time, a `data:` line that is JSON as JSON.
    - **A known limit:** escaped spellings in text (HTML character references, JSON `\u` escapes outside a JSON
      body, a percent-encoded URL) and spellings WHATWG URL doesn't map pass as they came. A missed one only breaks
      a link on the remote browser's own machine.
  - `frame-ancestors` names the canvas (`'self'`) and the tool's remote origin, with no loopback source.
  - Bodies are streamed with backpressure, as `forward()` streams them.
- **Revoke and expiry (A's `onEnd`):**
  - Each client's live proxied responses (SSE, long polls, in-flight requests) are kept, each with the
    AbortController of its upstream fetch.
  - On `onEnd(client, reason)` (`'revoked'` or `'expired'`, which end the same way; the reason goes to the log
    with the client's name, never a secret) all of them are aborted, so the client's streams close and the loop's long poll ends. The
    client's sessions and grants are deleted, and its next request is not-found.
  - `close()` unsubscribes.
- **The server:** `tamberServer({canvas: pairing => canvasGateway({loop, pairing, origin, toolOrigin})})` makes the
  gateway from the run's pairing and returns it (`canvas`, for B's canvas port to call `handle`); `close()` closes
  it. `canvas` is in A's METHOD_SCOPE as `read`, so A's dispatcher checks its scope like any method's. It then mints
  for the paired token the request came with. The local token passes the table but has no paired token, so it is
  answered 400 (the local tool frames the dev loop itself). With no gateway, the answer is 404.
- **Tests** (`tamber-canvas.test.mjs`, a fake pairing and a fake loop on loopback, plain HTTP):
  - the route table;
  - mint, entry, the cookie's exact attributes, 303, the page proxied;
  - the upstream sees no Cookie, token or client forwarding headers, and the loop's Host;
  - the client sees no upstream Set-Cookie or CORS header, and no loopback origin of the loop's in any header or
    passed body;
  - the query kept on `/__dev/commands`;
  - a grant reused, expired (an injected clock), wrong, another client's or in a query string is refused;
  - another client's, an unknown, a page-set or no cookie is refused, and a real one beside a tossed one still works;
  - `/__tamber`, `/__tamber/token` and the install routes are not-found and never reach the loop;
  - a cross-origin POST is refused;
  - `read` and `propose` are refused the `write` routes, and `write` passes;
  - revoke, then expiry: an open SSE stream and a held long poll close at once, the fake loop sees both upstream
    requests end, the next request is refused, and another client's stream is untouched;
  - the mint rate limit, and `close()` unsubscribing;
  - a `write` client's checkpoint between 1 MiB and the cap passes and one over it is 413, as is any other body over
    1 MiB, none reaching the loop;
  - with A's real pairing and dispatcher (a stand-in design server, the real wire): `canvas` minted by a remote
    paired client, entry, a proxied page and SSE stream, A's `revoke` ending the stream and the loop's request at
    once, the next request not-found and the token's next `canvas` 401; a `propose` client minting but refused the
    `write` routes while a `write` client passes; the local token 400, no or an unknown token not-found, the local
    token through B's listener not-found, a server with no gateway 404.
- **What waits on A and B:**
  - **A (reload1): landed.** The gateway runs on the run's pairing (`tamberServer(...).pairing`: each request acts
    as `lookup`'s client, and `onEnd`, at once for a client already ended, ends its streams). `limiter` and
    `allows` default to `scripts/tamber-pairing.mjs`'s. `canvas: 'read'` is in METHOD_SCOPE, and the method is
    answered in A's dispatcher (the early seam is gone).
  - **B (rt1):** the canvas's second TLS port gives `handle` its requests and the gateway its `origin`; the tool's
    remote origin is `toolOrigin`. The real-Chrome checks wait on B: the SameSite=Strict cookie reaching the framed
    canvas (the two origins differ only by port), the framed canvas unable to read the tool's origin, revoke closing
    both ports, and a page-set cookie granting nothing on either port.
  - **Then:** the test through the real dev loops (`devLoop`), a JS-target and a wasm page loading through the
    gateway and a live edit reaching each.
- **After review round 1 (Astra, Grok):**
  - The raw request target is checked, and the path forwarded is exactly the path authorized (`canonical`). A target
    the loop could read as another path is not-found: one not in origin form, an empty segment (a leading `//`
    too), a `.` or `..` segment, a backslash, NUL or other control character, an encoded `/`, `\`, NUL or `.`, a
    percent-encoded unreserved character, or a malformed escape. `/__` and `/.exact` compare without case.
    `//a//b/__dev/open`, `//127.0.0.1/__tamber/token` and `/%2eexact/install/` reach nothing (Astra 1, Grok 1).
  - Revoke ends every proxied request at once: its upstream fetch is aborted, and its request and response are
    destroyed, so a stalled upload and a download whose client never reads end too. The body read and the
    backpressure wait both observe the abort. A response stays tracked until it closes, not until `end()`. Requests
    are tracked per session, so a session evicted past the cap of 16 ends its own streams, and a client has at most
    32 requests in flight (503 past that) (Astra 2, Grok 6).
  - `/__dev/open` no longer hands a native client's admission token to anyone but a local request:
    `developmentLinks(app, page, access)` (host/apple/build.mjs) gives no links unless `access.local`. That covers
    the wasm loop and the resident loop behind the JS loop (the same code, its gate given the gateway's
    `forwarded` mark as it came). Local native clients keep their links (Astra 3, Grok 3).
  - The rewriter covers every loopback authority at any port, and the default port with none written, in
    percent-encoded host forms (`%6cocalhost`, `0x7f.1`, as WHATWG URL canonicalizes them). The loop's own origin
    maps to the canvas's; any other loopback authority becomes `loopback.invalid`. A bare `host:port` is rewritten
    only standing alone, so `https://example.test/localhost:9000.png` and `notlocalhost:9000` are untouched. One
    pass, so nothing it writes is read again. A loopback `toolOrigin` is refused (Astra 4, Grok 2).
  - The entry needs an explicit Origin, the canvas's or the tool's: a missing, `null` or malformed one is refused
    (Astra 5).
  - A `read` or `propose` canvas is refused `/__dev/checkpoint` (it stays `write`), so after a JS rebuild the JS
    loop's page now reloads afresh on that 403 instead of keeping the old page (Astra 6).
  - A session is looked up by its SHA-256 and the hash compared timing-safe, as a grant is (Grok 5).
  - The dev gate (install-page.mjs) treats `forwarded` and any `x-forwarded-*` header as forwarding (Grok 7).
  - Grok 4 (A's shapes) is settled in the plan.
  - Tests: the canonical table, and the same targets through the gateway reaching nothing; the address cases in
    headers, HTML, JSON and SSE; a stalled upload and a client that never reads, both closed at once on revoke with
    the upstream request ended; session eviction and the 32-request cap; the entry's Origin cases; the JS loop's
    client script parsing with the 403 fallback before the failure path; `install-page.test.mjs`, a later
    `x-forwarded-*` and the gateway's mark not local, with no admission links for it; `development.rs`, no links
    for a request that isn't local.
- **After review round 2 (Astra RIGHT WITH CHANGES, Grok RIGHT with two nits; theme2's ruling on the rewriter):**
  - The rewriter decides each candidate by WHATWG URL's parse with nothing decoded first, and never touches a host
    that isn't loopback: `http://127.0.0.1%2F@evil.test/a` passes byte for byte, in headers, text, JSON and SSE
    (Astra 1). A trailing dot (`localhost.`) is dropped in the loopback test. A bare `host:port` goes through the
    same parse, so `127.1:port`, `foo.localhost:port` and `[::ffff:127.0.0.1]:port` are caught (Grok 1).
  - JSON bodies are parsed and each string rewritten, so a `\u` escape in a JSON body is caught; a body with
    nothing to rewrite keeps its bytes; one that doesn't parse is text. SSE `data:` lines that are JSON likewise.
  - JS, CSS and plain text bodies are rewritten as HTML is; the URL-encoded and JSON-escaped text rules are gone,
    and escaped spellings in text are the known limit above (theme2).
  - `Range` and `If-Range` are not passed on, so a rewritten body is never a slice (Grok 2).
  - Astra's EADDRINUSE at fixture start: each test server binds 127.0.0.1:0 once and is never rebound; three
    concurrent runs here pass. A refused bind now fails with a message naming it (a sandbox that forbids
    listening).
  - Tests: the address cases in headers, HTML, JS, CSS, JSON and SSE, the evil.test URL untouched in each; a JSON
    body with no loopback byte for byte; a JSON key and a `\u`-escaped value rewritten; the known limits passing as
    they came; Range and If-Range not reaching the loop.
- **After review round 3 (Astra, Grok):**
  - A URL's authority now runs to the first `/`, `?`, `#`, `\`, whitespace, quote, `<` or `>` before WHATWG URL decides
    (theme2's ruling). `http://127.0.0.1;@evil.test/a` (evil.test's) passes byte for byte, and
    `http://user;name@localhost:<port>/a` is the loop's and is rewritten. Trailing `;`, `,`, `)` or `}` is set aside
    only when the authority doesn't parse with it, so a CSP's `http://127.0.0.1:<port>;` and a sentence's
    `(at http://localhost:<port>)` are still caught (Astra 1).
  - Astra 2 is answered by theme2: the loop's own origin keeps mapping to the canvas origin in text bodies, and ruling
    point 2 blanks only other loopback URLs (the downstream bullet says so).
  - Tests: `;`, `,`, `(` and `)` in userinfo, both a non-loopback host (byte for byte) and a loopback one (rewritten),
    in a header, HTML, JS, CSS, JSON and SSE; a CSP's and a list's separator and a sentence's bracket after a
    loopback authority.

  - Grok's nits: an https loop on its default port (no port written) is the loop's own origin, not another loopback (the port defaults from the scheme); addPort's same-origin check on the canvas port is C2's (its `origins`)
  - After the re-check (Astra): an authority that doesn't parse is read up to its FIRST `;`, `,`, `)` or `}` and the scan goes on from there, so a compact CSP (`default-src http://127.0.0.1:9000;img-src 'self'`) and a list of sources (`a,b`) are each rewritten; a userinfo `;@` still parses whole and is never cut.
  - After re-check 2 (Astra): that cut is made only past any userinfo (after the last `@`), so `(http://127.0.0.1;@evil.test:9000)` stays evil.test's, untouched, and `(http://user;name@localhost:9000)` is the loop's.
  - After re-check 3 (Astra): the cut is the LONGEST prefix that parses (still past any userinfo), so a host WHATWG reads whole, such as `localhost;evil.test`, is never cut into a loopback one. Other missed loopback spellings stay the known limit above (the lead: only a changed non-loopback host blocks)..

### Stage 10, commit C2: the canvas port (draft for theme2)

C1's gateway on B's remote doors, in both dev loops, under `tamber/1.13`.

- **The port:** with `--remote`, each dev loop (host/web/dev.mjs, host/web-js/dev.mjs) opens B's doors and then the canvas
  port: `openRemoteCanvas(tamber, asked, loop)` (scripts/tamber-canvas.mjs).
  - The port is another TLS listener from B's `addPort`, on the `--remote` host at a port the system picks. It uses the
    same per-run certificate, so the fingerprint the client pinned at pairing covers it. B's Host check (the
    certificate's names) and target check stand in front of it.
  - Its origin is `https://<the name B shows>:<its port>`, the same site as the remote tool and a different origin.
  - It is printed at start-up beneath B's lines. With no `--remote`, or with only `--trust-tunnel`, there is no canvas
    port and nothing changes.
  - A canvas port that cannot bind closes B's doors, and the loop says so and exits, as B's own failures do.
- **The origin is known only after the bind.** The gateway is the design server's own (`tamberServer`'s default
  `canvas`, made from the run's pairing), and it is dormant until `serve({loop, origin, toolOrigin})`. Of the three
  ways x2-perf4 listed, this takes the settable origin before first use. The port is bound with a handler that defers
  to the gateway, and the gateway answers nothing (not-found) and mints nothing (`canvas` answers 404, "this server has
  no canvas port") until it is served. So no request can see a half-made gateway. `loop` is the dev loop's own
  loopback origin, and `toolOrigin` is B's door (`info().origin`).
- **The fragment entry** (theme2's approval of dtool2's entry, SIGNOFFS.md). It replaces C1's grant POST from the
  tool's page.
  - The tool sets the frame's src to `<canvas origin>/canvas/<client>/#g=<grant>`.
  - `GET /canvas/<client>/` (no query) serves a static entry page: `no-store`, `referrer-policy: no-referrer`,
    `nosniff`. Its CSP is `default-src 'none'; script-src '<its one inline script's sha256>'; connect-src 'self';
    base-uri 'none'; form-action 'none'; frame-ancestors <the tool's origin>`. Nothing third-party, and no secret in
    it: the same page for every client.
  - Its script reads the grant from `location.hash`, then calls `history.replaceState(null, '', location.pathname)`
    before any request, so neither the address nor its history entry keeps the fragment.
  - It then POSTs `{grant}` as JSON, same-origin, with its own `referrerPolicy: 'same-origin'`. Under the page's
    `no-referrer` a POST would otherwise say `Origin: null`. On 204 (the cookie set) it does `location.replace('/')`.
  - Refused, it shows "This canvas link has expired; ask for a new one." and makes no further request; with no grant
    in the fragment, it makes no request at all.
  - Entered, and just before `location.replace('/')`, it tells the tool: `parent.postMessage({type:
    'exact-canvas-entered'}, <the tool's origin>)`, to that origin alone, with no secret and no client name (review2's
    T6 note, the lead). The origin rides in the page as `data-tool`, so the one script, and its CSP hash, stay the same
    for every gateway and client; with no remote tool there is no `data-tool` and no message. Tested: the call order
    (replaceState, fetch, postMessage to exactly the tool's origin, replace), no message on a refusal or without a
    tool, and the hash of the served script.
  - **A bounded exception to "no secrets in URLs":** the fragment carries only the grant: single use, 30 s, bound to
    one client, burned on any try, and never the token. A fragment is never sent to a server, a Referer or a log
    line, and the page removes it before its first request.
  - **theme2's conditions, each tested:**
    1. A grant is accepted only in that POST's JSON body, never from a query, a path or a form. A grant in any of
       those is never read, so it is not burned.
    2. Replay, expiry, another client's grant, and a POST from any Origin but the canvas's own (the tool's, none,
       `null`, malformed, a lookalike) are each refused.
    3. The real-Chrome gate shows that after entry the frame's `location.href` and `location.hash` carry no fragment,
       and that going back in the frame never shows it (gate lines (a0) and (a0, history)).
    4. The tool-origin POST path is gone: the entry admits only the canvas's own Origin. C2's draft `origins` on B's
       `addPort` is reverted, so `scripts/tamber-remote.mjs` is as B landed it, and the port refuses every other
       Origin, the tool's included.
- **`toolOrigin` must be https:** C1 refused a loopback `toolOrigin`. With B bound on loopback (a test, or a developer
  on one machine), the tool is `https://127.0.0.1:<port>`. The rule is now that `toolOrigin` is an https origin (B's
  door), never a plain one such as the local tool's `http://127.0.0.1:7740`.
- **Never plain HTTP from a LAN peer** (theme2's condition, with reload1's plain-LAN fix on the wire). A canvas session,
  grant or entry page is honoured only through the canvas's TLS door (`access.remote`, which B's `addPort` gives) or
  from this machine (`access.local`). The gateway's `handle` answers not-found to anything else, a plain request
  through a `--lan` bind included, and reads no grant, so none is burned. `canvas` is not minted for a caller that is
  neither local, remote nor tunnelled (404, the same as no gateway), whether or not reload1's earlier refusal on the
  wire is there. Tested: a valid session, the entry page and a valid grant through a plain-LAN door are each
  not-found and the grant then still enters through the TLS door; `canvas` from a plain-LAN peer is 404 and through
  B's door 200.
- **Revoke on both ports:** A's revoke ends the wire's streams and long polls, and through `onEnd` the canvas's
  proxied streams. The next request on either port is refused: 401 on the wire, not-found on the canvas.
- **Tests** (`tamber-canvas.test.mjs`, pinned TLS on loopback):
  - The server's default gateway is dormant before its port opens.
  - `openRemoteCanvas` on `127.0.0.1:0`: the canvas origin is printed, and a grant is minted over B's TLS wire door.
    The tool's Origin is refused at the port (403). The entry page is framed by the tool alone, and its POST from the
    canvas's own origin enters.
  - The proxied page carries the canvas origin, and another Origin, the tool's too, is 403 at the port.
  - The entry page: its headers and CSP, its one script by hash, no address in it. Its script, run in a stand-in
    page, calls `replaceState`, then the POST (the address already clean), then `location.replace('/')`. Refused or
    failing, it shows the message and makes no further request; with no grant, it makes no request.
  - The grant only in the POST's JSON body: replay, expiry, wrong, another client's, a query, a path, a form, and
    each foreign or missing Origin refused, none of them burning it.
  - A's revoke: the canvas port's next request is not-found and the wire door's 401.
  - No `--remote` gives no doors, and a tunnel alone gives no canvas port.
- **The gate in a real Chrome** (`/tmp/x2p4/s10c-gate/gate.mjs`, its run in `run.txt`):
  - A JS dev loop runs on apps/theme-fixture (by name; its design put back after) with `--remote 127.0.0.1:0`, loopback
    only.
  - A client is paired with a local code and then `/__tamber/pair` over TLS.
  - Chrome gets a fresh profile per run and trusts this run's certificate only by its SPKI pin
    (`--ignore-certificate-errors-spki-list`, read from the listener after its fingerprint is checked against the one
    printed). Everything is torn down after, on failure too; only the processes it started are stopped.
  - (a0) The frame's src is the entry with the grant in its fragment, as the tool will set it. After entry, the frame's
    `location.href` and `location.hash` carry no fragment, and no request URL but the src as set holds the grant. The
    joint session history has as many entries after entry as before the frame (2 and 2): the entry's address was
    replaced, so no entry holds the fragment. At
    the end, `history.back()` inside the frame never shows the fragment: the entry's history entry was replaced.
  - (a) The `__Host-` cookie is set (Secure, HttpOnly,
    SameSite=Strict, host-only) and sent to the framed canvas, same site and another port. The page loads, and a live
    edit of the design reaches the frame's stream.
  - (b) From inside the frame, `window.parent.document` and `window.parent.localStorage` throw, and fetches of the
    tool's and the loop's `/__tamber/token` fail.
  - (c) A's revoke ends the wire's subscription and the frame's EventSource, and the next request on both ports is
    refused.
  - (d) Page code's `document.cookie` cannot overwrite the HttpOnly `__Host-` cookie. After revoke, a request with only
    a forged value, a session-shaped forgery or the old real value is not-found. The wire answers not-found to a valid
    token or session carried only in a Cookie.
- **What the gate found, and fixed or names:**
  - **The tool's page sent `Origin: null`** (B's pages are `no-referrer`), which the port refused, as it should. The
    fragment entry settles it: the grant is posted by the canvas's own entry page, whose fetch sets its own referrer
    policy, and nothing depends on B's.
  - **`frame-ancestors` with B on loopback:** the CSP was rewritten after the remote tool's origin was added, so a
    loopback tool origin (`https://127.0.0.1:<port>`) became `loopback.invalid` and Chrome refused to frame the
    canvas. Now the addresses are rewritten first and `frame-ancestors` is fixed after: other loopback sources (the
    placeholder by then) go, and the tool's origin is added. Tested in `tamber-canvas.test.mjs`.
  - **The live edit's evidence:** a design edit the JS loop rebuilds for reaches the frame as its reload, and the
    framed page reloads through the gateway. That is what the gate records.
  - **Two observations outside C.** `--app <a path>` makes the JS loop write its module's crate into the app's
    directory (`buildModule`'s `out`); the harness passes the name. An edit of the fixture's `surface.page` (a
    token with a `when`) made the JS rebuild fail with "this app's design declares no axes"; the gate edits
    `accent.fill` instead. Both are QUEUE candidates, not C2's.
- **After review round 1 (Astra, Grok):** Grok RIGHT, no findings; Astra's one should-fix (proxied pages keep `'self'` in `frame-ancestors`) answered by the lead: `'self'` stays, since code on the canvas origin already is that origin and can do anything a frame of its own page could, and apps frame their own routes. The gateway's `frame-ancestors` is now exactly `'self'` and the tool's remote origin: any other source a page names (a foreign origin, the local tool's) is dropped, so no other origin frames a canvas page (tested with a page naming `https://evil.test`).
- **On main with the plain-LAN fix (81f2d32e8f):** the rule that a paired credential is taken only from this machine or through one of B's doors is one helper, `viaDoor(access)` in scripts/tamber-request.mjs, which the wire's check and the canvas gateway both call; dispatch's own `plain` flag is gone (the wire refuses such a request before dispatch).
- **After the re-check (Astra):** the tool origin is written into the entry page as an escaped attribute value, and `serve()` refuses a `toolOrigin` that isn't a plain https origin (a host name or address and a port), so neither the attribute nor the CSP header can be misread.

### Stage 10's gate: met (2026-10-19; the run is `~/x2theme/stage10-gate-run.txt`)

exact2 main at `29ec5ae613`, the clone at `baf8b6b` (a signed-out copy), a second Chrome profile as the remote
client trusting only the listener's certificate. Stage 10's commits: B `e85fb5c4d1`, `8f44ea2b8c` (and
pair-code's doors, `250ce13108`..`417d8c48f7`); A1 `98d26391ca`, A2 `1dbf03d789`; the plain-HTTP fix
`ae9e86e1ca`; C1 `f7ffce5644`..`78a7202c2b`; C2 `ec5c69fb0f`..`c92dace86a`; D (LLP 1122 T6)
`cd6300a22a`..`2ca911ea7f`. Wire: `tamber/1.13`.

| Gate line | Result |
|---|---|
| A second browser pairs over TLS, the fingerprint checked before anything is sent; a used code is refused and five misses burn the live codes | **Met** (1), both by the server and by the tool's own pair panel (1, D). |
| A paired `propose` client reads, inspects read-only and proposes; writes and commands are refused with its scope named; a `write` client writes and the event names it | **Met** (2); the tool greys what writes for a `propose` client and says why (2, D). |
| Revoke closes the client's subscription and held long poll at once and refuses its next request; separately, its canvas through the gateway, on both ports | **Met** (3), (3, C); the running tool says Revoked, its canvas goes, and it sends no more (3, D). |
| No wire auth reads a cookie; a cookie page code sets with the gateway's name grants nothing on either port | **Met** (4), (4, C). |
| The canvas on its own origin: entered through the gateway by the `#g=` fragment, which leaves the frame's address before any request and is in no URL or history entry; framed script can't read the tool origin's DOM, storage or token | **Met** (3, C), (5, C) ×3, and the tool frames it so and drops the grant once entered (5, D). |
| An unencrypted remote connection is refused, pairing included; the trusted tunnel takes paired tokens and pairing, never the local token; no non-loopback peer reaches the loopback-bound listener | **Met** (6) ×3. |
| Local clients unchanged; a release build has no Tamber endpoint; the server and tool suites green | **Met** (7) ×3: pairing 15, pairing-server 14, remote 27, canvas 31, tamber-server 30, dev-builds 3, design-tool 18. |

**Found by the gate's dry run and fixed in stage 10:** with `--lan`, a plain-HTTP LAN peer could pair. A paired
credential is now honoured only from this machine or through one of B's doors (`ae9e86e1ca`; one helper since C2).
**First final run:** 21 of 22; (5, D) read the frame's first `src` after the entry word had already dropped the
grant, a timing fault in the gate's check, not the tool. The check records each `src` as the tool sets it, and
the re-run met every line.
**Rulings written into LLP 1122 D12 with this close:** the canvas is on its own origin; `inspect` needs `read`;
the loopback rewrite is functional, not a security boundary; the hosted relay is stage 10c.

**Stage 10 is closed.** The `tamber/1.13` window is closed; the next wire change takes `tamber/1.14` (stage 11).

**A fix after the close, under `tamber/1.15` (it opens the version; stage 11 closed `tamber/1.14`): no answer names the machine's directories.** It changes what every client reads in a `file` field (a local one too: app-relative or `<package>/<path>`, no longer absolute), so it is a version, not a fix inside 1.14. `hello` reports `tamber/1.15`; docs/tamber-protocol.md opens its bullet.
`uses` gave a site's `file` as the source map's absolute path for a package's file and, past a link such as macOS's
`/tmp`, for the app's own. A remote client saw the machine's directories. Every site file in `uses` and in
`explain`'s icon sites is now named from the app's directory, as `explain` names a param's file
(`node_modules/@acme/reading/index.contract`), by one helper (`tamber_uses::shown`); a package linked from outside
the app keeps its path from `node_modules` on. A transaction's refusal names its file the same way. As a backstop,
the server drops the app's directory, as given or canonical, from every answer's text, so a read error says
`app.tamber: Permission denied`. Test: `tamber_serve_packages.rs` `no_answer_names_the_machines_directories`:
`uses` on a package component's param gives the relative path, and ten answers and their events, plus a read error,
hold neither form of the directory. It fails without the fix, checked by hand.
The JS server (theme2's ruling: leaking local directories to remote clients is a fix): `hello`'s `app` stays the
path for the local client, which `tamber tool` tells its own server by, and is the directory's name for any other
(paired, through B's door or a tunnel; stage 10c's relay has no door on main yet). Every reply, stream event and
long poll a client that is not local gets has the app's directory (as given and real) and the home directory
dropped (a selection's source in a package was the map's absolute path). docs/tamber-protocol.md says so. Test:
`tamber-remote.test.mjs`'s `no answer to a remote client names this machine's directories`, through the real design
server with the app under the home directory. A paired client through B's TLS door asks hello, describe, resolve,
explain, uses, a refused `set-row` on a package's file and a checker refusal with spans (`propose`, `transaction`),
then hello through the tunnel and a long poll. No reply holds the app's or the home directory in either form,
`hello.app` is the name, `uses` names `node_modules/@acme/reading/index.contract`, and the local hello is unchanged.
It fails without the fix, checked by hand. `outline`, which takes `uses`'s sites, reads a file named from the app's
directory as well as the map's own (its path, as given and real), so the design tool's outline of a token's sites
still finds them (`tamber-server.test.mjs`'s wasm inspect test, which caught it).

**After the path fix's review round 1 (Astra WRONG; Grok RIGHT WITH CHANGES; one rework):**
- **No blanket replacement.** The Rust backstop in `Server::handle` and the JS one over replies, events and long polls
  replaced the app's and home directory's text anywhere. That corrupted authored data: a token value holding
  `<app>/assets/icon.svg`, a history's text, `/tmp/x` inside `/private/tmp/x`, and `/Users/admin` inside
  `/Users/administrator`. The Rust one hit local clients too. Both are gone.
- **Path metadata through one rule.** A file is named from the app's directory where it is in it (as given or
  real); else from its package, the nearest directory above it whose `package.json` has a name
  (`<name>/<path inside it>`, as D15's `sources::home` names a package's file, so a `file:` link outside the app and
  the home directory is unique and stable); else `<outside>/<its name>`. Rust keeps it in `tamber_uses::shown` (for
  `uses`, explain's icon sites and param file through `AppParams.files`, and a refusal's diagnostics), and JS in
  `scripts/tamber-paths.mjs` `pathName`. JS applies it, by `file` fields only, to every reply and every event for
  every client: a selection's source and chain, `inspect`, and the long poll. The stale-map and moved-file messages
  of `tamber-inspect.mjs` name their file the same way.
- **Machine-written messages are scrubbed at path boundaries,** for a client that is not local. The app's
  directory becomes `<app>` and the home directory `~`, longest first, in the given and real forms, only where a
  whole path starts and ends. It applies only to an `error`'s messages (and its diagnostics') and the `down` event's,
  including the two 503s that bypassed it: a subscribe while down, and a long poll past the `down` event. Authored
  text is not read.
- **`outline` matches by the rule.** Each location's file and each map node's file are named by `pathName` and
  compared, so a package linked from outside the app round-trips and two packages' same-named files stay apart. A
  map's named nodes are kept per map read, since a map can move under one plan digest.
- **Tests:**
  - `scripts/tamber-paths.test.mjs`: the rule (inside the app, through its real path, through a link inside it, a
    package's own path outside it, outside both); the boundaries (`/private/tmp/xy`, `<home>istrator`); and that
    only an error's and `down`'s messages are scrubbed, never a commit's.
  - `tamber-remote.test.mjs`, through the real design server, with the app under the home directory and
    `@acme/linked` linked from outside both. A build is registered on the dev loop's channel whose map names the
    package's real path. A paired client through B's door asks hello, describe, resolve, explain, uses ×2, a
    refused `set-row` and a checker refusal (propose, transaction), and inspect. It also reads the selection in the
    long poll (`@acme/linked/index.contract`), outlines `uses`'s site (it reaches the build: sites `[0]`), and asks
    hello through the tunnel. No reply holds the app's, the home or the linked directory in either form, except
    describe's authored metadata. Authored text (the token's `help` holding the app's path, the commit's message
    naming it) is byte-identical to the local and the remote client.
  - A design server that is down (it says the app's path on stderr): the remote subscribe and long poll are 503 with
    `<app>`, and the `down` event in a long poll from the start is scrubbed too. Both tests fail without the
    rework, checked by hand.
  - `tamber_serve_packages.rs` `authored_text_holding_the_apps_path_is_kept`: a local proposal's diff, describe's
    metadata and a scrub's fold keep a `help` value holding the app's path byte for byte.
  - `tamber-server.test.mjs`'s wasm inspect test now expects the selection's file as `app.contract`.
- **Not covered (round 1's view; wrong, see round 2):** a machine message naming a linked package's directory outside
  the app and the home directory. A compile refusal does name one.

**After the path fix's review round 2 (Astra WRONG; Grok RIGHT WITH CHANGES; their findings overlap):**
- **A linked package's directory in a message is its name.** A compile or load refusal names a file by its path: a
  missing named design (`sources/packages.rs`), an unreadable `use` target (`sources.rs`), resolution
  (`resolve.rs`), and a transaction's diagnostics, which `uses`, `explain` and the others carry as `the app does
  not build: …`. Round 2 rewrote each package root found under the app's `node_modules`. Its quick re-check found
  that misses a nested dependency, a `node_modules` installed above the app, and Bun's per-file links (whose files
  link into a store). So the scrubber no longer lists roots. In a machine message to a client that is not local,
  every absolute path (a run from `/`, `~/` or a drive letter, up to white space, a quote, a backtick, `)` or a `:`
  before a line number) is named by `pathName`, but a server's route (`/__…`), exempted by name: app-relative, its
  package's `<name>/<path inside>`, or `<outside>/<name>`. It resolves from the path's nearest existing ancestor
  (realpath, then the nearest named `package.json`), so a missing file under a missing directory of a package is
  still the package's. (Re-check 2: a guard that kept a token unless it or its parent existed let a manifest's
  `dist/theme.tamber`, never built, through whole.) Rust's `shown` resolves the same way. A `:line:col` after it
  stays, and so does a sentence's full stop. The app's directory itself reads `<app>`, the home directory `~`, and
  the app and home rule stays as a backstop for any other mention.
- **Motion sites through a linked package keep their site.** `motion_sites` named its files for the reply before
  `motion_instances` joined them against the elements' paths, so a linked package's duration or easing use lost its
  instance (`site: null`) and its outline. It keeps the loader's path through the join; the finished reply is named
  (`relative_files`).
- **explain reads where the file is.** `AppParams` keeps each source's path on this machine (`paths`) beside its
  name (`files`). explain reads a param's default as written through the path, so an outside package's `"4px"` keeps
  its spelling. Only the emitted `file` is the name.
- **The rest named too:** the subscribe snapshot (named, and scrubbed for a client that is not local), the long
  poll's `proposals`, and export's `baseline` (`shown`, no `unwrap_or` to the absolute path).
- **Tests:**
  - `tamber-paths.test.mjs`: each path in a message is named and keeps its `:line:col` (`app.contract:3:7`); a
    route is left, and a missing file under a missing directory is still named (`dist/theme.tamber`); a longer name
    beside the app's or the home's is named from what exists above it (`<outside>/…`). A package's file is `<name>/…` however it is installed: a
    link outside the app, a dependency nested in its `node_modules`, a `node_modules` above the app, and a Bun-style
    per-file link into a store.
  - `tamber-remote.test.mjs`: a design server that is down, whose stderr names a package's file outside the app
    and the home directory, reads `… writes <app> (it read @acme/far/index.contract:2:1; no @acme/far/dist/theme.tamber)` to a remote client, in the
    503s and the `down` event. `@acme/linked`, outside the app and the home directory, names a design under a `dist/` that was
    never built (`dist/theme.tamber`). The local client's `uses` refusal holds the path; a paired client's refusals (`uses`, `explain`) hold
    neither the outside directory nor the app's nor the home's, and `uses` says `@acme/linked/missing.tamber`. It
    fails without the package rewrite.
  - `tamber_serve_packages.rs` `a_package_linked_from_outside_keeps_its_motion_sites_and_its_defaults`: a linked
    package outside the app. `uses` of a duration its view writes finds each instance (`site` set), named
    `@acme/linked/index.contract`; `explain` gives the param's default as written (`"4px"`); neither names the
    outside directory. It fails with either fix reverted.

**After the path fix's re-check 3 (Astra; theme2's ruling: refuse it):**
- **A package outside the app needs a name.** A linked package whose `package.json` has no `name` got an identity from its place from the app (`../../../Users/alice/shared/cards/index.contract#Card`),
  which `describe` and `explain` sent whole and every param's full name carried. It is now a compile refusal,
  `contract-package-unnamed`, at the `use` that reaches it: "the package `a` is outside the app and its `package.json`
  has no `name`: a package outside the app needs a name in its package.json", naming the package as the `use` does,
  never its path. `sources::home` answers none for it and the loader refuses, so no relative identity is made.
- **A linked package ends at its root.** `home` walked every directory above a file to the app's, so a package with no
  name inside an outer directory whose `package.json` has a name took that name: the same leak by another route. A
  package's file is now named only by a directory between it and its package's root. A package that holds its own
  test app (`packages/geolocation/test/fixture`) is its own: its root's name names it, where it was `../../…` before.
- **Apps unchanged.** `contract build --json --errors-only` of all 90 `app.contract` roots in the checkout, before and
  after: the same diagnostics (74 build, 16 refused as before). None hits the refusal; the geolocation fixture did
  until the package-holds-the-app case above.
- **Tests:** `tamber_params_tools.rs` `an_unnamed_package_outside_the_app_is_refused_even_inside_a_named_directory`
  (two unnamed linked packages, refused at the app's `use` line with the package's `use` name and no directory of
  theirs; again inside an outer named directory; named, two identities by their names) and
  `a_package_holding_its_own_test_app_names_its_files_by_its_name`; `tamber-remote.test.mjs`'s
  unnamed `@acme/linked` linked from outside the app and the home directory: a paired client's hello, describe,
  explain, resolve and uses hold neither its directories nor its name nor any `../`, and `uses` says the refusal. Each
  fails without the change.

### Stage 11, commit B1: the base scale table (draft for theme2)

Built to the approved plan (`~/x2theme/x2p4-s11b-plan.md`, theme2's 09:56 approval and its conditions) and theme2's 10:57
ruling on the measured cells. The brief is stage 11 part B, with rulings 3 to 6.

- **Nine base names** (ruling 5): `space.xs`, `space.sm`, `space.md`, `space.lg`, `space.xl`, `radius.sm`, `radius.md`,
  `radius.lg` and `radius.full`. A view reads one with no design defining it. An app may set any of them in its design
  (ruling 4), and its value wins app-wide, as any token's does. No other name is reserved, so the clone's `space.header`
  keeps its meaning.
- **The table:** `scale` in `kernel/tables/schema.json`, a row per name: `[name, ios, mac, neutral, material,
  terminal]`, px (points), never scaled by Dynamic Type (ruling 6).

  | name | iOS | Mac | neutral | Material | terminal |
  |---|---|---|---|---|---|
  | space.xs | 5 M | 6 M | 4 neutral | 4 G | 0 |
  | space.sm | 8 =grouped-row-gap | 8 = | 8 = | 8 = | 1 cell |
  | space.md | 16 =grouped-row-inset | 10 = | 16 = | 16 = | 1 cell |
  | space.lg | 20 M | 20 M | 24 neutral | 24 G | 2 cells |
  | space.xl | 35 M | 30 =grouped-label-inset (stretch) | 32 neutral | 32 G | 2 cells |
  | radius.sm | 5.6 M | 5 M | 4 =mark-radius | 4 G | 0 |
  | radius.md | 17 M | 6 M | 8 =control-radius | 8 G | 0 |
  | radius.lg | 26 =grouped-card-radius | 10 = | 12 =surface-radius | 12 G | 0 |
  | radius.full | full | full | full | full | 0 |

  **No web column** (theme2, 2026-10-19): every web build (the wasm host and the JS target) throws the neutral switch,
  so the browser reads the neutral column (LLP 1115.000 D17 as corrected). `Column::Web` is read only by a process that
  says no column, and as Material's fallback; for the scale it reads the neutral cell (`style::scale::cell`, held by a
  test for every name). The draft's `@ios` cell form is withdrawn with it.

  Material's G cells are Material 3's spacing and shape scales. The neutral look's own steps (4, 24, 32) are its
  definition (LLP 1115.000 D17), not Material's, though they coincide. Each look's steps are in order.
- **The cell forms** (theme2 (a)). A cell is never a bare number, and the build (`kernel/build/scale.rs`) refuses
  anything else: arithmetic, a missing source, a dangling metric or one with no px in that column, a reference to another
  column (`@…`), and `full` outside radius.full or in the terminal.
  - `=<metric>`: that metrics row's value in the same column, read when the kernel builds, so the cell cannot drift.
    Used only where the metric names the same role (theme2 (b)): an inset is space.md, a row gap space.sm, a control's
    corner radius.md, a card's or surface's corner radius.lg, a mark's corner radius.sm. The one exception is a role
    stretch theme2 ruled, recorded with its reason (the Mac's space.xl, below).
  - `{px, measured: id}` and `{px, guideline: id}`: `id` is an entry of `docs/scale-measured.md` with what was measured
    and how (or the citation), the number, and the reason for the choice.
  - `full` (radius.full): one oversized length, `style::scale::RADIUS_FULL` (9999), named once in the kernel crate.
    Every host clamps it into a pill by CSS's corner-overlap rule (stage 11 C), never `50%` (theme2's C ruling).
  - The terminal: `{cells: n}` for a spacing (`n ch`), `0` for a radius (it draws none).
- **Measured** (`bun scripts/measure-scale.mjs mac | ios --udid <sim>`, `scripts/measure-scale/{ios,mac}.swift`; raw
  output in `~/x2theme/s11b/`):
  - iOS 27.0 simulator: UIKit's own values where its API states them; otherwise the inked corner at 3×, read as the
    equivalent circular radius along the 45° diagonal. Check: the inset-grouped card measures 26.07 against the table's
    26.
  - macOS 27.0: AppKit's API on alignment rects; corners from the window server's composite of the tool's own window
    (AppKit 27 bezels reach no in-process capture), at 1× on this display (±0.7 pt, recorded as whole points).
  - Choices theme2 ruled (10:57):
    - iOS space.xs is a small button's top inset, 5 (UIKit states no 4).
    - iOS space.lg is a large button's leading inset, 20 (an iPhone's margin is 16, space.md's).
    - iOS radius.sm is `cornerStyle .small` at 44 points, 5.6, a fixed length recorded with that height. UIKit's
      `.small` scales with height; iOS 27 has no mark.
    - iOS radius.md is the medium button's measured corner, 17: iOS 27's controls are capsules, and 17 is a length,
      not `full`.
  - **space.xl is the gap between grouped sections without a header, on every look.** iOS's 35 was measured without a
    header (`.insetGrouped`, no headers), and theme2 then ruled xl headerless on every look. The Mac's headerless gap
    between a SwiftUI `.grouped` Form's sections measures 10, below its space.lg (20). So the Mac's space.xl is a ruled
    role stretch: 30, `=grouped-label-inset` (a grouped section label's inset, the Mac's largest grouped spacing in the
    table), with its reason in `docs/scale-measured.md`. A gap measured with a header (56) was withdrawn, since a header
    mixes a text height into a spacing.
- **The fallback** (theme2 (c)): `theme::measure` reads a base name no entry defines as the scale's length in the
  process's column, bound theme or none.
  - A name the design defines is the design's, even in a row that can't take it, so the base never stands in silently.
  - The read is recorded as a token's, so an edit that defines the name later sets the row again.
  - The column is the host's (`metrics::set_column`). This commit adds `Column::Terminal`: the `web` column's value
    in every metric (the terminal read it before) and the terminal's own in the scale. The hosts set their
    columns in B2.
- **theme2's conditions, and where each is held:**
  - **The compiler knows the nine names** (`contract/lower/src/scale.rs`, `theme::measure_values`). A view reading one
    compiles with no design. Its values are the px columns', which a row checks as a token's. It varies by column, never
    by an app axis, so an inherited row reading it is no `tamber-inherited-axis`.
  - **An app definition must be a length.** `tamber-base-kind` refuses a base name set to a colour, a number or any
    other kind. A length that varies by an app axis is the app's.
  - **One table, which describe, resolve and explain read** (`exact_kernel::style::scale`, public, every cell with its
    source). The wire's half is split out of this commit: the design server's `describe`, `resolve` and `explain` of
    the base names move to rt1's stage 10 D under `tamber/1.14`, reading this table, never a copy. B1 touches no wire,
    and `tamber/1.13` is unchanged.
  - **Retention keeps nothing for an unread base name.** The plan's theme carries no entry for a base name the design
    leaves unset, since the kernel holds every column. One the design sets is kept only when read, as any token is.
  - **The monotonic test** (`style::scale` tests): within each look xs ≤ sm ≤ md ≤ lg ≤ xl and radius sm ≤ md ≤ lg,
    with radius.full left out.
- **SCHEMA_DIGEST** moves once, in the stage's window: `0xd348_8e13_baf4_12bd` (main, after 7ce6cb98db) → `0xc76c_20d0_6202_3b03` (on the branch's base before that rebase, `0xad42_cc2d_c5b5_26f3` → `0x45a9_8459_9339_c7bf`)
  (`kernel/src/wire/codec.rs`; the `scale` table, the metrics' terminal column and theme2's unit flag, `style length`.
  Rounds 1 and 2 pinned `0x374f_477d_d305_5b7e` before the flag, round 3 `0x1360_51a6_5170_bf6a` before the `calc()` guard, as `codec.rs` is hashed). Nothing else pins it.
- **QUEUE:** "A Dynamic Type opt-in for the base scale" (ruling 6, the lead's assignment to this commit).
- **Tests:**
  - `kernel/src/style/scale.rs`:
    - the nine names;
    - every cell equals its source, re-read from the metrics table;
    - `Column::Web` reads the neutral cell for every name;
    - each look in order;
    - every measured and guideline id has its entry with the number;
    - the fallback: unbound, a design's own value winning, a defined name a row can't take not falling back, the read
      recorded.
  - Every build refusal was checked by hand on a mutated schema.
  - `contract/cli/tests/it/tamber_base_scale.rs`:
    - a bare app compiles with no plan entry, and the runner reads the kernel's values;
    - an app's own `space.md` wins and an unread `space.lg` is not kept;
    - a colour or number definition is refused, and an axis-varying length accepted;
    - an inherited row is no axis refusal, a number row is `tamber-kind`, and a name that is no base fails.
- **radius.full** (theme2): an app that sets `radius.full` sets a length, as for every base name. `full` is the base's
  own value, the clamped oversized length; an app that wants a pill leaves radius.full unset rather than overriding it.
- **B2:** the hosts' columns: the JS target's CSS variables (until then a JS-target page does not resolve a base name),
  Linux and Android wiring, and the terminal host's `set_column(Terminal)`, each with its unit tests.
- **After review round 1 (Astra RIGHT WITH CHANGES, Grok RIGHT):**
  - **Base names in design values** (Astra 1, BLOCKING; theme2's (a)). A design value or `when` arm may name a base name
    the design leaves unset bare: `card.pad: length = space.md` is an alias.
    - The vocabulary passes the nine names to Tamber (`Vocab::bases`). For a base name the design does not set, Tamber
      reads a bare one as a named length, kept as written and never folded, the way a role reference survives.
    - The kernel reads a length token whose value is a bare base name as the base in its column, the design's own
      definition first (`theme::measure`). Both reads are recorded, so an edit of either sets the row again. A base name
      set to itself reads the scale.
    - A base name the design sets is its token, folded as any token reference is. Its value wins through the alias, an
      edit of it re-folds the alias for the design server's batch, and a ranged param naming it keeps its px. theme2
      accepted this (2026-10-19): "never folded" applies to an unset base name only. D11 is amended with both cases:
      unset, a per-host reference never folded; set, the app's token, folded as any reference.
    - An expression over a base name the design leaves unset (`calc(space.md + 4px)`, a shadow's length) is refused,
      `tamber-base-expression`, telling the author to name it bare. QUEUE has the later work. Over one the design sets,
      it is that token's expression.
    - Tests:
      - `contract/cli/tests/base_alias.rs`, its own process: the plan's `card.pad` is `space.md` as written; it reads
        the neutral, Mac and iOS columns' lengths, and its `when tone=wide: space.lg` arm too; an app's `space.md`
        folds through and re-folds on an edit.
      - `tamber_base_scale.rs`: the expression refusals.
      - `style::scale`: the kernel's alias reads.
  - **An icon named as a base name is refused** (Astra 2). `tamber-base-kind` covers icon declarations as well as tokens
    (at lowering then; round 2 moves it into the shared checker), with a refusal test. A dial's name is an identifier, so none can be one.
  - **The build checks each source against its entry** (Astra 3): a `measured` or `guideline` id with no entry in
    `docs/scale-measured.md` stops the build, and the page is a rerun dependency. The rerun is said in
    `build/scale.rs`, not `build.rs`, which the schema digest hashes, so the digest does not move.
  - **Each documented value is bound to its step** (Astra 4). Every entry has a `Binds:` line (`space.xs = 4`), and both
    the build and the provenance test require the cell's exact `name = value` there. A shared entry's other numbers no
    longer stand in: Material radius.md 8 → 4 now fails the build.
- **After review round 2:**
  - **The runner's length binding reads an alias** (fix 1). `Tamber::length` reads its token through `theme::measure`
    at the binding's tuple, so `card.pad = space.md` resolves through the kernel's scale in the process's column, as a
    style row's does. `base_alias.rs` binds `Token<Length>` reads of `card.pad` and `space.xl` in each column, after a
    mode change and after an edit to `space.lg`.
  - **The DTCG export of an alias to an unset base** (fix 2). It wrote 1px; now it writes the neutral column's px from
    the kernel's table (the web's look; radius.full its 9999), the name kept in `lossy`. The export's conversions say
    so. `contract_lower::theme::vocab` lists the base names' lengths in `sizes`, which `tamber gen`'s CSS reads too, so
    neither writes a bare `space.md` outside Exact. Test: `tamber_base_scale.rs`.
  - **`tamber-base-kind` is the shared checker's** (fix 3). `check_design` refuses a base name declared as an icon or
    as any token but a length, by `Vocab::bases`, so a write's plan refuses it before it lands, as the build does; the
    lowering's copy is gone. Test: `tamber_serve.rs` (`propose` and `transaction` refused, file unchanged, a length
    accepted).
  - **A scale cell names a metric that holds a length** (fix 4). A `style` row's bare number may be a weight or a flex
    factor, which the table didn't mark. Round 2 asked the name's ending; theme2 ruled for a per-row unit flag
    instead (below).
- **theme2's unit flag** (ruling after round 2, in place of fix 4's name endings):
  - **The marker.** A `style` metric written on a length property says so in the schema: its kind is `style length`.
    It applies to a gap, an inset, a corner, a width or height, a padding, a margin or an offset: 59 of the 83 `style`
    rows. Its number is px; its other cells are `unset`, `auto`, a percentage, an `em`, `rem`, `px`, `vw` or `vh`
    length, or a `calc()`. The generated `Metric` carries it as `length`, true for a `length` row too, and its kind
    stays `Style`, so every reader of a style row reads it as before.
  - **The build holds the marker to the row both ways** (`codec.rs`, `style_length_marker`):
    - a `style` row whose name says a length property (every name but a colour's, a line style's, a position's, a
      font's, a weight's, a switch size's, an alignment's, an appearance's, a box model's or a flex factor's, as the
      row test reads names) is refused without the marker;
    - a marked row is refused where its name says no length, or where a cell holds no length.
  - **A scale cell may name only a `length` or `style length` metric** (`build/scale.rs`); the name endings are gone
    from it.
  - **The kernel's row test reads the marker** (`style::metrics`): `every_value_is_one_a_row_takes` takes a row's being
    a length from the marker, and only which property it is written on from its name. A new test,
    `the_length_marker_is_the_names_length_rows`, holds the marked set to the set the names gave before the flag,
    exactly (59 rows), so nothing the kernel reads moves.
  - **Checked by hand on a mutated schema, each refused with its message:**
    - `grouped-row-gap` unmarked;
    - `grouped-header-weight` marked;
    - a marked row with a keyword cell;
    - `=grouped-header-weight` in a scale cell.
  - **The digest moves within B1** (above): `0x374f_477d_d305_5b7e` → `0x45a9_8459_9339_c7bf`.
- **After review round 3 (Astra RIGHT, Grok RIGHT WITH CHANGES):**
  - **A binding reads the terminal's cells** (Grok, should-fix). Under `Column::Terminal` the scale's spacings are
    cells (`1ch`, `2ch`), which `Tamber::length`'s points read as no length. It now reads a cell as its px
    (`style::cells::parse`). `base_alias.rs` checks the terminal column: space.sm 8, space.lg 16, radius.md 0, and
    `card.pad` 8, then 16 under `tone=wide`.
  - **A host's length read is read aside** (Grok, nit). `Tamber::length` wraps its `theme::measure` in
    `theme::recorded`, as `color` does, so its read joins no style parse's names.
  - **Every misdeclared base name is said at once** (Grok, nit). Beside an icon's refusal, `check_design` appends
    `misnamed` too. Where the checker refused another token, it still has every token's kind, written or inferred
    (`check::check`'s refusal carries them), so `space.lg = #ffffff` is said beside it too (the re-check's nit). Test: `tamber_base_scale.rs`
    `every_misdeclared_base_name_is_said_beside_other_refusals`.
  - **A marked row's `calc()` is a length** (the re-check, Astra). The marker's guard parsed any `calc(…)`. It now
    reads CSS's typed grammar: numbers and lengths (`px`, `em`, `rem`, `vw`, `vh`, a percentage), `+` and `-` between
    two of one type and spaced, `*` by a number, `/` by a number, and parentheses; the result must be a length
    (`codec.rs`, `calc_length`). Checked by hand on a mutated cell:
    - refused: `calc(1deg)`, `calc(garbage)`, `calc(2 * 3)`, `calc(100%-16px)` and `calc(100% - 16)`;
    - accepted: `calc((100% - 16px) / 2 + 1em * 2)`.
### Stage 10b, E2+E3 follow-up: re-import judged whole (theme2's option B, 2026-10-19)

E23's round 5 found super-linear paths in the importers. They are fixed in two steps.

- **First, the exact part.** Outputs are byte-identical:
  - the DTCG importer walks each path's alias chain once per tuple;
  - re-import's transaction is tried on the text it has accepted, instead of the whole prefix again;
  - context and design lookups go by name;
  - one check replaces apply-then-check.
- **Then option B.** A proposal is applied as the whole edit it is, so it is judged whole:
  - **Each token is planned.** Its lines are planned without a check.
  - **Then rounds.** Every pending token's lines are applied together (`edit::stepped_all`: many
    `set-token` and `set-dial` operations with one parse). That gives the same text and the same refusal as stepping
    them one at a time: each operation's lines are parsed alone, and anything else falls back to stepping. The
    result is checked once, and each token's values are read from that folding.
  - **Re-asking.** The pins a token needs are added for the next round. A round that adds a pin or drops a token
    asks every token again, so a token is settled only on a round that changes nothing. There is no count of
    rounds: each change is a pin (three a token; a fourth refuses it), a refusal, or lines that leave, so the rounds
    end. A token settled beside a change that is later dropped is asked again without it (`export_dials` test).
  - **Dial candidates are judged alone.** A token that may be a dial's change (D14) is judged on its own. Its
    literal lines are only a fallback, and `export_dials::dials` decides which applies afterwards.
  - **The transaction.** The dials' changes come first, then the tokens, judged the same way. A refusal reads "the
    design would refuse it with the rest of the proposal (…)", where it used to read "…with the tokens before it".
- **A refused set** is resolved deterministically, in the export's order of names (`export_batch::judged`):
  - each diagnostic on a member's token or dial line refuses that member, with its own diagnostic as its reason;
  - when a diagnostic is on any other line (an unchanged token refused for what a change made of a token it reads),
    the set is halved, the first half before the second, each half on what was taken before it.

  `judged` checks its base, then once more when nothing is refused.
- **Where the report differs from per-token checking.** These are the two interaction cases (tests:
  `export_batch::tests`):
  - **A token refused alone but taken with the others now lands with them.** For example, `a` naming `b` as a
    colour while `b` changes from a length to a colour. It used to be reported.
  - **A token taken alone but refused with the others is now reported with its own reason.** For example, `a` naming
    `b` while `b` becomes a length. The refusal is on `a`'s line, so `a` is refused and `b` lands. It used to be taken,
    then refused in the transaction.
  - A dial's change and another token's literal, both reading the dial, land together (`export_dials` test).
  - The theme-fixture and Bluesky designs, with values flipped at four densities, report identically before and
    after.
- **Measured** (release, every value changed):

  | Design | Before | Now |
  |---|---|---|
  | 400 colour tokens, each with a `when` | 51.6 s | 0.03 s |
  | 800 colour tokens | 5–7.6 s (exact part) | 0.07 s |
  | 3,000 colour tokens | about 90 s (extrapolated) | 0.27 s |
  | 2,000 tokens over 64 contexts | not measured | 2.1 s |
  | 800 refused tokens (refusal-heavy) | not measured | 0.03 s |
  | 3,000 refused tokens (refusal-heavy) | not measured | 0.11 s |

  - The DTCG export (which re-import runs on the design) found groups by scanning every name for every token in
    every context. It now reads each name's dotted prefixes once: 0.7 s for 3,000 tokens over 64 contexts, which
    took 4.5 s for 2,000 before.
  - `judged` alone: 3,000 members all refused on their own lines take 30 ms. Refusals on other tokens' lines are
    found by halving, about two checks for each refused member at each halving.
- **The rest of rt1's entry** (iosfix, x2/import-rest; theme2's 19:07 ruling; the lead's 18:35Z task):
  - **The budget, calibrated to the isolate.** An import runs in a child of about 1 GiB (`tamber-isolate.mjs`), and a
    budget that outruns it turns the budget's refusal into an RSS kill. Measured (release, designs over 64 contexts,
    one `when` per token): a fresh DTCG import holds about 12 bytes of RSS for each unit, a re-import about 30 (it
    exports the design to compare, each context's tree now charged as it is built). So `budget.rs` has a limit per
    path, each refusing before the isolate's kill (at 90% of 1 GiB): `IMPORT` = 3 × 2²⁴ (about 600 MB, 59% of 1
    GiB) and `REIMPORT` = 3 × 2²³ (about 750 MB, 74%); before, one `LIMIT` of 2²⁶ let a re-import reach about 2.9
    GB. The server's check of a client's baseline against the design's own export (`contract tamber` `plan`) builds
    that export inside the re-import's budget too (`export::own_digest`), so a design too large for one re-import is
    refused there with the budget's message, not exported whole first or told to pull.
  - **Supported size:** about 650 tokens over 64 contexts re-import, and about 880 import fresh; 3,000 tokens over
    about 14 contexts re-import (real Figma files: thousands of variables, usually 8 modes or fewer). 3,000 × 64 is
    refused with the budget's message (re-import: 0.36 s, 470 MB). This is less than the 3,000 × 64 first asked for;
    making that import is the QUEUE entry "share a context's tree" (the DTCG Resolver Module's sets and modifiers).
  - Near the limits: 500 tokens × 64 re-import in 0.5 s (76% of `REIMPORT`); 700 import fresh in 0.28 s (79% of
    `IMPORT`).
  - **The CSS importers**, each linear and bounded, their output on 162 stylesheets (the fixtures and real Tailwind
    and shadcn sheets found on the machine) byte-identical before and after:
    - nested rules (`import/mod.rs` `body`): the stylesheet's marks (`{`, `}`, `;` outside strings and parentheses)
      are read once and each `{`'s match found once, and each rule reads its own range of them (each level re-read
      them before, and a custom property's unmatched braces were each matched by a scan to the end). A stray `)` no
      longer lowers the parenthesis depth below zero, so every rule reads its marks as the whole text does (malformed
      CSS only; the 162 sheets are unchanged). A rule nested past 64 is not read, and is reported (one value, with its
      prelude's line), as is a block whose context the blocks would copy past 8 MiB in all, counting the copy in each
      of its declarations' labels and reports; it was a stack overflow at about 100,000 levels. The context is
      borrowed down the rules and copied only into a block kept. A custom property's name past 256 bytes, its value
      past 4 KiB, or a prelude past 4 KiB is reported likewise (one value, its line), so no one string is copied into
      every token, report or message that reads it (no real stylesheet's is near them);
    - nested selectors (`engine::nested`): a selector past 1 KiB makes the chain none, checked before it is built, and
      a list past 1,024 likewise; `& &` at each of 40 levels ran past two minutes;
    - colour aliases (`engine.rs`): each block's chain followed once and remembered, cycles found once (Tarjan's),
      and a failure carried down its chain a step at a time; a chain past 256 links is reported ("an alias chain
      longer than 256", as the DTCG importer's), each block's walk stopping there, so blocks that share a long
      chain cost at most 256 steps each. 20,000 aliases in 80 chains took 28 s in debug, now 1.3 s;
    - shadcn's other kinds: each variable's last declaration and its declarations read once, a reference chain past
      256 none (no name's kind standing in); the other-kind tokens grouped by variable once; each token's sources
      (shadcn and Tailwind) and the themes found by name;
    - the kinds' fold check (`kinds::unfolded`, shared): tokens and the design's found by name through indexes, the
      probe's lines counted as it is written (it recounted the whole text per token), a diagnostic's token found by
      binary search;
    - Tailwind (`written_out`, `other_kinds`): each key written out once at each depth, with its chain's 16-deep
      refusal as before; one value past 64 KiB is refused ("its `var()`s write out past 64 KiB"), and all of them past
      16 MiB; the theme's keys held by name (a reset of one name looks it up; a namespace's clears a range of names,
      passing over the names it keeps, in the order they are held), owners and redefinitions looked up by index, and what reads a redefined `inline` key found once through
      its readers. 5,000 type keys took 38 s in debug, now 0.3 s; 20,000 keys and a 2,000-key `inline` chain 0.4 s.
  - **`marks`**: each token found by one index (its name, then in any ASCII case, the first so named, as before), and
    a dial's moved tokens by a name map. 3,000 set tokens and a dial take 0.14 s in debug (it was not slow there).
  - Tests: `tests/it/scale_css.rs` (each path on a large valid input in time, and the shapes that multiply cut off) and
    `scale.rs` `the_budget_takes_a_design_near_its_limit_and_refuses_one_past_it`.


    factor, which the table doesn't mark, so a `style` reference must end in `-gap`, `-inset`, `-radius`, `-width` or
    `-height`, the endings the kernel's row test maps to length properties. `=grouped-header-weight` stops the build,
    checked by hand.



### Stage 11, commit B2: the hosts' columns (draft for theme2)

Built on B1, under the plan's §3 columns. Each host resolves the base names in its own column of the kernel's table and
draws nothing new: stage 11 C draws and conforms on top.

- **The web's JS target**, which keeps no kernel at run time (`host/web-js/src/base_scale.rs`):
  - Its one look is the neutral one. Every build path says so with the role table's switch, which is the metrics'
    column too, so its column is `metrics::column()` at build time.
  - A static row naming a base name is that column's length: the kernel reads the literal while the sheet is
    projected (B1's fallback).
  - The sheet declares each base name the plan may read as a length, and its design does not define, once on `:root`,
    as `--exact-<name>` (`--exact-space-md:16px`). The values come from the kernel's table, never copied numbers, and
    a plan reading none declares nothing (round 1 below says which reads count).
  - A bound row whose value may name a base name maps it to that property's `var()` (`measures::map`). An SVG
    attribute row, which reads no property, maps it to the length.
  - An alias to a base name the design leaves unset (B1 round 1, `card.pad: length = space.md`) is the base's property
    wherever the token's CSS is written, except in an SVG attribute (round 1 below) (`measures::css`: `var(--exact-space-md)`), and the sheet declares that
    property although only the alias names it. Once the design sets the base name the alias folds (theme2), so the
    page shows the app's value and declares no base property.
  - A design that defines a base name makes it its own token, which the token path writes, and no base property is
    declared for it. theme2 accepted `--exact-<name>` on two conditions, each tested:
    - the app's override wins: its own value is what the page shows, inline in a release build;
    - it follows a live edit: in a development build the row reads the design's property (`--x-space-md`), which an
      edit's batch (`modes::design`) sets to the new value.
  - The web has no iOS or Mac look class to declare a second column under; one would add its rules in the same place.
    `emit.rs` stays at its cap: its one change joins the base sheet to the theme's.
- **The wasm web** reads the kernel's column, which its boot already sets to the neutral look's (`host.rs`). A test
  boots the bridge and finds a base name's neutral length in the batch, one that differs from the web's first look's
  (`@ios`) there (space.xs: 4 against 5).
- **The terminal:** the host says `metrics::set_column(Terminal)` at boot (`host.rs`).
  - Its metrics read as before (the web's first look's), its base spacings as cells (`n ch`, on the grid) and its base
    radii as 0.
  - The terminal check (`contract/cli/src/terminal.rs`) admits a base name as a terminal length, since it is cells in
    that column.
  - A design that redefines a base name in px is not seen by that check, as it does not see any other token's value: a
    known limit, as for every token on the terminal.
- **Linux and Android's reader**, through their own switches (`exact_linux::app::show_neutral_look`, the role table's;
  `show_material_look`, the metrics'): every base name reads that column's cell. Material's cells are Material 3's
  (`m3-spacing`, `m3-shape`). Their values coincide with the neutral look's own steps, which a test tells apart by
  source.
- **Tests:**
  - `host/web-js/src/base_scale_tests.rs`:
    - a static row is the column's length, and the sheet declares only the named bases, once, from the table;
    - a bound row's values map to their properties;
    - with no base named, nothing is declared; a design's own `space.md` is its token, and the others stay base.
    - an app's own `space.md` wins and follows a live edit (40 px to 48 px on the same property), and no base property
      is declared for it.
  - `host/web/tests/base_scale.rs`: a web boot reads the neutral column, told apart from iOS's cell (space.xs 4
    against 5; `Column::Web` reads the neutral cell since B1's round, so the boot's switch is what it holds).
  - `base_scale_tests.rs`: an unset-base alias's CSS is the base property and the sheet declares it, in release and
    development; an app-set base name folds the alias to the app's value with no base property.
  - `host/terminal/src/host_scale_tests.rs`: the terminal's column; space.lg is two cells (16 px inline, 16 px on top)
    and radius.md and radius.full are 0.
  - `contract/cli/src/terminal.rs`: base names pass the terminal check.
  - `host/linux/tests/base_scale.rs`: the desktop's neutral column and the reader's Material one, every base name, and
    a booted row.
- **After review round 1 (Astra: three blocking, one should-fix; Grok: one should-fix, Astra's first):**
  - **An alias in an SVG attribute** (Astra 1, Grok). An attribute reads no CSS property, so a bound attribute row's
    map writes an unset-base alias as the base's length in the build's column (`base_scale::value`), not its `var()`.
    A static attribute already folds through the kernel. Test: `base_scale_tests.rs`, release and development, bound
    (`$tla`'s `card.pad` is 16px) and static (`height` 16).
  - **A live switch to a base the sheet lacks** (Astra 2). `card.pad: space.md` to `space.lg` was a value-only edit,
    batched as `var(--exact-space-lg)`, which no rule declared. The dev design's `structure` (`modes::design`) now
    carries the declared base names too, so the dev loop (`sameShape` in `dev.mjs`) builds instead, and the build
    declares the new one. A plan declaring none keeps the plan's structure as it was. Test:
    `host/web-js/base-scale-live.test.mjs`, in Chrome: the edit's structure moves, the rebuilt sheet declares
    `--exact-space-lg:24px`, and the page's row is 24px.
  - **The plan's length reads, never its string pool** (Astra 3; theme2's (B), no plan format change). The declared
    set (`base_scale::named`) is:
    - each base name a length row's value may be, through `?:`, `match` and `let`, read from its bytecode
      (`returns`, an abstract walk of the VM's stack);
    - each base name a length token aliases;
    - all nine when a length row's value may be any string, as lowering keeps every length token then
      (`computed_measure`): a string-typed read, a string call, or a concatenation whose every part could be inside a
      base name (`"space." + size`). A read is typed by the plan: a state's, a derive's or a resource's type, a
      record's field, an each item by its list's subject and a match's bound value by its option's (through the
      region frames the node sits in); a param stays untyped. A read typed as a number or bool is no base, nor is a
      concatenation with a part no base name holds (`w.toString() + "px"`).
    - only rows whose codec reads a length (`base_scale::reads_length`, asked of the codec with `1px`), for the
      declared set and the maps alike. The kernel's `admits` takes a length token on every row that is no colour's or
      number's, keyword rows too (`color-scheme`, `font-style`, `dynamic-range-limit`), but there the codec refuses the
      length a base name reads, so the base name is refused either way. This departs from `admits` only for such
      values.

    The maps hold the same set, so a computed name finds its property. A text or a label that says `space.md` declares
    nothing. theme2 (B): declaring all nine where a name may be computed is safe, missing one is not. Tests in
    `base_scale_tests.rs`: `"space." + size` and a string state declare all nine and map each; plain text, a label, a
    condition over literals, a number state and a `+ "px"` length declare none; a literal row declares exactly its
    own, and one condition's leaf exactly its own; an alias declares its base (the earlier test). Release output
    compared with d7eaafb72a's exact-web-js (before B2) for 14 apps: all 14 are byte-identical in app.js, app.css,
    paint.js, names.js and pages.json (hello, caltrain, design-tool, messages, realworld, theme-fixture,
    color-gallery, tamber-host, svg-gallery, interaction-gallery, motion-gallery, markdown, typetour, spark). Further
    tests: a number list's item declares none and a string list's all nine, and a keyword row (`font-style`) bound
    to a string state declares none.
  - **A real live edit on a page** (Astra 4). `base-scale-live.test.mjs` builds an app for development with
    `space.md = 40px` and `card.pad: length = space.md`, shows it in Chrome, and compiles the edit to `48px` as the dev
    loop does (`exact-web-js design`). It checks that the structure and shape hold and that both tokens change, sends
    the batch through the page's `exact.design`, and finds the direct row and the alias's row both moved from 40px to
    48px, on the same page.
- **After review round 2 (Astra: four blocking, one should-fix; Grok: two should-fix, overlapping):**
  - **All nine or none** (theme2's yes, 2026-10-19; replaces round 1's per-name set). The sheet declares every base
    name the design does not define once the plan may read any base name as a length, and none when it can't
    (`base_scale::named`). It may read one through a length token that aliases one, a length row whose value may be
    one (its bytecode's constants, trimmed as the page's lookup trims), or a length row whose value may be any
    string: a string read, a string call, or a concatenation not proven base-free. The maps hold the same set. A
    switch between two base names then never names a property the sheet lacks, so the design's `structure` moves only
    between none and nine. This dissolves Astra 2 and 4 and Grok 1 and 2 as filed:
    - **Whitespace** (Astra 2, Grok 1). A concatenation is base-free only where one of its parts, trimmed, is
      non-empty and inside no base name. `"space." + size + " "`, `" " + gap` and `gap + " "` declare the nine.
    - **Scrubs** (Astra 4, Grok 2). `design-serve`'s patch reply now carries the structure:
      `modes::structure_of`, the held plan's own (patching appends to its pool) with the base names the patched
      values may read. `dev.mjs`'s `scrubPatch` keeps it, and `scrubShow` refuses a frame whose structure differs, as
      `sameShape` rebuilds a file edit. A frame that brings the plan its first base name is refused; one between two
      base names is shown.
  - **A keyword row's map is its own** (Astra 1). Where the plan declares the scale, a row the kernel lets name a
    length token but whose codec reads no length (`color-scheme`) gets a `$tk…` map without base names, so a length
    row's `$tl…` map, which holds them, is never the keyword row's cached one. Where the plan declares none the maps
    are alike and keep their names, so a release that uses no base name is unchanged.
  - **A bound SVG attribute and an alias** (Astra 3). A bound attribute's map holds an alias as its length (an
    attribute reads no property), so every token of one value such a row may name, a length or number its row takes
    (an alias as its length, a plain constant as its CSS), is written inline (`measures::inline_tokens`). A
    development build's edit of one builds, and a scrub refuses it, never a batch that leaves the attribute behind.
    (The constant's case is older than B2; the lead had it widened here.) A token that varies is no attribute map's,
    as before. Round 1's literal length
    in the attribute and its test stay.
  - **Built values in region frames** (Astra 5). The walk keeps a constructed option's or list's element (`Some(x)`,
    `[12, 16]`), which a `match` or `for` frame then holds, and a bound row returning an option counts its element.
  - **One reading an emit** (`base_scale::keep`, through `modes::every_for` as `emit.rs` is at its cap). An emit
    walks the plan once, not once a bound row.
  - **Tests:**
    - `base_scale_tests.rs` `the_scale_is_declared_whole_when_the_plan_may_read_a_base_name`:
      - a literal row, and a condition's leaf, declare the nine;
      - five computed and whitespace-wrapped names declare the nine, and the map holds each;
      - plain text, a label, a px condition, a number state, `w.toString() + "px"` and a keyword row bound to a string
        declare none, with no map entry;
      - `for n in [12, 16]`, `match some(w)` and a number list state declare none; their string forms declare the
        nine.
    - `a_keyword_rows_map_is_not_a_length_rows`: `color-scheme` bound first, then padding; `$tl` holds `space.md` and
      `$tk` doesn't.
    - `a_token_a_bound_svg_attribute_may_name_is_written_inline`: `card.pad` and the constant `tile` are both inline.
    - `a_switch_between_base_names_keeps_the_structure`: space.md and space.lg have the same structure; 12px's is
      another.
    - `base-scale-live.test.mjs`, in Chrome:
      - a scrub patch from 12px to `space.md` moves the structure, and one from `space.md` to `space.lg` doesn't;
      - the switch compiles with the same structure and shape, goes as a batch, and the page's row moves from 16px to
        24px with no reload;
      - the 40px to 48px test stands.
    - Release output against d7eaafb72a's exact-web-js: all 14 apps byte-identical.
- **After round 2's re-review (Grok RIGHT; Astra RIGHT WITH CHANGES):**
  - **Trimmed as JavaScript trims** (Astra, blocking). Rust's `trim` keeps U+FEFF (and strips U+0085), while the
    page's lookup is JavaScript's `trim`, which strips U+FEFF. So `"\u{feff}" + gap` was classed base-free although the
    page reads `space.md`. `base_scale::js_trim` trims JavaScript's set (its WhiteSpace: tab, vertical tab, form feed,
    U+FEFF and the space separators; its LineTerminator: LF, CR, U+2028, U+2029), for an alias's value, a returned
    constant and a concatenation's part alike. The test's leading and trailing U+FEFF and its U+3000 on both sides
    declare the nine, and fail under Rust's `trim`, checked by hand.
  - **`none` has no payload** (Astra, should-fix). The walk's `none` is an option whose payload is nothing, which a
    join takes as the other side's, so `match (wide ? some(12) : none)` binding `padding=n` declares none, and its
    string form declares the nine (tests in `the_scale_is_declared_whole_when_the_plan_may_read_a_base_name`).
  - **QUEUE:** "A Stretch cell form in the base scale table" (theme2's optional item), to land with the next commit
    that moves the schema digest.
  - Release output against the commit before B2 (`d7671b5263`'s exact-web-js): all 14 apps byte-identical.
- **After review round 3 (Grok RIGHT; Astra RIGHT WITH CHANGES):**
  - **A bound grid track reads a base name** (Astra, blocking). A bound `grid-template-columns` or `-rows` naming
    `space.md`, directly or through an unset-base alias, maps to `var(--exact-space-md)`. grid.js's track check
    admitted only a design token's `var(--x-…)` and refused it as `unset`, in a release build too. It now admits
    exactly the scale's nine `--exact-<base>` properties, no other `--exact-` one. The search for the same `--x-`-only
    assumption found no other validator:
    - rt.js writes a bound value through the browser, which takes any `var()`;
    - the corner map's check refuses only a list;
    - backdrop.js reads no length token;
    - modes.js, modes.rs, check.js, inspect-dev.js and input-glue.js read a token's property for the live table, the
      checker or the inspector, and never refuse a value.
  - **Tests:**
    - `grid.test.mjs`: the nine and a token's property are tracks; `--exact-accent`, an unknown step and a list
      aren't. grid.js is loaded from its text, so no module mock leaks into the other files run in that process.
    - `base_scale_tests.rs` `the_grid_track_check_names_the_scale`: grid.js's nine are the kernel's table.
    - `base-scale-live.test.mjs`, in Chrome, in a release build and a development one: a bound track naming
      `space.md` computes to 16px, and one naming the alias `card.pad` (space.lg) to 24px. It fails without the fix,
      checked by hand.
    - Release output against the commit before B2: all 14 apps byte-identical.

### Stage 11, commit C2: the scale conformed and held to each host's column (draft for theme2)

Built to theme2's 15:32 approval, on B1 and B2.

- **The conform target `host/web-js/conformance/scale/`** (`// linux: layout`):
  - corners by `radius.sm` to `radius.full`, inline and through a `style`;
  - `radius.full` on a 120×32 box and a 40×96 box: the cross-host pill check, which holds every host's
    corner-overlap clamp of the one oversized length;
  - padding by `space.xs` to `space.xl`, and a row for each gap, `space.xs` to `space.xl`;
  - the design's own `space.md = 12px` (ruling 4) and `space.header = 44px`, a name the scale does not reserve
    (ruling 5), as a box's height;
  - a bound corner that moves between `radius.md` and `radius.full` (`scale.steps`);
  - the probe's colours as tokens fixed in either scheme.
- **conform result.** The wasm page and the JS page agree on all 5 compares: boot, the two scripted taps,
  conform's own extra tap, and `clock +60000`. Linux's boxes agree on all 5 too. Linux compares boxes only, so its
  corners are held by scale-check below.
- **Each host against its own column:** `scripts/scale-check.mjs` takes lengths by name as expectations
  (`scale/expect.json`).
  - A base name is read from `kernel/tables/schema.json`'s `scale` in the host's column (a metric's reference read
    in that column). The design's own token comes from `contract tamber ir`, which finds the design as a build does.
    Nothing is copied.
  - It checks paddings, gaps, the header's height and corners, and the bound corner again after one and two taps,
    each from a fresh launch.
  - On 2026-10-19 all 20 checks passed on each host. Paddings and gaps were within 0.5 pt, corners within 0.2 pt:

    | Host | Paddings and gaps xs/sm/md/lg/xl | Corners sm/md/lg | Pills | Bound, after one tap |
    |---|---|---|---|---|
    | web (wasm) | 4/8/12/24/32 | 4/8/12 | 16 and 20 | 8, 20 |
    | Linux | 4/8/12/24/32 | 4.1/8.1/12.2 | 16.1 and 20 | 8.1, 20 |
    | macOS 27 | 6/8/12/20/30 | 5.1/6.1/10.2 | 16 and 20.1 | 6.1, 20 |
    | iOS 27 simulator | 5/8/12/20/35 | 5.6/17/26 | 16 and 20 | 17, 20 |
    | the Android reader (x2-android emulator) | 4/8/12/24/32 | 4/8/12 | 16 and 20 | 8, 20 |

    md is the design's 12 on every host, and the header is 44.
  - The neutral and Material columns have the same px for every name. The Material column is held by the Android
    reader's run (theme2's ruling: the scale's own boxes, not E's fixture):
    - the reader is tested by a plan push, not by an app build: scale-check gives the reader the compiled plan,
      pushed in place of the one an app's APK bakes (`--app unsaid`, an app that asks nothing of its data), so the
      reader runs exactly what the other hosts run (theme2's 16:55 ruling);
    - before measuring, every host must name the compiled plan in its `perf` (the SHA-256 of the plan's bytes), so a
      stale or wrong plan, such as the reader's own bake if the push were not read, fails loudly;
    - the reader's screenshot is the whole screen, so the page's top is found in it, to the fraction of a pixel,
      from the topmost box's fill;
    - a page offset or narrowed across the screen (side bars, a cutout, on either side) is refused rather than
      misread: the topmost box's left and right edges are held to where the page's pixels per point put them.
- **The terminal** (theme2's 17:39 ruling): `exact-terminal` gains two headless verbs, the terminal carrier's own
  (not the agent protocol):
  - `layout`: one JSON line with the viewport, the cell (8×16 px), and each node's kernel frame in px, its cells as the
    painter rounds them, its `testId` and its corners' radii, sized against the box as a host sizes them (a
    percentage across and down, a pair where the two differ);
  - `plan`: the runner's digest of the running plan.

  `scale-check --host terminal` drives it on the compiled plan:
  - it reads the table's terminal cells by the reply's cell width (a `ch` is 8 px);
  - it checks paddings, gaps and the header in px, and every corner at 0 from the reply (radius.full included);
  - it checks the bound corner after taps, from a fresh run;
  - it writes every box's cells, and each padded box's padding in cells (top/right/bottom/left), into its output
    for E's gate;
  - a step's text stays one argument (`type <id> <text…>`).

  On 2026-10-19 it first passed 20 of 21. The one failure was the assertion theme2 asked for: a box padded `space.sm`
  whose child fills it painted 1 row above and 0 below, since `1ch` down is half a row and the painter rounded each
  edge's position.
- **The terminal's spaces in whole cells** (theme2's 19:09 ruling, option (2), superseding the painter half of 18:35):
  the column stays in `ch`, and the kernel lays a terminal's spaces out in whole cells. Under the terminal's space rule
  (`Env::cell_spaces`, set only by the terminal host beside `cell_borders`; LLP 1101 §4's row), a padding, margin or
  gap that resolves to points (a `calc()` that resolves to points, or has no percentage, included; a multi-column
  box's `column-gap` too) is rounded before layout to whole cells
  on its axis, rows down and columns across, to the nearest with a tie toward zero (`cells::space`). A percentage,
  or a `calc()` with one, is not rounded: the rule's limit.
  - The numbers: down, space.xs, sm and md (0, 8 and 8 px) are 0 rows and space.lg and xl (16 px) 1 row; the clone's
    14, 12 and 10 px are 1 row each; across, a `ch` is a column (12 px, a column and a half, is one).
  - px and cells agree by construction, so the painter, scroll, `scrollIntoView`, follow-end, the collections' windows
    and the inline transcript need no change. The readers of a space outside layout follow the rule too: a
    virtualized list's keyed jump aligns a row by its root's margins, now read from the kernel as laid out
    (`Kernel::laid_margins`), and its `scroll-padding` and `scroll-margin` in whole cells when the rule is on
    (`runner/src/runner/collection.rs`); a host's cover inset adds to the rounded padding. Turning a cell rule (spaces or borders) on or off after a layout restyles every box. A painter-only rounding was built first (x2/s11c2-painter-14f0953eea,
    not landed); Astra's round 1 found it drifted from px cumulatively (item k of a gap-sm list at px 24k, painted on
    row k).
  - The visible change, accepted: a box padded `space.sm` shrinks to its content (the `even` box is its child's two
    rows, 0 rows of padding each side), and a gap-sm list has no blank row between items.
  - scale-check: the terminal's paddings and gaps are expected in whole cells on each axis, in px and in cells (a
    reply without whole cells for a padded box or a gap's children fails), and the `even` box paints equal rows above
    and below. Every other host is unchanged (the rule is off): conform scale 5/5, Linux 5/5.
  - Tests: `kernel/src/style/cells.rs` (`space`, and the engine's style with and without the rule),
    `host_scale_tests.rs` `spaces_are_whole_cells_so_rows_follow_the_layout` (item k of a gap-sm list on row k,
    where its px frame is; revealing item 30 in a ten-row scroller shows items 21 to 30), `collections.rs`
    `a_keyed_jump_aligns_rows_by_their_margins_in_whole_cells`, `kernel/tests/it/env.rs`
    `the_terminal_space_rule_rounds_resolved_spaces_and_relays_when_it_turns`, `layout_and_plan_answer_as_json`
    (`1ch` of padding: a column across, none down) and `scale-check.test.mjs`.
  - The run: terminal 21/21 (even 0/1/0/1 cells top/right/bottom/left; sm padding 8, 0 px; md, the design's own 12
    px, 8, 16; lg and xl 16,
    16; gaps 0, 8, 8, 16, 16 px across; the header 44; corners 0).
- The other hosts run the same plan, now 21 checks with the `even` box: 21/21 each on web, Linux, macOS, iOS and the
  reader. conform: 5/5, and Linux 5/5.

### Stage 11's gate: met (2026-10-20; the run is `~/x2theme/stage11-gate-run.txt`)

The run on `558b0bcf8e` (E on main), every host fresh: web, iOS, the Mac, Linux, Android's reader and the terminal.
Stage 11's parts, all on main: A1 package designs in the compiler (`d5b4507d63` and records), A2 part 1 (the server
and `contract tamber` fold packages) and part 2 (package designs on the wire, `d98591ee32`, *1.14*); B1 the scale
table (through `d7671b5263`) and B2 the hosts' columns (through `d750c4e996`); C2 conform and scale-check, the
Android reader by a plan push, the terminal host, and (c) the terminal's spaces in whole cells laid out by the kernel
(`8ddfce36be`); D's server (`ed6e1720fa` and before) and tool half (`e8cd42223a`..`7a9da404a4`); E the fixtures and
the gate script.

| Gate line | Result |
|---|---|
| Two packages restyle together from one app value | **Met**: `@exact/cards` and `@exact/notes` both move with the app's `space.md = 20px` on web, iOS, the Mac, Linux and the terminal; live on the web, `set-token space.md` moves both with no reload and `remove-token` returns them. |
| With no app value, `space.*` and `radius.*` are each host's | **Met** on every host, both packages included; every scale row cites a metric reference, a measured id or a guideline, and none is a bare number. |
| Package designs: set by full name; one name in two packages refused unless the app sets it; no dials, axes or base names in a package; a `when` only on the app's axes | **Met**, each by its test. |
| The server and the tool | **Met**: base rows with their columns and the app's override; package tokens with `package` and `file`; `set-token` of a base or package name writes the app's line, never a package file; `add-token` of a base name points to `set-token`; the design tool's real-server test. |
| No regressions | **Met**: the clone's own `space.header` keeps its meaning (on a signed-out copy); conform `--synthetic --strict --linux` 0 failures in 110 targets (two older Linux reds fixed first: metronome by LLP 1125 D12, decorations by the grammar's `unavailable`); release output over 504 comparisons: 446 identical, 45 only B1's schema digest, 2 the design tool's own source, 11 failing alike on both sides. |

**Decided in stage 11 and recorded here:** no web column (the web reads the neutral one); an app may set a base name
and a package may not; the terminal rounds padding, margin and gap to whole cells in the kernel
(`Env::cell_spaces`, LLP 1101 §4); a package file is what D11, D15 and the resolver see, judged on real paths; one
write op for overrides (`set-token`). **Left to QUEUE:** expressions over base names (stage 12 B2), the Stretch cell
form, incremental retention after a name-moving edit, a Linux InlineGeometry reader.

**Stage 11 is closed.** The `tamber/1.14` window is closed; the next wire change takes `tamber/1.15` (stage 12).

### Stage 12, commit B1: package components' params (draft for theme2)

Stage 12 §B's first commit, under no wire change (the server's existing fields carry it; the three levels in
`explain`/`resolve` are B2, under tamber/1.15).

- **What was there** (verified first): a package's Contract component's params already worked end to end. Its
  identity is its file under its package (`sources.rs` `home`: `@acme/reading/index.contract#Card`), `describe`
  lists them under `components`, `set-param`/`unset-param` by full or short name write the app's design (the server
  writes only the app's own files), the build keeps a set one, and `uses` lists its sites in the package's file.
  Only tests were missing.
- **A package design's components** (D15; QUEUE "A package's `.tamber` components", closed). A package's design
  may declare `component Name` with `params`, for a component that isn't Contract, as the app's design may. It is
  no longer `tamber-package-block`. Its identity is its design's home and its name
  (`@acme/reading/reading.tamber#Map`, `params::design_tokens`), so two packages' components of one name stay
  apart: their token names differ by file, and a short name naming both is `tamber-param-ambiguous`. The three
  places that read the app design's components read the packages' designs too: `params::tokens` (what `describe`,
  the server and the build list), the build's retention roots (D7: such a component's params are kept, with what
  their defaults name) and the loader's checks (`tamber-param-unknown`, `syntax-param-attribute`,
  `tamber-param-collision`), which say a package's refusal at its line in the package's design.
- **A package's `params` line stays refused** (`tamber-package-block`): a param's value is the app's to set, in the
  app's design. The message says so.
- **Tests** (`contract/cli/tests/it/tamber_package_params.rs`, through the server):
  - `describe` lists `@acme/reading/index.contract#Card` (Contract) and `@acme/reading/reading.tamber#Map` (design)
    with their params, full and short names, and the design param's default naming a package token;
  - `set-param` by full name (`Card.inset`) and short name (`Map.pin`) writes the app's `params` block alone, the
    package's files byte for byte unchanged; `resolve` and the build read both values (the plan keeps them);
  - `uses` lists `Card.inset`'s sites in the package's `index.contract`;
  - `unset-param` by short and full name removes both lines;
  - a package design's `params` line is refused, and a package param's `param.nothing` is said at
    `reading.tamber:9`;
  - two packages each declaring `component Map`: both listed, the short name ambiguous, the full name set.
  - `package_designs.rs`: the block test no longer lists components, and a package design with `component …
    params` builds.
- **Next (B2, tamber/1.15):** a param's three levels in `explain`/`resolve` (the package or base token its default
  reaches with stage 11 D's `bases`, the app's override of that token, the param's own app value), with A2 part 2's
  `package`/`file` fields once they land. Also: `uses` names a package file by its absolute path, where `explain`
  names it relative to the app.
- **After review round 1 (Astra and Grok, RIGHT WITH CHANGES):**
  - **A short name only where `set-param` takes it** (Astra 1 = Grok 1). `describe`'s `short` read the Contract
    components' identities alone, so two packages' `Map`, or the app's `Map` and a package's, each said
    `short: "Map.pin"`, which `set-param` refused as ambiguous. It now reads every param's identity too, as
    `resolve` does (`tamber_params::short`). Tests: both `Map.pin`s, across two packages and across the app and a
    package, have no `short`; `Card.inset` keeps its own.
  - **The server's retention key sees a package's components** (Astra 2). Retention now has a package design's
    component params as roots, but the server's cached build of it was keyed by the walk, which leaves design files
    out. A param a package came to declare after the server primed was no root, and an override of it varying by an
    axis said `live`. The key now holds each package design's components (`Server::synthetic`). Test: prime, add
    `size` to the package's `Map`, then set it under `when mode=b`: `rebuild`, as a fresh server says. It said
    `live` without the fix, checked by hand.
  - **A package param's default names its package's own** (Grok 2). The package reference check (`references`)
    read only the package band's tokens, so a param's default could name another package's token, an app token or
    a dial. A package design's param is now held to its package's rule: its own tokens, roles, base names and its
    component's other params (`param.pin`), and no dial. A refusal is `tamber-unknown-token` at the default, in the
    package's file. A param the app sets is the app's value and is not held to it. Tests: `maps.pin` and `ink` are
    refused at `reading.tamber:9`; a sibling param is admitted; the app's `Map.pin = ink` builds.
- **After review round 2 (Astra and Grok, RIGHT WITH CHANGES):**
  - **The retention key holds each package design whole** (Astra 1). Round 1's key held a package design's
    components, not its tokens, which a param's default reads through. The key now holds each package design's
    digest (`Server::synthetic`). Test: `a_package_tokens_edit_moves_what_its_params_keep` primes on
    `Map.pin → reading.pin → reading.old` (an override of `reading.old` over an axis is `rebuild`), moves
    `reading.pin` to `reading.new`, and holds both overrides' classes to a fresh server's. With round 1's key the
    primed server kept the old retention and classed `reading.old`'s override `rebuild` where a fresh server says
    `live`; the package's digest is what makes them match (corrected in round 3: an earlier check ran before the
    test primed on `reading.old`).
  - **A written whole alias is judged as written** (Astra 2 = Grok 1, 2). A profile colour folds as `transparent`,
    naming none, and an icon param lies on `icons`, so `pin: color = ink` (the app's `color(--brand …)`) and
    `glyph: icon = icon.app` (the app's icon) passed. The default as written is now held to the package's rule as
    the theme tokens' are: a whole token or icon name that isn't the package's, a role, a base name or a sibling param
    is `tamber-unknown-token` at the default in the package's file. A literal `icon(sf "…")` names nothing. Tests:
    both are refused at `reading.tamber:9`, and `glyph: icon = icon(sf "heart")` builds.
  - **The app sets a package param only by a name only it answers** (Grok 3). Whether the app's `params` line sets
    a param is decided with `resolve` over every param and the Contract components (`package_references` takes them
    now), so an ambiguous short name sets none, and the default's refusal is said beside `tamber-param-ambiguous`.
    Test: two packages' `Map`, the app's `Map.pin = ink`, and reading's default `ink`: `tamber-unknown-token` is
    among the build's refusals.
- **After review round 3 (Astra and Grok, RIGHT WITH CHANGES):**
  - **A package param's SVG is a source of the app** (Astra). The dependency scan (`source_graph`) folded only
    Contract params, so `glyph: icon = icon(svg "pin.svg")` in a package's design read `assets/pin.svg` without its
    being a source: no watcher rebuilt on it, and one made later was not looked for. The scan now takes each package
    design's params too, and so runs when the app has no design file. Test: `a_package_icon_params_svg_is_a_source`,
    with no `app.tamber`: `pin.svg`, there, is a source; `later.svg`, not there yet, is looked for (`consulted`).
  - **The app's design whole keys the server's retention** (Grok 1). The key held the app design's token names and
    components, not their values or its `params` block, so an app override of a package token moving what a param's
    default reaches (`reading.pin = reading.old` to `reading.new`) left the old retention. It now holds the app
    design's digest (`Server::synthetic`), as it holds each package design's. Each edit of the design builds the
    retention again, a compile, where a value edit used to reuse it. A commit needs no reset: its text moves the key.
    Test: `the_apps_design_whole_keys_what_its_params_keep`: prime on `reading.old`, move the app's override, then
    set `Map.pin = ink2`. The overrides of `reading.old` and `ink2` over an axis are classed as a fresh server
    classes them (`ink2`'s `rebuild`). With the names-and-components key it fails, checked by hand.
  - **§10's round-2 note corrected** (Grok 2): the package's digest is what makes round 2's classes match.
- **theme2's sign-off condition: the server's retention, timed on the clone, keyed narrower.** Round 3 keyed the
  server's cached retention (`Server::synthetic`) by each design's digest, so any design edit rebuilt it, a whole
  compile of the app. On the clone that added seconds to the next `propose`. The key is now what can move retention
  or the synthetic tokens (`tamber_walk::retention_key`), parsed, never compiled: for the app's design and each
  package's, each token's and icon's name, written kind, a value's rough kind (a colour, a length, a number, other),
  the names its value and `when` values reference, and its `when` conditions; the `params` entries, name to value;
  each component's params with their defaults; and the axes, dials and `exports`. A value that names nothing (a
  hex, a length) is left out, so editing it keeps the cache. A design that does not parse is keyed whole.
  - **Test:** `a_value_edit_naming_nothing_keeps_the_cached_retention` counts builds by the server's hook
    (`EXACT_TAMBER_RETENTION_LOG`, one line a build), not by time. A primed server keeps its one build through
    three value edits (a hex, a length, an alias target's colour). An alias moved, a param set to a token, and a
    colour turned into a length each build again. Round 3's alias-move and set-param tests still pass.
  - **Timed:** `/tmp/bsky-bench.py` on `/tmp/bsky-retention`, a copy of the clone's sources (the clone itself
    untouched). It uses debug builds, as the dev loop runs `cargo run -p contract`. A server is primed with a
    `propose`; then five times: an edit written to `app.tamber`, the next `propose` timed (after edit), and one
    more with no edit (warm). The value edit flips `contrast_25`'s light hex; the alias edit makes `white_50` name
    `black_20` and back. Medians of 5, in ms:

    | binary | edit | prime (cold) | after edit | warm |
    |---|---|---|---|---|
    | old key (token names and components; the stage-11 path fix's build) | value | 2621 | 220 | 6 |
    | old key | alias | 2528 | 222 (no rebuild: the stale case round 3 fixed) | 6 |
    | round 3's key (each design's digest) | value | 2607 | 3433 | 8 |
    | round 3's key | alias | 2717 | 2493 | 7 |
    | this key | value | 3107 | 238 | 8 |
    | this key | alias | 2507 | 2519 | 7 |

    Two more value runs, the old key and this key back to back: 223 and 224, then 224 and 221. A value edit's
    `propose` is the old key's (within 20 ms; the limit was 50). An edit that can move retention rebuilds it, a
    compile, as round 3's key did, in the same time. The old key took 222 ms there only because it did not rebuild
    and kept a stale retention. (An earlier run, under heavier load, had the old key at 469 ms after a value edit and
    round 3's at 5639 and 7695.)
  - **After the key's re-check (Astra, blocking):**
    - **Names compared as the checker resolves them.** A role's spelling is matched in any case and with or without
      `-exact-` (`CANVAS` is `Canvas`), so the key folded only exact matches: `pick = CANVAS` moved to
      `pick = CANVASTEXT` kept it while the retained chain moved. Names and references are now folded (lowercased,
      `-exact-` dropped) before they are compared and keyed. Test: `a_role_alias_moved_by_its_case_is_followed`: a
      param set to `pick` with `Canvas = old` and `CanvasText = fresh`. Moving `pick` by case builds again, and the
      primed server's answers (a value edit of `old`, of `fresh`) are a fresh server's.
    - **Each row says its section.** A token's row and an icon's were alike (`icon.brand None other []`), so a name
      moved from `icons` to `theme` kept the key though it changes what the views may read. Rows now open with
      `token` or `icon`. In the same sweep, a token's metadata keys the names it references (`pair=` another token).
      Params, components, axes, dials and `exports` already have rows of their own, and colour profiles are the
      Contract sources' (in the walk's key). Test: `a_name_moved_between_sections_or_a_pair_moved_builds_again`:
      `pair=paper` to `pair=ground` builds again, and so does `icon.brand = icon(sf "star")` moved verbatim from
      `icons` to `theme`. Both tests fail without their fixes, checked by hand.

  - After the key's second re-check (Astra): a reference is found by its folded form but keyed as written, so two tokens differing only in case (`Foo`, `foo`) stay two; a chain moved from one to the other builds again (`two_tokens_of_one_name_in_two_cases_stay_two`, failing without the fix)
  - After the key's fourth re-check (Astra; theme2's pre-approved fallback): the key holds a value VERBATIM whenever it names anything (a name of the designs, as folded, or any dotted word, a package token or a component's generated `param.…` name the designs don't declare); only a literal that names nothing (a hex, a length, `light-dark(#fff, #000)`, `rgba(…)`) is kept by its rough kind, so its edit is a cache hit. Each design's header name is keyed too (a package header repaired joins its tokens). So a non-literal value edit now recomputes retention, as an alias move does. Re-measured on the clone copy (/tmp/bsky-bench.py, debug, median of 5, ms; old key vs this key): a literal edit 370 vs 364 (warm 10 vs 12, a cache hit); an alias move 399 (the old key never rebuilt) vs 3354 (a whole-app compile, as round 3's key). A unit test in tamber_walk.rs holds the key's rules.
  - After the key's fifth re-check (Astra): every string the key holds (names, values, metadata, params, component params) is written quoted, so a metadata value decoding to a newline can't spell another token's row (a unit test, failing without the quoting)..
### Stage 12, commit B2-levels: a param's three levels (draft for theme2)

Stage 12 §B's levels, under `tamber/1.15` (stage 12 D1 opened it; B2-levels adds a clause to its one summary line).

- **One walker** (`contract::tamber_levels`, library). Per tuple it starts at the app's own value of a param (its
  `params` line, by its full name or a short name only it answers, with that line's `when` for the tuple), else the
  param's default. It then follows each bare reference, in order: another param (its app value or its default), the
  app's token (a role's spelling in any case), then a package's token, each line's value in the tuple. It goes at
  most 8 hops (`HOPS`) and stops at a line whose value is no reference (a literal or an expression), at a base name or
  at a role. It answers `Step {level, name}`, the level that wins and the name there: `app-param`, `app-token`,
  `package`, `base`, `role`, or `default`. `default` is not in the plan's list: it is the param's own default being a
  literal or an expression, the one case none of the five names (theme2: it stays; a tool's word for it is "the
  component's default"). rt1's stage 12 D3 reads the same walker; its
  signature was sent to rt1.
- **`resolve` of a param**: each tuple adds `level` and `name`, from the walker.
- **`explain` and `resolve` of a param**: `levels: {token, app, base, param}` when the default is a reference;
  `levels: null`, explicitly, for a literal or an expression (theme2's ruling; round 1).
  - `token` is the first token the default names, `{name, package?, file, line, value}`: a package's own line with
    stage 11 A2's `package` and `file` exactly, else the app's.
  - `app` is the app's override of that package token, `{file, line, value}`, or null.
  - `base` is the base name every tuple reaches, as stage 11 D's `bases` names it, or null when the tuples' walks
    differ or one reaches none (theme2's ruling; each tuple's own is on `resolve`'s tuples).
  - `param` is the app's own value, `{file, line, value}`, or null; `param.set` stays.
- **`describe`**: a param whose default is a bare reference adds `reaches: {token, base, app_set}`, compact.
- **Tests** (`contract/cli/tests/it/tamber_levels.rs`, through the server, with `@acme/reading`'s Contract `Card`
  and its design's `Map`):
  - each level wins in turn on one param: the package's line, the app's override (`levels.app`), the app's value
    (`levels.param`, and `param.set` kept), and an app value naming an app token;
  - a chain of package tokens;
  - a base name (`Card.inset` through `reading.gap = space.md`), a role (`AccentColor`), and an expression
    (`levels: null`, its tuples `default`);
  - `describe`'s `reaches`;
  - levels differing by tuple (`reading.gap` 12px, `when mode=b: space.lg`): `app-token` then `base`, and
    `levels.base` null; then a `when` sending one tuple to another base (`space.md`, `when mode=b: space.lg`):
    each tuple's base, and `levels.base` null; with no `when`, `space.md` in both and `levels.base` `space.md`;
  - a chain of ten app tokens stopping after eight (`c7`).

**After review round 1 (Astra RIGHT WITH CHANGES, Grok RIGHT WITH CHANGES):**
- **Roles as the checker keys them** (Astra 1 = Grok 1). A role's override is found in either spelling and any case,
  as `tamber/src/check.rs` `role_key` reads it: `Secondary-Label = …` or `-exact-Secondary-Label = …` overrides
  `-exact-secondary-label`, and `accentcolor = …` overrides `AccentColor`. An app value written `secondary-label`
  reaches the override too. A role or its override is named by the row spelling the fold publishes. Only a row
  spelling stops the walk as a role, as the fold names a role.
- **Icon params** (Astra 3). `resolve` and `explain` of an icon param returned before the additions; both return
  paths now carry them (`Designs::annotate`, one call for every param). The walker reads the app's icon tokens. A
  package's icons are not read yet (`tamber-package-block`), so a package icon param's default is a literal.
- **`levels.token` walks** (Astra 4 = Grok 2). A default naming another param (`edge: color = param.pin` in a
  `.tamber`, `edge: length = "param.inset"` in Contract) follows params' defaults, never the app's `params` lines, at
  most 8 hops, to the first app or package token (`reading.pin`, `reading.gap`).
- **References, not literals** (Astra 5). A reference is a bare name that is a param, an app token or icon, a package
  token, a base name or a role's row spelling. `transparent` and `-1` are literals. A literal or an expression
  default answers `levels: null`, explicitly. describe's `reaches` stays omitted there.
- **`levels.base`** (Astra 2): every tuple's walk agreeing (folded in from theme2's ruling above).
- **Tests** (Grok 3): `reaches.app_set` true under an override; a chain's `levels.token` is its first token
  (`reading.chain`); an app-set `space.md = 10px` wins as `app-token`. Two new tests:
  `a_roles_override_in_either_spelling_and_an_app_set_base_name` (both spellings, mixed case, the fold's value
  agreeing) and `icon_params_param_aliases_and_literals` (a literal package icon param with `levels: null`, an app
  component's icon param naming the app's icon, the app setting the package's icon param through its icon and as
  written, both aliases, `transparent` and `-1`). Each fails on the commit before.

**After review round 2 (Astra RIGHT WITH CHANGES, Grok RIGHT WITH CHANGES):**
- **`levels.token` keeps the walk's cap** (Astra 1 = Grok 1). `levels` took the default's reference and then followed
  8 more params, one hop past the walk. The default's reference is now the first hop, so `levels.token` and
  `reaches.token` name a line the walk can reach: at most 7 params passed, the token at hop 8.
- **A light/dark pair stops on its line** (Grok 2). A `when` naming an axis `under scheme=dark` makes a colour's pair
  (the checker's `under_value`: light from the line without that axis, dark from the `when`). The walk followed the
  dark side alone. It now follows neither: the line owning the pair wins, so `level` and `name` match the tuple's
  light/dark value.
- **Tests:**
  - `levels_token_keeps_the_walks_eight_hops`: nine `.tamber` params, each naming the next and the last
    `reading.pin` (eight aliases): the tuple's level is `default` on the last param, `levels.token` and
    `reaches.token` null. Eight reach `reading.pin` at hop 8, and both say it.
  - `a_light_dark_pair_stops_on_its_line`: `reading.chain = #ffffff` with `when dark-style=dim: reading.pin`: both
    tuples `app-token` `reading.chain`, the dim tuple light `#ffffff`, dark `#333333`.
  - `two_axes_overlapping_whens_on_a_package_token_and_an_app_param` (Astra 2): axes `mode` and `size`. A package
    token has `when mode=b`, `when size=l` and `when mode=b size=l`; then the app's `params` line has its own
    `when size=l` and `when mode=a size=l`. Every tuple's modes, level, name and value are asserted.
  - Each fails with its fix reverted (checked by hand: `0..HOPS`; the pair arm removed; the first holding `when` in
    place of the most specific).
- **§10's lead sentence** (Grok 3) says `levels: null` for a literal or an expression, as the wire does.
- **After review round 3 (Astra RIGHT, Grok RIGHT):** two NITs taken: the test summary says `levels: null` for an expression, and the stage-4 `uses` comment in contract/cli/src/lib.rs sits on `tamber_uses` again.
- **Follow-up: a level per side** (theme2's 19:30 ruling, option (a), from dtool2's T1 reviewers' counterexample). A
  tuple's value carries a light and a dark side, but its level was one: with every `when` on an axis `under
  scheme=dark`, the pair arm made the owning line win both sides, though Light follows the line's own value. Each
  tuple's `level` and `name` are now `{light, dark}`, each side walked on its own as the fold picks it: dark the most specific holding `when` of all, light the
  most specific holding `when` naming no axis under `scheme=dark`, either the line's own value when none holds (the
  pair arm above is gone).
  `levels.base` is the base every tuple reaches on both sides. It changes the type of a field added in this open 1.15
  window, which no released client reads: a correction within the window, said on the 1.15 line. rt1's D3 and
  dtool2's T1 read the per-side shape.
  - Tests: `a_params_line_with_dark_only_whens_wins_dark_and_follows_its_reference_light` (the counterexample: an
    app `params` line `Map.pin = reading.chain` with dark-only `when` lines: light `package` `reading.pin`, dark
    `app-param` in both tuples; red with the light side taking dark's `when`s); `a_light_dark_pair_is_walked_per_side`
    (was `…stops_on_its_line`: the dim tuple's dark side now `package` `reading.pin`); the other tests assert both
    sides alike where no axis is under `scheme=dark`.
  - After round 1 (Astra RIGHT WITH CHANGES, Grok RIGHT): `mixed_axis_whens_are_walked_per_side` (Astra 1, Grok 1):
    `when` lines mixing an axis under `scheme=dark` with a normal one on an app `params` line, and a two-axis normal
    `when` outranking one under dark on a package token, each side's level, name and value asserted (red with
    `.all(under)` for `.any(under)`); the docs say the light side takes the most specific `when` naming no axis under
    dark, not the line's own value alone (Astra 2).

### Stage 12, commit B3: shadows over base names, per host column (draft for theme2)

theme2's 15:30 ruling on the plan (`~/x2theme/x2p4-s12b3-plan.md`; QUEUE's two items, the lead's 15:20Z). D11's
2026-10-20 amendment reaches a shadow token's lengths. The schema, SCHEMA_DIGEST, FORMAT_DIGEST, `BoxShadows`, its
codec and the wire don't move: the theme row's value is the shadow's text, its lengths written as B2's expressions.

- **Tamber.** A shadow's length is a `ShadowLength`: px, or a B2 `Residual` over base names the design leaves
  unset. A bare unset base name (read through `Names::base` first, so the "any other value" refusal no longer fires
  in a shadow), a token aliasing one, a `Length::Base` token (inlined, its bases read), and a length's `calc()`,
  `min()`, `max()` or `clamp()` over one (B2's own fold, so `calc(space.md)` is `space.md`) are kept. B2's refusals
  hold as they are. `Shadows::css()` writes each residual's text: `0px calc(space.md * 2) 4px #000000ff`. The
  token's `bases` gain every base its shadows read. A literal blur is still refused when negative; one over base
  names is checked per column by the build. A base name the design sets is its token and folds to px, as before.
- **Kernel.** `theme::measure_at` folds a shadow entry whose text names a base: `scale::fold_lengths` splits it at
  white space and commas outside parentheses, folds each item naming a base (each base read as an alias reads it,
  the design's own first, the read recorded, under the row's own rule for a varying token) with the kernel's
  `calc()` to px in this process's column, and keeps every other item and separator byte for byte. The unchanged
  `BoxShadows`/`TextShadow` codec reads the result, so Apple, Linux, Android, the terminal (cells as px) and the
  wasm web get the column's px by one path. The runner's `set_tokens` keeps what a re-set row read beside what it
  read before (`ThemeIndex::also_read`), so a later edit of a base a token's new value reads sets the row again.
- **Lowering.** A shadow token is checked in every column (`scale::refusal`): its lengths must fold, and the row's
  codec must take the folded text (a blur not negative, a length within a shadow's limits). The first column that
  fails is refused, `tamber-base-expression`, naming the column and the expression (`card.shadow` is `0px 1px
  calc(space.lg - space.xs * 3.5) #000000ff`, the shadow `0px 1px -1px #000000ff` on mac, which no host draws: a
  blur radius is never negative). A row naming the token checks each column's text (`scale::expanded`), as a
  length's px.
- **JS target.** The token's CSS is the row's CSS for the build's column (colours as css.rs writes them, every
  length said) with each length written over a base its property's `var()` or B2's `calc()` over them
  (`base_scale::shadow`); a shadow over a base name declares the scale's nine properties. A release's static row is
  the column's px, as the kernel folds it.
- **Outside Exact.** The DTCG export folds each length with the base's px (the neutral column), the shadow's text in
  `$extensions.exact.lossy`; `tamber gen`'s CSS writes the neutral column's px (`Shadows::folded`).
- **The literal's word (QUEUE's item).** A refused shadow, border or gradient literal is looked at again: a word
  with a `.` that names no token of the design and no role is `tamber-unknown-token` (`` `box-shadow="0px 0px 8px
  eu.nope"`: `eu.nope` is no token of this app's design and no role``); in a shadow, a length token or a base name
  is `tamber-kind`, pointing to a shadow token (`` a shadow row's lengths are px: name a shadow token (`x:
  box-shadow = 0 space.md 4px black`) to read `space.md` ``). A literal shadow stays px (theme2's Q1: QUEUE). The
  kernel's own message is unchanged.
- **Wire.** One clause on the `tamber/1.15` line (theme2's Q2): a shadow token's `bases` list the base names its
  lengths read. Per-column folded shadow text in `explain` and `resolve` is QUEUE's, beside D1's per-column values.
- **Tests:** `contract/cli/tests/it/tamber_base_shadow.rs` (each kept form and its text, folded in the neutral, mac
  and iOS columns; `bases`; a base the design sets; B2's refusals in a shadow; a negative blur on mac alone and an
  unfoldable length, each naming its column; the literal's words; `uses` of an inlined length token; the DTCG
  export); `tamber_shadows.rs` (a live `set-token space.md` re-setting a shadow row over it; the unknown colour now
  `tamber-unknown-token`); `tamber_serve_base.rs` (`resolve`'s `lengths` as written, `explain`'s `bases`);
  `tests/base_expression.rs` (a shadow row in each of the five columns, in its own
  process); kernel `style::scale` (`fold_lengths`; `measure` of a shadow entry, the design's base first, every read
  recorded, a varying base refused); tamber `shadow` (text, `bases`, `folded`); web-js `base_scale_tests` (the
  `var()` text, a release row, a development build's property); host/web `tests/base_scale.rs` (a wasm boot's
  `box-shadow` in the neutral column, through the kernel).

**After review round 1 (Astra, Grok; theme2's rulings T1–T3):**
- **A length's function in a shadow folds as a length's** (Astra 1, blocking). `calc(space.md * 2)` in a shadow
  compiled while `space.md` was unset and was refused, read as a colour, once the design set it. Every `calc()`,
  `min()`, `max()` and `clamp()` in a shadow now folds by B2's length fold whatever it names: px, a px length token
  (`calc(gap * 2)`) or a base the design sets give px, an unset base a residual. Main read such a function over px or
  a length token as a colour and refused it; it is accepted now, by the one path.
- **`length_texts`** (theme2's T1). `resolve`'s shadow `lengths` keep their type: a length over base names is null
  there, and a parallel `length_texts` (only when one is) gives every length's text, px ones too (`["0px",
  "space.md", "calc(space.sm * 2)"]`). The 1.15 clause names `bases` and `length_texts`.
- **A state look keeps what it newly read** (Grok 1). `looks::op` returns the tokens a look read as resolved again,
  and `set_tokens` adds them to the look's (`ThemeIndex::looks_read`), as a row's: a `:hover` shadow re-set to read
  `space.md` follows the next edit of `space.md`. The bits are added, never replaced (Grok 3's nit, left so and said
  in `also_read`'s comment): a stale bit costs one re-set that changes nothing.
- **A base name or length token in a border or gradient literal** (Grok 2) is no unknown token: the row's own refusal
  stands (`lower-css-shorthand`, `lower-attr-value`). Only a dotted identifier is looked up as a token (Astra 4): a
  letter first, then letters, digits, `.`, `_` and `-`; a number, a dimension (`-1.5px`) or a function
  (`rgb(0.5 0.25)`) keeps the row's width or colour refusal.
- **A colour function is kept whole by `fold_lengths`** (Grok 4): `rgb()`, `hsl()`, `lab()`, `oklch()`, `color()`,
  `color-mix()`, `light-dark()` and their kin are never folded as lengths, a base name inside one left for the codec
  to refuse.
- **An export from before the header's shadow clause** (theme2's T3, Grok 5) still finds its baseline, by the header's
  design digest: kept, or the design's own export; one value edited in it proposes exactly that edit (Astra 5).
- **Tests:** `a_shadows_function_folds_as_a_lengths_whatever_it_names` (unset then set; `calc(gap * 2)`; px; a pick
  over both), the literal's words for a border's and a gradient's base name and length token,
  `state_looks::a_look_follows_a_base_its_shadows_new_value_reads`, `tamber_serve_base`'s `length_texts` (and none
  for a px shadow), kernel `fold_lengths`' colour functions, tamber `an_export_from_before_the_shadow_clause_still_finds_its_baseline`.

**After review round 2 (Astra RIGHT WITH CHANGES, Grok RIGHT WITH CHANGES; every round 1 item confirmed):**
- **`fold_lengths` folds only a length** (Astra 1): a bare word, or a `calc()`, `min()`, `max()` or `clamp()` in any
  case; every other function is kept whole, so a platform colour whose asset is named `space.md`
  (`-exact-platform-color(ios named:space.md, #000)`) or a `contrast-color()` no longer makes a px shadow unfoldable.
- **A literal's name is Tamber's** (Astra 2): `scan::dotted_name` is tamber's own `is_name` (now exported) with a `.`,
  so `_eu.nope` is looked up as a token.
- **An unkinded shadow of lengths' functions** (Grok 1): `shadow::looks` counts a `calc()`, `min()`, `max()` or
  `clamp()` as a length, so `card.shadow = 0 max(1px, 2px) calc(2px * 3) black` gets "reads as a shadow: write its
  kind", not a colour's refusal.
- **Tests:** kernel `fold_lengths` (a platform colour, `contrast-color()`, `CALC()`);
  `a_platform_colour_named_for_a_base_is_kept_whole` (box and text); `an_unkinded_shadow_of_functions_asks_for_its_kind`
  (both repros); the literal's words for `_eu.nope` on a shadow and a border.

**After review round 3 (Astra RIGHT WITH CHANGES, Grok RIGHT WITH CHANGES; every earlier item confirmed):**
- **One length predicate** (both): `scale::is_length_item` (px, unitless `0`, a bare base name, or a `calc()`,
  `min()`, `max()` or `clamp()` in any case) is what `fold_lengths` folds and what the JS target's shadow rewrite
  counts as a length, so a platform colour whose asset is named `space.md` stays a colour there too, on a box and a
  text shadow, and no inserted length is paired with it.
- **QUEUE's per-column shadow entry** (Astra) says theme2's shape: `lengths` null for a length over base names, with
  `length_texts`.
- **An unkinded value's functions over plain numbers** (Grok) are a number's: `shadow::looks` counts a `calc()`,
  `min()`, `max()` or `clamp()` as a length only over a dimension, a length token or dial, a named length or an
  unset base name, so `n = min(1, 2) max(3, 4) calc(5)` keeps its earlier refusal.
- **Tests:** web-js `a_shadow_over_base_names_writes_their_properties` (the platform colour, box and text, as the
  row's CSS); kernel `is_length_item`; `an_unkinded_shadow_of_functions_asks_for_its_kind` (functions over base
  names, and over numbers).

**Follow-up: per-column shadow values** (`tamber/1.16`'s first change, stage 12 having closed: hello bumps; QUEUE's B3 entry beside D1's, the lead's 20:40Z task;
theme2's 20:50 ruling on `~/x2theme/x2p4-s12-shadowcols-plan.md`). A shadow token's value with a length over unset
base names gets D1's `columns` in D1's places: `explain`'s value, each `when` line (as the first tuple choosing it
folds it) and each of `resolve`'s tuples, `{ios, mac, neutral, material: {value}}`, each `value` the whole shadow's
text as that column's host reads it, by the kernel's own `scale::fold_lengths` over the scale table's column (never
a copy; a colour, a platform colour named `space.md` included, kept whole). Each of `resolve`'s shadows with
`length_texts` gains `columns: {col: {value, lengths}}` (Q2): its own text there and every length's px as a number,
in `lengths`' order, never null. No terminal column for a shadow (Q1): the terminal draws none, the doc's one stated
exception to D1's "left out only where it doesn't fold". A px-only shadow, or one over a base the app sets (folded
to px), has no `columns` key. rt1's D1 carry-overs hold: a zero is never negative (`calc(space.md * -0)` compiles to
a px length of -0, which the per-shadow numbers write 0; the kernel's `calc()` folds no zero to -0), a default's
columns come from the first tuple choosing it, read per column, and the key is absent, not empty. The code is
`shadow_columns_of` and `one_shadow_columns` beside `columns_of` in contract/cli/src/tamber_base.rs; D1's length path
is unchanged.
- Tests (`contract/cli/tests/it/tamber_serve_shadow_columns.rs`, every expected px from the kernel's evaluator): a box
  shadow over `space.md` and `calc(space.sm * 2)` (explain, its px-only `when`, resolve's token and per-shadow
  columns); a `when` line's list with a px-only shadow among two over bases; a default the first tuple doesn't
  choose; a text shadow; a platform colour named `space.md`; px-only and app-set shadows with no key; a length
  keeping `terminal` while a shadow has none; no -0.

### Stage 13: package icon tokens

theme2's 03:22 ruling on `~/x2theme/x2p4-s13-package-icons-plan.md` (Q1 yes, Q2 (a)); QUEUE "What a package's
design can't do yet" (1). D11 is amended (above).
- **Tamber.** An `Icon` carries `written`, the line its `icon(…)` is written on: its own value's, or, for an icon
  naming another, that one's (`icon::aliased` carries the value's line with its text).
- **The join** (contract/lower/src/theme/packages.rs). A package's `icons` block is no longer
  `tamber-package-block`: its icons are named under its namespace as its tokens are, banded into the app's design
  as its tokens are (the app's icon of one name is the icon, whole), and `references` refuses one naming an icon
  that isn't its own package's. LLP 1139 D8's check reads a library package's icons as names it defines, and an
  app icon under its namespace that it doesn't define is `tamber-package-unknown`, as a token is.
- **Where an SVG is read** (contract/lower/src/icons.rs). `written_in` finds the package design an icon's
  `written` line is in (a package's band, or a param's line whose identity is a package design's); `Home::of` is
  that package's root (the new `DesignSource::root`, from the resolved `Package.root`) or the app's. `svg_of`,
  `draws` and `declare` take it per icon; a refusal names the package's file ("the package `@acme/reading`'s
  `assets/star.svg`"), and a package's non-template SVG is `tamber-icon-svg`: "a package's pictures are one-colour
  templates in v1". A missing file is `tamber-icon-svg`, as an app's is.
- **Readers.** The source graph lists a package's SVG under its root, with the package's origin (one not there yet
  is consulted); the design server's `draws`, `tamber check`'s copy and the ops check of an icon param's SVG read
  the per-icon root; the levels walker reads a package's icons as its tokens.
- **Wire** (one clause on `tamber/1.16`). `describe`'s icon rows gain `package` and `file`, and `explain` of a
  package icon `package`, `file`, `identity`, `package_value` under an app override, and its value as its package
  writes it (`tamber_packages::owner` learns icons for rows; a write's ops still take tokens alone, no new op).
- **Round 1's open questions** (the lead's ruling, a second commit). A package's Contract component's icon param
  with a literal `icon(svg …)` default reads its package's `assets/` too, whether or not the package ships a
  design: the loader gives a package's components their package and root (`Params::package`, `Params::root`,
  from the source's `Origin::Package`), a `ParamToken` carries them (a design's from its `DesignSource`), and
  `written_in` reads a param's line through them; the source graph finds the package's origin by its root.
  `resolve` of a package icon carries `package`, `file` and `identity`, as of a package token (the 1.16 clause
  says so). Tests: `a_package_contracts_icon_param_reads_its_packages_assets` (with and without a design, the
  app's same-named file unused, the graph, a write's check, the refusal, the app's `params` line), and resolve's
  fields in `describe_and_explain_name_a_package_icons_package_and_file`.
- **After review round 1 (Astra, Grok: RIGHT WITH CHANGES).** Astra 3 = Grok 1 and Astra 8 are the commit above.
  - A package component's icon param may name its own package's icons (Astra 1): `references` counts them as its
    own.
  - A write's plan folds the package's icons beside the app's design (Astra 2): `tamber_params::joined_all` gives
    the tokens and the icons, and `params::Params` gains `joined_icons`, which `laid` adds under `icons` (one the
    design defines is the design's), so an app icon naming a package's doesn't refuse an unrelated write.
  - A package's SVG is confined to its package (Astra 4 = Grok 2): `Home::leaves` canonicalizes a package's file
    that is there and requires it under the package's own canonical `assets/`; else `tamber-icon-svg` ("leaves
    the package"). The build, `draws`, the write's check and `tamber check`'s copy refuse it, and the source graph
    lists no such file as a source (the link is consulted, so mending it rebuilds). The app's `assets/` stay
    unconfined: they are the author's to arrange, and the web stages them as they are.
  - A package-only design (no app design, no params) folds in the source graph too (Astra 5), so its SVGs are
    build inputs.
  - A package SVG edit is announced (Astra 6): the local wire's `hello {paths: true}` gives each package
    `assets`, a digest of the package SVGs the build reads (path and bytes; one looked for and absent by its
    path), and `scripts/tamber-package-designs.mjs` compares it with the design's digest, so an edit announces a
    `changed` naming the package's design. A client's hello carries neither.
  - A `color` on any drawn element makes an SVG no template, for every icon (Astra 7): two `currentColor` fills
    under two `color`s are two colours.
  - Tests: `package_icons.rs`'s `a_package_param_may_name_its_own_packages_icon`, `a_write_folds_the_packages_icons`,
    `a_package_svg_linked_out_of_its_package_is_refused` (a linked file, a linked `assets/`, the graph, `draws`,
    a write), `a_package_only_design_lists_its_svgs`, `a_package_svg_edit_moves_its_assets_digest`; the module test
    in `scripts/tamber-package-designs.test.mjs` (an SVG edit announced); `icon_svg`'s `color` cases.
- **After review round 2 (Astra, Grok: RIGHT WITH CHANGES; every round 1 item confirmed).**
  - A component's assets are the named package its identity names (Astra 1): `named_home` (now in
    contract/cli/src/sources/home.rs, beside `home`) gives the package's name and directory with the identity, so
    a package vendored inside another, or inside the app, reached by a relative `use`, reads its own `assets/`.
  - A `color` makes an SVG no template only where a path paints `currentColor` (Astra 2); a `color` nothing paints
    in is ignored again, as on main.
  - A package's SVG is read at the canonical path its confinement checked (both, R2-3), and an entry that can't
    be resolved (a dangling link) leaves; so for the assets digest and `tamber check`'s copy.
  - A package with no design whose SVGs the build reads (a Contract component's icon param) is in the local
    wire's `watch.assets`, `{package, file: "<package>/assets/", assets}`, which the watcher compares as a
    design's, so an edit announces `changed` naming `<package>/assets/` (Astra, R2-4).
  - A `style`'s property names match in any case (R2-5).
  - The wording: an SVG path is relative to the `assets/` of the root whose file writes it, the app's directory
    or the package's root (R2-6, here, D11's amendment and contract-for-agents); D11's older paragraph marks the
    icons refusal superseded (R2-7).
  - Tests: `a_vendored_packages_icon_default_reads_its_own_assets` (inside a package and inside the app, each
    beside the surrounding package's and the app's same-named SVG), `a_dangling_link_in_a_packages_assets_leaves_it`,
    `a_contract_only_packages_svgs_are_watched`, the module test of a design-less package's SVG edit, and
    `icon_svg`'s cases (main's `color` with a black fill a template; `COLOR`/`Color` names).
- **After review round 3 (Astra, Grok: RIGHT WITH CHANGES; every earlier item confirmed).**
  - `currentColor` resolves to the element's inherited `color` (attribute or style, any case) and joins the one
    colour comparison (R3-1): under red it is red, beside an explicit red fill one colour; under red and blue two;
    with no `color` set, the host's colour, as on main. Round 1's and 2's `color` heuristics are gone.
  - `!important` is read in any case and with space after its `!` (R3-2), so it neither hides a paint's value nor
    loses its precedence.
  - A package vendored in the app and reached by a relative `use` owns its SVG in the graph (a package origin from
    its own manifest, which is consulted) and on the local wire (`watch.assets`, its directory in `watch.roots`)
    (R3-3).
  - A package's file is read from the file checked (R3-4, `icons/confined.rs`): opened, its descriptor's device
    and inode taken, the path resolved again and required under the canonical `<root>/assets/` and to be that
    file, and read from the descriptor; nothing at the path is missing with no second open. The build, `draws`,
    the source graph and the write check (`Home::leaves`), the assets digest and `tamber check`'s copy all read
    through it. It closes a link planted or swapped in after the check and a parent directory swapped between
    the open and the re-check; a file replaced by another inside the package's `assets/` is still the package's.
  - Tests: `icon_svg`'s cases (main's, R3-1's, red and blue, `currentColor` beside red, `!IMPORTANT` and
    `! Important`), `confined`'s unit test (a planted link, a link inside, dangling, absent),
    `an_app_vendored_packages_svg_is_its_packages`, and the module test of an app-vendored package's SVG edit.
- **After the narrow re-check (Grok RIGHT; Astra RIGHT WITH CHANGES).**
  - A package an icon's SVG is read from owns it there or not (RE-1): the source graph's `owners` lists each
    such package (one vendored in the app included) from the identity's owner, its manifest is consulted, and the
    local wire's `watch.roots` and `watch.assets` come from it, so making a missing SVG is announced.
  - `color="inherit"` or `currentColor` keeps the parent's colour (RE-2).
  - A three-swap interleaving of the confined read is a stated limit, in confined.rs and QUEUE (RE-3).
  - Tests: `an_app_vendored_packages_missing_svg_is_still_watched`, the module test of a vendored package's SVG
    made where none was, and `icon_svg`'s `inherit` and `currentColor` cases.
- **Unchanged:** every app's plan and `tamber check` output (no app has a package icon), and `@exact/cards` and
  `@exact/actions`, whose icons are still path functions (their move to icon tokens is the follow-up).
- **Tests:** `contract/cli/tests/it/package_icons.rs` (the namespace and its refusal; an alias within the package
  and an app icon naming a package's; a package icon naming the app's refused; the app's override whole; D7 keeping
  only a read one; the package's `assets/` and not the app's same-named file; a missing one naming the package's
  file; a non-template one refused; the source graph; describe and explain rows and `package_value`; the levels
  walker; a write's check of a package default's SVG), `library_rules.rs` (a library's icons as its names), and `tamber_package_params.rs`'s `a_package_icon_params_svg_is_a_source` now expects the package's
  `assets/` (and an app `params` line the app's).

### Stage 13: as built, and closed (2026-10-23; brief `~/x2theme/stage13-brief.md`)

Stage 13 was one stage with five parts, each gated on its own (the lead's 2026-10-21 decision), so a part waiting
on Charlie held no other. Every part was built and reviewed under this RFC's process and signed off by theme2 (the
design tool's code by review2). The rulings are in `~/x2theme/SIGNOFFS.md`. Its wire is the one `tamber/1.16`
bullet: per-column shadow values (`76748724f3`, which opened 1.16), D4's `bases_by_tuple`, D5's Stretch form,
package icons, and the selection's `fromAt` with `inspect {select: {up}}`.

- **A, the hosted relay** (rt1): **R3** landed, the self-hostable relay service, through `ba5ca5e381`, with
  3 rounds and a security pass. **R4 is carried** (below).
- **B, external clients** (rt1): the `pen` importer's plan is approved (3 rounds and a security pass); it is in
  flight. **The real-Figma run is carried** (below).
- **C, the library's second wave** (iosfix with x2-perf4): `@exact/lists` (ListRow, SwitchRow; `21a1a836b3`),
  ProfileHeader in `@exact/cards` (`b821a5f4d0`..`9aa14743f2`), ComposeFrame (`f926a60524`..`304de7b893`) and BarItem and
  HeaderButton with lower-library-control's button rules (`ba2d017788`..`4f29a65443`) in `@exact/actions`, adopted by
  the clone on its signed-out, synthetic copy. Package icon tokens followed (`22746a70cd`..`67c8eeed8f`, D11
  amended above). Its gate is LLP 1139 §10's.
- **D, the checker's open gaps** (reload1): the write check judges font rows, computed rows and `svg` subtrees
  (D1–D3, `fade920ee5`..`93b827ca19`); `explain`'s `bases_by_tuple` (D4, `..76becad4ef`); the Stretch cell
  (D5, `21a1a836b3`..`f134cefd86`, `SCHEMA_DIGEST` `0x41fc_75ef_b151_8e26`). Then it judges a style's rows at each
  element that applies it (`f85d38ae54`..`4cc93c4172`), and written shorthand and flex rows
  (`3d66eb069b`..`7b784e5751`).
  - **What the write check judges now:** font and font-family rows through `type_tokens`; computed arms,
    conservatively over the tokens they can produce; `svg` subtrees, with lowering's renaming; a style's rows
    where each element applies it, refused in the build's order; a written shorthand or flex row as lowering
    expands it. A choice of literals is judged whole, `none` arms included, and a row is also reached by its
    expansion's text.
  - **Still the build's:** each of these is refused by the build alone, and QUEUE says each.
    - a computed shorthand or flex row;
    - transition and animation rows;
    - a `font` shorthand naming no type token, since it can name no design token;
    - a style row at an element the check leaves to the build: under `for`/`match`, a choice of styles, SVG,
      transforms, grouped lists, buttons, audio, `delete=`.
- **E, the design tool after the hands-on pass** (dtool2, LLP 1122): the ghost-builds follow-up, the Stretch
  "why" in the tool, a second pass over the library components with findings #1–#9 fixed or closed (#5 and #6 the
  1.16 lines above; #9 closed by a regression test, since it no longer reproduced), and a polish commit. The pass
  showed no need for per-instance variant pins (LLP 1139 §9 Q5) or native copies, so neither was built.

| Part | Gate | Result |
|---|---|---|
| A | R4's lines on a deployed relay; the PSL PR filed | **Carried**: R3 met its loopback gate; R4 needs hosting |
| B | The Figma line on a real file; a `.pen` file imported, re-imported and refused past the budget | **Carried**: the Figma run needs Charlie; `pen` is in flight |
| C | Stage 12's gate lines over the new components | **Met** (LLP 1139 §10; `~/x2theme/s13c-gate-run.txt`) |
| D | Each gap closed with same-words tests; plans byte-identical; the clone's write check in budget | **Met**: 55 write-check tests, each new one red under its mutation; 48 plans byte-identical; the clone's check 11–29 ms |
| E | The pass's top findings fixed, each with a contract test; any 1.16 wire signed by theme2 | **Met** (`~/x2theme/stage13-e-gate.md`, confirmed by theme2) |

**Carried to the next stage, named:**
- **R4**, the deployed relay's gate. It waits on Charlie's hosting (a VPS), the domain, how accounts are issued,
  and the PSL submission.
- **The real-Figma run** (10b's gate line). It waits on Charlie's session.
- **The `pen` importer**, in rt1's rounds.

**Found and fixed on the way:** an open popover laid out in its parent's flow, fixed in the kernel with one
accessor for a box's layout position and a guard on direct reads (`3fd6e52197`..`5bf1aecc9a`); iOS bar items and
custom invokers that turned any popover into a pull-down; a clock that waited on replies a server holds (LLP
1016.000 D9, `exactHeld` and `clock held`, `5752d1b032`..`58baabfe67`).

**QUEUE:** top-layer layout as a computed-style default, with a compile-time guard; `fill-rule` in icon templates;
multicolour package SVGs on the web; the Android slot-marker flake.

**Stage 13 is closed** with C, D and E met and A and B carried as named. The `tamber/1.16` window is closed; the next
wire change, a carried part's included, takes `tamber/1.17`.

### Stage 14: as built, and closed (2026-10-26; brief `~/x2theme/stage14-brief.md`)

Stage 14 was one stage of five parts, each gated on its own, like stage 13, so the parts that waited on Charlie held no others. Each part was built and reviewed under this RFC's process and signed off by theme2 (the design tool's code by review2). The rulings are in `~/x2theme/SIGNOFFS.md`. Every gate's run file shows the full contract suite green on its gate commit (the lead's 2026-10-23 rule, adopted after stage 13 C's gate missed a red test).

Its wire is the one `tamber/1.17` bullet. It was opened by the `pen` importer (`3b79eea272`), and it carries:
- the design tool's GET long poll by an opaque cursor, with `open=`, `asked`, a 204 heartbeat and 90 s presence;
- `edit-held`, with `editedBy`;
- the stale accept (`replacedBy`, `by`);
- `select` walking the page's own tree by the plan's `copies`, which a page registers and receives once per plan;
- the batched `resolve`;
- the held proposal's `choice`, `choiceBy` and `choiceAt`, with `choose`;
- the checker's limits (`tamber-value-size`, `-value-depth`, `-length-size`, `-reference-depth`, `-axes` 32, `-whens` 64, `-ops-size` 1024), with the local token's 16 MiB body for `import`.

- **A, the carried parts** (rt1):
  - the `pen` importer (`..3b79eea272`), meeting stage 13 B's `.pen` gate line;
  - LLP 1016.000 D8, long-poll streams (`491a366ca6..6f44d0e4e7`), on D9's held definition, with RUST_ABI 4 and TYPESCRIPT_RUNTIME 3 (every Rust-data and native TS-data app gets a new compatibility id once);
  - R4's deploy scripts and deployed-mode gate (`..3feed2923a`).
  - **R4 itself and the real-Figma run are carried** (below).
- **B, the popover's top layer** (iosfix):
  - a kernel-implied viewport block for every top-layer box (`d128446876..1d262df941`), a popover in a content region keeping its in-region layout (QUEUE);
  - then the web's default position-area and flips for unauthored painted popovers (`dc604c5725..b64c9876a6`);
  - sizing in the chosen option's region, by a pure kernel trial layout with a host measure cache (`1e00319304..14111ca060`);
  - the Mac split-pane popover fix.
  - **QUEUE:** the subtree-scoped trial (taken only if an app passes 16 ms an open), flip-start's margins, a toolbar popover's margin while open, an unflipped non-node anchor.
- **C, package icons finished** (x2-perf4, with iosfix on the Apple half), as built:

    **C, as built (x2-perf4, with iosfix on C2's Apple half).** A template icon now draws as its path on every host, at its node's exact frame. Even-odd templates work end to end. Every nonzero and even-odd path icon in @exact/cards and @exact/actions is a package icon token.

    **Landed, in order:**
    - **C1, Apple draws a template path as its path** (5b2bd17345..dfa3431b66). On macOS, a template-path icon (a symbolPath, no SF name) is CAShapeLayers in the node's SymbolClip: placed by IconPath at the real fractional frame, clipped to the placed em square, each path opaque in one group, with the tint's alpha applied once as the leaf's opacity. SF Symbols, bitmaps, control faces, accessibility, the tint source and intrinsic size are unchanged. iOS was already exact, so it is unchanged.
    - **The 15 nonzero path icons as package icon tokens** (ccb559261a..0dfdab3030). They are coloured by `color=`, since Apple symbols ignore `-exact-tint-color` (130e009940). library-gate's sample-ar-saved became ar-saved.
    - **C2, even-odd icon templates** (81c0f578c7..e30590f56f). One push of 18 commits: the field (x2-perf4, 9 commits), the Apple half (iosfix, 7 commits), review records, and a split-only caps commit. Every commit builds alone.
      - An icon template may use `fill-rule="evenodd"`, one rule per icon. The rule rides in the existing `icons.path` string and a literal source's `<path>` as a leading `evenodd:` marker. `exact_kernel::Icon` (`parse` / `of_row`) is the one reader.
      - The web mask honours the rule. Linux needed only tests. Apple sends `symbolFillRule` beside `symbolPath`, honoured in IconShapeView, IconPath, FaceSymbol's token and the widget snapshot.
      - The compat id gains `iconFillRules` only when a shipped plan holds an even-odd icon: main, entry plans, literal sources and baked data values are all checked.
    - **The last moves** (f351441c15..0e796f5bc8): ToolGlyph's gallery, camera and GIF, the Attie mark, and the verified and verifier badges as stacked ink/mark templates labelled on the container. The verified ground's circle became an arc. BarItem and HeaderButton keep the caller's d.

    **Gates:** each is under theme2's edge rule. Every differing pixel lies on the old render's AA fringe (interior 0, outside 0), and each icon has centroid < 0.25 px, spread < 0.25 px and coverage < 5%. Each gate runs on web, macOS and iOS, light and dark, and the full contract suite passed on every gate commit.
    - **C1:**
      - The probe test drives a real node at six origins, 1x and 2x, against an independent CG fill: identical, except 3 fringe pixels at 2x (.25,.25). The old NSImageView draw failed at every fractional 1x origin (centroid 0.39–0.79 px).
      - Release identity: no release app's Mac pixels changed.
      - The 15-icon fixture went from a worst centroid of 0.827 px and 82.8–96.6% ink to 0.019 px and 97.4–100.6% ink. All 48 light and dark instances pass.
    - **The 15 path icons:** 48 instances per host pass; worst centroid 0.037 px (iOS). The shift, scale and wrong-d mutations are caught.
    - **C2:**
      - 50 plans byte-identical, digests unchanged.
      - Bake time: the extra compile was removed, so the cost is noise.
      - Per-commit `cargo check --workspace` green at all 18 commits.
      - Mac 276 classes and iOS 2166 tests, 0 failed.
    - **The last moves:**
      - 27 icons per host; worst centroid 0.042 px, spread 0.025, coverage 0.47%.
      - The gate re-ran over main's host changes with identical counts.
      - library-gate's compat id is unchanged.

    **Stated limits and QUEUE lines:**
    - **Mixed fill rules:** an icon whose paths mix rules is refused in v1.
    - **The template lint closes hard:** no `style` attribute, no `<style>`, no namespaces, DOCTYPE or CDATA; `fill` and `fill-rule` must be exact values.
    - **The reader guard's Rust side** is enforced by type (`IconPathId`, escaped once in `icon_of`). Its threat model is accidental readers; deliberate laundering is left to review.
    - **The JS scan is a tripwire** with a stated limit (QUEUE): only files that name a symbol source are scanned.
    - **Multicolour icon files:** a package's multicolour SVG still isn't staged on the web (QUEUE). Apple still needs an `sf` for a multicolour icon (QUEUE).
    - **Linux non-template files:** Linux draws an icon token only as a template; a QUEUE line asks for its SVG scene renderer to draw non-template files.

    **Decided on the way:**
    - **No digest move:** a new column would have moved FORMAT_DIGEST and given every app a new binary. The marker rides in an existing string instead.
    - **`iconFillRules` is absent, not null,** unless used, so every existing compat id is unchanged.
    - **The edge rule gained locators:** centroid, spread and coverage were added after a 1 px shift passed the fringe-only rule on thin strokes.
    - **AppKit's placement:** NSImageView resamples template images at fractional origins, so path icons bypass it rather than modelling its resample in a check.
    - **Rule-insensitive icons are written nonzero:** all eight even-odd candidates turned out rule-insensitive, so writing them nonzero keeps package users' compat ids. The wrong-rule check runs on a synthetic same-wound ring instead.
    - **Moved icons use `color=`, not `-exact-tint-color`:** a branch still using the old idiom had drawn opaque on Apple.

- **D, the design tool's long poll** (dtool2): its changes poll moved onto `exactLongPoll` by the GET form (`644a7151e5..8782d84245`), the ECONNRESET flake fixed with a shown cause, and then the third hands-on pass's fixes:
  - #1, a silent full build announced;
  - #2, the stale Apply and accept;
  - #3, a commit re-baselined after a hand commit;
  - #4, select by the page's tree with slot hosts as levels;
  - #5, a peer's change reaching the grid in about 1.3 s, with the batched resolve;
  - #6, a re-held import keeping its overrides;
  - #7, the token editor's dock (`..81d9a9e4ea`);
  - #8, the native first-build status;
  - #9, `tamber tool --via` waiting while the loop starts.
  - Also: `edit-held`, held requests' `clock held` (stage 13's follow-on), the build-swap generations, the GPU cache race under a kernel `flock`, and the design-tool test time halved.
- **E, the write check's next gap** (reload1): computed rows under `for` and `match` and through `let`, in lowering's own scope (`1b1bc1f420..5e2de39ccf`).
  - **Stated limit:** when an untyped row is the build's first refusal, the check's first refusal is the build's next one; every refusal said is still the build's, in its words and place.
- **Beside the parts:**
  - the design checker's own limits (`dd0de0b3e4..a50256a77d`) and the sweep (the caps above), so the build and the server refuse a hostile value or count alike, and importers refuse with the same id;
  - the symbol-tint lint and the web following "a symbol draws in its colour on every host";
  - the iOS dev-joined blank rows: a generation reload's presenter reset called the new runtime before its committed
    batch applied; closed at one choke point (`Runtime.on`, `ResetGate.swift`'s replayed and dropped lists), signed
    off and landed after the close (iosfix, `024efaa22c..097aedfd17`);
  - Excord's relaunch reds: a launch reopens the channel it left, once, with its draft, yielding to any user action
    (`9265bf2c0b`; the full contract suite 2469/0 on its tip).
  - **Known reds at close:** two more Excord app tests failed on clean main on every run at load 20–36 ("scrolling up
    from the end acknowledges what was on screen" and "Mark As Read away from the end"). They aren't stage 14 work;
    x2-perf4 owns them as main-red fixes, being fixed now, not QUEUE'd.
    *(Follow-up, 2026-10-26:)* both are fixed on main, with "Escape from history", the same race
    (`995a737454`): each test waited for the list's asynchronous scroll report (`clock settle`) before reading;
    the Excord file 73/0, no known reds.

| Part | Gate | Result |
|---|---|---|
| A | `pen`'s gate line; D8's lane and compat tests; R4 on a deployed relay; the Figma line | **Met** for `pen` and D8. **Carried:** R4 and Figma |
| B | The repro, the hosted Pins case against Chrome, the split case, the full suites | **Met** |
| C | The edge rule (centroid, spread and coverage) on web, macOS and iOS, light and dark | **Met** |
| D | The four design-tool files 18/18 under load, with times; the 1.17 lines signed | **Met** |
| E | Same-words tests red under mutation; plans byte-identical; the clone's check in range | **Met** |

**Carried to the next stage, by name:** R4 (the deployed relay's gate: Charlie's VPS, the domain, account issuance, the PSL PR) and the real-Figma run (Charlie's session).

**Stage 14 is closed.** The `tamber/1.17` window is closed, and the next wire change takes `tamber/1.18`.

**A note on commit hashes (2026-10-26):** exact2's history was rewritten at about 20:00Z that day. Hashes written in this RFC before 2026-10-26 20:00Z refer to the pre-rewrite history; the team keeps an old-to-new map.
