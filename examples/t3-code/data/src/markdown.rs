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
        &[
            "activityInteraction",
            "activityResumed",
            "snapshot",
            "sideChatData",
            "snapshotSettings",
            "modelCatalog",
            "settings",
            "settingsNavigation",
            "archivedSettings",
            "storageSettings",
            "keybindingSettings",
            "saveKeybinding",
            "scheduledSettings",
            "saveScheduledTask",
            "scopedControls",
            "saveScopedText",
            "licenseSettings",
            "diagnosticsSettings",
            "projectGroups",
            "providerManagement",
            "createProvider",
            "command",
            "composerAttach",
            "composerEditor",
            "composerRich",
            // A draft image's upload, a stream that tells its bytes sent (src/composer/composer-uploads.ts).
            "composerUpload",
            "composerField",
            "composerFieldWaiting",
            "composerFieldObserved",
            "composerFieldAcknowledged",
            "composerIntent",
            "composerChip", // a skill chip's details popover (composer-chip-popover.ts)
            "composerChipObserved",
            "composerChipPressed",
            "composerWorkspace",
            "refreshComposerWorkspace",
            "refreshTimelineReads",
            "highlightAsk",
            "highlightStore",
            "composerBranches",
            "acpRegistry",
            "connectionsPage",
            "settingsBPicker",
            "settingsBSshHosts",
            "sshPrompt",
            "sshPromptAnswer",
            "pairingFields",
            "integrationsPage",
            "keyboardDispatch",
            "projectsView",
            "providerAdd",
            "providerChange",
            "providerPage",
            "providerWizard",
            "settingsCore",
            "sourceControlPage",
            "promptLinkView",
            "promptLinksReady",
            "codexReceive",
            "pagesHome",
            "usagePage",
            "usageKeys",
            "paletteView",
            "paletteCommand",
            "shellView",
            "serverBrowserControl",
            "serverBrowserDecode",
            "serverBrowserDialog",
            "serverBrowserFrame",
            "serverBrowserKey",
            "serverBrowserMouse",
            "serverBrowserPainted",
            "serverBrowserResize",
            "serverBrowserStatus",
            "serverBrowserText",
            "serverBrowserUpload",
            "serverBrowserWheel",
            "shellDetails",
            "chatCanvas",
            "threadFind",
            "terminalDrawer", // the thread terminal drawer (terminal-drawer-view.ts)
            "sidebarLaunchWidth",
            "navigationHistory", // the window's Back and Forward (sidebar-history.ts)
            "prList",
            "prDetail",
            "welcome",
            "timelineAttachments",
            // The server connections (src/transport/transport.ts): sockets, attempts, update jobs, replies, the tick.
            "transportSocket",
            "transportConnect",
            "transportUpdateStep",
            "transportUpdateSocket",
            "transportDeliver",
            "transportPump",
        ],
        &[
            "renderMarkdown",
            "renderQuestionMarkdown",
            "renderPullRequestMarkdown",
            "renderFileMarkdown",
            "highlightCode",
        ],
    )
    .expect("T3 presentation sources have distinct owners")
    .with_embedded_rust(|placed| Ok(exact_data::Placed::new(Markdown, placed.given_placement())))
}

impl DataSource for Markdown {
    fn app_id(&self) -> &str {
        "com.exact.t3code.macos"
    }

    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        if source == "highlightCode" {
            return Ok(crate::highlights::highlight_code(args));
        }
        // Pull requests, their comments, Files and sent attachments share this Markdown parse.
        if source != "renderMarkdown"
            && source != "renderQuestionMarkdown"
            && source != "renderPullRequestMarkdown"
            && source != "renderFileMarkdown"
        {
            return Err(DataError::Unavailable(format!(
                "Unknown T3 source: {source}"
            )));
        }
        let Some(Value::List(messages)) = args.first() else {
            return Ok(Value::record(vec![Value::list(Vec::new())]));
        };
        // Render math (Settings › Appearance): renderMarkdown's seventh argument, renderFileMarkdown's second.
        let math = matches!(
            args.get(if source == "renderMarkdown" { 6 } else { 1 }),
            Some(Value::Bool(true))
        ) && (source == "renderMarkdown" || source == "renderFileMarkdown");
        let mut documents = Vec::new();
        for message in messages.iter() {
            let Value::Record(fields) = message else {
                continue;
            };
            let Some(id) = fields.first() else { continue };
            let kind = if source == "renderQuestionMarkdown" {
                "assistant"
            } else {
                fields.get(1).and_then(Value::as_str).unwrap_or("")
            };
            // A workflow thread's opening carries its script as a fenced block (timeline-workflow-opening.ts).
            if kind != "assistant"
                && kind != "workflow"
                && kind != "plan"
                && kind != "user"
                && kind != "breaks"
                && kind != "file"
                && kind != "attachment"
                && kind != "notebook"
            {
                continue;
            }
            let text = fields.get(3).and_then(Value::as_str).unwrap_or("");
            let fenced = if math {
                display_math_fences(text)
            } else {
                String::new()
            };
            let text = if math { fenced.as_str() } else { text };
            // FileMarkdownPreview: task checkboxes write the editable file.
            if kind == "file" {
                documents.push(document_with_tasks(id.clone(), text, None, true));
                continue;
            }
            // AttachmentFilePreview: no `cwd`, task checkboxes disabled.
            if kind == "attachment" {
                documents.push(document_with_links(id.clone(), text, None, false, false));
                continue;
            }
            // NotebookPreview's Markdown cell: an attachment's, made inert (`inert_blocks`).
            if kind == "notebook" {
                documents.push(chat_document(
                    id.clone(),
                    text,
                    None,
                    false,
                    false,
                    false,
                    true,
                ));
                continue;
            }
            // UserMessageBody keeps single newlines as U+2028 line breaks and its raw HTML literal.
            if kind == "user" {
                documents.push(user_document(id.clone(), &line_breaks(text), args.get(1)));
                continue;
            }
            // PullRequestMarkdown on a host whose own rendering breaks single newlines (`hardLineBreaks`).
            if kind == "breaks" {
                documents.push(document_with_skills(
                    id.clone(),
                    &line_breaks(text),
                    args.get(1),
                ));
                continue;
            }
            // A long plan carries its collapsed preview after U+0001 (timeline-plan.ts).
            let (shown, preview) = match text.split_once('\u{1}') {
                Some((shown, preview)) if kind == "plan" => (shown, Some(preview)),
                _ => (text, None),
            };
            documents.push(document_with_skills(id.clone(), shown, args.get(1)));
            if let (Some(preview), Some(name)) = (preview, id.as_str()) {
                documents.push(document_with_skills(
                    Value::str(&format!("{name}#preview")),
                    preview,
                    args.get(1),
                ));
            }
        }
        Ok(find_documents(documents, messages, args))
    }
}

include!("markdown_find.rs");
include!("pierre_icons.rs");
include!("markdown_tables.rs");
include!("markdown_table_align.rs");
include!("markdown_links.rs");
include!("markdown_media.rs");
include!("markdown_headings.rs");
include!("markdown_runs.rs");
include!("markdown_direction.rs");
include!("markdown_math.rs");
include!("markdown_breaks.rs");
include!("markdown_inert.rs");
/// The resolver's mark for a workspace file link (`[label](src/a.ts:3)`): T3
/// draws it as a file chip, not a web link.
const FILE_LINK: &str = "t3-file:";
/// A link to another thread (`[title](t3-thread://v1/<id>)`, or `t3-thread://v2/<environment>/<id>` and its autolink
/// `<t3-thread://v2/…>`): drawn as that thread's chip.
fn thread_link(href: &str) -> bool {
    href.starts_with("t3-thread://v1/") || href.starts_with("t3-thread://v2/")
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
    if line.is_empty() {
        name.to_string()
    } else {
        format!("{name} \u{b7} L{line}")
    }
}

/// A `shape ChatRun` record with T3's link presentation: `kind` is "file"
/// (a chip whose text is its label), "link-start"/"link" for a web link (the
/// favicon precedes its first word), or "" for prose; `icon` names the chip's
/// Pierre icon.
fn chat_run(value: Value, link_start: bool) -> Value {
    let Value::Record(fields) = value else {
        return value;
    };
    let mut fields = fields.to_vec();
    let href = fields
        .get(5)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let text = fields
        .get(1)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if text == LINE_BREAK {
        fields.push(Value::str("break"));
        fields.push(Value::str(""));
        return Value::record(fields);
    }
    let (kind, icon) = if let Some(rest) = href.strip_prefix("t3-context://v1/") {
        // A composer context reference: its kind picks the chip's accent and icon.
        let kind = rest.split('/').next().unwrap_or("");
        // The href stays: message-chips.ts resolves the chip by it.
        // A mention (an @-tagged file or folder) is the mention chip MarkdownFileLink also draws.
        if kind == "mention" {
            ("file", pierre_icon_token(&text))
        } else {
            let icon = if kind == "file" {
                pierre_icon_token(&text)
            } else {
                ""
            };
            (context_kind(kind), icon)
        }
    } else if href.starts_with("t3-skill:") {
        ("context-skill", "")
    } else if thread_link(&href) {
        // A thread link is its thread's chip (MarkdownThreadLink); message-chips.ts resolves it by the href.
        ("context-thread", "")
    } else if href.starts_with("t3-citation://") {
        // An assistant quote: the chip shows the quoted text.
        let quoted = query_param(&href, "text").unwrap_or_default();
        if !quoted.is_empty() {
            fields[1] = Value::str(&quoted);
        }
        // The href stays: message-chips.ts resolves the chip by it.
        ("context-citation", "")
    } else if href.starts_with(IMAGE_MARK) {
        // An image inside a line of text: FlowRuns draws it through its media chip.
        ("media", "")
    } else if let Some(target) = href.strip_prefix(FILE_LINK) {
        let (text, icon) = file_chip(target);
        fields[1] = Value::str(&text);
        ("file", icon)
    } else if href == NO_HREF {
        // An anchor the URL transform emptied.
        fields[5] = Value::str("");
        ("link", "")
    } else if href.is_empty() {
        ("", "")
    } else if link_start {
        ("link-start", "")
    } else {
        ("link", "")
    };
    if href.is_empty() && fields.get(4) == Some(&Value::Bool(true)) {
        fields[5] = Value::str(&format!("t3-inline-code:{text}"));
    }
    fields.push(Value::str(kind));
    fields.push(Value::str(icon));
    Value::record(fields)
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
                    b'+' => {
                        decoded.push(b' ');
                        at += 1;
                    }
                    b'%' if at + 2 < bytes.len() => {
                        match std::str::from_utf8(&bytes[at + 1..at + 3])
                            .ok()
                            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                            .ok_or(())
                        {
                            Ok(byte) => {
                                decoded.push(byte);
                                at += 3;
                            }
                            Err(_) => {
                                decoded.push(b'%');
                                at += 1;
                            }
                        }
                    }
                    byte => {
                        decoded.push(byte);
                        at += 1;
                    }
                }
            }
            return Some(String::from_utf8_lossy(&decoded).into_owned());
        }
    }
    None
}

// Ported isMarkdownFileLinkLabel from T3 Code 1e2ecbd975 markdownLinks.ts
// (MIT, LICENSE-T3). The parser already classified the destination as a file;
// Exact only adapts path/position decoding here, keeping one classification rule.
#[allow(non_snake_case)]
fn isMarkdownFileLinkLabel(label: &str, href: &str) -> bool {
    fn position(value: &str) -> (String, Option<u32>, Option<u32>) {
        let (path, hash) = value.split_once('#').unwrap_or((value, ""));
        let mut parts: Vec<&str> = path.split(':').collect();
        let number = |s: &str| {
            if !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) {
                s.parse::<u32>().ok()
            } else {
                None
            }
        };
        let last = parts.last().and_then(|s| number(s));
        if let Some(last) = last {
            parts.pop();
            let before = parts.last().and_then(|s| number(s));
            if let Some(line) = before {
                parts.pop();
                return (
                    parts.join(":"),
                    (line > 0).then_some(line),
                    (last > 0).then_some(last),
                );
            }
            return (parts.join(":"), (last > 0).then_some(last), None);
        }
        let hash = hash.to_ascii_lowercase();
        let (line, column) = hash
            .strip_prefix('l')
            .map(|v| v.split_once('c').unwrap_or((v, "")))
            .unwrap_or(("", ""));
        (
            path.into(),
            number(line).filter(|n| *n > 0),
            number(column).filter(|n| *n > 0),
        )
    }
    let decoded = query_param(
        &format!("?path={}", href.replace('+', "%2B").replace('&', "%26")),
        "path",
    )
    .unwrap_or_else(|| href.into());
    let destination = decoded
        .strip_prefix("file://localhost")
        .or_else(|| decoded.strip_prefix("file://"))
        .unwrap_or(&decoded);
    let destination =
        if destination.as_bytes().get(2) == Some(&b':') && destination.starts_with('/') {
            &destination[1..]
        } else {
            destination
        };
    let (label_path, line, column) = position(label.trim());
    let (destination_path, destination_line, destination_column) = position(destination);
    if line.is_some() && line != destination_line
        || column.is_some() && column != destination_column
    {
        return false;
    }
    let normalize = |path: &str| {
        let path = path.replace('\\', "/");
        path.strip_prefix("./")
            .unwrap_or(&path)
            .trim_end_matches('/')
            .to_string()
    };
    let mut label_path = normalize(&label_path);
    let mut destination_path = normalize(&destination_path);
    if label_path.is_empty() {
        return true;
    }
    if destination_path.as_bytes().get(1) == Some(&b':') || destination.starts_with("\\\\") {
        label_path = label_path.to_lowercase();
        destination_path = destination_path.to_lowercase();
    }
    destination_path == label_path || destination_path.ends_with(&format!("/{label_path}"))
}

// SkillInlineText's text-only traversal after Markdown parsing: code and links
// retain their source. Names/display names come from the validated TS skill list.
fn skill_runs(
    runs: &[markdown_parse::Run],
    skills: &[(String, String)],
) -> Vec<markdown_parse::Run> {
    let mut result = Vec::new();
    for run in runs {
        if run.code || !run.href.is_empty() {
            result.push(run.clone());
            continue;
        }
        let mut cursor = 0;
        for (at, character) in run.text.char_indices() {
            if character != '$'
                || at > 0
                    && !run.text[..at]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_whitespace)
            {
                continue;
            }
            let tail = &run.text[at + 1..];
            let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
            let name = &tail[..end];
            let Some((_, display)) = skills.iter().find(|(known, _)| known == name) else {
                continue;
            };
            if at > cursor {
                result.push(markdown_parse::Run {
                    text: run.text[cursor..at].into(),
                    ..run.clone()
                });
            }
            result.push(markdown_parse::Run {
                text: display.clone(),
                href: format!("t3-skill:${name}"),
                ..run.clone()
            });
            cursor = at + 1 + end;
        }
        if cursor < run.text.len() {
            result.push(markdown_parse::Run {
                text: run.text[cursor..].into(),
                ..run.clone()
            });
        }
    }
    result
}

/// One message's Markdown as the ChatMarkdown component draws it.
#[cfg(test)]
fn document(id: Value, text: &str) -> Value {
    document_with_skills(id, text, None)
}

// The parser strips a code fence and its final newline. Preserve whether each
// source fence closed so an unfinished block cannot inherit a runnable command
// from an earlier block with identical text.
fn closed_code_fences(text: &str) -> Vec<bool> {
    let mut closed = Vec::new();
    let mut open: Option<(char, usize)> = None;
    for line in text.lines() {
        let line = line.trim_start();
        match open {
            Some((marker, width)) => {
                let count = line.chars().take_while(|c| *c == marker).count();
                if count >= width && line[count..].trim().is_empty() {
                    closed.push(true);
                    open = None;
                }
            }
            None => {
                if let Some(marker @ ('`' | '~')) = line.chars().next() {
                    let width = line.chars().take_while(|c| *c == marker).count();
                    if width >= 3 {
                        open = Some((marker, width));
                    }
                }
            }
        }
    }
    if open.is_some() {
        closed.push(false);
    }
    closed
}

fn document_with_skills(id: Value, text: &str, skill_values: Option<&Value>) -> Value {
    document_with_tasks(id, text, skill_values, false)
}

/// `toggle_tasks`: a task item keeps its marker's offset, so its checkbox can write the
/// file; elsewhere (the transcript) GFM's checkbox is disabled and the offset is -1.
fn document_with_tasks(
    id: Value,
    text: &str,
    skill_values: Option<&Value>,
    toggle_tasks: bool,
) -> Value {
    document_with_links(id, text, skill_values, toggle_tasks, true)
}

/// `cwd`: the document has a workspace folder; without one (a sent attachment) a
/// relative file link is targetless (`links_without_cwd`).
fn document_with_links(
    id: Value,
    text: &str,
    skill_values: Option<&Value>,
    toggle_tasks: bool,
    cwd: bool,
) -> Value {
    chat_document(id, text, skill_values, toggle_tasks, cwd, true, false)
}

/// A user message: UserMessageBody renders with `parseRawHtml={false}`, so a raw HTML line is literal text.
fn user_document(id: Value, text: &str, skill_values: Option<&Value>) -> Value {
    chat_document(id, text, skill_values, false, true, false, false)
}

/// `raw_html`: the document parses raw HTML (rehype-raw), so a raw HTML line's text has a direction.
/// `inert`: a saved notebook's cell, which loads no URL and links nowhere (`inert_blocks`).
fn chat_document(
    id: Value,
    text: &str,
    skill_values: Option<&Value>,
    toggle_tasks: bool,
    cwd: bool,
    raw_html: bool,
    inert: bool,
) -> Value {
    let skills: Vec<(String, String)> = match skill_values {
        Some(Value::List(skills)) => skills
            .iter()
            .filter_map(|skill| {
                let Value::Record(fields) = skill else {
                    return None;
                };
                Some((
                    fields.first()?.as_str()?.into(),
                    fields.get(1)?.as_str()?.into(),
                ))
            })
            .collect(),
        _ => Vec::new(),
    };
    let inert_text;
    let text = if inert {
        inert_text = inert_source(text);
        inert_text.as_str()
    } else {
        text
    };
    // Task offsets name the file's own text: image_chip_links drops the `!` of each
    // `![name](t3-context://…)`, which would shift every later marker by one.
    let source = text;
    let text = image_chip_links(text);
    // An image inside a line of text is a media run.
    let text = strike_marks(&inline_image_links(&text)).into_owned();
    let mut doc = markdown_parse::parse(&text, &|href| image_link(&link_href, href));
    linked_images(&mut doc.blocks, &|href| image_link(&link_href, href));
    if inert {
        inert_blocks(&mut doc.blocks);
    }
    // An image alone on its line is a block the parser resolved as a link: its source resolves as media.
    for block in &mut doc.blocks {
        if matches!(block.kind, markdown_parse::Kind::Image)
            && block.href != NO_HREF
            && !block.href.starts_with(FILE_LINK)
        {
            block.href = media_href(&block.href);
        }
    }
    if !cwd {
        links_without_cwd(&mut doc.blocks);
    }
    unmark_runs(&mut doc.blocks);
    split_strikes(&mut doc.blocks);
    heading_ids(&mut doc.blocks);
    fragment_links(&mut doc.blocks);
    let gutters = list_gutters(&doc.blocks);
    let mut tasks = task_items(&mut doc.blocks, source);
    // After the task markers go: `- [x] تم` reads right to left.
    let directions = block_directions(&doc.blocks, &text, &context_image_hrefs(source), raw_html);
    if !toggle_tasks {
        tasks.iter_mut().for_each(|task| task.1 = -1.0);
    }
    for block in &mut doc.blocks {
        // An image's emptied source loads nothing, nor does a fragment (media-views.ts markdownImageHref).
        if matches!(block.kind, markdown_parse::Kind::Image)
            && (block.href == NO_HREF || block.href.starts_with('#'))
        {
            block.href.clear();
        }
        block.runs = skill_runs(&block.runs, &skills);
        for cell in &mut block.cells {
            *cell = skill_runs(cell, &skills);
        }
    }
    let fences = closed_code_fences(&text);
    let mut code_index = 0;
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
            while end < doc.blocks.len()
                && matches!(doc.blocks[end].kind, markdown_parse::Kind::TableRow)
                && !doc.blocks[end].header
            {
                end += 1;
            }
            let rows: Vec<&markdown_parse::Block> = doc.blocks[index..end].iter().collect();
            let mut table = table_block(
                index,
                gap,
                &rows,
                aligns.get(table_count).map(Vec::as_slice).unwrap_or(&[]),
            );
            if let Value::Record(fields) = &table {
                let mut fields = fields.to_vec();
                fields.push(Value::str(directions[index]));
                table = Value::record(fields);
            }
            blocks.push(table);
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
            let lines: Vec<&[markdown_parse::Run]> =
                tokens.split(|run| run.text == LINE_BREAK).collect();
            if lines.len() > 1 {
                for (line_index, line) in lines.iter().enumerate() {
                    let mut line_fields = fields.clone();
                    line_fields[0] = Value::str(&format!("{index}.{line_index}"));
                    line_fields[7] = chat_runs(&Value::list(
                        line.iter()
                            .enumerate()
                            .map(|(i, run)| markdown_parse::value::run(i, run))
                            .collect(),
                    ));
                    // A list item's later line goes on with its text: no marker and no checkbox of its own.
                    let task = if line_index > 0 {
                        line_fields[3] = Value::str("");
                        line_fields[9] = Value::Number(0.0);
                        ("", -1.0)
                    } else {
                        tasks[index]
                    };
                    line_fields.push(Value::Bool(true));
                    no_table(&mut line_fields);
                    line_fields.extend(task_fields(task));
                    line_fields.push(Value::Number(gutters[index]));
                    line_fields.push(Value::str(directions[index]));
                    blocks.push(Value::record(line_fields));
                }
                continue;
            }
            fields[7] = Value::list(
                tokens
                    .iter()
                    .enumerate()
                    .map(|(i, run)| markdown_parse::value::run(i, run))
                    .collect(),
            );
        }
        fields[7] = chat_runs(&fields[7]);
        if let Some(tokens) = &flow {
            standalone_media(block, tokens, tasks[index].0, &mut fields[7]);
        }
        if let Value::List(cells) = &fields[8] {
            fields[8] = Value::list(
                cells
                    .iter()
                    .map(|cell| match cell {
                        Value::Record(cell) => {
                            let mut cell = cell.to_vec();
                            if cell.len() > 1 {
                                cell[1] = chat_runs(&cell[1]);
                            }
                            Value::record(cell)
                        }
                        other => other.clone(),
                    })
                    .collect(),
            );
        }
        fields.push(Value::Bool(flow.is_some()));
        no_table(&mut fields);
        if matches!(block.kind, markdown_parse::Kind::Code) {
            if let Some(last) = fields.last_mut() {
                *last = Value::Bool(fences.get(code_index).copied().unwrap_or(false));
            }
            code_index += 1;
        }
        fields.extend(task_fields(tasks[index]));
        fields.push(Value::Number(gutters[index]));
        fields.push(Value::str(directions[index]));
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
    if !matches!(
        block.kind,
        Kind::Paragraph | Kind::Item | Kind::Quote | Kind::Heading
    ) || !block
        .runs
        .iter()
        .any(|run| run.code || !run.href.is_empty() || run.text.contains(LINE_BREAK))
    {
        return None;
    }
    let mut tokens = Vec::new();
    for run in &block.runs {
        if run.href.starts_with(IMAGE_MARK) {
            push_media_token(&mut tokens, run);
            continue;
        }
        if run.code
            || run.href.starts_with(FILE_LINK)
            || run.href.starts_with("t3-context://")
            || run.href.starts_with("t3-citation://")
            || run.href.starts_with("t3-skill:")
            || thread_link(&run.href)
        {
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
                    tokens.push(markdown_parse::Run {
                        text: text[start..at].to_string(),
                        ..run.clone()
                    });
                }
                tokens.push(markdown_parse::Run {
                    text: LINE_BREAK.to_string(),
                    href: String::new(),
                    code: false,
                    ..run.clone()
                });
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
                tokens.push(markdown_parse::Run {
                    text: text[start..at].to_string(),
                    ..run.clone()
                });
                start = at;
            }
            in_space = space;
        }
        if start < text.len() {
            tokens.push(markdown_parse::Run {
                text: text[start..].to_string(),
                ..run.clone()
            });
        }
    }
    Some(collapse_spaces(tokens))
}

/// Collapse prose spaces across tokens and omit line-leading spaces.
/// Code spans and chips retain whitespace.
fn collapse_spaces(tokens: Vec<markdown_parse::Run>) -> Vec<markdown_parse::Run> {
    let mut previous_space = true;
    let mut collapsed = Vec::with_capacity(tokens.len());
    for mut token in tokens {
        if token.text == LINE_BREAK {
            previous_space = true;
            collapsed.push(token);
            continue;
        }
        if token.code
            || token.href.starts_with(IMAGE_MARK)
            || token.href.starts_with(FILE_LINK)
            || token.href.starts_with("t3-context://")
            || token.href.starts_with("t3-citation://")
            || token.href.starts_with("t3-skill:")
            || thread_link(&token.href)
        {
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
fn block_gap(
    previous: Option<&markdown_parse::Block>,
    block: &markdown_parse::Block,
    index: usize,
) -> f64 {
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

    // Ported markdownLinks.test.ts: isMarkdownFileLinkLabel, "classifies %s for %s".
    #[test]
    #[allow(non_snake_case)]
    fn isMarkdownFileLinkLabel_classifies_labels() {
        for (label, href, expected) in [
            ("validates the input", "/repo/src/example.ts:12", false),
            ("read src/example.ts", "/repo/src/example.ts:12", false),
            ("example.ts?why this matters", "/repo/src/example.ts", false),
            ("example.ts", "/repo/src/example.ts:12", true),
            ("example.ts:12", "/repo/src/example.ts:12", true),
            ("example.ts:99", "/repo/src/example.ts:12", false),
            ("example.ts:12:2", "/repo/src/example.ts:12:2", true),
            ("example.ts:12:3", "/repo/src/example.ts:12:2", false),
            ("example.ts:12", "/repo/src/example.ts", false),
            ("src/example.ts:12", "/repo/src/example.ts:12", true),
            ("./src/example.ts", "/repo/src/example.ts", true),
            ("/repo/src/example.ts", "/repo/src/example.ts", true),
            ("src/", "/home/me/project/src/", true),
            ("EXAMPLE.TS", "C:/repo/src/example.ts:12", true),
            ("file name.ts", "file:///repo/file%20name.ts", true),
            ("", "/repo/src/example.ts", true),
        ] {
            assert_eq!(
                isMarkdownFileLinkLabel(label, href),
                expected,
                "{label} for {href}"
            );
        }
    }

    fn document_runs(doc: &Value) -> Vec<&[Value]> {
        let Value::Record(doc) = doc else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else {
            panic!("blocks")
        };
        blocks
            .iter()
            .flat_map(|block| {
                let Value::Record(fields) = block else {
                    panic!("block")
                };
                let Value::List(runs) = &fields[7] else {
                    panic!("runs")
                };
                runs.iter().map(|run| {
                    let Value::Record(fields) = run else {
                        panic!("run")
                    };
                    fields.as_ref()
                })
            })
            .collect()
    }

    // Clone-authored connected parse/render data tests, including source copy.
    #[test]
    fn prose_file_labels_survive_beside_one_chip() {
        let doc = document(
            Value::str("a"),
            "[parser](src/a.ts:3) [a.ts](src/a.ts) [src/a.ts:3](src/a.ts:3)",
        );
        let runs = document_runs(&doc);
        assert_eq!(
            runs.iter()
                .filter(|r| r[6].as_str() == Some("file"))
                .count(),
            3
        );
        assert_eq!(
            runs.iter()
                .filter(|r| r[1].as_str() == Some("parser "))
                .count(),
            1
        );
        assert!(!runs.iter().any(|r| r[1].as_str() == Some("src/a.ts:3 ")));
        let parsed =
            markdown_parse::parse("[parser](src/a.ts:3)", &|href| format!("{FILE_LINK}{href}"));
        assert_eq!(
            cell_markdown(&parsed.blocks[0].runs),
            "[parser](src/a.ts:3)"
        );
    }

    #[test]
    fn known_skills_render_outside_code_and_links_and_keep_copy_source() {
        let skills = Value::list(vec![Value::record(vec![
            Value::str("review-code"),
            Value::str("Review code"),
        ])]);
        let doc = document_with_skills(
            Value::str("m"),
            "$review-code $5 $1k $unknown `$review-code` [$review-code](https://example.com)\n\n```text\n$review-code\n```",
            Some(&skills),
        );
        let runs = document_runs(&doc);
        let chips: Vec<_> = runs
            .iter()
            .filter(|r| r[6].as_str() == Some("context-skill"))
            .collect();
        assert_eq!(chips.len(), 1);
        assert_eq!(chips[0][1].as_str(), Some("Review code"));
        assert_eq!(chips[0][5].as_str(), Some("t3-skill:$review-code"));
        assert!(runs
            .iter()
            .any(|r| r[1].as_str() == Some("$review-code") && r[4] == Value::Bool(true)));
        assert!(runs.iter().any(|r| r[1].as_str() == Some("$review-code")
            && r[5].as_str() == Some("https://example.com")));
        let parsed = markdown_parse::parse("Use $review-code now", &|href| href.into());
        let runs = skill_runs(
            &parsed.blocks[0].runs,
            &[("review-code".into(), "Review code".into())],
        );
        assert_eq!(cell_markdown(&runs), "Use $review-code now");
        assert_eq!(cell_plain(&runs), "Use $review-code now");
        assert!(!document_runs(&document(Value::str("m"), "$review-code"))
            .iter()
            .any(|r| r[6].as_str() == Some("context-skill")));
    }

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
        let doc =
            markdown_parse::parse("`a.md` changed  in **the** repo.", &|href| href.to_string());
        let tokens = flow_tokens(&doc.blocks[0]).expect("code makes a flow block");
        let texts: Vec<_> = tokens
            .iter()
            .map(|run| (run.text.as_str(), run.code, run.bold))
            .collect();
        assert_eq!(
            texts,
            vec![
                ("a.md", true, false),
                (" ", false, false),
                ("changed ", false, false),
                ("in ", false, false),
                ("the", false, true),
                (" ", false, false),
                ("repo.", false, false)
            ]
        );
        // Spaces collapse across a chip as a browser collapses them: "md]  please" keeps one.
        let doc = markdown_parse::parse(
            "Check [a.md](t3-context://v1/mention/m1)  please",
            &|href| href.to_string(),
        );
        let texts: Vec<String> = flow_tokens(&doc.blocks[0])
            .expect("a chip flows")
            .iter()
            .map(|run| run.text.clone())
            .collect();
        assert_eq!(texts, vec!["Check ", "a.md", " ", "please"]);
        let plain = markdown_parse::parse("No code here.", &|href| href.to_string());
        assert!(flow_tokens(&plain.blocks[0]).is_none());
    }

    #[test]
    fn file_links_are_chips_and_web_links_lead_with_their_favicon() {
        let Value::Record(doc) = document(
            Value::str("a"),
            "See [the result](fixture-result.md), [pkg](./package.json:3) and [the reference](https://example.com/docs).",
        ) else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else {
            panic!("blocks")
        };
        let Value::Record(block) = &blocks[0] else {
            panic!("block")
        };
        assert_eq!(block[10], Value::Bool(true));
        let Value::List(runs) = &block[7] else {
            panic!("runs")
        };
        let runs: Vec<(String, String, String, String)> = runs
            .iter()
            .map(|run| {
                let Value::Record(fields) = run else {
                    panic!("run")
                };
                let text = |at: usize| fields[at].as_str().unwrap_or("").to_string();
                (text(1), text(5), text(6), text(7))
            })
            .filter(|run| !run.2.is_empty())
            .collect();
        assert_eq!(
            runs,
            vec![
                (
                    "fixture-result.md".into(),
                    "t3-file:fixture-result.md".into(),
                    "file".into(),
                    "markdown".into()
                ),
                (
                    "package.json \u{b7} L3".into(),
                    "t3-file:./package.json:3".into(),
                    "file".into(),
                    "npm-plain".into()
                ),
                (
                    "the ".into(),
                    "https://example.com/docs".into(),
                    "link-start".into(),
                    "".into()
                ),
                (
                    "reference".into(),
                    "https://example.com/docs".into(),
                    "link".into(),
                    "".into()
                ),
            ]
        );
        assert_eq!(pierre_icon_token("src/app.tsx"), "react");
        assert_eq!(pierre_icon_token("Cargo.toml"), "toml");
    }

    /// A web link's own menu is dispatched per node (FlowRuns), not per inline run (ChatRuns), so
    /// every prose block holding a link, with no code or chip beside it, is laid out word by word.
    #[test]
    fn a_prose_block_with_only_a_web_link_flows_so_its_link_is_a_node() {
        for text in [
            "See [the docs](https://example.com/docs) today.",
            "- an item with [the docs](https://example.com/docs)",
            "> a quote with [the docs](https://example.com/docs)",
            "## A heading with [the docs](https://example.com/docs)",
            "Mail [me](mailto:a@example.com) or read [the docs](https://example.com/docs).",
        ] {
            let Value::Record(doc) = document(Value::str("a"), text) else {
                panic!("document")
            };
            let Value::List(blocks) = &doc[1] else {
                panic!("blocks")
            };
            let Value::Record(block) = &blocks[0] else {
                panic!("block")
            };
            assert_eq!(block[10], Value::Bool(true), "{text}");
            let Value::List(runs) = &block[7] else {
                panic!("runs")
            };
            let links: Vec<(String, String)> = runs
                .iter()
                .filter_map(|run| {
                    let Value::Record(fields) = run else {
                        return None;
                    };
                    let href = fields[5].as_str().unwrap_or("");
                    (href == "https://example.com/docs").then(|| {
                        (
                            fields[1].as_str().unwrap_or("").to_string(),
                            fields[6].as_str().unwrap_or("").to_string(),
                        )
                    })
                })
                .collect();
            assert_eq!(
                links,
                vec![
                    ("the ".to_string(), "link-start".to_string()),
                    ("docs".to_string(), "link".to_string())
                ],
                "{text}"
            );
        }
        // A bare URL (GFM's autolink) is one link-start run in a flow block too.
        let Value::Record(doc) = document(Value::str("a"), "Read https://example.com/docs today.")
        else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else {
            panic!("blocks")
        };
        let Value::Record(block) = &blocks[0] else {
            panic!("block")
        };
        assert_eq!(block[10], Value::Bool(true));
        let Value::List(runs) = &block[7] else {
            panic!("runs")
        };
        assert!(runs.iter().any(|run| matches!(run, Value::Record(fields)
            if fields[5].as_str() == Some("https://example.com/docs") && fields[6].as_str() == Some("link-start"))));
    }

    #[test]
    fn user_text_keeps_line_breaks_and_draws_context_references_as_chips() {
        let Value::Record(doc) = document(
            Value::str("u"),
            &line_breaks(
                "First line\nsecond [notes.md](t3-context://v1/file/file_1) and [Assistant quote](t3-citation://v1/e/t/m?text=Quoted%20bit&start=0&end=9)\n\n- one\n- two",
            ),
        ) else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else {
            panic!("blocks")
        };
        assert_eq!(blocks.len(), 4);
        let line = |at: usize| {
            let Value::Record(block) = &blocks[at] else {
                panic!("block")
            };
            let Value::List(runs) = &block[7] else {
                panic!("runs")
            };
            let runs: Vec<(String, String, String)> = runs
                .iter()
                .map(|run| {
                    let Value::Record(fields) = run else {
                        panic!("run")
                    };
                    let text = |at: usize| fields[at].as_str().unwrap_or("").to_string();
                    (text(1), text(6), text(7))
                })
                .collect();
            (
                block[0].as_str().unwrap_or("").to_string(),
                block[9].as_number().unwrap(),
                runs,
            )
        };
        assert_eq!(
            line(0),
            (
                "0.0".into(),
                0.0,
                vec![
                    ("First ".into(), "".into(), "".into()),
                    ("line".into(), "".into(), "".into())
                ]
            )
        );
        assert_eq!(
            line(1),
            (
                "0.1".into(),
                0.0,
                vec![
                    ("second ".into(), "".into(), "".into()),
                    ("notes.md".into(), "context-file".into(), "markdown".into()),
                    (" ".into(), "".into(), "".into()),
                    ("and ".into(), "".into(), "".into()),
                    ("Quoted bit".into(), "context-citation".into(), "".into()),
                ]
            )
        );
        assert_eq!(
            line_breaks("# Title\nText\n| a |\n|---|"),
            "# Title\nText\n| a |\n|---|"
        );
    }

    #[test]
    fn pull_request_text_breaks_single_newlines_only_where_its_host_does() {
        let body = |id: &str, kind: &str| {
            Value::record(vec![
                Value::str(id),
                Value::str(kind),
                Value::str(""),
                Value::str("first line\nsecond line"),
            ])
        };
        let Value::Record(result) = Markdown
            .query(
                "renderPullRequestMarkdown",
                &[Value::list(vec![
                    body("github", "breaks"),
                    body("gitlab", "assistant"),
                ])],
            )
            .unwrap()
        else {
            panic!("document record")
        };
        let Value::List(documents) = &result[0] else {
            panic!("document list")
        };
        let blocks = |at: usize| {
            let Value::Record(document) = &documents[at] else {
                panic!("document")
            };
            let Value::List(blocks) = &document[1] else {
                panic!("blocks")
            };
            blocks.len()
        };
        assert_eq!((blocks(0), blocks(1)), (2, 1));
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

    #[test]
    fn an_image_with_a_fragment_source_loads_nothing() {
        // A fragment stays a link's href, but an image's `#…` source loads nothing, as an emptied one
        // (media-views.ts markdownImageHref).
        let Value::Record(doc) = document(
            Value::str("m"),
            "![a](#shot)\n\n![b](https://example.com/b.png)\n\n![c](javascript:x)",
        ) else {
            panic!("document")
        };
        let Value::List(blocks) = &doc[1] else {
            panic!("blocks")
        };
        let images: Vec<(&str, &str)> = blocks
            .iter()
            .filter_map(|block| match block {
                Value::Record(fields) if fields[1].as_str() == Some("image") => {
                    Some((fields[4].as_str()?, fields[5].as_str()?))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            images,
            vec![("a", ""), ("b", "https://example.com/b.png"), ("c", "")]
        );
    }
}

#[cfg(test)]
mod terminal_fence_tests {
    use super::*;
    #[test]
    fn matching_closed_and_open_identical_commands_stay_distinct() {
        assert_eq!(
            closed_code_fences("```bash\necho hi\n```\n\n```bash\necho hi\n"),
            vec![true, false]
        );
        assert_eq!(closed_code_fences("````bash\necho hi\n```"), vec![false]);
        let doc = document(
            Value::str("m"),
            "```bash\necho hi\n```\n\n```bash\necho hi\n",
        );
        let Value::Record(document) = doc else {
            panic!("document")
        };
        let Value::List(blocks) = &document[1] else {
            panic!("blocks")
        };
        let flags: Vec<bool> = blocks
            .iter()
            .filter_map(|block| {
                let Value::Record(fields) = block else {
                    return None;
                };
                if fields[1].as_str() != Some("code") {
                    return None;
                }
                Some(fields[15] == Value::Bool(true))
            })
            .collect();
        assert_eq!(flags, vec![true, false]);
    }
}
