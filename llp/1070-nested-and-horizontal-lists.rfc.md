# LLP 1070: Nested and horizontal virtualized lists

**Type:** RFC
**Status:** Draft (r1)
**Systems:** Contract (`contract/lower/src/collection.rs`: the two refusals this replaces), Runner (`runner/src/instance/collection/`: the axis, the nested lifetime, pins, the kept index), Web host (`collection-glue.js`), Apple host (`Collection.swift`, `IOS/CollectionIOS.swift`, `IOS/ScrollPumpIOS.swift`, `IOS/ScrollViewIOS.swift`, `IOS/NodePoolIOS.swift`, `Mac/CollectionMac.swift`, `Mac/ChainingScrollView.swift`), Linux host (`presenter.rs`, `presenter/collection.rs`), Agent (LLP 1012: no new operation)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** none named. `rules/RULES.md` wants one before this is built; the lane that ports the Extra Heavy feed (`~/bench/xheavy`) is the natural owner, and Charlie names it when he rules
**Date:** 2026-09-27 (r1)
**Related:**
- LLP 1010 §6 (the collection: row state dies on retirement, `:334–340`; nested virtual rows rejected, `:760–763`).
- LLP 1050.000 D1–D6 (never blank by default; a costly row is never built mid-fling; the owed set, `limit`, retirement).
- LLP 1068 (heavy-view recycling; stage 1 pools rows around heavy leaves, on `feat/heavy-pool-stage1`, not on main; §2's oracle; §4.2 pools a nested scroll view at rest with its offset reset).
- LLP 1008 "Orthogonal carousels" (`:678–697`) and "Short vertical scroll containers" (`:664–675`); LLP 1033 D4 (`overscroll-behavior` as a kernel row); LLP 1057 (`touch-action` is the arbitration model).
- `~/bench/xheavy/SPEC.md` and `EXACT2-GAPS.md` gap 8; `gen.py:483–523` (the two new kinds).
- `QUEUE.md` "One list engine", "The rest of LLP 1050.000 stage 1", and the Apple entry "a pan chaining out of a nested scroll view at its edge".
- `rules/DEFERRED.md` §Components (no virtualList v2; the windowed list admitted 2026-09-14).

## Summary

The Extra Heavy feed gains two row kinds (`gen.py:516`):
- **`filmstrip`**, every sixth row (500 of 3,000): a horizontal virtualized strip of 2,000 thumbnails;
- **`inbox`**, every twelfth row (250): a fixed-height box holding its own vertical virtualized list of 1,000 messages.

That is 1.25 million inner items behind one feed. Charlie wants it as "an even more ridiculous stress test".

exact2 refuses both today. The compiler rejects a virtualized list inside another list's row (`collection.rs:72–76`) and a horizontal virtualized container (`:127–131`). The runner re-checks the first at creation (`traversal.rs:443–476`).

The kernel is already axis-generic (`overflow_x`/`_y`, `content` on both axes). Everything vertical-only lives in the collection seam:
- the runner's `scroll_top`, `port_height` and `HeightIndex`;
- the three feedback encoders;
- each host's geometry, `covers` and `rowsToCover`.

**Recommended decisions** (Charlie rules; questions in §12):

| # | Decision |
|---|---|
| H1 | One engine, two axes. The collection's seam becomes main/cross (`offset`, `port_main`, `cross`, `size`), wire v3; no second list type |
| H2 | A horizontal list is CSS's: `display="flex" flex-direction="row"` on the `list`. `row-reverse`, `column-reverse`, `flex-wrap: wrap` and `direction: rtl` are refused by name |
| H3 | LLP 1050.000's table applies unchanged on the main axis; the rest offset is `targetContentOffset.x` |
| H4 | The runner anchors on the main axis, both axes. Chrome anchors only on the block axis (measured, §2): a declared deviation, because every off-screen size change a virtualized list sees is an artefact of not having laid it out |
| N1 | An inner list lives and dies with its outer row, on every host. No host carries one across outer rows |
| N2 | What survives an outer row's retirement: nothing app-visible. The runner keeps a bounded cache of the inner list's **index** (keys and measured sizes), used as estimates, never as authority. The scroll offset is not kept (Q1) |
| N3 | Keyed by the outer collection, the outer row's key, and the inner list's plan node plus any eager `each` keys between them |
| N4 | Bounded: at most 8 kept indexes and 512 KiB per outer collection (trial values), dropped on memory pressure |
| N5 | A pin (focus, interaction) pins its row in every enclosing list: at most two chains, four rows at depth two |
| N6 | One level of nesting. Deeper is refused at compile |
| F1 | An inner row is owed when its own list shows it and its outer row is owed |
| F2 | While any ancestor list moves, an inner list builds only what it owes (`limit` 0). Its bootstrap is its port at the estimate, not 16 rows |
| F3 | An outer row's cost includes its inner lists' owed rows and their O(N) setup. D3 applies to the whole row |
| G1 | Chaining is CSS's everywhere: at gesture start, never mid-gesture. iOS implements it with `gestureRecognizerShouldBegin`; macOS latches a phased gesture |
| G2 | No direction lock property. Orthogonal nesting is UIKit's own and the browser's |
| G3 | No new agent operation: `tap <testId> wheel dx dy` addresses an inner list; `state` shows its snapshot with its parent |
| S | Horizontal first (the axis refactor, then top-level horizontal lists), then nesting, then the kept index if measured to pay |

## 1. What exists

**Two engines, one kept.** `list virtualized=true` is the collection (`runner/src/instance/collection/`). `item-height`/`estimated-item-height` without `virtualized` is the legacy windowed list (`window.rs`), which `QUEUE.md` "One list engine" deletes after parity. This RFC extends only the collection. Nothing here is built on the windowed list.

**The collection's shape** (`collection/mod.rs:63–101`, `views.rs`):
- Rows are flow children of the `list` node, interleaved with spacers (`views.rs:108–145`). A spacer is `height: gap; width: 100%; flex-shrink: 0` (`:86–107`).
- A row wrapper is `display: flex; flex-direction: column; width: 100%; flex-shrink: 0` (`:35–45`). Its border-box height is what the host measures.
- The index is `HeightIndex` (`index.rs:86–97`): `positions: BTreeMap<Rc<str>, usize>`, per-row heights and a prefix-sum tree.
- The window is one viewport of overscan each side plus a quarter-second lead, capped at two viewports (`mod.rs:36–54`).
- Rows farther than two viewports always retire. Rows kept past the window never outnumber the window's own (LLP 1050.000 §6).

**Already nesting-aware.** `collections()`, `find_collection` and `feedback_walk` descend into mounted collection rows (`traversal.rs:69–97`, `:292–346`). The web controller keys rows to their nearest owner (`collection-glue.js:109–120`). iOS finds the owning collection through the nearest row wrapper (`Collection.swift:232–253`). `CollectionMacTests.swift:282` tests nearest-pin ownership.

A virtualized list inside an **eager** list's row, or inside a plain scroller, is allowed today.

**Not nesting-aware:**
- `find_collection_mut` does not descend into a collection's mounted rows (`traversal.rs:383`, "Virtual row descendants cannot contain another collection").
- `release_other_pins` assumes no nesting (`:415–416`).
- Linux's `pin_owner` picks the first snapshot whose rows contain an ancestor, not the innermost (`presenter/collection.rs:223–240`).

**Vertical-only**, every site:
- *Runner*: `CollectionFeedback.scroll_top`, `port_height` and `row_width` (`api.rs:18–39`); `CollectionRow.top`/`height`; `AnchorCorrection.scroll_top`; spacers and wrappers above.
- *Web*: `geometry()`, `rowsToCover`, `move` and `portOf` read `scrollTop`, `clientHeight`, `.top` and `overflowY` (`collection-glue.js:69–108`, `:92–97`, `:224–226`).
- *iOS*: `contentOffset.y`, `contentSize.height`, `covers` and `rowsToCover` (`CollectionIOS.swift:40–119`); velocity sampling (`ScrollPumpIOS.swift:82–86`).
- *macOS*: `preparedCover` and `KnobDrag` (`CollectionMac.swift:83–254`); `PresenterMac.swift:325–335`.
- *Linux*: `max_top`, `padding_top`, `offset.1` and `model_top` (`presenter/collection.rs:11–61`, `:186–221`, `:669–703`); `paint/region.rs:61–69`.

**Single-list assumptions:**
- iOS: the text lead follows the fastest list's scalar velocity (`ScrollPumpIOS.swift:253`), and the raster throttle is one flag for any list (`:258`).
- iOS: two feedback passes per main-queue turn are shared by all collections (`Collection.swift:49–59`).
- Web: four reports per frame are shared by all collections (`collection-glue.js:146`).
- The interaction lease is global on web and Apple. That is correct: there is one pointer.

## 2. The oracle: what Chrome does

**Method.** The oracle is nested `overflow: auto` boxes in Chrome, the same DOM the web host emits.

Measured 2026-09-27 on Chrome 154.0.8037.57 headless, over CDP, with a scratch page (appendix):
- an 800 × 600 outer box of 40 rows;
- an inner box in row 3: vertical, 400 × 200 with 1,200 px of content; or horizontal, 600 × 120, a flex row of 30 cards with 2,640 px of travel.

"Wheel" is `Input.dispatchMouseEvent` `mouseWheel`: phase-less 120 px ticks, 16 ms or 400 ms apart. "Touch" is `Input.synthesizeScrollGesture` with `gestureSourceType: touch` and no fling.

| Case | Result |
|---|---|
| Wheel down over a vertical inner, ticks until past its end | The inner takes each tick. The tick that reaches its end is clamped (960 → 1000); the remainder is dropped. The *next* tick goes to the outer. Same at 16 ms and 400 ms spacing |
| Same, inner `overscroll-behavior: contain` | The outer never moves |
| Vertical tick over a horizontal-only inner | The outer scrolls |
| Horizontal tick over it | The inner scrolls to its end, then stops; the outer cannot take x |
| Diagonal tick (60, 100) over it | The inner takes x (60 per tick) and y is dropped. The outer does not move. No split |
| Touch drag of 1,500 px starting in a vertical inner at 0 (max 1,000) | The inner reaches 1,000. The outer does not move. **No hand-off mid-gesture** |
| Touch drag of 600 px, inner at 900 | The inner reaches 1,000; the rest is dropped |
| Touch drag of 300 px starting with the inner already at its end | The outer takes it (≈300, three runs) |
| Touch over a horizontal-only inner: vertical 300 / horizontal 300 | Outer 302 / inner 301 |
| Touch diagonal, x-dominant (200, 120) / y-dominant (120, 200) | Inner x 202, outer still / outer 201, inner still: **the dominant axis picks the scroller** |
| Touch pushing a horizontal inner past its end | Nothing moves |
| Scrolled inner (`scrollLeft` 350) removed; a clone inserted | The new element is at 0 |
| The **same** element removed and re-inserted, same task or next frame | 0: Chrome resets it |
| `display: none`, then back | 350 kept (reads 0 while hidden) |
| `moveBefore()` to another row (Chrome 133+) | 350 kept |
| Its row `content-visibility: auto`, scrolled 3,300 px away and back | 350 kept |
| Outer anchoring: a row above the viewport grows 300 px (outer at 1,500) | Outer → 1,800 |
| Same with `overflow-anchor: none` | 1,500: no anchoring |
| Inner vertical box anchors on its own: a message above grows 160 px | 500 → 660 |
| Horizontal inner (`horizontal-tb`, flex row or inline-blocks): a card left of view widens 300 px | `scrollLeft` stays 1,000: **no inline-axis anchoring** |
| Same content in `writing-mode: vertical-lr` (x is the block axis) | 1,000 → 1,300 |

**What exact2 takes from it:**

- **Chaining is decided at gesture start and latched.**
  - A touch gesture belongs to one scroller from its first movement to its end.
  - It goes to the innermost scroller that can move in the gesture's dominant direction. At its edge the rest of the gesture is dropped, or rubber-banded where the platform shows overscroll.
  - A gesture that *starts* at an inner edge in the outward direction goes to the outer.
  - A phase-less wheel tick is its own gesture.
  - `contain` and `none` stop chaining.
  - This is `overscroll-behavior: auto` as Chrome runs it (G1).
- **Phase-less diagonal ticks are Chrome's one surprise.** Chrome latched them to an inner that could take *either* axis, where touch used the dominant axis.
  - exact2's macOS and Linux route a wheel by its dominant axis (`ChainingScrollView.swift:88`, `presenter.rs:1071`).
  - The difference needs a diagonal phase-less event, which a mouse wheel does not produce.
  - A trackpad sends phased events, which CDP cannot synthesize. Chrome with a real trackpad is checked by hand before this is called a deviation (§8).
- **Re-creation resets the offset.** Removal is not hiding: Chrome keeps an offset only for an element that stays in the document (hidden, moved, or skipped by `content-visibility`).
  - exact2's declared lifecycle is removal (LLP 1068 §2): a retired row's views are gone, as its component slots are (LLP 1010 §6.2).
  - An inner list re-created with its outer row therefore starts at offset 0. SPEC's carousel says the same: "a recycled row starts at 0 too".
  - §4.2 records why exact2 does not keep it anyway, and Q1 asks.
- **Anchoring is on the block axis only.** H4 declares the horizontal deviation.

The eager oracle and the lifecycle oracle disagree in one place. On an eager page an inner box is never removed, so its offset survives scrolling the page away and back. A virtualized list removes the row. exact2 chose the lifecycle oracle for row state in LLP 1010 §6.2 and LLP 1068 §2. This RFC keeps that choice for the offset (Q1).

## 3. Horizontal virtualization

### 3.1 H1: one engine, two axes

The collection gains an axis, fixed at creation from the list's resolved style: block, or flex column, is vertical; flex row is horizontal. The seam is renamed to be axis-neutral, with no compatibility shim (RULES: delete, don't deprecate):

| Today | v3 | Horizontal meaning |
|---|---|---|
| `scroll_top` | `offset` | `scrollLeft` from the content's start edge |
| `port_height` | `port_main` | the scrollport's width |
| `port_width` | `port_cross` | its height |
| `row_width` | `cross` | the rows' available height, after padding |
| `RowMeasurement.height` | `size` | the wrapper's border-box width |
| `CollectionRow.top`, `.height` | `start`, `size` | |
| `AnchorCorrection.scroll_top` | `offset` | |
| `HeightIndex` | `SizeIndex` | |

`CollectionFeedback` becomes version 3. Its three encoders change together: web by hand (`collection-glue.js:6–31`), Apple by hand (`Collection.swift:20–35`), Linux in Rust (`presenter/collection.rs:669–703`). The snapshot gains `axis`.

The logic is unchanged: `window_led`, `lead`, retirement, anchors, `reachstart`/`reachend` and `scrollFollowEnd` are one-dimensional already. Only their names say "height".

**The views.** The runner already emits wrappers and spacers through the kernel's own styles (`views.rs:4–21`). On a row-axis list:
- the wrapper is `display: flex; flex-direction: column; flex: none; min-height: 0`, with no width. It is stretched across by the list's `align-items`, and its width is measured.
- a spacer is `width: gap; flex: none; align-self: stretch`.

`flex: none` is structural, as `flex-shrink: 0` is on the vertical wrapper today. In CSS a flex row's items shrink by default (`flex-shrink: 1`), so an *eager* strip of cards needs `flex: none` on each card to overflow at all. The virtualized wrapper supplies it; the author's card inside is unaffected.

### 3.2 H2: the author's surface is CSS's

A horizontal list is `display: flex; flex-direction: row` on a scroll container. That is how a web page builds a carousel, and exact2's web host emits exactly that DOM:

```contract
list virtualized=true display="flex" flex-direction="row" height=132 estimated-item-width=128 overflow-y="hidden" testId="strip"
  each t in strip.items key=t.id
    Thumb(t=t)
```

- **`estimated-item-width`** is the row-axis twin of `estimated-item-height` (LLP 1010 §6.5): a positive literal, kebab-case host policy, not CSS. The estimate on the other axis is refused.
  - CSS's own name for "the size to assume for what has not been laid out" is `contain-intrinsic-size`. Renaming both estimates to it is a separate change; this RFC does not make it.
- **`flex-direction` needs `display="flex"`.** CSS ignores it on a block; the compiler refuses that instead, since a silently vertical list would be the likelier bug.
- **Refused by name, each with its reason in the message:**
  - `row-reverse` and `column-reverse`: an inverted list (chat) is its own consumer;
  - `flex-wrap: wrap` or `wrap-reverse`: a grid, which `rules/DEFERRED.md` keeps out with virtualList v2 (LLP 1050.000 §4);
  - `direction: rtl` on a row list: Chrome's RTL `scrollLeft` is negative from the right edge, and no consumer needs it yet.
- **The main axis mirrors the vertical rules.**
  - Main-axis padding must be zero (`padding-left`/`-right` on a row list, as `padding-top`/`-bottom` are refused today; `collection.rs:100–108`). Cross-axis padding is allowed.
  - `gap` stays refused on both axes; spacing goes on the row root as a margin, which the wrapper encloses (`views.rs:33–34`).
  - `overflow-x` must scroll on a row list. `overflow-y` may be `hidden`, as vertical lists set `overflow-x="hidden"` today.
- **The cross size must be definite.** A row list needs `height` (or `min-height` with `max-height`).
  - An auto height would be the tallest *mounted* card. The eager oracle's height is the tallest card of all 2,000, so an auto height would change as cards mount.
  - The main axis needs nothing on a block-level list: an auto width in block flow is the container's. A shrink-to-fit parent is caught by the bake's measured-layout lint, as for vertical lists today (`collection.rs:26–27`).

### 3.3 H3: the fill policy on the horizontal axis

LLP 1050.000's owed-set table (§2.1) applies unchanged, read on the main axis. Visible means the horizontal band.

- **Velocity** is `contentOffset.x` per second on iOS and `scrollLeft` samples on the web. A step larger than the port's width is a jump.
- **The rest offset** is `targetContentOffset.x` on iOS.
- **D2 (thumb drags wait):** `KnobDrag` on macOS becomes axis-generic. iOS and the web have no signal, as vertically.
- **The web** still cannot make a user scroll wait. That is the declared deviation of LLP 1050.000 §2.3, on both axes.

### 3.4 H4: anchoring on the main axis

The runner keeps a list's first visible key and its offset within the row through measurement, as vertical lists do. The glue already sets `overflow-anchor: none` on the list (`collection-glue.js:371`).

Chrome does not anchor on the inline axis (§2), so this is a **declared deviation** for horizontal lists, entered in LLP 1001's list:
- **Where it could differ.** An app-visible size change of a mounted card left of the viewport shifts the strip in Chrome. The runner holds it still.
- **Why it is declared anyway.**
  - Every size change a virtualized list observes off-screen is its own artefact: a first measurement replacing an estimate, or a remount re-measuring.
  - The eager oracle, which lays out every card from the start, shows neither.
  - Without anchoring on the main axis, a strip jumps each time a card left of view is first measured.
  - The runner cannot tell a real change from an artefact.

## 4. Nesting

### 4.1 N1: an inner list's lifetime is its outer row's

**The runner.** An inner list is a `Collection` inside its outer row's instance tree. It is created when the outer row is realized and dropped when the row retires, with its rows, slots and views. No inner list outlives its outer row, on any host.

**Recycling.** A host's recycling of the outer row's views (LLP 1068) never carries an inner list:
- today a row holding a live `UIScrollView` is ineligible to pool (`NodePoolIOS.swift:153–163`);
- when LLP 1068 §4.2 pools a scroll view at rest, it resets the offset to the start edge. The inner list's rows are destroyed with the outer row (below).

**Outer row retired** (scrolled out of the window, or its item left the data): the inner collection's rows are destroyed in the same batch, then its list node. `item_left`'s exit animation (`views.rs:61–72`) applies to the outer row only.

**Inner list scrolled while its outer row is pinned or off-screen**: it keeps working. Its geometry reports come from its own port, as for any list.

### 4.2 N2: what survives the outer row's retirement

| State | Survives? | Why |
|---|---|---|
| Realized inner rows, their slots and views | No | LLP 1010 §6.2: row state dies on retirement; re-creation is removal then insertion (LLP 1068 §2) |
| The inner scroll offset | **No** (Q1) | Chrome resets a removed box (§2); SPEC's carousel resets on recycle. An app that wants it keeps it in keyed data (below) |
| Measured inner sizes and the key index | **Yes**, as a bounded cache (N3, N4), used only as estimates | Not observable as app state; brings geometry *closer* to the eager oracle, which always has true sizes; removes the O(N) key pass on a revisit (§5.3) |

**The kept index** is the inner list's `SizeIndex`: its keys, positions and measured sizes. It is kept with:
- the identity of the items it was built from (the same `Rc<[Value]>`);
- the `cross` size and typography epoch its measurements were taken at.

On re-creation:
- **Same items, same epoch:** the index is adopted whole. No key is evaluated, and measured sizes seed the geometry.
- **Different items, same epoch:** only the sizes are adopted, by key, as estimates.
- **Otherwise:** nothing is adopted.

In every case a mounted row is measured again: a cached size is an estimate, never an authority. This is invisible to the app, which sees a first frame nearer the truth.

**Keeping the offset, if the app wants it.** The app writes the list's `scrollLeft` (or `scrollTop`) from its own keyed data, updated by the list's scroll event, as the xheavy thread keeps its draft. An authored offset builds its rows before it moves (LLP 1050.000 stage 2; the web's `jumpTo`, `collection-glue.js:227–233`, becomes axis-generic).

With the kept index, the restored offset lands on the same item. Without it, estimates can land it a little away. Uniform thumbnails, whose estimate is exact, land exactly either way.

### 4.3 N3: keyed by what

The cache lives **in the outer `Collection`** and dies with it (navigation away, the outer list's data replaced). An entry's key is:
- **the outer row's key** (`listItemKey`), not its position, so an insert above does not misfile it;
- **the inner list's slot**: its plan node id, plus the keys of any eager `each` between the outer row root and the list (usually none).

It is the plan node, not a child index, because `when`/`match` arms choose which list exists: a row that changes kind must not inherit another kind's index.

An outer data change that removes a key drops its entries.

### 4.4 N4: memory bounds

**Live state.** It is the outer window's inner lists, each with:
- O(its window) rows;
- O(N_inner) index: keys, positions and sizes, about 40 bytes an item by `QUEUE.md`'s count (entry 1, item 5);
- the O(N_inner) item values the row body evaluated.

At SPEC's 1366 × 1024 landscape, rows of about 400 pt and an outer window of three to five viewports (overscan plus lead), that is about 8–13 feed rows: 2–4 filmstrips and 1–2 inboxes live, about 0.2–0.4 MB of inner index before the items themselves.

**Kept state.** At most **8 entries and 512 KiB per outer collection**, least recently retired evicted first. The numbers are trial values, to be replaced by the stage-3 measurement.
- A 2,000-item filmstrip's index is about 80 KB, so the byte cap binds first: about six filmstrips, or 36 feed rows of scroll-back.
- **Only non-default sizes are kept** where the index is not adopted whole. Uniform thumbnails whose measurements equal the estimate keep none.

**Dropped.**
- All of it on a memory warning (iOS `didReceiveMemoryWarning`, the path `NodePool.reset()` uses) and on backgrounding.
- An entry whose `cross` or typography epoch no longer matches.

**Not O(visited).** Kept state is bounded by the caps, not by how many outer rows were visited. This is the bounded form LLP 1010 `:339–340` allows ("a keep-alive map that grows with every visited key fails").

**Named, not hidden.** The feed's 1.25 million inner items are app data, O(N) input (LLP 1010 §6.1). The port decides whether its data module materializes them all at load or per row when asked. The first costs memory; the second costs time at each outer row's build (§5.3). This RFC does not add a lazy sequence value type (§11).

### 4.5 N5: pins chain

A focus or interaction pin pins the row containing it **in every enclosing list**, innermost first. A focused field in an inbox message pins that message and the inbox's feed row. Otherwise the outer list would retire the row holding the focus, which is exactly the case the current refusal cites (`collection.rs:62–63`).

- **Bounds.** LLP 1010 §6.2's "at most one focused row and one other interacting row" becomes at most one focus chain and one interaction chain: four pinned rows at depth two.
- **Transfers.** The per-category transfer rules stay (§6.5, `:756–763`), applied per chain: a new owner in a category releases the old chain.
- **Code.** `find_collection_mut` and `release_other_pins` descend into mounted collection rows (`traversal.rs:383`, `:415`). Linux's `pin_owner` takes the innermost owner and builds the chain.

### 4.6 N6: one level

A virtualized list may appear in a virtualized list's row template, including under `when`, `match`, `each` and component uses. It may not contain another.
- Depth two bounds the pin chains (N5), the budget order (F4) and the test matrix. No consumer asks for more.
- The eager-inside-virtual case stays allowed, as today.

## 5. Fill policy for inner lists

### 5.1 F1: what an inner list owes

- **The rule.** An inner row is **owed** when two things hold: it is in its own list's visible band (LLP 1050.000 §2.1), and its outer row is owed. The outer row is owed when it is visible, pinned, or at a jump's target.
- **Not computed.** The inner port's clipping by the outer port is not intersected further. An inbox half off the bottom of the feed owes its whole visible band: at most one inner viewport over, about seven messages. Exact clipping is not worth its geometry.
- **The table applies as it is.** D1 (never blank where the host can wait) and D3 (a costly row is not built mid-fling) apply to inner lists as the table states them. "User motion" for an inner row means its own list's motion **or any ancestor's**.

### 5.2 F2: optional rows wait for the ancestors

- **During an ancestor's motion.** While any ancestor list moves (drag, fling or wheel travel), an inner list reports with `limit: Some(0)`: it builds what it owes and no overscan or lead.
  - A feed flung at 24,000 pt/s passes about 60 rows a second, 15 of them nested. Each would otherwise build three viewports of thumbnails for a strip that is gone in a quarter of a second.
- **At rest.** When the ancestors are at rest, the inner list fills its overscan like any list.
- **Who decides.** The host, which knows the ancestry through the nearest-owner walk it already has (`Collection.swift:232–253`, `collection-glue.js:109–120`). The runner needs no nesting rule for the budget.
- **Bootstrap is the port, not 16 rows.** Today a list mounts up to 16 provisional rows before its first geometry report (LLP 1010 §6.5). An inner list's port size is known from the outer row's own layout, so its bootstrap is `ceil(port_main / estimate)` rows:
  - a 600-pt strip of 128-pt thumbnails bootstraps 5, not 16;
  - Apple's report-layout-report loop (`Host::list_viewport`, LLP 1010 §6.2 "one report settles") gives the inner list real geometry in the same batch as its outer row;
  - the web reports it before paint.

### 5.3 F3: an outer row's cost includes its inner lists

Building an outer row that holds an inner list costs:
- its own views;
- the inner list's owed rows;
- the inner list's setup:
  - evaluating its items expression, which is O(N_inner) values;
  - evaluating N keys and building the index, O(N log N);
  - the validation of every key that LLP 1010 §6.2 requires.

*Inference, to be measured in stage 3:* 2,000 keys and index entries are a few tenths of a millisecond to about a millisecond of runner time. The item values are the data module's (Rust or TypeScript). A filmstrip row may therefore cost a slice (1–4 ms, `ScrollPumpIOS.swift:48`) on its own.

Consequences:
- **The cost memo covers the row whole.** It is per outer key (LLP 1050.000 §6's D3 memo, not yet built, `QUEUE.md` "The rest of LLP 1050.000 stage 1"). The outer row's recorded cost is the whole build, inner setup included. A filmstrip row over the slice is pending mid-fling under D3 and built at rest: never blank at rest, and honest about the fling.
- **The kept index (N2) removes the key pass on a revisit** within its caps. It does nothing for a first visit. The first-visit remedy is cheaper setup (§11 lists what is not proposed), or the author's data returning the same items value per row, so identity holds.

### 5.4 F4: one budget, one order

The per-frame slice stays one per host, shared by every list. Its order:
1. owed rows of every list, outer first (pins, visible, jump targets);
2. the outer landing viewport, when a rest offset exists;
3. the outer lead;
4. a moving inner list's lead;
5. overscan of lists at rest.

The host budgets change to match:
- iOS's two feedback passes per main-queue turn (`Collection.swift:49–59`) become two passes that each visit every dirty list once, round-robin as today.
- The web's four reports per frame (`collection-glue.js:146`) become one report per dirty list per frame.
- iOS's text lead and raster throttle stop assuming one list:
  - the lead follows each list along its own axis;
  - the throttle holds while any list travels past 20,000 pt/s (`ScrollPumpIOS.swift:253–258`).

### 5.5 F5: inner flings never blank where the host can wait

An inner list gets its own rescue, on its own axis. On iOS the synchronous rescue in the scroll callback (`Collection.swift:260–280`) already runs per scroll view. It becomes axis-generic through `covers` and `rowsToCover` (`CollectionIOS.swift:77–119`), whose 64-row cap stays. macOS's `preparedCover` clamps each nested scroller's responsive overdraw to its own built rows, on its own axis (D5 of LLP 1050.000). The web declares its gap, as for any list.

## 6. Scroll gestures

### 6.1 G1: chaining at gesture start, per host

| Host | Today | Change |
|---|---|---|
| Web | The browser chains; `overscroll-behavior` passes through (`css.rs:1–13`) | None. The browser is the oracle |
| iOS | UIKit: an inner scroll view's pan takes the whole gesture and rubber-bands at its edge. There is no `overscroll-behavior` handling (`ScrollViewIOS.swift:32–49`) | Under `overscroll-behavior: auto` on the drag's dominant axis, the inner pan **does not begin** when the inner is at its edge in the drag's direction, so the outer's pan takes the gesture. This is Chrome's "starts at the edge → the outer" |
| macOS | `ChainingScrollView` routes each event by its dominant axis. An `auto` axis with room scrolls by hand; otherwise the event goes to the next responder (`:46–145`) | **Latch a phased gesture**: the view that took its `began` event keeps every event through momentum's end. Phase-less events route one by one, as Chrome's ticks did |
| Linux | `wheel_at` chains by dominant axis per event; authored `overscroll-behavior` is not read (`presenter.rs:1039–1108`) | Read `contain`/`none`: no chaining on that axis |

**iOS in detail.**
- `contain` begins the inner pan and keeps UIKit's bounce, which is the platform's overscroll affordance that CSS leaves to the user agent (LLP 1008 `:671`).
- `none` begins it with bouncing off. UIKit's `bounces` is per view, not per axis; a single-axis list does not care.
- There is no mid-gesture hand-off on either platform: Chrome drops the rest of the gesture (§2), and UIKit keeps it in the inner view.
- *Declared:* under `auto`, a drag that begins at an inner edge outward no longer shows the inner view's rubber band, because it chains. That is Chrome's behaviour, and iOS Safari's with `overscroll-behavior` (Q3).
- UIKit's own behaviour, and the change, are held to a device run with real fingers (§8). Neither this session nor the agent can synthesize a UIKit pan (`QUEUE.md`, the Apple entry).

**The macOS latch** is Chrome's wheel scroll latching for trackpads. CDP cannot send phases (§2), so it is checked by hand in Chrome and in the Mac host before landing (§8).

**Diagonal phase-less ticks** stay dominant-axis on macOS and Linux (§2). If the hand check shows Chrome's trackpad does the same, there is nothing to declare. If it does not, the difference goes into LLP 1001's list.

### 6.2 G2: direction locking

There is none to add.
- **Orthogonal nesting** (a horizontal strip in a vertical feed) is how the App Store's carousels work. The inner pan fails for a vertical-dominant drag when the inner has no vertical travel, and the outer takes it.
  - exact2 already depends on this. LLP 1008 "Orthogonal carousels" turns off forced vertical bounce on a scroller with only horizontal travel, because forcing it swallowed vertical input.
  - Chrome's touch picks the dominant axis's scroller the same way (§2).
- **`isDirectionalLockEnabled`** constrains one scroll view's two axes. CSS has no equivalent, and a single-axis list does not need it.
- **The author's lever** is `touch-action` (LLP 1057): `touch-action="pan-x"` on a strip hands every vertical-dominant touch to the feed. It is already intersected at gesture start on iOS (`ScrollViewIOS.swift:39–45`).

### 6.3 G3: the agent

No new operation: `rules/DEFERRED.md` says "a new input is a form of `tap`".
- **Scroll one inner list.** `tap <testId> wheel <dx> <dy>` where the testId is the inner list, or a row inside it. On every host the wheel starts at that box's centre and chains by the host's rule:
  - iOS: `AgentIOS.swift:560–588`, the first `ScrollView` that can move in the dominant direction;
  - macOS: `hitTest` then `scrollWheel`;
  - web: CDP `mouseWheel`;
  - Linux: `wheel_at`.

  A tick at an inner list's end moves the feed. That is the chaining test.
- **Flings and latching.** On the web, `tap … down` and `pointer move` contacts (`agent.mjs:261–289`) make Chrome fling and latch for real. Native touch flings stay a device run.
- **`state`.** Each collection snapshot, nested ones included (`collections_json` already descends), gains `axis` and `parent`: the outer list's view and the outer row's key. A pending interval (LLP 1050.000 §6) names its list. These are fields in an existing reply.
- **`layout` and `clock settle`.** `layout` already reports every scroller's `sx`/`sy`. `clock settle` fills every list, inner ones included, as today (`ScrollPumpIOS.swift:262–272`).

## 7. Host implementation

**Everywhere:** v3 feedback (H1), the axis in geometry, and nested ownership through the nearest owner.

| Host | Work |
|---|---|
| Web (`collection-glue.js`) | `portOf` tests `overflowX` for a row list (`:92–97`); `geometry`, `rowsToCover`, `move`, `jumpTo` read `scrollLeft`/`clientWidth`/`left`/`paddingLeft`; wrappers measured by width. The per-frame budget of F4. Gestures: nothing |
| iOS | `CollectionIOS.swift`: `geometry`, `correct`, `covers`, `rowsToCover` by axis; `contentSize.width` for a row list. `ScrollPumpIOS.swift`: velocity per list along its axis (`:80–96`); the per-list text lead and the any-list throttle (F4); ancestors' motion gates an inner `limit` (F2). `ScrollViewIOS.swift`: the `overscroll-behavior` begin rule (G1). `correct()` stays per scroll view: an inner horizontal correction does not disturb an outer vertical fling (`CollectionIOS.swift:69`) |
| iOS pool (LLP 1068) | A row holding a live scroll view stays ineligible (whole-row refusal; 1068 stage 1 does not change it). **Inside** one live inner list, the pool already works: an inner row's root sits in a collection-owned list's scroll view (`NodePoolIOS.swift:109–114`), so filmstrip thumbnails recycle during an inner fling. Across inner lists they do not: parked trees are dropped when their list leaves (`:85–90`). Pooling by the inner template's shape across inner lists, and pooling outer rows with their inner scroll view (1068 §4.2), are stage 4, only if measured (§9) |
| macOS | `CollectionMac.swift`: `preparedCover` and `KnobDrag` by axis (`:83–254`); `PresenterMac.swift:325–335` travel by axis. `ChainingScrollView`: the phased-gesture latch (G1). No pool (LLP 1068 §9) |
| Linux | `presenter/collection.rs`: `offset.0`, `padding_left` and `requested_left` for a row list; `paint/region.rs:61–69` clamps x by `collection_max`; `pin_owner` innermost (N5); `wheel_at` reads `overscroll-behavior` (G1). Fill stays unlimited: Linux has no fling (LLP 1050.000 §7) |
| Runner | The rename (H1), row-axis wrappers and spacers (§3.1), nested lookup and pin chains (N5), the kept index (N2–N4), port-sized inner bootstrap (F2), `parent` in snapshots (G3), and the creation check of `traversal.rs:443–476` changed to allow depth one |

## 8. Contract surface and compile rules

The Extra Heavy feed, as an author writes it. The kinds are one `column` with a `when` per kind, as the heavy bench's port does:

```contract
list virtualized=true flex=1 estimated-item-height=420 testId="feed"
  each post in feed.rows key=post.id
    column width="100%" padding-top=16 padding-bottom=16
      RowHeader(post=post)
      when post.kind == "filmstrip"
        list virtualized=true display="flex" flex-direction="row" height=132 overflow-y="hidden" estimated-item-width=128 testId=`strip-${post.id}`
          each t in stripItems(post) key=t.n
            Thumb(t=t)
      when post.kind == "inbox"
        list virtualized=true height=280 overflow-x="hidden" estimated-item-height=64 testId=`inbox-${post.id}`
          each m in inboxItems(post) key=m.n
            Message(m=m)
```

**Compile rules replacing the two refusals** (`contract/lower/src/collection.rs`):

| Rule | Error id | Replaces |
|---|---|---|
| A virtualized list inside a virtualized list's row template is allowed at depth one; a virtualized list inside *that* list's rows is refused | `lower-collection-depth` | `lower-collection-nested` (`:72–76`) |
| An inner list's main-axis size must not depend on its content: `height` or `max-height` for a vertical inner list. `flex` alone is not enough in a row, whose height is its content's | `lower-collection-unbounded` (widened) | — |
| A row list needs a definite `height` (§3.2) | `lower-collection-cross` | — |
| `flex-direction` requires `display="flex"`; only `row` is accepted; `display` is `block` or `flex` | `lower-collection-flow` | `:127–131`, `:118` |
| `row-reverse`, `column-reverse`, `flex-wrap` other than `nowrap`, `grid-template-*` and `direction="rtl"` on a row list are refused, each with its reason | `lower-collection-flow` | `:127–131` |
| Main-axis padding must be zero; cross-axis padding is allowed | `lower-collection-flow` | `:100–108` |
| `overflow-x` must scroll on a row list (`overflow-y` may be `hidden`), and the reverse on a vertical one | `lower-collection-flow` | `:120–123` |
| `estimated-item-width` on a row list only, `estimated-item-height` on a vertical one | `lower-collection-estimate` | — |

The runner's creation check (`traversal.rs:443–476`) mirrors the depth rule, including inactive arms.

## 9. Tests and measurement

Nothing new is added as apparatus: no script, check or harness. Tests go in the existing test files. Where a Rust file is near its 1,500-line cap (`collection/tests.rs` is at 1,445), they go in a new test module beside it, as `fill_tests.rs` was added.

- **Runner.** Unit tests in the existing collection test modules:
  - the rename is proven by the existing suites passing unchanged;
  - row-axis wrappers and spacers;
  - the depth refusal;
  - pin chains across two levels, and chain transfer;
  - inner lifetime: outer retirement destroys inner rows in the same batch; no inner row outlives it;
  - the kept index: adopted when items are the same `Rc`, sizes-only otherwise, dropped on epoch change, the caps honoured over 1,000 outer rows (the kept-state count is flat after the eighth);
  - port-sized bootstrap;
  - `limit: Some(0)` builds exactly the owed rows.
- **Compiler.** The table of §8 as cases, beside the six existing collection cases.
- **Chrome pins in `host/web/collection.test.mjs`**, the test file that already drives Chrome over CDP:
  - §2's table as assertions (wheel chaining, `contain`, touch latching, edge-start chaining, re-creation at 0, block-axis-only anchoring), so a Chrome change that moves the oracle fails a test rather than a memory;
  - eager-versus-virtualized parity: the same nested fixture with the inner `virtualized=false` and `true`, and the mounted rows' `getBoundingClientRect` equal at a set of inner and outer offsets.
- **The fixture** is one Contract file in `contract/corpus/` (the directory LLP 1010's `scroll.contract` lives in): a vertical list of 200 rows whose every fifth row holds a 500-item horizontal strip and every seventh a 300-item vertical inbox. The tests' own plans and the agent use it. It is a fixture, not a harness.
- **Cross-host parity**: the same fixture on web, macOS, iOS (simulator) and Linux, driven by the same agent script, comparing `state`'s snapshots and `layout`'s boxes at the same offsets.

  ```
  bun scripts/agent.mjs <host> tree "tap strip-5 wheel 900 0" state layout \
    "tap inbox-7 wheel 0 4000" "tap inbox-7 wheel 0 120" layout "clock settle" state
  ```

  The last wheel ticks past the inbox's end, and `layout` shows the feed moved.
- **By hand, before G1 lands:**
  - Chrome with a real Mac trackpad over the fixture: phased latching, diagonal routing;
  - the Mac host with the same;
  - an iPhone and the iPad with real fingers: the edge-start rule, `contain`'s bounce, orthogonal drags. `QUEUE.md`'s Apple entry names this gap already.
- **Apple XCTests** in the existing `CollectionMacTests.swift` and `NodePoolIOSTests.swift`:
  - axis geometry;
  - the latch state machine fed phased events;
  - `gestureRecognizerShouldBegin` given edge and velocity;
  - inner rows parking in the inner list during an inner fling.

**On `~/bench/xheavy`** (outside the repo, LLP 1050.000 D4). A new probe scenario, `innerfling`:
- **Setup.** Launch at a `BENCH_START_INDEX` whose screen shows a filmstrip and an inbox, with the feed at rest.
- **Finding the scrollers.** The probe finds the inner scroll views the way `findScroll` finds the feed (`probe.m:229–240`), with the opposite filters:
  - the filmstrip: bounds < 300 tall and `contentSize.width` > 20,000;
  - the inbox: `contentSize.height` > 20,000 and the bounds of the inbox.
- **Driving.** Each inner scroller's `contentOffset` is driven through `fling`'s constant-speed segments (±1k–24k pt/s, 2 s each), then `ladder`'s (to 96k), one scroller at a time. It is the same programmatic drive the probe gives the feed, which the host reads as an unclassified user scroll: owed, as a drag.
- **Measured per segment:**
  - fps, p95/p99 frame time and main-thread busy ms/s, as for `fling`;
  - blank area measured over the **inner** scroller's rect: vertical ink-free bands for the strip, horizontal for the inbox;
  - footprint at start, peak and end.
- **Also measured:**
  - the existing `fling` and `ladder`, now crossing 750 nested rows, with their blank and busy numbers read against the pre-nesting stream (`gen.py` keeps the 17-kind rows' own stream);
  - footprint after twenty full inner traversals of one strip (flat: O(window) inner rows);
  - after scrolling the feed past 100 nested rows (flat: kept state at its cap).
- **The comparison.** SwiftUI is `ScrollView(.horizontal)` + `LazyHStack`, and a fixed-frame `ScrollView` + `LazyVStack` in a `List` row; Expo is nested `FlashList`s. Each is its stack's ordinary choice, as SPEC's Decisions table does.

## 10. Staging

**Horizontal first, then nesting, each landing on its own.**

| Stage | Ships | Done when |
|---|---|---|
| 0 | Nothing. The xheavy port ships the strip as a plain horizontal `scroll` of 2,000 cards and the inbox as an eager inner `list` (GAPS gap 8's fallback), and is measured | The baseline for every later stage |
| 1 | The axis refactor (H1): rename, wire v3, all hosts. No behaviour change | Every existing collection test and smoke passes unchanged, including Markdown's reader on the collection if "One list engine" has moved it |
| 2 | Horizontal lists at top level (H2–H4): compile rules, row-axis views, host geometry, the Chrome pins, the fixture's strip | A 25,000-item top-level strip windowed on web, macOS, iOS and Linux with O(window) rows after twenty traversals; Chrome parity; the agent's `tap strip wheel` |
| 3 | Nesting (N1, N5, N6, F1–F5, G1–G3); the kept index (N2–N4) only if the stage's measurement of outer-row setup shows the key pass matters | The fixture on four hosts; the runner tests; the device and trackpad hand checks; xheavy `innerfling` and `fling` against stage 0 |
| 4 | Only if stage 3's numbers ask: iOS pooling of inner rows across inner lists by shape, and of outer rows with their scroll view (LLP 1068 §4.2) | xheavy against stage 3 |

**Why horizontal first:**
- The filmstrip needs both halves. The inbox needs only nesting.
- Stage 1 is mechanical, and the existing vertical suites prove it.
- Nesting's hard parts (pins, lifetime, budget, gestures) are axis-free. Built on the axis-generic seam, they are written once.
- Built first on the vertical-only seam, they would be rewritten by the refactor.
- The inbox waits one stage. Q4 asks whether that is the right trade.

## 11. What this does not do

- **Not built:**
  - grids, masonry, wrapping or reversed (inverted) virtualized lists;
  - visible-column windowing (LLP 1050.000 §3);
  - nesting deeper than one level;
  - RTL horizontal lists;
  - `gap` on a virtualized container.
- **No host keep-alive**, by row key, of an inner list, its views or its offset (Q1). No offset cache in the runner.
- **No lazy sequence value type.** An inner list's items are a list value the row body evaluates. A range or generator value that the index could read without materializing N records would cut §5.3's setup cost. It is a new value type, and a separate proposal if the stage-3 numbers ask for it.
- **Selection and copy stop at a list's boundary.** LLP 1010 §6.2's logical selection stays per list.
- **Nothing reorders across lists.** `reorderdrop` stays per list.
- **No new agent operation, check, script or harness.**
- **No change to the web's gestures.** The browser is the oracle.

## 12. `rules/DEFERRED.md`

`rules/DEFERRED.md` keeps virtualList v2 out ("cert wires, extent demand, proxy lanes") and admits "a straightforward windowed list with bounded row/view lifetime" (2026-09-14). Nested and horizontal lists are that windowed list on a second axis and one level down, with the same lifetime rule. They are not virtualList v2's machinery.

They are still new capability, so the entry says so. Proposed, beside the virtualList line:

> **Expanded (Charlie, 2026-09-__, LLP 1070):** horizontal windowed lists (`display: flex; flex-direction: row`) and one level of nesting, a windowed list in a windowed list's row, with the inner list's lifetime its outer row's. Unblocks the Extra Heavy feed's filmstrip and inbox, and any feed of carousels. Take: the legacy windowed list (`item-height`/`estimated-item-height` without `virtualized`, `runner/src/instance/window.rs`) is deleted before nesting lands, so nesting is built on one engine. Still out: grids, masonry, wrapping and inverted lists; nesting deeper than one level; host keep-alive of an inner list or its offset.

The take is a path removed. `QUEUE.md` already owes it ("One list engine"); this makes it a precondition rather than an intention, which is the precedent LLP 1026 D12 set ("fewer paths after than before"). Charlie may name another take (Q2).

## 13. Questions for Charlie

**Q1. An inner list re-created with its outer row starts at offset 0: the runner does not keep it, and an app that wants it keeps it in keyed data. The runner keeps only a bounded cache of measured sizes and keys, used as estimates. Agree?**
Recommendation: yes.
- Chrome resets a removed box, even the same element re-inserted (§2).
- exact2's lifecycle is removal (LLP 1010 §6.2, LLP 1068 §2), and SPEC's carousel already resets on recycle.
- The eager page keeps it only because it never removes the box.
- Feed apps (the Furrow pattern in LLP 1068's appendix) keep carousel offsets in their model, which is what the keyed-data route is.

Confidence: medium-high (0.7). The alternative, the offset in the kept cache, costs 8 bytes an entry. But it makes a row's state depend on whether its key was recently seen, which LLP 1068 §5.3 rejected for heavy views.

**Q2. Admit horizontal and one-level nested windowed lists (§12), with the legacy windowed list's deletion as the take?**
Recommendation: yes. Confidence: medium (0.6). The deletion is owed already. If that makes it too cheap a take, the alternative is that further Extra Heavy row kinds wait behind these two.

**Q3. On iOS, `overscroll-behavior: auto` at an inner list's edge chains a new drag to the outer list, as Chrome does, giving up UIKit's rubber band on the inner list at that edge. `contain` keeps the band. Agree?**
Recommendation: yes. It is the CSS meaning of `auto`, and the web behaves so. An author who wants the band says `contain`, as both Markdown readers already do (LLP 1033 D4). Confidence: medium (0.65), until a device run with real fingers shows the rule feels right on a phone.

**Q4. Stage horizontal first (axis refactor, then top-level strips), then nesting?**
Recommendation: yes. Nesting's code is written once, on the axis-generic seam. The cost is the inbox waiting one stage. Confidence: medium (0.6). Nesting first would give the inbox sooner, and Messages-style sheets with a list (LLP 1041 §8.5) are vertical too.

## Appendix: the Chrome probe

A scratch Bun script (session scratchpad, not in the repo) that:
- launches Chrome 154.0.8037.57 `--headless=new` over `--remote-debugging-pipe` with `scripts/agent-launch.mjs`'s `Cdp`;
- sets an 800 × 600 viewport at scale 1;
- builds the §2 page with `Runtime.evaluate`;
- drives it with `Input.dispatchMouseEvent` (`mouseWheel`) and `Input.synthesizeScrollGesture` (`gestureSourceType: "touch"`, `preventFling: true`, 400–800 px/s);
- reads `scrollTop`/`scrollLeft` after each event.

A first touch run that set the inner offset and started the gesture in the same turn moved the outer only 5 px. Three reruns with a 300 ms settle moved it 499–501 px, which is what §2 reports. The recorded Chrome process was killed by PID.

Not covered: phased trackpad input (CDP has none), real touch on a phone, Safari.
