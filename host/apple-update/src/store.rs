//! The update store on Apple platforms (LLP 1026 D9/D11/D12; LLP 1030 D7;
//! LLP 1030.000 §4 stage 4, the client half).
//!
//! **One store per process.** The app's container holds it and every
//! runtime in the process boots from the same selection, so it lives behind
//! a process-wide lock rather than in a runtime's bridge, and the C entries
//! that touch it take no handle — except `exact_update_sync`, which tells
//! one runtime's runner what the store has to say. The runtime registry is
//! thread-local besides, which is the other reason: the check runs on a
//! thread of its own and could not address a runtime from there.
//!
//! **What runs where.** `open`, `select`, the boot marks, `activate`, and
//! `sync` are main-thread calls that hold the lock only for local work. The
//! check ([`check`]) is a thread: it fetches over ibex2's transport — the
//! same `NSURLSession` the executor's requests use (LLP 1016 D2) —
//! verifies, writes the entry whole, and reports through the callback the
//! host gave, carrying one line; the host hops to its main thread and calls
//! `exact_update_sync` for each runtime. So the runner never does I/O, the
//! presenter never sees a request, and Swift holds no networking for
//! updates at all. While a check downloads without the store lock, the main thread answers
//! from the last known status ([`Snapshot`]) rather than waiting on a
//! download.
//!
//! The buffer discipline is the runtime's (`exact_in`/`exact_out`) for
//! calls that have no runtime: `exact_update_in(len)` hands out the input
//! buffer a path payload is written into, every call answers with a length,
//! and `exact_update_out()` is the output's address. Nothing here is
//! `unsafe`; the one pointer that leaves is the line a finished check hands
//! the callback, alive for that call only.

use exact_runner::agent::field_str;
use exact_runner::Delivery;
use exact_update::{Activate, Client, PreparedSelection, Status};
use std::ffi::c_void;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

/// The host's callback for a finished check: called on the check's thread
/// with `ctx` and one UTF-8 line — `current` · `staged seq N` · `refused: …`
/// — that lives only for the call.
pub type DoneFn = extern "C" fn(ctx: *mut c_void, line: *const u8, len: usize);

/// The head is a pointer card (LLP 1026 D11); a file is a plan or an asset.
const MAX_HEAD_BYTES: usize = exact_update::MAX_ENVELOPE_BYTES;
const MAX_FILE_BYTES: usize = 64 * 1024 * 1024;

/// The one store, once opened.
static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

#[derive(Clone)]
struct Pinned {
    token: u64,
    selection: PreparedSelection,
}
struct Generations {
    next: u64,
    initialized: bool,
    live: Option<Pinned>,
    candidate: Option<Pinned>,
}
static GENERATIONS: Mutex<Generations> = Mutex::new(Generations {
    next: 1,
    initialized: false,
    live: None,
    candidate: None,
});

fn pin(state: &mut Generations, selection: PreparedSelection) -> Pinned {
    let token = state.next;
    state.next = state
        .next
        .checked_add(1)
        .expect("update generation token exhausted");
    Pinned { token, selection }
}

fn quote(value: &str) -> String {
    let mut out = String::new();
    exact_runner::agent::quote(value, &mut out);
    out
}

fn descriptor(pinned: Option<&Pinned>) -> Vec<u8> {
    match pinned {
        Some(p) => format!(
            "{{\"token\":{},\"entry\":{},\"seq\":{},\"assets\":[{}]}}",
            p.token,
            p.selection
                .generation
                .entry
                .as_deref()
                .map(quote)
                .unwrap_or_else(|| "null".into()),
            p.selection.generation.seq,
            p.selection
                .assets
                .names()
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join(",")
        )
        .into_bytes(),
        None => format!(
            "{{\"token\":0,\"entry\":null,\"seq\":{},\"assets\":[]}}",
            lock(&SNAPSHOT)
                .status
                .as_ref()
                .map_or(0, |s| s.embedded_seq)
        )
        .into_bytes(),
    }
}

fn initial_selection() -> Option<Pinned> {
    let mut client = lock(&CLIENT);
    let mut generations = lock(&GENERATIONS);
    if !generations.initialized {
        generations.initialized = true;
        if let Some(client) = client.as_mut() {
            match client.prepare_selected() {
                Ok(Some(selection)) => generations.live = Some(pin(&mut generations, selection)),
                Ok(None) => {}
                Err(refusal) => {
                    let mut snapshot = lock(&SNAPSHOT);
                    snapshot.note = Some(format!("exact update: {refusal}; booted entry zero"));
                    snapshot.status = Some(*refusal.status);
                }
            }
        }
    }
    generations.live.clone()
}

fn pinned(token: u64) -> Option<Pinned> {
    let g = lock(&GENERATIONS);
    g.candidate
        .as_ref()
        .filter(|p| p.token == token)
        .or_else(|| g.live.as_ref().filter(|p| p.token == token))
        .cloned()
}

/// The cheap facts, readable while a check downloads.
struct Snapshot {
    /// The status as of the last store operation.
    status: Option<Status>,
    activate: Activate,
    /// The last check's line, for the journal: `exact update: …`.
    line: Option<String>,
    /// A boot's note — the selected entry refused at boot — for the
    /// journal of the runner that booted instead.
    note: Option<String>,
    checking: bool,
}

static SNAPSHOT: Mutex<Snapshot> = Mutex::new(Snapshot {
    status: None,
    activate: Activate::NextLaunch,
    line: None,
    note: None,
    checking: false,
});

static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// A lock that survives a panic on another thread: the store's state is
/// whole between operations, so the last holder's panic loses nothing.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn refresh(client: &Client) {
    let mut snap = lock(&SNAPSHOT);
    snap.status = Some(client.status());
    snap.activate = client.activate_policy();
}

/// Resize the input buffer; its address.
pub fn input(len: usize) -> *mut u8 {
    let mut input = lock(&INPUT);
    input.clear();
    input.resize(len, 0);
    input.as_mut_ptr()
}

/// The output buffer's address: the last call's answer.
pub fn output_ptr() -> *const u8 {
    lock(&OUTPUT).as_ptr()
}

fn emit(bytes: Vec<u8>) -> u32 {
    let mut out = lock(&OUTPUT);
    *out = bytes;
    out.len() as u32
}

fn input_text(len: usize) -> String {
    let input = lock(&INPUT);
    String::from_utf8_lossy(&input[..len.min(input.len())]).into_owned()
}

/// Open the store (`exact_update_open`): the input buffer's first `len`
/// bytes are `{"base":"<the platform's data directory>","assets":"<the
/// asset root>"}`; the store is `<base>/exact/<app id>/update` (or the dev
/// overrides `exact_update::client` names). `compat` and `plan` are the
/// binary's. Returns 0, or the refusal's length in the output buffer — a
/// binary that links no store, or a container that cannot be written, runs
/// without one and answers the embedded facts.
pub fn open(len: usize, compat: &str, plan: &[u8]) -> u32 {
    let request = input_text(len);
    let (Some(base), Some(assets)) = (field_str(&request, "base"), field_str(&request, "assets"))
    else {
        return emit(b"exact_update_open: the payload names no base and assets".to_vec());
    };
    match Client::open(Path::new(&base), Path::new(&assets), compat, plan) {
        Ok(client) => {
            refresh(&client);
            *lock(&CLIENT) = Some(client);
            let mut generations = lock(&GENERATIONS);
            generations.initialized = false;
            generations.live = None;
            generations.candidate = None;
            0
        }
        Err(e) => emit(e.into_bytes()),
    }
}

/// The selection (`exact_update_select`): one JSON line,
/// `{"entry":…|null,"seq":N,"plan":"…","assets":"…"}`, the paths empty for
/// entry zero and when no store is open.
pub fn select() -> u32 {
    emit(descriptor(initial_selection().as_ref()))
}

/// The immutable plan bytes pinned by a selection or activation token.
pub fn plan(token: u64) -> u32 {
    emit(
        pinned(token)
            .map(|p| p.selection.plan.to_vec())
            .unwrap_or_default(),
    )
}

/// A selected boot uses the verified bytes pinned once for the whole app.
pub fn selected_plan() -> Option<(String, Vec<u8>)> {
    let p = initial_selection()?;
    Some((p.selection.generation.entry?, p.selection.plan.to_vec()))
}

/// A selected direct C boot admits the same signed Rust pair as the Swift composition.
pub fn selected_module() -> Result<Option<(String, Vec<u8>)>, String> {
    let Some(pinned) = initial_selection() else {
        return Ok(None);
    };
    let assets = &pinned.selection.assets;
    let names = assets.names();
    let rust: Vec<_> = names
        .iter()
        .filter(|name| name.starts_with("rust/"))
        .collect();
    if rust.is_empty() {
        return Ok(None);
    }
    let receipt = assets
        .resolve("rust/app.module.json")?
        .ok_or("missing Rust pairing receipt")?;
    if receipt.len() > 1 << 20 {
        return Err("Rust receipt exceeds 1 MiB".into());
    }
    let receipt = String::from_utf8(receipt.to_vec()).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&receipt).map_err(|e| e.to_string())?;
    let file = json["module"]["file"].as_str().unwrap_or("");
    if rust.len() != 2
        || ![
            "app.module.wasm",
            "app.module.dylib",
            "app.module.so",
            "app.module.dll",
            "app.module.bin",
        ]
        .contains(&file)
    {
        return Err("invalid signed Rust replacement pair".into());
    }
    let bytes = assets
        .resolve(&format!("rust/{file}"))?
        .ok_or("missing Rust module")?;
    if bytes.len() > 32 << 20 {
        return Err("Rust replacement exceeds 32 MiB".into());
    }
    Ok(Some((receipt, bytes.to_vec())))
}

/// One signed asset: status byte 0 absent, 1 verified bytes, 2 refusal text.
/// The consumer receives immutable bytes, never a path it must reopen.
pub fn asset(token: u64, len: usize) -> u32 {
    let name = input_text(len);
    let answer = pinned(token)
        .ok_or_else(|| "unknown update generation".to_string())
        .and_then(|p| p.selection.assets.resolve(&name));
    let mut out = Vec::new();
    match answer {
        Ok(Some(bytes)) => {
            out.push(1);
            out.extend_from_slice(&bytes);
        }
        Ok(None) => out.push(0),
        Err(why) => {
            out.push(2);
            out.extend_from_slice(why.as_bytes());
        }
    }
    emit(out)
}

/// Discard a corrupt initial generation before it becomes visible. A staged
/// candidate refusal leaves every committed generation unchanged.
pub fn refuse(token: u64, len: usize) -> u32 {
    let why = input_text(len);
    let mut client = lock(&CLIENT);
    let mut g = lock(&GENERATIONS);
    if g.candidate.as_ref().is_some_and(|p| p.token == token) {
        g.candidate = None;
        return 0;
    }
    if g.live.as_ref().is_some_and(|p| p.token == token) {
        let p = g.live.take().unwrap();
        if let Some(client) = client.as_mut() {
            let refusal = client.selection_corrupt(&p.selection.generation, why);
            refresh(client);
            lock(&SNAPSHOT).note = Some(format!("exact update: {refusal}; booted entry zero"));
        }
    }
    0
}

/// The selection is booting: count it (LLP 1026 D11), once per process.
pub fn boot_started() {
    let mut guard = lock(&CLIENT);
    if let Some(c) = guard.as_mut() {
        if let Err(e) = c.boot_started() {
            lock(&SNAPSHOT).note = Some(format!("exact update: {e}"));
        }
        refresh(c);
    }
}

/// The selected entry's plan was refused at boot; entry zero boots instead.
pub fn entry_refused(entry: &str, why: &str) {
    let mut guard = lock(&CLIENT);
    lock(&GENERATIONS).live = None;
    if let Some(c) = guard.as_mut() {
        let (note, status) = c.entry_refused(entry, why);
        let mut snap = lock(&SNAPSHOT);
        snap.status = Some(status);
        snap.note = Some(note);
    }
}

/// Count only a generation the app actually accepted, once per process.
pub fn started(token: u64) {
    let Some(p) = pinned(token) else { return };
    let mut client = lock(&CLIENT);
    if let Some(client) = client.as_mut() {
        if client.generation() == p.selection.generation {
            if let Err(error) = client.boot_started() {
                lock(&SNAPSHOT).note = Some(error);
            }
            refresh(client);
        }
    }
}

/// First pixel (`exact_update_boot_succeeded`): the selection that booted
/// is good, once per process.
pub fn boot_succeeded(token: u64) {
    let Some(p) = pinned(token) else { return };
    let mut guard = lock(&CLIENT);
    if let Some(c) = guard.as_mut() {
        if let Err(e) = c.boot_succeeded(&p.selection.generation) {
            lock(&SNAPSHOT).note = Some(format!("exact update: {e}"));
        }
        refresh(c);
    }
}

/// Start a check (`exact_update_check`) on its own thread; `done` is called
/// there when it ends. Returns 0 when started, 1 when a check is already
/// running, 2 when no store is open (the line then never comes).
pub fn check(done: Option<DoneFn>, ctx: *mut c_void) -> u32 {
    if lock(&CLIENT).is_none() {
        return 2;
    }
    {
        let mut snap = lock(&SNAPSHOT);
        if snap.checking {
            return 1;
        }
        snap.checking = true;
    }
    // The context crosses to the thread as an integer: the host's, opaque
    // here, handed back untouched (the executor's wake does the same).
    let ctx = ctx as usize;
    let spawned = std::thread::Builder::new()
        .name("exact-update".into())
        .spawn(move || {
            let line = run_check();
            lock(&SNAPSHOT).checking = false;
            if let Some(f) = done {
                f(ctx as *mut c_void, line.as_ptr(), line.len());
            }
        });
    if spawned.is_err() {
        lock(&SNAPSHOT).checking = false;
        return 1;
    }
    0
}

/// One check, on the calling thread, over ibex2's transport: the outcome's
/// line. Every URL the store asks for is bounded — the head at its
/// envelope ceiling, a file at the plan's — while the bytes arrive.
fn run_check() -> String {
    run_check_with(ibex2::transport::default_transport().as_ref())
}

fn run_check_with(transport: &dyn ibex2::stdlib::fetch::Transport) -> String {
    let request = {
        let guard = lock(&CLIENT);
        guard
            .as_ref()
            .ok_or_else(|| "no store is open".to_string())
            .and_then(Client::begin_check)
    };
    let downloaded = request.map(|request| {
        let head = request.head_url().to_string();
        request.fetch(&mut |url| fetch_one(transport, Some(&head), url))
    });
    let mut guard = lock(&CLIENT);
    let outcome = match (guard.as_mut(), downloaded) {
        (Some(client), Ok(downloaded)) => client.finish_check(downloaded),
        (_, Err(why)) => exact_update::Outcome::Refused(why),
        (None, _) => exact_update::Outcome::Refused("no store is open".into()),
    };
    let line = outcome.to_string();
    let mut snap = lock(&SNAPSHOT);
    snap.status = guard.as_ref().map(Client::status);
    snap.line = Some(format!("exact update: {line}"));
    line
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

/// Pin the staged generation for app-wide preparation, without committing it.
pub fn prepare() -> u32 {
    let client = lock(&CLIENT);
    let result = client
        .as_ref()
        .ok_or_else(|| "no store is open".to_string())
        .and_then(Client::prepare_activation);
    match result {
        Ok(Some(selection)) => {
            let mut g = lock(&GENERATIONS);
            g.candidate = Some(pin(&mut g, selection));
            emit(descriptor(g.candidate.as_ref()))
        }
        Ok(None) => emit(descriptor(None)),
        Err(error) => emit(format!("{{\"error\":{}}}", quote(&error)).into_bytes()),
    }
}

/// Commit the exact candidate all sessions accepted. Zero succeeds; a refusal
/// string leaves the old running selection, assets and candidate intact.
pub fn commit(token: u64) -> u32 {
    let mut client = lock(&CLIENT);
    let mut g = lock(&GENERATIONS);
    let Some(p) = g.candidate.as_ref().filter(|p| p.token == token) else {
        return emit(b"no matching update candidate".to_vec());
    };
    let Some(client) = client.as_mut() else {
        return emit(b"no store is open".to_vec());
    };
    if let Err(error) = client.commit_activation(&p.selection.generation) {
        return emit(error.into_bytes());
    }
    g.live = g.candidate.take();
    g.initialized = true;
    refresh(client);
    0
}

/// Drop an uncommitted candidate.
pub fn discard(token: u64) {
    let mut g = lock(&GENERATIONS);
    if g.candidate.as_ref().is_some_and(|p| p.token == token) {
        g.candidate = None;
    }
}

/// What the store has to say, into a runner's delivery facts (LLP 1030
/// D7): the stream, the running and embedded `seq`, whether an entry is
/// staged, the sunset. The binary's own three (`with_compat`) are left as
/// they were; with no store open nothing changes.
pub fn status_into(delivery: &mut Delivery) {
    let snap = lock(&SNAPSHOT);
    if let Some(s) = &snap.status {
        delivery.stream = s.stream.clone();
        delivery.seq = s.running_seq;
        delivery.embedded_seq = s.embedded_seq;
        delivery.staged = s.staged;
        delivery.sunset = s.sunset.as_ref().map(|c| c.message.clone());
    }
}

/// Candidate facts used only by a prepared runner. The shared status stays live.
pub fn prepared_delivery(token: u64, compat: &str) -> Option<Delivery> {
    let pinned = pinned(token)?;
    let mut delivery = Delivery::default().with_compat(compat);
    status_into(&mut delivery);
    if let Some(client) = lock(&CLIENT).as_ref() {
        delivery.stream = format!(
            "{}/{}",
            client.embedded().channel,
            client.embedded().compatibility_id
        );
    }
    delivery.seq = pinned.selection.generation.seq;
    // Both an initial running selection and a committed activation are unstaged.
    delivery.staged = false;
    Some(delivery)
}

/// The last check's journal line, if any.
pub fn last_line() -> Option<String> {
    lock(&SNAPSHOT).line.clone()
}

/// A boot's note for the journal, taken once.
pub fn take_note() -> Option<String> {
    lock(&SNAPSHOT).note.take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibex2::boundary::HostError;
    use ibex2::stdlib::fetch::{Headers, Request, Response, Transport};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_STORE: Mutex<()> = Mutex::new(());

    const COMPAT: &str = r#"{"id":"abc","inputs":{"app":"com.exact.host-cache","keys":null,"store":{"L":"A"},"trust":"development"},"delivery":{"activate":"next-launch","channel":"prod","origin":"https://updates.example"}}"#;
    /// Dev binaries check only a named origin; name the fixture's.
    const ORIGIN: &str = "https://updates.example";

    struct RedirectTransport {
        requests: AtomicUsize,
    }

    impl Transport for RedirectTransport {
        fn open(
            &self,
            request: &Request,
            signal: &ibex2::stdlib::abort::AbortSignal,
        ) -> Result<ibex2::stdlib::fetch::StreamingResponse, HostError> {
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
            }
            .into_stream(request.body_limit(), signal.clone()))
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

    struct StalledTransport {
        block_head: bool,
        entered: std::sync::mpsc::Sender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        head: Vec<u8>,
        plan: Vec<u8>,
    }

    impl Transport for StalledTransport {
        fn open(
            &self,
            request: &Request,
            signal: &ibex2::stdlib::abort::AbortSignal,
        ) -> Result<ibex2::stdlib::fetch::StreamingResponse, HostError> {
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
            }
            .into_stream(request.body_limit(), signal.clone()))
        }
    }

    fn head(seq: u64, plan: &[u8]) -> Vec<u8> {
        format!(r#"{{"exact":1,"app":{{"id":"com.exact.host-cache"}},"plan":{{"url":"./app.plan","sha256":"{}","bytes":{}}},"assets":[],"stream":{{"channel":"prod","compatibilityId":"abc","seq":{seq}}}}}"#, exact_update::sha256_hex(plan), plan.len()).into_bytes()
    }

    #[test]
    fn a_stalled_check_allows_selection_session_boot_and_activation() {
        let _test = lock(&TEST_STORE);
        for block_head in [true, false] {
            let base = std::env::temp_dir().join(format!(
                "exact-apple-stalled-{}-{block_head}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&base);
            let plan = caltrain::build().unwrap().encode();
            let mut client =
                Client::open_at(&base, &base, COMPAT, b"embedded plan", Some(ORIGIN)).unwrap();
            assert!(matches!(
                client.check(&mut |url| Ok(if url.ends_with("exact.json") {
                    head(1, &plan)
                } else {
                    plan.clone()
                })),
                exact_update::Outcome::Staged { seq: 1, .. }
            ));
            refresh(&client);
            *lock(&CLIENT) = Some(client);
            {
                let mut g = lock(&GENERATIONS);
                g.initialized = false;
                g.live = None;
                g.candidate = None;
            }
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel();
            let transport = StalledTransport {
                block_head,
                entered: entered_tx,
                release: Mutex::new(release_rx),
                head: head(2, b"new plan"),
                plan: b"new plan".to_vec(),
            };
            let checking = std::thread::spawn(move || run_check_with(&transport));
            entered_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let (responsive_tx, responsive_rx) = std::sync::mpsc::channel();
            let foreground = std::thread::spawn(move || {
                assert!(prepare() > 0);
                let token = lock(&GENERATIONS).candidate.as_ref().unwrap().token;
                assert_eq!(commit(token), 0);
                assert!(select() > 0);
                let (_, bytes) = selected_plan().unwrap();
                let (_session, batch) = exact_apple::Host::boot(
                    &bytes,
                    caltrain_data::Caltrain,
                    Box::<exact_kernel::MonospaceMeasurer>::default(),
                    390.0,
                    844.0,
                )
                .unwrap();
                assert!(!batch.is_empty());
                assert_eq!(super::plan(token) as usize, bytes.len());
                boot_started();
                boot_succeeded(token);
                let mut delivery = Delivery::default();
                status_into(&mut delivery);
                responsive_tx.send((delivery.seq, delivery.staged)).unwrap();
            });
            let response = responsive_rx.recv_timeout(std::time::Duration::from_secs(2));
            release_tx.send(()).unwrap();
            foreground.join().unwrap();
            assert_eq!(checking.join().unwrap(), "staged seq 2");
            assert_eq!(response.unwrap(), (1, false));
            let mut delivery = Delivery::default();
            status_into(&mut delivery);
            assert_eq!(delivery.seq, 1);
            assert!(delivery.staged);
            *lock(&CLIENT) = None;
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
    fn boot_refusal_refreshes_the_cached_delivery_stream() {
        let _test = lock(&TEST_STORE);
        let base =
            std::env::temp_dir().join(format!("exact-apple-refusal-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let client = Client::open_at(&base, &base, COMPAT, b"embedded plan", Some(ORIGIN)).unwrap();
        *lock(&CLIENT) = Some(client);
        *lock(&SNAPSHOT) = Snapshot {
            status: Some(stale_status()),
            activate: Activate::NextLaunch,
            line: None,
            note: None,
            checking: false,
        };

        entry_refused("stale-entry", "plan refused");

        let mut delivery = Delivery::default();
        status_into(&mut delivery);
        assert_eq!(delivery.stream, "embedded");
        assert_eq!(delivery.seq, 0);
        assert!(take_note().unwrap().contains("booted entry zero"));
        *lock(&CLIENT) = None;
        let _ = std::fs::remove_dir_all(base);
    }
    #[test]
    fn selected_plan_and_assets_are_verified_once_and_shared_by_later_sessions() {
        let _test = lock(&TEST_STORE);
        let base = std::env::temp_dir().join(format!("exact-apple-pinned-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let plan_bytes = b"verified plan";
        let asset_bytes = b"verified asset";
        let header = String::from_utf8(head(1, plan_bytes)).unwrap().replace(
            "\"assets\":[]", &format!("\"assets\":[{{\"name\":\"assets/a\",\"url\":\"./assets/a\",\"sha256\":\"{}\",\"bytes\":{}}}]", exact_update::sha256_hex(asset_bytes), asset_bytes.len()));
        let mut client = Client::open_at(&base, &base, COMPAT, b"embedded", Some(ORIGIN)).unwrap();
        assert!(matches!(
            client.check(&mut |url| Ok(if url.ends_with("exact.json") {
                header.as_bytes().to_vec()
            } else if url.ends_with("app.plan") {
                plan_bytes.to_vec()
            } else {
                asset_bytes.to_vec()
            })),
            exact_update::Outcome::Staged { .. }
        ));
        let selection = client.selection();
        drop(client);
        let client = Client::open_at(&base, &base, COMPAT, b"embedded", Some(ORIGIN)).unwrap();
        refresh(&client);
        *lock(&CLIENT) = Some(client);
        {
            let mut g = lock(&GENERATIONS);
            g.initialized = false;
            g.live = None;
            g.candidate = None;
        }
        let selected = initial_selection().unwrap();
        assert_eq!(selected.selection.plan.as_ref(), plan_bytes);
        std::fs::write(selection.plan.as_ref().unwrap(), b"changed plan").unwrap();
        assert_eq!(selected_plan().unwrap().1, plan_bytes);
        assert_eq!(initial_selection().unwrap().token, selected.token);
        *lock(&INPUT) = b"assets/a".to_vec();
        asset(selected.token, 8);
        assert_eq!(&lock(&OUTPUT)[1..], asset_bytes);
        std::fs::write(
            selection.assets_dir.unwrap().join("assets/a"),
            b"changed asset",
        )
        .unwrap();
        asset(selected.token, 8);
        assert_eq!(&lock(&OUTPUT)[1..], asset_bytes);
        *lock(&INPUT) = b"removed".to_vec();
        asset(selected.token, 7);
        assert_eq!(*lock(&OUTPUT), vec![0]);
        // A new process must re-prove the plan, before counting or blessing it.
        let client = Client::open_at(&base, &base, COMPAT, b"embedded", Some(ORIGIN)).unwrap();
        *lock(&CLIENT) = Some(client);
        {
            let mut g = lock(&GENERATIONS);
            g.initialized = false;
            g.live = None;
            g.candidate = None;
        }
        assert!(initial_selection().is_none());
        assert_eq!(lock(&CLIENT).as_ref().unwrap().status().entry, None);
        *lock(&CLIENT) = None;
        std::fs::remove_dir_all(base).unwrap();
    }
}
