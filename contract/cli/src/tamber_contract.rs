//! The design server's `.contract` files (LLP 1121 stage 8 D, D13): the
//! app's own Contract sources join the design's files (packages stay
//! read-only), and `set-state style state row value` and `unset-state style
//! state row` edit a row of a `style`'s state part (D8). A written file goes
//! through `contract fmt`; the whole app is compiled with it, in memory,
//! before anything is written; its inverse is the preimage, restored line
//! for line and byte for byte, bypassing fmt (LLP 1122 D6).

use crate::tamber_ops::{optional, required};
use exact_tamber::Op;
use serde_json::{json, Value};
use std::path::Path;

/// A write to a `.contract` file.
#[derive(Debug, Clone)]
pub(crate) enum ContractOp {
    /// `row = value` in `style`'s `:state` block, made if it is not there.
    SetState {
        style: String,
        state: String,
        row: String,
        /// The value as Contract writes it: a string literal, or a number.
        value: String,
    },
    /// The row taken out, and its block when it empties.
    UnsetState {
        style: String,
        state: String,
        row: String,
    },
}

impl ContractOp {
    fn style(&self) -> &str {
        match self {
            ContractOp::SetState { style, .. } | ContractOp::UnsetState { style, .. } => style,
        }
    }
}

/// The app's own Contract sources, by path from its directory: what the
/// server may write. A package's file is its package's.
pub(crate) fn owned(dir: &Path) -> Vec<String> {
    let graph = contract::source_graph(&dir.join("app.contract"));
    let mut out: Vec<String> = graph
        .sources
        .iter()
        .filter(|s| matches!(s.origin, contract::Origin::App))
        .filter(|s| s.path.extension().is_some_and(|e| e == "contract"))
        .filter_map(|s| s.path.strip_prefix(dir).ok()?.to_str().map(str::to_string))
        .filter(|p| !p.starts_with(".exact") && !p.split('/').any(|c| c == "node_modules"))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// `ops` without its `.contract` writes, which are returned apart, each
/// with the file it names, if it names one.
pub(crate) fn split(ops: &Value) -> Result<(Value, Vec<(Option<String>, ContractOp)>), Value> {
    let Some(all) = ops.as_array() else {
        return Ok((ops.clone(), Vec::new()));
    };
    let mut rest = Vec::new();
    let mut out = Vec::new();
    for v in all {
        let op = match v["op"].as_str() {
            Some("set-state") => ContractOp::SetState {
                style: required(v, "style")?,
                state: state(v)?,
                row: required(v, "row")?,
                value: match &v["value"] {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => serde_json::to_string(s).expect("a string is JSON"),
                    _ => return Err(json!({"message": "`value` is a string or a number"})),
                },
            },
            Some("unset-state") => ContractOp::UnsetState {
                style: required(v, "style")?,
                state: state(v)?,
                row: required(v, "row")?,
            },
            _ => {
                rest.push(v.clone());
                continue;
            }
        };
        out.push((optional(v, "file")?, op));
    }
    Ok((Value::Array(rest), out))
}

/// A request's `state`, one of D8's, with or without its `:`.
fn state(v: &Value) -> Result<String, Value> {
    let s = required(v, "state")?;
    let s = s.strip_prefix(':').unwrap_or(&s).to_string();
    match contract_syntax::state::STATES.contains(&s.as_str()) {
        true => Ok(s),
        false => Err(
            json!({"message": format!("`:{s}` is not a state; a state part is one of {}", contract_syntax::state::STATES.join(", "))}),
        ),
    }
}

/// A text's lines, each without its `\n`, and whether it ended with one.
fn lines(text: &str) -> (Vec<String>, bool) {
    let newline = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);
    (body.split('\n').map(str::to_string).collect(), newline)
}

fn joined(lines: &[String], newline: bool) -> String {
    let mut out = lines.join("\n");
    if newline {
        out.push('\n');
    }
    out
}

/// The restore that takes `after` back to `before`, line for line.
pub(crate) fn inverse(before: &str, after: &str) -> Op {
    let ((a, newline), (b, _)) = (lines(before), lines(after));
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    Op::Restore {
        line: prefix as u32 + 1,
        remove: (b.len() - prefix - suffix) as u32,
        lines: a[prefix..a.len() - suffix].to_vec(),
        newline,
    }
}

/// A restore applied to `text` as it is: no format, no check.
pub(crate) fn restore(text: &str, op: &Op) -> Result<String, String> {
    let Op::Restore {
        line,
        remove,
        lines: put,
        newline,
    } = op
    else {
        return Err("an inverse is the restores an edit returned".into());
    };
    let (mut all, _) = lines(text);
    let start = (*line as usize).saturating_sub(1);
    let end = start + *remove as usize;
    if *line == 0 || end > all.len() {
        return Err("a restore names lines the file does not have".into());
    }
    all.splice(start..end, put.iter().cloned());
    Ok(joined(&all, *newline))
}

/// The indent of `line`, in spaces.
fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// `op` applied to `text`, the file that declares its style, then formatted.
pub(crate) fn edit(text: &str, op: &ContractOp) -> Result<String, String> {
    let file = contract_syntax::parse(text).map_err(|e| e.message)?;
    let style = file
        .styles
        .iter()
        .find(|s| s.name == op.style())
        .ok_or_else(|| format!("no `style {}` in this file", op.style()))?;
    let (mut all, newline) = lines(text);
    let (state, row) = match op {
        ContractOp::SetState { state, row, .. } | ContractOp::UnsetState { state, row, .. } => {
            (state, row)
        }
    };
    // A row is one property's name: no `=`, space or line of its own.
    if row.is_empty() || !row.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("`{row}` is no row's name"));
    }
    let name = contract_syntax::state::state_attr(&format!(":{state}"), row);
    let prefix = format!(":{state} ");
    let found = style.attrs.iter().find(|a| a.name == name);
    let block: Vec<&contract_syntax::Attr> = style
        .attrs
        .iter()
        .filter(|a| a.name.starts_with(&prefix))
        .collect();
    let at = |line: u32| line as usize - 1;
    match op {
        ContractOp::SetState { value, .. } => match found {
            // Its value replaced where it is written.
            Some(a) => {
                let v = a.value.span();
                let l = &mut all[at(v.line)];
                l.replace_range(v.col as usize - 1..v.end_col as usize - 1, value);
            }
            // A row added after its block's last.
            None if !block.is_empty() => {
                let last = block.iter().map(|a| a.span.line).max().expect("a row");
                let pad = " ".repeat(indent(&all[at(last)]));
                all.insert(at(last) + 1, format!("{pad}{row} = {value}"));
            }
            // The block added after the style's last row, at the style's
            // own indent (its first row's, or its header's and two), its
            // row as deep below it as another state's are.
            None => {
                let header = style.span.line;
                let last = style
                    .attrs
                    .iter()
                    .map(|a| a.span.line)
                    .max()
                    .unwrap_or(header);
                let body = (at(header) + 1..at(last) + 1)
                    .map(|i| &all[i])
                    .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
                    .map_or(indent(&all[at(header)]) + 2, |l| indent(l));
                let deeper = style
                    .attrs
                    .iter()
                    .find(|a| contract_syntax::state::state_of(&a.name).is_some())
                    .map_or(2, |a| {
                        indent(&all[at(a.span.line)]).saturating_sub(body).max(1)
                    });
                let (pad, row_pad) = (" ".repeat(body), " ".repeat(body + deeper));
                all.insert(at(last) + 1, format!("{row_pad}{row} = {value}"));
                all.insert(at(last) + 1, format!("{pad}:{state}"));
            }
        },
        ContractOp::UnsetState { .. } => {
            let a = found
                .ok_or_else(|| format!("`style {}` has no `{row}` in `:{state}`", op.style()))?;
            let (line, v) = (a.span.line, a.value.span());
            // Alone on its line, a comment after it or none.
            let l = &all[at(line)];
            let after = l.get(v.end_col as usize - 1..).unwrap_or("").trim();
            let alone = l.trim_start().len() == l.len() - (a.span.col as usize - 1)
                && (after.is_empty() || after.starts_with("//"));
            match alone {
                true => {
                    all.remove(at(line));
                    // Its block's header goes with its last row.
                    if block.len() == 1 {
                        let header = (0..at(line))
                            .rev()
                            .find(|&i| {
                                let rest = all[i].trim().strip_prefix(&format!(":{state}"));
                                rest.is_some_and(|r| {
                                    r.trim().is_empty() || r.trim().starts_with("//")
                                })
                            })
                            .ok_or("the state's block has no header")?;
                        all.remove(header);
                    }
                }
                false => {
                    let l = &mut all[at(line)];
                    let end = l[v.end_col as usize - 1..]
                        .find(|c: char| c != ' ')
                        .map_or(l.len(), |n| v.end_col as usize - 1 + n);
                    l.replace_range(a.span.col as usize - 1..end, "");
                }
            }
        }
    }
    contract_syntax::fmt::format(&joined(&all, newline)).map_err(|e| e.message)
}

/// The file `op` edits: the one it names, or the owned file that declares
/// its style; refused where none does, or two do.
pub(crate) fn file_of(
    named: Option<&str>,
    op: &ContractOp,
    texts: &[(String, String)],
) -> Result<String, Value> {
    if let Some(f) = named {
        return Ok(f.to_string());
    }
    let declares: Vec<&String> = texts
        .iter()
        .filter(|(p, t)| {
            p.ends_with(".contract")
                && contract_syntax::parse(t)
                    .is_ok_and(|f| f.styles.iter().any(|s| s.name == op.style()))
        })
        .map(|(p, _)| p)
        .collect();
    match declares.as_slice() {
        [one] => Ok((*one).clone()),
        [] => {
            Err(json!({"message": format!("no file of the app declares `style {}`", op.style())}))
        }
        many => Err(
            json!({"message": format!("`style {}` is declared in {}: name the `file`", op.style(), many.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(" and "))}),
        ),
    }
}

/// Plan the `.contract` writes (`ops`, each with its file) or an inverse's
/// restores of one over `snap`, beside the design's `planned`: each file's
/// new text and its inverse, a rebuild. A write is compiled whole first, in
/// memory, with the design `planned` writes; a refusal writes nothing. An
/// undo restores what was, and is not judged.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan(
    dir: &Path,
    design: &str,
    snap: &[(String, String, String)],
    ops: &[(String, ContractOp)],
    restores: &[(String, Op)],
    undo: bool,
    planned: &mut exact_tamber::Planned,
) -> Result<(), Value> {
    let refuse = |file: &str, m: String| json!({"message": "refused", "diagnostics": [{"file": file, "id": "tamber-contract-edit", "message": m, "line": 1, "col": 1}]});
    let mut written: Vec<(String, String)> = Vec::new();
    for (path, before, _) in snap.iter().filter(|(p, ..)| p.ends_with(".contract")) {
        let mut after = before.clone();
        for (_, op) in ops.iter().filter(|(p, _)| p == path) {
            after = edit(&after, op).map_err(|m| refuse(path, m))?;
        }
        for (_, op) in restores.iter().filter(|(p, _)| p == path) {
            after = restore(&after, op).map_err(|m| refuse(path, m))?;
        }
        if after != *before {
            written.push((path.clone(), after));
        }
    }
    for (path, _) in ops {
        if !snap.iter().any(|(f, ..)| f == path) {
            return Err(refuse(path, format!("`{path}` is no file of the app")));
        }
    }
    if written.is_empty() {
        return Ok(());
    }
    if !undo {
        let overlays = written
            .iter()
            .map(|(p, t)| (dir.join(p), t.clone()))
            .collect();
        let root = dir.join("app.contract");
        let design_text = planned
            .files
            .iter()
            .find(|(p, ..)| p == design)
            .map(|(_, t, _)| t.clone());
        let built = contract::with_contract_texts(overlays, || match design_text {
            Some(text) => contract::with_design_text(&dir.join(design), text, || {
                contract::compile_path_all(&root, false)
            }),
            None => contract::compile_path_all(&root, false),
        });
        if let Err(all) = built {
            let shown = |f: &Option<Box<Path>>| {
                f.as_deref()
                    .and_then(|f| f.strip_prefix(dir).ok())
                    .map_or_else(String::new, |p| p.to_string_lossy().into_owned())
            };
            return Err(
                json!({"message": "refused", "diagnostics": all.iter().map(|e| json!({
                "file": shown(&e.file),
                "id": e.id,
                "message": e.message,
                "line": e.span.line,
                "col": e.span.col,
            })).collect::<Vec<_>>()}),
            );
        }
    }
    for (path, after) in written {
        let before = &snap
            .iter()
            .find(|(f, ..)| *f == path)
            .expect("a file of the snapshot")
            .1;
        let back = inverse(before, &after);
        if let Op::Restore {
            line,
            remove,
            lines: put,
            ..
        } = &back
        {
            planned.diff.push_str(&format!("--- {path}\n+++ {path}\n"));
            let (now, _) = lines(&after);
            let start = *line as usize - 1;
            for l in put {
                planned.diff.push_str(&format!("-{l}\n"));
            }
            for l in &now[start..start + *remove as usize] {
                planned.diff.push_str(&format!("+{l}\n"));
            }
        }
        planned.files.push((path, after, vec![back]));
    }
    // A state part compiles into rows and looks: the ordinary rebuild (D13).
    planned.class = exact_tamber::Class::Rebuild;
    Ok(())
}

/// Each write's file over the server's read `snap`: the one it names, or
/// the one that declares its style; refused where that is no Contract
/// source of the app's own.
pub(crate) fn files(
    written: Vec<(Option<String>, ContractOp)>,
    snap: &[(String, String, String)],
) -> Result<Vec<(String, ContractOp)>, Value> {
    let texts: Vec<(String, String)> = snap
        .iter()
        .map(|(f, t, _)| (f.clone(), t.clone()))
        .collect();
    let mut out = Vec::new();
    for (named, op) in written {
        let file = file_of(named.as_deref(), &op, &texts)?;
        if !file.ends_with(".contract") || !texts.iter().any(|(f, _)| *f == file) {
            return Err(
                json!({"message": format!("`{file}` is no Contract source of the app's own")}),
            );
        }
        out.push((file, op));
    }
    Ok(out)
}
