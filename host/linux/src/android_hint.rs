//! Clocks and cores for input. A reader's own
//! exact2 thread answers a tap in a burst of a few milliseconds after
//! sleeping, and an idle phone meets that burst at its lowest clocks (on a
//! Pixel 10 Pro XL a tap's commit and paint ran at 400 MHz, 5-10 ms, so the
//! response missed the frame after the tap). A hint session (Android's ADPF,
//! `APerformanceHint`) over the threads that answer input, told of a
//! workload increase as input arrives, raises their clock floor for that
//! burst, as the platform's own touch boost does for input it delivers; and
//! [`colocate`] wakes the answering thread on the waiting thread's awake
//! core rather than an idle one.
//!
//! Looked up at run time (`dlsym`): `APerformanceHint_*` came with API 33 and
//! the workload notifications with API 36; on older systems this does
//! nothing, or reports a long work duration instead.
#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::Mutex;

type GetManager = unsafe extern "C" fn() -> *mut c_void;
type CreateSession = unsafe extern "C" fn(*mut c_void, *const i32, usize, i64) -> *mut c_void;
type Report = unsafe extern "C" fn(*mut c_void, i64) -> c_int;
type Notify = unsafe extern "C" fn(*mut c_void, bool, bool, *const c_char) -> c_int;

struct Session {
    session: *mut c_void,
    target_ns: i64,
    report: Report,
    increase: Option<Notify>,
}

// SAFETY: the session is only used under `SESSION`'s lock (one call at a
// time, as the NDK asks of a session).
unsafe impl Send for Session {}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

unsafe fn symbol(lib: *mut c_void, name: &CStr) -> *mut c_void {
    // SAFETY: `lib` is a handle from dlopen; `name` is NUL-terminated.
    unsafe { libc::dlsym(lib, name.as_ptr()) }
}

/// Make the hint session over `tids` (this process's threads) with
/// `target_ns` as their work's target duration; whether there is one.
pub fn session(tids: &[i32], target_ns: i64) -> bool {
    let mut slot = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_some() {
        return true;
    }
    // SAFETY: libandroid is already loaded (this library links it); dlopen
    // only takes a reference. Each symbol is cast to its NDK signature.
    unsafe {
        let lib = libc::dlopen(
            c"libandroid.so".as_ptr(),
            libc::RTLD_NOW | libc::RTLD_NOLOAD,
        );
        if lib.is_null() {
            return false;
        }
        let get = symbol(lib, c"APerformanceHint_getManager");
        let create = symbol(lib, c"APerformanceHint_createSession");
        let report = symbol(lib, c"APerformanceHint_reportActualWorkDuration");
        let increase = symbol(lib, c"APerformanceHint_notifyWorkloadIncrease");
        if get.is_null() || create.is_null() || report.is_null() {
            return false;
        }
        let get: GetManager = std::mem::transmute(get);
        let create: CreateSession = std::mem::transmute(create);
        let manager = get();
        if manager.is_null() {
            return false;
        }
        let session = create(manager, tids.as_ptr(), tids.len(), target_ns);
        if session.is_null() {
            return false;
        }
        *slot = Some(Session {
            session,
            target_ns,
            report: std::mem::transmute::<*mut c_void, Report>(report),
            increase: (!increase.is_null())
                .then(|| std::mem::transmute::<*mut c_void, Notify>(increase)),
        });
    }
    true
}

/// Input arrived: the session's threads are about to work. Whether a hint
/// was sent.
pub fn input() -> bool {
    let slot = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = slot.as_ref() else {
        return false;
    };
    // SAFETY: a live session (never closed), used under the lock.
    unsafe {
        match s.increase {
            Some(increase) => increase(s.session, true, false, c"exact input".as_ptr()) == 0,
            // Before API 36: work far over its target raises the floor too.
            None => (s.report)(s.session, s.target_ns * 4) == 0,
        }
    }
}

/// Run thread `tid` (this process's, asleep) on the CPU the calling thread
/// is on, until it widens its own mask again ([`crate::android::fast_cores`]):
/// a thread handed work by one about to wait for it then runs on the
/// waiter's awake core at once, instead of waking an idle core (~1 ms on a
/// Pixel 10 Pro XL between the post and the work starting). Whether it was
/// set (not when the caller is on a core of the slowest cluster).
pub fn colocate(tid: i32) -> bool {
    // SAFETY: sched_getcpu reads this thread's CPU; a zeroed cpu_set_t is the
    // empty set and CPU_SET writes one index below CPU_SETSIZE.
    unsafe {
        let cpu = libc::sched_getcpu();
        // Only onto a fast core: the waiter on a little core leaves the
        // thread where the scheduler wakes it.
        if cpu < 0 || !crate::android::fast_set().contains(&(cpu as usize)) {
            return false;
        }
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu as usize, &mut set);
        libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0
    }
}

/// Run thread `tid` only on the fast cores (`fast`), or anywhere again:
/// a thread that sleeps through a press wakes for the release on a little
/// core, where [`colocate`] declines. Whether the mask was set.
pub fn fast_thread(tid: i32, fast: bool) -> bool {
    let set = crate::android::fast_set();
    if set.is_empty() {
        return false;
    }
    // SAFETY: a zeroed cpu_set_t is the empty set; CPU_SET writes indexes below CPU_SETSIZE.
    unsafe {
        let mut mask: libc::cpu_set_t = std::mem::zeroed();
        if fast {
            for &cpu in set {
                libc::CPU_SET(cpu, &mut mask);
            }
        } else {
            for cpu in 0..libc::CPU_SETSIZE as usize {
                libc::CPU_SET(cpu, &mut mask);
            }
        }
        libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &mask) == 0
    }
}
