//! Native code stays mapped for process lifetime. Ownership never crosses the ABI.
use crate::Executor;
use exact_logic_abi::{ABI, MAX_MESSAGE};
use libloading::Library;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    sync::{mpsc, Arc, LazyLock, Mutex, OnceLock},
    time::Instant,
};

// One mapping per content identity, independent of host session count. Failed
// images are remembered too: retrying them cannot consume unbounded loader state.
#[derive(Default)]
struct Retained {
    bytes: usize,
    pending: usize,
    libraries: Vec<Library>,
    modules: HashMap<String, Image>,
}
static RETAINED: LazyLock<Mutex<Retained>> = LazyLock::new(|| Mutex::new(Retained::default()));
/// Wakes waiting for an image's load to finish, by digest.
type Waiter = (String, Box<dyn FnOnce() + Send>);
static WAITERS: LazyLock<Mutex<Vec<Waiter>>> = LazyLock::new(|| Mutex::new(Vec::new()));
const MAX_LOADS: usize = 64;
const MAX_RETAINED: usize = 128 << 20;
type Alloc = unsafe extern "C" fn(u32) -> usize;
type Dealloc = unsafe extern "C" fn(usize, u32);
type Call = unsafe extern "C" fn(usize, usize, u32) -> u32;
type Output = unsafe extern "C" fn(usize) -> usize;
type Length = unsafe extern "C" fn(usize) -> u32;
type Destroy = unsafe extern "C" fn(usize);
#[derive(Clone, Copy)]
struct Symbols {
    stateless: bool,
    create: unsafe extern "C" fn() -> usize,
    alloc: Alloc,
    dealloc: Dealloc,
    call: Call,
    output: Output,
    length: Length,
    destroy: Destroy,
}
struct Native {
    session: usize,
    symbols: Symbols,
}

pub(crate) fn load(bytes: &[u8]) -> Result<Box<dyn Executor>, String> {
    let (entry, _) = reserve(bytes, false)?.expect("unconditional reservation");
    let symbols = entry.get_or_init(|| load_symbols(bytes)).clone()?;
    // A content identity shares code, never mutable app session state.
    let session = unsafe { (symbols.create)() };
    if session == 0 {
        return Err("Rust native session creation failed".into());
    }
    Ok(Box::new(Native { session, symbols }))
}

type Image = Arc<OnceLock<Result<Symbols, String>>>;
type LoadJob = (Vec<u8>, Image, Instant);
static WORKER: OnceLock<Result<mpsc::Sender<LoadJob>, String>> = OnceLock::new();

fn reserve(bytes: &[u8], idle_only: bool) -> Result<Option<(Image, bool)>, String> {
    let digest = format!("{:x}", Sha256::digest(bytes));
    let mut retained = RETAINED
        .lock()
        .map_err(|_| "Rust native load budget lock poisoned")?;
    if let Some(entry) = retained.modules.get(&digest) {
        return Ok(Some((entry.clone(), false)));
    }
    // Tiered sessions keep executing Wasm instead of queuing obsolete images
    // behind a slow OS load. Only a still-live session retries when idle.
    if idle_only && retained.pending != 0 {
        return Ok(None);
    }
    if retained.modules.len() >= MAX_LOADS
        || bytes.len() > MAX_RETAINED.saturating_sub(retained.bytes)
    {
        return Err("Rust native retention budget exhausted; restart the app".into());
    }
    let entry = Arc::new(OnceLock::new());
    retained.bytes += bytes.len();
    retained.pending += 1;
    retained.modules.insert(digest, entry.clone());
    Ok(Some((entry, true)))
}

/// One bounded queue owns disk loading. No UI caller waits on dyld or a
/// mutex held by dyld. The budget includes queued and refused image bytes.
pub(crate) fn preload(bytes: &[u8]) -> Result<bool, String> {
    prepare(bytes, false)
}

/// Admit a new tiered image only when the loader is idle; cached images are
/// always available. Superseded sessions therefore leave no queued library.
pub(crate) fn preload_if_idle(bytes: &[u8]) -> Result<bool, String> {
    prepare(bytes, true)
}

fn prepare(bytes: &[u8], idle_only: bool) -> Result<bool, String> {
    let Some((entry, fresh)) = reserve(bytes, idle_only)? else {
        return Ok(false);
    };
    if fresh {
        let sender = WORKER.get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<LoadJob>();
            std::thread::Builder::new()
                .name("exact-rust-loader".into())
                .spawn(move || {
                    for (bytes, entry, queued) in receiver {
                        let digest = format!("{:x}", Sha256::digest(&bytes));
                        eprintln!(
                            "exact rust: native {digest}: queue {:.1} ms",
                            queued.elapsed().as_secs_f64() * 1000.0
                        );
                        entry.get_or_init(|| load_symbols(&bytes));
                        loaded(&digest);
                    }
                })
                .map(|_| sender)
                .map_err(|e| e.to_string())
        });
        let sent = sender.as_ref().map_err(Clone::clone).and_then(|sender| {
            sender
                .send((bytes.to_vec(), entry.clone(), Instant::now()))
                .map_err(|e| e.to_string())
        });
        if let Err(error) = sent {
            if entry.set(Err(error)).is_ok() {
                finished();
                loaded(&format!("{:x}", Sha256::digest(bytes)));
            }
        }
    }
    match entry.get() {
        Some(result) => result.clone().map(|_| true),
        None => Ok(false),
    }
}

fn load_symbols(bytes: &[u8]) -> Result<Symbols, String> {
    let result = map_symbols(bytes);
    finished();
    result
}

/// Call `wake` once this image's load has finished (at once, if it has).
pub(crate) fn when_loaded(bytes: &[u8], wake: Box<dyn FnOnce() + Send>) {
    let digest = format!("{:x}", Sha256::digest(bytes));
    let mut waiters = WAITERS.lock().unwrap_or_else(|e| e.into_inner());
    let loaded = RETAINED
        .lock()
        .ok()
        .and_then(|r| r.modules.get(&digest).map(|e| e.get().is_some()))
        .unwrap_or(true);
    if loaded {
        drop(waiters);
        wake();
    } else {
        waiters.push((digest, wake));
    }
}

/// The image with `digest` finished loading: wake whoever waits for it.
fn loaded(digest: &str) {
    let ready: Vec<Waiter> = {
        let mut waiters = WAITERS.lock().unwrap_or_else(|e| e.into_inner());
        let (ready, rest) = std::mem::take(&mut *waiters)
            .into_iter()
            .partition(|(d, _)| d == digest);
        *waiters = rest;
        ready
    };
    for (_, wake) in ready {
        wake();
    }
}

fn finished() {
    if let Ok(mut retained) = RETAINED.lock() {
        retained.pending -= 1;
    }
}

fn map_symbols(bytes: &[u8]) -> Result<Symbols, String> {
    let started = Instant::now();
    let digest = format!("{:x}", Sha256::digest(bytes));
    let identity = &digest[..12];
    let attempt = RETAINED
        .lock()
        .map_err(|_| "Rust native load budget lock poisoned")?
        .modules
        .len();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "exact-rust-{}-{stamp}-{attempt}",
        std::process::id()
    ));
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder.create(&directory).map_err(|e| e.to_string())?;
    let suffix = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    let path = directory.join(format!("module.{suffix}"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    let written = started.elapsed();
    file.sync_all().map_err(|e| e.to_string())?;
    let synced = started.elapsed();
    drop(file);
    eprintln!(
        "exact rust: native {identity}: write {:.1} ms, sync {:.1} ms; entering dlopen",
        written.as_secs_f64() * 1000.0,
        (synced - written).as_secs_f64() * 1000.0
    );
    // SAFETY: only admitted producer code reaches this native executor. A native
    // module has the process's authority; ABI checks do not sandbox its pointers.
    let library = unsafe { Library::new(&path) };
    eprintln!(
        "exact rust: native {identity}: dlopen {:.1} ms ({})",
        (started.elapsed() - synced).as_secs_f64() * 1000.0,
        if library.is_ok() { "loaded" } else { "refused" }
    );
    // Unix keeps the mapping alive independently of the pathname; unique copies
    // need not accumulate on disk. Windows retains its file while the image is open.
    #[cfg(unix)]
    {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&directory);
    }
    let library = library.map_err(|e| e.to_string())?;
    // Keep the handle even when export inspection refuses the image.
    let result = (|| unsafe {
        let version = *library
            .get::<unsafe extern "C" fn() -> u32>(b"exact_logic_abi\0")
            .map_err(|e| e.to_string())?;
        if version() != ABI {
            return Err("Rust native export ABI differs".into());
        }
        Ok(Symbols {
            stateless: library
                .get::<unsafe extern "C" fn() -> u32>(b"exact_logic_stateless\0")
                .is_ok_and(|function| function() == 1),
            create: *library
                .get(b"exact_logic_create\0")
                .map_err(|e| e.to_string())?,
            alloc: *library
                .get(b"exact_logic_alloc\0")
                .map_err(|e| e.to_string())?,
            dealloc: *library
                .get(b"exact_logic_dealloc\0")
                .map_err(|e| e.to_string())?,
            call: *library
                .get(b"exact_logic_call\0")
                .map_err(|e| e.to_string())?,
            output: *library
                .get(b"exact_logic_output\0")
                .map_err(|e| e.to_string())?,
            length: *library
                .get(b"exact_logic_output_len\0")
                .map_err(|e| e.to_string())?,
            destroy: *library
                .get(b"exact_logic_destroy\0")
                .map_err(|e| e.to_string())?,
        })
    })();
    match RETAINED.lock() {
        Ok(mut retained) => retained.libraries.push(library),
        Err(_) => std::mem::forget(library),
    }
    result
}
impl Executor for Native {
    fn stateless(&self) -> bool {
        self.symbols.stateless
    }
    fn call(&mut self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        if bytes.len() > MAX_MESSAGE {
            return Err("Rust native request exceeds bound".into());
        }
        // SAFETY: exact_logic_alloc allocates inside the pinned module; it alone
        // frees that input. Output is copied before another call can change it.
        unsafe {
            let len = bytes.len() as u32;
            let ptr = (self.symbols.alloc)(len);
            if ptr == 0 {
                return Err("Rust native allocation failed".into());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
            let result = (self.symbols.call)(self.session, ptr, len);
            (self.symbols.dealloc)(ptr, len);
            if result != 0 {
                return Err("Rust native refused encoded call".into());
            }
            let ptr = (self.symbols.output)(self.session);
            let len = (self.symbols.length)(self.session) as usize;
            if len > MAX_MESSAGE || (ptr == 0 && len > 0) {
                return Err("Rust native reply exceeds bound or has no buffer".into());
            }
            if len == 0 {
                return Ok(Vec::new());
            }
            Ok(std::slice::from_raw_parts(ptr as *const u8, len).to_vec())
        }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        if self.session != 0 {
            unsafe { (self.symbols.destroy)(self.session) }
        }
    }
}

#[cfg(all(test, unix))]
pub(crate) fn mapping_count(bytes: &[u8]) -> usize {
    let digest = format!("{:x}", Sha256::digest(bytes));
    usize::from(RETAINED.lock().unwrap().modules.contains_key(&digest))
}
