//! Inline parsing: one line of Markdown to the ordered styled runs the
//! kernel measures.
//!
//! The runs are what an Exact paragraph is made of (LLP 1033): a `text` node
//! whose children are `text` nodes, each with its own weight, slant, family
//! and `href`. Nothing here knows about layout, and nothing here does I/O.

/// One styled span of a paragraph.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Run {
    /// The characters.
    pub text: String,
    /// `**bold**`.
    pub bold: bool,
    /// `*italic*`.
    pub italic: bool,
    /// A `` `code` `` span: the reader gives it a monospaced family.
    pub code: bool,
    /// Where a link goes — empty for text that is not a link.
    pub href: String,
}

impl Run {
    /// A plain run of `text`.
    pub fn text(text: impl Into<String>) -> Run {
        Run {
            text: text.into(),
            ..Run::default()
        }
    }
}

// A stopped scan stays stopped, even if the caller's generation changes again.
// Used by block construction and inline indexes, without a parser-global token.
pub(crate) struct Scan<'a> {
    cancel: &'a dyn Fn() -> bool,
    stopped: std::cell::Cell<bool>,
}

impl<'a> Scan<'a> {
    pub(crate) fn new(cancel: &'a dyn Fn() -> bool) -> Self {
        Self {
            cancel,
            stopped: std::cell::Cell::new(false),
        }
    }

    pub(crate) fn cancelled(&self) -> bool {
        if !self.stopped.get() && (self.cancel)() {
            self.stopped.set(true);
        }
        self.stopped.get()
    }
}

/// A parse frame borrows a range of the original paragraph. Nested labels and
/// emphasis share delimiter indexes and emit directly into the final run list;
/// neither their source nor their descendant runs are copied per nesting level.
struct Frame {
    start: usize,
    end: usize,
    resume: usize,
    first_run: usize,
    bold: bool,
    italic: bool,
    href: std::rc::Rc<str>,
    plain_wrapper: bool,
    last_plain: bool,
}

struct Parser<'a> {
    text: &'a str,
    scan: &'a Scan<'a>,
    src: &'a [u8],
    at: usize,
    runs: Vec<Run>,
    pending: String,
    frame: Frame,
    parents: Vec<Frame>,
    brackets: Option<Vec<usize>>,
    parens: Option<Vec<usize>>,
    ticks: Option<Vec<(usize, usize)>>,
    stars: Option<Vec<[usize; 2]>>,
    underscores: Option<Vec<[usize; 2]>>,
    angle: Option<Angle>,
}

// One forward scan per interval ending at `>`, including failed mail tests.
// Remember the last relevant characters, not just the closing delimiter: a
// repeated `<` before one invalid autolink must not rescan its mail address.
struct Angle {
    close: usize,
    mail: usize,
    invalid: usize,
}

/// Parse one paragraph's text into runs. `link` resolves targets beside the
/// document. Indexes are built only when their delimiter is actually parsed;
/// ordinary prose allocates no delimiter index.
pub(crate) fn runs(text: &str, link: &dyn Fn(&str) -> String, scan: &Scan<'_>) -> Vec<Run> {
    let mut p = Parser {
        text,
        scan,
        src: text.as_bytes(),
        at: 0,
        runs: Vec::new(),
        pending: String::new(),
        frame: Frame {
            start: 0,
            end: text.len(),
            resume: text.len(),
            first_run: 0,
            bold: false,
            italic: false,
            href: "".into(),
            plain_wrapper: true,
            last_plain: false,
        },
        parents: Vec::new(),
        brackets: None,
        parens: None,
        ticks: None,
        stars: None,
        underscores: None,
        angle: None,
    };
    p.run(link);
    p.runs
}

impl Parser<'_> {
    fn peek(&self, ahead: usize) -> u8 {
        if self.at + ahead < self.frame.end {
            self.src[self.at + ahead]
        } else {
            0
        }
    }

    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.pending);
        // Preserve the old parser's run boundaries: only pending plain text
        // coalesces, and only with a run plain in this frame BEFORE inheritance.
        if self.frame.last_plain {
            self.runs.last_mut().unwrap().text.push_str(&text);
        } else {
            self.runs.push(Run {
                text,
                bold: self.frame.bold,
                italic: self.frame.italic,
                href: self.frame.href.to_string(),
                code: false,
            });
        }
        self.frame.last_plain = true;
    }

    fn push(&mut self, mut run: Run) {
        self.flush();
        if run.text.is_empty() {
            return;
        }
        self.frame.last_plain = !run.bold && !run.italic && !run.code && run.href.is_empty();
        run.bold |= self.frame.bold;
        run.italic |= self.frame.italic;
        if run.href.is_empty() {
            run.href = self.frame.href.to_string();
        }
        self.runs.push(run);
    }

    fn enter(
        &mut self,
        start: usize,
        end: usize,
        resume: usize,
        bold: bool,
        italic: bool,
        href: String,
    ) {
        self.flush();
        let child = Frame {
            start,
            end,
            resume,
            first_run: self.runs.len(),
            bold: bold || self.frame.bold,
            italic: italic || self.frame.italic,
            plain_wrapper: !bold && !italic && href.is_empty(),
            href: if href.is_empty() {
                self.frame.href.clone()
            } else {
                href.into()
            },
            last_plain: false,
        };
        self.parents.push(std::mem::replace(&mut self.frame, child));
        self.at = start;
    }

    fn run(&mut self, link: &dyn Fn(&str) -> String) {
        let mut checkpoint = 0;
        loop {
            if self.at >= checkpoint {
                if self.scan.cancelled() {
                    return;
                }
                checkpoint = self.at + 4096;
            }
            if self.at >= self.frame.end {
                self.flush();
                let Some(parent) = self.parents.pop() else {
                    break;
                };
                let child = std::mem::replace(&mut self.frame, parent);
                if self.runs.len() > child.first_run {
                    self.frame.last_plain = child.last_plain && child.plain_wrapper;
                }
                self.at = child.resume;
                continue;
            }
            let c = self.peek(0);
            match c {
                b'\\' if self.peek(1).is_ascii_punctuation() => {
                    self.pending.push(self.peek(1) as char);
                    self.at += 2;
                }
                b'`' if self.code_span() => {}
                b'!' if self.peek(1) == b'[' => {
                    if let Some((close, end)) = self.bracketed(self.at + 1) {
                        let label = &self.text[self.at + 2..close];
                        let target = target(&self.text[close + 2..end]);
                        self.pending
                            .push_str(if label.is_empty() { target } else { label });
                        self.at = end + 1;
                    } else {
                        self.pending.push('!');
                        self.at += 1;
                    }
                }
                b'[' => {
                    if let Some((close, end)) = self.bracketed(self.at) {
                        let href = link(target(&self.text[close + 2..end]));
                        self.enter(self.at + 1, close, end + 1, false, false, href);
                    } else {
                        self.pending.push('[');
                        self.at += 1;
                    }
                }
                b'<' if self.autolink(link) => {}
                b'*' | b'_' if self.emphasis(c) => {}
                b'h' if self.bare_url(link) => {}
                _ => {
                    // Copy ordinary text in bounded chunks. Only ASCII can
                    // begin syntax, so the scan can skip whole UTF-8 scalars.
                    let first = (self.at + utf8_width(c)).min(self.frame.end);
                    let mut limit = (first + 4096).min(self.frame.end);
                    while !self.text.is_char_boundary(limit) {
                        limit -= 1;
                    }
                    let end = self.src[first..limit]
                        .iter()
                        .position(|c| {
                            matches!(c, b'\\' | b'`' | b'!' | b'[' | b'<' | b'*' | b'_' | b'h')
                        })
                        .map_or(limit, |offset| first + offset);
                    self.pending.push_str(&self.text[self.at..end]);
                    self.at = end;
                }
            }
        }
    }

    fn code_span(&mut self) -> bool {
        let ticks = self
            .ticks
            .get_or_insert_with(|| tick_matches(self.src, self.scan));
        let (open, close) = ticks[self.at];
        if close >= self.frame.end {
            return false;
        }
        let count = open - self.at;
        let text = &self.text[open..close];
        let text = text
            .strip_prefix(' ')
            .and_then(|t| t.strip_suffix(' '))
            .unwrap_or(text);
        self.push(Run {
            text: text.to_string(),
            code: true,
            ..Run::default()
        });
        self.at = close + count;
        true
    }

    fn bracketed(&mut self, start: usize) -> Option<(usize, usize)> {
        let pairs = self
            .brackets
            .get_or_insert_with(|| matching(self.src, b'[', b']', self.scan));
        let close = pairs[start];
        if close >= self.frame.end || self.src.get(close + 1) != Some(&b'(') {
            return None;
        }
        let pairs = self
            .parens
            .get_or_insert_with(|| matching(self.src, b'(', b')', self.scan));
        let end = pairs[close + 1];
        (end < self.frame.end).then_some((close, end))
    }

    fn autolink(&mut self, link: &dyn Fn(&str) -> String) -> bool {
        if self.angle.as_ref().is_none_or(|a| a.close < self.at) {
            let mut angle = Angle {
                close: self.src.len(),
                mail: 0,
                invalid: 0,
            };
            for at in self.at + 1..self.src.len() {
                if at.is_multiple_of(4096) && self.scan.cancelled() {
                    return false;
                }
                match self.src[at] {
                    b'>' => {
                        angle.close = at;
                        break;
                    }
                    b'@' => angle.mail = at,
                    b' ' | b'/' => angle.invalid = at,
                    _ => {}
                }
            }
            self.angle = Some(angle);
        }
        let angle = self.angle.as_ref().unwrap();
        let close = angle.close;
        if close >= self.frame.end {
            return false;
        }
        let inner = &self.text[self.at + 1..close];
        let is_url = inner.starts_with("http://") || inner.starts_with("https://");
        let is_mail = !is_url && angle.mail > self.at && angle.invalid <= self.at;
        if !is_url && !is_mail {
            return false;
        }
        let href = if is_mail {
            format!("mailto:{inner}")
        } else {
            link(inner)
        };
        self.push(Run {
            text: inner.to_string(),
            href,
            ..Run::default()
        });
        self.at = close + 1;
        true
    }

    fn bare_url(&mut self, link: &dyn Fn(&str) -> String) -> bool {
        let rest = &self.text[self.at..self.frame.end];
        if !rest.starts_with("http://") && !rest.starts_with("https://") {
            return false;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '<' || c == ')' || c == '"')
            .unwrap_or(rest.len());
        let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if url.len() < 12 {
            return false;
        }
        self.push(Run {
            text: url.to_string(),
            href: link(url),
            ..Run::default()
        });
        self.at += url.len();
        true
    }

    fn emphasis(&mut self, marker: u8) -> bool {
        let count = if self.peek(1) == marker { 2 } else { 1 };
        if matches!(self.peek(count), 0 | b' ' | b'\t') {
            return false;
        }
        if marker == b'_'
            && count == 1
            && self.at > self.frame.start
            && self.src[self.at - 1].is_ascii_alphanumeric()
        {
            return false;
        }
        let index = if marker == b'*' {
            &mut self.stars
        } else {
            &mut self.underscores
        };
        let matches = index.get_or_insert_with(|| emphasis_matches(self.src, marker, self.scan));
        let open = self.at + count;
        let close = matches[open][count - 1];
        if close + count > self.frame.end {
            return false;
        }
        self.enter(
            open,
            close,
            close + count,
            count == 2,
            count == 1,
            String::new(),
        );
        true
    }
}

fn target(inside: &str) -> &str {
    let inside = inside.trim();
    let target = match inside.find(['"', '\'']) {
        Some(cut) => inside[..cut].trim(),
        None => inside,
    };
    target.trim_start_matches('<').trim_end_matches('>')
}

// Direct-address indexes have no sorting or per-opener suffix scan. Unmatched
// openers retain a sentinel, shared by every nested frame of this paragraph.
fn matching(src: &[u8], open: u8, close: u8, scan: &Scan<'_>) -> Vec<usize> {
    let mut pairs = vec![src.len(); src.len()];
    let mut stack = Vec::new();
    let mut at = 0;
    let mut checkpoint = 0;
    while at < src.len() {
        if at >= checkpoint {
            if scan.cancelled() {
                break;
            }
            checkpoint = at + 4096;
        }
        match src[at] {
            b'\\' => at += 1,
            c if c == open => stack.push(at),
            c if c == close => {
                if let Some(start) = stack.pop() {
                    pairs[start] = at;
                }
            }
            _ => {}
        }
        at += 1;
    }
    pairs
}

fn tick_matches(src: &[u8], scan: &Scan<'_>) -> Vec<(usize, usize)> {
    let mut pairs = vec![(src.len(), src.len()); src.len()];
    let mut next = vec![src.len(); src.len() + 1];
    let mut end = src.len();
    while end > 0 {
        if end.is_multiple_of(4096) && scan.cancelled() {
            break;
        }
        if src[end - 1] != b'`' {
            end -= 1;
            continue;
        }
        let mut start = end - 1;
        while start > 0 && src[start - 1] == b'`' {
            if start.is_multiple_of(4096) && scan.cancelled() {
                return pairs;
            }
            start -= 1;
        }
        // A failed opener loses ONE tick in the existing grammar. Every suffix
        // of this run can therefore open; only whole later runs can close it.
        for at in start..end {
            if at.is_multiple_of(4096) && scan.cancelled() {
                return pairs;
            }
            pairs[at] = (end, next[end - at]);
        }
        next[end - start] = start;
        end = start;
    }
    pairs
}

fn emphasis_matches(src: &[u8], marker: u8, scan: &Scan<'_>) -> Vec<[usize; 2]> {
    let mut next = [src.len(); 2];
    let mut pairs = vec![next; src.len() + 1];
    for at in (0..src.len()).rev() {
        if at.is_multiple_of(4096) && scan.cancelled() {
            break;
        }
        if src[at] == marker && at > 0 && src[at - 1] != b' ' {
            let slashes = src[..at].iter().rev().take_while(|&&c| c == b'\\').count();
            if slashes.is_multiple_of(2) {
                next[0] = at;
                if src.get(at + 1) == Some(&marker) {
                    next[1] = at;
                }
            }
        }
        pairs[at] = next;
    }
    pairs
}

fn utf8_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}
