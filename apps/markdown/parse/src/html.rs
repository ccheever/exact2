//! The raw HTML the parser draws: `<img>` and `<picture>` (LLP 1033 D9).
//!
//! What it reads is what GitHub's sanitizer keeps of those elements
//! (`hast-util-sanitize`'s default schema, which T3 Code's chat renders
//! through): an `<img>`'s `src`, `alt`, `width` and `height`, and of a
//! `<picture>` only the `<source>`s whose `media` is a `prefers-color-scheme`
//! query, the one choice a reader can make without a browser. Nothing else
//! in the markup is read, and markup holding anything else is not images: the
//! caller shows it literally, as it shows all other raw HTML.

/// One image the markup draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    /// The `<img>`'s `src`, empty when it has none or names a scheme the
    /// caller does not allow.
    pub src: String,
    /// The `<img>`'s `alt`.
    pub alt: String,
    /// The authored `width` and `height` in CSS pixels, rounded; 0 when not
    /// given or not a positive number (`width="50%"` is not one).
    pub width: u32,
    /// See `width`.
    pub height: u32,
    /// The first `(prefers-color-scheme: dark)` source of the `<picture>`
    /// around it, or empty.
    pub dark: String,
    /// The first `(prefers-color-scheme: light)` source, or empty.
    pub light: String,
}

/// The images that the image markup at the start of `text` draws, and the
/// byte length of that markup: one `<img>`, or one `<picture>` through its
/// `</picture>` (which may draw none, or several). None when `text` does not
/// start with either, or the element is unfinished or holds other markup.
/// `schemes` are the URL schemes a source may name (see [`allowed`]).
pub fn element(text: &str, schemes: &[&str]) -> Option<(Vec<Image>, usize)> {
    let tag = tag(text)?;
    match tag.name.as_str() {
        "img" if !tag.closing => Some((vec![image(&tag, schemes)], tag.len)),
        "picture" if !tag.closing => {
            let (images, len) = picture(&text[tag.len..], schemes)?;
            Some((images, tag.len + len))
        }
        _ => None,
    }
}

/// Whether `line` starts with an `<img` or `<picture` tag, the cheap test
/// before [`element`].
pub fn starts_element(line: &str) -> bool {
    ["<img", "<picture"].iter().any(|open| {
        line.get(..open.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(open))
            && matches!(
                line.as_bytes().get(open.len()),
                None | Some(b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
            )
    })
}

/// Whether a source may load: a URL with no scheme, a Windows drive path, or
/// one of `schemes`. A scheme is the text before the first `:`, unless a `/`,
/// `?` or `#` comes before it, and is compared case-sensitively, as the
/// sanitizer compares it. A drive path is a path, though the sanitizer reads
/// its letter as a scheme: T3 Code keeps such a path in its own attribute.
pub fn allowed(url: &str, schemes: &[&str]) -> bool {
    let Some(colon) = url.find(':') else {
        return true;
    };
    if url[..colon].contains(['/', '?', '#']) || drive_path(url) {
        return true;
    }
    schemes.contains(&&url[..colon])
}

fn drive_path(url: &str) -> bool {
    let bytes = url.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn image(tag: &Tag, schemes: &[&str]) -> Image {
    let src = tag.attribute("src").unwrap_or_default();
    Image {
        src: if allowed(&src, schemes) {
            src
        } else {
            String::new()
        },
        alt: tag.attribute("alt").unwrap_or_default(),
        width: pixels(tag.attribute("width")),
        height: pixels(tag.attribute("height")),
        ..Image::default()
    }
}

/// JavaScript's `Number(value)` for the plain decimal numbers HTML authors
/// write, kept when positive.
fn pixels(value: Option<String>) -> u32 {
    let parsed = value.and_then(|v| v.trim().parse::<f64>().ok());
    match parsed {
        Some(n) if n.is_finite() && n > 0.0 => n.round().min(u32::MAX as f64) as u32,
        _ => 0,
    }
}

/// A `<picture>`'s content after its start tag. Its colour-scheme sources
/// belong to its first `<img>`, the first of each scheme winning, as in a
/// browser; a source with any other `media` is never chosen.
fn picture(text: &str, schemes: &[&str]) -> Option<(Vec<Image>, usize)> {
    let mut images = Vec::new();
    let (mut dark, mut light) = (None, None);
    let mut at = 0;
    loop {
        at += space(&text[at..]);
        let rest = &text[at..];
        if let Some(len) = comment(rest) {
            at += len;
            continue;
        }
        let tag = tag(rest)?;
        at += tag.len;
        match (tag.name.as_str(), tag.closing) {
            ("picture", true) => break,
            ("img", false) => images.push(image(&tag, schemes)),
            ("source", false) => {
                let scheme = tag.attribute("media").and_then(|m| color_scheme(&m));
                let url = tag.attribute("srcset").map(|s| first_candidate(&s));
                let slot = match scheme {
                    Some(Scheme::Dark) => &mut dark,
                    Some(Scheme::Light) => &mut light,
                    None => continue,
                };
                if slot.is_none() {
                    *slot = url.filter(|u| !u.is_empty());
                }
            }
            // A browser ignores these end tags; they hold nothing.
            ("img" | "source", true) => {}
            _ => return None,
        }
    }
    // A source the sanitizer refuses is gone, and a later one does not take its place.
    if let Some(first) = images.first_mut() {
        let keep = |url: Option<String>| url.filter(|u| allowed(u, schemes)).unwrap_or_default();
        first.dark = keep(dark);
        first.light = keep(light);
    }
    Some((images, at))
}

enum Scheme {
    Dark,
    Light,
}

/// `(prefers-color-scheme: dark)` and `light`, spaced and cased as written.
fn color_scheme(media: &str) -> Option<Scheme> {
    let inner = media.strip_prefix('(')?.strip_suffix(')')?;
    let (feature, value) = inner.split_once(':')?;
    if !feature.trim().eq_ignore_ascii_case("prefers-color-scheme") {
        return None;
    }
    match value.trim() {
        v if v.eq_ignore_ascii_case("dark") => Some(Scheme::Dark),
        v if v.eq_ignore_ascii_case("light") => Some(Scheme::Light),
        _ => None,
    }
}

/// A `srcset`'s first URL: up to the first white space, a descriptor such as
/// `2x` following it, without the commas that end a candidate.
fn first_candidate(srcset: &str) -> String {
    let trimmed = srcset.trim();
    let url = trimmed.split(char::is_whitespace).next().unwrap_or("");
    url.trim_end_matches(',').to_string()
}

fn space(text: &str) -> usize {
    text.len() - text.trim_start().len()
}

fn comment(text: &str) -> Option<usize> {
    let rest = text.strip_prefix("<!--")?;
    Some(4 + rest.find("-->")? + 3)
}

struct Tag {
    name: String,
    closing: bool,
    attributes: Vec<(String, String)>,
    len: usize,
}

impl Tag {
    /// An attribute's value; a repeated attribute keeps its first, as HTML does.
    fn attribute(&self, name: &str) -> Option<String> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }
}

/// One start or end tag at the start of `text`, as an HTML tokenizer reads
/// it: a lowercased name, attributes whose names are lowercased and whose
/// values may be quoted either way or not at all, and its length through `>`.
fn tag(text: &str) -> Option<Tag> {
    let bytes = text.as_bytes();
    let mut at = 1;
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let closing = bytes.get(at) == Some(&b'/');
    if closing {
        at += 1;
    }
    let name_len = bytes[at..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric())
        .count();
    if name_len == 0 || !bytes[at].is_ascii_alphabetic() {
        return None;
    }
    let name = text[at..at + name_len].to_ascii_lowercase();
    at += name_len;
    if !matches!(
        bytes.get(at),
        Some(b'>' | b'/') | Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
    ) {
        return None;
    }
    let mut attributes = Vec::new();
    loop {
        while matches!(
            bytes.get(at),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b'/')
        ) {
            at += 1;
        }
        if *bytes.get(at)? == b'>' {
            break;
        }
        let start = at;
        while !matches!(
            bytes.get(at),
            None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b'/' | b'>')
        ) && (at == start || bytes[at] != b'=')
        {
            at += 1;
        }
        let name = text[start..at].to_ascii_lowercase();
        let mut after = at;
        while matches!(
            bytes.get(after),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
        ) {
            after += 1;
        }
        let mut value = String::new();
        if bytes.get(after) == Some(&b'=') {
            at = after + 1;
            while matches!(bytes.get(at), Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')) {
                at += 1;
            }
            match bytes.get(at)? {
                quote @ (b'"' | b'\'') => {
                    let close = at + 1 + text[at + 1..].find(*quote as char)?;
                    value = decode(&text[at + 1..close]);
                    at = close + 1;
                }
                _ => {
                    let end = text[at..]
                        .find([' ', '\t', '\n', '\r', '\x0c', '>'])
                        .map_or(text.len(), |offset| at + offset);
                    value = decode(&text[at..end]);
                    at = end;
                }
            }
        }
        if !attributes
            .iter()
            .any(|(n, _): &(String, String)| *n == name)
        {
            attributes.push((name, value));
        }
    }
    Some(Tag {
        name,
        closing,
        attributes,
        len: at + 1,
    })
}

/// The character references a URL or an alt is written with: `&amp;` and
/// its four siblings, and numeric ones. Any other `&` stays as written.
fn decode(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let reference = rest[1..]
            .find(';')
            .map(|semi| (&rest[1..1 + semi], semi + 2));
        let decoded = reference.and_then(|(name, len)| {
            let c = match name {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                _ => {
                    let digits = name.strip_prefix('#')?;
                    let code = match digits.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => digits.parse().ok()?,
                    };
                    char::from_u32(code).filter(|&c| c != '\0')?
                }
            };
            Some((c, len))
        });
        match decoded {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
