//! The design server's readings of git history (LLP 1122 D9, T1), for the
//! wire's `history` and `revert` (`scripts/tamber-server.mjs`), which reads
//! git and sends each commit's versions of a file here as text:
//!
//! - `compare` `{pairs: [{file, before, after}]}`: each pair as token-level
//!   edits (`exact_tamber::history`); a version that is null did not exist,
//!   and one that does not parse gives `edits: null` and why.
//! - `revert-plan` `{pairs}`: the inverse of the commit those pairs are,
//!   planned on the files as they are now: the operations (each naming its
//!   file), the edits the files already have as before, the edits it cannot
//!   revert, and the files that moved since the commit.
//!
//! An app's own `.contract` file (LLP 1121 stage 8 D) reads as the rows of
//! its styles' state parts, the design's part of it: each edit
//! `{what: "state", name: style, state, row, before, after}`, and `code`
//! where the commit changed lines outside them too, which a revert leaves
//! alone (they are the app's code, not its design).

use crate::tamber_import::op_json;
use exact_tamber::history::{self, Edit};
use serde_json::{json, Value};

/// One pair: the file, and its text before and after the commit.
type Pair = (String, Option<String>, Option<String>);

fn pairs(request: &Value) -> Result<Vec<Pair>, Value> {
    let bad = || json!({"message": "`pairs` is a list of {file, before, after}, each version a string or null"});
    let text = |v: &Value| match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(bad()),
    };
    request["pairs"]
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|p| {
            Ok((
                p["file"].as_str().ok_or_else(bad)?.to_string(),
                text(&p["before"])?,
                text(&p["after"])?,
            ))
        })
        .collect()
}

pub fn edit_json(e: &Edit) -> Value {
    json!({
        "what": e.what.name(),
        "name": e.name,
        "when": e.when.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(),
        "before": e.before,
        "after": e.after,
    })
}

/// A version parsed, or why not.
fn parsed(text: &Option<String>, which: &str) -> Result<Option<exact_tamber::Design>, String> {
    match text {
        None => Ok(None),
        Some(t) => exact_tamber::parse(t).map(Some).map_err(|all| {
            format!(
                "{which} does not parse: {}",
                all.first().map_or(String::new(), ToString::to_string)
            )
        }),
    }
}

/// A state part's row: its style, state and row, and its value.
type StateRow = ((String, String, String), contract_syntax::Expr);

/// A Contract source's state-part rows, in order, and the file with them
/// taken out (what else it says), or why it does not parse.
fn state_rows(text: &Option<String>, which: &str) -> Result<(Vec<StateRow>, String), String> {
    let Some(text) = text else {
        return Ok((Vec::new(), String::new()));
    };
    let file = contract_syntax::parse(text)
        .map_err(|e| format!("{which} does not parse: {}", e.message))?;
    let mut rows = Vec::new();
    let mut lines: Vec<&str> = text.lines().collect();
    for style in &file.styles {
        for a in &style.attrs {
            if let Some((state, row)) = contract_syntax::state::state_of(&a.name) {
                rows.push((
                    (style.name.clone(), state.to_string(), row.to_string()),
                    a.value.clone(),
                ));
                if let Some(l) = lines.get_mut(a.span.line as usize - 1) {
                    *l = "";
                }
            }
        }
    }
    // The rest, without state headers and blank lines: a moved row is no change to it.
    let header = |l: &str| {
        l.strip_prefix(':').is_some_and(|r| {
            let name = r.split("//").next().unwrap_or("").trim();
            contract_syntax::state::STATES.contains(&name)
        })
    };
    let rest = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !header(l))
        .collect::<Vec<_>>()
        .join("\n");
    Ok((rows, rest))
}

/// A row's value as the wire writes it (`set-state`'s `value`): a string
/// or a number; `None` for anything else.
fn row_value(e: &contract_syntax::Expr) -> Option<Value> {
    match e {
        contract_syntax::Expr::Str(s, _) => Some(json!(s)),
        contract_syntax::Expr::Number(n, _) => Some(json!(n)),
        _ => None,
    }
}

/// A value as an edit shows it.
fn shown(e: Option<&contract_syntax::Expr>) -> Value {
    match e {
        None => Value::Null,
        Some(e) => match row_value(e) {
            Some(Value::String(s)) => json!(s),
            Some(v) => json!(v.to_string()),
            // A style's rows are literals (`contract-style-literal`).
            None => json!("(an expression)"),
        },
    }
}

/// The state rows a commit changed: each key, its value before and after.
type StateEdit<'a> = (
    &'a (String, String, String),
    Option<&'a contract_syntax::Expr>,
    Option<&'a contract_syntax::Expr>,
);

fn state_edits<'a>(a: &'a [StateRow], b: &'a [StateRow]) -> Vec<StateEdit<'a>> {
    let find = |rows: &'a [StateRow], k: &(String, String, String)| {
        rows.iter().find(|(key, _)| key == k).map(|(_, v)| v)
    };
    let mut keys: Vec<&(String, String, String)> = a.iter().chain(b).map(|(k, _)| k).collect();
    keys.dedup();
    let mut seen = Vec::new();
    keys.into_iter()
        .filter(|k| {
            let fresh = !seen.contains(k);
            seen.push(*k);
            fresh
        })
        .filter_map(|k| {
            let (x, y) = (find(a, k), find(b, k));
            (shown(x) != shown(y)).then_some((k, x, y))
        })
        .collect()
}

fn state_edit_json((k, x, y): &StateEdit) -> Value {
    json!({"what": "state", "name": k.0, "state": k.1, "row": k.2, "when": [], "before": shown(*x), "after": shown(*y)})
}

/// A `.contract` pair compared: its state rows' edits, and `code` where
/// the commit changed its other lines too.
fn compare_contract(file: &str, before: &Option<String>, after: &Option<String>) -> Value {
    match (
        state_rows(before, "the version before"),
        state_rows(after, "the version after"),
    ) {
        (Ok((a, ra)), Ok((b, rb))) => {
            let edits: Vec<Value> = state_edits(&a, &b).iter().map(state_edit_json).collect();
            let mut out = json!({"file": file, "edits": edits});
            if ra != rb {
                out["code"] = json!(true);
            }
            out
        }
        (Err(why), _) | (_, Err(why)) => json!({"file": file, "edits": null, "refused": why}),
    }
}

pub fn compare(request: &Value) -> Result<Value, Value> {
    let files: Vec<Value> = pairs(request)?
        .iter()
        .map(|(file, before, after)| {
            if file.ends_with(".contract") {
                return compare_contract(file, before, after);
            }
            match (parsed(before, "the version before"), parsed(after, "the version after")) {
                (Ok(a), Ok(b)) => match history::edits(a.as_ref(), b.as_ref()) {
                    Ok(all) => json!({"file": file, "edits": all.iter().map(edit_json).collect::<Vec<_>>()}),
                    Err(why) => json!({"file": file, "edits": null, "refused": why}),
                },
                (Err(why), _) | (_, Err(why)) => json!({"file": file, "edits": null, "refused": why}),
            }
        })
        .collect();
    Ok(json!({ "files": files }))
}

/// `now`: each design file as it is, its path, text and digest. The reply's
/// `files` are the digests it planned against, for the held proposal's base.
pub fn revert_plan(request: &Value, now: &[(String, String, String)]) -> Result<Value, Value> {
    let (mut ops, mut already, mut conflicts, mut moved) = (vec![], vec![], vec![], vec![]);
    let whole =
        |file: &str, why: String| json!({"file": file, "edit": null, "now": null, "why": why});
    let mut code = vec![];
    for (file, before, after) in pairs(request)? {
        let current = now.iter().find(|(f, _, _)| *f == file).map(|(_, t, _)| t);
        if file.ends_with(".contract") {
            let Some(current) = current else {
                conflicts.push(whole(
                    &file,
                    "the file is not the app's own Contract source now".into(),
                ));
                continue;
            };
            if Some(current) != after.as_ref() {
                moved.push(file.clone());
            }
            let read = (
                state_rows(&before, "the version before"),
                state_rows(&after, "the version after"),
                state_rows(&Some(current.clone()), "the file now"),
            );
            let ((a, ra), (b, rb), (today, _)) = match read {
                (Ok(a), Ok(b), Ok(t)) => (a, b, t),
                (Err(why), ..) | (_, Err(why), _) | (.., Err(why)) => {
                    conflicts.push(whole(&file, why));
                    continue;
                }
            };
            if ra != rb {
                code.push(json!(file));
            }
            let styles: Vec<String> = contract_syntax::parse(current)
                .map(|f| f.styles.iter().map(|s| s.name.clone()).collect())
                .unwrap_or_default();
            for edit in state_edits(&a, &b) {
                let (k, was, made) = edit;
                let now_value = today.iter().find(|(key, _)| key == k).map(|(_, v)| v);
                let e = state_edit_json(&edit);
                if shown(now_value) == shown(was) {
                    already.push(json!({"file": file, "edit": e}));
                } else if !styles.contains(&k.0) {
                    conflicts.push(json!({"file": file, "edit": e, "now": null, "why": format!("no `style {}` in the file now", k.0)}));
                } else if shown(now_value) != shown(made) {
                    conflicts.push(json!({"file": file, "edit": e, "now": shown(now_value), "why": "the row has moved on since"}));
                } else {
                    let op = match was {
                        None => {
                            json!({"op": "unset-state", "style": k.0, "state": k.1, "row": k.2, "file": file})
                        }
                        Some(v) => match row_value(v) {
                            Some(value) => {
                                json!({"op": "set-state", "style": k.0, "state": k.1, "row": k.2, "value": value, "file": file})
                            }
                            None => {
                                conflicts.push(json!({"file": file, "edit": e, "now": shown(now_value), "why": "its value before is no string or number, which `set-state` writes"}));
                                continue;
                            }
                        },
                    };
                    ops.push(op);
                }
            }
            continue;
        }
        let (old, new) = match (
            parsed(&before, "the version before"),
            parsed(&after, "the version after"),
        ) {
            (Ok(Some(a)), Ok(Some(b))) => (a, b),
            (Ok(None), _) => {
                conflicts.push(whole(
                    &file,
                    "the commit created the file; no operation removes a design file".into(),
                ));
                continue;
            }
            (_, Ok(None)) => {
                conflicts.push(whole(
                    &file,
                    "the commit removed the file; no operation makes a design file".into(),
                ));
                continue;
            }
            (Err(why), _) | (_, Err(why)) => {
                conflicts.push(whole(&file, why));
                continue;
            }
        };
        let Some(current) = current else {
            conflicts.push(whole(&file, "the file is not in the design now".into()));
            continue;
        };
        if Some(current) != after.as_ref() {
            moved.push(file.clone());
        }
        let today = match exact_tamber::parse(current) {
            Ok(d) => d,
            Err(all) => {
                conflicts.push(whole(
                    &file,
                    format!(
                        "the file does not parse now: {}",
                        all.first().map_or(String::new(), ToString::to_string)
                    ),
                ));
                continue;
            }
        };
        let r = history::revert(&old, &new, &today);
        for op in &r.ops {
            let mut v = op_json(op);
            v["file"] = json!(file);
            ops.push(v);
        }
        for e in &r.already {
            already.push(json!({"file": file, "edit": edit_json(e)}));
        }
        for c in &r.conflicts {
            conflicts.push(
                json!({"file": file, "edit": edit_json(&c.edit), "now": c.now, "why": c.why}),
            );
        }
    }
    let files: serde_json::Map<String, Value> =
        now.iter().map(|(f, _, d)| (f.clone(), json!(d))).collect();
    Ok(
        json!({"ops": ops, "already": already, "conflicts": conflicts, "moved": moved, "code": code, "files": files}),
    )
}
