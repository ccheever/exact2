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
