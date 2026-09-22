//! The reader's flattening: a source string into display pieces one paragraph
//! engine paints as a single run sequence.
//!
//! @ref LLP 1045 D3, D4 — a host that measures and paints styled runs already
//! can show a whole Markdown segment as one node if something turns the
//! source into runs. This is that something, shared by every host, so what
//! is measured is what is painted. Block structure that runs cannot express
//! (indents, bullets, boxes, rules, quote bars, footnote marks) becomes glyph
//! pieces; a heading is a bigger, bolder run; spacing between blocks is a
//! short line. Colours are roles the host resolves from its theme.

use crate::block::BlockKind;
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
}

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
        }
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

/// Flattens `source` into pieces for one paragraph engine.
pub fn pieces(source: &str) -> Vec<Piece> {
    let analysis = analyze(source, None);
    let offsets = crate::offsets::Offsets::new(source);
    let mut out: Vec<Piece> = Vec::new();
    let mut previous: Option<BlockKind> = None;
    let blocks = &analysis.blocks;
    let (mut si, mut hi, mut ri) = (0, 0, 0);
    for (n, block) in blocks.iter().enumerate() {
        let (kind, depth, quote) = (&block.kind, block.depth as usize, block.quote as usize);
        // Spacing: none inside a run of list items or code lines, a short line otherwise.
        let tight = matches!(
            (&previous, kind),
            (
                Some(BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task(_)),
                BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task(_)
            ) | (
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
        let lead = format!("{}{}", "▎ ".repeat(quote), "    ".repeat(depth));
        let mut prefix = Piece::glyph(&lead, Role::Marker);
        // The block's own marker glyph.
        let marker = match kind {
            // Each glyph replaces a marker such as `-`, whose own following
            // space the source still carries.
            BlockKind::Bullet => "• ",
            BlockKind::Task(true) => "☑ ",
            BlockKind::Task(false) => "☐ ",
            BlockKind::Rule => "──────────",
            _ => "",
        };
        prefix.text.push_str(marker);
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
        let range = block.range.clone();
        // All three analyses are ordered. Keep cursors across blocks so a
        // many-paragraph document never scans its whole range tables per block.
        while si < analysis.spans.len() && analysis.spans[si].0.end <= range.start {
            si += 1;
        }
        while hi < analysis.hidden.len() && analysis.hidden[hi].end <= range.start {
            hi += 1;
        }
        while ri < analysis.replaced.len() && analysis.replaced[ri].0.end <= range.start {
            ri += 1;
        }
        let spans = &analysis.spans[si..];
        let spans = &spans[..spans.partition_point(|(r, _, _)| r.start < range.end)];
        let hidden = &analysis.hidden[hi..];
        let hidden = &hidden[..hidden.partition_point(|r| r.start < range.end)];
        let replaced = &analysis.replaced[ri..];
        let replaced = &replaced[..replaced.partition_point(|(r, _)| r.start < range.end)];
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
        let (mut span_at, mut hide_at, mut replace_at) = (0, 0, 0);
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            while hide_at < hidden.len() && hidden[hide_at].end <= start {
                hide_at += 1;
            }
            if hidden
                .get(hide_at)
                .is_some_and(|r| r.start <= start && end <= r.end)
            {
                continue;
            }
            while replace_at < replaced.len() && replaced[replace_at].0.end <= start {
                replace_at += 1;
            }
            if let Some((r, with)) = replaced
                .get(replace_at)
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
                        out.push(p);
                    }
                }
                continue;
            }
            let mut text = source[start..end].to_string();
            // A continuation line of a list item or quote keeps the block's lead
            // in place of its own indent.
            if text.contains('\n') && !mono {
                // The marker plus the source's own space after it.
                let indent = " ".repeat(
                    lead.chars().count() + marker.chars().count() + usize::from(!marker.is_empty()),
                );
                let mut lines = text.split('\n');
                let mut joined = lines.next().unwrap_or("").to_string();
                for line in lines {
                    joined.push('\n');
                    joined.push_str(&indent);
                    joined.push_str(line.trim_start());
                }
                text = joined;
            }
            while span_at < spans.len() && spans[span_at].0.end <= start {
                span_at += 1;
            }
            let span = spans
                .get(span_at)
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
            });
        }
        out.push(Piece::glyph("\n", Role::Ink));
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
