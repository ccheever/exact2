# LLP 0485: The Flat Plan, Specified — an implementation specification for the total-plan model

> **Execution-posture banner (clock-join annotations, 2026-08-20 —
> added after the holistic corpus reviews; frozen text below is
> unchanged).** This spec was frozen before the 2026-08-20 decisions
> (LLP 0505 r3 docket; the RFC 0498 r6 trio; RFC 0483 §2; LLP 0500 D1
> as amended). Where this file's §12/§13/§15 text disagrees with those
> decided sources, **the decisions govern and the text here is
> historical**; dated `DECIDED` annotations mark each affected site.
> Known semantic errata carry their own annotations and tickets.
> **Amendment 1 (end of document, 2026-08-20) is the pointed freeze
> amendment**: it consolidates the annotated sites into normative
> amended readings and lands both of LLP 0480 §14's carried pointed
> amendments (items 5 and 11).

**Type:** Spec
**Status:** Accepted
**Systems:** Contract (grammar/compiler/IR/runtime), contract-native, Kernel,
Protocol, Web/SSR, Router, Exact Native, Expose, Verification, Agent API,
Documentation
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-18
**Revised:** 2026-08-26 (**Amendment 8 appended** — the ActionSegments plan encoding, LLP 0535 AS-D4 = Option A (normalized global graph tables; Charlie Cheever 2026-08-25, recorded 0535 §8.5 @af0041d98): global `ActionSegments`/`AwaitSites` tables become the sole segment/await identity and graph authority; a global `ActionConcurrency` table places AS-D4’s cross-branch policy list; Actions gains segment-range/entry/concurrency refs; the three ride one required-where-used segment; boundary opcodes are execution hints that cannot define identity, and a plan whose hints disagree is refused. Content is derived from 0535 §§4.2/4.3/5.4 (the “AS-D4 must…” sentences), not from §8.5’s branch menu alone. Settles the one Option-A field that collides with the frozen body: the await **locus is not a row column** — both tables participate in the §3.5 loci segment by-table-class, preserving strip-invariance while meeting §4.3’s “may not lose their loci” through §3.5’s `(table, row)` floor. **Reserves and GATES the id space**: an `idReservations` list in `format-schema.json` holds segmentKind 27 + tables 88–90, and `generate-plan-format.mjs` refuses any schema allocating a reserved id while the reservation stands (deleting the list is itself a generation error) — the mechanism Amendment 2 announced and never landed. No ids allocated, no version move (target ≥ v1.7), no `.eplan` byte changes; the layout lands with AS-D3 = A’s single co-designed chunk, which is why allocation here would be its rejected Option B. Records as owed: LLP 0521’s graph-identity join, §5.3’s descriptor-purity proof, §5.4’s liveness mask + closed-value validation. Records unresolved: this document files rows 3/4/29/89/90 non-window while 0535 files row 75 window-bound — same class, opposite owner declarations. Claims nothing about row 75’s ledger disposition. 0506 D1(d) burn-down lane.) 2026-08-26 (**Amendment 7 appended** — the EPLC never-localized site classification, RFC 0536 §3.1’s named amendment: a string site is Localizable (default; a `localizable-strings` entry under a §3.3.15 stable id) or Never-localized (RFC 0536’s `text raw=` opt-out; no catalog entry in any locale, excluded from the extraction-completeness check by classification rather than absence, bytes staying in digest-covered `invariant-strings`). The id scheme and collision rule are unchanged — RFC 0536 defines no competing site scheme. Executes 0504 §3 row 52c’s 0485 half; Edition-2-paced, post-window. 0506 D1(d) burn-down lane.) 2026-08-26 (**Amendment 6 appended** — the virtual-visibility folds: §8.3 gains a region-level binding-emission gate under `hidden` rows with a semantic-prop exemption and a full re-emit on `hidden → prerender`, the `TransitionArtifact` carries the applied visibility mode and transition ordinal, `VirtualTopologyCertificateV1` is recorded as a producer-neutral content-addressed admission artifact reached by one path for all three producers, and 0543 [Open issue 1] — an optional `PlanAdmissionCertificate` proof row for the certificate body — is DECLINED for producer parity. Executes 0504 §3 rows 59, 66 (this spec's half) and 73 under the decided-content rule; 0543 §17.3 O-7 (the `TargetSiteId` canonical field set) remains owed. Delivery is post-window per RFC 0540 §8.6. Executed by the 0506 D1(d) burn-down lane.) 2026-08-25 (**Amendment 5 appended** — the §3.3.14 evaluator retarget: the referenced motion evaluator is `exact-motion` (worklet runtime = interim implementation) and the S-code→S-data paved path for compiled artifacts is added; executes Accepted RFC 0492 §8's explicitly named one-sentence amendment under the decided-content rule, discharging 0504 §3 row 5's owed residue. In-container graph bytes stay out (row 5(b)); the ceiling stays 0518 Ruling B (Amendment 3). Executed by the 0506 D1(d) burn-down lane.) 2026-08-25 (**Amendment 4 appended** — the enum-roster digest, format v1.6: the plan header binds the enum id space (`enumRosterDigest` at offset 80) the way it already binds the opcode table; mismatch refuses `enum-roster-digest` naming both digests, pre-1.6 plans refuse `enum-roster-absent` at the reader floor; minor + minReader move to v1.6 together per the v1.5 precedent; corpus and committed plan artifacts regenerated in lockstep. Motivated by the Track R roster-renumbering skew (issues/closed/20260825-track-r-plan-corpus-skew-red-on-main.md); closes issues/20260825-plan-enum-roster-digest-fail-open.md.) 2026-08-25 (**Amendment 3 appended** — the island certificate activation at RFC 0518's adoption event (Rulings A + B adopted verbatim by Charlie Cheever, 2026-08-25, via exact-9e, through the LLP 0508 §14 ratification): Amendment 2's reserved `island` entry class and `island-boundary`/`island-region` row kinds bind to the adopted 0518 §§4–6 semantics; island count/per-lane cost join §11.3 as claimable axes; Lane 1's budget source is the registered `wasm-embedding-ts-seam-benchmark` gate; admission stays policy-fail-closed with an empty initial whitelist (0518 OQ2: nothing); the §7 policy set stays deferred. Discharges 0504 §3 rows 9/24 per Amendment 2's closing sentence.) 2026-08-22 (**Amendment 2 appended** — the row-24 island certificate/format reservation: reserved entry class + row kinds, fail-closed until 0518's adoption event; no semantics adopted.) 2026-08-20 (wave-delta fold, three-family review of the
amendment's bytes: item 1 gains 0483 §2's omitted priced-costs cutover
conjunct; item 7 corrected to the decided definition-site rule (no
composition-site rejection — the first landing inverted the locus);
item 8's job→relation mapping corrected per 0489 §2.1 (publication →
`PublicationEquivalenceV1`; callback and replay → both
`CanonicalValueEquivalenceV1`; source equality separate); item 6
restored to the ticket's full strength (diagnostic **and** priced
migration) and re-pointed §7.2→§6.3; items 2/3 re-pointed §7.1→§7.4
and §6.1→§5.1; item 10's "cell identity" wording corrected to the
match-key/reuse-gate distinction; the preamble's provenance warrant
corrected — items 7–10 are ticket/ruling-sourced, not site-annotated.)
2026-08-20 (**Amendment 1 appended — the decided-program
alignment**: eleven items — ten site-annotated plus ticket-sourced
readings — converted into formal normative
amended readings (web runner = wasm engine; inbox declined; Option-only
absence; one merged ratification; no distinct Mode-1 runner with
amended-D1 matching; the false-SCC withdrawal; derived callback
tolerance; the three-way equality split; component-call in the composed
inventory; §6.3 as the subscriber key-shift trigger; the Track R
start-gate as exercised), each citing its
governing decision and its erratum ticket where filed; frozen body
unchanged, amendment governs on disagreement.)
2026-08-20 (clock-join annotations only — frozen text
unchanged: an execution-posture banner plus dated `DECIDED`/`ERRATUM`
notes at the ten sites the 2026-08-20 holistic corpus reviews cited as
stale against the LLP 0505 r3 docket, the RFC 0498 r6 trio, RFC 0483
§2, RFC 0482 r6, LLP 0489 r6, LLP 0500 D1-as-amended, and the recorded
0480 §12.2 adjudication; the false-SCC erratum annotated with its
ticket.)
2026-08-18 (initial draft). 2026-08-19 (r2 after super-refine
round 1, 2/2 NOT READY: certificate rebuilt fail-closed over the real
context taxonomy with census channel, widening axis, three-state
capability status; phases split into three gated tracks per 0480 §12.2's
verdict rule; value semantics named as 0330 amendment A-VAL; format
re-founded on a generated schema authority + digest manifest; identity
split four ways; one cursor rule; epoch-keyed speculation; tier-2
restated parallel-evaluation/serial-observable with an async join;
regions/events/tasks completed against the real IR; Semantics table
derived from the AX authorities; PE confidentiality lattice; style
lowering added as a Track W gate). 2026-08-19 (r3 after super-refine
round 2, 2/2 NOT READY: the web plan-runner and all parallel tiers
moved into Track R's F2/D1/W6 gate — Track W reduced to
representation-independent surface; every A-VAL consumer given a
reject branch and change 3 held until A-VAL is decided; journals
restated as finest-address patches; RNG restated as a per-event
substream and admission order given per-source ordinals — both listed,
with per-invocation command release, as proposed 0482 deltas; §0
re-pinned to 0330-as-written and the proposed-amendment inventory
completed; the undefined-elimination clause named inside change 1;
`mutates` restored to its shipped provider-mutation meaning with
compiler-derived stateReads added; graphics/motion reclassified as a
proposed sub-program certificate class; cursors given a table and
rebinding step; publish given its full effect row; coverage split into
three numerators with a proposed D1 witness predicate; HMR identity
reduced to the declaration tuple with the fingerprint as a
compatibility operand; strings re-addressed by id with catalogs as
auxiliary objects; slot confidentiality added to the Slots schema with
client-resumption/PE/server projections; the speculation key widened
to full dispatch identity; the zero-allocation claim demoted to a
reasoned target; the Phase-0 context authority made a hand-published
versioned artifact, breaking the format-schema circularity). 2026-08-19
(r4, author-authorized loop extension, closing the round-3 union: the
A-VAL `mutates` leftover deleted; publish gains heartbeat and pager its
full record, `async` the shared arm set, with IR-to-plan field coverage
a generated gate; RNG and the inbox model re-owned as proposed LLP 0330
amendments with reject branches; the 0482 deltas held with a
dormant-tier reject branch and conflict grain pinned to 0482's slot
bitmasks; Class S completed into clause (v), the tier matrix, and the
tierExecutable numerator, with atomic requirement rows; the
confidentiality lattice made a computed total join; PE re-derivability
re-founded on pure or version-witnessed reads; the cursor given
declared key identity and unbound/bound/missing states; cancellation
split from LLP 0330 §11.1 effect arbitration and evidence; the
TransitionArtifact unifying rollback/worker/speculation/replay commits;
tokenSchemaDigest and catalog-generation invalidation wired; `form`
re-pointed at llp/contract/0091; the peek opcode deleted; fail-closed
minor versioning; numeric identity scoped to pinned implementations).
2026-08-19 (r5 after round 4 — grok NOT READY (5) counted, codex
tainted-advisory: §3.4's surviving "ignore unknowns" and §3.3's
classifier-generation sentences deleted; the `form` spelling restated
as 0485's PROPOSED resolution of 0091's open notation decision
(inventory item (d)) with a component-descriptor reject branch; the PE
envelope given an explicit membership rule (successful controls ∪
opted-in client-class slots; "page-visible/never disclosure"
withdrawn); cursor rebinding added as §8.3 step 2a, generated from the
Deps source-kind table; Modules/Components/Entrypoints tables added
with interface/callsite records and the four-way IR disposition
ledger widened to ContractFileIR/ComponentIR; OOB dynamic indexing
made option-shaped under change 1; the digest preimage extended to
minReaderVersion + directory flags with a ValidatedPlan admission
pass, catalogs removed from plan segments; a merged cross-family
timer key, high-water drain cutoff, and numeric source ids; the
ArenaSnapshot lease protocol replacing "free under A-VAL"; structural
class-2 admission and the qualitative D1 witness half, stated as an
explicit 0480 §12.2 delta; F6 bound to the repo's REAL-input rule;
plus the verified minors — magic bytes, UTF-16 loci units, canonical
key universe, interval-on-§4, purity taxonomy ownership, catalog-swap
events, await-assign read gap, uniform change flags, draft-registry
promotion timing). 2026-08-19 (r6 after round 5 — 2/2 NOT READY, codex
clean: the digest re-founded on a canonical projection with
auxiliary-class segments (loci + baked first frame), making
strip/inline/rebundle and theme variants provably identity-neutral;
the journal made a forward+inverse delta `(address, before, after)`;
Deps gains guard/cell-initializer/cursor-resolution target kinds and
the runtime instance-subscription overlay, with §8.3 step 2a covering
bound-row column writes; fan-out split schematic-vs-live symbolic;
declaration identity gains owner kind + behavior use-site (matching
`instance.ts`); task fencing extended to every continuation resume and
new attempt, with task env/RNG snapshots and ambient masks; the PE
POST split into untrusted FormData + a server-emitted signed
preservation capsule, progressive flows requiring 0091's routed
target, anti-replay vs idempotency distinguished; F1 arithmetic
repaired — pending migration counts at underlying class, the widening
predicate made concrete (zero unbounded widened sites), dual verdicts
until 0480 adopts; Class S fallback reconciled with the structural
class-2 rule and §4.4; plus the verified minors — "free" leftovers,
catalog wording and view-only localized references, interval
de-triggered, publish() as capability-call, F6 REAL-input scoped per
stack, publish-id correction, busy as host prop, §4.1 tense, 0330 §2
bullet enumeration, quiesce clause, source-id generated authority,
0085 clause-kind inventory entry, W3.1 collision and
experimental-web-runner notes, protocol-conditional-on-form).
2026-08-19 (r7 after round 6 — 2/2 NOT READY, grok at 2 MATERIAL: the
digest preimage now specified once (§3.4's canonical projection; the
header row and §3.5 point at it); the journal restated as ordered
per-write `(address, before, after)` entries — forward in execution
order, reverse rollback, coalescing only under schema-defined
normalization; key cursors subscribe to the key-set generation in all
three states with deltas computed before row reclamation and
bound→missing before disposal, plus explicit key binders and
independent duplicate-key rejection; event source identity made a
canonical composite key with a synchronously allocated start-attempt
ordinal; F1 de-circularized — Phase 0 gains I6 (proof-only validator
designs), the final I2 receipt requires I5+I6, class-1 rows name their
proving validator and digest, missing proof is class 5; Class S split
S-data/S-code — current Motion worklets are generated JS source and
can never ride the class-2 fallback, classifying 4-or-5 under the
governing verdict; the PE capsule protocol filed as inventory item (f)
(proposed 0348/0335/0060 amendments) with anti-replay nonce and
idempotency-key-with-request-digest semantics separated and bound; plus
the minors — recursive IR-ledger closure, O(1)/lease/five/free/
priorValue leftovers, busy's three-source join, 0330-OOB citation
softened, Components-range dispositions, §3.3 tense, Track L gated on
the governing 0480-as-written verdict, Phase-0 phrasing corrected).
2026-08-19 (r8 after round 7 — grok 1 MATERIAL, codex 2, one shared:
the source-identity namespace paragraph deleted in favor of ONE
composite key with per-class fill rules and a named
`TaskSourceIdentity` used by inbox/replay/cancellation/RNG alike, with
the overlapping-manual-starts fixture; RFC 0067 added to the governing
set, inventory item (f), and Track W's gate with the 0060/0067
responsibility split quoted from 0060's own scope line; plus the
verified minors — §3.3 future voice, A-VAL reject-branch re-priced
against the real encoding, the renamed-prop join rule named, citation
drift, §7.6 trigger tense, the immutable pre-write journal snapshot
rule, localization softening, speculation's runner-state snapshot and
id reservation, host-prop-without-protocol-field dispositions, I6
scoped as a conservative projection). 2026-08-19 (r9 after round 8 —
grok READY, codex 2 MATERIAL, both honored rather than ruled nitpicks:
the `InvocationTargetStamp` added — target liveness captured at
admission, checked at the three fence points, generation mismatch
SUPPRESSING as replay-recorded `stale-target`, with
removal/replacement/HMR fixtures including the tier-2 slot-disjoint
case, filed into the proposed 0330 §4 amendment; §12.4.6 gains the
full atomic replay state machine as in-document owner-amendment text —
authenticate-first ordering, overlap rejection, cache-before-nonce
retry semantics, single-flight concurrency, 422 minting fresh
nonce/key, multipart obligations carried; plus the seven minors —
TimerSourceIdentity and TaskCancellationStamp fill rules, cleanup-order
citation to 0330 §11.1, interim/final receipt discriminator with
conditionally-required opcodeTableDigest, router-v2 0060/0067 in
Related, Track W design-vs-ship gate split, the RNG bit-derivation
named as pre-implementation owed text). 2026-08-19 (r10 after round 9
— grok READY (2nd), codex 2 MATERIAL, both honored: the
InvocationTargetStamp minted at event PRODUCTION by the published host
binding (fail-closed when no exact generation exists), closing the
above-the-cutoff remove-and-recreate race, with its fixture; §12.4.6's
anti-replay ledger made its own store with two explicit deployment
guarantees (shared durable atomic ledger, or issuer-bound capsules
failing closed after restart/replica), live tombstones never evicted,
capacity rejecting instead of evicting, lost outcomes returning
indeterminate rather than re-executing, crash points enumerated,
same-flight waiters receiving terminal failures, 422 successors minted
once and stored, the lookup namespace subsuming RFC 0067's, and a
dedicated registered `progressive-replay-conformance` check; plus the
minors — keyed-region collection/key expr refs in the Regions row, the
loci row set made total by table-class rule). 2026-08-19 (r11 after
round 10 — grok 1 MATERIAL, codex 3, all four honored: `TargetSiteId`
made a discriminated source-independent identity with
per-producer-class minting fill rules and per-family stale fixtures;
replay step 4 restated as a TOTAL function — every
consumed-nonce-without-retained-outcome state (true failure, 0067
TTL/capacity, restart, crash) is indeterminate-never-re-execute, filed
as a named 0067 amendment with 500-successor credentials; shared mode
re-founded on ONE atomic replay-coordination record (or issuer-affine
routing), with "never re-execution" restricted to mode (a) and
effectful PE flows requiring it; A-VAL gains `PublicationEquivalenceV1`
— aggregates always publish unless memo-stabilized, handle identity
never observable; plus the minors — callback-ref Types arm, the §8.1
cell-arms table row, F7 per-strategy subreceipts, capability
disclosure field and the opt-in marking's 0091 authority, DOM-intake
vs inbox dispatch disambiguation, tense slips, identity.ts wording,
the 0060-header/0062 staleness note). 2026-08-19 (r12 after round 11
— caps exhausted mid-adjudication; corrective revision under standing
editor authority, review pending the author's cap decision: effectful
successor credentials confined to proven-non-execution cases (422,
idempotent/pure) with a durable business-operation identity blocking
successors on indeterminate records and a provider-invocation-count=1
fixture; the speculation key and web-resumption tuple respelled on
TargetSiteId/host-handler variant; PublicationEquivalenceV1 made total
and type-directed over sums and callback refs without collapsing
field-granular dirtying; §8.3's "changed" bound to the rule; the §4.2
OOB arm confined to the change-1 reject branch; "never competes"
restated as reconciliation-by-named-amendment; inventory (d) gains the
envelope opt-in as a second proposed 0091 amendment; issuer-affine
routing required to carry the same durable record; cursor key notation
unified; F2 tense fixed). 2026-08-19 (r13 under the author's
disposition A — the round-12 union: the `BusinessOperationId` given
the TargetSiteId treatment — minted at first reservation as
(principal, action, formId, server-issued operation id), embedded in
the capsule, re-embedded by later renders while indeterminate, and
added as step 4's fourth axis so a fresh attempt key or fresh-GET
capsule cannot re-run the provider, with the record storing executable
422 successors and a NON-executable reconciliation handle for
500/indeterminate; tier-2 per-run admission bound in §13.3 — symbolic
work metric, certificate cutoff with strict boundary, a serial inline
path performing no snapshot/dispatch/join, predicate-recorded replay
metadata, and cutoff±1 + singleton fixtures; plus the round-12 minors —
value-kind and string-enum publication rows with recursive callback-env
comparison, the 0091 opt-in field tense, keyExprId wording, heartbeat
on 0330 §4's timer family with a source identity, hierarchical
invalidation stated, handler-prelude evaluation pinned to event
intake, 0330 §11.2's Object.is-faithful sentence named in the A-VAL
amendment, and fresh-equal aggregate resource churn priced with the
memo-stabilization recommendation). 2026-08-19 (r14, the landing
revision — round 13: grok READY (3rd, the counted verdict), codex
self-tainted advisory whose findings are adopted on their merits:
operation issuance made step 0 (render-time atomic mint-and-persist
before capsule signing), the one-unresolved-operation-per-namespace
recovery model with a resume handle and non-optional formId for
effectful PE (a further 0091 amendment), step 4's tuple stated once
with BusinessOperationId as a first-class axis; tier-2 admission
re-founded on a reducer-evaluation cost model with run.length ≥ 2,
width/skew-aware certificates, the uniform-shapes v1, and the
above-cutoff-singleton fixture; callback identity gains the target
runtime instance; cursor rebinding pinned inside the topological
pass; the cutoff vector's collection order defined; the Track W gate
deliverable named and ownered; one tense fix — and MATURITY MARKERS on
the two proposal-stage seams (PE replay; tier-2 dispatch), stating
what their gating already enforces: not implementation inputs until
their owners adopt)
**Related:** LLP 0480 (The Plan Is the Program — the umbrella RFC this spec
implements; 0480 §12 owns the falsifier matrix and §14 the decisions),
LLP 0481 (Five Semantic Tightenings — the language program §§5–9 here make
normative), LLP 0482 (Parallelism tiers — §13 here is its admission spec),
LLP 0483 (Web on the flat plan — §12.4 here is its runner spec), LLP 0484
(Canon and target state — §16 here is the knowledge-layer interface),
LLP 0478 (Contract at Full Speed — decision owner for W1–W6; §15.5 here
enumerates the interface), LLP 0479 (Two Lowerings and One Plan), LLP 0330
(Contract semantics spec v0 — Draft; every §5–§9 change lands as an
amendment to it), LLP 0328 (native execution tiers), LLP 0185 / LLP 0186
(compile-to-closures, signal scheduler — invariants preserved by §12.2),
LLP 0188 (static semantic graph), LLP 0184 (`match`), LLP 0154 / LLP 0160 /
LLP 0288 (routes, Contract-first, web target), LLP 0297 (threading contract —
binding throughout), LLP 0307 (precomputed first frame — §12.5 unifies with
its Phase B), LLP 0166 / LLP 0387 (editability substrate and sidecar —
§12.1.3), LLP 0099 (motion — §3.3.14), LLP 0330 §10 (deadline queue —
retained exactly), LLP 0442 (determinism as a system property), LLP 0331 /
LLP 0332 / LLP 0333 (Exact Native; §5.1 builds on 0333 §5.1's
`ClosedStateType`), LLP 0351 (server-generation map), LLP 0365 (the Guide),
LLP 0361 (filesystem tickets), `llp/contract/0082` / `0085` / `0086` /
`0087` (Contract runtime/verification/HMR/expression authorities this spec
amends or consumes), `llp/contract/0091` (forms/validation — the form-IR
authority §12.4.6 composes with, alongside LLP 0348/0335 and
`llp/router-v2/0060`/`0067` — key transport vs idempotency-replay
lifecycle), `tests/protocol/protocol-inventory.json` (op protocol
authority — unchanged by this spec) 2026-08-19 (Accepted by Charlie Cheever; Phase 0 I1-I6 landed with F1 accepted and F2 adjudicated pass — docs/reports/llp0485-0480-adjudications-20260819.md).

## Summary

LLP 0480–0483 derive a destination: the deployable Contract artifact becomes
a flat, position-independent, `mmap`-able **plan** — relational tables that
precompiled loops index into, never a pointer graph that engines walk — made
possible by five semantic tightenings plus a sixth closure (a typed
capability ABI at the world boundary). LLP 0484 adds the knowledge-layer
program that keeps the documentation of such a system current. Those five
documents are deliberately *derivations*: they argue direction, register
falsifiers, and stop short of construction detail.

This document is the construction detail: the single normative
implementation specification, written so a team (or agent fleet) could
build without re-deriving any decision the cluster argued. It specifies:

- the **plan container format** — segments, tables, encoding, digests,
  loci sidecar, semantics (accessibility) projection, localization split
  (§3);
- the **expression VM** and generated **opcode authority** that keep the
  execution modes and web runner from diverging (§4);
- the five semantic changes as **normative language rules**, with
  concrete resolutions of 0481's open questions — dependency granularity,
  the `cursor` plus budget-enforced widening, guarded resource edges,
  journaled rollback over value semantics, the task event contract, the
  paved multi-step form (§§5–9);
- the **capability/effect ABI** (§10);
- the **`PlanAdmissionCertificate`** — the F1 instrument — constructive
  class 5, per-tier admissibility, coverage as a verdict axis (§11);
- the four **runners** — dev, production, AOT, web — with plan-directed
  resumption, HMR state migration, and bake-and-patch (§12);
- **parallelism admission** as testable rules (§13);
- the **verification program** binding every 0480 §12 falsifier to a
  registered check with a receipt schema (§14);
- an **instrument-first program**: Phase 0 starts now; three
  **independently gated tracks** — language, representation,
  web/parallel — start only on their named verdicts (§15), preserving
  0480 §12.2's rule that the language changes survive an F2 failure.

**Decision posture, stated up front.** Nothing in this spec adopts LLP 0480
D1, amends RFC 0478's program, or changes any shipped default. 0480 §12.2's
verdict rule stands: F1 and F2 report, Charlie adjudicates, and only then
does the representation track exist.

> **DECIDED since freeze (2026-08-20):** the adjudication has happened —
> D1 was adopted with the W6 go (0480 §12.2's recorded verdict,
> 2026-08-20 adjudication record), and Track R is running under the
> exercised gate 0480 §12.2 records (instrument-acceptance + F2 pass +
> author go; F1 accepted as instrument with the residue dispositions
> owned by RFC 0478 D12). This paragraph and §1.3's "MUST NOT begin"
> sentence describe the pre-adjudication posture. Phase 0 **adopts no architectural
destination and flips no shipped default** (its one shipping-code item,
I3, is an independently owed conformance repair — "instruments, not the
system" was false as a blanket). Where this spec resolves a question the
cluster left open, the
resolution is a **specification decision subject to the same gates** — it
makes the model buildable and falsifiable; it does not pre-empt the
adjudication. Every amendment this spec carries to a document it does not
own is **proposed, never silently taken**, and the full inventory is:
(a) to **LLP 0330** — the five semantic changes of §§5–9 (change 3
breaking `llp/contract/0082`/`0087` and answering 0330 A4; change 1
including the §5.1 undefined-elimination clause; the value-semantics
amendment **A-VAL** with its array/object-`==` and function-in-state
clauses, §5.1/§7.5), **plus the per-event RNG substream (§7.1 — 0330 §5
and §11.2 own the RNG) and the inbox admission model against 0330 §4's
synchronous host-event dispatch (§7.4)** — all text-in-waiting for W2's
ratification owner; (b) to **LLP 0480 §12.2** — the
census/pending-migration channel, the widening axis, the
referenced-sub-program class, and the §11.3 witness-predicate proposal,
put to that document's adjudicator; (c) to **LLP 0482 §3.2/§3.3** —
per-invocation command release under the serial-observation model
(§13.3) and per-source ordinals in the admission order (§7.4); (d) to
**`llp/contract/0091`** — the built-in `form` tag as the resolution of
its explicitly open spelling/notation decision, AND the proposed
slot-level envelope opt-in field on its semantic form record (§12.4.6;
the current record has no such field); (e) to
**`llp/contract/0085`** — §9.2's NEW clause kinds (`fanout` / `cost` /
`plansize`) as proposed clause-vocabulary additions (§9.1's
compile-time discharge of *existing* clauses is additive — the live
verification layers run unchanged — but new clause kinds are 0085
surface and are filed as text-in-waiting); (f) to **LLP 0348, LLP 0335, LLP 0060, and RFC 0067** — the entire
§12.4.6 preservation-capsule protocol is proposed text pending those
owners, with the responsibility split 0060's own normative-scope line
draws (that header's PE-transport pointer to RFC 0062 predates 0091's
live 0348/0335 split — 0091 governs; noted so nobody bounces off
0062): 0348 owns the capsule-vs-`_exactPreserveField` transport, 0335
the HTTP/security rules, **0060 the `ActionResult`/`formId`/key
transport, and RFC 0067 the receipt contents and idempotency replay
lifecycle** (retained outcomes, canonical signatures, conflict
behavior, TTL/capacity, durability — §12.4.6's retained-outcome
semantics reconcile with 0067's, never compete). Reject branch = the
current 0348 transport or no PE claim. Until each is adopted by its owner,
the owning document stands as written, this spec's delta is pending,
**and every pending delta carries a stated reject branch** (§7.1, §7.4,
§7.5, §12.4.6, §13.3) — pending text is never the specified v1.

**Status of numbers.** This spec inherits RFC 0478 §P7's discipline. Every
latency, size, or coverage number herein is either (a) a **measured** figure
with a named receipt, (b) a **registered bar** proposed to an adjudicator
before capture, or (c) a **reasoned target** explicitly labeled as such. No
reasoned target is promotable.

---

## 0. Conformance language

The key words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and
**MAY** are used per RFC 2119. Sections are normative unless marked
*(informative)*. Where this spec and a governing authority disagree, the
authority wins and the disagreement is a defect in this spec:
`tests/protocol/protocol-inventory.json` for op codes,
`exact-contracts.json` for boundary authorities, **LLP 0330 as written**
for Contract semantics — this spec's pending amendment texts bind only
after 0330's owner adopts them — and RFC 0478 for program decisions
W1–W6.

## 1. Scope and gating

### 1.1 What this spec owns

- The plan container format and its versioning (§3).
- The opcode authority file and the generated-lowering invariant (§4).
- The normative statement of semantic changes 1–5 as LLP 0330 amendment
  text-in-waiting (§§5–9) — each lands as an amendment to 0330, per
  LLP 0481 §8, never as a fork.
- The capability-ABI declaration format (§10).
- The `PlanAdmissionCertificate` format and classifier rules (§11).
- Runner behavior required for conformance (§12), parallel-dispatch
  admission (§13), and the falsifier harness (§14).
- The phase plan and its gates (§15).

### 1.2 What this spec does not decide

- **D1 adoption** (the representation as stated destination) — 0480 §14,
  Charlie, after F1/F2.
- **Any RFC 0478 workstream decision** — the six asks 0480 §10 enumerates
  (W6 promotion, the W2/0330 timing, the W4a precondition, the W3.1 pause,
  the W4a-design caution, the web-speed-mechanism ask) remain asks; §15.5
  tracks them as external dependencies.
- **Mode 3 go/no-go** (§12.3) — specified so it can be built; whether it
  earns its place is 0480 §15.6's open question, decided after mode 2
  exists.
- **Cross-application plan-page sharing** — out of v1 entirely (§3.8);
  blocked on the integrity/provenance model 0480 §15.13 demands.
- **The tier-5 placement protocol** — sketched as future work (§13.6);
  LLP 0482 §6's owed list stands.
- **Timer bucket alignment** — not adopted. LLP 0330 §10's exact-deadline
  guarantee is retained verbatim (0480 §15.7 left the question open;
  the non-adoption is this spec's resolution, not 0480's).

### 1.3 Track gating (overview; full plan in §15)

Phase 0 plus three **independently gated tracks** — the structure 0480
§12.2's own verdict rule requires (F1 pass + F2 fail explicitly lets the
language changes proceed on the TS tier; a plan that chains them would
silently repeal that rule):

| Unit | Contents | Gate to start |
| --- | --- | --- |
| Phase 0 | Instruments plus one independently owed conformance repair: ABI declaration format, executable-context authority, admission-certificate tool, conformance re-green (product repair, §15.1), flagged flat-plan spike, opcode extraction | none |
| Track L (language) | The five semantic amendments to LLP 0330, sequenced 1→4→2→3→5, on the engines that exist | F1 pass **under the governing 0480-as-written verdict** (§11.3 emits dual verdicts; the stricter proposed witness binds nothing until 0480 adopts it); per-change paved path exists. **Proceeds on the TS tier even if F2 fails** (0480 §12.2) |
| Track R (representation) | Plan format v1; mode-2 native runner behind flag; **the web runner — the same plan bytes on a JS host, with resumption and the SSR partition**; dual-run equivalence; speculation; tier-2 dispatch (additionally gated on Track L changes 1–3, F7, and 0482's owner deciding §13.3's held deltas — tier 2 needs totality, not the annex); tier-3/3b offload (additionally requiring RFC 0478's owner to take the W6-research-annex decision, which 0482 scopes to tiers 3/3b — F7 registration is not that decision) | F1 + F2 pass; D1 adopted (0480 §12.2 verdict rule); **and the W6 go from RFC 0478's owner** (0478 §5 gates W6-shaped investment on W4a; 0480 D3's ask to amend that sits with 0478 — until decided, 0478's gate binds). An F2 miss kills the plan bytes everywhere they run, web included — Track R's web half dies with its native half (LLP 0483 §2's runner consumes the representation) |
| Track W (web authoring surface) | `form` tag, PE classifier + server-replay design, style lowering — representation-independent compiler/language work | **Design** starts on: Track L changes 1–4; the §12.4.7 style-lowering design. **PE implementation lands/ships** only after the 0091/0348/0335/0060/0067 owner amendments are adopted (§12.4.6) |

A track MUST NOT begin while its gate is open. Within a track, work items
are parallelizable unless §15 orders them. RFC 0478's W1–W5 continue on
their own clocks throughout; **this spec pauses nothing** — the only
pause anywhere in the cluster is 0480 §10.4's W3.1 recommendation, owned
there.

> **DECIDED since freeze (2026-08-20, LLP 0505 r3):** three rows of this
> table are superseded. Track R's "web runner — the same plan bytes on a
> JS host" is dead: the decided web runner is the **same Rust engine via
> wasm** (RFC 0483 §2 = (b), 0505 row 1, gated); no JS-host runner is
> built. Track R's tier-2 dispatch item is **replay-only for v1** and
> tier-5 is **parked** (0505 row 8; RFC 0482 r6). And the W3.1 pause
> **was taken** (RFC 0478 D11, 0505 row 2(d)) — "this spec pauses
> nothing" is historical.

## 2. Definitions

- **Plan** — the contiguous, position-independent artifact of §3; the unit
  of deployment for a Contract module or route.
- **Schema-total / instance-parametric** — every *schematic* fact (opcodes,
  templates, node sites, dependency edges, region kinds, effect signatures,
  types, byte layouts) is resolved at build time; *instances* (active
  branches, keys, cardinalities, payload sizes) remain runtime-parametric
  (LLP 0481 §5.1).
- **Executable context** — any site whose evaluation runs program text.
  The enumeration's authority is `plan/executable-contexts.json` — a
  **Phase-0, machine-readable, versioned inventory, hand-published at I2
  and generated thereafter** (present tense about generation was r2's
  error: TypeScript's `ReplayExecutableContextKindV1` union is 21
  hand-authored, type-erased members and cannot attest completeness).
  Its v1 content: the shipped 21-kind replay taxonomy
  (`replay-certificate.ts:17–38` — prop/param defaults, host bindings,
  state initializers, derives, cells, ephemerals, actions, tasks,
  behavior-scoped kinds, snippets, view, window, key commands, contract
  monitors) **plus per-site classes the replay enum does not carry**:
  binding expressions, event-payload/handler-prelude expressions, and
  region predicates, enumerated by an IR walk. The TS union, site
  walkers, and certificate schema will regenerate from this authority
  once I2 lands (hand-published until then); the
  later format schema (§3.3) *consumes* it, never generates it — that
  ordering is what breaks r2's circularity. An unrecognized context kind
  **fails closed** (§11.2).
- **Target tier** — the deployment class a route is built for. This spec
  defines four: `native-total` (no-JS native: Exact Native roots, Expose
  surfaces), `native-mixed` (native with a JS host present), `web` (the
  LLP 0483 runner over real DOM), `server` (evaluation on a server, ops or
  HTML shipped). A route names exactly one primary tier per build
  configuration; a build MAY produce several configurations.
- **Runner** — an engine that executes plans: mode 1 (dev), mode 2 (fixed
  production runner), mode 3 (AOT-generated code), web (a JS-hosted
  mode-2-equivalent). Conformance identity across runners is §14.1's
  subject.
- **Transition** — one reducer application: `(state, event, envSnapshot) →
  (state′, Command[])` (§7.1).
- **Certificate** — the `PlanAdmissionCertificate` of §11.

## 3. The plan container format

### 3.1 Container and segments

A plan file is a single contiguous buffer:

```
[Header]
[Segment directory]
[Segments...]
```

**Header** (fixed size, little-endian):

| Field | Type | Meaning |
| --- | --- | --- |
| magic | 4 bytes | the literal bytes `'E' 'P' 'L' 'N'` (a byte string, not a little-endian integer — r4's u32 form would have serialized as "NLPE") |
| formatVersion | u16.u16 | major.minor (§3.4) |
| minReaderVersion | u16.u16 | oldest reader version that may load this plan (fail-closed minor compatibility, §3.1) |
| flags | u32 | reserved, MUST be zero (auxiliary-class presence lives in the segment directory's per-entry flags, §3.4) |
| planDigest | 32 bytes | SHA-256 per §3.4's canonical projection — **the preimage is specified once, there, and nowhere else** (r6 restated it here and the copies disagreed) |
| opcodeTableDigest | 32 bytes | digest of the opcode authority the plan was compiled against (§4.1) |
| enumRosterDigest | 32 bytes | **Amendment 4 (2026-08-25, format v1.6):** digest of the enum id space (the generated enum-named-values model) the plan was encoded under; mismatch and absence both refuse fail-closed (Amendment 4) |
| segmentCount | u32 | |

**Segment directory:** `segmentCount` entries of
`(segmentKind: u16, offset: u64, length: u64, flags: u16)`, offsets
relative to buffer start, each segment 8-byte aligned. Unknown segment
kinds MAY be skipped **only when their directory `flags` mark them
optional**; an unknown segment marked required is a load error, and the
header additionally carries a `minReaderVersion` — so a minor revision
can add semantically load-bearing data without an old runner silently
half-reading the plan (r3's skip-all-unknowns rule was not fail-closed).
Known-but-absent required segments are a load error.

> **DECIDED (2026-08-25, Amendment 4):** the header table above
> already includes the `enumRosterDigest` row Amendment 4 added
> (format v1.6, offset 80; headerSize 84 → 116, segmentCount and the
> segment directory relocated). The surrounding frozen prose was
> written for the v1.5 layout; where they disagree, Amendment 4 and
> `format-schema.json` govern.

### 3.2 Encoding rules

1. **Position independence.** No absolute pointers anywhere. All references
   are table indices (`u32`) or byte offsets relative to the containing
   segment. A plan MUST be usable directly from an `mmap`ed read-only
   mapping or a fetched `ArrayBuffer` with zero relocation.
2. **Endianness.** Little-endian throughout. (Every supported target —
   arm64, x86-64, wasm — is little-endian; a big-endian port would add a
   decode step, not a format change.)
3. **Alignment.** Every table's rows are naturally aligned for their widest
   field; tables begin at 8-byte boundaries.
4. **Density.** Tables are struct-of-arrays where a column is independently
   scanned (Bindings, Deps), array-of-structs where rows are consumed whole
   (Nodes, Regions). The choice per table is fixed by this spec, not per
   plan.
5. **Determinism.** Compiling the same source with the same compiler version
   and configuration MUST produce byte-identical plans (content addressing
   and §14's parity gates depend on it). Sort orders, interning orders, and
   id assignment are therefore specified (§3.3) rather than left to hash-map
   iteration.

### 3.3 Table catalog

Segment kinds and their normative content. **Field lists below are
normative content; the byte encoding will be owned by a generated
authority:** `packages/exact-contract/plan/format-schema.json` (a
Track R deliverable — no `exact-contracts.json` row exists today;
promotion per §16) will define
every table header, row count, column framing, stride, enum and
variable-length encoding, alignment, and per-column optionality. The
TS/Rust/web readers, format docs, golden byte fixtures, and §3.2 rule 5's
canonicalization (a total sort/interning order per table) **will be
generated from it**, and the same meta-schema generates §3.3.13's
semantics parity check — turning format and AX drift into compile-time
failures. One ownership rule stated bluntly, because r4 recreated r2's
circularity in exactly this sentence: **the format schema CONSUMES
`plan/executable-contexts.json` and the Phase-0 classifier mapping
table; it never generates them** (they are Phase-0 authorities, §2 and
§11.1; a Track R artifact cannot generate what F1 needs before Track R
exists). Once Track R lands, the format schema MAY parity-check them.
Minor-version compatibility is concrete AND fail-closed: a minor
revision may add new segments (directory-flagged optional, or required
with a raised `minReaderVersion`) or **framed optional columns**
(skippable by older readers); anything else is a major bump.

**3.3.1 Slots** — one row per state slot, arena-ordered.

| Field | Content |
| --- | --- |
| slotId | stable u32 (§3.3.16) |
| typeRef | index into Types |
| byteOffset | offset within the component's state struct |
| initKind | `const` \| `expr` \| `env` (env-dependent: §12.5's patch class) |
| initRef | **always an expr id** — constants are constant expressions in the Exprs pool (r3's "constant index" referenced a Constants table that did not exist; one pool, no second encoding) |
| ambientMask | bitmask of ambient inputs the initializer reads (clock, RNG, locale, scheme…) |
| confidentiality | `client` \| `server-only` \| `secret` — a **total ordered lattice** (`client < server-only < secret`): every derive, cell value, cursor, and handler prelude carries the **join (max) of the classes of everything it reads**, computed over the Deps graph, so a derive mixing `client` and `server-only` inputs is `server-only` — never `client` by default. Authored defaults per route tier (`client` on client-rendered tiers, `server-only` on server-resident) apply to *source* slots only; derived classes are always computed (§12.4.6) |

**3.3.2 Types** — the closed type table (§5.1): primitives, string-enums,
sum types (tag + per-arm layout), closed objects (field list with offsets),
arrays (element typeRef), value-kinds (dimension/color/duration), and
**action/callback reference types** (plan-addressable action ref +
effect-row fingerprint — §5.1's function-in-state restriction encoded).
Types are structural; identical structures MUST intern to one row.

**3.3.2a Modules, Components, and Entrypoints** — the closure r4 omitted
entirely: a plan claiming to be "the deployment unit for a module or
route" must encode what a module and a component *are*. Three tables:

- **Modules** — module id, import links (plan-module refs and §10.1
  capability refs — `ContractFileIR`'s import surface), export list
  (component refs, declared reactive exports per §6.4).
- **Components** — component id, name, **interface record** (ordered
  prop/param descriptors: name, typeRef, default expr id, effect-row
  fingerprint for callback props), state-struct descriptor (instance
  byte size + slot range into Slots — the "component's state struct"
  §3.3.1 offsets resolve against), root region ref, layout fingerprint
  (§3.4), behavior/snippet/window/key-command ranges (each range's row
  table is format-schema surface — stub catalog entries or an explicit
  `build-time-erased` disposition where a kind lowers into Regions; the
  recursive ledger enforces one or the other).
- **Entrypoints** — route/root id → component ref + boot descriptor
  (the registration LLP 0154/0331 route identity maps onto).

A Nodes "component ref" is an index into Components; a composition site
carries its **callsite record** (argument expr ids in interface order,
event-row wiring, children-slot mapping). **The IR-coverage gate (§8.1)
widens accordingly**: the generated disposition ledger runs over every
`ContractFileIR` and `ComponentIR` field as well as `ViewNodeIR` —
each classified `encoded | build-time-erased | referenced-subprogram |
rejected` — so an omitted concept fails the build rather than a review.
One named prerequisite: `assignStructuralIds` today deliberately skips
pager's synthesized cells (`identity.ts:191–195`; publish itself gets
its ordinary visit-time id — it is childless); extending the
structural-path grammar through them, with recursive node-count parity,
is a Track R work item that MUST land before pager/publish descendants
get loci, handler, or resumption identity claims.

**3.3.3 Nodes** — one row per view node site: nodeId (stable), tag (u8,
from `CONTRACT_TAG_LOWERING`'s closed set) or component ref, parent nodeId,
owning region id, binding range (start,count into Bindings), semantics ref
(§3.3.13), first-frame flags.

**3.3.4 Bindings** — SoA. Columns: target nodeId; target attr id (u16, from
the closed attr schema); expr id; dep-range back-reference. Event bindings
live in Handlers (§3.3.10), not here.

**3.3.5 Deps** — the build-time dependency DAG, stored topologically sorted
(one global order per plan). SoA columns: source ref (slot / field /
column / derive / cell / **cursor** / **catalog-generation** — the
localized-string invalidation channel, §3.6), target ref (derive /
binding / region predicate / handler prelude / **guard** /
**cell-initializer** / **cursor-resolution** — r5's four-target enum
could not encode the §6.3 guard rows or §6.2 rebinding it specified),
granularity descriptor (§6.1), optional guard expr id (§6.3).
**Schematic templates vs the instance overlay:** Deps rows are
immutable plan data; the runtime derives from them an
**instance-subscription overlay** keyed
`(collection instance, row key, column, cursor instance)` — the
inverse index runtime read-tracking used to *discover*, now computed
from declared templates. The overlay is what invalidates a bound
cursor's consumers when a field of its currently bound row changes,
and what §8.3 step 2a retargets — retargeting mutates the overlay,
never the plan. (Theme tokens need no special
source kind: the theme table is arena state, so token-reading bindings
carry ordinary slot edges and a theme swap propagates like any write.)
The per-source out-edge list is contiguous, so **schematic fan-out is a
row count** — schematic only: one static edge inside `each` is N live
consumers (0480 §6.4, 0481 §6.1), so **live fan-out is a symbolic
cardinality** (§9.2), never this count.

**3.3.6 Derives** — derive id, expr id, result typeRef, declared fan-out
budget ref (§9.2), memo slot offset.

**3.3.7 Regions** — one row per structural construct (§8.1): region kind,
parent region, node span (start,count in Nodes), kind-specific refs
(predicate/scrutinee expr id; arm templates; cell id; **for keyed
regions, the collection expr id and key expr id** (`EachNodeIR`'s
`iterable`/`key`), with Deps rows targeting both; instance-arena
descriptor — a build-time descriptor of the runtime instance table's row
layout, columns, and key type, encoded per the format schema; the
instance table itself is runtime state, never a plan segment; snippet
target).

**3.3.8 Actions** — action id, param typeRefs, body (opcode range),
composed effect-signature ref (§7.2 — writes, **stateReads**, mutation
identities, commands, calls), **lexically ordered effect-site records**
(declaration effect identity + site ordinal, so LLP 0330 §11's
`EffectAddressV1` vocabulary derives mechanically from the flat
encoding), declared ambient-read mask, cost-budget ref.

**3.3.9 Tasks** — task id; trigger, all four kinds: `mount`,
`manual`, reactive `when` (the three `TaskIR` carries today) plus the
**new** event-driven form (§7.7) this spec adds — and NOT a fifth
"interval" discriminant: interval surface syntax lowers onto LLP 0330
§4's `every`/`after` timer family dispatching a `manual` task start
(rearm/resume/disposal per that section; surface form in §18); typed capture/parameter refs (§7.7); cleanup registration
metadata (§7.6); body (opcode range over the task-op subset);
dispatchable action set; capability set with lexically ordered
effect-site records (as in §3.3.8); cancellation scope (owning
region/component).

**3.3.10 Handlers** — event bindings: nodeId, event kind (from the
language's actual closed set, `CONTRACT_SCHEMA_EVENT_ATTRS` in
`builtin-schema.ts` — the core `EVENT_ATTRS` unioned with the per-tag
input/image/lottie/rive/webview/video event sets), action id,
curried-argument expr ids
(**evaluated at event intake, before inbox enqueue, against live
instance scope** — the LLP 0483 §3 identity rule and the moment §7.4's
target stamp is minted; never at the later inbox dispatch, and argument
values are never baked), handler prelude dep range
(the reads §13.3 folds into the invocation signature).

**3.3.11 Cells** — resource/query/mutation declarations: cell id,
initializer expr id, guarded dep range (§6.3), settlement event types,
state-machine region back-refs.

**3.3.11a Cursors** — one row per `cursor` declaration (§6.2): cursor id,
collection ref, `at`-expression id, **`keyExprId`** (absent for ordinal
cursors), key typeRef, read target columns. The runtime cursor instance
(the `unbound/bound/missing` state, stored key `K` or ordinal) is an
arena descriptor like the keyed-instance tables; **rebinding** —
re-resolving on an `at`-dependency change or a membership change in the
unbound/missing states, and re-pointing the cursor's out-edges to the
newly resolved row — is a defined step of the §8.3 sweep, ordered by
the Deps `cursor`-source rows.

**3.3.12 Exprs** — the opcode pool: a single byte array of §4 opcodes plus
a per-expr index (offset, length, result typeRef). Shared subexpressions
MAY be interned.

**3.3.13 Semantics** — the accessibility/semantics projection (0480 §15.11
resolved: **a table beside Nodes**). The field schema is **derived from
the two existing authorities, never invented here**: the Contract host
AX-prop schema (`builtin-schema.ts` — label, labelled-by/described-by,
disabled/checked/expanded/selected/modal, numeric/text values, actions,
heading level — plus the AX renames `builtin-lowering.ts`'s
`CONTRACT_PROP_RENAMES` makes authorable — `busy` among them, whose
host attr, `AccessibilityBusy` protocol prop, and graphics
`semantics.busy` field become **one column via the parity map's join
key**, the rule for every renamed prop) joined
with the protocol inventory's AX fields (protocol-only: elements-hidden,
order, position-in-set/set-size, hint, synthetic; value min/max/now/text
already live in the host schema — the generated join, not this
parenthetical, is the authority). Per node: role; static-or-bound sources for
label/description/value; relationship node refs; state/trait flags;
live-region class; collection metadata; heading and focus-order hints.
The format schema will generate a **parity check** — every semantic host
prop and inventory AX field (hint and synthetic props included) maps
into exactly one Semantics column, **and a host prop with no same-named
protocol field carries an explicit disposition** (inverse lowering —
e.g. `accessibilityFor` → labelled-by on the target — or a declared
host-only role, or a proposed protocol addition; a column alone does
not prove native emission); the generated join, not this prose, is the
field authority. When Track R lands, this table becomes the
**logical source from which the runner emits the existing protocol AX
props** — the presenter and the op protocol are untouched (§17); what
changes is where the runner reads AX truth — and Acto's semantics
substrate. A
node absent here is AX-transparent **only where the compiler proves it
decorative** (no semantic props, no interactive bindings); otherwise
omission is a compile error. RFC 0478 E19's release-gating stands: no
Semantics projection, no tier-readiness claim on an AX-obligated host.

**3.3.14 Motion** — v1 resolution of 0480 §15.3: the plan **references**
motion programs, it does not subsume them. Rows bind SharedValue graphs,
gesture claims (by testId), and `transition=` descriptors to nodes; the
evaluator remains the separate LLP 0099 / 0297 worklet runtime
**[Amendment 5, 2026-08-25: retargeted — the referenced evaluator is
`exact-motion` (RFC 0492 §8's named narrow amendment), the worklet
runtime standing as interim implementation; the S-code→S-data paved
path for compiled artifacts is added. See Amendment 5.]**. Lowering
motion into plan opcodes is out of v1.

**3.3.15 Strings** — interned UTF-8, **referenced by stable string id,
never by raw byte offset in opcodes** (translations change byte lengths;
an offset-carrying opcode could not survive a catalog swap). The
`invariant-strings` segment (testIds, keys, tags — inside `planDigest`)
carries its own id→offset index; `localizable-strings` catalogs are
**auxiliary objects** mapping id→bytes per locale, outside `planDigest`,
addressed by the §3.4 manifest (`planDigest, locale, catalogDigest`).
The digest-covered core plan is its own file/mapping; catalogs and
production loci are separate objects — dev MAY bundle them, and the
bundle then carries its own container digest (§3.4's one-mapping sharing
claim applies to the core plan object, not to bundles).

**3.3.16 Identity — four identities, not one** (`identity.ts`'s own
header: structural ids are stable within **one source version**, pair
with `sourceVersion`, and cover view nodes, not slots):

1. **Declaration identity** (slots, derives, actions, tasks, cells):
   `(module id, owner, declaration kind, declaration name)`, where
   **owner** is the component — or, for a declaration inside a behavior
   template, the component *plus the behavior use-site instance name*:
   two named uses of one behavior template are distinct owners, exactly
   the shipped rule (`runtime/instance.ts`: behavior slot identity =
   component identity × instance name × slot name — r5's
   component-level tuple would have aliased them). A compatible
   spelling of `llp/contract/0082`/`0086`'s component-identity-plus-
   state-name rule, not a third rule; declaration order never defines
   identity; duplicate canonical identities are a compile error;
   `compiler/slots.ts` is the *catalog* precedent, not the identity
   authority. §14.1 carries a two-uses-of-one-behavior HMR migration
   fixture. The type fingerprint (LLP 0333 §5.1 discipline)
   is **not part of identity**: it is an operand of the versioned
   compatibility relation (`equal | compatible-widening | reset`,
   §12.1.2) — it cannot both name the slot and be allowed to change.
   Stable under unrelated edits by construction; HMR keys on it.
2. **Static node-site identity**: the plan-local Nodes index, plus the
   `assignStructuralIds` path **paired with `sourceVersion`** for edit
   targeting — NOT edit-stable, never used for state migration.
3. **Runtime instance identity**: node site + instance path (the `each`
   key chain).
4. **Resumable handler identity**: §12.4.3's tuple.

**Derived, never stored.** Every table is a build product of the IR —
`compiler/slots.ts`'s "derived, never stored" pattern (LLP 0166 §4.2)
applies to the whole plan: the plan is never hand-edited, never the
editable artifact (§12.1.3), and always regenerable from source.

### 3.4 Digests, content addressing, versioning

- **`planDigest`** is the plan's executable identity, computed over a
  **canonical digest projection**: domain-separated (a format tag
  prefix), then `formatVersion`, `minReaderVersion`,
  `opcodeTableDigest` **[Amendment 4, 2026-08-25: then
  `enumRosterDigest` — the v1.6 projection binds the roster digest
  immediately after the opcode digest]**, then — with every **auxiliary-class** segment's
  directory entry and bytes removed first — the remaining directory's
  `(segmentKind, flags)` vector and the ordered
  `(segmentKind, length, bytes)` sequence. Auxiliary-class segments
  (directory-flagged): `loci` and the baked first frame — identity-
  neutral by construction, so inlining, stripping, or re-bundling them
  provably preserves `planDigest`, and theme/locale first-frame
  variants differ only in their manifest `firstFrameDigest` (r5 hashed
  the full directory vector, which broke both properties; §14.1 carries
  the inline/stripped/rebundled and theme-variant fixtures). Two
  containers that admit different readers or mark a *core* segment
  required-vs-optional hash differently. `localizable-strings` is
  **not a plan segment at all** (§3.3.15 — catalogs are
  manifest-required auxiliary objects).
- **Loading is a validation pass, never trusted indexing.** A fetched
  or mapped buffer becomes usable only through a one-time
  **`ValidatedPlan` admission pass**: digest verification; checked
  offset arithmetic; segment bounds, non-overlap, and uniqueness; row
  framing and count limits; every `*Ref`/index/range cross-checked
  against its table; UTF-8 validity; type-graph acyclicity; opcode-body
  well-formedness. Only the validated handle enters unchecked hot
  loops; §14.1 gains malformed-plan fuzz fixtures against the pass.
- **Auxiliary artifacts carry their own digests**, and deployment /
  resumption / cache identity is a **manifest tuple**:
  `(planDigest, formatVersion, opcodeTableDigest, locale + catalogDigest,
  themeDigest, tokenSchemaDigest, firstFrameDigest?, lociDigest?)` — the
  token-schema digest rides the manifest so a runner can reject a
  mounted theme table whose id/type schema mismatches the plan's token
  indices (wrong-schema rejection is a §14.1 fixture). String catalogs are
  addressed `(planDigest, locale, catalogDigest)` — updating visible text
  bumps the catalog digest, so caches cannot serve stale copy. Loci
  sidecars are addressed `(planDigest, lociDigest)`, so relocated-source
  rebuilds keep distinct, correct sidecars. Baked first frames key on the
  **resolved-theme digest** (plus locale), not merely the color scheme.
- **Same-app sharing.** Two plans (or plan-referenced component sub-plans)
  with equal digests MUST be shareable as one mapping/cache entry. v1 scope
  is same-application (native) and same-site (web, per LLP 0483 §2's
  partitioned-cache correction). Cross-application sharing is out of v1
  (§3.8).
- **Format versioning — §3.1's fail-closed rule, restated identically**
  (r3's looser "runners ignore unknowns" summary survived here through
  r4 and is deleted): major bump = incompatible layout (runners
  reject); a minor revision may add directory-flagged **optional**
  segments (skippable) or framed optional columns; a required unknown
  segment or a `minReaderVersion` above the reader is a **load error**,
  never a skip. The format version is independent of the opcode table
  digest: a runner MUST refuse a plan whose `opcodeTableDigest` it does
  not implement. **[Amendment 4, 2026-08-25: likewise the
  `enumRosterDigest` — a mismatch refuses (`enum-roster-digest`,
  naming both digests), and a pre-v1.6 plan that cannot carry the
  field refuses at the reader floor (`enum-roster-absent`); the
  canonical preimage and both rules live in `format-schema.json`
  (`container.enumRosterDigest`, `container.versioning`).]**
- **Monomorphization policy** (0480 §15.8 / 0481 §9.8 resolved):
  **layout-monomorphic, code-shared.** A generic (snippet/polymorphic-prop)
  component emits one plan body per distinct **layout fingerprint** — the
  canonical serialization of the full concrete layout tree: field offsets,
  sum tag maps, element layouts, and widths (r1's "widths and kinds" tuple
  could not distinguish equal-width object layouts). Expression opcodes
  are layout-agnostic wherever fingerprints agree. Facet components with
  closed prop types typically have exactly one fingerprint, so
  identical-bytes sharing (0480 §7.2) is preserved for the case that
  motivated it.
- **Theme late binding** (the other half of the §7.2 tension): theme tokens
  are NOT baked into plan bytes. Token references compile to indices into a
  **theme table** — a reserved region of the state arena populated at mount
  from the active resolved theme (`@exact/facet-core` remains the styling
  truth). Index-based token reads are validated by a generated
  **token-schema digest** (ids + types), separate from the resolved-value
  `themeDigest` — value digests distinguish themes; the schema digest
  protects against token reordering across builds. Consequences: plan bytes are byte-identical across themes; a theme
  change is a state update, not a rebuild; and baked first frames key on
  the resolved-theme digest (§12.5).

### 3.5 The loci sidecar

Resolution of 0480 §15.10 (loci must survive the flat encoding):

- Every row of every provenance-bearing table — exprs, bindings, nodes,
  actions, tasks, cells, cursors, handlers, regions, slots, components,
  entrypoints — has an entry in the **loci segment** (the rule is
  by-table-class, not this list: a table whose rows have authored
  provenance participates, so the §3.5 "(table, row)" diagnostic
  address is total over trap subjects): table id, row
  index, source span (file id, start, end — offsets in **UTF-16 code
  units**, preserving `ir.ts`'s `Span` semantics unchanged). The **file
  table** — module-relative normalized path, `sourceVersion` content
  hash — is part of the loci segment, so a `file id` resolves without
  external state.
- The sidecar as a whole is addressed by `(planDigest, lociDigest)`
  (§3.4) and excluded from `planDigest`, so stripping it never changes
  plan identity and relocated-source rebuilds keep distinct sidecars; a
  single locus is addressed within it by `(table, row)`.
- Dev and web-dev builds carry loci inline (a directory-flagged
  auxiliary segment, §3.4 — header `flags` stay zero). Production builds
  MAY strip the segment to a sidecar file published beside the plan and
  re-attachable by digest — crash reports and agent diagnostics then
  resolve spans by fetching the sidecar, never by shipping it to users.
  Published sidecars carry **module-relative path ids, never
  build-machine absolute paths** (a redaction rule, not an option).
- Every compiler diagnostic, runtime contract verdict, runner trap, and
  replay divergence report MUST carry a locus when the sidecar is
  available, and MUST carry the `(table, row)` address even when it is not
  (so a stripped-production trap is resolvable after the fact). This is
  goal (d)'s floor: a plan that cannot say *where* fails agent ergonomics
  (0480 §15.10).

### 3.6 Localization

Resolution of 0480 §15.12, scoped for v1:

- The string split of §3.3.15 makes locale a **parallel auxiliary
  object**, not state — and localized references are **view-only
  sites** (string ids in bindings and Semantics rows): state strings
  are ordinary UTF-16 values that never hold *catalog references* —
  ordinary string state may of course contain translated text that
  arrived through an event or capability; such strings simply do not
  participate in catalog-generation invalidation — so no
  localized-reference state type exists. Concretely:
  `localizable-strings` variants are addressed by
  `(planDigest, locale, catalogDigest)` (§3.4) and swapped as a unit;
  author-visible literals intern into the localizable segment rather
  than into slot values.
- Locale is an ambient input (§7.1) for formatting expressions, and
  **every localized string site — constant text included — carries a
  `catalog-generation` Deps edge (§3.3.5)**; a catalog or locale swap
  enters the system as a named host event (`locale-changed` /
  `catalog-swapped`, admitted through §7.4 like any host input — not an
  unnamed side door), whose transition dirties exactly the localized
  sites and
  re-emits their protocol values through the ordinary §8.3 sweep (r3
  had no invalidation path for constant text; a swapped catalog would
  have left stale bindings on screen).
- The format schema defines localizable-string **site identity**
  canonically (derivation from the source site, invariant-vs-localizable
  classification, deduplication, collision handling), so catalog-only
  edits provably leave `planDigest` unchanged.
- Baked first frames are per-locale build outputs where localized text
  appears in the initial frame; a locale without a baked frame falls back
  to bake-and-patch (§12.5).
- v1 is scoped to single-locale plans plus this segment split; a translation
  pipeline is out of scope here.

### 3.7 Required vs optional segments

Required for every plan: Header, directory, Modules, Components,
Entrypoints, Slots, Types, Nodes, Bindings,
Deps, Derives, Regions, Actions, Handlers, Exprs, invariant Strings,
Semantics. Required where used: Tasks, Cells, **Cursors**, Motion,
Assertions (§9.1's compiled contract assertions — its own segment so a
checks-off runner never touches it, §9.4). Optional core: partition
table (§13.2), speculation hints (§13.5). **Auxiliary-class**
(digest-neutral, §3.4; inlined or shipped as external objects, identity
unchanged either way): loci (per §3.5, with its file table), baked
first frame (§12.5). The **serialized-state encoding** used by web
resumption (§12.4.3) and state capsules is not a plan segment but is
format-schema-derived — from Slots + Types for values, plus framing for
the runtime-instance surface resumption must restore: region/site
identity, instance paths and key order, cursor states, generations,
per-instance slots (one canonical binary encoding, versioned with the
format).

### 3.8 Explicitly out of v1

- **Cross-application shared plan pages.** Sharing bytes across trust
  domains needs a signer, a verification story, and a threat model
  (0480 §15.13); until an LLP supplies one, plans are per-application
  artifacts (per-site on web).
- **Mode-3 artifacts in the deployment path** (§12.3's gate).
- **A wire/distribution encoding for tier-5 placement** (§13.6).
- **Plan-encoded motion** (§3.3.14).

## 4. The expression VM and the opcode authority

### 4.1 One authority, generated lowerings

The opcode set will be defined in **one authority file** (Phase 0 I5
drafts it),
`packages/exact-contract/plan/opcodes.json` (new; registered in
`exact-contracts.json` as the plan-VM boundary authority when Track R
lands). Per LLP 0150 discipline: the TypeScript evaluator tables, the Rust
runner's dispatch table, the mode-3 code generator's emission templates,
and the web runner's dispatch table are **generated from this file**;
CI parity-gates all four against it (the
`tests/protocol/protocol-inventory.json` pattern applied to the VM).

This is the invariant that keeps three modes from recreating the 2× tax
(0480 §5, §13.3). Honesty note: shared generation makes divergence
*structural to introduce*, not impossible — the conformance corpus, not
the generator, is the enforcement; the generator makes enforcement
cheap.

### 4.2 Evaluation model

- **Machine shape:** a stack VM with `u8` opcodes, dispatch by jump table.
  No closures at the leaves; every expression evaluates against
  `(state arena, instance scope chain, envSnapshot, theme table, strings)`.
- **Numeric semantics:** `number` is IEEE 754 binary64 on every runner;
  the cross-runner identity guarantee is exactly the scoped rule below —
  no broader claim. Integer fast paths are permitted only where the
  compiler proves the f64 result is exactly representable and the
  observable value is identical.
- **Strings**: **storage is UTF-8; observable semantics are UTF-16 code
  units** — length, indexing, slicing, and spans match LLP 0330 §2
  exactly, so no string program changes meaning across tiers or against
  today's runtime (the VM computes UTF-16 answers over UTF-8 storage).
  Strings are immutable and compared by value (0330 already gives strings
  value equality); template strings lower to interleaved concat opcodes;
  formatting opcodes that consume locale read it from the environment
  snapshot (§3.6).
- **Numerics follow LLP 0330 §2 verbatim**: division by zero is signed
  Infinity, `0 / 0` is NaN — IEEE results, never traps. `NaN != NaN`
  stays true. A canonical NaN bit pattern is fixed for hashing and
  serialization only; it is unobservable to `==`. Cross-runner identity
  is scoped to what is enforceable (r3's blanket "bit-identical
  arithmetic" over-promised against platform libm): `+ - * /`,
  comparisons, and number-to-string (ECMAScript's algorithm, per 0330)
  are bit/character-identical everywhere; transcendental and other
  `Math`-class opcodes ship **pinned software implementations in the
  opcode authority** — never platform libm — so their results are also
  identical, by construction rather than by hope.
- **Determinism:** no opcode reads a clock, RNG, or any ambient channel
  except through the envSnapshot operand; iteration opcodes iterate arena
  order (§8.2 defines it).
- **Traps:** the trap matrix — one specified behavior per opcode, never
  UB, never engine-dependent — lives in the opcode authority with
  LLP 0330 §2 as its semantic source (missing sum arm at runtime, which
  §5.2 makes unreachable except through host data; out-of-bounds access
  — an arm that exists only on the change-1 reject branch, since §6.2's
  option-shaped rule makes it unreachable once change 1 lands). Traps inside a
  transition abort it per §7.5. Binding evaluation follows 0330's
  existing error semantics; the repo's graceful-degradation rule (missing
  props degrade) is a property of prop defaulting, not a license for
  bindings to swallow evaluation errors — r1's "declared fallback" is
  withdrawn (Bindings carry no fallback field).

### 4.3 The pinned stdlib

The callable surface inside expressions is closed: arithmetic, comparison,
logical, ternary, `??`, string templates and the string/array/collection
helper set enumerated by the opcode authority, sum construction/matching,
and value-kind constructors. There is no FFI from an expression; anything
else is a capability call (§10) and legal only where §7's effect rules
admit it. The stdlib is versioned with the opcode table; adding a helper is
a minor opcode-table revision propagated to all runners by regeneration.

### 4.4 What lowers to the VM

All executable contexts of §2 except: task bodies (a restricted superset —
§7.6), capability implementations (native, behind the ABI), and the
Class S sub-programs — motion worklets and graphics scene programs
(separate declared evaluators, §3.3.14/§8.1; this list and §11.2's
Class S are the same set by construction). LLP 0479 §5.1's finding stands as a
design constraint: the compact-plan backend that kept per-site compiled
closures at the leaves was not a test of this design; a plan's leaves MUST
be opcodes over the shared evaluator, or F2's spike (§14.2) is not testing
the hypothesis.

## 5. Change 1, specified — closed static types

### 5.1 The type system

The language's value types (every tier, not just native admission —
the scope promotion of LLP 0481 §2.2):

- primitives: `string`, `number`, `boolean`, `null`, `never`;
  *(**DECIDED since freeze** — 2026-08-20, LLP 0505 r3 row 10 / LLP 0489
  r6 / LLP 0508 §4.2: absence is **Option-only** in the authored
  language; `null` survives solely as boundary-parse input normalized at
  the seam, and `T | null` is not an authored type. This row's `null`
  remains in the frozen §6.2 key universe for boundary values.)*
- `string-enum` (closed literal sets);
- **sum types**: tagged unions with per-arm payload types; exhaustiveness
  is a compile obligation wherever a sum is consumed (§5.2);
- **closed objects**: named fields, fixed layout, no additional
  properties;
- arrays of a single element type (SoA-decomposable when bound in `each`);
- value-kinds: dimension, color, duration (retained from
  `native-state-schema.ts`; its **ephemeral action-parameter arm dies
  with change 1** — action parameters become the typed event payloads of
  §7.1, so the escape it provided is no longer needed);
- action/callback types: first-class values carrying an **effect row**
  (§7.3, closure 4) — authors write the arrow form; the row is inferred
  at the composition site.

The existing `ClosedStateType` preflight
(`packages/exact-contract/src/compiler/native-state-schema.ts`, authority
LLP 0333 §5.1) is the embryo: this spec extends it with sums and effect
rows and promotes it from one tier's admission gate to the language,
keeping the dual-derivation discipline (Rust independently re-derives the
descriptor where an ABI boundary is crossed).

**The `undefined` clause — part of the change-1 amendment to 0330 §2,
proposed, not implied.** 0330 §2 makes `undefined` observable (absent
members, optional chaining, truthiness, `??`, interpolation), and the
universe above has no `undefined` — the shipped preflight already
rejects optional access as "a non-serializable undefined value". Change
1 therefore **eliminates observable `undefined` from the value model**:
closed objects have no absent members; optional access restates over
option sums (`none`) and `null`; **the rewritten 0330 §2 bullets are
enumerated in the amendment text, not implied** — truthiness, `??`,
optional member access, template interpolation, and out-of-bounds
indexing (§6.2's option-shaped rule) all lose their `undefined` arms; every undefined-producing expression
gets a fix-it to the explicit form; the migration census (§11.3's
channel) counts undefined-observing sites, report-only. A substantial
language change, stated as a named 0330 §2 delta in the §1.2 inventory —
never smuggled through the preflight's promotion.

**Function-typed values in state** are restricted to plan-addressable
action/callback refs (identity + curried environment, §3.3.10) — never
arbitrary closures: 0330 §2's opaque `function` values otherwise have no
dense encoding and no A-VAL copy semantics.

### 5.2 Inference

- **Authors write no state annotations** (hard constraint, LLP 0481 §2.1).
  State types infer from initializers; props keep their declared types;
  sums infer from construction sites and `match` arms; unification is
  bidirectional across `derive` chains within a component and across
  composition boundaries via prop types.
- An unresolvable inference is a **compile error with a fix-it** naming
  the narrowest resolution (mandatory-fix-it rule, 0480 §9.1) — a prop
  annotation, a `shape`, a sum constructor, or a restructure; **never a
  state annotation**, which the language does not have. `any`-like
  widening does not exist.
- Heterogeneous literals infer a sum when arms are closed and enumerable
  from the source; otherwise the boundary-parse path (§5.4) is the fix-it.
- Exhaustiveness: consuming a sum in `match` or field access on an arm
  requires either full arm coverage or an explicit `else`; the diagnostic
  enumerates missing arms (determinate fix-it).

### 5.3 Layout

- A component instance is a struct in an arena: slots at fixed offsets
  (Slots table), no per-slot heap cells, no `Rc<RefCell<…>>` graphs.
- `each` instances are **struct-of-arrays row arenas**: one contiguous
  column per bound field, plus a key column and an order index (§8.2).
- Sum layouts are tag + max-arm payload, arm layouts fixed at build time.
- Strings and arrays in state hold arena handles (length-prefixed,
  value-semantics copies on assignment; copy-on-write is a permitted
  optimization invisible to semantics — §7.5's value-semantics rule is
  what makes journaled rollback and worker snapshots sound).

### 5.4 The boundary-parse paved path

LLP 0481 §2.4's owed design, specified. Network/import data enters typed
state through a declared shape at the cell boundary:

```contract
resource stations = getStations() as
  shape { id: string, name: string, zone: number }[]
```

- `as shape` (surface syntax subject to grammar review; the construct is
  normative) attaches a parse/validate step at settlement: conforming
  data lands as the closed type; non-conforming data settles the error
  arm with a locus-carrying diagnostic value — never a partial value.
- The compiler derives the validator from the type. **The ergonomic bar
  is normative:** if the §11 audit shows authors routing around `shape`
  (blob-holding via escapes), change 1 fails on ergonomics and the
  construct is redesigned before the amendment lands (LLP 0481 §9.2).
- "Hold the response verbatim, read it lazily" is expressible only as a
  capability-held opaque handle (§10) — never as state.

### 5.5 Generics

Per §3.4's monomorphization policy: generic components type-check once
against their bounds; layout signatures instantiate per distinct concrete
layout; the certificate reports per-plan layout-signature counts so plan
size growth is observable (§9.2's plan-size budgets are the enforcement).

## 6. Change 2, specified — static dependencies

### 6.1 Resolution rules and granularity

- Every executable context's read set resolves at build time into Deps
  rows. **A derive whose dependencies cannot be resolved statically is a
  compile error** (fix-it: the §6.2 cursor, the §6.3 guard, or a
  restructure).
- **Granularity** (LLP 0481 §9.3 resolved): dependency edges are
  **field-granular for closed-object slots** (change 1 makes field offsets
  build-time facts) and **column-granular inside `each` row templates** (a
  binding depends on the columns it reads, not the row). Whole-slot edges
  remain for primitives and whole-value reads. This is what makes change
  5's fan-out budgets measure real width for LLP 0477's F3.1/F3.3 hazards
  rather than reproducing them at slot grain.
- The topological order is computed once per plan and stored (§3.3.5);
  runners never maintain `readHeight` bookkeeping.

### 6.2 Dynamic indexing: the cursor, plus budget-enforced widening

Resolution of the series' "single most important unresolved question"
(0480 §15.1, LLP 0481 §3.5), as a two-part rule:

1. **`cursor` — the paved element-granular form, with ONE identity
   rule.**

   ```contract
   cursor selected = stations at selectedIndex key s => s.id
   ```

   (The key expression binds its element explicitly — `s` above; the
   binder form is grammar-review surface, the requirement that the key
   expression carries an explicit binder is not.)

   A cursor's value type is option-shaped: `some(element) | none`;
   consumers match on it. **Key identity is declared, never inferred
   from the collection** — keyedness is a property of each `each` *site*
   (its required `key` expression), not of the array value, so a cursor
   names its own key: `cursor selected = stations at selectedIndex
   key s => s.id` (surface syntax per §18; `keyExprId` in the Cursors table,
   §3.3.11a). Key values are restricted to the canonical key universe
   keyed `each` already enforces (`identity.ts:21–25` key types; the
   accept-and-normalize rules with throwing rejections at `:54–65`:
   null, boolean,
   string, finite number with `-0` normalized; objects, arrays, NaN,
   and infinities are rejected at the key expression's type), and
   equality within it is SameValueZero;
   where the same collection also feeds an `each`, a differing key
   expression gets a `cursor_key_mismatch` warning with a fix-it.
   Identity and lifecycle:

   - A **key cursor** (one with a `keyExprId`) has three runtime
     states —
     `unbound(ordinal)`, `bound(K)`, `missing(K)` — surfacing as
     `some`/`none` (`bound` with a live row is `some`; the rest are
     `none`; the states drive edges, not the value type). **In every
     state the cursor subscribes to the collection's key-set
     generation** — r6 subscribed only `unbound`/`missing`, so a
     `bound(K)` cursor never observed `K`'s removal and could keep an
     overlay edge to a disposed row, still surfacing `some`. Key-set
     deltas are computed **before row reclamation**: an affected
     `bound(K)` transitions to `missing(K)`, its row overlay is
     removed, and its consumers are invalidated before the row is
     disposed; a change to a row's key-expression value is a
     remove-plus-add. The cursor's own key projection enforces
     duplicate-key rejection even when no `each` shares the
     collection.
     - **unbound** (including the initially-empty collection, r3's gap:
       an empty collection at first resolution stayed `none` forever):
       the cursor watches collection membership/cardinality and
       resolves ordinal → key as soon as its ordinal exists;
     - **bound(K)**: follows `K` — a reorder alone leaves it on the
       same element, and the ordinal state is NOT written back (the two
       are decoupled until an `at` dependency next changes; specified
       behavior, not an artifact);
     - **missing(K)** (K removed): watches for exactly `K`'s return;
       re-binds to the same key if it reappears; a fresh `at`-dependency
       change performs a new ordinal → key assignment instead.
     Every (re)assignment resolves against the post-transition snapshot
     in the §8.3 sweep. Duplicate keys are a keyed-`each` error
     (**runtime today** — `contract-native/src/view.rs:1146` — promoted
     to a compile obligation where keys are statically enumerable).
   - An **ordinal cursor** (no `key=`): always element `i`; reorders
     move what it names; out of bounds → `none`.

   Reads through the cursor get element-granular edges (the resolved
   row's target columns plus the `at` expression's dependencies; an
   unbound/missing cursor's edge is the membership watch).
2. **Bare dynamic indexing widens, visibly.** `items[i].x` with reactive
   `i` in a derive/binding lowers with edges on the collection's
   **whole-value version** — bumped by any write to the collection or to
   any element, so an element-field write is never missed (r1's
   "structural version" tracked shape only and was unsound) — plus `i`.
   The compiler emits a `dep_widened` info diagnostic carrying the
   symbolic width (§9.2's forms) and a cursor fix-it. Widening is legal —
   programs stay admissible — and a widened edge that breaks a declared
   fan-out or op budget is a build error. **Out-of-bounds is
   option-shaped under change 1** (part of the §5.1
   undefined-elimination clause: 0330 §2 pins `undefined` for absent
   members and optional chaining but leaves indexing OOB unpinned — the
   TS and Rust runtimes yield `undefined` there today, and this
   amendment closes the gap): a dynamic array or string
   index yields `some(T) | none`, so bare `items[i].x` no longer
   type-checks without matching/`?.`-unwrap — the fix-its are the match
   form or the cursor, the migration census counts these sites, and the
   VM trap table has no OOB trap because the type system made one
   unreachable. Statically-bounded indexing (a proven in-range index)
   stays `T`.

Consequence for §11 — **a proposed amendment to 0480 §12.2, put to its
adjudicator**: dynamic indexing *lowers* (class 1) under either form,
but lowering MUST NOT remove it from the kill gate ("counts inside the
residue like any other class"). The certificate carries a first-class
**widening axis** (§11.3) — widened-edge count, symbolic width, share of
collection-granular derives, per route — and the proposed verdict rule
is that a class-5-clean route whose live width reproduces LLP 0477's
F3.1/F3.3 hazard is not an F1 pass — concretely, §11.3's witness
predicate: zero *unbounded* widened sites; every widened site carries
an adjudicator-accepted symbolic bound. The axis reports from the
Phase 0 certificate onward, not from change 5.

### 6.3 Conditional dependencies

- **Pure derives:** `a ? b : c` over-approximates to the union; the
  compiler reports union edges distinctly (`dep_union` info) so authors
  can see recomputation width. Same value, possible extra recomputation —
  acceptable and visible (LLP 0481 §3.5).
  *(**KNOWN ERRATUM** — the "same value, merely extra recomputation"
  claim is false for mutually conditional derives: LLP 0481 §3.5 itself
  proves branch-unioning can mint an artificial SCC with no topological
  order even though every runtime valuation is acyclic. Tracked as
  `issues/20260820-llp0485-conditional-union-false-scc-erratum.md`; the
  resolution — guarded edges or a defined rejection diagnostic — lands
  via the LLP 0508 ratification, which must not carry `dep_union`
  forward as written.)*
- **Resource initializers:** union widening is **forbidden** — it would
  amend resource identity semantics (`llp/contract/0082:81–84`: identity =
  declaration instance + the dependency tuple the initializer read; a
  dependency change is an identity change, so a union lets an
  inactive-branch write restart/cancel/refetch — LLP 0481 §9.9). Resolved
  toward **guarded edges**, fully specified: a Deps row targeting a cell
  initializer MAY carry a guard expr id; **guards are dependency nodes in
  their own right** — a guard's statically resolved read set appears as
  ordinary Deps rows targeting the guard, topologically ordered before
  every edge it guards, so "evaluate guards before their edges propagate"
  is the normal §8.3 sweep order, not a special case. The atomic
  transition rule: at the §8.3 sweep, against the **post-transition
  snapshot**, the runner (1) re-evaluates dirty guards, (2) computes the
  cell's new active-dependency tuple, (3) compares it to the previous
  tuple, and (4) takes the cancel/restart/refetch decision from that one
  comparison — discriminant and newly-active dependency changing together
  is a single identity change, never two racing ones. This preserves
  `llp/contract/0082:81–84`'s identity semantics (declaration instance +
  the dependency tuple the initializer read) at the cost of guard
  evaluation in the invalidation path — the precision branch of 0481
  §3.5, priced there.
- Authored resource dependency lists (the annotation branch) are NOT
  introduced; guarded edges make them unnecessary.

### 6.4 Cross-module and capability dependencies

A derive reading across a module boundary needs the exporting module's
plan to declare the export's type and (for reactive exports) its slot —
plan modules link by declaration, not by execution (LLP 0183 / LLP 0206
machinery, made total). A derive reading a capability value is legal only
for capabilities declared `reactive` in the ABI (§10.2), which gives the
edge a declared invalidation channel; reading a non-reactive capability in
a derive is a compile error (fix-it: lift to a cell).

### 6.5 Deletion of read-tracking

With change 2 landed (Track L), the runtime tracking machinery —
`track(core)` / `currentComputation` / `observers` / `deps` /
`readHeight` (`packages/exact-contract/src/runtime/signals.ts:732–739`,
`currentComputation` at `:104`) — is deleted, not bypassed. `untrack` is
a **runtime helper, not a language form** (authors never had a keyword;
the runtime uses it internally at many sites — handler curry
evaluation, memo comparators, resource initializers, view mounting) —
so nothing is removed from the grammar, and **no `peek` opcode replaces
it** (r3 proposed one; it had no job): with the tracking machinery
deleted there is no subscription to avoid anywhere — *every* evaluation
is non-subscribing, edges come only from the build-time Deps table, and
each of `untrack`'s current uses maps to an existing static mode
(handler-prelude deps for curry evaluation, §3.3.10; ordinary
`stateReads` in action bodies; plain evaluation everywhere else).

Sound read *masks* for dispatch (§13.3) are new work on the semantic-graph
walker, not a relabeling: the current statement walker records an
assignment's value reads but **not its target-index reads**
(`semantic-graph.ts:1486` — `rows[index] = 1` records no read of
`index`), and handler preludes evaluate at dispatch (§3.3.10). §13.3
specifies the fail-closed composition.

## 7. Change 3, specified — actions as reducers

### 7.1 The transition

```
(state, event, envSnapshot) → (state′, Command[])
```

- **`event`** — a typed value, payload type per action params, carrying
  its enqueue sequence id (§7.4).
- **`envSnapshot`** — the declared ambient inputs, captured at enqueue:
  clock, locale, color scheme, viewport class — and RNG as a **per-event
  deterministic substream**. **This is a proposed amendment to LLP 0330
  §5/§11.2, which own the RNG** (0330 pins one mulberry32 stream over
  the seed, with replay oracles keyed by lexical site, logical instance,
  turn, and semantic ordinal — r3 mis-filed this as a 0482 delta). The
  amendment text owed to 0330's owner: substream seed =
  domain-separated fold of the session seed with the event's u64
  sequence id (exact bit-level derivation specified in the amendment);
  draws within a transition remain keyed by 0330 §11.2's lexical site
  and semantic ordinal *within* the substream, so the oracle vocabulary
  survives; a replay-migration note prices the observable-output
  change; and the exact bit-level derivation is part of that amendment
  text — **owed before any cross-runner RNG implementation begins**,
  the reject branch being the only implementable rule until then. What
  it buys: no two events share a stream, so draws cannot race under
  any §13 schedule and replay is exact per event.
  **Reject branch:** under 0330 §5 as written (one global stream),
  RNG-reading invocations are ineligible for parallel evaluation and
  dispatch serially — sound, merely narrower. The ambient-read mask
  (§3.3.8) is the closed channel list; reading an undeclared channel is
  a compile error.
- **`Command[]`** — the closed vocabulary: `task-start(taskId, args)`,
  `capability-call(abiRef, args)`, `navigate(intent)`, `emit-user-event`,
  `cancel-task(handle)`. Programmatic channel emission (`publish()` in
  action/task bodies, `ChannelEmitterWitness`, `ir.ts:829–833`) is a `capability-call` against
  the publish descriptor — named here so the command vocabulary, not
  only the IR ledger, covers it. Commands execute strictly after the transition
  commits, in transition-return order; a command MUST NOT synchronously
  re-enter the dispatcher.
- No `await` in an action body — asynchrony re-enters as typed events
  dispatched by tasks and settlements (§7.6, closure 3).

### 7.2 Effect signatures

Every action row carries a build-time signature with these fields —
**authored and derived kept distinct**:

- `writes` — authored slot/field mask (sound over-approximation;
  under-declaration errors, over-declaration warns — today's
  `analyze.ts` asymmetry);
- `mutates` — **its existing meaning, unchanged**: the provider/
  capability *mutation identities* an action or task may invoke
  (`compiler/ir.ts`'s separate field; `buildActionEdges` already links
  them to mutation descriptors). r2 redefined it as in-place state
  targets, which would have erased shipped provider-effect semantics —
  withdrawn. In-place state writes live in the write mask like any
  write; mutation identities are **conflict domains** for §13.3;
- `stateReads` — **compiler-derived**, stored in the Actions row: the
  composed read mask, including member/index *target* reads (closing
  the `semantic-graph.ts:1486` gap for `assign` AND the identical gap
  in `await-assign` at `:1495–1500`) — the field tier 2's signature
  consumes (handler-prelude and payload reads join per-invocation);
- `commands` (kinds + capability refs), `ambient` (read mask), `calls`
  (action ids).

Signatures compose transitively through `calls` and callback effect rows
(§7.3.4), computed at build time and stored composed. Generic sharing
note (§3.4): a component's **effect-row fingerprint is separate from its
layout fingerprint** — two instantiations may share layout and differ in
callback rows, and plan-body sharing requires both to match.

### 7.3 The seven closures, normatively

Restating LLP 0481 §4.1's inventory as rules:

1. **Loops.** Under A-VAL, `for` iterates a **snapshot** of the
   collection value taken at loop entry, so iteration is bounded by
   construction. **A-VAL-reject branch:** under 0330 §2's reference
   model the grow-ban returns — the compiler rejects structural writes
   to the iterated collection inside the loop body (element writes
   fine). Action-call cycles are rejected from the composed call graph.
   Cost bounds price loop bodies symbolically (§9.2); the rollback
   journal covers mid-loop throws (§7.5).
2. **Tasks do not write state.** A task body performs I/O and dispatches
   typed events; its only state effects are the transitions of the actions
   those events dispatch. `TaskIR` gains a dispatchable-action set
   (§3.3.9) and loses nothing else.
3. **Resource settlement is an event.** A cell's status/value/error
   transitions enqueue typed settlement events that drive the cell's
   state-machine region (§8.1) through ordinary transitions — replay
   covers the async world because the async world enters through the log.
4. **Callbacks carry effect rows.** An action-typed prop is an
   effect-carrying value (§5.1); a component's composed signature
   includes the rows of the callbacks it may invoke; passing a callback
   whose row exceeds the receiver's declared tolerance is a compile
   error at the composition site.
5. **Capability calls carry affinity and purity** from the ABI (§10.2)
   into the signature; §13.4's worker-affinity certificate reads them.
6. **Exceptions roll back via the journal** (§7.5): a throw aborts
   atomically — state restored, zero commands, the event logged
   `aborted` with the trap locus.
7. **Imported opaque calls** in an action body are not expressible: an
   imported symbol either lowers (its module is a plan module), resolves
   to a declared ABI capability, or the certificate classifies the
   context (§11) — at best an explicit escape on an admitting tier,
   never silent.

### 7.4 Events: ordering and the tie-break table

The sequential oracle is **monotonic enqueue order** (LLP 0482 §3.3),
made implementable by defining the linearization point exactly. There is
**one runtime-thread inbox**. Sources hold items until the dispatcher
**admits** them; admission assigns the u64 sequence id, and admission
order *is* event order — admission order is the definition of
simultaneity. **The inbox model itself is a proposed 0330 §4 amendment** *(**DECIDED
since freeze** — 2026-08-20, LLP 0505 r3 row 8 / RFC 0482 r6: the
amendment is **DECLINED for v1**; the reject branch below is the
decided state — host events dispatch synchronously at delivery, the
class order applies to timers and completions only, and tier 2 is
replay-only. Do not implement the live inbox; this passage is the
recorded design of the not-taken branch.)*
(0330 as written dispatches host events synchronously; queueing them
through an admission point is an observable-timing change and is named
in §1.2's inventory, and the amendment text extends 0330's quiesce
definition: quiescent additionally means every source queue is drained
to its admitted high-water — else replay fixtures disagree about "the
same" event script; reject branch: host events dispatch synchronously
at delivery, the class order below applies only to timers and
completions, and tier 2 is inadmissible). One dispatcher iteration takes
an **atomic drain cutoff**: at iteration start it collects the
**cutoff vector** — one inclusive high-water ordinal per source queue,
read in **canonical source-id order**, each an acquire-ordered atomic
read of the producer's monotonic tail counter — and an arrival landing
during collection is classified by its own queue's already-read
ordinal (at or below → this iteration; above → next); "atomic" names
the per-counter reads plus this fixed collection order, not a global
multi-queue snapshot. The whole existing prefix up to each ordinal is
admitted this iteration; arrivals above wait. Source ids
order by a **canonical composite key, not a generated hash** (a static
registry cannot enumerate runtime instance paths — r6's deferral was
unconstructible): `(source class, declaration id, canonical instance
path, scope generation, start-attempt ordinal)`, with the
start-attempt ordinal **allocated synchronously by the runtime owner
at task start** — so two overlapping starts of one `manual` task
(legal today) are distinct sources. Typed key serialization and
lexicographic comparison are format-schema-defined; capability sources
remain `(EffectAddressV1, attempt ordinal)`; a compact numeric interning
of these keys is permitted only as a deterministic injective cache with
a defined lifetime. Admission
then proceeds in a fixed class order:

1. due timers — one **merged cross-family key**, because 0330 orders
   `every`/`after` (§4) and the deadline queue (§10.2) each internally
   but never against each other: `(dueTime, familyPriority: §4 timers
   before §10 deadlines, registration ordinal within family)`. The
   intra-family halves are 0330's own rules verbatim; the cross-family
   key is part of §1.2's proposed inbox amendment;
2. host input, by `(stable source id, per-source ordinal)`;
3. task and capability completions, by `(stable source id, per-source
   ordinal)`.

"Delivery order" is thereby defined, not assumed — and so are the ids.
**There is ONE source identity — the composite key above — with
per-class fill rules, and no independent namespace tuples** (r7 restated
task identity here without the start-attempt ordinal, resurrecting the
overlapping-starts collision the key exists to prevent): host sources
fill `(host, registry id, ∅, ∅, 0)`; task sources fill
`(task, declaration id, canonical instance path, scope generation,
start-attempt ordinal)` — **`TaskSourceIdentity`, the one identity the
inbox, replay metadata, cancellation stamps, and the §7.6 RNG domain
all use**, with scope generation and cancellation generation explicitly
distinct fields; capability attempts fill their key from
`(EffectAddressV1, attempt ordinal)` per LLP 0330 §11. Every source
stamps a monotonic per-source ordinal; the admission table records
both. Due timers fill `(timer, family, global registration ordinal, ∅,
firing ordinal)`; `TaskCancellationStamp` is explicitly
`(TaskSourceIdentity, cancellation generation)` — the two generations
are different fields, never conflated. §14.1 carries the fixture: two
overlapping `manual` starts of one task — distinct source ids, distinct
RNG substreams, independent cancellation, racing completions
deterministically ordered.

**Target liveness — the inbox's other half.** Producer identity says
who sent an invocation; nothing yet said whether its *target* is still
alive when it dispatches (0330-as-written never faced this: synchronous
host dispatch cannot outlive its target; the inbox can — event A may
dispose the row or region holding event B's handler before B runs).
Every targeted invocation therefore carries an
**`InvocationTargetStamp`** — `(plan generation, TargetSiteId,
instance path, instance/scope generation)` — where **`TargetSiteId` is
a discriminated, source-independent identity** (r10's `binding-site id`
had a host-only fill rule while the contract is universal): host
handler binding sites; timer-action registrations; task
settlement/dispatch sites; cell settlement sites; direct command
dispatch sites. The stamp is **minted at event PRODUCTION by the
per-class producer** — the published host binding at input intake; the
timer owner at fire; the task/capability owner at completion enqueue;
the command emitter at command creation — and carried unchanged through
the source queue and admission (r9 stamped at admission, which left the
production-to-admission race: an event waiting above the drain cutoff
could bind a removed-and-recreated generation and deliver input aimed
at its predecessor). Any producer that cannot identify an exact target
generation **fails closed** — the event is rejected at production and
recorded. The same `TargetSiteId` vocabulary is what replay metadata,
conflict signatures, and the §13.5 speculation key use — one identity,
no per-consumer respelling. The stamp is part of the
invocation's conflict/liveness domains and is checked against live
state at the same three points as the task fence: before prelude
evaluation, before reducer evaluation, and before commit/command
release (§13.3's
"task/scope generations" recheck means exactly this stamp plus §7.6's
task fence). **Stale policy: a generation mismatch SUPPRESSES the
invocation**, replay-recorded with reason `stale-target` — never runs
against retained state, never traps, never resurrects (the fail-closed
choice; part of the proposed 0330 §4 amendment text). §14.1 carries
the fixtures: an earlier event removes, replaces, or HMR-resets the
later event's target — including the above-the-cutoff
remove-and-recreate race (the delayed event MUST suppress, not execute
against the reincarnation), the HMR/reset and real-input variants,
**stale-target cases for every producer family — host input, timer
fire, task settlement, cell settlement, direct command** — and the
tier-2 case where the two are slot-disjoint and would otherwise share a
parallel run. Concurrent completions from different sources order by
source id — arbitrary but fixed, cross-runner identical, and
replay-recorded (a proposed 0482 §3.3 delta: its "stable source ID"
tie-break becomes the within-class ordering key, with per-source
ordinals supplying the FIFO half).

Timers-before-input within one iteration is a definition, not a latency
claim: a deadline is a semantic commitment (0330 §10) and delivery timing
is not, and cross-source arrival order between OS queues is not otherwise
well-defined. Per-batch replay metadata (§13.1) records the admitted
sequence, so any divergence is detectable, never silent.

### 7.5 Rollback: value semantics plus a write journal

- **Value semantics — a NAMED breaking amendment of LLP 0330 §2/§2.1
  ("A-VAL"), proposed to W2's ratification owner, not smuggled.** The
  amendment, stated so it can be adjudicated:

  1. Contract values have no interior references: assignment copies
     (observably; copy-on-write permitted) and two names never alias one
     mutable value. Member/index state assignment is an ordinary write
     in the write mask, which implementations may serve by copy-on-write
     or uniquely-owned in-place storage — observably identical either
     way. **A-VAL does not alter the authored `mutates` clause**, which
     remains §7.2's provider-mutation effect set. Replaces 0330 §2's
     reference model for arrays/objects.
  2. **Equality:** `==` on arrays/objects becomes a **compile error with
     a fix-it** (compare fields, keys, or lengths explicitly) — under
     value semantics reference identity is incoherent, and deep
     structural `==` is a hidden O(n) the language refuses to hide.
     Primitives, strings, and sum tags keep 0330's strict-equality rules.
  2a. **0330 §11.2's "Object.is-faithful" replay-value sentence is
     amended by name**: primitive replay fidelity keeps `Object.is`;
     aggregate replay values are compared under
     `PublicationEquivalenceV1` (clause 5), since aggregate alias
     identity is no longer observable.
  3. **`memo` / `memoRows`:** restated without alias preservation —
     reconciliation keys on `key` (SameValueZero, as today) and value
     fingerprints; 0330 §2.1's "repeated input aliases remain aliases"
     is withdrawn, aliasing being no longer observable.
  4. **Loop iteration** is snapshot iteration (§7.3.1).
  5. **`PublicationEquivalenceV1` — the rule 0330 §2's `Object.is`
     publication cannot survive A-VAL without** (physical handle
     identity becomes an implementation choice under COW vs
     uniquely-owned in-place storage, so handle-based publication would
     make the storage strategy observable — version advancement,
     dirtiness, guarded resource tuples, and op emission would diverge
     between legal runners): primitives, strings, string-enums (which
     fold into strings), and sum tags keep 0330's `Object.is`/strict
     rules; **value-kinds (dimension/color/duration) compare by
     `Object.is` on their canonical packed encoding** (the row r12
     omitted — two runners could legally disagree on whether a color
     assignment publishes); **aggregate (array/object) slot
     writes and derive publications ALWAYS publish** unless an authored
     memo policy (`memo`/`memoRows`, clause 3) stabilizes them — priced
     honestly as possible extra recomputation, the same trade clause 2
     made for `==`, with no hidden deep equality. **The rule is total
     and type-directed**: always-publish applies to whole-slot
     aggregate assignment and derive results (it does NOT collapse
     §6.1's field-granular dirtying — `obj.x` vs `obj.y` stay distinct
     addresses); payload-bearing sums publish on tag change, and on
     equal tags compare the active payload under this same rule
     recursively; action/callback references compare by **action
     identity — the declaration id PLUS the target runtime instance
     path/generation** (shipped actions close over their component
     instance; two rows' callbacks to one declaration are different
     values, and the target rides the value encoding) — then
     **pairwise recursive `PublicationEquivalenceV1` over the typed,
     ordered curried environment** — never handle, shallow, or
     implementation-chosen depth. Guarded resource dependency tuples compare
     by this same rule — **priced honestly beyond recomputation**: a
     fresh-equal aggregate dependency re-publishing can change a
     resource's identity tuple and trigger an external
     cancel/refetch (0082's identity rule), so authored memo
     stabilization is the recommended pattern wherever identity
     stability matters. §14.1 carries
     differential fixtures: fresh-equal aggregates, copied handles, COW
     vs in-place mutation — byte-identical observable behavior
     required.
  6. **Migration is priced, report-only, in the certificate pass**: a
     corpus scan for array/object `==` and alias-dependent patterns
     rides §11's audit alongside the await census.

  What A-VAL buys: sound journaled rollback (below), cheap
  `ArenaSnapshot` worker snapshots (§13.3 — a mechanism with measured
  costs, not "O(1) free"), sound speculation (§13.5), and
  schema-derived dense serialization of the client-class projection
  (§12.4.3 — "memcpy-shaped" was a slogan for a handle-bearing arena
  and is withdrawn). The other branch — keep reference semantics,
  journal references under a defined alias rule — remains available to
  the adjudicator; this spec specifies A-VAL because every §13 mechanism
  is structurally simpler under it. Until adopted into 0330, §0's
  authority rule stands and this section is pending text, like change
  3's amendment.
- **Journaled transitions — the ordered-entries model.** The journal is
  **one ordered entry `(address, before, after)` per executed write, in
  execution order**: rollback applies `before` values in reverse order;
  a worker merge or speculation hit applies `after` values in forward
  order (r6's append-on-first-write rule was undefined for repeated
  writes — `x = 1; x = 2` could merge as `1` — and ambiguous when a
  whole-slot write preceded a field write to the same slot; ordered
  per-write entries are the simple correct v1, and any coalescing
  optimization must capture `before` once, update `after` on every
  later write, and use the format schema's normative
  ancestor/descendant address normalization — observably identical to
  the ordered model or it is wrong). **Entries are patches at the
  finest written address — never whole-object replacements**: a
  transition writing `obj.x` journals a field patch; a whole-slot
  entry is legal only when the whole slot was written. §14.1 carries
  the repeated-address, whole-slot-then-field, loop-body, rollback,
  worker-merge, and speculation-hit fixtures. **This is a
  rollback/merge-correctness rule, not a parallelism widener**: tier-2
  conflict signatures stay at 0482-as-written's slot-table bitmask
  grain (§13.3), so invocations writing `obj.x` and `obj.y` conflict at
  slot grain and serialize — field-granular conflict bits would be an
  unlisted 0482 address-space change and are not taken. `before`
  entries are frozen value snapshots — normatively: an implementation
  using uniquely-owned in-place mutation MUST take an immutable
  pre-write snapshot for the journal (fresh-handle writes get this for
  free; the freeze is the requirement either way). On a trap, the journal unwinds in reverse and
  the command buffer is discarded. On commit, the journal is the write-delta half of the
  §8.3 `TransitionArtifact` (workers return it with their command
  lists; speculation caches complete artifacts). Copy-on-write per
  transition is a permitted alternative with identical observable
  behavior.
- **A-VAL-reject branch, priced:** if 0330's owner refuses A-VAL, the
  journal operates over reference values with an explicit alias rule
  (journal-by-address with reference-tracking of interior mutation —
  strictly more machinery), worker snapshots (§13.3) become
  copy-on-write arena snapshots rather than free immutable reads, and
  §12.4.3's schema-derived dense encoding pays per-value copies instead
  of table copies (priced against the encoding actually specified —
  "memcpy shape" is withdrawn everywhere). Track L MUST NOT land
  change 3 until A-VAL is decided, because change 3's rollback text
  differs by branch (§15.2).

### 7.6 Tasks

- Task bodies MAY await (they live outside the interactive loop —
  LLP 0481 §4.4's "probably yes", adopted); the `action` / `task`
  distinction is enforced at the grammar level so the boundary is
  unmistakable.
- A task's capabilities and dispatchable actions are declared (§3.3.9);
  dispatching an undeclared action or calling an undeclared capability is
  a compile error.
- **Triggers and cleanup — the full TaskIR surface.** Plan task rows
  carry every trigger the IR has today: `mount`, `manual`, event-driven
  (§7.7, **new** — today's `TaskIR` union is `mount`/`manual`/`when`
  only) — interval being a lowering onto 0330 §4's timers per §3.3.9,
  not a trigger kind — and reactive `when`.
  Cleanup keeps the shipped contract: cleanups run on invalidation,
  unmount, and reset (`llp/contract/0082`'s occasions), in reverse
  registration order (**LLP 0330 §11.1's rule** — 0082 states when,
  0330 states the order).
- **Cancellation is a generation fence over AUTHORED delivery — it never
  erases effect evidence.** r3 conflated two layers 0330 keeps distinct.
  The split, normatively:

  - **Authored events** (task dispatches, settlement events into app
    state): every task instance carries a generation stamped at start;
    cancellation (unmount/reset/invalidation of the owning scope, or
    `cancel-task`) bumps it, and the fence checks the generation at
    **every continuation resume, immediately before every new
    capability/effect attempt or authored dispatch the body issues**
    (0330 §11.1's awaited-loser rule: no later authored write; r5
    checked only at admission, letting an awaited continuation resume
    after cancellation and issue fresh attempts), at the §7.4 admission
    boundary — **and again immediately before its reducer evaluates and
    before its commit/command release** (§13.3), so a cancellation
    sequenced earlier in the same batch takes effect on later
    invocations already admitted. Evidence is preserved only for
    attempts admitted before the fence. A non-suspending cancelled body
    cannot land authored effects; "a cancelled task never becomes an
    *authored event*" holds, and replay's event log contains no ghosts.
    **Task determinism:** a task captures its environment snapshot and
    RNG substream at task start (keyed by its `TaskSourceIdentity`
    5-tuple, §7.4 — the
    §7.1 rule extended off the event path); task rows carry
    ambient-read masks like actions (§3.3.9), and the replay
    certificate covers task read sites and substreams.
  - **Effect arbitration and evidence** stay with **LLP 0330 §11.1's
    runtime-global `EffectSupervisor`, unchanged**: a capability call
    already issued is an attempt with an `EffectAddressV1` identity and
    attempt ordinal; its terminal outcome and provider evidence commit
    to the supervisor's **append-only** journal whether or not the
    owning scope survives — the fence suppresses the *authored
    settlement dispatch* into a dead scope, never the recorded outcome;
    late/superseded terminals are classified as races, not dropped.
    Physical cancellation of in-flight work is a best-effort command,
    distinct from the fence. Replay consumes the supervisor journal
    alongside the event log, so external effects of cancelled tasks
    remain fully evidenced.
- Timer constructs: `every(1000, tick)` fires as batched actions per
  **LLP 0330 §4** (its owning rule; the §10 deadline queue is the
  signal-deadline machinery — r3's citation conflated the two); both
  families' guarantees are untouched (§1.2).

### 7.7 The paved multi-step form

The mitigation LLP 0481 §4.3 requires before change 3 lands. "Submit,
then on success navigate" in the reducer world:

```contract
task submitOrder from submit
  post("/orders", draft) -> ok(orderPlaced) | fail(orderFailed)

action submit writes status
  status = pending()
action orderPlaced writes status
  status = done()
  navigate(receiptRoute(event.orderId))
action orderFailed writes status
  status = error(event.reason)
```

Normative content: a `task … from <action>` header (task starts as a
command of the naming action); the `-> ok(a) | fail(b)` settlement clause
naming the success/failure actions whose param types the compiler checks
against the capability's declared result type; `navigate` as a command.
**Capture semantics, A-VAL-independent:** the task-start command's
arguments (here `draft`) are the reducer's computed values, **copied at
transition commit** — structurally a handle copy under A-VAL, an explicit deep copy under the
reject branch — so the task sees the submitted value, never later
edits. Free reads in the task body (like `draft`) become **typed
task-start arguments in canonical declaration order**, encoded in the
task row. The §7.8 codemod's obligations include exactly this: locals
spanning multiple awaits become event-payload fields, and `try`/`catch`
maps to the `fail` action.
Surface syntax is subject to grammar review; the construct — one line per
async step, actions for each outcome, no state writes in the task — is
normative, and the line-count bar of 0481 §9.4 (not meaningfully longer
than the `await` body it replaces) is an acceptance criterion of the
change 3 amendment.

### 7.8 Migration

The action-await census (report-only, riding the certificate pass — 0480
§12.2) drives a **mechanical codemod**: an awaiting action splits at each
await into the §7.7 shape; the codemod ships with the change 3 amendment,
covers the enumerated corpus patterns, and every non-mechanical residue
becomes a fix-it-carrying diagnostic. Change 3 does not land until the
codemod's coverage on the shipped corpus is reported alongside the census.

## 8. Change 4, specified — regions

### 8.1 Region kinds

One record kind per construct in the real inventory (LLP 0481 §5.2):

| Kind | Constructs | Discriminant | Arms |
| --- | --- | --- | --- |
| `exclusive` | `when`/`else`, `match`, `match-size` | predicate or scrutinee expr | N known templates |
| `keyed` | `each` | collection + key expr | 1 template + instance table |
| `cell` | `resource` / `async` / `boundary` | cell state machine (§7.3.3) | the construct's real arm set: loading/error/stale/empty/ready for `resource`/`async`; fallback/body for `boundary` |
| `composed` | snippet calls, `pager` | static target (or bounded choice) | callee plan/template refs |
| `graphics` | `graphics` surfaces (`GraphicsSurfaceNodeIR`) | surface region | the scene graph is a referenced sub-program with its own closed grammar (scene when/each/projection kinds), evaluated by the graphics evaluator beside the plan — the §3.3.14 motion pattern, not a flattening into view regions |
| `root` | component root | — | 1 template |

Two IR node families complete the inventory. **`publish` nodes carry
their full shipped shape**: channel expression, imported publish
descriptor (a declared-effect registry entry beside §10.1's
capabilities), compile-checked declared `rate`, optional payload
mapping, **and `heartbeat`/`heartbeatExpr`** — the idle keepalive floor
(re-emission after `1000/heartbeat` ms without a send, preserving
ephemeral presence; analyzer-enforced `heartbeat ≤ rate`; the existing
AOT publish plan already carries it) — with the heartbeat timer's
scheduling on **LLP 0330 §4's timer family** (a §7.4 class-1 timer
source with the timer fill rule — it has an admission class and a
source identity, not an unclassified side channel), and its dependency,
budget accounting, and cancellation-on-dispose following §7.6. Publish remains a declared mutating
effect through the effect boundary — never a bare value binding.
`Hole` / `ChildrenSlot` / `Nothing` nodes are compiler-internal
composition machinery that MUST NOT survive into Regions.

**`pager` is a kind-specific record, not a bare composed target**: its
region row preserves every `PagerNodeIR` field — axis, key policy,
controlled value, ordering, animation policy, navigation expressions,
witnesses, props, guarded cells, fallbacks. **The closure rule
generalizing all of this: IR-to-plan field coverage is a generated
compile-time gate** — the format schema (§3.3) diffs every `ViewNodeIR`,
`ComponentIR`, and `ContractFileIR` member's field list against the
plan encoding as a four-way disposition ledger (§3.3.2a) —
**recursively closed over every reachable IR type and every
discriminated-union arm** (`ActionIR`, `TaskIR`, `Stmt`, pager cells,
motion artifacts included), not just the three named roots — and an
undispositioned field fails the build (this gate is what would have
caught `heartbeat`, `pager`, and the missing module/component tables
mechanically; r3 and r4 hand-audited and each still missed one).

**Cell arms are the construct's real arm set from the IR, not one
triple**: `resource` **and `async`** regions carry the shared
`ResourceSlotKind` arm set — loading/error/stale/empty/ready
(`ir.ts:1355–1377`; r3 corrected `resource` and left `async` on the
stale triple) — and `boundary` carries fallback/body.

Sum-type matching lowers to `exclusive` over the existing `match`
(LLP 0184) — no new kind. **Dynamic component composition** (LLP 0481
§5.4) is expressible only as `composed` over a build-time-bounded
candidate set (a sum of component refs); unbounded by-value choice is
rejected with a fix-it enumerating the closure.

### 8.2 Keyed instance management

The honest algorithm (schema-total, instance-parametric — never tree
diffing, LLP 0481 §5.1): the instance table maps key → row index; on
collection change the runner computes key-set delta (create/dispose),
reorder (order-index permutation), and per-row dirty columns (Deps
column granularity). Creation instantiates the single closed template
against a new row; disposal releases the row and cancels owned tasks
(§7.6). Rows live in §5.3's SoA arena, insertion-ordered and compacted
on dispose; **iteration opcodes iterate key order via the order index**,
so evaluation order is deterministic and runner-independent (§4.2).
Instance create/reuse/dispose is the **metered allocation path** —
counted by §9.2 budgets, reported per update. Dirty-binding evaluation
over scalar and handle columns that constructs no new string or
collection values allocates nothing — a **reasoned target (0480 §6.3's
narrowed claim), not a conformance MUST**: a template-string or
array-constructing binding allocates, correctly, and its allocations are
metered like the instance path. F2's receipts carry the allocation
counters that make the target checkable.

### 8.3 The update algorithm

On each committed transition (under tier-2 dispatch, per invocation in
enqueue order — §13.3):

1. **Dirty marking** — the journal's address set marks slot/field/column
   dirty bits, with **hierarchical invalidation stated, not implied**:
   a field write invalidates exact-field and whole-slot observers,
   never sibling-field observers; a whole-slot write invalidates all
   descendant-field observers; row-column writes analogously invalidate
   the column's, the row's, and the collection's observers. §14.1
   carries the differential fixtures.
2. **Graph sweep** — one pass over the topologically ordered Deps rows
   restricted to the dirty closure; derives re-evaluate; guard exprs
   (§6.3) evaluate before their edges propagate.
2a. **Cursor rebinding and bound-row invalidation** — executed **at
   each cursor's own position within step 2's topological pass**, not
   as a separate later phase, so cursor-dependent derives downstream
   in the same pass observe the newly resolved row in the same
   transition — dirty cursors (an
   `at`-dependency change; a key-set generation change — observed in
   ALL three states, computed before row reclamation; or a write to a
   bound row's observed column, delivered through the §3.3.5
   instance-subscription overlay)
   re-resolve or re-fire against the post-transition snapshot,
   retargeting overlay edges (§6.2, §3.3.11a), ordered by the Deps
   `cursor`-source rows. §14.1 carries the guard-flip,
   discriminant-plus-value, bound-key-removal (the row must not survive
   as `some`), key-field-mutation, independent-cursor duplicate-key,
   reappearance, reorder, and bound-row-column-write fixtures. This step list is **generated from the Deps source-kind table**
   (a Track R gate, like every format-schema product), so adding a source kind without a sweep step is a build
   error, not a prose omission (r4 defined rebinding and left it out of
   this list).
3. **Region toggles** — dirty `exclusive`/`cell` predicates re-evaluate;
   toggled regions emit structural ops (subtree create/destroy from
   templates); dirty `keyed` collections run §8.2.
4. **Binding emission** — dirty bindings evaluate; changed —
   **per `PublicationEquivalenceV1` (§7.5 clause 5), never by handle
   identity** — values encode
   protocol ops (`tests/protocol/protocol-inventory.json` opcodes,
   unchanged).
5. **Flush** — one batch to the presenter per LLP 0186's invariants: batch
   atomicity, glitch-freedom, quiesce-equivalence (pinned at the invariant
   level; flush order stays implementation-defined per LLP 0328 D2).

**The `TransitionArtifact`.** The complete output of steps 1–5 for one
transition is a single atomic artifact: the state journal (§7.5), the
runner-internal deltas (derive memo slots, guard tuples, cursor
bindings, region toggles, keyed-instance table deltas, generation
bumps, meters), the encoded op stream, the command list, and any
trap/verdict. **Rollback (§7.5), worker results (§13.3 — workers
produce the reducer half, the runtime-thread sweep completes it),
speculation entries (§13.5 — precomputed in full), and replay all
traffic in this one artifact** — one commit representation instead of
four near-copies that drift.

Reasoned target (not promotable): steps 1–4 for a five-binding dirty set
on a 10K-node plan are index arithmetic and five expression evaluations —
the mechanism behind 0480 §6.3's p50 < 10 µs hypothesis, decided by F2
(§14.2), never by this paragraph.

### 8.4 First-frame emission

Where a route's initial state is a build-time fact, steps 2–4 run at build
time over the initial state and the op stream is baked (§12.5). Slots with
`initKind: env` (e.g. `state nowMs = now()`) force **bake-and-patch**: the
static frame bakes, and a first-evaluation patch for the env-dependent
closure runs at mount (0480 §6.1's carve-out, normative).

## 9. Change 5, specified — obligations and cost claims

### 9.1 Discharge classes

Contract clauses partition at compile time (`llp/contract/0085` remains
the clause authority):

- **Compile-discharged:** `has` / `missing` / `count` over unconditional
  nodes and statically sized regions; `action … writes` (already static);
  `derive … depends only on` (now checked against the Deps table, which is
  the authority rather than an approximation).
- **Plan-compiled runtime assertions:** guarded visibility, state
  assertions, interaction outcomes — compiled into the plan as assertion
  records that are **zero-cost when disabled** (assertion tables live in
  their own segment; a production runner with assertions off never touches
  the segment — §9.4). Live-agent checks via Acto continue unchanged.
- **Monitor-checked:** per-invocation `writes` verdicts (the runtime
  contract monitor), now reading the journal (§7.5) — exact by
  construction.
- **Live-agent-only, unchanged:** `llp/contract/0085`'s navigation
  clauses (`navigate`, `exits`, press-navigation outcomes) remain live
  Acto checks — properties of a running surface, per LLP 0481 §6.1.

### 9.2 Cost claims

New contract clauses (grammar additions, priced as such):

```contract
contract
  fanout stations <= 12
  cost toggle <= 4 + rows(stations) * 2 ops
  plansize <= 96kb            # route-level, in the route contract
```

- **Fan-out bounds** per derive/state: checked against the Deps table's
  out-edge counts; with §6.1 granularity these measure real width.
- **Op budgets** per action: **symbolic, never scalar** — the form is
  `base + Σ rows(collection) × perRow` over the collections the action's
  dirty closure reaches; the compiler evaluates the symbolic bound against
  the plan (statically) and the runner meters actual emission per
  invocation (dynamically, in dev and in the chaos arm). Keyed-instance
  create/dispose counts are metered in the same unit (§8.2).
- **Plan-size bounds** per route.
- A screen exceeding a declared budget **fails the build**; a metered
  runtime excess in dev surfaces as a contract verdict through
  `exact_contract_verify_claims`.
- **Op-count is a proxy for cost, not cost** (LLP 0481 §6.3, restated in
  the body because the claim ships here): budgets are stability and
  regression instruments; presenting an op budget as a performance
  guarantee requires the measured op-count↔time relationship RFC 0478's
  W4 lane exists to produce, and is forbidden before it.

### 9.3 Budget governance

LLP 0481 §6.3's owed design, resolved for v1: budgets are **inferred on
first green build and ratcheted**. The compiler records the measured
symbolic cost as the implicit budget in a generated neighbor file
(provenance-headed, per the generated-files rule); subsequent builds fail
on regression beyond a tolerance band (the 20% default is **provisional
policy, not a measured constant** — registered with the check and
re-derivable from corpus cost distributions before the ratchet ever
blocks a build); authors promote implicit budgets to explicit contract
clauses with a fix-it. No unwritten budget ever fails a build; no written
budget is ever silently loosened. `native-total` routes MUST carry
explicit budgets (they are the tier whose ceilings are the product —
§11.2).

### 9.4 Zero-cost-off

Assertion and metering machinery MUST be structured so a production
mode-2/3 runner with checks disabled executes no assertion instruction and
touches no assertion segment. The conformance corpus includes a fixture
pair asserting identical op output with checks on and off.

## 10. The capability/effect ABI (the sixth closure)

### 10.1 Declarations

The world boundary is a versioned, typed registry. Phase 0 delivers the
**declaration format** (a checked schema, not the implementations):
`packages/exact-contract/plan/capabilities.json` — entries:

| Field | Content |
| --- | --- |
| id | stable string, namespaced (`net.fetch`, `clock.now`, `nav.push`, `store.kv`…) |
| version | semver; plans record the (id, major) they were compiled against |
| signature | param/result types in the §5.1 type language |
| affinity | invocation affinity (`runtime-thread` \| `main-thread` \| `any`) **plus** the producer/delivery pair for its completion, in `docs/callback-affinity.md`'s actual vocabulary (`background-producer`, `provider-thread`, delivery `runtime-thread`, …) — each ABI entry's completion row IS a callback-affinity declaration and lands in that file's registry |
| purity | `pure` \| `idempotent` \| `effectful` — **a NEW ABI taxonomy defined here** (`pure` = deterministic from declared stable inputs; `idempotent` = repeat-safe, NOT replay-stable; LLP 0206 supplies pure-vs-opaque inference for imports only and does not define these classes) |
| reactivity | `none` \| `reactive` (declared invalidation channel — §6.4) |
| completion | `none` \| `event(type)` (what re-enters the log) |
| tiers | which target tiers implement it |
| disclosure | the §3.3.1 confidentiality class of the capability's results (`client` \| `server-only` \| `secret`) — absent means `server-only`, fail-closed; the lattice join consumes it |

- Contract reaches the world **only** through ABI calls (from commands,
  tasks, and cells) and through host events entering the log. Imports of
  executable `.ts`/`.js` do not exist inside plan modules; the 1,079-import
  residue of the current corpus (0480 §2.2) is exactly what §11 classifies
  and Track L (§15.2) migrates.
- **Registry admission is structural, or F1 is gameable**: a class-2
  entry must be an independently versioned **external-world or platform
  service** — its implementation MUST NOT execute app-authored code and
  MUST NOT read plan-local arena state. An app-owned deterministic
  transformation wrapped as `app.foo` is not a capability; it lowers as
  a plan module/opcode or stays residue. Without this rule an
  application could relabel every non-lowerable helper as class 2 and
  pass the F1 witness with an FFI orchestration shell — exactly the
  degeneration 0480 §2.2's sixth closure exists to forbid.
- The certificate classifies against **declared** entries: an entry may be
  declared-but-unimplemented on a tier (class 2 with a named gap) — this
  is what makes class-2 classification concrete before implementations
  exist (one of the four gaps in the 0480 loop's round-3 F1 finding).

### 10.2 Semantics

Capability invocations are commands or task calls (§7.1); results re-enter
as typed events; `reactive` capabilities expose a versioned read edge for
derives (§6.4). The runner enforces affinity: a `main-thread` capability
invoked from a worker context is a certificate-time error, not a runtime
surprise (§13.4). The ABI's threat-model discipline follows LLP 0333's
boundary schemas; LLP 0476's authority-native-held pattern is the frame
for any capability that carries durable authority.

## 11. Admission: the `PlanAdmissionCertificate` (F1's instrument)

### 11.1 The pass — a conservative, fail-closed classifier

One instrumented build pass over a named scope (a route, an application,
or the corpus) enumerating every executable context (§2's generated
enumeration) and classifying each. The pass runs against the shipped
corpus and real applications — never a fixture authored for it (0480
§12's rule).

**What the Phase 0 classifier is:** a **conservative over-approximating
static analysis over the current IR** — not a shadow compiler of Track L,
not a heuristic that can optimistically pass. **De-circularized (r6's
gap): a mapping-table assertion is not a proof.** Phase 0 gains **I6 —
the proof-only validator designs** (admission type inference,
dependency/effect closure, region disposition, opcode admissibility;
design + validators only, no shipping-semantics change), and the FINAL
I2 receipt requires I5 + I6: **every class-1 atomic requirement row
names the validator and authority digest that proved it — missing
proof is class 5**, and the final receipt's `opcodeTableDigest` is
required, not optional. An interim census MAY run earlier,
report-only, carrying no verdict. The published **mapping table**
(part of the certificate schema, committed with every receipt) maps IR
construct to classification: `await-assign` → census
channel; `items[i]` → class 1 + widening axis; `.ts`/`.js` import →
class 2 if I1's registry declares it, else class 5; `publish` → class 1
with its declared-effect row (§8.1); **graphics and motion →
`sub-program`, never class 1** (their encodings are referenced
evaluators, §3.3.14/§8.1 — counting them as plan opcodes before the
encoding exists would be exactly the optimistic laundering this rule
forbids); and so on across the §2 taxonomy. **A construct or context
kind absent from the table is class 5** — never class 1 by optimism, so
the instrument can under-promise but cannot launder a pass.

### 11.2 Classes, constructively

Incorporating the three-round convergent finding of the 0480 loop (the
kill class must be constructive, class 4 must be tier-dependent, coverage
must be a verdict axis) — plus this loop's round-1 correction: **a class
rollup alone cannot carry the verdict**, so the primary artifact is
§11.3's per-axis matrix and the five classes are its summary.

- **Class 1 — plan opcode.** Lowers fully into §3's tables under §§4–9's
  rules. Dynamic indexing lowers via §6.2 and is class 1 **and** reported
  on the widening axis — lowering does not remove it from the verdict
  (§6.2's proposed 0480 amendment).
- **Class 2 — capability ABI**, with a **three-state status per route
  tier**: `declared` (registry entry exists), `implemented-on-tier`,
  `deployment-admitted`. Only implemented-and-admitted entries count
  toward **executable coverage**; `declared`-with-gap reports separately
  in the projected/declared numerators (§11.3) with a named
  implementation gap — without the split, a route
  could reach 100% "coverage" while unrunnable.
- **Class 3 — server-only.** Admissible only for routes whose deployment
  declares a server evaluation locus (LLP 0482 §6 placement, LLP 0483 SSR);
  a `native-total` route with no server locus cannot hold class-3
  contexts.
- **Class S — referenced sub-program** (a proposed addition to 0480
  §12.2's taxonomy): motion and graphics contexts, evaluated by a
  declared, versioned sibling evaluator the plan references (§3.3.14,
  §8.1). Counted separately, **never inside class-1 coverage**;
  admissible per tier by that evaluator's availability on the tier
  (`native-total` needs the compiled evaluator, not a JS-hosted one).
  **Class S splits in two, because the data/code line is real in the
  shipped IR:** **S-data** — closed declarative sub-program IR (the
  graphics scene grammar is the candidate) — versus **S-code** —
  executable generated source: current Motion worklets are
  `installFormat: 'source-utf8'` JS generated from the authored action
  body (`ir.ts:1568`; r6's font-shaper analogy called that "data",
  which was false). Only S-data may ever use the class-2 fallback
  (an evaluator of closed data satisfies §10.1's structural rule the
  way a font shaper does); **S-code can NEVER ride class 2**. Under
  the governing 0480-as-written five-class verdict, S-code classifies
  class 4 where the tier admits an escape and class 5 otherwise —
  honestly reported, not gamed around Motion-heavy apps; Class S
  appears only in the proposed-rule verdict column until 0480 adopts
  it. Evaluator absence is a named implementation gap either way.
- **Class 4 — explicit escape.** A context marked with an explicit,
  greppable, reason-carrying escape annotation at its source site, on a
  tier whose admissibility matrix admits escapes. Phase 0 fixes the
  **canonical IR-level annotation and certificate representation** now;
  only the surface spelling waits for grammar review (§18).
- **Class 5 — rejected.** Constructive definition: a context that (i) does
  not lower under §§4–9, (ii) does not resolve to a declared ABI entry
  admissible on the route's tier, (iii) is not server-placed under the
  route's declared deployment, (iv) carries no explicit escape
  annotation admissible on the route's tier, **and (v) is not a
  declared Class S sub-program context**. **"Every context runs as JS
  today" no longer makes class 5 unreachable: an unannotated
  non-lowerable context is class 5 by definition, and annotation is a
  visible, countable author act.**
- **The census channel — proposed as an amendment to 0480 §12.2's rule,
  put to its adjudicator.** A context whose *only* bar to lowering is a
  mechanically-codemod-able await (§7.8) classifies as its underlying
  class with `pendingMigration: await-split`, counted in the census —
  **never class 5, never silently class 1**. Without this channel the
  constructive definition would kill on awaits, contradicting the
  report-only census rule 0480 §12.2 / LLP 0481 §4.3 already fixed; with
  it, class 5 kills only on contexts with no accepted disposition. The
  same channel carries A-VAL's migration census (§7.5).

**Tier admissibility matrix (normative):**

| Tier | Class 1 | Class 2 | Class 3 | Class 4 | Class S |
| --- | --- | --- | --- | --- | --- |
| native-total | ✓ | ✓ | only with declared server locus | **✗ — an escape forces class 5 or forces the route off the tier** | ✓ where the compiled sibling evaluator exists on-tier |
| native-mixed | ✓ | ✓ | with locus | ✓ (annotated) | ✓ (evaluator available) |
| web | ✓ | ✓ | with locus | ✓ (annotated) | ✓ (evaluator available) |
| server | ✓ | ✓ | ✓ | ✓ (annotated) | ✓ (evaluator available) |

### 11.3 The per-axis admission matrix, with coverage as a verdict axis

**The certificate's primary artifact is a per-axis matrix, not one
5-class number** — collapsing awaits, widening, and genuinely
unloweable contexts into one class number is what made every prior
version of this gate unfireable. Rows are the admission axes
(state/value layout; dependency resolution; import lowerability; ABI
coverage; task/resource transitions; motion/timer/lifecycle; effect
signatures; region inventory; **widening** (§6.2); **A-VAL migration**
(§7.5)); cells carry counts, fractions, and fail-closed residues. The
adjudicated verdict rule (0480 §12.2's, as amended there if the
proposals land) is a **predicate over this matrix**. Reported, per route
and per application, never pooled:

- the matrix itself, plus context counts per class;
- **three nested cumulative coverage numerators** per route (r3's
  "non-overlapping" was wrong — each contains the previous), over the
  same denominator: **tierExecutable** (class 1 + class 2 at
  implemented-AND-deployment-admitted + placed class 3 + Class S where
  its evaluator is available on-tier; **a pending-migration context
  counts at its underlying class** — the await census stays report-only
  for F1, per 0480's rule that no count of awaits can kill change 3;
  r5 excluded them here and thereby turned the census into a kill
  condition — retracted. `migrationOutstanding` is a separate
  report-only count, and "zero outstanding" is a Track L/R
  implementation gate, never an F1 input. During Phase 0 this numerator
  is honestly a **static lowerability-and-runnability projection**,
  class-1 runners not existing yet), **projected** (adds declared
  class 2/S with a committed implementation), and **declared**
  (everything with any disposition).
  **planOnly** — the class-1 fraction alone — is reported beside them as
  the D1 *representation* property. Multi-site contexts (an action with
  several effect sites of different tier status) emit **atomic
  requirement rows** beneath the parent context — opcode fragments,
  capability call sites, placements, sub-program refs — with the
  parent's class as the rollup of its rows, so one coarse row can never
  average away a missing capability. Implemented-and-admitted is the
  executable class-2 rule everywhere. This spec sets no floor, but it
  does **propose a pre-registered D1 witness predicate to 0480's
  adjudicator** so the verdict is a predicate, not a later inspection:
  at least one real shipping application whose primary tier shows zero
  class 4 and zero class 5, tierExecutable = 100% on every route
  (pending-migration counted at underlying class, above), **every
  app-owned deterministic computation classified 1 with class 2
  confined to §10.1's structural admission rule** (the qualitative
  half, so class 2 cannot launder computation), **and zero unbounded
  widened sites — every `dep_widened` site carries an
  adjudicator-accepted symbolic bound** (a first-green inferred budget
  is NOT acceptance; this is the concrete widening predicate r5's
  "hazardous at scale" lacked). No `planOnly` numeric floor is
  invented; any later percentage bar is pre-registered after the
  census, per P7. **Until 0480's owner adopts this delta, every
  certificate emits TWO verdicts — one under 0480 §12.2 as written,
  one under this proposed rule** — so the governing gate is never
  silently replaced. This
  witness is **an explicit proposed delta to 0480 §12.2's pass rule**
  (which as written allows class 4 and reads one aggregate) — including
  the open sub-question of whether class 4 stays allowed on
  non-`native-total` primary tiers; 0480's adjudicator settles both
  predicates as one rule. Each numerator carries a
  **denominator-completeness attestation** (the §2 authority version the
  pass ran under), so no verdict is taken on a partial denominator;
- the class-4 inventory (site, reason, tier) and the class-2
  implementation-gap list;
- the census channel (awaits with §7.8 codemod coverage; A-VAL
  migration sites) — report-only;
- the widening axis detail (widened-edge count, symbolic width,
  collection-granular share);
- per-plan layout-fingerprint counts (§5.5) and plan-size projections.

### 11.4 Format and registration

`PlanAdmissionCertificate` v1 is a JSON document:
`{ version, receiptKind: "interim" | "final", capturedAt, scope: {app,
route}, tier, compilerDigest, enumerationVersion, mappingTableDigest,
opcodeTableDigest (REQUIRED when receiptKind = "final"; optional on
interim census receipts), axes: {…},
contexts: [{site: {file, span}, kind, class, capabilityStatus?,
pendingMigration?, reason?, escape?, abiRef?, subProgram?, widening?}…],
coverage: {tierExecutable, projected, declared, planOnly,
countsByClass}, requirementRows: [{parent, kind, class, abiRef?,
subProgram?, placement?}…],
census: {awaits, codemodCoverage, avalSites, undefinedSites},
dispositions: […] }`.
The pass registers as `plan-admission-certificate` in `exact-verify.json`
on first run (no such check exists today — inspected by the 0480 loop,
2026-08-18); receipts commit under `docs/reports/` with pinned capture
configuration. The check itself is report-producing, never
threshold-enforcing: **the verdict rule lives in 0480 §12.2 and is
adjudicated by Charlie**; this section only guarantees the verdict has the
inputs the rule names.

## 12. Runners

### 12.1 Mode 1 — the dev runner

> **DECIDED since freeze (2026-08-20):** there is **no distinct Mode-1
> dev runner**. LLP 0500 D2 (Active) decides dev runs the **production
> runner plus a plan-patch channel and an introspection layer**; HMR is
> a plan patch under the HotRevision spine matching on declaration
> identity per **D1 as amended 2026-08-20** (the tuple parenthetical
> below is the pre-amendment text). This section's introspection and
> reset-report obligations survive as requirements on that
> configuration, not as a separate runner.

1. **Full introspection.** The dev runner exposes the plan's tables,
   live dirty sets, per-update metering, and the dataflow inspector;
   Acto's surfaces read from it; wire names remain `exact_*`.
2. **HMR = plan swap + state migration.** On edit, the compiler emits a
   new plan; the runner matches slots by **declaration identity alone**
   (§3.3.16 identity 1), then applies the versioned **compatibility
   relation** between old and new type fingerprints: `equal` carries the
   value forward; `compatible-widening` migrates (added field with a
   default — optionality is representable only as an explicit presence
   sum (`some(T) | none`), since §5.1's closed objects have no
   optional-field encoding; added sum arm; widened string-enum);
   anything else resets to initializers. The runner emits
   a **reset report** (JSON: kept/migrated/reset per slot, with loci) as
   an agent affordance (0480 §9.5). Reasoned target: sub-100 ms including
   native apply; not promotable.
3. **Editability (0480 §15.2 resolved).** The plan is never the editable
   artifact. Design Mode and the LLP 0387 sidecar operate on source/IR;
   the dev runner regenerates plans incrementally per edit. The LLP 0166
   slot catalog remains derived-never-stored; its derivation input becomes
   the plan's Slots table.

### 12.2 Mode 2 — the fixed production runner

- One app-independent binary per platform: the §4 VM, §8.3 update loop,
  §7 dispatcher, the LLP 0330 §10 deadline queue, journal/commit
  machinery, the protocol encoder. No codegen, no JIT.
- Threading per LLP 0297, unchanged: runner on the runtime thread; one
  persistent UI worklet runtime on main; async-first module rules
  intact; every new callback declares affinity in
  `docs/callback-affinity.md`.
- Scheduler invariants: LLP 0186's batch atomicity, glitch-freedom, and
  quiesce-equivalence hold observably; §7.4 is the event-level contract
  above them.
- Memory: plans map read-only and shared; per-surface incremental cost
  is state arena + instance tables + platform view objects (F4 measures
  this; nothing here promotes it).

### 12.3 Mode 3 — AOT (specified, gated)

The code generator is **generated from the opcode authority** (§4.1):
each opcode's emission template produces Rust; a plan compiles to a crate
invoking the same runtime services (journal, queue, encoder) as mode 2.
Divergence defense is §14.1's corpus, run across modes on every
promotion. Mode 3 builds nothing before: F2 passes, mode 2 is
conformance-green on the corpus, and a separate go decision (0480 §15.6)
is taken — its candidate justifications (OS shells, widgets, watch,
embedded) are 0328's deployment-ceiling case, not update-path speed.

### 12.4 The web runner (LLP 0483, specified)

> **DECIDED since freeze (2026-08-20, LLP 0505 r3 row 1 / RFC 0483 §2 =
> option (b)):** the web runner is the **same Rust engine compiled to
> wasm over real DOM**, gated on corpus green + the TS-seam cutover
> gate — never a JS-language runner. This section's runner-fixedness,
> size-budget, decode, resumption, and SSR obligations carry over to the
> wasm runner; "a fixed JS runner" is the superseded language choice.

1. **Artifact.** Same plan bytes; a fixed JS runner over real DOM
   (LLP 0288 posture unchanged). Runner fixedness is enforced by a
   registered size-budget check (`web-runner-size-budget`) with
   **infrastructure governance, not §9.3's route ratchet**: a fixed
   absolute cap, raised only by an explicit author decision recorded with
   the check — a shared runner must not auto-ratchet upward on its own
   growth. The LLP 0483 §9.1 drift risk gets a gate, not a hope.
2. **Decode strategy (0483 §9.2 resolved):** the runner MUST support
   zero-copy indexed access straight off the fetched `ArrayBuffer`
   (lazy); it MAY build accelerated typed-array views for hot tables at
   idle. The choice is per-route by size heuristic; both paths are
   conformance-identical.
3. **Resumption** — plan-directed, three named steps (0483 §3): **adopt**
   server-emitted DOM against the Nodes table (SSR annotates DOM with
   node ids); **restore** the slot array and instance tables from the
   serialized-state payload (§3.7's schema-derived encoding —
   a schema-derived dense encoding of the client-class projection,
   typed by §5.3); **bind** handlers lazily — root event
   delegation from load, per-node resolution on first dispatch. Handler
   identity is `(planDigest, host-handler TargetSiteId (§7.4's
   host-binding variant — resumption is host-input-only by nature),
   instance path/row key, action id)`; curried arguments re-evaluate at **DOM/event-intake
   dispatch** (§3.3.10 — the intake moment that mints §7.4's target
   stamp, distinct from inbox dispatch, which is the later fence
   point), never captured at render. A page/plan digest mismatch falls back to
   full render — never mixed-version resumption. **The client-resumption
   projection is the `client`-class slots (§3.3.1)**: a route whose
   client-executable dependency closure reads a `server-only` or
   `secret` slot is not client-resumable — the compiler rejects it (or
   the reading region is server-placed, §13.6) rather than shipping
   forbidden state or an unreconstructable arena.
4. **Chunking and deferral.** Route-level plans fetch on navigation
   (LLP 0154/0160 single-source routes). Region-level deferral is legal
   **only** for `cell` regions (async-by-construction, with a declared
   pending arm) or under an explicit `lazy` marker (priced as a semantic
   addition); a bare `when` slice is never deferrable (0483 §4's r3
   restriction, normative).
5. **SSR and streaming.** Server rendering = region evaluation → HTML
   segments → rope concatenation (§13.2's partition). Two declared modes
   per route: `stream: document-order` (no placeholders, no client
   receiver; compatible with a no-JS guarantee; head-of-line risk
   accepted) or `stream: backfill` (placeholders + receiver; spends the
   no-JS window). The compiler rejects `backfill` on a route declaring a
   no-JS guarantee.
6. **Forms and progressive enhancement — `llp/contract/0091` is the
   form-IR authority; this spec PROPOSES its spelling.** 0091 (forms,
   validation, and error boundaries) owns the semantic form IR —
   submission groups, successful controls, `formId` (LLP 0060),
   enhancement policy (`runtime-only | progressive-web`), encoding,
   ordered controls — and **explicitly left the source spelling open**
   ("a built-in `form` view node or a compiler-recognized first-party
   component descriptor … a notation decision"; r4 wrongly wrote the
   tag as "0091's spelling decision"). This spec's built-in `form`
   container tag is therefore **0485's proposed resolution of that open
   notation decision**, put to the Contract form/grammar owner and
   listed in §1.2's inventory; **reject branch:** a recognized
   first-party component descriptor (0091's `RouterForm` sketch) emits
   exactly the same semantic form record, and everything below binds to
   the record, not the spelling. PE classification binds to 0091's
   record plus LLP 0348/0335 (to which 0091 transferred progressive
   transport and security). Native semantics (grouping, default-submit, AX role) ride
   the tag per 0483 §9.7. Lowering: through `CONTRACT_TAG_LOWERING`
   like `section` — semantic tag plus AX role; **"no new protocol op"
   holds only insofar as 0091's host/SSR facts fit existing ops, and
   any new op is 0091's to propose**. The existing `submit=` Enter-key
   attr on inputs is untouched; the form's document-submit binding is
   its own `action=` attr — it names a Contract action, and the
   compiler derives the HTML `action`/`method` pair, default-button
   resolution, and successful-control/`name` rules from the 0091 IR,
   with typed field encoding/decoding per 0091's record. Exactly one
   submit boundary per flow. The PE classifier certifies a **complete
   flow**: 0483 §6's conditions (submit boundary; successful controls
   covering payload + curried args; request-time deployment;
   server-admitted effect signature; the full response protocol — 422
   re-render with errors and prior values, POST/redirect/GET, staleness
   refusal) **plus CSRF/origin evidence and idempotency keys for
   retried POSTs**.

   **The server-replay state locus, safely.** The lattice is §3.3.1's
   slot confidentiality column — a **total join**: derived values carry
   the max of their inputs' classes, so the class is computed, never a
   default a derive of server-only data can inherit. `client` means
   **client-disclosable** — a permission, not evidence the value was
   already rendered (r4's "page-visible, never disclosure" claimed
   exactly that and was untestable: a `client` slot the submit action
   reads but the view never paints would have entered the HTML). The
   > **Maturity marker (stated because thirteen review rounds proved
   > it needs stating): everything from here through the atomic replay
   > machine is PROPOSAL-STAGE text — the §1.2 inventory's
   > 0091/0348/0335/0060/0067 amendments — finalized by those owners'
   > adoption processes. It is not an implementation input until
   > adopted; PE implementation MUST restart from the adopted owner
   > text, and this spec's gates already enforce exactly that.**

   POST carries **two separately-trusted parts** (r5 wrapped
   user-typed controls in the signature, which authenticates nothing —
   the user edits them after render): (1) 0091's ordered successful
   controls, transmitted as ordinary **untrusted `FormData`**, decoded
   and type-checked server-side against 0091's record; and (2) the
   signed **preservation capsule** — only server-emitted values:
   `client`-class slots in the submit action's replay closure that
   carry an **explicit envelope opt-in marking** (an authored
   slot-level marking on the **proposed** envelope-opt-in field of
   0091's semantic form record — inventory (d)'s second 0091
   amendment; the record is its authority and certificate
   representation once adopted; absence fails closed), version witnesses
   (below), application, route, plan + catalog digests, state epoch,
   expiry, an anti-replay nonce, and the user/session where one exists.
   Everything else re-derives server-side or forces the session locus;
   the server combines (1) and (2) only after validating both. §14.1
   carries the fixtures: a derive over a `server-only` slot cannot
   appear in any capsule, and an unmarked `client` slot read by the
   submit action stays out. **Anti-replay and idempotency are distinct
   and both named**: the nonce is one-time (a bounded consumed set —
   rejection semantics); an idempotency key additionally returns the
   stored outcome for a retried POST (result-retention semantics).
   **Progressive flows require 0091's statically analyzable typed
   routed target** — 0091 rejects local callbacks for progressive
   forms, so `action=` naming a local Contract action is the
   runtime-only form, and the progressive form carries the routed
   target record; Track W's PE half additionally gates on the
   0091/0348/0335/0060/**0067** owner amendments being adopted. Confidentiality
   is the lattice: `server-only` values re-derive at request time,
   `secret` values never leave the server. **Re-derivability is a predicate over replay *stability*, not
   retry safety** (r3's "idempotent" test was the wrong property —
   an idempotent read can legally return different data between render
   and POST): a `server-only` value is stateless-re-derivable iff it is
   a function of route params plus capability reads that are either
   `pure` (§10.1's class; LLP 0206 supplies pure-vs-opaque inference
   for imports — the §10.1 purity taxonomy itself is NEW ABI metadata
   defined here, not 0206's) or **version-witnessed** — the read's
   version/snapshot token is signed into the envelope and revalidated
   at POST, with mismatch → staleness refusal. A flow reading any
   mutable unversioned value is classified **session-locus-required**
   (a real server session store), never silently serialized.

   **The atomic replay state machine — THE proposed owner-amendment
   text, in full** (r8 promised it "below" and never wrote it):

   1. **Authenticate first**: validate the capsule — schema, signature,
      expiry, application/route/plan/catalog digests, principal,
      action, `formId`. No cache read, no nonce consumption, no control
      decoding before this step (a cache read before authentication
      could disclose or replay another principal's outcome).
   2. **Decode** the untrusted controls through 0091's record; compute
      the canonical request digest over decoded controls + capsule.
   3. **Overlap rule**: a control-backed slot appearing in both the
      capsule (render-time value) and `FormData` (user-edited value) is
      **rejected** — unless the semantic form record explicitly maps
      that control as the authoritative update applied over the signed
      base state.
   4. **Atomic lookup** on `(principal, action, idempotency key)`:
      a matching retained digest returns the retained RFC 0067 outcome
      **even though its nonce was consumed** (this ordering — cache
      before nonce — is what makes the identical retry work); a
      different digest under the same key rejects as a conflict; a
      fresh key **atomically reserves the key and consumes an unused
      nonce**, then executes once.
   5. **Concurrency and failure**: identical concurrent requests
      single-flight on the reserved key; same-flight waiters receive
      the leader's terminal result **including a true failure** (which
      is not retained post-flight — 0067's rule — and whose nonce stays
      consumed); the reservation releases or marks per 0067's
      lifecycle.
   6. **422 rerender mints fresh, once**: every validation-failure
      rerender embeds a successor nonce and attempt key minted **once
      and stored with the retained 422** (replaying the retained
      rerender re-serves the same successors — repeated replay never
      mints an unbounded successor family), retaining `formId`; an
      ordinary corrected resubmission is never an idempotency conflict.
   7. **The anti-replay/coordination store is its own — explicitly NOT
      RFC 0067's receipt cache** (0067's mechanism is process-local,
      TTL-bounded, capacity-bounded, and retains no true failures, by
      design). **Step 4 is restated as a TOTAL function over
      `(nonce state × outcome state × key/digest)`** — r10's three arms
      omitted the state 0067 produces constantly: **any consumed nonce
      with NO retained outcome — true failure (0067 retains none), 0067
      TTL expiry, capacity aging, outcome-cache restart, or a crash
      between execute and retain — is one terminal:
      indeterminate/rejected, never re-execute.** That rule **amends
      0067's retry-after-failure-executes behavior for PE capsules and
      is filed as such in inventory (f)** (reject branch:
      0067-as-written, no PE claim); agent `mutate()` keys stay
      agent-generated per 0067 — PE capsules minting server-side keys
      is the other named 0067 delta. **Successor credentials are execution-safe only where
      non-execution is proven**: a retained 422 mints executable
      successors (validation proved the operation never ran), and
      `idempotent`/`pure` flows may mint them freely — but for an
      `effectful` flow, a 500/timeout/indeterminate outcome mints NO
      executable successor and the response MUST NOT embed a new
      executable capsule; it is an explicit reconciliation document.
      The blocking key gets the TargetSiteId treatment — **the durable
      `BusinessOperationId`, with a causally possible lifecycle**
      (r13's "minted at first reservation" could not be embedded in
      the capsule that precedes reservation): **operation issuance is
      step 0 of the machine** — for an effectful PE flow the server
      atomically mints and persists the operation record at
      RENDER time, BEFORE signing the initial capsule, as
      `(principal, action, formId, server-issued operation id)`.
      **Recovery model, chosen**: the namespace enforces **one
      unresolved operation per canonical
      `(principal, action, formId)`** — so a bare fresh GET of the
      same form while an operation is unresolved rediscovers and
      re-embeds the same operation id by namespace lookup — and the
      reconciliation document additionally carries a **stable resume
      handle** for cross-context recovery; two *independently
      intended* concurrent operations on one form therefore require
      distinct `formId`s, and **effectful PE flows require a
      non-optional stable `formId`** (part of inventory (d)'s 0091
      amendment — 0091 keeps `formId` optional today). Abandoned
      render-time issuances expire with their capsules. **Step 4's
      atomic lookup tuple, stated once and completely:**
      `(principal, action, idempotency key, BusinessOperationId)`,
      with the nonce state and canonical request digest as validated
      inputs — an indeterminate record for a `BusinessOperationId` is
      blocked-until-reconciled **even on a fresh attempt key**,
      closing 0067's agent-generated-key path and the
      new-capsule-from-a-fresh-GET path, which would otherwise both
      hit the execute-once arm and re-run the provider. §14.2's
      harness asserts provider invocation count = 1 across the
      lose-outcome-then-submit-successor path — constructible now —
      plus initial-GET→POST issuance, two independently intended
      concurrent operations (distinct `formId`s), abandoned
      render-time issuance expiry, and lost-outcome followed by both
      bare and resume-handle-bearing fresh GETs. Retry-after-500 is never an unlisted arm — it lands in
      blocked-until-reconciled. The deployment manifest picks one of two
      guarantees:
      **(a) shared mode** — one **atomic replay-coordination record**
      per canonical request namespace, holding the digest, nonce
      state, reservation/in-flight owner, terminal-or-indeterminate
      state, the retained outcome (or a durable outcome reference),
      the once-minted executable **422** successor credentials, and — for
      indeterminate/500 — a **non-executable reconciliation handle**
      (r12's record still stored "500 successor credentials", an r11
      leftover the r12 decision text had already outlawed), observed
      and waited on by
      **every** replica allowed to accept the capsule (a shared nonce
      ledger alone cannot give cross-replica single-flight or
      retained-result returns — the record is the transaction
      boundary); issuer-affine routing of both coordination and
      outcomes is the permitted alternative — with the SAME durable
      record semantics (sticky routing to process-local memory is not
      mode (a); it collapses into mode (b)) and defined
      issuer-unavailable behavior (fail closed as (b) restart does). **Only mode (a) supports
      "never re-execution of the business operation."**
      **(b) issuer-bound mode** — the capsule binds an issuer instance
      identity; after that issuer restarts (or on any other replica)
      authentication fails closed into a fresh rerender with new
      credentials. **(b) is anti-replay for one issuer generation
      ONLY**: a user who succeeded, lost the redirect, and resubmits
      the fresh rerender performs the operation again — so
      PE-classified flows whose submit action is `effectful` REQUIRE
      mode (a); under (b) the capability must be `idempotent` or
      `pure` so a post-restart resubmission is provider-safe. Under
      either mode: a consumed nonce remains rejecting until capsule
      expiry (live tombstones never evicted); capacity pressure
      rejects new issuance/admission, never evicts a live tombstone.
   8. **Namespace**: the lookup tuple's `principal` and `action` are
      canonical identities that **subsume RFC 0067's namespace** —
      served app, target platform, credential fingerprint; route,
      `formId`, method, and body hash bind through the canonical
      request digest — so plan-local action ids can neither collide
      across apps nor manufacture false conflicts.
   9. **Fixtures — a dedicated registered check**,
      `progressive-replay-conformance` (§14.2's Track W row; extending
      the owner's check if 0067's owner lands one): hostile capsule,
      concurrent identical, retry-after-success,
      **retry-after-non-retained-failure, outcome-TTL/capacity expiry
      with a live nonce tombstone, mode-(b) restart followed by
      resubmission of a succeeded effectful mutation (must be refused
      by classification, not executed)**, 422-then-corrected, overlap,
      expiry, capacity, restart, cross-replica, and crash-window
      cases. Multipart/file controls carry LLP 0475's
      accepted obligations through 0091's file-value conformance rules.

   Retention classes, TTL/capacity, and durability of *outcomes* are
   RFC 0067's; this machine **reconciles with 0067's lifecycle by
   NAMED amendment** (inventory (f) changes its retry-after-failure and
   key-minting rules — "never competes" would overclaim), and supplies
   the anti-replay ledger 0067 never claimed to own.
   Strict-freshness flows are session-locus flows. Capability authorization per §10.2 is
   server-derived, never inferred from the client having rendered the
   button. Envelope key management follows LLP 0476's frames; its design
   note is a named pre-ship deliverable (§18), and **PE does not ship
   before it and the form construct exist** (0483 §6's concession,
   preserved). Routes may declare a no-JS guarantee; the build then
   lists every non-conforming flow and stray interactive binding.
7. **Style lowering — a named Track W gate deliverable.** LLP 0483 §9.3
   requires resolved Facet tokens to become **real CSS with a real
   cascade**, not inline styles, or accessibility and print break. This
   spec requires and scopes the style artifact without fully designing
   it: static rule extraction into real stylesheets with deterministic
   class identity; dynamic values via CSS custom properties bound by the
   runner; theme switching through cascade layers over the §3.4 theme
   table; media/print preserved; SSR emits the stylesheet, resumption
   adopts it; third-party DOM mutation tolerated. Track W has no runner
   work (that moved to Track R at r3); the wiring that survives the
   move: **this section is the requirements note, and it does NOT
   itself satisfy the gate** — the gate artifact is a separate, owned
   **web style-lowering design deliverable** (owner: the Track W lane
   lead at kickoff) meeting these requirements; that deliverable gates
   Track W's start (§1.3) **and is also a start condition of Track R's
   F6 item** — F6 measured against
   inline-style payloads would compare the wrong artifact (0483 §9.3),
   so the capture waits for real-CSS lowering.

### 12.5 The first-frame artifact

Unifies with LLP 0307 Phase B (shipped, IR-emitted) rather than replacing
it: the emission input moves from per-construct IR lowering to the §8.4
table scan; the container format stays 0307's. Coverage MUST be strictly ≥
Phase B's on the same fixtures (F3's no-parity-escape half). Baked frames
are emitted per resolved-theme digest and per locale where used (§3.4,
§3.6);
env-dependent slots take bake-and-patch (§8.4).

## 13. Parallelism admission (LLP 0482, specified)

### 13.1 Output determinism and replay metadata

Normative invariant (0482 §1): under every admitted strategy on every
machine, `state′`, the op stream, exception behavior, command order, and
commit order are identical to sequential dispatch in enqueue order.
Strategy (path choice, worker count) is pinned by a per-hardware-class
certificate fixed at build/install time or **fails closed to serial**;
whichever ran is recorded in per-batch replay metadata. Thermal and
charge gating may vary strategy, never output; feedback-driven cutoff
tuning is forbidden. A permanent `--sequential` mode exists on every
runner; any divergence from it is stop-the-line (F7).

### 13.2 Tier 1 — the partition table

The compiler emits, per route, a partition segment: regions with input
sets and output spans. Server/build renderers evaluate regions
independently and concatenate segment ropes (LLP 0482 §2.2). Regions
sharing a data fetch are grouped mechanically from input-set overlap
(0482 §2.4's owed grouping, part of partition emission). Composes with
LLP 0351's strategy map per 0483 §5.

### 13.3 Tier 2 — conflict-free batch dispatch, v1

> **Maturity marker: this tier's dispatch contract is PROPOSAL-STAGE
> text — held on 0482's owner deciding the §1.2 deltas, plus the 0330
> inbox amendment — and is not an implementation input until adopted;
> the tier stays dormant regardless of any other gate.**
>
> **DECIDED since freeze (2026-08-20, LLP 0505 r3 row 8 / RFC 0482
> r6):** the decision came back **rejected-branch**: the inbox
> amendment is declined, tier 2 ships **replay-only** for v1, and the
> live dispatch contract below stays recorded design, not
> commissioned work.

- **Invocation signature, fail-closed:** R = action's composed read set ∪
  handler-prelude reads (§3.3.10) ∪ event-payload reads; W = composed
  write mask; plus commands and capability targets as conflict domains.
  An invocation whose signature cannot be closed (any escape-class
  content, any unresolvable callback row) **dispatches serially** —
  ineligibility is never an error.
- **v1 schedule:** the maximal contiguous run of mutually conflict-free
  invocations at the batch head (Bernstein R/W test as two bitmask
  intersections per pair, **over the slot table** — 0482 §3.2's
  machine-word grain, deliberately not field-granular per §7.5),
  barrier, repeat — the strictly-correct simplification of 0482 §3.1;
  the precedence-DAG scheduler is a later upgrade behind the same
  oracle. **Each successive run snapshots state after all preceding runs
  commit** — "pre-batch snapshot" means pre-*run*, never the original
  whole-inbox state.
- **Per-run admission — 0482 §1's mandatory predicate, bound here**
  (r12 stated it for tiers 3/3b in §13.4 and left tier 2 to infer it;
  a globally-enabled tier without the per-run predicate would pay
  snapshot/dispatch/join below break-even on the common cheap batch,
  which is exactly the latency violation 0482 §1 forbids):
  1. the run's **deterministic work metric measures REDUCER
     EVALUATION, the only work tier 2 parallelizes** — a separate
     compiled reducer-cost model, NOT §9.2's op/dirty-closure budgets
     (r13 used those, which mostly price the *serial* downstream
     sweep: a cheap reducer dirtying 10,000 bindings scored high while
     its parallelizable work was nil);
  2. admission structurally requires **`run.length ≥ 2` and ≥ 2 usable
     workers** — an above-cutoff singleton has zero reducer
     parallelism and stays serial by construction;
  3. the hardware-class **certificate compares certified serial
     reducer cost against estimated parallel critical-path reducer
     cost plus snapshot/dispatch/join overhead, width- and
     skew-aware**; the v1 simplification is admitting only repeated or
     sufficiently uniform action shapes against width-specific
     measured cutoffs, with every heterogeneous run serial;
  4. **the serial inline path is the default and performs NO
     `ArenaSnapshot`, worker submission, or join** — taken whenever
     the certificate is absent, the run is ineligible, or the
     comparison does not strictly favor parallel; a hardware class
     with no qualifying certificate stays serial, period;
  5. replay metadata records the predicate inputs and the decision;
  6. F7 carries fixtures at `cutoff − 1`, `cutoff`, and `cutoff + 1`,
     ordinary singleton/two-event batches, an **above-cutoff singleton
     (must stay serial)**, a skewed two-event run, an
     equal-total/different-distribution pair, and a
     high-op/cheap-reducer case — proving the zero-dispatch path is
     taken wherever the predicate requires it.
- **Hold-and-reject (the A-VAL treatment, §1.2):** tier-2
  implementation holds until 0482's owner decides the per-invocation
  command-release and per-source-ordinal deltas. Reject branch: the
  tier stays **dormant** — this spec does not implement 0482-as-written's
  batch-then-commands model, whose deferred command timing is exactly
  the oracle weakening §13.3 refuses; a rejected delta means no tier-2,
  not a weaker tier-2.
- **Execution — parallel evaluation, serial observation, over a real
  `ArenaSnapshot`.** A-VAL makes *values* immutable; the slot/row arena
  is still mutable fixed-offset storage, so "free snapshot" was r4's
  overclaim — a mechanism is specified instead. An **`ArenaSnapshot`**
  is an epoch of the arena taken at run start: copied scalar/handle
  tables (values behind the handles are immutable under A-VAL, so only
  the tables copy) or page-level copy-on-write — either way, workers
  hold **leases** on the snapshot, and **the runtime thread MUST NOT
  mutate or reuse storage a leased snapshot can read**: a commit writes
  a fresh epoch or **defers storage reuse until lease release — an
  event-loop deferral, never a cross-thread wait** — and a deadline-missed
  straggler's lease is honored until it acknowledges cancellation —
  serial-recompute proceeds, the straggler's results are discarded, but
  its memory is never pulled out from under it. Snapshot creation, page
  churn, and retirement are charged to F4/F7's accounting (a mechanism
  cost, measured, not assumed free). Under A-VAL's reject branch the
  copied-tables option disappears and only page-COW remains (§7.5's
  pricing). Workers read their snapshot and return
  write-journals (finest-address patches, §7.5) + command lists. The
  runtime thread — the arena's single owner per LLP 0297 — then
  processes each invocation **in enqueue order, one at a time**:
  **re-check the invocation's `InvocationTargetStamp` and task fence
  (§7.4/§7.6) — a cancel or target disposal earlier in the same batch
  suppresses it as `stale-target`**, merge its journal, run the
  ordinary §8.3 sweep and op capture, handle its abort-or-commit,
  release its commands — before touching the next. (The per-invocation
  command release is a proposed 0482 §3.2 delta — that text batches
  command submission after the whole batch commits; serial observation
  is the stronger oracle.) Observable behavior (intermediate
  op streams, per-transition command timing, exception ordering) is
  *identical* to sequential dispatch by construction; only reducer
  evaluation parallelizes (a merge-all-then-sweep-once shape would
  coalesce intermediate ops and defer commands past later commits,
  silently weakening the oracle — refused). Honest consequence: tier 2's
  speedup ceiling is the reducer-evaluation fraction only, and F7's
  break-even is judged against that smaller ceiling.
- **The join is an async continuation, never a cross-thread wait.**
  LLP 0297's standing rule (one sanctioned bounded-wait mechanism with a registered set — live resize, and since RFC 0540 §3.6 the visible-content hold)
  binds: the runtime thread never blocks on workers. Worker completion
  delivers like any completion; the dispatcher simply does not admit the
  *next* batch into commit sequencing until the current batch's journals
  arrive — event-loop structured, failing closed to serial (recompute,
  discard worker results) if a worker misses a bounded deadline, with
  the straggler's snapshot lease honored per the `ArenaSnapshot` rule
  above. No new 0297 exception is created or needed.
- Gated by F7 (§14.2): the conformance corpus's parallel arm plus a chaos
  arm, and a measured break-even on a reference device before the tier is
  anything but dormant.

### 13.4 Tier 3 / 3b — worker offload and SIMD

A region/derive is worker-eligible only under a compiled
**worker-affinity certificate**: arena state read through a leased
`ArenaSnapshot` (§13.3 — r5's "interior mutability is free" did not
survive its own snapshot mechanism), no main-affine capability or
callback rows in its composed signature (§10.2). Admission per 0482 §1: compile-time
eligibility + cutoff constants, runtime N-comparison, serial fast path
always present. Workloads: keyed-rebind evaluation, bulk collection
derives (SoA columns vectorize per 3b), parallel mount over the
partition table, speculation (§13.5). The presenter stays main-bound; op
application order is unchanged.

### 13.5 Speculation

Eligible actions: empty (or provably snapshot-invariant) ambient-read
mask (0480 §6.5's restriction — a clock-reading action never speculates).
**The cache is keyed on the full dispatch identity, or it is wrong**:
entries key on the canonical **`InvocationTargetStamp`** —
`TargetSiteId` (every producer family, §7.4) + instance path +
instance/scope generation + plan generation — plus
`(action id, payload fingerprint, state epoch)` — epoch alone cannot
distinguish two rows invoking the same action with the same payload, so
an under-keyed hit would apply another instance's journal. The epoch
bumps on **every committed transition** and a commit invalidates the
whole cache; a plan swap invalidates it too. **An entry is a complete
`TransitionArtifact` (§8.3)** — the state journal *and* the
runner-internal deltas (memos, guard tuples, cursor bindings,
region/instance deltas, meters) *and* ops *and* commands, precomputed
against the epoch's snapshot — because emitting cached ops without the
internal deltas would leave later evaluation inconsistent, and
re-running the sweep would make the cached ops redundant. A hit applies
the whole artifact atomically; §14.1 carries hit-vs-miss equivalence
fixtures that **continue into subsequent transitions**, not just the
immediate op stream. Speculation workers read a leased `ArenaSnapshot`
(§13.3) — the epoch key is also the memory-isolation mechanism, never a
substitute for one — **plus a coherent runner-state snapshot** (memos,
guards, cursors, instance tables, meters), and because host node/handler
ids are mutable counters today, precomputed ops either carry **reserved
id ranges or logical references late-bound at commit**; §14.1's
hit-vs-miss fixtures include cross-root and intervening-allocation
cases. Speculation is "precompute against the epoch's
snapshot, discard on the next commit": a hit requires a matching epoch,
and on a
hit the runner commits the precomputed journal, emits the precomputed
ops, then executes the precomputed commands — commands
never run speculatively, never from a stale epoch. Cache is bounded,
charged to the route's residency budget (F4's accounting), disabled
under thermal/charge gates (0482 §8.5's hook). Speculation hints are an
optional plan segment (§3.7).

### 13.6 Tier 5 — placement (future)

The encoding falls out of §13.2 + the op protocol; the protocol does not
exist and is not specified here. Owed per LLP 0482 §6: placement
authority, security model (LLP 0333 / 0476 frames), latency/loading
policy, and the distributed protocol (digests/epochs, staleness,
idempotency, reconnect, authn/authz, cancellation, backpressure,
offline). A dedicated LLP owns it when tier 1 exists; this spec reserves
the partition table's fields for placement annotations.

## 14. The verification program

### 14.1 Conformance identity

The conformance corpus becomes the runner oracle: every fixture runs on
(interpreter, mode 1, mode 2, web runner[, mode 3 when built]) and MUST
produce identical semantics-visible output (op streams normalized for
implementation-defined flush order per LLP 0328 D2; state transcripts;
contract verdicts). The corpus gains: plan-format fixtures (encode/decode
round-trips, digest stability, forward-compat skips), journal/rollback
and alias/equality/mid-loop-throw fixtures (A-VAL), event-ordering
fixtures (§7.4), parallel and chaos arms (F7), and assertion-on/off pairs
(§9.4). **Precondition (0480 §12.1): `contract-native-conformance` was
red on main per 0480's dated inspection (22/23, 2026-08-18 — that
document's author-asserted observation, carried here without a fresh
receipt) and MUST be re-greened before any equivalence claim; I3's first
deliverable is the dated baseline receipt.**

### 14.2 Falsifier bindings (0480 §12's matrix, wired)

Each row registers in `exact-verify.json` when it first runs, commits a
receipt under `docs/reports/`, pins capture configuration, and is
adjudicated by Charlie (0480 §12's common rules — restated as binding,
not amended):

| Row | Check id | Instrument here | Receipt |
| --- | --- | --- | --- |
| F1 | `plan-admission-certificate` | §11 | certificate JSON per route/app |
| F2 | `flat-plan-spike-bench` | §15.1 I4's spike; workload = RFC 0478 W4's normative keyed-collection rebind — **the 10K-node fixture is the confirming workload** (0478's normative dispatch), 1K reported as diagnostic; margin = beats interpreter p50 *and* p99 beyond the captured variance band. Receipt MUST carry 0478 §W4's normative fields: ≥1,000 dirtied bindings, ≥1,000 encoded ops per dispatch, full action→derive→reconciliation→emission path, encoded bytes, per-dispatch allocation counters, full histograms | histograms + variance band + workload fields |
| F3 | `first-frame-parity` | §12.5 vs shipped Phase B, same fixtures. Full 0480 oracle: construct coverage strictly ≥ Phase B's (no parity escape on the completeness half) AND cold page faults/bytes beat Phase B **or parity is declared**; on failure 0480 §6.1 is rewritten | bytes + cold fault counts |
| F4 | `marginal-residency` | ≥3 resident surfaces, smaps PSS/USS, shared-mapping verification. 0480's registered bars: ~1 MB marginal supports §7.1; **>3 MB kills it** (0480 F4's proposed thresholds, restated so this table carries its own pass/fail) | per-surface deltas |
| F5 | `idle-wakeups` | LLP 0330 §10 queue under idle; zero non-deadline wakeups. 0480's caveat carried verbatim: **scheduler policy only — a pass confirms nothing about the plan representation** | wakeup trace |
| F6 | `web-payload-interaction` | one real app, four builds (plan/SvelteKit/Qwik/Next per 0483 §8.1–8.2's protocol — the app, framework versions, and compression settings are pinned in the receipt at registration, before capture); compressed first-load bytes ≤ the SvelteKit build AND first successful interaction no later than the best hydrated comparator; Qwik reported as the resumability reference, no bar. **The interaction leg follows the repo's REAL-input rule, scoped correctly per stack**: the Exact plan build needs a real-input-path proof with principal-carrier evidence (agent taps carry the root principal; the `setTimeout(0)`-hop A/B discriminates the carrier class, which exists only on the ibex stack); the SvelteKit/Qwik/Next comparators need real browser input and the same success oracle — no carrier A/B, since the fail-closed principal machinery does not exist there. The receipt pins ≥1 real-input action per build, carriers, oracle, and timing endpoints; synthetic input is diagnostic only | build + trace receipts incl. real-input evidence |
| F7 | `parallel-dispatch-oracle` | §13.3 vs `--sequential` on corpus + chaos; any divergence stop-the-line; break-even measured on a reference device against §13.3's evaluation-only ceiling, else the tier stays dormant. **Per-strategy subreceipts** — tier 2, tier 3, and 3b each carry their own admission evidence and break-even (0482 §1 binds every parallel path); no tier borrows another's receipt | divergence log + per-tier bench |
| F8 | `agent-correction-turns` | task battery **pre-registered before any run**, comparator = the same battery on the current Contract stack, judged at equal task success | turn counts |
| — | `progressive-replay-conformance` (Track W; not a 0480 falsifier row — the PE safety harness §12.4.6 step 9 requires) | restart, cross-replica, crash-window, expiry, capacity, concurrency, 422-succession fixtures against the atomic replay machine | divergence log |

### 14.3 Dual-run equivalence (Track R's gate)

Mode-2 development runs dual-engine against the interpreter on the
corpus per promotion; divergences are release blockers. The interpreter
leaves the reference role only after mode 2 holds corpus-green through
one full checkpoint cycle, surviving thereafter as a debugging oracle.

## 15. Phase 0 and the three tracks

### 15.1 Phase 0 — instruments (start immediately)

| Item | Deliverable | Depends on | Honest label |
| --- | --- | --- | --- |
| I1 | Capability-ABI declaration schema + initial registry draft covering the corpus's world-boundary constructs (§10.1) | — | instrument |
| I2 | Admission-certificate pass (§11.1's fail-closed classifier with its published mapping table) + `plan-admission-certificate` check + receipts for `js/src/caltrain-contract` and each shipping example app. An interim report-only census may run on I1 alone; **the FINAL verdict-bearing receipt requires I5 + I6** (§11.1's proof rule) | I1; final receipt: I5, I6 | instrument |
| I3 | `contract-native-conformance` re-green (22/23 red per 0480 §12.1) | — | **product repair**, not an instrument — changes shipping code, owed independently (RFC 0478 W4a precondition 4), needs a red-suite owner |
| I4 | Flat-plan spike in `contract-native` (F2): data-only plan, opcodes over a shared evaluator, no per-site closures, no await path; workload and margin per §14.2. **Behind a non-default flag; MUST NOT change interpreter behavior** | I3 (equivalence oracle) | diagnostic — its receipt *informs* 0478's W4a/W6 gate and Track R; making it a W4a precondition is 0480 D3's ask, decided by 0478's owner |
| I5 | Opcode-authority extraction: `plan/opcodes.json` drafted from the existing evaluator, generated TS/Rust tables parity-gated. **No shipping-evaluator table swap in Phase 0** | — | instrument; zero decision dependencies — can start first |
| I6 | Proof-only validator designs (§11.1): admission type inference (**a conservative projection over the current IR — NOT Track L change 1's bidirectional inference algorithm**, which stays open per §18.10), dependency/effect closure, region disposition, opcode admissibility — designs + validators, **no shipping-semantics change** | I5 (opcode admissibility validates against the drafted table) | instrument; prerequisite of I2's final receipt |

Exit: F1 + F2 receipts before Charlie under 0480 §12.2's verdict rule.
Estimated at days-to-small-weeks each (reasoned; I4 inherits 0480 §12.1's
"days, not weeks, plus the re-green").

### 15.2 Track L — the semantic amendments (gate: F1 pass + paved paths;
independent of F2 and D1)

> **DECIDED since freeze (2026-08-20, LLP 0505 r3 row 3):** the five
> sequential per-change compiler flags and dual F1 verdicts below are
> superseded by **one merged Contract-v1 ratification** (LLP 0508: 0330
> ⊕ 0489 ⊕ 0501 ⊕ this spec's §§5–9 — one version, one flag, one
> migration). The content ordering below still informs 0508; do not
> implement per-change flags.

0480 §12.2's verdict rule is explicit: F1 pass + F2 fail means the
representation dies while "the language changes may still proceed on the
TS tier where each pays its own way" (LLP 0481 §9.7). Track L is
therefore gated on F1 and its paved alternatives only — chaining it to
the representation would silently repeal that rule.

Order (per LLP 0481 §8): **change 1** (scope promotion behind a compiler
flag; corpus rejection-rate report; `shape` lands with it) → **change 4**
(region records; co-designed with change 3's settlement events) →
**change 2** (cursor + guarded resource edges land first as paved paths,
then static-graph authority, then read-tracking deletion) → **change 3**
(paved task form + codemod + census-gated landing; the breaking amendment
of `llp/contract/0082`/`0087` and 0330 A4, named as such) → **change 5**
(budgets, needing the numbers 1–4 create). Every amendment: LLP 0330
amendment text (including A-VAL, §7.5), paved alternative shipped first,
fix-its for every rejection class, corpus fixtures on all engines before
claim (RFC 0478 P4/P7 discipline) — and **every change stays behind a
compiler flag until W2's owner adopts its amendment text** (change 1's
rule applied uniformly; r4 left change 3 without the flag, which would
have let a breaking amendment ship under a document this spec says
stands as written). Timing against W2's ratification is 0480 §15.9's
open question — this spec sequences content, not the W2 calendar.

### 15.3 Track R — the plan and mode 2 (gate: F1 + F2 pass; D1 adopted;
the RFC 0478 W6 go per §1.3)

> **DECIDED since freeze (2026-08-20, LLP 0505 r3 rows 1/2/8):** within
> this section's list — the web runner is the wasm engine (§12.4 note);
> tier-2 v1 dispatch is **replay-only** and **the tier-5 design LLP is
> parked**, not commissioned (row 8); and "the TS interpreter…
> untouched" is now bounded: the TS tree-walker survives as the
> conformance **oracle**, while `contract-native`'s mirror interpreter
> carries a deletion trigger at dual-run-equivalence green (row 2).

Plan format v1 (§3) + the format-schema authority (§3.3) with generated
readers and golden fixtures; IR→plan lowering in the compiler; mode-2
runner in `contract-native` behind `EXACT_PLAN_RUNNER=1`; **the web
runner — the same plan bytes on a JS host (§12.4), with resumption,
serialized-state encoding, SSR partition + streaming modes, and F6
capture — F6 additionally waits on the §12.4.7 style-lowering design**
(an F2 miss kills this half with the native half, §1.3);
dual-run equivalence (§14.3); loci sidecar; Semantics table wired to the
presenter AX path; first-frame emission unified with 0307 Phase B (F3).
Then, inside Track R and each behind its own flag: tier-2 v1 dispatch
(after Track L changes 1–3, F7 registration, and 0482's decision on
§13.3's held deltas), worker-affinity certificates + tier-3/3b offload
(**only after RFC 0478's owner takes the W6-research-annex decision —
the annex is tiers 3/3b per 0482, not tier 2**), the speculation cache
(§13.5), and the tier-5 design LLP commissioned. The TS interpreter and today's IR wire format are
untouched — the plan is an additional lowering until Track L completes.
Mode 3 has **no phase and no track**: it enters only through its own
future decision (§12.3).

### 15.4 Track W — the web authoring surface (gate: Track L changes 1–4;
the §12.4.7 style-lowering design)

Representation-independent compiler/language work: the `form` tag; the
PE classifier + server replay per §12.4.6's lattice (the reducer replays
server-side on any engine); the style-lowering artifact (§12.4.7);
composition with LLP 0351's map stated in the Guide (0483 §8.6's owed
page). Nothing in Track W consumes plan bytes; everything that does
lives in Track R.

### 15.5 RFC 0478 interface (asks, not decisions)

This spec's tracks interlock with 0478 exactly where 0480 §10 enumerated:
D1 promotes W6 (ask 1); Track L is the language program against W2
(ask 2); I4 is the W4a-precondition ask (ask 3); I2's census bears on the
W3.1 pause recommendation (ask 4); F2's design answers the W4a
contamination caution (ask 5); F6 carries the web-speed-mechanism ask
(ask 6). Every one is decided by 0478's owner, not here. 0478's W1–W5
remain the shipping program throughout and nothing here pauses them; the
asks appear in this spec only as **Track R's gate row** (§1.3), never as
locks on Phase 0 or Track L. Two consequences stated plainly: **if 0478
declines the W3.1 pause (ask 4), Track L change 3 and live W3.1
async-action lowering are mutually destructive** — every week of W3.1
work is stranded by change 3's landing, which is 0480 §10.4's argument,
restated here so the collision is priced where the tracks are defined.
And **Track R's web runner is experimental-for-F6-capture only**: the
production web replacement of W1's mechanism is 0478 D6's decision
(ask 6), taken after F6 reports — Track R landing does not flip the web
default. *(**DECIDED since freeze** — 2026-08-20, LLP 0505 r3 row 1 /
RFC 0478 D14(4) / RFC 0483 §2(b): the web-speed-mechanism decision is
taken — the wasm plan runner owns destination web speed, with the
cutover still gated on corpus green and the TS-seam gate. The "decision
taken after F6 reports" framing is superseded; the gates, not a future
decision, now govern the flip.)*

### 15.6 Tickets and tracking

Phase items land as filesystem tickets under `issues/` per LLP 0361,
scored through Issue Atlas; receipts under `docs/reports/`; no new root
`check:*` script without its registry row (standing rule).

## 16. Knowledge-layer interface (LLP 0484)

- **Canon disposition.** At 0484 M1 triage, the 0480–0484 cluster plus
  this spec are assigned like every other LLP. Expected shape (M1
  decides): once Implemented, this spec becomes the **canon-track
  as-built document for the Contract execution model** — written to
  0484's rebuild test — absorbing 0480–0483's durable content, which
  then take banners and move to `llp/archive/`.
- **Canon sync on Implemented** (0484 §2.2): each track's completion
  folds its durable deltas into the owning canon document once the canon
  exists.
- **Authority discipline:** Phase 0 creates `plan/opcodes.json`,
  `plan/capabilities.json`, and `plan/executable-contexts.json` as
  **draft registries** (working authorities for the instruments);
  each — plus Track R's `plan/format-schema.json` — is **promoted to an
  `exact-contracts.json` row when its owning track lands**, with
  generated-neighbor rules; and no others. Certificates and receipts
  are reports, not registries.

## 17. Compatibility and migration

- **The IR stays.** The compiler's frontend product and the editable
  substrate remain the IR; the plan is a derived lowering (§3.3.16). Wire
  compatibility for today's IR-consuming paths persists through Track L.
- **The interpreter stays** through Track L as reference engine (§14.3);
  LLP 0185/0186's TS tier continues per 0478 W1 unless/until F6 and
  0478's owner replace the web speed mechanism (§15.5 ask 6).
- **Existing corpus migrates by census:** §7.8's codemod for awaits;
  fix-it-driven migration for blob state (§5.4), dynamic indexing (§6.2),
  and `untrack` (§6.5). No silent behavior change: every migrated file
  diffs visibly and its contract verdicts must hold before/after.
- **Protocol unchanged — conditional only on the adopted form
  lowering** (§12.4.6: if 0091's host/SSR facts need a new op, that op
  is 0091's to propose; everything else here adds none). Presenters, the op protocol, and the binary
  ABI are untouched by every phase here; the plan changes what produces
  ops, never what they mean.

## 18. Open questions

1. Surface syntax final forms: `shape` (§5.4), `cursor` (§6.2),
   `task … from / -> ok|fail` (§7.7), the escape annotation (§11.2),
   cost-claim clauses (§9.2) — all normative as constructs, all subject
   to grammar review against `0087`. (`form` is no longer here: its
   spelling is §1.2 inventory item (d), a proposed resolution of 0091's
   open notation decision — only its attr-level grammar details ride
   ordinary review.)
2. The escape annotation's exact grammar and its interaction with the
   LLP 0160 §5.3 framework decision log (is an escape-carrying route a
   logged exception?).
3. Journal representation for wide `each` writes (per-column ranges vs
   per-row entries) — a performance question F2's spike should probe, not
   a semantics question.
4. Guard-evaluation cost in the invalidation path (§6.3) at realistic
   guard density — measure in the spike.
5. The PE envelope's key-management detail (§12.4.6 fixes the lattice,
   binding, and freshness rules; the KMS/rotation design note is owed
   before Track W ships PE — LLP 0476's frames apply).
6. Per-hardware-class strategy certificates (§13.1): who produces them,
   and where they live in the deployment manifest.
7. How mode-1 introspection surfaces plan tables through the existing
   Acto operation registry without new wire names.
8. Whether the localizable-strings segment needs bidi/plural metadata in
   v1 or defers to the (out-of-scope) translation pipeline.
9. The graphics scene sub-program's encoding (§8.1 places it beside the
   plan like motion; its own table/opcode format is a Track R design
   item).
10. The inference **algorithm** (§5.2 states requirements — total,
    bidirectional, no state annotations — not the algorithm; Track L
    change 1 owes the design before implementation).


---

## Amendment 1 (2026-08-20): the decided-program alignment

**This is a formal amendment to this Accepted specification**, applied
under the LLP process's decided-content rule: every item below was
decided by the author on 2026-08-20 (the LLP 0505 r3 docket, the
RFC 0498 r6 trio, the seam batch, and the recorded 0480 §12.2
adjudication). Items 1–6 and 11 formalize sites already marked in the
frozen body by dated `DECIDED` or `ERRATUM` annotations; items 7–10
carry **no in-body site annotation** and formalize decided content
recorded in the filed erratum tickets and RFC 0498 r6 §8's joint
ruling directly. In both cases the amendment adds nothing beyond what
those decided sources say. *(Corrected 2026-08-20 by the wave-delta
fold: the original preamble claimed every item was site-annotated —
false for items 7–10.)* **The original frozen text remains in place above, for the
record, under the execution-posture banner**; where this amendment and
the frozen body disagree, the amendment governs. This amendment
discharges the "pointed freeze amendment remains owed" line of the
banner.

1. **Web-runner posture (§12.4, §15.3, §15.5).** Governing decision:
   LLP 0505 r3 row 1 / RFC 0483 §2 = option (b) / RFC 0478 D13+D14(4).
   Amended reading: the web runner is the **same Rust engine compiled
   to wasm over real DOM**, gated on RFC 0483 §2's **three** cutover
   conjuncts — corpus green on the wasm/DOM path, **the priced wasm
   costs (compressed bytes, compile/instantiate latency,
   CSP/compatibility, the wasm↔DOM call boundary) measured and
   acceptable**, and the TS-seam gate; no JS-language runner is built,
   and the web-speed mechanism decision is taken — the gates, not a
   future decision, govern the flip. *(The priced-costs conjunct was
   omitted from this item as first landed; restored by the wave-delta
   fold from 0483 §2's decided text.)* §12.4's runner-fixedness, size-budget, decode,
   resumption, and SSR obligations transfer to the wasm runner.
2. **The 0330 inbox path (§7.4's linearization passage, §13.3).**
   Governing decision: LLP 0505 r3 row 8 / RFC 0482 r6. Amended
   reading: the proposed 0330 §4 inbox amendment is **declined for
   v1**; the reject branch is normative — host events dispatch
   synchronously at delivery, the class order applies to timers and
   completions only, tier 2 ships **replay-only**, and tier 5 is
   parked. The inbox passage stands as recorded design of the
   not-taken branch.
3. **Absence is Option-only (§5.1's value types).** Governing decision:
   LLP 0505 r3 row 10 / LLP 0489 r6 / LLP 0508 §4.2. Amended reading:
   `Option<T>` is the only authored absence form; `null` survives
   solely as boundary-parse input normalized at the seam
   (`T | null` is not an authored type). The frozen §6.2 key
   universe's admission of `null` for boundary values is unchanged.
4. **Track L lands as one ratification (§15.2).** Governing decision:
   LLP 0505 r3 row 3. Amended reading: the five sequential per-change
   compiler flags and dual F1 verdicts are superseded by **one merged
   Contract-v1 ratification** (LLP 0508: 0330 ⊕ 0489 ⊕ 0501 ⊕ this
   spec's §§5–9 — one version, one flag, one migration). §15.2's
   content ordering informs 0508; per-change flags are not
   implemented.
5. **No distinct Mode-1 dev runner (§12.1).** Governing decision:
   LLP 0500 D2 (Active) with D1 as amended 2026-08-20. Amended
   reading: dev runs the **production runner plus a plan-patch channel
   and an introspection layer**; HMR is a plan patch on the
   HotRevision spine matching on **declaration identity** (the
   dependency tuple feeds the runtime cache key and is not the patch
   key; canonical state is reused only on exact runtime-key equality —
   RFC 0498 r6 §4.1 / LLP 0509's reuse gate). §12.1's introspection
   and reset-report obligations survive as requirements on that
   configuration.
6. **The `dep_union` false-SCC erratum (§6.3's pure-derive rule).**
   Governing record:
   `issues/20260820-llp0485-conditional-union-false-scc-erratum.md`.
   Amended reading: the "same value, merely extra recomputation" claim
   is withdrawn as stated — branch-unioning can mint an artificial SCC
   (LLP 0481 §3.5's own proof). The resolution (guarded/predicate-labelled edges with branch-aware
   SCC analysis, or explicit rejection of mutually conditional derive
   pairs **with a diagnostic and priced migration** — the ticket's two
   options at full strength) lands via the LLP 0508 ratification,
   which MUST NOT carry `dep_union` forward as written; the ticket
   closes when 0508's ratified text lands that resolution.
7. **Callback tolerance is compiler-derived (§7.3 closure 4).**
   Governing record:
   `issues/20260820-llp0485-callback-tolerance-closure-erratum.md`,
   resolved per LLP 0489 r6 / LLP 0508 §4: no declared-tolerance
   grammar exists, so closure 4 is **narrowed, not relocated** — a
   callback prop's effect row is **inferred at its definition** by the
   fixed-point unification that joins the composed rows of every
   callback bound at every composition site; **conflicting row
   constraints reject once, at the definition**, identifying both
   constraint sites; and **no composition site rejects against a
   tolerance that cannot yet be declared** — the derived fixed-point
   row stands in for the declared maximum until the authored
   per-prop tolerance amendment lands. *(This item as first landed
   kept a composition-site rejection with "compiler-derived"
   substituted — the opposite of the decided locus; corrected by the
   wave-delta fold from 0489 r6's bytes.)* The ticket closes when the
   0508 ratified text lands.
8. **§7.5's equality relation is split three ways.** Governing record:
   `issues/20260820-llp0485-publication-equivalence-overload-erratum.md`,
   resolved per LLP 0489 r5+/r6 §2.1 and LLP 0508 §5: the three
   overloaded §7.5 jobs map to **two** named relations — publication
   dirtiness is **`PublicationEquivalenceV1`** (deliberately not an
   equivalence relation, and never usable where a total relation is
   required), while callback curried-environment comparison **and**
   aggregate replay comparison are both
   **`CanonicalValueEquivalenceV1`** (the total, alias-insensitive
   structural equivalence); **source equality** (`==`/`!=`) is the
   separate author-facing relation and was never one of the overloaded
   jobs. The always-publish aggregate rule stands inside the
   publication relation only. *(This item as first landed listed the
   jobs against the wrong relations; corrected by the wave-delta fold
   from 0489 §2.1's bytes.)* The ticket closes when the 0508 ratified
   text lands.
9. **`component-call` joins the §8.1 composed inventory.** Governing
   record:
   `issues/20260820-llp0485-component-call-composed-inventory-erratum.md`,
   resolved per LLP 0501 r5+: the `composed` Region kind's construct
   list reads "snippet calls, `pager`, **`component-call`**", with the
   component-call record's instance lifecycle as 0501 specifies. The
   ticket closes when the Contract-v1 ratification lands 0501's text.
10. **§6.3's tuple comparison is the subscriber's key-shift trigger.**
    Governing record: RFC 0498 r6 §8's recorded joint ruling. Amended
    reading: the §6.3 declaration-plus-tuple comparison operates as
    the **subscriber's key-shift trigger** under RFC 0498 §4.1 (the
    runtime cache key changing for that subscriber), **not** as the
    HMR preservation match key — that match key is **declaration
    identity** per amended LLP 0500 D1 (item 5 above), and matching is
    never sufficiency (RFC 0498 r6 §4.1's runtime-key reuse gate
    governs).

11. **The Track R start-gate, as exercised (§1.3).** Governing record:
    LLP 0480 §14 D1 (TAKEN, 2026-08-20, "Track R go") and §12.2's
    adjudication record. Amended reading: Track R's start gate is the
    **exercised** gate — instrument-acceptance + F2 pass + the explicit
    author go — with F1-pass an entry condition of Track L under the
    stricter predicate, not a Track R start condition. §1.3's "F1 + F2
    pass … MUST NOT begin" sentence describes the pre-adjudication
    posture. (This is the first of LLP 0480 §14's two carried pointed
    amendments; the second — Mode 1 collapsed — is item 5 above. Both
    are hereby landed.)

Ticket disposition note: this amendment adopts each correction as the
normative reading of this spec today; the cited ticket files close when
the named landing vehicle (the LLP 0508 / Contract-v1 ratification for
items 6–9) lands its text, by whoever lands it — the tickets are the
tracking instruments, this amendment is the authority.

## Amendment 2 (2026-08-22): the island certificate/format reservation

**This is a formal amendment to this Accepted specification**,
applied under the same decided-content rule as Amendment 1. It
executes 0504 §3 row 24 (from RFC 0518 §7.1 item 1) on the author's
direction of 2026-08-22, under the shared-clocks rule: this is the
window-bound *format reservation* — it reserves surface so a
post-window adoption is not a format break. It deliberately adopts
**no island semantics**: RFC 0518's two-ceilings ruling remains
pending at its venue (0481-OQ1-in-W2, row 9), and every reserved
token below is inert until that adoption event.

1. **Certificate entry class `island` (§11 schema).** The
   `PlanAdmissionCertificate` schema gains a reserved entry class
   `island` with three required fields — `lane` (one of the three
   0518 lanes: pure priced call / provider-shaped / operation-bound),
   `reason` (the declared justification tag, 0160 §5.3-style), and
   `budget` (the priced bound; for Lane 1 the LLP 0517 TS-seam
   benchmark number is the only legal source) — plus the standard
   proof-row binding of §11.1 (validator + authority digest;
   missing proof is class 5). Until the 0518 adoption event, a
   certificate containing an `island` entry is **refused by the
   validator** (reserved-but-inactive; fail-closed), so the class
   cannot be squatted on early.
2. **Deps/Regions row kinds.** The Deps and Regions tables gain one
   reserved row-kind id each (`island-boundary` dependency edge;
   `island-region` disposition) for the island boundary's needs —
   ids allocated in the format schema, semantics undefined until
   adoption, presence refused until adoption (same fail-closed rule).
3. **Shape discipline.** Islands reuse the §3.3.14
   referenced-artifact shape for any carried subprogram bytes —
   this amendment does not extend §3.3.14, and any island artifact
   kind is admitted through the existing referenced-artifact table,
   never a parallel one.

Where this amendment and the frozen body disagree, the amendment
governs. On the 0518 adoption event, the adopting change activates
these reservations and discharges 0504 §3 rows 9/24 together; if
that venue instead rejects the island inventory, the reserved ids
are retired by a follow-up amendment (retirement is cheap;
foreclosure was not).

## Amendment 3 (2026-08-25): the island certificate activation

**This is a formal amendment to this Accepted specification**, applied
under the same decided-content rule as Amendments 1 and 2. It is the
adopting change Amendment 2's closing sentence names: RFC 0518's
Rulings A + B were **adopted verbatim at the 0481-OQ1-in-W2 venue by
Charlie Cheever, 2026-08-25** (decision relayed via orchestration
session exact-9e), via the LLP 0508 §14 ratification (0508 §13
item 19). This amendment executes 0518 §7.1 (the narrow window-bound
certificate/format rider) by activating Amendment 2's reservations;
per Amendment 2 it discharges 0504 §3 rows 9/24 together (recorded
here by citation; the ledger flips are the map lane's derivation).

1. **The `island` entry class is ACTIVE.** Amendment 2 item 1's
   reserved certificate entry class binds to the adopted semantics:
   `lane` is one of RFC 0518 §4's three lanes, whose format ids this
   amendment pins as `pure-call` / `provider` / `command` (the
   0518 §4 / Amendment 2 prose names "pure priced call" /
   "provider-shaped" / "command-shaped"-slash-"operation-bound" name
   the same three lanes; these ids are the certificate and
   0510-Amendment-R3 encoding), `reason` is the island's
   `@dynamic-island: <lane> — <reason> (LLP 0518)` tag (0518 §6),
   and `budget` is the priced bound — **for Lane 1 the only legal
   source remains the LLP 0517 §10 TS-seam benchmark number, carried
   by the registered `wasm-embedding-ts-seam-benchmark` check (0518
   §7.3: no second number)**. The blanket reserved-class refusal is
   lifted *as format*; admission stays fail-closed *as policy*: the
   initial discretionary whitelist is empty (0518 OQ2 decided:
   nothing), so a certificate carrying an `island` entry outside the
   treaty's own named baseline classifies that region class 5 with a
   named `island-not-whitelisted` disposition, exactly as §11.1
   treats any other unproven claim. An island region is class 5 for
   speculation purposes by construction (0518 §5.1: non-speculable;
   consumers' speculation windows end at its boundary), whatever its
   admission status.
2. **The row kinds are ACTIVE.** `island-boundary` (Deps) carries the
   island's declared-input dependency edge set and declared write
   set — the plan sees a total node whose edges are exactly the
   declaration (0518 §4); `island-region` (Regions) carries the
   island's region disposition (lane, reason tag, budget binding,
   and the RFC 0495 / LLP 0511 evidence-axis labeling — receipts
   crossing the boundary are observed-not-proven per 0518 §5.4).
   Presence no longer refuses on
   reservation grounds; unknown-lane or missing-field rows refuse as
   malformed rows, as anywhere else in the certificate.
3. **Island axes join §11.3.** Island count and per-lane cost are
   certificate axes mirroring the widening axis, claimable in a
   contract block (`islands <= N`, 0518 §5.5); a build exceeding a
   declared island budget fails. **No default budget is set by this
   amendment** — budget defaults (including 0518 OQ3's `islands <= 0`
   proposal for `native-total` roots) are §7's deferred policy set.
4. **Shape discipline unchanged.** Amendment 2 item 3 stands: island
   subprogram bytes ride the §3.3.14 referenced-artifact table,
   never a parallel one. The 0492 instance (runtime motion-graph
   construction) is Lane 2 over the Class-S motion artifact with its
   retained bounded admission; the 0498 instance
   (descriptor-dynamic queries) is Lane 2 under its provider
   capability. Lanes 2/3 capability identities fold through
   LLP 0510's table under Amendment R3's island fold-row class —
   an island can never launder authority its fold row does not
   grant (0518 §5.3).

Deliberately **not** landed here (0518 §7's non-window-bound policy
set, deferred for census evidence): whitelist contents and growth,
Design Mode/agent-capability gating for Lane-2 motion graphs, island
budget defaults, and the §6 reason-tag check registration. Where this
amendment and the frozen body disagree, the amendment governs.

## Amendment 4 (2026-08-25): the enum-roster digest (format v1.6)

**Motivation (provenance):** the Track R plan-plane skew
(issues/closed/20260825-track-r-plan-corpus-skew-red-on-main.md, cured
at 6ae97b4e1) proved by incident that the plan `attrId`/`eventKind` id
spaces are **roster-ordinal** — `generate-plan-format.mjs` assigns ids
by roster position over the builtin-schema authority — so roster
growth renumbers ids, and a committed plan carrying old-id bytes
admitted cleanly under regenerated readers and **misread silently**
(attr 218, "testId" then, decoded as whatever 218 means now). The
header bound the opcode table but not the enum id space: exactly the
fail-open artifact class §3.4's digest discipline exists to forbid.
The cure landing removed the *reader* class (name-resolved lookups);
this amendment removes the *artifact* class. Filed as
issues/20260825-plan-enum-roster-digest-fail-open.md; closed by this
amendment's landing.

1. **Header field.** The v1.6 header carries `enumRosterDigest`
   (digest32) at offset 80, after `opcodeTableDigest`; `segmentCount`
   moves to 112 and the segment directory to 116 (headerSize
   84 → 116). Fields below offset 16 are layout-stable across v1.x.
2. **Amendment class: format-minor v1.6 with minReaderVersion v1.6**,
   by the landed v1.4 → v1.5 precedent (required material moves the
   minor and the minReader floor together; the schema `$comment`
   records that ruling). §3.4's majorRule stays reserved for
   incompatible layout that the minReader gate cannot fence; here it
   fences completely — a pre-1.6 reader refuses a v1.6 plan at its
   minReaderVersion gate before reading any relocated offset.
3. **Preimage.** SHA-256, domain-separated
   (`ExactPlanEnumRosterV1\0`), over the canonical enumeration of the
   generated enum-named-values model (every schema enum except the
   `external` declaration block, plus the roster-derived
   viewTag/attrId/eventKind): enums sorted by name bytes, members
   sorted by (value, name bytes), all integers u32 little-endian,
   names length-prefixed. Specified normatively in
   `format-schema.json` `container.enumRosterDigest`; the generator
   computes it once and embeds the constant in both readers and the
   writer — runtime code compares, never re-canonicalizes.
4. **Reader rules** (schema `container.versioning`): `rosterRule` — a
   digest mismatch is a typed refusal (`enum-roster-digest`) naming
   both digests; `floorRule` — within major 1, a plan whose
   formatMinor predates v1.6 refuses as `enum-roster-absent`
   (fail-closed: the field is absent by construction, the enum id
   space unverifiable — the exact class this amendment removes — and
   Phase 0 regenerates the committed corpus in lockstep, so nothing
   is grandfathered). The floor is paired: a plan whose
   `minReaderVersion` predates v1.6 refuses as `reader-version` — the
   v1.6 header relocation is not skippable, so a pre-1.6
   minimum-reader claim on a v1.6 layout is malformed (it would let a
   pre-1.6 reader misparse the relocated `segmentCount`).
   `planDigest`'s projection gains the roster
   digest after `opcodeTableDigest`.
5. **Evidence discipline.** The corpus and every committed plan
   artifact regenerate in the same landing; the negative corpus gains
   the roster-drift fixtures (`malformed-enum-roster-digest.eplan`,
   resealed so only the roster gate can be the refusing gate;
   `malformed-enum-roster-absent-v1.5.eplan` for the legacy layout)
   and the planrunner mutant probe
   (`resealed-enum-roster-digest-drift.eplan`) proving the
   old-roster-plan shape refuses instead of misreading.

Where this amendment and the frozen §3 text disagree, the amendment
and `format-schema.json` govern.

## Amendment 5 (2026-08-25): the §3.3.14 evaluator retarget (RFC 0492's named narrow amendment)

**This is a formal amendment to this Accepted specification**,
applied under the same decided-content rule as Amendments 1–4. It
executes the one narrow amendment Accepted RFC 0492 §8 explicitly
proposes and names ("this program retargets that one sentence
(evaluator = `exact-motion`) and adds the S-code→S-data paved path
for compiled artifacts; a one-sentence, explicitly proposed 0485
amendment, not silent conformance"), discharging 0504 §3 row 5's owed
residue. Decided content only — the deciding text is Accepted 0492
§8; no new semantics are introduced here.

1. **Evaluator retarget.** §3.3.14's sentence "the evaluator remains
   the separate LLP 0099 / 0297 worklet runtime" is retargeted: the
   referenced evaluator is the **`exact-motion` evaluator** (the
   RFC 0491 WS-G crate — value graph, drivers, recognizers,
   transport; RFC 0492 §8's atomic ruling with RFC 0490: one Rust
   evaluator on every tier, the web leg as the wasm build in the
   fixed runner). The shipped LLP 0099/0297 worklet runtime remains
   the interim implementation until RFC 0492 M-C/M-F execute; that
   interim status is an implementation fact, never a second
   referenced evaluator.
2. **The S-code→S-data paved path.** Today's motion programs remain
   classified **S-code**; a compiled `exact-motion` artifact is the
   **S-data** form of the same referenced Class S program (RFC 0492
   §4.3's compiled-artifact shape). S-code and S-data are two forms
   of one referenced artifact — recompiled from the same source,
   admitted under the one §3.3.14 referenced-artifact treatment —
   never two motion systems.
3. **Boundaries (0492 §8's own).** Graph bytes do NOT move into the
   plan container — that remains the explicit §3.3.14/§3.8 amendment
   RFC 0492 §4.3 reserves (0504 §3 row 5(b): not-an-ask unless 0492
   ever takes it). The escape-hatch ceiling is RFC 0518's adopted
   Ruling B (Amendment 3), not widened here.

Where this amendment and the frozen §3.3.14 text disagree, the
amendment governs.

## Amendment 6 (2026-08-26): the virtual-visibility folds (RFC 0540 / LLP 0543 rows 59, 66, 73)

**This is a formal amendment to this Accepted specification**, applied
under the same decided-content rule as Amendments 1–5. It folds the
three asks RFC 0540 §8.5 files against this spec's owner — 0504 §3
rows **59**, **66** (this spec's half), and **73**. The deciding text
is Accepted RFC 0540; the shapes are LLP 0543's; nothing here
re-decides either. **Delivery is post-window**: RFC 0540 §8.6
classifies the whole lists-v2 surface as D2 non-window, so this
amendment is a recorded reading, not a freeze-window rider, and its
implementation rides 0540's L0–L6 ladder.

**1. §8.3 gains a region-level emission gate, with a semantic-prop
exemption (row 59).** Under a **hidden** row — `hidden` being a
runner/presenter *mode*, never a 0508 instance lifecycle state; the
row's instance lifetime remains its key's lifetime in the
`CollectionProjection` — the runner **suppresses §8.3 step 4 (binding
emission) for the whole subtree**, with one exemption: **semantic
props keep flowing** (role, label, value, logical position), because a
semantics-only update is what keeps the platform accessibility proxy
and Acto current while the native view is released (0540 §1.1/§1.3).
Steps 1–3 are unchanged: dirty marking, the graph sweep, and region
toggles all run normally, so the gate is an *emission* gate and never
an evaluation gate — plan rows persist, derives re-evaluate, and no
state is lost. On `hidden → prerender` the runner **re-emits the full
subtree**; that resume path is also where 0478's N-per-frame GC-tail
claim now lands (0540 §8.3). The gate is metered under §9.2.

**2. The `TransitionArtifact` carries the applied mode and ordinal
(row 59).** §8.3's "complete output of steps 1–5 for one transition"
grows two fields: the **applied visibility mode** and the **applied
transition ordinal** for the participants the transition touched. This
keeps the one-commit-representation rule intact — the mode that was
actually applied is in the same artifact as the op stream that
did or did not carry the suppressed bindings, so rollback, worker
results, speculation entries, and replay all agree about what was
emitted. The presenter's *current* mode and ordinal remain host truth
fenced at capture (0540 §7); the artifact records what the runner
applied, and the two are compared, never conflated.

**3. `VirtualTopologyCertificateV1` is a producer-neutral admission
artifact (row 66, this spec's half).** The certificate is **not** a
Contract-only instrument and it is **not** a new §11 classifier class
(0540 OQ11 forbids that arm outright). Its body is published once per
root incarnation as a content-addressed artifact and looked up by the
`topologyCertificateDigest` prop at root adoption; **all three
producers use the one admission path** — Contract through this spec's
§11 pass, the React encoder through its artifact path, and the Rust
producer through LLP 0331 §2.3. The digest recipe is 0543 §5.1's
(SHA-256 over the 0507 §8.2 canonical serialization under the domain
tag `EXACT.VIRTUAL-TOPOLOGY-CERTIFICATE.SHA256.V1\0`; never a second
canonicalizer). Nothing here crosses the LLP 0510 boundary (0540
§8.7), so no v2 boundary work is opened by this row. **0508 §8.3
Region semantics are unchanged by projection**: the keyed Region reads
`CollectionProjection.items` — a contiguous slice — exactly as it reads
any collection expression, and `.aux` is unreachable from a template
(0543 E-C1).

**4. Row 73 — the certificate body's admission path, and the proof-row
question, DECIDED.** 0540 OQ11's fork is **resolved on the
content-addressed arm**, as LLP 0543 §5.3 specifies it: the body rides
the 0507 §6.3 extension envelope as a new tree-bound **sidecar family**
(`SetVirtualTopologyCertificate`), the digest travels on the command
frame as a typed prop, and a release host refuses a root whose digest
is absent or matches no adopted body (E-V2; a malformed digest is
E-17). The residue 0543 leaves to this owner — whether Contract
**additionally** records the body as a `PlanAdmissionCertificate`
proof row (0543 [Open issue 1]) — is **DECLINED**. Reason: the
content-addressed arm was chosen precisely because the proof-row arm
is Contract-only and would oblige the React and Rust producers to
invent equivalents (0540 OQ11's own words); adding the proof row
*back* as an optional Contract extra reintroduces exactly that
asymmetry, and gives the same fact two homes that can drift — the
sidecar body and a certificate row asserting it. **One admission path,
one carrier.** This is an owner disposition, not a 0540 re-decision:
if a Contract-side admission-time refusal is later shown to need the
fact inside the certificate, it returns as a normal amendment with
the producer-parity question answered first.

**5. Still owed, deliberately not claimed here.** LLP 0543 §17.3 **O-7**
— the canonical JSON **field set** for the §7.4 `TargetSiteId` record
that `VirtualTopologyCertificateV1.rootSite` serializes — remains an
open obligation on this spec's owner. It is a concrete schema
deliverable, not a reading, and it lands with the certificate's schema
rather than by fold; the canonicalization *recipe* above (0507 §8.2)
already binds whatever field set is pinned.
**Owner and timing, named so it does not rot:** this spec's owner
supplies it, and the natural landing is **with LLP 0543's O-1
generator work** — O-1 already widens the inventory and emits the
schema package that `VirtualTopologyCertificateV1` serializes into, so
the field set is decided in the same change that first needs it rather
than in a separate pass. Until it lands, `rootSite`'s canonical JSON
has a recipe but no pinned shape, and the V-1a golden fixture cannot
be authored — which is the concrete symptom to watch for, and the
reason this cannot quietly slip past 0540's L1. Row 66's LLP 0508 and LLP
0331 halves are folded on those owners, not here.

Where this amendment and the frozen §8.3 / §11 text disagree, the
amendment governs.

## Amendment 7 (2026-08-26): the EPLC never-localized site classification (RFC 0536's named amendment; 0504 §3 row 52c)

**This is a formal amendment to this Accepted specification**, applied
under the same decided-content rule as Amendments 1–6. It executes the
one amendment Accepted RFC 0536 (2026-08-22) names against this
document — "the matching 0485 EPLC classification amendment for
never-localized sites" (RFC 0536 §3.1) — as the companion to the
`text raw=` opt-out landing on LLP 0508's §14 clock (its §13 item 20).
Decided content only; the deciding text is RFC 0536.

**The problem it closes.** RFC 0536 makes the compiler extract Contract
text literals *and* user-visible attribute strings into the base-locale
`localizable-strings` catalog, keyed by §3.3.15's stable string ids. A
completeness check — "no unextracted user-visible literal ships" (RFC
0536 §3.7) — is only meaningful if the schema can distinguish a string
that was *missed* from a string the author deliberately declared not
copy. Without a classification, `text raw="OK"` and a forgotten literal
are indistinguishable in the artifact, and the check must either pass
both or fail both.

**The amendment.** A string site carries one of two extraction
classifications:

1. **Localizable** — the default. The site produces an entry in the
   base-locale `localizable-strings` catalog under a §3.3.15 stable
   string id, and per-locale catalogs are keyed by that id.
2. **Never-localized** — the site is authored with RFC 0536's opt-out
   (`text raw=` or its attribute equivalent). It produces **no
   `localizable-strings` entry in any locale**, is excluded from the
   extraction-completeness check by classification rather than by
   absence, and its bytes stay where a non-copy string belongs — in
   `invariant-strings`, inside `planDigest`, alongside testIds, keys,
   and tags.

That second placement is the substantive consequence and follows from
§3.3.15's existing split rather than extending it: a never-localized
string cannot change under a catalog swap, so it is digest-covered
core plan, not an auxiliary per-locale object. `testId`s need no
opt-out — RFC 0536 excludes them by construction, and they were already
`invariant-strings`.

**Boundaries.** The id scheme and collision handling are **unchanged**
and remain this schema's: RFC 0536 §3.1 is explicit that its extraction
pipeline "produces entries in the existing frozen format; it does not
define a competing site scheme", and that any inadequacy found there
returns as a named amendment here rather than as a second scheme
elsewhere. This amendment adds a classification, not an id rule, and
the §3.4 manifest tuple (`planDigest, locale, catalogDigest`) is
untouched. The authored spelling of the opt-out is LLP 0508's (§13
item 20); the `has text key=` selector is `llp/contract/0085`'s.

Delivery is Edition-2-paced and post-window: nothing here is a
`contract-v1` pre-flip obligation, and RFC 0536's own RTL acceptance
bar is separately gated on RFC 0491 WS-I.

Where this amendment and the frozen §3.3.15 text disagree, the
amendment governs.

## Amendment 8 (2026-08-26): the ActionSegments plan encoding (LLP 0535 AS-D4; 0504 §3 row 75)

**This is a formal amendment to this Accepted specification**, applied
under the same decided-content rule as Amendments 1–7. It executes
LLP 0504 §3 row 75 — the **window-sensitive §4 Track R rider** named in
that section's shared-clocks list — by writing the decided
ActionSegments encoding into this format's catalog and reserving its id
space. The deciding text is LLP 0535 §8.5: **AS-D4 = Option A,
normalized global graph tables** (Charlie Cheever, 2026-08-25, relayed
via orchestration session exact-9e, the orchestrator's recommendation
accepted; recorded at `af0041d98`, which also recorded AS-D1–AS-D3).
Nothing here re-opens that choice.

*Citation hygiene, because the ledger row gets this wrong: the
`decision: "A"` in `packages/exact-contract/contract-default-policy.json`
is **AS-D1's** field — the mode's default-on decision, per that file's
own `$authority` — not AS-D4's. AS-D4's record is LLP 0535 §8.5 itself.*

**Why this document is where the row closes.** LLP 0535 §8.5 records the
decision and then sends the encoding away from itself: "the encoding
itself is a separate 0485 Track R chunk (a distinct lane; nothing lands
here), and it is Track R **format work**." §3.3 of this spec splits that
format work in two — field lists are normative here, byte encoding is
owned by `packages/exact-contract/plan/format-schema.json`. This
amendment supplies the first half and **reserves** the id space the
second half will use. Item 9 states exactly why it reserves rather than
allocates, and item 10 names what remains.

**A reading note.** LLP 0535 §8.5's Option A paragraph is a *branch
menu entry*, not the model. The model is §4.2 (segments), §4.3 (await
sites), and §5.4 (carried state) — each of which contains a sentence
beginning "AS-D4 must…". Those sentences, not the menu, are what this
amendment is obliged to satisfy, and the field lists below are derived
from them.

**1. The graph-authority rule, restated as format law.** AS-D4's three
branches differ in one thing only — what holds segment and await-site
identity — and Option A puts it in global tables. In this format that
means:

- Two new global tables, **ActionSegments** (§3.3.8a) and **AwaitSites**
  (§3.3.8b), are the **sole** segment and await-site identity and graph
  authority for a plan.
- Action records **reference** them; they never restate them.
- There is **no** action-local segment-record section and **no** second
  segment or await table anywhere in the format. Option B is not
  partially available, and a future need for action-local storage
  returns as an amendment here, not as a parallel authority.
- **Boundary opcodes cannot define identity.** If a runner later wants
  `SEGMENT_BEGIN`/`COMMIT`-shaped opcodes as execution hints, they are
  hints and nothing more: a reader MUST NOT derive a segment id, an
  await-site id, or a graph edge from them. *This format's own choice,
  not AS-D4's:* a plan whose opcode hints **disagree** with the tables
  is **refused** rather than silently preferring the tables. AS-D4 only
  says hints cannot define identity; refusing the disagreement is the
  §3.4 fail-closed posture applied here, on the reasoning that a
  tolerated disagreement is two authorities for one fact with a winner
  picked at read time.

**2. §3.3.8a ActionSegments** — one row per statically derived segment.

LLP 0535 §4.2 fixes the model and the obligation: an ActionSegment's
**logical identity is `(invocation id, monotonically increasing segment
ordinal)`**, and "AS-D4 must encode it explicitly for Plan, receipts,
replay, and differential traces." A plan contains no invocations, so
*this format's choice, forced by that*: the table encodes the **static**
half — `(owning action ref, segment ordinal)` — and a runtime receipt or
trace names a segment as that static row **composed with** the runtime
invocation id. The two halves are never conflated, and the plan never
pretends to carry an invocation.

Row content, from §4.2's "logical segment record" list joined with
§8.5's:

- **Owning action ref** into §3.3.8 Actions, and the **segment ordinal**
  within that invocation's sequence (monotonically increasing, §4.2).
- **Authored statement/opcode extent** — the range into the `opcodeBytes`
  pool that §3.3.8's `body` column already targets. (Not an Exprs-row
  range: Actions bodies address `opcodeBytes`.)
- **Carried state** — see item 7; §5.4 makes this AS-D4's largest
  obligation and it is not a simple field.
- **Conservative read/write/`mutates`/command/effect masks.** §4.2's
  words are exact and this amendment keeps them: the masks are
  **subdivisions of the action's transitive effect row, not new
  authority**. Subdivision, not partition — two segments of one action
  may write the same slot, so the masks do not form a disjoint cover.
- **One journal decision** (§4.2; §5.1's write-ahead journal semantics).
- **One successor-suspension or terminal edge** — the successor segment
  reached through an await site, or the terminal arm reached directly
  (`ok` / `error` / `interrupted`, §6.6; §7.12 closes that set —
  `conflict-abort` does not exist).
- **Owner scope** — the invocation's owner scope in §4.2's sense, so an
  interrupted or HMR-removed segment's ownership is a static fact rather
  than a runtime search.

**3. §3.3.8b AwaitSites** — one row per **authored** await site.

LLP 0535 §4.3 fixes both the content and the prohibition: each authored
`await` is a site with "source action, statement locus, ordinal within
that action, success/error settlement arms, and the successor segment
reached after settlement", nested control flow does not allocate
synthetic sites, and "**AS-D4 chooses the physical Plan encoding but may
not merge distinct authored sites or lose their loci**." So:

- **Source action** ref and **ordinal within that action** — together
  the row's stable identity.
- **Statement locus — NOT a row column.** This is the one Option-A
  field that collides with the frozen body, so it is settled rather than
  copied. §3.5's loci rule is **by-table-class**: "a table whose rows
  have authored provenance participates", and both new tables have
  authored provenance. So AwaitSites and ActionSegments **participate in
  the loci segment**, and a site's locus is its loci entry addressed by
  `(table, row)` — *not* a column in the segment. That matters because
  §3.5 excludes the sidecar from `planDigest` and lets production builds
  strip it, "so stripping it never changes plan identity"; a locus
  column would put authored source provenance inside digest-covered core
  plan and destroy that strip-invariance.
  LLP 0535 §4.3's prohibition — AS-D4 "may not merge distinct authored
  sites or **lose their loci**" — is satisfied by §3.5's own floor
  rather than by a column: every row keeps its `(table, row)` address
  even when the sidecar is stripped, which is exactly what §3.5 requires
  so "a stripped-production trap is resolvable after the fact". §4.3's
  rejection path decorates the error with the await statement's locus;
  §3.5's floor is what makes that resolvable in a stripped build.
- **Descriptor and argument operands** (§8.5) — as expr refs. §5.3 admits
  the descriptor before start, so the plan carries what admission reads;
  §7.6's rule that descriptor and argument evaluation share the prefix
  transaction is why these operands belong to the prefix segment's
  extent rather than to the site.
- **Success and error settlement arms.** The success arm is a typeRef
  into §3.3.2 under §5.1's closed type table. *The error arm is flagged,
  not assumed:* LLP 0535 §6.6's `error` carries "an ordinary authored,
  descriptor, operation, queue-overflow, or runtime error" and §7.3
  preserves raw throwable identity, so it is **not** closed under §5.1
  today. Requiring a closed error typeRef would be a new obligation on
  §5.1, not a consequence of AS-D4; the encoding therefore carries the
  arm's **discriminant and settlement shape** and leaves the throwable
  open, and closing it is a separate §5.1 question if anyone wants it.
- **Successor segment** — the §3.3.8a row begun after settlement.

Merging two authored sites that happen to share a shape is prohibited
even when it would compress: identity is authored, not structural.

**Also owed at this boundary (LLP 0535 §5.3), and not supplied by expr
refs alone.** §5.3 records that the landed descriptor helper "is the
current admission witness and a **trusted assertion** on editable
engines", and states the obligation: "Compiler proof of descriptor
purity and the closed argument/result schema must become explicit in
AS-D4's Plan boundary rather than being inferred from an arbitrary
JavaScript closure." The operands above carry the *values*; the purity
proof and the closed argument/result schema are additional admission
material this amendment records as required and does not design here.
Naming it matters because the trusted assertion is precisely what must
not survive into the Plan path.

**4. §3.3.8 Actions gains its references.** The Actions row grows a
**segment range** into §3.3.8a (contiguous, ordinal-ordered), an
**entry segment** ref, and a **concurrency-policy** ref (item 5). *This
format's choice:* these are required columns on every Actions row rather
than framed optional columns, so segmented and unsegmented actions share
one row shape. The cost is stated rather than hidden — required columns
put the whole ≥ v1.7 addition behind a global `minReaderVersion` floor,
which is the same trade v1.5 made for required host-interface material
and which §3.1's paired floor rule already governs. An action compiled
without the mode carries an empty segment range.

**5. AS-D4's cross-branch list, placed.** AS-D4 binds *every* branch to
encode "independent/latest/serial policy, typed lane keys,
capacity/overflow, invocation/parent identity, HMR interruption
ownership, terminal arms, command-release ordering, and replay receipt
identity."

*This format's choice:* a **third** global table, **ActionConcurrency**,
carries the policy rather than columns on Actions. AS-D4 named two
tables as the sole *segment/await* authority and said nothing about
concurrency placement; a separate table keeps Actions' row stride from
growing with a policy most actions do not use, and keeps §6.2/§6.3's
lane vocabulary in one place. It is not a third *identity* authority —
segment and await identity remain items 2 and 3.

- The row carries the policy discriminant (`independent` | `latest` |
  `serial`) and the **typed lane key**. The key is not a general
  expression: §6.2 restricts it at compile time to a **literal or one
  captured action parameter**, so the column is that discriminated pair
  — a literal value, or a parameter index into §3.3.8's param list —
  beside the closed scalar type the runtime value must inhabit (`null`,
  boolean, finite number, string; `-0` canonicalizes to `0`). §6.2's
  lane identity is **action, scalar type, scalar value**; the plan
  encodes those so a runner does not reconstruct them.
  *This format's choice, with a consequence worth stating:* the row
  **also keeps the concurrency-key expr ref**, not only the projected
  literal/parameter pair. `ActionConcurrencyIR` carries `key: Expr`
  today, and LLP 0535 §7.2 requires that the compiler "includes the
  **concurrency-key expression** in the action target" for the LLP 0521
  join. Dropping the expression to keep only its projection would be
  cheaper bytes and would quietly break that join, so the projection is
  an addition to the expression, never a replacement for it.
- For `serial`, the mandatory **capacity** (a positive safe integer,
  counting the active item plus queued items) and the **overflow** arm
  (`error` is the only arm §6.3 and `ActionConcurrencyIR` admit; an enum
  column makes a second arm an enum addition, not a re-shape).
- **`independent` needs no row.** *This format's choice, stated because
  it is otherwise ambiguous:* independent is the §6.1 default and is
  encoded as the **absent** concurrency ref, not as a table row. An
  action with no explicit policy therefore costs nothing, and a plan
  with no explicit policies carries no ActionConcurrency table at all —
  which is what lets item 6's segment stay absent from mode-off plans.
- **Invocation/parent identity** is §4.2's — unique invocation id,
  action identity, owner scope, optional parent invocation. The plan
  encodes the static operands (action identity, owner scope, and the
  attached-child relationship §5.5/§7.5 fixes); the ids themselves are
  minted per invocation at runtime.
- **HMR interruption ownership** is the source path §4.3 registers a
  suspended invocation by and §6.5 keys interruption on. It is a static
  fact, and *the plan carries it by reference, not by copy*: an action
  already resolves to its component and thence to a §3.3.2a Modules row,
  and §3.3.16's declaration identity is already `(module id, owner, …)`.
  A second source-module column here would be the two-authorities-for-one-fact
  condition item 1 forbids, so the encoding resolves ownership through
  the existing chain instead.
- **Command-release ordering** is §5.2's atomic state-and-command release
  discipline. **What the plan encodes is the per-segment journal
  decision and command mask of item 2; the ordering law itself is runner
  law and stays in §5.2.** This is stated plainly because AS-D4's list
  reads as if ordering were a column, and it is not one.
- **Terminal arms** are the segment row's (item 2).
- **Replay receipt identity** keys on what these tables mint —
  the §3.3.8a row (owning action + segment ordinal) and, for a resumption,
  the §3.3.8b site — composed with the runtime invocation id, never on a
  body offset. *Which tuple* is this format's choice, forced by item 1's
  rule; that the identity comes from the tables is AS-D4's. §6.3 obliges
  future replay receipts to represent FIFO order, enqueue/start/terminal
  order, overflow, and cancellation, all of which need a stable key to
  hang on.

**6. §3.7 placement.** The tables ride one new **required-where-used**
segment: present exactly when the plan contains at least one action
compiled under the mode, absent otherwise, and never optional-skippable
— an unknown-but-required segment is a load error under §3.1. A plan
that carries the segment while claiming no segmented action, or that
claims one without carrying the segment, is refused.

**7. Carried state — AS-D4's largest obligation, recorded honestly.**
LLP 0535 §5.4 describes what the landed editable engines do today and
then states the obligation exactly: the engines capture **every**
`local` binding, "there is no runtime liveness mask", the compiler's
carried-read classification "is not a closed-type proof", and the
current path "treats this compiler classification and clone as a trusted
boundary." Its closing sentence is the requirement:

> AS-D4 must replace it with explicit typed carried slots, a liveness
> mask, and closed-value validation before Plan/native admission or a
> stronger closed-snapshot conformance claim.

The encoding therefore carries **three** things, not one: the **closed
typed carried slots**, a **liveness mask** over them, and enough type
information for **closed-value validation** at admission.

*Terminology, because this format already uses the word:* these are
**not** §3.3.1 Slots. §3.3.1 is "one row per **state** slot" with a
byte offset into the component's state struct, and §5.4 is explicit
that "State bindings are not part of the local snapshot." The carried
slots are a **continuation-frame** slot space introduced by this
encoding — locals, parameters, loop binders, and await-assignment
targets — typed by refs into §3.3.2 and addressed within the segment's
frame. That space is this format's, named here so the allocation chunk
does not silently reuse §3.3.1's. §4.4's §7.3 row says the same
in the ruling's own words — "a closed typed portable representation
remains AS-D4 work". This amendment does not invent the classification
algorithm; it fixes that all three are encoded rather than one, because
an encoding that carried only the slot list would let the trusted
boundary survive into the Plan path where §5.4 says it must not.

State bindings are **not** carried (§5.4): every successor segment reads
then-current state, and the snapshot/lost-update behaviour of a local
computed from state is intentional Option C law.

**8. The id-space reservation — the window act.** Amendment 2 established
this format's method for a window-bound rider, and its reasoning runs
the other way from restraint: it *allocated* reserved ids so that a
later adoption "is not a format break", and justified doing so with
"retirement is cheap; foreclosure was not" — i.e. reserve now, retire
later if the venue rejects. That method is adopted here, in the
authority's own id-bookkeeping idiom (the `$comment` allocation split
that already records "LLP 0514 M1 WP2 owns host-interface segmentKind 25
and tables 74-77; Track R Tier 1B owns optional partitions segmentKind
26 and tables 80-87"):

**LLP 0535 AS-D4 owns segmentKind 27 and tables 88–90.**

That reservation is **machine-enforced**, not merely described here. It
lands as an `idReservations` row in `format-schema.json`, and
`generate-plan-format.mjs` — the generator behind the registered
`plan-format-parity` and `plan-format-generated-freshness` checks —
refuses any schema that allocates a reserved `segmentKind` or table id
while its reservation stands, naming the owner and instructing the
allocating change to remove the row in the same edit. Deleting the
`idReservations` key is itself a generation error.

That mechanism exists because the alternative was tried and failed
silently: Amendment 2 said the island row-kind "ids allocated in the
format schema", and they never were — `format-schema.json` contains no
`island` token, `depSourceKind`/`depTargetKind`/`regionKind` are
unchanged, and neither Amendment 2's nor Amendment 3's commit touched a
plan authority, yet LLP 0504 §3 row 24 stands terminal on it
(`issues/20260826-llp0485-island-format-reservation-never-landed.md`).
**A reservation that lives only in prose reserves nothing**, because the
next segment or table insert allocates over it without anything
noticing. That is the whole failure this gate removes, and the island
reservation can adopt the same `idReservations` mechanism whenever its
owner chooses to make the row-24 claim true.

**9. Version target, and why ids are reserved rather than allocated.**
The encoding lands as a **≥ v1.7** minor addition with
`minReaderVersion` moving with it, per the v1.4→v1.5 required-material
precedent and Amendment 4's v1.6 pairing. v1.6 is spent on the
enum-roster digest, so v1.7 is the first free minor — the same re-target
LLP 0520's `styles` segment executed at its r6.3.

This amendment fixes normative content and reserves the id space; it
does **not** allocate column ids, strides, or offsets. That is not
timidity, and it is not this amendment's preference — **AS-D3 forbids
the alternative.** AS-D3 = Option A (Charlie Cheever, 2026-08-25)
requires that "the first native-executing ActionSegments chunk lands
AS-D4's explicit records, generated readers, the TS-plan oracle, and the
native runner **together** behind one non-default gate", and explicitly
rejected Option B, the representation-first delivery that would land the
format bytes on their own. Freezing a column layout here would be
Option B's first half, taken against a decided ruling — and it would
freeze it against a compiler graph that does not exist to validate it
(item 10). Reserving the id space gets the whole window benefit,
because what a later allocation could break is the id space, and that is
now held.

**10. What remains, named so it does not rot.** The layout is allocated
in `format-schema.json` — the §3.3 byte-format authority every reader,
IR ledger, format reference, and golden fixture is generated from — as
part of AS-D3's single co-designed chunk.

The precondition sits on the **compiler**, and is worth naming because
it is invisible from this document: **there is no static ActionSegments
graph anywhere in the tree today.** `ContractFileIR.actionSegments` is a
bare `true` flag, `ActionIR.concurrency` is the only related IR shape,
`compiler/action-segments.ts` is a validator rather than a lowering, and
segmentation happens dynamically in `runtime/action-segments.ts`. Items
2–7 describe rows a compiler must first learn to derive. Until that
chunk lands:

- the mode's Plan refusal stands — `actionSegments + plan` rejects with
  `action_segments_plan_lowering_unavailable` (LLP 0535 §4.1, §9), and
  the vite plugin refuses the production-program path; and
- `ContractFileIR.actionSegments`, `ActionIR.concurrency`, and
  `ActionConcurrencyIR` remain `rejected` dispositions in the recursive
  IR ledger — **with reasons that now name this decision instead of
  pointing at LLP 0522.** Those three reasons said the lowering "is not
  admitted in this implementation chunk", which reads as *undecided*
  rather than *decided-and-unbuilt*, and they are corrected.

  Two neighbours are deliberately **not** corrected here, because each
  belongs to someone else and claiming otherwise is how the reasons went
  stale in the first place. The compiler and plugin diagnostics
  (`action_segments_plan_lowering_unavailable`; the vite
  production-program refusal) carry the same "not admitted" wording and
  are **correct about implementation** — lowering genuinely is not
  built — so they stay. And `llp/refresh-ledger.json` still projects
  "AS-D1–D4 **owner-pending** (row 75)" from LLP 0504 §2, which is now
  simply wrong — all four were decided on 2026-08-25 — but that is a
  generated projection of the map, guarded by `refresh-map-projection`,
  and the map's rows are the map lane's to refresh, not this
  document's.

**11. Joins, one of them still owed.** AS-D4 requires every branch to
join LLP 0508 §10.4 schema totality and LLP 0521 graph identity.

- **0508 §10.4** is satisfied in its own terms: every IR field reaches
  one of the four dispositions. The ActionSegments IR fields reach
  `rejected` today, by item 10, and reach `encoded` at allocation; the
  totality obligation is on the disposition being *stated*, which the
  ledger does.
- **LLP 0521 graph identity is NOT discharged here, and saying so is the
  point.** 0521's identity space is static-dependency nodes and
  occurrences; segment and await-site rows are a new space. Whether they
  join 0521's graph as a new node kind or stand beside it is a real
  question this amendment does not answer, and it is owed with the
  allocation chunk rather than assumed. Recording it as owed is the
  honest half of a "join" AS-D4 asked for and this document cannot
  unilaterally supply.

**12. What this amendment does not claim.** It does not enable the mode
on the Plan path, does not implement lowering or a native runner, does
not change `.eplan` bytes, and does not move the format version — no
committed plan artifact changes because of it. Its window claim is
narrow and exact: the ActionSegments encoding *shape* is decided and
specified in-window and its id space is reserved in the format
authority, so its later allocation under AS-D3's co-designed chunk is an
additive v1.7 minor rather than a format break.

**13. The frozen §7.1 bullet this amendment stands beside.** §7.1's
closing normative bullet reads "No `await` in an action body —
asynchrony re-enters as typed events dispatched by tasks and
settlements", and LLP 0508 §10.3 carries the same rule for Edition 1.
This amendment defines an `AwaitSites` table, so the relationship must
be stated rather than left for a reader to trip over: **§7.1's rule is
unchanged for Edition 1 and for every plan not compiled under the
mode.** `actionSegments` is an accepted LLP 0508 §14 numbered amendment
(§13 item 17) whose §4.1 selection is explicit, default-off, and
requires `staticDependencies`; awaits exist only inside that flagged
mode. The tables specified here are therefore the encoding of an
explicitly amended language surface, not a relaxation of §7.1 — and
§7.8's codemod/corpus landing condition for change 3 is untouched by
anything here.

**14. A classification tension this amendment records and does not
resolve.** *(Descriptive only — this item declares no window standing
for any ask, and nothing in it should be read as a classification. The
declarations it discusses are made in the sections and documents it
names.)*

Three sections below, this document's own "Window relation for the
plan-format asks (recorded 2026-08-26)" answers one question — is
`.eplan` inside LLP 0506 D1(a)? — and settles five plan-format asks on
that answer. One of the five is LLP 0520's `styles` segment: a
plan-format segment addition, riding the same §4 Track R clock as the
encoding specified here, whose v1.7 re-target item 9 above borrows as
precedent. It sits on the far side of that line. The ask this amendment
executes sits on the near side, on LLP 0535's owner declaration.

So the format owner and the extension owner have answered the same
question differently for structurally identical changes, and
`llp/refresh-asks.json` faithfully records both, each as `basis:
owner-declaration`. **This amendment does not settle it**, for two
reasons: the classification belongs to LLP 0504's map lane and to those
two owners, not to a fold; and this amendment's content is correct
either way — decided content folded into the format catalog is worth
landing whether or not the clock is urgent. What turns on the answer is
urgency, and whether LLP 0506 D1(d)'s open surface counts this ask at
all.

The escape clause that section already records — "if LLP 0507's owner
reads 'the generated authorities' more broadly … this classification is
revisited for all five together" — is the natural venue, and the ask
this amendment executes should be revisited *with* that group rather
than on its own.

**15. What this amendment claims about the ledger row: nothing.** It
executes row 75's named residue as far as a format document can — the
encoding is specified and its id space is reserved and gated — but it
does **not** assert that row 75 is now terminal. `issues/20260826-d1d-closure-plan.md`
states the row's terminal condition as "land the encoding artifact", and
items 9–10 are explicit that the layout allocation is still ahead under
AS-D3's co-designed chunk. Whether that makes the row terminal, partial,
or unchanged is LLP 0504's map lane's derivation. Stating this is not
modesty: the co-landed island ticket exists because a row went terminal
on an amendment's say-so, and an amendment that quietly implied its own
discharge would be the same move.

Where this amendment and the frozen §3.3, §3.5, §3.7, or §7.1 text
disagree, the amendment governs.

## Window relation for the plan-format asks (recorded 2026-08-26)

Five LLP 0504 §3 rows propose to extend this format or its owner
vocabulary, and one question settles all five: **is `.eplan` inside
LLP 0506 D1(a)?** It is not. D1(a) names "the EXWF frame revision and
the generated authorities" — EXWF is the **wire** format; `.eplan` is
the **plan** format, separately versioned (v1.6 today) with its own
reader-floor discipline. "The generated authorities" scopes to the
authorities EXWF itself pins — schema package, published tuples,
consumer census, ledger rows — not every generated authority in the
repo. Charlie's 2026-08-26 route-manifest ruling reinforces the
separation: taking the manifest **out** of plan-format territory into
an app-grain sibling binary family treated format carriage as a
separable concern.

LLP 0504 §3 row 3 is non-window (D2).
LLP 0504 §3 row 4 is non-window (D2).
LLP 0504 §3 row 29 is non-window (D2).
LLP 0504 §3 row 75 is non-window (D2).
LLP 0504 §3 row 89 is non-window (D2).
LLP 0504 §3 row 90 is non-window (D2).

**Row 75 joined this set on 2026-08-26, by a ruling that resolved a
genuine disagreement between two owners.** LLP 0535 had declared it
window-bound, reading its own §4 phrase "the window-sensitive §4 Track
R rider" as a classification. That declaration is withdrawn there and
the row is classified here instead, on three grounds:

- **One authority per boundary (LLP 0150).** The plan format's window
  standing is a property of the plan format, and this spec is that
  authority. LLP 0535 is a *consumer* proposing a segment; a consumer
  cannot classify the format it consumes, or every consumer classifies
  it differently and the ledger records whichever spoke last.
- **"Window-sensitive" is not "D1 names it."** Window-sensitive
  describes an author's schedule and is true of anything that wants to
  land before a freeze. Only LLP 0506 D1's own enumeration, or an owner
  declaring against it, creates exit surface.
- **The consequence test.** The AS-D4 ActionSegments encoding is
  structurally identical to the `styles` segment (row 29) and the
  Cells-table and Entrypoints extensions (rows 3, 4): new global
  tables or segments in `.eplan`, all at ≥ v1.7, all on Track R. If
  row 75 were window-bound, so would all of those be — and this spec
  has explicitly classified them otherwise. A reading that forces an
  authority to contradict itself is the wrong reading of that
  authority.

Nothing about AS-D4's substance changes: the decision is taken and the
encoding artifact is still wanted early. Non-window is a statement
about the exit criterion, never about priority.

Two consequences worth stating rather than leaving to be inferred.
Row 3 leaves the window-exit critical path **without touching the
pending additive-vs-floor decision on it** — that decision stands on
its own merits; it simply is not a D1(d) conjunct member. And this
classifies window standing only: a non-window row can still be
urgent, and rows 89/90 gate dev-loop work that is waiting.

**Escape clause, recorded with the ruling:** if LLP 0507's owner reads
"the generated authorities" more broadly than the EXWF-pinned set
above, this classification is revisited for **all six together** (rows
3, 4, 29, 75, 89, 90), because they stand or fall on that one reading.

**Row 24 is routed to this venue but is deliberately NOT classified
here, and the counter-argument travels with it.** Its island
certificate/format reservation is on the same Track R clock, so the
reflex is to fold it into the six above. Two facts argue it may
genuinely differ, and whoever rules should have the strongest form of
them rather than re-deriving it:

- **A certificate is an admission artifact, not purely a format
  extension.** Admission sits on LLP 0506 D1(b)/D1(c)-adjacent ground —
  conformance corpora and capability posture — where a `styles` segment
  and an ActionSegments table simply are not.
- **The island rider was framed as deliberately narrow *because*
  RFC 0518's adoption was itself a window event.** That framing is not
  the scheduling language the row-75 correction above disposes of; it
  was a claim about what the window contained.

Its previous `window-bound` class cannot be repaired by re-reading,
because it was never a classification: it was harvested from a sentence
in RFC 0537 §5 that was *retracting* exactly that analogy. The row needs
an owner to say something new, and an honest unclassified beats a
plausible guess.
