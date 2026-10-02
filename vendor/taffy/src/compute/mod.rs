//! Low-level access to the layout algorithms themselves. For a higher-level API, see the [`TaffyTree`](crate::TaffyTree) struct.
//!
//! ### Layout functions
//!
//! The layout functions all take an [`&mut impl LayoutPartialTree`](crate::LayoutPartialTree) parameter, which represents a single container node and it's direct children.
//!
//! | Function                          | Purpose                                                                                                                                                                                            |
//! | ---                               | ---                                                                                                                                                                                                |
//! | [`compute_flexbox_layout`]        | Layout a Flexbox container and it's direct children                                                                                                                                                |
//! | [`compute_grid_layout`]           | Layout a CSS Grid container and it's direct children                                                                                                                                               |
//! | [`compute_block_layout`]          | Layout a Block container and it's direct children                                                                                                                                                  |
//! | [`compute_leaf_layout`]           | Applies common properties like padding/border/aspect-ratio to a node before deferring to a passed closure to determine it's size. Can be applied to nodes like text or image nodes.                |
//! | [`compute_root_layout`]           | Layout the root node of a tree (regardless of it's layout mode). This function is typically called once to begin a layout run.                                                                     |                                                                      |
//! | [`compute_hidden_layout`]         | Mark a node as hidden during layout (like `Display::None`)                                                                                                                                         |
//! | [`compute_cached_layout`]         | Attempts to find a cached layout for the specified node and layout inputs. Uses the provided closure to compute the layout (and then stores the result in the cache) if no cached layout is found. |
//!
//! ### Other functions
//!
//! | Function                          | Requires                                                                                                                                                                                           | Purpose                                                              |
//! | ---                               | ---                                                                                                                                                                                                | ---                                                                  |
//! | [`round_layout`]                  | [`RoundTree`]                                                                                                                                                                                      | Round a tree of float-valued layouts to integer pixels               |
//! | [`print_tree`](crate::print_tree) | [`PrintTree`](crate::PrintTree)                                                                                                                                                                    | Print a debug representation of a node tree and it's computed layout |
//!
pub(crate) mod common;
pub(crate) mod leaf;
// EXACT PATCH 12 (LLP 1053 G1): sizing through a preferred aspect ratio.
pub(crate) mod ratio;

#[cfg(feature = "block_layout")]
pub(crate) mod block;

#[cfg(feature = "float_layout")]
pub(crate) mod float;

#[cfg(feature = "flexbox")]
pub(crate) mod flexbox;

#[cfg(feature = "grid")]
pub(crate) mod grid;

pub use leaf::compute_leaf_layout;

#[cfg(feature = "block_layout")]
pub use self::block::{compute_block_layout, BlockContext, BlockFormattingContext};

#[cfg(feature = "flexbox")]
pub use self::flexbox::compute_flexbox_layout;

#[cfg(feature = "grid")]
pub use self::grid::compute_grid_layout;

#[cfg(feature = "float_layout")]
pub use self::float::{BfcSlot, ContentSlot, FloatContext, FloatIntrinsicWidthCalculator};

use crate::geometry::{Line, Point, Size};
use crate::style::{AvailableSpace, CoreStyle, Overflow};
use crate::tree::{
    Layout, LayoutInput, LayoutOutput, LayoutPartialTree, LayoutPartialTreeExt, NodeId, RoundTree, SizingMode,
};
use crate::util::debug::{debug_log, debug_log_node, debug_pop_node, debug_push_node};
use crate::util::sys::round;
use crate::util::sys::f32_max;
use crate::util::ResolveOrZero;
use crate::{CacheTree, MaybeMath, MaybeResolve};

/// Compute layout for the root node in the tree
pub fn compute_root_layout(tree: &mut impl LayoutPartialTree, root: NodeId, available_space: Size<AvailableSpace>) {
    // EXACT PATCH 19 (LLP 1074): a root is a block-level box in the space it is offered, whatever
    // it lays its children out as (CSS 2.1 §10.3.3).
    // - Its size styles go through the ratio (`compute::ratio`).
    // - An automatic width fills the offer less its margins, where auto margins are zero.
    // - Its margins place it: auto margins take the space a given width leaves (equal shares;
    //   a negative share goes to the end margin alone), and an over-constrained box ignores its
    //   end margin.
    // An absolutely positioned root uses the shared absolute solver in a definite offer.
    use crate::compute::ratio::sizes_through_ratio;
    use crate::{BoxSizing, Position};

    let parent_size = available_space.into_options();
    let style = tree.get_core_container_style(root);
    let in_flow = style.position() != Position::Absolute;
    if !in_flow && style.box_generation_mode() != crate::BoxGenerationMode::None {
        if let Size { width: Some(width), height: Some(height) } = parent_size {
            let direction = style.direction();
            drop(style);
            common::absolute::AbsolutePass {
                area_size: Size { width, height },
                area_offset: Point::ZERO,
                direction,
                sizing_mode: SizingMode::InherentSize,
                #[cfg(feature = "content_size")]
                is_scroll_container: false,
            }.place(tree, root, crate::tree::StaticPosition::point(Point::ZERO, false, 0), None, Point::ZERO);
            return;
        }
    }

    let is_rtl = style.direction().is_rtl();
    let margin = style.margin().map(|margin| margin.resolve_to_option(parent_size.width.unwrap_or(0.0), |val, basis| tree.calc(val, basis)));
    let non_auto_margin = margin.map(|m| m.unwrap_or(0.0));
    let padding = style.padding().resolve_or_zero(parent_size.width, |val, basis| tree.calc(val, basis));
    let border = style.border().resolve_or_zero(parent_size.width, |val, basis| tree.calc(val, basis));
    let padding_border_size = (padding + border).sum_axes();
    let box_sizing_adjustment =
        if style.box_sizing() == BoxSizing::ContentBox { padding_border_size } else { Size::ZERO };
    let scrollbar_size = Size {
        width: if style.overflow().y == Overflow::Scroll { style.scrollbar_width() } else { 0.0 },
        height: if style.overflow().x == Overflow::Scroll { style.scrollbar_width() } else { 0.0 },
    };

    let (style_size, min_size, max_size) = sizes_through_ratio(
        &style,
        padding_border_size,
        style.size().maybe_resolve(parent_size, |val, basis| tree.calc(val, basis)).maybe_add(box_sizing_adjustment),
        style.min_size().maybe_resolve(parent_size, |val, basis| tree.calc(val, basis)).maybe_add(box_sizing_adjustment),
        style.max_size().maybe_resolve(parent_size, |val, basis| tree.calc(val, basis)).maybe_add(box_sizing_adjustment),
    );
    // An automatic width fills, within the limits the ratio transferred; the ratio then gives the
    // height from it.
    // HTML form controls with an automatic inline size shrink to fit even
    // when they are the document's one root child. Exact carries that
    // distinction on Taffy's existing table marker (patch 21), as block
    // child layout does below the root.
    let fills = in_flow
        && !style.is_table_item()
        && style_size.width.is_none()
        && !style.size().width.is_sizing_keyword();
    let fill_width = if fills && style_size.height.is_none() {
        parent_size.width.maybe_sub(non_auto_margin.horizontal_axis_sum()).maybe_clamp(min_size.width, max_size.width)
    } else {
        None
    };
    let (style_size, min_size, max_size) = sizes_through_ratio(
        &style,
        padding_border_size,
        Size { width: style_size.width.or(fill_width), height: style_size.height },
        min_size,
        max_size,
    );
    let fill_width = if fills {
        parent_size.width.maybe_sub(non_auto_margin.horizontal_axis_sum()).maybe_clamp(min_size.width, max_size.width)
    } else {
        None
    };
    drop(style);

    // If both min and max in a given axis are set and max <= min then this determines the size in that axis
    let min_max_definite_size = min_size.zip_map(max_size, |min, max| match (min, max) {
        (Some(min), Some(max)) if max <= min => Some(min),
        _ => None,
    });
    let known_dimensions = if in_flow {
        min_max_definite_size
            .or(style_size.maybe_clamp(min_size, max_size))
            .or(Size { width: fill_width, height: None })
            .maybe_max(padding_border_size)
    } else {
        Size::NONE
    };

    let ratio_min = crate::compute::ratio::minimum_ratio_width(tree, root, parent_size);
    let known_dimensions = known_dimensions.map_width(|width| width.maybe_max(ratio_min));

    // Recursively compute node layout
    let mut output = tree.perform_child_layout(
        root,
        known_dimensions,
        parent_size,
        available_space,
        SizingMode::InherentSize,
        Line::FALSE,
    );
    // A height the ratio derived is a floor its content can pass (patch 12): a
    // flex or grid root that came out shorter is laid out again at the floor,
    // so its items stretch to it as a block root's content is clamped to it.
    if let (None, Some(floor)) = (known_dimensions.height, min_size.height) {
        if in_flow && output.size.height < floor {
            output = tree.perform_child_layout(
                root,
                Size { width: Some(output.size.width), height: Some(floor) },
                parent_size,
                available_space,
                SizingMode::InherentSize,
                Line::FALSE,
            );
        }
    }

    let margin = match parent_size.width {
        Some(available_width) if in_flow => {
            let free_space = available_width - output.size.width - non_auto_margin.horizontal_axis_sum();
            let (left, right) = match (margin.left, margin.right) {
                (None, None) if free_space < 0.0 => {
                    if is_rtl {
                        (free_space, 0.0)
                    } else {
                        (0.0, free_space)
                    }
                }
                (None, None) => (free_space / 2.0, free_space / 2.0),
                // Over-constrained: an auto margin is zero, and the end margin gives way.
                (None, Some(right)) => (f32_max(free_space, 0.0), right),
                (Some(left), None) => (left, f32_max(free_space, 0.0)),
                (Some(left), Some(right)) => (left, right),
            };
            crate::geometry::Rect { left, right, ..non_auto_margin }
        }
        _ => non_auto_margin,
    };
    let location = if in_flow {
        Point {
            x: match parent_size.width {
                Some(available_width) if is_rtl => available_width - output.size.width - margin.right,
                _ => margin.left,
            },
            y: margin.top,
        }
    } else {
        Point::ZERO
    };

    tree.set_unrounded_layout(
        root,
        &Layout {
            order: 0,
            location,
            size: output.size,
            #[cfg(feature = "content_size")]
            scrollable_overflow_rect: output.scrollable_overflow_rect,
            scrollbar_size,
            padding,
            border,
            margin,
        },
    );
}

/// Attempts to find a cached layout for the specified node and layout inputs.
///
/// Uses the provided closure to compute the layout (and then stores the result in the cache) if no cached layout is found.
#[inline(always)]
pub fn compute_cached_layout<Tree: CacheTree + ?Sized, ComputeFunction>(
    tree: &mut Tree,
    node: NodeId,
    inputs: LayoutInput,
    compute_uncached: ComputeFunction,
) -> LayoutOutput
where
    ComputeFunction: FnOnce(&mut Tree, NodeId, LayoutInput) -> LayoutOutput,
{
    debug_push_node!(node);

    // First we check if we have a cached result for the given input
    let cache_entry = tree.cache_get(node, &inputs);
    if let Some(cached_size_and_baselines) = cache_entry {
        debug_log_node!(inputs);
        debug_log!("RESULT (CACHED)", dbg:cached_size_and_baselines.size);
        debug_pop_node!();
        return cached_size_and_baselines;
    }

    debug_log_node!(inputs);

    let computed_size_and_baselines = compute_uncached(tree, node, inputs);

    // Cache result
    tree.cache_store(node, &inputs, computed_size_and_baselines);

    debug_log!("RESULT", dbg:computed_size_and_baselines.size);
    debug_pop_node!();

    computed_size_and_baselines
}

/// Rounds the calculated layout to exact pixel values
///
/// In order to ensure that no gaps in the layout are introduced we:
///   - Always round based on the cumulative x/y coordinates (relative to the viewport) rather than
///     parent-relative coordinates
///   - Compute width/height by first rounding the top/bottom/left/right and then computing the difference
///     rather than rounding the width/height directly
///
/// See <https://github.com/facebook/yoga/commit/aa5b296ac78f7a22e1aeaf4891243c6bb76488e2> for more context
///
/// In order to prevent innacuracies caused by rounding already-rounded values, we read from `unrounded_layout`
/// and write to `final_layout`.
pub fn round_layout(tree: &mut impl RoundTree, node_id: NodeId) {
    return round_layout_inner(tree, node_id, 0.0, 0.0);

    /// Recursive function to apply rounding to all descendents
    fn round_layout_inner(tree: &mut impl RoundTree, node_id: NodeId, cumulative_x: f32, cumulative_y: f32) {
        let unrounded_layout = tree.get_unrounded_layout(node_id);
        let mut layout = unrounded_layout;

        let cumulative_x = cumulative_x + unrounded_layout.location.x;
        let cumulative_y = cumulative_y + unrounded_layout.location.y;

        layout.location.x = round(unrounded_layout.location.x);
        layout.location.y = round(unrounded_layout.location.y);
        layout.size.width = round(cumulative_x + unrounded_layout.size.width) - round(cumulative_x);
        layout.size.height = round(cumulative_y + unrounded_layout.size.height) - round(cumulative_y);
        layout.scrollbar_size.width = round(unrounded_layout.scrollbar_size.width);
        layout.scrollbar_size.height = round(unrounded_layout.scrollbar_size.height);
        layout.border.left = round(cumulative_x + unrounded_layout.border.left) - round(cumulative_x);
        layout.border.right = round(cumulative_x + unrounded_layout.size.width)
            - round(cumulative_x + unrounded_layout.size.width - unrounded_layout.border.right);
        layout.border.top = round(cumulative_y + unrounded_layout.border.top) - round(cumulative_y);
        layout.border.bottom = round(cumulative_y + unrounded_layout.size.height)
            - round(cumulative_y + unrounded_layout.size.height - unrounded_layout.border.bottom);
        layout.padding.left = round(cumulative_x + unrounded_layout.padding.left) - round(cumulative_x);
        layout.padding.right = round(cumulative_x + unrounded_layout.size.width)
            - round(cumulative_x + unrounded_layout.size.width - unrounded_layout.padding.right);
        layout.padding.top = round(cumulative_y + unrounded_layout.padding.top) - round(cumulative_y);
        layout.padding.bottom = round(cumulative_y + unrounded_layout.size.height)
            - round(cumulative_y + unrounded_layout.size.height - unrounded_layout.padding.bottom);

        #[cfg(feature = "content_size")]
        round_scrollable_overflow_rect(
            &mut layout,
            unrounded_layout.scrollable_overflow_rect,
            cumulative_x,
            cumulative_y,
        );

        tree.set_final_layout(node_id, &layout);

        let child_count = tree.child_count(node_id);
        for index in 0..child_count {
            let child = tree.get_child_id(node_id, index);
            round_layout_inner(tree, child, cumulative_x, cumulative_y);
        }
    }

    #[cfg(feature = "content_size")]
    #[inline(always)]
    /// Round the scrollable overflow rect.
    /// This is split into a separate function to make it easier to feature flag.
    fn round_scrollable_overflow_rect(
        layout: &mut Layout,
        unrounded_rect: crate::geometry::Rect<f32>,
        cumulative_x: f32,
        cumulative_y: f32,
    ) {
        layout.scrollable_overflow_rect.left = round(cumulative_x + unrounded_rect.left) - round(cumulative_x);
        layout.scrollable_overflow_rect.right = round(cumulative_x + unrounded_rect.right) - round(cumulative_x);
        layout.scrollable_overflow_rect.top = round(cumulative_y + unrounded_rect.top) - round(cumulative_y);
        layout.scrollable_overflow_rect.bottom = round(cumulative_y + unrounded_rect.bottom) - round(cumulative_y);
    }
}

/// Creates a layout for this node and its children, recursively.
/// Each hidden node has zero size and is placed at the origin
pub fn compute_hidden_layout(tree: &mut (impl LayoutPartialTree + CacheTree), node: NodeId) -> LayoutOutput {
    // Clear cache and set zeroed-out layout for the node
    tree.cache_clear(node);
    tree.set_unrounded_layout(node, &Layout::with_order(0));

    // Perform hidden layout on all children
    for index in 0..tree.child_count(node) {
        let child_id = tree.get_child_id(node, index);
        tree.compute_child_layout(child_id, LayoutInput::HIDDEN);
    }

    LayoutOutput::HIDDEN
}

/// A module for unified re-exports of detailed layout info structs, used by low level API
#[cfg(feature = "detailed_layout_info")]
pub mod detailed_info {
    #[cfg(feature = "grid")]
    pub use super::grid::{
        DetailedGridInfo, DetailedGridItemsInfo, DetailedGridTracksInfo, GridLineNames, GridLineNamesIter,
    };
}

#[cfg(test)]
mod tests {
    use super::compute_hidden_layout;
    use crate::geometry::{Point, Size};
    use crate::style::{Display, Style};
    use crate::TaffyTree;

    #[test]
    fn hidden_layout_should_hide_recursively() {
        let mut taffy: TaffyTree<()> = TaffyTree::new();

        let style: Style = Style { display: Display::Flex, size: Size::from_lengths(50.0, 50.0), ..Default::default() };

        let grandchild_00 = taffy.new_leaf(style.clone()).unwrap();
        let grandchild_01 = taffy.new_leaf(style.clone()).unwrap();
        let child_00 = taffy.new_with_children(style.clone(), &[grandchild_00, grandchild_01]).unwrap();

        let grandchild_02 = taffy.new_leaf(style.clone()).unwrap();
        let child_01 = taffy.new_with_children(style.clone(), &[grandchild_02]).unwrap();

        let root = taffy
            .new_with_children(
                Style { display: Display::None, size: Size::from_lengths(50.0, 50.0), ..Default::default() },
                &[child_00, child_01],
            )
            .unwrap();

        compute_hidden_layout(&mut taffy.as_layout_tree(), root);

        // Whatever size and display-mode the nodes had previously,
        // all layouts should resolve to ZERO due to the root's DISPLAY::NONE

        for node in [root, child_00, child_01, grandchild_00, grandchild_01, grandchild_02] {
            let layout = taffy.layout(node).unwrap();
            assert_eq!(layout.size, Size::zero());
            assert_eq!(layout.location, Point::zero());
        }
    }
}
