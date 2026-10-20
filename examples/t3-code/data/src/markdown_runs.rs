// A block's runs as ChatMarkdown draws them (chat_runs), and GFM strikethrough, which the repository's
// parser does not read: remark-gfm's `~one~` and `~~two~~` become `<del>`.

/// Where a struck span opens and closes in the text the parser reads. Private-use marks are never
/// whitespace or punctuation, so the parser keeps them as text beside the span's neighbours.
const STRIKE_OPEN: char = '\u{e006}';
const STRIKE_CLOSE: char = '\u{e007}';

/// micromark's character groups for attention: 1 whitespace (or a line's edge), 2 punctuation (Unicode P or
/// S), 0 anything else.
fn attention_group(c: Option<char>) -> u8 {
    match c {
        None => 1,
        Some(c) if c.is_whitespace() => 1,
        Some(c) if c.is_ascii_punctuation() => 2,
        Some(c) => {
            let category = unicode_general_category::get_general_category(c).abbreviation();
            if category.starts_with(['P', 'S']) {
                2
            } else {
                0
            }
        }
    }
}

/// The byte ranges of one line's tilde runs that pair as strikethrough (micromark-extension-gfm-strikethrough):
/// a run of one or two tildes opens when what follows is not whitespace (punctuation only after whitespace or
/// punctuation), closes on the mirror rule, and closes the nearest open run of its own length. Code spans,
/// escapes, link destinations, autolinks and bare URLs hold no run.
fn strike_pairs(line: &str) -> Vec<(usize, usize, usize)> {
    let bytes = line.as_bytes();
    let mut open: Vec<(usize, usize)> = Vec::new();
    let mut pairs = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let rest = &line[at..];
        let word_start = at == 0 || line[..at].ends_with(char::is_whitespace);
        match bytes[at] {
            b'\\' => at += 1 + line[at + 1..].chars().next().map_or(0, char::len_utf8),
            b'`' => at = code_span_end(line, at).max(at + 1),
            b']' if rest.starts_with("](") => {
                let mut depth = 0usize;
                let mut end = line.len();
                for (offset, byte) in bytes[at + 1..].iter().enumerate() {
                    match byte {
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = at + 2 + offset;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                at = end;
            }
            b'<' if rest
                .find('>')
                .is_some_and(|close| rest[1..close].contains(':')) =>
            {
                at += rest.find('>').unwrap_or(0) + 1;
            }
            _ if word_start
                && ["http://", "https://", "www."]
                    .iter()
                    .any(|scheme| rest.starts_with(scheme)) =>
            {
                at += rest.find(char::is_whitespace).unwrap_or(rest.len());
            }
            b'~' => {
                let length = bytes[at..].iter().take_while(|byte| **byte == b'~').count();
                if length <= 2 {
                    let before = attention_group(line[..at].chars().next_back());
                    let after = attention_group(line[at + length..].chars().next());
                    let opens = after == 0 || (after == 2 && before != 0);
                    let closes = before == 0 || (before == 2 && after != 0);
                    let opener = if closes {
                        open.iter().rposition(|(_, size)| *size == length)
                    } else {
                        None
                    };
                    if let Some(index) = opener {
                        pairs.push((open[index].0, at, length));
                        open.truncate(index);
                    } else if opens {
                        open.push((at, length));
                    }
                }
                at += length;
            }
            _ => at += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    pairs
}

/// The text with each struck span's tildes replaced by STRIKE_OPEN and STRIKE_CLOSE. Fenced code is kept, and
/// a span stays on its line.
fn strike_marks(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('~') {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut fence: Option<(char, usize)> = None;
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        if let Some((marker, width)) = fence {
            let count = trimmed.chars().take_while(|c| *c == marker).count();
            if count >= width && trimmed[count..].trim().is_empty() {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some(marker @ ('`' | '~')) = trimmed.chars().next() {
            let width = trimmed.chars().take_while(|c| *c == marker).count();
            if width >= 3 {
                fence = Some((marker, width));
                out.push_str(line);
                continue;
            }
        }
        let mut marks: Vec<(usize, usize, char)> = strike_pairs(line)
            .into_iter()
            .flat_map(|(start, end, length)| {
                [(start, length, STRIKE_OPEN), (end, length, STRIKE_CLOSE)]
            })
            .collect();
        marks.sort_unstable();
        let mut from = 0;
        for (at, length, mark) in marks {
            out.push_str(&line[from..at]);
            out.push(mark);
            from = at + length;
        }
        out.push_str(&line[from..]);
    }
    std::borrow::Cow::Owned(out)
}

/// Each strike mark the parser kept inside a run's text as a run of its own, so the runs between them can be
/// struck whole however the words are later split (flow_tokens) or chipped.
fn split_strikes(blocks: &mut [markdown_parse::Block]) {
    let split = |runs: &mut Vec<markdown_parse::Run>| {
        if !runs
            .iter()
            .any(|run| run.text.contains([STRIKE_OPEN, STRIKE_CLOSE]))
        {
            return;
        }
        let mut out = Vec::with_capacity(runs.len() + 2);
        for run in runs.iter() {
            let mut from = 0;
            for (at, mark) in run.text.match_indices([STRIKE_OPEN, STRIKE_CLOSE]) {
                if at > from {
                    out.push(markdown_parse::Run {
                        text: run.text[from..at].to_string(),
                        ..run.clone()
                    });
                }
                out.push(markdown_parse::Run {
                    text: mark.to_string(),
                    ..run.clone()
                });
                from = at + mark.len();
            }
            if from < run.text.len() {
                out.push(markdown_parse::Run {
                    text: run.text[from..].to_string(),
                    ..run.clone()
                });
            }
        }
        *runs = out;
    };
    for block in blocks {
        split(&mut block.runs);
        block.cells.iter_mut().for_each(split);
    }
}

/// Whether a run's text is only a strike mark (the span's edge, drawn as nothing): Some(true) opens it.
fn strike_mark(text: &str) -> Option<bool> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(STRIKE_OPEN), None) => Some(true),
        (Some(STRIKE_CLOSE), None) => Some(false),
        _ => None,
    }
}

/// A block's runs with T3's link and chip presentation (chat_run, link_labels), each then `strike`: whether
/// it is inside a struck span. The marks themselves are dropped.
fn chat_runs(value: &Value) -> Value {
    let Value::List(runs) = value else {
        return value.clone();
    };
    let mut previous = String::new();
    let mut result = Vec::new();
    let mut struck = Vec::new();
    let mut strike = false;
    for run in runs.iter() {
        let text = match run {
            Value::Record(fields) => fields.get(1).and_then(Value::as_str).unwrap_or(""),
            _ => "",
        };
        if let Some(opens) = strike_mark(text) {
            strike = opens;
            continue;
        }
        let href = match run {
            Value::Record(fields) => fields
                .get(5)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            _ => String::new(),
        };
        let start = !href.is_empty() && href != previous;
        // The words after an image inside a link's text continue that link.
        previous = match media_link(&href) {
            (_, "") => href.clone(),
            (_, link) => link.to_string(),
        };
        if let (Some(target), Value::Record(fields)) = (href.strip_prefix(FILE_LINK), run) {
            let label = fields[1].as_str().unwrap_or("");
            if !isMarkdownFileLinkLabel(label, target) {
                let mut prose = fields.to_vec();
                prose[0] = Value::str(&format!("{}-label", fields[0].as_str().unwrap_or("")));
                prose[1] = Value::str(&format!("{label} "));
                prose[5] = Value::str("");
                result.push(chat_run(Value::record(prose), false));
                struck.push(strike);
            }
        }
        result.push(chat_run(run.clone(), start));
        struck.push(strike);
    }
    Value::list(
        link_labels(result)
            .into_iter()
            .zip(struck)
            .map(|(run, strike)| match run {
                Value::Record(fields) => {
                    let mut fields = fields.to_vec();
                    fields.push(Value::Bool(strike));
                    Value::record(fields)
                }
                other => other,
            })
            .collect(),
    )
}

#[cfg(test)]
mod strike_tests {
    use super::*;

    fn struck(text: &str) -> Vec<(String, bool)> {
        let Value::Record(document) = document(Value::str("m"), text) else {
            unreachable!()
        };
        let Value::List(blocks) = &document[1] else {
            unreachable!()
        };
        let Value::Record(block) = &blocks[0] else {
            unreachable!()
        };
        let Value::List(runs) = &block[7] else {
            unreachable!()
        };
        runs.iter()
            .map(|run| {
                let Value::Record(run) = run else {
                    unreachable!()
                };
                (
                    run[1].as_str().unwrap().to_string(),
                    run[10] == Value::Bool(true),
                )
            })
            .collect()
    }
    fn struck_text(text: &str) -> String {
        struck(text)
            .into_iter()
            .filter(|(_, strike)| *strike)
            .map(|(text, _)| text)
            .collect()
    }
    fn shown(text: &str) -> String {
        struck(text).into_iter().map(|(text, _)| text).collect()
    }

    #[test]
    fn double_and_single_tildes_strike_their_text() {
        assert_eq!(struck_text("Some ~~strike~~ here"), "strike");
        assert_eq!(shown("Some ~~strike~~ here"), "Some strike here");
        assert_eq!(struck_text("a ~one~ b"), "one");
        assert_eq!(struck_text("**bold**, ~~two words~~, *it*"), "two words");
    }

    #[test]
    fn a_struck_span_keeps_its_inline_styles_and_code() {
        let runs = struck("x ~~a `code` **b**~~ y");
        assert!(runs.iter().any(|(text, strike)| text == "code" && *strike));
        assert!(runs
            .iter()
            .any(|(text, strike)| text.trim() == "b" && *strike));
        assert!(runs
            .iter()
            .any(|(text, strike)| text.trim() == "y" && !*strike));
    }

    #[test]
    fn unmatched_spaced_or_coded_tildes_stay_text() {
        for text in [
            "a ~~ b ~~ c",
            "only ~~one side",
            "three ~~~x~~~ tildes",
            "in `~~code~~` only",
            "escaped \\~~x~~ is",
            "mixed ~~x~ sizes",
        ] {
            assert_eq!(struck_text(text), "", "{text}");
        }
        assert_eq!(shown("in `~~code~~` only"), "in ~~code~~ only");
        assert_eq!(shown("a ~~ b ~~ c"), "a ~~ b ~~ c");
    }

    #[test]
    fn link_destinations_urls_and_fences_keep_their_tildes() {
        assert_eq!(
            strike_marks("[x](https://a.com/~~y~~)"),
            "[x](https://a.com/~~y~~)"
        );
        assert_eq!(
            strike_marks("see https://a.com/~u~ and ~~z~~"),
            "see https://a.com/~u~ and \u{e006}z\u{e007}"
        );
        assert_eq!(
            strike_marks("```\n~~kept~~\n```\n~~gone~~"),
            "```\n~~kept~~\n```\n\u{e006}gone\u{e007}"
        );
    }

    #[test]
    fn a_table_cell_strikes_and_copies_its_source() {
        let source = "| a | b |\n| - | - |\n| ~~x~~ | y |";
        let text = strike_marks(source);
        let mut doc = markdown_parse::parse(&text, &|href| href.to_string());
        split_strikes(&mut doc.blocks);
        let row = doc
            .blocks
            .iter()
            .find(|block| !block.header && !block.cells.is_empty())
            .unwrap();
        assert_eq!(cell_markdown(&row.cells[0]), "~~x~~");
        assert_eq!(cell_plain(&row.cells[0]), "x");
    }
}
