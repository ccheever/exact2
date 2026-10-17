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
//!   of its own, fsynced; an intent record (each rename and its new text's
//!   digest) written beside and renamed to `.exact/tamber.journal`; each
//!   file read again, the record withdrawn and a conflict if one moved; the
//!   renames; the record removed; each directory flushed at each step. A
//!   write that fails part way is completed at once, or the server stops.
//!   An editor saving between that last read and its rename takes no lock
//!   and is overwritten; the digests narrow that window, they do not close
//!   it. Development builds only, on macOS and Linux.
//! - **At start** a record is completed: every rename judged first, then
//!   done when its temporary file has the recorded digest and the file the
//!   text it replaces, already done when the file has the new text. A set
//!   none of which landed, over a file changed since, is withdrawn. A
//!   record that names anything else, or cannot be read, is kept and the
//!   server refuses to start. Temporary files no record names are removed.
//! - **The inverse** a transaction returns is an id, unique to this run:
//!   `undo` with it applies the restores the server issued, only while each
//!   file it restores is as the write left it. The last 256 are kept. A
//!   client never sends a restore.
//! - **`propose`** plans without writing: the diff, the tokens it changes
//!   (by file), `live` or `rebuild`, and the digests it planned against; a
//!   digest it carries is checked as a transaction's is. `live` is the
//!   edit's closure (LLP 1121 D13, stage 4): no structure (a role override's
//!   value is not structure, stage 5), and the app's synthetic tokens (its rows' expressions over
//!   tokens, read from the plan the compiler builds) re-folded, those that
//!   change listed in `synthetic` with their new value per app tuple.
//!   A propose with `texts: true` also answers the texts it would write,
//!   by file, for a scrub's preview (stage 4, commit 6).
//! - **`requires`** (`tamber/1.1`), on a transaction or a propose: digests
//!   of files it does not edit but was planned on, checked with the rest.
//! - **Reads:** `hello`, `describe`, `resolve`, `explain`, `uses`, each of the
//!   design or of the `file` it names. `resolve`'s tuples carry `srgb
//!   {light, dark}`, each side's 8-bit clip, and `resolve` and `explain`
//!   the `files` digests they read (`tamber/1.1`); `describe` each token's
//!   declared `pairs` (a role's with its fallback) and the design's
//!   contrast `warnings`.
//! - **The wire's own** (LLP 1122 T1; `tamber/1.1`): `compare` and
//!   `revert-plan` read texts the wire took from git (`tamber_history.rs`),
//!   and `import-plan` plans an import from a source's text against the
//!   design (`tamber_import.rs`). None writes.
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
pub const PROTOCOL: &str = "tamber/1.10";

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
/// design file it becomes, that new text's digest, and the digest of the
/// text it replaces.
struct Rename {
    tmp: String,
    file: String,
    digest: String,
    before: String,
}

struct Server {
    /// The app's directory, canonical.
    dir: PathBuf,
    /// The lock file held, by device and inode: a `.exact` put in place of
    /// the one it is in is refused.
    lock_id: (u64, u64),
    /// The app's design, by path from the app's directory.
    design: String,
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
    /// The app's synthetic tokens (LLP 1121 D3), by the digest of the
    /// Contract sources they were read from, with the retention its plan
    /// keeps tokens by (D7), read by the same build.
    synthetic: Option<Synthetic>,
    /// The app's components' params (LLP 1121 D15), by the same key.
    params: (String, contract::tamber_params::AppParams),
}

/// A digest, its synthetic token expressions, and the plan's retention.
type Synthetic = (String, Vec<(String, bool)>, Option<exact_tamber::Retention>);

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

/// A file's device and inode.
fn identity(m: &std::fs::Metadata) -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (m.dev(), m.ino())
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        (0, 0)
    }
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
        if !cfg!(unix) {
            return Err(
                "the tamber server runs on macOS and Linux: its commit flushes directories".into(),
            );
        }
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
            Err(e) if e.kind() == ErrorKind::NotFound => std::fs::create_dir(&state)
                .and_then(|()| sync_dir(&dir))
                .map_err(|e| format!("{}: {e}", state.display()))?,
            Err(e) => return Err(format!("{}: {e}", state.display())),
        }
        // `.exact`'s entry durable, whichever start made it.
        sync_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let lock = no_follow(OpenOptions::new().create(true).truncate(false).write(true))
            .open(state.join("tamber.lock"))
            .map_err(|e| format!("the lock: {e}"))?;
        if lock.try_lock().is_err() {
            return Err(format!(
                "another tamber server writes {} (its lock is .exact/tamber.lock)",
                dir.display()
            ));
        }
        let started = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let lock_id = lock
            .metadata()
            .map(|m| identity(&m))
            .map_err(|e| format!("the lock: {e}"))?;
        let server = Server {
            lock_id,
            dir,
            design: rel,
            _lock: lock,
            vocab: contract_lower::tamber_vocab(),
            issued: HashMap::new(),
            order: VecDeque::new(),
            session: format!("{}-{started:x}", std::process::id()),
            next: 0,
            client: None,
            synthetic: None,
            params: Default::default(),
        };
        server.same_state()?;
        server.recover()?;
        // A journal's removal flushed before its temporaries go: unflushed,
        // the journal could come back without them.
        if sync_dir(&server.state()).is_ok() {
            server.sweep();
        }
        Ok(server)
    }

    /// The app's synthetic tokens: every expression over tokens its rows
    /// write, folding or not, as the compiler finds them (the plan keeps
    /// only those that fold). Read again when a Contract source, what
    /// resolved its imports, or the design's token names change, which is
    /// what decides them. None when the build stopped before the design:
    /// not kept, and tried again.
    fn synthetic(&mut self, snap: &Snapshot) -> Option<Vec<(String, bool)>> {
        let root = self.dir.join("app.contract");
        let graph = contract::source_graph(&root);
        let mut key = String::new();
        for s in &graph.sources {
            if s.path.extension().is_some_and(|e| e == "tamber") {
                continue;
            }
            key.push_str(&s.path.to_string_lossy());
            key.push('\0');
            key.push_str(
                &std::fs::read(&s.path)
                    .map(|b| digest(&b))
                    .unwrap_or_default(),
            );
            key.push('\n');
        }
        // Whether code beside the app can name a token outside its views
        // (D7's retention: app.ts, a data crate, app.json's modules or
        // hatches), which none of the sources say.
        key.push_str(&format!("bindable {}\n", contract::code_beside(&root)));
        // What resolved the imports (package manifests, the paths tried).
        for path in &graph.consulted {
            key.push_str(&path.to_string_lossy());
            key.push('\0');
            key.push_str(&std::fs::read(path).map(|b| digest(&b)).unwrap_or_default());
            key.push('\n');
        }
        if let Some(design) = snap
            .iter()
            .find(|(f, _, _)| *f == self.design)
            .and_then(|(_, t, _)| exact_tamber::parse(t).ok())
        {
            for t in &design.tokens {
                key.push_str(&t.name.value);
                key.push('\n');
            }
        }
        if self.params.0 != key {
            self.params = (key.clone(), contract::tamber_params::params_of(&root));
        }
        if let Some((k, exprs, _)) = &self.synthetic {
            if *k == key {
                return Some(exprs.clone());
            }
        }
        let _ = contract_lower::synthetic_candidates();
        let _ = contract_lower::retention();
        let built = contract::compile_path_all(&root, false);
        let exprs = contract_lower::synthetic_candidates();
        let retention = contract_lower::retention();
        // A build that stopped before the design says nothing of them.
        let reached = built.is_ok() || !exprs.is_empty();
        reached.then(|| {
            self.synthetic = Some((key, exprs.clone(), retention));
            exprs
        })
    }

    fn state(&self) -> PathBuf {
        self.dir.join(".exact")
    }

    /// Whether `.exact` is still the directory the held lock is in.
    fn same_state(&self) -> Result<(), String> {
        let state = std::fs::symlink_metadata(self.state());
        let lock = std::fs::symlink_metadata(self.state().join("tamber.lock"));
        match (state, lock) {
            (Ok(d), Ok(l)) if d.is_dir() && l.is_file() && identity(&l) == self.lock_id => Ok(()),
            _ => Err(".exact is not the directory this server locked; restart it".into()),
        }
    }

    /// Complete a transaction a crash left between its renames. Each rename
    /// is done when its temporary file is there with the recorded digest and
    /// the file still has the text it replaces, or already done when the
    /// file has the new text. A set none of which landed, with a file moved
    /// since it was planned, is withdrawn, as its write would have been;
    /// anything else keeps the journal and refuses, for a person to look at.
    fn recover(&self) -> Result<(), String> {
        let journal = self.state().join("tamber.journal");
        let Some(bytes) = read_plain(&journal).map_err(|e| format!("the journal: {e}"))? else {
            return Ok(());
        };
        let renames = self
            .journal(&bytes)
            .map_err(|e| format!("the journal .exact/tamber.journal: {e}; it is kept"))?;
        // The record durable before any file moves on its word.
        sync_dir(&self.state()).map_err(|e| format!(".exact: {e}; the journal is kept"))?;
        // Every rename judged before any is done: a set that cannot complete
        // is left as it is.
        let (mut pending, mut moved, mut landed) = (Vec::new(), Vec::new(), false);
        for r in &renames {
            let tmp = self.state().join(&r.tmp);
            match read_plain(&tmp)? {
                Some(b) if digest(&b) == r.digest => {
                    let now = read_plain(&self.dir.join(&r.file))?.map(|b| digest(&b));
                    match now {
                        Some(d) if d == r.before || d == r.digest => pending.push((tmp, r)),
                        _ => moved.push(r.file.as_str()),
                    }
                }
                Some(_) => {
                    return Err(format!(
                        "{} is not the text the journal records for {}; the journal is kept",
                        r.tmp, r.file
                    ))
                }
                None => {
                    if read_plain(&self.dir.join(&r.file))?.is_none_or(|b| digest(&b) != r.digest) {
                        return Err(format!(
                            "{} has neither its new text nor its temporary file; the journal is kept",
                            r.file
                        ));
                    }
                    landed = true;
                }
            }
        }
        if !moved.is_empty() {
            if landed {
                return Err(format!(
                    "{} changed since the journal's write was planned, and part of it landed; the journal is kept",
                    moved.join(", ")
                ));
            }
            eprintln!(
                "contract tamber serve: {} changed since the journal's write was planned, and none of it landed: withdrawn",
                moved.join(", ")
            );
            std::fs::remove_file(&journal).map_err(|e| format!("the journal: {e}"))?;
            return sync_dir(&self.state()).map_err(|e| format!(".exact: {e}"));
        }
        for (tmp, r) in pending {
            std::fs::rename(&tmp, self.dir.join(&r.file))
                .map_err(|e| format!("completing {}: {e}; the journal is kept", r.file))?;
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
                before: field(v, "before")?,
            };
            if !temp_name(&r.tmp) {
                return Err(format!("`{}` is no temporary file of this server", r.tmp));
            }
            if !self.files()?.contains(&r.file) {
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

    /// The design's files: the app's design first, then any other design
    /// beside it, as the directory has them now (a transaction may edit
    /// several, committed as one set).
    fn files(&self) -> Result<Vec<String>, String> {
        let mut others: Vec<String> = std::fs::read_dir(&self.dir)
            .map_err(|e| format!("{}: {e}", self.dir.display()))?
            .filter_map(|e| {
                let e = e.ok()?;
                e.file_type().ok()?.is_file().then_some(())?;
                e.file_name().into_string().ok()
            })
            .filter(|n| n.ends_with(".tamber") && *n != self.design)
            .collect();
        others.sort();
        Ok(std::iter::once(self.design.clone()).chain(others).collect())
    }

    /// Every design file, read once: the text a plan edits is the text its
    /// digest is of.
    fn snapshot(&self) -> Result<Snapshot, Value> {
        self.read_files(false)
    }

    /// The files read; with `absent`, a file that is not there (the design
    /// a checkout removed) is left out rather than an error.
    fn read_files(&self, absent: bool) -> Result<Snapshot, Value> {
        let mut out = Vec::new();
        for f in self.files().map_err(|m| json!({"message": m}))? {
            let bytes = match read_plain(&self.dir.join(&f)).map_err(|m| json!({"message": m}))? {
                Some(b) => b,
                None if absent => continue,
                None => return Err(json!({"message": format!("{f} is gone")})),
            };
            let d = digest(&bytes);
            let text = String::from_utf8(bytes)
                .map_err(|_| json!({"message": format!("{f} is not UTF-8")}))?;
            out.push((f, text, d));
        }
        Ok(out)
    }

    /// Each design file there now, its path and text.
    fn texts(&self) -> Result<Vec<(String, String)>, Value> {
        Ok(self
            .read_files(true)?
            .into_iter()
            .map(|(f, t, _)| (f, t))
            .collect())
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
            Some(m @ ("resolve" | "explain" | "uses")) => {
                let answer = match m {
                    "resolve" => self.resolve(&request),
                    "explain" => self.explain(&request),
                    _ => self.uses(&request),
                };
                vec![reply(
                    answer.map(|v| self.present(v)).map_err(|e| self.present(e)),
                )]
            }
            Some("propose") => vec![reply(self.transaction(&request, false))],
            Some("transaction") => self.commit_reply(&request, reply, false),
            Some("undo") => self.commit_reply(&request, reply, true),
            // The wire's own, for `history`, `revert` and `import`.
            Some("compare") => vec![reply(crate::tamber_history::compare(&request))],
            Some("revert-plan") => {
                vec![reply(self.read_files(true).and_then(|now| {
                    crate::tamber_history::revert_plan(&request, &now)
                }))]
            }
            Some("import-plan") => vec![reply(self.texts().and_then(|now| {
                let design = now.iter().find(|(f, _)| *f == self.design).map(|(_, t)| t);
                let kept = crate::tamber_export::kept(&self.dir.join(&self.design));
                crate::tamber_import::plan(&request, design.map_or("tamber app\n", |t| t), &kept)
            }))],
            Some("export") => vec![reply(self.export(&request))],
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
        // The design's files and digests, whether or not they check: a
        // client's (and the wire's watcher's) baseline. A file not there is
        // left out; an unreadable one is an error.
        let files = files_json(&self.read_files(true)?);
        Ok(
            json!({"protocol": PROTOCOL, "format": exact_tamber::FORMAT, "builds": [], "files": files}),
        )
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

    /// The file a read names (`file`, the app's design when unsaid),
    /// checked, from one snapshot that also gives the digests.
    /// The design folded with the app's params (D15), each in its
    /// `params` line's place, for a request's `token`, and the name to look
    /// up: a param's token where it names one in full or short, else the
    /// name as given (a design token, which may read a param). A short name
    /// two components share is refused, not read as a design token.
    fn lookup(&self, request: &Value) -> Result<(Snapshot, exact_tamber::Folding, String), Value> {
        let name = required(request, "token")?;
        let app = contract::tamber_params::params_of(&self.dir.join("app.contract"));
        let token = match contract_lower::params::resolve(&app.tokens, &app.identities, &name) {
            Ok(p) => p.token.clone(),
            Err(e) if e.id == "tamber-param-ambiguous" => {
                return Err(json!({"message": format!("{}: {}", e.id, e.message), "id": e.id}));
            }
            Err(_) => name,
        };
        let (snap, folding, _) = self.design_with(request, &app)?;
        Ok((snap, folding, token))
    }

    /// [`Self::design`], folded with the app's components' params (D15),
    /// and the design as parsed, without them.
    fn design_with(
        &self,
        request: &Value,
        app: &contract::tamber_params::AppParams,
    ) -> Result<(Snapshot, exact_tamber::Folding, exact_tamber::Design), Value> {
        let params = app.tokens.as_slice();
        if params.is_empty() {
            let (snap, folding) = self.design(request)?;
            let design = folding.design.clone();
            return Ok((snap, folding, design));
        }
        let snap = self.snapshot()?;
        let file = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        let text = &snap
            .iter()
            .find(|(f, _, _)| *f == file)
            .ok_or_else(|| json!({"message": format!("`{file}` is no file of this design")}))?
            .1;
        let (folding, design) = contract::tamber_params::fold_with(text, app, &self.vocab)
            .map_err(|all| {
                json!({"message": "the design is refused", "diagnostics": all.iter().map(|d| located(&file, d, app)).collect::<Vec<_>>()})
            })?;
        Ok((snap, folding, design))
    }

    /// A reply with each param named by its full name (D15), never by its
    /// name below the IR.
    fn present(&self, v: Value) -> Value {
        let all = contract::tamber_params::params_of(&self.dir.join("app.contract")).tokens;
        if all.is_empty() {
            return v;
        }
        let text = contract_lower::params::shown(&v.to_string(), &all);
        serde_json::from_str(&text).unwrap_or(v)
    }

    fn design(&self, request: &Value) -> Result<(Snapshot, exact_tamber::Folding), Value> {
        let file = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        let snap = self.snapshot()?;
        let text = &snap
            .iter()
            .find(|(f, _, _)| *f == file)
            .ok_or_else(|| json!({"message": format!("`{file}` is no file of this design")}))?
            .1;
        let folding = exact_tamber::check(text, &self.vocab).map_err(|all| refused(&file, all))?;
        Ok((snap, folding))
    }

    /// `export {kind: "dtcg"}` (*1.4*, LLP 1121 D14): the app's design as a
    /// DTCG export, its text, kept as the baseline a re-import compares with.
    fn export(&self, request: &Value) -> Result<Value, Value> {
        let kind = optional(request, "kind")?.unwrap_or_default();
        if kind != "dtcg" {
            return Err(json!({"message": format!("no exporter `{kind}` (dtcg)")}));
        }
        let (_, folding) = self.design(&json!({}))?;
        let text = exact_tamber::export::dtcg(&folding, &self.vocab)
            .map_err(|all| refused(&self.design, all))?;
        let kept = crate::tamber_export::keep(&self.dir.join(&self.design), &text)
            .map_err(|e| json!({"message": format!("the baseline: {e}")}))?;
        Ok(json!({
            "kind": "dtcg",
            "text": text,
            "baseline": kept.strip_prefix(&self.dir).unwrap_or(&kept).to_string_lossy(),
        }))
    }

    fn describe(&self, request: &Value) -> Result<Value, Value> {
        // The app's components' params (LLP 1121 D15) fold with the design,
        // so a dial only a param reads is read; listed by full name.
        let app = contract::tamber_params::params_of(&self.dir.join("app.contract"));
        let all = app.tokens.as_slice();
        let (snap, folding, design) = self.design_with(request, &app)?;
        let mut ir: Value =
            serde_json::from_str(&folding.ir()).map_err(|e| json!({"message": e.to_string()}))?;
        if !all.is_empty() {
            contract::tamber_params::hide(&mut ir);
            ir["components"] =
                contract::tamber_params::components(&app, Some(&design), &self.vocab);
        }
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
            .filter(|(t, _)| !t.name.starts_with(contract_lower::params::PREFIX))
            .map(|(t, declared)| (t.name.clone(), json!(meta(&declared.meta))))
            .collect();
        // What each token is drawn on (`pair=`): a token by its name, a role
        // no token overrides with its fallback; and the contrast warnings.
        let pairs: serde_json::Map<String, Value> = folding
            .tokens
            .iter()
            .filter(|t| !t.pairs.is_empty() && !t.name.starts_with(contract_lower::params::PREFIX))
            .map(|t| {
                let named = t.pairs.iter().map(|p| {
                    let token = folding.tokens.iter().any(|u| &u.name == p);
                    let fallback = (!token)
                        .then(|| exact_tamber::fallback(&exact_tamber::Folded::Host(exact_tamber::Host::Role(p.clone())), &self.vocab))
                        .flatten();
                    json!({"name": p, "token": token, "fallback": fallback.map(|[l, d]| json!({"light": l, "dark": d}))})
                });
                (t.name.clone(), json!(named.collect::<Vec<_>>()))
            })
            .collect();
        let file = optional(request, "file")?.unwrap_or_else(|| self.design.clone());
        let warnings: Vec<Value> = folding
            .warnings
            .iter()
            .map(|d| located(&file, d, &app))
            .collect();
        Ok(
            json!({"ir": ir, "files": files_json(&snap), "meta": {"dials": dials, "tokens": tokens}, "pairs": pairs, "warnings": warnings}),
        )
    }

    /// A token's stored value per tuple and scheme; a host value (a role, a
    /// platform colour) is a residual, labelled so, with its fallback.
    fn resolve(&self, request: &Value) -> Result<Value, Value> {
        let (snap, folding, name) = self.lookup(request)?;
        // An icon (D10): its sources, the same in every tuple.
        if let Some(icon) = folding.icons.iter().find(|i| i.name == name) {
            let tuples: Vec<Value> = (0..folding.tuples)
                .map(|t| json!({"modes": modes_of(&folding.design.axes, t), "kind": "icon", "sf": icon.sf, "material": icon.material, "svg": icon.svg}))
                .collect();
            return Ok(json!({"token": name, "tuples": tuples, "files": files_json(&snap)}));
        }
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
                let v = match v {
                    // A length's stored form: px, or the unit or named
                    // length it keeps, whose px is the build's (its text
                    // size, `resolve-on`).
                    exact_tamber::Value::Length(l) => {
                        let (unit, amount) = match l {
                            exact_tamber::Length::Px(n) => ("px", json!(n)),
                            exact_tamber::Length::Rem(n) => ("rem", json!(n)),
                            exact_tamber::Length::Em(n) => ("em", json!(n)),
                            exact_tamber::Length::Named(_) => ("named", Value::Null),
                        };
                        return json!({"modes": modes, "value": v.css(), "kind": "length", "unit": unit, "amount": amount});
                    }
                    exact_tamber::Value::Number(n) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "number", "amount": n});
                    }
                    // Each shadow: its lengths in px and its colour's CSS
                    // (null for a text shadow's currentcolor).
                    exact_tamber::Value::Shadow(s) => {
                        let shadows: Vec<Value> = s
                            .list
                            .iter()
                            .map(|one| json!({"inset": one.inset, "lengths": one.lengths, "color": one.color.as_ref().map(|c| c.css())}))
                            .collect();
                        return json!({"modes": modes, "value": v.css(), "kind": if s.text { "text-shadow" } else { "box-shadow" }, "shadows": shadows});
                    }
                    // A motion token: a time in ms, or an easing's text.
                    exact_tamber::Value::Duration(ms) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "duration", "ms": ms});
                    }
                    exact_tamber::Value::Easing(_) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "easing"});
                    }
                    // A type token's longhands as written (null: unsaid), a
                    // family's stack (stage 6 C, optional fields under 1.9).
                    exact_tamber::Value::Type(t) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "type", "style": t.style, "weight": t.weight, "size": t.size.css(), "leading": t.leading, "family": t.family});
                    }
                    exact_tamber::Value::Family(f) => {
                        return json!({"modes": modes, "value": v.css(), "kind": "family", "family": f});
                    }
                    exact_tamber::Value::Color(c) => c,
                };
                match v {
                    // `srgb`: each side's 8-bit sRGB clip, what a contrast
                    // ratio is taken on (`light` and `dark` keep the wide value).
                    exact_tamber::Folded::One(h) => {
                        json!({"modes": modes, "value": v.css(), "light": h.css(), "dark": h.css(), "srgb": {"light": h.clip(), "dark": h.clip()}})
                    }
                    exact_tamber::Folded::Pair(l, d) => {
                        json!({"modes": modes, "value": v.css(), "light": l.css(), "dark": d.css(), "srgb": {"light": l.clip(), "dark": d.clip()}})
                    }
                    exact_tamber::Folded::Host(_) => {
                        let fallback = exact_tamber::fallback(v, &self.vocab)
                            .map(|[l, d]| json!({"light": l, "dark": d}));
                        json!({"modes": modes, "value": v.css(), "residual": true, "fallback": fallback})
                    }
                }
            })
            .collect();
        // The digests it read, so a client reading several tokens can tell
        // one revision from two.
        Ok(json!({"token": name, "tuples": tuples, "files": files_json(&snap)}))
    }

    /// A token's chain: its default and `when` values, the dials it depends
    /// on, and the tokens those values name, as the checker resolved them.
    fn explain(&self, request: &Value) -> Result<Value, Value> {
        let (snap, folding, name) = self.lookup(request)?;
        let (token, declared) = folding
            .tokens
            .iter()
            .zip(&folding.design.tokens)
            .find(|(t, _)| t.name == name)
            .ok_or_else(|| no_token(&folding, &name))?;
        let whens: Vec<Value> = declared
            .whens
            .iter()
            .map(|w| {
                json!({"when": w.conds.iter().map(|(a, v)| json!({"axis": a, "value": v})).collect::<Vec<_>>(), "value": w.value.value})
            })
            .collect();
        Ok(
            json!({"token": name, "value": declared.value.value, "whens": whens, "dials": token.dials, "names": token.names, "files": files_json(&snap)}),
        )
    }

    /// Where the app reads a token (`crate::tamber_uses`): each plan site
    /// whose style rows name it, directly, through a token naming it or
    /// through a synthetic token, with its source span. `site` indexes the
    /// plan compiled here (`plan`, its digest, before any bake); a build
    /// joins the spans through its own source map (LLP 1122 T2b's
    /// `outline`).
    fn uses(&mut self, request: &Value) -> Result<Value, Value> {
        let (snap, folding, name) = self.lookup(request)?;
        if !folding.tokens.iter().any(|t| t.name == name) {
            return Err(no_token(&folding, &name));
        }
        // A design file the app does not build has no sites.
        if optional(request, "file")?.is_some_and(|f| f != self.design) {
            return Ok(
                json!({"token": name, "plan": null, "sites": [], "aliases": [], "files": files_json(&snap)}),
            );
        }
        let aliases = contract::tamber_uses::aliases(
            &name,
            folding
                .tokens
                .iter()
                .map(|t| (t.name.as_str(), t.names.as_slice())),
        );
        let synthetic: Vec<String> = self
            .synthetic(&snap)
            .unwrap_or_default()
            .into_iter()
            .map(|(e, _)| e)
            .collect();
        // A motion token is written into its rows: its sites are the source
        // literals the loader wrote it into (`motion_sites`), not plan nodes.
        let motion = folding.tokens.iter().any(|t| {
            t.name == name
                && matches!(
                    t.kind,
                    exact_tamber::Kind::Duration | exact_tamber::Kind::Easing
                )
        });
        if motion {
            let aliased: Vec<String> = aliases.iter().cloned().collect();
            let sites = contract::tamber_uses::motion_sites(
                &self.dir.join("app.contract"),
                &name,
                &aliased,
            )
            .map_err(|e| json!({"message": format!("the app does not build: {e}")}))?;
            return Ok(
                json!({"token": name, "plan": null, "sites": sites, "aliases": aliases, "files": files_json(&snap)}),
            );
        }
        let (plan, map) = contract::compile_path_mapped(&self.dir.join("app.contract"))
            .map_err(|e| json!({"message": format!("the app does not build: {}", e.message)}))?;
        let map: Value = serde_json::from_str(&map.json(&plan.encode()))
            .map_err(|e| json!({"message": format!("source map: {e}")}))?;
        let sites = contract::tamber_uses::sites(&plan, &map, &name, &aliases, &synthetic);
        Ok(
            json!({"token": name, "plan": map["digest"], "sites": sites, "aliases": aliases, "files": files_json(&snap)}),
        )
    }

    /// Plan, and when `write`, commit: the digests, the inverse's id, the
    /// tokens it changes, `live` or `rebuild`, the diff. Each file it edits
    /// must carry the digest the client saw (a `propose` checks the ones it
    /// carries).
    fn transaction(&mut self, request: &Value, write: bool) -> Result<Value, Value> {
        let mut ops = crate::tamber_ops::parse_ops(&request["ops"], &self.design)?;
        let seen = seen_files(request, "files")?;
        // Files it does not edit but was planned on (a revert's file already
        // as before), each as the plan read it.
        let requires = seen_files(request, "requires")?;
        let client = match write {
            true => Some(self.writer(request)?),
            false => None,
        };
        let snap = self.snapshot()?;
        let stale: Vec<&String> = snap
            .iter()
            .filter(|(p, _, d)| {
                ops.iter().any(|(f, _)| f == p)
                    && match seen.get(p) {
                        Some(s) => s != d,
                        None => write,
                    }
            })
            .map(|(p, _, _)| p)
            .chain(requires.keys().filter(|p| {
                snap.iter()
                    .find(|(f, _, _)| f == *p)
                    .is_none_or(|(_, _, d)| Some(d) != requires.get(*p))
            }))
            .collect();
        if !stale.is_empty() {
            return Err(
                json!({"conflict": true, "message": "a file changed since the client read it", "files": files_json(&snap), "stale": stale}),
            );
        }
        // A param set or unset by a name the app has (D15), on the line the
        // design already gives it, whichever name that line uses.
        let _ = self.synthetic(&snap);
        crate::tamber_ops::param_names(&mut ops, &snap, &self.params.1)?;
        // A scrub's preview (the wire's, LLP 1121 D13) compiles the texts
        // a propose would write, which it asks for.
        let texts = !write && request["texts"] == json!(true);
        self.run(&snap, &ops, client, false, texts)
    }

    /// Apply an inverse this server issued, while each file it restores is
    /// as the write left it.
    fn undo(&mut self, request: &Value) -> Result<Value, Value> {
        let client = self.writer(request)?;
        let id = required(request, "inverse")?;
        let issued = self.issued.get(&id).cloned().ok_or_else(|| {
            json!({"message": format!("no inverse `{id}`: this server issued none by that id, or it has dropped it (it keeps the last {ISSUED_LIMIT})")})
        })?;
        // A file it restores that is gone is stale, not an error.
        let snap = self.read_files(true)?;
        let stale: Vec<&String> = issued
            .after
            .iter()
            .filter(|(p, d)| {
                snap.iter()
                    .find(|(q, _, _)| q == p)
                    .is_none_or(|(_, _, e)| e != d)
            })
            .map(|(p, _)| p)
            .collect();
        if !stale.is_empty() {
            return Err(
                json!({"conflict": true, "message": "a file changed since that write: its inverse no longer applies", "files": files_json(&snap), "stale": stale}),
            );
        }
        let result = self.run(&snap, &issued.ops, Some(client), true, false);
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
        with_texts: bool,
    ) -> Result<Value, Value> {
        let texts: Vec<(String, String)> = snap
            .iter()
            .map(|(f, t, _)| (f.clone(), t.clone()))
            .collect();
        let known = self.synthetic(snap);
        let exprs = known.clone().unwrap_or_default();
        // The tokens the plan keeps (D7): an edit to one it does not keep
        // (a plain app's token no view names) leaves the plan as it is.
        let retention = self.synthetic.as_ref().and_then(|(_, _, r)| r.clone());
        let params = crate::tamber_ops::params(&self.params.1);
        let synthetic = exact_tamber::Synthetic {
            file: &self.design,
            exprs: &exprs,
            retention: retention.as_ref(),
            params: &params,
        };
        let planned = exact_tamber::plan(&texts, ops, &self.vocab, undo, synthetic).map_err(|all| {
            json!({"message": "refused", "diagnostics": all.iter().map(|(f, d)| located(f, d, &self.params.1)).collect::<Vec<_>>()})
        })?;
        // What the build refuses beyond the crate's check, a write refuses
        // too, so nothing is written that the app cannot build: a role
        // override that varies by an app axis (LLP 1121 D5, the compiler's
        // `tamber-role-axis`). An undo restores what was, and is not judged.
        if !undo {
            // A role that varied before this write is the file's own,
            // already refused by the build; only what the write makes vary.
            let varies = |text: &str| -> Vec<(String, u32, u32)> {
                exact_tamber::check(text, &self.vocab).map_or(Vec::new(), |f| {
                    f.tokens
                        .iter()
                        .zip(&f.design.tokens)
                        .filter(|(t, _)| {
                            t.role.is_some()
                                && t.values.windows(2).any(|w| w[0].css() != w[1].css())
                        })
                        .map(|(t, d)| (t.name.clone(), d.name.line, d.name.col))
                        .collect()
                })
            };
            let mut axis = Vec::new();
            for (path, text, _) in &planned.files {
                let before: Vec<String> = texts
                    .iter()
                    .find(|(p, _)| p == path)
                    .map_or(Vec::new(), |(_, t)| {
                        varies(t).into_iter().map(|(n, ..)| n).collect()
                    });
                for (name, line, col) in varies(text) {
                    if !before.contains(&name) {
                        axis.push(diagnostic(path, &exact_tamber::Diagnostic { id: "tamber-role-axis", message: format!("`{name}` overrides a role and varies by an app axis: a role has one value, which the hosts' own surfaces read; vary a token the rows name instead (LLP 1121 D5)"), line, col }));
                    }
                }
            }
            if !axis.is_empty() {
                return Err(json!({"message": "refused", "diagnostics": axis}));
            }
            // A param's value folds with the params where it lands, inside
            // its range (D15), as the build checks it.
            // Any write: an unset, a token's or an axis's edit can take a
            // param out of its range or its `when` out of the design too.
            if !self.params.1.tokens.is_empty() {
                let set: Vec<(String, String)> = planned
                    .files
                    .iter()
                    .filter(|(p, _, _)| *p == self.design)
                    .map(|(p, t, _)| (p.clone(), t.clone()))
                    .collect();
                crate::tamber_ops::param_values(&set, &self.params.1, &self.vocab)?;
            }
        }
        let mut out = json!({
            "files": files_json(snap),
            "changed": planned.changed.iter().map(|(f, t)| json!({"file": f, "token": t})).collect::<Vec<_>>(),
            // The closure unknown (the app's build stops before its design):
            // no edit is answered live.
            "class": if known.is_some() { planned.class.name() } else { exact_tamber::Class::Rebuild.name() },
            "diff": planned.diff,
            "synthetic": planned.synthetic.iter().map(|(expr, values)| json!({"expr": expr, "values": values.iter().map(|v| v.css()).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        });
        if with_texts {
            out["texts"] = json!(planned
                .files
                .iter()
                .map(|(p, text, _)| (p.clone(), json!(text)))
                .collect::<serde_json::Map<_, _>>());
        }
        let Some(client) = client else {
            return Ok(out);
        };
        out["client"] = json!(client);
        // The files this write wrote, and each one's digest as it read it;
        // the rest of `files` is as it read them.
        out["written"] = json!(planned.files.iter().map(|(p, _, _)| p).collect::<Vec<_>>());
        out["before"] = files_json(
            &snap
                .iter()
                .filter(|(f, _, _)| planned.files.iter().any(|(p, _, _)| p == f))
                .cloned()
                .collect::<Vec<_>>(),
        );
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
    /// fsynced; the intent record written beside and renamed into place;
    /// each file read again, and the record withdrawn if one moved since
    /// the snapshot; the renames; the record removed; each step's directory
    /// flushed.
    fn commit(
        &mut self,
        files: &[(String, String, Vec<Op>)],
        expected: &[(String, String)],
    ) -> Result<(), Value> {
        let fail = |m: String| json!({"message": m});
        let state = self.state();
        self.same_state().map_err(fail)?;
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
                before: expected
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, d)| d.clone())
                    .unwrap_or_default(),
            });
        }
        // The temporaries' names durable before a record names them.
        if let Err(e) = sync_dir(&state) {
            clean(&renames);
            return Err(fail(format!(".exact: {e}")));
        }
        let record = json!({"renames": renames.iter().map(|r| json!({"tmp": r.tmp, "file": r.file, "digest": r.digest, "before": r.before})).collect::<Vec<_>>()});
        let staged = state.join("tamber.journal.tmp");
        let published = no_follow(OpenOptions::new().write(true).create_new(true))
            .open(&staged)
            .and_then(|mut j| {
                j.write_all(record.to_string().as_bytes())?;
                j.sync_all()
            })
            .and_then(|()| std::fs::rename(&staged, &journal));
        if let Err(e) = published {
            let _ = std::fs::remove_file(&staged);
            match read_plain(&journal) {
                Ok(None) => {
                    clean(&renames);
                    return Err(fail(format!("the journal: {e}")));
                }
                // In place, or not known: the record and its temporaries
                // stay for the next start.
                _ => self.stop(format!("the journal: {e}")),
            }
        }
        // No file moves on the record's word until the record is durable.
        if let Err(e) = sync_dir(&state) {
            self.stop(format!(".exact, after its journal: {e}"));
        }
        // Another writer (an editor, `tamber fmt`) since the snapshot, as
        // late as this protocol can look: such a writer takes no lock, so a
        // save between this read and the rename is still overwritten.
        for (path, d) in expected {
            let moved = match read_plain(&self.dir.join(path)) {
                Ok(now) => now.is_none_or(|b| digest(&b) != *d),
                Err(_) => true,
            };
            if moved {
                // No file has moved: the record is withdrawn.
                if let Err(e) = std::fs::remove_file(&journal).and_then(|()| sync_dir(&state)) {
                    self.stop(format!("withdrawing the journal: {e}"));
                }
                clean(&renames);
                let files = self
                    .snapshot()
                    .map(|s| files_json(&s))
                    .unwrap_or(Value::Null);
                return Err(
                    json!({"conflict": true, "message": format!("{path} changed while the write was planned"), "files": files, "stale": [path]}),
                );
            }
        }
        for (n, r) in renames.iter().enumerate() {
            if let Err(e) = std::fs::rename(state.join(&r.tmp), self.dir.join(&r.file)) {
                eprintln!("contract tamber serve: {}: {e}", r.file);
                return self.complete_or_exit(&renames);
            }
            // A test's crash between renames (the gate: a restart completes them).
            if n == 0 && std::env::var_os("EXACT_TAMBER_CRASH_AFTER_RENAME").is_some() {
                std::process::exit(9);
            }
        }
        // While the record stands, recovery flushes and removes it.
        let done = self
            .sync_files(renames.iter().map(|r| r.file.as_str()))
            .and_then(|()| std::fs::remove_file(&journal).map_err(|e| format!("the journal: {e}")));
        if let Err(e) = done {
            eprintln!("contract tamber serve: {e}");
            return self.complete_or_exit(&renames);
        }
        // A removed record that came back would be replayed over later
        // writes: its removal is flushed, or the server stops.
        if let Err(e) = sync_dir(&state).or_else(|_| sync_dir(&state)) {
            self.stop(format!(".exact, after removing its journal: {e}"));
        }
        Ok(())
    }

    /// Stop serving: the state on disk is for the next start to judge.
    fn stop(&self, message: String) -> ! {
        eprintln!("contract tamber serve: {message}; stopping");
        std::process::exit(1);
    }

    /// After a write stopped with its journal on disk: complete it now, or
    /// stop serving and leave the journal for the next start.
    fn complete_or_exit(&self, renames: &[Rename]) -> Result<(), Value> {
        // Done only when each file has its new text: a journal gone from
        // view is not by itself a write completed.
        let landed = || {
            renames.iter().all(|r| {
                read_plain(&self.dir.join(&r.file))
                    .ok()
                    .flatten()
                    .is_some_and(|b| digest(&b) == r.digest)
            })
        };
        match self.recover() {
            Ok(()) if landed() => Ok(()),
            Ok(()) => {
                self.stop("a write stopped part way, and its files do not have its new text".into())
            }
            Err(e) => self.stop(format!(
                "a write stopped part way and could not be completed: {e}"
            )),
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
fn seen_files(request: &Value, key: &str) -> Result<HashMap<String, String>, Value> {
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
fn files_json(files: &[(String, String, String)]) -> Value {
    Value::Object(
        files
            .iter()
            .map(|(p, _, d)| (p.clone(), json!(d)))
            .collect(),
    )
}

/// Why `explain` or `uses` has no token `name`: an icon (a param's too) is
/// read through `describe` and `resolve` (D10).
fn no_token(folding: &exact_tamber::Folding, name: &str) -> Value {
    match folding.icons.iter().any(|i| i.name == name) {
        true => {
            json!({"message": format!("`{name}` is an icon: `describe` and `resolve` read an icon's sources; `explain` and `uses` take a token")})
        }
        false => json!({"message": format!("no token `{name}`")}),
    }
}

fn diagnostic(file: &str, d: &exact_tamber::Diagnostic) -> Value {
    json!({"file": file, "line": d.line, "col": d.col, "id": d.id, "message": d.message})
}

/// [`diagnostic`], one of a param's (D15) in its declaring file, at its
/// default, every name in full.
fn located(
    file: &str,
    d: &exact_tamber::Diagnostic,
    app: &contract::tamber_params::AppParams,
) -> Value {
    let mut d = d.clone();
    let declaring = contract::tamber_params::relocate(&mut d, app);
    diagnostic(declaring.as_deref().unwrap_or(file), &d)
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
