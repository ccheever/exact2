# Review: LLP 1074, the CSS Taffy lacks (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort="max" --sandbox read-only`, one fresh session in the main checkout.
- **Date:** 2026-09-30.
- **Reviewed:** `llp/1074-css-taffy-lacks.rfc.md` as staged over `617eed586` (blob `0a62b06d`), Draft, before any revision.
- **Redacted:** no. The document and the checkout contain no secrets the brief exposed.
- **Method:** a sanity check of the high-level decisions, asked for by Charlie. Mutually blind to the other family (Grok 4.7 xhigh), which got the identical brief (sha256 `a29f33a8661cc129ee1b172c10554c211e020bfe2407143672f6862207da35d8`). A static source audit: the reviewer ran nothing. The RFC's author (Claude) is not a reviewer.
- **Disposition:** in the RFC, §0.

---

## Round 1 (verbatim)

The RFC has the right general direction, but I would revise it before approving the proposed work. `static` should become the default, and Taffy should support containing-block-aware positioning; however, T1 needs a broader design covering native rendering and incremental layout. T3 is already implemented, the description of windowed lists is obsolete, and route B cannot recover correct CSS positioning merely by laying subtrees out again. T2 and T4’s grid case are worthwhile fixes. §4 is broadly accurate; T5 and T6 should remain deferred, with their scope clarified.

1. **HIGH — T1 cannot delete the web paint-order rule without changing native rendering.**

   The missing `static` value is real: [Taffy’s `Position`](/Users/ccheever/projects/exact2/vendor/taffy/src/style/mod.rs:308) contains only `Relative` and `Absolute`. Making `static` the default is the right destination. Calling the whole change “large” is believable; treating it primarily as three layout-algorithm changes is not.

   [The web rule](/Users/ccheever/projects/exact2/host/web/src/layers.rs:3) explicitly compensates for native tree-order painting. The Linux renderer still recursively paints each child subtree, with sibling ordering based on depth, and applies the parent’s clipping and scroll offset to those children ([paint.rs:1068](/Users/ccheever/projects/exact2/host/linux/src/paint.rs:1068)). Changing containing-block geometry does not change that traversal.

   Consider the RFC’s own motivating pattern: an absolute photo followed by static text. Removing the web’s forced `relative` changes their overlap order. Native must implement the corresponding CSS paint and hit-test order too. Likewise, an absolute descendant whose containing block lies outside an intervening static scroller cannot simply inherit every intervening scroll translation and overflow clip.

   **Resolution:** include native paint ordering, stacking, clipping, scroll attachment and hit testing in T1’s acceptance criteria. Preserve logical ancestry for inheritance and events while representing positioning and rendering dependencies separately. Audit host-created positioning too: [canvas styling explicitly injects `position:relative`](/Users/ccheever/projects/exact2/host/web/src/element.rs:23). Deleting `layers.rs` alone does not make every box’s containing-block behavior follow its authored value.

2. **HIGH — Route A is preferable; route B’s “first layout yields the static position” premise is false.**

   [RFC line 78](/Users/ccheever/projects/exact2/llp/1074-css-taffy-lacks.rfc.md:78) conflates the CSS *static position* with the final position produced by an incorrect containing block.

   Block layout records a separate static position during normal-flow traversal ([block.rs:1009](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/block.rs:1009)). Final absolute positioning uses that value only where the relevant insets are auto; otherwise the inset solution replaces it ([block.rs:1803](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/block.rs:1803)). The returned layout does not expose the discarded hypothetical position.

   Flex positioning also depends on the original parent’s alignment and the child’s **final** size ([flexbox.rs:3000](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/flexbox.rs:3000)). Resizing the child against a different containing block can change its centered or end-aligned static position. A saved point from the first pass is therefore insufficient even when both insets are auto.

   B also needs to repair ancestor overflow aggregates and their cached outputs. Overflow is accumulated by union; replacing the child’s frame cannot subtract its previous, incorrect contribution. Nested corrective subtree layouts can repeat work more than twice.

   **Resolution:** prefer A, retaining a deferred positioning record with the originating formatting context, static-position rectangle/alignment, containing-block owner and coordinate conversion. Compute the absolute subtree once its actual constraints are known.

   A useful third organization is a hybrid: the kernel maintains containing-block ownership, while Taffy exposes a shared positioned-child solver and deferred-placement records. This follows A’s scheduling model without requiring Taffy to understand every host transform or duplicating the positioning solver in the kernel. Preserve the original tree; wholesale reparenting would change other semantics.

3. **HIGH — T1 needs an explicit dependency model for patches 9 and 11, not merely another cache-key field.**

   Patch 11 already includes `parent_size`, definiteness, sizing mode and margin-collapse inputs ([cache.rs:83](/Users/ccheever/projects/exact2/vendor/taffy/src/tree/cache.rs:83)). Passing the real containing-block dimensions through those inputs can protect an absolute node’s sizing cache. It does not ensure that the node is visited: [a cached ancestor returns immediately](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/mod.rs:187).

   Two distinct failures need coverage:

   - A positioned ancestor changes width, while an intervening static box retains the same size and layout inputs. That static box’s cache can hide an absolute descendant whose percentage width must change.
   - The static parent moves while the containing block stays fixed. An absolute child with `left:0` must remain fixed in containing-block coordinates, so its **parent-relative** location must change even though its sizing inputs do not.

   Deferred placement records must therefore survive cache hits and be revisited for the correct dependencies. Not every dependency needs to enter every sizing cache key, but none can disappear behind a cached traversal.

   Patch 9 has a related assumption. [Boundary eligibility requires `Relative`](/Users/ccheever/projects/exact2/kernel/src/layout.rs:131), and [boundary replay deliberately suppresses internal-overflow changes when comparing parent-facing outputs](/Users/ccheever/projects/exact2/kernel/src/layout.rs:961). Leaving the predicate unchanged would lose locality for default-static scrollers. Simply admitting `Static` would be unsound when a descendant is positioned against something outside that boundary.

   Sparse publication must retain its write discipline: [Taffy journals changed layouts](/Users/ccheever/projects/exact2/vendor/taffy/src/tree/taffy_tree.rs:470), and [publication follows those changes and moved ancestor origins](/Users/ccheever/projects/exact2/kernel/src/layout/publication.rs:11). Corrective placement must update that journal, including necessary rebasing and overflow changes; directly patching published frames would bypass it.

   There is also a new invalidation dependency. Transform rows currently lack the layout flag ([schema.json:2124](/Users/ccheever/projects/exact2/kernel/tables/schema.json:2124)); [style changes restyle Taffy only for layout-affecting rows](/Users/ccheever/projects/exact2/kernel/src/txn.rs:1076). Adding or removing a containing-block-establishing transform would need to invalidate ownership and affected layouts.

   **Resolution:** specify dependency recording, cache-hit behavior, ownership changes, sparse writes and boundary eligibility together. Initially reject local replay across escaping positioning dependencies unless its independence can be proved. A block-only spike is useful, but cannot establish correctness for flex/grid static positioning or native clipping.

4. **MEDIUM — T1 contains several incorrect or incomplete CSS and repository assumptions.**

   These affect the implementation choice:

   - **Static boxes do not universally ignore `z-index`.** Static flex and grid items can use it. [RFC line 71](/Users/ccheever/projects/exact2/llp/1074-css-taffy-lacks.rfc.md:71) needs that exception, including the distinction between `auto` and an integer stacking level.
   - **Overflow alone does not establish an absolute-position containing block.** Scroll containment, a block formatting context and an absolute-position containing block are different concepts.
   - **A grid absolute child is not always positioned against the whole padding box.** The vendored engine already derives its containing area from grid placements ([grid/mod.rs:683](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/mod.rs:683)). A shared solver must preserve that behavior.
   - **Current windowed lists do not place their rows absolutely.** They use normal-flow flex wrappers and spacers ([views.rs:34](/Users/ccheever/projects/exact2/runner/src/instance/collection/views.rs:34), [views.rs:147](/Users/ccheever/projects/exact2/runner/src/instance/collection/views.rs:147)); the JS target uses the same styles ([list.js:143](/Users/ccheever/projects/exact2/host/web-js/list.js:143)). RFC line 85 describes the retired implementation.
   - **Existing “in flow” predicates require migration.** Block margin collapse explicitly checks `Relative` ([block.rs:517](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/block.rs:517)); auto-height flow does likewise ([flow.rs:102](/Users/ccheever/projects/exact2/kernel/src/flow.rs:102)); collection row validation admits only authored `relative` ([views.rs:206](/Users/ccheever/projects/exact2/runner/src/instance/collection/views.rs:206)). Merely adding an enum member would break or reject ordinary static content.

   **Resolution:** correct these assumptions before the spike. For collections, preserve work proportional to mounted content and explicitly handle row destruction, reparenting, escaping absolute descendants and dependency cleanup. Do not introduce a blanket “list content box” containing-block rule. Also distinguish the initial containing block from an auto-height application root.

5. **MEDIUM — T3 is already fixed; remove it from the implementation tranche.**

   The source directly contradicts [T3’s description](/Users/ccheever/projects/exact2/llp/1074-css-taffy-lacks.rfc.md:99):

   - [Block layout excludes replaced elements from stretch sizing](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/block.rs:1215).
   - [The kernel supplies the replaced-element flag](/Users/ccheever/projects/exact2/kernel/src/style.rs:1202).
   - [The current image test](/Users/ccheever/projects/exact2/kernel/tests/it/image.rs:263) expects an intrinsic `320×120` image inside a `390`-wide block, explicitly recording removal of the old deviation.
   - [The patch inventory](/Users/ccheever/projects/exact2/vendor/taffy/EXACT-PATCHES.md:81) says the same.

   Its proposed “small” estimate is consequently irrelevant: this is documentation reconciliation, not a missing implementation. The cited `kernel/tests/image.rs` path is also stale; the test is under `kernel/tests/it/`.

   **Resolution:** remove T3, reconcile LLP 1001’s obsolete deviation, and retain the current intrinsic-size expectation. Any newly discovered replaced-element failure should get its own precise case rather than reviving this already-fixed one.

6. **MEDIUM — T2 is real and correctly located, but “small” is optimistic for the complete scope.**

   The named paths still use upstream ratio transfer: [absolute block layout](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/block.rs:1648), [flex-container sizing](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/flexbox.rs:249), [grid-container sizing](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/mod.rs:70), [grid-item sizing](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/alignment.rs:144), and [root sizing](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/mod.rs:86).

   The width-derived-from-height omission is also credible: [the ratio helper derives width directly](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/ratio.rs:101), whereas its content-based floor machinery is specifically for height.

   Some replacements may be small. Completing grid intrinsic contributions and automatic minimums is not necessarily a mechanical helper substitution: the algorithm must distinguish authored, stretched, inset-derived and content-derived dimensions, including their definiteness.

   **Resolution:** keep T2 high in the order, but budget it as several bounded fixes, small to medium collectively. Cases should cover replaced/non-replaced boxes, both box-sizing modes, min/max transfer, intrinsic sizing and content exceeding the preferred ratio. One simple case per algorithm would not establish completion of patch 12.

7. **MEDIUM — T4’s grid bug is real; its explanation and coupling to root sizing need correction.**

   The grid function actually **does resolve** auto margins ([alignment.rs:447](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/alignment.rs:447)). Its absolute inset branches then use `non_auto_margin`, discarding the resolved values ([alignment.rs:486](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/alignment.rs:486)). Thus the reported `x=0` follows from source, but “resolves auto margins for in-flow items only” is inaccurate.

   This is a believable small fix. However, using the existing resolved margin blindly would also be wrong: its free space is calculated from the whole grid area without subtracting opposing insets. Cover nonzero insets, vertical margins, one/both auto margins, overflow and RTL.

   Root auto margins are genuinely missing ([compute/mod.rs:145](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/mod.rs:145)). The adjacent claim that Taffy does not stretch a block root is stale: [it already does so](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/mod.rs:109). The kernel nevertheless retains its broader [percentage-width/border-box workaround](/Users/ccheever/projects/exact2/kernel/src/style.rs:1368), which also covers roots whose internal display is flex or grid.

   **Resolution:** fix grid margins first. Treat root placement and root automatic sizing separately. Correcting root auto margins neither requires nor guarantees removal of the width workaround. Define the application root’s outer sizing semantics before removing it, including margins, padding, flex/grid roots and replaced roots.

8. **MEDIUM — T5 identifies a real measurement limitation, but it no longer unblocks the existing auto-height feature.**

   The provisional-offset problem is supported by the source: [measurement precedes collapsed-margin and auto-margin resolution](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/block.rs:1260). But auto-height exclusions already have an accepted implementation: [LLP 1043.000’s Stage 2](/Users/ccheever/projects/exact2/llp/1043.000-text-around-shapes.rfc.md:625) and [the kernel settlement loop](/Users/ccheever/projects/exact2/kernel/src/layout.rs:1043).

   T5 is therefore an API/performance improvement or an expansion of supported contexts, rather than a missing CSS feature that blocks present functionality.

   “Mark the offset provisional” is not an alternative implementation of stable-offset measurement; it leaves the caller needing settlement. Nor is the work confined to `block.rs`: [the measured-leaf callback receives no `BlockContext`](/Users/ccheever/projects/exact2/vendor/taffy/src/tree/taffy_tree.rs:408), and patch 11’s cache key contains no contextual origin or wrapping-context identity.

   **Resolution:** state the desired benefit—fewer passes, broader admitted layouts, or a particular performance target. Specify when offsets become final, how the measurement seam receives them, and how contextual changes invalidate cached measurements. “Medium” is plausible for a restricted contract; not established for arbitrary stable offsets throughout block layout.

9. **MEDIUM — T6 is a genuine gap, but complete `last baseline` support is larger than one extra text metric.**

   The alignment keyword is absent. Nevertheless, Taffy already has [both `first` and `last` baseline fields](/Users/ccheever/projects/exact2/vendor/taffy/src/tree/layout.rs:172). The kernel exposes only [a first baseline](/Users/ccheever/projects/exact2/kernel/src/text.rs:235).

   The remaining work includes consuming and propagating last baselines through containers, baseline-sharing groups, fallback synthesis and alignment placement. Existing [grid baseline calculations read `.first`](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/track_sizing.rs:501), as do the flex calculations.

   There is another patch-9 dependency: [the kernel considers non-startmost flex-column items’ baselines unread](/Users/ccheever/projects/exact2/kernel/src/layout.rs:596). That reasoning must be reconsidered once an ancestor can consume the column’s last baseline.

   **Resolution:** keep T6 demand-driven, but estimate complete cross-host support as medium rather than small. Explicitly include container propagation, measurement interfaces and replay dependencies.

10. **MEDIUM — The inventory misses useful sizing and ordering gaps.**

    Two omissions deserve an explicit disposition:

    **Intrinsic keywords in minimum/maximum sizes.** Taffy’s preferred `size` uses `Dimension`, but [its minimum and maximum sizes use `LengthPercentageAuto`](/Users/ccheever/projects/exact2/vendor/taffy/src/style/mod.rs:644). That type’s [CSS parser admits lengths, percentages and `auto`](/Users/ccheever/projects/exact2/vendor/taffy/src/style/dimension.rs:173), not intrinsic keywords. Supporting values such as `min-width: max-content` is distinct from T2’s automatic minimum. It matters for content-sized controls, labels and panels.

    **CSS `order`.** [The flex-item interface has no order property](/Users/ccheever/projects/exact2/vendor/taffy/src/style/flex.rs:53), and [flex layout consumes children in supplied order](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/flexbox.rs:771). This can be implemented at the adapter’s traversal boundary, without a new sizing algorithm. It must preserve logical/accessibility order while changing layout and paint order. The schema’s string prop named `order` is not a CSS layout-style row.

    **Resolution:** add both to the inventory with demand-based priority. They are plausible app-framework needs ahead of last-baseline support, but neither warrants immediate implementation without a consumer.

11. **LOW — §4’s three capability claims are true, but “only schema rows” understates integration and the list is incomplete.**

    - **Named grids:** real style fields exist, and [grid layout invokes named-line resolution](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/grid/mod.rs:683). No new grid algorithm is inherently required. However, Exact’s [numeric grid-placement representation](/Users/ccheever/projects/exact2/kernel/src/style.rs:1012) and track grammar need extension, together with parsing, validation, serialization and CSS emission.
    - **Safe/unsafe alignment:** real enum support and [overflow fallback logic](/Users/ccheever/projects/exact2/vendor/taffy/src/compute/common/alignment.rs:11) exist. The RFC correctly classifies this as exposure work.
    - **`flow-root`:** real support exists; [dispatch starts a fresh block context](/Users/ccheever/projects/exact2/vendor/taffy/src/tree/taffy_tree.rs:400). Its omission from the authored display vocabulary is an exposure gap.

    Other useful existing capabilities are omitted: preferred-size `min-content`/`max-content`/`fit-content`/`stretch` ([dimension.rs:414](/Users/ccheever/projects/exact2/vendor/taffy/src/style/dimension.rs:414)), implicit grid track sizes, richer grid track definitions, and `justify-self`. These should be distinguished from the missing intrinsic **min/max constraints** above.

    **Resolution:** retain §4, describe the integration work accurately, and expand the inventory before concluding that a requested CSS feature requires vendor changes.

12. **LOW — Most exclusions are reasonable, but their rationale should not create unnecessary dependencies.**

    Deferring full inline layout, subgrid, vertical writing modes, tables, collapse visibility and `display:contents` is reasonable absent a concrete consumer. I found no basis here to pull those large features into the immediate tranche.

    Three qualifications matter:

    - **Sticky:** keeping scroll-time execution in hosts is sensible. T1 is not a strict prerequisite: normal layout can already treat a sticky box as relative. A consumer could justify an independent design covering its scrollport, containing-block bounds, stacking and hit testing.
    - **Fixed:** it shares positioning machinery with T1, but its containing block is not always the viewport; transforms and other relevant ancestors can capture it. Scroll attachment adds host work. Do not assume A makes it cheap enough to include automatically.
    - **Floats and buttons:** exclusions do not implement float placement or `clear`; “covers the use” applies to the admitted shape-flow demos, not general CSS floats. Also, the block-button centering examples use block children ([button RFC:35](/Users/ccheever/projects/exact2/llp/1054.000.001-button-content-alignment.rfc.md:35)); fixing that control behavior need not wait for a general inline formatting engine.

    **Resolution and recommended order:** fix **T4’s grid case first**, then **T2 in bounded pieces**; remove T3. Give T1 its own complete design and cross-host landing. Handle root sizing separately. Keep T5 with a demonstrated text-flow need and T6 on demand. Expose §4 capabilities when requested; a concrete button need for safe centering should not wait behind T1.

My answers to the six open questions, in their order:

1. **Should `static` be the default? — Yes.** It follows the repository’s CSS rule and removes a pervasive authoring surprise. Change it when layout, rendering and invalidation are ready together. Sweep applications and internal generated views for actual containing-block requirements; blanket replacement with `relative` would preserve much of the problem.

2. **Route A or B? — A, preferably with kernel-owned containment bookkeeping and shared Taffy positioning machinery.** It preserves the original static-position context and avoids intentionally incorrect preliminary layouts. A block spike should include cached static ancestors, ancestor resizing, parent movement and escaped scroller descendants. Flex/grid cases remain necessary before approving the architecture.

3. **Which non-position properties establish containing blocks? — Follow the computed semantics of what Exact emits.** Include applicable non-`none` transforms and individual transform properties, including identity values; applicable filters and backdrop filters; and layout/paint containment if emitted. Size containment alone, scrolling alone, opacity and isolation are not interchangeable with those triggers. Audit animation/press-generated styles and host-created wrappers too. Containing-block creation/removal must become an invalidation event.

4. **Does fixed come with T1? — Design for it, but do not require shipping it in the first landing.** Distinguish absolute and fixed containing-block ownership now. Ship fixed only with correct viewport fallback, transformed-ancestor capture, scroll behavior, clipping and stacking. It is valuable for app overlays, but not a free enum extension.

5. **Are root auto margins worth fixing? — Yes, as a separate correctness fix.** The root is offered available space, not invariably forced to that exact size; an authored narrower root is a valid case. First settle its outer block semantics. Do not condition the fix on deleting the independent automatic-width workaround.

6. **Should upstreaming be part of this work? — Yes, as separately deliverable maintenance work.** Prepare focused fixes and reproducible upstream fixtures for independently useful patches. Existing fixes need not wait for all of T2, and local correctness should not wait for upstream acceptance. I did not check submission status.

This was a static source audit. I did not run builds, tests, interpreters, browsers or image tools; inspect the external scratch probes or external apps; compare against an external pristine Taffy checkout; verify upstream submissions; or reproduce the recorded Chrome and performance results. No files were changed. The permitted `git diff --cached --stat` invocation emitted sandbox-denied macOS launcher cache-write warnings; I did not retry it. Source inspection used checkout-local `rg` reads and `wc -l`.
