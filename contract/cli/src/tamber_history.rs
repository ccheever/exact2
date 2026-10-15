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

pub fn compare(request: &Value) -> Result<Value, Value> {
    let files: Vec<Value> = pairs(request)?
        .iter()
        .map(|(file, before, after)| {
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
    for (file, before, after) in pairs(request)? {
        let current = now.iter().find(|(f, _, _)| *f == file).map(|(_, t, _)| t);
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
        json!({"ops": ops, "already": already, "conflicts": conflicts, "moved": moved, "files": files}),
    )
}
