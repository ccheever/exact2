//! The refusal of a handler whose action takes the wrong number of
//! parameters spells the declaration that fits (@ref LLP 1088 D7.1,
//! pomodoro F1): what the call site passes, then what the event appends,
//! each payload named after the DOM property it carries and typed by the
//! element. Analysis and lowering print it through this one function.

use contract_syntax::Expr;
use contract_types::Ty;

/// What `event` appends on this element, as `(name, type, said)`: `said`
/// is how the message describes a single payload.
fn payload(event: &str, control: Option<&str>) -> Vec<(&'static str, &'static str, &'static str)> {
    let one = |name, ty, said| vec![(name, ty, said)];
    let numbers = |names: &[&'static str]| names.iter().map(|n| (*n, "number", "")).collect();
    match (event, control) {
        ("change" | "input", Some("checkbox")) => {
            one("checked", "bool", "the checkbox's new `checked`")
        }
        ("change" | "input", Some("radio")) => one("value", "string", "the radio's `value`"),
        ("change" | "input", Some("range")) => one("value", "number", "the range's new `value`"),
        ("change" | "input", Some("file")) => one("files", "list<Picked>", "the picked `files`"),
        ("change" | "input", Some("select")) => one("value", "string", "the select's new `value`"),
        ("change" | "input", _) => one("value", "string", "the field's new `value`"),
        ("key", _) => one("key", "string", "the key's name, `key`"),
        ("message", _) => one("data", "string", "the message's `data`"),
        ("error", _) => one("message", "string", "the error's `message`"),
        ("hover", _) => one("hovered", "bool", "whether the pointer is over, `hovered`"),
        ("timeupdate", _) => one("currentTime", "number", "the media's `currentTime`"),
        ("durationchange", _) => one("duration", "number", "the media's `duration`"),
        ("select", Some("field")) => one("event", "InputEvent", "the field's `InputEvent`"),
        ("fullscreenchange", _) => {
            one("fullscreen", "bool", "whether the media is now full screen")
        }
        ("select", _) => one("selection", "MarkdownSelection", "the editor's `selection`"),
        ("navigate", _) => one("location", "string", "the location"),
        ("scroll", _) => numbers(&["scrollLeft", "scrollTop"]),
        ("resize", _) => numbers(&["width", "height"]),
        ("pan", _) => numbers(&["dx", "dy"]),
        ("panrelease", _) => numbers(&["vx", "vy"]),
        ("heightrelease", _) => numbers(&["height", "velocity"]),
        ("transformgeometry", _) => numbers(&["bw", "bh", "pw", "ph"]),
        ("transformrelease", _) => numbers(&["x", "y", "scale", "vx", "vy", "vscale"]),
        ("reorderdrop", _) => vec![("item", "string", ""), ("before", "option<string>", "")],
        _ => Vec::new(),
    }
}

/// An argument as written, when it is short enough to quote.
fn written(e: &Expr) -> Option<String> {
    Some(match e {
        Expr::Ident(n, _) => n.split(['#', '@']).next().unwrap_or(n).to_string(),
        Expr::Member(inner, field, _) => format!("{}.{field}", written(inner)?),
        Expr::Number(n, _) => n.to_string(),
        Expr::Str(s, _) => format!("{s:?}"),
        Expr::Bool(b, _) => b.to_string(),
        _ => return None,
    })
}

/// The refusal's text. `control` is [`contract_syntax::input_control`] of
/// the element; `params` the action's declared parameters with their types
/// as far as they are known; `arg_types` the call site's arguments' types.
pub fn handler_arity_message(
    event: &str,
    control: Option<&str>,
    name: &str,
    args: &[Expr],
    params: &[(String, Ty)],
    arg_types: &[Option<Ty>],
) -> String {
    let name = name.split('#').next().unwrap_or(name);
    let quoted: Vec<String> = args
        .iter()
        .map(|a| written(a).map_or("…".into(), |w| format!("`{w}`")))
        .collect();
    let site = match args.iter().map(written).collect::<Option<Vec<_>>>() {
        Some(w) if !args.is_empty() => format!("`{event}={name}({})`", w.join(", ")),
        Some(_) => format!("`{event}={name}`"),
        None => format!("`{event}={name}(…)`"),
    };
    let appended = payload(event, control);
    let then = match appended.as_slice() {
        [] => None,
        [(_, ty, said)] => Some(format!("{said} ({ty})")),
        many => Some(
            many.iter()
                .map(|(n, ty, _)| format!("`{n}` ({ty})"))
                .collect::<Vec<_>>()
                .join(", "),
        ),
    };
    // The event's record, the action's to take or leave
    // (`contract_types::event_record`).
    let record = contract_types::event_record(event);
    let calls = match (quoted.is_empty(), &then) {
        (true, None) => format!("{site} calls `{name}` with nothing"),
        (true, Some(then)) => format!("{site} calls `{name}` with {then}"),
        (false, None) => format!(
            "{site} calls `{name}` with {} and nothing more",
            quoted.join(", ")
        ),
        (false, Some(then)) => format!(
            "{site} calls `{name}` with {} and then {then}",
            quoted.join(", ")
        ),
    };
    let mut declared: Vec<String> = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        let param = params.get(i);
        let pname = param.map(|(n, _)| n.clone()).unwrap_or_else(|| {
            written(arg)
                .and_then(|w| w.rsplit('.').next().map(str::to_string))
                .filter(|w| w.chars().next().is_some_and(char::is_alphabetic))
                .unwrap_or_else(|| format!("arg{}", i + 1))
        });
        let ty = param
            .map(|(_, t)| t)
            .filter(|t| t.is_complete())
            .or(arg_types
                .get(i)
                .and_then(Option::as_ref)
                .filter(|t| t.is_complete()))
            .map_or("<type>".into(), Ty::to_string);
        declared.push(format!("{pname}: {ty}"));
    }
    declared.extend(appended.iter().map(|(n, ty, _)| format!("{n}: {ty}")));
    let declare = |declared: &[String]| {
        if declared.is_empty() {
            format!("`action {name}`")
        } else {
            format!("`action {name}({})`", declared.join(", "))
        }
    };
    let mut declaration = declare(&declared);
    if let Some(record) = record {
        declared.push(format!("event: {record}"));
        declaration = format!(
            "{declaration}, or {} for its `{record}`",
            declare(&declared)
        );
    }
    if event == "navigate" {
        return format!(
            "`navigate={name}` passes no arguments of its own, then the location (string) when the action takes one; declare `action {name}` or `action {name}(location: string)`"
        );
    }
    format!("{calls}; declare {declaration}")
}
