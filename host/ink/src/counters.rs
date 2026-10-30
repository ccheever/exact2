//! Opt-in cumulative observations of the live painter. @ref LLP 1149.001 Stage 0.
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

macro_rules! counters {
    ($($name:ident => $label:literal),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub enum Counter { $($name),+ }
        pub const NAMES: &[&str] = &[$($label),+];
        pub const C_NAMES: &[&[u8]] = &[$(concat!($label, "\0").as_bytes()),+];
    };
}
counters! {
    PaintAttempts => "paint_attempts",
    Submitted => "submitted_frames",
    ServiceWheel => "service_wheel",
    ServiceLink => "service_link",
    ServiceWorker => "service_worker",
    ServiceTimer => "service_timer",
    ServiceAgent => "service_agent",
    ServiceWindow => "service_window",
    ServiceDispatch => "service_dispatch",
    ServiceDraw => "service_draw",
    ServiceTotal => "service_total",
    PassesInPeriod => "service_passes_in_period",
    DisplayPeriods => "display_periods",
    RowsReplayed => "rows_replayed",
    RowsRecorded => "rows_recorded_whole",
    RowsNew => "rows_new",
    RowsStale => "rows_stale",
    RowsSuspect => "rows_suspect_invalid",
    RowsPictures => "rows_picture_invalid",
    RowsOther => "rows_other_invalid",
    RowFractional => "rows_fractional_refused",
    A2Scaled => "a2_scaled",
    A2Zero => "a2_glyph_zero_interval",
    A2Eligible => "a2_eligible",
    ImagePatches => "image_slot_patches",
    TargetsAllocated => "targets_allocated",
    TargetBlocked => "target_reuse_blocked",
    TargetCandidatesBlocked => "target_candidates_incomplete",
    TargetsRetired => "targets_retired",
    TargetsRetained => "targets_retained",
    TargetBytes => "target_bytes",
    TargetsBusy => "targets_incomplete",
    TargetsReserved => "targets_reserved_this_frame",
    TargetsIdle => "targets_idle",
    TargetIdleBytes => "target_idle_bytes",
    ClipMasks => "clip_masks_built",
    ShapeHits => "shape_cache_hits",
    ShapeMisses => "shape_cache_misses",
    MeasureHits => "measure_cache_hits",
    MeasureMisses => "measure_cache_misses",
}

const COUNT: usize = NAMES.len();
#[derive(Clone)]
pub struct Snapshot {
    pub values: [u64; COUNT],
    pub high_water: [u64; COUNT],
    pub updates: u64,
    pub displayed: u64,
}
thread_local! {
    static COUNTERS: RefCell<Snapshot> = const { RefCell::new(Snapshot {
        values: [0; COUNT], high_water: [0; COUNT], updates: 0, displayed: 0,
    }) };
}
static DISPLAYED: AtomicU64 = AtomicU64::new(0);
static SUBMITTED: AtomicU64 = AtomicU64::new(0);
static REFRESH_MILLIHZ: AtomicU64 = AtomicU64::new(0);
static ENABLED: LazyLock<bool> = LazyLock::new(|| std::env::var_os("EXACT_INK_COUNTERS").is_some());

#[inline]
pub fn enabled() -> bool {
    *ENABLED
}

#[inline]
pub fn add(counter: Counter, n: u64) {
    if enabled() {
        COUNTERS.with_borrow_mut(|s| {
            let i = counter as usize;
            s.values[i] += n;
            s.high_water[i] = s.high_water[i].max(s.values[i]);
            s.updates += 1;
        });
    }
}

pub fn gauge(counter: Counter, n: u64) {
    if enabled() {
        COUNTERS.with_borrow_mut(|s| {
            let i = counter as usize;
            s.values[i] = n;
            s.high_water[i] = s.high_water[i].max(n);
            s.updates += 1;
        });
    }
}

/// A drawable callback runs on another thread; it contributes only a shown frame.
pub fn displayed() {
    if enabled() {
        DISPLAYED.fetch_add(1, Ordering::Relaxed);
    }
}

/// Count a present command at submission, which can run on a worker.
pub fn submitted() {
    if enabled() {
        SUBMITTED.fetch_add(1, Ordering::Relaxed);
    }
}

/// The carrier thread owns counters other than submitted and displayed frames.
pub fn snapshot() -> Option<Snapshot> {
    enabled().then(|| {
        let mut s = COUNTERS.with_borrow(Clone::clone);
        s.displayed = DISPLAYED.load(Ordering::Relaxed);
        let submitted = SUBMITTED.load(Ordering::Relaxed);
        s.values[Counter::Submitted as usize] = submitted;
        s.high_water[Counter::Submitted as usize] = submitted;
        s
    })
}

pub const WHEEL: u8 = 1;
pub const LINK: u8 = 2;
pub const WORKER: u8 = 4;
pub const TIMER: u8 = 8;
pub const AGENT: u8 = 16;
pub const WINDOW: u8 = 32;
pub const DRAW: u8 = 64;

/// Nominal monitor periods also advance while the app's display link is stopped.
pub fn refresh_rate(millihz: u32) {
    if enabled() {
        REFRESH_MILLIHZ.store(u64::from(millihz), Ordering::Relaxed);
    }
}

fn advance_period() {
    if !enabled() {
        return;
    }
    let hz = REFRESH_MILLIHZ.load(Ordering::Relaxed);
    if hz == 0 {
        return;
    }
    thread_local! {
        static PERIOD: RefCell<(std::time::Instant, u64)> = RefCell::new((std::time::Instant::now(), 0));
    }
    PERIOD.with_borrow_mut(|(start, previous)| {
        let current = (start.elapsed().as_nanos() * u128::from(hz) / 1_000_000_000_000) as u64;
        if current != *previous {
            add(Counter::DisplayPeriods, current.saturating_sub(*previous));
            gauge(Counter::PassesInPeriod, 0);
            *previous = current;
        }
    });
}

/// A pass can follow several deliveries on the same loop turn; keep that overlap.
pub fn service(causes: u8) {
    advance_period();
    add(Counter::ServiceTotal, 1);
    add(Counter::PassesInPeriod, 1);
    if causes == 0 {
        add(Counter::ServiceDispatch, 1);
    }
    for (bit, counter) in [
        (WHEEL, Counter::ServiceWheel),
        (LINK, Counter::ServiceLink),
        (WORKER, Counter::ServiceWorker),
        (TIMER, Counter::ServiceTimer),
        (AGENT, Counter::ServiceAgent),
        (WINDOW, Counter::ServiceWindow),
        (DRAW, Counter::ServiceDraw),
    ] {
        if causes & bit != 0 {
            add(counter, 1);
        }
    }
}

pub extern "C" fn probe_name(index: usize) -> *const core::ffi::c_char {
    C_NAMES
        .get(index)
        .map_or(std::ptr::null(), |n| n.as_ptr().cast())
}

pub extern "C" fn probe_value(index: usize, high: bool) -> u64 {
    if index == Counter::Submitted as usize {
        return SUBMITTED.load(Ordering::Relaxed);
    }
    if index == COUNT {
        return DISPLAYED.load(Ordering::Relaxed);
    }
    COUNTERS.with_borrow(|s| {
        (if high { &s.high_water } else { &s.values })
            .get(index)
            .copied()
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "measurement: EXACT_INK_COUNTERS=1; retain stdout"]
    fn update_cost() {
        let start = std::time::Instant::now();
        for _ in 0..1_000_000 {
            add(std::hint::black_box(Counter::PaintAttempts), 1);
        }
        let elapsed = start.elapsed();
        if let Some(s) = snapshot() {
            assert_eq!(s.values[Counter::PaintAttempts as usize], 1_000_000);
            assert_eq!(s.high_water[Counter::PaintAttempts as usize], 1_000_000);
            gauge(Counter::TargetsRetained, 10);
            gauge(Counter::TargetsRetained, 2);
            let s = snapshot().unwrap();
            assert_eq!(s.values[Counter::TargetsRetained as usize], 2);
            assert_eq!(s.high_water[Counter::TargetsRetained as usize], 10);
        }
        println!(
            "counter updates: enabled={}, count=1000000, elapsed_ns={}",
            enabled(),
            elapsed.as_nanos()
        );
    }
}
