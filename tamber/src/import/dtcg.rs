//! DTCG JSON (LLP 1121 D14, importer 3): Format 2025.10, with the Resolver
//! Module where present, from Tokens Studio and from Figma Variables
//! exports. The source is a set of files:
//!
//! - **Figma:** `<Collection>.<Mode>.json`, one file a collection's mode. A
//!   collection whose modes are `Light` and `Dark` is the scheme; one with
//!   one mode is the base; any other's modes are a proposed axis named for
//!   the collection, its first mode (by name) the default.
//! - **Tokens Studio:** `$themes.json` and its token sets (`primitive/colors`
//!   …), or one file holding them all. A theme group of one theme is the
//!   base; a group whose themes are `light` and `dark` is the scheme; any
//!   other group's themes are a proposed axis. Sets merge in
//!   `$metadata.tokenSetOrder`, a later set winning. Tokens Studio's legacy
//!   `value` and `type` are read as `$value` and `$type`.
//! - **A resolver** (`resolutionOrder`): its sets are the base; a modifier
//!   whose contexts are `light` and `dark` is the scheme; any other is a
//!   proposed axis with its contexts as values.
//! - **One DTCG file:** the base.
//!
//! Tokens are mapped by `$type`: `color` to a colour (a string, or 2025.10's
//! object form), `{a.b}` aliases to references; other types are
//! `kind pending (stage 6)` with the token each would become, and borders,
//! transitions, gradients and stroke styles are reported, as are Figma's
//! strings and booleans. A group path becomes a dotted name, each segment
//! made an identifier (`Primary Color` → `Primary-Color`, a leading digit
//! taking `s`); two paths that would be one name are reported.

use super::{colour_literal, counted, reconcile, Import, Mapped, Unmapped};
use crate::{Design, Diagnostic};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Where a layer's tokens apply.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    /// Always.
    Base,
    /// One side of the scheme: `true` dark.
    Scheme(bool),
    /// One value of a proposed axis.
    Axis(String, String),
}

/// One token as a layer declares it.
#[derive(Debug, Clone)]
struct Def {
    /// The file it is in, for the report.
    file: String,
    ty: Option<String>,
    value: Value,
}

/// A layer: where it applies, and its tokens by path.
struct Layer {
    scope: Scope,
    tokens: Vec<(String, Def)>,
}

/// A name segment as an identifier: spaces and other characters a name may
/// not hold become `-`, a leading digit takes `s`.
fn segment(raw: &str) -> String {
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

/// Every token in a DTCG (or Tokens Studio) tree: its dotted path and
/// definition, `$type` inherited from groups.
fn tokens(file: &str, tree: &Value, out: &mut Vec<(String, Def)>) {
    fn walk(
        file: &str,
        node: &Value,
        path: &mut Vec<String>,
        ty: Option<String>,
        out: &mut Vec<(String, Def)>,
    ) {
        let Some(map) = node.as_object() else { return };
        let ty = map
            .get("$type")
            .or_else(|| map.get("type").filter(|_| map.contains_key("value")))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(ty);
        let value = map
            .get("$value")
            .or_else(|| map.get("value").filter(|_| !map.contains_key("$value")));
        if let Some(value) = value {
            out.push((
                path.join("."),
                Def {
                    file: file.to_string(),
                    ty,
                    value: value.clone(),
                },
            ));
            return;
        }
        for (k, v) in map {
            if k.starts_with('$') {
                continue;
            }
            path.push(k.clone());
            walk(file, v, path, ty.clone(), out);
            path.pop();
        }
    }
    walk(file, tree, &mut Vec::new(), None, out);
}

/// Whether two mode names are the scheme's sides.
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

/// The layers a source's files make, or why it cannot be read.
fn layers(files: &[(String, String)]) -> Result<Vec<Layer>, String> {
    let parsed: Vec<(String, Value)> = files
        .iter()
        .map(|(name, text)| {
            serde_json::from_str(text)
                .map(|v| (name.clone(), v))
                .map_err(|e| format!("{name}: {e}"))
        })
        .collect::<Result<_, _>>()?;
    let stem = |name: &str| {
        let base = name.rsplit('/').next().unwrap_or(name);
        base.strip_suffix(".json").unwrap_or(base).to_string()
    };
    // Tokens Studio: `$themes` beside its sets, or in one file.
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
        return studio(&themes, metadata.as_ref(), &sets);
    }
    if let Some((name, resolver)) = parsed
        .iter()
        .find(|(_, v)| v.get("resolutionOrder").is_some())
    {
        return resolve(name, resolver, &parsed);
    }
    // Figma: `<Collection>.<Mode>.json`.
    let figma: Vec<(String, String, &Value)> = parsed
        .iter()
        .filter_map(|(n, v)| {
            let s = stem(n);
            let (collection, mode) = s.rsplit_once('.')?;
            Some((collection.to_string(), mode.to_string(), v))
        })
        .collect();
    if parsed.len() > 1 && figma.len() == parsed.len() {
        let mut collections: BTreeMap<String, Vec<(String, &Value)>> = BTreeMap::new();
        for (c, m, v) in figma {
            collections.entry(c).or_default().push((m, v));
        }
        let mut out = Vec::new();
        for (collection, mut modes) in collections {
            modes.sort_by(|a, b| a.0.cmp(&b.0));
            let names: Vec<String> = modes.iter().map(|(m, _)| m.clone()).collect();
            for (mode, v) in &modes {
                let scope = match (modes.len(), scheme(&names)) {
                    (1, _) => Scope::Base,
                    (_, true) => Scope::Scheme(is_dark(mode)),
                    _ => Scope::Axis(ident(&collection), ident(mode)),
                };
                let mut found = Vec::new();
                tokens(&format!("{collection}.{mode}"), v, &mut found);
                out.push(Layer {
                    scope,
                    tokens: found,
                });
            }
        }
        return Ok(out);
    }
    // DTCG files, each the base.
    Ok(parsed
        .iter()
        .map(|(n, v)| {
            let mut found = Vec::new();
            tokens(n, v, &mut found);
            Layer {
                scope: Scope::Base,
                tokens: found,
            }
        })
        .collect())
}

/// A Tokens Studio theme: its name and the sets it turns on.
type Theme = (String, Vec<String>);

/// Tokens Studio's themes: each group a dimension (base, scheme or axis).
fn studio(
    themes: &Value,
    metadata: Option<&Value>,
    sets: &[(String, Value)],
) -> Result<Vec<Layer>, String> {
    let order: Vec<String> = metadata
        .and_then(|m| m.get("tokenSetOrder"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    // A set's place: `tokenSetOrder` names sets by their last segment or
    // whole; unnamed sets keep the files' order after them.
    let rank = |set: &str| {
        let last = set.rsplit('/').next().unwrap_or(set);
        order
            .iter()
            .position(|o| o == set || o == last)
            .unwrap_or(order.len())
    };
    let set_tokens = |names: &[String]| {
        let mut chosen: Vec<&(String, Value)> =
            sets.iter().filter(|(n, _)| names.contains(n)).collect();
        chosen.sort_by_key(|(n, _)| rank(n));
        let mut found = Vec::new();
        for (n, v) in chosen {
            tokens(n, v, &mut found);
        }
        found
    };
    let Some(list) = themes.as_array() else {
        // No themes: every set is the base.
        let all: Vec<String> = sets.iter().map(|(n, _)| n.clone()).collect();
        return Ok(vec![Layer {
            scope: Scope::Base,
            tokens: set_tokens(&all),
        }]);
    };
    // Each group: its themes, each with the sets it turns on.
    let mut groups: Vec<(String, Vec<Theme>)> = Vec::new();
    for t in list {
        let name = t["name"].as_str().unwrap_or("").to_string();
        let group = t["group"].as_str().unwrap_or(&name).to_string();
        let on: Vec<String> = t["selectedTokenSets"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, s)| matches!(s.as_str(), Some("enabled" | "source")))
            .map(|(k, _)| k.clone())
            .collect();
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, members)) => members.push((name, on)),
            None => groups.push((group, vec![(name, on)])),
        }
    }
    let mut out = Vec::new();
    for (group, members) in groups {
        let names: Vec<String> = members.iter().map(|(n, _)| n.clone()).collect();
        for (theme, on) in &members {
            let scope = match (members.len(), scheme(&names)) {
                (1, _) => Scope::Base,
                (_, true) => Scope::Scheme(is_dark(theme)),
                _ => Scope::Axis(ident(&group), ident(theme)),
            };
            out.push(Layer {
                scope,
                tokens: set_tokens(on),
            });
        }
    }
    // The base first, as every theme sits on it.
    out.sort_by_key(|l| l.scope != Scope::Base);
    Ok(out)
}

/// A Resolver Module document: its sets, then its modifiers.
fn resolve(name: &str, doc: &Value, files: &[(String, Value)]) -> Result<Vec<Layer>, String> {
    // A source: an inline token tree, or `$ref` to a file (or a part of
    // this document).
    let source = |src: &Value| -> Result<Vec<(String, Def)>, String> {
        let mut found = Vec::new();
        match src.get("$ref").and_then(Value::as_str) {
            Some(r) if r.starts_with('#') => {
                let pointer = r.trim_start_matches('#');
                let node = doc
                    .pointer(pointer)
                    .ok_or_else(|| format!("{name}: no `{r}`"))?;
                tokens(name, node, &mut found);
            }
            Some(r) => {
                let (file, pointer) = r.split_once('#').map_or((r, ""), |(f, p)| (f, p));
                let (fname, tree) = files
                    .iter()
                    .find(|(n, _)| n == file || n.ends_with(&format!("/{file}")))
                    .ok_or_else(|| format!("{name}: no file `{file}` among the sources"))?;
                let node = if pointer.is_empty() {
                    tree
                } else {
                    tree.pointer(pointer)
                        .ok_or_else(|| format!("{fname}: no `{pointer}`"))?
                };
                tokens(fname, node, &mut found);
            }
            None => tokens(name, src, &mut found),
        }
        Ok(found)
    };
    let mut out = Vec::new();
    for entry in doc["resolutionOrder"].as_array().into_iter().flatten() {
        let r = entry.get("$ref").and_then(Value::as_str).unwrap_or("");
        let pointer = r.trim_start_matches('#');
        let node = doc.pointer(pointer).unwrap_or(entry);
        if pointer.starts_with("/modifiers/") || node.get("contexts").is_some() {
            let modifier = pointer.rsplit('/').next().unwrap_or("modifier");
            let contexts = node["contexts"].as_object().cloned().unwrap_or_default();
            let mut names: Vec<String> = contexts.keys().cloned().collect();
            // The declared default first.
            if let Some(d) = node["default"].as_str() {
                names.sort_by_key(|n| n != d);
            }
            for context in &names {
                let scope = match scheme(&names) {
                    true => Scope::Scheme(is_dark(context)),
                    false => Scope::Axis(ident(modifier), ident(context)),
                };
                let mut found = Vec::new();
                for src in contexts[context].as_array().into_iter().flatten() {
                    found.extend(source(src)?);
                }
                out.push(Layer {
                    scope,
                    tokens: found,
                });
            }
        } else {
            let mut found = Vec::new();
            for src in node["sources"].as_array().into_iter().flatten() {
                found.extend(source(src)?);
            }
            out.push(Layer {
                scope: Scope::Base,
                tokens: found,
            });
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
        "gradient" => Kind::Reported("no Tamber kind: a gradient"),
        "strokeStyle" => Kind::Reported("no Tamber kind: a stroke style"),
        "boolean" | "string" => Kind::Reported("no Tamber kind: a string or a boolean"),
        "" => Kind::Reported("no `$type`, on it or a group around it"),
        _ => Kind::Reported("a `$type` Tamber's DTCG mapping does not name"),
    }
}

/// A colour value as Tamber writes it: a string, or 2025.10's object form.
fn colour(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return colour_literal(s);
    }
    let obj = value.as_object()?;
    if let Some(hex) = obj.get("hex").and_then(Value::as_str) {
        let alpha = obj
            .get("alpha")
            .and_then(Value::as_f64)
            .filter(|a| *a < 1.0);
        let text = match alpha {
            Some(a) => format!("{hex}{:02x}", (a * 255.0).round() as u8),
            None => hex.to_string(),
        };
        return colour_literal(&text);
    }
    let space = obj.get("colorSpace")?.as_str()?;
    let parts: Vec<String> = obj
        .get("components")?
        .as_array()?
        .iter()
        .map(|c| match c {
            Value::String(s) if s == "none" => Some("none".to_string()),
            c => c.as_f64().map(|n| format!("{n}")),
        })
        .collect::<Option<_>>()?;
    let alpha = obj
        .get("alpha")
        .and_then(Value::as_f64)
        .filter(|a| *a < 1.0)
        .map_or(String::new(), |a| format!(" / {a}"));
    let text = match space {
        "oklch" | "oklab" | "lab" | "lch" | "hsl" | "hwb" => {
            format!("{space}({}{alpha})", parts.join(" "))
        }
        "srgb" | "srgb-linear" | "display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020"
        | "xyz-d50" | "xyz-d65" => {
            format!("color({space} {}{alpha})", parts.join(" "))
        }
        _ => return None,
    };
    colour_literal(&text)
}

/// `{a.b}`'s path.
fn alias(value: &Value) -> Option<&str> {
    let s = value.as_str()?.trim();
    s.strip_prefix('{')?
        .strip_suffix('}')
        .filter(|p| !p.contains(['{', '}']))
}

/// Import `files` (each its name and JSON) under `prefix`, planned against
/// `design`.
pub fn import(
    files: &[(String, String)],
    prefix: &str,
    design: &Design,
) -> Result<Import, Vec<Diagnostic>> {
    let refuse = |m: String| vec![Diagnostic::new("tamber-import-source", m, 1, 1)];
    let layers = layers(files).map_err(refuse)?;
    let mut unmapped = Vec::new();
    let mut report = |file: &str, path: &str, value: &Value, why: String, token: Option<String>| {
        unmapped.push(Unmapped {
            block: file.to_string(),
            name: path.to_string(),
            value: match value {
                Value::String(s) => s.clone(),
                v => v.to_string(),
            },
            why,
            token,
        });
    };
    let source: usize = layers.iter().map(|l| l.tokens.len()).sum();
    // Names: each path's segments as identifiers; a collision reported.
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut taken: BTreeMap<String, String> = BTreeMap::new();
    let mut collided: BTreeMap<String, String> = BTreeMap::new();
    for layer in &layers {
        for (path, _) in &layer.tokens {
            if names.contains_key(path) || collided.contains_key(path) {
                continue;
            }
            let name = std::iter::once(prefix.to_string())
                .chain(path.split('.').map(segment))
                .collect::<Vec<_>>()
                .join(".");
            match taken.get(&name) {
                Some(first) => {
                    collided.insert(path.clone(), first.clone());
                }
                None => {
                    taken.insert(name.clone(), path.clone());
                    names.insert(path.clone(), name);
                }
            }
        }
    }
    // The colour paths: typed colour in some layer.
    let colour_paths: BTreeSet<&String> = layers
        .iter()
        .flat_map(|l| l.tokens.iter())
        .filter(|(p, d)| {
            matches!(kind(d.ty.as_deref()), Kind::Colour) && !collided.contains_key(p.as_str())
        })
        .map(|(p, _)| p)
        .collect();
    // Each colour value by path and scope, the later layer (and set) winning;
    // the rest reported.
    let mut values: BTreeMap<(String, Scope), (String, String)> = BTreeMap::new();
    let mut mapped = 0;
    let mut winners: BTreeMap<(String, Scope), (String, Value)> = BTreeMap::new();
    for layer in &layers {
        for (path, def) in &layer.tokens {
            if let Some(first) = collided.get(path) {
                report(
                    &def.file,
                    path,
                    &def.value,
                    format!("its name is `{first}`'s ({})", names[first]),
                    None,
                );
                continue;
            }
            match kind(def.ty.as_deref()) {
                Kind::Colour => {
                    let key = (path.clone(), layer.scope.clone());
                    if let Some((file, old)) =
                        winners.insert(key, (def.file.clone(), def.value.clone()))
                    {
                        report(
                            &file,
                            path,
                            &old,
                            "declared again by a later set or file, which wins".into(),
                            None,
                        );
                    }
                }
                Kind::Pending(k) => report(
                    &def.file,
                    path,
                    &def.value,
                    format!("kind pending (stage 6): {k}"),
                    Some(names[path].clone()),
                ),
                Kind::Reported(why) => report(&def.file, path, &def.value, why.to_string(), None),
            }
        }
    }
    // An alias names its token; a cycle of aliases is no colour.
    let alias_of = |v: &Value| alias(v).map(str::to_string);
    let cyclic = |start: &str| {
        let mut seen = BTreeSet::new();
        let mut at = start.to_string();
        loop {
            if !seen.insert(at.clone()) {
                return true;
            }
            let next = winners
                .iter()
                .find(|((p, _), _)| *p == at)
                .and_then(|(_, (_, v))| alias_of(v));
            match next {
                Some(n) if n == start => return true,
                Some(n) => at = n,
                None => return false,
            }
        }
    };
    for ((path, scope), (file, value)) in &winners {
        let text = match alias(value) {
            Some(target) if colour_paths.contains(&target.to_string()) && !cyclic(path) => {
                Some(names[target].clone())
            }
            Some(target) => {
                report(
                    file,
                    path,
                    value,
                    format!("`{{{target}}}` names no colour token"),
                    Some(names[path].clone()),
                );
                continue;
            }
            None => colour(value),
        };
        match text {
            Some(t) => {
                mapped += 1;
                values.insert((path.clone(), scope.clone()), (t, file.clone()));
            }
            None => {
                let why = match colour_literal(value.as_str().unwrap_or("")) {
                    None if value
                        .as_str()
                        .is_some_and(|s| exact_color::parse(s).is_some()) =>
                    {
                        "a colour Tamber does not fold yet".to_string()
                    }
                    _ => "not a colour Tamber reads".to_string(),
                };
                report(file, path, value, why, Some(names[path].clone()));
            }
        }
    }
    // The tokens: the base, the scheme's sides, the axes' `when` lines.
    let mut axes: Vec<(String, Vec<String>)> = Vec::new();
    for layer in &layers {
        if let Scope::Axis(a, v) = &layer.scope {
            match axes.iter_mut().find(|(n, _)| n == a) {
                Some((_, vs)) if !vs.contains(v) => vs.push(v.clone()),
                Some(_) => {}
                None => axes.push((a.clone(), vec![v.clone()])),
            }
        }
    }
    let pair = |l: Option<&String>, d: Option<&String>| match (l, d) {
        (Some(l), Some(d)) if l == d => Some(l.clone()),
        (Some(l), Some(d)) => Some(format!("light-dark({l}, {d})")),
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (None, None) => None,
    };
    let mut paths: Vec<&String> = values.keys().map(|(p, _)| p).collect();
    paths.dedup();
    let mut tokens = Vec::new();
    let mut dropped_axes: Vec<(String, Scope)> = Vec::new();
    for path in paths {
        let get = |scope: &Scope| values.get(&(path.clone(), scope.clone())).map(|(t, _)| t);
        let base = get(&Scope::Base);
        let default = match (get(&Scope::Scheme(false)), get(&Scope::Scheme(true))) {
            (None, None) => base.cloned(),
            (l, d) => pair(l.or(base), d.or(base)),
        };
        // A token one axis sets; a second axis's lines would both hold.
        let mut owner: Option<&String> = None;
        let mut whens = Vec::new();
        for (axis, vals) in &axes {
            for v in vals {
                let scope = Scope::Axis(axis.clone(), v.clone());
                if let Some(t) = get(&scope) {
                    match owner {
                        Some(o) if o != axis => dropped_axes.push((path.clone(), scope)),
                        _ => {
                            owner = Some(axis);
                            whens.push((vec![(axis.clone(), v.clone())], t.clone()));
                        }
                    }
                }
            }
        }
        let Some(value) = default.or_else(|| whens.first().map(|(_, v)| v.clone())) else {
            continue;
        };
        whens.retain(|(_, v)| *v != value);
        tokens.push(Mapped {
            name: names[path].clone(),
            value,
            whens,
        });
    }
    for (path, scope) in dropped_axes {
        if let Some((_, file)) = values.get(&(path.clone(), scope.clone())) {
            mapped -= 1;
            let (axis, value) = match &scope {
                Scope::Axis(a, v) => (a.clone(), v.clone()),
                _ => unreachable!(),
            };
            report(
                file,
                &path,
                &Value::String(format!("{axis}={value}")),
                "set by a second axis too: two `when` lines would both hold".into(),
                None,
            );
        }
    }
    // The tuples the axes would make.
    let others: usize = design
        .axes
        .iter()
        .filter(|a| !axes.iter().any(|(n, _)| *n == a.name.value))
        .map(|a| a.values.len())
        .product();
    let total: usize = axes
        .iter()
        .map(|(n, vs)| {
            let have = design
                .axes
                .iter()
                .find(|a| a.name.value == *n)
                .map_or(Vec::new(), |a| a.values.clone());
            have.len() + vs.iter().filter(|v| !have.contains(v)).count()
        })
        .product::<usize>()
        * others;
    if !axes.is_empty() && total > crate::MAX_TUPLES {
        return Err(vec![Diagnostic::new(
            "tamber-import-tuples",
            format!("the proposed axes would take the design to {total} tuples; a design has at most {}", crate::MAX_TUPLES),
            1,
            1,
        )]);
    }
    let owned: Vec<&str> = axes.iter().map(|(a, _)| a.as_str()).collect();
    let ops = reconcile(design, &axes, &owned, &tokens);
    let reported = unmapped.len();
    let counted = counted(source, mapped, reported)?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
    })
}
