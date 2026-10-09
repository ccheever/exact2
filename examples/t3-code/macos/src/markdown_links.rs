// markdown-links-and-files-preview (T3 Code 1e2ecbd975, MIT, see LICENSE-T3):
// ChatMarkdown.tsx's `markdownUrlTransform` over react-markdown's `defaultUrlTransform`
// and its sanitize schema, which decide which link destinations survive to the anchor,
// and GFM task-list items (MarkdownListItem's `findTaskListMarkerOffset`, MarkdownInput).

/// An anchor whose destination the transform emptied: `[parser](fixture.txt:3)` reads
/// `fixture.txt` as a URL scheme, so the label is link-coloured text with no target.
const NO_HREF: &str = "t3-anchor:";

/// react-markdown's `safeProtocol` (and the sanitize schema's href protocols).
const SAFE_SCHEMES: [&str; 6] = ["http", "https", "irc", "ircs", "mailto", "xmpp"];

/// The scheme `defaultUrlTransform` reads: the text before a colon that comes
/// before any `/`, `?` or `#`. `src/a.ts:3` has none; `a.ts:3` has `a.ts`.
fn url_scheme(href: &str) -> Option<&str> {
    let colon = href.find(':')?;
    let earlier = |mark: char| href.find(mark).is_some_and(|at| at < colon);
    (!earlier('/') && !earlier('?') && !earlier('#')).then(|| &href[..colon])
}

/// isWindowsDrivePathHref (`C:\repo` or `C:/repo`, percent-encoded or not):
/// remarkNormalizeLinksAndTagInlineCode turns it into a `file:` URL first.
fn windows_drive(href: &str) -> bool {
    let bytes = href.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_alphabetic) {
        return false;
    }
    let rest = &href[1..];
    let rest = match rest.strip_prefix(':') {
        Some(rest) => rest,
        None if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case("%3a") => &rest[3..],
        None => return false,
    };
    rest.starts_with(['/', '\\'])
        || rest.len() >= 3
            && (rest[..3].eq_ignore_ascii_case("%2f") || rest[..3].eq_ignore_ascii_case("%5c"))
}

/// markdownUrlTransform: citation and context references, Windows drive paths and
/// `file:` URLs pass; anything else passes when it has no scheme or a safe one.
fn kept_by_transform(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    lower.starts_with("t3-context://")
        || lower.starts_with("t3-citation://")
        || lower.starts_with("file:")
        || windows_drive(href)
        || url_scheme(href)
            .is_none_or(|scheme| SAFE_SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()))
}

/// A destination T3 draws as a file chip: a `file:` URL, a Windows drive path, or
/// one with no scheme that is not a fragment or a protocol-relative URL.
fn is_file_link(href: &str) -> bool {
    href.to_ascii_lowercase().starts_with("file:")
        || windows_drive(href)
        || !href.is_empty()
            && !href.starts_with('#')
            && !href.starts_with("//")
            && url_scheme(href).is_none()
}

/// The parser's link resolver: what each destination becomes in a run's `href`.
/// media-views.ts `markdownLinkHref` is the same rule for the chips' side.
fn link_href(href: &str) -> String {
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("t3-context://") || lower.starts_with("t3-citation://") {
        href.to_string()
    } else if !kept_by_transform(href) {
        NO_HREF.to_string()
    } else if is_file_link(href) {
        format!("{FILE_LINK}{href}")
    } else if href.is_empty() || href.starts_with('#') {
        String::new()
    } else {
        href.to_string()
    }
}

/// `- [ ] `, `1. [x] `: the `[` of a list item's task marker (GFM wants whitespace and
/// some text after it), from the item's text after its bullet.
fn task_marker(rest: &str) -> Option<bool> {
    let checked = match rest.get(..3)? {
        "[ ]" => false,
        "[x]" | "[X]" => true,
        _ => return None,
    };
    let after = &rest[3..];
    (after.starts_with([' ', '\t']) && !after.trim().is_empty()).then_some(checked)
}

/// Every line the parser starts a list item on (outside fences, not a rule), in
/// order: whether it is a task item and the UTF-16 offset of its `[` in `text`.
fn item_lines(text: &str) -> Vec<Option<usize>> {
    let mut items = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.strip_suffix('\n').unwrap_or(raw);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim_start();
        if let Some((marker, width)) = fence {
            if trimmed.chars().take_while(|&c| c == marker).count() >= width
                && trimmed.trim_end_matches(marker).is_empty()
            {
                fence = None;
            }
        } else if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let width = trimmed.chars().take_while(|&c| c == marker).count();
            if width >= 3 {
                fence = Some((marker, width));
            } else {
                items.extend(item_line(line, trimmed, offset));
            }
        } else {
            items.extend(item_line(line, trimmed, offset));
        }
        offset += raw.encode_utf16().count();
    }
    items
}

/// One line's item start (markdown_parse's bullet_of after its is_rule check).
fn item_line(line: &str, trimmed: &str, offset: usize) -> Option<Option<usize>> {
    if trimmed.is_empty() || is_rule_line(trimmed) {
        return None;
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    let after = if trimmed.starts_with(['-', '*', '+']) {
        &trimmed[1..]
    } else if (1..=9).contains(&digits) && trimmed[digits..].starts_with(['.', ')']) {
        &trimmed[digits + 1..]
    } else {
        return None;
    };
    if !after.starts_with(' ') {
        return None;
    }
    let rest = after.trim_start();
    Some(task_marker(rest).map(|_| {
        let at = line.len() - rest.len();
        offset + line[..at].encode_utf16().count()
    }))
}

/// markdown_parse's is_rule: three or more of one of `-`, `*`, `_`, spaces between.
fn is_rule_line(line: &str) -> bool {
    let mut marks = line.bytes().filter(|&b| b != b' ');
    let Some(marker @ (b'-' | b'*' | b'_')) = marks.next() else {
        return false;
    };
    let mut count = 1;
    for mark in marks {
        if mark != marker {
            return false;
        }
        count += 1;
    }
    count >= 3
}

/// Strips each task item's marker from its first run and says, per block, its state
/// ("open" or "done") and the marker's offset in `text` (-1 when it cannot be placed:
/// the item lines and the parser's items disagree, so no toggle can name it).
fn task_items(blocks: &mut [markdown_parse::Block], text: &str) -> Vec<(&'static str, f64)> {
    let lines = item_lines(text);
    let items = blocks
        .iter()
        .filter(|block| matches!(block.kind, markdown_parse::Kind::Item))
        .count();
    let placed = lines.len() == items;
    let mut line = lines.into_iter();
    blocks
        .iter_mut()
        .map(|block| {
            if !matches!(block.kind, markdown_parse::Kind::Item) {
                return ("", -1.0);
            }
            let offset = line.next().flatten();
            let Some(first) = block.runs.first_mut() else {
                return ("", -1.0);
            };
            if first.code || !first.href.is_empty() || placed && offset.is_none() {
                return ("", -1.0);
            }
            let Some(checked) = task_marker(&first.text) else {
                return ("", -1.0);
            };
            first.text = first.text[3..].trim_start().to_string();
            if first.text.is_empty() {
                block.runs.remove(0);
            }
            let offset = offset.filter(|_| placed).map_or(-1.0, |at| at as f64);
            (if checked { "done" } else { "open" }, offset)
        })
        .collect()
}

#[cfg(test)]
mod link_tests {
    use super::*;

    fn blocks_of(doc: &Value) -> Vec<&[Value]> {
        let Value::Record(doc) = doc else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else {
            panic!("blocks")
        };
        blocks
            .iter()
            .map(|block| match block {
                Value::Record(fields) => fields.as_ref(),
                _ => panic!("block"),
            })
            .collect()
    }
    fn runs_of(block: &[Value]) -> Vec<&[Value]> {
        let Value::List(runs) = &block[7] else {
            panic!("runs")
        };
        runs.iter()
            .map(|run| match run {
                Value::Record(fields) => fields.as_ref(),
                _ => panic!("run"),
            })
            .collect()
    }

    // markdown-links-and-files-preview TH-9: react-markdown's defaultUrlTransform empties a
    // destination whose first colon comes before any `/`, `?` or `#` unless its scheme is safe.
    #[test]
    fn relative_name_line_links_lose_their_target() {
        for (href, expected) in [
            ("fixture.txt:3", NO_HREF.to_string()),
            ("Makefile:12", NO_HREF.to_string()),
            ("a.ts:12:4", NO_HREF.to_string()),
            ("fixture.txt", format!("{FILE_LINK}fixture.txt")),
            ("src/a.ts:3", format!("{FILE_LINK}src/a.ts:3")),
            ("./fixture.txt:3", format!("{FILE_LINK}./fixture.txt:3")),
            (
                "/repo/fixture.txt:3",
                format!("{FILE_LINK}/repo/fixture.txt:3"),
            ),
            ("a.ts#L3", format!("{FILE_LINK}a.ts#L3")),
            ("C:\\repo\\a.ts:3", format!("{FILE_LINK}C:\\repo\\a.ts:3")),
            ("C:/repo/a.ts", format!("{FILE_LINK}C:/repo/a.ts")),
            ("file:///repo/a.ts", format!("{FILE_LINK}file:///repo/a.ts")),
            (
                "https://example.com/a:b",
                "https://example.com/a:b".to_string(),
            ),
            ("mailto:me@example.com", "mailto:me@example.com".to_string()),
            ("xmpp:me@example.com", "xmpp:me@example.com".to_string()),
            ("tel:123", NO_HREF.to_string()),
            ("javascript:alert(1)", NO_HREF.to_string()),
            ("data:text/plain,hi", NO_HREF.to_string()),
            ("ftp://example.com/a", NO_HREF.to_string()),
            ("#section", String::new()),
            ("//example.com/a", "//example.com/a".to_string()),
            (
                "t3-context://v1/file/f1",
                "t3-context://v1/file/f1".to_string(),
            ),
        ] {
            assert_eq!(link_href(href), expected, "{href}");
        }
    }

    #[test]
    fn the_fixture_message_keeps_one_chip_and_two_targetless_links() {
        let doc = document(
            Value::str("m"),
            "Review [parser](fixture.txt:3), [fixture.txt](fixture.txt), [fixture.txt:3](fixture.txt:3).",
        );
        let blocks = blocks_of(&doc);
        let runs = runs_of(blocks[0]);
        let shown: Vec<(&str, &str, &str)> = runs
            .iter()
            .map(|r| {
                (
                    r[1].as_str().unwrap_or(""),
                    r[5].as_str().unwrap_or(""),
                    r[6].as_str().unwrap_or(""),
                )
            })
            .collect();
        assert_eq!(
            shown,
            vec![
                ("Review ", "", ""),
                ("parser", "", "link"),
                (", ", "", ""),
                ("fixture.txt", "t3-file:fixture.txt", "file"),
                (", ", "", ""),
                ("fixture.txt:3", "", "link"),
                (".", "", ""),
            ]
        );
        // A table cell copies the label alone, as serializeAnchor does for a link with no href.
        let parsed = markdown_parse::parse("[parser](fixture.txt:3)", &|href| link_href(href));
        assert_eq!(cell_markdown(&parsed.blocks[0].runs), "parser");
    }

    #[test]
    fn task_items_carry_their_state_and_marker_offset() {
        let text = "Tasks é:\n\n- [ ] Write the tests\n- [x] Ship **it**\n- plain\n\n```md\n- [ ] in a fence\n```\n\n1. [X] numbered\n- [ ]\n";
        let doc = document_with_tasks(Value::str("f"), text, None, true);
        let tasks: Vec<(String, f64, String)> = blocks_of(&doc)
            .into_iter()
            .filter(|fields| fields[1].as_str() == Some("item"))
            .map(|fields| {
                let first = runs_of(fields)[0][1].as_str().unwrap_or("").to_string();
                let Value::Number(offset) = fields[17] else {
                    panic!("offset")
                };
                (fields[16].as_str().unwrap_or("").to_string(), offset, first)
            })
            .collect();
        let utf16 = |needle: &str| text[..text.find(needle).unwrap()].encode_utf16().count() as f64;
        assert_eq!(
            tasks,
            vec![
                ("open".into(), utf16("[ ] Write"), "Write the tests".into()),
                ("done".into(), utf16("[x] Ship"), "Ship ".into()),
                ("".into(), -1.0, "plain".into()),
                ("done".into(), utf16("[X] numbered"), "numbered".into()),
                ("".into(), -1.0, "[ ]".into()),
            ]
        );
        // In the transcript the same items draw GFM's disabled checkbox: no offset to write.
        let transcript = document(Value::str("m"), text);
        let items: Vec<&[Value]> = blocks_of(&transcript)
            .into_iter()
            .filter(|fields| fields[1].as_str() == Some("item"))
            .collect();
        assert_eq!(
            items
                .iter()
                .filter(|f| f[16].as_str() == Some("done"))
                .count(),
            2
        );
        assert!(items.iter().all(|f| f[17] == Value::Number(-1.0)));
        // renderFileMarkdown answers a "file" source (r4-surfaces-files.ts markdownSource) with offsets.
        let source = Value::list(vec![Value::record(vec![
            Value::str("file:docs/notes.md"),
            Value::str("file"),
            Value::str(""),
            Value::str(text),
        ])]);
        let answer = Markdown
            .query("renderFileMarkdown", &[source])
            .expect("answer");
        let Value::Record(answer) = answer else {
            panic!("answer")
        };
        let Value::List(documents) = &answer[0] else {
            panic!("documents")
        };
        assert_eq!(
            blocks_of(&documents[0])
                .iter()
                .filter(|f| f[17] != Value::Number(-1.0))
                .count(),
            3
        );
    }
}
