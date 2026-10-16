//! `uses` (LLP 1121 stage 4, commit 6; LLP 1122 T2b's `outline`): where the
//! app reads a token, answered statically. The plan's style bindings whose
//! code names the token, directly, through a token whose value names it, or
//! through a synthetic token (a row's expression over tokens) that reads
//! it; each joined with the plan's source map into a source span. A
//! mounted-instance count would need a report from the builds, which the
//! build channel does not carry (LLP 1121 §9 candidate).

use exact_plan::{BindingKind, Opcode, Operand, Plan};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Whether `text` names `name` as a whole token name: not inside a longer
/// one (`accent.fill` is not in `accent.fill-2`).
pub fn names(text: &str, name: &str) -> bool {
    let part = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_');
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(part)
            && !text[i + name.len()..].chars().next().is_some_and(part)
    })
}

/// The strings `code` pushes, in order; empty when it does not walk whole.
fn strings<'a>(plan: &'a Plan, code: &[u8]) -> Vec<&'a str> {
    let mut r = exact_plan::bytes::Reader::new(code);
    let mut out = Vec::new();
    while !r.is_empty() {
        let Some(op) = r.u8().ok().and_then(Opcode::from_wire) else {
            return Vec::new();
        };
        for operand in op.operands() {
            let ok = match operand {
                Operand::U8 | Operand::Enum(_) => r.u8().is_ok(),
                Operand::U16 => r.u16().is_ok(),
                Operand::U32 | Operand::Idx(_) => r.u32().is_ok(),
                Operand::F64 => r.f64().is_ok(),
                Operand::Str => match r.u32() {
                    Ok(v) => {
                        if let Some(s) = plan.strings.get(v as usize) {
                            out.push(s.as_str());
                        }
                        true
                    }
                    Err(_) => false,
                },
            };
            if !ok {
                return Vec::new();
            }
        }
    }
    out
}

/// Every site whose style rows read `token`. `aliases` are the tokens whose
/// value names it (closed under naming); `synthetic` the app's synthetic
/// expressions. `map` is the plan's source map as JSON (`SourceMap::json`),
/// node by node.
pub fn sites(
    plan: &Plan,
    map: &Value,
    token: &str,
    aliases: &BTreeSet<String>,
    synthetic: &[String],
) -> Vec<Value> {
    // What a row may name to read it: the token, a token that aliases it,
    // or a synthetic expression over either.
    let via_token = |s: &str| {
        if names(s, token) {
            return Some(Value::Null);
        }
        aliases
            .iter()
            .find(|a| names(s, a))
            .map(|a| json!({"token": a}))
    };
    let via_synthetic: Vec<(&str, Value)> = synthetic
        .iter()
        .filter_map(|e| via_token(e).map(|v| (e.as_str(), v)))
        .collect();
    let mut out = Vec::new();
    for (i, node) in plan.nodes.iter().enumerate() {
        let site = &map["nodes"][i];
        let range =
            node.bindings.start as usize..(node.bindings.start + node.bindings.len) as usize;
        for b in &plan.bindings[range] {
            if b.kind != BindingKind::Style {
                continue;
            }
            let Some(row) = exact_kernel::StyleId::from_bit(b.id as u32) else {
                continue;
            };
            let found = strings(plan, plan.code(b.expr)).into_iter().find_map(|s| {
                // A synthetic token is its expression's text, in the row's.
                via_synthetic
                    .iter()
                    .find(|(e, _)| s.contains(e))
                    .map(|(e, v)| json!({"synthetic": e, "through": v}))
                    .or_else(|| via_token(s))
            });
            let Some(via) = found else { continue };
            let mut at = json!({
                "site": i,
                "row": row.name(),
                "file": site["file"],
                "line": site["line"],
                "col": site["col"],
                "end_col": site["end_col"],
                "component": site["component"],
            });
            if !via.is_null() {
                at["via"] = via;
            }
            out.push(at);
        }
    }
    out
}

/// The tokens whose values name `token`, directly or through another:
/// `names` gives each token's named tokens.
pub fn aliases<'a>(
    token: &str,
    names: impl Iterator<Item = (&'a str, &'a [String])> + Clone,
) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut frontier = vec![token.to_string()];
    while let Some(t) = frontier.pop() {
        for (name, named) in names.clone() {
            if named.contains(&t) && name != token && found.insert(name.to_string()) {
                frontier.push(name.to_string());
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_matched_whole() {
        assert!(names(
            "linear-gradient(90deg, accent.fill, surface.card)",
            "accent.fill"
        ));
        assert!(names("accent.fill", "accent.fill"));
        assert!(!names("accent.fill-2", "accent.fill"));
        assert!(!names("my.accent.fill", "accent.fill"));
        assert!(names("drop-shadow(0 0 4px accent.soft)", "accent.soft"));
    }

    #[test]
    fn aliases_close_under_naming() {
        let table = [
            ("a".to_string(), vec!["b".to_string()]),
            ("b".to_string(), vec!["c".to_string()]),
            ("d".to_string(), vec![]),
        ];
        let it = table.iter().map(|(n, v)| (n.as_str(), v.as_slice()));
        let found = aliases("c", it);
        assert_eq!(found.into_iter().collect::<Vec<_>>(), ["a", "b"]);
    }
}
