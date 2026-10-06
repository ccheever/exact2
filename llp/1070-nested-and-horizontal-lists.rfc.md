# LLP 1070: Nested and horizontal virtualized lists

**Type:** RFC
**Status:** Accepted (Charlie, 2026-09-27; rulings in §0.1, questions answered in §13)
**Systems:** Contract (`contract/lower/src/collection.rs`: the two refusals this replaces), Runner (`runner/src/instance/collection/`: the axis, the nested lifetime, pin chains, the size cache), Web host (`collection-glue.js`, `glue.js`), Apple host (`Collection.swift`, `IOS/CollectionIOS.swift`, `IOS/ScrollPumpIOS.swift`, `IOS/ScrollViewIOS.swift`, `IOS/AgentIOS.swift`, `IOS/NodePoolIOS.swift`, `Mac/CollectionMac.swift`, `Mac/ChainingScrollView.swift`), Linux host (`presenter.rs`, `presenter/collection.rs`), Agent (LLP 1012: no new operation)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** a Claude lane (Opus 5.5), `feat/horizontal-lists`, from 2026-09-27: stages 1–3 first; stage 4 (nesting) follows on the same lane
**Date:** 2026-09-27 (r1 and r2)
**Related:**
- LLP 1010 §6 (the collection: row state dies on retirement, `:334–340`; nested virtual rows rejected, `:760–763`).
- LLP 1050.000 D1–D6 (never blank by default; a costly row is never built mid-fling; the owed set, `limit`, retirement). Its stage 1 is partly built; the D3 cost memo (stage 3) and the rest offset (stage 4) are not (`queue/` "The rest of LLP 1050.000 stage 1").
- LLP 1068 (heavy-view recycling; stage 1 is on `feat/heavy-pool-stage1`, not on main; §2's oracle; §4.2 pools a nested scroll view at rest with its offset reset; §5.3 rejects a keyed keep-alive).
- LLP 1008 "Orthogonal carousels" (`:678–697`) and "Short vertical scroll containers" (`:664–675`); LLP 1033 D4 (`overscroll-behavior` as a kernel row); LLP 1057 and 1057.001 (`touch-action` is the arbitration model).
- `~/bench/xheavy/SPEC.md`, `EXACT2-GAPS.md` gap 8, `gen.py:483–523` (the two new kinds).
- `queue/` "One list engine", the crypto-memory entry item (5) (the index's bytes per item), the Apple entry "a pan chaining out of a nested scroll view at its edge".
- `rules/DEFERRED.md` §Components (no virtualList v2; the windowed list admitted 2026-09-14).
- Reviews: `llp/reviews/1070-nested-and-horizontal-lists.{astra,grok}.md`.

## Summary

The Extra Heavy feed gains two row kinds (`gen.py:516`):
- **`filmstrip`**, every sixth row (500 of 3,000): a horizontal virtualized strip of 2,000 thumbnails;
- **`inbox`**, every twelfth row (250): a fixed-height box holding its own vertical virtualized list of 1,000 messages.

That is 1.25 million inner items behind one feed. Charlie wants it as "an even more ridiculous stress test".

exact2 refuses both today. The compiler rejects a virtualized list inside another list's row (`collection.rs:72–76`) and a horizontal virtualized container (`:127–131`). The runner re-checks the first at creation, rejecting any nested `virtualized` that is not the constant `false` (`traversal.rs:443–476`).

The kernel is already axis-generic (`overflow_x`/`_y`, `content` on both axes). Everything vertical-only lives in the collection seam: the runner's `scroll_top`, `port_height` and `HeightIndex`; the three feedback encoders; each host's geometry, `covers` and `rowsToCover`; authored `scrollTop` handling.

**Recommended decisions** (Charlie rules; questions in §13):

| # | Decision |
|---|---|
| H1 | One engine, two axes. The collection's seam becomes main/cross (`offset`, `port_main`, `cross`, `size`), wire v3; no second list type |
| H2 | A horizontal list is CSS's: `display="flex" flex-direction="row"` with a literal `height`. Reverse, wrap, main-axis alignment other than `flex-start`, `gap` and RTL (authored or inherited) are refused by name |
| H3 | LLP 1050.000's owed-set table applies unchanged on the main axis |
| H4 | The runner anchors on the main axis, both axes. On a horizontal list it anchors only when an estimate is replaced by a first measurement; a re-measured card shifts the strip as in Chrome (Q3, ruled (a)) |
| N1 | An inner list lives and dies with its outer row, on every host |
| N2 | An inner list's scroll position survives its outer row's retirement by default, kept by the runner as an anchor key and an offset within it, bounded by recency; `scroll-restoration="manual"` opts out (Q1, ruled §4.2). Rows, slots and views do not survive. Optionally, a bounded cache of measured sizes seeds estimates on re-creation, built only if stage 4 measures a visible benefit |
| N3 | The cache is keyed by the outer collection, the outer row's key, and the inner list's plan node plus any eager `each` keys between them |
| N4 | Bounded by bytes counted over every retained allocation; the live index is ~220 bytes an item today and is the larger number |
| N5 | A pin chain: the addressed list and every ancestor whose mounted row contains the pin view. The runner derives it, transfers it atomically, and releases only lists off the new chain |
| N6 | One level of nesting, only a constant `virtualized=true`. Deeper or dynamic is refused |
| F1 | An inner row is owed when its own list shows it and its outer row is owed |
| F2 | An ancestor-moving flag in feedback: while set, an inner list builds only what it owes, even on first geometry. Bootstrap is a defined port estimate |
| F3 | Until LLP 1050.000 stage 3 lands, a visible nested row, its O(N) key pass included, is built during an outer fling: declared and measured, not hidden |
| F4 | The hosts' per-turn and per-frame report caps stay; they are spent outer owed, inner owed, then leads |
| F5 | A newly created inner port reports before the native frame that shows it; coverage includes inner ports |
| G1 | Chaining is CSS's: decided at gesture start, latched, never mid-gesture. iOS fails an inner pan only for a known outward direction at an edge; macOS latches a phased gesture |
| G2 | A phase-less wheel follows Chrome on every host: the deepest scroller that can take any component takes it, and the rest is dropped |
| G3 | No new agent operation: `tap <testId> wheel dx dy` addresses an inner list; `state` shows its snapshot with its parent |
| S | The legacy windowed list is deleted first (the take), then the axis refactor, horizontal lists, nesting, and the size cache only if measured |

## 0. What r2 changed (reviews by Astra and Grok)

Both reviews: "build with named changes". Dispositions are in the review files. Both independently found the same blocker and several of the same errors:

- **The pin chain was wrong and dangerous** (both, BLOCKER). Feedback releases other collections' pins before it applies the report (`traversal.rs:234–241`), and a release re-realizes at once (`mod.rs:1012–1038`). r1's "descend in `release_other_pins`" would retire the outer row that holds the focus. N5 now defines the chain, derives it in the runner, preflights it, and releases only lists off it.
- **F1–F3 described LLP 1050.000 behaviour that does not exist** (both). There is no cost memo, no rest offset and no fill policy in the tree; `limit` never rations owed rows, and first geometry, resizes and pin changes override it to unlimited (`mod.rs:835–851`). F2 is now a runner rule with an ancestor flag; F3 states that a visible nested row is built mid-fling until 1050.000 stage 3 lands.
- **The index's size was off by five** (Grok): ~220 bytes an item today, not 40. N4 is rewritten around it; the kept index is cut to sizes only.
- **Whole-index adoption was unsound** (Astra): keys can depend on inputs outside the items value (`mod.rs:250–262`), and cached zero sizes would hide rows. N2 keeps positive sizes as estimates only.
- **The Apple settle loop r1 cited is the legacy list's** (Astra, `host.rs:962`); the collection makes one runner call (`host.rs:705`). F5 now specifies inner-port settlement, and coverage includes inner ports.
- **Budgets** (Grok): r1's F4 removed the caps that keep a fling inside a slice; r2 keeps them.
- **Gestures**: `touch-action: pan-x` does not hand vertical drags to the feed (both); iOS must not fail a pan whose direction is unknown (Grok); the macOS latch needs a full rule, including zero-delta phases (both); the phase-less diagonal split was already measured and is now decided (G2), not deferred to a hand check (both).
- **Missed sites**: authored `scrollLeft` is not build-then-move anywhere (Grok); `find_collection_mut` breaks inner edge re-arm (Grok); logical copy and reorder do not descend (Astra); the iOS agent wheel ignores containment (Astra).
- **The take** (Astra): `rules/DEFERRED.md` wants the take in the admitting change, so the legacy deletion now precedes the first capability stage.
- **Corrections**: Linux already picks the innermost owner; the iOS raster throttle already covers any list; the iOS pool already matches trees across lists by shape; the bootstrap at estimate 128 is 4 rows; `tests.rs` asserts `scrollTop` in snapshot JSON; `pending` is a boolean.
- **Questions**: staging (r1 Q4) is decided in the RFC, as both suggested; H4 becomes a question (Astra).

## 0.1 Rulings (Charlie, 2026-09-27)

- **Q1, accepted as revised** ("if your rec has changed then let's do your rec"). The r2 recommendation (reset to 0) is withdrawn. An inner list nested in a virtualized list **keeps its scroll position** across its outer row's retirement, automatically and by default, with an opt-out. §4.2 states the rule.
  - *Why.* The oracle for a virtualized list is the same page unvirtualized. There the outer row's element is never destroyed, so the inner box keeps its offset when the page scrolls it away and back (§2, "the eager oracle"). Virtualization must not change what the user sees. Removal is how exact2 implements a list, not what the author wrote.
  - He first said: "leave it up to the app to save position. maybe worth creating an option to do it automatically since that seems more intuitively correct". The revision makes the automatic option the default, since the eager page is the oracle.
- **Q2, accepted:** "yes delete the old windowed list thing, collection list is now better." Horizontal lists and one level of nesting are admitted to `rules/DEFERRED.md`; the take is deleting the legacy windowed list, stage 1.
- **Q3, (a) accepted:** "(a) is ok for now, but we might want to allow an option for (b) at some point." A horizontal list anchors only when an estimate is replaced by a first measurement. (b), anchoring every size change, is recorded as a future option (`queue/`).
- **Q4, accepted provisionally:** "i don't really know, lt's try your rec and see how it feels." Under `overscroll-behavior: auto` on iOS, a new drag starting at an inner edge outward goes to the outer list; `contain` keeps the inner rubber band. Provisional until a real-finger feel test on a device (`queue/`), which may reverse it.

## 0.2 As built: stages 1–3 (2026-09-27, `feat/horizontal-lists`)

- **Stage 1, the take.** `runner/src/instance/window.rs` and `heights.rs`, the runner's `ListViewport`/`ListStatus`/`list_viewport*`, the `itemHeight` prop, the Apple host's settle loop and silent layout pass, `exact_list`/`exact_list_pending` on Apple and the web, the web's windowed reports (`list-selection.js` keeps logical selection only), and the macOS and iOS pumps' legacy covers and `syncLists` are deleted. `item-height` fails with `lower-list-height`, and `estimated-item-height` without `virtualized=true` with `lower-list-virtualized`, each saying how to migrate. Logical copy and key lookup stay, over the collection. The Markdown reader's document is a collection (its top and end space moved onto its first and last rows); `metrics.mjs --list-memory` compares eager with `--virtualized`. Outside the repo, `~/bench/exact2-bench` (a detached checkout) still has the reader on the windowed list: noted, not edited.
- **Stage 2.** The rename of §3.1's table and wire v3: the main axis first, then a flags word (bit 0, `ancestor_moving`) before the row count. No behaviour change; the renamed suites and the six smokes passed.
- **Stage 3.** A virtualized list is horizontal when its `display` is `flex` (CSS's default `row`; `flex-direction`, if written, must be `row`). The snapshot says `"axis":"x"`; wrappers and spacers are §3.1's; `estimated-item-width` is a new prop; the compiler adds `min-width: 0`. §8's refusals are built with their ids, `lower-collection-cross`, `-estimate` and `-reorder` included; RTL is refused as authored only (the runner does not yet re-check an inherited `rtl`). Q3 (a) is in the runner. `state` carries every snapshot (`collections`, G3). Hosts: geometry, corrections, coverage, velocity and knob drags by axis on the web, iOS, macOS and Linux; an authored `scrollLeft` builds before it moves on the web and Linux, and on Apple an agent's `tap far` lands with its cards built, through the scroll callback's rescue (not a build before the move); G2 for phase-less wheels on macOS, Linux and the iOS agent, with `contain`/`none` keeping the tick; Q4's begin rule on iOS (`ScrollView.handsOff`), provisional. H4's motion guard on the web applies to horizontal lists only: vertical lists keep today's behaviour.
- **Parity.** `apps/carousel` (25,000 cards of five widths, estimate 128): stepping the strip from 0 in 300-px wheel ticks to 4,500, every card box in the port equals the unvirtualized page's, in closed form and on the same host's eager strip, on the web (Chrome), macOS, iOS (simulator) and Linux (61–62 boxes at 16 offsets each, 0 mismatches). After twenty traversals 9–16 cards are mounted. A `(60, 100)` tick over the strip moves its x by 60 on all four; `(0, 120)` leaves it. `host/web/collection.test.mjs` pins §2's Chrome rows the hosts rely on (wheel chaining over a strip, no inline-axis anchoring, re-creation at 0).
- **Stage 4 (nesting), as built.** One level, a constant `virtualized=true` only: the compiler refuses a second level (`lower-collection-depth`), an inner vertical list without a literal `height`/`max-height` (`lower-collection-unbounded`) and an inner `reorderdrop` (`lower-collection-reorder`); the runner re-checks the template, inactive arms included. An inner list is its outer row's (N1): created with it, destroyed with it in the same batch; its snapshot names its `parent`. **Q1:** when an outer row retires, each inner list not `scroll-restoration="manual"` is kept in the outer list as its first-shown item's key and the offset into it (4,096 per outer list, least recently kept first out; nothing kept for a list at its start; a row key that leaves the data takes its entries); when the row is built again the inner list's window is built there and a correction moves the port before it paints, the kept item held as the anchor until it is measured or the reader moves, so an item whose estimate is shorter than the offset into it still lands. `state` lists `kept` and marks a restored list `"restored":true`. **N5:** hosts report a pin to its nearest owner; the runner derives the chain (an outer list keeps a row whose inner list holds a pin, and a pin's transfer never releases its ancestors). **F2:** hosts set `ancestor_moving` from the outer list's velocity (web, Apple; Linux has no fling); the runner then builds only what the inner list owes, first report included, and its pending reply continues the fill at rest. The bootstrap of an inner list covers the outer port on the same axis, or the outer rows' cross size across it. **G1:** macOS latches a phased trackpad gesture (`ChainingScrollView.latched`). Logical copy finds an inner list. The agent's `clock settle` now drains every collection report on Apple and Linux, as on the web. **Parity:** `apps/carousel`'s feed page (200 rows; every fifth a 200-card strip, every seventh a 120-message inbox of varied heights), virtualized against the same page unvirtualized on the same host, over 30 steps (strip and inbox wheels, 24 feed ticks, a round trip that retires row 0 and row 7 and restores them): 0 mismatched boxes of 659–696 on Chrome, macOS, iOS and Linux; restored strip and inbox land with their items exactly where the unvirtualized page has them. Not compared: the far end of an authored 6,000-px jump, which lands by estimates (§4.2). **Owed:** the hand gates (real fingers on an iPhone and iPad for Q4; a real trackpad for the latch), F4's budget order and F5's settle-before-commit for a newly created inner port (the bootstrap covers it instead), and xheavy's `innerfling`.
- **Next on this lane, after stage 4** (Charlie, 2026-09-27): scroll-to-row, `Element.scrollIntoView()`'s options aimed at a row by key, its own short LLP first (`queue/`).
- **Owed from stage 3.** The runner's re-check of an inherited `rtl` (§3.2); a true build-then-move for an authored offset on Apple; the iOS real-finger feel test (Q4); the macOS phased-gesture latch (G1) and nesting are stage 4; `overflow-x="auto"` is not a Contract value (the kernel has no `auto`), so a strip is written `overflow-x="scroll"`; the fixture's plan bakes its 27,000 cards (a 1.5 MB plan), acceptable for a fixture.

## 1. What exists

**Two engines, one kept.** `list virtualized=true` is the collection (`runner/src/instance/collection/`). `item-height`/`estimated-item-height` without `virtualized` is the legacy windowed list (`window.rs`), which `queue/` "One list engine" deletes after parity. This RFC extends only the collection, and deletes the legacy list first (§10, §12).

**The collection's shape** (`collection/mod.rs:63–101`, `views.rs`):
- Rows are flow children of the `list` node, interleaved with spacers (`views.rs:108–145`). A spacer is `height: gap; width: 100%; flex-shrink: 0` (`:86–107`).
- A row wrapper is `display: flex; flex-direction: column; width: 100%; flex-shrink: 0` (`:35–45`). Its border-box height is what the host measures. It publishes `listitem`, `aria-posinset` and `aria-setsize` per collection (`:46–84`).
- The index is `HeightIndex` (`index.rs:86–97`): order, `positions: BTreeMap<Rc<str>, usize>`, per-row heights and generations, and a prefix-sum tree. `queue/` measures it at about 1.1 MB for 5,000 items: ~220 bytes an item.
- Bootstrap is `ceil(16 × 32 / estimate)` rows, capped at 16 (`mod.rs:216–220`), before any geometry report (`:546–548`).
- The window is one viewport of overscan each side plus a quarter-second lead, capped at two viewports (`mod.rs:36–54`). Owed rows are the visible band plus pins; `limit` bounds only the rest (`:560–592`), and first geometry, port or width changes and pin changes make a report unlimited (`:835–851`).
- Rows farther than two viewports always retire. Rows kept past the window never outnumber the window's own (LLP 1050.000 §6).

**Already nesting-aware.**
- `collections()`, `find_collection` and `feedback_walk` descend into mounted collection rows (`traversal.rs:69–97`, `:292–358`). `NodeInst::contains` walks into a nested collection (`find.rs:74–77`).
- The web controller keys rows to their nearest owner (`collection-glue.js:109–120`). iOS finds the owning collection through the nearest row wrapper, and says ancestors never pin the outer row as well (`Collection.swift:232–239`). Linux walks from the focused view upward and returns the first row wrapper's owner, the innermost (`presenter/collection.rs:224–239`).
- `CollectionMacTests.swift:282–313` asserts nearest-owner pins and at most one focus.
- A virtualized list inside an **eager** list's row, or inside a plain scroller, is allowed today.
- The iOS pool keys parked trees by shape alone (`NodePoolIOS.swift:196–204`), so a thumbnail parked by one list can be taken by another live list of the same shape.

**Not nesting-aware:**
- `find_collection_mut` does not descend into mounted rows (`traversal.rs:383–384`). `rearm_collection_edge`, `take_collection_end` and `wake_collection_edge` use it (`:260–282`), so an inner `reachend` would fire once and never re-arm.
- `release_other_pins` walks from the root and releases every other collection before the report applies (`traversal.rs:234–241`, `:404–416`).
- Logical copy's `list_region` does not descend into mounted collection rows (`runner/src/instance/text.rs:64–72`). Reorder's `edit_walk` assumes no nesting (`traversal.rs:540–548`), and its preview motion is vertical (`reorder.rs:264`).
- The iOS agent wheel moves the first scroller that can take the dominant direction and reads no `overscroll-behavior` (`AgentIOS.swift:560–588`).

**Vertical-only**, every site:
- *Runner*: `CollectionFeedback.scroll_top`, `port_height`, `row_width` (`api.rs:18–39`); `CollectionRow.top`/`height`; `AnchorCorrection.scroll_top`; the snapshot JSON's `scrollTop` (asserted in `tests.rs:552–556`); spacers and wrappers above.
- *Web*: `geometry()`, `rowsToCover`, `move`, `jumpTo` and `portOf` read `scrollTop`, `clientHeight`, `.top` and `overflowY` (`collection-glue.js:69–108`, `:227–233`); the anchor correction writes `port.scrollTop` (`:381`); only `scrollTop` is build-then-move (`glue.js:815`).
- *iOS*: `contentOffset.y`, `contentSize.height`, `covers` and `rowsToCover` (`CollectionIOS.swift:40–119`); velocity sampling (`ScrollPumpIOS.swift:82–86`); an authored `scrollLeft` goes straight to `contentOffset` (`NodeViewIOS.swift:864–866`).
- *macOS*: `preparedCover` and `KnobDrag` (`CollectionMac.swift:83–254`); `PresenterMac.swift:325–335`.
- *Linux*: `max_top`, `padding_top`, `offset.1`, `model_top`, and only `ScrollTop` read for a collection (`presenter/collection.rs:11–61`, `:186–221`, `:625–635`, `:669–703`); `paint/region.rs:61–69`.

**Shared caps:** iOS allows two feedback passes per main-queue turn, one list each (`Collection.swift:49–59`, `:339–364`); the web four reports per frame (`collection-glue.js:146–152`); Linux two passes (`presenter/collection.rs:8`, `:594`). The iOS text lead follows the fastest list's scalar velocity (`ScrollPumpIOS.swift:253`); the raster throttle already holds while any list travels fast (`:258`).

## 2. The oracle: what Chrome does

**Method.** The oracle is nested `overflow: auto` boxes in Chrome, the same DOM the web host emits.

Measured 2026-09-27 on Chrome 154.0.8037.57 headless, over CDP, with a scratch page (appendix):
- an 800 × 600 outer box of 40 rows;
- an inner box in row 3: vertical, 400 × 200 with 1,200 px of content; or horizontal, 600 × 120, a flex row of 30 cards with 2,640 px of travel.

"Wheel" is `Input.dispatchMouseEvent` `mouseWheel`: phase-less 120 px ticks, 16 ms or 400 ms apart. It is also what the agent's web `tap … wheel` sends (`agent.mjs:306`). "Touch" is `Input.synthesizeScrollGesture` with `gestureSourceType: touch` and no fling.

| Case | Result |
|---|---|
| Wheel down over a vertical inner, ticks until past its end | The inner takes each tick. The tick that reaches its end is clamped (960 → 1000); the remainder is dropped. The *next* tick goes to the outer. Same at 16 ms and 400 ms spacing |
| Same, inner `overscroll-behavior: contain` | The outer never moves (`none` was not measured) |
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

- **Touch chaining is decided at gesture start and latched.** A gesture belongs to one scroller from its first movement to its end: the innermost that can move in the gesture's dominant direction. At its edge the rest is dropped, or rubber-banded where the platform shows overscroll. A gesture that *starts* at an inner edge outward goes to the outer. `contain` stops chaining (G1).
- **A phase-less wheel tick is its own gesture, and is not split.** It goes to the innermost scroller that can take *any* of its components, which takes what it can; the rest is dropped. exact2's macOS, Linux and iOS agent route such a tick by its dominant axis and apply both components (`ChainingScrollView.swift:88–111`, `presenter.rs:1071–1082`, `AgentIOS.swift:560–588`), so a y-dominant `tap … wheel 60 100` over a strip scrolls the feed natively and the strip on the web. G2 follows Chrome.
- **The offset follows the eager page.** Chrome keeps an offset for an element that stays in the document (hidden, moved, or skipped by `content-visibility`), and on an unvirtualized page the row's element always stays. Removal is exact2's implementation, not the author's page, so an inner list keeps its position by default (§4.2; Q1 as ruled).
- **Anchoring is on the block axis only.** H4 and Q3.

The eager oracle and the lifecycle oracle disagree in one place: on an eager page an inner box is never removed, so its offset survives scrolling the page away and back. exact2 keeps the lifecycle oracle for row state (slots, views; LLP 1010 §6.2, LLP 1068 §2) and, by Charlie's Q1 ruling, takes the eager oracle for the inner scroll position, which the user sees and did not ask to lose.

## 3. Horizontal virtualization

### 3.1 H1: one engine, two axes

The collection gains an axis, fixed at creation from the list's resolved style: `display: block` is vertical; `display: flex; flex-direction: row` is horizontal. The seam is renamed to be axis-neutral, with no compatibility shim (RULES: delete, don't deprecate):

| Today | v3 | Horizontal meaning |
|---|---|---|
| `scroll_top` | `offset` | `scrollLeft` from the content's start edge |
| `port_height` | `port_main` | the scrollport's width |
| `port_width` | `port_cross` | its height |
| `row_width` | `cross` | the rows' available height, after padding |
| `RowMeasurement.height` | `size` | the wrapper's border-box width |
| `CollectionRow.top`, `.height` | `start`, `size` | |
| `AnchorCorrection.scroll_top`; snapshot `scrollTop` | `offset` | |
| `HeightIndex` | `SizeIndex` | |

`CollectionFeedback` becomes version 3, with F2's `ancestor_moving` flag. Its three encoders change together: web by hand (`collection-glue.js:6–31`), Apple by hand (`Collection.swift:20–35`), Linux in Rust (`presenter/collection.rs:669–703`). The snapshot gains `axis` and `parent` (G3). Tests that assert the JSON (`tests.rs:552–556`) are updated with it.

The logic is unchanged: `window_led`, `lead`, retirement, anchors, `reachstart`/`reachend` and `scrollFollowEnd` are one-dimensional already.

**The views.** The runner emits wrappers and spacers through the kernel's own styles (`views.rs:4–21`). On a row list:
- the wrapper is `display: flex; flex-direction: column; flex: none; min-height: 0`, with no width, stretched across by the list's default `align-items`; its width is measured;
- a spacer is `width: gap; flex: none; align-self: stretch`.

`flex: none` is structural, as `flex-shrink: 0` is on the vertical wrapper. In CSS a flex row's items shrink by default, so an *eager* strip needs `flex: none` on each card to overflow at all; the virtualized wrapper supplies it.

**The strip's own width.** The whole strip's extent is in its spacers. A strip that is a flex item of a `column` (which is `display: flex`, `tags.rs:69–74`) must not let that extent widen its row: its used width is its container's. The compiler adds `min-width: 0` to a row list (CSS's own fix for a flex item that overflows), and the parity fixture asserts the outer row's width (§9).

### 3.2 H2: the author's surface is CSS's

A horizontal list is `display: flex; flex-direction: row` on a scroll container: how a web page builds a carousel, and the DOM exact2's web host emits.

```contract
list virtualized=true display="flex" flex-direction="row" height=132 overflow-y="hidden" estimated-item-width=128 testId="strip"
  each t in strip.items key=t.id
    Thumb(t=t)
```

- **`estimated-item-width`** is the row-axis twin of `estimated-item-height` (LLP 1010 §6.5): a positive literal, kebab-case host policy, not CSS. The estimate on the other axis is refused. CSS's own name for this is `contain-intrinsic-size`; renaming both is a separate change.
- **`flex-direction` needs `display="flex"`.** CSS ignores it on a block; the compiler refuses that, since a silently vertical list would be the likelier bug. Only `row` is accepted; `display` is `block` (vertical) or `flex` (with `row`).
- **The index assumes starts are prefix sums** (`mod.rs:1055`). So a row list refuses what would move starts:
  - `justify-content` other than `flex-start`/`normal` (CSS flex alignment would offset the first item or spread them);
  - `gap` (refused on both axes, as today); spacing goes on the row root as a nonnegative margin, which the wrapper encloses (`views.rs:33–34`);
  - `row-reverse`, `column-reverse` (an inverted list is its own consumer);
  - `flex-wrap` other than `nowrap` (a grid, which `rules/DEFERRED.md` keeps out with virtualList v2).
- **RTL is refused, authored or inherited.** `direction` inherits (`schema.json:1940`), so the compiler refuses an authored `direction="rtl"` on a row list, and the runner refuses a row list whose resolved direction is `rtl` at creation. Chrome's RTL `scrollLeft` is negative from the right edge; it comes with a consumer.
- **Main-axis padding must be zero** (`padding-left`/`-right` on a row list, as `padding-top`/`-bottom` today, `collection.rs:100–108`). Cross-axis padding is allowed.
- **`overflow-x` must scroll on a row list.** `overflow-y` may be `hidden`, as vertical lists set `overflow-x="hidden"`.
- **The cross size is a literal `height`.** An auto height would be the tallest *mounted* card; the eager oracle's is the tallest of all 2,000, so it would change as cards mount. The main axis needs nothing: an auto width in block flow is the container's, and a shrink-to-fit parent is caught by the bake's measured-layout lint, as for vertical lists (`collection.rs:26–27`).

### 3.3 H3: the fill policy on the horizontal axis

LLP 1050.000's owed-set table (§2.1) applies unchanged, read on the main axis:
- **velocity** is `contentOffset.x` per second on iOS and `scrollLeft` samples on the web; a step larger than the port's width is a jump;
- **D2 (thumb drags wait):** `KnobDrag` on macOS becomes axis-generic; iOS and the web have no signal, as vertically;
- **the web** cannot make a user scroll wait: LLP 1050.000 §2.3's declared deviation, on both axes;
- **the rest offset** (`targetContentOffset.x`) joins the seam only when 1050.000 stage 4 builds landing-first, for both axes at once.

### 3.4 H4: anchoring on the main axis

The runner keeps a list's first visible key and its offset through measurement, as vertical lists do; the glue already sets `overflow-anchor: none` on the list (`collection-glue.js:371`).

Chrome does not anchor on the inline axis (§2). For a horizontal list this is a **declared deviation**, entered in LLP 1001's list if Charlie accepts it (Q3):
- **Where it differs.** A real size change of a mounted card left of the viewport shifts the strip in Chrome; the runner holds it still.
- **Why.** Most off-screen size changes a virtualized list sees are its own artefacts: a first measurement replacing an estimate, or a remount re-measuring. The eager oracle shows neither. Without main-axis anchoring, a strip jumps each time a card left of view is first measured.
- **As ruled (Q3 (a)).** Anchor only first measurements (an estimate replaced), and let a re-measured card shift the strip as Chrome does. Anchoring every size change ((b)) may return as an option (`queue/`). The runner can tell those apart: a row's measured epoch says whether it had a measurement.
- **During motion.** No correction applies while the list itself is tracked or decelerating (as iOS already drops it, `CollectionIOS.swift:69`); the web glue gains the same guard (`collection-glue.js:373`). An ancestor's motion does not block an inner correction: moving an inner strip's `scrollLeft` does not disturb the feed's deceleration.

## 4. Nesting

### 4.1 N1: an inner list's lifetime is its outer row's

**The runner.** An inner list is a `Collection` inside its outer row's instance tree. It is created when the outer row is realized and dropped when the row retires, with its rows, slots and views, in the same batch. `item_left`'s exit animation (`views.rs:61–72`) applies to the outer row only. No inner list outlives its outer row, on any host.

**Recycling.** A host's recycling of the outer row's views (LLP 1068) never carries an inner list: a row holding a live `UIScrollView` is ineligible to pool (`NodePoolIOS.swift:153–163`), and when LLP 1068 §4.2 pools a scroll view at rest it resets the offset.

**An inner list scrolled while its outer row is pinned or off-screen** keeps working; its reports come from its own port.

### 4.2 N2: what survives the outer row's retirement

| State | Survives? | Why |
|---|---|---|
| Realized inner rows, their slots and views | No | LLP 1010 §6.2: row state dies on retirement; re-creation is removal then insertion (LLP 1068 §2) |
| The inner scroll position | **Yes, by default** (Q1) | The eager page never destroys the row, so its inner box keeps its offset (§2). Kept as below; `scroll-restoration="manual"` opts out |
| The inner index (keys, positions, generations) | No | Keys may depend on inputs outside the items value (`mod.rs:250–262`); identical items do not prove the same keys. The key pass runs again |
| Measured positive sizes, by inner key | **Optionally**, as a bounded cache, used only as estimates | Not app state; brings a re-created list's geometry closer to the eager oracle, which always has true sizes. Built only if stage 3 shows that first-measurement corrections are visible without it |

**The size cache**, if built:
- It holds only positive sizes that differ from the estimate; uniform thumbnails keep nothing. Zero sizes are never kept: a zero seeds no row, and would hide one from every window (`mod.rs:118–135`).
- It is valid for the `cross` size and typography generation it was measured at. A typography counter is added, bumped by `invalidate_typography` (`traversal.rs:481–515`), which today walks only live collections.
- Every seeded row is measured again when it mounts: the cache never becomes an authority.

**Keeping the position (Q1, as ruled).**
- **Default on**, for a virtualized list nested in a virtualized list's row. The opt-out is `scroll-restoration="manual"` on the inner list; the default is `auto`. The name and values are the web's own for the same choice: `history.scrollRestoration` is `auto` (the user agent restores the scroll position) or `manual` (the page does). A top-level list has nothing that retires it and ignores the row.
- **What is stored** is the inner list's anchor: the key of its first visible item and the offset into that item, never raw pixels. On re-creation the runner seeds its window at that key and places the offset from the item's start, so a changed estimate or a re-measured item above it does not move what the user sees. Storing pixels is what made LegendList's inbox land 44–53 pt off after re-measurement.
- **Keyed by** the outer row's key (`listItemKey`) plus the inner list's position in the row: its plan node, and the keys of any eager `each` between the row root and the list (N3).
- **Bounded by recency**: at most 4,096 entries per outer collection (about 200 KB at ~48 bytes an entry, keys shared with the index), least recently retired evicted first. **An evicted list falls back to its start**, as a list the user never scrolled. Nothing responds to memory pressure (N4).
- **Dropped when the row's key changes**: an outer data change that removes a key drops its entries, and a new key starts at 0. An anchor key no longer among the inner items falls back to the start.
- **The offset comes back too** (2026-09-29): an entry also keeps the anchor item's start when its row left. A new list has only estimates for the items above it, so the difference is spread over those estimates on restore, and the item starts, and the offset reads, where they did. Scroll anchoring itself stays Chrome's: measuring items above the port while the reader rests moves the offset and keeps the content (decided 2026-09-29, the web as the standard; the benchmark's `innerkeep` now judges the item at the same on-screen position).
- **Visible** in the agent's `state`: each collection snapshot lists its kept positions (`kept`: outer key, slot, anchor key, offset), and an inner list's snapshot says whether it was restored.
- Rows, slots and views are still re-created; only the position is kept. It is not a host keep-alive (LLP 1068 §5.3 stands): the runner keeps two values, not a view. An app that wants more (a position across launches) still writes `scrollLeft`/`scrollTop` from keyed data, with `manual`.

**Restoring needs build-then-move.** A restored or app-written offset must be **build-then-move on both axes and every host**, which today it is only for the web's `scrollTop` (`glue.js:815`): the web's `jumpTo` and anchor writes, iOS's `pendingScrollLeft`, Linux's collection offset (§7). Otherwise a restored strip paints an empty port, the bug LLP 1050.000 stage 2 closed for `scrollTop`. An anchor restore lands exactly on its item even with estimates.

### 4.3 N3: keyed by what

The size cache lives **in the outer `Collection`** and dies with it. An entry's key is:
- **the outer row's key** (`listItemKey`), not its position, so an insert above does not misfile it;
- **the inner list's slot**: its plan node id, plus the keys of any eager `each` between the outer row root and the list (usually none). It is the plan node, not a child index, because `when`/`match` arms choose which list exists.

An outer data change that removes a key drops its entries.

### 4.4 N4: memory bounds

**Live state**, the larger number. Each live inner list holds O(its window) rows, the O(N_inner) item values its row body evaluated, and its index at ~220 bytes an item (`queue/`'s crypto-memory entry, item 5; ~30 bytes after an `Rc<str>` sharing change that is not built).
- A 2,000-item filmstrip's index is ~440 KB; a 1,000-message inbox's ~220 KB.
- At SPEC's 1366 × 1024 landscape, rows of about 400 pt and an outer window of three to five viewports hold about 8–13 feed rows: 2–4 filmstrips and 1–2 inboxes live, ~1.1–2.2 MB of inner index before the item values.
- This is bounded by the outer window, not by the feed. It is the argument for the `Rc<str>` change, which this RFC does not make.

**Kept state**, if the cache is built: a byte cap per outer collection counted over every retained allocation (keys, sizes, map nodes), 256 KiB as a trial value to be replaced by stage 3's measurement. An entry larger than a quarter of the cap is not kept. Least recently retired goes first. Nothing responds to memory pressure: there is no host-to-runner signal for it today, and the cap is the bound on every host.

**Not O(visited).** Kept state is bounded by its cap, the bounded form LLP 1010 `:339–340` allows.

**Named, not hidden.** The feed's 1.25 million inner items are app data, O(N) input (LLP 1010 §6.1). Whether the data module materializes them at load or per row decides memory against build time (§5.3). No lazy sequence value type is added (§11).

### 4.5 N5: pin chains

**The chain.** For a focus or interaction pin view, the chain is the innermost list whose mounted row contains the view, plus every ancestor list whose mounted row contains it. A focused field in an inbox message pins that message and the inbox's feed row: otherwise the feed would retire the row holding the focus, the case the current refusal cites (`collection.rs:62–64`).

**Derived in the runner.** Hosts keep reporting a pin to its nearest owner, as all three do today. On accepting it, the runner walks up from that collection and pins the containing row in each ancestor (`NodeInst::contains` already sees through a nested collection, `find.rs:74–77`). One place derives it; no host changes its ownership rule.

**Transferred atomically.** Today a pin in a category releases every other collection's pin in that category before the report applies, and each release re-realizes at once (`traversal.rs:234–241`, `mod.rs:1012–1038`). With chains:
1. preflight the new chain: every list on it exists, its row is mounted, the report is current;
2. apply the pin on every list of the new chain;
3. release the category only on lists off the new chain;
4. only then realize windows.

A shared ancestor (focus moving between two inboxes in the same feed row, or between rows of one inbox) keeps its pin throughout, so the row is never retired between release and apply.

**Bounds.** LLP 1010 §6.2's "at most one focused row and one other interacting row" becomes one focus chain and one interaction chain: at most four pinned rows at depth two.

**Code.** `release_other_pins`, `find_collection_mut` and the edge functions descend into mounted collection rows (`traversal.rs:260–282`, `:383`, `:404–416`). `CollectionMacTests`' nearest-only test is replaced by chain tests: transfer within a chain, between chains, stale feedback, both categories at once, and an ancestor row's item deleted while pinned.

### 4.6 N6: one level, constant opt-in

A virtualized list may appear in a virtualized list's row template, including under `when`, `match`, `each` and component uses, with a **constant** `virtualized=true`. It may not contain another. A dynamic `virtualized` stays rejected at every depth, inactive arms included, as the runner does today (`traversal.rs:440–466`).
- Depth two bounds the pin chains, the budget order (F4) and the test matrix. No consumer asks for more.
- The eager-inside-virtual case stays allowed.

### 4.7 Selection, reorder, accessibility

- **Logical copy** stays within one list, as LLP 1010 §6.2 built it, but must find an inner list: `list_region` descends into mounted collection rows (`text.rs:64–72`).
- **Reorder** (`reorderdrop`) is refused on a row list and on a nested list in this RFC, by the compiler: its preview motion is vertical (`reorder.rs:264`) and `edit_walk` assumes no nesting (`traversal.rs:540–548`). It comes with a consumer.
- **Accessibility.** Each list publishes its own `listitem`, `posinset` and `setsize` (`views.rs:46–84`), so an inner list is its own list inside a list item. A keyboard, focus or accessibility move into an unmounted inner row is owed (LLP 1050.000 §6): the outer row is realized, then the inner row, then the move lands. UIKit's collection focus query sees first responders, not VoiceOver focus (`CollectionIOS.swift:121`); that gap exists for every list today and is not closed here.

## 5. Fill policy for inner lists

### 5.1 F1: what an inner list owes

- **The rule.** An inner row is owed when it is in its own list's visible band (LLP 1050.000 §2.1) **and** its outer row is owed. The outer row is owed when it is visible, pinned, or at a jump's target. An explicit inner pin, an authored inner jump and an accessibility target are owed regardless, since each already owes its outer row through the chain (N5).
- **Not computed.** The inner port's clipping by the outer port is not intersected further: an inbox half off the bottom owes its whole visible band, at most one inner viewport (about seven messages) over.
- **"User motion"** for an inner row means its own list's motion or any ancestor's.

### 5.2 F2: optional rows wait for the ancestors

- **In the runner.** v3 feedback carries `ancestor_moving`: the host sets it when any ancestor list is being dragged, flung or wheeled. While it is set, the inner list builds its owed rows and nothing else, and first geometry, resizes and pin changes do not make the report unlimited (today they do, `mod.rs:835–851`). A host-only `limit: 0` cannot express this, since owed rows are outside `limit`.
  - A feed flung at 24,000 pt/s passes about 60 rows a second, 15 of them nested; each would otherwise build three viewports of thumbnails for a strip gone in a quarter of a second.
- **At rest** the inner list fills its overscan like any list.
- **The host knows the ancestry** through the nearest-owner walk it already has (`Collection.swift:232–253`, `collection-glue.js:109–120`, `presenter/collection.rs:224–239`).
- **Bootstrap.** An inner list is created while its outer row is realized, before any host has laid it out. Its bootstrap covers an estimated port:
  - a row list stretched across its row: the outer list's `cross` (its row width), an upper bound;
  - a vertical inner list: its literal `height`;
  - otherwise today's 512-px budget.

  At the example's 600-pt strip and estimate 128 that is 5 rows; today's rule gives 4. On a wide feed the port estimate is larger than today's, which is the point: the first frame covers the port.

### 5.3 F3: an outer row's cost includes its inner lists, and D3 is not built

Building an outer row that holds an inner list costs its own views, the inner list's owed rows, and the inner list's setup: evaluating its items expression (O(N_inner) values), keying every item and building the index (`mod.rs:243`, `:272–299`, `:358–391`), and validating every key (LLP 1010 §6.2). *Inference, to be measured in stage 4:* 2,000 keys and index entries are a few tenths of a millisecond to about a millisecond of runner time, plus the data module's item values. A filmstrip row may cost a slice (1–4 ms, `ScrollPumpIOS.swift:48`) on its own.

**Declared.** LLP 1050.000's D3 ("a row known to cost more than a frame is not built mid-fling") needs its cost memo, stage 3 of that RFC, which is not built. Until it is, **a visible nested row, key pass included, is built during an outer fling**, and may cost a late frame. That is true of every costly row today; nesting adds the O(N) setup. Nesting does not wait for stage 3; xheavy's `fling` measures what it costs. When the memo lands, its per-key cost for an outer row is the whole build, inner setup included, and D3 applies to the row whole.

### 5.4 F4: the caps stay

The per-turn and per-frame caps that keep a fling inside one slice stay: iOS's two feedback passes per main-queue turn, the web's four reports per frame, Linux's two passes. They are spent in this order:
1. owed rows of outer lists;
2. owed rows of inner lists;
3. the outer lead;
4. a moving inner list's lead;
5. overscan of lists at rest.

A list not served in a turn stays dirty for the next. iOS's text lead follows each list along its own axis (`ScrollPumpIOS.swift:253`).

### 5.5 F5: inner ports settle before they show

The collection makes one runner call per report and commits (`host.rs:705`); the legacy list's report-layout-report loop (`host.rs:962`) is not the collection's. And `covers` checks only the outer list's wrapper rectangles (`CollectionIOS.swift:77–97`), so a covered outer row can hold an underfilled inbox.

- **After a batch creates an inner list**, the native host reports its geometry before the frame is committed, inside the same turn and outside the pass cap for owed rows only; its cost is charged to the outer row. An underestimated bootstrap is corrected in that report.
- **Coverage includes inner ports**: an outer row counts as covered only when each inner port inside its visible part is covered by the inner list's mounted rows.
- **Inner flings** get their own rescue on their own axis. On iOS the synchronous rescue in the scroll callback already runs per scroll view (`Collection.swift:260–280`); it becomes axis-generic through `covers` and `rowsToCover`, whose 64-row cap stays. macOS's `preparedCover` clamps each nested scroller's responsive overdraw to its own built rows, on its own axis (LLP 1050.000 D5). The web declares its gap, as for any list.

## 6. Scroll gestures

### 6.1 G1: chaining at gesture start, per host

| Host | Today | Change |
|---|---|---|
| Web | The browser chains; `overscroll-behavior` passes through (`css.rs:1–13`) | None. The browser is the oracle |
| iOS | UIKit: an inner scroll view's pan takes the whole gesture and rubber-bands at its edge. No `overscroll-behavior` handling (`ScrollViewIOS.swift:32–49`) | The inner pan **fails** only when all hold: the axis is `auto`, it actually scrolls, the drag's dominant direction is known, and the inner is at its edge in that direction. Unknown direction (velocity still zero): the pan is not failed. The check runs before `super` |
| macOS | `ChainingScrollView` routes each event by its dominant axis; zero-delta phased events on an `auto` axis are dropped (`:84–85`) | **Latch a phased gesture** (below) |
| Linux | `wheel_at` chains by dominant axis per event; authored `overscroll-behavior` is not read (`presenter.rs:1039–1108`) | Read `contain`/`none`; G2 for phase-less wheels |

**iOS.**
- `contain` begins the inner pan and keeps UIKit's bounce, the platform's overscroll affordance, which CSS leaves to the user agent (LLP 1008 `:671`). `none` begins it with bouncing off; UIKit's `bounces` is per view, and a single-axis list does not care.
- There is no mid-gesture hand-off on either platform: Chrome drops the rest of the gesture (§2), and UIKit keeps it in the inner view.
- Under `auto`, a drag that begins at an inner edge outward no longer shows the inner rubber band: it chains, as Chrome and iOS Safari with `overscroll-behavior` do. An author who wants the band says `contain`, as both Markdown readers do (LLP 1033 D4).
- A device run with real fingers is a landing gate (§9): neither this session nor the agent can synthesize a UIKit pan (`queue/`, the Apple entry).
- The iOS agent wheel (`AgentIOS.swift:560–588`) reads `overscroll-behavior` and follows G2.

**The macOS latch.** Chrome's wheel scroll latching for trackpads, stated fully:
- The owner is chosen at the gesture's **first non-zero `changed`** event, by the dominant-axis rule touch uses (§2), since a trackpad `began` is often zero-delta.
- Every later event of the gesture (`changed`, zero-delta `ended`, `cancelled`, and the momentum phases through momentum `ended`) goes to that owner, zero-delta ones included: the zero-delta `ended` is the lift AppKit needs, or a rubber band stays stretched (`:77–83`).
- The latch ends at `ended`/`cancelled` without momentum, at momentum `ended`, or when a new `began` arrives.
- If the owner is destroyed mid-gesture, the remaining events are dropped until the gesture ends; they are not retargeted.
- It is tested with the agent's `tap … wheel dx dy gesture`, which already sends `began`, `changed` and a zero-delta `ended` (`AgentMac.swift:423–448`). A physical trackpad in Chrome and in the Mac host is the comparison, not the spec.

### 6.2 G2: phase-less wheels follow Chrome

A phase-less wheel event, on macOS, Linux and the iOS agent: the deepest scroller under the point that can take **any** of its components takes the components it can, and the rest is dropped; if none can, it chains. That is §2's measured tick, and what the web does for the same `tap … wheel`. It changes the routing of diagonal ticks only; axis-aligned ticks route as today.

**Direction locking.** None is added.
- **Orthogonal nesting** (a strip in a feed) is UIKit's own: the inner pan fails for a vertical-dominant drag when the inner has no vertical travel, and the outer takes it. LLP 1008 "Orthogonal carousels" already turns off forced vertical bounce on a scroller with only horizontal travel. Chrome's touch picks the dominant axis's scroller the same way (§2).
- **`isDirectionalLockEnabled`** constrains one scroll view's two axes; CSS has no equivalent, and a single-axis list does not need it.
- **`touch-action`** (LLP 1057.001) is intersected from the hit view through every scroller up to the one panning (`ScrollViewIOS.swift:39–46`): `touch-action="pan-x"` on a strip means a touch there pans horizontally only, the feed included. It is not how a strip hands vertical drags to the feed; `auto` is.

### 6.3 G3: the agent

No new operation: `rules/DEFERRED.md` says "a new input is a form of `tap`".
- **Scroll one inner list.** `tap <testId> wheel <dx> <dy>` where the testId is the inner list or a row inside it. The wheel starts at that box's centre and chains by the host's rule. A tick at an inner list's end moves the feed: that is the chaining test.
- **Flings and latching.** On the web, `tap … down` and `pointer move` contacts (`agent.mjs:261–289`) make Chrome fling and latch for real. On macOS, `gesture` sends phases. Native touch flings stay a device run.
- **`state`.** Each collection snapshot, nested ones included (`collections_json` already descends), gains `axis` and `parent`: the outer list's view and the outer row's key. Its `pending` flag (`api.rs:96`) is per list, so a nested list's pending is visible. Fields in an existing reply.
- **`layout` and `clock settle`.** `layout` reports every scroller's `sx`/`sy`. `clock settle` fills every list, inner ones included (`ScrollPumpIOS.swift:262–272`).

## 7. Host implementation

**Everywhere:** v3 feedback (H1), the axis in geometry, `ancestor_moving` (F2), coverage over inner ports (F5), authored `scrollLeft` built before it moves (§4.2).

| Host | Work |
|---|---|
| Web (`collection-glue.js`, `glue.js`) | `portOf` tests `overflowX` for a row list (`:92–97`); `geometry`, `rowsToCover`, `move`, `jumpTo` and the anchor write (`:381`) read and write `scrollLeft`/`clientWidth`/`left`/`paddingLeft`; wrappers measured by width; `glue.js:815`'s build-then-move for `scrollLeft`; the correction motion guard (H4); the report cap spent in F4's order. Gestures: nothing |
| iOS | `CollectionIOS.swift`: `geometry`, `correct`, `covers`, `rowsToCover` by axis; `contentSize.width` for a row list. `ScrollPumpIOS.swift`: velocity per list along its axis (`:80–96`); per-list text lead. `ScrollViewIOS.swift`: the begin rule (G1). `AgentIOS.swift`: containment and G2. `NodeViewIOS.swift`: an authored offset on a collection builds first (`:864–866`). F5's settle-before-commit |
| iOS pool (LLP 1068) | A row holding a live scroll view stays ineligible (1068 stage 1 does not change it). Inner rows pool already, and across live inner lists of the same shape, since parked trees are matched by shape alone (`NodePoolIOS.swift:196–204`); they are dropped when the list they were parked in leaves (`:85–90`). Pooling outer rows with their inner scroll view (1068 §4.2) is stage 5, only if measured |
| macOS | `CollectionMac.swift`: `preparedCover` and `KnobDrag` by axis (`:83–254`); `PresenterMac.swift:325–335` travel by axis. `ChainingScrollView`: the latch (G1) and G2. No pool (LLP 1068 §9) |
| Linux | `presenter/collection.rs`: `offset.0`, `padding_left` and a collection's `ScrollLeft` for a row list (`:625–635`); `paint/region.rs:61–69` clamps x by `collection_max`; `wheel_at` reads `overscroll-behavior` and follows G2. Fill stays unlimited: Linux has no fling (LLP 1050.000 §7) |
| Runner | The rename (H1); row-axis wrappers and spacers (§3.1); the inherited-RTL refusal; nested lookup and edges; pin chains (N5); `ancestor_moving` (F2); the port-estimate bootstrap; `parent` in snapshots; `list_region` descends; the creation check allows constant depth one; the typography counter and the size cache only if built |

## 8. Contract surface and compile rules

The Extra Heavy feed, as an author writes it. The kinds are one `column` with a `when` per kind, as the heavy bench's port does. The data module returns each row's inner items (`stripItems`, `inboxItems`):

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
| A virtualized list with a constant `virtualized=true` inside a virtualized list's row template is allowed at depth one; a virtualized list inside *that* list's rows, or a dynamic `virtualized` at any depth, is refused | `lower-collection-depth` | `lower-collection-nested` (`:72–76`) |
| An inner vertical list needs a literal `height` or `max-height`; `flex` alone is not enough in a row, whose height is its content's | `lower-collection-unbounded` (widened) | — |
| A row list needs a literal `height` | `lower-collection-cross` | — |
| `flex-direction` requires `display="flex"`; only `row`; `display` is `block` or `flex` | `lower-collection-flow` | `:118`, `:127–131` |
| On a row list: reverse directions, `flex-wrap` other than `nowrap`, `grid-template-*`, `justify-content` other than `flex-start`/`normal`, and `direction="rtl"` are refused, each with its reason | `lower-collection-flow` | `:127–131` |
| Main-axis padding must be zero; cross-axis padding is allowed; `gap` is refused on both axes | `lower-collection-flow` | `:100–108`, `:124–126` |
| `overflow-x` must scroll on a row list (`overflow-y` may be `hidden`), and the reverse on a vertical one | `lower-collection-flow` | `:120–123` |
| `estimated-item-width` on a row list only, `estimated-item-height` on a vertical one | `lower-collection-estimate` | — |
| `reorderdrop` on a row list or a nested list is refused | `lower-collection-reorder` | — |

The runner's creation check (`traversal.rs:443–476`) mirrors the depth rule, inactive arms included, and adds the resolved-RTL refusal.

## 9. Tests and measurement

Nothing new is added as apparatus: no script, check or harness. Tests go in the existing test files, or in a new test module beside a Rust file near its 1,500-line cap (`collection/tests.rs` is at 1,445), as `fill_tests.rs` was added.

- **Runner.**
  - The rename: existing suites pass with their JSON assertions renamed.
  - Row-axis wrappers and spacers; the depth and dynamic refusals.
  - Pin chains (N5's list): transfer within a chain and between chains, stale feedback, both categories at once, an ancestor row's item deleted while pinned.
  - Inner lifetime: outer retirement destroys inner rows in the same batch.
  - `ancestor_moving`: exactly the owed rows are built, including on first geometry and resize.
  - The port-estimate bootstrap.
  - Inner `reachstart`/`reachend` re-arm; logical copy inside an inner list.
  - The size cache, if built: positive non-estimate sizes only, dropped on `cross` or typography change, byte cap honoured over 1,000 outer rows.
- **Compiler.** §8's table as cases, beside the six existing collection cases.
- **Chrome pins in `host/web/collection.test.mjs`**, which already drives Chrome over CDP:
  - §2's table as assertions (wheel chaining, `contain`, touch latching, edge-start chaining, the diagonal tick, re-creation at 0, block-axis-only anchoring), so a Chrome change that moves the oracle fails a test;
  - eager-versus-virtualized parity: the same nested fixture with the inner `virtualized=false` and `true`, mounted rows' `getBoundingClientRect` equal at a set of inner and outer offsets, and the outer row's used width equal to the feed's.
- **The fixture** is one Contract file in `contract/corpus/`, where LLP 1010's `scroll.contract` lives: a vertical list of 200 rows whose every fifth row holds a 500-item strip and every seventh a 300-item inbox, with testIds `strip-<n>` and `inbox-<n>`. A fixture, not a harness.
- **Cross-host parity.** The same fixture on web, macOS, iOS (simulator) and Linux, driven by the same agent script, comparing `state`'s snapshots and `layout`'s boxes at the same offsets. The script derives each inner list's end from `layout` (its `sy` against its content) before asserting that a further tick moves the feed:

  ```
  bun scripts/agent.mjs <host> tree "tap strip-5 wheel 900 0" state layout \
    "tap inbox-7 wheel 0 20000" layout "tap inbox-7 wheel 0 120" layout "clock settle" state
  ```

  Plus: concurrent outer and inner motion (web contacts), an outer row retired while its inner list is flinging, and focus moving between two inboxes of one feed row.
- **Landing gates by hand:** an iPhone and the iPad with real fingers (the edge-start rule, `contain`'s bounce, orthogonal drags); Chrome and the Mac host with a real trackpad (latching).
- **Apple XCTests** in the existing `CollectionMacTests.swift` and `NodePoolIOSTests.swift`: axis geometry; the latch fed phased events, zero-delta phases and owner destruction included; the begin rule given edge, unknown velocity and insets; inner rows parking during an inner fling.

**On `~/bench/xheavy`** (outside the repo, LLP 1050.000 D4). A new probe scenario, `innerfling`:
- **Setup.** Launch at a `BENCH_START_INDEX` whose screen shows a filmstrip and an inbox, the feed at rest.
- **Finding the scrollers** the way `findScroll` finds the feed (`probe.m:229–240`), with the opposite filters: the strip by bounds < 300 tall and `contentSize.width` > 20,000; the inbox by `contentSize.height` > 20,000 and the inbox's bounds.
- **Driving.** Each inner scroller's `contentOffset` through `fling`'s constant-speed segments (±1k–24k pt/s, 2 s each), then `ladder`'s (to 96k), one at a time. It is the probe's programmatic drive, which the host reads as an unclassified user scroll (owed, as a drag); it does not exercise deceleration or a rest target, which the device run does.
- **Measured per segment:** fps, p95/p99 frame time and main-thread busy ms/s as for `fling`; blank area over the **inner** scroller's rect (vertical ink-free bands for the strip, horizontal for the inbox); footprint at start, peak and end.
- **Also:** the existing `fling` and `ladder`, now crossing 750 nested rows, read against the 17-kind stream (`gen.py` keeps its seed); footprint after twenty full inner traversals of one strip (flat); after scrolling the feed past 100 nested rows (flat, and kept state at its cap if the cache is built).
- **The comparison.** SwiftUI: `ScrollView(.horizontal)` + `LazyHStack`, and a fixed-frame `ScrollView` + `LazyVStack` in a `List` row. Expo: nested `FlashList`s. Each its stack's ordinary choice, as SPEC's Decisions table does.

## 10. Staging

| Stage | Ships | Done when |
|---|---|---|
| 0 | Nothing. The xheavy port ships the strip as a plain horizontal `scroll` and the inbox as an eager inner list (GAPS gap 8's fallback), and is measured | The baseline |
| 1 | The legacy windowed list is deleted (`queue/` "One list engine"): the take (§12) | The Markdown reader on the collection; LLP 1050.000 stage 1's parity gates |
| 2 | The axis refactor (H1): rename, wire v3, all hosts. No behaviour change | Every existing collection test and smoke passes with renamed assertions |
| 3 | Horizontal lists at top level (H2–H4), build-then-move `scrollLeft`, G2, the Chrome pins, the fixture's strip | A 25,000-item top-level strip windowed on four hosts with O(window) rows after twenty traversals; Chrome parity; `tap strip wheel` |
| 4 | Nesting: N1, N3 (keys only), N5, N6, §4.7, F1–F5, G1, G3; the kept position and `scroll-restoration` (§4.2, Q1) | The fixture on four hosts; the runner tests; the hand gates; xheavy `innerfling` and `fling` against stage 0 |
| 5 | Only if stage 4's numbers ask: the size cache (N2–N4); pooling outer rows with their scroll view (1068 §4.2) | xheavy against stage 4 |

**Why this order.** The filmstrip needs both halves; the inbox needs only nesting. Stage 2 is mechanical, and the existing vertical suites prove it. Nesting's hard parts (pins, lifetime, budget, gestures) are axis-free; built on the axis-generic seam they are written once, where built on the vertical seam they would be rewritten by the refactor. The inbox waits one stage. Nesting does not wait for LLP 1050.000 stage 3 (F3).

## 11. What this does not do

- **Not built:** grids, masonry, wrapping or reversed (inverted) virtualized lists; visible-column windowing (LLP 1050.000 §3); nesting deeper than one level; RTL horizontal lists; `gap` or main-axis alignment on a virtualized container; reorder in row or nested lists.
- **No host keep-alive**, by row key, of an inner list or its views. The runner keeps only its position, bounded (§4.2).
- **No lazy sequence value type.** An inner list's items are a list value the row body evaluates. A range or generator value the index could read without materializing N records would cut §5.3's setup cost; it is a new value type, and a separate proposal if stage 4's numbers ask.
- **No memory-pressure signal** from hosts to the runner.
- **Selection and copy stop at a list's boundary.**
- **No new agent operation, check, script or harness. No change to the web's gestures.**

## 12. `rules/DEFERRED.md`

`rules/DEFERRED.md` keeps virtualList v2 out ("cert wires, extent demand, proxy lanes") and admits "a straightforward windowed list with bounded row/view lifetime" (2026-09-14). Nested and horizontal lists are that windowed list on a second axis and one level down, with the same lifetime rule; they are not virtualList v2's machinery. They are still new capability, so the entry says so. Proposed, beside the virtualList line:

> **Expanded (Charlie, 2026-09-27, LLP 1070):** horizontal windowed lists (`display: flex; flex-direction: row`) and one level of nesting, a windowed list in a windowed list's row, with the inner list's lifetime its outer row's. Unblocks the Extra Heavy feed's filmstrip and inbox, and any feed of carousels. Take: the legacy windowed list (`item-height`/`estimated-item-height` without `virtualized`, `runner/src/instance/window.rs`) is deleted in the first change of this work, before either capability lands. Still out: grids, masonry, wrapping and inverted lists; nesting deeper than one level; host keep-alive of an inner list or its views.

`rules/DEFERRED.md` requires the take in the admitting change (`:438`), which is why stage 1 is the deletion. The take is a path removed that `queue/` already owes; this makes it a precondition rather than an intention, the precedent LLP 1026 D12 set ("fewer paths after than before"). It is a cheap take, and Q2 says so.

## 13. Questions for Charlie (all ruled 2026-09-27)

**Q1. An inner list re-created with its outer row starts at offset 0. The runner keeps no offset; an app that wants one keeps it in keyed data. Agree?**
Recommendation: yes. Chrome resets a removed box, even the same element re-inserted (§2); exact2's lifecycle is removal (LLP 1010 §6.2, LLP 1068 §2); SPEC's carousel already resets on recycle; feed apps keep carousel offsets in their model (the Furrow pattern, LLP 1068's appendix). Keeping it in the runner would make a row's state depend on whether its key was recently seen, which LLP 1068 §5.3 rejected. Confidence: medium-high (0.7). The size cache (N2) is a separate, measured decision at stage 5 and needs no ruling now.
**Ruled (Charlie, 2026-09-27): no, keep it.** "leave it up to the app to save position. maybe worth creating an option to do it automatically since that seems more intuitively correct"; then, on the revised recommendation, "if your rec has changed then let's do your rec". The r2 recommendation above is withdrawn: an inner list keeps its position automatically by default, as an anchor key and offset, bounded by recency, with `scroll-restoration="manual"` to opt out (§0.1, §4.2).

**Q2. Admit horizontal and one-level nested windowed lists (§12), with the legacy windowed list's deletion, done first, as the take?**
Recommendation: yes. Confidence: medium (0.6). The deletion is owed already, so the take is cheap. If that is too cheap, the alternative take is that further Extra Heavy row kinds wait behind these two.
**Ruled (Charlie, 2026-09-27): yes.** "yes delete the old windowed list thing, collection list is now better." Entered in `rules/DEFERRED.md`.

**Q3. Horizontal lists anchor on their main axis, which Chrome does not do on the inline axis (§2, H4): anchor every size change, or only first measurements (an estimate replaced), letting a re-measured card shift the strip as in Chrome?**
Recommendation: anchor only first measurements. It removes the jumps virtualization causes and keeps Chrome's behaviour for real changes, with no declared deviation for mounted cards; a remount's re-measurement of a card whose data did not change measures the same. Confidence: medium (0.6). Anchoring everything is simpler and never jumps, at the cost of a declared deviation.
**Ruled (Charlie, 2026-09-27): (a).** "(a) is ok for now, but we might want to allow an option for (b) at some point." (b) is a `queue/` entry.

**Q4. On iOS, `overscroll-behavior: auto` at an inner list's edge chains a new outward drag to the outer list, as Chrome does, giving up UIKit's rubber band on the inner list at that edge; `contain` keeps it. Agree?**
Recommendation: yes. It is CSS's meaning of `auto` and how the web behaves. Confidence: medium (0.65), until a device run with real fingers shows it feels right on a phone; that run is a landing gate either way.
**Ruled (Charlie, 2026-09-27): provisionally yes.** "i don't really know, lt's try your rec and see how it feels." Provisional until the real-finger feel test on a device (`queue/`).

## Appendix: the Chrome probe

A scratch Bun script (session scratchpad, not in the repo) that:
- launches Chrome 154.0.8037.57 `--headless=new` over `--remote-debugging-pipe` with `scripts/agent-launch.mjs`'s `Cdp`;
- sets an 800 × 600 viewport at scale 1;
- builds the §2 page with `Runtime.evaluate`;
- drives it with `Input.dispatchMouseEvent` (`mouseWheel`) and `Input.synthesizeScrollGesture` (`gestureSourceType: "touch"`, `preventFling: true`, 400–800 px/s);
- reads `scrollTop`/`scrollLeft` after each event.

A first touch run that set the inner offset and started the gesture in the same turn moved the outer only 5 px; three reruns with a 300 ms settle moved it 499–501 px, which is what §2 reports. Each recorded Chrome process was killed by PID.

Not covered: phased trackpad input (CDP has none), real touch on a phone, Safari, `overscroll-behavior: none`.
