//! Experiment instrumentation: counts Rust allocator calls, not JS/Swift/RSS.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
pub static ALLOCS: AtomicU64 = AtomicU64::new(0);
pub static BYTES: AtomicU64 = AtomicU64::new(0);
pub static FIRST_TICK_US: AtomicU64 = AtomicU64::new(0);
pub static ENTRY_US: AtomicU64 = AtomicU64::new(0);
pub struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size() as u64, Relaxed);
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size() as u64, Relaxed);
        System.alloc_zeroed(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(n as u64, Relaxed);
        System.realloc(p, l, n)
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;
pub fn timestamp() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}
pub fn first_tick() {
    let _ = FIRST_TICK_US.compare_exchange(0, timestamp(), Relaxed, Relaxed);
}
pub fn entry() {
    ENTRY_US.store(timestamp(), Relaxed);
}
pub fn value() -> exact_runner::Value {
    use exact_runner::Value;
    Value::record(
        [&ALLOCS, &BYTES, &FIRST_TICK_US, &ENTRY_US]
            .map(|n| Value::Number(n.load(Relaxed) as f64))
            .to_vec(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub fn trace(source: &crate::TallySource, name: &str, args: &[exact_runner::Value]) {
    use std::sync::atomic::AtomicBool;
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static LAST: AtomicU64 = AtomicU64::new(u64::MAX);
    static FIRST: AtomicBool = AtomicBool::new(true);
    if !*ENABLED.get_or_init(|| std::env::var_os("X1_TRACE").is_some()) || name != "world" {
        return;
    }
    let now = args
        .first()
        .and_then(exact_runner::Value::as_number)
        .unwrap_or(0.);
    let second = (now / 1000.) as u64;
    if LAST.swap(second, Relaxed) != second {
        let w = source.0.sim.world();
        eprintln!("X1 {{\"now\":{now},\"tick\":{},\"wallUs\":{},\"allocations\":{},\"bytes\":{},\"queries\":{}}}",w.tick(),timestamp(),ALLOCS.load(Relaxed),BYTES.load(Relaxed),source.0.queries);
        if FIRST.swap(false, Relaxed) {
            eprintln!(
                "X1RESTORE {} {}",
                w.tick(),
                source.0.checkpoint().unwrap_or_default()
            );
        }
    }
}

/// Explicit live-store experiment, enabled only in the measurement process.
#[cfg(not(target_arch = "wasm32"))]
pub fn live_save(
    source: &mut crate::TallySource,
    store: &mut exact_runner::Store,
    name: &str,
    args: &[exact_runner::Value],
) -> Result<(), exact_runner::DataError> {
    use exact_runner::DataSource;
    use std::sync::atomic::AtomicBool;
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static SAVED: AtomicBool = AtomicBool::new(false);
    if !*ENABLED.get_or_init(|| std::env::var_os("X1_LIVE_SAVE").is_some()) {
        return Ok(());
    }
    let now = args
        .first()
        .and_then(exact_runner::Value::as_number)
        .unwrap_or(0.);
    if name == "world" && now >= 1000. && !SAVED.swap(true, Relaxed) {
        source.0.answer(store, "save", args)?;
        eprintln!(
            "X1SAVED {} {}",
            source.0.sim.world().tick(),
            source.0.checkpoint()?
        );
    }
    Ok(())
}
