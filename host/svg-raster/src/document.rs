//! Self-contained SVG image documents, decoded only by the loaded image module.
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, OnceLock};

/// Bound document parsing independently of the output bitmap's reservation.
pub const SOURCE_LIMIT: usize = 256 * 1024;

/// Errors crossing the image decoder ABI.
#[derive(Debug, PartialEq)]
#[repr(i32)]
pub enum Error {
    /// Invalid SVG or output dimensions.
    Invalid = 1,
    /// Document bytes or node count exceed the parsing limit.
    Limit = 2,
    /// A feature needs another document or unreserved intermediate bitmaps.
    Unsupported = 3,
    /// Source dimensions changed since worker admission.
    Changed = 4,
}

fn xml(bytes: &[u8]) -> Result<roxmltree::Document<'_>, Error> {
    if bytes.len() > SOURCE_LIMIT {
        return Err(Error::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Invalid)?;
    let xml = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 10_000,
            entity_resolver: None,
        },
    )
    .map_err(|error| match error {
        roxmltree::Error::NodesLimitReached => Error::Limit,
        _ => Error::Invalid,
    })?;
    if xml.root_element().tag_name().name() != "svg" {
        return Err(Error::Invalid);
    }
    // Images must not bypass the app's resolver. Filters allocate their own
    // intermediate pictures, outside the raster pipeline's reservation.
    if xml.descendants().any(|n| {
        n.is_element() && matches!(n.tag_name().name(), "image" | "filter" | "foreignObject")
    }) {
        return Err(Error::Unsupported);
    }
    Ok(xml)
}

fn tree(xml: &roxmltree::Document<'_>) -> Result<usvg::Tree, Error> {
    let mut options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    if xml.descendants().any(|n| n.has_tag_name("text")) {
        static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
        options.fontdb = FONTS
            .get_or_init(|| {
                let mut fonts = usvg::fontdb::Database::new();
                fonts.load_system_fonts();
                Arc::new(fonts)
            })
            .clone();
    }
    usvg::Tree::from_xmltree(xml, &options).map_err(|_| Error::Invalid)
}

/// Explicit pixel viewports need no path conversion or font discovery during
/// metadata inspection. CSS and other units take the complete SVG size resolver.
fn pixel_viewport(xml: &roxmltree::Document<'_>) -> Option<(u32, u32)> {
    if xml.descendants().any(|n| n.has_tag_name("style"))
        || xml.root_element().attribute("style").is_some()
    {
        return None;
    }
    let pixel = |name| {
        let value = xml.root_element().attribute(name)?.trim();
        let value = value.strip_suffix("px").unwrap_or(value);
        let number: f32 = value.trim().parse().ok()?;
        (number.is_finite() && number > 0.0).then_some(number)
    };
    viewport(pixel("width")?, pixel("height")?).ok()
}

fn viewport(w: f32, h: f32) -> Result<(u32, u32), Error> {
    let (w, h) = (w.ceil(), h.ceil());
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return Err(Error::Invalid);
    }
    if w * h > 64.0 * 1024.0 * 1024.0 {
        return Err(Error::Limit);
    }
    Ok((w as u32, h as u32))
}

/// The natural viewport, without retaining encoded data or a parsed tree.
pub fn size(bytes: &[u8]) -> Result<(u32, u32), Error> {
    let xml = xml(bytes)?;
    if let Some(size) = pixel_viewport(&xml) {
        return Ok(size);
    }
    let size = tree(&xml)?.size();
    viewport(size.width(), size.height())
}

/// Paint directly into the caller's reserved, padded, premultiplied BGRA8
/// bitmap. Revalidate actual document dimensions before writing any pixels.
pub fn render(
    bytes: &[u8],
    pixels: &mut [u8],
    w: u32,
    h: u32,
    stride: usize,
    expected: (u32, u32),
) -> Result<(), Error> {
    let row = (w as usize).checked_mul(4).ok_or(Error::Limit)?;
    let len = stride.checked_mul(h as usize).ok_or(Error::Limit)?;
    if w == 0
        || h == 0
        || stride < row
        || !stride.is_multiple_of(4)
        || pixels.len() != len
        || len > 32 * 1024 * 1024
    {
        return Err(Error::Invalid);
    }
    let xml = xml(bytes)?;
    let tree = tree(&xml)?;
    if viewport(tree.size().width(), tree.size().height())? != expected {
        return Err(Error::Changed);
    }
    pixels.fill(0);
    // Render packed rows in the caller's storage: padding is never painted.
    // Expanding the rows backwards afterward is overlap-safe and needs no
    // second bitmap, even for a very narrow image with large row padding.
    let packed = row.checked_mul(h as usize).ok_or(Error::Limit)?;
    let mut picture =
        tiny_skia::PixmapMut::from_bytes(&mut pixels[..packed], w, h).ok_or(Error::Invalid)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(
            w as f32 / tree.size().width(),
            h as f32 / tree.size().height(),
        ),
        &mut picture,
    );
    for y in (0..h as usize).rev() {
        let start = y * stride;
        if stride != row {
            pixels.copy_within(y * row..(y + 1) * row, start);
        }
        let dst = &mut pixels[start..start + stride];
        for rgba in dst[..row].chunks_exact_mut(4) {
            rgba.swap(0, 2);
        }
        dst[row..].fill(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_viewbox_and_inherited_fill_render_at_requested_size() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="110" height="130" viewBox="-5 -100 110 130" fill="#808080"><path d="M-5-100H105V30H-5Z"/></svg>"##;
        assert_eq!(size(svg), Ok((110, 130)));
        let mut pixels = vec![0; 64 * 13];
        render(svg, &mut pixels, 11, 13, 64, (110, 130)).unwrap();
        assert_eq!(&pixels[..4], &[128, 128, 128, 255]);
        assert!(pixels[44..64].iter().all(|v| *v == 0));
    }

    #[test]
    fn documents_cannot_load_external_images_or_unreserved_filter_bitmaps() {
        for element in [
            "<image href='/etc/passwd'/>",
            "<filter id='f'/>",
            "<foreignObject/>",
        ] {
            let svg = format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'>{element}</svg>"
            );
            assert_eq!(size(svg.as_bytes()), Err(Error::Unsupported));
        }
        assert_eq!(size(&vec![b' '; SOURCE_LIMIT + 1]), Err(Error::Limit));
        assert_eq!(size(b"<html/>"), Err(Error::Invalid));
    }

    #[test]
    fn premultiplied_color_is_copied_as_bgra() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#ff0000" opacity=".5"/></svg>"##;
        let mut pixels = vec![0; 4];
        render(svg, &mut pixels, 1, 1, 4, (1, 1)).unwrap();
        assert_eq!(pixels, [0, 0, 128, 128]);
    }
    #[test]
    fn padded_rows_preserve_clips_gradients_and_group_opacity() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="5" height="7"><defs><linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><clipPath id="c"><circle cx="3" cy="3" r="3"/></clipPath></defs><g opacity=".4" clip-path="url(#c)"><rect x="-3" y="-2" width="14" height="12" fill="url(#g)"/></g></svg>"##;
        let mut packed = vec![255; 20 * 7];
        let mut padded = vec![255; 64 * 7];
        render(svg, &mut packed, 5, 7, 20, (5, 7)).unwrap();
        render(svg, &mut padded, 5, 7, 64, (5, 7)).unwrap();
        for (a, b) in packed.chunks_exact(20).zip(padded.chunks_exact(64)) {
            assert_eq!(a, &b[..20]);
            assert!(b[20..].iter().all(|v| *v == 0));
        }
        assert_eq!(
            render(svg, &mut padded, 5, 7, 64, (6, 7)),
            Err(Error::Changed)
        );
    }

    #[test]
    fn metadata_size_matches_complete_svg_conversion() {
        for attrs in [
            "width='5.2px' height='7px'",
            "width='2cm' height='3cm'",
            "viewBox='-5 -10 11 13'",
            "width='50%' height='50%' viewBox='0 0 20 30'",
            "width='5' height='7' style='width:10px;height:20px'",
        ] {
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' {attrs}><rect width='100%' height='100%'/></svg>");
            let xml = xml(svg.as_bytes()).unwrap();
            let actual = tree(&xml).unwrap().size();
            assert_eq!(
                size(svg.as_bytes()),
                viewport(actual.width(), actual.height())
            );
        }
    }
}
