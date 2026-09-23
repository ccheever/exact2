# Vendored Taffy — Exact patches

- **Upstream:** `taffy` 0.14.0, crates.io package supplied offline at
  `~/Library/Caches/exact2-textflow/taffy-0.14.0/` (M8, 2026-09-18).
  Its `.cargo_vcs_info.json` pins commit `77f385683c1d698c91a23a259f87fdddf26925fb`.
- **Why vendored:** patches 3, 4 and 5 below remain. `[patch.crates-io]`
  selects this copy; the kernel declares `taffy = "0.14"`.
- **Owner:** Charlie Cheever (kernel/layout).
- **Features:** std, taffy_tree, flexbox, grid, block_layout, content_size.
  `BlockContext` belongs to block_layout; float_layout is unnecessary and
  disabled. No float row is exposed. Upstream's other newly default features
  (flexbox_balance, calc, detailed_layout_info) are unnecessary here too.
- **Replacement plan:** upstream the remaining fixes; remove each divergence
  when a published version provides it. The numbered inventory retains the
  history so a refresh cannot silently lose an Exact correction.

All upstream evidence below refers to the supplied, unmodified 0.14.0 source,
not to a moving network branch. The package's source is copied in full; examples,
Cargo.lock, Cargo.toml.orig and packaging receipts are omitted as before.

## Patch 1: used cross sizes and intrinsic cache entries — upstream

`src/compute/flexbox.rs::determine_flex_base_size` clamps
`child_known_dimensions` using `transferred_min_size`/`transferred_max_size`
before measuring the flex basis. `src/tree/cache.rs::CacheKey` encodes
known dimensions/available space, parent size and definiteness;
`Cache::get(&LayoutInput)` compares keys, never promotes a result's size
into an input dimension. Both parts of the old patch are therefore removed.

The 24 literal-Chrome reader cases in `kernel/tests/it/reader.rs` retain their
expectations, including both width spellings, both box-sizing modes and the
long-token overflow case. Incremental/fresh/rehydrated reader equality stays.

## Patch 2: first baseline from measured leaves — upstream API, kernel adapter

`src/tree/taffy_tree.rs::compute_layout_with_measure` now accepts
`FnMut(LayoutInput, NodeId, Option<&mut NodeContext>, &Style) -> LayoutOutput`.
`LayoutOutput.baselines` is `Baselines { first, last }`. The kernel calls
upstream's `compute_leaf_layout` for the ordinary size rules, captures the
host's first baseline, adds top border/padding, and writes `output.baselines`.
The custom MeasureOutput type and additive baseline entry points disappear.
No vendor baseline patch is needed. Region baseline-dependency regressions
and the 512-tree fresh-layout comparison remain consumers.

The style adapter lowers CSS `normal`, the alignment rows' default, to None
on every display (2026-09-23; before, only an implicit block align-content).
0.14's block algorithm establishes a BFC for Some(align-content), so a
stretch default carried into blocks would stop nested margins collapsing.
`default_block_alignment_preserves_nested_collapsed_margins`
pins 151px for both nested border tops, rather than the incorrect 211px
paragraph top. Authored block alignment establishes the BFC, as CSS says.
`measured_baselines_include_padding_and_inherited_direction_relayouts_boxes`
checks a literal padded baseline and inherited RTL placement/replay.

## Patch 3: allocation-free unfrozen set — re-applied

`src/compute/flexbox.rs::resolve_flexible_lengths` still collects a
`Vec<&mut FlexItem>` in upstream. Retain lazy forward iteration at every use.
Order and floating-point accumulation order are unchanged. Each item only
mutates its own frozen flag, so lazy filtering selects the same set.

## Patch 4: pooled FlexItem scratch buffer — re-applied

Upstream `generate_anonymous_flex_items` still returns a fresh Vec. Retain
the caller-supplied buffer and thread-local stack of at most 32 buffers.
The iterator body is upstream 0.14's, including its new definiteness logic.
Buffers are cleared before reuse; capacity is the only retained state.
More than 32 concurrent/returned buffers fall back to allocation/drop;
no_std retains upstream's allocate-per-container behavior. FlexLine allocation
remains unchanged. The 512-tree regression checks cached versus fresh layouts.

## Patch 5: replaced-element ratio constraints and overflow — re-applied

`src/compute/leaf.rs` still independently clamps axes and then floors height
from the ratio; ContentSize still discards the ratio. Port the CSS 2.1 §10.4
`replaced_constraints` table and retain the ratio in ContentSize. Retain the
replaced-element overflow correction, now expressed as
`scrollable_overflow_rect` covering the used padding box. Natural bitmap size
must not enlarge an explicitly sized image's scrollable extent.

`kernel/tests/it/image.rs` covers the constraint table and stretching flex column;
`review_fixes.rs` covers intrinsic item contributions. The new upstream block
algorithm itself respects an image's natural width: the former declared
block-stretch deviation no longer exists.

## Patch 6: intrinsic items exclude their container's inset — upstream

`src/compute/flexbox.rs::determine_container_main_size` adds the container's
inset after the item contributions. Neither item contribution is floored by
`main_content_box_inset`; the two old deletions need no port.
`intrinsic_flex_items_do_not_include_their_containers_padding` is unchanged.

## Patch 7: final-measure height proof — dropped, replaced by upstream input

The full LayoutInput above supplies run_mode, known_dimensions, parent_size
and sizing_mode. The kernel records the effective height proof separately;
upstream `compute_leaf_layout` preserves the ordinary callback contract:
ComputeSize receives border-box known dimensions, PerformLayout receives
Size::NONE. No proof metadata influences ordinary returned metrics.
`upstream_leaf_keeps_border_box_compute_size_inputs` keeps the 400px offer
for a 300px content-box text leaf with 50px side padding.

The old emulated-callback differential is removed. There were no recorded
old-frame fixture files. Its 512 seeded trees now compare incremental against
fresh 0.14 layouts (49,152 node layouts, including overflow and insets).
Literal browser fixtures, not two copies of the old callback, remain the
behavior oracle. The measurement-count assertion is the negative control.

## Patch 8: the automatic minimum is measured only where used — upstream

**Original implementer:** Claude (Fable 5.1), 2026-09-19, owner commit
`9d282bcd` (patch 7 on trunk's 0.9.2). Reconciled with 0.14 in M15.

The supplied upstream 0.14.0 `src/compute/flexbox.rs`,
`determine_flex_base_size`, already uses
`style_min_main_size.unwrap_or_else(|| { ... measure_child_size ... })`.
The option includes an authored minimum or the scroll container's zero
`Overflow::maybe_into_automatic_min_size`. Its closure therefore runs only
when the automatic minimum actually needs min-content. No port or additional
vendor source change is needed: **zero lines** beyond retained patches 3–5.

The owner's rationale still applies: 0.9.2's eager `unwrap_or` discarded
this measurement when a minimum was already known. A flex reader list laid
out its entire mounted content at zero width on every window change, evicting
row layouts before measuring again at the real width. His 2.3 MB document,
three seconds at 3,600 px/s, went from 59,682 to 614 measure calls, unchanged
row calls 57,793 to 9, and CoreText measurements 694 to 455. These are his
0.9.2 measurements, not a new 0.14 timing claim.

**Held by** his unchanged `kernel/tests/it/reader.rs` regression
`a_scroller_in_a_flex_row_is_not_probed_for_a_minimum_it_does_not_use`
(only 628px text offers; 5974px column), and the two adjacent intrinsic-cache
regressions, now with an ordinary non-scrolling box that requires the probe.
His `text_measurement_cache.rs::a_padded_text_that_flexes_wraps_in_its_content_box`
also retains its 260px border box / 244px content box / 56px height assertions.
The 0.14 kernel adapter now takes both text offers from content space while
keeping `LayoutInput`'s independent height proof and `LayoutOutput` baselines.

The owner's 0.9.2 upstream suite report was 2,181 passed, 6 failed, 4 ignored
both with and without his patch. Those results are historical, not results
for this 0.14 package. M15 validation: all 8 tests in the owner's reader and
text-measurement-cache suites pass with their expectations unchanged; the
source-identical 0.14 scratch copy passes 130 unit tests and 5 doctests using
M8's offline setup (unused uncached roxmltree dev dependency omitted).
The generated browser conformance corpus is absent from the supplied package.

## Changed expectations in the upgrade

- `image.rs`: auto-width block image 390×146.25 becomes intrinsic 320×120,
  as CSS replaced-element sizing requires. The old test explicitly pinned a
  deviation; the replacement tests its removal.
- `reader.rs`: remove the requirement to execute a 1716px MinContent probe.
  0.14 avoids that probe. The final 628px offer/468px measured height,
  5974px column and no-remeasurement cache assertion are unchanged.
- `reader.rs`: column's propagated horizontal overflow 1278 becomes 1246
  (32px start padding + 14px child inset + 1200px word). Upstream's
  `compute/common/scrollable_overflow.rs` and flexbox final layout implement
  CSS Overflow's end-padding contribution only for scroll containers. The
  prior value added 32px end padding to an overflow-visible column; the
  literal HTML fixture never recorded this value. Its 24 browser-backed
  geometry/scroll-height cases all remain unchanged. Chrome confirmation of
  this horizontal extent is owed outside the sandbox.

## M8 Part B investigation — no new patch shipped

`BlockContext` exists without float_layout, but its private `y_offset` and
`insets` are provisional, not always the final child border-box origin.
An instrumented source copy under ignored `target/textflow-scratch/m8/`
ran these cases; it is not linked into Exact:

- A 50px preceding block with 40px bottom margin followed by a paragraph
  with 10px top margin: context y=100, final y=90.
- The same paragraph at width 100 in a 200px parent, with auto side margins:
  context x=0, final x=50.
- A scratch correction uses the collapsed sibling margin and resolved auto
  side margins. These two cases pass, but a nested block with 60px top
  margin and first paragraph with 100px top margin still measures at y=110
  and is ultimately placed at y=150 (the paragraph is at local y=0).

`compute/block.rs::perform_final_layout` calls the child before it knows
`item_layout.top_margin`, then resolves `y_margin_offset` and final location.
It also resolves relative insets after measurement. Forwarding this context
alone is therefore insufficient for the requested arbitrary block parent
chain. A sound continuation needs settled margin-strut/position semantics
and the wrapping context's identity within the BFC, as well as the leaf
callback parameter and cache treatment. No relayout loop or partial offset
API was introduced. The Part B vendor diff is **zero lines**.

Auto-height exclusions, pre-resolving absolute boxes, auto-height replay
coverage, web height publication and the seventh article scene remain undone.
The existing definite-height demo path and skip/journal behavior are retained.

## M8 validation and handoff

All Cargo commands ran offline with EXACT_UPDATE_TRUST=development. Bun's
frozen offline install passed. Exact's public measure API and native ABI are
unchanged; this is an engine adapter change, not a new exclusion interface.

The five checks on the final source:

| Check | Result |
| --- | --- |
| cargo build --workspace --offline | Pass, 377.817s |
| cargo test --workspace --no-fail-fast --offline, three allowed exclusions | Fail: results below |
| cargo clippy --workspace --offline --all-targets -- -D warnings; cargo fmt --all -- --check | Both pass |
| bun scripts/caps.mjs | Pass: 715 source files, 93 vendored files excluded |
| bun scripts/boot.mjs | Pass: 2 modules, 1 wasm reference, 138248 reachable JS bytes |

The final workspace suite, with the brief's three exclusions (exact-js,
exact-js-bake, exact-js-web), completed in 2670.502s: **1632 passed, 2 failed,
9 ignored, 283 targets**. It is not a green workspace result:

- Apple `fresh_preparation_reads_platform_secrets_and_defers_effects_until_commit`
  fails at abi_tests.rs:260 because Keychain returns Operation not permitted.
- Linux `full_undrained_session_does_not_block_another_sessions_native_workers`
  fails at image/control_tests.rs:180: delivery_cells is 1 instead of 0.
  It passed the first workspace run; two isolated retries failed too (2 and 1
  cells). Stop after those three failures. The test and worker implementation
  are untouched, and its fixture does not compute layout. This remains an
  investigation for the orchestrator, not a claimed resolved flake.

The first workspace suite had 1632 passes and only the Keychain failure,
before the final block-default regression test was added. An earlier focused
host run also hit Apple's process-wide worker cap in
`byte_budget_and_illegal_opt_ins_refuse_before_transport`; it passed alone
and did not recur in either workspace suite.

Additional results:

- Kernel: 242 passed. All 24 literal-Chrome reader cases, the 512-tree
  differential, layout equality, scrolling/list and review fixes pass.
- Textflow app: all 9 Linux tests and the data test pass; the all-scenes
  larger-font/shape-avoidance test also passes in release mode.
- Available Taffy tests: 130 unit tests and 5 doctests pass in a source-identical
  scratch copy, before offset instrumentation. Its unused roxmltree dev dependency
  was omitted because that version is not cached. Generated upstream browser
  conformance fixtures are absent from the supplied package and were not run.
- Web glue: 21 tests pass. `bun host/web/build.mjs textflow-web` passes.
- `bun host/apple/build.mjs --app textflow` passes with the supplied Swift shim
  first on PATH. Swift XCTest: 159 of 163 tests pass; ten assertions fail in the
  same four untouched MacShortcut/MacToolbar window/sheet tests recorded by M9.
- Release Linux screenshots: all six scenes, 100% and 115%, at 960x900 and
  clock zero, captured and visually inspected. Every 100% image is pixel-identical
  to the requested M9 reference; all twelve match the debug captures. Explicit
  EXACT_PAINTER=cpu is required here: default GPU discovery never became ready
  in three attempts (including a 120s probe).

The 10,000-ordinary-leaf test still checks no unchanged remeasurement and
at most eight measurements after the small side context's exclusion moves.
The 2,000-paragraph/32-exclusion stress case checks 64,000 candidate leaf visits
per pass. This upgrade adds no exclusion traversal or relayout loop. Patch 4
retains at most 32 scratch buffers per thread; excess buffers are dropped,
with capacities proportional to encountered container sizes. The padded
baseline and explicit block-BFC cases are additional negative controls.

Logs, timing samples, offset probes, and inspected linux-cpu-*.png captures
are under target/textflow-scratch/m8/. Outside the sandbox, rerun the full
workspace suite including the three JS packages, the Swift suite, and drive
all six scenes in macOS and Chrome at both font sizes. Confirm reader.html's
horizontal overflow interpretation in Chrome. B/C remain pending; no seventh
scene or fixed-height-slack removal has been claimed. QUEUE.md also records
LLP 1001's now-obsolete block-image deviation for a governing-doc follow-up.

### Layout timings

Five paired serial runs of the existing kernel flow tests, alternating the
retained pre-upgrade executable and final 0.14 executable, after local builds
finished. One warm-up per executable is excluded; no measured sample is dropped.
Medians are milliseconds, not per-frame numbers:

| Existing workload | 0.9.2 + seven patches | 0.14 + retained patches |
| --- | ---: | ---: |
| 10,000 ordinary leaves, 30 cached passes | 53.126 | 52.980 |
| Same plus a small side wrapping context | 51.701 | 53.537 |
| 2,000 paragraphs, 10 passes, no exclusions | 3.253 | 3.546 |
| Same with 32 exclusions and one moving source | 453.706 | 477.922 |

Moving-exclusion medians are 5.3% slower in this sample; ranges overlap
(old 442.864–504.126ms, new 458.443–621.273ms). The loaded shared machine
also produced one failed 5% side-context timing gate: new run 3 measured
75.123ms plain versus 172.381ms with the small context. All five old runs and
the other four new runs passed all eight tests. The failure is retained in
paired-final-new-3.log and paired-final-timings.json. Earlier samples during
builds varied substantially too. No speedup or resolved performance result is
claimed: rerun the unchanged timing gate and paired comparison on an idle
machine. The measure-count and sparse-work assertions remain unchanged.

## Updating this copy

Reconcile every `EXACT PATCH` marker with this inventory, compare against the
selected upstream package, and run the available upstream tests and Exact's
five checks. The crates.io package does not include the generated browser
conformance corpus. Do not describe those absent fixtures as tested.

## Patch 9: sparse layout writes and caller-proven boundary replay (S6)

`tree/taffy_tree.rs` records changed unrounded layouts at their existing write
seam. `take_layout_changes` transfers those node identities to the kernel;
the sparse journal deduplicates repeated writes and removes destroyed identities,
so nonpublishing region trials retain at most one entry per live node. A dense
journal plus sparse positions makes both draining and removal proportional to
changed entries, without scanning a prior large hash-table capacity. No layout
algorithm, cache key, root sizing rule or rounding behavior changes.

A sparse opt-in map retains the exact final `LayoutInput` and `LayoutOutput`
for candidate boundaries only. `mark_dirty_to` clears the dirty path through
that boundary; `compute_boundary_with_measure` replays the same input through
Taffy's ordinary child-layout algorithm, retaining the parent-assigned location
and updating the box's own overflow. This is one serial Taffy owner, without a
second engine or a continuation/pending-layout API. Callers must establish an
independent formatting context and invalidate ancestors when its output changes.

The kernel currently admits text invalidation inside an ordinary block with
point width/height and hidden/scroll overflow on both axes, under block,
nonabsolute, nonintrinsic/nonpercentage ancestors and an unchanged definite
viewport. The root's viewport percentage lowering is retained. Descendant
percentages use the saved input's original parent size and definiteness. A
changed size, baseline or collapsed-margin output propagates normally; clipped
internal overflow publishes on the boundary. Flex/grid ancestors, auto/percent
boundary sizes, visible overflow, changed offers, exclusions, concurrent style
or topology changes, and nested dirty boundaries take the normal root path.

Kernel publication follows sparse ancestor paths in document order and descends
where absolute origins move. Per-root publication generations expire old geometry
flags without a sweep. Apple accumulates the resulting publication candidates
through silent list passes, including overflow-only changes, before comparing
against its presenter mirror. Region/exclusion publication retains its existing
conservative traversal.

Regressions in `kernel::locality_tests` compare all frames and overflow bitwise
with a fresh engine, assert one measure and three publication visits among 100
and 2,000 unrelated siblings, and exercise negative dependencies, mixed dirty
sources, changed viewports and reparenting. Apple layout tests cover silent
settlement and inherited spelling hints on unmoved editors. Existing layout,
reader, exclusion, region and upstream differential expectations are unchanged.
