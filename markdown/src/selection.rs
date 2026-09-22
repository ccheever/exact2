//! Toolbar facts derived from Markdown, never from an application selection slot.

use crate::block::BlockKind;
use crate::offsets::Offsets;
use crate::style::analyze;
use crate::{Range, BOLD, CODE, ITALIC, LINK, STRIKE};

/// The small `select` event record; offsets remain owned by the editor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Common formats as space-separated tokens: inline command names,
    /// `heading1` through `heading6`, `bullet`, `ordered`, `task`, `quote`, `codeblock`.
    pub formats: String,
    /// The selected content spans different formats or link targets.
    pub mixed: bool,
    /// The common link target, or empty when there is none or it is mixed.
    pub link: String,
    /// Space-separated command names unavailable in a fenced code block.
    pub unavailable: String,
}

fn block_formats(kind: &BlockKind, quote: u8) -> Vec<String> {
    let mut out = Vec::new();
    let name = match kind {
        BlockKind::Heading(n) => format!("heading{n}"),
        BlockKind::Bullet => "bullet".into(),
        BlockKind::Ordered => "ordered".into(),
        BlockKind::Task(_) => "task".into(),
        BlockKind::Code | BlockKind::Fence => "codeblock".into(),
        _ => String::new(),
    };
    if !name.is_empty() {
        out.push(name);
    }
    if quote > 0 {
        out.push("quote".into());
    }
    out
}

/// Read common formatting and link facts at a source-relative UTF-16 selection.
/// Mixed selections report only common formats, and a mixed link has no target.
pub fn selection(source: &str, selection: Range) -> Selection {
    let offsets = Offsets::new(source);
    let sel = offsets.range8(selection);
    let a = analyze(source, None);
    let mut samples: Vec<(u8, String, Vec<String>)> = Vec::new();
    let mut fenced = false;
    for b in &a.blocks {
        let touched = if sel.is_empty() {
            b.range.start <= sel.start && sel.start <= b.range.end
        } else {
            b.range.start < sel.end && sel.start < b.range.end
        };
        if !touched {
            continue;
        }
        fenced |= matches!(b.kind, BlockKind::Code | BlockKind::Fence);
        let block = block_formats(&b.kind, b.quote);
        if sel.is_empty() {
            let mut flags = 0;
            let mut href = String::new();
            for c in &a.constructs {
                if c.inner.start <= sel.start && sel.start <= c.inner.end {
                    flags |= c.kind;
                }
            }
            for (r, style, target) in &a.spans {
                if r.start <= sel.start && sel.start <= r.end {
                    flags |= style;
                    if style & LINK != 0 {
                        href.clone_from(target);
                    }
                }
            }
            samples.push((flags, href, block));
            continue;
        }
        let start = b.range.start.max(sel.start);
        let end = b.range.end.min(sel.end);
        let mut cuts = vec![start, end];
        // Sorted ranges only within the touched block, rather than rescanning
        // the whole document for each selected paragraph.
        let spans = &a.spans[a.spans.partition_point(|(r, _, _)| r.end <= start)..];
        let spans = &spans[..spans.partition_point(|(r, _, _)| r.start < end)];
        let hidden = &a.hidden[a.hidden.partition_point(|r| r.end <= start)..];
        let hidden = &hidden[..hidden.partition_point(|r| r.start < end)];
        for (r, _, _) in spans {
            cuts.extend([r.start.max(start), r.end.min(end)]);
        }
        for r in hidden {
            cuts.extend([r.start.max(start), r.end.min(end)]);
        }
        cuts.sort_unstable();
        cuts.dedup();
        let (mut si, mut hi) = (0, 0);
        for pair in cuts.windows(2) {
            let pos = pair[0];
            while hi < hidden.len() && hidden[hi].end <= pos {
                hi += 1;
            }
            if hidden.get(hi).is_some_and(|r| r.start <= pos) {
                continue;
            }
            while si < spans.len() && spans[si].0.end <= pos {
                si += 1;
            }
            let (flags, target) = spans
                .get(si)
                .filter(|(r, _, _)| r.start <= pos)
                .map_or((0, ""), |(_, f, h)| (*f, h.as_str()));
            samples.push((flags, target.to_string(), block.clone()));
        }
        if start == end {
            samples.push((0, String::new(), block));
        }
    }
    let Some(first) = samples.first() else {
        return Selection::default();
    };
    let mut flags = first.0;
    let mut blocks = first.2.clone();
    let mut link = first.1.clone();
    let mut mixed = false;
    let relevant = BOLD | ITALIC | CODE | STRIKE | LINK;
    for s in &samples[1..] {
        mixed |= s.0 & relevant != first.0 & relevant || s.1 != first.1 || s.2 != first.2;
        flags &= s.0;
        blocks.retain(|b| s.2.contains(b));
        if s.1 != link {
            link.clear();
        }
    }
    let mut formats: Vec<String> = [
        (BOLD, "bold"),
        (ITALIC, "italic"),
        (CODE, "code"),
        (STRIKE, "strike"),
        (LINK, "link"),
    ]
    .into_iter()
    .filter(|(bit, _)| flags & bit != 0)
    .map(|(_, name)| name.into())
    .collect();
    formats.extend(blocks);
    Selection {
        formats: formats.join(" "),
        mixed,
        link,
        unavailable: if fenced {
            "bold italic code strike link heading bullet ordered task quote footnote figure toggleTask".into()
        } else {
            String::new()
        },
    }
}
