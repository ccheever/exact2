//! Pencil's `.pen` files (LLP 1121 stage 13 B; rt1's plan, theme2's 14:43
//! approval): a design kept as JSON beside the code, its `variables` and
//! `themes` read into the DTCG importer's own layers, so the tuples,
//! aliases, kinds and the review are that importer's.
//!
//! - **Axes** (`themes`, in the order written, each axis's values with the
//!   first the default): the first axis of two values, `dark` and `light`
//!   or `default` in any case, is the scheme (its default the host's, as
//!   every scheme's); any other a proposed axis.
//! - **Values**: each entry of a variable's `value` (a scalar is one entry)
//!   is a layer of that one token, active where its `theme` holds (every
//!   axis it names at once; none, always), in the order written, so a later
//!   one wins, as Pencil's "the last whose theme is satisfied" does. An entry
//!   whose `theme` names an axis or value `themes` doesn't declare (Pencil's
//!   names are matched as written, case and all) never holds, and is
//!   reported. `"$name"` is an alias, of a variable of its own `type`.
//! - **Names** are flat, as Pencil's are: `a` and `a.b` are two tokens, and
//!   a name is matched as written; the token's name drops what an
//!   identifier can't hold (a leading `--`), so `--x` and `x` collide, said.
//! - **Kinds**: `color` a colour; `number` a length (px) where every node
//!   that reads it, or any number it aliases or that aliases it, reads it as
//!   a pixel property, else a number, each with a note naming the
//!   properties that decided it; `string` a family where every node reads
//!   it as `fontFamily`; any other string, a boolean and an unknown `type`
//!   reported.
//! - `imports` (other `.pen` files) are reported, not followed: a client's
//!   upload has no directory to resolve them against.
//! - A `version` other than `2.x` refuses the import, as do more than 16
//!   axes or 64 values of one. Every step charges the import's budget
//!   (`crate::budget`), names and the node walk included, before it copies.

use super::{is_dark, planned, Def, Read, Scope};
use crate::import::Import;
use crate::{Design, Diagnostic};
use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The node properties Pencil reads as pixels.
const PIXELS: [&str; 9] = [
    "cornerRadius",
    "gap",
    "padding",
    "fontSize",
    "letterSpacing",
    "strokeWidth",
    "thickness",
    "width",
    "height",
];
/// A pixel property's per-side or per-corner members (`thickness: {top, …}`).
const SIDES: [&str; 8] = [
    "top",
    "right",
    "bottom",
    "left",
    "topLeft",
    "topRight",
    "bottomRight",
    "bottomLeft",
];
/// Properties whose members are fractions of the box, never pixels (a
/// gradient's `size` and `center`).
const NORMALIZED: [&str; 2] = ["size", "center"];
/// The axes, and values of one, a file may declare.
const AXES: usize = 16;
const VALUES: usize = 64;

impl Scope {
    /// It and the scopes it holds (an `All`'s, which a Pencil entry's theme
    /// of several axes makes), each once, flat.
    pub(super) fn flat(&self) -> Vec<&Scope> {
        match self {
            Scope::All(all) => all.iter().flat_map(Scope::flat).collect(),
            s => vec![s],
        }
    }
}

/// A JSON object's members in the order written.
struct Ordered(Vec<(String, Value)>);

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = m.next_entry::<String, Value>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V)
    }
}

/// The document's parts this importer reads, `themes`, `variables` and
/// `imports` in the order written.
#[derive(Default)]
struct Doc {
    version: Option<Value>,
    themes: Option<Ordered>,
    variables: Option<Ordered>,
    imports: Option<Ordered>,
    children: Option<Value>,
}

impl<'de> serde::Deserialize<'de> for Doc {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Doc;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a `.pen` document, a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Doc, A::Error> {
                let mut doc = Doc::default();
                while let Some(k) = m.next_key::<String>()? {
                    match k.as_str() {
                        "version" => doc.version = Some(m.next_value()?),
                        "themes" => doc.themes = Some(m.next_value()?),
                        "variables" => doc.variables = Some(m.next_value()?),
                        "imports" => doc.imports = Some(m.next_value()?),
                        "children" => doc.children = Some(m.next_value()?),
                        _ => {
                            m.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(doc)
            }
        }
        d.deserialize_map(V)
    }
}

/// One variable's readers: each property that names it, counted, and
/// whether Pencil reads that property as pixels.
type Uses = BTreeMap<String, (usize, bool)>;

/// Each variable's readers in the node tree (`"$name"`), by the property
/// that reads it: its own key, a side's (`thickness.top`) or a fraction's
/// (`size.width`); every value visited charged to the budget.
fn readers(
    v: &Value,
    keys: &mut Vec<String>,
    out: &mut BTreeMap<String, Uses>,
) -> Result<(), String> {
    crate::budget::spend(1)?;
    match v {
        Value::String(s) => {
            if let (Some(name), Some(k)) = (s.strip_prefix('$'), keys.last()) {
                crate::budget::spend(name.len() as u64)?;
                let parent = keys.len().checked_sub(2).map(|i| keys[i].as_str());
                let side =
                    SIDES.contains(&k.as_str()) && parent.is_some_and(|p| PIXELS.contains(&p));
                let fraction = keys.iter().any(|k| NORMALIZED.contains(&k.as_str()));
                let pixel = !fraction && (PIXELS.contains(&k.as_str()) || side);
                let label = match (side || fraction, parent) {
                    (true, Some(p)) => format!("{p}.{k}"),
                    _ => k.clone(),
                };
                let entry = out
                    .entry(name.to_string())
                    .or_default()
                    .entry(label)
                    .or_insert((0, pixel));
                entry.0 += 1;
            }
        }
        Value::Array(all) => {
            for x in all {
                readers(x, keys, out)?;
            }
        }
        Value::Object(m) => {
            crate::budget::members(m)?;
            for (k, x) in m {
                crate::budget::spend(k.len() as u64)?;
                keys.push(k.clone());
                readers(x, keys, out)?;
                keys.pop();
            }
        }
        _ => {}
    }
    Ok(())
}

/// The properties as a note says them: `cornerRadius (2), gap`.
fn listed<'a>(uses: impl Iterator<Item = (&'a String, &'a (usize, bool))>) -> String {
    uses.map(|(k, (n, _))| match n {
        1 => k.clone(),
        n => format!("{k} ({n})"),
    })
    .collect::<Vec<_>>()
    .join(", ")
}

/// The name an alias value names, `"$name"`.
fn alias_of(v: &Value) -> Option<&str> {
    v.as_str().and_then(|s| s.strip_prefix('$'))
}

/// A variable's entries: each value and its theme; a scalar is one entry.
type Entries = Vec<(Value, Option<Value>)>;

/// A variable's entries, or why its `value` holds none.
fn entries_of(def: &Value) -> Result<Entries, &'static str> {
    match def.get("value") {
        Some(Value::Array(all)) if all.is_empty() => Err("its `value` is an empty list"),
        Some(Value::Array(all)) => all
            .iter()
            .map(|e| match e {
                Value::Object(o) if o.contains_key("value") => match o.get("theme") {
                    None | Some(Value::Object(_)) => {
                        Ok((o["value"].clone(), o.get("theme").cloned()))
                    }
                    Some(_) => Err("an entry's `theme` is no object of axes and values"),
                },
                _ => Err("an entry is no `{value, theme?}`"),
            })
            .collect(),
        Some(v) => Ok(vec![(v.clone(), None)]),
        None => Err("it has no `value`"),
    }
}

/// The layers a `.pen` document makes, and the notes on the kinds it chose.
fn read(file: &str, doc: &Doc) -> Result<(Read, Vec<String>), String> {
    match doc.version.as_ref().and_then(Value::as_str) {
        Some(v) if v.starts_with("2.") => {}
        Some(v) => {
            return Err(format!(
                "a `.pen` of version {}: this importer reads 2.x",
                v.chars().take(20).collect::<String>()
            ))
        }
        None => return Err("no `version`: is this a Pencil `.pen` file?".into()),
    }
    let mut out = Read {
        flat: true,
        ..Read::default()
    };
    // The axes, in the order written, refused past their bounds before any is registered.
    let themes = doc.themes.as_ref().map_or(&[][..], |t| &t.0[..]);
    if themes.len() > AXES {
        return Err(format!("{} axes in `themes`: at most {AXES}", themes.len()));
    }
    let mut axes: Vec<(String, Vec<String>)> = Vec::new();
    for (axis, values) in themes {
        crate::budget::spend(axis.len() as u64)?;
        let values: Vec<String> = values
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= VALUES)
            .and_then(|v| v.iter().map(|x| x.as_str().map(str::to_string)).collect())
            .ok_or_else(|| format!("`themes.{axis}` is a list of 1 to {VALUES} values"))?;
        for v in &values {
            crate::budget::spend(v.len() as u64)?;
        }
        axes.push((axis.clone(), values));
    }
    let scheme_axis = axes
        .iter()
        .find(|(_, vs)| {
            let mut lower: Vec<String> = vs.iter().map(|v| v.trim().to_ascii_lowercase()).collect();
            lower.sort();
            lower == ["dark", "light"] || lower == ["dark", "default"]
        })
        .map(|(a, _)| a.clone());
    for (axis, values) in &axes {
        if Some(axis) != scheme_axis.as_ref() {
            for v in values {
                out.axis(axis, v);
            }
        }
    }
    let mut readers: BTreeMap<String, Uses> = BTreeMap::new();
    if let Some(children) = &doc.children {
        self::readers(children, &mut Vec::new(), &mut readers)?;
    }
    let vars = doc.variables.as_ref().map_or(&[][..], |v| &v.0[..]);
    let types: BTreeMap<&str, &str> = vars
        .iter()
        .map(|(n, d)| {
            (
                n.as_str(),
                d.get("type").and_then(Value::as_str).unwrap_or(""),
            )
        })
        .collect();
    // A number's kind by its readers and those of every number it aliases or that aliases it: one group, one kind.
    let mut group: BTreeMap<&str, &str> = BTreeMap::new();
    fn root<'a>(g: &BTreeMap<&'a str, &'a str>, mut n: &'a str) -> &'a str {
        while let Some(&p) = g.get(n).filter(|p| **p != n) {
            n = p;
        }
        n
    }
    for (name, def) in vars {
        if types[name.as_str()] != "number" {
            continue;
        }
        group.entry(name).or_insert(name);
        for (v, _) in entries_of(def).unwrap_or_default() {
            crate::budget::spend(1)?;
            if let Some(t) = alias_of(&v).filter(|t| types.get(t) == Some(&"number")) {
                let (t, _) = types.get_key_value(t).unwrap();
                group.entry(t).or_insert(t);
                let (a, b) = (root(&group, name), root(&group, t));
                if a != b {
                    group.insert(a, b);
                }
            }
        }
    }
    let empty = Uses::new();
    let mut joined: BTreeMap<&str, Uses> = BTreeMap::new();
    for &n in group.keys() {
        let into = joined.entry(root(&group, n)).or_default();
        for (k, (c, p)) in readers.get(n).unwrap_or(&empty) {
            let e = into.entry(k.clone()).or_insert((0, *p));
            e.0 += c;
        }
    }
    let mut notes = Vec::new();
    if doc
        .children
        .as_ref()
        .is_some_and(|c| c.as_array().is_some_and(|a| !a.is_empty()))
    {
        notes.push(
            "the file's frames and components are not imported (its variables and themes are)"
                .to_string(),
        );
    }
    for (name, def) in vars {
        crate::budget::spend(1)?;
        let ty = types[name.as_str()];
        let uses = match ty {
            "number" => joined.get(root(&group, name)).unwrap_or(&empty),
            _ => readers.get(name).unwrap_or(&empty),
        };
        let shared = ty == "number"
            && group
                .keys()
                .filter(|n| root(&group, n) == root(&group, name))
                .count()
                > 1;
        let pixels = !uses.is_empty() && uses.values().all(|(_, p)| *p);
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
            let with = if shared {
                " (with the numbers it aliases or that alias it)"
            } else {
                ""
            };
            let pixel = listed(uses.iter().filter(|(_, (_, p))| *p));
            let other = listed(uses.iter().filter(|(_, (_, p))| !*p));
            notes.push(match (pixel.is_empty(), other.is_empty()) {
                (false, true) => format!("`{name}` is a length (px): Pencil reads it{with} as {pixel}"),
                (true, true) => format!("`{name}` is a number: no frame reads it{with}"),
                (true, false) => format!("`{name}` is a number: Pencil reads it{with} as {other}"),
                (false, false) => format!("`{name}` is a number: Pencil reads it{with} as a pixel length ({pixel}) and as {other}, so it is no length"),
            });
        }
        let entries = match entries_of(def) {
            Ok(e) => e,
            Err(why) => {
                crate::budget::spend(name.len() as u64)?;
                let value = def.get("value").cloned().unwrap_or(Value::Null);
                crate::budget::copy(&value)?;
                out.unused(
                    vec![Def {
                        file: file.to_string(),
                        path: name.clone(),
                        ty: None,
                        value,
                        refused: None,
                    }],
                    why,
                );
                continue;
            }
        };
        for (value, theme) in entries {
            crate::budget::spend(2 * name.len() as u64)?;
            crate::budget::copy(&value)?;
            // An alias names another variable, as written, of its own type.
            let mut refused = (as_dtcg.0.is_empty()).then(|| {
                format!(
                    "a Pencil `type` the importer does not name (`{}`)",
                    ty.chars().take(20).collect::<String>()
                )
            });
            let value = match alias_of(&value) {
                Some(target) => {
                    match types.get(target) {
                        None => {
                            refused.get_or_insert(format!(
                                "an alias to `{}`, which `variables` does not declare",
                                target.chars().take(80).collect::<String>()
                            ));
                        }
                        Some(t) if *t != ty => {
                            refused.get_or_insert(format!(
                                "an alias to `{target}`, a {t}, from a {ty}"
                            ));
                        }
                        _ => {}
                    }
                    Value::String(format!("{{{target}}}"))
                }
                None => (as_dtcg.1)(&value),
            };
            // Its scope: every axis its theme names at once; one undeclared, never.
            let mut scopes = Vec::new();
            let mut said = Vec::new();
            let mut undeclared = None;
            for (axis, v) in theme
                .as_ref()
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let v = v.as_str().unwrap_or_default();
                crate::budget::spend((axis.len() + v.len()) as u64)?;
                said.push(format!("{axis}={v}"));
                if !axes
                    .iter()
                    .any(|(a, vs)| a == axis && vs.iter().any(|x| x == v))
                {
                    undeclared.get_or_insert(format!("its theme names `{axis}={v}`, which `themes` does not declare, so it never holds"));
                    continue;
                }
                scopes.push(match &scheme_axis {
                    Some(s) if s == axis => Scope::Scheme(is_dark(v)),
                    _ => out.axis(axis, v),
                });
            }
            let block = match said.is_empty() {
                true => file.to_string(),
                false => format!("{file} ({})", said.join(", ")),
            };
            let def = Def {
                file: block,
                path: name.clone(),
                ty: Some(as_dtcg.0.to_string()).filter(|t| !t.is_empty()),
                value,
                refused,
            };
            if let Some(why) = undeclared {
                out.unused(vec![def], &why);
                continue;
            }
            let scope = match scopes.len() {
                0 => Scope::Base,
                1 => scopes.remove(0),
                _ => Scope::All(scopes),
            };
            out.layer(vec![scope], vec![def]);
        }
    }
    // Another `.pen`'s variables: reported, each alias once.
    if let Some(imports) = &doc.imports {
        let mut found = Vec::new();
        for (alias, to) in &imports.0 {
            crate::budget::spend(alias.len() as u64)?;
            crate::budget::copy(to)?;
            found.push(Def {
                file: file.to_string(),
                path: format!("imports.{alias}"),
                ty: None,
                value: to.clone(),
                refused: None,
            });
        }
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
    let doc: Doc = serde_json::from_str(text).map_err(|e| refuse(format!("{file}: {e}")))?;
    let (read, notes) = read(file, &doc).map_err(|m| refuse(format!("{file}: {m}")))?;
    let mut import = planned(read, prefix, design, vocab)?;
    import.notes = notes;
    Ok(import)
}
