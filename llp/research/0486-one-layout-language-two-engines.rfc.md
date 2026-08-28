# LLP 0486: One Layout Language, Two Engines

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Contract, Layout, Kernel, Renderer (web host), Verification, Tooling
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-19
**Revised:** 2026-08-20 (clock-join sync per the 2026-08-20 holistic corpus reviews — start/end widen-or-translate marked DECIDED translate-via-table per 0491 r16's joint amendment (discharging the WS2 obligation and 0491 Phase 2's prerequisite).) 2026-08-20 (r4 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision. Logical `start`/`end` alignment reclassified from portable to a
named current divergence with a WS2 widen-or-translate obligation (the wire
and kernel enums are flex-relative only while Taffy distinguishes logical
End — verified); §7's closing invalidation sentence scoped to families 1–2;
one 0487 phrase ("owner-in-Review; predicates frozen per 0487 §1"); WS2
gains the designated Frame-pair table, the capture-time metric-stack
fingerprint rule, the stable-gutter pin obligation, and the waiver-schema
enforceability obligations; §1 marked historical; three stale citations
fixed.) 2026-08-20 (r3 — program super-refine round 1: §5.2/§6.1/§10
restated against the landed tree (profile authority, `layout-profile-parity`,
the registered `contract-cross-host-parity` lower bound, closed EXLT/inspector
tickets); §4 Semantic/Decision split given the equal-input precondition and
Frame made machine-actionable per designated engine/provider pairs, with the
landed `exact-contracts.json` Frame sentence flagged as a WS2 amendment
obligation; §7 recast as three typed execution classes; §8.2 hidden-clone
option struck per 0487 M2; §8.3 graduation evidence pre-registered; 0487 cited
as owner-in-Review, never "frozen"). 2026-08-19 (r2)
**Related:** RFC 0084 (Layout, Styling, and Host Mapping — this RFC grows out
of its "Direction Stub: A Contract Layout Algebra, Not CSS Emulation" as
revised 2026-08-19, and proposes answers to two of its open questions),
LLP 0195 (Responsive/Adaptive Layout — owns size classes and the
container-size feedback channel this RFC sequences but does not spec),
LLP 0199 (Layout Intent and Constraints — owns relation IR; its measured
tier is a consumer of the shared channel), LLP 0202 (Contract Web Host —
owns the web host design and the §10 conformance honesty ruling; this RFC
directs that its promised `contract-cross-host-parity` check finally be
built), LLP 0288 (Contract Is the Web Production Target — Active; nothing
here changes that posture), LLP 0349 (Programmable Layout — owns registered
evaluators; the island realization in §7 is a bounded instance of its
model), LLP 0423 (minimum runtime substrate — its E4 instrument is
subsumed by §6 here), LLP 0478 / LLP 0483 (web stays real-DOM; this RFC is
the layout-semantics counterpart to their execution-tier rulings),
LLP 0150 (authority map — §5 adds a layout-profile authority row following
the protocol-inventory pattern), LLP 0157 (facet-core owns styling truth —
untouched; this RFC is about layout semantics, not tokens),
LLP 0487 (choice-layout semantic kernel — normative for §8 semantics),
LLP 0488 (implementation spec for the §7 channel)

## Summary

Exact runs Contract layout on two engines: kernel/Taffy on native and TUI,
and real browser CSS on web. Today those engines agree only because their
defaults were matched by hand, the doctrine that governs their divergence
(LLP 0202 §10) has no instrument measuring it, and every future layout
feature (container adaptation, measured relations, choice layout) is
specified in a different RFC with no shared account of what "the same
layout" means across hosts.

This RFC proposes the doctrine and the machinery:

1. **Different engines, same language.** Contract's layout vocabulary is a
   closed, growing catalog of operators with Exact-owned semantics. An
   operator exists on a host either through a **certified lowering**
   (CSS or Taffy, proven faithful by fixtures) or through an
   **Exact-owned evaluator** with an honest realization on that host.
   Nothing enters the public vocabulary without both stories.
2. **The Contract Layout Profile** (§5): a machine-readable **conformance
   manifest** for the portable layout vocabulary — pinned defaults, four
   portability states, spec citations, fixture IDs, and named Exact
   deviations — that generates the web defaults and parity-gates *resolved*
   cross-path behavior, ending the hand-matched-defaults era (ENG-22199
   class). It is deliberately not a property allowlist and never contains an
   algorithm.
3. **Parity tiers** (§4): **Semantic / Frame / Band / Decision**. Semantic
   parity everywhere; Frame parity only where every intrinsic-measure
   provider is normalized (an input condition, not a host list); Band
   tolerances wherever a browser or a host-owned measurement participates;
   and a Decision tier for operators that *select*, because a sub-pixel
   difference that flips a winner is not describable as a box tolerance.
   This answers RFC 0084's open question ("which layout invariants require
   frame parity across hosts?") and turns LLP 0202's "budgeted, never
   asserted" divergence into a number somebody measures.
4. **The conformance instrument** (§6): the dual-engine fixture harness —
   identical IR through kernel/Taffy (EXLT v1 export) and through real
   Chromium (CDP frames) under controlled fonts — plus the long-promised
   `contract-cross-host-parity` semantic check, a generated divergence
   catalog, and the version-controlled waiver allowlist LLP 0202 already
   designed.
5. **One feedback fabric, three typed execution classes** (§7): LLP 0195
   S2's container-size channel is built once and budgeted once — one
   budget ledger and one diagnostics/correlation envelope — but the
   consumers split into three classes with different timing: synchronous
   counterfactual evaluation (choice `fits`, §8), next-frame size
   observation (LLP 0195 branching, LLP 0199 `matchWidth`/`matchHeight`),
   and demand-driven placed geometry (`alignTo`, which ResizeObserver
   cannot serve). On web the size transport is ResizeObserver; native is
   the harder host. LLP 0488 is the implementation spec.
6. **Choice layout with a CSS-first realization ladder** (§8): the first
   algebra operator beyond flex/grid, whose semantics are **owned by
   LLP 0487**, not restated here. Its web realization is a ladder, certified
   per instance: lower to CSS `@container` queries (zero JS, SSG-safe) when
   0487's admission rules discharge, otherwise an Exact-evaluated island that
   preserves real DOM and source order. This RFC owns the certification,
   realization, sequencing, and graduation evidence.

Taffy flex/grid on native and browser CSS on web both remain exactly where
they are. This RFC changes who owns the *meaning*, and builds the
machinery that keeps every host honest to it.

## 1. Motivation

*(Historical: this section records the failure modes as they stood at r1,
2026-08-19. Several instruments it describes as missing have since landed —
§5.2, §6.1, and §10 carry the current state; where they disagree with this
section, they win.)*

### 1.1 We are already in "World 2" — in its degenerate case

A from-scratch layout system ("World 1": Exact owns rendering on every
platform) and a web-constrained one ("World 2": one host is a browser we
do not control) differ in *engine* but need not differ in *language* —
provided the vocabulary is chosen so every operator has a faithful
realization in both worlds. Exact is structurally committed to World 2:
LLP 0288 made Contract the production-web target on real DOM, LLP 0202
built the host, and LLP 0478/0483 keep web on real DOM for execution
reasons too. On web, the browser owns layout end to end
(`packages/exact-native-web/src/protocol-dom-host.ts:3`: "Browser layout
remains authoritative; no kernel/Taffy layout result is used").

That is the right call — real DOM buys selection, accessibility,
find-in-page, zoom, print, browser-quality text, and free incremental
layout; the wasm-engine-plus-absolute-DOM alternative is the Flutter-web
cautionary tale. But we currently hold the *degenerate* version of this
position: CSS lowering for everything, agreement by hand-matched
defaults, and no certification machinery. Three symptoms:

**Parity is manufactured, not structural.** Contract's RN-style containers get
`flexShrink: 0` — applied in TypeScript before protocol encoding
(`packages/exact-renderer/src/host-ops.ts:895`), *not* by the kernel, whose
own absent-field default is `1.0` (`kernel/src/style.rs:591`). CSS also
defaults to `1`, so the web host hand-matches
(`packages/exact-renderer/src/dom-element-description.ts`), and when the
matching slipped a bare `row`/`column` collapsed to zero height on web while
native was fine (ENG-22199, with a sibling bug in the React tier, ENG-22206).

The situation is worse than "two tables in two codebases": there are **four**
default layers (§5.2), they are not all in the language people assume, and
they already disagree per tag today — native gives RN-style shrink zero to
Text, Image, SVG, Video and TextInput leaves
(`packages/exact-renderer/src/tags/tag-map.ts:181`) where the DOM injection
covers only View, ScrollView, Pressable and List. Nothing generates or gates
any of it. This bug class will recur until resolved behavior has one
authority.

**The doctrine has no instrument.** LLP 0202 §10 rules that conformance
is asserted at the semantic-tree level only and that "pixel/layout-box
equivalence is never asserted, only budgeted." But the check it promises
(`contract-cross-host-parity`, spec'd at LLP 0202 with a version-controlled
waiver allowlist) has never been written; `contract-native-parity`'s
parity half is a manual stub that exits nonzero
(`scripts/check-contract-native-parity.mjs`); and the only real
cross-host geometry oracle in the repo is a 480-row single-line-Latin
scene-text *width* fixture (`tests/graphics/text-parity/`). A budget
nobody measures against is a vibe.

**The future layers have no shared semantic account.** LLP 0195 (S2
container feedback — unbuilt), LLP 0199 (measured relations — stored
intent returning `null` today,
`packages/exact-contract/src/runtime/layout-intent.ts`), LLP 0349
(programmable evaluators — unbuilt), and RFC 0084's choice-layout stub
each define a piece of post-flexbox layout, but no document says what
cross-host equivalence each piece must satisfy, or that they should share
one feedback channel and one certification bar.

### 1.2 What first principles actually say

Condensed from the analysis behind RFC 0084's direction stub; recorded
here because it drives the design:

- **Every workable layout system is one protocol** — constraints down,
  sizes up, parent positions — plus a library of operators. CSS
  implements the protocol without admitting it, which is why its
  emergent behaviors (cascade-entangled layout, margin collapsing,
  anonymous boxes, floats, percentage circularities) exist for
  document-history reasons, not layout reasons. Contract already
  refuses most of these by construction: no margins in the vocabulary,
  no implicit text boxes, no floats, no cascade.
- **Layout = a discrete arrangement decision ∘ a continuous metric
  solve.** Flexbox and grid solved the metric layer well; CSS handles
  the arrangement layer badly (media queries at document scope, far
  from the container they affect). Choice layout is a first-class,
  container-local arrangement layer — it does not compete with flexbox,
  it supplies the layer CSS never had.
- **Anything beyond single-pass is an explicit, budgeted feedback
  stage:** stage 0 = pure one-pass (flex, grid, `fill()`/`ratio()`);
  stage 1 = bounded fixed-point inside one layout pass (candidate
  selection, measured-sibling match); stage 2 = discrete re-layout
  through the reactive tier on a later frame (LLP 0195's channel).
  LLP 0195/0199/0349 already implicitly hold this shape; this RFC names
  it so all three carry the same budget-and-invalidation contract.
- **Bounded relations, not a solver.** Auto Layout's Cassowary history —
  ambiguity errors, unsatisfiability, performance cliffs, and Apple's
  own retreat to stacks — is why LLP 0199's "handful of relations with
  deterministic resolution" is correct from first principles, not just
  pragmatism. Nothing here reopens that.
- **Text is the boss.** Text is the only content whose size is a
  function (width → height), not a value, and the browser's line
  breaker will never match CoreText's. Cross-host invariants must be
  designed to tolerate text-metric variance; that is what forces the
  tolerance-band tier in §4.
- Even unconstrained, a from-scratch design converges on flex-basis /
  grow / shrink-with-min-clamping and fr-unit track sizing, because
  they are genuinely good solutions. So "not CSS emulation" (RFC 0084)
  is a claim about **authority and curation**, not about novel math:
  for the metric core the honest position is *CSS's two good
  algorithms, extracted from CSS's bad language* — adopt the spec,
  restricted, and own the conformance fixtures that decide who is wrong
  when engines disagree.

### 1.3 Why now

Three triggers. First, the choice-layout stub landed in RFC 0084
(2026-08-19) and needs graduation machinery before any code exists,
or it will graduate on vibes. Second, LLP 0199's measured tier and
LLP 0195's S2 are both parked on the same unbuilt channel, and building
it twice (or building it without a budget) is the expensive failure.
Third, the divergence catalog is empty at the exact moment agents author
most Contract UI: every undocumented Taffy/CSS disagreement is a bug an
agent will faithfully reproduce onto the other platform.

## 2. Goals

- One authoring vocabulary whose layout semantics are Exact-owned and
  identical in meaning on every host.
- A machine-readable core-semantics authority that ends hand-matched
  defaults.
- Explicit parity tiers, continuously measured, with a version-controlled
  waiver ledger.
- One container-size feedback channel with one per-frame budget, shared
  by every stage-1/stage-2 feature.
- A graduation protocol under which choice layout (and any future
  operator) ships with certified lowerings or honest evaluators on every
  host — and a CSS-first realization ladder that keeps the common case
  free on web.
- Keep everything that makes web *web*: real DOM, source order, a11y,
  SSG/no-JS, hydration economics (the LLP 0288 evidence gates).

## 3. Non-Goals

- Replacing browser layout with a wasm kernel on production web
  (reaffirms LLP 0202/0288/0478; the wasm `native-parity-web` profile
  stays an instrument).
- A general constraint solver (LLP 0199's stance stands).
- Pixel parity between kernel hosts and browsers, ever.
- Taking over LLP 0195/0199/0349/0202 ownership — this RFC sequences and
  consumes them; their specs stay theirs.
- A new styling/token system (LLP 0157 untouched).

## 4. Parity tiers (the doctrine)

Encoded as the layout boundary's contract in `exact-contracts.json`.

**Naming.** These tiers are named **Semantic / Frame / Band / Decision**, not
T1–T4. LLP 0487 §7 is titled "Re-selection timing" and already owns T1/T2/T3
for a completely different concept (when selection is recomputed relative to
the layout pass), and 0487 §10 cross-cites "LLP 0486 §4's T1 row." 0487 is
owner-in-Review with its predicates frozen per 0487 §1 — the phrase this RFC
uses for it everywhere; this RFC renames rather than perpetuate a collision
between two documents that cite each other.

| Tier | Claim | Where it holds | How it is checked |
| --- | --- | --- | --- |
| **Semantic** | Same tree, same order, same containment, same enabled/visible semantics, same relation intents, and — for selecting operators — the same *selection function* and logical slot tree with its order guarantees — plus the full LLP 0202 §10.1 list: roles, landmarks, heading order, list semantics, interactive names, focus order, `inert` exclusion. Selected *winners* are not in this row: they compare under Decision, which carries the equal-input precondition | Every host, always | `contract-cross-host-parity` (§6.1) |
| **Frame** | Same pre-presenter `x,y,w,h` up to rounding rules | **Only when Contract IR, environment, and every intrinsic-measure provider are normalized** — an input condition, not a host list. Machine-actionable form: Frame comparisons run only between **designated engine/provider pairs** recorded in the profile authority (e.g. kernel+CoreText vs kernel+CoreText); a pair not designated compares under Band | EXLT v1 fixture diffs (`tests/layout-export/`, once §6.2's gaps are closed) |
| **Band** | Frames agree within published per-class tolerance bands | Whenever browser layout or any host-owned intrinsic/environmental measurement participates | Dual-engine harness (§6.2); bands set empirically (§6.3) |
| **Decision** | Identical operator winners under canonical measurements; under live measurements, differing winners only where the decision boundary lies inside the declared measurement-uncertainty envelope; settlement within bounded frames | Every host with a selecting operator | Decision-record diffs (§9, LLP 0487 §9 D4) |

Rulings:

- **Semantic parity is unconditional.** A violation is a release-class bug on
  the default authoring path. For operators that *select among arrangements*
  (choice layout, LLP 0487), it binds the selection function and the logical
  child tree with its order guarantees; the realized arrangement is equal
  across hosts only where the operator's decision inputs are equal, and is
  otherwise governed by the Band and Decision tiers. The Decision-tier
  comparator therefore carries an explicit **equal-input precondition**
  (LLP 0487 P1/P2's shape): winners are compared only after decision-input
  records are established equal, or under 0487's declared
  measurement-uncertainty rule — never as an unconditional Semantic
  assertion.
- **The landed authority row must be amended to this ruling (WS2
  obligation).** The `exact-contracts.json` layout row still carries the
  pre-r2 scoping — "Frame x/y/w/h parity is claimed only among kernel-owned
  hosts" — which this section explicitly rejects (two kernel hosts with
  different shapers are Band-related). WS2 amends that row to the
  normalized-inputs / designated-pairs formulation and adds the Decision
  tier, which the row currently omits.
- **Frame parity is scoped by normalized inputs, not by "has a kernel."**
  The earlier draft scoped it to kernel-owned hosts, which is wrong: Taffy
  consumes host-supplied intrinsic measurements, including platform text and
  native-control callbacks (the kernel builds its Taffy tree at
  `kernel/src/lib.rs:1021` and feeds it host measure callbacks). Different native
  control metrics propagate through ancestors and siblings exactly like text
  metrics do. Two kernel hosts with different shapers or different scrollbar
  models are Band-related, not Frame-related. A useful way to read the
  condition: Frame parity holds *within* a metric-stack island
  (`{Apple, CoreText, overlay}`, `{Windows, DirectWrite, reserved gutter}`,
  `{TUI, cells}`) and Band parity holds *between* islands. A static host-pair
  allowlist is necessary but not sufficient: fonts and font assets, shaper
  versions, native-control measurers, scrollbar policy, DPR, locale, and
  writing mode all move frames, so every Frame-eligible capture records a
  **metric-stack fingerprint** at capture time, and Frame comparison runs
  only between captures whose governed fingerprints are equal — an unequal
  or ungoverned fingerprint automatically reclassifies the comparison as
  Band. The designated-pair table and the fingerprint's governed field list
  live in the profile authority (a WS2 obligation — neither exists there
  yet, which today makes Frame eligibility prose rather than data).
- **Frame parity is never claimed against a browser.** Cross-*browser*
  variance already makes it unclaimable on web alone; this resolves RFC 0084's
  frame-vs-semantic open question by measurement, not preference.
- **Band floors are data, not aspiration** — but cross-browser disagreement
  is not automatically the floor. Separate genuine algorithmic variance from
  repeatability noise and from known browser *defects*: a defect earns an
  engine-specific waiver or compensation, it does not widen every host's band.
- **The Decision tier is not reducible to a box tolerance.** A 0.4px
  measurement difference that flips the selected candidate is a categorical
  divergence, and no final-snapshot comparison detects a one-frame flash, a
  stale revision, or an oscillation. This tier is why §9's provenance and
  LLP 0487 §9's decision records are load-bearing rather than debug niceties.
- Waivers carry owner, issue, engine/version, affected fixture/property,
  maximum deviation, and expiry. Ordinary in-band differences do **not**
  generate waivers — see §6.1 for why conflating the two would break the
  first check we ship. **Enforceability status (2026-08-20):** the landed
  waiver ledger cannot yet represent this contract — its validator admits
  only `id`/`fixture`/`phase`/`box`/`reasonClass`/`reason`/`owner` and
  rejects the additional fields (`scripts/lib/contract-cross-host-parity.mjs`,
  `validateLayoutWaivers`) — so today's `layout-profile-parity` is a
  **declared-drift gate** (it stays green while recorded divergences stand),
  not proof that resolved paths agree. WS2 owes: the schema migration adding
  engine/version, maximum-deviation, and expiry fields; expiry enforcement
  and numeric deviation checks in the validator; and distinct names for the
  drift-recording gate versus the future conformance gate, so green cannot
  be quoted as the stronger claim.

## 5. The Contract Layout Profile (the shared core)

### 5.1 Definition

The Contract Layout Profile is a **conformance manifest** for the portable
layout vocabulary — deliberately not a property allowlist, and emphatically
not a parallel CSS specification (§11 names that as the failure mode). Each
entry carries a normalized feature/value ID, applicability, a portability
state, pinned defaults, explicit Exact deviations, a dated normative CSS
section reference, fixture IDs, and its enforcement consumers. **The file
never contains an algorithm**: if a reader could implement flexbox from it, it
has become the thing §11 warns about.

**Four portability states, not two.** A binary included/excluded split forces
the file to lie, because Exact's real behavior is full of *defined*
degradations:

- `portable` — same meaning on every host.
- `defined-degradation` — authorable everywhere, with a named, documented
  reduction on some host.
- `host-extension` — available on some hosts only, by design.
- `rejected` — excluded by construction.

Honest v1 content, corrected against the tree (the earlier draft of this
section stated three things that are false, and they are called out because
lanes were about to encode them):

- `portable`: flex line layout (basis/grow/shrink with min-size clamping),
  gap, padding, aspect-ratio, min/max constraints, relative/absolute
  positioning within a `stack`.
- **Logical alignment (`start`/`end`) is NOT portable today — a named
  current divergence, verified 2026-08-20.** The intent (logical, never bare
  left/right) stands, but the native wire cannot carry it: the protocol
  encoder's alignment vocabularies are flex-relative only
  (`JUSTIFY_CONTENT_VALUES`/`ALIGN_ITEMS_VALUES`/`ALIGN_SELF_VALUES` in
  `packages/exact-core/src/protocol/buffer-writer.ts` contain no
  `start`/`end`, and an unrecognized value takes the dev-warned fallback —
  `justify="end"` encodes as `flex-start`), the kernel's enums expose only
  the flex-relative variants (`kernel/src/style.rs:107`), while vendored
  Taffy *does* distinguish logical `Start`/`End` from `FlexStart`/`FlexEnd`
  (`vendor/taffy/src/style/alignment.rs`) and DOM preserves CSS `end`. Under
  `row-reverse` the two vocabularies diverge observably. WS2 obligation: the
  profile entry records this as a divergence (not `portable`), and the repair
  decision — widen the protocol/kernel enums to the logical variants, or
  prove a contextual translation at encode time — is taken with
  reverse-direction, RTL, and writing-mode fixtures for `align`, `alignSelf`,
  and `justify` before the entry may return to `portable`.
- `defined-degradation`: **grid**. General `fr`/`minmax`/`repeat` is *not*
  portable — native normalization reduces templates to a count of equal
  tracks and warns on non-uniform sizes
  (`packages/exact-renderer/src/style/normalize.ts:437`), and item-level
  spans are unauthorable (`issues/20260726-grid-does-not-place-on-native.md`).
  Portable grid v1 is effectively `cols=N` plus gap.
- `rejected`: **margin collapsing** (not margins — Contract publicly exposes
  `margin*`, `packages/exact-contract/src/compiler/builtin-lowering.ts:119`;
  the earlier claim that margins are "excluded forever" was simply wrong),
  anonymous box generation (text nodes are explicit), floats, the cascade,
  `box-sizing` other than border-box, percentage circularities outside the
  defined flex/grid resolution.
- **Pinned defaults** live in the authority file, not here — and they are
  keyed per *lowered host tag*, because the tag-level split is itself the
  divergence (§5.2).

**`style=` is outside the manifest.** It accepts arbitrary keys
(`packages/exact-contract/src/runtime/layout-intent.ts:39`) and DOM lowering
can emit arbitrary CSS, so no property list closes more than the portable
vocabulary (this RFC claims a **closed portable vocabulary**, never a closed
language). The manifest
covers the portable vocabulary and names `style=` as the escape hatch beyond
it; driving compiler diagnostics for `style=` keys and value grammars from the
manifest is a later, separately-scoped step.

Taffy is vendored **because** it is not CSS
(`vendor/taffy/EXACT-PATCHES.md`), so "the profile costs nothing on native"
is not a claim this RFC makes. Its value is that **the fixtures decide**:
where a browser and Taffy disagree inside the manifest, the conformance
fixture plus the cited spec section rules which is wrong, and the loser gets a
patch or a waiver with an owner. Fixtures alone can detect disagreement; they
cannot decide who is normatively correct without the spec citation, which is
why every non-`rejected` entry carries one.

### 5.2 Mechanics — the protocol-inventory pattern

Following LLP 0150's one-authority rule and the
`tests/protocol/protocol-inventory.json` precedent. **Status (2026-08-20):
this machinery is landed** — restated here as current state so the RFC stops
describing work WS2 already did:

- Authority file `tests/layout/layout-profile.json` **exists** — the manifest
  of §5.1, pinned defaults keyed per lowered host tag, the rounding record
  (§5.3, recorded as two-stage current law plus the target), and pinned
  facts (including the `measuredBoxObservation` border-box/node-relative
  pin LLP 0488 consumes) — with provenance headers.
- **Generated from it:** the web host's tag-default table consumed by
  `describeDomElement` (`packages/exact-renderer/src/dom-element-description.ts`).
- **Parity-gated against it:** *resolved behavior*, not one implementation's
  absent-field fallback — the check `layout-profile-parity` is registered in
  `exact-verify.json` (governance profile). ENG-22199 is CI-impossible.
- The `exact-contracts.json` "layout core semantics" row exists — but still
  carries the rejected Frame scoping and no Decision tier; amending it is a
  WS2 obligation (§4).
- **Still owed (WS2 hardening):** the profile has no fail-closed
  completeness rule — the generator validates declared entries and resolved
  defaults but does not prove the *total* join from the compiler's layout
  vocabulary (`builtin-lowering.ts` value domains) to profile entries. Owed:
  a total join in the generator, CI failure on an unknown/unprofiled
  addition, and an executable fixture for every non-`rejected` claim (an
  entry with an empty fixture list is a claim nobody can check). And
  **`style=` must be scoped, not just named**: unprofiled layout-affecting
  `style=` keys in a build that claims portability are an error or carry an
  explicit host-extension marker; today the escape silently defeats the
  manifest (arbitrary CSS survives DOM lowering while native drops it).

**Correction: there is no single "kernel default" to gate.** An earlier draft
of this section said to parity-gate "the kernel's `buildDefaultCanonicalStyle`
(Rust)". That function is **TypeScript**
(`packages/exact-renderer/src/host-ops.ts:895`), and gating Rust's defaults
would have gone green while effective behavior stayed divergent — Rust's
`StyleProps::default()` is `flex_shrink: 1.0`
(`kernel/src/style.rs:591`, asserted at `:1309`), while the effective native
`flexShrink: 0` is applied in TypeScript before protocol encoding.

Defaults arrive in **four layers**, and the manifest must model them rather
than flatten them:

1. Contract/RN tag defaults, in TypeScript (`host-ops.ts`);
2. protocol absent-field fallbacks, in Rust (`kernel/src/style.rs`);
3. DOM CSS compensation (`dom-element-description.ts`);
4. a further React-web defaults copy.

So the gate compares **resolved values per Contract tag along the two real
paths** — Contract lowering → canonical style → protocol decode/kernel, and
Contract lowering → DOM lowering — plus the precedence rules between authored,
tag, transport, and engine defaults. A separate check may pin Rust's transport
fallbacks; it is not this one.

**A verified divergence the manifest must record rather than smooth over:**
native applies RN-style `flexShrink: 0` to Text, Image, SVG, Video, TextInput
and other leaves (`packages/exact-renderer/src/tags/tag-map.ts:181`), while
the DOM injection covers only View, ScrollView, Pressable, and List (the
`WEB_CONTAINER_TAGS` set, `dom-element-description.ts:225`, branched at `:551`
and `:655`); web additionally injects `minWidth/minHeight: 0` where Rust
defaults them to `Auto`. A generator that emits one flat defaults map erases
this and reintroduces the ENG-22199 class. Text-as-flex-item,
Image-as-flex-item, and min-size fixtures belong in the first release.

### 5.3 Rounding and DPI

Pixel snapping at fractional scale factors is where Frame diffs hide.

**Current state, recorded honestly:** rounding does *not* happen once today.
Vendored Taffy enables internal rounding by default
(`vendor/taffy/src/tree/taffy_tree.rs:82`; the kernel builds the default tree
at `kernel/src/lib.rs:1021`), and presenter snapping happens again downstream.
So there are at least two rounding stages, and they are separate observable
contracts.

The single-rounding rule — rounding once, at frame emission, per host DPI,
with the kernel's rule as the reference — is a **target with an owner and its
own migration**, not a description of the system. The authority file records
what the two stages actually do now and cites the target; a profile that
stated the target as current fact would make every Frame-tier fixture
disagree with the code for reasons the fixture could not explain. Browsers sit
outside the Frame tier either way, so their subpixel behavior is absorbed by
Band tolerances.

## 6. The conformance instrument

### 6.1 `contract-cross-host-parity` (Semantic)

**Status (2026-08-20): the check exists — as a registered, deliberately
labelled lower bound, not as this section's full bar.**
`contract-cross-host-parity` is registered in `exact-verify.json` and
implemented (`scripts/check-contract-cross-host-parity.mjs`,
`docs/contract-cross-host-parity-capture.md`): its web leg mounts through
the compatibility DOM mirror in HappyDOM — no layout engine, no browser
accessibility tree — and its records are labelled
`authored-layout-declaration-lower-bound`. That is a real Semantic-tier
instrument and it is not §6.1 done: the bar this RFC sets — both host arms
demonstrably executed, and a web leg reading **rect-derived** frames from a
real browser — is still unmet, and the upgrade path is exactly §6.2's
dual-engine lane. Treating the lower bound going green as Band or Frame
evidence is the failure mode to refuse.

**Split the semantic gate from geometry policy.** LLP 0202 §10.1 pairs the
semantic comparison with a per-box layout waiver allowlist, and the earlier
draft of this section inherited that pairing. It cannot ship that way: running
the Caltrain twin and the facet-contract fixtures through two different layout
engines produces *thousands* of sub-pixel disagreements, and a per-box
allowlist meeting that volume either explodes or is rubber-stamped wholesale.
A rubber-stamped allowlist is worse than none, because it launders divergence
as reviewed. So:

- **Hard-failing now:** the canonical semantic/AX/decision-trace comparator,
  with **no geometry waivers**. Scope it to LLP 0202 §10.1's full list — roles,
  landmarks, heading order, list semantics, interactive names, focus order,
  `inert` exclusion — not the §4 table's summary line.
- **Reported, not enforced:** layout-box divergence, emitted as a classified
  artifact. That output is exactly the input §6.3's band-setting needs.
- Enforcement of geometry arrives with the Band tier, through §6.2's
  comparator and narrowly-scoped out-of-band waivers carrying owner, issue,
  engine/version, maximum deviation, and expiry.

This is a **proposed amendment to LLP 0202 §10.1's acceptance threshold**, and
belongs to that RFC's owner to adopt.

Two registration requirements, both learned from the check this one replaces:
the registered check must **fail if either host arm or the comparison itself
did not execute** — `contract-native-parity` today registers as
`bun scripts/check-contract-native-parity.mjs` with no `--launch`
(`exact-verify.json`), so it can green after the build/HBC preparation alone,
which is precisely the failure mode a check named "parity" must not have. And
the web leg must read **rect-derived** frames: the agent inspector's
`computeApproximateFrames` (`packages/exact-renderer/src/inspector.ts`) is a
hand-written JS flexbox, and a parity check whose web leg reads a
reimplementation of flexbox is measuring the wrong engine.

### 6.2 The dual-engine frame harness (Band, feeding Frame)

- **Fixture corpus:** compiled Contract IR programs — start from the
  existing corpus (caltrain app, the 47 facet components, labs) plus
  targeted profile-edge fixtures (min-content corners, percentage
  resolution, baseline propagation, nested scroll).
- **Native side:** kernel layout via the existing EXLT v1 export
  (exporter at `kernel/src/ffi.rs`; the decoder/cache is
  `packages/exact-renderer/src/layout-export.ts`). Note what exists and what
  does not: `tests/layout-export/` is a small ABI golden, not a frame
  comparator corpus, and the decoder currently rejects any export containing a
  Lottie or Rive node
  (`issues/20260819-exlt-decoder-rejects-lottie-rive-node-types.md`) — which
  must be fixed before the labs go through the harness, or the native leg's
  silence will read as a parity result.
- **Web side:** the same IR rendered through the real web host in Chromium.
  **Coordinate spaces must be normalized before anything is compared:** EXLT
  frames are pre-presenter, root-local values, while
  `getBoundingClientRect()` is viewport-relative and reflects scroll and
  transforms. The common space is the untransformed root-local border box
  (`DOM.getBoxModel` or equivalent), and the harness must also settle node
  correlation, freshness, box edge, scroll, transform, DPR/zoom, and
  font-readiness before its first number means anything. The wasm
  `native-parity-web` profile is Taffy-in-the-browser — a parity instrument,
  not the CSS leg — so it does not substitute for this capture.
- **Controlled fonts:** an embedded metrics-stable test font (Ahem or a
  pinned OFL face) on both sides, so box-math divergence separates from
  text-metric divergence. A second, text-realistic pass with platform
  fonts feeds the Band tolerances.
- **Output:** a generated **divergence catalog**
  (`docs/reports/layout-divergence.generated.md`) — every disagreement,
  classified (profile bug / Taffy patch candidate / browser quirk /
  text-metric / waived), with the waiver ledger. Runs on a quiet fleet
  host per the standing routing rules; registered as a manual check
  first, graduating into a profile once stable.

### 6.3 The empirical program the harness unlocks

- **Set the Band tolerances** from measured cross-browser variance (run the
  web leg on Chromium and WebKit at minimum).
- **Corpus mining:** classify every container in the corpus by which
  operator it needs; measure the actual demand curve for arrangement
  branching and relations before growing the vocabulary further.
  clone-project gap tickets are the running gap detector.
- **Agent bake-off** (the metric most framework teams cannot measure,
  and Exact's marginal author is a model): same target screenshots,
  authored in vocabulary A vs. B by several models, machine-scored by
  contract blocks and visual diff. This is graduation evidence for §8,
  per RFC 0084's "materially simpler on real surfaces" gate.

## 7. One feedback channel, three consumers

LLP 0195 S2's container-size feedback channel is the shared
infrastructure for every stage-1/stage-2 feature. Rulings:

**One fabric, three typed execution classes — not one payload, and not one
scheduler.** The scarce shared resource is the **budget ledger and the
diagnostics/correlation envelope**, not the data and not the timing. The r2
phrasing ("bounded fixed-point inside one layout pass" covering both choice
and measured relations) conflated timings the owners define incompatibly —
LLP 0487 requires kernel choice to settle in the current pass with no
runtime cycle handler, while LLP 0199 forbids same-frame re-entry for
measured relations. The corrected matrix, per consumer:

1. **Synchronous counterfactual evaluation** — choice-layout `fits`
   (LLP 0487 §6): an evaluator capability inside the layout pass, never a
   size observation. It shares the budget ledger, not the channel's
   scheduler or invalidation contract.
2. **Next-frame size observation** — container-relative branching
   (LLP 0195) and `matchWidth`/`matchHeight` (LLP 0199): the channel
   proper, one-directional per frame.
3. **Demand-driven placed geometry** — `alignTo`: coordinates plus
   coordinate-space identity and scroll/ancestor invalidation. **This is
   not servable by ResizeObserver**, which reports neither position-only
   movement nor scrolling; it needs its own bounded root-local geometry
   record with scroll/ancestor generations, or `alignTo` defers out of the
   size-only first release (LLP 0488 carries the record-family split).

Typed observations carry node lifetime, content and border boxes, logical
axes, root-local coordinates, coordinate-space and environment generations,
and source revision.

- **Built once.** LLP 0195 owns and specs it (unchanged); LLP 0488 is its
  implementation spec. It ships with the §4.3-style per-frame budget recorded
  in the lane report before any consumer rides it. Budget the scheduler
  globally **and** cap individual consumers: one 120Hz budget does not stop
  two consumers from cycling each other, so cycle detection, fairness,
  stale-revision rejection, and a deterministic fallback are prerequisites,
  not follow-ups.
- **Consumers, in order:** (1) LLP 0199's measured tier — `matchWidth`/
  `matchHeight`/`alignTo` are already parsed, stored, printed, and diagnosed,
  and the runtime resolver returns `null` into the authored fallback
  (`runtime/layout-intent.ts`), so it is the cheapest first consumer. The
  earlier claim that shipping it costs **zero new authoring surface** was too
  strong: ordinary parsed relation calls do not currently populate a fallback
  (`packages/exact-contract/src/compiler/expr.ts:252`), so WS3 must either add
  and verify fallback authoring end to end, or scope its first release to
  records that inject one out of band. (2) LLP 0195's own container-relative
  branching; (3) choice-layout selection where the island rung (§8.2) applies.
- **Web is the easier host:** the channel is ResizeObserver, coalesced per
  frame. Native needs the kernel→runtime measured-box report and is where the
  budget risk lives. Sequencing may therefore ship the web side first — the
  reverse of Exact's usual order, and fine: it de-risks the semantics while
  ENG-scoped kernel work lands. Two constraints on that inversion: web-first
  is sound for *transport instrumentation*, but publicly enabling relations on
  web while native silently continues down the authored fallback is a
  cross-host behavior split, so the public behavior stays capability-gated
  until the host-neutral record is prototyped on native. And ResizeObserver
  does not itself provide §4.3's one-directional guarantee — it may run
  repeated notification cycles and report undelivered observations — so Exact
  imposes its own next-snapshot boundary rather than inheriting the browser's.
- Family-1 and family-2 channel consumers inherit the same invalidation
  contract: one-directional per frame, coalesced by snapshot revision,
  ≤1 frame latency, no hand-rolled measurement edges (LLP 0199's own rule).
  Family 3 (choice `fits`) inherits the budget ledger and diagnostics
  envelope only — its timing is LLP 0487's own (§7's matrix; same-pass on
  kernel hosts).

## 8. Choice layout: the first algebra operator

### 8.1 Semantics — owned by LLP 0487

**LLP 0487 is normative for choice-layout semantics; this RFC does not restate
them.** That spec — still in Review, with its owner continuing to apply
rulings (r11/r12 landed after this RFC's r2), so "owner-in-Review," never
"frozen" — settled its semantic kernel through a nine-round dual-family
review loop, and an earlier draft of this section paraphrased it in terms that
had already been superseded — first-preference-wins rather than 0487's
first-true `select` under three-valued logic where unknown never selects;
"same children, same relative order" rather than 0487's slots and grouping
templates; DOM/AX order equal to *visual* order rather than 0487 I4's semantic
slot order; and an SSG fallback of "declared default **or**
highest-preference" where 0487 D2 fixes the static default precisely because a
highest-preference candidate can be the one that overflows without JS.

What survives here, as doctrine rather than semantics: choice layout is a
**container-local arrangement layer** — same content, different arrangement —
which is the layer §1.2 argues CSS never had. Its division of labor with
`match size` (policy branching on host size classes, which may change content)
is a teaching distinction the Guide carries, and the two constructs stay
separate: collapsing them would reintroduce conditional rendering into
templates and break the identity and order guarantees 0487 depends on.

This RFC owns only certification, realization, sequencing, and evidence.

**Division of labor with `match size`:** `match size` is policy
branching on viewport/host size classes (may change content); choice
layout is container-local fit (same content, different arrangement).
The Guide teaches this pair explicitly.

### 8.2 The realization ladder (the World-2 discipline, applied)

RFC 0084's rule — lower to CSS only when that preserves specified
behavior, otherwise an Exact-owned evaluator — is applied **per
instance, not per operator**, chosen by the compiler:

- **Rung 1 — pure CSS, zero JS.** When every fit condition is a pure size
  threshold on the container, lower to CSS `@container` queries. Cost on web:
  nothing. SSG/no-JS: works as-is. `@container` is supported in all evergreen
  engines.

  **The faithfulness condition is LLP 0487's A1/A4/A5, not a local
  shorthand.** An earlier draft said the rung is valid when the container's
  inline size is "independent of its children (fill/stretch/fixed width)".
  That test is neither necessary nor sufficient: percentage sizes in
  indefinite ancestors, intrinsic flex/grid sizing, scrollbar gutters,
  aspect-ratio coupling, and imported component context can all restore
  dependency — 0487's frozen counterexample is stretch into a wrap line — and
  a block-axis condition needs *both* axes independent, since CSS has no
  block-only `container-type`. The real predicate is "applying containment is
  a no-op here," proven compositionally and fail-closed, which is what 0487's
  A1 discharge forms and A5 admission witness are. Certification runs at
  composition/link time, where ancestor definiteness is visible, and emits a
  stable proof witness or a stable rejection code. Unknown or dynamic context
  is conservative in the tier-appropriate direction (0487's fail-closed law,
  restated here after the r16 sync): on an **eligibility/A4** question it
  chooses the island — a fully conforming realization; on an **A1/A5**
  question (S-atom axis independence, the admission witness) it **rejects at
  the instantiation site** — an A1/A5 failure is a compile/link **error**
  in 0487, never a silent demotion to rung 2, because an island realizes the
  same broken semantics an unprovable S atom would read.

  **A second certificate is required**, covering topology: candidates sharing
  membership and order does not by itself preserve focus, selection, scroll,
  media, iframe, or custom-element state across a *grouping* change. And
  `@container` toggling attrs on one DOM spine cannot add or remove wrapper
  elements, so grouping-differing templates are not rung-1 realizable at all
  without the union-tree/`display:contents` compilation LLP 0487 §12 leaves
  unfrozen. Rung 1 v1 is therefore the **blessed spine**: candidates differing
  in axis, attrs, and track values only.
- **Rung 2 — Exact-evaluated island.** When conditions involve measured
  content fit (does this row of chips actually fit?), the container
  becomes a bounded evaluator: on native, candidate evaluation inside
  the kernel pass (stage 1); on web, a ResizeObserver-fed island that
  measures candidates and applies the winner — real DOM, source order
  preserved, exactly the shape every serious masonry/virtualization
  library already uses. This is a bounded instance of LLP 0349's
  registered-evaluator model and adopts its budget/diagnostics contract.
  **Rung 2's open hole:** ResizeObserver observes the *committed*
  arrangement; it does not answer whether a non-winning arrangement would fit.
  Before WS4 depends on WS3, v1 must choose among an analytic
  intrinsic-metric model, offscreen/hidden measurement of the **single live
  subtree** (the one escape LLP 0487 M2 permits — cloning a slot identity
  into a second live tree is forbidden there, so "hidden clones" of slot
  content are off the option list; only arrangement *chrome* may ever be
  duplicated), forced layouts, or a deliberately restricted candidate
  subset — these differ materially in latency, state handling, and cost,
  and the choice is a prerequisite for the island, not a detail of it.

- **Degradation (per the standing graceful-degradation rule):** under
  SSG/no-JS, rung 2 server-selects **the declared static default** — LLP 0487
  D2's `kₙ`, not "highest preference," since the highest-preference candidate
  can be exactly the one that overflows under no-JS constraints. The page is
  correct, merely non-adaptive until JS arrives, and rung-1 instances never
  degrade at all.

### 8.3 Graduation gates

Choice layout graduates from the experimental grammar namespace only
with: deterministic cross-host selection fixtures in the §6 harness;
Semantic parity including AX/DOM order across candidates, and Decision-tier
agreement per §4; the §7 budget where
rung 2 applies; diagnostics ("which candidate, which condition bound" —
§9); and the §6.3 evidence — corpus rewrites of real `when`-ladder
surfaces plus the agent bake-off. The evidence parameters are
**pre-registered before any implementation evidence is collected**, not
chosen after: N ≥ 5 corpus surfaces drawn before rewriting begins; a
held-out surface set never used during vocabulary design; ≥ 3 models ×
≥ 3 attempts per formulation with a fixed retry rule; success scored by
contract blocks and visual diff against the same target screenshots; and
a pre-declared effect threshold (choice must beat the `match size`
formulation on first-try success by the registered margin, not merely tie).
RFC 0084 remains the owner of the graduation decision itself.

## 9. Explainability

Every frame must be able to answer *which operator placed me and which
constraint bound*. Extend `exact_layout` diagnostics with layout
provenance: the owning operator, the binding constraint, the selected
candidate (choice layout), and the relation that resolved (LLP 0199).
This is cheap where Exact owns evaluation, and it is what makes Design
Mode's "break this relationship" affordance (LLP 0199/0200) and agent
debugging real. On web, provenance is emitted from the lowering (the
mirror knows which attrs it wrote and why) even though the browser owns
the solve.

One scope narrowing, reconciling this section with LLP 0487 §9 D4's
static-output exemption: purely static output — the zero-JS CSS rung
before activation, server render, print — carries **no production
reporting obligation**, exactly as 0487 rules; requiring production JS
solely for diagnostics would defeat the rung's point. The provenance
claim above binds wherever a runtime exists, and the zero-JS rung is
covered instead by **test-only instrumentation**: the §6 harness evaluates
the same first-true predicate over the same declared box and asserts the
CSS encoding's winner against `select`'s, so the rung is verified without
being made observable in production. (Recorded as the proposed
0487-consistent resolution; 0487 owns the exemption.)

## 10. Sequencing

Four workstreams. WS1 *capture* and WS2 *inventory* can proceed in parallel,
but taxonomy, profile finalization, and band-setting are mutually dependent —
the earlier claim that WS1 and WS2 are simply "independent and cheap"
overstated it. Classification cannot precede measurement, and a profile whose
portability states are asserted before the harness reports Taffy-vs-Chromium
results is how every engine disagreement becomes a JSON exception.

- **WS1 — Instruments** (§6) — *landed 2026-08-20 in its core* (merge
  `460f9f840`, `agent/0486-ws1-harness`, mid-fold — this row updated by the
  orchestrator on landing): the dual-engine harness exists — native leg =
  production EXLT v1 through kernel/Taffy; web leg = the production Contract
  web host in real Chromium AND WebKit; controlled-font and realistic modes;
  per-leg production-IR SHA-256 parity receipts; the generated divergence
  catalog (`docs/reports/layout-divergence.generated.{md,json}`); and
  proposed Band floors from measured Chromium-vs-WebKit disagreement, with
  the generated-sibling recommendation this RFC leaned to. Remaining WS1:
  ratify the proposed Band floors and land the Band artifact beside the
  profile (`bandArtifact.status` flips from `not-built`), the canonical
  decision-trace records for the Decision tier, and upgrading §6.1's web
  leg to rect-derived frames — part of this lane, not a separate wish.
- **WS2 — Profile authority** (§5) — *landed, with hardening owed*: the
  authority file, generated web defaults, resolved-behavior gate,
  `layout-profile-parity`, and the contracts row all exist. Remaining WS2
  obligations: amend the contracts row's Frame sentence to §4's ruling and
  add the Decision tier; the designated Frame-pair table and the governed
  metric-stack-fingerprint field list in the profile (§4); the logical
  `start`/`end` divergence entry — its widen-or-translate repair is
  **DECIDED: translate-via-table** (RFC 0491 r16's joint 0486 amendment,
  2026-08-20; the WS2 obligation is discharged by that ruling, and 0491
  Phase 2's prerequisite on it is satisfied) — (§5.1); the waiver-schema migration and drift-vs-conformance gate naming
  (§4); the 0487 A5 **stable-gutter pin** — the profile records
  `scrollAncestorGutter` as `target-unimplemented`, and 0487 r15+ requires a
  **present** pin for A5 discharge — until the pin flips to a present fact
  the default `scroll(column(choice))` composition **fails closed**
  (`choice_feedback_cycle`; the corpus's fixture 05 two-phase obligation) —
  so the pin needs a named host owner per platform, and the reservation
  actually implemented, before that composition admits and WS4's rung 1 can
  certify it;
  the fail-closed completeness join and per-claim
  executable fixtures (§5.2); `style=` scoping (§5.2); fold the §4 tier
  ruling into RFC 0084 (resolving its open question); keep the §5.1
  inventory's portability states revisable as the WS1 harness reports.
- **WS3 — The channel** (§7): LLP 0195 S2 per its own spec + budget,
  implementation-spec'd in LLP 0488, prototyped on both web and native before
  the record is frozen; then flip LLP 0199's measured resolvers from `null`
  to live.
- **WS4 — Choice layout** (§8): experimental namespace. **Its real gate is
  the LLP 0487 fixture corpus, not WS3** — rung-1 selection needs 0487's
  classifier and its counterexample corpus, which is why that corpus is being
  built before the operator exists
  (`issues/20260819-llp0487-choice-fixture-corpus.md`). Rung 1 first, as the
  blessed spine (§8.2); rung 2 behind WS3 *and* an answered candidate-
  measurement contract; graduation per §8.3, which is RFC 0084's call.

**Papercut status (2026-08-20):** all three 2026-08-19 tickets are
**closed** — the `exact new` scaffold paved-road bypass (resolved with real
scaffold builds and a registered gate), the web agent inspector's
`computeApproximateFrames` (its constraint survives in §6.1's rect-derived
bar), and the EXLT decoder's Lottie/Rive node-type ceiling (WS1's native
leg is unblocked). They are cited as history, not as open work.

**The keep list:** the LLP 0288 evidence gates (`contract-ssg-no-js`,
hydration corpus, axe delta, bundle budget) are the moat against
Flutter-web-ification. Every new operator's web realization passes
through them; that pressure is exactly why the ladder's rung 1 exists.

## 11. Hard Parts

- Setting Band tolerances tight enough to catch bugs and loose enough
  to survive browser engines' own disagreements.
- The native side of the feedback channel meeting its per-frame budget
  on the runtime thread at 120Hz targets.
- Rung-2 islands under virtualization (RFC 0088): candidate measurement
  must compose with estimated-size windowing without thrash.
- DOM moves during candidate switches: element state mostly survives,
  but focus can drop and iframe/video state resets — the identity story
  needs explicit rules for focus restoration and a "sticky" escape for
  stateful embeds.
- Keeping the profile authority genuinely small; the failure mode is it
  growing into a parallel CSS spec.

## 12. Revision history

**r3 (2026-08-20)** — program super-refine round 1 (codex gpt-5.6-sol@ultra +
grok-4.6@xhigh, cluster B; artifacts under `llp/reviews/0486-*`). Both
families NOT READY, convergent on instrument staleness: the RFC still
described the WS2 profile authority, `layout-profile-parity`, and the
registered `contract-cross-host-parity` lower bound as unbuilt, and cited
three papercut tickets that had closed. All landed-state claims verified
against the tree before restating. Material folds beyond restatement: the
Decision-tier equal-input precondition and the Semantic-row narrowing
(codex); the machine-actionable Frame designation and the contracts-row
amendment obligation (both families); the three-typed-execution-classes
matrix replacing the single stage taxonomy, with `alignTo` moved off
ResizeObserver (codex + grok's typed observations); profile fail-closed
completeness and `style=` scoping as WS2 obligations (codex); the
hidden-clone strike per 0487 M2 (grok); §9's static-output narrowing
reconciled with 0487 D4 (codex); pre-registered §8.3 evidence parameters
(codex); three stale line citations fixed (grok).

**r2 (2026-08-19)** — first review round (codex gpt-5.6-sol@ultra with repo
access, grok-4.6@xhigh; artifacts under `llp/reviews/0486-*`). Both families
returned NOT READY, convergently. Every reviewer claim was independently
verified against the tree before adoption; the material corrections were that
several load-bearing statements were **false in the current repository**, and
lanes were already building against them:

- §5.2 named a Rust function that is TypeScript, so the proposed gate would
  have gone green over divergent behavior;
- §5.1 called margins excluded forever (Contract exposes them) and grid
  `fr`/`minmax`/`repeat` portable (native is an equal-fr count model);
- §5.3's single-rounding rule described a target, not the current two-stage
  reality;
- §4's tier names collided with frozen LLP 0487 §7, and Frame parity was
  scoped by "has a kernel" rather than by normalized inputs;
- §6.1 mixed a semantic gate with a per-box geometry allowlist that cannot
  survive contact with real fixture volume;
- §8.1 restated choice-layout semantics that LLP 0487 had already frozen
  differently, including an SSG rule 0487 deliberately closed;
- §6.2's `native-parity-web` quote was misattributed and its two capture legs
  use incompatible coordinate spaces.

Added: the Decision parity tier (both families found it independently, from
different directions); typed observations in §7; the blessed-spine scoping of
rung 1; §10's dependency correction. Three papercut tickets filed.

## 13. Open Questions

- Catalog vs. protocol (RFC 0084's other open question): this RFC's
  stance is **closed catalog first, public `measure(constraints)`
  protocol later or never** — a public measure protocol on web either
  forces the island realization on everyone or is unimplementable atop
  CSS. Does any near-term surface actually need the protocol?
- Does grid line-naming cover enough of the `alignTo` demand (per §6.3
  corpus mining) to defer cross-container `alignTo` indefinitely?
- Choice-layout selection changes and motion (LLP 0313): what animates
  when the winning candidate changes — nothing, opt-in transitions, or
  default FLIP on the moved children?
- How visible should stage cost be in the grammar — should a rung-2
  choice container *look* more expensive than a `row` (explicit keyword,
  lint, or nothing)?
- Do `match size` and choice layout remain two constructs long-term, or
  does one subsume the other once container-relative data exists?
- Where do Band tolerances live — a generated sibling of the profile
  authority (the r2 leaning, since they are measured output rather than
  pinned law), or per-fixture
  annotations?
