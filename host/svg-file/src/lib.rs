//! An SVG file as an image (LLP 1011.001): the document parsed into the
//! kernel's own SVG nodes in a scratch kernel, resolved by the kernel's
//! scene resolver (`exact_kernel::svg::scene`), for a native host to
//! rasterize as it rasterizes an island and hand to its raster loader as a
//! decoded picture. A browser's `<img>` of an SVG is the model: a picture
//! drawn at the size it is shown, no scripts, no external references, no
//! interaction; animation is ignored (§3's declared deviation).

mod css;
mod lower;
mod xml;

pub use lower::{MAX_DEPTH, MAX_ELEMENTS};

use exact_kernel::svg::scene::{self, Scene};
use exact_kernel::{Kernel, Op};
use std::fmt;

/// Why a file is not an SVG this host reads, with the line it was found on.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: usize,
    pub message: String,
}

impl Error {
    fn at(line: usize, message: impl Into<String>) -> Self {
        Error {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for Error {}

/// The browser's default size for an SVG with neither a size nor a
/// `viewBox` (CSS Images 3 §4.1's default object size, 300×150).
pub const DEFAULT_SIZE: (f32, f32) = (300.0, 150.0);

/// What the root element says of the picture's size (LLP 1011.001 D1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Natural {
    /// An absolute `width`, in CSS pixels; a percentage counts as absent.
    pub width: Option<f32>,
    /// An absolute `height`, likewise.
    pub height: Option<f32>,
    /// The `viewBox`: x, y, width, height, when it has an area.
    pub view_box: Option<[f32; 4]>,
}

impl Natural {
    /// From a root `svg`'s attributes.
    pub fn read(attrs: &[(String, String)]) -> Natural {
        let get = |name: &str| {
            attrs
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        let view_box = get("viewBox").and_then(|v| {
            let n: Vec<f32> = v
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| exact_num::parse_f64(s).ok())
                .map(|f| f as f32)
                .collect();
            (n.len() == 4 && n[2] > 0.0 && n[3] > 0.0 && n.iter().all(|f| f.is_finite()))
                .then(|| [n[0], n[1], n[2], n[3]])
        });
        Natural {
            width: get("width").and_then(absolute_length),
            height: get("height").and_then(absolute_length),
            view_box,
        }
    }

    /// The natural size the kernel's seam carries (D1): the absolute size;
    /// one absolute dimension with the other from the `viewBox`'s ratio;
    /// the `viewBox`'s own size when the file gives no size (the declared
    /// deviation: a browser gives such an image its container's width);
    /// else the browser's default.
    pub fn size(&self) -> (f32, f32) {
        let ratio = self.view_box.map(|v| v[2] / v[3]);
        match (self.width, self.height, ratio) {
            (Some(w), Some(h), _) => (w, h),
            (Some(w), None, Some(r)) => (w, w / r),
            (None, Some(h), Some(r)) => (h * r, h),
            (None, None, Some(_)) => {
                let v = self.view_box.unwrap_or([0.0; 4]);
                (v[2], v[3])
            }
            (Some(w), None, None) => (w, DEFAULT_SIZE.1),
            (None, Some(h), None) => (DEFAULT_SIZE.0, h),
            (None, None, None) => DEFAULT_SIZE,
        }
    }
}

/// A length with an absolute unit in CSS pixels; `None` for a percentage
/// or a unit no picture resolves. `em` and `rem` are the initial 16 px.
fn absolute_length(text: &str) -> Option<f32> {
    let text = text.trim();
    // The longest prefix that is a number: `2em` is 2 and `em`, not `2e` and `m`.
    let split = (1..=text.len())
        .rev()
        .find(|&i| text.is_char_boundary(i) && exact_num::parse_f64(&text[..i]).is_ok())?;
    let (number, unit) = text.split_at(split);
    let n = exact_num::parse_f64(number).ok()? as f32;
    if !n.is_finite() || n <= 0.0 {
        return None;
    }
    let per = match unit.trim() {
        "" | "px" => 1.0,
        "pt" => 4.0 / 3.0,
        "pc" => 16.0,
        "mm" => 96.0 / 25.4,
        "cm" => 96.0 / 2.54,
        "in" => 96.0,
        "em" | "rem" => 16.0,
        _ => return None,
    };
    Some(n * per)
}

/// A parsed document: the ops that build it in a scratch kernel, under a
/// root view holding the `svg`.
#[derive(Debug, Clone)]
pub struct Document {
    pub ops: Vec<Op>,
    /// The root view's id (`Op::AttachRoot`).
    pub root: u32,
    /// The `svg` node's id.
    pub svg: u32,
    pub natural: Natural,
    /// What was dropped or not read, each named once, for the build check
    /// and the host's log.
    pub warnings: Vec<String>,
    /// Whether the picture has text, which a host draws where its fonts
    /// are (D3).
    pub text: bool,
}

/// Whether `bytes` begin an SVG document: optional BOM, white space,
/// comments, an XML declaration and a DOCTYPE before `<svg`, as a browser
/// sniffs one (the file name decides nothing).
pub fn sniff(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(&bytes[..bytes.len().min(4096)]) else {
        // A prefix cut inside a multibyte sequence: read what is whole.
        let valid = std::str::from_utf8(bytes)
            .map(|s| s.len())
            .unwrap_or_else(|e| e.valid_up_to());
        return std::str::from_utf8(&bytes[..valid]).is_ok_and(sniff_text);
    };
    sniff_text(text)
}

fn sniff_text(text: &str) -> bool {
    let mut rest = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        if rest.starts_with("<svg:svg") {
            return true;
        }
        if let Some(after) = rest.strip_prefix("<svg") {
            return after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/');
        }
        if let Some(after) = rest.strip_prefix("<!--") {
            match after.find("-->") {
                Some(i) => rest = after[i + 3..].trim_start(),
                None => return false,
            }
        } else if rest.starts_with("<?")
            || rest.starts_with("<!DOCTYPE")
            || rest.starts_with("<!doctype")
        {
            match rest.find('>') {
                Some(i) => rest = rest[i + 1..].trim_start(),
                None => return false,
            }
        } else {
            return false;
        }
    }
}

/// The natural size a header read gives from a prefix of the file (D4):
/// the root tag's attributes alone. `None` when the prefix holds no root
/// tag or the document is not SVG.
pub fn natural(prefix: &[u8]) -> Option<Natural> {
    if !sniff(prefix) {
        return None;
    }
    let valid = std::str::from_utf8(prefix)
        .map(|s| s.len())
        .unwrap_or_else(|e| e.valid_up_to());
    let text = std::str::from_utf8(&prefix[..valid]).ok()?;
    let (name, attrs) = xml::root_tag(text)?;
    (name == "svg" || name == "svg:svg").then(|| Natural::read(&attrs))
}

/// Parse a document into its ops.
pub fn parse(text: &str) -> Result<Document, Error> {
    lower::lower(xml::tokenize(text)?)
}

/// A document built in a scratch kernel, ready to resolve at any size.
pub struct Loaded {
    kernel: Kernel,
    pub document: Document,
}

/// Nothing presented: an image has no running animation (§3).
fn still(_: exact_kernel::NodeKey, _: exact_motion::Property) -> Option<exact_motion::Value> {
    None
}

/// Parse and build `text` in a scratch kernel.
pub fn load(text: &str) -> Result<Loaded, Error> {
    let document = parse(text)?;
    let mut kernel = Kernel::with_monospace_on_demand();
    kernel
        .apply(document.root, 0, &document.ops)
        .map_err(|e| Error::at(0, format!("the kernel refused the document: {e:?}")))?;
    Ok(Loaded { kernel, document })
}

impl Loaded {
    /// The scene at a concrete object size of `width` × `height` CSS pixels
    /// (D3): user units to that box, `preserveAspectRatio` honoured.
    pub fn scene(&self, width: f32, height: f32) -> Scene {
        let node = self
            .kernel
            .node(self.document.svg)
            .expect("the svg node is in the kernel");
        scene::resolve(&self.kernel, &node, (0.0, 0.0, width, height), &still)
    }

    /// The kernel the document lives in, for a host's own serializer.
    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    /// The `svg` node's id in that kernel.
    pub fn svg(&self) -> u32 {
        self.document.svg
    }

    /// Whether the picture can show an SVG island (LLP 1055.000 D10, D14),
    /// by `exact_runner::svg_islands`'s rule for a plan: a `mask` or
    /// `filter` element, or `filter` set on an SVG element.
    pub fn islands(&self) -> bool {
        use exact_kernel::NodeType;
        self.document.ops.iter().any(|op| {
            let Op::CreateView { id, .. } = op else {
                return false;
            };
            self.kernel
                .node(*id)
                .is_some_and(|node| match node.node_type {
                    NodeType::SvgMask | NodeType::SvgFilter => true,
                    t if t == NodeType::Svg || t.is_svg_element() => !node.style.filter.is_none(),
                    _ => false,
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_natural_size_follows_the_browser_with_the_viewbox_deviation() {
        let n = |attrs: &[(&str, &str)]| {
            Natural::read(
                &attrs
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(n(&[("width", "24"), ("height", "12")]).size(), (24.0, 12.0));
        assert_eq!(
            n(&[("width", "1in"), ("height", "0.5in")]).size(),
            (96.0, 48.0)
        );
        assert_eq!(
            n(&[("width", "100"), ("viewBox", "0 0 4 2")]).size(),
            (100.0, 50.0)
        );
        assert_eq!(
            n(&[("height", "2em"), ("viewBox", "0,0,4,2")]).size(),
            (64.0, 32.0)
        );
        assert_eq!(
            n(&[("viewBox", "10 10 400 300")]).size(),
            (400.0, 300.0),
            "the declared deviation: the viewBox as the size"
        );
        assert_eq!(
            n(&[
                ("width", "100%"),
                ("height", "100%"),
                ("viewBox", "0 0 4 2")
            ])
            .size(),
            (4.0, 2.0),
            "percentages count as absent"
        );
        assert_eq!(n(&[]).size(), DEFAULT_SIZE);
        assert_eq!(n(&[("width", "40")]).size(), (40.0, 150.0));
        assert_eq!(
            n(&[("viewBox", "0 0 0 2")]).view_box,
            None,
            "no area is no viewBox"
        );
    }

    #[test]
    fn sniffing_reads_the_bytes_not_the_name() {
        assert!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"));
        assert!(sniff(
            "\u{feff}  <?xml version=\"1.0\"?>\n<!-- made by hand -->\n<!DOCTYPE svg>\n<svg/>"
                .as_bytes()
        ));
        assert!(sniff(b"<svg:svg xmlns:svg=\"x\">"));
        assert!(!sniff(b"\x89PNG\r\n"));
        assert!(!sniff(b"<svgfoo>"));
        assert!(!sniff(b"<html><svg/></html>"));
        assert!(
            sniff("<svg width=\"1\">\u{e9}".as_bytes()[..16].as_ref()),
            "a prefix cut inside a character"
        );
    }

    #[test]
    fn the_header_gives_the_natural_size_from_a_prefix() {
        let n = natural(b"<?xml version=\"1.0\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"320\" height=\"80\" viewBox=\"0 0 32 8\"><path d=\"M0 0").unwrap();
        assert_eq!(
            (n.size(), n.view_box),
            ((320.0, 80.0), Some([0.0, 0.0, 32.0, 8.0]))
        );
        assert!(natural(b"<svg").is_none(), "no complete root tag yet");
        assert!(natural(b"<?xml version=\"1.0\"?><!-- ...").is_none());
    }

    #[test]
    fn a_wordmark_loads_and_resolves_to_its_shapes_at_the_drawn_size() {
        let loaded = load("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 10\">\n  <rect x=\"0\" y=\"0\" width=\"10\" height=\"10\" fill=\"#f00\"/>\n  <circle cx=\"15\" cy=\"5\" r=\"4\" fill=\"#00f\" stroke=\"#000\" stroke-width=\"0.5\"/>\n</svg>").unwrap();
        assert!(
            loaded.document.warnings.is_empty(),
            "{:?}",
            loaded.document.warnings
        );
        assert_eq!(loaded.document.natural.size(), (20.0, 10.0));
        let scene = loaded.scene(200.0, 100.0);
        assert_eq!(scene.items.len(), 2, "{scene:?}");
        let view = scene.view.expect("a view box with area");
        assert!(
            (view[0] - 10.0).abs() < 1e-3 && (view[3] - 10.0).abs() < 1e-3,
            "user units scale 10× into the box: {view:?}"
        );
        // A box of another ratio letterboxes (`xMidYMid meet`): 2× and centred.
        let tall = loaded.scene(40.0, 100.0).view.unwrap();
        assert!(
            (tall[0] - 2.0).abs() < 1e-3 && (tall[5] - 40.0).abs() < 1e-3,
            "{tall:?}"
        );
    }

    #[test]
    fn what_a_browser_drops_from_a_picture_is_dropped_with_a_warning() {
        let text = "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xmlns:inkscape=\"http://www.inkscape.org/namespaces/inkscape\" width=\"10\" height=\"10\" inkscape:version=\"1.0\">\n<title>Mark</title>\n<style>.a{fill:#0f0}</style>\n<script>alert(1)</script>\n<defs><linearGradient id=\"g\"><stop offset=\"0\" stop-color=\"#fff\"/></linearGradient></defs>\n<a href=\"https://example.com\"><rect class=\"a\" width=\"5\" height=\"5\"><animate attributeName=\"width\" to=\"10\" dur=\"1s\"/></rect></a>\n<image xlink:href=\"https://example.com/x.png\" width=\"1\" height=\"1\"/>\n<use xlink:href=\"#g\"/>\n<switch><g requiredExtensions=\"x\"><circle r=\"1\"/></g><g><rect width=\"1\" height=\"1\"/></g><g><rect width=\"9\" height=\"9\"/></g></switch>\n<inkscape:grid/>\n</svg>";
        let loaded = load(text).unwrap();
        let w = loaded.document.warnings.join("\n");
        assert!(w.contains("`<script>` does not run"), "{w}");
        assert!(w.contains("animation inside an image is ignored"), "{w}");
        assert!(w.contains("`<image>` is dropped from an SVG image"), "{w}");
        assert!(
            !w.contains("inkscape"),
            "a foreign element is ignored without a word: {w}"
        );
        let scene = loaded.scene(10.0, 10.0);
        // The link's rect (filled by the class), the use, the switch's one taken child.
        assert_eq!(scene.items.len(), 3, "{scene:#?}");
    }

    #[test]
    fn a_stylesheet_and_a_style_attribute_cascade_over_presentation_attributes() {
        let loaded = load("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1 1\"><style>rect{fill:#0f0} .x{fill:#00f}</style><rect id=\"r\" class=\"x\" width=\"1\" height=\"1\" fill=\"#f00\" style=\"stroke:#123456\"/></svg>").unwrap();
        let node = loaded.kernel().node(loaded.svg() + 1).unwrap();
        let fill = format!("{:?}", node.style.fill);
        // 0x0000ffff: blue, the class's, over the attribute's red and the type rule's green.
        assert!(
            fill.contains("65535"),
            "the class wins over the attribute: {fill}"
        );
    }

    #[test]
    fn bounds_and_malformed_files_are_the_named_error() {
        assert!(parse("<svg><g>")
            .unwrap_err()
            .to_string()
            .contains("never closed"));
        assert!(parse("<html/>")
            .unwrap_err()
            .message
            .contains("not `<svg>`"));
        let mut deep = String::from("<svg>");
        for _ in 0..=MAX_DEPTH {
            deep.push_str("<g>");
        }
        for _ in 0..=MAX_DEPTH {
            deep.push_str("</g>");
        }
        deep.push_str("</svg>");
        assert!(parse(&deep).unwrap_err().message.contains("nested deeper"));
    }

    /// Files a few kilobytes long that name billions of items, dashes or
    /// surface pixels (LLP 1011.001 D2): each is refused by the limit it
    /// passes, quickly, before a host allocates for it.
    #[test]
    fn a_file_that_expands_past_a_limit_draws_nothing_and_names_it() {
        use exact_kernel::svg::scene::Limit;
        const NS: &str = "xmlns=\"http://www.w3.org/2000/svg\"";
        let zigzag =
            |n: usize| -> String { (0..n).map(|i| format!("l1 {}", i % 2)).collect::<String>() };
        let over = |file: String| {
            let started = std::time::Instant::now();
            let scene = load(&file).unwrap().scene(64.0, 64.0);
            assert!(started.elapsed().as_secs() < 5, "{:?}", started.elapsed());
            assert!(scene.items.is_empty() || scene.over.is_none());
            scene.over
        };
        // 16 levels of `symbol`, each using the one below twice.
        let mut defs = format!(
            "<symbol id=\"s0\" viewBox=\"0 0 9 9\"><path d=\"M0 0{}\" stroke=\"#000\"/></symbol>",
            zigzag(100)
        );
        for k in 1..=16 {
            defs += &format!("<symbol id=\"s{k}\" viewBox=\"0 0 9 9\"><use href=\"#s{}\"/><use href=\"#s{}\" x=\"1\"/></symbol>", k - 1, k - 1);
        }
        assert_eq!(
            over(format!(
                "<svg {NS} viewBox=\"0 0 9 9\"><defs>{defs}</defs><use href=\"#s16\"/></svg>"
            )),
            Some(Limit::Expansion)
        );
        // Markers whose content is a marked path, four deep: 100^4 copies.
        let mut defs = String::from("<marker id=\"m0\"><circle r=\"1\"/></marker>");
        for k in 1..=4 {
            defs += &format!("<marker id=\"m{k}\"><path d=\"M0 0{}\" marker-mid=\"url(#m{})\" stroke=\"#000\"/></marker>", zigzag(100), k - 1);
        }
        assert_eq!(
            over(format!("<svg {NS} viewBox=\"0 0 9 9\"><defs>{defs}</defs><path d=\"M0 0{}\" marker-mid=\"url(#m4)\"/></svg>", zigzag(100))),
            Some(Limit::Expansion)
        );
        // 300 paths of a million dashes each.
        let paths: String = (0..300)
            .map(|i| {
                format!("<path d=\"M0 {i}H99000\" stroke=\"#000\" stroke-dasharray=\"0.05\"/>")
            })
            .collect();
        assert_eq!(
            over(format!("<svg {NS} viewBox=\"0 0 99000 300\">{paths}</svg>")),
            Some(Limit::Dashes)
        );
        // Filters whose region is far past the drawing, each an island of
        // the largest size with its working images, on 400 shapes.
        let shapes = "<rect width=\"5\" height=\"5\" filter=\"url(#f)\"/>".repeat(400);
        assert_eq!(
            over(format!("<svg {NS} viewBox=\"0 0 10 10\"><filter id=\"f\" filterUnits=\"userSpaceOnUse\" x=\"-1e6\" y=\"-1e6\" width=\"2e6\" height=\"2e6\"><feGaussianBlur stdDeviation=\"20\"/></filter>{shapes}</svg>")),
            Some(Limit::Surfaces)
        );
        // A primitive whose work per pixel is a parameter of the file.
        let filtered = |primitive: &str| {
            format!("<svg {NS} viewBox=\"0 0 10 10\"><filter id=\"f\" filterUnits=\"userSpaceOnUse\" x=\"-100\" y=\"-100\" width=\"200\" height=\"200\">{primitive}</filter><rect width=\"5\" height=\"5\" filter=\"url(#f)\"/></svg>")
        };
        for primitive in [
            "<feTurbulence baseFrequency=\"0.1\" numOctaves=\"1000000000\"/>",
            "<feMorphology radius=\"1000000\"/>",
        ] {
            assert_eq!(
                over(filtered(primitive)),
                Some(Limit::FilterWork),
                "{primitive}"
            );
        }
        assert_eq!(over(filtered("<feMorphology radius=\"0.2\"/>")), None);
        // One filter named a thousand times in one list: each a copy.
        let offsets = "<feOffset dx=\"1\"/>".repeat(2000);
        let list = "url(#f) ".repeat(1000);
        assert_eq!(
            over(format!("<svg {NS} viewBox=\"0 0 10 10\"><filter id=\"f\">{offsets}</filter><rect width=\"5\" height=\"5\" style=\"filter: {list}\"/></svg>")),
            Some(Limit::Expansion)
        );
        // A transfer table each filtered shape carries a copy of.
        let table = "1 ".repeat(10_000);
        let shapes = "<rect width=\"1\" height=\"1\" filter=\"url(#t)\"/>".repeat(200);
        assert_eq!(
            over(format!("<svg {NS} viewBox=\"0 0 10 10\"><filter id=\"t\"><feComponentTransfer><feFuncR type=\"table\" tableValues=\"{table}\"/></feComponentTransfer></filter>{shapes}</svg>")),
            Some(Limit::Expansion)
        );
        // Copies that carry a gradient's stops, or empty spans of text.
        let doubling = |base: &str, defs: &str| {
            let mut defs = format!("{defs}<g id=\"l0\">{base}</g>");
            for k in 1..=14 {
                defs += &format!(
                    "<g id=\"l{k}\"><use href=\"#l{0}\"/><use href=\"#l{0}\"/></g>",
                    k - 1
                );
            }
            over(format!(
                "<svg {NS} viewBox=\"0 0 9 9\"><defs>{defs}</defs><use href=\"#l14\"/></svg>"
            ))
        };
        let stops: String = (0..200)
            .map(|i| {
                format!(
                    "<stop offset=\"{}\" stop-color=\"#{:06x}\"/>",
                    i as f32 / 200.0,
                    i * 997
                )
            })
            .collect();
        assert_eq!(
            doubling(
                "<rect width=\"1\" height=\"1\" fill=\"url(#g)\"/>",
                &format!("<linearGradient id=\"g\">{stops}</linearGradient>")
            ),
            Some(Limit::Expansion)
        );
        let spans = "<tspan dx=\"1\"/>".repeat(100);
        assert_eq!(
            doubling(&format!("<text>{spans}</text>"), ""),
            Some(Limit::Expansion)
        );
        // An icon with a drop shadow, a pattern and a few uses is none of these.
        assert_eq!(
            over(format!("<svg {NS} viewBox=\"0 0 24 24\"><defs><filter id=\"f\"><feGaussianBlur stdDeviation=\"1\"/></filter><pattern id=\"p\" width=\"4\" height=\"4\" patternUnits=\"userSpaceOnUse\"><circle cx=\"2\" cy=\"2\" r=\"1\"/></pattern><path id=\"a\" d=\"M2 2h8v8z\"/></defs><g filter=\"url(#f)\"><use href=\"#a\"/><use href=\"#a\" x=\"10\"/></g><rect width=\"24\" height=\"6\" fill=\"url(#p)\" stroke=\"#000\" stroke-dasharray=\"1 1\"/></svg>")),
            None
        );
    }

    #[test]
    fn a_picture_shows_an_island_for_a_mask_or_a_filter() {
        let islands = |body: &str| {
            load(&format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\">{body}</svg>"
            ))
            .unwrap()
            .islands()
        };
        // React Conf's sponsor logos: the art wrapped in a luminance mask.
        assert!(islands(
            "<mask id=\"m\" style=\"mask-type:luminance\"><path d=\"M0 0H10V10H0Z\" fill=\"white\"/></mask>\
             <g mask=\"url(#m)\"><path d=\"M0 0H5V5H0Z\" fill=\"red\"/></g>"
        ));
        assert!(islands(
            "<filter id=\"f\"><feGaussianBlur stdDeviation=\"1\"/></filter><rect width=\"5\" height=\"5\" filter=\"url(#f)\"/>"
        ));
        assert!(islands(
            "<rect width=\"5\" height=\"5\" style=\"filter: blur(1px)\"/>"
        ));
        assert!(!islands(
            "<clipPath id=\"c\"><rect width=\"5\" height=\"5\"/></clipPath>\
             <g clip-path=\"url(#c)\"><path d=\"M0 0H10V10H0Z\" fill=\"red\"/></g>"
        ));
        assert!(!islands(
            "<rect width=\"5\" height=\"5\" style=\"filter: none\"/>"
        ));
    }
}
