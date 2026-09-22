//! `NSURLSession` behind the `Transport` trait.
//!
//! See `src/engine/darwin_http.mm` for the Objective-C++ half, and in
//! particular for why redirects are refused there: Rust re-checks the
//! `net.fetch` grant on every hop, so a platform that followed redirects
//! internally would deliver a response from an origin the caller was never
//! granted.

use std::ffi::{c_char, c_int, c_uchar, CStr, CString};

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::fetch::{Body, BodySource, Headers, Request, StreamingResponse, Transport};
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    Arc,
};

extern "C" {
    fn ibex2_darwin_session_create() -> *mut std::ffi::c_void;
    fn ibex2_darwin_session_destroy(handle: *mut std::ffi::c_void);
    /// Reports whether the session kept a cookie jar / URL cache, so the
    /// claim can be tested rather than asserted in a comment. Test-only: the
    /// runtime never needs to ask.
    #[cfg(test)]
    fn ibex2_darwin_session_has_state(
        handle: *mut std::ffi::c_void,
        out_cookies: *mut c_int,
        out_cache: *mut c_int,
    );
    fn ibex2_darwin_http_start(
        session: *mut std::ffi::c_void,
        method: *const c_char,
        url: *const c_char,
        header_block: *const c_char,
        body: *const c_uchar,
        body_len: usize,
        max_body: usize,
        out_error: *mut *mut c_char,
    ) -> *mut std::ffi::c_void;
    fn ibex2_darwin_http_headers(
        handle: *mut std::ffi::c_void,
        out_status: *mut c_int,
        out_headers: *mut *mut c_char,
        out_error: *mut *mut c_char,
    ) -> c_int;
    fn ibex2_darwin_http_read(
        handle: *mut std::ffi::c_void,
        output: *mut c_uchar,
        capacity: usize,
        out_length: *mut usize,
        out_error: *mut *mut c_char,
        out_reused: *mut c_int,
    ) -> c_int;
    fn ibex2_darwin_http_cancel(handle: *mut std::ffi::c_void);
    fn ibex2_darwin_http_release(handle: *mut std::ffi::c_void);
    fn ibex2_darwin_free(value: *mut std::ffi::c_void);
}

/// The platform transport on Apple platforms.
///
/// Owns a lazy pool of at most four `NSURLSession`s. Each session has a serial
/// delegate queue and is leased to one response until native completion.
/// This bounds concurrent callbacks while keeping their order,
/// connection reuse, and isolation between hosts. A fifth open waits cancellably
/// for a lease; consumers must consume or drop large unfinished responses to
/// return their leases. Small responses prefetch into the bounded queue and
/// return their session when complete, even before the body is consumed.
///
/// Each body hands off through a 64 KiB queue. A full queue blocks only that
/// lease's delegate, never callbacks for another leased session. Foundation may
/// also own one callback's `NSData` and internal network buffers; their size is
/// platform-controlled. There is no body-sized allocation in Ibex.
///
/// CFNetwork can defer its response callback until an initial body byte arrives,
/// including with an explicit MIME type. `open` returns at that callback, before
/// body completion. Cancellation also interrupts this pre-header interval.
///
/// Sessions remain ephemeral, with no cookie jar or cache. Creating the pool is
/// lazy and creating each platform session happens on its first lease, keeping
/// CFNetwork initialization off the boot path of programs which never fetch.
#[derive(Debug, Default)]
pub struct DarwinTransport {
    /// The pool pointer, as a `usize` so the struct stays plainly `Send`
    /// and `Sync` without an `unsafe impl` asserting it. The native pool is
    /// guarded by NSCondition; this pointer is written once and only read after.
    session: std::sync::OnceLock<usize>,
    /// Whether the most recent request reused a connection: 1 yes, 0 no, -1
    /// not reported. Test-facing; nothing in the runtime reads it.
    last_reused: Arc<AtomicIsize>,
}

impl DarwinTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the last request rode an already-open connection.
    /// `Some(true)`/`Some(false)`, or `None` when the platform did not report.
    pub fn last_connection_was_reused(&self) -> Option<bool> {
        match self.last_reused.load(std::sync::atomic::Ordering::Relaxed) {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        }
    }

    /// Lazily initialize the native pool. Racing first calls may create two
    /// cheap empty pools; OnceLock keeps one and releases the unused one.
    fn session(&self) -> *mut std::ffi::c_void {
        if let Some(existing) = self.session.get() {
            return *existing as *mut std::ffi::c_void;
        }
        // SAFETY: returns a retained pool or null. Null is handled by the
        // caller, which fails the request rather than dereferencing it.
        let created = unsafe { ibex2_darwin_session_create() };
        match self.session.set(created as usize) {
            Ok(()) => created,
            Err(_) => {
                // Another thread won. Release ours and use theirs.
                // SAFETY: `created` is ours alone and has not been shared.
                unsafe { ibex2_darwin_session_destroy(created) };
                *self.session.get().expect("set by the winner") as *mut std::ffi::c_void
            }
        }
    }
}

impl Drop for DarwinTransport {
    fn drop(&mut self) {
        if let Some(session) = self.session.get() {
            // SAFETY: came from `ibex2_darwin_session_create`, released once.
            unsafe { ibex2_darwin_session_destroy(*session as *mut std::ffi::c_void) };
        }
    }
}

unsafe fn take_string(raw: *mut c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    let text = CStr::from_ptr(raw).to_string_lossy().into_owned();
    ibex2_darwin_free(raw as *mut std::ffi::c_void);
    Some(text)
}

// Native state is guarded by NSCondition. An Arc keeps it retained while an
// already claimed cancellation callback races the body's destruction.
struct Exchange(usize);
impl Exchange {
    fn pointer(&self) -> *mut std::ffi::c_void {
        self.0 as *mut std::ffi::c_void
    }
    fn cancel(&self) {
        // SAFETY: this Arc owns a retained, thread-safe native exchange.
        unsafe { ibex2_darwin_http_cancel(self.pointer()) };
    }
}
impl Drop for Exchange {
    fn drop(&mut self) {
        // SAFETY: released once, after the final Rust user including callbacks.
        unsafe { ibex2_darwin_http_release(self.pointer()) };
    }
}
struct DarwinBody {
    exchange: Arc<Exchange>,
    _registration: AbortRegistration,
    last_reused: Arc<AtomicIsize>,
}
impl Drop for DarwinBody {
    fn drop(&mut self) {
        self.exchange.cancel();
    }
}
impl BodySource for DarwinBody {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, HostError> {
        if output.is_empty() {
            return Ok(0);
        }
        let (mut count, mut error, mut reused) = (0, std::ptr::null_mut(), -1);
        // SAFETY: exchange and output span are live for the entire blocking call.
        let failed = unsafe {
            ibex2_darwin_http_read(
                self.exchange.pointer(),
                output.as_mut_ptr(),
                output.len(),
                &mut count,
                &mut error,
                &mut reused,
            )
        };
        self.last_reused.store(reused as isize, Ordering::Relaxed);
        let error = unsafe { take_string(error) };
        if failed != 0 {
            Err(HostError::Failed(
                error.unwrap_or_else(|| "TypeError: Failed to fetch".into()),
            ))
        } else {
            Ok(count)
        }
    }
}
impl Transport for DarwinTransport {
    fn open(
        &self,
        request: &Request,
        signal: &AbortSignal,
    ) -> Result<StreamingResponse, HostError> {
        signal.check()?;
        let method = CString::new(request.method.as_str())
            .map_err(|_| HostError::Failed("TypeError: invalid method".into()))?;
        let url = CString::new(request.url.as_str())
            .map_err(|_| HostError::Failed("TypeError: invalid URL".into()))?;
        let headers = request
            .headers
            .entries()
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<_>>()
            .join("\n");
        let headers = CString::new(headers)
            .map_err(|_| HostError::Failed("TypeError: invalid header value".into()))?;
        let body = request.body.as_deref().unwrap_or_default();
        let mut error = std::ptr::null_mut();
        // SAFETY: native start copies every input and returns a retained exchange.
        let handle = unsafe {
            ibex2_darwin_http_start(
                self.session(),
                method.as_ptr(),
                url.as_ptr(),
                headers.as_ptr(),
                body.as_ptr(),
                body.len(),
                request.body_limit(),
                &mut error,
            )
        };
        let error = unsafe { take_string(error) };
        if handle.is_null() {
            return Err(HostError::Failed(
                error.unwrap_or_else(|| "TypeError: Failed to fetch".into()),
            ));
        }
        let exchange = Arc::new(Exchange(handle as usize));
        let cancel = exchange.clone();
        let registration = signal.register(move || cancel.cancel());
        let source = DarwinBody {
            exchange,
            _registration: registration,
            last_reused: self.last_reused.clone(),
        };
        let (mut status, mut headers, mut error) = (0, std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: source owns the exchange until success transfers it to Body,
        // or any error drops it and cancels the outstanding task.
        let failed = unsafe {
            ibex2_darwin_http_headers(
                source.exchange.pointer(),
                &mut status,
                &mut headers,
                &mut error,
            )
        };
        let headers_text = unsafe { take_string(headers) }.unwrap_or_default();
        let error = unsafe { take_string(error) };
        signal.check()?;
        if failed != 0 {
            return Err(HostError::Failed(
                error.unwrap_or_else(|| "TypeError: Failed to fetch".into()),
            ));
        }
        let mut headers = Headers::new();
        for line in headers_text.lines() {
            if let Some((name, value)) = line.split_once(": ") {
                headers.set_response(name, value);
            }
        }
        Ok(StreamingResponse {
            status: status as u16,
            status_text: String::new(),
            headers,
            body: Body::new(Box::new(source), request.body_limit(), signal.clone()),
            url: request.url.clone(),
            redirected: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two runtimes must not share a session, or the ephemeral configuration
    /// stops meaning anything: connection state, and anything the platform
    /// caches with it, would cross a boundary the capability model draws.
    #[test]
    fn each_transport_owns_its_own_session() {
        let a = DarwinTransport::new();
        let b = DarwinTransport::new();
        assert!(!a.session().is_null(), "session was not created");
        assert!(!b.session().is_null(), "session was not created");
        assert_ne!(a.session(), b.session(), "two runtimes shared one session");
    }

    /// Construction must not build the session, or every program pays ~19ms of
    /// CFNetwork initialization on its boot floor whether it fetches or not.
    #[test]
    fn constructing_a_transport_does_not_build_a_session() {
        let t = DarwinTransport::new();
        assert!(
            t.session.get().is_none(),
            "the session was created eagerly; that cost belongs on the first request"
        );
    }

    /// The reuse claim, asserted directly.
    ///
    /// A session owns the connection pool. With one session per request every
    /// call opened a new connection and paid a full TLS handshake.
    ///
    /// **This does not time anything.** An earlier version required a repeat
    /// request under 40ms, which was wrong twice over: it passed because
    /// `NSURLCache` was answering from memory without any connection at all,
    /// and once the cache was disabled it failed whenever the network was
    /// merely slow — 64ms on a working pool. Latency cannot distinguish a
    /// pooled request from a fast handshake. `NSURLSessionTaskMetrics` says
    /// outright whether the connection was reused, so that is what is asserted.
    #[test]
    fn a_second_request_to_one_origin_reuses_the_connection() {
        let transport = DarwinTransport::new();
        if transport
            .send(&Request::get("https://example.com/"))
            .is_err()
        {
            return; // no network; nothing to measure
        }
        // The first request may or may not have opened a connection; only the
        // repeats carry the claim.
        let Some(false) = transport.last_connection_was_reused() else {
            return; // platform did not report metrics; nothing to assert on
        };

        for attempt in 0..3 {
            assert!(
                transport
                    .send(&Request::get("https://example.com/"))
                    .is_ok(),
                "request {attempt} failed after the first one succeeded"
            );
            assert_eq!(
                transport.last_connection_was_reused(),
                Some(true),
                "request {attempt} opened a new connection; the session is not being reused"
            );
        }
    }

    /// Cookies are ambient authority the grant check cannot see, so the
    /// session must not keep any. Asserted on the configuration rather than by
    /// round-tripping a `Set-Cookie`, which would need a server that sets one.
    #[test]
    fn the_session_keeps_no_cookies_and_no_cache() {
        let transport = DarwinTransport::new();
        let session = transport.session();
        assert!(!session.is_null(), "session was not created");
        // SAFETY: `session` is this transport's live session.
        let (mut cookies, mut cache) = (0, 0);
        unsafe { ibex2_darwin_session_has_state(session, &mut cookies, &mut cache) };
        assert_eq!(cookies, 0, "the session has a cookie jar");
        assert_eq!(cache, 0, "the session has a URL cache");
    }
}

#[cfg(test)]
mod session_cost {
    use super::*;

    /// What the first session in a process costs, which is why it is built on
    /// first use rather than at construction: on the runtime construction path
    /// this would land on the boot floor of every program, against LLP 0063's
    /// ~4ms. `constructing_a_transport_does_not_build_a_session` is the guard;
    /// this is the number behind it.
    #[test]
    #[ignore = "measurement, not an assertion"]
    fn report_session_construction_cost() {
        for i in 1..=5 {
            let t = DarwinTransport::new();
            let started = std::time::Instant::now();
            let _ = t.session();
            eprintln!("  session #{i}: {:?}", started.elapsed());
            drop(t);
        }
    }
}

#[cfg(test)]
#[path = "darwin_stream_tests.rs"]
mod stream_tests;
