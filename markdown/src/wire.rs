//! UTF-16 styling, edits and selection facts, with identical JSON on each host.

use crate::{Command, Range};

/// Serialize styling. A selection reveals every marker in its touched paragraphs.
pub fn style(source: &str, reveal: Option<Range>) -> String {
    let styled = crate::style(source, reveal);
    let mut json = String::from("{\"p\":[");
    for (n, p) in styled.paragraphs.iter().enumerate() {
        use crate::ParagraphKind as K;
        let (kind, level) = match &p.kind {
            K::Body => (0, 0),
            K::Heading(l) => (1, *l),
            K::Bullet => (2, 0),
            K::Ordered => (3, 0),
            K::Task(done) => (4, u8::from(*done)),
            K::Rule => (5, 0),
            K::Fence => (6, 0),
            K::Code(_) => (7, 0),
            K::Footnote => (8, 0),
            K::Table => (9, 0),
            K::Embed => (10, 0),
            K::Image => (11, 0),
            K::Video => (12, 0),
        };
        if n > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "[{},{},{kind},{level},{},{}]",
            p.range.start, p.range.end, p.depth, p.quote
        ));
    }
    json.push_str("],\"s\":[");
    for (n, sp) in styled.spans.iter().enumerate() {
        if n > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "[{},{},{},",
            sp.range.start, sp.range.end, sp.style
        ));
        json_string(&mut json, &sp.href);
        json.push(']');
    }
    json.push_str("],\"h\":[");
    for (n, h) in styled.hidden.iter().enumerate() {
        if n > 0 {
            json.push(',');
        }
        json.push_str(&format!("[{},{}]", h.start, h.end));
    }
    json.push_str("],\"r\":[");
    for (n, r) in styled.replaced.iter().enumerate() {
        use crate::Replacement as R;
        let (kind, text) = match &r.with {
            R::Bullet => (0, String::new()),
            R::TaskBox(false) => (1, String::new()),
            R::TaskBox(true) => (2, String::new()),
            R::Rule => (3, String::new()),
            R::Footnote(t) => (4, t.clone()),
        };
        if n > 0 {
            json.push(',');
        }
        json.push_str(&format!("[{},{},{kind},", r.range.start, r.range.end));
        json_string(&mut json, &text);
        json.push(']');
    }
    json.push_str("]}");
    json
}

fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parse the shared toolbar command vocabulary; arguments are literal strings.
pub fn command(name: &str, argument: &str) -> Option<Command> {
    Some(match name {
        "bold" => Command::Bold,
        "italic" => Command::Italic,
        "code" => Command::Code,
        "strike" => Command::Strike,
        "link" => Command::Link(argument.into()),
        "heading" => {
            let n = argument.parse::<u8>().ok()?;
            if !(1..=6).contains(&n) {
                return None;
            }
            Command::Heading(n)
        }
        "bullet" => Command::Bullet,
        "ordered" => Command::Ordered,
        "task" => Command::Task,
        "quote" => Command::Quote,
        "codeblock" => Command::CodeBlock,
        "footnote" => Command::Footnote,
        "figure" => Command::Figure(argument.into()),
        "indent" => Command::Indent,
        "outdent" => Command::Outdent,
        "toggleTask" => Command::ToggleTask,
        "newline" => Command::Newline,
        _ => return None,
    })
}

/// Serialize source-relative UTF-16 replacements and the resulting selection.
/// Hosts apply the replacements in one undo transaction, from last to first.
pub fn edit(source: &str, selection: Range, name: &str, argument: &str) -> String {
    let Some(command) = command(name, argument) else {
        return r#"{"error":"unknown command or invalid argument"}"#.into();
    };
    if crate::selection(source, selection)
        .unavailable
        .split_whitespace()
        .any(|s| s == name)
    {
        return r#"{"error":"command unavailable in a code block"}"#.into();
    }
    let edit = crate::edit(source, selection, command);
    let mut json = String::from("{\"replacements\":[");
    for (n, (range, text)) in edit.replacements.iter().enumerate() {
        if n > 0 {
            json.push(',');
        }
        json.push_str(&format!("[{},{},", range.start, range.end));
        json_string(&mut json, text);
        json.push(']');
    }
    json.push_str(&format!(
        "],\"selection\":[{},{}]}}",
        edit.selection.start, edit.selection.end
    ));
    json
}

/// Serialize the toolbar facts carried by the `select` event.
pub fn selection(source: &str, range: Range) -> String {
    let state = crate::selection(source, range);
    let mut json = String::from("{\"formats\":");
    json_string(&mut json, &state.formats);
    json.push_str(&format!(",\"mixed\":{},\"link\":", state.mixed));
    json_string(&mut json, &state.link);
    json.push_str(",\"unavailable\":");
    json_string(&mut json, &state.unavailable);
    json.push('}');
    json
}
