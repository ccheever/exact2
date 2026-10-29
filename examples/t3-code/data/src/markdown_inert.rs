// T3 Code 7c3b3b3605ef (MIT; see LICENSE-T3): a saved notebook's Markdown cell, made inert
// (NotebookPreview.tsx `inertNotebookMarkdown`).

/// NotebookPreview's `inertNotebookMarkdown` (upstream `NotebookPreview.tsx`), on the blocks this parser made, so
/// whatever it reads as an image or a link is caught: a saved notebook loads no URL and turns no authored link into
/// an app action. An image is its alt text and a link its text; inline code is plain text; raw HTML goes, its text
/// staying where CommonMark reads it as inline (`inline_html_text`); a Mermaid fence is text.
fn inert_blocks(blocks: &mut Vec<markdown_parse::Block>) {
    // The parser takes any line starting with `<` for an HTML block. CommonMark reads such a line as one only when it
    // opens one (`html_block_start`), and that block goes whole, as the reference drops the `html` node; any other is
    // a paragraph, whose tags go and whose text stays.
    let mut kept = Vec::with_capacity(blocks.len());
    for block in blocks.drain(..) {
        if !matches!(block.kind, markdown_parse::Kind::Html) {
            kept.push(block);
            continue;
        }
        if html_block_start(&block.text) {
            continue;
        }
        let text = inline_html_text(&block.text);
        let mut doc = markdown_parse::parse(&text, &|href| image_link(&link_href, href));
        linked_images(&mut doc.blocks, &|href| image_link(&link_href, href));
        kept.extend(
            doc.blocks
                .into_iter()
                .filter(|block| !matches!(block.kind, markdown_parse::Kind::Html)),
        );
    }
    *blocks = kept;
    for block in blocks.iter_mut() {
        if matches!(block.kind, markdown_parse::Kind::Image) {
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
            if !run.code && run.text.contains('<') {
                run.text = inline_html_text(&run.text);
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

/// Whether a line opens a CommonMark HTML block (§4.6, start conditions 1 to 7).
fn html_block_start(line: &str) -> bool {
    let line = line.trim_start();
    let lower = line.to_ascii_lowercase();
    let after_name = |rest: &str| {
        rest.is_empty() || rest.starts_with(['>', ' ', '\t']) || rest.starts_with("/>")
    };
    if ["<script", "<pre", "<style", "<textarea"]
        .iter()
        .any(|open| {
            lower
                .strip_prefix(open)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['>', ' ', '\t']))
        })
    {
        return true;
    }
    if lower.starts_with("<!--") || lower.starts_with("<?") || lower.starts_with("<![cdata[") {
        return true;
    }
    if lower
        .strip_prefix("<!")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic()))
    {
        return true;
    }
    let name = lower
        .strip_prefix("</")
        .or_else(|| lower.strip_prefix('<'))
        .unwrap_or("");
    let end = name
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(name.len());
    if HTML_BLOCK_TAGS.contains(&&name[..end]) && after_name(&name[end..]) {
        return true;
    }
    // Condition 7: one whole open or closing tag of another name, and nothing after it but space.
    raw_html_len(line).is_some_and(|len| {
        let tag = &lower[..len];
        !tag.starts_with("<!") && !tag.starts_with("<?") && line[len..].trim().is_empty()
    })
}

/// A line's text as CommonMark's inline content shows it once raw HTML is dropped (§6.6): its tags, comments,
/// processing instructions, declarations and CDATA go; an autolink is its address (§6.5); any other `<` stays.
fn inline_html_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(found) = text[at..].find('<') {
        let start = at + found;
        out.push_str(&text[at..start]);
        let rest = &text[start..];
        if let Some(address) = autolink(rest) {
            out.push_str(address);
            at = start + address.len() + 2;
        } else if let Some(len) = raw_html_len(rest) {
            at = start + len;
        } else {
            out.push('<');
            at = start + 1;
        }
    }
    out.push_str(&text[at..]);
    out
}

/// An autolink at the start of `text` (`<scheme:…>` or `<address@host>`, CommonMark §6.5): what is between its brackets.
fn autolink(text: &str) -> Option<&str> {
    let end = text.find('>')?;
    let inner = &text[1..end];
    if inner.is_empty() || inner.contains(|c: char| c.is_whitespace() || c == '<') {
        return None;
    }
    let scheme = inner.find(':').map(|colon| &inner[..colon]).unwrap_or("");
    let uri = (2..=32).contains(&scheme.len())
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    let email = inner.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && local
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c))
            && !domain.is_empty()
            && domain.split('.').all(|label| {
                !label.is_empty()
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
    });
    (uri || email).then_some(inner)
}

/// The length of the raw HTML at the start of `text` (CommonMark §6.6): an open or closing tag, a comment, a
/// processing instruction, a declaration or a CDATA section.
fn raw_html_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let until =
        |from: usize, end: &str| text[from..].find(end).map(|found| from + found + end.len());
    if text.starts_with("<!-->") {
        return Some(5);
    }
    if text.starts_with("<!--->") {
        return Some(6);
    }
    if text.starts_with("<!--") {
        return until(4, "-->");
    }
    if text.starts_with("<?") {
        return until(2, "?>");
    }
    if text.starts_with("<![CDATA[") {
        return until(9, "]]>");
    }
    if text.starts_with("<!") {
        return bytes
            .get(2)
            .is_some_and(u8::is_ascii_alphabetic)
            .then(|| until(2, ">"))
            .flatten();
    }
    let space = |at: &mut usize| {
        let from = *at;
        while bytes.get(*at).is_some_and(|b| b.is_ascii_whitespace()) {
            *at += 1;
        }
        *at > from
    };
    let name = |at: &mut usize, first: fn(&u8) -> bool, more: fn(&u8) -> bool| {
        if !bytes.get(*at).is_some_and(first) {
            return false;
        }
        *at += 1;
        while bytes.get(*at).is_some_and(more) {
            *at += 1;
        }
        true
    };
    let tag_first = |b: &u8| b.is_ascii_alphabetic();
    let tag_more = |b: &u8| b.is_ascii_alphanumeric() || *b == b'-';
    if text.starts_with("</") {
        let mut at = 2;
        if !name(&mut at, tag_first, tag_more) {
            return None;
        }
        space(&mut at);
        return (bytes.get(at) == Some(&b'>')).then_some(at + 1);
    }
    let mut at = 1;
    if !name(&mut at, tag_first, tag_more) {
        return None;
    }
    loop {
        let before = at;
        if !space(&mut at) {
            break;
        }
        let attribute_first = |b: &u8| b.is_ascii_alphabetic() || *b == b'_' || *b == b':';
        let attribute_more =
            |b: &u8| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'.' | b':' | b'-');
        if !name(&mut at, attribute_first, attribute_more) {
            at = before;
            break;
        }
        let value_at = at;
        space(&mut at);
        if bytes.get(at) == Some(&b'=') {
            at += 1;
            space(&mut at);
            match bytes.get(at) {
                Some(b'"') => at = until(at + 1, "\"")?,
                Some(b'\'') => at = until(at + 1, "'")?,
                Some(_) => {
                    let from = at;
                    while bytes.get(at).is_some_and(|b| {
                        !b.is_ascii_whitespace()
                            && !matches!(*b, b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
                    }) {
                        at += 1;
                    }
                    if at == from {
                        return None;
                    }
                }
                None => return None,
            }
        } else {
            at = value_at;
        }
    }
    space(&mut at);
    if bytes.get(at) == Some(&b'/') {
        at += 1;
    }
    (bytes.get(at) == Some(&b'>')).then_some(at + 1)
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
        // An HTML block goes whole, a lone tag of any name among them; a code span keeps its tag as text.
        assert_eq!(shown("<div>\n\nInside\n\n</div>"), ["Inside"]);
        assert_eq!(
            shown("<img src=\"https://attacker.example/a.png\">"),
            Vec::<String>::new()
        );
        assert_eq!(shown("<!-- a comment -->\n\nAfter"), ["After"]);
        assert_eq!(shown("`<b>` stays a < b"), ["<b> stays a < b"]);
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
