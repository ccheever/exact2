//! The wire's operations (`tamber/1`, LLP 1121 D13): each `ops` entry read
//! into the crate's [`Op`], with the file it edits.

use crate::tamber_serve::{optional, required};
use exact_tamber::Op;
use serde_json::{json, Value};

/// The wire's operations (D13's names, kebab-case; named fields), each with
/// the file it edits (the design's own when unsaid). A `restore` is no
/// client's to send.
pub(crate) fn parse_ops(ops: &Value, design: &str) -> Result<Vec<(String, Op)>, Value> {
    let bad = |m: String| json!({"message": m});
    let s = required;
    let opt = optional;
    let list = |v: &Value, k: &str| -> Result<Vec<Value>, Value> {
        match &v[k] {
            Value::Null => Ok(Vec::new()),
            Value::Array(a) => Ok(a.clone()),
            _ => Err(bad(format!("`{k}` is a list"))),
        }
    };
    let conds = |v: &Value| -> Result<Vec<(String, String)>, Value> {
        list(v, "when")?
            .iter()
            .map(|c| Ok((s(c, "axis")?, s(c, "value")?)))
            .collect()
    };
    let mut out = Vec::new();
    for v in ops
        .as_array()
        .ok_or_else(|| bad("`ops` is a list".into()))?
    {
        if !v.is_object() {
            return Err(bad("each of `ops` is an object".into()));
        }
        let file = opt(v, "file")?.unwrap_or_else(|| design.to_string());
        let op = match v["op"].as_str() {
            Some("set-dial") => Op::SetDial {
                name: s(v, "name")?,
                value: s(v, "value")?,
            },
            Some("set-token") => Op::SetToken {
                name: s(v, "name")?,
                when: conds(v)?,
                value: s(v, "value")?,
                at: None,
                comment: opt(v, "comment")?,
            },
            Some("unset-token-when") => Op::UnsetTokenWhen {
                name: s(v, "name")?,
                when: conds(v)?,
            },
            Some("add-token") => Op::AddToken {
                name: s(v, "name")?,
                kind: opt(v, "kind")?,
                value: s(v, "value")?,
                // `key=value` words, as the source writes them.
                meta: list(v, "meta")?
                    .iter()
                    .map(|m| {
                        m.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| bad("each of `meta` is a `key=value` string".into()))
                    })
                    .collect::<Result<_, _>>()?,
                whens: list(v, "whens")?
                    .iter()
                    .map(|w| Ok((conds(w)?, s(w, "value")?, opt(w, "comment")?)))
                    .collect::<Result<Vec<_>, Value>>()?,
                comment: opt(v, "comment")?,
                before: opt(v, "before")?,
            },
            Some("rename-token") => Op::RenameToken {
                from: s(v, "from")?,
                to: s(v, "to")?,
            },
            Some("remove-token") => Op::RemoveToken {
                name: s(v, "name")?,
            },
            Some("add-axis-value") => Op::AddAxisValue {
                axis: s(v, "axis")?,
                value: s(v, "value")?,
                before: opt(v, "before")?,
            },
            Some("add-axis") => Op::AddAxis {
                name: s(v, "name")?,
                values: list(v, "values")?
                    .iter()
                    .map(|x| {
                        x.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| bad("each of `values` is a string".into()))
                    })
                    .collect::<Result<_, _>>()?,
                under_dark: match opt(v, "under")?.as_deref() {
                    None => false,
                    Some("scheme=dark") => true,
                    Some(other) => {
                        return Err(bad(format!("`under` is `scheme=dark`, not `{other}`")))
                    }
                },
            },
            Some("set-param") => Op::SetParam {
                name: s(v, "name")?,
                when: conds(v)?,
                value: s(v, "value")?,
            },
            Some("unset-param") => Op::UnsetParam {
                name: s(v, "name")?,
                when: conds(v)?,
            },
            Some("remove-axis-value") => Op::RemoveAxisValue {
                axis: s(v, "axis")?,
                value: s(v, "value")?,
            },
            other => return Err(bad(format!("no operation {}", other.unwrap_or("(none)")))),
        };
        out.push((file, op));
    }
    Ok(out)
}

/// The app's params as the design's folds take them (LLP 1121 D15).
pub(crate) fn params(
    list: &[contract_lower::params::ParamToken],
) -> Vec<exact_tamber::params::Param> {
    list.iter()
        .map(|p| exact_tamber::params::Param {
            token: p.token.clone(),
            full: p.full.clone(),
            kind: p.param.kind.clone(),
            default: p.param.default.clone(),
        })
        .collect()
}

/// Each `set-param` and `unset-param` by a name the app has: refused for a
/// name no param answers or an ambiguous short one; written on the line the
/// design already has for that param, by the name that line uses.
pub(crate) fn param_names(
    ops: &mut [(String, Op)],
    snap: &[(String, String, String)],
    app: &contract::tamber_params::AppParams,
) -> Result<(), Value> {
    let (list, identities) = (&app.tokens, &app.identities);
    for (file, op) in ops.iter_mut() {
        let name = match op {
            Op::SetParam { name, .. } | Op::UnsetParam { name, .. } => name,
            _ => continue,
        };
        let p = contract_lower::params::resolve(list, identities, name)
            .map_err(|e| json!({"message": format!("{}: {}", e.id, e.message), "id": e.id}))?;
        let design = snap
            .iter()
            .find(|(f, _, _)| f == file)
            .and_then(|(_, text, _)| exact_tamber::parse(text).ok());
        let line = design.as_ref().and_then(|d| {
            d.params
                .iter()
                .find(|e| {
                    contract_lower::params::resolve(list, identities, &e.name.value)
                        .is_ok_and(|q| q.token == p.token)
                })
                .map(|e| e.name.value.clone())
        });
        if let Some(line) = line {
            *name = line;
        }
    }
    Ok(())
}

/// Each of `files` (a design and its new text) folded with `list`, its
/// `params` lines in their params' places: refused where one does not fold,
/// or falls outside its param's range, each param named in full.
pub(crate) fn param_values(
    files: &[(String, String)],
    list: &[contract_lower::params::ParamToken],
    vocab: &exact_tamber::Vocab,
) -> Result<(), Value> {
    let folds = params(list);
    for (file, text) in files {
        let Ok(design) = exact_tamber::parse(text) else {
            continue;
        };
        let laid = exact_tamber::params::laid(&design, &folds);
        let message = |m: &str| contract_lower::params::shown(m, list);
        let refused = match exact_tamber::check_design(laid, vocab) {
            Err(all) => all
                .iter()
                .map(|d| json!({"file": file, "line": d.line, "col": d.col, "id": d.id, "message": message(&d.message)}))
                .collect::<Vec<_>>(),
            Ok(folding) => contract_lower::params::folded(list, &folding, &[])
                .iter()
                .map(|e| json!({"file": file, "id": e.id, "message": message(&e.message)}))
                .collect(),
        };
        if !refused.is_empty() {
            return Err(json!({"message": "refused", "diagnostics": refused}));
        }
    }
    Ok(())
}
