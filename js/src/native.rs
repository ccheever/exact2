//! A separately linked app capability at the data seam, after first pixel.
//! @ref LLP 1027 D8 / D10 — native computation beside TypeScript; host grants.

use serde_json::Value;
use std::path::PathBuf;

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
}
