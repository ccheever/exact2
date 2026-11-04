//! The raw HTML the parser draws: `<img>` and `<picture>` (LLP 1033 D9).
//!
//! What it reads is what T3 Code's sanitizer keeps of those elements
//! (`hast-util-sanitize`'s default schema, GitHub's, with T3 Code's image
//! attributes): an `<img>`'s `src`, `alt`, `width` and `height`, its
//! `data-dark-src`, `data-light-src` and `data-local-src`, and of a
//! `<picture>` only the `<source>`s whose `media` is a `prefers-color-scheme`
//! query, the one choice a reader can make without a browser. The images may
//! sit inside elements that draw nothing of their own (`<div>`, `<p>`, …),
//! whose `align` is read as the CSS `text-align` HTML maps it to, and, for a
//! caller that draws images inside a line, inside an `<a href>`, whose link
//! they then carry. Nothing else in the markup is read, and markup holding
//! anything else is not images: the caller shows it literally, as it shows
//! all other raw HTML.
//!
//! An `<img>` is inline content, so the markup's images are laid out in line
//! boxes ([`Line`]): those that follow one another with no `<p>`, `<div>` or
//! `<br>` between are drawn side by side.

use crate::entities::decode;

/// One image the markup draws.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    /// The `<img>`'s `data-local-src` or else its `src`, empty when it has
    /// none or names a scheme the caller does not allow. A `src` that is a
    /// Windows drive path wins over `data-local-src`, as T3 Code copies it
    /// there.
    pub src: String,
    /// The `<img>`'s `alt`.
    pub alt: String,
    /// The authored `width` and `height` in CSS pixels, rounded; 0 when not
    /// given or not a positive number (`width="50%"` is not one).
    pub width: u32,
    /// See `width`.
    pub height: u32,
    /// The `<img>`'s `data-dark-src`, or else the first
    /// `(prefers-color-scheme: dark)` source of the `<picture>` around it,
    /// or empty.
    pub dark: String,
    /// The same for `data-light-src` and `(prefers-color-scheme: light)`.
    pub light: String,
    /// The `href` of the `<a>` around it, empty when there is none or it
    /// names a scheme the caller does not allow (the sanitizer drops such an
    /// `href`, and its image is drawn with no link).
    pub link: String,
    /// Whether white space comes before it on its line, which HTML draws as
    /// one space between it and what is before it.
    pub spaced: bool,
}

/// One line box of a markup's images: `images[start..end]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// The index of its first image.
    pub start: usize,
    /// One past its last image.
    pub end: usize,
    /// The CSS `text-align` HTML maps the `align` of the innermost `<p>` or
    /// `<div>` around it that has one to: `left`, `right`, `center` or
    /// `justify`, or empty.
    pub align: String,
    /// Whether no `<p>`, `<div>` or `<br>` comes before it in the markup, so
    /// it goes on with the line of what the markup follows.
    pub continues: bool,
    /// Whether none comes after it either, so the markup that follows may
    /// go on with it.
    pub open: bool,
    /// Whether a `<p>` holds it, whose margins are then its own.
    pub paragraph: bool,
}

/// How far one run of image markup may reach: a `<picture>` with a few
/// sources is well under 1 KiB. Each place that starts an element reads
/// ahead this far at most, so text of such starts that never close parses
/// in linear time.
pub(crate) const MARKUP_LIMIT: usize = 8 * 1024;

/// The elements an image may sit in that draw nothing of their own in a
/// reader of blocks: those around which T3 Code still counts an image
/// standalone (`markStandaloneImages`). `<a>` is not one: its link would be
/// lost.
const WRAPPERS: [&str; 6] = ["div", "p", "center", "figure", "strong", "em"];

/// The wrappers whose start tag closes an open `<p>`, as HTML's does.
const CLOSES_P: [&str; 4] = ["div", "p", "center", "figure"];

/// The wrappers that are blocks of their own after the sanitizer: their
/// images start a line and end it. It drops `<center>` and `<figure>`,
/// keeping what is inside them, and `<strong>`, `<em>` and `<a>` are inline.
const BLOCKS: [&str; 2] = ["div", "p"];

/// What one element's markup at the start of a text draws.
#[derive(Debug, Default)]
pub struct Markup {
    /// The images, in order.
    pub images: Vec<Image>,
    /// The line boxes the images are drawn in, in order.
    pub lines: Vec<Line>,
    /// The byte length of the markup.
    pub len: usize,
    /// The elements the end of the text closed, innermost last, each with
    /// the `text-align` its `align` gives what it holds (see [`Line`]):
    /// their end tags may follow later in the document.
    pub open: Vec<(String, String)>,
}

/// The images that the element at the start of `text` draws: one `<img>`,
/// one `<picture>`, or one wrapper with only images, wrappers, `<br>`s and
/// comments inside. None when `text` does not start with one, or the element
/// holds anything else. `ends` says whether the end of `text` is the end of
/// the markup (a blank line, the end of a paragraph or document), which
/// closes the elements left open, as the end of an HTML parser's input does;
/// otherwise an unfinished element is none. `schemes` are the URL schemes a
/// source may name (see [`allowed`]). With `links`, the schemes an `<a>`'s
/// `href` may name, an `<a>` holding only images is read too; without, it is
/// anything else, as a reader that draws images only as blocks would lose
/// its link.
pub fn element(text: &str, schemes: &[&str], links: Option<&[&str]>, ends: bool) -> Option<Markup> {
    let first = tag(text)?;
    let link = links.is_some() && first.name == "a";
    let starts = first.name == "img" || first.name == "picture" || wrapper(&first.name) || link;
    if first.closing || !starts {
        return None;
    }
    let inline = matches!(first.name.as_str(), "strong" | "em" | "a");
    let mut markup = Markup::default();
    let mut open: Vec<Open> = Vec::new();
    // Each image's line: a `<p>` or `<div>` starting or ending, or a `<br>`,
    // starts the next one.
    let mut line = 0;
    let mut lines = Vec::new();
    let mut spaced = false;
    let mut next = Some(first);
    let mut at = 0;
    loop {
        let tag = match next.take() {
            Some(tag) => tag,
            None => {
                let skipped = space(&text[at..]);
                spaced |= skipped > 0;
                at += skipped;
                if let Some(len) = comment(&text[at..]) {
                    at += len;
                    continue;
                }
                if at == text.len() && ends {
                    markup.open = open
                        .iter()
                        .map(|o| (o.name.clone(), o.align.clone()))
                        .collect();
                    close(&mut open, 0, &mut markup.images, schemes);
                    break;
                }
                tag(&text[at..])?
            }
        };
        at += tag.len;
        let picture = open.last_mut().and_then(|o| o.picture.as_mut());
        match (tag.name.as_str(), tag.closing, picture) {
            ("img", false, picture) => {
                let (mut image, authored) = image(&tag, schemes);
                if let Some(picture) = picture.filter(|p| p.first.is_none()) {
                    picture.first = Some(markup.images.len());
                    picture.authored = authored;
                }
                image.link = open
                    .iter()
                    .rev()
                    .find_map(|o| o.link.clone())
                    .unwrap_or_default();
                image.spaced = spaced && lines.last().is_some_and(|(l, _, _)| *l == line);
                spaced = false;
                let align = open
                    .iter()
                    .rev()
                    .map(|o| o.align.as_str())
                    .find(|a| !a.is_empty());
                let paragraph = open.iter().any(|o| o.name == "p");
                lines.push((line, align.unwrap_or_default().to_string(), paragraph));
                markup.images.push(image);
            }
            ("source", false, Some(picture)) => picture.source(&tag),
            ("picture", false, None) => open.push(Open::new("picture", Some(Picture::default()))),
            (name, false, None) if wrapper(name) => {
                if CLOSES_P.contains(&name) {
                    if let Some(p) = open.iter().rposition(|o| o.name == "p") {
                        close(&mut open, p, &mut markup.images, schemes);
                        line += 1;
                    }
                }
                let mut element = Open::new(name, None);
                if BLOCKS.contains(&name) {
                    line += 1;
                    element.align = text_align(name, tag.attribute("align"));
                }
                open.push(element);
            }
            ("a", false, None) if links.is_some() => {
                // An `<a>` inside an `<a>` is one HTML would close first.
                if open.iter().any(|o| o.link.is_some()) {
                    return None;
                }
                let href = tag.attribute("href").unwrap_or_default();
                let allowed = links.is_some_and(|links| protocol_allowed(&href, links));
                let mut element = Open::new("a", None);
                element.link = Some(if allowed { href } else { String::new() });
                open.push(element);
            }
            ("br", false, None) => line += 1,
            (name, true, _) => match open.iter().rposition(|o| o.name == name) {
                Some(index) => {
                    if open[index..]
                        .iter()
                        .any(|o| BLOCKS.contains(&o.name.as_str()))
                    {
                        line += 1;
                    }
                    close(&mut open, index, &mut markup.images, schemes)
                }
                // A stray `</p>` is an empty paragraph; HTML ignores any
                // other end tag that closes nothing.
                None if name == "p" => return None,
                None => {}
            },
            _ => return None,
        }
        if open.is_empty() {
            break;
        }
    }
    if inline && markup.images.is_empty() {
        return None;
    }
    for (index, (id, align, paragraph)) in lines.iter().enumerate() {
        if index > 0 && lines[index - 1].0 == *id {
            if let Some(last) = markup.lines.last_mut() {
                last.end = index + 1;
            }
            continue;
        }
        markup.lines.push(Line {
            start: index,
            end: index + 1,
            align: align.clone(),
            continues: *id == 0,
            open: false,
            paragraph: *paragraph,
        });
    }
    if let (Some(last), Some((id, _, _))) = (markup.lines.last_mut(), lines.last()) {
        last.open = *id == line;
    }
    markup.len = at;
    Some(markup)
}

/// The `text-align` HTML's rendering maps a `<p>`'s or `<div>`'s `align` to
/// (HTML §15.3.3, §15.3.6): `left`, `right`, `center` (a `<div>`'s `middle`
/// too) and `justify`, compared without case; any other value is none.
fn text_align(name: &str, align: Option<String>) -> String {
    let align = align.unwrap_or_default().trim().to_ascii_lowercase();
    match align.as_str() {
        "left" | "right" | "center" | "justify" => align,
        "middle" if name == "div" => "center".to_string(),
        _ => String::new(),
    }
}

/// Whether `text` starts with an `<img` or `<picture` tag, or with links an
/// `<a` one, the cheap test before [`element`] where only an image may start.
pub fn starts_image(text: &str, links: bool) -> bool {
    starts_tag(text, &["img", "picture"]) || (links && starts_tag(text, &["a"]))
}

/// Whether `line` starts with a tag that may begin image markup: `<img`,
/// `<picture`, a wrapper's, or with links `<a`.
pub fn starts_element(line: &str, links: bool) -> bool {
    starts_image(line, links) || starts_tag(line, &WRAPPERS)
}

/// Whether `line` starts a CommonMark HTML block (spec 0.31 §4.6) of the
/// tags image markup may start with: a `<div>`, `<p>`, `<center>` or
/// `<figure>` tag (condition 6), or one whole tag alone on its line
/// (condition 7). Markup that starts none is inline HTML in a paragraph,
/// which HTML lays out as a `<p>` of its own.
pub fn starts_block(line: &str) -> bool {
    let line = line.trim_start();
    starts_tag(line, &CLOSES_P) || tag(line).is_some_and(|tag| line[tag.len..].trim().is_empty())
}

/// The names of the end tags `line` consists of, innermost first, when it
/// holds only end tags of wrappers and `<picture>`.
pub fn end_tags(line: &str) -> Option<Vec<String>> {
    let mut names = Vec::new();
    let mut at = space(line);
    while at < line.len() {
        let tag = tag(&line[at..]).filter(|t| t.closing && t.attributes.is_empty())?;
        if !wrapper(&tag.name) && tag.name != "picture" {
            return None;
        }
        names.push(tag.name);
        at += tag.len;
        at += space(&line[at..]);
    }
    (!names.is_empty()).then_some(names)
}

fn starts_tag(text: &str, names: &[&str]) -> bool {
    let bytes = text.as_bytes();
    names.iter().any(|name| {
        bytes.first() == Some(&b'<')
            && text
                .get(1..1 + name.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(name))
            && matches!(
                bytes.get(1 + name.len()),
                None | Some(b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
            )
    })
}

fn wrapper(name: &str) -> bool {
    WRAPPERS.contains(&name)
}

/// Whether a source may load: a URL with no scheme, a Windows drive path, or
/// one of `schemes`. A scheme is the text before the first `:`, unless a `/`,
/// `?` or `#` comes before it, and is compared case-sensitively, as the
/// sanitizer compares it. A drive path is a path, though the sanitizer reads
/// its letter as a scheme: T3 Code keeps such a path in its own attribute.
pub fn allowed(url: &str, schemes: &[&str]) -> bool {
    drive_path(url) || protocol_allowed(url, schemes)
}

/// The sanitizer's own test (`hast-util-sanitize`'s `safeProtocol`), which
/// an `<img>`'s `data-dark-src` and `data-light-src` pass through as written.
fn protocol_allowed(url: &str, schemes: &[&str]) -> bool {
    let Some(colon) = url.find(':') else {
        return true;
    };
    url[..colon].contains(['/', '?', '#']) || schemes.contains(&&url[..colon])
}

/// A `<source>`'s URL: T3 Code writes a drive path as a `file:` URL before
/// the sanitizer sees it.
fn source_allowed(url: &str, schemes: &[&str]) -> bool {
    if drive_path(url) {
        schemes.contains(&"file")
    } else {
        protocol_allowed(url, schemes)
    }
}

fn drive_path(url: &str) -> bool {
    let bytes = url.as_bytes();
    bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

/// An `<img>`, and whether it carries its own dark and light sources.
fn image(tag: &Tag, schemes: &[&str]) -> (Image, [bool; 2]) {
    let src = match (tag.attribute("src"), tag.attribute("data-local-src")) {
        (Some(src), _) if drive_path(&src) => src,
        (_, Some(local)) => local,
        (src, None) => src.unwrap_or_default(),
    };
    let themed = |name| {
        tag.attribute(name).map(|url| {
            if protocol_allowed(&url, schemes) {
                url
            } else {
                String::new()
            }
        })
    };
    let (dark, light) = (themed("data-dark-src"), themed("data-light-src"));
    let authored = [dark.is_some(), light.is_some()];
    let image = Image {
        src: if allowed(&src, schemes) {
            src
        } else {
            String::new()
        },
        alt: tag.attribute("alt").unwrap_or_default(),
        width: pixels(tag.attribute("width")),
        height: pixels(tag.attribute("height")),
        dark: dark.unwrap_or_default(),
        light: light.unwrap_or_default(),
        ..Image::default()
    };
    (image, authored)
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

/// An element still open while its markup is read: a `<picture>`'s
/// sources, a block's `text-align`, an `<a>`'s link (empty when refused).
struct Open {
    name: String,
    picture: Option<Picture>,
    align: String,
    link: Option<String>,
}

impl Open {
    fn new(name: &str, picture: Option<Picture>) -> Open {
        Open {
            name: name.to_string(),
            picture,
            align: String::new(),
            link: None,
        }
    }
}

/// Closes `open[index..]`. A `<picture>`'s colour-scheme sources belong to
/// its first `<img>`, the first of each scheme winning, as in a browser; a
/// source with any other `media` is never chosen.
fn close(open: &mut Vec<Open>, index: usize, images: &mut [Image], schemes: &[&str]) {
    for element in open.drain(index..) {
        let Some(picture) = element.picture else {
            continue;
        };
        let Some(first) = picture.first.map(|i| &mut images[i]) else {
            continue;
        };
        // A source the sanitizer refuses is gone, and a later one does not
        // take its place. The image's own attribute wins over a source,
        // even one the sanitizer then refuses.
        let keep = |url: Option<String>| {
            url.filter(|u| source_allowed(u, schemes))
                .unwrap_or_default()
        };
        if !picture.authored[0] {
            first.dark = keep(picture.dark);
        }
        if !picture.authored[1] {
            first.light = keep(picture.light);
        }
    }
}

/// A `<picture>` being read: the index of its first `<img>` and whether that
/// one has its own dark and light sources, and the first source of each
/// scheme.
#[derive(Default)]
struct Picture {
    first: Option<usize>,
    authored: [bool; 2],
    dark: Option<String>,
    light: Option<String>,
}

impl Picture {
    fn source(&mut self, tag: &Tag) {
        let scheme = tag.attribute("media").and_then(|m| color_scheme(&m));
        let url = tag.attribute("srcset").map(|s| first_candidate(&s));
        let slot = match scheme {
            Some(Scheme::Dark) => &mut self.dark,
            Some(Scheme::Light) => &mut self.light,
            None => return,
        };
        if slot.is_none() {
            *slot = url.filter(|u| !u.is_empty());
        }
    }
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

/// A `srcset`'s first URL: its first comma-separated candidate, up to the
/// first white space (a descriptor such as `2x` may follow).
fn first_candidate(srcset: &str) -> String {
    let candidate = srcset.split(',').next().unwrap_or("").trim();
    let url = candidate.split(char::is_whitespace).next().unwrap_or("");
    url.to_string()
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
