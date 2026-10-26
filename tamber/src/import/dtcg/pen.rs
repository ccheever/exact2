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

/// The node properties Pencil reads as pixels (an effect's `radius` and a
/// shadow's `blur` among them).
const PIXELS: [&str; 10] = [
    "cornerRadius",
    "gap",
    "padding",
    "fontSize",
    "letterSpacing",
    "strokeWidth",
    "width",
    "height",
    "radius",
    "blur",
];
/// A pixel property's per-side or per-corner members (`strokeWidth: {top, …}`).
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
/// A typed object's maps of the author's own names (a shader's `uniforms`,
/// a script's `inputs`, any `metadata`): no key in one is a property, so
/// nothing in one decides a kind.
const OPAQUE: [&str; 3] = ["uniforms", "inputs", "metadata"];
/// The other properties that take a variable (Pencil's schema names them:
/// paints and strokes, a mesh's `colors`, opacity, rotation, the flags
/// (`enabled`, `flipX`/`flipY`, `clip`, `underline`, `strikethrough`), the
/// type longhands and `content`, an ellipse's angles and ratio, a polygon's
/// count, an icon's names and weight, and a gradient stop's `position`,
/// that one only there). A `$…` under any other key (an id, a layout enum,
/// any other position, a `context`) binds nothing and decides no kind.
const BINDABLE: [&str; 26] = [
    "fill",
    "stroke",
    "colors",
    "color",
    "opacity",
    "rotation",
    "enabled",
    "flipX",
    "flipY",
    "clip",
    "underline",
    "strikethrough",
    "fontWeight",
    "fontStyle",
    "lineHeight",
    "fontFamily",
    "content",
    "innerRadius",
    "startAngle",
    "sweepAngle",
    "polygonCount",
    "weight",
    "position",
    "icon",
    "library",
    "model",
];
/// A variable's name, at most: Pencil's are short, and a report says one in
/// every message about it.
const NAME: usize = 128;
/// A literal value's bytes, at most (a colour, a family, a string).
const VALUE: usize = 1024;
/// An axis's or value's name, at most (a report says it in every tuple's words).
const AXIS_NAME: usize = 64;
/// The axes, and values of one, a file may declare.
const AXES: usize = 16;
const VALUES: usize = 64;
/// A file name as a report's block says it, at most.
const LABEL: usize = 120;

impl Scope {
    /// Whether it holds in tuple `t` (`axes` the proposed axes in order):
    /// an `All` when every scope it holds does.
    pub(super) fn holds(&self, t: &super::Tuple, axes: &[(String, Vec<String>)]) -> bool {
        match self {
            Scope::Base => true,
            Scope::Scheme(dark) => *dark == t.0,
            Scope::Axis(a, v) => axes
                .iter()
                .position(|(n, _)| n == a)
                .is_some_and(|i| t.1[i] == *v),
            Scope::All(all) => all.iter().all(|s| s.holds(t, axes)),
        }
    }

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

/// Where a value sits: its key, its parent's key, the `type` of the object
/// holding it and of the one holding that (a shadow's, for its `offset`),
/// and whether it is inside a gradient (whose sizes and centres are
/// fractions of the box) or a map of the author's own names.
#[derive(Clone, Copy, Default)]
struct At<'a> {
    key: Option<&'a str>,
    parent: Option<&'a str>,
    holder: Option<&'a str>,
    outer: Option<&'a str>,
    gradient: bool,
    opaque: bool,
    /// This object is a `descendants` map: its keys are node ids.
    ids: bool,
}

/// Each variable's readers in the node tree (`"$name"`), by the property
/// that reads it: its own key, a side's (`strokeWidth.top`), a shadow
/// offset's (`offset.x`); every value visited, and every name, charged to
/// the budget.
fn readers(v: &Value, at: At, out: &mut BTreeMap<String, Uses>) -> Result<(), String> {
    crate::budget::spend(1)?;
    match v {
        Value::String(s) => {
            if let (Some(name), Some(k)) = (s.strip_prefix('$'), at.key) {
                let side = (SIDES.contains(&k) && at.parent.is_some_and(|p| PIXELS.contains(&p)))
                    || (matches!(k, "x" | "y")
                        && at.parent == Some("offset")
                        && at.outer == Some("shadow"));
                // (a use only where Pencil binds a variable: a schema property, one's side, or a map of the
                // author's names)
                // (and `position` only as a gradient stop's)
                let stop = at.gradient && at.parent == Some("colors");
                if !(at.opaque
                    || side
                    || PIXELS.contains(&k)
                    || BINDABLE.contains(&k) && (k != "position" || stop))
                {
                    return Ok(());
                }
                let pixel = !at.gradient && !at.opaque && (PIXELS.contains(&k) || side);
                let label = match (side || at.opaque || at.gradient, at.parent) {
                    (true, Some(p)) => format!("{p}.{k}"),
                    _ => k.to_string(),
                };
                crate::budget::spend((name.len() + label.len()) as u64)?;
                let entry = out
                    .entry(name.to_string())
                    .or_default()
                    .entry(label)
                    .or_insert((0, pixel));
                entry.0 += 1;
                // (one property read as pixels in one place and not in another is no pixel property)
                entry.1 &= pixel;
            }
        }
        Value::Array(all) => {
            for x in all {
                readers(x, at, out)?;
            }
        }
        Value::Object(m) => {
            crate::budget::spend(m.len() as u64)?;
            // (a `descendants` map's keys are node ids: none is its type)
            let ty = (!at.ids)
                .then(|| m.get("type").and_then(Value::as_str))
                .flatten();
            let gradient = at.gradient || ty == Some("gradient");
            // (a node's overrides, a `descendants` map's value, hold properties as a typed node does)
            let holder = ty.or((at.key == Some("descendants") && !at.ids).then_some("override"));
            // (a map of the author's names only as a typed object's property: a descendant's id is none)
            let opaque =
                at.opaque || (at.holder.is_some() && at.key.is_some_and(|p| OPAQUE.contains(&p)));
            for (k, x) in m {
                crate::budget::spend(k.len() as u64)?;
                // (a node's `theme` names axes and their values, literals all: no variable is bound there;
                // in a `descendants` map the key is a node's id, its overrides read as any node's)
                if k == "theme" && !opaque && !at.ids {
                    continue;
                }
                // (and a `descendants` map's value is a node's overrides, an object: anything else binds nothing)
                if at.ids && !x.is_object() {
                    continue;
                }
                // (a node's overrides are read as a node is: its id no property, their parent none of them)
                let inner = At {
                    key: if at.ids { Some("descendants") } else { Some(k) },
                    parent: at.key,
                    holder,
                    outer: at.holder,
                    gradient,
                    opaque,
                    ids: k == "descendants" && !at.ids && !opaque,
                };
                readers(x, inner, out)?;
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

/// One entry of a variable: its value and theme, or why it is no entry.
type Entry<'a> = Result<(&'a Value, Option<&'a Value>), &'static str>;

/// A variable's entries, each on its own (a scalar is one); or why its
/// `value` holds none.
fn entries_of(def: &Value) -> Result<Vec<Entry<'_>>, &'static str> {
    match def.get("value") {
        Some(Value::Array(all)) if all.is_empty() => Err("its `value` is an empty list"),
        Some(Value::Array(all)) => Ok(all
            .iter()
            .map(|e| match e {
                Value::Object(o) if o.contains_key("value") => match o.get("theme") {
                    None | Some(Value::Object(_)) => Ok((&o["value"], o.get("theme"))),
                    Some(_) => Err("an entry's `theme` is no object of axes and values"),
                },
                _ => Err("an entry is no `{value, theme?}`"),
            })
            .collect()),
        Some(v) => Ok(vec![Ok((v, None))]),
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
        if axis.chars().count() > AXIS_NAME {
            return Err(format!("an axis named in more than {AXIS_NAME} characters"));
        }
        let values: Vec<String> = values
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= VALUES)
            .and_then(|v| v.iter().map(|x| x.as_str().map(str::to_string)).collect())
            .ok_or_else(|| format!("`themes.{axis}` is a list of 1 to {VALUES} values"))?;
        for v in &values {
            crate::budget::spend(v.len() as u64)?;
            if v.chars().count() > AXIS_NAME {
                return Err(format!(
                    "`themes.{}` names a value in more than {AXIS_NAME} characters",
                    axis.chars().take(AXIS_NAME).collect::<String>()
                ));
            }
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
        self::readers(children, At::default(), &mut readers)?;
    }
    // The file's name as each report says it: bounded, so a long one is no multiplier.
    let label: String = file.chars().take(LABEL).collect();
    crate::budget::spend(label.len() as u64)?;
    // The variables: a name declared twice keeps its last, the earlier reported.
    let all = doc.variables.as_ref().map_or(&[][..], |v| &v.0[..]);
    let mut last: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, (n, _)) in all.iter().enumerate() {
        crate::budget::spend(n.len() as u64)?;
        last.insert(n, i);
    }
    // (a type as compared and said: its first 32 characters, no Pencil type being longer, so a long unknown one,
    // reported anyway, is no multiplier where every alias of it compares it)
    let types: BTreeMap<&str, &str> = all
        .iter()
        .map(|(n, d)| {
            let t = d.get("type").and_then(Value::as_str).unwrap_or("");
            (
                n.as_str(),
                &t[..t.char_indices().nth(32).map_or(t.len(), |(i, _)| i)],
            )
        })
        .collect();
    // A number's kind by its readers and those of every number it aliases or that aliases it: one group (a union
    // of roots, each walk compressed, every step charged), one kind.
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    fn root<'a>(p: &mut BTreeMap<&'a str, &'a str>, n: &'a str) -> Result<&'a str, String> {
        let mut r = n;
        while let Some(&up) = p.get(r).filter(|up| **up != r) {
            crate::budget::spend(1)?;
            r = up;
        }
        let mut at = n;
        while at != r {
            let up = p[at];
            p.insert(at, r);
            at = up;
        }
        Ok(r)
    }
    for (i, (name, def)) in all.iter().enumerate() {
        // (only the declaration that stands: a discarded one aliases nothing)
        if types[name.as_str()] != "number" || last[name.as_str()] != i {
            continue;
        }
        parent.entry(name).or_insert(name);
        for (v, _) in entries_of(def).unwrap_or_default().into_iter().flatten() {
            crate::budget::spend(1)?;
            if let Some((t, _)) = alias_of(v)
                .and_then(|t| types.get_key_value(t))
                .filter(|(_, ty)| **ty == "number")
            {
                parent.entry(t).or_insert(t);
                let (a, b) = (root(&mut parent, name)?, root(&mut parent, t)?);
                if a != b {
                    parent.insert(a, b);
                }
            }
        }
    }
    let empty = Uses::new();
    let mut joined: BTreeMap<&str, (Uses, usize)> = BTreeMap::new();
    let members: Vec<&str> = parent.keys().copied().collect();
    for n in members {
        let r = root(&mut parent, n)?;
        let (into, count) = joined.entry(r).or_default();
        *count += 1;
        for (k, (c, p)) in readers.get(n).unwrap_or(&empty) {
            crate::budget::spend(k.len() as u64)?;
            let e = into.entry(k.clone()).or_insert((0, *p));
            e.0 += c;
            e.1 &= *p;
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
    let none = (Uses::new(), 0);
    for (i, (name, def)) in all.iter().enumerate() {
        crate::budget::spend(1)?;
        if last[name.as_str()] != i {
            // (each of its entries reported, so the count is every entry the file holds)
            for _ in 0..entries_of(def).map_or(1, |e| e.len()) {
                crate::budget::spend(name.len() as u64)?;
                out.unused(
                    vec![Def {
                        file: label.clone(),
                        path: name.clone(),
                        ty: None,
                        value: Value::Null,
                        refused: None,
                    }],
                    "declared again later in `variables`, which stands",
                );
            }
            continue;
        }
        // (a name past its bound is reported, entry by entry, and never a token another can name)
        if name.chars().count() > NAME {
            for _ in 0..entries_of(def).map_or(1, |e| e.len()) {
                crate::budget::spend(name.len() as u64)?;
                out.unused(
                    vec![Def {
                        file: label.clone(),
                        path: name.clone(),
                        ty: None,
                        value: Value::Null,
                        refused: None,
                    }],
                    &format!("a name of more than {NAME} characters"),
                );
            }
            continue;
        }
        let ty = types[name.as_str()];
        let (uses, shared) = match ty {
            "number" => {
                let (u, n) = joined.get(root(&mut parent, name)?).unwrap_or(&none);
                (u, *n > 1)
            }
            _ => (readers.get(name).unwrap_or(&empty), false),
        };
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
            let note = match (pixel.is_empty(), other.is_empty()) {
                (false, true) => format!("`{name}` is a length (px): Pencil reads it{with} as {pixel}"),
                (true, true) => format!("`{name}` is a number: no frame reads it{with}"),
                (true, false) => format!("`{name}` is a number: Pencil reads it{with} as {other}"),
                (false, false) => format!("`{name}` is a number: Pencil reads it{with} as a pixel length ({pixel}) and as {other}, so it is no length"),
            };
            crate::budget::spend(note.len() as u64)?;
            notes.push(note);
        }
        let entries = match entries_of(def) {
            Ok(e) => e,
            Err(why) => {
                crate::budget::spend(name.len() as u64)?;
                out.unused(
                    vec![Def {
                        file: label.clone(),
                        path: name.clone(),
                        ty: None,
                        value: Value::Null,
                        refused: None,
                    }],
                    why,
                );
                continue;
            }
        };
        for entry in entries {
            crate::budget::spend(name.len() as u64)?;
            // (each entry counted on its own: one of no shape reported, the rest read)
            let (value, theme) = match entry {
                Ok(e) => e,
                Err(why) => {
                    out.unused(
                        vec![Def {
                            file: label.clone(),
                            path: name.clone(),
                            ty: None,
                            value: Value::Null,
                            refused: None,
                        }],
                        why,
                    );
                    continue;
                }
            };
            crate::budget::copy(value)?;
            let mut refused = (as_dtcg.0.is_empty()).then(|| {
                format!(
                    "a Pencil `type` the importer does not name (`{}`)",
                    ty.chars().take(20).collect::<String>()
                )
            });
            // An alias names another variable, as written, of its own type; a literal that DTCG would read as one
            // (`{x}`) can't be said, nor an alias to a name DTCG can't hold (`{` or `}` in it).
            let value = match alias_of(value) {
                Some(target) if target.chars().count() > NAME => {
                    refused.get_or_insert(format!(
                        "an alias to a name of more than {NAME} characters"
                    ));
                    Value::Null
                }
                Some(target) if target.contains(['{', '}']) => {
                    refused.get_or_insert(format!(
                        "an alias to `{}`, a name with a brace, which no token can follow",
                        target.chars().take(80).collect::<String>()
                    ));
                    Value::Null
                }
                Some(target) => {
                    match types.get(target) {
                        None => {
                            refused.get_or_insert(format!(
                                "an alias to `{}`, which `variables` does not declare",
                                target.chars().take(80).collect::<String>()
                            ));
                        }
                        Some(t) if *t != ty => {
                            let short = |x: &str| x.chars().take(20).collect::<String>();
                            refused.get_or_insert(format!(
                                "an alias to `{}`, a {}, from a {}",
                                target.chars().take(80).collect::<String>(),
                                short(t),
                                short(ty)
                            ));
                        }
                        _ => {}
                    }
                    Value::String(format!("{{{target}}}"))
                }
                // (a Pencil value is a scalar: an object or a list, which DTCG would read as an alias or a
                // composite, is none)
                None if value.is_object() || value.is_array() => {
                    refused.get_or_insert(
                        "a value of no Pencil shape (a colour, number, string or boolean)"
                            .to_string(),
                    );
                    Value::Null
                }
                // (a colour, family or string as written is at most 1024 bytes: a token's value is copied wherever
                // it is laid, probed and marked, so a longer one would multiply)
                None if value.as_str().is_some_and(|s| s.len() > VALUE) => {
                    refused.get_or_insert(format!("a value of more than {VALUE} bytes"));
                    Value::Null
                }
                None if value.as_str().is_some_and(|s| {
                    s.trim_start().starts_with('{') && s.trim_end().ends_with('}')
                }) =>
                {
                    refused.get_or_insert(
                        "a literal in braces, which the importer would read as an alias"
                            .to_string(),
                    );
                    value.clone()
                }
                // (a Pencil number is a number: a string under one, which DTCG would read as a CSS expression
                // for the design to fold, is none)
                None if ty == "number" && !value.is_number() => {
                    refused.get_or_insert("a number variable whose value is no number".to_string());
                    Value::Null
                }
                None => (as_dtcg.1)(value),
            };
            // Its scope: every axis its theme names at once; one undeclared, never.
            let mut scopes = Vec::new();
            let mut said = Vec::new();
            let mut undeclared = None;
            for (axis, v) in theme.and_then(Value::as_object).into_iter().flatten() {
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
                true => label.clone(),
                false => format!("{label} ({})", said.join(", ")),
            };
            crate::budget::spend(block.len() as u64)?;
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
                file: label.clone(),
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
    // (a prefix starts every token name, each copied in every tuple's result: bounded as a name is)
    if prefix.chars().count() > NAME {
        return Err(refuse(format!("a prefix is at most {NAME} characters")));
    }
    let _work = crate::budget::Work::begin(limit);
    crate::budget::spend(text.len() as u64).map_err(refuse)?;
    let doc: Doc = serde_json::from_str(text).map_err(|e| refuse(format!("{file}: {e}")))?;
    // Its themes are axes: past this importer's own count, refused with the
    // design's id for too many axes (`tamber-axes`, theme2's 19:26).
    let themes = doc.themes.as_ref().map_or(0, |t| t.0.len());
    if themes > AXES {
        let why = format!("{file}: {themes} axes in `themes`: at most {AXES}");
        return Err(vec![Diagnostic::new("tamber-axes", why, 1, 1)]);
    }
    // A value past the checker's bound refuses the import with that bound's
    // id (`super::super::source_bound`); one past this importer's own 1024
    // bytes and within it is reported, as before. A value only: not a
    // variable's `type`, nor an alias (`$name`), whose name this importer
    // reports past 128 characters before any checker reads it.
    for (name, v) in doc.variables.as_ref().map_or(&[][..], |v| &v.0[..]) {
        // The value, or each entry's value in a themed list (not its
        // `theme`'s labels).
        let mut strings = Vec::new();
        match v.get("value") {
            Some(Value::Array(entries)) => entries
                .iter()
                .filter_map(|e| e.get("value"))
                .for_each(|e| super::super::json_strings(e, &mut strings)),
            Some(value) => super::super::json_strings(value, &mut strings),
            None => {}
        }
        super::super::source_bound(
            strings
                .into_iter()
                .filter(|s| !s.starts_with('$'))
                .map(|s| (name.as_str(), s)),
        )?;
    }
    let (read, notes) = read(file, &doc).map_err(|m| refuse(format!("{file}: {m}")))?;
    let mut import = planned(read, prefix, design, vocab)?;
    import.notes = notes;
    Ok(import)
}
