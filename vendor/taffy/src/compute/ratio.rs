//! EXACT PATCH 12 (LLP 1053 G1): a box's size styles through its preferred
//! aspect ratio, as CSS Box Sizing 4 §5 and Chrome size them.
//!
//! - The ratio relates the sizes of the box `box-sizing` names, except that a
//!   replaced element's natural ratio and `aspect-ratio: auto <ratio>` always
//!   relate content-box sizes.
//! - One dimension given: it is clamped by its own min/max, and the other is
//!   derived from the clamped value and clamped on its own. Two given: the
//!   ratio does nothing.
//! - Min/max transfer through the ratio only into a dimension that is not
//!   given (a block's stretched width takes a `max-height` as a max width;
//!   a set width does not).
//! - A derived height of a box that is neither replaced nor a scroll
//!   container, whose `min-height` is `auto`, is a floor its content can
//!   pass (§5.2's automatic minimum), not a fixed height.
use crate::geometry::Size;
use crate::util::sys::{f32_max, f32_min};
use crate::util::MaybeMath;
use crate::{BoxSizing, CoreStyle};

/// A usable preferred ratio and the inset between the border box and the
/// box it relates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ratio {
    /// Width ÷ height, finite and positive.
    ratio: f32,
    /// What a border-box size loses to be of the related box.
    inset: Size<f32>,
}

impl Ratio {
    /// The style's ratio, if it has a usable one. `pb_sum` is the resolved
    /// padding + border of each axis.
    pub(crate) fn of(style: &impl CoreStyle, pb_sum: Size<f32>) -> Option<Self> {
        let ratio = style.aspect_ratio().filter(|r| r.is_finite() && *r > 0.0)?;
        let inset = if style.box_sizing() == BoxSizing::ContentBox || style.aspect_ratio_content_box() {
            pb_sum
        } else {
            Size::ZERO
        };
        Some(Self { ratio, inset })
    }

    /// A ratio from a style's parts, for an algorithm that keeps the parts and
    /// not the style: `content_box` is whether the ratio relates content-box
    /// sizes (see `of`).
    pub(crate) fn from_parts(ratio: Option<f32>, content_box: bool, pb_sum: Size<f32>) -> Option<Self> {
        let ratio = ratio.filter(|r| r.is_finite() && *r > 0.0)?;
        Some(Self { ratio, inset: if content_box { pb_sum } else { Size::ZERO } })
    }

    /// The border-box height for a border-box width.
    pub(crate) fn height(self, width: f32) -> f32 {
        f32_max(width - self.inset.width, 0.0) / self.ratio + self.inset.height
    }

    /// The border-box width for a border-box height.
    pub(crate) fn width(self, height: f32) -> f32 {
        f32_max(height - self.inset.height, 0.0) * self.ratio + self.inset.width
    }

    /// The border-box size, min and max of a box with these given sizes and
    /// authored min/max (all border-box). `floor_height`: the derived height
    /// is an automatic minimum rather than a size (see the module note).
    pub(crate) fn resolve(
        self,
        given: Size<Option<f32>>,
        min: Size<Option<f32>>,
        max: Size<Option<f32>>,
        floor_height: bool,
    ) -> (Size<Option<f32>>, Size<Option<f32>>, Size<Option<f32>>) {
        let (mut min_out, mut max_out) = (min, max);
        if given.width.is_none() {
            if let Some(h) = min.height {
                min_out.width = Some(f32_max(min.width.unwrap_or(0.0), self.width(h)));
            }
            if let Some(h) = max.height {
                max_out.width = Some(max.width.map_or(self.width(h), |w| f32_min(w, self.width(h))));
            }
        }
        if given.height.is_none() {
            if let Some(w) = min.width {
                min_out.height = Some(f32_max(min.height.unwrap_or(0.0), self.height(w)));
            }
            if let Some(w) = max.width {
                max_out.height = Some(max.height.map_or(self.height(w), |h| f32_min(h, self.height(w))));
            }
        }
        let mut size = given;
        match (given.width, given.height) {
            (Some(w), None) => {
                let derived = self.height(w.maybe_clamp(min_out.width, max_out.width));
                if floor_height {
                    let floor = derived.maybe_min(max_out.height);
                    min_out.height = Some(f32_max(min_out.height.unwrap_or(0.0), floor));
                } else {
                    size.height = Some(derived);
                }
            }
            (None, Some(h)) => {
                size.width = Some(self.width(h.maybe_clamp(min_out.height, max_out.height)));
            }
            _ => {}
        }
        (size, min_out, max_out)
    }
}

/// Whether a box's derived height is a floor its content can pass: not
/// replaced, not a scroll container, `min-height: auto`.
pub(crate) fn floors_height(style: &impl CoreStyle, min_height: Option<f32>) -> bool {
    let overflow = style.overflow();
    !style.is_compressible_replaced()
        && min_height.is_none()
        && !overflow.x.is_scroll_container()
        && !overflow.y.is_scroll_container()
}

/// A style's resolved border-box size, min and max through its ratio, if
/// it has one; unchanged otherwise.
pub(crate) fn sizes_through_ratio(
    style: &impl CoreStyle,
    pb_sum: Size<f32>,
    size: Size<Option<f32>>,
    min: Size<Option<f32>>,
    max: Size<Option<f32>>,
) -> (Size<Option<f32>>, Size<Option<f32>>, Size<Option<f32>>) {
    match Ratio::of(style, pb_sum) {
        Some(ratio) => ratio.resolve(size, min, max, floors_height(style, min.height)),
        None => (size, min, max),
    }
}

/// `limits` (a min or a max) transferred through `ratio` into the axes the
/// style does not size (`sized`); a sized axis keeps its own limit only.
pub(crate) fn transfer_into_unsized(
    limits: Size<Option<f32>>,
    ratio: Option<f32>,
    sized: Size<bool>,
) -> Size<Option<f32>> {
    let transferred = limits.maybe_apply_aspect_ratio(ratio);
    Size {
        width: if sized.width { limits.width } else { transferred.width },
        height: if sized.height { limits.height } else { transferred.height },
    }
}

/// [`Ratio::resolve`] for a ratio that may be absent.
pub(crate) fn resolve_through(
    ratio: Option<Ratio>,
    size: Size<Option<f32>>,
    min: Size<Option<f32>>,
    max: Size<Option<f32>>,
    floor_height: bool,
) -> (Size<Option<f32>>, Size<Option<f32>>, Size<Option<f32>>) {
    match ratio {
        Some(ratio) => ratio.resolve(size, min, max, floor_height),
        None => (size, min, max),
    }
}

/// The ratio's preferred block size is definite for percentage resolution
/// even when the content-based automatic minimum makes the used box taller.
pub(crate) fn percentage_height(style: &impl CoreStyle, pb: Size<f32>, width: Option<f32>) -> Option<f32> {
    if !style.size().height.is_auto() {
        return None;
    }
    Some(Ratio::of(style, pb)?.height(width?))
}

/// The automatic inline minimum of a non-replaced ratio box whose width
/// derives from a definite height is its min-content width, capped by max-width.
pub(crate) fn minimum_ratio_width(
    tree: &mut impl crate::LayoutPartialTree,
    node: crate::NodeId,
    parent: Size<Option<f32>>,
) -> Option<f32> {
    use crate::{LayoutPartialTreeExt, MaybeResolve, ResolveOrZero};
    let style = tree.get_core_container_style(node);
    if style.aspect_ratio().is_none() || !style.size().width.is_auto()
        || !style.min_size().width.is_auto() || style.is_compressible_replaced()
        || style.overflow().x.is_scroll_container() || style.overflow().y.is_scroll_container()
        || style.size().height.maybe_resolve(parent.height, |v, b| tree.calc(v, b)).is_none() {
        return None;
    }
    let pb = (style.padding().resolve_or_zero(parent.width, |v, b| tree.calc(v, b))
        + style.border().resolve_or_zero(parent.width, |v, b| tree.calc(v, b))).sum_axes();
    let adjustment = if style.box_sizing() == BoxSizing::ContentBox { pb.width } else { 0.0 };
    let max = style.max_size().width.maybe_resolve(parent.width, |v, b| tree.calc(v, b)).map(|w| w + adjustment);
    drop(style);
    let intrinsic = tree.measure_child_size(
        node,
        Size::NONE,
        parent,
        Size { width: crate::AvailableSpace::MinContent, height: crate::AvailableSpace::MinContent },
        crate::SizingMode::ContentSize,
        crate::AbsoluteAxis::Horizontal,
        crate::geometry::Line::FALSE,
    );
    Some(intrinsic.maybe_min(max))
}
