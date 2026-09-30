# LLP 1074: The CSS Taffy lacks, and which of it to add

**Type:** RFC
**Status:** Draft (r3: T1, T2 and T4 designed and built on `lane/taffy-css`, §0; r2: dispositions after two reviews, §0.1)
**Systems:** Vendored Taffy (`vendor/taffy`, upstream 0.14.0 plus Exact's patches 18–20), Kernel layout (`kernel/src/layout.rs`, `kernel/tables/schema.json`), Contract compiler (`contract/lower`), Web hosts (`host/web/src/layers.rs`, `host/web-js`), Apple presenters (`usedZIndex`), Runner collections (row validation), Text flow and content regions (their containing-block gates), Messages
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30 (r1, r2 and r3)
**Related:** LLP 1001 §5/§6 (declared deviations; the web is the standard), LLP 1010 §1 (the root's width), LLP 1053 (the `aspect-ratio` and `direction` tranche; "it's ok to modify exact2 and even taffy"), LLP 1043.000 (text around shapes), LLP 1054.000 and 1054.000.001 (Taffy patch 14; button content alignment), LLP 1011 (images), `vendor/taffy/EXACT-PATCHES.md` (patches 12, 18, 19, 20), `QUEUE.md`, reviews `llp/reviews/1074-css-taffy-lacks.{astra,grok}.md`

## Summary

Charlie asked, 2026-09-30: are there parts of CSS that Taffy does not implement and that we should add to make exact2 better? Then, after two reviews: "static default seems fine. do the design round yourself… move forward with this and implement everything that makes sense to."

| | Item | Disposition |
|---|---|---|
| T1 | `position: static` and the CSS containing block | **Built** (§3) |
| T2 | The rest of `aspect-ratio` | **Built**, including the ratio follow-ups (§4) |
| T3 | A replaced element's width in block flow | **Withdrawn**: the 0.14 upgrade had fixed it; LLP 1001 corrected |
| T4 | Auto margins on absolute boxes and on the root | **Built**, and wider than proposed (§4) |
| T5 | Stable block offsets at measure time | Not built: text flow no longer depends on it |
| T6 | `last baseline` | Not built: on demand |

What else is in Taffy and not exposed is in §5; what is missing and unasked for is in §6; what is not proposed is in §7. What is owed is in §8.

## 0. What r3 decided and built

The design round for T1 had one question to settle: the reviewers showed that "delete the web's paint-order rule" would split the web from native, and that an absolute box escaping a native scroll view cannot be presented without re-parenting views. Two decisions make T1 a layout change and nothing else.

**D1. The kernel is CSS; the compiler carries the one deviation.**
- In the kernel and in Taffy, `position` is `static | relative | absolute`, `static` by default, and an absolute box's containing block is its nearest positioned ancestor, or the root.
- The Contract compiler lowers `position: relative` onto a box that clips, scrolls, transforms or animates and names no position, and refuses an authored `position: static` there, or a bound `position` that could take that value (`contract/lower/src/tags.rs`, `CONTAINS_ABSOLUTE`). A literal `visible` or `none` makes no containing block.
- The kernel lowers a static root to `relative`, as the web hosts give the root element `position: relative`: it is the containing block of last resort on every host, and its insets apply.
- So no absolute box ever escapes a box that a native host clips or scrolls, and a box a browser makes a containing block while it animates is one at rest too. The rule is decided once, at compile time, from the attributes a box has (literal or bound), so every host and the ahead-of-time JS build see the same literal row.
- A transform or a filter makes a containing block in CSS as well. `overflow` and motion at rest do not; that is the declared deviation (LLP 1001 §5).

**D2. Paint order does not change.**
- The kernel paints in tree order and native hosts follow it. That was already so, and it is already declared.
- The web host's rule made a static box `position: relative` where it would otherwise paint under something positioned before it. It now makes that box `isolation: isolate`, which paints in the same order and is not a containing block. Chrome confirms both halves (§2).
- The parts of the old rule that existed only because every kernel box was a containing block are gone: no `relative` over an absolute child, none for insets or a `z-index`.

**Also decided:**
- A static box's insets do nothing. Its `z-index` applies only when it is a flex or grid item, on the web by CSS and on Apple by `usedZIndex`.
- The root is the fallback containing block. The web hosts give the root element `position: relative`.
- `fixed` and `sticky` are not added. `fixed` inside a scroller needs the view re-parenting D1 avoids.
- Hoisting is inside Taffy, with the kernel keeping the record of which box contains which (the hybrid Astra proposed). Route B is dead.

**Built, all on `lane/taffy-css`:**

| Where | What |
|---|---|
| Taffy patch 18 | One solver for an absolutely positioned box, shared by block, flex and grid |
| Taffy patch 12, extended | The ratio for flex and grid containers, grid items, absolute boxes and the root |
| Taffy patch 19 | A root is a block-level box in its offer: margins, auto margins, the ratio |
| Taffy patch 20 | `Position::Static`, static positions, the containing-block seam |
| Kernel | The schema value and default; the record of containing blocks; replay boundaries; gates for text flow and content regions; the root-width rewrite removed |
| Contract compiler | The D1 lowering; `static` rows in a virtualized list |
| Web hosts | The isolation rule; the root element positioned |
| Apple presenters | `usedZIndex` |
| Messages | `position: relative` on 29 boxes (§3.5) |

## 0.1 Disposition after review (r2)

Astra (`gpt-6-astra`, reasoning max) and Grok (`grok-4.7`, xhigh, served as `grok-4.7-build`) reviewed r1 blind to each other, from one brief, as static source audits. Both said the direction was right and the draft needed revising.

**Corrections to r1, each taken:**
- **T3 was not a gap.** Upstream 0.14 already gives an auto-width replaced element its intrinsic width in block flow, and a kernel test pins it. LLP 1001 §5 still declared the deviation; r1 copied it without running the case.
- **T1 is not mainly a Taffy change.** The web does not run Taffy. Its paint rule compensates for native tree-order painting. Answered by D1 and D2.
- **Route B's premise was false.** Laying an absolute child out against its parent does not yield its static position, and overflow already unioned into an ancestor cannot be subtracted.
- **The cache risk is traversal, not the key.** Patch 11 already keys on the parent size. The hazard is a change that is laid out without the containing block being laid out. Answered in §3.4; the differential found one such case.
- **Windowed lists do not place rows absolutely.** What `static` touches there is the replay-boundary predicate.
- **`static` changes more than absolute children.** A box with an inset or a `z-index` and no `position` moved before and does not now. The Messages sweep covers both kinds.
- **r1's CSS was wrong in three places.** A static flex or grid item takes `z-index`; `overflow` makes no containing block in CSS; a grid that is the containing block places an absolute child in its grid area.
- **T4's explanation was wrong**, and the fix is CSS's equation, not a swap of one variable.
- **T5 blocks nothing.** Auto-height flow shipped in LLP 1043.000 stage 2 as a bounded re-layout.
- **T6 is medium, not small.**
- **T2 is not uniformly small.** Built anyway, grid items included.

**One reviewer claim the measurements did not bear out:** Grok held that CSS places an absolute child of a flex or grid container at its hypothetical in-flow position. Chrome aligns it in the content box as the container's only item, which is what Taffy did (§2).

**Where the reviewers differed, and what r3 did:**

| Question | Astra | Grok | r3 |
|---|---|---|---|
| Does `fixed` land with T1? | Design for it, ship later | Yes | Not added (D1) |
| A third route for T1 | Kernel-owned bookkeeping, Taffy's solver | None | Astra's, built |
| A block-only spike | Useful, not sufficient | Misleading | All three algorithms built together |
| Upstreaming | Separate maintenance work | Not in the tranche | Patches are written to be sent; not sent |

## 1. What this is based on

- The vendored source, `vendor/taffy` (0.14.0, commit `77f38568`), and its patch inventory.
- Chrome 154, headless, 2026-09-30: 358 cases as plain HTML and CSS, by the method of `kernel/tests/it/browser_cases.rs`. They are the two fixtures of `kernel/tests/it/browser_position.rs`, which holds the kernel to them.
- A paint-order probe in the same Chrome, read with `elementFromPoint` (§2).
- The kernel's incremental-against-fresh differential (`layout_equality.rs`), now drawing positions, over 1,500 seeds.
- Two blind reviews (§0.1).

## 2. Measured

**Layout.** Each case is one line of a fixture: the root's declarations, the nodes, and Chrome's frames.

| Fixture | Cases | What they cover |
|---|---|---|
| `browser_position.tsv` | 279 | Absolute boxes in block, flex and grid containers (insets, auto margins, overflow, RTL, ratios); a flex or grid container's own ratio under four kinds of parent; grid items' ratios; roots' ratios and margins. Every box is positioned |
| `browser_containing_block.tsv` | 79 | Real `static`: which ancestor contains an absolute box, what else makes a containing block, static positions in block, flex and grid, static and relative insets |

The ticket sweep added 40 Chrome cases, including ratio automatic minima and
percentage children, logical alignment, and absolute roots (416 total; the position fixture has 297 cases). The five
previously owed ratio cases now pass. The remaining seven exceptions are what else makes a containing block in a browser (a transform, a filter, `backdrop-filter`): the kernel's rule is position alone, and the compiler lowers `relative` onto such a box (D1).

What Chrome said that shaped the design:

| Question | Chrome 154 |
|---|---|
| Does `overflow`, `opacity`, `z-index`, `isolation`, or being a flex or grid container make a containing block? | No |
| Does `translate`, `scale`, `rotate`, `transform`, `filter`, `backdrop-filter`? | Yes, identity values included |
| Static position in a static flex container | Aligned in the content box by `justify-content` and `align-self` |
| Static position in a static grid | Aligned in the content box |
| Static position in a grid that is the containing block | Aligned in the padding box (the grid area) |
| An absolute box with both block insets, taller than the space, `margin: auto 0` | Centred: equal negative margins |
| The same in the inline axis | Start-aligned: the end margin takes the negative space |

**Paint order.** A 100×100 absolute box, then a 100×100 static box over it, inside a positioned container:

| The static box | On top |
|---|---|
| plain | the absolute box (CSS paints the positioned later) |
| `isolation: isolate` | the static box (tree order) |
| `position: relative` | the static box (tree order) |
| `isolation: isolate`, holding an absolute child | the child is placed against the outer container, not the isolated box |

`z-index` on a static block child does nothing; on a static flex item it applies.

## 3. T1: `position: static` and the containing block, as built

### 3.1 Taffy (patch 20)

- `Position::Static`: in flow, insets ignored, contains nothing. Upstream's default stays `Relative`, so a tree that never says `Static` is unchanged.
- A container that is `Static` does not lay an absolute child out. It records the child's **static position**: a rectangle in its own coordinates and an alignment in it, which is all its algorithm knows that the containing block does not.
- A container lays out, after its in-flow children, the absolute boxes it contains that are not its children. It asks the tree for them, solves each against its padding box, and writes the location relative to the box's parent, because a layout's location is parent-relative.
- One solver (patch 18) sizes and places every absolute box, for all three algorithms.

### 3.2 Kernel

- `PositionType` gains `static`, the default. The value is appended, so the wire's existing numbers keep their meaning.
- `LayoutTree` keeps the set of absolute boxes. When a position or a child list changed since the last layout, it rebuilds the record of which containing block holds which box, and hands it to Taffy. The cost is the number of absolute boxes times their depth; a layout where nothing of the kind changed pays nothing.
- A static box at the top of a tree holds what nothing else does, so the root needs no special style.

### 3.3 Compiler, hosts

- **Compiler:** D1's lowering, in `contract/lower/src/lib.rs` beside the other attribute rewrites. A `scroll` or `list`, a `canvas` and a Markdown editor are included by tag.
- **Web hosts:** D2's rule. `Paint.positioned` reads the row's value. `holds_absolute` and the insets rule are deleted, with the kernel query that served them.
- **Apple:** `usedZIndex` on both presenters.
- **Linux:** no change; it does not read `z-index`.

### 3.4 Incremental layout

The hazards the reviewers named, and what holds each:

| Hazard | What holds it |
|---|---|
| A containing block resizes; a static box between it and the absolute box is clean and cached | The containing block's pass places the box itself, from the recorded static position. The static box is never asked |
| The static parent moves; the containing block does not | The pass recomputes the parent's origin from the layouts between them every time it runs |
| A box changes containing block | It happens only when a box on the path between the two is restyled or re-parented, which marks both dirty |
| A replay boundary below the containing block | A static box on the path is no boundary. Neither is one that was on the path at the last layout, for the change that took it off: the old containing block still counts the box that left |
| A clipping box that is static | It stays a boundary unless an absolute box is contained through it, so lists keep their locality whether or not the compiler positioned them |

The last-but-one row was found by the differential, not by design: seed 3 of the pane run left a root's scroll extent 16 px too wide after an absolute box under a static pane was destroyed.

### 3.5 What moved in apps

A temporary compiler audit (not committed) listed every absolute box under a static parent and every inset or `z-index` on a static box, across every `.contract` file in the repo.

- **Messages** was the only app with findings: 29 boxes got `position="relative"`, which keeps each where it was.
- Every other app already wrote `position: relative` on its containing blocks.
- Two fixtures place absolute boxes under the root element, which contains them as before.
- Apps outside the repo (Weird Castle, Interview) were not swept. The rule for them: give `position: relative` to the parent of each absolute box, and to each box that has an inset and no position.

### 3.6 Two gates that assumed the parent contains

- **Text around shapes:** auto-height flow is admitted only where the exclusion's parent is its containing block (the root, or positioned). Elsewhere the leaf is refused with the existing `Context` refusal, whose message now says so.
- **Content regions:** a region whose content or placeholder is absolutely positioned needs a positioned owner. A trial lays the owner out as the top of its own tree, where it contains them, and the ordinary tree must agree.

## 4. T2 and T4, as built

**The solver (patch 18).** Upstream sized and placed an absolute box three times and the three disagreed. Against Chrome:
- a grid ignored auto margins on a box with both insets;
- a flex container gave auto margins the container's free space, not the space the insets leave;
- a block container centred a box that had only a `right` inset;
- no container centred, in the block axis, a box taller than the space;
- a box without a width was measured in the whole containing block, not the space its insets and margins leave;
- none sized through the ratio as patch 12 does.

**The ratio (patch 12, extended)** now covers a flex or grid container's own size, grid items (their size, their known dimensions during track sizing and their minimum contribution), absolute boxes and the root.

The solver also subtracts a static inline inset before shrink-to-fit sizing and
uses the CSS min-content / available / max-content clamp (2026-09-30 ticket sweep).

**The root (patch 19).** A root is a block-level box in its offer whatever it lays its children out as. Its margins are subtracted from an automatic width and place it; auto margins centre a root narrower than its offer; its size goes through the ratio. An absolute root uses the shared solver against a definite offer. The kernel's rewrite of a root's `width: auto` to `100%` under border-box sizing is deleted, with the three deviations LLP 1010 declared for it.

**Ratio follow-ups, built 2026-09-30:** an automatic inline minimum uses the
min-content width when a non-replaced box derives its width from a definite
height, respecting `min-width`, `max-width` and scroll-container opt-outs.
A ratio-derived height is the percentage basis even when content makes the used
height larger. Block and flex layout keep those two quantities separate; the
five formerly owed cases and content-overflow cases pass against Chrome.

## 5. Already in Taffy, not exposed

These need no new layout algorithm. They do need schema rows, Contract grammar, validation, serialization and CSS emission. Read from source; not run.

- **`grid-template-areas` and named grid lines.** The kernel's grid placement is numeric and would need extending. Differential cases first.
- **`safe` and `unsafe` alignment.** LLP 1054.000.001 already proposes `safe center`.
- **`start`, `end`, `self-start`, `self-end`: exposed 2026-09-30.** Item/self alignment accepts all four; content alignment accepts `start` and `end`. Schema-generated validation and CSS serialization share the vocabulary; Contract also exposes `justify-items` and `align-content`. Chrome fixtures cover LTR and RTL.
- **`display: flow-root`.** The web host already appends it on a block root.
- **Preferred-size keywords** `min-content`, `max-content`, `fit-content`, `stretch`.
- **`overflow: clip`, `justify-self`, implicit grid track sizes.**

`display: contents` is not here: Taffy's `BoxGenerationMode` has `Normal` and `None` only.

## 6. Missing from Taffy, on demand

Found by Astra's review; no app has asked.

- **Intrinsic keywords in `min-` and `max-` sizes** (`min-width: max-content`).
- **CSS `order`.** It can be done at the kernel's traversal boundary without a new algorithm.
- **T5, stable block offsets at measure time.** Worth doing only to lift LLP 1043.000 stage 2's restrictions or to remove passes.
- **T6, `last baseline`.** A second metric from every text engine, propagation through containers, and one patch 9 assumption to revisit.

## 7. Not proposed

- **Inline layout** (`display: inline`, `inline-block`, `inline-flex`, anonymous boxes). The largest CSS gap in layout. It belongs at the text-engine seam and needs its own RFC.
- **CSS paint order and whole-context `z-index` on native.** This is what would let the `overflow` deviation go: a host that paints by stacking context and re-parents a positioned view to its containing block. It is a presenter rewrite on three hosts.
- **`position: fixed` and `sticky`.** `fixed` needs the above. `sticky` is scroll-time placement on each host.
- **Floats.** Disabled in the vendored build on purpose.
- **Subgrid, vertical writing modes, `visibility: collapse` in flex and grid, tables.**

## 8. Verification (r3, 2026-09-30, on `lane/taffy-css` at its tip)

| Check | Result |
|---|---|
| The five checks | build, test, clippy, fmt, caps and boot pass |
| `cargo test --workspace` (Expose's three crates excluded: their TypeScript does not type-check without `EXPOSE_OPENROUTER_KEY`) | one failure, `exact-js`'s 503-deadline test, which fails at `origin/main` on this Mac too (`QUEUE.md`) |
| `layout_equality.rs`, both differentials, 1,500 seeds × 40 rounds | 0 failures |
| Before and after, the Linux host: every app's settled boot layout, and seven Messages screens, base `9fdf7e564` against the lane, frames by node | 0 of 3,600 frames differ |
| Before and after, the web (JS target): 33 app boots and 10 Messages screens, frames and screenshots | frames identical; screenshots byte-identical but two: 40 anti-aliased edge pixels of a rounded avatar in the emoji picker (Δ ≤ 8/255, deterministic) and 3 pixels in sparkline |
| Before and after, macOS: Messages inbox, a conversation, the new-message sheet, search, contact details | frames identical, screenshots byte-identical |
| Web conformance (`conform.mjs --synthetic --linux --strict`, 23 apps) | passes, but photo-editor's two rotate screenshots, which fail on base the same way (a running rotation compared mid-flight) |
| Smokes | Linux, web, macOS and iOS (an iPhone 18 Pro simulator) pass |

Not verified: the UIKit XCTests; apps outside the repo.

## 9. The code review (r4, 2026-09-30)

After the landing, Astra (`gpt-6-astra`, xhigh) and Grok (`grok-4.7`, xhigh) reviewed `d8a4a0ebf` blind, from one brief, as source audits (`llp/reviews/code-2026-09-30-taffy-css.{astra,grok}.md`, each with a disposition at its end). Both found the same two defects: a hoisted absolute box under a static ancestor that becomes `display: none` was laid out again from the record its parent kept while visible (the kernel's differential now proves the fix against a fresh tree, and fails at `d8a4a0ebf`), and Apple read a static flex or grid item's `z-index` before the view had a parent. Fixed with them: a region's owner must be positioned; a bound `position` on a box that contains is refused unless every value it can take is positioned, and a literal `visible` or `none` makes no containing block; a static root is lowered `relative`; one auto margin on an over-constrained root is zero; a flex or grid root shorter than its ratio's height is laid out again at that floor (the fixtures now assert root frames); `contextTarget` contains; the stale LLP 1010 sentence and QUEUE gap line. The rest is filed under `issues/20260930-*` (nine tickets from the review, plus what §4 and §8 already owed and a pre-existing hidden-subtree bug the extended differential found).
