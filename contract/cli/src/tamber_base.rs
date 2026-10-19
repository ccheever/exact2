//! The base scale on the wire (LLP 1121 stage 11 D, `tamber/1.14`; theme2's
//! base-row shape): `describe`'s `base` and `targets`, `resolve` and `explain`
//! of a base name, and `explain`'s `bases` for a token whose values reach one.
//! Every cell is read from the kernel's scale table (`style::scale`), never a
//! copy; an app's own line for a base name is its `override`.

use super::{files_json, optional, Server};
use exact_kernel::style::scale::{self, ScaleSource, ScaleValue};
use serde_json::{json, Value};

/// A cell's value as the shape writes it: a CSS length (`16px`), `full`
/// (radius.full, the pill), or the terminal's `<n>cell` (`0` for none).
fn value(v: ScaleValue) -> String {
    match v {
        ScaleValue::Px(_) => scale::css(v),
        ScaleValue::Cells(0.0) => "0".into(),
        ScaleValue::Cells(n) => format!("{n}cell"),
        ScaleValue::Full => "full".into(),
    }
}

/// A cell's source as the shape writes it.
fn source(s: ScaleSource) -> Value {
    match s {
        ScaleSource::Metric(m) => json!({"kind": "metric", "ref": m}),
        ScaleSource::Measured(id) => json!({"kind": "measured", "id": id}),
        ScaleSource::Guideline(id) => json!({"kind": "guideline", "cite": id}),
        ScaleSource::Full => json!({"kind": "full"}),
        ScaleSource::Terminal => json!({"kind": "terminal"}),
    }
}

/// The design file a request names (`file`, else the app's).
fn file(s: &Server, request: &Value) -> Result<String, Value> {
    Ok(optional(request, "file")?.unwrap_or_else(|| s.design.clone()))
}

/// One base row: its kind, every column's cell with its source, and the
/// line of the design asked about that sets the name (`override`), or null.
fn row(name: &str, file: &str, design: &exact_tamber::Design) -> Option<Value> {
    let r = scale::row(name)?;
    let columns: serde_json::Map<String, Value> = scale::COLUMNS
        .iter()
        .map(|(key, column)| {
            let cell = scale::cell(r, *column);
            (
                key.to_string(),
                json!({"value": value(cell.value), "source": source(cell.source)}),
            )
        })
        .collect();
    let set = design.tokens.iter().find(|t| t.name.value == name);
    let over = set.map_or(
        Value::Null,
        |t| json!({"file": file, "line": t.name.line, "col": t.name.col, "text": t.value.value}),
    );
    let kind = if name.starts_with("radius.") {
        "radius"
    } else {
        "length"
    };
    Some(json!({"name": name, "kind": kind, "columns": columns, "override": over}))
}

/// The base names a checked token reaches, in the order it folds them: an
/// unset one is a named length in its values, a set one a token it names.
/// Read from the checker, so a quoted family or a word in a string is none.
fn bases_of(token: &exact_tamber::Checked) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    let mut add = |n: &str| {
        if let Some(b) = scale::names().find(|b| *b == n) {
            if !out.contains(&b) {
                out.push(b);
            }
        }
    };
    for v in &token.values {
        if let exact_tamber::Value::Length(exact_tamber::Length::Named(n)) = v {
            add(n);
        }
    }
    for n in &token.names {
        add(n);
    }
    out
}

/// `describe`, with `base` (every base name in the table's order) and
/// `targets` (the targets the app ships, each with the column it reads, null
/// for one the kernel names none for). The design is folded again for its
/// overrides; a save between the two reads is read again, so the base rows
/// and `files` are one revision.
pub(super) fn describe(s: &Server, request: &Value) -> Result<Value, Value> {
    let app = s.app_params();
    for _ in 0..3 {
        let mut out = s.describe(request)?;
        let (snap, _, design) = s.design_with(request, &app)?;
        if files_json(&snap) != out["files"] {
            continue;
        }
        let file = file(s, request)?;
        out["base"] = json!(scale::names()
            .filter_map(|n| row(n, &file, &design))
            .collect::<Vec<_>>());
        out["targets"] = json!(contract_lower::icons::shipped_targets(&s.dir)
            .iter()
            .map(|t| json!({"target": t, "column": scale::column_for(t)}))
            .collect::<Vec<_>>());
        return Ok(out);
    }
    Err(json!({"message": "the design kept changing while it was read: ask again"}))
}

/// The request's token as the server resolves it (a param's full name
/// first, `lookup`'s rule), and whether that is a base name: a param named
/// like one (`space.md` a component `space`'s param) is the param.
fn as_base(
    s: &Server,
    request: &Value,
) -> Result<Option<(super::Snapshot, exact_tamber::Folding, String)>, Value> {
    let asked = super::required(request, "token")?;
    let (snap, folding, name) = s.lookup(request)?;
    Ok((name == asked && scale::is_base(&name)).then_some((snap, folding, name)))
}

/// `resolve` of a base name: with no override `{name, base}` (no per-tuple
/// fold, the value is each column's); with one, the token's tuples and `base`.
/// The file must be a design file that checks, as for any token.
pub(super) fn resolve(s: &Server, request: &Value) -> Result<Value, Value> {
    let Some((snap, folding, name)) = as_base(s, request)? else {
        return s.resolve(request);
    };
    let base = row(&name, &file(s, request)?, &folding.design).unwrap_or(Value::Null);
    if base["override"].is_null() {
        return Ok(json!({"name": name, "base": base, "files": files_json(&snap)}));
    }
    let mut out = s.resolve(request)?;
    out["base"] = base;
    Ok(out)
}

/// `explain` of a base name: `{kind: "base", name, base}`, with the app
/// line's own explain fields beside them when it sets it (theme2's 18:11
/// addition); of any token that reaches base names, `bases`, those names
/// as strings (18:26), the overridden base's own included.
pub(super) fn explain(s: &Server, request: &Value) -> Result<Value, Value> {
    if let Some((snap, folding, name)) = as_base(s, request)? {
        let base = row(&name, &file(s, request)?, &folding.design).unwrap_or(Value::Null);
        let mut out = match base["override"].is_null() {
            true => json!({"name": name, "files": files_json(&snap)}),
            false => s.explain(request)?,
        };
        if let Some(t) = folding.tokens.iter().find(|t| t.name == name) {
            let bases = bases_of(t);
            if !bases.is_empty() {
                out["bases"] = json!(bases);
            }
        }
        out["kind"] = json!("base");
        out["name"] = json!(name);
        out["base"] = base;
        return Ok(out);
    }
    let mut out = s.explain(request)?;
    // The checked token the answer names (a param by its name below the IR).
    let (_, folding, name) = s.lookup(request)?;
    if let Some(t) = folding.tokens.iter().find(|t| t.name == name) {
        let bases = bases_of(t);
        if !bases.is_empty() {
            out["bases"] = json!(bases);
        }
    }
    Ok(out)
}
