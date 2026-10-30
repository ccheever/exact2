//! The host CSS of the boxes that keep their own width in block flow, where
//! a `div` would stretch: a canvas and an indeterminate progress.
use exact_kernel::NodeFacts;

/// A canvas's `div` sized as a `<canvas>` is: a replaced element whose
/// natural size is 300×150, ratio 2:1, and whose children never size it.
/// `contain: size` keeps the children out and `contain-intrinsic-size`
/// stands in for the natural size (none when a height is given, so the
/// width comes through the ratio); `justify-self: start` keeps an auto width
/// at that size in block flow, where a `div` would stretch, unless the
/// parent's `justify-items` aligns it ([`aligns`]); the natural
/// ratio holds under `aspect-ratio: auto`; an automatic minimum height is
/// its content's, as a replaced box's is, unless a height row bounds it; an
/// absolutely positioned one is its natural size whatever its insets. Chrome
/// sizes it as a `<canvas>` everywhere but a flex row's automatic minimum
/// width, which would need the parent's direction (LLP 1001 §1).
pub(super) fn canvas_css(node: &NodeFacts<'_>, parent: Option<&NodeFacts<'_>>, css: &mut String) {
    use exact_kernel::{Dimension, PositionType, StyleId};
    let style = &node.style;
    let given = |id: StyleId, value: Dimension| style.mask.has(id) && value != Dimension::Auto;
    let (width, height) = (
        given(StyleId::Width, style.width),
        given(StyleId::Height, style.height),
    );
    css.push_str("contain:size;");
    if !in_flow(node) || parent.is_none_or(|p| !aligns(p)) {
        css.push_str("justify-self:start;");
    }
    if !height {
        css.push_str("contain-intrinsic-size:300px 150px;");
    }
    if style.rare.aspect_ratio.defers_to_natural() {
        css.push_str("aspect-ratio:auto 2/1;");
    }
    if !given(StyleId::MinHeight, style.min_height) {
        css.push_str(if height || given(StyleId::MaxHeight, style.max_height) {
            "min-height:0;"
        } else {
            "min-height:min-content;"
        });
    }
    if style.position_type == PositionType::Absolute {
        if !width {
            css.push_str("width:fit-content;");
        }
        if !height {
            css.push_str("height:fit-content;");
        }
    }
}

/// A progress's box sized as the kernel's measured leaf: 20 × 20 content,
/// each axis CSS's where it is given. Size containment (`container-type:
/// size`) with a `contain-intrinsic-size` is that content, a flex item's
/// automatic minimum included; a grid, so the base sheet's ring (its
/// `::before`) fills the content box whether or not a height is given
/// (container units resolve to 0 in an automatic height). Like the kernel's
/// form controls (`item_is_table`), it keeps that width in block flow, where
/// a `div` would stretch (`justify-self: start`, as a canvas's), unless its
/// block parent's `justify-items` aligns it ([`aligns`]); it stretches in a
/// flex or grid container and between an absolute box's insets.
pub(super) fn progress_css(node: &NodeFacts<'_>, parent: Option<&NodeFacts<'_>>, css: &mut String) {
    css.push_str("container-type:size;contain-intrinsic-size:20px 20px;");
    if node.style.display != exact_kernel::Display::None {
        css.push_str("display:grid;");
    }
    let block = |p: &NodeFacts<'_>| p.style.display == exact_kernel::Display::Block;
    if in_flow(node) && parent.is_none_or(|p| block(p) && !aligns(p)) {
        css.push_str("justify-self:start;");
    }
}

/// Sticky is laid out as relative, in flow (kernel `style.rs`).
fn in_flow(node: &NodeFacts<'_>) -> bool {
    use exact_kernel::PositionType as P;
    matches!(
        node.style.position_type,
        P::Static | P::Relative | P::Sticky
    )
}

/// A parent's `justify-items` that aligns (`start`, `center`, …) aligns an in-flow child, in block
/// flow too since the kernel's 6d81878b30: a host's `justify-self: start` is not written over it.
fn aligns(parent: &NodeFacts<'_>) -> bool {
    use exact_kernel::JustifyItems as J;
    !matches!(parent.style.justify_items, J::Normal | J::Stretch)
}
