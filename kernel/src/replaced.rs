//! A replaced element's measure: an image, video, `svg`, canvas or iframe is
//! sized from its natural size, else CSS Images 3 §5's default object size,
//! never from children.
//!
//! Taffy applies the ratio (natural, or authored `aspect-ratio`) to a known
//! dimension and clamps (patches 5 and 12); this answers only what the
//! element itself contributes. Literal Chrome cases: `tests/it/browser_ratio.rs`
//! (images) and `tests/it/browser_replaced.rs` (the default object size).

use crate::arena::NodeArena;
use crate::generated::NodeType;
use taffy::geometry::{Rect, Size};
use taffy::style::{AvailableSpace, BoxSizing};

/// The content size of `slot` given what is `known` and the `space` offered,
/// or `None` when it is not a replaced element. `inset` is its padding plus
/// border.
pub(crate) fn measure(
    arena: &NodeArena,
    slot: u32,
    style: &taffy::Style,
    inset: Rect<f32>,
    known: Size<Option<f32>>,
    space: Size<AvailableSpace>,
) -> Option<Size<f32>> {
    let node_type = arena.node_type(slot);
    if !node_type.is_replaced() {
        return None;
    }
    let natural = arena.intrinsic(slot);
    let default = node_type.default_object_size();
    // What the ratio relates: the content box, or the border box
    // (box-sizing, unless `auto <ratio>`).
    let (pw, ph) = if style.aspect_ratio_content_box || style.box_sizing == BoxSizing::ContentBox {
        (0.0, 0.0)
    } else {
        (inset.horizontal_axis_sum(), inset.vertical_axis_sum())
    };
    // A ratio without a natural size: an `svg`'s view box, or
    // `aspect-ratio` on an iframe, a video before its metadata or an `svg`
    // without a view box.
    let unsized_ratio = match (natural, default) {
        (Some(_), _) | (None, Some((_, true))) => None,
        // The style's ratio: the view box's under `aspect-ratio: auto`, else
        // the authored one.
        _ => style
            .aspect_ratio
            .filter(|_| default.is_some() || crate::svg::natural_ratio(arena, slot).is_some()),
    };
    if let Some(ratio) = unsized_ratio {
        // @ref LLP 1055.000 D4 — a replaced element with a ratio and no
        // natural size fills the offered width (CSS Sizing's stretch fit, as
        // Chrome sizes one), else the 300 px default object width.
        let width = known
            .width
            .or(known.height.map(|h| (h + ph) * ratio - pw))
            .unwrap_or(match space.width {
                AvailableSpace::Definite(w) => w,
                _ => 300.0,
            });
        return Some(Size {
            width,
            height: known.height.unwrap_or(((width + pw) / ratio - ph).max(0.0)),
        });
    }
    // The natural size, else the default object size; nothing at all
    // before an image loads, as a broken `<img>` is 0×0.
    let Some((iw, ih)) = natural.or(default.map(|(size, _)| size)) else {
        return Some(Size::ZERO);
    };
    // An `svg` has no natural size at all, only the default object size:
    // nothing floors its min-content width (a flex row shrinks it; an
    // iframe or video it does not).
    if node_type == NodeType::Svg
        && natural.is_none()
        && known.width.is_none()
        && space.width == AvailableSpace::MinContent
    {
        return Some(Size {
            width: 0.0,
            height: known.height.unwrap_or(ih),
        });
    }
    // With neither dimension known, an authored ratio takes the natural
    // width's height (CSS Sizing 4 §5.1), as Chrome sizes a canvas.
    let ih = match (known.width, known.height, style.aspect_ratio) {
        (None, None, Some(r)) => ((iw + pw) / r - ph).max(0.0),
        _ => ih,
    };
    Some(Size {
        width: known.width.unwrap_or(iw),
        height: known.height.unwrap_or(ih),
    })
}
