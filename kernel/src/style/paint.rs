//! SVG's `<paint>` for a path's `fill` and `stroke` (LLP 1065 D3).

use super::{ColorValue, RowValue};
use crate::generated::{NodeType, PropId, StyleId};
use crate::kernel::NodeRef;
use crate::vector::{markers, parse_view_box, MarkerDef, PathData, Placed, PreserveAspectRatio};

/// `none`, `currentcolor`, or a colour — a `light-dark()` pair included.
/// `currentcolor` stays a keyword through inheritance, as CSS computes it,
/// so each path paints its own `color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// `none`: nothing is painted.
    None,
    /// `currentcolor`: the element's `color`.
    CurrentColor,
    /// A colour.
    Color(ColorValue),
}

impl Paint {
    /// The colour painted, `current` standing for `currentcolor`; `None`
    /// for `none`.
    pub fn resolve(self, current: ColorValue) -> Option<ColorValue> {
        match self {
            Paint::None => None,
            Paint::CurrentColor => Some(current),
            Paint::Color(c) => Some(c),
        }
    }

    /// The row as a generic reader sees it: a colour, or its keyword.
    pub fn row(self) -> RowValue<'static> {
        match self {
            Paint::None => RowValue::Enum("none"),
            Paint::CurrentColor => RowValue::Enum("currentcolor"),
            Paint::Color(c) => RowValue::ColorValue(c),
        }
    }
}

impl NodeRef<'_> {
    /// A path's computed SVG paint (`fill` or `stroke`, LLP 1065): the
    /// colour it paints — `currentcolor` as the computed `color` — or `None`
    /// for `none`.
    pub fn paint(&self, row: StyleId) -> Option<ColorValue> {
        match self.computed(row) {
            RowValue::ColorValue(c) => Some(c),
            RowValue::Enum("currentcolor") => Some(self.text_color()),
            _ => None,
        }
    }
}

impl<'a> NodeRef<'a> {
    /// Whether this is a path in a path (LLP 1065 D12): no CSS box, but its
    /// parent's content box, in its coordinate system.
    pub fn in_path(&self) -> bool {
        self.node_type == NodeType::Path
            && self
                .arena
                .parent(self.slot)
                .is_some_and(|p| self.arena.node_type(p) == NodeType::Path)
    }

    /// The parent's style rows, if the node has a parent.
    pub fn parent_style(&self) -> Option<&'a crate::generated::StyleProps> {
        self.arena.parent(self.slot).map(|p| self.arena.style(p))
    }

    /// The coordinate system a path draws in (LLP 1065 D12): its own view
    /// box and `preserveAspectRatio`, or, for a path in a path, those of
    /// the outermost path it belongs to, whose content box it covers.
    pub fn path_viewport(&self) -> (Option<[f32; 4]>, PreserveAspectRatio) {
        let arena = self.arena;
        let mut slot = self.slot;
        while let Some(parent) = arena
            .parent(slot)
            .filter(|p| arena.node_type(*p) == NodeType::Path)
        {
            slot = parent;
        }
        let props = arena.props(slot);
        (
            props.str(PropId::ViewBox).and_then(parse_view_box),
            props
                .str(PropId::PreserveAspectRatio)
                .and_then(PreserveAspectRatio::parse)
                .unwrap_or_default(),
        )
    }

    /// Every marker a path draws on `data` (LLP 1065 D11), by its computed
    /// `marker-*` rows and stroke width.
    pub fn path_markers(&self, data: &PathData) -> Vec<(&'a MarkerDef, Placed)> {
        let row = |id| match self.computed(id) {
            RowValue::Marker(m) => m,
            _ => unreachable!("a marker row"),
        };
        let width = match self.computed(StyleId::StrokeWidth) {
            RowValue::Number(n) => n as f32,
            _ => 1.0,
        };
        markers(
            data,
            [
                row(StyleId::MarkerStart),
                row(StyleId::MarkerMid),
                row(StyleId::MarkerEnd),
            ],
            width,
        )
    }
}
