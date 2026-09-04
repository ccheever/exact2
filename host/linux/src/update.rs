//! The update store on Linux (LLP 1026 D9/D11/D12; LLP 1030 D7; LLP
//! 1030.000 §4 stage 4, the client half): `exact_update::Client` behind a
//! lock the check's thread shares with the presenter, the check over
//! ibex2's transport — rustls off Apple, the platform's on a Mac, the
//! executor's own (`executor.rs`) — and its outcome delivered like a reply:
//! one byte on a socketpair the display loop polls beside the executor's,
//! the line on a channel the presenter drains (`Presenter::poll_update`).
//!
//! Where the store lives: `$XDG_DATA_HOME/exact/<app id>/update`, else
//! `~/.local/share/exact/<app id>/update`; `EXACT_UPDATE_DIR` and
//! `EXACT_UPDATE_ORIGIN` as `exact_update::client` documents, and under
//! `EXACT_AGENT=1` a fresh temporary directory. The store is opened before
//! the boot, since the boot is what it selects (`app.rs`); first pixel is
//! the first frame presented (the display loop) or, headless, the boot
//! whole — laid out, its images in — before the agent serves or the frame
//! is painted. While a check holds the store, the presenter answers from
//! the last known status rather than waiting on a download.

use exact_runner::Delivery;
use exact_update::{Client, Status};
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};

/// The head is a pointer card (LLP 1026 D11); a file is a plan or an asset.
const MAX_HEAD_BYTES: usize = exact_update::MAX_ENVELOPE_BYTES;
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

/// The store, the check's thread, and the wake.
pub struct Updates {
    client: Arc<Mutex<Client>>,
    /// The status as of the last store operation, readable while a check
    /// holds the store.
    status: Arc<Mutex<Status>>,
    checking: Arc<AtomicBool>,
    lines: Receiver<String>,
    tx: Sender<String>,
    wake: UnixStream,
    signal: UnixStream,
    /// A boot's note — the selected entry refused at boot — taken once.
    note: Option<String>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The platform's data directory: `$XDG_DATA_HOME`, else `~/.local/share`.
fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".local/share")
}

impl Updates {
    /// Open the store for the binary whose `compat.json` and baked plan are
    /// given, with `assets` as what it embeds by name. Refused when the
    /// binary links no store, or the directory cannot be made.
    pub fn open(compat: &str, plan: &[u8], assets: &Path) -> Result<Updates, String> {
        let client = Client::open(&data_dir(), assets, compat, plan)?;
        let status = client.status();
        let (tx, lines) = channel();
        let (wake, signal) = UnixStream::pair().map_err(|e| format!("the update wake: {e}"))?;
        wake.set_nonblocking(true)
            .map_err(|e| format!("the update wake: {e}"))?;
        Ok(Updates {
            client: Arc::new(Mutex::new(client)),
            status: Arc::new(Mutex::new(status)),
            checking: Arc::new(AtomicBool::new(false)),
            lines,
            tx,
            wake,
            signal,
            note: None,
        })
    }

    /// The store's directory.
    pub fn dir(&self) -> PathBuf {
        lock(&self.client).dir().to_path_buf()
    }

    /// The selected entry's name and plan bytes; `None` for entry zero.
    pub fn selected_plan(&self) -> Option<(String, Vec<u8>)> {
        lock(&self.client).selected_plan()
    }

    /// The selection is booting: count it (LLP 1026 D11).
    pub fn boot_started(&mut self) {
        let mut c = lock(&self.client);
        if let Err(e) = c.boot_started() {
            self.note = Some(format!("exact update: {e}"));
        }
        *lock(&self.status) = c.status();
    }

    /// The selected entry's plan was refused at boot; entry zero boots.
    pub fn entry_refused(&mut self, entry: &str, why: &str) {
        let note = lock(&self.client).entry_refused(entry, why);
        eprintln!("{note}");
        self.note = Some(note);
    }

    /// First pixel: the selection that booted is good (LLP 1026 D11).
    pub fn boot_succeeded(&mut self) {
        let mut c = lock(&self.client);
        if let Err(e) = c.boot_succeeded() {
            self.note = Some(format!("exact update: {e}"));
        }
        *lock(&self.status) = c.status();
    }

    /// A boot's note for the journal, taken once.
    pub fn take_note(&mut self) -> Option<String> {
        self.note.take()
    }

    /// The wake's reading end: readable when a check ended (for `poll`).
    pub fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
    }

    /// Start a check on its own thread; `false` when one is running.
    pub fn check(&self) -> bool {
        if self.checking.swap(true, Ordering::SeqCst) {
            return false;
        }
        let client = Arc::clone(&self.client);
        let status = Arc::clone(&self.status);
        let checking = Arc::clone(&self.checking);
        let tx = self.tx.clone();
        let Ok(mut signal) = self.signal.try_clone() else {
            self.checking.store(false, Ordering::SeqCst);
            return false;
        };
        let spawned = std::thread::Builder::new()
            .name("exact-update".into())
            .spawn(move || {
                let line = run_check(&client, &status);
                checking.store(false, Ordering::SeqCst);
                let _ = tx.send(line);
                let _ = signal.write_all(&[1]);
            });
        if spawned.is_err() {
            self.checking.store(false, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// A finished check's line, when one landed since the last take —
    /// `current` · `staged seq N` · `refused: …`; the wake bytes go with it.
    pub fn take_line(&mut self) -> Option<String> {
        let mut buf = [0u8; 64];
        while let Ok(n) = (&self.wake).read(&mut buf) {
            if n == 0 {
                break;
            }
        }
        self.lines.try_recv().ok()
    }

    /// The staged plan's bytes and its assets directory, for an activation
    /// now; `None` when nothing is staged or a check holds the store.
    pub fn activate(&self) -> Option<(Vec<u8>, PathBuf)> {
        let mut c = self.client.try_lock().ok()?;
        let taken = c.activate()?;
        *lock(&self.status) = c.status();
        Some(taken)
    }

    /// What the store has to say, into a runner's delivery facts (LLP 1030
    /// D7): the stream, the running and embedded `seq`, whether an entry is
    /// staged, the sunset; the binary's own three are left as they were.
    pub fn status_into(&self, delivery: &mut Delivery) {
        let s = lock(&self.status);
        delivery.stream = s.stream.clone();
        delivery.seq = s.running_seq;
        delivery.embedded_seq = s.embedded_seq;
        delivery.staged = s.staged;
        delivery.sunset = s.sunset.as_ref().map(|c| c.message.clone());
    }
}

/// One check over ibex2's transport, on the calling thread: the outcome's
/// line. Every URL is bounded while the bytes arrive — the head at its
/// envelope ceiling, a file at the plan's.
fn run_check(client: &Mutex<Client>, status: &Mutex<Status>) -> String {
    let transport = ibex2::transport::default_transport();
    let mut c = lock(client);
    let head = c.head_url().map(str::to_string);
    let mut fetch = |url: &str| -> Result<Vec<u8>, String> {
        let limit = if head.as_deref() == Some(url) {
            MAX_HEAD_BYTES
        } else {
            MAX_FILE_BYTES
        };
        let mut req = ibex2::stdlib::fetch::Request::get(url);
        req.headers.set("cache-control", "no-cache");
        req.max_body = Some(limit);
        let r = transport.send(&req).map_err(|e| format!("{url}: {e}"))?;
        if r.status != 200 {
            return Err(format!("{url}: HTTP {}", r.status));
        }
        Ok(r.body)
    };
    let outcome = c.check(&mut fetch);
    *lock(status) = c.status();
    outcome.to_string()
}
