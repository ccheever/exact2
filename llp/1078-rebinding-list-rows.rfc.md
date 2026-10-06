# LLP 1078: Rebinding list rows instead of building them

**Type:** RFC
**Status:** Built 2026-10-05 on `android/reuse` (opt-in per runner; on in the Android Canvas host, off everywhere else). Measured on a Pixel 10a (§5).
**Systems:** Runner (`instance/collection/reuse.rs`, `realize_window`, `Runner::set_row_reuse`), Kernel (`CommitReceipt::renewed`; motion, paint motion and layout motion treat a renewed node as destroyed and created), Linux/Canvas host (`Host::commit_effects`, `Presenter::renew`, `Images::renew`; `EXACT_ROW_REUSE`)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-05
**Related:** LLP 1010 §6 (row state dies on retirement), LLP 1050.000 §7 (view reuse on iOS; "with a host reset contract"), LLP 1068 §2 (the oracle: "removed, then a new element inserted") and §4.9 (incarnations), LLP 1072 (rows built on the owner thread), `queue/` "Android steady-state CPU, what is left" (6).

## Summary

On Android the remaining CPU gap to Views during a fling is per-row
construction: every row that scrolls in runs the runner's realize, the
kernel's apply of every create and style op, a layout of the new subtree with
text measured from nothing, a second layout when its symbols' natural sizes
arrive, and motion's registration of every new node. Views rebinds a recycled
`ViewHolder`. This RFC rebinds the row instead: when a window pass retires a
row and needs another, the retiring row is moved to the new key and position
and its bindings are evaluated against the new item; only the props and
styles whose values differ are written. Its kernel nodes, Taffy nodes, layout
caches, text and pictures of an unchanged source stay.

A rebound row must be indistinguishable from a freshly mounted one (the web's
oracle, LLP 1068 §2). Three mechanisms make it so:

1. **The runner restates the row whole.** A rebind is a full update of the
   row (`Update::full` for its subtree) against a frame with the new item and
   index, its own slots initialized again from their `init`, so every binding
   ends equal to what a create would emit (emitted only where it differs).
   The wrapper takes the new `listItemKey`, a new measurement token and epoch,
   and publishes its place again. `when`/`match` arms and nested `each` rows
   follow the new item as any update makes them.
2. **The receipt names the row's views `renewed`.** Every view of a rebound
   row (the wrapper's included) is listed in `CommitReceipt::renewed`.
   `Kernel::motion_sync` puts each in `removed` and then restates it as a new
   node (its `transition` row, targets and `animation` row): no transition
   runs from the old item's values and animations start from their beginning,
   as on a new element. Paint motion and layout motion forget it the same way.
3. **A host that opts in resets what it keeps by view.** The Canvas/Linux
   host drops a renewed view's presented values and press feedback, scrolls a
   renewed scroller back to its start, and drops a picture whose source the
   node no longer names (with its natural size: a new image has none until
   it loads); a picture of the same source stays, as a new image would find
   it decoded.

What a rebind cannot make fresh refuses it (§3): a row holding an editor, a
form control, a native, canvas, web or video view, an inner virtualized list,
`autofocus`, a bound scroll offset, a drag handle, an exit animation, a layout
transition or a timeline is destroyed and built as before; so is every row of
a list whose body has slots below the row's own.

## 1. Where the time went

Simpleperf of the easy list's fling on the Pixel 10a (base, all speeds, the
`exact` thread, 28,511 samples): the collection pass was 60% of the thread,
painting 21%, idle 13%. Inside the pass: the host's commit effects 21%
(layout 13.5%, motion sync 2.8%), the runner's feedback 11% (realize 5.2%,
dropping retired rows 0.9%), the kernel's apply 11.8% (node creation, style
application and `style_changed` 8.3%, detaching destroyed subtrees 1.1%),
the image sync 14.7%, of which 6.5% was `set_intrinsics`' second layout once
each new row's symbols reported their sizes. A rebind avoids node creation
and destruction, most style application, the symbol re-report and its layout,
most of motion's registration, and the layout of subtrees whose inputs did
not change; it still evaluates the row's bindings, lays out what changed,
measures changed text, and records the row's drawing again.

## 2. Design

### 2.1 The window pass

`realize_window` used to build each needed row inside its walk of the window
and destroy the rows past it afterwards. With reuse on, the walk only notes
the rows it needs; after the retirement rules (unchanged: all rows past the
window, or within a limited report's cap and the two-viewport rule) have
chosen the rows that go, each needed row takes a retiring row that admits a
rebind, preferring one whose item steers the same way (per field: an option
present, a flag set, a list non-empty; so a photo row is rebound to a photo
row in a feed of kinds). A limited report (a slice, LLP 1072) keeps rows past
the window up to its cap instead of retiring them, so it often needs rows
and retires none: a needed row then takes the farthest such kept row, which
would have been destroyed by a later slice anyway. Retiring rows left over
are destroyed as before. There is no pool across passes: a parked row would
be detached (its inherited rows recomputed as an orphan, then again on
return) or kept hidden in the list, and the profile left too little for it
to win (§5). A
row whose item left the data is never rebound: it leaves with
`listItemKey` emptied, for its exit (LLP 1063). Build-only reports (LLP 1072
§5) retire nothing, so they never rebind. Pinned rows (focus, interaction, a
reorder's source) never retire, so they are never rebound.

### 2.2 Identity

The view ids, kernel keys and wrapper of a rebound row are kept: a rebind is
an update, and the runner still never reuses an id for another node. What is
new is said by the receipt (`renewed`), the wrapper's key and epoch, and its
position. Consequences:

- Kept row recordings (`paint/rows.rs`) are keyed by the wrapper's node; a
  rebind touches nodes in the row, so its drawing is recorded again into the
  same reader node, never shown stale.
- A measurement the host made for the old item carries the old epoch and is
  ignored.
- An agent's or host's held `NodeKey` for a node of a rebound row now
  resolves to the same node showing another item, where a destroyed node's
  key would fail closed. Events address views a frame at a time and a
  touched row is pinned, so no event reaches a rebound row meant for its old
  item; a stale reference held across commits is the declared difference.

### 2.3 Refusals

By site, once per list (`Reuse::new` walks the row body): node types
`List` (an inner list also by instance), `TextInput`, `Control`,
`NativeView`, `Canvas` (and any `surface`), `WebView`, `Video`, `Head`; props
`autofocus`, `scrollTop`, `scrollLeft`, `heightDragFor`, `transformDragFor`,
`backgroundMaterial`, `glassGroup`, `popover`, `retainFocus`, `refreshing`;
style rows `exit-animation`, `layout-transition`, `drag-timeline`,
`animation-timeline`, `animation-range`, `timeline-scope`. A row is refused if
any node it holds now is of a refused site (an arm the new item activates is
built fresh by the update, so only the kept nodes matter). A body with slots
owned by a region inside the row refuses the list: those rows' state would
outlive their item.

### 2.4 Who opts in

`Runner::set_row_reuse` (off by default). The Canvas host turns it on
(`EXACT_ROW_REUSE=0` turns it off); the Linux host turns it on only with
`EXACT_ROW_REUSE=1`. The Apple host keeps its own view pool (LLP 1050.000 §7)
and the web its fresh elements; neither resets per-view state on `renewed`
yet, so neither opts in.

## 3. Tests

`contract/cli/tests/it/collection_reuse.rs`: a rebound row's subtree (types,
styles, props, children, recursively) equals a freshly built one's at every
offset of a scroll through 400 rows with fields, arms, a nested `each`, a row
slot seeded from the item, a transition, an animation, a picture and a
scroller, and in limited reports for every row both runners mount; every view of a rebound row is renewed and none created, and
motion restates each one; a measurement for the old item does not reach the
new one; a focused row keeps its item; an editor, a layout transition or
slots below the row refuse a rebind; reuse is off by default.

## 4. Not done

- The Apple and web hosts could opt in once they reset by `renewed` (iOS's
  pool would then have nothing to do for these rows).
- Rows refused for an inner list (LLP 1070) could rebind around it.
- A rebind still records the whole row again; partial redraw is LLP 1072's
  and the damage lane's.

## 5. Measurements

Pixel 10a (`5B281JEA315841`), 2026-10-05, the fling scenario, one build run
with `EXACT_ROW_REUSE=0` (off: the code path of the base) and with reuse,
interleaved with Views in the same session; easy and crypto three runs of
each, heavy and xheavy two; medians over runs and both directions. Each cell
is fps / CPU ms/s / the `exact` thread's ms/s / Mcycles/s.

| app | speed | off | reuse | Views |
|---|---|---|---|---|
| easy | 6k | 119 / 1018 / 304 / 860 | 119 / 740 / 223 / 642 | 119 / 550 / – / 610 |
| easy | 12k | 119 / 846 / 335 / 724 | 119 / 801 / 278 / 678 | 119 / 744 / – / 838 |
| easy | 24k | 119 / 1211 / 472 / 1048 | 119 / 828 / 342 / 702 | 119 / 848 / – / 972 |
| crypto | 6k | 120 / 1260 / 320 / 1254 | 120 / 1037 / 269 / 1070 | 120 / 737 / – / 828 |
| crypto | 12k | 120 / 1322 / 424 / 1384 | 120 / 1231 / 391 / 1184 | 120 / 772 / – / 832 |
| crypto | 24k | 119 / 1307 / 568 / 1556 | 120 / 1290 / 521 / 1442 | 120 / 966 / – / 1002 |
| heavy | 12k | 120 / 1014 / 318 / 990 | 119 / 1099 / 322 / 1107 | 118 / 633 / – / 873 |
| heavy | 24k | 119 / 1444 / 454 / 1744 | 119 / 1420 / 427 / 1711 | 119 / 810 / – / 1220 |
| xheavy | 12k | 115 / 2759 / 482 / 5660 | 116 / 2770 / 466 / 5812 | 112 / 1593 / – / 3132 |
| xheavy | 24k | 110 / 4191 / 566 / 9870 | 111 / 4357 / 560 / 10296 | 87 / 2461 / – / 5134 |

At 1k and 3k the `exact` thread is unchanged (few rows enter). Whole-process
ms/s varies by ±30% between runs of one variant (the reader's thread and
RenderThread move between a low and a high mode in both variants); the
`exact` thread is the stable signal. Fling fps is unchanged everywhere.

- **easy:** the `exact` thread −17 to −28% at 6k–24k; at 24k the process
  uses less CPU than Views (828 vs 848 ms/s) and fewer cycles at 12k and
  24k. A profile (12k–24k) shows the kernel's apply down from 11.8 to 6.5% of
  the thread and the symbols' second layout from 6.5 to 3.2%; layout of the
  rebound rows' changed text and painting are what is left.
- **crypto:** −8 to −16%. A rebind evaluates every binding again (the
  chart's `points` string from 48 prices is most of it) and restarts the
  chart's animations, as a fresh row does; layout is 21% of the thread.
- **heavy, xheavy:** within noise (heavy's thread −6% at 24k). Their rows'
  cost is their content: paragraphs shaped and laid out anew, pictures
  decoded, recorded again; and in xheavy half the kinds hold a refused leaf
  (canvas, map, video, web view, an inner list).

Rendering: mid-fling screencaps of easy satisfy the data's invariants on
every visible row (body by index mod 4, the attachment on every fifth, the
icon by parity); the rest frames after scripted swipes are pixel-identical
with reuse on and off; crypto's charts draw in on rows entering mid-fling as
without reuse; heavy's pictures show the placeholder until their own picture
arrives, never another row's. The empty list area some crypto frames show at
24k appears as often without reuse (it is the fill falling behind).
