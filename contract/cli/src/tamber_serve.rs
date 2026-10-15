//! `contract tamber serve --stdio <app dir>` (LLP 1121 D13, stage 3): the one
//! writer of an app's design. One JSON request a line on stdin, one response
//! a line on stdout, and an event line for every change it commits. The wire
//! to clients (`scripts/tamber-server.mjs`) speaks to it; this owns the
//! files: the digests, the lock, the journal, every write.
//!
//! - **The files:** the app's design and any other `.tamber` file beside it,
//!   each a plain file inside the app's directory.
//! - **One writer per project:** `.exact/tamber.lock`, held while it runs; a
//!   second server for the app refuses to start.
//! - **A transaction** names its `client` and carries each edited file's
//!   digest as the client saw it; a different digest on disk is a conflict.
//!   The files are read once, and that read is what is checked, planned and
//!   compared. It is committed as a set: each new text to a temporary file
//!   of its own, fsynced; each file read again, a conflict if it moved; an
//!   intent record (each rename and its new text's digest) written beside
//!   and renamed to `.exact/tamber.journal`; the renames; the record
//!   removed; each directory flushed. A write that fails part way is
//!   completed at once, or the server stops.
//! - **At start** a record is completed: a rename is done when its
//!   temporary file has the recorded digest, already done when the file
//!   has it. A record that names anything else, or cannot be read, is kept
//!   and the server refuses to start. Temporary files no record names are
//!   removed.
//! - **The inverse** a transaction returns is an id, unique to this run:
//!   `undo` with it applies the restores the server issued, only while each
//!   file it restores is as the write left it. The last 256 are kept. A
//!   client never sends a restore.
//! - **`propose`** plans without writing: the diff, the tokens it changes
//!   (by file), `live` or `rebuild`, and the digests it planned against; a
//!   digest it carries is checked as a transaction's is.
//! - **Reads:** `hello`, `describe`, `resolve`, `explain`, each of the
//!   design or of the `file` it names.
//! - **A request** is one JSON object on a line of at most 1 MiB; a field
//!   of the wrong type is an error, not absent.

use exact_tamber::Op;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

/// The protocol this server speaks (D13: minor versions only add).
pub const PROTOCOL: &str = "tamber/1.0";

/// The longest request line it reads; a longer one is refused whole.
const LINE_LIMIT: usize = 1 << 20;

/// How many inverses it keeps; the oldest goes first.
const ISSUED_LIMIT: usize = 256;

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
    let mut input = stdin.lock();
    let mut out = std::io::stdout().lock();
    let error = |m: &str| json!({"id": null, "error": {"message": m}});
    loop {
        let replies = match read_line(&mut input, LINE_LIMIT) {
            Ok(None) => break,
            Ok(Some(None)) => vec![error("a request is one line of at most 1 MiB")],
            Ok(Some(Some(bytes))) => match String::from_utf8(bytes) {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => server.handle(&line),
                Err(_) => vec![error("a request is UTF-8")],
            },
            Err(e) => {
                eprintln!("contract tamber serve: stdin: {e}");
                return ExitCode::from(1);
            }
        };
        for reply in replies {
            let _ = writeln!(out, "{reply}");
        }
        let _ = out.flush();
    }
    ExitCode::SUCCESS
}

/// One line's bytes, `Some(None)` for a line longer than `limit` (read to
/// its end and dropped), `None` at the end of input.
fn read_line(input: &mut impl BufRead, limit: usize) -> std::io::Result<Option<Option<Vec<u8>>>> {
    let mut line = Some(Vec::new());
    let mut any = false;
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return Ok(any.then_some(line));
        }
        any = true;
        let newline = buf.iter().position(|&b| b == b'\n');
        let chunk = &buf[..newline.unwrap_or(buf.len())];
        if let Some(l) = &mut line {
            if l.len() + chunk.len() > limit {
                line = None;
            } else {
                l.extend_from_slice(chunk);
            }
        }
        let used = newline.map_or(buf.len(), |n| n + 1);
        input.consume(used);
        if newline.is_some() {
            return Ok(Some(line));
        }
    }
}

/// The digest of a file's bytes, as the wire carries it.
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An inverse the server issued: the digest of each file it restores, as
/// the write left it, and the restores per file.
#[derive(Clone)]
struct Issued {
    after: Vec<(String, String)>,
    ops: Vec<(String, Op)>,
}

/// The design's files as one read: each path, its text, its digest.
type Snapshot = Vec<(String, String, String)>;

/// One rename the journal records: the temporary file in `.exact`, the
/// design file it becomes, and that new text's digest.
struct Rename {
    tmp: String,
    file: String,
    digest: String,
}

struct Server {
    /// The app's directory, canonical.
    dir: PathBuf,
    /// The design's files, by path from the app's directory.
    files: Vec<String>,
    /// Held while the server runs: the one writer.
    _lock: File,
    vocab: exact_tamber::Vocab,
    /// The inverses issued, by id, and their order, the oldest first.
    issued: HashMap<String, Issued>,
    order: VecDeque<String>,
    /// This run's name in inverse ids and temporary files: its pid and
    /// start time, so an id from an earlier run never matches.
    session: String,
    next: u64,
    /// The client a `hello` named.
    client: Option<String>,
}

/// Open without following a symbolic link at the last component.
fn no_follow(options: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options
}

/// Flush a directory's entries (the renames in it) to disk.
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// A file's bytes when it is a regular file, `None` when it is not there;
/// a link or anything else is an error.
fn read_plain(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
        Ok(m) if !m.is_file() => Err(format!("{} is not a plain file", path.display())),
        Ok(_) => std::fs::read(path)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
    }
}

impl Server {
    fn start(dir: &Path) -> Result<Server, String> {
        let dir = dir
            .canonicalize()
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        let design = contract::design_path(&dir.join("app.contract"))
            .filter(|p| p.is_file())
            .ok_or_else(|| {
                format!(
                    "{} has no design (app.tamber, or the file app.json's \"tamber\" names)",
                    dir.display()
                )
            })?;
        // The design is a file inside the app, reached by no link.
        let real = design
            .canonicalize()
            .map_err(|e| format!("{}: {e}", design.display()))?;
        let rel = real
            .strip_prefix(&dir)
            .ok()
            .filter(|r| r.components().all(|c| matches!(c, Component::Normal(_))))
            .and_then(|r| r.to_str())
            .map(str::to_string)
            .ok_or_else(|| {
                format!(
                    "the design {} is outside {}",
                    design.display(),
                    dir.display()
                )
            })?;
        if real != dir.join(&rel) || rel.starts_with(".exact") {
            return Err(format!(
                "the design {} is outside the app",
                design.display()
            ));
        }
        let state = dir.join(".exact");
        match std::fs::symlink_metadata(&state) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(format!("{} is not a directory", state.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                std::fs::create_dir(&state).map_err(|e| format!("{}: {e}", state.display()))?
            }
            Err(e) => return Err(format!("{}: {e}", state.display())),
        }
        let lock = no_follow(OpenOptions::new().create(true).truncate(false).write(true))
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
        let mut others: Vec<String> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| {
                let e = e.ok()?;
                e.file_type().ok()?.is_file().then_some(())?;
                e.file_name().into_string().ok()
            })
            .filter(|n| n.ends_with(".tamber") && *n != rel)
            .collect();
        others.sort();
        files.extend(others);
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let server = Server {
            dir,
            files,
            _lock: lock,
            vocab: contract_lower::tamber_vocab(),
            issued: HashMap::new(),
            order: VecDeque::new(),
            session: format!("{}-{started:x}", std::process::id()),
            next: 0,
            client: None,
        };
        server.recover()?;
        server.sweep();
        Ok(server)
    }

    fn state(&self) -> PathBuf {
        self.dir.join(".exact")
    }

    /// Complete a transaction a crash left between its renames. Each rename
    /// is done when its temporary file is there with the recorded digest, or
    /// already done when the file has it; anything else keeps the journal
    /// and refuses, for a person to look at.
    fn recover(&self) -> Result<(), String> {
        let journal = self.state().join("tamber.journal");
        let Some(bytes) = read_plain(&journal).map_err(|e| format!("the journal: {e}"))? else {
            return Ok(());
        };
        let renames = self
            .journal(&bytes)
            .map_err(|e| format!("the journal .exact/tamber.journal: {e}; it is kept"))?;
        for r in &renames {
            let tmp = self.state().join(&r.tmp);
            let target = self.dir.join(&r.file);
            match read_plain(&tmp)? {
                Some(b) if digest(&b) == r.digest => std::fs::rename(&tmp, &target)
                    .map_err(|e| format!("completing {}: {e}; the journal is kept", r.file))?,
                Some(_) => {
                    return Err(format!(
                        "{} is not the text the journal records for {}; the journal is kept",
                        r.tmp, r.file
                    ))
                }
                None => {
                    if read_plain(&target)?.is_none_or(|b| digest(&b) != r.digest) {
                        return Err(format!(
                            "{} has neither its new text nor its temporary file; the journal is kept",
                            r.file
                        ));
                    }
                }
            }
        }
        self.sync_files(renames.iter().map(|r| r.file.as_str()))?;
        std::fs::remove_file(&journal).map_err(|e| format!("the journal: {e}"))?;
        sync_dir(&self.state()).map_err(|e| format!(".exact: {e}"))
    }

    /// A journal's renames, each a temporary file in `.exact` and one of the
    /// design's files.
    fn journal(&self, bytes: &[u8]) -> Result<Vec<Rename>, String> {
        let record: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let field = |v: &Value, k: &str| {
            v[k].as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("a rename without its `{k}`"))
        };
        let mut out = Vec::new();
        for v in record["renames"]
            .as_array()
            .ok_or("no `renames`".to_string())?
        {
            let r = Rename {
                tmp: field(v, "tmp")?,
                file: field(v, "file")?,
                digest: field(v, "digest")?,
            };
            if !temp_name(&r.tmp) {
                return Err(format!("`{}` is no temporary file of this server", r.tmp));
            }
            if !self.files.contains(&r.file) {
                return Err(format!("`{}` is no file of this design", r.file));
            }
            out.push(r);
        }
        Ok(out)
    }

    /// Remove temporary files no journal names (a write that stopped before
    /// its journal); the lock is held, so none is another server's.
    fn sweep(&self) {
        let Ok(entries) = std::fs::read_dir(self.state()) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if temp_name(&name) || name == "tamber.journal.tmp" {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    /// Flush the directories holding `files` after renames into them.
    fn sync_files<'a>(&self, files: impl Iterator<Item = &'a str>) -> Result<(), String> {
        let mut dirs: Vec<PathBuf> = files
            .filter_map(|f| self.dir.join(f).parent().map(Path::to_path_buf))
            .collect();
        dirs.sort();
        dirs.dedup();
        for d in dirs {
            sync_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        Ok(())
    }

    /// Every design file, read once: the text a plan edits is the text its
    /// digest is of.
    fn snapshot(&self) -> Result<Snapshot, Value> {
        self.files
            .iter()
            .map(|f| {
                let bytes = read_plain(&self.dir.join(f))
                    .and_then(|b| b.ok_or_else(|| format!("{f} is gone")))
                    .map_err(|m| json!({"message": m}))?;
                let d = digest(&bytes);
                let text = String::from_utf8(bytes)
                    .map_err(|_| json!({"message": format!("{f} is not UTF-8")}))?;
                Ok((f.clone(), text, d))
            })
            .collect()
    }

    fn handle(&mut self, line: &str) -> Vec<Value> {
        let request: Value = match serde_json::from_str(line) {
            Ok(v @ Value::Object(_)) => v,
            Ok(_) => {
                return vec![json!({"id": null, "error": {"message": "a request is an object"}})]
            }
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
            Some("hello") => vec![reply(self.hello(&request))],
            Some("describe") => vec![reply(self.describe(&request))],
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

    fn hello(&mut self, request: &Value) -> Result<Value, Value> {
        // A hello that names no client leaves the one before it.
        if let Some(c) = optional(request, "client")? {
            self.client = Some(c);
        }
        Ok(json!({"protocol": PROTOCOL, "format": exact_tamber::FORMAT, "builds": []}))
    }

    /// A write's reply, and its event when it wrote.
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
        let event = result
            .as_ref()
            .ok()
            .filter(|ok| ok["inverse"].is_string())
            .map(|ok| {
                json!({"event": "changed", "files": ok["files"], "changed": ok["changed"], "client": ok["client"]})
            });
        let mut out = vec![reply(result)];
        out.extend(event);
        out
    }

    /// The writer a write names: its own `client`, or its hello's.
    fn writer(&self, request: &Value) -> Result<String, Value> {
        optional(request, "client")?
            .or_else(|| self.client.clone())
            .ok_or_else(|| json!({"message": "a write names its `client`, or its hello does"}))
    }

    /// The file a read names (`file`), the app's design when unsaid.
    fn read_file(&self, request: &Value) -> Result<String, Value> {
        let file = optional(request, "file")?.unwrap_or_else(|| self.files[0].clone());
        match self.files.contains(&file) {
            true => Ok(file),
            false => Err(json!({"message": format!("`{file}` is no file of this design")})),
        }
    }

    /// A read's file checked, from one snapshot that also gives the digests.
    fn design(&self, request: &Value) -> Result<(Snapshot, exact_tamber::Folding), Value> {
        let file = self.read_file(request)?;
        let snap = self.snapshot()?;
        let text = &snap
            .iter()
            .find(|(f, _, _)| *f == file)
            .expect("a design file")
            .1;
        let folding = exact_tamber::check(text, &self.vocab).map_err(|all| refused(&file, all))?;
        Ok((snap, folding))
    }

    fn describe(&self, request: &Value) -> Result<Value, Value> {
        let (snap, folding) = self.design(request)?;
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
        // By the IR's names: a role's row spelling, not the key written.
        let tokens: serde_json::Map<String, Value> = folding
            .tokens
            .iter()
            .zip(&folding.design.tokens)
            .map(|(t, declared)| (t.name.clone(), json!(meta(&declared.meta))))
            .collect();
        Ok(
            json!({"ir": ir, "files": files_json(&snap), "meta": {"dials": dials, "tokens": tokens}}),
        )
    }

    /// A token's stored value per tuple and scheme; a host value (a role, a
    /// platform colour) is a residual, labelled so, with its fallback.
    fn resolve(&self, request: &Value) -> Result<Value, Value> {
        let (_, folding) = self.design(request)?;
        let name = required(request, "token")?;
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
                        let fallback = exact_tamber::fallback(v, &self.vocab)
                            .map(|[l, d]| json!({"light": l, "dark": d}));
                        json!({"modes": modes, "value": v.css(), "residual": true, "fallback": fallback})
                    }
                }
            })
            .collect();
        Ok(json!({"token": name, "tuples": tuples}))
    }

    /// A token's chain: its default and `when` values, the dials it depends
    /// on, and the tokens those values name, as the checker resolved them.
    fn explain(&self, request: &Value) -> Result<Value, Value> {
        let (_, folding) = self.design(request)?;
        let name = required(request, "token")?;
        let (token, declared) = folding
            .tokens
            .iter()
            .zip(&folding.design.tokens)
            .find(|(t, _)| t.name == name)
            .ok_or_else(|| json!({"message": format!("no token `{name}`")}))?;
        let whens: Vec<Value> = declared
            .whens
            .iter()
            .map(|w| {
                json!({"when": w.conds.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(), "value": w.value.value})
            })
            .collect();
        Ok(
            json!({"token": name, "value": declared.value.value, "whens": whens, "dials": token.dials, "names": token.names}),
        )
    }

    /// Plan, and when `write`, commit: the digests, the inverse's id, the
    /// tokens it changes, `live` or `rebuild`, the diff. Each file it edits
    /// must carry the digest the client saw (a `propose` checks the ones it
    /// carries).
    fn transaction(&mut self, request: &Value, write: bool) -> Result<Value, Value> {
        let ops = parse_ops(&request["ops"], &self.files[0])?;
        let seen = seen_files(request)?;
        let client = match write {
            true => Some(self.writer(request)?),
            false => None,
        };
        let snap = self.snapshot()?;
        let stale = snap.iter().any(|(p, _, d)| {
            ops.iter().any(|(f, _)| f == p)
                && match seen.get(p) {
                    Some(s) => s != d,
                    None => write,
                }
        });
        if stale {
            return Err(
                json!({"conflict": true, "message": "a file changed since the client read it", "files": files_json(&snap)}),
            );
        }
        self.run(&snap, &ops, client, false)
    }

    /// Apply an inverse this server issued, while each file it restores is
    /// as the write left it.
    fn undo(&mut self, request: &Value) -> Result<Value, Value> {
        let client = self.writer(request)?;
        let id = required(request, "inverse")?;
        let issued = self.issued.get(&id).cloned().ok_or_else(|| {
            json!({"message": format!("no inverse `{id}`: this server issued none by that id, or it has dropped it (it keeps the last {ISSUED_LIMIT})")})
        })?;
        let snap = self.snapshot()?;
        if issued.after.iter().any(|(p, d)| {
            snap.iter()
                .find(|(q, _, _)| q == p)
                .is_none_or(|(_, _, e)| e != d)
        }) {
            return Err(
                json!({"conflict": true, "message": "a file changed since that write: its inverse no longer applies", "files": files_json(&snap)}),
            );
        }
        let result = self.run(&snap, &issued.ops, Some(client), true);
        if result.is_ok() {
            self.issued.remove(&id);
            self.order.retain(|i| *i != id);
        }
        result
    }

    /// Plan over `snap`; with a `client`, commit and issue the inverse.
    fn run(
        &mut self,
        snap: &Snapshot,
        ops: &[(String, Op)],
        client: Option<String>,
        undo: bool,
    ) -> Result<Value, Value> {
        let texts: Vec<(String, String)> = snap
            .iter()
            .map(|(f, t, _)| (f.clone(), t.clone()))
            .collect();
        let planned = exact_tamber::plan(&texts, ops, &self.vocab, undo).map_err(|all| {
            json!({"message": "refused", "diagnostics": all.iter().map(|(f, d)| diagnostic(f, d)).collect::<Vec<_>>()})
        })?;
        let mut out = json!({
            "files": files_json(snap),
            "changed": planned.changed.iter().map(|(f, t)| json!({"file": f, "token": t})).collect::<Vec<_>>(),
            "class": planned.class.name(),
            "diff": planned.diff,
        });
        let Some(client) = client else {
            return Ok(out);
        };
        out["client"] = json!(client);
        if planned.files.is_empty() {
            out["inverse"] = Value::Null;
            return Ok(out);
        }
        let expected: Vec<(String, String)> = planned
            .files
            .iter()
            .filter_map(|(p, _, _)| snap.iter().find(|(f, _, _)| f == p))
            .map(|(f, _, d)| (f.clone(), d.clone()))
            .collect();
        self.commit(&planned.files, &expected)?;
        // The inverse holds while each file it restores has the text written.
        let after: Vec<(String, String)> = planned
            .files
            .iter()
            .map(|(p, text, _)| (p.clone(), digest(text.as_bytes())))
            .collect();
        let mut files: Vec<(String, String, String)> = snap.clone();
        for (f, _, d) in &mut files {
            if let Some((_, a)) = after.iter().find(|(p, _)| p == f) {
                d.clone_from(a);
            }
        }
        self.next += 1;
        let id = format!("inv-{}-{}", self.session, self.next);
        let inverse: Vec<(String, Op)> = planned
            .files
            .iter()
            .flat_map(|(path, _, inv)| inv.iter().map(move |op| (path.clone(), op.clone())))
            .collect();
        self.issued.insert(
            id.clone(),
            Issued {
                after,
                ops: inverse,
            },
        );
        self.order.push_back(id.clone());
        while self.order.len() > ISSUED_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.issued.remove(&old);
            }
        }
        out["files"] = files_json(&files);
        out["inverse"] = json!(id);
        Ok(out)
    }

    /// Commit new texts as a set: each to a temporary file of its own,
    /// fsynced; each file read again and refused if it moved since the
    /// snapshot; the intent record written beside and renamed into place;
    /// the renames; the record removed; each step's directory flushed.
    fn commit(
        &mut self,
        files: &[(String, String, Vec<Op>)],
        expected: &[(String, String)],
    ) -> Result<(), Value> {
        let fail = |m: String| json!({"message": m});
        let state = self.state();
        let journal = state.join("tamber.journal");
        if read_plain(&journal).map_err(fail)?.is_some() {
            return Err(fail(
                "a journal from an unfinished write is outstanding; restart the server to complete it".into(),
            ));
        }
        let mut renames: Vec<Rename> = Vec::new();
        let clean = |renames: &[Rename]| {
            for r in renames {
                let _ = std::fs::remove_file(state.join(&r.tmp));
            }
        };
        for (path, text, _) in files {
            self.next += 1;
            let tmp = format!("tamber-{}-{}.tmp", self.session, self.next);
            let written = no_follow(OpenOptions::new().write(true).create_new(true))
                .open(state.join(&tmp))
                .and_then(|mut f| {
                    f.write_all(text.as_bytes())?;
                    f.sync_all()
                });
            if let Err(e) = written {
                let _ = std::fs::remove_file(state.join(&tmp));
                clean(&renames);
                return Err(fail(format!(".exact/{tmp}: {e}")));
            }
            renames.push(Rename {
                tmp,
                file: path.clone(),
                digest: digest(text.as_bytes()),
            });
        }
        // Another writer (an editor, `tamber fmt`) since the snapshot.
        for (path, d) in expected {
            let now = read_plain(&self.dir.join(path)).map_err(fail)?;
            if now.is_none_or(|b| digest(&b) != *d) {
                clean(&renames);
                let files = self
                    .snapshot()
                    .map(|s| files_json(&s))
                    .unwrap_or(Value::Null);
                return Err(
                    json!({"conflict": true, "message": format!("{path} changed while the write was planned"), "files": files}),
                );
            }
        }
        let record = json!({"renames": renames.iter().map(|r| json!({"tmp": r.tmp, "file": r.file, "digest": r.digest})).collect::<Vec<_>>()});
        let staged = state.join("tamber.journal.tmp");
        let published = no_follow(OpenOptions::new().write(true).create_new(true))
            .open(&staged)
            .and_then(|mut j| {
                j.write_all(record.to_string().as_bytes())?;
                j.sync_all()
            })
            .and_then(|()| std::fs::rename(&staged, &journal))
            .and_then(|()| sync_dir(&state));
        if let Err(e) = published {
            let _ = std::fs::remove_file(&staged);
            if read_plain(&journal).ok().flatten().is_some() {
                // Renamed into place but not flushed: the record stands, and
                // completing it now is as good as completing it at start.
                return self.complete_or_exit();
            }
            clean(&renames);
            return Err(fail(format!("the journal: {e}")));
        }
        for (n, r) in renames.iter().enumerate() {
            if let Err(e) = std::fs::rename(state.join(&r.tmp), self.dir.join(&r.file)) {
                eprintln!("contract tamber serve: {}: {e}", r.file);
                return self.complete_or_exit();
            }
            // A test's crash between renames (the gate: a restart completes them).
            if n == 0 && std::env::var_os("EXACT_TAMBER_CRASH_AFTER_RENAME").is_some() {
                std::process::exit(9);
            }
        }
        let done = self
            .sync_files(renames.iter().map(|r| r.file.as_str()))
            .and_then(|()| std::fs::remove_file(&journal).map_err(|e| format!("the journal: {e}")))
            .and_then(|()| sync_dir(&state).map_err(|e| format!(".exact: {e}")));
        match done {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("contract tamber serve: {e}");
                self.complete_or_exit()
            }
        }
    }

    /// After a write stopped with its journal on disk: complete it now, or
    /// stop serving and leave the journal for the next start.
    fn complete_or_exit(&self) -> Result<(), Value> {
        match self.recover() {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!(
                    "contract tamber serve: a write stopped part way and could not be completed: {e}"
                );
                std::process::exit(1);
            }
        }
    }
}

/// Whether `name` is one of this server's temporary files in `.exact`.
fn temp_name(name: &str) -> bool {
    name.strip_prefix("tamber-")
        .and_then(|n| n.strip_suffix(".tmp"))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// A string field when the request has it; a field of another type is an
/// error, not absent.
fn optional(v: &Value, key: &str) -> Result<Option<String>, Value> {
    match &v[key] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(json!({"message": format!("`{key}` is a string")})),
    }
}

fn required(v: &Value, key: &str) -> Result<String, Value> {
    optional(v, key)?.ok_or_else(|| json!({"message": format!("`{key}` is a string, and needed")}))
}

/// A request's `files`, each path's digest as the client saw it.
fn seen_files(request: &Value) -> Result<HashMap<String, String>, Value> {
    let bad = || json!({"message": "`files` is an object of digests"});
    match &request["files"] {
        Value::Null => Ok(HashMap::new()),
        Value::Object(m) => m
            .iter()
            .map(|(k, v)| Ok((k.clone(), v.as_str().ok_or_else(bad)?.to_string())))
            .collect(),
        _ => Err(bad()),
    }
}

/// `{path: digest}`.
fn files_json(files: &[(String, String, String)]) -> Value {
    Value::Object(
        files
            .iter()
            .map(|(p, _, d)| (p.clone(), json!(d)))
            .collect(),
    )
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

/// The wire's operations (D13's names, kebab-case; named fields), each with
/// the file it edits (the design's own when unsaid). A `restore` is no
/// client's to send.
fn parse_ops(ops: &Value, design: &str) -> Result<Vec<(String, Op)>, Value> {
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
