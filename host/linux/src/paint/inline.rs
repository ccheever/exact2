//! A paragraph's paint-only run data in canonical run order: colour and
//! source (the palette), and inline backgrounds (LLP 1053 §0).
use super::rgba;
use crate::text::markup::QUIET;
use crate::text::{backgrounds::InlineBackground, Paragraph, RunPaint};
use exact_kernel::{Kernel, NodeRef, NodeType, PropId, StyleId};

pub(super) fn focus_fragments(
    kernel: &Kernel,
    paragraph: &Paragraph,
    palette: &[RunPaint],
    focus: exact_kernel::ViewId,
) -> Vec<super::Shape> {
    let backgrounds: Vec<_> = palette
        .iter()
        .map(|run| {
            let mut at = Some(run.source);
            while let Some(id) = at {
                if id == focus {
                    return Some(InlineBackground {
                        id: focus,
                        color: [255; 4],
                        radii: [exact_kernel::Dimension::Points(2.0); 4],
                    });
                }
                at = kernel.node(id).and_then(|n| n.parent);
            }
            None
        })
        .collect();
    paragraph
        .inline_backgrounds(&backgrounds, &kernel.env())
        .into_iter()
        .map(|(shape, _)| shape)
        .collect()
}

/// Mirror the canonical run ownership (own text suppresses descendants),
/// retaining paint-only information without adding it to the metric ABI.
pub(crate) fn text_palette(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    dark: bool,
    reveal: Option<exact_kernel::ViewId>,
    out: &mut Vec<RunPaint>,
) {
    let _accent = super::accent_scope(node);
    if node.props.str(PropId::Text).is_some() {
        // A hidden run keeps its advance and paints no glyphs. The ghost's
        // `reveal` is `revealed`, so a label the wrapper hid still draws.
        let color = if super::paints(kernel, node.id, reveal) {
            rgba(node.text_color().resolve(dark))
        } else {
            [0, 0, 0, 0]
        };
        out.push(RunPaint {
            color,
            source: node.id,
        });
    } else {
        for child in node.children() {
            let Some(child) = kernel.node(child) else {
                continue;
            };
            if child.node_type == NodeType::Text {
                text_palette(kernel, &child, dark, reveal, out);
            } else if child.inline_box().is_some() {
                // An image's run (LLP 1011.002): a box, no glyphs.
                out.push(RunPaint {
                    color: [0, 0, 0, 0],
                    source: child.id,
                });
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
    dark: bool,
    reveal: Option<exact_kernel::ViewId>,
) -> Vec<Option<InlineBackground>> {
    let mut out = Vec::new();
    let mut nested = std::collections::HashSet::new();
    collect_backgrounds(kernel, node, None, dark, reveal, &mut out, &mut nested);
    // One background per leaf cannot paint an ancestor under a descendant's
    // own background. Keep both square so their seam never exposes the page.
    for background in out.iter_mut().flatten() {
        if nested.contains(&background.id) {
            background.radii = [exact_kernel::Dimension::Points(0.0); 4];
        }
    }
    out
}

fn collect_backgrounds(
    kernel: &Kernel,
    node: &NodeRef<'_>,
    inherited: Option<InlineBackground>,
    dark: bool,
    reveal: Option<exact_kernel::ViewId>,
    out: &mut Vec<Option<InlineBackground>>,
    nested: &mut std::collections::HashSet<exact_kernel::ViewId>,
) -> bool {
    let own = |n: &NodeRef<'_>| {
        let _accent = super::accent_scope(n);
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
            .map(|color| InlineBackground {
                color,
                id: n.id,
                radii: [
                    n.style.border_radius_top_left,
                    n.style.border_radius_top_right,
                    n.style.border_radius_bottom_right,
                    n.style.border_radius_bottom_left,
                ]
                .map(|d| d.resolve(&kernel.env())),
            })
    };
    if let Some(text) = node.props.str(PropId::Text) {
        out.push(inherited);
        !text.is_empty()
    } else {
        let mut emitted = false;
        for child in node.children() {
            let Some(child) = kernel.node(child) else {
                continue;
            };
            if child.inline_box().is_some() {
                out.push(None); // an image's run paints its own box
            }
            if child.node_type == NodeType::Text {
                // A hidden box contributes no background; a visible ancestor
                // still covers its characters.
                let own = super::paints(kernel, child.id, reveal)
                    .then(|| own(&child))
                    .flatten();
                if collect_backgrounds(kernel, &child, own.or(inherited), dark, reveal, out, nested)
                {
                    emitted = true;
                    if let (Some(parent), Some(child)) = (inherited, own) {
                        nested.extend([parent.id, child.id]);
                    }
                }
            }
        }
        emitted
    }
}

/// A text node's presented `color` while a colour animation or transition
/// runs on the node its `color` comes from (LLP 1055.000 D6: an animated
/// `color` is inherited like any computed value).
pub(super) fn presented_color(
    walk: &super::Walk<'_, '_>,
    node: &NodeRef<'_>,
    dark: bool,
) -> Option<[u8; 4]> {
    let moving = moving_color(walk, node);
    match pressed_ink(walk, node, dark) {
        Some((ink, progress)) => {
            let own = moving.unwrap_or_else(|| rgba(node.text_color().resolve(dark)));
            Some(super::presented::press_mix(own, ink, progress))
        }
        None => moving,
    }
}

/// [`presented_color`] where the node's `color` stops at it (a platform
/// field's start, LLP 1104 D4): only its own motion and its own press.
pub(super) fn own_presented_color(
    walk: &super::Walk<'_, '_>,
    node: &NodeRef<'_>,
    dark: bool,
) -> Option<[u8; 4]> {
    let presented = (walk.scene.presented)(node.id);
    let moving = presented.colors.color(exact_motion::Property::Color);
    if node.style.mask.has(StyleId::PressColor) && presented.press_paint > 0.0 {
        let own = moving.unwrap_or_else(|| rgba(node.text_color().resolve(dark)));
        let ink = rgba(node.style.press_color.resolve(dark));
        return Some(super::presented::press_mix(own, ink, presented.press_paint));
    }
    moving
}

fn moving_color(walk: &super::Walk<'_, '_>, node: &NodeRef<'_>) -> Option<[u8; 4]> {
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

/// The press colour a text shows (LLP 1061 D7), and how far: the nearest
/// node from it up to where its `color` comes from (that node included)
/// that is pressed with a `-exact-press-color`, as CSS's `color` reaches a
/// subtree. A text with a colour of its own takes only its own press's.
fn pressed_ink(
    walk: &super::Walk<'_, '_>,
    node: &NodeRef<'_>,
    dark: bool,
) -> Option<([u8; 4], f32)> {
    let source = node.source_of(StyleId::TextColor);
    let mut at = Some(node.id);
    while let Some(n) = at.and_then(|id| walk.scene.kernel.node(id)) {
        if n.style.mask.has(StyleId::PressColor) {
            let progress = (walk.scene.presented)(n.id).press_paint;
            if progress > 0.0 {
                return Some((rgba(n.style.press_color.resolve(dark)), progress));
            }
        }
        if Some(n.id) == source {
            break;
        }
        at = n.parent;
    }
    None
}

/// The paragraph's run colours with presented ones applied, run by run. A
/// Markdown paragraph's one source run paints as its expanded runs, the
/// quieter roles (markers, quotes) at the web's `opacity: 0.62` (LLP 1045 D3).
pub(super) fn presented_text_colors(
    walk: &super::Walk<'_, '_>,
    node: &NodeRef<'_>,
    paragraph: &Paragraph,
    palette: &mut Vec<RunPaint>,
    dark: bool,
) {
    for run in palette.iter_mut() {
        if let Some(n) = walk.scene.kernel.node(run.source) {
            if let Some(c) = moving_color(walk, &n) {
                run.color = c;
            }
            if let Some((ink, progress)) = pressed_ink(walk, &n, dark) {
                run.color = super::presented::press_mix(run.color, ink, progress);
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
    // After presented colours and Markdown's expansion: a hidden run's ink
    // stays clear, including a colour animation on that run.
    for run in palette.iter_mut() {
        if !super::paints(walk.scene.kernel, run.source, walk.reveal) {
            run.color = [0, 0, 0, 0];
        }
    }
}
