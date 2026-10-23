//! @ref LLP 1084 D7 — the platform's separator under a grouped list's row.
//!
//! A row spans its card, `border-box`, with the platform's insets as its
//! padding, so a full-width row stays inside the card. The line under it is
//! the platform's decoration, not a border: it takes no layout, it is drawn
//! under the row's children, and it is in the platform's colour. Every host
//! that draws the sheet reads it here; the web's sheet, which keeps no
//! kernel, draws the same line in CSS (`host/web/index.html`).

use crate::generated::{BoxSizing, Direction, Display, Overflow, PropId, StyleId, Visibility};
use crate::id::{Frame, ViewId};
use crate::kernel::{Kernel, NodeRef};
use crate::style::{metrics, ColorValue, StyleValue};

/// Where a grouped row's separator is drawn, and in what colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupedSeparator {
    /// The line in the row's own space, in points: from the row's content
    /// edge to `grouped-separator-end` before its padding box's trailing
    /// edge (mirrored right to left), one point high at the foot of its padding box; where rows
    /// stand apart (`grouped-separator-gap`, the Mac's), in that gap, below
    /// the row's box.
    pub rect: Frame,
    /// The platform's separator colour (`grouped-separator`).
    pub color: ColorValue,
}

impl Kernel {
    /// The separator under the grouped row `id` (Contract marks it,
    /// `groupedRowSeparator`): none under a row that is not shown, the last
    /// row of its card shown, or a row whose author wrote that edge
    /// (`rows::yields`, the rule a list's rows follow, the web's channel
    /// being `background-image`).
    pub fn grouped_separator(&self, id: ViewId) -> Option<GroupedSeparator> {
        let row = self.node(id)?;
        if row.props.bool(PropId::GroupedRowSeparator) != Some(true)
            || row.style.display == Display::None
            || row.computed_row(StyleId::Visibility, |s| s.visibility) != Visibility::Visible
            || !self.grouped_edge_platforms(&row)?
        {
            return None;
        }
        let (left, _, right, _) = self.resolved_padding(row.key)?;
        let (border_left, _, border_right, _) = self.resolved_border(row.key)?;
        let f = row.frame;
        // The Mac's destructive row is its push button's box, which stands
        // in from both ends of the card by its margins (`grouped-button-`
        // metrics): the line spans it, as it spans a row's insets.
        let pushed = row.style.box_sizing == BoxSizing::ContentBox
            && row.props.bool(PropId::Destructive) == Some(true);
        let end = if pushed {
            0.0
        } else {
            super::sheet("-exact-grouped-separator-end")
        };
        // Right to left the row's insets mirror (`style::sides::mirror_unsaid`),
        // and the line runs from its content's right edge to the left.
        let rtl = row.computed_row(StyleId::Direction, |s| s.direction) == Direction::Rtl;
        let (from, to) = match (pushed, rtl) {
            (true, _) => (border_left, f.width - border_right),
            (false, false) => (border_left + left, f.width - border_right - end),
            (false, true) => (border_left + end, f.width - border_right - right),
        };
        if to <= from {
            return None;
        }
        // In the gap below the row where rows stand apart (the Mac's), but
        // inside a row that clips what it holds (a `delete=` row's swipe),
        // which would clip the line away.
        let gap = super::sheet("-exact-grouped-separator-gap");
        let clips = row.style.overflow_y != Overflow::Visible;
        let gap = if clips { 0.0 } else { gap };
        Some(GroupedSeparator {
            rect: Frame {
                x: from,
                y: f.height - 1.0 + gap,
                width: to - from,
                height: 1.0,
            },
            color: separator_color()?,
        })
    }

    /// Whether the platform's line stays at the foot of the grouped row
    /// `row`, as far as its author goes: `None` for the last row of its
    /// card shown, which has none; `Some(false)` where the author wrote the
    /// edge. A row's neighbour is the next one shown, as UIKit's list has
    /// it; a `delete=` row's edges are its content's, which its author wrote.
    pub(crate) fn grouped_edge_platforms(&self, row: &NodeRef<'_>) -> Option<bool> {
        let next = self.next_shown(row)?;
        let (own, after) = (self.swiped(row), self.swiped(&next));
        Some(!crate::rows::yields(
            row,
            Some(own.as_ref().unwrap_or(row)),
            Some(after.as_ref().unwrap_or(&next)),
            StyleId::BackgroundImage,
        ))
    }

    /// Which side's author wrote the edge under the grouped row `row`: the
    /// row's own (its bottom edge, its background image, or hidden from
    /// assistive technology), and the next shown row's (its top edge).
    pub(crate) fn grouped_edge_owners(&self, row: &NodeRef<'_>) -> (bool, bool) {
        let own = self.swiped(row);
        let mine = crate::rows::yields(
            row,
            Some(own.as_ref().unwrap_or(row)),
            None,
            StyleId::BackgroundImage,
        );
        let theirs = self.next_shown(row).is_some_and(|next| {
            let after = self.swiped(&next);
            let w = &after.as_ref().unwrap_or(&next).style.written;
            w.has(StyleId::BorderWidthTop) || w.has(StyleId::BorderStyleTop)
        });
        (mine, theirs)
    }

    /// The row after `row` that is shown.
    fn next_shown(&self, row: &NodeRef<'_>) -> Option<NodeRef<'_>> {
        let siblings = self.node(row.parent?)?.children();
        let at = siblings.iter().position(|c| *c == row.id)?;
        siblings[at + 1..]
            .iter()
            .filter_map(|c| self.node(*c))
            .find(|n| n.style.display != Display::None)
    }

    /// The content a swipe `scroll` names (`swipeContent`), which is the
    /// row its author wrote.
    fn swiped(&self, row: &NodeRef<'_>) -> Option<NodeRef<'_>> {
        let name = row.props.str(PropId::SwipeContent)?;
        // In a wrapper too, as the swipe's own hosts find it (a row holding
        // leading actions and the content).
        let mut stack: Vec<ViewId> = row.children().into_iter().rev().collect();
        while let Some(id) = stack.pop() {
            let Some(node) = self.node(id) else { continue };
            if node.props.str(PropId::Id) == Some(name) {
                return Some(node);
            }
            stack.extend(node.children().into_iter().rev());
        }
        None
    }
}

/// `grouped-separator` in this process's column, as a colour row reads it.
fn separator_color() -> Option<ColorValue> {
    let name = StyleValue::Text("-exact-grouped-separator".into());
    metrics::of(StyleId::BorderColorBottom, &name)??
        .color_value(StyleId::BorderColorBottom)
        .ok()
}
