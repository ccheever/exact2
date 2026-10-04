# LLP 1083: `position: sticky`

**Type:** RFC
**Status:** Implemented 2026-10-03 on `lane/sticky`. Not reviewed. §6 is Charlie's.
**Systems:** Kernel (`PositionType::Sticky` in `schema.json`; `style.rs`'s Taffy mapping; `kernel/src/kernel/sticky.rs`: `StickyConstraint`, `Kernel::sticky_constraint`, `Kernel::sticky_nodes`; the arena's `sticky_slots`), Contract (the literal checks in `tags.rs`, `class.rs` and `collection.rs`), Apple host (`host/apple/src/layout.rs` `emit_sticky`, the `sticky` batch op, `Sticky.swift`, `applyTransform` on both platforms, `usedZIndex`), Linux host (`paint.rs`: the sticky offset and siblings stacked by used `z-index`), web hosts (none: the browser does it)
**Author:** Claude (Opus 5.5) for Charlie Cheever, from James's request (2026-10-03)
**Date:** 2026-10-03
**Related:** LLP 1001 §"Additional deviations as built" (which said sticky was not a row; it now points here); LLP 1074 §7 (which deferred sticky as "scroll-time placement on each host"); LLP 1010 (scroll position is host state); LLP 1061 D2/D3 (`applyTransform`, the one writer of a view's transform); CSS Positioned Layout 3 §3.4 (sticky positioning); Chrome's `StickyPositionScrollingConstraints::ComputeStickyOffset`.

## Summary

`position: sticky` is a `position` value on every host. On the web the browser does all of it. On the native hosts, the kernel lays the box out where an unscrolled page would put it and gives the host the box's **constraint**. Each time the box's scroller scrolls, the host moves the box by the constraint's offset, as Chrome computes it. The offset matches literal Chrome at every scroll offset across nine cases (`kernel/tests/it/browser_sticky.rs`).

```
scroll height=300
  column                                           // a section
    column position="sticky" top=0 z-index=1 …     // its header
    …rows
  row position="sticky" bottom=0 z-index=1 …       // a footer
```

## Decisions

**D1. A value of `position`, laid out as `relative` with no offset.** `sticky` is the fourth `PositionType`, appended so the wire's ordinals stand. Taffy has no sticky, so the kernel gives Taffy `Position::Relative` with every inset `auto`. The box sits in flow where a static box would, and its insets are thresholds, not offsets. Like any non-`static` box it is positioned: it contains its absolutely positioned descendants, its `z-index` applies, and the compiler's literal checks count it as positioned (`always_positioned`; `may_hold_absolute` does not count it as absolute). Text flow treats it as in flow (`flow.rs`).

**D2. The constraint is the kernel's, the offset Chrome's.** `Kernel::sticky_constraint(key)` returns the following, as rectangles in the scroller's border-box space at offset zero:
- **The scroller.** The nearest ancestor whose `overflow` is not `visible` on either axis, else the root, which the page scrolls.
- **The box's natural border box.**
- **The limit.** The parent's content box, less the box's margins.
- **The port.** The scroller's content box. Chrome keeps a sticky box inside its scroller's padding, not its padding box; the first fixture run showed this, off by exactly the padding.
- **The four insets.** Percentages are of the port.

`StickyConstraint::offset(scroll)` is Chrome's rule on each axis. The end inset pulls the box back into the port, never before the limit's start. Then the start inset pushes it, never past the limit's end, so the start wins where both cannot hold. `Sticky.swift` mirrors it line for line. The fixture was measured in Chrome 154 (`scrollbar-width: none`, the method of `browser_cases.rs`), and its cases are:
- pinned headers that push each other out;
- an inset with margins;
- a bottom footer;
- a percentage inset;
- both insets;
- a box taller than the port;
- a horizontal `left`;
- a box nested in a padded, bordered wrapper;
- a flex column.

**D3. Apple hears it as one batch op.** After each layout, the Apple host re-reads every sticky node's constraint, since a parent or a scroller can move without the box moving. `Kernel::sticky_nodes` is a maintained slot set, so this costs nothing when there are none. The host sends `{"op":"sticky","id":…,"scroller":…,"natural":[…],"limit":[…],"port":[…],"insets":[…]}` when a constraint changed, and `{"op":"sticky","id":…}` when the node stopped being sticky. The op follows the frames in the same batch.

**D4. Each native host moves the box at scroll time.**
- **iOS.** The offset folds into `applyTransform`'s outer translation, beside a layout transition's offset. UIKit hit-tests through the transform, and a frame op keeps the offset (`applyGeometry` re-applies it).
- **macOS.** The offset moves the frame, as a lifted Arrange row's translation already does. AppKit paints, culls and hits by frame (`arrangeShift` is now that row's lift plus the sticky offset).
- **Both Apple platforms.** `StickyHost` keeps the constraints by scroller. A node's `scrollViewDidScroll` (macOS: its clip view's bounds) re-places that scroller's boxes. The page viewport's scroll re-places the boxes whose scroller is a root that does not scroll itself. A sticky flat leaf is promoted to a view.
- **Linux.** The painter moves the box when it paints it, from the same constraint at the scroller's offset (the page's for the root). The boxes it records for hit-testing follow.

**D5. Not a windowed row's root.** A virtualized list lays each row in a wrapper of its own, so a row root would stick only inside its own row. The compiler refuses it with that reason and the pattern that works: a section is one row, and its header is `position: sticky` inside it.

**D6. Paint order stays tree order; `z-index` lifts a sticky box.** The kernel paints in tree order (LLP 1001). The web host keeps that order by isolating a static box that follows a positioned one. So, as on every Exact host, the content that scrolls under a sticky header paints over it unless the header has a `z-index`. A plain page would paint the header on top, because CSS paints positioned boxes after static ones. This is the existing declared deviation, not a new one. It shows more often with sticky, because a header is meant to be overlapped. The Linux painter did not read `z-index` at all. It now stacks siblings by their used `z-index`, by the rule of Apple's `usedZIndex` (a positioned box's, or a flex or grid item's, with tree order among equals), so `z-index=1` lifts a header there too.

## What is not done

- **Not handled:**
  - a sticky box inside another sticky box (Chrome composes them);
  - a sticky box under a transformed ancestor between it and its scroller;
  - a scroller inside a sticky box.
- **The agent and `frame(id)`.** Both report the box where layout put it, free of scrolling, as every frame is (LLP 1010). A tap goes through the host's hit path, which sees the moved box.
- **`position: fixed`** is still not a row (LLP 1074 §7).

## 6. For Charlie

1. **D6.**
   - **Option (a), the recommendation, as built.** Keep tree order, and expect a sticky header to say `z-index`.
   - **Option (b).** Paint a sticky box after its in-flow siblings, as CSS does without one. The web host would then not isolate the static siblings after a sticky box. Apple would raise its layer above them. Linux would order it so. That is CSS's order among siblings for sticky alone, which is not CSS's order in general.
