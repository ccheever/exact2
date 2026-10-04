//! Transports: the platform half of the sandwich.
//!
//! LLP 0057 §3 draws the line here. Everything above — header folding, redirect
//! policy, the body state machine, the error taxonomy — is Rust's and is
//! identical on every platform. Everything below is the operating system's:
//! sockets, TLS, proxy configuration, HTTP/2 and /3, connection pooling, and
//! the certificate store. Inverting that gives four platforms four different
//! `fetch`es, which is the failure a cross-platform runtime exists to prevent.
//!
//! @ref LLP 0057#3-the-boundary — Rust owns semantics, the platform owns transport

pub mod dev_tcp;
pub use dev_tcp::DevTcpTransport;

#[cfg(target_vendor = "apple")]
pub mod darwin;
#[cfg(target_vendor = "apple")]
pub use darwin::DarwinTransport;
// Also on macOS, never the default there: a server may choose it (Exact patch 3).
#[cfg(any(not(target_vendor = "apple"), target_os = "macos", test))]
pub mod rustls_http;
#[cfg(any(not(target_vendor = "apple"), target_os = "macos"))]
pub use rustls_http::RustlsHttpTransport;
// A listening WebSocket (LLP 0059.000 §3.12): the platform's own on Apple,
// TCP and rustls elsewhere (and in tests everywhere).
#[cfg(target_vendor = "apple")]
pub mod darwin_websocket;
#[cfg(any(not(target_vendor = "apple"), test))]
pub mod websocket;
#[cfg(windows)]
mod windows_connect;

/// The transport this build uses by default.
///
/// Apple platforms get `NSURLSession`; everything else gets rustls through
/// `ureq` (the development TCP transport stays for tests that want plaintext
/// and no dependency).
pub fn default_transport() -> Box<dyn crate::stdlib::fetch::Transport> {
    // The platform transport, engine or not: a Rust consumer (LLP 0068) has
    // no engine in the process and gets the same `fetch` underneath.
    #[cfg(target_vendor = "apple")]
    {
        Box::new(DarwinTransport::new())
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        // rustls through `ureq` (OQ2, 2026-08-30): TLS off Apple, in Rust.
        Box::new(RustlsHttpTransport::new())
    }
}

/// Context construction should not initialize a platform transport that an
/// owning runtime is about to replace. The cloned host endowment keeps this
/// cell, so the selected default is constructed only if that endowment fetches.
pub(crate) struct LazyDefaultTransport {
    transport: std::sync::OnceLock<Box<dyn crate::stdlib::fetch::Transport>>,
}

impl LazyDefaultTransport {
    pub(crate) fn new() -> Self {
        Self {
            transport: std::sync::OnceLock::new(),
        }
    }
}

impl crate::stdlib::fetch::Transport for LazyDefaultTransport {
    fn open(
        &self,
        request: &crate::stdlib::fetch::Request,
        signal: &crate::stdlib::abort::AbortSignal,
    ) -> Result<crate::stdlib::fetch::StreamingResponse, crate::boundary::HostError> {
        self.transport
            .get_or_init(default_transport)
            .open(request, signal)
    }
}

#[cfg(test)]
mod stream_tests;
