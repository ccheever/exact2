# LLP 1093: Text that flows across boxes — CSS multi-column, and the break rules that only mean something with it

**Type:** RFC
**Status:** Accepted (r4, by the orchestrator under Charlie's delegation after three review rounds; Grok 4.7 only — Codex budget exhausted; round-3 findings folded unreviewed — the implementation review checks them), 2026-10-05. Stage 1, stage 2's Apple half and the reader port built (§7, As built).
- r1 was reviewed with two scopes: CSS fidelity against Chrome 154 (`llp/reviews/1093-r1.grok-a.md`) and kernel and host implementation (`llp/reviews/1093-r1.grok-b.md`). Both were NOT READY.
- r2 had a delta review (`llp/reviews/1093-r2.grok.md`): NOT READY, with five MATERIAL findings.
- r3 had the final round (`llp/reviews/1093-r3.grok.md`): NOT READY, with four MATERIAL findings.
- r4 folds round 3's fixes as proposed, with no further review (§9).
- Admitted (orchestrator for Charlie, 2026-10-05) by the trade §8 Q1 offered, recorded in `rules/DEFERRED.md`.
**Systems:** Kernel (`schema.json` rows; a new `kernel/src/fragment.rs`; `TextMeasurer::lines`; `Kernel::fragments`), vendored Taffy (one patch), Contract (lowering), Apple host (`exact_set_lines`, a `fragments` op, fragment layers), Linux host (new `fragments.rs`), web hosts (none: the browser does it), Agent (LLP 1012 `layout`, `tap`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Revised:** 2026-10-05 (r2, r3, r4)
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

`column-gap` stays an `f32` defaulting to 0. `to_taffy` keeps copying it into Taffy's engine gap (`kernel/src/style.rs:1331`), so flex and grid are unchanged.

The multicol gap is resolved separately, at layout, into the patch's `gap` (D2). It is the row when its mask bit is set, so an authored 0 stays 0. Otherwise it is the element's computed `font-size`, CSS's 1em. Chrome gives a 16px gap for a 16px multicol inside a 40px parent, and 40px for `font-size: 2em` on a 20px parent.

The web emits nothing for an unset bit, and the computed value stays `normal`.

**Refused by name at lowering, each with what to write instead:**

- `column-span: all`. Only `none` exists, so the row is not added.
- `balance-all` and the page and region break values: `page`, `left`, `right`, `recto`, `verso`, `always`, `all`, `region`, `avoid-page`, `avoid-region`.
- The rule styles no host paints: `dashed`, `dotted`, `double`, `groove`, `ridge`, `inset`, `outset`. Chrome would paint them on the web and the native hosts would not.
- Multicol rows on a flex or grid container (Contract's `column`, `row`, `grid`). CSS ignores them there, so accepting them would be a silent no-op. The diagnostic says to write `view`.

## 2. Decisions

**D1. A multicol container is a block container with `column-count` or `column-width` other than `auto`.** That means `view`, a `text`, or any tag whose `display` is `block`.

- It establishes an independent formatting context. Its first child's `margin-top` stays inside it (Chrome: 30px below the container's top), and it shares no floats with its parent.
- A `text` that is a multicol container is one paragraph whose lines flow through its columns.
- The used count N and width W are CSS Multicol §3.4's pseudo-algorithm over the content-box width U, the two rows and the gap (for `column-width: w`, N = max(1, ⌊(U + gap)/(w + gap)⌋), W = (U + gap)/N − gap).
- Columns progress from the inline-start edge. Column i's left edge in the content box is i·(W + gap) under `ltr` and U − W − i·(W + gap) under `rtl` (Chrome, three columns of 100 with a 10px gap in 320: x = 220, 110, 0). Columns beyond N are overflow columns. They are scrollable overflow, clipped by `overflow` as anything else is.

**D2. The flow thread is laid out by Taffy, once, at width W.** This takes one vendored-Taffy patch, the next number in `EXACT-PATCHES.md` (Exact's: upstream has no multicol). A block container whose style carries `multicol: Some { count, width, gap, used_height }`:

- **Is its own formatting context.** It joins both of `block.rs`'s conditions:
  - the one that chooses a new `BlockFormattingContext` in `compute_block_layout` (`vendor/taffy/src/compute/block.rs:369` and `:419–425`), so it shares no floats with its parent;
  - the local of the same name in `compute_inner` (`:516`), which keeps its first child's margin inside.
- **Lays its children out at W.** Once its content width U is known, it computes W. Its in-flow children get W as their available width and as their basis for percentage widths (Chrome: `width: 50%` is 100px in a 400px two-column container). When the used width is content-sized, the outer width stays the `:552` result. U is that width less the horizontal content-box inset, which is what children already resolve against (`container_inner_width`, `:972–973`). The children are laid out as one column, N = 1 and W = U (D10.5).
- **For `overflow: scroll`, U and H are its client box.** Taffy reserves a scrollbar gutter only there (`:465–476`). The native hosts' scrollbars are overlays, so `overflow: auto` reserves none, and the fixtures use `scrollbar-width: none`, as LLP 1083's did.
- **Gives children a percentage-height basis only when its own height is definite before the flow thread is laid out.** In Chrome, `height: 50%` in a 200px-tall multicol is 100px. Under an auto height the basis is indefinite: Chrome makes the box 18px in a 59px container.
- **Takes `used_height`, when the kernel has written one (D5), only at `container_outer_height`, after its children are laid out (`:615–618`).** `used_height` is the columns' content-box height H.
  - The value written there is H plus the vertical padding and border, then the existing `min-height`/`max-height` clamp. Chrome 154 gives a padded `balance` container 60px for 40px of columns, `min-height: 100px` 100px, and `max-height: 30px` with `padding: 10px` 50px.
  - It is never written to `style.size`, to `known_dimensions`, or to `container_percentage_resolution_height`. So the children's inputs, their indefinite basis and Taffy's cache all stand.

  Writing it as a style height would be wrong. Chrome makes the same tree with `height: 59px` resolve the box to 30px and push the sibling into a third column, with a `scrollWidth` of 600, not 400.

Every container without the field is unchanged. That is held by the 512-tree incremental-against-fresh comparison Patch 7 left behind, and by the existing fixtures.

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

**The balancer is Chrome's.** It starts at the content's height over N, clamped to the cap below and no lower than the tallest unbreakable piece, and runs D4's cut at that height. Without the clamp, a guess that already fits in N columns would never meet the cap.

Chrome 154: six 20px blocks under `height: 40px` in two columns balance to 40px, three columns of two (`scrollWidth` 600), not to the guess of 60px. Under a definite height the container's border box stays the specified one. Under `max-height` it is D2's clamped outer height: four 20px blocks under `max-height: 30px` make a 30px container, `scrollWidth` 600. Its cap is:

- `h` under a definite height;
- `m` under `max-height`;
- the flow thread's single-column height under an auto height.

While the cut needs more than N columns and H is below its cap, it adds the cut's *shortage* to H. The shortage is the least addition that would let some column end later than it does: the overflow of the next line or box past that column's bottom. It is 0 when no addition would, as when every column ends at a forced break. Four paragraphs with `break-before: column` in `column-count: 2` keep four columns at any height.

The loop stops when the content fits in N columns, the shortage is 0, or H reaches the cap. Extra columns are then overflow columns of that height. A step need not reduce the column count, because a later one can. There is no N − 1 bound: when the shortage is one line, each walk moves one line. The shortage includes the lines D4's earlier perfect break pushes on. That is why six lines with `widows: 4` in two columns balance to 80px (2 + 4), not 60px (3 + 3): at 60, the perfect break after line 2 needs a third column. Eight lines with `orphans: 3; widows: 3` in three columns stay at 60px (3 + 3 + 2), because that split fits in N columns. No split there is perfect, so stretching stops.

Each walk reruns the cut over geometry already laid out, with no re-measure and no Taffy pass.

With `balance` under an auto or `max-height` height, the kernel writes the balanced H into the container's `used_height` (D2) and lays out once more from the root. Only the container, its ancestors and what follows it move. The children see the same indefinite basis, so they hit Taffy's cache. H depends on the flow thread and the flow thread on W, and neither on the container's height, so the second pass is a fixed point. A fresh replay reaches the same frames, which `layout_equality.rs` holds with multicol trees.

`used_height` lives in the kernel's fragmentation state, and `taffy_style` maps it into the patch's `multicol` field, never into `size`. `measure_auto_height` (`kernel/src/kernel/geometry.rs:57–75`) is the hypothetical layout behind `measure()`. It clears `used_height` for its compute, runs the cut on the result, and answers the border box D2 would write: the cut's H plus padding and border, clamped. Its restore then puts the live `used_height` back.

**D6. Lines come from the host's engine, on request.** `TextMetrics` stays a size and a baseline. `TextMeasurer` gains a method with a default that returns nothing, in which case the paragraph is monolithic:

```rust
fn lines(&mut self, stamp: &ParagraphStamp, request: &TextMeasureRequest<'_>, bottoms: &mut Vec<f32>) {}
```

It returns each line box's bottom in content coordinates, for the request the measure just answered. The kernel asks only for a paragraph that straddles a cut, and never for one whose `paragraph.markup` is Markdown (D10). Each host answers from the paragraph it just built and retains:

- Apple from `Paragraph.lineBottoms`, through a hook registered by a new `exact_set_lines` in `host/apple/src/abi/exports.rs` (586 lines) and called by `CallbackMeasurer`. `EXACT_ABI_VERSION` (`host/apple/include/exact.h:33`) moves.
- Linux from its buffer's layout lines.
- `MonospaceMeasurer` by arithmetic.

Measured is cut is painted, because it is one paragraph.

**D7. What the kernel publishes.** Fragmentation stays sparse, like 1043.000's resolved exclusions. The cut's output is stored, not recomputed later. Once frames are translated, a function rederiving it from published frames, as `sticky_constraint` is rederived (`kernel/src/kernel/sticky.rs:91–148`), could not recover a line range. The record is `Kernel::fragments(key)`. It is written by each cut and removed when a box no longer straddles.

- Every box in a multicol container's flow is published at its translated frame. A box wholly inside one column needs nothing new from a host.
- A box that straddles cuts is published with its **union** frame, which is what `getBoundingClientRect` returns (Chrome: two client rects at x = 0 and 220 have a 420-wide union) and so what `frame()` answers on the web.
- Each such box also gets a list of fragments, one per column it touches. A fragment is a rectangle in the box's coordinates, sliced as CSS `slice` does: the first keeps the top edge, the last the bottom, and each one before a break runs to its column's end. A paragraph's fragment also carries its line range and an (dx, dy) to add to those lines' unfragmented positions.
- **The container's scrollable overflow is one border-box rectangle**, written through publication's `set_content` (`kernel/src/layout/publication.rs:92–97`) in place of the flow thread's.
  - Its extent is the union of the column boxes and every flow box's translated border box, which keeps D10.4's absolutely positioned boxes, plus the end padding. It is never smaller than the client box.
  - Chrome 154: three 80px columns with a 10px gap give `scrollWidth` 260. With `padding: 10px` they give 280 by 60. An absolutely positioned box at `top: 200px` in the flow gives 620.
  - The hosts' walks (`host/apple/src/host.rs:1344–1350`, `host/linux/src/paint.rs:1439–1445`) start from that field and take each child's published frame. The frames are already translated, so the walks agree with it.
  - `set_content` stores the union's right and bottom edges, not its width, which is how `scrollable_overflow_rect` is stored today.
  - Under `rtl`, overflow columns extend left of the box (Chrome: columns at x = 68, −22, −112 in an 80px box, still `scrollWidth` 260). Those edges cannot record overflow left of 0, and the hosts' extents grow only rightward, which D10.8 declares.
- The container is published with its column rectangles, and with which of them hold a box. A zero-height box from a forced break counts as holding one.

A host draws `column-rule` centred in each gap between two columns that both hold a box, as tall as the columns. A column with no box gets no rule. A rule wider than its gap stays centred on the gap, moves no column, and paints under the columns' contents (Chrome: a 12px rule in a 4px gap shows only in the gap).

On Apple these reach the batch as a `fragments` op beside `sticky`, the presenter's encoding of the record. `LayoutReceipt` names every box whose translated frame changed. That can be every later box when an early paragraph grows, as in Chrome.

**D8. Each host paints and hits the fragments it was given.** The files that would hold this are at the line cap, so each host's part goes in new files called from the existing functions:

- **Linux, stage 1.** A new `host/linux/src/fragments.rs` paints a paragraph's buffer lines at each fragment's offset and a rule as a rectangle. It also produces the `layout` reply's fragments and tests hits.
  - `paint.rs` has two lines to spare (1,498), so the same change moves its paragraph paint arm (`paint.rs:1078–1143`) into `fragments.rs`, and `paint.rs` ends at or below 1,500. `presenter.rs` (1,488) only calls it.
  - Hits follow the record. A box with no `fragments` record is hit as its translated frame, like any box. A box with one is hit only inside a fragment, never in its union's gap. `tap` aims at the centre of the first fragment, not `PaintedBox::center` (`presenter.rs:1156`), which sits in the gap.
- **Apple, stage 2 (macOS and iOS).** A fragmented paragraph's view holds one layer per fragment, sized to it, drawing its line range at the fragment's offset, so no paragraph backs a two-page layer. Column rules are thin layers on the container.
  - This lives in new files beside `NodeViewMac.swift` (1,480) and `NodeViewIOS.swift` (1,479). Their `hitTest` overrides (`NodeViewMac.swift:668`, `NodeViewIOS.swift:727`) each make a one-line call into it, with Linux's rule.
  - One map serves every text hit: the point's fragment, by its rectangle inside the union, then the inverse of its (dx, dy). A point in no fragment, in the union's gap, maps to nothing.
  - On macOS the map feeds `TextSelectionMac`'s `line` and `index` (`:293–310`), so `lineIndex` and the caret's `stringIndex` both see unfragmented content coordinates.
  - `draw` (`:315–320`) paints `selectionRects` per fragment, for that fragment's line range, offset by its (dx, dy).
  - On both platforms the map also feeds `InlineText.textOffset` (`InlineText.swift:116–128`), which link hits use.
  - iOS has no text selection (`TextSelectionMac.swift` is macOS-only), so there it is link hits alone. `selectionchange` then holds across columns on macOS, the one host with a selection owner (`PresenterMac.swift:60`).
- **Web (JS target and wasm host).** The rows reach CSS by their own names; the browser lays out, breaks and paints, and the kernel's cut never runs.

**D9. The web is CSS multicol itself, and Chrome is the oracle.** Firefox has never implemented `widows` and `orphans`, and Firefox's and WebKit's balancers are their own. `conform-firefox` and `conform-webkit` record those cases as browser differences, not exact2's. Unlike `wrap-flow` (LLP 1043.000 D1), nothing executes multicol for the browser: every engine ships it.

**D10. Declared deviations, journalled once per box with what to change.** This is the `FlowRefusal` pattern of LLP 1043.000, recorded in LLP 1001's declared deviations. Chrome fragments each of the following, and the native kernel does not:

1. **A box with a definite `height` or `min-height`** is monolithic. Chrome slices it (a 200px box in 150px columns is 190×150 and 190×50; `min-height: 300px` spans three columns). **A box with `max-height`** is monolithic too. Chrome keeps its border box at the cap and fragments its overflowing children. The message for both says to use an auto height.
2. **Row and wrapped flexboxes, grids, and a nested multicol container taller than its column** are monolithic. Chrome fragments them; a nested multicol that fits is one rect in both.
3. **A box with its own decoration** that would fragment is monolithic until hosts slice decorations (§5, deferred). Decoration here means a background, border, radius, shadow, gradient, filter or clip-path. Chrome slices it. A highlight is an inline run's background, painted with its line, and unaffected.
4. **An absolutely positioned box whose containing block lies inside the flow** is placed at its flow-thread position, then translated by the column whose interval holds its own flow-thread top. It is never fragmented. Chrome places a 10×10 box at `top: 200px` in 40px columns with a 90px pitch at (540, 0): flow y = 240, column 6, `scrollWidth` 620, `scrollHeight` 40. The kernel matches that, so the case is parity. Chrome would fragment a box that crosses its column's bottom. A box whose containing block is the multicol container itself is placed against the container, as CSS does.
5. **A multicol container whose width is sized from content** (shrink-to-fit in a flex row) is one column, N = 1 and W = U, and U is Taffy's content-based width (D2). Chrome sizes it from N columns.
6. **Markdown, exclusions and virtualized lists.** One-node Markdown (LLP 1045) is monolithic. Exclusions (`wrap-flow`) and virtualized lists inside a flow are refused. `position: sticky` inside a flow behaves as `relative`.
7. **Fixed-height items in a column flexbox are pushed whole, where Chrome slices them.** This is item 1, inside a flexbox. A column of fixed-height items will not match Chrome until it is lifted. A gap before an item Chrome slices through is kept there; in the kernel the item moves with its gap truncated.
8. **Under `rtl`, overflow columns, which lie left of the box, are painted and clipped but not scrollable** on native hosts, whose scroll extents grow only rightward. Paging by `translate` (D11) is unaffected. Chrome scrolls them.

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

- **Chrome fixtures.** `kernel/tests/it/browser_columns.rs` and `fixtures/browser_columns.tsv`, measured in Chrome 154 by `browser_cases.rs`'s method, with each box's fragments from `getClientRects`. Text is `white-space: pre` with a pixel `line-height`, so Chrome's line boxes are exact and `MonospaceMeasurer` reproduces them. Every Chrome result cited in D1–D7 is a parity case, about forty in all. `mismatches` (`browser_cases.rs:177–188`) compares one rect per node. A row whose Chrome result is several `getClientRects` is also compared, rect by rect, against `Kernel::fragments`, through a fragment comparison beside it in `browser_columns.rs`. A case D10 declares carries the kernel's expected rects beside Chrome's, and both comparisons use the former. Each D10 row then holds the deviation to exactly what is declared. The reviews' measurements (`llp/reviews/1093-r1.grok-a.md`, `1093-r2.grok.md`) are the first draft of the TSV.
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
| Slicing `height` and `min-height` boxes, and fixed-height items (D10.1, D10.7) | the cut can end a fragment inside a box with no children to break between; stage 1's D10 fixture rows |
| Fragmenting a `max-height` box's overflowing children (D10.1) | the cut fragments children past a border box that stays at its cap (Chrome: a second, zero-height fragment) |
| Scrolling `rtl` overflow columns (D10.8) | hosts scroll overflow left of a box's origin |
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

### Stage 1 (kernel, Contract, Linux, web), 2026-10-04

- **Rows.** Bits 180–190 in `schema.json`: `column_count` (`u16`, 0 is `auto`), `column_width`, `column_fill`, `column_rule_width` (3, `medium`), `column_rule_style`, `column_rule_color` (`currentcolor`), `widows` and `orphans` (inherited, 2), `break_before`, `break_after`, `break_inside`, with enums `ColumnFill`, `BreakBetween` and `BreakInside`. The unset `column-gap` is the element's font size on a multicol container (`fragment::gap`), and Taffy's engine gap keeps the row's 0.
- **Taffy Patch 27** (`vendor/taffy/EXACT-PATCHES.md`), as D2 says, with one addition. While `used_height` is set, the container gives its children no percentage-height basis at all, even where a parent hands its height back as known. The equality differential found the case: an auto-height, absolutely positioned multicol root. Taffy's absolute solver lays it out again at its measured height, so `height: 84%` resolved against the used height and the cut never reached a fixed point. Chrome's basis there is indefinite too. The patch also reports the cut's overflow as the container's scrollable overflow, so ancestors see the columns, not the flow thread.
- **The cut** is `kernel/src/fragment.rs` (state, `Kernel::fragments`, `Kernel::columns`, `Kernel::fragment_refusal`, `settle`), `fragment/build.rs` (the flow thread as atoms) and `fragment/cut.rs` (the walk, the balancer, the placements). D3–D5 are as written. These details were decided in code:
  - A fragmentable box's top and bottom inset are glued to its first and last atom. The break before a box is then the break before its first child, so forced breaks propagate outward with no extra rule.
  - A better candidate, or a later one of the best appeal, may lie between the lines of a paragraph the column holds whole. When the column's best break is not perfect, or not the last candidate, the walk asks for those paragraphs' lines and walks the column again (Chrome: a widows violation beats a `break-after: avoid`).
  - Balancing starts from the runs between forced breaks: each run takes the columns its height asks for, and the guess is the tallest run's share (Chrome: four forced one-line runs in two columns are 20px tall, not 40). The guess is also raised to the tallest unit a walk had to take whole. A paragraph whose host gives no line boxes is one such unit; this was found driving macOS, where paragraphs stay whole until stage 2.
  - A middle break scores `widows` against every line after it, as Chrome does. This is the `a middle break keeps widows` case, measured before the code.
  - The cut is stored and replaced as a whole. Every old record is cleared before any new one is written, so a box that moved between flows is placed by its new container. A destroyed container takes its boxes' placements with it, and a container under `display: none` drops its record.
  - After the first cut, the extra engine pass writes only used heights and overflow. A container is cut again only when its used height appeared or went, its box resized, it holds a container whose height moved, or its overflow counts absolutely positioned boxes.
- **D10.5 as built.** A content-sized multicol takes its content's width as one column, then divides that width into its N columns. In a flex row Taffy's final layout hands the item a known width, so the r4 text's "one column, N = 1" could hold only for intrinsic probes.
- **D6.** `TextMeasurer::lines` defaults to nothing. `MonospaceMeasurer` answers by arithmetic, and Linux answers from its paragraph's line boxes (`Paragraph::line_bottoms`, kept beside the baselines). The kernel asks only when a paragraph would cross a column's end, and asks for no second measure.
- **Contract** (`contract/lower`): `columns`, `column-count` (with `auto`), `column-rule` (with `thin`/`medium`/`thick`), `column-rule-width` and every longhand lower to their rows. §1's refusals are diagnostics, each saying what to write. `column-span` and `page-break-*` are refused by name. Multicol rows on `row`, `column` or a literal `display` of flex or grid are refused with "write `view`". These replace `fix/typo`'s refusals.
- **Web.** The rows reach CSS by their own names: `column-count` 0 is `auto`, and `column-count`, `widows` and `orphans` are unitless. On both web targets `layout <node>` prints a fragmented box's `getClientRects`. The web's `tap` already aims inside a client rect.
- **Linux.** The paragraph arm moved out of `paint.rs` (now 1,437 lines) into `host/linux/src/paint/fragments.rs`, beside `paint`'s other parts rather than at the crate root, so it reaches `paint`'s private walk:
  - A fragmented paragraph is laid out at its own width and painted once per fragment at the fragment's offset, clipped to the fragment.
  - `column-rule` is a rectangle centred in each gap between two columns that hold a box.
  - `hit` accepts a point only inside a fragment, and `tap` aims at the first fragment's centre.
- **Agent (D12).** The runner's node detail gives every native host's `layout <node>` a box's `column_fragments` (with line ranges), a container's `columns` (with `holds`) and a D10 refusal. The key is `column_fragments` because `fragments` already means wrap-flow's line fragments. `scripts/agent-inspect.mjs` prints all three. D10's refusals reach `LayoutReceipt::fragment_skipped`; native hosts journal each once (`Runner::report_fragment_skipped`).
- **Tests.**
  - `kernel/tests/it/browser_columns.rs` with `fixtures/browser_columns.tsv`: 73 Chrome 154.0.8037.98 cases measured by `getClientRects`. Every D1–D7 result the LLP cites is among them. 61 are parity. 12 are D10 rows, each carrying the kernel's own rects, scroll sizes and line starts. Line starts are compared through `Kernel::fragments`.
  - `layout_equality::multicol_relayout_is_result_equal_to_full_relayout`: 16 seeds are checked in; 3,000 ran once. The random trees also draw multicol rows from the side stream.
  - The cut's unit tests (`fragment/tests.rs`).
  - Contract's `typography.rs`.
  - The web host's `css_tests`.
  - Linux's `pinned/text.rs`: paint, hit, tap and `layout` on a paragraph across two columns with a rule.
- **§3, measured** (M-series Mac, release; `fragment::tests::multicol_probe`, ignored, async lane):
  - 400 paragraphs in 37 columns lay out in 0.98–1.06 ms, against 0.90–0.94 ms as one column: +8% to +12%, at the +10% target.
  - One cut is about 0.07 ms.
  - A one-word edit in the last column relays in about 0.075 ms.
  - A balanced three-column, 400-paragraph container takes 2 walks.
- **Driven.** A scratch app (paged flow, `translate` page turns, a balanced block with a rule) was driven with `agent web`, `agent linux` (its plan on `caltrain-linux`) and `agent macos`. Web and Linux page alike; macOS places every box in its column but keeps paragraphs whole. The reader was not ported: that is stage 2, and it waits on the element resize event, which has not landed.
- **Not built in stage 1:** the Apple half (stage 2, below); `TextParityMacTests`; the reader port.

### Stage 2, Apple's half, 2026-10-04

- **D6.** `exact_set_lines` (`host/apple/src/abi/exports.rs`) registers an `ExactLinesFn` hook. It takes the measure request and returns each line box's bottom, and `CallbackMeasurer::lines` calls it. `EXACT_ABI_VERSION` is 12. Swift's `Runtime.setMeasure` registers `TextEngine.linesText` with the measurer. The hook answers from the paragraph the presenter paints at that width (`requestSpec`, which is now `measure`'s own spec construction, and `Paragraph.lineBottoms`).
- **D7.** A `fragments` op beside `sticky` (`Batch::fragments`, `Host::emit_fragments`) carries each box's fragments in its own frame and each container's columns. It is sent when a record changes and cleared when one goes. `Kernel::fragmented` lists what a host must read.
- **D8, as built:**
  - `host/apple/Sources/ExactKit/Columns.swift`. A fragmented paragraph is laid out at its unfragmented width (`paragraphBox`) and drawn once per fragment in its own view: clipped to the fragment and moved by its offset (`eachFragment`, called from `draw` on both platforms). It is not one layer per fragment, so a paragraph across a gap backs one layer as wide as its union. Text rasters and the content-region reader paragraph stand aside for it.
  - `column-rule` is a layer under the container's children, one rectangle per gap between two columns that both hold a box. It is rebuilt on the record and on a restyle.
  - Hits follow Linux's rule: both `hitTest` overrides refuse a point in the union's gap.
  - One map, the fragment's rectangle less its offset, feeds `TextSelectionMac`'s `line`, `index` and `draw` and `InlineText.textOffset`. The caret's map snaps a gap point to the nearest fragment, so a selection dragged across a gap keeps its focus.
- **D12.** The Apple agents' `tap` aims at the first fragment's centre (`tapBox`).
- **Tests and drives.**
  - `TextParityMacTests.testAParagraphInColumnsBreaksAsChromesColumns`: the paragraph's lines at the column width equal Chrome 154's columns, and its line boxes are 20px apart.
  - The scratch app on macOS: a three-column block with a rule matches the web's screenshot, line for line. A drag selection across the column boundary picks exactly the text the web picks ("teen seventeen "). A tap lands in the first fragment.
  - The scratch app on iOS: the same split.
  - The Swift host tests' three macOS failures (`BorderParityMacTests`, `BoxPaintMacTests`, `ClipMacTests`) fail on the base too.

### The reader, 2026-10-05

Stage 2's last part. The app lives in `~/projects/x2apps/reader` and is not edited there; the port is a patch against that tree.

- **One flow.** The chapter is a single column wide and a page tall, `column-fill: auto`, so columns past the window are overflow columns, one page each (D5). A spread is `column-count: 2` on a window two pages wide. A turn translates the flow by one column and its gap, or by two for a spread (D11). `column-count: 1` on a container one column wide is the same used width as `column-width` of that measure (D1).
- **Where the pages are read.** `resize` on the flow and on the chapter (`measured`). `frame("flow-end")`, an empty block after the last paragraph, is the last page; the first paragraph that starts on the page shown is the anchor a later resize finds again. The flow's own `translate` cancels, because `frame()` follows it on both reads. The 100 ms settle timer, the line-grid page height and the duplicated `r-` chapter are gone.
- **Driven** on web and macOS (`bun exact.mjs test`, which is `scripts/agent.mjs`). The eleven authored tests pass on both with the same page counts, plus one: a resize keeps the paragraph the page starts with. At 1200×800 Chapter I is 8 pages and page 3 starts at `p7`; at 800×600 that paragraph is page 4 of 12; at 1400×860 the spread is 9 pages, turned two at a time. A page of each, and the resized page, was screenshotted on both hosts.

## 8. Questions, as decided (orchestrator for Charlie, 2026-10-05)

1. **Admission:** by the trade offered. LLP 1043.000's stage 5 (`shape-outside: <image>`) moves behind stage 2 of this document, recorded in `rules/DEFERRED.md` in its own commit.
2. **When the reader reads:** an element resize event lands before stage 2 and is built separately. This document names its call site (D11, §6).
3. **Markdown splitting** waits for a viewer that asks (§5).
4. **Decorated boxes** stay deferred (§5).

## 9. Revisions

**r2 (2026-10-05).** Every finding of both r1 reviews is resolved except one, rejected with evidence. Each disposition is in its review file.

- *A1's fix* was to keep stretching while appeal improves within N columns. It is rejected, because it predicts 80px (4 + 4, perfect, two columns) for the reviewer's own eight-line case, where Chrome stays at 60. r2 said 100px; the r2 review corrected it to 80px. The six-line case it raised is explained by D4's earlier perfect break pushing a third column, which the ordinary stretch then removes. D5 now says so, and both cases are fixtures.
- *The changes:* A2–A9 and B1–B5, B9 rewrote D1–D5 and §1. A10–A13, B7, B8 and B10 changed D7, D8 and D10. B6, B12 and B13 changed D2, D6 and D8. B11 and the orchestrator's decisions changed D11, §5, §6 and §8.

**r3 (2026-10-05).** This answers the r2 delta review (`llp/reviews/1093-r2.grok.md`). Each of its findings was checked in the code and re-measured in Chrome 154.

- *Finding 1:* `used_height` becomes Taffy's `container_outer_height` only, so it is never a percentage basis (D2, D5). `measure_auto_height` answers the cut's height.
- *Finding 2:* the scrollable overflow is a union of the column boxes and translated boxes, plus padding (D7). `rtl`'s leftward overflow is declared as D10.8.
- *Finding 3:* the patch joins both BFC conditions, and a content-sized width is one column (D2, D10.5).
- *Finding 4:* fragments are a stored record. A box without one is hit as its frame, and `tap` aims at the first fragment (D7, D8).
- *Finding 5:* one fragment map feeds selection, the caret, highlights and `textOffset`. iOS has link hits only (D8).
- *Findings 6–9:* the balancer's cap and shortage (D5), the layout-time 1em gap (§1), D10's fixture rows and the split deferred rows (§4, §5), and the paragraph arm leaving `paint.rs` (D8).
- *Finding 10:* the status line and A1's height (above).

**r4 (2026-10-05).** This folds the round-3 review (`llp/reviews/1093-r3.grok.md`) as the reviewer proposed. Under the three-round rule it gets no further review; the implementation review checks it. The code citations were checked on origin/main.

- *Finding 1:* `used_height` is the content-box H. The outer height written is H plus padding and border, then clamped (D2, D5).
- *Finding 2:* the balancer's first guess is clamped to the cap (D5).
- *Finding 3:* an absolutely positioned box takes its own flow position's column, so 620 is parity (D10.4).
- *Finding 4:* for a content-sized width, U is the outer width less the inset (D2).
- *Findings 5–6:* `set_content` stores edges (D7, D10.8), and fragmented rows get a fragment comparison (§4).
- *Findings 7–9:* the status line and the line numbers.
