//! A table laid out as one grid (LLP 1153 D7.3). Its cells are placed where
//! HTML's table-forming algorithm puts them (HTML §4.9.12.1), so `colspan`
//! and `rowspan` land as the browser lands them, and the table's engine node
//! holds its captions, row groups, rows and cells as the grid's items: a
//! cell's column is the table's, whatever row holds it. A row or row group
//! keeps no engine children; its box spans its rows, and publication places
//! what it holds from the grid's origin, moved as the row or group is moved
//! relatively ([`Tables::flat`]). A table whose author said a `display` is
//! that box, holding an anonymous table, a node of the engine's own, as the
//! grid (CSS 2.2 §17.2.1). The columns are `auto` tracks, the deviation
//! `style/table.rs` declares.
//!
//! A change to a table's rows or cells marks it stale; its grid is made
//! again once, before the next layout ([`LayoutTree::refresh_tables`]).

use super::{order, LayoutTree};
use crate::arena::NodeArena;
use crate::generated::{Display, PropId};
use crate::id::{IdMap, IdSet};
use crate::props::PropValue;
use crate::style::table::{lays_out, table_of, Part};
use taffy::prelude::{
    auto, length, line, span, GridPlacement, GridTemplateComponent, Line, NodeId,
};

/// Where a part stands in its table's grid, from 0: its first row and
/// column and how many of each it covers; `columns` 0 is every column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Area {
    pub row: u32,
    pub rows: u32,
    pub column: u32,
    pub columns: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Placed {
    table: u32,
    area: Area,
    /// A cell's `colspan` and `rowspan` as it was placed by them.
    spans: (u32, u32),
    /// A row group or row: the table lays out what it holds.
    flat: bool,
}

#[derive(Clone, Debug, Default)]
struct Formed {
    columns: u32,
    /// Its parts in grid order: captions, then each group and its rows, each
    /// row followed by its cells.
    parts: Vec<u32>,
    /// The table's engine padding, which its captions' margins follow; none
    /// for a table that is another box.
    padding: Option<taffy::geometry::Rect<taffy::style::LengthPercentage>>,
    /// The anonymous table that holds the parts of a table whose author said
    /// a `display`.
    anon: Option<NodeId>,
}

/// The engine style of an anonymous table (CSS 2.2 §17.2.1): a grid of
/// `columns` `auto` tracks, shrink-to-fit, spaced as HTML's UA sheet
/// spaces a table's cells, or as the table's `border-spacing` does.
fn anonymous(
    columns: u32,
    direction: taffy::style::Direction,
    spacing: f32,
) -> taffy::style::Style {
    let mut s = taffy::style::Style {
        display: taffy::style::Display::Grid,
        item_is_table: true,
        direction,
        padding: taffy::geometry::Rect {
            left: length(spacing),
            right: length(spacing),
            top: length(spacing),
            bottom: length(spacing),
        },
        gap: taffy::geometry::Size {
            width: length(spacing),
            height: length(spacing),
        },
        ..Default::default()
    };
    let track = GridTemplateComponent::Single(auto());
    s.rare.grid_template_columns = vec![track; columns as usize];
    s
}

/// The tables an engine tree lays out as grids.
#[derive(Clone, Default)]
pub(crate) struct Tables {
    stale: IdSet<u32>,
    formed: IdMap<u32, Formed>,
    placed: IdMap<u32, Placed>,
}

impl Tables {
    /// Whether `slot`'s children are its table's to lay out, marking that
    /// table to be made again: a table, or a row group or row in one. Its
    /// engine node holds none of them itself.
    pub(super) fn holds(&mut self, arena: &NodeArena, slot: u32) -> bool {
        let table = match Part::of(arena, slot) {
            Some(Part::Table) => slot,
            Some(Part::Group | Part::Row) => match table_of(arena, slot) {
                Some(table) => table,
                None => return false,
            },
            _ => return false,
        };
        self.stale.insert(table);
        true
    }

    /// Whether publication places `slot`'s children from the table's
    /// origin: `slot` is a row group or row of a table laid out as a grid.
    pub(super) fn flat(&self, slot: u32) -> bool {
        self.placed.get(&slot).is_some_and(|p| p.flat)
    }

    /// The tables among `slots` (a rebuilt or trial tree's), to be made.
    pub(super) fn find(&mut self, arena: &NodeArena, slots: &[u32]) {
        let tables = slots
            .iter()
            .filter(|s| Part::of(arena, **s) == Some(Part::Table));
        self.stale.extend(tables);
    }

    /// The anonymous table holding `slot`'s parts, when it has one.
    pub(super) fn anon(&self, slot: u32) -> Option<NodeId> {
        self.formed.get(&slot).and_then(|f| f.anon)
    }

    /// The table whose anonymous table `node` is.
    pub(super) fn holding(&self, node: NodeId) -> Option<u32> {
        let mut tables = self.formed.iter();
        tables.find(|(_, f)| f.anon == Some(node)).map(|(t, _)| *t)
    }

    /// A removed node's records, and the anonymous table it had.
    pub(super) fn forget(&mut self, slot: u32) -> Option<NodeId> {
        self.stale.remove(&slot);
        self.placed.remove(&slot);
        let formed = self.formed.remove(&slot)?;
        self.placed.retain(|_, p| p.table != slot);
        formed.anon
    }

    /// `slot`'s engine style with its grid placement, or its table's
    /// columns. A change that moves the grid (a span, a part shown or
    /// hidden, a table's `display`) marks the table to be made again.
    pub(super) fn style(&mut self, arena: &NodeArena, slot: u32, s: &mut taffy::style::Style) {
        let Some(part) = Part::of(arena, slot) else {
            return;
        };
        if part == Part::Table {
            let laid = lays_out(arena, slot);
            match self.formed.get(&slot) {
                Some(formed) if formed.anon.is_none() == laid => {
                    if laid {
                        let track = GridTemplateComponent::Single(auto());
                        s.rare.grid_template_columns = vec![track; formed.columns as usize];
                        if formed.padding.as_ref() != Some(&s.padding) {
                            self.stale.insert(slot);
                        }
                    }
                }
                _ => {
                    self.stale.insert(slot);
                }
            }
            return;
        }
        match (self.placed.get(&slot), shown_in(arena, slot)) {
            (Some(p), Some(table)) if p.table == table && p.spans == spans(arena, slot, part) => {
                place(p.area, s)
            }
            (None, None) => {}
            (p, table) => self
                .stale
                .extend(table.into_iter().chain(p.map(|p| p.table))),
        }
    }
}

/// What `position: relative` moves `slot`'s box by where its insets are
/// lengths, `left` over `right` and `top` over `bottom` (CSS 2.2 §9.4.3).
/// A row's or row group's cells are the grid's items beside it, not its
/// children in the engine, so publication moves them by this.
pub(super) fn relative(arena: &NodeArena, slot: u32) -> (f32, f32) {
    use crate::style::Dimension;
    let s = arena.style(slot);
    if s.position_type != crate::generated::PositionType::Relative {
        return (0.0, 0.0);
    }
    let env = arena.env();
    let px = |d: Dimension| match d.resolve(env) {
        Dimension::Points(v) => Some(v),
        _ => None,
    };
    let x = px(s.left).or_else(|| px(s.right).map(|v| -v));
    let y = px(s.top).or_else(|| px(s.bottom).map(|v| -v));
    (x.unwrap_or(0.0), y.unwrap_or(0.0))
}

/// The table `slot` is placed in: `slot` and each part between it and the
/// table shown, as [`form`] places them.
fn shown_in(arena: &NodeArena, slot: u32) -> Option<u32> {
    let table = table_of(arena, slot)?;
    let mut at = slot;
    while at != table {
        if arena.style(at).display == Display::None {
            return None;
        }
        at = arena.parent(at)?;
    }
    Some(table)
}

/// A cell's `colspan` and `rowspan`, as HTML reads them (HTML §4.9.11): unset
/// or negative is 1, a `colspan` of 0 is 1, a `rowspan` of 0 is 0, the rest
/// of the cell's row group, and each at most HTML's limit.
fn spans(arena: &NodeArena, slot: u32, part: Part) -> (u32, u32) {
    if part != Part::Cell {
        return (1, 1);
    }
    let read = |id, least: i64, most: i64| {
        let n = match arena.props(slot).get(id) {
            Some(PropValue::Int(n)) => *n,
            Some(PropValue::Float(n)) if n.is_finite() => *n as i64,
            _ => 1,
        };
        if n < 0 {
            1
        } else {
            n.clamp(least, most) as u32
        }
    };
    (
        read(PropId::ColSpan, 1, 1000),
        read(PropId::RowSpan, 0, 65534),
    )
}

fn place(area: Area, s: &mut taffy::style::Style) {
    let at = |n: u32| line::<GridPlacement>(i16::try_from(n + 1).unwrap_or(i16::MAX));
    let cover = |n: u32| span::<GridPlacement>(u16::try_from(n.max(1)).unwrap_or(u16::MAX));
    s.rare.grid_row = Line {
        start: at(area.row),
        end: cover(area.rows),
    };
    s.rare.grid_column = if area.columns == 0 {
        Line {
            start: line(1),
            end: line(-1),
        }
    } else {
        Line {
            start: at(area.column),
            end: cover(area.columns),
        }
    };
    // A row group with no shown row has no area.
    if area.rows == 0 {
        s.display = taffy::style::Display::None;
    }
}

/// One placed part: its slot, its area, and a cell's spans as read.
type Item = (u32, Area, (u32, u32));

/// HTML's table-forming algorithm (HTML §4.9.12.1) over `table`'s shown
/// parts, as the browser lays it out: the captions first, each in a row of
/// its own; then the row groups and runs of rows in order, but the first
/// `thead` before them and the first `tfoot` after them (CSS 2.1 §17.2's
/// header and footer groups; a later one is a row group where it stands); a
/// cell's rows end with its row group's, where CSS's rendering ends them.
fn form(arena: &NodeArena, table: u32) -> (Formed, Vec<Item>) {
    let of =
        |s: u32, part| Part::of(arena, s) == Some(part) && arena.style(s).display != Display::None;
    let children = arena.children(table);
    let mut out: Vec<Item> = Vec::new();
    let whole = |row, rows| Area {
        row,
        rows,
        column: 0,
        columns: 0,
    };
    let mut y = 0;
    for &c in children.iter().filter(|c| of(**c, Part::Caption)) {
        out.push((c, whole(y, 1), (1, 1)));
        y += 1;
    }
    // Each group: its node, if it is one, and its rows; a run of rows the
    // table holds itself is a group of its own.
    let mut groups: Vec<(Option<u32>, Vec<u32>)> = Vec::new();
    let (mut head, mut foot) = (None, None);
    let mut run = Vec::new();
    for &c in children {
        if of(c, Part::Row) {
            run.push(c);
            continue;
        }
        if !run.is_empty() {
            groups.push((None, std::mem::take(&mut run)));
        }
        if of(c, Part::Group) {
            let rows = arena.children(c).iter().copied();
            let group = (Some(c), rows.filter(|r| of(*r, Part::Row)).collect());
            match arena.props(c).str(PropId::SemanticTag) {
                Some("thead") if head.is_none() => head = Some(group),
                Some("tfoot") if foot.is_none() => foot = Some(group),
                _ => groups.push(group),
            }
        }
    }
    if !run.is_empty() {
        groups.push((None, run));
    }
    let groups = head.into_iter().chain(groups).chain(foot);
    let mut columns = 0;
    for (group, rows) in groups {
        if let Some(g) = group {
            out.push((g, whole(y, rows.len() as u32), (1, 1)));
        }
        // Which of the group's slots a cell covers, by row from its first.
        let mut taken: Vec<Vec<bool>> = vec![Vec::new(); rows.len()];
        for (r, &row) in rows.iter().enumerate() {
            out.push((row, whole(y, 1), (1, 1)));
            let mut x = 0u32;
            for &cell in arena.children(row).iter().filter(|c| of(**c, Part::Cell)) {
                while taken[r].get(x as usize).copied().unwrap_or(false) {
                    x += 1;
                }
                let (colspan, rowspan) = spans(arena, cell, Part::Cell);
                let left = (rows.len() - r) as u32;
                let covers = if rowspan == 0 {
                    left
                } else {
                    rowspan.min(left)
                };
                let end = (x + colspan) as usize;
                for slots in taken.iter_mut().skip(r).take(covers as usize) {
                    if slots.len() < end {
                        slots.resize(end, false);
                    }
                    slots[x as usize..end].fill(true);
                }
                let area = Area {
                    row: y,
                    rows: covers,
                    column: x,
                    columns: colspan,
                };
                out.push((cell, area, (colspan, rowspan)));
                x += colspan;
                columns = columns.max(x);
            }
            y += 1;
        }
    }
    let parts = out.iter().map(|(s, _, _)| *s).collect();
    let formed = Formed {
        columns,
        parts,
        ..Formed::default()
    };
    (formed, out)
}

impl LayoutTree {
    /// Make every stale table's grid: its parts as its engine node's items,
    /// each placed, a row's and a row group's own children none. A part no
    /// longer in it, or a table that no longer lays out as one, gets its
    /// children and its style back. `node_for` is the tree's handle map.
    pub(super) fn refresh_tables(
        &mut self,
        arena: &NodeArena,
        node_for: &dyn Fn(u32) -> Option<NodeId>,
    ) {
        // A part restored from one table can stand in another, which its
        // restyle marks: that one is made in the next round.
        for _ in 0..4 {
            if self.tables.stale.is_empty() {
                return;
            }
            let mut stale: Vec<u32> = self.tables.stale.drain().collect();
            stale.sort_unstable();
            for table in stale {
                self.make_table(arena, table, node_for);
            }
        }
        self.tables.stale.clear();
    }

    fn make_table(
        &mut self,
        arena: &NodeArena,
        table: u32,
        node_for: &dyn Fn(u32) -> Option<NodeId>,
    ) {
        let before = self.tables.formed.remove(&table);
        let first = before.is_none();
        let before = before.unwrap_or_default();
        // Where each part stood: one that stays there keeps its engine style.
        let was: IdMap<u32, Placed> = (self.tables.placed.iter())
            .filter(|(_, p)| p.table == table)
            .map(|(s, p)| (*s, *p))
            .collect();
        self.tables.placed.retain(|_, p| p.table != table);
        if !arena.is_live(table) || Part::of(arena, table) != Some(Part::Table) {
            if let Some(anon) = before.anon {
                self.remove(anon);
            }
            return;
        }
        let (mut formed, placed) = form(arena, table);
        let laid = lays_out(arena, table);
        let style = crate::style::taffy_style(arena, table);
        formed.padding = laid.then_some(style.padding);
        for &(slot, area, spans) in &placed {
            let flat = matches!(Part::of(arena, slot), Some(Part::Group | Part::Row));
            let placed = Placed {
                table,
                area,
                spans,
                flat,
            };
            self.tables.placed.insert(slot, placed);
        }
        // What left the grid holds its own children again, unless another
        // table placed it.
        let kept: IdSet<u32> = formed.parts.iter().copied().collect();
        let left = before.parts.iter().copied().filter(|s| {
            !kept.contains(s) && arena.is_live(*s) && !self.tables.placed.contains_key(s)
        });
        for slot in left.collect::<Vec<u32>>() {
            if let Some(node) = node_for(slot) {
                let before = self.alert_prefixes.get(&slot).copied();
                let ids = order::laid_out(arena, slot, &self.taffy, node, before, node_for);
                self.set_children(node, &ids);
                self.restyle_with(arena, slot, node);
            }
        }
        for &(slot, _, _) in &placed {
            if self.tables.flat(slot) {
                if let Some(node) = node_for(slot) {
                    self.set_children(node, &[]);
                }
            }
        }
        let Some(node) = node_for(table) else {
            return;
        };
        // A table whose author said a `display` is that box, and its parts
        // are an anonymous table inside it (CSS 2.2 §17.2.1): the grid is a
        // node of the engine's own, the box's only child.
        let items: Vec<NodeId> = formed.parts.iter().filter_map(|s| node_for(*s)).collect();
        formed.anon = match (laid, before.anon) {
            (true, anon) => {
                self.set_children(node, &items);
                if let Some(anon) = anon {
                    self.remove(anon);
                }
                None
            }
            (false, anon) => {
                let grid = anonymous(
                    formed.columns,
                    style.direction,
                    crate::style::table::spacing(arena.style(table)),
                );
                let anon = match anon {
                    Some(anon) => {
                        self.write_style(anon, grid);
                        anon
                    }
                    None => {
                        let grid = self.share(grid);
                        match self.taffy.new_leaf(grid) {
                            Ok(anon) => {
                                self.made(anon);
                                self.taffy.track_layout_input(anon, false);
                                anon
                            }
                            Err(e) => {
                                self.fault.get_or_insert_with(|| format!("table: {e:?}"));
                                return;
                            }
                        }
                    }
                };
                self.set_children(anon, &items);
                self.set_children(node, &[anon]);
                Some(anon)
            }
        };
        let restyle = first
            || before.columns != formed.columns
            || before.anon.is_some() != formed.anon.is_some();
        // A caption's margins and width follow the table's padding.
        let padded = before.padding != formed.padding;
        let parts = formed.parts.clone();
        self.tables.formed.insert(table, formed);
        if restyle {
            self.restyle_with(arena, table, node);
        }
        for slot in parts {
            let moved = was.get(&slot) != self.tables.placed.get(&slot);
            let caption = padded && Part::of(arena, slot) == Some(Part::Caption);
            if let Some(node) = node_for(slot).filter(|_| moved || caption) {
                self.restyle_with(arena, slot, node);
            }
        }
    }

    /// `slot`'s engine style as `restyle` derives it.
    pub(super) fn restyle_with(&mut self, arena: &NodeArena, slot: u32, node: NodeId) {
        let mut style = crate::style::taffy_style(arena, slot);
        self.tables.style(arena, slot, &mut style);
        self.set_style(node, style);
    }
}
