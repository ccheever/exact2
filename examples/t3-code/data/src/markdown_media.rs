// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3): images inside a line of text,
// and the links of a document with no workspace folder.
//
// ChatMarkdown's `img` renderer draws every image: one alone in its block standalone (`markStandaloneImages`), one in a
// line of text or a table cell inline (ChatMarkdownImage with `standalone` false). The repository's parser keeps an
// image only when it starts a block on a line of its own (`markdown_parse` `image_of`) and flattens any other one into
// its alt text, so each other image is rewritten into a link whose destination carries IMAGE_MARK; `chat_run` makes it a
// "media" run that markdown.contract FlowRuns (CellRuns in a table) draws through the image's media chip (media-views.ts
// `markdownInlineImages`), with the href media-views.ts computes for it: IMAGE_MARK, then the image source as the URL
// transform leaves it. A link cannot hold that link, so an image inside a link's text is written into the link's text
// between LINKED_IMAGE marks and split out of the link's run after parsing; its run keeps the link's destination after
// IMAGE_LINK, which `link_labels` moves to the run's `link` (MarkdownAnchor's `<a>` around the `<img>`).

/// The mark of an inline image's run href.
const IMAGE_MARK: &str = "t3-image:";
/// The mark the rewrite puts before each inline image's alt text (a private-use character, as LINE_BREAK): the run it
/// starts is a new image, so two adjacent images with one source stay two, and an image with no alt text keeps a run.
const IMAGE_START: char = '\u{e001}';
/// A media run's `icon` when its image is the only content of its block (ChatMarkdown's `markStandaloneImages`).
const STANDALONE: &str = "standalone";
/// An image inside a link's text: LINKED_IMAGE, its alt text in hex, LINKED_SOURCE, its destination in hex, LINKED_END
/// (hex, so the parser reads no emphasis or escape in either).
const LINKED_IMAGE: char = '\u{e002}';
const LINKED_SOURCE: char = '\u{e003}';
const LINKED_END: char = '\u{e004}';
/// Between a linked image's IMAGE_MARK href and the destination of the link around it.
const IMAGE_LINK: char = '\u{e005}';

/// `![alt](src)` alone on a line: the parser's image block (`markdown_parse` `image_of`, the line trimmed at its start),
/// when that one image spans the line. `image_of` takes any line that starts with `![` and ends with `)` for one image,
/// so a badge row (`![CI](ci.png) ![npm](npm.png)`) would be one image whose source is `ci.png) ![npm](npm.png`; such a
/// line is rewritten image by image instead, as ChatMarkdown draws it (inline images: a block of several is not
/// standalone). A line whose first image the span cannot read (unbalanced parentheses) is left to the parser.
fn sole_image_line(trimmed: &str) -> bool {
    if !trimmed.starts_with("![") {
        return false;
    }
    match image_span(trimmed, 0) {
        Some((_, end)) => end + 1 == trimmed.len(),
        None => trimmed[2..].contains("](") && trimmed.ends_with(')'),
    }
}

/// Each image the parser would flatten, as `[alt](<IMAGE_MARK>src)`, or between LINKED_IMAGE marks inside a link's text.
/// Kept: fenced code, code spans, and an image line that starts a block (the parser's image block). An image line that
/// continues a paragraph is inline, as the parser's paragraph takes it in; a table line's images are its cells' own.
fn inline_image_links(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("![") {
        return std::borrow::Cow::Borrowed(text);
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let tables = table_lines(&lines);
    let mut out = String::with_capacity(text.len() + 32);
    let mut fence: Option<(char, usize)> = None;
    // The quote depth (0 outside a quote) of the paragraph the previous line continues, when it continues one.
    let mut paragraph: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start().trim_end_matches('\r');
        if let Some((marker, width)) = fence {
            let count = trimmed.chars().take_while(|c| *c == marker).count();
            if count >= width && trimmed[count..].trim().is_empty() {
                fence = None;
            }
            out.push_str(line);
            paragraph = None;
            continue;
        }
        if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let width = trimmed.chars().take_while(|c| *c == marker).count();
            if width >= 3 {
                fence = Some((marker, width));
                out.push_str(line);
                paragraph = None;
                continue;
            }
        }
        // remarkBreakAroundImageLines: an image alone on its source line inside a paragraph keeps that line (a label
        // such as `**Palette**` above a screenshot), with line breaks before and after it. Spaces around it, the quote
        // marks of a paragraph in a quote and a list item's indent are not the line's content.
        let (depth, content) = quoted(trimmed);
        let shown = content.trim_matches([' ', '\t']);
        let starts = paragraph != Some(depth);
        // The parser's image block: a sole image starting a block outside a quote and a list item's text.
        let block_image = depth == 0 && starts && !tables[index] && sole_image_line(trimmed);
        let owns_line = !block_image
            && !tables[index]
            && (sole_image_line(shown) || sole_linked_image_line(shown));
        if owns_line && !starts && !out.ends_with(&format!("{LINE_BREAK}\n")) {
            out.pop();
            out.push_str(LINE_BREAK);
            out.push('\n');
        }
        if block_image || !line.contains("![") {
            out.push_str(line);
        } else {
            image_links_in_line(line, &mut out);
        }
        if owns_line
            && lines.get(index + 1).is_some_and(|next| continues(next, depth))
            && !trimmed.ends_with(LINE_BREAK)
        {
            out.push_str(LINE_BREAK);
        }
        // A list item's text opens a paragraph that its indented lines go on with (markdown_parse's item joins them).
        paragraph = match item_text(shown) {
            Some(text) => (!text.trim().is_empty()).then_some(depth),
            None => (!block_image && continues(line, depth)).then_some(depth),
        };
    }
    std::borrow::Cow::Owned(out)
}

/// A line's quote depth (its `>` marks, each with the one space after it) and the text after them.
fn quoted(trimmed: &str) -> (usize, &str) {
    let (mut depth, mut rest) = (0, trimmed);
    while let Some(after) = rest.strip_prefix('>') {
        depth += 1;
        rest = after.strip_prefix(' ').unwrap_or(after);
    }
    (depth, rest)
}

/// A list item line's text after its marker, as markdown_parse's bullet_of reads one (`-`, `*`, `+`, or one to nine
/// digits and `.` or `)`, then a space; a rule is no item).
fn item_text(line: &str) -> Option<&str> {
    if is_rule_line(line) {
        return None;
    }
    if let Some(rest) = line.strip_prefix(['-', '*', '+']).filter(|rest| rest.starts_with(' ')) {
        return Some(rest);
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if !(1..=9).contains(&digits) {
        return None;
    }
    line[digits..].strip_prefix(['.', ')']).filter(|rest| rest.starts_with(' '))
}

/// Whether `line` is prose going on with a paragraph at quote depth `depth`.
fn continues(line: &str, depth: usize) -> bool {
    let (at, content) = quoted(line.trim_start());
    at == depth && prose_line(content)
}

/// `[![alt](src)](link)` alone on a line: a link whose only content is one image, which isImageNode counts as an image.
fn sole_linked_image_line(trimmed: &str) -> bool {
    let Some(image) = trimmed.strip_prefix('[').filter(|rest| rest.starts_with("![")) else {
        return false;
    };
    let Some((_, end)) = image_span(image, 0) else {
        return false;
    };
    image[end + 1..]
        .strip_prefix("](")
        .and_then(|rest| rest.strip_suffix(')'))
        .is_some_and(|link| !link.is_empty() && !link.contains(')'))
}

/// `![alt](inside)` at `at`: the `]` and the `)` that close it (the destination's parentheses balanced).
fn image_span(line: &str, at: usize) -> Option<(usize, usize)> {
    LineMarks::new(line).image_span(line.as_bytes(), at)
}

/// One line's brackets, parentheses and backtick runs, each found once, so each `![`, `[` and run of backticks the line
/// holds is read in constant time: a line of many unclosed `![` or `(`, or of images inside an unclosed link's text,
/// reads in linear time, where reading on from each to the line's end did not.
struct LineMarks {
    /// Where each `[` and each `]` is, in order.
    opens: Vec<usize>,
    closes: Vec<usize>,
    /// The `)` that balances the `(` at each position, every parenthesis counted, as an image's destination counts them.
    parens: std::collections::HashMap<usize, usize>,
    /// The start of each run of backticks, by its length.
    ticks: std::collections::HashMap<usize, Vec<usize>>,
    /// Whether the `[` at each position starts a link's text: the `]` that matches it, escapes and code spans aside, is
    /// followed by `(`.
    link_texts: std::collections::HashMap<usize, bool>,
}

impl LineMarks {
    fn new(line: &str) -> Self {
        let bytes = line.as_bytes();
        let mut marks = LineMarks {
            opens: Vec::new(),
            closes: Vec::new(),
            parens: std::collections::HashMap::new(),
            ticks: std::collections::HashMap::new(),
            link_texts: std::collections::HashMap::new(),
        };
        let mut unclosed = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            match bytes[at] {
                b'[' => marks.opens.push(at),
                b']' => marks.closes.push(at),
                b'(' => unclosed.push(at),
                b')' => {
                    if let Some(open) = unclosed.pop() {
                        marks.parens.insert(open, at);
                    }
                }
                b'`' => {
                    let run = bytes[at..].iter().take_while(|b| **b == b'`').count();
                    marks.ticks.entry(run).or_default().push(at);
                    at += run;
                    continue;
                }
                _ => {}
            }
            at += 1;
        }
        let mut brackets = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            match bytes[at] {
                b'\\' => at += 2,
                b'`' => at = marks.code_span_end(bytes, at),
                b'[' => {
                    brackets.push(at);
                    at += 1;
                }
                b']' => {
                    if let Some(open) = brackets.pop() {
                        marks
                            .link_texts
                            .insert(open, bytes.get(at + 1) == Some(&b'('));
                    }
                    at += 1;
                }
                _ => at += 1,
            }
        }
        marks
    }

    /// `![alt](inside)` at `at`: the first `]` after its `![`, followed by a `(` whose `)` balances it.
    fn image_span(&self, bytes: &[u8], at: usize) -> Option<(usize, usize)> {
        let close = *self
            .closes
            .get(self.closes.partition_point(|&close| close < at + 2))?;
        if bytes.get(close + 1) != Some(&b'(') {
            return None;
        }
        self.parens.get(&(close + 1)).map(|&end| (close, end))
    }

    /// The end of the code span whose backticks start at `at`: past the next run of as many backticks, or, with none,
    /// past its own run, which is then text.
    fn code_span_end(&self, bytes: &[u8], at: usize) -> usize {
        let run = bytes[at..].iter().take_while(|b| **b == b'`').count();
        self.ticks
            .get(&run)
            .and_then(|starts| starts.get(starts.partition_point(|&start| start < at + run)))
            .map_or(at + run, |&start| start + run)
    }

    /// Whether a `[` lies in `from..to`.
    fn opens_within(&self, from: usize, to: usize) -> bool {
        self.opens
            .get(self.opens.partition_point(|&open| open < from))
            .is_some_and(|&open| open < to)
    }
}

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}
fn unhex(text: &str) -> Option<String> {
    let bytes = (0..text.len())
        .step_by(2)
        .map(|at| text.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()))
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

/// A link destination as written between the parentheses (`<a b.png>` or `a.png "title"`).
fn destination(inside: &str) -> &str {
    let inside = inside.trim_start();
    match inside.strip_prefix('<') {
        Some(rest) => rest.split('>').next().unwrap_or(rest),
        None => inside.split(char::is_whitespace).next().unwrap_or(inside),
    }
}

fn image_links_in_line(line: &str, out: &mut String) {
    let bytes = line.as_bytes();
    let marks = LineMarks::new(line);
    let (mut at, mut copied) = (0usize, 0usize);
    // The `[` of each bracket still open.
    let mut opened: Vec<usize> = Vec::new();
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'`' => at = marks.code_span_end(bytes, at),
            b'[' => {
                opened.push(at);
                at += 1;
            }
            b']' => {
                opened.pop();
                at += 1;
            }
            b'!' if bytes.get(at + 1) == Some(&b'[') => {
                // An alt text holding a `[` is left as the parser reads it (the span's `]` would not be its link's).
                let Some((close, end)) = marks.image_span(bytes, at).filter(|(close, _)| !marks.opens_within(at + 2, *close))
                else {
                    at += 1;
                    continue;
                };
                let inside = &line[close + 2..end];
                if opened.last().is_some_and(|open| marks.link_texts.get(open) == Some(&true)) {
                    out.push_str(&line[copied..at]);
                    out.push(LINKED_IMAGE);
                    out.push_str(&hex(&line[at + 2..close]));
                    out.push(LINKED_SOURCE);
                    out.push_str(&hex(destination(inside)));
                    out.push(LINKED_END);
                    at = end + 1;
                    copied = at;
                    continue;
                }
                let mut lead = inside.len() - inside.trim_start().len();
                if inside[lead..].starts_with('<') {
                    lead += 1;
                }
                out.push_str(&line[copied..at]);
                out.push('[');
                out.push(IMAGE_START);
                out.push_str(&line[at + 2..close + 2]);
                out.push_str(&inside[..lead]);
                out.push_str(IMAGE_MARK);
                out.push_str(&inside[lead..]);
                out.push(')');
                at = end + 1;
                copied = at;
            }
            _ => at += 1,
        }
    }
    out.push_str(&line[copied.min(line.len())..]);
}

/// The parser's resolver with inline images: an image's source goes through `link` as a block image's does, and an
/// emptied source or a fragment loads nothing (media-views.ts `markdownImageHref`).
fn image_link(link: &dyn Fn(&str) -> String, href: &str) -> String {
    match href.strip_prefix(IMAGE_MARK) {
        Some(source) => {
            // An image's source resolves as its media chip does, not as a link.
            let resolved = media_href(source);
            let resolved = if resolved == NO_HREF || resolved.starts_with('#') {
                ""
            } else {
                resolved.as_str()
            };
            format!("{IMAGE_MARK}{resolved}")
        }
        None => link(href),
    }
}

/// AttachmentFilePreview's ChatMarkdown has no `cwd`: resolveMarkdownFileLinkTarget finds
/// no folder for a relative destination, so the anchor is a plain one, which the desktop window opens nowhere (its
/// window-open handler passes only safe external URLs): link-coloured text with no target, as an emptied anchor is. An
/// absolute path or a `file:` URL is still a chip. Only links change: an image keeps the source the URL transform gives
/// it, so its media chip matches (media-views.ts resolves it with no folder and no thread: "Image unavailable · <alt>").
fn links_without_cwd(blocks: &mut [markdown_parse::Block]) {
    let fix = |run: &mut markdown_parse::Run| {
        if let Some(target) = run.href.strip_prefix(FILE_LINK) {
            if !target.to_ascii_lowercase().starts_with("file:")
                && !target.starts_with('/')
                && !windows_drive(target)
            {
                run.href = NO_HREF.to_string();
            }
        }
    };
    for block in blocks {
        block.runs.iter_mut().for_each(fix);
        block.cells.iter_mut().flatten().for_each(fix);
    }
}

/// An inline image's alt text is one token: the run IMAGE_START marks begins it, and the runs after it with its href
/// (emphasis splits the alt text) join it; the next marked run is the next image, even one with the same source.
fn push_media_token(tokens: &mut Vec<markdown_parse::Run>, run: &markdown_parse::Run) {
    match (run.text.strip_prefix(IMAGE_START), tokens.last_mut()) {
        (Some(text), _) => tokens.push(markdown_parse::Run {
            text: text.to_string(),
            ..run.clone()
        }),
        (None, Some(last)) if last.href == run.href => last.text.push_str(&run.text),
        (None, _) => tokens.push(run.clone()),
    }
}

/// IMAGE_START out of every run but an inline image's (its media token takes it off): the text of an image the parser
/// did not read as one. A table cell's runs are its tokens already (no flow splits them), so its images join here.
fn unmark_runs(blocks: &mut [markdown_parse::Block]) {
    let unmark = |run: &mut markdown_parse::Run| {
        if run.text.contains(IMAGE_START) {
            run.text = run.text.replace(IMAGE_START, "");
        }
    };
    for block in blocks {
        block
            .runs
            .iter_mut()
            .filter(|run| !run.href.starts_with(IMAGE_MARK))
            .for_each(unmark);
        for cell in &mut block.cells {
            let mut tokens = Vec::with_capacity(cell.len());
            for run in cell.iter() {
                if run.href.starts_with(IMAGE_MARK) {
                    push_media_token(&mut tokens, run);
                } else {
                    let mut run = run.clone();
                    unmark(&mut run);
                    tokens.push(run);
                }
            }
            *cell = tokens;
        }
    }
}

/// Each image written into a link's text (LINKED_IMAGE) as its own media run, between the link's other words: its
/// source resolved as an inline image's, then IMAGE_LINK and the link's destination (none when the parser read no link).
fn linked_images(blocks: &mut [markdown_parse::Block], resolve: &dyn Fn(&str) -> String) {
    let split = |runs: &mut Vec<markdown_parse::Run>| {
        if !runs.iter().any(|run| run.text.contains(LINKED_IMAGE)) {
            return;
        }
        let mut out = Vec::with_capacity(runs.len() + 2);
        for run in runs.iter() {
            let mut rest = run.text.as_str();
            while let Some(start) = rest.find(LINKED_IMAGE) {
                let after = &rest[start + LINKED_IMAGE.len_utf8()..];
                let parts = after.find(LINKED_END).and_then(|end| {
                    let (alt, source) = after[..end].split_once(LINKED_SOURCE)?;
                    Some((unhex(alt)?, unhex(source)?, end))
                });
                let Some((alt, source, end)) = parts else { break };
                if start > 0 {
                    out.push(markdown_parse::Run { text: rest[..start].to_string(), ..run.clone() });
                }
                let image = resolve(&format!("{IMAGE_MARK}{source}"));
                let href = if run.href.is_empty() { image } else { format!("{image}{IMAGE_LINK}{}", run.href) };
                out.push(markdown_parse::Run { text: format!("{IMAGE_START}{alt}"), href, code: false, ..run.clone() });
                rest = &after[end + LINKED_END.len_utf8()..];
            }
            if !rest.is_empty() {
                out.push(markdown_parse::Run { text: rest.to_string(), ..run.clone() });
            }
        }
        *runs = out;
    };
    for block in blocks {
        split(&mut block.runs);
        block.cells.iter_mut().for_each(split);
    }
}

/// A media run's href and the link around it (`link_labels`): IMAGE_LINK parts them.
fn media_link(href: &str) -> (&str, &str) {
    match href.starts_with(IMAGE_MARK).then(|| href.split_once(IMAGE_LINK)).flatten() {
        Some((image, link)) => (image, link),
        None => (href, ""),
    }
}

/// An inline image's Markdown as markdownImageCopy writes it (`![alt](src)`, the source as authored): a table's Copy
/// as Markdown.
fn image_markdown(alt: &str, href: &str) -> String {
    let source = href.strip_prefix(IMAGE_MARK).unwrap_or(href);
    let source = source.strip_prefix(FILE_LINK).unwrap_or(source);
    let alt = alt.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]");
    format!("![{alt}]({source})")
}

/// ChatMarkdown's `markStandaloneImages`: an image that is the only content of a paragraph, a list item or a quote
/// (whitespace aside; not a task item, whose checkbox shares it) is a figure, ChatMarkdownImage `standalone`, so its
/// media run's `icon` is STANDALONE and FlowRuns gives it the 16:9 slot an image line has. An image in a heading, or
/// beside text, a line break or another image, stays inline.
fn standalone_media(
    block: &markdown_parse::Block,
    tokens: &[markdown_parse::Run],
    task: &str,
    runs: &mut Value,
) {
    use markdown_parse::Kind;
    let media = |token: &&markdown_parse::Run| token.href.starts_with(IMAGE_MARK);
    let sole = matches!(block.kind, Kind::Paragraph | Kind::Item | Kind::Quote)
        && task.is_empty()
        && tokens.iter().filter(media).count() == 1
        && tokens.iter().all(|token| {
            media(&token)
                || (token.href.is_empty() && !token.code && token.text.trim().is_empty())
        });
    let Value::List(list) = runs else { return };
    if !sole {
        return;
    }
    *runs = Value::list(
        list.iter()
            .map(|run| match run {
                Value::Record(fields) if fields.get(6).and_then(Value::as_str) == Some("media") => {
                    let mut fields = fields.to_vec();
                    fields[7] = Value::str(STANDALONE);
                    Value::record(fields)
                }
                other => other.clone(),
            })
            .collect(),
    );
}

#[cfg(test)]
mod media_tests {
    use super::*;

    /// Each run of a document: its block's kind, its text, `href|kind`, and its `icon`.
    fn runs_of(document: &Value) -> Vec<(String, String, String, String)> {
        let Value::Record(fields) = document else {
            unreachable!()
        };
        let Value::List(blocks) = &fields[1] else {
            unreachable!()
        };
        let text = |value: &Value| value.as_str().unwrap_or("").to_string();
        blocks
            .iter()
            .flat_map(|block| {
                let Value::Record(block) = block else {
                    unreachable!()
                };
                let Value::List(runs) = &block[7] else {
                    return Vec::new();
                };
                runs.iter()
                    .map(|run| {
                        let Value::Record(run) = run else {
                            unreachable!()
                        };
                        let href_kind = format!("{}|{}", text(&run[5]), text(&run[6]));
                        (text(&block[1]), text(&run[1]), href_kind, text(&run[7]))
                    })
                    .collect()
            })
            .collect()
    }
    /// The media runs of `text` as Files renders it: block kind, alt text, href, icon.
    fn media_of(text: &str) -> Vec<(String, String, String, String)> {
        runs_of(&document_with_tasks(Value::str("m"), text, None, true))
            .into_iter()
            .filter(|run| run.2.ends_with("|media"))
            .map(|(kind, alt, href, icon)| (kind, alt, href.trim_end_matches("|media").to_string(), icon))
            .collect()
    }
    fn run(kind: &str, alt: &str, href: &str, icon: &str) -> (String, String, String, String) {
        (kind.into(), alt.into(), href.into(), icon.into())
    }

    #[test]
    fn a_line_of_unclosed_images_reads_in_linear_time() {
        // Each of these once read on to the line's end from each `![`, `(`, bracket, run of backticks or `<`, or back
        // over every open run of tildes: from half a second to two seconds for this size in a release build.
        let ticks: String = (1..=1_000).map(|k| format!("{}x ", "`".repeat(k))).collect();
        let lines = [
            format!("x {}", "![a".repeat(85_000)),
            "![a](".repeat(51_000),
            format!("[{}", "[![a](b)".repeat(32_000)),
            format!("![a](b) {ticks}"),
            format!("~a~ {}", "](".repeat(128_000)),
            format!("~a~ {}", "<".repeat(255_000)),
            format!("{}{}", " ~x".repeat(40_000), " x~~".repeat(40_000)),
        ];
        // A release build renders each in under 100 ms (the link's 64,000 images the most); a debug build is about ten
        // times slower.
        let limit = std::time::Duration::from_millis(if cfg!(debug_assertions) { 3_000 } else { 150 });
        for line in &lines {
            let start = std::time::Instant::now();
            document_with_skills(Value::str("m"), line, None);
            let took = start.elapsed();
            assert!(took < limit, "{} bytes of {:?}… took {took:?}", line.len(), &line[..8]);
        }
    }

    #[test]
    fn images_inside_a_line_are_rewritten_and_block_images_kept() {
        let text = "Beside: ![Shot](shot.png) and ![Logo **x**](<../a b.png> \"t\").\n\n![Alone](shot.png)\n\nText\n![Joined](j.png)\n\n`![code](c.png)` \\![esc](e.png) [![linked](l.png)](https://a.com) ![a[b](x.png)\n\n```\n![fenced](f.png)\n```\n\n| a | b |\n| - | - |\n| ![cell](c.png) | x |\n\n![CI](ci.png) ![npm](npm.png)\n\n![a](x.png) and (b)";
        let rewritten = inline_image_links(text);
        assert!(rewritten.contains(
            "Beside: [\u{e001}Shot](t3-image:shot.png) and [\u{e001}Logo **x**](<t3-image:../a b.png> \"t\")."
        ));
        assert!(
            rewritten.contains("\n![Alone](shot.png)\n"),
            "a block image stays"
        );
        assert!(
            rewritten.contains("Text\u{e000}\n[\u{e001}Joined](t3-image:j.png)"),
            "a line that continues a paragraph is inline, on a line of its own: {rewritten}"
        );
        assert!(
            rewritten.contains(&format!(
                "`![code](c.png)` \\![esc](e.png) [\u{e002}{}\u{e003}{}\u{e004}](https://a.com) ![a[b](x.png)",
                hex("linked"),
                hex("l.png")
            )),
            "code, an escape and an alt text with a `[` are left; an image in a link's text is marked: {rewritten}"
        );
        assert!(rewritten.contains("![fenced](f.png)") && rewritten.contains("| [\u{e001}cell](t3-image:c.png) |"));
        assert!(
            rewritten.contains("\n[\u{e001}CI](t3-image:ci.png) [\u{e001}npm](t3-image:npm.png)\n\n[\u{e001}a](t3-image:x.png) and (b)"),
            "a line of several images, or an image and text, is not one image block: {rewritten}"
        );
        assert_eq!(
            rewritten.lines().count(),
            text.lines().count(),
            "line structure kept for task offsets"
        );
    }

    // remarkBreakAroundImageLines (#22787): an image alone on its source line sits under its label.
    #[test]
    fn an_image_on_its_own_line_keeps_that_line() {
        let lines = |text: &str| {
            let Value::Record(fields) = document_with_tasks(Value::str("m"), text, None, true) else {
                unreachable!()
            };
            let Value::List(blocks) = &fields[1] else { unreachable!() };
            blocks
                .iter()
                .map(|block| {
                    let Value::Record(block) = block else { unreachable!() };
                    let Value::List(runs) = &block[7] else { return String::new() };
                    runs.iter()
                        .map(|run| match run {
                            Value::Record(run) if run[6].as_str() == Some("media") => "<img>".to_string(),
                            Value::Record(run) => run[1].as_str().unwrap_or("").to_string(),
                            _ => String::new(),
                        })
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            lines("**Palette**\n![Palette](/tmp/palette.png)\n**Hero** (dark)\n![Hero](/tmp/hero.png)"),
            ["Palette", "<img>", "Hero (dark)", "<img>"]
        );
        assert_eq!(lines("Shot\n[![CI](/ci.svg)](/ci)\nafter").len(), 3, "a linked image alone on its line too");
        assert_eq!(lines("Built [![CI](/ci.svg)](/ci)\nand wrapped").len(), 1, "an inline badge still wraps");
        assert_eq!(lines("Hard-wrapped\nprose"), ["Hard-wrapped prose"]);
        // Spaces after the image and a quote's marks are not the line's content (remarkBreakAroundImageLines reads
        // the paragraph's own text).
        let trimmed = |text: &str| lines(text).iter().map(|line| line.trim().to_string()).collect::<Vec<_>>();
        assert_eq!(trimmed("**Palette**\n![P](p.png) \nnext"), ["Palette", "<img>", "next"]);
        assert_eq!(trimmed("> **Palette**\n> ![P](p.png)"), ["Palette", "<img>"]);
        assert_eq!(trimmed("> **Palette**\n> ![P](p.png)\n> next"), ["Palette", "<img>", "next"]);
        // A list item's indented line keeps its picture (the item would take it in as alt text), and an image that
        // opens a quote's paragraph keeps its line too.
        assert_eq!(trimmed("1. Open settings\n   ![Settings](s.png)"), ["Open settings", "<img>"]);
        // Its picture's line is the item's own: no second marker.
        let Value::Record(fields) = document_with_tasks(Value::str("m"), "1. Open settings\n   ![Settings](s.png)", None, true) else {
            unreachable!()
        };
        let Value::List(blocks) = &fields[1] else { unreachable!() };
        let markers: Vec<&str> = blocks.iter().map(|block| match block { Value::Record(block) => block[3].as_str().unwrap_or(""), _ => "" }).collect();
        assert_eq!(markers, ["1.", ""]);
        assert_eq!(trimmed("> ![P](p.png)\n> caption"), ["<img>", "caption"]);
    }

    #[test]
    fn an_inline_image_is_one_media_run_with_its_resolved_source() {
        assert_eq!(
            media_of("See ![Logo **x**](<../a b.png>) and ![Web](https://a.com/w.png) or ![Bad](javascript:x.png)."),
            vec![
                run("paragraph", "Logo x", "t3-image:t3-file:../a b.png", ""),
                run("paragraph", "Web", "t3-image:https://a.com/w.png", ""),
                run("paragraph", "Bad", "t3-image:", ""),
            ]
        );
        // Two adjacent images with one source are two (`![a](x.png)![a](x.png)`), and one with no alt text
        // still has its run.
        assert_eq!(
            media_of("Twice ![a](x.png)![a](x.png) and ![](y.png)."),
            vec![
                run("paragraph", "a", "t3-image:t3-file:x.png", ""),
                run("paragraph", "a", "t3-image:t3-file:x.png", ""),
                run("paragraph", "", "t3-image:t3-file:y.png", ""),
            ]
        );
        // A badge row is two inline images, not one image whose source is `ci.png) ![npm](npm.png`.
        let badges = document_with_tasks(Value::str("m"), "![CI](ci.png) ![npm](npm.png)", None, true);
        assert_eq!(
            runs_of(&badges).into_iter().map(|run| (run.1, run.2)).collect::<Vec<_>>(),
            vec![
                ("CI".into(), "t3-image:t3-file:ci.png|media".into()),
                (" ".into(), "|".into()),
                ("npm".into(), "t3-image:t3-file:npm.png|media".into()),
            ]
        );
        // No run keeps the rewrite's mark: an alt text with a `[` is the parser's text.
        let marks = runs_of(&document_with_tasks(Value::str("m"), "See ![a[b](x.png) here", None, true));
        assert!(marks.iter().all(|run| !run.1.contains(IMAGE_START)), "{marks:?}");
    }

    /// The cells of a document's first table: each cell's runs as (text, href|kind, link).
    fn cells_of(document: &Value) -> Vec<Vec<(String, String, String)>> {
        let Value::Record(fields) = document else {
            unreachable!()
        };
        let Value::List(blocks) = &fields[1] else {
            unreachable!()
        };
        let text = |value: &Value| value.as_str().unwrap_or("").to_string();
        let table = blocks
            .iter()
            .find_map(|block| match block {
                Value::Record(block) if block[1].as_str() == Some("table") => Some(block.clone()),
                _ => None,
            })
            .expect("a table block");
        let Value::List(rows) = &table[11] else {
            unreachable!()
        };
        rows.iter()
            .flat_map(|row| {
                let Value::Record(row) = row else { unreachable!() };
                let Value::List(cells) = &row[2] else { unreachable!() };
                cells
                    .iter()
                    .map(|cell| {
                        let Value::Record(cell) = cell else { unreachable!() };
                        let Value::List(runs) = &cell[1] else { unreachable!() };
                        runs.iter()
                            .map(|run| {
                                let Value::Record(run) = run else { unreachable!() };
                                (text(&run[1]), format!("{}|{}", text(&run[5]), text(&run[6])), text(&run[9]))
                            })
                            .collect()
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[test]
    fn an_image_in_a_table_cell_is_a_media_run_and_copies_as_markdown() {
        let text = "| Shot | Note |\n| - | - |\n| ![Sh**o**t](shot.png) | see ![Logo](https://a.com/l.png) |";
        let document = document_with_tasks(Value::str("m"), text, None, true);
        let cells = cells_of(&document);
        let media = |text: &str, href: &str| (text.to_string(), format!("{href}|media"), String::new());
        assert_eq!(cells[2], vec![media("Shot", "t3-image:t3-file:shot.png")]);
        assert_eq!(
            cells[3],
            vec![("see ".to_string(), "|".to_string(), String::new()), media("Logo", "t3-image:https://a.com/l.png")]
        );
        let Value::Record(fields) = &document else { unreachable!() };
        let Value::List(blocks) = &fields[1] else { unreachable!() };
        let Value::Record(table) = &blocks[0] else { unreachable!() };
        // markdownImageCopy for the image; ToCsv reads the cell's text, where an image has none.
        assert_eq!(
            table[13].as_str(),
            Some("| Shot | Note |\n| --- | --- |\n| ![Shot](shot.png) | see ![Logo](https://a.com/l.png) |")
        );
        assert_eq!(table[14].as_str(), Some("Shot,Note\n,see"));
    }

    #[test]
    fn an_image_inside_a_link_is_a_media_run_that_names_its_link() {
        let document = document_with_tasks(
            Value::str("m"),
            "Badge [![CI](ci.png)](https://ci.example/run) and [go ![b](<b c.png>) now](https://a.com).",
            None,
            true,
        );
        let runs: Vec<_> = runs_of(&document).into_iter().map(|run| (run.1, run.2)).collect();
        assert!(runs.contains(&("CI".to_string(), "t3-image:t3-file:ci.png|media".to_string())), "{runs:?}");
        assert!(runs.contains(&("b".to_string(), "t3-image:t3-file:b c.png|media".to_string())), "{runs:?}");
        assert!(runs.contains(&("go ".to_string(), "https://a.com|link-start".to_string())), "{runs:?}");
        assert!(runs.contains(&("now".to_string(), "https://a.com|link".to_string())), "{runs:?}");
        let Value::Record(fields) = &document else { unreachable!() };
        let Value::List(blocks) = &fields[1] else { unreachable!() };
        let Value::Record(block) = &blocks[0] else { unreachable!() };
        let Value::List(list) = &block[7] else { unreachable!() };
        let links: Vec<(String, String)> = list
            .iter()
            .filter_map(|run| match run {
                Value::Record(run) if run[6].as_str() == Some("media") => {
                    Some((run[1].as_str()?.to_string(), run[9].as_str()?.to_string()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            links,
            vec![("CI".into(), "https://ci.example/run".into()), ("b".into(), "https://a.com".into())]
        );
        // A badge alone in its paragraph is a figure, as markStandaloneImages looks through the `<a>`.
        let alone = media_of("[![CI](ci.png)](https://ci.example)");
        assert_eq!(alone, vec![run("paragraph", "CI", "t3-image:t3-file:ci.png", STANDALONE)]);
    }

    #[test]
    fn an_image_alone_in_a_paragraph_item_or_quote_is_standalone() {
        let text = "- ![Item](a.png)\n\n> ![Quoted](q.png)\n\n**![Bold](b.png)**\n\n- [ ] ![Task](t.png)\n\n# Title ![Heading](h.png)\n\nText ![Beside](s.png)\n\n- ![One](o.png) ![Two](w.png)";
        assert_eq!(
            media_of(text),
            vec![
                run("item", "Item", "t3-image:t3-file:a.png", STANDALONE),
                run("quote", "Quoted", "t3-image:t3-file:q.png", STANDALONE),
                run("paragraph", "Bold", "t3-image:t3-file:b.png", STANDALONE),
                run("item", "Task", "t3-image:t3-file:t.png", ""),
                run("heading", "Heading", "t3-image:t3-file:h.png", ""),
                run("paragraph", "Beside", "t3-image:t3-file:s.png", ""),
                run("item", "One", "t3-image:t3-file:o.png", ""),
                run("item", "Two", "t3-image:t3-file:w.png", ""),
            ]
        );
        // A sent message's lines: an image line between two lines of text shares their paragraph (remark-breaks).
        let mut markdown = Markdown;
        let message = Value::list(vec![Value::record(vec![
            Value::str("u"),
            Value::str("user"),
            Value::str(""),
            Value::str("Look:\n![Shot](s.png)\nmore"),
        ])]);
        let Value::Record(fields) = markdown.query("renderMarkdown", &[message]).unwrap() else {
            unreachable!()
        };
        let Value::List(documents) = &fields[0] else {
            unreachable!()
        };
        let shot: Vec<_> = runs_of(&documents[0]).into_iter().filter(|run| run.1 == "Shot").collect();
        assert_eq!(shot, vec![run("paragraph", "Shot", "t3-image:t3-file:s.png|media", "")]);
    }

    #[test]
    fn an_attachment_keeps_its_image_sources_and_empties_relative_links() {
        let mut markdown = Markdown;
        let source = Value::list(vec![Value::record(vec![
            Value::str("attachment:a"),
            Value::str("attachment"),
            Value::str(""),
            Value::str("[guide](docs/guide.md) [abs](/repo/a.ts)\n\n![Relative](docs/shot.png)\n\n![Absolute](</repo/a.png>)\n\nInline ![In](docs/in.png)"),
        ])]);
        let result = markdown.query("renderFileMarkdown", &[source]).unwrap();
        let Value::Record(fields) = &result else {
            unreachable!()
        };
        let Value::List(documents) = &fields[0] else {
            unreachable!()
        };
        let Value::Record(document) = &documents[0] else {
            unreachable!()
        };
        let Value::List(blocks) = &document[1] else {
            unreachable!()
        };
        let images: Vec<&str> = blocks
            .iter()
            .filter_map(|block| match block {
                Value::Record(block) if block[1].as_str() == Some("image") => block[5].as_str(),
                _ => None,
            })
            .collect();
        assert_eq!(
            images,
            vec!["t3-file:docs/shot.png", "t3-file:</repo/a.png>"]
        );
        let runs = runs_of(&documents[0]);
        assert!(
            runs.iter().any(|run| run.1 == "guide" && run.2 == "|link"),
            "a relative link is targetless: {runs:?}"
        );
        assert!(runs
            .iter()
            .any(|run| run.1 == "a.ts" && run.2 == "t3-file:/repo/a.ts|file"));
        assert!(runs
            .iter()
            .any(|run| run.1 == "In" && run.2 == "t3-image:t3-file:docs/in.png|media"));
    }
}
