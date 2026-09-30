//! Markdown as a block model (markdown.rs `Markdown::parse`, without
//! pulldown-cmark or GPUI): what the transcript view and the message overlay
//! render. The parser is by hand and covers what agent replies use: ATX
//! headings, paragraphs with soft and hard breaks, block quotes, bullet,
//! ordered and task lists (nested by indentation), fenced code (a fence left
//! open by a streaming reply still counts), pipe tables with an alignment
//! row, thematic breaks, and inline code, bold, italic, strikethrough, links,
//! images (`[Image: alt]`), autolinks and backslash escapes. Literal HTML
//! stays as text, as the source's `Event::Html` did.
//!
//! `parse` gives the nested [`Block`] tree; `to_json` flattens it into the
//! records a Contract iterates (no recursion there), one per rendered row:
//!
//! ```text
//! shape MarkdownRun
//!   text: string      // the words
//!   bold: bool
//!   italic: bool
//!   code: bool        // inline code: Menlo on `element`
//!   strike: bool
//!   url: string       // a link's target, "" for plain text
//!   subdued: bool     // progress text: italic `muted`
//! shape MarkdownCell
//!   runs: list<MarkdownRun>
//! shape MarkdownTableRow
//!   header: bool      // the first row: bold on `elevated`
//!   cells: list<MarkdownCell>
//! shape MarkdownBlock
//!   kind: string      // heading | paragraph | code | table | rule
//!   level: number     // heading level 1-4 (H4+ share a size), else 0
//!   depth: number     // list nesting, 0 outside lists
//!   marker: string    // "•", "3.", "☑", "☐", or "" (a list item's later blocks)
//!   quote: number     // block quote nesting, 0 outside quotes
//!   language: string  // a code block's info word
//!   text: string      // a code block's text
//!   subdued: bool     // the whole block is progress text
//!   runs: list<MarkdownRun>          // heading and paragraph text
//!   rows: list<MarkdownTableRow>     // a table's rows, header first
//!   align: list<string>              // a table's column alignment: left | center | right
//! ```
//!
//! A list item's first block carries the marker and depth; the item's other
//! blocks carry the depth with an empty marker, so the marker column stays
//! blank under them. A nested list's items sit one depth deeper.

use serde_json::{json, Value as Json};

/// One styled span of inline text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// The words.
    pub text: String,
    /// `**bold**`.
    pub bold: bool,
    /// `*italic*`.
    pub italic: bool,
    /// `` `code` ``.
    pub code: bool,
    /// `~~strike~~`.
    pub strike: bool,
    /// A link's target.
    pub url: Option<String>,
}

/// A table column's alignment, from its separator cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// `---`.
    None,
    /// `:---`.
    Left,
    /// `:---:`.
    Center,
    /// `---:`.
    Right,
}

/// One item of a list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListItem {
    /// `[ ]` / `[x]` at the item's start.
    pub task: Option<bool>,
    /// The item's content, a nested list included.
    pub blocks: Vec<Block>,
}

/// One block of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// `# Heading`, level 1-4.
    Heading(u8, Vec<Run>),
    /// Running text.
    Paragraph(Vec<Run>),
    /// `> quoted`.
    BlockQuote(Vec<Block>),
    /// A bullet (`None`) or ordered (`Some(start)`) list.
    List(Option<u64>, Vec<ListItem>),
    /// Fenced code: (language, text without the closing newline).
    Code(String, String),
    /// A pipe table: alignments, the header cells, the body rows.
    Table(Vec<Align>, Vec<Vec<Run>>, Vec<Vec<Vec<Run>>>),
    /// `---`.
    Rule,
}

/// Parse a document.
pub fn parse(source: &str) -> Vec<Block> {
    let lines: Vec<&str> = source.lines().collect();
    blocks(&lines)
}

/// Web and mail links open externally; local paths become clickable only in
/// a session surface with a machine-aware handler (markdown.rs `web_link`).
pub fn web_link(url: &str) -> bool {
    let scheme = url.split(':').next().unwrap_or("").to_ascii_lowercase();
    matches!(scheme.as_str(), "https" | "http" | "mailto")
}

/// The document's words, marks dropped, blocks joined by newlines.
pub fn plain_text(blocks: &[Block]) -> String {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            Block::Heading(_, runs) | Block::Paragraph(runs) => out.push(runs_text(runs)),
            Block::BlockQuote(inner) => out.push(plain_text(inner)),
            Block::List(_, items) => {
                for item in items {
                    out.push(plain_text(&item.blocks));
                }
            }
            Block::Code(_, text) => out.push(text.clone()),
            Block::Table(_, header, rows) => {
                out.push(header.iter().map(|c| runs_text(c)).collect::<Vec<_>>().join(" | "));
                for row in rows {
                    out.push(row.iter().map(|c| runs_text(c)).collect::<Vec<_>>().join(" | "));
                }
            }
            Block::Rule => {}
        }
    }
    out.join("\n")
}

fn runs_text(runs: &[Run]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

// ----- blocks ----------------------------------------------------------------

fn blocks(lines: &[&str]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }
        if let Some((fence, language)) = fence_open(line) {
            let mut text = Vec::new();
            i += 1;
            while i < lines.len() && !fence_close(lines[i], fence) {
                text.push(lines[i]);
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            out.push(Block::Code(language, text.join("\n")));
            continue;
        }
        if let Some((level, rest)) = heading(trimmed) {
            out.push(Block::Heading(level, inline(rest.trim())));
            i += 1;
            continue;
        }
        if is_rule(trimmed) {
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if trimmed.starts_with('>') {
            let mut quoted = Vec::new();
            while i < lines.len() {
                let t = lines[i].trim_start();
                if let Some(rest) = t.strip_prefix('>') {
                    quoted.push(rest.strip_prefix(' ').unwrap_or(rest));
                } else if t.is_empty() || quoted.is_empty() {
                    break;
                } else if paragraph_continues(t) {
                    // Lazy continuation.
                    quoted.push(lines[i]);
                } else {
                    break;
                }
                i += 1;
            }
            out.push(Block::BlockQuote(blocks(&quoted)));
            continue;
        }
        if let Some(marker) = list_marker(line) {
            let (block, next) = list(lines, i, marker);
            out.push(block);
            i = next;
            continue;
        }
        if i + 1 < lines.len() && line.contains('|') && is_table_separator(lines[i + 1]) {
            let header = table_cells(line);
            let align: Vec<Align> = table_cells_raw(lines[i + 1])
                .iter()
                .map(|c| alignment(c))
                .collect();
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                let mut row = table_cells(lines[i]);
                row.resize_with(header.len(), Vec::new);
                rows.push(row);
                i += 1;
            }
            out.push(Block::Table(align, header, rows));
            continue;
        }
        // A paragraph: until a blank line or another block's start.
        let mut text = vec![line.trim_start()];
        i += 1;
        while i < lines.len() {
            let next = lines[i].trim_start();
            if next.is_empty() || !paragraph_continues(next) {
                break;
            }
            text.push(lines[i].trim_start());
            i += 1;
        }
        out.push(Block::Paragraph(inline(&text.join("\n"))));
    }
    out
}

/// A line that does not open another block interrupts no paragraph.
fn paragraph_continues(trimmed: &str) -> bool {
    !(fence_open(trimmed).is_some()
        || heading(trimmed).is_some()
        || is_rule(trimmed)
        || trimmed.starts_with('>')
        || list_marker(trimmed).is_some())
}

fn fence_open(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    if line.len() - t.len() > 3 {
        return None;
    }
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|x| *x == c).count();
    if n < 3 {
        return None;
    }
    let info = &t[n..];
    if c == '`' && info.contains('`') {
        return None;
    }
    let language = info.split_whitespace().next().unwrap_or("").to_string();
    Some((t[..n].to_string(), language))
}

fn fence_close(line: &str, fence: &str) -> bool {
    let t = line.trim();
    let c = fence.chars().next().unwrap_or('`');
    t.len() >= fence.len() && t.chars().all(|x| x == c)
}

fn heading(trimmed: &str) -> Option<(u8, &str)> {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let rest = rest.trim().trim_end_matches('#').trim_end();
    Some((hashes.min(4) as u8, rest))
}

fn is_rule(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && (compact.chars().all(|c| c == '-')
            || compact.chars().all(|c| c == '*')
            || compact.chars().all(|c| c == '_'))
}

/// A list item's marker: (indent, marker width, ordered start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Marker {
    indent: usize,
    width: usize,
    start: Option<u64>,
}

fn list_marker(line: &str) -> Option<Marker> {
    let indent = line.len() - line.trim_start().len();
    let t = &line[indent..];
    let first = t.chars().next()?;
    if matches!(first, '-' | '*' | '+') {
        let rest = &t[1..];
        if rest.is_empty() || rest.starts_with(' ') {
            if is_rule(t) {
                return None;
            }
            let spaces = rest.len() - rest.trim_start().len();
            return Some(Marker {
                indent,
                width: 1 + spaces.clamp(1, 4),
                start: None,
            });
        }
        return None;
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let after = &t[digits..];
    if !(after.starts_with('.') || after.starts_with(')')) {
        return None;
    }
    let rest = &after[1..];
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    let spaces = rest.len() - rest.trim_start().len();
    Some(Marker {
        indent,
        width: digits + 1 + spaces.clamp(1, 4),
        start: t[..digits].parse().ok(),
    })
}

/// The list starting at `lines[from]`; returns it and the next line index.
fn list(lines: &[&str], from: usize, first: Marker) -> (Block, usize) {
    let mut items = Vec::new();
    let mut i = from;
    let ordered = first.start.is_some();
    while i < lines.len() {
        let Some(marker) = list_marker(lines[i]) else {
            break;
        };
        if marker.indent != first.indent
            || marker.start.is_some() != ordered
            || marker.indent > first.indent
        {
            break;
        }
        // The item's own lines, dedented by the marker's width.
        let content_indent = marker.indent + marker.width;
        let mut body: Vec<String> = vec![lines[i][content_indent.min(lines[i].len())..].to_string()];
        i += 1;
        let mut blank_run = 0;
        while i < lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                blank_run += 1;
                body.push(String::new());
                i += 1;
                continue;
            }
            let indent = line.len() - line.trim_start().len();
            if indent >= content_indent {
                body.push(line[content_indent..].to_string());
                blank_run = 0;
                i += 1;
                continue;
            }
            if blank_run == 0 && list_marker(line).is_none() && paragraph_continues(line.trim_start())
            {
                // Lazy continuation of the item's paragraph.
                body.push(line.trim_start().to_string());
                i += 1;
                continue;
            }
            break;
        }
        while body.last().is_some_and(|l| l.is_empty()) {
            body.pop();
        }
        let mut task = None;
        if let Some(first_line) = body.first_mut() {
            let t = first_line.trim_start();
            if let Some(rest) = t.strip_prefix("[ ] ").or_else(|| t.strip_prefix("[ ]")) {
                task = Some(false);
                *first_line = rest.to_string();
            } else if let Some(rest) = t
                .strip_prefix("[x] ")
                .or_else(|| t.strip_prefix("[X] "))
                .or_else(|| t.strip_prefix("[x]"))
                .or_else(|| t.strip_prefix("[X]"))
            {
                task = Some(true);
                *first_line = rest.to_string();
            }
        }
        let refs: Vec<&str> = body.iter().map(String::as_str).collect();
        items.push(ListItem {
            task,
            blocks: blocks(&refs),
        });
    }
    (Block::List(first.start, items), i)
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    if !t.contains('-') || !t.contains('|') && !t.starts_with(':') {
        return false;
    }
    let cells = table_cells_raw(t);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim();
            let inner = c.trim_start_matches(':').trim_end_matches(':');
            !inner.is_empty() && inner.chars().all(|x| x == '-')
        })
}

fn table_cells_raw(line: &str) -> Vec<String> {
    let mut t = line.trim();
    if let Some(rest) = t.strip_prefix('|') {
        t = rest;
    }
    if let Some(rest) = t.strip_suffix('|') {
        if !rest.ends_with('\\') {
            t = rest;
        }
    }
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = t.chars().peekable();
    let mut in_code = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '`' => {
                in_code = !in_code;
                cell.push(c);
            }
            '|' if !in_code => cells.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    cells.push(cell);
    cells
}

fn table_cells(line: &str) -> Vec<Vec<Run>> {
    table_cells_raw(line)
        .iter()
        .map(|c| inline(c.trim()))
        .collect()
}

fn alignment(cell: &str) -> Align {
    let c = cell.trim();
    match (c.starts_with(':'), c.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        (false, false) => Align::None,
    }
}

// ----- inline ----------------------------------------------------------------

#[derive(Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
    url: Option<String>,
}

/// Parse inline marks in `text`.
pub fn inline(text: &str) -> Vec<Run> {
    let chars: Vec<char> = text.chars().collect();
    let mut runs = Vec::new();
    append(&chars, &Style::default(), &mut runs);
    runs
}

fn push(runs: &mut Vec<Run>, text: &str, style: &Style, code: bool) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if !code
            && !last.code
            && last.bold == style.bold
            && last.italic == style.italic
            && last.strike == style.strike
            && last.url == style.url
        {
            last.text.push_str(text);
            return;
        }
    }
    runs.push(Run {
        text: text.to_string(),
        bold: style.bold,
        italic: style.italic,
        code,
        strike: style.strike,
        url: style.url.clone(),
    });
}

fn run_len(chars: &[char], at: usize, c: char) -> usize {
    chars[at..].iter().take_while(|x| **x == c).count()
}

fn is_word(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric())
}

/// The closing delimiter run for an opener of `want` characters `c` starting
/// the search at `from`: the index of the run and how many of its characters
/// close (a 3-run closes a 2-opener with its last two, leaving one inside).
fn closer(chars: &[char], from: usize, c: char, want: usize) -> Option<(usize, usize)> {
    let mut i = from;
    let mut code_ticks = 0;
    while i < chars.len() {
        let x = chars[i];
        if x == '\\' {
            i += 2;
            continue;
        }
        if x == '`' {
            let n = run_len(chars, i, '`');
            if code_ticks == 0 {
                code_ticks = n;
            } else if code_ticks == n {
                code_ticks = 0;
            }
            i += n;
            continue;
        }
        if code_ticks == 0 && x == c {
            let n = run_len(chars, i, c);
            let before_ws = i == 0 || chars[i - 1].is_whitespace();
            let after_word = is_word(chars.get(i + n));
            let flanking = !before_ws && (c != '_' || !after_word);
            if flanking {
                if n == want {
                    return Some((i, want));
                }
                if want == 2 && n == 3 {
                    return Some((i + 1, 2));
                }
                if want == 1 && n == 3 {
                    return Some((i + 2, 1));
                }
                if n > want && want == 1 {
                    return Some((i, 1));
                }
            }
            i += n;
            continue;
        }
        i += 1;
    }
    None
}

fn append(chars: &[char], style: &Style, runs: &mut Vec<Run>) {
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if i + 1 < chars.len() => {
                if chars[i + 1] == '\n' {
                    text.push('\n');
                } else if chars[i + 1].is_ascii_punctuation() {
                    text.push(chars[i + 1]);
                } else {
                    text.push('\\');
                    text.push(chars[i + 1]);
                }
                i += 2;
            }
            '\n' => {
                // Two trailing spaces make a hard break; a soft break is a space.
                if text.ends_with("  ") {
                    let trimmed = text.trim_end_matches(' ').len();
                    text.truncate(trimmed);
                    text.push('\n');
                } else {
                    let trimmed = text.trim_end_matches(' ').len();
                    text.truncate(trimmed);
                    text.push(' ');
                }
                i += 1;
                while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                    i += 1;
                }
            }
            '`' => {
                let n = run_len(chars, i, '`');
                let mut j = i + n;
                let mut found = None;
                while j < chars.len() {
                    if chars[j] == '`' {
                        let m = run_len(chars, j, '`');
                        if m == n {
                            found = Some(j);
                            break;
                        }
                        j += m;
                    } else {
                        j += 1;
                    }
                }
                match found {
                    Some(end) => {
                        push(runs, &text, style, false);
                        text.clear();
                        let mut code: String = chars[i + n..end].iter().collect();
                        code = code.replace('\n', " ");
                        if code.len() > 1
                            && code.starts_with(' ')
                            && code.ends_with(' ')
                            && code.trim().is_empty() == false
                        {
                            code = code[1..code.len() - 1].to_string();
                        }
                        push(runs, &code, style, true);
                        i = end + n;
                    }
                    None => {
                        text.extend(std::iter::repeat_n('`', n));
                        i += n;
                    }
                }
            }
            '*' | '_' | '~' => {
                let n = run_len(chars, i, c);
                let before_word = i > 0 && is_word(chars.get(i - 1));
                let after_ws = chars.get(i + n).is_none_or(|x| x.is_whitespace());
                let can_open = !after_ws && (c != '_' || !before_word);
                let want = if c == '~' {
                    if n == 2 {
                        2
                    } else {
                        0
                    }
                } else if n >= 2 {
                    2
                } else {
                    1
                };
                let mut done = false;
                if can_open && want > 0 {
                    let inner_from = i + want;
                    if let Some((at, len)) = closer(chars, inner_from, c, want) {
                        push(runs, &text, style, false);
                        text.clear();
                        let mut next = style.clone();
                        match (c, want) {
                            ('~', _) => next.strike = true,
                            (_, 2) => next.bold = true,
                            _ => next.italic = true,
                        }
                        append(&chars[inner_from..at], &next, runs);
                        i = at + len;
                        done = true;
                    }
                }
                if !done {
                    text.push(c);
                    i += 1;
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => match link(chars, i + 1) {
                Some((label, _url, end)) => {
                    push(runs, &text, style, false);
                    text.clear();
                    push(runs, "[Image: ", style, false);
                    append(&label, style, runs);
                    push(runs, "]", style, false);
                    i = end;
                }
                None => {
                    text.push('!');
                    i += 1;
                }
            },
            '[' => match link(chars, i) {
                Some((label, url, end)) => {
                    push(runs, &text, style, false);
                    text.clear();
                    let mut next = style.clone();
                    next.url = Some(url);
                    append(&label, &next, runs);
                    i = end;
                }
                None => {
                    text.push('[');
                    i += 1;
                }
            },
            '<' => {
                let close = chars[i + 1..].iter().position(|x| *x == '>').map(|p| p + i + 1);
                let inner: Option<String> = close.map(|c| chars[i + 1..c].iter().collect());
                match (close, inner) {
                    (Some(close), Some(inner))
                        if web_link(&inner) && !inner.contains(char::is_whitespace) =>
                    {
                        push(runs, &text, style, false);
                        text.clear();
                        let mut next = style.clone();
                        next.url = Some(inner.clone());
                        push(runs, &inner, &next, false);
                        i = close + 1;
                    }
                    _ => {
                        text.push('<');
                        i += 1;
                    }
                }
            }
            _ => {
                text.push(c);
                i += 1;
            }
        }
    }
    push(runs, &text, style, false);
}

/// `[label](url "title")` at `chars[at] == '['`: the label's characters, the
/// url, and the index after the closing paren.
fn link(chars: &[char], at: usize) -> Option<(Vec<char>, String, usize)> {
    let mut depth = 0;
    let mut close = None;
    let mut i = at;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let close = close?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    let mut j = close + 2;
    let url: String;
    if chars.get(j) == Some(&'<') {
        let end = chars[j + 1..].iter().position(|x| *x == '>')? + j + 1;
        url = chars[j + 1..end].iter().collect();
        j = end + 1;
    } else {
        let start = j;
        let mut parens = 0;
        while j < chars.len() {
            match chars[j] {
                '(' => parens += 1,
                ')' if parens == 0 => break,
                ')' => parens -= 1,
                c if c.is_whitespace() => break,
                _ => {}
            }
            j += 1;
        }
        url = chars[start..j].iter().collect();
    }
    // An optional title, then the closing paren.
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if matches!(chars.get(j), Some('"') | Some('\'')) {
        let quote = chars[j];
        let end = chars[j + 1..].iter().position(|x| *x == quote)? + j + 1;
        j = end + 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
    }
    if chars.get(j) != Some(&')') {
        return None;
    }
    Some((chars[at + 1..close].to_vec(), url, j + 1))
}

// ----- JSON ------------------------------------------------------------------

fn run_json(run: &Run, subdued: bool) -> Json {
    json!({
        "text": run.text,
        "bold": run.bold,
        "italic": run.italic,
        "code": run.code,
        "strike": run.strike,
        "url": run.url.clone().unwrap_or_default(),
        "subdued": subdued,
    })
}

fn runs_json(runs: &[Run], subdued: bool) -> Vec<Json> {
    runs.iter().map(|r| run_json(r, subdued)).collect()
}

fn block_json(kind: &str, depth: usize, marker: &str, quote: usize, subdued: bool) -> Json {
    json!({
        "kind": kind, "level": 0, "depth": depth, "marker": marker, "quote": quote,
        "language": "", "text": "", "subdued": subdued, "runs": [], "rows": [], "align": [],
    })
}

fn set(v: &mut Json, key: &str, value: Json) {
    if let Some(map) = v.as_object_mut() {
        map.insert(key.into(), value);
    }
}

fn flatten(blocks: &[Block], depth: usize, marker: &str, quote: usize, subdued: bool, out: &mut Vec<Json>) {
    let mut marker = marker;
    for block in blocks {
        match block {
            Block::Heading(level, runs) => {
                let mut v = block_json("heading", depth, marker, quote, subdued);
                set(&mut v, "level", json!(level));
                set(&mut v, "runs", Json::Array(runs_json(runs, subdued)));
                out.push(v);
            }
            Block::Paragraph(runs) => {
                let mut v = block_json("paragraph", depth, marker, quote, subdued);
                set(&mut v, "runs", Json::Array(runs_json(runs, subdued)));
                out.push(v);
            }
            Block::BlockQuote(inner) => {
                let before = out.len();
                flatten(inner, depth, marker, quote + 1, subdued, out);
                if out.len() == before {
                    let v = block_json("paragraph", depth, marker, quote + 1, subdued);
                    out.push(v);
                }
            }
            Block::List(start, items) => {
                for (index, item) in items.iter().enumerate() {
                    let item_marker = match item.task {
                        Some(true) => "☑".to_string(),
                        Some(false) => "☐".to_string(),
                        None => match start {
                            Some(start) => format!("{}.", start.saturating_add(index as u64)),
                            None => "•".to_string(),
                        },
                    };
                    let before = out.len();
                    flatten(&item.blocks, depth + 1, &item_marker, quote, subdued, out);
                    if out.len() == before {
                        out.push(block_json("paragraph", depth + 1, &item_marker, quote, subdued));
                    }
                }
            }
            Block::Code(language, text) => {
                let mut v = block_json("code", depth, marker, quote, subdued);
                set(&mut v, "language", json!(language));
                set(&mut v, "text", json!(text.trim_end_matches('\n')));
                out.push(v);
            }
            Block::Table(align, header, rows) => {
                let mut v = block_json("table", depth, marker, quote, subdued);
                let row = |cells: &Vec<Vec<Run>>, header: bool| {
                    json!({
                        "header": header,
                        "cells": cells.iter().map(|c| json!({ "runs": runs_json(c, subdued) })).collect::<Vec<_>>(),
                    })
                };
                let mut all = vec![row(header, true)];
                all.extend(rows.iter().map(|r| row(r, false)));
                set(&mut v, "rows", Json::Array(all));
                set(
                    &mut v,
                    "align",
                    Json::Array(
                        align
                            .iter()
                            .map(|a| {
                                json!(match a {
                                    Align::Center => "center",
                                    Align::Right => "right",
                                    _ => "left",
                                })
                            })
                            .collect(),
                    ),
                );
                out.push(v);
            }
            Block::Rule => out.push(block_json("rule", depth, marker, quote, subdued)),
        }
        // Only a list item's first block carries its marker.
        marker = "";
    }
}

/// The flat `MarkdownBlock` records for a document; `subdued` marks every
/// block and run as progress text.
pub fn to_json(blocks: &[Block], subdued: bool) -> Vec<Json> {
    let mut out = Vec::new();
    flatten(blocks, 0, "", 0, subdued, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(source: &str) -> Vec<Run> {
        match parse(source).into_iter().next() {
            Some(Block::Paragraph(runs)) => runs,
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    fn text(runs: &[Run]) -> String {
        runs_text(runs)
    }

    #[test]
    fn nested_emphasis_code_and_unicode_have_correct_runs() {
        let runs = paragraph("Hello **bold *café*** and `a_b()` with ~~old~~.");
        assert_eq!(text(&runs), "Hello bold café and a_b() with old.");
        let cafe = runs.iter().find(|r| r.text == "café").unwrap();
        assert!(cafe.bold && cafe.italic);
        let code = runs.iter().find(|r| r.text == "a_b()").unwrap();
        assert!(code.code);
        assert!(runs.iter().find(|r| r.text == "old").unwrap().strike);
    }

    #[test]
    fn progress_styling_applies_to_every_run() {
        let doc = parse("**Working** on `file.rs`; see [details](https://example.com).");
        let flat = to_json(&doc, true);
        assert_eq!(flat.len(), 1);
        let runs = flat[0]["runs"].as_array().unwrap();
        assert!(!runs.is_empty());
        for run in runs {
            assert_eq!(run["subdued"], true);
        }
        assert_eq!(
            runs.iter().filter(|r| r["url"] == "https://example.com").count(),
            1
        );
    }

    #[test]
    fn links_keep_their_targets_and_only_web_or_mail_open_externally() {
        let runs = paragraph(
            "é [**docs**](https://example.com) [run](javascript:alert) [local](/tmp/file)",
        );
        assert_eq!(text(&runs), "é docs run local");
        let docs = runs.iter().find(|r| r.text == "docs").unwrap();
        assert!(docs.bold);
        assert_eq!(docs.url.as_deref(), Some("https://example.com"));
        assert_eq!(
            runs.iter().find(|r| r.text == "local").unwrap().url.as_deref(),
            Some("/tmp/file")
        );
        assert!(web_link("mailto:hello@example.com"));
        assert!(!web_link("file:///tmp/example"));
        assert!(!web_link("command:delete"));
        assert!(!web_link("javascript:alert"));
    }

    #[test]
    fn session_links_accept_paths_with_spaces() {
        let runs = paragraph(
            "[report](</tmp/Ocho remote report.pdf>) [binary](<file:///tmp/Ocho%20sample.bin>)",
        );
        let urls: Vec<&str> = runs.iter().filter_map(|r| r.url.as_deref()).collect();
        assert_eq!(
            urls,
            vec!["/tmp/Ocho remote report.pdf", "file:///tmp/Ocho%20sample.bin"]
        );
        let runs = parse("[report](./out/report.pdf) [web](https://example.com)");
        let Block::Paragraph(runs) = &runs[0] else {
            panic!("paragraph");
        };
        assert_eq!(runs[0].url.as_deref(), Some("./out/report.pdf"));
        assert_eq!(runs[1].text, " ");
        assert_eq!(runs[2].url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn preserves_nested_lists_tasks_tables_and_literal_code() {
        let doc = parse(
            "## Result\n\n3. First\n   - Nested\n4. Next\n\n- [x] Done\n\n> Quoted\n\n| Name | Value |\n| --- | ---: |\n| **one** | 2 |\n\n```rust\nlet x = \"**literal**\";\n```",
        );
        assert!(matches!(&doc[0], Block::Heading(2, _)));
        let Block::List(Some(3), items) = &doc[1] else {
            panic!("ordered list lost: {:?}", doc[1]);
        };
        assert_eq!(items.len(), 2);
        assert!(items[0]
            .blocks
            .iter()
            .any(|b| matches!(b, Block::List(None, _))));
        let Block::List(None, items) = &doc[2] else {
            panic!("task list lost");
        };
        assert_eq!(items[0].task, Some(true));
        assert_eq!(plain_text(&items[0].blocks), "Done");
        assert!(matches!(&doc[3], Block::BlockQuote(_)));
        let Block::Table(align, header, rows) = &doc[4] else {
            panic!("table lost: {:?}", doc[4]);
        };
        assert_eq!(align, &[Align::None, Align::Right]);
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 1);
        assert!(rows[0][0][0].bold);
        let Block::Code(language, code) = &doc[5] else {
            panic!("code lost");
        };
        assert_eq!(language, "rust");
        assert_eq!(code, "let x = \"**literal**\";");
    }

    #[test]
    fn streaming_fences_breaks_and_html_preserve_visible_text() {
        let doc = parse("```sh\necho 'hello'");
        assert_eq!(doc[0], Block::Code("sh".into(), "echo 'hello'".into()));
        assert_eq!(text(&paragraph("one\ntwo  \nthree")), "one two\nthree");
        assert_eq!(
            text(&paragraph("a <b>literal</b> tag")),
            "a <b>literal</b> tag"
        );
        assert_eq!(
            text(&paragraph("![a diagram](https://example.com/image.png)")),
            "[Image: a diagram]"
        );
        assert_eq!(
            text(&paragraph("<https://example.com> and a\\*b")),
            "https://example.com and a*b"
        );
    }

    #[test]
    fn underscores_inside_words_are_text() {
        let runs = paragraph("snake_case stays, _em_ goes, __strong__ too");
        assert_eq!(text(&runs), "snake_case stays, em goes, strong too");
        assert!(runs.iter().find(|r| r.text == "em").unwrap().italic);
        assert!(runs.iter().find(|r| r.text == "strong").unwrap().bold);
        assert_eq!(text(&paragraph("a * b * c")), "a * b * c");
        assert_eq!(text(&paragraph("***both***")), "both");
        let both = paragraph("***both***");
        assert!(both[0].bold && both[0].italic);
    }

    #[test]
    fn flat_json_carries_markers_depth_and_quotes() {
        let doc = parse("- one\n  - two\n\n  more\n- [ ] todo\n\n> # Q\n> text\n\n---");
        let flat = to_json(&doc, false);
        let rows: Vec<(String, u64, String, u64)> = flat
            .iter()
            .map(|b| {
                (
                    b["kind"].as_str().unwrap().to_string(),
                    b["depth"].as_u64().unwrap(),
                    b["marker"].as_str().unwrap().to_string(),
                    b["quote"].as_u64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("paragraph".into(), 1, "•".into(), 0),
                ("paragraph".into(), 2, "•".into(), 0),
                ("paragraph".into(), 1, "".into(), 0),
                ("paragraph".into(), 1, "☐".into(), 0),
                ("heading".into(), 0, "".into(), 1),
                ("paragraph".into(), 0, "".into(), 1),
                ("rule".into(), 0, "".into(), 0),
            ]
        );
        assert_eq!(flat[4]["level"], 1);
        assert_eq!(flat[2]["runs"][0]["text"], "more");
        let table = to_json(&parse("| a | b |\n|:-:|--|\n| 1 | 2 |"), false);
        assert_eq!(table[0]["kind"], "table");
        assert_eq!(table[0]["rows"][0]["header"], true);
        assert_eq!(table[0]["rows"][1]["cells"][1]["runs"][0]["text"], "2");
        assert_eq!(table[0]["align"][0], "center");
    }

    #[test]
    fn headings_rules_and_ordered_markers() {
        let doc = parse("# One\n##### Deep\n* * *\n1) a\n2) b\n\ntext\n---");
        assert!(matches!(&doc[0], Block::Heading(1, _)));
        assert!(matches!(&doc[1], Block::Heading(4, _)));
        assert_eq!(doc[2], Block::Rule);
        let Block::List(Some(1), items) = &doc[3] else {
            panic!("ordered list: {:?}", doc[3]);
        };
        assert_eq!(items.len(), 2);
        let flat = to_json(&doc, false);
        assert_eq!(flat[4]["marker"], "2.");
        assert!(matches!(&doc[4], Block::Paragraph(_)));
        assert_eq!(doc[5], Block::Rule);
    }
}
