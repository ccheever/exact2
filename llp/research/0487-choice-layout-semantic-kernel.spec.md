# LLP 0487: Choice Layout — the Semantic Kernel

**Type:** Spec
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Contract, Layout, Kernel, Renderer (web host), Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-19
**Revised:** 2026-08-20 (r17 — editorial W3C-conformance conversion
under the LLP 0505 spec wave; **no semantic change**: no frozen
predicate, normative clause, owner decision, or diagnostic changed —
the r16 body §§1–13 is byte-identical. Additions only: §14 Conformance
(RFC 2119 statement, four conformance classes with obligations indexed
to the existing sections, and the formal conformance-suite designation
restating the Summary's existing tie-break rule in suite terms); §15
Terminology index (pointers to the §§2–9 definitions, no new
definitions); §16 numbered algorithms (non-normative restatements of
the classifier closure, select, Kleene evaluation, A1/A2/A5 discharge,
the two-step `fits`, `normalize(record)`, and outcome-projection
derivation/trace validation — each marked derived-from and yielding to
its normative section on any conflict).)
2026-08-20 (r16 — dedicated follow-up loop, round-2 fold
of both families' convergent MATERIALs; no frozen predicate changed, no
open owner decision closed. D4: `normalize(record)` now **strips**
unconsulted class-S axes (sparse is the one canonical form; omission is
represented by absence, never by writing `content-dependent`, a real
classifier state A1 forbids on S-atom axes); D4(b) split into a
**cross-host outcome projection** (classification, eligibility, winner,
candidate-level condition values, M5 facts, I3 disclosures — the P1
comparison object) and a **host-local evaluation trace** (per-atom
values in host order — validated independently against the AST/M4,
never byte-compared cross-host, honoring §4's short-circuit/reorder
freedom), with required diagnostics compared as set containment; the
sealed executor's stricter ordered-trace comparator is recorded in §11
as owed repair gated on the first compound-condition fixture. D4's
envelope named exactly: 0485 §3.4 planDigest+sourceVersion, §3.3.16
identity-3 runtime instance identity with its generations and
ExecutionGeneration, layout-pass/snapshot key, monotone decision
revision, 0500 HotRevision (staleness/atomicity/equality roles stated;
agent representations bind to the Acto snapshotId), plus the quotable
0488 family-3 join (adopt §3.8 identity/fencing vocabulary + budget
ledger; not the channel's scheduler/damping; exhaustion never M5
unknown — mirrored in the owed M4 cap bullet, which now also names its
unit). §5: A2 gains the normative **pin-identity license** (identity
witnesses A2's closed-scroll-chain arm; A5 needs PRESENT status),
making fixture 05's `choice_feedback_cycle` spec-determined; A5's
present-status wording defers to the profile's token vocabulary.
§6 class-M menu re-sorted: axes declared layered ((b) ⊆ (a)'s
interpretability), (a) fenced from the frozen two-step tuples and S5,
nested-choice admit/reject moved from (a) to (b) so the default cannot
be read as amending S5, (b) renamed away from "eligibility," (c) made
host-indexed with M2-illegal techniques named illegal; default
restated. §11 gains the normalize-equivalence, D4 branch-coverage,
I3 visible-focus, and conditional class-S gutter-carrier obligations.
Related/T3 0195 wording corrected to the shares-fabric/0487-owns-
evaluation-and-timing form. §13 proof-DAG adoption nudge added.)
2026-08-20 (r15 — dedicated follow-up loop, round-1 fold of
the two convergent dual-family MATERIALs plus shared minors; no frozen
predicate changed, no open owner decision closed. Gutter: A5's
invariant-overflow-ancestors form now states that only a PRESENT profile
pin (`pinned-fact`/`implemented`) discharges — `target-unimplemented`
records target law and fails closed — with "admits by construction"
qualified accordingly; the analyzer's `hasStableGutterPin` split into
identity-vs-present checks (identity keeps the A2 closed-scroll-chain
arm; A5 discharge requires present status), fixture 05 flipped to the
two-phase obligation (current law: `choice_feedback_cycle` rejection;
target-law accept recorded in its gate note and the README, the schema
having no conditional-expectation form), and the unit test gained the
flipped-pin companion accept; §13's closed gutter OQ aligned. Class M:
§6's owed-freeze bullet rewritten as three named axes — (a) measurement
environment (frozen kernel law: full vs named-narrower context), (b)
admitted subset (including codex's defer-class-M-entirely option), (c)
realization technique (unfrozen; live-web demonstration is evidence for
(c), never a substitute for (a), and must not make web the semantic
authority) — with the recorded default "narrow named environment +
matching subset + M2 hidden-live-subtree, widen on evidence." Minors:
§11's named-fixture list gains 03 (L1 specified-definite) and 09c
(baseline) plus a compound-condition D4 fixture as a schema-extension
obligation; D4(a) completed (class-M records carry both axes; named
`normalize(record)` step); D4 gains the 0485/0500 envelope sentence;
P1's equal-snapshot winner equality relabeled Decision-tier; P2's
"tier-3" renamed Band; §8 I3 option 2 completed (name on the proxy,
never the carrier; no-labeled-ancestor fallback named); §12 gains the
M4/M5/§12 cap-amendment consistency note; §13's non-finite flag priced
(unknown-on-painted-class-S breaches frozen L2) with the
fail-the-pass-as-host-defect third option; exact-verify corpus notes
refreshed.) 2026-08-20 (r14 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision; no frozen predicate changed. The Summary's 20/20 count gains its
executor disclaimer; §10 P1's local "tier-1" vocabulary renamed to the
Semantic tier; §6's owed class-M freezes reframed with the round-2
convergent recommendation (restrict class-M v1 to the demonstrated live-web
subset as the default path; full-context freeze as the alternative); §11
gains the fixture-05 stable-gutter honesty obligation (fail-closed while
the profile pin is unimplemented); §13 gains a flagged owner question on
the frozen non-finite→definite(0) clause and the typed proof-DAG sketch
for the generated-classifier question; corpus README satellite synced to
the closed D4(a) canonicalization.) 2026-08-20 (r13 — program super-refine round 1, light pass:
no frozen predicate changed. Process/wording folds: the conformance corpus
and its diagnostic codes now exist, so §1's maturity-marker and the
Summary's corpus tense are updated, and the corpus becomes a **versioned
executable authority** — a fixture edit that changes admission or
observable selection is an LLP amendment, never a silent vocabulary
refinement; §9 D4(a) gains the sparse-`avail` canonicalization sentence
the sealed corpus needed (unconsulted axes MAY be omitted and canonicalize
to content-dependent); §10 P1's cross-cite retargeted at LLP 0486 §4's
renamed **Semantic** row; §13's scrollbar-gutter OQ closed onto A5's
stable-host-independent recommendation; §6 gains two owed pre-implementation
freezes recorded as open decisions — the full abstract measurement context
for the two-step, and a deterministic work-admission cap for M4 — plus the
codex-proposed restrict-class-M-v1 alternative; §8 I3's fallback target
must satisfy the host accessibility audit (the bare unnamed carrier trips
`focusable-no-label`), with the labeled-container vs named-focus-proxy
choice recorded as an owner decision; §12's `fitsPreferred` lean
strengthened.) 2026-08-20 (r12, owner ruling on carrier-lowering freedom:
§2 now states explicitly that **any lowering exhibiting the carrier
profile conforms**, with the conformance corpus as the enforcement
mechanism and per-host declaration of the lowering the evidence was
produced under. Prompted by two independent implementations reaching
opposite conclusions about whether family 9 was satisfiable at all —
one refused to activate its four gates after correctly applying CSS
Flexbox §8.5 to a flex carrier, on a premise §2 gave it no help with.
A non-normative note now names that trap and both known-good
discharges. The ruling's accepted cost is recorded in §2: a host can
conform today and regress tomorrow if the corpus does not cover its
lowering. No frozen predicate changed; the profile's observable
clauses are untouched.) 2026-08-20 (r11, owner rulings closing the two gaps the
conformance fixture corpus surfaced — neither fell inside §1's
maturity marker, so both waited for Charlie: §11 gains
`choice_slot_out_of_flow` as a distinct normative diagnostic for §6's
in-flow slot-ancestry rule, which was normative but unnamed and which
the corpus had been recording as `unassigned-in-spec`; and §2's
carrier profile gains **baseline transparency** — the carrier
forwards its content's first baseline unchanged, empty carriers
synthesize none — closing a gap that had made carrier baseline
behavior unassertable by any conforming fixture. Both rulings are
additive; no frozen predicate changed.) 2026-08-19 (r10, post-loop, applying the round-9
materials on merits — the second extension is exhausted and NO
reviewer has seen r10: the classifier is frozen as a unique
**closure result** in saturating stages (CD-closure absorbing →
definite closure → unresolved closure → residue CD; any algorithm
producing it conforms), ending the axis-processing-order ambiguity
both families demonstrated with the same aspect-ratio example; the
endogenous rule generalized to the **endogenous-measurement
admission rule** — transitive over every selection input (content,
style, relations, typography, intrinsics) with closed
positive/negative forms, fail-closed, folded into M6's certificate,
renamed `choice_endogenous_measurement`; the slot carrier's layout/
semantic profile frozen (styling-free, metric- and
semantics-transparent, programmatically focusable, 0×0 when empty);
fixture obligations extended with the round-9 counterexamples.
2026-08-19 (r9 after super-refine round 8 (second
extension), 2/2 NOT READY: the classifier is now exclusive, sticky,
dependency-first (content-dependent dominates — including
content-valued clamps on definite lengths — and containing-block
definiteness is distinguished from the offer, so
fraction-of-definite-CB stays definite under intrinsic offers,
producing L1's case by a rule); A5's subject set is conditional on
`fits`, its positive forms certify the whole used-content-box
expression (percentage insets disqualify the fixed-length shortcut),
and invariant overflow ancestors (reserved/stable gutters, pinned
overlay) are a positive form with the v1 scroll composition pinned so
`scroll(column(choice))` admits by construction; the warning-only
endogenous cut became a class-M admission error
(`choice_endogenous_content`) restoring S2/M1 coherence, with nested
choice clarified as arrangement rather than endogenous content; the
CSS rung's comparison domain became a rung-relative embedding of one
boolean predicate certified structurally (the unsatisfiable
quantization-proof clause deleted, D1 kept non-empty); class-M
templates require in-flow slot ancestry; the loop's six
counterexamples are frozen as fixture obligations. 2026-08-19 (r8 after super-refine round 7 (second
extension), 2/2 NOT READY: L1 rewritten as the classifier's actual
theorem (both families' identical counterexample — a specified-definite
inner axis stays definite under intrinsic offers); the classifier
became a staged two-axis fixpoint so profile transfers cannot strand
an axis; A5 gained a static subject set and A1-shaped
positive/negative discharge forms with the overlay-vs-classic
scrollbar ruling pinned as a profile obligation; §7 T1 gained the
endogenous cut (commit-driven content responses are not re-selection
inputs in v1, `choice_endogenous_content`) closing the class-M
content-feedback orbit codex constructed; the `stack` flow rule
pinned in §6 and absolute positioning of slot references forbidden;
M5 narrowed to missing-backend/non-finite; step-2's vacuous `min`
stated; I4(a) restated over slot-box order; D4(b) typo and I3
disclosure fixed; L2 gained the disjoint-ranges sentence. 2026-08-19
(r7, post-loop, applying the round-6 materials
on merits — the extended budget is exhausted and NO reviewer has seen
r7: the T4 look-ahead is **deleted** and replaced by §5 A5, a v1
admission requirement that every instance carry a certified
feedback-unreachability witness (`choice_feedback_cycle`), making the
stationary cycle unreachable for admitted instances and pure `select`
the committed selection again (both families' round-6 exit: codex
"make cycle-unreachability universal… remove counterfactual
quiescence" ≡ grok "one A-rule, two discharges"); A1's certified
positive forms narrowed to allocation independence (wrap-line
cross-size and auto-track stretch are named negative forms, per grok's
two-fixed-point counterexample); A2 given A1's declare/discharge
shape; L2 scoped to committed painted evaluation; runtime feedback
handling recorded in §12 as an out-of-v1 extension. 2026-08-19 (r6 after super-refine round 5 (extension), 2/2
NOT READY: classifier rules 2–4 restated over the **offer to C**
(Taffy `AvailableSpace` vocabulary — an intrinsic child measure inside
a definite-parent layout pass is unresolved); T4's multi-frame
quiescence replaced by a same-pass one-step look-ahead inside the
committed selection procedure `choose*`, quiescing to `kₙ` (never D2),
trace in D4(a) so P1/P3 hold, no history/window/epoch, and
cycle-unreachability added as a declarative-eligibility principle;
`definite(v)` is the pre-overflow content-box (dynamic auto gutters
are descendant-dependence and fail A1; stable gutters subtract
statically; runtime resolution failure on an A1-positive axis yields
definite(0)); D4(a) measurements became per-step tagged transcripts
(`not_required`/`success`/`failure`) so early-false never demands an
`ideal.block` and M5 unknowns replay; I3 narrowed with a deterministic
fallback-focus rule resolving the I2 conflict; template chrome frozen
semantics-transparent and non-focusable. 2026-08-19 (r5 after super-refine round 4 (extension), 2/2
NOT READY: the `avail` classifier became a winner-independent numbered
algorithm over (C's sizing constraints on the axis, parent constraint) —
the selected candidate is explicitly not an input, candidate templates
count as static paths conservatively, and `definite(v)` is the resolved
content-box after min/max/padding/border; a required-measurement rule
aligns M4/M5 with Kleene (non-required measurements are never taken and
never rewrite determined atoms; the two-step is early-false); one
comparison domain frozen for every rung (the r4 CSS-snapping exemption
removed; declarative eligibility must preserve the M3 decision); A1
made compositional (declared assumptions, link-time discharge); T4
cycle quiescence added; I4 restated over accessible descendants; D4
split into input/outcome/provenance records; L2's rationale corrected
(CSS CQ is itself Kleene). 2026-08-19 (r4, post-loop, applying the round-3 materials
on merits — the loop's 3-round budget is exhausted and NO reviewer has
seen r4 (escalation record in the loop journal): `avail` states frozen
as an exhaustive, disjoint-by-precedence table with corrected examples
and two lemmas (A1-positive axes are definite at commit; observable
class-S evaluation never sees unknown — the license for the Boolean
`@container` encoding); A3 narrowed to same-pass acyclicity; the
class-M measure-safety boundary frozen fail-closed
(`choice_slot_not_measurable`); D4 split into a replayable
decision-input record and an outcome record with P1/P3 over the input
record; the nested two-step example added to the honesty rule.
2026-08-19 (r3 after super-refine round 2, 2/2 NOT READY:
conditions moved to strong-Kleene three-valued semantics (unknown from
unresolved axes or failed measurement; a candidate is selected only on
true — replacing r2's asymmetric fail-closed rule whose `not` branch
selected the wide candidate on unknown size); `avail` given three states
(definite / unresolved / content-dependent) with each rule told which it
consumes, raw unpinned snapshots; S5's measure-time env fixed to the
constraints the step actually assigns; §6 steps frozen as exact
constraint tuples; slots materialize as exactly one stable box and
template roots must be containers; T1 widened to every preferred-size
input via measurement generations; I4's universal visual-order claim
removed (topology interleaves; counterexample recorded) and a
participation clause added; D4 made replay-complete; P2 moved to
uncertainty intervals with the T1 scope clarified (and mirrored into
LLP 0486 §4); vacuous `fits` statically rejected. r2/r1 change logs:
the loop journal under `llp/reviews/`. r1 applied Draft→Review at loop
start — the loop's one authorized transition.)
**Related:** RFC 0084 (owns choice layout's graduation into the public
vocabulary and keeps surface syntax deliberately unfrozen — this spec
freezes only the semantics underneath), LLP 0486 (One Layout Language,
Two Engines — the doctrine this kernel instantiates: §8 realization
ladder, §4 parity tiers), LLP 0195 (size classes and the container-size
feedback channel; the island shares the governed budget-ledger and
correlation fabric — §9 D4's 0488 family-3 join — while `fits`
evaluation and §7 T3 timing remain 0487-owned; the channel's
oscillation pinning is consumer-local and never applies to `select`),
LLP 0199 (layout relations — a sibling stage-1 consumer, not a
dependency), LLP 0349 (registered evaluators — the island adopts its
budget/diagnostics contract), LLP 0202 (web host conformance honesty),
RFC 0088 (collections/virtualization — composition deferred), LLP 0313
(motion layout islands — transition behavior deferred)

## Summary

This document freezes the **semantic kernel** of choice layout: the
part of the operator's meaning that no syntax experiment, cost-model
measurement, or bake-off result should be allowed to change. It defines
candidates, logical children and their boxes, the selection function,
the three-valued condition semantics and the two condition classes,
`avail`'s three states, consulted axes and the anti-cycle rule, the
measured-fit procedure and its failure rule, re-selection timing,
identity/state/focus/accessibility guarantees, static (no-JS)
realization, the decision snapshot, and determinism and cross-host
parity obligations, plus the normative diagnostic minimum.

Everything else — surface syntax, IR encoding, the kernel evaluation
algorithm and its caching, the realization-eligibility predicate's
concrete form and the web lowering (union trees, `display:contents`,
island mechanics, measure-safe slot restrictions), budget numbers,
transition/motion behavior on selection change, virtualization
composition — is **explicitly unfrozen** (§12) and will be specified
later, from the measured prototype, per LLP 0486 §8.3's graduation
gates.

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) are used in the
RFC 2119 sense. The companion conformance fixture corpus
(`tests/layout/choice/` — landed, registered, 20/20 executing across its
two executors as of 2026-08-20; read that count honestly: 2 fixtures run the
production `Kernel`, 18 run the isolated choice prototype, and no
analyzer-to-Taffy end-to-end path exists yet — prototype replay/carrier
evidence, not end-to-end conformance)
is the tie-breaking authority where prose and fixtures disagree, per
LLP 0486 §5.1's rule; a disagreement is itself a defect in this
document and MUST be resolved by revising one of the two. The corpus is a
**versioned executable authority**: an edit that changes what is admitted,
which diagnostics fire, an axis's classified state, or the selected
candidate is a semantics change and requires an amendment to this
document — never a silent fixture-side refinement. Fixture-driven edits
to the certified-form *vocabularies* (§1's maturity marker) remain
non-semantic exactly because they cannot change those observables.

## 1. Scope

**Frozen by this spec:** the semantics observable by an author, a
verification fixture, or an agent: which candidate is selected, when
selection changes, what is preserved across changes, what a static
rendering shows, and what every conforming host must agree on.

**Not frozen (see §12):** everything about *how* hosts realize these
semantics, and every authoring-surface decision RFC 0084 reserves.

**Maturity marker (author-ratified at loop close, 2026-08-19).** The
kernel's *predicates and their fail-closed shapes* are frozen. The
**certified-form vocabularies** that discharge them — A1/A5's
positive/negative form lists, §7 T1's endogenous-measurement
positive/negative sets, and the §2 classifier's stage seeds — are
**proposal-stage seeds pending the conformance fixture corpus**
(`tests/layout/choice/`, whose required counterexamples §11
enumerates). Per this document's own tie-breaking rule, the fixtures
refine those vocabularies as they land; a vocabulary edit driven by a
fixture is not a semantic change to the kernel. These vocabularies
are fixture-governed: the corpus now exists and refines them as fixtures
land. (This is the
LLP 0485 seam pattern: nine dual-family review rounds converged on
every architectural element and kept regenerating findings only
inside these enumerations — the fixture corpus, not further prose
review, is the authority that closes them.)

## 2. Definitions

- **Choice container (C):** a view node declaring an ordered list of
  **candidates** `K = [k₁ … kₙ]` and an ordered list of **logical
  children** (slots) `L = [c₁ … c_m]`, `m ≥ 0`. Candidate order is
  declaration order; declaration order **is** preference order
  (`k₁` most preferred). There are no numeric preference weights in
  the kernel. An unconditional candidate before `kₙ` shadows every
  later candidate; this is legal (tooling MAY warn).
- **Slot:** one logical child position in `L`. A slot materializes as
  **exactly one stable box** — the identity carrier (§8 I1) and the
  unit templates arrange — with a **frozen carrier profile**, since
  the carrier participates in `ideal`, order, and focus: no authored
  styling surface (no padding, border, or decoration);
  metric-transparent (its intrinsic contributions equal its
  content's; an empty slot's carrier is 0×0 under the profile);
  **baseline-transparent** — the carrier's first baseline **is** its
  content's first baseline, forwarded unchanged, and an empty
  carrier synthesizes no baseline (owner ruling, 2026-08-20, closing
  the gap the conformance corpus surfaced: the earlier profile froze
  metric transparency without ever defining baseline derivation, so
  no conforming fixture could assert carrier baseline behavior at
  all; transparency here is the reading consistent with every other
  clause of this profile — the carrier is invisible to baseline
  alignment exactly as it is to intrinsic contribution);
  semantics-transparent (no role or name); programmatically
  focusable but absent from sequential focus order (it is §8 I3's
  fallback target).
  **Any lowering exhibiting this profile conforms** (owner ruling,
  2026-08-20). The profile is the carrier's *observable* contract;
  §1 already places host realization outside this document, and two
  conforming implementations may therefore materialize the carrier
  differently — a kernel host may pass its content's layout output
  through unchanged while a web host uses a styled box — without
  either being wrong. The corpus is the enforcement mechanism: each
  host declares the carrier lowering its evidence was produced under,
  and §11's conformance fixtures assert this profile against that
  declaration, so a new lowering demonstrates the profile for itself
  rather than inheriting another host's proof. The accepted cost of
  this ruling is that a host can conform today and regress tomorrow
  if the corpus does not cover its lowering.
  *Non-normative.* Baseline transparency constrains the lowering, not
  only its observable result, and the constraint is easy to miss. A
  flex-based carrier satisfies it only when its content participates
  in baseline alignment — CSS Flexbox §8.5 generates a flex
  container's first baseline from its startmost baseline-aligned
  item, and *otherwise synthesizes one from the border box* — and an
  empty flex-based carrier must additionally opt out of its parent's
  baseline alignment, or it manufactures the baseline this profile
  requires it not to have. A carrier that passes its content's
  layout output through unchanged satisfies the clause by
  construction. These are illustrations, not requirements.
  A slot's *content*, below that box, is an
  ordinary Contract subtree and MAY be internally dynamic (`when`
  branches, an `each` collection region, a nested choice container);
  dynamic content never changes the slot's box count. Slot membership
  and order are static: candidates can never add, remove, or reorder
  slots.
- **Arrangement template:** each candidate `kᵢ` is a tree whose root
  MUST be a **container node** and whose container nodes are drawn
  from `column`, `row`, `grid`, `stack`; **terminals** are either
  **slot references** or `spacer`. Every slot in `L` is referenced
  **exactly once** per template, and the in-order (depth-first,
  left-to-right) sequence of *slot* terminals in every template MUST
  equal the order of `L` (`spacer` terminals are unconstrained in
  count and position). A zero-slot template (`m = 0`) is legal; the
  order rule is vacuous. Templates MUST NOT contain content leaves
  (`text`, `image`, …), interactive nodes, `scroll`, or nested choice
  containers — content and behavior live in slots; templates only
  arrange.
- **Certified template attrs:** template nodes carry attrs from a
  certified subset (enumerated by the implementation spec, unfrozen)
  constrained by frozen principles: (a) no attr may, under any
  candidate, remove a slot's content from rendering or suppress its
  **accessible descendants** (`display:none`-like, `hidden`,
  visibility-hidden or aria-hidden wrappers — every slot's accessible
  descendants participate under every candidate; §8 I4 states the
  empty-slot boundary); (b) explicit reorder features (flex `order`,
  reversed-direction values, order-permuting grid placement) are
  excluded as hygiene; grid auto-placement follows `L` order (a
  useful frozen lemma: the slot-terminal order rule makes every
  template a laminar interval hierarchy over `L`); and
  (c) template containers and `spacer` terminals are
  **semantics-transparent and non-focusable** — no certified attr may
  give arrangement chrome a role, name, or interaction behavior. See
  §8 I4 for what is and is not guaranteed.
- **Fit condition:** a predicate attached to a candidate (§4). The
  final candidate `kₙ` MUST be unconditional (always valid), making
  selection total by construction.
- **Available size (`avail`):** per axis, one of three states —
  **definite(v)**, **unresolved**, or **content-dependent** —
  describing **C's content-box inner size at the current layout
  evaluation**. The state is computed by this **classifier
  algorithm**, whose only inputs are (i) C's sizing constraints on
  the axis (including min/max, aspect-ratio, and track/basis modes)
  and (ii) the parent constraint at this evaluation. **The selected
  candidate is never an input**: wherever a dependency from "the
  arrangement" could exist, the classifier judges the conservative
  union of **all** of the instance's candidate templates as a static
  path. Two distinctions are load-bearing: profile transfers
  (aspect-ratio, min/max referencing the other axis) couple the
  axes; and **the containing block and the offer are different
  inputs** — a containing size can be definite while the offer to C
  is an intrinsic measure. What is frozen is the **unique closure
  result** over both axes together (any worklist, caching, or
  ordering that produces it is conforming and unfrozen — no per-axis
  scan order can change the answer). The closure runs in stages;
  each stage saturates before the next, and residue is judged only
  at the end:

  1. **CD-closure (absorbing).** Seed **content-dependent** wherever
     the axis's used content-box **expression** can depend on C's
     descendants under **any** candidate template — through the
     sizing mode itself (shrink-to-fit `auto`,
     min-/max-/fit-content, content-derived flex bases,
     `min-size:auto` floors, intrinsic tracks), through a
     content-valued clamp on an otherwise-definite length
     (`width=300` with `max-width: max-content` is
     content-dependent), or as stretch/fill/fraction whose offer is
     indefinite at commit in the CSS/Taffy
     treat-as-content-sized-`auto` sense. Close over transfers: a
     transfer from a content-dependent axis is content-dependent.
     Nothing later promotes out of this stage.
  2. **Definite closure.** On the remaining axes, seed
     **definite(v)** where a numeric pre-overflow content-box is
     derivable **without descendant input, from this evaluation's
     containing sizes**: a specified definite length whose applied
     clamps and insets are all themselves descendant-independent; a
     percentage/fraction of an **already-definite independent
     containing size** — definite even when the offer is an
     intrinsic measure (the offer does not demote a size the CB
     determines); stretch/fill whose offer is already `Definite`.
     Close over transfers from definite axes until saturated.
  3. **Unresolved closure.** On what remains, seed **unresolved**
     for stretch/fill/fraction whose determining containing size or
     offer is not yet available this evaluation (an intrinsic offer
     against a not-yet-determined CB, stretch not yet allocated);
     close over transfers from unresolved axes. Where an axis could
     seed both stages, definite (stage 2) is preferred.
  4. **Residue.** Axes in no closure — including mutually dependent
     transfer components with no seed — are **content-dependent**
     (fail-closed).

  For every definite axis, `v` is the **pre-overflow resolved
  content-box length**: min/max clamped, with border, padding, and
  any *statically reserved* stable scrollbar gutter subtracted from
  the border-box length (`width=300` with padding does **not** give
  `avail.inline = 300`). Dynamic, content-driven auto gutters are
  never subtracted — a consulted axis subject to one is
  descendant-dependent and fails §5 A1 — and the Layout Profile
  pins stable-gutter/off-axis overflow for choice containers so
  `v` agrees with the size-containment (as-if-empty) box. If
  resolution of an A1-positive axis fails at runtime (non-finite
  or negative), `v` is **0** — never content-dependent, since §4
  has no runtime value for S atoms on such an axis. C generates a
  principal box (`display:contents`-like modes are excluded on C
  and on template roots); the selected template root is C's
  in-flow child, so C's own border/padding are outside `ideal`.

  Two lemmas follow and are frozen as obligations: **(L1, committed
  definiteness)** every §5 A1-positive axis is **never
  content-dependent**; it is **definite** in the committed layout of
  its pass — and also under intrinsic offers when it is a specified
  definite length with descendant-independent clamps and insets, a
  fraction of an already-definite independent containing size, a
  `Definite`-offer stretch, or a transfer from an already-definite
  axis (exactly the classifier's rule-2 cases); it is **unresolved only** when it is
  stretch/fill/fraction under an intrinsic or not-yet-allocated
  offer (unresolved states are unpainted per §7 T2 but are defined
  S5 inputs — a definite axis is never rewritten to unresolved, per
  S5); **(L2, observable class-S determinacy)** committed,
  *painted* class-S evaluation never sees unknown — L2 says nothing
  about S5 measure-time inner evaluation, which may see unresolved
  axes by design. (CSS container-query evaluation is
  itself three-valued Kleene, so unknown would not corrupt an
  `and not` chain; L2's real work is keeping unknown off committed
  class-S paths entirely, which licenses complement-range encodings
  such as `max-width:` duals that are genuinely Boolean. L2 is not a
  license to invert preference: first-true MUST still compile to
  disjoint ranges or explicit `not`-prefixes — a default-to-most-
  preferred encoding is the fail-open bug in CSS clothing.)

  The constraints C receives from its parent (known dimensions,
  available space, measure modes) are *inputs to this table*;
  condition atoms never compare against the raw parent offer. S-atom
  comparands are content-box lengths of C (`fits` compares the boxes
  §6 names). `select` consumes the **raw, unpinned size snapshot** of
  the current evaluation; LLP 0195's oscillation pinning is
  consumer-local and MUST NOT be applied to `select`'s inputs. Two
  conforming hosts MUST NOT disagree on an axis's *state* for the
  same layout circumstance — the table makes state decidable, and a
  divergence is an engine/profile defect, never a §10 P2 tolerance
  matter.

## 3. The selection function

```
select(K, env) = the first kᵢ (lowest index) whose condition
                 VALUES to true under env          (three-valued; §4)
```

where `env` consists of: `avail` (per-axis states and values), the
resolved token environment (theme snapshot), and — for class-M
conditions only — the candidate measurements defined in §6.

Normative properties:

- **S1 (totality).** Because `kₙ` is unconditional, `select` always
  yields a candidate — under unresolved axes and measurement failure
  alike (§4, §6 M5).
- **S2 (purity/determinism).** `select` is a pure function of `env`.
  Equal `env` MUST yield equal selection on every host, in every pass,
  with no history dependence. There is **no hysteresis** in the
  kernel; any future hysteresis is a semantic extension to this spec,
  not a host option.
- **S3 (single ordered scan).** Selection is one pass over `K` in
  order; evaluating or measuring a candidate MUST NOT alter the `env`
  used for subsequent candidates in the same scan.
- **S4 (locality).** Conditions consult only `env` as defined here.
  They MUST NOT read other nodes' geometry, app state, or platform
  properties. (`fits` incorporates the layout of the candidate being
  measured, descendants included; that is part of `env`, not a
  cross-node read. Cross-node relationships are LLP 0199's domain;
  state-driven branching is `when`'s.)
- **S5 (nesting).** When choice containers nest (via slots): an inner
  container's selection is **never** an input to an outer **class-S**
  evaluation. An outer **class-M** measurement necessarily includes
  inner arrangements: during a §6 measurement step, a nested choice
  evaluates `select` under the `avail` **that step actually assigns
  to it** — each axis in whichever of §2's three states the step's
  constraints put it in. A host MUST NOT rewrite an inner axis that
  is definite under the step's constraints into unresolved. After the
  outer winner commits, inner containers re-select under the
  committed constraints (§7 T2); measurement-time inner selections
  are never observable, and the honesty rule (§6) applies exactly
  when the measurement-time and committed envs differ.

## 4. Conditions: three-valued semantics

A condition is an expression over **atoms** combined with `and`,
`or`, `not`. Conditions are evaluated in **strong Kleene
three-valued logic**: every atom takes a value in
{**true**, **false**, **unknown**}; `not` maps unknown to unknown;
`and`/`or` are the Kleene extensions (`true or unknown = true`,
`false and unknown = false`, `unknown` otherwise propagates). The
semantics is denotational — the value of a condition is defined by
this algebra, independent of evaluation order — so hosts MAY
short-circuit or reorder evaluation freely provided the value is
preserved and §6 M2/M4 are respected. **A candidate is selected only
when its condition values to true**; unknown never selects (uniform
fail-closed at the candidate, symmetric under `not`).

**Class S (size-threshold) atoms:** comparisons (`<`, `<=`, `>`,
`>=`) between one of C's per-axis `avail` values and an **absolute
length** that is static per theme snapshot (a literal or a
token-resolved value; never a fraction of C or of `avail`). Value:
on a definite axis, the numeric comparison (true/false); on an
unresolved axis, **unknown**; a content-dependent axis is illegal for
S atoms (§5 A1, compile-time). Equality atoms are excluded from v1.

**Class M (measured-fit) atoms:** `fits` — "the **current** candidate
fits," valued per §6 (true/false, or **unknown** on measurement
failure per M5). `fits` is implicitly bound to the candidate it
annotates; naming another candidate is a compile error
(`choice_fits_arity`). `not fits` is well-defined (and is unknown
when `fits` is unknown).

**Classification vs. realization eligibility.** A candidate is
class M if its condition contains a class-M atom; an **instance** is
class M if any conditional candidate is, else class S. Classification
is *semantic* and frozen here. **Realization eligibility** — whether
a class-S instance may be realized declaratively with zero script
(§9 D1) — is a *separate, host-facing predicate* over the instance's
templates and sizing (its concrete form is unfrozen, §12; §5 A4 is
one necessary condition). A class-S instance that fails eligibility
is realized by evaluator (island) and is **fully conforming**. The
compiler MUST statically record both the class and the eligibility
verdict in build output.

## 5. Consulted axes and the anti-cycle rule

An axis of C is **consulted** by an instance iff (a) some S atom
names it, or (b) the condition contains `fits` and `avail` is
definite on that axis at evaluation time.

- **A1 (S-atom axes; static, fail-closed, compositional).** On every
  axis named by an S atom, C's used size MUST NOT depend on C's
  descendants — judged in **every statically possible** parent
  formatting context of the instance. Certified positive forms are
  about **allocation independence**: a definite length; a fraction of
  a containing size itself independent of C's descendants;
  stretch/fill **only where the parent's used size on that axis is
  independent of C's descendants** (single-line flex stretch into an
  axis-definite container; stretch into a definite grid track). Named
  negative forms — allocations that consume C's hypothetical content
  size do NOT qualify even when the parent's outer size is definite:
  **flex-wrap line cross-size** (line cross-size is the max of items'
  content-derived hypothetical sizes, giving a stretch item's
  consulted axis two fixed points; the conformance corpus MUST carry
  this counterexample as a fixture), **stretch into
  `auto`/min-/max-/fit-content grid tracks**, shrink-to-fit `auto`,
  min-/max-/fit-content-like sizing, content-derived flex bases, and
  `min-size: auto`-driven floors. **Proof locus:** a component compiled in isolation
  does not fail A1; it **declares** its symbolic requirement (e.g.,
  "requires descendant-independent definite inline size"), and
  closed-application/route linking MUST discharge that requirement at
  every instantiation site, emitting the diagnostic at the site where
  it cannot. Libraries stay compilable; fail-closed safety moves to
  the link step. This is a **proof obligation**: where sizing is opaque or
  dynamic and independence cannot be statically proven, the analyzer
  MUST fail closed and reject (`choice_condition_axis_sizing`).
  (A1 makes an S-atom axis never *content-dependent*; it can still be
  *unresolved* during measure passes — then the atom is unknown, §4.)
  A1 failure is a hard error, deliberately unlike A4's
  island-degrade: an S atom on a content-dependent axis reads a value
  its own choice would cause, which is meaningless in **every**
  realization, not merely unrealizable on the CSS rung.
- **A2 (`fits` axes; dynamic, permissive — with one static floor).**
  `fits` imposes no per-axis sizing obligation: a content-dependent
  or unresolved axis is simply not compared (§6), so the motivating
  case — `width` fill, `height` auto, "does this row of chips
  fit?" — is legal with only the inline axis consulted. However, a
  `fits` instance none of whose axes can be statically proven
  descendant-independent would be vacuously true at commit
  (author-hostile, never meaningful); it is rejected
  (`choice_fits_unconstrained`) — **compositionally, in A1's shape**:
  an isolated component declares "requires at least one
  descendant-independent axis," link-time discharge checks every
  instantiation site, and the diagnostic fires at a site where no
  axis discharges. Isolation compile never fails A2 for a declared
  requirement. **Pin-identity license (normative, r16):** for A2's
  static proof, a *referenced* Layout Profile pin may witness
  descendant-independence **by identity** — the pin's version,
  reference, and ruling establish that the composition is meaningful
  under the profile's target law (the analyzer's closed-scroll-chain
  arm), regardless of the pin's status; A5 discharge, by contrast,
  requires the pin's **present** status (its invariant-overflow form).
  The split is what makes the two-phase stable-gutter obligation
  spec-determined: closed allocation evidence plus a
  `target-unimplemented` gutter pin passes A2 (the instance is not
  `choice_fits_unconstrained`) and fails A5
  (`choice_feedback_cycle`) until the pin is a present fact.
- **A3 (same-pass acyclicity, stated narrowly).** Within one layout
  evaluation, by A1/A2 and the §2 table, every `avail` value `select`
  consults is determined without descendant input — `select` never
  reads a size its own choice caused **in that pass**. (`fits`
  depends on candidate content by design; the theorem is about C's
  consulted sizes.) A3 does **not** claim cross-pass environmental
  acyclicity: selection can change an ancestor (a scrollbar appears),
  changing C's consulted `avail` in a *later* pass — that is the
  documented no-hysteresis flicker (§12), out of A3's scope. A
  consulted axis is a **contained axis** in the Contract Layout
  Profile sense; CSS `container-type` supplies one certified witness
  of that property on the web.
- **A4 (CSS containment-axis prerequisite).** CSS provides
  `container-type: inline-size` (contains the inline axis only) and
  `container-type: size` (contains **both** axes); there is no
  block-only value. Consequently: an instance whose S atoms consult
  only the inline axis passes the containment-axis prerequisite with
  inline-size containment; an instance with any block-axis S atom
  passes only if C's inline axis is also descendant-independent (it
  will be size-contained). Passing this prerequisite is **necessary,
  not sufficient**, for declarative realization (§4's eligibility
  predicate, unfrozen); failing it makes the instance island-realized
  (still conforming, per §4). Two further frozen principles of that
  predicate: **declarative eligibility is certified by structural
  equivalence, not numeric-domain identity** — the CSS encoding must
  query the same box `avail` names (content-box, static gutter
  reduction per §2), compile first-true to disjoint ranges or
  explicit `not`-prefixes (L2's rule, preference never inverted), and
  rely on containment that does not change used size (A1); an
  encoding that cannot be proven boolean-equivalent to `select` on
  that box is island-realized. The browser's used-value rounding
  relative to M3's unrounded domain is a §10 P2 matter, never an
  eligibility test — a static "provably agrees at every runtime
  size" predicate would be unsatisfiable and would empty D1; and on the
  web rung, consulted axes carry CSS containment's layout/style side
  effects, whose observability the Contract Layout Profile MUST
  record (expected to be a no-op under its pinned defaults —
  recorded, not assumed).
- **A5 (v1 admission: feedback-unreachability).** Every v1 instance —
  class S and class M, every realization — MUST carry a certified
  witness over a **static subject set**: every S-atom-named axis,
  **plus, only when the condition contains `fits`,** every axis the
  §2 classifier can report definite in any statically possible parent
  context of the instance (the over-approximation of `fits`'
  evaluation-time consulted set; a pure class-S instance is never
  rejected over an axis its atoms cannot consult). The witness
  certifies that no subject axis can be affected by C's own layout
  contribution through **any** path — directly (A1/A2) or through
  ancestors — and it certifies the axis's **entire used-content-box
  expression**: a specified definite length discharges only when
  every applied clamp, inset, transfer, and gutter term is itself
  ancestor-immune (a percentage inset or content-valued clamp inside
  the expression disqualifies the shortcut — `width:300px` with
  percentage padding is NOT immune, since an ancestor size change
  reaches `avail` through the padding). Certified discharge forms, in
  A1's declare/discharge shape:
  - **Positive (axis-local):** a specified definite length whose
    content-box expression terms are all literal or otherwise proven
    ancestor-immune.
  - **Positive (allocation-chain):** stretch/fill/fraction discharges
    only if every ancestor allocation path to that offer's containing
    block is itself A1-positive on the axis **and** contains no
    wrap-flex line, no `auto`/min-/max-/fit-content track, and no
    overflow ancestor with a **dynamic** gutter on the axis.
  - **Positive (invariant overflow ancestors):** an overflow ancestor
    whose gutter cannot vary with content — overlay scrollbars where
    the profile pins them, or a **reserved/stable gutter**
    (`scrollbar-gutter: stable`-like), whose reduction enters `v`
    statically per §2 — is ancestor-immune and discharges. **Only a
    PRESENT pin discharges this form:** a Layout Profile pin whose
    status is a present fact witnesses the reservation — the
    profile's own status vocabulary is authoritative (`pinned-fact`
    is its present token; an analyzer MAY accept a present-status
    superset such as `implemented` but MUST NOT treat any
    target-state token as present); a `target-unimplemented` (or
    `v1-ruling-target-unimplemented`) pin records
    target law and does NOT discharge — until the pin flips, the
    composition fails closed through the feedback-cycle gate
    (`choice_feedback_cycle`). **The v1 scroll composition is pinned
    here:** the Layout Profile MUST pin reserved/stable gutters (or
    overlay, where pinned per host) on scroll ancestors of choice
    containers, so the default `scroll(column(… choice …))` page
    admits by construction once the profile pin is a present fact;
    recommendation: stable gutter, host-independent.
  - **Negative:** wrap-line cross-sizing, auto tracks, any allocation
    consuming C's hypothetical size, **dynamic** gutter consumption
    (classic `overflow:auto` without a reserved gutter on the
    consulted axis), percentage insets or content clamps inside a
    subject axis's expression.
  Opaque or dynamic ancestry fails closed. Instances without the
  witness are rejected at compile/link time
  (`choice_feedback_cycle`), at the instantiation site, like A1.
  Consequence: for every admitted instance, the stationary
  `avail`-feedback cycle (candidate → ancestor scrollbar → different
  `avail` → different candidate → …) is **unreachable**; together
  with §7 T1's endogenous-measurement admission rule (which closes
  the other feedback edge — candidate → committed-geometry-responsive
  slot measurement → different `fits`), pure `select` is the committed
  selection and no runtime cycle handling exists in v1.
  Runtime-feedback-capable instances (a finite transition system over
  candidate orbits with deterministic cycle fallback) are an
  explicitly out-of-v1 semantic extension (§12) that must first
  freeze the transition boundary this rule lets v1 omit.

## 6. Measured fit: `fits`

`fits` values true iff no compared axis fails. The normative measured
quantity is the **two-step ideal tuple** below — the template root's
**border-box** under each step's constraints, compared against C's
content-box `avail` (there is no single "the candidate's preferred
size"; S5 can legally give the two steps different inner
arrangements). The `stack` flow rule is **pinned** (by reference to
current host behavior, not left to the profile): `stack` establishes
a positioning context and its children remain in normal flow unless
explicitly positioned absolute; explicitly out-of-flow descendants do
not contribute to preferred size; and, to keep `fits` from measuring
a structurally hollow candidate, in a **class-M template every
slot's ancestor path to the template root MUST remain in normal
flow** — certified attrs may position absolutely only spacer-only
subtrees, and never a slot reference or any ancestor of one (a
`stack → absolute column → slot` nesting would silently drop the
slot's whole subtree from `ideal` while §8 I4 still requires it to
render). The procedure is **early-false** (Taffy vocabulary
normative):

1. **Inline step:** if `avail.inline` is definite(v), measure the
   template root under `(inline: MaxContent, block: MaxContent)`;
   if `ideal.inline > v`, `fits` is **false — stop**. (If
   `avail.inline` is not definite, `ideal.inline` for step 2 is
   still the max-content inline size, but no inline comparison
   occurs.)
2. **Block step:** evaluated only if step 1 did not stop and
   `avail.block` is definite(v): measure under
   `(inline: Definite(w), block: MaxContent)` where
   `w = min(ideal.inline, avail.inline)` when `avail.inline` is
   definite, else `w = ideal.inline` — note the `min` is vacuous
   under early-false: whenever step 1 compared and did not stop,
   `ideal.inline ≤ avail.inline` already, so `w = ideal.inline`
   always (block is measured at the unwrapped preferred width — the
   honesty rule in one line); if `ideal.block > v`, `fits` is
   **false**. Otherwise — no compared axis failed — `fits` is
   true.

Axes that are unresolved or content-dependent are not compared. A
host MUST NOT substitute a large definite number for a max-content
constraint.

**Preferred-size honesty rule.** `fits` is a *preferred-size*
predicate. It does **not** guarantee that the committed layout is
overflow-free: a selected template subsequently stretched to C's full
inline size may grow taller than measured; a measurement-time nested
selection may differ from the committed one exactly when the two envs
differ (§3 S5); and — because S5 re-runs inner `select` under *each*
step's constraints — one measurement pair may even mix inner
candidates across its two steps (step 1's unresolved inline can pick
an inner fallback whose width passes, while step 2's definite `w`
picks a wider inner candidate whose height is what gets compared).
This composite is the frozen meaning; implementers MUST NOT "repair"
S5 to avoid it. Two adjacent honest corners, named: a `fits` whose
axes are all non-definite at some evaluation is classically **true**
there (everything fits unconstrained space; L2/§5 A2 keep this off
committed class-S paths), and for wrapping text or `fr`-track grids
the max-content tuple answers "does the *unwrapped preferred* size
fit," which is the chips-row question, not overflow-freedom after
wrapping. A placed-fit atom (`fitsPlaced`) is deliberately deferred
(§12); this spec does not pretend the two-step answers it.

Normative properties:

- **M1.** Measurement uses the same text-measurement and box-math
  path the host uses for layout (no parallel estimator). Measured
  slot content is the slot's *current* content; nested choices select
  per S5.
- **M2.** Measurement is observationally side-effect-free: it MUST
  NOT paint, run author-visible lifecycle, perturb live slot state,
  clone a slot identity into a second live tree, or (per S3) mutate
  `env`. Hidden or offscreen measurement of the single live subtree
  is permitted where the platform requires it.
- **M3.** Comparisons in §4–§6 are performed on unrounded
  logical-pixel values at the host's native layout precision, which
  MUST be at least f32; pixel snapping happens after selection.
  (M3 fixes single-host arithmetic; cross-host agreement is governed
  by §10, not by M3.)
- **M4 (required measurements only).** A remaining measurement is
  **required** iff some completion of it could change the containing
  condition's three-valued result. A non-required measurement MUST
  NOT be taken and MUST NOT rewrite an already-determined atom (in
  particular: step 2 is not required once step 1 has falsified
  `fits`; a `fits` atom whose condition is already determined by its
  S atoms is never measured; a `fits` atom with no definite axis
  invokes no backend and emits no diagnostic). At most one
  preferred-size measurement pair per class-M candidate, none after
  the winner. No fixed-point iteration exists in the kernel; hosts
  MAY cache, MUST NOT change observable results by caching.
- **M5 (failure rule; applies only to required measurements).** If a
  **required** measurement cannot be produced (missing measurement
  backend, non-finite result — and **only** those: exhaustion of a
  time or host-local budget is a host defect, never `unknown`,
  or class-M selection would become a hidden host capability), that **`fits`
  atom values to unknown** (§4 — so `not fits` is also unknown, and
  the candidate cannot be selected through that atom), and a typed
  diagnostic (`choice_measure_failed`) is emitted on the §9 D4
  channel. A failure during a non-required measurement a host took
  anyway MUST NOT alter any atom's value. The scan continues; S1
  totality lands on `kₙ` at worst. Wall-clock interrupts MUST NOT
  decide committed selection. Structural caps (candidate/template
  limits) are compile-time concerns, not runtime behavior.
- **M6 (measure-safety boundary).** Class-M measurement is a
  *support contract*, not best-effort: every slot of a class-M
  instance MUST carry a certified side-effect-free measurement
  projection — content the host can measure while honoring M1 and
  M2. Slot content without one is a **compile-time error**
  (`choice_slot_not_measurable`), fail-closed like A1; it is never a
  routine M5 runtime fallback, so host capability never becomes a
  hidden `select` input. Which content is certified (and how the web
  island measures it) is the eligibility/lowering layer's list,
  unfrozen (§12); the boundary's existence and its fail-closed
  compile-time shape are frozen here.

**Owed pre-implementation freezes (recorded 2026-08-20; open decisions,
not new kernel semantics).** Two gaps the round-1 program review
established must close before class-M implementation, and their closure
is an amendment to this section rather than an implementation choice:

- **The class-M measurement decision — three named axes, not one
  knob.** The round-2 "restrict class-M v1 to the demonstrated
  live-web subset" recommendation conflated three decisions. They are
  separated here so the owner decision names which axis each option
  moves. The axes are **layered, not mutually independent**: (b) MUST
  be a subset of what (a)'s frozen environment can interpret ("full
  admissible class" is a legal (b) only when (a) freezes the full
  context), and every (c) option must realize (a)'s semantics for
  (b)'s subset. None of the three reopens frozen law: axis (a) is
  fenced from the frozen two-step constraint tuples
  (`(MaxContent, MaxContent)` / `(Definite(w), MaxContent)`, this
  section) and from §3 S5 — nested-choice measurement semantics stay
  S5's law for every admitted instance, and the honesty rule's
  mixed-inner-winner composite stays the frozen meaning.
  - **(a) The measurement environment** — kernel semantics: what each
    two-step measurement step supplies to the template root beyond
    its constraint tuple — the containing block vs the offer (§2
    already distinguishes them; §6 must say which each step
    supplies), the percentage-resolution origin for slot descendants,
    and the visibility of template-root clamps that consult the
    parent. Options: freeze the **full** abstract context, or freeze
    a **named narrower context** that makes the genuinely
    environment-shaped hard cases compile-time errors — percentages
    in slot descendants during measurement, and parent-consulting
    template-root clamps. Either way the environment is FROZEN
    kernel law; the isolated prototype had to invent it
    independently — exactly the divergence a frozen kernel exists to
    prevent.
  - **(b) The admitted subset** — admission/support (deliberately not
    "eligibility," which §4 reserves for class-S realization): which
    class-M compositions v1 admits at all. **Nested choice instances
    inside a measured template belong here**, not in (a): admitting
    or compile-rejecting them is a subset cut, and S5 remains the
    measurement law for any nesting that is admitted. Options range
    from the full admissible class down to codex's recorded option of
    deferring class M entirely (v1 ships class S only; class M stays
    compile-rejected until its environment freeze lands — if this
    option is taken, the two-phase stable-gutter obligation needs a
    class-S carrier fixture, §11).
  - **(c) The realization technique** — unfrozen, per §12 and
    LLP 0486 §8.2's list, and **host-indexed**: each host prices its
    own technique (native: a synthetic kernel/Taffy measurement pass;
    web: M2's hidden-live-subtree measurement), and an option that
    violates frozen M2 — cloning a slot identity into a second live
    tree, or forced layouts that perturb live state — is illegal, not
    an alternative. "Demonstrated end-to-end on a live web
    host" is **evidence for (c)** — it shows a technique realizes the
    semantics — and is never a substitute for freezing (a); a web
    demonstration must not quietly make the web host the semantic
    authority for what measurement means.

  **Recorded recommended default (r15, restated after the r16
  re-sort):** freeze a narrow named environment under (a), admit the
  matching subset under (b) (nested-choice admission decided there,
  with S5 untouched), realize via M2's hidden-live-subtree technique
  on the web and a synthetic kernel pass natively under (c), and
  widen each axis on evidence. The choice remains the owner's; this
  fold re-sorts the menu, not the decision.
- **Deterministic work admission for M4.** M4 bounds the *number* of
  measurement pairs, not their content or dynamic cardinality, while
  T3 promises frame-bounded latency and M5 rightly forbids wall-clock
  interrupts from deciding selection. Owed: either an aggregate
  per-snapshot measurement cap certified at compile/link time
  (recommended), or a deterministic over-budget result with T3's
  guarantee relaxed accordingly. Whichever is chosen, the amendment
  MUST name the cap's **unit** — per instance, per layout pass, and
  whether nested class-M work (if axis (b) admits nesting) counts
  inside the outer instance's budget — and budget exhaustion
  must remain deterministic data, never a timing race (M5's rule):
  in particular, LLP 0488's shared budget ledger (the §9 D4 family-3
  join) can surface exhaustion but can never turn it into an M5
  `unknown` — exhaustion stays a host/compile defect, never a hidden
  selection input.

## 7. Re-selection timing

- **T1.** Selection is (re)computed during layout, before the
  selected candidate's own layout, whenever any `env` input changes:
  `avail` (state or value), the theme snapshot, or — for class M —
  **any consumed input to the preferred-size path**, tracked as
  measurement-dependency generations: slot intrinsic content (data,
  structure, text), font registration/loading, locale and direction,
  text scale, replaced-element and native-control intrinsics,
  measurement-backend generation (non-exhaustive). Two rules keep
  this set coherent with S2/M1: the committed frames of the current
  winner are never a T1 input (no same-pass feedback; M4); and — the
  **endogenous-measurement admission rule** — no selection input of a
  class-M instance may be a function of committed layout, judged
  **transitively over every selection input** (measured content,
  style, relations, typography, replaced intrinsics), with certified
  forms in A1's shape:
  - **Negative (closed set):** container-relative branching or
    container queries keyed to C or to any container C's committed
    layout can affect (own size, ancestors C feeds, siblings sharing
    an allocation); layout-dependent replaced intrinsics
    (`srcset`/`sizes`, container-query length units in slot
    content); LLP 0195 layout-responsive *style* in slot content
    keyed to such containers; LLP 0199 measured relations whose
    source end is not proven independent of C.
  - **Positive:** viewport/host size-class branching; data, theme,
    and locale inputs; nested choice (arrangement, carved out
    below).
  - **Fail-closed:** slot content the analyzer cannot prove positive
    is rejected at the instantiation site
    (`choice_endogenous_measurement`, an **error** for class M;
    informational elsewhere). The witness is part of M6's class-M
    slot certificate: measurable side-effect-free **and** no
    dependency path from committed choice geometry to any selection
    input.
  Consequently every admitted T1 dependency is exogenous, T1 tracks
  them **all** (no suppressed-but-current staleness), and
  measurement of current content (M1) never disagrees with the
  committed selection's inputs. One clarification is frozen: a
  nested choice's post-commit re-selection (S5/T2) is *arrangement*,
  never a T1 measurement input — nesting is not endogenous. This
  closes the commit → responsive-measurement → `fits` orbit that
  A5's axis witness alone cannot see; the §12 extension owns
  readmitting such content with real transition semantics.
- **T2.** Within one layout pass a host MAY evaluate selection under
  tentative constraints (e.g., flex measure passes), but the rendered
  result MUST correspond to `select` under the **final** constraints
  of that pass, and nested containers re-select under committed
  constraints (S5). Intermediate selections are never
  *painted* or reported; they are, however, legitimate inputs to an
  ancestor's `fits` measurement (S5) — that is their one observable
  influence.
- **T3.** Re-selection latency: the rendered candidate MUST reflect
  current `env` by the end of the layout pass that consumed the
  change on kernel-owned hosts; on the web's island realization it
  MUST reflect it within one frame of the host's resize/content
  notification — the one-frame bound is **this document's** T3
  promise (the notification delivery rides the host's observer
  machinery, correlated through the 0488-governed fabric; the
  0195 channel's scheduler contract does not own it). Declaratively
  realized instances inherit the browser's own `@container` timing,
  which is accepted as conforming.
- **T4 (feedback cycles: unreachable by admission).** `select` is pure
  (S2) and layout feedback through ancestors is real (§5 A3's
  exclusion), so a stationary cycle would otherwise be possible
  (candidate A creates ancestor overflow, the scrollbar narrows C and
  selects B, B removes the overflow, the width returns and re-selects
  A — with no settled rendering and divergent implicit host
  outcomes). v1 removes that state by construction: §5 A5's admission
  witness certifies, per instance, that consulted axes cannot be
  affected by C's own contribution, so re-selection under T1 only
  ever responds to exogenous change and pure `select` is the whole
  committed behavior. There is no runtime cycle handling in v1
  (see §12 for the deferred extension).

## 8. Identity, state, focus, accessibility

- **I1 (identity).** Each slot's box is one stable identity across
  all candidates and across re-selection. A selection change is a
  *move*, never an unmount/remount, of slot boxes. Component state,
  input values and composition state, scroll positions, and
  media/playback state MUST be preserved wherever the host platform
  preserves them under reparenting.
- **I2 (platform caveat, normative disclosure).** Hosts whose
  platform resets certain embeds on reparent (web: `webview`/iframe
  and some media elements) MUST document this, and the analyzer
  SHOULD warn (`choice_stateful_embed`) when such tags appear in a
  slot of a class-M or multi-candidate instance.
- **I3 (focus).** If focus is inside C at re-selection: where the
  focused element survives the move (the I1 case), hosts MUST restore
  focus to it when the platform drops focus on move; where an
  I2-disclosed embed reset destroyed the focused element (a
  cross-origin iframe reload cannot be restored into), hosts MUST
  move focus to a deterministic fallback target,
  disclosed in the §9 D4 outcome record — never silently to the
  document root. The fallback target MUST satisfy the host's
  accessibility audit: the bare slot carrier is focusable but
  deliberately has no role or name (§2), so focusing it as-is trips the
  repo's `focusable-no-label` error class. The owner decision (recorded
  2026-08-20, open): the fallback is either the nearest *labeled*
  surviving container, or the carrier exposed through a temporarily
  named **focus proxy** — under option 2 the accessible name lives on
  the proxy and never on the §2 carrier itself (the carrier stays
  semantics-transparent; naming it would change the frozen profile),
  and the proxy is removed once focus moves on. Under either option,
  when **no labeled ancestor survives** the re-selection, the host
  falls back to the nearest labeled landmark/root of the containing
  focus scope — still disclosed in the D4 outcome record, still never
  silently the document root. Either choice requires visible-focus
  evidence in the corpus. The carrier profile itself is unchanged. Lowerings SHOULD keep internally-focusable embeds
  under stable parents (a projection-style realization; eligibility
  MAY require it — unfrozen). Focus scopes and open overlays anchored
  inside C MUST NOT be dismissed by re-selection.
- **I4 (order and participation).** Guaranteed, under every
  candidate: (a) on web, **the relative DOM order of slot boxes
  equals `L`**; (b)
  templates **never suppress a slot's accessible descendants** — no
  candidate may remove a slot's content from rendering or from
  accessibility participation; and (c) **accessible descendants of
  distinct slots retain `L` order in Exact's semantic tree**. These
  follow from §2's slot-terminal order rule, the certified-attr
  participation principle, and the host obligation that slot boxes
  are single-identity and never permuted or omitted. Two honest
  boundaries: an **empty** slot (dynamic content currently rendering
  nothing) keeps its box and identity but need not produce an
  accessibility node — I4 preserves accessible *descendants*, not
  empty group nodes — and `spacer` terminals are a11y-invisible. An
  I2-style caveat: some user agents relinearize flex/grid content
  visually in their own accessibility trees; hosts MUST preserve (c)
  in the semantic tree they own and MUST document UA-level
  divergence. **Not guaranteed:** a universal visual-order theorem.
  Nested grouping legitimately interleaves spatial order
  (`row(column(A,B), column(C,D))` reads spatially A,C,B,D with no
  reorder attrs); `stack` overlaps. The certified set still excludes
  explicit reorder features as hygiene (§2), without upgrading that
  to an order theorem.
- **I5.** Selection change fires no author-visible event in the
  kernel. (An observation surface, if any, is an unfrozen extension;
  read-only contract clauses can consume §9 D4 instead.)

## 9. Static realization and degradation

For a rendering with no dynamic evaluation at view time (SSG output
before activation, server render, print):

- **D1.** **Realization-eligible** class-S instances (§4, §5 A4) MUST
  be realized declaratively where the target supports it (web: CSS
  `@container`), so the full selection behavior works with zero
  script. Such output is not "degraded." Class-S instances that fail
  the eligibility predicate are realized as islands and remain
  conforming.
- **D2.** Island-realized and class-M instances render the **static
  default candidate**: `kₙ` (the unconditional fallback), unless the
  author has marked a different candidate as the static default (the
  marker's syntax is unfrozen; its meaning — "render this one
  statically" — is frozen here). The static default MUST be a valid
  full rendering, not a placeholder.
- **D3.** On activation, ordinary selection per §3 occurs; the
  transition from static default to live selection is a normal
  re-selection under §7/§8 (state-preserving, order-preserving).
- **D4 (decision record, three parts).** Wherever an agent bridge
  exists, the host MUST be able to report, per instance:
  **(a) a replayable decision-input record** — sufficient for an
  independent evaluator to recompute `select` without consulting the
  host's answer: the ordered candidate list and each candidate's
  canonical condition structure as **executable data** (a DAG/AST
  with stable atom IDs and candidate ownership; a content-addressed
  program artifact is acceptable only if it is retrievable with the
  record — a bare digest is not replayable); per axis, `avail`'s
  state and definite value as compared — for a **class-S** instance
  an unconsulted axis MAY be omitted from the record; a **class-M**
  record MUST carry both axes
  explicitly (measurement consults the whole environment, so sparse
  omission is a class-S affordance only). P1/P3 equality is computed
  over the **normalized** record — the named `normalize(record)` step
  **strips** every unconsulted class-S axis before comparison, so the
  sparse record is the one canonical form (a producer that includes
  an unconsulted axis's true classifier state has it dropped; the
  sealed corpus's sparse records are already canonical). Omission is
  represented by **absence, never by a classifier state**:
  normalization MUST NOT write `content-dependent` (or any §2 state)
  as a stand-in for an omitted axis — content-dependent is a real
  classifier verdict that §5 A1 forbids on S-atom axes and the web
  replay rejects for size atoms, so filling omissions with it would
  alias two different facts. Consulted axes remain mandatory and
  missing ones fail closed;
  every outcome-affecting
  atom's resolved operator and operands; a **per-step tagged
  measurement transcript** for every class-M candidate scanned —
  each step recorded as `not_required(reason)`,
  `success(constraints, value)`, or `failure(code)`, so an inline
  early-false never demands an `ideal.block` and an M5 unknown is
  replayable from the input record; the block-step inline constraint
  `w` where step 2 ran; and backend capability state;
  **(b) an outcome record, in two projections** (r16 — §4's
  denotational algebra deliberately lets conforming hosts
  short-circuit and reorder, so byte-equality over an ordered atom
  trace is not a legal cross-host test):
  the **cross-host outcome projection** — the classification and
  eligibility verdict (§4), the selected candidate (index, and name
  if named), each scanned candidate's *condition-level* three-valued
  result (denotational, hence strategy-independent), any M5
  failures, and any §8 I3 fallback-focus disclosures — is the object
  §10 P1 compares across hosts; and the **host-local evaluation
  trace** — each atom the host actually evaluated, with its
  three-valued result and the host's evaluation order, plus any
  host-added diagnostics — which is never compared byte-wise across
  hosts. Instead each trace is validated **independently** against
  the condition AST and the input record: every recorded atom value
  MUST equal its denotational value under the record, the trace MUST
  entail the recorded winner, and no recorded measurement may
  violate M4's required-measurement rule. Diagnostics required by
  §11's normative minimum are compared as **set containment** — a
  host may add diagnostics, never omit required ones; and
  **(c) a provenance/invalidation record** — token-environment and
  measurement-dependency generations (§7 T1), backend epochs, and
  cache/timing facts. Records (a) and (b) are semantic; (c) is
  host-local and is **not** part of §10's equality comparisons.
  **Decision-record identity envelope (named tuple, r16).** Every
  decision record rides an envelope of exactly: the **build/plan
  identity** (LLP 0485 §3.4 `planDigest`, paired with
  `sourceVersion`); the **runtime instance identity** of the choice
  instance (LLP 0485 §3.3.16 identity 3 — node site plus instance
  path — with its scoped producer/root generations and the
  ExecutionGeneration it lives in); the **layout-pass/snapshot key**
  the decision was computed in; a monotone per-instance **decision
  revision**; and, under live revision, the LLP 0500 **HotRevision**
  that last changed this instance's decision inputs (HotRevision is
  meaningful only inside its ExecutionGeneration and never bumps it,
  so an HMR patch is not a producer restart). **Staleness** is
  judged by ExecutionGeneration, then HotRevision, then decision
  revision; **atomicity** by the whole tuple — (a), (b), and (c) for
  one decision MUST come from one atomic decision revision (never
  stitched across revisions); **equality** (P1/P3) excludes every
  envelope field, like (c). An agent-facing representation MUST
  additionally bind to the Acto snapshot consistency token
  (`snapshotId`) of the snapshot that carried it.
  **The 0488 family-3 join, stated once for both documents:** choice
  adopts LLP 0488 §3.8's scoped identity and pass/snapshot fencing
  vocabulary (what 0488 names the **correlation envelope**) for this
  envelope, and shares 0488's budget **ledger**
  for §6's owed M4 work-admission cap; it does NOT adopt the
  0195/0488 channel's scheduling, invalidation, damping, or
  oscillation contract — `fits` evaluation and §7 T3 timing are
  0487-owned — and ledger exhaustion is deterministic data on the
  M5 host-defect path, never `unknown`.
  Purely static output (D1 without script, D2 before activation)
  carries no reporting obligation.

## 10. Determinism and cross-host parity

- **P1.** Equal decision-**input** records (§9 D4(a) after
  `normalize` — the outcome is
  never part of the compared input) MUST yield the same selected
  candidate on every host; the compared outcome object is D4(b)'s
  **cross-host outcome projection** (host-local evaluation traces
  are validated per D4(b), never cross-compared). **Tier scope:** the
  selection *function* (this spec's §3–§6) and the logical slot tree
  with its order/participation guarantees (§8 I4) are Semantic-tier
  everywhere; the equal-snapshot winner-equality claim — same
  decision-input record ⇒ same selected candidate — is
  **Decision-tier** (it holds exactly where the snapshots are equal),
  and equality of the *realized arrangement* across hosts where
  snapshots differ is governed by P2. (LLP 0486 §4's **Semantic** row
  — renamed from "T1" by its r2, precisely to avoid colliding with
  this document's §7 T1/T2/T3 — carries the matching clarification.)
- **P2 (uncertainty intervals).** Hosts legitimately differ in
  measured preferred sizes and in resolved `avail` values within
  LLP 0486's **Band**-tier tolerance bands (renamed from "tier-3" by
  its r2; never in `avail` *states*, §2).
  A fixture asserting a specific selection is valid only if, giving
  every host-variable operand its uncertainty interval, the decision
  margin of every outcome-affecting comparison lies **strictly on one
  side of zero**; host-variability propagates through nested
  decisions (an inner selection inside a band makes every quantity
  it feeds host-variable). Otherwise the fixture MUST declare
  per-host expected selections.
- **P3.** On a single host, an identical serialized decision-input
  record (after `normalize`) MUST reproduce the identical selection,
  and the recomputed
  result MUST be compared against the separately recorded
  cross-host outcome projection
  (§9 D4(b)) — a consistently-serialized wrong answer is thereby
  detectable — while the host's evaluation trace is validated
  against the AST per D4(b). Nondeterminism at a fixed input record is always a
  defect; runs whose records legitimately differ (fonts, environment,
  content) are compared under P2, not P3. Comparison domains are
  **rung-relative embeddings of one boolean predicate**: kernel-owned
  hosts and islands compare in M3's domain (engine pre-paint used
  content-box values; device-pixel snapping strictly after
  selection); the declarative CSS rung evaluates the same first-true
  predicate over the browser's used query-container size, certified
  structurally per §5 A4, with its rounding differences governed by
  P2. What is frozen is the predicate and the box, not a single
  numeric representation no browser can hold.

## 11. Diagnostics (normative minimum)

Compile-time (analyzer) errors: fewer than two candidates
(`choice_single_candidate`); final candidate conditional
(`choice_no_fallback`); slot membership/order violation in a template
(`choice_slot_order`); content, forbidden node kind, or non-container
root in a template (`choice_template_content`); unprovable
descendant-independence on an S-atom axis
(`choice_condition_axis_sizing`, fail-closed per §5 A1); a `fits`
instance with no statically provable descendant-independent axis
(`choice_fits_unconstrained`); `fits` naming another candidate
(`choice_fits_arity`); class-M slot content without a certified
measurement projection (`choice_slot_not_measurable`, §6 M6); a
class-M template in which some slot's ancestor path to the template
root leaves normal flow (`choice_slot_out_of_flow`, §6). The last
two are deliberately distinct codes: `choice_slot_not_measurable`
is reserved for a missing M6 measurement projection, and reusing it
for the structural out-of-flow rejection would drift M6's meaning
until a perfectly measurable text slot under an absolute ancestor
became the same error as an unprojected webview (owner ruling,
2026-08-20, closing the gap the conformance corpus surfaced).
Compile/link-time for class M additionally: an endogenous selection input
(`choice_endogenous_measurement`, §7 T1 — error for class M,
informational elsewhere). Warning: `choice_stateful_embed` (§8 I2).
The conformance corpus MUST carry the counterexamples this document
names as fixtures: the wrap-line two-fixed-point,
fraction-of-definite-CB under an intrinsic offer, the L1
specified-definite fixed width under a max-content offer,
percentage-padding-inside-a-fixed-width feedback, stable-gutter
scroll admission, nested out-of-flow stack, endogenous-measurement
rejection (content, responsive-style, and measured-relation
variants), the aspect-ratio transfer closure in both axis-processing
directions plus an unseeded transfer component, and slot-carrier
intrinsic/empty/**baseline**/focus behavior on kernel and web hosts.
One further named obligation: a **compound-condition** D4 fixture —
a condition combining atoms with and/or/not whose replayable input
record and per-atom three-valued results round-trip through §9 D4 —
which, because the sealed fixture schema's condition arms are today
`always`/`size`/`size-token`/`fits` only, is recorded as a
**schema-extension obligation** (adding combinator arms is a corpus
amendment under §1's versioned-authority rule, not a silent edit).
**The same corpus amendment MUST repair the sealed executor's
comparator** to D4(b)'s r16 shape — compare the cross-host outcome
projection, validate each host's evaluation trace independently
against the AST, and check required diagnostics by set containment —
because the current harness byte-compares ordered per-atom results
and full diagnostics arrays cross-host, which is stricter than the
kernel on compound conditions (it would reject a conforming
short-circuit strategy or an extra host diagnostic); today's
single-atom fixtures conceal the difference, so the repair is owed
with, and gated on, the first compound fixture, not before. The
amendment also owes: `normalize` **equivalence fixtures**
(sparse vs over-complete records of one environment normalizing to
the identical canonical form, run through an actual normalizer, not
a raw compare); D4 **branch-coverage fixtures** (a successful
two-step measurement, an inline early-false, an M5 failure
transcript, and compound-condition M4 non-requirement); an **I3
visible-focus fixture** landing with the owner's §8 I3 pick (09d's
carrier focus profile is a different claim); and — only if the §6
class-M decision's axis (b) defers class M — a **class-S carrier**
for the two-phase stable-gutter obligation, so fixture 05's
target-law accept is not vacated by class M's own deferral.
One honesty
obligation on an existing fixture: while the Layout Profile records the
A5 stable-gutter pin as `target-unimplemented`, the stable-gutter
scroll-admission fixture MUST expect the fail-closed
`choice_feedback_cycle` rejection — a target-state pin is not present
conformance evidence, and an analyzer that treats `target-unimplemented`
as satisfying the A5 proof is a corpus defect, not a pass. (Discharged
by r15: the analyzer now discharges A5 only through a PRESENT pin,
and fixture 05 is encoded as a **two-phase obligation** — current law
expects the rejection; the target-law accept expectation, keyed to
the pin flipping to a present fact, is recorded in the fixture's gate
note and the corpus README, since the fixture schema cannot express
conditional expectations.)
Compile/link-time additionally: a missing feedback-unreachability
witness (`choice_feedback_cycle`, §5 A5, discharged like A1).
Runtime: `choice_measure_failed` (§6 M5) and the §9 D4 decision
records. Hosts and tooling MAY add more; they MUST NOT ship the
operator with less.

## 12. Explicitly unfrozen

Surface syntax and keywords (including any axis-named `fits` form —
and the atom's surface name, where the strong lean of record is the
more honest `fitsPreferred`: §6's own honesty rule says the predicate
answers preferred-size fit, not overflow-freedom, and the shorter name
invites exactly the misreading the rule exists to prevent);
IR encoding; the kernel evaluation algorithm, caching, and its
interaction with Taffy's pass structure (§4's denotational semantics
makes evaluation order unobservable); the realization-eligibility
predicate's concrete form beyond §5 A4 — candidate shapes recorded: a
blessed structural subset; a laminar-interval test (every template
container an interval over `L`; laminar across candidates ⇒ one
union tree, crossing ⇒ island); a general union-tree/
`display:contents` compilation proof; a projection model in which
templates are immutable layout projections over permanent slot
parents rather than semantic-tree parents — and all web lowering
mechanics, including the M6 certification list (which slot content
counts as measurable and how the island measures it); budget *numbers*
(M4/M5 fix the shape, not the values — and when the §6 owed M4
work-admission cap lands, its amendment MUST update M4, M5, and this
clause together, so the cap's shape, its determinism rule, and the
unfrozen-numbers boundary cannot drift apart); transition/motion behavior on
selection change (LLP 0313's domain; kernel v1 renders the new
selection with no implicit animation); composition with virtualized
collections (RFC 0088; class-M instances inside virtualized items
need an estimated-measure rule that does not exist yet);
**runtime-feedback-capable instances** — v1 admits only
A5-certified instances, so cycle handling does not exist in the
kernel; the deferred extension (a finite candidate-transition system
with deterministic strongly-connected-component fallback, per the
round-6 review sketch) must first freeze a pure transition boundary
(`advance(layoutSnapshot, candidate) → nextEnvironment`) and a web
transaction model compatible with LLP 0195's channel; `fitsPlaced`
(a placed-fit atom asserting overflow-freedom at committed size —
add only with corpus evidence of a cycle-free definition);
additional class-M atoms; equality atoms; explicit preference
weights; hysteresis (none in v1; class-M selections near a wrap
boundary — or fed by an ancestor scrollbar — can flicker under
continuous resize; documented, accepted, revisit with evidence via an
explicit `stable`/hysteresis extension rather than a silent host
behavior); non-`horizontal-tb` writing modes (v1 fixes the profile's
logical inline/block axes and the §6 step order; other writing modes
are a semantic extension, not an open question); `scroll` or nested
`choose` in templates; any observation event for selection changes;
the static-default marker's syntax.

## 13. Open Questions

- Is `kₙ`-as-static-default the right D2 rule, or should the static
  default be the *most preferred* candidate under a declared assumed
  size? Current ruling (both review families concur): fallback, for
  guaranteed validity; revisit with SSG evidence.
- Eligibility predicate: start from the conservative blessed subset
  or the laminar-interval test (§12)? §5 A4 is frozen either way.
- A read-only LLP 0085 contract clause asserting the selected
  candidate after settled layout (consuming §9 D4): both families
  endorse adding it once D4 lands; exact clause shape is open.
- What corpus evidence would justify `fitsPlaced`, and can it be
  specified without reintroducing a feedback edge?
- Should D4 additionally expose a distance-to-decision-boundary
  interval (tooling aid for P2 fixture authoring)?
- ~~The overlay-vs-classic scrollbar ruling~~ **Closed (2026-08-20)**
  onto A5's own recommendation: the v1 pin is **reserved/stable gutter,
  host-independent** — already stated normatively in A5's
  invariant-overflow-ancestors form. What remains is implementation:
  the profile records the pin as `target-unimplemented`, and per A5
  (r15) a target-state pin does not discharge — the default scroll
  composition fails closed (`choice_feedback_cycle`, fixture 05's
  two-phase obligation) until the pin becomes a present fact. That is
  the Layout Profile's work, not an open semantic question. (The
  `stack` flow rule, previously open here, is pinned in §6.)
- Are layout-dependent replaced intrinsics ever safe to readmit as
  T1 inputs (undoing the endogenous cut), or is that permanently the
  §12 extension's problem?
- For §5 A1, which positive forms beyond the named ones should the
  certified proof set admit (percentage chains, definite grid
  tracks), and what is the vocabulary of the declared symbolic
  requirements a library component exports for link-time discharge?
- Should the §2 `avail` table become a generated authority artifact
  (one source generating the analyzer's A1 prover and the runtime
  state function — the LLP 0486 §5 protocol-inventory move)? The round-2
  sketch worth adopting if yes: a versioned **typed proof DAG** — forms
  like `Const`, `Fraction(of)`, `Allocation(chain)`, `Transfer(from)`,
  `ContentDependent`, `Unresolved` — replacing the mutable string-form
  lists, with the analyzer's facts, the kernel's admission receipt, and
  the D4 classifier trace all generated from that one authority.
  (Round-2 follow-up nudge: adopt with the first analyzer change that
  touches the A1/A2/A5 form lists or the closed-scroll-chain arm —
  those lists are exactly the churn the artifact would freeze.)
- **Flagged owner question (round 2; touches a frozen §2 clause, so it is
  recorded, not changed):** §2 maps a failed runtime resolution of an
  A1-positive axis — non-finite included — to **definite(0)**. The
  round-2 review argues a non-finite result is a failed computation, not
  a physically zero box: definite(0) can make a `<` atom true and select
  a non-fallback candidate, and the kernel prototype rejects invalid
  constraints instead. The candidate repair is routing non-finite (as
  distinct from legitimate clamped negatives) through typed
  failure/unknown — but that repair carries a priced cost the flag
  must state: `unknown` is an L2 concern, and routing an A1-positive
  axis of a **committed, painted class-S instance** through unknown
  would put unknown on exactly the surface frozen L2 forbids it on.
  The third option, avoiding both a fictitious zero box and an L2
  breach: treat a non-finite resolution of an A1-certified axis as a
  **host defect that fails the layout pass** (the certificate proved
  the axis resolvable; non-finite means the host broke its own
  proof), surfacing through the host's pass-failure path rather than
  through selection. Changing any of this is an owner amendment to a
  frozen clause; until taken, definite(0) stands.

## 14. Conformance

*(Added by r17's editorial conversion; this section indexes and
formalizes obligations the body already states. Where this section and
§§1–13 could be read to disagree, §§1–13 govern.)*

### 14.1 Requirement keywords

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY in this
document are to be interpreted as described in RFC 2119 (as the
Summary already declares). Only sections and clauses stating
requirements with these keywords are normative; examples,
illustrations marked *non-normative*, the revision history, §13's open
questions, and §16's algorithm restatements are not.

### 14.2 Conformance classes

A product claims conformance to this specification as one or more of
four classes. Each class's obligations are the body clauses indexed
here; this table adds none.

1. **Analyzer** (compile/link tooling). MUST implement the
   compile/link-time obligations: the candidate/template structural
   rules and certified-attr principles (§2); classification and the
   recorded class/eligibility verdicts (§4); the A1/A2 declare/
   discharge proofs with their certified-form vocabularies and the A5
   admission witness, all fail-closed at the instantiation site (§5,
   incl. the A2 pin-identity vs A5 present-pin split); the M6
   measure-safety certificate and the §7 T1 endogenous-measurement
   admission rule (class-M error); and every compile/link diagnostic
   of §11's normative minimum. An analyzer MUST NOT admit an instance
   whose required witness it cannot certify.
2. **Evaluator** (a selection engine — kernel, island, or replay
   executor). MUST implement the §2 classifier's unique closure
   result; `select` with S1–S5 (§3); strong-Kleene condition
   evaluation with fail-closed unknown (§4); the two-step `fits`
   tuple with M1–M5 (§6); and re-selection timing T1–T4 (§7). It MAY
   choose any evaluation order, caching, or short-circuit strategy
   that preserves the denotational values (§4) and M2/M4.
3. **Host / presenter** (a platform realization). MUST exhibit the
   frozen slot-carrier profile (§2) under its declared lowering;
   preserve identity/state/focus/accessibility I1–I5 (§8); realize
   static output per D1–D3 (§9); meet T3's latency bound (§7); and,
   where an agent bridge exists, produce the three-part D4 decision
   record on its identity envelope (§9). Hosts MAY add diagnostics
   and MUST NOT ship the operator with less than §11's minimum.
4. **Corpus runner** (the conformance-suite executor). MUST execute
   the designated suite (§14.3) against a declared host lowering;
   compare selections and outcome objects per P1–P3's comparison
   domains (§10), using D4(b)'s cross-host outcome projection —
   validating host-local traces independently and checking required
   diagnostics by set containment — and honor P2's
   uncertainty-interval rule when adjudicating fixtures. (The sealed
   executor's stricter ordered-trace comparator is the §11 owed
   repair, gated on the first compound-condition fixture.)

A single implementation commonly claims several classes (the
production kernel is an evaluator and part of a host; the prototype
replay harness is an evaluator plus corpus runner). Claims are
per-class, and evidence for one class never substitutes for another
(§2's carrier ruling: each host demonstrates the profile for itself).

### 14.3 Conformance suite

The conformance suite of this specification is the fixture corpus
`tests/layout/choice/` together with its execution-expectations
authority `tests/layout/choice/corpus-execution-expectations.json`;
its runner of record is the registered check
`choice-layout-fixture-corpus` in `exact-verify.json` (run via
`bun scripts/exact-verify.mjs`). The suite is a **versioned executable
authority** exactly as the Summary states: where prose and fixtures
disagree, the corpus is the tie-breaking authority for the observable
in question, *and* the disagreement is itself a defect in this
document that MUST be resolved by revising one of the two — a
fixture edit that changes admission, diagnostics, a classified state,
or the selected candidate is an amendment to this document, never a
silent fixture-side refinement (§1's maturity marker governs which
vocabulary edits are non-semantic). Suite coverage is read honestly
per the Summary's executor disclaimer: prototype replay/carrier
evidence, not yet end-to-end conformance.

## 15. Terminology index

*(Added by r17; pointers only — every definition lives where cited,
and this index defines nothing.)*

- **Choice container (C), candidate, preference order** — §2.
- **Slot; slot carrier; frozen carrier profile** (styling-free,
  metric-, baseline-, semantics-transparent, programmatically
  focusable, 0×0 when empty) — §2.
- **Arrangement template; certified template attrs; laminar interval
  lemma** — §2.
- **Available size (`avail`); the three states definite(v) /
  unresolved / content-dependent; the classifier's staged closure;
  lemmas L1 (committed definiteness) and L2 (observable class-S
  determinacy)** — §2.
- **`select`; S1 totality, S2 purity, S3 single ordered scan,
  S4 locality, S5 nesting** — §3.
- **Condition; strong-Kleene three-valued evaluation; class S
  (size-threshold) and class M (measured-fit) atoms; classification
  vs realization eligibility** — §4.
- **Consulted axis; A1 (S-atom axes, fail-closed); A2 (`fits` axes;
  pin-identity license); A3 (same-pass acyclicity); A4 (CSS
  containment prerequisite); A5 (admission: feedback-unreachability
  witness; present-pin discharge)** — §5.
- **Two-step ideal tuple; early-false; preferred-size honesty rule;
  M1–M6** — §6.
- **Re-selection timing T1–T4; endogenous-measurement admission
  rule** — §7.
- **Identity/state/focus/accessibility I1–I5** — §8.
- **Static realization D1–D3; decision record D4 (replayable input
  record, `normalize(record)`, cross-host outcome projection vs
  host-local evaluation trace, provenance record); decision-record
  identity envelope; the 0488 family-3 join (correlation envelope +
  budget ledger)** — §9.
- **Parity P1–P3; Semantic / Decision / Band tiers; rung-relative
  comparison domains** — §10.
- **Normative diagnostic minimum** (`choice_*` codes) — §11.
- **Explicitly unfrozen surface** — §12.

## 16. Algorithms (non-normative restatements)

*(Added by r17. Each algorithm is **derived from** the cited section
and restates it in step form for implementers; the prose it is derived
from remains the normative text, and on any divergence the cited
section governs — an algorithm here is non-normative exactly where it
would conflict. The frozen-predicate discipline of §1 is unaffected:
nothing here supersedes a frozen clause by paraphrase.)*

**Algorithm 1 — `avail` classification** *(derived from §2)*.
Given C's per-axis sizing constraints and the parent constraint at
this evaluation (the selected candidate is never an input; judge the
conservative union of all candidate templates):
1. **CD-closure (absorbing):** seed content-dependent wherever the
   axis's used content-box expression can depend on C's descendants
   under any candidate (sizing mode, content-valued clamp, or
   indefinite-offer stretch); close over transfers; nothing later
   promotes out.
2. **Definite closure:** on remaining axes, seed definite(v) where a
   numeric pre-overflow content-box is derivable without descendant
   input from this evaluation's containing sizes; close over
   transfers from definite axes until saturated.
3. **Unresolved closure:** on what remains, seed unresolved for
   stretch/fill/fraction whose determining size or offer is not yet
   available; close over transfers. Prefer stage 2 where both could
   seed.
4. **Residue:** any axis in no closure is content-dependent
   (fail-closed).
Return, for each definite axis, `v` = the pre-overflow resolved
content-box length per §2's subtraction rules. Any stage ordering,
worklist, or cache producing this unique closure result conforms.

**Algorithm 2 — selection** *(derived from §3)*.
Evaluate candidates `k₁ … kₙ` in declaration order; return the first
whose condition VALUES to true under `env` (Algorithm 3). `kₙ` is
unconditional, so the scan always returns. Evaluating or measuring a
candidate never alters `env` within the scan.

**Algorithm 3 — condition evaluation** *(derived from §4)*.
Value each atom in {true, false, unknown}: a class-S atom compares
`avail`'s definite value (unknown on an unresolved axis; illegal on
content-dependent); a class-M atom is `fits` (Algorithm 6), unknown
on M5 failure. Combine with strong-Kleene `and`/`or`/`not`. The
result is denotational; any order or short-circuit preserving it (and
M2/M4) conforms. Unknown never selects.

**Algorithm 4 — A1/A2 declare/discharge** *(derived from §5)*.
At isolation compile: emit each instance's symbolic requirements
(A1: descendant-independence per S-atom axis; A2: at least one
descendant-independent axis for `fits`, where a referenced profile
pin witnesses by identity). At closed-application/route link: for
every instantiation site, discharge each requirement through the
certified positive forms; on any axis where independence cannot be
proven, fail closed at that site (`choice_condition_axis_sizing`;
`choice_fits_unconstrained`).

**Algorithm 5 — A5 admission witness** *(derived from §5)*.
Compute the static subject set (every S-atom axis; plus, iff the
condition contains `fits`, every axis the classifier can report
definite in any statically possible parent context). For each
subject axis, certify the entire used-content-box expression
ancestor-immune via the certified discharge forms — noting that the
invariant-overflow-ancestors form discharges only through a PRESENT
profile pin (`target-unimplemented` records target law and fails
closed). Opaque or dynamic ancestry fails closed; a missing witness
rejects at the instantiation site (`choice_feedback_cycle`).

**Algorithm 6 — measured fit (`fits`)** *(derived from §6)*.
Early-false, at most one measurement pair per class-M candidate,
required measurements only (M4):
1. If `avail.inline` is definite(v): measure the template root under
   `(MaxContent, MaxContent)`; if `ideal.inline > v`, return false.
2. If `avail.block` is definite(v): measure under
   `(Definite(w), MaxContent)` with `w = ideal.inline` (the `min` is
   vacuous under early-false); if `ideal.block > v`, return false.
3. Return true (axes not definite are not compared).
On a required measurement that cannot be produced (missing backend or
non-finite only), return unknown and emit `choice_measure_failed`
(M5); budget exhaustion is a host defect, never unknown.

**Algorithm 7 — `normalize(record)`** *(derived from §9 D4(a))*.
Given a decision-input record: strip every unconsulted class-S axis
(absence is the canonical representation; never write
`content-dependent` or any classifier state as a stand-in). Class-M
records carry both axes and are unchanged by the strip. Consulted
axes are mandatory; a missing one fails closed. P1/P3 equality is
computed over this canonical form.

**Algorithm 8 — outcome projection and trace validation**
*(derived from §9 D4(b), §10 P1/P3)*.
To compare hosts: (1) build each host's cross-host outcome projection
(classification, eligibility, winner, per-candidate condition-level
three-valued results, M5 failures, I3 disclosures) and compare those
objects; (2) validate each host-local evaluation trace independently —
every recorded atom value equals its denotational value under the
input record, the trace entails the recorded winner, and no recorded
measurement violates M4; (3) compare diagnostics against §11's
required minimum by set containment (extras allowed, omissions fail).
Never byte-compare traces across hosts.
