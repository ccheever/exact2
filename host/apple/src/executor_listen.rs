//! A resource's local listener, natively (LLP 1133.005): the stream's
//! thread checks the address and its grant, opens the listener module and
//! queues its events for the runner within D3's bounds, until the stream is
//! let go or the listener fails. Sends go to the module by connection.
//!
//! The module is a separate artifact (D13): `libexact_listen.dylib` beside
//! the executable on macOS (`EXACT_LISTEN_DYLIB` names another), opened on
//! the first listener; a host that links `exact-listen` installs its table
//! instead ([`install`]: the Linux host, the tests).
#![allow(unsafe_code)] // The module's C entry points, and its events' context.
use super::super::{complete, failed, has_ready, wake, Completed, Shared};
use exact_runner::listen::{self as words, ConnectionView, ListenView, LISTEN_STATUS};
use exact_runner::{FailureKind, Message, Outcome, Request, SocketOp, SocketSend};
use ibex2::stdlib::abort::AbortController;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// The module's event callback (`exact_listen::abi::Emit`).
pub type Emit = extern "C" fn(*mut c_void, u32, u64, *const u8, usize) -> u32;

type OpenFn = unsafe extern "C" fn(*const u8, usize, Emit, *mut c_void) -> *mut c_void;
type SendFn = unsafe extern "C" fn(*mut c_void, u64, *const u8, usize, u32) -> i32;
type BufferedFn = unsafe extern "C" fn(*mut c_void, u64) -> i64;
type ViewFn = unsafe extern "C" fn(*mut c_void, *mut u64, usize) -> usize;
type CloseFn = unsafe extern "C" fn(*mut c_void);

/// The module's entry points (`exact_listen::abi::Module`).
#[derive(Clone, Copy)]
pub struct Module {
    /// `exact_listen_open`.
    pub open: OpenFn,
    /// `exact_listen_send`.
    pub send: SendFn,
    /// `exact_listen_buffered`.
    pub buffered: BufferedFn,
    /// `exact_listen_view`.
    pub view: ViewFn,
    /// `exact_listen_close`.
    pub close: CloseFn,
}

static MODULE: OnceLock<Result<Module, String>> = OnceLock::new();

/// Use `module` instead of opening the library: a host that links it.
#[allow(dead_code)] // the Linux host's and the tests', not the Apple library's
pub fn install(module: Module) {
    let _ = MODULE.set(Ok(module));
}

#[cfg(unix)]
extern "C" {
    fn dlopen(path: *const std::ffi::c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void;
    fn dlerror() -> *const std::ffi::c_char;
}
const RTLD_NOW: i32 = 2;
#[cfg(target_vendor = "apple")]
const RTLD_LOCAL: i32 = 4;
#[cfg(not(target_vendor = "apple"))]
const RTLD_LOCAL: i32 = 0;
#[cfg(target_vendor = "apple")]
const LIBRARY: &str = "libexact_listen.dylib";
#[cfg(not(target_vendor = "apple"))]
const LIBRARY: &str = "libexact_listen.so";

/// The module, opened at the first listener; why not, when it is absent.
fn module() -> Result<Module, String> {
    MODULE.get_or_init(open_library).clone()
}

#[cfg(not(unix))]
fn open_library() -> Result<Module, String> {
    Err("no listener on this platform".into())
}

#[cfg(unix)]
fn open_library() -> Result<Module, String> {
    let path = match std::env::var_os("EXACT_LISTEN_DYLIB") {
        Some(path) => PathBuf::from(path),
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name(LIBRARY),
    };
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| "a library path with a NUL".to_string())?;
    // SAFETY: dlopen and dlsym with NUL-terminated names; each symbol is the
    // module's own function of the type its ABI (version 1) gives it, and the
    // library stays loaded for the process's life.
    unsafe {
        let handle = dlopen(name.as_ptr(), RTLD_NOW | RTLD_LOCAL);
        if handle.is_null() {
            let why = dlerror();
            let why = if why.is_null() {
                "not found".into()
            } else {
                std::ffi::CStr::from_ptr(why).to_string_lossy().into_owned()
            };
            return Err(format!("this build carries no listener ({why})"));
        }
        let symbol = |name: &str| {
            let name = std::ffi::CString::new(name).unwrap();
            let found = dlsym(handle, name.as_ptr());
            (!found.is_null()).then_some(found)
        };
        let abi = std::mem::transmute::<*mut c_void, extern "C" fn() -> u32>(
            symbol("exact_listen_abi").ok_or("the listener library has no ABI")?,
        );
        if abi() != 1 {
            return Err(format!("the listener library speaks ABI {}, not 1", abi()));
        }
        let missing = || "the listener library lacks an entry point".to_string();
        Ok(Module {
            open: std::mem::transmute::<*mut c_void, OpenFn>(
                symbol("exact_listen_open").ok_or_else(missing)?,
            ),
            send: std::mem::transmute::<*mut c_void, SendFn>(
                symbol("exact_listen_send").ok_or_else(missing)?,
            ),
            buffered: std::mem::transmute::<*mut c_void, BufferedFn>(
                symbol("exact_listen_buffered").ok_or_else(missing)?,
            ),
            view: std::mem::transmute::<*mut c_void, ViewFn>(
                symbol("exact_listen_view").ok_or_else(missing)?,
            ),
            close: std::mem::transmute::<*mut c_void, CloseFn>(
                symbol("exact_listen_close").ok_or_else(missing)?,
            ),
        })
    }
}

/// Undelivered events a listener may hold, and their bytes (D3).
const LISTENER_EVENTS: usize = 1024;
const LISTENER_BYTES: usize = 4 << 20;
/// Undelivered lines one connection may hold, and their bytes (D3).
const CONNECTION_LINES: usize = 256;
const CONNECTION_BYTES: usize = 1 << 20;

/// An open listener: the module's handle, and its address.
struct Open {
    module: Module,
    handle: usize,
    address: String,
}

/// The open listeners of every executor in the process, by the executor's
/// shared state and the stream's ticket.
static OPEN: Mutex<Vec<((usize, u64), Open)>> = Mutex::new(Vec::new());

fn key(shared: &Shared, ticket: u64) -> (usize, u64) {
    (shared as *const Shared as usize, ticket)
}

/// What the module's thread reports to the stream's.
struct Context {
    /// The executor's state, alive while the stream's thread runs.
    shared: *const Shared,
    ticket: u64,
    address: String,
    /// The listener's error, its last event.
    ended: Mutex<Option<Outcome>>,
    done: Condvar,
}

/// The path with its existing ancestors resolved (D4): `/tmp/…` and
/// `/private/tmp/…` on macOS are one path.
fn resolve(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = at.canonicalize() {
            return rest
                .iter()
                .rev()
                .fold(real, |p: PathBuf, part| p.join(part));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                at = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Run listener `ticket` until it is let go or fails.
pub(super) fn run(
    shared: &Shared,
    ticket: u64,
    request: Request,
    grants: &str,
    abort: AbortController,
) {
    let end = |kind, why: String| complete(shared, ticket, failed(kind, why));
    if let Some(why) = request.listen_refusal() {
        return end(FailureKind::Refused, why);
    }
    let Some(options) = request.listen.as_deref() else {
        return;
    };
    let written = words::unix_path(&request.url).unwrap_or_default();
    let path = resolve(Path::new(written));
    let shown = path.to_string_lossy().into_owned();
    let resolve_prefix = |prefix: &str| resolve(Path::new(prefix)).to_string_lossy().into_owned();
    if words::admitting(grants, request.grants.as_deref(), &shown, resolve_prefix).is_none() {
        let why = format!("{} is outside the app's net.listen grants", request.url);
        return end(FailureKind::Refused, why);
    }
    if shown.len() >= words::SUN_PATH || shown.contains('\0') {
        return end(
            FailureKind::Refused,
            format!("{shown} is too long for sun_path"),
        );
    }
    let module = match module() {
        Ok(module) => module,
        Err(why) => return end(FailureKind::Unsupported, why),
    };
    let config = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        request.url,
        shown,
        if options.takeover { 1 } else { 0 },
        options.first_line_timeout_ms.unwrap_or(0),
        options.max_line_bytes,
        options.max_connections,
    );
    let context = Box::new(Context {
        shared,
        ticket,
        address: request.url.clone(),
        ended: Mutex::new(None),
        done: Condvar::new(),
    });
    let ctx = &*context as *const Context as *mut c_void;
    // SAFETY: `config` is live for the call; `context` outlives the module's
    // thread, which `close` joins below before the box drops.
    let handle = unsafe { (module.open)(config.as_ptr(), config.len(), emit, ctx) };
    if handle.is_null() {
        return end(FailureKind::Network, "the listener could not start".into());
    }
    let address = request.url.clone();
    OPEN.lock().unwrap().push((
        key(shared, ticket),
        Open {
            module,
            handle: handle as usize,
            address,
        },
    ));
    // Until the stream is let go, the executor retires, or the listener fails.
    let mut ended = context.ended.lock().unwrap();
    while ended.is_none() && !abort.signal().aborted() && wanted(shared, ticket) {
        ended = (context.done)
            .wait_timeout(ended, std::time::Duration::from_millis(100))
            .unwrap()
            .0;
    }
    let outcome = ended.take();
    drop(ended);
    OPEN.lock()
        .unwrap()
        .retain(|(k, _)| *k != key(shared, ticket));
    // SAFETY: the handle is the module's, closed once; no event follows.
    unsafe { (module.close)(handle) };
    drop(context);
    let outcome =
        outcome.unwrap_or_else(|| failed(FailureKind::Aborted, "the listener was let go"));
    complete(shared, ticket, outcome);
}

/// Whether the runner still wants `ticket`.
fn wanted(shared: &Shared, ticket: u64) -> bool {
    let state = shared.state.lock().unwrap();
    !state.retired && (state.running.iter()).any(|run| run.ticket == ticket && !run.forgotten)
}

/// One event from the module: queued as a message, within D3's bounds.
extern "C" fn emit(ctx: *mut c_void, kind: u32, id: u64, data: *const u8, len: usize) -> u32 {
    // SAFETY: `ctx` is the stream's `Context`, alive until the module is
    // closed; `data` points at `len` bytes for the call.
    let (context, bytes) = unsafe {
        let bytes = if data.is_null() {
            &[][..]
        } else {
            std::slice::from_raw_parts(data, len)
        };
        (&*(ctx as *const Context), bytes)
    };
    let text = String::from_utf8_lossy(bytes).into_owned();
    let message = |event: &str, id: String, data: String| Message {
        event: event.into(),
        id,
        data,
        ..Message::default()
    };
    let (message, bounded) = match kind {
        0 => (message("listening", "0".into(), text), false),
        1 => (message("connection", id.to_string(), String::new()), true),
        2 => (message("message", id.to_string(), text), true),
        3 => (message("end", id.to_string(), text), false),
        4 => (message(LISTEN_STATUS, context.address.clone(), text), false),
        _ => {
            // An error: its kind, a NUL, its message. `AddressInUse` is no
            // failure kind of the runner's: it travels as `Refused`, named.
            let (what, why) = text.split_once('\0').unwrap_or(("Network", &text));
            let (kind, why) = match what {
                "Refused" => (FailureKind::Refused, why.to_string()),
                "AddressInUse" => (FailureKind::Refused, format!("AddressInUse: {why}")),
                _ => (FailureKind::Network, why.to_string()),
            };
            *context.ended.lock().unwrap() = Some(failed(kind, why));
            context.done.notify_all();
            return 1;
        }
    };
    // SAFETY: the stream's thread, which holds the executor's state, closes
    // the module before it returns.
    queue(
        unsafe { &*context.shared },
        context.ticket,
        message,
        bounded,
    )
}

/// Queue one event behind those the runner has not taken: 0 queued, 1
/// gone, 2 past a bound (a connection or a line; D3).
fn queue(shared: &Shared, ticket: u64, message: Message, bounded: bool) -> u32 {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return 1;
    }
    let Some((lane, elapsed_ms)) = (state.running.iter())
        .find(|run| run.ticket == ticket && !run.forgotten)
        .map(|run| {
            (
                run.lane,
                run.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            )
        })
    else {
        return 1;
    };
    if bounded {
        let waiting = (state.completed[lane].iter())
            .filter(|done| done.ticket == ticket && done.message)
            .filter_map(|done| match &done.outcome {
                Outcome::Message(m) => Some(m),
                _ => None,
            });
        let (mut events, mut bytes, mut lines, mut line_bytes) = (0, 0usize, 0, 0usize);
        for m in waiting {
            events += 1;
            bytes += m.data.len();
            if m.event == "message" && m.id == message.id {
                lines += 1;
                line_bytes += m.data.len();
            }
        }
        let size = message.data.len();
        let line = message.event == "message";
        if events >= LISTENER_EVENTS
            || bytes + size > LISTENER_BYTES
            || line && (lines >= CONNECTION_LINES || line_bytes + size > CONNECTION_BYTES)
        {
            return 2;
        }
    }
    exact_runner::stream_timing::received(ticket);
    state.completed[lane].push_back(Completed {
        elapsed_ms,
        ticket,
        outcome: Outcome::Message(message),
        bytes: 0,
        message: true,
        stream: true,
        light: false,
        again: false,
        reply: false,
    });
    if has_ready(&state) {
        wake(&mut state);
    }
    0
}

/// Send one batch's listener operations, whose commit stood: each
/// connection must take its lines within its 1 MiB, or nothing is sent; a
/// line or end on a connection that has ended is dropped. For the stream,
/// the lines taken and the bytes its connections hold now.
pub(super) fn send(shared: &Shared, batch: &SocketSend) -> Result<Vec<(u64, u64, u64)>, String> {
    let open = OPEN.lock().unwrap();
    let find = |ticket: u64| {
        (open.iter())
            .find(|(k, _)| *k == key(shared, ticket))
            .map(|(_, o)| o)
    };
    // The whole batch's share per connection, checked before anything goes.
    let mut shares: HashMap<(u64, u64), usize> = HashMap::new();
    for (ticket, _, op) in &batch.ops {
        let (Some(_), Some(connection)) = (find(*ticket), op.connection()) else {
            return Err("InvalidStateError: the listener is not open".into());
        };
        *shares.entry((*ticket, connection)).or_default() += op.size();
    }
    for ((ticket, connection), share) in &shares {
        let o = find(*ticket).expect("checked above");
        // SAFETY: an open handle of this module.
        let held = unsafe { (o.module.buffered)(o.handle as *mut c_void, *connection) };
        if held >= 0 && held as usize + share > exact_listen_send_bytes() {
            return Err("QuotaExceededError: a connection's buffer exceeds 1 MiB".into());
        }
    }
    let mut taken: Vec<(u64, u64, u64)> = Vec::new();
    for (ticket, _, op) in &batch.ops {
        let o = find(*ticket).expect("checked above");
        let handle = o.handle as *mut c_void;
        // SAFETY: an open handle; the line's bytes are live for the call.
        let code = unsafe {
            match op {
                SocketOp::Line { connection, text } => {
                    (o.module.send)(handle, *connection, text.as_ptr(), text.len(), 0)
                }
                SocketOp::End { connection } => {
                    (o.module.send)(handle, *connection, std::ptr::null(), 0, 1)
                }
                _ => 1,
            }
        };
        if code == 2 {
            return Err("QuotaExceededError: a connection's buffer exceeds 1 MiB".into());
        }
        let total = match taken.iter_mut().find(|(t, ..)| t == ticket) {
            Some(total) => total,
            None => {
                taken.push((*ticket, 0, 0));
                taken.last_mut().unwrap()
            }
        };
        total.1 += u64::from(code == 0 && matches!(op, SocketOp::Line { .. }));
    }
    for (ticket, _, buffered) in &mut taken {
        let o = find(*ticket).expect("checked above");
        *buffered = view(o)
            .iter()
            .map(|c| {
                // SAFETY: an open handle of this module.
                let n = unsafe { (o.module.buffered)(o.handle as *mut c_void, c.id) };
                n.max(0) as u64
            })
            .sum();
    }
    Ok(taken)
}

/// The most a connection holds unwritten (`exact_listen::SEND_BYTES`).
fn exact_listen_send_bytes() -> usize {
    1 << 20
}

fn view(o: &Open) -> Vec<ConnectionView> {
    let mut out = vec![0u64; 3 * 64];
    // SAFETY: an open handle; `out` holds 64 triples, `maxConnections`' most.
    let n = unsafe { (o.module.view)(o.handle as *mut c_void, out.as_mut_ptr(), 64) };
    (out.chunks(3).take(n.min(64)))
        .map(|c| ConnectionView {
            id: c[0],
            received: c[1],
            sent: c[2],
        })
        .collect()
}

/// `shared`'s listeners as `state.streams` shows them, by stream ticket.
pub(super) fn reader(shared: Arc<Shared>) -> words::ListenReader {
    Arc::new(move |ticket| {
        let open = OPEN.lock().unwrap();
        let (_, o) = open.iter().find(|(k, _)| *k == key(&shared, ticket))?;
        Some(ListenView {
            address: o.address.clone(),
            connections: view(o),
        })
    })
}

#[cfg(test)]
#[path = "executor_listen_tests.rs"]
mod tests;
