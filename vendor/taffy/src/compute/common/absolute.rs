//! EXACT PATCH 18 (LLP 1074): one solver for an absolutely positioned box,
//! shared by the block, flex and grid algorithms, as CSS 2.1 §10.3.7 and
//! §10.6.4 and Chrome size and place it.
//!
//! - The sizes go through `compute::ratio` (patch 12), as an in-flow box's do.
//!   With a ratio and neither dimension given, the inline size comes first
//!   (from both insets, else shrink-to-fit) and the ratio gives the block size.
//! - With both insets in an axis, `inset + margin + size + margin + inset` is
//!   the containing block's size: auto margins take the space left (equal
//!   shares; in the inline axis a negative share goes to the end margin alone),
//!   and an over-constrained box ignores its end inset.
//! - With one inset or none, an auto margin is zero. With none, the box sits
//!   at its static position, which the calling algorithm supplies.
//! - A box without a width is measured in the space its insets and margins
//!   leave (shrink-to-fit).
use crate::compute::common::sizing_keyword::resolve_absolute_sizing_keywords;
use crate::compute::ratio::{floors_height, Ratio};
use crate::geometry::{AbsoluteAxis, Line, Point, Rect, Size};
use crate::style::{AvailableSpace, BoxSizing, CoreStyle, Dimension, Direction, Overflow};
#[cfg(feature = "content_size")]
use crate::compute::common::scrollable_overflow::compute_scrollable_overflow_contribution;
use crate::tree::{
    Layout, LayoutOutput, LayoutPartialTree, LayoutPartialTreeExt, NodeId, SizingMode, StaticAlignment, StaticPosition,
};
use crate::util::sys::f32_max;
use crate::util::{MaybeMath, MaybeResolve, ResolveOrZero};
use crate::BoxGenerationMode;

/// An absolutely positioned box's styles, resolved against its containing
/// block (`area_size`).
struct AbsoluteBox {
    /// Margins; `None` is `auto`.
    margin: Rect<Option<f32>>,
    /// Padding.
    padding: Rect<f32>,
    /// Border widths.
    border: Rect<f32>,
    /// Insets; `None` is `auto`.
    inset: Rect<Option<f32>>,
    /// The authored size, for its sizing keywords.
    size_style: Size<Dimension>,
    /// The border-box size, min and max the styles give, before the ratio.
    size: Size<Option<f32>>,
    /// See `size`.
    min_size: Size<Option<f32>>,
    /// See `size`.
    max_size: Size<Option<f32>>,
    /// Padding + border of each axis.
    padding_border_sum: Size<f32>,
    /// The preferred ratio, if usable.
    ratio: Option<Ratio>,
    /// Whether a height the ratio derives is a floor content can pass.
    floors_height: bool,
    /// A replaced element's insets place it, never size it (patch 17).
    is_replaced: bool,
    /// `overflow`.
    overflow: Point<Overflow>,
    /// `contain`.
    #[cfg(feature = "content_size")]
    contain: crate::style::Contain,
    /// `scrollbar-width`.
    scrollbar_width: f32,
}

/// Where the solver put a box: its border box in the container's coordinates,
/// and its layout's output.
struct Placed {
    /// The border box's origin.
    #[cfg_attr(not(feature = "content_size"), allow(dead_code))]
    location: Point<f32>,
    /// The border box's size.
    #[cfg_attr(not(feature = "content_size"), allow(dead_code))]
    size: Size<f32>,
    /// The output of the box's own layout.
    #[cfg_attr(not(feature = "content_size"), allow(dead_code))]
    output: LayoutOutput,
}

/// EXACT PATCH 20 (LLP 1074 T1): a container's pass over the absolutely
/// positioned boxes it is the containing block of. Its own children are
/// given by its algorithm, each with its [`StaticPosition`]; the descendants
/// whose parents are `Static` come from the tree
/// ([`LayoutPartialTree::hoisted_absolute`]).
pub(crate) struct AbsolutePass {
    /// The container's padding box less its scrollbar gutter: the containing block.
    pub(crate) area_size: Size<f32>,
    /// That box's origin in the container's border-box coordinates.
    pub(crate) area_offset: Point<f32>,
    /// The container's direction.
    pub(crate) direction: Direction,
    /// How the boxes are measured.
    pub(crate) sizing_mode: SizingMode,
    /// Whether the container scrolls, for the overflow its boxes add.
    #[cfg(feature = "content_size")]
    pub(crate) is_scroll_container: bool,
}

impl AbsolutePass {
    /// Sizes, lays out and places one box, and returns what it adds to the
    /// container's scrollable overflow. `area` is the box's containing block
    /// where that is not the whole padding box (a grid area). `origin` is the
    /// origin of the box's parent in the container's coordinates.
    pub(crate) fn place(
        &self,
        tree: &mut impl LayoutPartialTree,
        node: NodeId,
        position: StaticPosition,
        area: Option<(Size<f32>, Point<f32>)>,
        origin: Point<f32>,
    ) -> Rect<f32> {
        let (area_size, area_offset) = area.unwrap_or((self.area_size, self.area_offset));
        let style = tree.get_core_container_style(node);
        let Some(absolute) = AbsoluteBox::of(&style, area_size, |val, basis| tree.calc(val, basis)) else {
            return Rect::ZERO;
        };
        drop(style);
        #[cfg(feature = "content_size")]
        let (overflow, contain) = (absolute.overflow, absolute.contain);
        #[cfg_attr(not(feature = "content_size"), allow(unused_variables))]
        // CSS 2.1 §10.3.7: the inset is the static position before the box is sized, so the
        // space runs from the static edge to the containing block's far edge; for a centred
        // position, twice the distance to the nearer edge (as Chrome measures it).
        let static_inline_space = match position.align.x {
            StaticAlignment::Start => area_size.width - (position.rect.left + origin.x - area_offset.x),
            StaticAlignment::End => position.rect.right + origin.x - area_offset.x,
            StaticAlignment::Center => {
                let centre = (position.rect.left + position.rect.right) / 2.0 + origin.x - area_offset.x;
                2.0 * crate::util::sys::f32_min(centre, area_size.width - centre)
            }
        };
        let placed = absolute.layout(
            tree,
            node,
            position.order,
            area_size,
            area_offset,
            self.direction,
            self.sizing_mode,
            origin,
            static_inline_space,
            |size, margin| {
                let at = position.resolve(size, margin);
                Point { x: at.x + origin.x, y: at.y + origin.y }
            },
        );

        #[cfg(feature = "content_size")]
        {
            // Measured from the scroll origin (the inline-start edge: the right side in RTL).
            let from_area =
                Point { x: placed.location.x - self.area_offset.x, y: placed.location.y - self.area_offset.y };
            let location = if self.direction.is_rtl() {
                Point { x: self.area_size.width - from_area.x - placed.size.width, y: from_area.y }
            } else {
                from_area
            };
            compute_scrollable_overflow_contribution(
                location,
                placed.size,
                placed.output.scrollable_overflow_rect,
                overflow,
                contain,
                self.is_scroll_container,
            )
        }
        #[cfg(not(feature = "content_size"))]
        Rect::ZERO
    }

    /// Places the descendants `node` contains that are not its children.
    pub(crate) fn place_hoisted(&self, tree: &mut impl LayoutPartialTree, node: NodeId) -> Rect<f32> {
        let mut overflow = Rect::ZERO;
        for index in 0..tree.hoisted_absolute_count(node) {
            if let Some((child, position, origin)) = tree.hoisted_absolute(node, index) {
                overflow = overflow.union(self.place(tree, child, position, None, origin));
            }
        }
        overflow
    }
}

impl AbsoluteBox {
    /// The styles of a child that generates a box, resolved against `area_size`.
    /// `None` for `display: none`.
    fn of(
        style: &impl CoreStyle,
        area_size: Size<f32>,
        calc: impl Fn(*const (), f32) -> f32 + Copy,
    ) -> Option<Self> {
        if style.box_generation_mode() == BoxGenerationMode::None {
            return None;
        }
        let padding = style.padding().resolve_or_zero(Some(area_size.width), calc);
        let border = style.border().resolve_or_zero(Some(area_size.width), calc);
        let padding_border_sum = (padding + border).sum_axes();
        let box_sizing_adjustment =
            if style.box_sizing() == BoxSizing::ContentBox { padding_border_sum } else { Size::ZERO };
        let inset = style.inset();
        let min_size = style.min_size().maybe_resolve(area_size, calc).maybe_add(box_sizing_adjustment);
        Some(Self {
            margin: style.margin().map(|margin| margin.resolve_to_option(area_size.width, calc)),
            padding,
            border,
            inset: Rect {
                left: inset.left.maybe_resolve(area_size.width, calc),
                right: inset.right.maybe_resolve(area_size.width, calc),
                top: inset.top.maybe_resolve(area_size.height, calc),
                bottom: inset.bottom.maybe_resolve(area_size.height, calc),
            },
            size_style: style.size(),
            size: style.size().maybe_resolve(area_size, calc).maybe_add(box_sizing_adjustment),
            min_size,
            max_size: style.max_size().maybe_resolve(area_size, calc).maybe_add(box_sizing_adjustment),
            padding_border_sum,
            ratio: Ratio::of(style, padding_border_sum),
            floors_height: floors_height(style, min_size.height),
            is_replaced: style.is_compressible_replaced(),
            overflow: style.overflow(),
            #[cfg(feature = "content_size")]
            contain: style.contain(),
            scrollbar_width: style.scrollbar_width(),
        })
    }

    /// Sizes the box, lays it out, places it in its containing block (`area_size`
    /// at `area_offset` in the container's coordinates) and writes its layout.
    ///
    /// `static_position` gives the border box's origin, in the container's
    /// coordinates, for an axis with neither inset: it is given the box's size
    /// and its margins with `auto` as zero. `origin` is the origin of the box's
    /// parent in the container's coordinates (zero for the container's own
    /// child): a layout's location is relative to the parent.
    #[allow(clippy::too_many_arguments)]
    fn layout(
        mut self,
        tree: &mut impl LayoutPartialTree,
        node: NodeId,
        order: u32,
        area_size: Size<f32>,
        area_offset: Point<f32>,
        direction: Direction,
        sizing_mode: SizingMode,
        origin: Point<f32>,
        static_inline_space: f32,
        static_position: impl FnOnce(Size<f32>, Rect<f32>) -> Point<f32>,
    ) -> Placed {
        let Self { margin, inset, .. } = self;
        let non_auto_margin = margin.map(|m| m.unwrap_or(0.0));
        // The space the insets and margins leave: what an auto size fills with both insets, and
        // what a box without them is measured in.
        let inset_space = Size {
            width: if inset.left.is_none() && inset.right.is_none() { static_inline_space } else { area_size.width }
                - inset.left.unwrap_or(0.0)
                - inset.right.unwrap_or(0.0)
                - non_auto_margin.horizontal_axis_sum(),
            height: area_size.height
                - inset.top.unwrap_or(0.0)
                - inset.bottom.unwrap_or(0.0)
                - non_auto_margin.vertical_axis_sum(),
        }
        .map(|space| f32_max(space, 0.0));
        let between_insets = Size {
            width: (!self.is_replaced && inset.left.is_some() && inset.right.is_some()).then_some(inset_space.width),
            height: (!self.is_replaced && inset.top.is_some() && inset.bottom.is_some())
                .then_some(inset_space.height),
        };
        let parent_size = area_size.map(Some);

        if let Some(min) = crate::compute::ratio::minimum_ratio_width(tree, node, parent_size) {
            self.min_size.width = Some(min).maybe_max(self.min_size.width);
        }
        let mut given = self.size;
        if self.size_style.width.is_sizing_keyword() || self.size_style.height.is_sizing_keyword() {
            resolve_absolute_sizing_keywords(
                tree,
                node,
                &mut given,
                self.size_style,
                area_size,
                inset,
                margin,
                sizing_mode,
            );
        }

        // CSS shrink-to-fit clamps available width between the intrinsic
        // widths; a wrapped leaf's measured ink width is not its used width.
        if given.width.is_none()
            && (self.ratio.is_none() || (given.height.is_none() && between_insets.height.is_none()))
            && between_insets.width.is_none() && !self.is_replaced {
            given.width = Some(super::fit_content_width(
                tree, node, Size::NONE, Size { width: true, height: true },
                parent_size, inset_space.width, sizing_mode,
            ));
        }

        let (size, min_size, max_size) = match self.ratio {
            None => (given.or(between_insets), self.min_size, self.max_size),
            Some(ratio) => {
                if given.width.is_none() && given.height.is_none() {
                    // An automatic size takes the limits the ratio transfers from the other axis.
                    let (_, min, max) = ratio.resolve(given, self.min_size, self.max_size, false);
                    if let Some(width) = between_insets.width {
                        given.width = Some(width.maybe_clamp(min.width, max.width));
                    } else if let Some(height) = between_insets.height {
                        given.height = Some(height.maybe_clamp(min.height, max.height));
                    } else if !self.is_replaced {
                        // Shrink-to-fit inline size first; the ratio gives the block size from it.
                        let width = tree.measure_child_size(
                            node,
                            Size::NONE,
                            parent_size,
                            Size {
                                width: AvailableSpace::Definite(inset_space.width.maybe_clamp(min.width, max.width)),
                                height: AvailableSpace::Definite(inset_space.height),
                            },
                            sizing_mode,
                            AbsoluteAxis::Horizontal,
                            Line::FALSE,
                        );
                        given.width = Some(width.maybe_clamp(min.width, max.width));
                    }
                }
                ratio.resolve(given, self.min_size, self.max_size, self.floors_height)
            }
        };
        let min_size = min_size.or(self.padding_border_sum.map(Some)).maybe_max(self.padding_border_sum);
        let known_dimensions = size.maybe_clamp(min_size, max_size);
        let available_space = Size {
            width: AvailableSpace::Definite(inset_space.width.maybe_clamp(min_size.width, max_size.width)),
            height: AvailableSpace::Definite(inset_space.height.maybe_clamp(min_size.height, max_size.height)),
        };

        let final_size = match (known_dimensions.width, known_dimensions.height) {
            (Some(width), Some(height)) => Size { width, height },
            _ => {
                let measured_size = tree.measure_child_size_both(
                    node,
                    known_dimensions,
                    parent_size,
                    available_space,
                    sizing_mode,
                    Line::FALSE,
                );
                known_dimensions.unwrap_or(measured_size)
            }
        }
        .maybe_clamp(min_size, max_size);

        let output = tree.perform_child_layout(
            node,
            final_size.map(Some),
            parent_size,
            available_space,
            sizing_mode,
            Line::FALSE,
        );

        let x = solve_axis(
            area_size.width,
            Line { start: inset.left, end: inset.right },
            final_size.width,
            Line { start: margin.left, end: margin.right },
            true,
            direction.is_rtl(),
        );
        let y = solve_axis(
            area_size.height,
            Line { start: inset.top, end: inset.bottom },
            final_size.height,
            Line { start: margin.top, end: margin.bottom },
            false,
            false,
        );
        let static_position =
            if x.is_none() || y.is_none() { static_position(final_size, non_auto_margin) } else { Point::ZERO };
        let (x, x_margin) = match x {
            Some((offset, margin)) => (area_offset.x + offset, margin),
            None => (static_position.x, Line { start: non_auto_margin.left, end: non_auto_margin.right }),
        };
        let (y, y_margin) = match y {
            Some((offset, margin)) => (area_offset.y + offset, margin),
            None => (static_position.y, Line { start: non_auto_margin.top, end: non_auto_margin.bottom }),
        };
        let location = Point { x, y };

        // Note: axis intentionally switched here as scrollbars take up space in the opposite axis
        // to the axis in which scrolling is enabled.
        let scrollbar_size = Size {
            width: if self.overflow.y == Overflow::Scroll { self.scrollbar_width } else { 0.0 },
            height: if self.overflow.x == Overflow::Scroll { self.scrollbar_width } else { 0.0 },
        };
        tree.set_unrounded_layout(
            node,
            &Layout {
                order,
                size: final_size,
                #[cfg(feature = "content_size")]
                scrollable_overflow_rect: output.scrollable_overflow_rect,
                scrollbar_size,
                location: Point { x: location.x - origin.x, y: location.y - origin.y },
                padding: self.padding,
                border: self.border,
                margin: Rect { left: x_margin.start, right: x_margin.end, top: y_margin.start, bottom: y_margin.end },
            },
        );

        Placed { location, size: final_size, output }
    }
}

/// One axis of CSS 2.1 §10.3.7 / §10.6.4 for a box whose size is known: its
/// offset from the containing block's start edge and its used margins, or
/// `None` when neither inset is set (the static position).
fn solve_axis(
    area: f32,
    inset: Line<Option<f32>>,
    size: f32,
    margin: Line<Option<f32>>,
    inline: bool,
    rtl: bool,
) -> Option<(f32, Line<f32>)> {
    let non_auto = Line { start: margin.start.unwrap_or(0.0), end: margin.end.unwrap_or(0.0) };
    match (inset.start, inset.end) {
        (Some(start), Some(end)) => {
            let free_space = area - start - end - size - non_auto.start - non_auto.end;
            let margin = match (margin.start, margin.end) {
                (None, None) if inline && free_space < 0.0 => {
                    if rtl {
                        Line { start: free_space, end: 0.0 }
                    } else {
                        Line { start: 0.0, end: free_space }
                    }
                }
                (None, None) => Line { start: free_space / 2.0, end: free_space / 2.0 },
                (None, Some(end)) => Line { start: free_space, end },
                (Some(start), None) => Line { start, end: free_space },
                (Some(start), Some(end)) => Line { start, end },
            };
            // Over-constrained: the end inset gives way (the start inset, in an RTL inline axis).
            let offset = if rtl { area - end - size - margin.end } else { start + margin.start };
            Some((offset, margin))
        }
        (Some(start), None) => Some((start + non_auto.start, non_auto)),
        (None, Some(end)) => Some((area - end - size - non_auto.end, non_auto)),
        (None, None) => None,
    }
}
