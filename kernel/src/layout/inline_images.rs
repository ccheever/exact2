//! Placing the images in a paragraph's text (LLP 1011.002 D2): the
//! measurer reserved each box where its run stands; the frame each image
//! takes is where the measurer says that box went, read when its paragraph
//! is published.

use super::*;

/// Publish the frames of the images in the paragraph at `slot`, laid out at
/// `frame` with box `l`: each one's box from its margin box's origin in the
/// paragraph's content box. A moved image is in `changed` and `updated`; one
/// whose paragraph moved is in `updated`, its place in its parent changed.
pub(super) fn place(
    arena: &mut NodeArena,
    measurer: &mut dyn TextMeasurer,
    root: u32,
    (slot, frame, l, moved): (u32, Frame, &taffy::tree::Layout, bool),
    images: &mut Vec<u32>,
    receipt: (&mut Vec<NodeKey>, &mut Vec<NodeKey>),
) {
    images.clear();
    arena.inline_images(slot, images);
    if images.is_empty() {
        return;
    }
    let left = l.padding.left + l.border.left;
    let top = l.padding.top + l.border.top;
    let width = (l.size.width - left - l.padding.right - l.border.right).max(0.0);
    let mut origins = Vec::with_capacity(images.len());
    // Columns and text flowed around exclusions lay its lines out apart:
    // where a box went there is not asked, and its image shows nothing
    // rather than standing over the text (LLP 1011.002, "Not built").
    if arena.frag.multicol.contains_key(&slot) || !arena.flow_shapes(slot).is_empty() {
        origins.resize(images.len(), (f32::NAN, f32::NAN));
    } else if let Some(stamp) = arena.paragraph_stamp(slot) {
        let mut runs = Vec::new();
        arena.text_runs(slot, &mut runs);
        let request = fit::request(
            arena,
            slot,
            &runs,
            &[],
            AxisOffer::Definite(width),
            AxisOffer::MaxContent,
        );
        measurer.inline_boxes(&stamp, &request, &mut origins);
    }
    let (changed, updated) = receipt;
    let answered = !origins.is_empty();
    for (i, &image) in images.iter().enumerate() {
        let geometry = crate::text::inline_box::inline_image(arena, image);
        // A box the measurer did not place (on a line `line-clamp` cut)
        // shows nothing; with no answer at all, each is at the origin.
        let origin = match origins.get(i) {
            Some(&(x, y)) if x.is_finite() && y.is_finite() => Some((x, y)),
            _ if answered => None,
            _ => Some((0.0, 0.0)),
        };
        let (ox, oy) = origin.unwrap_or((0.0, 0.0));
        let (width, height) = origin.map_or((0.0, 0.0), |_| geometry.size);
        let placed = Frame {
            x: frame.x + left + ox + geometry.offset.0,
            y: frame.y + top + oy + geometry.offset.1,
            width,
            height,
        };
        let old = arena.frame(image);
        let first = arena.flags(image).has(NodeFlags::CREATED);
        arena.flags_mut(image).remove(NodeFlags::HIDDEN);
        arena.set_frame(image, placed);
        arena.set_content(image, geometry.size);
        let (padding, border) = geometry.inset;
        let inset = Some(crate::id::BoxInset {
            padding,
            border,
            cover: [0.0; 4],
        });
        let inset_changed = arena.inset(image) != inset;
        arena.set_inset(image, inset);
        arena.consume_layout_flags(image);
        if first || !old.bits_eq(placed) {
            arena.mark_geometry_changed(image, root);
            changed.push(arena.key(image));
            updated.push(arena.key(image));
        } else if inset_changed || moved {
            updated.push(arena.key(image));
        }
    }
}
