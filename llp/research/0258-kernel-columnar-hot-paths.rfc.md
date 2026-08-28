# LLP 0258: Kernel Columnar Hot Paths

**Type:** RFC
**Status:** Superseded (curation pass 2026-08-23)
**Superseded by:** LLP 0491
**Systems:** Kernel, Layout, Protocol, FFI, Agent API, Semantics, Selection, Performance
**Author:** Charlie Cheever / Codex; Phase A executed by Tuft
**Date:** 2026-06-24
**Revised:** 2026-07-12 (as-built and evidence refresh — the Phase A follow-on landed as the packed structural export (`LayoutTreeNode`, consumed by the Windows host); LLP 0297/0322 threading resolutions folded in (kernel `hit_test` never serves pointer dispatch, and tree-domain sidecars are single-domain — no double-buffering); the loss of the Phase A benchmark evidence is recorded, with a re-measure-at-promotion-entry disposition and binding entry preconditions — see "As-Built and Evidence Status (2026-07-12)". Super-refine round 1 applied same day (fresh Fable Max + Codex gpt-5.6-sol ultra, artifacts under `llp/reviews/`): kernel hit-test caller inventory corrected — keyboard avoidance is a production commit-cadence caller via the inspector blocker path; lookup accounting completed (portal probe per DFS node, per-node event-handler join, composed-text runs); promotion-entry preconditions generalized and expanded (keyboard-avoidance fixture, lookup ledger, host fingerprint + committed baseline path, no-sidecar control arm); SharedValue cross-thread scope qualifier; stale tuple-API references re-pointed to `get_layout_tree_nodes`; hit-test cost conditionality stated precisely. Super-refine round 2 applied same day (fresh Fable Max + Codex ultra, both NOT READY on convergent residue): the two remaining "agent tier" annotations corrected to the bounded commit-cadence framing; Phase B animation-column text and the June Windows-join description dated/superseded inline; the rebuild recipe re-pointed to the as-built surfaces; promotion gates generalized to every candidate (sidecar or packed ABI) with a quantitative hit-test materiality gate (≥5% of eligible-commit cost on the keyboard fixture, or ≥0.25 ms per agent probe batch); promotion evidence redefined as paired same-session/same-machine runs with a demonstrated noise floor (`exact-perf.yml`'s `macos-latest` is a host class, not a machine); control-arm loophole closed (cleanup that clears a gate alone declines the columnar promotion) with named first candidates; precision fixes (inert-before-Taffy ordering, per-exported vs per-emitted row counts, root-init ancestor walk, frame-containment advisory semantics, eligible-commit cadence); OQ2 annotated; OQ9 (promotion-entry ownership) added. Super-refine round 3 applied same day (fresh Fable Max READY + Codex ultra NOT-READY on one Material): `portal_node_ids` moved out of the no-dual-state control arm — a persistent presence set is a derived sidecar and runs as its own gated candidate, never as ungated cleanup; `svg_node_ids` references updated to the as-built `svg_raster_node_ids` (renamed in `4305f68b8`, broader coverage and prop-mutation maintenance); the hit-test materiality denominator pinned to the tree-domain span and the agent probe batch defined (1K-probe fixture, per-request latency recorded); the lookup ledger upgraded to required-before-first-promotion and usable as a deterministic test invariant; the `Scene::layout` join leg specified as kernel-API simulation; `get_absolute_layout` hop cost enumerated; the advisory-semantics non-element short-circuit recorded; one-binary arm interleaving recommended; the resolver-alternative claim softened to "likely to dominate"; keyboard-avoidance and host-ops files added to Related. Super-refine round 4 applied same day (fresh Fable Max READY + Codex ultra NOT-READY, both independently flagging the same defect): the hit-test materiality gate rewritten in normalized live units — the 1K-probe batch threshold was a paper gate (0.25 ms over 1,000 probes ≈ 0.25 µs per request; the June figures cleared it 200× over) and is replaced by per-live-request-equivalent (≥0.25 ms per probe) or share-of-request (≥5%) arms, with the keyboard-arm denominator pinned to the full tree-domain resolution pipeline including the TS-side pass (deliberately, with rationale); Phase C item 2 corrected (tree columns alone remove the wide-`Node` lookup; the `taffy.layout` chase needs the Q3 cached-rect column under its own rules); the Shelved clause re-pointed to the rebuilt baseline; the non-element short-circuit widened to unknown-id/root; the all-roots walk scope recorded with a multi-root fixture variant; expected end-to-end gain (share × speedup) added to promotion reports; the ledger receipt specified as a versioned per-fixture cost signature; property-based mutation sequences recommended for derived sidecars; the handler-fold no-FFI clarification and the resolver-lands re-demotion pre-commitment added. Super-refine round 5 applied same day (fresh Fable Max READY + Codex ultra NOT-READY on gate executability): the keyboard-arm gate now names its instrument — a registered renderer/live-runtime benchmark driving eligible commits through the real TS path against a real kernel binding (today's renderer bench is dispatch-capture only), paired alongside the kernel-side fixture; the 3% write-path budget generalized to a candidate-specific maintenance matrix (prop set/clear, event bind/unbind, layout-pass cache population — with explicit N/A legs) and the in-tree bench coverage gaps recorded; the agent-arm fixture pins a canonical selector shape with invocation counts (coordinate-targeted requests probe the kernel twice); the serialization claim qualified (incremental serialization exists, default-off); the geometry-only resolver's output shape completed (adds the nearest scroll-container frame); mutation-tape replay recorded against OQ8; a versioned packed-schema fixture required before packed-ABI implementation. Previously revised 2026-06-24: Phase A baselines measured — see "Phase A Results (2026-06-24)"; reviewed round 1 - production layout surfaces, baseline gates, flat child columns, ownership rules; reviewed round 2 - cached layout rect for hit testing, staged RenderColumns, svg_node_ids precedent, reset() clearing, packed-export ABI as end state; reviewed round 3 - measure-first hit-test cost claims, per-child z-index read cost + paint-order rationale, ViewId-vs-NodeIndex hash-per-hop honesty, per-node cached-rect validity, Windows repeated Node re-fetches, write-path non-regression evidence)
**Related:** LLP 0003 (kernel boundary), LLP 0005 (incremental style updates), LLP 0017 (cross-view text selection), LLP 0145 (performance benchmarks), LLP 0150/0151 (lanes and verification), LLP 0159 (codebase health), LLP 0297/0322 (runtime-thread contract; main-side geometry severed from kernel storage), `kernel/src/lib.rs`, `kernel/src/layout.rs`, `kernel/src/tree.rs`, `kernel/benches/kernel_benchmarks.rs`, `kernel/benches/protocol_profile.rs`, `packages/exact-host-windows/src/scene.rs`, `packages/exact-renderer/src/inspector.ts`, `packages/exact-renderer/src/keyboard-avoidance.ts`, `packages/exact-renderer/src/host-ops.ts`, `ios/ExactApp/ExactApp/Kernel/ExactKernel.swift`, `vendor/ibex/src/engine/hermes_runtime_ios.cc`

## Summary

Evaluate and, if the measurements justify it, introduce **columnar hot-path
storage** inside the Rust kernel for the fields that are read in tight loops:
layout geometry, parent/child traversal metadata, transform/opacity/overflow,
event-handler presence, node type, style dirty bits, and selected semantics/
selection flags.

This is a **Rust-side SoA/AoS hybrid proposal**, not an Odin rewrite. Odin's
first-class SoA syntax is attractive for this style of code, but the likely
Exact win is the data layout itself: move hot scalar fields out of the wide
`Node` object and `HashMap<ViewId, Node>` lookup path when a frame operation
does not need props, full styles, transitions, text payloads, or per-node maps.
Rust can express that with arenas and sidecar columns while keeping the
existing Taffy integration, FFI surface, fuzzing, and Cargo benchmark stack.

The proposal is intentionally gated. We add benchmark coverage first, then
prototype narrow sidecars. We only promote a sidecar to production if it clears
predeclared thresholds on realistic workloads and does not fork the kernel's
canonical state.

Two different layout-read surfaces must be measured separately. The Windows
host consumes the whole-tree export when building `RenderNode`s (since
2026-06 the packed `get_layout_tree_nodes` variant — see "As-Built and
Evidence Status (2026-07-12)"), while the Apple/Hermes path exposes per-node
`exact_get_layout` / `exact_get_absolute_layout` calls through Swift and JSI.
Optimizing one does not prove the other is fixed.

## Motivation

The current kernel is clean and simple for Phase 0:

- `Kernel.nodes` is a `HashMap<ViewId, Node>`.
- `Node` is a wide object: id, type, props map, full `StyleProps`, Taffy node,
  children vector, parent, event-handler map, and transitions map.
- Taffy owns layout computation and exposes computed geometry through
  `taffy.layout(node.taffy_node)`.
- Several hot-path operations walk the render tree and repeatedly read only a
  tiny subset of each `Node`.

That shape is good for feature velocity but not obviously ideal for the target
performance envelope. A 1K-node frame operation often wants just:

- `ViewId`
- parent and children
- `taffy::NodeId`
- `x/y/width/height`
- transform translate
- overflow
- z-index
- "has press handler"
- a handful of boolean semantics/selection flags

Today those reads usually go through `HashMap` lookups and a wide `Node` cache
line that also carries cold maps and full style state. This is exactly where a
columnar sidecar can win: not by making layout math faster than Taffy, but by
making the frame-adjacent scans cheaper and less allocation-heavy.

One honest caveat sets expectations for the phases below: as long as the tree is
traversed in `ViewId` space (children stored as `ViewId`), each hop still costs
one `view_to_index` hash lookup. Phase B/C therefore shed the wide-`Node` load,
the `taffy.layout` chase, and the secondary string/handler/z lookups, but the
per-hop `ViewId -> index` hash only disappears when traversal moves to
generation-checked `NodeIndex` child tables, which is Phase D. The locality
thesis is realized in stages, not all at once in Phase B.

The first decision is therefore not "add SoA." It is "prove which frame-adjacent
reads are material." A path must show up in a benchmark or host trace before it
earns production sidecar state.

## Current Hot Paths

This review found real potential in four areas, with one caveat: every area
needs a baseline that reports both local speedup and total frame share.

### 1. Layout Reads and Export

`Kernel::get_layout_tree` in `kernel/src/layout.rs` does a DFS from a root,
fetches each `Node`, fetches Taffy layout for the node, accumulates absolute
coordinates, and returns `Vec<(ViewId, LayoutResult)>`.

This path is production-relevant for the Windows host. In the June 2026 shape
this section benchmarked, `Scene::layout` computed layout, called
`get_layout_tree`, then joined each layout result back to `Node` state while
building `RenderNode`s — a join heavier than the call itself.
`get_layout_tree` already did one `get_node` plus one `taffy.layout` per node
internally, and `Scene::layout` then re-fetched each `Node` twice more: once in
the rendered-text-ancestor filter and once in the main build loop, to read
`node_type`, `parent`, and render props (plus ancestor walks). So the Windows
render path performed roughly three wide-`Node` hash lookups per node per
frame.
A packed export carrying geometry + `node_type` + `parent` would remove the
*structural* re-fetches; sparse string props stay on `Node` by design (see
Non-Candidates), so they remain a join. *(As-built 2026-07: exactly this
export landed as `LayoutTreeNode` / `get_layout_tree_nodes` and the Windows
host now consumes it — see "As-Built and Evidence Status (2026-07-12)".)*

The Apple/Hermes path is different. `ExactKernel.swift` and
`hermes_runtime_ios.cc` expose per-node `exact_get_layout`,
`exact_get_absolute_layout`, and `exact_get_layout_generation` calls. A packed
layout-export sidecar might help later, but only after we measure whether
per-node layout reads are a hot Apple/Hermes path or mostly an agent/debug API.

These paths are candidates for columnar storage because they read:

- children
- parent/portal metadata
- Taffy node id
- computed geometry

They do not need props or most style fields for the common non-portal case. The
current implementation already fixed an O(n * depth) absolute-layout pattern
under LLP 0159, so the next likely win is memory locality, fewer hash-table
touches, and eventually a packed export shape for hosts that need whole-subtree
frames.

### 2. Hit Testing

`Kernel::hit_test` in `kernel/src/lib.rs` walks roots in paint order, reads a
node, checks `inert`, reads layout, applies transform translation, checks
overflow clipping, checks whether the node has a press handler, and sorts/
pushes children by z-index.

This path has a clear hot subset:

- children
- Taffy node id / cached layout rect
- transform translate x/y
- overflow
- z-index
- inert flag
- press-handler presence
- paint-order child order

Today inert and handler presence come from maps, and `inert` specifically is a
string-keyed `props.get("inert")` lookup (a string hash) on every visited
node. For per-pointer hit testing, especially drag/hover paths or agent
hit-test probes, this is the cleanest place to test sidecar flags. *(As-built
2026-07: no pointer-dispatch path — drag/hover included — reaches
`Kernel::hit_test`; input hit-testing is presenter-side on every host under
LLP 0297. The surface's live consumers are agent probes plus the renderer's
keyboard-avoidance target resolution, which probes it about once per commit
while a focused editable input and a visible non-interactive software
keyboard coexist (eligible-commit cadence). See "As-Built and Evidence
Status (2026-07-12)", which demotes this sidecar to a bounded
commit-cadence tier.)* The
implementation also clones and sorts child lists when any sibling has a
non-zero z-index; to *decide* whether to sort it does a `nodes.get(child)`
HashMap lookup for every child on every hit test, even in the common
all-zero-z case. A precomputed paint-order cache therefore matters more than a
raw `z_index` column for overlapping or animated scenes: it removes those
per-child lookups outright, not just the clone-and-sort.

Which single per-node cost dominates `hit_test` is exactly what Phase A must
settle; the RFC should not pre-judge it. Each visited node unconditionally
pays a wide `nodes.get` and a string-keyed `props.get("inert")` hash;
non-inert nodes then pay the `self.taffy.layout(node.taffy_node)` slotmap
lookup (inert exits first); in-bounds nodes additionally pay an
`event_handlers.contains_key` probe; and the paint-order step pays one
`nodes.get` per child in the all-zero-z probe, while the non-zero-z path
re-invokes that per-child lookup inside the `sort_by_key` comparator (it is
not `sort_by_cached_key`) on top of the clone. The string hash alone is
plausibly comparable to the slotmap index. The leading hypothesis
is that the cached-rect win (removing the `taffy.layout` indirection) and the
paint-order cache (removing the per-child z lookups) together dominate. A cached
layout rect (`x/y/width/height`) populated after each `compute_layout` pass is
therefore a priority Phase C candidate - "priority" meaning measured first, not
assumed proven. Storing it alongside the transform/overflow/press flags and the
paint-order cache lets the hit-test walk shed the Taffy indirection, the
per-child z lookups, and the wide `Node` cache line in one pass. *(2026-07:
Phase A settled the dominance question — paint-order confirmed — and the
"priority" here has since been re-tiered: the cached rect rides the
commit-cadence tier's gates now; see the as-built section and Q3's note.)*

### 3. Style Mutation and Animation Fields

`set_style`, `set_style_size`, `set_transform_components`, `set_opacity`, and
`set_background_color` update a full `StyleProps` stored inside `Node`.
`StyleProps` is intentionally broad: layout, paint, text, grid, masks, font,
image, gradient, RTL, and protocol set-ness.

The existing code already separates some fast paths:

- `layout_fields_eq` avoids Taffy updates for paint-only changes.
- transform, opacity, and background updates avoid rebuilding Taffy style.
- batch FFI helpers exist for benchmark workloads.

The potential next step is not to split all style state. It is to move
frame-mutated render fields into a small `RenderColumns` sidecar:

- transform x/y/scale/rotate
- opacity
- background/tint maybe
- z-index
- overflow
- style generation / dirty flags

That would let animation and hit-test paths avoid touching the full
`StyleProps` object, while keeping `StyleProps` as the canonical protocol and
Taffy conversion source until measurements show a larger split is warranted.
The sidecar must have an explicit ownership rule: either it is derived and
checked from `Node.styles`, or it becomes the canonical owner for that render
field and `Node.styles` reads through it. A permanent "write both and hope"
model is not acceptable.

`RenderColumns` should be staged, not landed whole. The first promoted
sidecar is the minimal animation + hit-test surface: transform translate
`x/y`, `opacity`, cached layout rect, `has_press_handler`, `inert`, and
`overflow`/paint-order for hit testing. *(2026-06/07: Phase A finding 3
dropped the animation fields — transform/opacity mutation is already one
`get_mut` + field writes — so the stage-1 surface, if promoted at all, is
the hit-test/export subset; the staging and ownership rules here still
govern whatever field set is promoted. See "Phase A Results" and the
as-built section.)* `transform_scale`, `transform_rotate`,
and a raw `z_index` column ride along only after the ownership and
invariant-test machinery is proven on that minimal surface. Translate + opacity
dominate real animation hot paths and scale/rotate change comparatively rarely;
z-index is different: it is *read* on every hit test (once per child), so it is
read-hot even though it is written rarely. What lets a separate raw `z_index`
column wait is that the stage-1 paint-order cache already absorbs those per-child
reads; the column earns its place only when a non-hit-test consumer needs it. The kernel already ships one narrow derived sidecar that proves the
pattern is low-risk: `svg_node_ids` (`HashSet<ViewId>` of live Svg node ids,
maintained incrementally on `create_view`/`destroy_view` under LLP 0159) so
layout passes skip the SVG scan when no Svg nodes are present. *(As-built
2026-07: renamed `svg_raster_node_ids` — commit `4305f68b8` — now covering
Svg and qualifying Image nodes and maintained on prop mutations as well as
create/destroy.)* Phase B is the
generalization of that existing, working pattern.

### 4. Semantics and Selection Snapshot Builds

`semantics::build_snapshot` and `SelectionDocument::build` both walk the tree
and derive flattened outputs. LLP 0159 already moved them away from recursive
walks and added memoization, but both still pull full nodes and maps repeatedly.

These paths are lower-frequency than layout export or hit testing, but they are
agent/API-visible and can be expensive on text-heavy or accessibility-rich
trees. A sidecar can help with coarse pruning:

- node type
- children
- inert/accessibility-hidden flags
- native id presence
- text node presence
- interactive presence
- selection mode

This should be treated as a second-order candidate after layout/hit-test data
proves the pattern.

## Non-Candidates

Some code should not be moved to SoA yet:

- **Taffy internals.** We use Taffy as the layout engine. Rewriting or
  mirroring its internal layout algorithm is not justified by this evaluation.
- **Props storage.** Props are string-keyed, sparse, and semantically rich.
  For now they remain cold state on `Node`.
- **Full `StyleProps` split.** A deep style storage rewrite risks correctness
  churn. Start with render/interaction columns only.
- **SVG rasterization and text shaping.** These are dominated by parsing,
  native text measurement, cache keys, and image work. Columnar node storage is
  unlikely to be the main lever.
- **Odin migration.** Odin's built-in SoA support would improve ergonomics for
  a fresh kernel, but it would force new FFI/build/tooling risk and likely keep
  Taffy behind FFI or require a rewrite. The performance thesis can be tested
  directly in Rust first.
- **Per-node `Vec` children as the final columnar shape.** `Vec<Vec<ViewId>>`
  is useful as an illustrative scaffold, but it keeps one heap allocation per
  parent. A real locality-oriented tree column should prefer a flat child table
  plus per-node ranges unless benchmarks show child mutation cost dominates.
  Note that the flat child table still stores `ViewId` in Phase B (safe under
  sparse, reused IDs); migrating it to generation-checked `NodeIndex` entries -
  the shape that actually removes the per-hop hash - is Phase D work, not
  Phase B.

## Design

### Phase A: Add Benchmarks Before Storage Changes

Extend `kernel/benches/kernel_benchmarks.rs` with tree-shape and frame-adjacent
benchmarks that isolate the suspected wins. Before any sidecar is implemented,
record a baseline report with:

- local path time;
- path share of a simulated frame;
- allocation count where practical;
- approximate bytes per live node;
- whether the path is used by Windows, Apple/Hermes, agent/debug only, or a
  benchmark-only helper.

Initial benchmark set:

1. `layout_read_paths`
   - 1K, 5K, 10K nodes.
   - vertical list, nested feed cells, shallow grid, and mixed portal-free tree.
   - measures `compute_layout` separately from Windows-style
     `get_layout_tree`.
   - measures repeated Apple/Hermes-style `exact_get_layout` and
     `exact_get_absolute_layout` calls.
   - records whether a packed subtree export would remove meaningful host
     call overhead.

2. `hit_test`
   - dense overlapping siblings.
   - deep nested list rows.
   - visible vs hidden overflow.
   - 1K repeated pointer probes against a stable tree.

3. `frame_mutation`
   - update transform/opacity for 20, 200, and 1K nodes.
   - check mutation cost separately from layout recomputation.

4. `semantics_selection_snapshot`
   - text-heavy tree.
   - accessibility-rich tree.
   - selectable document tree.

5. `memory_and_allocations`
   - approximate bytes per live node.
   - allocation count during snapshot/export paths where practical.
   - sidecar capacity overhead for sparse protocol-created view IDs.

The benchmark output remains informational under the existing `perf` profile,
but the RFC requires before/after artifacts for any storage promotion.
Allocation counts and approximate bytes-per-live-node are not available from
Criterion alone; Phase A should wire a counting global allocator (e.g. `dhat`
or a trivial `GlobalAlloc` counter) so the baseline artifact is reproducible
rather than aspirational.

### Phase B: Introduce Narrow Sidecars

Add sidecar structs behind the existing `Kernel` API:

```rust
struct TreeColumns {
    ids: Vec<ViewId>,
    parents: Vec<Option<ViewId>>,
    child_ranges: Vec<std::ops::Range<u32>>,
    child_ids: Vec<ViewId>,
    taffy_nodes: Vec<taffy::NodeId>,
    node_types: Vec<NodeType>,
    view_to_index: HashMap<ViewId, NodeIndex>,
}

struct RenderColumns {
    // Cached layout rect, populated for each node visited by a successful
    // `compute_layout(root, ...)` pass. This mirrors Taffy's last-computed
    // geometry per node; it is not validated by one global generation equality.
    layout_x: Vec<f32>,
    layout_y: Vec<f32>,
    layout_width: Vec<f32>,
    layout_height: Vec<f32>,
    layout_rect_valid: Vec<bool>,
    layout_rect_epoch: Vec<u32>,
    // First-stage animation fields (translate + opacity dominate real
    // animation hot paths). Phase A finding 3 later dropped these from
    // stage 1; kept in the sketch for shape only.
    transform_x: Vec<f32>,
    transform_y: Vec<f32>,
    opacity: Vec<f32>,
    // Hit-test flags.
    overflow: Vec<style::Overflow>,
    has_press_handler: Vec<bool>,
    inert: Vec<bool>,
    paint_order_ranges: Vec<std::ops::Range<u32>>,
    paint_order_child_ids: Vec<ViewId>,
    // Second-stage fields, added once the ownership model is proven on the
    // minimal set above.
    transform_scale: Vec<f32>,
    transform_rotate: Vec<f32>,
    z_index: Vec<i32>,
}
```

This is illustrative, not a required exact shape. The implementation may use
slotmaps, dense indexes, bitsets, or typed columns if benchmarks show one shape
is better.

Rules:

- `Node` remains the canonical rich record during Phase B.
- Sidecars are maintained incrementally by existing mutation methods,
  including `reset()` - every sidecar must be cleared alongside `nodes`,
  `roots`, `view_roots`, `svg_node_ids` (today `svg_raster_node_ids`), and
  the generation counters in `Kernel::reset`, or benchmark stability and
  test invariants break silently.
- Any cached computed-geometry column (the hit-test layout rect) must be
  per-node valid, not inferred from `layout_generation == current_generation`.
  Exact supports multiple roots and `compute_layout(root, ...)` updates only
  that root's Taffy subtree. A single global generation equality would either
  invalidate still-correct rects for previously laid-out roots or incorrectly
  bless nodes never visited by the latest root pass. Store a per-node validity
  bit or epoch, populate it only for nodes visited by the successful layout
  pass, clear it on `reset()` and node destruction, and fall back to
  `taffy.layout` for unstamped nodes. Mutations without relayout keep the
  previous rect valid, matching today's semantics where `taffy.layout` returns
  last-computed geometry.
- Any sidecar read path must have an invariant test against the existing
  `Node` path.
- Sidecars must tolerate sparse `ViewId`s; protocol-created IDs are not always
  dense.
- Any sidecar must declare whether it is canonical or derived for each field.
- Derived sidecars need debug assertions or invariant tests that compare them
  to `Node` after create, destroy, style, prop, event, and children mutations
  (ideally property-based mutation sequences — create, prop changes,
  reparent, destroy, `reset()`, multi-root layout — continuously compared
  against canonical `Node` state).
- Canonical sidecars need a compatibility read path so existing APIs that
  expose `Node.styles` keep returning the same values.
- The first promoted sidecar must be memory-neutral or document why its memory
  cost is justified by frame-time wins.
- No changes to existing FFI signatures in Phase B. A packed layout-export
  ABI for Apple/Hermes (Phase C item 4) is an additive new entry point, gated
  separately, and does not modify `exact_get_layout` /
  `exact_get_absolute_layout` / `exact_get_layout_generation`.

### Phase C: Route One Hot Path at a Time

Promotion order, assuming the Phase A baseline shows the path is material
*(order revised 2026-07-12 — LLP 0297 plus the corrected caller inventory
demoted item 1 to a bounded commit-cadence tier, and item 2's structural
half landed; see "Revised Phase C promotion order" in the as-built section,
which supersedes the ordering below)*:

1. `hit_test` uses sidecar fields for inert, transform, overflow, press
   presence, cached layout rect, and cached paint order. The cached rect is
   the priority field: it removes the per-node `taffy.layout()` call from
   the walk.
2. Windows-style `get_layout_tree` uses tree columns for traversal and Taffy
   node lookup.
3. frame mutation writes render columns first and mirrors to `Node.styles`
   only after the ownership rule for render fields is explicit.
4. Apple/Hermes layout reads get either a packed export API or a faster
   per-node lookup path, depending on the baseline. A packed subtree export
   is the preferred end state: it is the natural unification point that lets
   both native hosts stop maintaining divergent read surfaces, with per-node
   `exact_get_layout` retained only for agent/debug paths. It lands as an
   additive new FFI entry point, gated separately from the Phase B sidecars.
5. semantics/selection use sidecar flags only for pruning; rich derivation
   remains on `Node`.

Each step lands with an A/B benchmark note in the PR description and keeps the
old implementation available under test-only comparison until confidence is
high.

### Phase D: Consider a Denser Arena

Only after Phase C demonstrates clear wins should we consider replacing
`HashMap<ViewId, Node>` as the primary storage with:

- `HashMap<ViewId, NodeIndex>` for lookup;
- dense node arrays for hot/cold records;
- free-list reuse for destroyed views;
- generation counters to avoid stale indexes.

This is a larger correctness surface and should be a follow-on implementation
plan, not the first patch.

## Phase A Results (2026-06-24)

Phase A was executed on a throwaway worktree branch (`llp-0258-phase-a`):
`kernel/benches/phase_a.rs` (criterion) isolates the four candidate paths against
`compute_layout` as the in-kernel reference cost, and `kernel/examples/memprobe.rs`
wires a counting global allocator for bytes-per-node + export allocation counts.
Numbers are release/`bench`-profile medians on a 2-core CI-class VM, so magnitudes
are **directional** (layout variance ran ±30%, hit-test ±10–15%); the orderings and
large ratios are robust to that band. `[observed]`

Materiality gate (RFC Acceptance Criteria): ≥5% of a realistic frame, or ≥0.25 ms
per frame/probe batch.

| Candidate | Measured | Material? |
|---|---|---|
| **Layout export** (`get_layout_tree`) | **17–50%** of the in-kernel frame (compute+export); 0.24 ms (feed ~1K) → 13.5 ms (list 10K) | **Yes — strongly** |
| **Hit testing** (`hit_test`) | 56 µs/probe (1K tree); **3.56× → 200 µs/probe** with non-zero-z siblings | **Yes** |
| **Frame mutation** (transform/opacity) | 1 µs (20) / 8 µs (200) / 46 µs (1000) = **0.01–0.28%** of a 16.7 ms frame | **No** |
| **Semantics** `build_snapshot` | 1.39 ms / 1K (≈ `compute_layout` on the same tree) but low-frequency | Borderline / 2nd-order |
| **Selection** `SelectionDocument::build` | 0.14 ms / 1K | **No** (below gate) |
| **Memory** | 6.5 KB/node @1K → 3.1 KB/node @10K; `get_layout_tree` = 30–41 allocs total | informs Phase D |

Findings `[observed]` unless noted:

1. **Layout export is the lead candidate.** It is a large fraction of the in-kernel
   frame on every fixture — on a flat list it rivals Taffy `compute_layout` itself.
   This figure *understates* the real Windows win: it benches `get_layout_tree`
   alone, not the additional ~3× wide-`Node` re-fetches `Scene::layout` performs on
   top (the structural join the Motivation section describes). The Apple/Hermes
   per-node read path costs about the same as the whole-tree export, so a packed
   export helps both hosts.
2. **Hit testing clears the gate, and the paint-order hypothesis is confirmed.** The
   non-zero-z fixture is **3.56× slower** than all-zero-z, isolating the per-child
   `nodes.get()` z-probe + clone-and-sort as the dominant cost. `[inferred: this
   confirms the RFC's claim that a precomputed paint-order cache — not a raw
   `z_index` column — is the win; the cached layout rect rides along in the same
   walk by removing the per-node `taffy.layout()` indirection.]`
3. **Frame mutation is NOT material — the animation-columns rationale is unsupported.**
   `set_transform_components`/`set_opacity` are already one `nodes.get_mut` + field
   writes (no `StyleProps` rebuild, no Taffy). A `RenderColumns`-for-animation sidecar
   would only swap the hash lookup for an indexed write while adding maintenance tax
   to every create/destroy/`set_children` — failing the write-path non-regression
   rule. `[inferred: drop the animation columns from Phase B; revisit only if a real
   animation workload, not synthetic bulk updates, shows otherwise.]`
4. **Semantics/selection stay second-order.** `build_snapshot` is material in absolute
   terms but low-frequency, and its three-pass string/relationship construction —
   not node lookups — is the likely cost, so a pruning sidecar's upside is uncertain.
5. **Export is not allocation-heavy** (30–41 allocs for 1K–10K results): its cost is
   per-node hash lookups + the `taffy.layout` chase + the wide-`Node` cache line, not
   allocation. Memory is ~3 KB/live node at scale, which justifies a hot/cold split
   for *locality* (bytes touched per scan), the Phase D lever.

**Honest scope limit:** the reference cost is `compute_layout`; native-host paint,
text shaping, and protocol I/O were not measured, so "share of in-kernel frame" is not
"share of device frame." If native host work dominates a real frame (the Taffy-
dominance risk generalized), even a material in-kernel path may not move total frame
time. The export/hit-test wins are real kernel-side; their device-frame impact needs
the Phase C host harness.

**Recommendation:** do **not** shelve, but **tighten Phase B scope to the two paths
that cleared the gate** — layout export (lead) and hit testing (paint-order cache as
the first field). Drop `RenderColumns`-for-animation. Keep semantics/selection
second-order; keep the dense arena as a Phase D follow-on. Confidence: **high** that
export + hit-test are material and animation columns are not; **moderate** on
device-frame magnitude pending the host harness. Full data and per-fixture tables
were recorded in `PHASE_A_REPORT.md` on the `llp-0258-phase-a` branch.
*(2026-07-12: that throwaway branch was deleted and its objects pruned —
`git log --all`, the reflog, and `git fsck --unreachable` hold no trace of
the bench sources or the report. The table above is the surviving record;
see "As-Built and Evidence Status (2026-07-12)" for the disposition.)*

## As-Built and Evidence Status (2026-07-12)

This section reconciles the RFC with the repository and with LLP 0297/0322,
which landed after the June revision, and records the disposition for the
lost Phase A evidence. Nothing here reopens the June design; it re-grounds
it.

### What landed from Phase A's recommendation

The lead finding (layout export) produced a shipped follow-on — without any
columnar storage:

- `LayoutTreeNode` and `Kernel::get_layout_tree_nodes` — a packed
  *structural* export row (`id`, `parent`, `node_type`, absolute rect) per
  visited node (`kernel/src/layout.rs:22`, `:51`), attributed to this RFC's
  Phase A data in its doc comment.
- The Windows host consumes it: `Scene::layout` builds `RenderNode`s from
  `get_layout_tree_nodes` and derives its parent map, ancestor walks, and
  node types from the packed rows
  (`packages/exact-host-windows/src/scene.rs:369`).

Scope honesty about what that bought. The packed rows removed the
*structural* wide-`Node` reads and the per-ancestor `Node` walks (the
rendered-text-ancestor filter now walks a `ViewId -> parent` map built from
the export). Host-side node-table lookups remain: two wide-`Node` fetches
per exported row — the rendered-text probe and the render-prop join (sparse
string props stay on `Node` by design; see Non-Candidates) — plus a
`get_event_handler(id, Press)` probe per *emitted* row
(`packages/exact-host-windows/src/scene.rs:487`, another `nodes.get`) whose
result feeds the presenter-side hit testing, and composed-text run
collection for Text nodes. Inside the kernel, the export pays one
`get_node` + one `taffy.layout` per visited node (`kernel/src/layout.rs:85`)
*and* a `portal_target_parent` probe per node (`kernel/src/layout.rs:94` →
`kernel/src/lib.rs:1506`) — a second `nodes.get` plus a string-keyed
`props.get("portalTarget")` hash — with root initialization performing its
own lookup/layout pair plus `get_absolute_layout`'s ancestor walk (each hop
pays `get_layout` — a node lookup and Taffy read — plus `visual_parent_id`'s
node lookup and portal probe; the lookup ledger below enumerates these
exactly). That inventory is the
surface Phase B tree columns target; it is also why the rebuilt baseline
must carry a no-sidecar control arm (see the promotion-entry
preconditions): part of this cost is recoverable by reusing an existing
node borrow, checking `node_type` before the rendered-text fetch, or
folding handler presence into the export row, without any dual state. The
tuple `get_layout_tree` remains; its callers today are kernel tests.

So: Phase A is complete, and the first production win landed as **export
API shape rather than sidecar state**. No Phase B sidecar exists in-tree.
The landing (`65bdc1973`, 2026-06-24) carried no before/after benchmark
artifact and did not claim the Windows acceptance target; as an additive
API-shape change it did not trip the sidecar promotion gate, which still
applies in full to any future columnar promotion.

### LLP 0297 resolved two open questions

**Q4 (how often do hosts call kernel hit testing) — answered: never for
pointer dispatch; one bounded commit-cadence production caller remains.**
Pointer input does not reach `Kernel::hit_test` on any host. The Windows
host hit-tests its own `RenderNode` list, presenter-local
(`packages/exact-host-windows/src/scene.rs:511`); Apple gesture hit-testing
runs against the main-side presenter snapshot by design, as does
`measure(nodeId)` (LLP 0297); web input is DOM-native. `Kernel::hit_test`
has two live caller classes, both funneling through the inspector's
`findElementByPoint` (`packages/exact-renderer/src/inspector.ts:2933` → the
native `exact.hitTest` JSI binding):

- **Agent probes** (`exact_hit_test` / `POST /agent/hit-test`), on demand.
- **Keyboard avoidance, in production.** `syncKeyboardAvoidanceAfterCommit`
  runs in the host-ops commit path, and while a focused editable input and
  a visible, non-interactive software keyboard coexist, its target
  resolution (`packages/exact-renderer/src/keyboard-avoidance.ts` →
  `resolveTargetDetails` → `buildHitTestBlockers` → `findElementByPoint`)
  probes the kernel about once per *eligible* commit on the focused root —
  typically typing cadence, though any commit while the focus/keyboard
  guards hold triggers it; never pointer or frame cadence, and nothing
  outside those guards. (The focused root is the resolution context; the
  kernel walk itself spans all roots — `hit_test` takes no root filter.)

In both cases the kernel answer is advisory: the inspector accepts it only
when the kernel-returned node's scroll-adjusted frame contains the probe
point, and otherwise falls back to a TS frame walk — with one exception, a
kernel id that fails to resolve to an element (unknown id, root, or
non-element node) short-circuits to no-hit — because the
kernel speaks root-local content coordinates without tracked scroll offsets
(ENG-22790). The Swift `ExactKernel.hitTest` wrapper
(`ios/ExactApp/ExactApp/Kernel/ExactKernel.swift:747`) currently has no
callers.

Consequence: the hit-test sidecar (Phase C item 1, previously co-lead) is
**demoted from frame-critical co-lead to a bounded commit-cadence tier**.
Its surviving Phase A figures remain recorded (56 µs/probe; 3.56× under
non-zero z), but its live workloads are one advisory probe per eligible
commit during
keyboard-avoidance sessions plus on-demand agent probes — neither is a
pointer- or frame-budget problem. A promotion here must clear the
re-denominated materiality gate on those workloads (see the Acceptance
Criteria addendum), which is why the promotion-entry preconditions add a
keyboard-avoidance fixture to the rebuilt harness. Two cheaper alternatives
must be priced first: giving keyboard avoidance a
geometry-and-scroll-chain-only resolver (returning the focused frame, the
scroll chain, and the nearest scroll container's frame — the three things
the caller actually reads) — the production caller currently pays
`resolveTargetDetails`' full-root serialization (rebuilt in full by
default; incremental serialization exists behind
`EXACT_AGENT_INCREMENTAL_TREE`, default-off) and occlusion/blocker pass
per eligible commit (`packages/exact-renderer/src/inspector.ts:4605`), so
skipping that work entirely is likely to dominate any kernel-side win, and
it restores an agent-only call graph — and the no-sidecar cleanup control
arm. Any
investment here should also coordinate with LLP 0297 OQ8 — during
Motion-driven transforms, agent hit-tests see model geometry while the
screen shows presented geometry — since fixing *which geometry* this path
resolves against likely matters more than resolving it faster. (OQ8's
divergence already applies to the production caller today: keyboard
avoidance resolves model-geometry frames, so a Motion-driven transform on a
focused input can auto-scroll toward pre-transform positions.) If the
geometry-only resolver lands and removes the production caller, this
surface reverts to agent-only and drops to second-order alongside
semantics/selection — pre-committed here so the tier is not re-litigated.

**Q7 (Windows convergence on a packed export) — partially resolved.**
Windows now consumes the packed structural export. Whether both native
hosts converge on one packed *ABI* (Q2's buffer end state, with
Apple/Hermes migrating off per-node reads) remains open and is the next
material question for the export path.

One ranking is retained deliberately un-reopened: semantics/selection
snapshots (Current Hot Paths §4) stay second-order. Unlike hit testing,
their call graph did not change under LLP 0297 — `build_snapshot` still
runs in the tree domain and its consumers (the per-root accessibility
mirror and the agent tree) survive intact — so the June ranking carries
forward and is re-tested by the rebuilt baseline like everything else.

**Threading (a question this RFC predates) — resolved favorably: sidecars
need no double-buffering.** Under LLP 0297 (runtime thread default-on for
macOS since LLP 0322; opt-in on iOS/Windows pending their gates) the kernel
tree/layout domain is single-domain: app JS, the reconciler, kernel
mutation, Taffy, layout export, and the agent's kernel reads all execute in
the tree domain, and Swift-side kernel accessors assert that scope
(`ExactRuntimeAffinityAudit.assertTreeDomainScope`). Main-side geometry
consumers were severed from kernel storage by design — gesture hit-testing
and `measure()` read the presenter snapshot, and the Motion geometry mirror
is fed by the host data-plane feed, not the kernel. With the runtime thread
off, everything runs on main — still one domain. Either way, **tree-domain
kernel storage** — the node table, Taffy state, and the layout/hit-test
export surfaces this RFC covers — has no cross-thread readers, so Phase B/C
sidecars are plain single-domain structures: no double-buffering, locks, or
atomics. (The SharedValue slab is also kernel-owned memory but is
deliberately cross-thread under LLP 0297 §4.5 — per-slot atomics for
single-word values, 0297's seqlock protocol for multi-word records; it is a
different domain and outside this RFC's storage scope.) The
inverse rule also binds: no sidecar may become a new cross-thread read
surface; main-side consumers go through LLP 0297's snapshot/feed machinery.

### Phase A evidence: lost, and the disposition

The Phase A instruments and full data — `kernel/benches/phase_a.rs`,
`kernel/examples/memprobe.rs`, and `PHASE_A_REPORT.md` — existed only on
the throwaway branch `llp-0258-phase-a`, which has been deleted. As of
2026-07-12 no trace survives in `git log --all`, the reflog, or
`git fsck --unreachable`. The results table above is the only surviving
record. It remains adequate for what it was used for — directional
materiality and the Phase B scope cut (export + hit-test cleared the gate;
animation columns dropped) — and the hit-test demotion above now rests on
call-graph facts rather than those numbers. It is **not** usable as a
promotion baseline: the medians came from a 2-core CI-class VM that no
longer exists, and the acceptance gates were always before/after on the
promotion host.

**Decision (2026-07-12): do not rebuild the harness speculatively;
re-measure at promotion-work entry, in-tree.** Rebuilding now would produce
numbers on yet another host with no scheduled consumer — promotion
baselines must be measured where promotion is claimed, so pre-building buys
reproducibility theater, not decision-relevant information. Instead, the
loss converts into binding **promotion-entry preconditions** (they gate the
first post-Phase-A promotion, whichever phase it belongs to — the revised
order below puts packed-ABI work, a Phase C item, first):

1. **Rebuild the Phase A harness in-tree first, on the as-built surfaces.**
   Re-create the benchmark set specified under "Phase A: Add Benchmarks
   Before Storage Changes" (including the counting-allocator probe),
   updated to what production ships: `layout_read_paths` measures
   `get_layout_tree_nodes` (the production Windows surface) and the current
   `Scene::layout` join/frame simulation, plus the current Apple/Hermes
   per-node reads; the tuple `get_layout_tree` rides along only as an
   invariant check or control. Land it by extending
   `kernel/benches/kernel_benchmarks.rs` or adding a committed sibling
   bench target, surfaced through the existing `kernel-bench` entry in
   `exact-verify.json`'s `perf` profile (no unregistered root checks, per
   LLP 0150/0151; that entry's command enumerates bench targets explicitly,
   so a new sibling target must be added to it). Two additions to the June
   set: a keyboard-avoidance fixture (two instruments, because the
   keyboard-arm gate's denominator includes TS-side work a Cargo bench
   cannot execute: a kernel-side fixture — one probe per eligible commit
   against a mutating text-editing tree, including unrelated high-rate
   commits while the keyboard stays visible, matching the production
   guards recorded above, plus a multi-root variant, since
   `Kernel::hit_test` walks every root — **and** a registered
   renderer/live-runtime benchmark that drives eligible commits with a
   focused editable input and a visible non-interactive keyboard through
   the real TS resolution path against a real kernel binding, recording
   total pipeline time and kernel-probe time on the identical fixture;
   today's `js/src/benchmark/renderer-commit.ts` installs only a dispatch
   capture, so this is new registered work, and its before/control/
   candidate arms follow the same pairing rules as precondition 2), and —
   required before the first
   promotion, landable ahead of the rest of the harness since it is
   host-independent — a debug "lookup ledger" that counts node-map,
   prop-map, Taffy, event-handler, and child-z probes per operation and
   emits a machine-readable receipt — a versioned per-fixture cost
   signature (portal-free vs portal trees, zero vs non-zero-z siblings,
   export joins) with operation-specific assertions, so an unrelated
   refactor does not rewrite whole totals — keeping this document's cost
   inventory mechanically falsifiable without timing noise (it also works
   as a deterministic test invariant: asserted probe counts per operation
   on fixed fixtures).
2. **Commit a fingerprinted baseline; pair every promotion comparison.**
   Before any promotion-work change lands, record the rebuilt baseline
   in-tree alongside the benches (e.g. `kernel/benches/BASELINE-<host>.md`,
   listed in the registry entry's `artifacts`, with machine-readable
   Criterion output committed alongside the prose report), fingerprinted —
   commit, CPU model and core count, OS, power state, Rust toolchain,
   fixture seeds, host build configuration. Promotion reports additionally
   state the expected end-to-end gain explicitly — path share × local
   speedup — alongside the write tax and bytes-per-node, so a visually
   impressive local percentage cannot stand in for a real win. The
   committed baseline serves
   materiality confirmation and trend context. Promotion evidence itself
   must be **paired**: the before leg, the no-sidecar control arm, and the
   candidate run interleaved in one session on one machine — a stable named
   host, or a single job on an ephemeral runner (`exact-perf.yml`'s
   `macos-latest` is a host *class*, not a machine; cross-job or cross-week
   comparisons are not promotion evidence) — with arm ordering interleaved
   against thermal and order bias, ideally with all arms compiled into one
   benchmark binary so interleaving is mechanical. The chosen host must
   first demonstrate a
   noise floor compatible with the tightest gate it adjudicates (the 3%
   write-path budget): if identical-code runs cannot resolve 3%, use a
   quieter host or widen the benchmark per the Acceptance Criteria
   noise-band rule.
3. **Carry a no-sidecar control arm.** Part of the current cost is
   recoverable with no dual state at all — reusing an existing node borrow,
   checking `node_type` before the rendered-text fetch, and folding handler
   presence into the export row (the strongest-looking candidate: it
   deletes the per-emitted-row lookup and feeds `Scene::hit_test` directly,
   and the bit is read off the node already borrowed during the walk, so
   nothing persists). The baseline prices this cleanup arm first, and every
   columnar promotion is measured against the best no-sidecar control, not
   against the unmodified status quo. If the control arm alone clears a
   path's gate, the columnar promotion is declined, the cleanup lands as a
   plain optimization, and later candidates re-baseline against the
   improved status quo. A `portal_node_ids` presence set (skipping the
   portal probe for portal-free trees, on the `svg_raster_node_ids`
   precedent in §3) is deliberately **not** in this arm: a persistent
   presence set is a derived sidecar — it duplicates canonical
   `portalTarget` prop state and owes the full sidecar treatment
   (ownership declaration, invariant tests, `reset()` clearing, memory
   accounting, the write-path budget) — so if priced, it runs as its own
   gated candidate, never as ungated cleanup. (Folding the handler bit into
   `LayoutTreeNode` is a Rust-API change consumed directly by the Windows
   crate — not an FFI-signature change — so it does not collide with the
   Phase B no-new-FFI rule.) Because the Windows host
   crate is not a kernel-bench dependency, the control arm's
   `Scene::layout` join leg is simulated as kernel-API sequences in the
   bench (the two wide-`Node` fetches plus `get_event_handler` and
   `composed_text_paint_runs` against the fixture).
4. **Re-confirm materiality before building.** If the re-measured baseline
   contradicts the June ordering (for example, layout export no longer
   clearing the 5% / 0.25 ms gate now that the packed structural export has
   landed), the June scope cut is re-decided on the new data; the Shelved
   outcome in Acceptance Criteria stays available. Restate the materiality
   denominator for the LLP 0297 world at the same time: the kernel's
   cadence is commit-driven and hosts paint from presenter snapshots, so
   "share of a simulated frame" should be re-derived against the
   commit/feed pipeline the numbers are meant to predict (candidate
   denominators: per-commit tree-domain CPU on a typing fixture,
   feed-publish latency, p95 commit time on the canonical fixtures). The
   recorded denominator must state explicitly whether app-JS/TS-side
   commit work is inside it, and why — the tree domain includes app JS
   under LLP 0297, and a kernel-only denominator inflates the kernel
   share of any pipeline it is compared against.

**Evidence-retention rule (process, binding for this RFC's future phases):**
benchmark evidence that gates an LLP decision lands in-tree in the same PR
that records the decision — bench source committed, report artifact
committed, registry entry wired. A throwaway branch has already cost this
RFC its Phase A instruments once.

### Revised Phase C promotion order

1. Packed layout export as the cross-host end state (Q2): Windows landed
   the structural row; the next candidate is the packed *buffer* ABI and
   the Apple/Hermes migration off per-node reads, measured against the
   rebuilt baseline. Define a versioned packed-schema fixture (field
   widths, alignment, endianness, decode compatibility) before
   implementation, so cross-host convergence is testable independent of
   the host migrations.
2. Tree columns under `get_layout_tree_nodes` — removing the internal
   per-node wide-`Node` lookup; the `taffy.layout` chase goes away only if
   the Q3 cached-rect column joins the candidate, under its per-node
   validity, ownership, memory, and write-path rules — if the re-measured
   export share still justifies them.
3. Hit-test sidecar — commit-cadence tier: needs a measured
   keyboard-avoidance and/or agent-workload justification, priced against
   the geometry-only-resolver alternative, coordinated with LLP 0297 OQ8.
4. Semantics/selection pruning — unchanged, second-order.

Phase D (denser arena) is unchanged: strictly after Phase C wins are
demonstrated on the rebuilt baseline.

## Acceptance Criteria

Before a sidecar can be promoted, the baseline must show that the target path is
material: either at least **5%** of the simulated frame cost for a realistic
fixture, or at least **0.25 ms** per frame/probe batch on the benchmark host.
*(Denominators are re-derived at promotion entry per precondition 4; the
2026-07-12 addendum below binds all candidates and normalizes the hit-test
units.)*

After that materiality gate, this proposal is successful if the measured
Rust-side sidecars achieve at least one of the following without API breakage:

- the whole-subtree export is at least **20% faster** on 5K+ node trees after
  layout has already been computed — measured on `get_layout_tree_nodes`, the
  production Windows surface (the tuple `get_layout_tree` is test-only since
  2026-06; both share `visit_layout_tree`) — and the Windows host frame
  simulation improves by at least **5%**.
- repeated Apple/Hermes-style layout reads are at least **20% faster** or are
  replaced by a packed export path that reduces host-call overhead by at least
  **20%** on 1K+ nodes.
- `hit_test` is at least **25% faster** on repeated probes over 1K+ interactive
  trees, including a fixture with non-zero z-index siblings.
- transform/opacity frame mutation is at least **20% faster** for 200+ updated
  nodes. *(2026-07-12: not material per Phase A — animation columns were
  dropped from Phase B; this target is live only if the re-measured baseline
  reverses that finding.)*
- semantics or selection snapshot construction is at least **15% faster** on
  text/accessibility-heavy 1K+ trees.
- live kernel memory per node drops by at least **10%** after a dense-arena
  follow-on, without increasing common mutation time.

Every promotion is additionally gated on **not regressing the write path**.
Sidecar maintenance is pure overhead on `create_view`, `destroy_view`,
`set_children`, and the `set_*` style mutators; it helps only readers. No
sidecar may regress create/destroy/`set_children`/style-mutation throughput by
more than **3%** on the `frame_mutation` and `protocol_profile` benches (the
former recreated per the promotion-entry preconditions; today's in-tree
equivalents are the `transform_updates`/`set_styles`/`create_views` groups),
measured from stable Criterion medians or an equivalent repeated benchmark run.
If the noise band overlaps 3%, rerun or widen the benchmark before making the
claim. A read-path win that costs more on the write path than it saves on reads
is not a win.

*(2026-07-12)* These gates bind every promotion candidate this RFC now
orders — columnar sidecars and the packed export ABI alike; read "sidecar"
throughout this section as "promotion candidate". The original Phase A
baseline is unrecoverable (see "As-Built and Evidence Status"), so every
before/after claim above is measured as the paired, same-host comparison
the promotion-entry preconditions require — never against the June 2026
table — and against the no-sidecar control arm those preconditions define.
For the hit-test path specifically, the 5%-of-frame materiality gate no
longer applies as written (no pointer-dispatch path calls
`Kernel::hit_test`); a hit-test promotion instead requires materiality on
its live workloads measured on the rebuilt baseline, in normalized live
units:

- **Keyboard arm:** kernel hit-test work at or above **5% of the eligible
  commit's keyboard-avoidance pipeline cost** on the keyboard fixture. The
  denominator is the full per-commit resolution pipeline executing in the
  tree domain — the kernel probe *plus* the TS-side target-resolution work —
  and that is deliberate: while the TS pass dominates, a kernel-side sidecar
  is immaterial by construction and the geometry-only resolver is the
  correct spend. Precondition 4 records the final denominator and must state
  what is in and out of it, and why.
- **Agent arm:** kernel hit-test work at or above **0.25 ms per live
  request-equivalent (per probe)**, or at or above **5% of a representative
  end-to-end agent hit-test request**. The 1K-probe batch fixture is kept
  for measurement stability, but the decision unit is normalized per-request
  cost — the live API takes one target per request, so a raw batch threshold
  would be trivially satisfied (the June figures alone give
  56 µs × 1K ≈ 56 ms per batch) while proving nothing about request
  materiality. Pin the fixture's canonical selector shape and report
  invocation counts alongside per-probe and per-external-request latency: a
  coordinate-targeted request invokes the kernel probe twice (once resolving
  the coordinate to an element, once during blocker evaluation), while
  ref/testId targets probe once.

After the materiality gate, the 25% per-probe improvement target still
applies. The 3% write-path budget applies to a **candidate-specific
maintenance matrix**, not only the four June-named mutators: isolated
paired timings for every mutation path the candidate actually maintains —
create/destroy, representative `set_children` and reparenting, relevant
style updates, prop set/clear (portal and inert flags live there),
event-handler bind/unbind (handler-presence state), and layout-pass cache
population for cached-geometry columns — with an explicit N/A recorded for
each leg a candidate does not touch; only a candidate with no persistent
maintenance state may skip the matrix. The rebuilt harness must add the
missing legs: today's in-tree benches exercise `set_children` only as
setup, fold destruction into recycling, and `protocol_profile` has no
destroy, children, prop, or event case.

If Phase A benchmarks show none of these paths are material relative to
`compute_layout`, text measurement, protocol processing, or native host work,
the correct outcome is to close this RFC as Shelved with the benchmark data.
That is a successful negative result, not a failed implementation.
*(2026-07-12: read "Phase A benchmarks" here as the rebuilt promotion-entry
baseline — Phase A itself is complete and its raw evidence lost.)*

## Verification

- `cargo test -p exact-kernel`
- `cargo bench -p exact-kernel --bench kernel_benchmarks --bench protocol_profile -- --output-format bencher`
- `bun scripts/exact-verify.mjs --profile governance`
- For host-impact claims: a small Windows `Scene::layout` fixture or an
  Apple/Hermes layout-read harness, whichever path the sidecar claims to
  improve.

If new benchmarks become permanent checks, they must be represented through
`exact-verify.json`; do not add an unregistered root check.

## Risks

- **Dual state bugs.** Sidecars can drift from `Node`. This is the main risk.
  Keep them derived/incrementally updated through one mutation API and compare
  against the old path in tests.
- **`reset()` drift.** `Kernel::reset` rebuilds all kernel state for
  benchmark reuse. Any sidecar that is not cleared in `reset()` will leak
  stale columns into the next bench iteration and silently corrupt invariant
  tests. Treat `reset()` as a first-class mutation path, not an afterthought.
- **Wrong production surface.** The whole-tree export matters for Windows
  (as-built 2026-07: the packed `get_layout_tree_nodes` variant), but
  Apple/Hermes mostly reads layout through per-node FFI/JSI calls. Benchmark
  and optimize the surface being claimed.
- **Sparse ID waste.** Protocol view IDs may be sparse. Do not index columns
  directly by `ViewId` unless memory bounds are proven.
- **Flat child table mutation cost.** A flat child table improves scans but can
  make `set_children` more expensive. Measure both read-heavy and mutation-heavy
  workloads before promoting it.
- **Write-path tax.** Every sidecar adds maintenance cost to create, destroy,
  `set_children`, and style mutation while helping only readers. `svg_node_ids`
  (today `svg_raster_node_ids`, also maintained on prop mutations) is cheap
  (a `HashSet` insert/remove), but a flat child
  table forces range rewrites on `set_children`. Hold each promotion to the
  write-path non-regression budget in Acceptance Criteria.
- **Global-generation misuse.** `layout_generation` is a useful external
  staleness signal, but it is not enough to validate per-node cached rects in a
  multi-root kernel. Cached geometry needs per-node validity or epoch state, and
  host-facing generation semantics must remain unchanged.
- **Overfitting synthetic benches.** Tree shapes must include nested feed/list
  fixtures, not only flat vertical stacks.
- **Taffy dominance.** If `compute_layout` dominates frame time, export-path
  wins may not move total frame time enough. That is why benchmarks separate
  compute from export.
- **Complexity creep.** A full ECS-style rewrite is not warranted by the
  current evidence. Sidecars should remain narrow until the benchmark data says
  otherwise.

## Alternatives Considered

### Use Odin for the Kernel

Odin has first-class SoA types and data-oriented syntax. That would be pleasant
for this kind of storage. The downside is much larger than the current
performance question: new build integration, new FFI/debugging surface,
unclear Taffy story, and loss of Rust ecosystem leverage. Since the performance
hypothesis is testable in Rust, migrating languages is not justified.

### Do Nothing

Reasonable if Phase A shows frame cost is dominated elsewhere. Not reasonable
before measuring, because the current `HashMap<ViewId, Node>` plus wide `Node`
layout is visibly mismatched with several hot scans.

### Rewrite the Kernel Around ECS

An ECS-style kernel may be right eventually, especially for animation,
visibility, and hit testing. It is too broad for the current evidence. This RFC
proposes the smallest benchmarkable slice of that idea.

### Push Everything Into Taffy or Native Hosts

Taffy owns layout math; native hosts own pixels. Exact still needs kernel-side
layout export, hit testing, semantics, selection, protocol mutation, and agent
inspection. Those are valid kernel responsibilities and worth optimizing if
measured hot.

## Open Questions

1. What tree shapes should be blessed as the canonical kernel perf fixtures:
   Caltrain-scale list, Mini-Slack feed, text-lab document, or synthetic
   generated trees?
2. Should `get_layout_tree` return a packed FFI-friendly buffer eventually
   instead of `Vec<(ViewId, LayoutResult)>`? The proposal's preferred end state
   is a packed subtree-export ABI shared by both the Windows and Apple/Hermes
   hosts, with per-node reads kept only for agent/debug paths. *(2026-07-12:
   the live question is now framed on `get_layout_tree_nodes` / the packed
   structural export — the tuple API is test-only; see Q7's note and the
   revised Phase C order.)*
3. Should render columns store computed layout snapshots after each Taffy pass,
   or should they only store Taffy node IDs and query Taffy on demand? For the
   hit-test path this proposal leans yes: cache `x/y/width/height` after
   `compute_layout`, pending the Phase A measurement that the cached rect and
   paint-order cache are in fact the hit-test wins (the per-node `taffy.layout()`
   call is the leading hypothesis for the dominant cost, not an established
   fact). The cached rect must use per-node validity or epoch state; the global
   `layout_generation` counter is not sufficient by itself in a multi-root
   kernel. The question remains open for the layout-export path, where a packed
   export may make a separate cached-rect column redundant. *(2026-07-12: with
   hit testing demoted to the bounded commit-cadence tier, the cached-rect
   column's priority drops with it; the layout-export half of this question is
   the live one.)*
4. How often do hosts call kernel hit testing versus performing presenter-local
   hit testing? The answer affects priority. *(Answered 2026-07-12: never for
   pointer dispatch — every host hit-tests presenter-side under LLP 0297. The
   live callers are agent probes plus the renderer's keyboard-avoidance
   resolution, about once per eligible commit during focused-keyboard
   sessions. See "As-Built and Evidence Status (2026-07-12)".)*
5. Should semantics/selection sidecars be maintained incrementally, or only
   used as ephemeral build-time indexes?
6. What memory metric is the official per-node budget for the kernel?
7. Should Windows keep consuming `get_layout_tree`, or should all native hosts
   converge on one packed layout-export ABI? This proposal leans toward
   convergence as the preferred end state (see Phase C item 4 and Q2), pending
   the Phase A baseline showing whole-subtree export is material on both
   hosts. *(Partially resolved 2026-07-12: Windows moved to the packed
   structural export (`get_layout_tree_nodes`); the shared packed-ABI end
   state and the Apple/Hermes migration remain open with Q2.)*
8. What is the mutation/read ratio for `set_children` in real Contract and
   React lanes, and does that ratio rule out flat child columns for Phase B?
   *(2026-07-12: best answered by committing representative Contract/React
   mutation tapes and replaying them against all benchmark arms —
   frequency-weighted net benefit, combinable with the lookup-ledger
   receipts.)*
9. *(Added 2026-07-12.)* Who owns the first promotion-entry slice — the
   packed-ABI candidate at the top of the revised Phase C order? No lane or
   ticket names it yet (no-lane-no-blocker applies, so this blocks nothing),
   but a tracking issue should exist before anyone treats the revised order
   as a work queue, and the geometry-and-scroll-chain-only
   keyboard-avoidance resolver deserves its own ticket independent of any
   sidecar work.
