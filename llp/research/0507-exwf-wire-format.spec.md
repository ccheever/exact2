# LLP 0507 — EXWF: the Exact Wire Frame Format

**Type:** Spec
**Status:** Accepted
**Systems:** Kernel, Protocol, FFI, Apple Host, Windows Host, Android, Web, Acto, Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-20
**Revised:** 2026-08-26 (LLP 0506 D1(a), sixth landing — **open issue 9 is ruled** and the §8.1 `frame-header` class is concrete, leaving two activation gaps. **An event frame carries the §7.1 attribution tuple and no batch number and no commit generation**: events are input, not commits, so a batch number would require the host to know the runtime's batch state and a commit generation describes a commit the event is not part of. Frame-kind discriminants are 1 (command) and 2 (event), with 0 reserved so a zeroed buffer refuses E-3 rather than decoding as a command frame. **One correction to the ruling as given, made on this document's own evidence:** it named *the root's incarnation coordinate*, but the fence this carriage exists to serve (`mechanism/event-generation-fencing` → `unit-incarnation-fence`) discriminates a completion carried over from a *replaced JS runtime*, and §5.3's identity table records `rootIncarnation` **holding** on `ProducerRestart` and `CoherentReload` with the note \"re-fenced by ExecutionGeneration\" — it bumps only on `RootReset` and `KernelReset`. A root incarnation alone therefore could not refuse the stale cross-runtime completion. The frame carries the whole tuple instead — issued context (`ProducerId`, `ExecutionGeneration`) plus target root plus `rootIncarnation` — which is the same attribution a command frame carries under the issue-8 carriage ruling, so this is the minimal consistent choice rather than a new envelope. A host supplies both halves by construction: it holds the issued context from attach, and issue 8's root→incarnation map resolves the incarnation for the root it reports against. Both headers are packed from field lists by the stated rule; padding runs are emitted as zero-checked byte runs, never as fictional integer widths. Blockers fall from three to two, and the census reaches fourteen concrete of sixteen.) 2026-08-26 (LLP 0506 D1(a), fifth landing — **open issue 8 is ruled** and the §8.1 `capability-context` class is concrete. Topology: **one context bearing a root→incarnation map**, selected because this spec's own `capabilityContext.logicalFields` already declare `rootIncarnations` as a root-to-u64 association — per-root contexts would make that declared field permanently a singleton and would duplicate the per-producer `ProducerId`/`ExecutionGeneration` into N copies. Attribution carriage: **frame-carried**, because V-21 requires replay to apply the same admission rules as live and a replayed frame has no connection, so connection-bound attribution would force replay to synthesise one and diverge exactly there; frame-carried is also the incumbent (`BufferHeaderV3` already carries `root_id` per frame). The accepted cost is named: V-26's sibling fencing moves from a structural guarantee to a **tested invariant**, enforced at generation — a root-scoped reset must advance exactly its own root, leave every sibling byte-identical, and leave producer identity untouched. That invariant pins the specified semantics only; no context implementation exists yet and each kernel or host owes its own V-26 vector at activation. The physical layout is packed from the logical fields by a stated rule (declaration order, natural alignment, explicit zero-checked reserved gaps, tail padding) rather than authored: an 88-byte fixed prefix followed by the root-incarnation and attach-issued-limit extents. **Open issue 9 is NOT ruled**: §12.3's batch-less shape governs the §12.1 upward family, whose three delivery owners do not include input events, and §6.5 makes the event frame a distinct frame kind — so `frame-header` stays an activation gap pending that selection. Blockers fall from four to three; the census reaches thirteen concrete of sixteen.) 2026-08-26 (LLP 0506 D1(a), fourth landing — the §8.1 `style-layouts` class is **concrete**: all 88 wire style bits carry a **derived** wire encoding, absent-field default, Auto admission, and generated Taffy mapping. Wire encoding is read from each bit's `StyleProps` carrier type through a closed type map (a carrier type outside it is an error, never a default); the default and the layout/non-layout partition come from the registered non-layout default authority the package already digest-pins, so the style table consumes that authority rather than re-deriving beside it. **Auto admission is resolved against the vendored Taffy type system**, which is the CSS grammar §6.2 names and is the engine that computes the layout: `size`/`flex_basis` are `Dimension` and `inset`/`margin` are `LengthPercentageAuto` (all admit `auto`), while `padding`/`border` are `LengthPercentage` and admit none — so padding refuses `Auto` exactly as CSS does, with no per-row judgment written anywhere. 15 of 88 rows admit `Auto`; the other 73 cannot represent it. The §8.1 census check for this class now requires each sub-property to be `derived` and to name its authority, which is strictly stronger than the `concrete` status it replaced. Activation blockers fall from five to four; the census reaches twelve concrete of sixteen. No activation, kernel-cutover, or vector-satisfaction claim.) 2026-08-26 (LLP 0506 D1(a) item 5 — seven conceptually integral scalar props are branded `Int32`/`Uint32` at the declaration (`packages/exact-renderer/src/types.ts`), so the §6.2 derivation reads `i32-le`/`u32-le` from the authority instead of the `number → f32` default, which is exact only to 2^24. The brands are read from the *declared name* before alias resolution, since they are structurally `number`; a brand that stops resolving to `number` is an error. Signed where the value space genuinely is (HTML `tabindex` uses -1, ARIA `aria-setsize` uses -1 for "unknown"), unsigned where the standard says non-negative (DOM selection offsets, `aria-level`, `aria-posinset`). Zero authored per-prop wire types: the declaration says what it means and the derivation reads it. Blockers and census unchanged.) 2026-08-26 (LLP 0506 D1(a), third landing — the §8.1 `typed-prop-table` class is **concrete**: all 50 legacy stringified-scalar props carry a revision-1 wire type **derived** from the producer-side declaration, never authored. The chain is `SCALAR_PROP_ENCODINGS` (declared `readonly [keyof CanonicalProps, PropId][]`) → `CanonicalProps` → each key's declared TypeScript type, with bidirectional set-match totality against the inventory and closed shape classes; an unclassifiable declaration is an error, never a default. Four declarations that stated no single type were ruled and narrowed at the authority: `focusScope` → `boolean | 'trapped'` (the `| string` widening had absorbed the literals and hidden the one spelling every producer actually emits; `contain`, `restore`, `containRestore`, `contained`, and case variants are refusals, and the React tier was aligned to the same spelling), `accessibilityLive` → the closed ARIA set `'off' | 'polite' | 'assertive'`, `accessibilityChecked` → the ARIA tri-state, and `glassEffect` → an admissible value-tag set `{Boolean, Utf8}` (deliberate two-shape API; expressible because the u8 value tag rides with every value, so §6.2's "booleans as booleans, never `\"true\"`" holds exactly). The §8.1 census check was retargeted from a row-count proxy ("no stringified-scalar rows", a statement about the unchanged LEGACY encoder) to the substantive property, and strengthened: every legacy scalar row must be derived from `CanonicalProps`, so a hand-authored wire type reddens where the proxy would have passed. Activation blockers fall from six to five, and the §8.1 census from six activation gaps to five (eleven concrete of sixteen). No activation, kernel-cutover, or vector-satisfaction claim.) 2026-08-26 (LLP 0506 D1(a), second landing — the two deferred §8.4 companions are discharged, so the activation semantic epoch is no longer blocked on a missing member. The **breaking-window ledger rows** are minted as `tests/protocol/exwf/breaking-window-ledger.json`: eight rows projecting RFC 0491 Phase 2's decided in-rev list, every row validated with ajv against the committed W0-A row schema, `coreMembershipClass` DERIVED from each row's non-waivable kind by the ratified RFC 0496 §4.2.1 seed mapping rather than authored, and every `exwf-vector:` fixture id resolved against this document's §15 table. The four non-waivable rows are exactly the RFC's four classes — active-producer-codec, identity, recovery-transactionality, entry-security. The **canonical consumer census** is promoted to `tests/protocol/exwf/consumer-census.json` from the W0-A hand-verified inventory, with its coverage obligation DISCOVERED from the generated sidecar-family roster (which immediately found the opcode-118 virtual-topology-certificate family uncensused — two rows added), anchors resolved to path-and-line-in-range, and reservation-only families excused by their inventory exemption rather than by hand. The two are joined **exact-once in both directions**, so the ledger can neither green by omission nor claim a consumer that does not exist. The ledger rows join the §8.4 semantic package as the ratified member they always were, so the draft semantic epoch now covers them; `deferredMembers` is empty. Activation blockers fall from seven to six. No row is claimed satisfied, no consumer is claimed migrated, and `assertActivationReady()` still refuses on the remaining §8.1 class gaps.) 2026-08-26 (LLP 0506 D1(a) — two §8.1 activation gaps closed mechanically, no semantic change to this spec. (1) The §6.2 clear operation received its numeric identity: `ClearStyle` at 12/`0x0C` in `tests/protocol/protocol-inventory.json`, reservation-only under a per-instance Rust-mirror exemption (the 119/`0x77` precedent) — no producer, consumer, or kernel mirror exists before the revision-1 cutover — and the break package now *derives* the clear-rules class and its blocker from the inventory instead of asserting the row's absence. (2) The registered non-layout default authority §8.2 names now exists: boundary `exwf-non-layout-defaults`, authority `tests/protocol/non-layout-defaults.json`, generated from `kernel/src/style.rs` with its style-bit watch list discovered from the protocol inventory and layout-vs-non-layout classified by what `StyleProps::to_taffy_style` reads; its canonical projection digest enters the schema-package preimage, so a non-layout default change cannot move under an unchanged schema digest. The draft package's activation blockers fall from nine to seven and its §8.1 census from seven activation gaps to six. No activation, kernel-cutover, host, or vector-satisfaction claim is made or implied.) 2026-08-25 (the `gpu-canvas-attachment` sidecar family was allocated mechanically at opcode 119/`0x77`, with its §11.3 recovery row and generated break-package treatment; this executes LLP 0512 O-11's allocation half and LLP 0504 §3 row 21, while the producer remains not live and the pre-native-claim conformance residue remains owed.) 2026-08-25 (§8.4 member-list amendment — SemanticFrame IR (RFC 0491 WS-H, `tests/protocol/semanticframe/v1/schema.json`) added to the ratified semantic-package member list, ruled by Charlie Cheever 2026-08-25 (decision relayed via orchestration session exact-9e; register item 5); mechanical landing by agent under the standing semantic-decisions rule. The implementation moves SemanticFrame from the break-package `companionConsumers` digest pin (the sf-2 landing @6c6e75e72) to a true enumeration member; the draft semantic epoch flips accordingly, and because the epoch rides attach and admission (§8.4/§9.3), SemanticFrame schema changes now flip attach compatibility. Frozen preimage rules (canonicalization, domain tag, no self-reference, path+content-digest enumeration) untouched; break package regenerated in both generator modes.) 2026-08-23 (the four RFC 0491 §8 companion amendments landed — ratified by Charlie Cheever 2026-08-23 as drafted in LLP 0491.001, decision relayed via orchestration session exact-9e; mechanical landing by agent under the standing semantic-decisions rule: `A-0507/0515-PRODUCER-SUCCESSOR-ID` (§5.3 fresh mint on new-generation events; §14 item 6 resolved; V-5 extended), `A-0507/0515-BATCH-SUCCESSOR` (§7.3 successor key/initial 1/refused-consumes-nothing/same-key reconnect resume, shipped web gate = reference behavior; §9.2 migration note discharged; E-10 row updated; §14 item 7 resolved), `A-0507/0515-ROOT-MIGRATION` (§5.3 tenth enum row + atomic rekey prose; E-18 `migration-refused`; V-28), `A-0507-EXWF-SEMANTIC-EPOCH` (§3 term, §7.1 context field, new §8.4 preimage, §9.3/§9.4 `exwf-semantic-epoch` dimension, §10.1 triple, V-29). Break package regenerated per the amendments.) 2026-08-20 (**ACCEPTED by Charlie, 2026-08-20 (batch acceptance of the reviewed platform specs). This spec is now the EXWF wire-format authority; RFC 0491 references it; tests/protocol/protocol-inventory.json remains the opcode authority per §2/§6. Terminal loop state: grok READY at r2, both families' full residue adjudicated through the freeze+delta extension with complete ledgers.**)
2026-08-20 (seam-batch adoption notes — the corpus-review load-bearing seams' recorded defaults marked DECIDED by Charlie, dated annotations only) (r6 — **final close-out fixes; unreviewed** (no
review covers this revision; the delta-2 verdicts bind to r5). All
delta-2 findings adjudicated: the "n/a cells" stale vocabulary →
scope-end; open issue 9's index now repeats §6.5's ownership split
(framing + session/batch = schema package; admission ownership =
ledger/census); the residual "every frame" batch/context claims scoped
to command frames with event-frame pointers (§7.1 attribution rule,
§3 terminology); the recovery owner neither owns nor coordinates the
producer/presentation sides (each side owns its steps); the E-1
header/body locus split completed — header-locus E-1 listed in
partition set (i), body-locus in (ii), the §13 E-1 row split by
locus, and the header rule made attribution-mode-neutral
(connection-bound session/root fields are association state, not
frame bytes); §6.5's §11.1 exclusion labeled decided-not-interim with
only the handling shape interim. Codex's demand for literal bump/hold
values in session-ending cells remains rejected — ledgered with
evidence in the close-out disposition ledger.)
2026-08-20 (r5 — delta-1 fold under the scope freeze:
structured-apply parity scoped to the commit path (not frame-envelope
admission); the identity-events table's session-ending cells restated
as scope-end (no bare n/a third state; generated-matrix obligation
recorded); rootId "naming" corrected to attributed-target-root at
§9.2/E-14/V-26 per §7.1's carriage-neutral rule; event admission
*ownership* re-attributed to the break-package ledger (schemas stay
schema-package); §7.1 frame-carriage requirements scoped to command
frames; attach-check precedence demoted to package-owned; the recovery
owner scoped to its side of the recovery transaction; the §11.1
partition restated as exclusive per refusal instance; header-decode
E-1 split (simple refusal at the header, content rejection past it);
§9.4's envelope mislabel fixed; the §12.2 coverage row and §6.5
leftover unknown-payload clause corrected; V-13 narrowed to
batch-originated records; §15 records the vector-coverage debts.
Rejected delta-1 items are ledgered in the punch files.)
2026-08-20 (r4 — extension freeze-fold; see Revision
history) 2026-08-20 (r3 — sr-specs loop round-2 fold) 2026-08-20 (r2 —
round-1 fold)

> **Scope freeze (2026-08-20, author-authorized extension rounds):**
> this specification's normative surface is closed as of r3. Extension
> revisions make corrections and restorations only; demands for new
> normative closure are recorded as obligations, never folded as new
> normative prose. Delta reviews judge the r(N−1)→rN diff and the
> punch-list dispositions.
**Related:** RFC 0491 r16 (the Kernel Refresh Program — the design authority this spec extracts; WS-A identity, WS-D frame/break, WS-C receipts, §5 Phase-2 ledger), LLP 0488 §3.8 (report keys on the six-field address), LLP 0506 (breaking-window exit — conjunct (a) is this spec's machinery being in force), `tests/protocol/protocol-inventory.json` (the LLP 0150 opcode/ID authority this spec consumes and never forks), the 2026-08-20 spec-wave authoring directive (author-ordered extraction; LLP 0505 r3 is the decision docket that shaped the program context, not the charter for this document), LLP 0417/0500 (the ExecutionGeneration/HotRevision algebra §5 binds to), LLP 0331/0333 (App-ABI coordination, §10.4), LLP 0374 (Acto `producerIncarnationId`, §5.6), RFC 0495 (receipt/evidence consumer; upward-record sequence/gap/retention rules §12.2 inherits), RFC 0496 (non-quarantinable check classes the conformance suite lands into), LLP 0362 (crash-capsule projection governing SemanticFrame, out of scope here)

## Status of This Document

This document is the **wire-format specification** for EXWF, the framed
binary protocol between Exact producers (JS runtime, Rust Native
producers, replay/test harnesses) and the Exact kernel and hosts. It is
extracted from RFC 0491 r16, which remains the **program document**
(sequencing, workstreams, gates). Division of authority:

- Until this spec is Accepted, RFC 0491 r16 is authoritative and this
  document is its restatement; a disagreement between the two is a bug
  **here**.
- On acceptance, **this spec owns the wire format** — frame structure,
  identity algebra, capability context, schema digest, admission,
  recovery classes, error handling — and RFC 0491 references it. The
  program document keeps sequencing, gates, and workstream ownership.
- `tests/protocol/protocol-inventory.json` remains the **opcode and
  identity-table authority** (LLP 0150, boundary `protocol-abi`).
  This spec defines the *frame* that carries ops and the *rules* that
  admit frames; it never enumerates opcode values, prop IDs, or style
  bits. At the break, the inventory's **frame-header rows**
  (`BufferHeader`/`BufferHeaderV3`, the trusted `op_count`, and
  `protocol_version` 1–3) are replaced by the EXWF frame schema — that
  replacement is explicit, and this disclaimer covers only the
  opcode/prop/style enumerations, which stay inventory-owned. The
  inventory's **`OpHeader`** row (the 7-byte unaligned op header) is
  likewise replaced by the schema package's 8-byte aligned op header
  (§6.1) — named here so exactly one byte-layout authority remains.
- The generated **break package** (RFC 0491 Week 0) is the single
  reviewable artifact containing the **schema package** plus its
  companion authorities (the identity-carrier matrix, recovery-class
  assignments, ledger rows, golden preimage/digest fixtures, and the
  consumer census). The `exwfSchemaDigest` (§8) is computed over the
  **schema package only** — never over the companion authorities, so a
  ledger or census change cannot move wire identity (RFC 0491
  Phase 2). The companion authorities carry their own independent
  identity, the **`exwfSemanticEpoch`** (§8.4) — a companion change
  moves the epoch, never the schema digest, and the two dimensions
  move independently (§7.1).
- **Conformance precondition:** no implementation can claim
  conformance to this spec before the schema package and its golden
  fixtures exist and are registered (§15). This spec plus the schema
  package together form the encoding authority; the spec alone is
  deliberately not sufficient to encode a frame (§6.1).

Nothing in this document re-decides RFC 0491 r16. Where r16 leaves a
question open, the corresponding section carries an inline
**[Open issue]** marker (§14 indexes them) rather than an answer.

## 1. Scope

EXWF (the **EXWF frame revision**; deliberately not "protocol v4" — the
numerical `protocol_version` counter 1–3 is retired with its producers)
covers:

1. The **frame**: header, op stream, extension-chunk envelope (§6).
2. The **capability context** issued at attach and the per-frame
   identity it fences (§7).
3. The **identity algebra**: the six-field raw address, its minting,
   bump, and hold rules (§5), and the fence domains that make it
   enforceable (§5.7).
4. The **schema digest**: the canonical preimage and the
   revision/digest lockstep rule (§8).
5. **Admission**: what a conforming gate accepts, refuses, and reports
   (§9), including EXFF baked-frame and Exact Native artifact
   admission (§10).
6. **Rejected-frame recovery**: the per-sidecar-family recovery classes
   and their declared properties (§11).
7. The **upward family**: receipts and sidecar records — one encoding
   and correlation design, three delivery owners (§12).
8. **Error handling**, total over malformed input (§13).

Out of scope, with owners: opcode/prop/style enumerations
(`protocol-inventory.json`); EXNODE/feed export schemas
(`docs/host-data-plane-feed.md`, RFC 0491 WS-C); the SemanticFrame
replay IR (RFC 0491 WS-H; LLP 0362 governs its production projection);
motion evaluator semantics (RFC 0492); the C ABI and IDL (RFC 0491
WS-B/WS-E); layout semantics and defaults (LLP 0486/0487 authorities).

## 2. Conformance

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**,
**SHALL NOT**, **SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **MAY**, and
**OPTIONAL** in this document are to be interpreted as described in
RFC 2119.

### 2.1 Conformance classes

**Frame producer.** An entity that encodes EXWF frames (the TS encoder,
Rust Native producer encoders via the shared wire crate, replay
harnesses). A conforming producer MUST encode against the exact schema
package identified by its capability context's
`{exwfFrameRevision, exwfSchemaDigest}` (§7), MUST emit monotonic
per-root batch numbers (§7.3), MUST hard-error on unknown enum values in
every build (§13, E-2), and MUST NOT emit any op, discriminant, or
container form absent from the schema package.

**Frame consumer.** An entity that decodes and applies frames (the
kernel decode core, the web-host decoder, the Android JNI
typed-operation projection, `HostInterpositionFrame` in-process decode).
A conforming consumer MUST validate before applying
(validate-then-apply is a guarantee, not a mechanism — RFC 0491 §2),
MUST treat every malformation in §13 as a typed refusal (never a silent
skip), MUST check `frame_len` and MUST NOT trust any producer-supplied
op count, and MUST apply typed slice casts only after checked base
alignment, falling back to unaligned copy (§6.1).

**Admission gate.** The entity that admits a producer at attach and
each frame thereafter (the kernel where one is present; the TS web
admission owner on the kernelless web host). A conforming gate MUST
issue the capability context (§7), MUST verify the revision/digest
lockstep rule (§8.3), MUST enforce batch-number gap rejection (§9.2),
and MUST report the exact mismatched compatibility dimension on a
compatibility-mismatch refusal (E-7/E-8 — §9.4; every other refusal
names its §13 rule).

**Sidecar-record producer.** An entity that emits extension-chunk
records (motion snapshots, list-model chunks, graphics publications).
A conforming sidecar-record producer MUST declare its family's
recovery class per §11 and MUST key every cross-stream reference on
the full raw address (§5.7).

**Upward-record producer.** An entity that emits upward records
(LLP 0488 reports, commit receipts, present receipts, GPU
completions). A conforming upward-record producer MUST stamp its
delivery owner's monotonic sequence, carry `causedBy` links, and key
node references on the full raw address (§12); upward-record failure
handling follows §12.2's rules and never §11's recovery machine.

**Upward-record consumer.** An entity that reads upward records (the
Acto host surface, RFC 0495 evidence consumers). A conforming
consumer MUST detect per-owner sequence gaps (gap detectability is
RFC 0495's decided requirement — §12.2), MUST NOT assume a total
cross-owner order or renumber a domain-filtered view (§12.2), and
MUST validate carried node references as full raw addresses (E-11).

**Schema-package producer.** The generator that emits the schema
package and its golden fixtures (RFC 0491 Week 0/WS-B). A conforming
producer MUST emit one canonical serialization (§8.2), MUST keep the
package closed over §8.1's list, and MUST regenerate the golden
vectors (§15 V-1/V-2) with every package change.

**EXFF reader.** A consumer of baked first-frame containers. A
conforming reader MUST enforce §10.1's container-version rules
(E-12) in both directions.

The **kernel** and every host decode path are Frame consumers; §4.1's
one-commit-engine rule binds the Frame consumer class (a consumer that
exposes a mutation surface bypassing the command-buffer commit engine
is non-conformant; the in-process structured apply API is a conforming
second *ingress*, never a second *engine* — §4.1). The Admission gate
class additionally owns §11.2's producer-isolation rule, and the
**recovery owner** (the kernel where one is present; the host recovery
owner on the kernelless web path) MUST implement §11.1's machine —
classification, containment, and state preservation — for the
content-rejection set §11.1 names; the full recovery *transaction*
spans producer and presentation responsibilities per §11's per-class
rows (producer freeze/rebuild, resync, acknowledgement — the
2026-08-17 resync ticket's shape), with each side owning its own
steps — the recovery owner neither owns nor coordinates the producer
or presentation sides; it exposes its classification outcome and
state-preservation guarantee, which the per-class rows consume. The
class bullets alone, without the machine, do not conform.

### 2.2 Conformance suite

Conformance checks land in the single verification registry
(`exact-verify.json`) under RFC 0496's model; identity, active-codec,
entry-security, and transactionality checks are **non-quarantinable**
(RFC 0491 Week-0 ledger row schema). This spec's normative statements
are testable claims; the break package's golden fixtures (§15) are the
canonical instruments.

## 3. Terminology

- **EXWF frame revision** — the integer identifying the frame-format
  generation. Bootstraps at 1 with the Phase-2 break; bumps with every
  schema-package change, always in lockstep with the digest (§8.3).
- **Schema package** — the one complete generated artifact containing
  every schema a frame carries (§8.1). The unit the digest covers.
- **EXWF semantic epoch** — the domain-separated SHA-256 identity of
  the canonical **semantic package**: the identity-events enum and
  identity-carrier matrix, the recovery-class assignments, the
  breaking-window ledger rows, and the selected `BatchId`
  successor/initial/refusal/reconnect rules — the companion
  authorities the `exwfSchemaDigest` deliberately excludes (Status of
  This Document). Defined in §8.4. *(Amendment
  `A-0507-EXWF-SEMANTIC-EPOCH`, RFC 0491 §8 — ratified 2026-08-23.)*
- **Raw address** — the six-field node identity
  `(ProducerId, ExecutionGeneration, rootId, rootIncarnation,
  localViewId, nodeAllocationGeneration)` (§5.1).
- **Capability context** — the attach-issued identity-and-limits
  structure every command frame is fenced by (§7; event frames: §6.5,
  open issue 9).
- **Batch number** — the monotonic per-producer-per-root command-frame
  sequence (§7.3; event-frame carriage: §6.5, open issue 9).
- **Extension chunk** — a record in the single sidecar envelope (§6.3);
  **sidecar family** — a class of extension chunks keyed by the
  extension inventory (pager, motion snapshot, motion command, list
  model, menu, graphics publication, graphics teardown — the live
  families at r16).
- **Fence domain** — the scope within which an identity shorthand is
  safe (§5.7).
- **Restore** — re-presentation of an allocation that already happened
  (reconnect full snapshot, recovery adoption, same-incarnation-pair
  rebuild) (§5.4).
- **Producer session** — the admission-gate state for one
  `(ProducerId, ExecutionGeneration)`, owning the allocation-generation
  table (§5.5).
- **`localViewId`** — the canonical spelling in this spec for the one
  field LLP 0488 calls "local node id" and RFC 0491 spells
  "local ViewId". One field, three historical spellings; sibling
  documents SHOULD converge on `localViewId`.

## 4. Design invariants (normative summary)

1. **One commit engine, two conforming ingress forms.** All mutation
   commits through the **one transactional apply engine** (the command
   buffer). Its conforming ingress forms are (a) EXWF byte frames and
   (b) the **in-process Rust structured apply API** — RFC 0491's
   host-tier rlib surface, which shares the command-buffer commit path
   ("rlib is a primary surface" is not satisfied by a bytes-only
   shim). Direct-setter mutation surfaces are deleted (RFC 0491 WS-F:
   "mutation goes through the command buffer only"); a kernel exposing
   any mutation surface that bypasses the commit engine is
   non-conformant. Structured-apply operations are decoded operations:
   they enter validate-then-apply and the command-buffer commit engine
   exactly as decoded frame content does (RFC 0491's commit-path
   parity); frame-envelope admission (§9.2) applies to byte frames —
   the in-process ingress is fenced by its session context, not by
   envelope checks it has no bytes for.
2. **Validate-then-apply.** A frame is validated in its entirety before
   any of it is applied; rejection yields a typed outcome and leaves
   canonical state, derived state, indexes, and receipts untouched.
3. **Fail closed.** Every malformed, unknown, stale, or exhausted
   condition in this spec names a refusal (§13). There are no silent
   fallbacks, silent truncations, or best-effort decodes.
4. **Identity is namespaced, not merely fenced.** Two live
   producer/root pairs reusing a local id MUST NOT collide; a same-root
   destroy/recreate MUST NOT alias; two simultaneously live roots MUST
   never present equal addresses (§5).
5. **No total cross-owner order.** Upward records are ordered per
   delivery owner with causal links; no consumer may assume a global
   order across owners (§12.2).

## 5. The identity algebra

### 5.1 The raw address

Every carrier of node identity that crosses a stream boundary (§5.7)
MUST carry the full **raw address**:

| Field | Width | Minted by | Bumped by |
| --- | --- | --- | --- |
| `ProducerId` | 64-bit | protocol-admission owner, at first attach (§5.6) | never — reissued on re-attach with matching `ExecutionGeneration` |
| `ExecutionGeneration` | per LLP 0417 (wire width schema-package-owned) | producer runtime per LLP 0417 | producer restart; coherent full reload. **Never** by an LLP 0500 HotRevision plan patch |
| `rootId` | stable wire root id | producer | never (stable beside the incarnation pair) |
| `rootIncarnation` | 64-bit | admission owner, per root | root-scoped `reset()` (§7.4); whole-kernel reset bumps every root |
| `localViewId` | u32 (wire) | producer | n/a (namespace component) |
| `nodeAllocationGeneration` | 64-bit | producer session (§5.5) | per §5.4's allocation classes |

The address maps internally to a generational handle; EXNODE,
semantics, agent refs, receipts, and parent/child validation all bind
to it.

### 5.2 Counters never wrap

All identity counters (`ProducerId`, allocation generations,
incarnations, batch numbers) are 64-bit and MUST NOT wrap. On
exhaustion the implementation MUST refuse the operation fail-closed
(§13, E-9).

### 5.3 The closed identity-events enum

Identity transitions are a **closed enum**; the identity-carrier matrix
and the recovery-class table (§11) are generated from it (the break
package), never maintained as prose. The enum, with per-field
bump/hold:

| Event | `ExecutionGeneration` | `rootIncarnation` | `nodeAllocationGeneration` |
| --- | --- | --- | --- |
| `ProducerRestart` | **bump** | hold (re-fenced by generation) | scope ends — the superseded session's table retires (§5.5); the successor session mints per §5.4 |
| `CoherentReload` | **bump** | hold | scope ends — as `ProducerRestart` (§5.5) |
| `HotRevision` | **hold** (LLP 0500 D1's identity-stable edit path) | hold | hold |
| `RootReset` | hold | **bump** (that root) | scope ends — fresh key-space: generations mint per `(rootIncarnation, localViewId)` (§5.4) |
| `KernelReset` | hold | **bump** (every root) | scope ends — as `RootReset`, every root |
| `AllocateAfterDestroy` | hold | hold | **bump** (that `(rootIncarnation, localViewId)`) |
| `LiveNoop` | hold | hold | **hold** (idempotent create on a live same-type id) |
| `RestoreReconnect` | hold | hold | **hold** (reuse — §5.4(iii)) |
| `RecoveryAdopt` | hold | hold | **hold** (reuse — §5.4(iii)) |
| `RootMigration` | hold | hold (both roots) | **hold** (the allocation's generation moves with its binding) |

The generation column is meaningful only **within** a producer session
and root incarnation: RFC 0491 guarantees reuse for the restore class
(same incarnation pair) and decides table ownership at the session
(§5.5); it decides no cross-session or cross-incarnation carry.
Numeric seeding of a successor session's table is **not observable
through conforming carriers** — a ref carrying a superseded
`ExecutionGeneration` fails session-table validation (E-11) whatever
its generation value — so the scope-end cells assert retirement, never
a rekeyed carry.

The no-bump rows (`HotRevision`, `LiveNoop`, `RestoreReconnect`,
`RecoveryAdopt`, `RootMigration`) are explicit rows of this enum,
never inferences.

**`RootMigration`** atomically moves one still-live node or subtree
binding from a source `rootId` to a destination `rootId`.
`ProducerId`, `ExecutionGeneration`, both roots' incarnations, and
every moved allocation's `nodeAllocationGeneration` hold — it is
neither a destroy/recreate nor a pair of collateral root resets. The
producer-session table (§5.5) **tombstones the source binding and
rekeys the same live allocation under the destination address in one
transaction**; selector indexes, parent/child validation,
cross-stream carriers, and receipts move in the same transaction. An
already-live destination address, or any partial index/table move,
**refuses the whole migration** (E-18) — there is no partially
migrated state. After a committed migration every raw source address
fails closed (its `rootId` no longer names the allocation — E-11),
while logical keys may rebind per §5.8. v1 restricts `RootMigration`
to whole-subtree moves rooted at one node (arbitrary node-set
migration is not needed by any named consumer and complicates the
atomicity proof).
*(Decision provenance: RFC 0491 §8 `A-0507/0515-ROOT-MIGRATION`,
drafted in LLP 0491.001 §4, ratified 2026-08-23 by Charlie Cheever —
decision relayed via orchestration session exact-9e; activates the
tenth identity event that the generated break package carried as a
pre-activation obligation.)*

**`ProducerId` across new-generation events (selected).** A
`ProducerRestart` or `CoherentReload` attach presents a **new**
`ExecutionGeneration`, and the admission owner **mints a fresh
`ProducerId`** for it. Reissue remains the rule for exactly one case:
re-attach with a **matching** `ExecutionGeneration` (reconnect —
§5.6, unchanged). "Never bumped" therefore means: within one producer
session the value never changes, and no event *mutates* a live
`ProducerId`; a new generation is a new session with a new mint. All
other closed rows hold: `HotRevision`, `RootReset`, `KernelReset`
within the surviving session, `LiveNoop`, `RestoreReconnect`,
`RecoveryAdopt`, and `RootMigration` (landed in the same amendment
batch — see below) neither mint nor rotate it.
*(Decision provenance: RFC 0491 §8 `A-0507/0515-PRODUCER-SUCCESSOR-ID`,
drafted in LLP 0491.001 §2, ratified 2026-08-23 by Charlie Cheever —
decision relayed via orchestration session exact-9e; supersedes the
2026-08-20 seam-batch conservative interim, which remains the recorded
rule for any producer built before this amendment's regeneration
lands.)*

### 5.4 Allocation classes

The `nodeAllocationGeneration` is minted per
`(rootIncarnation, localViewId)` allocation with **three** classes:

- **(i) Allocate-after-destroy** — producer `DestroyView` then
  `CreateView` of the same local id in the same root incarnation:
  **bumps**. This is production behavior today
  (`create_view_with_id` rematerializes retired ids); the bump is what
  makes a stale pre-recreate ref detectably stale rather than
  hopefully absent.
- **(ii) Live no-op** — `create_view_with_id` on a live same-type id:
  **does not bump**.
- **(iii) Restore** — the reconnect full snapshot, the rejected-frame
  scratch-preflight adopt, and any same-incarnation-pair rebuild:
  **reuses** existing generations and MUST NOT bump. A restore
  `CreateView` re-presents an allocation that already happened.
  Without this class every recovery snapshot would restart every
  generation and stale refs would alias again; restore-as-no-bump is
  an acceptance criterion of the stale-handle guarantee, not an
  optimization.

### 5.5 Ownership of the allocation-generation table

The table is owned by the **producer session** — keyed
`(ProducerId, ExecutionGeneration, rootId, rootIncarnation,
localViewId)` — and hydrates any scratch kernel that a recovery or
reconnect snapshot targets. It MUST NOT be a field of a kernel object
that scratch-replace destroys (that ownership is what makes §5.4(iii)
implementable). A superseded producer session's table is retired with
its session: a raw ref carrying a superseded `ExecutionGeneration`
fails session-table validation (E-11) — kernel **address** validation
fails closed on a generation mismatch even though Acto **pin**
invalidation keys on LLP 0374's `producerIncarnationId` (§5.6; the
agent-surface spec states the pin rule).

### 5.6 Two producer identities; one is a kernel field

`ProducerId` is minted by the **protocol-admission owner** at first
attach — the kernel where one is present; the TS web admission owner on
the kernelless web host. Re-attach with a matching `ExecutionGeneration`
MUST reissue the previous `ProducerId` (reconnect is not a fresh mint;
attach-mints-always would rotate every raw ref on reconnect).

LLP 0374's crypto-fresh `producerIncarnationId` is the **Acto authority
identity**: it joins at the Acto boundary on agent refs and receipts
and MUST NOT appear as a component of the kernel node key or the
capability context.

`ExecutionGeneration` is a **separate context field beside
`ProducerId`**, never "a component of producer incarnation."

### 5.7 Fence domains

Identity shorthands are safe only inside their fence domain:

- **Within one producer's own op stream**: connection FIFO ordering
  plus absent-id rejection make bare-`localViewId` op targets safe — a
  producer cannot race its own destroy. Per-op wire traffic therefore
  does **not** carry the allocation generation; `CreateView` grows no
  field.
- **Across the stream boundary** — Acto refs, receipts,
  motion/graphics claims, host callbacks, LLP 0488 report and
  subscription keys, selection/handler/FFI carriers — every reference
  MUST carry the full raw address and MUST validate against the
  producer-session table. A bare-u32 cross-stream carrier is
  non-conformant. A sibling agent-surface spec that defines node
  references (e.g. LLP 0511's `(snapshotId, ref)` pairs, whose opaque
  `ref` internally carries a `node` component) MUST define that node
  component as this full raw address — snapshot fencing composes
  with, and never substitutes for, the address (cluster obligation; a
  bare-`viewId` agent target would reopen the aliasing hole WS-A
  closes).

### 5.8 Logical keys are a separate identity

Logical rebinding keys (`testId`, producer-named ids) **rebind** across
HMR and root migration; they are how agent scripts survive reloads.
They MUST NOT masquerade as raw addresses, and raw refs MUST fail
closed on `reset()`, incarnation bump, and id reuse (RFC 0491 WS-A's
three closed Acto rules).

### 5.9 The identity-carrier matrix

The exhaustive matrix — every carrier of node identity (handler ids,
event payloads, presenter resource references, motion/graphics
carriers, selection wire identities, FFI holders, 0488 report keys,
Acto refs), with the fields it carries, who mints them, and what bumps
them — is a **generated break-package artifact** (RFC 0491 Week 0),
derived from §5.3's enum. A carrier absent from the matrix is a red;
"fenced by other means" is a recorded matrix row, never an assumption.

## 6. The frame

### 6.1 Framing and alignment

- Ops use an **8-byte aligned op header**. Alignment makes typed slice
  casts *possible*; a consumer MUST additionally check the staging
  buffer's base alignment and fall back to an unaligned copy, under
  explicit POD and endianness rules carried in the schema package.
- The frame carries a checked **`frame_len`**. There is **no trusted
  op count**: the unvalidated `op_count` field is removed; a consumer
  MUST derive op iteration from validated lengths.
- Byte-level field offsets, discriminant values, and table layouts are
  **generator-owned**: they are defined by the schema package the
  digest pins (§8), not restated here. This spec constrains their
  *rules* (alignment, boundedness, closedness); the package is the
  byte-layout authority. Concrete layout tables in prose that drift
  from the package would be a second truth — the exact failure the
  digest exists to prevent.

### 6.2 Typed values

- Prop values are **typed on the wire** (the declared value type per
  prop id from the WS-B tables — booleans as booleans, never
  `"true"`). The ~50 stringified scalar forms are not representable in
  revision 1.
- `SetStyle` is a **masked patch**; `ClearStyle` clears. Full-
  replacement style ops and the legacy patch opcodes are not part of
  revision 1's closed op list.
- The generated style mask width is derived and MAY be multi-word.
- `Percent` rides the wire as **authored percentage points (0–100)**;
  the fractional conversion Taffy needs happens exactly once, in the
  generated Taffy mapping. `Auto` is admitted **per generated style
  row** (only rows whose CSS grammar admits `auto` carry the
  discriminant); `Auto` on a non-admitting row is a typed decode
  rejection (§13, E-5).
- `PropValue::Handle`-class resource references are **not** in
  revision 1's closed list; resource references arrive later via the
  extension envelope. **[Open issue: RFC 0491 OQ8 —
  `token-ref u16` discriminant and dependency-domain bits are
  *reserved* in the value encoding; adoption is a separate program.]**

### 6.3 The extension-chunk envelope

One envelope replaces the bespoke sidecar framings (RFC 0491's "six
bespoke sidecar framings", spanning the seven live sidecar ops): **one
reassembler, one FNV checksum, one reader** (RFC 0491 WS-D's decided
wording — the checksum family is FNV; the exact variant and width are
pinned by the schema package. Of the shipped families, the three that
checksum reassembled payloads today — motion snapshot, list model,
graphics publication — use FNV-1a-64; the others carry no payload
digest today, which is exactly what the unified envelope ends). A
checksum or reassembly failure is a
typed refusal (§13, E-16). Rules:

- Every extension chunk names its **sidecar family** from the
  extension inventory; the family determines its recovery class (§11).
  The closed family roster is a **schema-package artifact derived from
  the protocol inventory's opcode rows** — today's inventory records
  ordinary opcode rows without a family classifier, so the package
  owns that classifier (a recorded generation obligation), and this
  spec never maintains the roster as prose.
- A zero-opcode path exists for future **commit-rate** publications.
  Per-frame GPU data is main-owned SharedValue sampling and MUST NOT
  ride kernel opcodes.
- The MSCH schema handshake moves to **runtime attach**; there is no
  per-frame schema envelope or per-frame re-validation.

### 6.4 `DestroyView` semantics

`DestroyView`'s wire semantics are **subtree destroy** as of the EXWF
revision (the semantic correction RFC 0491 WS-I decides). The ordering
constraint is normative for the transition: until the EXWF revision
activates, the legacy wire meaning stays single-node, and recursive
destruction exists only as internal arena reclamation — a wire-meaning
flip under an unchanged frame revision is forbidden.

### 6.5 Event frames

Inbound events are **binary frames against per-event payload-family
schemas** (RFC 0491 WS-C): the inventory grows a payload-family schema
per event — discriminants for families sharing an event ID, bounds,
versioning, and unknown-payload handling — and generated
producer/consumer parity over those schemas **gates the Phase-2
break**. The payload-family schemas are REQUIRED members of the schema
package (§8.1). Refusals partition per §13: an **unknown** payload
family or discriminant is E-3; a **known** family violating a declared
bound or constraint is E-17 — never a silent skip.

Event frames are a **distinct frame kind** from command frames. RFC
0491 decides their schemas and parity, not their transport shape: the
event frame's outer framing and its session/batch relationship are
**schema-package-owned**, and its admission *ownership* is a
**break-package ledger/census row** — owners live in the ledger, not
the digest-covered schema package (open issue 9 covers all three).
The §11.1 exclusion itself is **decided, not interim**: RFC 0491
decides no tree recovery for events, so an event decode failure never
breaks the producer's command stream. What is interim, pending the
package pins, is the handling shape: a refused event frame is
**refused and surfaced whole** (nothing applied, diagnostics per
§13).

### 6.6 The closed wire-visible input list (revision 1)

RFC 0491 Phase 2 decides the break's wire-visible inputs **as a
closed list**, all carried by revision 1's schema package and covered
by the digest (§8): the `overflowX`/`overflowY` split; grid track
lists under the portable track grammar (the 0486 layout-profile
amendment is a Phase-2 prerequisite; semantics stay 0486-owned);
logical-edge representation and direction-live box layout (the WS-I
generated truth table; semantics stay with the generator authority);
`Percent`/`Auto` per §6.2; the capability-context frame identity
(§7); RFC 0493's typed input props and event-payload decisions
(freeze-window riders); and WS-B's reserved `token-ref` and
dependency-domain discriminants (§6.2's open issue). This spec pins
that these representations are **in** revision 1 and digest-covered;
their semantics remain with their named owners.

## 7. The capability context

### 7.1 Issuance and contents

At attach, the admission gate issues a **capability context**:

```
{ ProducerId,              // §5.6; reissued on re-attach
  ExecutionGeneration,     // separate field, never nested
  rootIncarnations,        // per root (WS-A: "per-root root incarnation")
  exwfFrameRevision,       // §8.3
  exwfSchemaDigest,        // §8
  exwfSemanticEpoch,       // §8.4 (amendment A-0507-EXWF-SEMANTIC-EPOCH)
  limits }                 // admission limits (bounds, budgets)
```

`exwfSemanticEpoch` is **never an alias of, input to, or derivable
from** `exwfSchemaDigest`; the two move independently (a ledger or
recovery change moves the epoch under an unchanged schema digest; a
wire-schema change moves the digest and only moves the epoch if a
companion file also changed).

Every **command** frame is fenced by this context plus a monotonic
per-root **batch number**, and receipts echo the batch number with
the commit generation (event frames: §6.5, open issue 9). The node-allocation generation is **not** a frame-header
field (it rides refs and receipts, §5.7); LLP 0374's
`producerIncarnationId` never appears here (§5.6).

**Attach-only vs frame-carried (this spec's attribution rule):**
RFC 0491 decides that the context is **attach-issued**, that the
per-root batch number is **carried by every frame** (a sentence 0491
states for the command stream; the event frame's session/batch
relationship is unpinned — §6.5, open issue 9), and that the MSCH
handshake happens at attach with no per-frame schema envelope. This
spec's rule is stated at the attribution level: the gate MUST be able
to associate every **command** frame with exactly one issued context
(its session) and one target root (event-frame attribution: §6.5,
open issue 9), and every command frame carries the batch
number, the checked `frame_len`, the op stream, and the extension
envelope (event-frame framing: §6.5, open issue 9).
**How** session and root attribution ride — frame-carried
correlation/`rootId` fields, connection-bound association, or a mix —
is **schema-package-owned**: frame-carried fields and connection-bound
attribution are both source-compatible encodings, and this paragraph
mandates neither.
**[Open issue: context physical topology and attribution carriage.]**
The context carries one incarnation **per owned root**; whether the
gate issues one context per root or one context bearing a
root→incarnation map, and whether session/root attribution is
frame-carried or connection-bound, are schema-package-owned. The
encodings MUST be observationally equivalent under §7.4's per-root
re-attach — a multi-root producer's root-scoped reset MUST NOT
disturb its other roots' fencing either way (V-26).

### 7.2 One structure, four jobs

The context answers stale-frame fencing across `reset()`, reconnect
correlation, Acto attribution, and future per-app budget enforcement
(RFC 0491 OQ11). Fencing is **not** recovery — recovery is §11.

### 7.3 Batch numbers

Batch numbers are monotonic **per producer per root** — LLP 0331's
producer-identified sequences and the web host's gap check compose
instead of colliding. A gap or regression is an admission refusal
(§9.2).

**Batch-number successor semantics (selected).**
`successorKey = (ProducerId, ExecutionGeneration, rootId,
rootIncarnation)` — one monotonic stream per key. `initialValue = 1`.
`refusedFrameConsumesNumber = false`: the gate's expected-next value
advances only when a frame **commits**; a refused frame's number is
re-presented by the producer's next attempt, and a producer that
skips past a refused number creates a gap, which refuses (§9.2,
E-10). `reconnectPersistence = true` **only** for a matching
successor key: a same-key reconnect (T4) resumes at the committed
high-water + 1; `ProducerRestart`, `CoherentReload`, `RootReset`,
and any root-incarnation replacement start a fresh stream at 1 under
their new key. Same-key reconnect resume is **mandatory** (MUST — a
"MAY restart at 1" would be a regression under an unchanged key and
refuse). This selection matches the shipped native-web gate
(`packages/exact-native-web/src/protocol-dom-host.ts`
`expectedSequence`: initial 1, exact-next, increment after commit),
which becomes the reference behavior rather than mere evidence; its
convergence disposition (§9.2's migration note) is discharged.
*(Decision provenance: RFC 0491 §8 `A-0507/0515-BATCH-SUCCESSOR`,
drafted in LLP 0491.001 §3, ratified 2026-08-23 by Charlie Cheever —
decision relayed via orchestration session exact-9e; supersedes the
2026-08-20 seam-batch session-scoped interim.)*

### 7.4 Root-scoped reset lowering

There is **no Reset opcode** (revision 1's wire-visible op list is
closed). A root-scoped reset is the producer **re-attaching that root
with a bumped `rootIncarnation`** in the capability context. Whole-
kernel reset is a host lifecycle operation that bumps every root the
same way. Reconnect presents the address components unchanged
("same incarnation pair") and receives a full snapshot, replayed under
§5.4(iii)'s restore class against the session-owned table.

## 8. The schema digest

### 8.1 Coverage

`exwfSchemaDigest` is computed over the **one complete generated schema
package**, closed over every schema the frame carries:

- the frame header and capability-context schema themselves;
- receipt and acknowledgement headers;
- op headers and typed prop tables;
- style layouts and style enums;
- extension-chunk envelope schemas;
- event payload-family schemas;
- patch/clear rules, bounds, alignment, and POD/endianness rules.

Component digests (the opcode table, event payload families) are
enumerated *inside* the package; **admission checks the package
digest**.

### 8.2 Preimage recipe

- **Hash:** SHA-256, **domain-separated** (the domain-separation tag
  is part of the generated canonicalization, pinned by golden
  fixtures).
- **Canonical byte projection:** defined by the generator — one
  serialization; a second canonicalizer is non-conformant.
- **No self-reference:** digest-bearing fields are excluded from their
  own preimage.
- **External authorities are digest-pinned:** externally referenced
  authorities that carry decoder semantics — the LLP 0486
  layout-profile `defaultRef` targets and the registered non-layout
  default authority (boundary `exwf-non-layout-defaults`, authority
  `tests/protocol/non-layout-defaults.json`, registered 2026-08-26) —
  enter the preimage as digests, so a default
  change cannot move under an unchanged schema digest.
- **Golden fixtures** pin the canonicalization (§15).

This digest **is** the capability context's schema digest — one digest,
two carriers, never two spellings. The `exwf` prefix is deliberate:
no field of this spec ever sits ambiguously beside LLP 0485's
`opcodeTableDigest` (the flat-plan expression VM's identity) or the
App-ABI `protocolVersion` (the JCS capability-call protocol's
identity); **neither of those is overloaded to carry EXWF identity**.

### 8.3 Revision/digest lockstep

`exwfFrameRevision` bootstraps at **1** with the Phase-2 break and
bumps with **every** schema-package change. The revision and digest
**always move together** — they are one carrier in two fields. A
revision bump without a digest change, or a digest change without a
revision bump, is an **admission error** (§13, E-7).

### 8.4 The semantic-epoch preimage

*(Amendment `A-0507-EXWF-SEMANTIC-EPOCH`, RFC 0491 §8 — drafted in
LLP 0491.001 §5, ratified 2026-08-23 by Charlie Cheever; decision
relayed via orchestration session exact-9e.)*

Same canonicalization discipline as §8.2: SHA-256, domain-separated
(proposed tag `EXACT.EXWF.SEMANTIC-PACKAGE.SHA256.V1\0`), one
generator-defined canonical byte projection over the semantic
package, no self-reference, golden preimage/epoch fixtures pinning
the canonicalization. The semantic package enumerates its member
artifacts by path and content digest inside the preimage.

The semantic-package member list (ratified with the amendment): the
identity-events enum and identity-carrier matrix, the recovery-class
assignments, the breaking-window ledger rows, the selected
`BatchId` successor/initial/refusal/reconnect rules, and the
SemanticFrame IR schema (RFC 0491 WS-H,
`tests/protocol/semanticframe/v1/schema.json`). *(SemanticFrame
membership added by amendment 2026-08-25 — ruled by Charlie Cheever
2026-08-25, decision relayed via orchestration session exact-9e;
mechanical record by agent under the standing semantic-decisions
rule. This supersedes the sf-2 landing's companion-consumer
registration (@6c6e75e72): SemanticFrame moves from digest-pinned
consumer to true member, so a SemanticFrame schema change now flips
the semantic epoch — and, because the epoch rides attach and
admission, flips attach compatibility. The frozen preimage rules
below — canonicalization, domain tag, no self-reference,
enumeration by path and content digest — are unchanged; only the
enumerated membership grows.)* The consumer
census stays **outside** it — census rows are provenance, not
semantics. The epoch rides **attach and admission only** (§7.1,
§10.1); it does not ride receipts — receipts stay lean.

## 9. Admission

### 9.1 Attach

A conforming gate admits a producer by issuing the capability context
(§7.1) after verifying the producer's declared
`{exwfFrameRevision, exwfSchemaDigest}` against the gate's schema
package under §8.3. The MSCH handshake happens here, once.

### 9.2 Per-frame checks

Per-frame checking has **two phases** preceded by outer-framing
decode, and the phase boundary is the §11.1 partition line.
**Header decode precedes both phases**: a frame whose outer framing
or header cannot be decoded — truncation before the frame-carried
header fields the attribution mode requires (§7.1; at minimum the
batch number on a command frame — under connection-bound attribution
the session/root fields are association state, not frame bytes) can
be read — refuses E-1 as a **simple refusal** (there is no admitted
content to recover); E-1 inside an admitted **command** frame's
content (body truncation past a decoded header) is a content
rejection in §11.1(ii) (event frames: §6.5's conservative interim,
outside §11.1). The two phases:

- **Pre-content admission checks** (failures are simple refusals,
  never recovery): the capability context matches a live session, and
  the frame's **attributed target root** (§7.1 — frame-carried or
  connection-bound) has a live incarnation in that session (both
  E-14); the batch number is monotonic per producer per root within
  that live session (E-10; §7.3 — the exact successor rule is the
  §7.3 open issue; gap and regression both refuse).
- **Content validation** (failures on an admitted session are content
  rejections and enter §11.1 — command frames; event-frame refusals
  follow §6.5, and independent-family E-16 units follow §11.3):
  `frame_len` consistency (E-4) and full validation before apply
  (§4.2).

Checks run in this order, and a refusal reports the **first failing
check** (§13's precedence rule). **Gap-rejection provenance:** the
break makes gap rejection a **universal** admission requirement. That
generalizes shipped behavior rather than inventing or merely
preserving it — the `exact-native-web` DOM host already enforces
exact-successor sequencing fail-closed
(`protocol-dom-host.ts` `expectedSequence`), while other decode paths
do not gap-check today. The existing web path's successor rule IS the
selected rule (§7.3 as amended 2026-08-23 — the shipped host is the
reference behavior; the convergence/migration disposition is
discharged). (RFC 0491's "no production web protocol decoder
gap-checks it" sentence predates that shipped host and is recorded as
an owed 0491 erratum, not repeated here.)

### 9.3 Compatibility dimensions

Compatibility is **per-dimension epochs, never one flip**. EXWF
carries its own revision/digest dimension **and, independently,
`exwf-semantic-epoch`** (§8.4 — amendment
`A-0507-EXWF-SEMANTIC-EPOCH`); event schemas, EXNODE/feed,
the C ABI, rlib, and Motion each carry their own (RFC 0491 Phase 2–4
epochs). On refusal, attach/replay MUST report the **exact mismatched
dimension** (§13, E-8).

### 9.4 Refusal reporting

A **compatibility-mismatch** refusal (E-7, E-8 — the attach/replay
class RFC 0491's reporting rule covers) MUST name the exact
mismatched compatibility dimension. A semantic-epoch mismatch is a
compatibility-mismatch refusal of the E-8 class naming
`exwf-semantic-epoch` (§8.4). Every other refusal — admission
(gaps, dead sessions, handshake failures) and content rejection alike
(envelope units included: E-16 is content, not admission — §11.1) —
has no mismatched dimension to name and MUST instead identify its
failing §13 rule. Every refusal SHOULD additionally carry the expected and
presented values as its diagnostic minimum; the generated error enum
(RFC 0491 WS-E) pins the machine record shape at the break, and §13's
names are spec-local handles, not wire identifiers. Silent
non-admission is non-conformant.

## 10. Container and artifact admission

### 10.1 EXFF baked frames

The `{exwfFrameRevision, exwfSchemaDigest, exwfSemanticEpoch}` triple
(the third field per §8.4 — amendment `A-0507-EXWF-SEMANTIC-EPOCH`)
lands in EXFF baked-frame admission as **required fields under a
bumped EXFF container version**. The downgrade edge is closed by
construction: existing EXFF readers parse permissive headers and
would silently ignore unknown fields, so an old reader MUST refuse
the new container version (rather than skip the fence), and a new
reader MUST refuse a container that lacks the triple.

### 10.2 Plans and baked frames across revisions

A plan or baked frame produced against revision R is either
**regenerated** for R+1 or **rejected** at admission — never
best-effort decoded (the regeneration-or-rejection rule; the owed
LLP 0485/0307/0333 amendment rows ride RFC 0491's Phase-2 ledger).

### 10.3 Replay

Replay tapes are SemanticFrame IR (RFC 0491 WS-H), not raw EXWF bytes;
raw bytes cannot survive the very revisions the recorder must replay
across. Frame consumers used in replay MUST apply the same admission
rules as live consumers.

### 10.4 App-ABI coordination — one rule, two dimensions

The EXWF revision/digest is its own compatibility dimension. The
App-ABI schema — whose `operationProtocol` names the JCS
capability-call protocol, not EXWF — widens **only if its own schema
changes**, and any such widening is an explicit M5-STOP ruling. A
schema-*package* pin refresh caused by a regenerated encoder artifact
is a pin refresh, not an App-ABI widening. Adding the EXWF tuple to
Exact Native artifact admission is an explicit **compatibility-tuple
major bump** with no App-ABI widening.

## 11. Rejected-frame recovery

### 11.1 The recovery state machine

**The refusal partition** (exclusive per refusal *instance* — an
E-id alone does not pick the set: E-16 splits by the failed unit's
family coupling, E-1/E-3 by frame kind and locus; V-9/V-11/V-24
follow it):

- **(i) Simple admission/fencing refusals** — **header-locus E-1**
  (§9.2's header-decode rule: outer framing or required header fields
  unreadable), E-7, E-8, E-10, E-14,
  E-15 (§9.2's pre-content phase and attach), producer-side E-2, and
  the non-frame refusals E-9, E-11, E-12. Fencing is not recovery
  (RFC 0491 WS-D); none of these enter this machine.
- **(ii) Whole-frame content rejections** on an admitted live
  session — a **command frame** that passed the pre-content admission
  checks and failed content validation: **body-locus E-1** (past a
  decoded header), E-3 (command-frame
  decode), E-4, E-5, E-6, E-17, **and E-16 where the failed unit's
  family is tree-bound** (a tree-bound sidecar adopts atomically with
  the tree commit, so its unit failure rejects the frame per
  validate-then-apply). These are RFC 0491's "rejected frame" and
  follow this machine — the producer's incremental stream cannot
  continue coherently past them.
- **(iii) Sidecar-unit failures in independent families** — E-16
  where the failed unit's family declares `independent` coupling:
  the unit is dropped per its §11.3 class record (e.g. graphics
  retains the last complete scene) and the frame's tree content is
  unaffected; §11.3 governs, never this machine.

Event-frame refusals are simple refusals of the event frame (§6.5's
conservative interim), outside all three sets above.

A rejected frame in the recovery class
follows the acknowledged state machine: freeze
incrementals → rebuild the affected live roots → scratch-preflight →
adopt atomically (§5.4(iii)'s restore class — no generation bumps) →
preserve last-good pixels on failure → resume only after presentation
acknowledgement. The legacy `ProtocolTreeReset` nuclear path is
deleted only after this machine is green, never on capability identity
alone.

### 11.2 Producer isolation

Atomicity scope is **producer-isolated by default**: one producer's
rejected frame freezes and rebuilds *that producer's* roots; one
producer MUST NOT freeze or snapshot another producer's roots. A
cross-producer host recovery coordinator (with prepare/freeze
acknowledgements from every affected producer) is built only where a
shared host sidecar genuinely spans producers. **[Open issue: no such
coordinator is specified; isolation is the v1 rule.]**

### 11.3 Recovery classes

Every sidecar family declares its full recovery-class record with
RFC 0491's **two generated properties**: **`treeCoupling`**
(tree-bound — adopted atomically with the kernel tree commit — vs
independent) and **`failureDisposition`** (`rollback-to-last-good` vs
`drop` of the offending unit), plus the declared machinery: the
adoption owner and point, the rollback-or-generation-swap mechanism,
the resync rule, the presentation-ack outcome, and a REQUIRED
**retained-state note** describing what survives the disposition
(0491's own per-family notes: graphics' `drop` *retains the last
complete scene*; list-model rollback lands on *staged last-good*
chunks; menu rolls back with *no retained kernel menu state*). The
note is descriptive machinery, never a third schema property — the
generated table keeps 0491's two property names. Classes key on
**sidecar families from the extension inventory, never hosts**. The
generated
break-package table is the machine authority and carries the **full
record per family**; it is **total over the inventory's live sidecar
ops** — a row per op or an explicit deletion-before-activation row; a
claimed-complete matrix that omits a live family is a red. This
section's totality claim is scoped to **extension-inventory sidecar
families**; upward records follow §12's per-owner rules, never this
machine.

Assignments (RFC 0491 r16's **decided target classes** — an
informative summary; op ids stay inventory-owned and are cited here by
family name only). Where a shipped path diverges from its target class
today, the divergence is a **fail-open migration obligation proven red
by injection** until migrated — notably the list-model path, whose
shipped ingestion silently drops a failed revision (diagnostics
counters only; `SetListModelChunk` dispatch unconditionally succeeds)
while other tree mutations commit: that proves staged last-good
retention but **not** the tree coupling the target class declares.
The assignment stands as the target; the shipped silent-drop path is
exactly the fail-open class the break deletes:

| Family | `treeCoupling` | `failureDisposition` | Retained-state note |
| --- | --- | --- | --- |
| pager | tree-bound | rollback-to-last-good | applied and rejected inside the Apple commit transaction |
| motion snapshot | tree-bound | rollback-to-last-good | Apple commit transaction distinguishes tree-bound Motion from last-good Graphics |
| motion command | tree-bound | rollback-to-last-good | with motion snapshot |
| list model | tree-bound | rollback-to-last-good | onto staged last-good chunks (`SetListModelChunk` staging) |
| menu | tree-bound | rollback-to-last-good | no retained kernel menu state — trivial rollback does not make it independent |
| graphics publication | independent | drop | drop-whole *is* retain-last-complete-scene (LLP 0352) |
| graphics teardown | independent | drop | with graphics publication |
| gpu canvas attachment | tree-bound | rollback-to-last-good | rolls back to the last adopted attachment snapshot per canvas leaf (allocated 2026-08-25, LLP 0512 O-11; producer not yet live) |

Default class for a new family: **tree-bound + rollback**. RFC 0491's
opt-in sentence, quoted so the generated table cannot be derived two
ways: "**independent + last-good is opt-in per row**" — in the
two-property schema that opt-in encodes as `independent` coupling with
`drop` disposition **plus a REQUIRED retained-state note**, because
for such families drop-whole *is* retain-last-complete (the graphics
row is the model).

Assignments are **proven by injection, never by declaration**: the
failure-injection suite exercises each declared assignment at each
owner's adoption point, not only the kernel tree's.

### 11.4 The TUI ordering quarantine

The TUI host mutates host mirrors before kernel apply and can reject
tree-bound sidecars after the kernel tree has committed — an ordering
that violates §4.2. This is a **host repair with a known-red injection
row** (quarantined), never a class assignment or a standing exception;
declaring it a "coupling fact" would let the injection suite green the
broken order. (The TUI lane is additionally frozen for the breaking
window — LLP 0505 r3 row 9.)

## 12. The upward family

### 12.1 One encoding family, three delivery owners

Upward flows share a **framing/correlation design, never one literal
channel** (a shared channel would create the cross-thread surface the
threading contract forbids):

- **Kernel exports on the runtime thread**: LLP 0488 container-size
  feedback, Acto commit receipts (each applied frame yields an
  attributable changed-set plus generation), Aquifer write-outcome and
  cell-transition receipts (typed records in the envelope, under the
  non-knowledge rule — the kernel grows no cell types).
- **GPU completions**: `gpu-service → exact-motion` command ingress on
  main, never through the kernel.
- **Present receipts**: with the host clock (LLP 0354), sharing the
  frame-correlation id without passing through the kernel.

### 12.2 Ordering is a causal DAG

Each delivery owner stamps **one monotonic sequence covering every
record domain it delivers** (the kernel-exports owner carries 0488
reports, commit receipts, and Aquifer receipts under one sequence);
records carry **`causedBy` links** across domains. A consumer MUST
NOT assume a total cross-owner order. **Scope rule:** the sequencing
scope is the **delivery owner**; a record *domain* is a filter over
its owner's stream (LLP 0511's rule) and filtering MUST NOT renumber
— RFC 0495 §4's "per-domain monotonic sequence numbers" wording
enumerates the stampers, which are exactly the delivery owners, so it
denotes the owner stream read through a domain filter; there is no
third sequencing scope (the alignment of 0495's wording is a recorded
owed 0495 sync — 0507 is the sequence authority).
**What is decided vs owed:** gap *detectability* (a missing sequence
number is detectable, never silent) and `causedBy` causal linkage are
RFC 0495's decided requirements and bind here. Retention and
backpressure bounds are RFC 0495 **OQ1** — undecided; consumers MUST
NOT assume any particular bound (recorded obligation on the 0495
loop). Upward-record failure handling never enters §11's machine
(decided by that machine's §11.1 scope); the re-publication policy
for lost records is likewise OQ1 territory, recorded as an
obligation, not specified here.

### 12.3 Receipt identity

The batch/commit echo is defined over records **originating from an
applied batch**: such a receipt echoes its originating batch number
and commit generation (§7.1). An upward record with **no originating
batch** (e.g. an off-tick GPU present — LLP 0512 §5.4's conservative
reading, adopted here as the one rule) carries the attribution fields
**absent, never improvised**; per-owner sequencing and its `causedBy`
causal linkage still apply. Every upward record carries full raw
addresses for any node reference (§5.7). LLP 0488 report and
subscription keys are the full six-field address (0488 §3.8); LLP
0374's `producerIncarnationId` joins on Acto-boundary refs/receipts
only.

## 13. Error handling (total)

Every rule below is a **typed refusal**: the consumer or gate rejects
with the named class, applies nothing from the offending unit, and
surfaces the refusal to diagnostics. Silent variants of any row are
non-conformant. The ids and names below are **spec-local handles**
that bind this document's rules, fixtures, and reviews; the generated
error enum (RFC 0491 WS-E) is the machine authority for wire-visible
error identity at the break. Each row here maps to at least one enum
variant, and each **wire-visible** variant of that enum maps to
exactly one row (the enum's non-wire FFI/status variants are WS-E
territory outside this table). **Precedence** (this spec's
conformance-reporting rule): when one unit violates several rows, the
refusal reports the first failing check — at attach, in the **schema
package's declared attach-check order** (generator-owned, like the
decode walk; RFC 0491 does not decide this order, so pinning it here
would be new closure — the V-27 attach-order vector pins whatever the
package declares); per frame, in §9.2's phase order; within frame
content, in the **schema package's canonical decode walk** (the
generated validator's traversal order — generator-owned, pinned by
the V-27 overlap vectors, never an implementation's private choice).
Secondary violations MAY be listed as diagnostics. **E-7 vs E-8
partition:** E-7 is *tuple inconsistency against the lockstep law*
(a `{revision, digest}` pair that is itself illegal — the revision's
known digest differs, or one moved without the other); E-8 is a
*coherent* tuple (or other dimension epoch) that does not match the
gate's. **E-3 vs E-17 partition:** E-3 is an **unknown** schema
element; E-17 is a **known** element violating a declared constraint.

| Id | Condition | Refusal |
| --- | --- | --- |
| E-1 | Trailing, unconsumed, or truncated bytes against a validated structure (the `style_decoder.rs:472` class; truncated op payloads included) — split by locus per §9.2: **header-locus** (outer framing / required header fields unreadable) vs **body-locus** (past a decoded header) | header-locus: frame refused: `malformed-encoding` — a **simple admission refusal**, never an entry into §11 (partition set (i)); body-locus: frame rejected: `malformed-encoding` (partition set (ii)) |
| E-2 | Unknown enum value at encode time | **producer** hard error in every build — dev throws, production rejects the encode; never a fallback index |
| E-3 | Unknown op, discriminant, container form, prop id, or payload family at decode (§6.5's partition: *known*-element constraint violations are E-17). Unknown-prop note: the shipped decoders' silent unknown-prop skip is a silent fallback and is replaced by this refusal (§4.3); the inventory's reserved `u16::MAX` unknown-prop sentinel stays the forever-unknown conformance input — under EXWF it exercises this refusal instead of a skip (the package carries that fixture repurposing) | frame rejected: `unknown-schema-element` |
| E-4 | `frame_len` inconsistent with content; any frame-level trusted op count (the removed `op_count` class — bounded counts *inside* validated payload schemas are validated per schema, not this row) | frame rejected: `bad-framing` |
| E-5 | `Auto` on a non-admitting style row | typed decode rejection: `inadmissible-value` |
| E-6 | Absent target id in an op | op stream rejected per validate-then-apply: `absent-target` |
| E-7 | Revision/digest lockstep violation (§8.3) | admission error: `revision-digest-mismatch` |
| E-8 | Compatibility-dimension mismatch at attach/replay | admission refusal naming the exact dimension: `dimension-mismatch(<dim>)` |
| E-9 | Identity-counter exhaustion (§5.2) | operation refused: `counter-exhausted` |
| E-10 | Batch-number gap or regression (§7.3) | frame refused: `sequence-gap` — a **simple admission refusal**, never an entry into §11's recovery machine; a producer whose session sequencing is broken recovers by re-attach (§7.4, §9.1). §7.3 (as amended 2026-08-23) pins successor semantics: same-key reconnect resumes at the committed high-water + 1 — the matching recovery path — and every new successor key starts a fresh stream at 1 |
| E-11 | Cross-stream carrier without a full raw address, or address failing session-table validation (§5.7) | reference invalid: `stale-or-unqualified-ref` (fail closed per §5.8) |
| E-12 | EXFF container lacking the tuple (new reader) or carrying an unknown container version (old reader) (§10.1) | artifact refused: `container-version` |
| E-14 | Capability context matching no live session, or a frame whose attributed target root (§7.1) has no live incarnation in that session (§9.2 — the classification is identical under both of §7.1's topology encodings) | frame refused: `dead-session` |
| E-15 | MSCH schema-handshake failure at attach (§9.1) | attach refused: `schema-handshake-failure` |
| E-16 | Extension-envelope checksum or reassembly failure (§6.3) | envelope unit rejected: `envelope-integrity` |
| E-17 | A **known, well-framed schema element violating a package-declared constraint** — bounds, POD/endianness, non-finite value, non-zero reserved field, size limit, or declared structural invariant (e.g. a dependency cycle in a payload that declares acyclicity; `CreateView` on a live id with a **different node type**, which shipped code already rejects while preserving the original node — a declared structural invariant, not an identity event) — the shipped decoders' known-element failure class | frame rejected: `schema-constraint-violation` |
| E-18 | `RootMigration` refused: destination address already live, or the migration transaction cannot complete atomically (§5.3) | operation refused: `migration-refused` — nothing moved, source binding intact |

(The former E-13 — unregistered text measurer — is a host-services
rule, not a frame malformation: RFC 0491 WS-E's per-kernel
host-services object hard-errors at measure time, never a per-measure
log line. It binds the Frame consumer's host environment and is
recorded here as a pointer, outside the wire refusal table.)

## 14. Open issues (index)

Carried from RFC 0491 r16 as owner questions; none are decided here:

1. **OQ7 / freeze scope** — which surfaces freeze at window exit (the
   EXWF frame rev among them) is LLP 0506's D1, activating on the
   author's confirmation.
2. **OQ8 / token-ref** — the `token-ref u16` discriminant and
   dependency-domain bits are reserved (§6.2); adoption is a separate
   program (RFC 0497 F-B is the measurement driver).
3. **OQ11 / trust posture** — v1: one kernel per app process; the
   protocol is a correctness boundary, not yet a security boundary
   (§16). Adversarial-input guarantees are Expose-gated.
4. **Cross-producer recovery coordinator** — unspecified; producer
   isolation is the v1 rule (§11.2).
5. **EXPAINT / OQ5** — the export family header is designed for a
   paint-shaped sibling; EXNODE-only is RFC 0491's **recorded
   default, not a decision** (WS-C owns it; out of scope here).
6. **`ProducerId` across `ProducerRestart`** — RESOLVED 2026-08-23 by
   amendment `A-0507/0515-PRODUCER-SUCCESSOR-ID` (§5.3 as amended:
   fresh mint on new-generation events; reissue only on
   matching-generation reconnect).
7. **Batch-number successor semantics** — RESOLVED 2026-08-23 by
   amendment `A-0507/0515-BATCH-SUCCESSOR` (§7.3 as amended: successor
   key, initial value 1, refused frames consume nothing, same-key
   reconnect resumes).
8. **Context physical topology and attribution carriage** — §7.1's
   open issue: per-root context vs root→incarnation map, and
   frame-carried vs connection-bound session/root attribution;
   package-owned, observational equivalence required (V-26).
9. **Event-frame admission model** — §6.5's open issue, with §6.5's
   ownership split: the event frame's outer framing and session/batch
   relationship are **schema-package-owned**; its admission
   *ownership* is a **break-package ledger/census row** (owners never
   live in the digest-covered schema package); §6.5's conservative
   interim (refuse-and-surface, never §11.1) governs until pinned.

## 15. Test vectors

Concrete vector bytes are **generation obligations against the break
package and the registered corpus** — this document specifies vector
*requirements* and never fabricates bytes no encoder produced.
Recorded coverage debts beyond the table (delta-round obligations, not
new rows): vectors for §5.8's ladder, §10.2's replay admission, and
§10.4's one-rule-two-dimensions; deeper ClearStyle coverage beyond
V-7's round-trip; and the generated identity matrix stating the
session-crossing generation cells §5.3's intra-session bump/hold
column cannot (scope-end rows). The
golden set MUST include, at minimum, one vector per row:

| Vector | Exercises |
| --- | --- |
| V-1 | Canonical schema-package serialization → `exwfSchemaDigest` (the §8.2 preimage recipe, including domain separation and the excluded digest fields) |
| V-2 | Digest-pinned external authority: a `defaultRef` target change flips the package digest |
| V-3 | Attach handshake: context issuance; §8.3 lockstep violation (E-7) |
| V-4 | Batch-number gap and regression (E-10) |
| V-5 | Each §5.3 enum row, including the no-bump rows — reconnect full snapshot and recovery adoption MUST leave every allocation generation unchanged — **and the reconnect `ProducerId` reissue**: a matching-`ExecutionGeneration` re-attach reissues the previous `ProducerId` with existing refs retaining their intended identity (§5.6); **and the new-generation fresh mint** (§5.3 as amended 2026-08-23): a `ProducerRestart` re-attach observes a fresh `ProducerId`, a matching-generation reconnect observes reissue |
| V-6 | Allocate-after-destroy aliasing: a pre-recreate ref fails E-11 after the bump |
| V-7 | `SetStyle` masked patch + `ClearStyle` round-trip; multi-word masks |
| V-8 | `Percent` points-on-wire with the single Taffy conversion; `Auto` admitted row and E-5 rejection row |
| V-9 | Trailing-bytes rejection (E-1); unknown discriminant (E-3); bad `frame_len` (E-4) |
| V-10 | Extension envelope: one chunk per live sidecar family; reassembly; zero-opcode commit-rate path |
| V-11 | Recovery-class injection per §11.3 row, at the declared owner's adoption point (incl. the graphics last-good retention and the list-model staged chunks) |
| V-12 | EXFF container-version admission both directions (E-12) |
| V-13 | Upward records: per-owner sequences (one sequence spanning an owner's several domains; domain filters never renumber), consumer gap detection, `causedBy` cross-links, receipt echo of batch number + commit generation on batch-originated records only (§12.3 — batch-less upward records carry the fields absent) |
| V-14 | Subtree-destroy wire semantics under the EXWF revision; the pre-revision single-node meaning in legacy replay |
| V-15 | 64-bit counter exhaustion refusal (E-9) at the boundary value |
| V-16 | E-2 producer hard error in **both** build modes (dev throw; production encode rejection) |
| V-17 | E-6 absent-target refusal under validate-then-apply (nothing applied) |
| V-18 | E-8 per epoch dimension: one mismatch vector per Phase 2–4 dimension, each naming its dimension |
| V-19 | Base-alignment fallback: aligned-cast and unaligned-copy decode paths byte-equivalent on the same frame |
| V-20 | One-write-path: forbidden-symbol/closure obligation — no direct-setter mutation surface reachable (§4.1) |
| V-21 | Replay parity: a replayed consumer applies the same §9 admission rules as live (§10.3) |
| V-22 | Producer isolation: one producer's rejected frame does not freeze or snapshot another's roots (§11.2) |
| V-23 | Within-stream bare-`localViewId` accepted; the same shorthand crossing the stream boundary refused (E-11) |
| V-24 | E-14 dead-session, E-15 handshake-failure, and E-16 envelope-integrity refusals |
| V-25 | E-17 schema-constraint refusals: one vector per constraint kind the package declares (bounds, non-finite, reserved-field, size limit, structural invariant), each a known well-framed element |
| V-26 | §7.1's context-topology observational equivalence: a multi-root producer's root-scoped reset leaves its other roots' fencing undisturbed under the package's chosen encoding, and a frame whose attributed target root has no live incarnation refuses E-14 identically under both encodings |
| V-27 | Overlap/precedence vectors: units violating several §13 rows report the first failing check per the precedence rule — at minimum one attach-order vector (E-15 vs E-7 vs E-8), one E-7-vs-E-8 tuple vector, one E-3-vs-E-17 known/unknown pair, and one multi-violation content frame pinned to the package's canonical decode walk |
| V-28 | `RootMigration` (§5.3, amendment `A-0507/0515-ROOT-MIGRATION`): a committed migration → a stale raw source address fails E-11; a destination-collision migration refuses E-18 with nothing moved; a logical-key rebind across the migration is observed (§5.8) |
| V-29 | The semantic epoch (§8.4, amendment `A-0507-EXWF-SEMANTIC-EPOCH`): a companion-file-only change flips `exwfSemanticEpoch` and not `exwfSchemaDigest`; attach and EXFF admission each report `exwf-semantic-epoch` on an epoch-only mismatch (the E-8 dimension row — extends V-18's per-dimension set) |

Vectors land as break-package golden fixtures; the registered checks
that consume them are non-quarantinable where §2.2 requires. Until
the break package exists, this table is the recorded obligation set
(§ Status's conformance precondition); no vector may be claimed
satisfied by anything but a registered check over generated bytes.

## 16. Security considerations

Trust posture v1 (RFC 0491 §2, OQ11's default): one kernel per app
process; EXWF is a **correctness** boundary. Nothing in this spec
claims adversarial-input hardening; differential parser fuzzing is
best-effort defense in depth. Before any Expose milestone ships
third-party-adjacent surfaces on this kernel, the trust model MUST be
restated (kernel-per-app isolation, per-app budgets via the capability
context's `limits`, fuzzing graduation) — the capability context is
designed so budget enforcement is an additive change, not a wire
migration. Receipt confidentiality and capability folding are
RFC 0499 / LLP 0333-v2 territory, out of scope here.

## 17. Coverage map (informative)

| RFC 0491 r16 source | This spec |
| --- | --- |
| WS-A identity (six-field address, three allocation classes, session table, carrier matrix, Acto rules) | §5 |
| WS-A fence domains | §5.7 |
| WS-D frame contents (alignment, typed values, patch/clear, envelope, MSCH-at-attach, deletions) | §6 |
| WS-I `DestroyView` ordering, `Percent`/`Auto` | §6.4, §6.2 |
| WS-D capability context + batch numbers + reset lowering | §7 |
| Phase-2 digest carrier (preimage, lockstep, EXFF, 0485/App-ABI non-overload) | §8, §10.1–10.2 |
| WS-D gap rejection (generalizing the shipped web exact-successor behavior — §9.2's corrected statement) | §9.2 |
| Phase-2 per-dimension epochs | §9.3 |
| WS-D App-ABI one-rule-two-dimensions | §10.4 |
| WS-D recovery state machine, producer isolation, recovery classes, TUI quarantine | §11 |
| WS-C one-family-three-owners, causal DAG, Aquifer non-knowledge rule | §12 |
| Week-0 loud-failure rules, WS-B hard-error enums, error conventions | §13 |
| OQ7/OQ8/OQ11 + coordinator + EXPAINT | §14 |
| Break-package golden fixtures | §15 |
| §2 trust posture | §16 |

## Revision history

- **r4 (2026-08-20)** — extension freeze-fold (author-authorized;
  scope frozen at r3's normative surface — corrections and
  restorations only; every folded claim byte-verified against RFC 0491
  r16 and shipped code; **zero refuted**, including one place where
  the shipped tree contradicts 0491's own prose). Restorations: §4.1
  restored to 0491's one-commit-engine rule with **two conforming
  ingress forms** (EXWF frames + the in-process structured apply API
  sharing the command-buffer commit path — bytes-only ingress was an
  over-narrowing); §5.3's session-crossing cells corrected from
  "table re-keyed" to **n/a-retirement** (no cross-session carry is
  decided; numeric seeding is unobservable through conforming
  carriers); §7.1 softened to an **attribution rule** (frame-carried
  vs connection-bound carriage is package-owned — open issue 8
  widened); the §11.3 header re-labeled **decided target classes**
  with the shipped list-model silent-drop divergence named as a
  fail-open migration obligation; 0491's opt-in sentence quoted
  verbatim with its two-property mapping; §12.2's 0495 delegation
  made honest (gap detectability decided; retention/backpressure/
  republication = 0495 OQ1 obligations) and the stale 0510
  per-domain gloss removed (the wording's source is RFC 0495 §4;
  owed 0495 sync recorded); §9.2's gap-rejection provenance corrected
  — the shipped `exact-native-web` DOM host already enforces
  exact-successor sequencing, so the break *generalizes* gap
  rejection (0491's "no production web decoder gap-checks" sentence
  recorded as an owed erratum, not repeated). Partition repairs
  (corrections of r3's own machinery): §9.2 split into pre-content
  admission vs content validation phases; §11.1 rewritten as an
  **exclusive three-set refusal partition** (simple refusals /
  whole-frame content rejections / independent-family sidecar-unit
  drops, with tree-bound E-16 folding into whole-frame rejection by
  entailment); §6.5's E-3/E-17 partition and event-frame
  refuse-and-surface interim (event frames never enter §11.1; their
  admission model = new open issue 9); §13 precedence completed
  (attach order E-15→E-7→E-8; the E-7/E-8 tuple-vs-epoch partition;
  decode order = the package's canonical validator walk, pinned by
  new V-27 overlap vectors); unknown-prop decode pinned to E-3 with
  the inventory's `u16::MAX` sentinel repurposed to exercise the
  refusal (the shipped silent skip is the fallback class §4.3
  forbids); different-type live `CreateView` classified under E-17
  (shipped rejection behavior); E-14 extended to rootId-without-live-
  incarnation identically under both topologies; §2.1's gate bullet
  scoped to E-7/E-8 and the §11.1 machine bound to a recovery owner;
  V-5 gains the reconnect `ProducerId`-reissue proof; the sidecar
  family roster stated as a package artifact derived from inventory
  rows. Routed cross-spec corrections: §12.3's batch/commit echo
  scoped to batch-originated records with attribution-fields-absent
  for batch-less upward records (one rule with LLP 0512 §5.4,
  verified against its r4 bytes; discharges the echo half of 0512's
  O-12 ask — the off-tick causal-target vocabulary remains O-12's
  outstanding half on 0512's side, per its r5).
  Cross-doc obligations recorded, not folded: 0511's bare-`viewId`
  act arm and `(snapshotId, node)` spelling (0511-side); the 0491
  gap-check erratum; the 0495 per-domain-wording and OQ1 syncs.
- **r3 (2026-08-20)** — sr-specs round-2 fold (grok READY with minors;
  codex NOT READY ×9 — every claim byte-verified against RFC 0491 r16
  before folding; zero refuted). Fidelity restorations: the
  recovery-class record returned to 0491's **two generated
  properties** (`treeCoupling`, `failureDisposition ∈
  {rollback-to-last-good, drop}`) with retained-state as a REQUIRED
  descriptive note, never a third schema property, and the default
  restored to 0491's exact "tree-bound + rollback"; §7.1's context
  sketch corrected to **per-root incarnations** with the physical
  topology (per-root context vs root-map) marked package-owned under
  an observational-equivalence rule (open issue 8, V-26); the
  "closed split, RFC 0491 WS-D" attribution dropped — the
  attach-vs-frame split is this spec's encoding rule implementing
  0491's attach-issued context + frame-carried batch number; §9.4's
  dimension-naming duty scoped to compatibility-mismatch refusals
  (E-7/E-8), all other refusals name their §13 rule. Totality
  closures: **E-17** (known well-framed element violating
  package-declared constraints — the shipped decoders' known-element
  failure class) + V-25; §11.1 now defines which refusals enter the
  recovery machine (content rejections on an admitted session) vs
  simple refusals; the §13 enum bijection scoped to the wire-visible
  variant subset; §12.2's scope rule corrected — sequencing is
  per-owner, a domain is a filter that never renumbers (one owner may
  carry several domains), with an Upward-record **consumer** class
  added; E-10's re-attach promise made conditional on §7.3's open
  issue; `OpHeader` added to the Status replacement list; the FNV
  present-tense claim narrowed to the three checksummed families;
  0511's ref spelling corrected to `(snapshotId, ref)`; §5.5 states
  session-table retirement (superseded-generation refs fail E-11
  while Acto pins key on `producerIncarnationId`). Growth: +~10% over
  r2 (~+40% cumulative over r1, over the loop budget — flagged;
  overage is reviewer-demanded totality, not drift).
- **r2 (2026-08-20)** — sr-specs super-refine round-1 fold (codex
  gpt-5.6-sol@ultra + grok-4.6@xhigh, both NOT READY; every folded
  claim byte-verified against RFC 0491 r16). Fidelity restorations:
  digest scoped to the **schema package** (the break package is the
  containing artifact, never the preimage); the extension envelope's
  checksum restored to 0491's **FNV** decision; `ExecutionGeneration`
  wire width returned to schema-package ownership (not in 0491's
  64-bit list); E-4 narrowed to the frame-level `op_count` class;
  E-10 made a simple admission refusal (fencing is not recovery);
  §9.2's spec-local successor key removed in favor of 0491's exact
  sentence plus a recorded open issue; §9.4 reduced to 0491's decided
  dimension-reporting rule with the rest as diagnostic minimum.
  Additions demanded by totality/coverage: §6.5 event payload-family
  frames; §6.6 the closed Phase-2 wire-visible input list; the
  attach-vs-frame closed split; E-14/E-15/E-16 and the refusal
  precedence rule; the recovery record split into
  `failureAction`×`retainedState` (shipped graphics drop-and-retain
  vs list rollback-staged proved the axes independent) with the
  in-spec table demoted to an informative summary keyed by family
  name (op ids stay inventory-owned); upward-record producers split
  into their own conformance class under §12/RFC 0495 rules;
  Schema-package producer and EXFF reader classes added; V-16..V-24;
  the inventory header-row replacement stated; the 0511 raw-address
  cluster obligation and the per-owner/per-domain scope equivalence
  stated; former E-13 moved out of the wire table as a host-services
  pointer; provenance corrected (spec-wave directive, not an 0505
  docket row); §5.3/§14 gain the `ProducerRestart` `ProducerId` open
  issue. Growth: +~27% over r1 — **over the ~20% loop budget,
  flagged**: the overage is reviewer-demanded totality content (three
  new refusal rows, two conformance classes, nine vectors, two open
  issues); the author may trim the informative recovery summary or
  V-table prose for headroom.
- **r1 (2026-08-20)** — initial extraction from RFC 0491 r16 in the
  author-directed 2026-08-20 spec wave. Restates; decides nothing.
