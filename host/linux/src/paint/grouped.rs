//! Grouped-row separators paint in the border pass, under the row's content.
use super::{border, rgba, Rect4, SeparatorGroups};
use exact_kernel::{Display, Kernel, NodeRef, PropId, StyleId};

pub(super) fn capture(
    node: &NodeRef<'_>,
    kernel: &Kernel,
    groups: &SeparatorGroups,
    dark: bool,
) -> Option<(f32, [u8; 4])> {
    if node.props.bool(PropId::GroupedRowSeparator) != Some(true)
        || node.style.display == Display::None
        || node.parent.is_none_or(|parent| {
            *groups
                .borrow_mut()
                .entry(parent)
                .or_insert_with(|| kernel.grouped_last_visible_row(parent))
                == Some(node.id)
        })
    {
        return None;
    }
    let s = node.style;
    let inset = if s.mask.has(StyleId::PaddingLeft) {
        kernel.resolved_padding(node.key).unwrap_or_default().0
    } else {
        16.0
    };
    let ink = if s.mask.has(StyleId::BorderColorBottom) {
        rgba(
            s.border_colors(node.computed_row(StyleId::TextColor, |s| s.text_color))[2]
                .resolve(dark),
        )
    } else if dark {
        [84, 84, 88, 128]
    } else {
        [60, 60, 67, 31]
    };
    Some((inset, ink))
}

pub(super) fn append(
    fills: &mut Vec<border::BorderFill>,
    (x, y, w, h): Rect4,
    widths: [f32; 4],
    inset: f32,
    ink: [u8; 4],
) {
    let left = x + widths[3] + inset.max(0.0);
    let right = x + w - widths[1];
    let bottom = y + h - widths[2];
    let top = (y + widths[0]).max(bottom - 1.0);
    if right > left && bottom > top {
        fills.push(border::BorderFill {
            region: vec![
                border::PathOp::Move(left, top),
                border::PathOp::Line(right, top),
                border::PathOp::Line(right, bottom),
                border::PathOp::Line(left, bottom),
                border::PathOp::Close,
            ],
            clip: None,
            color: ink,
            ring: None,
            ring_clip: None,
        });
    }
}
