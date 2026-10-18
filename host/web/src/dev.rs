//! The resident dev driver: source change observed → plan ready, in one
//! long-lived process (LLP 1004 D5; LLP 1006 §8's owed piece).
//!
//! A [`Session`] watches one `.contract` file and every source it uses
//! (LLP 1091 D10). When their bytes change it
//! compiles, bakes against the app's data source, and writes the plan
//! atomically where the page fetches it. A reload is a restart: the page
//! boots the new plan from initial state (`exact_boot_plan`), with no state
//! migration and no patch format — D5 taken literally. `host/web/dev.mjs`
//! runs [`main`] for an app, pushes each ready plan to the page, and prints
//! the edit → present numbers against `rules/RULES.md`'s row.
//!
//! A value-only design edit (LLP 1121 stage 4): when only `.tamber` files
//! changed and the new plan's structure ([`exact_plan::Plan::structure`]) is
//! the published one's, no plan is published and nothing restarts. The
//! changed tokens go out as a batch instead ([`exact_runner::design`]), which
//! every running build applies in place. (The plan file is still written,
//! for a host that watches the file rather than the stream.) This is decided by content, so a hand
//! edit and the Tamber server's write are one case, in any process.

use exact_runner::DataSource;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// One plan, built from one save.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    /// The baked plan's bytes.
    pub bytes: Vec<u8>,
    /// The source file's modification time, unix milliseconds — when the
    /// edit happened, as the filesystem saw it.
    pub saved_ms: f64,
    /// Compile time, milliseconds.
    pub compile_ms: f64,
    /// Bake time (one runner boot against the data source), milliseconds.
    pub bake_ms: f64,
    /// Unix milliseconds when the plan was on disk.
    pub ready_ms: f64,
    /// A value-only design edit: its plan is on disk for a host that watches
    /// the file, but the stream publishes no plan; the batch from the design
    /// the builds run, and a snapshot of the new one for a build that
    /// connects or missed a batch.
    pub live: Option<Live>,
}

/// A value-only design edit's batch and snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    /// From the design the builds run to the new one.
    pub batch: exact_runner::design::Edit,
    /// The new design, whole.
    pub snapshot: exact_runner::design::Edit,
}

/// A watch on one source file.
#[derive(Debug)]
pub struct Session {
    source: PathBuf,
    out: PathBuf,
    last: Option<String>,
    stamp: Option<(SystemTime, u64)>,
    surfaces_stamp: Option<(SystemTime, u64)>,
    /// The files the last build read besides the root, and how they looked.
    uses: Vec<(PathBuf, Option<(SystemTime, u64)>)>,
    failed: bool,
    source_map: bool,
    /// The published plan's structure digest and the design the builds run
    /// (the plan's, or the last batch's).
    design: Option<(String, Vec<exact_runner::design::Token>)>,
    /// The published plan's design shape, which a patched frame's batch is
    /// for.
    shape: String,
    /// An open scrub's last preview (LLP 1121 stage 4, commit 6): its
    /// design shape and the design the builds run while it lasts.
    scrub: Option<(String, Vec<exact_runner::design::Token>)>,
    /// That scrub is being written: the next build batches from it and
    /// ends it.
    released: bool,
}

/// A used file as it reads: its time and a fingerprint of its bytes, so a
/// link retargeted at a twin, a copy that kept its times, or a junction
/// moved on any platform is a change. Used files are small Contract sources
/// and manifests.
fn stamp_of(path: &Path) -> Option<(SystemTime, u64)> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::fs::read(path).ok()?.hash(&mut hasher);
    // Which file it really is, on every platform: a link or junction
    // retargeted at a byte-identical twin leads to another path.
    std::fs::canonicalize(path).ok().hash(&mut hasher);
    // And, where the platform has one, which file it is: a link retargeted
    // at a byte-identical twin whose relative uses differ.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (meta.dev(), meta.ino()).hash(&mut hasher);
    }
    Some((meta.modified().ok()?, hasher.finish()))
}

/// The files compiling `root` reads besides itself: used files, packages'
/// files and their `package.json`s, as they are now.
fn uses(root: &Path) -> Vec<(PathBuf, Option<(SystemTime, u64)>)> {
    let graph = contract::source_graph(root);
    let mut out = graph.consulted;
    // The root by the path it is opened at: retargeting it at a twin whose
    // relative uses differ is a change its bytes do not show.
    out.push(root.to_path_buf());
    for source in graph.sources.into_iter().skip(1) {
        if source.path.is_absolute() {
            out.push(source.path);
        }
    }
    out.sort();
    out.dedup();
    out.into_iter()
        .map(|path| {
            let stamp = stamp_of(&path);
            (path, stamp)
        })
        .collect()
}

fn unix_ms(t: SystemTime) -> f64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

impl Session {
    /// Watch `source`; write each plan to `out`.
    pub fn new(source: impl Into<PathBuf>, out: impl Into<PathBuf>) -> Session {
        Session {
            source: source.into(),
            out: out.into(),
            last: None,
            stamp: None,
            surfaces_stamp: None,
            uses: Vec::new(),
            failed: false,
            source_map: true,
            design: None,
            shape: String::new(),
            scrub: None,
            released: false,
        }
    }

    /// Build a static plan without development-only source information.
    pub fn static_build(source: impl Into<PathBuf>, out: impl Into<PathBuf>) -> Session {
        let mut session = Self::new(source, out);
        session.source_map = false;
        session
    }

    /// The source path.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Look once. `None` when the source has not changed since the last
    /// build (a save with identical bytes is not a change); otherwise the
    /// build or why it failed — a compile error names its line and column.
    pub fn poll<D: DataSource + Default>(&mut self) -> Option<Result<Built, String>> {
        let meta = std::fs::metadata(&self.source).ok()?;
        let stamp = (meta.modified().ok()?, meta.len());
        let surfaces_stamp = std::fs::metadata(self.source.with_file_name(".shells/surfaces.json"))
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        // By fingerprint (bytes and which file it is), not time: a used file
        // re-saved with identical bytes is no change, as the root is not.
        let print = |s: Option<(SystemTime, u64)>| s.map(|s| s.1);
        let changed: Vec<(&Path, Option<SystemTime>)> = self
            .uses
            .iter()
            .filter_map(|(path, seen)| {
                let now = stamp_of(path);
                (print(now) != print(*seen)).then(|| (path.as_path(), now.map(|s| s.0)))
            })
            .collect();
        // When the edit was saved: its newest file, a used one (a design)
        // included, not only the root's.
        let edited = changed.iter().filter_map(|(_, at)| *at).max();
        let uses_changed = !changed.is_empty();
        // Only design files: a value-only edit, if the structure agrees.
        let design_only = uses_changed
            && changed
                .iter()
                .all(|(p, _)| p.extension().is_some_and(|e| e == "tamber"));
        let surfaces_changed = self.surfaces_stamp != surfaces_stamp || uses_changed;
        // A failed compile may have observed a file while an editor was
        // replacing its bytes. Until a good plan lands, re-read even when
        // the coarse metadata stamp is unchanged; identical bad bytes are
        // still suppressed below.
        if !self.failed && self.stamp == Some(stamp) && !surfaces_changed {
            return None;
        }
        self.stamp = Some(stamp);
        let src = match std::fs::read_to_string(&self.source) {
            Ok(s) => s,
            Err(e) => {
                self.failed = true;
                return Some(Err(format!("{}: {e}", self.source.display())));
            }
        };
        if self.last.as_deref() == Some(src.as_str()) && !surfaces_changed {
            return None;
        }
        let design_only = design_only
            && self.last.as_deref() == Some(src.as_str())
            && self.surfaces_stamp == surfaces_stamp;
        self.last = Some(src.clone());
        self.surfaces_stamp = surfaces_stamp;
        let saved = surfaces_stamp
            .map_or(stamp.0, |s| s.0.max(stamp.0))
            .max(edited.unwrap_or(stamp.0));
        let built = self.build::<D>(&src, unix_ms(saved), design_only);
        self.failed = built.is_err();
        self.uses = uses(&self.source);
        Some(built)
    }

    fn build<D: DataSource + Default>(
        &mut self,
        src: &str,
        saved_ms: f64,
        design_only: bool,
    ) -> Result<Built, String> {
        let t = Instant::now();
        // Compile exactly the snapshot `poll` compared with `last`. Reading
        // the path again here can observe the middle of the next save and
        // then suppress its final bytes as already seen.
        // Every independent refusal, one after another as `contract build`
        // prints them: the page's overlay shows them all.
        let (plan, map) = contract::compile_path_source_all(&self.source, src, self.source_map)
            .map_err(|errors| {
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let baked = contract::bake(plan, D::default()).map_err(|e| {
            map.as_ref().map_or_else(
                || format!("bake: {e:?}"),
                |map| map.bake_error(&e).to_string(),
            )
        })?;
        let bake_ms = t.elapsed().as_secs_f64() * 1000.0;
        let structure = exact_runner::design::structure(&baked);
        let shape = exact_runner::design::shape(&baked);
        let tokens = exact_runner::design::tokens(&baked);
        // While a scrub previews, the builds stay on it: a value-only build
        // changes only what its end restores (an empty batch, which the loop
        // does not send); a new plan ends it. A released one's build (what
        // it wrote) goes from it, and ends it.
        if self.released {
            self.released = false;
            let shown = self.scrub.take().map(|(_, t)| t);
            if let (Some(shown), Some((_, committed))) = (shown, self.design.as_mut()) {
                *committed = shown;
            }
        }
        let previewing = self.scrub.as_ref().map(|(_, t)| t.clone());
        let live = design_only
            .then(|| {
                let (published, committed) = self.design.as_ref()?;
                let (from, to) = match &previewing {
                    Some(shown) => (shown, shown),
                    None => (committed, &tokens),
                };
                let batch = (*published == structure)
                    .then(|| exact_runner::design::batch(&shape, from, to))??;
                let snapshot = exact_runner::design::snapshot(&shape, to);
                Some(Live { batch, snapshot })
            })
            .flatten();
        if live.is_none() {
            self.scrub = None;
        }
        let bytes = baked.encode();
        exact_bake::write_development_artifacts(&baked)?;
        // Publish the map first, then its plan. Readers must check the digest
        // because the two complete files cannot be renamed as one operation.
        if let Some(map) = map {
            let mut path = self.out.as_os_str().to_os_string();
            path.push(".map.json");
            write_atomic(&PathBuf::from(path), map.json(&bytes).as_bytes())?;
        }
        // Atomic, and per process: the page never fetches a half-written
        // plan, and two drivers on one file cannot trip over one tmp.
        // A value-only edit's plan is written too, for a host that watches
        // the file (`EXACT_DEV_PLAN=<path>`, which restarts from it); the
        // stream publishes only its batch.
        write_atomic(&self.out, &bytes)?;
        self.design = Some((structure, tokens));
        self.shape = shape;
        Ok(Built {
            bytes,
            saved_ms,
            compile_ms,
            bake_ms,
            ready_ms: unix_ms(SystemTime::now()),
            live,
        })
    }
}

impl Session {
    /// A scrub's preview (LLP 1121 stage 4, commit 6): the design `text`
    /// compiled in place of the file's and baked, written nowhere; the batch
    /// from the design the builds run to it (`None` when it changes nothing
    /// they read) and its snapshot. A structure change is refused: a
    /// rebuild has no scrub.
    pub fn scrub<D: DataSource + Default>(&mut self, text: String) -> Result<Scrubbed, String> {
        let (shape, tokens) = self.compile_design::<D>(text)?;
        Ok(self.show(shape, tokens))
    }

    /// A scrub's frame patched (LLP 1121 D13): each folded row
    /// (`scrub-fold`'s `values`, a name and its CSS text per full app
    /// tuple) set on the committed design, compiling nothing; the batch from
    /// the design the builds run and the snapshot, as [`Session::scrub`]
    /// answers. `Ok(None)` when it cannot patch the frame (a row would gain
    /// or lose its slices, which is a structure change): the caller compiles
    /// it instead. A name the plan keeps no row for changes nothing the
    /// builds read.
    pub fn patch(&mut self, values: &[(String, Vec<String>)]) -> Result<Option<Scrubbed>, String> {
        let (_, committed) = self.design.as_ref().ok_or("no plan is built yet")?;
        let mut tokens = committed.clone();
        for (name, css) in values {
            let Some(t) = tokens.iter_mut().find(|t| t.name == *name) else {
                continue;
            };
            let Some(first) = css.first() else {
                return Ok(None);
            };
            let varies = css.iter().any(|c| c != first);
            if varies != !t.slices.is_empty() || varies && css.len() != t.slices.len() {
                return Ok(None);
            }
            t.value.clone_from(first);
            if varies {
                t.slices.clone_from(css);
            }
        }
        Ok(Some(self.show(self.shape.clone(), tokens)))
    }

    /// A check of the scrub's frame shown, to run off this thread: the
    /// design `text` compiled in full (unbaked: baking leaves the theme as it
    /// is), its shape and tokens, for [`Session::verified`].
    pub fn verify_job(
        &self,
        text: String,
    ) -> impl FnOnce() -> Result<(String, Vec<exact_runner::design::Token>), String> + Send + 'static
    {
        let (source, src) = (self.source.clone(), self.last.clone());
        move || {
            let path = contract::design_path(&source).ok_or("the app has no design")?;
            let src = src.ok_or("no plan is built yet")?;
            let (plan, _) = contract::with_design_text(&path, text, || {
                contract::compile_path_source_all(&source, &src, false)
            })
            .map_err(|errors| {
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
            Ok((
                exact_runner::design::shape(&plan),
                exact_runner::design::tokens(&plan),
            ))
        }
    }

    /// The scrub's frame shown, against `compiled` ([`Session::verify_job`]'s):
    /// `Some` naming what differs. None with no frame shown.
    pub fn verified(
        &self,
        compiled: &(String, Vec<exact_runner::design::Token>),
    ) -> Option<String> {
        let (shape, shown) = self.scrub.as_ref()?;
        let (compiled_shape, compiled) = compiled;
        if compiled_shape != shape || compiled.len() != shown.len() {
            return Some("its compile changes the design's shape".into());
        }
        compiled
            .iter()
            .zip(shown)
            .find(|(a, b)| a != b)
            .map(|(a, b)| {
                format!(
                    "`{}`: compiled {:?} {:?}, patched {:?} {:?}",
                    a.name, a.value, a.slices, b.value, b.slices
                )
            })
    }

    /// The design `text` compiled in place of the file's and baked, written
    /// nowhere: its shape and tokens. A structure change is refused.
    fn compile_design<D: DataSource + Default>(
        &self,
        text: String,
    ) -> Result<(String, Vec<exact_runner::design::Token>), String> {
        let (published, _) = self.design.as_ref().ok_or("no plan is built yet")?;
        let path = contract::design_path(&self.source).ok_or("the app has no design")?;
        let src = self.last.clone().ok_or("no plan is built yet")?;
        let (plan, _) = contract::with_design_text(&path, text, || {
            contract::compile_path_source_all(&self.source, &src, false)
        })
        .map_err(|errors| {
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })?;
        let baked = contract::bake(plan, D::default()).map_err(|e| format!("bake: {e:?}"))?;
        if exact_runner::design::structure(&baked) != *published {
            return Err(
                "the scrub changes the plan's structure: a rebuild applies on release".into(),
            );
        }
        Ok((
            exact_runner::design::shape(&baked),
            exact_runner::design::tokens(&baked),
        ))
    }

    /// Show the design `tokens` while a scrub lasts: the batch from the one
    /// the builds run (none when nothing they read moved) and its snapshot.
    fn show(&mut self, shape: String, tokens: Vec<exact_runner::design::Token>) -> Scrubbed {
        let committed = self.design.as_ref().map(|(_, t)| t);
        let from = self.scrub.as_ref().map(|(_, t)| t).or(committed);
        let batch = from
            .and_then(|from| exact_runner::design::batch(&shape, from, &tokens))
            .filter(|b| !b.tokens.is_empty());
        let snapshot = exact_runner::design::snapshot(&shape, &tokens);
        // A new preview is no released scrub's.
        self.scrub = Some((shape, tokens));
        self.released = false;
        (batch, snapshot)
    }

    /// A scrub being written: the build of what it writes goes from it.
    pub fn release(&mut self) {
        self.released = self.scrub.is_some();
    }

    /// A scrub ended without its write: the batch back to the committed
    /// design and its snapshot, if one previewed.
    pub fn restore(&mut self) -> Option<Scrubbed> {
        self.released = false;
        let (shape, scrubbed) = self.scrub.take()?;
        let (_, committed) = self.design.as_ref()?;
        let batch = exact_runner::design::batch(&shape, &scrubbed, committed)
            .filter(|b| !b.tokens.is_empty());
        Some((batch, exact_runner::design::snapshot(&shape, committed)))
    }
}

/// A scrub's batch (none when nothing the builds read moved) and the
/// snapshot of the design it leaves them on.
pub type Scrubbed = (
    Option<exact_runner::design::Edit>,
    exact_runner::design::Edit,
);

/// `text` as a JSON string: one stdout line however many lines it holds.
fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_atomic(out: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut tmp = out.as_os_str().to_os_string();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    let result = file.write_all(bytes).and_then(|_| {
        drop(file);
        std::fs::rename(&tmp, out)
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("{}: {e}", out.display()))
}

/// `dev <source.contract> <out.plan>`: watch forever, one line per event on
/// stdout — `plan <bytes> <saved_ms> <compile_ms> <bake_ms> <ready_ms>`,
/// `design <saved_ms> <compile_ms> <bake_ms> <ready_ms> {"batch":…,"snapshot":…}`
/// for a value-only design edit (no plan written), or `error <message as a
/// JSON string>`, its lines kept — for `dev.mjs` to relay. On stdin, a scrub
/// (LLP 1121 stage 4, commit 6): `scrub <id> <design text as a JSON string>`,
/// `restore <id>` and `release <id>` (the scrub is being written: the build
/// of what it writes goes from it), each answered `scrubbed <id>
/// {"batch":…,"snapshot":…}` (each null when there is none) or `scrubbed
/// <id> {"error":…}`, no file written. Polls every 10 ms; a stat is
/// microseconds. `--once` builds the plan a single time and exits —
/// `build.mjs` uses it so `dist/` carries `app.plan` for a static host
/// (LLP 1023 D2: the envelope points at a file that must exist).
pub fn main<D: DataSource + Default>() -> std::process::ExitCode {
    use std::io::Write as _;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(source), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: dev <source.contract> <out.plan> [--once]");
        return std::process::ExitCode::from(2);
    };
    if args.iter().any(|a| a == "--once") {
        let mut session = Session::static_build(source, out);
        return match session.poll::<D>() {
            Some(Ok(b)) => {
                println!("plan {} bytes", b.bytes.len());
                std::process::ExitCode::SUCCESS
            }
            Some(Err(e)) => {
                eprintln!("error {e}");
                std::process::ExitCode::FAILURE
            }
            None => {
                eprintln!("error {source}: nothing to build");
                std::process::ExitCode::FAILURE
            }
        };
    }
    let mut session = Session::new(source, out);
    let mut stdout = std::io::stdout();
    let (tx, commands) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    // A command that woke the wait, answered before the next poll.
    let mut pending: Option<String> = None;
    // Verifications compiled off this thread, answered as they finish.
    let verified = std::sync::mpsc::channel::<(String, Result<_, String>)>();
    loop {
        while let Ok((id, compiled)) = verified.1.try_recv() {
            let reply = match compiled.map(|c| session.verified(&c)) {
                Ok(Some(m)) => format!("{{\"mismatch\":{}}}", json_string(&m)),
                Ok(None) => "{}".into(),
                Err(e) => format!("{{\"error\":{}}}", json_string(&e)),
            };
            let _ = writeln!(stdout, "scrubbed {id} {reply}");
        }
        while let Some(line) = pending.take().or_else(|| commands.try_recv().ok()) {
            let mut parts = line.splitn(3, ' ');
            let (verb, id, rest) = (parts.next(), parts.next().unwrap_or(""), parts.next());
            let answer = match (verb, rest) {
                (Some("scrub"), Some(text)) => serde_json::from_str::<String>(text)
                    .map_err(|e| format!("scrub: {e}"))
                    .and_then(|text| session.scrub::<D>(text))
                    .map(|(b, s)| (b, Some(s))),
                (Some("patch"), Some(values)) => {
                    match serde_json::from_str::<Vec<(String, Vec<String>)>>(values)
                        .map_err(|e| format!("patch: {e}"))
                        .and_then(|values| session.patch(&values))
                    {
                        Ok(Some((b, s))) => Ok((b, Some(s))),
                        Ok(None) => {
                            let _ = writeln!(stdout, "scrubbed {id} {{\"fallback\":true}}");
                            continue;
                        }
                        Err(e) => Err(e),
                    }
                }
                (Some("verify"), Some(text)) => {
                    // Compiled off this thread: frames keep coming meanwhile.
                    match serde_json::from_str::<String>(text) {
                        Ok(text) => {
                            let (job, tx, id) =
                                (session.verify_job(text), verified.0.clone(), id.to_string());
                            std::thread::spawn(move || {
                                let _ = tx.send((id, job()));
                            });
                        }
                        Err(e) => {
                            let _ = writeln!(
                                stdout,
                                "scrubbed {id} {{\"error\":{}}}",
                                json_string(&format!("verify: {e}"))
                            );
                        }
                    }
                    continue;
                }
                (Some("restore"), _) => Ok(session
                    .restore()
                    .map_or((None, None), |(b, s)| (b, Some(s)))),
                (Some("release"), _) => {
                    session.release();
                    Ok((None, None))
                }
                _ => Err(format!("unknown command {}", json_string(&line))),
            };
            let json =
                |e: Option<exact_runner::design::Edit>| e.map_or("null".into(), |e| e.json());
            let reply = match answer {
                Ok((batch, snapshot)) => format!(
                    "{{\"batch\":{},\"snapshot\":{}}}",
                    json(batch),
                    json(snapshot)
                ),
                Err(e) => format!("{{\"error\":{}}}", json_string(&e)),
            };
            let _ = writeln!(stdout, "scrubbed {id} {reply}");
        }
        match session.poll::<D>() {
            // Nothing the builds read changed.
            Some(Ok(Built {
                live: Some(live), ..
            })) if live.batch.tokens.is_empty() => {}
            Some(Ok(Built {
                live: Some(live),
                saved_ms,
                compile_ms,
                bake_ms,
                ready_ms,
                ..
            })) => {
                let _ = writeln!(
                    stdout,
                    "design {saved_ms:.0} {compile_ms:.3} {bake_ms:.3} {ready_ms:.0} {{\"batch\":{},\"snapshot\":{}}}",
                    live.batch.json(),
                    live.snapshot.json()
                );
            }
            Some(Ok(b)) => {
                let _ = writeln!(
                    stdout,
                    "plan {} {:.0} {:.3} {:.3} {:.0}",
                    b.bytes.len(),
                    b.saved_ms,
                    b.compile_ms,
                    b.bake_ms,
                    b.ready_ms
                );
            }
            Some(Err(e)) => {
                let _ = writeln!(stdout, "error {}", json_string(&e));
            }
            None => {}
        }
        let _ = stdout.flush();
        // The next poll in 10 ms; a command wakes it at once.
        let tick = std::time::Duration::from_millis(10);
        match commands.recv_timeout(tick) {
            Ok(line) => pending = Some(line),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => std::thread::sleep(tick),
        }
    }
}
