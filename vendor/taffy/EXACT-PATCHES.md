# Vendored Taffy — Exact patches

- **Upstream:** `taffy` 0.14.0, crates.io package supplied offline at
  `~/Library/Caches/exact2-textflow/taffy-0.14.0/` (M8, 2026-09-18).
  Its `.cargo_vcs_info.json` pins commit `77f385683c1d698c91a23a259f87fdddf26925fb`.
- **Why vendored:** patches 3, 4, 5, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19 and 20 below remain. `[patch.crates-io]`
  selects this copy; the kernel declares `taffy = "0.14"`.
- **Owner:** Charlie Cheever (kernel/layout).
- **Features:** std, taffy_tree, flexbox, grid, block_layout, content_size, calc.
  `BlockContext` belongs to block_layout; float_layout is unnecessary and
  disabled. No float row is exposed. Upstream's other newly default features
  (flexbox_balance, detailed_layout_info) are unnecessary here too.
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

Auto-height exclusions landed later without any vendor change (LLP 1043.000
§8, "Stage 2"): the kernel restricts them to block contexts, where a leaf's
offset depends only on preceding content, and re-lays out to a verified fixed
point instead of reading `BlockContext` at measure time. Still zero lines here.

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

A sparse opt-in map retains, for candidate boundaries only, the exact final
`LayoutInput` and `LayoutOutput` and whether an ancestor could have consumed
any other content-dependent answer since it was last invalidated through the
node: a `ComputeSize` without both known dimensions (or a known width on the
horizontal axis: every algorithm's short-circuit answers those from the query
alone), or a `PerformLayout` under other inputs. Recording is at the
`cache_store` seam, which every miss passes, and a hit returns an entry stored
under the same known dimensions, so the record is complete. A full
`mark_dirty` through the node starts a new record, since every ancestor that
asked it anything is invalidated with it; a hidden layout's `cache_clear`
drops the saved inputs. `last_layout_input` answers only while nothing else
was consumed. `mark_dirty_to` clears the dirty path through that boundary;
`compute_boundary_with_measure` replays the same input through Taffy's
ordinary child-layout algorithm, retaining the parent-assigned location and
updating the box's own overflow. `set_style_unmarked` and
`set_children_unmarked` (children detached or already the parent's) change
the tree without invalidating it, for a caller that marks the node dirty,
either way, before any layout. This is one serial Taffy owner, without a
second engine or a continuation/pending-layout API. Callers must establish an
independent formatting context and invalidate ancestors when its output changes.

The kernel defers style, child-list and text invalidation to the next layout
and then admits, as a boundary for each change, the nearest box at or above it
(strictly above a restyled one) that clips both axes, is in flow, is not
restyled or dirty, and has a replayable record, under no hidden ancestor and
an unchanged definite viewport. Flex, grid, percentage and auto sizing need no
rule of their own: the record says whether the ancestors asked. Every
ordinary invalidation walks before any local one, so none stops at a box a
local walk cleared. A changed size, collapsed-margin or baseline output
propagates normally, except that a flex column's non-startmost item's
baselines are unread (CSS Flexbox §8.5 and §9.4: the column aligns no item by
baseline and takes its own from its startmost item). Clipped internal overflow
publishes on the boundary. Exclusions, changed offers, a dirty root and
nested boundaries take the normal root path. A virtualized list (`flex: 1`,
`min-height: 0`, `width: 100%` under a column) is such a boundary for its
window changes, row measurement, inserts and removes.

Kernel publication follows sparse ancestor paths in document order and descends
where absolute origins move. Per-root publication generations expire old geometry
flags without a sweep. Apple accumulates the resulting publication candidates
through silent list passes, including overflow-only changes, before comparing
against its presenter mirror. Region/exclusion publication retains its existing
conservative traversal.

Regressions in `kernel::locality_tests` compare all frames and overflow bitwise
with a fresh engine, assert one measure and three publication visits among 100
and 2,000 unrelated siblings, and exercise negative dependencies, mixed dirty
sources, changed viewports and reparenting. `layout::containment_tests` holds
the list shape local and its coupled cases (first item, content-sized, header,
restyled) at the root; `layout_equality`'s random trees gain clipping boxes and
a list-shaped differential of clipping panes, each round compared with a
rehydrated and a replayed kernel, frames and scroll extents. Over 1,500 seeds
of both (about 14,000 contained replays) this patch adds no divergence: the
four seeds that differ also differ on the prior tip; patch 11 fixes all four. Apple layout tests cover silent
settlement and inherited spelling hints on unmoved editors. Existing layout,
reader, exclusion, region and upstream differential expectations are unchanged.

## Patch 10: percentage padding and border resolve against the inline size — to upstream

**Implementer:** Claude, 2026-09-23 (found by the 2026-09-22 kernel review).

`src/compute/block.rs::generate_item_list` resolved each child's padding and
border with `resolve_or_zero(node_inner_size, …)`, a `Size` basis, so the top
and bottom sides resolved against the container's inner *height*. CSS Box
Model 3 §4 (CSS 2.1 §8.4) resolves percentage padding, and Taffy's
percentage border, against the containing block's inline size on every side,
as the item's own layout (`parent_size.width`) and the flex and grid item
paths already do. The wrong basis fed the item's `box_sizing_adjustment` and
`padding_border_sum`: a content-box `height: 50px; padding-top: 5%` child of
a 400px-wide auto-height block was 50px tall (Chrome 153: 70px), and under a
300px-tall parent `height: 10px; padding: 5% 0 10%` was 60px (Chrome: 70px).
`src/compute/flexbox.rs::determine_used_cross_size` had the same basis in the
content-box adjustment of a stretched item's maximum cross size (a 400×300
row, `max-height: 50px; padding-top: 10%`: 80px, Chrome: 90px). Both sites
now pass `.width`; nothing else changes.

Upstream wants the two-line fix and a gentest per site in its HTML fixture
format; the cases above are that fixture's content. Held by
`kernel/tests/it/browser_cases.rs::percentage_padding_resolves_against_the_containing_block_width`:
nine literal-Chrome cases, including the `padding-top: 56.25%` embed idiom,
which a zero `height` already kept right (only the box's own padding
counts then). Taffy's 130 unit tests pass on the patched source (a scratch
copy without the uncached roxmltree dev-dependency, as in M8).

## Patch 11: the layout cache is keyed on every layout input — to upstream

**Implementer:** Claude, 2026-09-25 (found by `layout_equality`'s seeds).

`src/tree/cache.rs::CacheKey` left three inputs out of the key under which it
reuses a result, so an entry computed under one input answered another:

- `vertical_margins_are_collapsible`. Whether the node sits in its parent's
  block formatting context decides whether a first or last child's margin
  collapses through it (CSS 2.1 §8.3.1), and so its size, its children's
  positions and the margins it reports. A block parent that turns flex or grid
  reused its child's final layout with the inner margin still collapsed through
  it (`run(2423214, 14)`: a 4.5-point margin in the wrong place).
- `sizing_mode`. `InherentSize` applies the node's own size, min and max
  styles; `ContentSize` ignores them. Flex asks `ContentSize` (flex basis,
  final layout); block and grid ask `InherentSize`, with the same known
  dimensions (`run_panes(1412, 40)`: a 58-point styled width answered a
  110-point basis probe).
- The parent's height, for measurements. `ComputeSize` lookups compared only
  the parent's width, but a node resolves its own percentage height against the
  parent's height (CSS 2.1 §10.5): `height: 86%` measured under a 62-point
  parent answered once the parent's height was auto (`run_panes(503, 40)`;
  `run_panes(405, 40)` is the same through `sizing_mode` too).
  Final-layout entries already compared the whole parent size.

The key now carries the two fields, and a measurement matches on everything but
the requested axis. The alternative, clearing a child's cache when its parent's
display changes, is not correct in general: the cache memoizes a function of the
node's subtree, which its own dirty flag covers, and of its `LayoutInput`, which
only the key covers. One unchanged block parent asks the same child under both
flag values (it measures every child's width with collapsible margins, then lays
a flex, grid or scrolling child out without), and a parent's height or sizing
mode changes without its display changing. Keying costs only hits between
genuinely different inputs.

**Upstream fixture** (`cache.rs` tests): three key tests, one per omitted input,
and three trees laid out, restyled and laid out again against a fresh tree:
block to flex and block to grid (margin collapse; grid differs from the block
only in the flag), and a block losing its height under a percentage-height flex
child. All six fail on the unpatched source and pass patched; Taffy's 135 unit
tests and 5 doctests pass (scratch copy without roxmltree, as in M8).

**Held by** `kernel/tests/it/layout_equality.rs`: `seed_2423214_…`,
`pane_seed_405_…`, `pane_seed_503_…` and `pane_seed_1412_…`, each failing before.
Seeds 1–10,000 of `run` and of `run_panes` (40 rounds) differed from a fresh
layout 21 and 16 times before and 0 times after; the 512-tree differential
passes before and after.

## Patch 12: sizing through `aspect-ratio` as CSS does — to upstream

**Implementer:** Claude (Opus 5.5), 2026-09-25, for LLP 1053 G1.

Upstream applies a ratio by `Size::maybe_apply_aspect_ratio` wherever a size,
min or max is resolved, in the box-sizing box, into any unset axis. Patch 5's
leaf then treated every leaf with a ratio as a replaced element and resolved
it by CSS 2.1 §10.4's table. Against Chrome 154 that is wrong in five ways:

- **A non-replaced box is not replaced.** A text leaf or empty box with
  `width: 200px; max-height: 50px; aspect-ratio: 1` is 200×50, not 50×50.
- **Min/max transfer only into an unsized axis.** A `max-height` becomes a max
  width for a stretched block (100×100 from `aspect-ratio: 1; max-height:
  100px`), never for a set width. The same holds for images: `width: 96px;
  max-height: 20px` on a 320×120 image is 96×20. The §10.4 table is for
  neither dimension set.
- **The derived block size is a floor.** For a box that is neither replaced
  nor a scroll container and whose `min-height` is `auto` (CSS Box Sizing 4
  §5.2), taller content grows it: `width: 20px; aspect-ratio: 1` holding five
  lines is 90 tall, not 20. `min-height: 0`, `overflow: hidden` or a
  `max-height` still hold it.
- **The ratio's box.** `auto <ratio>` and an image's natural ratio relate
  content-box sizes even under `box-sizing: border-box`.
- **A provisional stretch is not definite.** Flex measured a ratio item's
  base size, and its automatic minimum, under the stretched cross size of an
  indefinite container: `flex-grow: 1; aspect-ratio: 2` in a 400px row laid
  out at 1200×600 under a 600px-tall offer.

The fix is one module, `src/compute/ratio.rs`, plus its call sites.
`Ratio::resolve` takes a box's given border-box sizes and authored min/max,
and returns its size, min and max:
- min/max transferred into unsized axes;
- one given axis clamped, the other derived from it;
- a floored derived height becomes a minimum instead of a size.

`Style::aspect_ratio_content_box` (with `CoreStyle::aspect_ratio_content_box`)
carries the box. The call sites are:
- `leaf.rs`: sizes through `Ratio`, keeps the §10.4 table only for a
  replaced element with neither dimension given, and derives a
  content-sized box's height from its width.
- `block.rs`: the entry size, the item sizes and the container's
  known-dimension transfer.
- `flexbox.rs`: the item size; min/max transfer only into axes the item
  does not size (`transfer_into_unsized`); the base-size and automatic-minimum
  measurements drop a provisional stretched cross size for ratio items.

**Extended 2026-09-30 (Claude, Fable 5.1, for LLP 1074 T2)** to the paths the
first tranche left on upstream's `maybe_apply_aspect_ratio`:
- a flex and a grid container's own size, min and max (`flexbox.rs`
  `compute_flexbox_layout` and `compute_constants`; `grid/mod.rs`);
- grid items, in `grid/alignment.rs::align_and_position_item`, in
  `GridItem::known_dimensions` and in `GridItem::minimum_contribution`, where a
  minimum only the ratio gives (a transferred one, or the floor a derived
  height is) holds beside the automatic minimum, never in its place. The item
  keeps the ratio's box and whether its height floors
  (`aspect_ratio_content_box`, `ratio_floors_height`), and `Ratio::from_parts`
  and `resolve_through` serve an algorithm that holds the parts, not the style;
- absolutely positioned boxes (patch 18) and the root (patch 19).

Completed 2026-09-30: the automatic minimum of a width derived from a definite
height, and percentage resolution against the preferred ratio height even
when the content minimum enlarges the used box. The five `OWED` cases were
removed; new Chrome fixtures cover overflowing percentage children, inline
minima, explicit min/max bounds, scroll-container opt-outs and absolute roots.

**Held by** `kernel/tests/it/browser_ratio.rs`: 60 literal-Chrome cases.
`image.rs` and `video.rs` each change one expectation to Chrome's: a set width
stays when `max-height` clamps the ratio's height. Taffy's 136 unit tests and
5 doctests pass on a scratch copy without roxmltree. The 512-tree differential
passes.

## Patch 13: `TaffyTree` resolves `calc()` through a caller's function

**Implementer:** Claude, 2026-09-24 (the iOS Messages port's
`calc(100% - 89px)`).

Upstream's `calc` feature stores a `calc()` length as an opaque pointer and
asks the tree for its value through `LayoutPartialTree::resolve_calc_value`,
but `TaffyTree`'s own implementation (`src/tree/taffy_tree.rs`, on
`TaffyView`) is a stub returning `0.0`: only a custom tree can resolve one.
`TaffyTree` gains a `calc_resolver: fn(*const (), f32) -> f32` field,
`|_, _| 0.0` by default, a `set_calc_resolver` setter, and `TaffyView`
forwards to it. Nothing else changes; the `calc` feature is now enabled.

The kernel interns each `(percent, points)` pair (`kernel/src/style.rs`,
`calc_handle`) and names it by index shifted past the three tag bits, so a
handle is never null or misaligned, equal pairs compare equal as styles, and
resolution (`resolve_calc`: `basis × percent / 100 + points`) needs no
unsafe code. Held by
`kernel/tests/it/browser_cases.rs::calc_of_a_percentage_and_a_length_resolves_against_the_containing_block`
(a `calc(100% - 89px)` child of a 400px block is 311px; a calc height, margin
and padding resolve against their own bases) and the codec's round trip of
wire kind 7. Upstream would want the same hook or a `TaffyTree` generic over
a resolver; either removes this patch.

## Patch 14: a flex item's intrinsic contribution clamps before its margin — to upstream

**Implementer:** Claude (Opus 5.5), 2026-09-26, for LLP 1054 P1.

`determine_container_main_size`'s intrinsic path (`compute/flexbox.rs`, the
min/max-content contribution of an item with a min or max main size) added
the item's margin to its content size and *then* took
`max(flex_basis)` and clamped by `min-height`/`max-height`, both inner
sizes. A negative margin therefore vanished: a content-sized column holding
`row min-height: 48px; margin-top: -44px` over 90px of content measured 90
where Chrome measures 46, so a Bluesky profile header was 44 pt too tall on
iOS (whose list rows are the kernel's layout) and right on the web (the
browser's). The fix clamps the inner size and adds the margin after, in
both the measured branch and the definite-preferred-size branch. Held by
`contract/cli/tests/it/negative_margin.rs` (five item shapes, and the
header the port found). Upstream has the same order.

## Patch 15: a node's style is shared — Exact's

**Implementer:** Claude (Opus 5.5), 2026-09-27, for the crypto list's memory.

`NodeData::style` is an `Rc<Style>` (`src/tree/taffy_tree.rs`), and
`new_leaf`, `new_leaf_with_context`, `new_with_children`, `set_style` and
`set_style_unmarked` take `impl Into<Rc<Style>>`, so an owned `Style` still
works and a caller may hand the same allocation to many nodes. A `Style` is
552 bytes and most of a list's nodes repeat a few: the kernel interns the
styles it lowers (`kernel/src/layout.rs`, `LayoutTree::share`, equality as
`write_style` already compares them) and each node holds a pointer. Layout
reads the style through the `Rc`; nothing else changes, and no result can.
It makes `TaffyTree` `!Send`, which the kernel already was. Held by the
kernel's layout suites (`cargo test -p exact-kernel`), which lay out through
shared styles throughout, and `kernel/src/kernel/trim.rs`. Upstream would
want an `Arc` or a style store keyed by handle; this stays Exact's.

## Patch 16: a replaced element that holds children is sized as a leaf — Exact's

**Implementer:** Claude (Opus 5.5), 2026-09-27, for `canvas`'s default size.

`TaffyView::compute_child_layout` (`src/tree/taffy_tree.rs`) dispatches a
node with children to its display's algorithm, which sizes an auto box from
those children. A canvas is a replaced element whose children are laid out
in its box and never size it (LLP 1014 D1, the web's `layoutsubtree`). A
node whose style says `item_is_replaced` and that has a measure context is
therefore sized by the measure function, as a leaf with no children is; on
`PerformLayout` its display's algorithm then lays the children out at that
border-box size as an independent formatting context, and only their
scrollable overflow is kept from it. Held by `kernel/tests/it/canvas.rs`
(`a_canvas_with_children_is_its_natural_size`). Upstream has no replaced
container; this stays Exact's.

## Patch 17: a replaced element never stretches to a grid area or its insets — to upstream

**Implementer:** Claude (Opus 5.5), 2026-09-27, with patch 16.

Upstream stretches an auto-sized grid item whose `justify-self`/`align-self`
are `normal`, and derives an absolutely positioned box's size from opposing
insets, whatever the box. CSS gives a replaced element neither: `normal` is
`start` for an item with a ratio or a natural size (CSS Grid 1 §6.2), and a
replaced absolute box keeps its natural or ratio size, the insets only
placing it (CSS 2.1 §10.3.8, §10.6.5). With `item_is_replaced`:
`compute/grid/alignment.rs::align_and_position_item` defaults both axes to
`start` and ignores insets for size; `compute/grid/mod.rs` gives track sizing
the same default after placement, so an auto column is not sized for a
stretched item; the absolute pass skips the inset-derived width and height
(`compute/common/absolute.rs` since patch 18). Held by
`kernel/tests/it/browser_replaced.rs`
(`replaced_elements_do_not_stretch_in_a_grid_area_or_between_insets`, 66
literal-Chrome cases: canvas, iframe, video, `svg` with and without a view
box, and a loaded image).

## Patch 18: one solver for an absolutely positioned box — to upstream

**Implementer:** Claude (Fable 5.1), 2026-09-30, for LLP 1074 T2 and T4.

Upstream sizes and places an absolutely positioned box three times, in
`compute/block.rs`, `compute/flexbox.rs` and `compute/grid/alignment.rs`, and
the three disagree with each other and with Chrome 154:
- **Auto margins in a grid.** `left: 0; right: 0; width: 100px; margin: 0
  auto` stays at x = 0: the branch for an inset box read the margins with
  `auto` as zero.
- **Auto margins in a flex container** take the container's free space, not
  the space the insets leave: with `left: 20px; right: 60px` the box is 40 px
  off.
- **A block container** centres a box that has only a `right` inset.
- **The block axis.** No container centres a box taller than the space its
  insets leave; CSS gives the two margins equal negative shares there (§10.6.4
  has no exception, as §10.3.7 has for the inline axis).
- **The ratio.** Patch 12's five differences, in all three.
- **Shrink-to-fit.** A box without a width is measured in the whole
  containing block, not in the space its insets and margins leave.

`compute/common/absolute.rs` is the one solver (its module note states the
rules). Each algorithm gives it the containing block and, for an axis with
neither inset, where the box sits by that algorithm's own rule (patch 20's
`StaticPosition`). Patch 17's rule for a replaced element lives there now.

**Held by** `kernel/tests/it/browser_position.rs`
(`positioned_boxes_roots_and_ratios_match_chrome`): 279 literal-Chrome cases
in `fixtures/browser_position.tsv`, 93 of them absolutely positioned boxes in
block, flex and grid containers, LTR and RTL.

## Patch 19: a root is a block-level box in its offer — Exact's

**Implementer:** Claude (Fable 5.1), 2026-09-30, for LLP 1074 T2 and T4.

`compute/mod.rs::compute_root_layout` resolved a root's size only for
`display: block`, through upstream's ratio transfer, stretched an auto width
less its margins, and then placed the root at the origin whatever its margins
said. The kernel worked around the first part by rewriting every root's `width:
auto` to `100%` under border-box sizing (LLP 1010 §1), which left a root's
margins unsubtracted and made its `min-width`, `max-width` and authored height
border-box under `box-sizing: content-box`.

Now, for any root that is not absolutely positioned, whatever it lays its
children out as (CSS 2.1 §10.3.3):
- its size styles go through the ratio (patch 12);
- an automatic width fills the offer less its margins, within the limits the
  ratio transfers;
- its margins place it: auto margins take the space a given width leaves
  (equal shares; a negative share goes to the end margin alone), and an
  over-constrained box ignores its end margin. An RTL root is placed from the
  offer's right edge, as before.

The kernel's rewrite and its re-derivation on `AttachRoot` are gone
(`kernel/src/style.rs`, `kernel/src/txn.rs`). Upstream keeps a flex or grid
root content-sized, so this stays Exact's.

**Held by** the 39 root cases of `fixtures/browser_position.tsv` (block, flex
and grid roots: ratios, margins, auto margins, a root wider than its offer).
`presented_height.rs` changes one expectation: a content-box root's authored
height is its content height.

## Patch 20: `Position::Static` and the CSS containing block — to upstream

**Implementer:** Claude (Fable 5.1), 2026-09-30, for LLP 1074 T1.

Upstream has `Relative` and `Absolute`, and every container lays its own
absolutely positioned children out against its own padding box. CSS places
such a box against its nearest positioned ancestor. This patch adds the value
and the containing block; a tree that never says `Static` behaves as before.

- **`Position::Static`** (`style/mod.rs`): in flow, insets do nothing (block
  and flex item insets, grid's were already `Relative`-only), and the box
  contains no absolutely positioned descendant. Block layout's in-flow and
  margin-collapse predicates test `!= Absolute`.
- **`StaticPosition`** (`tree/layout.rs`): where a box sits on an axis with
  neither inset, as its parent's algorithm gives it — a rectangle in the
  parent's coordinates and an alignment. Block gives the point the box would
  be at in flow. Flex gives its content box and the box's alignment as the
  container's only item. Grid gives the area when the grid is the containing
  block and its content box when it is not, as Chrome does.
- **The seam** (`tree/traits.rs`, three defaulted methods on
  `LayoutPartialTree`): a `Static` parent does not lay an absolute child out.
  It keeps the child's static position (`set_static_position`). The box that
  contains the child asks the tree for the boxes it holds that are not its
  children (`hoisted_absolute_count`, `hoisted_absolute`), after its in-flow
  children are placed, and lays each out through the solver (patch 18) against
  its own padding box, writing the location relative to the child's parent.
  A `Static` box at the top of a tree holds what nothing else does.
- **`AbsolutePass`** (`compute/common/absolute.rs`) is that pass, shared by the
  three algorithms.
- **`TaffyTree`** keeps the static positions and the caller's record of which
  box holds which (`set_hoisted_absolutes`), and computes a parent's origin in
  the holder's coordinates from the unrounded layouts between them.

The record is the caller's to keep current, because only the caller knows when
a position or a child list changed, and a walk of the tree per layout would
undo patch 9. The kernel rebuilds it when one did (`LayoutTree::refresh_hoists`,
proportional to the absolute boxes and their depth). The dirty marks need no
help: a box moves between containing blocks only when a box on the path
between them is restyled or re-parented, which marks both dirty.

Patch 9's replay needs one rule more. A static box between an absolute box
and its containing block is no replay boundary, now or for the change that
takes it off that path: laying its subtree out alone would not place the box,
and its old containing block still counts the box that left
(`hoist_paths`, `hoist_paths_prior`). The 1,500-seed differential found the
second half.

**Held by** `kernel/tests/it/browser_position.rs`
(`containing_blocks_and_static_positions_match_chrome`): 79 literal-Chrome
cases in `fixtures/browser_containing_block.tsv`, of which seven are what else
makes a containing block in a browser (a transform, a filter) and are declared
not the kernel's (the Contract compiler lowers `position: relative` onto such
a box). `layout_equality.rs` draws positions, missing insets and end insets on
a second random stream, so its trees' seeds are unchanged: both differentials
(incremental against rehydrated and replayed, 40 rounds) pass over 1,500 seeds.


2026-09-30 ticket sweep: patch 18 measures shrink-to-fit between intrinsic widths
after subtracting the static inline inset; patch 19 uses that solver for an
absolute root in a definite offer. Chrome cases are in `browser_containing_block.tsv`.
Patch 12 also resolves percentages against the ratio height while allowing the
content minimum to enlarge the used height, and measures the automatic inline
minimum for height-derived widths (block, flex, grid, absolute boxes and roots).
