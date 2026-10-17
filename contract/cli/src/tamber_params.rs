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

/// The app's params, and the identity of every component it has, with
/// params or none (a short name is ambiguous when two share a name).
#[derive(Debug, Clone, Default)]
pub struct AppParams {
    /// Every param, as a token.
    pub tokens: Vec<ParamToken>,
    /// Every component's identity.
    pub identities: Vec<String>,
}

/// The app's params, loaded from `app_contract`; none when it has no
/// Contract, or it does not load (the build says why).
pub fn params_of(app_contract: &Path) -> AppParams {
    let Ok(src) = std::fs::read_to_string(app_contract) else {
        return AppParams::default();
    };
    let Ok(root) = crate::app_root(app_contract) else {
        return AppParams::default();
    };
    match crate::sources::load(app_contract, &src, &root) {
        Ok((file, _)) => AppParams {
            tokens: params::tokens(&file),
            identities: params::identities(&file),
        },
        Err(_) => AppParams::default(),
    }
}

/// Diagnostic `d` of a design folded with `params` where an author finds
/// it: one of a param's (on [`params::LINE`] and after) at its default, in
/// its declaring file, which is returned; every name in full.
pub fn relocate(d: &mut exact_tamber::Diagnostic, params: &[ParamToken]) -> Option<String> {
    d.message = params::shown(&d.message, params);
    let p = params.get(d.line.checked_sub(params::LINE)? as usize)?;
    d.line = p.param.default_span.line;
    d.col = p.param.default_span.col;
    Some(
        p.identity
            .rsplit_once('#')
            .map_or(p.identity.clone(), |(f, _)| f.to_string()),
    )
}

/// [`relocate`]d, as a line a terminal prints: `file:line:col: id: message`,
/// the design's file `design` unless a param's.
pub fn located(d: &exact_tamber::Diagnostic, params: &[ParamToken], design: &Path) -> String {
    let mut d = d.clone();
    match relocate(&mut d, params) {
        Some(file) => format!("{file}:{d}"),
        None => format!("{}:{d}", design.display()),
    }
}

/// `text`, a design, folded with `params`, and the design as parsed,
/// without them; a refusal as the fold gives it ([`relocate`] it).
pub fn fold_with(
    text: &str,
    params: &[ParamToken],
    vocab: &exact_tamber::Vocab,
) -> Result<(exact_tamber::Folding, exact_tamber::Design), Vec<exact_tamber::Diagnostic>> {
    let design = exact_tamber::parse(text)?;
    let mut laid = design.clone();
    params::add_to(&mut laid, params);
    let folding = exact_tamber::check_design(laid, vocab)?;
    Ok((folding, design))
}

/// The IR's tokens with every param's left out: a tool reads params by
/// full name, in `components`, never by their names below the IR.
pub fn hide(ir: &mut Value) {
    for list in ["tokens", "icons"] {
        if let Some(items) = ir.get_mut(list).and_then(Value::as_array_mut) {
            items.retain(|t| {
                !t["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with(params::PREFIX))
            });
        }
    }
}

/// A param's short name, `Component.param`, when no other component of
/// that name is in the app (`identities`), with params or none.
fn short(p: &ParamToken, identities: &[String]) -> Option<String> {
    let component = |identity: &str| {
        identity
            .rsplit_once('#')
            .map_or(identity.to_string(), |(_, c)| c.to_string())
    };
    let name = component(&p.identity);
    let ambiguous = identities
        .iter()
        .any(|o| *o != p.identity && component(o) == name);
    (!ambiguous).then(|| format!("{name}.{}", p.param.name))
}

/// The `components` block for `all`, each param's value per tuple folded
/// with `design` (the app's, or none).
pub fn components(
    app: &AppParams,
    design: Option<&exact_tamber::Design>,
    vocab: &exact_tamber::Vocab,
) -> Value {
    let all = app.tokens.as_slice();
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
                // An icon's sources, the same in every tuple.
                let values: Option<Value> = folding.as_ref().and_then(|f| match q.param.kind.as_str() {
                    "icon" => {
                        let i = f.icons.iter().find(|i| i.name == q.token)?;
                        let one = json!({"sf": i.sf, "material": i.material, "svg": i.svg});
                        Some(json!(vec![one; f.tuples]))
                    }
                    _ => {
                        let t = f.tokens.iter().find(|t| t.name == q.token)?;
                        Some(json!(t.values.iter().map(|v| v.css()).collect::<Vec<_>>()))
                    }
                });
                json!({
                    "name": q.full,
                    "short": short(q, &app.identities),
                    "param": q.param.name,
                    "token": q.token,
                    "kind": q.param.kind,
                    "default": params::shown(&q.param.default, all),
                    "values": values,
                    "range": params::range(&q.param).and_then(Result::ok).map(|(lo, hi)| json!([lo, hi])),
                    "step": num("step"),
                    "control": attr("control").unwrap_or_else(|| "token".into()),
                    "options": attr("options").map(|o| params::shown(&o, all).split('|').map(|s| s.trim().to_string()).collect::<Vec<_>>()),
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
