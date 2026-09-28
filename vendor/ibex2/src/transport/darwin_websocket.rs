//! `NSURLSessionWebSocketTask` behind `SocketTransport`. See
//! `src/engine/darwin_websocket.mm` for the Objective-C++ half: an ephemeral
//! session per socket that refuses redirects and only ever receives.

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::websocket::{Incoming, MessageSource, SocketTransport};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::Arc;

extern "C" {
    fn ibex2_darwin_ws_start(url: *const c_char, max_message: usize) -> *mut c_void;
    fn ibex2_darwin_ws_wait_open(handle: *mut c_void, out_error: *mut *mut c_char) -> c_int;
    fn ibex2_darwin_ws_next(
        handle: *mut c_void,
        out_kind: *mut c_int,
        out_data: *mut *mut c_char,
        out_len: *mut usize,
        out_code: *mut c_int,
    ) -> c_int;
    fn ibex2_darwin_ws_cancel(handle: *mut c_void);
    fn ibex2_darwin_ws_release(handle: *mut c_void);
    fn ibex2_darwin_ws_free(value: *mut c_void);
}

/// The platform socket on Apple platforms.
#[derive(Debug, Default, Clone, Copy)]
pub struct DarwinSocketTransport;

/// The native socket, retained until the last Rust user (a cancellation
/// callback may race the reader's drop).
struct Handle(usize);
impl Handle {
    fn pointer(&self) -> *mut c_void {
        self.0 as *mut c_void
    }
    fn cancel(&self) {
        // SAFETY: a retained, thread-safe native socket; cancel is idempotent.
        unsafe { ibex2_darwin_ws_cancel(self.pointer()) };
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: released once, after the final Rust user.
        unsafe { ibex2_darwin_ws_release(self.pointer()) };
    }
}

/// Take ownership of a native string (or its bytes).
unsafe fn take(raw: *mut c_char, len: Option<usize>) -> Option<Vec<u8>> {
    if raw.is_null() {
        return None;
    }
    let bytes = match len {
        Some(n) => std::slice::from_raw_parts(raw as *const u8, n).to_vec(),
        None => CStr::from_ptr(raw).to_bytes().to_vec(),
    };
    ibex2_darwin_ws_free(raw as *mut c_void);
    Some(bytes)
}

struct DarwinSocket {
    handle: Arc<Handle>,
    signal: AbortSignal,
    _registration: AbortRegistration,
    closed: bool,
}

impl SocketTransport for DarwinSocketTransport {
    fn connect(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        let target = CString::new(url.as_str())
            .map_err(|_| HostError::Failed("SyntaxError: invalid socket URL".into()))?;
        // SAFETY: start copies the URL and returns a retained socket or null.
        let raw = unsafe { ibex2_darwin_ws_start(target.as_ptr(), max_message) };
        if raw.is_null() {
            return Err(HostError::Failed(
                "the socket did not open: an invalid URL".into(),
            ));
        }
        let handle = Arc::new(Handle(raw as usize));
        let cancel = handle.clone();
        let registration = signal.register(move || cancel.cancel());
        let mut error = std::ptr::null_mut();
        // SAFETY: the handle is live for the whole blocking call.
        let failed = unsafe { ibex2_darwin_ws_wait_open(handle.pointer(), &mut error) };
        let error = unsafe { take(error, None) };
        signal.check()?;
        if failed != 0 {
            let why = String::from_utf8_lossy(&error.unwrap_or_default()).into_owned();
            return Err(HostError::Failed(format!("the socket did not open: {why}")));
        }
        Ok(Box::new(DarwinSocket {
            handle,
            signal: signal.clone(),
            _registration: registration,
            closed: false,
        }))
    }
}

impl MessageSource for DarwinSocket {
    fn next(&mut self) -> Result<Incoming, HostError> {
        if self.closed {
            return Ok(Incoming::Closed {
                code: 1006,
                reason: String::new(),
            });
        }
        let (mut kind, mut data, mut len, mut code) = (0, std::ptr::null_mut(), 0usize, 0);
        // SAFETY: the handle is live for the whole blocking call.
        let aborted = unsafe {
            ibex2_darwin_ws_next(
                self.handle.pointer(),
                &mut kind,
                &mut data,
                &mut len,
                &mut code,
            )
        };
        let bytes = unsafe { take(data, (kind == 0).then_some(len)) };
        if aborted != 0 {
            self.signal.check()?;
            return Err(HostError::Failed("the socket was closed".into()));
        }
        Ok(match kind {
            0 => Incoming::Text(String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned()),
            1 => Incoming::Binary(len),
            3 => Incoming::TooLarge,
            _ => {
                self.closed = true;
                Incoming::Closed {
                    code: code as u16,
                    reason: String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned(),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_darwin_transport_holds_the_whole_conversation() {
        super::super::websocket::tests::conversation(&super::DarwinSocketTransport);
    }
}
