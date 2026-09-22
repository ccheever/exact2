//! Installable bindings for a caller-owned JSI runtime.
//!
//! Rust consumers use `host::Bindings` directly. A JS embedder compiles
//! `JSI_SOURCE` against its own JSI headers, bakes `SQLITE_SOURCE` with its
//! engine's compiler, and creates an `Adapter` from `JSI_HEADER`. This module
//! links no engine, installs no globals, and owns no application loop.
//!
//! @ref LLP 0068#2-synchronous-and-why — the consumer owns execution
use crate::{grant::GrantSet, task::RuntimeState};
use std::{ffi::c_void, sync::Arc, time::Duration};

pub const JSI_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/engine/ibex2_jsi.cc");
pub const JSI_HEADER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/include/ibex2_jsi.h");
pub const HARDEN_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/harden.js");
pub const SQLITE_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/sqlite.js");
pub const TYPESCRIPT: &str = include_str!("bindings/storage.d.ts");

/// Rust resources borrowed by one JSI adapter. Create after first pixel,
/// configure before installation, and detach the adapter before dropping this.
/// Each context has a separate completion queue and database handle space.
pub struct Context {
    state: Arc<RuntimeState>,
    grants: Arc<GrantSet>,
}

impl Context {
    /// The host supplies already-admitted grants; never trust app exports to
    /// authorize themselves. Empty grants install capabilities that refuse.
    pub fn new(grants: GrantSet) -> Self {
        Self {
            state: Arc::new(RuntimeState::new(crate::transport::default_transport())),
            grants: Arc::new(grants),
        }
    }

    pub fn set_app_directories(
        &self,
        directories: crate::stdlib::app_fs::AppDirectories,
    ) -> Result<(), crate::boundary::HostError> {
        self.state.set_app_directories(directories)
    }

    pub fn set_sqlite_provider(
        &self,
        provider: Arc<dyn crate::stdlib::sqlite::Provider>,
    ) -> Result<(), crate::boundary::HostError> {
        self.state.set_sqlite_provider(provider)
    }

    /// Worker-safe notification that schedules the embedder's loop; never
    /// execute JS in this callback. Install before starting work.
    pub fn set_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.state.queue.set_wake(Some(wake));
    }

    /// A blocking executor can wait instead of installing a wake callback.
    pub fn wait(&self, timeout: Duration) -> bool {
        self.state.queue.wait(timeout)
    }

    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    /// Borrowed Arc-backed pointer for the JSI adapter. The context must
    /// outlive the adapter's detach; the adapter never releases this pointer.
    pub fn state_ptr(&self) -> *const c_void {
        Arc::as_ptr(&self.state).cast()
    }

    /// Borrowed Arc-backed grants. Adapter-created functions retain their
    /// own references, so copied capabilities keep the installer's authority.
    pub fn grants_ptr(&self) -> *const c_void {
        Arc::as_ptr(&self.grants).cast()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        self.state.queue.set_wake(None);
        self.state.shutdown();
    }
}
