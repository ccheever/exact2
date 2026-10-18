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
use std::path::{Path, PathBuf};

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
/// server may write, and whether the graph loaded whole. A package's file
/// is its package's: one in a directory, below the app's, whose
/// `package.json` names a package (installed, linked or vendored, as a
/// component's identity reads it, LLP 1121 D15), or one a `node_modules`
/// entry links to, however a `use` reached it. `graph` is the app's
/// (`contract::source_graph` of its `app.contract`).
pub(crate) fn owned(dir: &Path, graph: &contract::SourceGraph) -> (Vec<String>, bool) {
    let app = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let mut packages: Vec<PathBuf> = graph.packages.iter().map(|p| p.root.clone()).collect();
    let installed = |d: &Path| std::fs::read_dir(d).into_iter().flatten().flatten();
    for entry in installed(&dir.join("node_modules")) {
        let path = entry.path();
        match entry.file_name().to_string_lossy().starts_with('@') {
            true => packages.extend(installed(&path).filter_map(|e| e.path().canonicalize().ok())),
            false => packages.extend(path.canonicalize().ok()),
        }
    }
    // A link back to the app or above it (a workspace's own) is no package.
    packages.retain(|root| !app.starts_with(root));
    let named = |d: &Path| {
        std::fs::read_to_string(d.join("package.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .is_some_and(|m| m["name"].is_string())
    };
    let in_package = |path: &Path| {
        packages.iter().any(|root| path.starts_with(root))
            || path
                .ancestors()
                .skip(1)
                .take_while(|d| d.starts_with(&app) && *d != app)
                .any(named)
    };
    let mut out: Vec<String> = graph
        .sources
        .iter()
        .filter(|s| matches!(s.origin, contract::Origin::App))
        .filter(|s| s.path.extension().is_some_and(|e| e == "contract"))
        .filter(|s| !in_package(&s.path))
        .filter_map(|s| s.path.strip_prefix(&app).ok()?.to_str().map(str::to_string))
        .filter(|p| !p.starts_with(".exact") && !p.split('/').any(|c| c == "node_modules"))
        .collect();
    out.sort();
    out.dedup();
    (out, graph.errors.is_empty())
}

/// [`owned`], and where the graph stopped loading (a syntax error) the
/// files the last whole one listed while they are there, so their pending
/// writes stay the session's; `last` is that list.
pub(crate) fn owned_kept(
    dir: &Path,
    (mut out, whole): (Vec<String>, bool),
    last: &std::cell::RefCell<Vec<String>>,
) -> Vec<String> {
    if whole {
        *last.borrow_mut() = out.clone();
        return out;
    }
    out.extend(
        last.borrow()
            .iter()
            .filter(|f| dir.join(f).is_file())
            .cloned(),
    );
    out.sort();
    out.dedup();
    out
}

/// A `.contract` write, with the file it names, if it names one.
pub(crate) type Named = (Option<String>, ContractOp);

/// `ops` without its `.contract` writes, which are returned apart, each
/// with the file it names, if it names one.
pub(crate) fn split(ops: &Value) -> Result<(Value, Vec<Named>), Value> {
    let Some(all) = ops.as_array() else {
        return Ok((ops.clone(), Vec::new()));
    };
    let mut rest = Vec::new();
    let mut out = Vec::new();
    for v in all {
        let state_op = matches!(v["op"].as_str(), Some("set-state" | "unset-state"));
        if state_op && v.get("node").is_some() {
            return Err(
                json!({"message": "a state part written under a node is `set-row`'s (LLP 1122 T5); `set-state` and `unset-state` edit a `style`'s"}),
            );
        }
        let op = match v["op"].as_str() {
            Some("set-state") => ContractOp::SetState {
                style: required(v, "style")?,
                state: state(v)?,
                row: required(v, "row")?,
                value: match &v["value"] {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => string_literal(s),
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

/// `s` as a Contract string literal, in the escapes its lexer takes
/// (`\n`, `\t`, `\"`, `\\`, and `\u{…}` for any other control).
pub(crate) fn string_literal(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
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

/// How many parentheses `text` leaves open, outside its strings and
/// before a `//` comment.
fn depth(text: &str) -> usize {
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if !quoted && c == '/' && chars.peek() == Some(&'/') {
            break;
        }
        match (quoted, escaped, c) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (_, false, '"') => quoted = !quoted,
            (false, _, '(') => depth += 1,
            (false, _, ')') => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
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
    // A found row's value, on its row's line: its byte range there, and the
    // range with the parentheses around it. A value over more lines is the
    // author's to edit.
    let value_at = |a: &contract_syntax::Attr| -> Result<(usize, usize, usize), String> {
        let (v, l) = (a.value.span(), &all[at(a.span.line)]);
        let (start, end) = (v.col as usize - 1, v.end_col as usize - 1);
        let key = (a.span.col as usize - 1).min(l.len());
        if v.line != a.span.line || start < key || end < start || end > l.len() {
            return Err(format!(
                "`{row}` in `:{state}` is written over more than one line: edit it by hand"
            ));
        }
        // Parentheses opened before the value's end and not closed by it
        // (`(0.5)`, `-(0.2)`) close after it; a line that leaves one open
        // goes on to the next.
        let open = depth(&l[key..end]);
        if depth(&l[key..]) != 0 {
            return Err(format!(
                "`{row}` in `:{state}` is written over more than one line: edit it by hand"
            ));
        }
        let mut close = end;
        for _ in 0..open {
            let rest = &l[close..];
            match rest.trim_start_matches(' ').strip_prefix(')') {
                Some(after) => close = l.len() - after.len(),
                None => {
                    return Err(format!(
                    "`{row}` in `:{state}`: its parentheses close on another line: edit it by hand"
                ))
                }
            }
        }
        Ok((start, end, close))
    };
    // Where a found row's value starts as written: before the parentheses
    // and any `-` that opened them (`-(0.2)`), so a set replaces it whole.
    let written_at = |a: &contract_syntax::Attr, start: usize| -> usize {
        let l = &all[at(a.span.line)];
        let key = (a.span.col as usize - 1).min(start);
        let eq = l[key..start].find('=').map_or(key, |i| key + i + 1);
        eq + (l[eq..start].len() - l[eq..start].trim_start().len())
    };
    match op {
        ContractOp::SetState { value, .. } => match found {
            // Its value replaced where it is written.
            Some(a) => {
                let (start, _, close) = value_at(a)?;
                let from = written_at(a, start);
                all[at(a.span.line)].replace_range(from..close, value);
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
            let line = a.span.line;
            let (_, _, close) = value_at(a)?;
            // Alone on its line, a comment after it or none.
            let l = &all[at(line)];
            let after = l[close..].trim();
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
                    let end = l[close..]
                        .find(|c: char| c != ' ')
                        .map_or(l.len(), |n| close + n);
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
        [] => Err(
            json!({"message": format!("no file of the app declares `style {}` (a node's own state part is `set-row`'s, LLP 1122 T5)", op.style())}),
        ),
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
        planned.diff.push_str(&file_diff(&path, before, &after));
        planned.files.push((path, after, vec![back]));
    }
    // A state part compiles into rows and looks: the ordinary rebuild (D13).
    planned.class = exact_tamber::Class::Rebuild;
    Ok(())
}

/// A file's unified diff, one hunk, as the design's (`exact_tamber`'s).
fn file_diff(path: &str, a: &str, b: &str) -> String {
    let (x, y): (Vec<&str>, Vec<&str>) = (
        a.split_inclusive('\n').collect(),
        b.split_inclusive('\n').collect(),
    );
    let start = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
    let end = x[start..]
        .iter()
        .rev()
        .zip(y[start..].iter().rev())
        .take_while(|(p, q)| p == q)
        .count();
    let (old, new) = (&x[start..x.len() - end], &y[start..y.len() - end]);
    // An empty side starts at the line before it (unified diff's rule).
    let at = |n: usize| if n == 0 { start } else { start + 1 };
    let mut out = format!(
        "--- {path}\n+++ {path}\n@@ -{},{} +{},{} @@\n",
        at(old.len()),
        old.len(),
        at(new.len()),
        new.len()
    );
    for (sign, lines) in [('-', old), ('+', new)] {
        for l in lines {
            match l.strip_suffix('\n') {
                Some(l) => out.push_str(&format!("{sign}{l}\n")),
                None => out.push_str(&format!("{sign}{l}\n\\ No newline at end of file\n")),
            }
        }
    }
    out
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

#[cfg(test)]
mod tests {
    #[test]
    fn a_string_literal_is_one_contract_lexes_back() {
        for s in [
            "#000",
            "a\"b\\c",
            "line\nnext\ttab",
            "cr\rhere\u{1}",
            "$ {x} `t`",
        ] {
            let lit = super::string_literal(s);
            let file = contract_syntax::parse(&format!("style S\n  content={lit}\n")).unwrap();
            assert!(
                matches!(&file.styles[0].attrs[0].value, contract_syntax::Expr::Str(v, _) if v == s),
                "{lit}"
            );
        }
    }
}
