//! A paragraph's paint-only run data in canonical run order: colour and
//! source (the palette), and inline backgrounds (LLP 1053 §0).
use super::rgba;
use crate::text::markup::QUIET;
use crate::text::{Paragraph, RunPaint};
use exact_kernel::{Kernel, NodeRef, NodeType, PropId, StyleId};

/// Mirror the canonical run ownership (own text suppresses descendants),
/// retaining paint-only information without adding it to the metric ABI.
pub(crate) fn text_palette(
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
            .then(|| {
                let current = || n.text_color();
                rgba(
                    n.style
                        .background_color
                        .unwrap_or_else(current)
                        .resolve(dark),
                )
            })
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

/// A text node's presented `color` while a colour animation or transition
/// runs on the node its `color` comes from (LLP 1055.000 D6: an animated
/// `color` is inherited like any computed value).
pub(super) fn presented_color(walk: &super::Walk<'_, '_>, node: &NodeRef<'_>) -> Option<[u8; 4]> {
    // The node's own animation first; else whoever it inherits `color` from.
    if node
        .style
        .animation
        .properties()
        .contains(&exact_motion::Property::Color)
    {
        return (walk.scene.presented)(node.id)
            .colors
            .color(exact_motion::Property::Color);
    }
    let source = node.source_of(StyleId::TextColor)?;
    (walk.scene.presented)(source)
        .colors
        .color(exact_motion::Property::Color)
}

/// The paragraph's run colours with presented ones applied, run by run. A
/// Markdown paragraph's one source run paints as its expanded runs, the
/// quieter roles (markers, quotes) at the web's `opacity: 0.62` (LLP 1045 D3).
pub(super) fn presented_text_colors(
    walk: &super::Walk<'_, '_>,
    node: &NodeRef<'_>,
    paragraph: &Paragraph,
    palette: &mut Vec<RunPaint>,
) {
    for run in palette.iter_mut() {
        if let Some(n) = walk.scene.kernel.node(run.source) {
            if let Some(c) = presented_color(walk, &n) {
                run.color = c;
            }
        }
    }
    if node.props.str(PropId::Markup) == Some("markdown") && palette.len() == 1 {
        let ink = palette[0];
        palette.clear();
        palette.extend(paragraph.runs().iter().map(|run| {
            let mut paint = ink;
            if run.mark & QUIET != 0 {
                paint.color[3] = (f32::from(paint.color[3]) * 0.62).round() as u8;
            }
            paint
        }));
    }
}
