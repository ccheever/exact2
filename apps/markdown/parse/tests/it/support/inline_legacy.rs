// Frozen inline parser from 497acca7, used only as the equivalence oracle.
use markdown_parse::Run;

/// The runs of `line`, with `bold`/`italic`/`href` inherited from the
/// context a nested parse started in (a link's label, an emphasis span).
struct Parser<'a> {
    text: &'a str,
    src: &'a [u8],
    at: usize,
    runs: Vec<Run>,
    pending: String,
    style: Run,
}

/// Parse one paragraph's text into runs. `link` resolves a link target the
/// way the document's own location does — a relative `./notes.md` beside an
/// opened file is an absolute path by the time a run carries it, because the
/// host that opens it has no idea where the document came from.
pub fn runs(text: &str, link: &dyn Fn(&str) -> String) -> Vec<Run> {
    let mut p = Parser {
        text,
        src: text.as_bytes(),
        at: 0,
        runs: Vec::new(),
        pending: String::new(),
        style: Run::default(),
    };
    p.run(link);
    p.finish()
}

impl<'a> Parser<'a> {
    fn peek(&self, ahead: usize) -> u8 {
        *self.src.get(self.at + ahead).unwrap_or(&0)
    }

    fn rest(&self) -> &'a str {
        // Input is already valid UTF-8. The cursor advances by whole scalars
        // or past ASCII delimiters, so this checked slice is O(1). Revalidating
        // the entire suffix at each ordinary 'h' made bare-URL probing quadratic.
        &self.text[self.at..]
    }

    /// Close the run being accumulated, if it has anything in it.
    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.pending);
        // One run per style change, never one per character: a paragraph of
        // 4,000 characters is 4,000 nodes if this coalescing is dropped.
        if let Some(last) = self.runs.last_mut() {
            if last.bold == self.style.bold
                && last.italic == self.style.italic
                && last.code == self.style.code
                && last.href == self.style.href
            {
                last.text.push_str(&text);
                return;
            }
        }
        self.runs.push(Run {
            text,
            ..self.style.clone()
        });
    }

    fn finish(mut self) -> Vec<Run> {
        self.flush();
        self.runs
    }

    fn push(&mut self, run: Run) {
        self.flush();
        if !run.text.is_empty() {
            self.runs.push(run);
        }
    }

    fn run(&mut self, link: &dyn Fn(&str) -> String) {
        while self.at < self.src.len() {
            let c = self.peek(0);
            match c {
                // `\*` is a literal asterisk. Only punctuation escapes; a
                // backslash before a letter is a backslash, as CommonMark says.
                b'\\' if self.peek(1).is_ascii_punctuation() => {
                    self.pending.push(self.peek(1) as char);
                    self.at += 2;
                }
                b'`' => {
                    if !self.code_span() {
                        self.pending.push('`');
                        self.at += 1;
                    }
                }
                b'!' if self.peek(1) == b'[' => {
                    // An inline image inside a paragraph is its alt text: an
                    // image is a block in this reader (LLP 1033), and a
                    // paragraph's runs are text the kernel measures.
                    if let Some((label, target)) = self.bracketed(1) {
                        let alt = if label.is_empty() { target } else { label };
                        self.pending.push_str(&alt);
                    } else {
                        self.pending.push('!');
                        self.at += 1;
                    }
                }
                b'[' => {
                    if let Some((label, target)) = self.bracketed(0) {
                        let href = link(&target);
                        let nested = runs(&label, link);
                        for mut run in nested {
                            if run.href.is_empty() {
                                run.href = href.clone();
                            }
                            run.bold |= self.style.bold;
                            run.italic |= self.style.italic;
                            self.push(run);
                        }
                    } else {
                        self.pending.push('[');
                        self.at += 1;
                    }
                }
                // `<https://example.com>` — an autolink is its own text.
                b'<' => {
                    if !self.autolink(link) {
                        self.pending.push('<');
                        self.at += 1;
                    }
                }
                b'*' | b'_' => {
                    if !self.emphasis(c, link) {
                        self.pending.push(c as char);
                        self.at += 1;
                    }
                }
                b'h' if !self.style.code && self.style.href.is_empty() && self.bare_url(link) => {}
                _ => {
                    // One UTF-8 character, not one byte.
                    let width = utf8_width(c);
                    let end = (self.at + width).min(self.src.len());
                    self.pending.push_str(&self.text[self.at..end]);
                    self.at = end;
                }
            }
        }
    }

    /// `` `code` ``, or ``` ``code with a ` in it`` ```.
    fn code_span(&mut self) -> bool {
        let ticks = self.src[self.at..]
            .iter()
            .take_while(|&&c| c == b'`')
            .count();
        let open = self.at + ticks;
        let fence = &self.src[self.at..open];
        let mut at = open;
        while at < self.src.len() {
            if self.src[at] == b'`' {
                let run = self.src[at..].iter().take_while(|&&c| c == b'`').count();
                if run == ticks {
                    let text = std::str::from_utf8(&self.src[open..at]).unwrap_or("");
                    // CommonMark strips one leading and trailing space so
                    // `` ` `` can be written; nothing else is touched.
                    let text = text
                        .strip_prefix(' ')
                        .and_then(|t| t.strip_suffix(' '))
                        .unwrap_or(text);
                    self.push(Run {
                        text: text.to_string(),
                        code: true,
                        href: self.style.href.clone(),
                        ..Run::default()
                    });
                    self.at = at + run;
                    return true;
                }
                at += run;
                continue;
            }
            at += 1;
        }
        let _ = fence;
        false
    }

    /// `[label](target)` starting `skip` bytes in (1 for an image's `!`).
    /// Returns the label and the target with its optional `"title"` dropped.
    fn bracketed(&mut self, skip: usize) -> Option<(String, String)> {
        let start = self.at + skip;
        let close = matching(self.src, start, b'[', b']')?;
        if *self.src.get(close + 1)? != b'(' {
            return None;
        }
        let end = matching(self.src, close + 1, b'(', b')')?;
        let label = std::str::from_utf8(&self.src[start + 1..close])
            .ok()?
            .to_string();
        let inside = std::str::from_utf8(&self.src[close + 2..end]).ok()?.trim();
        // `(url "title")` — the title is not shown anywhere in this reader.
        let target = match inside.find(['"', '\'']) {
            Some(cut) => inside[..cut].trim(),
            None => inside,
        };
        let target = target.trim_start_matches('<').trim_end_matches('>');
        self.at = end + 1;
        Some((label, target.to_string()))
    }

    /// `<https://…>` and `<someone@example.com>`.
    fn autolink(&mut self, link: &dyn Fn(&str) -> String) -> bool {
        let Some(close) = self.src[self.at..].iter().position(|&c| c == b'>') else {
            return false;
        };
        let inner = std::str::from_utf8(&self.src[self.at + 1..self.at + close]).unwrap_or("");
        let is_url = inner.starts_with("http://") || inner.starts_with("https://");
        let is_mail = !is_url
            && inner.contains('@')
            && !inner.contains(' ')
            && !inner.contains('/')
            && !inner.starts_with('/');
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
            ..self.style.clone()
        });
        self.at += close + 1;
        true
    }

    /// A bare `https://…` in running prose, which is how most of a
    /// repository's Markdown actually writes a link.
    fn bare_url(&mut self, link: &dyn Fn(&str) -> String) -> bool {
        let rest = self.rest();
        if !rest.starts_with("http://") && !rest.starts_with("https://") {
            return false;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '<' || c == ')' || c == '"')
            .unwrap_or(rest.len());
        // Trailing sentence punctuation belongs to the sentence.
        let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if url.len() < 12 {
            return false;
        }
        self.push(Run {
            text: url.to_string(),
            href: link(url),
            ..self.style.clone()
        });
        self.at += url.len();
        true
    }

    /// `**bold**`, `*italic*`, `__bold__`, `_italic_`.
    fn emphasis(&mut self, marker: u8, link: &dyn Fn(&str) -> String) -> bool {
        let count = self.src[self.at..]
            .iter()
            .take_while(|&&c| c == marker)
            .count()
            .min(2);
        // `snake_case` and `a * b` are not emphasis: an opener is followed by
        // something other than whitespace, and `_` only opens at a boundary.
        let after = self.peek(count);
        if after == 0 || after == b' ' || after == b'\t' {
            return false;
        }
        if marker == b'_' && count == 1 {
            let before = if self.at == 0 {
                0
            } else {
                self.src[self.at - 1]
            };
            if before.is_ascii_alphanumeric() {
                return false;
            }
        }
        let open = self.at + count;
        let mut at = open;
        while at < self.src.len() {
            if self.src[at] == b'\\' {
                at += 2;
                continue;
            }
            if self.src[at] == marker
                && self.src[at..].iter().take_while(|&&c| c == marker).count() >= count
                && self.src[at - 1] != b' '
            {
                let inner = std::str::from_utf8(&self.src[open..at]).unwrap_or("");
                let mut style = self.style.clone();
                if count == 2 {
                    style.bold = true;
                } else {
                    style.italic = true;
                }
                for mut run in runs(inner, link) {
                    run.bold |= style.bold;
                    run.italic |= style.italic;
                    if run.href.is_empty() {
                        run.href = style.href.clone();
                    }
                    self.push(run);
                }
                self.at = at + count;
                return true;
            }
            at += 1;
        }
        false
    }
}

/// The index of the `close` that matches the `open` at `from`, honouring
/// nesting and backslash escapes; `None` when the line never closes it.
fn matching(src: &[u8], from: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = from;
    while at < src.len() {
        match src[at] {
            b'\\' => at += 1,
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
        at += 1;
    }
    None
}

fn utf8_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}
