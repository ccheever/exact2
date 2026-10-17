//! A platform text style's weight and leading (LLP 1115.000 D12.2): where a
//! node's `font-size` names a style (`-exact-headline`) and the node does not
//! write `font-weight` or `line-height`, the style's, for this process's
//! column (the Mac's `headline` is bold, its leadings its own). Named through
//! the `font` shorthand (`fontShorthand`) the style resets both, as CSS's
//! shorthand does, so it beats an inherited value; named by a bare
//! `font-size` it fills only what nothing above says. Per longhand: written
//! on the node, then a shorthand's style, then inherited, then a bare
//! style's, then CSS's default.
use super::*;
use crate::style::metrics;
use crate::style::relative::Unit;
use crate::LineHeight;
use std::sync::LazyLock;

/// Each text style's rows in each column, as a start: its weight and its
/// leading (a ratio, or `normal`).
static STARTS: LazyLock<Vec<[StyleProps; 4]>> = LazyLock::new(|| {
    crate::TEXT_STYLES
        .iter()
        .map(|style| {
            std::array::from_fn(|column| {
                let mut rows = StyleProps {
                    font_weight: style.weights[column],
                    line_height: style.leadings[column]
                        .map_or(LineHeight::Normal, LineHeight::Number),
                    ..StyleProps::default()
                };
                rows.mask.set(StyleId::FontWeight);
                rows.mask.set(StyleId::LineHeight);
                rows
            })
        })
        .collect()
});

use crate::style::relative::column_index as index;

impl NodeArena {
    /// The text style `slot`'s own `font-size` names, and whether the `font`
    /// shorthand named it.
    pub(crate) fn named_text_style(&self, slot: u32) -> Option<(u8, bool)> {
        let own = self.style(slot);
        if !own.mask.has(StyleId::FontSize) {
            return None;
        }
        match own.relative.get(StyleId::FontSize) {
            Some((Unit::TextStyle(id), _)) => Some((
                id,
                self.props(slot).bool(crate::PropId::FontShorthand) == Some(true),
            )),
            _ => None,
        }
    }

    /// The rows a style named through the `font` shorthand gives `slot`: a
    /// start, beneath what the node writes and above what it inherits.
    pub(crate) fn text_style_start(&self, slot: u32) -> Option<&'static StyleProps> {
        match self.named_text_style(slot)? {
            (id, true) => Some(&STARTS.get(usize::from(id))?[index(metrics::column())]),
            _ => None,
        }
    }

    /// A run's rows inside its paragraph (`rows`, its parent's): its own
    /// over them, and its text style's where it names one (LLP 1115.000
    /// D12.2): a shorthand's beneath its own rows, a bare one's only where
    /// nothing at or above it says the row.
    pub(crate) fn run_style_rows(
        &self,
        slot: u32,
        rows: crate::text::RunRows,
    ) -> crate::text::RunRows {
        let own = self.style(slot);
        let above = rows.said();
        let rows = rows.child(own);
        let unsaid = |style: &StyleProps| style.mask.minus(own.mask).minus(own.written);
        if let Some(start) = self.text_style_start(slot) {
            rows.fill(start, unsaid(start))
        } else if let Some(bare) = self.bare_style_rows(slot) {
            rows.fill(bare, unsaid(bare).minus(above))
        } else {
            rows
        }
    }

    /// The rows `slot`'s own bare `font-size` style gives, when its size
    /// names one and no shorthand did (a shorthand's is a start,
    /// [`Self::text_style_start`]).
    pub(crate) fn bare_style_rows(&self, slot: u32) -> Option<&'static StyleProps> {
        match self.named_text_style(slot)? {
            (id, false) => Some(&STARTS.get(usize::from(id))?[index(metrics::column())]),
            _ => None,
        }
    }

    /// The nearest bare `font-size` style at or above `slot`, as CSS would
    /// have its weight inherited from where it is named: its node, and the
    /// rows it gives less those a node on the way up (that node included)
    /// writes, a clear among them (`inherit` takes its parent's).
    pub(crate) fn bare_text_style(
        &self,
        slot: u32,
    ) -> Option<(u32, &'static StyleProps, StyleMask)> {
        let mut written = StyleMask::EMPTY;
        let mut cur = Some(slot);
        while let Some(s) = cur {
            written = written.union(self.style(s).written);
            if let Some(rows) = self.bare_style_rows(s) {
                return Some((s, rows, written));
            }
            cur = self.parents[s as usize];
        }
        None
    }
}

impl crate::NodeRef<'_> {
    /// Whether this node names a text style through the `font` shorthand
    /// (or a heading): its weight and leading reset there.
    pub fn names_text_style_through_font(&self) -> bool {
        matches!(self.arena.named_text_style(self.slot), Some((_, true)))
    }

    /// Whether own row `id` is a heading's implied size or weight: the
    /// compiler's, no author's (LLP 1115.000 D12.2).
    pub fn heading_implies(&self, id: StyleId) -> bool {
        matches!(id, StyleId::FontSize | StyleId::FontWeight)
            && self.style.mask.has(id)
            && !self.style.written.has(id)
            && self.props.str(crate::PropId::AccessibilityRole) == Some("heading")
    }

    /// The node whose platform text style supplies inherited row `id` here
    /// (LLP 1115.000 D12.2): its weight or leading, named through the `font`
    /// shorthand with nothing written below it, or by a bare `font-size` with
    /// nothing above saying the row. `None` when no style does.
    pub fn style_source_of(&self, id: StyleId) -> Option<crate::ViewId> {
        self.arena
            .text_style_source(self.slot, id)
            .map(|s| self.arena.local_id(s))
    }

    /// Row `id` as this node's own text style gives it, when the style is
    /// named here (LLP 1115.000 D12.2): for a writer that declares a style's
    /// weight and leading where it is named and lets them inherit, as the
    /// JS target's stylesheet does.
    pub fn style_given(&self, id: StyleId) -> Option<StyleProps> {
        if self.arena.text_style_source(self.slot, id) != Some(self.slot) {
            return None;
        }
        let mut rows = StyleProps::default();
        let mut mask = crate::StyleMask::default();
        mask.set(id);
        rows.copy_rows(self.arena.computed_source(self.slot, id), mask);
        Some(rows)
    }
}
