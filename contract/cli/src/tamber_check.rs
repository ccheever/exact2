//! The design server's background whole build (LLP 1121 stage 12 B1's
//! follow-up, theme2's 12:15 and 12:19 rulings, option (4)). A retention
//! build stops once the design's retention is known, so it can't tell a
//! build that fails after the design. Main's server could, for an app with
//! no synthetic expressions: such a build left the closure unknown, and
//! every later edit was answered `rebuild`, so the dev loop's build said
//! the refusal. This check restores that.
//!
//! Each retention key has a verdict:
//! - unverified, after the retention build that made it: answered from
//!   that build, as theme2 ruled, while a whole build runs;
//! - passed, when a whole build of the files as they were passed;
//! - failed, when a whole build under it failed, of any files then.
//!   Answers are `rebuild` until a whole build of exactly the files as they
//!   are now passes (12:19: the one way a result moves toward `live`).
//!
//! While a key is unverified or failed, every answer makes sure a whole
//! build of the files as they are now is done or under way. Only a
//! completed build's verdict for a fingerprint is remembered (the last 64),
//! so the same files are not built again while it is, and a lost build
//! (its files moved while it built) is asked for again. A build that
//! panics has failed.
//!
//! - One worker thread, kept for the server's life. It builds the newest
//!   job it was sent, dropping any older one still waiting, so a check
//!   progresses while the server is idle.
//! - A job's fingerprint ([`fingerprint`]) covers what a whole build reads:
//!   the walk's files by content (Contract sources, designs, the
//!   `package.json`s resolved, icons), the walk's other facts (each
//!   `package.json` above a source, each `node_modules` entry and where it
//!   leads), and every entry in the app's directory and in each package an
//!   input is in (strings, assets, `.shells`, an implementation's
//!   `exact.json`; directories, links and where they lead), all but
//!   `.exact`, `target`, a nested `node_modules` and `.git`. The worker takes it before and
//!   after its build, and a result whose files moved is lost, never used.
//! - No answer waits on the worker. If no thread can be had, the answer is
//!   unknown (`rebuild`). Close drops the job queue; a running build ends
//!   with the process, and its result goes unread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

/// A whole build to run: the app's root and directory, the retention key
/// the answer was made under, and the files it reads with their
/// fingerprint then.
#[derive(Clone, PartialEq)]
pub(super) struct Job {
    pub root: PathBuf,
    pub dir: PathBuf,
    pub key: String,
    pub inputs: Vec<PathBuf>,
    /// The `package.json`s above the sources (`Walk::facts`).
    pub manifests: Vec<PathBuf>,
    pub print: String,
}

/// What a whole build found.
#[derive(Clone, Copy, PartialEq)]
enum Outcome {
    Built(bool),
    /// A file moved while it built: no verdict, asked for again.
    Lost,
}

#[derive(Clone, PartialEq)]
enum Verdict {
    Unverified,
    Passed,
    Failed,
}

/// The worker's ends: the jobs sent it, and its results.
type Worker = (Sender<Job>, Receiver<(Job, Outcome)>);

/// The server's background whole build and its verdicts.
#[derive(Default)]
pub(super) struct Checker {
    worker: Option<Worker>,
    /// The job last sent, until its result is read.
    sent: Option<Job>,
    /// Each retention key's verdict.
    verdicts: HashMap<String, Verdict>,
    /// Each completed build's verdict, by key and fingerprint, the newest
    /// last (the last few kept).
    done: Vec<(String, String, bool)>,
}

/// What the background build reads, as one string: each of `inputs` (the
/// walk's files) by its content's digest; the `package.json`s at
/// `manifests` by digest and each `node_modules` entry with where it
/// leads (`walk::facts`); then every entry under `dir` but `.exact`,
/// `target`, `node_modules` and `.git`: a directory as one, a file by its
/// length and time, a link by where it leads and that file's length and
/// time.
pub(super) fn fingerprint(dir: &Path, inputs: &[PathBuf], manifests: &[PathBuf]) -> String {
    let read: Vec<String> = inputs.iter().map(|p| super::walk::digest_of(p)).collect();
    fingerprint_read(dir, inputs, &read, &super::walk::facts(dir, manifests))
}

/// [`fingerprint`] with each input's digest and the facts as already read
/// (the server's walk, refreshed for this request): an answer reads no
/// input file again.
pub(super) fn fingerprint_read(
    dir: &Path,
    inputs: &[PathBuf],
    digests: &[String],
    facts: &str,
) -> String {
    let mut out = String::new();
    for (p, d) in inputs.iter().zip(digests) {
        out.push_str(&format!("{}\t{d}\n", p.display()));
    }
    out.push_str(facts);
    let stamp = |m: &std::fs::Metadata| {
        let modified = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        format!("{}\t{modified}", m.len())
    };
    // The app's directory, and each package an input is in (the nearest
    // directory above it with a `package.json`): a package's files the
    // graph doesn't name (an implementation's `exact.json`) are read too.
    let inside = |p: &Path| p.starts_with(dir) && !p.starts_with(dir.join("node_modules"));
    let mut stack = vec![dir.to_path_buf()];
    for p in inputs.iter().filter(|p| !inside(p)) {
        if let Some(root) = p
            .ancestors()
            .skip(1)
            .find(|d| d.join("package.json").is_file())
        {
            if !stack.iter().any(|r| r == root) {
                stack.push(root.to_path_buf());
            }
        }
    }
    let mut entries: Vec<String> = Vec::new();
    // Each directory walked, as it really is: a link to one is walked once.
    let mut walked: Vec<PathBuf> = Vec::new();
    while let Some(d) = stack.pop() {
        let real = d.canonicalize().unwrap_or_else(|_| d.clone());
        if walked.contains(&real) {
            continue;
        }
        walked.push(real);
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in read.flatten() {
            let path = e.path();
            if matches!(
                e.file_name().to_str(),
                Some(".exact" | "target" | "node_modules" | ".git")
            ) {
                continue;
            }
            let Ok(m) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let line = if m.file_type().is_symlink() {
                // Where it leads, and that: a file's stamp, a directory
                // walked (once, however many links lead to it).
                let to = std::fs::read_link(&path).unwrap_or_default();
                let target = std::fs::metadata(&path);
                if target.as_ref().is_ok_and(|t| t.is_dir()) {
                    stack.push(path.clone());
                }
                let target = target.map(|t| stamp(&t)).unwrap_or_default();
                format!("link\t{}\t{}\t{target}", path.display(), to.display())
            } else if m.is_dir() {
                stack.push(path.clone());
                format!("dir\t{}", path.display())
            } else {
                format!("file\t{}\t{}", path.display(), stamp(&m))
            };
            entries.push(line);
        }
    }
    entries.sort();
    for e in entries {
        out.push_str(&e);
        out.push('\n');
    }
    out
}

/// Run `job`'s whole build, reading its fingerprint before and after.
fn build(job: &Job) -> Outcome {
    if fingerprint(&job.dir, &job.inputs, &job.manifests) != job.print {
        return Outcome::Lost;
    }
    log("began");
    // A test's: each build waits, once its files were read, until the
    // file `EXACT_TAMBER_CHECK_GATE` names is there, and takes it.
    if let Some(gate) = std::env::var_os("EXACT_TAMBER_CHECK_GATE") {
        let gate = PathBuf::from(gate);
        while std::fs::remove_file(&gate).is_err() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    // A build that panics is one that failed: main's server, which built
    // in place, went down with it.
    let built = std::panic::catch_unwind(|| contract::compile_path_all(&job.root, false).is_ok())
        .unwrap_or(false);
    match fingerprint(&job.dir, &job.inputs, &job.manifests) == job.print {
        true => Outcome::Built(built),
        false => Outcome::Lost,
    }
}

/// A test reads what each build did (`EXACT_TAMBER_CHECK_LOG`, a line
/// each): `began` once its files were read the first time, then what it
/// found once its result can be read.
fn log(line: &str) {
    if let Some(log) = std::env::var_os("EXACT_TAMBER_CHECK_LOG") {
        use std::io::Write;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log);
        let _ = file.and_then(|mut f| writeln!(f, "{line}"));
    }
}

fn logged(outcome: Outcome) {
    log(match outcome {
        Outcome::Built(true) => "passed",
        Outcome::Built(false) => "failed",
        Outcome::Lost => "lost",
    });
}

/// The worker: the newest job it holds, built; its result sent, then
/// logged.
fn work(jobs: Receiver<Job>, results: Sender<(Job, Outcome)>) {
    while let Ok(mut job) = jobs.recv() {
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let outcome = build(&job);
        if results.send((job, outcome)).is_err() {
            return;
        }
        logged(outcome);
    }
}

impl Checker {
    /// A retention build made `key`'s retention: unverified until a whole
    /// build says, unless a whole build under it failed.
    pub fn made(&mut self, key: &str) {
        // Bounded: a long session's keys, the failed ones kept.
        if self.verdicts.len() > 64 {
            self.verdicts.retain(|_, v| *v == Verdict::Failed);
        }
        self.verdicts
            .entry(key.to_string())
            .and_modify(|v| {
                if *v == Verdict::Passed {
                    *v = Verdict::Unverified
                }
            })
            .or_insert(Verdict::Unverified);
    }

    /// Read every finished build's result.
    pub fn poll(&mut self) {
        let mut lost_worker = false;
        while let Some((_, results)) = &self.worker {
            match results.try_recv() {
                Ok((job, outcome)) => self.read(job, outcome),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    lost_worker = true;
                    break;
                }
            }
        }
        if lost_worker {
            self.worker = None;
            self.sent = None;
        }
    }

    fn read(&mut self, job: Job, outcome: Outcome) {
        if self.sent.as_ref() == Some(&job) {
            self.sent = None;
        }
        let Outcome::Built(passed) = outcome else {
            return;
        };
        if !passed {
            self.verdicts.insert(job.key.clone(), Verdict::Failed);
        }
        self.done
            .retain(|(k, p, _)| !(*k == job.key && *p == job.print));
        self.done.push((job.key, job.print, passed));
        if self.done.len() > 64 {
            self.done.remove(0);
        }
    }

    /// The verdict of a completed whole build of `job`'s files under its
    /// key.
    fn passed(&self, job: &Job) -> Option<bool> {
        self.done
            .iter()
            .rev()
            .find(|(k, p, _)| *k == job.key && *p == job.print)
            .map(|(_, _, ok)| *ok)
    }

    /// Whether the answer under `job`'s key, of the files it fingerprints,
    /// is known: not when a whole build under the key failed and none of
    /// exactly these files has passed since. While the key is unverified
    /// or failed, makes sure a whole build of these files is done or under
    /// way.
    /// `job` makes the job (its fingerprint reads the app's files), asked
    /// only when the key's verdict needs it.
    pub fn known(&mut self, key: &str, job: impl FnOnce() -> Job) -> bool {
        self.poll();
        let verdict = self.verdicts.get(key).cloned();
        if matches!(verdict, None | Some(Verdict::Passed)) {
            return true;
        }
        let job = job();
        match self.passed(&job) {
            Some(true) => {
                self.verdicts.insert(job.key.clone(), Verdict::Passed);
                return true;
            }
            Some(false) => return false,
            // No worker to be had: unknown, never a build in the answer's
            // way.
            None => {
                if !self.send(job.clone()) {
                    return false;
                }
            }
        }
        match self.verdicts.get(&job.key) {
            Some(Verdict::Failed) => self.passed(&job) == Some(true),
            _ => true,
        }
    }

    /// Send `job` to the worker, starting it where there is none: whether
    /// it is sent (no thread can be had, not).
    fn send(&mut self, job: Job) -> bool {
        if self.sent.as_ref() == Some(&job) {
            return true;
        }
        if self.worker.is_none() {
            let (jobs, take) = channel();
            let (give, results) = channel();
            let spawned = std::thread::Builder::new()
                .name("tamber-check".into())
                .spawn(move || work(take, give));
            match spawned {
                Ok(_) => self.worker = Some((jobs, results)),
                Err(_) => return false,
            }
        }
        let (jobs, _) = self.worker.as_ref().expect("started");
        match jobs.send(job.clone()) {
            Ok(()) => {
                self.sent = Some(job);
                true
            }
            Err(_) => {
                self.worker = None;
                self.sent = None;
                false
            }
        }
    }

    /// Whether a whole build under `key` failed and none of the files since
    /// has passed: what a scrub opened under a retention the files have
    /// since left asks.
    pub fn failed(&mut self, key: &str) -> bool {
        self.poll();
        self.verdicts.get(key) == Some(&Verdict::Failed)
    }

    /// [`Checker::failed`] as last polled, for a caller holding no `&mut`
    /// (the write check's rows, `tamber_scrub`).
    pub fn seen_failed(&self, key: &str) -> bool {
        self.verdicts.get(key) == Some(&Verdict::Failed)
    }

    /// Whether a build was sent and its result not yet read (a test's).
    #[cfg(test)]
    pub fn busy(&mut self) -> bool {
        self.poll();
        self.sent.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, Checker, Job};
    use std::path::{Path, PathBuf};

    /// An app whose whole build fails after its design (`broken`) or not.
    fn app(tag: &str, broken: bool) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exact2-check-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("app.contract"),
            "component App\n  view\n    button \"Go\" -exact-press-scale=\"scale\"\n",
        )
        .unwrap();
        design(&dir, if broken { "neg" } else { "one" });
        dir
    }

    fn design(dir: &Path, to: &str) {
        std::fs::write(
            dir.join("app.tamber"),
            format!("tamber t\n\ntheme\n  one = 1\n  two = 2\n  neg = -1\n  scale = {to}\n"),
        )
        .unwrap();
    }

    fn job(dir: &Path) -> Job {
        let inputs: Vec<PathBuf> = ["app.contract", "app.tamber"]
            .iter()
            .map(|f| dir.join(f))
            .collect();
        Job {
            root: dir.join("app.contract"),
            dir: dir.to_path_buf(),
            key: "k".into(),
            print: fingerprint(dir, &inputs, &[]),
            inputs,
            manifests: Vec::new(),
        }
    }

    /// Ask until no build is under way; the last answer.
    fn settle(c: &mut Checker, dir: &Path) -> bool {
        for _ in 0..600 {
            c.known("k", || job(dir));
            if !c.busy() {
                return c.known("k", || job(dir));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("the check never settled");
    }

    #[test]
    fn a_failing_whole_build_marks_its_key_unknown_and_a_pass_of_the_files_clears_it() {
        let dir = app("fails", true);
        let mut c = Checker::default();
        c.made("k");
        assert!(!settle(&mut c, &dir), "failed");
        design(&dir, "two");
        assert!(!c.known("k", || job(&dir)), "failed until these files pass");
        assert!(settle(&mut c, &dir), "passed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A, then B, then A again: the newest job is what is built, and a
    /// failure of any files under the key marks it.
    #[test]
    fn the_newest_job_is_built_and_a_failure_under_the_key_stands() {
        let dir = app("newest", true);
        let mut c = Checker::default();
        c.made("k");
        c.known("k", || job(&dir));
        design(&dir, "two");
        c.known("k", || job(&dir));
        design(&dir, "neg");
        assert!(
            !settle(&mut c, &dir),
            "broken again: the newest build fails"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same files are never built twice: a failure is remembered.
    #[test]
    fn a_failure_is_not_built_again() {
        let dir = app("again", true);
        let mut c = Checker::default();
        c.made("k");
        settle(&mut c, &dir);
        let done = c.done.len();
        for _ in 0..3 {
            assert!(!c.known("k", || job(&dir)));
            assert!(!c.busy());
        }
        assert_eq!(c.done.len(), done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The fingerprint sees an empty directory made (`strings/`, which the
    /// build reads) and where a link leads.
    #[test]
    fn the_fingerprint_sees_directories_and_links() {
        let dir = app("print", false);
        let inputs = vec![dir.join("app.contract")];
        let before = fingerprint(&dir, &inputs, &[]);
        std::fs::create_dir(dir.join("strings")).unwrap();
        let made = fingerprint(&dir, &inputs, &[]);
        assert_ne!(before, made);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("app.tamber"), dir.join("strings/en.json"))
                .unwrap();
            let linked = fingerprint(&dir, &inputs, &[]);
            assert_ne!(made, linked);
            // Non-input files use length and mtime; change length even if mtime stays equal.
            design(&dir, "2");
            let edited = fingerprint(&dir, &inputs, &[]);
            assert_ne!(linked, edited, "the link's target changed");
            // A linked directory is walked: a file in it is seen.
            let outside = app("print-outside", false);
            std::os::unix::fs::symlink(&outside, dir.join("linked")).unwrap();
            let with_link = fingerprint(&dir, &inputs, &[]);
            std::fs::write(outside.join("app.tamber"), "tamber t\n\ntheme\n  one = 3\n").unwrap();
            assert_ne!(
                with_link,
                fingerprint(&dir, &inputs, &[]),
                "a file in a linked directory"
            );
            let _ = std::fs::remove_dir_all(&outside);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A package's file the graph doesn't name (an implementation's
    /// `exact.json`) is in the fingerprint of an input in that package.
    #[test]
    fn the_fingerprint_sees_a_packages_other_files() {
        let dir = app("print-package", false);
        let pkg = dir.join("node_modules/@acme/geo");
        std::fs::create_dir_all(pkg.join("web")).unwrap();
        std::fs::write(pkg.join("package.json"), r#"{"name":"@acme/geo"}"#).unwrap();
        std::fs::write(
            pkg.join("index.contract"),
            "component Geo\n  view\n    text \"g\"\n",
        )
        .unwrap();
        std::fs::write(pkg.join("web/exact.json"), "{}").unwrap();
        let inputs = vec![dir.join("app.contract"), pkg.join("index.contract")];
        let before = fingerprint(&dir, &inputs, &[]);
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(pkg.join("web/exact.json"), r#"{"shapes": []}"#).unwrap();
        assert_ne!(before, fingerprint(&dir, &inputs, &[]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
