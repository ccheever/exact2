//! The wire's operations (`tamber/1`, LLP 1121 D13): each `ops` entry read
//! into the crate's [`Op`], with the file it edits.

use exact_tamber::Op;
use serde_json::{json, Value};
use std::collections::HashMap;

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
                respell: None,
            },
            Some("unset-param") => Op::UnsetParam {
                name: s(v, "name")?,
                when: conds(v)?,
                respell: None,
            },
            Some("remove-axis-value") => Op::RemoveAxisValue {
                axis: s(v, "axis")?,
                value: s(v, "value")?,
            },
            other => {
                let what = other.unwrap_or("(none)");
                return Err(bad(exact_tamber::bounded_words(format!(
                    "no operation {what}"
                ))));
            }
        };
        out.push((file, op));
    }
    Ok(out)
}

/// The app's params as the design's folds take them (LLP 1121 D15).
pub(crate) fn params(app: &contract::tamber_params::AppParams) -> exact_tamber::params::Params {
    // And its packages' tokens (LLP 1121 D11), which every plan folds beside the design.
    let (joined, joined_icons) = contract::tamber_params::joined_all(app);
    exact_tamber::params::Params {
        joined,
        joined_icons,
        ..contract_lower::params::folds(&app.tokens, &app.identities)
    }
}

/// Each `set-param` and `unset-param` by a name the app has: refused for a
/// name no param answers or an ambiguous short one; written on the line the
/// design already has for that param, by the name that line uses, or by
/// the name an earlier op of this transaction wrote it by, so one
/// transaction never gives a param two lines; an exported library
/// component's param set by its full name, its line respelled if it is
/// short (LLP 1139 D8).
pub(crate) fn param_names(
    ops: &mut [(String, Op)],
    snap: &[(String, String, String)],
    app: &contract::tamber_params::AppParams,
) -> Result<(), Value> {
    // Each file parsed once, its params' lines by token, the names resolved
    // by one index, and the spellings written by token: none of it a scan
    // per operation (the checker's work in param counts).
    let resolver = contract_lower::params::Resolver::new(&app.tokens, &app.identities);
    let parsed: std::collections::HashMap<&str, exact_tamber::Design> = snap
        .iter()
        .filter_map(|(f, text, _)| Some((f.as_str(), exact_tamber::parse(text).ok()?)))
        .collect();
    let lines: std::collections::HashMap<&str, std::collections::HashMap<&str, String>> = parsed
        .iter()
        .map(|(f, d)| {
            let by_token = resolver
                .lines(d)
                .into_iter()
                .map(|(t, e)| (t, e.name.value.clone()))
                .collect();
            (*f, by_token)
        })
        .collect();
    let mut written: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    for (file, op) in ops.iter_mut() {
        // A line written as the design has it may always be removed: the
        // repair of one the build refuses (a short name grown ambiguous).
        if let Op::UnsetParam { name, .. } = &*op {
            let written = parsed
                .get(file.as_str())
                .is_some_and(|d| d.params.iter().any(|e| e.name.value == *name));
            if written && resolver.resolve(name).is_err() {
                continue;
            }
        }
        let (name, respell) = match op {
            Op::SetParam { name, respell, .. } => (name, Some(respell)),
            // A `when` line's removal keeps the param's line, respelled;
            // the whole param's removes it as it is spelled.
            Op::UnsetParam {
                name,
                when,
                respell,
            } => (name, (!when.is_empty()).then_some(respell)),
            _ => continue,
        };
        // Bounded before anything formats it, as `edit::valid` would.
        if let Some((id, why)) = exact_tamber::over_name(name) {
            return Err(json!({"message": format!("{id}: {why}"), "id": id}));
        }
        let p = resolver.resolve(name).map_err(|e| {
            let message = exact_tamber::bounded_words(format!("{}: {}", e.id, e.message));
            json!({"message": message, "id": e.id})
        })?;
        let line = lines
            .get(file.as_str())
            .and_then(|by_token| by_token.get(p.token.as_str()).cloned());
        // An earlier op's spelling first: it may have respelled the line.
        let earlier = written.get(&(file.clone(), p.token.clone())).cloned();
        // An exported library component's param is written by its full
        // name whatever name it was asked by, a new line and a set of a
        // line spelled short alike: a short one is
        // `tamber-param-short-library`, which the build refuses (LLP 1139
        // D8). An unset removes the line as it is spelled.
        let library = app
            .library
            .iter()
            .any(|(identity, _, d)| *identity == p.identity && d.library);
        let mut spelled = earlier.or(line).unwrap_or_else(|| name.clone());
        if let (true, Some(respell)) = (library, respell) {
            if spelled != p.full {
                *respell = Some(spelled);
                spelled = p.full.clone();
            }
        }
        written.insert((file.clone(), p.token.clone()), spelled.clone());
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
        let message = |m: &str| app.shown(m);
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
                // An icon param's SVG is a file beside the design it is
                // written in (D10; a package's default's, its package's, stage 13), as the build reads it.
                for i in folding.icons.iter().filter(|i| i.name.starts_with(contract_lower::params::PREFIX)) {
                    let home = contract_lower::icons::Home::of(i, Some(dir), &app.packages, list);
                    let whose = home.package.map_or_else(|| "the app".to_string(), |p| format!("the package `{p}`"));
                    if let Some(svg) = i.svg.as_ref() {
                        // A package's confined to its own `assets/` (round 1, Astra 4).
                        let why = match home.svg(svg) {
                            Some(p) if home.leaves(&p) => Some("leaves"),
                            Some(p) if p.is_file() => None,
                            _ => Some("is no file of"),
                        };
                        if let Some(why) = why {
                            refused.push(json!({"file": file, "id": "tamber-icon-svg", "message": message(&format!("`{}`: `assets/{svg}` {why} {whose}", i.name))}));
                        }
                    }
                }
                // A value set charged to its line, a default to its own.
                let resolver = contract_lower::params::Resolver::new(list, &app.identities);
                let overridden: Vec<(String, contract_syntax::Span)> = design
                    .params
                    .iter()
                    .filter_map(|e| {
                        let p = resolver.resolve(&e.name.value).ok()?;
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

/// A string field when the request has it; a field of another type is an
/// error, not absent.
pub(crate) fn optional(v: &Value, key: &str) -> Result<Option<String>, Value> {
    match &v[key] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(json!({"message": format!("`{key}` is a string")})),
    }
}

pub(crate) fn required(v: &Value, key: &str) -> Result<String, Value> {
    optional(v, key)?.ok_or_else(|| json!({"message": format!("`{key}` is a string, and needed")}))
}

/// A request's `files` (or `requires`), each path's digest as the client saw it.
pub(crate) fn seen_files(request: &Value, key: &str) -> Result<HashMap<String, String>, Value> {
    let bad = || json!({"message": format!("`{key}` is an object of digests")});
    match &request[key] {
        Value::Null => Ok(HashMap::new()),
        Value::Object(m) => m
            .iter()
            .map(|(k, v)| Ok((k.clone(), v.as_str().ok_or_else(bad)?.to_string())))
            .collect(),
        _ => Err(bad()),
    }
}

/// `{path: digest}`.
pub(crate) fn files_json(files: &[(String, String, String)]) -> Value {
    Value::Object(
        files
            .iter()
            .map(|(p, _, d)| (p.clone(), json!(d)))
            .collect(),
    )
}
