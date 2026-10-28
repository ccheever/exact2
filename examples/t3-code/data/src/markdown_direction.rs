// T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3): apps/web/src/textDirection.ts and
// components/chat/markdownTextDirection.ts. Each top-level element takes the direction of its
// own words, so an Arabic or Hebrew reply reads right to left beside English ones; a list's or
// quote's lines follow the whole list or quote, and code keeps left to right.

/// Hebrew through Arabic Extended-A, their presentation forms, and the supplementary
/// right-to-left blocks (textDirection's RTL_LETTER).
fn rtl_letter(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// textDirection: words are counted so mixed prose follows its main language (`dir="auto"` reads only the first
/// strong letter); a tie goes to the first word, and text without letters has none ("").
fn text_direction(text: &str) -> &'static str {
    let (mut rtl, mut ltr, mut first) = (0usize, 0usize, "");
    for word in text.split_whitespace() {
        let Some(letter) = word.chars().find(|c| {
            unicode_general_category::get_general_category(*c)
                .abbreviation()
                .starts_with('L')
        }) else {
            continue;
        };
        let direction = if rtl_letter(letter) { "rtl" } else { "ltr" };
        if first.is_empty() {
            first = direction;
        }
        if direction == "rtl" {
            rtl += 1;
        } else {
            ltr += 1;
        }
    }
    if rtl == ltr {
        first
    } else if rtl > ltr {
        "rtl"
    } else {
        "ltr"
    }
}

/// proseText: a block's words, each run, cell or item apart. Inline code is a space, and an image has no words (its
/// alt text is an attribute), a context image (`image_refs`, its references' destinations) too. A raw HTML line counts
/// its text, not its tags, where the document parses raw HTML (`raw_html`); a user message shows it as literal text
/// (`parseRawHtml={false}`), whose raw node has no words.
fn prose_text(
    block: &markdown_parse::Block,
    image_refs: &[String],
    raw_html: bool,
    out: &mut String,
) {
    for run in block.runs.iter().chain(block.cells.iter().flatten()) {
        out.push(' ');
        if !run.code && !run.href.starts_with(IMAGE_MARK) && !image_refs.contains(&run.href) {
            out.push_str(&run.text);
        }
    }
    if raw_html && matches!(block.kind, markdown_parse::Kind::Html) {
        let mut tag = false;
        out.push(' ');
        for c in block.text.chars() {
            match c {
                '<' => tag = true,
                '>' if tag => {
                    tag = false;
                    out.push(' ');
                }
                _ if !tag => out.push(c),
                _ => {}
            }
        }
    }
    out.push(' ');
}

/// rehypeTextDirection over the parser's flat blocks: one direction per top-level element (`top_level_elements`),
/// given to each of its blocks; a code block keeps none (MarkdownCodeBlock's `dir="ltr"`). One direction per block.
fn block_directions(
    blocks: &[markdown_parse::Block],
    text: &str,
    image_refs: &[String],
    raw_html: bool,
) -> Vec<&'static str> {
    let lines: Vec<&str> = text.lines().collect();
    let mut groups = Vec::new();
    let mut next = 0;
    // The parser reads an element's lines alone as it reads them in place, so it makes the same blocks of them.
    for (from, to) in top_level_elements(&lines) {
        let count = markdown_parse::parse(&lines[from..to].join("\n"), &|href| href.to_string())
            .blocks
            .len();
        groups.push(next..next + count);
        next += count;
    }
    // Should the two readings ever disagree, each block reads on its own.
    if next != blocks.len() {
        groups = (0..blocks.len()).map(|at| at..at + 1).collect();
    }
    let mut directions = vec![""; blocks.len()];
    for group in groups {
        let mut words = String::new();
        blocks[group.clone()]
            .iter()
            .for_each(|block| prose_text(block, image_refs, raw_html, &mut words));
        let direction = text_direction(&words);
        for at in group {
            if !matches!(blocks[at].kind, markdown_parse::Kind::Code) {
                directions[at] = direction;
            }
        }
    }
    directions
}

/// The line ranges of the source's top-level elements (CommonMark's: a paragraph, heading, rule, code block, table,
/// raw HTML line, or a whole list or blockquote, nested content and lazy lines included), each starting where
/// markdown_parse starts a block.
fn top_level_elements(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut elements = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let trimmed = lines[at].trim_start();
        let indent = lines[at].len() - trimmed.len();
        if trimmed.is_empty() {
            at += 1;
            continue;
        }
        let start = at;
        at = if let Some(fence) = fence_mark(trimmed) {
            fence_end(lines, at, fence)
        } else if is_rule_line(trimmed) || heading_line(trimmed) {
            at + 1
        } else if let Some(end) = table_end(lines, at) {
            end
        } else if trimmed.starts_with('>') {
            quote_end(lines, at)
        } else if let Some(marker) = list_marker(trimmed) {
            list_end(lines, at, indent, marker)
        } else if trimmed.starts_with('<') && trimmed.len() > 2 {
            // markdown_parse's raw HTML line, one element each (after rehype-raw, each top-level element has its own).
            at + 1
        } else {
            paragraph_end(lines, at)
        };
        elements.push((start, at));
    }
    elements
}

/// markdown_parse's fence_of.
fn fence_mark(trimmed: &str) -> Option<(char, usize)> {
    ['`', '~'].into_iter().find_map(|marker| {
        let count = trimmed.chars().take_while(|&c| c == marker).count();
        (count >= 3).then_some((marker, count))
    })
}

/// markdown_parse's code_block: past the closing fence, or the end of the text.
fn fence_end(lines: &[&str], at: usize, (marker, width): (char, usize)) -> usize {
    (at + 1..lines.len())
        .find(|&i| {
            let trimmed = lines[i].trim_start();
            trimmed.chars().take_while(|&c| c == marker).count() >= width
                && trimmed.trim_end_matches(marker).is_empty()
        })
        .map_or(lines.len(), |close| close + 1)
}

/// markdown_parse's heading_of: one to six `#`, then a space or nothing.
fn heading_line(trimmed: &str) -> bool {
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes)
        && (trimmed[hashes..].is_empty() || trimmed[hashes..].starts_with(' '))
}

/// markdown_parse's table_at: a line with a pipe over a `| --- |` rule, and the piped lines after them.
fn table_end(lines: &[&str], at: usize) -> Option<usize> {
    let rule = lines.get(at + 1)?.trim();
    if !lines[at].contains('|')
        || !rule.contains('|')
        || !rule.contains('-')
        || !rule
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
    {
        return None;
    }
    let mut end = at + 2;
    while end < lines.len() && lines[end].contains('|') && !lines[end].trim().is_empty() {
        end += 1;
    }
    Some(end)
}

/// A list item's marker as markdown_parse's bullet_of reads it (a rule is no item): its kind (`-`, `*`, `+`, or an
/// ordered list's `.` or `)`; another kind starts another list) and the columns up to the item's text.
fn list_marker(trimmed: &str) -> Option<(char, usize)> {
    if is_rule_line(trimmed) {
        return None;
    }
    // CommonMark's content column: one to four spaces after the marker belong to it; more (or none before the line
    // ends) leave one.
    let gap = |rest: &str| match rest.chars().take_while(|&c| c == ' ').count() {
        spaces if spaces > 4 || spaces == rest.len() => 1,
        spaces => spaces,
    };
    for marker in ['-', '*', '+'] {
        if let Some(rest) = trimmed
            .strip_prefix(marker)
            .filter(|rest| rest.starts_with(' '))
        {
            return Some((marker, 1 + gap(rest)));
        }
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if !(1..=9).contains(&digits) {
        return None;
    }
    ['.', ')'].into_iter().find_map(|close| {
        let rest = trimmed[digits..].strip_prefix(close)?;
        rest.starts_with(' ')
            .then(|| (close, digits + 1 + gap(rest)))
    })
}

/// A line no paragraph continues into: a heading, fence, rule, quote, list item or table.
fn block_start(lines: &[&str], at: usize) -> bool {
    let trimmed = lines[at].trim();
    heading_line(trimmed)
        || fence_mark(trimmed).is_some()
        || is_rule_line(trimmed)
        || trimmed.starts_with('>')
        || list_marker(trimmed).is_some()
        || table_end(lines, at).is_some()
}

/// markdown_parse's paragraph: up to a blank line or a block's start (an image line starts a paragraph that goes on,
/// as CommonMark's does).
fn paragraph_end(lines: &[&str], at: usize) -> usize {
    (at + 1..lines.len())
        .find(|&i| lines[i].trim().is_empty() || block_start(lines, i))
        .unwrap_or(lines.len())
}

/// A blockquote: its `>` lines and the lazy lines that continue its paragraph.
fn quote_end(lines: &[&str], at: usize) -> usize {
    let open = |line: &str| !line.trim_start().trim_start_matches(['>', ' ']).is_empty();
    let mut paragraph = open(lines[at]);
    let mut i = at + 1;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with('>') {
            paragraph = open(lines[i]);
        } else if trimmed.is_empty() || !paragraph || block_start(lines, i) {
            break;
        }
        i += 1;
    }
    i
}

/// A list from its first item (`indent`, its marker): its items of that kind, everything indented to an item's text
/// (nested lists, paragraphs, fenced code), and lazy lines, across blank lines that do not end it.
fn list_end(lines: &[&str], at: usize, indent: usize, (kind, width): (char, usize)) -> usize {
    let mut content = indent + width;
    let mut blank = false;
    let mut i = at + 1;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        let here = lines[i].len() - trimmed.len();
        if trimmed.is_empty() {
            blank = true;
            i += 1;
            continue;
        }
        if let Some((next, next_width)) = list_marker(trimmed) {
            if here < content {
                if next != kind {
                    break;
                }
                content = here + next_width;
            }
        } else if here >= content {
            if let Some(fence) = fence_mark(trimmed) {
                i = fence_end(lines, i, fence);
                blank = false;
                continue;
            }
        } else if blank || block_start(lines, i) {
            break;
        }
        blank = false;
        i += 1;
    }
    while i > at + 1 && lines[i - 1].trim().is_empty() {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod direction_tests {
    use super::*;

    fn directions(text: &str) -> (Vec<&'static str>, Vec<&'static str>) {
        let blocks = markdown_parse::parse(text, &|href| href.to_string()).blocks;
        let kinds = blocks.iter().map(|block| block.kind.name()).collect();
        (kinds, block_directions(&blocks, text, &[], true))
    }

    #[test]
    fn words_decide_and_a_tie_goes_to_the_first() {
        for (text, direction) in [
            ("اكتب دالة تجمع الأرقام", "rtl"),
            ("תקן את הבאג", "rtl"),
            ("fix the failing test", "ltr"),
            ("StrictMode: يعيد الـ render مرتين في التطوير", "rtl"),
            ("translate the label مرحبا for the login screen", "ltr"),
            ("React مكتبة", "ltr"),
            ("مكتبة React", "rtl"),
            ("عدّل السطر 42، ثم شغّل الاختبارات (3)", "rtl"),
            ("42 — 7 ✓", ""),
        ] {
            assert_eq!(text_direction(text), direction, "{text}");
        }
    }

    #[test]
    fn a_list_follows_its_words_and_inline_code_is_not_counted() {
        let (kinds, directions) = directions("- مرحبا\n- hello world\n\nاستخدم `useEffect useLayoutEffect React` هنا\n\n```\n// احسب المجموع هنا\n```");
        assert_eq!(kinds, ["item", "item", "paragraph", "code"]);
        assert_eq!(directions, ["ltr", "ltr", "rtl", ""]);
    }

    #[test]
    fn each_top_level_element_reads_alone() {
        for (text, expected) in [
            // Two quotes apart, two lists of different markers, two tables.
            ("> مرحبا\n\n> hello world", vec!["rtl", "ltr"]),
            ("- مرحبا\n\n* hello world", vec!["rtl", "ltr"]),
            ("1. مرحبا\n- hello world", vec!["rtl", "ltr"]),
            (
                "| ا | ب |\n| --- | --- |\n| ج | د |\n\n| a | b |\n| --- | --- |\n| c | d |",
                vec!["rtl", "rtl", "ltr", "ltr"],
            ),
            // One quote, its lazy line too.
            ("> مرحبا يا\nhello", vec!["rtl", "rtl"]),
        ] {
            assert_eq!(directions(text).1, expected, "{text}");
        }
    }

    #[test]
    fn a_list_keeps_one_direction_across_its_code_and_blank_lines() {
        let (kinds, directions) = directions(
            "- مرحبا يا عالم\n\n  ```\n  code here\n  ```\n- hello\n\n  نص آخر هنا\n\nhello world",
        );
        assert_eq!(kinds, ["item", "code", "item", "paragraph", "paragraph"]);
        assert_eq!(directions, ["rtl", "", "rtl", "rtl", "ltr"]);
    }

    #[test]
    fn a_task_marker_and_an_image_have_no_words() {
        let dirs = |document: Value| {
            let Value::Record(fields) = document else {
                unreachable!()
            };
            let Value::List(blocks) = &fields[1] else {
                unreachable!()
            };
            blocks
                .iter()
                .map(|block| match block {
                    Value::Record(fields) => fields
                        .last()
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
        };
        let blocks = |text: &str| dirs(document_with_tasks(Value::str("m"), text, None, true));
        assert_eq!(blocks("- [x] تم"), ["rtl"]);
        assert_eq!(
            blocks("مرحبا ![a long English description](x.png)"),
            ["rtl"]
        );
        // A context image is an `img` too, though the parse reads its reference as a link.
        assert_eq!(
            blocks("مرحبا ![a long English description](t3-context://v1/image/image_1)"),
            ["rtl"]
        );
        // Raw HTML: each top-level element reads alone; a user message shows it literally, with no direction.
        assert_eq!(blocks("<p>مرحبا</p>\n<p>hello world</p>"), ["rtl", "ltr"]);
        assert_eq!(
            dirs(user_document(Value::str("u"), "<p>مرحبا</p>", None)),
            [""]
        );
    }
}
