//! The block pass: lines into blocks, with the ranges of their markers.
//!
//! Nesting is a depth, never a tree: a host styles paragraphs, and a paragraph
//! has an indent and a quote level.

use crate::segment;

pub(crate) type B = std::ops::Range<usize>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BlockKind {
    Body,
    Heading(u8),
    Bullet,
    Ordered,
    Task(bool),
    Rule,
    Fence,
    Code,
    FootnoteDef,
    Table,
    Embed,
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MarkerKind {
    /// Hidden unless the selection touches the block's group.
    Hide,
    /// Always drawn as a bullet glyph.
    Bullet,
    /// Always drawn as a checkbox.
    TaskBox(bool),
    /// Stays visible, dimmed.
    Ordered,
    /// The whole line drawn as a rule unless touched.
    Rule,
    /// A footnote definition's `[^label]:`.
    FootnoteLabel(String),
}

#[derive(Clone, Debug)]
pub(crate) struct Block {
    pub kind: BlockKind,
    /// The block's lines, without the final newline.
    pub range: B,
    /// What the selection must touch to reveal `Hide` markers: the block, or
    /// for a fence or code paragraph the whole fenced block.
    pub group: B,
    pub depth: u8,
    pub quote: u8,
    /// A code block's info string, on its `Code` and opening `Fence`.
    pub lang: String,
    pub markers: Vec<(B, MarkerKind)>,
    /// Inline content, one range per line.
    pub content: Vec<B>,
}

impl Block {
    fn new(kind: BlockKind, range: B) -> Self {
        Self {
            kind,
            group: range.clone(),
            range,
            depth: 0,
            quote: 0,
            lang: String::new(),
            markers: Vec::new(),
            content: Vec::new(),
        }
    }

    /// Whether following plain lines continue this block.
    fn continues(&self) -> bool {
        matches!(
            self.kind,
            BlockKind::Body
                | BlockKind::Bullet
                | BlockKind::Ordered
                | BlockKind::Task(_)
                | BlockKind::FootnoteDef
        )
    }
}

/// A line's list marker, in bytes relative to the line.
pub(crate) struct ListMarker {
    /// Columns of indent; a tab is one level.
    pub columns: usize,
    /// The symbol: `-`, or `12.`.
    pub symbol: B,
    /// A task's `[ ]`, and whether it is checked.
    pub task: Option<(B, bool)>,
    /// The ordered number, if ordered.
    pub number: Option<u64>,
    /// Where the item's text begins.
    pub content: usize,
}

/// Leading `>` markers: their count, their ranges, and where the rest begins.
pub(crate) fn quote_prefix(line: &str) -> (u8, Vec<B>, usize) {
    let b = line.as_bytes();
    let (mut depth, mut markers, mut i) = (0u8, Vec::new(), 0);
    loop {
        let mut j = i;
        while j < b.len() && b[j] == b' ' && j - i < 3 {
            j += 1;
        }
        if j >= b.len() || b[j] != b'>' || depth == u8::MAX {
            return (depth, markers, i);
        }
        let mut end = j + 1;
        if end < b.len() && b[end] == b' ' {
            end += 1;
        }
        markers.push(j..end);
        depth += 1;
        i = end;
    }
}

/// `#` to `######` and the spaces after, as (level, marker length).
pub(crate) fn heading(s: &str) -> Option<(u8, usize)> {
    let level = s.bytes().take_while(|&c| c == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &s[level..];
    if rest.is_empty() {
        return Some((level as u8, level));
    }
    let spaces = rest.bytes().take_while(|&c| c == b' ').count();
    (spaces > 0).then_some((level as u8, level + spaces))
}

fn rule(s: &str) -> bool {
    let t = s.trim();
    let Some(first) = t.chars().next() else {
        return false;
    };
    matches!(first, '-' | '*' | '_')
        && t.chars().all(|c| c == first || c == ' ')
        && t.chars().filter(|&c| c == first).count() >= 3
}

pub(crate) fn list_marker(s: &str) -> Option<ListMarker> {
    let b = s.as_bytes();
    let (mut i, mut columns) = (0, 0);
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        columns += if b[i] == b'\t' { 2 } else { 1 };
        i += 1;
    }
    let start = i;
    let mut number = None;
    if i < b.len() && matches!(b[i], b'-' | b'*' | b'+') {
        i += 1;
    } else {
        while i < b.len() && b[i].is_ascii_digit() && i - start < 9 {
            i += 1;
        }
        if i == start || i >= b.len() || !matches!(b[i], b'.' | b')') {
            return None;
        }
        number = s[start..i].parse().ok();
        i += 1;
    }
    let symbol = start..i;
    if i < b.len() && b[i] != b' ' {
        return None;
    }
    let content = (i + 1).min(b.len());
    let mut marker = ListMarker {
        columns,
        symbol,
        task: None,
        number,
        content,
    };
    let rest = &s[content..];
    if number.is_none()
        && rest.len() >= 3
        && rest.as_bytes()[0] == b'['
        && rest.as_bytes()[2] == b']'
    {
        let mark = rest.as_bytes()[1];
        let after = rest.as_bytes().get(3);
        if matches!(mark, b' ' | b'x' | b'X') && matches!(after, None | Some(b' ')) {
            marker.task = Some((content..content + 3, mark != b' '));
            marker.content = (content + 4).min(b.len());
        }
    }
    Some(marker)
}

/// An opening or closing fence: (character, length, info string).
fn fence(s: &str) -> Option<(u8, usize, &str)> {
    let t = s.trim_start_matches(' ');
    if s.len() - t.len() > 3 {
        return None;
    }
    let ch = *t.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let len = t.bytes().take_while(|&c| c == ch).count();
    let info = t[len..].trim();
    (len >= 3 && !(ch == b'`' && info.contains('`'))).then_some((ch, len, info))
}

/// `[^label]: ` as (label, marker length).
pub(crate) fn footnote_def(s: &str) -> Option<(&str, usize)> {
    let rest = s.strip_prefix("[^")?;
    let close = rest.find(']')?;
    let label = &rest[..close];
    if label.is_empty() || label.contains(char::is_whitespace) {
        return None;
    }
    let after = rest[close + 1..].strip_prefix(':')?;
    let spaces = after.bytes().take_while(|&c| c == b' ').count();
    Some((label, 2 + close + 2 + spaces))
}

fn table_rule(s: &str) -> bool {
    let t = s.trim();
    t.contains('-') && t.contains('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

struct Line<'a> {
    start: usize,
    text: &'a str,
}

pub(crate) fn scan(source: &str) -> Vec<Block> {
    let mut lines = Vec::new();
    let mut at = 0;
    for raw in source.split_inclusive('\n') {
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        lines.push(Line {
            start: at,
            text: text.strip_suffix('\r').unwrap_or(text),
        });
        at += raw.len();
    }

    let mut blocks: Vec<Block> = Vec::new();
    let mut open = false; // whether the last block takes continuation lines
    let mut i = 0;
    while i < lines.len() {
        let Line { start, text } = lines[i];
        let end = start + text.len();
        let (quote, quote_markers, offset) = quote_prefix(text);
        let rest = &text[offset..];
        let base = start + offset;
        if rest.trim().is_empty() {
            open = false;
            i += 1;
            continue;
        }
        let mut block = Block::new(BlockKind::Body, start..end);
        block.quote = quote;
        block.markers.extend(
            quote_markers
                .into_iter()
                .map(|m| (start + m.start..start + m.end, MarkerKind::Hide)),
        );

        if quote == 0 {
            if let Some((ch, len, info)) = fence(rest) {
                i = fenced(&lines, i, (ch, len, info), &mut blocks);
                open = false;
                continue;
            }
        }
        if let Some((level, marker)) = heading(rest) {
            block.kind = BlockKind::Heading(level);
            block.markers.push((base..base + marker, MarkerKind::Hide));
            block.content.push(base + marker..end);
            open = false;
        } else if rule(rest) {
            block.kind = BlockKind::Rule;
            block.markers.push((base..end, MarkerKind::Rule));
            open = false;
        } else if let Some((label, marker)) = footnote_def(rest) {
            block.kind = BlockKind::FootnoteDef;
            block.markers.push((
                base..base + marker,
                MarkerKind::FootnoteLabel(label.to_string()),
            ));
            block.content.push(base + marker..end);
            open = true;
        } else if let Some(m) = list_marker(rest) {
            block.depth = (m.columns / 2).min(u8::MAX as usize) as u8;
            match (&m.task, m.number) {
                (Some((task, checked)), _) => {
                    block.kind = BlockKind::Task(*checked);
                    block
                        .markers
                        .push((base + m.symbol.start..base + task.start, MarkerKind::Hide));
                    block.markers.push((
                        base + task.start..base + task.end,
                        MarkerKind::TaskBox(*checked),
                    ));
                }
                (None, Some(_)) => {
                    block.kind = BlockKind::Ordered;
                    block.markers.push((
                        base + m.symbol.start..base + m.symbol.end,
                        MarkerKind::Ordered,
                    ));
                }
                (None, None) => {
                    block.kind = BlockKind::Bullet;
                    block.markers.push((
                        base + m.symbol.start..base + m.symbol.end,
                        MarkerKind::Bullet,
                    ));
                }
            }
            block.content.push(base + m.content..end);
            open = true;
        } else if rest.contains('|')
            && lines.get(i + 1).is_some_and(|next| table_rule(next.text))
            && quote == 0
        {
            block.kind = BlockKind::Table;
            let mut last = i + 1;
            while lines
                .get(last + 1)
                .is_some_and(|l| l.text.contains('|') && !l.text.trim().is_empty())
            {
                last += 1;
            }
            block.range = start..lines[last].start + lines[last].text.len();
            block.group = block.range.clone();
            block.content = lines[i..=last]
                .iter()
                .map(|l| l.start..l.start + l.text.len())
                .collect();
            blocks.push(block);
            open = false;
            i = last + 1;
            continue;
        } else {
            // A plain line continues an open paragraph, item or definition.
            if let Some(previous) = blocks
                .last_mut()
                .filter(|b| open && b.continues() && (quote == 0 || quote == b.quote))
            {
                previous.range.end = end;
                previous.group.end = end;
                previous.markers.append(&mut block.markers);
                let indent = rest.len() - rest.trim_start().len();
                previous.content.push(base + indent..end);
                i += 1;
                continue;
            }
            block.content.push(base..end);
            open = true;
        }
        blocks.push(block);
        i += 1;
    }

    for block in &mut blocks {
        if block.kind == BlockKind::Body && block.quote == 0 && block.content.len() == 1 {
            let text = &source[block.content[0].clone()];
            if let Some(kind) = segment::kind(text) {
                block.kind = kind;
            }
        }
    }
    blocks
}

/// A fenced block from line `i`: an opening fence, the code, a closing fence
/// if there is one. Returns the next line.
fn fenced(
    lines: &[Line],
    i: usize,
    (ch, len, info): (u8, usize, &str),
    blocks: &mut Vec<Block>,
) -> usize {
    let open = &lines[i];
    let mut close = None;
    let mut j = i + 1;
    while j < lines.len() {
        if fence(lines[j].text).is_some_and(|(c, l, info)| c == ch && l >= len && info.is_empty()) {
            close = Some(j);
            break;
        }
        j += 1;
    }
    let last = close.unwrap_or(lines.len() - 1);
    let group = open.start..lines[last].start + lines[last].text.len();
    let body_end = close.unwrap_or(lines.len());

    let mut opening = Block::new(BlockKind::Fence, open.start..open.start + open.text.len());
    opening.lang = info.to_string();
    opening
        .markers
        .push((opening.range.clone(), MarkerKind::Hide));
    opening.group = group.clone();
    blocks.push(opening);
    if body_end > i + 1 {
        let tail = &lines[body_end - 1];
        let mut code = Block::new(
            BlockKind::Code,
            lines[i + 1].start..tail.start + tail.text.len(),
        );
        code.lang = info.to_string();
        code.group = group.clone();
        blocks.push(code);
    }
    if let Some(c) = close {
        let mut closing = Block::new(
            BlockKind::Fence,
            lines[c].start..lines[c].start + lines[c].text.len(),
        );
        closing
            .markers
            .push((closing.range.clone(), MarkerKind::Hide));
        closing.group = group;
        blocks.push(closing);
    }
    last + 1
}
