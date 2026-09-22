//! The resident dev driver: source change observed → plan ready, in one
//! long-lived process (LLP 1004 D5; LLP 1006 §8's owed piece).
//!
//! A [`Session`] watches one `.contract` file. When its bytes change it
//! compiles, bakes against the app's data source, and writes the plan
//! atomically where the page fetches it. A reload is a restart: the page
//! boots the new plan from initial state (`exact_boot_plan`), with no state
//! migration and no patch format — D5 taken literally. `host/web/dev.mjs`
//! runs [`main`] for an app, pushes each ready plan to the page, and prints
//! the edit → present numbers against `rules/RULES.md`'s row.

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
}

/// A watch on one source file.
#[derive(Debug)]
pub struct Session {
    source: PathBuf,
    out: PathBuf,
    last: Option<String>,
    stamp: Option<(SystemTime, u64)>,
    surfaces_stamp: Option<(SystemTime, u64)>,
    failed: bool,
    source_map: bool,
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
            failed: false,
            source_map: true,
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
        let surfaces_changed = self.surfaces_stamp != surfaces_stamp;
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
        self.last = Some(src.clone());
        self.surfaces_stamp = surfaces_stamp;
        let saved = surfaces_stamp.map_or(stamp.0, |s| s.0.max(stamp.0));
        let built = self.build::<D>(&src, unix_ms(saved));
        self.failed = built.is_err();
        Some(built)
    }

    fn build<D: DataSource + Default>(&self, src: &str, saved_ms: f64) -> Result<Built, String> {
        let t = Instant::now();
        // Compile exactly the snapshot `poll` compared with `last`. Reading
        // the path again here can observe the middle of the next save and
        // then suppress its final bytes as already seen.
        let (plan, map) = if self.source_map {
            contract::compile_path_source_mapped(&self.source, src)
                .map(|(plan, map)| (plan, Some(map)))
        } else {
            contract::compile_path_source(&self.source, src).map(|plan| (plan, None))
        }
        .map_err(|e| e.to_string())?;
        let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let baked = contract::bake(plan, D::default()).map_err(|e| {
            map.as_ref().map_or_else(
                || format!("bake: {e:?}"),
                |map| map.bake_error(&e).to_string(),
            )
        })?;
        let bake_ms = t.elapsed().as_secs_f64() * 1000.0;
        let bytes = baked.encode();
        contract::write_development_artifacts(&baked)?;
        // Publish the map first, then its plan. Readers must check the digest
        // because the two complete files cannot be renamed as one operation.
        if let Some(map) = map {
            let mut path = self.out.as_os_str().to_os_string();
            path.push(".map.json");
            write_atomic(&PathBuf::from(path), map.json(&bytes).as_bytes())?;
        }
        // Atomic, and per process: the page never fetches a half-written
        // plan, and two drivers on one file cannot trip over one tmp.
        write_atomic(&self.out, &bytes)?;
        Ok(Built {
            bytes,
            saved_ms,
            compile_ms,
            bake_ms,
            ready_ms: unix_ms(SystemTime::now()),
        })
    }
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
/// stdout — `plan <bytes> <saved_ms> <compile_ms> <bake_ms> <ready_ms>` or
/// `error <message>` — for `dev.mjs` to relay. Polls every 10 ms; a stat
/// is microseconds. `--once` builds the plan a single time and exits —
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
    loop {
        match session.poll::<D>() {
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
                let _ = writeln!(stdout, "error {}", e.replace('\n', " "));
            }
            None => {}
        }
        let _ = stdout.flush();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
