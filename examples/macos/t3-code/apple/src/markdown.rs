//! T3's presentation of the repository's Markdown parser, without changing
//! generic Exact Markdown styling or the server's message protocol.
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Placement};

pub struct Markdown;
pub type Data<J> = exact_data::Mixed<J, exact_data::Placed<Markdown>>;

/// Every source app.ts answers must be listed as TypeScript-owned below, or the
/// bake refuses it as an unknown source (chat.test.ts checks the two agree).
pub fn mixed<J: DataSource>(javascript: J, placement: Placement) -> Data<J> {
    exact_data::Mixed::new(
        javascript,
        exact_data::Placed::new(Markdown, placement),
        &["snapshot", "snapshotSettings", "modelCatalog", "settings", "settingsNavigation", "archivedSettings", "storageSettings", "keybindingSettings", "saveKeybinding", "scheduledSettings", "saveScheduledTask", "scopedControls", "saveScopedText", "licenseSettings", "diagnosticsSettings", "projectGroups", "providerManagement", "createProvider", "command", "composerEditor", "composerBranches", "acpRegistry", "connectionsPage", "settingsBPicker", "settingsBSshHosts", "pairingFields", "integrationsPage", "keyboardDispatch", "projectsView", "providerAdd", "providerChange", "providerPage", "providerWizard", "settingsCore", "sourceControlPage", "pagesHome", "usagePage", "usageKeys", "paletteView", "paletteCommand", "shellView", "shellDetails", "sidebarLaunchWidth", "prList", "prDetail", "welcome", "timelineAttachments"],
        &["renderMarkdown", "renderPullRequestMarkdown"],
    )
    .expect("T3 presentation sources have distinct owners")
    .with_embedded_rust(|placed| Ok(exact_data::Placed::new(Markdown, placed.given_placement())))
}

impl DataSource for Markdown {
    fn app_id(&self) -> &str {
        "com.exact.t3code.macos"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        // renderPullRequestMarkdown: the same parse for a pull request body and its comments (lane pages).
        if source != "renderMarkdown" && source != "renderPullRequestMarkdown" {
            return Err(DataError::Unavailable(format!(
                "Unknown T3 source: {source}"
            )));
        }
        let Some(Value::List(messages)) = args.first() else {
            return Ok(Value::record(vec![Value::list(Vec::new())]));
        };
        let mut documents = Vec::new();
        for message in messages.iter() {
            let Value::Record(fields) = message else {
                continue;
            };
            let Some(id) = fields.first() else { continue };
            let kind = fields.get(1).and_then(Value::as_str).unwrap_or("");
            if kind != "assistant" && kind != "plan" && kind != "user" {
                continue;
            }
            let text = fields.get(3).and_then(Value::as_str).unwrap_or("");
            // UserMessageBody renders ChatMarkdown with `lineBreaks`: a single
            // newline is a line break, kept through the parse as U+2028.
            if kind == "user" {
                documents.push(document(id.clone(), &line_breaks(text)));
                continue;
            }
            // A long plan carries its collapsed preview after U+0001 (timeline-plan.ts).
            let (shown, preview) = match text.split_once('\u{1}') {
                Some((shown, preview)) if kind == "plan" => (shown, Some(preview)),
                _ => (text, None),
            };
            documents.push(document(id.clone(), shown));
            if let (Some(preview), Some(name)) = (preview, id.as_str()) {
                documents.push(document(Value::str(&format!("{name}#preview")), preview));
            }
        }
        Ok(Value::record(vec![Value::list(documents)]))
    }
}

include!("pierre_icons.rs");
include!("r4_timeline_tables.rs");
include!("r4_integrate_align.rs");

/// The resolver's mark for a workspace file link (`[label](src/a.ts:3)`): T3
/// draws it as a file chip, not a web link.
const FILE_LINK: &str = "t3-file:";

fn is_file_link(href: &str) -> bool {
    let lower = href.to_ascii_lowercase();
    !href.is_empty() && !href.starts_with('#') && !lower.contains("://")
        && !["data:", "javascript:", "mailto:", "tel:"].iter().any(|scheme| lower.starts_with(scheme))
}

/// MarkdownFileLink's chip label: the file's name, then " · L3" for a line.
fn file_chip(target: &str) -> (String, &'static str) {
    let mut path = target;
    let mut line = "";
    if let Some((before, anchor)) = path.split_once("#L") {
        if !anchor.is_empty() && anchor.chars().take_while(|c| c.is_ascii_digit()).count() > 0 {
            line = &anchor[..anchor.chars().take_while(|c| c.is_ascii_digit()).count()];
            path = before;
        }
    }
    // `path:12` or `path:12:4`: the first number after the path is its line.
    let mut pieces = path.splitn(2, ':');
    let head = pieces.next().unwrap_or(path);
    if let Some(rest) = pieces.next() {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && line.is_empty() {
            path = head;
            return (label(path, &digits), pierre_icon_token(path));
        }
    }
    (label(path, line), pierre_icon_token(path))
}
fn label(path: &str, line: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    if line.is_empty() { name.to_string() } else { format!("{name} \u{b7} L{line}") }
}

/// A `shape ChatRun` record with T3's link presentation: `kind` is "file"
/// (a chip whose text is its label), "link-start"/"link" for a web link (the
/// favicon precedes its first word), or "" for prose; `icon` names the chip's
/// Pierre icon.
fn chat_run(value: Value, link_start: bool) -> Value {
    let Value::Record(fields) = value else { return value };
    let mut fields = fields.to_vec();
    let href = fields.get(5).and_then(Value::as_str).unwrap_or("").to_string();
    let text = fields.get(1).and_then(Value::as_str).unwrap_or("").to_string();
    if text == LINE_BREAK {
        fields.push(Value::str("break"));
        fields.push(Value::str(""));
        return Value::record(fields);
    }
    let (kind, icon) = if let Some(rest) = href.strip_prefix("t3-context://v1/") {
        // A composer context reference: its kind picks the chip's accent and icon.
        let kind = rest.split('/').next().unwrap_or("");
        // The href stays: r4-timeline-chips.ts resolves the chip by it.
        // A mention (an @-tagged file or folder) is the mention chip MarkdownFileLink also draws.
        if kind == "mention" {
            ("file", pierre_icon_token(&text))
        } else {
            let icon = if kind == "file" { pierre_icon_token(&text) } else { "" };
            (context_kind(kind), icon)
        }
    } else if href.starts_with("t3-citation://") {
        // An assistant quote: the chip shows the quoted text.
        let quoted = query_param(&href, "text").unwrap_or_default();
        if !quoted.is_empty() { fields[1] = Value::str(&quoted); }
        // The href stays: r4-timeline-chips.ts resolves the chip by it.
        ("context-citation", "")
    } else if let Some(target) = href.strip_prefix(FILE_LINK) {
        let (text, icon) = file_chip(target);
        fields[1] = Value::str(&text);
        ("file", icon)
    } else if href.is_empty() {
        ("", "")
    } else if link_start {
        ("link-start", "")
    } else {
        ("link", "")
    };
    fields.push(Value::str(kind));
    fields.push(Value::str(icon));
    Value::record(fields)
}
/// A private-use mark (never whitespace, so the parser's line trim keeps it).
const LINE_BREAK: &str = "\u{e000}";

/// Marks each single newline between two lines of prose (outside fences) so
/// the paragraph keeps it as a line break.
fn line_breaks(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    // A pipe line outside a table is prose (remark-breaks keeps its newline).
    let tables = table_lines(&lines);
    let prose = |index: usize| prose_line(lines[index]) || (lines[index].trim().starts_with('|') && !tables[index]);
    let mut out = String::new();
    let mut fenced = false;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        out.push_str(line);
        if index + 1 < lines.len() {
            if !fenced && prose(index) && prose(index + 1) {
                out.push_str(LINE_BREAK);
            }
            out.push('\n');
        }
    }
    out
}
/// A line that continues a paragraph rather than starting another block.
fn prose_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let ordered = trimmed.split_once(['.', ')']).is_some_and(|(digits, _)| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()));
    !(trimmed.starts_with('#') || trimmed.starts_with('|') || trimmed.starts_with('>') || trimmed.starts_with("```") || trimmed.starts_with("~~~")
        || trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") || ordered || trimmed == "---" || trimmed == "***")
}
fn context_kind(kind: &str) -> &'static str {
    match kind {
        "file" => "context-file",
        "thread" => "context-thread",
        "review-comment" => "context-review-comment",
        "terminal" => "context-terminal",
        "element" | "preview-annotation" => "context-element",
        "image" => "context-image",
        "skill" => "context-skill",
        _ => "context-other",
    }
}
/// One `?name=value` from a URL, percent-decoded (`+` as a space).
fn query_param(href: &str, name: &str) -> Option<String> {
    let query = href.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == name {
            let bytes = value.as_bytes();
            let mut decoded = Vec::with_capacity(bytes.len());
            let mut at = 0;
            while at < bytes.len() {
                match bytes[at] {
                    b'+' => { decoded.push(b' '); at += 1; }
                    b'%' if at + 2 < bytes.len() => {
                        match std::str::from_utf8(&bytes[at + 1..at + 3]).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok()).ok_or(()) {
                            Ok(byte) => { decoded.push(byte); at += 3; }
                            Err(_) => { decoded.push(b'%'); at += 1; }
                        }
                    }
                    byte => { decoded.push(byte); at += 1; }
                }
            }
            return Some(String::from_utf8_lossy(&decoded).into_owned());
        }
    }
    None
}

fn chat_runs(value: &Value) -> Value {
    let Value::List(runs) = value else { return value.clone() };
    let mut previous = String::new();
    Value::list(runs.iter().map(|run| {
        let href = match run { Value::Record(fields) => fields.get(5).and_then(Value::as_str).unwrap_or("").to_string(), _ => String::new() };
        let start = !href.is_empty() && href != previous;
        previous = href;
        chat_run(run.clone(), start)
    }).collect())
}

/// One message's Markdown as the ChatMarkdown component draws it.
fn document(id: Value, text: &str) -> Value {
    let text = image_chip_links(text);
    let doc = markdown_parse::parse(&text, &|href| {
        let lower = href.to_ascii_lowercase();
        if ["https://", "http://", "mailto:", "tel:"]
            .iter()
            .any(|scheme| lower.starts_with(scheme))
        {
            href.to_string()
        } else if lower.starts_with("t3-context://") || lower.starts_with("t3-citation://") {
            href.to_string()
        } else if is_file_link(href) {
            format!("{FILE_LINK}{href}")
        } else {
            String::new()
        }
    });
    let mut previous: Option<&markdown_parse::Block> = None;
    let mut blocks = Vec::new();
    let mut skip = 0;
    let aligns = table_aligns(&text);
    let mut table_count = 0;
    for (index, block) in doc.blocks.iter().enumerate() {
        let gap = block_gap(previous, block, index);
        previous = Some(block);
        if index < skip {
            continue;
        }
        // A table's rows (a header row, then rows up to the next header) draw as one block.
        if matches!(block.kind, markdown_parse::Kind::TableRow) {
            let mut end = index + 1;
            while end < doc.blocks.len() && matches!(doc.blocks[end].kind, markdown_parse::Kind::TableRow) && !doc.blocks[end].header {
                end += 1;
            }
            let rows: Vec<&markdown_parse::Block> = doc.blocks[index..end].iter().collect();
            blocks.push(table_block(index, gap, &rows, aligns.get(table_count).map(Vec::as_slice).unwrap_or(&[])));
            table_count += 1;
            skip = end;
            continue;
        }
        let Value::Record(fields) = markdown_parse::value::block(index, block) else {
            unreachable!()
        };
        let mut fields = fields.to_vec();
        fields.push(Value::Number(gap));
        let flow = flow_tokens(block);
        if let Some(tokens) = &flow {
            // A paragraph's line breaks split it into one block per line, so each
            // line wraps on its own and a bubble fits its longest line.
            let lines: Vec<&[markdown_parse::Run]> = tokens.split(|run| run.text == LINE_BREAK).collect();
            if lines.len() > 1 {
                for (line_index, line) in lines.iter().enumerate() {
                    let mut line_fields = fields.clone();
                    line_fields[0] = Value::str(&format!("{index}.{line_index}"));
                    line_fields[7] = chat_runs(&Value::list(line.iter().enumerate().map(|(i, run)| markdown_parse::value::run(i, run)).collect()));
                    if line_index > 0 {
                        line_fields[9] = Value::Number(0.0);
                    }
                    line_fields.push(Value::Bool(true));
                    no_table(&mut line_fields);
                    blocks.push(Value::record(line_fields));
                }
                continue;
            }
            fields[7] = Value::list(tokens.iter().enumerate().map(|(i, run)| markdown_parse::value::run(i, run)).collect());
        }
        fields[7] = chat_runs(&fields[7]);
        if let Value::List(cells) = &fields[8] {
            fields[8] = Value::list(cells.iter().map(|cell| match cell {
                Value::Record(cell) => {
                    let mut cell = cell.to_vec();
                    if cell.len() > 1 { cell[1] = chat_runs(&cell[1]); }
                    Value::record(cell)
                }
                other => other.clone(),
            }).collect());
        }
        fields.push(Value::Bool(flow.is_some()));
        no_table(&mut fields);
        blocks.push(Value::record(fields));
    }
    Value::record(vec![id, Value::list(blocks)])
}

/// Prose holding inline code is laid out word by word so each code span can be
/// a real box (T3's 1px border, 6pt radius and 1.6/5.6 padding): the shared
/// inline text renderer paints span backgrounds only. Words keep their trailing
/// spaces; a code span stays whole, as a browser keeps an inline box together.
fn flow_tokens(block: &markdown_parse::Block) -> Option<Vec<markdown_parse::Run>> {
    use markdown_parse::Kind;
    // Links flow too: a file link is a chip box and a web link opens with its favicon.
    if !matches!(block.kind, Kind::Paragraph | Kind::Item | Kind::Quote)
        || !block.runs.iter().any(|run| run.code || !run.href.is_empty() || run.text.contains(LINE_BREAK))
    {
        return None;
    }
    let mut tokens = Vec::new();
    for run in &block.runs {
        if run.code || run.href.starts_with(FILE_LINK) || run.href.starts_with("t3-context://") || run.href.starts_with("t3-citation://") {
            tokens.push(run.clone());
            continue;
        }
        let mut start = 0;
        let mut in_space = false;
        let text = run.text.as_str();
        for (at, character) in text.char_indices() {
            // A line break is its own token; the space the parser joined after it is dropped.
            if character == '\u{e000}' {
                if at > start {
                    tokens.push(markdown_parse::Run { text: text[start..at].to_string(), ..run.clone() });
                }
                tokens.push(markdown_parse::Run { text: LINE_BREAK.to_string(), href: String::new(), code: false, ..run.clone() });
                start = at + character.len_utf8();
                if text[start..].starts_with(' ') {
                    start += 1;
                }
                in_space = false;
                continue;
            }
            if at < start {
                continue;
            }
            let space = character.is_whitespace();
            // A token is a word with its trailing spaces, or leading spaces alone.
            if at > start && in_space && !space {
                tokens.push(markdown_parse::Run { text: text[start..at].to_string(), ..run.clone() });
                start = at;
            }
            in_space = space;
        }
        if start < text.len() {
            tokens.push(markdown_parse::Run { text: text[start..].to_string(), ..run.clone() });
        }
    }
    Some(collapse_spaces(tokens))
}

/// CSS `white-space: normal` for the flowed words: a run of spaces is one
/// space, across token boundaries too (a chip followed by "  next" keeps one),
/// and none starts a line. Code spans and chips keep their text.
fn collapse_spaces(tokens: Vec<markdown_parse::Run>) -> Vec<markdown_parse::Run> {
    let mut previous_space = true;
    let mut collapsed = Vec::with_capacity(tokens.len());
    for mut token in tokens {
        if token.text == LINE_BREAK {
            previous_space = true;
            collapsed.push(token);
            continue;
        }
        if token.code || token.href.starts_with(FILE_LINK) || token.href.starts_with("t3-context://") || token.href.starts_with("t3-citation://") {
            previous_space = false;
            collapsed.push(token);
            continue;
        }
        let mut text = String::with_capacity(token.text.len());
        for character in token.text.chars() {
            if character.is_whitespace() {
                if !previous_space {
                    text.push(' ');
                }
                previous_space = true;
            } else {
                text.push(character);
                previous_space = false;
            }
        }
        if text.is_empty() {
            continue;
        }
        token.text = text;
        collapsed.push(token);
    }
    collapsed
}

/// The collapsed vertical margin T3's `.chat-markdown` CSS leaves before a
/// block: headings take 1.25rem, prose/lists/code .65rem, sibling list items
/// .25rem, and the rows of one table none.
fn block_gap(previous: Option<&markdown_parse::Block>, block: &markdown_parse::Block, index: usize) -> f64 {
    use markdown_parse::Kind;
    let Some(previous) = previous.filter(|_| index > 0) else {
        return 0.0;
    };
    match (previous.kind, block.kind) {
        (_, Kind::Heading) => 20.0,
        (Kind::Item, Kind::Item) if previous.depth == block.depth => 4.0,
        (Kind::TableRow, Kind::TableRow) => 0.0,
        _ => 10.4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_keeps_message_identity_and_parses_message_rows() {
        let message = |id: &str, kind: &str, text: &str| {
            Value::record(vec![
                Value::str(id),
                Value::str(kind),
                Value::str(""),
                Value::str(text),
            ])
        };
        let result = Markdown
            .query(
                "renderMarkdown",
                &[Value::list(vec![
                    message("user-1", "user", "# Plain user text"),
                    message(
                        "answer-1",
                        "assistant",
                        "# Answer\n\nA paragraph.\n\n- First\n- Second",
                    ),
                    message("checkpoint-1", "checkpoint", ""),
                ])],
            )
            .unwrap();
        let Value::Record(result) = result else {
            panic!("document record")
        };
        let Value::List(documents) = &result[0] else {
            panic!("document list")
        };
        assert_eq!(documents.len(), 2);
        let Value::Record(document) = &documents[1] else {
            panic!("message document")
        };
        assert_eq!(document[0].as_str(), Some("answer-1"));
        let Value::List(blocks) = &document[1] else {
            panic!("blocks")
        };
        let gaps: Vec<_> = blocks
            .iter()
            .map(|block| {
                let Value::Record(fields) = block else {
                    panic!("block")
                };
                fields[9].as_number().unwrap()
            })
            .collect();
        assert_eq!(gaps, vec![0.0, 10.4, 10.4, 4.0]);
    }

    #[test]
    fn prose_with_inline_code_flows_word_by_word() {
        let doc = markdown_parse::parse("`a.md` changed  in **the** repo.", &|href| href.to_string());
        let tokens = flow_tokens(&doc.blocks[0]).expect("code makes a flow block");
        let texts: Vec<_> = tokens.iter().map(|run| (run.text.as_str(), run.code, run.bold)).collect();
        assert_eq!(texts, vec![("a.md", true, false), (" ", false, false), ("changed ", false, false),
            ("in ", false, false), ("the", false, true), (" ", false, false), ("repo.", false, false)]);
        // Spaces collapse across a chip as a browser collapses them: "md]  please" keeps one.
        let doc = markdown_parse::parse("Check [a.md](t3-context://v1/mention/m1)  please", &|href| href.to_string());
        let texts: Vec<String> = flow_tokens(&doc.blocks[0]).expect("a chip flows").iter().map(|run| run.text.clone()).collect();
        assert_eq!(texts, vec!["Check ", "a.md", " ", "please"]);
        let plain = markdown_parse::parse("No code here.", &|href| href.to_string());
        assert!(flow_tokens(&plain.blocks[0]).is_none());
    }

    #[test]
    fn file_links_are_chips_and_web_links_lead_with_their_favicon() {
        let Value::Record(doc) = document(Value::str("a"), "See [the result](fixture-result.md), [pkg](./package.json:3) and [the reference](https://example.com/docs).") else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else { panic!("blocks") };
        let Value::Record(block) = &blocks[0] else { panic!("block") };
        assert_eq!(block[10], Value::Bool(true));
        let Value::List(runs) = &block[7] else { panic!("runs") };
        let runs: Vec<(String, String, String, String)> = runs.iter().map(|run| {
            let Value::Record(fields) = run else { panic!("run") };
            let text = |at: usize| fields[at].as_str().unwrap_or("").to_string();
            (text(1), text(5), text(6), text(7))
        }).filter(|run| !run.2.is_empty()).collect();
        assert_eq!(runs, vec![
            ("fixture-result.md".into(), "t3-file:fixture-result.md".into(), "file".into(), "markdown".into()),
            ("package.json \u{b7} L3".into(), "t3-file:./package.json:3".into(), "file".into(), "npm-plain".into()),
            ("the ".into(), "https://example.com/docs".into(), "link-start".into(), "".into()),
            ("reference".into(), "https://example.com/docs".into(), "link".into(), "".into()),
        ]);
        assert_eq!(pierre_icon_token("src/app.tsx"), "react");
        assert_eq!(pierre_icon_token("Cargo.toml"), "default");
    }

    #[test]
    fn user_text_keeps_line_breaks_and_draws_context_references_as_chips() {
        let Value::Record(doc) = document(Value::str("u"), &line_breaks("First line\nsecond [notes.md](t3-context://v1/file/file_1) and [Assistant quote](t3-citation://v1/e/t/m?text=Quoted%20bit&start=0&end=9)\n\n- one\n- two")) else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else { panic!("blocks") };
        assert_eq!(blocks.len(), 4);
        let line = |at: usize| {
            let Value::Record(block) = &blocks[at] else { panic!("block") };
            let Value::List(runs) = &block[7] else { panic!("runs") };
            let runs: Vec<(String, String, String)> = runs.iter().map(|run| {
                let Value::Record(fields) = run else { panic!("run") };
                let text = |at: usize| fields[at].as_str().unwrap_or("").to_string();
                (text(1), text(6), text(7))
            }).collect();
            (block[0].as_str().unwrap_or("").to_string(), block[9].as_number().unwrap(), runs)
        };
        assert_eq!(line(0), ("0.0".into(), 0.0, vec![("First ".into(), "".into(), "".into()), ("line".into(), "".into(), "".into())]));
        assert_eq!(line(1), ("0.1".into(), 0.0, vec![
            ("second ".into(), "".into(), "".into()), ("notes.md".into(), "context-file".into(), "markdown".into()), (" ".into(), "".into(), "".into()),
            ("and ".into(), "".into(), "".into()), ("Quoted bit".into(), "context-citation".into(), "".into()),
        ]));
        assert_eq!(line_breaks("# Title\nText\n| a |\n|---|"), "# Title\nText\n| a |\n|---|");
    }

    #[test]
    fn tables_are_one_block_with_sizing_columns_and_copy_text() {
        let Value::Record(doc) = document(Value::str("t"), &line_breaks("Intro\n\n| Name | Note |\n|:-|:-|\n| **a** | x, \"y\" |\n| `b` | a long one |\n\n| not | a table\nafter ![shot.png](t3-context://v1/image/image_1)")) else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else { panic!("blocks") };
        let kinds: Vec<String> = blocks.iter().map(|block| {
            let Value::Record(fields) = block else { panic!("block") };
            assert_eq!(fields.len(), 15);
            fields[1].as_str().unwrap_or("").to_string()
        }).collect();
        assert_eq!(kinds, vec!["paragraph", "table", "paragraph", "paragraph"]);
        let Value::Record(table) = &blocks[1] else { panic!("table") };
        let Value::List(rows) = &table[11] else { panic!("rows") };
        assert_eq!(rows.len(), 3);
        let Value::List(columns) = &table[12] else { panic!("columns") };
        assert_eq!(columns.len(), 2);
        assert_eq!(table[13].as_str(), Some("| Name | Note |\n| --- | --- |\n| **a** | x, \"y\" |\n| `b` | a long one |"));
        assert_eq!(table[14].as_str(), Some("Name,Note\na,\"x, \"\"y\"\"\"\nb,a long one"));
        // The pipe line that is no table keeps its break; the image reference is a chip.
        let Value::Record(last) = &blocks[3] else { panic!("line") };
        let Value::List(runs) = &last[7] else { panic!("runs") };
        let chip = runs.iter().find_map(|run| {
            let Value::Record(fields) = run else { return None };
            (fields[6].as_str() == Some("context-image")).then(|| (fields[1].as_str().unwrap_or("").to_string(), fields[5].as_str().unwrap_or("").to_string()))
        });
        assert_eq!(chip, Some(("shot.png".into(), "t3-context://v1/image/image_1".into())));
    }

    #[test]
    fn gaps_follow_collapsed_chat_markdown_margins() {
        let doc = markdown_parse::parse(
            "Intro.\n\n## Section\n\n- One\n  - Nested\n- Two\n\n| a | b |\n|---|---|\n| 1 | 2 |",
            &|href| href.to_string(),
        );
        let mut previous = None;
        let gaps: Vec<f64> = doc
            .blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let gap = block_gap(previous, block, index);
                previous = Some(block);
                gap
            })
            .collect();
        assert_eq!(gaps, vec![0.0, 20.0, 10.4, 10.4, 10.4, 10.4, 0.0]);
    }
}
