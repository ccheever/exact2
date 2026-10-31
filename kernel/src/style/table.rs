//! What HTML's UA sheet gives a table's and a description list's elements,
//! as layout, where the kernel lays them out (LLP 1153 D7.3): a native host.
//! The web's browser has the sheet itself, and the plan carries none of it.
//!
//! **The declared deviation (LLP 1153 D7.3).** A table here is a grid, not
//! CSS 2.1 §17.5's table. `layout/table.rs` places its cells where HTML's
//! table-forming algorithm puts them (HTML §4.9.12) and makes each column an
//! `auto` grid track; the table is as wide as its columns, as an auto-width
//! table is. What differs from §17.5.2.2's automatic table layout is how
//! width is shared among columns: a grid shares it equally among its `auto`
//! tracks, where the browser shares it in proportion to the columns' widths.
//! So a spanning cell wider than the columns it spans, over columns of
//! unequal widths, and a table narrower than its content, whose columns
//! fall between their least and their widest, have columns of other widths
//! than the browser's: on 2026-10-30, 55 and 35 px where Chrome's were 55.70
//! and 34.30, and 57 and 57 where Chrome's were 50 and 64 (conformance
//! fixture `table-widths`). Columns that fit their content, spans over
//! columns of equal widths, row spans, captions and rows are the browser's
//! to 0.5 px (fixture `table`).

use crate::arena::NodeArena;
use crate::generated::{Direction, NodeType, PropId, StyleId, VerticalAlign};
use crate::style::Dimension;
use taffy::prelude::{auto, length, zero, Rect};

/// HTML's UA `border-spacing: 2px`: between neighbouring cells, and between
/// the outer cells and the table's padding edge (CSS 2.1 §17.6.1).
pub(crate) const SPACING: f32 = 2.0;

/// HTML's UA padding of a `th` and a `td`, on each side where none is said.
const CELL_PADDING: f32 = 1.0;

/// HTML's UA `margin-inline-start` of a `dd`.
const DD_INDENT: f32 = 40.0;

/// What a node is to its table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Table,
    Caption,
    /// `thead`, `tbody` or `tfoot`.
    Group,
    Row,
    /// `th` or `td`.
    Cell,
}

impl Part {
    pub(crate) fn of(arena: &NodeArena, slot: u32) -> Option<Part> {
        if arena.node_type(slot) != NodeType::View {
            return None;
        }
        Some(match arena.props(slot).str(PropId::SemanticTag)? {
            "table" => Part::Table,
            "caption" => Part::Caption,
            "thead" | "tbody" | "tfoot" => Part::Group,
            "tr" => Part::Row,
            "th" | "td" => Part::Cell,
            _ => return None,
        })
    }
}

/// Whether [`apply`] gives `slot` anything: it is a table's part or a
/// description list or its description.
pub(crate) fn styled(arena: &NodeArena, slot: u32) -> bool {
    Part::of(arena, slot).is_some()
        || (arena.node_type(slot) == NodeType::View
            && matches!(
                arena.props(slot).str(PropId::SemanticTag),
                Some("dl" | "dd")
            ))
}

/// Whether `table` lays out as a table: CSS's `display: table`, which a
/// node whose author said no `display` is. One that says `block`, `flex`
/// or `grid` is that box, as the browser makes it.
pub(crate) fn lays_out(arena: &NodeArena, table: u32) -> bool {
    !arena.style(table).mask.has(StyleId::Display)
}

/// The table that lays `slot` out among its grid items, when `slot` is a
/// part standing where HTML's content model puts it: a caption or row group
/// in the table, a row in it or in its row group, a cell in such a row.
pub(crate) fn table_of(arena: &NodeArena, slot: u32) -> Option<u32> {
    let parent = arena.parent(slot)?;
    let table = match (Part::of(arena, slot)?, Part::of(arena, parent)?) {
        (Part::Caption | Part::Group | Part::Row, Part::Table) => parent,
        (Part::Row, Part::Group) => {
            let table = arena.parent(parent)?;
            (Part::of(arena, table) == Some(Part::Table)).then_some(table)?
        }
        (Part::Cell, Part::Row) => return table_of(arena, parent),
        _ => return None,
    };
    lays_out(arena, table).then_some(table)
}

/// The UA sheet's layout for `slot`, over its authored rows in `s`; the
/// grid placement is `layout/table.rs`'s, which only the table's tree knows.
pub(crate) fn apply(arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
    let style = arena.style(slot);
    match arena.props(slot).str(PropId::SemanticTag) {
        // `margin-block: 1em`.
        Some("dl") if arena.node_type(slot) == NodeType::View => {
            let em = arena.computed_source(slot, StyleId::FontSize).font_size;
            if !style.mask.has(StyleId::MarginTop) {
                s.margin.top = length(em);
            }
            if !style.mask.has(StyleId::MarginBottom) {
                s.margin.bottom = length(em);
            }
            return;
        }
        // `margin-inline-start`, where its author said none. With neither
        // side said it is the left, which right to left mirrors
        // (`sides::mirror_unsaid`); with one said, the start side itself.
        Some("dd") if arena.node_type(slot) == NodeType::View => {
            let (left, right) = (
                style.mask.has(StyleId::MarginLeft),
                style.mask.has(StyleId::MarginRight),
            );
            let rtl = arena.computed_source(slot, StyleId::Direction).direction == Direction::Rtl;
            match (left, right, rtl) {
                (false, false, _) | (false, true, false) => s.margin.left = length(DD_INDENT),
                (true, false, true) => s.margin.right = length(DD_INDENT),
                _ => {}
            }
            return;
        }
        _ => {}
    }
    let Some(part) = Part::of(arena, slot) else {
        return;
    };
    let table = match part {
        Part::Table => lays_out(arena, slot).then_some(slot),
        _ => table_of(arena, slot),
    };
    let Some(table) = table else {
        return;
    };
    let env = arena.env();
    match part {
        Part::Table => {
            s.display = taffy::style::Display::Grid;
            // Shrink-to-fit in block flow, as an auto-width table is.
            s.item_is_table = true;
            // HTML's UA sheet: `box-sizing: border-box`, so a width holds
            // the spacing at the table's edges.
            if !style.mask.has(StyleId::BoxSizing) {
                s.box_sizing = taffy::style::BoxSizing::BorderBox;
            }
            s.padding = Rect {
                left: style.padding_left.floor_lp(env, SPACING),
                right: style.padding_right.floor_lp(env, SPACING),
                top: style.padding_top.floor_lp(env, SPACING),
                bottom: style.padding_bottom.floor_lp(env, SPACING),
            };
            s.gap = taffy::geometry::Size {
                width: length(SPACING),
                height: length(SPACING),
            };
        }
        Part::Caption => {
            // Above the rows, from the table's padding box: outside its
            // spacing and padding, as CSS's caption is outside the table box,
            // but inside a border the table has, where CSS's is outside it
            // too. An authored margin is from that box's edge, as CSS's is
            // from the table's.
            let t = arena.style(table);
            let inset = |d: Dimension| match d.resolve(env) {
                Dimension::Points(v) => v.max(0.0) + SPACING,
                _ => SPACING,
            };
            let (left, right) = (inset(t.padding_left), inset(t.padding_right));
            // Writes a side's engine margin, from that box's edge, and gives
            // the points its author said there (0 where none).
            let from = |side: StyleId,
                        said: Dimension,
                        at: f32,
                        m: &mut taffy::style::LengthPercentageAuto| {
                match said.resolve(env) {
                    Dimension::Points(v) if style.mask.has(side) => {
                        *m = length(v - at);
                        v
                    }
                    _ if style.mask.has(side) => 0.0,
                    _ => {
                        *m = length(-at);
                        0.0
                    }
                }
            };
            let start = from(
                StyleId::MarginLeft,
                style.margin_left,
                left,
                &mut s.margin.left,
            );
            let end = from(
                StyleId::MarginRight,
                style.margin_right,
                right,
                &mut s.margin.right,
            );
            let top = inset(t.padding_top);
            from(StyleId::MarginTop, style.margin_top, top, &mut s.margin.top);
            // Unsized, it is as wide as that box less its margins and asks the
            // columns for none of its width, so a long caption wraps where
            // CSS's does.
            let sized = [StyleId::Width, StyleId::MinWidth, StyleId::MaxWidth];
            if !sized.iter().any(|id| style.mask.has(*id)) {
                let wider = left + right - start - end;
                s.size.width = length(0.0);
                s.min_size.width = Dimension::Calc(100.0, wider).floor_lpa(env);
            }
        }
        Part::Group | Part::Row => {
            // No padding, margin or border (CSS 2.1 §17.5.3, §17.6.1), no
            // overflow of its own; a height is the row's least.
            s.padding = zero();
            s.margin = zero();
            s.border = zero();
            s.overflow.x = taffy::style::Overflow::Visible;
            s.overflow.y = taffy::style::Overflow::Visible;
            least(style, env, s);
        }
        Part::Cell => {
            s.margin = zero();
            for (id, side) in [
                (StyleId::PaddingLeft, &mut s.padding.left),
                (StyleId::PaddingRight, &mut s.padding.right),
                (StyleId::PaddingTop, &mut s.padding.top),
                (StyleId::PaddingBottom, &mut s.padding.bottom),
            ] {
                if !style.mask.has(id) {
                    *side = length(CELL_PADDING);
                }
            }
            least(style, env, s);
            // `vertical-align: middle`, the UA's for a cell.
            if s.display == taffy::style::Display::Block {
                use taffy::style::AlignContent;
                let said = style.mask.has(StyleId::VerticalAlign);
                s.align_content = Some(match style.rare.vertical_align {
                    VerticalAlign::Baseline | VerticalAlign::TextTop if said => AlignContent::START,
                    VerticalAlign::TextBottom if said => AlignContent::END,
                    _ => AlignContent::SAFE_CENTER,
                });
            }
        }
    }
}

/// A row's or a cell's width and height are its least (CSS 2.1 §17.5.2.2,
/// §17.5.3): the box fills its grid area, as a cell fills its slot.
fn least(style: &crate::StyleProps, env: &crate::style::Env, s: &mut taffy::style::Style) {
    if style.width != Dimension::Auto {
        s.min_size.width = style.width.floor_lpa(env);
        s.size.width = auto();
    }
    if style.height != Dimension::Auto {
        s.min_size.height = style.height.floor_lpa(env);
        s.size.height = auto();
    }
    s.align_self = None;
    s.justify_self = None;
}
