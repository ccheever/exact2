//! Installable bindings for a caller-owned JSI runtime.
//!
//! Rust consumers use `host::Bindings` directly. A JS embedder compiles
//! `JSI_SOURCE` against its own JSI headers, bakes [`scripts`] with its
//! engine's compiler, and creates an `Adapter` from `JSI_HEADER`. This module
//! links no engine and owns no application loop.
//!
//! @ref LLP 0068#2-synchronous-and-why — the consumer owns execution
use crate::{grant::GrantSet, host, task::RuntimeState};
use std::{
    collections::HashSet,
    ffi::c_void,
    fmt, ops,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

pub const JSI_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/install.cc");
pub const JSI_HEADER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/include/ibex2_jsi.h");
/// Hardening source an embedder must evaluate after installation and before
/// application code. The guarantees documented for binding private state and
/// trusted events require this step.
pub const HARDEN_SOURCE: &str = include_str!(env!("IBEX2_HARDEN_SOURCE_PATH"));
/// Filesystem path to the same source for build systems that accept inputs by path.
pub const HARDEN_SOURCE_PATH: &str = env!("IBEX2_HARDEN_SOURCE_PATH");
/// Filesystem path to hardening bytecode compiled by the matching `hermesc`.
pub const HARDEN_BYTECODE_PATH: &str = env!("IBEX2_HARDEN_BYTECODE_PATH");
/// Hardening bytecode compiled from [`HARDEN_SOURCE`].
pub const HARDEN_BYTECODE: &[u8] = include_bytes!(env!("IBEX2_HARDEN_BYTECODE_PATH"));
/// Digest of the full VM archive in the one `hermes-lean-sys` resolution.
pub const ENGINE_DIGEST: &str = env!("IBEX2_BINDINGS_ENGINE_DIGEST");
/// Digest of the lean VM archive in that resolution, when present. If the
/// install has a receipt, its archive manifest is always checked before this
/// identity is exported; an unbound lean archive fails the build.
pub const LEAN_ENGINE_DIGEST: Option<&str> = option_env!("IBEX2_BINDINGS_LEAN_ENGINE_DIGEST");
/// Receipt-bound base ICU data archive available on Linux.
///
/// This is not a selected or linked identity: bindings link no ICU data. A
/// process must use the linking `hermes-lean-sys` instance's
/// `LINKED_ICU_DATA_ARCHIVE` to learn what it actually links.
pub const ICU_DATA_ARCHIVE: Option<&str> = option_env!("IBEX2_BINDINGS_ICU_DATA_ARCHIVE");
/// Digest of the available [`ICU_DATA_ARCHIVE`].
pub const ICU_DATA_DIGEST: Option<&str> = option_env!("IBEX2_BINDINGS_ICU_DATA_DIGEST");
/// Receipt-bound English Intl ICU data archive available on Linux.
pub const ICU_EN_DATA_ARCHIVE: Option<&str> = option_env!("IBEX2_BINDINGS_ICU_EN_DATA_ARCHIVE");
/// Digest of the available [`ICU_EN_DATA_ARCHIVE`].
pub const ICU_EN_DATA_DIGEST: Option<&str> = option_env!("IBEX2_BINDINGS_ICU_EN_DATA_DIGEST");
/// Receipt-bound full ICU data archive available on Linux.
pub const ICU_FULL_DATA_ARCHIVE: Option<&str> = option_env!("IBEX2_BINDINGS_ICU_FULL_DATA_ARCHIVE");
/// Digest of the available [`ICU_FULL_DATA_ARCHIVE`].
pub const ICU_FULL_DATA_DIGEST: Option<&str> = option_env!("IBEX2_BINDINGS_ICU_FULL_DATA_DIGEST");
/// HBC version for the full VM selected to compile binding bytecode.
pub const BYTECODE_VERSION: &str = env!("IBEX2_BINDINGS_BYTECODE_VERSION");
/// HBC version for the lean VM, or `None` when the selected install has no
/// lean archive. `build.rs` asserts equality with [`BYTECODE_VERSION`] when
/// this value is present.
pub const LEAN_BYTECODE_VERSION: Option<&str> = option_env!("IBEX2_BINDINGS_LEAN_BYTECODE_VERSION");
pub const SQLITE_SOURCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/sqlite.js");
pub const TYPESCRIPT: &str = include_str!("bindings/storage.d.ts");

/// Windows: what this process observes of the operating system's ICU, which
/// backs the `INTL` group there. `Err` names why `INTL` is unavailable.
#[cfg(all(windows, feature = "intl"))]
pub use crate::stdlib::intl_os::{os_icu, OsIcu};

/// Named projections of the Rust standard library into a JavaScript runtime.
///
/// Cargo features decide what code is linked; this set independently decides
/// what one caller-owned runtime receives. Dependencies are checked rather
/// than silently added, so the installed surface is exactly the surface the
/// caller requested.
///
/// @ref LLP 0057.000#51-included-gated-or-a-crate — D6 chooses linked code and installed globals separately
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Groups(u16);

impl Groups {
    pub const PURE: Self = Self(1 << 0);
    pub const CONSOLE: Self = Self(1 << 1);
    pub const TIMERS: Self = Self(1 << 2);
    pub const ABORT: Self = Self(1 << 3);
    pub const CRYPTO: Self = Self(1 << 4);
    pub const FETCH: Self = Self(1 << 5);
    pub const STORAGE: Self = Self(1 << 6);
    pub const ENV: Self = Self(1 << 7);
    pub const SECRETS: Self = Self(1 << 8);
    pub const KV: Self = Self(1 << 9);
    pub const INTL: Self = Self(1 << 10);
    pub const EVENTS: Self = Self(1 << 11);
    pub const BLOB: Self = Self(1 << 12);
    /// Application WebSockets. The secure runtime injects a grant-bound
    /// constructor per module; caller-owned runtimes receive an installer-
    /// endowed global.
    pub const WEBSOCKET: Self = Self(1 << 13);

    const PORTABLE_ALL: Self = Self(
        Self::PURE.0
            | Self::CONSOLE.0
            | Self::TIMERS.0
            | Self::ABORT.0
            | Self::CRYPTO.0
            | Self::FETCH.0
            | Self::STORAGE.0
            | Self::ENV.0
            | Self::SECRETS.0
            | Self::KV.0
            | Self::EVENTS.0
            | Self::BLOB.0
            | Self::WEBSOCKET.0,
    );

    /// Every group linked into this build. Linux and Windows Intl exist only
    /// when their over-budget Cargo feature is selected.
    #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
    pub const ALL: Self = Self(Self::PORTABLE_ALL.0 | Self::INTL.0);
    /// Every group linked into this build.
    #[cfg(not(all(any(target_os = "linux", windows), feature = "intl")))]
    pub const ALL: Self = Self::PORTABLE_ALL;

    /// The ordinary runtime profile. BLOB and WEBSOCKET stay within LLP
    /// 0057.000 D5's 150 KB / 150 µs budget and are installed by default.
    #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
    pub const DEFAULT: Self = Self(Self::PORTABLE_ALL.0 | Self::INTL.0);
    /// The ordinary runtime profile. Intl is opt-in under LLP 0057.000 D5/D6.
    #[cfg(not(all(any(target_os = "linux", windows), feature = "intl")))]
    pub const DEFAULT: Self = Self::PORTABLE_ALL;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Combine install groups in a const context.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Remove an install group without changing any of the groups that remain.
    /// The resulting set is still checked by [`Self::validate`] at install.
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Refuse a selection that omits something its installed JavaScript uses.
    pub fn validate(self) -> Result<(), GroupError> {
        let requirements = [
            (Self::TIMERS, Self::CONSOLE),
            (Self::ABORT, Self::PURE),
            (Self::CRYPTO, Self::PURE),
            (Self::FETCH, Self(Self::PURE.0 | Self::ABORT.0)),
            (Self::EVENTS, Self::PURE),
            (Self::WEBSOCKET, Self(Self::PURE.0 | Self::EVENTS.0)),
            (Self::BLOB, Self::PURE),
        ];
        for (group, required) in requirements {
            if self.contains(group) && !self.contains(required) {
                return Err(GroupError {
                    group,
                    missing: Self(required.0 & !self.0),
                });
            }
        }
        #[cfg(not(all(any(target_os = "linux", windows), feature = "intl")))]
        if self.contains(Self::INTL) {
            return Err(GroupError {
                group: Self::INTL,
                missing: Self::INTL,
            });
        }
        // @ref LLP 0057.000#511-windows-intl-uses-the-os-icu — linked is not
        // enough on Windows: the OS must provide the ICU the shims call.
        #[cfg(all(windows, feature = "intl"))]
        if self.contains(Self::INTL) && crate::stdlib::intl_os::available().is_err() {
            return Err(GroupError {
                group: Self::INTL,
                missing: Self::INTL,
            });
        }
        Ok(())
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        const NAMES: [(Groups, &str); 14] = [
            (Groups::PURE, "PURE"),
            (Groups::CONSOLE, "CONSOLE"),
            (Groups::TIMERS, "TIMERS"),
            (Groups::ABORT, "ABORT"),
            (Groups::CRYPTO, "CRYPTO"),
            (Groups::FETCH, "FETCH"),
            (Groups::STORAGE, "STORAGE"),
            (Groups::ENV, "ENV"),
            (Groups::SECRETS, "SECRETS"),
            (Groups::KV, "KV"),
            (Groups::INTL, "INTL"),
            (Groups::EVENTS, "EVENTS"),
            (Groups::BLOB, "BLOB"),
            (Groups::WEBSOCKET, "WEBSOCKET"),
        ];
        NAMES
            .into_iter()
            .filter_map(move |(group, name)| self.contains(group).then_some(name))
    }
}

impl fmt::Debug for Groups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.names()).finish()
    }
}

impl Default for Groups {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ops::BitOr for Groups {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl ops::BitOrAssign for Groups {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl ops::BitAnd for Groups {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

/// A refused group selection and the dependency bits it omitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupError {
    pub group: Groups,
    pub missing: Groups,
}

impl fmt::Display for GroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.group == Groups::INTL && self.missing == Groups::INTL {
            #[cfg(all(windows, feature = "intl"))]
            return write!(
                f,
                "binding group INTL is unavailable; this Windows lacks the OS ICU it uses: {}",
                crate::stdlib::intl_os::available()
                    .err()
                    .unwrap_or("the ICU probe failed")
            );
            #[cfg(not(all(windows, feature = "intl")))]
            return write!(
                f,
                "binding group INTL is unavailable; enable ibex2's `intl` Cargo feature on Linux or Windows"
            );
        }
        write!(
            f,
            "binding group {:?} requires missing group(s) {:?}",
            self.group, self.missing
        )
    }
}

impl std::error::Error for GroupError {}

/// One engine-specific bytecode input, in the order `Adapter::install` takes
/// the compiled results. The embedder compiles each path with the compiler
/// belonging to the engine that owns its JSI runtime.
pub type Script = (&'static str, &'static str);

/// One binding script compiled by the `hermesc` selected by
/// `hermes-lean-sys`. The runtime and external embedders can feed these bytes
/// to the engine from the same resolution without linking a VM through this
/// crate.
pub struct CompiledBinding {
    pub name: &'static str,
    pub bytes: &'static [u8],
}

/// JavaScript shapes needed by `groups`, in deterministic installation order.
/// Runtime-only files (`esm.js` and `testharness.js`) and the separate
/// post-install hardening step are deliberately absent.
pub fn scripts(groups: Groups) -> Result<Vec<Script>, GroupError> {
    groups.validate()?;
    let mut result = Vec::new();
    let path = |name| match name {
        "headers" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/headers.js"),
        "timers" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/timers.js"),
        "url" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/url.js"),
        "domexception" => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/domexception.js")
        }
        "crypto" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/crypto.js"),
        "abort" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/abort.js"),
        "events" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/events.js"),
        "websocket" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/websocket.js"),
        "structured_clone" => {
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/bindings/structured_clone.js"
            )
        }
        "fetch" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/fetch.js"),
        "blob" => concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/blob.js"),
        "sqlite" => SQLITE_SOURCE,
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        "intl_number_format" => {
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/bindings/intl_number_format.js"
            )
        }
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        "intl_case" => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/intl_case.js")
        }
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        "intl_datetime" => {
            concat!(env!("CARGO_MANIFEST_DIR"), "/src/bindings/intl_datetime.js")
        }
        _ => unreachable!("known binding script"),
    };
    let mut push = |name| result.push((name, path(name)));

    // Preserve the shipping runtime's existing bytecode evaluation order.
    if groups.contains(Groups::PURE) {
        push("headers");
    }
    if groups.contains(Groups::TIMERS) {
        push("timers");
    }
    if groups.contains(Groups::PURE) {
        push("url");
        push("domexception");
    }
    if groups.contains(Groups::CRYPTO) {
        push("crypto");
    }
    if groups.contains(Groups::EVENTS) {
        push("events");
    }
    if groups.contains(Groups::ABORT) {
        push("abort");
    }
    if groups.contains(Groups::BLOB) {
        push("blob");
    }
    if groups.contains(Groups::WEBSOCKET) {
        push("websocket");
    }
    #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
    if groups.contains(Groups::INTL) {
        push("intl_number_format");
        push("intl_case");
        push("intl_datetime");
    }
    if groups.contains(Groups::FETCH) {
        push("fetch");
    }
    if groups.contains(Groups::STORAGE) {
        push("sqlite");
    }
    // The platform factories above capture the private identity-brand writer.
    // structuredClone consumes its reader and removes both bootstrap helpers,
    // so it must be the final binding whenever PURE supplies it.
    if groups.contains(Groups::PURE) {
        push("structured_clone");
    }
    Ok(result)
}

/// Precompiled form of [`scripts`], in the identical checked order.
pub fn compiled_scripts(groups: Groups) -> Result<Vec<CompiledBinding>, GroupError> {
    scripts(groups).map(|scripts| {
        scripts
            .into_iter()
            .map(|(name, _)| CompiledBinding {
                name,
                bytes: compiled_script(name),
            })
            .collect()
    })
}

fn compiled_script(name: &str) -> &'static [u8] {
    match name {
        "headers" => include_bytes!(concat!(env!("OUT_DIR"), "/headers.hbc")),
        "timers" => include_bytes!(concat!(env!("OUT_DIR"), "/timers.hbc")),
        "url" => include_bytes!(concat!(env!("OUT_DIR"), "/url.hbc")),
        "domexception" => include_bytes!(concat!(env!("OUT_DIR"), "/domexception.hbc")),
        "crypto" => include_bytes!(concat!(env!("OUT_DIR"), "/crypto.hbc")),
        "events" => include_bytes!(concat!(env!("OUT_DIR"), "/events.hbc")),
        "abort" => include_bytes!(concat!(env!("OUT_DIR"), "/abort.hbc")),
        "websocket" => include_bytes!(concat!(env!("OUT_DIR"), "/websocket.hbc")),
        "blob" => include_bytes!(concat!(env!("OUT_DIR"), "/blob.hbc")),
        "structured_clone" => {
            include_bytes!(concat!(env!("OUT_DIR"), "/structured_clone.hbc"))
        }
        "fetch" => include_bytes!(concat!(env!("OUT_DIR"), "/fetch.hbc")),
        "sqlite" => include_bytes!(concat!(env!("OUT_DIR"), "/sqlite.hbc")),
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        "intl_number_format" => {
            include_bytes!(concat!(env!("OUT_DIR"), "/intl_number_format.hbc"))
        }
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        "intl_case" => include_bytes!(concat!(env!("OUT_DIR"), "/intl_case.hbc")),
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        "intl_datetime" => {
            include_bytes!(concat!(env!("OUT_DIR"), "/intl_datetime.hbc"))
        }
        _ => unreachable!("scripts returned an unknown binding"),
    }
}

/// Rust resources borrowed by one JSI adapter. Create after first pixel,
/// configure before installation, and detach the adapter before dropping this.
/// Each context has a separate completion queue and database handle space.
pub struct Context {
    endowment: Arc<InstallEndowment>,
    _owner: crate::task::OwnerLease,
}

const BINDINGS_MAGIC: u64 = 0x4942_4558_3242_4e44;
const ADOPTED_BINDINGS_MAGIC: u64 = 0x4942_4558_3241_4450;

/// Opaque handle type passed to the JSI adapter at installation.
#[repr(C)]
pub struct Ibex2Bindings {
    _private: [u8; 0],
}

/// The Rust allocation behind an [`Ibex2Bindings`] pointer. The tag is checked
/// only after the address has been found in the live handle registry, so a
/// pointer of some other type is refused without first reinterpreting its
/// storage as this structure.
struct InstallEndowment {
    magic: u64,
    state: Arc<RuntimeState>,
    grants: Arc<GrantSet>,
    bindings: host::Bindings,
}

fn live_bindings() -> &'static Mutex<HashSet<usize>> {
    static LIVE: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
    LIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

fn register_bindings(bindings: &Arc<InstallEndowment>) {
    let address = Arc::as_ptr(bindings) as usize;
    let inserted = live_bindings()
        .lock()
        .expect("bindings registry poisoned")
        .insert(address);
    assert!(inserted, "new bindings handle reused a live address");
}

fn valid_bindings(bindings: *const Ibex2Bindings) -> Option<&'static InstallEndowment> {
    if bindings.is_null()
        || !live_bindings()
            .lock()
            .expect("bindings registry poisoned")
            .contains(&(bindings as usize))
    {
        return None;
    }
    // SAFETY: membership is added only for an allocated InstallEndowment and is
    // removed by its Drop. The ABI requires the producing Context to remain
    // live for the call, just as state_ptr() does.
    let bindings = unsafe { &*bindings.cast::<InstallEndowment>() };
    matches!(bindings.magic, BINDINGS_MAGIC | ADOPTED_BINDINGS_MAGIC).then_some(bindings)
}

impl Drop for InstallEndowment {
    fn drop(&mut self) {
        let removed = live_bindings()
            .lock()
            .expect("bindings registry poisoned")
            .remove(&(self as *const Self as usize));
        debug_assert!(removed, "bindings handle was not registered");
        self.magic = 0;
    }
}

impl Context {
    /// Convenience for the platform-default host. Consumers that select a
    /// transport or store use [`Context::from_bindings`] instead.
    pub fn new(grants: GrantSet) -> Self {
        // An owning runtime adopts this endowment after constructing its own
        // state. Preserve the default selection without eagerly constructing
        // a platform transport that adoption would immediately supersede.
        // @ref LLP 0068#3-no-engine-in-the-process — Context endowments are adopted without sharing runtime state
        let bindings =
            host::Host::with_transport(Box::new(crate::transport::LazyDefaultTransport::new()))
                .endow(grants);
        Self::from_bindings(&bindings)
    }

    /// Build the runtime endowment from the exact bindings a host returned.
    /// The transport, stores, mounts, provider, and grants are retained
    /// together for the lifetime of the runtime state.
    // @ref LLP 0057.000#50-three-doors-one-implementation — the install door consumes Host::endow's Bindings
    pub fn from_bindings(bindings: &host::Bindings) -> Self {
        let state = Arc::new(RuntimeState::from_bindings(bindings));
        let endowment = Arc::new(InstallEndowment {
            magic: BINDINGS_MAGIC,
            state: Arc::clone(&state),
            grants: bindings.grants(),
            bindings: bindings.clone(),
        });
        register_bindings(&endowment);
        Self {
            endowment,
            _owner: crate::task::OwnerLease::new(state),
        }
    }

    pub fn set_app_directories(
        &self,
        directories: crate::stdlib::app_fs::AppDirectories,
    ) -> Result<(), crate::boundary::HostError> {
        self.endowment.state.set_app_directories(directories)
    }

    /// The table `doc:` paths resolve through (Exact patch 5).
    pub fn set_documents(
        &self,
        documents: Arc<crate::stdlib::fs::Documents>,
    ) -> Result<(), crate::boundary::HostError> {
        self.endowment.state.set_documents(documents)
    }

    pub fn set_sqlite_provider(
        &self,
        provider: Arc<dyn crate::stdlib::sqlite::Provider>,
    ) -> Result<(), crate::boundary::HostError> {
        self.endowment.state.set_sqlite_provider(provider)
    }

    /// The codec `fs.compressImage` runs (Exact patch 9). Without one the
    /// operation refuses as unsupported.
    pub fn set_image_codec(
        &self,
        codec: Arc<crate::stdlib::fs::ImageCodec>,
    ) -> Result<(), crate::boundary::HostError> {
        self.endowment.state.set_image_codec(codec)
    }

    /// Call when giving up on a storage wait (Exact patch 9): no
    /// `fs.compressImage` still in flight will write after this returns
    /// `Abandoned`; `Written` means one finished its write and its
    /// completion is coming, so keep waiting for it.
    pub fn abandon_image_work(&self) -> crate::stdlib::fs::Abandoned {
        self.endowment.state.abandon_image_work()
    }

    /// Worker-safe, edge-triggered notification that schedules the embedder's
    /// loop. Admissions coalesce and at most one callback runs at a time; a
    /// publisher that finds one running records another edge and returns.
    ///
    /// The callback must return without entering JS or waiting for owner-thread
    /// work. Releasing the last owner from the callback is safe. Releasing it
    /// on another thread while holding a lock that the in-flight callback needs
    /// is unsupported ordinary lock ordering, because shutdown waits for the
    /// callback. Install before starting work.
    pub fn set_wake(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.endowment.state.queue.set_wake(Some(wake));
    }

    /// A blocking executor can wait instead of installing a wake callback.
    pub fn wait(&self, timeout: Duration) -> bool {
        self.endowment.state.queue.wait(timeout)
    }

    pub fn is_idle(&self) -> bool {
        self.endowment.state.is_idle()
    }

    /// Borrowed Arc-backed pointer for the JSI adapter. The context must
    /// outlive the adapter's detach; the adapter never releases this pointer.
    pub fn state_ptr(&self) -> *const c_void {
        Arc::as_ptr(&self.endowment.state).cast()
    }

    /// Borrowed Arc-backed grants. Adapter-created functions retain their
    /// own references, so copied capabilities keep the installer's authority.
    pub fn grants_ptr(&self) -> *const c_void {
        Arc::as_ptr(&self.endowment.grants).cast()
    }

    /// Opaque install input consumed by the engine-independent adapter.
    pub fn bindings_ptr(&self) -> *const Ibex2Bindings {
        Arc::as_ptr(&self.endowment).cast()
    }
}

/// Return the runtime state carried by an install endowment.
///
/// # Safety
/// `bindings` must be a live pointer returned by [`Context::bindings_ptr`].
#[no_mangle]
pub unsafe extern "C" fn ibex2_bindings_state(bindings: *const Ibex2Bindings) -> *const c_void {
    valid_bindings(bindings).map_or(std::ptr::null(), |endowment| {
        Arc::as_ptr(&endowment.state).cast()
    })
}

/// Return the grant set carried by an install endowment.
///
/// # Safety
/// `bindings` must be a live pointer returned by [`Context::bindings_ptr`].
#[no_mangle]
pub unsafe extern "C" fn ibex2_bindings_grants(bindings: *const Ibex2Bindings) -> *const c_void {
    valid_bindings(bindings).map_or(std::ptr::null(), |endowment| {
        Arc::as_ptr(&endowment.grants).cast()
    })
}

/// Copy a live host endowment into an owning Hermes runtime's existing state
/// and return a handle whose state identity matches that runtime.
///
/// # Safety
/// `bindings` must be a live pointer returned by [`Context::bindings_ptr`], and
/// `state` must be a live owner state created by this crate.
#[no_mangle]
pub unsafe extern "C" fn ibex2_bindings_adopt(
    bindings: *const Ibex2Bindings,
    state: *const RuntimeState,
) -> *const Ibex2Bindings {
    let Some(source) = valid_bindings(bindings) else {
        return std::ptr::null();
    };
    let Some(state) = crate::task::clone_queue(state) else {
        return std::ptr::null();
    };
    let snapshot = source.state.bindings_snapshot(&source.bindings);
    if state.adopt_bindings(&snapshot).is_err() {
        return std::ptr::null();
    }
    let adopted = Arc::new(InstallEndowment {
        magic: ADOPTED_BINDINGS_MAGIC,
        state,
        grants: Arc::clone(&source.grants),
        bindings: snapshot,
    });
    register_bindings(&adopted);
    Arc::into_raw(adopted).cast()
}

/// Release a handle returned by [`ibex2_bindings_adopt`]. Invalid handles are
/// refused without dereferencing them.
///
/// # Safety
/// A valid adopted handle must be released exactly once.
#[no_mangle]
pub unsafe extern "C" fn ibex2_bindings_destroy(bindings: *const Ibex2Bindings) {
    if valid_bindings(bindings).is_some_and(|bindings| bindings.magic == ADOPTED_BINDINGS_MAGIC) {
        drop(Arc::from_raw(bindings.cast::<InstallEndowment>()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_group_dependencies_are_refused() {
        let error = Groups::FETCH.validate().unwrap_err();
        assert_eq!(error.group, Groups::FETCH);
        assert_eq!(error.missing, Groups::PURE | Groups::ABORT);

        let error = (Groups::PURE | Groups::FETCH).validate().unwrap_err();
        assert_eq!(error.group, Groups::FETCH);
        assert_eq!(error.missing, Groups::ABORT);

        let error = Groups::WEBSOCKET.validate().unwrap_err();
        assert_eq!(error.group, Groups::WEBSOCKET);
        assert_eq!(error.missing, Groups::PURE | Groups::EVENTS);

        let error = Groups::BLOB.validate().unwrap_err();
        assert_eq!(error.group, Groups::BLOB);
        assert_eq!(error.missing, Groups::PURE);
    }

    #[test]
    fn scripts_follow_the_shipping_install_order() {
        assert!(Groups::DEFAULT.contains(Groups::BLOB));
        assert!(Groups::ALL.contains(Groups::BLOB));
        assert!(Groups::DEFAULT.contains(Groups::WEBSOCKET));
        assert!(Groups::ALL.contains(Groups::WEBSOCKET));
        let names: Vec<_> = scripts(Groups::DEFAULT)
            .unwrap()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let mut expected = vec![
            "headers",
            "timers",
            "url",
            "domexception",
            "crypto",
            "events",
            "abort",
            "blob",
            "websocket",
        ];
        #[cfg(all(any(target_os = "linux", windows), feature = "intl"))]
        expected.extend(["intl_number_format", "intl_case", "intl_datetime"]);
        expected.extend(["fetch", "sqlite", "structured_clone"]);
        assert_eq!(names, expected);
    }

    #[test]
    fn intl_group_tracks_the_cargo_feature() {
        let available = cfg!(all(any(target_os = "linux", windows), feature = "intl"));
        assert_eq!(Groups::DEFAULT.contains(Groups::INTL), available);
        assert_eq!(Groups::ALL.contains(Groups::INTL), available);
        let result = Groups::INTL.validate();
        assert_eq!(result.is_ok(), available);
        if !available {
            assert_eq!(
                result.unwrap_err().to_string(),
                "binding group INTL is unavailable; enable ibex2's `intl` Cargo feature on Linux or Windows"
            );
        }
    }
}
