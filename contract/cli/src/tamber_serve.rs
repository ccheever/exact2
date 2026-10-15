//! `contract tamber serve --stdio <app dir>` (LLP 1121 D13, stage 3): the one
//! writer of an app's design. One JSON request a line on stdin, one response
//! a line on stdout, and an event line for every change it commits. The wire
//! to clients (`scripts/tamber-server.mjs`) speaks to it; this owns the
//! files: the digests, the lock, the journal, every write.
//!
//! - **The files:** the app's design and any other `.tamber` file beside it.
//! - **One writer per project:** `.exact/tamber.lock`, held while it runs; a
//!   second server for the app refuses to start.
//! - **A transaction** carries each edited file's digest as the client saw
//!   it; a different digest on disk is a conflict. It is checked whole, then
//!   committed as a set: each new text to a temporary file, fsynced, an
//!   intent record in `.exact/tamber.journal`, the renames, the record
//!   removed. A server that starts and finds a record completes its renames.
//! - **The inverse** a transaction returns is an id: `undo` with it applies
//!   the restores the server issued, only while each file is as it left it.
//!   A client never sends a restore.
//! - **`propose`** plans without writing: the diff, the tokens it changes,
//!   `live` or `rebuild`.
//! - **Reads:** `hello`, `describe`, `resolve`, `explain`.

use exact_tamber::Op;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The protocol this server speaks (D13: minor versions only add).
pub const PROTOCOL: &str = "tamber/1.0";

/// `contract tamber serve --stdio <app dir>`.
pub fn run(args: &[String]) -> ExitCode {
    let dir = match args {
        [flag, dir] | [dir, flag] if flag == "--stdio" => PathBuf::from(dir),
        _ => {
            eprintln!("usage: contract tamber serve --stdio <app dir>");
            return ExitCode::from(2);
        }
    };
    let mut server = match Server::start(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("contract tamber serve: {e}");
            return ExitCode::from(1);
        }
    };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        for reply in server.handle(&line) {
            let _ = writeln!(out, "{reply}");
        }
        let _ = out.flush();
    }
    ExitCode::SUCCESS
}

/// The digest of a file's bytes, as the wire carries it.
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An inverse the server issued: each file's digest after the write, and
/// the restores per file.
type Issued = (Vec<(String, String)>, Vec<(String, Op)>);

struct Server {
    dir: PathBuf,
    /// The design's files, by path from the app's directory.
    files: Vec<String>,
    /// Held while the server runs: the one writer.
    _lock: std::fs::File,
    vocab: exact_tamber::Vocab,
    /// The inverses issued: id → (each file's digest after the write, the
    /// restores per file).
    issued: HashMap<String, Issued>,
    next: u64,
    /// The client a `hello` named.
    client: Option<String>,
}

impl Server {
    fn start(dir: &Path) -> Result<Server, String> {
        let design = contract::design_path(&dir.join("app.contract"))
            .filter(|p| p.is_file())
            .ok_or_else(|| {
                format!(
                    "{} has no design (app.tamber, or the file app.json's \"tamber\" names)",
                    dir.display()
                )
            })?;
        let rel = design
            .strip_prefix(dir)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| design.to_string_lossy().into_owned());
        let state = dir.join(".exact");
        std::fs::create_dir_all(&state).map_err(|e| format!("{}: {e}", state.display()))?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(state.join("tamber.lock"))
            .map_err(|e| format!("the lock: {e}"))?;
        if lock.try_lock().is_err() {
            return Err(format!(
                "another tamber server writes {} (its lock is .exact/tamber.lock)",
                dir.display()
            ));
        }
        // The app's design first, then any other design beside it (a
        // transaction may edit several, committed as one set).
        let mut files = vec![rel.clone()];
        let mut others: Vec<String> = std::fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.ends_with(".tamber") && *n != rel)
            .collect();
        others.sort();
        files.extend(others);
        let server = Server {
            dir: dir.to_path_buf(),
            files,
            _lock: lock,
            vocab: contract_lower::tamber_vocab(),
            issued: HashMap::new(),
            next: 0,
            client: None,
        };
        server.recover()?;
        Ok(server)
    }

    /// Complete a transaction a crash left between its renames.
    fn recover(&self) -> Result<(), String> {
        let journal = self.dir.join(".exact/tamber.journal");
        let Ok(text) = std::fs::read_to_string(&journal) else {
            return Ok(());
        };
        let record: Value = serde_json::from_str(&text).map_err(|e| format!("the journal: {e}"))?;
        for pair in record["renames"].as_array().into_iter().flatten() {
            let (Some(tmp), Some(target)) = (pair[0].as_str(), pair[1].as_str()) else {
                continue;
            };
            let tmp = self.dir.join(tmp);
            if tmp.exists() {
                std::fs::rename(&tmp, self.dir.join(target))
                    .map_err(|e| format!("completing {target}: {e}"))?;
            }
        }
        std::fs::remove_file(&journal).map_err(|e| format!("the journal: {e}"))
    }

    fn read(&self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(self.dir.join(path)).map_err(|e| format!("{path}: {e}"))
    }

    fn digests(&self) -> Vec<(String, String)> {
        self.files
            .iter()
            .map(|f| {
                (
                    f.clone(),
                    std::fs::read(self.dir.join(f)).map_or(String::new(), |b| digest(&b)),
                )
            })
            .collect()
    }

    fn handle(&mut self, line: &str) -> Vec<Value> {
        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                return vec![json!({"id": null, "error": {"message": format!("not JSON: {e}")}})]
            }
        };
        let id = request["id"].clone();
        let reply = |r: Result<Value, Value>| match r {
            Ok(ok) => json!({"id": id, "ok": ok}),
            Err(error) => json!({"id": id, "error": error}),
        };
        match request["method"].as_str() {
            Some("hello") => {
                self.client = request["client"].as_str().map(str::to_string);
                vec![reply(Ok(
                    json!({"protocol": PROTOCOL, "format": exact_tamber::FORMAT, "builds": []}),
                ))]
            }
            Some("describe") => vec![reply(self.describe())],
            Some("resolve") => vec![reply(self.resolve(&request))],
            Some("explain") => vec![reply(self.explain(&request))],
            Some("propose") => vec![reply(self.transaction(&request, false))],
            Some("transaction") => self.commit_reply(&request, reply, false),
            Some("undo") => self.commit_reply(&request, reply, true),
            other => vec![reply(Err(
                json!({"message": format!("no method {}", other.unwrap_or("(none)"))}),
            ))],
        }
    }

    /// A write's reply, and its event when it committed.
    fn commit_reply(
        &mut self,
        request: &Value,
        reply: impl Fn(Result<Value, Value>) -> Value,
        undo: bool,
    ) -> Vec<Value> {
        let result = match undo {
            true => self.undo(request),
            false => self.transaction(request, true),
        };
        let event = result.as_ref().ok().map(|ok| {
            json!({"event": "changed", "files": ok["files"], "client": request["client"].as_str().or(self.client.as_deref())})
        });
        let mut out = vec![reply(result)];
        out.extend(event);
        out
    }

    fn design(&self) -> Result<(String, exact_tamber::Folding), Value> {
        let path = &self.files[0];
        let text = self.read(path).map_err(|m| json!({"message": m}))?;
        let folding = exact_tamber::check(&text, &self.vocab).map_err(|all| refused(path, all))?;
        Ok((text, folding))
    }

    fn describe(&self) -> Result<Value, Value> {
        let (_, folding) = self.design()?;
        let ir: Value =
            serde_json::from_str(&folding.ir()).map_err(|e| json!({"message": e.to_string()}))?;
        let meta =
            |m: &[(String, String)]| m.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>();
        let dials: serde_json::Map<String, Value> = folding
            .design
            .dials
            .iter()
            .map(|d| (d.name.value.clone(), json!(meta(&d.meta))))
            .collect();
        let tokens: serde_json::Map<String, Value> = folding
            .design
            .tokens
            .iter()
            .map(|t| (t.name.value.clone(), json!(meta(&t.meta))))
            .collect();
        Ok(
            json!({"ir": ir, "files": files_json(&self.digests()), "meta": {"dials": dials, "tokens": tokens}}),
        )
    }

    /// A token's stored value per tuple and scheme; a host value (a role, a
    /// platform colour) is a residual, labelled so.
    fn resolve(&self, request: &Value) -> Result<Value, Value> {
        let (_, folding) = self.design()?;
        let name = request["token"]
            .as_str()
            .ok_or_else(|| json!({"message": "resolve names a `token`"}))?;
        let token = folding
            .tokens
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| json!({"message": format!("no token `{name}`")}))?;
        let axes = &folding.design.axes;
        let tuples: Vec<Value> = token
            .values
            .iter()
            .enumerate()
            .map(|(tuple, v)| {
                let modes = modes_of(axes, tuple);
                match v {
                    exact_tamber::Folded::One(h) => {
                        json!({"modes": modes, "value": v.css(), "light": h.css(), "dark": h.css()})
                    }
                    exact_tamber::Folded::Pair(l, d) => {
                        json!({"modes": modes, "value": v.css(), "light": l.css(), "dark": d.css()})
                    }
                    exact_tamber::Folded::Host(_) => {
                        json!({"modes": modes, "value": v.css(), "residual": true})
                    }
                }
            })
            .collect();
        Ok(json!({"token": name, "tuples": tuples}))
    }

    /// A token's chain: the dials it depends on, and the tokens its value
    /// names.
    fn explain(&self, request: &Value) -> Result<Value, Value> {
        let (_, folding) = self.design()?;
        let name = request["token"]
            .as_str()
            .ok_or_else(|| json!({"message": "explain names a `token`"}))?;
        let (token, declared) = folding
            .tokens
            .iter()
            .zip(&folding.design.tokens)
            .find(|(t, _)| t.name == name)
            .ok_or_else(|| json!({"message": format!("no token `{name}`")}))?;
        let names: Vec<&str> = folding
            .design
            .tokens
            .iter()
            .map(|t| t.name.value.as_str())
            .filter(|n| *n != declared.name.value && named(&declared.value.value, n))
            .collect();
        Ok(
            json!({"token": name, "value": declared.value.value, "dials": token.dials, "names": names}),
        )
    }

    /// Plan, and when `write`, commit: the new digests, the inverse's id, the
    /// tokens it changes, `live` or `rebuild`, the diff.
    fn transaction(&mut self, request: &Value, write: bool) -> Result<Value, Value> {
        let ops = parse_ops(&request["ops"], &self.files[0])?;
        let seen = request["files"].as_object().cloned().unwrap_or_default();
        let current = self.digests();
        if write {
            // Each file the transaction edits, as the client last saw it.
            let stale: Vec<&(String, String)> = current
                .iter()
                .filter(|(p, d)| {
                    ops.iter().any(|(f, _)| f == p)
                        && seen.get(p).and_then(Value::as_str) != Some(d.as_str())
                })
                .collect();
            if !stale.is_empty() {
                return Err(
                    json!({"conflict": true, "message": "a file changed since the client read it", "files": files_json(&current)}),
                );
            }
        }
        self.run(&ops, write, false)
    }

    /// Apply an inverse this server issued, while every file is as it left it.
    fn undo(&mut self, request: &Value) -> Result<Value, Value> {
        let id = request["inverse"]
            .as_str()
            .ok_or_else(|| json!({"message": "undo names an `inverse` the server issued"}))?;
        let (after, ops) = self
            .issued
            .get(id)
            .cloned()
            .ok_or_else(|| json!({"message": format!("no inverse `{id}` was issued")}))?;
        let current = self.digests();
        if after.iter().any(|(p, d)| {
            current
                .iter()
                .find(|(q, _)| q == p)
                .is_none_or(|(_, e)| e != d)
        }) {
            return Err(
                json!({"conflict": true, "message": "a file changed since that write: its inverse no longer applies", "files": files_json(&current)}),
            );
        }
        let result = self.run(&ops, true, true);
        if result.is_ok() {
            self.issued.remove(id);
        }
        result
    }

    fn run(&mut self, ops: &[(String, Op)], write: bool, undo: bool) -> Result<Value, Value> {
        let texts: Vec<(String, String)> = self
            .files
            .iter()
            .map(|f| self.read(f).map(|t| (f.clone(), t)))
            .collect::<Result<_, _>>()
            .map_err(|m| json!({"message": m}))?;
        let planned = exact_tamber::plan(&texts, ops, &self.vocab, undo).map_err(|all| {
            json!({"message": "refused", "diagnostics": all.iter().map(|(f, d)| diagnostic(f, d)).collect::<Vec<_>>()})
        })?;
        let mut out = json!({
            "changed": planned.changed,
            "class": planned.class.name(),
            "diff": planned.diff,
        });
        if !write {
            return Ok(out);
        }
        self.commit(&planned.files)
            .map_err(|m| json!({"message": m}))?;
        let after = self.digests();
        self.next += 1;
        let id = format!("inv-{}", self.next);
        let inverse: Vec<(String, Op)> = planned
            .files
            .iter()
            .flat_map(|(path, _, inv)| inv.iter().map(move |op| (path.clone(), op.clone())))
            .collect();
        self.issued.insert(id.clone(), (after.clone(), inverse));
        out["files"] = files_json(&after);
        out["inverse"] = json!(id);
        Ok(out)
    }

    /// Commit new texts as a set: temporary files fsynced, the intent record,
    /// the renames, the record removed.
    fn commit(&self, files: &[(String, String, Vec<Op>)]) -> Result<(), String> {
        let mut renames = Vec::new();
        for (i, (path, text, _)) in files.iter().enumerate() {
            let tmp = format!(".exact/tamber-{}-{i}.tmp", std::process::id());
            let mut f =
                std::fs::File::create(self.dir.join(&tmp)).map_err(|e| format!("{tmp}: {e}"))?;
            f.write_all(text.as_bytes())
                .and_then(|()| f.sync_all())
                .map_err(|e| format!("{tmp}: {e}"))?;
            renames.push((tmp, path.clone()));
        }
        let journal = self.dir.join(".exact/tamber.journal");
        let record =
            json!({"renames": renames.iter().map(|(t, p)| json!([t, p])).collect::<Vec<_>>()});
        let mut j = std::fs::File::create(&journal).map_err(|e| format!("the journal: {e}"))?;
        j.write_all(record.to_string().as_bytes())
            .and_then(|()| j.sync_all())
            .map_err(|e| format!("the journal: {e}"))?;
        for (n, (tmp, path)) in renames.iter().enumerate() {
            std::fs::rename(self.dir.join(tmp), self.dir.join(path))
                .map_err(|e| format!("{path}: {e}"))?;
            // A test's crash between renames (the gate: a restart completes them).
            if n == 0 && std::env::var_os("EXACT_TAMBER_CRASH_AFTER_RENAME").is_some() {
                std::process::exit(9);
            }
        }
        std::fs::remove_file(&journal).map_err(|e| format!("the journal: {e}"))
    }
}

/// `{path: digest}`.
fn files_json(files: &[(String, String)]) -> Value {
    Value::Object(files.iter().map(|(p, d)| (p.clone(), json!(d))).collect())
}

fn diagnostic(file: &str, d: &exact_tamber::Diagnostic) -> Value {
    json!({"file": file, "line": d.line, "col": d.col, "id": d.id, "message": d.message})
}

fn refused(file: &str, all: Vec<exact_tamber::Diagnostic>) -> Value {
    json!({"message": "the design is refused", "diagnostics": all.iter().map(|d| diagnostic(file, d)).collect::<Vec<_>>()})
}

/// Tuple `tuple`'s modes, `{axis: value}`, the first axis most significant.
fn modes_of(axes: &[exact_tamber::Axis], tuple: usize) -> Value {
    let mut rest = tuple;
    let mut out = vec![(String::new(), String::new()); axes.len()];
    for (i, a) in axes.iter().enumerate().rev() {
        out[i] = (
            a.name.value.clone(),
            a.values[rest % a.values.len()].clone(),
        );
        rest /= a.values.len();
    }
    Value::Object(out.into_iter().map(|(a, v)| (a, json!(v))).collect())
}

/// Whether `value` names the token `name` as a whole word.
fn named(value: &str, name: &str) -> bool {
    let c = |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.');
    value.match_indices(name).any(|(at, _)| {
        value[..at]
            .chars()
            .next_back()
            .is_none_or(|p| !c(p) && p != '#')
            && value[at + name.len()..]
                .chars()
                .next()
                .is_none_or(|n| !c(n))
    })
}

/// The wire's operations (D13's names, kebab-case; named fields), each with
/// the file it edits (the design's own when unsaid). A `restore` is no
/// client's to send.
fn parse_ops(ops: &Value, design: &str) -> Result<Vec<(String, Op)>, Value> {
    let bad = |m: String| json!({"message": m});
    let s = |v: &Value, k: &str| {
        v[k].as_str()
            .map(str::to_string)
            .ok_or_else(|| bad(format!("`{k}` is a string")))
    };
    let opt = |v: &Value, k: &str| v[k].as_str().map(str::to_string);
    let conds = |v: &Value| -> Result<Vec<(String, String)>, Value> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|c| Ok((s(c, "axis")?, s(c, "value")?)))
                    .collect()
            })
            .unwrap_or(Ok(Vec::new()))
    };
    let mut out = Vec::new();
    for v in ops
        .as_array()
        .ok_or_else(|| bad("`ops` is a list".into()))?
    {
        let file = opt(v, "file").unwrap_or_else(|| design.to_string());
        let op = match v["op"].as_str() {
            Some("set-dial") => Op::SetDial {
                name: s(v, "name")?,
                value: s(v, "value")?,
            },
            Some("set-token") => Op::SetToken {
                name: s(v, "name")?,
                when: conds(&v["when"])?,
                value: s(v, "value")?,
                at: None,
                comment: opt(v, "comment"),
            },
            Some("unset-token-when") => Op::UnsetTokenWhen {
                name: s(v, "name")?,
                when: conds(&v["when"])?,
            },
            Some("add-token") => Op::AddToken {
                name: s(v, "name")?,
                kind: opt(v, "kind"),
                value: s(v, "value")?,
                meta: v["meta"]
                    .as_array()
                    .map(|m| {
                        m.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
                whens: v["whens"]
                    .as_array()
                    .map(|w| {
                        w.iter()
                            .map(|w| Ok((conds(&w["when"])?, s(w, "value")?, opt(w, "comment"))))
                            .collect::<Result<Vec<_>, Value>>()
                    })
                    .unwrap_or(Ok(Vec::new()))?,
                comment: opt(v, "comment"),
                before: opt(v, "before"),
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
                before: opt(v, "before"),
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
