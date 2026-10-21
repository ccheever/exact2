# Code review: an absolute footer over a route's bottom-edge scroller rests on the tab bar (kernel cover chain), round 3 of 3, 2026-10-21 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `3016f733c7`.
- **Method:** one brief (sha256 `110832621ea1b7739a418e7e6ea7594e695d50c85d07e714784fcbaf0727be7b`), blind to the other reviewer; 3 rounds (kernel, theme2's plan approval) then a narrow check. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** changes (0 BLOCKER, 1 SHOULD-FIX, 0 NIT mentions).
- **Disposition:** taken (the round 3 commit).

## SHOULD-FIX

An absolute child's length `top` stays stale when `header_inset` changes. `apply` shifts that inset by `added_padding`, and the top component is the Edges top plus `header_inset` (`cover.rs:246-259`, `cover.rs:383-387`, `cover.rs:456-479`). The bottom component is the raw Edges bottom, and that path is restyled. The Edges box itself is restyled when the inset changes, so in-flow content moves. Its absolute children are not.

Three inputs of `header_inset` change without that restyle:

- The first child becomes or stops being `Whole`. `set_host_cover` restyles the parent (`cover.rs:104-106`) and collects absolute children only for the box whose cover changed (`cover.rs:116-126`). iOS builds one batch from a dictionary (`NavigationCoversIOS.swift:147`) and applies each cover before layout (`host/apple/src/covers.rs:43-47`). Edges-then-Whole is the order `a_replaced_header_that_cleared_the_status_bar_hands_that_inset_to_the_bar` already uses: the route's padding grows by `env().inset(Top)`, and a `top: 0` sibling keeps the shift from before the header was replaced.
- That `Whole` child's `padding-top` changes. `style_changed` restyles the parent only (`cover.rs:347-351`).
- `env().inset(Top)` changes while the Edges tuple stays equal, so `set_host_cover` returns at `cover.rs:82-84`. `replace_env` restyles covered boxes and env users (`environment.rs:157-176`), which includes the route and the replaced header, and skips an absolute child whose insets are plain lengths. A rotation that keeps the bar height and changes the status-bar inset leaves `top: 0` short or long by that delta for the rest of the session.

Fix: one helper that restyles an Edges box's absolute children (the loop at `cover.rs:432-437`; `set_style` already drops an unchanged style). Call it for the Whole toggle's parent in `set_host_cover`, for that parent in `style_changed`, and for each Edges box `replace_env` restyles. `children_changed` already covers a first-child swap.

## Round-2 fixes

All four hold, and the unmirrored nit is closed.

1. Edges appearing or disappearing re-derives the covered box above: `edges(before) != edges(cover)` feeds `covering` (`cover.rs:98-103`). A value change on an Edges box that stays Edges restyles that box and its absolute children. `the_covers_in_any_order_leave_the_sheet_its_padding` covers the presence flip.
2. A footer attached or moved after the cover settled is restyled against its new parent (`cover.rs:428-438`), after `SetChildren` has updated the parent pointer. `a_footer_attached_late_a_clipping_box_and_the_reported_padding` covers late attach and the move onto the uncovered header.
3. `scrolls` matches the engine's own-style formula, including `visible` beside `hidden` (`cover.rs:299-311`, `style.rs:1116-1125` and `1288-1296`). `OverflowX` joins the chain rows (`cover.rs:340-345`).
4. `cover_padding` is `applied_padding` (`kernel.rs:43`). `BoxInset::of` and `layout_padding` report the bottom layout actually padded (`id.rs:192-198`, `geometry.rs:416-419`). `cover_bottom` reads the same value (`cover.rs:53-56`).

The children walk and the absolute restyle run only when `tree_ref()` is `Some` (`cover.rs:428`). On an unmirrored kernel, `covering` still walks, then `taffy()` is `None` and the restyle does not run.

Q1 holds: a marked scroller off the last in-flow spine, or past a scroller, leaves the route's padding (`cover.rs:272-296`, `a_marked_scroller_off_the_chain_leaves_the_route_its_padding`). Q2 holds: an Inset bottom crossing zero re-derives only the nearest Edges ancestor, and a scroller in between yields none (`cover.rs:317-329`, `an_inset_three_boxes_down_restyles_its_route_and_nothing_above`).

`scroller_insets_bottom` and the bottom shift have no remaining stale input. Children, display, position, overflow, Whole, Inset bottom crossing zero, and Edges presence all re-derive the covered box. The bottom shift follows Edges presence and magnitude, attach, and move.

Linux sets Edges and Whole only, so it never zeroes padding; its absolute footer is shifted and tested. Web, terminal, and Mac never set Inset (Mac reports Whole only). Android has no runtime cover path. Hosts that do not set Inset keep the padding they had before the walk.

Cost is a depth walk that returns at the first scroller, plus a style rebuild per absolute child that `set_style` drops when the derived style is unchanged (`layout.rs:550-552`). `children_changed` also dirties the nearest Edges ancestor when the edit is off the last-child spine. That is extra layout on a rare edit, and the scroller path skips it.

The plan file `/Users/admin/x2book/reload1-footer-plan.md` is not on disk. This pass is against the stated Q1 and Q2 conditions and the kernel's own cover contracts.
