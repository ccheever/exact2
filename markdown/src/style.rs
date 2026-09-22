//! How to draw a source string: paragraphs, spans, and what to hide.

use crate::block::{self, Block, BlockKind, MarkerKind, B};
use crate::inline::{self, Construct, Mark};
use crate::offsets::Offsets;
use crate::{Range, LINK, MARKER};

/// A block's paragraph style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParagraphKind {
    /// Ordinary prose.
    Body,
    /// A heading of level 1 to 6.
    Heading(u8),
    /// A bulleted list item.
    Bullet,
    /// A numbered list item; its number is visible text.
    Ordered,
    /// A task item, checked or not.
    Task(bool),
    /// A thematic break.
    Rule,
    /// A code block's fence line: collapsed unless revealed.
    Fence,
    /// A code block's lines, with its info string.
    Code(String),
    /// A footnote definition.
    Footnote,
    /// A pipe table's source lines, drawn in a fixed pitch.
    Table,
    /// A lone URL of a known provider.
    Embed,
    /// A lone image.
    Image,
    /// A lone video file.
    Video,
}

/// One block of the source and how its paragraphs are set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paragraph {
    /// The block's lines, without the final newline.
    pub range: Range,
    /// Its style.
    pub kind: ParagraphKind,
    /// List nesting, from zero.
    pub depth: u8,
    /// Block quote nesting; zero is unquoted.
    pub quote: u8,
}

/// A stretch of one inline style. Spans are sorted and never overlap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// The text it covers.
    pub range: Range,
    /// `BOLD | ITALIC | …` flags.
    pub style: u8,
    /// A link's target or an image's source; empty otherwise.
    pub href: String,
}

/// What is drawn in place of a range of source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Replacement {
    /// A bullet glyph.
    Bullet,
    /// A checkbox.
    TaskBox(bool),
    /// A horizontal rule across the line.
    Rule,
    /// A superscript footnote mark with this text.
    Footnote(String),
}

/// A range drawn as something other than its characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replaced {
    /// The source it stands for.
    pub range: Range,
    /// What is drawn.
    pub with: Replacement,
}

/// Everything a host needs to draw a source string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Styled {
    /// Every block, in order.
    pub paragraphs: Vec<Paragraph>,
    /// Inline styles, sorted, never overlapping.
    pub spans: Vec<Span>,
    /// Marker ranges that draw nothing, sorted, never overlapping.
    pub hidden: Vec<Range>,
    /// Ranges drawn as a glyph or a mark, sorted.
    pub replaced: Vec<Replaced>,
    /// Every footnote label, in the order its first reference appears.
    pub footnotes: Vec<Footnote>,
}

/// A footnote of the document: its label, and where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footnote {
    /// The label as written after `[^`.
    pub label: String,
    /// The number drawn for it: its position in first-reference order, from 1.
    pub ordinal: u32,
    /// Each `[^label]` reference, in order.
    pub references: Vec<Range>,
    /// The `[^label]:` definition paragraph, if there is one.
    pub definition: Option<Range>,
}

/// The analysis in bytes, shared by styling, editing and plain text.
pub(crate) struct Analysis {
    pub blocks: Vec<Block>,
    pub spans: Vec<(B, u8, String)>,
    pub hidden: Vec<B>,
    pub replaced: Vec<(B, Replacement)>,
    pub constructs: Vec<Construct>,
    /// (label, reference ranges, definition block range)
    pub footnotes: Vec<(String, Vec<B>, Option<B>)>,
}

fn touches(reveal: &Option<B>, construct: &B) -> bool {
    reveal.as_ref().is_some_and(|r| {
        r.start <= construct.end
            && (if r.is_empty() {
                construct.start <= r.end
            } else {
                construct.start < r.end
            })
    })
}

/// Marks into sorted spans that never overlap: flags joined, the innermost
/// link's target kept, and nothing over a hidden marker. One sweep over the
/// range boundaries, so a paragraph of many marks stays linear.
fn flatten(marks: &[Mark], hidden: &[B], spans: &mut Vec<(B, u8, String)>) {
    // (position, +1 opens / -1 closes, mark index; hidden marks use usize::MAX)
    let mut events: Vec<(usize, i8, usize)> = Vec::with_capacity((marks.len() + hidden.len()) * 2);
    for (n, m) in marks.iter().enumerate() {
        if !m.range.is_empty() {
            events.push((m.range.start, 1, n));
            events.push((m.range.end, -1, n));
        }
    }
    for h in hidden {
        events.push((h.start, 1, usize::MAX));
        events.push((h.end, -1, usize::MAX));
    }
    events.sort_unstable();
    let mut open: Vec<usize> = Vec::new();
    let mut covered = 0usize;
    let mut at = 0usize;
    for (n, &(pos, delta, mark)) in events.iter().enumerate() {
        if pos > at && !open.is_empty() && covered == 0 {
            let mut flags = 0u8;
            let mut best: Option<&Mark> = None;
            for &m in &open {
                let m = &marks[m];
                flags |= m.flags;
                if m.href.is_some() && best.is_none_or(|b| m.range.len() < b.range.len()) {
                    best = Some(m);
                }
            }
            let href = best.and_then(|m| m.href.as_deref()).unwrap_or("");
            match spans.last_mut() {
                Some(last) if last.0.end == at && last.1 == flags && last.2 == href => {
                    last.0.end = pos
                }
                _ => spans.push((at..pos, flags, href.to_string())),
            }
        }
        at = pos.max(at);
        if mark == usize::MAX {
            covered = (covered as isize + delta as isize) as usize;
        } else if delta > 0 {
            open.push(mark);
        } else if let Some(i) = open.iter().position(|&m| m == mark) {
            open.swap_remove(i);
        }
        let _ = n;
    }
}

pub(crate) fn analyze(source: &str, reveal: Option<B>) -> Analysis {
    let blocks = block::scan(source);
    let mut analysis = Analysis {
        blocks: Vec::new(),
        spans: Vec::new(),
        hidden: Vec::new(),
        replaced: Vec::new(),
        constructs: Vec::new(),
        footnotes: Vec::new(),
    };
    // Footnotes number by first reference in document order, whatever their
    // labels say, and never by what the selection reveals. A definition with
    // no reference numbers after every referenced one.
    let mut footnotes: Vec<(String, Vec<B>, Option<B>)> = Vec::new();
    fn index(footnotes: &mut Vec<(String, Vec<B>, Option<B>)>, label: &str) -> usize {
        footnotes
            .iter()
            .position(|(l, _, _)| l == label)
            .unwrap_or_else(|| {
                footnotes.push((label.to_string(), Vec::new(), None));
                footnotes.len() - 1
            })
    }
    for block in &blocks {
        if matches!(
            block.kind,
            BlockKind::Fence | BlockKind::Code | BlockKind::Rule | BlockKind::Table
        ) {
            continue;
        }
        let mut out = inline::Out::default();
        inline::scan(source, &block.content, &mut out);
        for (range, label) in out.footnotes {
            let at = index(&mut footnotes, &label);
            footnotes[at].1.push(range);
        }
    }
    for block in &blocks {
        for (_, kind) in &block.markers {
            if let MarkerKind::FootnoteLabel(label) = kind {
                let at = index(&mut footnotes, label);
                footnotes[at].2.get_or_insert(block.range.clone());
            }
        }
    }
    let ordinal = |label: &str| -> String {
        footnotes
            .iter()
            .position(|(l, _, _)| l == label)
            .map_or_else(|| "?".to_string(), |at| (at + 1).to_string())
    };

    for block in &blocks {
        let mut out = inline::Out::default();
        let revealed = touches(&reveal, &block.group);
        let first_hidden = analysis.hidden.len();
        for (range, kind) in &block.markers {
            let dim = || Mark {
                range: range.clone(),
                flags: MARKER,
                href: None,
            };
            match kind {
                MarkerKind::Hide if revealed => out.marks.push(dim()),
                MarkerKind::Hide => analysis.hidden.push(range.clone()),
                MarkerKind::Bullet | MarkerKind::TaskBox(_) if revealed => out.marks.push(dim()),
                MarkerKind::Bullet => analysis.replaced.push((range.clone(), Replacement::Bullet)),
                MarkerKind::TaskBox(checked) => analysis
                    .replaced
                    .push((range.clone(), Replacement::TaskBox(*checked))),
                MarkerKind::Ordered => out.marks.push(dim()),
                MarkerKind::Rule if revealed => out.marks.push(dim()),
                MarkerKind::Rule => analysis.replaced.push((range.clone(), Replacement::Rule)),
                MarkerKind::FootnoteLabel(_) if revealed => out.marks.push(dim()),
                MarkerKind::FootnoteLabel(label) => {
                    analysis
                        .replaced
                        .push((range.clone(), Replacement::Footnote(ordinal(label))));
                }
            }
        }
        match block.kind {
            BlockKind::Fence | BlockKind::Code | BlockKind::Rule | BlockKind::Table => {}
            // A figure is one link or image span over its caption, or over
            // the bare URL; the syntax around a caption hides as usual.
            BlockKind::Embed | BlockKind::Image | BlockKind::Video => {
                let range = block.content[0].clone();
                let text = &source[range.clone()];
                match crate::segment::figure(text) {
                    Some(("", target, false)) => out.marks.push(Mark {
                        range,
                        flags: LINK,
                        href: Some(target.to_string()),
                    }),
                    _ => inline::scan(source, &block.content, &mut out),
                }
            }
            _ => inline::scan(source, &block.content, &mut out),
        }
        for (marker, _) in out.hidden.drain(..) {
            if marker.is_empty() {
                continue;
            }
            if revealed {
                out.marks.push(Mark {
                    range: marker,
                    flags: MARKER,
                    href: None,
                });
            } else {
                analysis.hidden.push(marker);
            }
        }
        for (range, label) in out.footnotes.drain(..) {
            if revealed {
                out.marks.push(Mark {
                    range,
                    flags: MARKER,
                    href: None,
                });
            } else {
                analysis
                    .replaced
                    .push((range, Replacement::Footnote(ordinal(&label))));
            }
        }
        flatten(
            &out.marks,
            &analysis.hidden[first_hidden..],
            &mut analysis.spans,
        );
        analysis.constructs.append(&mut out.constructs);
    }

    analysis.hidden.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<B> = Vec::new();
    for range in analysis.hidden.drain(..) {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    analysis.hidden = merged;
    analysis.replaced.sort_by_key(|r| r.0.start);
    analysis.footnotes = footnotes;
    analysis.blocks = blocks;
    analysis
}

/// How to draw `source`. `reveal` is the selection while editing: a construct
/// touching a paragraph reveals all its markers, as [`MARKER`](crate::MARKER) spans. `None`
/// is reading: every marker hidden.
pub fn style(source: &str, reveal: Option<Range>) -> Styled {
    let offsets = Offsets::new(source);
    let analysis = analyze(source, reveal.map(|r| offsets.range8(r)));
    Styled {
        paragraphs: analysis
            .blocks
            .iter()
            .map(|b| Paragraph {
                range: offsets.range16(&b.range),
                kind: match b.kind {
                    BlockKind::Body => ParagraphKind::Body,
                    BlockKind::Heading(level) => ParagraphKind::Heading(level),
                    BlockKind::Bullet => ParagraphKind::Bullet,
                    BlockKind::Ordered => ParagraphKind::Ordered,
                    BlockKind::Task(checked) => ParagraphKind::Task(checked),
                    BlockKind::Rule => ParagraphKind::Rule,
                    BlockKind::Fence => ParagraphKind::Fence,
                    BlockKind::Code => ParagraphKind::Code(b.lang.clone()),
                    BlockKind::FootnoteDef => ParagraphKind::Footnote,
                    BlockKind::Table => ParagraphKind::Table,
                    BlockKind::Embed => ParagraphKind::Embed,
                    BlockKind::Image => ParagraphKind::Image,
                    BlockKind::Video => ParagraphKind::Video,
                },
                depth: b.depth,
                quote: b.quote,
            })
            .collect(),
        spans: analysis
            .spans
            .iter()
            .map(|(r, style, href)| Span {
                range: offsets.range16(r),
                style: *style,
                href: href.clone(),
            })
            .collect(),
        hidden: analysis.hidden.iter().map(|r| offsets.range16(r)).collect(),
        replaced: analysis
            .replaced
            .iter()
            .map(|(r, with)| Replaced {
                range: offsets.range16(r),
                with: with.clone(),
            })
            .collect(),
        footnotes: analysis
            .footnotes
            .iter()
            .enumerate()
            .map(|(n, (label, references, definition))| Footnote {
                label: label.clone(),
                ordinal: n as u32 + 1,
                references: references.iter().map(|r| offsets.range16(r)).collect(),
                definition: definition.as_ref().map(|r| offsets.range16(r)),
            })
            .collect(),
    }
}
