//! Formatting commands as text replacements.
//!
//! @ref LLP 1045 D2, D6 — a command never restyles anything: it edits the
//! source the way a person would type it, and the styler draws the result.
//! A host applies the replacements as ordinary, undoable edits.

use crate::block::{self, BlockKind, B};
use crate::offsets::Offsets;
use crate::style::analyze;
use crate::{Range, BOLD, CODE, ITALIC, LINK, STRIKE};

/// What a toolbar button, a shortcut or the Return key asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Toggle `**bold**`.
    Bold,
    /// Toggle `*italic*`.
    Italic,
    /// Toggle `` `code` ``.
    Code,
    /// Toggle `~~strikethrough~~`.
    Strike,
    /// Wrap the selection in a link to this target, or unwrap the link.
    Link(String),
    /// Toggle a heading of this level on the selected lines.
    Heading(u8),
    /// Toggle a bulleted list.
    Bullet,
    /// Toggle a numbered list.
    Ordered,
    /// Toggle a task list.
    Task,
    /// Toggle a block quote.
    Quote,
    /// Toggle a fenced code block around the selected lines.
    CodeBlock,
    /// Insert a footnote reference and begin its definition.
    Footnote,
    /// Insert an image or video figure of this source as its own paragraph,
    /// with the caret in its caption.
    Figure(String),
    /// Nest the selected lines one level.
    Indent,
    /// Lift the selected lines one level.
    Outdent,
    /// Check or uncheck the caret's task.
    ToggleTask,
    /// The Return key: continue a list or quote, or end it on an empty item.
    Newline,
}

/// Replacements against the source the command was given, sorted and never
/// overlapping, and the selection once they are applied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edit {
    /// (range of the old source, its new text)
    pub replacements: Vec<(Range, String)>,
    /// The selection in the new source.
    pub selection: Range,
}

impl Edit {
    /// The source with the replacements applied.
    pub fn apply(&self, source: &str) -> String {
        let offsets = Offsets::new(source);
        let mut out = String::with_capacity(source.len());
        let mut at = 0;
        for (range, text) in &self.replacements {
            let bytes = offsets.range8(*range);
            out.push_str(&source[at..bytes.start]);
            out.push_str(text);
            at = bytes.end;
        }
        out.push_str(&source[at..]);
        out
    }
}

/// Replacements in bytes, and the selection in the new text's bytes.
struct Draft {
    reps: Vec<(B, String)>,
    selection: Option<B>,
}

/// Where an old position lands. At a replacement's start, `after` decides
/// which side of the new text it takes.
fn shift(reps: &[(B, String)], mut pos: usize, after: bool) -> usize {
    let mut delta = 0isize;
    for (range, text) in reps {
        if range.start > pos || (pos == range.start && !after) {
            break;
        }
        delta += text.len() as isize - range.len() as isize;
        pos = pos.max(range.end);
    }
    (pos as isize + delta) as usize
}

struct Lines<'a> {
    source: &'a str,
}

impl Lines<'_> {
    fn start(&self, at: usize) -> usize {
        self.source[..at].rfind('\n').map_or(0, |i| i + 1)
    }

    fn end(&self, at: usize) -> usize {
        self.source[at..]
            .find('\n')
            .map_or(self.source.len(), |i| at + i)
    }

    /// The lines a selection touches; a selection ending at a line's start
    /// does not take that line.
    fn touched(&self, sel: &B) -> Vec<B> {
        let first = self.start(sel.start);
        let last = if sel.end > sel.start && sel.end == self.start(sel.end) {
            sel.end - 1
        } else {
            sel.end
        };
        let last = self.end(last.max(first));
        let mut out = Vec::new();
        let mut at = first;
        loop {
            let end = self.end(at);
            out.push(at..end);
            if end >= last {
                return out;
            }
            at = end + 1;
        }
    }
}

/// A line's block syntax, in bytes of the source.
struct Prefix {
    /// After any quote markers and indent: where a block marker goes.
    marker: usize,
    /// Where the text begins.
    content: usize,
    kind: Option<BlockKind>,
    number: u64,
    quoted: bool,
}

fn prefix(source: &str, line: &B) -> Prefix {
    let text = &source[line.clone()];
    let (quote, _, offset) = block::quote_prefix(text);
    let rest = &text[offset..];
    let base = line.start + offset;
    let mut p = Prefix {
        marker: base + (rest.len() - rest.trim_start().len()),
        content: base,
        kind: None,
        number: 0,
        quoted: quote > 0,
    };
    p.content = p.marker;
    if let Some((level, len)) = block::heading(rest) {
        p.kind = Some(BlockKind::Heading(level));
        p.content = base + len;
    } else if let Some(m) = block::list_marker(rest) {
        p.marker = base + m.symbol.start;
        p.content = base + m.content;
        p.number = m.number.unwrap_or(0);
        p.kind = Some(match (m.task, m.number) {
            (Some((_, checked)), _) => BlockKind::Task(checked),
            (None, Some(_)) => BlockKind::Ordered,
            (None, None) => BlockKind::Bullet,
        });
    }
    p
}

fn inline(source: &str, sel: B, kind: u8, marker: &str) -> Draft {
    let analysis = analyze(source, None);
    let inside = analysis
        .constructs
        .iter()
        .filter(|c| c.kind == kind && c.outer.start <= sel.start && sel.end <= c.outer.end)
        .min_by_key(|c| c.outer.len());
    if let Some(c) = inside {
        return Draft {
            reps: vec![
                (c.outer.start..c.inner.start, String::new()),
                (c.inner.end..c.outer.end, String::new()),
            ],
            selection: None,
        };
    }
    let m = marker.len();
    if sel.is_empty() {
        // A pair just made and left empty is taken back.
        if source[..sel.start].ends_with(marker) && source[sel.start..].starts_with(marker) {
            return Draft {
                reps: vec![(sel.start - m..sel.start + m, String::new())],
                selection: Some(sel.start - m..sel.start - m),
            };
        }
        return Draft {
            reps: vec![(sel.clone(), format!("{marker}{marker}"))],
            selection: Some(sel.start + m..sel.start + m),
        };
    }
    // Markers hug the text, a line at a time: `** a**` is not bold.
    let mut reps = Vec::new();
    let mut at = sel.start;
    for piece in source[sel.clone()].split('\n') {
        let lead = piece.len() - piece.trim_start().len();
        let text = piece.trim();
        if !text.is_empty() {
            reps.push((at + lead..at + lead, marker.to_string()));
            reps.push((
                at + lead + text.len()..at + lead + text.len(),
                marker.to_string(),
            ));
        }
        at += piece.len() + 1;
    }
    let selection = match (reps.first(), reps.last()) {
        (Some(first), Some(last)) => {
            Some(shift(&reps, first.0.start, true)..shift(&reps, last.0.start, false))
        }
        _ => None,
    };
    Draft { reps, selection }
}

fn link(source: &str, sel: B, target: &str) -> Draft {
    let analysis = analyze(source, None);
    if let Some(c) = analysis
        .constructs
        .iter()
        .find(|c| c.kind == LINK && c.outer.start <= sel.start && sel.end <= c.outer.end)
    {
        return Draft {
            reps: if target.is_empty() {
                vec![
                    (c.outer.start..c.inner.start, String::new()),
                    (c.inner.end..c.outer.end, String::new()),
                ]
            } else {
                vec![(c.inner.end..c.outer.end, format!("]({target})"))]
            },
            selection: if target.is_empty() {
                None
            } else {
                Some(c.inner.clone())
            },
        };
    }
    let reps = vec![
        (sel.start..sel.start, "[".to_string()),
        (sel.end..sel.end, format!("]({target})")),
    ];
    // The caret goes where something is missing: the label, else the target.
    let selection = if sel.is_empty() {
        sel.start + 1..sel.start + 1
    } else if target.is_empty() {
        sel.end + 3..sel.end + 3
    } else {
        sel.start + 1..sel.end + 1
    };
    Draft {
        reps,
        selection: Some(selection),
    }
}

fn blocks(source: &str, sel: B, command: &Command) -> Draft {
    let lines = Lines { source }.touched(&sel);
    let single = lines.len() == 1;
    let prefixes: Vec<(B, Prefix)> = lines
        .into_iter()
        .map(|l| {
            let p = prefix(source, &l);
            (l, p)
        })
        .filter(|(l, _)| single || !source[l.clone()].trim().is_empty())
        .collect();
    let same = |p: &Prefix| match (command, &p.kind) {
        (Command::Heading(n), Some(BlockKind::Heading(level))) => n == level,
        (Command::Bullet, Some(BlockKind::Bullet))
        | (Command::Ordered, Some(BlockKind::Ordered))
        | (Command::Task, Some(BlockKind::Task(_))) => true,
        _ => false,
    };
    let remove = prefixes.iter().all(|(_, p)| same(p));
    let reps = prefixes
        .iter()
        .enumerate()
        .map(|(n, (_, p))| {
            let marker = match command {
                _ if remove => String::new(),
                Command::Heading(level) => {
                    format!("{} ", "#".repeat((*level).clamp(1, 6) as usize))
                }
                Command::Ordered => format!("{}. ", n + 1),
                Command::Task => "- [ ] ".to_string(),
                _ => "- ".to_string(),
            };
            (p.marker..p.content, marker)
        })
        .collect();
    Draft {
        reps,
        selection: None,
    }
}

fn quote(source: &str, sel: B) -> Draft {
    let lines = Lines { source }.touched(&sel);
    let single = lines.len() == 1;
    let lines: Vec<B> = lines
        .into_iter()
        .filter(|l| single || !source[l.clone()].trim().is_empty())
        .collect();
    let quoted = lines.iter().all(|l| prefix(source, l).quoted);
    let reps = lines
        .iter()
        .map(|l| {
            if !quoted {
                return (l.start..l.start, "> ".to_string());
            }
            let (_, markers, _) = block::quote_prefix(&source[l.clone()]);
            (
                l.start + markers[0].start..l.start + markers[0].end,
                String::new(),
            )
        })
        .collect();
    Draft {
        reps,
        selection: None,
    }
}

fn code_block(source: &str, sel: B) -> Draft {
    let analysis = analyze(source, None);
    let fenced = analysis.blocks.iter().filter(|b| {
        b.kind == BlockKind::Fence && b.group.start <= sel.start && sel.end <= b.group.end
    });
    let mut reps: Vec<(B, String)> = fenced
        .map(|b| {
            // A fence goes with its newline; the last line's goes with the one before.
            if b.range.end < source.len() {
                (b.range.start..b.range.end + 1, String::new())
            } else {
                (b.range.start.saturating_sub(1)..b.range.end, String::new())
            }
        })
        .collect();
    // Two fences with nothing between share a newline: one removal.
    if let [a, b] = reps.as_mut_slice() {
        if b.0.start < a.0.end {
            b.0.start = a.0.end;
        }
    }
    if !reps.is_empty() {
        return Draft {
            reps,
            selection: None,
        };
    }
    let lines = Lines { source }.touched(&sel);
    let (start, end) = (lines[0].start, lines[lines.len() - 1].end);
    Draft {
        reps: vec![
            (start..start, "```\n".to_string()),
            (end..end, "\n```".to_string()),
        ],
        selection: Some(sel.start + 4..sel.end + 4),
    }
}

fn indent(source: &str, sel: B, out: bool) -> Draft {
    let reps = Lines { source }
        .touched(&sel)
        .into_iter()
        .filter_map(|l| {
            if !out {
                return Some((l.start..l.start, "  ".to_string()));
            }
            let text = &source[l.clone()];
            let lead = if text.starts_with('\t') {
                1
            } else {
                text.bytes().take(2).take_while(|&c| c == b' ').count()
            };
            (lead > 0).then(|| (l.start..l.start + lead, String::new()))
        })
        .collect();
    Draft {
        reps,
        selection: None,
    }
}

fn toggle_task(source: &str, sel: B) -> Draft {
    let lines = Lines { source };
    let line = lines.start(sel.start)..lines.end(sel.start);
    let p = prefix(source, &line);
    let reps = match p.kind {
        // `- [ ] `: the mark is three bytes before the text, or two at a line's end.
        Some(BlockKind::Task(checked)) => {
            let open = source[p.marker..line.end]
                .find('[')
                .map_or(p.marker, |i| p.marker + i);
            vec![(
                open + 1..open + 2,
                if checked { " " } else { "x" }.to_string(),
            )]
        }
        _ => Vec::new(),
    };
    Draft {
        reps,
        selection: Some(sel),
    }
}

fn figure(source: &str, sel: B, src: &str) -> Draft {
    // The figure is a paragraph of its own: blank lines on both sides.
    let before = &source[..sel.start];
    let after = &source[sel.end..];
    let lead = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let trail = if after.is_empty() || after.starts_with("\n\n") {
        ""
    } else if after.starts_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let caret = sel.start + lead.len() + 2;
    Draft {
        reps: vec![(sel, format!("{lead}![]({src}){trail}"))],
        selection: Some(caret..caret),
    }
}

fn footnote(source: &str, sel: B) -> Draft {
    let mut next = 1u64;
    let mut rest = source;
    while let Some(at) = rest.find("[^") {
        rest = &rest[at + 2..];
        let digits = rest.bytes().take_while(|c| c.is_ascii_digit()).count();
        if rest[digits..].starts_with(']') {
            next = next.max(rest[..digits].parse::<u64>().map_or(next, |n| n + 1));
        }
    }
    let gap = if source.is_empty() || source.ends_with("\n\n") {
        ""
    } else if source.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let reps = vec![
        (sel.end..sel.end, format!("[^{next}]")),
        (source.len()..source.len(), format!("{gap}[^{next}]: ")),
    ];
    let end = source.len() + reps.iter().map(|r| r.1.len()).sum::<usize>();
    Draft {
        reps,
        selection: Some(end..end),
    }
}

fn newline(source: &str, sel: B) -> Draft {
    let lines = Lines { source };
    let line = lines.start(sel.start)..lines.end(sel.start);
    let plain = |text: String| {
        let caret = sel.start + text.len();
        Draft {
            reps: vec![(sel.clone(), text)],
            selection: Some(caret..caret),
        }
    };
    // Inside a fence a line is literal: Return keeps its indent and nothing else.
    let analysis = analyze(source, None);
    let coded = analysis.blocks.iter().any(|b| {
        matches!(b.kind, BlockKind::Code | BlockKind::Fence)
            && b.group.start <= sel.start
            && sel.start <= b.group.end
            && b.range.start <= sel.start
            && sel.start <= b.range.end
    });
    if coded {
        let text = &source[line.clone()];
        let lead = &text[..text.len() - text.trim_start().len()];
        return plain(format!("\n{lead}"));
    }
    let p = prefix(source, &line);
    let listed = matches!(
        p.kind,
        Some(BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task(_))
    );
    if (!listed && !p.quoted) || sel.start < p.content {
        return plain("\n".to_string());
    }
    if source[p.content..line.end].trim().is_empty() {
        // Return on an empty item ends the list.
        return Draft {
            reps: vec![(line.start..line.end.max(sel.end), String::new())],
            selection: Some(line.start..line.start),
        };
    }
    let lead = &source[line.start..p.marker];
    let marker = match p.kind {
        Some(BlockKind::Ordered) => {
            // The delimiter follows the authored digits, which may be `01`.
            let symbol = source[p.marker..p.content].trim_end();
            let digits = symbol.bytes().take_while(|c| c.is_ascii_digit()).count();
            format!("{}{} ", p.number + 1, &symbol[digits..])
        }
        Some(BlockKind::Task(_)) => "- [ ] ".to_string(),
        Some(BlockKind::Bullet) => source[p.marker..p.content].to_string(),
        _ => String::new(),
    };
    let mut draft = plain(format!("\n{lead}{marker}"));
    if p.kind == Some(BlockKind::Ordered) {
        // The items below at this level count on from the new one.
        let (mut at, mut number) = (lines.end(sel.end), p.number + 2);
        while at < source.len() {
            let next = at + 1..lines.end(at + 1);
            let q = prefix(source, &next);
            if source[next.clone()].trim().is_empty()
                || q.marker - next.start < p.marker - line.start
            {
                break;
            }
            if q.marker - next.start == p.marker - line.start {
                if q.kind != Some(BlockKind::Ordered) {
                    break;
                }
                let digits = source[q.marker..]
                    .bytes()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                draft
                    .reps
                    .push((q.marker..q.marker + digits, number.to_string()));
                number += 1;
            }
            at = next.end;
        }
    }
    draft
}

/// Turns `command` at `selection` into replacements and the selection after.
pub fn edit(source: &str, selection: Range, command: Command) -> Edit {
    let offsets = Offsets::new(source);
    let sel = offsets.range8(selection);
    // A code span's delimiter is one backtick more than any run inside it.
    let backticks = "`".repeat(
        1 + source[sel.clone()]
            .split(|c| c != '`')
            .map(str::len)
            .max()
            .unwrap_or(0),
    );
    let draft = match &command {
        Command::Bold => inline(source, sel.clone(), BOLD, "**"),
        Command::Italic => inline(source, sel.clone(), ITALIC, "*"),
        Command::Code => inline(source, sel.clone(), CODE, &backticks),
        Command::Strike => inline(source, sel.clone(), STRIKE, "~~"),
        Command::Link(target) => link(source, sel.clone(), target),
        Command::Heading(_) | Command::Bullet | Command::Ordered | Command::Task => {
            blocks(source, sel.clone(), &command)
        }
        Command::Quote => quote(source, sel.clone()),
        Command::CodeBlock => code_block(source, sel.clone()),
        Command::Footnote => footnote(source, sel.clone()),
        Command::Figure(src) => figure(source, sel.clone(), src),
        Command::Indent => indent(source, sel.clone(), false),
        Command::Outdent => indent(source, sel.clone(), true),
        Command::ToggleTask => toggle_task(source, sel.clone()),
        Command::Newline => newline(source, sel.clone()),
    };
    let mut reps = draft.reps;
    reps.retain(|(range, text)| !range.is_empty() || !text.is_empty());
    reps.sort_by_key(|(range, _)| (range.start, range.end));
    let after = draft.selection.unwrap_or_else(|| {
        let start = shift(&reps, sel.start, true);
        start..shift(&reps, sel.end, true).max(start)
    });
    let mut edit = Edit {
        replacements: reps
            .iter()
            .map(|(r, t)| (offsets.range16(r), t.clone()))
            .collect(),
        selection: Range::default(),
    };
    if !source.is_ascii() || reps.iter().any(|(_, t)| !t.is_ascii()) {
        edit.selection = Offsets::new(&edit.apply(source)).range16(&after);
    } else {
        edit.selection = Range::new(after.start as u32, after.end as u32);
    }
    edit
}
