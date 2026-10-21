//! Pencil's `.pen` files (LLP 1121 stage 13 B; rt1's plan, theme2's 14:43
//! approval): a design kept as JSON beside the code, its `variables` and
//! `themes` read into the DTCG importer's own layers, so the tuples,
//! aliases, kinds and the review are that importer's.
//!
//! - **Axes** (`themes`, each axis's values, the first the default): an axis
//!   of two values, `dark` and `light` or `default` in any case, is the
//!   scheme; any other a proposed axis.
//! - **Values**: each entry of a variable's `value` (a scalar is one entry)
//!   is a layer of that one token, active where its `theme` holds (every
//!   axis it names at once; none, always), in the order written, so a later
//!   one wins, as Pencil's "the last whose theme is satisfied" does.
//!   `"$name"` is an alias, `{name}`.
//! - **Kinds**: `color` a colour; `number` a length (px) where every node
//!   that reads it reads it as a pixel property, else a number, each with a
//!   note naming the properties that decided it; `string` a family where
//!   every node reads it as `fontFamily`; any other string, a boolean and
//!   an unknown `type` reported. A leading `--` (the shadcn kit's spelling)
//!   is no part of a name.
//! - `imports` (other `.pen` files) are reported, not followed: a client's
//!   upload has no directory to resolve them against.
//! - A `version` other than `2.x` refuses the import. Every step charges
//!   the import's budget (`crate::budget`), the node walk included.

use super::{ident, is_dark, planned, Def, Read, Scope};
use crate::import::Import;
use crate::{Design, Diagnostic};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The node properties Pencil reads as pixels.
const PIXELS: [&str; 8] = [
    "cornerRadius",
    "gap",
    "padding",
    "fontSize",
    "letterSpacing",
    "strokeWidth",
    "width",
    "height",
];

/// A variable name as a token path: without the shadcn kit's leading `--`.
fn path_of(name: &str) -> String {
    name.strip_prefix("--").unwrap_or(name).to_string()
}

/// Each variable's readers: the properties of every node that names it
/// (`"$name"`), counted; every value visited charged to the budget.
fn readers(
    v: &Value,
    key: Option<&str>,
    out: &mut BTreeMap<String, BTreeMap<String, usize>>,
) -> Result<(), String> {
    crate::budget::spend(1)?;
    match v {
        Value::String(s) => {
            if let (Some(name), Some(k)) = (s.strip_prefix('$'), key) {
                *out.entry(name.to_string())
                    .or_default()
                    .entry(k.to_string())
                    .or_default() += 1;
            }
        }
        Value::Array(all) => {
            for x in all {
                readers(x, key, out)?;
            }
        }
        Value::Object(m) => {
            crate::budget::members(m)?;
            for (k, x) in m {
                // (a nested object's own keys name the property: `stroke: {fill: "$ink"}` is `fill`)
                readers(x, Some(k), out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// The properties as a note says them: `cornerRadius (2), gap`.
fn listed(uses: &BTreeMap<String, usize>) -> String {
    uses.iter()
        .map(|(k, n)| match n {
            1 => k.clone(),
            n => format!("{k} ({n})"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The layers a `.pen` document makes, and the notes on the kinds it chose.
fn read(file: &str, doc: &Value) -> Result<(Read, Vec<String>), String> {
    let root = doc.as_object().ok_or("a `.pen` file is a JSON object")?;
    match root.get("version").and_then(Value::as_str) {
        Some(v) if v.starts_with("2.") => {}
        Some(v) => {
            return Err(format!(
                "a `.pen` of version {}: this importer reads 2.x",
                v.chars().take(20).collect::<String>()
            ))
        }
        None => return Err("no `version`: is this a Pencil `.pen` file?".into()),
    }
    let mut out = Read::default();
    // The axes, each value's scope; the scheme's two sides by the axis's name.
    let mut scheme_axis: Option<String> = None;
    let mut axes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(themes) = root.get("themes") {
        let themes = themes
            .as_object()
            .ok_or("`themes` is an object of axes and their values")?;
        crate::budget::members(themes)?;
        for (axis, values) in themes {
            let values: Vec<String> = values
                .as_array()
                .filter(|v| !v.is_empty())
                .and_then(|v| v.iter().map(|x| x.as_str().map(str::to_string)).collect())
                .ok_or_else(|| format!("`themes.{axis}` is a list of values"))?;
            let mut lower: Vec<String> = values
                .iter()
                .map(|v| v.trim().to_ascii_lowercase())
                .collect();
            lower.sort();
            let is_scheme = scheme_axis.is_none()
                && (lower == ["dark", "light"] || lower == ["dark", "default"]);
            if is_scheme {
                scheme_axis = Some(axis.clone());
            } else {
                for v in &values {
                    out.axis(axis, v);
                }
            }
            axes.insert(axis.clone(), values);
        }
    }
    let readers = {
        let mut r = BTreeMap::new();
        if let Some(children) = root.get("children") {
            self::readers(children, None, &mut r)?;
        }
        r
    };
    let mut notes = Vec::new();
    let empty = BTreeMap::new();
    if let Some(vars) = root.get("variables") {
        let vars = vars
            .as_object()
            .ok_or("`variables` is an object of names and definitions")?;
        crate::budget::members(vars)?;
        for (name, def) in vars {
            crate::budget::spend(1)?;
            let path = path_of(name);
            let uses = readers.get(name).unwrap_or(&empty);
            let ty = def.get("type").and_then(Value::as_str).unwrap_or("");
            // The DTCG type it is read as, and a value as DTCG writes one.
            let pixels = !uses.is_empty() && uses.keys().all(|k| PIXELS.contains(&k.as_str()));
            let as_dtcg: (&str, fn(&Value) -> Value) = match ty {
                "color" => ("color", |v| v.clone()),
                "number" if pixels => ("dimension", |v| match v.as_f64() {
                    Some(n) => json!({"value": n, "unit": "px"}),
                    None => v.clone(),
                }),
                "number" => ("number", |v| v.clone()),
                "string" if !uses.is_empty() && uses.keys().all(|k| k == "fontFamily") => {
                    ("fontFamily", |v| v.clone())
                }
                "string" => ("string", |v| v.clone()),
                "boolean" => ("boolean", |v| v.clone()),
                _ => ("", |v| v.clone()),
            };
            if ty == "number" {
                let pixel: BTreeMap<String, usize> = uses
                    .iter()
                    .filter(|(k, _)| PIXELS.contains(&k.as_str()))
                    .map(|(k, n)| (k.clone(), *n))
                    .collect();
                let other: BTreeMap<String, usize> = uses
                    .iter()
                    .filter(|(k, _)| !PIXELS.contains(&k.as_str()))
                    .map(|(k, n)| (k.clone(), *n))
                    .collect();
                notes.push(match (pixel.is_empty(), other.is_empty()) {
                    (_, true) if pixels => format!("`{name}` is a length (px): Pencil reads it as {}", listed(&pixel)),
                    (true, true) => format!("`{name}` is a number: no frame reads it"),
                    (true, false) => format!("`{name}` is a number: Pencil reads it as {}", listed(&other)),
                    (false, false) => format!("`{name}` is a number: Pencil reads it as a pixel length ({}) and as {}, so it is no length", listed(&pixel), listed(&other)),
                    (false, true) => unreachable!("every use a pixel property is a length"),
                });
            }
            // Each entry: a scalar is one, always active.
            let entries: Vec<(Value, Option<&Map<String, Value>>)> = match def.get("value") {
                Some(Value::Array(all)) => all
                    .iter()
                    .map(|e| match e {
                        Value::Object(o) if o.contains_key("value") => Ok((
                            o["value"].clone(),
                            match o.get("theme") {
                                None => None,
                                Some(Value::Object(t)) => Some(t),
                                Some(_) => {
                                    return Err(format!(
                                    "`variables.{name}`: a `theme` is an object of axes and values"
                                ))
                                }
                            },
                        )),
                        _ => Err(format!(
                            "`variables.{name}`: each value is {{value, theme?}}"
                        )),
                    })
                    .collect::<Result<_, _>>()?,
                Some(v) => vec![(v.clone(), None)],
                None => return Err(format!("`variables.{name}` has no `value`")),
            };
            for (value, theme) in entries {
                crate::budget::copy(&value)?;
                // An alias names another variable by its Pencil name.
                let value = match value.as_str().and_then(|s| s.strip_prefix('$')) {
                    Some(target) => Value::String(format!("{{{}}}", path_of(target))),
                    None => (as_dtcg.1)(&value),
                };
                let mut refused = (as_dtcg.0.is_empty()).then(|| {
                    format!(
                        "a Pencil `type` the importer does not name (`{}`)",
                        ty.chars().take(20).collect::<String>()
                    )
                });
                let mut scopes = Vec::new();
                let mut said = Vec::new();
                for (axis, v) in theme.into_iter().flatten() {
                    let v = v.as_str().unwrap_or_default();
                    said.push(format!("{axis}={v}"));
                    if !axes.get(axis).is_some_and(|vs| vs.iter().any(|x| x == v)) {
                        refused.get_or_insert(format!(
                            "its theme names `{axis}={v}`, which `themes` does not declare"
                        ));
                        continue;
                    }
                    scopes.push(match &scheme_axis {
                        Some(s) if s == axis => Scope::Scheme(is_dark(v)),
                        _ => Scope::Axis(ident(axis), ident(v)),
                    });
                }
                let scope = match scopes.len() {
                    0 => Scope::Base,
                    1 => scopes.remove(0),
                    _ => Scope::All(scopes),
                };
                let block = match said.is_empty() {
                    true => file.to_string(),
                    false => format!("{file} ({})", said.join(", ")),
                };
                let def = Def {
                    file: block,
                    path: path.clone(),
                    ty: Some(as_dtcg.0.to_string()).filter(|t| !t.is_empty()),
                    value,
                    refused,
                };
                out.layer(vec![scope], vec![def]);
            }
        }
    }
    // Another `.pen`'s variables: reported, each alias once.
    if let Some(imports) = root.get("imports").and_then(Value::as_object) {
        crate::budget::members(imports)?;
        let found = imports
            .iter()
            .map(|(alias, to)| Def {
                file: file.to_string(),
                path: format!("imports.{alias}"),
                ty: None,
                value: to.clone(),
                refused: None,
            })
            .collect();
        out.unused(found, "an imported `.pen`: not followed (a client's upload has no directory to find it in); import that file on its own");
    }
    Ok((out, notes))
}

/// Import Pencil's `.pen` document `text` (named `file`) under `prefix`,
/// planned against `design`, the design checked against `vocab`.
pub fn import_with(
    file: &str,
    text: &str,
    prefix: &str,
    design: &Design,
    vocab: &crate::Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    import_within(file, text, prefix, design, vocab, crate::budget::IMPORT)
}

/// [`import_with`] inside a budget of `limit` (a test's smaller one).
#[doc(hidden)]
pub fn import_within(
    file: &str,
    text: &str,
    prefix: &str,
    design: &Design,
    vocab: &crate::Vocab,
    limit: u64,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-import-source", m, 1, 1)];
    let _work = crate::budget::Work::begin(limit);
    crate::budget::spend(text.len() as u64).map_err(refuse)?;
    let doc: Value = serde_json::from_str(text).map_err(|e| refuse(format!("{file}: {e}")))?;
    let (read, notes) = read(file, &doc).map_err(|m| refuse(format!("{file}: {m}")))?;
    let mut import = planned(read, prefix, design, vocab)?;
    import.notes = notes;
    Ok(import)
}
