//! Bound resolved pattern tiles and compositor layers before resvg allocates them.
use super::{tiny_skia, usvg, Error};

type Surface = (u32, u32);

pub(super) fn renderable(
    tree: &usvg::Tree,
    transform: tiny_skia::Transform,
    w: u32,
    h: u32,
    output_bytes: usize,
) -> Result<(), Error> {
    // A clipped group holds RGBA group + RGBA clip + alpha coverage (9 bytes
    // per pixel); one isolated ancestor raises that to 13. Four output-sized
    // buffers plus 64 KiB cover this ordinary composition and layer padding.
    // Match RasterDecodePlan; makeImage's copy follows these intermediates.
    let allowance = output_bytes
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or(Error::Limit)?;
    // Match resvg 0.48.1's max_filter_bbox, retained even inside pattern tiles.
    let bounds = tiny_skia::IntRect::from_xywh(
        i32::try_from(w).unwrap_or(i32::MAX).saturating_mul(-2),
        i32::try_from(h).unwrap_or(i32::MAX).saturating_mul(-2),
        w.saturating_mul(5),
        h.saturating_mul(5),
    )
    .unwrap_or_else(|| {
        tiny_skia::IntRect::from_ltrb(i32::MIN / 2, i32::MIN / 2, i32::MAX / 2, i32::MAX / 2)
            .unwrap()
    });
    nodes(tree.root(), transform, (w, h), allowance, bounds)
}

fn reserve(surface: Surface, channels: usize, allowance: usize) -> Result<usize, Error> {
    let bytes = (surface.0 as usize)
        .checked_mul(surface.1 as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or(Error::Unsupported)?;
    allowance.checked_sub(bytes).ok_or(Error::Unsupported)
}

fn nodes(
    root: &usvg::Group,
    transform: tiny_skia::Transform,
    surface: Surface,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    for node in root.children() {
        match node {
            usvg::Node::Group(group) => {
                group_paints(group, transform, surface, allowance, bounds)?;
            }
            usvg::Node::Path(path) if path.is_visible() => {
                fill(path, transform, allowance, bounds)?;
                if let Some(stroke) = path.stroke() {
                    paint(stroke.paint(), transform, allowance, bounds)?;
                }
            }
            usvg::Node::Text(text) => {
                group_paints(text.flattened(), transform, surface, allowance, bounds)?;
            }
            usvg::Node::Image(_) => return Err(Error::Unsupported),
            _ => {}
        }
    }
    Ok(())
}

fn group_paints(
    group: &usvg::Group,
    transform: tiny_skia::Transform,
    surface: Surface,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    let transform = transform.pre_concat(group.transform());
    if !group.should_isolate() {
        return nodes(group, transform, surface, allowance, bounds);
    }
    // Match resvg's unfiltered render_group layer bounds. Shifting its layer
    // origin changes only translation, so subsequent tile scales stay the same.
    let Some((layer, transform)) = layer(group, transform, bounds) else {
        return Ok(()); // resvg skips a layer with invalid or fully clipped bounds.
    };
    let remaining = reserve(layer, 4, allowance)?;
    nodes(group, transform, layer, remaining, bounds)?;
    if let Some(clip) = group.clip_path() {
        clip_paints(clip, transform, layer, remaining, bounds)?;
    }
    if let Some(mask) = group.mask() {
        mask_paints(mask, transform, layer, remaining, bounds)?;
    }
    Ok(())
}

fn layer(
    group: &usvg::Group,
    transform: tiny_skia::Transform,
    bounds: tiny_skia::IntRect,
) -> Option<(Surface, tiny_skia::Transform)> {
    let bbox = group.layer_bounding_box().transform(transform)?;
    let rect = tiny_skia::IntRect::from_xywh(
        (bbox.x().floor() as i32).checked_sub(2)?,
        (bbox.y().floor() as i32).checked_sub(2)?,
        (bbox.width().ceil() as u32).checked_add(4)?,
        (bbox.height().ceil() as u32).checked_add(4)?,
    )?;
    let rect = tiny_skia::IntRect::from_ltrb(
        rect.left().max(bounds.left()),
        rect.top().max(bounds.top()),
        rect.right().min(bounds.right()),
        rect.bottom().min(bounds.bottom()),
    )?;
    let mut dx = bbox.x();
    let mut dy = bbox.y();
    dx -= bbox.x() - rect.x() as f32;
    dy -= bbox.y() - rect.y() as f32;
    let transform = tiny_skia::Transform::from_translate(-dx, -dy).pre_concat(transform);
    Some(((rect.width(), rect.height()), transform))
}

fn clip_paints(
    clip: &usvg::ClipPath,
    transform: tiny_skia::Transform,
    surface: Surface,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    // Clip coverage needs a colour pixmap and one-byte alpha mask. Keeping
    // both charged through nested clips conservatively bounds their peak.
    let remaining = reserve(surface, 5, allowance)?;
    clip_nodes(
        clip.root(),
        transform.pre_concat(clip.transform()),
        surface,
        remaining,
        bounds,
    )?;
    if let Some(clip) = clip.clip_path() {
        clip_paints(clip, transform, surface, remaining, bounds)?;
    }
    Ok(())
}

fn clip_nodes(
    root: &usvg::Group,
    transform: tiny_skia::Transform,
    surface: Surface,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    for node in root.children() {
        match node {
            usvg::Node::Path(path) if path.is_visible() => {
                fill(path, transform, allowance, bounds)?;
            }
            usvg::Node::Text(text) => {
                clip_nodes(text.flattened(), transform, surface, allowance, bounds)?;
            }
            usvg::Node::Group(group) => {
                let transform = transform.pre_concat(group.transform());
                if let Some(clip) = group.clip_path() {
                    let remaining = reserve(surface, 4, allowance)?;
                    clip_nodes(group, transform, surface, remaining, bounds)?;
                    clip_paints(clip, transform, surface, remaining, bounds)?;
                } else {
                    clip_nodes(group, transform, surface, allowance, bounds)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn mask_paints(
    mask: &usvg::Mask,
    transform: tiny_skia::Transform,
    surface: Surface,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    if mask.root().children().is_empty() {
        return Ok(());
    }
    // The mask pixmap and alpha coverage coexist with its rendered children.
    let remaining = reserve(surface, 5, allowance)?;
    nodes(mask.root(), transform, surface, remaining, bounds)?;
    if let Some(mask) = mask.mask() {
        mask_paints(mask, transform, surface, remaining, bounds)?;
    }
    Ok(())
}

fn fill(
    path: &usvg::Path,
    transform: tiny_skia::Transform,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    if path.data().bounds().width() > 0.0 && path.data().bounds().height() > 0.0 {
        if let Some(fill) = path.fill() {
            paint(fill.paint(), transform, allowance, bounds)?;
        }
    }
    Ok(())
}

fn paint(
    paint: &usvg::Paint,
    transform: tiny_skia::Transform,
    allowance: usize,
    bounds: tiny_skia::IntRect,
) -> Result<(), Error> {
    let usvg::Paint::Pattern(pattern) = paint else {
        return Ok(());
    };
    // Match resvg 0.48.1's render_pattern_pixmap: tiles are rounded in device
    // pixels using the referenced paint's complete transform, never viewport-
    // clipped. usvg has already resolved units, hrefs and bounding boxes.
    let (sx, sy) = transform.pre_concat(pattern.transform()).get_scale();
    let width = (pattern.rect().width() * sx).round();
    let height = (pattern.rect().height() * sy).round();
    if !width.is_finite() || !height.is_finite() {
        return Err(Error::Unsupported);
    }
    if width <= 0.0 || height <= 0.0 {
        return Ok(()); // resvg cannot create a zero-sized tile.
    }
    if width as f64 > u32::MAX as f64 || height as f64 > u32::MAX as f64 {
        return Err(Error::Unsupported);
    }
    let surface = (width as u32, height as u32);
    let remaining = reserve(surface, 4, allowance)?;
    // A nested pattern is rendered while its enclosing tile is still alive.
    nodes(
        pattern.root(),
        tiny_skia::Transform::from_scale(sx, sy),
        surface,
        remaining,
        bounds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg(content: &str) -> String {
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'>{content}</svg>")
    }

    fn check(source: &str, w: u32, h: u32) -> Result<(), Error> {
        let tree = usvg::Tree::from_str(source, &usvg::Options::default()).unwrap();
        renderable(
            &tree,
            tiny_skia::Transform::from_scale(
                w as f32 / tree.size().width(),
                h as f32 / tree.size().height(),
            ),
            w,
            h,
            w as usize * h as usize * 4,
        )
    }

    #[test]
    fn css_filter_functions_are_refused_during_metadata_inspection() {
        for filter in ["blur(8)", "contrast(50%)"] {
            for content in [
                format!("<rect width='64' height='64' filter='{filter}'/>"),
                format!("<rect width='64' height='64' style='filter:{filter}'/>"),
                format!("<style>rect {{filter:{filter}}}</style><rect width='64' height='64'/>"),
                format!(
                    "<defs><rect id='r' width='64' height='64' style='filter:{filter}'/></defs><use href='#r'/>"
                ),
            ] {
                let source = svg(&content);
                assert!(matches!(
                    crate::document::size(source.as_bytes()),
                    Err(Error::Unsupported)
                ));
                let mut pixels = vec![7; 64 * 64 * 4];
                assert_eq!(
                    crate::document::render(
                        source.as_bytes(),
                        &mut pixels,
                        64,
                        64,
                        256,
                        (64.0, 64.0),
                        (64.0, 64.0)
                    ),
                    Err(Error::Unsupported)
                );
                assert!(pixels.iter().all(|byte| *byte == 7));
            }
        }
        assert!(crate::document::size(
            svg("<rect width='64' height='64' filter='none'/>").as_bytes()
        )
        .is_ok());
    }

    #[test]
    fn oversized_pattern_is_refused_before_render_allocates_a_tile() {
        let source = svg(
            "<defs><pattern id='p' patternUnits='userSpaceOnUse' width='16384' height='16384'><rect width='1' height='1'/></pattern></defs><rect width='64' height='64' fill='url(#p)'/>",
        );
        assert_eq!(check(&source, 64, 64), Err(Error::Unsupported));
        let mut pixels = vec![7; 64 * 64 * 4];
        assert_eq!(
            crate::document::render(
                source.as_bytes(),
                &mut pixels,
                64,
                64,
                256,
                (64.0, 64.0),
                (64.0, 64.0)
            ),
            Err(Error::Unsupported)
        );
        assert!(pixels.iter().all(|byte| *byte == 7));
        // The same allocation exists on a stroke, including a zero-height path.
        let source = svg(
            "<defs><pattern id='p' patternUnits='userSpaceOnUse' width='16384' height='16384'><rect width='1' height='1'/></pattern></defs><path d='M0 32H64' fill='none' stroke='url(#p)'/>",
        );
        assert_eq!(check(&source, 64, 64), Err(Error::Unsupported));
    }

    #[test]
    fn pattern_budget_uses_device_scale_and_resolved_units() {
        let source = svg(
            "<defs><pattern id='p' patternUnits='userSpaceOnUse' width='320' height='160'><rect width='1' height='1'/></pattern></defs><rect width='64' height='64' fill='url(#p)'/>",
        );
        assert_eq!(check(&source, 64, 64), Err(Error::Unsupported));
        assert_eq!(check(&source, 16, 16), Ok(()));
        for attributes in [
            "patternUnits='userSpaceOnUse' width='8' height='8' patternTransform='scale(32)'",
            "width='4' height='4'",
        ] {
            let source = svg(&format!(
                "<defs><pattern id='p' {attributes}><rect width='1' height='1'/></pattern></defs><rect width='64' height='64' fill='url(#p)'/>"
            ));
            assert_eq!(check(&source, 64, 64), Err(Error::Unsupported));
        }
    }

    #[test]
    fn simultaneously_live_pattern_tiles_share_the_allowance() {
        let single = svg(
            "<defs><pattern id='p' patternUnits='userSpaceOnUse' width='160' height='160'><rect width='1' height='1'/></pattern></defs><rect width='64' height='64' fill='url(#p)'/>",
        );
        assert_eq!(check(&single, 64, 64), Ok(()));
        let nested = svg(
            "<defs><pattern id='q' patternUnits='userSpaceOnUse' width='160' height='160'><rect width='1' height='1'/></pattern><pattern id='p' patternUnits='userSpaceOnUse' width='160' height='160'><rect width='160' height='160' fill='url(#q)'/></pattern></defs><rect width='64' height='64' fill='url(#p)'/>",
        );
        assert_eq!(check(&nested, 64, 64), Err(Error::Unsupported));
    }

    #[test]
    fn isolated_layers_and_mask_coverage_cannot_amplify_the_reservation() {
        let offscreen =
            svg("<g opacity='.5'><rect x='-128' y='-128' width='320' height='320'/></g>");
        assert_eq!(check(&offscreen, 64, 64), Err(Error::Unsupported));
        let groups = format!(
            "{}<rect width='64' height='64'/>{}",
            "<g opacity='.5'>".repeat(9),
            "</g>".repeat(9)
        );
        assert_eq!(check(&svg(&groups), 64, 64), Err(Error::Unsupported));
        let masks = svg(
            "<defs><mask id='a'><rect width='64' height='64' fill='white'/></mask><mask id='b' mask='url(#a)'><rect width='64' height='64' fill='white'/></mask><mask id='c' mask='url(#b)'><rect width='64' height='64' fill='white'/></mask><mask id='d' mask='url(#c)'><rect width='64' height='64' fill='white'/></mask><mask id='e' mask='url(#d)'><rect width='64' height='64' fill='white'/></mask><mask id='f' mask='url(#e)'><rect width='64' height='64' fill='white'/></mask></defs><rect width='64' height='64' mask='url(#f)'/>",
        );
        assert_eq!(check(&masks, 64, 64), Err(Error::Unsupported));
    }

    #[test]
    fn small_patterns_masks_clips_and_opacity_still_render() {
        let source = svg(
            "<defs><pattern id='p' patternUnits='userSpaceOnUse' width='8' height='8'><rect width='8' height='8' fill='red'/></pattern><clipPath id='c'><rect width='64' height='64'/></clipPath><mask id='m'><rect width='64' height='64' fill='white'/></mask></defs><g opacity='.5' clip-path='url(#c)' mask='url(#m)'><rect width='64' height='64' fill='url(#p)'/></g>",
        );
        assert_eq!(check(&source, 64, 64), Ok(()));
        let tree = usvg::Tree::from_str(&source, &usvg::Options::default()).unwrap();
        let mut pixels = tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        assert_eq!(&pixels.data()[..4], &[128, 0, 0, 128]);
    }
    #[test]
    fn gallery_clip_and_opacity_render_when_upscaled() {
        let source = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/svg-gallery/assets/image.svg"
        ));
        for side in [160, 320] {
            let stride = side as usize * 4 + 64;
            let mut pixels = vec![7; stride * side as usize];
            crate::document::render(
                source,
                &mut pixels,
                side,
                side,
                stride,
                (20.0, 20.0),
                (80.0, 80.0),
            )
            .unwrap();
            assert!(pixels
                .chunks_exact(stride)
                .any(|row| row[..side as usize * 4]
                    .chunks_exact(4)
                    .any(|pixel| pixel[3] > 0 && pixel[0] != pixel[2])));
            assert!(pixels
                .chunks_exact(stride)
                .all(|row| row[side as usize * 4..].iter().all(|byte| *byte == 0)));
        }
        let source = svg("<defs><clipPath id='c'><rect width='64' height='64'/></clipPath></defs><g opacity='.5'><g clip-path='url(#c)'><rect width='64' height='64' fill='red'/></g></g>");
        let mut pixels = vec![0; 1024 * 1024 * 4];
        crate::document::render(
            source.as_bytes(),
            &mut pixels,
            1024,
            1024,
            4096,
            (64.0, 64.0),
            (64.0, 64.0),
        )
        .unwrap();
        assert_eq!(&pixels[(512 * 1024 + 512) * 4..][..4], &[0, 0, 128, 128]);
    }
}
