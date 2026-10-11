// media-signed-urls-and-attachment-markdown (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): images inside a line of text,
// and the links of a document with no workspace folder.
//
// ChatMarkdown's `img` renderer draws every image: one alone in its block standalone (`markStandaloneImages`), one in a
// line of text inline (ChatMarkdownImage with `standalone` false). The repository's parser keeps an image only when it
// starts a block on a line of its own (`markdown_parse` `image_of`) and flattens any other one into its alt text, so
// each other image is rewritten into a link whose destination carries IMAGE_MARK; `chat_run` makes it a "media" run that
// markdown.contract FlowRuns draws through the image's media chip (media-views.ts `markdownInlineImages`), with the
// href media-views.ts computes for it: IMAGE_MARK, then the image source as the URL transform leaves it.

/// The mark of an inline image's run href.
const IMAGE_MARK: &str = "t3-image:";

/// `![alt](src)` alone on a line, as `markdown_parse` `image_of` reads it (the line trimmed at its start).
fn sole_image_line(trimmed: &str) -> bool {
    trimmed
        .strip_prefix("![")
        .is_some_and(|rest| rest.contains("](") && rest.ends_with(')'))
}

/// Each image the parser would flatten, as `[alt](<IMAGE_MARK>src)`. Kept: fenced code, code spans, table lines (their
/// cells draw text runs), an image inside a link's text, and an image line that starts a block (the parser's image
/// block). An image line that continues a paragraph is inline, as the parser's paragraph takes it in.
fn inline_image_links(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains("![") {
        return std::borrow::Cow::Borrowed(text);
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let tables = table_lines(&lines);
    let mut out = String::with_capacity(text.len() + 32);
    let mut fence: Option<(char, usize)> = None;
    let mut paragraph = false;
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
            paragraph = false;
            continue;
        }
        if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let width = trimmed.chars().take_while(|c| *c == marker).count();
            if width >= 3 {
                fence = Some((marker, width));
                out.push_str(line);
                paragraph = false;
                continue;
            }
        }
        let block_image = !paragraph && sole_image_line(trimmed);
        if block_image || tables[index] || !line.contains("![") {
            out.push_str(line);
        } else {
            image_links_in_line(line, &mut out);
        }
        paragraph = !block_image && prose_line(line);
    }
    std::borrow::Cow::Owned(out)
}

/// `![alt](inside)` at `at`: the `]` and the `)` that close it (the destination's parentheses balanced).
fn image_span(line: &str, at: usize) -> Option<(usize, usize)> {
    let bytes = line.as_bytes();
    let close = at + 2 + line[at + 2..].find(']')?;
    if bytes.get(close + 1) != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, byte) in bytes[close + 1..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((close, close + 1 + offset));
                }
            }
            _ => {}
        }
    }
    None
}

fn image_links_in_line(line: &str, out: &mut String) {
    let bytes = line.as_bytes();
    let (mut at, mut copied, mut brackets) = (0usize, 0usize, 0usize);
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'`' => {
                // A code span: the same number of backticks closes it on this line, else they are text.
                let run = bytes[at..].iter().take_while(|b| **b == b'`').count();
                let fence = &line[at..at + run];
                let mut from = at + run;
                at = loop {
                    match line[from..].find(fence) {
                        Some(found) => {
                            let start = from + found;
                            let length = bytes[start..].iter().take_while(|b| **b == b'`').count();
                            if length == run {
                                break start + run;
                            }
                            from = start + length;
                        }
                        None => break at + run,
                    }
                };
            }
            b'[' => {
                brackets += 1;
                at += 1;
            }
            b']' => {
                brackets = brackets.saturating_sub(1);
                at += 1;
            }
            b'!' if brackets == 0 && bytes.get(at + 1) == Some(&b'[') => {
                let Some((close, end)) = image_span(line, at) else {
                    at += 1;
                    continue;
                };
                let inside = &line[close + 2..end];
                let mut lead = inside.len() - inside.trim_start().len();
                if inside[lead..].starts_with('<') {
                    lead += 1;
                }
                out.push_str(&line[copied..at]);
                out.push_str(&line[at + 1..close + 2]);
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
            let resolved = link(source);
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

/// AttachmentFilePreview's ChatMarkdown has no `cwd` (audit-wave-followups-3 FW-2): resolveMarkdownFileLinkTarget finds
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

/// An inline image's alt text is one token: its runs (emphasis splits it) join into the first.
fn push_media_token(tokens: &mut Vec<markdown_parse::Run>, run: &markdown_parse::Run) {
    match tokens.last_mut() {
        Some(last) if last.href == run.href => last.text.push_str(&run.text),
        _ => tokens.push(run.clone()),
    }
}

#[cfg(test)]
mod media_tests {
    use super::*;

    fn runs_of(document: &Value) -> Vec<(String, String, String)> {
        let Value::Record(fields) = document else {
            unreachable!()
        };
        let Value::List(blocks) = &fields[1] else {
            unreachable!()
        };
        blocks
            .iter()
            .flat_map(|block| {
                let Value::Record(block) = block else {
                    unreachable!()
                };
                let kind = block[1].as_str().unwrap_or("").to_string();
                let Value::List(runs) = &block[7] else {
                    return Vec::new();
                };
                runs.iter()
                    .map(|run| {
                        let Value::Record(run) = run else {
                            unreachable!()
                        };
                        (
                            kind.clone(),
                            run[1].as_str().unwrap_or("").to_string(),
                            format!(
                                "{}|{}",
                                run[5].as_str().unwrap_or(""),
                                run[6].as_str().unwrap_or("")
                            ),
                        )
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn images_inside_a_line_are_rewritten_and_block_images_kept() {
        let text = "Beside: ![Shot](shot.png) and ![Logo **x**](<../a b.png> \"t\").\n\n![Alone](shot.png)\n\nText\n![Joined](j.png)\n\n`![code](c.png)` \\![esc](e.png) [![linked](l.png)](https://a.com)\n\n```\n![fenced](f.png)\n```\n\n| a | b |\n| - | - |\n| ![cell](c.png) | x |";
        let rewritten = inline_image_links(text);
        assert!(rewritten.contains(
            "Beside: [Shot](t3-image:shot.png) and [Logo **x**](<t3-image:../a b.png> \"t\")."
        ));
        assert!(
            rewritten.contains("\n![Alone](shot.png)\n"),
            "a block image stays"
        );
        assert!(
            rewritten.contains("Text\n[Joined](t3-image:j.png)"),
            "a line that continues a paragraph is inline"
        );
        assert!(rewritten
            .contains("`![code](c.png)` \\![esc](e.png) [![linked](l.png)](https://a.com)"));
        assert!(rewritten.contains("![fenced](f.png)") && rewritten.contains("| ![cell](c.png) |"));
        assert_eq!(
            rewritten.lines().count(),
            text.lines().count(),
            "line structure kept for task offsets"
        );
    }

    #[test]
    fn an_inline_image_is_one_media_run_with_its_resolved_source() {
        let document = document_with_tasks(Value::str("m"), "See ![Logo **x**](<../a b.png>) and ![Web](https://a.com/w.png) or ![Bad](javascript:x.png).", None, false);
        let media: Vec<(String, String, String)> = runs_of(&document)
            .into_iter()
            .filter(|run| run.2.ends_with("|media"))
            .collect();
        assert_eq!(
            media,
            vec![
                (
                    "paragraph".into(),
                    "Logo x".into(),
                    "t3-image:t3-file:../a b.png|media".into()
                ),
                (
                    "paragraph".into(),
                    "Web".into(),
                    "t3-image:https://a.com/w.png|media".into()
                ),
                ("paragraph".into(), "Bad".into(), "t3-image:|media".into()),
            ]
        );
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
