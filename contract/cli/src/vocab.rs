//! `contract vocab [--json] [<name> …]` — what the compiler admits: every tag,
//! every attribute by kind (a style's rows with codec, values and default; a
//! prop's type; a handler), the renamed spellings, and the rules no lookup
//! lists, the host commands an action states, the standard functions:
//! each one's signature, rules and example (LLP 1116.000 D5), and the
//! idioms a builder looks up by what they want (`plural`, LLP 1116.004 D4). @ref LLP 1086
//! D3. The list is `contract_lower::vocab`'s, which keeps only what the live
//! lookups admit; the commands are `contract_syntax::HOST_COMMANDS`; the
//! functions are `contract::functions`', the roster's signatures beside the
//! grammar's tables. Several names are one call, each entry in turn (LLP
//! 1116.001 D4.3), so a build's lookups are one step. A concept (`scroll`,
//! `plural`, `choice`, `modal`) answers with its whole idiom, after the
//! entries of a tag or attribute of the same name (`concepts.md`, LLP
//! 1116.004 D5.1).

mod card;
mod concepts;

use concepts::Concept;
use contract::functions::{self, Function};
use contract_lower::tags::{self, AttrTarget, Tag};
use contract_lower::vocab;
use contract_syntax::HOST_COMMANDS;
use exact_kernel::{PropId, StyleId};
use serde_json::{json, Value};
use std::process::ExitCode;

const USAGE: &str = "usage: contract vocab [--json] [--all] [<name> …]\n  no name: the card, what an app writes, in one read; --all: every tag, attribute and function\n  names: each one's entry in turn (exit 1 if one is unknown); --json: everything, as JSON";

pub fn run(args: &[String]) -> ExitCode {
    let mut json = false;
    let mut all = false;
    let mut names = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => json = true,
            "--all" => all = true,
            // A vendor-prefixed name (`-exact-fit`, `-webkit-line-clamp`) is a name, not a flag.
            _ if !arg.starts_with('-') || (arg.len() > 2 && !arg.starts_with("--")) => {
                names.push(arg.as_str())
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match names.as_slice() {
        [] if json => {
            println!("{:#}", everything());
            ExitCode::SUCCESS
        }
        [] if all => {
            print!("{}", listing());
            ExitCode::SUCCESS
        }
        [] => {
            print!("{}", card::card());
            ExitCode::SUCCESS
        }
        [name] if json => {
            let (doc, known) = one_json(name);
            println!("{doc:#}");
            exit(known)
        }
        // Several names: each entry in order, a refusal in its place, and exit 1
        // when any name is unknown (LLP 1116.001 D4.3). JSON is an array of the
        // entries one name prints.
        _ if json => {
            let (docs, known): (Vec<Value>, Vec<bool>) = names.iter().map(|n| one_json(n)).unzip();
            println!("{:#}", Value::Array(docs));
            exit(known.iter().all(|k| *k))
        }
        _ => {
            let mut known = true;
            // A concept two names share prints once, at the first.
            let mut shown = Vec::new();
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                known &= one_text(name, &mut shown);
            }
            exit(known)
        }
    }
}

/// Exit 0 when every name was known, else 1.
fn exit(known: bool) -> ExitCode {
    if known {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// One name's entries, for its text and its JSON: its tag and attribute
/// (`input` is both), the style attributes it is a value of (`tabular-nums`),
/// an open set's rule, a host command, its function (`filter` is an attribute
/// and a function); `None` when nothing admits it.
struct Entry {
    tag: Option<Value>,
    attr: Option<Value>,
    value_of: Vec<&'static str>,
    open: Option<&'static str>,
    command: Option<String>,
    function: Option<Function>,
    part: Option<Part>,
    concept: Option<Concept>,
}

/// A component of a built-in library (`exact:content`'s `RoundButton`), read
/// from its source: its props, the comment above it (LLP 1116.003 D4), and the
/// call that comment ends with (`// Call: EmptyState(…)`, LLP 1116.010 D3): one
/// that works as written, which a test compiles for every part, so it cannot
/// drift from the props.
struct Part {
    module: &'static str,
    props: Vec<String>,
    about: String,
    call: Option<String>,
}

const PARTS: &[(&str, &str)] = &[("exact:content", include_str!("../../lib/content.contract"))];

fn part(name: &str) -> Option<Part> {
    PARTS.iter().find_map(|(module, source)| {
        let lines: Vec<&str> = source.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.strip_prefix("component ") == Some(name))?;
        let first = lines[..at]
            .iter()
            .rposition(|l| !l.starts_with("//"))
            .map_or(0, |i| i + 1);
        let comment: Vec<&str> = lines[first..at]
            .iter()
            .map(|l| l.trim_start_matches('/').trim())
            .collect();
        let call = comment
            .iter()
            .find_map(|l| l.strip_prefix("Call:"))
            .map(|c| c.trim().to_owned());
        let about: Vec<&str> = comment
            .into_iter()
            .filter(|l| !l.starts_with("Call:"))
            .collect();
        let mut props = Vec::new();
        let mut rest = lines[at + 1..].iter().skip_while(|l| l.trim() != "props");
        if rest.next().is_some() {
            props = rest
                .take_while(|l| l.starts_with("    "))
                .map(|l| l.trim().to_owned())
                .collect();
        }
        Some(Part {
            module,
            props,
            about: about.join(" "),
            call,
        })
    })
}

fn entry(name: &str) -> Option<Entry> {
    let tag = tags::tag(name).map(|t| tag_json(name, &t));
    let attr = tags::attr(name).map(|a| attr_json(name, &a));
    let value_of: Vec<&'static str> = vocab::attrs()
        .into_iter()
        .filter(|(n, a)| rows_of(n, a).iter().any(|r| r.enum_names().contains(&name)))
        .map(|(n, _)| n)
        .collect();
    let open =
        vocab::open_set(name).filter(|_| tag.is_none() && attr.is_none() && value_of.is_empty());
    let command = HOST_COMMANDS.contains(&name).then(|| command_text(name));
    let function = functions::function(name);
    let part = part(name);
    let concept = concepts::concept(name);
    let known = tag.is_some()
        || attr.is_some()
        || !value_of.is_empty()
        || open.is_some()
        || command.is_some()
        || function.is_some()
        || part.is_some()
        || concept.is_some();
    known.then_some(Entry {
        tag,
        attr,
        value_of,
        open,
        command,
        function,
        part,
        concept,
    })
}

/// The compiler's refusal of a name nothing admits.
fn refusal(name: &str) -> String {
    idiom(name).unwrap_or_else(|| refused(name))
}

/// One name as JSON, and whether it was known.
fn one_json(name: &str) -> (Value, bool) {
    let Some(Entry {
        tag,
        attr,
        value_of,
        open,
        command,
        function,
        part,
        concept,
    }) = entry(name)
    else {
        return (json!({ "name": name, "refused": refusal(name) }), false);
    };
    let mut doc = json!({ "name": name });
    if let Some(tag) = tag {
        doc["tag"] = tag;
    }
    if let Some(attr) = attr {
        doc["attribute"] = attr;
    }
    if !value_of.is_empty() {
        doc["valueOf"] = value_of.into();
    }
    if let Some(open) = open {
        doc["note"] = open.into();
    }
    if let Some(command) = command {
        doc["command"] = command.into();
    }
    if let Some(f) = &function {
        doc["function"] = function_json(f);
    }
    if let Some(p) = part {
        doc["part"] =
            json!({ "module": p.module, "props": p.props, "about": p.about, "call": p.call });
    }
    if let Some(c) = concept {
        // A name the web has and Contract refuses (`every`) says so beside it.
        if function.is_none() {
            if let Some(note) = idiom(name) {
                doc["refusal"] = note.into();
            }
        }
        doc["concept"] = concept_json(&c);
    }
    (doc, true)
}

/// One name as text, printed (a refusal on stderr), and whether it was
/// known; a concept already in `shown` is named, not printed again.
fn one_text(name: &str, shown: &mut Vec<&'static str>) -> bool {
    let Some(Entry {
        tag,
        attr,
        value_of,
        open,
        command,
        function,
        part,
        concept,
    }) = entry(name)
    else {
        eprintln!("{}", refusal(name));
        return false;
    };
    // Nothing but the concept: a name the web has and Contract refuses
    // (`every`) says so first; anything else is printed above the concept.
    let only = tag.is_none()
        && attr.is_none()
        && value_of.is_empty()
        && open.is_none()
        && command.is_none()
        && function.is_none()
        && part.is_none();
    if let Some(p) = part {
        println!(
            "{name}: a part of `{}` (`use {name} from \"{}\"`)",
            p.module, p.module
        );
        println!("  props {}", p.props.join(", "));
        println!("  {}", p.about);
        if let Some(call) = &p.call {
            println!("  call {call}");
        }
    }
    if let Some(t) = tags::tag(name) {
        println!("{name}: tag, {}", tag_text(&t));
        if let Some(note) = vocab::tag_note(name) {
            println!("  {note}");
        }
    }
    if let Some(a) = tags::attr(name) {
        println!("{name}: {} attribute", kind(&a));
        for line in attr_detail(name, &a) {
            println!("  {line}");
        }
        if let Some(only) = only_on(name) {
            println!("  only on {only}");
        }
        if matches!(name, "min" | "max" | "step") {
            println!("  on an `input` a number of either sign is written as its text: `min=-40`, `step=0.5`, `max=1e6` (LLP 1116.010 D3); a decimal pad (`inputmode=\"decimal\"`) with a negative `min` has the ± key");
        }
        if name == "markup" {
            println!("  \"markdown\" or \"none\" (the default): a `text` reads its string as Markdown, a `textarea` edits it (LLP 1045)");
        }
        if name == "resize" {
            println!("  given an action, the element resize event: ResizeObserver's, after layout, with the content box's width and height and its `DOMRectReadOnly`");
        }
        if name == "title" {
            println!("  on `head`, the document's title; elsewhere HTML's advisory text, the platform's tooltip (prop title, str)");
        }
        if name == "navigationBar" {
            println!("  on a route (LLP 1115.000 D5.2). \"none\": the screen has no navigation bar on iOS, a full-bleed screen that draws its own chrome and way back (a photo viewer, a header laid out by hand); its `header` paints as written and the edge swipe still goes back. \"auto\", the default, is the platform's bar: the route's `header` when it is one (`contract vocab header`), else the system Back button alone on a pushed route, and no bar on a stack's first route. A `navigationBack` control drawn in a pushed route's page without it is a second Back (warning `lower-back-twice`). On the navigation root or off a route it is refused. docs/recipes/navigation-stack.md");
        }
        if name == "navigationSplit" {
            println!("  on a stack's root route (LLP 1116.004 D1.2). A wide window (900 px and up on the web) shows a stack whose root route is a list whose rows push beside its top, as a split view does, with nothing to write: a row pushes when it draws the disclosure, an `image \"symbol:forward-chevron\"` (the chevron an iOS list draws) in a `link`, or in a `button` or pressable whose action writes the router (`push`, `open`, `go`); a chevron on a row that only selects is decoration, and its route is left unsplit, `navigationSplit=\"none\"` said for you. The list is a 320-wide column, its chevrons hidden and the row whose press pushed the detail marked; the top route is the detail beside it (a detail a row pushed has no Back), a `navigationPresentation=\"modal\"` route stays a sheet, and a phone pushes. The router is unchanged: a push from a row while a detail shows stacks it, so `replace` there to swap the detail. \"none\" keeps the stack pushed; a number is the column's width and splits a list the rule does not find. While nothing is selected the detail pane shows the platform's placeholder (\"No Selection\", centred, in the secondary label's colour); \"placeholder\" on a box in the root route (an `EmptyState`) is your own instead, shown only there and then and laid out nowhere else. A root `tablist` left as the tabs recipe places it is a sidebar on the same window. docs/recipes/navigation-stack.md");
        }
        if name == "-exact-fit" {
            println!("  on a `text`: \"shrink\" draws a line too wide for its box smaller until it fits, down to half its size, as UIKit's adjustsFontSizeToFitWidth (a timer's readout at the largest text size); with `white-space=\"nowrap\"` the whole line fits, else its longest word (LLP 1116.001 D1.4)");
        }
    }
    if !value_of.is_empty() {
        let of: Vec<String> = value_of.iter().map(|n| format!("`{n}`")).collect();
        println!(
            "{name}: a value of {} (`contract vocab {}` lists the others)",
            of.join(", "),
            value_of[0]
        );
    }
    if let Some(open) = open {
        // A hyphenated name is usually a CSS property Contract lacks (r26 t1 read
        // `outline-width` as a module): say that first.
        if open == vocab::MODULE_NOTE {
            println!("{name}: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; {open}");
        } else {
            println!("{name}: {open}");
        }
    }
    if let Some(command) = command {
        println!("{name}: {command}");
    }
    if let Some(f) = &function {
        print!("{}", function_text(f));
    }
    if let Some(c) = concept {
        if only {
            if let Some(note) = idiom(name) {
                println!("{note}");
            }
        }
        if shown.contains(&c.name) {
            println!("{name}: the concept `{}`, above", c.name);
        } else {
            if !only {
                println!();
            }
            print!("{}", c.text());
            shown.push(c.name);
        }
    }
    true
}

fn concept_json(c: &Concept) -> Value {
    json!({
        "name": c.name,
        "title": c.title,
        "names": c.names,
        "answer": c.answer,
        "contract": c.contract(),
    })
}

/// Why the compiler refuses a name the web has (`sort`, `now`, `padStart`),
/// with what to write instead; `None` for any other.
fn idiom(name: &str) -> Option<String> {
    if name == "now" {
        return Some(contract_syntax::idioms::NOW_REFUSED.into());
    }
    contract_syntax::idioms::refusal(name)
}

/// A name nothing admits: the lookups' hints, and a function one edit away.
fn refused(name: &str) -> String {
    let refusal = vocab::refusal(name);
    let head = format!("`{name}` is not a tag or an attribute");
    let hints = refusal.strip_prefix(&head).unwrap_or(&refusal);
    let mut out = format!("`{name}` is not a tag, an attribute or a standard function{hints}");
    if let Some(near) = functions::functions()
        .into_iter()
        .find(|f| one_edit(name, &f.name))
    {
        out += &format!("; did you mean the function `{}`?", near.name);
    }
    out
}

/// Whether two names differ by one insertion, deletion, substitution or
/// swap of neighbours, or by case alone.
fn one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a == b {
        return false;
    }
    if a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| x.eq_ignore_ascii_case(y)) {
        return true;
    }
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let (ra, rb) = (&a[prefix..], &b[prefix..]);
    match ra.len() as isize - rb.len() as isize {
        0 => {
            ra[1..] == rb[1..]
                || (ra.len() >= 2 && ra[0] == rb[1] && ra[1] == rb[0] && ra[2..] == rb[2..])
        }
        1 => ra[1..] == *rb,
        -1 => *ra == rb[1..],
        _ => false,
    }
}

fn function_json(f: &Function) -> Value {
    let mut doc = json!({
        "name": f.name,
        "kind": if f.signature.is_some() { "standard function" } else { "intrinsic" },
        "signature": f.signature,
        "rules": f.rows.iter().map(|r| json!({"call": r.call, "rule": r.rule, "example": r.example})).collect::<Vec<_>>(),
    });
    if let Some(note) = idiom(&f.name) {
        doc["note"] = note.into();
    }
    if routed(f) {
        doc["routes"] = true.into();
    }
    doc
}

/// Whether the function exists only in an app with a `routes` declaration.
fn routed(f: &Function) -> bool {
    exact_plan::Stdlib::from_name(&f.name).is_some_and(contract_types::routes::needs_routes)
}

/// A function's entry: its signature, each row's call and rule, and its
/// example, then what the web's function of that name becomes.
fn function_text(f: &Function) -> String {
    let mut out = match &f.signature {
        Some(signature) => format!("{}: standard function\n  signature: {signature}\n", f.name),
        None => format!("{}: compiler intrinsic\n", f.name),
    };
    for row in &f.rows {
        out += &format!("  {}: {}\n", row.call, row.rule);
        if !row.example.is_empty() {
            out += &format!("    example: {}\n", row.example);
        }
    }
    if routed(f) {
        out += "  only in an app with a `routes` declaration\n";
    }
    if let Some(note) = idiom(&f.name) {
        out += &format!("  the web's `{}`: {note}\n", f.name);
    }
    out += "  docs/contract-grammar.md#standard-functions-and-intrinsics\n";
    out
}

/// A host command: an action statement, never a value.
fn command_text(name: &str) -> String {
    let what = match name {
        "showNotification" => "; `showNotification(title=…, body=…, tag=…, showTrigger=…, navigate=…)` posts a local notification; its plain click opens `navigate=` through the navigation root (warm event or cold launch location). Needs `device.notifications` (docs/reference.md#notifications)",
        "setRootFontSize" => "; `setRootFontSize(px)` sets CSS's `:root { font-size }`, the size every `rem` follows (px above 0), over the host's own; `setRootFontSize(\"medium\")` hands it back (LLP 1069.000 D3)",
        _ => "",
    };
    format!("host command, an action statement (docs/contract-grammar.md#host-commands){what}")
}

fn family(t: &Tag) -> &'static str {
    if t.node_type.name().starts_with("Svg") {
        "svg"
    } else {
        "html"
    }
}

fn kind(a: &AttrTarget) -> &'static str {
    match a {
        AttrTarget::Styles(_) => "style",
        AttrTarget::Prop(_) => "prop",
        AttrTarget::InvertedBoolProp(_) => "inverted-prop",
        AttrTarget::Handler(_) => "handler",
        AttrTarget::Flex => "flex",
        AttrTarget::Shorthand => "shorthand",
        AttrTarget::Surface => "surface",
        AttrTarget::MediaMetadata => "record",
    }
}

fn prop_type(p: PropId) -> String {
    format!("{:?}", tags::prop_ty(p)).to_lowercase()
}

/// What `metadata=` takes (LLP 1098 D1): the Media Session's record.
const METADATA: &str = "MediaMetadata(title=, artist=, album=, artwork=), each a string";

/// Where an attribute is admitted, if not on every tag.
fn only_on(name: &str) -> Option<&'static str> {
    // `title` is every element's, HTML's tooltip; `head`'s is the document's.
    if tags::HEAD_FIELDS.contains(&name) && name != "title" {
        return Some("`head`");
    }
    vocab::CONTEXTUAL
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, on)| *on)
}

/// The rows `flex: <n>` sets.
const FLEX_ROWS: [StyleId; 3] = [StyleId::FlexGrow, StyleId::FlexShrink, StyleId::FlexBasis];

fn rows_of(name: &str, a: &AttrTarget) -> &'static [StyleId] {
    match a {
        AttrTarget::Styles(rows) => rows,
        AttrTarget::Flex => &FLEX_ROWS,
        AttrTarget::Shorthand => vocab::shorthand_rows(name),
        _ => &[],
    }
}

fn row_json(row: StyleId) -> Value {
    json!({
        "row": row.name(),
        "codec": vocab::codec(row),
        "values": row.enum_names(),
        "default": vocab::default(row),
    })
}

fn tag_json(name: &str, t: &Tag) -> Value {
    let mut doc = json!({
        "name": name,
        "node": t.node_type.name(),
        "family": family(t),
        "fixedStyles": t.fixed_styles.iter().map(|(row, v)| json!({"row": row.name(), "value": v})).collect::<Vec<_>>(),
        "fixedProps": t.fixed_props.iter().map(|(p, v)| json!({"prop": p.name(), "value": v})).collect::<Vec<_>>(),
        "positional": t.positional.map(PropId::name),
    });
    if let Some(note) = vocab::tag_note(name) {
        doc["note"] = note.into();
    }
    doc
}

fn attr_json(name: &str, a: &AttrTarget) -> Value {
    let mut doc = json!({ "name": name, "kind": kind(a) });
    match a {
        AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand => {
            doc["rows"] = rows_of(name, a).iter().map(|r| row_json(*r)).collect();
            if rows_of(name, a).iter().any(|r| vocab::takes_roles(*r)) {
                let (exact, css) = vocab::color_roles();
                doc["roles"] = json!({ "exact": exact, "css": css });
            }
        }
        AttrTarget::Prop(p) | AttrTarget::InvertedBoolProp(p) => {
            doc["prop"] = p.name().into();
            doc["type"] = prop_type(*p).into();
            if let Some(accepts) = vocab::accepts(*p) {
                doc["accepts"] = accepts.into();
            }
        }
        AttrTarget::Handler(event) => doc["event"] = (*event).into(),
        AttrTarget::Surface => {}
        AttrTarget::MediaMetadata => {
            doc["type"] = METADATA.into();
            doc["props"] = json!(["mediaTitle", "mediaArtist", "mediaAlbum", "mediaArtwork"]);
        }
    }
    if let Some(only) = only_on(name) {
        doc["only"] = only.into();
    }
    if name == "accept" {
        doc["note"] = "On input type=file, omit, leave empty, or use */* for any file; specific types are media or app file_handlers types".into();
    }
    if name == "buttonStyle" {
        doc["note"] = vocab::BUTTON_DEFAULT_NOTE.into();
    }
    doc
}

fn everything() -> Value {
    json!({
        "tags": vocab::tags().iter().map(|(n, t)| tag_json(n, t)).collect::<Vec<_>>(),
        "attributes": vocab::attrs().iter().map(|(n, a)| attr_json(n, a)).collect::<Vec<_>>(),
        "renamed": vocab::renamed().iter().map(|(n, r)| json!({"name": n, "use": r})).collect::<Vec<_>>(),
        "html": vocab::html_tags().iter().map(|(n, h)| json!({"name": n, "hint": h})).collect::<Vec<_>>(),
        "head": tags::HEAD_FIELDS,
        "contextual": vocab::CONTEXTUAL.iter().map(|(n, on)| json!({"name": n, "only": on})).collect::<Vec<_>>(),
        "commands": HOST_COMMANDS,
        "functions": functions::functions().iter().map(function_json).collect::<Vec<_>>(),
        "concepts": concepts::concepts().iter().map(concept_json).collect::<Vec<_>>(),
        "notes": { "modules": vocab::MODULE_NOTE, "data": vocab::DATA_NOTE },
    })
}

fn tag_text(t: &Tag) -> String {
    let mut text = format!("{} ({})", t.node_type.name(), family(t));
    let fixed: Vec<String> = t
        .fixed_styles
        .iter()
        .map(|(row, v)| format!("{}: {v}", row.name()))
        .chain(
            t.fixed_props
                .iter()
                .map(|(p, v)| format!("{}={v}", p.name())),
        )
        .collect();
    if !fixed.is_empty() {
        text += &format!("; sets {}", fixed.join(", "));
    }
    if let Some(p) = t.positional {
        text += &format!("; positional {}", p.name());
    }
    text
}

/// One row's codec, values and default, as one phrase.
fn row_text(row: StyleId) -> String {
    let mut text = vocab::codec(row);
    let values = row.enum_names();
    if !values.is_empty() {
        text += &format!(" {}", values.join("|"));
    }
    if let Some(default) = vocab::default(row) {
        text += &format!(", default {default}");
    }
    text
}

/// An attribute's detail lines: a style's rows (shared description once),
/// a prop's type, a handler's event.
fn attr_detail(name: &str, a: &AttrTarget) -> Vec<String> {
    match a {
        AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand => {
            let rows = rows_of(name, a);
            let names: Vec<&str> = rows.iter().map(|r| r.name()).collect();
            let texts: Vec<String> = rows.iter().map(|r| row_text(*r)).collect();
            let mut lines = vec![format!("rows {}", names.join(", "))];
            if texts.iter().all(|t| *t == texts[0]) {
                lines.push(texts[0].clone());
            } else {
                lines.extend(names.iter().zip(&texts).map(|(n, t)| format!("{n}: {t}")));
            }
            if matches!(a, AttrTarget::Flex) {
                lines.push("`flex: <n>` is CSS's `<n> 1 0%`".into());
            }
            // The roles a colour takes, by name: a builder guessed one (`contract vocab background-color`, tip-splitter).
            if rows.iter().any(|r| vocab::takes_roles(*r)) {
                let (exact, css) = vocab::color_roles();
                lines.push(format!(
                    "roles `-exact-<role>`, a role that follows the platform and the scheme: {}",
                    exact.join(", ")
                ));
                lines.push(format!("CSS system colours, by name: {}", css.join(", ")));
            }
            if rows == [StyleId::Transition] {
                lines.push(format!(
                    "properties {}",
                    vocab::transition_properties().join(", ")
                ));
            }
            if name == "backdrop-filter" {
                lines.push("macOS samples only the parent's subtree; make the glass a sibling of what it covers (LLP 1053.000 §3)".into());
            }
            if name == "font-family" {
                lines.push("CSS family list: literal lists and literal choices work on every host; a string from state or data names installed or generic families on web, macOS, iOS and Linux. Invalid lists leave the row unset. Bundled faces use a `font` declaration".into());
            }
            lines
        }
        AttrTarget::Prop(p) => {
            let mut line = format!("prop {}, {}", p.name(), prop_type(*p));
            if let Some(accepts) = vocab::accepts(*p) {
                line += &format!(", accepts {accepts}");
            }
            // A prop with a fixed set of names lists them (authoring bench: `vocab buttonStyle`).
            if p.name() == "buttonStyle" {
                line += &format!(
                    ", one of {}",
                    exact_kernel::generated::BUTTON_STYLES.join("|")
                );
                // LLP 1115.000 D14.5: a presentation default is the platform's, said per host.
                return vec![line, vocab::BUTTON_DEFAULT_NOTE.into()];
            }
            if name == "accept" {
                return vec![line, "on input type=file: omit, leave empty, or use */* for any file; specific types are media or app file_handlers types".into()];
            }
            vec![line]
        }
        AttrTarget::InvertedBoolProp(p) => {
            vec![format!("prop {}, bool, set to the inverse", p.name())]
        }
        AttrTarget::Handler(event) => vec![format!("event {event}")],
        AttrTarget::Surface => vec!["a canvas's surface binding: surface=name(args)".into()],
        AttrTarget::MediaMetadata => vec![
            format!("record {METADATA}"),
            "props mediaTitle, mediaArtist, mediaAlbum, mediaArtwork, its fields".into(),
            "claims the media session (LLP 1098 D1)".into(),
        ],
    }
}

/// Names wrapped into indented lines.
fn wrapped(out: &mut String, names: impl IntoIterator<Item = String>) {
    let mut line = String::from(" ");
    for name in names {
        if line.len() + name.len() > 92 {
            out.push_str(line.trim_end_matches(','));
            out.push('\n');
            line = String::from(" ");
        }
        line += &format!(" {name},");
    }
    out.push_str(line.trim_end_matches(','));
    out.push('\n');
}

fn listing() -> String {
    let mut out = String::new();
    let tags = vocab::tags();
    out += &format!(
        "tags ({}): name  node (family); fixed rows and props\n",
        tags.len()
    );
    for (name, t) in &tags {
        out += &format!("  {name:<20} {}\n", tag_text(t));
    }
    let attrs = vocab::attrs();
    let of = |k: &'static str| attrs.iter().filter(move |(_, a)| kind(a) == k);
    out += &format!(
        "\nstyle attributes ({}): name  codec [values], default (rows, when not one of the same name)\n",
        of("style").count() + of("flex").count() + of("shorthand").count()
    );
    for (name, a) in of("style").chain(of("flex")).chain(of("shorthand")) {
        let rows = rows_of(name, a);
        let mut text = if rows.iter().all(|r| row_text(*r) == row_text(rows[0])) {
            row_text(rows[0])
        } else {
            rows.iter()
                .map(|r| format!("{}: {}", r.name(), row_text(*r)))
                .collect::<Vec<_>>()
                .join("; ")
        };
        if rows.len() > 1 || rows[0].name() != name.replace('-', "_") {
            text += &format!(
                " ({})",
                rows.iter().map(|r| r.name()).collect::<Vec<_>>().join(" ")
            );
        }
        if let Some(only) = only_on(name) {
            text += &format!("; only on {only}");
        }
        out += &format!("  {name:<28} {text}\n");
    }
    let props: Vec<_> = of("prop")
        .chain(of("inverted-prop"))
        .chain(of("surface"))
        .chain(of("record"))
        .collect();
    out += &format!("\nprop attributes ({}): name  type\n", props.len());
    for (name, a) in props {
        let mut text = match a {
            AttrTarget::Prop(p) => prop_type(*p),
            AttrTarget::InvertedBoolProp(p) => format!("bool (inverse of {})", p.name()),
            AttrTarget::MediaMetadata => METADATA.into(),
            _ => "surface=name(args), on `canvas`".into(),
        };
        if let Some(only) = only_on(name) {
            text += &format!("; only on {only}");
        }
        out += &format!("  {name:<28} {text}\n");
    }
    out += &format!("\nhandlers ({}):\n", of("handler").count());
    wrapped(&mut out, of("handler").map(|(n, _)| n.to_string()));
    out += "\nrenamed (refused, with the name to write):\n";
    wrapped(
        &mut out,
        vocab::renamed().iter().map(|(n, r)| format!("{n} -> {r}")),
    );
    out += "\nHTML elements Contract spells differently:\n";
    for (name, hint) in vocab::html_tags() {
        out += &format!("  {name:<8} {hint}\n");
    }
    out += &format!(
        "\nonly on `head`: {}\nonly on some tags:\n",
        tags::HEAD_FIELDS.join(", ")
    );
    for (name, on) in vocab::CONTEXTUAL {
        out += &format!("  {name:<16} {on}\n");
    }
    out += &format!(
        "\nhost commands ({}), action statements:\n",
        HOST_COMMANDS.len()
    );
    wrapped(&mut out, HOST_COMMANDS.iter().map(|c| c.to_string()));
    let (mut standard, intrinsic): (Vec<Function>, Vec<Function>) = functions::functions()
        .into_iter()
        .partition(|f| f.signature.is_some());
    standard.sort_by_key(|f| f.name.to_lowercase());
    out += &format!(
        "\nstandard functions ({}): the roster's signature; `contract vocab <name>` gives its rules and an example\n",
        standard.len()
    );
    for f in &standard {
        out += &format!("  {}\n", f.signature.as_deref().unwrap_or_default());
    }
    out += "\ncompiler intrinsics:\n";
    wrapped(&mut out, intrinsic.into_iter().map(|f| f.name));
    out += &format!(
        "\nopen sets:\n  {}\n  {}\n",
        vocab::MODULE_NOTE,
        vocab::DATA_NOTE
    );
    let concepts = concepts::concepts();
    out += &format!(
        "\nconcepts ({}): `contract vocab <name>` answers with the whole idiom, its code and its recipe\n",
        concepts.len()
    );
    for c in &concepts {
        out += &format!("  {:<10} {}\n", c.name, c.title);
    }
    out
}
