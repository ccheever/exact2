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
//! HTML are not interpreted; raw HTML stays visible and inert, which is
//! what a reader that cannot render it should do rather than hide it.
//!
//! @ref LLP 1033 (the reader), `rules/RULES.md` §Scope (the web is the standard)

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod inline;
pub mod theme;
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
    /// `![alt](src)` alone on a line; `href` is the source, `text` the alt.
    Image,
    /// One row of a pipe table; `cells` are its cells and `header` says which.
    TableRow,
    /// A line of raw HTML, shown literally and interpreted by nothing.
    Html,
}

impl Kind {
    /// The name Contract switches on (`when b.kind == "heading"`).
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
    let mut doc = Document::default();
    let lines: Vec<&str> = source.lines().collect();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if trimmed.is_empty() {
            at += 1;
            continue;
        }
        if let Some(fence) = fence_of(trimmed) {
            at = code_block(&lines, at, fence, indent, &mut doc);
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
            let runs = inline::runs(text, link);
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
        if let Some(rows) = table_at(&lines, at) {
            at = table(&lines, at, rows, link, &mut doc);
            continue;
        }
        if trimmed.starts_with('>') {
            at = quote(&lines, at, link, &mut doc);
            continue;
        }
        if let Some((marker, rest)) = bullet_of(trimmed) {
            at = item(&lines, at, indent, marker, rest, link, &mut doc);
            continue;
        }
        if let Some(image) = image_of(trimmed, link) {
            doc.blocks.push(image);
            at += 1;
            continue;
        }
        if trimmed.starts_with('<') && trimmed.len() > 2 {
            doc.blocks.push(Block {
                kind: Kind::Html,
                text: trimmed.to_string(),
                ..Block::default()
            });
            at += 1;
            continue;
        }
        at = paragraph(&lines, at, link, &mut doc);
    }
    doc
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
    doc: &mut Document,
) -> usize {
    let info = lines[at].trim_start().trim_start_matches(marker).trim();
    let mut text = String::new();
    let mut i = at + 1;
    while i < lines.len() {
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
    link: &dyn Fn(&str) -> String,
    doc: &mut Document,
) -> usize {
    let mut text = first.to_string();
    let mut i = at + 1;
    // A wrapped item continues on a line indented past its marker that does
    // not itself start a block. Anything else ends it.
    while i < lines.len() {
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
        runs: inline::runs(&text, link),
        ..Block::default()
    });
    i
}

fn quote(lines: &[&str], at: usize, link: &dyn Fn(&str) -> String, doc: &mut Document) -> usize {
    let mut i = at;
    let mut text = String::new();
    let mut depth = 0;
    while i < lines.len() {
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
        runs: inline::runs(text.trim(), link),
        ..Block::default()
    });
    i
}

/// Whether a pipe table starts here, and how many rows it has. A table is a
/// header row and a `| --- | --- |` rule under it; without the rule a line
/// with pipes in it is a paragraph, which is what a shell command written in
/// prose needs it to be.
fn table_at(lines: &[&str], at: usize) -> Option<usize> {
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
    link: &dyn Fn(&str) -> String,
    doc: &mut Document,
) -> usize {
    for (offset, line) in lines[at..at + rows].iter().enumerate() {
        if offset == 1 {
            continue; // the `| --- |` rule is grammar, not a row
        }
        doc.blocks.push(Block {
            kind: Kind::TableRow,
            header: offset == 0,
            cells: cells_of(line, link),
            ..Block::default()
        });
    }
    at + rows
}

fn cells_of(line: &str, link: &dyn Fn(&str) -> String) -> Vec<Vec<Run>> {
    let trimmed = line.trim().trim_start_matches('|').trim_end_matches('|');
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for c in trimmed.chars() {
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
    cells.iter().map(|c| inline::runs(c.trim(), link)).collect()
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

fn paragraph(
    lines: &[&str],
    at: usize,
    link: &dyn Fn(&str) -> String,
    doc: &mut Document,
) -> usize {
    let mut text = String::new();
    let mut i = at;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty()
            || (i > at
                && (heading_of(trimmed).is_some()
                    || fence_of(trimmed).is_some()
                    || is_rule(trimmed)
                    || bullet_of(trimmed).is_some()
                    || trimmed.starts_with('>')
                    || table_at(lines, i).is_some()))
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
        runs: inline::runs(&text, link),
        ..Block::default()
    });
    i
}
