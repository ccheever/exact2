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
pub(crate) fn params(app: &contract::tamber_params::AppParams) -> exact_tamber::params::Params {
    contract_lower::params::folds(&app.tokens, &app.identities)
}

/// Each `set-param` and `unset-param` by a name the app has: refused for a
/// name no param answers or an ambiguous short one; written on the line the
/// design already has for that param, by the name that line uses, or by
/// the name an earlier op of this transaction wrote it by, so one
/// transaction never gives a param two lines.
pub(crate) fn param_names(
    ops: &mut [(String, Op)],
    snap: &[(String, String, String)],
    app: &contract::tamber_params::AppParams,
) -> Result<(), Value> {
    let mut written: Vec<(String, String, String)> = Vec::new();
    for (file, op) in ops.iter_mut() {
        // A line written as the design has it may always be removed: the
        // repair of one the build refuses (a short name grown ambiguous).
        if let Op::UnsetParam { name, .. } = &*op {
            let written = snap
                .iter()
                .find(|(f, _, _)| f == file)
                .and_then(|(_, text, _)| exact_tamber::parse(text).ok())
                .is_some_and(|d| d.params.iter().any(|e| e.name.value == *name));
            if written
                && contract_lower::params::resolve(&app.tokens, &app.identities, name).is_err()
            {
                continue;
            }
        }
        let name = match op {
            Op::SetParam { name, .. } | Op::UnsetParam { name, .. } => name,
            _ => continue,
        };
        let resolve = |n: &str| contract_lower::params::resolve(&app.tokens, &app.identities, n);
        let p = resolve(name)
            .map_err(|e| json!({"message": format!("{}: {}", e.id, e.message), "id": e.id}))?;
        let design = snap
            .iter()
            .find(|(f, _, _)| f == file)
            .and_then(|(_, text, _)| exact_tamber::parse(text).ok());
        let line = design.as_ref().and_then(|d| {
            d.params
                .iter()
                .find(|e| resolve(&e.name.value).is_ok_and(|q| q.token == p.token))
                .map(|e| e.name.value.clone())
        });
        let earlier = written
            .iter()
            .find(|(f, t, _)| f == file && *t == p.token)
            .map(|(_, _, n)| n.clone());
        let spelled = line.or(earlier).unwrap_or_else(|| name.clone());
        written.push((file.clone(), p.token.clone(), spelled.clone()));
        *name = spelled;
    }
    Ok(())
}

/// Each of `files` (a design and its new text) folded with the app's
/// params, its `params` lines in their params' places: refused where one
/// does not fold, or a value falls outside its param's range, each at its
/// line (a param's own at its declaration) and named in full.
pub(crate) fn param_values(
    files: &[(String, String)],
    app: &contract::tamber_params::AppParams,
    vocab: &exact_tamber::Vocab,
    dir: &std::path::Path,
) -> Result<(), Value> {
    let list = &app.tokens;
    for (file, text) in files {
        let Ok(design) = exact_tamber::parse(text) else {
            continue;
        };
        let message = |m: &str| contract_lower::params::shown(m, list);
        let at = |line: u32, col: u32, id: &str, m: &str| json!({"file": file, "line": line, "col": col, "id": id, "message": message(m)});
        // A line the build refuses (ambiguous, or a second for one param),
        // then each value written out of its range.
        let mut refused: Vec<Value> =
            contract_lower::params::entry_errors(&design, list, &app.identities)
                .iter()
                .chain(&contract_lower::params::written_ranges(
                    &design,
                    list,
                    &app.identities,
                    vocab,
                ))
                .map(|(line, col, e)| at(*line, *col, e.id, &e.message))
                .collect();
        let laid = contract::tamber_params::laid(&design, app);
        match exact_tamber::check_design(laid, vocab) {
            Err(all) => refused.extend(all.iter().map(|d| {
                let mut d = d.clone();
                match contract::tamber_params::relocate(&mut d, app) {
                    Some(declaring) => json!({"file": declaring, "line": d.line, "col": d.col, "id": d.id, "message": d.message}),
                    None => at(d.line, d.col, d.id, &d.message),
                }
            })),
            Ok(folding) if refused.is_empty() => {
                // An icon param's SVG is the app's file (D10), as the build
                // reads it.
                for i in folding.icons.iter().filter(|i| i.name.starts_with(contract_lower::params::PREFIX)) {
                    if let Some(svg) = i.svg.as_ref().filter(|f| !dir.join("assets").join(f).is_file()) {
                        refused.push(json!({"file": file, "id": "tamber-icon-svg", "message": message(&format!("`{}`: `assets/{svg}` is no file of the app", i.name))}));
                    }
                }
                // A value set charged to its line, a default to its own.
                let overridden: Vec<(String, contract_syntax::Span)> = design
                    .params
                    .iter()
                    .filter_map(|e| {
                        let p = contract_lower::params::resolve(list, &app.identities, &e.name.value).ok()?;
                        Some((p.token.clone(), contract_syntax::Span { line: e.name.line, col: e.name.col, end_col: e.name.col, source_id: 0 }))
                    })
                    .collect();
                refused.extend(
                    contract_lower::params::folded(list, &folding, &overridden, None)
                        .iter()
                        .map(|e| match overridden.iter().any(|(_, s)| *s == e.span) {
                            true => at(e.span.line, e.span.col, e.id, &e.message),
                            false => match list.iter().find(|t| t.param.default_span == e.span) {
                                Some(t) => json!({"file": t.identity.rsplit_once('#').map_or("", |(f, _)| f), "line": e.span.line, "col": e.span.col, "id": e.id, "message": message(&e.message)}),
                                None => json!({"file": file, "id": e.id, "message": message(&e.message)}),
                            },
                        }),
                );
            }
            Ok(_) => {}
        }
        if !refused.is_empty() {
            return Err(json!({"message": "refused", "diagnostics": refused}));
        }
    }
    Ok(())
}
