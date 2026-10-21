//! A streaming reply settled block by block (LLP 1101.001 P1). The reply's
//! text grows by tokens; a top-level block is finished once a later block
//! has begun and no fence or list is still open, and from then on it never
//! changes. Only the unfinished tail is parsed again per token.
//!
//! Where a block ends is decided here by the same line rules the parser
//! (`markdown-parse`) uses, over complete lines only. The parser keeps no
//! state from one block to the next, so parsing a finished block's lines
//! alone gives exactly the blocks the whole reply gives there.

use crate::highlight::CodeHighlighter;
use crate::markdown;
use crate::state::{Block, Line};

/// One reply being streamed.
#[derive(Default)]
pub struct Reply {
    text: String,
    /// Bytes of `text` already settled.
    offset: usize,
    /// An open fenced block whose completed code lines need not be rebuilt.
    open_code: Option<OpenCode>,
}

/// What a token changed: blocks newly finished (one group per entry, in
/// order) and the tail as it reads now.
pub struct Step<'a> {
    pub finished: Vec<Vec<Block>>,
    pub tail: TailUpdate<'a>,
}

/// Either the whole current tail or a suffix of one open code block.
pub(super) enum TailUpdate<'a> {
    Replace(Vec<Block>),
    Code(CodePatch<'a>),
}

/// A lazy, validated edit to the lines of the live code block.
pub(super) struct CodePatch<'a> {
    source: &'a str,
    lang: &'a str,
    expected_lines: usize,
    replace_from: usize,
    lines: Vec<Line>,
}

impl TailUpdate<'_> {
    /// Apply the cheap suffix edit when the consumer still has the block
    /// this Reply produced. Reparse the borrowed tail on any shape mismatch.
    pub(super) fn apply(self, blocks: &mut Vec<Block>) {
        match self {
            TailUpdate::Replace(next) => *blocks = next,
            TailUpdate::Code(patch) if patch.matches(blocks) => {
                blocks[0].lines.truncate(patch.replace_from);
                blocks[0].lines.extend(patch.lines);
            }
            TailUpdate::Code(patch) => *blocks = markdown::streaming_blocks(patch.source),
        }
    }

    /// Materialize a complete tail when there is no live consumer entry.
    pub(super) fn into_blocks(self) -> Vec<Block> {
        match self {
            TailUpdate::Replace(blocks) => blocks,
            TailUpdate::Code(patch) => markdown::streaming_blocks(patch.source),
        }
    }
}

impl CodePatch<'_> {
    fn matches(&self, blocks: &[Block]) -> bool {
        let Some(block) = blocks.first().filter(|_| blocks.len() == 1) else {
            return false;
        };
        block.kind == "code"
            && block.lang == self.lang
            && block.depth == 0.0
            && block.marker.is_empty()
            && block.runs.is_empty()
            && block.lines.len() == self.expected_lines
            && self.replace_from <= self.expected_lines
    }
}

/// Parser and lexer metadata for one unterminated fenced block.
struct OpenCode {
    marker: char,
    width: usize,
    indent: usize,
    lang: String,
    /// First raw byte whose line may still change.
    scan_at: usize,
    /// Completed empty rows after the last committed visible row.
    pending_empty: usize,
    /// Visible rows before `scan_at` and `pending_empty`.
    committed_lines: usize,
    /// Rows the consumer should currently have.
    expected_lines: usize,
    /// Lexer state after `committed_lines`.
    highlighter: CodeHighlighter,
}

struct Rendered {
    expected_lines: usize,
    replace_from: usize,
    lines: Vec<Line>,
}

impl Reply {
    /// The whole reply so far.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Append `delta`: the groups it finished and the tail's blocks.
    pub fn push(&mut self, delta: &str) -> Step<'_> {
        self.text.push_str(delta);
        if self.open_code.is_some() {
            let closed = {
                let code = self.open_code.as_ref().expect("checked");
                code.has_closer(&self.text)
            };
            if !closed {
                let source = &self.text[self.offset..];
                let code = self.open_code.as_mut().expect("checked");
                let rendered = code.render(&self.text);
                return Step {
                    finished: Vec::new(),
                    tail: TailUpdate::Code(CodePatch {
                        source,
                        lang: &code.lang,
                        expected_lines: rendered.expected_lines,
                        replace_from: rendered.replace_from,
                        lines: rendered.lines,
                    }),
                };
            }
            self.open_code = None;
        }
        self.push_slow()
    }

    fn push_slow(&mut self) -> Step<'_> {
        let tail = &self.text[self.offset..];
        let complete = tail.rfind('\n').map_or(0, |i| i + 1);
        let lines: Vec<&str> = tail[..complete].split_inclusive('\n').collect();
        let groups = groups(&lines);
        let mut finished = Vec::new();
        if groups.len() > 1 {
            let starts: Vec<usize> = lines
                .iter()
                .scan(0, |at, l| {
                    let here = *at;
                    *at += l.len();
                    Some(here)
                })
                .collect();
            let last = starts[groups[groups.len() - 1]];
            for pair in groups.windows(2) {
                let text = &tail[starts[pair[0]]..starts[pair[1]]];
                finished.push(markdown::blocks(text));
            }
            self.offset += last;
        }
        let source = &self.text[self.offset..];
        let blocks = markdown::streaming_blocks(source);
        self.open_code = OpenCode::new(&self.text, self.offset, &blocks);
        Step {
            finished,
            tail: TailUpdate::Replace(blocks),
        }
    }

    /// The tail's blocks at the end of the reply.
    pub fn finish(&self) -> Vec<Block> {
        markdown::blocks(&self.text[self.offset..])
    }
}

impl OpenCode {
    /// Recognize exactly one open code block, then advance its incremental
    /// state once and compare it with the parser before trusting the cache.
    fn new(text: &str, offset: usize, blocks: &[Block]) -> Option<OpenCode> {
        let block = blocks
            .first()
            .filter(|b| blocks.len() == 1 && b.kind == "code")?;
        let tail = &text[offset..];
        let mut at = 0;
        let (marker, width, indent, lang, body_at) = loop {
            let newline = tail[at..].find('\n')? + at;
            let raw = tail[at..newline]
                .strip_suffix('\r')
                .unwrap_or(&tail[at..newline]);
            let trimmed = raw.trim_start();
            if trimmed.is_empty() {
                at = newline + 1;
                continue;
            }
            let (marker, width) = fence_of(trimmed)?;
            let indent = raw.len() - trimmed.len();
            let lang = trimmed.trim_start_matches(marker).trim().to_string();
            break (marker, width, indent, lang, offset + newline + 1);
        };
        if lang != block.lang {
            return None;
        }
        let mut code = OpenCode {
            marker,
            width,
            indent,
            highlighter: CodeHighlighter::new(&lang),
            lang,
            scan_at: body_at,
            pending_empty: 0,
            committed_lines: 0,
            expected_lines: 0,
        };
        if code.has_closer(text) {
            return None;
        }
        let rendered = code.render(text);
        (rendered.lines == block.lines).then_some(code)
    }

    fn has_closer(&self, text: &str) -> bool {
        physical_lines(&text[self.scan_at..]).any(|(line, _)| {
            let trimmed = line.trim_start();
            trimmed.chars().take_while(|&c| c == self.marker).count() >= self.width
                && trimmed.trim_end_matches(self.marker).is_empty()
        })
    }

    /// Highlight only rows after the immutable prefix. Completed blank rows
    /// stay as a count until later content makes them visible.
    fn render(&mut self, text: &str) -> Rendered {
        let expected_lines = self.expected_lines;
        let replace_from = self.committed_lines;
        let mut at = self.scan_at;
        let mut pending = self.pending_empty;
        let mut highlighter = self.highlighter;
        let mut lines = Vec::new();

        while let Some(relative) = text[at..].find('\n') {
            let newline = at + relative;
            let raw = text[at..newline]
                .strip_suffix('\r')
                .unwrap_or(&text[at..newline]);
            let line = strip_indent(raw, self.indent);
            at = newline + 1;
            if line.is_empty() {
                pending += 1;
                continue;
            }
            let before = lines.len();
            lines.extend((0..pending).map(|_| highlighter.line("")));
            pending = 0;
            lines.push(highlighter.line(line));
            self.committed_lines += lines.len() - before;
            self.highlighter = highlighter;
        }

        self.pending_empty = pending;
        self.scan_at = at;
        let partial = strip_indent(&text[at..], self.indent);
        let mut display_highlighter = self.highlighter;
        let display_start = lines.len();
        if !partial.is_empty() {
            lines.extend((0..pending).map(|_| display_highlighter.line("")));
            lines.push(display_highlighter.line(partial));
        } else if self.committed_lines == 0 {
            lines.push(display_highlighter.line(""));
        }
        self.expected_lines = self.committed_lines + lines.len() - display_start;
        Rendered {
            expected_lines,
            replace_from,
            lines,
        }
    }
}

fn strip_indent(line: &str, indent: usize) -> &str {
    let cut = line.len() - line.trim_start().len();
    &line[cut.min(indent)..]
}

/// Physical rows as the Markdown parser sees them and whether they ended.
fn physical_lines(mut text: &str) -> impl Iterator<Item = (&str, bool)> {
    std::iter::from_fn(move || {
        if text.is_empty() {
            return None;
        }
        match text.find('\n') {
            Some(at) => {
                let line = text[..at].strip_suffix('\r').unwrap_or(&text[..at]);
                text = &text[at + 1..];
                Some((line, true))
            }
            None => {
                let line = text;
                text = "";
                Some((line, false))
            }
        }
    })
}

fn trimmed(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

fn fence_of(line: &str) -> Option<(char, usize)> {
    ['`', '~'].into_iter().find_map(|m| {
        let n = line.chars().take_while(|&c| c == m).count();
        (n >= 3).then_some((m, n))
    })
}

fn is_rule(line: &str) -> bool {
    let mut marks = line.bytes().filter(|&b| b != b' ');
    let Some(marker @ (b'-' | b'*' | b'_')) = marks.next() else {
        return false;
    };
    let mut count = 1;
    for m in marks {
        if m != marker {
            return false;
        }
        count += 1;
    }
    count >= 3
}

fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes) && (line[hashes..].is_empty() || line[hashes..].starts_with(' '))
}

fn is_bullet(line: &str) -> bool {
    if ['-', '*', '+']
        .iter()
        .any(|m| line.strip_prefix(*m).is_some_and(|r| r.starts_with(' ')))
    {
        return true;
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    (1..=9).contains(&digits)
        && ['.', ')'].iter().any(|c| {
            line[digits..]
                .strip_prefix(*c)
                .is_some_and(|r| r.starts_with(' '))
        })
}

fn table_rows(lines: &[&str], at: usize) -> Option<usize> {
    if !trimmed(lines[at]).contains('|') {
        return None;
    }
    let rule = trimmed(lines.get(at + 1)?).trim();
    if !rule.contains('|')
        || !rule.contains('-')
        || !rule
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
    {
        return None;
    }
    let mut rows = 2;
    while let Some(line) = lines.get(at + rows) {
        let line = trimmed(line);
        if !line.contains('|') || line.trim().is_empty() {
            break;
        }
        rows += 1;
    }
    Some(rows)
}

fn is_image(line: &str) -> bool {
    line.strip_prefix("![")
        .is_some_and(|rest| rest.contains("](") && rest.ends_with(')'))
}

/// The first line of each settle group in `lines` (complete lines): a
/// block, or a run of list items. The parser's own line rules.
fn groups(lines: &[&str]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let mut in_list = false;
    let mut at = 0;
    while at < lines.len() {
        let line = trimmed(lines[at]);
        let t = line.trim_start();
        let indent = line.len() - t.len();
        if t.is_empty() {
            at += 1;
            continue;
        }
        let start = at;
        let mut item = false;
        if let Some((marker, width)) = fence_of(t) {
            at += 1;
            let mut closed = false;
            while at < lines.len() {
                let t = trimmed(lines[at]).trim_start();
                at += 1;
                if t.chars().take_while(|&c| c == marker).count() >= width
                    && t.trim_end_matches(marker).is_empty()
                {
                    closed = true;
                    break;
                }
            }
            if !closed {
                // Open to the end: everything from here is the tail.
                out.push(start);
                return out;
            }
        } else if is_rule(t) || is_heading(t) {
            at += 1;
        } else if let Some(rows) = table_rows(lines, at) {
            at += rows;
        } else if t.starts_with('>') {
            while at < lines.len() && trimmed(lines[at]).trim_start().starts_with('>') {
                at += 1;
            }
        } else if is_bullet(t) {
            item = true;
            at += 1;
            while at < lines.len() {
                let l = trimmed(lines[at]);
                let n = l.trim_start();
                if n.is_empty()
                    || l.len() - n.len() <= indent
                    || is_bullet(n)
                    || is_heading(n)
                    || fence_of(n).is_some()
                    || n.starts_with('>')
                {
                    break;
                }
                at += 1;
            }
        } else if is_image(t) || (t.starts_with('<') && t.len() > 2) {
            at += 1;
        } else {
            at += 1;
            while at < lines.len() {
                let n = trimmed(lines[at]).trim();
                if n.is_empty()
                    || is_heading(n)
                    || fence_of(n).is_some()
                    || is_rule(n)
                    || is_bullet(n)
                    || n.starts_with('>')
                    || table_rows(lines, at).is_some()
                {
                    break;
                }
                at += 1;
            }
        }
        // Consecutive list items are one group: a list is still open
        // until a block that is not an item begins.
        if !(item && in_list) {
            out.push(start);
        }
        in_list = item;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FullReply {
        text: String,
        offset: usize,
    }

    impl FullReply {
        fn push(&mut self, delta: &str) -> (Vec<Vec<Block>>, Vec<Block>) {
            self.text.push_str(delta);
            let tail = &self.text[self.offset..];
            let complete = tail.rfind('\n').map_or(0, |i| i + 1);
            let lines: Vec<&str> = tail[..complete].split_inclusive('\n').collect();
            let starts: Vec<usize> = lines
                .iter()
                .scan(0, |at, line| {
                    let here = *at;
                    *at += line.len();
                    Some(here)
                })
                .collect();
            let groups = groups(&lines);
            let mut finished = Vec::new();
            if groups.len() > 1 {
                let last = starts[groups[groups.len() - 1]];
                for pair in groups.windows(2) {
                    finished.push(markdown::blocks(&tail[starts[pair[0]]..starts[pair[1]]]));
                }
                self.offset += last;
            }
            (
                finished,
                markdown::streaming_blocks(&self.text[self.offset..]),
            )
        }

        fn finish(&self) -> Vec<Block> {
            markdown::blocks(&self.text[self.offset..])
        }
    }

    fn compare_deltas(text: &str, cuts: &[usize]) -> usize {
        let mut reply = Reply::default();
        let mut full = FullReply::default();
        let mut tail = Vec::new();
        let mut start = 0;
        let mut patches = 0;
        for &end in cuts.iter().chain(std::iter::once(&text.len())) {
            if end == start {
                continue;
            }
            let (expected_finished, expected_tail) = full.push(&text[start..end]);
            let Step {
                finished,
                tail: update,
            } = reply.push(&text[start..end]);
            assert_eq!(finished, expected_finished, "finished at {start}..{end}");
            patches += usize::from(matches!(&update, TailUpdate::Code(_)));
            update.apply(&mut tail);
            assert_eq!(tail, expected_tail, "tail at {start}..{end}");
            start = end;
        }
        assert_eq!(reply.finish(), full.finish(), "final tail");
        patches
    }

    fn scalar_cuts(text: &str) -> Vec<usize> {
        text.char_indices().skip(1).map(|(at, _)| at).collect()
    }

    #[test]
    fn incremental_code_matches_the_full_parser_at_every_boundary() {
        let docs = [
            "Intro.\n\n```rust\nfn café<T>() {\n\tlet s = \"λ\"; /* open\n  still */ Foo::new(42)\n\n`\n`` trailing\n``` \n}\n``\ninside\n```\n\nAfter.\n",
            "\r\n  ~~~~js extra words\r\n    const x = `é`; /* a\r\n    b */ return 2;\r\n  ~~~\r\n  ~~~~\r\nNext\r\n",
            "```unknown\nplain\t字\n\n\nlast",
            "   ```python\n   # note\n x = None\n   ```\n\n- finished\n",
        ];
        let mut patch_count = 0;
        for text in docs {
            let boundaries = scalar_cuts(text);
            patch_count += compare_deltas(text, &boundaries);
            for &cut in &boundaries {
                patch_count += compare_deltas(text, &[cut]);
            }
            for size in [2, 3, 7, 16, 31] {
                let cuts: Vec<usize> = boundaries
                    .iter()
                    .copied()
                    .filter(|at| at % size == 0)
                    .collect();
                patch_count += compare_deltas(text, &cuts);
            }
        }
        assert!(
            patch_count > 100,
            "the incremental path ran {patch_count} times"
        );
    }

    #[test]
    fn finished_groups_are_owned_after_the_borrowed_tail_is_gone() {
        let mut reply = Reply::default();
        let step = reply.push("Before.\n\n```rust\nfn one() {}\n");
        let Step { finished, tail } = step;
        assert_eq!(finished.len(), 1);
        let kept = finished;
        let mut live = tail.into_blocks();
        reply.push("fn two() {}\n").tail.apply(&mut live);
        assert_eq!(kept[0][0].runs[0].text, "Before.");
        assert_eq!(live[0].lines.len(), 2);
    }

    /// Stream `text` in `size`-byte tokens; every settled group and the
    /// final tail, and how many groups settled before the end.
    fn stream(text: &str, size: usize) -> (Vec<Vec<Block>>, usize) {
        let mut reply = Reply::default();
        let mut groups = Vec::new();
        let mut cut = 0;
        while cut < text.len() {
            let mut end = (cut + size).min(text.len());
            while !text.is_char_boundary(end) {
                end += 1;
            }
            let step = reply.push(&text[cut..end]);
            groups.extend(step.finished);
            cut = end;
        }
        let before = groups.len();
        groups.push(reply.finish());
        assert_eq!(reply.text(), text);
        (groups, before)
    }

    const DOC: &str = "# Title\n\nA paragraph that\nwraps two lines.\n\n- one\n- two\n  continued\n\n  - nested\n\n1. first\n2. second\n\n> a quote\n> more\n\n---\n\n```rust\nfn main() {\n\n    let x = 1;\n}\n```\nAfter the fence.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n![img](x.png)\n\nLast *words*.";

    #[test]
    fn the_settled_entries_render_what_the_whole_reply_does() {
        let whole = markdown::blocks(DOC);
        for size in [1, 2, 3, 7, 16, 1000] {
            let (groups, before) = stream(DOC, size);
            let joined: Vec<Block> = groups.concat();
            assert_eq!(joined, whole, "token size {size}");
            if size < 1000 {
                assert!(before >= 8, "only {before} settled at size {size}");
            }
        }
    }

    #[test]
    fn a_list_settles_as_one_group_once_it_ends() {
        let (groups, _) = stream(DOC, 5);
        // Items with only blank lines between them are one open list:
        // the bullets, the nested item and the numbered items.
        let list = groups.iter().find(|g| g[0].kind == "li").unwrap();
        assert_eq!(list.len(), 5);
        assert!(list.iter().all(|b| b.kind == "li"));
        assert_eq!(groups.iter().filter(|g| g[0].kind == "li").count(), 1);
    }

    #[test]
    fn an_open_fence_stays_in_the_tail() {
        let mut reply = Reply::default();
        let first = reply.push("Intro.\n\n```py\n");
        assert_eq!(first.finished.len(), 1);
        let mut tail = first.tail.into_blocks();
        for i in 0..200 {
            let step = reply.push(&format!("line {i}\n\n"));
            assert!(step.finished.is_empty(), "settled inside the fence at {i}");
            step.tail.apply(&mut tail);
            assert_eq!(tail[0].kind, "code");
        }
        let step = reply.push("```\n");
        assert!(step.finished.is_empty(), "nothing after it has begun yet");
        let step = reply.push("Then");
        assert!(
            step.finished.is_empty(),
            "a partial line has not begun a block"
        );
        let step = reply.push(" more.\n");
        assert_eq!(step.finished.len(), 1);
        // 400 lines, less the blank one the parser trims from the end.
        assert_eq!(step.finished[0][0].lines.len(), 399);
        step.tail.apply(&mut tail);
        assert_eq!(tail[0].kind, "p");
    }

    #[test]
    fn code_lines_compact_only_when_their_group_is_final() {
        fn run_capacity(blocks: &[Block]) -> (usize, usize) {
            blocks
                .iter()
                .filter(|block| block.kind == "code")
                .flat_map(|block| &block.lines)
                .fold((0, 0), |(len, capacity), line| {
                    (len + line.runs.len(), capacity + line.runs.capacity())
                })
        }

        let open = "```rust\nfn main() {\n";
        let mut ending = Reply::default();
        let live = ending.push(open).tail.into_blocks();
        let (live_len, live_capacity) = run_capacity(&live);

        let final_tail = ending.finish();
        assert_eq!(final_tail, live, "compaction cannot change content");
        let (final_len, final_capacity) = run_capacity(&final_tail);
        assert_eq!(final_len, live_len);
        assert!(final_capacity <= live_capacity);
        assert_eq!(final_tail, markdown::blocks(open));

        let mut settling = Reply::default();
        let complete = "```rust\nfn main() {\n}\n```\n";
        let live = settling.push(complete).tail.into_blocks();
        let (live_len, live_capacity) = run_capacity(&live);
        let step = settling.push("\nAfter.\n");
        assert_eq!(step.finished.len(), 1);
        let (settled_len, settled_capacity) = run_capacity(&step.finished[0]);
        assert_eq!(step.finished[0], live, "settling cannot change content");
        assert_eq!(settled_len, live_len);
        assert!(settled_capacity <= live_capacity);
        let tail = step.tail.into_blocks();
        assert_eq!(tail[0].kind, "p", "the later block remains after it");
    }

    #[test]
    fn a_blank_line_alone_finishes_nothing() {
        let mut reply = Reply::default();
        assert!(reply.push("One.\n\n").finished.is_empty());
        assert!(reply.push("\n\n").finished.is_empty());
        assert_eq!(reply.push("Two.\n").finished.len(), 1);
    }
}
