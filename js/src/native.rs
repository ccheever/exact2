//! A separately linked app capability at the data seam, after first pixel.
//! @ref LLP 1027 D8 / D10 — native computation beside TypeScript; host grants.

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

/// A native module's handler for long calls (`native.later` in TypeScript).
pub type LaterHandler = Arc<dyn Fn(Value, NativeReply) + Send + Sync>;

/// The answer to one `native.later` call, sent once from whatever thread
/// the work ends on. Dropped unsent, the call fails as aborted.
pub struct NativeReply(exact_runner::Reply);

impl NativeReply {
    /// Wrap the host's reply for a native request.
    pub fn new(reply: exact_runner::Reply) -> NativeReply {
        NativeReply(reply)
    }

    /// Resolve the TypeScript promise with `Ok`'s value, or reject it with
    /// `Err`'s message.
    pub fn send(self, result: Result<Value, String>) {
        let (status, body) = match result {
            Ok(value) => (200, value.to_string().into_bytes()),
            Err(message) => (500, message.into_bytes()),
        };
        self.0
            .send(exact_runner::Outcome::Response(exact_runner::Response {
                status,
                headers: Vec::new(),
                body,
            }));
    }
}

/// The app's native JSON module. The executor knows no module names or crates;
/// the app links and supplies one implementation, which enforces its grants.
/// HTTP remains ordinary `fetch`, executed through the host's request seam.
pub trait NativeModule {
    /// Record host-selected roots; do not open files here. This is never called
    /// for bake, agent mode, or a disposable replacement validation.
    fn configure_storage(
        &mut self,
        data: PathBuf,
        cache: PathBuf,
        temporary: PathBuf,
    ) -> Result<(), String>;

    /// Answer one JSON call synchronously on the owning source's thread.
    /// Called only inside an activated answer and counted as an external read.
    fn call(&mut self, request: &Value) -> Result<Value, String>;

    /// Long work — a transcription, a model's answer — that must not hold the
    /// source's thread or its 100 ms budget: the host calls the handler with
    /// each `native.later` request on a thread of its own, and the promise
    /// settles when the handler's work sends the reply. The handler starts
    /// the work and returns; it never blocks for it. Asked once, after
    /// `configure_storage`. `None`, the default: `native.later` answers
    /// through [`NativeModule::call`], inside the answer.
    fn later(&mut self) -> Option<LaterHandler> {
        None
    }
}
