//! Storage installed into an independently created runtime, without its loader.
#![cfg(feature = "hermes")]
use ibex2::stdlib::app_fs::AppDirectories;
use ibex2::{
    bindings::{Context, Groups},
    grant::GrantSet,
};
use std::{
    ffi::{c_char, c_void, CStr},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[repr(C)]
struct CompiledScript {
    name: *const c_char,
    bytes: *const u8,
    len: usize,
}

extern "C" {
    fn bindings_validate_groups(groups: u16, error: *mut *mut c_char) -> i32;
    fn bindings_expected_scripts(groups: u16, error: *mut *mut c_char) -> *mut c_char;
    fn bindings_consumer_create_uninstalled(queue: *const c_void) -> *mut c_void;
    fn bindings_consumer_install(
        handle: *mut c_void,
        bindings: *const ibex2::bindings::Ibex2Bindings,
        groups: u16,
        scripts: *const CompiledScript,
        script_count: usize,
        error: *mut *mut c_char,
    ) -> i32;
    fn bindings_consumer_create(
        queue: *const c_void,
        bindings: *const ibex2::bindings::Ibex2Bindings,
        groups: u16,
        scripts: *const CompiledScript,
        script_count: usize,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn storage_consumer_create(
        queue: *const c_void,
        grants: *const c_void,
        factory: *const u8,
        len: usize,
        harden: *const u8,
        harden_len: usize,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn storage_consumer_eval(
        h: *mut c_void,
        data: *const u8,
        len: usize,
        out: *mut *mut c_char,
    ) -> i32;
    fn storage_consumer_step(h: *mut c_void, deliver: bool, out: *mut *mut c_char) -> i32;
    fn storage_consumer_detach(h: *mut c_void);
    fn storage_consumer_destroy(h: *mut c_void);
    fn storage_consumer_free(s: *mut c_char);
}

fn compiled_script(name: &str) -> CompiledScript {
    let (name, bytes): (&'static [u8], &'static [u8]) = match name {
        "headers" => (
            b"headers\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/headers.hbc")),
        ),
        "timers" => (
            b"timers\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/timers.hbc")),
        ),
        "url" => (
            b"url\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/url.hbc")),
        ),
        "domexception" => (
            b"domexception\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/domexception.hbc")),
        ),
        "crypto" => (
            b"crypto\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/crypto.hbc")),
        ),
        "abort" => (
            b"abort\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/abort.hbc")),
        ),
        "structured_clone" => (
            b"structured_clone\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/structured_clone.hbc")),
        ),
        "fetch" => (
            b"fetch\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/fetch.hbc")),
        ),
        "sqlite" => (
            b"sqlite\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/sqlite.hbc")),
        ),
        #[cfg(target_os = "linux")]
        "intl_number_format" => (
            b"intl_number_format\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/intl_number_format.hbc")),
        ),
        #[cfg(target_os = "linux")]
        "intl_case" => (
            b"intl_case\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/intl_case.hbc")),
        ),
        #[cfg(target_os = "linux")]
        "intl_datetime" => (
            b"intl_datetime\0",
            include_bytes!(concat!(env!("OUT_DIR"), "/intl_datetime.hbc")),
        ),
        _ => unreachable!(),
    };
    CompiledScript {
        name: name.as_ptr().cast(),
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

struct BareConsumer {
    handle: *mut c_void,
    context: Option<Context>,
    directory: PathBuf,
}

impl BareConsumer {
    fn new(groups: Groups) -> Self {
        Self::from_context(groups, Context::new(GrantSet::none()))
    }
    fn from_context(groups: Groups, context: Context) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "ibex2-groups-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let scripts: Vec<_> = ibex2::bindings::scripts(groups)
            .unwrap()
            .into_iter()
            .map(|(name, _)| compiled_script(name))
            .collect();
        let mut error = std::ptr::null_mut();
        let handle = unsafe {
            bindings_consumer_create(
                context.state_ptr(),
                context.bindings_ptr(),
                groups.bits(),
                scripts.as_ptr(),
                scripts.len(),
                &mut error,
            )
        };
        assert!(!handle.is_null(), "{}", take(error));
        Self {
            handle,
            context: Some(context),
            directory,
        }
    }

    fn eval_result(&self, source: &str) -> Result<String, String> {
        let input = self.directory.join("test.js");
        let output = self.directory.join("test.hbc");
        std::fs::write(&input, source).unwrap();
        let compiler = std::env::var("IBEX2_HERMESC")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let arch = if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x64"
                };
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tools/hermes-vanilla/hermesc-{}-{arch}{}",
                    std::env::consts::OS,
                    std::env::consts::EXE_SUFFIX
                ))
            });
        assert!(std::process::Command::new(compiler)
            .args(["-O", "-emit-binary", "-out"])
            .arg(&output)
            .arg(&input)
            .status()
            .unwrap()
            .success());
        let bytes = std::fs::read(output).unwrap();
        let mut out = std::ptr::null_mut();
        let status =
            unsafe { storage_consumer_eval(self.handle, bytes.as_ptr(), bytes.len(), &mut out) };
        let text = take(out);
        if status == 0 {
            Ok(text)
        } else {
            Err(text)
        }
    }

    fn eval(&self, source: &str) -> String {
        self.eval_result(source).unwrap()
    }

    fn detach_and_drop_context(&mut self) {
        unsafe { storage_consumer_detach(self.handle) };
        drop(self.context.take());
    }
}

impl Drop for BareConsumer {
    fn drop(&mut self) {
        unsafe { storage_consumer_destroy(self.handle) };
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct Consumer {
    handle: *mut c_void,
    context: Context,
    directory: PathBuf,
    wakes: Arc<AtomicUsize>,
}
fn take(s: *mut c_char) -> String {
    if s.is_null() {
        return String::new();
    }
    let result = unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned();
    unsafe { storage_consumer_free(s) };
    result
}
impl Consumer {
    fn new(grants: &str) -> Self {
        Self::configured(grants, true)
    }
    fn configured(grants: &str, hardened: bool) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "ibex2-embed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for name in ["data", "cache", "tmp"] {
            std::fs::create_dir_all(directory.join(name)).unwrap();
        }
        let context = Context::new(GrantSet::parse(grants).unwrap());
        context
            .set_app_directories(
                AppDirectories::new(
                    directory.join("data"),
                    directory.join("cache"),
                    directory.join("tmp"),
                )
                .unwrap(),
            )
            .unwrap();
        context
            .set_sqlite_provider(Arc::new(ibex2_sqlite::SqliteProvider))
            .unwrap();
        let wakes = Arc::new(AtomicUsize::new(0));
        let observed = wakes.clone();
        context.set_wake(Arc::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        }));
        let factory = include_bytes!(concat!(env!("OUT_DIR"), "/sqlite.hbc"));
        let harden = include_bytes!(concat!(env!("OUT_DIR"), "/harden.hbc"));
        let mut error = std::ptr::null_mut();
        let handle = unsafe {
            storage_consumer_create(
                context.state_ptr(),
                context.grants_ptr(),
                factory.as_ptr(),
                factory.len(),
                harden.as_ptr(),
                if hardened { harden.len() } else { 0 },
                &mut error,
            )
        };
        assert!(!handle.is_null(), "{}", take(error));
        Self {
            handle,
            context,
            directory,
            wakes,
        }
    }
    fn eval(&self, source: &str) -> Result<String, String> {
        let input = self.directory.join("test.js");
        let output = self.directory.join("test.hbc");
        std::fs::write(&input, source).unwrap();
        let compiler = std::env::var("IBEX2_HERMESC")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let arch = if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x64"
                };
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tools/hermes-vanilla/hermesc-{}-{arch}{}",
                    std::env::consts::OS,
                    std::env::consts::EXE_SUFFIX
                ))
            });
        assert!(std::process::Command::new(compiler)
            .args(["-O", "-emit-binary", "-out"])
            .arg(&output)
            .arg(&input)
            .output()
            .unwrap()
            .status
            .success());
        let bytes = std::fs::read(output).unwrap();
        let mut out = std::ptr::null_mut();
        let status =
            unsafe { storage_consumer_eval(self.handle, bytes.as_ptr(), bytes.len(), &mut out) };
        let text = take(out);
        if status == 0 {
            Ok(text)
        } else {
            Err(text)
        }
    }
    fn step(&self, deliver: bool) -> i32 {
        let mut out = std::ptr::null_mut();
        let n = unsafe { storage_consumer_step(self.handle, deliver, &mut out) };
        assert!(n >= 0, "{}", take(out));
        n
    }
    fn finish(&self) -> String {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            self.step(false);
            let result = self.eval("globalThis.result || ''").unwrap();
            if !result.is_empty() {
                return result;
            }
            assert!(Instant::now() < end, "application did not settle");
            self.context.wait(Duration::from_millis(50));
            self.step(true);
        }
    }
}
impl Drop for Consumer {
    fn drop(&mut self) {
        unsafe { storage_consumer_destroy(self.handle) };
        // Context shutdown follows destruction; all tests explicitly close DBs.
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn global_names(consumer: &BareConsumer) -> std::collections::BTreeSet<String> {
    consumer
        .eval("Object.getOwnPropertyNames(globalThis).sort().join(',')")
        .split(',')
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn pure_installs_exactly_its_globals_into_a_bare_runtime() {
    let baseline = global_names(&BareConsumer::new(Groups::empty()));
    let installed = global_names(&BareConsumer::new(Groups::PURE));
    let added: std::collections::BTreeSet<_> = installed.difference(&baseline).cloned().collect();
    let expected: std::collections::BTreeSet<_> = [
        "URL",
        "URLSearchParams",
        "Headers",
        "TextEncoder",
        "TextDecoder",
        "atob",
        "btoa",
        "DOMException",
        "QuotaExceededError",
        "structuredClone",
    ]
    .into_iter()
    .map(str::to_string)
    .filter(|name| !baseline.contains(name))
    .collect();
    assert_eq!(added, expected);
}

#[test]
fn fetch_group_does_not_install_timers_or_crypto() {
    let baseline = global_names(&BareConsumer::new(Groups::empty()));
    let installed = global_names(&BareConsumer::new(
        Groups::PURE | Groups::ABORT | Groups::FETCH,
    ));
    let added: std::collections::BTreeSet<_> = installed.difference(&baseline).cloned().collect();
    let expected: std::collections::BTreeSet<_> = [
        "URL",
        "URLSearchParams",
        "Headers",
        "TextEncoder",
        "TextDecoder",
        "atob",
        "btoa",
        "DOMException",
        "QuotaExceededError",
        "AbortController",
        "AbortSignal",
        "fetch",
        "structuredClone",
    ]
    .into_iter()
    .map(str::to_string)
    .filter(|name| !baseline.contains(name))
    .collect();
    assert_eq!(added, expected);
}

#[test]
fn omitted_crypto_group_exposes_no_crypto_surface_or_subtle_ops() {
    let consumer = BareConsumer::new(Groups::PURE);
    assert_eq!(
        consumer.eval(
            r#"[
              typeof crypto,
              typeof Crypto,
              typeof CryptoKey,
              typeof SubtleCrypto,
              typeof globalThis.__ibex2_random_uuid,
              typeof globalThis.__ibex2_get_random_values,
              typeof globalThis.__ibex2_subtle
            ].join(',')"#,
        ),
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined"
    );
    assert_eq!(
        consumer.eval(
            r#"try {
              globalThis.__ibex2_subtle.digest("SHA-256", new Uint8Array());
              "reachable";
            } catch (error) {
              error instanceof TypeError ? "unreachable" : error.name;
            }"#,
        ),
        "unreachable"
    );
}

#[test]
fn borrowed_runtime_refuses_fetch_without_its_dependencies() {
    let context = Context::new(GrantSet::none());
    let mut error = std::ptr::null_mut();
    let handle = unsafe {
        bindings_consumer_create(
            context.state_ptr(),
            context.bindings_ptr(),
            Groups::FETCH.bits(),
            std::ptr::null(),
            0,
            &mut error,
        )
    };
    assert!(handle.is_null(), "dependency-invalid groups were installed");
    let error = take(error);
    assert!(
        error.contains("missing a dependency"),
        "unexpected dependency error: {error}"
    );
}

#[test]
fn rust_and_cpp_group_validation_tables_agree() {
    let group_bits = [
        Groups::PURE,
        Groups::CONSOLE,
        Groups::TIMERS,
        Groups::ABORT,
        Groups::CRYPTO,
        Groups::FETCH,
        Groups::STORAGE,
        Groups::ENV,
        Groups::SECRETS,
        Groups::KV,
        Groups::INTL,
    ];
    for mask in 0..(1usize << group_bits.len()) {
        let mut groups = Groups::empty();
        for (index, group) in group_bits.iter().enumerate() {
            if mask & (1 << index) != 0 {
                groups |= *group;
            }
        }
        let mut error = std::ptr::null_mut();
        let cpp_valid = unsafe { bindings_validate_groups(groups.bits(), &mut error) } == 1;
        if !error.is_null() {
            let _ = take(error);
        }
        assert_eq!(
            cpp_valid,
            groups.validate().is_ok(),
            "Rust and C++ disagree for {groups:?}"
        );
        if groups.validate().is_ok() {
            let mut error = std::ptr::null_mut();
            let cpp_scripts = unsafe { bindings_expected_scripts(groups.bits(), &mut error) };
            assert!(error.is_null(), "C++ script list refused {groups:?}");
            assert!(!cpp_scripts.is_null(), "C++ returned no script list");
            let cpp_scripts = take(cpp_scripts);
            let cpp_scripts: Vec<_> = cpp_scripts.lines().collect();
            let rust_scripts = ibex2::bindings::scripts(groups).unwrap();
            let rust_scripts: Vec<_> = rust_scripts.iter().map(|(name, _)| *name).collect();
            assert_eq!(
                cpp_scripts, rust_scripts,
                "script order differs for {groups:?}"
            );
        }
    }

    let mut error = std::ptr::null_mut();
    let cpp_intl = unsafe { bindings_validate_groups(Groups::INTL.bits(), &mut error) } == 1;
    if !error.is_null() {
        let _ = take(error);
    }
    assert_eq!(cpp_intl, cfg!(target_os = "linux"));
}

#[test]
fn public_header_documents_every_bindings_handle_producer_and_releaser() {
    let header = include_str!("../include/ibex2_jsi.h");
    for name in [
        "Context::bindings_ptr()",
        "ibex2_bindings_adopt",
        "ibex2_bindings_destroy",
    ] {
        assert!(
            header.lines().take(20).any(|line| line.contains(name)),
            "the Ibex2Bindings header comment omits {name}"
        );
    }
}

#[test]
fn retained_pure_bindings_refuse_after_detach_and_context_drop() {
    let mut consumer = BareConsumer::new(Groups::PURE);
    assert_eq!(
        consumer.eval("globalThis.SavedHeaders = Headers; globalThis.SavedURL = URL; 'saved'"),
        "saved"
    );
    consumer.detach_and_drop_context();

    for source in ["new SavedHeaders()", "new SavedURL('https://example.com')"] {
        let error = consumer.eval_result(source).unwrap_err();
        assert!(
            error.contains("Ibex2 bindings are detached"),
            "unexpected detached error for {source}: {error}"
        );
    }
}

#[test]
fn dropping_the_last_context_owner_cancels_fetch_without_a_late_wake() {
    use ibex2::host::Host;
    use ibex2::stdlib::fetch::{Request, StreamingResponse, Transport};
    use std::sync::{mpsc, Condvar, Mutex};

    struct BlockingTransport {
        entered: mpsc::Sender<()>,
        cancelled: mpsc::Sender<()>,
        returned: mpsc::Sender<()>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }
    impl Transport for BlockingTransport {
        fn open(
            &self,
            _request: &Request,
            signal: &ibex2::stdlib::abort::AbortSignal,
        ) -> Result<StreamingResponse, ibex2::boundary::HostError> {
            let cancelled = Arc::new((Mutex::new(false), Condvar::new()));
            let notify = Arc::clone(&cancelled);
            let _registration = signal.register(move || {
                *notify.0.lock().unwrap() = true;
                notify.1.notify_all();
            });
            self.entered.send(()).unwrap();
            let mut was_cancelled = cancelled.0.lock().unwrap();
            while !*was_cancelled {
                was_cancelled = cancelled.1.wait(was_cancelled).unwrap();
            }
            self.cancelled.send(()).unwrap();
            drop(was_cancelled);

            // Keep the worker alive until after Context::drop returns. Its Arc
            // must not count as an owner or preserve the wake callback.
            let mut release = self.release.0.lock().unwrap();
            while !*release {
                release = self.release.1.wait(release).unwrap();
            }
            self.returned.send(()).unwrap();
            signal.check()?;
            unreachable!()
        }
    }

    let (entered_tx, entered_rx) = mpsc::channel();
    let (cancelled_tx, cancelled_rx) = mpsc::channel();
    let (returned_tx, returned_rx) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let bindings = Host::with_transport(Box::new(BlockingTransport {
        entered: entered_tx,
        cancelled: cancelled_tx,
        returned: returned_tx,
        release: Arc::clone(&release),
    }))
    .endow(GrantSet::parse("net.fetch https://blocked.example\n").unwrap());
    let context = Context::from_bindings(&bindings);
    let (wake_tx, wake_rx) = mpsc::channel();
    context.set_wake(Arc::new(move || {
        let _ = wake_tx.send(());
    }));
    let groups = Groups::PURE | Groups::ABORT | Groups::FETCH;
    let mut consumer = BareConsumer::from_context(groups, context);
    consumer.eval("fetch('https://blocked.example/').catch(function () {})");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("fetch did not reach transport");

    consumer.detach_and_drop_context();
    cancelled_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Context drop did not promptly cancel fetch");
    *release.0.lock().unwrap() = true;
    release.1.notify_all();
    returned_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("cancelled transport did not return");
    assert!(
        wake_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "a completion invoked the wake callback after Context drop"
    );
}

#[test]
fn grouped_install_refuses_source_bytes() {
    let context = Context::new(GrantSet::none());
    let name = b"headers\0";
    let source = include_bytes!("../src/bindings/headers.js");
    let mut scripts: Vec<_> = ibex2::bindings::scripts(Groups::PURE)
        .unwrap()
        .into_iter()
        .map(|(name, _)| compiled_script(name))
        .collect();
    scripts[0] = CompiledScript {
        name: name.as_ptr().cast(),
        bytes: source.as_ptr(),
        len: source.len(),
    };
    let mut error = std::ptr::null_mut();
    let handle = unsafe {
        bindings_consumer_create(
            context.state_ptr(),
            context.bindings_ptr(),
            Groups::PURE.bits(),
            scripts.as_ptr(),
            scripts.len(),
            &mut error,
        )
    };
    assert!(handle.is_null(), "source payload unexpectedly installed");
    let error = take(error);
    assert!(
        error.contains("not Hermes bytecode"),
        "unexpected install error: {error}"
    );
}

#[test]
fn grouped_install_refuses_a_grant_pointer_as_the_install_handle() {
    let context = Context::new(GrantSet::none());
    let scripts: Vec<_> = ibex2::bindings::scripts(Groups::PURE)
        .unwrap()
        .into_iter()
        .map(|(name, _)| compiled_script(name))
        .collect();
    let mut error = std::ptr::null_mut();
    let handle = unsafe {
        bindings_consumer_create(
            context.state_ptr(),
            context.grants_ptr().cast(),
            Groups::PURE.bits(),
            scripts.as_ptr(),
            scripts.len(),
            &mut error,
        )
    };
    assert!(
        handle.is_null(),
        "a grant pointer installed as an endowment"
    );
    let error = take(error);
    assert!(
        error.contains("live endowment"),
        "unexpected error: {error}"
    );
}

fn assert_failed_install_is_terminal(
    context: &Context,
    first: &[CompiledScript],
    expected_error: &str,
) -> String {
    let valid: Vec<_> = ibex2::bindings::scripts(Groups::PURE)
        .unwrap()
        .into_iter()
        .map(|(name, _)| compiled_script(name))
        .collect();
    let handle = unsafe { bindings_consumer_create_uninstalled(context.state_ptr()) };
    assert!(!handle.is_null(), "test runtime");

    let mut error = std::ptr::null_mut();
    let installed = unsafe {
        bindings_consumer_install(
            handle,
            context.bindings_ptr(),
            Groups::PURE.bits(),
            first.as_ptr(),
            first.len(),
            &mut error,
        )
    };
    assert_eq!(installed, 0, "malformed bytecode unexpectedly installed");
    let error = take(error);
    assert!(
        error.contains(expected_error),
        "unexpected first error: {error}"
    );

    let mut retry_error = std::ptr::null_mut();
    let retried = unsafe {
        bindings_consumer_install(
            handle,
            context.bindings_ptr(),
            Groups::PURE.bits(),
            valid.as_ptr(),
            valid.len(),
            &mut retry_error,
        )
    };
    assert_eq!(retried, 0, "a failed Adapter accepted a retry");
    let retry_error = take(retry_error);
    assert!(
        retry_error.contains("spent") && retry_error.contains("discarded"),
        "retry did not require runtime disposal: {retry_error}"
    );
    unsafe { storage_consumer_destroy(handle) };
    error
}

#[test]
fn truncated_binding_bytecode_is_refused_and_spends_the_adapter() {
    let context = Context::new(GrantSet::none());
    let mut bytes = include_bytes!(concat!(env!("OUT_DIR"), "/headers.hbc")).to_vec();
    bytes.truncate(bytes.len() - 1);
    let name = b"headers\0";
    let mut scripts: Vec<_> = ibex2::bindings::scripts(Groups::PURE)
        .unwrap()
        .into_iter()
        .map(|(name, _)| compiled_script(name))
        .collect();
    scripts[0] = CompiledScript {
        name: name.as_ptr().cast(),
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    };
    let _ = assert_failed_install_is_terminal(&context, &scripts, "declared length");
}

#[test]
fn spoofed_binding_header_is_refused_in_preflight_and_spends_the_adapter() {
    let context = Context::new(GrantSet::none());
    let valid = include_bytes!(concat!(env!("OUT_DIR"), "/headers.hbc"));
    let mut bytes = [0; 36];
    bytes[..12].copy_from_slice(&valid[..12]);
    bytes[32..36].copy_from_slice(&36u32.to_le_bytes());
    let name = b"headers\0";
    let mut scripts: Vec<_> = ibex2::bindings::scripts(Groups::PURE)
        .unwrap()
        .into_iter()
        .map(|(name, _)| compiled_script(name))
        .collect();
    scripts[0] = CompiledScript {
        name: name.as_ptr().cast(),
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    };
    let error =
        assert_failed_install_is_terminal(&context, &scripts, "truncated Hermes bytecode header");
    assert!(
        !error.contains("after mutating"),
        "the fixed-header refusal happened after publication: {error}"
    );
}

#[test]
fn wrong_binding_version_is_refused_and_spends_a_versioned_adapter() {
    let context = Context::new(GrantSet::none());
    let mut bytes = include_bytes!(concat!(env!("OUT_DIR"), "/headers.hbc")).to_vec();
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    bytes[8..12].copy_from_slice(&version.wrapping_add(1).to_le_bytes());
    let name = b"headers\0";
    let mut scripts: Vec<_> = ibex2::bindings::scripts(Groups::PURE)
        .unwrap()
        .into_iter()
        .map(|(name, _)| compiled_script(name))
        .collect();
    scripts[0] = CompiledScript {
        name: name.as_ptr().cast(),
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    };
    let _ = assert_failed_install_is_terminal(&context, &scripts, "version does not match");
}

#[test]
fn caller_owns_checkpoints_and_storage_is_typed_and_granted() {
    let c = Consumer::new("fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/db");
    // Both names exercise escaped strings; Windows refuses control characters.
    let filename = if cfg!(windows) { r"a\u00e9 b" } else { r"a\nb" };
    c.eval(&format!("globalThis.storageFilename = '{filename}';"))
        .unwrap();
    c.eval(r#"globalThis.result = ''; storage.fs.atomicWriteFile('app:/data/' + storageFilename, new Uint8Array([1,2])).then(function(){ result = 'written'; });"#).unwrap();
    assert!(c.context.wait(Duration::from_secs(5)));
    assert_eq!(c.eval("result").unwrap(), "");
    assert_eq!(c.step(true), 1);
    assert_eq!(
        c.eval("result").unwrap(),
        "",
        "delivery must not drain microtasks"
    );
    c.step(false);
    assert_eq!(c.eval("result").unwrap(), "written");
    c.eval(r#"result = ''; (async function(){
      const names = await storage.fs.readdir('app:/data');
      if (names.length !== 1 || names[0] !== storageFilename) throw Error('filename');
      const stat = await storage.fs.stat('app:/data/' + storageFilename);
      if (!stat.isFile || stat.isDirectory || stat.size !== 2) throw Error('stat');
      const bytes = await storage.fs.readFile('app:/data/' + storageFilename);
      if (!(bytes instanceof ArrayBuffer) || new Uint8Array(bytes)[1] !== 2) throw Error('bytes');
      const db = await storage.sqlite.open('app:/data/db');
      await db.execute('CREATE TABLE notes(body TEXT)');
      await db.transaction([{sql:'INSERT INTO notes VALUES (?)',params:['remember']}]);
      await db.close();
      const again = await storage.sqlite.open('app:/data/db');
      const rows = await again.query('SELECT body, 9223372036854775807 FROM notes');
      await again.close();
      if(rows.rows[0][0] !== 'remember' || rows.rows[0][1] !== BigInt('9223372036854775807')) throw Error('persistence');
      try { await storage.fs.writeFile('app:/cache/no', new Uint8Array([0])); throw Error('leaked'); }
      catch(e) { if(e.message === 'leaked') throw e; }
      result='ok';
    })().catch(e => result=String(e));"#).unwrap();
    assert_eq!(c.finish(), "ok");
    assert!(c.wakes.load(Ordering::SeqCst) > 0);
    assert!(!c.directory.join("cache/no").exists());
}

#[test]
fn detached_capabilities_and_pending_work_do_not_reach_dead_runtime() {
    let c = Consumer::new("fs.write app:/data");
    c.eval("storage.fs.writeFile('app:/data/file', new Uint8Array([1]));")
        .unwrap();
    unsafe { storage_consumer_detach(c.handle) };
    assert!(c
        .eval("storage.fs.writeFile('app:/data/other', new Uint8Array([1]));")
        .is_err());
    assert!(!c.directory.join("data/other").exists());
}

#[test]
fn empty_grants_refuse_files_and_sqlite_before_creation() {
    let c = Consumer::new("");
    c.eval(r#"globalThis.result = ''; (async function(){
      let refused = 0;
      try { await storage.fs.writeFile('app:/data/no', new Uint8Array([1])); } catch(e) { refused++; }
      try { await storage.sqlite.open('app:/data/db'); } catch(e) { refused++; }
      result = String(refused);
    })();"#).unwrap();
    assert_eq!(c.finish(), "2");
    assert_eq!(
        std::fs::read_dir(c.directory.join("data")).unwrap().count(),
        0
    );
}

#[test]
fn sqlite_refuses_a_caller_that_has_not_hardened_its_intrinsics() {
    let c = Consumer::configured("sqlite.open app:/data/db", false);
    c.eval("globalThis.result=''; storage.sqlite.open('app:/data/db').then(() => result='opened', e => result=String(e));").ok();
    // The adapter refuses synchronously before publishing native work.
    let error = c.eval("storage.sqlite.open('app:/data/db')").unwrap_err();
    assert!(error.contains("harden"), "{error}");
    assert!(!c.directory.join("data/db").exists());
}

#[test]
fn freezing_modified_intrinsics_does_not_satisfy_the_installation_contract() {
    let c = Consumer::configured("sqlite.open app:/data/db", false);
    c.eval("WeakMap.prototype.get = function () { return undefined; };")
        .unwrap();
    c.eval(include_str!("../src/bindings/harden.js")).unwrap();
    let error = c.eval("storage.sqlite.open('app:/data/db')").unwrap_err();
    assert!(error.contains("harden"), "{error}");
    assert!(!c.directory.join("data/db").exists());
}
