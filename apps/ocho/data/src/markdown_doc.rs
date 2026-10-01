//! Markdown as a block model (markdown.rs `Markdown::parse`, without
//! pulldown-cmark or GPUI): what the transcript view and the message overlay
//! render. The parser is by hand and covers what agent replies use: ATX and
//! setext headings, paragraphs with soft and hard breaks, block quotes,
//! bullet, ordered and task lists (nested by indentation), fenced code (a
//! fence left open by a streaming reply still counts) and indented code,
//! pipe tables with an alignment row, thematic breaks, and inline code,
//! bold, italic, strikethrough, links, images (`[Image: alt]`), autolinks,
//! entities, backslash escapes and LaTeX math (`$…$`, `$$…$$`, `\(…\)`,
//! `\[…\]`; see [`math`]). Literal HTML stays as text, as the source's
//! `Event::Html` did. Only web, mail and local-file links are clickable
//! (`web_link` or `remote_file::path_from_link`); any other link is text.
//!
//! `parse` gives the nested [`Block`] tree; `to_json` flattens it into the
//! records a Contract iterates (no recursion there), one per rendered row:
//!
//! ```text
//! shape MarkdownRun
//!   text: string      // the words; a formula's TeX source when math != ""
//!   bold: bool
//!   italic: bool
//!   code: bool        // inline code: Menlo on `element`
//!   strike: bool
//!   url: string       // a clickable link's target, "" for plain text
//!   subdued: bool     // progress text: italic `muted`
//!   math: string      // "" | inline | display (display only inside table cells)
//! shape MarkdownCell
//!   runs: list<MarkdownRun>
//!   flow: bool        // the runs hold a formula: lay out with markdown_doc::flow
//! shape MarkdownTableRow
//!   header: bool      // the first row: bold on `elevated`
//!   cells: list<MarkdownCell>
//! shape MarkdownBlock
//!   kind: string      // heading | paragraph | code | table | rule | math
//!   level: number     // heading level 1-4 (H4+ share a size), else 0
//!   depth: number     // list nesting, 0 outside lists
//!   marker: string    // "•", "3.", or "" (task items and a list item's later blocks)
//!   quote: number     // block quote nesting, 0 outside quotes
//!   language: string  // a code block's info word
//!   text: string      // a code block's text; a math block's TeX source
//!   subdued: bool     // the whole block is progress text
//!   flow: bool        // runs hold an inline formula: lay out with markdown_doc::flow
//!   runs: list<MarkdownRun>          // heading and paragraph text
//!   rows: list<MarkdownTableRow>     // a table's rows, header first
//!   align: list<string>              // a table's column alignment: left | center | right
//! ```
//!
//! A list item's first block carries the marker and depth; the item's other
//! blocks carry the depth with an empty marker, so the marker column stays
//! blank under them. A nested list's items sit one depth deeper. A task item
//! has an empty marker and its first paragraph starts with a "☑ " or "☐ "
//! run (pulldown's `TaskListMarker`, which replaces the bullet). A display
//! formula inside a paragraph or heading cuts it (upstream `math_flow`): the
//! text before, a `math` record, the text after, each its own record (only
//! the first carries the marker).
//!
//! Drawing (markdown.rs `blocks`/`block`, 1:1): records stack with `gap_3`
//! (12 px); the base text is the surface's (transcript 15 px, line height
//! 1.7). Paragraph: the runs. Heading: `pt_2` (8 px) above, bold, size by
//! level 1 → 25, 2 → 21, 3 → 18, 4 → 16 px, line height size × 1.4. Quote
//! (per nesting level): `border_l_2` in `border`, `pl_4` (16 px). List: items
//! `gap_2` (8 px); a row of the marker column (`min_w` 24 px, `muted`) and
//! the content, `gap_2` between. Code: `rounded_lg` (8 px) on `elevated`,
//! clipped; a header row `px_4 py_2`, 12 px `muted`, the language left and a
//! "Copy" button right (`px_2`, `rounded_md` 6 px; hover: `hover` bg, `text`
//! colour; copies `text`); then the code, `px_4 pb_4`, Menlo 13 px, line
//! height 21 px, no wrapping, scrolling sideways (`muted` italic when
//! subdued). Table: scrolling sideways, `min_w` 140 px per column, every row
//! `border_b_1` in `border`, the header row on `elevated` and bold; cells
//! `flex_1`, `px_3 py_2` (12 × 8 px), aligned per column (none = left).
//! Rule: 1 px of `border`, `my_2` (8 px above and below). Math: see
//! [`math`]. Runs: `.SystemUIFont`, Menlo for code (on `element`), bold
//! and italic as marked, a 1 px strikethrough; links `accent` with a 1 px
//! underline; subdued runs `muted` italic, otherwise `text`.

use serde_json::{json, Value as Json};

pub mod flow;
mod inline;
pub mod math;
#[cfg(test)]
mod tests;

pub use inline::inline;

/// Whether a run is a formula.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MathKind {
    /// Text.
    #[default]
    None,
    /// `$…$` / `\(…\)`: on the baseline.
    Inline,
    /// `$$…$$` / `\[…\]`: on its own row.
    Display,
}

/// One styled span of inline text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    /// The words, or a formula's TeX source.
    pub text: String,
    /// `**bold**`.
    pub bold: bool,
    /// `*italic*`.
    pub italic: bool,
    /// `` `code` ``.
    pub code: bool,
    /// `~~strike~~`.
    pub strike: bool,
    /// A clickable link's target.
    pub url: Option<String>,
    /// A formula.
    pub math: MathKind,
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
    /// `# Heading` or a setext heading, level 1-4.
    Heading(u8, Vec<Run>),
    /// Running text.
    Paragraph(Vec<Run>),
    /// `> quoted`.
    #[allow(clippy::enum_variant_names)]
    BlockQuote(Vec<Block>),
    /// A bullet (`None`) or ordered (`Some(start)`) list.
    List(Option<u64>, Vec<ListItem>),
    /// Fenced or indented code: (language, text without the closing newline).
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

/// A link to a file on the session's machine (remote_file.rs
/// `path_from_link`): `file://` (local host only) or a path that is
/// absolute, `./`, `../`, `~/`, or ends in a name with an extension; the
/// path, percent-decoded, without its `#fragment`.
pub fn path_from_link(link: &str) -> Option<String> {
    let link = link.split_once('#').map_or(link, |(path, _)| path);
    let path = if link
        .get(..7)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file://"))
    {
        let rest = &link[7..];
        if rest.starts_with('/') {
            rest
        } else {
            let local = rest
                .strip_prefix("localhost")
                .or_else(|| rest.strip_prefix("127.0.0.1"))?;
            if !local.starts_with('/') {
                return None;
            }
            local
        }
    } else {
        if link.starts_with("//") || link.contains(':') {
            return None;
        }
        link
    };
    if path.is_empty() || path.ends_with('/') || path.chars().any(char::is_control) {
        return None;
    }
    if !path.starts_with('/')
        && !path.starts_with("./")
        && !path.starts_with("../")
        && !path.starts_with("~/")
        && !path.rsplit('/').next()?.contains('.')
    {
        return None;
    }
    percent_decode(path)
}

fn percent_decode(path: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(path.len());
    let mut input = path.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let hi = (input.next()? as char).to_digit(16)?;
            let lo = (input.next()? as char).to_digit(16)?;
            bytes.push((hi * 16 + lo) as u8);
        } else {
            bytes.push(byte);
        }
    }
    let decoded = String::from_utf8(bytes).ok()?;
    (!decoded.chars().any(char::is_control)).then_some(decoded)
}

/// A link the renderer makes clickable (markdown.rs `InlineText::append`
/// with a link handler, as every block surface has).
pub fn link_allowed(url: &str) -> bool {
    web_link(url) || path_from_link(url).is_some()
}

/// The document's words, marks dropped, blocks joined by newlines; formulas
/// keep their delimiters.
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
                out.push(
                    header
                        .iter()
                        .map(|c| runs_text(c))
                        .collect::<Vec<_>>()
                        .join(" | "),
                );
                for row in rows {
                    out.push(
                        row.iter()
                            .map(|c| runs_text(c))
                            .collect::<Vec<_>>()
                            .join(" | "),
                    );
                }
            }
            Block::Rule => {}
        }
    }
    out.join("\n")
}

/// The runs' text in a line, as a compact surface shows it (markdown.rs
/// `render_inline`, `literal_math`): formulas with their delimiters.
pub fn runs_text(runs: &[Run]) -> String {
    runs.iter()
        .map(|r| match r.math {
            MathKind::None => r.text.clone(),
            MathKind::Inline => math::literal(&r.text, false),
            MathKind::Display => math::literal(&r.text, true),
        })
        .collect()
}

// ----- blocks ----------------------------------------------------------------

/// Columns of leading whitespace (a tab to the next multiple of 4).
fn indent_of(line: &str) -> usize {
    let mut cols = 0;
    for c in line.chars() {
        match c {
            ' ' => cols += 1,
            '\t' => cols += 4 - cols % 4,
            _ => break,
        }
    }
    cols
}

/// `line` without its first `cols` columns of indentation.
fn dedent(line: &str, cols: usize) -> &str {
    let mut seen = 0;
    for (i, c) in line.char_indices() {
        if seen >= cols {
            return &line[i..];
        }
        match c {
            ' ' => seen += 1,
            '\t' => seen += 4 - seen % 4,
            _ => return &line[i..],
        }
    }
    ""
}

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
        if indent_of(line) >= 4 {
            // Indented code, to the last indented line before a dedent.
            let mut text: Vec<&str> = Vec::new();
            while i < lines.len() && (lines[i].trim().is_empty() || indent_of(lines[i]) >= 4) {
                text.push(dedent(lines[i], 4));
                i += 1;
            }
            while text.last().is_some_and(|l| l.trim().is_empty()) {
                text.pop();
            }
            out.push(Block::Code(String::new(), text.join("\n")));
            continue;
        }
        if let Some((fence, language)) = fence_open(line) {
            let mut text = Vec::new();
            let indent = indent_of(line);
            i += 1;
            while i < lines.len() && !fence_close(lines[i], &fence) {
                text.push(dedent(lines[i], indent));
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            out.push(Block::Code(language, text.join("\n")));
            continue;
        }
        if let Some((level, rest)) = heading(trimmed) {
            out.push(Block::Heading(level, inline(rest)));
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
        if i + 1 < lines.len()
            && line.contains('|')
            && is_table_separator(lines[i + 1])
            && table_cells_raw(line).len() == table_cells_raw(lines[i + 1]).len()
        {
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
        // A paragraph: until a blank line or another block's start; a setext
        // underline turns it into a heading.
        let mut text = vec![trimmed];
        let mut level = 0;
        i += 1;
        while i < lines.len() {
            let next = lines[i].trim_start();
            if next.is_empty() {
                break;
            }
            if indent_of(lines[i]) < 4 {
                if let Some(l) = setext(next) {
                    level = l;
                    i += 1;
                    break;
                }
            }
            if !paragraph_continues(next) {
                break;
            }
            text.push(next);
            i += 1;
        }
        let joined = text.join("\n");
        let runs = inline(joined.trim_end());
        out.push(if level > 0 {
            Block::Heading(level, runs)
        } else {
            Block::Paragraph(runs)
        });
    }
    out
}

/// A setext underline: `===` (level 1) or `---` (level 2).
fn setext(trimmed: &str) -> Option<u8> {
    let t = trimmed.trim_end();
    if !t.is_empty() && t.chars().all(|c| c == '=') {
        return Some(1);
    }
    if !t.is_empty() && t.chars().all(|c| c == '-') {
        return Some(2);
    }
    None
}

/// A line that does not open another block interrupts no paragraph. Only a
/// bullet or an ordered list starting at 1, with content, interrupts one.
fn paragraph_continues(trimmed: &str) -> bool {
    let interrupting_list = list_marker(trimmed).is_some_and(|m| {
        m.start.is_none_or(|s| s == 1) && !trimmed[m.width.min(trimmed.len())..].trim().is_empty()
    });
    !(fence_open(trimmed).is_some()
        || heading(trimmed).is_some()
        || is_rule(trimmed)
        || trimmed.starts_with('>')
        || interrupting_list)
}

fn fence_open(line: &str) -> Option<(String, String)> {
    let t = line.trim_start();
    if indent_of(line) > 3 {
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
    indent_of(line) <= 3 && t.len() >= fence.len() && t.chars().all(|x| x == c)
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
    let rest = rest.trim();
    // A closing run of `#` counts only after a space (`# C#` keeps its `#`).
    let without = rest.trim_end_matches('#');
    let rest = if without.is_empty() {
        ""
    } else if without.len() < rest.len() && without.ends_with([' ', '\t']) {
        without.trim_end()
    } else {
        rest
    };
    Some((hashes.min(4) as u8, rest))
}

fn is_rule(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && (compact.chars().all(|c| c == '-')
            || compact.chars().all(|c| c == '*')
            || compact.chars().all(|c| c == '_'))
}

/// A list item's marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Marker {
    /// Columns before the marker.
    indent: usize,
    /// The marker and the spaces after it, to the content.
    width: usize,
    /// An ordered list's number.
    start: Option<u64>,
    /// The bullet, or the ordered delimiter: a change starts a new list.
    ch: char,
}

fn list_marker(line: &str) -> Option<Marker> {
    let indent = line.len() - line.trim_start().len();
    let t = &line[indent..];
    let first = t.chars().next()?;
    // Content after more than 4 spaces is indented code one space in.
    let width_after = |rest: &str| {
        let spaces = rest.len() - rest.trim_start().len();
        if rest.trim().is_empty() || spaces > 4 {
            1
        } else {
            spaces
        }
    };
    if matches!(first, '-' | '*' | '+') {
        let rest = &t[1..];
        if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
            if is_rule(t) {
                return None;
            }
            return Some(Marker {
                indent,
                width: 1 + width_after(rest),
                start: None,
                ch: first,
            });
        }
        return None;
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let after = &t[digits..];
    let delimiter = after.chars().next()?;
    if delimiter != '.' && delimiter != ')' {
        return None;
    }
    let rest = &after[1..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    Some(Marker {
        indent,
        width: digits + 1 + width_after(rest),
        start: t[..digits].parse().ok(),
        ch: delimiter,
    })
}

/// The list starting at `lines[from]`; returns it and the next line index.
fn list(lines: &[&str], from: usize, first: Marker) -> (Block, usize) {
    let mut items = Vec::new();
    let mut i = from;
    while i < lines.len() {
        let Some(marker) = list_marker(lines[i]) else {
            break;
        };
        if marker.indent != first.indent || marker.ch != first.ch {
            break;
        }
        // The item's own lines, dedented by the marker's width.
        let content_indent = marker.indent + marker.width;
        let mut body: Vec<String> = vec![lines[i]
            .get(content_indent.min(lines[i].len())..)
            .unwrap_or("")
            .to_string()];
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
            if indent_of(line) >= content_indent {
                body.push(dedent(line, content_indent).to_string());
                blank_run = 0;
                i += 1;
                continue;
            }
            if blank_run == 0
                && list_marker(line).is_none()
                && paragraph_continues(line.trim_start())
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
            let checked = ["[x]", "[X]"].iter().find_map(|p| t.strip_prefix(p));
            let open = t.strip_prefix("[ ]");
            let found = match (open, checked) {
                (Some(rest), _) => Some((false, rest)),
                (_, Some(rest)) => Some((true, rest)),
                _ => None,
            };
            // A task marker needs whitespace (or the line's end) after it.
            if let Some((done, rest)) = found {
                if rest.is_empty() || rest.starts_with([' ', '\t']) {
                    task = Some(done);
                    *first_line = rest.trim_start().to_string();
                }
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
        "math": match run.math {
            MathKind::None => "",
            MathKind::Inline => "inline",
            MathKind::Display => "display",
        },
    })
}

fn runs_json(runs: &[Run], subdued: bool) -> Vec<Json> {
    runs.iter().map(|r| run_json(r, subdued)).collect()
}

fn has_math(runs: &[Run]) -> bool {
    runs.iter().any(|r| r.math != MathKind::None)
}

fn block_json(kind: &str, depth: usize, marker: &str, quote: usize, subdued: bool) -> Json {
    json!({
        "kind": kind, "level": 0, "depth": depth, "marker": marker, "quote": quote,
        "language": "", "text": "", "subdued": subdued, "flow": false, "runs": [], "rows": [], "align": [],
    })
}

fn set(v: &mut Json, key: &str, value: Json) {
    if let Some(map) = v.as_object_mut() {
        map.insert(key.into(), value);
    }
}

/// Where flattening is: the list depth, the pending marker, the quote depth.
struct At<'a> {
    depth: usize,
    marker: &'a str,
    quote: usize,
    subdued: bool,
}

/// A paragraph or heading, cut at its display formulas (`math_flow`).
fn text_records(kind: &str, level: u8, runs: &[Run], at: &mut At, out: &mut Vec<Json>) {
    let mut pieces: Vec<Result<Vec<Run>, String>> = vec![Ok(Vec::new())];
    for run in runs {
        if run.math == MathKind::Display {
            pieces.push(Err(run.text.clone()));
            pieces.push(Ok(Vec::new()));
        } else if let Some(Ok(current)) = pieces.last_mut() {
            current.push(run.clone());
        }
    }
    let cut = pieces.len() > 1;
    for piece in pieces {
        let mut v = match piece {
            Ok(runs) if cut && runs.iter().all(|r| r.text.is_empty()) => continue,
            Ok(runs) => {
                let mut v = block_json(kind, at.depth, at.marker, at.quote, at.subdued);
                set(&mut v, "level", json!(level));
                set(&mut v, "flow", json!(has_math(&runs)));
                set(&mut v, "runs", Json::Array(runs_json(&runs, at.subdued)));
                v
            }
            Err(source) => {
                let mut v = block_json("math", at.depth, at.marker, at.quote, at.subdued);
                set(&mut v, "text", json!(source));
                v
            }
        };
        if kind == "heading" {
            set(&mut v, "level", json!(level));
        }
        out.push(v);
        at.marker = "";
    }
}

fn flatten(blocks: &[Block], at: &mut At, out: &mut Vec<Json>) {
    for block in blocks {
        match block {
            Block::Heading(level, runs) => text_records("heading", *level, runs, at, out),
            Block::Paragraph(runs) => text_records("paragraph", 0, runs, at, out),
            Block::BlockQuote(inner) => {
                let before = out.len();
                let mut inside = At {
                    quote: at.quote + 1,
                    marker: at.marker,
                    ..*at
                };
                flatten(inner, &mut inside, out);
                if out.len() == before {
                    out.push(block_json(
                        "paragraph",
                        at.depth,
                        at.marker,
                        at.quote + 1,
                        at.subdued,
                    ));
                }
            }
            Block::List(start, items) => {
                for (index, item) in items.iter().enumerate() {
                    let item_marker = match (item.task, start) {
                        (Some(_), _) => String::new(),
                        (None, Some(start)) => format!("{}.", start.saturating_add(index as u64)),
                        (None, None) => "•".to_string(),
                    };
                    let mut item_blocks = item.blocks.clone();
                    if let Some(done) = item.task {
                        // The task glyph leads the item's text, as pulldown's
                        // TaskListMarker does.
                        let glyph = Run {
                            text: if done { "☑ " } else { "☐ " }.into(),
                            ..Default::default()
                        };
                        match item_blocks.first_mut() {
                            Some(Block::Paragraph(runs)) => runs.insert(0, glyph),
                            _ => item_blocks.insert(0, Block::Paragraph(vec![glyph])),
                        }
                    }
                    let before = out.len();
                    let mut inside = At {
                        depth: at.depth + 1,
                        marker: &item_marker,
                        quote: at.quote,
                        subdued: at.subdued,
                    };
                    flatten(&item_blocks, &mut inside, out);
                    if out.len() == before {
                        out.push(block_json(
                            "paragraph",
                            at.depth + 1,
                            &item_marker,
                            at.quote,
                            at.subdued,
                        ));
                    }
                }
            }
            Block::Code(language, text) => {
                let mut v = block_json("code", at.depth, at.marker, at.quote, at.subdued);
                set(&mut v, "language", json!(language));
                set(&mut v, "text", json!(text.trim_end_matches('\n')));
                out.push(v);
            }
            Block::Table(align, header, rows) => {
                let mut v = block_json("table", at.depth, at.marker, at.quote, at.subdued);
                let subdued = at.subdued;
                let row = |cells: &Vec<Vec<Run>>, header: bool| {
                    json!({
                        "header": header,
                        "cells": cells
                            .iter()
                            .map(|c| json!({ "runs": runs_json(c, subdued), "flow": has_math(c) }))
                            .collect::<Vec<_>>(),
                    })
                };
                let mut all = vec![row(header, true)];
                all.extend(rows.iter().map(|r| row(r, false)));
                set(&mut v, "rows", Json::Array(all));
                let align = align
                    .iter()
                    .map(|a| {
                        json!(match a {
                            Align::Center => "center",
                            Align::Right => "right",
                            _ => "left",
                        })
                    })
                    .collect();
                set(&mut v, "align", Json::Array(align));
                out.push(v);
            }
            Block::Rule => out.push(block_json(
                "rule", at.depth, at.marker, at.quote, at.subdued,
            )),
        }
        // Only a list item's first block carries its marker.
        at.marker = "";
    }
}

/// The flat `MarkdownBlock` records for a document; `subdued` marks every
/// block and run as progress text.
pub fn to_json(blocks: &[Block], subdued: bool) -> Vec<Json> {
    let mut out = Vec::new();
    let mut at = At {
        depth: 0,
        marker: "",
        quote: 0,
        subdued,
    };
    flatten(blocks, &mut at, &mut out);
    out
}
