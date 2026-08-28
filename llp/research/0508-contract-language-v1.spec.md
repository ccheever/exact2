# LLP 0508 — Contract Language Specification, Edition 1

**Type:** Spec
**Status:** Accepted

**Scope freeze (2026-08-20, author-authorized extension rounds):** this
specification's normative surface is closed as of r3. Extension
revisions make corrections and restorations only; demands for new
normative closure are recorded as obligations, never folded as new
normative prose. Delta reviews judge the r(N-1)→rN diff and the
punch-list dispositions.

**Systems:** Contract, Compiler, Runtime, Conformance
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-20
**Revised:** 2026-08-26 (**three §14 records + §13 items 20–22**, executing 0504 §3 rows 52c, 53c and 6 as independent numbered amendments in acceptance order, per LLP 0551 §3 decision 4 — they are NOT members of the 0523/0531 atomic batch. Item 20 / RFC 0536 (Accepted 2026-08-22): the `text raw=` never-localized opt-out, logical layout attrs, and the `has text key=` selector, with the string-id authority left to LLP 0485 §3.3.15’s frozen EPLC schema. Item 21 / RFC 0537 (Accepted 2026-08-22): the deferred-binding annotation and footprint-policy grammar, with the `when pending`-vs-default spelling left to 0537 OQ2, deliberately unpicked. Item 22 / RFC 0499 C-3: a **routing record, not an amendment** — the `use … from capability(…)` grammar is re-homed onto this clock because the 0485 grammar window closed with this edition’s freeze, and no numbered amendment can be minted until C-3’s owner specifies the grammar. Edition 1, the `contract-v1` flag, and the one-migration rule unchanged; none is a pre-flip obligation. 0506 D1(d) burn-down lane.) 2026-08-26 (§10.3 target-liveness open issue gains a dated recording — RFC 0540's `VisibilityTransition` family is a second confirmed customer of the **reduced** `(address, lifetimeGeneration)` fence, entry-only `stale-target` suppression, so the generalization is neither resolved nor blocked by the lists work; §8.3 Region semantics confirmed unchanged by the `CollectionProjection` projection. Executes 0504 §3 row 66's 0508 half; recording only, no normative change. 0506 D1(d) burn-down lane.) 2026-08-25 (**Numbered amendment — The Dynamic Ceiling (RFC 0518) adopted**: the 0481-OQ1-in-W2 dynamic-ceiling decision taken by Charlie Cheever, 2026-08-25 (decision relayed via orchestration session exact-9e), executed through this document per RFC 0518 ask 1 — Ruling A confirms Edition 1's own incorporated 0485 §6.2 text as the shared data-access rule with the escape hatch unavailable for data access; Ruling B closes the computation-escape inventory at exactly three island lanes; the initial discretionary whitelist is empty (0518 OQ2 decided: nothing). Recorded as §13 item 19 + the §14 "Dynamic Ceiling" numbered-amendment block; Edition 1, the `contract-v1` flag, and the one-migration rule unchanged.) 2026-08-20 (**ACCEPTED by Charlie, 2026-08-20 — this
acceptance IS the LLP 0505 r3 row-3 merged Contract-v1 ratification:
one version, one flag, one migration. Supersession executed per §14:
LLP 0330, LLP 0489, and LLP 0501 move to Superseded as documents while
every §14-enumerated incorporated section remains frozen incorporated
text in force, with the admission-boundary carve intact; the
`exact-contracts.json` `contract-semantics` row re-points here; the
four 0485 erratum tickets cited by 0485 Amendment 1 items 6–9 close
resolved-by-ratification. Implementation-side §14 mechanics — the
`contract-v1` flag, the one migration wave with its census, and
engine/oracle v1 conformance claimed at the flag flip — are recorded
obligations of the migration landing, not discharged by this edit.**)
(r6 — **final close-out fixes; unreviewed** (no
review covers this revision; the delta-2 verdicts bind to r5). All
three codex delta-2 findings adjudicated real and fixed: the
`stale-target` recording's provenance corrected to 0485 §7.4 (both
families' citation); the 0489 §6.1 item-5 migration-evidence limb
restored to §6.1's own timing — a precondition to this ratification,
not a pre-flip obligation; §13 item 13's A4 entry reconciled with
§14's carve (resolved by exclusion, no Edition-1 ambiguity remains).
Appendix A's §12 row completed with 0330 §8 per the A8
re-attribution.)
2026-08-20 (r5 — delta-1 fold under the scope freeze:
the §5.5 null-arm reachability clause rewritten to §6.2's scoping
(seam-normalized `null` never a live program-level key; both families'
convergent finding); the ordinal `cursor` restored to a standalone
component-scope declaration (the `each`-body confinement was not
0485's); "recorded" dropped from the decided stale-delivery fences —
replay-recording belongs to the declined 0330 §4 amendment text and
is stated as such; the A8 projection re-attributed to 0330 §8 and
§14's carve amended to incorporate it explicitly; the §14 carve now
excludes 0330 §4's async-action-segmentation/A4 clause (superseded by
§10.3's awaited actions per the decided 0505 posture); §12.2 item 7
restores §6.1 item 5's migration-evidence limb as a carried pre-flip
obligation. Rejected delta-1 items ledgered in the punch files.)
2026-08-20 (r4 — extension freeze-fold of the terminal
round-3 reviews (codex 10 MATERIAL + 3 minor; grok READY, 6 minors).
Corrections/restorations only: provenance method note rewritten to the
self-reference law (0501's whole-file hash necessarily drifts when its
§6.4 records a proof result — inputs recorded as-at-proof-run, with
the current-file hash stated; the stale "0501 §6.4 owes the refresh"
parenthetical dropped — §6.4 records `4aab4e32…` now); §12.1
registration claims made honest (the apply-order proof is NOT
registered — open ticket cited); 0330's stdlib receiver exemptions
restored in §6.6; §11's stale-delivery sentence scoped to the decided
settlement/publish fences; §2.3's event-attr authority description
corrected to the shipped shape (local `EVENT_ATTRS` aggregation,
exported flat union, exported helper accessors over the file-local
scoped map, scroll/collection applicability in
`CONTRACT_BUILTIN_ATTRS`/`builtinAttrInfo`); §5.4's value-kind
canonical components pinned (magnitude/unit; channels/color-space;
magnitude/unit); quiesce defined in place and 0330 §§4–6 added to
§14's frozen-incorporated list (fixture contract, canonicalization,
unknown-opcode hard failure, A8 projection, ambient/no-host branch);
§7.4 restores 0330 §5's installed-vs-ambient split; §11's code-owing
and shape-owing lists plus 0501 §6.1 item 6's rejection-class report
recorded as pre-flip obligations in §13, with the MUST bindings
per-class; the 0489-mandated `ephemeral-action-parameter` removal
carried into §11/§14 as a named burn-down; 0501's whole §4.1
conformance gate carried in §12.2 item 9; blockquote fence closed
before the decided rollback law; Host class pointer §7.7→§7.3;
cell-variant shared-cache qualifier per 0509; null-key reachability
clarified; ordinal-cursor wording disambiguated; §12.2 item 7
parenthetical scoped to the fixture families.)
2026-08-20 (r3 — sr-specs round-2 fold, both families
(codex 10 findings, grok 3 MATERIAL + 5 minors; zero refuted after
byte verification): provenance hash corrected to the current byte
triple (amended `4aab4e32…` — the recorded `2691cac4…` predated the
0489 ledger-refresh and 0501 D1-sync edits; independently re-run) with
the proof-rebinding rule stated; `cursor_key_mismatch` restored to
0485 §6.2's rule and the shipped diagnostic (cursor-vs-keyed-`each`
differing identity, **warning**) — the r2 two-`each` error withdrawn;
`??` restored to 0489 R3's decided text with the type-incoherence on
option-typed right operands recorded as a merge-found Open issue
(lifting = recommended resolution, not decided here); `?.`
field-flattening and the non-option payload qualifier relabeled as
derived completions of the no-nest rule; target liveness rescoped to
the decided settlement/publish fences (0501) with the universal
`InvocationTargetStamp` question opened (it was packaged in the
declined 0330 §4 amendment); the 0489 R6 exclusion inventory and
0501's classification-does-not-admit rule carried in §2.3 and made
supersession-proof in §14 (publish shape preservation qualified
accordingly); unbounded dynamic component targets get their decided
compile rejection + closure fix-it; null truthiness/interpolation rows
reachability-scoped to boundary/diagnostic surfaces; authored shape
fields stated required (no optional spelling exists; the nullable
marker is the one optional identifier, generated-shapes-only today);
stdlib split corrected (both shipped codes named; compile-time only;
`Math.random` resolves to the §7.4 seam builtin per 0330 §3/§5, the
shipped warning on it a named divergence); quiesce tuple gains the
envSnapshot ambient captures; §12.2 gains the carried 0489 §5.2/§6.1
all-engine families and hooks for deterministic Region ids, guarded
edges, and command non-reentrancy; §11's named-code claim made honest
(enumerated code-owing list, due before flip); 0489 task-assignment
and layout-summary merge deltas named (§4.6/§4.7); Status wording
aligned to "0330 stands until adopted".)
2026-08-20 (r2 — sr-specs round-1 fold, both families:
the declined 0330 inbox amendment and per-event RNG substream removed
from Edition-1 law (LLP 0505 r3 row 8 / LLP 0482 r6 — the reject branch
is v1: synchronous host-event dispatch, class order for timers and
completions only, one global RNG stream); §9 and §6.7 restored to
semantics-preserving carries of 0489 r6's boundary grammar and memo
domains; §14 supersession gains the frozen-incorporated-text carve;
Option fold completed (`??` lifting rule, `?.` option-field flattening,
non-option payload, `temporal` `nextChangeAt` delta); tasks-write-state
contradiction fixed; §8 carries 0501's snippet walk-once/parenting and
at-own-topological-position sweep placement; §8.4's cell variant stated
per the amended 0500 D1 + 0509; settlement interface fields named;
§10.4 compiler obligations made explicit; shipped-code anchors
corrected (CONTRACT_SCHEMA_EVENT_ATTRS, oracle conformance gated on
migration, stdlib warning-today/reject-at-flag, closed-types/cursor);
provenance fixes (0501 r5, 0501-§6.4-only proof cite, Status
alignment, 0506 conjunct-(b) ask).)
**Related:** LLP 0505 r3 row 3 (the decided merged-ratification vehicle:
one version, one flag, one migration), LLP 0330 (Contract semantics v0 —
the frozen base this edition consolidates; **governs until this edition
is ratified**), LLP 0489 r6 (closed-types amendment — proposed; adopted
at this ratification), LLP 0501 r5, hash note refreshed after 0489 r6
(regions amendment — proposed; adopted at this ratification),
LLP 0509 (Aquifer cell identity/readiness — the landing vehicle for
§8.4's cell-variant refinement), LLP 0485 §§5–9 (flat-plan
semantic changes 1–5 — incorporated per §10), LLP 0480 (the plan is the
program), LLP 0328 (execution tiers, D10 feature matrix), LLP 0186
(scheduler invariants), LLP 0082 `llp/contract/` (runtime/reactivity),
LLP 0341 (`ReplayValueEncodingV1` artifact authority), LLP 0292 (host
bindings, excluded), LLP 0337/0345 (declarative windows, excluded),
RFC 0401 (canonical identity family), LLP 0500 D1 (HMR preservation
spine), LLP 0505 r3 row 10 (Option-only absence, decided)

## Abstract

This document is the consolidated specification of the Contract
language, **Edition 1 ("Contract v1")**: the closed type system, the
three value relations, expression semantics, the reactive execution
model, Region semantics, boundary parsing, and the flat-plan execution
model. It merges the semantics of LLP 0330 (v0) with its two adopted
amendments — LLP 0489 (closed types) and LLP 0501 (regions) — and the
five semantic changes of LLP 0485 §§5–9, into one document with one
version, one adoption flag, and one migration, per the decided
LLP 0505 r3 row 3. The two amendments are proposed documents adopted
by this ratification, not previously adopted text.

## Status of This Document

**LLP status: Accepted — RATIFIED 2026-08-20** (the LLP 0505 r3 row-3
merged Contract-v1 ratification, applied by the author). **This
document is now the governing language authority.** The paragraph
below records the pre-ratification posture for the historical record;
each "on ratification" mechanic listed has been executed (see the
Revised entry): *until it was ratified, LLP 0330 remained the
governing language authority (0489/0501 were proposed amendments) and
this document was a restructuring of already-adjudicated text, not a
second source of truth.* On ratification (a single owner decision,
per LLP 0505 row 3):

- this document becomes the `exact-contracts.json` language-semantics
  authority row;
- LLP 0330, LLP 0489, and LLP 0501 move to `Superseded` (their
  adjudication records and redline provenance remain the historical
  record);
- LLP 0485 §§5–9 remain frozen text, **incorporated by reference**
  where §10 says so — this edition does not fork 0485's tables;
- the per-amendment feature flags collapse into **one language flag**
  (`contract-v1`), and the migration ships once.

**Method note (provenance).** The semantic source of truth for the
merged base is the amended-0330 text produced by applying LLP 0489 r6
and LLP 0501 (r5) to LLP 0330 in either order — the two redline sets
are disjoint and both orders are byte-identical. **The binding inputs
are the pinned 0330 bytes and the two documents' quoted-old/new
redline blocks** (each validated by exactly-once match); the proof
re-runs whenever either amendment's redlines change. The proof is now a
registered check — `contract-apply-order-proof`
(`scripts/check-apply-order-proof.mjs`), in the `contract` and
`contract-ci-core` profiles — so it re-derives itself from live bytes
on every run instead of being recited. Recorded run (2026-08-26,
fourth): 0330 `316d5d45…` + 0489 `72479210…` → amended SHA-256
`f60c5ce651cb4f2f7e1910b5e8be07d99cb21ebc23b3109f547a469047615c36`,
also recorded in 0501 §6.4. The prior recorded run (2026-08-20): 0330
`bb180ec7…` + 0489 `d380ec96…` + 0501-at-proof-time `e90ec286…` →
amended `4aab4e32…`; it went stale when LLP 0330 itself moved, which
is why the registration exists. **Self-reference law:** 0501's
whole-file hash necessarily drifts the moment its own §6.4 records a
proof result, so 0501's file hash is never recorded here as a
current-file claim; that drift changes no redline block and therefore
no amended output. (The
apply-order proof lives in **0501 §6.4**; 0489's §6.4 is its
rejection-contingency section, not the proof.) Where this
edition restates
that text, the restatement is intended to be semantics-preserving;
where a conflict is found, the amended-0330 bytes win and the
restatement is the bug.

## 1. Introduction (non-normative)

Contract is a declarative UI language with an indent grammar, a closed
value model, explicit reactivity (`state` / `derive` / `action` /
`task`), and structural choice expressed as Regions. A Contract
program compiles to a **flat plan** — a total, statically analyzable
program encoding (LLP 0480: "the plan is the program") — executed by a
plan runner. The language is deliberately closed: no `any`, no open
objects, no observable `undefined`, no reference-identity aggregates,
exactly one absence form, and every construct either lowers to the
plan, is declared at a typed boundary, or is rejected by name.

## 2. Conformance

### 2.1 Requirement language

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be
interpreted as in RFC 2119. Text marked *(non-normative)* and all
examples are informative.

### 2.2 Conformance classes

1. **Compiler** — consumes Contract source, produces IR and plans;
   owns every compile-time obligation (typing, inference,
   exhaustiveness, rejection classes and fix-its, Region
   classification, dependency resolution, effect rows, budgets).
2. **Plan producer** — any tool emitting or transforming the plan
   container; owns schema-total field coverage (§10.4) and the
   generated-authority discipline.
3. **Plan runner / engine** — executes plans; owns the scheduler
   invariants (§7.2), the committed-update algorithm (§8.5), the
   determinism seam, and the fail-closed rejection of constructs
   outside its admitted feature matrix (LLP 0328 D10). The production
   engine target is the single Rust engine of LLP 0505 row 1; the
   **TS tree-walking interpreter is the retained conformance oracle**
   (LLP 0505 row 2(b)). No engine is a conforming v1 engine today:
   the shipping walker predates this edition (it admits authored
   `null` and lacks the option operators) — its v1 conformance is
   **reached by the one migration (§14)** and claimed only at the
   flag flip, never asserted of pre-migration behavior.
4. **Oracle implementation** — the reference implementation whose
   goldens define fixture expectations: the TS runtime, under the
   same §14 migration gate.
5. **Host** — a platform embedding (web DOM, Apple, Windows, worker);
   owns event delivery, presenter application, and the host seams this
   edition constrains (boundary parsing §9, host-observable equality
   §7.3).

A claim of conformance names its class(es) and, for engines, the
LLP 0328 D10 feature-matrix row it admits. An engine MUST reject an
unadmitted construct **by its actionable name**; it MUST NOT skip,
weaken, or silently reinterpret admitted IR.

### 2.3 Authorities this edition does not own

- Tag and attribute inventories: `CONTRACT_TAG_LOWERING`
  (builtin-lowering.ts) and, for event attributes, the shipped
  builtin-schema.ts surfaces as they actually divide: the exported
  `CONTRACT_SCHEMA_EVENT_ATTRS` **flat event-name union** (built over
  the file-local `EVENT_ATTRS` aggregation — core plus scroll,
  collection, reserved, and pointer spellings — and the per-media/input
  families); **tag applicability lives in two places** — the
  file-local `TAG_SCOPED_EVENT_ATTR_TAGS` map, consulted only through
  its exported helpers (`scopedEventAttrTags` /
  `contractTagScopedEventAllowed`), scopes the collision-sensitive
  media/input spellings, while scroll/collection/reserved
  applicability is carried by `CONTRACT_BUILTIN_ATTRS` entries and
  answered by `builtinAttrInfo`. No single exported pair is the
  complete applicability authority; the roster authority is this file
  as a whole. These live in
  `packages/exact-contract/src/compiler/`; this edition constrains
  their *semantics*, not their rosters.
- Protocol opcodes: `tests/protocol/protocol-inventory.json`.
- Replay artifact bytes: LLP 0341 (`ReplayValueEncodingV1`;
  `exact-contracts.json` row `contract-replay-semantics`). §5.4
  defines the *relation*; 0341 owns the *encoding*.
- Host bindings (`host` declarations): LLP 0292. Declarative windows:
  LLP 0337/0345 (excluded; §8.6). Data-cell lifecycle beyond §9:
  RFC 0082 / 0194 / 0259 / RFC 0498's program.
- **The carried exclusion inventory (no-widen rules, in force).**
  Per 0489 r6 R6's exclusion sentence: query, mutation, and ephemeral
  data surfaces; resource-cell lifecycle semantics other than §9's
  shaped boundary parse and closed binding types; channels/`publish`
  execution semantics; measured-sibling feedback and resolution beyond
  §6.8's direct-sink relation intents; behavior templates beyond
  same-file inlining; forms; overlays; JS islands; and the capability
  ABI remain **matrix-gated or post-v1 per LLP 0328 D10**, their
  semantics owned by their LLPs and TS tests until a later revision
  admits them. And per 0501 R1: **Region classification is narrower
  than admission** — classifying `resource`, `async`, `boundary`,
  `publish`, or graphics preserves every field and rejects
  unsupported execution by the construct's actionable name;
  classification alone never makes a cell arm reachable, authorizes an
  effect, or implements an evaluator. Ratifying this edition does not
  widen either rule (§14).

## 3. Terminology

- **Closed universe** — the set of value types §4 admits. Everything a
  Contract program stores, passes, or publishes is in it.
- **Slot** — a component instance's storage cell for one `state` or
  `derive` result, at a build-time offset.
- **Region** — the semantic unit of structural choice, keyed instance
  ownership, cell-arm ownership, composition, graphics delegation, or
  component-root ownership (§8).
- **Logical instance / logical address** — an element of
  `RegionActivationAddressV1` (§8.4): where a thing lives in the
  instance tree, independent of storage.
- **Lifetime generation** — the per-logical-address monotonic counter
  of §8.4. **Key-set generation** — the per-collection counter bumped
  on any ordered-key-sequence change. These are always named
  explicitly; "generation" never means both.
- **Plan** — the flat program encoding of LLP 0485 §3; **transition**
  — one `(state, event, envSnapshot) → (state′, Command[])` reducer
  step (§10.3).
- **Fix-it** — the mandatory narrow resolution carried by every
  rejection (§11).

## 4. The type system

### 4.1 Value kinds

The inhabited primitive types are `null`, `boolean`, `number`
(IEEE-754 binary64, including `NaN` and both infinities), and `string`
(UTF-16). `never` is uninhabited. The closed type language further
contains:

- **string enums** — finite closed sets of string literals;
- **sums** — uniquely tagged unions of closed-object arms (§4.3);
- **closed objects** — named fields, fixed layout, no additional
  properties;
- **arrays** — one element type;
- **value-kinds** — `dimension`, `color`, `duration`;
- **action/callback references** — plan-addressable, effect-rowed
  (§4.5); arbitrary host functions and closures are not storable
  Contract values;
- the sink-only **`layout-relation`** type (§6.8) — not storable;
- **`Option<T>`** — the built-in option sum (§4.2).

`null` is a **boundary-only** value kind: no authored type contains it
(§4.2), so a checked program observes `null` only inside the §9
boundary parser, which normalizes it away before a value enters the
program.

**No observable `undefined`.** `undefined` is not a Contract value and
has no type, literal, absent-field arm, optional-access result,
interpolation arm, or out-of-bounds indexing result. Every
closed-object field is present. An expression that could produce
`undefined` rejects with the narrowest fix-it to an option, exhaustive
match, the §9 boundary parser, or a restructure. A runtime MUST NOT
synthesize `undefined`.

### 4.2 Absence is Option-only

`Option<T>` is the language's **single absence form** (decided:
LLP 0505 r3 row 10).

- `T | null` is **not a type**: the prop-type grammar has no spelling
  for it; inference never infers a `null` arm; an initializer or
  assignment constraint that would join `T` with `null` is a compile
  error whose fix-it is `Option<T>` with explicit `some(...)`/`none`.
  No authored, stored, passed, or returned value is `null`-typed.
- The IR's `nullable` marker is retained as a **boundary-parse
  annotation only**: it never denotes an authored type, and it lowers
  at the §9 `shape` seam under the **seam-normalization rule** — a
  parsed `null` (or absent optional field) normalizes to `none`; a
  parsed defined value `v` normalizes to `some(v)`. The same
  normalization governs every host seam that formerly produced
  nullable data, so `null` crosses into a checked program **nowhere**.
- There is exactly one absence semantics: `none`. Optional access,
  `find`-style misses, out-of-bounds indexing, and boundary absence
  all produce it; the eliminator (§6.4), `?.` (§6.3), `??` (§6.3),
  and the `opt == none` emptiness test (§5.2) all consume it.

**The canonical option type.** `Option<T>` is a built-in sum defined
by the language, not an authored discriminated-object sum: its arms
are `some(T)` and `none`; its constructors are language metadata (so
the payload may be any **storable, non-option** closed type,
primitives included — the *non-option* qualifier is the
construction-site reading of 0489 R3's no-nest rule ("any storable
closed type" bounded by "no `Option<Option<…>>` is constructible …
anywhere"), a derived completion marked as such; the
shared-discriminant restriction of §4.3
governs *authored* sums only; `layout-relation` is not storable and is
not an admissible payload); it is matched exhaustively like any sum.
Its canonical object-compatible encoding, used wherever an option
crosses a plain-data boundary, is
`{kind:"some", value: T} | {kind:"none"}`; its logical layout is the
ordinary sum layout. **No `Option<Option<T>>` exists anywhere in the
language**, by three co-operating rules: the type grammar rejects a
nested option type expression (fix-it: flatten, or restructure as an
authored sum when both absence layers are semantically distinct);
`some(e)` where `e` is option-typed is a compile error at the
construction site (the payload constraint above); and §6.3's operators
flatten by construction. `none` is the only bottom of the absence
lattice — there is no distinct "present but empty" option value.

**Syntactic closure.** The prop-type grammar spells the type
`Option<T>`. `some` resolves as a constructor only in call and pattern
position. Nullary `none` resolves as a constructor in pattern position
and in an expression position whose **expected type is an option** (a
declared `Option<T>` prop constraint, the opposite operand of a
comparison against an option-typed value, or an initializer/assignment
position constrained to an option through ordinary inference); a bare
`none` with no option-typed expectation is a compile error naming the
ambiguity. An existing plain binding named `some`/`none` keeps its
lexical meaning outside those positions; shadowing a constructor in
pattern position is a compile error with a rename fix-it.

> **Open issue (0489 §4.8).** A first-class nominal `Option` with
> dedicated surface syntax remains a recorded alternative to this
> pinned-canonical form. The pinned form is the recommendation and
> matches the implemented direction.

### 4.3 Composite types

A **string enum** is a finite closed set of string literals; it is a
**subtype of `string`**. A **closed object** has named fields and no
additional properties; assignability requires equal field sets and
assignable corresponding types. An **array** has one element type;
heterogeneous elements join only when their arms form an inferable
closed sum. An authored **sum** is either a string enum or a union of
closed-object arms with one shared singleton-string discriminant whose
tags are unique; discriminant selection prefers `kind`, then `type`,
then `tag`, then the lexically first remaining unambiguous field.
Non-discriminated heterogeneous values reject; this edition adds no
syntax for them.

> **Open issue (0489 §4.3).** Whether a later amendment admits
> non-discriminated sums.

> **Open issue (0489 §4.1).** There is no authored generic-parameter
> or bounds syntax; the source language is monomorphic (§4.7). Opaque
> forms such as `Box<string>` reject with a fix-it naming the missing
> generic surface.

### 4.4 Value semantics (A-VAL)

Contract values have no observable interior references. Assignment
copies a value; two names never alias one mutable value. Copy-on-write
and uniquely owned in-place storage are permitted only when observably
equivalent. A member/index state assignment, once admitted, is an
ordinary write in the write mask and does not alter the authored
`mutates` clause. Loop iteration observes the collection snapshot
selected when iteration begins; mutation through another name cannot
affect that snapshot, because aliases are not observable.

### 4.5 Action and callback types

An action/callback value has typed parameters and a **complete closed
effect row** (LLP 0485 §7.2): authored `writes` (sound
over-approximation) and `mutates` (provider/capability mutation
identities), plus the compiler-derived `stateReads` mask, emitted
`commands`, ambient reads, and called-action set — composed
transitively through action calls and callback values. Callback
parameters are contravariant; effect rows compose covariantly.

**Tolerance is compiler-derived in Edition 1** — there is no authored
tolerance annotation: a callback prop's effect row is inferred at its
definition by the same fixed-point unification that types its
parameters; conflicting row constraints reject once at the definition
and identify both constraint sites. A row that crosses a component
boundary carries **symbolic build-time identities** — declaration id
plus static callsite path (the RFC 0401 path without `each` keys) —
never bare state names and never a runtime instance path. (The
runtime callback *value* separately carries its concrete target
instance path; §5.2.) An authored per-prop tolerance surface is a
later named amendment; no composition site rejects against a tolerance
that cannot yet be declared. This narrows LLP 0485 §7.3 closure 4 by
name (filed erratum:
`issues/20260820-llp0485-callback-tolerance-closure-erratum.md`).

### 4.6 Inference

Authors write **no state type annotations**; a state annotation is a
compile error whose fix-it removes it. State types start at their
initializers; prop types come from declared prop types. The compiler
reaches a monotone fixed point across state initializers, whole-state
**action** assignments (tasks do not write state — §7.1 — so no task
assignment contributes a state-type constraint; task-dispatched event
payloads constrain types through the actions those events invoke;
*named merge delta over 0489 R3: its "whole-state action/task
assignments" phrase reads as action-only — 0485 §7.3 closure 2
governs; recorded here, never silently*),
derives, direct identifier references, direct derive aliases, callback
bindings, and component composition boundaries.

Expected prop types constrain literals, closed objects and arrays,
arrows, callback payloads, and direct state/derive identifier chains.
An expected type *checks* an import, token, intrinsic, host value, or
other opaque result; it never *proves* it. Tokens require a compiler
token schema; imported Contract components require project-resolved
prop declarations. External data requires the §9 boundary parser. An
**undescribed built-in event payload stays untyped and rejects**
(a fully curried handler can still infer from its bound values).
String-literal construction and writes infer closed string enums;
closed-object construction sites infer a tagged sum under §4.3's
discriminant rule (even when arms share identical field names). A
direct sum scrutinee (or direct discriminant access) narrows the bound
identifier in a literal `match` arm; **access to a field absent from
some arm requires prior narrowing**. A `match` requires complete
enumerated arms or `else`; diagnostics sort and enumerate missing
arms.

No `any`, open-object widening, or silent fallback exists. Backward
constraints cross direct identifiers and derive aliases, not arbitrary
member paths; nested member-path sum narrowing and untagged
heterogeneous construction reject. A checker MAY reject deep state
writes as a conservative admission boundary; any such rejection MUST
carry the stable feature stamp `closed-types/deep-write-conservative`,
so cross-engine admission differences are named, never silent.

### 4.7 Logical layout and monomorphization

The compiler derives deterministic, target-neutral logical layout
metadata from every fully inferred component: a component instance is
an arena-struct with stable slot offsets (`perSlotHeapCells: false`);
an admitted `each` site is a struct-of-arrays row arena — a
closed-object element contributes one lexically ordered column per
bound field, **any other element type contributes one bound-value
column** (a sum-valued element is that one column, laid out as tag
plus maximum-arm payload), plus an inferred-type key column and a
numeric order column; sum layouts are tag plus maximum-arm payload
with fixed arm layouts and explicit logical padding; strings and
arrays are arena-handle descriptors with copy-on-assignment value
semantics. This metadata fixes type-directed shape; physical bytes,
allocation strategy, and SoA mutation are not claimed by it.

The source language is monomorphic: each definition is checked once;
each complete layout contributes its canonical signature and literal
instance count, and **unused components report zero**; equal scalar
layouts may deduplicate, **distinct aggregate or arena layouts do
not**; the compiler reports a
deterministic distinct-signature summary.

## 5. Equality and identity

### 5.1 Three relations, never interchanged

The value model carries exactly three comparison relations, each with
one job:

1. **Source equality** — `==`/`!=` in Contract source (§5.2). What
   authors observe.
2. **`PublicationEquivalenceV1`** (§5.3) — the dirtiness rule deciding
   whether a computed value republishes. **Deliberately not an
   equivalence relation** (a fresh-equal aggregate still publishes
   absent an authored memo policy); it MUST NOT be used where a total
   relation is required.
3. **`CanonicalValueEquivalenceV1`** (§5.4) — the total,
   alias-insensitive structural equivalence over closed logical
   values. Governs replay checkpoint comparison and callback
   curried-environment comparison.

(This three-way split refines LLP 0485 §7.5's single overloaded
relation; filed erratum:
`issues/20260820-llp0485-publication-equivalence-overload-erratum.md`.)

### 5.2 Source equality

- On primitives, strings, string enums, and sum tags: strict, no
  coercion; `NaN != NaN` is true; `-0 == 0` is true. A string enum
  compares against `string` as its underlying string; two distinct
  enums compare as strings.
- On arrays or objects: **compile error** with a fix-it to compare
  explicit fields, keys, or lengths.
- On `dimension`/`color`/`duration` operands of one kind:
  componentwise over the kind's canonical components; mixed-kind
  comparison is a compile error.
- On action/callback references: identity — declaration id, target
  instance path, and lifetime generation (§8.4) — then pairwise
  `CanonicalValueEquivalenceV1` over the typed, ordered curried
  environment. Never by storage reference; never under
  `PublicationEquivalenceV1`.
- There is **no `null`-narrowing comparison form**: no authored type
  carries a `null` arm, so a `null` literal operand never type-checks
  against a program value.
- Whole-sum/whole-option comparison is admitted in exactly two forms:
  (a) both operands share a sum type all of whose arms are
  payload-free — tag equality; (b) one operand is the payload-free
  option constructor literal `none` — a tag test regardless of the
  `some` payload, so `opt == none` / `opt != none` is the pinned
  emptiness test. Every other whole-sum comparison is a compile error
  whose fix-it is an exhaustive `match`.
- Operands whose closed types do not unify (outside string-enum ⊂
  string) are a compile error.

### 5.3 `PublicationEquivalenceV1`

Used by plain derive publication, redundant state-write suppression,
binding emission (§8.5), and guarded resource dependency tuples:

- primitives, strings, string enums, and sum tags: **`Object.is`
  exactly** (pinned: `NaN` publication-equal — no republish; `-0`
  publication-distinct from `0` — republish; deliberately not the
  same relation as source `==`);
- `dimension`/`color`/`duration`: `Object.is` over their canonical
  packed encodings;
- aggregate array/object whole-slot writes and derive results:
  **always publish** unless an authored `memo`/`memoRows` policy
  stabilizes them (this does not collapse field-granular dirty
  addresses);
- payload-bearing sums: publish on tag change; on equal tag, compare
  the active payload recursively under this same rule;
- action/callback references: identity (declaration id, target
  instance path, lifetime generation), then pairwise recursive
  **`CanonicalValueEquivalenceV1`** over the curried environment (the
  total relation — the aggregate always-publish rule here would make
  an aggregate-carrying callback unequal to itself forever). Never by
  storage handle or implementation-chosen depth.

A fresh-equal aggregate may therefore republish, change a resource
identity tuple, and cause external cancel/refetch; authored memo
stabilization is the recommended pattern where identity stability
matters. This cost is explicit and is not a performance claim.

### 5.4 `CanonicalValueEquivalenceV1`

The closed, type-directed case table — never a byte encoding:

- `null`, `boolean`, `number`, `string`, string enums, sum tags:
  `Object.is`;
- `dimension`/`color`/`duration`: same kind, then componentwise
  `Object.is` over the kind's canonical components — pinned per
  0489 as **magnitude and unit** (dimension), **channels and color
  space** (color), and **magnitude and unit** (duration) ("canonical
  packed encoding" names the deterministic packing of exactly these
  components — a projection owned by the layout/plan authority, never
  a second relation);
- values of different closed type kinds: never equivalent (single
  pinned exception: string-enum vs `string` compare as strings);
- closed objects: equal own-field sets (order-insensitive), recursive
  field-wise;
- arrays: equal length, recursive index-wise;
- authored sums and `Option<T>`: equal tag, then recursive equivalence
  of the active payload;
- action/callback references: the callback identity rule, then this
  relation pairwise over the typed, ordered curried environment
  (recursion through this same case);
- `layout-relation` inhabits no storable or comparable position; the
  table needs no case for it.

Total over the closed universe by induction; alias-insensitive by
construction (A-VAL). The **replay codec is a projection of this
table, not its definition**: conforming replay comparison
canonicalizes (sorted field sets, no `undefined` arm) before or during
comparison, and any change to artifact bytes is a versioned LLP 0341
amendment — never a side effect of this edition.

### 5.5 The one key law

Every keyed identity in the language — `each` keys, `memoRows` row
keys, cursor keys — draws from **one universe**: a key value is a
canonical finite scalar — `null`, `boolean`, well-formed-UTF-16
`string` (the **empty string included**), or finite `number` (no
`NaN`, no infinities, no aggregates). Key identity is
**SameValueZero** (`-0` and `0` are one key). A duplicate key within
one collection snapshot is an error — at compile time where provable,
otherwise a runtime error at collection application, before any row
mutation. (This is LLP 0485 §6.2's universe; implementation
divergences and their burn-down are tracked in the 0489 §2.7 ledger.)
**Reachability of the `null` arm:** no authored Edition-1 key
expression can evaluate to `null` (§4.2's Option-only law), and
boundary-parsed `null` is seam-normalized to `none` before any value
enters a checked program (§9) — the arm is retained for 0485 §6.2
fidelity and remains defined, like §6.2's null truthiness rows, only
for boundary/diagnostic surfaces and the v0→v1 migration window,
never as a live program-level key. The open-issue box below tracks
the runtime enforcement point and the shipping divergences.

> **Open issue (0489 §4.6).** The runtime enforcement point
> (`contract_each_identity_key`) and the burn-down of the shipping
> paths' divergences (empty-string rejection; `memoRows`'
> null-rejecting, NaN-admitting domain) before the flag lifts.

## 6. Expressions

### 6.1 Operators

Arithmetic is IEEE-754 binary64 exactly (`0.1 + 0.2` is
`0.30000000000000004`); `%` is JS-style remainder (sign of dividend),
including negative and non-integer operands; `/ 0` yields signed
Infinity; `0 / 0` yields NaN. `+` admits exactly `number + number`
(numeric) or `string + string` (concatenation, string enums admitted
as strings). `-`, `*`, `/`, `%` admit exactly `number` with `number`.
Relational `<`, `<=`, `>`, `>=` admit exactly `number` with `number`
(orderings over other types are stdlib calls, never operators). Every
other mixed arithmetic or relational expression is a compile error.
`**` remains unpinned (no corpus use).

### 6.2 Truthiness

`false`, `null`, `0`, `-0`, `NaN`, and `""` are falsy; every other
admitted closed value is truthy. There is no `undefined` case.
**Reachability scoping (Option-only consequence, not a new rule):**
no expression in a checked v1 program is `null`-typed (§4.2), so the
`null` truthiness row — like §6.5's `null` interpolation row — is
unreachable from checked programs; both rows remain defined only for
boundary/diagnostic surfaces and the v0→v1 migration window (0489
retained item 7's "null interpolation remains legal" adjudication
predates r6's Option-only decision — the merge records this scoping
rather than silently deleting either source sentence). An
**option-typed operand in any truthiness position** (a `when`/`else`
test, a ternary condition, an `||`/`&&` operand, a `!` operand) is a
compile error whose fix-it is an exhaustive match or an explicit
comparison — `none` is neither falsy nor truthy. Under the closed
checker a `when`/`else` test is typed `boolean`.

### 6.3 `??` and `?.`

**`??` is option-coalescing** — restating 0489 r6 R3's decided rule
exactly: the left operand MUST be option-typed (`a : Option<T>`); a
non-option left operand is a compile error (the coalesce is provably
dead). `a ?? b` **yields `a`'s payload** when `a` is `some(payload)`
and yields `b` when `a` is `none`; the expression's type is `T` joined
with the right operand's type under ordinary closed inference. The
coalesce is a total, non-nesting eliminator: **when the right operand
is itself option-typed, the result type is that option** (no
`Option<Option<…>>` is constructible here or anywhere).

> **Open issue (merge-found; owner ratifies at merge).** 0489 R3's two
> sentences are in tension when the right operand is option-typed
> (`b : Option<V>`): the `some` branch yields a bare payload of type
> `T` while the stated result type is the option `Option<V>` — a bare
> payload does not inhabit that type unless the `some` branch *lifts*
> (`some(payload)`, giving `Option<T ⊔ V>`). The lift is the unique
> type-coherent completion and is the **recommended resolution**,
> recorded here for the owner rather than decided by this edition; a
> conforming implementation MUST NOT ship `optA ?? optB` until the
> owner rules (the non-option-right case is unaffected and total).

`||`/`&&` remain truthiness-based and short-circuit.

**`?.` is option projection.** `a?.b` requires `a` option-typed with
an object payload carrying field `b`. When `b : V` for non-option `V`,
the result is `Option<V>` (`none` when `a` is `none`, `some(payload.b)`
otherwise). When the field is itself option-typed (`b : Option<V>`),
the access **flattens**: the result is `Option<V>` — `none` when `a`
is `none`, and `payload.b` unchanged otherwise; no nested option
arises. Chained optional access therefore flattens end-to-end —
`a?.b?.c` yields one `Option<W>`. *(Source note: the field-flattening
case is the unique reading coherent with 0489 R3's no-nest rule and
its "projects the payload's field into a single `Option<V>`" sentence
— a derived completion of the source text, marked as such, not a
verbatim restatement.)* Non-optional member access on an
option-typed value is a compile error whose fix-it is `?.` with
exhaustive match or prior narrowing. Missing closed-object members do
not exist, so plain member access is total. Dynamic indexing without a
static bounds proof and `find`-style misses are option-shaped and
matched exhaustively.

### 6.4 `match`

An option or sum is consumed by exhaustive `match`. In **view
position** the view `match` construct carries option arms
`some(binding)` and `none`, the binding scoped to its arm. In
**expression position** (derive formulas, action-body right-hand
sides) a `match` expression carries arms each yielding a value; the
expression's type is the join of arm types. In **statement position**
(action bodies) a `match` statement's arms are assignment blocks under
the action's `writes` mask. `some(x)` binds the payload. Sum matching
is structurally an `exclusive` Region (§8.1) in view position and a
total expression elsewhere; a `match` requires complete arms or
`else`.

### 6.5 Strings, numbers, and interpolation

Strings are UTF-16: lengths, indices, and spans count UTF-16 code
units. Number-to-string (template interpolation, `String(n)`, every
stdlib rendering) is the ECMAScript Number::toString algorithm
(ECMA-262 §6.1.6.1.20, radix 10) — shortest round-trip decimal with
ECMAScript's fixed/exponential switchover. In template parts, `null`
renders as the empty string (`String(null)` still yields `"null"`);
there is no `undefined` interpolation arm; closed array/object
operands are admitted with the template typed `string`, retaining JS
`String()` rendering pending a later narrowing (ambiguity A6, open
for closed programs).

### 6.6 The stdlib

The v1 stdlib is enumerated; the pinned lists live in
`packages/exact-contract/src/compiler/stdlib-v1.ts`. **Under this
edition an out-of-roster call is a compile rejection.** The shipped
compiler carries **two** diagnostics for it, both at warning severity
today — `stdlib_member_outside_v1` (member calls) and
`stdlib_namespace_call_outside_v1` (namespace builtin calls,
`Math.*`/`JSON.*`) — and the promotion of both to rejection lands at
the flag flip; these are compile-time diagnostics, never runtime
errors (§11). **Receivers rooted at imports, props, or behavior
instances are exempt** — their surface belongs to the import boundary
or the runtime, not the value stdlib (0330 §3's rule, unchanged; the
shipped analyzer implements it). Additions are semantics changes: they need a
written local-semantics note and conformance coverage before a second
engine implements them. **`Math.random` precedence (0330 §3/§5's own
split):** `Math.random` is not a stdlib namespace member — it is one
of the four reserved determinism-seam builtins (§7.4), so a source
`Math.random()` call resolves to the seam builtin (the deterministic
stream) and is admitted, never stdlib-rejected; the shipped compiler's
`stdlib_namespace_call_outside_v1` warning on it is a **named shipped
divergence** to burn down before the flip, not the v1 rule. `console`
is dev-surface.

### 6.7 Derived-result stabilization (`memo` policies)

The suffixes `memo`, `memo(comparator)`, and `memoRows("key")` are
explicit value-stabilization policies applied after the derive formula
evaluates and before its computed publishes; they never change
dependency tracking and stabilize logical values rather than expose
aliases:

- **`memo` (shallow):** scalar, dense scalar-leaf array, or
  scalar-leaf object; `Object.is` leaves; arrays by length and index;
  objects by own-key set (order ignored) and keyed values. Equal
  selects the previously published logical value.
- **`memo(comparator)`:** the comparator expression is **evaluated
  exactly once per component/behavior instance, outside reactive
  dependency tracking**; the **first derive result publishes without
  invoking it**; later comparisons invoke the captured function as
  `Reflect.apply(comparator, undefined, [previous, next])`; it MUST
  return a boolean primitive (`true` selects the previous logical
  value, `false` the next); throws and non-booleans are runtime errors
  at the authored derive locus. The compiler **rejects
  action/task/mutation/action-prop/effectful-import dependencies** in
  that pure expression position. Alias identity MUST NOT be observable
  through the comparison.
- **`memoRows("key")`:** bottom-up reconciliation of object-row
  arrays: an array containing object rows contains only object rows,
  and every row has a unique own `key` data property drawing from
  §5.5's one key law (SameValueZero; non-finite numbers reject).
  Matching unchanged rows and nested values select their previous
  logical values; reorder publishes the new order. Repeated input
  aliasing is neither preserved nor observable.

The structural domains are **host-admission conditions for foreign JS
values** entering the stabilization policies — not part of the closed
value model; a value typed by the closed language satisfies them by
construction. Every traversed object/array MUST have the standard
`Object.prototype`/`Array.prototype` or a null prototype, be
extensible, and expose only own string-keyed, enumerable, writable,
configurable data properties (an array's standard non-enumerable
`length` is the sole exception). Sparse arrays, extra array
properties, symbols, accessors, nonstandard descriptors,
frozen/sealed/non-extensible values, custom prototypes, cycles, mixed
object/scalar row arrays, and missing, non-scalar, or duplicate row
keys are runtime errors — all checked **before authored getters could
run**.
Feature stamp `derive-memo`; an engine that does not implement a memo
policy MUST reject it by name, never ignore it.

> **Open issue (0489 §4.7).** `memo(comparator)`'s comparator is a
> JS-host callable; its long-term shape under full A-VAL is a W2
> question.

### 6.8 Layout-relation intent

`fill`, `ratio`, `center`, `pinToSafeArea`, `matchWidth`,
`matchHeight`, and `alignTo` produce the sink-only `layout-relation`
type, checked against compiler signatures. A relation may flow through
a derive but is admitted only as the direct value of a recognized
built-in layout/style sink; storing it in state, nesting it in
aggregates, or sending it to another sink is a compile error.
Measured-sibling feedback and resolution remain excluded (owned by the
LLP 0486/0487/0488 program).

## 7. Reactivity and execution

### 7.1 State, derive, action, task

`state` declares reactive storage typed by its initializer. `derive`
declares a pure computed value; a derive publishes a settled value —
a thenable result is a runtime error at the authored locus
(`thenable-derive`). `action` declares a transition body; every state
an action writes appears in its `writes` list (§4.5's effect rows).
`task` declares asynchronous work: task bodies MAY await; **tasks do
not write state** — a task performs I/O and dispatches typed events
whose actions perform the transitions (§10.3).

### 7.2 Scheduler invariants

Every conforming engine preserves (flush order and patch granularity
within them are implementation-defined):

- **Batch atomicity.** All state writes in one action body commit as
  one batch; no observer sees a partial batch.
- **Glitch-freedom.** A derive never observes a mix of pre- and
  post-batch inputs; within one commit a derive recomputes at most
  once observably.
- **Quiesce-state equivalence.** After quiesce — **no pending
  recomputes, microtasks, or due timers** (0330 §4's definition,
  unchanged) — the semantics tree,
  accumulated protocol state, and witness are fully determined by
  (IR/plan, props, event script — **each scripted event carrying its
  `envSnapshot` ambient captures**: clock, locale, color scheme,
  viewport class, per 0485 §7.1's declared ambient inputs and the
  §3.3.8 closed channel list — and the clock/rng script). This is THE
  conformance comparison; two runs differing only in unscripted
  ambient state are not comparable runs.
- **Timer semantics.** `every(ms, action)` / `after(ms, action)` fire
  as normal batched actions; under the scripted clock, due timers fire
  in (dueTime, registration order); `every` rearms at `dueAt + ms`;
  disposal on unmount/reset is mandatory.
- **Event dispatch.** A host event fires the bound action
  **synchronously at delivery** with its payload — LLP 0330 §4 stands
  unamended (the one-inbox admission model is a 0485-proposed
  amendment whose **reject branch was decided for v1**: LLP 0505 r3
  row 8 / LLP 0482 r6; the class order applies only to timers and
  completions). Curried bindings re-evaluate argument expressions at
  dispatch time against current state.

### 7.3 Host-observable equality

The renderer MAY suppress a host mutation only after comparing the
complete state observable at that host boundary (raw text by
`Object.is`; canonical host descriptions recursively; uncertainty can
only forgo a skip). Event handlers are not description identity; the
latest handler remains live when a mutation is skipped. LLP 0330 §9's
rules are incorporated by reference, unchanged.

### 7.4 The determinism seam

Time and randomness enter only through explicit builtins: `now()`,
`every()`, `after()`, and Contract `Math.random`. Every engine
implements the determinism-host seam; **when a host is installed all
four builtins route through it; when none is installed, ambient engine
behavior applies** (0330 §5's installed/ambient split, incorporated by
reference — ordinary non-fixture execution is the ambient branch).
Fixtures declare `clock.startMs`
and `clock.rngSeed`; the pinned rng is mulberry32. The deadline-driven
time system — `now(granularity)`, `invalidateAt`,
`timezoneGeneration()`, the shared logical deadline queue with one
physical wake, lifecycle/resume/timezone revalidation, and the
`temporal` terminal-text form — is **LLP 0330 §10, incorporated by
reference in full, with one named Edition-1 delta**: under v1 the
`temporal` callback's `nextChangeAt` is option-typed —
`some(timestamp)` / `none` (no next change) — replacing 0330 §10's
`null` spelling, which no authored v1 value may carry (§4.2); the v0
spelling is converted by the one migration (§14). All four temporal
builtins are reserved direct-call forms (not first-class values, not
shadowable). The RNG remains **one global mulberry32 stream over the
seed per 0330 §5/§11.2** — the per-event RNG substream is a
0485-proposed amendment that is **not** Edition-1 law (its bit-level
derivation is unwritten and its adoption was declined with the inbox
model, LLP 0505 r3 row 8).

### 7.5 Portable effects and verified replay

Effect attempts, the `EffectSupervisor`'s single-committer terminal
arbitration, scope-close fencing, append-only evidence, the
evidence-join classification (Amendment A1), the replayability
certificate, capture leases, and replay verification are **LLP 0330
§11 (as amended), incorporated by reference in full**, with two
Edition-1 deltas already adjudicated: aggregate replay values compare
under §5.4's canonical relation (primitives keep `Object.is`
fidelity), and the artifact grammar remains LLP 0341's.

## 8. Regions

### 8.1 The Region kind inventory (exhaustive)

| Kind | Constructs | Discriminant | Arms or target |
| --- | --- | --- | --- |
| `exclusive` | `when`/`else`, `match`, `match-size` | predicate, scrutinee, or container-size-class signal | the known branch templates |
| `keyed` | `each` | collection expression + required key expression | one row template + an instance table |
| `cell` | `resource`, `async`, `boundary` | the construct's cell state | `loading | error | stale | empty | ready` (resource/async); `fallback | body` (boundary) |
| `composed` | snippet calls, `pager`, component calls | static target (bounded dynamic target lowers to `exclusive` over its selector) | callee template references |
| `graphics` | `graphics` surface | surface Region | one referenced closed scene sub-program (id + SHA-256 digest) |
| `root` | component root | — | one body template |

Sum matching is `exclusive`; there is no seventh kind. Authored
`nothing` is an admitted explicit zero-node template; `Hole` and
`ChildrenSlot` are compiler-internal and MUST NOT survive the Region
boundary. Recording an arm in a cell universe does not make an
unauthored or unreachable arm reachable; reachability stays with the
construct's owning semantics. **Region ids MUST be deterministic for
identical admitted IR**; identity and lifetime generations bind to the
§8.4 address spine, never to Region ids. `publish` is an auxiliary
declared-effect record, not a Region kind; its full shipped shape
(rate, heartbeat, budgets, lifetime-generation-fenced delivery,
cancellation on owner disposal) is **preserved as classified record
shape under §2.3's carried exclusion** — channels/`publish` execution
semantics remain matrix-gated (LLP 0328 D10) and owned by their LLPs;
an engine whose matrix admits publish runs the heartbeat as an
ordinary §7.2 timer under §7.2's `(dueTime, registration order)` rule
(the inbox-model positional identities recorded in 0501 §3 remain
dormant alternative text, not Edition-1 law — §10.3); an engine whose
matrix does not admits nothing and rejects by name.

### 8.2 Component calls and composition

An ordinary component call is a `component-call` composed record with
a **static target**; a dynamic target is admitted **only when
compilation closes it to a finite candidate set** — an unbounded
by-value target is a **compile rejection whose fix-it enumerates the
known candidate closure** (0501 R1 / 0485 §8.1, both sources) — and an
admitted bounded dynamic target is normatively
lowered at compile time to an `exclusive` Region whose discriminant is
the selector and whose arms are static `component-call` records (a
selector outside the bounded set is the exclusive no-match — a runtime
error, never a silent fallback). The record carries three input
surfaces — `props`, `events`, `childrenTemplate`:

- prop bindings flow into the callee's props;
- event bindings are ordinary action/callback references under §5's
  identity rules;
- caller children are a **caller-owned template realized at the
  callee's children slot with the caller's scope** (closure
  semantics). **Realization lifecycle is dual-bounded:** each
  realization is a logical instance at the realized-children address
  (§8.4), disposed when *either* the callee instance disposes (a
  target switch **remounts** children under the new callee) or the
  call's Region deactivates. On a surviving callee, a
  children-template *binding* change is stage-2 input propagation; a
  *shape* change creates/disposes exactly the affected
  realized-children addresses.

The callee instance is created when the call's Region first activates,
carries its own lifetime generation, receives input updates without
recreation, and is disposed (tasks cancelled, generation retired) when
the Region deactivates or its parent disposes. A target switch is
dispose-then-create — never state migration between candidates.
**Input propagation is a sweep operation, not emission**: a surviving
callee's changed inputs propagate before the callee's own derives
re-evaluate and before child-Region reconciliation, so a committed
batch is never observable to the callee with pre-batch props. Snippet
calls compose the same way (static target; changing arguments are
input propagation), with 0501's structural rule preserved: **a snippet
body is one standalone callee template, walked once; a top-level
Region in that body has the callee template as its parent — never a
sibling of the call site's Region or the component root.** A pager is a kind-specific composed record whose
side-named cells are **stable slots whose bound data changes**
(LLP 0336's predicate): a center-identity change re-binds data through
ordinary input propagation; only guard-occupancy or component-target
changes dispose/create cell content.

### 8.3 Keyed instance management

Each keyed Region owns a key-to-row lookup, an order index, and one
closed row template. On collection change it computes the key-set
delta, creates and disposes rows, applies the order-index permutation,
and exposes which surviving logical row fields changed. New keys
instantiate the template; surviving keys reuse their logical row;
removed keys dispose the row and cancel its owned tasks. Iteration
follows key order through the order index. Conforming storage is
observably equivalent to an insertion-ordered SoA row arena compacted
on disposal.

### 8.4 Lifetime generations and the address spine

Every Region-owned logical instance — component instance, behavior
instance, keyed row, cell, composed callee — carries a **lifetime
generation**: a per-logical-address monotonic counter within its root,
a different layer from LLP 0417's `ExecutionGeneration` / LLP 0500's
`HotRevision` and from kernel root incarnations. Rules: **minting** on
creation (strictly greater than every prior generation at that
address); **increment on recreate** (`(address, generation)` never
aliases across destroy/recreate); **reuse** by surviving instances;
**root reset** retires every generation under the root; **HMR** — a
compatible LLP 0500 D1 plan patch preserves the generations of
preserved identities; a full reload retires all. Counters are at least
64-bit and never wrap; exhaustion is a defect, never reuse.

**Stability precondition (normative).** Preservation is only as strong
as the address spine's edit-stability, and structural RFC 0401 paths
are not yet production-stable. A patch preserves a generation only
when it **proves** the logical address survived; where preservation
cannot be proven, the conforming fallback is the full-reload path for
the affected subtree — never a guessed match. Until the 0401 stability
amendment lands, this edition defines the algebra LLP 0500's
state-preservation objective binds to but does not claim that
objective.

**`RegionActivationAddressV1`** — the versioned, total typed sum of
logical addresses (the identity spine; **never Region ids**), each
embedding its ancestors: **root**; **component-instance** (the
RFC 0401 logical instance path); **behavior-instance** (owner +
behavior declaration id); **keyed-row** (the owning `each` site's 0401
path + the row's canonical key per §5.5); **cell** (declaration-level
identity under its owning instance; the RFC 0498 runtime-key
refinement is an open amendment dependency); **composed-callsite**
(the call site's 0401 path); **callee-instance** (composed-callsite +
active candidate's component id); **realized-children** (the children
template's caller-relative path qualified by the active
callee-instance address). The **cell** variant's governing identity is
the **amended LLP 0500 D1 rule**: the declaration identity
(LLP 0485 §3.3.16) is the match key; **for shared-cache-path cells**
the evaluated dependency tuple
feeds the **runtime cell key's** canonical arguments and is not part
of the address; canonical cell state is reused only on exact
runtime-key equality (RFC 0498 r6 §4.1's reuse gate, specified in
LLP 0509 — the landing vehicle for this variant's refinement;
LLP 0509's **subscriber-scoped** cells carry neither
`ProviderSourceKey` nor `RuntimeCellKey`, and their reuse is
address-local under this same declaration match key). Two
declarations therefore never alias one lifetime through a shared
tuple, and a tuple change alone never retires a generation. (0501
§8.4's "declaration-plus-dependency tuple" sentence predates the D1
amendment; the amended D1 governs.)

Separately, **key-set generation** is the per-collection counter
bumped when the collection's **ordered key sequence** changes
(membership or order — a pure reorder bumps it, because ordinal
cursors are order-sensitive). Consumers of the algebra:
action/callback comparison (§5), publish lifetime-generation-fenced
delivery, and settlement target-liveness (a settlement whose target
`(address, generation)` is retired is suppressed as stale).

### 8.5 Committed Region update

On each committed batch, a conforming runner preserves §7.2's
invariants and produces the quiescent result as if it performs Region
reconciliation, binding emission, and flush in that order after state
and derives have post-batch values. Under the plan execution model the
refinement is five stages (LLP 0485 §8.3, incorporated; restated):

1. **Dirty marking** — the journal's address set marks
   slot/field/row-column dirty bits with hierarchical invalidation
   (field writes never invalidate sibling-field observers).
2. **Graph sweep** — one topologically ordered pass over the dirty
   closure; derives re-evaluate; guards evaluate before their edges
   propagate. **Two-phase keyed reconciliation over one post-delta
   snapshot:** a dirty keyed collection's delta/provision step runs
   **at that collection's own topological position in the pass —
   immediately after the derive that produced its new value, never as
   a global pre-pass**. There the sweep computes the key-set delta,
   bumps the key-set generation **iff the ordered key sequence
   changed** (a pure reorder bumps; a value-only change does not), and
   installs the complete ordered post-delta snapshot as the only
   sweep-visible collection view; new/reappearing rows are provisioned
   (fresh generations) before any cursor rebinds; removed keys leave
   the key set now — a `bound(K)` cursor transitions to `missing(K)`,
   **its row overlay is removed**, and its consumers invalidate before
   any cursor rebinds; removed-row storage survives only as an
   inaccessible tombstone until stage 3. **No old-union-new collection
   view exists at any point.** Cursors rebind at their own topological
   positions against the post-delta view; composed-input propagation
   (§8.2) runs in this pass.
3. **Region toggles** — dirty `exclusive`/`cell` discriminants
   re-evaluate; a changed arm emits subtree destruction and creation
   from templates; keyed collections publish provisioned creations,
   encode the already-installed order, and dispose tombstoned rows
   (cancelling tasks, retiring generations); composed Regions apply
   §8.2's lifecycle.
4. **Binding emission** — dirty bindings evaluate; values changed
   under `PublicationEquivalenceV1` (§5.3) encode host/protocol
   operations.
5. **Flush** — one batch to the presenter under §7.2's invariants;
   granularity inside them is implementation-defined.

Cell settlements are typed events consumed after a target-liveness
check and before Region reconciliation (stage 3). The settlement
**interface points are normative** (0501's requirement): a settlement
row MUST identify the **target cell Region and arm transition**, the
**target instance and its lifetime generation**, the **source/attempt
identity**, the **cancellation identity**, and the **journal addresses
dirtied by the settlement**; a settlement whose target
`(address, generation)` is retired is suppressed as stale. The full
event-row encoding, dispatcher, and source-queue machinery remain
owed by the change-3 amendment (§10.3); until it lands no engine may
claim conformance for a vocabulary this edition does not define.

### 8.6 Secondary windows

A component's `window` views stay **outside the Region forest** in
their governed side table (LLP 0337/0345); classification never walks
them into the primary root. Rejection is execution-tier-scoped: a
Region runner and Contract Native admission reject a window-carrying
component by name until a separately governed secondary-root model
exists; the TS host's existing window execution remains permitted.

## 9. Boundary parsing (`shape`)

External data enters typed state through a declared shape at the cell
boundary: `resource name = expression as shape TYPE` (one logical
line; `shape` is not a global keyword; unshaped resources keep their
previous path). This section is a semantics-preserving carry of
0489 r6's boundary-parse rules (whose full text also remains frozen
incorporated text under §14's supersession carve):

- **Root and field grammar.** `TYPE` has a closed object, uniquely
  discriminated closed-object sum, or an array of one of those at its
  root. Fields may contain `string`, `number`, `boolean`, `null`,
  `never`, string-literal enums, nested closed objects, uniquely
  discriminated closed-object sums, and homogeneous arrays
  (comma-separated; trailing comma allowed). Shapes are **exact**:
  extra fields reject.
- **Materialization.** Compilation derives data-only validator IR
  (never evaluating authored type text, never `eval`). A conforming
  terminal settlement is a recursively materialized **detached
  snapshot**, never source identity; later source mutation cannot
  affect it. Source objects require `Object.prototype` or null;
  output is a fresh ordinary object. For every declared field,
  materialization accepts **any own data property regardless of
  enumerability, writability, or configurability**; a missing own
  property or an **accessor rejects without invoking the getter**.
  After declared fields are read, **every additional own key rejects —
  string or symbol, enumerable or not**. Arrays are **dense**, accept
  only their own `length` and in-range index data properties, and are
  rebuilt fresh. Class instances, custom prototypes, functions,
  accessors, and exotic values do not cross.
- **Numbers.** Shape `number` accepts every binary64 number including
  non-finite values; no finite-number subtype exists (§13).
- **Diagnostics (pinned values).** The first mismatch settles the
  resource's error arm with the plain-data diagnostic carrying exactly
  `kind`/`code`/`path`/`locus`/`expected`/`actual`/`message`;
  validation failures use `kind: "contract-shape-validation"`,
  `code: "resource-shape-mismatch"`; an arbitrary initializer
  rejection projects to the same surface with
  `kind: "contract-resource-rejection"`,
  `code: "resource-initializer-rejected"`. Raw throwable identity,
  subclass, stack, and accessors never enter the slot value. No
  partial value publishes.
- **Trust split.** Inspecting a live JS proxy may run hostile
  reflection traps; a trap may reject or fabricate conforming answers,
  but output is only ever the detached plain snapshot — never the
  proxy, getter, capability, or source identity. Plan/native **byte
  parsing owns its bytes and runs no foreign reflection code**;
  live-value validation is not byte parsing.
- **Gating.** Shape diagnostics are construct-gated, not
  closed-checker gated; a module without the postfix has no validator
  and no validation branch.

Under the closed checker, shaped `ready` and `stale` bindings have the
declared type and `error` the diagnostic type; a checker's typing of
an arm is never a reachability claim.

The **seam-normalization rule** (§4.2) applies at this and every host
seam: parsed `null`/absent-optional lowers to `none`, defined values
to `some(v)`; nullable-marked boundary fields land as `Option<T>`.
**What identifies an optional field (totality):** the seam annotation
is exactly the IR's `nullable` marker (§4.2) — Edition 1's *authored*
shape grammar has **no optional-field spelling**, so every authored
declared field is **required** (a missing own property rejects, per
the materialization rule above; there is no contradiction with §4.2's
absent-optional clause, which applies only to `nullable`-marked fields
— today reachable through generated/IR-produced shapes, not authored
text). The authored spelling for optional fields, and the shipping
authored-shape parser and `ShapeFieldIR`'s missing
optional/nullable field bit, are one tracked 0489 §2.7
burn-down row — not a licence to admit `null` past the seam, and until
it lands the authored fix-it for an optional source field is an
explicit discriminated arm.

> **Open issue (0489 §4.2).** Whether a finite-number subtype is ever
> admitted at the boundary. **Open issue (0489 §4.4).** The
> currently-unreachable resource `stale` arm's disposition.

## 10. The plan execution model

This section binds the language to its execution encoding. The
**normative tables live in frozen LLP 0485 and are incorporated by
reference** — restating them here would fork them; this edition owns
the language-level statements and names its deltas.

### 10.1 Closed static types in the plan

LLP 0485 §5 is incorporated by reference: the type system of §4 *is*
the plan's type system (every tier, not one tier's admission gate);
layout per §4.7; the boundary parse per §9; monomorphization per
§4.7. **Where a language-level statement in §§4–6 of this edition and
an incorporated 0485 §5.2/§5.3 restatement overlap, this edition's
statement governs.** **Edition-1 deltas over the frozen text
(exhaustive):**

1. absence is Option-only (0485 §5.1's "option sums and `null`" reads
   as `Option` alone; the `??`/null-coalescing restatements in 0485
   §4.3/§5.1 are superseded by §6.3 — recorded for this ratification
   per 0489 r6);
2. callback tolerance is compiler-derived (§4.5's named narrowing of
   0485 §7.3 closure 4, filed erratum);
3. **the source language is monomorphic** (§4.7): 0485 §5.5's
   check-once/instantiate-layouts-per-signature text reads as its
   monomorphic degenerate — an explicit W2 narrowing recorded here per
   0489 §4.1's requirement, never a shortcut by silence; authored
   generics remain the §13 open issue;
4. 0485 §7.5's single overloaded equivalence relation reads as §5's
   three relations (filed erratum, §5.1).

### 10.2 Static dependencies

LLP 0485 §6 is incorporated by reference: every executable context's
read set resolves at build time into Deps rows (an unresolvable derive
is a compile error); dependency edges are field-granular for
closed-object slots and column-granular inside `each` row templates;
the topological order is computed once per plan. **The cursor**
(`cursor sel = items at i key s => s.id`) is the paved
element-granular form: option-shaped value; declared key identity from
§5.5's universe; three runtime states (`unbound(ordinal)`, `bound(K)`,
`missing(K)`) surfacing as `some`/`none`; in every state the cursor
subscribes to the collection's key-set generation; `bound(K)` follows
`K` across reorders (ordinal decoupled until the next `at` change).
Bare dynamic indexing widens visibly (whole-value version edges, a
`dep_widened` diagnostic, budget enforcement); out-of-bounds is
option-shaped (§6.3). Conditional dependencies: pure derives
over-approximate to the union (`dep_union`); resource initializers
MUST use guarded edges (guards are dependency nodes ordered before the
edges they guard; the cancel/restart/refetch decision is taken from
one post-transition tuple comparison, preserving RFC 0082's identity
semantics). The bare **ordinal `cursor`** form — a `cursor`
construct declared with no `key=`, a standalone component-scope
declaration exactly like the keyed form (0485 §6.2; when the same
collection also feeds an `each`, the `each` carries its own required
key expression independently, §8.1) —
remains 0485 §6.2's form: it always denotes post-transition element
`i`, reorders move what it names, and out-of-bounds is `none`; the
declared keyed `cursor` form above is the paved element-granular
alternative. **Keyedness is a property of each `each` site (its
required key expression), never of the array value** — two `each`
sites over one collection may legitimately declare different keys —
and where a declared `cursor` and a keyed `each` over the same
collection project **different identities**, the compiler emits the
pinned `cursor_key_mismatch` **warning** with a fix-it (0485 §6.2's
rule and the shipped diagnostic's exact subject and severity; the r2
two-`each`-sites error restatement was wrong on both and is
withdrawn).
Runtime read-tracking is deleted, not bypassed; every
evaluation is non-subscribing and edges come only from the build-time
Deps table.

### 10.3 Actions as reducers

LLP 0485 §7 is incorporated by reference **on its reject branches
where 0485 carries a proposed 0330 amendment** (below). The transition
is `(state, event, envSnapshot) → (state′, Command[])`; the command
vocabulary is closed (`task-start`, `capability-call`, `navigate`,
`emit-user-event`, `cancel-task`; programmatic publish is a
capability-call); commands execute strictly after commit, in return
order, and MUST NOT synchronously re-enter the dispatcher; no `await`
in action bodies — asynchrony re-enters as typed events (settlements
included). Effect signatures keep authored (`writes`, `mutates`) and
derived (`stateReads`, `commands`, `ambient`, `calls`) fields
distinct, composed transitively.

**Event ordering (the decided reject branch — LLP 0505 r3 row 8,
LLP 0482 r6):** host events dispatch **synchronously at delivery**
(0330 §4 stands unamended); the class order applies **only to timers
and completions**; there is no admission point, no composite source
identity, no drain cutoff, and no quiesce extension in Edition 1 —
the one-inbox admission model and the per-event RNG substream remain
0485-*proposed* amendments (0485 itself notes the substream's
bit-level derivation is unwritten and the reject branch is the only
implementable rule), revivable only by a demand-justified future
amendment. Tier 2 of LLP 0482 ships replay-only accordingly.

**Target liveness — what is decided vs open.** The decided Edition-1
law is exactly what the sources decide: **settlement target-liveness**
(0501 R1: a settlement whose target `(address, lifetime generation)`
is retired is suppressed as stale — §8.5's normative interface) and
**publish lifetime-generation-fenced delivery** (0501 R1). Frozen 0485
§7.4's *universal* `InvocationTargetStamp` (plan generation,
discriminated `TargetSiteId`, instance path, generation — minted at
production, fail-closed, checked at three points) was packaged **as
part of the proposed 0330 §4 amendment text whose reject branch was
decided** (LLP 0505 r3 row 8), and 0501 decided only the settlement
half.

> **Open issue (merge-found; owner decision).** Whether target
> liveness generalizes to *every* invocation class as a standalone
> rule — and with which stamp (0485's full `InvocationTargetStamp` vs
> a reduced `(address, lifetime generation)`) — is not decided by any
> source outside the declined packaging. Recommended default: adopt
> 0485 §7.4's stamp semantics as a standalone Edition-1 rule (its
> design is orthogonal to the inbox). Conservative interim: engines
> MUST enforce the settlement and publish fences above and the task
> cancellation fence below; they MAY suppress other stale-targeted
> deliveries (recording `stale-target`); they MUST NOT run a
> settlement or fenced publish delivery against retained state.
>
> **Second confirmed customer, recorded 2026-08-26 (RFC 0540 §8.5 row
> 66; content LLP 0543 §6.5).** The `VisibilityTransition` event
> payload family stamps every entry with the **reduced** fence —
> `(address, lifetimeGeneration)` — and suppresses a retired entry as
> `stale-target`, entry-only, never refusing the chunk or the bundle.
> That is exactly the conservative interim above, applied to a class
> the interim already permits, so **this open issue is not resolved by
> the lists work and is not blocked by it**: RFC 0540 asks only that
> its family be placed against this generalization, and it lands on the
> reduced arm. Two consequences worth stating so a later reader does
> not over-read the fold. (1) If the owner later adopts 0485 §7.4's
> full `InvocationTargetStamp` as the standalone Edition-1 rule, the
> visibility family widens with everything else; it has taken no
> position that would need unwinding. (2) The reduced fence here is a
> *delivery* fence on host→runtime event entries, not an admission
> rule: root admission for virtual roots is a separate,
> command-frame-time check on the topology certificate (0543 E-V2 /
> E-V4) and is untouched by whichever arm this issue takes. Recording
> only; no normative change, and the choice of stamp stays this
> spec's owner's.

Rollback is A-VAL (§4.4) plus the
ordered-entry write journal (`(address, before, after)` per executed
write; patches at the finest written address; reverse-order unwind on
trap; the `TransitionArtifact` is the one commit representation). Task
cancellation is a generation fence over authored delivery — checked
at every continuation resume, before every new attempt/dispatch,
before reducer evaluation, and before commit/command release — and
never erases supervisor evidence (§7.5). The paved multi-step form
(`task … from action` with `-> ok(a) | fail(b)`) is the normative
replacement for awaiting actions.

### 10.4 Regions in the plan

LLP 0485 §8 is incorporated by reference for the plan encoding
(region rows, the schema-total field-coverage gate, keyed SoA
arenas, `TransitionArtifact` composition, first-frame bake-and-patch);
**§8 of this edition is the semantic authority** where the two
overlap, carrying 0501's adjudicated refinements (the
`component-call` record and lowering, the dual-bounded children
lifecycle, the two-phase post-delta sweep, tombstones, the lifetime
generation algebra, tier-scoped window rejection).

**Plan-producer obligations (normative, carried from 0501 R3):**
Region construction is **schema-total over admitted IR** — every named
field recursively reachable from the file IR, including every
discriminated-union arm, has exactly one of four dispositions
(`consumed-into-region`, `expression-walked`,
`compiler-internal-must-not-survive`,
`explicitly-deferred-with-reason`); an undispositioned field or arm
**fails the build**, a deferred row names its owning table, and
"ignored" is not a disposition. The gate has two independent halves —
exact-key record maps with exhaustive union indexes (catching newly
declared fields/arms) and a **recursive audit of populated IR**
(catching stale, widened, or cast-produced data, continuing into
records nested under imports, witnesses, diagnostics, shapes, Motion,
and view/graphics arms) — plus a **Region-builder read-set audit**
showing every observed named IR field read is consumed or
expression-walked. Adding a **pager field** without a Region
disposition is a compile failure (the record preserves the complete
admitted pager field set); a **graphics arm** carries only the id and
SHA-256 digest of its referenced scene sub-program, which carries the
matching id, digest, and closed grammar marker; the **publish record**
preserves its complete admitted shape (§8.1). Corpus acceptance
requires zero undispositioned populated fields and zero builder-read
violations.

The two known frozen-0485 inventory errata are filed and tracked
(`…component-call-composed-inventory-erratum.md`,
`…authored-nothing-region-classification-erratum.md`).

### 10.5 Obligations and cost claims

LLP 0485 §9 is incorporated by reference: contract-clause discharge
classes (compile-discharged; plan-compiled runtime assertions,
zero-cost when off; monitor-checked per-invocation `writes` verdicts
reading the journal; live-agent-only navigation clauses); symbolic op
budgets (`base + Σ rows(collection) × perRow`, never scalar); fan-out
and plan-size bounds; ratcheted inferred budgets; op-count as a proxy
for cost, never presented as a performance guarantee.

## 11. Error handling

The language is **fail-closed and total** at every boundary:

- **Compile rejections** carry a conventional display fix-it and a
  structured `closedTypeFixIt`. **Every compile rejection this
  edition defines is classifiable**: the shipped stable table is
  `closed-types/prop-annotation`, `closed-types/shape`,
  `closed-types/cursor`, `closed-types/sum-constructor`,
  `closed-types/restructure`, `closed-types/nonexhaustive`; the
  conservatism stamp `closed-types/deep-write-conservative` is
  0489-required and **not yet in the shipped table** (a named
  burn-down item, due before the flag flips). Rejections owned by
  individual sections divide honestly: those whose owning section pins
  a code carry it; those described today only as "compile error with
  fix-it" — **the enumerated code-owing list: aggregate `==`, option
  truthiness, mixed arithmetic, bare `none`, state annotations,
  layout-relation storage, non-option `??` left operands** — owe a
  pinned stable code **before the flag flips** (a recorded pre-flip
  obligation, §13 item 15), and **compilers** (the class owning
  compile diagnostics, §2.2) MUST NOT
  invent divergent codes meanwhile. The 0489-mandated removal of the
  legacy `ephemeral-action-parameter` descriptor from the promoted
  language is likewise carried: its two surviving tool emitters
  (Exact Native preflight; the I6 admission validator) are a named
  burn-down due before the flip (§14's migration). The stdlib rejection is
  **compile-time** (both codes, §6.6: `stdlib_member_outside_v1`,
  `stdlib_namespace_call_outside_v1` — warning today, rejection at the
  flip), never a runtime error. No fix-it may recommend a state
  annotation or
  widening. One rejection per site; a precise diagnosis replaces a
  provisional restructure.
- **Engine rejection** of unadmitted constructs is by actionable name
  (LLP 0328 D10); silently dropping an admitted field is a schema
  failure (the §10.4 coverage gate).
- **Runtime errors** — a non-exhaustive roster; the owning sections
  govern: memo comparator misuse, duplicate keys at
  collection application, thenable derives (`thenable-derive`),
  bounded-target no-match, deadline/temporal misuse, the **§6.7
  host-admission structural-domain errors** (sparse arrays, extra
  properties, symbols, accessors, cycles, mixed row arrays,
  missing/non-scalar/duplicate row keys — checked before getters
  run), and the §9 shape
  diagnostics (`contract-shape-validation` /
  `resource-shape-mismatch`; `contract-resource-rejection` /
  `resource-initializer-rejected`). An error's engine-independent shape is
  pinned **where its owning section pins one**; a listed error whose
  section does not yet pin a shape owes that shape before the flag
  flips (§13 item 15) — engines MUST NOT invent divergent shapes
  meanwhile. Where
  the type system made a former runtime error unreachable (member
  access on absence), no engine reintroduces it.
- **Transitions** are atomic: a trap unwinds the journal in reverse,
  discards commands, and logs the event `aborted` with the trap locus.
- **Stale delivery at the decided fences** — settlement targets and
  lifetime-generation-fenced publish (§10.3) — is suppressed, never
  executed, and settlements check target liveness before consumption.
  Replay-*recording* of the suppression (`stale-target`) is part of
  the declined 0330 §4 amendment text (0485 §7.4), not decided law —
  an obligation only if that posture is later adopted; broader
  stale-target suppression is the §10.3 open issue's conservative
  MAY, likewise not decided law.

## 12. The conformance suite

### 12.1 Existing instruments (registration status stated per item)

The fixture-contract mechanics — manifest and script ops,
canonicalization, unknown-opcode hard failure, and the comparison
rules — are **0330 §6, incorporated by reference in full**, and the
**A8 projection/comparison rule itself lives in 0330 §8** (§6 only
references it) and is incorporated alongside (§14); this section adds
no second protocol.

- The language-neutral fixture corpus and four golden schemas
  (`packages/exact-contract/src/conformance/semantics/`): decoded
  protocol record, semantics tree at quiesce, contract witness,
  kernel tree snapshot — quiesce-equivalence comparison, never
  byte-identical streams; differential fuzz beside it (runs via the
  package test suites).
- The `.contract` vitest corpus (`cd js && bun run test`).
- The closed-types rejection corpus and report (0489 §5.1's
  instrument).
- The apply-order proof (0501 §6.4) — a registered check since
  2026-08-26: `contract-apply-order-proof`
  (`scripts/check-apply-order-proof.mjs`, `contract` and
  `contract-ci-core` profiles) re-derives the proof from live LLP
  0330/0489/0501 bytes and reddens when the recorded amended SHA-256
  drifts (`issues/closed/20260820-apply-order-proof-registered-check.md`).
  Retired at ratification (the merged text has no application order),
  retained as provenance.
- The registered `contract` verify profile
  (`bun scripts/exact-verify.mjs --profile contract`) — the one
  entry here that is a registered runner today.

### 12.2 What Edition 1's suite MUST add

1. **Option-only absence fixtures**: seam normalization (null →
   `none`, defined → `some`), `??`/`?.` typing and flattening,
   truthiness rejections, bare-`none` expectation rules, `opt == none`
   emptiness.
2. **Three-relation differential fixtures**: fresh-equal aggregates
   (publish), callback environments (canonical relation), COW vs
   in-place (byte-identical observables), value-kind publication rows.
3. **One-key-law fixtures** including the empty-string key and the
   burn-down of the named §5.5 divergences.
4. **Region fixtures**: bound-key removal (`some` must not survive),
   pure-reorder key-set-generation bump with post-transition cursor
   binding, component-call target switch (remount), surviving-callee
   input propagation ordering, dual-bounded children disposal,
   generation non-aliasing across destroy/recreate.
5. **Reducer fixtures** (with §10.3): overlapping manual task starts,
   stale-target suppression, synchronous host-event dispatch under the
   decided reject branch (a host event observed mid-timer-drain
   dispatches at delivery, not through an admission queue), journal
   repeated-address and whole-slot-then-field cases, cancellation
   fence at each checkpoint. Also §10.4's compile-evidence
   obligations: schema-total disposition (a newly added undispositioned
   IR field fails), pager-field closure, graphics id+digest join, and
   the builder read-set audit.
6. **The oracle discipline**: every fixture green on the Rust engine
   is green on the TS oracle under the same script, and divergence is
   a release blocker for the flag, not a quarantine row (RFC 0496's
   non-quarantinable core).
7. **The 0489 all-engine families (carried whole — 0489 §5.2/§6.1's
   **fixture families**, which this ratification inherits undiminished;
   §6.1's pre-adoption *decision* items follow §13's posture —
   recorded, none blocking; and §6.1 item 5's **migration-evidence
   limb** — the §5.1 ergonomics-bar evidence: accepted fix-its,
   remaining restructures, escape attempts — is carried undiminished
   at 0489 §6.1's own timing: a **precondition to this ratification**
   (ratification adopts 0489; the flag flip is later and is not the
   gate), alongside §13 item 15's corpus report,
   which does not replace it):**
   positive and negative closed inference/descriptor fixtures, shape
   `ready`/`error` settlement, hostile-value tier distinctions, sums
   and missing-arm diagnostics, A-VAL
   copies/equality/publication/memo/replay cases, deterministic layout
   descriptor comparison, the chosen stale-arm behavior,
   **required-prop diagnostics and validated root/import boundaries
   (no `undefined` at component and mount seams)**, and callback
   effect-row enforcement with derived-tolerance unification — on
   every claiming engine, with unsupported engines rejecting by name.
8. **Hooks for this edition's own MUSTs** (every normative MUST binds
   to an executable obligation — this list plus the family hooks
   above; unlisted MUSTs bind through their owning family): deterministic Region ids for
   identical admitted IR (§8.1), guarded resource dependency edges and
   the one post-transition tuple comparison (§10.2), command
   non-reentrancy (§10.3 — commands never synchronously re-enter the
   dispatcher), the no-synthesized-`undefined` rule (§4.1), the
   `closed-types/deep-write-conservative` stamp's presence in the
   shipped table (§11's named burn-down), and the §6.7
   structural-domain rejections (each named case exercised).
9. **The 0501 §4.1 conformance gate, carried whole** (incorporated by
   reference from 0501's gate blockquote): compile evidence compares
   the exact kind inventory, parent/template ownership,
   construct-specific fields, cell arm universes, pager field closure,
   publish effects, and graphics reference/sub-program joins; runtime
   evidence always exercises keyed create/reuse/dispose and reorder,
   Region reconciliation, logical publication equivalence, one-batch
   flush, and cancellation. Hierarchical address dirtiness, guard and
   cursor ordering, settlement rows, meters, a `TransitionArtifact`,
   and rollback/replay are exercised only by a conformance profile
   claiming the separately adopted amendments defining those terms; a
   data-only compiler model is not runtime or all-engine conformance.

## 13. Open issues

Collected from the merged documents. Items 1–14 are owner decisions and
none blocks implementation of the decided text. Items 15–18 are recorded
obligations, not decisions; each states its own timing.

1. 0489 §4.1 — authored generics/bounds syntax (monomorphic until
   then).
2. 0489 §4.2 — non-finite numbers at the boundary/subtypes.
3. 0489 §4.3 — non-discriminated sums.
4. 0489 §4.4 — resource `stale` reachability disposition.
5. 0489 §4.5 / 0501 §4.5 — ordinary compiler result custody.
6. 0489 §4.6 — key-universe enforcement-point burn-down.
7. 0489 §4.7 — `memo(comparator)` under full A-VAL.
8. 0489 §4.8 — nominal Option surface alternative.
9. 0501 §4.1 (W2-D1) — snippet-Region id stability.
10. 0501 §4.2 — settlement-event co-design with change 3.
11. 0501 §4.3 — duplicate keys in a keyed Region (compile-promotion
    extent).
12. 0501 §4.4 — first-frame bake-and-patch refinements.
13. 0330 residual ambiguities: A2 (integer-like object-key
    reordering), A4 (async action segmentation — **resolved by
    exclusion, no Edition-1 ambiguity remains**: 0330 §4's
    segmentation clause is excluded from §14's frozen carve and
    superseded by §10.3's awaited-action replacement per the decided
    0505 posture; the declined inbox amendment would have narrowed
    the old clause and does not apply in v1), A5 (case-mapping table
    version), A6 (non-primitive
    interpolation for closed programs), `**`.
14. The RFC 0401 production-stable instance-address amendment (§8.4's
    stability precondition) and the RFC 0498 runtime-cell-key
    amendment (§8.4's cell variant) — tracked dependencies, not
    Edition-1 blockers.
15. **Recorded pre-flip obligations (not decisions):** the §11
    code-owing list's pinned stable codes; the engine-independent
    shapes for §11's shape-owing runtime errors; the
    `closed-types/deep-write-conservative` stamp landing in the
    shipped table; the `ephemeral-action-parameter` emitter burn-down
    (§11/§14); and 0501 §6.1 item 6's report of a representative app
    corpus and **every rejection/fix-it class** — all due before the
    `contract-v1` flag flips, none blocking implementation of the
    decided text meanwhile.

16. **Recorded Edition-2 fold obligation (not a decision; not a
    `contract-v1` pre-flip obligation):** LLP 0521 is the accepted numbered
    §14 amendment governing explicitly flagged `staticDependencies`
    artifacts. Contract Language Edition 2 must fold its graph authority,
    occurrence/grain/guard/cursor/external-edge rules, shadow/deletion gate,
    stable authority-corruption classification, containment rules, and
    resolved SD decisions into §§7/8/10/11/12, reusing §10.4 totality and
    §12.2 items 8–9. This obligation is due when Edition 2 is drafted and
    adopted; it does not gate the Edition-1 `contract-v1` flag flip.
    (Accepted by Charlie Cheever, 2026-08-21, with LLP 0521's ratification.)

17. **Recorded Edition-2 ActionSegments fold obligation (not a decision; not
    a `contract-v1` pre-flip obligation):** LLP 0535 is the accepted numbered
    §14 amendment governing explicitly flagged `actionSegments` artifacts.
    Contract Language Edition 2 must fold its per-segment transaction and
    atomic-release law including the restoration-failure and hard-failure
    cleanup-arbitration rules,
    descriptor-shaped await admission, editable carried-local snapshots plus
    the required closed typed Plan/native representation,
    synchronous/attached child rules, explicit concurrency,
    interruption/HMR/terminal/task rules, LLP 0521 composition and
    containment, Plan encoding, native sequencing, and resolved AS decisions
    into §§7/8/10/11/12, while preserving item 16's graph authority. This
    obligation is due when Edition 2 is drafted and adopted; it does not gate
    the Edition-1 `contract-v1` flag flip.
    (Adopted by Charlie Cheever, 2026-08-22.)

18. **Recorded Edition-2 Contract forms fold obligation (not a decision; not
    a `contract-v1` pre-flip obligation):** LLP 0539 is the accepted numbered
    §14 amendment governing explicitly flagged `contractForms` artifacts.
    Contract Language Edition 2 must fold its lowercase `form` grammar and
    attrs, typed GET and routed-mutation target spelling, containment,
    control and named/unnamed submitter participation, successful-control
    ordering, default/Enter behavior, exact projection to LLP 0091's single
    `SemanticFormRecord`, compiler `executionAuthority` classification,
    stable diagnostics, Package 4–9 fail-closed gates, and the explicit
    deferral of form contract clauses into §§7/8/10/11/12, while
    preserving items 16 and 17's graph and ActionSegment authorities. This
    obligation is due when Edition 2 is drafted and adopted; it does not gate
    the Edition-1 `contract-v1` flag flip.
    (Adopted by Charlie Cheever, 2026-08-22.)

19. **Recorded W2 decision + Edition-2 island fold obligation (a taken
    decision; not a `contract-v1` pre-flip obligation):** RFC 0518 is the
    accepted numbered §14 amendment recording the 0481-OQ1-in-W2
    dynamic-ceiling decision — the ruling LLP 0480 §15.1 named "the single
    most important unresolved question in the series," routed here as the
    ratification vehicle by 0518 ask 1. **Ruling A:** dynamic data access is
    answered in-language, never by escape — LLP 0485 §6.2 (the `cursor` plus
    visible, budget-enforced widening, option-shaped out-of-bounds), already
    frozen incorporated text of this edition via §10, is confirmed as the
    shared data-access rule; an escape that merely reads program state and
    selects within it is rejected with a cursor/widening fix-it.
    **Ruling B:** computation escapes are declared islands in exactly three
    lanes (pure priced call / provider-shaped / command-shaped) under
    RFC 0518 §§4–6's treaty, with the no-fourth-lane rule; the initial
    discretionary whitelist is **empty** (0518 OQ2 decided: nothing beyond
    the treaty's own named §6 baseline). Contract Language Edition 2 must
    fold the island boundary (declaration, lanes, invariants, reason-tag
    governance) into §§7/8/10/11/12, preserving items 16–18's authorities.
    This obligation is due when Edition 2 is drafted and adopted; it does not
    gate the Edition-1 `contract-v1` flag flip. (Adopted by Charlie Cheever,
    2026-08-25, decision relayed via orchestration session exact-9e.)

20. **Recorded Edition-2 localization-authoring fold obligation (not a
    decision; not a `contract-v1` pre-flip obligation):** RFC 0536 is
    Accepted (2026-08-22) and names three authoring amendments against this
    edition. Contract Language Edition 2 must fold: the **`text raw=`
    opt-out** marking a never-localized literal (RFC 0536 §3.1 — the
    compiler extracts text nodes *and* user-visible attribute strings such
    as `label` and accessibility strings into the base-locale catalog, so
    the opt-out is the only authored way to say "this string is not copy";
    `testId`s are excluded by construction and need no opt-out); the
    **logical layout attrs** (start/end forms alongside the physical ones,
    RFC 0536 §3.4, lowering through RFC 0491 WS-I's generated
    direction-resolution tables, with LLP 0520's machine schema as the web
    seam); and the **`has text key=` contract-block selector** (RFC 0536
    §3.7, jointly with `llp/contract/0085`, so a claim can assert text by
    catalog key and therefore hold in any locale). Two boundaries carried
    from the source: the catalog id and collision-handling authority is
    LLP 0485 §3.3.15's frozen EPLC schema and this edition defines no
    competing site scheme, and the matching **EPLC never-localized
    classification** is LLP 0485's own amendment, not this document's. This
    obligation is due when Edition 2 is drafted and adopted; it does not
    gate the Edition-1 `contract-v1` flag flip.
    (Recorded 2026-08-26 under RFC 0536's acceptance; 0504 §3 row 52c.)

21. **Recorded Edition-2 progressive-delivery-authoring fold obligation (not
    a decision; not a `contract-v1` pre-flip obligation):** RFC 0537 is
    Accepted (2026-08-22) and names its authoring surface against this
    edition and RFC 0498. Contract Language Edition 2 must fold the
    **deferred-binding annotation** by which a streaming-capable route
    declares which data bindings are deferred (RFC 0537 §2.1) and the
    **authored footprint-policy grammar** per deferred region — reserved
    geometry, aspect ratio, overlay, or a declared layout-shift budget —
    which out-of-order backfill requires. The **surface is named here; its
    spelling is not decided**: whether the pending presentation is an
    authored **`when pending`** fallback or a plan-defined default skeleton
    is RFC 0537's own **OQ2**, still open, and a named amendment against
    this edition **either way**. Recording the obligation does not pick
    the arm. Two constraints inherited from the source and worth keeping
    visible in the fold: a deferred cell is present in the first flush in
    LLP 0509's `pending` data-axis state, so a consumer that receives no
    further bytes still holds an honest document with a plan-defined
    pending presentation; and settlement segments are not HTML, so a
    no-engine consumer gets the pending shell and never the deferred data
    unless RFC 0537's document-order lowering ships (its OQ1, LLP 0483's
    call). This obligation is due when Edition 2 is drafted and adopted; it
    does not gate the Edition-1 `contract-v1` flag flip.
    (Recorded 2026-08-26 under RFC 0537's acceptance; 0504 §3 row 53c.)

22. **Recorded Edition-2 capability-import routing (a routing record, not an
    amendment; not a `contract-v1` pre-flip obligation):** RFC 0499 C-3's
    **Contract capability-import grammar** (`use … from capability(…)`) was
    scoped to land "inside the LLP 0485 grammar window" (RFC 0499 §6). That
    window **closed with this edition's freeze on 2026-08-20**, so the
    grammar is re-homed onto §14's numbered-amendment clock as Edition-2
    material — the same route LLP 0521, LLP 0535, LLP 0539, and RFC 0518
    took. Stated plainly so no one waits on the wrong door: **no numbered
    amendment can be minted for it yet.** Every §14 numbered amendment
    names an accepted governing document, and none exists for this grammar
    — RFC 0499 C-3 names the surface (import grammar, Rust typed handles,
    manifest declarations, compiler enforcement on no-JS-guarantee routes)
    without specifying it. The obligation is therefore on **RFC 0499 C-3's
    owner** to supply the grammar, in C-3 or a spec it commissions; when it
    exists it enters §14 as a numbered amendment and Edition 2 folds it.
    Nothing about C-3's other halves — the §4.4 capabilitize-or-forbid
    sweep and its `native-total` exit — is affected by this routing.
    (Recorded 2026-08-26; 0504 §3 row 6, and the routing half of
    RFC 0499 C-3.)

## 14. Ratification and migration

Per LLP 0505 r3 row 3 (decided): **one version, one flag, one
migration.**

- **One version:** this document, Edition 1. Post-ratification changes
  are numbered errata/amendments against this edition.
- **One flag:** `contract-v1` replaces the per-amendment flags; a
  compiler/engine pair is either v0 (pre-ratification behavior) or v1
  (this edition). No intermediate flag combinations are supported or
  tested.
- **One migration:** the Option-only codemod (nullable →
  `Option<T>` at seams; the `temporal` `nextChangeAt` delta of §7.4),
  the `==`-on-aggregates fix-its, the await-action codemod (§10.3),
  the stdlib warning→rejection promotion (§6.6), the
  `ephemeral-action-parameter` emitter burn-down (0489's removal
  mandate: Exact Native preflight and the I6 admission validator stop
  emitting the descriptor; the closed checker never emits it), and the
  key-law
  burn-down land as one migration wave over the first-party corpus,
  with the migration census report-only until the flag flips and
  mandatory at the flip. Engine/oracle v1 conformance (§2.2) is
  claimed at the flip, not before.
- **Supersession (with the frozen-incorporated-text carve):** on
  ratification, 0330/0489/0501 move to `Superseded` **as documents**,
  while every section this edition incorporates by reference remains
  **frozen incorporated text, in force**: 0330 **§4 (ordering,
  quiesce, timer, and dispatch invariants — as restated in §7.2 with
  the named deltas, and excluding §4's async-action-segmentation/A4
  clause, which §10.3's awaited-action replacement supersedes per the
  decided 0505 posture), §5 (the determinism seam's installed/ambient
  split), §6 (the conformance fixture contract: manifest and script
  ops, canonicalization, unknown-opcode hard failure, comparison
  rules), §8's A8 projection/comparison rule**, §9 (host-observable
  equality), §10 (temporal, with §7.4's delta), and §11 (effects and
  replay, as amended); 0489 r6's boundary-parse rules (§9's source)
  and stabilization-policy domains (§6.7's source); and 0501's
  construct tables and schema-total gate (§8/§10.4's sources).
  Supersession retires a document's *standing as an independent
  authority*; it never repeals an incorporated section, **and it never
  widens an admission boundary**: 0489 R6's matrix-gated exclusion
  inventory and 0501's classification-does-not-admit rule (both
  carried in §2.3) remain in force verbatim after supersession —
  ratifying this edition admits no construct those rules gate.
  0485 §§5–9
  remain frozen and incorporated; the `exact-contracts.json` language
  row points here. Whether the LLP 0506 window-exit conjunct (b) adds
  this edition's conformance corpus to its named corpus list is a
  **recorded 0506 ask** — 0506's current list does not include a
  language corpus, and this edition does not widen it by assertion.
  *(Decided 2026-08-23: yes — Charlie, per LLP 0551 §3 decision 5;
  0506 D1(b) now names the §12 corpus. Record edit under the author's
  decision; 0504 §3 row 87 is folded.)*

**Numbered amendment — ActionSegments (Edition 2 material):** LLP 0535 is
the accepted amendment governing explicitly flagged `actionSegments`
artifacts. It conditions §7.2 batch atomicity to ActionSegment grain and
§10.3's no-await reducer rule only for that explicit mode, composes with LLP
0521's `staticDependencies` amendment, and leaves Edition 1, the
`contract-v1` flag, and the one-migration rule unchanged. Its normative fold
is due in Contract Language Edition 2; it is not a `contract-v1` pre-flip
obligation and does not itself authorize a default, Flat Plan admission, or
native admission.

**Numbered amendment — Contract forms (Edition 2 material):** LLP 0539 is
the accepted amendment governing explicitly flagged `contractForms`
artifacts. Its lowercase `form` node, sanctioned control/submitter attrs,
typed GET and routed-mutation target spelling, containment and diagnostics
project LLP 0091's single `SemanticFormRecord`; they do not create a second
form semantics. The amendment preserves LLP 0538's GET exception and LLP
0537's server-authoritative mutation POST, composes with LLP 0521 and LLP
0535, and leaves Edition 1, the `contract-v1` flag, and the one-migration
rule unchanged. Its normative fold is due in Contract Language Edition 2;
it is not a `contract-v1` pre-flip obligation and does not itself authorize
a default, HTML/native lowering, progressive execution, server Plan action,
multipart transport, or form contract clauses.

**Numbered amendment — The Dynamic Ceiling (RFC 0518; Edition 2 material for
the island fold):** RFC 0518 is the accepted amendment recording the
0481-OQ1-in-W2 decision (Charlie Cheever, 2026-08-25, via exact-9e; §13
item 19). Ruling A adds no text — it confirms this edition's incorporated
0485 §6.2 as the one data-access rule and rules the escape hatch
**unavailable** for data access. Ruling B governs explicitly declared
computation islands: exactly three lanes (RFC 0518 §4's inventory), the §5
invariants (local visible totality degradation, never on the frame path, no
authority laundering, observed-not-proven evidence labeling, claimable
budgets), and §6's reason-tag/census governance. It does not itself admit
any island use — the initial discretionary whitelist is empty (0518 OQ2:
nothing) — and it does not authorize a default, Flat Plan admission beyond
0485 Amendment 3's activated format surface, or native admission. The
amendment composes with LLP 0521, LLP 0535, and LLP 0539, and leaves
Edition 1, the `contract-v1` flag, and the one-migration rule unchanged. Its
normative island fold is due in Contract Language Edition 2.

**Numbered amendment — Localization authoring (RFC 0536; Edition 2
material):** RFC 0536 is the accepted amendment (2026-08-22) naming three
authoring additions: the `text raw=` never-localized opt-out, logical
layout attrs (start/end) lowering through RFC 0491 WS-I's generated
direction tables, and the `has text key=` contract-block selector shared
with `llp/contract/0085`. It composes with LLP 0521, LLP 0535, LLP 0539,
and RFC 0518, and leaves Edition 1, the `contract-v1` flag, and the
one-migration rule unchanged. It does **not** define a string-id or
collision scheme — that authority is LLP 0485 §3.3.15's frozen EPLC schema,
and the never-localized site classification is LLP 0485's own matching
amendment. It does not authorize a default, a runtime locale bus (the host
sets a plan input; apps observe state, not events), or any data-tier
localization: dynamic strings are explicitly out of RFC 0536's scope and
localize at the data tier under LLP 0513/0498/0509. Its normative fold is
due in Contract Language Edition 2; it is not a `contract-v1` pre-flip
obligation. (§13 item 20; 0504 §3 row 52c.)

**Numbered amendment — Progressive-delivery authoring (RFC 0537; Edition 2
material):** RFC 0537 is the accepted amendment (2026-08-22) naming the
deferred-binding annotation and the authored footprint-policy grammar for
deferred regions. The **spelling of the pending presentation is not settled
by this amendment** — authored `when pending` fallback versus a
plan-defined default skeleton is RFC 0537's OQ2, open, and a named
amendment against this edition either way. It composes with LLP 0521,
LLP 0535, LLP 0539, and RFC 0518, and leaves Edition 1, the `contract-v1`
flag, and the one-migration rule unchanged. It does not authorize a
default, HTML lowering of settlement segments (RFC 0537's OQ1, LLP 0483's
call), the stream envelope itself (a named LLP 0513-family amendment), or
native admission. Its normative fold is due in Contract Language
Edition 2; it is not a `contract-v1` pre-flip obligation. (§13 item 21;
0504 §3 row 53c.)

**Routing record — capability-import grammar (RFC 0499 C-3):** not a
numbered amendment, because none can be minted yet. The
`use … from capability(…)` grammar was scoped to the LLP 0485 grammar
window, which closed with this edition's 2026-08-20 freeze; it is re-homed
onto this clock as Edition-2 material, and the obligation to specify it
sits with RFC 0499 C-3's owner. See §13 item 22. (0504 §3 row 6.)

**Window relation for the three records above.**
LLP 0504 §3 row 6 is non-window (D2).
LLP 0504 §3 row 52c is non-window (D2).
LLP 0504 §3 row 53c is non-window (D2).
LLP 0504 §3 row 76 is non-window (D2) — it is the tracking row for all
six §14 Edition-2 fold obligations, so it inherits their standing by
construction and cannot outrank them.
All three of the records above are Edition-2 material
by construction: none is a `contract-v1` pre-flip obligation, none
changes Edition 1's normative surface, and the one-version /
one-flag / one-migration rule above is unchanged by each of them.
LLP 0506 D1(b) names this edition's **§12 conformance corpus**, not
its Edition-2 fold obligations, so the corpus that must pass in-window
is unaffected by whether these three have folded. Stated here rather
than in the map, so the classification travels with the amendments.

## Appendix A — Source-document mapping (non-normative)

| Edition-1 section | Source |
| --- | --- |
| §4.1–4.3, 4.6–4.7 | 0489 r6 R3 (amended 0330 §2) |
| §4.2 seam rule | 0489 r6 R3 + R6 (§7 shape) |
| §4.4 A-VAL | 0489 r6 R3/R5; 0485 §7.5 |
| §4.5 effect rows | 0489 r6 R3; 0485 §7.2/§7.3 |
| §5 | 0489 r6 R5 (amended 0330 §2.1) |
| §5.5 | 0489 r6 R3; 0485 §6.2 |
| §6.1–6.5 | 0489 r6 R3/R4 (amended 0330 §2/§8) |
| §6.6 | 0330 §3 |
| §6.7 | 0489 r6 R5 |
| §6.8 | 0489 r6 R6 |
| §7.1–7.2 | 0330 §4; 0489/0501 touches |
| §7.3 | 0330 §9 |
| §7.4 | 0330 §5/§10 |
| §7.5 | 0330 §11 + A1; 0489 R7 (§11.2) |
| §8 | 0501 r5 R1–R3 (amended 0330 §4.1/§6/§7) |
| §9 | 0489 r6 R6 |
| §10 | 0485 §§5–9 (incorporated) |
| §11 | 0489 r6 R3; 0485 §7.5/§8.1 |
| §12 | 0330 §6 and §8 (A8 projection/comparison); 0489 §5; 0496 |


## Window relation (recorded 2026-08-26, LLP 0506 D2)

LLP 0504 §3 row 31 is non-window (D2).
LLP 0504 §3 row 32 is non-window (D2).
Both are Edition-2 authoring material on this edition's §14 clock, in
the atomic batch that leaves Edition 1, the `contract-v1` flag, and
the one-migration rule unchanged. Recorded here rather than in either
batch document, because both are hash-bound at reviewed revisions
awaiting acceptance and no byte of them may change.
