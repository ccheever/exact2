RFC 0491 W0-A draft artifact — hand-verified inventory/draft, provenance commands inline; generated authorities under tests/protocol/exwf/ own their boundaries and this pack never restates them. Registered in exact-verify.json as `kernel-refresh-w0a-pack` (RFC 0491 W0-B).

# SemanticFrame schema skeleton (DRAFT)

This skeleton is not the registered W0-A schema. It fixes the semantic surface that the later owned schema must encode without activating a recorder or changing runtime behavior.

## Artifact and version

- Proposed registered artifact: `tests/protocol/semanticframe/v1/schema.json`.
- Root record: `{ format: "SemanticFrame", version: { major: 1, minor: 0 }, ... }`.
- Major changes may reinterpret or remove a typed field. Minor changes may add fields only when the v1 reader's declared unknown-field disposition preserves semantic equality; otherwise the change is major.
- A tape is an ordered sequence of SemanticFrames plus one tape header carrying schema identity, producer/capability context, retention, privacy class, and truncation state.

The version numbers above are proposals, not current-tree facts. RFC authority and the required field list can be re-read with:

```sh
sed -n '1074,1132p' llp/current/0491-kernel-refresh-program.rfc.md
```

## Equality

Semantic equality is equality of the normalized typed operation stream and its explicitly captured ingress, identity, recovery, and terminal effects—not EXWF byte identity, JSON spelling, object-member order, allocation addresses, map iteration order, diagnostic prose, or timestamps that are not declared semantic ingress.

Normalization must:

1. Decode every operation into its closed typed variant.
2. Canonicalize unordered sets/maps by their schema-owned key, while preserving order for children, operations, effects, and receipts where order is semantic.
3. Compare floats by the owning field's canonical rules, including signed-zero/NaN rejection or normalization specified per field; never by JSON text.
4. Compare nondeterministic ingress by the captured value/effect record actually supplied to execution.
5. Compare recovery by checkpoints, transitions, watermarks, staged deltas, and terminal disposition. A later tree snapshot cannot substitute for any of these.

## Captured nondeterministic ingress

Every record has `ingressId`, `kind`, causal operation/action identity, privacy class, typed request, typed result/effect, and an outcome (`returned | threw | refused | cancelled`). The closed initial kinds are:

- `time`: Contract's determinism host enters at `packages/exact-contract/src/runtime/instance.ts:714`; the Rust Native `clock.now` host capability samples at `contract-native/src/ffi.rs:2265`. Kernel list staging also consumes monotonic time at `kernel/src/native_list.rs:422` and `:441`; if expiry affects a replay result, those samples are ingress.
- `measure-uniform-v1`, `measure-uniform-v2`, `measure-runs-v1`, `measure-runs-v2`, `measure-runs-v3`, and `native-control-measure`: callback results enter layout through `kernel/src/ffi.rs:1942-2205` and native-control measure registration at `kernel/src/ffi.rs:1716-1756`.
- `measure-text-op`: the legacy wire request is decoded at `kernel/src/protocol/dispatch.rs:541` and its result is produced at `kernel/src/protocol/dispatch.rs:809-812`. Current batch validation rejects this request path because there is no response channel; the schema still needs to represent old oracle tapes honestly.
- `module-action-effect`: request/admission and the native effect enter at `kernel/src/ffi.rs:4591-4633` through `GLOBAL_ACTION_CALLBACK`.
- `module-sync-effect`: request/admission and returned bytes enter at `kernel/src/ffi.rs:4644-4728` through `GLOBAL_SYNC_CALLBACK`/its one-shot writer.
- `module-wire-skipped`: module opcodes are recognized but skipped by view dispatch at `kernel/src/protocol/dispatch.rs:480-490`; a recorder must not fabricate effects for them. If another layer routes them, that layer records the actual callback ingress above.

Provenance:

```sh
rg -n 'measure_text_for_taffy_with_callback|exact_set_text_measure|exact_set_native_control_measure|exact_module_dispatch_action|exact_module_call_sync' kernel/src/ffi.rs
sed -n '480,545p;799,835p' kernel/src/protocol/dispatch.rs
```

## Identity and recovery records

The schema must carry, without inference:

- `identityCheckpoint`: the six-field address context `{ producerId, executionGeneration, rootId, rootIncarnation, viewId, allocationGeneration }`, complete per-root incarnation/allocation table digest plus its typed entries where replay needs them, and the active `ApplyMode`.
- `identityTransition`: an `eventId` that must resolve in the generated `tests/protocol/exwf/identity-events.json` authority, before/after values, cause, owner, and `ApplyMode`. The initial mode alignment is `Normal` for `ProducerRestart`, `CoherentReload`, `HotRevision`, `RootReset`, `KernelReset`, `AllocateAfterDestroy`, `LiveNoop`, and `RootMigration`; `RestoreReconnect` for the same-named event; and `RecoveryAdopt` for the same-named event. The schema must reject any event ID absent from the generated authority and must not copy that authority's per-dimension effects.
- `rootWatermarks[]`: per `(ProducerId, ExecutionGeneration, rootId, rootIncarnation)` values for `observedRejected`, `lastCommitted`, `replacement`, and `nextExpected`, each with presence/unknown distinct from zero.
- `recovery`: token, rejected producer generation and batch, recovery generation, complete root-set manifest, per-root replacement status, and token freshness/duplicate outcome.
- `stagedAllocationGenerationDeltas[]`: address key, before/after generation, allocation class, and commit state; deltas remain staged until atomic adoption.
- `terminalDisposition`: `committed | refused | rolledBack | retainedLastGood | dropped | recoveryAdopted | recoveryRefused`, affected root set, sidecar family dispositions, and `presentationAck` (`notRequired | pending | presented | refused | timedOut`) with the presentation receipt identity when present.

These fields trace RFC 0491 WS-D/WS-H. Reproduce the governing paragraph with:

```sh
sed -n '795,825p;1080,1091p' llp/current/0491-kernel-refresh-program.rfc.md
jq -r '.events[].id' tests/protocol/exwf/identity-events.json
```

## Proposed bounds and truncation

These are W0-A proposals to be owner-reviewed before registration, not measurements of current behavior:

- `maxSemanticFrameBytes`: 32 MiB.
- `maxTapeBytes`: 256 MiB.
- `maxOperationsPerFrame`: 65,535.
- `maxStructuredValueDepth`: 64.
- `maxSingleIngressValueBytes`: 1 MiB before privacy projection.

The machine schema must encode these values in its tape header so a reader never relies on ambient defaults. On a cap:

- Never serialize a partial typed operation, partial UTF-8 scalar, partial recovery record, or partial identity transition.
- Finish the last complete record, append a terminal `truncated` record containing the cap name, bytes/records retained, bytes/records omitted when known, and the last complete causal cursor, then close the tape.
- Reserve space for the truncation terminal when the tape opens. If even that terminal cannot be emitted, refuse the recording and do not label it replayable.
- A truncated tape may support diagnostics but is never a migration oracle or semantic-equality witness.

Retention is explicit in every tape header: `{ class: "ephemeral-dev" | "test-fixture" | "migration-oracle", deleteAfterMs: u64 | null, owner, reason }`. `ephemeral-dev` must have a finite deadline; fixture/oracle permanence requires repository review and content classification. The specific byte/depth values and retention classes above must be accepted in the registered schema; this draft alone does not activate them.

## Privacy and production projection

- Raw tapes are dev/test/migration-oracle only. Production does not persist them and does not gain a second flight recorder.
- Production performs literal closed-schema selection into `observability-crash-capsule-v1`, then privacy transformation. Fields absent from that allowlist are dropped, not tunneled through an open metadata map.
- Values use `public | digest | vault` classification. Non-public values may expose only keyed commitments in persisted production artifacts—never raw values and never unkeyed content digests, including an unkeyed SHA-256 confirmation oracle.
- Vault material and keys are never embedded in SemanticFrame. A dev/test raw tape may carry an opaque vault reference only under its explicit retention/access policy.

## Registration gaps

The follow-up schema still needs an owned JSON Schema/codec, canonical normalization algorithm, error taxonomy, cap owner acceptance, fixture corpus, registry entry, and exact compatibility policy. This document intentionally supplies none of those runtime/build changes.

**Owned successor (2026-08-23, RFC 0491 Phase-1 prerequisite):** the registered schema authority landed at `tests/protocol/semanticframe/v1/schema.json`, with generated Rust/TypeScript codecs, the schema-digest fixture, the golden corpus, and registry rows (`semanticframe-codec-parity`, `semanticframe-kernel-codec`, `semanticframe-ts-reader`; boundary `semanticframe-ir` in `exact-contracts.json`). This draft remains the W0-A census record; the owned authority is the design's current word.
