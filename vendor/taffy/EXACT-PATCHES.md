# Vendored Taffy — Exact patches

- **Upstream:** `taffy` 0.9.2 from crates.io
  (`https://github.com/DioxusLabs/taffy`, tag `v0.9.2`).
- **Why vendored:** six behavioral/API/performance patches (below) that the high-level
  `TaffyTree` API gives us no way to apply from the outside. Wired in via
  `[patch.crates-io]` in the root `Cargo.toml`, so `kernel/Cargo.toml` still
  declares a normal `taffy = "0.9"` dependency.
- **Owner:** Charlie Cheever (kernel/layout).
- **Replacement plan:** upstream the patch to DioxusLabs/taffy; drop this
  vendored copy and return to the crates.io release once a version containing
  the fix ships. Re-evaluate at the 2026-09-09 checkpoint (LLP 0151 cadence).

## Patch 1: used cross sizes and intrinsic cache entries (ENG-22727, LLP 1035.000.001)

`src/compute/flexbox.rs`, `determine_flex_base_size`: the resolved style
cross-size that seeds `child_known_dimensions` is now clamped by the item's
resolved min/max cross size before the content-based flex basis is measured.

Without the clamp, a flex item whose used cross size is bounded by a
min/max constraint has its content-based flex basis measured at the
*unclamped* cross size, while the final layout pass lays it out at the
*clamped* cross size. Any wrapping content (text) then reflows taller than
the frozen basis, and descendants overflow the item — scroll surfaces
computed from node frames stop short of the true content bottom.

Both directions of the mismatch are real:

- `width: 100%; max-width: 680` on a wide viewport (macOS window): basis
  measured at the full percent-resolved width, final clamped to 680.
- `width: 680; max-width: 100%` on a narrow viewport (iPhone): basis
  measured at 680, final clamped to the viewport.

CSS sizing resolves used sizes through min/max clamping before content is
laid out against them (CSS 2.1 §10.4; css-flexbox-1 §9.2.3 sizes the item
"into the available space" with its used cross size), and browsers agree —
this patch matches browser behavior for the reduced test cases.

**Intrinsic-cache extension (2026-09-12).** `src/tree/cache.rs` no longer
promotes an intrinsic result to a known dimension merely because its returned
size equals that dimension. A percentage column containing a code token wider
than its maximum is measured with unresolved descendant percentages during a
min-content probe. Its clamped width can equal the final width while its height
still describes the intrinsic probe. Reusing that height gave LLP 1032 a
67,333-point column around 30,049 points of blocks. Exact-input hits and
promotion from definite available-space offers remain cached; both measurement
and final-layout entries follow the rule. There is no global invalidation.

`kernel/tests/reader.rs` holds 24 literal-Chrome cases for both width spellings,
both box-sizing modes, three viewport widths, and ordinary/overwide-token text
inside padded blocks. `kernel/tests/fixtures/reader.html` regenerates their
geometry without Exact lowering. The measured intrinsic text height is 1,716;
at the final 628-point content width it is 468. `layout_equality.rs` holds the
resize, text replacement, maximum change, and subtree recreation sequence,
including scroll overflow, against fresh replay and rehydration. The focused
reader tests exercise the original cross-size clamp as well as this cache fix.
The upstream generated suite below predates this extension.

Validation: the full upstream test suite at `v0.9.2` passes with this patch
applied — 89 unit tests, **2060 generated conformance fixtures**
(`tests/fixtures.rs`), measure/caching/relayout/rounding suites. (The
generated fixtures live only in the git repo, not the crates.io package;
they were run against a patched clone of the tag.)

Local changes beyond the patch: removed `Cargo.lock`, `Cargo.toml.orig`,
`.cargo-ok`, and `examples/` from the crates.io package copy. Grep for
`EXACT PATCH` in `src/` to find every divergence from upstream.

## Patch 2: optional first baseline from measure-function leaves (LLP 0440 D5)

**What:** `MeasureOutput` adds an optional first-baseline channel alongside a
leaf's measured `Size`. Additive `compute_leaf_layout_with_baselines` and
`TaffyTree::compute_layout_with_measure_and_baselines` entry points carry it
into `LayoutOutput.first_baselines`. The incumbent size-only entry points keep
their original signatures and behavior by supplying `Point::NONE`.
Content-box baselines are translated through the leaf's border and padding
before flexbox consumes them.

**Why:** Taffy 0.9.2 already implements flex-row baseline positioning, but its
public measure-function contract returns only `Size` and
`compute_leaf_layout` hard-codes `first_baselines: Point::NONE`. Exact text
leaves therefore fell back to bottom-edge alignment even when CoreText knew
the paragraph's first baseline. LLP 0440 D5 requires `row align="baseline"`
to align different-sized text by that platform-measured baseline.

**Upstream status:** Exact-local; not yet submitted upstream. The additive
shape intentionally leaves Taffy's existing public APIs intact so the patch
can be proposed independently and removed when an upstream baseline-capable
measure contract ships.

The integration path is exercised by `kernel/src/layout.rs` using
`compute_layout_with_measure_and_baselines`, and `kernel/src/text.rs` covers
the emitted baseline metric. The focused flex-row test names formerly listed
here no longer exist; the next patch refresh must restore that coverage.

Authority: LLP 0440 D5.

## Patch 3: allocation-free unfrozen set in `resolve_flexible_lengths` (RFC 0491 WS-A)

**What:** `src/compute/flexbox.rs`, `resolve_flexible_lengths`: the unfrozen
item set was materialized on every iteration of the freeze loop as
`let mut unfrozen: Vec<&mut FlexItem> = line.items.iter_mut().filter(...).collect()`.
Each of its six uses is a single forward pass that never needs two items
borrowed at once, so the set is now expressed as a lazy
`line.items.iter().filter(|child| !child.frozen)` at each use site. No
allocation, no pool, no unsafe.

**Why:** one heap allocation per flex line per freeze round. Measured on
Exact's pinned 1,000-node fixture: **395 of 1,643 allocator calls** per
steady-state full relayout.

**Behavioral equivalence:** iteration order over `line.items` is unchanged, so
every floating-point accumulation (`sum_flex_grow`, `sum_flex_shrink`,
`sum_scaled_shrink_factor`, `total_violation`) sums the same values in the same
order and produces the same bits. Step (e) mutates `frozen` while iterating,
but each item's predicate is evaluated before its own body runs and no item
mutates another, so lazy filtering selects exactly the eagerly collected set.

**Upstream status:** Exact-local; a clean candidate for upstreaming — it is a
pure de-allocation with no API change and no behavior change.

## Patch 4: pooled `FlexItem` scratch buffer (RFC 0491 WS-A)

**What:** `src/compute/flexbox.rs`: `generate_anonymous_flex_items` fills a
caller-supplied `&mut Vec<FlexItem>` instead of returning a freshly allocated
one, and `compute_preliminary` checks that buffer out of a thread-local pool
(`mod flex_item_scratch`) that reclaims it on `Drop`. The pool is a *stack* of
buffers because flexbox recurses — a container measures children that may
themselves be flex containers holding a live borrow of their own buffer — and
is capped at 32 retained buffers. Under `not(feature = "std")` there is no
thread-local storage, so the scratch type degrades to the incumbent
allocate-per-container behavior.

**Why:** the single largest allocation site in the engine. Measured on Exact's
pinned 1,000-node fixture: **600 allocator calls and 537,600 bytes** per
steady-state full relayout — 74% of all bytes a layout pass allocates.

**Behavioral equivalence:** the buffer's contents are fully overwritten on
every use (`clear()` then `extend` of the identical iterator), so retaining its
capacity between passes cannot change a computed value; only allocator traffic
changes. `FlexItem` owns no heap data, so a pooled buffer retains capacity
only.

**Measured effect of patches 3 and 4 together**, pinned 1K fixture, steady
state, gross alloc/alloc_zeroed/realloc requests:

| axis | before | after | signed 0.5x target |
| --- | --- | --- | --- |
| allocator calls | 1,643 | 648 | 822 |
| allocated bytes | 722,956 | 172,716 | 361,390 |

**Upstream status:** Exact-local. Patch 3 is straightforwardly upstreamable.
Patch 4 introduces a thread-local pool, which upstream may prefer to express as
a buffer owned by the `LayoutPartialTree` implementor; if such a scratch seam
lands upstream this patch should be rewritten onto it rather than carried.

**Remaining known site, deliberately not patched here:** the per-container
`Vec<FlexLine>` from `collect_flex_lines` (`src/util/sys.rs:47`,
`new_vec_with_capacity`) is a further 600 calls but only 14,400 bytes. It
borrows `&'a mut [FlexItem]`, so pooling it needs either lifetime erasure or
an inline small-vector; both signed targets are met without it. Tracked in
`issues/20260826-taffy-flexline-vec-per-container.md`.

Authority: RFC 0491 WS-A; W0-A signed precommitment axes
`allocation-full-relayout-1k-{calls,bytes}`.

## Patch 5: replaced-element constraints for leaves with an aspect ratio (LLP 1011 §1)

**Files:** `src/compute/leaf.rs` (`compute_leaf_layout_with_baselines`, the
final size; `replaced_constraints`; the `SizingMode::ContentSize` arm).

**Why.** An `Image` is a measure-function leaf with a Taffy `aspect_ratio`
(its natural ratio, LLP 1011 §1). Upstream sizes such a leaf by clamping each
axis independently against min/max and then only flooring the height from the
width, so a 320×120 image with `max-width: 100` became 100×120 and one with
`max-height: 40` stayed 320×120 — where CSS (2.1 §10.4, the constraint table
for replaced elements with an intrinsic ratio) gives 100×37.5 and 106.67×40.
Upstream also dropped the ratio entirely under `SizingMode::ContentSize`, so a
flex container measuring the item's content-based flex basis at a stretched
cross size (css-flexbox §9.2 rule B) got the natural height (390×120 in a
stretching column) instead of the height by ratio (390×146.25).

**What.** With a ratio, the leaf's tentative size is the known/set dimension
and the other by ratio (both when both are known; the measured natural size
when neither is), resolved against min/max by the §10.4 table
(`replaced_constraints`), never axis-by-axis. `ContentSize` mode keeps the
style's ratio (rows and min/max are still ignored). Leaves without a ratio are
untouched (`_ =>` is the upstream code verbatim).

**Held by** `kernel/tests/image.rs` (`the_css_replaced_element_constraint_table`,
`in_a_stretching_flex_column_an_auto_width_image_fills_it_too`) and
`kernel/tests/layout_equality.rs` unchanged.

**Upstream status:** Exact-local; upstreamable as a correctness fix.

## Updating this vendor copy

Every Taffy refresh must review the fork against the selected upstream tag,
reconcile the numbered inventory with every `EXACT PATCH` marker, and verify
that every cited Exact test still exists. Run the upstream suite for the
patched tag and Exact's five checks before changing the pinned copy. Record
any intentionally missing focused regression here instead of retaining a
stale path.

**Scrollable content extension (Codex, 2026-09-11).** Exact now sets Taffy's
existing `item_is_replaced` marker for `Image`. A replaced leaf's
`LayoutOutput.content_size` is its used padding box, not its measured natural
bitmap plus padding. A 132×132 bitmap displayed at 32×32 must not contribute
132×132 to an ancestor's scrollable overflow (CSS Overflow §2.1). The marker
also enables Taffy's existing compressible grid-minimum behavior for images.
The kernel image regressions cover source replacement, up/down scaling,
padding/borders, all `object-fit` values and a percentage-constrained grid image;
independent browser cases and the Messages focused reply are under
`/tmp/messages-reply-overflow/`. Ordinary measured text still reports its content.

## Patch 6: intrinsic item contributions exclude the container's inset (LLP 1001 §5)

**Implementer:** Codex, 2026-09-11.

`src/compute/flexbox.rs`, `determine_container_main_size`: remove the two
`max(main_content_box_inset)` floors from the row/column item's intrinsic
contribution. The container adds its inset after summing the contributions;
using it to floor every item first counts parent padding twice for small content.
The item's own measured size and min/max constraints remain in force.

Messages demonstrates the row case: a nested text column inside a flex bubble
with 14-point horizontal padding makes `min-width: 48px` resolve to 56px. The
browser resolves it to 48px. A column container can likewise overstate height.
The high-level kernel regression
`intrinsic_flex_items_do_not_include_their_containers_padding` fails before the
repair and passes after it across 48 parent/child direction and padding cases.
A separate 24-case browser matrix confirms their width/height expectations.
Artifacts: `/tmp/messages-short-width/`. The full kernel suite passes; the full
upstream generated conformance corpus has not been rerun for this patch.

The current [upstream implementation](https://github.com/DioxusLabs/taffy/blob/main/src/compute/flexbox.rs)
also omits these floors (checked 2026-09-11). This is the bounded correction to
our 0.9.2 copy, not an import of upstream's other intrinsic-sizing changes.
