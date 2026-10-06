# LLP 1094: Dropping across lists

**Type:** RFC
**Status:** Accepted; stages 1–5 built 2026-10-04 (§6, "As built"). Accepted (r3, by the orchestrator under Charlie's delegation after three rounds; Grok 4.7 only — Codex budget exhausted). Each round was Grok 4.7 (xhigh). r1 (`cbe16cc1e`) had two scopes, semantics and authoring (`llp/reviews/1094-r1.grok-a.md`, READY WITH CHANGES) and implementation (`llp/reviews/1094-r1.grok-b.md`, NOT READY); r2 (`e52586d26`) had a delta review (`llp/reviews/1094-r2.grok.md`, NOT READY: five MATERIAL, five MINOR, two NIT). r3 is the final edit, with no further round, and resolves every finding of r2 (§10). The orchestrator ruled r1's open questions under Charlie's 2026-10-04 delegation.
**Systems:** Contract compiler and the test-step parser (`contract/syntax/src/parser/steps.rs`), `kernel/tables/schema.json`, runner (`collection/reorder*.rs`, `runner/reorder.rs`, `runner/pointer.rs`, `geometry.rs`), web (`host/web/{motion,geometry,input}-glue.js`, `host/web/src/reorder_drag.rs`, `host/web-js/{reorder,arrange,pointer,rt}.js`), Apple (`Reorder{IOS,Hold}.swift`, `MouseReorderMac.swift`, `Bridge.swift`, `arrange.rs`), Linux (`presenter/{arrange,contact,pointer}.rs`, `paint.rs`), the driver, conformance (`host/web-js/{conform.mjs,conformance}`), docs
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Revised:** 2026-10-05 (r2, r3)
**Implementer:** Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever: stage 1 on 2026-10-06, stage 2 on 2026-10-07, stage 3 on 2026-10-08, stage 4 on 2026-10-09–10, stage 5 on 2026-10-11 (§6)
**Amends:** LLP 1056 §8.6 (the `PointerEvent` record and its wire); LLP 1001 (one declared deviation, D11); `docs/contract-grammar.md`'s payload table; `rules/DEFERRED.md` **Motion** (D13)
**Related:** LLP 1051.000 D1 (`frame`); LLP 1057.001 rule 3; LLP 1070 §4.7 (still in force, §7); LLP 1083.000 (paint order); LLP 1088 §9.1; LLP 1012. Diaries: `~/projects/x2apps/{kanban,kanban2}/DIARY.md`. The fix/input lane's report of 2026-10-04 (Options A–C; C shipped in LLP 1051.000 D1).

## Summary

`reorderdrop` moves a row within one vertical virtualized list. Two kanban
builds moved cards between columns by hand, with `pan` deltas, `frame()` of
every column and card, a root ghost, an insertion index and edge autoscroll
bound to state. The first build took about 60 lines. The second took about
100, unrolled eight `frame` calls and capped a board at eight columns.

This RFC lets vertical virtualized lists that share a `reorderGroup` exchange
rows:

- **The drop.** It fires on the target list's `reorderdrop` with today's two
  keys. An action that takes one more parameter also gets a `ReorderEvent`
  naming both lists.
- **The board.** Each column holds a grouped card list, and the columns sit
  in a plain horizontal `scroll`.
- **Column reorder** needs a row list at the window's height and a nested
  list. Both are refused today, so it is deferred with its preconditions
  (§7).
- **Hand-built drags** get `elementFromPoint(x, y)` and `PointerEvent`
  client coordinates. `frame()` stays untransformed.

| | Decision | Diaries | Stage |
|---|---|---|---|
| D1 | `reorderGroup` on a vertical virtualized list | kanban F4, kanban2 | 1 |
| D2 | One drop, on the target: `item`, `before`, then optionally `ReorderEvent { from, to }` | kanban F4 | 1 |
| D3 | The board, and the action's one `send` | both | 1 |
| D4 | The runner's session: the source's token, two previews, one commit | — | 1 |
| D5 | The host calls and wires | — | 1–5 |
| D6 | Lift, the host's ghost and its look | kanban2 (aim) | 2–5 |
| D7 | Target, gap and autoscroll | both | 1–5 |
| D8 | Three endings: cancel, drop, hold | — | 1–5 |
| D9 | Keyboard and assistive technology | kanban (built by hand) | 1–5 |
| D10 | `elementFromPoint(x, y) -> option<string>` | kanban2 #2 | 1–2, 4–5 |
| D11 | `frame()` untransformed, declared; `PointerEvent` gains `clientX`/`clientY` | kanban2 #3 | 1 |
| D12 | Driver, test files and conformance | kanban F14, F18 | 2 |
| D13 | `rules/DEFERRED.md` | — | 1 |

## 1. Evidence

- **Kanban, first build (F4).** `app.contract:198–245` maps `frame()` over
  every column and the target's cards on each move. It finds the column
  with `length(filter(lefts, l => l <= px))` and the slot with
  `length(filter(mids, m => m < py))`, and sets `boardScrollTo` within 48 px
  of an edge. `moveCard` (`app.ts:302–309`) inserts at the end when `before`
  is missing. `stepCard` left/right keeps the index, clamped (`:311–320`).
- **Kanban2** (`423e4c4bc`). `app.contract:480–580` unrolls
  `frame("col-0")`…`frame("col-7")`. A drop aimed at the grip's
  untransformed frame stayed in the source column while the card was over
  the next one. The fix was the card's centre, `base + delta + size/2`.
- **What exists** (verified at `e097e4cae`):
  - The payload is `item`, `before` (`analyze/src/payload.rs:37`), with no
    record (`types/src/selection.rs:7–23`).
  - A virtualized row list needs a literal `height`
    (`lower-collection-cross`, `lower/src/collection.rs:139`). A nested list
    needs a literal `height` or `max-height` (`:186`). There is one direct
    `each` (`:72`).
  - The runner has one owner (`runner/reorder.rs`) and one preview per
    collection. `Offsets::at` shifts rows only between a source index and
    `before` in that list (`collection/reorder.rs:293–310`). `end_preview`
    zeroes every target in the drop's commit (`:240`).
  - The JS target reimplements the preview, the owner and the drop
    (`host/web-js/reorder.js`, `arrange.js`).
  - The wires are vertical: the web motion packet v3 (176-byte header,
    `reorder_drag.rs`), Apple's `exact_reorder_move(rt, token, dy,
    scroll_top, …)` (`exact.h:363`), Linux in process.
  - The pumps run only while an edge band scrolls (`motion-glue.js:1034`,
    `ReorderHold.swift:209`).
  - Line caps: `host/linux/src/paint.rs` is at 1,498 lines, `motion-glue.js`
    at 1,146 and `rt.js` at 1,338.

## 2. What the platforms offer

- **HTML drag and drop** moves data between elements and applications.
  - A target hears `dragover` per element and computes its own index.
  - The drag image is a bitmap the page cannot animate.
  - Touch support came late, through a long press; touch sortable libraries
    use pointer events (SortableJS's fallback).

  Taken from the web: the names (SortableJS's `group`, `from`, `to`; DOM's
  `elementFromPoint`, `clientX`, `clientY`), `pointercancel`, and the top
  layer. SortableJS's group also means `pull`, `put` and `clone`. Here a
  group is only a shared name and one list-to-list move.
- **UIKit.** `UICollectionView`'s drop delegates move items between
  collection views. A `.move` proposal opens a gap, and a
  `UICollectionViewDropPlaceholder` holds the slot until the data source
  commits; D8's hold is that placeholder. A session that ends outside every
  view fails the drop. D8 commits to the last target instead, as both
  boards and SortableJS do.
- **AppKit.** `NSTableView` and `NSCollectionView` show a gap, and
  `animatesToStartingPositionsOnCancelOrFail` snaps back. D7 and D8 take
  both. Exact starts no `NSDraggingSession`.

## 3. Decisions

### D1 — `reorderGroup`

```
list id=`cards-${col.id}` virtualized=true reorderGroup="cards" reorderdrop=dropCard(col.id) flex=1 min-height=48
  each card in filter(col.cards, k => k.visible) key=card.id
    column …
      box reorderFor=`cards-${col.id}` touch-action="none" aria-label=`Move ${card.title}` …
```

- **The prop.** `reorderGroup` is a string prop on `list`. It takes the
  table's next free prop id; 77 stays `reorderFor`.
- **What it does.** Vertical virtualized lists with the same non-empty
  value, mounted in one session, exchange rows.
- **What a grouped list needs.** String keys, a `reorderdrop` and an `id`.
  Otherwise `lower-reorder-group`: "`reorderGroup` joins lists that take a
  drop: give this `list virtualized=true` a `reorderdrop` and an `id`".
- **What it refuses.** A row list and a nested list still get
  `lower-collection-reorder` (§7).
- **Groups stay within one session** (ruled).
- **Keys.** The source list gaps around the lifted key, as today. Any other
  list that already holds `item` is not a target: the sticky target stays,
  no drop fires there, and the development journal says so once a drag. An
  index would reject the duplicate (`collection/index.rs:27`, `DuplicateKey`).

### D2 — The drop

- **One drop, on the target list.** It fires once, on the **target** list's
  `reorderdrop`, with `item` and `before` (the target's key it lands before,
  `none` at the end).
- **The record.** `reorderdrop` joins `event_record`. An action that takes
  one more parameter gets `ReorderEvent { from: string, to: string }`, the
  two lists' `id`s. Within one list, `from == to`.
- **The source fires nothing.** One gesture is one action and one commit.
- **The grammar row reads:** "A string, an `option<string>`, then
  optionally a `ReorderEvent` | `reorderdrop`".

### D3 — The board, and the action

- **Layout.** Columns are a plain `each` in a horizontal
  `scroll overflow-x="auto"`. Each column is a flex column: a header, the
  grouped list (`flex=1`, a bound on a list that is not nested), and the
  quick-add.
  - The header and the quick-add are outside the list, so they act as
    gutters (D7).
  - The list keeps one direct `each`, over the visible cards. The filtered
    keys then take no slots.
  - `min-height` makes an empty column a target.
- **The action.** Board data is the data module's (LLP 1088 §9.1), so the
  drop's action is one `send`:

  ```
  action dropCard(col: string, item: string, before: option<string>)
    let at = match before { case some(k) => k, case none => "" }
    send edited = edit("moveCard", boardId, item, col, at)
  ```

- **Grips.** The conversion deletes `press`, `pan`, `pointerdown` and `key`
  from each grip (kanban2's `press=gripTap`), so D9's keys install.
- **Columns.** Column drag stays the app's (§7).

### D4 — The runner's session

- **One token.** The token stays the **source**'s identity for the whole
  gesture, and `reorder_owner` stays the source list. The session adds
  `target: NodeKey` (initially the source) and a phase: `active`,
  `holding`, `cancelling`, `settling`.
- **The previews.** `preview_reorder_into(token, target, geometry, y)`
  writes both previews in one commit:
  - **target == source:** today's `Offsets::at`, byte for byte, so
    `reorder.contract` and every in-list host path are unchanged.
  - **Outgoing,** on the source: the source row keeps its slot. The host
    hides it, and each row after it translates by `−h` (h = the source
    row's height).
  - **Incoming { key, extent },** on the target: `certified_gap_excluding`
    with nothing excluded picks the gap. Every row at or after it
    translates by `+extent` (extent = h), and rows before it by 0.
  - **Transitions, each in one commit.** Leaving the source replaces
    `Offsets::at` with `Outgoing` on the source and opens `Incoming` on the
    new target. Moving between foreign lists closes the old `Incoming` and
    opens the new one. Returning to the source closes `Incoming` and
    `Outgoing` and writes `Offsets::at` again.
  - **An empty target.** An empty index with a positive port certifies gap
    0, `before = none`, with no measured row. Today `gap` returns `None` for
    an empty index (`index/gaps.rs:22`), and the drop would never certify.
- **The drop** (`drop_reorder`) dispatches `reorderdrop` on
  `session.target`. Every other step (`begin`, `cancel`, `reorder_frame`,
  `finish`, `reconcile_reorder`) keeps the source token.
- **The JS target.** `host/web-js/reorder.js` and `arrange.js` implement
  the same session in stage 2, case for case (`Outgoing`, `Incoming`,
  retarget, the three endings), so the wasm and JS targets agree in
  conformance.

### D5 — The host calls and wires

The 68-byte `ReorderGeometry` and every existing packet are unchanged. Each
host gains one call:

- **Web, wasm:** motion op 21, `reorder-preview-into`, made legal in
  `Input::decode` (today `15..=20`, `reorder_drag.rs:106`). It uses the v3
  header with the source binding and token. `count = 1`: the record's key
  is the target list, and its serial and two floats are 0. The header's
  geometry floats, revision and scroll sequence are the target's, and the
  decoded `ReorderGeometry.list` is the record's key, not `binding.list`. It
  is certified with `reorder_geometry(target)`.
- **Web, JS target:** `arrange.js`'s `reorder-preview` packet gains
  `target` and `targetGeometry`. Its staleness check compares
  `targetGeometry` with `L().geometry(target)`.
- **Apple:** `exact_reorder_move_into(rt, token, target_key, content_y,
  target_scroll_top, inside, now_ms)`. Here `content_y` is the ghost's
  centre in the target's content coordinates.
- **Linux:** the same call, in process.

Each host computes `y` in the target's content space: the ghost centre's
viewport y, minus the target port's viewport top, plus its scroll top.
`inside` means the ghost's centre is in that target's port.

- **Only an inside sample moves `before`.** When the centre is outside
  every grouped port (a header, the quick-add, a gutter), the host does not
  call `preview_reorder_into`, so the last certified gap stands, and so
  does the drop on it.
- **The source keeps today's semantics.** On today's in-list call, a false
  `inside` keeps the existing end probe (`arrange.rs:256`).

### D6 — Lift and ghost

- **Recognition** is each host's, as today. For a grouped list, a move past
  the slop in any direction lifts; `touch-action="none"` keeps scrolling off
  the grip.
- **The ghost.** A grouped lift shows the row as a ghost in the top layer,
  since the real row cannot leave its scrollport's clip. The source keeps
  its node identity, ids and testIds, hidden while D4's `Outgoing` is
  active.
  - **Web:** a `cloneNode(true)` of the wrapper, with these stripped from
    it and every descendant: `id`, `data-view` (which `document-glue.js`
    adopts, `:199`), `data-exact-on`, `data-agent-view` and
    `data-testid`. It sits in a `popover="manual"`
    shown with `showPopover()`, with `position: fixed` and `pointer-events:
    none`.
  - **iOS:** `snapshotView(afterScreenUpdates: false)` in the window.
  - **macOS:** `cacheDisplay(in:to:)` into a topmost image view.
  - **Linux:** the lifted subtree painted last with no ancestor clip, in a
    new `presenter/lift.rs` that the paint walk calls. Today's lift moves
    there too, so `paint.rs` shrinks.
- **New files.** The web ghost, retargeting, hold and keyboard live in a
  new `host/web/group-glue.js` that `arrangeController` calls, not in
  `motion-glue.js`. `navigation.js` loads it beside `motion-glue.js`
  (`:254`). Apple's live in `ReorderGroup.swift`.
- **The look is the host's** (ruled): a shadow (`0 8px 24px` at 25% black)
  and a 1.03 scale, the same on every host. There is no scale under
  `prefers-reduced-motion`. iOS also plays a light impact at the lift.
- **The ghost's centre** picks the target and the gap, not the pointer. The
  ghost follows the contact by the grip's offset.

### D7 — Target, gap and autoscroll

- **The target** is the grouped list whose scrollport's viewport box
  contains the ghost's centre.
  - Over no list (a gutter, a header, the quick-add), the last target
    stays, as SortableJS keeps its placeholder.
  - A list that holds `item` is never a target (D1).
- **Autoscroll** is geometric, not a parent walk: the ghost is detached
  (D6). The candidates are the target list's port, then each scroll
  ancestor of the target list in the source tree whose viewport box
  contains the ghost's centre (or the source's, before any retarget).
  - The innermost candidate that can still move toward its edge band on its
    own axis scrolls first; for a board, that is the column vertically,
    then the board horizontally.
  - Each host keeps its band and speed, measured on that scroller's axis.
  - The pump runs while any candidate is in a band.
  - After a scroll, the gap re-certifies on source and target.
- **The gap is the indicator.** No line is drawn.

### D8 — Three endings

1. **Cancel, only before the drop.** Any of these cancels:
   - Escape;
   - `pointercancel`;
   - a real loss of the grip's capture (a child's loss while the grip takes
     capture is not a cancel, habits F10, `motion-glue.js:1100`);
   - the window resigning key;
   - the source binding dying.

   On a cancel the ghost springs home, both previews close, and no action
   runs.
2. **Drop.** A pointer-up anywhere, inside the window or out (the grip
   holds capture), drops into the current, sticky target. Leaving the
   window does not cancel.
   - If the drop's commit already shows the move, the preview ends as today
     (terminal, rebase, finish). Every host's current path stays correct
     for a synchronous action.
   - Otherwise `drop_reorder` returns `holding`. The `Incoming` and
     `Outgoing` targets stay, the hold is not released, and the ghost stays
     at the gap.
3. **The hold ends** at the first commit where one of these is true:
   - **It landed.** With `before = some(k)`, the key's successor in the
     target is `k`. With `none`, the key is the target's last.
   - **It landed elsewhere.** The key is in the target at another index.
     The ghost springs onto that row in both cases.
   - **It is gone.** The key is in no grouped list, so the ghost fades.
   - **Timeout.** 1 s has passed on the session clock. The ghost springs
     onto wherever the key now is (home if unmoved), and the journal says
     `reorderdrop: the move did not show within 1 s`.

   **The deadline.** The session carries `deadline_ms`, set at the drop.
   `timer_due_ms` (`runner.rs:1199`) includes it, and `advance_within`'s
   due selection (`commit.rs:239`) fires it beside the plan's timers and
   `then_due`. So the web batch publishes it, Linux sleeps on it, and the
   agent's `clock settle` reaches it, on a board with no timer of its own.
   The firing commit carries `ending: timeout`.

   **The signal.** Each host learns the ending on the channel it already
   reads:
   - **Web:** the batch op `reorder-state` (`batch.rs:129`) gains `phase`
     (`holding` or `settling`) and `ending` (`landed`, `gone` or
     `timeout`). `group-glue.js` keeps the ghost while `holding`.
   - **Apple:** the reorder op's `phase` gains `holding`, with `ending`
     (`exact.h:358`). `ReorderHold.observe` keeps a `holding` phase.
     `ReorderHold.finish` skips its `cancel()` and `finished()`
     (`ReorderHold.swift:153–155`) when the answer is `holding`.
   - **Linux:** in process, it reads `reorder_frame`, which gains `ending`. While
   `holding`, Escape does nothing, because the send is out. A new lift is
   refused while `holding`, `cancelling` or `settling`. The source pin is
   kept until `finish_reorder`, after the spring settles.

### D9 — Keyboard and assistive technology

- **Keyboard,** on a grip with no `press`, `key`, `pan` or `pointerdown`
  handler of its own.
  - **Focus.** The host makes the grip focusable (`tabindex=0` and
    `role="button"` on the web; the key view loop on macOS).
  - **Keys.** Space lifts. Up and down move `before` one row. Left and
    right move to the previous or next grouped list **in tree order** (all
    are mounted, D3) at the same index, clamped to the end; `before` is that
    list's key at the index, or `none`. The gap is kept in view with
    `scrollIntoView`'s `nearest`. Space or Enter drops, and Escape cancels
    (dnd-kit's keys).
  - **The drop** is D8's, hold included. Focus then goes to the moved row's
    grip when the move landed, and back to the source grip otherwise.
- **Custom actions.** iOS (`accessibilityCustomActions`) and macOS
  (`NSAccessibilityCustomAction`) get "Move earlier", "Move later", "Move to
  previous list" and "Move to next list".
  - Each is `begin_reorder`, one `reorder_step`, and `drop_reorder`, in one
    turn. It uses the same index rule and draws no ghost.
  - The hold still runs. With no ghost, the gap closes in place when the
    hold ends, and focus follows the keyboard's rule.
  - The web's path is the keyboard. Linux has no accessibility tree
    (LLP 1015 §7), so it gets the keyboard.
- **The runner's `reorder_step(token, direction)`** serves both paths.
- **Announcements are the app's** in v1 (ruled), through its own
  `aria-live` text.

### D10 — `elementFromPoint(x, y)`

- **Signature.** `elementFromPoint(x: number, y: number) -> option<string>`
  is a stdlib action read. Outside an action it is refused with
  `type-geometry-outside-action`.
- **What it returns.** The `id` of the topmost node at viewport `(x, y)`, or
  of its nearest ancestor with an `id`. That is DOM's
  `elementFromPoint(x, y)?.closest("[id]")?.id`. Contract names elements by
  id, and `testId` is the driver's.
- **Which boxes count.** `frame()`'s: layout boxes in the viewport, every
  scroll applied, no transforms. Ancestors' `overflow` clips apply.
  `pointer-events: none`, `display: none` and `visibility: hidden` are
  skipped. Front to back follows LLP 1083.000: among one parent's
  children, the higher rank paints later, then tree order, and a child is
  in front of its parent. `Kernel::paint_order`'s post-order vec gives the
  ranks (`paint_order.rs:307`). The walk descends by those ranks; scanning
  the vec backwards would put an ancestor first. The hit is the front-most
  untransformed box that contains the point, then the nearest ancestor with
  an `id`.
- **Where it runs.**
  - **Native:** the runner, from kernel boxes and `Scrolled`.
  - **Web:** a new `point(x, y)` in `geometry-glue.js` over the same
    untransformed boxes, reached from a few lines of `rt.js` (`x_elementFromPoint`)
    and the wasm import beside `read`.
  - Linux's `Presenter::hit` and the browser's own `elementFromPoint` test
    painted, transformed boxes, so neither is used.
- **The recipe, for a drag still built by hand.** Test the dragged node's
  **visual centre**: `frame(id)`, plus the pan delta the action stored, plus
  half the box. Do not test the pointer. `clientX`/`clientY` (D11) are for
  hits that really want the pointer. A `pan` carries only deltas.

### D11 — `frame()` stays untransformed; `PointerEvent` gains client coordinates

- **`frame()` keeps LLP 1051.000 D1's box.** `getBoundingClientRect`
  includes transforms, so this deviation is declared in LLP 1001:
  - A presented transform lives in each host's motion engine, or in CSS,
    which the runner cannot read during an action.
  - Including it on the web alone would break parity.
  - Decisions ask where layout put a box.
- **`PointerEvent` gains `clientX` and `clientY`,** the viewport point in
  CSS px, after `pointerId`:
  - **The string wire.** It becomes
    `offsetX,offsetY,buttons,pressure,pointerType,pointerId,clientX,clientY`,
    with the optional modifier chord still last (Apple and the web glue
    write it). Linux still assigns `held` after the parse.
  - **The JS target's array and the type's shape.** The two numbers go
    after `pointerId` and before the four modifier bools, in the shape
    (`types/src/selection.rs:58`), in `value()`, and in `pointer.js`'s
    array.
- **Every writer changes in stage 1:**
  - `runner/src/runner/pointer.rs`'s `parse`;
  - Linux `pointer_record` (`presenter/pointer.rs:39`);
  - Apple `PointerSample.line` (`Bridge.swift:603`);
  - `host/web/input-glue.js`'s `record`;
  - `host/web-js/pointer.js`'s `record`.

  There is no compatibility parse. All the writers are in the repo
  ("Delete; don't deprecate").

### D12 — Driver, test files and conformance

- **The agent.** `tap A drag to B [at x y] [over ms] [hold ms] [during "op"…]`
  ends at B's box centre, or at `(x, y)` from B's top-left. The delta comes
  from both boxes at the press, then today's `dragTap` runs. If B is
  unmounted or off screen, it is refused by name. An autoscrolling drag is
  written with `drag dx dy hold ms`.
- **Test files.** `tap "A" drag to "B"`, in `parser/steps.rs` beside
  `drag`.
- **`state.reorder`.** During a session it reads `{ item, from, to, before,
  phase }`.
- **The keyboard path** drives with `type <grip> key Space` and the arrow
  keys.
- **Conformance.** `conform.mjs` gains the op `drag <target> to <target>
  [ms]`. The fixture is `reorder-group.contract` with its `.steps`:
  - three grouped lists in a horizontal `scroll`, one of them empty;
  - a cross-list drag, a drop into the empty list, and a drag that
    autoscrolls the board;
  - Escape mid-drag;
  - a keyboard move across lists;
  - a drop whose mutation answers after 200 ms, and one that never answers
    (the timeout).

  The Chrome pair (wasm and JS) runs every step. Firefox and WebKit skip
  drag steps, as they skip `reorder.steps` today (`conform.mjs:356`), and
  run the keyboard steps. Linux joins in stage 5:
  - `drag` is added to `LINUX_OPS` (`:579`);
  - a step Linux refuses is skipped and the session kept, as Firefox skips
    `drag`, so it no longer ends the comparison (`onLinux`, `:262`).

### D13 — `rules/DEFERRED.md`

The orchestrator's waiver, under Charlie's 2026-10-04 delegation, is
recorded in its own commit with r2. It goes under **Motion**:

> **Expanded (LLP 1094; waived by the orchestrator under Charlie's
> 2026-10-04 delegation, "make decisions without me"):** a reorder spanning
> vertical virtualized lists that share a `reorderGroup`, in one session.
> It brings a host-drawn top-layer ghost, a drop hold, keyboard and
> custom-action moves, the action read `elementFromPoint`, and
> `PointerEvent`'s `clientX`/`clientY`.
>
> - **Consumers:** the two kanban builds (F4).
> - **Unblocks:** card moves on a board without a hand-built drag.
> - **Take:** none offered.
> - **Still out:** reorder on row and nested lists (LLP 1094 §7), drags
>   into or out of the app, multi-item drags, grids, and a gesture arena.

## 4. Effect on each implementation

| | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| compiler, schema | `reorderGroup`, `ReorderEvent`, `elementFromPoint`, `lower-reorder-group`, step `drag to` | — | — | — | — |
| runner | session, `preview_reorder_into`, the endings and deadline, `reorder_step`, `elementFromPoint`, pointer wire | — | — | — | — |
| web, both targets | pointer writers | op 21, `group-glue.js`, the JS session, `point()` | — | — | — |
| driver, conformance | — | `drag to`, `state.reorder`, the fixture | — | — | — |
| apps | — | — | kanban ×2 converted (web) | macOS run | — |
| Apple | `PointerSample` | — | — | `move_into`, ghost, autoscroll, hold, keys, custom actions | — |
| Linux | `pointer_record` | — | — | — | `lift.rs`, target, autoscroll, hold, keys; `LINUX_OPS` `drag` and skip-not-stop in `conform.mjs` |

## 5. Tests

- **Compiler:**
  - a grouped list is accepted;
  - each `lower-reorder-group` refusal is asserted whole;
  - row and nested refusals are unchanged;
  - `ReorderEvent` is typed;
  - `elementFromPoint` outside an action is refused;
  - the test step `drag to` parses.
- **Runner:**
  - an in-list preview is byte-identical to today's;
  - `Outgoing` and `Incoming` offsets;
  - a retarget happens in one commit;
  - a foreign list holding `item` is never a target;
  - each ending: a synchronous drop, landed (`some`, `none`), elsewhere,
    gone, and the timeout reached by `clock settle` with no plan timer
    (`timer_due_ms` reports it);
  - leaving the source, returning to it, and moving between foreign lists;
  - an empty target certifying `before = none`;
  - an outside sample leaving the certified gap;
  - op 21 decoded, with the target's geometry certified;
  - a custom action as begin, step, drop;
  - Escape while holding is ignored, and a lift while holding is refused;
  - `reorder_step` across lists with clamping;
  - `elementFromPoint` under a clip, under `pointer-events: none` and under
    a higher `z-index` in a sibling's subtree;
  - the eight-field pointer wire.
- **Hosts.** Each host's reorder tests add a cross-list drag, an
  autoscrolled board, the hold, the timeout and the ghost's removal. The web
  ghost's clone is checked to carry none of D6's five attributes. Swift's
  `ReorderHold` keeps a `holding` answer.
- **Conformance:** D12's fixture.
- **Driven.** Both kanban builds (`EXACT_APP_DIR`) convert their card lists
  (D3) and delete `dragCard`, `dropCard`, the frame unrolling, the card
  ghost and the card half of `boardScrollTo`. Their authored tests pass
  with `drag to`, on the web in stage 3 and on macOS in stage 4. One iOS
  `--touch platform` drag runs in stage 4.

## 6. Implementation plan

Implementer: Claude (Opus 5.5) lanes, orchestrated for Charlie Cheever, from
origin/main. Each commit passes the five checks. Every new web or Linux
behaviour goes in a new file (D6), not in a file near the cap.

1. **2026-10-06, compiler and runner** (D1, D2, D4, D8–D11's runner halves,
   D13, and the pointer writers).
   - **Exit:** §5's compiler and runner tests;
     `interaction-gallery`, `exact-live` and `reorder.contract` unchanged.
2. **2026-10-07, the web (both targets), the driver and conformance** (D5–D10,
   D12).
   - **Exit:** the fixture green on the Chrome pair, with Firefox and
     WebKit running the keyboard steps.
3. **2026-10-08, the kanban conversions on the web.** They start only after
   stage 2's fixture is green.
4. **2026-10-09–10, Apple.**
   - **Exit:** the kanban tests on macOS; one iOS real-touch drag.
5. **2026-10-11, Linux.**
   - **Exit:** the arrange tests, and the fixture's Linux run with `drag`
     in `LINUX_OPS`.
   - If the macOS ghost, the custom actions or `lift.rs` slip, the rest
     lands and the slipped piece gets a `queue/` entry.

### As built (stages 1–5, 2026-10-04)

All five stages were built on one day by one lane with two sub-lanes (Apple,
Linux) working from stage 1's commit; each stage's commits pass the five
checks. Where the code disagreed with a decision, the decision moved, and
each such point is listed here.

- **Stage 1.** As D1, D2, D4, D8–D11, with these differences:
  - **Two begins.** `begin_reorder` keeps today's in-list session on every
    list, a grouped one included, so a host that has not adopted groups is
    unchanged. A host opts in with `begin_group_reorder(binding, geometry,
    ghost)`. A ghost session (a pointer's) hides the row through the runner's
    `visibility: hidden` until `finish_reorder`; a key's or custom action's
    has none and keeps its row shown, even while another list is the target.
    Hiding it took the keys away: a hidden `NSView` resigns first responder, a
    browser blurs the element, and the driver refuses to type at it (the Apple
    lane found it). The source still closes the gap behind the row.
  - **Holding only when grouped.** An ungrouped list's drop keeps today's
    terminal path. A grouped drop holds, and offsets that move with their rows
    in one commit apply with `transition: none`, so a landed move needs no
    host rebase; a cancel or a timeout springs.
  - **What the hold watches.** "Landed elsewhere" is read as "the row is no
    longer where it was at the drop" (its list or its neighbours changed). At
    the drop of a move within one list, the row is still in the target, so
    the literal rule would have ended every hold at once. An unrelated edit
    beside the row ends a hold early, and the row is then where it is.
  - **The record.** `ReorderEvent { from, to }` is filled at dispatch from
    the lists' `id`s; `Event::ReorderDrop` keeps its two fields.
  - **The frame.** `ReorderFrame` gains `phase`, `ending`, `target` and
    `row`, which is the wrapper that now holds the dragged row, where a ghost
    lands. `state.reorder` also carries `ending`.
  - **Steps.** `reorder_step` returns its receipts, the second one the
    `scrollIntoView` (`nearest`) that keeps the gap in view.
  - **String keys** are checked by `lower-reorder-group` when the key's type
    is known.
  - **Lean.** The semantics refuse `elementFromPoint` as unsupported, as they
    refuse `frame`.
- **Stage 2.**
  - **The wire.** Op 21 names the target by its view id, not its key: the
    page holds no other list's key. The header's spare word carries a grouped
    begin's kind (1 for a ghost, 2 for the keys) and op 22 (`reorder-step`)
    carries the direction. A grouped drop is op 17 with the same one record.
    Grouped sessions take no motion holds. A `reorder-group` batch op gives
    each grip its group, and whether the keys may drive it.
  - **The ghost** is the wrapper's clone, carrying its computed style
    inline: the page's rules for the app's root do not reach a top-layer
    popover. The click a lifted drag's release makes is swallowed, since a
    card is often also a button.
  - **JS target fixes.** A computed `reorderFor` (`cards-${col.id}`) was
    never bound, because the target resolved only literal names; it now
    names its list at run time, as the runner does. A grip built after the
    motion piece arrived was registered before its row was in the document,
    and was dropped for good; it now waits.
  - **Conformance.** `reorder-group.contract` runs on interaction-gallery's
    data, which gains a small board. The steps add `drag … to`, `drag … hold`
    and `move`. A 200 ms tick stands in for the late answer. 31 of 31 steps
    are equal on the Chrome pair, on Firefox and WebKit (which skip pointer
    steps), and on the Linux reference, which now also takes held contacts'
    phases. `state.reorder` is compared everywhere.
- **Stage 3.** Both kanban builds were converted in copies outside the
  repository, with patches beside them.
  - **kanban.** Its cards are the grips, and their own `press` and `key`
    stay. `ColumnView`'s list replaces the scroll, the drop marks and the card
    ghost; the board's columns become full height so a list is bounded. Its
    module imports needed LLP 1091's explicit names first. A new test drags a
    card into another list and back.
  - **kanban2.** `grabCard`, `panCard`, the frame unrolling, eight scroll
    slots and the card ghost are deleted. `app.ts`'s geometric `drop-card`
    becomes `place-card`, by key. Its drag tests use `drag to`.
  - **Results.** Every authored test passes on the JS target, on wasm and on
    macOS: kanban 15 of 15, kanban2 7 of 7.
  - **§9 Q1.** kanban2's `place-card` answered in 7–36 ms of wall time, so
    1 s stands.
- **Stage 4 (Apple).**
  - **The calls.** `exact_reorder_group_begin`, `exact_reorder_move_into`
    (by view id), `exact_reorder_step`, `exact_reorder_group_end` and
    `exact_reorder_group_finish`, in `arrange_group.rs`, with
    `ReorderGroup.swift`.
  - **Visibility.** A box with `visibility: hidden` now hides on Apple, as
    `display: none` does; Apple had ignored the row. Its subtree hides with
    it, a declared deviation in LLP 1001.
  - **Mouse.** A grouped mouse drag takes its drags from the window, since a
    hidden grip gets no more of its contact. The macOS agent now sends its
    contact through `NSApp.sendEvent`.
  - **The landing** runs in real time, except under the agent, where the
    ghost lands at once so `clock settle` sees the session finish.
  - **Not built.** iOS has the custom actions and no keyboard.
  - **Driven.** Both kanbans' tests pass on macOS, and one `--touch
    platform` drag on an iOS simulator moved a card into the next list.
- **Stage 5 (Linux).**
  - **The lift.** The lifted subtree is painted by `paint/lift.rs`, not
    `presenter/lift.rs`, because the paint walk is private to `paint`; the
    session is in `presenter/group.rs`. `paint.rs` went from 1,499 to 1,472
    lines.
  - **The ghost** is the row's subtree painted again rather than a
    snapshot, so a `gone` ending cannot fade it.
  - **Visibility.** The painter now skips `visibility: hidden`, with the same
    subtree deviation as Apple.
  - **Escape** cancels a ghost drag whatever has the focus.
- **Tests.** Runner: `contract/cli/tests/it/reorder_group.rs` (12), the
  geometry test of `elementFromPoint`, and the parser's `drag to`.
  Hosts: `host/web/src/reorder_tests.rs` (op 21),
  `host/web/tests/group.test.mjs` (the ghost's clone),
  `host/apple/src/arrange_group_tests.rs`, `ReorderGroupMacTests.swift`, and
  Linux's `arrange_tests.rs` and `tests/it/arrange.rs`.
- **Note, 2026-10-04 (batch 6 integration): D11's first half is
  superseded.** fix/geometry landed beside this LLP and amended LLP
  1051.000 D1: `frame()` now answers the box where the viewer sees it,
  through every transform on it and above it, as `getBoundingClientRect`
  does (natively from the committed transform rows; a transform in flight
  counts at its end). Owner's decision: that change wins. LLP 1001's
  "`frame(id)` answers the untransformed layout box" deviation is dropped,
  and D10's "no transforms" goes with it. `elementFromPoint` tests the
  transformed boxes: natively the runner's walk maps the point into each
  box's own space through the composed transform (`kernel_point`,
  `own_transform`), so a turned box answers its own shape; on the web it is
  the page's own hit test (`document.elementsFromPoint`, less leaving nodes
  and anything outside the root) in place of the walk over layout boxes.
  The visual-centre recipe stands, now without adding a pan delta when the
  dragged node itself carries the `translate`. D11's second half
  (`clientX`/`clientY`) is unchanged.

## 7. Deferred, with preconditions

- **Reorder on a row list and on a nested list** (r1's D4: dragging
  columns). LLP 1070 §4.7's refusals stand. They lift when all of these
  land, with a consumer:
  1. A virtualized row list that takes a parent-bounded height (a
     percentage or `flex`, not only a literal; `lower-collection-cross`),
     and an inner list bounded by its stretched row (`lower-collection-unbounded`).
  2. `edit_walk` (`traversal.rs:633`) descends `collection.mounted` as
     `find_collection_mut` does, pushing each outer row's frame.
  3. An interaction pin on the outer row, paired with the inner grip's
     lease and released in `finish_reorder`.
  4. A main axis on every wire: `axis` in the motion header's spare `u32`,
     the floats read as `scroll_main`/`port_main`, and a main-axis sample
     in place of `dy`/`scroll_top` on Apple and Linux. All of them change
     together (`reorder_drag.rs`, `motion-glue.js`, `reorder.js`,
     `arrange.js`, Apple, Linux, `exact-live`'s `arrange.rs` test).
  5. Keyboard and custom actions reach an unmounted neighbour, as LLP 1070
     §4.7 owes.

  Until then, kanban's column drag stays the app's.

## 8. Considered and not taken

- **HTML drag-and-drop events** (Option A): see §2.
- **A ghost the app draws:** half the hand-build remains, and a row is
  still clipped.
- **Firing the source's action too:** two commits for one gesture.
- **Cancelling when the pointer leaves the window** (UIKit): both boards
  commit to the last target.
- **A compatibility parse of the six-field pointer line:** every writer is
  in the repo.
- **The browser's `elementFromPoint`:** it would disagree with native.

## 9. Open questions

1. **Is 1 s the right hold bound?** It is long enough for a local data
   module and short enough not to strand a ghost. A networked board may
   want more. Stage 3 measures kanban's answer time.

Decided (the orchestrator, 2026-10-05, under Charlie's delegation): the
DEFERRED waiver (D13); `aria-live` is the app's in v1; the lifted look is
the host's (D6); a group stays within one session (D1).

## 10. Revisions

- **r3** (2026-10-05, final; round 3 of 3). It answers the delta review of
  r2 (`llp/reviews/1094-r2.grok.md`). Each finding was checked against the
  code and each held; the dispositions are in that file.
  - **The hold:** the deadline joins `timer_due_ms` and `advance_within`;
    the ending rides `reorder-state` and Swift's `phase`.
  - **The gap:** an empty target certifies gap 0; an outside sample keeps
    the certified gap.
  - **Wires and transitions:** op 21's decode and certification; the
    pointer wire's order; the retarget transitions; custom actions as
    begin, step and drop.
  - **The hit order:** spelled out.
  - **The clone and its loader:** the clone's four attributes and
    `group-glue.js`'s loader.
  - **Linux:** its conformance moves to stage 5.

  The orchestrator accepted it under Charlie's delegation.
- **r2** (2026-10-05, round 2 of 3: it answers round 1's reviews). This revision answers the two Grok 4.7
  xhigh reviews of r1, which are one family with two scopes. Every finding
  was checked against the code at `e097e4cae`.
  - Accepted (A1–A9, B1–B11): the dispositions are in
    `llp/reviews/1094-r1.grok-{a,b}.md`.
  - Rejected: one part of B8, the compatibility parse.
  - Descoped: row and nested reorder (A1, B2's axis, B4), to §7.
- **r1** (2026-10-05): first draft. It was pushed first as `llp/1091-…`
  (`42bc4e658`), a number LLP 1091 (Contract modules) already held, and was
  renumbered 1094.
