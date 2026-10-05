//! `contract vocab [--json] [<name>]` — what the compiler admits: every tag,
//! every attribute by kind (a style's rows with codec, values and default; a
//! prop's type; a handler), the renamed spellings, and the rules no lookup
//! lists. @ref LLP 1086 D3. The list is `contract_lower::vocab`'s, which
//! keeps only what the live lookups admit.

use contract_lower::tags::{self, AttrTarget, Tag};
use contract_lower::vocab;
use exact_kernel::{PropId, StyleId};
use serde_json::{json, Value};
use std::process::ExitCode;

const USAGE: &str = "usage: contract vocab [--json] [<name>]";

pub fn run(args: &[String]) -> ExitCode {
    let mut json = false;
    let mut name = None;
    for arg in args {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => json = true,
            _ if !arg.starts_with('-') && name.is_none() => name = Some(arg.as_str()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match name {
        None if json => {
            println!("{:#}", everything());
            ExitCode::SUCCESS
        }
        None => {
            print!("{}", listing());
            ExitCode::SUCCESS
        }
        Some(name) => one(name, json),
    }
}

/// One name: its tag and attribute entries (`input` is both), an open set's
/// rule, or the compiler's refusal with exit 1.
fn one(name: &str, json: bool) -> ExitCode {
    let tag = tags::tag(name).map(|t| tag_json(name, &t));
    let attr = tags::attr(name).map(|a| attr_json(name, &a));
    let open = vocab::open_set(name).filter(|_| tag.is_none() && attr.is_none());
    if tag.is_none() && attr.is_none() && open.is_none() {
        let refusal = vocab::refusal(name);
        if json {
            println!("{:#}", json!({ "name": name, "refused": refusal }));
        } else {
            eprintln!("{refusal}");
        }
        return ExitCode::from(1);
    }
    if json {
        let mut doc = json!({ "name": name });
        if let Some(tag) = tag {
            doc["tag"] = tag;
        }
        if let Some(attr) = attr {
            doc["attribute"] = attr;
        }
        if let Some(open) = open {
            doc["note"] = open.into();
        }
        println!("{doc:#}");
    } else {
        if let Some(t) = tags::tag(name) {
            println!("{name}: tag, {}", tag_text(&t));
        }
        if let Some(a) = tags::attr(name) {
            println!("{name}: {} attribute", kind(&a));
            for line in attr_detail(name, &a) {
                println!("  {line}");
            }
            if let Some(only) = only_on(name) {
                println!("  only on {only}");
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
        }
        if let Some(open) = open {
            println!("{name}: {open}");
        }
    }
    ExitCode::SUCCESS
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
    }
}

fn prop_type(p: PropId) -> String {
    format!("{:?}", tags::prop_ty(p)).to_lowercase()
}

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
    json!({
        "name": name,
        "node": t.node_type.name(),
        "family": family(t),
        "fixedStyles": t.fixed_styles.iter().map(|(row, v)| json!({"row": row.name(), "value": v})).collect::<Vec<_>>(),
        "fixedProps": t.fixed_props.iter().map(|(p, v)| json!({"prop": p.name(), "value": v})).collect::<Vec<_>>(),
        "positional": t.positional.map(PropId::name),
    })
}

fn attr_json(name: &str, a: &AttrTarget) -> Value {
    let mut doc = json!({ "name": name, "kind": kind(a) });
    match a {
        AttrTarget::Styles(_) | AttrTarget::Flex | AttrTarget::Shorthand => {
            doc["rows"] = rows_of(name, a).iter().map(|r| row_json(*r)).collect();
        }
        AttrTarget::Prop(p) | AttrTarget::InvertedBoolProp(p) => {
            doc["prop"] = p.name().into();
            doc["type"] = prop_type(*p).into();
        }
        AttrTarget::Handler(event) => doc["event"] = (*event).into(),
        AttrTarget::Surface => {}
    }
    if let Some(only) = only_on(name) {
        doc["only"] = only.into();
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
            lines
        }
        AttrTarget::Prop(p) => {
            let mut line = format!("prop {}, {}", p.name(), prop_type(*p));
            // A prop with a fixed set of names lists them (authoring bench: `vocab buttonStyle`).
            if p.name() == "buttonStyle" {
                line += &format!(
                    ", one of {}",
                    exact_kernel::generated::BUTTON_STYLES.join("|")
                );
            }
            vec![line]
        }
        AttrTarget::InvertedBoolProp(p) => {
            vec![format!("prop {}, bool, set to the inverse", p.name())]
        }
        AttrTarget::Handler(event) => vec![format!("event {event}")],
        AttrTarget::Surface => vec!["a canvas's surface binding: surface=name(args)".into()],
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
        .collect();
    out += &format!("\nprop attributes ({}): name  type\n", props.len());
    for (name, a) in props {
        let mut text = match a {
            AttrTarget::Prop(p) => prop_type(*p),
            AttrTarget::InvertedBoolProp(p) => format!("bool (inverse of {})", p.name()),
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
        "\nopen sets:\n  {}\n  {}\n",
        vocab::MODULE_NOTE,
        vocab::DATA_NOTE
    );
    out
}
