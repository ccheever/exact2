//! `NSURLSessionWebSocketTask` behind `SocketTransport`. See
//! `src/transport/darwin_websocket.mm` for the Objective-C++ half: an ephemeral
//! session per socket that refuses redirects and sends with the platform task.

use crate::boundary::HostError;
use crate::stdlib::abort::{AbortRegistration, AbortSignal};
use crate::stdlib::websocket::{
    Event, Incoming, Message, MessageSender, MessageSource, SocketTransport,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::Arc;

extern "C" {
    fn ibex2_darwin_ws_start(
        url: *const c_char,
        protocols: *const c_char,
        max_message: usize,
    ) -> *mut c_void;
    #[cfg(test)]
    fn ibex2_darwin_ws_start_with_test_certificate(
        url: *const c_char,
        protocols: *const c_char,
        max_message: usize,
        certificate: *const u8,
        certificate_len: usize,
    ) -> *mut c_void;
    fn ibex2_darwin_ws_wait_open(
        handle: *mut c_void,
        out_error: *mut *mut c_char,
        out_protocol: *mut *mut c_char,
    ) -> c_int;
    fn ibex2_darwin_ws_next(
        handle: *mut c_void,
        out_kind: *mut c_int,
        out_data: *mut *mut c_char,
        out_len: *mut usize,
        out_code: *mut c_int,
    ) -> c_int;
    fn ibex2_darwin_ws_cancel(handle: *mut c_void);
    fn ibex2_darwin_ws_send_text(handle: *mut c_void, data: *const u8, len: usize) -> c_int;
    fn ibex2_darwin_ws_send_binary(handle: *mut c_void, data: *const u8, len: usize) -> c_int;
    fn ibex2_darwin_ws_close(handle: *mut c_void, code: c_int, reason: *const u8, len: usize);
    fn ibex2_darwin_ws_buffered_amount(handle: *mut c_void) -> usize;
    fn ibex2_darwin_ws_release(handle: *mut c_void);
    fn ibex2_darwin_ws_free(value: *mut c_void);
    #[cfg(test)]
    fn ibex2_darwin_ws_test_pause_next_send(handle: *mut c_void);
    #[cfg(test)]
    fn ibex2_darwin_ws_test_wait_send_admitted(handle: *mut c_void);
    #[cfg(test)]
    fn ibex2_darwin_ws_test_wait_close_attempted(handle: *mut c_void);
    #[cfg(test)]
    fn ibex2_darwin_ws_test_wait_close_submitted(
        handle: *mut c_void,
        timeout_seconds: f64,
    ) -> c_int;
    #[cfg(test)]
    fn ibex2_darwin_ws_test_resume_send(handle: *mut c_void);
    #[cfg(test)]
    fn ibex2_darwin_ws_test_send_submitted_after_close(handle: *mut c_void) -> c_int;
}

/// The platform socket on Apple platforms.
#[derive(Debug, Default, Clone, Copy)]
pub struct DarwinSocketTransport;

#[cfg(test)]
struct DarwinTestSocketTransport {
    certificate: Vec<u8>,
}

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
    sender: Arc<DarwinSender>,
    protocol: String,
}

struct DarwinSender {
    handle: Arc<Handle>,
}

impl MessageSender for DarwinSender {
    fn send_text(&self, text: &str) -> Result<(), HostError> {
        let result =
            unsafe { ibex2_darwin_ws_send_text(self.handle.pointer(), text.as_ptr(), text.len()) };
        (result == 0)
            .then_some(())
            .ok_or_else(|| HostError::Failed("the socket could not queue a text message".into()))
    }

    fn send_binary(&self, bytes: &[u8]) -> Result<(), HostError> {
        let result = unsafe {
            ibex2_darwin_ws_send_binary(self.handle.pointer(), bytes.as_ptr(), bytes.len())
        };
        (result == 0)
            .then_some(())
            .ok_or_else(|| HostError::Failed("the socket could not queue a binary message".into()))
    }

    fn close(&self, code: Option<u16>, reason: &str) -> Result<(), HostError> {
        unsafe {
            ibex2_darwin_ws_close(
                self.handle.pointer(),
                code.map_or(0, c_int::from),
                reason.as_ptr(),
                reason.len(),
            )
        };
        Ok(())
    }

    fn buffered_amount(&self) -> usize {
        unsafe { ibex2_darwin_ws_buffered_amount(self.handle.pointer()) }
    }
}

impl DarwinSocketTransport {
    fn connect_impl(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
        protocols: &[String],
        test_certificate: Option<&[u8]>,
    ) -> Result<DarwinSocket, HostError> {
        let target = CString::new(url.as_str())
            .map_err(|_| HostError::Failed("SyntaxError: invalid socket URL".into()))?;
        let protocols = CString::new(protocols.join(","))
            .map_err(|_| HostError::InvalidArgument("invalid WebSocket protocol".into()))?;
        // SAFETY: start copies the URL, protocols and optional test anchor,
        // and returns a retained socket or null.
        let raw = match test_certificate {
            #[cfg(test)]
            Some(certificate) => unsafe {
                ibex2_darwin_ws_start_with_test_certificate(
                    target.as_ptr(),
                    protocols.as_ptr(),
                    max_message,
                    certificate.as_ptr(),
                    certificate.len(),
                )
            },
            _ => unsafe { ibex2_darwin_ws_start(target.as_ptr(), protocols.as_ptr(), max_message) },
        };
        if raw.is_null() {
            return Err(HostError::Failed(
                "the socket did not open: an invalid URL".into(),
            ));
        }
        let handle = Arc::new(Handle(raw as usize));
        let cancel = handle.clone();
        let registration = signal.register(move || cancel.cancel());
        let (mut error, mut selected) = (std::ptr::null_mut(), std::ptr::null_mut());
        // SAFETY: the handle is live for the whole blocking call.
        let failed =
            unsafe { ibex2_darwin_ws_wait_open(handle.pointer(), &mut error, &mut selected) };
        let error = unsafe { take(error, None) };
        let selected = unsafe { take(selected, None) };
        signal.check()?;
        if failed != 0 {
            let why = String::from_utf8_lossy(&error.unwrap_or_default()).into_owned();
            return Err(HostError::Failed(format!("the socket did not open: {why}")));
        }
        let sender = Arc::new(DarwinSender {
            handle: Arc::clone(&handle),
        });
        Ok(DarwinSocket {
            handle,
            signal: signal.clone(),
            _registration: registration,
            closed: false,
            sender,
            protocol: String::from_utf8_lossy(&selected.unwrap_or_default()).into_owned(),
        })
    }
}

impl SocketTransport for DarwinSocketTransport {
    fn connect(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        self.connect_with_protocols(url, max_message, signal, &[])
    }

    fn connect_with_protocols(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
        protocols: &[String],
    ) -> Result<Box<dyn MessageSource>, HostError> {
        self.connect_impl(url, max_message, signal, protocols, None)
            .map(|socket| Box::new(socket) as Box<dyn MessageSource>)
    }
}

#[cfg(test)]
impl SocketTransport for DarwinTestSocketTransport {
    fn connect(
        &self,
        url: &url::Url,
        max_message: usize,
        signal: &AbortSignal,
    ) -> Result<Box<dyn MessageSource>, HostError> {
        DarwinSocketTransport
            .connect_impl(url, max_message, signal, &[], Some(&self.certificate))
            .map(|socket| Box::new(socket) as Box<dyn MessageSource>)
    }
}

enum Received {
    Text(String),
    Binary(Vec<u8>),
    TooLarge,
    Closed { code: u16, reason: String },
}

impl DarwinSocket {
    fn receive(&mut self) -> Result<Received, HostError> {
        if self.closed {
            return Ok(Received::Closed {
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
        let bytes = unsafe { take(data, matches!(kind, 0 | 1).then_some(len)) };
        if aborted != 0 {
            self.signal.check()?;
            return Err(HostError::Failed("the socket was closed".into()));
        }
        Ok(match kind {
            0 => Received::Text(String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned()),
            1 => Received::Binary(bytes.unwrap_or_default()),
            3 => Received::TooLarge,
            _ => {
                self.closed = true;
                Received::Closed {
                    code: code as u16,
                    reason: String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned(),
                }
            }
        })
    }
}

impl MessageSource for DarwinSocket {
    fn next(&mut self) -> Result<Incoming, HostError> {
        Ok(match self.receive()? {
            Received::Text(text) => Incoming::Text(text),
            Received::Binary(bytes) => Incoming::Binary(bytes.len()),
            Received::TooLarge => Incoming::TooLarge,
            Received::Closed { code, reason } => Incoming::Closed { code, reason },
        })
    }

    fn next_event(&mut self) -> Result<Event, HostError> {
        Ok(match self.receive()? {
            Received::Text(text) => Event::Message(Message::Text(text)),
            Received::Binary(bytes) => Event::Message(Message::Binary(bytes)),
            Received::TooLarge => {
                return Err(HostError::Failed(
                    "the socket message exceeded its configured limit".into(),
                ));
            }
            Received::Closed { code, reason } => Event::Close {
                code,
                reason,
                was_clean: code != 1006,
            },
        })
    }

    fn sender(&self) -> Option<Arc<dyn MessageSender>> {
        Some(self.sender.clone())
    }

    fn protocol(&self) -> &str {
        &self.protocol
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn concurrent_send_and_close_submit_to_the_platform_in_admission_order() {
        use crate::stdlib::abort::AbortSignal;
        use crate::stdlib::websocket::MessageSender;

        let (port, seen) = super::super::websocket::tests::peer();
        let url = url::Url::parse(&format!("ws://127.0.0.1:{port}/echo")).unwrap();
        let socket = super::DarwinSocketTransport
            .connect_impl(&url, 1024, &AbortSignal::default(), &[], None)
            .unwrap();
        let handle = socket.handle.pointer();
        // SAFETY: the concrete socket remains alive through both racing calls.
        unsafe { super::ibex2_darwin_ws_test_pause_next_send(handle) };

        let sending = std::sync::Arc::clone(&socket.sender);
        let send = std::thread::spawn(move || sending.send_text("racing"));
        unsafe { super::ibex2_darwin_ws_test_wait_send_admitted(handle) };

        let closing = std::sync::Arc::clone(&socket.sender);
        let close = std::thread::spawn(move || closing.close(Some(1000), ""));
        unsafe { super::ibex2_darwin_ws_test_wait_close_attempted(handle) };
        // On the broken implementation close crosses the gap while send is
        // paused. On the fixed implementation it is blocked by send's atomic
        // admission/submission section, so this bounded wait returns false.
        let _ = unsafe { super::ibex2_darwin_ws_test_wait_close_submitted(handle, 0.25) };
        unsafe { super::ibex2_darwin_ws_test_resume_send(handle) };

        send.join().unwrap().unwrap();
        close.join().unwrap().unwrap();
        assert_eq!(
            unsafe { super::ibex2_darwin_ws_test_send_submitted_after_close(handle) },
            0,
            "an admitted send was submitted to NSURLSession after close"
        );
        assert_eq!(
            seen.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            "/echo message 1 6"
        );
        assert_eq!(
            seen.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            "/echo close 1000 "
        );
    }

    #[test]
    fn the_darwin_transport_holds_the_whole_conversation() {
        super::super::websocket::tests::conversation(&super::DarwinSocketTransport);
    }

    #[test]
    fn the_darwin_transport_echoes_and_closes_over_local_tls() {
        use crate::stdlib::abort::AbortSignal;
        use crate::stdlib::websocket::{Incoming, SocketTransport};

        let (port, certificate, _client, peer) = super::super::websocket::tests::tls_echo_peer();
        let transport = super::DarwinTestSocketTransport { certificate };
        let url = url::Url::parse(&format!("wss://localhost:{port}/echo")).unwrap();
        let mut socket = transport
            .connect(&url, 1024, &AbortSignal::default())
            .unwrap();
        socket.send_text("secure apple").unwrap();
        assert_eq!(
            socket.next().unwrap(),
            Incoming::Text("secure apple".into())
        );
        socket.close(1000, "tls done").unwrap();
        assert_eq!(
            socket.next().unwrap(),
            Incoming::Closed {
                code: 1000,
                reason: "tls done".into(),
            }
        );
        peer.join().unwrap();
    }
}
