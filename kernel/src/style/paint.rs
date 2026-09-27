//! SVG's `<paint>` for a path's `fill` and `stroke` (LLP 1065 D3).

use super::{ColorValue, RowValue};
use crate::generated::StyleId;
use crate::kernel::NodeRef;

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
