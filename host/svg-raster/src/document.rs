//! Self-contained SVG image documents, decoded only by the loaded image module.
use resvg::{tiny_skia, usvg};

pub(crate) mod fonts;
mod safety;

/// Bound document parsing independently of the output bitmap's reservation.
pub const SOURCE_LIMIT: usize = 256 * 1024;

/// Errors crossing the image decoder ABI.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    if filter_declarations(&xml) {
        return Err(Error::Unsupported);
    }
    Ok(xml)
}

fn filter_declarations(xml: &roxmltree::Document<'_>) -> bool {
    let unsupported = |declaration: simplecss::Declaration<'_>| {
        declaration.name == "filter" && declaration.value.trim() != "none"
    };
    xml.descendants().filter(|n| n.is_element()).any(|node| {
        node.attribute("filter").is_some_and(|v| v.trim() != "none")
            || node
                .attribute("style")
                .is_some_and(|style| simplecss::DeclarationTokenizer::from(style).any(unsupported))
            || (node.has_tag_name("style")
                && node.attribute("type").is_none_or(|v| v == "text/css")
                && node.text().is_some_and(|text| {
                    simplecss::StyleSheet::parse(text)
                        .rules
                        .iter()
                        .any(|rule| rule.declarations.iter().copied().any(unsupported))
                }))
    })
}

/// Font staging charged before an SVG worker can enter the decoder.
pub fn font_budget(bytes: &[u8]) -> Result<usize, Error> {
    let xml = xml(bytes)?;
    Ok(if xml.descendants().any(|n| n.has_tag_name("text")) {
        fonts::BYTE_LIMIT
    } else {
        0
    })
}

fn tree(xml: &roxmltree::Document<'_>, resolve_fonts: bool) -> Result<usvg::Tree, Error> {
    let mut options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let fonts = resolve_fonts.then(fonts::Fonts::new);
    if let Some(fonts) = &fonts {
        options.font_resolver = fonts.resolver();
    }
    let tree = usvg::Tree::from_xmltree(xml, &options).map_err(|_| Error::Invalid)?;
    if let Some(fonts) = fonts {
        fonts.check()?;
    }
    // CSS functions in presentation attributes, inline styles and stylesheets
    // become filters too, even when the source contains no <filter> element.
    if !tree.filters().is_empty() {
        return Err(Error::Unsupported);
    }
    Ok(tree)
}

/// Explicit pixel viewports need no path conversion or font discovery during
/// metadata inspection. CSS and other units take the complete SVG size resolver.
fn pixel_viewport(xml: &roxmltree::Document<'_>) -> Option<(f32, f32)> {
    if xml.descendants().any(|n| {
        n.has_tag_name("style") || n.attribute("style").is_some() || n.attribute("filter").is_some()
    }) {
        return None;
    }
    let pixel = |name| {
        let value = xml.root_element().attribute(name)?.trim();
        let value = value.strip_suffix("px").unwrap_or(value);
        let number: f32 = value.trim().parse().ok()?;
        (number.is_finite() && number > 0.0).then_some(number)
    };
    dimensions(pixel("width")?, pixel("height")?).ok()
}

fn dimensions(w: f32, h: f32) -> Result<(f32, f32), Error> {
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return Err(Error::Invalid);
    }
    if w.ceil() * h.ceil() > 64.0 * 1024.0 * 1024.0 {
        return Err(Error::Limit);
    }
    Ok((w, h))
}

/// Give only the outer SVG the negotiated viewport. Nested SVG sizes, the
/// authored viewBox, and preserveAspectRatio remain intact.
fn with_viewport(xml: &roxmltree::Document<'_>, size: (f32, f32)) -> String {
    with_lengths(xml, (&size.0.to_string(), &size.1.to_string()))
}

fn with_lengths(xml: &roxmltree::Document<'_>, lengths: (&str, &str)) -> String {
    let root = xml.root_element();
    let mut text = xml.input_text().to_owned();
    let mut ranges: Vec<_> = root
        .attributes()
        .filter(|a| a.namespace().is_none() && matches!(a.name(), "width" | "height"))
        .map(|a| a.range())
        .collect();
    ranges.sort_by_key(|r| r.start);
    for range in ranges.into_iter().rev() {
        text.replace_range(range, "");
    }
    let start = root.range().start + 1;
    let end = start
        + text[start..]
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .unwrap();
    text.insert_str(
        end,
        &format!(" width='{}' height='{}'", lengths.0, lengths.1),
    );
    text
}

struct CssNode<'a, 'input>(roxmltree::Node<'a, 'input>);
impl simplecss::Element for CssNode<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(CssNode)
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(CssNode)
    }
    fn has_local_name(&self, name: &str) -> bool {
        self.0.tag_name().name() == name
    }
    fn attribute_matches(&self, name: &str, operator: simplecss::AttributeOperator) -> bool {
        self.0
            .attribute(name)
            .is_some_and(|value| operator.matches(value))
    }
    fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
        matches!(class, simplecss::PseudoClass::FirstChild)
            && self.0.prev_sibling_element().is_none()
    }
}

/// usvg 0.48 resolves SVG presentation CSS but not the SVG 2 sizing
/// properties. Apply those two root properties with the same selector parser.
fn root_lengths<'a>(xml: &'a roxmltree::Document<'a>) -> [Option<&'a str>; 2] {
    let root = xml.root_element();
    let mut lengths = [
        (root.attribute("width"), false),
        (root.attribute("height"), false),
    ];
    let mut css = simplecss::StyleSheet::new();
    for node in xml.descendants().filter(|n| n.has_tag_name("style")) {
        if node.attribute("type").is_none_or(|t| t == "text/css") {
            if let Some(text) = node.text() {
                css.parse_more(text);
            }
        }
    }
    let mut apply = |declaration: simplecss::Declaration<'a>| {
        let axis = match declaration.name {
            "width" => 0,
            "height" => 1,
            _ => return,
        };
        if (declaration.value == "auto" || declaration.value.parse::<svgtypes::Length>().is_ok())
            && (declaration.important || !lengths[axis].1)
        {
            lengths[axis] = (Some(declaration.value), declaration.important);
        }
    };
    for rule in &css.rules {
        if rule.selector.matches(&CssNode(root)) {
            for declaration in &rule.declarations {
                apply(*declaration);
            }
        }
    }
    if let Some(style) = root.attribute("style") {
        for declaration in simplecss::DeclarationTokenizer::from(style) {
            apply(declaration);
        }
    }
    lengths.map(|(value, _)| value)
}

fn natural_size(xml: &roxmltree::Document<'_>) -> Result<(f32, f32), Error> {
    if let Some(size) = pixel_viewport(xml) {
        return Ok(size);
    }
    let root = xml.root_element();
    let lengths = root_lengths(xml);
    let absolute = |value: Option<&str>| {
        value
            .and_then(|v| v.parse::<svgtypes::Length>().ok())
            .is_some_and(|v| v.unit != svgtypes::LengthUnit::Percent)
    };
    let (width, height) = (absolute(lengths[0]), absolute(lengths[1]));
    let normalized = with_lengths(
        xml,
        (
            lengths[0].filter(|v| *v != "auto").unwrap_or("100%"),
            lengths[1].filter(|v| *v != "auto").unwrap_or("100%"),
        ),
    );
    let actual = tree(&self::xml(normalized.as_bytes())?, false)?.size();
    if width && height {
        return dimensions(actual.width(), actual.height());
    }
    let viewbox = root
        .attribute("viewBox")
        .and_then(|v| v.parse::<svgtypes::ViewBox>().ok());
    let ratio = viewbox
        .filter(|_| {
            root.attribute("preserveAspectRatio")
                .and_then(|v| v.split_whitespace().next())
                != Some("none")
        })
        .filter(|v| v.w.is_finite() && v.h.is_finite() && v.w > 0.0 && v.h > 0.0)
        .map(|v| (v.w / v.h) as f32);
    let size = match (width, height, ratio) {
        (true, false, Some(r)) => (actual.width(), actual.width() / r),
        (false, true, Some(r)) => (actual.height() * r, actual.height()),
        (true, false, None) => (actual.width(), 150.0),
        (false, true, None) => (300.0, actual.height()),
        (false, false, Some(r)) => {
            if r >= 2.0 {
                (300.0, 300.0 / r)
            } else {
                (150.0 * r, 150.0)
            }
        }
        (false, false, None) => (300.0, 150.0),
        _ => unreachable!(),
    };
    dimensions(size.0, size.1)
}

/// The default object size, without retaining encoded data or a parsed tree.
/// Percentages supply no intrinsic length; an authored viewBox supplies ratio.
pub fn size(bytes: &[u8]) -> Result<(f32, f32), Error> {
    natural_size(&xml(bytes)?)
}

/// Paint directly into the caller's reserved, padded, premultiplied BGRA8
/// bitmap. Revalidate actual document dimensions before writing any pixels.
pub fn render(
    bytes: &[u8],
    pixels: &mut [u8],
    w: u32,
    h: u32,
    stride: usize,
    expected: (f32, f32),
    viewport: (f32, f32),
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
    if natural_size(&xml)? != expected {
        return Err(Error::Changed);
    }
    dimensions(viewport.0, viewport.1)?;
    let negotiated = with_viewport(&xml, viewport);
    let tree = tree(&self::xml(negotiated.as_bytes())?, true)?;
    // Integer bitmap allocation rounds outward; the document's transform is
    // uniform. The host crops that extra fraction instead of stretching it.
    let scale = (w as f32 / viewport.0).min(h as f32 / viewport.1);
    let transform = tiny_skia::Transform::from_scale(scale, scale);
    safety::renderable(&tree, transform, w, h, pixels.len())?;
    pixels.fill(0);
    // Render packed rows in the caller's storage: padding is never painted.
    // Expanding the rows backwards afterward is overlap-safe and needs no
    // second bitmap, even for a very narrow image with large row padding.
    let packed = row.checked_mul(h as usize).ok_or(Error::Limit)?;
    let mut picture =
        tiny_skia::PixmapMut::from_bytes(&mut pixels[..packed], w, h).ok_or(Error::Invalid)?;
    resvg::render(&tree, transform, &mut picture);
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
        assert_eq!(size(svg), Ok((110.0, 130.0)));
        let mut pixels = vec![0; 64 * 13];
        render(svg, &mut pixels, 11, 13, 64, (110.0, 130.0), (110.0, 130.0)).unwrap();
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
        render(svg, &mut pixels, 1, 1, 4, (1.0, 1.0), (1.0, 1.0)).unwrap();
        assert_eq!(pixels, [0, 0, 128, 128]);
    }
    #[test]
    fn padded_rows_preserve_clips_gradients_and_group_opacity() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="5" height="7"><defs><linearGradient id="g"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient><clipPath id="c"><circle cx="3" cy="3" r="3"/></clipPath></defs><g opacity=".4" clip-path="url(#c)"><rect x="-3" y="-2" width="14" height="12" fill="url(#g)"/></g></svg>"##;
        let mut packed = vec![255; 20 * 7];
        let mut padded = vec![255; 64 * 7];
        render(svg, &mut packed, 5, 7, 20, (5.0, 7.0), (5.0, 7.0)).unwrap();
        render(svg, &mut padded, 5, 7, 64, (5.0, 7.0), (5.0, 7.0)).unwrap();
        for (a, b) in packed.chunks_exact(20).zip(padded.chunks_exact(64)) {
            assert_eq!(a, &b[..20]);
            assert!(b[20..].iter().all(|v| *v == 0));
        }
        assert_eq!(
            render(svg, &mut padded, 5, 7, 64, (6.0, 7.0), (5.0, 7.0)),
            Err(Error::Changed)
        );
    }

    #[test]
    fn natural_dimensions_keep_fractional_lengths_and_ignore_percentages() {
        assert_eq!(size(b"<svg width='5.2px' height='7px'/>"), Ok((5.2, 7.0)));
        assert_eq!(
            size(b"<svg width='5' height='7' style='width:10px;height:20px'/>"),
            Ok((10.0, 20.0))
        );
        assert_eq!(
            size(b"<svg class='art'><style>.art {width:10px;height:20px}</style></svg>"),
            Ok((10.0, 20.0))
        );
        assert_eq!(
            size(b"<svg width='100%' height='50%' viewBox='0 0 200 200'/>"),
            Ok((150.0, 150.0))
        );
        assert_eq!(
            size(b"<svg width='100%' height='50%'/>"),
            Ok((300.0, 150.0))
        );
        assert_eq!(
            size(b"<svg viewBox='0 0 200 200' preserveAspectRatio='none'/>"),
            Ok((300.0, 150.0))
        );
        assert_eq!(
            size(b"<svg width='100' height='50%' viewBox='0 0 200 200'/>"),
            Ok((100.0, 100.0))
        );
        assert_eq!(
            size(b"<svg viewBox='-5 -10 11 13'/>"),
            Ok((150.0 * 11.0 / 13.0, 150.0))
        );
    }

    fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
        &pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
    }

    #[test]
    fn concrete_viewport_honors_root_aspect_ratio_and_asymmetric_colors() {
        for (aspect, left, top) in [
            ("xMidYMid meet", false, true),
            ("none", true, true),
            ("xMinYMin meet", true, true),
        ] {
            let svg = format!("<svg width='100' height='100' viewBox='0 0 100 100' preserveAspectRatio='{aspect}' style='width:10px;height:20px'><rect width='100' height='50' fill='red'/><rect y='50' width='100' height='50' fill='blue'/></svg>");
            let mut pixels = vec![0; 200 * 100 * 4];
            render(
                svg.as_bytes(),
                &mut pixels,
                200,
                100,
                800,
                (10.0, 20.0),
                (200.0, 100.0),
            )
            .unwrap();
            assert_eq!(pixel(&pixels, 200, 60, 25), &[0, 0, 255, 255]);
            assert_eq!(pixel(&pixels, 200, 60, 75), &[255, 0, 0, 255]);
            assert_eq!(pixel(&pixels, 200, 10, 25)[3] > 0, left);
            assert_eq!(pixel(&pixels, 200, 60, 10)[3] > 0, top);
        }
    }

    #[test]
    fn fractional_viewport_is_not_stretched_to_the_rounded_bitmap() {
        let svg = b"<svg width='5.2' height='7'><rect width='2.6' height='7' fill='red'/><rect x='2.6' width='2.6' height='7' fill='blue'/></svg>";
        let mut pixels = vec![0; 52 * 70 * 4];
        render(svg, &mut pixels, 52, 70, 208, (5.2, 7.0), (5.2, 7.0)).unwrap();
        assert_eq!(pixel(&pixels, 52, 24, 35), &[0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 52, 27, 35), &[255, 0, 0, 255]);
        let mut rounded = vec![0; 6 * 7 * 4];
        render(svg, &mut rounded, 6, 7, 24, (5.2, 7.0), (5.2, 7.0)).unwrap();
        assert!(pixel(&rounded, 6, 5, 3)[3] < 80);
    }
}
