# LLP 1093: Text that flows across boxes — CSS multi-column, and the break rules that only mean something with it

**Type:** RFC
**Status:** Draft r2 (round 1 of 3). r1 was reviewed twice by Grok 4.7 (xhigh), both one family because the Codex/Astra budget was exhausted, with two scopes: CSS fidelity against Chrome 154 (`llp/reviews/1093-r1.grok-a.md`) and kernel and host implementation (`llp/reviews/1093-r1.grok-b.md`). Both NOT READY. r2 resolves every finding but one fix, A1's, which is rejected on its own evidence (§9). Admitted (orchestrator for Charlie, 2026-10-05) by the trade §8 Q1 offered, recorded in `rules/DEFERRED.md`.
**Systems:** Kernel (`schema.json` rows; a new `kernel/src/fragment.rs`; `TextMeasurer::lines`; `Kernel::fragments`), vendored Taffy (one patch), Contract (lowering), Apple host (`exact_set_lines`, a `fragments` op, fragment layers), Linux host (new `fragments.rs`), web hosts (none: the browser does it), Agent (LLP 1012 `layout`, `tap`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Revised:** 2026-10-05 (r2)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-05, stage 2 on 2026-10-06 (§6)
**Related:** the reader's diary (`~/projects/x2apps/reader/DIARY.md`, Rough and Top 5 item 4); the typography lane (`fix/typo` `0b56caef4`, which refuses these rows by name, and its QUEUE line, whose recommendation this takes); LLP 1001 §5–§6; LLP 1043.000 D4–D7, §8; LLP 1051.000 (`frame()`); LLP 1083; `vendor/taffy/EXACT-PATCHES.md`; the element resize event (`resize=action`, its own document, landing before stage 2). External, read 2026-10-04: CSS Multi-column Layout 1 §3–§7; CSS Fragmentation 3 §3–§5; Chrome 154.0.8037.98's block fragmentation (break appeal, column balancer), measured by the reviews and re-measured for r2.

## Summary

The reader app pages a chapter without fragmentation. It lays the chapter out as one tall column a whole number of lines per page, translates it a page at a time, polls `frame()` on a timer for its height, and renders the chapter twice, with renamed ids, for a two-page spread. It cannot keep a paragraph's last line off the top of a page.

CSS's answer is multi-column layout with a fixed height: content flows through columns of one width, and columns past the box's width overflow sideways, one per page, which is how browser e-book readers page. This document admits it:

```
view width=pageW height=pageH overflow="hidden"              // the window
  view id="flow" height=pageH column-width=pageW column-gap=gap column-fill="auto"
       translate=`${-page * (pageW + gap)}px 0px`
    …the chapter's paragraphs: text, headings, figures
    view id="flow-end"                                        // its column is the page count
```

The design rests on a property of multicol that regions and exclusions lack. **Every column has the same width, so a paragraph breaks into lines once, at that width, whatever column it lands in.** Fragmenting does not change any line. It only chooses which lines go in which column. So the kernel:

- lays the content out once, as a single column (the *flow thread*), with ordinary Taffy and one patch;
- walks it in document order and cuts it into column-height intervals at legal break points, applying `widows`, `orphans` and `break-*` as Chrome does;
- publishes every box translated into its column, and a straddling box's fragments.

The native hosts already paint lines at per-fragment origins for text around shapes (LLP 1043.000), so painting is a generalization of what exists. The web emits the CSS and the browser does all of it. Chrome is the oracle.

## 1. What CSS has, and the subset taken

CSS fragmentation has three kinds of container: columns, pages (print, `@page`) and regions (withdrawn, shipped nowhere). Only columns exist on screen in every browser. The break rules mean nothing outside a fragmentation container, which is why the typography lane refused them with multicol.

**Admitted:**

| Row | Values taken | Initial |
|---|---|---|
| `column-count` | `auto`, a positive integer | `auto` |
| `column-width` | `auto`, a length | `auto` |
| `columns` | the shorthand of the two | |
| `column-gap` | the existing `f32` row; `normal` clears it (below) | `normal` |
| `column-fill` | `balance`, `auto` | `balance` |
| `column-rule-width` | a length; `thin`, `medium`, `thick` as 1, 3, 5 px | `medium` |
| `column-rule-style` | `none`, `hidden`, `solid` (the `BorderStyle` the hosts paint) | `none` |
| `column-rule-color`, `column-rule` | a color; the shorthand | `currentcolor` |
| `widows`, `orphans` | a positive integer; inherited | `2` |
| `break-before`, `break-after` | `auto`, `avoid`, `column`, `avoid-column` | `auto` |
| `break-inside` | `auto`, `avoid`, `avoid-column` | `auto` |

`column-gap` stays an `f32` defaulting to 0, so flex and grid are unchanged. An unset bit on a multicol container computes to 1em of that element's own `font-size` (Chrome: a 16px multicol inside a 40px parent has a 16px gap); an authored 0 stays 0, and the web emits nothing for an unset bit.

**Refused by name at lowering, each with what to write instead:**

- `column-span: all`. Only `none` exists, so the row is not added.
- `balance-all` and the page and region break values: `page`, `left`, `right`, `recto`, `verso`, `always`, `all`, `region`, `avoid-page`, `avoid-region`.
- The rule styles no host paints: `dashed`, `dotted`, `double`, `groove`, `ridge`, `inset`, `outset`. Chrome would paint them on the web and the native hosts would not.
- Multicol rows on a flex or grid container (Contract's `column`, `row`, `grid`). CSS ignores them there, so accepting them would be a silent no-op. The diagnostic says to write `view`.

## 2. Decisions

**D1. A multicol container is a block container with `column-count` or `column-width` other than `auto`.** That means `view`, a `text`, or any tag whose `display` is `block`.

- It establishes an independent formatting context: its first child's `margin-top` stays inside it (Chrome: 30px below the container's top).
- A `text` that is a multicol container is one paragraph whose lines flow through its columns.
- The used count N and width W are CSS Multicol §3.4's pseudo-algorithm over the content-box width U, the two rows and the gap (for `column-width: w`, N = max(1, ⌊(U + gap)/(w + gap)⌋), W = (U + gap)/N − gap).
- Columns progress from the inline-start edge. Column i's left edge in the content box is i·(W + gap) under `ltr` and U − W − i·(W + gap) under `rtl` (Chrome, three columns of 100 with a 10px gap in 320: x = 220, 110, 0). Columns beyond N are overflow columns. They are scrollable overflow, clipped by `overflow` as anything else is.

**D2. The flow thread is laid out by Taffy, once, at width W.** This takes one vendored-Taffy patch, the next number in `EXACT-PATCHES.md` (Exact's: upstream has no multicol). A block container whose style carries `multicol: Some { count, width, gap, used_height }`:

- sets `establishes_new_bfc` (`vendor/taffy/src/compute/block.rs:516`), before margin collapsing is decided;
- once its content width U is known, computes W, and lays its in-flow children out with W as their available width and as the basis for percentage widths (Chrome: `width: 50%` is 100px in a 400px two-column container);
- gives children a percentage-height basis only when its own height is definite before the flow thread is laid out (`height: 50%` in a 200px-tall multicol is 100px). Under an auto height the basis is indefinite (Chrome: an 18px box in a 59px container);
- sizes itself from `used_height` when the kernel has written one (D5). `used_height` is never a percentage basis, so writing it does not change the children's inputs, and Taffy's cache keeps them.

Every container without the field is unchanged. That is held by the 512-tree incremental-against-fresh comparison Patch 7 left behind and by the existing fixtures.

**D3. The cut is a walk over the flow thread in document order, in the kernel** (`kernel/src/fragment.rs`; `layout.rs` is at 1,492 of its 1,500 lines). It produces intervals [s₀, e₀), [s₁, e₁), … in flow-thread y, each at most the column height H. Column k shows the content in its interval, translated by (column k's left edge (D1), −sₖ). Translation replaces CSS's pagination struts, and nothing is re-laid out.

The break points are CSS Fragmentation §4.1's:

- *Class A*: between siblings in a fragmentable box.
- *Class B*: between two line boxes of a paragraph.
- *Class C*: between a fragmentable box's content edge and its first or last child.

**Forced breaks propagate** (Fragmentation §3.1.1). `break-before` on a first in-flow child and `break-after` on a last one move outward to the parent's own edge, up to the multicol container. Chrome moves a parent with `padding-top: 30px` whole into the next column when its first child has `break-before: column`, so the padding goes with the child.

*Fragmentable boxes* are:

- block containers whose `overflow` is `visible` or `clip`, with `height`, `min-height` and `max-height` all `auto`;
- single-line column flexboxes on the same conditions (Contract's `column`, the commonest container). With an auto height a column flexbox has no free space, so its items stack like blocks without margin collapsing (Chrome: `justify-content: space-between` distributes nothing).

*Monolithic in Chrome too, so the kernel must not fragment them:* replaced elements (images, video, canvas), form controls, and boxes whose `overflow` is neither `visible` nor `clip`. *Monolithic in the kernel only*, declared in D10: everything else that is not fragmentable.

A monolithic box that does not fit is pushed whole to the next column. One taller than a column starts it and overflows (Chrome: a 180px image in a 100px column is one rect; the next sibling starts the next column).

**Margins, and gaps, at breaks** (Fragmentation §5.2):

- An unforced break truncates both adjoining margins to zero. The next interval starts at the next box's border edge (Chrome: margins of 50 and 10 put both boxes at their columns' tops).
- A forced break truncates the margin before it and keeps the next box's own `margin-top`, not the collapsed one. With 50 and 10, Chrome puts the next box 10px down.
- A column flexbox's gap at a break between items truncates like an unforced margin (Chrome: item C starts the next column at its top).

Every column takes at least one line or one monolithic box, so the walk always makes progress.

**D4. The cut rule, as Chrome breaks.** Chrome's terms, not CSS's. Each candidate gets an *appeal*, from best to worst:

1. *perfect*;
2. *violates `orphans`/`widows`*;
3. *violates `break-*: avoid`*: a Class A point beside an `avoid` or `avoid-column`, or any point inside a box with `break-inside: avoid`;
4. *last resort*: inside a line or a monolithic box.

When the next line or monolithic box would cross the interval's bottom, the walk breaks at the latest candidate of the best appeal seen since the interval began. A forced break (`column`, propagated as in D3) ends the interval there, whatever fits. Chrome prefers a widows violation to a `break-after: avoid` violation: a figure after an `avoid` paragraph stays whole and in one column.

How `orphans` and `widows` score a break inside a paragraph of L lines, as Chrome 154 does it:

- **Fewer than `orphans` lines fit in the interval:** the break goes before the paragraph. An 80px pad in a 100px column leaves room for one line, and the paragraph starts the next column.
- **`orphans` or more fit, and L ≥ `orphans + widows`:** a break leaving fewer than `widows` lines after it is a violation. The walk takes the latest perfect break instead. With six lines and `widows: 4`, an 80px column breaks after line 2.
- **`orphans` or more fit, and L < `orphans + widows`:** `widows` does not score, and the paragraph splits at the last line that fits. With a 40px pad and three lines in an 80px column, two lines stay and one starts the next column, although a perfect break before the paragraph was available. Five lines with `orphans: 3; widows: 3` in a 60px column split 3 + 2.

Every case above is a fixture (§4), measured before code. A pattern the fixtures do not pin, such as a middle fragment of a long paragraph under both rules, gets a Chrome case added first and is not guessed.

**`widows` and `orphans` default to 2.** Chrome therefore applies them to every multicol paragraph that nobody styled, so parity needs them from the first commit. This is the reader's missing rule: a paragraph's first line never ends a page alone.

**D5. Column height and `column-fill`.** The two values have four cases:

| Height | `column-fill: auto` | `column-fill: balance` |
|---|---|---|
| definite `h` | columns of `h`, filled in turn, then overflow columns | balanced within `h`; the container stays `h` |
| `auto` | one column as tall as the content, every child at x = 0, no second pass | the container is the balanced height |
| `max-height: m` | as `auto` while the content fits under `m`; past it, columns of `m` filled in turn, then overflow columns | balanced, capped at `m`; past the cap, overflow columns of `m` |

Chrome's own results are the table's evidence: six 20px blocks with `auto` under an auto height make a 300×120 container with every block at x = 0. Four 20px blocks under `max-height: 200` make a 40px container with `balance` and an 80px single column with `auto`.

**The balancer is Chrome's.** It starts at the content's height over N, no lower than the tallest unbreakable piece. It runs D4's cut at that height. While the cut needs more than N columns and H is below its cap, it stretches H by the least space shortage the cut reported, and it stops when H no longer grows. There is no N − 1 bound: when the shortage is one line, each walk moves one line. The shortage includes the lines D4's earlier perfect break pushes on. That is why six lines with `widows: 4` in two columns balance to 80px (2 + 4), not 60px (3 + 3): at 60, the perfect break after line 2 needs a third column. Eight lines with `orphans: 3; widows: 3` in three columns stay at 60px (3 + 3 + 2), because that split fits in N columns. No split there is perfect, so stretching stops.

Each walk reruns the cut over geometry already laid out, with no re-measure and no Taffy pass. With `balance` under an auto or `max-height` height, the kernel writes the balanced H into the container's `used_height` (D2) and lays out once more. Only the container and what follows it move. H depends on the flow thread, the flow thread depends on W, and neither depends on the container's height, so the second pass is a fixed point. A fresh replay reaches the same frames, which `layout_equality.rs` holds with multicol trees. `used_height` lives in the kernel's fragmentation state and is applied by `taffy_style`, so `measure_auto_height`'s restore (`kernel/src/kernel/geometry.rs:57–75`) puts it back. `measure()` on a multicol container runs the same cut and answers the balanced height.

**D6. Lines come from the host's engine, on request.** `TextMetrics` stays a size and a baseline. `TextMeasurer` gains a method with a default that returns nothing, in which case the paragraph is monolithic:

```rust
fn lines(&mut self, stamp: &ParagraphStamp, request: &TextMeasureRequest<'_>, bottoms: &mut Vec<f32>) {}
```

It returns each line box's bottom in content coordinates, for the request the measure just answered. The kernel asks only for a paragraph that straddles a cut, and never for one whose `paragraph.markup` is Markdown (D10). Each host answers from the paragraph it just built and retains:

- Apple from `Paragraph.lineBottoms`, through a hook registered by a new `exact_set_lines` in `host/apple/src/abi/exports.rs` (586 lines) and called by `CallbackMeasurer`. `EXACT_ABI_VERSION` (`host/apple/include/exact.h:33`) moves.
- Linux from its buffer's layout lines.
- `MonospaceMeasurer` by arithmetic.

Measured is cut is painted, because it is one paragraph.

**D7. What the kernel publishes.** Fragmentation stays sparse, like 1043.000's resolved exclusions. It is a kernel record, `Kernel::fragments(key)`, read the way `Kernel::sticky_constraint` is read:

- Every box in a multicol container's flow is published at its translated frame. A box wholly inside one column needs nothing new from a host.
- A box that straddles cuts is published with its **union** frame, which is what `getBoundingClientRect` returns (Chrome: two client rects at x = 0 and 220 have a 420-wide union) and so what `frame()` answers on the web.
- Each such box also gets a list of fragments, one per column it touches. A fragment is a rectangle in the box's coordinates, sliced as CSS `slice` does: the first keeps the top edge, the last the bottom, and each one before a break runs to its column's end. A paragraph's fragment also carries its line range and an (dx, dy) to add to those lines' unfragmented positions.
- The container's scrollable overflow is the fragmented extent: through the right edge of its last column, and H tall. Publication's `set_content` (`kernel/src/layout/publication.rs:92–97`) gets that, never the flow thread's height. Hosts grow scroll extents from that field (`host/apple/src/host.rs:1344`, `host/linux/src/paint.rs:1439`). The fixture is Chrome's `scrollWidth` of 260 for three 80px columns with a 10px gap. It uses `scrollbar-width: none`, as LLP 1083's did.
- The container is published with its column rectangles, and with which of them hold a box. A zero-height box from a forced break counts as holding one.

A host draws `column-rule` centred in each gap between two columns that both hold a box, as tall as the columns. A column with no box gets no rule. A rule wider than its gap stays centred on the gap, moves no column, and paints under the columns' contents (Chrome: a 12px rule in a 4px gap shows only in the gap).

On Apple these reach the batch as a `fragments` op beside `sticky`, the presenter's encoding of the record. `LayoutReceipt` names every box whose translated frame changed. That can be every later box when an early paragraph grows, as in Chrome.

**D8. Each host paints and hits the fragments it was given.** The files that would hold this are at the line cap, so each host's part goes in new files called from the existing functions:

- **Linux, stage 1.** A new `host/linux/src/fragments.rs` covers three things. It paints a paragraph's buffer lines at each fragment's offset, and a rule as a rectangle. It produces the `layout` reply's fragments. It tests hits against fragments. `paint.rs` (1,498 lines) and `presenter.rs` (1,488) only call it. `hit` and `tap` accept a point only inside a fragment, never anywhere in a union.
- **Apple, stage 2 (macOS and iOS).** A fragmented paragraph's view holds one layer per fragment, sized to it, drawing its line range at the fragment's offset, so no paragraph backs a two-page layer. Column rules are thin layers on the container. This lives in new files beside `NodeViewMac.swift` (1,480) and `NodeViewIOS.swift` (1,479). Their `hitTest` overrides (`NodeViewMac.swift:668`, `NodeViewIOS.swift:722`) call one line into it, answering only inside fragments. Selection (`TextSelectionMac.swift:293–310`, and iOS's) picks the fragment containing the point and inverts its (dx, dy) into content coordinates. The existing `lineIndex` then applies, so `selectionchange` holds across columns.
- **Web (JS target and wasm host).** The rows reach CSS by their own names; the browser lays out, breaks and paints, and the kernel's cut never runs.

**D9. The web is CSS multicol itself, and Chrome is the oracle.** Firefox has never implemented `widows` and `orphans`, and Firefox's and WebKit's balancers are their own. `conform-firefox` and `conform-webkit` record those cases as browser differences, not exact2's. Unlike `wrap-flow` (LLP 1043.000 D1), nothing executes multicol for the browser: every engine ships it.

**D10. Declared deviations, journalled once per box with what to change.** This is the `FlowRefusal` pattern of LLP 1043.000, recorded in LLP 1001's declared deviations. Chrome fragments each of the following, and the native kernel does not:

1. **A box with a definite `height` or `min-height`** is monolithic. Chrome slices it (a 200px box in 150px columns is 190×150 and 190×50; `min-height: 300px` spans three columns). **A box with `max-height`** is monolithic too. Chrome keeps its border box at the cap and fragments its overflowing children. The message for both says to use an auto height.
2. **Row and wrapped flexboxes, grids, and a nested multicol container taller than its column** are monolithic. Chrome fragments them; a nested multicol that fits is one rect in both.
3. **A box with its own decoration** that would fragment is monolithic until hosts slice decorations (§5, deferred). Decoration here means a background, border, radius, shadow, gradient, filter or clip-path. Chrome slices it. A highlight is an inline run's background, painted with its line, and unaffected.
4. **An absolutely positioned box whose containing block lies inside the flow** is placed unfragmented, translated with its containing block's first fragment. A box whose containing block is the multicol container itself is placed against the container, as CSS does.
5. **A multicol container whose width is sized from content** (shrink-to-fit in a flex row) takes its content's intrinsic widths as one column. W is not solved from an intrinsic U.
6. **Markdown, exclusions and virtualized lists.** One-node Markdown (LLP 1045) is monolithic. Exclusions (`wrap-flow`) and virtualized lists inside a flow are refused. `position: sticky` inside a flow behaves as `relative`.
7. **Fixed-height items in a column flexbox are pushed whole, where Chrome slices them.** This is item 1, inside a flexbox. A column of fixed-height items will not match Chrome until it is lifted. A gap before an item Chrome slices through is kept there; in the kernel the item moves with its gap truncated.

**D11. How an app pages: by `translate`, and by reading where the flow ends.** There is no page-count fact and no new `Geometry` field. A spread is `column-count=2` on a container two pages wide, and each turn moves it by the container's width plus a gap. The page arithmetic is all `frame()`, read in the element resize event's action on the flow (`resize=action`, its own document, landing before stage 2):

```
end   = frame("flow-end")                          // an empty block after the last paragraph
flow  = frame("flow")                              // the flow's translate moves both reads alike
pages = floor((end.x - flow.x) / (pageW + gap)) + 1
page  = floor((frame(para).x - flow.x) / (pageW + gap))   // keep the place on resize
```

`frame()` places every box through the same transforms (`getBoundingClientRect`'s box, LLP 1051.000 D1 as changed 2026-10-04), so the flow's own `translate` cancels in each difference, and a straddling paragraph's union starts in its first column, so both reads hold on every host. Under `rtl`, measure from the right edges instead. The reader's page-turn animations keep working, because a turn moves the container by its transform and that never re-lays anything out (LLP 1002 D2).

Not taken:

- `scroll-snap` across columns. Columns are not boxes, CSS reaches them only through `::column`, and Contract has no pseudo-elements.
- `scrollWidth` on `Geometry`. It would be the first overflow read, and nothing else asks for one.

**D12. The agent sees fragments.** `layout <node>` prints a container's columns and a box's fragments on every host (on the web, from `getClientRects`). `tree` shows union frames. A `tap` aims inside a node's first fragment, never at a union's middle, which may be a gap. That is the diary's inline-run fix, made for fragments.

## 3. Performance

- **Nothing without multicol.** Publication takes one branch per node, as 1043.000 met for exclusions, and Taffy one `None` check per block layout.
- **The flow thread costs what the reader pays today** for its one tall column, and a spread builds half the nodes it builds now.
- **The cut is O(boxes in the flow), with a line query per straddling paragraph.** Apple answers the query from the retained `Paragraph`. The walk resumes at the column holding the earliest box whose flow frame changed. Balancing costs one walk per stretch, and the worst case is one walk per line moved.
- **A page turn is a transform:** no layout or repaint. Hosts cull off-screen layers by frame, as they do now.
- **Owed by stage 1**, in §7, from an ignored probe in the async lane (no metrics line, no check):
  - the cut for a 40-column, roughly 400-paragraph chapter;
  - that chapter's layout against the same chapter as one column, with a target of +10%;
  - a balanced 3-column, 400-paragraph container's walk count;
  - a one-word edit in the last column.

## 4. Tests

- **Chrome fixtures.** `kernel/tests/it/browser_columns.rs` and `fixtures/browser_columns.tsv`, measured in Chrome 154 by `browser_cases.rs`'s method, with each box's fragments from `getClientRects`. Text is `white-space: pre` with a pixel `line-height`, so Chrome's line boxes are exact and `MonospaceMeasurer` reproduces them. Every Chrome result cited in D1–D7 and D10 is a case, about forty in all. The reviews' measurements (`llp/reviews/1093-r1.grok-a.md`) are the first draft of the TSV.
- **Kernel.** `layout_equality.rs` gains multicol trees: incremental equals rehydrated equals fresh replay, with the cut's resume point and `used_height` exercised. Unit tests hold the §3.4 arithmetic, propagation and the appeal ranking.
- **Hosts.** `TextParityMacTests` gains a multicol case: a bundled face, so lines match Chrome's, and each column's first and last words equal Chrome's. The Linux host's `css_tests` gains the same case, plus a fragment hit-test.
- **Contract.** The admitted names lower, and each refusal in §1 and D10 is a diagnostic test. These replace `fix/typo`'s fragmentation refusals in `contract/cli/tests/it/typography.rs`.
- **The reader, driven** with `agent web` and `agent macos`. Its eleven authored tests pass on both with equal page counts, plus one: after a resize, the page shows the same paragraph. None of this adds a check.

## 5. Out, and deferred with preconditions

**Out:**

- Paged media: `@page`, `page`, page margins and printing. exact2 does not print.
- Regions. They shipped nowhere, and boxes of different widths would make a paragraph's lines depend on where it is cut (1043.000's harder problem).
- `box-decoration-break: clone`, `::column` and scroll markers, and `balance-all`.

**Deferred.** Each returns when its preconditions hold, and none is scheduled:

| What | Preconditions |
|---|---|
| Slicing a box's own decorations per fragment (D10.3; the orchestrator kept it here) | stage 2's per-fragment layers; an app that needs a decorated box to split; Chrome fixtures for radius and shadow slicing |
| Slicing fixed-, min- and max-height boxes, and items (D10.1, D10.7) | the cut can end a fragment inside a box with no children to break between; Chrome fixtures exist already |
| Row-flex, grid and nested-multicol fragmentation (D10.2) | an app that needs it; Chrome fixtures for flex lines and grid rows |
| Fragmenting absolutely positioned boxes (D10.4) | an app that needs it |
| Splitting one-node Markdown (D10.6) | the Markdown viewer (LLP 1033) asks; its engine then answers `lines` with block boundaries |
| `column-span: all` | an app that needs it; a second balancing context per column row |
| Dashed, dotted and the other rule styles | `BorderStyle` gains them for borders first |

## 6. Stages

1. **Kernel, Contract, Linux and web, 2026-10-05.**
   - The rows in `schema.json`, and lowering with §1's refusals.
   - The Taffy patch (D2), `fragment.rs` (D3–D5), `TextMeasurer::lines` (D6), `Kernel::fragments` and publication (D7), and the journal (D10).
   - Linux's `fragments.rs`: paint, hit, `layout` (D8, D12).
   - The Chrome fixtures, the equality trees and §3's probe.
   - On the web, CSS emission only.
2. **Apple and the reader, 2026-10-06, after the element resize event lands.**
   - `exact_set_lines`, the `fragments` op, fragment layers, hit testing and selection on macOS and iOS, and column rules.
   - `TextParityMacTests`, and the driver's first-fragment tap.
   - The reader in `~/projects/x2apps/reader`. It becomes one flow, with the spread as `column-count=2`. Its duplicated `r-` ids, its line-grid page height and its 100 ms settle timer are deleted; pages are read in `resize=action` on the flow (D11). Its diary is updated.

## 7. As built

Nothing yet.

## 8. Questions, as decided (orchestrator for Charlie, 2026-10-05)

1. **Admission:** by the trade offered. LLP 1043.000's stage 5 (`shape-outside: <image>`) moves behind stage 2 of this document, recorded in `rules/DEFERRED.md` in its own commit.
2. **When the reader reads:** an element resize event lands before stage 2 and is built separately. This document names its call site (D11, §6).
3. **Markdown splitting** waits for a viewer that asks (§5).
4. **Decorated boxes** stay deferred (§5).

## 9. Revisions

**r2 (2026-10-05).** Every finding of both r1 reviews is resolved except one, rejected with evidence. Each disposition is in its review file.

- *A1's fix* was to keep stretching while appeal improves within N columns. It is rejected, because it predicts 100px (3 + 5, perfect, two columns) for the reviewer's own eight-line case, where Chrome stays at 60. The six-line case it raised is explained by D4's earlier perfect break pushing a third column, which the ordinary stretch then removes. D5 now says so, and both cases are fixtures.
- *The changes:* A2–A9 and B1–B5, B9 rewrote D1–D5 and §1. A10–A13, B7, B8 and B10 changed D7, D8 and D10. B6, B12 and B13 changed D2, D6 and D8. B11 and the orchestrator's decisions changed D11, §5, §6 and §8.
