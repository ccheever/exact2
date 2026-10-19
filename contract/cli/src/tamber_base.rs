//! The base scale on the wire (LLP 1121 stage 11 D, `tamber/1.14`; theme2's
//! base-row shape): `describe`'s `base` and `targets`, `resolve` and `explain`
//! of a base name, and `explain`'s `bases` for a token whose values reach one.
//! Every cell is read from the kernel's scale table (`style::scale`), never a
//! copy; an app's own line for a base name is its `override`.

use super::{files_json, optional, required, Server, Snapshot};
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

/// The design asked about (`file`, else the app's) as parsed, by its path.
fn design(
    s: &Server,
    request: &Value,
    snap: &Snapshot,
) -> Result<(String, Option<exact_tamber::Design>), Value> {
    let file = optional(request, "file")?.unwrap_or_else(|| s.design.clone());
    let parsed = snap
        .iter()
        .find(|(f, ..)| *f == file)
        .and_then(|(_, text, _)| exact_tamber::parse(text).ok());
    Ok((file, parsed))
}

/// One base row: its kind, every column's cell with its source, and the
/// app's own line for it (`override`), or null.
fn row(name: &str, file: &str, design: Option<&exact_tamber::Design>) -> Option<Value> {
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
    let set = design.and_then(|d| d.tokens.iter().find(|t| t.name.value == name));
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

/// `describe`, with `base` (every base name in the table's order) and
/// `targets` (the targets the app ships, each with the column it reads).
pub(super) fn describe(s: &Server, request: &Value) -> Result<Value, Value> {
    let mut out = s.describe(request)?;
    let snap = s.snapshot()?;
    let (file, parsed) = design(s, request, &snap)?;
    out["base"] = json!(scale::names()
        .filter_map(|n| row(n, &file, parsed.as_ref()))
        .collect::<Vec<_>>());
    out["targets"] = json!(contract_lower::icons::shipped_targets(&s.dir)
        .iter()
        .filter_map(|t| scale::column_for(t).map(|c| json!({"target": t, "column": c})))
        .collect::<Vec<_>>());
    Ok(out)
}

/// `resolve` of a base name: with no override `{name, base}` (no per-tuple
/// fold, the value is each column's); with one, the token's tuples and `base`.
pub(super) fn resolve(s: &Server, request: &Value) -> Result<Value, Value> {
    let name = required(request, "token")?;
    if !scale::is_base(&name) {
        return s.resolve(request);
    }
    let snap = s.snapshot()?;
    let (file, parsed) = design(s, request, &snap)?;
    let base = row(&name, &file, parsed.as_ref()).unwrap_or(Value::Null);
    if base["override"].is_null() {
        return Ok(json!({"name": name, "base": base, "files": files_json(&snap)}));
    }
    let mut out = s.resolve(request)?;
    out["base"] = base;
    Ok(out)
}

/// `explain` of a base name: `{kind: "base", name, base}`, with the app
/// line's own explain fields beside them when it overrides it (theme2's
/// 18:11 addition); of any other token, its explain with `bases`, the base
/// names its values reach (unset, folded as a named length; or set, its
/// token), as strings.
pub(super) fn explain(s: &Server, request: &Value) -> Result<Value, Value> {
    let name = required(request, "token")?;
    let snap = s.snapshot()?;
    let (file, parsed) = design(s, request, &snap)?;
    if scale::is_base(&name) {
        let base = row(&name, &file, parsed.as_ref()).unwrap_or(Value::Null);
        let mut out = match base["override"].is_null() {
            true => json!({"name": name, "files": files_json(&snap)}),
            false => s.explain(request)?,
        };
        out["kind"] = json!("base");
        out["name"] = json!(name);
        out["base"] = base;
        return Ok(out);
    }
    let mut out = s.explain(request)?;
    let declared = parsed
        .as_ref()
        .and_then(|d| d.tokens.iter().find(|t| t.name.value == name));
    let mut bases: Vec<&str> = Vec::new();
    for text in declared
        .into_iter()
        .flat_map(|t| std::iter::once(&t.value.value).chain(t.whens.iter().map(|w| &w.value.value)))
    {
        for word in
            text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'))
        {
            if let Some(b) = scale::names().find(|b| *b == word) {
                if !bases.contains(&b) {
                    bases.push(b);
                }
            }
        }
    }
    // As strings, parallel to `names` (theme2's 18:26 ruling): a client looks
    // each up in `describe`'s base rows.
    if !bases.is_empty() {
        out["bases"] = json!(bases);
    }
    Ok(out)
}
