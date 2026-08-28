# Host Data-Plane Feed — Schema (v0)

**Tracked artifact** — required by LLP 0297 A1
(`llp/0297-runtime-thread-and-ui-worklets.rfc.md`); authored for W4a
(ENG-22783). **Status: W4a COMPLETE** — every record type is implemented
(producer + routing + removals in phase 1, selection in phase 2, module
identity + constraint queue in phase 3, secondary surfaces + per-root
a11y in phase 4/ENG-22820), the A1 owner-scope audit is always-on in dev
builds with a clean suite and clean live sessions, A2 off-main
measurement is validated, and OQ10 is decided (hybrid as-built). Every
PR that extends the feed reviews this file (B6-style rule).

**Implementation (pre-flip form):** `ExactEngine.syncNodesFromKernel` is
the producer — it runs inside the tree-domain owner scope and ends by
publishing an `ExactHostDataPlaneCommit`
(`ios/ExactApp/ExactApp/Engine/ExactHostDataPlaneFeed.swift`): commit
generation (monotonic per engine instance; §4.8 resets bump a reset epoch
and restart numbering), synced-node count, the commit's removal records,
and the OQ10 production cost. The record *stores* pre-flip are the mirrors
the pass already builds — `ExactEngine.nodes` (NodeRecord, now including
the per-node `eventHandlers` routing table), the render/presenter
snapshots, and the merged accessibility tree.

## Purpose and ownership

After W4a, the main/UI thread never enters kernel FFI (outside the
tree-domain owner scope) — the kernel keeps exactly one owner, the runtime
thread. Everything main needs from the tree arrives through this feed:
a **runtime-produced, versioned, root-scoped snapshot** published alongside
each commit and consumed lock-free on main.

- **Producer:** the runtime thread, at commit time (after
  `processBuffer` + layout, where `syncNodesFromKernel` runs today).
- **Consumers on main:** `ExactEngine`/`ExactView` node mirrors and the
  presenter, event routing, the accessibility tree, the selection surface,
  the Motion-domain geometry mirror (§4.4), and agent/a11y readers (which
  state their freshness via the generation).
- **Not in the feed:** state-mirror slices handed to modules — raw
  seqlock-safe shared-memory reads, exempt by design (A1).

## Versioning, scoping, delivery

- Every publication carries a **commit generation** (monotonic, per
  engine generation — §4.8 resets bump the engine generation and restart
  commit numbering; consumers treat records from a stale engine generation
  as droppable).
- Records are **root-scoped** (multi-window: one logical stream per
  `rootId`; secondary windows consume only their root's stream).
- Transport is a main-consumable buffer published with the B4 opcode-ring
  commit (same swap discipline; the feed rides the commit, it is not a
  second synchronization protocol). Whether a publication is full-snapshot,
  dirty-diff, or hybrid is **OQ10** — decided by measurement, not here.

## Record types (v0)

### 1. NodeRecord

Replaces the `syncNodesFromKernel` read pass (`ExactEngine.swift:1688-1847`).
Per node: `id`, `nodeType`, `parentId`, `children[]` (ordered), `depth`,
**layout frame** (root-local x/y/w/h — the feed keeps today's root-local
convention; scroll adjustment stays a consumer concern), **style snapshot**
(the numeric style the presenter applies plus the ~20 string props read
today — text, fontFamily, imageSource/svgSource, accessibility strings,
testId, href-adjacent props, etc.; W4a enumerates the exact list from the
read pass and pins it here), and **text content**.

### 2. NodeTransitionRecord

Structured transition entries consumed for main-side animation
(`ExactEngine.swift:1830`, `applyTransitionEntries`). Emitted with the
commit that created them, in tree order.

### 3. RemovalRecord — ✅ implemented (ENG-22783)

Explicit node-removal / native-view-disposal records
(`ExactHostDataPlaneRemovalRecord`) — replace the prune-during-sync
stale-node sweep: the producer computes removals once and consumers
dispose in record order (detach-before-destroy), so OQ10's dirty-diff
decision changes the record *source*, not the consumer contract, and
Motion never holds a binding to a node it has not been told is gone.

### 4. EventRoutingRecord — ✅ implemented (ENG-22783)

The event-handler routing table: per node, the full `eventType →
handlerId` map, read with one batched FFI call per node
(`exact_get_node_event_handlers`, packed u64 records) during the sync
pass and mirrored on `ExactViewNode.eventHandlers`. Main routes input
against the mirror (`ExactEngine.getEventHandler` /
`isInteractiveNode`) instead of querying the kernel per event — this
also removed an owner-scope violation: interactivity used to be computed
by kernel probes during the *coalesced deferred* snapshot rebuild,
outside the tree-domain scope. Freshness rides the commit: the
host-patch fast path admits only text/opacity/transform/background ops,
so every batch that can carry bind/unbind takes the full sync path.
`ExactView` (ENG-22820) no longer carries copies at all: secondary
surfaces are root-scoped projections of the engine mirror and delegate
routing to `ExactEngine.getEventHandler` / `nodeHasInteractiveHandlers`.
The dispatch bootstrap additionally learns each surface root's NODE root
from its ComputeLayout ops (`surfaceRootNodeIds`), which is what lets
window roots join `rootIds`, sync into this feed, be partitioned out of
the primary snapshot, and lay out at their own window's viewport.

### 5. AccessibilityTreeRecord — ✅ implemented (W4a)

The merged accessibility/semantics tree plus per-root snapshots
(`ExactEngine.accessibilityTree` / `accessibilitySnapshotsByRoot`),
captured together by the sync pass at the same cost as the previous
merge (the merge already serialized per root). The host bridge's
per-root a11y reads serve from the mirror — no kernel FFI.

### 6. SelectionDocumentRecord — ✅ implemented (ENG-22783, W4a phase 2)

Main's selection surface is a set of interaction-time *computations*, not
reads. The kernel serializes its **built** selection document
(`exact_selection_document_json`: segments with the kernel's grapheme
offsets/counts, physical per-text-host grapheme lengths, total positions,
contain ranges, kernel-resolved per-node selection modes, and
source-identified `contain` / `all` regions — wire types in
`kernel/src/selection.rs::SelectionDocumentWire`), and the sync pass
mirrors it per root (`ExactSelectionDocumentMirror`,
`ios/ExactApp/ExactApp/Engine/ExactSelectionDocument.swift`) **only when
the kernel's selection generation moved** (`exact_selection_generation` —
every live invalidation source bumps it and rides a protocol commit;
`exact_selection_content_changed` has no live callers today, and a future
structured-selection caller must also dirty this mirror).

**Per-operation decision table (all five run locally on main):**

| Operation | Decision | Why |
|---|---|---|
| `selectionDocumentTotalPositions` | local (stored) | serialized field |
| `projectSelectionRange` | local | integer arithmetic over kernel-segmented offsets; repeated view ids represent disjoint physical ranges around non-selectable inline runs (ENG-24419); runs per mouse-move during drag — an async hop here would lag the highlight |
| `selectionDocumentRangeForViewRange` | local | same arithmetic, inverse direction; physical positions inside an inline hole snap to its collapsed document boundary |
| `constrainSelectionRange` | local | contain-range clamping over serialized ranges |
| `selectionText` | local | the one grapheme-sensitive op: within-segment slicing walks Swift graphemes, validated against the kernel's per-segment `graphemeCount` at decode (divergence counted + warned once; worst case is copy text off by a cluster at range edges under Unicode-version skew — never index corruption, since all offsets are kernel-computed) |
| `copyItemsForRange` | local | emits the walk-authored copy skeleton (text slices, `BlockBreak` separators, participant slots) so Apple plain/RTF/HTML and copy preview cannot fork membership/order/separators (ENG-24421) |

The coordinator renders `copyItemsForRange` on main. Registered text/module
hosts may enrich exactly their item ranges; they never supply an independent
walk. Rich and participant MIME payloads are bounded below the canonical
plain-text cap, and UIKit/AppKit share the same assembler. Repeated view ids
remain separate ordered items around composed-text holes (ENG-24419).

Parity is enforced by an exhaustive test
(`testSelectionDocumentMirrorParityWithKernel`): every (start, end) pair
over a tree with nested inline runs, ZWJ-family emoji, flags, combining
marks, CJK, explicit/Pressable inline holes with opt-back-in, block breaks,
and a `selectable="contain"` subtree must match
the kernel FFI results for all five operations, plus synthetic-wire
fixtures for the inline-object/full-view-sentinel branches.

Resolved selection behavior is not re-derived from NodeRecord's raw
`selectable` prop. Each `resolvedModes` entry carries `viewId`, the effective
mode (`disabled | enabled | contain | all`), and the nearest explicit
`sourceViewId` when inheritance applies. Each `regions` entry carries that
source node, its `contain` / `all` mode, and its document range. The producer
joins those records onto the node batch before presenter apply; source ids let
Select All target the authored region instead of whichever descendant host
received the gesture. The shared
`tests/selection/selectable-mode-fixtures.json` corpus pins explicit and
inherited false, SelectionGroup containment, inherited all, and tierless native
views across the kernel serializer, Swift mirror/presenter gating, and web
`user-select` lowering (`selection-mode-agreement` in `exact-verify.json`).
`ExactEngine`'s coordinator providers consume the mirror; `ExactView`
(ENG-22820) shares the engine's coordinator and session store outright —
selection state is rootViewId-keyed internally, so one feed-backed
instance serves every surface and the per-view kernel-probing providers
are gone.

### 7. Module-id allocation — ✅ decided + implemented (ENG-22783, W4a phase 3)

`registerModule` returns a `moduleId` consumed synchronously
(`ModuleRegistry.swift:116,172`). **Decision: module bootstrap moves with
the tree-domain owner** (the RFC's second option), not a reserved
main-side id range. Registration is confined to the boot
ownership-handoff window: `ExactEngine.init`/`ExactRuntime.init` run
kernel creation + `syncWithKernel` (registerModule, state-offset read,
state-mirror attach) inside `ExactTreeDomainScope`, so the sync id return
never crosses a thread boundary — post-flip the whole window executes on
the runtime thread. Main-side consumers read ids/offsets from
`ModuleRegistry`'s lock-guarded `moduleMetadata` mirror
(`allMetadata()`/`getMetadata`), which is this feed's record store for
module identity; the kernel-side scope audit warns on any future
out-of-window registration. Rationale: ids are already
deterministic (sorted-name order, sequential from 1), there are no
post-boot registration call sites, and a reserved-range scheme would add
a kernel ABI (`register_module_with_id`) plus a deterministic-offset
constraint on the state-mirror allocator for no live caller.
State-mirror slice pointers handed to modules remain raw seqlock reads
(exempt).

### 8. Island descriptor + consumed-input vectors — ✅ producer live (ENG-22919 increment 1 data plane, ENG-23515 increment 4 runtime producer)

Same-frame layout islands (LLP 0313) need a data plane both sides of the
island can read. **OQ1 decision: this is a new runtime-published feed record
type**, not a Motion-domain side table. The descriptor is genuinely new data
— the §1 NodeRecord style snapshot is the *applied presenter* style, while the
island pass needs the Taffy *input* style (flex bases, min/max, percentages,
aspect-ratio, gap). Making it a feed record type means it rides this feed's
commit versioning and reset epoch instead of standing up a second
synchronization surface while the Motion domain (LLP 0099) is still unbuilt
(consistent with the "no dedicated Motion-domain geometry structure while
Motion is unbuilt" decision below).

The record store is `ExactHostDataPlaneIslandStore`
(`Engine/ExactHostDataPlaneIslandDescriptor.swift`), owned by
`ExactHostDataPlaneFeed.islandStore`. Per LLP 0313 §3 each
`ExactIslandDescriptor` carries: island root id, commit generation, reset
epoch, per-root descriptor generation, closure class (fixed-envelope |
closed-ancestor), external envelope, frozen subtree topology + per-node Taffy
input style, latest parent constraints, the §5 measurement records (the three
admissible shapes: `.none`, `.frozenIntrinsic`, `.constraintTable`), binding
descriptors (SharedValue slot → node/layout-property), and per-slot
generation/tombstone metadata. Commits record their island inputs (§6) as
`ExactIslandConsumedInputVector`s kept in a bounded main-side history for the
input-matched drift replay; freshness is compared **per slot** (§6 apply
policy).

Lifecycle (§5/§6): descriptors retire on explicit removal, font-registry
change (measured islands only), intrinsic-size invalidation (covering nodes),
and SharedValue slot tombstone; an engine/reset epoch bump kills every
descriptor and its input history, wired through `ExactHostDataPlaneFeed.reset()`.

**Producer (increment 4, ENG-23515; Motion-table absorption, LLP 0099 M2):**
`ExactIslandRuntimeProducer` (`Engine/ExactIslandRuntimeProducer.swift`)
publishes on Apple presenters
descriptors at the feed publication point of every commit apply, lifted from
the commit batch's own records — `ExactNodeSyncRecord.style` carries the
kernel's Taffy *input* style (`StylePropsFFI`), so the lift is bit-faithful —
and records provenance-bearing consumed-input vectors from the durable
SharedValue registry. Renderer registration lowers each eligible edge into
the inventory-owned `layout-island-binding` Motion descriptor; the Apple
graph resolves its SharedValue dependency to an opaque live registry tenant
and reconciles complete per-island sets around presenter apply. The legacy
`island.register`/`island.unregister` host operations are deprecated
debug-compatibility fallback, not production authority. Content-unchanged commits do not republish
(bound-property motion is not content). The store stays empty — and the
commit path pays only a boolean check — until something registers an
island. No new cross-boundary callback (see docs/callback-affinity.md).

### 9. Lists-v2 collection and anchor records — ✅ L2 producer live

`ExactListVisibilityCommitRecords` is the root-scoped collection projection
copied while `ExactEngineTreeDomain` owns the kernel. It carries the topology
certificate and presentation metadata, participant index, visibility receipts,
`AnchorResolutionReceiptRecordV1`, and `AnchorDeltaRecordV1` in the immutable
presenter commit. String and KeyV1 arms are copied before their FFI arenas can
expire; main never calls a collection getter.

The Apple presenters adopt both anchor record families from that commit. After
reconciling its frames, the presenter matches the delta's commit generation,
adds `deltaAlongAxis` to the native scroll offset, and publishes the corrected
offset plus the same commit's collection projection through the root's existing
family-1 scroll binding before the apply returns. An index-only commit publishes
the current offset too, allowing the Contract CC to perform boundary→item
handoff on the first intersecting row. This is feed content plus an existing
rooted event projection, not a second synchronization surface.

## A2 off-main measurement validation — ✅ validated (W4a)

LLP 0297 A2 requires text measurement proven correct off-main **before**
the flip. `ExactOffMainTextMeasurementTests` pins main/off-main parity for
every descriptor-construction path the measure pipeline exercises —
weight resolution (`fontApplyingWeight`), synthetic italic
(`fontApplyingItalic`), emoji incl. ZWJ families, CJK + mixed-script
fallback cascades, combining marks, letter spacing, and the
wrapped/clamped framesetter modes — uncached (full font resolution +
CoreText shaping off-main), cached (cross-thread cache reads), and under
4-thread cold-cache contention. The registry-invalidation half of A2 (the
invalidate-vs-mid-measure race) was closed in phase 3 by the
`.fontRegistryChanged` constraint message.

## Motion-domain geometry mirror — ✅ decided (W4a; §4.4)

Decision (same build posture as the OQ3 CDP target): **no dedicated
Motion-domain geometry structure is built while the Motion domain
(LLP 0099) is unbuilt.** The feed's NodeRecord frames
(`ExactViewNode.frame` in the engine mirror) and the presenter snapshot
are the main-side geometry surface — the two consumers that exist today
(worklet `measure(nodeId)` and `ExactHitTester`) already read the
presenter snapshot, which satisfies §4.4's "gesture hit-testing runs
against the main-side presenter snapshot, never the kernel." When
LLP 0099 lands its first driver, its recognizer/binding tables consume
these same feed records; the mirror is a reader of this feed, not a
second synchronization protocol.

The first driver exists as the ENG-22834 DEBUG demo
(`ExactWorkletGestureDrivers` + the `worklet.installGestureDriver` /
`worklet.simulateGestureDriver` host ops, macOS): a pan recognizer on the
node's presenter view invokes a worklet on the resident UI runtime and
applies the returned translation to the view's layer — geometry and view
resolution ride the presenter view index exactly as decided here. The
LLP 0099 recognizer/binding tables replace that registry when the Motion
domain lands; the invoke and apply seams carry over.

For the LLP 0297 §8 gesture row, `worklet.gestureDriverBurst` /
`worklet.gestureDriverBurstStatus` (DEBUG, ENG-22897) drive repeated
samples through the same production path on a main run-loop timer at a
fixed cadence and report the per-sample latency and cadence distribution
— measured 2026-07-05 at 120Hz × 600 samples: p95 0.248ms per sample
against the 8.33ms frame budget, zero over-budget
(`testGestureBurstSameFramePerformance120Hz` is the pinned gate).

For the LLP 0313 layout-island rows, the DEBUG macOS
`island.demoInstall` / `island.demoDrive` / `island.demoQuiesce` surface
now reports a `metrics` object in the status JSON: display target FPS,
target frame budget, display-frame count, max frame interval,
estimated dropped frames, and the last/max same-tick prediction cost from
inside the display-link tick.

**ENG-23517 hardware record (2026-07-09):** measured on Exact `909471408`
with an `ExactAppMac` Debug build, the default-on dedicated runtime thread,
and the built-in 3456×2234 Liquid Retina XDR display. The harness itself
reported `targetFps: 120` and `targetFrameMs: 8.333333333333334`; this is the
runtime display contract, not an inference from the machine model.

1. Open `/layout-lab`, press `direction-toggle`, and take a fresh Acto
   snapshot. Install a closed-ancestor demo over the `direction-column`
   node and its three `dir-box-*` children using the async main-serviced
   `island.demoInstall` host op.
2. Drive 600 sinusoidal height writes at an 8ms timer cadence. The display
   link recorded 597 prediction samples; `maxPredictionCostMs` was
   **0.214208ms** against the **8.333333333ms** 120Hz same-tick budget
   (`sameTickBudget120HzPassed: true`). Quiesce/settle reported
   `driftCount: 0` and `snapDistance: 0`.
3. Uninstall/reinstall to reset the frame counters. Before the stall the
   harness read 30 display frames, zero estimated drops, and an
   8.333333335ms max interval. Run an exactly 100ms busy loop through
   `/agent/run-js` on the dedicated app-runtime thread, then read status
   again. The main display-link counter continued advancing while
   `droppedFrameEstimate` stayed **0** and `maxFrameIntervalMs` stayed
   **8.333333335ms**. The runtime stall therefore introduced zero observed
   UI-frame drops on this 120Hz pass.

The initial and final bundled snapshots had zero diagnostics. A live semantic
`/agent/hit-test` returned `geometry: "model"`; the snapshot-pinned toggle tap
also returned `nativeReachability.geometry: "model"` and
`reachesTarget: true`. The node ids are snapshot-local; reproduce by the
stable test ids above, then pass the fresh numeric ids to `island.demoInstall`.

## Main-initiated writes (not feed content, listed for completeness)

Kernel *writes* from main become messages on the input/constraint queues,
never direct FFI.

**Implemented (ENG-22783, W4a phase 3):** the host constraint-message
queue (`Engine/ExactHostConstraintQueue.swift`) is the typed enqueue
point — thread-safe, case-coalescing, drained through the W1 runtime
executor inside the tree-domain owner scope. The font-registry remeasure
(ENG-22728's invalidate-and-relayout flow) is its first message: the font
bridge enqueues `.fontRegistryChanged` from the JS engine thread and the
owner performs `invalidateTextMeasurements` + relayout between its own
passes, which is what closes the A2 registry-invalidation-vs-mid-measure
race across the W4b flip. At the flip the executor re-points to the
dedicated runtime thread and this queue feeds the B5 platform-constraint
fast path.

Secondary-window viewport sizing (`ExactView.flushPendingViewportSizeUpdate`
write + root probes, `registerViewRoot`/`unregisterViewRoot`) is
scope-entered pre-flip; the B5 constraint buffer takes the write over at
W4b, and the macOS primary-surface live-resize tick becomes the §4.4
sanctioned servicing wait.

Lists-v2 scroll samples use the same queue for the inverse constraint direction:
main stores the latest `{root, contentOffsetAlongAxis, reason}` with
`.collectionAnchorContextChanged`, and the runtime owner calls
`exact_kernel_set_collection_anchor_context` inside its next tree-domain drain.
The write is latest-per-root and asynchronous; inset and scroll-command callers
use the same setter's §7.4 reason lane, while ordinary samples use `layout`;
prepend and boundary→item are inferred from consecutive kernel projections.

## Exit gate wiring

**Extended in phase 1 (ENG-22783):** the opt-in scope assertion now
covers **every `ExactKernel` method** (57 asserts, `#function`-keyed
dedup) — exemptions are `getStateMirrorSlice` (seqlock read, exempt by
design) and the `rawHandle`/`nodeCount`/`maxNodeId` accessors.
`finalizeProcessedBuffer` now enters the tree-domain scope itself (its
kernel reads previously ran bare on the shell drain path). A live
`EXACT_SCOPE_AUDIT=1` caltrain session (boot + navigation + presses)
after phase 1 reported exactly four out-of-scope contexts remaining —
`reset()`, `registerModule`, `getModuleStateOffset`, `getNodeChildren`
(diagnostics) — i.e. the audit became the residual work-list.

**Phase 3 (ENG-22783) retired all four:** boot (kernel create + module
bootstrap) and reset are scope-entered lifecycle windows in
`ExactEngine`/`ExactRuntime`; `debugRootChildren` serves from the node
mirror; the font-registry remeasure rides the constraint-message queue.
Expected primary-surface audit result: **zero out-of-scope contexts**.

**Phase 4 (ENG-22820) retired the secondary surface:** `ExactView` is a
root-scoped projection of the engine mirror (routing, selection, text
editing, and preflight all delegate to engine feed state); a live
desktop-lab window session under `EXACT_SCOPE_AUDIT=1` reported zero
out-of-scope contexts with a fully rendering secondary window. The one
remaining known kernel-backed main path is the a11y snapshot host bridge
(converts with the W3 a11y semaphore-bucket migration — the read marshals
through the runtime executor inside the owner scope). The always-on audit
flip in dev builds happens when that converts and the suite runs clean.

## OQ10 decision — ✅ hybrid as-built (W4a exit)

**Decision run (2026-07-04, macOS Debug, main app home = the 322-node
baseline fixture, `EXACT_FEED_TIMING=1`, always-on scope audit clean):**
boot full pass **5.81ms**; whole-tree-dirty steady state (theme flip —
every node's style changes, the true worst case) **9.9–10.6ms** per
commit over 5 commits. Small-change steady state from phase 1: 0.60ms @
25 nodes, ~1.6ms @ 92.

**Decision: hybrid, exactly as built.**
1. **Full-snapshot sync per structural commit stays v1.** Cost is
   bounded by tree size, matches the pre-feed baseline at boot, and
   lands on the runtime thread after W4b (not main). An incremental
   node-mirror would buy the most in exactly the case it cannot help
   (whole-tree style changes dirty everything).
2. **The host-patch fast path is the dirty tier** — text/opacity/
   transform/background-only commits bypass production entirely; B4
   unifies the two streams at the flip.
3. **Escalation trigger (recorded, not built):** if W4b/W5 telemetry
   shows sustained full-pass production above ~8ms on shipping trees
   (≈450+ nodes at the measured ~0.02ms/node), extend the patch-path
   allowlist and/or produce per-node dirty diffs from kernel change
   tracking. RemovalRecords already decouple the consumer contract from
   the record source, so that change stays producer-side.

## OQ10 baseline and phase-1 measurements (feed cost)

Baseline: the pre-feed full `syncNodesFromKernel` pass measured **5–8ms
for a 322-node tree** on macOS Debug (boot-log `kernel-parse-done →
sync-nodes-done`, 2026-07-04, W1 verification runs).

**Phase-1 measurement (ENG-22783, 2026-07-04, macOS Debug, caltrain-demo,
`EXACT_FEED_TIMING=1`):** the full pass *including* the new per-node
event-routing reads costs **0.60ms at 25 nodes, ~1.4–1.6ms at 76 nodes,
1.61ms avg / 2.04ms max at 92 nodes over 13 steady-state commits**
(live-departures ticker re-renders). Roughly linear ≈0.02ms/node —
extrapolating to the 322-node baseline tree lands ~6.5ms, inside the
pre-feed envelope: the routing-table read did not materially move
full-pass cost. The full-snapshot vs dirty-diff decision therefore stays
open on big-tree data; the deciding measurement should re-run the exact
322-node baseline fixture. Note the **host-patch fast path**
(text/opacity/transform/background-only commits, LLP 0161) bypasses the
sync pass and therefore does not publish a feed commit — patch commits
bump the render-snapshot revision but not the feed generation; the B4
transport unifies the two streams at the flip.

## Open questions — all closed at W4a exit

1. ~~Exact NodeRecord field list~~ — pinned by reference: the record IS
   `ExactViewNode` as populated by `syncNodesFromKernel` (type, parent /
   children / depth, layout frame, the applied style snapshot, text, the
   synced string props, `eventHandlers`, `isEditable`); the pass is the
   single producer, so the field list evolves with it under this file's
   review rule.
2. ~~Full-snapshot vs dirty-diff vs hybrid~~ — closed above (OQ10:
   hybrid as-built, with a recorded escalation trigger).
3. ~~Selection per-operation local-vs-hop decision table~~ — closed, §6
   above (all five local).
4. ~~Module-id strategy~~ — closed, §7 above (bootstrap confined to the
   scoped boot window; metadata mirror serves main).
5. ~~Feed memory ownership~~ — pre-flip the mirrors are main-side owned
   copies (`ExactEngine.nodes`, the merged + per-root a11y trees, the
   selection mirrors), so consumers that outlive a commit are safe by
   construction; B4's ring-slot lifetime governs only the transport at
   the flip, not these long-lived mirrors.
