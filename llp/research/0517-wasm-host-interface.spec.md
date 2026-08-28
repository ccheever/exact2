# LLP 0517 — The Wasm Host Interface

**Type:** Spec
**Status:** Accepted (2026-08-23, ratified by Charlie Cheever — decision relayed via orchestration session exact-9e; ratification of r3 ratifies the four `OI-WASM-*` closures as normative)
**Systems:** Web, Runtime, Kernel, Contract, Verification
**Author:** Charlie Cheever / Codex
**Date:** 2026-08-20
**Revised:** 2026-08-23 (acceptance — mechanical status edit under the author's decision: Status Draft → Accepted, ratified by Charlie Cheever 2026-08-23 (decision relayed via orchestration session exact-9e); the r3 "acceptance-ready pending author ratification" markers updated to record ratification; the four `OI-WASM-*` closures (§8.2.1 boot config, §8.3.1 result header, §8.6 symbol spellings, §8.7 clock/timer carrier) are now ratified normative resolutions, unblocking M2 gate registration against the pinned names, layouts, and signatures — LLP 0504 §3 row 28 / §4 M2-ABI clock; the gate registration itself remains open work for the M2 lane.) 2026-08-23 (r3 — all four `OI-WASM-*` blockers carry proposed closures grounded in the landed M1/Track-R code (§8.2.1 boot config, §8.3.1 result header, §8.6 symbol spellings, §8.7 clock/timer carrier), adversarially checked against the named code paths by a codex xhigh review round with all verified findings folded; acceptance-ready pending author ratification) 2026-08-23 (§13 item 10 verified and closed against landed format-schema v1.5's `v15NullableRule` and 0508 §9's generated-shapes marker text — the 0504 §3 row 84 verify-and-close, LLP 0551 finding 27, executed under 0551 §2.3; no normative change) 2026-08-21 (LLP 0530 fold wave: unreviewed mechanical edits under the author-decided batch of 2026-08-21) 2026-08-21 (r2 — shared byte surfaces closed; wasm-only ABI issues named as M2 registration blockers)
**Related:** LLP 0483 §2 (same Rust plan engine via wasm and its web cutover gates), RFC 0478 D13/D14(4) and LLP 0505 r3 row 1 (one engine; declared synchronous app-TS seam; destination web-speed ownership), LLP 0508 §§4/6/10 (value and execution semantics), LLP 0485 §3/§10 (plan container and capability declarations), LLP 0507 (EXWF wire; this spec owns embedding, never host-wire semantics), LLP 0512 §4 (WebGPU/browser-surface passthrough), LLP 0297 §4.2/§7 W0 (affinity registration and no cross-queue synchronous wait), LLP 0288 (Contract is the real-DOM web production target), LLP 0514 M1/M2 (native specialization and wasm milestone)

## Abstract

This Draft owns two layers that must not drift apart:

1. the **target-neutral Plan Host Interface family** used when a Contract plan calls a declared app TypeScript function, reads a reactive host value, or dispatches a governed capability; and
2. the **wasm/page-JS embedding** that carries that family when Exact's one Rust plan engine runs in a browser and drives the real-DOM host.

The shared layer defines descriptor identity, the closed value model, synchronous-call and failure semantics, re-entry rules, copied-value and memory-lifetime policy, reactive publication, capability outcomes, and one benchmark result schema. The wasm layer defines the proposed imports/exports, linear-memory ownership, page-agent posture, admission/version refusal, and DOM-host boundary.

This document is **Accepted** (ratified by Charlie Cheever, 2026-08-23, at r3). Acceptance ratifies the interface family and the four `OI-WASM-*` closures; it does not claim an implemented M2 path or benchmark success — those remain gated milestones. Native M1 work may use the shared names, but native C/Swift/Hermes mechanics remain owned by the M1 design and do not make this wasm ABI stable by precedent beyond what acceptance pins.

## 1. Commission and boundary

Charlie commissioned LLP 0517 on 2026-08-20 after the holistic corpus review found the wasm embedding seam unowned. Until this draft exists, no landing may present a de-facto wasm embedding as stable. Until this draft is accepted and its gates pass, no web cutover may claim the same Rust engine is production-ready.

**Staging (LLP 0530 §3.1, decided by Charlie 2026-08-21):** the two
layers stage separately — the target-neutral Plan Host Interface
family (§§3–7, §§9–11) is 0514 **M1** scope; the wasm/page-JS
embedding (§8) stays **M2**. M1's native leg consumes the PHI layer
without the wasm embedding.

**Wasm composition note (LLP 0530 §2.6):** three wasm mechanisms in
the corpus are distinct and share nothing unnamed — this spec's
plan-engine wasm embedding; 0524's reload-backend wasm (a per-role
gated hot-reload instantiation backend); and 0529's canvas
worker-like native source profile. A shared toolchain or runtime
between any two is a future named decision, never an inference.

This spec owns:

- the names and semantics of `PlanHostInterfaceV1`, `PlanHostImportDescriptorV1`, `ContractHostValueV1`, `PlanHostCallOutcomeV1`, `PlanHostDiagnosticV1`, `PlanReactiveSnapshotV1`, `PlanCapabilityOutcomeV1`, and `PlanHostBenchmarkResultV1`;
- semantic synchronous invocation of declared app-TS imports;
- typed failure and effect-disposition behavior;
- the no-nested-plan-mutation rule and top-level-job publication rule;
- copied-value, borrow, release, size/depth, and alias policy;
- wasm exports/imports, linear-memory transfer, provider-result tokens, page-agent topology, embedding version/admission, and benchmark conformance;
- the boundary by which plan-engine output reaches the existing real-DOM host.

It does not own:

- `.eplan` tables/opcodes or plan admission, owned by the plan authorities;
- capability semantics or grants, owned by their capability authorities;
- EXWF frames, presenter semantics, or DOM mutation meaning, owned by LLP 0507 and the web host;
- Contract language types, match, reducers, or scheduling, owned by LLP 0508 and incorporated authorities;
- the browser's WebGPU surface, which remains a pass-through per LLP 0512;
- native C layouts, Swift ownership, Hermes/HBC symbols, AppKit wiring, or `ExactRuntimeThreading`, owned by the LLP 0514 M1 native-leg design.

## 2. Conformance language and classes

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are interpreted as RFC 2119 requirements for an implementation claiming this Draft revision. A repository implementation MUST NOT advertise stable LLP 0517 conformance while the document is Draft.

Conformance classes are:

1. **Interface producer** — emits canonical descriptors and their digest.
2. **Plan engine** — evaluates a plan and invokes the abstract interface.
3. **Provider adapter** — maps admitted selectors to app/framework JS and encodes outcomes.
4. **Embedding** — binds one engine instance, provider adapter, host, memory, and lifecycle.
5. **Benchmark producer** — emits `PlanHostBenchmarkResultV1` under a predeclared environment and bound.

A conformance claim names its class, interface major, transfer profile, adapter (`native-hermes` or `wasm-page-js`), plan/interface/provider digests, and the registered checks that produced it.

## 3. Names, identity, and version family

The shared V1 family starts with the concrete version tuple `(major=1, minor=0, minReader=1)`. `minReader` is the minimum reader **major**, so a V1 reader accepts V1.0 and later V1 minors only when every unknown addition is optional and independently framed. A compatible optional addition increments `minor` and leaves `major=1,minReader=1`; changing an existing tag, discriminant, field meaning, required framing, canonical order, or failure/effect code requires a new major and raises `minReader` to that major. No revision retroactively reinterprets bytes emitted under an earlier tuple.

### 3.1 `PlanHostInterfaceV1`

`PlanHostInterfaceV1` is the sorted set of declared host imports, capability requirements/fingerprints, and declared environment-read requirement kinds reachable from one admitted plan. Its canonical digest is `seamInterfaceDigest`. The digest excludes provider route classes/selectors and concrete static or generation-scoped environment operands; each embedding manifest admits and binds those separately. Native EPPM and the proposed wasm manifest may therefore bind different providers/operands to the same plan interface without changing its identity.

The interface contains no physical thread or queue identity. Each embedding separately declares callback affinity under LLP 0297; affinity is admission/runtime evidence, not cross-target descriptor identity.

### 3.2 `PlanHostImportDescriptorV1`

Each descriptor contains:

| Field | Meaning |
| --- | --- |
| `hostImportId` | stable identity derived from canonical provider module identity, export name, and binding kind |
| `kind` | `sync-function` or `reactive-value` |
| `purity` | `pure` in V1; effects are capabilities, not mislabeled functions |
| `reactivity` | `none` or `reactive` |
| `transferProfile` | `contract-v1-data` |
| `interfaceMajor` | exact supported major, initially `1` |
| `providerModule`, `exportName` | canonical selector identity inputs, never absolute paths |
| `params` | ordered name/type/projection/declared-default entries |
| `resultType` | one closed Contract type |
| `environmentReads` | generated closed set of declared environment-read requirement kinds; concrete operands are embedding-bound |
| `descriptorDigest` | domain-separated digest of all semantic fields above |

`descriptorDigest` excludes row indices, traversal order, comments, physical affinity, provider artifact bytes, and target-specific selectors. `providerArtifactDigest` elsewhere means the digest of the exact admitted provider bytes for that embedding—HBC on the proposed native arm, a page-JS artifact on the wasm arm—never “HBC” as a cross-target semantic type.

Repeated imports of the same canonical `(provider module, export, kind)` share one descriptor. An import binding or table row is not durable provider identity.

### 3.3 Semantic operations

The family has four operations:

1. `callSync` — invoke one declared synchronous function with ordered Contract arguments and receive one value or typed failure.
2. `readReactiveSync` — read one declared reactive value as a full `(hostImportId, version, value)` snapshot.
3. `publishReactive` — submit a later full snapshot as a new top-level engine job.
4. `dispatchCapability` — after a committed transition, invoke an admitted `(capabilityId, major)` route and receive an effect disposition plus dirtied reactive IDs; it returns no program value.

Plan-byte ingress is a separate embedding operation. EXWF output is a separate host boundary. Neither is disguised as a host import.

## 4. `ContractHostValueV1`

### 4.1 Closed tags

JSON is not the host-import value ABI. `ContractHostValueV1` is the following canonical recursive value:

| Tag | Value and rule |
| ---: | --- |
| `0` | reserved; receipt of JS/engine `undefined` is `host-value-unsupported` |
| `1` | boundary `Null`; legal only during ingress when the expected boundary type carries LLP 0508's existing IR `nullable` marker, then immediately normalized to `Option.none` |
| `2` / `3` | `Bool(false)` / `Bool(true)` |
| `4` | `Num`, eight raw IEEE-754 binary64 bits, preserving NaN payload, infinities, and negative zero |
| `5` | `Str`, observable UTF-16 code units including isolated surrogates |
| `6` | `Array`, ordered homogeneous values under the declared type |
| `7` | `Object`, insertion-ordered UTF-16 keys and closed always-present fields |
| `8` | `Option.none`, no payload |
| `9` | `Option.some`, exactly one encoded non-Option payload |

No `Option<Option<T>>`, function, component, behavior, proxy, symbol, sparse array, accessor, custom prototype, or cycle crosses this boundary. Callable identity remains in descriptors. Authored/program `null` and observable `undefined` do not exist.

### 4.2 Canonical bytes

Each value begins with its one-byte tag. Integers/counts are little-endian. Strings are `u32 codeUnitCount` plus exactly `count * 2` UTF-16LE bytes. Arrays are `u32 elementCount` plus that many values. Objects are `u32 entryCount` plus entries in observable insertion order; each key is `u32 codeUnitCount + UTF-16LE` without a value tag, followed by one encoded value. Tag `9` is followed directly by its payload. There is no padding inside a value.

A decoder uses checked arithmetic, validates the exact expected closed type, and requires exact buffer consumption with no trailing bytes. It preserves object insertion order; it does not sort keys in the value codec.

### 4.3 Envelopes

- `ContractHostArgumentsV1` is `u32 argumentCount` followed by that many values. It distinguishes N arguments from one array.
- A successful function result is exactly one value.
- `PlanReactiveSnapshotV1` carries one value plus a nonzero `u64 version` in its transport header.
- `PlanCapabilityOutcomeV1` is `(effectDisposition:u8, reserved[3]=0, count:u32, hostImportRef[count]:u32)` and carries no value.
- `PlanHostDiagnosticV1` is `(code:u16, effectDisposition:u8, reserved:u8=0, message, stack)` where each string is `u32 byteLength + UTF-8 bytes`, has a maximum byte length of 65,536, and must be consumed exactly. Invalid UTF-8, a length mismatch, trailing bytes, or a larger string is malformed. Message/stack are diagnostic-only and redacted by host policy.

The shared `effectDisposition:u8` table is closed:

| Value | Name | Meaning |
| ---: | --- | --- |
| `0` | `none` | no governed effect was attempted; required for pure call/read outcomes |
| `1` | `not-started` | the capability did not begin an external effect |
| `2` | `committed` | the provider reports that the external effect committed |
| `3` | `outcome-unknown` | the provider cannot prove whether the external effect committed |

`PlanCapabilityOutcomeV1` requires `1`–`3`; `0` is invalid there. Its `hostImportRef` rows are strictly increasing and duplicate-free. A pure call/read diagnostic requires `none`; a capability diagnostic requires the effect disposition consistent with its code. Unknown disposition bytes are `host-value-malformed`.

`PlanHostCallOutcomeV1` has one exact byte framing. Its eight-byte little-endian header is `(discriminant:u8 @0, flags:u8=0 @1, reserved:u16=0 @2, payloadBytes:u32 @4)`, immediately followed by exactly `payloadBytes` bytes and no trailing material. The discriminants are `success=0`, `refused=1`, `threw=2`, and `provider-fault=3`. `success` contains exactly one `ContractHostValueV1`; every other variant contains exactly one `PlanHostDiagnosticV1`. Nonzero reserved or flag bytes, an unknown discriminant, a payload-length mismatch, a variant/payload mismatch, or trailing bytes is `host-value-malformed`. A transport failure that occurs before this complete header and payload exist is not a malformed fifth semantic variant; it is the embedding transport failure described in §5.3.

### 4.4 Bounds and projection

The `contract-v1-data` profile bounds each value and each complete arguments/call/reactive/capability envelope to 16 MiB encoded bytes, depth 64, and 100,000 aggregate array/object entries. Diagnostic strings retain the tighter per-string bound above. Exceeding a bound fails before unbounded allocation growth.

`InteractionSnapshotV1` is the shared projection for a plan-native Facet interaction passed to a declared recipe: `{pressed:boolean, hovered:boolean, focused:boolean, focusVisible:boolean, state:"idle"|"pressed"|"hovered"|"focused"}`. No behavior proxy, handler, `force()` method, or identity crosses the seam.

Boundary normalization uses **only** LLP 0508's existing IR `nullable` marker. `null` or representable missing legacy input becomes `none`; defined input becomes `some(v)`. This spec creates no parallel boundary-optional bit. If a generated field's missing-vs-required distinction cannot be expressed by the accepted IR, that field refuses pending a 0508 amendment.

## 5. Calls, failures, and re-entry

### 5.1 Synchronous call law

For `callSync`, the engine evaluates arguments left-to-right, validates arity and types, encodes one arguments envelope, and invokes the provider exactly once. On success it validates the result against the descriptor before making it observable. A provider adapter MUST NOT construct/evaluate source text per call.

The operation is **semantically synchronous**: evaluation resumes with a value or failure before the calling expression continues. This requirement does not prescribe a native runtime thread, browser worker, queue, or physical owner. An embedding MUST declare that separately and MUST NOT implement sync semantics by blocking another queue with a semaphore/`Atomics.wait`-style bounce.

Pure imports do not mutate provider state, schedule work, read undeclared ambient inputs, or invoke plan mutation. The engine does not cache, reorder, deduplicate, or retry calls merely because the descriptor says `pure`.

### 5.2 Re-entry

Provider JS may call ordinary helpers inside its admitted realm. It MUST NOT synchronously re-enter a plan mutation, event, reactive-publication, lifecycle, or scheduler entry while a call/read/capability callback is active. Such entry returns `host-import-reentrant` before mutation.

An invalidation observed during provider execution is recorded/coalesced and enters `publishReactive` only as a **new top-level job after the callback returns**. It never publishes inline. Capability dirtiness follows the same rule.

### 5.3 Failure model

The transport distinguishes success, typed refusal, caught provider throw, provider/ABI fault, and transport failure before a valid outcome exists.

`PlanHostDiagnosticV1.code:u16` uses this closed shared table. `0` and unlisted values are invalid in V1; additions require the version evolution rule in §3.

| Value | Stable name |
| ---: | --- |
| `1` | `host-import-missing` |
| `2` | `host-import-version` |
| `3` | `host-import-descriptor-mismatch` |
| `4` | `host-import-environment` |
| `5` | `host-import-environment-range` |
| `6` | `host-import-thread` |
| `7` | `host-import-reentrant` |
| `8` | `host-import-arity` |
| `9` | `host-import-argument-type` |
| `10` | `host-import-result-type` |
| `11` | `host-value-unsupported` |
| `12` | `host-value-cycle` |
| `13` | `host-value-limit` |
| `14` | `host-value-malformed` |
| `15` | `host-import-ts-throw` |
| `16` | `host-import-provider-fault` |
| `17` | `host-capability-denied` |
| `18` | `host-capability-outcome-unknown` |

The `threw` outcome discriminant requires code `15`; `provider-fault` requires code `16`. Code `17` requires `not-started`, and code `18` requires `outcome-unknown`. `host-import-thread` applies only when an embedding has an affinity contract. Target-specific plan, wasm, native-ABI, EXWF, capability-authority, and environment-container refusals remain in their owning code spaces and do not acquire ad hoc values in this table.

A thrown JS value is caught in the provider adapter. Only its bounded/redacted diagnostic projection crosses; raw throwable identity, getters, prototype, and stack object do not.

Failure continuation follows the plan transition:

- boot/initial-fixed-point failure prevents publication;
- a pure-call failure during an event/timer aborts the transition, unwinds its journal, discards unrun commands, records the locus, and leaves the engine available unless integrity failed;
- a capability failure is post-commit and reports `not-started`, `committed`, or `outcome-unknown`; committed state is never rolled back;
- malformed descriptors/payloads, target-affinity violations, and invariant faults fence the generation and require relaunch.

No failure becomes `undefined`, a Rust panic, an unwind across an FFI boundary, automatic fallback to another engine, or a silently omitted style/prop.

## 6. Reactive values and capability outcomes

`readReactiveSync` returns a full copied snapshot and a nonzero monotonically increasing version. `publishReactive` also carries a full snapshot, never a delta. A version greater than the current version is accepted after descriptor/type validation even when intermediate versions were coalesced; stale/duplicate versions are ignored with a receipt.

The initial reactive read occurs before the plan generation is published. A reactive dependency is an ordinary plan dependency; accepting a newer snapshot dirties and evaluates it under the engine's normal fixed-point/commit rules.

A capability call is identified by `(capabilityId, major, callId)`. It is admitted before evaluation, queued by the transition, and dispatched only after state commits. A successful outcome contains an effect disposition and sorted/canonical dirtied reactive selectors, not a Contract value. An idempotent provider MAY deduplicate the same nonzero `callId`; the engine MUST NOT blindly retry a general effect.

## 7. Memory and copy policy

### 7.1 Target-neutral ownership

Requests are engine-owned bytes borrowed only for the synchronous callback. Results/diagnostics are provider-owned bytes represented by an opaque result token or native owner handle until copied/validated by the engine. Every acquired result is released exactly once on success, validation failure, or contained unwind. Neither side retains the other's pointer/view after return.

Values cross by copy. Repeated acyclic references are encoded repeatedly as independent equal values; A-VAL makes interior aliases unobservable. Cycle detection uses the active recursion stack, not a global “seen once” set. Implementations may use internal copy-on-write only when observably equivalent.

Every benchmark reports request/result bytes copied and adapter-visible allocations. A future zero-copy profile requires a new named transfer profile and compatibility argument; it cannot silently weaken `contract-v1-data`.

### 7.2 Wasm32 linear-memory policy

The proposed V1 wasm adapter uses exported wasm32 linear memory and provider-result tokens:

1. the engine allocates/encodes request bytes in its memory and calls a JS import with `(ptr,len)`; JS borrows that view only during the import and must reacquire `memory.buffer` on every entry because memory growth can detach an older buffer;
2. JS invokes the provider once, encodes the complete outcome into host-owned bytes, stores it under a generation-scoped nonzero result token, and writes only a fixed result header into guest memory;
3. the engine allocates an exact-size destination and calls `exact_host_copy_result_v1(token,dst,capacity)` — in V1 exactly once, with `capacity` equal to the header's payload length (the one-shot arm pinned in §8.3.1) — without re-invoking provider JS;
4. the engine calls `exact_host_release_result_v1(token)` exactly once, including validation-failure paths.

The host must not retain a `Uint8Array`, pointer, or decoded guest object past the import. The guest must not treat a result token as a Contract value or retain it after release. Unknown/stale/generation-mismatched/double-released tokens refuse. Memory64 and shared-memory profiles require a later revision.

## 8. Proposed wasm/page-JS embedding

### 8.1 Version and admission

The embedding ABI starts at the concrete tuple `(major=1, minor=0, minReader=1)` and follows §3's evolution rule independently of the shared interface tuple. At instantiation, JS compares that exact exported tuple, plan/container/opcode digest, `seamInterfaceDigest`, exact descriptor/capability set, environment operands, and provider source/artifact/compiler digests. A mismatch refuses before provider evaluation or DOM mutation.

The exact admitted page-JS provider artifact is prebundled and digest-bound. No per-call eval, dynamic source-string compilation, ambient module lookup, or “find a function with this name” fallback is admitted. CSP/wasm availability and compile/instantiate latency are explicit M2 gate evidence.

### 8.2 Exports

The wasm module exports these V1 entry families. As of r3, the spellings in this table and §8.3's are the **accepted table** that `OI-WASM-SYMBOL-SPELLINGS` required (the atomic replacement of the r2 provisional names — which it left unchanged — plus the three additions below); §8.6 pins the naming law:

| Export | Purpose |
| --- | --- |
| `memory` | engine linear memory |
| `exact_plan_alloc_v1(len) -> ptr` / `exact_plan_dealloc_v1(ptr,len)` | bounded host-to-guest ingress allocation |
| `exact_plan_embedding_version_v1(outPtr) -> status` | write ABI major/minor/minReader and feature bits |
| `exact_plan_boot_v1(configPtr,configLen) -> handle` | admit plan/provider/environment and boot one engine generation through event admission (`automatic-v1`); the config bytes are one `ExactWasmPlanBootConfigV1` (§8.2.1, closing `OI-WASM-BOOT-CONFIG`) |
| `exact_plan_boot_diagnostic_v1(dst,capacity) -> lenOrStatus` | copy the most recent boot-refusal diagnostic (§8.2.1) |
| `exact_plan_dispatch_event_v1(handle,ptr,len) -> status` | typed event ingress |
| `exact_plan_publish_reactive_v1(handle,hostImportRef,versionLo,versionHi,ptr,len) -> status` | new-top-level full reactive publication |
| `exact_plan_take_output_v1(handle,resultHeaderPtr) -> status` | expose the next engine-owned EXWF/diagnostic output token |
| `exact_plan_copy_output_v1(token,dst,capacity) -> status` / `exact_plan_release_output_v1(token)` | bounded page-host copy/release |
| `exact_plan_shutdown_v1(handle) -> status` | consuming generation shutdown after validation/fencing |
| `exact_plan_wake_scheduler_v1(handle,wakeTokenLo,wakeTokenHi) -> status` | admitted physical-wake ingress (§8.7) |
| `exact_plan_notify_clock_lifecycle_v1(handle,lifecycleKind) -> status` | clock lifecycle ingress (§8.7) |

Config/event/reactive bytes use their owning versioned formats. These exports do not define EXWF semantics or DOM operations.

### 8.2.1 Boot configuration: `ExactWasmPlanBootConfigV1` (closes `OI-WASM-BOOT-CONFIG`)

The bytes consumed by `exact_plan_boot_v1` are one **`ExactWasmPlanBootConfigV1`** document. It is an independent versioned family under §3's evolution rule — magic ASCII `EWBC`, concrete tuple `(major=1, minor=0, minReader=1)` — owned by this embedding the way `EPPM`/`EENV` are owned by their authorities. It is not an `.eplan` segment, and it deliberately does **not** inherit the native `CnHostPlanBootConfigurationV1` C layout: the native struct's borrowed-pointer/vtable fields have no meaning in a serialized guest-memory document, and the native staged/task-control family is not carried over by analogy.

All integers are little-endian. The document is:

1. a fixed 24-byte header — `magic[4]="EWBC"`, `formatMajor:u16=1`, `formatMinor:u16=0`, `minReaderMajor:u16=1`, `reserved:u16=0`, `headerBytes:u32=24`, `sectionCount:u32`, `totalBytes:u32` (which MUST equal the `configLen` argument);
2. a section table of exactly `sectionCount` 16-byte entries — `(kind:u16, flags:u16, reserved:u32=0, byteOffset:u32, byteLength:u32)` — sorted strictly ascending by `kind` with no duplicates. The reader MUST verify `totalBytes >= headerBytes + 16*sectionCount` under checked arithmetic **before** decoding any entry. Flag bit 0 means *reader-optional*: a V1 reader MUST refuse an unknown kind with bit 0 clear and MUST skip an unknown kind with bit 0 set (this is how a compatible minor adds material under §3); all other flag bits MUST be 0;
3. the section payloads, **canonically packed**: the first section's `byteOffset` equals `headerBytes + 16*sectionCount`, each subsequent section's `byteOffset` equals the previous section's `byteOffset + byteLength` (a zero-length section carries the cursor offset), and `totalBytes` equals the last section's end. There are no gaps, no padding, and no trailing bytes, so one configuration has exactly one byte representation.

The V1 section kinds, all required and `flags=0`:

| Kind | Name | Payload |
| ---: | --- | --- |
| `1` | `plan-bytes` | the complete `.eplan` container, nonempty; validated by the plan authority |
| `2` | `provider-manifest` | the `EPPM` bytes, **nonempty in every boot**: the canonical no-provider plan is expressed *inside* EPPM per its authority (provider-presence flags and empty route tables), never by an absent manifest — the same rule the landed native boot enforces |
| `3` | `environment` | the `EENV` bytes, **nonempty in every boot**: EENV binds more than environment operands (viewport, appearance, validity interval, RNG seed), so it is unconditionally required, as on the landed native path. The engine cross-checks the kind-`7` viewport width against EENV's `viewportWidth` exactly as native admission does |
| `4` | `initial-props` | one encoded `ContractHostValueV1` closed `Object` validated against the plan's entry-prop type descriptor; zero length means no props (the same contract the native plan boot applies) |
| `5` | `root-id` | nonempty UTF-8, at most 128 bytes, drawn from the landed state-bridge root-identifier alphabet (ASCII alphanumerics and `.`, `_`, `:`, `/`, `-`, `#`); this reuses the existing root-identifier authority rather than minting a looser one |
| `6` | `hydration` | one nonempty canonical Contract state-bridge document in its owning format (schema 1, or schema 2 with the seed cursor when the plan admits seeded RNG); a cold launch supplies the canonical empty document — zero length is a refusal, as on the landed native path |
| `7` | `viewport` | exactly 8 bytes: `width:f32, height:f32`, both finite and **strictly positive** (the landed admission rule); the width MUST equal EENV's `viewportWidth` |
| `8` | `admission-expectations` | exactly 128 bytes: four raw `digest32` values in order — `planDigest`, `opcodeTableDigest`, `seamInterfaceDigest`, `providerArtifactDigest` (the same 32-byte raw digest encoding the EENV/EPPM headers use). The engine cross-checks each against the artifacts it decodes, so §8.1's JS-side comparison becomes two-sided for the identities that would otherwise be compared only outside the engine. The remaining §8.1 comparisons — exact descriptor/capability sets, environment operands, provider source/compiler identity — are already bound *inside* the admitted plan/EPPM/EENV artifacts and are enforced by artifact admission itself |
| `9` | `generation-mode` | 4 bytes: `startMode:u8 = 0`, `reserved:u8 = 0`, `reserved:u16 = 0`. V1 admits exactly one mode, **`automatic-v1` (0)**: `exact_plan_boot_v1` performs admission, the initial fixed point, generation commit, task start, and event admission before it returns, and stages the first output for `exact_plan_take_output_v1` — the wasm embedding drives the engine's immediate-start/deferred-task machinery internally and exposes no separate commit/start exports. A staged or task-controlled web boot is a later minor or major of **this** format; the native mode enums are not imported |
| `10` | `host-features` | exactly 28 bytes: `manifestVersion:u32 = 1`, `protocolNodeTypeBits:u64`, `nativeViewModuleBits:u64`, `hostInputBits:u64` — the serialized form of the same closed v1 host-feature bitset authority the native C manifest carries (the values, not the C struct). The engine admits the plan's view tags and input/module demands against it exactly as landed native admission does; a plan demanding an unset bit refuses |

Bounds: `configLen <= 256 MiB` and `sectionCount <= 64`; every constituent format (plan container, EPPM, EENV, state bridge, host values) retains its own bounds and validators.

Ownership: the config bytes are guest linear memory (typically obtained via `exact_plan_alloc_v1`) borrowed by `exact_plan_boot_v1` only for the synchronous call. The engine copies or decodes everything the generation needs before returning and retains no pointer into the config; the caller then frees its own allocation.

At most **one live generation** exists per wasm instance: `exact_plan_boot_v1` while a handle is live refuses, and a consuming `exact_plan_shutdown_v1` retires the generation's result, output, and wake token registries atomically with it (a later copy/release against a retired token refuses without harm).

Refusal: any structural, version, bounds, digest, or artifact-admission failure returns handle `0`, installs no generation, and retains nothing. On that path the engine retains one most-recent **boot diagnostic**: a bounded UTF-8 JSON document of at most 65,536 bytes in the `wasm-boot-config` owning code space — the same JSON-diagnostic posture as the native boot's `diagnostic_json_out`, and deliberately **not** a `PlanHostDiagnosticV1` (whose §5.3 code table is closed to seam call/read/capability outcomes and does not admit boot codes). `exact_plan_boot_diagnostic_v1(dst,capacity) -> lenOrStatus` reads it: the read is non-consuming; each failed boot replaces the document and a successful boot clears it; `capacity = 0` is a size query that writes nothing; the return is the exact byte length when a diagnostic exists (the copy occurs only when `capacity >=` that length), `-1` when none exists, and `-2` when the destination range is invalid.

### 8.3 Imports

The page-JS adapter provides these functions, all under the wasm import module `"exact_host_v1"` (§8.6):

| Import | Purpose |
| --- | --- |
| `exact_host_call_sync_v1(bindingKind,bindingRef,callIdLo,callIdHi,argsPtr,argsLen,resultHeaderPtr) -> status` | declared pure function or routed post-commit capability call |
| `exact_host_read_reactive_sync_v1(hostImportRef,resultHeaderPtr) -> status` | initial/on-demand full reactive read |
| `exact_host_copy_result_v1(token,dst,capacity) -> status` | copy exact provider-owned bytes into guest memory |
| `exact_host_release_result_v1(token) -> status` | exactly-once result release |
| `exact_host_now_epoch_ms_v1(outPtr) -> status` | deterministic clock sample (§8.7) |
| `exact_host_arm_wake_v1(wakeTokenLo,wakeTokenHi,dueEpochMs) -> status` | install/replace the single one-shot physical wake (§8.7) |
| `exact_host_cancel_wake_v1(wakeTokenLo,wakeTokenHi) -> status` | cancel the armed wake (§8.7) |

The fixed result header carries ABI/size, semantic status, result kind, flags, payload length, result token, and reactive version. Its layout is pinned in §8.3.1 (closing `OI-WASM-RESULT-HEADER`); its offsets/widths are pinned there directly, never inferred from the native C struct. The clock/timer rows are pinned in §8.7 (closing `OI-WASM-CLOCK-TIMER-IMPORTS`).

### 8.3.1 The fixed result header: `WasmPlanResultHeaderV1` (closes `OI-WASM-RESULT-HEADER`)

**`WasmPlanResultHeaderV1`** is one fixed 32-byte little-endian record written into guest memory at an engine-supplied, 8-byte-aligned `resultHeaderPtr`. The same record serves both directions: page JS writes it for `exact_host_call_sync_v1` and `exact_host_read_reactive_sync_v1`; the engine writes it for `exact_plan_take_output_v1`.

| Offset | Width | Field | Rule |
| ---: | ---: | --- | --- |
| `0` | `u16` | `headerMajor` | `= 1` |
| `2` | `u16` | `headerBytes` | `= 32` |
| `4` | `u8` | `status` | host-import direction: exactly §4.3's outcome discriminants (`0` success, `1` refused, `2` threw, `3` provider-fault — the same values the native seam status uses); output direction: `= 0` |
| `5` | `u8` | `resultKind` | `1` contract-value, `2` capability-outcome, `3` diagnostic (the same values as the native result-kind projection), plus wasm-only `4` host-output |
| `6` | `u16` | `flags` | `= 0` |
| `8` | `u32` | `payloadBytes` | exact encoded payload length |
| `12` | `u32` | `resultToken` | nonzero token in the owning direction's token space |
| `16` | `u64` | `reactiveVersion` | nonzero exactly for a successful `exact_host_read_reactive_sync_v1`; `0` everywhere else |
| `24` | `u64` | `reserved` | `= 0` |

The payload named by `(resultToken, payloadBytes)` is the **bare** encoded body — never a `PlanHostCallOutcomeV1` envelope; the discriminant lives in this header. The header *projects* the §4.3 outcomes without being them, the same posture the native `CnHostPlanSeamResultV1` takes. Diagnostic ownership is split by direction: a host-import `resultKind=3` payload is one `PlanHostDiagnosticV1` (the §5.3 seam family); an output-direction `resultKind=3` payload is one bounded UTF-8 **plan-runtime diagnostic JSON document** in its owning code space — the same posture as the native `cn_host_copy_runtime_diagnostic` — because §5.3's closed table does not admit plan/scheduler/EXWF codes.

The `bindingKind` parameter of `exact_host_call_sync_v1` uses the native allocation: `1` host-import, `2` capability; any other value refuses before provider invocation.

The complete consistency matrix — every row not listed is `host-value-malformed` (or the applicable §9 token class):

| Direction / operation | `status` | `resultKind` | Payload | `reactiveVersion` |
| --- | ---: | ---: | --- | ---: |
| `call_sync`, `bindingKind=1` | `0` | `1` | one `ContractHostValueV1` | `0` |
| `call_sync`, `bindingKind=2` | `0` | `2` | one `PlanCapabilityOutcomeV1` (disposition `1`–`3` per §4.3) | `0` |
| `call_sync`, either binding | `1` | `3` | one `PlanHostDiagnosticV1`; code MUST NOT be `15`/`16`; a `bindingKind=1` diagnostic requires disposition `none`; a `bindingKind=2` diagnostic requires disposition `1`–`3` — exactly `1` for code `17`, exactly `3` for code `18`, any of `1`–`3` for the other codes (the landed validator's matrix) | `0` |
| `call_sync`, either binding | `2` | `3` | one `PlanHostDiagnosticV1` with code `15`; disposition `none` for `bindingKind=1`, `1`–`3` for `bindingKind=2` | `0` |
| `call_sync`, either binding | `3` | `3` | one `PlanHostDiagnosticV1` with code `16`; disposition `none` for `bindingKind=1`, `1`–`3` for `bindingKind=2` | `0` |
| `read_reactive_sync` | `0` | `1` | one `ContractHostValueV1` | nonzero |
| `read_reactive_sync` | `1`/`2`/`3` | `3` | as the `call_sync` rows above with disposition `none` | `0` |
| `take_output` | `0` | `4` | one complete host-output document in its owning versioned format (semantics stay with LLP 0507 and the web host) | `0` |
| `take_output` | `0` | `3` | one plan-runtime diagnostic JSON document (owning code space), at most 65,536 bytes — the boot-diagnostic bound; oversize refuses | `0` |

Unknown `status`/`resultKind`/`bindingKind` values, nonzero `flags`/`reserved`, matrix violations, a zero token, or `payloadBytes` exceeding its payload's owning bound refuse fail-closed. The `PlanHostDiagnosticV1` bound is exact: 4 fixed bytes plus two length-prefixed strings of at most 65,536 bytes each — a maximum encoded body of **131,084 bytes**; values and envelopes keep §4.4's 16 MiB bound.

Transport-vs-semantic separation applies to exactly three entries — `exact_host_call_sync_v1`, `exact_host_read_reactive_sync_v1`, and `exact_plan_take_output_v1`: a return of `0` means a complete header and outcome exist; `exact_plan_take_output_v1` alone adds `1` = no output pending (header not written); a negative return is the §5.3 transport failure — no valid outcome exists and the engine fences the generation, retiring every outstanding token of that generation with it. All other entries carry their own pinned returns (§8.2.1 boot/diagnostic; §8.7 scheduler; and below).

Copy and release are pinned, both directions symmetric:

- `exact_host_copy_result_v1(token,dst,capacity)` / `exact_plan_copy_output_v1(token,dst,capacity)` — V1 selects the **one-shot exact-size copy** arm of §7.2 (no chunk cursor exists in a V1 signature): `capacity` MUST equal the header's `payloadBytes`. Returns `0` copied, `1` unknown/stale/released token, `2` capacity mismatch, negative an integrity fault. Copy is non-consuming and repeatable while the token is live.
- `exact_host_release_result_v1(token)` / `exact_plan_release_output_v1(token)` — the sole consuming attempt: `0` released, `1` unknown/stale/double-release refusal, negative fault; a nonzero return is reported and never retried (the native release law).
- Tokens are never reused within their generation (monotonic allocation on each side); 32-bit token exhaustion inside one generation is an integrity fault that fences it.
- `exact_plan_alloc_v1` guarantees at least 8-byte alignment for every allocation and returns `0` on exhaustion (a refusal, not a trap); every `resultHeaderPtr` and clock `outPtr` in this ABI is therefore 8-aligned.

Memory-growth discipline binds the writer: any provider execution or guest re-entry can grow (and thereby detach) a previously acquired `memory.buffer` view, so the page adapter MUST reacquire the buffer and re-validate the header/destination ranges **after** provider execution and immediately before each header write or payload copy — the discipline the landed conformance driver already applies.

### 8.4 Browser agent and re-entry

Proposed V1 instantiates the wasm engine and admitted provider registry on the same browser JS agent so a wasm import can call page JS synchronously without `Atomics.wait`, a cross-worker semaphore, or a second dispatch-loop interpreter. This is the wasm embedding's own topology choice, not a portable `PlanHostImportDescriptorV1` field and not a statement about native owner threads.

Provider-originated reactive changes enqueue a later page job/microtask which calls `exact_plan_publish_reactive_v1` after the import returns. A provider callback cannot synchronously call any `exact_plan_*` mutation export. A future worker topology must preserve the semantic call law without a blocking cross-agent bounce and therefore requires a separately reviewed revision.

### 8.5 Real-DOM and WebGPU boundary

The engine emits the existing versioned host-output family; the Contract web host applies it to real DOM. Host-import callbacks do not mutate DOM as a side effect of expression evaluation. Output application occurs only after the engine transition/commit boundary.

The embedding neither wraps nor replaces browser `navigator.gpu`, canvas acquisition, or WebGPU objects. LLP 0512's web path remains browser-native; the Exact DOM host services the canvas attachment/frame-clock boundary after applying ordinary engine output.

### 8.6 Symbol spellings (closes `OI-WASM-SYMBOL-SPELLINGS`)

Every product V1 **function** import/export name matches `exact_(plan|host)_[a-z0-9]+(_[a-z0-9]+)*_v1`; the linear-memory export named `memory` is the sole non-function export and the sole exception to the grammar. The accepted spellings are exactly the §8.2 export table and the §8.3 import table, whose functions all live under the single wasm import module **`"exact_host_v1"`**; a product V1 embedding module imports nothing under `"env"` or any other module name.

Signature widths are uniform: pointers, lengths, handles, refs, kinds, and tokens cross as `i32` whose bit pattern is **unsigned** (`u32`; JS normalizes with `>>> 0` — a high-bit pointer or handle is not negative); statuses and `lenOrStatus` returns are **signed** two's-complement `i32`. 64-bit quantities (call IDs, reactive versions, wake tokens) cross as unsigned `(lo, hi)` pairs of `i32` parameters, low word first — no `i64` appears in any product V1 signature, keeping the JS boundary BigInt-free; `f64` appears only for epoch-milliseconds.

Instantiation ordering is pinned: a product module MUST NOT use a wasm start function to invoke any import or perform product work — imports are invoked only during a product export call. `exact_plan_alloc_v1`/`exact_plan_dealloc_v1` are version-neutral memory primitives and MAY be called first (obtaining the `outPtr` the version probe needs); `exact_plan_embedding_version_v1` MUST then precede every other product export, and the embedder binds the exported `memory` and validates the tuple before any of them.

The Track R conformance ABI in this repository (`cn_alloc`, `cn_free`, `cn_run_conformance_fixture`, and the `cn_planrunner_*_conformance` family behind the `wasm-conformance-abi` cargo feature of `contract-native`) is a disjoint namespace: a product V1 embedding module MUST NOT export any `cn_*` symbol, and no `cn_*` surface ever counts as LLP 0517 conformance evidence. This section, together with the two tables it governs, is the atomic replacement the r2 open issue required.

### 8.7 Deterministic clock/timer carrier (closes `OI-WASM-CLOCK-TIMER-IMPORTS`)

The clock/timer service is the wasm carrier of the engine-owned, target-neutral scheduler law — the same law the native scheduler vtable carries: the engine (not the host) owns logical timers as a per-generation queue keyed `(dueEpochMs, registrationOrdinal)`, arms at most one physical wake at a time (arming replaces any previously armed token), applies the bounded missed-period catch-up rule, and cancels by owner scope and generation. This section pins only the wasm spelling of that carrier.

Imports (module `"exact_host_v1"`), sharing one status law that follows the **landed** shared scheduler rather than a receipt tier: `0` is the only success value, and **any nonzero return is a scheduler fault** — the engine records the matching `wasm-scheduler-*` diagnostic (`clock`/`arm`/`cancel`) and fences the generation, exactly as the landed Rust scheduler treats a nonzero native callback status:

- `exact_host_now_epoch_ms_v1(outPtr) -> status` — on status `0`, writes exactly one finite IEEE-754 binary64 at the engine-supplied 8-byte-aligned `outPtr`: **milliseconds since the Unix epoch**, the same epoch/unit the EENV validity interval (`validFromUtcMs`/`validUntilUtcMs`) uses. A non-finite or unwritten sample under status `0` is a fault. There is never a `now() = 0`, ambient-clock, or partial fallback.
- `exact_host_arm_wake_v1(wakeTokenLo,wakeTokenHi,dueEpochMs) -> status` — installs or replaces the embedding's single one-shot physical wake for the current generation at the given deadline. It MUST NOT call any `exact_plan_*` export inline; the page adapter arms a host timer and later enters `exact_plan_wake_scheduler_v1` as a **new page task**, preserving §8.4's re-entry rule.
- `exact_host_cancel_wake_v1(wakeTokenLo,wakeTokenHi) -> status` — cancels the pending physical wake for that token. The host treats cancellation as **idempotent success**: a token that already fired, was superseded, or is unknown returns `0` (the engine only cancels what it armed); a nonzero return means the host could not honor cancellation and is a fault that leaves the engine's armed token retryable, per the landed cancel law.

None of the three imports may synchronously invoke **any** `exact_plan_*` export — the §5.2/§8.4 no-inline-re-entry rule covers the whole trio, not just arming.

Ingress exports:

- `exact_plan_wake_scheduler_v1(handle,wakeTokenLo,wakeTokenHi) -> status` — validates handle and token: a token that is not the currently armed one (superseded, canceled, stale-generation, or unknown) returns `1` **without sampling the clock** — the landed mismatch path. Only the currently armed token samples the clock, exactly once, and runs every logical timer due at that sample as one scheduler batch under the shared law, returning `0`. An early wake for the armed token also returns `1`, and the engine has already cleared the fired arm and re-armed its earliest remaining deadline **before** returning that receipt (the landed behavior; a bare receipt that stranded the queue would be a defect). An invalid or retired handle, and any sampling/batch fault, returns `-1`.
- `exact_plan_notify_clock_lifecycle_v1(handle,lifecycleKind) -> status` — the closed kind table is `1` became-inactive, `2` resume, `3` wall-jump, `4` timezone-change (the same table the native lifecycle entry uses). Returns `0` applied, `1` coalesced/no-op, `2` relaunch-required (the timezone-replacement posture; the embedding boots a replacement generation with newly resolved EENV rather than executing on stale rules), `-1` a fault in the `wasm-scheduler-lifecycle` class — including a `resume` with no preceding inactive epoch, which the landed engine rejects. The page agent derives edges from Page Visibility/`pageshow`/timezone observation and coalesces pending notifications without erasing lifecycle edges, with priority timezone-change > wall-jump > resume.

Wake tokens are opaque, engine-minted, nonzero, monotonically allocated 64-bit values, generation-scoped and compared exactly; token exhaustion is a fault.

Relationship to the runtime-host authorities — two independent admissions govern liveness, as they do on the landed native path:

- The plan's `runtimeHostRequirements` table (the v1.5 plan-format enum: `clock=1`, `timer-queue=2`, `seeded-rng=3`) governs the clock/wake machinery: `clock` alone requires only the `now` import; `timer-queue` requires all three imports plus `exact_plan_wake_scheduler_v1`; `seeded-rng` requires no import at all — the seed is EENV-admitted. Wall-jump handling is live only when the plan requires `clock` (a wall-jump on a clockless plan is a receipt-only no-op, the landed rule).
- The **time-zone lifecycle is admitted independently**: the landed engine derives its time-zone requirement from EPPM's admitted `time-zone`/`dynamic-environment` static-environment row, not from `runtimeHostRequirements`. `exact_plan_notify_clock_lifecycle_v1` is therefore live whenever the plan admits `clock`, `timer-queue`, **or** the dynamic time-zone operand; a timezone-change on a plan without that operand is a receipt-only no-op.

Epoch arithmetic law: EENV encodes its validity endpoints as signed `i64`, and the landed native path casts them to `f64` without a range check. **This embedding's V1 admission** (not the EENV format, which is unchanged and stays with its owner) additionally refuses at boot any EENV endpoint whose magnitude exceeds 2^53, so every epoch value the scheduler compares — the endpoints and each `dueEpochMs`/clock sample — is an exactly representable binary64 and all comparisons are performed in `f64`; whether native admission adopts the same check is its owner's call. Because WebAssembly instantiation structurally requires every declared import, all three functions are always present in the import object; the **engine** may invoke one only under an admitted requirement, an unauthorized invocation is an engine integrity fault, and the adapter MUST NOT let ambient time leak into declared-pure app imports through any other door.

## 9. Versioning and refusals

The interface family, transfer profile, embedding ABI, provider artifact, and plan/wire formats are separate version dimensions and are compared separately. For the shared interface and embedding tuples defined here, a compatible minor may add only optional independently framed material that a V1 reader can skip; any new required material or change to an existing name/integer mapping, discriminant, framing, or meaning requires a new major and matching `minReader`. A different or insufficient major refuses. An owning external format may have a more granular reader-floor law, but it cannot silently relax these two tuples.

Minimum refusal classes are:

- `plan-host-interface-version` / `plan-host-interface-digest`;
- `wasm-embedding-version` / `wasm-embedding-feature`;
- `provider-manifest-binding` / `provider-artifact-digest`;
- `wasm-boot-config` (§8.2.1) and `wasm-scheduler-clock` / `wasm-scheduler-arm` / `wasm-scheduler-cancel` / `wasm-scheduler-wake` / `wasm-scheduler-lifecycle` (§8.7);
- `host-result-token` / `host-result-release` / `host-memory-range`;
- the shared call/value failures in §5;
- the owning plan, EXWF, capability, and environment refusal families where those boundaries reject.

No mismatch may fall back to the demoted web engine, an ambient JS function, JSON, or a best-effort DOM update.

## 10. `PlanHostBenchmarkResultV1`

### 10.1 Workloads

Every adapter uses the same four workload IDs:

1. `cold-registry-first-call` — artifact load/compile/instantiate, registry admission, and first successful call reported separately;
2. `warm-scalar` — `formatCountdownMinutes` plus a two-number-to-number control, at least 1,000 calls;
3. `warm-closed-aggregate` — `searchStations`, `getDirectionBoard`, and one bounded large-payload arm at pinned Caltrain dataset sizes;
4. `caltrain-tick-batch` — one-second recompute/render batch including both boards and visible-row formatting/styles, repeated until at least 1,000 actual seam calls while retaining per-tick samples.

The V1 comparator set is exactly `direct-provider` (direct admitted JS invocation without the engine seam) and `runner-no-seam` (equivalent engine batch with host results preinstalled). Adding another comparator requires a versioned schema revision; an adapter may neither add nor omit a row in V1.

### 10.2 Result schema

One canonical JSON document contains exactly these required top-level fields:

```text
schemaVersion: 1
interfaceVersion: { major: 1, minor: 0, minReader: 1 }
interfaceFamily: "PlanHostInterfaceV1"
adapter: "native-hermes" | "wasm-page-js"
gitCommit, planDigest, seamInterfaceDigest, providerArtifactDigest: string
capturedAt, boundDeclaredAt: RFC3339 string
environment: {
  machineId, cpu, memoryBytes, os, browser?, browserVersion?,
  hermesIbexRevision?, wasmEngine?, optimization, datasetDigest,
  warmupPolicy, timerSource
}
bounds: BenchmarkBoundV1[]
workloads: WorkloadResultV1[]
comparators: ComparatorResultV1[]
verdict: "pass" | "fail"
```

`PercentilesV1` is exactly `{p50,p95,p99,max}`, all nonnegative integer nanoseconds. `BenchmarkSamplesV1` is exactly:

```text
callLatencyNs: integer[]
batchLatencyNs: integer[] | null
batchSeamLatencyNs: integer[] | null
coldArtifactLoadCompileInstantiateNs: integer[] | null
coldRegistryAdmissionNs: integer[] | null
```

These are retained post-warmup raw samples, not a count or a histogram. `callLatencyNs` contains one duration for every actual seam call. For `caltrain-tick-batch`, both batch arrays contain one element per retained tick; `batchSeamLatencyNs[i]` is the sum of non-overlapping seam-call intervals wholly inside `batchLatencyNs[i]`. For `cold-registry-first-call`, the two cold arrays and `callLatencyNs` have equal length—one complete fresh-realm trial each—and `callLatencyNs` is the separately reported first successful call. The cold arrays are null for every other workload. The batch arrays are non-null only for `caltrain-tick-batch`; otherwise both are null.

Each `WorkloadResultV1` requires:

```text
id, iterations, actualSeamCalls
samples: BenchmarkSamplesV1
latencyNs: { p50, p95, p99, max }
batchLatencyNs: { p50, p95, p99, max } | null
coldPhaseLatencyNs: {
  artifactLoadCompileInstantiate: { p50, p95, p99, max },
  registryAdmission: { p50, p95, p99, max }
} | null
argumentBytesCopied, resultBytesCopied, adapterAllocations
resultBytes
seamShareOfBatch: number | null
```

`iterations` is the number of complete retained workload trials after warmup. `actualSeamCalls` equals `samples.callLatencyNs.length`; the batch sample count equals `iterations` for the tick workload, and all three cold sample counts equal `iterations` for the cold workload. `latencyNs`, `batchLatencyNs`, and `coldPhaseLatencyNs` are derived only from their corresponding raw arrays. `argumentBytesCopied`, `resultBytesCopied`, `adapterAllocations`, and `resultBytes` are totals over those retained trials; `resultBytes` is logical encoded outcome size before adapter copies. `seamShareOfBatch` is null outside the tick workload and otherwise equals `sum(batchSeamLatencyNs) / sum(batchLatencyNs)` in `[0,1]`.

`workloads` contains exactly one `WorkloadResultV1` for each §10.1 workload ID and no other row.

`ComparatorResultV1` is fully closed and requires:

```text
comparatorId: "direct-provider" | "runner-no-seam"
workloadId: one of the four workload IDs
iterations, actualCalls
samples: BenchmarkSamplesV1
latencyNs: { p50, p95, p99, max }
batchLatencyNs: { p50, p95, p99, max } | null
coldPhaseLatencyNs: {
  artifactLoadCompileInstantiate: { p50, p95, p99, max },
  registryAdmission: { p50, p95, p99, max }
} | null
argumentBytesCopied, resultBytesCopied, adapterAllocations, resultBytes
```

The comparator sample/null/summary rules are identical except `batchSeamLatencyNs` is always null. `actualCalls` equals `samples.callLatencyNs.length`: direct admitted JS invocations for `direct-provider`, and equivalent preinstalled-result consumptions for `runner-no-seam`. The document contains exactly one row for each of the two comparator IDs × four workload IDs, under the same inputs/dataset/result assertions as the adapter workload.

Each `BenchmarkBoundV1` is exactly:

```text
id, authority: string
workloadId: one of the four workload IDs
metric: "call-latency-ns" | "batch-latency-ns" |
        "cold-artifact-load-compile-instantiate-ns" |
        "cold-registry-admission-ns"
percentile: "p50" | "p95" | "p99" | "max"
operator: "<="
value: nonnegative integer
unit: "ns"
```

`bounds` is a nonempty array with unique IDs, so independent scalar, aggregate, tick, cold-start, and first-call gates can be expressed simultaneously as distinct workload × metric × percentile rows. A bound may target only a metric present for its workload; `call-latency-ns` on the cold workload is the first-successful-call gate. Every bound shares the top-level `boundDeclaredAt`, which must precede `capturedAt`, and the receipt's `verdict` is `pass` iff every bound evaluates true.

All counts, bytes, and nanoseconds are nonnegative JSON safe integers. Percentiles use nearest rank: sort ascending and select one-indexed rank `ceil(p*N)` for p50/p95/p99; `max` selects rank `N`. Empty required series refuse rather than inventing zero. Warmup measurements never enter raw samples or counters. The cold workload may use fewer trials but states the exact count; warm workloads require at least 1,000 actual seam calls.

The complete bound array is selected and committed before measurement. A receipt validator rejects a missing/stale/wrong-commit result, changed hardware/browser/engine/provider/interface/workload/dataset/comparator, post-hoc bound, insufficient calls, inconsistent raw-sample/summary/counter relationships, or any failed bound. Native and wasm results are separate adapter documents under this one schema.

## 11. Conformance and registered gates

The landing implementation must register executable checks in `exact-verify.json`. The suite includes:

- byte-identical TS/Rust value/outcome vectors, including UTF-16 isolated surrogates, non-finite numbers, negative zero, closed objects, Option, every outcome discriminant, diagnostic-code/effect-disposition mapping, capability framing, malformed/trailing bytes, cycles, bounds, and repeated aliases;
- descriptor/digest/order/arity/type/environment fixtures;
- call success/refusal/throw/provider-fault/transport-fault and effect-disposition fixtures;
- a sabotage test proving nested mutation refuses and invalidation publishes only in a later top-level job;
- memory-growth/view-detachment, bad pointer/range, stale token, double release, release-on-validation-failure, and generation-fence fixtures;
- wasm export/import/version/provider-artifact admission fixtures;
- `ExactWasmPlanBootConfigV1` structural/version/bounds/overlap/digest-mismatch refusal fixtures and the boot-diagnostic read path (§8.2.1);
- `WasmPlanResultHeaderV1` consistency-matrix fixtures across both directions, including every malformed status/kind/flags/token/version combination (§8.3.1);
- scripted-clock scheduler fixtures — equal-deadline order, re-arm/replace, early/stale/superseded wakes, cancel receipts, lifecycle coalescing, and never-`now()=0` (§8.7);
- same-source Rust-wasm/page-JS integration against the LLP 0508 conformance corpus;
- `PlanHostBenchmarkResultV1` raw-sample/summary consistency, complete comparator matrix, per-workload × per-percentile bound-array/freshness validation, and a captured wasm result before M2 cutover.

The native M1 design may register native codec/seam checks using the same vocabulary. Passing them does not substitute for the wasm checks or accept this Draft.

## 12. Security and privacy

Provider selectors resolve only within the digest-admitted registry. Diagnostics are bounded and redacted. Raw thrown objects, proxies, getters, capabilities, native modules, filesystem/network ambient authority, DOM objects, and browser globals do not cross as values.

Capabilities remain separately granted/routed. A declared pure import is not a capability escape hatch, and a capability is not relabeled pure to make it callable from a derive. Provider artifact admission binds exact bytes and compiler/build identity as required by the embedding; a development origin is development trust, not release evidence.

## 13. Open issues before acceptance

The four M2-registration blockers are **closed at r3 with normative resolutions**, each grounded in the landed M1 native seam and Track R code rather than invented fresh; Charlie Cheever's ratification of r3 (2026-08-23) ratified the four closures:

1. **`OI-WASM-BOOT-CONFIG` — CLOSED (r3, proposed) by §8.2.1.** `ExactWasmPlanBootConfigV1` (`EWBC`, tuple `1.0/minReader 1`): a canonically packed sectioned little-endian document carrying plan bytes, always-nonempty EPPM and EENV (the landed admission rules), `ContractHostValueV1` initial props, the landed root-identifier alphabet, a nonempty canonical state-bridge hydration document, a strictly positive EENV-agreeing viewport, four raw `digest32` admission expectations, the serialized closed host-feature bitset, and the single `automatic-v1` generation mode — with pinned bounds, borrow-only ownership, the one-live-generation rule, handle-`0` refusal, a JSON boot diagnostic in the `wasm-boot-config` code space (the native `diagnostic_json_out` posture, never `PlanHostDiagnosticV1`), and pinned `exact_plan_boot_diagnostic_v1` semantics. It deliberately does not inherit the native `CnHostPlanBootConfigurationV1` layout.
2. **`OI-WASM-RESULT-HEADER` — CLOSED (r3, proposed) by §8.3.1.** `WasmPlanResultHeaderV1`: one 32-byte, 8-aligned, little-endian record with every offset/width/value pinned, bare-body payloads discriminated by the header, the native binding-kind allocation, a complete closed status/kind/payload/version matrix (no open "etc." rows), split diagnostic ownership (seam `PlanHostDiagnosticV1` with its exact 131,084-byte bound vs. the plan-runtime JSON diagnostic on the output path), one-shot exact-size copy with pinned copy/release status tables, never-reused generation-scoped tokens, an 8-aligned `exact_plan_alloc_v1` guarantee, and the transport-status rule scoped to exactly three entries. It projects the shared outcomes but is not `PlanHostCallOutcomeV1`.
3. **`OI-WASM-SYMBOL-SPELLINGS` — CLOSED (r3, proposed) by §8.6 and the §8.2/§8.3 tables.** The r2 provisional spellings are accepted verbatim, three exports are added (`exact_plan_boot_diagnostic_v1`, `exact_plan_wake_scheduler_v1`, `exact_plan_notify_clock_lifecycle_v1`), imports live under module `"exact_host_v1"`, the function-name grammar has `memory` as its sole exception, i32 signedness and `(lo, hi)` pair order are pinned, a start function may do no product work, and the `cn_*` conformance namespace is excluded from product embeddings.
4. **`OI-WASM-CLOCK-TIMER-IMPORTS` — CLOSED (r3, proposed) by §8.7.** Three imports (`now`/`arm`/`cancel`) and two ingress exports with pinned signatures and pinned numeric statuses matching the **landed** scheduler (0-or-fault, idempotent-success cancel, early-wake re-arm before receipt, inadmissible-resume fault), Unix-epoch-millisecond f64 clock matching the EENV interval unit with the 2^53 representability law, engine-minted monotonic 64-bit wake tokens, replace-single-wake and whole-trio no-inline-re-entry rules, the closed lifecycle-kind table, and the two-authority liveness matrix (`runtimeHostRequirements` for clock/wake; EPPM's dynamic time-zone operand for timezone lifecycle). Logical-timer semantics remain the engine-owned target-neutral scheduler law; §8.7 pins only the wasm carrier.
5. Measure result-token copy overhead against a guest-provided-buffer alternative; retain exactly-once provider invocation either way.
6. Decide whether wasm memory64 or shared memory enters a later profile; V1 proposed here is wasm32, unshared.
7. Confirm same-page-agent V1 against CSP, Safari/Chrome, long-task, and browser lifecycle evidence; a worker design is not silently substituted.
8. Pin the complete M2 `BenchmarkBoundV1[]` for this spec's compile/instantiate and scalar/aggregate/tick seam-cost rows. Compressed bytes and first successful interaction remain separately bounded by their owning M2/web-performance receipt; they are not smuggled into a latency-only 0517 bound metric.
9. Pin output polling/backpressure details against the final EXWF web-host delivery API without duplicating EXWF semantics here.
10. ~~Resolve any 0508 amendment needed to attach its existing nullable boundary marker to generated optional fields; do not create a second marker in this spec.~~ **CLOSED 2026-08-23 (verified; 0504 §3 row 84 / LLP 0551 finding 27):** no 0508 amendment is needed and no second marker exists. Landed format-schema v1.5 (`packages/exact-contract/plan/format-schema.json`, `formatVersion` 1.5) pins the existing flag: its `v15NullableRule` reads "For formatMinor>=5, nullable is boundary-decoder metadata only and is forbidden on authored/stored program types; option rows themselves must have flags=0", and the flag's set-meaning covers "boundary null or representable generated-field absence normalizes immediately to Option.none". 0508 §9's totality text already sanctions exactly this attachment ("the seam annotation is exactly the IR's `nullable` marker … today reachable through generated/IR-produced shapes, not authored text"). This spec's §5 single-marker normalization rule stands unchanged.

Closure rule: **0517 must close `OI-WASM-BOOT-CONFIG`, `OI-WASM-RESULT-HEADER`, `OI-WASM-SYMBOL-SPELLINGS`, and `OI-WASM-CLOCK-TIMER-IMPORTS` before M2's gates can be registered.** A spike may inform those decisions, but a command registered against provisional names, layouts, or signatures is not an M2 gate. **Ratified: Charlie Cheever ratified r3 on 2026-08-23 (decision relayed via orchestration session exact-9e); the four closures (items 1–4 above) are normative.** M2 gate registration may now proceed against the pinned names, layouts, and signatures (LLP 0504 §3 row 28 / §4 M2-ABI clock); items 5–10 were acceptance-shaping and did not block gate registration.

## 14. Acceptance and milestone relationship

Acceptance requires the four named M2-registration blockers and the other acceptance-shaping open ABI/layout choices above to be resolved, the proposed imports/exports implemented, registered conformance green, and the wasm benchmark captured inside every predeclared bound. LLP 0514 M2 cannot register its gates while any named blocker remains open and cannot be offered before all required evidence is green. Two author decisions are therefore distinct: **ratifying the r3 blocker closures** (which closes the four `OI-WASM-*` items and lets M2 register gates against pinned names) and **accepting the document** (which additionally awaits the implemented exports, green conformance, and the captured in-bound benchmark). r3 queues the first; it does not claim the second.

LLP 0514 M1 is different: its native C/Swift/Hermes/HBC implementation can proceed while this document remains Draft because the native design owns those mechanics. M1 cites this document only for shared family names and target-neutral semantics; it does not claim that doing so accepts the wasm embedding on Charlie's behalf.


## Ledger standing for the late-boot buffer (recorded 2026-08-26)

LLP 0504 §3 row 53b carries the late-boot segment buffer — the host
buffering unapplied settlement segments that arrive before the wasm
engine boots (RFC 0537 §2.1).

LLP 0504 §3 row 53b is window-reserved, conditional on this spec's
owner ruling whether the buffer is M2-ABI-freeze-relevant.

The reason is that the classification *is* the open question: the row's
own text routes it here precisely so this spec's owner can rule
whether the buffer is **M2-ABI-freeze-relevant**. A ledger-side verdict
would pre-empt that ruling.

Not to be mistaken for a ruling on it: RFC 0537 §5 says no D1 conjunct
names the **payload envelope** as freeze surface. That statement is
about the envelope, not about this buffer, and the two are different
surfaces.
