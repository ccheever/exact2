// T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3): a saved notebook's Markdown cell, made inert
// (NotebookPreview.tsx `inertNotebookMarkdown`).

/// What a piece of inline HTML leaves in a cell's source (`inert_source`): a noncharacter the parser reads as text,
/// so the text on either side stays apart, as it does around the reference's `html` node (`<a id="x"></a># x`
/// stays a paragraph, and `[x]<b></b>(y)` does not become a link). The runs drop it once parsed.
const HTML_GONE: char = '\u{FDD0}';

/// NotebookPreview's `inertNotebookMarkdown` (upstream `NotebookPreview.tsx`), on the blocks this parser made of
/// `inert_source`'s text, so whatever it reads as an image or a link is caught: a saved notebook loads no URL and
/// turns no authored link into an app action. An image is its alt text and a link its text; inline code is plain
/// text; a Mermaid fence is text.
fn inert_blocks(blocks: &mut Vec<markdown_parse::Block>) {
    for block in blocks.iter_mut() {
        // A line the parser still reads as HTML is shown as text. `inert_source` leaves one only in indented code, or
        // in a code span it reads past a line where the parser starts a new block; upstream shows both as code.
        if matches!(
            block.kind,
            markdown_parse::Kind::Image | markdown_parse::Kind::Html
        ) {
            block.kind = markdown_parse::Kind::Paragraph;
            block.runs = vec![markdown_parse::Run::text(std::mem::take(&mut block.text))];
            block.href.clear();
        }
        // A fence's language is its info string's first word, as mdast's `lang` and the diagram scan read it.
        if matches!(block.kind, markdown_parse::Kind::Code)
            && block
                .href
                .split_whitespace()
                .next()
                .is_some_and(|lang| lang.eq_ignore_ascii_case("mermaid"))
        {
            block.href = "text".into();
        }
        for run in block
            .runs
            .iter_mut()
            .chain(block.cells.iter_mut().flatten())
        {
            if run.text.contains(HTML_GONE) {
                run.text.retain(|c| c != HTML_GONE);
            }
            run.href.clear();
            run.code = false;
        }
    }
    blocks.retain(|block| {
        !matches!(block.kind, markdown_parse::Kind::Paragraph)
            || block.runs.iter().any(|run| !run.text.is_empty())
    });
}

/// A notebook cell's source with its raw HTML gone, as the reference drops each mdast `html` node: an HTML block
/// (CommonMark §4.6) leaves its lines empty, inline HTML (§6.6) leaves `HTML_GONE`, an autolink (§6.5) leaves its
/// address, and any other `<` is escaped, so the parser, which takes a line starting with `<` for HTML, finds none.
/// Fences, indented code, code spans and backslash escapes stay as they are.
fn inert_source(text: &str) -> String {
    if !text.contains('<') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut paragraph: Vec<CellLine<'_>> = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut block: Option<HtmlBlock> = None;
    // The open list item's quote depth and content column, and the quote depth of the line before.
    let mut item: Option<(usize, usize)> = None;
    let mut depth = 0;
    let mut in_paragraph = false;
    for raw in text.split_inclusive('\n') {
        let line = cell_line(raw);
        let body = &raw[..raw.len() - line.ending.len()];
        // Inside a fence, to the line the parser closes it with.
        if let Some((marker, width)) = fence {
            out.push_str(raw);
            let trimmed = body.trim_start();
            if trimmed.chars().take_while(|&c| c == marker).count() >= width
                && trimmed.trim_end_matches(marker).is_empty()
            {
                fence = None;
            }
            continue;
        }
        let blank = line.content.trim().is_empty();
        if let Some(open) = block {
            // A block ends with its container (no lazy line continues HTML), and kinds 6 and 7 at a blank line.
            let left = line.depth < open.depth
                || (!blank
                    && open.column > 0
                    && line.depth == open.depth
                    && line.column < open.column);
            if !left && !(blank && open.kind >= 6) {
                out.push_str(&">".repeat(open.depth));
                out.push_str(line.ending);
                if html_block_ends(open.kind, line.content) {
                    block = None;
                }
                continue;
            }
            block = None;
        }
        if blank {
            flush_paragraph(&mut paragraph, &mut out);
            out.push_str(raw);
            in_paragraph = false;
            depth = line.depth;
            continue;
        }
        // The parser opens a fence on any line that starts with one, whatever its indentation.
        if let Some(opened) = fence_mark(body.trim_start()) {
            flush_paragraph(&mut paragraph, &mut out);
            out.push_str(raw);
            fence = Some(opened);
            in_paragraph = false;
            depth = line.depth;
            continue;
        }
        // An item or a deeper quote opens a container with no paragraph in it yet; a line in a shallower quote
        // continues the paragraph lazily unless it starts a block.
        let continues = in_paragraph && line.item.is_none() && line.depth <= depth;
        let base = match item {
            Some((at, column)) if at == line.depth && line.column >= column => column,
            _ => 0,
        };
        let indent = if line.item.is_some() {
            0
        } else {
            line.column - base
        };
        if indent >= 4 && !continues {
            flush_paragraph(&mut paragraph, &mut out);
            out.push_str(raw);
            in_paragraph = false;
            depth = line.depth;
            continue;
        }
        let kind = if indent < 4 {
            html_block_start(line.content, continues)
        } else {
            None
        };
        let whole = body.trim();
        let ends_paragraph = kind.is_some() || heading_line(whole) || is_rule_line(whole);
        if let Some(column) = line.item {
            item = Some((line.depth, column));
        } else if item.is_some_and(|(at, column)| {
            at != line.depth || (line.column < column && (ends_paragraph || !continues))
        }) {
            item = None;
        }
        depth = line.depth;
        if let Some(kind) = kind {
            flush_paragraph(&mut paragraph, &mut out);
            out.push_str(line.prefix);
            out.push_str(line.ending);
            let column = item
                .filter(|(at, _)| *at == line.depth)
                .map_or(0, |(_, column)| column);
            block = (!html_block_ends(kind, line.content)).then_some(HtmlBlock {
                kind,
                depth: line.depth,
                column,
            });
            in_paragraph = false;
            continue;
        }
        if !continues || ends_paragraph {
            flush_paragraph(&mut paragraph, &mut out);
        }
        paragraph.push(line);
        in_paragraph = !ends_paragraph;
        if ends_paragraph {
            flush_paragraph(&mut paragraph, &mut out);
        }
    }
    flush_paragraph(&mut paragraph, &mut out);
    out
}

/// An HTML block `inert_source` is in: its start condition, and the quote depth and item column it is inside.
#[derive(Clone, Copy)]
struct HtmlBlock {
    kind: u8,
    depth: usize,
    column: usize,
}

/// A line of a cell, split where its containers end (`inert_source`).
struct CellLine<'a> {
    /// Its block quote markers, then any list markers that open items, each with the space after it.
    prefix: &'a str,
    /// The rest, without the line ending.
    content: &'a str,
    ending: &'a str,
    /// How many block quotes it is in.
    depth: usize,
    /// When it opens a list item: its content's column after the quote markers.
    item: Option<usize>,
    /// The column, after the quote markers, of its first character that is not a space.
    column: usize,
}

fn cell_line(raw: &str) -> CellLine<'_> {
    let body = raw
        .strip_suffix('\n')
        .map_or(raw, |body| body.strip_suffix('\r').unwrap_or(body));
    let bytes = body.as_bytes();
    let spaces = |from: usize| {
        bytes[from..]
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count()
    };
    let (mut at, mut depth) = (0, 0);
    while bytes.get(at + spaces(at)) == Some(&b'>') {
        at += spaces(at) + 1;
        depth += 1;
        if matches!(bytes.get(at), Some(b' ' | b'\t')) {
            at += 1;
        }
    }
    let quoted = at;
    let column = spaces(at);
    let mut item = None;
    while let Some((_, width)) = list_marker(&body[at + spaces(at)..]) {
        at += spaces(at) + width;
        item = Some(at - quoted);
    }
    CellLine {
        prefix: &body[..at],
        content: &body[at..],
        ending: &raw[body.len()..],
        depth,
        item,
        column,
    }
}

fn flush_paragraph(paragraph: &mut Vec<CellLine<'_>>, out: &mut String) {
    if !paragraph.is_empty() {
        inline_html_gone(paragraph, out);
        paragraph.clear();
    }
}

/// Writes a paragraph's lines (`inert_source`) with their inline HTML gone. A tag, comment, processing instruction,
/// declaration or CDATA section (CommonMark §6.6) leaves `HTML_GONE` and joins the lines it spans; an autolink
/// (§6.5) leaves its address; a backslash escape and a code span, as the parser reads them, stay; any other `<` is
/// escaped.
fn inline_html_gone(lines: &[CellLine<'_>], out: &mut String) {
    let mut text = String::new();
    let mut breaks = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            breaks.push(text.len());
            text.push('\n');
        }
        text.push_str(line.content);
    }
    let bytes = text.as_bytes();
    let closers = Closers::new(&text);
    let mut ticks = None;
    let mut tick_end = 0;
    let mut writer = LineWriter {
        lines,
        breaks,
        next: 0,
    };
    out.push_str(lines[0].prefix);
    let (mut at, mut kept) = (0, 0);
    while at < bytes.len() {
        match bytes[at] {
            b'\\' if bytes.get(at + 1).is_some_and(u8::is_ascii_punctuation) => at += 2,
            b'`' => {
                if tick_end <= at {
                    tick_end = at + bytes[at..].iter().take_while(|&&b| b == b'`').count();
                }
                let runs = ticks.get_or_insert_with(|| tick_runs(bytes));
                at = tick_span_end(runs, tick_end - at, tick_end).unwrap_or(at + 1);
            }
            b'<' => {
                writer.keep(&text, kept, at, out);
                if let Some(end) = autolink_end(bytes, at) {
                    out.push_str(&text[at + 1..end - 1]);
                    at = end;
                } else if let Some(len) = raw_html_len(&text, at, &closers) {
                    out.push(HTML_GONE);
                    at += len;
                    writer.pass(at);
                } else {
                    out.push_str("\\<");
                    at += 1;
                }
                kept = at;
            }
            _ => at += 1,
        }
    }
    writer.keep(&text, kept, bytes.len(), out);
    out.push_str(lines[lines.len() - 1].ending);
}

/// Writes a paragraph's joined text back as its lines.
struct LineWriter<'a, 'b> {
    lines: &'b [CellLine<'a>],
    /// Where the break after each line but the last is in the joined text.
    breaks: Vec<usize>,
    next: usize,
}

impl LineWriter<'_, '_> {
    /// Writes `text[from..to]`, each line break in it as its line's ending and the next line's prefix.
    fn keep(&mut self, text: &str, from: usize, to: usize, out: &mut String) {
        let mut at = from;
        while self.breaks.get(self.next).is_some_and(|&line| line < to) {
            let line = self.breaks[self.next];
            out.push_str(&text[at..line]);
            out.push_str(self.lines[self.next].ending);
            out.push_str(self.lines[self.next + 1].prefix);
            at = line + 1;
            self.next += 1;
        }
        out.push_str(&text[at..to]);
    }

    /// Passes the line breaks before `to` without writing them: the lines they end join the next.
    fn pass(&mut self, to: usize) {
        while self.breaks.get(self.next).is_some_and(|&line| line < to) {
            self.next += 1;
        }
    }
}

/// The start of each run of backticks in a paragraph, by its length, as the parser pairs code spans: an opener
/// closes at the next later run of its length, and one that does not opens a tick later, a tick shorter.
fn tick_runs(bytes: &[u8]) -> std::collections::HashMap<usize, Vec<usize>> {
    let mut runs: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'`' {
            at += 1;
            continue;
        }
        let len = bytes[at..].iter().take_while(|&&b| b == b'`').count();
        runs.entry(len).or_default().push(at);
        at += len;
    }
    runs
}

/// Where a code span ends when its opener is `len` ticks ending at `from`: after the next run of exactly `len`.
fn tick_span_end(
    runs: &std::collections::HashMap<usize, Vec<usize>>,
    len: usize,
    from: usize,
) -> Option<usize> {
    let starts = runs.get(&len)?;
    starts
        .get(starts.partition_point(|&start| start < from))
        .map(|start| start + len)
}

/// The end of an autolink at `at` (`<scheme:…>` or `<local@domain>`, CommonMark §6.5). Its scan stops at the next
/// `<`, so the autolinks a paragraph tries read it once.
fn autolink_end(bytes: &[u8], at: usize) -> Option<usize> {
    let inner = &bytes[at + 1..];
    let len = inner
        .iter()
        .position(|&b| matches!(b, b'<' | b'>') || b <= b' ' || b == 0x7f)?;
    if len == 0 || inner[len] != b'>' {
        return None;
    }
    let inner = &inner[..len];
    let scheme = inner
        .iter()
        .position(|&b| b == b':')
        .map_or(&inner[..0], |colon| &inner[..colon]);
    let uri = (2..=32).contains(&scheme.len())
        && scheme[0].is_ascii_alphabetic()
        && scheme
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'));
    let email = inner.iter().position(|&b| b == b'@').is_some_and(|sign| {
        let (local, domain) = (&inner[..sign], &inner[sign + 1..]);
        !local.is_empty()
            && local
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+/=?^_`{|}~-".contains(b))
            && domain.split(|&b| b == b'.').all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && label[0] != b'-'
                    && label[label.len() - 1] != b'-'
                    && label
                        .iter()
                        .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
            })
    });
    (uri || email).then_some(at + len + 2)
}

/// What raw HTML waits for to close: an attribute value's quote, a declaration's `>`, a comment's, a processing
/// instruction's and a CDATA section's end.
const CLOSERS: [&str; 6] = ["\"", "'", ">", "-->", "?>", "]]>"];
const DOUBLE_QUOTE: usize = 0;
const SINGLE_QUOTE: usize = 1;
const DECLARATION_END: usize = 2;
const COMMENT_END: usize = 3;
const INSTRUCTION_END: usize = 4;
const CDATA_END: usize = 5;

/// Where each of `CLOSERS` occurs in a paragraph, listed when first wanted, so a closer that never comes is looked
/// for once: `<?` or `<!--` over a whole cell, unclosed, reads in linear time.
struct Closers<'a> {
    text: &'a str,
    starts: [std::cell::OnceCell<Vec<usize>>; 6],
}

impl<'a> Closers<'a> {
    fn new(text: &'a str) -> Self {
        Closers {
            text,
            starts: Default::default(),
        }
    }

    /// The end of the first `CLOSERS[closer]` that starts at or after `from`.
    fn after(&self, closer: usize, from: usize) -> Option<usize> {
        let starts = self.starts[closer].get_or_init(|| {
            self.text
                .match_indices(CLOSERS[closer])
                .map(|(at, _)| at)
                .collect()
        });
        starts
            .get(starts.partition_point(|&start| start < from))
            .map(|start| start + CLOSERS[closer].len())
    }
}

/// The block-level HTML tag names of CommonMark 0.31 (§4.6, start condition 6).
const HTML_BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// The start condition (CommonMark §4.6, 1 to 7) of the HTML block a line's content opens, if it opens one. Only
/// kind 7 cannot interrupt the paragraph the line `continues`.
fn html_block_start(content: &str, continues: bool) -> Option<u8> {
    let line = content.trim_start();
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'<') {
        return None;
    }
    let opens = |prefix: &str| {
        bytes.len() >= prefix.len() && bytes[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
    };
    if ["<script", "<pre", "<style", "<textarea"]
        .iter()
        .any(|open| {
            opens(open) && matches!(bytes.get(open.len()), None | Some(b' ' | b'\t' | b'>'))
        })
    {
        return Some(1);
    }
    if line.starts_with("<!--") {
        return Some(2);
    }
    if line.starts_with("<?") {
        return Some(3);
    }
    if line.starts_with("<![CDATA[") {
        return Some(5);
    }
    if bytes.get(1) == Some(&b'!') && bytes.get(2).is_some_and(u8::is_ascii_alphabetic) {
        return Some(4);
    }
    let name_at = if bytes.get(1) == Some(&b'/') { 2 } else { 1 };
    let name_end = name_at
        + bytes[name_at..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric())
            .count();
    let name = &line[name_at..name_end];
    if HTML_BLOCK_TAGS
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(name))
        && (matches!(bytes.get(name_end), None | Some(b' ' | b'\t' | b'>'))
            || line[name_end..].starts_with("/>"))
    {
        return Some(6);
    }
    if continues || !bytes.get(name_at).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    // Kind 7: one whole open or closing tag of another name, and nothing after it but space.
    let full_name = name_at
        + bytes[name_at..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || **b == b'-')
            .count();
    if ["script", "style", "pre", "textarea"]
        .iter()
        .any(|raw| raw.eq_ignore_ascii_case(&line[name_at..full_name]))
    {
        return None;
    }
    let len = raw_html_len(line, 0, &Closers::new(line))?;
    line[len..].trim().is_empty().then_some(7)
}

/// Whether an HTML block of `kind` ends on a line with this content (CommonMark §4.6's end conditions 1 to 5;
/// kinds 6 and 7 end at a blank line).
fn html_block_ends(kind: u8, content: &str) -> bool {
    match kind {
        1 => {
            let lower = content.to_ascii_lowercase();
            ["</script>", "</pre>", "</style>", "</textarea>"]
                .iter()
                .any(|end| lower.contains(end))
        }
        2 => content.contains("-->"),
        3 => content.contains("?>"),
        4 => content.contains('>'),
        5 => content.contains("]]>"),
        _ => false,
    }
}

/// The length of the raw HTML at `at` in `text` (CommonMark §6.6): an open or closing tag, a comment, a processing
/// instruction, a declaration or a CDATA section.
fn raw_html_len(text: &str, at: usize, closers: &Closers<'_>) -> Option<usize> {
    let bytes = text.as_bytes();
    let rest = &text[at..];
    let until = |from: usize, closer: usize| closers.after(closer, at + from).map(|end| end - at);
    if rest.starts_with("<!-->") {
        return Some(5);
    }
    if rest.starts_with("<!--->") {
        return Some(6);
    }
    if rest.starts_with("<!--") {
        return until(4, COMMENT_END);
    }
    if rest.starts_with("<?") {
        return until(2, INSTRUCTION_END);
    }
    if rest.starts_with("<![CDATA[") {
        return until(9, CDATA_END);
    }
    if rest.starts_with("<!") {
        return bytes
            .get(at + 2)
            .is_some_and(u8::is_ascii_alphabetic)
            .then(|| until(2, DECLARATION_END))
            .flatten();
    }
    let space = |pos: &mut usize| {
        let from = *pos;
        while bytes.get(*pos).is_some_and(u8::is_ascii_whitespace) {
            *pos += 1;
        }
        *pos > from
    };
    let name = |pos: &mut usize, first: fn(&u8) -> bool, more: fn(&u8) -> bool| {
        if !bytes.get(*pos).is_some_and(first) {
            return false;
        }
        *pos += 1;
        while bytes.get(*pos).is_some_and(more) {
            *pos += 1;
        }
        true
    };
    let tag_first = |b: &u8| b.is_ascii_alphabetic();
    let tag_more = |b: &u8| b.is_ascii_alphanumeric() || *b == b'-';
    if rest.starts_with("</") {
        let mut pos = at + 2;
        if !name(&mut pos, tag_first, tag_more) {
            return None;
        }
        space(&mut pos);
        return (bytes.get(pos) == Some(&b'>')).then(|| pos + 1 - at);
    }
    let mut pos = at + 1;
    if !name(&mut pos, tag_first, tag_more) {
        return None;
    }
    loop {
        let before = pos;
        if !space(&mut pos) {
            break;
        }
        let attribute_first = |b: &u8| b.is_ascii_alphabetic() || *b == b'_' || *b == b':';
        let attribute_more =
            |b: &u8| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'.' | b':' | b'-');
        if !name(&mut pos, attribute_first, attribute_more) {
            pos = before;
            break;
        }
        let value_at = pos;
        space(&mut pos);
        if bytes.get(pos) == Some(&b'=') {
            pos += 1;
            space(&mut pos);
            match bytes.get(pos) {
                Some(b'"') => pos = closers.after(DOUBLE_QUOTE, pos + 1)?,
                Some(b'\'') => pos = closers.after(SINGLE_QUOTE, pos + 1)?,
                Some(_) => {
                    let from = pos;
                    while bytes.get(pos).is_some_and(|b| {
                        !b.is_ascii_whitespace()
                            && !matches!(*b, b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
                    }) {
                        pos += 1;
                    }
                    if pos == from {
                        return None;
                    }
                }
                None => return None,
            }
        } else {
            pos = value_at;
        }
    }
    space(&mut pos);
    if bytes.get(pos) == Some(&b'/') {
        pos += 1;
    }
    (bytes.get(pos) == Some(&b'>')).then(|| pos + 1 - at)
}

#[cfg(test)]
mod notebook_inert_tests {
    use super::*;

    /// Every block kind, and every field but a run's or a block's text that names the attacker's host: an image's
    /// source, a link, a media run's source and the link around it (a chat run is
    /// `[index, text, weight, style, code, href, kind, placement, …, link, …]`).
    fn walk(value: &Value, kinds: &mut Vec<String>, links: &mut Vec<String>) {
        match value {
            Value::Record(fields) => {
                if fields.len() > 6 && matches!(fields[2], Value::Number(_)) {
                    if let Some(kind) = fields[1].as_str() {
                        kinds.push(kind.into());
                    }
                }
                for (index, field) in fields.iter().enumerate() {
                    match field.as_str() {
                        Some(text)
                            if index != 1 && index != 4 && text.contains("attacker.example") =>
                        {
                            links.push(text.into())
                        }
                        Some(_) => {}
                        None => walk(field, kinds, links),
                    }
                }
            }
            Value::List(items) => items.iter().for_each(|item| walk(item, kinds, links)),
            _ => {}
        }
    }

    fn inert(text: &str) -> (Vec<String>, Vec<String>, String) {
        let document = chat_document(Value::str("n"), text, None, false, false, false, true);
        let (mut kinds, mut links) = (Vec::new(), Vec::new());
        walk(&document, &mut kinds, &mut links);
        (kinds, links, format!("{document:?}"))
    }

    // Images and links where a parser could disagree about where one starts: escaped and nested brackets, an image
    // inside a link, a link over two lines, reference images with their definition in a quote, a list or the cell, an
    // autolink and bare addresses, raw HTML over two lines, a code span before an image, a fence closed by an indented
    // line, and a table's cells.
    const ATTACKS: &[&str] = &[
        "![x](https://attacker.example/a.png)",
        "![x\\]y](https://attacker.example/p.png)",
        "![a [b] c](https://attacker.example/p.png)",
        "[![x](https://attacker.example/a.png)](https://attacker.example/)",
        "[click [here]](https://attacker.example/)",
        "[a [b]](https://attacker.example/)",
        "[click\nhere](https://attacker.example/)",
        "![logo]\n\n> [logo]: https://attacker.example/a.png",
        "![logo]\n\n- [logo]: https://attacker.example/a.png",
        "![logo][l]\n\n[l]: https://attacker.example/a.png",
        "<https://attacker.example/>",
        "see https://attacker.example/x and www.attacker.example",
        "<img\nsrc=\"https://attacker.example/a.png\">",
        "``a`b`` ![x](https://attacker.example/a.png)",
        "```\ncode\n    ```\n![x](https://attacker.example/p.png)\n[docs](https://attacker.example/)",
        "| a |\n| - |\n| ![x](https://attacker.example/t.png) [l](https://attacker.example/) |",
    ];

    #[test]
    fn a_notebook_cell_loads_no_image_and_links_nowhere() {
        for attack in ATTACKS {
            let (kinds, links, document) = inert(attack);
            assert!(
                links.is_empty(),
                "{attack:?} keeps a link or an image source: {links:?}\n{document}"
            );
            assert!(
                !kinds.iter().any(|kind| kind == "image" || kind == "html"),
                "{attack:?}: {kinds:?}"
            );
        }
    }

    #[test]
    fn the_same_cells_outside_a_notebook_do_link() {
        // What the pass removes is there without it, so each case above tests it.
        for attack in [ATTACKS[0], ATTACKS[1], ATTACKS[3], ATTACKS[11], ATTACKS[14]] {
            let document = chat_document(Value::str("a"), attack, None, false, false, false, false);
            let (mut kinds, mut links) = (Vec::new(), Vec::new());
            walk(&document, &mut kinds, &mut links);
            assert!(
                !links.is_empty(),
                "{attack:?} has no link even as an attachment: {kinds:?}"
            );
        }
    }

    #[test]
    fn alt_text_and_link_text_stay_as_text() {
        let (_, _, document) = inert("![a chart](https://attacker.example/a.png)\n\nSee [the docs](https://attacker.example/) and `code`.");
        assert!(
            document.contains("a chart")
                && document.contains("the docs")
                && document.contains("code"),
            "{document}"
        );
    }

    /// Each block's text as a reader sees it: its runs' text, joined.
    fn shown(text: &str) -> Vec<String> {
        fn runs(value: &Value, out: &mut String) {
            match value {
                Value::Record(fields) => {
                    if fields.len() >= 10
                        && matches!(fields[2], Value::Number(_))
                        && matches!(fields[4], Value::Bool(_))
                    {
                        out.push_str(fields[1].as_str().unwrap_or(""));
                    }
                    fields.iter().for_each(|field| runs(field, out));
                }
                Value::List(items) => items.iter().for_each(|item| runs(item, out)),
                _ => {}
            }
        }
        fn blocks(value: &Value, out: &mut Vec<String>) {
            match value {
                Value::Record(fields)
                    if fields.len() > 8
                        && fields[1].as_str().is_some()
                        && matches!(fields[2], Value::Number(_))
                        && fields[4].as_str().is_some() =>
                {
                    let mut text = String::new();
                    runs(value, &mut text);
                    out.push(text);
                }
                Value::Record(fields) => fields.iter().for_each(|field| blocks(field, out)),
                Value::List(items) => items.iter().for_each(|item| blocks(item, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        blocks(
            &chat_document(Value::str("n"), text, None, false, false, false, true),
            &mut out,
        );
        out
    }

    #[test]
    fn raw_html_keeps_its_text_where_commonmark_reads_it_inline() {
        // A line that only starts with a tag is a paragraph: its tags go and its text stays; an autolink is its address.
        assert_eq!(shown("<b>Tip:</b> Use the GPU"), ["Tip: Use the GPU"]);
        assert_eq!(
            shown("<span style=\"color:red\">Warning</span>: careful"),
            ["Warning: careful"]
        );
        assert_eq!(
            shown("Click <kbd>Ctrl</kbd>+<kbd>C</kbd>"),
            ["Click Ctrl+C"]
        );
        assert_eq!(
            shown("<https://example.com/docs>"),
            ["https://example.com/docs"]
        );
        assert_eq!(shown("<user@example.com>"), ["user@example.com"]);
        assert_eq!(
            shown("Line<br/>break and <a name=\"x\"></a>text"),
            ["Linebreak and text"]
        );
        // An attribute's Markdown is not read, nor does a gone tag join its neighbours into Markdown.
        assert_eq!(shown("a <span title=\"**x**\">t</span> b"), ["a t b"]);
        assert_eq!(shown("<a id=\"x\"></a># Title"), ["# Title"]);
        assert_eq!(
            shown("[x]<b></b>(https://example.com/)"),
            ["[x](https://example.com/)"]
        );
        assert_eq!(shown("`<b>` stays a < b"), ["<b> stays a < b"]);
    }

    #[test]
    fn escaped_and_literal_angle_brackets_stay_as_text() {
        assert_eq!(
            shown("Escaped \\<b>bold\\</b> tag"),
            ["Escaped <b>bold</b> tag"]
        );
        assert_eq!(shown("\\<div>escaped"), ["<div>escaped"]);
        // An escaped backslash leaves the tag after it a tag.
        assert_eq!(shown("x \\\\<b>y</b>"), ["x \\y"]);
        assert_eq!(shown("< 10 ms latency"), ["< 10 ms latency"]);
        assert_eq!(shown("<= 0.05 is significant"), ["<= 0.05 is significant"]);
        assert_eq!(shown("<3 love"), ["<3 love"]);
        assert_eq!(shown("1 < 2 > 0 <3"), ["1 < 2 > 0 <3"]);
    }

    #[test]
    fn a_tag_goes_before_a_url_in_it_is_read_as_a_link() {
        assert_eq!(
            shown("text <img src=\"https://example.com/a.png\"> more"),
            ["text  more"]
        );
        assert_eq!(
            shown("See <a href=\"https://example.com/docs\">the docs</a> here"),
            ["See the docs here"]
        );
        // A lone tag in an item or a quote is an HTML block there, as in a cell of a table it is inline.
        assert_eq!(shown("- <img src=\"https://example.com/l.png\">"), [""]);
        assert_eq!(shown("> <img src=\"https://example.com/q.png\">"), [""]);
        let (_, _, table) = inert("| h |\n| - |\n| <img src=\"https://example.com/c.png\"> |");
        assert!(
            !table.contains("example.com") && !table.contains("img"),
            "{table}"
        );
    }

    #[test]
    fn an_html_block_goes_whole_as_commonmark_reads_it() {
        // Kind 6, a block-level tag, runs to a blank line: Jupyter's alert box, a disclosure, a centred figure.
        assert_eq!(
            shown("<div class=\"alert alert-block alert-info\">\n<b>Tip:</b> Use the GPU\n</div>\n\nAfter"),
            ["After"]
        );
        assert_eq!(
            shown("<details>\n<summary>More</summary>\nHidden text\n</details>"),
            Vec::<String>::new()
        );
        assert_eq!(
            shown("<p align=\"center\">\n  <img src=\"logo.png\" width=\"200\">\n</p>"),
            Vec::<String>::new()
        );
        assert_eq!(shown("<center>Title</center>"), Vec::<String>::new());
        // Kinds 1 to 6 interrupt a paragraph; kind 7, a lone tag of another name, does not.
        assert_eq!(shown("Some text\n<div>block</div>"), ["Some text"]);
        assert_eq!(shown("Some text\n<!-- c -->\nMore"), ["Some text", "More"]);
        assert_eq!(shown("Para\n<b>\nbold"), ["Para  bold"]);
        assert_eq!(shown("<b>\nbold\n</b>\n\nAfter"), ["After"]);
        // Kinds 1 to 5 end on the line that closes them, and take all of it.
        assert_eq!(shown("<!-- comment --> text after"), Vec::<String>::new());
        assert_eq!(shown("<script>\nalert(1)\n</script>\nafter"), ["after"]);
        assert_eq!(shown("<!--\n\ncomment\n\n-->\nafter"), ["after"]);
        // A block ends with the item or quote it is in.
        assert_eq!(shown("- <div>\n  x\n  </div>\n- next"), ["", "next"]);
        assert_eq!(shown("- a\n\n  <div>\n  x\n- b"), ["a", "b"]);
        assert_eq!(shown("> <div>\n> x\n\nafter"), ["", "after"]);
        // A fence keeps its HTML as code.
        let (_, _, fenced) = inert("```\n<b>x</b>\n```");
        assert!(fenced.contains("<b>x</b>"), "{fenced}");
    }

    #[test]
    fn a_cell_of_unclosed_openers_reads_in_linear_time() {
        // Each of these once looked for its closer to the end of the cell, once per `<`: seconds for this size.
        let cells = [
            format!("x {}", "<?".repeat(128_000)),
            format!("x {}", "<!--".repeat(64_000)),
            format!("x {}", "<!D".repeat(85_000)),
            "<a b=\"".repeat(42_000),
            "<a b='<a c=\"".repeat(21_000),
            "`<".repeat(128_000),
        ];
        // A release build reads each in a few milliseconds; a debug build is about forty times slower.
        let limit =
            std::time::Duration::from_millis(if cfg!(debug_assertions) { 2_000 } else { 50 });
        for cell in &cells {
            let start = std::time::Instant::now();
            chat_document(Value::str("n"), cell, None, false, false, false, true);
            let took = start.elapsed();
            assert!(
                took < limit,
                "{} bytes of {:?}… took {took:?}",
                cell.len(),
                &cell[..8]
            );
        }
    }

    #[test]
    fn a_mermaid_fence_is_text_and_other_fences_keep_their_language() {
        let (kinds, _, document) = inert(
            "```mermaid\ngraph TD; A-->B\n```\n\n```Mermaid title\ngraph LR; C-->D\n```\n\n```python\nprint(1)\n```",
        );
        assert_eq!(
            kinds.iter().filter(|kind| *kind == "code").count(),
            3,
            "{document}"
        );
        assert!(
            document.contains("\"text\"")
                && document.contains("\"python\"")
                && !document.to_ascii_lowercase().contains("\"mermaid"),
            "{document}"
        );
    }
}
