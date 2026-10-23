//! The local listener (LLP 1133.005 D1–D4): a `unix:` address bound under
//! D4's directory and path rules, its connections read as lines and written
//! as lines, behind a C ABI the native executor opens on first use (D13).
//!
//! One listener is one thread polling its socket, its connections, the
//! directory it binds in and a wake pipe. What it sees goes to the
//! executor's [`Sink`] in order, from that thread; the sink's reply carries
//! the executor's bounds back (D3). Within a process, listeners on one path
//! follow the rule between processes (D2, [`registry`]).
#![cfg_attr(not(unix), allow(unused))]

#[cfg(unix)]
mod address;
#[cfg(unix)]
mod publish;
#[cfg(unix)]
mod registry;
#[cfg(unix)]
mod server;
#[cfg(unix)]
mod watch;

use std::ffi::c_void;

/// The ABI's version: [`exact_listen_abi`]'s answer.
pub const ABI: u32 = 1;

/// What a [`Sink`] is told, in order, from the listener's thread.
pub mod event {
    /// The address is bound: `data` is `{"address":…,"port":0,"tookOver":…}`.
    pub const LISTENING: u32 = 1;
    /// A peer connected: `connection` is its id.
    pub const CONNECTION: u32 = 2;
    /// One line from `connection`, without its terminator, as UTF-8.
    pub const MESSAGE: u32 = 3;
    /// `connection` ended: `data` is the reason (`peer`, `app`, `timeout`,
    /// `overflow`).
    pub const END: u32 = 4;
    /// Another listener of this process holds the address (D2): `data` is
    /// the address. Bound later, when it lets go.
    pub const WAITING: u32 = 5;
    /// A newer listener of this process took the address over (D2): this
    /// one keeps its connections and receives no new ones.
    pub const DISPLACED: u32 = 6;
    /// This listener took the address back: the newer one let go, or the
    /// path vanished (D4).
    pub const RESUMED: u32 = 7;
    /// The listener stopped: `data` is `<Kind>: <message>`, the kind one of
    /// `Refused`, `AddressInUse`, `Unsupported`, `Network`. Nothing follows.
    pub const ERROR: u32 = 8;
}

/// A [`Sink`]'s answer.
pub mod reply {
    /// Taken.
    pub const TAKEN: u32 = 0;
    /// Nobody wants more (the stream was let go): the listener goes idle
    /// until it is closed.
    pub const GONE: u32 = 1;
    /// A `MESSAGE` past its connection's or the listener's bound: the
    /// connection ends `overflow`.
    pub const OVERFLOW: u32 = 2;
    /// A `CONNECTION` the listener has no room for: it is closed at once,
    /// with no `END`.
    pub const FULL: u32 = 3;
}

/// Where a listener's events go: called on its thread, never after
/// [`exact_listen_close`] returns. `data` is valid for the call only.
pub type Sink = extern "C" fn(
    ctx: *mut c_void,
    event: u32,
    connection: u64,
    data: *const u8,
    len: usize,
) -> u32;

/// What a listener is opened with (D1's options, D11's grant).
#[repr(C)]
pub struct Config {
    /// The address, `unix:<absolute path>`.
    pub address: *const u8,
    /// Its length in bytes.
    pub address_len: usize,
    /// The `net.listen` grant's prefixes, one per line, each `unix:` with
    /// its prefix (an empty prefix admits any path).
    pub grants: *const u8,
    /// Their length in bytes.
    pub grants_len: usize,
    /// D4's `takeover`: 1 or 0.
    pub takeover: u8,
    /// Milliseconds a connection has for its first complete line; 0 is none.
    pub first_line_timeout_ms: u32,
    /// The longest line, in bytes.
    pub max_line_bytes: u32,
    /// Connections open at once; one past it is closed on accept.
    pub max_connections: u32,
}

/// The ABI's version.
#[no_mangle]
pub extern "C" fn exact_listen_abi() -> u32 {
    ABI
}

/// Open a listener: its thread binds the address and reports to `sink`
/// with `ctx` until [`exact_listen_close`]. The handle is never 0. A
/// failure is the sink's `ERROR`, from the listener's thread.
///
/// # Safety
/// `config` and the bytes it points to are valid for the call; `ctx` stays
/// valid until `exact_listen_close(handle)` returns.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_open(
    config: *const Config,
    sink: Sink,
    ctx: *mut c_void,
) -> u64 {
    let c = &*config;
    let bytes = |p: *const u8, n: usize| {
        if n == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(p, n).to_vec()
        }
    };
    let options = Options {
        address: bytes(c.address, c.address_len),
        grants: String::from_utf8_lossy(&bytes(c.grants, c.grants_len)).into_owned(),
        takeover: c.takeover != 0,
        first_line_timeout_ms: c.first_line_timeout_ms,
        max_line_bytes: c.max_line_bytes.max(1) as usize,
        max_connections: c.max_connections.max(1) as usize,
    };
    open(options, sink, ctx as usize)
}

/// Queue `len` bytes and a `\n` on `connection`. 0: queued; 1: the
/// connection is gone; 2: the listener is.
///
/// # Safety
/// `data` is valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn exact_listen_send(
    handle: u64,
    connection: u64,
    data: *const u8,
    len: usize,
) -> i32 {
    let bytes = if len == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(data, len)
    };
    send(handle, connection, Some(bytes))
}

/// End `connection` once what was queued before has been written. 0, 1 or
/// 2 as [`exact_listen_send`].
#[no_mangle]
pub extern "C" fn exact_listen_end(handle: u64, connection: u64) -> i32 {
    send(handle, connection, None)
}

/// The bytes `connection` has queued and not yet written; `u64::MAX` when
/// it is gone.
#[no_mangle]
pub extern "C" fn exact_listen_buffered(handle: u64, connection: u64) -> u64 {
    buffered(handle, connection)
}

/// Close the listener: its connections end without events, its socket file
/// goes when it is still this listener's (D4), and its sink is not called
/// again once this returns.
#[no_mangle]
pub extern "C" fn exact_listen_close(handle: u64) {
    close(handle)
}

/// A listener's options, owned.
#[allow(dead_code)]
pub(crate) struct Options {
    pub address: Vec<u8>,
    pub grants: String,
    pub takeover: bool,
    pub first_line_timeout_ms: u32,
    pub max_line_bytes: usize,
    pub max_connections: usize,
}

#[cfg(unix)]
use server::{buffered, close, open, send};

#[cfg(not(unix))]
fn open(_: Options, sink: Sink, ctx: usize) -> u64 {
    let why = b"Unsupported: no listener on this platform";
    sink(ctx as *mut c_void, event::ERROR, 0, why.as_ptr(), why.len());
    1
}
#[cfg(not(unix))]
fn send(_: u64, _: u64, _: Option<&[u8]>) -> i32 {
    2
}
#[cfg(not(unix))]
fn buffered(_: u64, _: u64) -> u64 {
    u64::MAX
}
#[cfg(not(unix))]
fn close(_: u64) {}
