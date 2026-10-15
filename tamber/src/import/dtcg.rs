//! DTCG JSON (LLP 1121 D14, importer 3): Format 2025.10, with the Resolver
//! Module where present, from Tokens Studio and from Figma Variables
//! exports. The source is a set of files read as layers, each the base, one
//! side of the scheme, or one value of a proposed axis, in precedence order:
//!
//! - **Figma:** `<Collection>.<Mode>.json`. A collection whose modes are
//!   `Light` and `Dark` is the scheme; one with one mode is the base; any
//!   other's modes are a proposed axis named for the collection, its first
//!   mode (by name) the default. Any other file is the base.
//! - **Tokens Studio:** `$themes.json` and its token sets, or one file holding
//!   them all. A group of one theme is the base; a group whose themes are
//!   `light` and `dark` is the scheme; any other group (ungrouped themes are
//!   one group) a proposed axis. A theme's `source` sets come before its
//!   `enabled` ones, each in `$metadata.tokenSetOrder`; a set no theme turns
//!   on is reported. The legacy `value`/`type` form is read as
//!   `$value`/`$type`; a value Tokens Studio modifies (`studio.tokens`
//!   `modify`) is reported.
//! - **A resolver** (`resolutionOrder`): each set the base, a modifier whose
//!   contexts are `light` and `dark` the scheme, any other a proposed axis;
//!   references resolved (`#/sets/…`, files relative to the document),
//!   sibling keys overriding what they name, in the order the document gives.
//! - **Any other DTCG file:** the base.
//!
//! Each mode tuple (a side of the scheme with a value of every axis) is
//! resolved on its own, its layers merged in order (a later one winning,
//! whatever its kind), and every `{a.b}` or `$ref` alias followed there. A
//! token is a colour in every tuple or makes no token; its default is the
//! default tuple's (a `light-dark()` of the scheme's sides), and each other
//! tuple Tamber would not otherwise select its value in is a `when` line.
//! Other `$type`s are `kind pending (stage 6)` with the token each would
//! become; borders, transitions, gradients and stroke styles are reported, as
//! are strings and booleans. Names come from group paths, each segment made
//! an identifier (`$root` is `root`); collisions are reported.

use super::{colour_literal, counted, reconcile, Import, Mapped, Unmapped};
use crate::{Design, Diagnostic};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Where a layer applies.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Base,
    /// One side of the scheme: `true` dark.
    Scheme(bool),
    /// One value of a proposed axis.
    Axis(String, String),
}

/// One token as a source declares it.
#[derive(Debug, Clone)]
struct Def {
    /// Where it came from, for the report.
    file: String,
    path: String,
    ty: Option<String>,
    value: Value,
    /// Why it cannot be mapped whatever its type (a Tokens Studio modifier).
    refused: Option<String>,
}

/// A layer: where it applies, and its tokens (indices into the defs).
struct Layer {
    scope: Scope,
    defs: Vec<usize>,
}

/// What the files make: every def, the layers in precedence order, and the
/// defs no layer uses (a set no theme turns on), with why.
#[derive(Default)]
struct Read {
    defs: Vec<Def>,
    layers: Vec<Layer>,
    unused: Vec<(usize, String)>,
}

impl Read {
    fn layer(&mut self, scope: Scope, found: Vec<Def>) {
        let start = self.defs.len();
        self.defs.extend(found);
        self.layers.push(Layer {
            scope,
            defs: (start..self.defs.len()).collect(),
        });
    }
}

/// A name segment as an identifier: spaces and other characters a name may
/// not hold become `-`, a leading digit takes `s`; `$root` is `root`.
fn segment(raw: &str) -> String {
    if raw == "$root" {
        return "root".into();
    }
    let mut out: String = raw
        .trim()
        .chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                true => c,
                false => '-',
            },
        )
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let out = out.trim_matches('-').to_string();
    match out.chars().next() {
        Some(c) if c.is_ascii_digit() => format!("s{out}"),
        Some(_) => out,
        None => "x".into(),
    }
}

/// An axis or axis value's name: a segment, lowercased.
fn ident(raw: &str) -> String {
    segment(raw).to_ascii_lowercase()
}

/// A Tokens Studio legacy token: `value` beside a `type` string, or a
/// `value` that is no group (a DTCG member may be named `value`).
fn legacy(map: &Map<String, Value>) -> bool {
    !map.contains_key("$value")
        && map
            .get("value")
            .is_some_and(|v| map.get("type").is_some_and(Value::is_string) || !v.is_object())
}

/// A group `$extends` or `$ref` names: `{a.b}` or a JSON pointer.
fn group<'a>(root: &'a Value, target: &str) -> Option<&'a Value> {
    let t = target.trim();
    if let Some(pointer) = t.strip_prefix('#') {
        return root.pointer(pointer);
    }
    let path = t.strip_prefix('{')?.strip_suffix('}')?;
    path.split('.').try_fold(root, |node, seg| node.get(seg))
}

/// Every token in a tree: its dotted path and definition, `$type` (or a
/// legacy group `type`) inherited from groups, `$root` a token of its group,
/// `$extends` and a group's `$ref` merged under it (its own members
/// overriding).
fn tokens(file: &str, tree: &Value) -> Vec<Def> {
    fn walk(
        file: &str,
        root: &Value,
        node: &Value,
        path: &mut Vec<String>,
        ty: Option<String>,
        depth: usize,
        out: &mut Vec<Def>,
    ) {
        let Some(map) = node.as_object() else { return };
        if depth > 32 {
            return;
        }
        let is_token = map.contains_key("$value") || legacy(map);
        let own_ty = map
            .get("$type")
            .and_then(Value::as_str)
            .or_else(|| {
                map.get("type")
                    .filter(|_| !map.contains_key("$value"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string);
        // A group it extends lends its type when it has none of its own.
        let lent = ["$extends", "$ref"].iter().find_map(|k| {
            let target = map.get(*k)?.as_str()?;
            group(root, target)?
                .get("$type")?
                .as_str()
                .map(str::to_string)
        });
        let ty = own_ty.or(lent).or(ty);
        if is_token {
            let value = map
                .get("$value")
                .or_else(|| map.get("value"))
                .cloned()
                .unwrap_or(Value::Null);
            let refused = map
                .get("$extensions")
                .and_then(|e| e.get("studio.tokens"))
                .and_then(|s| s.get("modify"))
                .map(|_| {
                    "a value Tokens Studio modifies (`modify`), which no Tamber value is yet"
                        .to_string()
                });
            out.push(Def {
                file: file.to_string(),
                path: path.join("."),
                ty,
                value,
                refused,
            });
            return;
        }
        // What the group inherits, then its own members over it.
        let mut members: Vec<(String, Value)> = Vec::new();
        for key in ["$extends", "$ref"] {
            if let Some(target) = map.get(key).and_then(Value::as_str) {
                if let Some(inherited) = group(root, target).and_then(Value::as_object) {
                    for (k, v) in inherited {
                        if !k.starts_with('$') || k == "$root" {
                            members.push((k.clone(), v.clone()));
                        }
                    }
                }
            }
        }
        for (k, v) in map {
            if k.starts_with('$') && k != "$root" {
                continue;
            }
            members.retain(|(m, _)| m != k);
            members.push((k.clone(), v.clone()));
        }
        for (k, v) in members {
            path.push(k);
            walk(file, root, &v, path, ty.clone(), depth + 1, out);
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(file, tree, tree, &mut Vec::new(), None, 0, &mut out);
    out
}

/// Whether mode names are the scheme's sides.
fn scheme(modes: &[String]) -> bool {
    let mut lower: Vec<String> = modes
        .iter()
        .map(|m| m.trim().to_ascii_lowercase())
        .collect();
    lower.sort();
    lower == ["dark", "light"]
}

fn is_dark(mode: &str) -> bool {
    mode.trim().eq_ignore_ascii_case("dark")
}

fn stem(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    base.strip_suffix(".json").unwrap_or(base).to_string()
}

/// The layers a source's files make, or why it cannot be read.
fn read(files: &[(String, String)]) -> Result<Read, String> {
    let parsed: Vec<(String, Value)> = files
        .iter()
        .map(|(name, text)| {
            serde_json::from_str(text)
                .map(|v| (name.clone(), v))
                .map_err(|e| format!("{name}: {e}"))
        })
        .collect::<Result<_, _>>()?;
    let themes_file = parsed.iter().find(|(n, _)| stem(n) == "$themes");
    let single = parsed.iter().find(|(_, v)| v.get("$themes").is_some());
    if themes_file.is_some() || single.is_some() {
        let (themes, metadata, sets): (Value, Option<Value>, Vec<(String, Value)>) = match single {
            Some((_, v)) => (
                v["$themes"].clone(),
                v.get("$metadata").cloned(),
                v.as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(k, _)| !k.starts_with('$'))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
            None => (
                themes_file.map(|(_, v)| v.clone()).unwrap_or(Value::Null),
                parsed
                    .iter()
                    .find(|(n, _)| stem(n) == "$metadata")
                    .map(|(_, v)| v.clone()),
                parsed
                    .iter()
                    .filter(|(n, _)| !stem(n).starts_with('$'))
                    .map(|(n, v)| (n.strip_suffix(".json").unwrap_or(n).to_string(), v.clone()))
                    .collect(),
            ),
        };
        return Ok(studio(&themes, metadata.as_ref(), &sets));
    }
    if let Some((name, doc)) = parsed
        .iter()
        .find(|(_, v)| v.get("resolutionOrder").is_some())
    {
        return resolver(name, doc, &parsed);
    }
    // Figma's `<Collection>.<Mode>.json`, each file on its own; any other
    // file is the base.
    let mut out = Read::default();
    let mut collections: BTreeMap<String, Vec<(String, &Value, String)>> = BTreeMap::new();
    let mut plain: Vec<(&String, &Value)> = Vec::new();
    for (n, v) in &parsed {
        match stem(n).rsplit_once('.') {
            Some((c, m)) if !c.is_empty() && !m.is_empty() => collections
                .entry(c.to_string())
                .or_default()
                .push((m.to_string(), v, n.clone())),
            _ => plain.push((n, v)),
        }
    }
    for (n, v) in plain {
        out.layer(Scope::Base, tokens(n, v));
    }
    for (collection, mut modes) in collections {
        modes.sort_by(|a, b| a.0.cmp(&b.0));
        let names: Vec<String> = modes.iter().map(|(m, _, _)| m.clone()).collect();
        for (mode, v, file) in &modes {
            let scope = match (modes.len(), scheme(&names)) {
                (1, _) => Scope::Base,
                (_, true) => Scope::Scheme(is_dark(mode)),
                _ => Scope::Axis(ident(&collection), ident(mode)),
            };
            out.layer(scope, tokens(file, v));
        }
    }
    // The base, then the scheme, then the axes: a collection's own modes over
    // what every mode shares.
    out.layers.sort_by_key(|l| match l.scope {
        Scope::Base => 0,
        Scope::Scheme(_) => 1,
        Scope::Axis(..) => 2,
    });
    Ok(out)
}

/// A Tokens Studio theme: its name and its sets, `source` ones first.
type Theme = (String, Vec<String>);

/// Tokens Studio's themes: each group a dimension (base, scheme or axis).
fn studio(themes: &Value, metadata: Option<&Value>, sets: &[(String, Value)]) -> Read {
    let order: Vec<String> = metadata
        .and_then(|m| m.get("tokenSetOrder"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    // A set's place: its exact id in `tokenSetOrder`, else its last segment
    // when that names one set only; sets it does not name keep their order
    // after the ones it does.
    let rank = |set: &str| {
        if let Some(i) = order.iter().position(|o| o == set) {
            return i;
        }
        let last = set.rsplit('/').next().unwrap_or(set);
        let alike = sets
            .iter()
            .filter(|(n, _)| n.rsplit('/').next() == Some(last))
            .count();
        match (alike, order.iter().position(|o| o == last)) {
            (1, Some(i)) => i,
            _ => order.len(),
        }
    };
    let mut out = Read::default();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let list: Vec<&Value> = themes
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    if list.is_empty() {
        // No themes: every set is the base, in order.
        let mut all: Vec<&(String, Value)> = sets.iter().collect();
        all.sort_by_key(|(n, _)| rank(n));
        let mut found = Vec::new();
        for (n, v) in all {
            found.extend(tokens(n, v));
        }
        out.layer(Scope::Base, found);
        return out;
    }
    let mut groups: Vec<(String, Vec<Theme>)> = Vec::new();
    for t in list {
        let name = t["name"].as_str().unwrap_or("").to_string();
        // Ungrouped themes are one group.
        let group = t["group"].as_str().unwrap_or("").to_string();
        let mut on: Vec<(u8, String)> = t["selectedTokenSets"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, s)| match s.as_str() {
                Some("source") => Some((0, k.clone())),
                Some("enabled") => Some((1, k.clone())),
                _ => None,
            })
            .collect();
        // `source` sets under `enabled` ones, each in the set order.
        on.sort_by_key(|(status, k)| (*status, rank(k)));
        let on: Vec<String> = on.into_iter().map(|(_, k)| k).collect();
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, members)) => members.push((name, on)),
            None => groups.push((group, vec![(name, on)])),
        }
    }
    for (group, members) in &groups {
        let names: Vec<String> = members.iter().map(|(n, _)| n.clone()).collect();
        for (theme, on) in members {
            let scope = match (members.len(), scheme(&names)) {
                (1, _) => Scope::Base,
                (_, true) => Scope::Scheme(is_dark(theme)),
                _ => Scope::Axis(
                    ident(if group.is_empty() { "theme" } else { group }),
                    ident(theme),
                ),
            };
            let mut found = Vec::new();
            for set in on {
                if let Some((n, v)) = sets.iter().find(|(n, _)| n == set) {
                    used.insert(n.clone());
                    found.extend(tokens(n, v));
                }
            }
            out.layer(scope, found);
        }
    }
    out.layers.sort_by_key(|l| l.scope != Scope::Base);
    // A set no theme turns on: its tokens counted and reported.
    for (n, v) in sets.iter().filter(|(n, _)| !used.contains(n)) {
        for def in tokens(n, v) {
            out.unused
                .push((out.defs.len(), "in a set no theme turns on".into()));
            out.defs.push(def);
        }
    }
    out
}

/// A reference object resolved against `doc` and the source files: the node
/// it names, its sibling keys merged over it, its own references followed;
/// and the file it came from (empty for `doc` itself).
fn deref(
    doc: &Value,
    files: &[(String, Value)],
    relative: &dyn Fn(&str) -> String,
    node: &Value,
    depth: usize,
) -> Result<(String, Value), String> {
    if depth > 16 {
        return Err("references nest too deep".into());
    }
    let Some(r) = node.get("$ref").and_then(Value::as_str) else {
        return Ok((String::new(), node.clone()));
    };
    let (file, pointer) = r.split_once('#').map_or((r, ""), |(f, p)| (f, p));
    let (from, base) = match file {
        "" => (String::new(), doc),
        f => {
            let path = relative(f);
            let (n, v) = files
                .iter()
                .find(|(n, _)| *n == path)
                .ok_or_else(|| format!("no file `{path}` among the sources"))?;
            (n.clone(), v)
        }
    };
    let target = match pointer {
        "" => base,
        p => base.pointer(p).ok_or_else(|| format!("no `{r}`"))?,
    };
    let (inner, mut value) = deref(doc, files, relative, target, depth + 1)?;
    if let (Some(over), Some(obj)) = (node.as_object(), value.as_object_mut()) {
        for (k, v) in over {
            if k != "$ref" {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    Ok((if inner.is_empty() { from } else { inner }, value))
}

/// A Resolver Module document.
fn resolver(name: &str, doc: &Value, files: &[(String, Value)]) -> Result<Read, String> {
    let dir = name.rsplit_once('/').map_or("", |(d, _)| d);
    // A path relative to the document, normalised.
    let relative = |r: &str| {
        let mut parts: Vec<&str> = match dir {
            "" => Vec::new(),
            d => d.split('/').collect(),
        };
        for seg in r.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                s => parts.push(s),
            }
        }
        parts.join("/")
    };
    let source = |src: &Value| -> Result<Vec<Def>, String> {
        let (from, tree) = deref(doc, files, &relative, src, 0)?;
        // A set reached by reference brings its own sources.
        if let Some(srcs) = tree.get("sources").and_then(Value::as_array) {
            let mut out = Vec::new();
            for s in srcs {
                let (f, t) = deref(doc, files, &relative, s, 0)?;
                out.extend(tokens(if f.is_empty() { name } else { &f }, &t));
            }
            return Ok(out);
        }
        Ok(tokens(if from.is_empty() { name } else { &from }, &tree))
    };
    let mut out = Read::default();
    let order = doc["resolutionOrder"]
        .as_array()
        .ok_or("`resolutionOrder` is a list")?;
    for entry in order {
        let (_, node) =
            deref(doc, files, &relative, entry, 0).map_err(|e| format!("{name}: {e}"))?;
        let label = entry
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| r.rsplit('/').next())
            .or_else(|| node.get("name").and_then(Value::as_str))
            .unwrap_or("modifier")
            .to_string();
        if let Some(contexts) = node.get("contexts") {
            let contexts = contexts
                .as_object()
                .filter(|c| !c.is_empty())
                .ok_or_else(|| format!("{name}: the modifier `{label}` has no contexts"))?;
            let mut names: Vec<String> = contexts.keys().cloned().collect();
            if let Some(d) = node.get("default") {
                let d = d
                    .as_str()
                    .ok_or_else(|| format!("{name}: `{label}`'s default is a context's name"))?;
                if !contexts.contains_key(d) {
                    return Err(format!(
                        "{name}: `{label}`'s default `{d}` is no context of it"
                    ));
                }
                names.sort_by_key(|n| n != d);
            }
            for context in &names {
                let scope = match scheme(&names) {
                    true => Scope::Scheme(is_dark(context)),
                    false => Scope::Axis(ident(&label), ident(context)),
                };
                let mut found = Vec::new();
                let sources = contexts[context]
                    .as_array()
                    .ok_or_else(|| format!("{name}: `{label}.{context}` is a list of sources"))?;
                for src in sources {
                    found.extend(source(src).map_err(|e| format!("{name}: {e}"))?);
                }
                out.layer(scope, found);
            }
        } else {
            let sources = node
                .get("sources")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!("{name}: `{label}` is a set (with `sources`) or a modifier (with `contexts`)")
                })?;
            let mut found = Vec::new();
            for src in sources {
                found.extend(source(src).map_err(|e| format!("{name}: {e}"))?);
            }
            out.layer(Scope::Base, found);
        }
    }
    Ok(out)
}

/// What Tamber makes of a `$type`.
enum Kind {
    Colour,
    Pending(&'static str),
    Reported(&'static str),
}

fn kind(ty: Option<&str>) -> Kind {
    match ty.unwrap_or("") {
        "color" => Kind::Colour,
        "dimension" | "borderRadius" | "spacing" | "sizing" | "borderWidth" | "fontSizes"
        | "letterSpacing" | "paragraphSpacing" => Kind::Pending("length"),
        "fontFamily" | "fontFamilies" => Kind::Pending("family"),
        "fontWeight" | "fontWeights" | "number" | "lineHeights" | "opacity" => {
            Kind::Pending("number")
        }
        "duration" => Kind::Pending("duration"),
        "cubicBezier" => Kind::Pending("easing"),
        "shadow" | "boxShadow" => Kind::Pending("box-shadow"),
        "typography" => Kind::Pending("type"),
        "border" => Kind::Reported("no Tamber kind: a border"),
        "transition" => Kind::Reported("no Tamber kind: a transition"),
        "gradient" => Kind::Reported("no Tamber kind: a gradient (its stops stay where they are)"),
        "strokeStyle" => Kind::Reported("no Tamber kind: a stroke style"),
        "boolean" | "string" => Kind::Reported("no Tamber kind: a string or a boolean"),
        "" => Kind::Reported("no `$type`, on it, a group around it, or the token it aliases"),
        _ => Kind::Reported("a `$type` Tamber's DTCG mapping does not name"),
    }
}

/// A colour value as Tamber writes it: a string, or 2025.10's object form
/// (its colour space first; `hex` only without one).
fn colour(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return colour_literal(s);
    }
    let obj = value.as_object()?;
    let alpha = obj
        .get("alpha")
        .and_then(Value::as_f64)
        .filter(|a| *a < 1.0);
    let space = obj.get("colorSpace").and_then(Value::as_str);
    let components = obj.get("components").and_then(Value::as_array);
    if let (Some(space), Some(components)) = (space, components) {
        let parts: Option<Vec<String>> = components
            .iter()
            .map(|c| match c {
                Value::String(s) if s == "none" => Some("none".to_string()),
                c => c.as_f64().map(|n| format!("{n}")),
            })
            .collect();
        if let Some(parts) = parts {
            let a = alpha.map_or(String::new(), |a| format!(" / {a}"));
            let text = match space {
                "oklch" | "oklab" | "lab" | "lch" | "hsl" | "hwb" => {
                    Some(format!("{space}({}{a})", parts.join(" ")))
                }
                "srgb" | "srgb-linear" | "display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020"
                | "xyz-d50" | "xyz-d65" => Some(format!("color({space} {}{a})", parts.join(" "))),
                _ => None,
            };
            if let Some(c) = text.and_then(|t| colour_literal(&t)) {
                return Some(c);
            }
        }
    }
    let hex = obj.get("hex").and_then(Value::as_str)?;
    let full = match (hex.len(), alpha) {
        (_, None) => hex.to_string(),
        (7, Some(a)) => format!("{hex}{:02x}", (a * 255.0).round() as u8),
        _ => return None,
    };
    colour_literal(&full)
}

/// An alias's target path: `{a.b}`, or `{"$ref": "#/a/b/$value"}`;
/// `Err` for a reference to part of a value.
fn alias(value: &Value) -> Option<Result<String, String>> {
    if let Some(s) = value.as_str() {
        let p = s.trim().strip_prefix('{')?.strip_suffix('}')?;
        return (!p.contains(['{', '}'])).then(|| Ok(p.to_string()));
    }
    let r = value
        .as_object()
        .filter(|o| o.len() == 1)?
        .get("$ref")?
        .as_str()?;
    let pointer = r.strip_prefix("#/")?;
    let segs: Vec<&str> = pointer.split('/').collect();
    match segs.iter().position(|s| *s == "$value") {
        Some(i) if i + 1 == segs.len() => Some(Ok(segs[..i].join("."))),
        Some(_) => Some(Err(format!(
            "`{r}` names part of a value, which no Tamber value reads"
        ))),
        None => Some(Ok(segs.join("."))),
    }
}

/// One mode tuple: the scheme's side and a value of each axis.
type Tuple = (bool, Vec<String>);

/// A path's colour in one tuple: as written (a reference a token's name),
/// resolved, and the path it references.
type Colour = (String, String, Option<String>);

/// A path's colour in one tuple, or why not.
type Seen = Result<Colour, String>;

/// A path's type in a tuple: its own, else the type of what it aliases.
fn type_of(defs: &[Def], w: &BTreeMap<&str, usize>, path: &str, depth: usize) -> Option<String> {
    let d = &defs[*w.get(path)?];
    if d.ty.is_some() || depth > 16 {
        return d.ty.clone();
    }
    match alias(&d.value) {
        Some(Ok(target)) => type_of(defs, w, &target, depth + 1),
        _ => None,
    }
}

/// A path's colour in a tuple, aliases followed there.
fn seen(
    defs: &[Def],
    w: &BTreeMap<&str, usize>,
    names: &BTreeMap<String, String>,
    path: &str,
    visiting: &mut Vec<String>,
) -> Seen {
    let Some(&i) = w.get(path) else {
        return Err(format!("`{{{path}}}` is no token in this mode"));
    };
    let d = &defs[i];
    if let Some(why) = &d.refused {
        return Err(why.clone());
    }
    match alias(&d.value) {
        Some(Ok(target)) => {
            if visiting.contains(&target) {
                return Err(format!("a cycle of aliases through `{{{target}}}`"));
            }
            visiting.push(target.clone());
            let found = seen(defs, w, names, &target, visiting);
            visiting.pop();
            let (_, resolved, _) = found?;
            match names.get(&target) {
                Some(n) => Ok((n.clone(), resolved, Some(target))),
                None => Ok((resolved.clone(), resolved, None)),
            }
        }
        Some(Err(why)) => Err(why),
        None => colour(&d.value)
            .map(|c| (c.clone(), c, None))
            .ok_or_else(|| match d.value.as_str() {
                Some(s) if exact_color::parse(s).is_some() => {
                    "a colour Tamber does not fold yet".to_string()
                }
                _ => "not a colour Tamber reads".to_string(),
            }),
    }
}

/// A tuple as a report names it: `dark, brand=loud`.
fn tuple_name(t: &Tuple, scheme: bool, axes: &[(String, Vec<String>)]) -> String {
    let mut parts = Vec::new();
    if scheme {
        parts.push(if t.0 { "dark" } else { "light" }.to_string());
    }
    for ((a, _), v) in axes.iter().zip(&t.1) {
        parts.push(format!("{a}={v}"));
    }
    match parts.is_empty() {
        true => "the base".into(),
        false => parts.join(", "),
    }
}

/// Import `files` (each its path and JSON) under `prefix`, planned against
/// `design`.
pub fn import(
    files: &[(String, String)],
    prefix: &str,
    design: &Design,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |id: &'static str, m: String| vec![Diagnostic::new(id, m, 1, 1)];
    let read = read(files).map_err(|m| refuse("tamber-import-source", m))?;
    let defs = &read.defs;
    // The dimensions: the scheme, and each axis with its values in order.
    let has_scheme = read
        .layers
        .iter()
        .any(|l| matches!(l.scope, Scope::Scheme(_)));
    let mut axes: Vec<(String, Vec<String>)> = Vec::new();
    for l in &read.layers {
        if let Scope::Axis(a, v) = &l.scope {
            match axes.iter_mut().find(|(n, _)| n == a) {
                Some((_, vs)) if !vs.contains(v) => vs.push(v.clone()),
                Some(_) => {}
                None => axes.push((a.clone(), vec![v.clone()])),
            }
        }
    }
    // An axis the host owns, or one the design has with another default:
    // no proposal can make it.
    for (a, vs) in &axes {
        if matches!(a.as_str(), "scheme" | "contrast" | "color-scheme") {
            return Err(refuse(
                "tamber-import-axis",
                format!("a mode `{a}` is the host's trait, not an app axis: name it otherwise in the source"),
            ));
        }
        if let Some(have) = design.axes.iter().find(|x| x.name.value == *a) {
            if have.values.first() != vs.first() {
                return Err(refuse(
                    "tamber-import-axis",
                    format!(
                        "the design's axis `{a}` defaults to `{}`, the source's to `{}`",
                        have.values.first().cloned().unwrap_or_default(),
                        vs[0]
                    ),
                ));
            }
        }
    }
    // The tuples, the default first; their count capped as a design's is.
    let sides: &[bool] = if has_scheme { &[false, true] } else { &[false] };
    let others: usize = design
        .axes
        .iter()
        .filter(|a| !axes.iter().any(|(n, _)| *n == a.name.value))
        .map(|a| a.values.len())
        .product();
    let app_tuples = axes
        .iter()
        .try_fold(others, |acc, (_, vs)| acc.checked_mul(vs.len()));
    if app_tuples.is_none_or(|t| t > crate::MAX_TUPLES) {
        return Err(refuse(
            "tamber-import-tuples",
            format!(
                "the proposed axes would take the design past {} tuples",
                crate::MAX_TUPLES
            ),
        ));
    }
    let mut tuples: Vec<Tuple> = sides.iter().map(|s| (*s, Vec::new())).collect();
    for (_, vs) in &axes {
        tuples = tuples
            .iter()
            .flat_map(|(s, picked)| {
                vs.iter().map(move |v| {
                    let mut p = picked.clone();
                    p.push(v.clone());
                    (*s, p)
                })
            })
            .collect();
    }
    // Names: each path's segments as identifiers; a collision reported.
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    let mut collided: BTreeMap<String, String> = BTreeMap::new();
    for d in defs {
        if names.contains_key(&d.path) || collided.contains_key(&d.path) {
            continue;
        }
        let name = std::iter::once(prefix.to_string())
            .chain(d.path.split('.').map(segment))
            .collect::<Vec<_>>()
            .join(".");
        match taken.get(&name) {
            Some(first) => {
                collided.insert(d.path.clone(), first.clone());
            }
            None => {
                taken.insert(name.clone(), d.path.clone());
                names.insert(d.path.clone(), name);
            }
        }
    }
    // Each tuple's winners: its layers merged in order, a later one winning
    // whatever its kind.
    let active = |t: &Tuple, scope: &Scope| match scope {
        Scope::Base => true,
        Scope::Scheme(dark) => *dark == t.0,
        Scope::Axis(a, v) => axes
            .iter()
            .position(|(n, _)| n == a)
            .is_some_and(|i| t.1[i] == *v),
    };
    let unused: BTreeSet<usize> = read.unused.iter().map(|(i, _)| *i).collect();
    let winners: Vec<BTreeMap<&str, usize>> = tuples
        .iter()
        .map(|t| {
            let mut w = BTreeMap::new();
            for l in read.layers.iter().filter(|l| active(t, &l.scope)) {
                for &i in l.defs.iter().filter(|i| !unused.contains(i)) {
                    if !collided.contains_key(&defs[i].path) {
                        w.insert(defs[i].path.as_str(), i);
                    }
                }
            }
            w
        })
        .collect();
    // Every path's colour in every tuple, or why it makes no token.
    let paths: Vec<&str> = names.keys().map(String::as_str).collect();
    let mut values: BTreeMap<&str, Vec<Colour>> = BTreeMap::new();
    let mut fates: BTreeMap<&str, String> = BTreeMap::new();
    for &path in &paths {
        let mut per = Vec::new();
        let mut failure: Option<String> = None;
        for (t, w) in tuples.iter().zip(&winners) {
            let label = tuple_name(t, has_scheme, &axes);
            if !w.contains_key(path) {
                failure.get_or_insert(format!(
                    "not set in {label}, and no value of it is invented"
                ));
                continue;
            }
            match kind(type_of(defs, w, path, 0).as_deref()) {
                Kind::Colour => match seen(defs, w, &names, path, &mut vec![path.to_string()]) {
                    Ok(v) => per.push(v),
                    Err(why) => {
                        failure.get_or_insert(format!("{why} ({label})"));
                    }
                },
                _ => {
                    failure.get_or_insert(String::new());
                }
            }
        }
        match failure {
            None => {
                values.insert(path, per);
            }
            Some(why) => {
                fates.insert(path, why);
            }
        }
    }
    // A reference to a token not made is written as its colour; one round a
    // cycle of tokens too.
    let made: BTreeSet<&str> = values.keys().copied().collect();
    let mut graph: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (path, per) in values.iter_mut() {
        for v in per.iter_mut() {
            if let Some(target) = v.2.clone() {
                if made.contains(target.as_str()) {
                    graph.entry(path).or_default().insert(target);
                } else {
                    *v = (v.1.clone(), v.1.clone(), None);
                }
            }
        }
    }
    let reaches = |from: &str| {
        let mut stack: Vec<String> = graph
            .get(from)
            .map_or(Vec::new(), |s| s.iter().cloned().collect());
        let mut visited = BTreeSet::new();
        while let Some(n) = stack.pop() {
            if n == from {
                return true;
            }
            if visited.insert(n.clone()) {
                stack.extend(graph.get(n.as_str()).into_iter().flatten().cloned());
            }
        }
        false
    };
    let cyclic: BTreeSet<&str> = graph.keys().copied().filter(|p| reaches(p)).collect();
    for (path, per) in values.iter_mut() {
        if cyclic.contains(path) {
            for v in per.iter_mut() {
                *v = (v.1.clone(), v.1.clone(), None);
            }
        }
    }
    // The tokens: the default tuple's value, and a `when` line for each other
    // tuple Tamber would not otherwise select its value in.
    let pair = |l: &str, d: &str| match l == d {
        true => l.to_string(),
        false => format!("light-dark({l}, {d})"),
    };
    let value_at = |per: &Vec<Colour>, combo: &Vec<String>| {
        let at = |dark: bool| {
            tuples
                .iter()
                .position(|t| t.0 == dark && t.1 == *combo)
                .map(|i| per[i].0.clone())
                .unwrap_or_default()
        };
        match has_scheme {
            true => pair(&at(false), &at(true)),
            false => at(false),
        }
    };
    let default_combo: Vec<String> = axes.iter().map(|(_, vs)| vs[0].clone()).collect();
    let mut combos: Vec<&Vec<String>> = tuples
        .iter()
        .filter(|t| !t.0 && t.1 != default_combo)
        .map(|t| &t.1)
        .collect();
    // Fewer conditions first: a line naming more axes is added only where the
    // ones naming fewer leave the value wrong or ambiguous.
    combos.sort_by_key(|c| c.iter().zip(&default_combo).filter(|(a, b)| a != b).count());
    let mut tokens = Vec::new();
    for (path, per) in &values {
        let value = value_at(per, &default_combo);
        let mut whens: Vec<(Vec<(String, String)>, String)> = Vec::new();
        for combo in &combos {
            let conds: Vec<(String, String)> = axes
                .iter()
                .zip(combo.iter())
                .filter(|((_, vs), v)| **v != vs[0])
                .map(|((a, _), v)| (a.clone(), v.clone()))
                .collect();
            let want = value_at(per, combo);
            let holding: Vec<&(Vec<(String, String)>, String)> = whens
                .iter()
                .filter(|(c, _)| c.iter().all(|x| conds.contains(x)))
                .collect();
            let most = holding.iter().map(|(c, _)| c.len()).max();
            let top: Vec<&String> = holding
                .iter()
                .filter(|(c, _)| Some(c.len()) == most)
                .map(|(_, v)| v)
                .collect();
            let selected = match top.len() {
                0 => Some(&value),
                1 => Some(top[0]),
                _ => None,
            };
            if selected != Some(&want) {
                whens.push((conds, want));
            }
        }
        tokens.push(Mapped {
            name: names[*path].clone(),
            value,
            whens,
        });
    }
    // Each def: mapped when it wins a tuple of a token made, else reported.
    let wins: BTreeSet<usize> = winners.iter().flat_map(|w| w.values().copied()).collect();
    let mut unmapped = Vec::new();
    let mut mapped = 0;
    for (i, d) in defs.iter().enumerate() {
        let report = |why: String, token: Option<String>| Unmapped {
            block: d.file.clone(),
            name: d.path.clone(),
            value: match &d.value {
                Value::String(s) => s.clone(),
                v => v.to_string(),
            },
            why,
            token,
        };
        if let Some((_, why)) = read.unused.iter().find(|(j, _)| *j == i) {
            unmapped.push(report(why.clone(), None));
            continue;
        }
        if let Some(first) = collided.get(&d.path) {
            unmapped.push(report(
                format!("its name is `{first}`'s ({})", names[first]),
                None,
            ));
            continue;
        }
        if !wins.contains(&i) {
            unmapped.push(report(
                "declared again by a later layer, which wins".into(),
                None,
            ));
            continue;
        }
        if values.contains_key(d.path.as_str()) {
            mapped += 1;
            continue;
        }
        let token = names.get(&d.path).cloned();
        let ty =
            d.ty.clone()
                .or_else(|| winners.iter().find_map(|w| type_of(defs, w, &d.path, 0)));
        let why = match (&d.refused, kind(ty.as_deref())) {
            (Some(why), _) => why.clone(),
            (None, Kind::Pending(k)) => format!("kind pending (stage 6): {k}"),
            (None, Kind::Reported(why)) => {
                unmapped.push(report(why.to_string(), None));
                continue;
            }
            (None, Kind::Colour) => fates
                .get(d.path.as_str())
                .cloned()
                .filter(|w| !w.is_empty())
                .unwrap_or_else(|| "a colour in some modes, another kind in others".into()),
        };
        unmapped.push(report(why, token));
    }
    let owned: Vec<&str> = axes.iter().map(|(a, _)| a.as_str()).collect();
    let ops = reconcile(design, &axes, &owned, &tokens);
    let reported = unmapped.len();
    let counted = counted(defs.len(), mapped, reported)?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
    })
}
