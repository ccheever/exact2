//! The Markdown reader's parser: text in, an ordered list of blocks out.
//!
//! Shared by both readers (LLP 1033): `apps/markdown` opens one file,
//! `apps/llp` opens a directory of them, and the document they draw is the
//! same document, parsed once here. Pure — no I/O, no host, no allocation
//! the caller cannot see — so the same code runs in the wasm the browser
//! fetches and in the static library macOS links.
//!
//! Deliberately a subset of CommonMark, and the subset is the one an
//! engineering corpus is written in: headings, paragraphs with emphasis,
//! links and code spans, fenced code, lists, block quotes, rules, images,
//! and pipe tables. Reference links, setext headings, footnotes, and inline
//! HTML are not interpreted. Raw HTML stays visible and inert, which is
//! what a reader that cannot render it should do rather than hide it, with
//! one exception: `<img>` and `<picture>` markup is images, read as T3
//! Code's sanitizer reads it (`html`; [`Html`] turns it off).
//!
//! @ref LLP 1033 (the reader, D9 raw HTML images), LLP 1115 §2 (names stay CSS)

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod entities;
pub mod html;
mod inline;
pub mod value;

pub use inline::Run;

/// What a block is. A document is a flat list of these: nesting is a
/// `depth`, never a tree, because Contract inlines components syntactically
/// and cannot recurse — and because a flat list is what a windowed reader
/// can measure one row at a time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    /// `# Heading` — `level` is 1 to 6.
    Heading,
    /// Running prose.
    #[default]
    Paragraph,
    /// A fenced block; `text` is its literal content and `info` its language.
    Code,
    /// `> quoted`; `depth` is how many `>` deep.
    Quote,
    /// `---`.
    Rule,
    /// A list item; `depth` is its nesting and `marker` what stands in front.
    Item,
    /// `![alt](src)` alone on a line, or `<img>` and `<picture>` markup;
    /// `href` is the source, `text` the alt.
    Image,
    /// One row of a pipe table; `cells` are its cells and `header` says which.
    TableRow,
    /// A line of raw HTML, shown literally and interpreted by nothing.
    Html,
}

impl Kind {
    /// The name Contract switches on (`if b.kind == "heading"`).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Heading => "heading",
            Kind::Paragraph => "paragraph",
            Kind::Code => "code",
            Kind::Quote => "quote",
            Kind::Rule => "rule",
            Kind::Item => "item",
            Kind::Image => "image",
            Kind::TableRow => "table-row",
            Kind::Html => "html",
        }
    }
}

/// One block of a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Block {
    /// Which kind.
    pub kind: Kind,
    /// A heading's level (1–6), a list item's or quote's nesting depth.
    pub depth: u32,
    /// What stands in front of a list item: `•`, or `3.`.
    pub marker: String,
    /// A code block's literal text, an image's alt, a raw HTML line.
    pub text: String,
    /// An image's source, a code block's language, a heading's anchor.
    pub href: String,
    /// The styled runs of a heading, paragraph, quote or item.
    pub runs: Vec<Run>,
    /// A table row's cells.
    pub cells: Vec<Vec<Run>>,
    /// Whether a table row is the header row.
    pub header: bool,
    /// The source an image shows in a dark appearance instead of `href`, or
    /// empty: a `<picture>`'s `prefers-color-scheme` choice. With one, `href`
    /// is the light appearance's.
    pub dark: String,
    /// An `<img>`'s authored `width` and `height` in CSS pixels, or 0.
    pub width: u32,
    /// See `width`.
    pub height: u32,
    /// The CSS `text-align` the `align` of a `<p>` or `<div>` around the
    /// block gives it (`left`, `right`, `center` or `justify`), or empty
    /// for the reader's own ([`html::Line`]).
    pub align: String,
    /// Whether the block is a line of raw HTML images that no `<p>` holds:
    /// at the document's top level or in a `<div>`. HTML gives such a line
    /// no margins, where a paragraph has its own.
    pub bare: bool,
}

/// A parsed document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    /// The first heading, or empty.
    pub title: String,
    /// Every block, in order.
    pub blocks: Vec<Block>,
}

impl Document {
    /// The headings, in order, as (level, text, index-of-the-block): the
    /// outline a reader puts beside the document.
    pub fn outline(&self) -> Vec<(u32, String, usize)> {
        self.blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == Kind::Heading)
            .map(|(i, b)| (b.depth, plain(&b.runs), i))
            .collect()
    }

    /// Every distinct link target in the document, in order of first use.
    pub fn links(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for run in self
            .blocks
            .iter()
            .flat_map(|b| b.runs.iter().chain(b.cells.iter().flatten()))
        {
            if !run.href.is_empty() && !seen.contains(&run.href) {
                seen.push(run.href.clone());
            }
        }
        seen
    }
}

/// The text of `runs` with every style dropped.
pub fn plain(runs: &[Run]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

/// Parse `source`. `link` is given every link target as written and returns
/// what the app should do with it — the identity function for a document
/// with no location, and a path resolver for one opened from disk.
pub fn parse(source: &str, link: &dyn Fn(&str) -> String) -> Document {
    parse_cancellable(source, link, &|| false).unwrap_or_default()
}

/// Which raw HTML a parse draws rather than shows as written.
#[derive(Clone, Copy)]
pub struct Html<'a> {
    /// Whether `<img>` and `<picture>` markup is images.
    pub images: bool,
    /// The URL schemes an image may load besides a scheme-less URL
    /// ([`html::allowed`]); a source naming another loads nothing.
    pub schemes: &'a [&'a str],
    /// The run an image inside a line of text becomes, given its sources as
    /// written; with none, its alt text, as a Markdown image inside a line
    /// is its alt text in a reader that draws images only as blocks. With
    /// one, images that HTML draws side by side are a paragraph of such
    /// runs, and an image may sit inside an `<a href>`.
    pub inline: Option<&'a dyn Fn(html::Image) -> Run>,
    /// The URL schemes an `<a>`'s `href` around images may name besides a
    /// scheme-less URL; one naming another is dropped and its images are
    /// drawn with no link, as the sanitizer drops it.
    pub links: &'a [&'a str],
}

impl Html<'_> {
    /// The schemes an `<a>` around images may name, when one is read: only
    /// with `inline`, as a reader that draws images as blocks would lose
    /// its link.
    pub(crate) fn links(&self) -> Option<&[&str]> {
        self.inline.map(|_| self.links)
    }
}

impl std::fmt::Debug for Html<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Html")
            .field("images", &self.images)
            .field("schemes", &self.schemes)
            .field("inline", &self.inline.is_some())
            .field("links", &self.links)
            .finish()
    }
}

impl Html<'static> {
    /// GitHub's: images, loaded over `http` and `https`.
    pub const GITHUB: Html<'static> = Html {
        images: true,
        schemes: &["http", "https"],
        inline: None,
        links: &["http", "https", "irc", "ircs", "mailto", "xmpp"],
    };
    /// Every line of raw HTML shown as written.
    pub const LITERAL: Html<'static> = Html {
        images: false,
        schemes: &[],
        inline: None,
        links: &[],
    };
}

struct Context<'a> {
    link: &'a dyn Fn(&str) -> String,
    scan: inline::Scan<'a>,
    html: Html<'a>,
}

impl Context<'_> {
    fn runs(&self, text: &str) -> Vec<Run> {
        inline::runs(text, self.link, &self.scan, self.html)
    }
}

/// Parse with cooperative cancellation. A cancelled parse returns no partial
/// document. The caller owns the generation; scanners check every 4 KiB and
/// block builders check between lines. File I/O is the caller's responsibility.
pub fn parse_cancellable(
    source: &str,
    link: &dyn Fn(&str) -> String,
    cancel: &dyn Fn() -> bool,
) -> Option<Document> {
    parse_with(source, link, cancel, Html::GITHUB)
}

/// [`parse_cancellable`], drawing the raw HTML `html` names.
pub fn parse_with(
    source: &str,
    link: &dyn Fn(&str) -> String,
    cancel: &dyn Fn() -> bool,
    html: Html<'_>,
) -> Option<Document> {
    let context = Context {
        link,
        scan: inline::Scan::new(cancel),
        html,
    };
    let mut doc = Document::default();
    let mut lines = Vec::new();
    for line in source.lines() {
        if context.scan.cancelled() {
            return None;
        }
        lines.push(line);
    }
    // The elements image markup left open, whose end tags draw nothing,
    // with the `text-align` each gives what it holds.
    let mut open: Vec<(String, String)> = Vec::new();
    // The blocks from `aligned` on take `align`, an open `<div>`'s.
    let (mut aligned, mut align) = (0, String::new());
    // The line of images the next image markup may go on with.
    let mut going: Option<Pending> = None;
    let mut at = 0;
    while at < lines.len() {
        if context.scan.cancelled() {
            return None;
        }
        inherit(&mut doc.blocks[aligned..], &align);
        aligned = doc.blocks.len();
        align = open
            .iter()
            .rev()
            .find(|(name, align)| name == "div" && !align.is_empty())
            .map_or_else(String::new, |(_, align)| align.clone());
        let line = lines[at];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if trimmed.is_empty() {
            at += 1;
            continue;
        }
        if let Some(fence) = fence_of(trimmed) {
            at = code_block(&lines, at, fence, indent, &context.scan, &mut doc);
            continue;
        }
        if is_rule(trimmed) {
            doc.blocks.push(Block {
                kind: Kind::Rule,
                ..Block::default()
            });
            at += 1;
            continue;
        }
        if let Some((level, text)) = heading_of(trimmed) {
            let runs = context.runs(text);
            if doc.title.is_empty() && level == 1 {
                doc.title = plain(&runs);
            }
            doc.blocks.push(Block {
                kind: Kind::Heading,
                depth: level,
                href: anchor(&plain(&runs)),
                runs,
                ..Block::default()
            });
            at += 1;
            continue;
        }
        if let Some(rows) = table_at(&lines, at, &context.scan) {
            at = table(&lines, at, rows, &context, &mut doc);
            continue;
        }
        if trimmed.starts_with('>') {
            at = quote(&lines, at, &context, &mut doc);
            continue;
        }
        if let Some((marker, rest)) = bullet_of(trimmed) {
            at = item(&lines, at, indent, marker, rest, &context, &mut doc);
            continue;
        }
        if let Some(image) = image_of(trimmed, context.link) {
            doc.blocks.push(image);
            at += 1;
            continue;
        }
        if let Some((images, boxes, next, left_open)) = html_images(&lines, at, &context) {
            for (
                index,
                html::Line {
                    start,
                    end,
                    align,
                    continues,
                    open: left,
                    paragraph,
                },
            ) in boxes.into_iter().enumerate()
            {
                let mut images = images[start..end].to_vec();
                let mut align = align;
                let mut bare = !paragraph;
                // A line left open goes on, a blank line or not, until a block comes.
                if let Some(before) = going
                    .take()
                    .filter(|p| index == 0 && continues && p.end == doc.blocks.len())
                {
                    doc.blocks.truncate(before.start);
                    images[0].spaced = true;
                    images.splice(0..0, before.images);
                    if align.is_empty() {
                        align = before.align;
                    }
                    bare = before.bare;
                }
                let start = doc.blocks.len();
                doc.blocks
                    .extend(line_blocks(images.clone(), &align, bare, &context));
                going = left.then_some(Pending {
                    start,
                    end: doc.blocks.len(),
                    images,
                    align,
                    bare,
                });
            }
            open.extend(left_open);
            at = next;
            continue;
        }
        if closes_open(trimmed, &mut open) {
            going = None;
            at += 1;
            continue;
        }
        if trimmed.starts_with('<') && trimmed.len() > 2 && !inline_image(trimmed, &context) {
            doc.blocks.push(Block {
                kind: Kind::Html,
                text: trimmed.to_string(),
                ..Block::default()
            });
            at += 1;
            continue;
        }
        at = paragraph(&lines, at, &context, &mut doc);
    }
    inherit(&mut doc.blocks[aligned..], &align);
    (!context.scan.cancelled()).then_some(doc)
}

/// A line of images the image markup after it may go on with (see
/// [`html::Line`]): its blocks are `doc.blocks[start..end]`.
struct Pending {
    start: usize,
    end: usize,
    images: Vec<html::Image>,
    align: String,
    bare: bool,
}

/// `align` for each block that has none of its own: a `<div align>` left
/// open holds them, as HTML's tree does.
fn inherit(blocks: &mut [Block], align: &str) {
    if align.is_empty() {
        return;
    }
    for block in blocks.iter_mut().filter(|b| b.align.is_empty()) {
        block.align = align.to_string();
    }
}

/// The blocks of one line of images. A reader that draws images inside a
/// line gets one paragraph of its runs, a space between two images that
/// white space parts, for two images or more, or one inside a link; any
/// other draws each image as a block, one under the other.
fn line_blocks(
    images: Vec<html::Image>,
    align: &str,
    bare: bool,
    context: &Context<'_>,
) -> Vec<Block> {
    match context.html.inline {
        Some(run) if images.len() > 1 || images.iter().any(|i| !i.link.is_empty()) => {
            let mut runs = Vec::with_capacity(images.len() * 2);
            for (index, image) in images.into_iter().enumerate() {
                if index > 0 && image.spaced {
                    runs.push(Run::text(" "));
                }
                runs.push(run(image));
            }
            vec![Block {
                kind: Kind::Paragraph,
                runs,
                align: align.to_string(),
                bare,
                ..Block::default()
            }]
        }
        _ => images
            .into_iter()
            .map(|image| Block {
                align: align.to_string(),
                bare,
                ..image_block(image, context.link)
            })
            .collect(),
    }
}

/// The fence a line opens (its character and its length), or none.
fn fence_of(line: &str) -> Option<(char, usize)> {
    for marker in ['`', '~'] {
        let count = line.chars().take_while(|&c| c == marker).count();
        if count >= 3 {
            return Some((marker, count));
        }
    }
    None
}

fn code_block(
    lines: &[&str],
    at: usize,
    (marker, width): (char, usize),
    indent: usize,
    scan: &inline::Scan<'_>,
    doc: &mut Document,
) -> usize {
    let info = lines[at].trim_start().trim_start_matches(marker).trim();
    let mut text = String::new();
    let mut i = at + 1;
    while i < lines.len() {
        if scan.cancelled() {
            return lines.len();
        }
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.chars().take_while(|&c| c == marker).count() >= width
            && trimmed.trim_end_matches(marker).is_empty()
        {
            i += 1;
            break;
        }
        // The fence's own indentation is the block's left edge, not content.
        let cut = line.len() - line.trim_start().len();
        text.push_str(&line[cut.min(indent)..]);
        text.push('\n');
        i += 1;
    }
    doc.blocks.push(Block {
        kind: Kind::Code,
        text: text.trim_end_matches('\n').to_string(),
        href: info.to_string(),
        ..Block::default()
    });
    i
}

fn is_rule(line: &str) -> bool {
    let mut marks = line.bytes().filter(|&b| b != b' ');
    let Some(marker @ (b'-' | b'*' | b'_')) = marks.next() else {
        return false;
    };
    let mut count = 1;
    for mark in marks {
        if mark != marker {
            return false;
        }
        count += 1;
    }
    count >= 3
}

fn heading_of(line: &str) -> Option<(u32, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    // `#hashtag` is not a heading: ATX needs the space.
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }
    Some((hashes as u32, rest.trim().trim_end_matches('#').trim_end()))
}

/// GitHub's heading anchor: lowercased, punctuation dropped, spaces to dashes.
fn anchor(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// `- `, `* `, `+ `, `1. ` — the marker to show and the text after it.
fn bullet_of(line: &str) -> Option<(String, &str)> {
    for marker in ['-', '*', '+'] {
        if let Some(rest) = line.strip_prefix(marker) {
            if rest.starts_with(' ') {
                return Some(("•".to_string(), rest.trim_start()));
            }
        }
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 9 {
        let rest = &line[digits..];
        for close in ['.', ')'] {
            if let Some(after) = rest.strip_prefix(close) {
                if after.starts_with(' ') {
                    return Some((format!("{}{close}", &line[..digits]), after.trim_start()));
                }
            }
        }
    }
    None
}

fn item(
    lines: &[&str],
    at: usize,
    indent: usize,
    marker: String,
    first: &str,
    context: &Context<'_>,
    doc: &mut Document,
) -> usize {
    let mut text = first.to_string();
    let mut i = at + 1;
    // A wrapped item continues on a line indented past its marker that does
    // not itself start a block. Anything else ends it.
    while i < lines.len() {
        if context.scan.cancelled() {
            return lines.len();
        }
        let line = lines[i];
        let trimmed = line.trim_start();
        let next_indent = line.len() - trimmed.len();
        if trimmed.is_empty()
            || next_indent <= indent
            || bullet_of(trimmed).is_some()
            || heading_of(trimmed).is_some()
            || fence_of(trimmed).is_some()
            || trimmed.starts_with('>')
        {
            break;
        }
        text.push(' ');
        text.push_str(trimmed);
        i += 1;
    }
    doc.blocks.push(Block {
        kind: Kind::Item,
        // Two spaces is one level in every corpus this reads; four is one
        // level in CommonMark's, and dividing by two puts both at a depth
        // that reads correctly.
        depth: (indent / 2) as u32,
        marker,
        runs: context.runs(&text),
        ..Block::default()
    });
    i
}

fn quote(lines: &[&str], at: usize, context: &Context<'_>, doc: &mut Document) -> usize {
    let mut i = at;
    let mut text = String::new();
    let mut depth = 0;
    while i < lines.len() {
        if context.scan.cancelled() {
            return lines.len();
        }
        let trimmed = lines[i].trim_start();
        if !trimmed.starts_with('>') {
            break;
        }
        let markers = trimmed.chars().take_while(|&c| c == '>').count() as u32;
        depth = depth.max(markers);
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(trimmed.trim_start_matches('>').trim());
        i += 1;
    }
    doc.blocks.push(Block {
        kind: Kind::Quote,
        depth,
        runs: context.runs(text.trim()),
        ..Block::default()
    });
    i
}

/// Whether a pipe table starts here, and how many rows it has. A table is a
/// header row and a `| --- | --- |` rule under it; without the rule a line
/// with pipes in it is a paragraph, which is what a shell command written in
/// prose needs it to be.
fn table_at(lines: &[&str], at: usize, scan: &inline::Scan<'_>) -> Option<usize> {
    if !lines[at].contains('|') {
        return None;
    }
    let rule = lines.get(at + 1)?.trim();
    if !rule.contains('|') || !rule.contains('-') {
        return None;
    }
    if !rule
        .chars()
        .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
    {
        return None;
    }
    let mut rows = 2;
    while let Some(line) = lines.get(at + rows) {
        if scan.cancelled() {
            return None;
        }
        if !line.contains('|') || line.trim().is_empty() {
            break;
        }
        rows += 1;
    }
    Some(rows)
}

fn table(
    lines: &[&str],
    at: usize,
    rows: usize,
    context: &Context<'_>,
    doc: &mut Document,
) -> usize {
    for (offset, line) in lines[at..at + rows].iter().enumerate() {
        if context.scan.cancelled() {
            return lines.len();
        }
        if offset == 1 {
            continue; // the `| --- |` rule is grammar, not a row
        }
        doc.blocks.push(Block {
            kind: Kind::TableRow,
            header: offset == 0,
            cells: cells_of(line, context),
            ..Block::default()
        });
    }
    at + rows
}

fn cells_of(line: &str, context: &Context<'_>) -> Vec<Vec<Run>> {
    let trimmed = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for (index, c) in trimmed.chars().enumerate() {
        if index.is_multiple_of(4096) && context.scan.cancelled() {
            return Vec::new();
        }
        match c {
            '\\' if !escaped => escaped = true,
            '|' if !escaped => cells.push(std::mem::take(&mut cell)),
            _ => {
                if escaped && c != '|' {
                    cell.push('\\');
                }
                escaped = false;
                cell.push(c);
            }
        }
    }
    cells.push(cell);
    cells.iter().map(|c| context.runs(c.trim())).collect()
}

/// `![alt](src)` alone on a line — the only place an image is a block.
fn image_of(line: &str, link: &dyn Fn(&str) -> String) -> Option<Block> {
    let rest = line.strip_prefix("![")?;
    let close = rest.find("](")?;
    if !rest.ends_with(')') {
        return None;
    }
    let alt = &rest[..close];
    let src = rest[close + 2..rest.len() - 1].trim();
    let src = match src.find(['"', '\'']) {
        Some(cut) => src[..cut].trim(),
        None => src,
    };
    Some(Block {
        kind: Kind::Image,
        text: alt.to_string(),
        href: link(src),
        ..Block::default()
    })
}

fn paragraph(lines: &[&str], at: usize, context: &Context<'_>, doc: &mut Document) -> usize {
    let mut text = String::new();
    let mut i = at;
    while i < lines.len() {
        if context.scan.cancelled() {
            return lines.len();
        }
        let trimmed = lines[i].trim();
        if trimmed.is_empty()
            || (i > at
                && (heading_of(trimmed).is_some()
                    || fence_of(trimmed).is_some()
                    || is_rule(trimmed)
                    || bullet_of(trimmed).is_some()
                    || trimmed.starts_with('>')
                    || table_at(lines, i, &context.scan).is_some()
                    || html_images(lines, i, context).is_some()))
        {
            break;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(trimmed);
        i += 1;
    }
    doc.blocks.push(Block {
        kind: Kind::Paragraph,
        runs: context.runs(&text),
        ..Block::default()
    });
    i
}

/// The `<img>` and `<picture>` markup that starts line `at`: its images,
/// their line boxes, the line after it, and the elements its end left open
/// with their `text-align`. The
/// markup may run over several lines, up to the next blank one, and its last
/// element must end its line; the lines after that element are the rest of
/// the document's. The blank line or the document's end closes what is still
/// open, as an HTML parser's end of input does.
///
/// A paragraph ends before such a line, which CommonMark would read as inline
/// HTML in the paragraph: a reader of blocks draws an image only as one.
#[allow(clippy::type_complexity)]
fn html_images(
    lines: &[&str],
    at: usize,
    context: &Context<'_>,
) -> Option<(
    Vec<html::Image>,
    Vec<html::Line>,
    usize,
    Vec<(String, String)>,
)> {
    let links = context.html.links();
    if !context.html.images || !html::starts_element(lines[at].trim_start(), links.is_some()) {
        return None;
    }
    let mut text = String::new();
    let mut ends = true;
    for line in &lines[at..] {
        if line.trim().is_empty() {
            break;
        }
        if text.len() + line.len() > html::MARKUP_LIMIT {
            ends = false;
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(line);
    }
    let mut images = Vec::new();
    let mut boxes: Vec<html::Line> = Vec::new();
    let mut found = None;
    let mut offset = 0;
    loop {
        if context.scan.cancelled() {
            return None;
        }
        let skipped = text[offset..].len() - text[offset..].trim_start().len();
        offset += skipped;
        let Some(markup) = html::element(&text[offset..], context.html.schemes, links, ends) else {
            break;
        };
        offset += markup.len;
        let base = images.len();
        for (index, mut line) in markup.lines.into_iter().enumerate() {
            line.start += base;
            line.end += base;
            match boxes.last_mut() {
                // Elements one after another, with no block between, share a line.
                Some(last) if index == 0 && line.continues && last.open => {
                    last.end = line.end;
                    last.open = line.open;
                }
                _ => boxes.push(line),
            }
        }
        let mut drawn = markup.images;
        if let Some(first) = drawn.first_mut() {
            first.spaced |= skipped > 0;
        }
        images.extend(drawn);
        let line_end = text[offset..].find('\n').map_or(text.len(), |n| offset + n);
        if text[offset..line_end].trim().is_empty() {
            let line = at + text[..line_end].matches('\n').count();
            found = Some((images.len(), boxes.clone(), line + 1, markup.open));
        }
    }
    let (count, mut boxes, next, open) = found?;
    images.truncate(count);
    // Inline HTML is a paragraph of its own, which no image outside it shares a line with.
    if !html::starts_block(lines[at]) {
        if let Some(first) = boxes.first_mut() {
            first.continues = false;
        }
        if let Some(last) = boxes.last_mut() {
            last.open = false;
        }
        for line in &mut boxes {
            line.paragraph = true;
        }
    }
    Some((images, boxes, next, open))
}

/// Whether `line` is the end tags of elements image markup left open, which
/// close them and draw nothing, as T3 Code's HTML parser reads them.
fn closes_open(line: &str, open: &mut Vec<(String, String)>) -> bool {
    let Some(names) = html::end_tags(line) else {
        return false;
    };
    let closes =
        names.len() <= open.len() && names.iter().zip(open.iter().rev()).all(|(a, b)| *a == b.0);
    if closes {
        open.truncate(open.len() - names.len());
    }
    closes
}

/// Whether `line` starts with an image that the paragraph it opens draws
/// inline, with text after it on the line: CommonMark's inline HTML.
fn inline_image(line: &str, context: &Context<'_>) -> bool {
    let links = context.html.links();
    if !context.html.images || !html::starts_image(line, links.is_some()) {
        return false;
    }
    let mut limit = line.len().min(html::MARKUP_LIMIT);
    while !line.is_char_boundary(limit) {
        limit -= 1;
    }
    html::element(&line[..limit], context.html.schemes, links, false).is_some()
}

fn image_block(image: html::Image, link: &dyn Fn(&str) -> String) -> Block {
    let resolve = |url: &str| {
        if url.is_empty() {
            String::new()
        } else {
            link(url)
        }
    };
    let themed = !image.dark.is_empty() || !image.light.is_empty();
    let light = if image.light.is_empty() {
        &image.src
    } else {
        &image.light
    };
    let dark = if image.dark.is_empty() {
        &image.src
    } else {
        &image.dark
    };
    Block {
        kind: Kind::Image,
        text: image.alt,
        href: resolve(light),
        dark: if themed { resolve(dark) } else { String::new() },
        width: image.width,
        height: image.height,
        ..Block::default()
    }
}
