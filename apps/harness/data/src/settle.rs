//! A streaming reply settled block by block (LLP 1101.001 P1). The reply's
//! text grows by tokens; a top-level block is finished once a later block
//! has begun and no fence or list is still open, and from then on it never
//! changes. Only the unfinished tail is parsed again per token.
//!
//! Where a block ends is decided here by the same line rules the parser
//! (`markdown-parse`) uses, over complete lines only. The parser keeps no
//! state from one block to the next, so parsing a finished block's lines
//! alone gives exactly the blocks the whole reply gives there.

use crate::markdown;
use crate::state::Block;

/// One reply being streamed.
#[derive(Default)]
pub struct Reply {
    text: String,
    /// Bytes of `text` already settled.
    offset: usize,
}

/// What a token changed: blocks newly finished (one group per entry, in
/// order) and the tail as it reads now.
pub struct Step {
    pub finished: Vec<Vec<Block>>,
    pub tail: Vec<Block>,
}

impl Reply {
    /// The whole reply so far.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Append `delta`: the groups it finished and the tail's blocks.
    pub fn push(&mut self, delta: &str) -> Step {
        self.text.push_str(delta);
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
        Step {
            finished,
            tail: markdown::blocks(&self.text[self.offset..]),
        }
    }

    /// The tail's blocks at the end of the reply.
    pub fn finish(&self) -> Vec<Block> {
        markdown::blocks(&self.text[self.offset..])
    }
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
        assert!(reply.push("Intro.\n\n```py\n").finished.len() == 1);
        for i in 0..200 {
            let step = reply.push(&format!("line {i}\n\n"));
            assert!(step.finished.is_empty(), "settled inside the fence at {i}");
            assert_eq!(step.tail[0].kind, "code");
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
        assert_eq!(step.tail[0].kind, "p");
    }

    #[test]
    fn a_blank_line_alone_finishes_nothing() {
        let mut reply = Reply::default();
        assert!(reply.push("One.\n\n").finished.is_empty());
        assert!(reply.push("\n\n").finished.is_empty());
        assert_eq!(reply.push("Two.\n").finished.len(), 1);
    }
}
