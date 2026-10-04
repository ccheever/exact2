//! @ref LLP 1093 D1–D7, D10 — CSS multi-column layout: one flow, cut into columns.
//!
//! A multi-column container's in-flow content is laid out once by Taffy, as
//! one column of the used column width (the *flow thread*, vendored Taffy's
//! Patch 27). Every column has that width, so a paragraph breaks into lines
//! once, whatever column it lands in: fragmenting changes no line, it only
//! chooses which lines go in which column. [`settle`] walks each container's
//! flow thread in document order and cuts it into column-height intervals
//! at legal break points, as Chrome breaks ([`cut`]). The result is a sparse
//! record, like resolved exclusions: a translation for every box in a flow,
//! the fragments of a box that straddles columns, and each container's column
//! rectangles. Publication applies the translations; hosts paint and hit the
//! fragments. The web emits the rows as CSS and the browser does all of it.

mod build;
mod cut;
#[cfg(test)]
mod tests;

use crate::arena::NodeArena;
use crate::error::LayoutError;
use crate::generated::{Display, NodeType, StyleProps};
use crate::id::{IdMap, NodeKey};
use crate::layout::LayoutTree;
use crate::sorted::SlotSet;
use crate::text::TextMeasurer;
use crate::{Dimension, FlexDirection, FlexWrap, Kernel, Overflow};

/// A column of a multi-column container, in its border-box space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Column {
    /// Left edge.
    pub x: f32,
    /// Top edge: the container's content top.
    pub y: f32,
    /// The used column width.
    pub width: f32,
    /// The column height.
    pub height: f32,
    /// Whether some box (a zero-height one included) is in this column. A
    /// column rule is drawn between two columns that both hold one (D7).
    pub holds: bool,
}

/// A multi-column container's columns: the `N` the width makes room for,
/// then the overflow columns its content needed, left to right in inline
/// order. Hosts draw `column-rule` from these and the container's rows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Columns {
    /// Each column, in the order content flows through them.
    pub columns: Vec<Column>,
    /// The used gap between two columns.
    pub gap: f32,
}

/// One piece of a box that straddles columns (D7): a rectangle in the box's
/// published (union) frame, sliced as CSS's `box-decoration-break: slice`
/// slices it. A paragraph's fragment also names its lines: the line boxes
/// `lines` paint at their unfragmented content positions plus (dx, dy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fragment {
    /// Left edge, in the box's published frame.
    pub x: f32,
    /// Top edge, in the box's published frame.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// The paragraph's line boxes in this fragment, `start..end`; empty for
    /// a box that is not a paragraph.
    pub lines: (u32, u32),
    /// What to add to a point of the unfragmented box (its own border-box
    /// space, as one column laid it out) to place it in this fragment.
    pub dx: f32,
    /// See [`Fragment::dx`].
    pub dy: f32,
}

impl Fragment {
    /// Whether `(x, y)`, in the box's published frame, is inside.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// Why a box in a multi-column flow is kept whole where Chrome would
/// fragment it (LLP 1093 D10). Journalled once per box, when it does not fit
/// its column; `message` says what to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentRefusal {
    /// D10.1, D10.7: a `height`, `min-height` or `max-height`.
    Height,
    /// D10.2: a row or wrapped flexbox, a grid, or a nested multi-column box.
    Layout,
    /// D10.3: its own background, border, radius, shadow, filter or clip.
    Decoration,
    /// D10.6: one-node Markdown.
    Markdown,
}

impl FragmentRefusal {
    /// What the author can change, for the journal line.
    pub fn message(self) -> &'static str {
        match self {
            FragmentRefusal::Height => "it has a height, min-height or max-height; give it an automatic height so its content can continue in the next column",
            FragmentRefusal::Layout => "it is a row or wrapping flexbox, a grid or a multi-column box, which exact2 keeps whole in a column; use a block or a column",
            FragmentRefusal::Decoration => "it has its own background, border, radius, shadow, filter or clip, which exact2 does not slice across columns yet; move the decoration to boxes that fit a column",
            FragmentRefusal::Markdown => "one-node Markdown is laid out as one box; split the text into paragraphs to let it continue in the next column",
        }
    }
}

/// A box's place in its flow: what publication adds to its position in the
/// flow thread (relative to its container's own translation), and for a box
/// that straddles columns, the union frame's size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    pub dx: f32,
    pub dy: f32,
    pub size: Option<(f32, f32)>,
}

/// One container's cut, as stored.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Multicol {
    /// The columns' content-box height Taffy sizes an automatic height from.
    pub used_height: Option<f32>,
    /// The columns' scrollable overflow, in the border-box space.
    pub overflow: Option<taffy::Rect<f32>>,
    pub columns: Columns,
    /// The boxes this cut placed, so the next cut clears those it does not.
    placed: Vec<u32>,
}

/// The kernel's fragmentation record. Empty, it costs one branch per layout.
#[derive(Debug, Clone, Default)]
pub(crate) struct FragState {
    /// Every node whose style makes it a multi-column container.
    pub containers: SlotSet,
    pub multicol: IdMap<u32, Multicol>,
    pub placements: IdMap<u32, Placement>,
    pub fragments: IdMap<u32, Vec<Fragment>>,
    pub refusals: IdMap<u32, FragmentRefusal>,
    /// Containers whose cut changed since publication last walked them.
    pub republish: SlotSet,
}

impl FragState {
    pub(crate) fn forget(&mut self, slot: u32) {
        self.containers.remove(slot);
        if let Some(cut) = self.multicol.remove(&slot) {
            for placed in cut.placed {
                self.placements.remove(&placed);
                self.fragments.remove(&placed);
                self.refusals.remove(&placed);
            }
        }
        self.placements.remove(&slot);
        self.fragments.remove(&slot);
        self.refusals.remove(&slot);
        self.republish.remove(slot);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.containers.is_empty() && self.multicol.is_empty()
    }
}

/// Whether a style makes its node a multi-column container (D1): a block
/// container whose `column-count` or `column-width` is not `auto`.
pub(crate) fn is_multicol(s: &StyleProps) -> bool {
    s.display == Display::Block && (s.column_count != 0 || s.column_width != Dimension::Auto)
}

/// The used gap (§1): the row when authored, else CSS's `normal`, 1em of the
/// element's own font size.
fn gap(s: &StyleProps) -> f32 {
    if s.mask.has(crate::StyleId::ColumnGap) {
        s.column_gap.max(0.0)
    } else {
        s.font_size
    }
}

/// The engine's multicol field for `slot` (D2): its rows, and the height and
/// overflow its last cut wrote.
pub(crate) fn taffy_multicol(arena: &NodeArena, slot: u32) -> Option<taffy::Multicol> {
    let s = arena.style(slot);
    if !is_multicol(s) || !matches!(arena.node_type(slot), NodeType::View | NodeType::Text) {
        return None;
    }
    let cut = arena.frag.multicol.get(&slot);
    Some(taffy::Multicol {
        count: (s.column_count != 0).then_some(s.column_count),
        width: match s.column_width.resolve(arena.env()) {
            Dimension::Points(w) => Some(w.max(0.0)),
            _ => None,
        },
        gap: gap(s),
        used_height: cut.and_then(|c| c.used_height),
        overflow: cut.and_then(|c| c.overflow),
    })
}

/// How a box takes part in a flow (D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    /// Its children continue in the next column.
    Fragmentable,
    /// Its line boxes do.
    Paragraph,
    /// Pushed whole to the next column; `Some` where Chrome would fragment it.
    Monolithic(Option<FragmentRefusal>),
}

/// Whether a box paints decoration a host would have to slice (D10.3).
fn decorated(s: &StyleProps) -> bool {
    use crate::style::ColorValue;
    let background = match s.background_color {
        Some(ColorValue::Fixed(c)) => c.a() > 0,
        Some(ColorValue::LightDark(a, b)) => a.a() > 0 || b.a() > 0,
        _ => true,
    };
    let borders = [
        (s.border_width_top, s.border_style_top),
        (s.border_width_right, s.border_style_right),
        (s.border_width_bottom, s.border_style_bottom),
        (s.border_width_left, s.border_style_left),
    ]
    .iter()
    .any(|(w, style)| *w > 0.0 && *style == crate::BorderStyle::Solid);
    let radius = [
        s.border_radius_top_left,
        s.border_radius_top_right,
        s.border_radius_bottom_right,
        s.border_radius_bottom_left,
    ]
    .iter()
    .any(|r| !matches!(r, Dimension::Points(p) if *p == 0.0));
    background
        || borders
        || radius
        || s.background_image != Default::default()
        || !s.box_shadow.0.is_empty()
        || !s.filter.is_none()
        || s.clip_path != Default::default()
}

pub(crate) fn kind(arena: &NodeArena, slot: u32) -> Kind {
    let s = arena.style(slot);
    let sized = |d: Dimension| !matches!(d, Dimension::Auto);
    let floor = |d: Dimension| !matches!(d, Dimension::Auto | Dimension::Points(0.0));
    let sized = sized(s.height) || floor(s.min_height) || sized(s.max_height);
    match arena.node_type(slot) {
        NodeType::Text if sized => Kind::Monolithic(Some(FragmentRefusal::Height)),
        NodeType::Text if arena.paragraph(slot).markup == crate::text::Markup::Markdown => {
            Kind::Monolithic(Some(FragmentRefusal::Markdown))
        }
        NodeType::Text => Kind::Paragraph,
        NodeType::View | NodeType::Pressable => {
            if s.overflow_x != Overflow::Visible
                || s.overflow_y != Overflow::Visible
                || arena.is_button(slot)
            {
                // Chrome keeps these whole too.
                return Kind::Monolithic(None);
            }
            let layout = match s.display {
                _ if is_multicol(s) => false,
                Display::Block => true,
                Display::Flex => {
                    s.flex_direction == FlexDirection::Column && s.flex_wrap == FlexWrap::Nowrap
                }
                _ => false,
            };
            if !layout {
                Kind::Monolithic(Some(FragmentRefusal::Layout))
            } else if sized {
                Kind::Monolithic(Some(FragmentRefusal::Height))
            } else if decorated(s) {
                Kind::Monolithic(Some(FragmentRefusal::Decoration))
            } else {
                Kind::Fragmentable
            }
        }
        // Replaced elements, controls, scrollers and the rest: whole in
        // Chrome too.
        _ => Kind::Monolithic(None),
    }
}

/// Whether `slot` is laid out inside some multi-column container's flow.
pub(crate) fn in_flow(arena: &NodeArena, slot: u32) -> bool {
    let mut at = arena.parent(slot);
    while let Some(s) = at {
        if arena.frag.containers.contains(s) && arena.frag.multicol.contains_key(&s) {
            return true;
        }
        at = arena.parent(s);
    }
    false
}

/// Cut every multi-column container under `root`, writing each one's used
/// height and overflow into the engine and laying out again until the cuts
/// hold (D5: the flow thread depends on the column width only, so a second
/// pass reaches the fixed point; a nested container may take one more).
/// Returns the extra engine passes.
pub(crate) fn settle(
    arena: &mut NodeArena,
    tree: &mut LayoutTree,
    measurer: &mut dyn TextMeasurer,
    root: u32,
    offer: crate::id::Offer,
) -> Result<usize, LayoutError> {
    if arena.frag.is_empty() {
        return Ok(0);
    }
    let Some(root_node) = arena.taffy(root) else {
        return Ok(0);
    };
    let mut passes = 0;
    loop {
        let mut cuts = Vec::new();
        for slot in arena.frag.containers.iter().collect::<Vec<_>>() {
            match laid_out_under(arena, root, slot) {
                Some(true) => cuts.push((slot, cut::container(arena, tree, measurer, slot))),
                // Hidden under this root: no columns until it shows.
                Some(false) if arena.frag.multicol.contains_key(&slot) => {
                    cuts.push((slot, cut::Cut::default()));
                }
                _ => {}
            }
        }
        // A container that stopped being one drops its record and its boxes'
        // placements.
        for &slot in arena.frag.multicol.keys() {
            if !arena.frag.containers.contains(slot) {
                cuts.push((slot, cut::Cut::default()));
            }
        }
        // Every old record goes before any new one is written: a box that
        // moved from one flow to another is placed by its new container.
        let old: Vec<_> = cuts.iter().map(|(slot, _)| clear(arena, *slot)).collect();
        let mut engine = Vec::new();
        for ((slot, cut), old) in cuts.into_iter().zip(old) {
            if store(arena, slot, cut, old) {
                engine.push(slot);
            }
        }
        if engine.is_empty() {
            return Ok(passes);
        }
        for &slot in &engine {
            if let Some(node) = arena.taffy(slot) {
                tree.set_style(node, crate::style::taffy_style(arena, slot));
            }
        }
        if passes == 8 {
            debug_assert!(false, "multi-column layout did not settle");
            return Ok(passes);
        }
        tree.compute(root_node, offer, arena, measurer)?;
        passes += 1;
    }
}

// Whether `slot` is under `root`, and then whether it was laid out (no
// `display: none` on the way); `None` under another root or none.
fn laid_out_under(arena: &NodeArena, root: u32, slot: u32) -> Option<bool> {
    let mut shown = true;
    let mut at = Some(slot);
    while let Some(s) = at {
        shown &= arena.style(s).display != Display::None && arena.taffy(s).is_some();
        if s == root {
            return Some(shown);
        }
        at = arena.parent(s);
    }
    None
}

/// A container's last record, taken out with its boxes' placements.
#[derive(Default)]
struct Old {
    record: Multicol,
    placements: IdMap<u32, Placement>,
    fragments: IdMap<u32, Vec<Fragment>>,
}

fn clear(arena: &mut NodeArena, slot: u32) -> Old {
    let frag = &mut arena.frag;
    let mut old = Old {
        record: frag.multicol.remove(&slot).unwrap_or_default(),
        ..Old::default()
    };
    for &placed in &old.record.placed {
        if let Some(p) = frag.placements.remove(&placed) {
            old.placements.insert(placed, p);
        }
        if let Some(f) = frag.fragments.remove(&placed) {
            old.fragments.insert(placed, f);
        }
        frag.refusals.remove(&placed);
    }
    old
}

// Store a container's cut, marking it for publication when the record
// changed; whether what the engine reads of it (its used height and
// overflow) changed.
fn store(arena: &mut NodeArena, slot: u32, cut: cut::Cut, old: Old) -> bool {
    let frag = &mut arena.frag;
    let engine = old.record.used_height != cut.used_height || old.record.overflow != cut.overflow;
    let mut changed = old.record.columns != cut.columns;
    let mut placed = Vec::with_capacity(cut.placements.len() + cut.fragments.len());
    for (s, p) in cut.placements {
        changed |= old.placements.get(&s) != Some(&p);
        frag.placements.insert(s, p);
        placed.push(s);
    }
    for (s, f) in cut.fragments {
        changed |= old.fragments.get(&s) != Some(&f);
        frag.fragments.insert(s, f);
        placed.push(s);
    }
    for (s, r) in cut.refusals {
        frag.refusals.insert(s, r);
        placed.push(s);
    }
    changed |= placed.len() != old.record.placed.len();
    if changed || engine {
        frag.republish.insert(slot);
        arena.layout_dirty.insert(slot);
    }
    if cut.columns.columns.is_empty() && cut.used_height.is_none() && placed.is_empty() {
        return engine;
    }
    arena.frag.multicol.insert(
        slot,
        Multicol {
            used_height: cut.used_height,
            overflow: cut.overflow,
            columns: cut.columns,
            placed,
        },
    );
    engine
}

/// The kernel-only monolithic boxes the last cuts kept whole across a
/// column's end, in no particular order (LLP 1093 D10).
pub(crate) fn skipped(arena: &NodeArena) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = arena.frag.refusals.keys().map(|&s| arena.key(s)).collect();
    keys.sort_by_key(|k| k.index);
    keys
}

impl Kernel {
    /// The fragments of a box that straddles columns of a multi-column
    /// container, as last laid out (LLP 1093 D7): one per column it touches,
    /// in its published (union) frame. `None` for a box wholly in one column
    /// or outside every multi-column flow, which is hit and painted as its
    /// frame. A multi-column `text` has one fragment per column holding
    /// lines, in its own frame.
    pub fn fragments(&self, key: NodeKey) -> Option<&[Fragment]> {
        let slot = self.arena().resolve(key)?;
        self.arena().frag.fragments.get(&slot).map(Vec::as_slice)
    }

    /// A multi-column container's columns as last laid out (LLP 1093 D7).
    pub fn columns(&self, key: NodeKey) -> Option<&Columns> {
        let slot = self.arena().resolve(key)?;
        self.arena().frag.multicol.get(&slot).map(|m| &m.columns)
    }

    /// Why a box in a multi-column flow is kept whole where Chrome would
    /// fragment it, when its last cut could not fit it in its column.
    pub fn fragment_refusal(&self, key: NodeKey) -> Option<FragmentRefusal> {
        let slot = self.arena().resolve(key)?;
        self.arena().frag.refusals.get(&slot).copied()
    }
}
