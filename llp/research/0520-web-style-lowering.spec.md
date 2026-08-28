# LLP 0520: Web Style Lowering

**Type:** Spec
**Status:** Accepted (Charlie Cheever, 2026-08-21 — adopted as the LLP 0485 §12.4.7 / LLP 0483 §8 item 7 gate artifact; Track W design and the Track R web-runner capture may start against it)
**Systems:** contract web host, facet-core, plan runner (web), SSR
**Author:** Exact agents with Charlie Cheever
**Date:** 2026-08-20
**Revised:** 2026-08-26 (§6.4 gains the RFC 0540 virtual-visibility writer-census row plus a dated three-part fold: `content-visibility` / `contain-intrinsic-size` / `overflow-anchor` admitted as §5.6 grammar rows under a profile revision with the CSS `auto` keyword kept in both positions; **two** registered `--ex-h-*` retained-size variables (LLP 0543 §13's spec pin of 0540 §1.7's singular wording — the two-axis `contain-intrinsic-size` form needs width and height) under §6.3's sentinel discipline, where `removeProperty()` would expose an ancestor row's retained size; one census row under the one-writer-one-row rule. Executes 0504 §3 row 69; delivery post-window per 0540 §8.6. 0506 D1(d) burn-down lane.) 2026-08-25 (r6.3 — §10's proposed `styles` segment re-targeted from "≥ v1.6" to **≥ v1.7**: plan-format v1.6 landed 2026-08-25 @2d9995f89 as the enum-roster digest (LLP 0485 Amendment 4), taking the number. Deciding source: 0504 §3 row 29 as updated at r41 ("0520's `styles` segment re-targets ≥ v1.7"); mechanical citation alignment under LLP 0551 §2.3, same class as r6.2; wire shape and compatibility semantics unchanged) 2026-08-23 (r6.2 — §10's proposed `styles` segment re-targeted from "v1.4" to **≥ v1.6**: plan-format v1.4 (identity stability) and v1.5 (M1 WP2 host interface) landed with other content after this spec was drafted against v1.3. Deciding source: 0504 §3 row 29 ("0520's `styles` segment must land as ≥ v1.6") / LLP 0551 finding 28, executed under 0551 §2.3. §10.1 carries the re-target note and document-wide reading rule; wire shape and compatibility semantics unchanged) 2026-08-21 (r6.1 — citation alignment to the landed v1.4 identity tuples; no normative change)
**Related:** LLP 0485, LLP 0483, LLP 0497, LLP 0402, LLP 0386, LLP 0210, LLP 0288, LLP 0304, LLP 0157, LLP 0084

> **Revision status:** r5 closes the round-4 style/plan, pre-paint handoff,
> framing, and nested-scope residue. The accepted
> architecture and the round-2/round-3-resolved inheritance split, v1.4 wire
> shape, census, RecipeIR forms, cascade, recipe gate, production seam, and
> classifier decisions remain unchanged.

## 0. Summary

This is the web style-lowering design required by LLP 0485 §12.4.7 and LLP 0483 §8 item 7. One compiler-owned `WebStyleBundle` serves the development DOM mirror, the current production Contract direct host, SSR, and the wasm plan runner. A shared style pipeline produces:

1. target-neutral `ResolvedStyleIR` after Contract winner resolution;
2. web-specific, canonical `WebStyleIR` after host projection;
3. real CSS plus a `WebStyleManifest`; and
4. class/custom-property references for either the current module runtime or the proposed plan-format `styles` segment (targeting ≥ v1.7; §10.1).

The fixed layered order is:

```css
@layer exact.reset, exact.tokens, exact.facet-recipes,
  exact.app-static, exact.app-dynamic, exact.overrides;
```

The Exact-owned forced-colors/reduced-motion sheet is deliberately **unlayered** and follows every Exact layered asset. `@page` is also unlayered and lives in a separately activated route sheet. This avoids the CSS rule that every normal unlayered author declaration outranks every normal layered author declaration. Exact uses no `!important`.

Static declarations become content-addressed classes. A runtime change either selects a finite emitted class set or writes a typed value to an emitted custom-property binding. Runtime-generated selectors, runtime-generated rules, arbitrary CSS strings, and ordinary inline presentation on author-owned nodes are outside production admission.

The plan change in §10 is a proposal to the owner of the authority in `packages/exact-contract/plan/format-schema.json`: a new compatible minor — **targeting ≥ v1.7**, drafted as "v1.4" against the then-landed v1.3 (§10.1's re-target note) — not a reinterpretation of the landed schema.

## 1. Scope and gate effect

### 1.1 Owned here

This specification owns:

- static web extraction from built-ins, Contract style inputs, admitted RecipeIR, themes, transitions, conditions, and print records;
- canonical rule/class/CSS/manifest identity;
- typed custom-property binding and removal semantics;
- cascade order and selector specificity;
- projection of LLP 0485 §3.4 resolved themes;
- class-first DOM ownership in both live Contract writers;
- SSR delivery, stylesheet custody, and resumption adoption;
- the proposed styles-segment plan rows (≥ v1.7; §10.1) required by the wasm web runner; and
- verification, rollout, budgets, and falsifiers.

The non-Facet Track W implementation may start from this design. Facet CSS extraction and every Facet-bearing F6 capture additionally require the LLP 0497 F-A RecipeIR gate in §4.3.

### 1.2 Not reopened

This specification does not reopen Contract as the production web target on real DOM (LLP 0288), the wasm build of the Rust plan engine as the eventual web runner (LLP 0483/0485 as amended), `@exact/facet-core` as styling truth, LLP 0485 §3.4 theme-table identity, or LLP 0485 §12.5 first-frame ownership.

### 1.3 Conformance language

`MUST`, `MUST NOT`, `SHOULD`, `SHOULD NOT`, and `MAY` are normative. A stylesheet alone is not conformance: CSS bytes, manifest bytes, DOM binding behavior, SSR custody, and any plan rows must derive from the same canonical model.

## 2. Current-state evidence and the seam to change

### 2.1 Theme and recipe truth

`ThemeResolveInputs` contains `scheme`, `reducedMotion`, continuous `textScale`, `contrast`, `platform`, explicit `sizeClass`, `sizeClasses`, and continuous `viewportWidth` (`packages/exact-facet-core/src/theme-definition.ts`). `resolveThemeCascade()` resolves definition/definition overrides, then app → scope → agent token overrides, then restamps size class and applies `applyMotionPreference()` (`theme-cascade.ts`). Those post-merge transforms are not a CSS cascade rung.

The current recipes demonstrate why arbitrary TS evaluation is not an extractor:

- Button/Card change declaration presence with `undefined`, call recipe-parameter and density helpers, and return `TransitionMap` objects.
- Toggle branches on resolved scheme and reduced motion.
- Input performs continuous `rows` arithmetic and rounded token arithmetic.
- focus and hover helpers mix colors; `applyShadow()` emits one web `boxShadow` plus native shadow fields.
- `lowerDomStyle()` rewrites gradient `backgroundColor` to `backgroundImage` and lowers transition/transform objects.

The generated token schema excludes spring/preset transition objects from bindable scalar leaves, and the current projection-manifest check deliberately rejects a continuous producer-local style input (`FacetProgress`). These facts require the RecipeIR gate in §4.3.

### 2.2 Contract style assembly

The runtime merge is lowering defaults → `style=` → authored style attrs (`packages/exact-contract/src/runtime/view.ts`). Literal detection currently recognizes only string/number/boolean/null; `$token` is `Expr.kind === "token"` and therefore currently takes the dynamic path even though its semantics are statically recognizable. The new classifier handles token expressions explicitly and preserves the existing winner order.

Structural `match size` reads the live Facet size-class source. It remains structural. Style conditions use the coordinated size-class/environment protocol in §8.5 or an independent typed viewport query; they do not invent another breakpoint table.

### 2.3 The two live DOM writers

There are two writer implementations to migrate:

- `packages/exact-renderer/src/dom-mirror.ts` backs `installDomMirror()` and its compatibility `installContractWebHost()` export. It clears every attribute, writes `el.style`, and uses `styleDeclarationsMatch()` for inline hydration correspondence.
- Production Vite builds rewrite `@exact/renderer/web-host` to `@exact/renderer/web-host-production` in `packages/exact-devtools/src/contract-web.ts`. That module calls `installDirectContractWebHost()` from `web-host-direct.ts`. Its `applyDescription()` removes stale attributes and replaces/removes the whole `style` attribute.

Production in Phase B therefore means `web-host-direct.ts`, not the mirror export. Both writers MUST consume one shared class-first module, provisionally `packages/exact-renderer/src/web-style-dom-writer.ts`. Neither may retain a private CSS emitter or whole-attribute presentation path in `css` mode.

### 2.4 Existing static scheme and hydration behavior

`static-scheme.ts` stamps `data-scheme`/`data-scheme-preference`, writes root `style.colorScheme`, and on a system OS flip updates themed image sources while CSS media rules change other pixels. `contract-static-theme.ts` independently diffs light/dark inline declarations and hoists them to sequential `--sr-<n>` variables. Both mechanisms are replaced by the theme records and environment transaction in §8; themed image byte selection remains separate.

The mirror's `styleDeclarationsMatch()` compares complete inline declarations. The direct host's total hydration preflight adopts server presentation for the custody turn without class/stylesheet correspondence. Section 11 names the LLP 0386 correspondence amendment required for both.

### 2.5 Landed plan authority

`packages/exact-contract/plan/format-schema.json` is the hand-authored format authority and currently declares format/minimum-reader **1.3**, with generated TypeScript/Rust readers and binary goldens. Section 10 follows its optional-segment, framed-column, range, bounds, and SHA-256 conventions.

## 3. Canonical style pipeline and bundle

### 3.1 Pipeline

The compiler MUST execute these stages in order:

1. parse/type-check Contract and resolve imports;
2. resolve existing style precedence symbolically per property and condition;
3. produce `ResolvedStyleIR` with Exact property ids, typed values, provenance, temporal ownership, and condition/source-order metadata;
4. run the web projector that today lives across `describeDomElement()`, `lowerDomStyle()`, and renderer defaults, producing final CSS property ids and typed values;
5. classify every value by §6 and partition declaration blocks by §5.5;
6. emit canonical `WebStyleIR`, CSS, and `WebStyleManifest` through the single emitter in §5.7;
7. attach module-runtime refs or proposed styles-segment plan refs (§10); and
8. emit the first-frame/resumption tuple.

CSS emission, SSR comparison, and live DOM comparison consume the same projected records. The DOM mirror is a consumer, never an extractor.

### 3.2 Bundle artifacts

One logical `WebStyleBundle` contains:

| Artifact | Contents | Identity |
| --- | --- | --- |
| `WebStyleIR` | rules, ranks, conditions, classes, vars, token/derived refs, page records | canonical binary bytes |
| CSS assets | exact emitted bytes in logical order | SHA-256 per asset |
| environment table | canonical admitted family/environment → resolved-theme records | `environmentTableDigest` |
| pre-paint boot program | versioned executable bytes that select from that exact table | SHA-256 content digest |
| `WebStyleManifestCore` | canonical ordered identities and joins | `styleBundleDigest` |
| JSON manifest | diagnostic projection of the core | core digest must verify |
| loci sidecar | rule/binding → source/recipe locus | auxiliary; excluded from class identity |

The logical asset order is:

1. layer prologue/base reset;
2. theme records;
3. route Facet/app rules;
4. unlayered Exact accessibility rules; and
5. the active route's unlayered page sheet, when any.

Assets may be transport-coalesced only when this logical ordering and independent page activation remain equivalent.

The pre-paint boot program is a separately identified delivery asset, not a CSS role. Its verified bytes execute before any author-owned element can paint. Its digest and the environment-table digest enter `WebStyleManifestCore` under §5.7 and the delivery/resumption tuple under §10.5.

### 3.3 CSS remains outside `.eplan`

CSS is a cacheable web artifact beside the route plan/module. Plan bytes contain identities and bindings, not CSS text. The deployment manifest joins the artifacts. Repackaging cannot change class identity.

## 4. Static extraction

### 4.1 Extraction classes

| Input | Output | Runtime work |
| --- | --- | --- |
| built-in/host default | `exact.reset` class | ensure class |
| final resolved token record | `exact.tokens` custom properties | select admitted digest |
| RecipeIR rule | `exact.facet-recipes` class/derived ref | finite selection or typed var |
| literal/constant Contract value | static app class | ensure class |
| `$token` or recognized `theme.*` path | `var(--ex-t-*)` | none on theme flip |
| admitted token algebra | canonical `calc()` or derived-theme ref | none on theme flip |
| typed value-only expression | declaration consuming `--ex-v-*` | set local var |
| finite presence/shape arm | emitted class set per arm | select set |
| temporal/motion sink | one declaration consuming typed temporal var | set var through its owner |
| independent viewport/print/forced-colors arm | typed condition rule | browser evaluates |

Static means build-derivable from literals, identities, and closed symbolic references. A value observed during SSR is not thereby static.

### 4.2 Canonical property projection

Before classification, the shared web projector MUST:

- expand `paddingHorizontal`/`paddingVertical`/margin aliases;
- add border style when a width requires it;
- lower typed `TransitionMap` and transform arrays;
- compose shadow inputs into `box-shadow` while retaining native fields only outside web IR;
- rewrite typed gradients from `backgroundColor` to `backgroundImage`;
- project layout-profile defaults, graphics/pointer `touch-action`, backdrop/material, line-clamp/text, and semantic tag defaults; and
- record exclusive temporal owners for transform, opacity, shadow, anchored position, and pager/presence channels.

No runtime writer may repeat those static expansions after CSS promotion.

### 4.3 Facet gate: RecipeIR, not TS interpretation

The decision is a prerequisite gate, not a new symbolic interpreter for arbitrary TypeScript:

1. LLP 0497 F-A RecipeIR MUST be the authority before a Facet recipe enters `css` mode.
2. The current TypeScript recipes remain a conformance oracle during F-A migration, not extraction input.
3. A public Facet component without complete RecipeIR is `web-style-recipe-ir-required` and is excluded from Facet CSS/F6 admission. It may run only in `legacy-inline`/`compare`.
4. Track W may implement built-ins, raw Contract attrs, typed `styles` blocks, variables, themes, SSR, and non-recipe fixtures before F-A.

The RecipeIR consumer is mechanical and consumes only the forms RFC 0402 §3 actually defines:

1. an existing Contract `Expr` enters the §6.1 classifier;
2. the RecipeIR `paramRead` extension classifies its declared override value and fallback expression together: a closed bool/enum/option domain may select a complete class map, a compatible typed scalar becomes a value binding, and incompatible shape/presence branches reject;
3. a registry intrinsic classifies every key in its declared complete produced-key set through the same property classifier; a theme-only result may become a `DerivedThemeRef`, a typed runtime scalar may become a value binding, and an opaque or undeclared result rejects; and
4. explicit RecipeIR `unset` removes the accumulated key. It becomes declaration absence or a complete finite presence arm; it is never serialized as the CSS keyword `unset`.

For each closed recipe row the consumer enumerates declared finite axes, checks complete output-property closure, and emits the resulting class/var records. A recipe output that is missing, opaque, ambient, nondeterministic, or not covered by its declared finite/continuous input classification fails closed. No additional value-expression IR is introduced.

Theme-derived recipe computations that cannot be represented by the admitted `calc()` algebra use a `DerivedThemeRef` supplied by RecipeIR. Its name is content-addressed from the typed expression, for example `--ex-d-<hash>`, and every admitted `ThemeCssRecord` contains its evaluated value. Color mixing, density rounding, concentric radii, and other registered pure functions may enter only through this closed evaluator registry. The expression identity, not any resolved theme value, names the class or derived property. A theme-value-only edit therefore changes theme assets/digests, not app/Facet class names or plan class rows.

### 4.4 Contract `style=` and calls

A `style=` object is admitted only when its complete key set and winner order are statically closed and every value has a §6 disposition. Spreads must have statically known keys. A call is admitted only when it is a compiler intrinsic, a registered typed value constructor, or RecipeIR. Arbitrary imported style functions, reflection, computed keys, post-construction mutation, and raw CSS strings are rejected in `css` mode.

## 5. Byte-decidable identity and serialization

### 5.1 Machine-readable WebStyle authority

`packages/exact-contract/webstyle/webstyle-schema.json` is the sole hand-authored, machine-readable authority for `webStyleFormatVersion` 1.0. This LLP decides the semantic rules and assignment policy; the schema contains the exhaustive numeric tables and byte templates delivered with Phase A. Prose examples, generated TypeScript, generated reference documentation, tests, and golden files are consumers, never competing authorities.

The schema MUST contain:

- every enum, bitmask, opcode, and id registry used by WebStyle, including at minimum `scopeKind`, `propertyId`, `serializerId`, `syntaxId`, `unitId`, `stateId`, `operatorId`, every condition/value opcode, unset policy, asset/sheet role, and merge-source-rank values;
- one complete grammar row for every property admitted by the v1 target profiles, including its final CSS property spelling, serializer, accepted typed value union, ranges, keyword spellings, unit set, fallback legality, and projector ownership;
- the exact framed layouts for `RuleKey`, conditions, selector suffixes, every variable-name preimage, `WebStyleIR`, the environment table, the target-feature-profile preimage, the pre-paint program IR/instruction stream, and `WebStyleManifestCore`;
- exact CSS printer templates for qualified rules, theme records, `@property`, `@layer`, every admitted condition wrapper, and `@page`, including whitespace, indentation, escaping, terminators, ordering, and final-newline behavior; and
- the exact selector template in §8.3, the exact required `calc` folds in §5.6,
  and the pre-paint program's bounded grammar, instruction encoding, canonical
  ES2020 source template, configuration embedding, and final-byte rules.

Registry assignment is explicit, not array-order-derived. **Zero has no global
sentinel rule.** A registry may explicitly assign zero any one canonical
meaning; that assignment is valid and identity-bearing. Thus v1 deliberately
keeps `ruleKind.qualified=0`, placement `reset=0`, condition opcode `true=0`,
and `serializerId.none=0`. An unassigned zero rejects exactly like any other
unknown value. Within a format major, an assigned numeric value—including
zero—and its canonical serialized meaning never change or get reused; removal
leaves a tombstone. New values append with a format/profile revision, and
aliases resolve before serialization to one existing canonical id. Canonical
table order is ascending numeric id, while semantic order such as merge
precedence comes from the registry's explicit rank values. Missing rows,
duplicate ids/names, unreferenced property grammars, or a property grammar
referring to an unavailable serializer/unit/syntax row fail schema generation.

The LLP owns every outer framed-record field tag; the schema owns exhaustive
row content, widths, and generated spellings under those tags. The closed v1
tag assignments are:

| Record | ASCII magic | Field tags in order |
| --- | --- | --- |
| rule key | `EXWR` | `1 version`, `2 profileDigest`, `3 ruleKind`, `4 placement`, `5 cascadeRank`, `6 condition`, `7 selectorSuffix`, `8 scopeKind`, `9 declarations` |
| target profile | `EXWP` | `1 version`, `2 profileId`, `3 properties`, `4 serializers`, `5 syntaxes`, `6 units`, `7 scopeKinds`, `8 conditions`, `9 states`, `10 operators`, `11 featureBits`, `12 colorGradient`, `13 page`, `14 transitionLowerings`, `15 parameterizedValues` |
| WebStyle IR | `EXWI` | `1 version`, `2 profileDigest`, `3 rules`, `4 variables`, `5 tokenRefs`, `6 derivedRefs`, `7 themes`, `8 environmentTable`, `9 pages`, `10 assetPartitions`, `11 bootProgramDigest` |
| token-name preimage | `EXWT` | `1 version`, `2 tokenPath`, `3 valueType`, `4 serializer`, `5 tokenSchemaDigest` |
| runtime-var preimage | `EXWV` | `1 version`, `2 profileDigest`, `3 sourceModuleId`, `4 bindingSiteId`, `5 property`, `6 serializer`, `7 scopeKind` |
| derived-name preimage | `EXWD` | `1 version`, `2 profileDigest`, `3 expression`, `4 property`, `5 resultType`, `6 serializer` |
| host-var preimage | `EXWH` | `1 version`, `2 profileDigest`, `3 owner`, `4 structuralNode`, `5 property`, `6 serializer`, `7 scopeKind`, `8 channel` |
| theme-family preimage | `EXWY` | `1 version`, `2 definitionOrCustomBase`, `3 definitionOverrides`, `4 appOverrides`, `5 scopeOverrides`, `6 agentOverride`, `7 sizeClasses` |
| theme CSS record | `EXWQ` | `1 familyId`, `2 familyDigest`, `3 resolvedDigest`, `4 tokenSchemaDigest`, `5 environment`, `6 tokens`, `7 derived`, `8 colorScheme` |
| environment table | `EXWE` | `1 version`, `2 profileDigest`, `3 tokenSchemaDigest`, `4 families`, `5 records` |
| environment-family row | `EXWF` | `1 familyDigest`, `2 familyId`, `3 compactBounds`, `4 regularBounds`, `5 expandedBounds` |
| environment-result row | `EXWJ` | `1 familyDigest`, `2 scheme`, `3 reducedMotion`, `4 textScale`, `5 contrast`, `6 sizeClass`, `7 resolvedDigest` |
| pre-paint program IR | `EXWB` | `1 version`, `2 profileDigest`, `3 environmentTableDigest`, `4 environmentTableBytes`, `5 storageKey`, `6 legacyStorageKeys`, `7 defaultTextScale`, `8 instructions`, `9 toggleTestIdPrefix (optional)` |
| manifest core | `EXWS` | tags `1`–`13` exactly as §5.7 assigns |

Adding a field uses the next unused tag and a format/profile revision; deletion leaves a tombstone. Renumbering or another `EXW*` outer record requires an LLP amendment.
When vector members, `EXWR`, `EXWT`, `EXWV`, `EXWD`, `EXWH`, `EXWY`, `EXWQ`, `EXWF`, and `EXWJ` are complete magic-prefixed ascending-tag records. They are the closed framed v1 member list; every other member is length-delimited positional. A later framed kind first requires LLP-assigned tags.

The schema MUST contain exactly one named production profile instance for v1,
`exact-web-css-v1`. It contains explicit closed lists—never wildcards or
"all registered" inheritance—for admitted properties, conditions, states,
operators, serializers, syntaxes, units, scope kinds, feature bits, and every
parameterized feature value. Schema closure and the byte golden for this
instance are a Phase A gate; experimental profiles do not satisfy production,
Facet, or F6 admission.

`packages/exact-contract/scripts/generate-webstyle-schema.mjs` consumes the schema and generates both `packages/exact-contract/src/web-style/webstyle-schema.generated.ts` (the emitter/reference-evaluator tables) and `packages/exact-contract/webstyle/webstyle-schema.reference.md` (the human-readable projection), plus the byte-golden manifest. The Phase A `web-style-schema-parity` check runs that generator with `--check`, validates registry closure, and proves the committed outputs/goldens fresh, following the same authority → generated readers/tables → `--check` pattern as `plan/format-schema.json`. The emitter MUST import the generated tables; hand-maintained switch copies are forbidden. Any identity-bearing schema change updates `webStyleFormatVersion` or a target profile as the schema's versioning rules require.

### 5.2 Primitive encoding

All canonical binary records use little-endian integers. A framed field is:

```text
tag:u8 | byteLength:u32 | payload:byteLength
```

Fields occur once in ascending tag order; unknown tags are invalid for `webStyleFormatVersion` 1.0. Counts are `u32`; enum discriminants are `u8` or `u16` as declared. Strings are a `u32` byte length followed by strict UTF-8. Lone surrogates and invalid UTF-8 are rejected. Strings are not Unicode-normalized because normalization can change quoted CSS semantics; keyword grammars perform their own ASCII case normalization.

Every numeric source entering style IR is an IEEE-754 binary64. NaN and positive/negative infinity are `web-style-exceptional-number`; `-0` becomes `0`; property-specific ranges are then enforced. `f64LE` means those normalized raw binary64 bits reinterpreted as `u64` and written little-endian. Decimal output uses Ulf Adams's PLDI-2018 Ryū `d2s` shortest-round-trip algorithm. The emitter lowercases `E`, removes an exponent `+` and redundant exponent zeroes, and chooses the shorter of plain/scientific spellings from the Ryū decimal pair (plain on a tie). Golden fixtures pin boundary/subnormal/rounding cases.

### 5.3 Canonical `RuleKey`

The record begins with ASCII `EXWR`, then these framed fields:

| Tag | Payload |
| --- | --- |
| 1 | `webStyleFormatVersion` as `major:u16, minor:u16` (v1 = 1,0) |
| 2 | `targetFeatureProfileDigest`, 32 raw SHA-256 bytes |
| 3 | rule kind `u8` |
| 4 | layer/sheet id `u8` |
| 5 | `cascadeRank`: `partCount:u16` then `partCount × u32` |
| 6 | canonical condition IR bytes |
| 7 | canonical selector-suffix IR bytes |
| 8 | scope kind `u8` |
| 9 | canonical declaration block |

A v1 rule kind is `0=qualified`, `1=property-registration`, or `2=page`. Placement ids are `0=reset`, `1=tokens`, `2=facet-recipes`, `3=app-static`, `4=app-dynamic`, `5=overrides`, `128=unlayered-accessibility`, and `129=unlayered-page`. Every other v1 discriminant rejects.

A qualified/page declaration block is `count:u16` followed by entries sorted by `propertyId`: `propertyId:u16, serializerId:u16, valueLength:u32, valueBytes`; the page kind accepts only the §9.1 page-property registry. A property-registration block is `namePreimageLength:u32, namePreimage, syntaxId:u16, inherits:u8, hasInitial:u8`, followed when present by `initialLength:u32, initialValueBytes`. Duplicate property ids are invalid. Paths, offsets, component names, timestamps, route traversal order, and minifier versions never enter the key.

For a qualified rule, the class digest is the first 16 bytes of the standard 32-byte BLAKE3 output over the complete `RuleKey`. “First” means offsets 0–15 in BLAKE3 output order. It is encoded with unpadded lowercase RFC 4648 Base32 alphabet `abcdefghijklmnopqrstuvwxyz234567`, most-significant bits first; the final character's two unused low bits are zero:

```text
exr_<26 base32 characters>
```

The canonical rule digest stored in manifests and used to validate plan class refs is SHA-256 over `ExactWebRuleKeyV1\0 || RuleKey`.

### 5.4 Condition and selector IR

Condition IR is a recursive `opcode:u8, payloadLength:u32, payload` record. V1 opcodes are fixed: `0=true`, `1=false`, `2=all`, `3=any`, `4=not-atom`, `16=viewport-min-px`, `17=viewport-max-px`, `18=orientation`, `19=environment-scheme`, `20=environment-reduced-motion`, `21=environment-contrast`, `22=environment-size-class`, `23=forced-colors`, `24=hover-capability`, `25=pointer-capability`, and `26=medium`. `all`/`any` payloads are `childCount:u16` plus framed children; `not-atom` contains exactly one framed atom; viewport payload is one normalized `f64LE`; every other atom payload is its registry `u8` enum. Unknown opcodes or payload widths reject.

Normalization is total:

1. push negation to typed atoms and represent a remaining negative atom only as `not-atom`;
2. flatten nested equal boolean operators;
3. normalize numeric endpoints with §5.2;
4. intersect same-axis range atoms and constant-fold contradictions/tautologies;
5. sort children by canonical bytes and remove duplicates; and
6. reject an atom not admitted by the target feature profile.

Equivalent commutative AST permutations therefore have equal condition bytes. Authored arm order is deliberately **not** erased; it lives in `cascadeRank`.

Selector suffix bytes are `count:u16`, then rows sorted by `(stateId,operatorId,valueBytes)`: `stateId:u16, operatorId:u8, valueLength:u32, valueBytes`. The versioned state registry covers admitted pseudo-classes and typed attributes. These suffixes emit inside `:where()`; raw selector text is not an IR value.

### 5.5 Declaration-block partition

After property winners and web expansion are known, declarations are partitioned by:

```text
(node site, layer, canonical condition, selector suffix, cascadeRank)
```

All declarations in one tuple form one grouped class, sorted by property id. This is the justified exception to “one class per `(node, layer, condition)`”: a node may carry more than one class for the same layer/condition only when existing merge precedence gives the blocks different `cascadeRank` values. Coalescing such blocks would let an unrelated high-rank property drag a low-rank property later in the cascade. There is at most one class per complete tuple above.

For every pair of conditions the conservative overlap checker either proves them disjoint or treats them as co-matching. Two co-matching blocks on one node/layer/rank may not declare the same property unless their value bytes are identical. Ambiguity is a compile error, never a hash-order tie.

### 5.6 Canonical value grammars

Every admitted property names exactly one schema grammar and serializer. The schema's property registry is closed over the v1 set: a property without a complete grammar is unsupported, not an invitation to use a CSS token stream. The load-bearing primitive rules are:

- **Dimensionless number:** canonical binary form is `unitId=unitless,f64LE`; CSS is the §5.2 Ryū spelling with no suffix.
- **Length, percentage, angle, and time:** canonical binary form is `unitId:u8,f64LE`, where the schema's exhaustive `unitId` row supplies the value dimension and one lowercase ASCII CSS keyword. CSS is exactly the §5.2 Ryū spelling followed immediately by that keyword, with no space. A dimensioned zero retains its unit (`0px`, `0%`, `0deg`, `0ms`); only a dimensionless zero emits `0`. Cross-dimension and property-disallowed units reject.
- **Bounded keyword:** author ASCII case is normalized before IR construction and CSS uses the schema row's one lowercase canonical spelling. A non-ASCII or unregistered spelling rejects.
- **Font, URL, and image primitives:** each admitted property uses its schema-declared structured grammar. URL/string payloads are typed data, never raw surrounding CSS; the printer applies the exact §5.7 double-quote escaping template. `url()`, font-family lists, and image functions not represented by those rows reject.

V1 composite grammars are:

- **Color:** `rgba8(r:u8,g:u8,b:u8,a:u8)`, `system-color(enum:u16)`, or `current-color`. CSS is lowercase `#rrggbb` when alpha is 255, otherwise lowercase `#rrggbbaa`; system keywords have one registry spelling. Unparsed color strings are rejected.
- **Transform:** an ordered list of tagged functions. V1 admits `translateX/Y(length)`, `scale/scaleX/scaleY(number)`, and `rotate(angle)`. Each argument uses its typed number/unit serializer. Raw transform strings are not admitted.
- **Shadow:** an ordered non-empty list of `(inset:u8,x:length,y:length,blur:nonnegative-length,spread:length,color)`, emitted with one ASCII space between fields and `, ` between layers.
- **Easing/transition:** `linear`, `ease-in`, `ease-out`, `ease-in-out`, or `cubic-bezier(x1,y1,x2,y2)` with finite values and `x1/x2 ∈ [0,1]`. A static `TransitionMap` first lowers to sorted transition longhands under the target profile. A spring lowers through that profile's registered approximation and therefore carries the profile digest in class identity. Dynamic/opaque transition objects are rejected.
- **Gradient:** the typed two-stop Exact gradient IR emits one canonical `linear-gradient`/`radial-gradient`; it is a `background-image` value, never a color string.
- **Scalar/algebra:** a scalar is `unitId:u8,f64LE`; token and derived refs are their full name-preimage frames. `calc` is a typed prefix tree with fixed `add=1,subtract=2,multiply-scalar=3,divide-scalar=4,min=5,max=6,clamp=7` opcodes and length-framed operands in authored order after the required folds below. CSS emits one outer `calc()`, lowercase function names, and exactly one ASCII space around binary operators; `min`/`max`/`clamp` use `, `. No algebraic reassociation is permitted.

The total v1 `calc` fold set is deliberately small. It folds only finite literal operands: `add` and `subtract` when both literals have the identical `unitId`; `multiply-scalar` when the left literal has any admitted unit and the right literal is dimensionless; `divide-scalar` under the same shape with a nonzero right literal; and `min`, `max`, or `clamp` when every literal operand has the identical `unitId`. Each result is normalized and range-checked again under §5.2. No cross-unit conversion, reference identity fold (`x + 0`), commutation, reassociation, distribution, or nested-function fusion occurs. The schema enumerates these exact patterns and their result-unit rule; an unlisted pattern remains a tree node or rejects on type grounds.

The named `exact-web-css-v1` profile registers exactly one spring lowering,
`spring-fast-ease-out-v1`. It preserves the current web behavior in
`packages/exact-renderer/src/style/css-motion.ts`; it is not represented as a
physics solver that the repository does not have:

1. the input is a statically known `SpringTransition` from
   `@exact/core/style/transitions`;
2. `duration` is the authored finite nonnegative duration in milliseconds, or
   exactly `300` when absent;
3. present `damping`, `stiffness`, and `mass` must be finite and greater than
   zero; present `velocity` must be finite. They do not alter this compatibility
   approximation;
4. `delay` MUST be absent or zero, and `respectsReducedMotion` MUST already
   have been folded by final-theme resolution; and
5. the result is that duration plus exactly
   `cubic-bezier(0.22, 1, 0.36, 1)`.

The schema profile row stores the lowering id, default duration, four Bézier
coordinates, accepted parameter set, and the ignored-parameter disposition as
parameterized feature values. The Facet `default`/`snappy` spring presets
therefore lower deterministically today, even though their damping/stiffness
differences intentionally converge to the same curve. Replacing this with a
sampled physical solver is a new named lowering and target-profile digest,
not an implementation refinement under the same identity.

### 5.7 One emitter and digest authority

The sole CSS/RuleKey/manifest emitter is the proposed TypeScript module `packages/exact-contract/src/web-style/emitter.ts`. The Contract compiler, SSR tooling, and bundle builder call it. Rust readers/runners validate and consume identities; they do not emit CSS. A future second-language emitter cannot ship until byte-for-byte parity with the TypeScript golden corpus is registered.

CSS bytes are UTF-8 without BOM, LF-only, no comments/source maps, lowercase property/at-rule names, and declarations in property-id order. The schema pins one printer: a block header ends with one ASCII space plus `{` and LF; every nesting level adds exactly two ASCII spaces; every declaration is `<indent><property>: <value>;\n`, including the final declaration; a close is `<indent>}\n`; comma-separated selectors use `,\n` and repeat the current indent; statement at-rules end `;\n`; there are no blank lines or trailing spaces; a nonempty asset has exactly one final LF and an empty asset has zero bytes. A condition wrapper remains at the rule's sorted position; regrouping media blocks may not change source order. Flattened rules are emitted by `(sheet role, layer id, decoded cascadeRank under §7.2, RuleKey bytes)`. The count-prefixed rank bytes in `RuleKey` are storage and identity only; they are never a byte-sort key. General minifiers may compress transport but may not rewrite identity bytes.

All emitter-generated identifiers use their registered lowercase ASCII spelling and therefore need no CSS escaping. Typed CSS strings and URL payloads use double quotes. Printable ASCII other than `"` and `\` emits literally; those two emit `\"` and `\\`; every other Unicode scalar emits a backslash, its shortest lowercase hexadecimal code point, and exactly one terminating ASCII space. NUL and lone surrogates reject before printing. Attribute-selector values use that same quoted-string algorithm; the digest selector in §8.3 contains only lowercase hexadecimal ASCII. The schema's templates are the byte authority for selectors and every admitted at-rule; host CSSOM serialization is never fed back into identity.

The target feature profile begins with ASCII `EXWP` and uses the §5.1 tags and
§5.2 frames for the named profile id; sorted admitted property, serializer,
syntax, unit, scope-kind, condition, state, and operator ids; `@property` and
printer feature bits; color/gradient capabilities; page capabilities;
transition/spring-lowering rows; and every parameterized feature value. The
schema defines widths, row frames, and sort orders under the LLP-assigned tags.
The digest is:

```text
SHA-256("ExactWebStyleTargetFeatureProfileV1\0" || EXWP-record)
```

`WebStyleIR` begins with ASCII `EXWI` and uses the §5.1 tags and §5.2
frames. Its required v1 sections are: version; target-profile digest; rules as
complete `RuleKey` rows in emission order; runtime/host variable definitions
sorted by emitted name bytes; token refs sorted by token-name preimage; derived
refs sorted by derived-name preimage; complete theme records sorted by
`(themeFamilyDigest,resolvedThemeDigest)`; the canonical environment table;
page records sorted by route identity; logical asset partitions in delivery
order; and the boot-program digest. Each vector is `count:u32` followed by
`rowByteLength:u32 | rowBytes`; exact row layouts live in the schema. No field
is inferred from filesystem order or JSON property order. Its digest is:

```text
SHA-256("ExactWebStyleIRV1\0" || WebStyleIR)
```

Asset digests are SHA-256 of exact CSS bytes. `WebStyleManifestCore` begins with ASCII `EXWS`, followed by the same ascending-tag frames as §5.2:

| Tag | Payload |
| --- | --- |
| 1 | `webStyleFormatVersion` (`u16,u16`) |
| 2 | `targetFeatureProfileDigest:bytes32` |
| 3 | `tokenSchemaDigest:bytes32` |
| 4 | assets: `count:u16`, then `(role:u8, placement:u8, byteLength:u64, sha256:bytes32)` in logical order |
| 5 | classes: `count:u32`, then `(classHash128:bytes16, ruleDigest:bytes32, assetIndex:u16)` sorted by raw class hash then rule digest |
| 6 | variables: `count:u32`, then each full variable-name preimage frame plus serializer/scope/unset ids |
| 7 | derived refs: `count:u32`, then each full derived-name preimage frame plus serializer id |
| 8 | themes: `count:u32`, then `(themeFamilyDigest:bytes32, resolvedThemeDigest:bytes32, tokenAssetIndex:u16, tokenNameCount:u32, tokenNameHash64:bytes8…, derivedNameCount:u32, derivedNameHash96:bytes12…)` sorted by the two digests; both name vectors sorted raw and unique |
| 9 | pages: `count:u32`, then `(routeIdentity:framed UTF-8, pageRuleDigest:bytes32, assetIndex:u16)` sorted by route UTF-8 bytes |
| 10 | `webStyleIrDigest:bytes32`, the domain-separated digest above |
| 11 | `environmentTableDigest:bytes32`, defined in §8.3 |
| 12 | pre-paint boot program: `programVersion:u16,u16, byteLength:u64, sha256:bytes32` |
| 13 | token refs: `count:u32`, then each full token-name preimage frame plus serializer id, sorted by token-name hash/preimage |

Each vector row is `rowByteLength:u32 | rowBytes`, including fixed-size rows; a “full preimage frame” is itself an ascending-tag record under §5.2. Counts, widths, sort order, and row framing are part of v1.0. URLs and loci are absent. `styleBundleDigest` is:

```text
SHA-256("ExactWebStyleBundleV1\0" || WebStyleManifestCore)
```

The pre-paint program is therefore a versioned, content-addressed part of bundle identity. URLs, SRI strings, and inline-vs-linked packaging live in the delivery manifest and do not change this digest when logical assets/program bytes/order are identical. Every JSON manifest must round-trip to identical core bytes.

The pre-paint executable has two canonical forms joined by the schema. `EXWB`
is its non-executable instruction record. Its instruction vector is
`count:u16` followed by `opcode:u8 | payloadLength:u32 | payload`; v1 assigns
`1=read-root-family`, `2=read-scheme-preference`, `3=read-media`,
`4=read-viewport`, `5=resolve-size-class`, `6=lookup-environment`,
`7=stamp-root`, `8=sync-themed-images`, and `9=install-change-listeners`.
Every opcode appears exactly once in that order; there are no jumps, calls,
imports, author strings, or extension opcodes in v1. Payload layouts and bounds
are schema rows under the LLP-assigned opcode numbers.

Optional `EXWB` tag 9 carries the inherited `toggleTestIdPrefix` as strict UTF-8, bounded to 256 code units. Absent or source-empty normalizes to absent and disables toggle behavior; present-empty is noncanonical. Opcode 9 consumes the field rather than embedding an unowned string.

The delivered program bytes are canonical ES2020 JavaScript generated only by
the schema's one fixed interpreter/source template with the complete `EXWB`
bytes embedded as unpadded RFC 4648 base64url. The schema fixes the prefix,
decoder/interpreter bytes, suffix, quoting, semicolons, whitespace, and absence
of a final newline. `generate-webstyle-schema.mjs --check` evaluates the
generated program against the TypeScript reference coordinator over the boot
corpus and proves source regeneration byte-identical. The §5.7 manifest
program SHA-256 is over those exact JavaScript bytes; because they embed
`EXWB`, `styleBundleDigest` commits to the program grammar, instructions,
constants, and source. An implementation may not hand-minify, stringify a
function, or substitute a behaviorally similar program under that digest.

### 5.8 Collision rules

The emitter stores every truncated-hash preimage. A collision between distinct class, token-name, binding-name, derived-name, or host-name preimages fails `web-style-hash-collision`; no ordinal or salt is allowed. Test injection must exercise every namespace.

## 6. Total static/dynamic split and custom properties

### 6.1 Expression disposition

Every `Expr.kind` and RecipeIR extension form has a production style disposition:

| Expression/value form | Disposition |
| --- | --- |
| string/number/boolean/null | constant-fold, property-serialize; null means omitted |
| `token` or statically recognized `theme.*` member path | `TokenRef` even though today's `isLiteral()` excludes it |
| ident/member/index | typed `--ex-v-*` when value-only; closed option/enum may select classes; otherwise reject |
| unary/binary | constant-fold; admitted typed token algebra → `calc()`/derived ref; other typed runtime result → var |
| ternary | same-shape arms → typed value; presence/shape difference → complete finite class map |
| template | static property-specific constructor only; runtime string templates reject |
| array | admitted structured transform/font/etc. serializer with closed items; spreads require closed shape |
| object | closed style object recursively classified, or property-specific TransitionMap/shadow/etc.; otherwise reject |
| call | compiler intrinsic, registered value constructor, or RecipeIR only; arbitrary call rejects |
| RecipeIR `paramRead` | classify override and declared fallback together; closed finite domain may select classes, compatible scalar becomes a typed var, incompatible shape/presence rejects |
| RecipeIR intrinsic call | classify every registry-declared produced key; theme-only result may use a derived ref, typed runtime scalar may use a var, undeclared/opaque result rejects |
| RecipeIR `unset` | remove the accumulated property; emit absence/finite presence arm, never the CSS `unset` keyword |
| relation | layout authority must lower it to a declared web property/value before this pass; otherwise not a style value |
| arrow | reject |

`undefined` is not an Expr arm but can result from a typed option/member/call. Static undefined emits no declaration. Dynamic `T | undefined` is a finite presence discriminant: the present class consumes a typed var; the absent class set omits the declaration. An untyped value that may be absent is rejected.

Token arithmetic admits `+`/`-` on compatible dimensions and `*`/`/` by a finite dimensionless scalar, emitted as canonical `calc()`. Registered `min`/`max`/`clamp` are profile-gated. `%`, rounding, string construction, color mixing, and concentric-radius logic are not guessed into CSS; theme-only forms require a RecipeIR `DerivedThemeRef`, runtime forms use a typed var, and otherwise admission fails.

A static gradient in `backgroundColor` is projected to `backgroundImage`. A dynamic closed `Color | Gradient` sum emits a discriminant-to-class-set map and one typed value binding per arm; an opaque string that might be either rejects. A static motion preset lowers under §5.6; a finite preset choice selects classes; a dynamic object rejects.

Temporal owners are exclusive. If motion/pager/presence owns `transform`, `opacity`, shadow, or position, the compiler emits one property declaration consuming the owner's typed variable. Static authored contribution is an input to that owner's registered composition; no competing class also declares the property.

### 6.2 Variable identities

Token names are:

```text
--ex-t-<canonical-ascii-slug>-<base32(blake3-64(token-path,type,serializer,format))>
```

Runtime binding names are:

```text
--ex-v-<base32(blake3-96(sourceModuleId,bindingSiteId,propertyId,serializerId,format,profile))>
```

Derived theme names are `--ex-d-<base32(blake3-96(EXWD-record))>`. Host temporal names are `--ex-h-<base32(blake3-96(EXWH-record))>` and require a census row.

These formulas hash framed binary records, never string concatenation. Each record starts with its shown ASCII magic and then §5.2 ascending-tag fields:

| Record | Fields in ascending tag order |
| --- | --- |
| `EXWT` token | `webStyleFormatVersion`; canonical token-path UTF-8; value-type id `u16`; serializer id `u16`; `tokenSchemaDigest:bytes32` |
| `EXWV` runtime binding | `webStyleFormatVersion`; `targetFeatureProfileDigest`; `sourceModuleId` UTF-8; canonical `bindingSiteId` bytes; property id `u16`; serializer id `u16`; scope kind `u8` |
| `EXWD` derived theme | `webStyleFormatVersion`; `targetFeatureProfileDigest`; canonical RecipeIR expression/intrinsic bytes; produced property id `u16`; result-type id `u16`; serializer id `u16` |
| `EXWH` host/temporal | `webStyleFormatVersion`; `targetFeatureProfileDigest`; registered owner id `u16`; canonical structural-node identity bytes; property id `u16`; serializer id `u16`; scope kind `u8`; registered channel id `u16` |

Section 5.1 assigns the tags; the schema supplies their exact nested layouts.
`blake3-64/96` means the first 8/12 standard output bytes, encoded with §5.3
Base32. The token slug is outside the hash suffix but derives from the same
canonical token path.

`sourceModuleId` is the canonical package-relative module identity from the resolved import graph: package identity plus slash-normalized module specifier, never an absolute path and never `planDigest`. `bindingSiteId` is the canonical component declaration identity plus structural node path and source-slot ordinal — the component half is `ComponentDeclarationIdentityV1` and the instance-site encoding follows `ComponentInstanceSiteIdentityV1`, both from the landed plan format v1.4 identities block (`packages/exact-contract/plan/format-schema.json`, the citable tuple authorities); property id is the separate preimage field above. Both are available before plan serialization, eliminating the former circular “plan-or-module-id” input. Host structural identity uses the same component-plus-node-path encoding and never a live DOM id or traversal order.

The token slug is part of emitted bytes and `tokenSchemaDigest`; it is not merely diagnostic. Slug generation is lowercase ASCII, non-alphanumerics collapsed to one hyphen, with empty slugs represented as `token`.

### 6.3 Inheritance split and removal

The v1 `StyleVar` plan/manifest rows are only runtime/host bindings in the `--ex-v-*` and `--ex-h-*` namespaces. Every such row records serializer, CSS syntax, scope kind, and a fallback/unset policy. In a target profile supporting registered custom properties, the emitter writes an `@property` record with `inherits: false`. It uses a typed, computationally-independent `initial-value` when one exists; otherwise it emits `syntax: "*"`, omits `initial-value`, and relies on the resulting guaranteed-invalid initial value plus the declaration's typed `var()` fallback.

Theme token and derived namespaces are the opposite: v1 leaves `--ex-t-*` and `--ex-d-*` unregistered, so they inherit from the nearest theme root/scope. A future profile may register them only with `inherits: true`. They are set only by the complete theme-record selector in §8.3 on the root or nested scope node; a renderer never writes them on an ordinary descendant and never installs a local sentinel for them.

In a profile without `@property`, every runtime/host binding site writes a **local** guaranteed-invalid sentinel (`--ex-v-…: initial` or `--ex-h-…: initial`) before any possible read and whenever the value is absent. It MUST NOT use `removeProperty()`, because removal would expose an ancestor instance's same-site variable. A defined value replaces the sentinel. V1 has no region-hoisted binding kind; every binding remains on its declared node-local site.

For a present transition, the writer sets the value before adding the consuming class. For removal it removes/selects away the consuming class before restoring the sentinel. Both operations occur inside one renderer commit. This defines SSR unset, option-none, node reuse, scope move, and teardown behavior.

### 6.4 Inline budget and complete writer census

On an author-owned node, legal inline declarations are only registered `--ex-v-*` and `--ex-h-*` values/sentinels. Ordinary properties are zero. The following table is the migration census; a scanner over the named files must map every `.style`/`style.cssText`/`setAttribute("style")` writer to exactly one row.

| Current writer | Current source | Owner | CSS-mode disposition |
| --- | --- | --- | --- |
| mirror whole attribute/style pass and inline hydration comparison | `dom-mirror.ts` `clearElementAttributes`, `applyDescriptionStyles`, `styleDeclarationsMatch` | renderer props writer | replace with owned attr diff + class/var correspondence |
| direct-host stale-attr removal and `cssText` replacement | `web-host-direct.ts` `applyDescription` | production renderer props writer | replace with the same class-first writer; preserve unknown attrs/classes/vars |
| root presentation and `LAYOUT_PROFILE_WEB_DEFAULTS` | both hosts; `dom-element-description.ts`; generated defaults | framework reset | emit `exact.reset` classes; no root ordinary inline defaults |
| `lowerDomStyle` shorthands, borders, transforms, transitions, shadows, gradients, materials | `dom-element-description.ts` | shared web projector | move to `ResolvedStyleIR → WebStyleIR`; no runtime static re-lowering |
| graphics/pointer `touchAction` after authored style | `dom-element-description.ts` | input policy | static class when closed; typed host var when live |
| live Motion gesture `touchAction` reconciliation | `dom-mirror.ts` `syncDomMotionGesture` | Motion input-policy owner (mirror live writer) | registered `--ex-h-motion-touch-action-*` consumed by one static declaration, with projected authored/static `touch-action` as fallback; update/sentinel on descriptor change/teardown; this is not the static description-lowering row above |
| drag-time text-selection suppression and capture-guarded restore | `dom-mirror.ts` `suppressWebMotionTextSelection`, `restoreWebMotionTextSelection` | Motion selection-policy owner | registered non-inheriting `--ex-h-motion-user-select-*` and `--ex-h-motion-webkit-user-select-*` consumed by static declarations; set both to `none` on arena win and restore their local sentinels on end/cancel/reset/invalidation, preserving the authored class values without ordinary inline writes |
| presence opacity/transform pins | `dom-mirror.ts` `playPresenceEnter` | presence temporal owner | author node uses registered `--ex-h-presence-*`; no ordinary inline endpoint |
| pager transform/transition/`willChange` and touch policy | `dom-mirror.ts` pager functions | pager temporal/input owner | registered host vars on author nodes; pager guard elements remain counted host residue |
| other motion/channel writes including `SHADOW_GROUP_PROPS`; direct glass/backdrop channel (excluding the two named Motion policy writers above) | both hosts' channel seams | motion/channel owner | typed vars consumed by one static declaration; shadow group composes before serialization |
| drag placeholder/projection | `dom-mirror.ts` projection/channel paths | drag owner | source placeholder via host var; detached projection node may retain counted direct styles |
| anchored overlay translate/edges and tail flipping | `dom-mirror.ts` anchor positioning | overlay measurement owner | typed host vars; ephemeral measurement helpers may retain counted residue |
| inline SVG/backend-child sizing/tint and media backend internals | renderer backend helpers | host-owned child | registered `data-ex-host-style` row with exact properties and ratchet count |
| static `root.style.colorScheme` | `static-scheme.ts` | theme environment | delete; emit `color-scheme` in selected theme record |
| static `--sr-<n>` hoist | `contract-static-theme.ts` | legacy static theme | retire when canonical theme records ship; `--sr-*` count must be zero in `css` mode |
| virtual-visibility mode application on certified collection roots and their rows (`content-visibility`, `contain-intrinsic-size`, `overflow-anchor`) | web host list controller (RFC 0540 §1.7 / LLP 0543 §13) | collections temporal owner | registered non-inheriting `--ex-h-*` retained-size vars consumed by one static declaration per mode class; `hidden` sets `content-visibility: auto` with `contain-intrinsic-size: auto var(--ex-h-<w>) auto var(--ex-h-<h>)` and `prerender`/`visible` restore the local sentinels; `overflow-anchor: none` is a static declaration on Exact-controlled collection roots, never a per-frame write; no ordinary inline properties |

> **Row added 2026-08-26 (RFC 0540 §1.7 / §8.5 row 69; shapes LLP 0543
> §13).** Three folds in one, all of them this document's own
> machinery rather than new machinery.
> 1. **Grammar rows.** `content-visibility`, `contain-intrinsic-size`,
>    and `overflow-anchor` are admitted as §5.6 canonical value
>    grammars **under a profile revision** — not as an ad-hoc
>    passthrough. The CSS keyword `auto` is kept in both positions
>    (`content-visibility: auto`, and the `auto <length>` form of
>    `contain-intrinsic-size`) per RFC 0540 §1.7: `auto` is doing real
>    work there — it is what lets the browser keep the last rendered
>    size — so lowering it to a fixed length would silently change
>    behavior.
> 2. **Host variables — two, not one.** RFC 0540 §1.7 says "the
>    retained size is a registered host variable" in the singular;
>    LLP 0543 §13 pins **two**, because `contain-intrinsic-size`'s
>    two-axis form needs a width and a height
>    (`auto var(--ex-h-<w>) auto var(--ex-h-<h>)`). That is a 0543
>    **spec pin** of an implementation value, not a contradiction of a
>    0540 decision, and this document follows 0543. Both take the §6.2
>    `--ex-h-<base32(blake3-96(EXWH-record))>` identity form, are
>    non-inheriting, and follow §6.3's sentinel discipline — local
>    guaranteed-invalid sentinel on `prerender`/`visible`, never
>    `removeProperty()`, which here would be a real defect and not a
>    style nit: removal would expose an **ancestor row's** retained
>    size to a descendant row.
> 3. **One census row.** The table row above is that row. The scanner's
>    one-writer-one-row rule applies: the list controller's mode
>    application may not also be counted under the broad channel or
>    static-lowering rows.
>
> Delivery is post-window (RFC 0540 §8.6, D2) and the profile revision
> lands with 0540's web rung; `web-style-writer-census` sees the new
> writer only when that host code exists.

Host-owned residue rows must declare marker, exact property allowlist, lifecycle, and ceiling in the registered check artifact. The scanner enforces one writer → one row: a named specialized writer cannot also be counted under a broad channel/static-lowering row, and a new direct writer without exactly one row fails the census. F6 requires zero ordinary author-owned declarations and zero `--sr-*`; host residue is reported separately and ratchets downward.

## 7. Cascade order is not hash order

### 7.1 Specificity

The generated class is never inside `:where()`. Root/state scaffolding is:

```css
:where([data-ex-style-root]) .exr_<hash>:where(<typed-state-suffix>),
:where([data-ex-style-root]).exr_<hash>:where(<typed-state-suffix>) { ... }
```

Without a suffix, the `:where()` suffix is omitted. Both root and descendant forms have one class of specificity `(0,1,0)`. Non-reset declaration classes MUST use this template and may not zero their generated class.

`exact.reset` is pinned to zero specificity. Its schema template places the generated class inside `:where()` in both forms:

```css
:where([data-ex-style-root]) :where(.exr_<hash>):where(<typed-state-suffix>),
:where([data-ex-style-root].exr_<hash>):where(<typed-state-suffix>) { ... }
```

The suffix is again omitted when empty. Theme-scope predicates use typed attribute selectors and do not mint arbitrary app selectors.

The document root carries `data-ex-style-doc-root` and `data-ex-style-root`; nested token scopes carry only the latter. A qualified rule containing an environment-scheme/reduced-motion/contrast/size-class condition instead uses `:where([data-ex-style-doc-root]<environment predicates>)` in both scaffolds. Nested environment attrs never qualify app rules. Theme records retain §8.3's `[data-ex-style-root][data-ex-theme]` selector for nearest-scope token inheritance.

### 7.2 `cascadeRank`

`cascadeRank` is a bounded vector of `u32` parts compared
part-lexicographically. Compare the first unequal part; if all parts of the
shorter vector equal the corresponding prefix of the longer vector, the
shorter vector sorts first. The compiler derives it from the already-defined
source precedence and authored conditional order:

```text
[merge-source-rank, composition-site-ordinal,
 condition-block-ordinal, nested-arm-ordinal...]
```

The maximum is 16 parts. Merge-source rank preserves lowering defaults < `style=` < authored attrs inside a layer. Component/style composition sites use canonical structural order. Base has condition ordinal 0; conditional arms use their 1-based lexical order; nested arms append an ordinal. Reversing two authored overlapping arms reverses their ranks and therefore their CSS winner.

Rank is in the `RuleKey` as the count-prefixed storage encoding in §5.3, but
emission compares the decoded vectors by the rule above. Emission is
`(sheet role, layer, decoded cascadeRank, RuleKey bytes)`. The key is only a
deterministic tie-break after semantic order. The overlap/property validation
in §5.5 guarantees that a tie-break cannot choose a winner.

### 7.3 Layer and unlayered placement

| Placement | Owns |
| --- | --- |
| `exact.reset` | UA normalization, layout-profile defaults, semantic parity |
| `exact.tokens` | complete final theme records and `color-scheme` |
| `exact.facet-recipes` | RecipeIR-derived component rules |
| `exact.app-static` | static app winner blocks |
| `exact.app-dynamic` | declarations consuming runtime vars and finite arms |
| `exact.overrides` | compiler-generated app override rules not represented by token records |
| unlayered accessibility asset | Exact forced-colors/reduced-motion corrections |
| unlayered active page asset | one route's `@page` records |

App/scope/agent token inputs are **not** sparse CSS rungs. Section 8 resolves them into one final record. This removes the r1 conflict between a full resolved theme and a second sparse cascade model.

Unlayered app/third-party author CSS outranks all normal Exact layers. The later Exact accessibility asset is no longer categorically below it, but ordinary specificity/source-order rules still apply; intentional later or more-specific app CSS can override it. Normal user-origin CSS does not generally outrank normal author-origin CSS. User-origin `!important` retains its browser-defined precedence.

### 7.4 Required order fixture

The corpus MUST include two equal-specificity, overlapping conditions that set the same property. In fixture A the wider condition precedes the narrower; in fixture B their authored order is reversed. Computed style must reverse accordingly while class hashes remain irrelevant to the winner. A mutant emitter sorted only by RuleKey must fail.

The negative fixture also pins the unequal-length pair `[2,0,2]` versus
`[2,0,1,1]`. Part-lex order emits `[2,0,1,1]` first and `[2,0,2]` second;
sorting their count-prefixed storage bytes produces the opposite order and
MUST fail. This pair makes falsifier 7 decidable for nested-versus-sibling
arms.

## 8. One resolved-theme model

### 8.1 Final-record ownership

One `ThemeCssRecord` owns the **complete final token values after** definition/definition overrides → app → scope → agent merge and post-merge size-class/motion adaptation. CSS does not re-run those rungs. Provenance stays in the loci/theme diagnostic sidecar.

Each record contains:

```text
themeFamilyId
themeFamilyDigest
resolvedThemeDigest
tokenSchemaDigest
resolved environment tuple
ordered(token id, serialized final value)
ordered(derived-theme id, serialized final value)
color-scheme
```

`resolvedThemeDigest` is exactly LLP 0485 §3.4 resolved-value identity.
`data-ex-theme` always contains that digest as 64 lowercase hexadecimal
characters. `data-ex-theme-family` contains the diagnostic family id;
`data-ex-theme-family-digest` contains the authoritative family digest as 64
lowercase hexadecimal characters. The id never substitutes for the digest,
and the pre-paint/environment join never reverse-resolves a digest from the
human id. `themeFamilyDigest` is the 32-byte canonical family identity used by
binary joins:

```text
SHA-256("ExactWebStyleThemeFamilyV1\0" || EXWY-record)
```

`EXWY` uses the §5.1 tags and §5.2 frames. Tag 1 is
`webStyleFormatVersion:u16,u16`; tag 2 is
`baseKind:u8 | valueLength:u32 | valueBytes`, where `0=definition` and
`1=custom-base`; tags 3–6 are the optional definition/app/scope/agent override
inputs; and tag 7 is the effective `sizeClasses` after defaulting, encoded as
three length-framed rows in compact → regular → expanded order, each containing
its registered `sizeClass:u8` and optional min/max bounds as normalized `f64LE`.
The schema assigns closed member ids and sum discriminants for the definition,
complete custom base, and override trees. Objects are `count:u32` followed by
`rowByteLength:u32 | memberId:u16 | valueLength:u32 | valueBytes` rows sorted
by ascending member id; lists are `count:u32` followed by
`itemByteLength:u32 | itemBytes` in semantic order. Booleans are `u8` zero/one;
numbers and strings use §5.2; registered enum/sum widths come from the schema;
and token values and conditions reuse §5.6 and §5.4 respectively. Every
optional value is `present:u8`, followed only when present by
`valueLength:u32 | valueBytes`; null/undefined overrides are absent, and empty
object members are recursively pruned before an empty override normalizes to
absent. The environment axes that select records are excluded.
`EXWQ`/`EXWF`/`EXWJ`, `StyleThemeRef`, SSR family-digest attributes, and
manifest joins consume the resulting 32 bytes (hex-encoded only in SSR
attributes).

### 8.2 Complete `ThemeResolveInputs` disposition

| Input field | V1 class | Normative web behavior |
| --- | --- | --- |
| `scheme` | media-query-selected → admitted-digest | explicit value selects a record; `system` uses `matchMedia` only as coordinator input, then selects a concrete light/dark digest |
| `reducedMotion` | media-query-selected → admitted-digest | preference selects a record after `applyMotionPreference`; CSS paint keys the coordinated attr/digest, not the raw query |
| `textScale` (continuous) | admitted-digest | `exact-web-css-v1` admits exactly `1` as its production browser source; other exact values require another named profile/deployment record, and unknown values fail without changing state |
| `contrast` | media-query-selected → admitted-digest | standard/increased selects a final resolved record; forced-colors remains a separate CSS environment |
| `platform` | not-in-v1 as a live axis | web build requires `platform=web`; another value is a different target/profile, never a runtime theme flip |
| `sizeClass` | admitted-digest | explicit compact/regular/expanded selects a final record and structure/style together |
| `sizeClasses` | rebuild | thresholds are family/style-bundle inputs; a live override is rejected |
| `viewportWidth` (continuous) | media-query-selected | coordinator maps width through the active family's fixed `sizeClasses`; only a discrete size-class/digest transition is published |

No row uses an unconstrained CSS variable as a substitute for resolved-theme identity. Browser zoom is not silently reinterpreted as `textScale`.

### 8.3 Environment transaction

The bundle contains a canonical environment table mapping
`(themeFamilyDigest, concrete scheme, reducedMotion, textScale, contrast,
sizeClass)` to one admitted `resolvedThemeDigest`. `EXWE` also contains one
`EXWF` family row per referenced digest. That row owns the diagnostic family
id and the exact optional min/max bounds for compact, regular, and expanded;
resolution order is always compact → regular → expanded, inclusive at both
bounds, with expanded as the no-match fallback. Bounds use normalized §5.2
binary64 values. The table therefore owns the width→size-class function the
boot program executes; thresholds are not hidden constants in JavaScript or
raw CSS. Its binary records use the §5.1 tags; the schema defines widths,
optional-bound encoding, normalized `textScale` bytes, and raw-digest row
sorts. Its identity is:

```text
environmentTableDigest =
  SHA-256("ExactWebStyleEnvironmentTableV1\0" || EXWE-record)
```

**Every record in that admitted environment table ships in the initially loaded `exact.tokens` CSS.** There is no per-environment sheet enable/disable step on the normal boot or coordinator path. Section 8.4 theme assets are exclusively for manifest-declared records outside this admitted table.

Each admitted record uses this exact selector template, with the resolved digest encoded as 64 lowercase hexadecimal characters:

```css
:where([data-ex-style-root][data-ex-theme="<resolvedThemeDigest>"]) {
  --ex-t-<canonical-ascii-slug>-<base32-token-hash>: <serialized-token-value>;
  --ex-d-<base32-derived-hash>: <serialized-derived-value>;
  color-scheme: <light-or-dark>;
}
```

The schema printer expands the repeated declarations in canonical token-name-preimage order, then derived-name-preimage order, then `color-scheme`; the displayed indentation/spacing follows §5.7 exactly. This selector is used unchanged on a document root and on a nested theme-scope node. Theme-bearing CSS selects only through this stamped resolved digest. Raw `prefers-*` media queries may set non-painting probes but may not select token/derived values in a resumable artifact.

The root/scope owner stamps initial identity before paint. The document root always carries both root markers and its complete request-captured (SSR) or current (CSR) identity/environment. A nested `SetThemeScope` activation expression is evaluated in that same environment before emission/insertion: true emits its style-root marker plus complete target identity/environment attrs; false emits the marker only. This IFF rule covers streamed chunks. Pre-paint touches only the document root; resumption verifies nested scopes.

The canonical pre-paint boot program is the versioned content-addressed asset
in §3.2/§5.7 and embeds the exact canonical `EXWE` table bytes it consumes. Its
expected SHA-256 and `environmentTableDigest` are verified from the delivery
tuple by SRI/hash policy before execution. Boot input semantics are closed:

- root family identity comes only from the SSR-stamped
  `data-ex-theme-family-digest`; the diagnostic family id is not a join;
- scheme preference uses `EXACT_SCHEME_STORAGE_KEY`, whose exact value remains
  `exact:scheme-preference`, plus the deployment's schema-bounded ordered
  legacy keys and the existing light/dark/system normalization/migration;
- media strings are exactly `(prefers-color-scheme: dark)`,
  `(prefers-reduced-motion: reduce)`, and `(prefers-contrast: more)`; false
  means light, no reduction, and standard contrast respectively;
- viewport width is finite `window.innerWidth`, or `0` when unavailable,
  clamped to at least zero. Width uses the `EXWF` bounds and never a generated
  width `matchMedia` query; resize listens to `window`'s `resize` event, matching
  Contract's current `readViewportHostSize()` ingress; and
- production `textScale` is the `EXWB.defaultTextScale` constant, exactly `1`
  in `exact-web-css-v1`. Browser zoom is not a text-scale signal in v1.

Before first paint boot verifies the SSR-stamped root record against the delivery tuple and EXWE, then re-stamps only that SSR `data-ex-theme` digest and its corresponding attrs (including the serialized `data-scheme*` values and canonical `data-ex-text-scale`); it never writes `colorScheme`. It resolves the current client environment to exactly one EXWE row but stores that snapshot/target privately and MUST NOT apply it before adoption.

Until activation, boot listeners and `globalThis.__exactApplySchemePreference` update persistence and the latest private snapshot only—not root attrs, images, toggle `aria-pressed`, classes, or variables. An unresolvable input changes neither snapshot nor DOM. Resumption therefore adopts the serialized SSR tuple and populates the LLP 0485 root table from that exact record.

In `css` mode this program replaces `STATIC_SCHEME_BOOT_SCRIPT_PREFIX` and its direct `colorScheme` write. It inherits that module's storage/migration/normalization, `data-scheme*`, themed-image, toggle-targeting, and global-hook semantics, but defers client-environment DOM effects to the coordinator. `legacy-inline`/`compare` retain the old script. The corpus compares final outcomes and asserts no pre-adoption mutation.

At the activation fence, one non-yielding all-or-nothing `transferBootEnvironmentOwnership` takes the final recorded snapshot, uninstalls every boot listener, deletes boot-private state, transfers the hook/listener token and snapshot, and activates coordinator replacements. Event dispatch cannot interleave; failure leaves boot sole owner and runner inert, so ownership never overlaps or lapses.

After that transfer, one `WebThemeEnvironmentCoordinator` owns every preference/viewport change. It:

1. consumes the transferred snapshot as its first input and captures one fresh snapshot per later change;
2. resolves an admitted record or fails without mutation;
3. prepares the new **root** theme arena/token-table pointer and verifies that the initially loaded `exact.tokens` asset contains the root target and every currently active fixed nested target record;
4. in one renderer activation-fenced task, swaps the root logical pointer and root paint attributes; pending boolean activation/removal of fixed `SetThemeScope` records may commit in the same task, but the coordinator does not reselect them from the root environment; and
5. releases semantic invalidations after the commit.

If transferred target B differs from adopted SSR target A, the swap is the coordinator's first atomic post-adoption commit; equality is a no-op. The deliberate cost is one A-themed first frame before B, preserving tuple/first-frame identity. Stamped selectors prevent an early pixel flip; no app-node walk or recipe rerun occurs.

### 8.4 System, unknown, and scope themes

Explicit light/dark retain current `data-scheme` semantics; system retains absence of that attribute and uses `data-scheme-preference="system"`. `color-scheme` is in the selected final record. A production switch to an unbundled digest may load only a manifest-declared digest-addressed theme asset; activation waits for verification. Otherwise it fails `web-style-theme-digest-unavailable` with old pixels/table intact.

That load path applies only to a manifest-declared record **outside** the admitted environment table and is never selected automatically by the pre-paint program or media/viewport coordinator. It loads the asset disabled, verifies URL/SRI/bytes/digest and the record selector, enables it while its digest selector is still inactive, then performs the logical-pointer/attribute swap in the activation fence. Enabling early cannot paint because no root yet carries the new digest. The previous token asset may remain enabled because its selector no longer matches; any later disable/unload is custody cleanup, not part of correctness.

A nested scope selects one **fixed** complete record already admitted by EXWE
and present in the initially loaded `exact.tokens` asset. It is not reselected
when the document-root scheme, contrast, size class, or other environment axis
changes; an environment-following nested family is outside v1. `SetThemeScope` cannot
target the fenced outside-table load path; external-theme activation remains a
coordinator operation with the disabled-load/verify/enable fence above, not a
plan opcode. Repeated app/scope/agent merges at runtime are not allowed in
production. Development may compile a new record, update the dev manifest, and
then activate it; that is compilation, not runtime rule synthesis.

V1 nested themes are deliberately **CSS-inheritance paint scopes only**. Their
complete token/derived custom properties affect emitted CSS beneath that scope,
but LLP 0485 §3.4 still has one arena theme table populated from the document
root. Every plan `$token` read—including one evaluated for a node inside a
nested scope—therefore resolves the root table record. A nested subtree may
paint nested token values through static CSS while plan-computed/dynamic values
in that subtree use root tokens; tools MUST diagnose this split when both occur.
No implementation may install a hidden nested arena overlay or retarget token
indices in v1.

Genuinely mixed-theme plan data requires the deferred named amendment
`A-0520-MULTI-RECORD-THEME-TABLE`, which must extend LLP 0485 with explicit
per-scope record pointers, dependency invalidation, resumption identity, and
first-frame rules before such reads are admitted. It is not part of v1.4.

### 8.5 Size-class synchronization

Named size-class style arms emit selectors keyed only by the document root's
`data-ex-style-doc-root` + `data-ex-size-class`, not nested attrs or an
independent threshold copy. Structural `match size` reads that same committed
document value. Changing `sizeClasses` requires a bundle/family rebuild.

An explicitly authored numeric viewport condition is independent of `match size` and lowers to real `@media`; it may not claim atomic structural correspondence.

## 9. Conditions, print, and style realms

### 9.1 Typed authoring

Track W may add a bounded `styles` block. It compiles to the condition IR above; arbitrary selectors/at-rules remain unavailable. Named size classes use §8.5. Numeric viewport width, orientation, hover/pointer capability, forced colors, and print/screen lower to real CSS media conditions. Theme environment preferences use only the coordinated document-root marker/attributes in §7.1/§8.3; same-named nested attrs are ineligible.

`@page` accepts only static typed `size`, canonical margins, and profile-admitted bleed/marks. Theme-dependent or runtime values are rejected unless identical across every admitted theme.

### 9.2 Page custody

`@page` is document-global and unlayered. A route with page rules receives a separate page asset. Prefetch loads it disabled (`media="not all"` or disabled sheet). Navigation activation disables the old page asset and enables the new one inside the route commit fence. At most one primary page owner may be active per `Document`; a second Exact root requesting different page rules fails `web-style-page-owner-conflict`. Roots without page records may coexist.

### 9.3 Shadow DOM boundary

V1 `StyleRealm` is `Document` with light-DOM Exact roots. Mounting a `css`-mode root in a `ShadowRoot` fails `web-style-shadow-root-unsupported` before DOM mutation. Future `Document | ShadowRoot` custody and adopted stylesheets require a profile/version extension; document sheets are never claimed to style through a shadow boundary.

## 10. Proposed plan representation — targeting ≥ v1.7

### 10.1 Version status

**Re-target (2026-08-23, per 0504 §3 row 29 / LLP 0551 finding 28):**
at drafting the landed authority was v1.3 and this proposal was named
"v1.4". Plan-format **v1.4 (identity stability)** and **v1.5 (the
LLP 0514 M1 WP2 host interface)** have since landed with other
content, so the `styles` segment now targets the next compatible
minor, **≥ v1.7**. Throughout this document, "v1.4" used as the name
of this styles proposal reads as that ≥ v1.7 target, and the "v1.3"
baseline contrasts read as the pre-styles landed version (v1.6, the enum-roster digest); the
separately landed **v1.4 identities block** cited in §6 is a
different, landed minor and keeps its literal meaning. The
compatibility semantics below are unchanged; re-deriving the deltas
against the landed v1.4/v1.5 schema is the format owner's
adoption-time work.

The following is a schema-shaped compatible-minor **proposal
(≥ v1.7)** to the format owner:

- add `columnEncoding` values `bytes8=14`, `bytes12=15`, and `bytes16=16` for fixed raw byte strings;
- add optional-core segment `styles`, `segmentKind=25`, `flags=optional (1)`, with required tables 74–83 below when the segment is present;
- add `depTargetKind.style-binding=8`;
- add optional Nodes column `staticStyleClassSetRef`, column id 11, `optionalRef32`, offset 36, length 4, stride/new v1.4 node `rowStride=40`, target `StyleClassSet`, and column flag `optional (1)`; and
- preserve every other landed v1.3 table/column/discriminant unchanged.

Only the web style profile emits these semantics and raises both `formatVersion` and `minReaderVersion` to 1.4. Native-target plan emission remains byte-for-byte v1.3 and never emits the optional Nodes column, `styles` segment, or `depTargetKind=8`; a native/v1.3 reader therefore never encounters the new enum. A v1.4-capable native reader validates a received v1.4 style segment and `style-binding` deps but treats the three style-binding operations as non-rendering no-ops. A v1.3 reader rejects a web v1.4 plan from the header before parsing any new discriminant. The v1.4 web profile requires the segment and joins; their optional framing exists for compatible readers and non-web artifacts, not as permission for a web runner to omit them.

### 10.2 Tables and wire shapes

All refs/counts are `u32`; `0xffffffff` is the only absent `optionalRef32`. Counts and starts are checked against the landed maximum row limit. Every table uses AoS layout and required columns unless stated otherwise.

| Table | id | stride | Columns (`columnId: encoding @ offset`) | Canonical row order |
| --- | ---: | ---: | --- | --- |
| `StyleClass` | 74 | 16 | `1 classHash128:bytes16 @0` | raw `classHash128` |
| `StyleClassRefList` | 75 | 4 | `1 classRef:ref32→StyleClass @0` | packed-vector stream order below |
| `StyleClassSet` | 76 | 8 | `1 classRefs:range32→StyleClassRefList @0` | referenced class-ref vector lexicographically |
| `StyleVar` | 77 | 24 | `1 varHash96:bytes12 @0`; `2 propertyId:u16 @12`; `3 serializerId:u16 @14`; `4 scopeKind:enum8 @16`; `5 unsetPolicy:enum8 @17`; `6 reserved:u16=0 @18`; `7 fallbackExprRef:optionalRef32→Exprs @20` | raw `varHash96` |
| `StyleSelectArm` | 78 | 8 | `1 discriminantValueRef:ref32→Exprs @0`; `2 classSetRef:ref32→StyleClassSet @4` | owning binding order, then canonical discriminant expression bytes |
| `StyleBinding` | 79 | 24 | `1 nodeRef:ref32→Nodes @0`; `2 kind:enum8 @4`; `3 scopeKind:enum8 @5`; `4 serializerId:u16 @6`; `5 exprRef:ref32→Exprs @8`; `6 targetRef:u32 @12`; `7 arms:range32→StyleSelectArm @16` | `(nodeRef, kind, targetRef, exprRef, scopeKind, serializerId, arm-vector bytes)` |
| `StyleTokenNameRefList` | 80 | 8 | `1 tokenNameHash64:bytes8 @0` | packed-vector stream order below |
| `StyleDerivedNameRefList` | 81 | 12 | `1 derivedNameHash96:bytes12 @0` | packed-vector stream order below |
| `StyleThemeRef` | 82 | 112 | `1 themeFamilyDigest:digest32 @0`; `2 resolvedThemeDigest:digest32 @32`; `3 tokenSchemaDigest:digest32 @64`; `4 tokenNames:range32→StyleTokenNameRefList @96`; `5 derivedNames:range32→StyleDerivedNameRefList @104` | raw `(themeFamilyDigest,resolvedThemeDigest,tokenSchemaDigest)` |
| `StylePageRef` | 83 | 40 | `1 routeRef:ref32→Entrypoints @0`; `2 pageRuleDigest:digest32 @4`; `3 custodyFlags:u8 @36`; `4 reserved:u8=0 @37`; `5 reserved2:u16=0 @38` | `(routeRef,pageRuleDigest)` |

The new enum/bit assignments are:

| Registry | Values |
| --- | --- |
| `StyleBinding.kind` (`u8`) | `SetStyleVar=1`, `SelectStyleClass=2`, `SetThemeScope=3` |
| `StyleVar.unsetPolicy` (`u8`) | `registered-typed-initial=1`, `registered-guaranteed-invalid=2`, `local-initial-sentinel=3` |
| `StylePageRef.custodyFlags` (`u8` bitmask) | `route-primary-owner=0x01`; known mask `0x01`, every other bit rejects |
| `Deps.targetKind` (`u8`) | existing values 1–7 unchanged; `style-binding=8` |

The v1 `scopeKind` registry has exactly two semantic rows:
`node-local` and `theme-scope-paint`; their numeric ids live in the named
production profile. `scopeKind`, `propertyId`, and `serializerId` use the exact
generated discriminants from the §5.1 WebStyle schema. That schema explicitly
assigns `serializerId.none=0` for `SelectStyleClass`/`SetThemeScope`; zero in a
registry without an explicit assignment and every unknown value reject under
§5.1. Plan-format generation consumes those generated registries and
parity-gates their digest, so the plan and CSS emitter cannot assign them
independently.

Class strings, rule digests, layers/ranks, and custom-property strings are mechanically derived or manifest-owned and are not duplicated in the plan. The active manifest must map every plan class hash, token-name hash, derived-name hash, and var hash to exactly one full spelling/preimage and its applicable serializer/rule row. `fallbackExprRef` is absent for no fallback; otherwise it points to a constant expression whose type matches the serializer. `StyleSelectArm.discriminantValueRef` points to a constant expression of the binding expression's closed bool/enum/option discriminant type.

`StyleClass` rows sort by raw `classHash128`; the manifest join supplies the unique rule digest. A class set is a sorted unique vector of class refs. Identical vectors intern once; unique vectors sort lexicographically; v1.4 packs each non-empty vector contiguously into `StyleClassRefList` without overlap sharing. The empty set is `(start=0,count=0)`. This represents arbitrary subsets such as `{A,C}` without accidentally selecting `B`.

For each `StyleThemeRef`, token-name hashes and derived-name hashes are independently sorted and unique. Identical vectors intern once; unique vectors sort lexicographically and pack contiguously, without overlap sharing, into `StyleTokenNameRefList` and `StyleDerivedNameRefList`. Empty vectors use `(0,0)`. These are the only backing tables for the two ranges; dangling or overlapping-subvector interpretations are invalid.

### 10.3 Joins and binding semantics

`Nodes.staticStyleClassSetRef` is `0xffffffff` for no static set; a valid empty
set remains distinguishable. `SelectStyleClass` owns a complete, unique
discriminant-to-class-set `arms` range; missing, duplicate, or out-of-domain
arms fail validation. `SetStyleVar` carries the §6 serializer/unset policy.
`SetThemeScope` targets one exact EXWE-admitted theme record through a
`StyleThemeRef` whose `themeFamilyDigest` **and** `resolvedThemeDigest` both
match the manifest/EXWE row; family identity alone is never a target.

The per-kind field invariants are total:

| Kind | Required invariants |
| --- | --- |
| `SetStyleVar` | `scopeKind=node-local`; `exprRef` produces the serialized value; `targetRef` indexes `StyleVar`; binding `scopeKind` and `serializerId` equal the target row exactly; `arms=(0,0)` |
| `SelectStyleClass` | `scopeKind=node-local`; `exprRef` produces the closed bool/enum/option discriminant; `targetRef=0xffffffff`; `serializerId=none (0)`; `arms` is complete and unique |
| `SetThemeScope` | `scopeKind=theme-scope-paint`; node is a compiler-declared nested theme-scope owner, never the document root; `exprRef` has result type boolean; `targetRef` indexes one exact `StyleThemeRef`; `serializerId=none (0)`; `arms=(0,0)` |

There is at most one `SetThemeScope` binding per node; multiple candidates or
priority selection reject rather than inventing last-writer semantics. On
initial SSR/CSR emission, the binding is evaluated at the request-captured or
current environment respectively: true emits the structurally owned
`data-ex-style-root` marker plus target identity/environment attrs; false emits
the marker only. Later true/false changes atomically stamp/remove those attrs,
never the marker. A nested scope never writes `data-ex-style-doc-root`,
`data-scheme`, or `data-scheme-preference`, never changes the root arena table,
and its environment attrs are diagnostic identity only—not §7.1 conditions.

Every discriminant value ref points to a constant expression and an arm range
is sorted by canonical expression bytes. The global `StyleSelectArm` table
packs ranges in canonical `StyleBinding` row order.

These targets exist **only on `StyleBinding`**. The ordinary LLP 0485 `Bindings.target attr id` schema is unchanged. The existing global `Deps` table gains a v1.4 `styleBinding` target kind whose `targetRef` points to a `StyleBinding` row; style bindings do not invent a second dependency range. This is a declared format addition, not a silent attr extension.

### 10.4 Validation and custody

`ValidatedPlan` checks every ref/range, exact table/column layout, canonical row order, packed-list construction, unique class-set/theme-name vectors, derived spellings, serializer/type compatibility, complete select arms, theme schema equality, page-route ownership, known custody bits, and the absence of inline escape kinds. It verifies every class hash/rule digest and every variable/token/derived hash join against the active manifest before handlers activate. For `SetThemeScope` it requires the target row's family/resolved/token-schema digests and both backing name lists to equal the manifest's exact record, requires that exact family/environment/result row in EXWE, and verifies its selector is present in the initially loaded token asset. A dangling, same-family/wrong-resolved, outside-EXWE, fenced-external, wrong-scope, non-boolean, or duplicate-candidate target is invalid. A `StylePageRef` is usable only while its route owns the active page sheet under §9.2.

### 10.5 Named LLP 0485 amendment proposal

**Amendment proposal A-0520-STYLE-TUPLE (target: LLP 0485 §3.4 and §12.5):** v1.4 adds `styleBundleDigest`, `targetFeatureProfileDigest`, `environmentTableDigest`, and `prePaintBootProgramDigest` to the deployment/resumption/first-frame identity tuple:

```text
(planDigest, formatVersion, opcodeTableDigest,
 locale, catalogDigest,
 themeDigest, tokenSchemaDigest,
 styleBundleDigest, targetFeatureProfileDigest,
 environmentTableDigest, prePaintBootProgramDigest,
 firstFrameDigest?, lociDigest?)
```

`styleBundleDigest` uses §5.7 SHA-256 and already commits to the latter two digests; their explicit tuple positions let pre-paint delivery reject a mismatched program/table before the core is otherwise consumed. This proposal must be adopted through `format-schema.json` and its generated TS/Rust readers/goldens before Track R consumes the segment. Current-module Track W may use the same tuple in its web manifest without claiming a plan-format change.

The amendment deliberately spells `locale, catalogDigest` as separate tuple
fields. LLP 0485 §3.4's prose currently groups them as
`locale + catalogDigest`, but the landed `format-schema.json` authority already
uses separate fields; adopting A-0520-STYLE-TUPLE amends the prose to that
schema form rather than regrouping the schema.

## 11. SSR, adoption, and hydration correspondence

### 11.1 Delivery and no FOUC

Exact assets appear in logical order in `<head>` before body streaming. Linked assets are immutable, same-origin, and carry exact `href`, `integrity`, byte length, role, and SHA-256 identity from the delivery manifest. Inline `<style>` text is the exact canonical bytes. The content-addressed pre-paint program carries its version, byte length, SHA-256/SRI, and embedded `environmentTableDigest` from the same tuple and precedes any author-owned element. F6 records linked/inline mode and all transferred style/program/environment-table bytes.

### 11.2 Adoption verification

Before DOM mutation or handler activation, resumption:

1. verifies the complete tuple and compares `planDigest` first. Any plan-digest
   difference—including one caused only by the digest-bearing `styles` segment
   or Nodes style column—exits to LLP 0386's Stage-4 clear+CSR fallback; steps 2–8 and
   §11.4 path 1 are ineligible. With equal `planDigest`, a difference limited
   to style bundle/profile/environment/boot/assets/page custody takes path 1;
   every other identity mismatch remains ordinary hydration failure;
2. verifies each inline asset by hashing its UTF-8 text bytes;
3. verifies each linked node's resolved immutable URL, SRI, role, and successful load;
4. verifies Exact-owned nodes appear in manifest-relative order while ignoring foreign sheets between them;
5. verifies the active page-sheet owner;
6. verifies both document/style-root markers, root family/digest/environment attributes, the environment-table digest, and the boot-program digest, then adopts that exact theme table;
7. verifies each node's Exact-owned class subset and registered variable names/values against the tuple-owning manifest selected by §11.4; and
8. only then binds handlers/state under the LLP 0386 activation fence.

Marker-only adoption is forbidden. Unknown third-party classes/attrs/vars/sheets do not enter Exact digests.

### 11.3 Named hydration amendment proposal

**Amendment proposal A-0520-HYDRATION-CORRESPONDENCE (target: LLP 0386 §3.1 Stage 3, “total correspondence,” and §3.8.1's activation fence; LLP 0210 §4.5–4.6):** class-first presentation correspondence is:

```text
stylesheet tuple + Exact class subset + registered local variable state
+ theme/environment attrs + existing semantic/live-property correspondence
```

This is a precise presentation-specific amendment to LLP 0210 §4.6's
whole-document byte predicate, not a claim that §4.6 was only about `class`.
What survives in `css` mode is exact tag/namespace, anchor/structural order,
text-guard bytes, capability-seeded values, and exact canonical Exact asset
bytes. Attribute serialization is instead compared semantically:

- `class` compares the exact manifest-known Exact class **set**, not the
  attribute string; permutation adopts, unknown third-party classes are
  outside Exact correspondence, and a missing/extra known Exact class fails;
- `style` compares the exact registered `--ex-v-*`/`--ex-h-*` name set and
  each canonical typed value/sentinel, independent of declaration order.
  Unknown third-party declarations/variables remain outside correspondence;
  an Exact-owned ordinary inline presentation declaration remains forbidden;
- attribute order never carries identity. Exact-owned attributes compare by
  canonical name/value, while unknown attributes remain outside Exact
  correspondence; and
- HTML boolean spellings compare by presence/live semantic state, so
  `disabled`, `disabled=""`, and `disabled="disabled"` are equivalent when
  they parse to the same owned state.

The old whole-document serialization diff may remain a producer diagnostic,
but it is not an adoption predicate for those four classes. The mirror retires
`styleDeclarationsMatch()` in `css` mode. The direct host extends LLP 0386
Stage 3 total correspondence with the rules above. Both preserve unknown state
and preserve input/focus/selection/scroll exactly as LLP 0386 requires.

### 11.4 Style-asset mismatch versus node-correspondence fallback

Section 11.2 classifies the mismatch once; the selected machine is mandatory:

1. **Style-asset plane:** a §11.2-eligible mismatch enters asset
   reconciliation. The runner keeps SSR CSS active and the DOM
   inert, loads the expected assets disabled, and verifies them. If the
   candidate's non-style structure/semantics correspond and the existing DOM's
   Exact class/var state is internally valid against the **observed** SSR
   manifest, one activation-fenced commit switches enabled Exact assets, page
   custody, root bundle/theme markers, and candidate-required Exact class/var
   state without clearing the DOM. Failure to load/verify leaves the original
   styled document inert; PE-classified forms retain document semantics.
2. **Node-correspondence plane:** a tag, text, anchor, semantic/live-property,
   theme-scope, registered-var, or per-node Exact class-set mismatch against
   the manifest that owns that node is LLP 0386 Stage 3 failure. Stage 4 clears
   the server tree and CSR-commits the prepared candidate exactly as LLP 0386
   §3.8.1 requires. It does not use the asset-swap exception.

An eligible internally coherent asset set therefore takes path 1; a
stray/missing known class on one node takes path 2. Asset reconciliation is a
named exception to Stage 4 only for that classified mismatch. It never clears to an
unstyled frame and never rebuilds a successfully corresponding DOM; node-plane
failure retains 0386's full CSR fallback with no implementer choice.

### 11.5 First-frame variables

SSR serializes initial typed variables/sentinels under §6.3. The boot program
preserves that SSR theme/variable frame; environment-dependent values and
themed images patch only in the coordinator's first post-adoption atomic
commit. First-frame records remain keyed by the serialized SSR `themeDigest`
and locale as LLP 0485 requires.

## 12. Third-party mutation and stylesheet custody

The class writer uses `classList.add/remove` for only manifest-known Exact classes; it never assigns `className`. It sets/sentinels only registered variable names and diffs only renderer-owned semantic attrs. Unknown classes, attrs, custom properties, and stylesheets survive.

If an owned class or variable is removed/changed, the next relevant reconciliation restores the exact desired state without node replacement. A wrong known Exact class is removed. Development may observe the subtree; production does not install a permanent whole-document observer merely to police classes.

Exact-owned style/link nodes are different: they are immutable bundle custody. A narrowly scoped observer on those nodes detects text, `href`, `integrity`, order, disable/media, or removal mutation and atomically reloads the expected asset or fails the root inert. It never observes/deletes/reorders foreign stylesheets.

## 13. Accessibility and print obligations

The unlayered Exact accessibility asset must cover forced-color focus visibility, borders when fills/shadows disappear, state distinction without color alone, decorative gradient/shadow suppression, and audited `forced-color-adjust`. Reduced-motion corrections use the coordinated preference attribute so theme tokens and semantics move together. A raw literal color is not exempt.

Real semantic DOM remains browser-owned. Exact's class contributes one class of specificity; scaffolding is in `:where()`. Normal app author CSS may override under the ordinary cascade. Normal user-origin rules do not automatically beat author-origin rules; user-origin `!important` does.

Print conformance requires JS-disabled readable document order, semantic/link structure, page margins, useful undisclosed content, and no fixed/animated/overflow presentation that clips primary content. Verification inspects computed styles and rendered print/PDF, not substrings.

## 14. One design across web tiers

| Phase | Producer | Consumer | Gate |
| --- | --- | --- | --- |
| A | canonical IR/bundle behind flag | `dom-mirror.ts` compare mode | byte determinism, classifier totality, computed parity |
| B | same bundle | production `web-host-direct.ts` plus mirror compatibility | inline budget, hydration, theme, media/print, mutation |
| C | same bundle + accepted v1.4 rows | wasm plan runner | reader/host equivalence and SSR adoption |
| D | class-first default | all admitted Contract web tiers | checkpoint evidence; bounded legacy removal |

The shared DOM writer applies class sets, typed variables, theme attrs, and adoption checks for both current hosts. The wasm host ABI exposes only those operations and page/style custody. Rust owns plan semantics; the browser owns cascade/layout/paint; neither runtime parses arbitrary CSS.

F6 uses the wasm plan runner, v1.4 style profile, canonical bundle, production SSR/resumption, zero ordinary inline presentation, all style bytes in payload totals, and LLP 0485 §14.2 REAL-input evidence. A legacy mirror capture is invalid.

## 15. Verification program

### 15.1 Checks to register

These ids are proposals until complete `exact-verify.json` rows exist. The first implementation exercising a row must register owner, systems, platforms, lanes, inputs, artifacts, command/manual procedure, expected runtime, and profile placement.

| Check | Class | Obligation |
| --- | --- | --- |
| `web-style-schema-parity` | command | validate LLP-owned tag/zero assignments, closed `exact-web-css-v1`, boot grammar/source, and generated emitter/reference/golden freshness with `generate-webstyle-schema.mjs --check` |
| `web-style-css-determinism` | command | golden RuleKey/condition/value/CSS/manifest bytes across path/order/Bun/Node permutations |
| `web-style-class-identity` | command | theme-only stability, every collision namespace, property-order/equivalent-AST permutations |
| `web-style-classifier-totality` | command | every `Expr.kind`, property projector, optional/presence, token algebra, gradient, motion object, RecipeIR gate |
| `web-style-writer-census` | command | every direct style writer maps to §6.4; zero uncatalogued writer |
| `web-style-cascade-order` | browser | overlapping-condition reversal, unequal-length rank-vector counterexample, and specificity/layer fixtures |
| `web-style-inline-budget` | browser/command | zero ordinary/`--sr-*`; registered vars/residue counted |
| `web-style-theme-transaction` | browser | SSR-A/client-B adoption, single-owner boot handoff, document-only conditions, root-table/nested-paint split; digest/table/token reads/pixels agree at each commit |
| `web-style-media-print` | browser/fleet/manual | forced colors, viewport, JS-disabled print/PDF, page activation |
| `web-style-ssr-adoption` | browser | inline/linked bytes, URL/SRI/order, no FOUC, no duplicate, bake-and-patch |
| `web-style-digest-mismatch` | browser | §11.2-eligible atomic swap/inert failure; any plan difference takes Stage 4 |
| `web-style-dom-mutation` | browser | foreign preservation; owned class/var/sheet repair without identity loss |
| `web-style-tier-equivalence` | browser | mirror/direct/wasm computed styles and semantics |

Focus/state distinction fixtures must assert named computed system-color/border/outline outcomes and semantic state, plus a rendered forced-colors/print oracle where computed values alone are insufficient. Browser, paint-trace, forced-colors, and print legs run on the qualified fleet/manual lane rather than masquerading as platform-neutral command checks.

### 15.2 Required negative corpus

The corpus includes:

- reversed overlapping conditions, the `[2,0,2]` versus `[2,0,1,1]`
  part-lex/count-prefix mutant, and a hash-order mutant;
- reordered properties and equivalent condition ASTs;
- missing/duplicate/tombstone-reused schema ids, an unassigned-zero mutant,
  renumbered canonical-zero/EXW-tag/framed-row mutants, absent/present/empty
  EXWB toggle-prefix cases, theme-family-preimage field/order/optional goldens
  and mutants, incomplete v1 property grammars, a wildcard profile, stale
  generated emitter/reference tables, and profile-preimage drift;
- golden `spring-fast-ease-out-v1` default/authored-duration cases and a mutant
  that lets damping/stiffness alter the registered fixed curve;
- NaN/infinity/negative-zero/Ryū boundary numbers;
- nested same-site variables with unset/ancestor inheritance;
- descendant token/derived inheritance from document and nested theme scopes,
  with a mutant `inherits:false` registration that must fail; nested static CSS
  must read nested values while a plan `$token` fixture reads the root table;
- optional declaration presence and Color/Gradient sum switching;
- current `$token` classification and unsupported token rounding;
- theme-dependent recipe shape and continuous recipe rejection before RecipeIR;
- SSR-A/client-B flip with tuple adoption, exact one-owner handoff, first
  coordinator commit, and Contract token-read/theme-table parity;
- exact inline tampering, linked URL/SRI/order tampering, and post-adoption sheet mutation;
- mismatched/stale pre-paint program or environment table; wrong/missing root
  family digest; scheme/reduced-motion/contrast query-string drift; width API,
  threshold, storage-key, legacy-migration, or text-scale drift; and an admitted
  table record missing from initially loaded `exact.tokens`;
- duplicate diagnostic family ids with distinct correctly joined family
  digests, plus a mutant that joins by the id and must fail;
- streamed nested scopes arriving after boot: true stamps marker plus identity/
  environment; false stamps only the marker; pre-paint touches neither;
- `SetThemeScope` outside EXWE/initial CSS, wrong kind/scope/serializer,
  non-boolean activation, duplicate candidates, false/removal inheritance,
  nested-attr condition-selector mutant, and fixed-record root flips;
- class-attribute permutation, style-declaration permutation, attribute-order
  permutation, and equivalent boolean spelling that must adopt, contrasted
  with a changed Exact class set or registered var that must enter Stage 4;
- an eligible coherent asset mismatch that reconciles atomically, contrasted
  with simultaneous style+plan or per-node mismatch, both of which must clear
  and CSR-commit under LLP 0386 Stage 4;
- post-adoption owned class/var removal and repair;
- multiple roots, inactive/prefetched page sheets, page-owner conflict; and
- ShadowRoot rejection before mutation.

### 15.3 Inline and payload receipts

The production author-owned budget is:

```text
ordinary inline declarations = 0
unregistered custom properties = 0
legacy --sr variables = 0
legacy inline escape nodes = 0
```

Receipts separately count `--ex-v-*`, `--ex-h-*`, host-residue nodes/properties, logical and transferred CSS bytes, `planDigest`, `styleBundleDigest`, profile/theme/token/first-frame digests, and delivery/compression mode.

## 16. Failures and diagnostics

Stable build failures include `web-style-schema-invalid`, `web-style-schema-drift`, `web-style-profile-not-closed`, `web-style-exceptional-number`, `web-style-opaque-object`, `web-style-unsupported-property`, `web-style-unsupported-value`, `web-style-dynamic-rule-shape`, `web-style-unbounded-condition`, `web-style-cascade-ambiguity`, `web-style-recipe-ir-required`, `web-style-theme-scope-invalid`, `web-style-runtime-page-value`, `web-style-page-owner-conflict`, `web-style-shadow-root-unsupported`, `web-style-hash-collision`, and `web-style-inline-escape-forbidden`.

Runtime failures include `web-style-boot-program-mismatch` and `web-style-environment-table-mismatch` and report expected/observed plan, style, profile, environment-table, boot-program, theme, token-schema, and first-frame digests without user values. Development diagnostics show merge provenance, rank, condition, class/var disposition, RecipeIR/derived refs, and temporal/host owner through the loci sidecar.

## 17. Security and CSP

Only registered serializers can produce bound values; no value can introduce a selector, declaration terminator, at-rule, property name, or importance flag. Linked assets are immutable and SRI-bound. Inline canonical CSS and the pre-paint environment program follow deployment nonce/hash policy.

V1 transports dynamic values as custom-property style declarations. A CSP profile forbidding all `style-src-attr` admits only routes with zero dynamic/host variable bindings. A future nonce-backed CSSOM binding sheet needs a new profile and the same typing, inheritance, ownership, determinism, and mutation gates. Raw URLs, selectors, `@import`, arbitrary at-rules, and raw CSS are not Contract style inputs.

## 18. Compatibility and migration

One setting exists:

```text
EXACT_WEB_STYLE_MODE=legacy-inline | compare | css
```

Mode is stamped in SSR/delivery identity. `compare` displays the legacy writer and records canonical computed/classification differences; it does not let both paths own visible presentation. `css` has no per-node legacy escape. A server/client mode mismatch rejects resumption.

Implementation order is:

1. `webstyle-schema.json`, its generator/reference/emitter tables and drift gate, LLP-side tag assignments, closed `exact-web-css-v1`, boot program grammar/source, complete property/value/condition grammars, RuleKey/WebStyleIR/profile bytes, and `ResolvedStyleIR`;
2. writer census and shared class-first DOM writer;
3. non-recipe Contract extraction, vars, theme records, and canonical emitter;
4. SSR/adoption and environment transaction;
5. LLP 0497 F-A RecipeIR consumption and Facet corpus;
6. production direct-host promotion; then
7. adoption of v1.4/A-0520-STYLE-TUPLE and wasm/F6 wiring.

`static-scheme.ts` loses inline `colorScheme` only when its pre-paint/environment duties are covered. `contract-static-theme.ts` and `--sr-*` are then deleted from `css` mode. Rollback is a redeploy built in `legacy-inline`; an already-rendered page never changes mode in place.

## 19. Non-goals and maturity

V1 excludes arbitrary CSS authoring, raw selectors/nesting/`@supports`/at-rules, arbitrary TS recipe extraction, automatic structural `match size` conversion, container queries, ShadowRoot mounting, React extraction, native styling changes, and a no-style-attribute dynamic backend. React remains supported with its own web style mechanism; shared Facet truth may not fork.

| Maturity | Claim allowed |
| --- | --- |
| Design (Draft r5) | decidable implementation input; no shipped gate claim |
| Experimental | non-recipe compare fixtures and canonical artifacts |
| Facet-capable | LLP 0497 F-A RecipeIR gate and Facet corpus green |
| Current-runner production candidate | mirror/direct CSS/adoption gates green |
| Plan-runner experimental | accepted v1.4 and wasm equivalence green |
| F6-admissible | production bundle, zero ordinary inline, SSR/REAL-input receipt |
| Default | checkpoint evidence and legacy removal date |

## 20. Falsifiers

The design or implementation is wrong if any occurs:

1. Equal canonical inputs produce different RuleKey, CSS, manifest, or class bytes.
2. A theme-value-only change renames app/Facet classes or changes plan rows outside theme/style manifest identity.
3. A theme/environment flip walks app nodes, re-runs recipes, exposes mismatched `data-ex-theme`/theme-table identity, or paints a mixed frame.
4. An author-owned production node has an ordinary inline property or `--sr-*`.
5. A dynamic value generates a selector/rule/class or passes an untyped string.
6. Nested unset bindings inherit an ancestor instance's value.
7. Hash order, rather than authored `cascadeRank`, changes an overlapping-condition winner.
8. Structural `match size` and named size-class CSS diverge, or a nested environment attr qualifies an app rule.
9. Forced colors loses required focus/state distinction, or Exact needs a root `forced-color-adjust:none` escape.
10. Inactive/prefetched route `@page` rules affect the active document.
11. SSR paints unstyled, adopts tampered/reordered Exact assets, or duplicates a matching asset.
12. A §11.2-eligible asset mismatch clears instead of reconciling/failing inert, or any plan/node mismatch takes that exception instead of LLP 0386 Stage 4.
13. Foreign state is destroyed by an Exact commit.
14. Removing/changing an owned class or variable is not repaired at the next reconciliation, or repair replaces the node/loses input, focus, selection, or scroll.
15. Mirror, direct host, and wasm need different CSS emitters or disagree on computed style.
16. A Facet recipe enters CSS/F6 without complete RecipeIR.
17. A v1.3 reader silently consumes v1.4 style semantics or a v1.4 web profile admits a missing/mismatched style segment.
18. F6 omits style bytes, uses legacy inline presentation, or lacks style/profile/residue identity.
19. A nested theme scope changes the v1 root arena table, or a plan `$token` read observes a hidden nested overlay without `A-0520-MULTI-RECORD-THEME-TABLE`.
20. Pre-paint changes SSR paint state to client B before adoption, joins by family id, derives thresholds outside EXWE, treats zoom as text scale, touches nested scopes, or boot/coordinator ownership overlaps or lapses.

## 21. Closed decisions and later work

V1 decisions in this document are closed: one generated-table-backed TypeScript emitter; explicit canonical-zero and framed-record assignments; the closed `exact-web-css-v1`; full initially loaded theme records; SSR identity preserved through boot/adoption, then one coordinator commit; document-only app environment conditions; CSS-only nested scopes with root-table token reads; the inheritance split; RecipeIR as the Facet prerequisite; part-lex rank emission; local invalid unset semantics; light-DOM roots; one document page owner; the plan-first hydration mismatch split; and the v1.4 amendment proposal. No Charlie decision is required to implement r5.

Container queries, ShadowRoot `StyleRealm` support, React extraction,
`A-0520-MULTI-RECORD-THEME-TABLE`, a physical spring solver profile, and a
nonce-backed no-style-attribute binding backend are later proposals, not
unresolved v1 choices.
