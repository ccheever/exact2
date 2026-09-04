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
//! is painted. While a check downloads without the store lock, the presenter answers from
//! the last known status rather than waiting on a download.

use exact_runner::Delivery;
use exact_update::{Client, Generation, PreparedSelection, Status};
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
    /// downloads.
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
        Updates::from_client(client)
    }

    /// Wrap an already-open client in the host's status and wake plumbing.
    /// Kept separate from [`open`](Self::open) so launch precedence can be
    /// exercised against a real selected store without process environment.
    pub(crate) fn from_client(client: Client) -> Result<Updates, String> {
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

    /// The selected generation's verified plan and complete asset roster;
    /// `None` for entry zero or after a durable launch-time refusal.
    pub fn prepare_selected(&mut self) -> Option<PreparedSelection> {
        let result = lock(&self.client).prepare_selected();
        match result {
            Ok(prepared) => prepared,
            Err(refusal) => {
                let line = format!("exact update: {refusal}; booted entry zero");
                eprintln!("{line}");
                *lock(&self.status) = *refusal.status;
                self.note = Some(line);
                None
            }
        }
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
        let (note, status) = lock(&self.client).entry_refused(entry, why);
        eprintln!("{note}");
        self.note = Some(note);
        *lock(&self.status) = status;
    }

    /// A selected asset failed its signed card during the boot transaction;
    /// durably discard that generation and refresh the cached fallback facts.
    pub fn selection_corrupt(&mut self, generation: &Generation, why: &str) {
        let refusal = lock(&self.client).selection_corrupt(generation, why);
        let line = format!("exact update: {refusal}; booted entry zero");
        eprintln!("{line}");
        *lock(&self.status) = *refusal.status;
        self.note = Some(line);
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
    /// now; `None` when nothing is staged. A concurrent download cannot
    /// suppress activation: this lock covers local store operations only.
    pub fn activate(&self) -> Option<(Vec<u8>, PathBuf)> {
        let mut c = lock(&self.client);
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
    run_check_with(
        client,
        status,
        ibex2::transport::default_transport().as_ref(),
    )
}

fn run_check_with(
    client: &Mutex<Client>,
    status: &Mutex<Status>,
    transport: &dyn ibex2::stdlib::fetch::Transport,
) -> String {
    let request = lock(client).begin_check();
    let downloaded = request.map(|request| {
        let head = request.head_url().to_string();
        request.fetch(&mut |url| fetch_one(transport, Some(&head), url))
    });
    let mut c = lock(client);
    let outcome = match downloaded {
        Ok(downloaded) => c.finish_check(downloaded),
        Err(why) => exact_update::Outcome::Refused(why),
    };
    *lock(status) = c.status();
    outcome.to_string()
}

/// Fetch one update object. Update cards are same-origin, immutable objects;
/// a redirect is an answer to refuse, never authority to make another request.
fn fetch_one(
    transport: &dyn ibex2::stdlib::fetch::Transport,
    head: Option<&str>,
    url: &str,
) -> Result<Vec<u8>, String> {
    let limit = if head == Some(url) {
        MAX_HEAD_BYTES
    } else {
        MAX_FILE_BYTES
    };
    let mut req = ibex2::stdlib::fetch::Request::get(url);
    req.redirect = ibex2::stdlib::fetch::RedirectMode::Manual;
    req.headers.set("cache-control", "no-cache");
    req.max_body = Some(limit);
    let r = transport.send(&req).map_err(|e| format!("{url}: {e}"))?;
    if r.status != 200 {
        return Err(format!("{url}: HTTP {}", r.status));
    }
    Ok(r.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibex2::boundary::HostError;
    use ibex2::stdlib::fetch::{Headers, Request, Response, Transport};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const COMPAT: &str = r#"{"id":"abc","inputs":{"app":"com.exact.host-cache","keys":null,"trust":"development","store":{"L":"A"}},"delivery":{"activate":"next-launch","channel":"prod","origin":"https://updates.example"}}"#;

    struct RedirectTransport {
        requests: AtomicUsize,
    }

    impl Transport for RedirectTransport {
        fn send(&self, request: &Request) -> Result<Response, HostError> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.redirect, ibex2::stdlib::fetch::RedirectMode::Manual);
            let mut headers = Headers::new();
            headers.set_response("location", "https://attacker.example/entry.json");
            Ok(Response {
                status: 302,
                status_text: "Found".into(),
                headers,
                body: b"not an update".to_vec(),
                url: request.url.clone(),
                redirected: false,
            })
        }
    }

    struct StalledTransport {
        block_head: bool,
        entered: std::sync::mpsc::Sender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        head: Vec<u8>,
        plan: Vec<u8>,
    }

    impl Transport for StalledTransport {
        fn send(&self, request: &Request) -> Result<Response, HostError> {
            let head = request.url.ends_with("exact.json");
            if head == self.block_head {
                self.entered.send(()).unwrap();
                lock(&self.release).recv().unwrap();
            }
            Ok(Response {
                status: 200,
                status_text: "OK".into(),
                headers: Headers::new(),
                body: if head {
                    self.head.clone()
                } else {
                    self.plan.clone()
                },
                url: request.url.clone(),
                redirected: false,
            })
        }
    }

    fn head(seq: u64, plan: &[u8]) -> Vec<u8> {
        format!(r#"{{"exact":1,"app":{{"id":"com.exact.host-cache"}},"plan":{{"url":"./app.plan","sha256":"{}","bytes":{}}},"assets":[],"stream":{{"channel":"prod","compatibilityId":"abc","seq":{seq}}}}}"#, exact_update::sha256_hex(plan), plan.len()).into_bytes()
    }

    #[test]
    fn a_stalled_check_allows_selection_session_boot_and_activation() {
        for block_head in [true, false] {
            let base = std::env::temp_dir().join(format!(
                "exact-linux-stalled-{}-{block_head}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&base);
            let plan = caltrain::build().unwrap().encode();
            let mut client = Client::open(&base, &base, COMPAT, b"embedded plan").unwrap();
            assert!(matches!(
                client.check(&mut |url| Ok(if url.ends_with("exact.json") {
                    head(1, &plan)
                } else {
                    plan.clone()
                })),
                exact_update::Outcome::Staged { seq: 1, .. }
            ));
            let mut updates = Updates::from_client(client).unwrap();
            let client = Arc::clone(&updates.client);
            let status = Arc::clone(&updates.status);
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let transport = StalledTransport {
                block_head,
                entered: entered_tx,
                release: Mutex::new(release_rx),
                head: head(2, b"new plan"),
                plan: b"new plan".to_vec(),
            };
            let checking = std::thread::spawn(move || run_check_with(&client, &status, &transport));
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let (responsive_tx, responsive_rx) = std::sync::mpsc::channel();
            let foreground = std::thread::spawn(move || {
                assert!(!updates.dir().as_os_str().is_empty());
                let (bytes, _) = updates.activate().unwrap();
                assert!(updates.activate().is_none());
                let prepared = updates.prepare_selected().unwrap();
                assert_eq!(prepared.plan.as_ref(), bytes);
                let (session, _) = crate::host::Host::boot(
                    &bytes,
                    caltrain_data::Caltrain,
                    Box::<exact_kernel::MonospaceMeasurer>::default(),
                    390.0,
                    844.0,
                )
                .unwrap();
                assert!(session.kernel().live_count() > 0);
                updates.boot_started();
                updates.boot_succeeded();
                let mut delivery = Delivery::default();
                updates.status_into(&mut delivery);
                responsive_tx
                    .send((updates, delivery.seq, delivery.staged))
                    .unwrap();
            });
            let response = responsive_rx.recv_timeout(std::time::Duration::from_secs(2));
            release_tx.send(()).unwrap();
            foreground.join().unwrap();
            assert_eq!(checking.join().unwrap(), "staged seq 2");
            let (updates, seq, staged) = response.unwrap();
            assert_eq!((seq, staged), (1, false));
            let mut delivery = Delivery::default();
            updates.status_into(&mut delivery);
            assert_eq!(delivery.seq, 1);
            assert!(delivery.staged);
            drop(updates);
            let _ = std::fs::remove_dir_all(base);
        }
    }

    fn stale_status() -> Status {
        Status {
            stream: "prod/abc".into(),
            selected_seq: 4,
            running_seq: 4,
            embedded_seq: 0,
            staged: false,
            sunset: None,
            entry: Some("stale-entry".into()),
        }
    }

    #[test]
    fn update_fetch_refuses_redirect_without_following_location() {
        let transport = RedirectTransport {
            requests: AtomicUsize::new(0),
        };
        let head = "https://updates.example/apps/demo/head.json";

        let error = fetch_one(&transport, Some(head), head).unwrap_err();

        assert_eq!(error, format!("{head}: HTTP 302"));
        assert_eq!(transport.requests.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn boot_refusal_refreshes_the_cached_delivery_stream() {
        let base =
            std::env::temp_dir().join(format!("exact-linux-refusal-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let client = Client::open(&base, &base, COMPAT, b"embedded plan").unwrap();
        let (tx, lines) = channel();
        let (wake, signal) = UnixStream::pair().unwrap();
        let mut updates = Updates {
            client: Arc::new(Mutex::new(client)),
            status: Arc::new(Mutex::new(stale_status())),
            checking: Arc::new(AtomicBool::new(false)),
            lines,
            tx,
            wake,
            signal,
            note: None,
        };

        updates.entry_refused("stale-entry", "plan refused");

        let mut delivery = Delivery::default();
        updates.status_into(&mut delivery);
        assert_eq!(delivery.stream, "embedded");
        assert_eq!(delivery.seq, 0);
        assert!(updates.take_note().unwrap().contains("booted entry zero"));
        drop(updates);
        let _ = std::fs::remove_dir_all(base);
    }
}
