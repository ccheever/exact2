# LLP 1091: Dropping across lists

**Type:** RFC
**Status:** Draft (r1)
**Systems:** Contract compiler, `kernel/tables/schema.json`, runner (`collection/reorder*.rs`, `runner/reorder.rs`, `geometry.rs`), web (`host/web/{motion,geometry}-glue.js`, `host/web-js/{reorder,arrange}.js`), Apple (`Reorder{IOS,Hold}.swift`, `MouseReorderMac.swift`, `arrange.rs`), Linux (`presenter/{arrange,contact,paint}.rs`), the driver, conformance, docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-06, stage 2 on 2026-10-07, stage 3 on 2026-10-08 (§6)
**Amends:** LLP 1070 §4.7 (reorder on a row list and a nested list); LLP 1056 §8.6 (the `PointerEvent` record); LLP 1001 (one declared deviation, D10); `docs/contract-grammar.md`'s payload table; `rules/DEFERRED.md` **Motion** (D12)
**Related:** LLP 1051.000 D1 (`frame`); LLP 1057.001 rule 3; LLP 1070.000 (`scrollIntoView`); LLP 1083.000 (paint order); LLP 1088 §9.1 (durable lists live in the data module); LLP 1012 (the driver). Diaries: `~/projects/x2apps/{kanban,kanban2}/DIARY.md`. The fix/input lane's report of 2026-10-04 (Options A–C; C shipped in LLP 1051.000 D1).

## Summary

`reorderdrop` moves a row within one vertical virtualized list. Two kanban
builds needed to move a card between columns and built it by hand: `pan`
deltas, `frame()` of every column and card, a root ghost, an insertion index
from `filter` and `length`, and edge autoscroll bound to state. The first
took about 60 lines; the second about 100, unrolling eight `frame` calls and
capping a board at eight columns.

This RFC widens `reorderdrop` to lists that share a group, SortableJS's
model. Lists name a group (`reorderGroup="cards"`). A drop fires on the
target list's `reorderdrop` with today's two keys, and an action taking one
more parameter also gets a `ReorderEvent` naming the source and target. Today's
lift, preview and autoscroll extend to row lists and nested lists, which
make a board. For hand-built drags it adds `elementFromPoint(x, y)` and
keeps `frame()` untransformed.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `reorderGroup` on a virtualized list; same-group lists in one session exchange rows | kanban F4, kanban2 | 1 |
| D2 | One drop, on the target list: `item`, `before`, then an optional `ReorderEvent { from, to }` | kanban F4 | 1 |
| D3 | The action moves the data with one `send` | both | 1 |
| D4 | Reorder on a row list and a nested list (LLP 1070 §4.7's refusals lifted) | kanban F4 (columns) | 1–3 |
| D5 | Today's recognizers; a grouped lift shows a top-layer ghost | kanban2 (aim) | 2, 3 |
| D6 | Target, gap, and autoscroll of every scroller under the ghost | both | 1–3 |
| D7 | Drop, a hold until the move shows, cancel | — | 1–3 |
| D8 | Keyboard and assistive technology | kanban (built by hand) | 1–3 |
| D9 | `elementFromPoint(x, y) -> option<string>`, an action read | kanban2 #2 | 1–3 |
| D10 | `frame()` stays untransformed, declared; `PointerEvent` gains `clientX`/`clientY` | kanban2 #3 | 1 |
| D11 | Driver `tap A drag to B`, in tests too; `state.reorder`; conformance | kanban F14, F18 | 2 |
| D12 | The `rules/DEFERRED.md` entry | — | 1 |

## 1. Evidence

- **Kanban (first build, F4).** `app.contract:198–245`: `dragCard` maps
  `frame()` over every column and the target's cards on each move, picks
  the column with `length(filter(lefts, l => l <= px))` and the slot with
  `length(filter(mids, m => m < py))`, and writes `boardScrollTo` within
  48 px of an edge. F6 is fixed (LLP 1057.001 rule 3).
- **Kanban2** (`423e4c4bc`, Grok 4.7). `app.contract:480–580` unrolls
  `frame("col-0")`…`frame("col-7")` three times. A drop aimed at the grip's
  untransformed frame stayed in the source column while the card's body was
  over the next; aiming at `base + delta + size/2` cost 30 minutes.
- **What exists** (`a98895a22`):
  - The payload is `item: string`, `before: option<string>`
    (`contract/analyze/src/payload.rs:37`), with no event record
    (`contract/types/src/selection.rs:7–23`).
  - Refused outside a virtualized list (`lower-reorder-collection`,
    `contract/lower/src/collection.rs:40`), on a row list (`:133`) and a
    nested list (`:183`, `lower-collection-reorder`).
  - The runner keeps one preview per collection
    (`instance/collection/reorder.rs:6–16`) and one owner
    (`runner/reorder.rs:83–111`). The gap is vertical, certified against
    `ReorderGeometry` v1 (`scroll_top`, `port_height`). The drop dispatches
    in the commit that resets the preview (`runner/reorder.rs:148–173`).
  - Every host lifts the real row, clipped by its own scrollport. Autoscroll
    is the host's: 32 px at 720 px/s on web and Apple, 24 px ramping to
    300 px/s on Linux.
  - No host has a keyboard or accessibility path for reorder.

## 2. What the platforms offer

- **HTML drag and drop** (`draggable`, `dragover`, `drop`, `DataTransfer`)
  moves data between elements, windows and applications; it is not a list
  move. A target hears `dragover` per element and computes its own index. The drag image is a bitmap taken at
  `dragstart` that the page cannot animate, and the page moves siblings
  itself. Mobile support came late and only through a long press; touch
  sortable libraries ship a pointer-event path (SortableJS's fallback).
  Taken from the web: the names (SortableJS's `group`,
  `from`, `to`; DOM's `elementFromPoint`, `clientX`, `clientY`),
  `pointercancel` as a cancel, and the top layer for the ghost.
- **UIKit.** `UICollectionView`'s drag and drop delegates move items
  between collection views in one app: a `.move` proposal with
  `.insertAtDestinationIndexPath` opens a gap, and a
  `UICollectionViewDropPlaceholder` holds the slot until the data source
  commits (D7's hold). A `UIDragSession` is asynchronous over item providers
  (a collection view's drag is off by default on iPhone); Exact keeps its own recognizer for parity.
- **AppKit.** `NSDraggingSession` is pasteboard-based; `NSTableView` and
  `NSCollectionView` show a gap, and `animatesToStartingPositionsOnCancelOrFail`
  snaps back. D6 and D7 take the gap and the snap back; Exact starts no
  session.

## 3. Decisions

### D1 — `reorderGroup` names a group of lists

```
list id=`cards-${col.id}` virtualized=true reorderGroup="cards" reorderdrop=dropCard(col.id) …
  each card in col.cards key=card.id
    box reorderFor=`cards-${col.id}` touch-action="none" aria-label=`Move ${card.title}` …
```

- `reorderGroup` is a string prop on `list` (a new `schema.json` id beside
  `reorderFor`, 77). Lists with the same non-empty value mounted in one
  session exchange rows. Without it a list behaves as today.
- A grouped list is virtualized, keys its `each` by string, and has a
  `reorderdrop` and an `id` (`reorderFor` already resolves a list by id).
  Otherwise `lower-reorder-group`: "`reorderGroup` joins lists that take a
  drop: give this `list virtualized=true` a `reorderdrop` and an `id`". On
  any other tag, `lower-reorder-collection`'s message.
- Keys are unique across a group. The runner opens no gap in a list that
  already has the dragged key, and the development journal says so once a
  drag.

### D2 — The drop: two keys, then an optional `ReorderEvent`

- A drop fires once, on the **target** list's `reorderdrop`: `item` (the
  dragged key) and `before` (the target's key it lands before, `none` at the
  end), as today.
- `reorderdrop` joins `event_record`'s table: an action taking one more
  parameter gets `ReorderEvent { from: string, to: string }`, the source and
  target lists' `id`s (SortableJS's names). Within one list `from == to`;
  an action that ignores it is unchanged.
- The source list's action does not fire: one gesture, one action, one
  commit. The captured `col.id` already names the target.
- Grammar row: "A string, an `option<string>`, then optionally a
  `ReorderEvent` | `reorderdrop`".

### D3 — The action moves the data with one send

Board data is durable, so it is the data module's (LLP 1088 §9.1):

```
action dropCard(col: string, item: string, before: option<string>)
  let at = match before { case some(k) => k, case none => "" }
  send edited = edit("moveCard", boardId, item, col, at)
```

The module removes and inserts in one answer. Session-state lists get the
same move when LLP 1088 §9.1's list construction lands; this RFC adds no
list function. D7's hold covers the answer's latency.

### D4 — Reorder on a row list and a nested list

LLP 1070 §4.7 refused both "until a consumer". Kanban is it: the columns are
a row list, the cards a list in each column's row.

- The preview runs along the list's main axis (`translate: {offset}px 0px`
  on a row list, the same spring).
- `ReorderGeometry` v2 carries `axis`, `scroll_main` and `port_main` in
  place of `scroll_top` and `port_height`. The codec and every host's
  encoder move together; v1 is deleted.
- `reorder_collection` and `edit_walk` (`traversal.rs:540–548`) reach an
  inner list. A lift from an inner list pins its outer row too, so the
  board's window keeps the source column while the board autoscrolls. A
  lifted column carries its inner list in the ghost.
- LLP 1070's other row-list refusals stand (wrapping, reversed and
  right-to-left flow, `gap`, main-axis padding).

### D5 — The lift and the ghost

- **Recognition** is each host's, as today: `pointerdown` plus an 8 px slop
  on the web; the handle's pan, or a long press where the list pans, on iOS;
  the mouse or contact slop on macOS and Linux. For a grouped or row list a
  move past the slop in any direction lifts (today a vertical list's grip
  lifts only on a mostly vertical move); the grip's `touch-action="none"`
  keeps scrolling off it.
- **A grouped lift shows a ghost in the top layer**, because the real row
  cannot leave its scrollport's clip. The source slot keeps its space,
  hidden, while the ghost is over the source list.
  - Web: a `cloneNode(true)` of the row wrapper in a `popover="manual"`
    element shown with `showPopover()`, `position: fixed` at the row's
    viewport box, `pointer-events: none`; the source gets `visibility:
    hidden`.
  - iOS: `snapshotView(afterScreenUpdates: false)` in the window, above the
    content. macOS: `cacheDisplay(in:to:)` into a topmost image view.
    Linux: the lifted subtree painted last with no ancestor clip.
- The ghost follows the contact by the grip's offset. The point that picks
  target and gap is the **ghost's centre**, not the pointer: the fix
  kanban2 found by hand.
- Ungrouped vertical lists keep today's raised real row.
- iOS plays a light impact at the lift, as UIKit's drag lift does.

### D6 — Target, gap and autoscroll

- **Target:** the grouped list whose scrollport contains the ghost's centre,
  found by the host from the lists' viewport boxes. Between lists the last
  target stays (as SortableJS keeps its placeholder), so a gutter does not
  flicker. An empty list is a target with `before = none`; the docs say to
  give it a `min-height`.
- **Gap:** the runner opens a gap of the ghost's extent along the target's
  main axis, before the certified key (`certified_gap_excluding`, excluding
  nothing in a foreign list). While the target is another list, the source
  closes its slot. The gap is the insertion indicator; no line is drawn.
- **Session:** `reorder_owner` becomes a session holding `source`,
  `target`, and one preview per list, `Outgoing` in the source and
  `Incoming { key, extent }` in a target. A target change closes the old
  target's preview and opens the new one in one commit.
- **Autoscroll:** the innermost scroller under the ghost's centre that can
  move toward its edge band scrolls, then each enclosing one, so the target
  list scrolls on its axis and the board's scroller (a row list or a plain
  `scroll`) on the other. Each host keeps today's band and speed; the gap
  re-certifies against the moved geometry.

### D7 — Drop, hold and cancel

- **Drop:** `reorderdrop` (D2) dispatches in the commit that ends the
  preview, as today.
- **Hold:** when that commit does not show the move (the action sent a
  mutation, D3), the ghost and the target's gap stay: UIKit's drop
  placeholder. The hold ends at the first commit where the dragged key sits
  in the target before `before` (the ghost springs onto the row, which then
  shows), or the key is in no grouped list (deleted: the ghost fades), or
  after 1 s on the session's clock (the ghost springs home, the gaps close,
  and the journal says `reorderdrop: the move did not show within 1 s`). A
  within-list drop holds too, so an asynchronous reorder no longer springs
  back, then jumps.
- **Cancel:** Escape, `pointercancel` or lost capture, the window resigning
  key, the source binding dying (`reconcile_reorder`), or the pointer
  leaving the window. The ghost springs to the source slot, the gaps close,
  no action runs. A drop outside every list goes to the sticky target, as
  SortableJS and both kanban builds do.

### D8 — Keyboard and assistive technology

- **Keyboard**, on a grip with no `press` or `key` handler of its own (an
  authored one keeps its keys and the app's alternative stands). The grip is
  focusable (`tabindex=0` on the web; the key view loop on macOS). Space
  lifts; arrows along the target's main axis move `before` one row; arrows
  across it move to the nearest grouped list that way, by viewport boxes, at
  the row nearest the ghost; the gap stays in view by `scrollIntoView`'s
  `nearest` (LLP 1070.000); Space or Enter drops; Escape cancels. (dnd-kit's keys.) A keyboard drop is the same
  `reorderdrop` and holds the same way; when the hold ends, focus moves to
  the moved row's grip. The runner's `reorder_step` API serves keyboard and
  custom actions alike.
- **Assistive technology:** a grip gets custom actions,
  `accessibilityCustomActions` on iOS and `NSAccessibilityCustomAction` on
  macOS: "Move earlier", "Move later", and in a group "Move to previous
  list" and "Move to next list". Each is one drop without a lift. The web
  has no custom actions, so its path is the keyboard; Linux exposes no
  accessibility tree (LLP 1015 §7) and gets the keyboard only.
- **Announcements:** none from the host in r1 (§8 Q2); the app's own
  `aria-live` text, written in the drop's action, carries them.

### D9 — `elementFromPoint(x, y)`

- `elementFromPoint(x: number, y: number) -> option<string>`, an action
  read like `frame` (`type-geometry-outside-action` elsewhere).
- It answers the `id` of the topmost node at viewport point `(x, y)`, or of
  its nearest ancestor with one: DOM's
  `document.elementFromPoint(x, y)?.closest("[id]")?.id`, `none` when there
  is none. Contract names elements by id (`frame`, `focus`). `testId` is the
  driver's, not the app's.
- Boxes are `frame()`'s: layout boxes in the viewport, every scroll applied,
  transforms not (D10). Ancestors' `overflow` clips apply; `pointer-events:
  none`, `display: none` and `visibility: hidden` are skipped; paint order is
  CSS's (LLP 1083.000), so a later sibling and a higher `z-index` win.
- Natively the runner walks the kernel's boxes in reverse paint order with
  `Scrolled`'s offsets (`runner/src/geometry.rs`), no presenter needed. The
  web glue walks the same layout boxes it uses for `frame`
  (`geometry-glue.js`), not the browser's `elementFromPoint`, which tests
  painted, transformed boxes and would disagree with native.

### D10 — `frame()` stays untransformed; `PointerEvent` gains client coordinates

- `frame()` keeps LLP 1051.000 D1's box (viewport, scrolled, untransformed).
  `getBoundingClientRect` includes transforms, so this is a deviation,
  declared in LLP 1001 with its reason:
  - a transform's presented value lives in each host's motion engine (or in
    CSS on the web), a spring in flight or a drag the engine holds, which
    the runner cannot read while an action runs (LLP 1051.000 D1, change of
    2026-09-28);
  - including it on the web alone breaks the parity the web-as-standard rule
    exists to protect;
  - decisions (a snap stop, a fit, a drop slot) ask where layout put a box.
- A hand-built drag adds the translate it wrote, as kanban2 did.
- `PointerEvent` gains `clientX` and `clientY`, the viewport point in CSS px
  (DOM's names), which `elementFromPoint` takes. Today a drag adds
  `offsetX` to its node's `frame().x` by hand.

### D11 — Driver, tests and conformance

- `tap A drag to B [at x y] [over ms] [hold ms] [during "op"…]` ends at B's
  box centre, or at `(x, y)` from B's top-left. The driver computes the
  delta from both boxes at the press and runs today's `dragTap`, on every
  host where `drag` runs. Test files gain `tap "A" drag to "B"`
  (`scripts/agent-test.mjs`, beside `drag`), so a move test does not depend
  on the viewport.
- During a session `state.reorder` is `{ item, from, to, before, phase }`,
  phase `lifted`, `holding` or `cancelling`, for `during` reads. The
  keyboard path drives with today's `type <grip> key Space` and arrow keys.
- `host/web-js/conformance/reorder-group.contract` and its `.steps`: three
  grouped lists (one empty) in a row list that itself reorders; a cross-list
  drag, a drop into the empty list, a column drag, an autoscrolling drag,
  Escape mid-drag, a keyboard move across lists, and a drop whose mutation
  answers after 200 ms. It runs in the async lane beside `reorder.contract`,
  which passes unchanged.

### D12 — `rules/DEFERRED.md`

Stage 1 adds under **Motion**, beside the `panrelease` and pointer entries:

> **Expanded (LLP 1091):** a reorder spanning lists that share a
> `reorderGroup`, on vertical and row virtualized lists and one level of
> nesting, with a top-layer ghost, a drop hold, keyboard and custom-action
> moves, and the action read `elementFromPoint`. Consumers: the two kanban
> builds. Unblocks a board without a hand-built drag. Take: none offered.
> Still out: drags out of or into the app, multi-item drags, grids, a
> gesture arena.

It needs Charlie's waiver, or the orchestrator's under his delegation, as
LLP 1089's was recorded (§8 Q1).

## 4. Effect on each implementation

| | Stage 1 | Stage 2 | Stage 3 |
|---|---|---|---|
| schema, compiler | `reorderGroup`; `ReorderEvent` in `event_record` and the payload table; `elementFromPoint`; `clientX`/`clientY`; row and nested refusals deleted; `lower-reorder-group` | — | — |
| runner | the session (`Outgoing`/`Incoming`); main axis; geometry v2; outer pin; hold; `reorder_step`; `elementFromPoint` | — | — |
| web, both targets | — | direction; popover ghost; target; nested autoscroll; hold; keyboard; `elementFromPoint` glue; `reorder.js`'s `Incoming` | — |
| Apple, Linux | geometry v2 encoders | — | ghost, target, autoscroll, hold, keyboard; Apple custom actions; iOS lift haptic |
| driver | — | `drag to`, the test step, `state.reorder` | macOS, Linux and iOS (`--touch platform`) drives |
| docs | grammar rows; LLP 1001 deviation; DEFERRED | the board recipe in `contract-for-agents.md` | — |

## 5. Tests

- **Compiler** (`contract/cli/tests/it/{reorder_collection,collection_axis,collection_nest}.rs`):
  a grouped list accepted; each `lower-reorder-group` refusal asserted whole;
  `reorderdrop` on a row list and a nested list accepted; a `ReorderEvent`
  parameter typed and a wrong record refused; `elementFromPoint` outside an
  action refused.
- **Runner**: outgoing and incoming offsets; a target change in one commit;
  a duplicate key opens no gap; a horizontal gap; the outer pin across a
  window move; each of the hold's three ends; cancel; keyboard steps across
  lists; `elementFromPoint` under a clip, `pointer-events: none` and a higher
  `z-index`; `runner/tests/it/reorder_codec.rs` for v2.
- **Hosts** (each host's reorder and arrange tests): a cross-list drag, a column drag, an
  outer scroller's autoscroll, the hold, the ghost's removal.
- **Conformance:** D11's fixture on Chrome, Firefox and WebKit.
- **Driven:** both kanban builds (outside the repo, `EXACT_APP_DIR`) convert
  their columns to grouped `list virtualized=true`, delete the hand-built
  drag (`dragCard`, `dragColumn`, the frame unrolling, the ghost,
  `boardScrollTo`) and pass their authored tests with `drag to` steps on web
  and macOS. One cross-column
  drag on the iOS simulator under `--touch platform`.

## 6. Implementation plan

Implementer: Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, on a
branch from origin/main. Each commit passes the five checks.

1. **Stage 1, 2026-10-06: compiler and runner** (D1–D4, the runner halves of
   D6–D8, D9, D10, D12). The Apple and Linux encoders move to geometry v2 so
   single-list reorder keeps running. **Exit:** §5's compiler and runner
   tests; interaction-gallery, exact-live and `reorder.contract` unchanged on
   web and macOS.
2. **Stage 2, 2026-10-07: the web, both targets, and the driver** (D5–D8,
   D11). **Exit:** the conformance fixture green on three browsers; both
   kanban builds converted and passing on the web.
3. **Stage 3, 2026-10-08: Apple and Linux** (D5–D8). **Exit:** the kanban
   builds' tests on macOS; Linux's arrange tests; one iOS real-touch drive.
   If the macOS ghost or the custom actions slip, the rest lands and the
   slipped piece gets a `QUEUE.md` line.

## 7. Considered and not taken

- **HTML drag-and-drop events** (Option A): a per-element target and no
  index, an image that cannot animate, weak on touch, costly to map onto
  UIKit and AppKit sessions (§2).
- **A ghost the app draws, the runner only reporting the target:** half the
  hand-build stays, and a row that moves itself is still clipped.
- **Firing the source list's action too:** two actions per gesture means two
  commits or an order between them.
- **`frame()` with transforms at their model values:** right only once
  motion settles, wrong mid-spring on every host but the web. Trigger: a
  consumer for which the layout box is wrong even after D1–D7.
- **The browser's own `elementFromPoint`** on the web: it would disagree
  with native (D9).

## 8. Open questions

1. **The DEFERRED waiver** (D12): Charlie's, or the orchestrator's under
   the 2026-10-04 delegation?
2. **Host announcements.** Should the web host own a polite `aria-live`
   region ("Moved X to position 3 of 7 in Doing")? It needs localized host
   strings, which v1 lacks; r1 leaves announcing to the app.
3. **Lift styling.** Is the lifted look (a shadow, UIKit's 1.05 scale) the
   host's or the app's? No state tells the app "lifted"; `state.reorder` is
   the driver's.
4. **A group across sessions** (the sample host's two embedded sessions):
   r1 says one session.

## 9. Revisions

- **r1** (2026-10-05): first draft.
