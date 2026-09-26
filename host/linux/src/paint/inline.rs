//! A paragraph's paint-only run data in canonical run order: colour and
//! source (the palette), and inline backgrounds (LLP 1053 §0).
use super::rgba;
use crate::text::RunPaint;
use exact_kernel::{Kernel, NodeRef, NodeType, PropId, StyleId};

/// Mirror the canonical run ownership (own text suppresses descendants),
/// retaining paint-only information without adding it to the metric ABI.
pub(super) fn text_palette(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    dark: bool,
    out: &mut Vec<RunPaint>,
) {
    if node.props.str(PropId::Text).is_some() {
        out.push(RunPaint {
            color: rgba(node.text_color().resolve(dark)),
            source: node.id,
        });
    } else {
        for child in node.children() {
            if let Some(child) = kernel.node(child).filter(|c| c.node_type == NodeType::Text) {
                text_palette(kernel, &child, dark, out);
            }
        }
    }
}

/// Each canonical run's inline `background-color`, in [`text_palette`]'s
/// order: its own, else its nearest inline ancestor's (CSS: a container's
/// background covers its descendants' fragments). The paragraph's own
/// background is its box's, painted with the box.
pub(super) fn text_backgrounds(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    inherited: Option<[u8; 4]>,
    dark: bool,
    out: &mut Vec<Option<[u8; 4]>>,
) {
    let own = |n: &NodeRef<'_>| {
        n.style
            .mask
            .has(StyleId::BackgroundColor)
            .then(|| rgba(n.style.background_color.resolve(dark)))
            .filter(|c| c[3] != 0)
    };
    if node.props.str(PropId::Text).is_some() {
        out.push(inherited);
    } else {
        for child in node.children() {
            if let Some(child) = kernel.node(child).filter(|c| c.node_type == NodeType::Text) {
                let background = own(&child).or(inherited);
                text_backgrounds(kernel, &child, background, dark, out);
            }
        }
    }
}
