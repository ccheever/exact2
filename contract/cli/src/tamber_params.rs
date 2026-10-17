//! The IR's `components` block (LLP 1121 D15, stage 7 A): each component of
//! the app that declares params, by its identity, and each param by its full
//! name, with its short name where it is unambiguous, its token name below
//! the IR, its kind, its default as written, its value per tuple, and the
//! editor's metadata. `describe` and `contract tamber ir` return it beside
//! the design's IR. Read from the app's Contract, loaded as a build loads it
//! (the same identities and token names).

use contract_lower::params::{self, ParamToken};
use contract_syntax::params::ParamValue;
use serde_json::{json, Value};
use std::path::Path;

/// The app's params, loaded from `app_contract`; none when it has no
/// Contract, or it does not load (the build says why).
pub fn params_of(app_contract: &Path) -> Vec<ParamToken> {
    let Ok(src) = std::fs::read_to_string(app_contract) else {
        return Vec::new();
    };
    let Ok(root) = crate::app_root(app_contract) else {
        return Vec::new();
    };
    match crate::sources::load(app_contract, &src, &root) {
        Ok((file, _)) => params::tokens(&file),
        Err(_) => Vec::new(),
    }
}

/// A param's short name, `Component.param`, when no other component of
/// that name declares params.
fn short(p: &ParamToken, all: &[ParamToken]) -> Option<String> {
    let component = p
        .identity
        .rsplit_once('#')
        .map_or(p.identity.as_str(), |(_, c)| c);
    let ambiguous = all.iter().any(|o| {
        o.identity != p.identity
            && o.identity
                .rsplit_once('#')
                .is_some_and(|(_, c)| c == component)
    });
    (!ambiguous).then(|| format!("{component}.{}", p.param.name))
}

/// The `components` block for `all`, each param's value per tuple folded
/// with `design` (the app's, or none).
pub fn components(
    all: &[ParamToken],
    design: Option<&exact_tamber::Design>,
    vocab: &exact_tamber::Vocab,
) -> Value {
    let mut folded = design.cloned().unwrap_or_default();
    params::add_to(&mut folded, all);
    let folding = exact_tamber::check_design(folded, vocab).ok();
    let mut out: Vec<Value> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for p in all {
        if seen.contains(&p.identity.as_str()) {
            continue;
        }
        seen.push(&p.identity);
        let list: Vec<Value> = all
            .iter()
            .filter(|q| q.identity == p.identity)
            .map(|q| {
                let attr = |name: &str| q.param.attr(name).map(ParamValue::text);
                let num = |name: &str| q.param.attr(name).and_then(params::number);
                let values: Option<Vec<String>> = folding.as_ref().and_then(|f| {
                    let t = f.tokens.iter().find(|t| t.name == q.token)?;
                    Some(t.values.iter().map(|v| v.css()).collect())
                });
                json!({
                    "name": q.full,
                    "short": short(q, all),
                    "param": q.param.name,
                    "token": q.token,
                    "kind": q.param.kind,
                    "default": params::shown(&q.param.default, all),
                    "values": values,
                    "range": params::range(&q.param).and_then(Result::ok).map(|(lo, hi)| json!([lo, hi])),
                    "step": num("step"),
                    "control": attr("control").unwrap_or_else(|| "token".into()),
                    "options": attr("options").map(|o| o.split('|').map(|s| s.trim().to_string()).collect::<Vec<_>>()),
                    "label": attr("label"),
                    "help": attr("help"),
                    "group": attr("group"),
                    "priority": num("priority").unwrap_or(2.0),
                })
            })
            .collect();
        let name = p
            .identity
            .rsplit_once('#')
            .map_or(p.identity.as_str(), |(_, c)| c);
        out.push(json!({"identity": p.identity, "name": name, "params": list}));
    }
    Value::Array(out)
}
