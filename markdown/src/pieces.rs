//! The reader's flattening: a source string into display pieces one paragraph
//! engine paints as a single run sequence.
//!
//! @ref LLP 1045 D3, D4 — a host that measures and paints styled runs already
//! can show a whole Markdown segment as one node if something turns the
//! source into runs. This is that something, shared by every host, so what
//! is measured is what is painted. Block structure that runs cannot express
//! (bullets, boxes, rules, quote bars, footnote marks) becomes glyph pieces;
//! a heading is a bigger, bolder run; spacing between blocks is a short line.
//! A list item is the one paragraph attribute: its pieces carry the item's
//! head indent and its marker hangs in it, which every host applies as the
//! browser lays out `<ul>`/`<ol>` (LLP 1045 D4). Colours are roles the host
//! resolves from its theme.

use crate::block::{BlockKind, MarkerKind, B};
use crate::style::{analyze, Replacement};
use crate::{BOLD, CODE, ITALIC, LINK, MARKER, STRIKE};

/// The colour a piece takes, resolved by the host from its theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Body text.
    Ink,
    /// Code, inline and block.
    Code,
    /// A link.
    Link,
    /// Bullets, boxes, rules, quote bars, footnote marks: quieter than ink.
    Marker,
    /// Quoted text.
    Quote,
}

/// One run of display text.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// The text, exactly as painted; may end in a newline.
    pub text: String,
    /// Font size relative to the node's own.
    pub scale: f32,
    /// CSS weight; 0 keeps the node's own.
    pub weight: u16,
    /// Italic.
    pub italic: bool,
    /// Monospace.
    pub mono: bool,
    /// Strikethrough.
    pub strike: bool,
    /// A link's target; empty otherwise.
    pub href: String,
    /// Its colour.
    pub role: Role,
    /// Where in the source it came from, in UTF-16 units; `None` for a
    /// synthesized glyph such as a bullet.
    pub source: Option<crate::Range>,
    /// The head indent, CSS px, of the paragraph (the line up to a newline)
    /// this piece is in: every line of it starts this far in. A list item's
    /// is [`LIST_INDENT`] per level, as the browser's `padding-inline-start:
    /// 40px` on each `<ul>`/`<ol>`; 0 elsewhere. A host takes a paragraph's
    /// from its first piece.
    pub indent: f32,
    /// A list item's marker (`• `, `1. `, `☐ `): the first piece of its
    /// paragraph, outside the indent — its end at the indent, so it is
    /// right-aligned in the gutter and a wrapped line starts under the text,
    /// as the browser's `list-style-position: outside` marker sits.
    pub hang: bool,
}

/// One list level's indent, CSS px: the browser's `padding-inline-start` on
/// `<ul>` and `<ol>`.
pub const LIST_INDENT: f32 = 40.0;

impl Piece {
    fn glyph(text: &str, role: Role) -> Piece {
        Piece {
            text: text.to_string(),
            scale: 1.0,
            weight: 0,
            italic: false,
            mono: false,
            strike: false,
            href: String::new(),
            role,
            source: None,
            indent: 0.0,
            hang: false,
        }
    }
}

/// A list item's marker at `depth` (0 the outermost), as the browser's UA
/// sheet chooses it for nested `<ul>`s: disc, circle, then square, and its
/// scale. No system face has the square; a fallback face's taller line box
/// would open the line it is on, so it is drawn at three quarters.
fn bullet(depth: usize) -> (&'static str, f32) {
    match depth {
        0 => ("• ", 1.0),
        1 => ("◦ ", 1.0),
        _ => ("▪ ", 0.75),
    }
}

/// The heading scale: h1 largest, h6 the body size, all bold.
fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.6,
        2 => 1.4,
        3 => 1.2,
        4 => 1.1,
        _ => 1.0,
    }
}

/// The blank between two blocks: a short line, about half a line of body.
const GAP: f32 = 0.5;

fn advance_ranges<T>(ranges: &mut &[T], at: usize, bounds: fn(&T) -> &B) {
    while ranges.first().is_some_and(|entry| bounds(entry).end <= at) {
        *ranges = &ranges[1..];
    }
}

fn block_ranges<'a, T>(pending: &mut &'a [T], block: &B, bounds: fn(&T) -> &B) -> &'a [T] {
    advance_ranges(pending, block.start, bounds);
    let remaining = *pending;
    &remaining[..remaining.partition_point(|entry| bounds(entry).start < block.end)]
}

/// Flattens `source` into pieces for one paragraph engine.
pub fn pieces(source: &str) -> Vec<Piece> {
    let analysis = analyze(source, None);
    let offsets = crate::offsets::Offsets::new(source);
    let mut out: Vec<Piece> = Vec::new();
    let mut previous: Option<BlockKind> = None;
    let blocks = &analysis.blocks;
    let mut remaining_spans = analysis.spans.as_slice();
    let mut remaining_hidden = analysis.hidden.as_slice();
    let mut remaining_replaced = analysis.replaced.as_slice();
    for (n, block) in blocks.iter().enumerate() {
        let (kind, depth, quote) = (&block.kind, block.depth as usize, block.quote as usize);
        // Spacing: none inside a run of list items or code lines, a short line
        // otherwise. A blank line between items ends the list or makes it
        // loose, and either way the browser puts the items' margins between.
        let adjacent = n > 0
            && !source
                .get(blocks[n - 1].range.end..block.range.start)
                .is_some_and(|between| between.contains("\n\n"));
        let tight = matches!(
            (&previous, kind),
            (
                Some(BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task(_)),
                BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task(_)
            ) if adjacent
        ) || matches!(
            (&previous, kind),
            (
                Some(BlockKind::Fence | BlockKind::Code),
                BlockKind::Code | BlockKind::Fence
            ) | (Some(BlockKind::Table), BlockKind::Table)
        );
        if n > 0 && !tight {
            let mut gap = Piece::glyph("\n", Role::Ink);
            gap.scale = GAP;
            out.push(gap);
        }
        previous = Some(kind.clone());
        if matches!(kind, BlockKind::Fence) {
            continue;
        }
        let list = matches!(
            kind,
            BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task(_)
        );
        // A list item's lines all start at its indent; quote bars and the
        // marker hang before it. Elsewhere the quote bars lead the line.
        let indent = if list {
            LIST_INDENT * (depth + 1) as f32
        } else {
            0.0
        };
        let bars = "▎ ".repeat(quote);
        let marker = match kind {
            BlockKind::Bullet => bullet(depth).0.to_string(),
            BlockKind::Task(true) => "☑ ".to_string(),
            BlockKind::Task(false) => "☐ ".to_string(),
            // The source's own number and delimiter, `1.` or `1)`.
            BlockKind::Ordered => block
                .markers
                .iter()
                .find(|(_, m)| matches!(m, MarkerKind::Ordered))
                .map(|(r, _)| format!("{} ", &source[r.clone()]))
                .unwrap_or_default(),
            BlockKind::Rule => "──────────".to_string(),
            _ => String::new(),
        };
        let mut prefix = Piece::glyph(&format!("{bars}{marker}"), Role::Marker);
        prefix.indent = indent;
        prefix.hang = list;
        if matches!(kind, BlockKind::Bullet) {
            prefix.scale = bullet(depth).1;
        }
        if !prefix.text.is_empty() {
            out.push(prefix);
        }
        let (scale, weight) = match kind {
            BlockKind::Heading(level) => (heading_scale(*level), 700),
            _ => (1.0, 0),
        };
        let role = match kind {
            BlockKind::Code | BlockKind::Table => Role::Code,
            _ if quote > 0 => Role::Quote,
            _ => Role::Ink,
        };
        let mono = matches!(kind, BlockKind::Code | BlockKind::Table);
        if matches!(kind, BlockKind::Rule) {
            out.push(Piece::glyph("\n", Role::Ink));
            continue;
        }
        // The block's text, cut at every span, hidden range and replacement;
        // the markers are hidden or replaced, so the block's whole range serves.
        let mut range = block.range.clone();
        if list {
            // The marker glyph stands for the source up to the item's text.
            range.start = block.content.first().map_or(range.end, |c| c.start);
        }
        let mut spans = block_ranges(&mut remaining_spans, &range, |s| &s.0);
        let mut hidden = block_ranges(&mut remaining_hidden, &range, |r| r);
        let mut replaced = block_ranges(&mut remaining_replaced, &range, |r| &r.0);
        let mut cuts: Vec<usize> = vec![range.start, range.end];
        for (r, _, _) in spans {
            cuts.extend([r.start, r.end]);
        }
        for r in hidden {
            cuts.extend([r.start, r.end]);
        }
        for (r, _) in replaced {
            cuts.extend([r.start, r.end]);
        }
        cuts.retain(|&c| c >= range.start && c <= range.end);
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            // All range endpoints are cuts. The first unexpired range is
            // therefore the first covering range, even if ranges overlap.
            advance_ranges(&mut hidden, start, |r| r);
            advance_ranges(&mut replaced, start, |r| &r.0);
            advance_ranges(&mut spans, start, |s| &s.0);
            if hidden
                .first()
                .is_some_and(|h| h.start <= start && end <= h.end)
            {
                continue;
            }
            if let Some((r, with)) = replaced
                .first()
                .filter(|(r, _)| r.start <= start && end <= r.end)
            {
                if start == r.start {
                    // A definition's label keeps its trailing space.
                    let spaced = source[start..end].ends_with(' ');
                    let (text, small) = match with {
                        Replacement::Bullet | Replacement::TaskBox(_) | Replacement::Rule => {
                            (String::new(), false)
                        }
                        Replacement::Footnote(n) => {
                            (format!("[{n}]{}", if spaced { " " } else { "" }), true)
                        }
                    };
                    if !text.is_empty() {
                        let mut p = Piece::glyph(&text, Role::Marker);
                        p.scale = if small { 0.75 } else { 1.0 };
                        p.indent = indent;
                        out.push(p);
                    }
                }
                continue;
            }
            let mut text = source[start..end].to_string();
            // A continuation line of a list item starts at the item's indent;
            // one of a quote keeps the bars in place of its own indent.
            if text.contains('\n') && !mono {
                let lead = if list { String::new() } else { bars.clone() };
                let mut lines = text.split('\n');
                let mut joined = lines.next().unwrap_or("").to_string();
                for line in lines {
                    joined.push('\n');
                    joined.push_str(&lead);
                    joined.push_str(line.trim_start());
                }
                text = joined;
            }
            let span = spans
                .first()
                .filter(|(r, _, _)| r.start <= start && end <= r.end);
            let flags = span.map_or(0, |s| s.1);
            let href = span.map(|s| s.2.clone()).unwrap_or_default();
            out.push(Piece {
                text,
                scale: scale * if flags & CODE != 0 { 0.92 } else { 1.0 },
                weight: if flags & BOLD != 0 { 700 } else { weight },
                italic: flags & ITALIC != 0,
                mono: mono || flags & CODE != 0,
                strike: flags & STRIKE != 0,
                href: if flags & LINK != 0 {
                    href
                } else {
                    String::new()
                },
                role: if flags & LINK != 0 {
                    Role::Link
                } else if flags & CODE != 0 {
                    Role::Code
                } else if flags & MARKER != 0 {
                    Role::Marker
                } else {
                    role
                },
                source: Some(offsets.range16(&(start..end))),
                indent,
                hang: false,
            });
        }
        let mut end = Piece::glyph("\n", Role::Ink);
        end.indent = indent;
        out.push(end);
    }
    // Merge neighbours that paint alike, and drop the final newline.
    let mut merged: Vec<Piece> = Vec::with_capacity(out.len());
    for p in out {
        match merged.last_mut() {
            Some(last)
                if last.scale == p.scale
                    && last.weight == p.weight
                    && last.italic == p.italic
                    && last.mono == p.mono
                    && last.strike == p.strike
                    && last.href == p.href
                    && last.role == p.role
                    && last.indent == p.indent
                    && !last.hang
                    && !p.hang
                    && last.source.is_some() == p.source.is_some()
                    && last
                        .source
                        .as_ref()
                        .zip(p.source.as_ref())
                        .is_none_or(|(a, b)| a.end == b.start) =>
            {
                last.text.push_str(&p.text);
                if let (Some(a), Some(b)) = (&mut last.source, &p.source) {
                    a.end = b.end;
                }
            }
            _ => merged.push(p),
        }
    }
    if let Some(last) = merged.last_mut() {
        if last.text.ends_with('\n') {
            last.text.pop();
            if last.text.is_empty() {
                merged.pop();
            }
        }
    }
    merged
}
