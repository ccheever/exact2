//! Opt-in cumulative observations of the live painter. @ref LLP 1149.001 Stage 0.
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Once};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
    Displayed => "displayed_frames",
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
    WheelInPeriod => "period_wheel",
    LinkInPeriod => "period_link",
    WorkerInPeriod => "period_worker",
    TimerInPeriod => "period_timer",
    AgentInPeriod => "period_agent",
    WindowInPeriod => "period_window",
    DispatchInPeriod => "period_dispatch",
    DrawInPeriod => "period_draw",
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
    TargetPartitionAt => "target_partition_unix_us",
    TargetPartitionCompleted => "target_partition_completed",
    ClipMasks => "clip_masks_built",
    GlyphUploadBatches => "glyph_upload_batches",
    GlyphUploadCopies => "glyph_upload_copies",
    GlyphUploadBytes => "glyph_upload_bytes",
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
    pub submitted_available: bool,
    pub displayed_available: bool,
}
thread_local! {
    static COUNTERS: RefCell<Snapshot> = const { RefCell::new(Snapshot {
        values: [0; COUNT], high_water: [0; COUNT], updates: 0, displayed: 0,
        submitted_available: false, displayed_available: false,
    }) };
}
static DISPLAYED: AtomicU64 = AtomicU64::new(0);
static SUBMITTED: AtomicU64 = AtomicU64::new(0);
static REFRESH_MILLIHZ: AtomicU64 = AtomicU64::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static CLOCK: LazyLock<(Instant, f64)> = LazyLock::new(|| {
    (
        Instant::now(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            * 1e6,
    )
});

/// Called at carrier/device startup, before any frame or worker observation.
pub fn initialize() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        ENABLED.store(
            std::env::var_os("EXACT_INK_COUNTERS").is_some(),
            Ordering::Relaxed,
        );
        if enabled() {
            LazyLock::force(&CLOCK);
        }
    });
}

/// All observation timestamps share one wall/monotonic anchor.
pub fn timestamp(at: Instant) -> f64 {
    let (start, unix_us) = *CLOCK;
    if let Some(elapsed) = at.checked_duration_since(start) {
        unix_us + elapsed.as_secs_f64() * 1e6
    } else {
        unix_us - start.duration_since(at).as_secs_f64() * 1e6
    }
}

/// Capability of the attached presentation path, not the operating system.
pub fn presentation_support(submitted: bool, displayed: bool) {
    observe(|s| {
        s.submitted_available = submitted;
        s.displayed_available = displayed;
    });
}

#[inline]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

impl Snapshot {
    pub fn add(&mut self, counter: Counter, n: u64) {
        let i = counter as usize;
        self.gauge(counter, self.values[i] + n);
    }

    pub fn gauge(&mut self, counter: Counter, n: u64) {
        let i = counter as usize;
        self.values[i] = n;
        self.high_water[i] = self.high_water[i].max(n);
        self.updates += 1;
    }
}

/// Compound observations pay one enabled load before computing their inputs.
#[inline]
pub fn observe<R>(f: impl FnOnce(&mut Snapshot) -> R) -> Option<R> {
    if !enabled() {
        return None;
    }
    Some(COUNTERS.with_borrow_mut(f))
}

#[inline]
pub fn add(counter: Counter, n: u64) {
    observe(|s| s.add(counter, n));
}

pub fn gauge(counter: Counter, n: u64) {
    observe(|s| s.gauge(counter, n));
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
        s.values[Counter::Displayed as usize] = s.displayed;
        s.high_water[Counter::Displayed as usize] = s.displayed;
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

const PERIOD_CAUSES: &[(u8, Counter, Counter)] = &[
    (WHEEL, Counter::ServiceWheel, Counter::WheelInPeriod),
    (LINK, Counter::ServiceLink, Counter::LinkInPeriod),
    (WORKER, Counter::ServiceWorker, Counter::WorkerInPeriod),
    (TIMER, Counter::ServiceTimer, Counter::TimerInPeriod),
    (AGENT, Counter::ServiceAgent, Counter::AgentInPeriod),
    (WINDOW, Counter::ServiceWindow, Counter::WindowInPeriod),
    (DRAW, Counter::ServiceDraw, Counter::DrawInPeriod),
];

/// Nominal monitor periods also advance while the app's display link is stopped.
pub fn refresh_rate(millihz: u32) {
    if enabled() {
        REFRESH_MILLIHZ.store(u64::from(millihz), Ordering::Relaxed);
    }
}

fn advance_period(s: &mut Snapshot) {
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
            s.add(Counter::DisplayPeriods, current.saturating_sub(*previous));
            s.gauge(Counter::PassesInPeriod, 0);
            s.gauge(Counter::DispatchInPeriod, 0);
            for &(_, _, period) in PERIOD_CAUSES {
                s.gauge(period, 0);
            }
            *previous = current;
        }
    });
}

/// A pass can follow several deliveries on the same loop turn; keep that overlap.
pub fn service(causes: u8) {
    if !enabled() {
        return;
    }
    COUNTERS.with_borrow_mut(|s| {
        advance_period(s);
        record_service(s, causes);
    });
}

fn record_service(s: &mut Snapshot, causes: u8) {
    s.add(Counter::ServiceTotal, 1);
    s.add(Counter::PassesInPeriod, 1);
    if causes == 0 {
        s.add(Counter::ServiceDispatch, 1);
        s.add(Counter::DispatchInPeriod, 1);
    }
    for &(bit, counter, period) in PERIOD_CAUSES {
        if causes & bit != 0 {
            s.add(counter, 1);
            s.add(period, 1);
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
        return COUNTERS.with_borrow(|s| {
            if s.submitted_available {
                SUBMITTED.load(Ordering::Relaxed)
            } else {
                u64::MAX
            }
        });
    }
    if index == Counter::Displayed as usize {
        return COUNTERS.with_borrow(|s| {
            if s.displayed_available {
                DISPLAYED.load(Ordering::Relaxed)
            } else {
                u64::MAX
            }
        });
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
    fn draw_and_combined_deliveries_are_not_dispatch() {
        let mut s = Snapshot {
            values: [0; COUNT],
            high_water: [0; COUNT],
            updates: 0,
            displayed: 0,
            submitted_available: false,
            displayed_available: false,
        };
        record_service(&mut s, DRAW);
        record_service(&mut s, DRAW | AGENT | LINK);
        assert_eq!(s.values[Counter::ServiceTotal as usize], 2);
        assert_eq!(s.values[Counter::ServiceDraw as usize], 2);
        assert_eq!(s.values[Counter::ServiceAgent as usize], 1);
        assert_eq!(s.values[Counter::ServiceLink as usize], 1);
        assert_eq!(s.values[Counter::ServiceDispatch as usize], 0);
        record_service(&mut s, 0);
        assert_eq!(s.values[Counter::ServiceDispatch as usize], 1);
    }

    #[test]
    fn disabled_compound_observations_do_not_evaluate_their_inputs() {
        initialize();
        if !enabled() {
            observe(|_| panic!("disabled observation evaluated"));
            service(u8::MAX);
            COUNTERS.with_borrow(|s| assert_eq!(s.updates, 0));
        }
    }

    #[test]
    fn timestamps_keep_monotonic_differences() {
        initialize();
        let start = Instant::now();
        assert_eq!(
            timestamp(start + std::time::Duration::from_millis(125)) - timestamp(start),
            125_000.
        );
    }

    #[test]
    #[ignore = "measurement: EXACT_INK_COUNTERS=1; retain stdout"]
    fn update_cost() {
        initialize();
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
