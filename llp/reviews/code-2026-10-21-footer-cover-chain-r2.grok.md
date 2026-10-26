# Code review: an absolute footer over a route's bottom-edge scroller rests on the tab bar (kernel cover chain), round 2 of 3, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `201cb1b7f6`.
- **Method:** one brief (sha256 `85cb1af022b13fe692d41543717f224def904fe6dcbc53de668d53eff388dac3`), blind to the other reviewer; 3 rounds (kernel, theme2's plan approval) then a narrow check. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** changes (1 BLOCKER, 1 SHOULD-FIX, 0 NIT mentions).
- **Disposition:** taken (the round 2 commit).

**BLOCKER** — Gaining or losing `Edges` does not re-derive the covered box above. `kernel/src/kernel/cover.rs:98`

`scroller_insets_bottom` stops at the first descendant `Edges` (`cover.rs:291`), so a sheet's bottom padding depends on whether the route inside it is covered. `set_host_cover` restyles that ancestor only when an `Inset` bottom crosses 0 or `Whole` toggles (`cover.rs:98-100`). An `Edges` change restyles the node itself and its absolute children, not the ancestor.

`set_covers` applies the batch in dictionary order, then lays out once. Two of the six orders stick:

1. Sheet `Edges`, then the scroller's `Inset`, then the route's `Edges`. The inset's nearest covered box is the sheet (the route is not covered yet), so the sheet's padding is zeroed. The route's `Edges` never restyles the sheet.
2. `Inset` first, then the sheet, then the route. Same result: the sheet is styled while the route has no `Edges`, and nothing styles it again.

The sheet's in-flow content stays 34 short of its own bar for the rest of the screen. `the_covers_in_any_order` only runs inset, then route, then sheet, which is one of the safe orders. Re-setting the same `Edges` does not heal it (`before == cover` returns early).

Treat an `Edges` appearance or disappearance like the other chain changes, and restyle `covering(slot)` (the parent walk already starts above this node, so Q2 still stops at the nearest ancestor):

```rust
let edges = |c: Option<HostCover>| matches!(c, Some(HostCover::Edges(_)));
let chain = (bottom(before) != bottom(cover)
    || whole(before) != whole(cover)
    || edges(before) != edges(cover))
    .then(|| covering(&self.arena, slot))
    .flatten();
```

Add the two orders above to `the_covers_in_any_order`, including sheet-then-inset-then-route.

**SHOULD-FIX** — The recorded cover is the full `Edges` bottom after `apply` has zeroed it. `kernel/src/id.rs:192`

`apply` sets that bottom to 0 when the chain takes it (`cover.rs:222-224`). `BoxInset::of` still stores `added_padding`, which does not. For a route whose nested scroller takes an 83-point bar and whose own padding-bottom is 0, the engine padding is 0 and `layout_padding` reports 83. With an authored 16 it reports 83 and `resolved_padding` reports 0. `content_size` (`host/apple/src/layout.rs:588`) then adds that phantom end padding for any clipping or scrolling route. `cover_bottom` is right for the fit-content subtraction; the recorder was not updated.

Record the cover `apply` actually added: run the same zeroing, and subtract that from the engine padding.

```rust
let [top, right, mut bottom, left] = crate::kernel::cover_padding(arena, slot);
if matches!(arena.cover(slot), Some(HostCover::Edges(_))) && scroller_insets_bottom(arena, slot) {
    bottom = 0.0;
}
```

`cover_bottom` matches `apply`'s padding branch (raw `Edges` bottom, or 0 when the chain takes it), not the absolute-footer shift, which correctly keeps the full cover. A fit-content sheet with a top bar agrees: the trial keeps the top (including `header_inset`) and drops the bottom when the scroller takes it, and `cover_bottom` subtracts only that bottom, so the published extent still contains the top bar.

Reparent, attach-after-inset, and covers set while `layout` is `None` all re-derive. A children change restyles both parents; an inset set before attach is picked up by `children_changed`; an on-demand rebuild styles from the arena after every cover is stored. The extra work is one ancestor restyle per children change outside a scroller, plus a last-child walk as deep as the chain. Cover changes are rare. That cost is fine.
