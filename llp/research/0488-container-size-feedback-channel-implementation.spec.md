# LLP 0488: Container-Size Feedback Channel — Implementation Spec (LLP 0195 S2)

**Type:** Spec
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Contract, Kernel, Renderer (web host), Apple host, Runtime, Verification
**Author:** Charlie Cheever / Claude (Fable 5)
**Date:** 2026-08-19
**Revised:** 2026-08-20 (r7 — editorial W3C-processing-model conversion
(spec wave, LLP 0505 program): new §1.1 Conformance (BCP 14 + four
conformance classes), §1.2 Terminology, §3.9 processing-model
algorithms (production, publication, application, loop
prevention/termination on the ResizeObserver-depth precedent,
budget-ledger accounting quoting LLP 0487's family-3 join verbatim),
and §5.1 conformance suite (registered hooks verified against
exact-verify.json; owed obligations listed, none fabricated). Additions
only; **no semantic change** — §§3–6 govern on any disagreement, and
existing section numbering is untouched.) 2026-08-20 (r6 — family-3 sharing set made internally
consistent: §3.7 item 3 now states the same closed set as §6 — budget
ledger **and correlation envelope** only — and defers to LLP 0487's
decision envelope for the identity fields a recorded choice decision
binds, rather than restating them.) 2026-08-20 (r5 — lockstep identity sync with RFC 0491 r15:
§3.8's report tuple restated as 0491's full six-field raw address — the
node-allocation generation added so same-root destroy/recreate cannot
alias a report, `ProducerId` vs LLP 0374's Acto `producerIncarnationId`
distinctness held by reference to 0491 WS-A, and the D1 sentence renamed
onto `ExecutionGeneration`.) 2026-08-20 (r4 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision. §3.2's normative text rewritten to the landed sparse
last-emitted projection (the round-2 catch: the §2.8 delta had amended it
while the normative bullet still specified the wrong diff); the landed
`contract-container-feedback-budget` web A/B check inventoried (native
numbers still owed); the §5 budget restatement + cardinality caps flagged
as a proposed LLP 0195 amendment in §3.4's shape; §6's inherited
invalidation scoped to families 1–2; per-producer/per-root report
partitioning + commit-epoch publication added to §3.4/§3.8;
overflow/lifecycle-loss repair (admission caps over report drops,
latest-dirty retention, resync snapshot, typed
Update/Invalidate/CapabilityLost); the `measuredBoxFeedbackV1` capability
negotiation + host matrix; the 64-bit counter encoding rule; invariant 4
re-homed on typed consumers; TUI decoupled from the identity-tuple
freeze; stale §2.1 line cites and §8's W2/W3 leftover fixed.) 2026-08-20 (r3 — program super-refine round 1: recast as a
**completion spec** — W1 (web leg), the W2-shape registered checks, and W3
(kernel measured set + report FFI, `PropId.Measure` 103) have landed, and
the box/coordinate pin now lives in the layout-profile authority
(`measuredBoxObservation`: border-box, node-relative), so §2 gains a landed
audit, the stale §9 questions close, and remaining work narrows to W4/W5 +
hardening. New normative content from the round: typed record families
(size observation / placed geometry / counterfactual evaluation — one
budget+correlation envelope, three records, per LLP 0486 §7's corrected
matrix); a W4-prerequisite identity/staleness contract (scoped identity
tuple aligned with RFC 0491's producer/root incarnations, invalidation
records, applied watermark); latest-valid-wins coalescing replacing
FIFO-replay of stale frames; the no-measurable-cost budget claim restated
as a differential no-consumer-update claim with cardinality/per-consumer
caps owed; convergence/detect-and-pin policy located in typed consumers;
`pinToSafeArea` dropped from the consumer list (LLP 0199 routes it through
the host safe-area channel); the 0199 fallback-authoring gap inherited from
0486's correction as a W5 gate; the web leg bound to the production-runner
direction per LLP 0500; W4 reconciled with RFC 0491 WS-C's envelope.)
2026-08-20 (§4.1 hysteresis ruled: static admission + detect-and-pin, not a deadband)
**Related:** LLP 0195 §4.3 (owns the channel's design and its named
deliverable/budget — **this spec implements it and does not amend it**),
LLP 0199 (measured relations — consumer 1; its resolvers return `null` today),
LLP 0486 §7 (the one-channel-three-consumers ruling this spec executes) and
§10 WS3, LLP 0161 (macOS live resize — the direct-apply path and
`renderSnapshotRevision`, the intended coalescing key), LLP 0297 (runtime
thread and UI worklets — the affinity rules every hop here obeys), LLP 0139
(EXLR reverse frames — the nearest shipped kernel-geometry→JS pipeline),
LLP 0150 (authority map / verification registry), LLP 0349 (programmable
layout — a later consumer of the same budget)

## Summary

LLP 0195 §4.3 specifies a kernel→runtime container-size feedback channel as a
named deliverable with a precise per-frame invariant and a performance
budget, and rules that no measured-layout feature in LLP 0195 or LLP 0199 may
start before that invariant and budget are met. LLP 0486 §7 then ruled that
this one channel is built once, budgeted once, and consumed by three features.

When this spec was drafted (2026-08-19) none of it existed. As of
2026-08-20 the picture has inverted for three of its five workstreams: the
opt-in's compiler/protocol hops, the kernel measured set and its
one-crossing report FFI, the web ResizeObserver leg with its registered
check, and the box/coordinate pin have **landed** (§2.8 carries the
audit). This spec is therefore a **completion spec**: it records what
exists, pins the contracts the landed pieces already chose, and scopes
the remaining work — the native host→runtime hop (W4), the first consumer
with real fallback authoring (W5), and the hardening items the round-1
program review added (§3.7–§3.8).

The load-bearing findings that shaped the design, each verified against
the tree at drafting time (§2 carries the citations; §2.8 the delta):

1. **The kernel has no per-node opt-in concept of any kind.** `measure=true`
   is not a new value on an existing mechanism; it is the first per-node
   "interested set" the kernel will carry. This is the single largest piece
   of new machinery in the channel.
2. **No layout result ever reaches JS by a push today.** The kernel's
   post-layout `publish_layout_changes` receipt is *pull-based, host-side,
   and unconditional over every node*; it is consumed by the Apple host and
   never enters the JS runtime. The channel needs a genuinely new hop
   (host→runtime), not a widening of an existing one.
3. **`renderSnapshotRevision` — the coalescing key LLP 0195 §4.3 names — is a
   host-side (Swift) counter, not a kernel concept.** The kernel cannot key
   its reports by it. The key must therefore be *assigned at the host publish
   boundary*, and this spec makes that explicit rather than leaving an
   impossible instruction in the design.
4. **The web host observes no container size at all today** — only whole
   window size, through one lazily-installed `resize` listener. The web leg
   is genuinely the easy one, but it is new code, not a configuration change.
5. **The S2 acceptance benchmark LLP 0195 §4.3 gates on does not exist.**
   The registered `contract-responsive-layout` check is S0–S1 (viewport size
   classes) only. The budget gate must be built as part of this work, and it
   must be built *before* the consumers, or the gate is retroactive theater.

## 1. Scope

**In scope:** the channel — opt-in, kernel measured set, transport to the
runtime, the runtime-side reactive source, the web leg, the coalescing and
quiescence rules, the budget benchmark, diagnostics, and the
callback-affinity rows.

**Out of scope:** the three consumers themselves. Flipping LLP 0199's
resolvers, LLP 0195's container-relative branching, and LLP 0486 §8.3's
rung-2 choice islands are separate, gated work items that ride this channel
once it is green. This spec defines the contract they consume.

**Explicitly not amended:** LLP 0195 owns the channel's design. Where this
spec appears to add semantics rather than mechanism, the addition is that
RFC's owner's call, not this spec's — this spec must not become a second
design authority for the channel. There were two such places, and both are now
settled: **§4.1 (hysteresis) was ruled by the owner on 2026-08-20** — static
admission where provable, detect-and-pin as the fail-safe, not a deadband —
and is recorded as resolved in LLP 0195's Open Questions; **§3.4 (the
coalescing key)** remains a proposed amendment, since LLP 0195 §4.3 names a
key the kernel cannot stamp.

### 1.1 Conformance

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**,
**SHOULD NOT**, and **MAY** in §§1.1–1.2, §3.9, and §5.1 are to be
interpreted as described in BCP 14 (RFC 2119 / RFC 8174) when, and only
when, they appear in all capitals. Pre-existing prose in the rest of
this document keeps its plain-language meaning; its testable form is §4's
invariants and §5.1's conformance suite. These sections were added in r7
as an **editorial W3C-form restatement**: they restate the normative
content of §§3–6 and define no new semantics; on any perceived
disagreement, the owning section (§§3–6) governs and the discrepancy is
a bug in the restatement.

Four **conformance classes** partition the obligations:

1. **Record producer** — the kernel measured-set/report machinery
   (§§3.1–3.2) plus the host publish boundary (§3.3 hop 1–2), and the
   web observer leg (§3.6) acting as producer on web. A record producer
   MUST report only opted-in nodes (§3.1), MUST use the sparse
   last-emitted projection (§3.2), MUST NOT emit empty reports (§3.2),
   MUST report the pinned box and coordinate space
   (`measuredBoxObservation`: border-box, node-relative — §3.6), MUST
   key every report by the §3.8 scoped identity tuple, and MUST emit
   typed invalidation on node disposal and root teardown (§3.8).
2. **Channel scheduler** — the transport and runtime staging (§§3.3–3.5).
   A channel scheduler MUST marshal host→runtime asynchronously (never a
   synchronous bounce or wait, §3.3), MUST deliver at most one
   runtime-visible transaction per frame per §3.4's keying, MUST
   partition per producer/root per pass (or carry root identity plus the
   root's causal revision on every row) and publish commit-causally
   (§3.4), MUST apply latest-valid-wins coalescing with countable drop
   diagnostics (§3.8), MUST enforce the applied watermark (§3.8), and
   MUST have a `docs/callback-affinity.md` row for every boundary
   crossing before merge (§3.3).
3. **Consumer** — a family-typed reader (§3.7, §6). Family-1/2 consumers
   MUST inherit the invalidation contract and the next-frame rule (§6),
   MUST render correctly against the `null` pre-report value (§3.5), and
   own their admission forms and detect-and-pin policy (§4.1 — the
   channel itself never pins, damps, or rewrites a report). A family-3
   consumer's seam is the §3.7 item-3 closed sharing set (budget ledger
   and correlation envelope only), governed by the LLP 0487 join
   sentence quoted in §3.9 Algorithm 5.
4. **Budget-ledger accountant** — the accounting obligations of §5 and
   §3.8: cardinality caps bound **admission**, not published-row
   delivery; overflow behavior is deterministic and diagnosed; the
   differential A/B benchmark is the evidence form.

### 1.2 Terminology

- **Measured set** — the kernel's per-node opt-in membership: the node
  ids whose resolved boxes are to be reported (§3.1).
- **Measured-box report** — the post-layout sparse record: for each
  measured node whose box changed, the node id and resolved box (§3.2).
- **Record family 1 / 2 / 3** — size observation / placed geometry
  (`alignTo`) / counterfactual evaluation (choice `fits`), per §3.7's
  one-envelope-three-records ruling (LLP 0486 §7).
- **Correlation envelope** — this spec's §3.8 scoped identity and
  pass/snapshot fencing vocabulary, under the name LLP 0487 uses for it
  ("what 0488 names the **correlation envelope**").
- **Budget ledger** — the shared accounting of the channel's §5 budget:
  per-frame changed-row caps, total-measured-node caps, per-consumer
  caps, and deterministic exhaustion data (§3.9 Algorithm 5).
- **Scoped identity tuple** — RFC 0491 r15's six-field raw address
  `(ProducerId, ExecutionGeneration, rootId, root incarnation, local
  node id, node-allocation generation)` (§3.8).
- **Coalescing key** — native: `renderSnapshotRevision` stamped at the
  host publish boundary plus the kernel layout-pass sequence (§3.4);
  web: the delivery-frame counter (§3.6). Not comparable across hosts;
  each monotonic and frame-granular.
- **Applied watermark** — the runtime's last applied (pass sequence,
  snapshot revision) per scoped node; out-of-order or pre-reset reports
  are rejected (§3.8).
- **Latest-valid-wins** — per scoped node per delivery, only the latest
  valid report publishes; superseded reports drop with a countable
  diagnostic (§3.8).
- **Admission cap** — refusing the Nth `measure=true` acquisition with a
  diagnostic, in preference to dropping published rows (§3.8).
- **Oscillation / detect-and-pin / pin** — a node's class flipping
  across consecutive frames beyond a bound; the owning typed consumer
  detects it and pins to the last stable value, emitting a diagnostic; a
  pinned surface is a reported defect, not a resolved one (§4.1).
- **Resync snapshot** — the consumer-initiated re-read of current boxes
  after a detected gap (§3.8).
- **Typed lifecycle states** — `Update(box)`, `Invalidate(...)`,
  `CapabilityLost(...)` (§3.8).
- **`measuredBoxFeedbackV1`** — the negotiated host capability with
  explicit unsupported/degraded semantics (§3.8).

## 2. Ground truth: what exists today

Everything below was verified against the tree on 2026-08-19. This section
exists because the channel's design has been discussed for two months in
terms that assume plumbing which does not exist.

### 2.1 Kernel side

- `Kernel::publish_layout_changes` (`kernel/src/lib.rs:2196-2218`) diffs
  every laid-out node's frame against `last_layout_frames` after a successful
  layout pass, populating `published_changed_nodes` and
  `published_changed_frames`.
- Consumed by probe/copy/consume FFI getters
  `exact_kernel_take_changed_node_ids` (`kernel/src/ffi.rs:377`) and
  `exact_kernel_take_changed_frame_ids` (`kernel/src/ffi.rs:412`);
  frames are then read via `exact_get_layout` / `exact_get_absolute_layout`
  (`kernel/src/ffi.rs:2879-2935`) or bulk-exported through
  `exact_get_layout_tree_buffer_v1` (`kernel/src/ffi.rs:3012-3051`,
  `kernel/src/layout_export.rs`).
- `docs/callback-affinity.md:283-293` classifies these explicitly as
  *synchronous sizing-probe/copy FFI getters, not callbacks*, running only
  inside `ExactEngineTreeDomain` on the kernel owner thread, with Apple
  copying the geometry records into the commit output before main applies
  them — "main never queries the kernel."
- Layout-touching opcodes are exactly three: `MeasureText = 0x06`,
  `ComputeLayout = 0x07`, `QueryLayout = 0x71`
  (`kernel/src/protocol/opcodes.rs:59-65,168-170`), confirmed against
  `tests/protocol/protocol-inventory.json`. **None carries per-node computed
  layout results upward as an unsolicited push.**
- There is no per-node interested/observed/opt-in set anywhere in the kernel.

**Reading:** the receipt machinery is real and shipped, but it is
unconditional (every node), pull-shaped, and terminates at the native host.

### 2.2 The coalescing key

- `renderSnapshotRevision` is defined at
  `ios/ExactApp/ExactApp/Engine/ExactEngine.swift:368` and incremented at
  `:1726` (`applyRenderSnapshot`), `:1741` (`patchRenderSnapshotNodes`),
  `:1795` (`publishHostReloadStateRestore`), plus reset at `:940`; the
  presenter protocol requires it at
  `ios/ExactApp/ExactApp/Renderer/ExactRenderSurface.swift:2338`.
- LLP 0161's direct-apply resize path: `ExactEngine.updateViewportSize`
  (`:948`) → `flushPendingViewportSizeUpdate` (`:1193-1277`) → per-root
  `commitLayout` → `rebuildRenderSnapshotImmediately()` →
  `syncViewportMetricsToJS` (`:1534-1553`). Under runtime-thread mode the
  flush instead routes through `commitViewportOnRuntimeThread` (`:1321`) —
  the single sanctioned bounded-wait carve-out during `inLiveResize`.

**Reading:** the key LLP 0195 §4.3 names lives on the Swift side of the FFI
boundary. §3.4 resolves this.

### 2.3 Runtime-thread affinity

`docs/callback-affinity.md` is the merge-blocking ledger (its header rules,
lines 8–13: "every runtime callback … must have a row here… Unclassified
callbacks block merge"). The rows closest in shape to a post-layout
measurement report are the **Contract reactive host-state callbacks**
(ENG-24199, `docs/callback-affinity.md:324-329`): platform callbacks received
on main or a sensor queue "must marshal asynchronously to the runtime
executor, never synchronously bounce or wait," one physical callback fans out
to per-root signals, and `publishHostTransaction`/`publishHostState` are
runtime-thread affine with the Contract graph, staging FIFO during an action
batch and draining at the outer batch exit.

The kernel→host FFI callbacks (`docs/callback-affinity.md:454-465`) are all
*host answers a question the kernel asked mid-layout* (text measure, native
control measure, segment preparation) — the reverse direction. There is no
existing kernel→host push-callback registration pattern for layout results.

### 2.4 Web side

- ResizeObserver appears in `packages/exact-renderer/src/` only for
  graphics/media surfaces: WebGPU (`gpu/webgpu-canvas-backend.ts:168-172`),
  Rive (`rive/webgl2-backend.ts:706-711`), graphics surface input
  (`graphics/surface-input-web.ts:447-467`), canvas2d
  (`graphics/canvas2d-backend.ts:817-820`). `dom-mirror.ts:5225,5262`
  references them only for teardown.
- Per-node DOM attachment point: the `viewIdByDom`/`domByViewId` bidirectional
  index in `dom-mirror.ts` (`:5215-5216`, `pruneViewIdMappings`).
- Size reaching Contract reactivity today is whole-viewport only:
  `runtime/viewport-host-input.ts:39-89` (one lazily-installed `window`
  `resize` listener fanning to a subscriber `Set`) surfaced as the `viewport`
  capability (`runtime/viewport-capability.ts:1-40`).

### 2.5 The runtime-side push shape

`packages/exact-contract/src/runtime/signals.ts` provides `signal` (`:753`),
`computed` (`:994`), `effect` (`:1090`), `batch` (`:696`), and owner scoping
(`:1163-1214`). The end-to-end precedent for pushing host values into
Contract reactivity is `runtime/host-bindings.ts`:
`publishHostTransaction` (`:615-627`, FIFO-staged inside a batch), the
reference-counted per-root acquire/dispose cell pattern (`:686-760`), and the
hydration `replayGate` (`:646-660`) that buffers one pending value across a
hydration boundary and applies it exactly once.

`runtime/responsive.ts:16-41` additionally establishes the *ownerless*
module-global signal pattern, adopted deliberately so one component's unmount
cannot freeze every consumer.

### 2.6 Opt-in precedent

No per-node flag exists that travels Contract source → IR → protocol → kernel
*and* gates kernel behavior. The best structural wire trace is `selectable`,
which makes every hop but is an opaque kernel passthrough:
`compiler/builtin-schema.ts:131` → `packages/exact-renderer/src/protocol/encoder.ts:1100-1129`
→ `packages/exact-core/src/protocol/opcodes.generated.ts:76`
(`PropId.Selectable = 9`) → `kernel/src/protocol/prop_decoder.rs:60`.

Global (not per-node) kernel behavior flags do exist —
`exact_kernel_set_default_pixel_density` (`kernel/src/ffi.rs:280-297`) and
the interactive-resize span declaration (`:298-310`) — but they are declared
by the host over FFI, not routed from Contract source.

### 2.7 Verification

- `contract-responsive-layout` (`exact-verify.json:7347-7365`) is titled and
  scoped **LLP 0195 S0–S1**; it validates size-class tokens, `match size`
  parsing/analysis, and viewport-breakpoint branch swaps. `contract-match-size`
  runs the same test file.
- `contract-relation-measured` — LLP 0199's companion check
  (`llp/0199-…:389`) — is registered nowhere.
- `macos-live-resize-benchmark` (`exact-verify.json:9907-9929`,
  `scripts/check-macos-live-resize.mjs`) gates the **presenter apply** path
  (LLP 0161), not this channel.

**Reading:** LLP 0195 §4.3's sentence "the channel does not ship until it
meets them under the `contract-responsive-layout` benchmark" currently points
at a benchmark that cannot measure it. §5 fixes that.

### 2.8 What has landed since drafting (audited 2026-08-20)

Every §2.1–§2.7 "does not exist" claim above is drafting-time ground
truth; this delta supersedes it where they disagree:

- **W1 (web leg): landed.** `packages/exact-renderer/src/container-measurement.ts`
  + `container-measurement-web.ts`, the runtime source
  `packages/exact-contract/src/runtime/measured-box.ts`, and the
  registered check `contract-container-size-feedback-web`
  (`exact-verify.json`, in-profile).
- **W3 (kernel): landed.** The measured set, `MeasuredBox`/report types,
  `publish_measured_box_changes` (a sparse projection keeping its own
  last-emitted map — the correct mechanism; §3.2's "reuse
  `publish_layout_changes`'s diff state" is amended accordingly, since
  size-only change detection needs the sparse map, not the absolute-frame
  diff), and the one-crossing getter
  `exact_kernel_take_measured_box_report` returning ids+boxes with a
  layout-pass sequence (`kernel/src/lib.rs`, `kernel/src/ffi.rs`).
- **Opt-in hops: landed.** `measure` in `builtin-schema.ts`,
  `PropId.Measure = 103` in `tests/protocol/protocol-inventory.json`, the
  encoder mapping, and the kernel decode arm.
- **The box/coordinate pin: landed in the right authority.**
  `tests/layout/layout-profile.json` pins `measuredBoxObservation` as
  border-box, node-relative, origin (0,0) — both engines cite one
  sentence, exactly as §9 hoped; the question is closed, not open.
- **Affinity rows: landed** for the W3 getter and the web coalescer
  (`docs/callback-affinity.md`). The W4 host→runtime hop remains unwired
  and unclassified — still true, still the gate.

Still true from the drafting-time audit: no native host→runtime
measured-box hop exists (W4); LLP 0199's resolvers still `return null`;
the S2 budget benchmark's native leg does not exist; TUI is unaddressed.

## 3. The channel (normative)

### 3.1 Opt-in

A node opts in with `measure=true` (LLP 0195 §4.3's spelling). The flow
follows the `selectable` hop structure of §2.6 exactly — allow-list entry in
`compiler/builtin-schema.ts`, resolve/encode in the renderer's protocol
encoder, a new `PropId` in the generated opcode table (regenerated from
`tests/protocol/protocol-inventory.json`, never hand-edited), and a decode
arm in `kernel/src/protocol/prop_decoder.rs`.

It diverges from `selectable` at the last hop: the kernel does not merely
store the value. It maintains a **measured set** — the node ids whose
resolved boxes are to be reported. Requirements:

- Membership is a property of the node, updated by the same `SetProp` path
  that sets it, and removed when the node is removed or the prop cleared.
- The set is expected to be small and sparse relative to the tree. The
  implementation must not iterate the full tree per frame to compute it.
- **Opt-in is the cost boundary.** LLP 0195 §4.3: the kernel reports only for
  nodes that asked, "so the measurement edge — and its cost — exists exactly
  where a container query or an 0199 measured relationship needs it, and
  nowhere else." A design that measures everything and filters at a later hop
  violates the deliverable even if its output is identical.

### 3.2 Kernel side: the measured-box report

After a layout pass completes for a root, and after `publish_layout_changes`
has run, the kernel produces a **measured-box report**: for each node in the
measured set whose resolved box changed since its last report, the node id
and its resolved box.

- The report is a **sparse projection over the pass's laid-out subtree
  index with its own last-emitted map** — the landed mechanism
  (`publish_measured_box_changes`, `last_reported_measured_boxes`,
  `kernel/src/lib.rs:2221-2261`): iterate only the measured set, skip nodes
  not in this pass's laid-out index, compare the node-relative border-box
  *size* against the last-emitted entry, and emit on change. Do **not**
  literally reuse `publish_layout_changes`'s absolute-frame diff state (the
  drafting-time instruction, amended in round 1 and now here in the
  normative text): that diff keys on absolute frames, so it would emit on
  position-only moves and contradict the `measuredBoxObservation` pin. No
  second all-node frame table is created either way — the sparse map holds
  only measured nodes.
- Report **resolved box**, and state in the payload's schema which box
  (content vs. border) and which coordinate space (node-relative vs.
  absolute) it is. The existing split between `exact_get_layout` and
  `exact_get_absolute_layout` shows this ambiguity is real and has bitten
  before; the channel must not inherit it as an implicit convention.
- Empty reports are not emitted. A frame in which no measured node's box
  changed costs one comparison per measured node and produces nothing — this
  is what makes "no measurable cost to a resize that does not cross a
  breakpoint" (§5) achievable.

### 3.3 Transport: kernel → host → runtime

Three hops, each with a named affinity:

1. **Kernel → host.** A probe/copy/consume FFI getter in the shape of
   `exact_kernel_take_changed_frame_ids` (`kernel/src/ffi.rs:382-411`),
   returning ids *and* boxes together. Returning ids alone would force the
   host into N follow-up `exact_get_layout` calls, which is the shape the
   budget cannot afford. Runs on the kernel owner thread inside
   `ExactEngineTreeDomain`, exactly like its neighbor.
2. **Host → runtime.** The host copies the report into the commit output and
   publishes it to the JS runtime. Per LLP 0297 and the ENG-24199 rows, this
   marshals **asynchronously to the runtime executor** — never a synchronous
   bounce, never a wait. This hop is new; it is the one §2.1 says does not
   exist today.
3. **Runtime → Contract graph.** Delivered through the
   `publishHostTransaction` shape of §2.5: one transaction per frame carrying
   all measured boxes, staged FIFO if inside an action batch and drained at
   the outer batch exit.

Every one of these gets a row in `docs/callback-affinity.md` before merge.
Unclassified callbacks block merge, and three new boundary crossings without
rows would be the largest affinity gap the ledger has taken.

**W4 rides the WS-C envelope or declares its adapter.** RFC 0491 WS-C
defines the upward encoding family — shared framing and frame-correlation
ids, three delivery owners — and names container-size feedback as one of
its kernel-export flows; RFC 0490 likewise places container feedback in
the common receipt family. The W4 hop therefore either adopts WS-C's
framing/correlation from the start, or lands as an explicitly temporary
adapter with a named migration-and-deletion gate tied to WS-C's Phase 3.
A third, permanently bespoke channel is not an option this spec offers.

### 3.4 The coalescing key — proposed LLP 0195 amendment

LLP 0195 §4.3 requires reports "batched into one runtime-visible update keyed
to the frame's `renderSnapshotRevision` (LLP 0161)". Per §2.2 that counter is
host-side; the kernel cannot stamp it.

**Proposed rule:** the kernel's report carries a kernel-side monotonic
**layout pass sequence number**; the host stamps the outgoing transaction
with the `renderSnapshotRevision` of the snapshot that pass produced,
carrying both. The runtime keys coalescing and staleness on
`renderSnapshotRevision` as LLP 0195 intends; the pass sequence exists so a
report can be attributed to its layout pass on the kernel side of the
boundary, which is where its diagnostics live.

On web there is no `renderSnapshotRevision`; §3.6 defines the web key.

**Per-root partitioning (round-2 correction).** One report-level revision
cannot identify mixed-root rows: the kernel's compute-all path coalesces
boxes from multiple roots into one report, while Apple primary and
secondary root views maintain **independent** `renderSnapshotRevision`
counters. The transaction therefore either partitions **per producer/root
per pass**, or carries root identity plus the root's causal revision on
every row. And publication is **commit-causal**: the host publishes a
report only after the corresponding commit applies in the current epoch,
stamping the *existing* commit epoch/sequence — this channel mints no
third sequencing authority beside the commit path and the pass sequence.

This is mechanism, not semantics — but it changes a sentence in LLP 0195
§4.3, so it is **flagged for that RFC's owner** rather than assumed.

### 3.5 Runtime side: the measured-box source

Per node, a reactive source exposing the last reported box, built on the §2.5
primitives:

- **Reference-counted acquire/dispose**, per the `acquireScheme`/`acquireInsets`
  pattern (`host-bindings.ts:686-760`): the physical subscription exists while
  a consumer holds it and is disposed at zero references.
- **Ownerless signals**, per `responsive.ts:16-41`'s deliberate choice, so one
  component's unmount cannot freeze other consumers of the same node's box.
- **Hydration**: the `replayGate` pattern (`host-bindings.ts:646-660`) applies.
  A measured box that arrives across a hydration boundary is buffered and
  applied exactly once. This is what keeps the channel from breaking the
  LLP 0288 byte-identical-first-paint gate.
- **Initial value before the first report is `null`**, and consumers must
  render correctly against it. This is not a corner case: it is the
  server-render state, the pre-first-layout state, and the no-JS state. It is
  the same discipline LLP 0199's resolvers already follow by returning `null`
  into the authored fallback, and the same graceful-degradation rule the repo
  applies everywhere — a missing measurement degrades, never fails.

### 3.6 The web leg

The browser already computes every box; the web leg observes rather than
reports.

- Attach a `ResizeObserver` per opted-in node, installed on acquire and
  removed on release — the lazy-install/refcount-release shape of
  `viewport-host-input.ts:56-89`, applied per node instead of per window.
  **Binding target (LLP 0500):** the conformance-bearing web leg binds to
  the **production runner's** direct-DOM node index; the compatibility DOM
  mirror's `viewIdByDom`/`domByViewId` keying (§2.4) is a secondary
  diagnostics surface on the mirror's own retirement clock, never the leg
  the checks certify. (A shared observer per renderer/document, rather
  than one observer object per node, is the recommended implementation
  shape; observably equivalent either way.)
- **Coalesce per frame.** ResizeObserver batches observations, but the
  platform does **not** guarantee at most one delivery per animation frame
  (nested-change loops re-deliver); the leg imposes its own frame boundary —
  one transaction per frame under §3.8's latest-valid-wins — and must not
  re-enter layout synchronously inside the callback.
- **The web coalescing key** is the frame in which the observer batch was
  delivered (a runtime-side monotonic counter), since
  `renderSnapshotRevision` does not exist here. Consumers must not depend on
  the two hosts' keys being comparable — only on both being monotonic and
  frame-granular.
- Use the observation mode matching the pinned box — the layout-profile
  authority pins `measuredBoxObservation` as **border-box, node-relative,
  origin (0,0)** — and assert that correspondence in a test. A silent
  box-model mismatch between the two hosts is precisely the class of bug
  LLP 0486's parity work exists to prevent.

### 3.7 Typed record families (one envelope, three records)

LLP 0486 §7's corrected matrix rules that the consumers are three
execution classes, not one payload. This channel's raw measured box is
**family 1** only:

1. **Size observation** — `{scoped node identity, border box, generations,
   pass/frame key}`: serves container-relative branching (LLP 0195) and
   `matchWidth`/`matchHeight` (LLP 0199).
2. **Placed geometry** (`alignTo`) — root-local coordinates plus
   coordinate-space identity and scroll/ancestor invalidation generations.
   **Not derivable from ResizeObserver** (position-only movement and
   scrolling produce no resize entries) and not carried by family 1's
   record; a distinct, demand-driven record family sharing this channel's
   budget and correlation envelope. Deferred until specified; `alignTo`
   is not a v1 consumer of family 1.
3. **Counterfactual evaluation** (choice `fits`) — an evaluator
   capability, not an observation; its closed sharing set is the
   **budget ledger and correlation envelope only** (the §6 family-3
   wording is the same set — never this channel's size records or
   scheduler). LLP 0487 §6 owns the evaluator semantics, and 0487's
   decision envelope defines the identity fields any recorded choice
   decision binds; this spec defers to that envelope rather than
   restating it.

A consumer declared against the wrong family is a specification error,
not an integration bug.

### 3.8 Identity and staleness (W4 prerequisites)

The landed web leg keys routing by bare `viewId` and applies frames
without a monotonic fence, and the kernel's `reset()` re-issues node ids —
sufficient for the single-root dev web case, not for the native hop.
Before W4:

- **Scoped identity tuple.** Reports and subscriptions key on
  RFC 0491 r15's **full six-field raw address** —
  `(ProducerId, ExecutionGeneration, rootId, root incarnation,
  local node id, node-allocation generation)` — the allocation
  generation included so a same-root destroy/recreate of a local id
  cannot alias a report, and `ProducerId` distinct (by reference to
  0491 WS-A) from LLP 0374's Acto `producerIncarnationId`, which joins
  at the Acto boundary and is never a report key. A reset or producer
  restart thus makes stale reports unrepresentable rather than merely
  unlikely. Per frozen LLP 0500 D1 / RFC 0491's binding: a
  **HotRevision plan patch never bumps `ExecutionGeneration`** — an
  HMR edit must not read as a
  producer restart to this channel, or every edit would orphan live
  subscriptions.
- **Counter encoding, frozen before W4.** Kernel and Swift sequences are
  64-bit; the current web frame field is a JS safe-integer `number`. The
  host-neutral schema specifies a lossless 64-bit representation (or an
  explicit split encoding) plus a wrap/incarnation rule, rather than
  letting each bridge narrow it independently.
- **Lifecycle invalidation.** Node disposal and root teardown emit
  invalidation records on the same channel, so a consumer holding a
  measured-box source learns the node is gone instead of holding a
  forever-stale box.
- **Applied watermark.** The runtime records the last applied
  (pass sequence, snapshot revision) per scoped node and rejects
  out-of-order or pre-reset reports.
- **Coalescing is latest-valid-wins, not FIFO replay.** The current web
  staging delivers every staged frame in order; under backlog that
  replays obsolete sizes inside one runtime frame. Normative rule: per
  scoped node per delivery, only the latest valid report publishes;
  superseded reports are dropped with a countable diagnostic (gap
  metrics, never silent loss of the *fact* that frames were dropped).
  Transaction atomicity holds per delivery, not per staged frame.
- **Loss must be recoverable, so caps bind admission, not delivery.** The
  kernel advances its last-emitted state when it publishes, so a row
  dropped *downstream* of publication is never re-emitted while the box
  stays unchanged — a dropped-newest-row policy can strand a consumer on a
  stale box forever. Rule: prefer **subscription admission caps** (refuse
  the Nth `measure=true` acquisition with a diagnostic) over dropping
  published rows; where per-frame delivery must bound, retain the
  **latest dirty** row per scoped node across deliveries (chunking, not
  loss); and provide a **resync snapshot** request so a consumer that
  detected a gap can re-read current boxes without waiting for a change.
- **Lifecycle states are typed.** A measured-box source publishes
  `Update(box)`, `Invalidate(node disposed / root torn down)`, and
  `CapabilityLost(channel gone)` — with defined consumer rebinding on
  each. Today kernel disposal clears its internal state without emitting
  any invalidation record; W4 closes that.
- **Capability negotiation.** The channel is a negotiated
  **`measuredBoxFeedbackV1`** host capability with explicit
  unsupported/degraded semantics: a Contract consumer can distinguish
  "this host does not implement the channel" from
  "supported-but-not-yet-reported" (today's `acquire(viewId)` is silently
  inert when the host is missing). The host matrix — Apple, Windows, web,
  TUI (§9) — is written before W5 flips any public behavior.

### 3.9 Processing model — the algorithms (r7 editorial restatement)

The five algorithms below restate §§3.2–3.8 and §5 in W3C
processing-model form (the ResizeObserver specification is the register
model; its loop-limiting depth rule is Algorithm 4's direct precedent).
They define no new semantics; §§3.2–3.8 govern on any disagreement.

**Algorithm 1 — produce a measured-box report** (record producer; §3.2).
When a layout pass completes for a root, after `publish_layout_changes`
has run:

1. Let *report* be an empty list.
2. For each node *n* in the measured set:
   1. If *n* is not in this pass's laid-out subtree index, continue.
   2. Let *box* be *n*'s resolved border-box size, node-relative,
      origin (0,0) (the `measuredBoxObservation` pin).
   3. If *box* equals the last-emitted entry for *n*, continue.
   4. Append (*n*'s scoped identity tuple, *box*) to *report* and set
      the last-emitted entry for *n* to *box*.
3. If *report* is empty, emit nothing and stop (a no-change frame costs
   one comparison per measured node and produces nothing).
4. Otherwise stamp *report* with the kernel layout-pass sequence and
   make it available to the host via the one-crossing getter.

The producer MUST NOT iterate the full tree (step 2 iterates the
measured set only) and MUST NOT reuse the absolute-frame diff state
(position-only moves would emit).

**Algorithm 2 — publish a report to the runtime** (channel scheduler;
§§3.3–3.4). For each report taken from the kernel:

1. Partition rows per producer/root per pass, or attach root identity
   plus the root's causal revision to every row.
2. Wait until the corresponding commit applies in the current epoch;
   stamp the outgoing transaction with that commit's epoch/sequence and
   the root's `renderSnapshotRevision` (web: the delivery-frame
   counter), carrying the kernel pass sequence alongside.
3. Marshal the transaction **asynchronously** to the runtime executor —
   never a synchronous bounce or wait.

**Algorithm 3 — apply a delivery in the runtime** (channel scheduler →
consumer boundary; §§3.5, 3.8). On receipt of staged transactions at
the outer batch exit:

1. Per scoped node, select the **latest valid** report among staged
   rows; count and drop superseded rows (the drop count is observable).
2. Reject rows whose (pass sequence, snapshot revision) does not
   advance the applied watermark for that scoped node, and rows whose
   scoped identity tuple does not match a live subscription (a reset or
   producer restart makes stale reports unrepresentable).
3. If inside a hydration boundary, buffer through the `replayGate` and
   apply exactly once.
4. Publish `Update(box)` to each node's reactive source in one
   transaction; on disposal/teardown rows publish `Invalidate`; on
   channel loss publish `CapabilityLost`.

**Algorithm 4 — loop prevention and termination** (§4 invariant 1 +
§4.1). The ResizeObserver analogue: where ResizeObserver breaks
same-frame loops by deferring deliveries deeper than its depth limit to
the next frame and signalling an error, this channel (a) defers **all**
feedback to the next frame, and (b) bounds cross-frame oscillation by
admission and detect-and-pin in the owning typed consumer:

1. Within a frame: reports are produced only after layout completes; a
   runtime reaction that changes a layout input applies on frame N+1,
   never within frame N; no synchronous re-entry into layout occurs
   from a measurement report. Per-frame work is therefore finite (one
   comparison per measured node, at most one transaction), so each
   frame terminates.
2. Across frames: a feedback chain advances at most one step per frame.
   Where a feedback path from a measured box to its own layout inputs
   is statically establishable, admission refuses it fail-closed
   (§4.1's A5 posture) and the loop never exists. What escapes
   admission is detected by the owning typed consumer (the same node's
   class flipping across consecutive frames beyond a bound) and pinned
   to the last stable value with a diagnostic.
3. Termination argument: every execution either reaches the §4
   invariant-4 fixpoint within its bounded frame count, or is pinned in
   bounded frames — loud and bounded, never invisible thrash. The
   channel itself never pins, damps, or rewrites a report; the policy
   lives in the typed consumers (§4.1's placement rule).

**Algorithm 5 — budget-ledger accounting** (budget-ledger accountant;
§3.8, §5). Charges bind admission, not published-row delivery:

1. On the Nth `measure=true` acquisition beyond the admission cap:
   refuse the acquisition with a diagnostic.
2. Where per-frame delivery must bound: retain the latest dirty row per
   scoped node across deliveries (chunking, not loss).
3. On a detected gap: serve the resync-snapshot request so the consumer
   re-reads current boxes without waiting for a change.
4. Exhaustion is deterministic, countable data — never silent loss of
   the fact that rows were dropped. For the family-3 seam this is
   governed by the LLP 0487 join, quoted verbatim: "**The 0488 family-3
   join, stated once for both documents:** choice adopts LLP 0488
   §3.8's scoped identity and pass/snapshot fencing vocabulary (what
   0488 names the **correlation envelope**) for this envelope, and
   shares 0488's budget **ledger** for §6's owed M4 work-admission cap;
   it does NOT adopt the 0195/0488 channel's scheduling, invalidation,
   damping, or oscillation contract — `fits` evaluation and §7 T3
   timing are 0487-owned — and ledger exhaustion is deterministic data
   on the M5 host-defect path, never `unknown`."

**Web-leg identity note (flagged, unchanged from r4/r6):** the landed
web leg keys routing by bare `viewId` and applies frames without a
monotonic fence — a recorded **W4 rebase obligation onto the six-field
scoped identity tuple** (§3.8), sufficient only for the single-root dev
web case until rebased.

## 4. Invariants (LLP 0195 §4.3, restated as testable assertions)

Each of these is a test, not a paragraph. A channel that cannot demonstrate
them has not met the deliverable.

1. **One-directional per frame.** Data flows layout → measure → runtime read
   → (next frame) layout. A runtime reaction to a reported size that changes
   a layout input applies on the **next** frame. **Test:** a fixture whose
   measured node's report changes a layout input must be observed to change
   layout on frame N+1, never within frame N; and no synchronous re-entry
   into layout occurs from a measurement report (assert on the call path, not
   just the outcome).
2. **Coalesced.** All reports for a frame arrive as one runtime-visible
   update keyed per §3.4, under §3.8's latest-valid-wins rule. **Test:**
   K measured nodes changing in one frame produce exactly one runtime
   transaction, for K > 1; and a backlog of N staged frames for one node
   publishes only the latest valid box, with the drop count observable.
3. **Opt-in only.** **Test:** a tree with zero `measure=true` nodes produces
   zero reports and no per-frame cost attributable to the channel; a tree
   with one produces reports for exactly that node.
4. **Convergence/quiescence.** A content-stable surface reaches a fixpoint
   within a bounded number of frames. **Test:** the bound is asserted with a
   concrete number, and a fixture that would oscillate across a breakpoint
   boundary is detected and pinned **by the owning typed consumer** rather
   than thrashed — the channel itself stays truthful observations (§4.1's
   placement rule), so this invariant's fixtures live with each consumer's
   detection, and the channel-level assertion is only that observations
   remain unmodified under the oscillation.

### 4.1 The oscillation policy — ruled 2026-08-20

LLP 0195 §4.3 deferred hysteresis to its Open Questions while requiring that
oscillation be "detected and pinned rather than thrashed." Detection and
pinning cannot be implemented without choosing the policy, so this blocked
invariant 4. **Charlie ruled on 2026-08-20: static admission where provable,
with detect-and-pin as the fail-safe. Explicitly not a deadband.**

Two reasons drove it, and both should be carried into the implementation:

- **A deadband would break more than it fixes.** Requiring `threshold + δ` to
  cross back makes a size class *history-dependent*: the same width resolves
  differently depending on which side it was approached from. That destroys
  the pure size→class function, and the server — which has no history — cannot
  reproduce the client's class. It therefore lands directly on LLP 0288's
  byte-identical-first-paint gate. The textbook control-theory answer is the
  one that fights hardest with the rest of Exact.
- **This program already solved the same problem once.** LLP 0487's A5 does
  not damp feedback at runtime; it refuses at compile/link time to admit an
  instance whose feedback is not provably unreachable, fail-closed. Adopting
  the same posture here gives the platform **one** account of layout feedback
  instead of two, and makes the pathological case largely stop existing rather
  than being managed after the fact.

So the channel's obligations are:

1. **Admission first.** Where a feedback path from a measured box back to that
   box's own layout inputs can be established statically, reject it at
   compile/link time in A5's shape — fail-closed on unprovable or opaque
   evidence, with a diagnostic — rather than accepting it and damping the
   result. LLP 0487 §5 A5 is the reference for what a certified
   unreachability witness looks like; this channel's subject set is broader
   (it covers LLP 0199's relations and LLP 0195's own branching), so the
   forms will differ even though the posture does not.
2. **Detect-and-pin as the fail-safe.** For what escapes admission — dynamic
   or cross-boundary cases that cannot be proven either way — detect
   oscillation (the same node's class flipping across consecutive frames
   beyond a bound) and pin to the last stable value, emitting a diagnostic.
   A pinned surface is a **reported defect**, not a resolved one: the point is
   to make thrash loud and bounded, not to make it invisible.

**Sequencing:** detection is policy-free and lands first, so the bound and the
pin's exact shape are set against observed behavior rather than guessed. This
also means invariant 4 can be closed incrementally — detection demonstrates
boundedness before the admission forms are complete.

**Where the policy lives (round-1 clarification):** the channel itself
stays truthful observations — it never pins, damps, or rewrites a report.
Admission checking, oscillation detection, the pin *value* (raw size vs
relation value vs size class vs candidate — each consumer pins in its own
vocabulary), reset conditions, and the diagnostics live in each **typed
consumer** (LLP 0195 branching, LLP 0199 relations; LLP 0487 explicitly
consumes raw unpinned snapshots and has no v1 cycle handler, which this
firewall preserves). **Admission-form seeds**, so W5 does not invent a
second A5: positive — a measured node whose consulted box is
descendant-independent (A1-positive in 0487's sense); negative — a
shrink-to-fit node whose descendants change *because of* the consumer's
response to the measurement. The full form vocabulary grows in the
consumers' own specs.

This is a **proposed amendment to LLP 0195's Open Questions**, which is where
the hysteresis question lives; it is recorded there as resolved.

## 5. Budget and acceptance

LLP 0195 §4.3's budget, which this spec adopts unchanged as the gate:

- No more than **one frame** of added latency to a size-dependent
  re-resolution (read on frame N, re-resolved layout on frame N+1).
- The LLP 0161 direct-apply resize path stays within its existing budget
  (apply ≈ publish, ~3ms) while container queries are active.
- An active `measure=true` node adds **no consumer layout/presentation
  update** on a resize that does not cross its breakpoint; a crossing
  costs at most one extra frame. Stated as a *differential* claim, not
  "no measurable cost": observation, transport, transaction, and signal
  work exist for every reported change, and the A/B fixture pair is what
  bounds them. Owed with the benchmark: **cardinality caps** —
  per-frame changed-row and total-measured-node caps, a per-consumer
  cap, and a deterministic overflow behavior (drop-with-diagnostic per
  §3.8) — LLP 0486 §7's cap requirement, made concrete here.

**This restatement is a proposed LLP 0195 amendment (the §3.4 shape), not
an implementation note:** 0195 §4.3's third bullet reads "no measurable
cost to a resize that does not cross its breakpoint," and the differential
no-consumer-update form plus cardinality caps replaces that acceptance
criterion. Flagged for 0195's owner exactly as §3.4's coalescing-key
change is; until adopted, two texts state the gate and 0195's controls.

**The benchmark must be built before the consumers — and its web half has
landed.** `contract-container-feedback-budget` is registered
(`exact-verify.json`, "LLP 0488 W2"): the production-preset A/B pair —
control route vs `{measure:true}` route, no-crossing max extra frames = 0,
crossing ≤ 1 — manual, web. Still owed: the **native** A/B numbers on the
same differential shape (the W4 gate's evidence), and
`contract-responsive-layout` remains S0–S1. Required:

- Extend the `contract-responsive-layout` registry entry's coverage to S2, or
  register a sibling S2 entry — either way registered in `exact-verify.json`
  per LLP 0150, never an unregistered root `check:*` script.
- The resize-budget leg follows `scripts/check-macos-live-resize.mjs`'s
  established fixture-route shape (`/__exact/benchmarks/resize-sentinel*`),
  adding a measured-node variant so "with the channel active" and "without"
  are the same scene differing in one prop. **The no-crossing budget claim is
  a differential measurement**; without the A/B pair it is unfalsifiable.
- Run on a quiet fleet host per the standing routing rules. Heavy gates on
  the primary box produce fake timing reds, and this budget is exactly the
  kind of number that would be misread.

**Charlie's sign-off gate:** the measured budget numbers are a semantic
decision (they set what every future stage-1/stage-2 feature may spend), and
Charlie named this as one of his decision gates. The lane reports the
measured numbers and its recommendation; the ruling is his.

**RULED (Charlie, 2026-08-20) — web-leg evidence signed off; the budget is
ratified as the spend ceiling.** Strict-enforcement quiet-fleet-host capture
(Office M1 Max, operator label `quiet-office-m1max`, 12 samples per leg,
artifact `report-2026-08-20T23-23-53-548Z.json`): read→re-resolved layout
exactly 1 frame (min=median=max); presentation→re-resolved max 1 frame;
no-crossing A/B differential 0 frames; crossing 0 extra frames; A/B scene
parity PASS; consumer on the Contract reactive path. The diagnostic
no-crossing millisecond differential (median 0 ms, single-sample max 1.1 ms
against the suggested ±1 ms band) was ruled scheduler noise on an
otherwise-zero distribution and accepted. The native-leg numbers arriving
with W4 are measured against this same, now-ratified budget; that
measurement is a conformance check, not a reopened ruling.

**Native leg (2026-08-22) — CONFORMS.** The W4 native A/B differential ran
on a macOS host (Durand M4 Pro, dedicated runtime thread, 12 samples per leg;
durable report `docs/reports/llp0488-native-differential-durand-20260822.json`):
read→re-resolved layout exactly 1 frame; presentation→re-resolved 0 frames;
no-crossing A/B differential 0 frames (min = median = max); crossing 0 extra
frames; diagnostic no-crossing ms differential median 0.97 ms (−1.4…3.5 ms).
Measured against the ratified ceiling above; it does not reopen the ruling.
Getting there fixed a presenter defect (patch-eligible commits carrying
geometry rendered stale frames since 2026-07-14) and gave macOS the
display-paced `requestAnimationFrame` path with a host-monotonic stamp; the
ms diagnostic still wants a quiet-host re-capture (the capture host carried a
sibling lane). Record: `issues/closed/20260821-llp0488-native-differential-not-run.md`.

### 5.1 Conformance suite — registered hooks and owed obligations

The conformance suite is the set of **registered** `exact-verify.json`
checks below (LLP 0150: one verification registry; never an
unregistered root `check:*` script). Verified against the registry
2026-08-20:

**Registered hooks (exist today):**

- **`contract-container-size-feedback-web`** (LLP 0488 W1) — the web
  leg: runtime source + observer tests, the `measure=true` encoder
  opt-in, protocol-table regeneration check. Covers Algorithm 1 (web
  producer), Algorithm 3's K>1 coalescing, and invariant 3's
  zero-opt-in leg on web.
- **`contract-container-feedback-budget`** (LLP 0488 W2, manual,
  quiet-host) — the differential A/B budget pair (control vs
  `{measure:true}`; no-crossing max extra frames = 0, crossing ≤ 1),
  frame-gate acceptance, provenance-stamped reports; Charlie's
  sign-off gate consumes its diagnostic millisecond differential.
  Covers Algorithm 5's evidence form and invariants 1–3 on web.
- **`contract-responsive-layout`** / **`contract-match-size`** —
  LLP 0195 **S0–S1 only** (size-class tokens, `match size`,
  viewport-breakpoint swaps); adjacent, not this channel's gate.
- **`macos-live-resize-benchmark`** — gates the LLP 0161 presenter
  apply path whose ~3ms budget §5 bullet 2 protects; adjacent guard,
  not a channel check.

**Owed obligations (no registered check exists — test-obligation list,
in W-order; registering each is part of the owning workstream, and no
check name below may be cited as existing until it appears in the
registry):**

1. Native A/B numbers on the W2 differential shape (the W4 gate's
   evidence) — §5's "required" list.
2. The S2 extension (or registered S2 sibling) of
   `contract-responsive-layout` — §5's "required" list.
3. `contract-relation-measured` (LLP 0199's companion) — registered
   nowhere today; lands with W5 (§6 item 1).
4. Invariant-1 fixtures: next-frame application + no-synchronous-re-entry
   asserted on the call path (native).
5. Invariant-4 fixtures: the concrete convergence bound, and the
   oscillation fixture pinned by the owning typed consumer with the
   channel's observations unmodified.
6. The §3.6 box-model correspondence assertion (observer mode ↔
   `measuredBoxObservation` pin) as a cross-host parity test.
7. §3.8 contract fixtures at W4: watermark rejection, typed
   `Invalidate`/`CapabilityLost` emission, admission-cap refusal,
   latest-dirty retention under bounding, resync snapshot, and the
   64-bit counter encoding rule.

## 6. Consumers and what each needs from this contract

In LLP 0486 §7's order:

1. **LLP 0199 measured relations** (`runtime/layout-intent.ts:77-99`):
   `matchWidth` and `matchHeight` currently `return null` into the
   authored fallback. Scope corrections of record: **`pinToSafeArea` is
   not this consumer** — LLP 0199 routes it through the existing
   safe-area host channel — and **`alignTo` is family 2** (§3.7), not
   servable by the size record. And the r1 claim of "zero new authoring
   surface" was too strong, per 0486 §7's verified correction: LLP 0199
   requires every relation to carry a `fallback`, but ordinary parsed
   relation calls build `{kind, op, args, span}` with **no fallback
   populated** (`compiler/expr.ts`). W5 therefore either lands
   fallback authoring end to end (parse → IR → runtime → lost-flag
   diagnostics) before flipping resolvers, or scopes its first release
   to records that receive a fallback through another governed path —
   and public behavior stays **capability-gated until the native leg
   carries the same record**, so web-first transport work never becomes
   a cross-host behavior split.
   Register `contract-relation-measured` (LLP 0199 §389) when it lands; it is
   registered nowhere today.
2. **LLP 0195 container-relative branching** — its own S2 surface.
3. **LLP 0486 §8.2 rung-2 choice islands** — a **family-3 consumer**
   (§3.7): counterfactual candidate evaluation shares this channel's
   budget ledger and correlation envelope, never its size records or
   scheduler (LLP 0487 §6 owns the evaluator semantics). It inherits the
   budget and cannot ship before the channel's ledger exists.

Family-1 and family-2 consumers inherit the same invalidation contract:
one-directional per frame, coalesced by snapshot revision, ≤1 frame
latency, no hand-rolled measurement edges (LLP 0199's own rule). Family 3
inherits the budget ledger and correlation envelope only — its timing is
LLP 0487's (same-pass on kernel hosts, §7 T3), and imposing this channel's
next-frame rule on it would violate that spec (LLP 0486 §7's matrix).

## 7. Sequencing

LLP 0486 §7 rules that the web side may ship first — "the reverse of Exact's
usual order, and fine: it de-risks the semantics while ENG-scoped kernel work
lands." This spec concurs and sharpens it:

1. **W1 — the contract and the web leg.** ✅ **Landed** (§2.8): runtime
   source, web observer, opt-in hops, registered web check.
2. **W2 — the benchmark** (§5). *Web half landed:* both the W1-shape
   check (`contract-container-size-feedback-web`) and the differential
   A/B budget pair (`contract-container-feedback-budget`) are registered.
   Owed: the native A/B numbers. Still ordered before W5.
3. **W3 — the kernel measured set and report.** ✅ **Landed** (§2.8),
   including the sparse last-emitted map and the one-crossing getter.
4. **W4 — the host→runtime hop** (§3.3, on the WS-C envelope or a
   declared adapter) with its `docs/callback-affinity.md` rows, the §3.8
   identity/staleness contract, and the native numbers against W2's
   budget. Charlie's sign-off gate. **This is now the program's critical
   path.**
5. **W5 — consumer 1**: LLP 0199's `matchWidth`/`matchHeight` resolvers,
   with fallback authoring per §6, capability-gated until native parity;
   register `contract-relation-measured`.

W2's remaining half runs in parallel with LLP 0486's WS1 lanes; W4 is the
item to schedule against kernel/host capacity, not calendar.

## 8. Risks

- **The kernel's first per-node interested set** is new structure in the
  hottest code in the system. The failure mode is a per-frame full-tree walk
  hiding inside an innocuous-looking membership query.
- **Runtime-thread mode + live resize.** The channel's host hop lands in the
  same neighborhood as LLP 0161's `commitViewportOnRuntimeThread` bounded-wait
  carve-out (`ExactEngine.swift:1321`). Adding a second synchronization point
  there is how the ~3ms budget dies. The async-marshal rule is not
  stylistic.
- **Box-model divergence between hosts** (§3.6) — silent until a consumer
  disagrees across hosts, which is exactly the bug class LLP 0486 exists to
  end. Pin it and test it, do not converge on it by accident.
- **Hydration**: a measured box arriving mid-hydration that changes rendered
  output would break the LLP 0288 byte-identical-first-paint gate. The
  `replayGate` pattern is the mitigation; it must be used, not merely cited.
- **Oscillation** — ruled (§4.1: admission first, detect-and-pin
  fail-safe, policy located in typed consumers); the residual risk is a
  consumer shipping before its admission forms exist.
- **Retroactive budget.** If the consumers land before the benchmark, the
  budget becomes a description of whatever was built. §7 orders W2's
  remaining native numbers before W5 for this reason (W3 has already
  landed).

## 9. Open questions

*(Closed since drafting: the hysteresis policy — ruled in §4.1 and
recorded in LLP 0195; the box/coordinate-space pin — landed in
`tests/layout/layout-profile.json` `measuredBoxObservation`, exactly the
one-sentence authority this section asked for.)*

- Does the measured-box source belong to the node or to the *container role*
  a consumer asked about? Choice layout's island rung may want "the box my
  candidates are being fitted into," which is not always the opted-in node's
  own box.
- Should the web leg's coalescing counter and the native
  `renderSnapshotRevision` be unified into one runtime-visible "frame id"
  concept? §3.6 deliberately does not, to avoid inventing a cross-host
  identity nobody needs yet.
- **TUI: in or out, explicitly — a consumer-scope decision, decoupled
  from the identity tuple.** The tuple is RFC 0491's and host-agnostic;
  nothing about TUI blocks freezing it. What is owed is TUI's *scope*
  ruling: either the channel exists on TUI (a kernel host — the measured
  set already compiles there) with a consumer named, or TUI is recorded
  out-of-scope for S2 with a reason. An implicit third host is the one
  disposition this spec refuses.
- The W4 delivery vehicle: native-first WS-C envelope adoption vs a
  temporary adapter with a deletion gate (§3.3) — decided with the
  RFC 0491 WS-C owner's schedule.
