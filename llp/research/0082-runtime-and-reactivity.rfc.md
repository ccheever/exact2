# RFC 0082: Contract Runtime and Reactivity

**Type:** RFC
**Status:** Draft
**Systems:** Contract, Tooling, Runtime
**Author:** Charlie Cheever / GPT-5 Codex
**Date:** 2026-03-27
**Revised:** 2026-03-27; 2026-08-13 (async-cell alignment: `refreshing` is a public resource status; last-good `value` is retained on same-identity refresh; thenable/`Promise` `derive` results are a language error on every tier. Snapshot shape is LLP 0259 `AsyncCellSnapshot`. Identity-change vs same-identity refresh follows LLP 0263 §4.3. The TS interpreter still publishes only `pending`/`ready`/`error` — that is an implementation gap against this section, not a second semantics.); 2026-08-23 (RFC 0498 AQ-A close: the Resources transition table is now the **data axis of the one product transition table** — RFC 0498 §4.2 names this document's machine the data-axis winner, LLP 0284 §2.2's connection axis is the orthogonal second axis, LLP 0509 specifies identity/readiness, and the registered corpus under the `cell-semantics` authority row pins the cases. The reconciliation follow-up sentence is deleted (LLP 0194 §4.1/§4.6 and LLP 0259 now point here); the shared-cache/optimistic-mutation pointer retargets RFC 0093 → RFC 0498 (Aquifer); the TS-interpreter gap sentence updates to the decided known-red posture, LLP 0505 r3 row 2(e).)
**Related:** RFC 0081, RFC 0083, RFC 0086, RFC 0089, RFC 0091, RFC 0498 (Aquifer — the data tier; §4.2's product table adopts this document's data axis), LLP 0509 (cell identity and readiness algorithms), LLP 0194 (async-state view grammar; `refreshing` / `stale` boundary), LLP 0259 (`AsyncCellSnapshot`, `stale="keep"`; Superseded by 0498/0509), LLP 0263 (query identity vs `refreshing`), LLP 0284 (the connection axis), LLP 0328 Q8 (native cell is this snapshot, not a thenable), LLP 0330 A10, RFC 0093 (historical shared-data-plane pointer; data went to RFC 0498)
**Track:** Experimental
**Parent:** RFC 0080

## Summary

Define the Contract runtime model for local state, derivation, resources, actions, tasks, scheduling, and batching. The runtime should be fine-grained, deterministic, and explicit about cleanup and stale async work.

## Goals

- Local state updates should not require virtual DOM diffing.
- Derivations should be pure, memoized, and dependency-tracked automatically.
- Async work should have explicit lifecycle semantics.
- Side effects should be scheduled and disposable.
- The runtime model must leave room for future state-preserving refresh.
- The scheduling model must not preclude host-driven animation and transition systems.
- Errors must have a precise propagation path to boundaries and diagnostics.

## Reactive Primitives

### State slots

A `state` declaration lowers to a named writable signal scoped to one component instance.

Rules:

- writes are allowed only from `action`, `task`, or resource settlement
- writes batch within a synchronous segment
- slot identity is derived from the state name, not declaration order

### Derives

A `derive` lowers to a memoized pure computation.

Rules:

- dependencies are tracked by reads
- derives recompute lazily or eagerly depending on runtime strategy, but semantics must be equivalent
- derives may depend on props, state, other derives, and settled resource values
- derive results are settled values in the Contract value model (LLP 0330
  A10). After evaluating a derive formula and **before** publishing the
  memo, every engine rejects a **thenable**: a value whose `then` property
  is a function (a native `Promise`, or a structural thenable). The error
  is reported at the authored derive locus. Compile-time rejection is
  allowed only where thenability is statically proven; it is not a D10
  construct gate and must not be emitted as bundle feature metadata.
  Diagnostic / conformance code: `thenable-derive`. Async work is a
  `resource` (or a `query` / `mutation` / deferred data-plane cell), never
  a thenable memo. This is a language rule on every tier, not a
  native-only gate (contrast `derive-memo`, LLP 0328 D10 / LLP 0330 §2.1).

### Resources

A `resource` lowers to an async state machine with generations. Values are
never thenables: a resource publishes settled data plus a status; it does
not publish a Promise.

Two surfaces, not one (LLP 0259 §5.1.1): the **resource handle** is the
authored accessor (`status`, `pending`, `ready`, `error`, `value`,
`refresh()`). The **normalized snapshot** is LLP 0259's
`AsyncCellSnapshot`, projected for boundaries, the Agent API, and
conformance. During `pending` / `error`, a retained last-good settlement
is `previous` on the snapshot; the handle may still expose it as
`.value` until the new generation publishes. Do not treat the two
shapes as the same object.

Per resource:

- `idle` is optional implementation detail
- public statuses are `pending`, `ready`, `error`, and `refreshing`
- each run has a generation number
- only the latest non-cancelled generation may publish
- **Resource identity** is the declaration instance plus the current
  dependency tuple (the reactive inputs the initializer read). A
  dependency change is an identity change. `.refresh()` with unchanged
  dependencies is same-identity.
- **`refreshing`** is a same-identity re-run that still holds a last-good
  value (LLP 0263 §4.3 applied to resources). During `refreshing`, handle
  `.value` is the last good settlement, not `undefined`.
- `pending` may carry snapshot `previous` when the runtime can prove a
  retained value after an identity change. A source that cannot prove
  retention must not claim `refreshing` (LLP 0259 §5.1.1).
- View slot dispatch: `pending` → `loading`; `refreshing` stays on the
  ready / empty / connection-`stale` slots (the AOT viewer already
  branches this way). Coordinated boundaries may override with
  `stale="keep"` / `stale="fallback"` (LLP 0194 §4.6, LLP 0259). Boundary
  `stale` is a *presentation* term ("showing retained prior content while
  work is in flight") and includes `pending`+`previous`, not only
  `refreshing` — derived vocabulary over cell state, never a second
  state store. LLP 0194 §4.1/§4.6 and LLP 0259 are amended to this
  reading (RFC 0498 §4.2's named winners); there is no second resource
  semantics to reconcile.

Transition table (resource):

| Event | Published status | Retained data |
| --- | --- | --- |
| New instance, first run | `pending` | none |
| `.refresh()` after success, same deps | `refreshing` | last-good as handle `.value` |
| Dependency / identity change, last-good proven | `pending` | snapshot `previous`; handle `.value` still last-good |
| Dependency / identity change, no last-good | `pending` | none |
| Generation settles ok | `ready` | new value |
| Generation settles error | `error` | snapshot `previous` if a last-good exists |
| Superseded generation settles | ignored | unchanged |

This table is the **data axis of the one product cell transition table**
(RFC 0498 §4.2 — this document's machine is the named data-axis winner;
LLP 0284 §2.2's connection axis `connecting | live | polling | stale |
disconnected` is the orthogonal second axis). Identity, the reuse gate,
rotation fencing, and readiness are specified by LLP 0509; the executable
cases live in the registered conformance corpus
(`packages/exact-contract/src/conformance/aquifer-cell-machine-v1.json`,
the `cell-semantics` row in `exact-contracts.json`). LLP 0194, LLP 0259,
and RFC 0263's cell-state sentences point at that table rather than
carrying their own.

`resource` is the component-local async primitive. Shared cache identity, invalidation, optimistic mutation, and cross-screen server-state coordination belong to **Aquifer, the Exact-owned data tier (RFC 0498)** — this pointer previously named RFC 0093's shared-data plane; the 0093 split sends data to 0498 (Exact Native: LLP 0329 E5 overlays and LLP 0331 INV-9, not a component-local optimistic store).

When dependencies change:

1. a new generation starts
2. prior generation is cancelled if supported
3. otherwise the prior generation is marked stale
4. stale settlement is ignored
5. if the new run is same-identity and a last-good value is retained, the
   published status is `refreshing` until that generation settles

The TS interpreter still publishes only `pending` / `ready` / `error` and
maps every re-run — including `.refresh()` — to `pending` (query handles
likewise collapse provider `refreshing` to `ready`; provider-backed query
cells do carry LLP 0284's shipped connection axis). That gap is
now **decided known-red on the dying engine** (LLP 0505 r3 row 2(e),
recorded in RFC 0498 AQ-A): the TS cell path is not upgraded;
`refreshing` is published by the `exact-aquifer` engine (AQ-B, whose
product state also carries the connection axis), and the legacy TS cell
path's terminal disposition is deletion at the AQ-B cutover. It remains
one semantics — this table — with a recorded red row, never a second
semantics.

### Actions

An `action` lowers to an event-callable function with batching semantics.

Rules:

- synchronous writes batch together
- each `await` boundary closes the current batch
- actions may call actions
- actions may call imported capabilities

### Tasks

A `task` lowers to a scheduled side-effect function with explicit trigger semantics.

Triggers:

- `mount`
- `manual`
- `when [a, b, ...]`

Tasks may register cleanup. Cleanup runs when:

- the task is invalidated and rerun
- the component unmounts
- a reset/remount refresh occurs

## Scheduling Model

The runtime should use microtask-based flushing for v0:

1. state writes mark bindings dirty
2. dirty bindings are coalesced
3. a single flush updates derived/view subscribers
4. host updates are emitted
5. one commit closes the batch

The observable rule is simple: multiple synchronous writes cause one render/commit wave.

## Binding Model

Each dynamic binding subscribes directly to the symbols it reads:

- text bindings subscribe to text dependencies
- prop bindings subscribe to prop dependencies
- `when` regions subscribe to their predicate
- `each` regions subscribe to list identity and keyed children

No whole-component rerender is required as the primary mechanism.

## Animation-Friendly Scheduling

The runtime does not need a full animation language in v0, but it must not make motion impossible later.

Required properties:

- keyed nodes keep stable identity across ordinary reactive updates
- `when` and collection region updates emit enough structure for enter/exit/reorder transitions
- host commits can carry transition metadata without changing reactive semantics
- animation execution should live on the host/UI-thread side when possible, not depend on JS running every frame

RFC 0089 defines the motion model that builds on these constraints.

## Imported Stores, Capabilities, and Shared Data

The runtime distinguishes:

- **Contract-aware stores**
- **Contract-aware capabilities**
- **Shared query/mutation descriptors**
- **Opaque imports**

Contract-aware imports may participate in dependency tracking and effect analysis. Opaque imports do not; they are explicit analysis boundaries.

This distinction matters for both correctness and tooling honesty.

## Cleanup Discipline

Cleanup is not optional if Contract wants a viable long-term refresh story.

Required guarantees:

- stale resource settlements do not publish
- event subscriptions created by tasks are removable
- timers created by tasks are cancellable
- component unmount tears down task cleanup
- root reset tears down all task cleanup for that root

## Error Semantics

Unhandled failures should be attributed precisely:

- parse/compile failures: compiler diagnostics
- derive purity violations: compile-time error where possible
- resource rejection: resource `error` state
- action/task exception: surfaced to dev diagnostics and optionally boundary/error UI

The runtime should not silently swallow exceptions in a way that hides them from agents or humans.

Escalation rules:

- resource failures become `resource.error` unless explicitly elevated
- render/derive failures bubble to the nearest Contract boundary
- action/task failures surface to diagnostics immediately and may also trip a boundary if the caller opts into boundary-mediated handling

RFC 0091 defines the boundary model in more detail.

## Relationship to Refresh/HMR

This runtime is designed so that future refresh can:

- preserve named state slots
- recompute derives
- optionally retain or revalidate resources
- teardown and rerun tasks safely

That design is specified more fully in RFC 0086.

## Hard Parts

- Async cancellation that is precise enough for correctness but simple enough for v0.
- Cleanup APIs that are ergonomic without letting side effects escape the runtime's control.
- Shared-state participation without making imported modules invisible ambient dependencies.

## Open Questions

- Should tasks register cleanup by `return` value, helper API, or both?
- How much scheduling behavior should be user-configurable versus fixed by the runtime?

Closed 2026-08-13: resources **do** retain last-good values. Same-identity
refresh publishes `refreshing` with handle `.value` held; identity change
publishes `pending` with snapshot `previous` when a last-good is proven.
See the Resources transition table.
