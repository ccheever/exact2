//! The app's journal on stderr, for a phone nobody drives (`EXACT_LOG_JOURNAL=1`,
//! the reader's `x_log_journal=1`): each line as the host comes to hold it,
//! with the milliseconds since its boot began. On Android stderr is logcat,
//! whose own timestamps place the lines against the process's start, which is
//! how what a data module adds to a launch is measured (when it was asked,
//! when it answered). Without the switch nothing is read or written.

use super::CanvasHost;
use exact_runner::DataSource;
use std::sync::atomic::{AtomicUsize, Ordering};

static ON: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| std::env::var("EXACT_LOG_JOURNAL").is_ok_and(|v| v == "1"));

/// Journal lines already written (a process has one host).
static WRITTEN: AtomicUsize = AtomicUsize::new(0);

impl<D: DataSource + Default> CanvasHost<D> {
    /// Write the journal's lines since the last call, when the switch is on.
    pub(super) fn echo_journal(&self) {
        if !*ON {
            return;
        }
        let runner = self.p.host().runner();
        let start = runner.journal_start();
        let from = WRITTEN.load(Ordering::Relaxed).max(start);
        let ms = self.started.elapsed().as_secs_f64() * 1000.0;
        let mut next = from;
        for line in runner.journal().skip(from - start) {
            eprintln!("exact: journal +{ms:.1} ms: {line}");
            next += 1;
        }
        WRITTEN.store(next, Ordering::Relaxed);
    }
}
