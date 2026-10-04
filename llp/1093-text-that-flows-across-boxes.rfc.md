# LLP 1093: Text that flows across boxes — CSS multi-column, and the break rules that only mean something with it

**Type:** RFC
**Status:** Draft r1. Not reviewed. Not admitted: §8 Q1 is Charlie's (`rules/DEFERRED.md`).
**Systems:** Kernel (`schema.json` rows; a new `kernel/src/fragment.rs`; `TextMeasurer::lines`), vendored Taffy (one patch), Contract (lowering), Apple host (measure ABI, a `fragments` op, fragment layers), Linux host (`paint.rs`), web hosts (none: the browser does it), Agent (LLP 1012 `layout`, `tap`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-05, stage 2 on 2026-10-06 (§6)
**Related:** the reader's diary (`~/projects/x2apps/reader/DIARY.md`, Rough and Top 5 item 4); the typography lane (`fix/typo` `0b56caef4`, which refuses these rows by name, and its QUEUE line, whose recommendation this takes); LLP 1001 §5–§6; LLP 1043.000 D4–D7, §8; LLP 1051.000 (`frame()`); LLP 1083; `vendor/taffy/EXACT-PATCHES.md`. External, read 2026-10-04: CSS Multi-column Layout 1 §3–§7; CSS Fragmentation 3 §3–§5; Chrome 154's block fragmentation (break appeal, column balancer).

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
- walks it in document order and cuts it into column-height intervals at legal break points, applying `widows`, `orphans` and `break-*`;
- publishes every box translated into its column.

A box that straddles a cut gets a list of *fragments*. A paragraph's fragments are line ranges with offsets. The native hosts already paint lines at per-fragment origins for text around shapes (LLP 1043.000), so painting is a generalization of what exists. The web emits the CSS and the browser does all of it. Chrome is the oracle.

## 1. What CSS has, and the subset taken

CSS fragmentation has three kinds of container: columns, pages (print, `@page`) and regions (withdrawn, shipped nowhere). Only columns exist on screen in every browser. The break rules mean nothing outside a fragmentation container, which is why the typography lane refused them with multicol.

**Admitted:**

| Row | Values taken | Initial |
|---|---|---|
| `column-count` | `auto`, a positive integer | `auto` |
| `column-width` | `auto`, a length | `auto` |
| `columns` | the shorthand of the two | |
| `column-gap` | the existing row, plus `normal` (1em in a multicol container, 0 in flex and grid) | `normal` |
| `column-fill` | `balance`, `auto` (`balance-all` is paged media) | `balance` |
| `column-rule-width`, `-style`, `-color`, `column-rule` | a length; the border styles the hosts paint; a color | `medium`, `none`, `currentcolor` |
| `widows`, `orphans` | a positive integer; inherited | `2` |
| `break-before`, `break-after` | `auto`, `avoid`, `column`, `avoid-column` | `auto` |
| `break-inside` | `auto`, `avoid`, `avoid-column` | `auto` |

**Refused by name at lowering, each with what to write instead:**

- `column-span: all`. Only `none` exists, so the row is not added.
- The page and region values: `page`, `left`, `right`, `recto`, `verso`, `always`, `all`, `region`, `avoid-page`, `avoid-region`.
- Multicol rows on a flex or grid container (Contract's `column`, `row`, `grid`). CSS ignores them there, so accepting them would be a silent no-op. The diagnostic says to write `view`.

## 2. Decisions

**D1. A multicol container is a block container with `column-count` or `column-width` other than `auto`.** That means `view`, a `text`, or any tag whose `display` is `block`.

- It establishes an independent formatting context, as CSS's does: its first child's margin never collapses through it.
- A `text` that is a multicol container is one paragraph whose lines flow through its columns.
- The used count N and width W are CSS Multicol §3.4's pseudo-algorithm over the content-box width U, the two rows and the gap (for `column-width: w`, N = max(1, ⌊(U + gap)/(w + gap)⌋), W = (U + gap)/N − gap).
- Columns progress in the container's inline direction: right to left under `direction: rtl`.
- Column i sits at x = i·(W + gap), whether or not it fits inside the box. Columns beyond N are overflow columns. They exist only when the column height is bounded (D5). They are scrollable overflow, clipped by `overflow` as anything else is.

**D2. The flow thread is laid out by Taffy, once, at width W.** This takes one vendored-Taffy patch, the next number in `EXACT-PATCHES.md` (Exact's, not upstream's: upstream has no multicol). A block container whose style carries `multicol: Some { count, width, gap }`:

- computes W from its own content box;
- lays its in-flow children out against W in place of its content width (percentages inside resolve against W, as in CSS);
- sizes itself as its style says.

Every container without the field is unchanged, held as Patch 7 was by a randomized differential. The children's positions are flow-thread coordinates. The patch decides nothing about breaks, and since no node's layout depends on where it is cut, a definite-height container needs one pass; an auto-height one needs a second (D5).

**D3. The cut is a walk over the flow thread in document order, in the kernel** (`kernel/src/fragment.rs`; `layout.rs` is at 1,492 of its 1,500 lines). It produces intervals [s₀, e₀), [s₁, e₁), … in flow-thread y, each at most the column height H. Column k shows the content in its interval, translated by (k·(W + gap), −sₖ). Translation replaces CSS's pagination struts, and nothing is re-laid out.

The break points are CSS Fragmentation §4.1's:

- *Class A*: between siblings in a fragmentable box.
- *Class B*: between two line boxes of a paragraph.
- *Class C*: between a fragmentable box's content edge and its first or last child.

*Fragmentable boxes* are:

- block containers whose `overflow` is `visible` or `clip`, with `height`, `min-height` and `max-height` all `auto`;
- single-line column flexboxes on the same conditions (Contract's `column`, the commonest container). With an auto height a column flexbox has no free space, so its items stack like blocks without margin collapsing.

Everything else is *monolithic*: images, video, canvas, inputs, scroll containers, row and wrapped flexboxes, grids, a nested multicol container, and a box with a definite height. CSS makes replaced elements, scroll containers and line boxes monolithic. The kernel also makes the rest monolithic, where Chrome fragments them. That is declared in LLP 1001 (D10).

A monolithic box that does not fit is pushed whole to the next column; one taller than a column starts it and overflows (a fixture case).

Margins at breaks follow CSS Fragmentation §5.2:

- An unforced break truncates the adjoining margins to zero, so the next interval starts at the next box's border edge.
- A forced break keeps the margin after it.

Every column takes at least one line or one monolithic box, so the walk always makes progress.

**D4. The cut rule: the latest break point with the best appeal.** These are Chrome's terms, not CSS's, and they implement Fragmentation §4.4's relaxation order. Each candidate break point gets an appeal, from best to worst:

1. *perfect*;
2. *violates `orphans`/`widows`*: fewer than `orphans` lines before it in its paragraph, or fewer than `widows` after;
3. *violates `break-*: avoid`*: a Class A point beside an `avoid` or `avoid-column`, or any point inside a box with `break-inside: avoid`;
4. *last resort*: inside a line or a monolithic box, never taken while anything else fits.

When the next line or monolithic box would cross the interval's bottom, the walk breaks at the latest candidate of the best appeal it has seen since the interval began. `break-before: column` or `break-after: column` ends the interval there, whatever fits.

**`widows` and `orphans` default to 2.** Chrome therefore applies them to every multicol paragraph that nobody styled. Parity needs them from the first commit, not as an option. A paragraph with fewer than `orphans + widows` lines cannot be split perfectly, so it moves whole unless that leaves the interval empty.

This is the reader's missing rule: a paragraph's first line never ends a page alone.

**D5. Column height and `column-fill`.**

- With a definite `height`, H is the content-box height. `max-height` bounds H the same way.
- `column-fill: auto` fills each column in turn, and content past N columns goes to overflow columns. Pagination wants this.
- `column-fill: balance`, CSS's initial value, mirrors Chrome's balancer. It starts at the content's height over N, no lower than its tallest unbreakable piece. It then stretches by the least space shortage the cut reports, until the content fits in N columns or H is reached.
- Each balancing step reruns the cut walk over geometry already laid out, with no re-measure and no Taffy pass.
- With an auto height, the container's height is the balanced H. The kernel writes it into the container's engine style and lays out once more. H depends only on the flow thread, which depends only on W, never on the container's height, so the second pass is a fixed point. It is an accelerator: a fresh replay reaches the same frames, which `layout_equality.rs` holds with multicol trees.
- Chrome's choice for `column-fill: auto` under an auto height is a fixture case. The kernel does what it does.

**D6. Lines come from the host's engine, on request.** `TextMetrics` stays a size and a baseline. `TextMeasurer` gains a method with a default that returns nothing, in which case the paragraph is monolithic:

```rust
fn lines(&mut self, stamp: &ParagraphStamp, request: &TextMeasureRequest<'_>, bottoms: &mut Vec<f32>) {}
```

It returns each line box's bottom in content coordinates, for the request the measure just answered. The kernel asks only for a paragraph that straddles a cut, at most a few per column. Each host answers from the paragraph it just built and retains:

- Apple from `Paragraph.lineBottoms`, through a new call on the measure C ABI (`host/apple/src/measure.rs`, whose ABI version moves);
- Linux from its buffer's layout lines;
- `MonospaceMeasurer` by arithmetic.

Measured is cut is painted, because it is one paragraph.

**D7. What the kernel publishes.** Fragmentation stays sparse, like 1043.000's resolved exclusions:

- Every box in a multicol container's flow is published at its translated frame. A box wholly inside one column needs nothing new from a host.
- A box that straddles cuts is published with its **union** frame, which is what `getBoundingClientRect` returns and what `frame()` therefore answers on the web. It also gets a `fragments` list, one entry per column it touches: the fragment's rectangle in the box's coordinates (CSS's `slice`: the first fragment keeps the top edge, the last the bottom, each one before a break running to its column's end), plus, for a paragraph, the line range and an (dx, dy) to add to those lines' unfragmented positions.
- The multicol container is published with its column rectangles and which of them have content. A host draws `column-rule` in the middle of each gap between two contentful columns, as tall as the columns, as Chrome does.

These reach the host's batch as one `fragments` op beside `sticky`. `LayoutReceipt` names every box whose translated frame changed, which can be every later box when an early paragraph grows, as in Chrome.

**D8. Each host paints the fragments it was given.**

- **Apple (macOS and iOS).** A fragmented paragraph's view holds one layer per fragment, sized to it, drawing its line range at the fragment's offset, so no paragraph backs a two-page layer. Hit testing answers only inside fragments, so a union never takes a tap meant for the next column. `TextSelectionMac` and `selectionchange` map through the same offsets. Column rules are thin layers on the container.
- **Linux.** `paint.rs` draws a paragraph's buffer lines with each fragment's offset added, and a rule as a rectangle.
- **Web (JS target and wasm host).** The rows reach CSS by their own names; the browser lays out, breaks and paints, and the kernel's cut never runs.

**D9. The web is CSS multicol itself, and Chrome is the oracle.** Firefox has never implemented `widows` and `orphans`, and Firefox's and WebKit's balancers are their own; `conform-firefox` and `conform-webkit` record those cases as browser differences, not exact2's. Unlike `wrap-flow` (LLP 1043.000 D1), nothing executes multicol for the browser: every engine ships it.

**D10. Declared deviations, journalled once per box with what to change.** This is the `FlowRefusal` pattern of LLP 1043.000, recorded in LLP 1001's declared deviations. Chrome fragments each of the following, and the native kernel does not:

1. **Monolithic where Chrome fragments** (D3): row and wrapped flexboxes, grids, and a box with a definite `height`, `min-height` or `max-height`. The message says to make it a `view` or `column` with an auto height.
2. **A box with its own decoration** (background, border, radius, shadow, gradient, filter, clip-path) that would fragment is monolithic until hosts slice decorations (stage 3). The deviation most likely to be met; a highlight is an inline run's background, painted with its line, and unaffected.
3. **An absolutely positioned box whose containing block lies inside the flow:** it is placed unfragmented, translated with its containing block's first fragment. A box whose containing block is the multicol container itself is placed against the container, as CSS does.
4. **A multicol container whose width is sized from content** (shrink-to-fit in a flex row): it takes its content's intrinsic widths as if it were one column.
5. **Exclusions (`wrap-flow`) inside a flow are refused, and so is a virtualized list.** `position: sticky` inside a flow behaves as `relative`.

**D11. How an app pages: by `translate`, and by reading where the flow ends.** There is no page-count fact and no new `Geometry` field. A spread is `column-count=2` on a container two pages wide, and each turn moves it by the container's width plus a gap. The page arithmetic is all `frame()`:

```
end   = frame("flow-end")                          // an empty block after the last paragraph
flow  = frame("flow")                              // untransformed, so translate is ignored
pages = floor((end.x - flow.x) / (pageW + gap)) + 1
page  = floor((frame(para).x - flow.x) / (pageW + gap))   // keep the place on resize
```

`frame()` is untransformed and a straddling paragraph's union starts in its first column, so both reads hold on every host. The reader's page-turn animations keep working, because a turn moves the container by its transform and that never re-lays anything out (LLP 1002 D2).

Not taken: **`scroll-snap` across columns** (columns are not boxes; CSS reaches them only through `::column`, and Contract has no pseudo-elements), and **`scrollWidth` on `Geometry`** (the first overflow read, asked for by nothing else).

What still costs the reader a timer is *when* to read. This document does not decide that (§8 Q2).

**D12. The agent sees fragments.**

`layout <node>` prints a container's columns and a box's fragments on every host (on the web, from `getClientRects`); `tree` shows union frames; a `tap` aims inside a node's first fragment, never at a union's middle, which may be a gap (the diary's inline-run fix, made for fragments).

## 3. Performance

- **Nothing without multicol:** one branch per node in publication, as 1043.000 met for exclusions, and a `None` check per block layout in Taffy.
- **The flow thread costs what the reader pays today** for its one tall column, and a spread builds half the nodes it builds now.
- **The cut is O(boxes in the flow), with a line query per straddling paragraph.** Apple answers that query from the retained `Paragraph`, so it is a lookup. The walk resumes at the column holding the earliest box whose flow frame changed, so an edit near the end of a chapter does not re-cut its beginning. Balancing reruns the walk at most N − 1 times.
- **A page turn is a transform:** no layout or repaint; hosts cull off-screen layers by frame as they do now.
- **Owed by stage 1**, in §7 from an ignored probe in the async lane (no metrics line, no check): the cut for a 40-column, ~400-paragraph chapter; that chapter's layout against it as one column (target +10%); a one-word edit in its last column.

## 4. Tests

- **Chrome fixtures.** `kernel/tests/it/browser_columns.rs` and `fixtures/browser_columns.tsv`, measured in Chrome 154 by `browser_cases.rs`'s method, each box's fragments from `getClientRects`. Text is `white-space: pre` with a pixel `line-height`, so Chrome's line boxes are exact and `MonospaceMeasurer` reproduces them. About twenty cases: count, width and both; `gap: normal`; overflow columns; `balance` and `auto` under fixed and auto heights; `widows`/`orphans` at 1, the default, and where neither can hold; forced breaks, `avoid` with and without an alternative, `break-inside: avoid`; margins at unforced and forced breaks; a nested block and a nested column flexbox; a pushed figure and one taller than a column; a multicol `text`; `rtl`; rules beside an empty column; an absolute child; a padded fragmented paragraph.
- **Kernel.** `layout_equality.rs` gains multicol trees: incremental equals rehydrated equals fresh replay, with the cut's resume point exercised. Unit tests hold the §3.4 arithmetic and the appeal ranking.
- **Hosts.** `TextParityMacTests` gains a multicol case: a bundled face, so lines match Chrome's, and each column's first and last words equal Chrome's. The Linux host's `css_tests` gains the same case.
- **Contract.** The admitted names lower, and each refusal in §1 and D10 is a diagnostic test. These replace `fix/typo`'s fragmentation refusals in `contract/cli/tests/it/typography.rs`.
- **The reader, driven** with `agent web` and `agent macos`: its eleven authored tests pass on both with equal page counts, plus one: after a resize, the page shows the same paragraph. None of this adds a check.

## 5. Out, and why

- **Paged media:** `@page`, `page`, page margins and printing. exact2 does not print.
- **Regions:** shipped nowhere, and boxes of different widths would make a paragraph's lines depend on where it is cut (1043.000's harder problem).
- **`column-span: all`:** it splits the flow into column rows, each balanced; it returns with a consumer.
- **Nested multicol, `box-decoration-break: clone`, `::column` and scroll markers, and `balance-all`.**
- **A layout-change event.** Repagination's trigger is the diary's Top 5 item 3, a separate decision (§8 Q2).

## 6. Stages

1. **Kernel, Contract, Linux and web, 2026-10-05.**
   - The rows in `schema.json`, and lowering with §1's refusals.
   - The Taffy patch (D2), `fragment.rs` (D3–D5), `TextMeasurer::lines` (D6), publication (D7) and the journal (D10).
   - Linux painting (D8) and the agent's `layout` (D12).
   - The Chrome fixtures, the equality trees and §3's probe.
   - On the web, CSS emission only.
2. **Apple and the reader, 2026-10-06.**
   - The measure ABI's `lines` call, the `fragments` op, fragment layers, hit testing and selection on macOS and iOS, and column rules.
   - `TextParityMacTests`, and the driver's first-fragment tap.
   - The reader in `~/projects/x2apps/reader`: one flow, the spread as `column-count=2`, its duplicated `r-` ids and its line-grid page height deleted, and its diary updated.
3. **Not scheduled; specified here only so stage 1 leaves room for it:**
   - slicing a fragmented box's own decorations per fragment, which is D10 item 2;
   - row-flex and grid fragmentation, which is D10 item 1.

   Each returns with an app that needs it.

## 7. As built

Nothing yet.

## 8. Open questions

1. **Admission (Charlie's).** The reader is not one of `rules/DEFERRED.md`'s consumers. The typography lane's rows came from the same diary. This needs an **Expanded** entry. The take offered: LLP 1043.000's stage 5 (`shape-outside: <image>`, the dancer's traced silhouette, a QUEUE line) moves behind stage 2 of this document. Or Charlie waives the take.
2. **When the reader reads.** D11 answers *how many pages* and *where a paragraph is*, not *when the layout changed*. The reader keeps its 100 ms settle timer (zero evaluations idle, by its diary) until an element resize event is decided. Should that come before stage 2, so the port deletes the timer too?
3. **One-node Markdown** (LLP 1045) is monolithic inside a flow: its blocks are not boxes the kernel sees. If the Markdown viewer (LLP 1033) wants columns, its engine would answer `lines` with block boundaries; that waits for it to ask.
4. **D10 item 2's reach.** Should slicing decorations move into stage 2? A styled pull quote in a book would need it; the reader's highlights do not.
