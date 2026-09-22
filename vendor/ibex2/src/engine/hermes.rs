//! The vanilla Hermes adapter.
//!
//! Built only with the `hermes` feature, against the platform's unpatched
//! vanilla Hermes install.
//! It uses stock JSI and nothing else — see `hermes_shim.cc`.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::time::{Duration, Instant};

// RuntimeState crosses as an opaque pointer that only Rust ever dereferences;
// the C++ side treats it as `const void *`. clippy's improper_ctypes fires on
// the type name rather than on the usage.
#[allow(improper_ctypes)]
extern "C" {
    fn ibex2_hermes_create(enable_eval: c_int) -> *mut c_void;
    fn ibex2_hermes_destroy(handle: *mut c_void);
    fn ibex2_hermes_eval(
        handle: *mut c_void,
        source: *const c_char,
        out: *mut *mut c_char,
    ) -> c_int;
    fn ibex2_hermes_install_probe(
        handle: *mut c_void,
        name: *const c_char,
        value: *const c_char,
    ) -> c_int;
    fn ibex2_hermes_install_throwing_probe(
        handle: *mut c_void,
        name: *const c_char,
        native: c_int,
    ) -> c_int;
    fn ibex2_hermes_free_string(value: *mut c_char);
    fn ibex2_hermes_install_stdlib(handle: *mut c_void) -> c_int;
    fn ibex2_hermes_pump(handle: *mut c_void, out_ran: *mut c_int) -> c_int;
    fn ibex2_hermes_collect_garbage(handle: *mut c_void) -> c_int;
    fn ibex2_hermes_drain_microtasks(handle: *mut c_void, out: *mut *mut c_char) -> c_int;
    fn ibex2_hermes_set_deadline(handle: *mut c_void, remaining_nanos: u64) -> c_int;
    fn ibex2_hermes_clear_deadline(handle: *mut c_void) -> c_int;
    fn ibex2_hermes_wait(handle: *mut c_void, timeout_ms: u64) -> c_int;
    fn ibex2_hermes_install_fetch(handle: *mut c_void, grants: *const c_void) -> c_int;
    fn ibex2_hermes_install_async_echo(handle: *mut c_void) -> c_int;
    fn ibex2_hermes_install_fetch_factory(
        handle: *mut c_void,
        bytes: *const u8,
        len: usize,
    ) -> c_int;
    fn ibex2_hermes_install_sqlite_factory(
        handle: *mut c_void,
        bytes: *const u8,
        len: usize,
    ) -> c_int;
    fn ibex2_hermes_accept_intl_intrinsics(handle: *mut c_void) -> c_int;
    fn ibex2_hermes_install_intl_datetime(
        handle: *mut c_void,
        bytes: *const u8,
        len: usize,
    ) -> c_int;
    fn ibex2_hermes_state(handle: *mut c_void) -> *const crate::task::RuntimeState;
    fn ibex2_hermes_eval_bytes(
        handle: *mut c_void,
        data: *const u8,
        len: usize,
        out: *mut *mut c_char,
    ) -> c_int;
    fn ibex2_hermes_run_entry(
        handle: *mut c_void,
        specifier: *const c_char,
        out_error: *mut *mut c_char,
    ) -> c_int;
    fn ibex2_grants_create(spec: *const c_char) -> *const c_void;
    fn ibex2_grants_destroy(grants: *const c_void);
}

/// A grant set, owned for as long as the bindings that carry it.
pub struct Grants(*const c_void);

impl Grants {
    /// Parse a grant spec. See `GrantSet::parse`.
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = CString::new(spec).ok()?;
        // SAFETY: the pointer is either null or an owned grant set.
        let raw = unsafe { ibex2_grants_create(spec.as_ptr()) };
        if raw.is_null() {
            return None;
        }
        Some(Self(raw))
    }
}

impl Drop for Grants {
    fn drop(&mut self) {
        // SAFETY: created here, released exactly once.
        unsafe { ibex2_grants_destroy(self.0) };
    }
}

/// Whether JavaScript may compile source of its own.
///
/// `Closed` is the Ibex 2 posture (LLP 0060 D4): with a Rust standard library
/// over an ahead-of-time bytecode graph, nothing on the boot path needs `eval`,
/// so it is refused at construction rather than latched off afterwards. `Open`
/// exists to make that difference testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicCode {
    Closed,
    Open,
}

/// A vanilla Hermes runtime.
pub struct Hermes {
    handle: *mut c_void,
    /// The armed deadline as this side handed it over. The engine holds the
    /// same point on its own clock and is what stops JavaScript; this copy
    /// is what the helpers that block *between* entrances cap their waits
    /// by, so that no wait outlives the deadline either.
    deadline: Option<Instant>,
}

/// Why an entrance returned no value.
///
/// The entrances are `eval`, `eval_bytes`, `drain_microtasks`, `pump`, and
/// `run_entry`: every way JavaScript runs on a runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsError {
    /// JavaScript threw, and this is what it threw, as text.
    Thrown(String),
    /// The deadline armed by [`Hermes::set_deadline`] passed: at the door, so
    /// nothing ran, or while JavaScript was running, so the engine stopped
    /// it. Reported by kind and never by the text of what was thrown — text
    /// is JavaScript's to forge. Nothing JavaScript did on the way to the
    /// deadline is reported, and its state is whatever the stop left.
    // @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — the one outcome a consumer may act on without reading text: classified by the clock, uncatchable in JavaScript
    Deadline,
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsError::Thrown(text) => f.write_str(text),
            JsError::Deadline => f.write_str("the deadline passed"),
        }
    }
}

impl std::error::Error for JsError {}

/// What one entrance reported, with the string the shim handed back. The
/// statuses are the shim's IBEX2_STATUS_* constants; anything else is a
/// programming error on this side of the boundary, not a JavaScript outcome.
fn checked(status: c_int, out: *mut c_char, entrance: &str) -> Result<String, JsError> {
    let text = take_c_string(out);
    match status {
        0 => Ok(text.unwrap_or_default()),
        1 => Err(JsError::Thrown(
            text.unwrap_or_else(|| "unknown error".into()),
        )),
        2 => Err(JsError::Deadline),
        other => panic!("{entrance} returned {other}"),
    }
}

impl Hermes {
    /// Configure stable app mounts before evaluating application modules.
    pub fn set_app_directories(
        &self,
        directories: crate::stdlib::app_fs::AppDirectories,
    ) -> Result<(), crate::boundary::HostError> {
        let state = unsafe { crate::task::borrow_state(ibex2_hermes_state(self.handle)) }
            .expect("live runtime state");
        state.set_app_directories(directories)
    }
    /// Install a separately linked SQLite provider; no database opens at startup.
    pub fn set_sqlite_provider(
        &self,
        provider: std::sync::Arc<dyn crate::stdlib::sqlite::Provider>,
    ) -> Result<(), crate::boundary::HostError> {
        let state = unsafe { crate::task::borrow_state(ibex2_hermes_state(self.handle)) }
            .expect("live runtime state");
        state.set_sqlite_provider(provider)
    }

    pub fn new(dynamic_code: DynamicCode) -> Option<Self> {
        let enable = match dynamic_code {
            DynamicCode::Open => 1,
            DynamicCode::Closed => 0,
        };
        // SAFETY: the shim returns either null or a pointer we own until
        // ibex2_hermes_destroy, which Drop is the only other caller of.
        let handle = unsafe { ibex2_hermes_create(enable) };
        if handle.is_null() {
            return None;
        }
        Some(Self {
            handle,
            deadline: None,
        })
    }

    /// Evaluate source as the **host**. This is not JavaScript's `eval`: it
    /// stays available with `DynamicCode::Closed`, which is what lets a closed
    /// runtime still run prepared code.
    pub fn eval(&mut self, source: &str) -> Result<String, JsError> {
        let source = CString::new(source).expect("source contains a NUL byte");
        let mut out: *mut c_char = std::ptr::null_mut();
        // SAFETY: `handle` is non-null for the lifetime of self; `out` receives
        // a malloc'd string we take ownership of and free below.
        let status = unsafe { ibex2_hermes_eval(self.handle, source.as_ptr(), &mut out) };
        checked(status, out, "ibex2_hermes_eval")
    }

    /// Arm this runtime's one deadline, replacing any already armed.
    ///
    /// From now until [`clear_deadline`](Self::clear_deadline), every
    /// entrance — `eval`, `eval_bytes`, `drain_microtasks`, `pump`,
    /// `run_entry` — is refused with [`JsError::Deadline`] once the deadline
    /// has passed, and stopped with it if it passes while JavaScript runs.
    /// What each entrance is given is the time left of this same deadline,
    /// never a fresh budget per call or per drain iteration.
    ///
    /// The stop is the engine's own: the pinned Hermes's time-limit monitor
    /// raises an uncatchable error at the next async-break check, which
    /// evaluated source carries at every loop back-edge and every return.
    /// Bytecode carries only the checks it was compiled with, so a
    /// check-free program runs to its end and is reported at exit — a
    /// deadline is not, by itself, a bound on precompiled code, and an
    /// adapter running bytecode it does not trust with time needs a bound
    /// of its own.
    ///
    /// The deadline is a point on the monotonic clock. It crosses to the
    /// engine as the time left at this call, which the engine adds to its
    /// own monotonic clock — the same clock the `Instant` reads on this
    /// platform is not guaranteed, so the deadline cannot be handed over as
    /// an absolute value; the microseconds this costs are in the caller's
    /// favour.
    ///
    /// Owner thread only, like every entrance (LLP 0058.000.000 §2,
    /// invariant 8): the deadline is read at each entrance without
    /// synchronization, so arming or clearing it from another thread races
    /// an entrance in flight.
    // @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — one deadline per runtime; every entrance is given what is left of it, never a fresh budget
    pub fn set_deadline(&mut self, deadline: Instant) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let nanos = u64::try_from(remaining.as_nanos()).unwrap_or(u64::MAX);
        // SAFETY: `handle` is non-null for the lifetime of self.
        let status = unsafe { ibex2_hermes_set_deadline(self.handle, nanos) };
        assert_eq!(status, 0, "ibex2_hermes_set_deadline returned {status}");
        self.deadline = Some(deadline);
    }

    /// Disarm the deadline. A runtime whose deadline fired is usable again
    /// after this; its JavaScript state is whatever the stop left, which is
    /// the consumer's to judge (Snapback 2 discards the runtime). Harmless
    /// when nothing is armed. Owner thread only, as `set_deadline` says.
    // @ref LLP 0058.000.000#8-tasks-microtasks-timers-and-callbacks — a fired deadline leaves the runtime reusable once cleared; its pending break was flushed at the entrance that ended past it
    pub fn clear_deadline(&mut self) {
        // SAFETY: `handle` is non-null for the lifetime of self.
        let status = unsafe { ibex2_hermes_clear_deadline(self.handle) };
        assert_eq!(status, 0, "ibex2_hermes_clear_deadline returned {status}");
        self.deadline = None;
    }

    /// Cap a wait between entrances by the armed deadline. A helper that
    /// blocks must not outlive the deadline any more than an entrance may:
    /// the wait is cut to the time left, rounded up so it ends at or after
    /// the deadline and the next entrance is what reports it, and is zero
    /// once the deadline has passed.
    fn capped_by_deadline(&self, millis: u64) -> u64 {
        let Some(deadline) = self.deadline else {
            return millis;
        };
        let left = deadline.saturating_duration_since(Instant::now());
        let whole = u64::try_from(left.as_millis()).unwrap_or(u64::MAX);
        let ceil = whole.saturating_add(u64::from(left.subsec_nanos() % 1_000_000 != 0));
        millis.min(ceil)
    }

    /// Evaluate the JavaScript binding preludes that SHIP.
    ///
    /// Source, not bytecode, for now — a production boot compiles these with
    /// hermesc so nothing is parsed at launch (LLP 0058 §1.1). They are small
    /// enough that it does not yet matter, and this is the seam where that
    /// changes.
    ///
    /// The test harness is deliberately not here. It used to be, and the
    /// LLP 0062 R5 global-name assertion caught it the first time the binary
    /// ran: fourteen assertion helpers were being published to every program's
    /// global object. That is the whole argument for R5 — R1 is a property of a
    /// list, and a list nothing checks drifts.
    pub fn install_bindings(&mut self) -> Result<(), JsError> {
        // Bytecode, compiled by build.rs from src/bindings/*.js with the
        // engine's own hermesc: the runtime parses nothing of its own at
        // start, which is the rule application code already lives under.
        for binding in [
            &include_bytes!(concat!(env!("OUT_DIR"), "/esm.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/headers.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/timers.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/url.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/domexception.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/crypto.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/abort.hbc"))[..],
        ] {
            self.eval_bytes(binding)?;
        }
        #[cfg(target_os = "linux")]
        for binding in [
            &include_bytes!(concat!(env!("OUT_DIR"), "/intl_number_format.hbc"))[..],
            &include_bytes!(concat!(env!("OUT_DIR"), "/intl_case.hbc"))[..],
        ] {
            self.eval_bytes(binding)?;
        }
        #[cfg(target_os = "linux")]
        {
            let datetime = include_bytes!(concat!(env!("OUT_DIR"), "/intl_datetime.hbc"));
            let status = unsafe {
                ibex2_hermes_install_intl_datetime(self.handle, datetime.as_ptr(), datetime.len())
            };
            if status != 0 {
                return Err(JsError::Thrown(
                    "the DateTimeFormat binding did not evaluate to its factory".into(),
                ));
            }
        }
        // These precompiled, repository-owned bindings intentionally replace
        // four locale methods captured by SQLite's integrity witness. Update
        // only those expected identities; the rest of the construction-time
        // snapshot continues to detect pre-hardening application changes.
        let status = unsafe { ibex2_hermes_accept_intl_intrinsics(self.handle) };
        if status != 0 {
            return Err(JsError::Thrown(
                "the trusted Intl bindings did not preserve intrinsic integrity".into(),
            ));
        }
        // fetch.js is different: its value is the fetch factory, and it must
        // not be a global (see the file). The shim keeps it.
        let fetch = include_bytes!(concat!(env!("OUT_DIR"), "/fetch.hbc"));
        // SAFETY: `handle` is non-null for the lifetime of self; the bytes
        // outlive the call.
        let status =
            unsafe { ibex2_hermes_install_fetch_factory(self.handle, fetch.as_ptr(), fetch.len()) };
        if status != 0 {
            return Err(JsError::Thrown(
                "the fetch binding did not evaluate to its factory".into(),
            ));
        }
        let sqlite = include_bytes!(concat!(env!("OUT_DIR"), "/sqlite.hbc"));
        // SAFETY: the runtime and bytecode remain alive throughout the call.
        let status = unsafe {
            ibex2_hermes_install_sqlite_factory(self.handle, sqlite.as_ptr(), sqlite.len())
        };
        if status != 0 {
            return Err(JsError::Thrown(
                "the SQLite binding did not evaluate to its factory".into(),
            ));
        }
        Ok(())
    }

    /// The LLP 0067 R4 freeze, from bytecode: after the standard library and
    /// bindings are installed and before any module code runs.
    pub fn harden(&mut self) -> Result<(), JsError> {
        self.eval_bytes(include_bytes!(concat!(env!("OUT_DIR"), "/harden.hbc")))
            .map(|_| ())
    }

    /// The WPT test harness. Tests only; never installed by the binary.
    pub fn install_test_harness(&mut self) -> Result<(), JsError> {
        self.eval(include_str!("../bindings/testharness.js"))?;
        Ok(())
    }

    /// Install the pure standard-library tier: `console`, `btoa`/`atob`, and
    /// the text/URL host calls the binding layer will wrap.
    pub fn install_stdlib(&mut self) -> bool {
        // SAFETY: `handle` is non-null for the lifetime of self.
        unsafe { ibex2_hermes_install_stdlib(self.handle) == 0 }
    }

    /// Ask the engine to collect unreachable objects and their native resources.
    /// Normal execution uses the engine's own collection schedule.
    pub fn collect_garbage(&mut self) -> bool {
        // SAFETY: the handle is live and this is the owning JavaScript thread.
        unsafe { ibex2_hermes_collect_garbage(self.handle) == 0 }
    }

    /// Drain the console records this runtime's thread has queued.
    pub fn drain_console(&self) -> Vec<crate::stdlib::console::Record> {
        crate::boundary_abi::drain_console()
    }

    /// Run at most one host task, with a microtask checkpoint either side.
    ///
    /// One task per cycle, per LLP 0058.000.000 §8 — so this returns 0 or 1 and
    /// a caller wanting to reach quiescence loops. A callback or job that
    /// throws is a console error, never a return: one bad task does not stop
    /// the tasks behind it. The deadline does, and is the only `Err`.
    pub fn pump(&mut self) -> Result<i32, JsError> {
        let mut ran: c_int = 0;
        // SAFETY: `handle` is non-null for the lifetime of self, and this is
        // the JavaScript thread.
        let status = unsafe { ibex2_hermes_pump(self.handle, &mut ran) };
        match status {
            0 => Ok(ran),
            2 => Err(JsError::Deadline),
            other => panic!("ibex2_hermes_pump returned {other}"),
        }
    }

    /// Pump until `expected` host tasks have run, the armed deadline passes,
    /// or thirty seconds go by.
    ///
    /// Blocks on the completion signal rather than spinning. The earlier
    /// version looped a fixed number of times yielding, which raced through
    /// 10,000 iterations in microseconds and gave up long before a real network
    /// request could finish — fine against a loopback socket, useless against
    /// NSURLSession.
    ///
    /// Under an armed deadline no wait outlives it: the deadline ends this
    /// loop as it ends any entrance, and what was delivered by then is the
    /// answer. The count does not say which of the three ended the loop; a
    /// caller that must tell the deadline from the rest checks its own clock
    /// against the deadline it armed.
    pub fn pump_until(&mut self, expected: i32) -> i32 {
        let give_up = Instant::now() + Duration::from_secs(30);
        let mut delivered = 0;
        loop {
            match self.pump() {
                Ok(ran) => delivered += ran.max(0),
                // The runtime's own deadline: what was delivered is the answer.
                Err(_) => return delivered,
            }
            if delivered >= expected || Instant::now() >= give_up {
                return delivered;
            }
            let wait = self.capped_by_deadline(100);
            // SAFETY: `handle` is non-null for the lifetime of self.
            unsafe { ibex2_hermes_wait(self.handle, wait) };
        }
    }

    /// Evaluate a buffer, which may be Hermes bytecode.
    ///
    /// Hermes detects the HBC magic, so this is the same entry point as source
    /// with a different payload.
    pub fn eval_bytes(&mut self, bytes: &[u8]) -> Result<String, JsError> {
        let mut out: *mut c_char = std::ptr::null_mut();
        // SAFETY: the slice outlives the call; `out` receives an owned string.
        let status =
            unsafe { ibex2_hermes_eval_bytes(self.handle, bytes.as_ptr(), bytes.len(), &mut out) };
        checked(status, out, "ibex2_hermes_eval_bytes")
    }

    /// Point the loader at a project root and its grant manifest.
    ///
    /// Without a compiler the loader falls back to source, which
    /// `rules/RULES.md` forbids for anything shippable; see `set_loader_with`.
    pub fn set_loader(
        &mut self,
        root: crate::loader::Root,
        grants: crate::loader::ModuleGrants,
    ) -> Result<(), String> {
        self.set_loader_with(root, grants, None, false)
    }

    /// Point the loader at a root, with ahead-of-time compilation.
    ///
    /// Fails when the manifest cannot be bound to the root — a package section
    /// naming something not installed — because a manifest that silently
    /// grants nothing is worse than one refused.
    pub fn set_loader_with(
        &mut self,
        root: crate::loader::Root,
        mut grants: crate::loader::ModuleGrants,
        compiler: Option<crate::bytecode::Compiler>,
        precompiled_only: bool,
    ) -> Result<(), String> {
        grants.bind(root.path())?;
        let cache = root.join(".ibex2/cache");
        let mut manifest = compiler
            .as_ref()
            .and_then(|_| crate::bytecode::Manifest::read(&cache));
        // Artifacts are bytecode for one engine. A manifest built by another
        // binary would still resolve by key and hand this engine bytecode it
        // may not accept — so the manifest carries the engine it was built
        // for, and this is where it is checked, before any module loads.
        if let Some(found) = &manifest {
            let linked = crate::bytecode::Compiler::linked_engine();
            let stale = match found.engine() {
                Some(built_for) => built_for != linked,
                None => true,
            };
            if stale && precompiled_only {
                return Err(format!(
                    "the precompiled artifacts under {} were built for another engine\n  \
                     built for: {}\n  this runtime links: {linked}\n\
                     Run `ibex2 build` again with this binary.",
                    cache.display(),
                    found
                        .engine()
                        .unwrap_or("(a build that predates engine binding)")
                ));
            }
            if stale {
                manifest = None;
            }
        }
        // The bundle is read only behind an accepted manifest: its keys are the
        // manifest's, and a manifest refused above takes its bundle with it.
        let bundle = manifest
            .as_ref()
            .and_then(|_| crate::bytecode::Bundle::read(&cache));
        // SAFETY: `handle` is non-null for the lifetime of self.
        let state = unsafe { ibex2_hermes_state(self.handle) };
        // SAFETY: the state pointer belongs to this runtime and outlives the call.
        if let Some(state) = unsafe { crate::task::borrow_state(state) } {
            state.set_loader(crate::task::LoaderConfig {
                root,
                grants,
                compiler,
                precompiled_only,
                manifest,
                cache: Default::default(),
                bundle,
            });
        }
        Ok(())
    }

    /// Load and run an entry module. Its dependencies load through `require`.
    pub fn run_entry(&mut self, specifier: &str) -> Result<(), JsError> {
        let specifier = CString::new(specifier).expect("specifier contains a NUL");
        let mut error: *mut c_char = std::ptr::null_mut();
        // SAFETY: `handle` is non-null; `error` receives a Rust-owned string.
        let status = unsafe { ibex2_hermes_run_entry(self.handle, specifier.as_ptr(), &mut error) };
        let message = take_c_string(error);
        match status {
            0 => Ok(()),
            1 => Err(JsError::Thrown(
                message.unwrap_or_else(|| "module failed".into()),
            )),
            2 => Err(JsError::Deadline),
            other => panic!("ibex2_hermes_run_entry returned {other}"),
        }
    }

    /// Run the loop until nothing is pending: no completions, no timers —
    /// or until `budget` or the armed deadline passes.
    ///
    /// The embedder's turn, and the thing that makes a program with a
    /// `setTimeout` in it terminate at the right moment rather than early.
    ///
    /// Under an armed deadline no wait outlives it: the deadline ends this
    /// loop as it ends any entrance. Returning says nothing about which of
    /// the three ended it; a caller that must tell the deadline from
    /// quiescence checks its own clock against the deadline it armed.
    pub fn run_to_quiescence(&mut self, budget: Duration) {
        let give_up = Instant::now() + budget;
        loop {
            // The runtime's own deadline, if one is armed, ends the loop too.
            if self.pump().is_err() {
                return;
            }
            if Instant::now() >= give_up {
                return;
            }
            // SAFETY: `handle` is non-null for the lifetime of self.
            let state = unsafe { ibex2_hermes_state(self.handle) };
            let idle = unsafe { crate::task::borrow_state(state) }
                .map(|s| s.is_idle())
                .unwrap_or(true);
            if idle {
                return;
            }
            // Wait for the next thing that can make work: a completion, which
            // signals the Condvar, or the next timer's deadline. Exactly what
            // the Condvar in `CompletionQueue` is for — this loop used to sleep
            // 1 ms and poll, which put up to a millisecond of latency on every
            // async round trip (issues/20260829-async-host-op-spawns-a-thread.md).
            let until_timer = unsafe { crate::task::borrow_state(state) }
                .and_then(|s| s.millis_until_next_timer())
                .map(|ms| ms.ceil() as u64);
            let remaining = give_up
                .saturating_duration_since(Instant::now())
                .as_millis() as u64;
            let timeout = self.capped_by_deadline(until_timer.unwrap_or(u64::MAX).min(remaining));
            if timeout == 0 {
                continue;
            }
            // SAFETY: `handle` is non-null for the lifetime of self.
            unsafe { ibex2_hermes_wait(self.handle, timeout) };
        }
    }

    /// The global names a module can see.
    ///
    /// LLP 0062 R5: R1 is a property of a list, and a list nothing checks
    /// drifts. This is what makes the check mechanical.
    pub fn global_names(&mut self) -> Vec<String> {
        let raw = self
            .eval("Object.getOwnPropertyNames(globalThis).sort().join(',')")
            .unwrap_or_default();
        raw.split(',').map(str::to_string).collect()
    }

    /// Install the `__ibex2_async_echo` op. Tests only: a delegating op with
    /// no transport behind it, for holding the pump to its ordering contract.
    pub fn install_async_echo(&mut self) -> bool {
        // SAFETY: `handle` is non-null for the lifetime of self.
        unsafe { ibex2_hermes_install_async_echo(self.handle) == 0 }
    }

    /// Install `fetch`, carrying `grants` for the lifetime of the binding.
    pub fn install_fetch(&mut self, grants: &Grants) -> bool {
        // SAFETY: both pointers outlive the call; the binding keeps its own
        // reference to the grants.
        unsafe { ibex2_hermes_install_fetch(self.handle, grants.0) == 0 }
    }

    /// Drain microtasks without delivering completions.
    ///
    /// A job that throws out of the queue is `Thrown` — only an engine-raised
    /// error can, since Promise reactions and `queueMicrotask` catch their own
    /// — and the deadline is `Deadline`.
    pub fn drain_microtasks(&mut self) -> Result<(), JsError> {
        let mut out: *mut c_char = std::ptr::null_mut();
        // SAFETY: as above; `out` receives an owned string.
        let status = unsafe { ibex2_hermes_drain_microtasks(self.handle, &mut out) };
        checked(status, out, "ibex2_hermes_drain_microtasks").map(|_| ())
    }

    /// Install a zero-argument host function returning a fixed string.
    ///
    /// A placeholder for the real boundary, present to prove the LLP 0058
    /// requirement-2 path works on stock JSI before anything is built on it.
    pub fn install_probe(&mut self, name: &str, value: &str) -> bool {
        let name = CString::new(name).expect("name contains a NUL byte");
        let value = CString::new(value).expect("value contains a NUL byte");
        // SAFETY: both pointers outlive the call; the shim copies what it keeps.
        unsafe { ibex2_hermes_install_probe(self.handle, name.as_ptr(), value.as_ptr()) == 0 }
    }

    /// Install a zero-argument host function that throws `"<name> threw"`:
    /// natively, as a `std::runtime_error` the engine translates into a
    /// JavaScript error, or as a JavaScript error outright. Tests only — it
    /// is how the pump is held to its contract for a task that fails.
    pub fn install_throwing_probe(&mut self, name: &str, native: bool) -> bool {
        let name = CString::new(name).expect("name contains a NUL byte");
        // SAFETY: the pointer outlives the call; the shim copies what it keeps.
        unsafe {
            ibex2_hermes_install_throwing_probe(self.handle, name.as_ptr(), c_int::from(native))
                == 0
        }
    }
}

fn take_c_string(raw: *mut c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    // SAFETY: the shim malloc'd this and transferred ownership to us.
    let text = unsafe { CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { ibex2_hermes_free_string(raw) };
    Some(text)
}

impl Drop for Hermes {
    fn drop(&mut self) {
        // SAFETY: constructed non-null, destroyed exactly once.
        unsafe { ibex2_hermes_destroy(self.handle) };
    }
}

#[cfg(test)]
#[path = "hermes_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "hermes_deadline_tests.rs"]
mod deadline_tests;
