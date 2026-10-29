//! DTCG JSON (LLP 1121 D14, importer 3): Format 2025.10, with the Resolver
//! Module where present, from Tokens Studio and from Figma Variables
//! exports. The source is a set of files read as layers, each active in some
//! modes (always, one side of the scheme, or one value of a proposed axis),
//! in precedence order:
//!
//! - **Figma:** `<Collection>.<Mode>.json`. A collection whose modes are
//!   `Light` and `Dark` is the scheme; one with one mode is the base; any
//!   other's modes are a proposed axis named for the collection, its first
//!   mode (by name) the default. Any other file is the base. The base, then
//!   the scheme, then the axes.
//! - **Tokens Studio:** `$themes.json` and its token sets, or one file holding
//!   them all. A group of one theme is the base; a group whose themes are
//!   `light` and `dark` is the scheme; any other group (ungrouped themes are
//!   one group) a proposed axis. Each set is read once and is active in the
//!   modes of every theme that turns it on (`enabled` or `source`); the sets
//!   apply in `$metadata.tokenSetOrder`, the lowest winning; a set no theme
//!   turns on is reported. The legacy `value`/`type` form is read as
//!   `$value`/`$type`; a value Tokens Studio modifies (`studio.tokens`
//!   `modify`) is reported.
//! - **A resolver** (`resolutionOrder`): each set the base, a modifier whose
//!   contexts are `light` and `dark` the scheme, any other a proposed axis;
//!   references resolved in the document that holds them (`#/sets/…`, files
//!   relative to it), sibling keys overriding what they name, sets within
//!   sets expanded; a set the order never reaches is reported.
//! - **Any other DTCG file:** the base.
//!
//! Two axes, or two values of one, whose names are one identifier refuse the
//! import. Each mode tuple (a side of the scheme with a value of every axis)
//! is resolved on its own, its active layers merged in order (a later one
//! winning, whatever its kind), and every `{a.b}` or `$ref` alias followed
//! there. A token is a colour in every tuple or makes no token; its default
//! is the default tuple's (a `light-dark()` of the scheme's sides), and each
//! other tuple Tamber would not otherwise select its value in is a `when`
//! line. A dimension, a number, a family, a duration, a curve, a shadow or
//! a typography is its kind's token alike, written out (`kinds`), with one
//! value on both sides of the scheme; borders, transitions, gradients and
//! stroke styles are reported, as are strings and booleans. Names come from group paths, each
//! segment made an identifier (`$root` is `root`); collisions are reported.

use super::{counted, reconcile, Import, Mapped, Unmapped};
use crate::import::kinds as other;
use crate::{Design, Diagnostic};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

mod kinds;
pub mod pen;
mod resolve;
pub(crate) use kinds::colour;
use resolve::{Colour, Resolve};

/// Where a layer applies.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Base,
    /// One side of the scheme: `true` dark.
    Scheme(bool),
    /// One value of a proposed axis.
    Axis(String, String),
    /// Every one of these at once (a Pencil entry's theme of several axes).
    All(Vec<Scope>),
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

/// A layer: the modes it is active in (any of them), and its tokens
/// (indices into the defs).
struct Layer {
    scopes: Vec<Scope>,
    defs: Vec<usize>,
}

/// What the files make: every def, the layers in precedence order, the defs
/// no layer uses (with why), and each axis and value's name as written, to
/// refuse two that become one identifier.
#[derive(Default)]
struct Read {
    defs: Vec<Def>,
    layers: Vec<Layer>,
    unused: Vec<(usize, String)>,
    written: BTreeMap<(String, Option<String>), BTreeSet<String>>,
    /// Each axis and its values, in the order the source gives them (a
    /// theme group's, a collection's modes by name, a modifier's default
    /// first): the first value the default.
    dims: Vec<(String, Vec<String>)>,
    /// Names are flat (Pencil's): `a` and `a.b` are two tokens, no group.
    flat: bool,
}

impl Read {
    fn layer(&mut self, scopes: Vec<Scope>, found: Vec<Def>) {
        let start = self.defs.len();
        self.defs.extend(found);
        self.layers.push(Layer {
            scopes,
            defs: (start..self.defs.len()).collect(),
        });
    }

    /// An axis value's scope, its written names kept.
    fn axis(&mut self, axis: &str, value: &str) -> Scope {
        // An ungrouped theme's axis is `theme`, written as nothing.
        let (a, v) = (
            ident(if axis.is_empty() { "theme" } else { axis }),
            ident(value),
        );
        match self.dims.iter_mut().find(|(n, _)| *n == a) {
            Some((_, vs)) if !vs.contains(&v) => vs.push(v.clone()),
            Some(_) => {}
            None => self.dims.push((a.clone(), vec![v.clone()])),
        }
        self.written
            .entry((a.clone(), None))
            .or_default()
            .insert(axis.to_string());
        self.written
            .entry((a.clone(), Some(v.clone())))
            .or_default()
            .insert(value.to_string());
        Scope::Axis(a, v)
    }

    fn unused(&mut self, found: Vec<Def>, why: &str) {
        for def in found {
            self.unused.push((self.defs.len(), why.to_string()));
            self.defs.push(def);
        }
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

/// Whether a node is a token (as opposed to a group).
fn is_token(node: &Value) -> bool {
    node.as_object()
        .is_some_and(|m| m.contains_key("$value") || legacy(m))
}

/// Whether a node is a token in `root`: one with a value, or a whole `$ref`
/// to a token or to anything that is no group (chains followed).
fn token_like(root: &Value, node: &Value) -> bool {
    let mut at = node;
    for _ in 0..64 {
        if is_token(at) {
            return true;
        }
        let Some(r) = at.get("$ref").and_then(Value::as_str) else {
            return false;
        };
        match target(root, r) {
            Some(next) if next.is_object() => at = next,
            _ => return true,
        }
    }
    true
}

/// A JSON Pointer's segments, `~1` and `~0` decoded.
fn pointer_segments(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}

/// What `$extends` or `$ref` names in `root`: `{a.b}` or a JSON pointer.
fn target<'a>(root: &'a Value, name: &str) -> Option<&'a Value> {
    let t = name.trim();
    if let Some(pointer) = t.strip_prefix('#') {
        return root.pointer(pointer);
    }
    let path = t.strip_prefix('{')?.strip_suffix('}')?;
    path.split('.').try_fold(root, |node, seg| node.get(seg))
}

/// A group with what it extends merged under it, deeply: the inherited
/// tree first, its own members over it (a group into a group, a token or a
/// value replacing). A cycle of extensions is an error.
fn extended(
    root: &Value,
    node: &Map<String, Value>,
    stack: &mut Vec<String>,
) -> Result<Map<String, Value>, String> {
    let mut merged = Map::new();
    crate::budget::spend(1)?;
    for key in ["$extends", "$ref"] {
        let Some(name) = node.get(key).and_then(Value::as_str) else {
            continue;
        };
        if stack.iter().any(|s| s == name) {
            return Err(format!("a cycle of `{key}` through `{name}`"));
        }
        if stack.len() > 64 {
            return Err(format!("`{key}` chains deeper than 64 at `{name}`"));
        }
        let Some(inherited) = target(root, name).and_then(Value::as_object) else {
            return Err(format!("`{key}` names `{name}`, which is not there"));
        };
        crate::budget::members(inherited)?;
        if token_like(root, &Value::Object(inherited.clone())) {
            return Err(format!("`{key}` names `{name}`, a token, not a group"));
        }
        stack.push(name.to_string());
        let base = extended(root, inherited, stack)?;
        stack.pop();
        deep(root, &mut merged, &base)?;
        if merged.get("$type").is_none() {
            if let Some(t) = inherited.get("$type") {
                merged.insert("$type".into(), t.clone());
            }
        }
    }
    crate::budget::members(node)?;
    let own: Map<String, Value> = node
        .iter()
        .filter(|(k, _)| *k != "$extends" && *k != "$ref")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    deep(root, &mut merged, &own)?;
    Ok(merged)
}

/// `over` merged into `into`: groups into groups, anything else (a token,
/// a whole-token alias, a value) replacing; each copy charged first.
fn deep(
    root: &Value,
    into: &mut Map<String, Value>,
    over: &Map<String, Value>,
) -> Result<(), String> {
    for (k, v) in over {
        crate::budget::copy(v)?;
        match (into.get_mut(k), v) {
            (Some(Value::Object(a)), Value::Object(b))
                if !token_like(root, &Value::Object(a.clone())) && !token_like(root, v) =>
            {
                crate::budget::members(a)?;
                let b = b.clone();
                deep(root, a, &b)?;
            }
            _ => {
                into.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(())
}

/// Every token in a tree: its dotted path and definition, `$type` (or a
/// legacy group `type`) inherited from groups, `$root` a token of its group,
/// `$extends` and a group's `$ref` merged under it; a token that is a whole
/// `$ref` an alias. An error for a cycle, a missing target, nesting past
/// 64, or more work than one import may do (`crate::budget`).
fn tokens(file: &str, tree: &Value) -> Result<Vec<Def>, String> {
    fn walk(
        file: &str,
        root: &Value,
        node: &Value,
        path: &mut Vec<String>,
        ty: Option<String>,
        out: &mut Vec<Def>,
    ) -> Result<(), String> {
        let Some(map) = node.as_object() else {
            return Ok(());
        };
        if path.len() > 64 {
            return Err(format!("`{}` nests deeper than 64", path.join(".")));
        }
        crate::budget::spend(1)?;
        // A whole-token `$ref`: to a token, a `$value` or anything not a group, it is an alias.
        if let Some(r) = map.get("$ref").and_then(Value::as_str) {
            let to_group = target(root, r).is_some_and(|t| t.is_object() && !token_like(root, t));
            if !map.contains_key("$value") && !to_group {
                out.push(Def {
                    file: file.to_string(),
                    path: path.join("."),
                    ty: map
                        .get("$type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or(ty),
                    value: serde_json::json!({ "$ref": r }),
                    refused: None,
                });
                return Ok(());
            }
        }
        if is_token(node) {
            let own = map
                .get("$type")
                .and_then(Value::as_str)
                .or_else(|| {
                    map.get("type")
                        .filter(|_| !map.contains_key("$value"))
                        .and_then(Value::as_str)
                })
                .map(str::to_string);
            let value = map.get("$value").or_else(|| map.get("value"));
            crate::budget::copy(value.unwrap_or(&Value::Null))?;
            let value = value.cloned().unwrap_or(Value::Null);
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
                ty: own.or(ty),
                value,
                refused,
            });
            return Ok(());
        }
        let group = extended(root, map, &mut Vec::new())?;
        let ty = group
            .get("$type")
            .and_then(Value::as_str)
            .or_else(|| group.get("type").and_then(Value::as_str))
            .map(str::to_string)
            .or(ty);
        for (k, v) in &group {
            if k.starts_with('$') && k != "$root" {
                continue;
            }
            path.push(k.clone());
            walk(file, root, v, path, ty.clone(), out)?;
            path.pop();
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(file, tree, tree, &mut Vec::new(), None, &mut out).map_err(|e| format!("{file}: {e}"))?;
    Ok(out)
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

/// A file's name without its directory and its `.json`, any case.
fn stem(name: &str) -> String {
    set_id(name.rsplit('/').next().unwrap_or(name))
}

/// A file's path without its `.json`, any case. A name of any characters
/// (a person's file picker gives it) is cut only at a character boundary.
fn set_id(name: &str) -> String {
    let cut = name.len().saturating_sub(5);
    match name.len() > 5 && name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(".json")
    {
        true => name[..cut].to_string(),
        false => name.to_string(),
    }
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
    // One themes document or one resolver: a second would be read as
    // nothing, so it is refused, named.
    let roots: Vec<&String> = parsed
        .iter()
        .filter(|(n, v)| {
            stem(n) == "$themes" || v.get("$themes").is_some() || v.get("resolutionOrder").is_some()
        })
        .map(|(n, _)| n)
        .collect();
    if roots.len() > 1 {
        return Err(format!(
            "more than one themes or resolver document ({}); import them one at a time",
            roots
                .iter()
                .map(|n| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
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
                    .map(|(n, v)| (set_id(n), v.clone()))
                    .collect(),
            ),
        };
        return studio(&themes, metadata.as_ref(), &sets);
    }
    if let Some((name, doc)) = parsed
        .iter()
        .find(|(_, v)| v.get("resolutionOrder").is_some())
    {
        return resolver(name, doc, &parsed);
    }
    // Figma's `<Collection>.<Mode>.json`, each file on its own; any other file is the base.
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
    let mut layers: Vec<(u8, Vec<Scope>, Vec<Def>)> = Vec::new();
    for (n, v) in plain {
        layers.push((0, vec![Scope::Base], tokens(n, v)?));
    }
    for (collection, mut modes) in collections {
        modes.sort_by(|a, b| a.0.cmp(&b.0));
        let names: Vec<String> = modes.iter().map(|(m, _, _)| m.clone()).collect();
        for (mode, v, file) in &modes {
            let (rank, scope) = match (modes.len(), scheme(&names)) {
                (1, _) => (0, Scope::Base),
                (_, true) => (1, Scope::Scheme(is_dark(mode))),
                _ => (2, out.axis(&collection, mode)),
            };
            layers.push((rank, vec![scope], tokens(file, v)?));
        }
    }
    // The base, then the scheme, then the axes: a collection's own modes over
    // what every mode shares.
    layers.sort_by_key(|(rank, _, _)| *rank);
    for (_, scopes, found) in layers {
        out.layer(scopes, found);
    }
    Ok(out)
}

/// Tokens Studio's themes and sets: each set once, active in the modes of
/// every theme that turns it on, the sets in `tokenSetOrder`.
fn studio(
    themes: &Value,
    metadata: Option<&Value>,
    sets: &[(String, Value)],
) -> Result<Read, String> {
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
    // when that names one set only; sets it does not name after the ones it
    // does, in the files' order.
    // (each id's first place, and how many sets end in each last segment,
    // counted once: round 3 of the checker sweep)
    let mut place: HashMap<&str, usize> = HashMap::new();
    for (i, o) in order.iter().enumerate() {
        place.entry(o.as_str()).or_insert(i);
    }
    let mut ending: HashMap<&str, usize> = HashMap::new();
    for (n, _) in sets {
        *ending.entry(n.rsplit('/').next().unwrap_or(n)).or_insert(0) += 1;
    }
    let rank = |set: &str, at: usize| {
        if let Some(&i) = place.get(set) {
            return (i, at);
        }
        let last = set.rsplit('/').next().unwrap_or(set);
        match (ending.get(last).copied().unwrap_or(0), place.get(last)) {
            (1, Some(&i)) => (i, at),
            _ => (order.len(), at),
        }
    };
    let mut out = Read::default();
    let list: Vec<&Value> = themes
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    // Each set's modes: the scopes of the themes that turn it on.
    let mut modes: BTreeMap<String, Vec<Scope>> = BTreeMap::new();
    if list.is_empty() {
        for (n, _) in sets {
            modes.insert(n.clone(), vec![Scope::Base]);
        }
    }
    let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for t in list {
        // Ungrouped themes are one group.
        let group = t["group"].as_str().unwrap_or("").to_string();
        match seen.get(&group) {
            Some(&g) => groups[g].1.push(t),
            None => {
                seen.insert(group.clone(), groups.len());
                groups.push((group, vec![t]));
            }
        }
    }
    for (group, members) in &groups {
        let names: Vec<String> = members
            .iter()
            .map(|t| t["name"].as_str().unwrap_or("").to_string())
            .collect();
        // Each theme of a group of several is a mode: one with no name, or
        // two with one name, would fold into one mode unseen.
        let shown = if group.is_empty() {
            "the ungrouped themes".to_string()
        } else {
            format!("the theme group `{group}`")
        };
        // A group's themes are an axis's values: past the tuples a design
        // may have, refused before anything compares them (round 2 of the
        // checker sweep).
        if members.len() > crate::MAX_TUPLES {
            return Err(format!(
                "{shown} has {} themes; a design has at most {} tuples",
                members.len(),
                crate::MAX_TUPLES
            ));
        }
        if members.len() > 1 {
            if names.iter().any(|n| n.trim().is_empty()) {
                return Err(format!("{shown} has a theme with no name"));
            }
            if let Some(n) = names
                .iter()
                .enumerate()
                .find_map(|(i, n)| names[..i].contains(n).then_some(n))
            {
                return Err(format!("{shown} names two themes `{n}`"));
            }
        }
        let by_scheme = scheme(&names);
        for (t, theme) in members.iter().zip(&names) {
            let scope = match (members.len(), by_scheme) {
                (1, _) => Scope::Base,
                (_, true) => Scope::Scheme(is_dark(theme)),
                _ => out.axis(group, theme),
            };
            for (set, status) in t["selectedTokenSets"].as_object().into_iter().flatten() {
                if matches!(status.as_str(), Some("enabled" | "source")) {
                    let scopes = modes.entry(set.clone()).or_default();
                    if !scopes.contains(&scope) {
                        scopes.push(scope.clone());
                    }
                }
            }
        }
    }
    let mut ordered: Vec<(usize, &(String, Value))> = sets.iter().enumerate().collect();
    ordered.sort_by_key(|(at, (n, _))| rank(n, *at));
    for (_, (n, v)) in ordered {
        let found = tokens(n, v)?;
        match modes.get(n) {
            Some(scopes) => out.layer(scopes.clone(), found),
            None => out.unused(found, "in a set no theme turns on"),
        }
    }
    Ok(out)
}

/// A document and the directory its relative paths start from.
#[derive(Clone, Copy)]
struct Doc<'a> {
    name: &'a str,
    value: &'a Value,
}

impl Doc<'_> {
    /// `path` from this document's directory, normalised; one leaving the
    /// sources' root is an error.
    fn relative(&self, path: &str) -> Result<String, String> {
        let dir = self.name.rsplit_once('/').map_or("", |(d, _)| d);
        let mut parts: Vec<&str> = match dir {
            "" => Vec::new(),
            d => d.split('/').collect(),
        };
        for seg in path.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    parts
                        .pop()
                        .ok_or_else(|| format!("`{path}` leaves the sources"))?;
                }
                s => parts.push(s),
            }
        }
        Ok(parts.join("/"))
    }
}

/// A reference object resolved: the document holding what it names and
/// that node, its sibling keys merged over it, its own references followed
/// in the document that holds them.
fn deref<'a>(
    files: &'a [(String, Value)],
    doc: Doc<'a>,
    node: &Value,
    depth: usize,
) -> Result<(Doc<'a>, Value), String> {
    if depth > 16 {
        return Err("references nest too deep".into());
    }
    crate::budget::spend(1)?;
    let Some(r) = node.get("$ref").and_then(Value::as_str) else {
        crate::budget::copy(node)?;
        return Ok((doc, node.clone()));
    };
    // Sibling keys written here keep this document: a `sources` among them
    // is read where it was written.
    if node.get("sources").is_some() {
        let (_, mut value) = deref(files, doc, &serde_json::json!({ "$ref": r }), depth)?;
        if let (Some(over), Some(obj)) = (node.as_object(), value.as_object_mut()) {
            for (k, v) in over {
                if k != "$ref" {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        return Ok((doc, value));
    }
    let (file, pointer) = r.split_once('#').map_or((r, ""), |(f, p)| (f, p));
    let held = match file {
        "" => doc,
        f => {
            let path = doc.relative(f)?;
            let (n, v) = files
                .iter()
                .find(|(n, _)| *n == path)
                .ok_or_else(|| format!("no file `{path}` among the sources"))?;
            Doc { name: n, value: v }
        }
    };
    let found = match pointer {
        "" => held.value,
        p => held.value.pointer(p).ok_or_else(|| format!("no `{r}`"))?,
    };
    let (owner, mut value) = deref(files, held, found, depth + 1)?;
    if let (Some(over), Some(obj)) = (node.as_object(), value.as_object_mut()) {
        for (k, v) in over {
            if k != "$ref" {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    Ok((owner, value))
}

/// The root document's set a reference names, decoded, if it names one.
fn root_set(doc: Doc, r: &str, top: &str) -> Option<String> {
    let (file, pointer) = r.split_once('#')?;
    let held = match file {
        "" => doc.name.to_string(),
        f => doc.relative(f).ok()?,
    };
    let segs = pointer_segments(pointer.strip_prefix('/')?);
    (held == top && segs.len() == 2 && segs[0] == "sets").then(|| segs[1].clone())
}

/// A Resolver Module document.
fn resolver(name: &str, doc: &Value, files: &[(String, Value)]) -> Result<Read, String> {
    let top = Doc { name, value: doc };
    let mut reached: BTreeSet<String> = BTreeSet::new();
    // A source's tokens: a token tree, or a set whose sources are expanded
    // in turn (a cycle refused).
    fn expand(
        files: &[(String, Value)],
        doc: Doc,
        top: &str,
        src: &Value,
        stack: &mut Vec<String>,
        reached: &mut BTreeSet<String>,
        depth: usize,
    ) -> Result<Vec<Def>, String> {
        if depth > 16 {
            return Err("sets nest too deep".into());
        }
        crate::budget::spend(1)?;
        let mut pushed = false;
        if let Some(r) = src.get("$ref").and_then(Value::as_str) {
            let key = format!("{}{r}", doc.name);
            if stack.contains(&key) {
                return Err(format!("a cycle of sets through `{r}`"));
            }
            stack.push(key);
            pushed = true;
            // A set of the root document, by its decoded name.
            if let Some(name) = root_set(doc, r, top) {
                reached.insert(name);
            }
        }
        let (owner, tree) = deref(files, doc, src, 0)?;
        let out = match tree.get("sources").and_then(Value::as_array) {
            Some(srcs) => {
                let mut out = Vec::new();
                for s in srcs {
                    out.extend(expand(files, owner, top, s, stack, reached, depth + 1)?);
                }
                out
            }
            None => tokens(owner.name, &tree)?,
        };
        if pushed {
            stack.pop();
        }
        Ok(out)
    }
    let mut out = Read::default();
    let order = doc["resolutionOrder"]
        .as_array()
        .ok_or("`resolutionOrder` is a list")?;
    for entry in order {
        let (owner, node) = deref(files, top, entry, 0).map_err(|e| format!("{name}: {e}"))?;
        if let Some(set) = entry
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|r| root_set(top, r, name))
        {
            reached.insert(set);
        }
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
                    false => out.axis(&label, context),
                };
                let sources = contexts[context]
                    .as_array()
                    .ok_or_else(|| format!("{name}: `{label}.{context}` is a list of sources"))?;
                let mut found = Vec::new();
                for src in sources {
                    found.extend(
                        expand(files, owner, name, src, &mut Vec::new(), &mut reached, 0)
                            .map_err(|e| format!("{name}: {e}"))?,
                    );
                }
                out.layer(vec![scope], found);
            }
        } else {
            let sources = node.get("sources").and_then(Value::as_array).ok_or_else(|| format!("{name}: `{label}` is a set (with `sources`) or a modifier (with `contexts`)"))?;
            let mut found = Vec::new();
            for src in sources {
                found.extend(
                    expand(files, owner, name, src, &mut Vec::new(), &mut reached, 0)
                        .map_err(|e| format!("{name}: {e}"))?,
                );
            }
            out.layer(vec![Scope::Base], found);
        }
    }
    // A set the order never reaches: its tokens counted and reported.
    for (set, node) in doc
        .get("sets")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        if reached.contains(set) {
            continue;
        }
        // Not part of the resolution: a set that does not read is reported
        // as one row, and the rest go on.
        match expand(files, top, name, node, &mut Vec::new(), &mut reached, 0) {
            Ok(found) => out.unused(found, "in a set the resolution order never reaches"),
            Err(why) => out.unused(
                vec![Def {
                    file: name.to_string(),
                    path: format!("sets.{set}"),
                    ty: None,
                    value: Value::String(why.clone()),
                    refused: Some(why),
                }],
                "a set the resolution order never reaches, which does not read",
            ),
        }
    }
    Ok(out)
}

/// What Tamber makes of a `$type`.
enum Kind {
    Colour,
    /// Another kind, by the name `add-token` writes.
    Other(&'static str),
    Reported(&'static str),
}

fn kind(ty: Option<&str>) -> Kind {
    match ty.unwrap_or("") {
        "color" => Kind::Colour,
        "dimension" | "borderRadius" | "spacing" | "sizing" | "borderWidth" | "fontSizes"
        | "letterSpacing" | "paragraphSpacing" => Kind::Other("length"),
        "fontFamily" | "fontFamilies" => Kind::Other("family"),
        "fontWeight" | "fontWeights" | "number" | "lineHeights" | "opacity" => {
            Kind::Other("number")
        }
        "duration" => Kind::Other("duration"),
        "cubicBezier" => Kind::Other("easing"),
        "shadow" | "boxShadow" => Kind::Other("box-shadow"),
        "typography" => Kind::Other("type"),
        "border" => Kind::Reported("no Tamber kind: a border"),
        "transition" => Kind::Reported("no Tamber kind: a transition"),
        "gradient" => Kind::Reported("no Tamber kind: a gradient (its stops stay where they are)"),
        "strokeStyle" => Kind::Reported("no Tamber kind: a stroke style"),
        "boolean" | "string" => Kind::Reported("no Tamber kind: a string or a boolean"),
        "" => Kind::Reported("no `$type`, on it, a group around it, or the token it aliases"),
        _ => Kind::Reported("a `$type` Tamber's DTCG mapping does not name"),
    }
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
    let segs = pointer_segments(pointer);
    match segs.iter().position(|s| s == "$value") {
        Some(i) if i + 1 == segs.len() => Some(Ok(segs[..i].join("."))),
        Some(_) => Some(Err(format!(
            "`{r}` names part of a value, which no Tamber value reads"
        ))),
        None => Some(Ok(segs.join("."))),
    }
}

/// One mode tuple: the scheme's side and a value of each axis.
type Tuple = (bool, Vec<String>);

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
    import_with(files, prefix, design, &crate::Vocab::default())
}

/// [`import`], the design checked against `vocab` where the import lands.
pub fn import_with(
    files: &[(String, String)],
    prefix: &str,
    design: &Design,
    vocab: &crate::Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    let _work = crate::budget::Work::begin(crate::budget::IMPORT);
    let read = read(files).map_err(|m| vec![Diagnostic::new("tamber-import-source", m, 1, 1)])?;
    planned(read, prefix, design, vocab)
}

/// The import a source's layers make (DTCG's, or another format read into
/// them: `pen`), planned against `design`; inside the caller's budget.
fn planned(
    read: Read,
    prefix: &str,
    design: &Design,
    vocab: &crate::Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    super::bounded(planned_unbounded(read, prefix, design, vocab))
}

fn planned_unbounded(
    read: Read,
    prefix: &str,
    design: &Design,
    vocab: &crate::Vocab,
) -> Result<Import, Vec<Diagnostic>> {
    for d in &read.defs {
        let mut strings = Vec::new();
        super::json_strings(&d.value, &mut strings);
        super::source_bound(strings.into_iter().map(|v| (d.path.as_str(), v)))?;
    }
    let refuse = |id: &'static str, m: String| vec![Diagnostic::new(id, m, 1, 1)];
    let defs = &read.defs;
    // The dimensions: the scheme, and each axis with its values in order.
    // Two axes, or two values of one, written differently but one
    // identifier: no proposal can tell them apart.
    for ((axis, value), written) in &read.written {
        if written.len() > 1 {
            let what = match value {
                None => format!("the axis `{axis}`"),
                Some(v) => format!("the value `{v}` of `{axis}`"),
            };
            let all: Vec<&str> = written.iter().map(String::as_str).collect();
            return Err(refuse(
                "tamber-import-axis",
                format!(
                    "{what} is written {}: name them apart in the source",
                    all.join(" and ")
                ),
            ));
        }
    }
    let has_scheme = read.layers.iter().any(|l| {
        l.scopes
            .iter()
            .flat_map(Scope::flat)
            .any(|s| matches!(s, Scope::Scheme(_)))
    });
    let mut axes: Vec<(String, Vec<String>)> = read.dims.clone();
    for l in &read.layers {
        for scope in l.scopes.iter().flat_map(Scope::flat) {
            if let Scope::Axis(a, v) = scope {
                match axes.iter_mut().find(|(n, _)| n == a) {
                    Some((_, vs)) if !vs.contains(v) => vs.push(v.clone()),
                    Some(_) => {}
                    None => axes.push((a.clone(), vec![v.clone()])),
                }
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
            if have.under_dark {
                return Err(refuse(
                    "tamber-import-axis",
                    format!("the design's axis `{a}` is `under scheme=dark`, which the source's modes are not"),
                ));
            }
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
    // Each axis as reconcile leaves it: the design's values and the source's.
    let app_tuples = axes.iter().try_fold(others, |acc, (a, vs)| {
        let have = design
            .axes
            .iter()
            .find(|x| x.name.value == *a)
            .map_or(Vec::new(), |x| x.values.clone());
        acc.checked_mul(have.len() + vs.iter().filter(|v| !have.contains(v)).count())
    });
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
    // (a def no layer uses makes no name, nor takes one; each name made charged)
    let unused: BTreeSet<usize> = read.unused.iter().map(|(i, _)| *i).collect();
    for (i, d) in defs.iter().enumerate() {
        if unused.contains(&i) || names.contains_key(&d.path) || collided.contains_key(&d.path) {
            continue;
        }
        crate::budget::spend(2 * (prefix.len() + d.path.len()) as u64)
            .map_err(|m| refuse("tamber-import-source", m))?;
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
    let active = |t: &Tuple, scopes: &[Scope]| scopes.iter().any(|scope| scope.holds(t, &axes));
    // (each entry charged: a tuple holds every path, so tuples times paths are kept)
    let mut winners: Vec<BTreeMap<&str, usize>> = Vec::with_capacity(tuples.len());
    for t in &tuples {
        let mut w = BTreeMap::new();
        // A colliding path still resolves; only its name is not made.
        for l in read.layers.iter().filter(|l| active(t, &l.scopes)) {
            for &i in l.defs.iter().filter(|i| !unused.contains(i)) {
                crate::budget::spend(1).map_err(|m| refuse("tamber-import-source", m))?;
                w.insert(defs[i].path.as_str(), i);
            }
        }
        winners.push({
            // A token and a group at one path: the later layer's stands,
            // the other's tokens go (a token replaces a group, a group a
            // token).
            // (each path against its own dotted ancestors, not every other path)
            let paths: BTreeMap<&str, usize> = if read.flat {
                BTreeMap::new()
            } else {
                w.clone()
            };
            for (q, j) in &paths {
                for (at, _) in q.match_indices('.') {
                    match paths.get(&q[..at]) {
                        Some(i) if j < i => {
                            w.remove(q);
                        }
                        Some(i) if i < j => {
                            w.remove(&q[..at]);
                        }
                        _ => {}
                    }
                }
            }
            w
        });
    }
    // Every path's colour in every tuple, or why it makes no token.
    let paths: Vec<&str> = names.keys().map(String::as_str).collect();
    let mut values: BTreeMap<&str, Vec<Colour>> = BTreeMap::new();
    // Each path's kind besides colour.
    let mut kinds_of: BTreeMap<&str, &'static str> = BTreeMap::new();
    let mut fates: BTreeMap<&str, String> = BTreeMap::new();
    let mut resolvers: Vec<Resolve> = winners
        .iter()
        .map(|w| Resolve {
            defs,
            w,
            names: &names,
            memo: HashMap::new(),
            types: RefCell::new(HashMap::new()),
        })
        .collect();
    for &path in &paths {
        let mut per = Vec::new();
        let mut failure: Option<String> = None;
        let mut first: Option<&'static str> = None;
        for ((t, w), r) in tuples.iter().zip(&winners).zip(resolvers.iter_mut()) {
            // (named only for a failure's words: not made for every path in every tuple)
            let label = || tuple_name(t, has_scheme, &axes);
            if !w.contains_key(path) {
                failure.get_or_insert(format!(
                    "not set in {}, and no value of it is invented",
                    label()
                ));
                continue;
            }
            let ty = r.type_of(path);
            let k = match kind(ty.as_deref()) {
                Kind::Colour => "colour",
                Kind::Other(k) => k,
                Kind::Reported(_) => "",
            };
            if let Some(f) = first.replace(k).filter(|f| *f != k) {
                let one = match (f, k) {
                    ("colour", _) | (_, "colour") => "colour",
                    ("", k) => k,
                    (f, _) => f,
                };
                failure.get_or_insert(format!("a {one} in some modes, another kind in others"));
            }
            match kind(ty.as_deref()) {
                Kind::Colour => match r.seen(path, &mut BTreeSet::from([path.to_string()])) {
                    Ok(v) => per.push(v),
                    Err(why) => {
                        failure.get_or_insert(format!("{why} ({})", label()));
                    }
                },
                Kind::Other(k) => {
                    let found = r
                        .raw(path, &mut BTreeSet::from([path.to_string()]))
                        .and_then(|v| kinds::text(ty.as_deref().unwrap_or_default(), &v));
                    match found {
                        Ok(v) => {
                            // (kept twice a tuple, as a colour's names are: charged)
                            crate::budget::spend(2 * v.len() as u64)
                                .map_err(|m| refuse("tamber-import-source", m))?;
                            per.push((v.clone(), v, None))
                        }
                        Err(why) => {
                            failure.get_or_insert(format!("{why} ({})", label()));
                        }
                    }
                    kinds_of.insert(path, k);
                }
                _ => {
                    failure.get_or_insert(String::new());
                }
            }
        }
        // One value on both sides of the scheme, but for a colour.
        // (each tuple's value beside its axis values' other side: one pass, not every pair)
        if failure.is_none() && kinds_of.contains_key(path) {
            let mut side: HashMap<&[String], (bool, &str)> = HashMap::new();
            let sided = tuples.iter().zip(&per).any(|(t, v)| {
                match side.insert(&t.1, (t.0, v.0.as_str())) {
                    Some((dark, x)) => dark != t.0 && x != v.0,
                    None => false,
                }
            });
            if sided {
                failure = Some(other::SIDED.to_string());
            }
        }
        match failure {
            None => {
                values.insert(path, per);
            }
            Some(why) => {
                // (a why names a tuple: kept, so charged)
                crate::budget::spend(why.len() as u64)
                    .map_err(|m| refuse("tamber-import-source", m))?;
                fates.insert(path, why);
            }
        }
    }
    // A walk the budget ran out in is the import's refusal, not a value's. A reference to a token
    // not made is written as its colour; one round a cycle of tokens too.
    crate::budget::spend(1).map_err(|m| refuse("tamber-import-source", m))?;
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
    // (each step charged: a long chain walked from every link is a square of it)
    let reaches = |from: &str| -> Result<bool, String> {
        let mut stack: Vec<String> = graph
            .get(from)
            .map_or(Vec::new(), |s| s.iter().cloned().collect());
        let mut visited = BTreeSet::new();
        while let Some(n) = stack.pop() {
            crate::budget::spend(1)?;
            if n == from {
                return Ok(true);
            }
            if visited.insert(n.clone()) {
                stack.extend(graph.get(n.as_str()).into_iter().flatten().cloned());
            }
        }
        Ok(false)
    };
    let mut cyclic: BTreeSet<&str> = BTreeSet::new();
    for p in graph.keys().copied() {
        if reaches(p).map_err(|m| refuse("tamber-import-source", m))? {
            cyclic.insert(p);
        }
    }
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
    // (each tuple's place found once: a path's lines ask it for every combination)
    let place: HashMap<(bool, &[String]), usize> = tuples
        .iter()
        .enumerate()
        .map(|(i, t)| ((t.0, t.1.as_slice()), i))
        .collect();
    let value_at = |per: &Vec<Colour>, combo: &Vec<String>| {
        let at = |dark: bool| {
            place
                .get(&(dark, combo.as_slice()))
                .map(|&i| per[i].0.clone())
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
    let mut made: Vec<&str> = Vec::new();
    for (path, per) in &values {
        let value = value_at(per, &default_combo);
        let mut whens: Vec<(Vec<(String, String)>, String)> = Vec::new();
        for combo in &combos {
            // (each combination's lines weighed against those made so far: charged)
            crate::budget::spend(1 + whens.len() as u64)
                .map_err(|m| refuse("tamber-import-source", m))?;
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
            kind: kinds_of.get(path).map(|k| other::written(k)),
        });
        made.push(path);
    }
    // Each of another kind that does not fold, and each colour that would not
    // hold where it lands, makes no token.
    let owned: Vec<&str> = axes.iter().map(|(a, _)| a.as_str()).collect();
    let why = other::refused(&tokens, (&axes, &owned), design, vocab).map_err(|d| vec![d])?;
    for (path, why) in made.iter().zip(&why) {
        if let Some(why) = why {
            let why = kinds_of
                .get(path)
                .map_or(why.clone(), |k| format!("not a {k} Tamber folds: {why}"));
            fates.insert(path, why);
            values.remove(path);
        }
    }
    let mut at = 0;
    tokens.retain(|_| {
        at += 1;
        why[at - 1].is_none()
    });
    // Each def: mapped when it wins a tuple of a token made, else reported.
    let wins: BTreeSet<usize> = winners.iter().flat_map(|w| w.values().copied()).collect();
    let mut unmapped = Vec::new();
    let mut mapped = 0;
    // (indexed: a source of many unused defs is no square of them)
    let unused_why: HashMap<usize, &String> = read.unused.iter().map(|(i, w)| (*i, w)).collect();
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
        if let Some(why) = unused_why.get(&i) {
            unmapped.push(report((*why).clone(), None));
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
                .or_else(|| resolvers.iter().find_map(|r| r.type_of(&d.path)));
        let why = match (&d.refused, kind(ty.as_deref())) {
            (Some(why), _) => why.clone(),
            (None, Kind::Reported(why)) => {
                unmapped.push(report(why.to_string(), None));
                continue;
            }
            (None, Kind::Colour | Kind::Other(_)) => fates
                .get(d.path.as_str())
                .cloned()
                .filter(|w| !w.is_empty())
                .unwrap_or_else(|| "a colour in some modes, another kind in others".into()),
        };
        unmapped.push(report(why, token));
    }
    let ops = reconcile(design, &axes, &owned, &tokens);
    let reported = unmapped.len();
    let counted = counted(defs.len(), mapped, reported)?;
    Ok(Import {
        ops,
        offers: Vec::new(),
        unmapped,
        counted,
        dials: Vec::new(),
        pins: Vec::new(),
        notes: Vec::new(),
        accent: None,
    })
}
