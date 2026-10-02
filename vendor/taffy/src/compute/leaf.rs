//! Computes size using styles and measure functions

#[cfg(feature = "content_size")]
use crate::geometry::Rect;
use crate::geometry::Size;
use crate::style::{AvailableSpace, Overflow, Position};
use crate::tree::{Baselines, CollapsibleMarginSet, RunMode};
use crate::tree::{LayoutInput, LayoutOutput, SizingMode};
use crate::util::debug::debug_log;
use crate::compute::ratio::{floors_height, Ratio};
use crate::util::sys::{f32_max, f32_min};
use crate::util::MaybeMath;
use crate::util::{MaybeResolve, ResolveOrZero};
use crate::{BoxSizing, CoreStyle};
use core::unreachable;

/// Compute the size of a leaf node (node with no children)
pub fn compute_leaf_layout<MeasureFunction>(
    inputs: LayoutInput,
    style: &impl CoreStyle,
    resolve_calc_value: impl Fn(*const (), f32) -> f32,
    measure_function: MeasureFunction,
) -> LayoutOutput
where
    MeasureFunction: FnOnce(Size<Option<f32>>, Size<AvailableSpace>) -> Size<f32>,
{
    let LayoutInput { known_dimensions, parent_size, available_space, sizing_mode, run_mode, .. } = inputs;

    // Note: both horizontal and vertical percentage padding/borders are resolved against the container's inline size (i.e. width).
    // This is not a bug, but is how CSS is specified (see: https://developer.mozilla.org/en-US/docs/Web/CSS/padding#values)
    let padding = style.padding().resolve_or_zero(parent_size.width, &resolve_calc_value);
    let border = style.border().resolve_or_zero(parent_size.width, &resolve_calc_value);
    let padding_border = padding + border;
    let pb_sum = padding_border.sum_axes();
    let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox { pb_sum } else { Size::ZERO };

    // Resolve node's preferred/min/max sizes (width/heights) against the available space (percentages resolve to pixel values)
    // For ContentSize mode, we pretend that the node has no size styles as these should be ignored.
    // EXACT PATCH 12 (LLP 1053 G1): the size styles through the ratio
    // (`compute::ratio`); the authored minimum is kept for the table below.
    let ratio = Ratio::of(style, pb_sum);
    let (given, raw_min_size, raw_max_size) = match sizing_mode {
        SizingMode::ContentSize => (known_dimensions, Size::NONE, Size::NONE),
        SizingMode::InherentSize => (
            known_dimensions.or(style.size().maybe_resolve(parent_size, &resolve_calc_value).maybe_add(box_sizing_adjustment)),
            style.min_size().maybe_resolve(parent_size, &resolve_calc_value).maybe_add(box_sizing_adjustment),
            style.max_size().maybe_resolve(parent_size, &resolve_calc_value).maybe_add(box_sizing_adjustment),
        ),
    };
    let floor_height = floors_height(style, raw_min_size.height);
    let (node_size, node_min_size, node_max_size) = match ratio {
        Some(ratio) => ratio.resolve(given, raw_min_size, raw_max_size, floor_height),
        None => (given, raw_min_size, raw_max_size),
    };

    // Scrollbar gutters are reserved when the `overflow` property is set to `Overflow::Scroll`.
    // However, the axis are switched (transposed) because a node that scrolls vertically needs
    // *horizontal* space to be reserved for a scrollbar
    let scrollbar_gutter = style.overflow().transpose().map(|overflow| match overflow {
        Overflow::Scroll => style.scrollbar_width(),
        _ => 0.0,
    });
    // TODO: make side configurable based on the `direction` property
    let mut content_box_inset = padding_border;
    content_box_inset.right += scrollbar_gutter.x;
    content_box_inset.bottom += scrollbar_gutter.y;

    let has_styles_preventing_being_collapsed_through = !style.is_block()
        || style.overflow().x.is_scroll_container()
        || style.overflow().y.is_scroll_container()
        || style.position() == Position::Absolute
        || style.contain().establishes_independent_formatting_context()
        || padding.top > 0.0
        || padding.bottom > 0.0
        || border.top > 0.0
        || border.bottom > 0.0
        || matches!(node_size.height, Some(h) if h > 0.0)
        || matches!(node_min_size.height, Some(h) if h > 0.0);

    debug_log!("LEAF");
    debug_log!("node_size", dbg:node_size);
    debug_log!("min_size ", dbg:node_min_size);
    debug_log!("max_size ", dbg:node_max_size);

    // Return early if both width and height are known
    if run_mode == RunMode::ComputeSize && has_styles_preventing_being_collapsed_through {
        if let Size { width: Some(width), height: Some(height) } = node_size {
            let size = Size { width, height }
                .maybe_clamp(node_min_size, node_max_size)
                .maybe_max(padding_border.sum_axes().map(Some));
            return LayoutOutput {
                size,
                #[cfg(feature = "content_size")]
                scrollable_overflow_rect: Rect::ZERO,
                baselines: Baselines::NONE,
                top_margin: CollapsibleMarginSet::ZERO,
                bottom_margin: CollapsibleMarginSet::ZERO,
                margins_can_collapse_through: false,
            };
        };
    }

    // Compute available space
    let available_space = Size {
        width: known_dimensions
            .width
            .map(AvailableSpace::from)
            .unwrap_or(available_space.width)
            .maybe_set(known_dimensions.width)
            .maybe_set(node_size.width)
            .map_definite_value(|size| {
                size.maybe_clamp(node_min_size.width, node_max_size.width) - content_box_inset.horizontal_axis_sum()
            }),
        height: known_dimensions
            .height
            .map(AvailableSpace::from)
            .unwrap_or(available_space.height)
            .maybe_set(known_dimensions.height)
            .maybe_set(node_size.height)
            .map_definite_value(|size| {
                size.maybe_clamp(node_min_size.height, node_max_size.height) - content_box_inset.vertical_axis_sum()
            }),
    };

    // Measure node
    let measured_size = measure_function(
        match run_mode {
            RunMode::ComputeSize => known_dimensions,
            RunMode::PerformLayout => Size::NONE,
            RunMode::PerformHiddenLayout => unreachable!(),
        },
        available_space,
    );
    let clamped_size = known_dimensions
        .or(node_size)
        .unwrap_or(measured_size + content_box_inset.sum_axes())
        .maybe_clamp(node_min_size, node_max_size);
    // EXACT PATCH 5 + 12 (LLP 1011 §1, LLP 1053 G1): with a ratio and
    // neither dimension given, a replaced element resolves its natural size
    // against min/max by CSS 2.1 §10.4's table, keeping the ratio; any other
    // box takes its content width and derives its height from it. A given
    // dimension was resolved through the ratio above.
    let size = match ratio {
        Some(ratio) if node_size.width.is_none() && node_size.height.is_none() => {
            if style.is_compressible_replaced() {
                replaced_constraints(measured_size + content_box_inset.sum_axes(), raw_min_size, raw_max_size)
            } else {
                let derived = ratio.height(clamped_size.width);
                let height = if floor_height {
                    f32_max(clamped_size.height, derived.maybe_min(node_max_size.height))
                } else {
                    derived
                };
                Size { width: clamped_size.width, height: height.maybe_clamp(node_min_size.height, node_max_size.height) }
            }
        }
        _ => clamped_size,
    };
    let size = size.maybe_max(padding_border.sum_axes().map(Some));

    // A scroll container's own padding at the end of the content is part of its scrollable
    // overflow region, so it is included in the overflow rect. Boxes that are not scroll
    // containers do not extend their overflow region by their own padding.
    #[cfg(feature = "content_size")]
    let scrollable_overflow_rect = {
        let is_scroll_container = style.overflow().x.is_scroll_container() || style.overflow().y.is_scroll_container();
        let is_rtl = style.direction().is_rtl();
        let start_padding = if is_rtl { padding.right } else { padding.left };
        let end_padding = if is_rtl { padding.left } else { padding.right };
        // EXACT PATCH 5: replaced pixels do not enlarge scrollable overflow.
        if style.is_compressible_replaced() {
            let padding_box = size - border.sum_axes();
            Rect { left: 0.0, top: 0.0, right: padding_box.width, bottom: padding_box.height }
        } else { Rect {
            left: 0.0,
            right: start_padding + measured_size.width + if is_scroll_container { end_padding } else { 0.0 },
            top: 0.0,
            bottom: padding.top + measured_size.height + if is_scroll_container { padding.bottom } else { 0.0 },
        } }
    };

    LayoutOutput {
        size,
        #[cfg(feature = "content_size")]
        scrollable_overflow_rect,
        baselines: Baselines::NONE,
        top_margin: CollapsibleMarginSet::ZERO,
        bottom_margin: CollapsibleMarginSet::ZERO,
        margins_can_collapse_through: !has_styles_preventing_being_collapsed_through
            && size.height == 0.0
            && measured_size.height == 0.0,
    }
}

// EXACT PATCH (LLP 1011 §1): CSS 2.1 §10.4, the min/max constraint table for
// replaced elements with an intrinsic ratio. `w`/`h` are the tentative size.
fn replaced_constraints(size: Size<f32>, min: Size<Option<f32>>, max: Size<Option<f32>>) -> Size<f32> {
    let (w, h) = (size.width, size.height);
    if w <= 0.0 || h <= 0.0 {
        return Size { width: w, height: h }.maybe_clamp(min, max);
    }
    let min_w = min.width.unwrap_or(0.0);
    let min_h = min.height.unwrap_or(0.0);
    let max_w = max.width.unwrap_or(f32::INFINITY).max(min_w);
    let max_h = max.height.unwrap_or(f32::INFINITY).max(min_h);
    let (rw, rh) = match (w > max_w, w < min_w, h > max_h, h < min_h) {
        (false, false, false, false) => (w, h),
        (true, _, true, _) => {
            if max_w / w <= max_h / h {
                (max_w, f32_max(min_h, max_w * h / w))
            } else {
                (f32_max(min_w, max_h * w / h), max_h)
            }
        }
        (_, true, _, true) => {
            if min_w / w <= min_h / h {
                (f32_min(max_w, min_h * w / h), min_h)
            } else {
                (min_w, f32_min(max_h, min_w * h / w))
            }
        }
        (_, true, true, _) => (min_w, max_h),
        (true, _, _, true) => (max_w, min_h),
        (true, _, _, _) => (max_w, f32_max(max_w * h / w, min_h)),
        (_, true, _, _) => (min_w, f32_min(min_w * h / w, max_h)),
        (_, _, true, _) => (f32_max(max_h * w / h, min_w), max_h),
        (_, _, _, true) => (f32_min(min_h * w / h, max_w), min_h),
    };
    Size { width: rw, height: rh }
}
