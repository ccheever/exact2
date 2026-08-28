# LLP 0489: Contract closed types amendment

> **Language-authority note (2026-08-20):** the Edition-1 consolidation
> of the Contract language is **LLP 0508** (Review). Its ratification —
> already decided, LLP 0505 r3 row 3 (one merged Contract-v1
> ratification) — supersedes this document as the language authority.
> **Ratified 2026-08-20:** LLP 0508 is Accepted and this document is
> **Superseded** as an independent authority. Per 0508 §14's carve,
> the sections 0508 incorporates by reference remain **frozen
> incorporated text, in force** — supersession retires standing, never
> repeals an incorporated section, and never widens an admission
> boundary. All reading starts at LLP 0508.

**Type:** Spec
**Status:** Superseded
**Systems:** Contract, Compiler, Runtime, Conformance
**Author:** Codex (GPT-5.6-sol), LLP 0485 Track L program
**Date:** 2026-08-20
**Revised:** 2026-08-26 (§5.1 erratum, recorded — evidence-instrument
section only, no normative or incorporated text touched: every §5.1 class
table published before this date omitted the `closed-types/cursor` row and
folded its sites into `closed-types/restructure`, contradicting the JSON
artifact the same section pins by sha256. The corrected class vectors for
both the frozen 2026-08-20 snapshot and the current census are recorded in
the §5.1 erratum block, which also names the four documents the defect
propagated into.)
2026-08-26 (§5.1 census regenerated and re-pinned to
`a0ccebc6…` with the registration of
`contract-closed-types-rejection-report`, landed @`8fe6e8a60` under
LLP 0508 §12.1 demand 12.1-c; this entry is recorded retroactively —
that commit changed §5.1 without a Revised entry.)
2026-08-20 (**SUPERSEDED by LLP 0508's ratification**
(Accepted 2026-08-20): standing as an independent authority retired;
the §14 carve keeps r6's boundary-parse rules, stabilization-policy
domains, and the R6 matrix-gated exclusion inventory as frozen
incorporated text in force — ratification admits no construct those
rules gate.)
2026-08-20 (language-authority banner added — LLP 0508 is
the Edition-1 consolidation; no semantic change)
2026-08-20 (r6 — owner decision applied: **absence is
Option-only** (LLP 0505 r3 row 10, decided by Charlie 2026-08-20 — a
**deliberate reversal** of r5's just-closed nullable fold, taken with
the straight-path lens, not a drift). `T | null` is dropped as an
authored/constructible closed type; `Option<T>` is the only absence
form; `null` survives solely as boundary-parse input, normalized at the
§7 `shape` seam (parsed `null`/absent-optional → `none`, defined `v` →
`some(v)`) — the IR `nullable` marker becomes that boundary-parse
annotation and never denotes a type. Consequential edits: the `??`
typing rule is **re-typed option-left** (option-coalescing; supersedes
r5's nullable-left rule and, deliberately, 0485 §4.3/§5.1's
null-coalescing restatement — recorded for the merged Contract-v1
ratification, 0505 row 3); `?.` requires an option-typed receiver (null
arms removed); the equality table drops the nullable-`null` narrowing
row (no `null` literal type-checks against a program value — the table
stays total); the §2.7 `nullable: true` divergence row becomes a
missing-seam-lowering row (the checker's type rejection is now
correct); §6.1 item 6 tracks the seam lowering instead of a nullable
implementation. `null` remains a boundary-only value kind (0485 §6.2's
frozen key universe is untouched; a `null` key is unreachable from
authored programs).)
2026-08-20 (r5 — dedicated follow-up round 2, both families
NOT READY on r4: the **nullable type `T | null` is made constructible**
(r4 gave nullable values behavior — `??`, `?.`, null narrowing — with no
closed type to inhabit; the IR's `nullable` marker now has language
semantics, and the checker's rejection of it becomes a §2.7 divergence
row); **`??` gains a closed typing rule** (left operand must be
nullable-typed; an option-typed left operand is a compile error with a
`match` fix-it — under the canonical encoding `none` is not `null`, so
`??` would silently never coalesce it); **`?.` typed and flattened**
(option-returning, never nested); the **source-equality operator table
made total** (value-kinds componentwise; payload-free-tag and
payload-free-constructor-literal sum comparisons admitted, payload-bearing
comparisons rejected with a `match` fix-it; non-unifying operand types
rejected; `opt == none` becomes the pinned emptiness test);
`CanonicalValueEquivalenceV1` gains the explicit cross-kind rule and a
component-level definition of value-kind packing; the **one key law admits
the empty string** (0485 §6.2's universe — r4 misdescribed
`identity.ts`'s empty-string rejection as a SameValueZero
canonicalization; it is a divergence, now a §2.7 row with the Rust
state-key serializer); the **Option surface syntactically closed**
(type-directed nullary `none` resolution, `Option<T>` prop spelling,
view-pattern option arms named as an AST/binder extension rather than
"the existing structural match"); compile-time effect rows switched to
**symbolic build-time targets** (declaration id + static callsite path,
never a runtime instance path), and the derived-tolerance narrowing of
Accepted 0485 §7.3 closure 4 made an **explicit filed precedence
erratum** rather than two standing texts; truthiness contexts extended
to ternary conditions; the 0485 §7.5 three-relation refinement likewise
filed as an erratum ask.)
2026-08-20 (r4 — dedicated follow-up round 1, both families
NOT READY on r3: `CanonicalValueEquivalenceV1` is rewritten as a closed,
type-directed **case table** over logical values (it was defined twice —
"the replay-value canonicalization" is enumeration-ordered,
whole-input-alias-rejecting, `undefined`-tagged, and cannot encode
callbacks — the case table is now the single definition and the replay
codec is a **projection** whose byte changes are versioned LLP 0341
amendments); **one key law** — `each` and `memoRows` keys share 0485's
finite-canonical-scalar universe (the `memoRows` "NaN matches NaN"
sentence corrected; the shipping null-rejecting/NaN-admitting domain is
a §2.7 divergence row); a normative-minimum **Option eliminator**
(expression- and statement-position exhaustive `match`) specified, with
`some`/`none` constructor-precedence and layout-relation-payload
exclusions; **option truthiness pinned** (no truthiness — compile error
in `when`/`||`/`&&`/`!` position; the checker's boolean `when` test made
normative); callback effect **tolerance made compiler-derived** with
instance-qualified rows (the r3 declared-tolerance rule had no grammar
or IR to declare it in); callback **generation constant-absent** until
the Region amendment's algebra is adopted, breaking the 0489/0501
adoption cycle; non-optional access on a null-arm type made an explicit
compile error; string-enum ⊂ string subtyping pinned; §3 item 5
retracted in favor of R3/§4.6.)
2026-08-20 (r3 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision: the single-relation overload on `PublicationEquivalenceV1` is
split into **three named relations** — source equality (§2), publication
stability (`PublicationEquivalenceV1`, deliberately not an equivalence
relation), and `CanonicalValueEquivalenceV1` (total, alias-insensitive;
governs callback comparison and replay — aggregates-always-publish can no
longer make a callback unequal to itself or replay checkpoints
incomparable); options get a representation — the **canonical option
type** pinned in R3 (built-in `some(T) | none` tagged sum with
language-defined constructors admitting any closed payload; §4.8 records
the first-class-nominal alternative); the callback effect row completed
per 0485 §7.2 (stateReads/commands/ambient/calls, transitive
composition, receiver-tolerance ⊆ rule); the finite-scalar key universe
moved INTO R3 (SameValueZero, duplicate rejection, error timing) with
§4.6 remaining the decision record; R1's and R6's residual
implementation snapshots ("presently the inference authority"; the TS
engines' `stale` gap) moved to §2.7/§4.4; the deep-state-write MAY gains
a stable feature stamp.)
2026-08-20 (r2 — program super-refine round 1, both families
NOT READY on r1: R3 reconciled with Accepted LLP 0485 §5.1/§4.3/§6.2 —
`??` is **retained as null-coalescing** (its `undefined` arm removed,
never the operator), optional member access **restates as
option-returning with exhaustive match**, and out-of-bounds indexing is
option-shaped, replacing r1's outright deletions; where the shipped
checker implements the stricter deletion, that checker-vs-spec
divergence is now an explicit §6.1 adoption gate; every
implementation-snapshot sentence ("presently the checker…") moved out of
the proposed 0330 redline into §2.7; new adoption gates and W2
decisions added: required-prop diagnostics + validated root/import
boundaries; one cross-engine key universe (0485's finite canonical
scalars proposed; the checker's object-key acceptance recorded as
divergence); `memo(comparator)` vs A-VAL; callback effect-row closure;
`Object.is`-pinned primitive publication equality (NaN/−0) and
per-operator comparison typing; the monomorphic policy named as a
narrowing of 0485 §5.5's generic-instantiation text, decided by W2.)
2026-08-20 (r1)
**Related:** LLP 0485, LLP 0330, LLP 0481,
`issues/20260819-llp0485-track-l-change1-closed-types.md`

## 1. Amendment status, authority, and scope

This is the proposed Track L change-1 amendment that LLP 0330's W2 owner may
adopt or reject. While it is unadopted (this document's status
notwithstanding), LLP 0330 stands and the checker remains
default-off behind `CompileOptions.closedTypes`. This document does not itself
adopt the amendment or lift that flag.

If adopted, §2 amends LLP 0330 at the named sections and promotes the flagged
TS compiler pass to the Contract language definition. It covers only LLP 0485
Track L change 1:

- the closed type language and the removal of observable `undefined`;
- initializer-led, bidirectional inference and its fail-closed diagnostics;
- compiler-derived logical layout metadata;
- the `resource ... as shape ...` boundary-parse paved path;
- the implemented monomorphic form of the generic policy; and
- A-VAL, the value-semantics amendment required by LLP 0485 §7.5 and §15.2.

It does not adopt Track L changes 2–5, a flat-plan representation, physical
arena allocation, a Rust/native execution claim, authored generic syntax,
non-discriminated sums, or a finite-number subtype. It states the narrower
implemented boundary wherever LLP 0485 is broader and fails closed outside it.

The baseline is `4ff92b78c` (A), `7c571a6c8` (B), and `e94e9dc9f` (C). §5
records evidence and work owed before adoption.

## 2. Proposed normative redline of LLP 0330

Line anchors below refer to the current `llp/0330-contract-semantics.spec.md`
in this worktree. Each redline quotes the complete current text being replaced
and then supplies the complete replacement. An adopter can apply these edits in
order without importing any section number from this LLP into LLP 0330.

### 2.1 LLP 0330 §1: authority and conformance

#### Redline R1 — closed-language authority

At LLP 0330 lines 51–55, replace exactly:

> **The narrowing rule (normative).** Where today's TS behavior is
> accidentally JS-ish, the default resolution is to narrow this spec and
> migrate the TS runtime first — Rust implements a spec, it does not emulate
> Hermes. LLP 0186 is the natural TS-side migration vehicle for observable
> narrowings.

with:

> **The narrowing rule (normative).** Where today's TS behavior is
> accidentally JS-ish, the default resolution is to narrow this spec and
> migrate the TS runtime first — Rust implements a spec, it does not emulate
> Hermes. LLP 0186 is the natural TS-side migration vehicle for observable
> narrowings.
>
> **Closed-language authority.** Contract source admitted by the closed-type
> checker is statically typed in every execution tier. A runtime may implement
> the types with different storage, but it may not widen them, introduce an
> `any`-like value, preserve an unvalidated foreign value as typed state, or
> silently accept a construct the checker cannot classify. A tier that does not
> implement an admitted construct rejects that construct by its actionable
> feature name.
>
> The compiler pass this amendment promotes is the inference authority; a
> native or plan tier independently validates descriptors at its ABI or
> admission boundary. Cross-engine conformance never follows from any
> single tier's tests.

#### Redline R2 — conformance evidence

At LLP 0330 lines 57–61, replace exactly:

> **Conformance evidence.** The language-neutral fixture corpus and goldens
> live in `packages/exact-contract/src/conformance/semantics/` (fixtures,
> golden schemas, TS harness, differential fuzz). A behavior change that
> shifts goldens is a semantics change and reconciles against this document
> before the goldens are regenerated.

with:

> **Conformance evidence.** The language-neutral fixture corpus and goldens
> live in `packages/exact-contract/src/conformance/semantics/` (fixtures,
> golden schemas, TS harness, differential fuzz). A behavior change that
> shifts goldens is a semantics change and reconciles against this document
> before the goldens are regenerated. A closed-type semantics claim requires
> positive and negative fixtures on every claiming engine; an unsupported
> engine rejects the construct by name. A TS-only fixture is not an all-engine
> semantics claim.

### 2.2 LLP 0330 §2: closed values, inference, and logical layout

#### Redline R3 — replace the complete value-model bullet list and add the closed-type rules

At LLP 0330 lines 65–91, replace exactly:

> - **Value kinds:** `null`, `boolean`, `number` (IEEE-754 binary64),
>   `string` (UTF-16), `array`, `object` (string-keyed, insertion-ordered),
>   `function` (host/action/arrow — opaque, callable, no further reflection).
>   `undefined` exists as the result of absent members/optional chaining but
>   has **no literal**; the language spells absence `null`.
> - **Equality:** `==`/`!=` in Contract source **lower to strict
>   `===`/`!==`** (a pinned narrowing, not an accident to preserve): no
>   coercion; `NaN != NaN` is true; `-0 == 0` is true. Reference equality for
>   arrays/objects/functions.
> - **Truthiness (pinned):** `false`, `null`, `undefined`, `0`, `-0`, `NaN`,
>   and `""` are falsy; everything else — including `[]` and `{}` — is
>   truthy (fixture `022-toggle-truthiness`).
> - **`??`** returns the right operand iff the left is `null` or
>   `undefined`; `||`/`&&` are truthiness-based and short-circuit
>   (fixture `006-nullish-logical`).
> - **Optional member** `a?.b` yields `undefined` when `a` is
>   null/undefined; **non-optional member on null/undefined is a runtime
>   error** with the engine-independent message shape
>   `[contract] cannot read "<prop>" of <null|undefined>`.
> - **Null/undefined in string interpolation render as the empty string**
>   (template parts only; `String(null)` the conversion callable still
>   yields `"null"`).
> - **Arithmetic** is IEEE-754 binary64 exactly (`0.1 + 0.2` is
>   `0.30000000000000004`); `%` is JS-style remainder (sign of dividend);
>   `/ 0` yields signed Infinity; `0 / 0` yields NaN. `+` concatenates when
>   either operand is a string, numeric-adds otherwise; other mixed-type
>   arithmetic is **unspecified in v0** (ambiguity A1).

with:

> - **Value kinds:** the inhabited primitive types are `null`, `boolean`,
>   `number` (IEEE-754 binary64), and `string` (UTF-16). `number` includes
>   `NaN`, positive Infinity, and negative Infinity; `never` is uninhabited.
>   `null` is a **boundary-only** value kind: no authored type contains
>   it (see the absence rule below), so a checked program observes
>   `null` only inside the §7 `shape` boundary parser, which normalizes
>   it away before a value enters the program.
>   The closed type language also contains finite string enums, uniquely tagged
>   sums, closed objects, homogeneous arrays, the value-kinds `dimension`,
>   `color`, and `duration`, plan-addressable action/callback references, and a
>   sink-only `layout-relation` type. Arbitrary host functions and closures are
>   not storable Contract values.
> - **No observable `undefined`:** `undefined` is not a Contract value and has
>   no type, literal, absent-field arm, optional-access result, interpolation
>   arm, or out-of-bounds indexing result. Every closed-object field is present.
>   An expression that could produce `undefined` rejects with the narrowest
>   fix-it to an option sum, exhaustive match, the `shape`
>   boundary parser in §7, or a restructure.
> - **Equality:** `==`/`!=` on primitives, strings, string enums, and sum tags
>   are strict and perform no coercion; `NaN != NaN` is true and `-0 == 0` is
>   true. A string enum is a **subtype of `string`**: `==`/`!=` and `+`
>   admit an enum operand wherever `string` is admitted (the mixed
>   result types as `string`); two distinct enums compare as their
>   underlying strings. `==`/`!=` on arrays or objects are compile
>   errors with a fix-it to
>   compare explicit fields, keys, or lengths. Action/callback references
>   compare by declaration id, target instance path, and lifetime
>   generation, then
>   pairwise `CanonicalValueEquivalenceV1` (the total, alias-insensitive
>   relation defined in the derived-result stabilization section below)
>   over the typed, ordered curried environment — never by storage
>   reference, and never under the publication-stability relation (which
>   is deliberately not an equivalence relation). **Until LLP 0330
>   separately adopts the Region amendment's lifetime-generation
>   algebra, the generation component is absent** and callback identity
>   is `(declaration id, target instance path)` — this amendment is
>   fully defined and independently adoptable without it; when that
>   algebra is adopted, its generations join the identity unchanged.
>   The remaining operator cases are pinned so the table is **total over
>   the closed universe**: `==`/`!=` on `dimension`/`color`/`duration`
>   operands of one kind compare componentwise over the kind's canonical
>   components (the same components `CanonicalValueEquivalenceV1`
>   reads); mixed value-kind comparison is a compile error. There is no
>   `null`-narrowing comparison form: no authored type carries a `null`
>   arm (see the absence rule below), so a `null` literal operand never
>   type-checks against a program value. A whole-sum or
>   whole-option comparison is admitted in exactly two forms: (a) both
>   operands share a sum type **all of whose arms are payload-free** —
>   the comparison is tag equality; (b) one operand is a
>   **payload-free option constructor literal `none`** (authored sums
>   have no constructor literals in this language; their payload-free
>   comparisons go through form (a)) — a tag test regardless of the
>   `some` arm's payload, so `opt == none` / `opt != none` is the pinned
>   emptiness test. Every other whole-sum or whole-option comparison is
>   a compile error whose fix-it is an exhaustive `match` comparing
>   payload fields explicitly. Operands whose closed types do not unify
>   (outside the string-enum ⊂ string rule) are a compile error.
> - **Truthiness (pinned):** `false`, `null`, `0`, `-0`, `NaN`, and `""` are
>   falsy; every other admitted closed value is truthy. There is no
>   `undefined` case. An **`Option<T>` value has no truthiness**: an
>   option-typed operand in a truthiness position (a `when`/`else`
>   test, a **ternary condition**, an `||`/`&&` operand, a `!` operand
>   — every boolean/truthiness context the grammar has) is a compile
>   error whose fix-it
>   is an exhaustive match or an explicit comparison — `none` is
>   neither falsy nor truthy, so `opt || fallback` can never silently
>   fail to take its fallback and `!opt` can never be constantly
>   `false`. Under the closed checker a `when`/`else` test is **typed
>   `boolean`** (the implemented rule; truthiness coercion of
>   non-boolean `when` tests remains the unflagged runtime behavior
>   only).
> - **`??`, `||`, and `&&`:** `??` is **retained, re-typed as
>   option-coalescing** (the language's only absence is `Option<T>`, so
>   the operator coalesces the only absence there is). **Typing rule
>   (closed):** the left operand of `??` must be **option-typed**
>   (`Option<T>`); `a ?? b` yields `a`'s payload when `a` is
>   `some(payload)` and yields `b` when `a` is `none`; the expression's
>   type is `T` joined with the right operand's type under ordinary
>   closed inference. The coalesce is a total, non-nesting eliminator:
>   when the right operand is itself option-typed the result type is
>   that option (no `Option<Option<…>>` is constructible here or
>   anywhere). A left operand that is not option-typed is a compile
>   error (the coalesce is provably dead). This supersedes the r5
>   nullable-left rule and, deliberately, the null-coalescing
>   restatement Accepted 0485 §4.3/§5.1 carries — nullable types no
>   longer exist to coalesce (LLP 0505 r3 row 10); the §5.1 wording
>   supersession is recorded for the merged Contract-v1 ratification
>   (LLP 0505 row 3) rather than left as two standing texts. `||` and
>   `&&` remain truthiness-based and
>   short-circuit over closed operands.
> - **Member and index access:** missing closed-object members do not
>   exist (every field is present), so member access is total. Optional
>   member access **restates as an option-returning operator**: on an
>   option-typed value, `a?.b` yields an
>   option matched exhaustively (or eliminated by `??`) — it never
>   yields `undefined`. **Typing rule (closed):**
>   `a?.b` requires `a` to be option-typed with an object payload
>   carrying field `b : V`; the result is `Option<V>`, and chained
>   optional access **flattens** — `a?.b` projects the payload's field
>   into a single `Option<V>`, and
>   `a?.b?.c` yields one `Option<W>`, never a nested option.
>   **Non-optional member access
>   on an option-typed value is a compile
>   error** whose fix-it is `?.` with exhaustive match or prior
>   narrowing (0330's former "cannot read of null" runtime-error
>   sentence is subsumed: the situation no longer type-checks, so no
>   engine reaches it). Indexing without a static
>   bounds proof, `find`-style misses, and other stdlib results that
>   formerly had an `undefined` arm are **option-shaped** (`some(T) |
>   none`, per LLP 0485 §6.2) and matched exhaustively. A runtime may
>   not synthesize `undefined`.
> - **Template interpolation:** `null` renders as the empty string in a
>   template part, while `String(null)` still yields `"null"`; there is no
>   `undefined` interpolation arm. Closed array and object operands are
>   admitted with the enclosing template typed `string`, and ambiguity
>   A6's JS `String()` behavior remains in force for those operands
>   pending a later narrowing (see the A6 entry).
> - **Arithmetic:** arithmetic is IEEE-754 binary64 exactly (`0.1 + 0.2` is
>   `0.30000000000000004`); `%` is JS-style remainder (sign of dividend);
>   `/ 0` yields signed Infinity; and `0 / 0` yields NaN. `+` admits exactly
>   `number + number` (numeric addition) or `string + string` (concatenation).
>   `-`, `*`, `/`, and `%` admit exactly `number` with `number`. The
>   relational operators `<`, `<=`, `>`, `>=` admit exactly `number`
>   with `number` (string and other orderings are stdlib calls, never
>   operators; a checker admitting relational operators over wider
>   closed operand types is divergent — §2.7). Every other
>   mixed arithmetic or relational expression is a compile error. This
>   closes ambiguity A1.
>
> **Closed composite types.** A string enum is a finite closed set of string
> literals. A closed object has named fields and no additional properties;
> assignability requires equal field sets and assignable corresponding types.
> An array has one element type. Heterogeneous elements join only when their
> arms form an inferable closed sum. An implemented sum is either a string enum
> or a union of closed-object arms with one shared singleton-string
> discriminant whose tags are unique. Discriminant selection prefers `kind`,
> then `type`, then `tag`, then the lexically first remaining unambiguous field.
> Non-discriminated heterogeneous values reject; this specification adds no
> syntax for them.
>
> **Absence is Option-only.** `Option<T>` is the language's **single
> absence form**. `T | null` is **not a type**: the prop-type grammar
> has no spelling for it, inference never infers a `null` arm (an
> initializer or assignment constraint that would join `T` with `null`
> is a compile error whose fix-it is `Option<T>` with explicit
> `some(...)`/`none`), and no authored, stored, passed, or returned
> value is `null`-typed. The IR's existing `nullable` marker is
> **retained as a boundary-parse annotation only**: it never denotes an
> authored type, and it lowers at the §7 `shape` seam to `Option<T>`
> under the **seam-normalization rule** — a parsed `null` (or absent
> optional field) normalizes to `none`; a parsed defined value `v`
> normalizes to `some(v)`. The same normalization governs every host
> seam that formerly produced nullable data, so `null` crosses into a
> checked program **nowhere**. There is exactly one absence semantics:
> `none` — optional access, misses, out-of-bounds, and boundary absence
> all produce it, and all of §2's option machinery (the eliminator,
> `?.`, `??`, the `opt == none` emptiness test) consumes it.
>
> **The canonical option type.** `Option<T>` is a **built-in sum defined
> by the language**, not an authored discriminated-object sum: its arms
> are `some(T)` and `none`, its constructors `some(value)` and `none` are
> language metadata (so the payload may be any **storable** closed type,
> primitives included — the shared-singleton-string-discriminant
> restriction above governs *authored* sums only; the sink-only
> `layout-relation` type is not storable and is not an admissible
> payload), and it is matched exhaustively like any
> sum. Its canonical object-compatible encoding — used wherever an option
> crosses a boundary that carries plain data — is
> `{kind:"some", value: T} | {kind:"none"}`; its logical layout is the
> ordinary sum layout (tag plus maximum-arm payload). Optional access,
> out-of-bounds indexing, and `find`-style misses produce exactly this
> type. **The surface is syntactically closed:** the prop-type grammar
> spells the type `Option<T>`; `some` is resolved as a constructor only
> in call and pattern position; and the nullary `none` is resolved as a
> constructor in pattern position and in an expression position whose
> **expected type is an option** (a declared `Option<T>` prop
> constraint, the opposite operand of a comparison against an
> option-typed value, or an initializer/assignment position constrained
> to an option through ordinary closed inference) — a bare `none` with
> no option-typed expectation is a compile error naming the ambiguity,
> with a fix-it to introduce the option via `some(...)` or a typed prop
> constraint. An existing plain binding named `some` or `none`
> therefore keeps its lexical meaning outside those positions;
> shadowing a constructor in pattern position is a compile error with a
> rename fix-it (the migration rule for programs that already use those
> names). (§4.8 records the alternative —
> first-class nominal `Option` with
> dedicated surface syntax — as a W2 decision; this pinned-canonical form
> is the recommendation and matches the implemented direction.)
>
> **Option elimination (normative minimum).** An option is consumed by
> exhaustive `match`. In **view position** this is the view `match`
> construct **extended with option arms** `some(binding)` and `none`,
> the binding scoped to its arm — an AST/parser/binder extension, not
> existing surface (today's view `match` arms are `case` plus an
> expression and carry no payload binder; a §2.7 divergence row). In **expression position** (derive formulas,
> action-body right-hand sides) a `match` expression carries the same
> two arms, each yielding a value; the expression's type is the join of
> the arm types under ordinary closed inference. In **statement
> position** (action bodies) a `match` statement's arms are assignment
> blocks under the action's `writes` mask. `some(x)` binds the payload.
> Without this eliminator, optional
> access, option-shaped indexing, and `find`-style results would be
> constructible but not consumable outside a view — so it is part of
> the promoted language, not an ergonomic extra. (Today's parser and IR
> have only the view-structural `match`; no expression- or
> statement-position form exists — a §2.7 divergence row. The fix-it
> constructors and this eliminator land together before the flag
> lifts.)
>
> An action/callback value has typed parameters and a **complete closed
> effect row** (per Accepted 0485 §7.2): authored `writes` (sound
> over-approximation) and `mutates` (provider/capability mutation
> identities, its existing meaning), plus the compiler-derived
> `stateReads` mask, emitted `commands` (task starts, navigation),
> ambient reads, and called-action set — composed **transitively through
> action calls and callback values**. Callback parameters are
> contravariant; effect rows compose covariantly. **Tolerance is
> compiler-derived in this amendment — there is no authored tolerance
> annotation:** a callback prop's effect row is inferred at its
> definition by the same fixed-point unification that types its
> parameters, joining the composed rows of every callback bound at
> every composition site; conflicting row constraints reject once at
> the definition and identify both constraint sites, exactly like
> parameter-type conflicts. A row that crosses a component boundary
> carries **symbolic build-time identities** — declaration id plus
> **static callsite path** (the RFC 0401 path without `each` keys) —
> for its writes, reads, and called actions, never bare state names
> and **never a runtime instance path**: compile-time rows are
> build-time artifacts and runtime instance paths do not exist at
> build time. (The runtime callback *value* separately carries its
> concrete target instance path per §2's equality bullet; the two
> qualifications are distinct by design.) A composed row therefore
> stays meaningful outside
> its defining component and the monomorphic language needs no row
> polymorphism or row variables. An authored per-prop tolerance
> surface (a declared maximum row that composition may not exceed,
> with an empty-row pure default) is a later named amendment; this
> amendment deliberately does not define its grammar or IR, and no
> composition site rejects against a tolerance that cannot yet be
> declared. **Precedence with Accepted 0485 §7.3 closure 4:** that
> closure requires rejecting a callback whose row exceeds the
> receiver's *declared* tolerance; this amendment deliberately
> narrows it — no tolerance can be declared, so no composition site
> rejects against one, and the derived fixed-point row stands in for
> the declared maximum until the tolerance amendment lands. The
> narrowing is a filed 0485 erratum ask
> (`issues/20260820-llp0485-callback-tolerance-closure-erratum.md`);
> W2's adoption records the ruling — the two texts do not both stand
> silently. The value carries
> declaration identity, target instance identity, a typed curried environment,
> and that closed effect row; it does not broaden the authored `mutates` clause.
> The governing Track L specification prescribes removal of the legacy
> `ephemeral-action-parameter` descriptor from the promoted language.
> (Its two surviving tool emitters are an implementation fact recorded
> in §2.7 and §3 item 9, not part of this redline.)
>
> **A-VAL.** Contract values have no observable interior references.
> Assignment copies a value, and two names never alias one mutable value.
> Copy-on-write and uniquely owned in-place storage are permitted only when
> observably equivalent. A member/index state assignment, once admitted, is an
> ordinary write in the write mask and does not alter the authored `mutates`
> clause. Loop iteration observes the collection snapshot selected when
> iteration begins; mutation through a second alias cannot affect that snapshot
> because Contract aliases are not observable.
>
> **Closed-type inference.** Authors write no state type annotations. A state
> annotation is a compile error whose fix-it removes the annotation. State types
> start at their initializers; prop types come from declared prop types. The
> compiler reaches a monotone fixed point across state initializers, whole-state
> action/task assignments, derives, direct identifier references, direct derive
> aliases, callback bindings, and component composition boundaries.
>
> Expected prop types constrain literals, closed objects and arrays, arrows,
> callback payloads, and direct state/derive identifier chains. An expected
> type checks an import, token, intrinsic, host value, or other opaque result;
> it never proves that result. Tokens require a compiler token schema. Imported
> Contract components require project-resolved prop declarations. External data
> requires the boundary parser in §7.
>
> Every action-parameter composition site participates in one unification;
> conflicts reject once at the definition and identify both constraints. An
> undescribed built-in event payload stays untyped and rejects, while a fully
> curried handler can infer from its bound values. String-literal construction
> and writes infer closed string enums. Closed-object construction sites infer
> a tagged sum under the discriminant rule above, even when the arms have
> identical field names. A direct sum scrutinee or direct access to its
> discriminant narrows the bound identifier in a literal `match` arm. A match
> requires complete enumerated arms or `else`; diagnostics sort and enumerate
> missing arms. Access to a field absent from any arm requires prior narrowing.
>
> No `any`, open-object widening, or silent fallback exists. An unresolved
> `never` needs representative construction, a typed prop constraint, a
> `shape`, or restructure. Backward constraints cross direct identifiers and
> derive aliases, not arbitrary member paths; nested member-path sum narrowing
> and untagged heterogeneous construction reject. A checker MAY reject
> deep state writes as a conservative admission boundary (that is not
> reference semantics); member/index writes are admissible only under
> A-VAL. Any such conservative rejection MUST carry the stable feature
> stamp `closed-types/deep-write-conservative`, so a program rejected by
> one engine's conservatism and admitted by another's is a **named,
> visible admission difference** — never silent cross-engine drift in
> which programs compile.
>
> Every closed-type rejection is an error with both a conventional display
> fix-it and a structured `closedTypeFixIt`. Advisory edit arrays may be empty;
> the mandatory part is the narrow resolution and stable classification. The
> stable class/fix-it pairs are `closed-types/prop-annotation` /
> `prop-annotation`, `closed-types/shape` / `shape`,
> `closed-types/sum-constructor` / `sum-constructor`, and
> `closed-types/restructure` or `closed-types/nonexhaustive` / `restructure`.
> No fix-it may recommend a state annotation or widening. A site emits at most
> one rejection; a precise diagnosis replaces a provisional restructure, and a
> failed definition reports once rather than at every use.
>
> **Closed logical layouts.** The compiler derives deterministic,
> target-neutral logical layout metadata from every fully inferred component.
> A component instance is an `arena-struct` with stable logical-cell slot
> offsets and `perSlotHeapCells: false`. This metadata fixes type-directed
> shape; it does not claim that the current TS runtime physically allocates that
> arena.
>
> An admitted `each` site is a struct-of-arrays row arena. A closed object
> contributes one lexically ordered column per bound field; another type
> contributes one bound-value column. Each site adds an inferred-type key column
> and numeric order column. **One key law (normative, cross-engine):**
> every keyed identity in the closed language — `each` keys and
> `memoRows` row keys alike — draws from **one** universe: a key value
> is a canonical finite scalar — `null`, `boolean`, well-formed-UTF-16
> `string` (the **empty string included** — the universe is 0485
> §6.2's, and SameValueZero gives `""` ordinary identity), or finite
> `number` (no `NaN`, no infinities, no aggregates); key
> identity is **SameValueZero** (`-0` and `0` are one key); a duplicate
> key within one collection
> snapshot is an error — at compile time where provable, otherwise a
> runtime error at collection application, before any row mutation. This
> is 0485 §6.2's finite-canonical-scalar rule. The named implementation
> (`compiler/identity.ts`) rejects lone surrogates (the
> well-formedness rule itself) and stores `-0` as `0` (a SameValueZero
> canonicalization). Its **empty-string rejection is neither** — it is
> a divergence from this law, not a canonicalization: SameValueZero
> does not reject `""` and 0485 §6.2's universe admits it. The
> throwing enforcement points (`compiler/identity.ts`; the Rust
> state-key serializer) relax to the law before the flag lifts, while
> the TS view-key path already admits `""` — a §2.7 divergence row.
> `contract_each_identity_key` remains the
> runtime enforcement point (§4.6 records the decision; the checker's
> current object-key acceptance and `memoRows`' current
> null-rejecting, NaN-admitting domain are §2.7 divergence
> rows). A sum-valued `each`
> element remains one bound-value column laid out as a tag plus a maximum-arm
> payload. Sum arms have fixed payload layouts and explicit logical padding;
> the authored discriminant is represented by the tag rather than duplicated in
> the payload. String and array layouts are arena-handle descriptors with
> copy-on-assignment value semantics; copy-on-write is an unobservable
> optimization. Physical bytes, allocation, SoA mutation, Slots emission, and
> certificate consumption are not claimed by this logical metadata.
>
> **Monomorphization policy.** The implemented source language is monomorphic.
> Each definition is checked once. Each complete layout contributes its
> canonical signature and literal instance count; unused components report
> zero. Equal scalar layouts may deduplicate, distinct aggregate or arena
> layouts do not, and the compiler reports a deterministic distinct-signature
> summary. There is no authored generic-parameter or bounds syntax. Opaque
> forms such as `Box<string>` reject with a fix-it naming the missing generic
> surface. Multiple concrete signatures for one authored definition and
> certificate enforcement remain dormant until a later amendment defines the
> source and IR surface.

#### Redline R4 — close ambiguities A1 and A3's `%` half; retain an honest A6 record

At LLP 0330 lines 265–266, replace exactly:

> - **A1 — mixed-type arithmetic** (`5 - "x"`, `true + 1`): unspecified;
>   candidate narrowing = compile-time type diagnostic + runtime error.

with:

> - **A1 — mixed-type arithmetic: CLOSED.** `+` admits exactly
>   `number + number` or `string + string`; `-`, `*`, `/`, and `%` admit
>   exactly `number` with `number`. Every other mixed arithmetic expression is
>   a compile error.

At LLP 0330 line 269, replace exactly:

> - **A3 — `%` on negative non-integers**, `**`: unpinned (no corpus use).

with:

> - **A3 — `%`: CLOSED; `**`: unpinned.** `%` on `number` operands is
>   JS-style binary64 remainder (sign of the dividend), including negative
>   and non-integer operands. `**` remains unpinned (no corpus use).

At LLP 0330 lines 276–278, replace exactly:

> - **A6 — template rendering of arrays/objects**: currently JS `String()`
>   behavior (arrays join with `,`, objects `[object Object]`); candidate
>   narrowing = compile diagnostic for non-primitive interpolation.

with:

> - **A6 — template rendering of arrays/objects: OPEN FOR CLOSED PROGRAMS.**
>   The closed checker currently accepts closed array/object interpolation and
>   types the enclosing template as `string`; the runtime retains JS `String()`
>   behavior (arrays join with `,`, objects render `[object Object]`). A future
>   compile diagnostic for non-primitive interpolation remains a candidate
>   narrowing and requires a later semantics amendment.

### 2.3 LLP 0330 §2.1: complete derived-result-stabilization replacement

#### Redline R5 — reconcile the complete memo/alias complex with A-VAL

At LLP 0330 lines 93–140, replace exactly:

> ### 2.1 Derived result stabilization
>
> A plain `derive` retains the scheduler's ordinary `Object.is` result check.
> The suffixes `memo`, `memo(comparator)`, and `memoRows("key")` are explicit
> result-identity policies applied after the derive formula evaluates and before
> its computed publishes. They never change the formula's dependency tracking.
> Their semantics are:
>
> - **`memo` (shallow):** accepts a Contract scalar, a dense plain array of
>   scalar leaves, or a plain object with scalar leaves. Scalars and leaves
>   compare with `Object.is`. Arrays compare by length and index. Objects
>   compare by the set of own string keys (insertion order is intentionally
>   ignored) and each keyed value. An equal result publishes the previous
>   value by reference; a different result publishes the new value.
> - **`memo(comparator)`:** evaluates the comparator expression exactly once
>   per component or behavior instance, outside reactive dependency tracking.
>   The first derive result is published without invoking it. Later comparisons
>   invoke the captured function as `Reflect.apply(comparator, undefined,
>   [previous, next])`; it must return the boolean primitive. `true` publishes
>   `previous`, `false` publishes `next`. Comparator throws and non-boolean
>   results are runtime errors at the authored derive locus. The compiler
>   rejects action/task/mutation/action-prop/effectful-import dependencies in
>   this pure expression position.
> - **`memoRows("key")`:** recursively reconciles plain data bottom-up. An
>   array containing object rows contains only object rows, and every row has a
>   unique own `key` data property whose value is string, number, or boolean.
>   Key comparison is **SameValueZero**, matching keyed `each`: `NaN` matches
>   `NaN`, and `-0` and `0` are the same key. Matching unchanged rows and
>   nested values retain their previous references; reorder publishes the new
>   array order while reusing row references. Repeated input aliases remain
>   aliases in reconciled output.
>
> The structural domains are intentionally strict and effect-free. Every
> traversed object/array must have the standard `Object.prototype`/null or
> `Array.prototype`, be extensible, and expose only own string-keyed,
> enumerable, writable, configurable data properties; an array's standard
> non-enumerable, writable, non-configurable `length` property is the sole
> exception. Sparse arrays, extra array properties, symbols, accessors,
> nonstandard descriptors, frozen/sealed/non-extensible values, custom
> prototypes, cycles, mixed object/scalar row arrays, missing or non-scalar row
> keys, and duplicate keys are runtime errors. These restrictions are checked
> before authored getters could run.
>
> The feature stamp is `derive-memo`; fixture `023-derive-memo-rows` pins a
> board-style equal refresh followed by one keyed-row change in both TS
> execution modes. Per LLP 0328 D10, Contract Native currently classifies this
> construct as unsupported and must reject it by name; it may never ignore a
> memo policy or silently use plain-derive identity.

with this complete §2.1:

> ### 2.1 Derived result stabilization
>
> **Three value relations, named and never interchanged.** The closed
> value model carries exactly three comparison relations, each with one
> job. (Accepted 0485 §7.5 overloads one relation for publication,
> callback comparison, and aggregate replay; this three-relation
> refinement is a filed 0485 erratum ask —
> `issues/20260820-llp0485-publication-equivalence-overload-erratum.md`
> — so frozen 0485's readers see the split.)
>
> 1. **Source equality** — `==`/`!=` in Contract source (§2): strict on
>    primitives/enums/tags; compile-error on plain aggregates;
>    identity-plus-`CanonicalValueEquivalenceV1` on action/callback
>    references. What authors observe.
> 2. **Publication stability** — `PublicationEquivalenceV1`, below: the
>    dirtiness rule deciding whether a computed value republishes. It is
>    **deliberately not an equivalence relation** (a fresh-equal
>    aggregate still publishes absent an authored memo policy), and for
>    exactly that reason it may never be used where a total relation is
>    required — source `==`, callback comparison, or replay.
> 3. **`CanonicalValueEquivalenceV1`** — the total, alias-insensitive
>    structural equivalence over closed **logical values**, defined by
>    this closed, type-directed case table — never by a byte encoding:
>    - `null`, `boolean`, `number`, `string`, string enums, and sum
>      tags: `Object.is`;
>    - `dimension` / `color` / `duration`: same kind, then componentwise
>      `Object.is` over the kind's canonical components (magnitude and
>      unit; channels and color space; magnitude and unit) — "canonical
>      packed encoding" names the deterministic packing of exactly
>      these components, a projection owned by the layout/plan
>      authority, never a second relation;
>    - values of **different closed type kinds** are never equivalent
>      (there is no cross-kind case), with the single pinned exception
>      that a string-enum value and a `string` compare as their
>      underlying strings;
>    - closed objects: equal own-field **sets** (order-insensitive) and
>      recursive equivalence field-wise;
>    - arrays: equal length and recursive equivalence index-wise;
>    - authored sums and `Option<T>`: equal tag, then recursive
>      equivalence of the active payload;
>    - action/callback references: the callback identity rule —
>      declaration id, target instance path (and lifetime generation
>      once defined; §2's equality bullet), then this relation pairwise
>      over the typed, ordered curried environment (recursion through
>      this same case — nested callbacks compare by identity plus
>      environment, which no byte codec can express);
>    - the sink-only `layout-relation` type inhabits no storable or
>      comparable position, so the table needs no case for it.
>    The table is total over the closed universe by induction;
>    alias-insensitivity holds by construction because it reads logical
>    values and A-VAL makes aliases unobservable. An equivalence
>    relation; governs
>    replay checkpoint comparison and callback curried-environment
>    comparison. The **replay codec is a projection of this table, not
>    its definition**: the shipped `encodeReplayValueV1`
>    (`runtime/replay-value.ts`) writes object keys in ECMAScript
>    enumeration order, rejects aliases across the whole input, retains
>    an `undefined` byte tag, and cannot encode callbacks or
>    value-kinds — it is today's LLP 0341 artifact grammar, not this
>    relation (§2.7). Conforming replay comparison canonicalizes
>    (sorted field sets, no `undefined` arm) before or during
>    comparison, and **any change to the artifact bytes is a versioned
>    amendment to LLP 0341** — the `contract-replay-semantics` authority
>    row in `exact-contracts.json`, whose schema name is
>    `ReplayValueEncodingV1` — never a side effect of adopting this
>    amendment.
>
> Plain derive publication and redundant state writes use
> `PublicationEquivalenceV1`:
>
> - primitives, strings, string enums, and sum tags compare with
>   **`Object.is` exactly** (pinned: `NaN` is publication-equal to `NaN`
>   — no republish; `-0` is publication-distinct from `0` — republish;
>   this is the dirtiness/identity rule and is deliberately not the
>   same relation as source-level `==`, whose §2 semantics keep
>   `NaN != NaN` true and `-0 == 0` true);
> - `dimension`, `color`, and `duration` compare with `Object.is` over their
>   canonical packed encodings;
> - aggregate array/object whole-slot writes and derive results always publish
>   unless an authored `memo` or `memoRows` policy stabilizes them; this does not
>   collapse field-granular dirty addresses;
> - a payload-bearing sum publishes on a tag change and, for an equal tag,
>   compares the active payload recursively under this same rule; and
> - action/callback references compare by declaration id plus target runtime
>   instance path and lifetime generation (per §2's equality bullet, the
>   generation component is absent until the Region amendment's algebra
>   is adopted), then by pairwise recursive
>   **`CanonicalValueEquivalenceV1`** over the typed, ordered curried
>   environment (the total relation — using the aggregate
>   always-publish rule here would make a callback carrying an aggregate
>   environment publication-unequal to itself forever). They
>   never compare by storage handle or an implementation-chosen shallow depth.
>
> Guarded resource dependency tuples use the same rule. A fresh-equal aggregate
> may therefore republish, change the resource identity tuple, and cause
> external cancel/refetch; authored memo stabilization is the recommended
> pattern when identity stability matters. This cost is explicit and is not a
> performance claim.
>
> The suffixes `memo`, `memo(comparator)`, and `memoRows("key")` are explicit
> value-stabilization policies applied after the derive formula evaluates and
> before its computed publishes. They never change the formula's dependency
> tracking, and they stabilize logical values rather than expose or preserve
> aliases. Their semantics are:
>
> - **`memo` (shallow):** accepts a Contract scalar, a dense plain array of
>   scalar leaves, or a plain object with scalar leaves. Scalars and leaves
>   compare with `Object.is`. Arrays compare by length and index. Objects
>   compare by the set of own string keys (insertion order is intentionally
>   ignored) and each keyed value. An equal result selects the previously
>   published logical value; a different result selects the newly evaluated
>   logical value. Physical reference reuse is neither required nor observable.
> - **`memo(comparator)`:** evaluates the comparator expression exactly once
>   per component or behavior instance, outside reactive dependency tracking.
>   The first derive result is published without invoking it. Later comparisons
>   invoke the captured function (the JS-host `thisArg` slot, not a
>   Contract value) as `Reflect.apply(comparator, undefined,
>   [previous, next])`; it must return the boolean primitive. `true` selects the
>   previous logical value and `false` selects the next logical value. Comparator
>   throws and non-boolean results are runtime errors at the authored derive
>   locus. The compiler rejects action/task/mutation/action-prop/effectful-import
>   dependencies in this pure expression position. A conforming implementation
>   may not make physical alias identity observable through this comparison.
> - **`memoRows("key")`:** recursively reconciles plain data bottom-up. An
>   array containing object rows contains only object rows, and every row has a
>   unique own `key` data property whose value draws from §2's **one key
>   law** — a canonical finite scalar (`null`, `boolean`, well-formed
>   `string`, or finite `number`), the same universe as keyed `each`.
>   Key comparison is **SameValueZero** (`-0` and `0` are the same
>   key); `NaN` is not a key anywhere — non-finite numbers reject
>   (correcting r3's "`NaN` matches `NaN`" sentence, which contradicted
>   the `each` universe it claimed to match; the shipping runtime's
>   null-rejecting, NaN-admitting domain is a §2.7 divergence
>   row). Matching unchanged rows and nested
>   values select their previous logical values; reorder publishes the new
>   array order. Repeated input aliasing is neither preserved nor observable.
>
> The structural domains are intentionally strict and effect-free, and
> they are **host-admission conditions for foreign JS values** entering
> the stabilization policies — not part of the closed value model: a
> value already typed by the closed language satisfies them by
> construction. Every
> traversed object/array must have the standard `Object.prototype`/null or
> `Array.prototype`, be extensible, and expose only own string-keyed,
> enumerable, writable, configurable data properties; an array's standard
> non-enumerable, writable, non-configurable `length` property is the sole
> exception. Sparse arrays, extra array properties, symbols, accessors,
> nonstandard descriptors, frozen/sealed/non-extensible values, custom
> prototypes, cycles, mixed object/scalar row arrays, missing or non-scalar row
> keys, and duplicate keys are runtime errors. These restrictions are checked
> before authored getters could run.
>
> The feature stamp is `derive-memo`; fixture `023-derive-memo-rows` pins a
> board-style equal refresh followed by one keyed-row change in both TS
> execution modes. Per LLP 0328 D10, Contract Native currently classifies this
> construct as unsupported and must reject it by name; it may never ignore a
> memo policy or silently use plain-derive identity.

### 2.4 LLP 0330 §7: precise exclusions, shaped resources, and relation intent

#### Redline R6 — rewrite the exclusion sentence and add the admitted boundary forms

At LLP 0330 lines 250–254, replace exactly:

> Resource/query/mutation/ephemeral data surfaces, channels/publish,
> relation (measured-sibling) expressions, behavior templates beyond
> same-file inlining, forms, overlays, JS islands, and the capability ABI —
> all matrix-gated or post-v1 per LLP 0328 D10. Their semantics stay
> defined by their LLPs and TS tests until a spec revision admits them.

with:

> Query, mutation, and ephemeral data surfaces; resource-cell lifecycle
> semantics other than the shaped boundary parse and closed binding types
> specified below; channels/publish; measured-sibling feedback and resolution
> beyond the admitted direct-sink relation intent forms below; behavior
> templates beyond same-file inlining; forms; overlays; JS islands; and the
> capability ABI remain matrix-gated or post-v1 per LLP 0328 D10. Their
> semantics stay defined by their owning LLPs and TS tests until a later spec
> revision admits them.
>
> **Shaped resource boundary.** A resource declaration may opt into a
> compiler-derived boundary parse with the contextual postfix
> `resource name = expression as shape TYPE`. The declaration and postfix
> occupy one Contract logical line. `shape` is not a global keyword, and
> unshaped resources retain their previous IR and settlement path.
>
> `TYPE` has a closed object, uniquely discriminated closed-object sum, or an
> array of one of those at its root. Fields may contain `string`, `number`,
> `boolean`, `null`, `never`, string-literal enums, nested closed objects,
> uniquely discriminated closed-object sums, and homogeneous arrays. Fields are
> comma-separated; a trailing comma is allowed. A shape is exact: extra fields
> are not admitted.
>
> Compilation derives data-only validator IR interpreted without evaluating
> authored type text or using `eval`. A conforming terminal settlement is a
> recursively materialized detached snapshot, never source identity; later
> source mutation cannot affect it. Source objects require
> `Object.prototype` or null, and output is a fresh ordinary object.
>
> For every declared field, materialization accepts any own data property,
> regardless of enumerability, writability, or configurability. A missing own
> property or an accessor rejects without invoking a getter. After declared
> fields are read, every additional own key rejects whether it is a string or a
> symbol and regardless of enumerability. Arrays are dense, accept only their
> own `length` and in-range index data properties, and are rebuilt fresh. Class
> instances, custom prototypes, functions, accessors, and exotic values do not
> cross.
>
> Shape `number` accepts every IEEE-754 binary64 number, including `NaN` and
> both infinities. This specification defines no finite-number subtype. A
> transport that cannot encode non-finite numbers does not change the Contract
> type.
>
> The first mismatch settles the resource's existing error arm and never
> publishes a valid prefix or partial ready value. The published diagnostic is
> plain closed data with exactly `kind`, `code`, `path`, `locus`, `expected`,
> `actual`, and `message`. Validation failures use
> `kind: "contract-shape-validation"` and
> `code: "resource-shape-mismatch"`. An arbitrary initializer rejection is
> projected to the same field surface with
> `kind: "contract-resource-rejection"` and
> `code: "resource-initializer-rejected"`; raw throwable identity, subclass,
> stack, accessors, and reporting metadata never enter the slot value.
>
> Inspecting a live JS proxy may execute hostile reflection traps. A trap may
> reject or fabricate conforming answers, but output is still only a detached
> plain snapshot—never the proxy, getter, capability, or source identity.
> Plan/native byte parsing instead owns its bytes and runs no foreign reflection
> code. Live-value validation is not byte parsing.
>
> Shape diagnostics and validation are construct-gated, not closed-checker
> gated. Malformed `as shape` syntax receives registered parser diagnostics
> when the promoted static pass is off; a valid authored shape validates at
> settlement in that mode. A module without the postfix has no validator and no
> validation branch.
>
> Under the closed checker, shaped `ready` and `stale` bindings have the
> declared closed type and the `error` binding has the diagnostic object
> type. A checker's typing of an arm is never a reachability claim: arm
> reachability stays with the construct's owning resource semantics, and
> shaped-resource conformance may be claimed only for arms the owning
> semantics make reachable. (Which arms today's engines reach, and the
> disposition of the currently-unreachable `stale` arm, are
> implementation facts recorded in §4.4 — deliberately outside this
> redline.)
>
> **Layout-relation intent.** `fill`, `ratio`, `center`, `pinToSafeArea`,
> `matchWidth`, `matchHeight`, and `alignTo` produce the sink-only
> `layout-relation` type. Their arguments are checked against their compiler
> signatures. A relation may flow through a derive but is admitted only as the
> direct value of a recognized built-in layout/style sink; storing it in state,
> nesting it in an array or object, or sending it to another sink is a compile
> error. The TS runtime carries all seven intent forms. `matchWidth`,
> `matchHeight`, and `alignTo` are measured-sibling operations whose direct-sink
> intent and fallback are admitted here; actual measured-sibling feedback and
> resolution remain excluded.

This chooses the reading the runtime ships today: typed direct-sink intent is
in scope, including the measured-sibling operation names, while the unresolved
measurement feedback channel is not.

### 2.5 LLP 0330 §8 A10: reconcile the historical §7 note

#### Redline R7 — scope “§7 is unchanged” to the A10 close

At LLP 0330 lines 296–304, replace exactly:

> - **A10 — thenable derive**: **CLOSED BY DELEGATION (2026-08-13).**
>   This spec owns one cross-engine rule: a `derive` publishes a settled
>   Contract value. After formula evaluation and before memo publication,
>   a thenable (`then` is a function) is a runtime error at the authored
>   locus; diagnostic code `thenable-derive`. Compile-time rejection is
>   allowed only where thenability is statically proven. **§7 is
>   unchanged:** resource / query / mutation cell semantics stay with
>   LLP 0082 / 0259 / 0263 and are not admitted into v0 by this close.
>   Residual executor / action-segmentation questions remain A4.

with:

> - **A10 — thenable derive**: **CLOSED BY DELEGATION (2026-08-13).**
>   This spec owns one cross-engine rule: a `derive` publishes a settled
>   Contract value. After formula evaluation and before memo publication,
>   a thenable (`then` is a function) is a runtime error at the authored
>   locus; diagnostic code `thenable-derive`. Compile-time rejection is
>   allowed only where thenability is statically proven. The A10 close itself
>   does not amend §7: resource/query/mutation cell lifecycle semantics stay
>   with LLP 0082 / LLP 0259 / LLP 0263. §7 now separately admits the shaped
>   resource boundary and direct-sink layout-relation intent described there;
>   neither admission changes A10's thenable rule. Residual executor and action
>   segmentation questions remain A4.

### 2.6 LLP 0330 §11.2: A-VAL replay projection

#### Redline R8 — replace the complete replay-value bullet

At LLP 0330 lines 513–516, replace exactly:

> - Replay values are capped, Object.is-faithful, UTF-16-exact encodings. Seeded
>   mulberry32 values are oracle checks keyed by their lexical site, logical
>   instance, turn, and semantic ordinal. Privacy projection happens before
>   hashing or retention.

with:

> - Primitive replay values are capped, Object.is-faithful, UTF-16-exact
>   encodings. Aggregate replay values are canonically encoded and
>   compared under **`CanonicalValueEquivalenceV1`** (§2.1's total
>   relation — the publication-stability rule would make identical
>   aggregate checkpoints incomparable); physical handle or alias
>   identity is not replay
>   surface. Seeded mulberry32 values are oracle checks keyed by their lexical
>   site, logical instance, turn, and semantic ordinal. Privacy projection
>   happens before hashing or retention.

### 2.7 Redline coverage index and implementation gap

| 0330 location | Quoted current text | Complete replacement |
| --- | --- | --- |
| §1 lines 51–55 | Narrowing-rule paragraph | R1 |
| §1 lines 57–61 | Conformance-evidence paragraph | R2 |
| §2 lines 65–91 | Complete value-model bullet list | R3 |
| §8 lines 265–266 | A1 ambiguity entry | R4 |
| §8 line 269 | A3 ambiguity entry (`%` half closed) | R4 |
| §8 lines 276–278 | A6 ambiguity entry | R4 |
| §2.1 lines 93–140 | Complete derived-result-stabilization section | R5 |
| §7 lines 250–254 | Complete exclusion sentence | R6 |
| §8 lines 296–304 | Complete A10 entry | R7 |
| §11.2 lines 513–516 | Complete replay-value bullet | R8 |

The A-VAL clauses are proposed language semantics, not a claim that chunks A–C
have converted every TS runtime path. Today the detached `shape` snapshot and
logical handle descriptors embody the rule, while the general aggregate
equality diagnostic, publication rule, memo restatement, replay projection, and
all-engine differential fixtures remain pre-adoption work.

**Checker-vs-spec divergence inventory (implementation snapshot, kept
out of the redline; each row is a §6.1 adoption gate):**

- the flagged checker types comparison expressions without an
  aggregate-specific `==`/`!=` rejection, and admits relational
  operators over operand types wider than the redline's
  number-with-number rule;
- the checker implements the r1 *deletions* of `??` and optional access
  rather than the redline's restatements (option-coalescing `??` with
  its option-left typing rule;
  option-returning optional access; option-shaped indexing) — the
  stricter behavior must relax to the spec, or the spec text wins;
- the checker deliberately accepts object-valued `each` keys
  (`closed-types-layout.test.ts`) while the proposed cross-engine key
  universe (below, §4.6) is 0485's finite canonical scalars;
- the shipping `memoRows` key domain (`runtime/memo.ts`) rejects
  `null` and admits `NaN` — both divergent from R3's one key law,
  which admits `null` and rejects non-finite numbers;
- `compiler/identity.ts` and the Rust state-key serializer
  (`contract-native/src/state_schema.rs`) throw on empty-string keys
  while R3's one key law (0485 §6.2's universe) admits `""` and the
  TS view-key path (`runtime/view-key-identity.ts`) already accepts
  it — the throwing enforcement points relax to the law before
  adoption (three engines currently disagree; the law picks one
  answer);
- the checker rejects `nullable: true` prop IR with a sum-constructor
  fix-it (`closed-types/type-system.ts`); under R3's Option-only rule
  the rejection of `nullable` **as an authored type** is correct, but
  the marker must instead **lower to `Option<T>` at the boundary seam**
  (the seam-normalization rule) rather than reject — the divergence is
  the missing lowering, not the missing type;
- declared action props normalize to empty effect rows and
  assignability ignores effects (`closed-types/type-system.ts`) —
  callback effect-row closure is specified but unenforced, and the
  derived-tolerance unification (R3) has no implementation;
- option truthiness is unchecked: the checker types `!` as `boolean`
  over any operand and does not reject option operands in `||`/`&&`
  position (the boolean `when` test is implemented; the rest of R3's
  option-truthiness pin is not);
- same-file checking validates only provided props and reports no
  missing non-defaulted prop; runtime lookup can return `undefined`
  where neither value nor default exists (`runtime/instance.ts`), and
  root mount props are unvalidated (`runtime/mount-core.ts`) — the
  no-observable-`undefined` claim fails at these boundaries today;
- `memo(comparator)` passes retained physical objects to the captured
  function (`runtime/memo.ts`), observably exposing alias identity
  against A-VAL;
- the TS compiler is today the only inference implementation (the
  redline states the authority timelessly; single-authority-in-practice
  is this snapshot fact);
- the current TS resource engines publish `pending`, `ready`, and
  `error` but not `stale` (the R6 typed `stale` arm is statically
  prepared, unreachable today — dispositioned in §4.4);
- no `CanonicalValueEquivalenceV1` implementation exists yet as a named
  relation, and the shipped replay encoder is **not** that relation —
  `encodeReplayValueV1` (`runtime/replay-value.ts`) writes objects in
  enumeration order, rejects aliases across the whole input, retains an
  `undefined` byte tag, and has no callback or value-kind arms; R5's
  case table governs, the codec is its projection, and artifact-byte
  changes are versioned LLP 0341 amendments; callback comparison and
  replay checkpoints do not yet call one shared routine;
- no `Option<T>` constructors exist, no expression- or
  statement-position `match` eliminator exists, and the **view
  `match` has no option-arm payload binder** (today's `match` is
  view-structural `case`-plus-expression only — `compiler/parser.ts`
  / `ir.ts` — so R3's `some(binding)`/`none` view arms are an AST,
  parser, and binder extension, not existing surface) — the
  checker's option-shaped fix-its currently recommend surface that is
  not implemented; R3's eliminator, constructors, and view-pattern
  arms land together.

W2 must not lift the flag until these rows and §5.2's fixture debt are
closed (§6.1).

## 3. Normative adjudication record

These dual-family review clarifications are normative. Items 1–3 come from
`NOTES-l1b.md`'s chunk B round-3 adjudication and commit `7c571a6c8`; items 4–6
come from `NOTES-l1c.md` and chunk C's review in commit `e94e9dc9f`; items 7–9
record the round-2 chunk-D adjudications.

1. **“Lands as the closed type” means a detached value snapshot, never source
   identity.** This is incorporated in redline R6 and A-VAL.
2. **Primitive `number` has no finite subtype.** `NaN` and positive/negative
   Infinity conform; a finite policy would be a future amendment.
3. **The JS and byte-boundary trust guarantees differ.** Live JS validation
   may execute hostile proxy reflection traps and can materialize only plain
   data; plan/native byte parsing runs no foreign reflection code.
4. **Shape diagnostics are construct-gated.** Authoring `as shape` opts into
   its parser diagnostics and settlement validator; `closedTypes` gates only
   the promoted static pass.
5. **Retracted (r4).** This item recorded chunk C's ruling that an
   `each` key column "adds no admission restriction," which read as a
   second normative sentence beside R3's key law. R3's one key law
   (with the §4.6 decision record) now governs key admission; the
   checker's object-key acceptance is a §2.7 divergence row, not
   authority. The item is retracted rather than deleted because
   adjudication records are never silently rewritten.
6. **Layout relations are admissible only as direct layout-sink values.** A
   relation may pass through a derive, but state storage, array/object nesting,
   and non-layout sinks reject.
7. **Null interpolation remains legal.** Literal `null` and a null-typed value
   both render the empty string in a template part; removing observable
   `undefined` does not remove LLP 0330's independent null arm.
8. **Declared shape fields are descriptor-value semantics, not enumerability
   semantics.** Any own data property satisfies the ownership/descriptor half
   of a declared field. Every additional own key rejects, including
   non-enumerable string keys and symbols.
9. **The legacy ephemeral descriptor has two surviving tool paths.** Exact
   Native preflight and the I6 proof/preflight admission validator can still
   emit `ephemeral-action-parameter`; the Track L spec prescribes removal from
   the promoted language, and the closed checker itself never emits that arm.

These decisions may be reversed only by a later named semantics amendment, not
by widening the checker or changing a runtime in isolation.

## 4. Open decisions for W2's owner

### 4.1 Authored generics and bounds syntax

**Decision:** What, if any, source/IR syntax activates LLP 0485 §5.5 beyond its
implemented monomorphic degenerate? (Named plainly: R3's
monomorphic-only policy is a **narrowing** of 0485 §5.5's
check-once/instantiate-layouts-per-signature text — a legitimate later
narrowing only if W2 chooses it explicitly, never an implementation
shortcut promoted by silence.)

- **Option A — monomorphic-only adoption:** adopt redline R3's monomorphic
  policy as written; generic
  spellings remain classed rejections and a later LLP defines parameters,
  bounds, inference, and ABI identity.
- **Option B — explicit parameters and bounds now:** define grammar, bounds,
  composition inference, deterministic IR, and multiple-signature evidence.
- **Option C — declaration-only bounds:** add named generic parameters and
  bounds but require every composition to supply explicit concrete arguments;
  defer call-site type-argument inference.

Only A is implemented. B or C needs parser, IR, diagnostics, fixtures, and
layout-signature evidence.

### 4.2 Non-finite numbers

**Decision:** Does the primitive `number` continue to include every binary64
value?

- **Option A — one binary64 type:** keep `NaN` and both infinities conforming,
  matching the implementation and LLP 0330's arithmetic.
- **Option B — add a finite subtype:** retain `number`, add authored/inferred
  `finite-number`, and define arithmetic closure and boundary diagnostics.
- **Option C — finite by default:** narrow `number` itself and add an explicit
  non-finite-capable type.

Only A is implemented. B or C is a breaking future amendment, not a `shape`
implementation choice.

### 4.3 Non-discriminated sums

**Decision:** How, if at all, may a closed sum exist without a shared
singleton-string data discriminant?

- **Option A — continue to reject:** keep string enums and uniquely
  discriminated object arms as the entire sum surface.
- **Option B — explicit constructor identity:** add named constructors whose
  encoded tag is language metadata rather than a payload field.
- **Option C — explicit arm labels over structural payloads:** add an authored
  sum declaration with arm names, exhaustiveness, encoding, and shape syntax.

Only option A is implemented. The `shape_sum_requires_discriminant` diagnostic
must not promise B or C.

### 4.4 Resource `stale` reachability with shaped resources

**Decision:** How does `issues/20260819-resource-stale-arm-unreachable.md`
interact with shaped resource typing before adoption?

- **Option A — implement stale publication:** make each claiming TS engine
  publish `stale` under the owning resource semantics and add shaped
  ready/stale/error conformance fixtures.
- **Option B — narrow the type surface:** stop exposing a shaped value binding
  in `stale` until the runtime state is reachable, with a migration diagnostic
  for authored bindings.
- **Option C — retain a typed but unreachable arm temporarily:** document it as
  static preparation and prohibit any reachability claim. This is today's
  behavior but leaves an ergonomics trap.

The owner must choose explicitly; fixtures cannot prove an unreachable arm.

### 4.5 Ordinary compiler result custody

**Decision W2-D5:** Does ordinary `compileContractSource` attach promoted
closed inference/layout/monomorphization results, or does it remain a
diagnostics-only consumer while downstream plan tooling calls the separately
exported `inferClosedTypes`?

- **Option A — attach once:** add a typed optional/result field to
  `CompileResult`, populate it on the default closed path, and make downstream
  consumers use that retained result.
- **Option B — diagnostics-only compilation:** keep `CompileResult` unchanged;
  ordinary compilation runs the pass for diagnostics, while callers needing
  bindings, logical layouts, row arenas, or monomorphization summaries invoke
  `inferClosedTypes(file, options)` separately.

Option B is the implementation today. Adoption must name A or B; merely lifting
the flag does not attach results.

### 4.6 The cross-engine key universe

**Decision:** what value universe and equality govern `each` keys on
every engine?

- **Option A — 0485's finite canonical scalars (proposed):** keys are
  null, boolean, well-formed string (empty string included), or finite
  number; equality is SameValueZero;
  duplicate keys reject. Matches `compiler/identity.ts`'s canonical
  identity **except its empty-string rejection, which R3's law
  resolves by admitting `""`** (the rejection is a divergence, not a
  SameValueZero canonicalization); requires the checker to stop
  accepting object-valued keys,
  TS reconciliation to stop using raw `Map<unknown>` identity,
  `memoRows` to stop rejecting `null` and admitting `NaN`, and the
  empty-string-throwing enforcement points (`compiler/identity.ts`,
  the Rust state-key serializer) to relax to the law (R3's one
  key law spans both constructs).
- **Option B — engine-local universes:** rejected by construction (the
  cross-engine claim dies).

`contract_each_identity_key` remains the runtime authority; this
decision aligns the checker and both engines with it before adoption.

### 4.7 `memo(comparator)` under A-VAL

**Decision:** how does an arbitrary captured comparator coexist with
unobservable aliasing?

- **Option A — detached canonical values:** the comparator receives
  detached snapshots (alias identity unobservable; cost paid per
  comparison).
- **Option B — typed value-language comparators:** restrict comparator
  expressions to the closed value language (no arbitrary JS function).
- **Option C — exclude aggregate custom comparators** from the promoted
  language.

Today's runtime passes retained physical objects (§2.7); any of A–C is
a change. The owner must choose before adoption; A is the least
breaking.

### 4.8 The option representation

**Decision:** what is `Option<T>` concretely?

- **Option A — pinned canonical tagged sum (recommended; R3's text):**
  a built-in language-defined sum, constructors `some(v)`/`none` as
  language metadata (any closed payload, primitives included), canonical
  object-compatible encoding `{kind:"some", value: T} | {kind:"none"}`,
  ordinary sum layout and exhaustive match. Cheapest: reuses sum
  machinery; the encoding is inspectable plain data at boundaries.
- **Option B — first-class nominal `Option`:** dedicated surface syntax
  and a nominal type distinct from every structural sum; stronger
  hygiene (no accidental structural match against user data shaped like
  the encoding), at the price of new grammar, IR, and layout surface.

R3 carries option A as the proposed normative text; choosing B rewrites
that paragraph before adoption. Either choice must land constructors,
match/narrowing, layout, and fixtures before the flag lifts — the
checker's option-shaped fix-its currently recommend constructors that do
not exist (§2.7).

## 5. Evidence and ergonomics assessment

### 5.1 Rejection-rate report

`docs/reports/llp0485-l1-rejection-report.json` is the census of the corpus
under `closedTypes: true`. It covers Caltrain, shipping examples, and
semantics fixtures; baseline failures or unknown diagnostics are
`unclassified`, never skipped. It is regenerated by
`packages/exact-contract/scripts/closed-types-rejection-report.mjs` and
kept honest by the registered check `contract-closed-types-rejection-report`
(`… --check`), which regenerates the census in memory, refuses any difference
from the committed bytes other than the `generatedOn` wall-clock stamp, and
compares the committed file's sha256 against the hash recorded immediately
below. Before that registration the recorded hash went stale silently twice
(LLP 0508 §12.1 demand 12.1-c).

Snapshot SHA-256:
`61177a8ada91060a10c7b19d84bb2ba461a2f03fdcdbee3961ab444612511681`
*(refreshed 2026-08-27 by the instrument repair recorded in the third
erratum below — the previous `a0ccebc6…` census carried 68 sites that were
artifacts of an incomplete boundary descriptor and a stringification bug.
Before that, refreshed 2026-08-26 with the registration. The previously recorded
`127966c1…` was the 2026-08-20 snapshot; it predated the M1 WP1c/WP1d
Contract-v1 work, and by 2026-08-26 the instrument itself had gone
stale in two ways — its preflight import-resolution parse used the v0
grammar, so every file importing a Contract-v1-migrated Facet component
mis-parsed and was buried as `unclassified`, and it had no notion of a
source already authored in the v1 edition. Both are fixed; the census
is again 100% classified. Earlier recorded hashes: `8498e480…`,
`127966c1…`.)*

Report `schemaVersion` is 2: each file row now carries `sourceEdition`
(`v0` | `v1`). A `v1` row is a source that cannot parse or compile
flag-off because it is **already migrated** (Option constructors,
`match` eliminators — LLP 0508 §14); that is not an instrument failure
and is no longer reported as one. The flag-off/flag-on delta this
census measures does not apply to those files, and they are counted
separately below.

| Measure | Count | Share |
| --- | ---: | ---: |
| Discovered/classified files | 71 / 71 | 100% classified |
| v0-edition files (the migration population) | 66 | 93.0% |
| Contract-v1-authored files (already migrated) | 5 | 7.0% |
| Accepted files | 32 | 45.1% |
| Rejected files | 39 | 54.9% |
| Unclassified files | 0 | 0% |
| Rejection sites | 227 | 100% |
| `closed-types/prop-annotation` | 95 | 41.9% of sites |
| `closed-types/restructure` | 70 | 30.8% of sites |
| `closed-types/shape` | 28 | 12.3% of sites |
| `closed-types/cursor` | 24 | 10.6% of sites |
| `closed-types/sum-constructor` | 10 | 4.4% of sites |
| `closed-types/nonexhaustive` | 0 | 0% of sites |

> **Erratum (recorded 2026-08-26).** Every table this section published
> before the registration above **omitted the `closed-types/cursor` row
> and folded its sites into `closed-types/restructure`** — the
> 2026-08-20 table (snapshot `127966c1…`) printed `restructure` **63**
> where the bytes it pinned read `restructure` **59** + `cursor` **4**.
> The prose was a hand-transcription of a JSON artifact the same section
> pins by sha256, and it disagreed with those bytes for six days. The
> defect propagated verbatim into **LLP 0519** §1/I-3, **Accepted
> LLP 0532** §1.1 and AX-D, and LLP 0523 r1 (corrected at r2, whose
> §2.5 carries the downstream erratum). All three, and this section,
> are corrected as of 2026-08-26. Folding
> a mechanizable class into the unmechanizable one also **understated
> the mechanizable share** of the preregistered ≥90% baseline-discharge
> gate, whose honest feasibility condition is computed from exactly
> these classes. The table above is now reconciled to the pinned bytes
> row for row, and the registered check
> `contract-closed-types-rejection-report` re-derives the census and
> re-verifies the recorded hash on every run — but it does **not** yet
> validate this prose table against the artifact, which is the residual
> hole this erratum leaves open.
>
> **Reading downstream citations.** Documents that cite "the frozen
> 0489 census" mean the **2026-08-20 snapshot `127966c1…`** (64 files,
> 39 rejected, 278 sites; `prop-annotation` 120 / `shape` 90 /
> `restructure` 59 / `sum-constructor` 5 / `cursor` 4 /
> `nonexhaustive` 0), which is the pinned baseline of LLP 0523 §2.5's
> coverage gate. That is **not** the table above: the 2026-08-26
> refresh **replaced most of that population rather than relabelling it**
> — see the third erratum below, which corrects the "same 278 sites,
> reclassified" reading this paragraph used to carry. The two censuses
> are not interchangeable, and
> the frozen bytes are no longer at
> `docs/reports/llp0485-l1-rejection-report.json` — that path now holds
> the refreshed census, and the registered check keeps it there.
>
> **Erratum 3 (recorded 2026-08-27) — "the same 278 sites, reclassified"
> was wrong, and this section said it.** The paragraph above used to state
> that the 2026-08-26 refresh "reclassified the same 278 sites across the
> same 39 rejected files". It did not. The two censuses are two **disjoint
> populations that coincidentally total 278**: `js/src/caltrain-contract/`
> `app.contract` left with **45** sites (codemodded to Contract v1, now
> accepted), `019-change-input` left with 1, `examples/widget-aot/`
> `status-widget.contract` arrived with 9 and `009-ternary-unary` with 1,
> and the 37 files rejected in **both** went 232 → 268 sites.
> −46 + 10 + 36 = 0. Nothing was transcribed wrong here — the total was
> right and **the inference from it was wrong**, which no amount of
> re-reading the documents would have caught.
>
> Matching site-to-site by **span**, on the 63 files whose source bytes did
> not change: **168 sites matched, 163 kept their class, and 5 changed**
> (all `shape` → `restructure`). 65 rejections were withdrawn and 101 raised
> at spans that had produced nothing before. **Five sites out of 278 were
> reclassified**, not 278. This became checkable only once the per-site
> denominator carrying spans was recovered
> (`docs/reports/llp0485-l1-rejection-sites.frozen-20260820.json`).
>
> **And nobody decided the change.** `closed-types/diagnostics.ts`, where
> `CLOSED_TYPE_REJECTION_CLASSES` lives, is **byte-identical** across
> `4494fbcad..8fe6e8a60`; no hunk in those nine commits swaps the class on a
> surviving condition. What moved is which code paths the checker can
> **reach** once imports stopped being opaque — and it moved in two opposite
> directions at once: import refusals **coarsened** (61 of the 90 old `shape`
> sites were one refusal per *call site* of an opaque import; that emitter
> now fires twice), while theme reads **fanned out** (field-existence
> checking became reachable against a descriptor its own author had scoped to
> "the token branches reachable from the linked Caltrain/Facet closure").
>
> **Repaired 2026-08-27, in the instrument rather than the baseline.** Of the
> 66 `reads absent field` sites in the `a0ccebc6…` census, **64 were false
> positives** — probed against the resolved `lightTheme`, the tokens exist and
> only the descriptor lacked them — and **2 were a genuine author bug**
> (`theme.spacing.s5`; the real scale runs s1–s4, s6, s8…). Three fixes:
> `packages/exact-facet-contract/src/index.contract-meta.ts` now models the
> whole theme; `reads absent field` on an **external** receiver is now
> `closed-types/shape` (its own summary — "an external value needs the §5.4
> shape boundary parser" — is exactly the fix) while locally constructed
> objects keep `restructure`; and the two `s5` reads were corrected in
> `examples/`. Separately, six sites printed `opaque declared type
> "[object Object]"` because descriptor params in the `{ type: … }` record
> form were coerced with `String(type)` — which also silently steered the
> rejection class, since `type-system.ts` chooses between `prop-annotation`
> and `restructure` by testing `raw.includes('<')`.
>
> Result, measured on the 70-file corpus as it stood that day: **278 → 210
> sites**, and the unmechanizable `restructure` share — the one feeding the
> ≥90% baseline-discharge gate's feasibility condition — falls from **46.4% to
> 30.0%**, against 21.2% on the frozen census. No baseline was moved to achieve
> that. The table above reads **227 sites across 71 files**, higher only because
> a 71st example landed from another lane between the repair and the commit
> (+17 sites, mostly `cursor`); the repaired `restructure` share there is
> **30.8%**. This corpus is a live directory scan, so its totals move with the
> tree — which is precisely why the coverage gate reads a frozen fixture and
> not this file.
>
> **Resolved 2026-08-26.** The frozen snapshot is materialized at
> **`docs/reports/llp0485-l1-rejection-report.frozen-20260820.json`**,
> recovered from commit `4494fbcad` and verified to hash
> `127966c171747e0f4adc9539592966fc9d9b282e275049d7e5cf6379f337105c` —
> so LLP 0523 §2.5's pin is satisfiable again without editing that spec
> (it is hash-bound at r3 pending the author's acceptance). The same
> registered check now holds that file immutable: it is never
> regenerated, and editing, regenerating, or deleting it reds the gate.
> It also derives this section's measure table from the census rather
> than trusting it — the residual hole named above is closed, and the
> two limbs were proved by planted defect (RFC 0496).
>
> **What the frozen artifact can and cannot underwrite.** It carries
> per-file `perRejectionClass` counts, so `contract-fixit-coverage`'s
> **class-level and file×class denominators are materialized today**.
> It carries **no per-site spans and no per-file hashes**, so the
> per-site baseline manifest LLP 0523 §2.5 requires as its first
> deliverable is *not* derivable from these bytes. Nor can it be
> recovered by re-running the current instrument: the 2026-08-26
> classifier assigns the same 278 sites to different classes, so it
> cannot reconstruct baseline site identity. Producing that manifest
> means checking out `4494fbcad`, adding span emission to the
> 2026-08-20 classifier, and re-running it. Until then the ratchet has
> no stable per-site denominator and, per §2.5's own rule, the gate
> MUST NOT be declared green. Tracked in
> `issues/20260826-frozen-closed-types-census-baseline-unmaterialized.md`.

Every site is classed with a permitted fix-it, supporting the diagnostic
claims. The five Contract-v1-authored files are the first evidence in this
instrument that the migration is *possible* on real surfaces — Caltrain's app
and four Option fixtures — but they are not the §6.1 item 5 migration
evidence, which must observe authors completing migrations rather than
observe finished sources.

The same numbers do **not** establish LLP 0485 §5.4 / LLP 0481 §9.2's
ergonomics bar. A 55.7% file rejection rate and 278 interventions across 70
files demonstrate substantial migration cost. The report is an unchanged
corpus rejection census; it does not observe authors completing migrations and
therefore cannot show whether they use `shape` or route around it with opaque
blob/capability escapes. Before default-on adoption, a representative migration
must record accepted fix-its, remaining restructures, and any escape attempts.
If authors route around `shape`, change 1 fails the bar and the paved path must
be redesigned before adoption.

### 5.2 Fixture status and the all-engine debt

Focused evidence exists today on the TypeScript toolchain only:

- compiler tests cover inference, sums, exhaustiveness, every rejection class,
  fix-it payloads, conservative deferrals, flag-off identity, logical layouts,
  key columns, relation sinks, and monomorphic signature summaries;
- TS runtime unit tests cover shape validator derivation, detached
  materialization, rejection projection, prototypes, accessors, hidden/symbol
  properties, proxy traps, non-finite numbers, and closed resource slot types;
- round-2 loader-backed Vitest coverage is 96 tests in four files, and the
  prior scoped Bun verification recorded 206 tests in fifteen files; and
- the 23 existing semantics fixtures were compiled to pin flag-off identity and
  were audited by the rejection report, but they are not yet change-1
  cross-engine goldens.

The generated and editable TS paths consume the validator, but this remains TS
evidence. No complete closed-type/shape/A-VAL fixtures run across every LLP
0330 engine today, and there is no Rust Native conformance claim. LLP 0485
§15.2's all-engine obligation is owed before adoption. Required additions
include positive and negative closed inference/descriptor fixtures, shape
ready/error settlement, hostile-value tier distinctions, sums and missing-arm
diagnostics, A-VAL copies/equality/publication/memo/replay cases, deterministic
layout descriptor comparison, and the chosen stale-arm behavior. A tier that
does not implement a feature must produce the named construct-level rejection.

## 6. Adoption and rejection mechanics

### 6.1 Preconditions to adoption

W2's owner may adopt this amendment only after:

1. resolving §4's decisions (now including §4.6 key universe, §4.7
   comparator, and §4.8 option representation), or explicitly choosing
   the currently implemented option where deferral is acceptable;
2. implementing the unenforced A-VAL clauses identified after redlines R3/R5/R8,
   including the aggregate equality diagnostic and the **three-relation
   family** (`PublicationEquivalenceV1` publication behavior; one shared
   `CanonicalValueEquivalenceV1` routine implementing **R5's case
   table** and consumed by callback comparison
   and replay; source-equality enforcement) — with replay comparison
   canonicalizing per the case table and any artifact-byte change
   landed as a versioned LLP 0341 amendment, never silently;
3. satisfying §5.2's all-engine fixture gate;
4. resolving or consciously narrowing shaped `stale` typing under §4.4;
5. producing migration evidence sufficient to decide the §5.1 ergonomics bar;
6. closing every §2.7 checker-divergence row — in particular relaxing
   the checker's `??`/optional-access deletions to the redline's
   restatements (option-coalescing `??` with its option-left typing
   rule; option-returning, flattening optional access), implementing
   the `nullable`-marker **seam lowering to `Option<T>`** (the
   seam-normalization rule; no nullable type exists to implement),
   required-prop diagnostics plus
   validated root/import
   boundaries (no `undefined` at component and mount seams), and
   callback effect-row enforcement with the derived-tolerance
   unification;
7. recording the §4.1 monomorphic narrowing (or its alternative) as an
   explicit ruling against 0485 §5.5; and
8. landing the `Option` constructors, the view-pattern option arms
   (AST/parser/binder), AND the expression/statement
   `match` eliminator (R3's normative minimum) together, and aligning
   `each`, `memoRows`, the checker, and the empty-string-throwing
   enforcement points on the one key law, before the
   flag lifts.

Adoption is the owner's semantic decision under LLP 0001; machine provenance
does not substitute for it.

### 6.2 If W2 adopts

The mechanical flag lift occurs at the compiler entry point in
`packages/exact-contract/src/compiler/index.ts`: `CompileOptions.closedTypes`
changes from optional default-off to the default language path. A migration
window may spell that `options?.closedTypes !== false`; afterward it invokes
`inferClosedTypes` unconditionally. Project and generated builds receive the
same default. There is no ambient environment switch. This flip makes closed
diagnostics default; it does not by itself retain the rest of the inference
result.

The `as shape` runtime behavior does not change during that lift. It remains an
authored construct-level opt-in, and unshaped resource IR remains unchanged.
Parser diagnostics and validator IR remain construct-selected. The report
becomes a migration baseline rather than the only corpus caller enabling the
checker.

The adoption edit records this LLP in LLP 0330, incorporates §2 at the named
sections, and records the chosen §4 options and gate-closing evidence.

### 6.3 Compiler result custody during adoption

Today `compileContractSource` calls `inferClosedTypes` only when
`closedTypes: true` and retains only `.diagnostics`. Its `CompileResult` remains
`{ file, diagnostics }`; inferred bindings, component logical layouts, `each`
row arenas, per-component monomorphization, and the plan layout-signature
summary live only on the separately exported `ClosedTypeInferenceResult`
returned by a direct `inferClosedTypes` call.

Adoption must record W2-D5 (§4.5). Under option A, the adoption patch attaches
that result once to ordinary compilation through a named typed result field.
Under option B, ordinary compilation deliberately remains diagnostics-only and
every layout/plan consumer calls the exported pass explicitly. The flag lift
alone chooses neither custody model and must not be described as attaching
layout or monomorphization metadata.

### 6.4 If W2 rejects or defers

`CompileOptions.closedTypes` remains default-off. An explicit flagged ordinary
compile continues to retain only diagnostics; direct `inferClosedTypes` calls
continue to expose logical layout metadata and monomorphic accounting for
experimentation. The rejection report and stable diagnostic classes remain
available.

The paved `as shape` alternative remains its own opt-in; shape-less programs
are inert. Its snapshot and diagnostic do not imply language-wide closed types
or A-VAL. Reversing any reviewed adjudication requires a named amendment.
