//! Per-task Windows timers; the OS owns each tolerance window and repeat schedule.
#![allow(unsafe_code)]
use exact_runner::NativeTimer;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::Threading::{
        CloseThreadpoolTimer, CreateThreadpoolTimer, SetThreadpoolTimerEx,
        WaitForThreadpoolTimerCallbacks, PTP_CALLBACK_INSTANCE, PTP_TIMER,
    },
};

type Wake = Arc<dyn Fn(Arc<Fired>) + Send + Sync>;

pub(super) struct Timers {
    entries: BTreeMap<u32, Timer>,
    deliveries: Vec<(f64, Arc<Fired>)>,
    epoch: u64,
    wake: Wake,
}

/// A posted callback retains its instance identity after native cancellation.
pub(super) struct Fired {
    pub row: NativeTimer,
    pub epoch: u64,
    active: AtomicBool,
    pending: AtomicBool,
}
impl Fired {
    pub fn live(&self, epoch: u64) -> bool {
        self.epoch == epoch && self.active.load(Ordering::Acquire)
    }
    pub fn complete(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

impl Timers {
    pub fn new(wake: impl Fn(Arc<Fired>) + Send + Sync + 'static) -> Self {
        Self {
            entries: BTreeMap::new(),
            deliveries: Vec::new(),
            epoch: 0,
            wake: Arc::new(wake),
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.deliveries.clear();
    }
    /// Retain an OS callback until the runner's monotonic clock reaches its deadline.
    pub fn defer(&mut self, due: f64, fired: Arc<Fired>) {
        self.deliveries.push((due, fired));
    }
    pub fn next_delivery(&self) -> Option<f64> {
        self.deliveries.iter().map(|(due, _)| *due).reduce(f64::min)
    }
    pub fn take_due(&mut self, now: f64) -> Vec<Arc<Fired>> {
        let mut ready = Vec::new();
        self.deliveries.retain(|(due, fired)| {
            if !fired.live(self.epoch) {
                false
            } else if *due <= now {
                ready.push(fired.clone());
                false
            } else {
                true
            }
        });
        ready
    }
    pub fn update(&mut self, epoch: u64, rows: Vec<NativeTimer>, now: f64) -> Result<(), String> {
        if self.epoch != epoch {
            self.clear();
            self.epoch = epoch;
        }
        let rows: BTreeMap<_, _> = rows.into_iter().map(|row| (row.index, row)).collect();
        self.entries.retain(|index, timer| {
            rows.get(index)
                .is_some_and(|row| row.generation == timer.context.fired.row.generation)
        });
        for (index, row) in rows {
            if self.entries.contains_key(&index) {
                continue;
            }
            let timer = Timer::new(row, epoch, now, self.wake.clone())?;
            self.entries.insert(index, timer);
        }
        self.deliveries.retain(|(_, fired)| fired.live(self.epoch));
        Ok(())
    }
}

struct Context {
    fired: Arc<Fired>,
    wake: Wake,
}
struct Timer {
    handle: PTP_TIMER,
    context: Box<Context>,
}
impl Timer {
    fn new(row: NativeTimer, epoch: u64, now: f64, wake: Wake) -> Result<Self, String> {
        let ticks = relative_due(row.due_ms, now)? as u64;
        let due = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let period = if row.once { 0 } else { row.interval_ms };
        let tolerance = row.tolerance_ms;
        let mut context = Box::new(Context {
            fired: Arc::new(Fired {
                row,
                epoch,
                active: AtomicBool::new(true),
                pending: AtomicBool::new(false),
            }),
            wake,
        });
        // Keep the context alive until cancellation has joined every callback
        let handle = unsafe {
            CreateThreadpoolTimer(
                Some(fire),
                (&mut *context as *mut Context).cast(),
                std::ptr::null(),
            )
        };
        if handle == 0 {
            return Err(format!(
                "CreateThreadpoolTimer: {}; restart the application to retry",
                std::io::Error::last_os_error()
            ));
        }
        // FALSE reports the previous timer state, not an error; the new timer is armed
        unsafe { SetThreadpoolTimerEx(handle, &due, period, tolerance) };
        Ok(Self { handle, context })
    }
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.context.fired.active.store(false, Ordering::Release);
        // Stop new callbacks, cancel queued work, and join running callbacks before freeing context
        unsafe {
            SetThreadpoolTimerEx(self.handle, std::ptr::null(), 0, 0);
            WaitForThreadpoolTimerCallbacks(self.handle, 1);
            CloseThreadpoolTimer(self.handle);
        }
    }
}

unsafe extern "system" fn fire(_: PTP_CALLBACK_INSTANCE, context: *mut c_void, _: PTP_TIMER) {
    // The native timer owns this allocation until Drop joins all callback threads
    let context = unsafe { &*(context as *const Context) };
    let fired = &context.fired;
    if fired.active.load(Ordering::Acquire) && !fired.pending.swap(true, Ordering::AcqRel) {
        (context.wake)(fired.clone());
    }
}

fn relative_due(due: f64, now: f64) -> Result<i64, String> {
    let ticks = ((due - now).max(0.0) * 10_000.0).ceil();
    if !due.is_finite() || !now.is_finite() || ticks >= i64::MAX as f64 {
        return Err(
            "native timer deadline is outside Windows' range; restart with a valid live clock"
                .into(),
        );
    }
    Ok(-(ticks as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    fn row(index: u32, generation: u64) -> NativeTimer {
        NativeTimer {
            index,
            generation,
            due_ms: 3_600_000.0,
            interval_ms: 1000,
            tolerance_ms: 100,
            once: false,
        }
    }
    fn signal(timer: &Timer) {
        // Drive the production callback at a controlled point, without waiting on wall time
        unsafe {
            fire(
                0,
                (&*timer.context as *const Context).cast_mut().cast(),
                timer.handle,
            )
        }
    }
    #[test]
    fn deadline_conversion_rounds_up_and_accepts_the_full_authored_interval() {
        for (due, now, expected) in [
            (0.0, 0.0, 0),
            (1.0, 0.0, -10_000),
            (1.00001, 1.0, -1),
            (10.0, 11.0, 0),
            (u32::MAX as f64, 0.0, -(u32::MAX as i64 * 10_000)),
        ] {
            assert_eq!(relative_due(due, now).unwrap(), expected);
        }
        for invalid in [f64::NAN, f64::INFINITY] {
            assert!(relative_due(invalid, 0.0).is_err());
            assert!(relative_due(1.0, invalid).is_err());
        }
        assert!(relative_due(f64::MAX, 0.0).is_err());
    }
    #[test]
    fn native_instances_repeat_independently_and_cancel_stale_deliveries() {
        let (send, receive) = mpsc::channel();
        let mut timers = Timers::new(move |fired| {
            let _ = send.send(fired);
        });
        let a = row(0, 0);
        let mut b = row(1, 0);
        b.tolerance_ms = 50;
        timers.update(1, vec![a.clone()], 0.0).unwrap();
        let original = timers.entries[&0].handle;
        timers.update(1, vec![a.clone(), b], 40.0).unwrap();
        assert_eq!(timers.entries[&0].handle, original);
        signal(&timers.entries[&1]);
        signal(&timers.entries[&0]);
        signal(&timers.entries[&0]);
        let second = receive.try_recv().unwrap();
        let first = receive.try_recv().unwrap();
        assert_eq!((second.row.index, first.row.index), (1, 0));
        assert_eq!((first.row.tolerance_ms, second.row.tolerance_ms), (100, 50));
        assert!(receive.try_recv().is_err());
        let mut next = a;
        next.due_ms += 1000.0;
        timers.update(1, vec![next], 40.0).unwrap();
        assert!(!second.live(1));
        assert!(first.live(1));
        assert_eq!(timers.entries[&0].handle, original);
        first.complete();
        signal(&timers.entries[&0]);
        assert_eq!(receive.try_recv().unwrap().row.index, 0);
        timers.update(1, vec![row(0, 1)], 40.0).unwrap();
        assert!(!first.live(1));
        let replacement = timers.entries[&0].context.fired.clone();
        timers.update(2, vec![row(0, 1)], 40.0).unwrap();
        assert!(!replacement.live(2));
        let reloaded = timers.entries[&0].context.fired.clone();
        timers.clear();
        assert!(!reloaded.live(2));
    }
    #[test]
    fn windows_callback_reaches_the_owner_channel_and_cancellation_joins_it() {
        let (send, receive) = mpsc::channel();
        let mut timers = Timers::new(move |fired| {
            let _ = send.send(fired);
        });
        let mut immediate = row(0, 0);
        immediate.due_ms = 0.0;
        immediate.once = true;
        immediate.tolerance_ms = 0;
        timers.update(1, vec![immediate], 0.0).unwrap();
        let fired = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("Windows timer callback did not arrive");
        assert!(fired.live(1));
        timers.clear();
        assert!(!fired.live(1));
        assert!(receive.try_recv().is_err());
    }
    #[test]
    fn early_callbacks_keep_their_native_instance_and_cancel_before_delivery() {
        let (send, receive) = mpsc::channel();
        let mut timers = Timers::new(move |fired| {
            let _ = send.send(fired);
        });
        let timer = row(0, 0);
        timers.update(1, vec![timer.clone()], 0.0).unwrap();
        let original = timers.entries[&0].handle;
        signal(&timers.entries[&0]);
        let fired = receive.try_recv().unwrap();
        timers.defer(100.25, fired.clone());
        assert_eq!(timers.next_delivery(), Some(100.25));
        assert!(timers.take_due(100.24).is_empty());
        signal(&timers.entries[&0]);
        assert!(receive.try_recv().is_err());
        let ready = timers.take_due(100.25);
        assert_eq!(ready.len(), 1);
        assert!(Arc::ptr_eq(&ready[0], &fired));
        assert_eq!(timers.entries[&0].handle, original);
        assert_eq!(timers.next_delivery(), None);
        fired.complete();
        signal(&timers.entries[&0]);
        timers.defer(200.0, receive.try_recv().unwrap());
        timers.update(1, vec![row(0, 1)], 100.25).unwrap();
        assert_eq!(timers.next_delivery(), None);
        assert!(timers.take_due(200.0).is_empty());
        signal(&timers.entries[&0]);
        timers.defer(300.0, receive.try_recv().unwrap());
        timers.update(2, vec![timer], 200.0).unwrap();
        assert_eq!(timers.next_delivery(), None);
        assert!(timers.take_due(300.0).is_empty());
    }
}
