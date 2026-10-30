//! A DTCG export's token trees and the resolver's references to them,
//! read back for a re-import (moved from `export.rs`, under its line cap).

use super::*;

/// A token tree's tokens: each name (a group's `$root` the group's) and its
/// token object.
pub(super) fn flatten(tree: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    let Some(obj) = tree.as_object() else { return };
    if obj.contains_key("$value") {
        out.push((prefix.to_string(), tree.clone()));
        return;
    }
    for (k, v) in obj {
        let name = match (k.as_str(), prefix) {
            ("$root", _) => prefix.to_string(),
            (k, _) if k.starts_with('$') => continue,
            (k, "") => k.to_string(),
            (k, p) => format!("{p}.{k}"),
        };
        flatten(v, &name, out);
    }
}

/// A context source's tokens, in order: a token tree, or a reference to a
/// set or a tree in the same document (`#/sets/…`), resolved as the
/// Resolver Module says. A reference elsewhere, a cycle, or a document
/// whose references multiply past one import's work is refused, charged
/// before each tree is copied.
pub(super) fn source_tokens(
    doc: &Value,
    src: &Value,
    depth: usize,
    out: &mut Vec<(String, Value)>,
) -> Result<(), String> {
    if depth > 16 {
        return Err("references nest too deep".into());
    }
    crate::budget::spend(1)?;
    let Some(r) = src.get("$ref").and_then(Value::as_str) else {
        crate::budget::copy(src)?;
        flatten(src, "", out);
        return Ok(());
    };
    let target = r
        .strip_prefix('#')
        .and_then(|p| doc.pointer(p))
        .ok_or_else(|| format!("`{r}` names nothing in this document"))?;
    match target.get("sources").and_then(Value::as_array) {
        Some(srcs) => {
            for s in srcs {
                source_tokens(doc, s, depth + 1, out)?;
            }
        }
        None => {
            crate::budget::copy(target)?;
            flatten(target, "", out);
        }
    }
    Ok(())
}

/// Each context's tokens by name, from a document's `mode` modifier, a
/// later source after an earlier.
pub(crate) type ByContext = std::collections::BTreeMap<String, Vec<(String, Value)>>;

pub(super) fn tokens_by_context(doc: &Value) -> Result<ByContext, String> {
    let mut out = ByContext::new();
    for (context, srcs) in doc["modifiers"]["mode"]["contexts"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let mut all = Vec::new();
        for src in srcs.as_array().into_iter().flatten() {
            source_tokens(doc, src, 0, &mut all).map_err(|e| format!("`{context}`: {e}"))?;
        }
        out.insert(context.clone(), all);
    }
    Ok(out)
}

/// The export's header, when `doc` carries it.
pub(super) fn header(doc: &Value) -> Option<&Value> {
    let h = &doc["sets"]["exact"]["sources"][0]["$extensions"]["exact"];
    (h["export"] == MARK).then_some(h)
}

/// A context's tuple and side, read from its name (`axis=value, …, dark`),
/// for an export a tool stripped of its `$extensions`.
pub(super) fn named(context: &str) -> Option<(Modes, bool)> {
    let mut parts: Vec<&str> = context.split(", ").collect();
    let dark = match parts.pop()? {
        "light" => false,
        "dark" => true,
        _ => return None,
    };
    let modes = parts
        .iter()
        .map(|p| {
            p.split_once('=')
                .map(|(a, v)| (a.to_string(), v.to_string()))
        })
        .collect::<Option<Modes>>()?;
    Some((modes, dark))
}

/// Each context of an export, its tuple and side: from the header, or from
/// the names of the `mode` modifier's contexts when a tool dropped it.
pub(super) fn context_map(doc: &Value) -> Option<Vec<(String, Modes, bool)>> {
    let contexts = doc
        .get("modifiers")?
        .get("mode")?
        .get("contexts")?
        .as_object()?;
    if let Some(h) = header(doc) {
        let all: Vec<(String, Modes, bool)> = h["contexts"]
            .as_object()?
            .iter()
            .map(|(name, c)| {
                // In the design's axis order, as the header lists the axes.
                let order = |a: &str| {
                    h["axes"]
                        .as_array()
                        .and_then(|all| all.iter().position(|x| x["name"] == a))
                };
                let mut modes: Modes = c["modes"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(a, v)| (a.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect();
                modes.sort_by_key(|(a, _)| order(a));
                (name.clone(), modes, c["scheme"] == "dark")
            })
            .collect();
        return all
            .iter()
            .all(|(n, ..)| contexts.contains_key(n))
            .then_some(all);
    }
    // Without the header, the export's shape: a set `exact` and the one
    // modifier `mode`, in that order, its contexts named for their tuples.
    let refs: Vec<&str> = doc
        .get("resolutionOrder")?
        .as_array()?
        .iter()
        .filter_map(|r| r["$ref"].as_str())
        .collect();
    if refs != ["#/sets/exact", "#/modifiers/mode"] || doc["modifiers"].as_object()?.len() != 1 {
        return None;
    }
    contexts
        .keys()
        .map(|name| named(name).map(|(m, d)| (name.clone(), m, d)))
        .collect()
}
