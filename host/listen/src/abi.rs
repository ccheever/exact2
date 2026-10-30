//! The C ABI the executor opens (`libexact_listen.dylib` on Apple), so the
//! listener links into no binary that does not listen (LLP 1133.005 D13).
//! [`Module::here`] is the same table for a host that links the crate.
//!
//! The configuration is `address`, `path`, `takeover` (`0`/`1`), the first
//! line's timeout in ms (`0` for none), `maxLineBytes` and `maxConnections`,
//! joined by NUL (a path has none, D4). Events reach `emit` as a kind, a
//! connection id and bytes: 0 `listening` (the address), 1 `connection`, 2
//! one line, 3 `end` (the reason), 4 a standing word, 5 `error` (its kind,
//! a NUL, its message). `emit` answers 0 queued, 1 gone, 2 full ([`Taken`]).
//!
//! The loopback receiver for `openAuthSession` (D9) is here too, so the auth
//! arm needs no code of its own: its configuration is `host`, `port`,
//! `path`, `state` and the app's name, joined by NUL.
use crate::{Config, Event, Listener, Refused, Taken};
use std::ffi::c_void;
use std::time::Duration;

/// The ABI version this library speaks.
pub const ABI: u32 = 1;

/// The caller's event callback.
pub type Emit =
    extern "C" fn(ctx: *mut c_void, kind: u32, id: u64, data: *const u8, len: usize) -> u32;

/// The library's entry points, by symbol or linked.
#[derive(Clone, Copy)]
pub struct Module {
    /// `exact_listen_open`.
    pub open: unsafe extern "C" fn(*const u8, usize, Emit, *mut c_void) -> *mut c_void,
    /// `exact_listen_send`.
    pub send: unsafe extern "C" fn(*mut c_void, u64, *const u8, usize, u32) -> i32,
    /// `exact_listen_buffered`.
    pub buffered: unsafe extern "C" fn(*mut c_void, u64) -> i64,
    /// `exact_listen_view`.
    pub view: unsafe extern "C" fn(*mut c_void, *mut u64, usize) -> usize,
    /// `exact_listen_close`.
    pub close: unsafe extern "C" fn(*mut c_void),
}

impl Module {
    /// The table of this crate, linked.
    pub fn here() -> Module {
        Module {
            open: exact_listen_open,
            send: exact_listen_send,
            buffered: exact_listen_buffered,
            view: exact_listen_view,
            close: exact_listen_close,
        }
    }
}

/// The ABI version.
#[no_mangle]
pub extern "C" fn exact_listen_abi() -> u32 {
    ABI
}

fn config(bytes: &[u8]) -> Option<Config> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut parts = text.split('\0');
    let address = parts.next()?.to_string();
    let path = parts.next()?.into();
    let takeover = parts.next()? == "1";
    let first_line = parts.next()?.parse::<u64>().ok()?;
    let max_line = parts.next()?.parse().ok()?;
    let max_connections = parts.next()?.parse().ok()?;
    Some(Config {
        address,
        path,
        takeover,
        first_line: (first_line > 0).then(|| Duration::from_millis(first_line)),
        max_line,
        max_connections,
    })
}

/// The caller's context, carried to the loop's thread.
struct Context(*mut c_void);
// SAFETY: the caller promises `ctx` may be used from the listener's thread
// until `exact_listen_close` returns.
unsafe impl Send for Context {}

/// Start a listener; null when the configuration does not parse or no
/// thread could start. Its events go to `emit(ctx, …)` from its own thread
/// until [`exact_listen_close`] returns.
///
/// # Safety
/// `config` points at `len` bytes; `ctx` stays valid for `emit` until close.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_open(
    config_ptr: *const u8,
    len: usize,
    emit: Emit,
    ctx: *mut c_void,
) -> *mut c_void {
    let bytes = std::slice::from_raw_parts(config_ptr, len);
    let Some(config) = config(bytes) else {
        return std::ptr::null_mut();
    };
    let address = config.address.clone();
    let ctx = Context(ctx);
    let sink = Box::new(move |event: Event| {
        let ctx = &ctx;
        let call =
            |kind: u32, id: u64, data: &[u8]| emit(ctx.0, kind, id, data.as_ptr(), data.len());
        let code = match event {
            Event::Listening => call(0, 0, address.as_bytes()),
            Event::Connection(id) => call(1, id, &[]),
            Event::Line(id, line) => call(2, id, line),
            Event::End(id, reason) => call(3, id, reason.as_bytes()),
            Event::Standing(word) => call(4, 0, word.as_bytes()),
            Event::Error(failure) => {
                let text = format!("{}\0{}", failure.kind(), failure.message());
                call(5, 0, text.as_bytes())
            }
        };
        match code {
            0 => Taken::Queued,
            2 => Taken::Full,
            _ => Taken::Gone,
        }
    });
    match Listener::open(config, sink) {
        Ok(listener) => Box::into_raw(Box::new(listener)).cast(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Queue a line on `connection` (`len` bytes at `data`; none when `data`
/// is null), then end it when `end` is 1: 0 taken, 1 the connection has
/// ended, 2 over its 1 MiB.
///
/// # Safety
/// `listener` is from [`exact_listen_open`], not yet closed.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_send(
    listener: *mut c_void,
    connection: u64,
    data: *const u8,
    len: usize,
    end: u32,
) -> i32 {
    let listener = &*(listener as *const Listener);
    let line = (!data.is_null()).then(|| std::slice::from_raw_parts(data, len));
    match listener.send(connection, line, end == 1) {
        Ok(()) => 0,
        Err(Refused::Gone) => 1,
        Err(Refused::Quota) => 2,
    }
}

/// The bytes `connection` holds unwritten, or -1 when it has ended.
///
/// # Safety
/// `listener` is from [`exact_listen_open`], not yet closed.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_buffered(listener: *mut c_void, connection: u64) -> i64 {
    let listener = &*(listener as *const Listener);
    listener.buffered(connection).map_or(-1, |n| n as i64)
}

/// The open connections as (id, received, sent) triples, at most `cap` of
/// them into `out`; the number open.
///
/// # Safety
/// `listener` is from [`exact_listen_open`]; `out` holds `3 * cap` values.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_view(
    listener: *mut c_void,
    out: *mut u64,
    cap: usize,
) -> usize {
    let listener = &*(listener as *const Listener);
    let view = listener.view();
    for (i, (id, received, sent)) in view.iter().take(cap).enumerate() {
        *out.add(3 * i) = *id;
        *out.add(3 * i + 1) = *received;
        *out.add(3 * i + 2) = *sent;
    }
    view.len()
}

/// Stop the listener and free it: no event follows once this returns.
///
/// # Safety
/// `listener` is from [`exact_listen_open`] and is not used again.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_close(listener: *mut c_void) {
    drop(Box::from_raw(listener as *mut Listener));
}

/// The loopback receiver's callback: the full callback URL, once.
pub type Done = extern "C" fn(ctx: *mut c_void, url: *const u8, len: usize);

struct Sendable(*mut c_void);
// SAFETY: the caller promises `ctx` may be used from the receiver's thread
// until `exact_listen_loopback_close` returns.
unsafe impl Send for Sendable {}

/// Bind the loopback receiver (D9) and start answering; null when it could
/// not, with `*error` 1 for a port in use and 2 for anything else. `done`
/// runs once, from its own thread, unless it is closed first.
///
/// # Safety
/// `config` points at `len` bytes; `error` is writable; `ctx` stays valid
/// for `done` until [`exact_listen_loopback_close`] returns.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_loopback_open(
    config: *const u8,
    len: usize,
    done: Done,
    ctx: *mut c_void,
    error: *mut i32,
) -> *mut c_void {
    let text = String::from_utf8_lossy(std::slice::from_raw_parts(config, len)).into_owned();
    let parts: Vec<&str> = text.split('\0').collect();
    let callback = match parts.as_slice() {
        [host, port, path, state, app] => port.parse().ok().map(|port| crate::loopback::Callback {
            host: host.to_string(),
            port,
            path: path.to_string(),
            state: state.to_string(),
            app: app.to_string(),
        }),
        _ => None,
    };
    let Some(callback) = callback else {
        *error = 2;
        return std::ptr::null_mut();
    };
    let ctx = Sendable(ctx);
    let answered = Box::new(move |url: String| {
        let ctx = &ctx;
        done(ctx.0, url.as_ptr(), url.len());
    });
    match crate::loopback::open(callback, answered) {
        Ok(receiver) => Box::into_raw(Box::new(receiver)).cast(),
        Err(failed) => {
            *error = if matches!(failed, crate::loopback::Failed::PortInUse) {
                1
            } else {
                2
            };
            std::ptr::null_mut()
        }
    }
}

/// Stop the receiver and free its port: `done` does not run after this.
///
/// # Safety
/// `receiver` is from [`exact_listen_loopback_open`] and is not used again.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_loopback_close(receiver: *mut c_void) {
    drop(Box::from_raw(receiver as *mut crate::loopback::Receiver));
}
