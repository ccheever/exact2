//! The inline pass: emphasis, code, links, images, footnote references.
//!
//! One scan over a block's content produces marks (what to draw), the marker
//! ranges to hide, and the constructs an editing command toggles.

use crate::block::B;
use crate::{BOLD, CODE, IMAGE, ITALIC, LINK, STRIKE};

pub(crate) struct Mark {
    pub range: B,
    pub flags: u8,
    pub href: Option<String>,
}

/// A marked-up stretch: `outer` includes its markers, `inner` does not.
pub(crate) struct Construct {
    pub kind: u8,
    pub outer: B,
    pub inner: B,
}

#[derive(Default)]
pub(crate) struct Out {
    pub marks: Vec<Mark>,
    /// (marker, the construct that reveals it)
    pub hidden: Vec<(B, B)>,
    /// (range, footnote label, the construct that reveals it)
    pub footnotes: Vec<(B, String)>,
    pub constructs: Vec<Construct>,
}

#[derive(Clone, Copy)]
struct Delim {
    ch: char,
    start: usize,
    len: usize,
    total: usize,
    open: bool,
    close: bool,
}

#[derive(Default)]
struct AutolinkEnd {
    close: usize,
    last_at: Option<usize>,
}

struct Scanner<'a> {
    source: &'a str,
    /// The content's characters at their byte positions, lines joined by `\n`.
    cs: Vec<(usize, char)>,
    end: usize,
    /// For each `[`, the `]` that closes it, found in one pass; `usize::MAX`
    /// for none. An unmatched bracket then costs nothing to reject.
    closes: Vec<usize>,
    out: &'a mut Out,
}

/// The closing bracket of every `[`, skipping escapes and code spans.
fn match_brackets(cs: &[(usize, char)]) -> Vec<usize> {
    let mut closes = vec![usize::MAX; cs.len()];
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        match cs[i].1 {
            '\\' => i += 1,
            '`' => {
                let len = (i..cs.len()).take_while(|&j| cs[j].1 == '`').count();
                let mut j = i + len;
                while j < cs.len() {
                    if cs[j].1 == '`' {
                        let run = (j..cs.len()).take_while(|&k| cs[k].1 == '`').count();
                        if run == len {
                            i = j + run - 1;
                            break;
                        }
                        j += run;
                    } else {
                        j += 1;
                    }
                }
                if j >= cs.len() {
                    i += len - 1;
                }
            }
            '[' => stack.push(i),
            ']' => {
                if let Some(open) = stack.pop() {
                    closes[open] = i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    closes
}

/// Scans one block's content ranges as a single run of inline text.
pub(crate) fn scan(source: &str, content: &[B], out: &mut Out) {
    let mut cs = Vec::new();
    for (n, range) in content.iter().enumerate() {
        if n > 0 {
            cs.push((content[n - 1].end, '\n'));
        }
        cs.extend(
            source[range.clone()]
                .char_indices()
                .map(|(i, c)| (range.start + i, c)),
        );
    }
    let Some(end) = content.last().map(|r| r.end) else {
        return;
    };
    let hi = cs.len();
    let closes = match_brackets(&cs);
    Scanner {
        source,
        cs,
        end,
        closes,
        out,
    }
    .scan(0, hi);
}

fn punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace() && !c.is_ascii())
}

impl Scanner<'_> {
    fn pos(&self, i: usize) -> usize {
        self.cs.get(i).map_or(self.end, |c| c.0)
    }

    fn ch(&self, i: usize) -> char {
        self.cs[i].1
    }

    fn run(&self, i: usize, hi: usize) -> usize {
        let c = self.ch(i);
        (i..hi).take_while(|&j| self.ch(j) == c).count()
    }

    fn construct(&mut self, kind: u8, outer: B, inner: B, href: Option<String>) {
        self.out
            .hidden
            .push((outer.start..inner.start, outer.clone()));
        self.out.hidden.push((inner.end..outer.end, outer.clone()));
        self.out.marks.push(Mark {
            range: inner.clone(),
            flags: kind,
            href,
        });
        self.out.constructs.push(Construct { kind, outer, inner });
    }

    fn scan(&mut self, lo: usize, hi: usize) {
        let mut delims: Vec<Delim> = Vec::new();
        let mut autolink_end = AutolinkEnd::default();
        let mut i = lo;
        while i < hi {
            let c = self.ch(i);
            let next = match c {
                '\\' if i + 1 < hi && self.ch(i + 1).is_ascii_punctuation() => {
                    self.out
                        .hidden
                        .push((self.pos(i)..self.pos(i + 1), self.pos(i)..self.pos(i + 2)));
                    Some(i + 2)
                }
                '`' => self.code(i, hi),
                '<' => self.autolink(i, hi, &mut autolink_end),
                '!' if i + 1 < hi && self.ch(i + 1) == '[' => self.bracket(i + 1, hi, true),
                '[' => self.bracket(i, hi, false),
                'h' if i == lo || self.ch(i - 1).is_whitespace() || self.ch(i - 1) == '(' => {
                    self.bare_url(i, hi)
                }
                '*' | '_' | '~' => {
                    let len = self.run(i, hi);
                    if c != '~' || len == 2 {
                        delims.push(self.delimiter(i, len, lo, hi));
                    }
                    Some(i + len)
                }
                _ => None,
            };
            i = next.unwrap_or(i + 1);
        }
        self.emphasis(delims);
    }

    fn code(&mut self, i: usize, hi: usize) -> Option<usize> {
        let len = self.run(i, hi);
        let mut j = i + len;
        while j < hi {
            if self.ch(j) == '`' {
                let close = self.run(j, hi);
                if close == len {
                    let outer = self.pos(i)..self.pos(j + len);
                    self.construct(CODE, outer, self.pos(i + len)..self.pos(j), None);
                    return Some(j + len);
                }
                j += close;
            } else {
                j += 1;
            }
        }
        Some(i + len)
    }

    fn autolink(&mut self, i: usize, hi: usize, end: &mut AutolinkEnd) -> Option<usize> {
        // Rejected starts before this boundary share the same terminator and
        // last @. Keep this cursor local to each scan, including link labels.
        if end.close <= i {
            end.close = i + 1;
            end.last_at = None;
            while end.close < hi {
                let c = self.ch(end.close);
                if c == '>' || c.is_whitespace() {
                    break;
                }
                if c == '@' {
                    end.last_at = Some(end.close);
                }
                end.close += 1;
            }
        }
        let close = end.close;
        if close == hi || self.ch(close) != '>' || close == i + 1 {
            return None;
        }
        let target = &self.source[self.pos(i + 1)..self.pos(close)];
        // An invalid scheme prefix ends the search before a later '<' can
        // cause the same suffix to be searched again.
        let scheme = target
            .bytes()
            .position(|c| !c.is_ascii_alphanumeric() && !b"+.-".contains(&c))
            .is_some_and(|colon| {
                target.as_bytes()[colon] == b':' && colon >= 2 && colon + 1 < target.len()
            });
        let href = if scheme {
            target.to_string()
        } else if end.last_at.is_some_and(|at| at > i)
            && !target.starts_with('@')
            && !target.ends_with('@')
        {
            format!("mailto:{target}")
        } else {
            return None;
        };
        let outer = self.pos(i)..self.pos(close + 1);
        self.construct(LINK, outer, self.pos(i + 1)..self.pos(close), Some(href));
        Some(close + 1)
    }

    fn bare_url(&mut self, i: usize, hi: usize) -> Option<usize> {
        let tail = &self.source[self.pos(i)..];
        let scheme = if tail.starts_with("https://") {
            8
        } else if tail.starts_with("http://") {
            7
        } else {
            return None;
        };
        let mut j = i;
        while j < hi && !self.ch(j).is_whitespace() && !matches!(self.ch(j), '<' | '>') {
            j += 1;
        }
        let mut open = self.cs[i..j].iter().filter(|c| c.1 == '(').count();
        let mut shut = self.cs[i..j].iter().filter(|c| c.1 == ')').count();
        while j > i + scheme {
            let last = self.ch(j - 1);
            if matches!(
                last,
                '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '*' | '_' | '~'
            ) || (last == ')' && shut > open)
            {
                j -= 1;
                match last {
                    ')' => shut -= 1,
                    '(' => open -= 1,
                    _ => {}
                }
            } else {
                break;
            }
        }
        if j <= i + scheme {
            return None;
        }
        let range = self.pos(i)..self.pos(j);
        let href = self.source[range.clone()].to_string();
        self.out.marks.push(Mark {
            range,
            flags: LINK,
            href: Some(href),
        });
        Some(j)
    }

    /// `[label](target)`, `![alt](src)` or `[^label]`, from the `[` at `i`.
    fn bracket(&mut self, i: usize, hi: usize, image: bool) -> Option<usize> {
        let j = self.closes[i];
        if j >= hi {
            return None;
        }
        let linked = j + 1 < hi && self.ch(j + 1) == '(';
        if !image && !linked && j > i + 2 && self.ch(i + 1) == '^' {
            let label = &self.source[self.pos(i + 2)..self.pos(j)];
            if !label.contains(char::is_whitespace) {
                self.out
                    .footnotes
                    .push((self.pos(i)..self.pos(j + 1), label.to_string()));
                return Some(j + 1);
            }
        }
        if !linked {
            return None;
        }
        let (mut parens, mut k) = (0usize, j + 2);
        loop {
            if k >= hi {
                return None;
            }
            match self.ch(k) {
                '\\' => k += 1,
                '(' => parens += 1,
                ')' if parens == 0 => break,
                ')' => parens -= 1,
                _ => {}
            }
            k += 1;
        }
        let target = self.source[self.pos(j + 2)..self.pos(k)].trim();
        let target = target.split_whitespace().next().unwrap_or("");
        let href = target
            .strip_prefix('<')
            .and_then(|t| t.strip_suffix('>'))
            .unwrap_or(target)
            .to_string();
        let start = if image { i - 1 } else { i };
        if !image {
            self.scan(i + 1, j);
        }
        let outer = self.pos(start)..self.pos(k + 1);
        self.construct(
            if image { IMAGE } else { LINK },
            outer,
            self.pos(i + 1)..self.pos(j),
            Some(href),
        );
        Some(k + 1)
    }

    fn delimiter(&self, i: usize, len: usize, lo: usize, hi: usize) -> Delim {
        let c = self.ch(i);
        let before = if i > lo { self.ch(i - 1) } else { ' ' };
        let after = if i + len < hi { self.ch(i + len) } else { ' ' };
        let left = !after.is_whitespace()
            && (!punctuation(after) || before.is_whitespace() || punctuation(before));
        let right = !before.is_whitespace()
            && (!punctuation(before) || after.is_whitespace() || punctuation(after));
        let (open, close) = if c == '_' {
            (
                left && (!right || punctuation(before)),
                right && (!left || punctuation(after)),
            )
        } else {
            (left, right)
        };
        Delim {
            ch: c,
            start: i,
            len,
            total: len,
            open,
            close,
        }
    }

    /// CommonMark's delimiter algorithm: each closer takes its nearest opener.
    /// CommonMark's `openers_bottom`: once a closer of this shape finds no
    /// opener, no later closer of the same shape searches below it.
    fn emphasis(&mut self, mut d: Vec<Delim>) {
        let mut active = 0;
        let shape = |c: &Delim| {
            (match c.ch {
                '*' => 0,
                '_' => 1,
                _ => 2,
            }) * 6
                + usize::from(c.open) * 3
                + c.total % 3
        };
        let mut bottoms = [0usize; 18];
        // Processed delimiters occupy a compact prefix. The unread tail stays
        // in place; a match retires intervening entries by shortening the prefix.
        for read in 0..d.len() {
            d[active] = d[read];
            loop {
                let ci = active;
                if !d[ci].close || d[ci].len == 0 {
                    break;
                }
                let bottom = bottoms[shape(&d[ci])];
                let found = (bottom..ci).rev().find(|&oi| {
                    let (o, c) = (&d[oi], &d[ci]);
                    let thirds = (o.open && o.close || c.open && c.close)
                        && (o.total + c.total) % 3 == 0
                        && !(o.total % 3 == 0 && c.total % 3 == 0);
                    o.ch == c.ch && o.open && o.len > 0 && !thirds
                });
                let Some(oi) = found else {
                    bottoms[shape(&d[ci])] = ci;
                    break;
                };
                let take = if d[oi].len >= 2 && d[ci].len >= 2 {
                    2
                } else {
                    1
                };
                let kind = match (d[ci].ch, take) {
                    ('~', _) => STRIKE,
                    (_, 2) => BOLD,
                    _ => ITALIC,
                };
                let open_end = d[oi].start + d[oi].len;
                let close_start = d[ci].start;
                let outer = self.pos(open_end - take)..self.pos(close_start + take);
                self.construct(kind, outer, self.pos(open_end)..self.pos(close_start), None);
                d[oi].len -= take;
                d[ci].start += take;
                d[ci].len -= take;
                active = oi + usize::from(d[oi].len != 0);
                d[active] = d[ci];
                for b in &mut bottoms {
                    *b = (*b).min(active);
                }
            }
            active += 1;
        }
    }
}
