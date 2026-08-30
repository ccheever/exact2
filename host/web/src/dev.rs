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
        }
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
        if self.stamp == Some(stamp) {
            return None;
        }
        self.stamp = Some(stamp);
        let src = match std::fs::read_to_string(&self.source) {
            Ok(s) => s,
            Err(e) => return Some(Err(format!("{}: {e}", self.source.display()))),
        };
        if self.last.as_deref() == Some(src.as_str()) {
            return None;
        }
        self.last = Some(src.clone());
        Some(self.build::<D>(&src, unix_ms(stamp.0)))
    }

    fn build<D: DataSource + Default>(&self, src: &str, saved_ms: f64) -> Result<Built, String> {
        let t = Instant::now();
        let _ = src;
        let plan = contract::compile_path(&self.source)
            .map_err(|e| format!("{}:{e}", self.source.display()))?;
        let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let baked = contract::bake(plan, D::default()).map_err(|e| format!("bake: {e:?}"))?;
        let bake_ms = t.elapsed().as_secs_f64() * 1000.0;
        let bytes = baked.encode();
        // Atomic, and per process: the page never fetches a half-written
        // plan, and two drivers on one file cannot trip over one tmp.
        let tmp = self
            .out
            .with_extension(format!("plan.{}.tmp", std::process::id()));
        std::fs::write(&tmp, &bytes)
            .and_then(|_| std::fs::rename(&tmp, &self.out))
            .map_err(|e| format!("{}: {e}", self.out.display()))?;
        Ok(Built {
            bytes,
            saved_ms,
            compile_ms,
            bake_ms,
            ready_ms: unix_ms(SystemTime::now()),
        })
    }
}

/// `dev <source.contract> <out.plan>`: watch forever, one line per event on
/// stdout — `plan <bytes> <saved_ms> <compile_ms> <bake_ms> <ready_ms>` or
/// `error <message>` — for `dev.mjs` to relay. Polls every 10 ms; a stat
/// is microseconds.
pub fn main<D: DataSource + Default>() -> std::process::ExitCode {
    use std::io::Write as _;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(source), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: dev <source.contract> <out.plan>");
        return std::process::ExitCode::from(2);
    };
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
