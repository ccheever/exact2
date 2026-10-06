//! Generic code that is shared between multiple layout algorithms
pub(crate) mod absolute;
pub(crate) mod alignment;
pub(crate) mod sizing_keyword;

#[cfg(feature = "content_size")]
pub(crate) mod scrollable_overflow;

/// CSS fit-content is the available border-box width clamped between intrinsic
/// widths. A wrapped text measurer returns ink width, not this used width.
pub(crate) fn fit_content_width(
    tree: &mut impl crate::tree::LayoutPartialTree,
    node: crate::tree::NodeId,
    known_dimensions: crate::geometry::Size<Option<f32>>,
    known_dimensions_are_definite: crate::geometry::Size<bool>,
    parent_size: crate::geometry::Size<Option<f32>>,
    available_width: f32,
    sizing_mode: crate::tree::SizingMode,
) -> f32 {
    use crate::geometry::{AbsoluteAxis, Line, Size};
    use crate::style::AvailableSpace;
    use crate::tree::{LayoutInput, RunMode};
    let mut intrinsic = |width| tree.compute_child_layout(node, LayoutInput {
        known_dimensions, known_dimensions_are_definite, parent_size,
        available_space: Size { width, height: AvailableSpace::MaxContent },
        sizing_mode, axis: AbsoluteAxis::Horizontal.into(), run_mode: RunMode::ComputeSize,
        vertical_margins_are_collapsible: Line::FALSE,
    }).size.width;
    let min = intrinsic(AvailableSpace::MinContent);
    let max = intrinsic(AvailableSpace::MaxContent);
    // CSS Sizing 3: max(min-content, min(max-content, available)); min-content
    // wins when the two invert (a negative margin), as Chrome does.
    min.max(available_width.min(max))
}
