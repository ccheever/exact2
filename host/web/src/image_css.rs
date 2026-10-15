//! The replaced image's alpha template, kept in its existing element box.
use exact_kernel::{NodeFacts, NodeType, ObjectFit, PropId, StyleId};

pub(super) fn template(node: &NodeFacts<'_>, css: &mut String) {
    // LLP 1011 §3: tint is masked by the image, fitted in the content box.
    // Its picture moves out of the replaced box; only its tint remains.
    if node.node_type != NodeType::Image || !node.style.mask.has(StyleId::TintColor) {
        return;
    }
    let Some(source) = node
        .props
        .str(PropId::ImageSource)
        .filter(|s| !s.starts_with("symbol:"))
    else {
        return;
    };
    let size = match node.style.object_fit {
        ObjectFit::Fill => "100% 100%",
        ObjectFit::Contain => "contain",
        ObjectFit::Cover => "cover",
        ObjectFit::None => "auto",
        ObjectFit::ScaleDown => "var(--exact-tint-fit,contain)",
    };
    let image = format!("url({})", crate::css::css_string(source));
    css.push_str(&format!("background-color:var(--exact-tint);mask-image:{image};mask-size:{size};mask-repeat:no-repeat;mask-position:center;mask-origin:content-box;mask-clip:content-box;object-position:-100000px 0;"));
    // The separator is independent box paint. The host's image mask adds
    // its one-pixel rectangle rather than cutting the line to the picture.
    if node.props.bool(PropId::GroupedRowSeparator).is_some() {
        css.push_str(&format!(
            "--exact-grouped-image-mask:{image};--exact-grouped-image-fit:{size};"
        ));
    }
}
