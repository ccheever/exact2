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

/// A state part's row: its style, state and row.
type Key = (String, String, String);

/// A literal as history compares it: by kind, so `0.5` is no `"0.5"`.
#[derive(Debug, Clone, PartialEq)]
enum Lit {
    Str(String),
    Num(f64),
    Bool(bool),
    Ident(String),
    /// Anything else, by its source text (a style's rows are literals,
    /// `contract-style-literal`, so this is a version that doesn't check).
    Other(String),
}

impl Lit {
    fn of(e: &contract_syntax::Expr, line: &str) -> Lit {
        use contract_syntax::Expr;
        match e {
            Expr::Str(s, _) => Lit::Str(s.clone()),
            Expr::Number(n, _) => Lit::Num(*n),
            Expr::Bool(b, _) => Lit::Bool(*b),
            Expr::Ident(i, _) => Lit::Ident(i.clone()),
            other => {
                let sp = other.span();
                Lit::Other(
                    line.get(sp.col as usize - 1..sp.end_col as usize - 1)
                        .unwrap_or("(an expression)")
                        .to_string(),
                )
            }
        }
    }

    /// As source text: a string quoted, as Contract writes it.
    fn text(&self) -> String {
        match self {
            Lit::Str(s) => crate::tamber_contract::string_literal(s),
            Lit::Num(n) => json!(n).to_string(),
            Lit::Bool(b) => b.to_string(),
            Lit::Ident(i) | Lit::Other(i) => i.clone(),
        }
    }

    /// As `set-state` writes it, a string or a number; `None` for the rest.
    fn value(&self) -> Option<Value> {
        match self {
            Lit::Str(s) => Some(json!(s)),
            Lit::Num(n) => Some(json!(n)),
            _ => None,
        }
    }
}

/// A Contract source read for history: its styles' state rows in order,
/// and the rest of its text (the app's code), or why not.
struct Read {
    rows: Vec<(Key, Lit)>,
    rest: String,
}

/// The rows the border's colour shares: their order is the design's too.
fn border(row: &str) -> bool {
    row.starts_with("border-") && row.ends_with("-color")
}

fn read(text: &Option<String>, which: &str) -> Result<Read, String> {
    let Some(text) = text else {
        return Ok(Read {
            rows: Vec::new(),
            rest: String::new(),
        });
    };
    let file = contract_syntax::parse(text)
        .map_err(|e| format!("{which} does not parse: {}", e.message))?;
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let mut rows = Vec::new();
    // A line's comment, which stays the code's.
    let comment = |l: &str| l.find("//").map_or(String::new(), |i| l[i..].to_string());
    for (i, style) in file.styles.iter().enumerate() {
        if file.styles[..i].iter().any(|s| s.name == style.name) {
            return Err(format!("{which} declares `style {}` twice", style.name));
        }
        let mut last = style.span.line as usize;
        for a in &style.attrs {
            let Some((state, row)) = contract_syntax::state::state_of(&a.name) else {
                continue;
            };
            let at = a.span.line as usize - 1;
            let line = lines.get(at).cloned().unwrap_or_default();
            rows.push((
                (style.name.clone(), state.to_string(), row.to_string()),
                Lit::of(&a.value, &line),
            ));
            // The row's lines, through a value's parentheses left open.
            let mut depth = 0i32;
            let mut end = at;
            for (j, l) in lines.iter().enumerate().skip(at) {
                let code = l.split("//").next().unwrap_or("");
                depth += code.matches('(').count() as i32 - code.matches(')').count() as i32;
                end = j;
                if depth <= 0 {
                    break;
                }
            }
            for l in &mut lines[at..=end] {
                *l = comment(l);
            }
            last = last.max(end + 1);
        }
        // Its state parts' headers, inside the style only.
        for l in &mut lines[style.span.line as usize..last.min(text.split('\n').count())] {
            let name = l
                .trim()
                .strip_prefix(':')
                .map(|r| r.split("//").next().unwrap_or("").trim());
            if name.is_some_and(|n| contract_syntax::state::STATES.contains(&n)) {
                *l = comment(l);
            }
        }
    }
    let rest = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Read { rows, rest })
}

/// One state edit: a row's value before and after, or a block's border
/// order (`row` "border-*-color order", the values its rows' names).
#[derive(Debug)]
struct StateEdit {
    key: Key,
    before: Option<Lit>,
    after: Option<Lit>,
    order: bool,
}

impl StateEdit {
    fn json(&self) -> Value {
        let text = |l: &Option<Lit>| l.as_ref().map_or(Value::Null, |l| json!(l.text()));
        json!({"what": "state", "name": self.key.0, "state": self.key.1, "row": self.key.2, "when": [], "before": text(&self.before), "after": text(&self.after)})
    }
}

fn state_edits(a: &[(Key, Lit)], b: &[(Key, Lit)]) -> Vec<StateEdit> {
    let find = |rows: &[(Key, Lit)], k: &Key| {
        rows.iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    let mut keys: Vec<&Key> = Vec::new();
    for (k, _) in a.iter().chain(b) {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    let mut out: Vec<StateEdit> = keys
        .iter()
        .filter_map(|k| {
            let (x, y) = (find(a, k), find(b, k));
            (x != y).then(|| StateEdit {
                key: (*k).clone(),
                before: x,
                after: y,
                order: false,
            })
        })
        .collect();
    // A block whose border rows both versions write, in another order.
    let order = |rows: &[(Key, Lit)], style: &str, state: &str, both: &dyn Fn(&str) -> bool| {
        rows.iter()
            .filter(|((s, st, r), _)| s == style && st == state && border(r) && both(r))
            .map(|((.., r), _)| r.clone())
            .collect::<Vec<_>>()
    };
    let mut blocks: Vec<(&str, &str)> = Vec::new();
    for ((s, st, r), _) in a {
        if border(r) && !blocks.contains(&(s.as_str(), st.as_str())) {
            blocks.push((s, st));
        }
    }
    for (s, st) in blocks {
        let in_b = |r: &str| {
            b.iter()
                .any(|((bs, bst, br), _)| bs == s && bst == st && br == r)
        };
        let in_a = |r: &str| {
            a.iter()
                .any(|((as_, ast, ar), _)| as_ == s && ast == st && ar == r)
        };
        let (x, y) = (order(a, s, st, &in_b), order(b, s, st, &in_a));
        if x != y {
            out.push(StateEdit {
                key: (s.to_string(), st.to_string(), "border-*-color order".into()),
                before: Some(Lit::Other(x.join(", "))),
                after: Some(Lit::Other(y.join(", "))),
                order: true,
            });
        }
    }
    out
}

/// A `.contract` pair compared: its state rows' edits, and `code` where
/// the commit changed its other lines too.
fn compare_contract(file: &str, before: &Option<String>, after: &Option<String>) -> Value {
    match (
        read(before, "the version before"),
        read(after, "the version after"),
    ) {
        (Ok(a), Ok(b)) => {
            let edits: Vec<Value> = state_edits(&a.rows, &b.rows)
                .iter()
                .map(StateEdit::json)
                .collect();
            let mut out = json!({"file": file, "edits": edits});
            if a.rest != b.rest {
                out["code"] = json!(true);
            }
            out
        }
        (Err(why), _) | (_, Err(why)) => json!({"file": file, "edits": null, "refused": why}),
    }
}

/// A `.contract` pair's inverse on the file as it is now (`current`,
/// `None` where the server does not own it now): onto `ops`, `already`
/// and `conflicts`, and its name onto `code` where it changed its code.
#[allow(clippy::too_many_arguments)]
fn revert_contract(
    file: &str,
    before: &Option<String>,
    after: &Option<String>,
    current: Option<&String>,
    ops: &mut Vec<Value>,
    already: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
    moved: &mut Vec<String>,
    code: &mut Vec<Value>,
) {
    let whole = |why: String| json!({"file": file, "edit": null, "now": null, "why": why});
    let (a, b) = match (
        read(before, "the version before"),
        read(after, "the version after"),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(why), _) | (_, Err(why)) => return conflicts.push(whole(why)),
    };
    let edits = state_edits(&a.rows, &b.rows);
    if a.rest != b.rest {
        code.push(json!(file));
    }
    // A file whose code alone changed is no design edit to undo.
    if edits.is_empty() {
        return;
    }
    if before.is_none() {
        return conflicts.push(whole(
            "the commit created the file; a revert leaves files where they are".into(),
        ));
    }
    if after.is_none() {
        return conflicts.push(whole(
            "the commit removed the file; no operation makes a Contract source".into(),
        ));
    }
    let Some(current) = current else {
        return conflicts.push(whole(
            "the file is not the app's own Contract source now".into(),
        ));
    };
    if Some(current) != after.as_ref() {
        moved.push(file.to_string());
    }
    let today = match read(&Some(current.clone()), "the file now") {
        Ok(t) => t,
        Err(why) => return conflicts.push(whole(why)),
    };
    let styles: Vec<String> = contract_syntax::parse(current)
        .map(|f| f.styles.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default();
    for edit in edits {
        let k = &edit.key;
        let e = edit.json();
        let now = today
            .rows
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone());
        let refuse =
            |why: &str, now: Value| json!({"file": file, "edit": e, "now": now, "why": why});
        if edit.order {
            conflicts.push(refuse(
                "an order of border rows is not what set-state writes: put them back by hand",
                Value::Null,
            ));
            continue;
        }
        let shown = now.as_ref().map_or(Value::Null, |l| json!(l.text()));
        if now == edit.before {
            already.push(json!({"file": file, "edit": e}));
        } else if !styles.contains(&k.0) {
            conflicts.push(refuse(
                &format!("no `style {}` in the file now", k.0),
                Value::Null,
            ));
        } else if now != edit.after {
            conflicts.push(refuse("the row has moved on since", shown));
        } else if let Some(was) = &edit.before {
            let Some(value) = was.value() else {
                conflicts.push(refuse(
                    "its value before is no string or number, which set-state writes",
                    shown,
                ));
                continue;
            };
            // Put back, a border row would land after another and change
            // which one paints a side.
            let others = today.rows.iter().any(|((s, st, r), _)| {
                *s == k.0 && *st == k.1 && *r != k.2 && border(r) && border(&k.2)
            });
            if now.is_none() && others {
                conflicts.push(refuse("put back last, it would paint over the block's other border rows: restore it by hand", shown));
                continue;
            }
            ops.push(json!({"op": "set-state", "style": k.0, "state": k.1, "row": k.2, "value": value, "file": file}));
        } else {
            ops.push(
                json!({"op": "unset-state", "style": k.0, "state": k.1, "row": k.2, "file": file}),
            );
        }
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
            revert_contract(
                &file,
                &before,
                &after,
                current,
                &mut ops,
                &mut already,
                &mut conflicts,
                &mut moved,
                &mut code,
            );
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
