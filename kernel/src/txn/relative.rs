//! The end of every commit resolves `rem` and `em` (LLP 1069.000 D3): a
//! root font size the host set since the last commit reaches the nodes that
//! inherit it, and every row written in `rem`/`em` is given the pixels it now
//! means — parents before children, so an `em` font size builds on its
//! parent's resolved one, as CSS computes it.
use super::*;
use crate::generated::StyleProps;
use crate::style::relative::{Unit, MEDIUM};
use crate::StyleId;

pub(super) fn resolve(
    arena: &mut NodeArena,
    layout: &mut dyn LayoutMirror,
    touched: &mut Vec<NodeKey>,
    receipt: &mut CommitReceipt,
) {
    if let Some(px) = arena.root_font_size_next.take() {
        if px != arena.document_style.font_size {
            arena.document_style.font_size = px;
            if px == MEDIUM {
                arena.document_style.mask = arena
                    .document_style
                    .mask
                    .minus(StyleMask::of(StyleId::FontSize));
            } else {
                arena.document_style.mask.set(StyleId::FontSize);
            }
            let rows = StyleMask::of(StyleId::FontSize);
            let slots: Vec<u32> = arena.iter_live().collect();
            for slot in slots {
                if arena.inherited_source(slot, StyleId::FontSize).is_none() {
                    inherited_changed(arena, layout, slot, rows, receipt);
                    touched.push(arena.key(slot));
                }
            }
        }
    }
    if arena.relative_slots.is_empty() {
        return;
    }
    let depth = |arena: &NodeArena, mut slot: u32| {
        let mut depth = 0u32;
        while let Some(parent) = arena.parent(slot) {
            depth += 1;
            slot = parent;
        }
        depth
    };
    let mut slots: Vec<(u32, u32)> = arena
        .relative_slots
        .iter()
        .map(|slot| (depth(arena, slot), slot))
        .collect();
    slots.sort_unstable();
    let root = arena.document_style.font_size;
    for (_, slot) in slots {
        let font_of = |slot: u32| {
            arena
                .inherited_source(slot, StyleId::FontSize)
                .map_or(root, |s| arena.style(s).font_size)
        };
        let parent = arena.parent(slot).map_or(root, font_of);
        let mut next = StyleProps::clone(arena.style(slot));
        let mut changed = StyleMask::EMPTY;
        let relative: Vec<_> = next.relative.iter().collect();
        // `font-size` first: an `em` elsewhere on the node is its font size.
        let mut font = parent;
        for (id, unit, n) in relative.iter().copied() {
            if id != StyleId::FontSize {
                continue;
            }
            let px = n * if unit == Unit::Rem { root } else { parent };
            if next.set_resolved(id, px) {
                changed.set(id);
            }
        }
        if next.mask.has(StyleId::FontSize) {
            font = next.font_size;
        }
        for (id, unit, n) in relative {
            if id == StyleId::FontSize {
                continue;
            }
            let px = n * if unit == Unit::Rem { root } else { font };
            if next.set_resolved(id, px) {
                changed.set(id);
            }
        }
        if changed.is_empty() {
            continue;
        }
        let excluded = crate::flow::is_exclusion(arena, slot);
        arena.set_style(slot, next);
        arena.update_exclusion_count(slot, excluded);
        style_changed(arena, layout, slot, changed, receipt);
        touched.push(arena.key(slot));
        propagate_inherited(arena, layout, slot, changed, touched, receipt);
    }
}
