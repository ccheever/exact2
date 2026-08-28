# RFC 0491: Kernel Refresh Program — the breaking-window rebuild of exact-kernel

**Type:** RFC
**Status:** Accepted
**Systems:** kernel, protocol, FFI, Apple Host, Windows Host, Android, Web, Motion, Text, Selection, Semantics, Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-20
**Revised:** 2026-08-26 (WS-B's 88-bit style-order fixtures LANDED as the registered `kernel-style-order-fixtures` gate — the fourth and last of WS-B's named interim non-mutating W0-A gates, so remaining W0-A debt before Phase 1 entry is the three pinned tape apps alone; a phase-status note records the derivation rule, the four planted-defect runs, and three facts it pins that nothing pinned before — the live wire order is decoder declaration order rather than ascending style bit (3 violations recorded, and the EXWF schema package's styleLayouts.wireOrder declares the ascending-bit post-break order), six of 88 bits have no TypeScript producer, and the web host decodes 72 of 88 and fail-closed-refuses the rest. No phase gate moves, no ledger row claimed satisfied, no design change.) 2026-08-26 (W0-A's last two deferred artifacts landed — the canonical consumer census and the exact-once census↔ledger join, plus the Phase-2 breaking-window ledger rows they join against, generated as `tests/protocol/exwf/consumer-census.json` and `tests/protocol/exwf/breaking-window-ledger.json` by the EXWF break-package generator (LLP 0506 D1(a)). The eight ledger rows project this section's decided in-rev list and are ajv-validated against the W0-A row schema; their non-waivable set is the four classes this table names, and `coreMembershipClass` is derived from the ratified `A-0496-CORE-MEMBERSHIP` seed rule rather than authored. The census promotion's discovered coverage join found the opcode-118 virtual-topology-certificate family uncensused and it was added. Pre-activation only: no ledger row is claimed satisfied, no consumer is claimed migrated, and no phase gate moves. No design change.) 2026-08-26 (W0-A performance precommitment SIGNED as drafted by Charlie Cheever 2026-08-26 (exact-9e packet ruling item 2, recorded @11992d827; `performance-precommitment.json` reads `status: "signed"`) — the 2026-08-25 phase-status note's DRAFT-AWAITING-CHARLIE-SIGNATURE clause and its "remaining W0-A debt" sentence corrected so neither reads as a still-open ask, and a 2026-08-26 note added recording the ratification and the now-binding per-axis limits; remaining W0-A debt is the three pinned tape apps and the 88-bit style-order fixtures; no design change); 2026-08-26 (**§10 post-acceptance amendment appended** — the RFC 0540 collection folds (0504 §3 row 60): WS-H gains the hidden set as a second named presence-set customer and 0540 L6 hidden-run coalescing as WS-H skip-work; WS-G drops `exact-list-core` from the interim-home and later-extraction lists because its exit is deletion under 0540 §8.4 rather than extraction, with a naming correction (no such Rust crate exists — `packages/exact-list-core` is JS, `kernel/src/native_list.rs` is the kernel surface) and the 0540 §8.4(2) not-after ordering on WS-G Phase 4 recorded; the Week-0 recovery-class table gains an explicit deletion-before-activation row for op 38; WS-C gains four tree-bound feed records with WS-E FFI projections. Delivery post-window per 0540 §8.6; no phase gate moves. 0506 D1(d) burn-down lane.) 2026-08-25 (OQ3 closed — staged generation snapshots selected by Charlie Cheever via orchestration session exact-9e, register item 12, the undo journal staying the named fallback, unblocking the arena transaction engine, the rejection-atomicity gate, and the `BatchValidator`/`ProtocolTreeReset` deletion path for the kernel Phase 1 lanes; the W0-A fingerprinted performance baseline landed — capture harness `kernel/benches/w0a_baseline.rs`, pinned Bones capture and the draft per-axis regression limits / layout+allocation positive targets / trend-only wake→presentation disposition in `docs/kernel-refresh/w0a/`, marked DRAFT-AWAITING-CHARLIE-SIGNATURE, with the registered `kernel-w0a-performance-baseline` check; phase-status note updated; no other design change); 2026-08-23 (Phase 1 entry — the LLP 0487 corpus harness over the current kernel landed with its freeze-pinned differential; phase-status note updated; no design change); 2026-08-23 (W0-B executed — phase-status note added after the phase table; mechanical factual corrections from `docs/kernel-refresh/w0a/DISCREPANCIES.md` folded into §1/§2/WS-F with citations: `Node` is 576 B not ~570, the 54 prop probes are a syntax-probe count, the Rust motion clock registry is a 204-line block/125-line impl and a `pub` rlib surface, 21 literal `Err(_) => -1` sites not ~130, the in-process decode seams are TUI's `HostInterpositionFrame` and Windows' `OpParser`+`execute_ops`, `InteractionArena`/direct setters are live migration candidates, and `MotionDescriptorKind::Driver` is the only census-proven dead variant — no design change); 2026-08-23 (§8 in-force note added: all seven companion amendments ratified by Charlie Cheever 2026-08-23 as drafted in LLP 0491.001 (decision relayed via orchestration session exact-9e) and landed in their target authorities with the break package regenerated; the W0-B gate condition "A-0496-CORE-MEMBERSHIP landed" is MET, W0-B now waits only on RFC 0496 V-A/V-B exits; the two WS-D interim caveats updated to their landed state); 2026-08-22 (accepted by Charlie Cheever — "accept 0491" — after the owner-authorized r17/r18 revision cycle reached dual-READY; §8 companion-amendment asks remain owner-gated); 2026-08-22 (r18 — owner-requested fold of both r17 NOT READY
verdicts. The five endorsed theses remain unchanged. Verified the union
against the tree and restructured every cross-authority decision as an
explicit owner-gated amendment ask: Accepted LLP 0507/0515 remain
controlling for `ProducerId` restart behavior and the still-null batch
successor/refusal/reconnect cells, while a separately admitted EXWF semantic
epoch and the Exact Native v2/Motion dimensions are requested rather than
silently installed. Added `RootMigration` to the proposed closed identity
obligation; completed the token-bound multi-root recovery transaction and
LLP 0297 island records; split W0-A from V-A/V-B-gated W0-B; restored shipped
`state … hidden` absence semantics; retargeted Exact Native to Accepted 0510;
and folded the terminal editorial/provenance repairs. Where the families
diverged, the tree selects Codex's stricter readings: `host-data-plane-feed`
§8 disproves Grok's EXNODE-complete assessment, the rejected-frame ticket's
all-root acknowledged transaction disproves the partial-recovery close, and
0507/0515 plus the generated null pins disprove the claim that r17 could
close the producer/successor cells. Grok's verified ApplyMode,
Phase-2 `DestroyView`, and lossless 64-bit bridge repairs remain.)
2026-08-22 (r17 — owner-authorized 0491 revision cycle. The
authorization named the checked-in source r11 and requested r12, but this
tree already contained r12 through r16; this entry stays monotonic rather
than duplicating an existing revision number. Reverified the requested
review claims against the current tree; repaired the live batch identity and
receipt contract, made the LLP 0297 feed sections atomically correlated in
EXNODE, restored fingerprinted paired baseline rules, tightened selector and
multi-kernel acceptance, made logical-edge migration precedence explicit,
and corrected the Phase-3 boundary projection. The Codex/Grok sequence-gap
disagreement is adjudicated in favor of Codex: the current native-web host
does reject gaps.) 2026-08-20 (clock-join sync per the 2026-08-20 holistic corpus reviews — TUI validate-then-apply repair deferred past the window per the row-9 freeze (known-red row kept, no longer a Phase-2 gate; 0506 D2 anti-hostage rule cited); OQ7 annotated with LLP 0506 as the program-wide answer.) 2026-08-20 (r16 — follow-up loop round-2 revision, both
families folded. Identity: a third **restore** allocation class
(reconnect/recovery-adopt snapshots reuse, never bump), the alloc-gen
table owned by the producer session and hydrating scratch kernels,
`ProducerId` minted by the protocol-admission owner and **reissued** on
re-attach with matching `ExecutionGeneration`, explicit fence domains
(producer FIFO vs cross-stream carriers), 64-bit no-wrap counters, and
the identity-carrier matrix generated from a closed `identity-events`
enum with per-field bump/hold including the no-bump rows. Recovery:
the class table is total over the inventory's live sidecar ops (pager/
motion/list-model/menu/graphics), default tree-bound+rollback, and
TUI's mirror-before-apply is reclassified as a §2-violating **Phase-2
host repair with a known-red injection row**, never a declared
coupling fact. Wire semantics: `DestroyView` stays single-node until
the Phase-2 EXWF revision; Percent wire = authored points (0–100) with
one generated Taffy conversion; per-row `Auto` admission with typed
rejection; the direction table's exhaustiveness is flex-scoped with a
separate grid-direction transform and declared RTL-grid degradation
until it lands. EXWF: the digest is closed over frame/capability/
receipt headers too, SHA-256 domain-separated with a defined
projection and no self-reference, referenced default authorities
digest-pinned, revision/digest move together and both ride the
capability context; EXFF adds the tuple under a bumped container
version. Ledger: a canonical consumer census joins rows exact-once
bidirectionally so the ledger cannot green by omission; non-waivable
rows name their 0496 class. Phase 3 gate reworded to
projection-by-boundary; Phase 4 gains real wasm/forbidden-closure
checks, facade-edge retirement before the rlib freeze, and the Motion
compatibility-object disposition; §7.1 cites RFC 0492 §8 as the made
evaluator ruling awaiting acceptance; SemanticFrame's
confirmation-oracle sentence re-attributed to LLP 0499; the web
gap-check claim narrowed to a Phase-2 admission requirement;
root-scoped reset lowering spelled (context incarnation bump, no new
opcode); Related adds 0492 §8/0493/0496/0497.) 2026-08-20 (r15 — dedicated follow-up loop, round-1 revision:
both round-1 families folded on merits. One identity algebra: the raw
address becomes `(ProducerId, ExecutionGeneration, rootId, root
incarnation, local ViewId, node-allocation generation)` with the kernel
`ProducerId` minted at attach, LLP 0374's crypto-fresh
`producerIncarnationId` joining at the Acto boundary (never in the
kernel node key), `ExecutionGeneration` a separate context field (the
r13 "component of producer incarnation" naming retired here and in
Related/WS-D), alloc-gen host/kernel-owned riding refs/receipts with a
bump-only-on-real-reallocation rule, WS-D's capability context restated
in the six-field terms, the identity-carrier matrix promoted to a Week-0
authority artifact, and the owed LLP 0488 W4 amendment recorded and
landed in lockstep; §7.4 added — the owed RFC 0492 performance-claim
transfer in §7.3's shape (0491 supplies instrument + result equality;
0492 owns the ≤1 ms/≤64 number, gesture classification, and M-D's gate;
OQ10's stale criterion repaired; 0492 r4 lands in lockstep); the
Phase-1 adapter circularity removed (pre-Phase-1 corpus harness over the
current kernel as entry; the arena adapter is Phase 1's first slice,
the retargeted corpus its exit); recovery atomicity made executable
(orthogonal treeCoupling × failureDisposition + adoption owner/point,
rollback-or-swap, resync, presentation-ack; current families assigned —
Motion tree-bound, Graphics independent+last-good per 0352, TUI
mirror-before-apply — proven by injection); WS-B `defaultRef`
domain-qualified (layout rows → 0486 profile; non-layout defaults get
their own registered authority); the EXWF carrier becomes
`{exwfFrameRevision, exwfSchemaDigest}` with a canonical domain-
separated whole-package preimage, golden fixtures, unification with
WS-D's schema digest, and an explicit compatibility-tuple major bump;
the ledger row schema made enforceable against 0496 (non-waivable rows
bound to the non-quarantinable core) and the Week-0 authorities
consolidated into one generated break package; SemanticFrame
restricted — raw is dev/test/migration-oracle only, production
projects one-way into observability-crash-capsule-v1, no unkeyed
content digests for non-public values; WS-G's aquifer row cites 0498's
layered identity; minors per Revision history — performance-gate seams
aligned, DAG transition-vs-final edges + HostInterpositionFrame split,
EXNODE tuple marked illustrative, Phase-2 0486 WS2 prerequisite
narrowed, direction lowering recorded as the joint 0486
translate-via-table amendment, evidence counts corrected.) 2026-08-20 (r14 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision. Identity algebra becomes one document: WS-D's incarnation mapping
now repeats WS-A's 0417/0500 rule verbatim (the stray "HMR generation
change" phrase deleted) and identity/reset joins the Phase-2 epoch ledger;
raw-ref aliasing is closed with a node-allocation generation plus a
`producerAuthorityId` kept distinct from `ExecutionGeneration` (the
documented `create_view_with_id` rematerialization gap is the exhibit);
Phase 5's relayout exit is demoted to result-equality + the dirty-set
instrument, with the ≤1 ms/≤64 claim and its gesture classification moved
to RFC 0492; WS-B's `default` column becomes a profile-owned `defaultRef`;
recovery atomicity is scoped (producer-isolated default, per-class
atomicity declarations); WS-G's DAG completes (shared wire crate, kernel
facade/staticlib, interim homes, the generated-app archive consumer);
SemanticFrame adopts the replay privacy authority (raw = dev/test-only);
performance exits gain precommitted limits + positive targets + one
integrated device measurement; the Phase-2 ledger gains an independent
EXWF digest carrier ({frameRevision, opcodeDigest, eventPayloadDigest} —
never overloading 0485's plan-VM digest or the App-ABI protocol id),
0486-instrument prerequisites for grid/direction, entry-security tests on
the Ibex row, non-waivable rows, and a Week-0 row schema; the
dependency-domain bits are reserved beside `token-ref`; §7.1/§7.3 defer
correctly to 0500 D2 and to Accepted 0490's controlling status; V-A/V-B
enters Phase 1; AQ-B exits come off the Phase 4 gate. See Revision
history.) 2026-08-20 (r13 — program super-refine round 1 (LLP 0478–0504 loop) applied, both families NOT READY on r12: the owed RFC 0490 WS-C amendment table added as §7.3; App-ABI coordination reduced to one rule with two independent compatibility dimensions (WS-D's unconditional same-rev bump deleted); Phase 1's `exact-motion` gate names its crate contents (slab + closed-form evaluator + phase-consumer API) so 0490 M3 cannot be unblocked by a stub; the identity tuple binds to LLP 0417/0500 (producer incarnation = ExecutionGeneration; HotRevision never bumps it) with an exhaustive identity-carrier matrix obligation; Phase 2 becomes a machine-readable breaking-window ledger carrying the 0493/0497/0485 riders, the 0486 grid-amendment prerequisite, the 0495 A-B before-trace-deletion ordering, and a per-host transactionality matrix; WS-G publishes a checked crate DAG, resolves the selection-sha2 contradiction, and carries 0498's AQ-B exits at Phase 4; hop caps survive as bounded traversals or a reviewed depth; SemanticFrame is decided as the owned persisted flight-recorder format and registered Week 0, with the legacy oracle frozen content-addressed; Phase 5's benchmark is fully executable; r12 leftovers swept (Week-0 OQ4 text, WS-E abort wording, WS-B one-file hedge and Kotlin mention, §2 consumer matrix and byte-compare phrasing, evidence-bullet cites). See Revision history.) 2026-08-20 (r12 — author-authorized extension: super-refine round 3 applied (both families NOT READY on r11) plus the author-relayed RFC 0498 §8 Aquifer ask. Identity becomes a namespaced tuple with the three-rule Acto split; rejected-frame recovery is the ticket's acknowledged state machine as a Phase 2 gate; the oracle tape is an owned versioned SemanticFrame IR; RTL becomes a generated truth table (no double-reverse — FlexStart is already flex-relative); grid is a portable track grammar amending the layout profile jointly with 0486; compatibility is per-dimension epochs and App-ABI widens only on its own schema; the consumer matrix corrects supervisor/worker to transport-only and EXNODE to one-schema-two-projections; Android closed as JNI typed-ops, EXNODE-exempt (OQ4 closed); the abort boundary is kernel-scoped, not workspace-wide; the early exact-motion gate enters the phase table; exact-aquifer joins WS-G with exact-motion hygiene and Aquifer receipts join WS-C; see Revision history)
**Related:** LLP 0150 (Family A ideas 3–4 — the deferred ABI-breaking simplifications this RFC executes), LLP 0258 (kernel columnar hot paths — absorbed as WS-A, un-gated; marked Superseded by this RFC at acceptance), LLP 0283 §B/§D2 (July kernel survey), LLP 0159 ("the hot paths contradict the brand"), LLP 0297 / LLP 0322 (threading contract — hard constraint), RFC 0490 (Accepted r5 — §7 rulings in concert; M3 blocked both legs on the 0099↔0491 evaluator amendment; the §7.3 WS-C delivery-ownership addendum landed 2026-08-20), LLP 0417 (native source HMR, Accepted — the ExecutionGeneration/HotRevision algebra the WS-A/WS-D identity tuple binds to), LLP 0500 (dev-loop plan patch, Active — HotRevision plan patches never bump `ExecutionGeneration`), LLP 0331 / LLP 0332 (Accepted Exact Native product/delivery authorities), LLP 0333 (Superseded by Accepted LLP 0510; only 0510 §10's carried-forward sections remain normative), LLP 0510 (Accepted Exact Native v2 boundary and compatibility-tuple authority), LLP 0328 (contract-native execution tier), RFC 0423 (minimum runtime substrate / engine selection, Review), LLP 0486 (one layout language — parity instrument prerequisite; its WS2 `tests/layout/layout-profile.json` authority binds WS-B), LLP 0487 (choice layout semantic kernel, Review — specifies the frozen-intent semantics + the conformance corpus this program regresses against), LLP 0488 (container-size feedback — return-path co-design; W3 measured set landed), LLP 0489 (Superseded by Accepted LLP 0508 — incorporated closed-type provenance for WS-B enums is read through 0508), LLP 0095 (text geometry), LLP 0099 (Motion), LLP 0313/0349 (layout islands / programmable layout), LLP 0406 (Expose/DRM milestone), RFC 0492 (Motion refresh program — consumer of the §7 rulings; §8 is the evaluator ruling §7.1 awaits), RFC 0493 (input refresh — Phase-2 freeze-window riders), RFC 0495 (Acto on the substrate — receipt consumer), RFC 0496 (verification refresh — V-A/V-B gates mutating W0-B and all implementation phases; its core-membership mechanism is an r18 amendment ask), RFC 0497 (Facet refresh — the WS-B dependency-domain bits' adoption driver), RFC 0498 (Aquifer data tier — §8 ask folded into WS-G/WS-C), LLP 0507 / LLP 0515 (Accepted EXWF and session-identity authorities; r18 amendment asks are isolated in §8), LLP 0482 (five parallelism tiers — the governing taxonomy for OQ9; WS-A supplies the contiguous representation its on-device tiers require), `issues/20260801-stale-handle-alias-audit.md`, `issues/20260817-protocol-rejected-frame-resync-path.md`, `tests/protocol/protocol-inventory.json`

## Summary

Rebuild the load-bearing internals of `exact-kernel` in one coordinated
program while Exact has zero external users, so that every breaking change —
storage model, wire format, FFI shape, error model, crate boundaries — lands
inside a single window instead of trickling out as post-1.0 migrations.

The program has nine workstreams grouped around five theses established by
the 2026-08-20 review (six parallel independent reads of all 54 kernel source
files, cross-checked against the iOS/Windows/TUI/Android consumers, the JS
encoders, and the LLP corpus):

1. **The node/prop data model is the single root cause** of most other
   pathologies: the wire interns props as `u16` IDs and the kernel throws
   that away into `HashMap<String, PropValue>` on a 576-byte `Node` in a
   hash-mapped tree (measured by the W0-A integration pin,
   `kernel/tests/w0a_layout_pins.rs`; `docs/kernel-refresh/w0a/DISCREPANCIES.md`
   item 1).
2. **"Binary zero-copy shared memory" is ~40% true**, and the false parts
   (per-field egress, JSON event ingress, JSON motion outcomes rebuilt
   twice per drain) are
   load-bearing production paths.
3. **Hand-synchronized parallel truths are the dominant drift risk** — one
   style prop touches 8–9 hand-ordered places across four languages, and the
   style decoder silently ignores trailing bytes.
4. **The kernel pays compatibility debt to a population of zero** — ~4–5K
   deletable lines of superseded shims, dead opcodes, and parallel write
   paths.
5. **Several semantics are wrong and only cheap to fix now** — subtree leaks
   on destroy, unfenced view identity across `reset()`, full-replacement
   `SetStyle`, an equal-fr-only grid model, an eight-convention error
   surface.

This is a breaking-window RFC, the structural sibling of Accepted
RFC 0490: 0490 makes the GPU the substrate Exact paints with; this RFC
makes the kernel a substrate worth painting from. §7 records two rulings
made in concert with 0490: the clock-phase authority (adopted by 0490
§3.3.1) and native motion-evaluation ownership (owned here — 0490
explicitly does not co-decide it, and the atomic evaluator decision is
owed to the 0099↔0491 amendment, drafted in RFC 0492 and awaiting acceptance).

## 1. Motivation

The kernel works, is unusually well-commented, and holds real discipline
(a near-zero `expect` count across the production FFI surface; zero TODO
markers). But it
still runs on the Phase-0 prototype's data model, and its own metadata says
so: `kernel/Cargo.toml` still describes the crate as a "prototype for
protocol benchmarking," and `ffi.rs:1-5` still labels the direct-setter
surface "the 'JSI direct' path for comparison with the binary protocol" —
a benchmark that shipped as permanent production API and still runs in the
Swift host alongside the command buffer.

Representative evidence per thesis (full detail in the review; file:line
refs verified against `main` 2026-08-20):

- **Data model.** `protocol/prop_decoder.rs:49-149` maps the wire's fixed `u16`
  ID table to `&'static str`; dispatch produces only `PropValue::String`
  values (`dispatch.rs:803-805`) and `Kernel::set_prop` allocates the
  `String` key per write; 54 string-literal prop probes are scattered
  across the kernel (a syntax-probe count — 42 same-line `props.get`, one
  `contains_key`, seven `set_prop` dynamic-key comparisons, four
  boolean-string parses; neither 54 unique lines nor 54 unique prop names —
  `docs/kernel-refresh/w0a/DISCREPANCIES.md` item 2);
  `PropValue::{Bool,Int,Float}` exist but the protocol path can never
  produce them, so booleans are matched against `"true" | "1" | "yes" |
  "on"`. `Node` (`tree.rs:157-185`) carries three mostly-empty HashMaps;
  the tree lives in `HashMap<ViewId, Node>` so every hop hashes.
- **Brand vs hot paths.** Egress is 30–60 FFI crossings per node
  (`ExactEngineTreeDomain.swift:786-878` — 13 unconditional string-prop
  probes; the probe-then-copy pattern spends a second crossing per
  present value). Inbound events are JSON C-strings
  (`ex_hermes_dispatch_event`; the 231 `JSONSerialization` sites are an
  iOS-wide count that bounds the adapter surface, not all event ingress).
  Motion outcomes rebuild an O(N) JSON snapshot twice per drain while Swift
  consumes `records.first` (`motion_outcome_ffi.rs:378-472`). A 128-byte
  motion-schema envelope wraps every frame and is re-validated per frame
  (`motion_transport.rs:236-544`).
- **Parallel truths.** The SetStyle value order is maintained by comment
  ("Write values in the EXACT order the kernel reads them!") across four
  implementations, only 2 of 88 mask bits have cross-language byte-parity
  coverage, and `style_decoder.rs:472` ends `let _ = offset;` — trailing
  bytes are silently discarded, so drift is wrong pixels, not an error.
  Of the wire style-enum vocabularies (15 hand-ordered `*_VALUES`
  tables in the TS encoder plus the imported BackdropMaterial), only
  BackdropMaterial has an inventory check, and the TS encoder
  still encodes unknown strings as the fallback index — dev builds warn
  once per (vocabulary, value) since the 2026-07 silent-fallback
  incidents, but an explicit `NODE_ENV=production` silences the warning
  and the fallback byte still ships (`buffer-writer.ts`), so the WS-B
  hard-error rule remains the repair. Every FFI signature is
  written four times (Rust, the hand-written ≈2.8K-line C header, Swift
  wrapper, test macro). The Kotlin decoder trusts the frame's `op_count`
  and silently drops many style bits (empty `read*IfPresent` arms); only
  its BackdropMaterial slice is parity-gated.
- **Compat debt.** Protocol v1 has no producer; five-plus text-measure
  callback generations (plus the segment-preparation and native-control
  registrations WS-E counts separately) cascade at runtime; `exact_motion_pager_answer_turn` is a
  "frozen v1 adapter ABI" against the project's own previous month; a Rust
  motion clock registry (a 204-line support block, 125 of them the registry
  type/impl — `docs/kernel-refresh/w0a/DISCREPANCIES.md` items 5 and 10) is a
  `pub` rlib surface with zero repo-local production consumers; a dead pager
  and a live second arena survive past their LLP 0099 M4 absorption date.
- **Semantics.** `destroy_view` detaches a subtree and leaves the
  descendants alive in the node map and in Taffy (`lib.rs:1228+`; its own
  comment calls the survival deliberate — the correction is a semantic
  choice, not an accident repair); `ViewId` is an unfenced `u32`
  above Taffy's *fenced* slotmap handle and `reset()` re-issues IDs
  (`issues/20260801-stale-handle-alias-audit.md`); one overflow value is
  written to both axes so `overflowX/Y` cannot exist (`style.rs:989-1000`);
  `Dimension::Percent` is 0–100 against Taffy's 0–1; grid is a u8
  equal-fr count that silently degrades on native; 21 literal
  `Err(_) => -1` sites (all in `ffi.rs`; broader `return -1`, `None => -1`,
  and `unwrap_or(-1)` conventions in other syntax push the collapse surface
  higher — `docs/kernel-refresh/w0a/DISCREPANCIES.md` item 4) collapse typed
  errors under eight incompatible `i32` status
  conventions, including one subsystem where `1` means success and another
  where `0` does, in the same header.

Every one of these is individually known — LLP 0150 deferred two of the
fixes explicitly "for a breaking window," LLP 0258 argued the storage
rebuild through five review rounds, LLP 0283 §B flagged the style-order and
mask-saturation hazards in July. What has been missing is the program that
spends the window on all of them at once.

## 2. Constraints (non-negotiables this RFC preserves)

- **LLP 0297 threading contract.** Single-owner `Kernel` on the runtime
  thread; the motion domain is a separate main-owned structure; no new
  cross-thread read surfaces; no new sync module-call surfaces. Every
  workstream below is shaped to preserve this.
- **The Sandwich Model** (restated by RFC 0490 §2). Native views remain the
  default materialization; this program serves that path first.
- **Frame transactionality survives.** Validate-then-apply with typed
  rejection outcomes is a guarantee hosts rely on; WS-H may change the
  *mechanism* (journal vs shadow validator) but never the guarantee.
- **The protect list.** The unified `dispatch.rs` decode core, the EXLT
  export design, the closed-form motion math, the gesture recognizer
  framework, crash-capsule signal discipline, the no-panic policy, and the
  inventory parity gates are load-bearing and correct; workstreams extend
  them rather than replacing them.
- **Exact Native is a first-class consumer — at the layer LLP 0331
  assigns it.** The kernel's Rust rlib is a primary **host-tier** surface
  on equal footing with the C ABI: the Windows/TUI hosts consume the
  kernel in-process as Rust, and Apple consumes it through the generated
  ABI. **Supervisors and workers are transport/test consumers only** (the
  tree agrees: the supervisor has no kernel dependency and the worker
  links it only for tests — WS-G's consumer matrix is the authority), so
  they never judge the rlib. The **app/producer
  tier stays behind LLP 0331's generated App-ABI / host-operation
  boundary** — producers emit host operations and never link the kernel
  (the tree agrees: `exact-native-ui` disables `contract-native`'s
  optional kernel feature; `exact-native-runtime` has no kernel
  dependency), and this RFC does not amend that Accepted boundary.
  Rust Native roots emit the shared binary protocol under App-ABI schema
  pins (`exact-native-web-wasm`). Consequences this program honors:
  Phase 1's test migration doubles as *defining* the host-tier Rust API,
  judged by the Windows/TUI/structured-apply consumers and the generated
  Apple ABI, not only the FFI;
  the in-process decode seams — the TUI host's `HostInterpositionFrame`
  path and the Windows host's `OpParser` + `execute_ops` path (they differ
  today; `docs/kernel-refresh/w0a/DISCREPANCIES.md` item 8) — and the
  Rust-producer protocol paths (e.g. `sameGenerationReconnectFullSnapshot`)
  are protected surfaces every wire change must keep first-class; no
  workstream may privilege the Swift host's shape over the in-process Rust
  shape when the two pull apart; and in-process Rust mutation is a
  **structured apply API** sharing the command-buffer commit path —
  "rlib is a primary surface" is not satisfied by a bytes-only shim.
- **Trust posture v1, stated (OQ11's default).** One kernel per app
  process; the protocol is a correctness boundary, not yet a security
  boundary; adversarial-input guarantees are Expose-gated (OQ11), and no
  WS-D claim pretends otherwise.
- **Acto is a first-class consumer of kernel outputs.** Agents are a
  primary user class of this framework, and their read path today is the
  worst-served: full-walk JSON snapshots over probe-then-copy FFI,
  invalidated by the selection generation (any prop write), single-slot
  cached, with `testId` — the primary agent handle — string-probed per
  node with no index. The idealized Acto (high-frequency structured
  observation, `observeAfter` receipts, tree diffs, contract-clause
  checks against the live surface) is a named design input to WS-A, WS-C,
  and WS-H below, and agent-ref stability across `reset()`/HMR — listed
  as unproven by the stale-handle audit — is an acceptance criterion of
  WS-A, not a hoped-for side effect.
- **Single-threaded by contract, parallel-ready by construction.** This
  program adds no threads inside the kernel: concurrency stays at the
  LLP 0297 boundaries (runtime thread, main-owned motion domain, host
  display link) plus the GPU service, where the genuinely massive
  parallelism already lives (RFC 0490's rasterization tier — pixels,
  glyphs, and path segments are the 10⁵–10⁷-item workloads GPUs are for;
  a UI tree's 10²–10³ dependency-coupled nodes are not). What WS-A *does*
  commit to is the data-oriented shape that keeps every parallel option
  open and cheap later: contiguous columns, index-based topology,
  no iteration-order-dependent results, no interior mutability on the
  frame path, and derivations that read immutable snapshots — so
  fork-join inside a call, per-root/per-island layout, or off-thread
  export derivation each become a local change, not a redesign.
  Determinism is a hard requirement throughout: the LLP 0486 parity
  program compares layout across hosts under its tiered
  Semantic/Frame/Band equality rules — and Frame-tier fixtures compare
  exact values — so any future parallel
  reduction must be partition-ordered, never racy-accumulated. Revisit
  triggers live in OQ9.
- **One verification registry, one authority per boundary** (LLP 0150).
  New generated tables register their authority in `exact-contracts.json`;
  no new root `check:*` without a registry entry.

## 3. Workstreams

### WS-A — Node/prop data model (absorbs LLP 0258, un-gated)

Generated `#[repr(u16)] enum PropId` used wire→storage→reads, with a
declared value type per prop (decode `disabled` to `Bool`, not `"true"`);
`props` as a sorted small-vec or bitmask+slots keyed by `PropId`; nodes in
a slotmap/arena with index-based child links and generation-checked IDs
(subsumes the stale-handle-audit gaps: generational fencing survives
`reset()`, and subtree destroy becomes a free-list push); hot fields
(parent, taffy node, flags, frame) in parallel columns; per-prop
side-effect flags replacing the `after_prop_write` string chain. A
`PropValue::Handle`-class variant reserves a typed lane for resource
references (RFC 0490 L1 textures) so binary resources never ride the
string-prop tunnel the way SVG rasters do today.

One frozen-semantics obligation: LLP 0487 (Review) specifies
choice-layout's frozen-intent semantics as a unique closure result with
a conformance corpus — but today only
2 of its 20 fixtures execute the production `Kernel`
(`kernel/tests/choice_layout_fixtures.rs` covers D2 static defaults;
the other 18 run the isolated choice prototype, and the corpus README
records that no end-to-end analyzer-to-Taffy path exists). (As of the
Phase-1 entry landing, the harness at
`packages/exact-kernel-corpus-harness` realizes all 20 fixtures'
templates through the production `Kernel`; see the phase-status note). The
obligation splits into two non-circular steps (r14 made the adapter
both Phase 1's product and its entry gate). First, a **pre-Phase-1
corpus harness over the *current* kernel** lands in W0-A as entry
work: every applicable fixture's already-chosen template is realized
through today's production `Kernel` and judged under the corpus's own
semantic-equality rules (not undefined byte identity), so the
pre-rebuild baseline exists before any storage changes. Second, WS-A
**builds the production-arena adapter as Phase 1's first slice**, and
Phase 1's *exit* retargets the same harness at the new arena. A gate
that greenlights 2/20 would insulate 18 semantic cases from the
rebuild — the harness closes that hole before the rebuild starts.

Two Acto-driven additions: selector indexes become kernel citizens with
the right **cardinality** — `testId` is deliberately one-to-many
(repeated literals serve `count` clauses; `^`-prefix queries are the
each-row idiom) — so the index is an exact-value multimap plus an
ordered prefix structure, uniqueness enforced only at operation time,
lifecycle maintained on set/clear/destroy/`reset()`/root migration, and
built lazily per root when a selector family is first used. Query rules
are closed rather than left to each consumer: `has`/`missing` test zero
versus nonzero matches, `count` consumes the full match set, and
`press`/`change`, every non-`hidden` `state`, and singular host actions
require exactly one match and reject both zero and ambiguity. **Shipped
`state … hidden` is the explicit absence-claim exception:** zero matches
passes and any present match fails, evaluated before the exactly-one gate
(`contract-monitor.ts:727-740`). Exact and prefix results retain structural
tree order, so repeated rows never inherit hash iteration order.
Identity is
**namespaced, not merely fenced**, and the namespace has two aliasing
holes the tuple alone cannot close — both repaired here. First,
destroy/recreate of the *same local id inside one root incarnation* is
already production behavior: the protocol path (`OpCode::CreateView` →
`create_view_with_id`) rematerializes retired ids, and
`kernel/tests/stale_view_id_alias.rs` documents that gap as such — so a
raw ref that carries only incarnations plus a local id aliases across
that recreate. Every raw ref therefore additionally carries a
**node-allocation generation** minted per (root incarnation, local id)
allocation, with **three allocation classes, not two** — the third is
what makes reconnect and recovery compose with the fence instead of
resetting it: (i) **allocate-after-destroy** (producer `DestroyView`
then `CreateView` of the same local id in the same root incarnation)
bumps; (ii) the idempotent **live no-op** (`create_view_with_id` on a
live same-type id) does not bump; (iii) **restore** — the reconnect
full snapshot, the rejected-frame scratch-preflight adopt, and any
same-incarnation-pair rebuild — **reuses** the existing generations
and never bumps, because a restore `CreateView` re-presents an
allocation that already happened. That reuse is only implementable if
the generation table survives the scratch/`reset()` kernel a restore
replays into, so ownership is stated at the right object: the table is
**owned by the producer session** (keyed
`(ProducerId, ExecutionGeneration, rootId, root incarnation, local
ViewId)`), hydrating any scratch kernel a recovery or reconnect
snapshot targets — never a field of the `Kernel` object that
scratch-replace destroys. Without the restore class, every recovery
snapshot would restart every generation at 1 and a stale pre-recreate
ref would alias again — the exact `stale_view_id_alias.rs` failure —
so restore-as-no-bump is an acceptance criterion of the WS-A
stale-handle claim, not an optimization.
The producer session chooses the class through an explicit
`ApplyMode = Normal | RestoreReconnect | RecoveryAdopt`; it is never
inferred from otherwise identical `CreateView` bytes. The mode is scoped
to the staged batch/recovery transaction, not added to every inner op:
`Normal` performs allocate-after-destroy bumps, while the two restore
modes hydrate and reuse the session table and are echoed in recovery
disposition evidence.
The carrier rule is
unchanged: the generation rides **refs and receipts, never per-op
wire traffic** — `CreateView` grows no field — and the **fence
domains are explicit** so this is enforceable rather than hopeful:
within one producer's own op stream, connection FIFO plus
absent-id rejection already make bare-`u32` targets safe (a producer
cannot race its own destroy); the aliasing hazard lives entirely in
references that **cross the stream boundary** — Acto refs, receipts,
motion/graphics claims, host callbacks, LLP 0488 report keys,
selection/handler/FFI carriers — and every one of those carries the
full address and validates against the session table. Second, there
are **two producer identities, and only
one is a kernel field**. The **`ProducerId` is minted by the
protocol-admission owner at first attach** — the kernel where one is
present; the TS web admission owner on the kernelless web host — and
**re-attach with a matching `ExecutionGeneration` reissues the
previous `ProducerId`** (reconnect is not a fresh mint; attach-mints-
always would rotate every raw ref on reconnect and contradict the
reconnect rule below). Across `ProducerRestart` and coherent full reload,
however, Accepted LLP 0507 §5.3 and LLP 0515 §11.1 deliberately retain a
conservative open cell: producers MUST NOT depend on fresh mint versus
reissue. This Review RFC does not fill that cell by assertion. §8
`A-0507/0515-PRODUCER-SUCCESSOR-ID` asks those owners to select fresh mint
for both new-`ExecutionGeneration` events; until that amendment lands, the
generated package keeps the selection null. The non-open cells remain
explicit: HotRevision, root reset, whole-kernel reset within the surviving
producer session, live no-op, restore reconnect, recovery adopt, and root
migration hold `ProducerId`. Identity
counters (`ProducerId`, allocation
generation, incarnations, batch numbers) are 64-bit and **never
wrap** — exhaustion refuses the operation fail-closed. Their
host-neutral encoding is fixed with the break: unsigned little-endian
`u64` in EXWF/EXNODE and `{lo: u32, hi: u32}` at JS/JSON boundaries;
a JavaScript safe-integer `number` is not an identity carrier. This is
the lossless 0488 §3.8 rule applied once for every 0491 identity field,
not independently narrowed by each bridge. This is the
identity WS-D's capability context and batch numbers
key on. LLP 0374's crypto-fresh `producerIncarnationId`
(`automation-session.ts`, minted per Acto producer-authority instance)
**joins at the Acto boundary** as the authority identity on agent refs
and receipts — it is never a component of the kernel node key — and
`ExecutionGeneration` is a **separate context field beside
`ProducerId`**, never "a component of producer incarnation" (the r13
identification of the two names is retired; LLP 0417 keeps them
decoupled). The full **raw address** is therefore
`(ProducerId, ExecutionGeneration, rootId, root incarnation,
local ViewId, node-allocation generation)` — with the stable wire
`root_id` retained beside the incarnation pair — mapped to an internal
generational handle. EXNODE, semantics, agent
refs, receipts, and parent/child validation all bind to this address —
so two live producer/root pairs reusing a local id cannot collide,
same-root recreate cannot alias, and two simultaneously live roots can
never present equal addresses. **Logical rebinding keys stay a separate
identity** (rule (b) below): they are how scripts survive reloads, and
they never masquerade as raw addresses. The address binds to the
decided HMR algebra rather than a free
"HMR generation" phrase: **the `ExecutionGeneration` component IS
LLP 0417's** — it bumps on producer restart and coherent full
reload only; an LLP 0500 **HotRevision plan patch never bumps it** (that
is exactly the edit path 0500 exists to keep identity-stable); root
incarnation bumps on root-scoped `reset()`. Reset is **root-scoped on
the wire**, and its lowering is spelled: there is **no new Reset
opcode** (the Phase-2 wire-visible list stays closed) — a root-scoped
reset is the producer re-attaching that root with a bumped
incarnation in the WS-D capability context, and it carries its own
ledger row; whole-kernel reset remains a host
lifecycle operation (today's `Kernel::reset()` is whole-kernel) that
bumps every root the same way. Subsuming the stale-handle
audit is claimed only when an **exhaustive identity-carrier matrix** is
green: every carrier of node identity — handler ids (`HandlerId` is a
bare `u32` today), event payloads, presenter resource references,
motion/graphics carriers, selection wire identities (which embed the
old runtime/root/reset fields), and FFI holders — is enumerated and
either rebased onto the tuple or explicitly recorded as
fenced-by-other-means. The matrix is a **W0-A authority artifact**
on the same list as the SemanticFrame schema and the ledger row
schema — one mapping table (raw address ↔ WS-D capability context ↔
LLP 0488 report keys ↔ Acto refs ↔ selection/handler/FFI carriers)
stating, per consumer, which fields it carries, who mints them, and
what bumps them. The event list is a **closed `identity-events`
enum with a per-field bump/hold/rebind column**, and the no-bump events are
in it explicitly — `{ProducerRestart, CoherentReload, HotRevision,
RootReset, KernelReset, AllocateAfterDestroy, LiveNoop,
RestoreReconnect, RecoveryAdopt, RootMigration}` — so the paths that must
*not* bump (restore, recovery adopt, migration) are matrix rows, never
inferences. `RootMigration` is not shorthand for a destroy/recreate or a
pair of collateral root resets: it atomically moves one still-live node or
subtree binding from a source `rootId` to a destination `rootId` while
`ExecutionGeneration`, both roots' incarnations, `ProducerId`, and the
node-allocation generation hold. The producer-session table atomically
tombstones the source binding and rekeys that same live allocation under the
destination address; an already-live destination address or a partial
index/table move refuses the migration. All raw source addresses therefore
fail closed because their `rootId` no longer names the allocation, while the
logical key may rebind under rule (b). Selector indexes, parent/child
validation, cross-stream carriers, and receipts move in the same transaction.
Accepted LLP 0507/0515 still publish the earlier nine-row algebra, so this
tenth row is a **generated pre-activation amendment obligation**, not a
silent override; §8 `A-0507/0515-ROOT-MIGRATION` is required before the row
can activate. The break package generates the matrix and the
recovery-class table from this enum and the extension inventory,
so WS-D, WS-A, 0488 report keys, and Acto refs cannot drift by
prose — so
WS-C receipts and WS-D frames are never designed against a tuple that
later grows a field. One sibling already copied the pre-r14 shape:
LLP 0488 §3.8 keyed reports on a three-field tuple "aligned with
RFC 0491's identity namespace" — exactly the aliasing surface the
allocation generation closes — so the **owed 0488 W4 amendment**
lands in lockstep with r15, aligning its report keys with the full
address. The Acto criterion splits
into three closed rules, ending the stability-vs-fail-closed
ambiguity: (a) raw refs are generational and **fail closed** on
`reset()`, incarnation bump, and id reuse — the node-allocation
generation is what makes same-root reuse detectable, not hoped-absent;
(b) **logical keys**
(`testId`, producer-named ids) **rebind** across HMR and root
migration — that is what agent scripts that survive a reload key on;
(c) a live agent ref is the full raw address, never a bare u32.

One capability to preserve, not build: the columnar arena keeps
**whole-tree snapshot cheap for the Exact-owned columns**, with
rehydration defined as columns-plus-rebuild — Taffy state, intern
tables, and side indexes are reconstructed from the columns, not
serialized. Nothing in this program ships on top of that, but three
roadmap items will want it — prewarmed/instant launches extending
LLP 0307's precomputed first frame, crash-recovery restoring the last
good tree, and build-time/server-side layout for native payload
delivery (LLP 0351), whose real blocker is a portable text measurer
(the RFC 0490 OQ1 paint-tier bake-off and LLP 0095 own that). WS-A's
only obligation is to not design the arena in a way that forecloses
cheap serialization.

LLP 0258's promotion gate (rebuild the lost Phase A benchmark first) is
**dropped as a veto but kept as evidence**: with zero users, restructure
first — but W0-A captures a pinned pre-change baseline (named machine,
fixture set; layout, mutation, export, allocation, and bytes-per-node
metrics) so Phase 1's "≤ current" gate has a real comparand instead of a
lost one. The baseline is fingerprinted by source commit and legacy-kernel
artifact digest, CPU/core count, OS and power state, Rust toolchain, build
configuration, fixture hashes/seeds, and benchmark parameters. Every
"≤ baseline" claim is a paired comparison: legacy/control/candidate arms
run interleaved in one session on one machine under a recorded noise floor;
cross-job or cross-week numbers are trend context, not acceptance evidence.
These instruments restore 0258's comparability discipline without restoring
its promotion veto. 0258's risk register (reset drift, dual-state, generation
misuse) carries forward as this workstream's checklist.

### WS-B — Single declaration authorities + codegen

Extend the proven `protocol-inventory.json` pattern to the three boundaries
that never got it:

1. **Styles:** one table carrying name, bit, wire order, encoding, a
   **domain-qualified `defaultRef`** (never a semantic-default value of
   its own: **layout rows** reference the 0486 layout-profile
   authority, while **non-layout rows** — paint, typography, image,
   gradient, material, domains that authority explicitly does not
   own — reference a separate registered non-layout default authority,
   itself a WS-B deliverable with an `exact-contracts.json` row; where
   storage needs a distinct unset/initializer sentinel it is
   separately named as such and stays non-semantic), Taffy mapping, and layout/text-measure
   membership — generating the Rust
   struct + masks + decoder (the generated Rust `Default` impl is
   storage initialization only and is documented as **not** a
   semantic-default authority — the profile stays the single default
   truth across all four layers), the TS encoder write sequence, the
   web decoder and the Android JNI typed-operation projection (the Kotlin
   decoder is deleted per OQ4, never regenerated), `StylePropsFFI` + both
   conversions, and both
   eq-functions. The value encoding **reserves a `token-ref u16`
   discriminant now**, and **reserves the dependency-domain bits beside
   it** (RFC 0497 §4.2's paint/layout/text/semantics invalidation
   carrier) — both are OQ8-class representation-extensibility
   obligations: reserving discriminants and domain bits is cheap inside
   the break, a later sidecar migration is not, and 0497's F-B is the
   named adoption driver for both; adopting the
   theme table stays follow-up-scoped. `SetStyle` becomes a patch (see WS-D). The table's end
   state is a **mutation algebra**, scoped to LLP 0150's generated-code
   rule: each row declares storage lane, dirty effects, transaction
   record (per the OQ3 engine), EXNODE column, and fuzz strategy — and
   the generator emits **data tables consumed by one readable,
   hand-authored generic mutation engine**, never per-prop imperative
   code. The 0486 layout-profile stays a separate conformance-data file
   (its "never contains an algorithm" boundary holds); the style table
   consumes it.
2. **Enums:** every wire style-enum vocabulary (the 15 hand-ordered
   `*_VALUES` tables plus BackdropMaterial) generated both ways; an unknown
   string is a **hard encode error in every build** — dev throws,
   production rejects the encode — never index 0.
3. **FFI:** a small IDL per capability group generating the Rust
   `extern "C"` wrappers, the C header, the Swift wrapper + typed Swift
   enums, the 26 discriminant tables, and layout assertions both sides.
   (cbindgen is an *interim* fallback only — it generates the header
   but not the Swift wrappers or discriminant tables, so program exit
   requires the full IDL or an equivalently complete generator; uniffi
   is rejected — it fights the zero-copy pointer ABI.)
4. **Clock phases** (joint with RFC 0490 M2): the frame-clock phase list
   becomes a generated cross-language authority (§7).

**One-authority coupling with the 0486 program (binding).** LLP 0486 WS2
already extracted `tests/layout/layout-profile.json` as the LLP 0150
layout-profile authority — the manifest of layout semantics, defaults,
and divergence tiers whose absence caused the four-disagreeing-default-
layers bug class. The WS-B style table does not stand beside it as a
second truth: **layout** absent-field defaults, per-tag defaults, and
divergence tiers are **defined in the profile authority and consumed by
the style table's generator** (the profile's scope is the portable
layout vocabulary — it does not grow paint or typography rows; those
live in the separate registered non-layout default authority above) —
never merged into one file: the profile's "never
contains an algorithm" boundary holds, and the style table's
mutation-algebra columns would violate it. A WS-B that mints its own
default column would be the fifth default layer — the exact failure 0486
exists to end. Softer pointer, same principle: Accepted LLP 0508
(which superseded 0489/0330) makes the Contract-level closed value domains
and the wire-level enum tables authoritative: they share provenance or are
parity-gated — never hand-mirrored.

Interim non-mutating gates land in W0-A regardless: FFI arity/field-order diff,
`size_of` assertions on `StylePropsFFI`, style-order fixtures across all
88 bits, and differential fuzzing of the two parsers.

### WS-C — Egress and the return path

Generalize the EXLT export design into an `EXNODE` columnar node-snapshot
export (id, parent, type, styles, presence-bitmasked interned props, text,
handlers — an illustrative shorthand, not the schema; the real schema
is the LLP 0297 sectioned envelope below) — one crossing per sync
instead of 30–60 per node — deleting
~22 per-field getters. Inbound events become binary frames — which first
requires the schemas that do **not** exist yet: the inventory today
records only event names and numeric IDs, and one `Change` ID carries
booleans, floats, IME transactions, and media lifecycle maps. The
inventory therefore grows a payload-family schema per event
(discriminants for families sharing an ID, bounds, versioning,
unknown-payload handling), and generated producer/consumer parity over
those schemas gates the Phase 2 break. Motion outcomes become a
typed take-one record (Phase 3 deliverable, before the Swift consumer
of the JSON snapshots is deleted in Phase 4). The export family is designed so a paint-shaped
sibling (`EXPAINT`: resolved styles, text runs, glyph geometry) is additive
for RFC 0490 M6. The upward paths are **one encoding family, three
delivery owners** — a shared framing/correlation design, never one
literal channel, because the four flows on the table do not share an
owner and pretending they do would create exactly the cross-thread
surface §2 forbids: LLP 0488 container-size feedback and **Acto commit
receipts** (each applied frame yields an attributable changed-set plus
generation — WS-H dirty flags are the substrate — so `observeAfter` and
tree diffs become "read the receipt") are kernel exports on the runtime
thread; 0490 OQ6 GPU completions are `exact-motion` command ingress on
main (gpu-service → exact-motion, never through `exact-kernel`); present
receipts stay with the host clock (LLP 0354), sharing a frame-correlation
id with kernel receipts without passing through the kernel. One more
receipt *producer*, no mechanism change: Aquifer write outcomes and
cell transitions ride WS-C frames as receipts (RFC 0498 §4.6;
RFC 0495 reads them as causal evidence) — under a **non-knowledge
rule**: Aquifer receipts are typed records in the WS-C envelope,
`exact-kernel` grows no cell types, and `exact-aquifer` depends on
`exact-kernel-abi` at most. Ordering across the three owners is a
**causal DAG, never an implied global order**: each delivery owner
stamps its own monotonic sequence, records carry `causedBy` links
across domains, and no consumer may assume a total cross-thread order
the threading contract cannot provide. And the ordering against WS-D is
binding: the minimal receipt/evidence schema RFC 0495 A-B consumes (or
an explicitly retained compatibility trace) lands **before** WS-D
deletes protocol v1 — the agent evidence stream is never demolished
ahead of its replacement.

**EXNODE's semantic authority is `docs/host-data-plane-feed.md`** — the
LLP 0297 A1 feed schema (structural order, transitions, removal/disposal
records, selection, routing), already implemented through W4a. EXNODE is
that schema's binary projection as a **sectioned envelope per feed
publication** — matching the feed's decided hybrid: a full snapshot per
structural commit, while patch-tier commits (text/opacity/transform/
background) publish no feed generation and carry only their patch
receipt. "One crossing per sync" means one envelope carrying the record
sections 0297 requires, never a NodeRecord that silently drops them:
ordered `NodeRecord` rows with structural order and layout, transitions,
the merged semantics tree, the built selection document, event routing,
and ordered removal/disposal records, **plus both island record families of the LLP 0297 A1 host feed
(docs/host-data-plane-feed.md)**. `IslandDescriptorRecord` carries the island root id, commit
generation, reset epoch, per-root descriptor generation, closure class,
external envelope, frozen topology and per-node Taffy input style, latest
parent constraints, measurement records, binding descriptors, and each
SharedValue slot's generation/tombstone. `IslandConsumedInputVectorRecord`
binds the reset epoch + descriptor generation to the exact per-slot
generations/values consumed by a commit and preserves the feed authority's
bounded history/freshness policy. Their lifecycle is part of the schema:
explicit removal, measured-island font-registry changes, covering-node
intrinsic invalidation, and a slot tombstone retire the affected descriptor;
an engine/reset-epoch bump retires every descriptor and consumed-input
history. An empty island set may encode empty sections, but omission may not
mean “unknown” or create a separate crossing. The envelope header carries
the root-scoped WS-D `BatchId`, reset epoch, and resulting commit generation;
its section directory, lengths, and checksums validate before any section is
adopted, and consumers atomically adopt or reject the whole publication. A
selection/semantics subgeneration may suppress unchanged payload bytes,
but any section that is present still correlates to that one batch and
commit; it is never independently publishable as a competing tree truth.
Patch-only receipts carry the same correlation fields even though they do
not mint a feed generation. The semantics snapshot joins the same columnar
family (binary and incrementally maintained) rather than remaining a JSON
string; which geometry a receipt
reports (model vs presented) is answered by LLP 0297 OQ8, cited here so
agent receipts and 0490 promotion resolve it the same way.

### WS-D — The batched wire break (EXWF frame revision)

Naming first: the numerical `protocol_version` already counts 1–3, so
this break is the **EXWF frame revision**, not "v2" of that counter.
Contents: 8-byte aligned op header (alignment makes typed slice casts
*possible*; they additionally require checked base alignment of the
staging buffer with an unaligned-copy fallback plus explicit
POD/endianness rules — alignment is necessary, not sufficient); typed
prop values (kills the 50 stringified scalars); `SetStyle` as a masked
patch plus `ClearStyle` (deletes opcodes 0x20–0x22, the ENG-23278
aspect-ratio resync hack, and the stale-mask bug — LLP 0150 idea 3,
executed); one extension-chunk envelope replacing the six bespoke
sidecar framings (one reassembler, one FNV, one reader; a zero-opcode
path for future **commit-rate** publications — per-frame GPU data is
main-owned SharedValue sampling and never rides kernel opcodes); the
MSCH schema handshake moved to runtime attach; delete the ten dead
opcodes and producer-less numerical protocol v1. Explicitly not
flatbuffers — a command log needs no random access.

**Frame identity is redesigned, not dropped.** The kernel mutation engine
does not use the header's `sequence_id` as tree data, but the field is a
live ordering/correlation contract: `HostInterpositionFrame` stores it,
exposes it, preserves it through in-process conversion, and echoes it in
`AppliedHostFrame`; the native-web protocol host rejects any value other
than its expected next sequence; and LLP 0331 R-5.3-2 requires framed,
producer/root-identified sequenced batches (with equivalent admission on
embedded direct calls). The replacement is therefore transport-neutral,
not a deletion. Define `AttachIncarnation = (ProducerId,
ExecutionGeneration)` and
`BatchId = (AttachIncarnation, rootId, root incarnation, batchSequence)`,
where `batchSequence` is monotonic per attach-incarnation/root-incarnation
stream. Every direct, in-process, IPC, worker, and reconnect path preserves
that identity. The EXWF `HostInterpositionFrame` successor exposes the full
`BatchId` before host conversion and returns it unchanged in the apply
receipt beside the resulting commit generation. That tuple is the
correlation identity; it does **not** silently select the successor-state key.
Accepted LLP 0507 §7.3 and LLP 0515 §11.2 require gap/regression refusal
within one live producer-session attach and prohibit producer dependence on
successor behavior across restart, reset, or reconnect, while the generated
package leaves `successorKey`, `initialValue`,
`refusedFrameConsumesNumber`, and `reconnectPersistence` null. Those rules
control here. The current native-web host's observed candidate — initial 1,
exact-next, and increment only after commit — is evidence, not authority.
§8 `A-0507/0515-BATCH-SUCCESSOR` asks the owners to select that candidate,
including same-generation reconnect persistence and a fresh root-incarnation
stream. *(Landed 2026-08-23: the amendment and package regeneration are in
force — see the §8 in-force note; 0507 §7.3 as amended pins the successor
rule, and refused frames consume no number.)*

The break replaces the bare u32 with an attach-issued **capability context
stated in WS-A's six-field terms** —
`{ProducerId (reissued on re-attach per WS-A), ExecutionGeneration (a
separate
context field, never "a component of producer incarnation"), per-root
root incarnation, exwfFrameRevision, exwfSchemaDigest, limits}` — the
revision and digest **always move together** (they are one carrier in
two fields; a revision bump without a digest change or vice versa is
an admission error) — plus a monotonic
per-root batch number, carried by every frame and echoed with the
commit generation in receipts. Accepted LLP 0507's Status-of-This-Document
scope rule and §8 intentionally
computes `exwfSchemaDigest` over the schema package only and excludes the
companion authorities. The desired compatibility vector therefore adds a
**separate admitted `exwfSemanticEpoch`**, never an alias or input to
`exwfSchemaDigest`: a domain-separated SHA-256 identity of the canonical
semantic package containing the complete identity-event/carrier matrix,
recovery-class assignments, breaking-window ledger, and the package-selected
`BatchId` successor/initial/refusal/reconnect rules. Attach, EXFF replay, and
Exact Native artifact admission compare this dimension independently and
name `exwf-semantic-epoch` on mismatch. Identity, recovery, or ordering can
therefore change without lying about wire-schema bytes, but cannot skew
silently. This field landed 2026-08-23 under §8
`A-0507-EXWF-SEMANTIC-EPOCH` and `A-0510-COMPATIBILITY-DIMENSIONS` (see the
§8 in-force note; LLP 0507 §8.4 owns the preimage, LLP 0510 §13 the tuple
carriage — the 0510 schema-file major rides Phase-2 EXWF activation). The node-allocation generation is not a
frame-header field — **batch identity never defines node-reference
lifetime**. Node references separately carry and validate WS-A's full raw
address including node-allocation generation; it rides refs and receipts,
not inner op targets. And
LLP 0374's `producerIncarnationId` joins at the Acto boundary, never
in this context. One structure answers stale-frame fencing across `reset()`,
reconnect correlation, Acto attribution, and OQ11's future per-app
budget enforcement — but fencing is **not** recovery: the rejected-frame
ticket's answer is one **token-bound recovery transaction**. A typed request
binds a fresh recovery token to the rejected producer generation and rejected
`BatchId`; JS freezes incrementals and emits a complete root-set manifest plus
full snapshots for **every live root of that producer**, not only the root
named by the rejected frame. Native stages, without publishing: the full
replacement trees and tree-bound sidecars; copy-on-write deltas to the
producer-session allocation-generation table under `RecoveryAdopt`; and the
per-root observed-rejected, last-committed, replacement, and next-expected
batch watermarks (interpreted by the successor rule above — selected by the
§8 amendment, landed 2026-08-23).
Scratch preflight validates the complete root set, generation table, indexes,
sidecars, and watermarks under the same recovery token. One commit point then
atomically publishes all affected roots, table deltas, indexes/sidecars, and
watermarks; any missing root, stale/duplicate token, generation replacement,
second rejection, timeout, or presentation failure discards every staged
delta and preserves last-good pixels and live session state. The token is
acknowledged only after the replacement is accepted **and presented**; JS
resumes incrementals only after that presentation acknowledgement. This is
the full acknowledged state machine the ticket specifies, which is a
**Phase 2 gate** — the `ProtocolTreeReset` nuclear path is deleted only
after that machine is green, never on capability identity alone. Its
**atomicity scope is producer-isolated by default**: one producer's
rejected frame freezes and rebuilds *that producer's* roots — one
producer cannot freeze or snapshot another producer's roots, and
kernel-tree application and host sidecar adoption are separate
adoptions today. A cross-producer **host recovery coordinator** (with
prepare/freeze acknowledgements from every affected producer) is built
only where a shared host sidecar genuinely spans producers; the
default stays isolation because the coordinator is the more fragile
machine and most recoveries do not need it. The r14 atomicity classes
mixed two axes (existing graphics "drop whole" already *retains the
last complete scene*, so it was also last-good). Every extension-chunk
class therefore **declares two orthogonal properties plus its
machinery**: `treeCoupling` (tree-bound — adopted atomically with the
kernel tree commit — vs independent), `failureDisposition` (roll back
to last-good vs drop), the **adoption owner and point**, the
rollback-or-generation-swap mechanism, the resync rule, and the
presentation-ack outcome. **Recovery classes are sidecar families
keyed by the extension inventory, not hosts**, and the W0-A table
is **total over the inventory's live sidecar ops** (today: pager 35,
motion snapshot 36, motion command 37, list model 38, menu 39,
graphics publication 40, graphics teardown 41) — a row per op, or an
explicit deletion-before-activation row; a claimed-complete matrix
that omits a live family is a Week-0 red. The default class is
**tree-bound + rollback**; independent + last-good is opt-in per
row. Current assignments: **Motion is tree-bound** (the Apple commit
transaction already distinguishes tree-bound Motion from last-good
Graphics — `ExactNativeHostCommitTransaction.swift`); **pager and
menu are tree-bound** by the same evidence (both are applied and
rejected inside that Apple commit transaction; menu's "no retained
kernel menu state" makes its rollback trivial but does not make it
independent); **list model is tree-bound with staged last-good
chunks** (the kernel already stages `SetListModelChunk` last-good);
**Graphics is independent +
last-good** (LLP 0352's drop-whole *is* retain-last-complete-scene).
The **TUI host is not a class assignment and not a standing
exception**: it mutates host mirrors before kernel apply and can
reject Motion *after* `HostInterpositionFrame::apply` has committed
the kernel tree without rolling it back (`projection.rs`,
`host_interposition.rs`) — an ordering that **violates §2's
validate-then-apply guarantee** for tree-bound sidecars, so it is
recorded as a **host repair with a known-red injection row**
(quarantined the way Android is) — and **DEFERRED PAST THE WINDOW
(2026-08-20, LLP 0505 r3 row 9): the TUI lane is frozen for the
breaking window**, so this repair is thaw-work at the LLP 0506 exit,
its known-red row stays recorded but is **not a Phase-2 gate**, and
per 0506 D2 the frozen lane cannot hold the ABI hostage; declaring it
a "coupling fact" would let the injection suite green the broken
order. The Phase 2 failure-injection
suite exercises each declared assignment at each owner's adoption
point, not only the kernel tree's — assignments are proven by
injection, never by declaration. Incarnation mapping,
stated — **verbatim the WS-A rule, no second phrasing**: `reset()`
bumps the root incarnation; `ExecutionGeneration` bumps on producer
restart and coherent
full reload only, and an LLP 0500 HotRevision plan patch never bumps
it; reconnect presents the address components unchanged and receives a
full snapshot (`sameGenerationReconnectFullSnapshot` becomes "same
incarnation pair") — which is only satisfiable because WS-A's
**restore allocation class** replays that snapshot against the
session-owned generation table (no bumps) and re-attach **reissues**
the prior `ProducerId` on a matching `ExecutionGeneration`. Batch numbers are monotonic **per producer per
root** within the live admission scope Accepted LLP 0507 pins; no successor
key is inferred across reconnect/reset/restart until §8's amendment lands.
That minimum makes LLP 0331's producer-identified sequences and the web
host's gap check compose instead of colliding. The unvalidated `op_count` is
dropped; `frame_len` is checked. `PropValue::Handle` is **not** in this
break's closed list — resource references arrive later via the
extension envelope. The generated mask width is derived and may be
multi-word; nothing new squeezes into the saturated u64.

**App-ABI coordination (Exact Native) — one rule, independent dimensions.** The
EXWF break reaches beyond the JS→kernel stream: Rust Native artifacts
emit the same binary protocol under pinned schema digests
(`APP_ABI_SCHEMA_JCS_SHA256`, `SCHEMA_PACKAGE_SHA256`), and the
`exact-native-app-abi` / `exact-native-compatibility-tuple` boundaries
are owned by **Accepted LLP 0510's v2 package**; LLP 0333 is Superseded and
there is no live v0 package. Compatibility dimensions stay independent,
never one coupled bump: (1) the EXWF revision/schema digest; (2) the
separately admitted EXWF semantic epoch above; (3) Motion wire identity;
(4) Motion evaluator/artifact identity in Phase 4; and (5) the App-ABI
schema — whose
`operationProtocol` names the JCS capability-call protocol, not EXWF —
widens **only if its own schema changes**, and any such widening is an
explicit M5-STOP ruling. The live v2 tuple does not yet carry either EXWF
dimension and its `motion` object carries wire schema/layout identity only;
§8 `A-0510-COMPATIBILITY-DIMENSIONS` is therefore required before Exact
Native admission can claim these checks. Generator provenance is kept
factual: `schemaPackageSha256` is computed from the canonical schema-source
coverage, while the generator records its own `generatorSha256`; merely
regenerating an encoder does not refresh the schema-package pin. A changed
schema source followed by regeneration refreshes the pin without implying an
App-ABI widening unless the App-ABI schema itself changed. The Rust-producer
paths get EXWF encoders in the same motion as the TS encoder.

### WS-E — FFI reshape

Domain logic out of the FFI files (the SharedValue slab, gesture
controller, pager and navigation controllers — ~3,000 lines that aren't
FFI); the generated boundary from WS-B; one `ExactStatus { code, detail }`
with a generated error enum (the SharedValue verdict enum is the model);
typed opaque handle structs replacing ten `typedef void*`; per-kernel
injected callbacks (`Box<dyn TextMeasurer>`, one version) replacing
**every** process-global callback — the five text-measure ABI
generations plus the segment-preparation callback, the native-control
measure callback, and the module action/sync registry callbacks (whose
setters take no kernel handle today) — collected into one per-kernel
host-services object; any callback that provably must stay
process-global is enumerated with its multi-kernel routing-safety
argument, never defaulted; thread-affinity checked in Rust the
way `SharedValueSlab::is_writer_thread` already does; the Week-0
**kernel-scoped abort boundary** (a dedicated kernel/host build profile
or FFI-boundary abort — never a workspace-wide dev `panic="abort"`,
which would break the supervisor's `catch_unwind` fault boundaries); the
TLS out-param and the null-slice UB fixed. End state:
~60–80 operations — bulk state as columnar buffers, mutation through the
one command buffer, control plane as typed functions. The isolation gate
keeps two kernels alive concurrently with distinct host-services objects
and exercises text, prepared segments, native-control measurement, module
action, and module sync dispatch; every callback must reach only its owning
kernel/session. A remaining process-global callback cannot pass by being
"set once": its row must prove collision-free identity, routing, teardown,
and replacement behavior under that same two-kernel test.

### WS-F — Deletion sweep (compat debt, population zero)

Protocol v1; the text-measure callback chain; the frozen pager ABIs; the
dead **Rust** `MotionClockRegistry` in `kernel/src/motion.rs` — zero
production consumers; the similarly-named live Swift
`ExactMotionClockRegistry` is untouched (with the WS-B(4) phase-enum
carve-out, §7); the
dead pager (its constants move to f64 in the live one); v1 arena absorption
into `GestureArena` (navigation migrates — the arena is live production code
today, like the direct-setter path: `docs/kernel-refresh/w0a/DISCREPANCIES.md`
item 6); dead enum variants and the
digest-slot-only descriptor kind (the census-proven member is
`MotionDescriptorKind::Driver`; the break package must enumerate any
remainder — DISCREPANCIES.md item 7); benchmark batch setters; the dead
selection state API; **and the direct-setter write path** — mutation goes
through the command buffer only.

### WS-G — Crate split

The `exact-motion` extraction is a **parallel track that may start at
Phase 1** — it does not depend on the WS-A arena or the EXWF break, and
pulling it forward means RFC 0490's native M3 waits on a motion crate
plus the evaluator ruling, not on the whole tree rebuild. The layering
win needs only three crates, and Phase 4 completes the cut:
`exact-layout-core` (tree/layout/style/text/selection/semantics/island —
drops libc, windows-sys, and resvg; **sha2 ruling:** production
selection computes SHA-256 wire identities today, so the digest
*projection* moves behind the `exact-kernel-abi` boundary — selection
semantics stay in `exact-layout-core`, digest emission lives with the
export surface; if that extraction proves awkward the recorded fallback
is keeping sha2 and amending the dependency-closure claim),
`exact-kernel-abi` (the generated
boundary), and `exact-motion` (value graph + drivers + recognizers +
transport + navigation model — the *interactive-navigation* bottom half
(RFC 0100 / `interactive_navigation.rs`) only; the SPA matcher and
navigation commit/gate state machine live in RFC 0494's separate
`exact-router-core` crate, composing with `exact-motion` by receipts, so
gpu-service's motion closure never links a router). The split's exit gate is a **published
crate DAG that is complete, not three-crate shorthand**: it also names
the small **shared wire crate** (EXWF encoders both tiers use), the
**retained `exact-kernel` facade/staticlib** (the compatibility surface
existing consumers keep linking through the transition), and the
**interim homes** of the later extractions — `exact-svg`,
`exact-modules`, `exact-crash`, and `exact-list-core` live inside
`exact-layout-core`/the facade behind features until their extraction
triggers fire (`exact-svg` is a default kernel feature today; OQ6 still
owns the module registry's final seat). Every required and every
forbidden edge is stated and
machine-checked (gpu-service → exact-motion required, gpu-service →
exact-kernel forbidden; exact-aquifer → exact-kernel forbidden,
`exact-kernel-abi` at most; supervisor/worker → kernel forbidden
outside dev-deps) — never a prose list a later crate drifts past. The
DAG distinguishes **transition edges from final edges** (Windows and
TUI link `exact-kernel` directly today — a legal transition edge
through the facade era, not the end state), and "three-crate split" is
the headline, never the count — the published DAG is the authority.
`exact-list-core`, `exact-crash`,
`exact-modules`, and `exact-svg` extract later, when a second consumer
or the wasm size profile actually demands them; `terminal_text` moves to
the TUI host. One real consumer the r12 matrix missed is now first-class:
the **generated app-owned archive** — `exact new`'s packaged app crate
links `exact-kernel` directly as a staticlib/rlib dependency
(`packages/exact-cli/src/native_package.rs` writes the manifest and its
symbol audit checks kernel objects), so the split carries a
**package-freshness and placement gate at Phase 4**: the generated
manifest re-targets the post-split crates in the same rev, and a stale
generated archive is a red row, not a discovery. Dependency direction is a requirement, not hygiene: the
GPU service consumes `exact-motion` without depending on `exact-kernel`,
and the kernel never depends on `gpu/` (RFC 0490 M3/M5). **`exact-aquifer` joins the crate map** (author-relayed ask, RFC 0498
§8): the Aquifer cell engine — unified cell state machine,
identity/dedup per RFC 0498's **layered identity** (the declaration
key for HMR identity; the shipped LLP 0263/0373 four-position logical
query key plus the engine-stamped realm for runtime cache identity —
never a flat "declaration plus dependency tuple"), cache/staleness/
scheduling, and the RFC 0391 provider seam — with the same hygiene as
`exact-motion`: wasm-compilable from day one (no libc/windows-sys in
its closure; the web leg runs the same Rust engine in the LLP 0483
fixed runner, never a parallel TS implementation), and dependency
direction fixed — the plan runner / `contract-native` consumes
`exact-aquifer`; `exact-aquifer` never depends on `exact-kernel`
(`exact-kernel-abi` at most). `exact-motion`
is additionally **wasm-compilable from day one** (no libc/windows-sys in
its closure) — a crate-hygiene requirement so the web leg *can* run the
same Rust evaluator; when that build actually replaces LLP 0099's
shipped TS web evaluator is a parity-gated LLP 0099 amendment owned by
RFC 0492 (§7.1) under frozen LLP 0500 D2's dev-runs-the-production-runner
constraint, not a day-one deployment claim. The split turns the
worst layering inversion (text_segments reaching FFI statics) into a
compile error. Kernel-side SVG rasterization is frozen pending 0490 OQ1;
if Vello wins, it retires into the paint tier. The dependency-closure
reduction (`exact-layout-core` without libc/windows-sys/resvg/sha2)
directly serves the `native-parity-wasm` size profile the workspace
already governs (LLP 0331 R-9.4-2), and the split is validated against
its real consumers at their 0331 layers — stated as a consumer matrix:
**platform hosts** (Windows, TUI; Apple via FFI) consume
`exact-layout-core`; **supervisors and workers consume transport/
shared-wire authorities only** (the supervisor has no kernel dependency
and the worker links it only for tests — they stay that way); app-tier
crates (`exact-native-ui`, `exact-native-runtime`) link **neither** —
the generated app archive's direct kernel link (above) is CLI-owned
*packaging* of the host tier into one binary, not app-authored linkage,
and it is the matrix row the Phase 4 freshness gate covers.
EXNODE is **one schema, two projections**: borrowed typed columns for
in-process Rust hosts (preserving the layout-export contract's
deliberate typed-row consumption on Windows) and a binary envelope for
FFI/transport consumers, parity-gated against shared golden fixtures.
`HostInterpositionFrame` is **split, not moved wholesale** — today it
imports `Kernel` plus the motion/graphics/menu sidecars and applies
directly, so wholesale relocation into `exact-layout-core` would
cycle: framing/decode lives with the shared wire/ABI surface, tree
mutation with `exact-layout-core`, and extension application with each
extension's owner. EXWF encoders for Rust producers generate into a
small shared wire crate both tiers may use.

### WS-H — Per-frame work proportional to change

Presence sets/bits make the portal and inline-embed frame walks no-ops on
trees without them (the existing `svg_raster_node_ids` proves the pattern);
the five dirty-tracking mechanisms unify into per-node dirty flags plus one
published epoch — which is also the seed of damage tracking for 0490
promoted-subtree repaints; the double frame decode and the `BatchValidator`
shadow mirror are replaced per OQ3 — **closed 2026-08-25 for staged
generation snapshots** (near-free on the WS-A arena, and they double as
Acto's read isolation: agents read generation G while G+1 applies; the
undo journal stays the named fallback). The chosen engine lands
behind a rejection-atomicity gate — a generated failure-injection suite
that aborts after every mutation class and asserts canonical state,
Taffy reconstruction, indexes, receipts, **and the producer-session
allocation-generation table plus every observed/committed/replacement
watermark** are untouched — before the
validator and the `ProtocolTreeReset` nuclear path are deleted (never
earlier than WS-D's capability-context resync answer). The flight
recorder's persisted format **is the `SemanticFrame` IR** — decided
here, resolving r12's "no third encoding" sentence: SemanticFrame is an
owned, versioned migration/replay format because today's borrowed
`OpEvent` cannot represent the new semantics and raw EXWF bytes cannot
survive the very revisions the recorder must replay across; EXWF and
EXNODE remain the wire and export encodings it correlates with (three
formats, three jobs — not two formats for one job), with
nondeterministic
inputs — time, measure results, module effects — captured at their
ingress points rather than reconstructed. The raw format also carries the
recovery semantics replay needs explicitly: identity checkpoints and
identity-event transitions (including `ApplyMode`), per-root observed,
committed, replacement, and next-expected batch watermarks, the recovery
token/generation/root-set manifest, staged allocation-generation deltas, and
the terminal recovery disposition including presentation acknowledgement.
None may be inferred from a later tree snapshot. SemanticFrame is
**content-bearing and therefore governed, not an ungoverned
observability side door**: **raw SemanticFrame tapes are
dev/test/migration-oracle artifacts only**. Production grows no second
flight recorder — it **projects one-way into the existing
`observability-crash-capsule-v1` allowlist** (LLP 0362 §8, Review, with its
closed schema); replacing or widening that
allowlist is an explicit 0362/schema amendment, never a side effect of
this program. **Projection is literal closed-schema selection:** it emits
only fields the current crash-capsule schema names and drops every other
SemanticFrame field, including replay-only identity/watermark/recovery values
and raw commitments for which that schema has no slot. A dropped field does
not survive by implication or by being encoded into an open metadata map.
Privacy transformation runs only after that allowlist selection. The
projection inherits the replay privacy classes
(`public | digest | vault`, `runtime/replay-value.ts`) with one
correction folded in rather than adopted blind: **non-public values
must not expose unkeyed content digests** — the current projection
(`projectReplayPrivacyV1`) emits an unkeyed SHA-256 `digest` beside
the keyed commitment in
`digest`/`vault` records; the low-entropy **confirmation-oracle
rejection is LLP 0499's** (0362 owns the capsule allowlist this
projects into, not that sentence), and this program adopts it: only
keyed commitments may cross into any
persisted production artifact. The format carries explicit byte/depth
caps, truncation semantics, and a retention statement. The W0-A registration is not a hollow name: its
first deliverable is the schema itself — artifact path, version field,
equality definition, and the enumerated captured-ingress fields; motion snapshot
validation memoizes behind a `Validated<T>` newtype — and no further:
if the Motion program adopts plan-compiled motion (the graph as a flat-
plan segment validated at compile time, jointly ruled with LLP 0480/
0485), the runtime motion-validation tier is on a deletion trajectory,
so it gets memoization, not investment; the 48 KB per-prepare
buffer, per-measure run Vec, and per-sample gesture allocations get scratch
buffers; selection and a11y caches become keyed and multi-slot with their
own generation counters.

**The idealized-Motion requirement (named customer; the number lives
with the customer).**
Everything above serves commit-rate efficiency; animated layout — the
one Motion frontier still unresolved after RFC 0490 — needs frame-rate
relayout. Shared-element/FLIP transitions, reorder animation,
container-size motion, and keyboard avoidance all reduce to: re-solve
layout for a k-node dirty set, k ≪ n, inside a display-rate budget.
The division of ownership is now exact, ending the round-2 tension
where a kernel-freeze exit was hostage to a layout-performance target
this program cannot guarantee. **0491's own exit obligations are two:**
(1) incremental relayout is **result-equal to full relayout** on the
0486/0487 corpora, and (2) the **dirty-set instrument exists** —
per-binding affected-set sizes, changed-geometry receipts, and the
patch-application→receipt-publication timing endpoints are exposed and
measured. **The ≤1 ms / ≤64-node budget is RFC 0492's claim**, made
achievable by this substrate and adjudicated there: which animations
may claim the ≤64 budget (reorder and keyboard avoidance are not
obviously k ≪ n) is 0492's ruling, and a Phase-5 miss of the target on
a case 0492 has not yet classified is a 0492 input, never a 0491
freeze blocker. The target's Taffy assumption is still
named, not implied: vendored Taffy is not an independent 64-node
solver — flex/grid constraint coupling grows the affected set — so
"dirty-scoped" means Taffy's existing dirty walk plus WS-A
constant-factor wins, measured; if that misses the budget, the
additional skip-work is scheduled work with 0492 as the customer, and
"we made full relayout faster" never satisfies the claim. The budget
stays **claimable, not just benchmarked**: the dirty-set machinery
exposes per-binding affected-set sizes so the Motion program can make
"this gesture's layout dirty set is ≤64 nodes" a declared,
contract-block-style claim checked like any other — the kernel supplies
the measurement; the claim discipline lives in RFC 0492. FLIP transitions
additionally consume WS-C commit receipts as their before/after
geometry source — no new mechanism needed there. Which *tier* delivers
an animated layout value is OQ10.

### WS-I — Semantic corrections

Subtree destroy — with its activation ordered against the break:
today `destroy_view` detaches and preserves descendants, tests pin
that, and the direct web/TUI consumers remove only the named node, so
**`DestroyView`'s wire semantics stay single-node until the Phase-2
EXWF revision activates**; recursive destruction before that point is
an internal arena-reclamation helper only, never a wire meaning
change (a Phase-1 wire flip would silently change behavior under an
unchanged frame revision); generational IDs surviving reset (both via
WS-A); split
overflow axes; `Percent` representation standardized **with the wire
convention chosen, not gestured at**: the wire and generated style
rows carry **authored percentage points (0–100)** — the web-standard
serialization the TS encoder already emits and the web host already
renders — and the fractional (0–1) conversion Taffy needs happens
exactly once, in the generated Taffy mapping (every current
conversion already divides by 100 — this is unification, not a
layout bug); `Auto` is **admitted per generated style row** (only
rows whose CSS grammar admits `auto` carry the discriminant), and an
`Auto` on a non-admitting row is a **typed decode rejection** — the
`Auto → 0.0` silent conversion loss is removed, never re-spelled as
a default; real grid track lists as a **closed portable track grammar amending the
layout-profile authority** (a joint 0486-owned amendment — the
profile's current equal-track-count model is deliberate, so the grammar
extends it with defined degradation rather than contradicting it); one spec'd `rubber_band()` (LLP 0099's curve) with a spring
handoff replacing the decay clamp-teleport; typed `KernelError` variants
replacing `LayoutError(String)`; the hop-cap cycle guards (the
`hops > 64` ancestor walk, the node-count-bounded traversals, and the
island/spec bounds) consolidated — but **not deleted on acyclicity
alone**: `set_children`
already rejects cycles, and the 64/128 caps also bound recursion depth
and valid-tree pathology, so the replacement is one enforced
`set_children` invariant **plus** either iterative/node-count-bounded
structural traversals or one reviewed admission-time maximum depth; a
hard error (not a per-measure
`eprintln!`) for an unregistered text measurer; the four-way text-content
walk unified into one structural `text_fragments()` IR — one traversal
with per-consumer projections (measurement, selection, semantics,
islands each keep their own inclusion/boundary rules over the shared
structure; one flattened universal result would be wrong for at least
one of them).

**Kernel box layout is direction-blind; WS-I owns the ruling, precisely
scoped.** `StyleProps.direction`/`writing_mode` never reach
`to_taffy_style`, so `direction: rtl` cannot mirror a flex row — but the
same field is already live for *text*: it participates in
`text_measure_fields_eq`, the Windows host applies it to DirectWrite,
and the Apple host applies paragraph direction. Logical start/end
properties also already exist, resolved to physical edges in TS
normalization using direction and writing mode. The correction is
therefore not "make direction exist" but **choose one owner for
logical-edge resolution and make box layout consistent with the text
behavior that already ships**. Ruling: **exhaustive direction
lowering in the WS-B generator, specified as a generated truth table**
over (flex-direction, wrap, direction, writing-mode) that distinguishes
logical `Start`/`End` from Taffy's already-flex-relative
`FlexStart`/`FlexEnd` — naive main-axis toggling *plus* alignment
remapping double-reverses, so the table is the authority and each row
carries a fixture; `wrap-reverse` cross-axis
rows included; solver-native direction is rejected for v1. **The
four-input table's exhaustiveness claim is flex-scoped** — grid
inline-axis direction additionally depends on auto-flow, track
definitions/counts, and item placement, and vendored Taffy computes
grid layout unreversed — so RTL grid gets its own **grid-direction
transform** specified over those inputs with its own fixture rows; until that transform's rows are green, RTL grid is an
**explicit declared degradation** (LTR-resolved, flagged), never a
silent claim of the flex table. This choice
**closes 0486 §5.1's widen-or-translate question as the
translate-via-table branch and is recorded as a joint 0486
amendment** — Phase 2 does not silently foreclose widen; 0486
co-signs the table. The TS normalization
resolver retires in the same rev (two live resolvers would corrupt
edges): until the EXWF activation it remains the sole owner; at activation
producers carry authored logical and physical edges without pre-resolution,
and the generated kernel table becomes the sole owner. **Its precedence is
the current TS precedence, frozen as fixture rows:** shorthand first,
logical inline edges next, explicit physical edges last/highest when both
target the same physical side. This includes the tested
vertical-writing-mode inline-edge mapping, which the generator preserves
(the pinned normalize tests move to generator fixtures; no silent
regression). Physical properties remain authorable (0486 owns the
authoring-default question); existing text/presenter direction behavior is
unchanged. Gated by 0486 parity fixtures with direction rows including
row-reverse composition and logical/physical collisions.
Vertical *layout* writing modes remain deferred — declared, inert,
documented.

## 4. What this deletes or simplifies

~4–5K lines of verified-dead compatibility surface (WS-F); ~7,500
mechanically-derivable hand-written lines across four languages (WS-B);
~400 lines of duplicated sidecar envelope code (WS-D); the 200-line shadow
validator and the reset-the-world failure path (WS-H); the ≈2.8K-line
hand-written C header as a review obligation (WS-B); the three patch
opcodes and their JS special-casing (WS-D); the "prototype for protocol
benchmarking" identity, replaced by an honest architecture line: a
copied-in, borrow-parsed binary command stream with transactional apply and
columnar binary exports — "shared memory" is retired from the description
until a design needs it.

## 5. Sequencing

| Phase | Content | Gate |
| --- | --- | --- |
| **W0-A — non-mutating preparation** | Read-only inventories and disabled/draft artifacts only: WS-B parity/differential-fuzz harnesses; the fingerprinted paired WS-A baseline; the current-kernel 0487 corpus harness; the `SemanticFrame` schema (including identity/recovery/watermark fields); the breaking-window ledger row schema; the proposed ten-row identity-carrier matrix; recovery-class assignments; golden schema-digest fixtures; the canonical consumer census and exact-once census↔ledger join; and the canonical semantic-package preimage for the proposed `exwfSemanticEpoch`. Generated codecs may exist disabled for shadow replay, but W0-A changes no runtime/build behavior, deletes no live path, activates no revision/epoch, creates no outage/known-red, and changes no abort boundary. | W0-A artifacts and read-only checks green; baseline limits/targets and wake→presentation disposition precommitted. RFC 0496 is not presumed to have a machine-readable closed core: §8 records the required membership amendment. |
| **W0-B — mutating preparation** | Only after W0-A: make trailing bytes fail loudly; land WS-F prerequisite-free deletions and presence-set early-outs; install the kernel-scoped debug abort boundary; fix the null-slice UB; and gate off the Kotlin decoder so Android is an explicit bounded known-red until the JNI typed-operation projection lands. No patching constants in code this deletes. | **Blocked until RFC 0496 V-A and V-B have met their exits and §8 `A-0496-CORE-MEMBERSHIP` has landed.** Every behavior/build mutation runs under typed-red/quarantine semantics; checks that the accepted membership rule places in the non-quarantinable core cannot be suppressed. Previously ignored malformation may now fail loudly, but no change is silent. |
| **Phase 1** | The early `exact-motion` track lands with a registered gate, and the crate's Phase 1 contents are named: the **SharedValue slab, the closed-form evaluator math, and the tick-source-agnostic phase-consumer API** — the main-owned motion domain — so "`gpu-service` compiles against `exact-motion` without `exact-kernel`" means the real graph, never a Cargo-graph stub (this, plus the 0492 evaluator ruling, is what RFC 0490's native M3 waits on; WS-E's Phase 3 is the remaining C-ABI/opaque-handle reshape, and Phase 4 deletes the Swift mirror under §7.1's evidence bar). The target Rust API is defined first (a facade over the new arena; white-box invariant tests stay crate-private test support — the durable rlib is not bloated for tests); behavior tests migrate onto it; then WS-A + the WS-H dirty/epoch substrate (receipts depend on it) | W0-B has already established RFC 0496 V-A/V-B and the owner-approved core-membership rule; Phase 1 may not bypass or weaken them; **every applicable** 0487 corpus fixture through the production arena under the corpus's semantic equality — where "applicable" means each fixture's *already-chosen* template is realized through the new arena and judged under 0487's semantic equality (it does not mean implementing 0487's missing production choice operator), per an explicit executor coverage matrix (today 2/20 run `kernel-static` and 18 run the choice prototype — the Week-0 harness over the *current* kernel is the entry requirement, the WS-A production-arena adapter is **Phase 1's first slice**, the retargeted full corpus is a Phase-1 **exit** gate, and **enumerating the 18 as prototype-only is not a substitute for the adapter**; only genuinely host-policy-only observations may stay enumerated-with-reasons); 0486 profile/parity checks green; paired comparison against the fingerprinted Week-0 baseline in one interleaved same-machine session: 1K-node layout ≤ baseline, with the Week-0 precommitted limits on every other baseline axis carried through each later phase |
| **Phase 2** | WS-B tables + WS-D EXWF break as **one** rev, governed by a **machine-readable breaking-window ledger** (every producer, consumer, artifact, epoch dimension, owner, fixture, and migration/rejection rule — replacing this prose list as the authority), with the closed wire-visible input list decided in-rev: recursive `DestroyView` activation, overflowX/Y split, grid track lists (whose portable-grammar **0486 layout-profile amendment is a Phase 2 prerequisite**, landing in both documents before the break and **stating the v1 portable subset** — what native must implement vs what stays defined-degradation; the grid and direction changes additionally take **0486's WS1 dual-engine instrument and the specific WS2 repairs that let the instrument observe grid and direction — the profile rows, the fail-closed completeness join, and the start/end ruling — as Phase 2 prerequisites**, never advisory later evidence and never the whole WS2 bag (style= scoping, the 0084 fold, the gutter pin, and Band-floor ratification do not gate this program) — wire-visible layout semantics do not ship past a parity instrument that cannot yet see them), logical-edge representation + resolver ownership (WS-I), direction-live box layout, `Percent`/`Auto` conventions, capability-context frame identity and `BatchId` gap/reordering admission, RFC 0493's typed input props + event-payload decisions (freeze-window riders), and WS-B's reserved `token-ref` + dependency-domain discriminants; WS-C binary events; **EXWF compatibility gets independent carriers** — `{exwfFrameRevision, exwfSchemaDigest}` for schema bytes plus separately admitted `exwfSemanticEpoch` for companion semantics, added to EXFF and, after the §8 LLP 0510 amendment, Exact Native v2 artifact admission, with the downgrade edge stated: existing EXFF readers parse permissive JSON headers and would silently ignore unknown fields, so both carriers land as **required fields under a bumped EXFF container version** — an old reader refuses the new container version rather than skipping the fence, and a new reader refuses a container that lacks the tuple. The digest has a **canonical preimage, not a stated intention**: `exwfSchemaDigest` is **SHA-256, domain-separated**, and computed over the one complete generated schema package — **closed over every schema the frame carries but explicitly excluding the identity matrix, recovery assignments, ledger, census, and BatchId successor rules** (those are covered by `exwfSemanticEpoch`): the frame header and capability-context schema themselves, receipt/acknowledgement headers, op headers, typed prop tables, style layouts and enums, extension-chunk envelope schemas, event payload-family schemas, patch/clear rules, bounds, alignment, and POD/endianness rules — with component digests (opcode table, event payload families) enumerated *inside* the package while admission checks the package digest; the canonical byte projection is defined by the generator (one serialization, no self-reference: digest-bearing fields are excluded from their own preimage); **externally referenced authorities that carry decoder semantics — the 0486 layout-profile `defaultRef` targets and the non-layout default authority — enter the preimage digest-pinned**, so a default change cannot move under an unchanged schema digest; after OQ7 freezes the EXWF surface, changing any decoder-visible `defaultRef` target is therefore an EXWF ABI change that bumps the revision/digest (unless a later accepted contract introduces an independently negotiated default-authority dimension); `exwfFrameRevision` bootstraps at 1 with this break and bumps with every package change (always in lockstep with the digest); golden preimage/digest fixtures pin the canonicalization; this digest **is** WS-D's capability-context schema digest (one digest, two carriers, never two spellings); the `exwf` prefix is deliberate so no field ever sits ambiguously beside 0485's `opcodeTableDigest`; and adding the EXWF schema and semantic fields to Exact Native v2 artifact admission requires §8's owner-approved **compatibility-tuple major bump** (the tuple's evolution policy makes any shape change major) with no App-ABI widening: 0485's `opcodeTableDigest` identifies the flat-plan expression VM and the App-ABI `protocolVersion` identifies the JCS capability-call protocol, and **neither is overloaded to carry EXWF identity** — the ledger carries the owed 0485/0307/0510 amendment rows for that carrier plus the explicit **plan/baked-frame regeneration-or-rejection rule across EXWF revisions**; the RFC 0495 A-B receipt/evidence schema (or retained compatibility trace) lands **before** protocol-v1/trace deletion; direct-write consumers migrate (including the Swift standalone resize/font-remeasure path) **before** the gate; a **producer-by-host transactionality matrix** with injected failures gates the recovery story per host — Android stays quarantined (known-red) until its transactional typed-operation row passes; mechanics: generated EXWF-revision codecs and host adapters land disabled, tapes replay through them in shadow, and the **compatibility vector is per-dimension epochs, not one flip**: EXWF carries independent schema revision/digest and semantic-epoch dimensions and activates them together at Phase 2; event schemas with the schema dimension; EXNODE/feed and the C ABI activate at Phase 3; rlib and Motion at Phase 4 — attach/replay reports the exact mismatched dimension. App-ABI coordination follows WS-D's one rule (independent dimensions; no automatic coupled bump). The consumer inventory carries an explicitly **unverified Ibex row** (the submodule's dispatch ABI is verified against the real tree, never assumed from generic codec wording), and that row's parity bar is **execution-entry semantics, not payload bytes alone**: named real-input and synthetic-agent entry tests — `ExactRuntimeDriveGuard`, generation validation, host-session entry, principal reset, and the `setTimeout(0)`-hop A/B — per the closed 2026-08-05 no-user-carrier input-dispatch incident and 0495's fail-closed principal rule | one write path; every producer/consumer codec regenerated (Rust, TS encoder, web host, Android per the OQ4 ruling, Rust-producer App-ABI); `protocol-abi` boundary flips to `active`; cross-language byte-parity over all style bits; ledger rows green under the W0-A row schema — with **non-waivable rows** (identity, active producer codecs, entry security, transactionality) **mapped by the owner-approved §8 `A-0496-CORE-MEMBERSHIP` rule** so neither quarantine nor explicit suppression greens whichever of them the amended authority admits to the core, and every other waiver carrying owner, issue, exact fixture/consumer, maximum deviation, expiry, and removal condition |
| **Phase 3** | WS-E + WS-C EXNODE/feed projection + commit receipts | sync cost = **one schema publication per structural sync for every host, with the projection selected by boundary** per WS-G's two-projection model — the sectioned binary envelope for Apple/Swift and any FFI or out-of-process transport boundary; borrowed typed columns for in-process Rust hosts, including Windows and TUI, preserving their existing typed-row consumption; Exact Native app/producer crates are not EXNODE consumers at all (they emit host operations behind the App-ABI) — inventory-checked, not iOS-only, **plus an end-to-end encode/decode/adopt latency budget** (crossing count alone is not the cost — the feed authority's ~0.02 ms/node and sustained-8ms/450-node escalation trigger apply, and a miss invokes the existing diff/allowlist escalation). **Android is exempt from the EXNODE gate**: OQ4 is closed as JNI over the shared Rust decoder projecting typed operations into the existing Kotlin retained-tree applier, whose gate is a green transactional typed-operation interface; kernel-on-Android and EXNODE-on-Android belong to LLP 0287's program; one status convention; generated header replaces hand-written; LLP 0297 OQ8 (model vs presented geometry) is closed before the receipt/EXNODE schemas freeze |
| **Phase 4** | WS-G three-crate split; WS-F motion consolidation; §7.1 executed (Swift graph mirror deleted); Motion publishes **two non-aliasing compatibility dimensions**: the existing Exact Native `motion` object remains wire schema/layout identity, while a required `motionEvaluatorEpoch` identifies evaluator semantics/artifact ABI independently | the WS-G crate DAG published and machine-checked (every required and forbidden edge, the wire crate and facade included); gpu-service → exact-motion compiles without exact-kernel — **as registered wasm-build + forbidden-closure checks for `exact-motion` and `exact-layout-core`**, not Cargo-graph inspection (today's `native-parity-wasm` builds Contract Native against direct Taffy, which proves neither); **evaluator-only skew is detected at artifact admission** by comparing the artifact-carried `motionEvaluatorEpoch` with the host's locally generated epoch (canonical evaluator sources + deterministic-math profile + feature set + exported evaluator ABI); this check fails even when every Motion wire schema/layout byte is unchanged, and neither field may be derived from or substituted for the other; §8's 0492/0510 amendment ask must land first; **transition-only Windows/TUI facade edges are gone before the Phase-5 rlib freeze**; the generated app archive's package-freshness/placement gate green **across manifest, generated source, and symbol-attribution logic** (the CLI-written manifest re-targets the post-split crates); one arena, one driver hierarchy; Swift-evaluator deletion passes the §7.1 behavioral parity evidence (trace replay, settle outcomes, arena receipts); `exact-aquifer` rides this gate for **crate hygiene only** — DAG edges honored, wasm-day-one compilability — while RFC 0498's AQ-B product exits (cell-corpus parity, plan-runner consumption, the zero-JS `native-total` data route) complete on 0498's own clock and never block the kernel split |
| **Phase 5** | Program exit: remaining WS-H (validation memoization, cache multi-slotting), WS-I long tail, the OQ7 freeze statement enumerating frozen surfaces (EXWF frame rev, generated C ABI, rlib API, EXNODE/feed schema) vs negotiated contracts | freeze statement ratified by the author; registered **pass/fail** exit checks green: (1) the forced-full-relayout **result-equality differential** on the 0486/0487 corpora, (2) the **dirty-set instrument** existing and measured (per-binding affected-set sizes, changed-geometry receipts, patch-application→receipt-publication endpoints), (3) the W0-A precommitted **regression limits on every baseline axis and positive targets on every non-maintenance axis** (matching W0-A's declaration — a maintenance-only axis still carries its regression limit), and (4) one **integrated device measurement** — wake → layout → receipt → presentation under representative JS/GC contention on the pinned hardware class — beside the isolated compute gate, judged against the disposition W0-A precommitted for it (non-regression envelope + sampling protocol, or explicit trend-only status — never an unpredicated pass/fail line). The sparse-dirty relayout benchmark still runs fully executable as registered (reproducible 1K-node generator with pinned fixture hash, named dirty-set cases including a reorder case and a keyboard-avoidance case, p95 over ≥1,000 measured patches after ≥100 warmups, the timing endpoints and quiet-host environment above) — but it **reports against RFC 0492's ≤1 ms/≤64 target rather than gating 0491's exit on it**: whether reorder and keyboard avoidance are ≤64-node cases, and whether a target miss blocks anything, are 0492's rulings (`kernel-bench` itself stays informational — the exit gates are registered checks) |

> **Phase status (2026-08-23).** **W0-A landed** (@8b9938b84): the
> hand-verified census pack and draft schemas in `docs/kernel-refresh/w0a/`
> (its README states the generated-authority boundary rule), the
> `w0a_layout_pins` integration pins (`Node` 576 B, `StylePropsFFI` 408 B),
> and `DISCREPANCIES.md` — ten verified spec-vs-tree deltas; this
> revision's factual corrections cite it. **W0-B executed** the same day,
> its gate met (§8 in-force note; RFC 0496 V-A/V-B landed @ee26ff49f):
> trailing style bytes fail loudly (`ProtocolError`; the TS encoder emits
> exact-length payloads and the web decoder already refused trailing
> bytes, so nothing green went red); the WS-F prerequisite-free deletions
> landed — frozen pager ABI v1, Rust `MotionClockRegistry`, legacy
> `MotionPager`, benchmark batch setters, dead selection state API, net
> −869 lines, no constants patched — while protocol v1 (0495 A-B precedes
> its deletion), the text-measure chain, `InteractionArena`, the
> direct-setter path, and the digest-bearing `MotionDescriptorKind::Driver`
> stay for their owning phases; the portal and inline-embed frame walks
> take presence-set early-outs (WS-H pattern, conservative: one repair
> pass after the last portal leaves); the kernel-scoped debug abort
> boundary is installed as the pin-tested `kernel-abort-dev` profile
> (never workspace-wide); the null-slice UB is fixed in the FFI slice
> helpers with every raw-slice site audited; and the Kotlin decoder is
> gated off — Android is an explicit bounded known-red
> (`android-kotlin-decoder-gate`,
> `issues/20260823-android-kotlin-decoder-gated-known-red.md`) until the
> JNI typed-operation projection lands. Three W0-A items its non-mutating
> rule deferred landed with W0-B: the differential fuzz harness (the
> pre-existing `kernel/fuzz/` no-panic crate gains `parser_differential`
> and `style_decoder` targets over a shared doc-hidden core;
> `kernel-w0b-differential-fuzz-smoke` is the stable-toolchain gate — 4,000
> seeded cases, no divergence found), the legacy-oracle content-addressed freeze
> (`docs/kernel-refresh/w0a/legacy-oracle-freeze.json`, pinning the
> post-W0-B `kernel/` tree object), and the pack's registration
> (`kernel-refresh-w0a-pack`; profile `kernel-refresh-w0` groups the
> Week-0 checks). The 0487 corpus harness over the current kernel **landed**
> as `choice-corpus-kernel-harness` and the manual
> `choice-corpus-kernel-differential`, with the explicit
> `kernel-harness-coverage.json` matrix, a committed freeze-pinned oracle
> tape, and the `LayoutEngine` seam that Phase 1's arena adapter implements.
> Two current-kernel facts the tape now pins: the production
> `Kernel::compute_layout` accepts only definite available space (no typed
> max-content offer), and it lays an auto-sized root out shrink-to-fit under
> that space — so family 9's carrier metric transparency and empty 0×0 ARE
> judged on the production kernel (through the D2 root's own fit-content
> size, an M3-domain used-value observation), while the carrier baseline is
> not exposed by the layout-tree export and stays enumerated. A typed
> intrinsic offer plus a baseline read are named Phase 1 slice-1 inputs
> (`issues/20260823-kernel-intrinsic-offer-api-for-0487-harness.md`), not a
> Phase 1 start. **Still-open W0-A
> debt owed before Phase 1 entry:** the fingerprinted performance baseline
> with precommitted limits/targets and the wake→presentation disposition,
> and the three pinned tape apps (the 88-bit style-order fixtures landed
> 2026-08-26 — see the last phase-status note). OQ3 stays
> open for Phase 1; W0-B pre-decided nothing there.

> **Phase status update (2026-08-25).** **OQ3 is closed**: staged
> generation snapshots are the selected WS-H transaction engine (Charlie
> Cheever, 2026-08-25, relayed via orchestration session exact-9e,
> decision register item 12; the undo journal stays the named fallback) —
> unblocking the arena transaction engine, the WS-H rejection-atomicity
> gate, and the `BatchValidator`/`ProtocolTreeReset` deletion path for
> the kernel Phase 1 lanes (the deletions still wait for WS-D's
> capability-context resync answer). The **fingerprinted performance
> baseline landed** the same day, discharging that W0-A debt item
> structurally: the capture harness is `kernel/benches/w0a_baseline.rs`,
> the pinned capture is
> `docs/kernel-refresh/w0a/performance-baseline.capture.json` (Bones —
> the pinned box per the same ruling — with machine, toolchain, source
> tree objects, the legacy-oracle freeze-pin join, fixture hash, and the
> session A/A noise floor recorded), and the per-axis regression limits
> ("no worse than baseline beyond the recorded noise floor" on **every**
> baseline axis), the positive targets on **layout and allocation only**,
> and the **trend-only** wake→presentation device disposition (envelope
> revisited at Phase 5 only if variance proves tight) are drafted in
> `docs/kernel-refresh/w0a/performance-precommitment.md` —
> drafted that day as **DRAFT-AWAITING-CHARLIE-SIGNATURE** — the structure
> and derivation rule precommitted per the 2026-08-25 ruling, the specific
> numbers converting on his signature, never by self-ratification.
> **They were signed as drafted on 2026-08-26** (see the next note); this
> paragraph records the drafting event, not a still-open ask. The registered
> `kernel-w0a-performance-baseline` check validates the capture, the
> derivations, and the freeze-pin join. The measured `kernel/src` had
> drifted additively from the W0-B freeze pin (standalone `arena.rs`,
> motion clock/math modules, the SemanticFrame codec, the generated
> PropId table, test-only decoder additions); the capture records both
> tree objects, and the measured layout/mutation/export path is unchanged
> from the pin. **Remaining W0-A debt before Phase 1 entry (as of the
> signature — see the next note):** the three pinned tape apps and the
> 88-bit style-order fixtures. (The style-order half landed 2026-08-26 —
> see the last phase-status note.)

> **Phase status update (2026-08-25, later).** **The WS-H transaction
> engine landed**: staged generation snapshots on the WS-A arena
> (`kernel/src/arena/txn.rs` — sparse copy-on-touch overlay building G+1
> beside G, atomic phased commit with Taffy-reconstruction recovery
> instead of reset-the-world, `&NodeArena` staging as the Acto
> read-isolation property, one published arena epoch over authored state,
> and the `TxnSession` producer-session sidecar carrying per-root
> observed/committed/replacement/next-expected watermarks, the
> allocation-generation table, and receipts). It landed **behind both
> mandated gates**, registered as `kernel-wsh-rejection-atomicity` (the
> generated failure-injection suite: abort after every mutation class and
> every Taffy commit failpoint, asserting canonical state, immediate
> observable Taffy reconstruction, indexes, receipts, and the
> producer-session allocation-generation table plus every watermark
> untouched, against populated tables) and
> `kernel-wsh-batchvalidator-equivalence` (the engine runs BESIDE
> `BatchValidator`/`ProtocolTreeReset`, differential verdict-class and
> failing-op-index parity plus normalized structural parity over the
> shared vocabulary, with the thesis-5 destroy-subtree delta pinned as an
> expected divergence). Non-regression evidence: interleaved base/branch
> `w0a_baseline` captures on Bones — every allocation axis bit-identical
> (including allocation-mutation-styles-1k-calls = 0 and the 576-byte
> `Node` pin) and timing deltas within the observed inter-capture drift.
> The validator and reset path are NOT deleted; that still waits on this
> gate holding plus WS-D's capability-context resync answer per §"WS-H".

> **Phase status update (2026-08-25, WS-G early track).** **The early
> `exact-motion` extraction landed.** The crate exists at `motion/`
> (workspace member `exact-motion`) and carries the real main-owned
> motion domain, not a Cargo-graph stub: the **SharedValue slab**
> (`shared_value` — the LLP 0099 M1/M2 segmented registry, moved out of
> `ffi.rs`; the validating C ABI stays in `exact-kernel`), the
> **closed-form evaluator math and driver table** (`motion` —
> spring/timing/decay closed forms, `AnimationDriver`,
> `MotionDriverTable`), the pinned **deterministic-math profile**
> (`motion_math`, provenance re-pointed), the **tick-source-agnostic
> phase-consumer API** (`MotionTickPhase`/`MOTION_TICK_ORDER` generated
> into the crate; the fixed-step `motion_clock` virtual clock), the
> **recognizers and arena** (`motion_gesture`) plus the **gesture-graph
> controller** (`motion_gesture_graph`, the pure section extracted from
> `motion_gesture_ffi.rs`), **Motion's typed transport**
> (`motion_transport`, the `DecisionCell` family included), and the
> **RFC 0100 interactive-navigation bottom half**
> (`interactive_navigation`). `exact-kernel` is the retained WS-G
> transition facade: same-named modules re-export the crate so every
> existing `exact_kernel::motion*` path and all FFI adapters keep
> compiling; visibility promotions for the adapter are documented
> in-source, and the test-only promotions serving the retained kernel
> white-box tests are `#[doc(hidden)]` transition debt with a named
> Phase-4 discharge
> (`issues/20260825-wsg-facade-era-test-visibility-debt.md` — the
> durable rlib is not bloated for tests). Registered gates: `kernel-wsg-motion-crate` (fmt + clippy +
> the crate's own suite — the real-graph gate),
> `kernel-wsg-motion-wasm-closure` (wasm32-unknown-unknown build plus
> the machine-checked forbidden closure — no libc/windows-sys/
> exact-kernel), and `kernel-wsg-crate-dag` (the published DAG at
> `docs/kernel-refresh/wsg/crate-dag.json`, required/forbidden edges
> machine-checked in the governance profile; Phase 4 still owes the
> complete post-split DAG as its exit). Non-regression evidence:
> interleaved base/branch `w0a_baseline` captures on Bones (A/B/A/B,
> clean per-capture builds) — every allocation axis and every
> bytes-per-node pin bit-identical across all four captures (the
> extraction touches no layout/mutation/export path), and per-arm
> timing medians within the observed inter-capture drift (branch/base
> ratios 0.989–0.998); the four capture receipts are committed at
> `docs/kernel-refresh/wsg/w0a-ab-receipts/` (branch captures taken at
> the extraction checkpoint — the post-checkpoint review fold changed
> only test/doc/visibility surfaces and moved the raw-ingestion helpers
> between crates, never a measured layout/mutation/export path).
> Consequences: RFC 0490 M3's
> native leg now waits only on M3's own work (the crate exists and the
> accepted RFC 0492 ruling stands), and **RFC 0494 R-C's named blocker
> is discharged** — the `exact-motion` seam it composes against is
> settled per 0494 §7 "(Phase 4, or the early track)"; R-C's remaining
> prerequisite is its own OQ1 thread-home decision. The `exact-layout-
> core`/`exact-kernel-abi` split and the Swift-mirror deletion remain
> Phase 4 under §7.1's evidence bar.

> **Phase status update (2026-08-26, W0-A precommitment SIGNED).** The
> drafted baseline numbers are **ratified as drafted** — Charlie Cheever,
> 2026-08-26, relayed via orchestration session exact-9e (packet ruling
> item 2, verbatim: "rec fine"), recorded mechanically at `11992d827`.
> `docs/kernel-refresh/w0a/performance-precommitment.json` now reads
> `"status": "signed"` with `signedBy`/`signedDate`/`signedBasis`
> populated, and the `.md` companion's signature box is checked; the
> registered `kernel-w0a-performance-baseline` check enforces the
> json↔md coherence in both directions. Consequence: the per-axis
> regression limits ("no worse than baseline beyond the recorded noise
> floor" on **every** baseline axis), the layout+allocation positive
> targets, and the trend-only wake→presentation disposition are **in
> force** and bind the Phase 1–5 gates; they are no longer drafted
> numbers awaiting conversion. **Remaining W0-A debt before Phase 1
> entry: the three pinned tape apps and the 88-bit style-order
> fixtures** — the signature is not among them. (The style-order half
> landed 2026-08-26 — see the last phase-status note.)

> **Phase status update (2026-08-26, Phase 5 exit checks REGISTERED).**
> The four **registered pass/fail exit checks** this table's Phase 5 row
> names now exist as registered checks under the new
> `kernel-refresh-p5` profile (LLP 0506 D1(e); pack and evidence home:
> `docs/kernel-refresh/p5/`). **Two are green, two are honestly red**, and
> the freeze statement — the fifth, non-check conjunct — **is still not
> written; it is author-ratified by definition and nothing here drafts
> it.**
>
> - **(1) result equality — `kernel-p5-relayout-result-equality`, GREEN.**
>   Incremental relayout is byte-equal to full relayout on raw `f32` bits
>   across two independent reference arms (forced-full via the new
>   `Kernel::force_full_relayout`, and a fresh tree rebuilt from the same
>   post-patch state), over the pinned 1K fixture's seven named patch cases
>   — including this row's two named ones, reorder and keyboard avoidance —
>   and the LLP 0487 corpus (20 fixtures, 43 applicable cases), realized
>   through the corpus harness's own template seam rather than a second
>   copy of it. **Named remainder: the 0486 arm of this row's "0486/0487
>   corpora" phrase is UNMET.** LLP 0486's conformance fixtures are
>   realized by `layout-profile-parity` through the TypeScript lowering,
>   not through the kernel, and `tests/layout/layout-profile.json` records
>   its own `bandArtifact.status: "not-built"`; a kernel-executable 0486
>   corpus does not exist today. It is recorded, not counted as covered.
> - **(2) the dirty-set instrument — `kernel-p5-dirty-set-instrument`,
>   GREEN. RFC 0492 M-D's declared prerequisite is discharged.** The
>   instrument is `kernel/src/dirty_set.rs` plus
>   `begin_dirty_set_scope`/`mark_patch_applied`/`end_dirty_set_scope`,
>   exposing all three components this row names: per-binding affected-set
>   sizes (`k` beside the live `n`), the scoped changed-geometry receipt,
>   and the patch-application→receipt-publication endpoints. An unused
>   kernel pays one `Option` test per layout pass. It gates **no**
>   threshold: the ≤1 ms/≤64-node budget stays RFC 0492's claim per §7.4.
>   **First measurement, and it is a 0492 input:** on the pinned 1K
>   fixture (`n` = 1000), `single-leaf-height` k=5, `single-leaf-width`
>   k=3, `container-size` k=4, `keyboard-avoidance` k=1, `root-resize`
>   k=741, **`reorder` k=999**, and the paint-only negative control k=0.
>   This row's own suspicion is confirmed with a number: reorder is not a
>   `k` ≪ `n` case at all on this fixture, while keyboard avoidance is the
>   sparsest case measured.
> - **(3) the signed per-axis limits — `kernel-p5-precommitment-axes`,
>   RED. The hardware ran; this is not "owed hardware".** The capture
>   `docs/kernel-refresh/p5/performance-exit.capture.json` was taken on the
>   pinned box on 2026-08-26 (three alternating freeze/live rounds in one
>   session, A/A noise floor p95 0.12%/0.14%). The check reads the signed
>   limits and never re-derives or relaxes them. Six failures, and they
>   split into **two classes that must not be conflated**:
>   - **Three are a real program gap that no measurement can move.**
>     `allocation-full-relayout-1k-calls` is **1643** against a signed
>     target of 822 and `…-bytes` is **722780** against 361390 — allocator
>     counts, deterministic and machine-independent, and live main is
>     **byte-identical to the pre-program baseline**. `layout-full-1k` is
>     349729 against a target of 289539 (≥15% required; ~0% delivered).
>     **RFC 0491's arena work has landed structurally but the allocation
>     reduction has not reached the production layout path.** That is this
>     conjunct's substantive finding. Located 2026-08-26: the arena is
>     compiled in ungated (`kernel/src/lib.rs:56`) but has **zero
>     production call sites** — `struct Kernel` (`lib.rs:355`) has no
>     `NodeArena` field and still stores `nodes: HashMap<ViewId, Node>`
>     (`:364`); every consumer is a test or the differential harness; and
>     there is no arena feature on `exact-kernel` at all
>     (`kernel/Cargo.toml:9-12` — the `arena-engine` feature belongs to the
>     corpus-harness crate). The module says so itself
>     (`kernel/src/arena/mod.rs:4-8`: *"Nothing routes production traffic
>     through this module yet."*). **Scoping caution for whoever takes
>     this:** `NodeArena` carries its own `taffy: TaffyTree<()>`
>     (`arena/mod.rs:212`) and delegates straight to it (`:539-559`), so the
>     WS-A cutover alone would replace the node storage and the receipt
>     walk but **not** Taffy's per-flex-container `Vec<FlexItem>`/`FlexLine`
>     allocations (`vendor/taffy/src/compute/flexbox.rs:573,864-900,1265`),
>     which a source reading suggests dominate the 1643. That attribution
>     is **read, not measured** — a `dhat`/`heaptrack` run over
>     `Kernel::compute_layout` on the pinned fixture settles it, and should
>     precede any implementation.
>   - **Three are a derivation defect, not a regression.** The signed
>     timing limits carry **0.094%** headroom. The **freeze arm is
>     unchanged baseline code and exceeds two of its own signed limits** in
>     rounds 2 and 3 while passing all three in round 1; and the paired
>     within-round layout delta **flips sign across rounds** (+2.09%,
>     −0.17%, +3.40%). A signal that changes sign between rounds cannot
>     support a 0.094% threshold. This is surfaced as an explicit open
>     question for the author in **LLP 0491.002 §6 Q1**, because the
>     precommitment's own signed `derivationRule` says later "≤ baseline"
>     claims are **paired same-machine same-session** comparisons and that
>     **cross-session numbers are trend context only**, yet the check
>     judges `current` against a limit derived from an earlier session's
>     median. Nothing in this program has re-derived, relaxed, or
>     re-baselined a signed limit, and nothing should without the author.
> - **(4) the integrated device measurement —
>   `kernel-p5-integrated-device-measurement`, RED. Owed
>   INSTRUMENTATION, not a hardware booking** — the earlier wording here
>   was wrong and would have sent a lane to the pinned box to produce
>   something the codebase cannot produce. Three of the four stage
>   endpoints do not exist in any readable form and the fourth is
>   deliberately fenced: no runtime-wake timestamp exists anywhere; the
>   kernel's `DirtySetObservation` timing endpoints have **zero FFI
>   exposure** and an unanchored `std::time::Instant` clock; host layout
>   timing is millisecond `Double`s scoped to boot; and **no present
>   receipt exists on any Apple host** (`recordCommitPresent` hardcodes
>   `fidelity: "proxy"`, taken before any CATransaction flush, and two
>   validators actively reject a `commitPresent` not labeled `"proxy"`).
>   Gap analysis:
>   `issues/20260826-conjunct4-integrated-device-measurement-has-no-instrumentation.md`.
>   The check is judged against the signed `trend-only` disposition exactly
>   as this row requires — pinned hardware class, representative JS/GC
>   contention, ≥30 samples with all four ordered stage endpoints, declared
>   disposition matching the signed one — and deliberately asserts **no
>   latency threshold**, because an unpredicated pass/fail line is what
>   this row forbids. **A fail-open was closed here on 2026-08-26:** gating
>   on structure alone accepted an artifact assembled from proxy values,
>   and since the hosts can currently produce only proxies the first
>   good-faith attempt would have gone green while measuring nothing. The
>   artifact now must declare per-stage `endpointFidelity`, only
>   `"measured"` is admissible, and absence fails closed.
>
> **This block has been stale before.** Between 2026-08-26 14:00 and 17:00
> it asserted that conjunct (3) was "owed hardware" and that its capture
> "does not exist" while the capture was already committed and the pack
> README said otherwise — and it was quoted as current in that state.
> **`docs/kernel-refresh/p5/README.md` is the live surface for conjunct
> status; this block is a summary that can lag it.** Quote the README, and
> attach the SHA you read it at.
>
> Per RFC 0496's gates-prove-themselves rule none of the four landed
> without being watched to go red: four planted defects in the kernel
> (a missed height invalidation, a `force_full_relayout` that dirties
> nothing, a `publish_layout_changes` that publishes every node, and a
> receipt endpoint that is never taken) and nine synthetic-artifact arms
> for (3)/(4), including two that go **green**, so neither hardware-owed
> check is an unconditional failure. The full matrix is in
> `docs/kernel-refresh/p5/README.md`. Conjuncts (1) and (2) are also
> `governance` profile members, so they re-run on main per push when their
> declared inputs change; (3) and (4) are `kernel-refresh-p5` only, because
> parking a permanent red in a per-push gate teaches people to re-run reds
> instead of believing them.

> **Phase status update (2026-08-26, WS-B style-order fixtures LANDED).**
> **WS-B's fourth and last named interim non-mutating W0-A gate is in**
> (`kernel-style-order-fixtures`); the other three — differential fuzz,
> the FFI census, and the `size_of` pins — landed earlier, so WS-B's W0-A
> bullet is discharged and the remaining W0-A debt before Phase 1 entry is
> **the three pinned tape apps alone**. The gate has two halves. The per-bit
> table (`tests/protocol/style-order-table.generated.json`) is **derived, not
> transcribed**: bit constants and enum `TryFrom<u8>` vocabularies parsed from
> `kernel/src/style.rs`, and the read order, per-bit codec, destination field,
> and byte width parsed from `style_decoder.rs` itself; the four live SetStyle
> implementations are joined on for **presence and order only**, because a
> width parsed out of a second language would be a second hand-authored truth.
> The executable half (`kernel/src/protocol/style_order_gate.generated.rs`,
> crate-internal because `style_decoder` is `pub(crate)` and WS-G forbids
> bloating the durable rlib for tests) proves the table by execution: one
> payload with every style bit set decodes to exactly the expected value in
> every field, and per bit a payload of exactly the declared width decodes
> while one byte short is a truncation refusal and one byte long is a
> trailing-bytes refusal (W0-B). Per RFC 0496's gates-prove-themselves rule it
> was watched go red four times, each reddening only the half it should:
> a same-width `TRANSFORM_X`/`TRANSFORM_Y` read-order swap failed the all-bits
> test while the width test stayed green; disabling the W0-B trailing-byte
> refusal failed the width test while the all-bits test stayed green; a
> `buffer-writer.ts` write-order swap and a deleted decoder read block each
> fired a **substantive** refusal that **still fired after regenerating the
> artifact**, so the gate is not a regeneration diff that greens the moment
> somebody commits the divergence.
>
> Three facts it pins that nothing pinned before. **(1) The live wire order is
> decoder DECLARATION order, not ascending style bit** — three ascending-bit
> violations are recorded (`FLEX_WRAP` bit 34 precedes `JUSTIFY_CONTENT` bit
> 15; `ALIGN_SELF` bit 35 precedes `FLEX_GROW` bit 17; `COLUMN_GAP` bit 33
> precedes `POSITION_TYPE` bit 20). The EXWF schema package's
> `components.styleLayouts.wireOrder` declares *"ascending style bit; ascending
> u64 mask word"*, which is a statement about the **post-break** wire: a
> Phase-2 WS-B codec generated from `styleLayouts` in bit order will not
> interoperate with this one, and that reordering is not currently recorded as
> a wire-visible change in the Phase-2 closed input list. **(2) Six of the 88
> bits have no TypeScript producer at all** (`TINT_COLOR`, `GRID_COLUMN`,
> `GRID_ROW`, `JUSTIFY_ITEMS`, `ALIGN_CONTENT`, `JUSTIFY_CONTENT_GRID`), so
> they cannot be exercised end to end from the default producer path.
> **(3) The web host decodes 72 of 88** and fail-closed-refuses the other 16
> through its `allowed`-mask check — correct behaviour, but the complement was
> nowhere enumerated until this table named it. The Kotlin decoder is
> deliberately not joined and not counted as coverage: it is gated off under
> `android-kotlin-decoder-gate` until the JNI typed-operation projection lands
> (OQ4). No phase gate moves and no ledger row is claimed satisfied.


Dependency edges the table encodes: the WS-H dirty/epoch substrate
precedes WS-C receipts (Phase 1 → 3); every wire-visible WS-I decision
precedes the single break (inside Phase 2); direct-write consumer
migration precedes the one-write-path gate; OQ3 is closed (staged
generation snapshots, 2026-08-25) and its dependents implement in
Phase 1;
WS-B(4)'s clock-phase authority is a W0-A/Phase-1 slice (0490 M2
consumes it — it does not wait for the style tables); Phase 5 owns
everything OQ7 freezes. Standing instrument across Phases 1–3: **the
old kernel as a differential oracle over semantic tapes** — tapes are
an **owned, versioned `SemanticFrame` IR** (capability context, batch
boundary, typed operations with explicit patch/clear semantics,
captured nondeterministic ingress results), to which both the legacy
and EXWF decoders adapt; today's borrowed `OpEvent` cannot represent
the new semantics and is not the tape. The legacy oracle itself is
**frozen as content-addressed binaries** (or captured outputs) in
W0-A — it must not evolve beside the new kernel and become
self-confirming. Byte-level parser fuzzing stays
separate. The same IR is the flight-recorder log and the delta
ledger's substrate — one migration artifact, many consumers. Equality is asserted only over **unaffected
observations**; every intended behavior change (overflow, grid,
direction, `Auto`) is an entry in a machine-readable **semantic delta
ledger** — old result, new result, owning authority, fixture — which is
the oracle's exception list, the migration note, and the review
checklist in one. Receipts and EXNODE, which the old kernel never
produced, are validated by schema recomputation and the failure-
injection suite, not by oracle equality. Tapes come from the fixture
corpora plus three pinned in-tree apps recorded in W0-A. The shadow
mode is deleted at cutover.

RFC 0490's M1 runs concurrently with W0-A; M2 consumes the WS-B(4)
phase-authority slice (pulled into W0-A/Phase 1 for exactly that
reason). **0490 M3 is blocked on both legs** pending the atomic
evaluator decision (the 0099↔0491 amendment ruling, drafted in RFC 0492 and awaiting
acceptance) —
0490 r5 says so and this document conforms; the native leg additionally
waits for the `exact-motion` extraction (early track, WS-G) rather than
the whole program. 0490 M6 is the payoff customer for Phase 1 + WS-H +
the WS-I text unification.

The 0486/0487/0488 program has kernel lanes in flight *now* (the 0487
WS4 kernel candidate-selection half; the 0488 W3 measured set landed
2026-08-20). Phase 1 does not start under an in-flight 0486-program
kernel lane — landing order is coordinated with that program's
orchestrator, and its corpus/parity gates run green at every phase exit
of this one. The relationship is symbiotic, not competitive: 0486–0487
supply the semantic ground truth and the regression instruments; this
program supplies the substrate they certify against.

## 6. Honest costs and tensions

- **The test migration is the real price of WS-A.** ~14K in-file test
  lines are welded to private fields; migrating them first is slow,
  unglamorous, and non-negotiable — skipping it drowns the data-model
  change in churn.
- **The batched EXWF rev breaks everything at once.** That is the point,
  but it means Phase 2 must land the Rust decoder, TS encoder, web-host
  decoder, Rust-producer encoders, and the Android JNI projection in one
  motion (the Kotlin decoder is deleted, not regenerated — OQ4); the
  W0-A/W0-B gates exist so the break is loud everywhere. The binary-event
  consumer inventory includes the Hermes/ibex dispatch ABI and every
  host JSON adapter.
- **LLP 0486's warning applies to WS-C/WS-D:** parity claims need the
  WS1 dual-engine instrument, not assertion. This program's cross-host
  claims should be measured by that harness once it exists.
- **The Android decoder is a fork in the road:** parity-gate its ~100
  hand constants, or give it the `HostInterpositionFrame` treatment (thin
  JNI over the Rust decoder) and delete ~700 Kotlin lines. The second is
  better and is the default here (OQ4).
- **Two baselines, two jobs.** The W0-A pinned capture is the
  *pre-change comparator* every phase gates against through Phase 5;
  "re-baseline after" means only that the post-cutover numbers become
  the new ongoing baseline once the program exits — never that the
  comparator moves mid-program. That is still a deliberate inversion
  of 0258's build-the-benchmark-first gate and it means a temporary
  evidence gap; `kernel-bench` re-runs at each phase exit as an
  informational check.

## 7. Motion and clock rulings (in concert with Accepted RFC 0490)

1. **Motion evaluation ownership — scoped.** *Native* motion-graph
   evaluation is Rust, living in the LLP 0297 §4.4 **main-owned motion
   domain** (not the runtime-thread `Kernel` struct — no ownership rule
   breaks). Native hosts become sink appliers; the 8.4K-line Swift graph
   mirror is deleted in Phase 4; the per-frame MSCH fencing apparatus
   dissolves as a side effect (WS-D gets it structurally, this ruling
   gets it philosophically). This is the shape that gives 0490's "one
   value graph" a single implementation across the native targets
   including Expose/DRM. Per Accepted 0490 r5, this ruling is
   **0491-owned, not joint** — 0490 does not co-decide it — and the
   **atomic evaluator decision is not made here either** — and it is
   no longer unmade: RFC 0492 §8 states that its §4.1/§4.6 *are* the
   0099↔0491 evaluator ruling (one Rust `exact-motion` evaluator on
   every tier, the web leg as the wasm build), so **0490 M3 waits on
   0492's acceptance of that ruling, both legs** — this document
   cites it as the ruling awaiting acceptance, never as an open
   amendment — and this document's *input* to that ruling is not a
   preferred evaluator at all: it is the constraint frozen LLP 0500 D2
   already sets (the web dev loop runs the production runner) plus
   WS-G's wasm-day-one crate hygiene that makes the same-engine answer
   *possible* — the ruling itself is 0492's. The CA/CSS
   delegation question for two-endpoint `transition=` stays with
   RFC 0492 (per the r8 note). Deleting the Swift evaluator (Phase 4)
   additionally requires behavioral parity evidence — golden
   gesture/driver trace replay equality, settle outcomes, and arena
   receipts, extending the existing cross-language trace corpus to the
   evaluator level — never dependency-compilation alone.
2. **Clock phases.** The tick *source* stays host-owned (CADisplayLink /
   rAF — platforms own vsync); the phase list and consumer enrollment
   become a Rust-side generated authority (WS-B(4)) that Swift and web
   mirror. The evaluator and phase authority are **tick-source-agnostic
   by construction**: a display link, a rAF, and a virtual clock (test
   harness, Acto, deterministic replay) are interchangeable drivers of
   the same phases. The closed-form motion math already makes every
   value a pure function of (time, inputs); this ruling keeps that
   purity reachable from the outside — seekable animations,
   frame-at-t queries, and settle-flake-free agent verification are
   consumers the Motion program (RFC 0492) builds on top,
   not retrofits. The dead Rust `MotionClockRegistry` machinery is deleted (WS-F), but
   `MotionTickPhase` is kept and made real as that authority — resolving
   the one direct collision between the review and 0490 M2. Whether more of
   the loop later migrates into `exact-motion` remains 0490 OQ5, decided
   with M2/M3 evidence.

3. **Landed RFC 0490 amendment — WS-C delivery ownership (recorded in
   0490 §6's own amendment-table shape).** Before its 2026-08-20 addendum,
   Accepted 0490 consumed an older WS-C reading in three places — §3.3.3 ("present receipts ride
   RFC 0491 WS-C's upward channel"), §3.4.4 (GPU completions on the
   same channel), and OQ6 (ordering on it). WS-C's ruled shape is one
   *encoding/correlation family with three delivery owners*, because a
   literal shared channel would create exactly the cross-thread surface
   §2 forbids. The landed addendum restates the three sentences as:
   kernel exports carry only runtime-thread receipts (LLP 0488
   container feedback, Acto commit receipts, Aquifer receipts); GPU
   completions are `gpu-service → exact-motion` command ingress on
   main, never through `exact-kernel`; present receipts stay with the
   host clock under LLP 0354 — all three sharing the frame-correlation
   id, none sharing a transport. Process discipline (LLP 0001):
   **Accepted 0490 remained controlling until the addendum landed** — a
   Review document cannot temporarily override an Accepted one — and
   **the 0490 delivery-ownership addendum LANDED 2026-08-20** via the
   author-authorized coherence wave (rather than this RFC's
   acceptance-lockstep, a recorded process variance): the hold on the
   three affected flows is lifted; 0490's addendum now states the
   ownership in its own voice. The disagreement was a recorded owed edit
   with a named landing event — now discharged — never a silent
   revision of an Accepted document.

4. **Owed RFC 0492 amendment — performance-claim ownership (lockstep,
   §7.3's shape; landed with r15).** r14 demoted the ≤1 ms/≤64-node
   relayout number off this program's exit, but 0492 still read the
   number as "RFC 0491 WS-H's" — a claim 0491 no longer makes, leaving
   the family a deadlock: 0491 would never demonstrate the number, and
   0492 would not start M-D until 0491 did. The transfer, stated once:
   **0491 supplies the dirty-set instrument and the result-equality
   guarantee (WS-H's two exit obligations); RFC 0492 owns the ≤1 ms /
   ≤64-node number, the gesture classification (which animations may
   claim the ≤64 budget), and M-D's gate — M-D starts when the
   instrument exists and 0492's own target is demonstrated on it.** A
   target miss is a 0492 ruling with 0491 as the substrate supplier
   (further kernel skip-work is commissioned as scheduled work), never
   a 0491 freeze blocker. Process discipline, same as §7.3: had the
   0492 amendment not landed, 0492 would remain controlling on its old
   reading; it lands in lockstep with this revision (0492 r4 carries
   the matching Summary/§3/§5 M-D/§7/§8 edits), so the two documents
   state one owner. OQ10's decision criterion cites 0492's target
   accordingly.

## 8. Required companion amendments (owner-gated)

This Review RFC records asks; it does not amend Accepted owners by being
accepted itself. Each row below must land in the named authority, receive that
authority's owner disposition, and regenerate its governed artifacts before
the affected 0491 gate may activate. Until then, the conservative language in
the current owner wins and any generated null/unselected cell stays null.

> **ALL SEVEN COMPANION AMENDMENTS ARE IN FORCE (2026-08-23).** Charlie
> Cheever ratified the seven asks as drafted in LLP 0491.001 (decision
> relayed via orchestration session exact-9e); each landed in its target
> authority the same day: `A-0496-CORE-MEMBERSHIP` → RFC 0496 §4.2.1 +
> the §4.1 `core` field + `exact-verify-core-membership.json`;
> `A-0507/0515-PRODUCER-SUCCESSOR-ID`, `A-0507/0515-BATCH-SUCCESSOR`,
> `A-0507/0515-ROOT-MIGRATION`, `A-0507-EXWF-SEMANTIC-EPOCH` → LLP 0507
> §§5.3/7.3/13/8.4 as amended with the LLP 0515 fidelity-restoration
> sync; `A-0510-COMPATIBILITY-DIMENSIONS` → LLP 0510 §13 (text in
> force; schema-file major deferred to Phase-2 EXWF activation per the
> ratified sequencing); `A-0492/0510-MOTION-EVALUATOR-EPOCH` → RFC 0492
> §8. The break package is regenerated (identity cells selected,
> batch-successor cells pinned, `RootMigration` active, the draft
> semantic epoch emitted; `exwf-break-package-parity` green).
> **Consequence: the W0-B gate condition "A-0496-CORE-MEMBERSHIP has
> landed" is now MET** — W0-B remains gated only on RFC 0496 V-A and
> V-B meeting their exits (§5's other conjunct).

| Ask | Required owner amendment | Requested content and gate |
| --- | --- | --- |
| **`A-0507/0515-PRODUCER-SUCCESSOR-ID`** | Accepted LLP 0507 §5.3/§14.6 and Accepted LLP 0515 T2/T3/§11.1 | Select fresh `ProducerId` mint on `ProducerRestart` and coherent full reload (both introduce a new `ExecutionGeneration`); keep matching-generation reconnect as reissue and all other closed rows as hold. Regenerate the open-issue selection and identity artifacts. Until owner acceptance, producers MUST NOT depend on mint versus reissue. |
| **`A-0507/0515-BATCH-SUCCESSOR`** | Accepted LLP 0507 §7.3/§14.7 and Accepted LLP 0515 row 8/T4/§11.2 | Select `successorKey = (ProducerId, ExecutionGeneration, rootId, rootIncarnation)`, `initialValue = 1` (matching the shipped native-web gate), `refusedFrameConsumesNumber = false`, and `reconnectPersistence = true` only for a matching producer/execution/root-incarnation key; restart or root-incarnation replacement starts a fresh key. Regenerate `schema-package.json` so all four cells cease to be null. Until owner acceptance, §7.3's no-dependence interim controls. |
| **`A-0507/0515-ROOT-MIGRATION`** | Accepted LLP 0507 closed identity algebra and Accepted LLP 0515 lifecycle/totality table | Add the `RootMigration` row specified in WS-A: all counters hold; the live allocation's root binding rekeys atomically; the source binding tombstones; destination collision or partial publication refuses; raw refs fail closed while logical keys rebind. The generated `identity-events.json` carries this as a pre-activation obligation and MUST NOT activate before both owner texts agree. |
| **`A-0507-EXWF-SEMANTIC-EPOCH`** | Accepted LLP 0507 capability context, EXFF, admission, compatibility, and break-package sections | Add independently admitted `exwfSemanticEpoch`, computed as the domain-separated SHA-256 of the canonical identity matrix, recovery assignments, breaking-window ledger, and selected BatchId rules. It is excluded from `exwfSchemaDigest` and compared/name-reported separately. Phase 2 cannot activate identity/recovery/ordering semantics without it. |
| **`A-0510-COMPATIBILITY-DIMENSIONS`** | Accepted LLP 0510 and its v2 compatibility-tuple schema/generator/validator | Major the v2 tuple once in the breaking window to carry required EXWF schema identity, `exwfSemanticEpoch`, and `motionEvaluatorEpoch`; keep the existing `motion` object exclusively wire/layout-shaped. Exact Native admission compares each dimension independently. No v0 edit exists or is requested. |
| **`A-0492/0510-MOTION-EVALUATOR-EPOCH`** | Review RFC 0492 evaluator authority plus Accepted LLP 0510 artifact admission | Define the canonical `motionEvaluatorEpoch` preimage (evaluator sources, deterministic-math profile, feature set, exported evaluator ABI) and generated local-host comparison. An evaluator-only change MUST move this epoch even when Motion wire schemas/layouts do not; a wire-only change MUST NOT fabricate an evaluator change. Phase 4 and Swift-evaluator deletion wait for this detector. |
| **`A-0496-CORE-MEMBERSHIP`** | Review RFC 0496 registry/quarantine design | Add an owner-approved machine-readable membership predicate (or explicit registry field) for the prose core categories `registry consistency`, `security-class`, and `release admission`. 0491 requests entry-security checks map to security-class and identity, active-codec, and recovery-transactionality checks map to release admission; 0496's owner decides the exact membership. There is no current “closed kind list,” so W0-B and later activation remain blocked until this mapping exists and V-A/V-B meet their exits. |

## 9. Open questions

- **OQ1 — Prop storage shape.** Sorted small-vec vs bitmask+dense-slots vs
  hybrid for `props`; decided by a benchmark on the WS-A branch. Lean:
  true dense columns with presence bits for the common props —
  "median count is small" is how HashMaps survive too long.
- **OQ2 — FFI IDL vs cbindgen-first.** Full IDL (kills the Swift wrapper
  and discriminant tables too) vs cbindgen now + IDL later. Default: IDL,
  scoped to one capability group first to prove the generator.
- **OQ3 — closed (2026-08-25).** Undo journal vs staged apply for WS-H
  transactionality: **staged generation snapshots** are the selected
  engine — the recorded lean, confirmed by Charlie Cheever 2026-08-25
  (relayed via orchestration session exact-9e, decision register
  item 12) — near-free on the WS-A arena and doubling as Acto's read
  isolation; the undo journal remains the named fallback if staging
  regresses on the production arena. Closure unblocks the arena
  transaction engine, the WS-H rejection-atomicity gate, and the
  `BatchValidator`/`ProtocolTreeReset` deletion path; implementation
  belongs to the kernel Phase 1 lanes, and the deletions still wait for
  WS-D's capability-context resync answer per §"WS-H".
- **OQ4 — closed (r12).** Android's disposition is JNI over the shared
  Rust decoder with typed-operation projection; the Kotlin decoder is
  deleted; Android is EXNODE-exempt in this program (LLP 0287 owns
  kernel-on-Android). W0-B declares Android known-red until the JNI
  projection lands.
- **OQ5 — EXPAINT scope.** Whether the paint-shaped export ships in WS-C
  or waits for 0490 OQ1's paint-tier bake-off; default: design the family
  header for it, ship only EXNODE.
- **OQ6 — Where the module registry lands** after the split — `exact-modules`
  as a kernel dependency vs a trait the kernel consumes; selection's
  single coupling point decides it.
- **OQ7 — What "1.0 ABI freeze" means afterward.** *(The
  program-wide answer now exists: **LLP 0506** (Active) owns the exit
  conjuncts, waiver authority, and scope protection; this OQ's
  kernel-local surface list feeds 0506 D1(a)/(e), and Phase 5's
  ratified freeze statement remains a conjunct there rather than a
  freestanding gate.)* **The statement itself is drafted at
  `llp/0491.002-oq7-abi-freeze-statement.decision.md` — Status Draft,
  NOT RATIFIED, awaiting the author.** It is derived, not authored: every
  surface is enumerated by measurement with file:line citations, every
  classification the tree already settles is read out and cited, and every
  classification that is a genuine choice is left as an explicit open
  question in its §6 rather than silently resolved. Ratifying it is a
  semantic act on an enumeration, not an authoring task. This window
  closes when
  external users arrive; the exit criterion for the program is a written
  statement of which surfaces are then frozen (the EXWF frame rev, the
  generated C ABI, the rlib API, the EXNODE/feed schema) and which stay
  negotiated. Window protection: **external onboarding stays frozen
  through Phase 5's ratified freeze statement** — Phases 3–5 still break
  the C ABI, EXNODE, the rlib graph, and Motion; app-wire-only exposure
  after Phase 2 requires the author's explicit acceptance with the
  remaining breaks documented. The window has an owner — the author
  closes it, not the calendar.
- **OQ8 — Kernel token indirection for Facet (follow-up-scoped: adoption
  is a separate program unless the Phase 2 patch-path benchmark forces
  it; this program only keeps the representation extensible).** On web, theme/density
  switching is late-bound for free (CSS custom properties); on native the
  kernel is the equivalent engine and has no variable concept — a theme
  flip re-renders through JS and re-encodes styles node by node, and
  `rem`-like scaling is impossible (`docs/css-dom-restrictions.md`). The
  idealized Facet (instant theme/density switch, live Design Mode token
  edits) argues for style values that may be **token references** resolved
  against a kernel-held theme table — switch = swap table + invalidate
  the dependent **domains** (RFC 0497 §4.2's corrected model: tokens
  feed paint, layout, text-shaping, and semantics, so a flip is O(1)
  *wire* traffic but never assumed paint-only renderer work). The
  counterweight is real: WS-D's
  SetStyle-as-patch plus typed props may make JS-side switching cheap
  enough, and a token layer is genuine new kernel complexity. Decision
  criteria: measured theme-flip latency on a 5K-node tree via the patch
  path, Design Mode edit latency, and whether `rem`-equivalent type
  scaling ships. If adopted, it is one new value encoding in the WS-B
  style table (`token-ref u16`) plus one theme-table sidecar op in WS-D's
  extension envelope — and WS-B now **reserves both the discriminant and
  the dependency-domain bits inside the Phase 2 break** so a later
  adoption is a table addition, not a
  wire migration; the program's shapes accommodate it either way,
  which is why it is an OQ and not a workstream. RFC 0497 F-B is the
  named driver of the adoption measurement.
- **OQ9 — When (if ever) the kernel goes parallel internally.** Governed
  by LLP 0482's tier taxonomy and latency invariant; this OQ is its
  kernel-local application. Two rulings are made here rather than left
  open. First, the **work-grain rule**: parallelism pays when
  work-per-task dwarfs handoff cost — the webnode/livenode precedent
  (3–4× across 8 processes rendering component trees server-side) held
  because each subtree was milliseconds of interpreted work with no
  cross-subtree layout constraints; the kernel's whole 1K-node layout is
  <5 ms with sub-µs per-node work and flex/grid constraint coupling
  between siblings, so that shape recurs in Exact at 0482's Tier 1
  (server generation / build fan-out, LLP 0351 — outside this RFC), not
  inside the kernel. Second, **process-level parallelism inside the
  kernel is rejected outright**: serializing subtrees and results across
  an IPC boundary costs more than the work it moves at this grain;
  in-kernel candidates are threads or nothing. Otherwise decided by
  measurement after WS-A/WS-H land, not by appetite: the review found
  frame cost dominated by avoidable constant-factor waste (string
  hashing, hash-map hops, unconditional O(n) walks), and parallelizing
  waste multiplies it. The trigger is layout or text preparation
  exceeding its frame budget on target trees *after* the constant-factor
  and skip-work fixes. If triggered, candidates in payoff order:
  (a) **batched text preparation** — the `TextMeasurer` trait (WS-E)
  ships batch-shaped requests from day one so *hosts* may shape N
  paragraphs concurrently under their own threading rules, no kernel
  threads involved; (b) **per-root / per-island parallel layout** —
  independent Taffy trees with measured-leaf boundaries (LLP 0313 shape),
  structurally clean; (c) **pipeline concurrency** — semantics/selection/
  EXPAINT derivation from an immutable generation snapshot while the next
  frame mutates, the browser-style split WS-A's snapshots enable;
  (d) Servo-style fork-join inside one layout pass, last — sequential
  constraint dependencies and small n make Amdahl unkind at UI sizes.
  GPU-side layout or shaping is explicitly out: flexbox/grid constraint
  solving and HarfBuzz-class shaping are irregular and dependency-bound —
  the GPU's parallelism is spent where RFC 0490 already spends it.
- **OQ10 — The animated-layout mechanism ruling.** Three partial
  mechanisms exist for a motion value that targets layout: same-frame
  **layout islands** (LLP 0313, opened as the `layoutIsland` sink family
  by LLP 0449 — main-owned, bounded, no kernel round-trip), the
  **runtime-thread relayout path** through the LLP 0297 constraint
  buffer (general, but with the accepted one-frame-late cost), and
  **baked timelines** (pre-solved keyframes, open measurement ticket).
  Idealized Motion needs a ruling on which tier serves which case — the
  0422-style "taking a higher tier than needed is a defect" discipline
  applied to layout motion — and the ruling is empirical: it depends on
  what RFC 0492's ≤1 ms/≤64-node target proves achievable when measured
  on WS-H's dirty-set instrument (the number is 0492's, per §7.4)
  and on RFC 0490 M3's clock evidence. Decide it with LLP 0099/0313/
  0349/0449 as the authorities, after WS-H lands, before promising
  general layout animation in any public API. The one structural rule
  this RFC does fix now: whatever the ruling, layout-targeting motion
  values route through declared sink families and the frame clock's
  phases — never a fourth ad-hoc channel into the kernel.

- **OQ11 — The Expose multi-app trust model.** LLP 0406 makes Exact the
  UI foundation of an OS shell, and an OS shell eventually hosts code
  its author didn't write. At that point the protocol stops being a
  hygiene boundary and becomes a **security** boundary: kernel-per-app
  isolation vs shared kernel, per-app resource budgets (node counts,
  buffer sizes, GPU demand via 0490's account model), and how far the
  Week-0 differential fuzzing must graduate toward adversarial-input
  guarantees. The v1 answer may legitimately be "one kernel per app
  process, budgets deferred, fuzzing best-effort" — but it must be
  *stated* before an Expose milestone ships third-party-adjacent
  surfaces on this kernel, because retrofitting a trust boundary into a
  protocol is the one migration the breaking window cannot help with
  later.

## 10. Post-acceptance amendment (2026-08-26) — the RFC 0540 collection folds (0504 §3 row 60)

This RFC is Accepted, so RFC 0540's asks against it land here as a
dated post-acceptance amendment rather than as edits to the frozen
workstream prose. The deciding text is Accepted RFC 0540 (§1.2, §5,
§8.4, §8.5 row 60); the record shapes are LLP 0543 §7 and §9.5.
Nothing below re-decides either. **Delivery is post-window** — RFC
0540 §8.6 classifies the lists-v2 surface as D2 non-window — so no
0491 phase gate moves and no Week-0 or Phase-2 exit changes because of
this amendment. Where it and the frozen §3 text disagree, this
amendment governs.

**10.1 WS-H gains a second named customer.** WS-H's presence-set
pattern gets a second real user beside the portal/inline-embed walks:
the **hidden set of a certified collection root is a WS-H presence
set** (0540 §1.2), so a tree with no hidden rows pays nothing, and the
frame walk over hidden rows is a presence-set early-out rather than a
traversal. The scaling path RFC 0540 §5 reserves — **hidden-run
coalescing at its L6 rung** — is explicitly WS-H skip-work, and its
trigger threshold comes from WS-H's own dirty-set instrument (0540
OQ5), not from a number chosen in 0540. This is a customer record, not
a new WS-H obligation: WS-H's exit gates are unchanged, and 0540's L6
is gated on WS-H, never the reverse.

**10.2 WS-G: `exact-list-core` leaves the interim-home and
later-extraction lists — and a naming correction.** WS-G's published
crate DAG lists `exact-list-core` among the crates living inside
`exact-layout-core`/the facade behind features "until their extraction
triggers fire", and again among the four that "extract later". RFC
0540 §8.4(1)–(2) puts that surface on the **subtraction** path
instead: `@exact/list-core` windowing, `native-cell-style.ts`, and
`NativeVirtualizedList` are the retained legacy path until a
population-zero deletion certificate (LLP 0503 §3.2), and
`native_list.rs`'s planner, its FFI exports, the Swift callers, and
the Phase 2a bridge are deleted not-before all-host L5 readiness and
**not-after WS-G Phase 4**. Its exit is therefore **deletion, not
extraction**, and the name is dropped from both WS-G lists: the DAG's
**final** edges carry no `exact-list-core` node, while a transition
edge through the facade remains legal until the certificate lands.
Two consequences worth stating. (1) WS-G Phase 4 now carries a real
ordering constraint from outside this program — it cannot complete
while the 0540 §8.4(2) row is undeleted, because the row's own
not-after is Phase 4; that is 0540's sequencing, recorded here so
WS-G's planning sees it. (2) **Naming correction:** there is no Rust
`exact-list-core` crate on this tree — `packages/exact-list-core` is a
JS package and the kernel-side list surface is
`kernel/src/native_list.rs`. WS-G's list conflated the two. A reader
should not go looking for a crate to extract; the row above is what
the name stood for.

**10.3 The Week-0 recovery-class table gains an explicit
deletion-before-activation row for op 38.** The W0-A table is total
over the inventory's live sidecar ops and today assigns **list model
38** the class "tree-bound with staged last-good". RFC 0540 §8.4(3)
deletes op 38: descriptor publication is gone with LLP 0139's
supersession, and the push model publishes nothing in its place. Op 38
therefore takes the table's **explicit deletion-before-activation
row** — the escape the table already provides ("a row per op, or an
explicit deletion-before-activation row") — rather than carrying a
recovery class into a revision that will not contain it. The
completeness rule is satisfied by the deletion row, not bypassed by
it; a claimed-complete matrix that simply omits 38 is still a Week-0
red. The matching row belongs in LLP 0507 §11.3's matrix (0540
§8.4(3)'s other half, that owner's).

**10.4 WS-C gains four tree-bound feed records, with WS-E FFI
projections.** RFC 0540 §8.5 row 60 and LLP 0543 §7 add four
root-scoped families to the WS-C/feed surface
(`docs/host-data-plane-feed.md`), each stamped with the commit
generation it rides and **adopted with that commit's frames** —
`tree-bound`, never a LLP 0507 §11.3 sidecar:

| Record | Cardinality | Carries |
| --- | --- | --- |
| `ParticipantIndexRecordV1` | per root, per commit changing membership, any participant box, retention state, or the root's classification props | axis, collection stable id + generation, expansion, content extent, and the sorted participant entries (`end[i] ≤ start[i+1]`) |
| `VisibilityReceiptRecordV1` | one per applied bundle | source id and ordinal span, bundle id, engine epoch, originating batch number, optional hold registration, per-entry outcome + render state |
| `AnchorResolutionReceiptRecordV1` | per resolved **`item`** intent change only | collection stable id, item key, logical edge, resolved raw address |
| `AnchorDeltaRecordV1` | per anchor delta | signed delta along the axis, intent kind, optional resolved address, reason |

Every node reference is a raw address and every key a `KeyV1`. Their
**generated FFI projections are WS-E's** (0543 §9.5): `#[repr(C)]`,
field order as declared, little-endian, strings as `{ptr, len}` into
kernel-owned arenas, optionals inline plus a `present: u8`. The
consume discipline splits by family — the index, anchor-resolution,
and anchor-delta getters follow the existing probe/copy/consume-on-
success measured-box pattern, while the **receipt** family uses a
snapshot token so a header and its outcomes are read from one
consistent frozen snapshot that later publishes never alias or
invalidate. Acto-visible copies ride the upward kernel-exports owner
with LLP 0507 §12.3 receipt identity.

**10.5 Boundaries, so this amendment is not over-read.** The
`VisibilityTransition` **event payload family** and the typed props
are **not** this row — they are 0504 row 65, already landed in
`protocol-inventory.json`, and this amendment does not restate their
schema. Nothing here crosses the LLP 0510 boundary (0540 §8.7): these
are tree-bound records and typed props, not App-ABI, grant,
transport, or state records. And the four record schemas above are
shapes RFC 0540 delegates to WS-C: WS-C decides them and grows them in
the inventory, so a later WS-C revision may refine the fields without
returning to 0540.

## Revision history

- **r18 (2026-08-22)** — owner-requested fold of both r17 NOT READY
  verdicts, with every cited claim checked against the tree. The five theses
  remain owner-endorsed and unchanged. The material union is closed in this
  revision: Accepted 0507/0515 open cells were removed from 0491's effective
  rules and re-homed as named, owner-gated §8 amendment asks; RootMigration
  gained an explicit row and generated pre-activation obligation; EXNODE now
  includes island descriptors and consumed-input vectors with their reset,
  generation, slot/tombstone, and retirement lifecycle; EXWF gains a proposed
  separately admitted semantic epoch instead of overloading 0507's
  schema-only digest; Motion wire identity and evaluator/artifact epoch no
  longer alias; Week 0 is split into non-mutating W0-A and V-A/V-B-gated W0-B;
  shipped `state … hidden` absence semantics is explicit; rejected-frame
  recovery is one token-bound all-root transaction including staged
  allocation-generation deltas, watermarks, atomic publication, and presented
  acknowledgement; WS-H injection covers the producer-session table;
  SemanticFrame names recovery fields and literal closed-schema projection;
  and Exact Native routes through Accepted 0510 v2. Freshness/editorial fixes
  cover Related, 0504's M-D wording, the generator/hash explanation, and the
  missing r16 history entry. Adjudication where reviewers diverged: the tree
  supports Codex on EXNODE completeness (feed §8 was omitted), recovery
  atomicity (the ticket requires every live root and acknowledged
  presentation), and authority scope (0507/0515 and the generated package
  retain null successor fields); Grok's verified ApplyMode, recursive
  Phase-2 `DestroyView`, and 64-bit `{lo,hi}` tightenings are preserved.

- **r17 (2026-08-22)** — owner-authorized 0491 revision cycle. The
  authorization described the target as r11 and requested r12, but the
  checked-in document already contained distinct r12–r16 entries (with r16
  recorded in `Revised:`); reusing r12 would falsify provenance, so this
  cycle is recorded monotonically as r17. The endorsed diagnosis and five
  theses are unchanged. Tree verification adjudicated the one relevant
  Codex/Grok factual disagreement: Codex was right that
  `packages/exact-native-web/src/protocol-dom-host.ts` rejects sequence gaps;
  Grok's earlier “no production web decoder” search was stale. r17
  introduced `AttachIncarnation`/`BatchId`, exact-next admission and a
  committed-high-water candidate across all transports, in-process preservation, and
  receipts echoing batch identity with commit generation, while keeping
  node-reference lifetime on WS-A's separate six-field raw address. EXNODE
  remains the binary projection of `docs/host-data-plane-feed.md`; r17 made
  the then-enumerated sections and atomic batch/commit correlation explicit
  but omitted the feed's island descriptor/consumed-input sections, repaired
  in r18. The
  already-correct full-corpus arena adapter, RTL/text preservation,
  per-kernel callback inventory, and testId multimap were reverified and
  tightened with, respectively, fingerprinted same-session comparisons,
  physical-vs-logical precedence, a two-kernel isolation gate, and closed
  ambiguity/query rules. Grok's terminal Phase-3 material was verified
  against the Cargo graph and Windows typed-row call path and fixed: typed
  columns are for in-process Rust hosts (Windows/TUI), the envelope is for
  Apple/FFI/transport, and Exact Native producers consume neither. Grok's
  non-blocking tightenings also landed: explicit restore apply modes, a
  proposed completion of `ProducerId` event cells (restructured as owner-gated
  asks in r18), recursive `DestroyView` in the Phase-2
  list, EXWF/default freeze coupling, and lossless 64-bit bridge encoding.

- **r16 (2026-08-20)** — follow-up loop round-2 revision, both families
  folded. Added the restore allocation class and session-owned allocation-
  generation table, explicit fence domains and lossless no-wrap counters,
  the generated identity-event/carrier obligations, total recovery-class
  assignments over the live sidecar inventory, and the TUI ordering
  known-red; kept `DestroyView` single-node until Phase 2; selected authored
  percentage points and per-row `Auto` admission; separated flex and grid
  direction handling; closed EXWF header/digest/defaultRef coverage and the
  consumer-census↔ledger join; and tightened Phase 3/4 projection, wasm, crate
  DAG, and Motion disposition gates. r18 later corrects r16/r17's treatment
  of Accepted 0507/0515 open cells and adds the omitted RootMigration event.

- **r15 (2026-08-20)** — dedicated follow-up loop (L1), round-1
  revision: the first round of 0491's own dual-family loop after the
  program-wide loop closed (codex gpt-5.6-sol@ultra + grok-4.6@xhigh on
  the r14 capsule, both NOT READY, zero cross-family contradictions;
  artifacts in `llp/reviews/`, round-1 follow-up sections). The two
  convergent materials: (1) the identity algebra unified into one
  vocabulary — kernel `ProducerId` at attach, Acto's LLP 0374 authority
  id joined at the Acto boundary, `ExecutionGeneration` a separate
  field, alloc-gen ownership/carrier/bump rules decided, WS-D restated
  in six-field terms, the carrier matrix promoted to Week 0, the owed
  0488 W4 amendment recorded and landed in lockstep; (2) §7.4 — the
  owed 0492 performance-ownership transfer, with OQ10's criterion
  repaired and 0492 r4 landing in lockstep. Codex's further materials
  all folded: the Phase-1 adapter circularity, executable recovery
  classification with current-family assignments, domain-qualified
  WS-B defaults, the canonical `exwfSchemaDigest` preimage, the
  0496-enforceable ledger row schema plus the generated Week-0 break
  package, and the SemanticFrame one-way crash-capsule-v1 projection
  with the unkeyed-digest prohibition. Minors folded per the Revised:
  header entry. Owner decisions (OQ7/OQ8/OQ11 and the recorded closes)
  untouched.

- **r14 (2026-08-20)** — post-loop fold of the round-2 dual-family
  materials (both families NOT READY on r13), authorized by the author
  under the program loop's disposition A; no reviewer has seen this
  revision (the 0487 r7/r10 precedent). The full change inventory is
  the Revised: header entry above; the loop artifacts are
  `llp/reviews/0491-kernel-refresh-program.{codex,grok}.md` (round-2
  sections). One-line summary: identity algebra unified and its two
  aliasing holes closed; the relayout number re-homed to RFC 0492;
  defaults, recovery scope, the crate DAG, SemanticFrame privacy,
  performance-exit precommitment, the EXWF digest carrier, the 0486
  instrument prerequisites, entry-security tests, non-waivable ledger
  rows, the dependency-domain reservation, and the 0490/0500 deference
  framings all landed per the reviews.

- **r13 (2026-08-20)** — program super-refine round 1 (the LLP 0478–0504
  loop; codex gpt-5.6-sol@ultra + grok-4.6@xhigh, both NOT READY on
  r12, zero cross-family contradictions). The four convergent
  materials: (1) §7.3 records the owed RFC 0490 amendment restating its
  three "ride WS-C" sentences as one encoding family / three delivery
  owners; (2) one App-ABI rule — EXWF and App-ABI are independent
  compatibility dimensions, the unconditional same-rev bump is deleted,
  pin refresh ≠ widening; (3) Phase 1's early `exact-motion` gate names
  its contents (SharedValue slab, closed-form evaluator, tick-agnostic
  phase API) so "compiles without exact-kernel" cannot be satisfied by
  a stub; (4) producer incarnation = LLP 0417 `ExecutionGeneration`,
  HotRevision patches never bump it, root incarnation on root-scoped
  reset — 0417/0500 added to Related. Codex's further materials all
  folded: §2 consumer-matrix alignment (supervisors/workers =
  transport/test, never rlib judges); the WS-A identity-carrier matrix
  (handlers, events, presenter resources, motion/graphics carriers,
  selection wire identities, FFI holders) plus globally-unique
  incarnation minting for simultaneous roots; hop caps kept as bounded
  traversals or reviewed depth; the Phase 2 machine-readable ledger
  with riders (0493 props/events, reserved `token-ref u16`, 0485
  integration amendment + plan regeneration/rejection rule, 0486 grid
  amendment as prerequisite), the per-host transactionality matrix
  (Android quarantined until its row passes), and the 0495 A-B
  receipt-schema-before-trace-deletion ordering with causal-DAG
  ordering semantics; WS-G's published/checked crate DAG, the
  selection-sha2 ruling (digest projection behind `exact-kernel-abi`;
  recorded fallback), and 0498 AQ-B exits at Phase 4; executable Phase
  1/Phase 5 gates (executor coverage matrix; pinned generator/statistic
  benchmark spec); SemanticFrame ruled the owned persisted
  flight-recorder format, registered at Week 0, legacy oracle frozen
  content-addressed; EXNODE end-to-end latency budget at Phase 3;
  explicit unverified-Ibex inventory row; Aquifer receipt non-knowledge
  rule. Minors: evidence-bullet cite corrections (protocol/ paths,
  destroy_view deliberate-survival nuance, JSONSerialization scoping,
  expect-count phrasing, Android decoder nuance), Week-0 OQ4 leftover,
  WS-E abort wording, WS-B one-file hedge and Kotlin-decoder mention,
  §2 byte-compare phrasing, 0258 supersession note, 0487 "frozen"
  phrasing, WS-H Taffy assumption named, OQ8 discriminant reservation.

- **r12 (2026-08-20)** — extension round authorized by the author.
  Applied: all round-3 materials (codex 8, grok 3, deduplicated) — the
  identity-namespace tuple + three-rule Acto split; the acknowledged
  multi-root recovery state machine as the Phase 2 deletion gate; the
  `SemanticFrame` IR replacing the insufficient `OpEvent` tape; the RTL
  truth table (fixing r11's double-reverse defect); the portable grid
  grammar as a joint 0486 profile amendment; per-dimension
  compatibility epochs with App-ABI decoupled from EXWF; the corrected
  consumer matrix (supervisors/workers = transport-only; EXNODE =
  one schema, two projections, Windows keeps typed rows); Android
  closed coherently (OQ4 closed: JNI typed-ops, EXNODE-exempt, Kotlin
  decoder deleted); kernel-scoped abort boundary; the early
  `exact-motion` gate in the phase table; per-phase baseline ratchets;
  the relayout timing boundary; 0297 OQ8 closed before schema freeze.
  Folded the author-relayed RFC 0498 §8 ask on the r8 precedent:
  `exact-aquifer` joins WS-G (wasm-day-one, plan-runner-consumed,
  never depends on exact-kernel) and Aquifer write outcomes/cell
  transitions join WS-C as receipt producers (RFC 0495 reads them as
  causal evidence). RFC 0492/0495/0498 now exist and are cited by
  number; the 0491 numbering collision resolved in this document's
  favor (contract-regions → LLP 0501).

- **r11 (2026-08-20)** — super-refine round 2 applied (both families
  NOT READY on r10: grok 2 materials, codex 11, overlapping on 0490
  conformance and the oracle). Conformed to **Accepted RFC 0490 r5**
  (ruling-ownership language; M3 blocked both legs; WS-B(4) clock
  authority pulled forward for M2); Exact Native boundary restored to
  LLP 0331's layers (rlib = host-tier; producers stay behind the
  App-ABI — verified: `exact-native-ui` disables the kernel feature);
  event payload-family schemas become inventory authority before the
  break (the "fixed shapes exist" claim was false — names/IDs only);
  RTL becomes exhaustive direction lowering (main-axis rewrite,
  alignment remap, grid inline axis, preserved vertical-edge mapping
  with its pinned tests); OQ3 closes in Phase 1 behind a
  failure-injection atomicity gate; the oracle re-based on semantic
  tapes plus a machine-readable delta ledger; Phase 5 gains registered
  pass/fail exit benchmarks; onboarding frozen through Phase 5; Android
  disposition decided (JNI + typed ops into the Kotlin applier); WS-B
  scoped to LLP 0150's generated-data rule (engine stays hand-authored;
  profile stays a separate algorithm-free file); `exact-motion`
  extraction becomes an early parallel track; Swift-evaluator deletion
  behavioral-evidence-gated; OQ11 relocated into §8; Percent softened
  to representation cleanup; leftover "wire v2" naming and the O(N²)
  claim corrected.
- **r10** — round 1 applied (both NOT READY on r9); ten materials fixed:
  §7.1 scoped, EXWF naming + capability-context identity, WS-C
  three-owner split + feed authority, Phase 2 WS-I input list,
  corpus-adapter + Week-0 baseline instruments, RTL rescope, WS-E
  globals completed, testId multimap, §5 dependency graph + Phase 5.
- **r9** — blind-spot sweep: RTL verified inert; serialization
  preserve-note; flight recorder; OQ11 (Expose trust).
- **r8** — RFC 0492 as consuming program: tick-source-agnostic clock,
  wasm-compilable `exact-motion`, claimable budgets, motion-validation
  deletion-bound.
- **r7** — OQ9 placed under LLP 0482; work-grain rule; in-kernel process
  parallelism rejected.
- **r6** — animated layout: WS-H ≤1 ms requirement; OQ10 tier ruling.
- **r5** — 0486–0489 bound in: WS-B consumes the layout-profile
  authority; 0487 corpus as regression instrument; lane coordination.
- **r4** — parallelism posture: single-threaded by contract,
  parallel-ready by construction.
- **r3** — Facet/Acto factored in: Acto a first-class consumer; OQ8.
- **r2** — Exact Native factored in: App-ABI coordination under M5-STOP
  governance; wasm-profile tie.
- **r1** — initial draft from the same-day six-lane kernel architecture
  review and its RFC 0490 addendum. Review report:
  https://claude.ai/code/artifact/03f39e3b-6320-4bd1-b8fc-31e16c48db6f
