//! A local listener, natively (LLP 1133.005 D1–D4, D13): a stream whose
//! URL is `unix:<path>`, run by the listener module (`exact-listen`), a
//! separate artifact this executor opens the first time a source listens.
//! The module binds and reads; this side keeps its events in the stream's
//! queue under D3's bounds, sends its connections' lines, and ends the
//! listener when the runner lets the stream go.
#![allow(unsafe_code)] // The module's C ABI: its entry points, and the sink it calls back.
use super::super::{complete, failed, has_ready, wake, Completed, Core, Shared};
use exact_runner::{FailureKind, Message, Outcome, Request, SocketOp, SocketSend};
use ibex2::stdlib::abort::AbortController;
use std::{
    ffi::c_void,
    sync::{Arc, Condvar, Mutex, OnceLock},
};

/// Undelivered lines a connection may hold, by count and by bytes (D3).
const CONNECTION_LINES: usize = 256;
const CONNECTION_BYTES: usize = 1 << 20;
/// Undelivered events the listener may hold, by count and by bytes (D3).
const LISTENER_EVENTS: usize = 1024;
const LISTENER_BYTES: usize = 4 << 20;
/// What a connection's unwritten bytes may reach with a batch (D3).
const SEND_BYTES: u64 = 1 << 20;

// The module's ABI (`exact_listen`'s, version 1): its events and replies.
const ABI: u32 = 1;
const LISTENING: u32 = 1;
const CONNECTION: u32 = 2;
const MESSAGE: u32 = 3;
const END: u32 = 4;
const WAITING: u32 = 5;
const DISPLACED: u32 = 6;
const RESUMED: u32 = 7;
const ERROR: u32 = 8;
const TAKEN: u32 = 0;
const GONE: u32 = 1;
const OVERFLOW: u32 = 2;
const FULL: u32 = 3;

type Sink = extern "C" fn(*mut c_void, u32, u64, *const u8, usize) -> u32;

/// `exact_listen::Config`, as this side lays it out: the tests compare the
/// two (`executor_listen_tests.rs`).
#[cfg_attr(test, allow(dead_code))]
#[repr(C)]
struct AbiConfig {
    address: *const u8,
    address_len: usize,
    grants: *const u8,
    grants_len: usize,
    takeover: u8,
    first_line_timeout_ms: u32,
    max_line_bytes: u32,
    max_connections: u32,
}
#[cfg(test)]
use exact_listen::Config;
#[cfg(not(test))]
use AbiConfig as Config;

type Open = unsafe extern "C" fn(*const Config, Sink, *mut c_void) -> u64;
type Send = unsafe extern "C" fn(u64, u64, *const u8, usize) -> i32;
type ByConnection<T> = extern "C" fn(u64, u64) -> T;
type Close = extern "C" fn(u64);

/// The module's entry points.
struct Module {
    open: Open,
    send: Send,
    end: ByConnection<i32>,
    buffered: ByConnection<u64>,
    close: Close,
}

/// The module, opened once per process; why not, when it is absent.
fn module() -> Result<&'static Module, String> {
    static MODULE: OnceLock<Result<Module, String>> = OnceLock::new();
    MODULE.get_or_init(load).as_ref().map_err(String::clone)
}

/// Under test the executor links the module's crate.
#[cfg(test)]
fn load() -> Result<Module, String> {
    assert_eq!(exact_listen::exact_listen_abi(), ABI);
    Ok(Module {
        open: exact_listen::exact_listen_open,
        send: exact_listen::exact_listen_send,
        end: exact_listen::exact_listen_end,
        buffered: exact_listen::exact_listen_buffered,
        close: exact_listen::exact_listen_close,
    })
}

/// `libexact_listen.dylib` (`.so` on Linux) beside the executable: where the
/// macOS bundle puts it (`Contents/MacOS`) and where Cargo builds it beside
/// a Linux host's binary.
#[cfg(all(not(test), unix))]
fn load() -> Result<Module, String> {
    let name = if cfg!(target_vendor = "apple") {
        "libexact_listen.dylib"
    } else {
        "libexact_listen.so"
    };
    let path = (std::env::current_exe().ok())
        .and_then(|exe| Some(exe.parent()?.join(name)))
        .ok_or("the executable's directory is unknown")?;
    let absent = || format!("the listener module ({name}) is not in this build");
    let c_path =
        std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).map_err(|_| absent())?;
    // SAFETY: a NUL-terminated path; the library stays loaded for the process.
    let library = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if library.is_null() {
        return Err(absent());
    }
    let symbol = |name: &str| {
        let c = std::ffi::CString::new(name).expect("a symbol name");
        // SAFETY: `library` is a handle dlopen returned.
        let found = unsafe { libc::dlsym(library, c.as_ptr()) };
        (!found.is_null())
            .then_some(found)
            .ok_or_else(|| format!("the listener module lacks {name}"))
    };
    // SAFETY: each symbol is the module's export of exactly this type (its ABI 1).
    unsafe {
        type Abi = extern "C" fn() -> u32;
        let abi = std::mem::transmute::<*mut c_void, Abi>(symbol("exact_listen_abi")?);
        if abi() != ABI {
            return Err(format!(
                "the listener module speaks ABI {}, not {ABI}",
                abi()
            ));
        }
        Ok(Module {
            open: std::mem::transmute::<*mut c_void, Open>(symbol("exact_listen_open")?),
            send: std::mem::transmute::<*mut c_void, Send>(symbol("exact_listen_send")?),
            end: std::mem::transmute::<*mut c_void, ByConnection<i32>>(symbol("exact_listen_end")?),
            buffered: std::mem::transmute::<*mut c_void, ByConnection<u64>>(symbol(
                "exact_listen_buffered",
            )?),
            close: std::mem::transmute::<*mut c_void, Close>(symbol("exact_listen_close")?),
        })
    }
}

/// No listener off Unix (Windows' named pipes are `later`, §8).
#[cfg(all(not(test), not(unix)))]
fn load() -> Result<Module, String> {
    Err("no listener on this platform".into())
}

/// What the module's thread and this stream's share.
struct Listening<'a> {
    shared: &'a Shared,
    ticket: u64,
    /// The listener's end, once the module said it, or the stream let go.
    ended: Arc<(Mutex<Option<Outcome>>, Condvar)>,
}

/// Run listener `request` on the stream's thread until it ends or the
/// runner lets it go; `grants` are the ones admitted for it.
pub(super) fn run(
    shared: &Shared,
    ticket: u64,
    request: Request,
    grants: &str,
    abort: AbortController,
) {
    // Outside the grants is refused before the module is asked: a build
    // with no `net.listen` line carries no module (D13), and a grant set
    // that did not parse grants nothing.
    let refused = |why: String| complete(shared, ticket, failed(FailureKind::Refused, why));
    if let Err(errors) = exact_runner::grants::parse(grants) {
        return refused(exact_runner::grants::refusal(&errors));
    }
    let prefixes: String = (grants.lines())
        .filter_map(|line| exact_runner::grants::listen_prefix(line.trim())?.ok())
        .map(|address| format!("{address}\n"))
        .collect();
    if prefixes.is_empty() {
        return refused(format!("{} is outside the net.listen grants", request.url));
    }
    let module = match module() {
        Ok(module) => module,
        Err(why) => return complete(shared, ticket, failed(FailureKind::Unsupported, why)),
    };
    let options = request.listen.as_deref().cloned().unwrap_or_default();
    let config = Config {
        address: request.url.as_ptr(),
        address_len: request.url.len(),
        grants: prefixes.as_ptr(),
        grants_len: prefixes.len(),
        takeover: options.takeover.into(),
        first_line_timeout_ms: options.first_line_timeout_ms,
        max_line_bytes: options.max_line_bytes,
        max_connections: options.max_connections,
    };
    let listening = Listening {
        shared,
        ticket,
        ended: Arc::new((Mutex::new(None), Condvar::new())),
    };
    // The runner letting the stream go, and retirement, end it.
    let ended = listening.ended.clone();
    let _released = abort.signal().register(move || {
        let (lock, cv) = &*ended;
        lock.lock().unwrap().get_or_insert(gone());
        cv.notify_all();
    });
    if abort.signal().aborted() {
        return complete(shared, ticket, gone());
    }
    let ctx = &listening as *const Listening as *mut c_void;
    // SAFETY: `config`'s bytes live through the call; `listening` outlives
    // the module's use of `ctx`, which ends when `close` returns below.
    // Registered before any event can reach the runner, so a send that
    // event lets through finds the handle.
    let handle = {
        let mut listeners = shared.listeners.lock().unwrap();
        let handle = unsafe { (module.open)(&config, sink, ctx) };
        listeners.insert(ticket, handle);
        handle
    };
    let outcome = {
        let (lock, cv) = &*listening.ended;
        let mut end = lock.lock().unwrap();
        loop {
            if let Some(outcome) = end.take() {
                break outcome;
            }
            end = cv.wait(end).unwrap();
        }
    };
    shared.listeners.lock().unwrap().remove(&ticket);
    (module.close)(handle);
    complete(shared, ticket, outcome);
}

fn gone() -> Outcome {
    failed(FailureKind::Aborted, "the stream was let go")
}

/// The module's events, on its thread.
extern "C" fn sink(
    ctx: *mut c_void,
    event: u32,
    connection: u64,
    data: *const u8,
    len: usize,
) -> u32 {
    // SAFETY: `ctx` is the `Listening` `run` passed, alive until `close`.
    let listening = unsafe { &*(ctx as *const Listening) };
    let bytes = if len == 0 {
        &[][..]
    } else {
        // SAFETY: the module's data, valid for the call.
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    let text = String::from_utf8_lossy(bytes).into_owned();
    if event == ERROR {
        let (kind, message) = match text.split_once(": ") {
            Some(("Refused", m)) => (FailureKind::Refused, m.to_string()),
            Some(("Unsupported", m)) => (FailureKind::Unsupported, m.to_string()),
            // `AddressInUse: …` keeps its word: the prelude names the kind.
            Some(("Network", m)) => (FailureKind::Network, m.to_string()),
            _ => (FailureKind::Network, text),
        };
        let (lock, cv) = &*listening.ended;
        lock.lock().unwrap().get_or_insert(failed(kind, message));
        cv.notify_all();
        return TAKEN;
    }
    let (name, id) = match event {
        LISTENING => ("listening", String::new()),
        CONNECTION => ("connection", connection.to_string()),
        MESSAGE => ("message", connection.to_string()),
        END => ("end", connection.to_string()),
        WAITING => (exact_runner::LISTEN_WAITING, String::new()),
        DISPLACED => (exact_runner::LISTEN_DISPLACED, String::new()),
        RESUMED => (exact_runner::LISTEN_RESUMED, String::new()),
        _ => return TAKEN,
    };
    let message = Message {
        event: name.into(),
        id,
        data: text,
        ..Message::default()
    };
    queue(listening.shared, listening.ticket, event, message)
}

/// Queue one event behind the ones the runner has not taken, never
/// coalesced, within D3's bounds: a connection's lines and the listener's
/// events. An end and the host's notes always go in.
fn queue(shared: &Shared, ticket: u64, event: u32, message: Message) -> u32 {
    let mut state = shared.state.lock().unwrap();
    if state.retired {
        return GONE;
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
        return GONE;
    };
    if event == CONNECTION || event == MESSAGE {
        let held = (state.completed[lane].iter())
            .filter(|done| done.ticket == ticket && done.message)
            .filter_map(|done| match &done.outcome {
                Outcome::Message(m) => Some(m),
                _ => None,
            });
        let (mut events, mut bytes, mut lines, mut line_bytes) = (0, 0, 0, 0);
        for m in held {
            events += 1;
            bytes += m.data.len();
            if m.event == "message" && m.id == message.id {
                lines += 1;
                line_bytes += m.data.len();
            }
        }
        let size = message.data.len();
        let full = events >= LISTENER_EVENTS || bytes + size > LISTENER_BYTES;
        let line_full = lines >= CONNECTION_LINES || line_bytes + size > CONNECTION_BYTES;
        match event {
            CONNECTION if full => return FULL,
            MESSAGE if full || line_full => return OVERFLOW,
            _ => {}
        }
    }
    state.completed[lane].push_back(Completed {
        elapsed_ms,
        ticket,
        outcome: Outcome::Message(message),
        bytes: 0,
        message: true,
        stream: true,
        light: false,
        again: false,
    });
    if has_ready(&state) {
        wake(&mut state);
    }
    TAKEN
}

impl Core {
    /// Each listener connection's unwritten bytes, by its stream ticket and
    /// id, read live from the module: the runner's snapshot at each turn's
    /// start.
    pub(crate) fn listen_reader(&self) -> exact_runner::ListenReader {
        let shared = self.shared.clone();
        Arc::new(move |ticket, connection| {
            let handle = *shared.listeners.lock().unwrap().get(&ticket)?;
            let module = module().ok()?;
            let bytes = (module.buffered)(handle, connection);
            (bytes != u64::MAX).then_some(bytes)
        })
    }
}

/// Check the listener operations of `batch`, whose commit stood: every
/// connection it names must be open, and each must take its share without
/// its unwritten bytes passing 1 MiB, or nothing of the batch is sent (D3,
/// D6). An ended connection refuses: its peer is gone.
pub(super) fn check(shared: &Shared, batch: &SocketSend) -> Result<(), String> {
    if batch.ops.iter().all(|(.., op)| op.connection().is_none()) {
        return Ok(());
    }
    let module = module()?;
    let listeners = shared.listeners.lock().unwrap();
    for (ticket, _, op) in &batch.ops {
        let Some(connection) = op.connection() else {
            continue;
        };
        let handle = *listeners
            .get(ticket)
            .ok_or("InvalidStateError: the listener is not open")?;
        let buffered = (module.buffered)(handle, connection);
        if buffered == u64::MAX {
            return Err(format!(
                "InvalidStateError: connection {connection} ended before the send went out"
            ));
        }
        let staged: u64 = (batch.ops.iter())
            .filter(|(t, _, o)| t == ticket && o.connection() == Some(connection))
            .map(|(.., o)| o.size() as u64)
            .sum();
        if buffered + staged > SEND_BYTES {
            return Err("QuotaExceededError: connection buffer exceeds 1 MiB".into());
        }
    }
    Ok(())
}

/// Send the listener operations of `batch`, which [`check`] passed: for
/// each listener, the lines it accepted (its bytes are per connection).
pub(super) fn send(shared: &Shared, batch: &SocketSend) -> Result<Vec<(u64, u64, u64)>, String> {
    let mut totals: Vec<(u64, u64, u64)> = Vec::new();
    if batch.ops.iter().all(|(.., op)| op.connection().is_none()) {
        return Ok(totals);
    }
    let module = module()?;
    let listeners = shared.listeners.lock().unwrap();
    for (done, (ticket, _, op)) in batch.ops.iter().enumerate() {
        let (Some(connection), Some(handle)) = (op.connection(), listeners.get(ticket)) else {
            continue;
        };
        let sent = match op {
            // SAFETY: the text's bytes, valid for the call.
            SocketOp::Line { text, .. } => unsafe {
                (module.send)(*handle, connection, text.as_ptr(), text.len())
            },
            _ => (module.end)(*handle, connection),
        };
        if sent != 0 {
            return Err(format!(
                "connection {connection} ended (after {done} of {} operations)",
                batch.ops.len()
            ));
        }
        if matches!(op, SocketOp::Line { .. }) {
            match totals.iter_mut().find(|(t, ..)| t == ticket) {
                Some(total) => total.1 += 1,
                None => totals.push((*ticket, 1, 0)),
            }
        }
    }
    Ok(totals)
}

#[cfg(test)]
#[path = "executor_listen_tests.rs"]
mod tests;
