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
//! - **The wire's own** (LLP 1122 T1, *1.1*): `compare`, `revert-plan` (texts from git); `import-plan`
//!   marks a held re-import (sources are read alone). None writes. Base names, *1.14*: `tamber_base.rs`.
//! - **A request** is one JSON object on a line of at most 1 MiB; a field
//!   of the wrong type is an error, not absent.

use crate::tamber_ops::{files_json, optional, required, seen_files};
use exact_tamber::Op;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
#[path = "tamber_answers.rs"]
mod answers;
#[path = "tamber_base.rs"]
mod base;
#[path = "tamber_check.rs"]
mod check;
#[path = "tamber_scrub.rs"]
mod scrub;
#[path = "tamber_walk.rs"]
mod walk;
use answers::{diagnostic, located, modes_of, no_token, refused, temp_name};

/// The protocol this server speaks (D13: minor versions only add).
pub const PROTOCOL: &str = "tamber/1.18";

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
    /// The last retention build's types and expansion, by the Contract
    /// sources' key: a build the design alone moved reuses them where the
    /// program loads as it did (`contract::retention_build`).
    kept: Option<(String, contract::Kept)>,
    /// The style rows the last retention build found literal, with each
    /// source's path, by the Contract sources' key: what a write's rows
    /// check judges again (`contract_lower::rows`).
    rows: Option<(String, Vec<contract_lower::rows::Row>, Vec<PathBuf>)>,
    /// The background whole build, which says when a retention build's
    /// build fails after the design (theme2's 12:15 ruling).
    checker: check::Checker,
    /// The retention key the last answer was made under (a scrub's frames
    /// ask the check under it).
    last_key: String,
    /// The app's components' params (LLP 1121 D15), by the Contract
    /// sources' key ([`Server::sources_key`]): every read and write asks
    /// them, and loading them parses the app (~200 ms on the clone).
    params: std::cell::RefCell<(String, std::rc::Rc<contract::tamber_params::AppParams>)>,
    /// The exported library components' slot chains (LLP 1139 D4, stage 12
    /// D2), built from the params above when they are loaded again, never
    /// per write: what a write's candidate fold is checked against.
    #[allow(clippy::type_complexity)]
    slots: std::cell::RefCell<
        Option<(
            std::rc::Rc<contract::tamber_params::AppParams>,
            Option<std::rc::Rc<contract_lower::anatomy::slot::SlotChains>>,
        )>,
    >,
    /// The Contract sources as last walked ([`walk::Walk`]): every snapshot and
    /// every request's params ask them, and walking them parses the app
    /// (~200 ms on the clone).
    graph: std::cell::RefCell<Option<walk::Walk>>,
    /// Requests handled: a walk is checked once per request.
    asked: std::cell::Cell<u64>,
    /// The app's own Contract sources at the last read that loaded them
    /// all: a root that stops parsing for a moment keeps its imports.
    contract: std::cell::RefCell<Vec<String>>,
    /// What an open scrub's frames fold over (`tamber_scrub.rs`).
    scrub: Option<scrub::Base>,
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
            kept: None,
            rows: None,
            checker: check::Checker::default(),
            last_key: String::new(),
            params: Default::default(),
            slots: Default::default(),
            graph: Default::default(),
            asked: Default::default(),
            contract: Default::default(),
            scrub: None,
        };
        server.same_state()?;
        server.recover()?;
        // A journal's removal flushed before its temporaries go: unflushed, the journal could come back without them.
        if sync_dir(&server.state()).is_ok() {
            server.sweep();
        }
        Ok(server)
    }

    /// The app's synthetic tokens: every expression over tokens its rows
    /// write, folding or not, as the compiler finds them (the plan keeps
    /// only those that fold), and the plan's retention beside them. Read
    /// again when a Contract source or what resolved its imports changes, or
    /// what of the app's or a package's design can move them does
    /// (`walk::retention_key`): an edit of a value that names nothing
    /// keeps them; one that moves an alias, a `params` entry or a component's
    /// param builds them again on the next propose.
    /// None when the build stopped before the design: not kept, and tried
    /// again.
    /// The retention key of `snap`: the Contract sources' key and what of
    /// the app's design and each package's can move retention
    /// (`retention_key`: names and what values name, the `params` block,
    /// components' params), parsed: a value edit naming nothing (a hex)
    /// keeps it, where a whole compile of the clone costs seconds.
    fn retention_key(&self, snap: &Snapshot) -> String {
        let mut key = self.sources_key();
        let app = self.app_params();
        let texts: Vec<&str> = snap
            .iter()
            .find(|(f, _, _)| *f == self.design)
            .map(|(_, t, _)| t.as_str())
            .into_iter()
            .chain(app.packages.iter().map(|p| p.text.as_str()))
            .collect();
        key.push_str(&walk::retention_key(&texts));
        key
    }

    fn synthetic(&mut self, snap: &Snapshot) -> Option<Vec<(String, bool)>> {
        let root = self.dir.join("app.contract");
        let key = self.retention_key(snap);
        // A whole build failed under this retention: the closure is
        // unknown, as on main, and every answer under it is `rebuild` until
        // a whole build of the files as they are passes (`check`).
        self.last_key = key.clone();
        if !self.known(&key) {
            return None;
        }
        if let Some((k, exprs, _)) = &self.synthetic {
            if *k == key {
                return Some(exprs.clone());
            }
        }
        // Built as far as the design's retention and no further, reusing
        // the last build's types and expansion where its Contract loads as
        // it did (theme2's 11:32 ruling). A build that fails after the
        // design still says them; that it fails is the whole build's to say.
        let sources = self.sources_key();
        let kept = self
            .kept
            .take()
            .filter(|(k, _)| *k == sources)
            .map(|(_, kept)| kept);
        let built = contract::retention_build(&root, kept);
        // A test reads how often they are built, and how (`EXACT_TAMBER_RETENTION_LOG`, a line each).
        if let Some(log) = std::env::var_os("EXACT_TAMBER_RETENTION_LOG") {
            use std::io::Write;
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log);
            let line = match (built.reused, built.stale) {
                (_, true) => "built stale\n",
                (true, false) => "built reused\n",
                (false, false) => "built\n",
            };
            let _ = file.and_then(|mut f| f.write_all(line.as_bytes()));
        }
        self.kept = built.kept.map(|kept| (sources.clone(), kept));
        self.rows = Some((sources, built.rows, built.paths));
        // A build that stopped before the design says nothing of them.
        let retention = built.retention?;
        self.synthetic = Some((key.clone(), built.candidates.clone(), Some(retention)));
        // Whether the whole build fails after the design: unverified, and
        // built in the background from here.
        self.checker.made(&key);
        self.known(&key);
        Some(built.candidates)
    }

    /// Whether answers under retention key `key` are known, of the files as
    /// they are now (`check::Checker::known`): a whole build under it
    /// failed and none of these files has passed since, not.
    pub(super) fn known(&mut self, key: &str) -> bool {
        let walk = self.walk();
        let (inputs, digests): (Vec<PathBuf>, Vec<String>) = walk.inputs().into_iter().unzip();
        let (manifests, facts) = walk.facts();
        drop(walk);
        let dir = self.dir.clone();
        self.checker.known(key, || check::Job {
            root: dir.join("app.contract"),
            print: check::fingerprint_read(&dir, &inputs, &digests, &facts),
            dir,
            key: key.to_string(),
            inputs,
            manifests,
        })
    }

    /// [`Server::known`] for a scrub opened under retention key `key`: the
    /// files as they are now are built only when they are still under it,
    /// so a pass clears only the key its files are under; else whether a
    /// whole build under it failed.
    pub(super) fn known_under(&mut self, key: &str) -> bool {
        let now = self.snapshot().map(|snap| self.retention_key(&snap));
        match now {
            Ok(now) if now == key => self.known(key),
            _ => !self.checker.failed(key),
        }
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
    /// several, committed as one set), then the app's own Contract sources
    /// (stage 8 D: a state part's row).
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
        let owned = self.walk().owned.clone();
        let contract = crate::tamber_contract::owned_kept(&self.dir, owned, &self.contract);
        Ok(std::iter::once(self.design.clone())
            .chain(others)
            .chain(contract)
            .collect())
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
        self.asked.set(self.asked.get() + 1);
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
                // A batch's answers are each presented as they are made, once.
                let batch = m == "resolve" && request.get("tokens").is_some();
                vec![reply(
                    answer
                        .map(|v| if batch { v } else { self.present(v) })
                        .map_err(|e| self.present(e)),
                )]
            }
            Some("propose") => vec![reply(self.transaction(&request, false))],
            Some("scrub-fold") => vec![reply(self.scrub_fold(&request))],
            Some("transaction") => self.commit_reply(&request, reply, false),
            Some("undo") => self.commit_reply(&request, reply, true),
            // The wire's own: history, revert, a re-import's marks (sources are read alone).
            Some("compare") => vec![reply(crate::tamber_history::compare(&request))],
            Some("revert-plan") => {
                vec![reply(self.read_files(true).and_then(|now| {
                    crate::tamber_history::revert_plan(&request, &now)
                }))]
            }
            Some("import-plan") if crate::tamber_import::marks_only(&request) => {
                vec![reply(self.texts().and_then(|now| {
                    let design = now.iter().find(|(f, _)| *f == self.design).map(|(_, t)| t);
                    crate::tamber_import::plan(&request, design.map(String::as_str), &[])
                }))]
            }
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
        let mut out = json!({"protocol": PROTOCOL, "format": exact_tamber::FORMAT, "builds": [], "files": files});
        let paths = request["paths"] == json!(true);
        let app = self.app_params();
        crate::tamber_packages::packaged(&app, &mut out, paths);
        if paths {
            // The walk's graph, read this request: no walk of its own.
            let at = self.dir.join("app.contract");
            let walk = self.walk();
            out["watch"] = crate::tamber_packages::watched(&at, &walk.graph);
            crate::tamber_packages::assets(&app, &mut out, &walk.graph);
        }
        Ok(out)
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
        let app = self.app_params();
        let token = Self::token_of(&app, &required(request, "token")?)?;
        let (snap, folding, _) = self.design_with(request, &app)?;
        Ok((snap, folding, token))
    }

    /// The name [`Self::lookup`] looks `name` up by: a param's token where
    /// it names one in full or short, else `name`.
    fn token_of(app: &contract::tamber_params::AppParams, name: &str) -> Result<String, Value> {
        match contract_lower::params::resolve(&app.tokens, &app.identities, name) {
            Ok(p) => Ok(p.token.clone()),
            Err(e) if e.id == "tamber-param-ambiguous" => {
                Err(json!({"message": format!("{}: {}", e.id, e.message), "id": e.id}))
            }
            Err(_) => Ok(name.to_string()),
        }
    }

    /// [`Self::design`], folded with the app's components' params (D15),
    /// and the design as parsed, without them.
    fn design_with(
        &self,
        request: &Value,
        app: &contract::tamber_params::AppParams,
    ) -> Result<(Snapshot, exact_tamber::Folding, exact_tamber::Design), Value> {
        // Folded as the build folds it (an icon naming another included),
        // with or without params.
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
        let app = self.app_params();
        if app.tokens.is_empty() {
            return v;
        }
        let text = app.shown(&v.to_string());
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
        let folding = exact_tamber::parse(text)
            .and_then(|d| exact_tamber::check_design_cached(d, &self.vocab))
            .map_err(|all| refused(&file, all))?;
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
        // `keep: false` (below write, stage 10 A1): keeping one can prune another's.
        let digest = exact_tamber::export::digest(&text); // what a client's `import` names as its baseline (stage 10b)
        if request.get("keep") == Some(&json!(false)) {
            return Ok(json!({"kind": "dtcg", "text": text, "baseline": null, "digest": digest}));
        }
        let kept = crate::tamber_export::keep(&self.dir.join(&self.design), &text)
            .map_err(|e| json!({"message": format!("the baseline: {e}")}))?;
        Ok(json!({
            "kind": "dtcg", "text": text, "digest": digest,
            "baseline": contract::tamber_uses::shown(&kept, &self.dir),
        }))
    }

    /// Where the app reads a token (`crate::tamber_uses`): each plan site
    /// whose style rows name it, directly, through a token naming it or
    /// through a synthetic token, with its source span. `site` indexes the
    /// plan compiled here (`plan`, its digest, before any bake); a build
    /// joins the spans through its own source map (LLP 1122 T2b's
    /// `outline`).
    fn uses(&mut self, request: &Value) -> Result<Value, Value> {
        let (snap, folding, name) = self.lookup(request)?;
        let icon = folding.icons.iter().any(|i| i.name == name);
        if !icon && !folding.tokens.iter().any(|t| t.name == name) {
            return Err(no_token(&folding, &name));
        }
        // A design file the app does not build has no sites.
        if optional(request, "file")?.is_some_and(|f| f != self.design) {
            return Ok(
                json!({"token": name, "plan": null, "sites": [], "aliases": [], "files": files_json(&snap)}),
            );
        }
        // An icon's image sites, an icon param's too (stage 9 C2, *1.12*).
        if icon {
            return crate::tamber_explain::icon_uses(
                &self.dir,
                &self.design,
                &self.app_params(),
                &snap,
                &name,
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
            // Each instance at the element or style holding a literal is a
            // site (stage 9 C2, *1.12*).
            let (plan, map) = contract::compile_path_mapped(&self.dir.join("app.contract"))
                .map_err(
                    |e| json!({"message": format!("the app does not build: {}", e.message)}),
                )?;
            let map: Value = serde_json::from_str(&map.json(&plan.encode()))
                .map_err(|e| json!({"message": format!("source map: {e}")}))?;
            let mut sites = contract::tamber_uses::motion_instances(
                &self.dir.join("app.contract"),
                sites,
                &map,
            )
            .map_err(|e| json!({"message": format!("the app does not build: {e}")}))?;
            contract::tamber_uses::relative_files(&mut sites, &self.dir);
            return Ok(
                json!({"token": name, "plan": map["digest"], "sites": sites, "aliases": aliases, "files": files_json(&snap)}),
            );
        }
        let (plan, map) = contract::compile_path_mapped(&self.dir.join("app.contract"))
            .map_err(|e| json!({"message": format!("the app does not build: {}", e.message)}))?;
        let map: Value = serde_json::from_str(&map.json(&plan.encode()))
            .map_err(|e| json!({"message": format!("source map: {e}")}))?;
        let mut sites = contract::tamber_uses::sites(&plan, &map, &name, &aliases, &synthetic);
        // Each site's file as the app names it, never the machine's path.
        contract::tamber_uses::relative_files(&mut sites, &self.dir);
        Ok(
            json!({"token": name, "plan": map["digest"], "sites": sites, "aliases": aliases, "files": files_json(&snap)}),
        )
    }

    /// Plan, and when `write`, commit: the digests, the inverse's id, the
    /// tokens it changes, `live` or `rebuild`, the diff. Each file it edits
    /// must carry the digest the client saw (a `propose` checks the ones it
    /// carries).
    fn transaction(&mut self, request: &Value, write: bool) -> Result<Value, Value> {
        let (rest, written) = crate::tamber_contract::split(&request["ops"])?;
        let mut ops = crate::tamber_ops::parse_ops(&rest, &self.design)?;
        let seen = seen_files(request, "files")?;
        // Files it does not edit but was planned on (a revert's file already
        // as before), each as the plan read it.
        let requires = seen_files(request, "requires")?;
        let client = match write {
            true => Some(self.writer(request)?),
            false => None,
        };
        let snap = self.snapshot()?;
        // A package's design is read-only; set-token is the app's override (*1.14*, D11).
        let design = snap
            .iter()
            .find(|(f, _, _)| *f == self.design)
            .map_or(String::new(), |(_, t, _)| t.clone());
        // A write judges package files on a fresh load, never a cached one (D11).
        let app = match write {
            true => std::rc::Rc::new(contract::tamber_params::params_of(
                &self.dir.join("app.contract"),
            )),
            false => self.app_params(),
        };
        crate::tamber_packages::ops(
            &mut ops,
            &app,
            Some(&self.dir),
            &self.design,
            &design,
            request["restore"] == json!(true),
        )?;
        // Every write (a Contract op's too) puts its temporary files and journal in `.exact`.
        if write {
            crate::tamber_packages::read_only(&app, &self.dir, std::iter::empty())?;
        }
        let contract_ops = crate::tamber_contract::files(written, &snap)?;
        let stale: Vec<&String> = snap
            .iter()
            .filter(|(p, _, d)| {
                (ops.iter().any(|(f, _)| f == p) || contract_ops.iter().any(|(f, _)| f == p))
                    // A held proposal's base gives a file it planned on
                    // and left as it was (a style op with no `file` that
                    // changed nothing there) in `requires`: as seen.
                    && match seen.get(p).or_else(|| requires.get(p)) {
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
        crate::tamber_ops::param_names(&mut ops, &snap, &self.app_params())?;
        // A scrub's preview (the wire's, LLP 1121 D13) compiles the texts
        // a propose would write, which it asks for.
        let texts = !write && request["texts"] == json!(true);
        self.run(&snap, &ops, &contract_ops, client, false, texts)
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
        // A file that has become a package's since (a link made to it) is read-only now too (D11).
        crate::tamber_packages::read_only(
            &contract::tamber_params::params_of(&self.dir.join("app.contract")),
            &self.dir,
            issued.ops.iter().map(|(f, _)| f.as_str()),
        )?;
        let result = self.run(&snap, &issued.ops, &[], Some(client), true, false);
        if result.is_ok() {
            self.issued.remove(&id);
            self.order.retain(|i| *i != id);
        }
        result
    }

    /// Plan over `snap`; with a `client`, commit and issue the inverse.
    #[allow(clippy::too_many_arguments)]
    fn run(
        &mut self,
        snap: &Snapshot,
        all_ops: &[(String, Op)],
        contract_ops: &[(String, crate::tamber_contract::ContractOp)],
        client: Option<String>,
        undo: bool,
        with_texts: bool,
    ) -> Result<Value, Value> {
        // The design's files are the crate's; a `.contract` file's writes
        // (and an inverse's restores of one) are `tamber_contract`'s.
        let contract = |p: &str| p.ends_with(".contract");
        let texts: Vec<(String, String)> = snap
            .iter()
            .filter(|(f, _, _)| !contract(f))
            .map(|(f, t, _)| (f.clone(), t.clone()))
            .collect();
        let ops: Vec<(String, Op)> = all_ops
            .iter()
            .filter(|(f, _)| !contract(f))
            .cloned()
            .collect();
        let ops = ops.as_slice();
        let known = self.synthetic(snap);
        let exprs = known.clone().unwrap_or_default();
        // The tokens the plan keeps (D7): an edit to one it does not keep
        // (a plain app's token no view names) leaves the plan as it is.
        let retention = self.synthetic.as_ref().and_then(|(_, _, r)| r.clone());
        let app = self.app_params();
        let params = crate::tamber_ops::params(&app);
        let synthetic = exact_tamber::Synthetic {
            file: &self.design,
            exprs: &exprs,
            retention: retention.as_ref(),
            params: &params,
        };
        let mut planned = exact_tamber::plan(&texts, ops, &self.vocab, undo, synthetic).map_err(|all| {
            json!({"message": "refused", "diagnostics": all.iter().map(|(f, d)| located(f, d, &app)).collect::<Vec<_>>()})
        })?;
        let restores: Vec<(String, Op)> = all_ops
            .iter()
            .filter(|(f, _)| contract(f))
            .cloned()
            .collect();
        crate::tamber_contract::plan(
            &self.dir,
            &self.design,
            snap,
            contract_ops,
            &restores,
            undo,
            &mut planned,
        )?;
        // What the build refuses beyond the crate's check, a write refuses
        // too, so nothing is written that the app cannot build: a role
        // override that varies by an app axis (LLP 1121 D5, the compiler's
        // `tamber-role-axis`). An undo restores what was, and is not judged.
        if !undo {
            self.write_checks(&texts, &planned, &self.last_key)?;
        }
        let mut out = json!({
            "files": files_json(snap),
            // A param a token's edit reaches is named in full (LLP 1122 T3:
            // the IR token a plan names it by reaches no client).
            "changed": self.present(json!(planned.changed.iter().map(|(f, t)| json!({"file": f, "token": t})).collect::<Vec<_>>())),
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
                self.recheck();
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
