//! Workers for host tasks.
//!
//! An async host operation ran on a thread spawned for it and joined after —
//! tens of microseconds of the round trip a one-byte `fs.readFile` paid, and
//! one thread per in-flight request. This pool keeps a few workers alive,
//! grows on demand when every worker is busy — a burst of parallel fetches
//! must not serialize behind a fixed count, since a transport blocks its
//! worker while the request is in flight — and lets the extra workers go
//! after a quiet spell. Process-wide: a job carries its own runtime state.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Pool {
    sender: Mutex<Sender<Job>>,
    receiver: Arc<Mutex<Receiver<Job>>>,
    /// Workers waiting for a job.
    idle: AtomicUsize,
    /// Workers alive.
    total: AtomicUsize,
    pending: AtomicUsize,
    max: usize,
}

/// Workers kept alive through a quiet spell, so the common case never spawns.
const KEEP: usize = 2;
/// The most workers a burst may grow to.
const MAX: usize = 64;
/// How long an extra worker waits for work before leaving.
const IDLE: Duration = Duration::from_secs(10);

static POOL: OnceLock<Pool> = OnceLock::new();
static BODY_POOL: OnceLock<Pool> = OnceLock::new();

fn pool(cell: &'static OnceLock<Pool>, max: usize) -> &'static Pool {
    cell.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        Pool {
            sender: Mutex::new(sender),
            receiver: Arc::new(Mutex::new(receiver)),
            idle: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            pending: AtomicUsize::new(0),
            max,
        }
    })
}

/// Run `job` on a worker, spawning one if none is free and the cap allows.
pub fn run(job: impl FnOnce() + Send + 'static) {
    run_on(pool(&POOL, MAX), job);
}

/// Reserve four workers for body reads: queued opens waiting for native session
/// leases must never occupy every worker that can drain and release those leases.
/// This is the JS host executor only; Rust consumers still choose their executor.
pub fn run_body(job: impl FnOnce() + Send + 'static) {
    run_on(pool(&BODY_POOL, 4), job);
}

fn run_on(pool: &'static Pool, job: impl FnOnce() + Send + 'static) {
    pool.pending.fetch_add(1, Ordering::AcqRel);
    pool.sender
        .lock()
        .expect("pool poisoned")
        .send(Box::new(job))
        .expect("the pool's receiver lives for the process");
    ensure_worker(pool);
}

fn ensure_worker(pool: &'static Pool) {
    if pool.pending.load(Ordering::Acquire) > pool.idle.load(Ordering::Acquire) {
        spawn_worker(pool);
    }
}

fn spawn_worker(pool: &'static Pool) {
    if pool
        .total
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |total| {
            (total < pool.max).then_some(total + 1)
        })
        .is_err()
    {
        return;
    }
    let receiver = Arc::clone(&pool.receiver);
    std::thread::Builder::new()
        .name("ibex2-host-task".into())
        .spawn(move || loop {
            pool.idle.fetch_add(1, Ordering::AcqRel);
            // The lock is held while waiting: the other idle workers queue on
            // it, and the first job goes to whoever holds the receiver.
            let next = receiver.lock().expect("pool poisoned").recv_timeout(IDLE);
            pool.idle.fetch_sub(1, Ordering::AcqRel);
            match next {
                Ok(job) => {
                    pool.pending.fetch_sub(1, Ordering::AcqRel);
                    // A burst may have arrived while this worker still counted
                    // as idle. Recheck before this job can block.
                    ensure_worker(pool);
                    job();
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Quiet: keep a few, let the rest go.
                    if pool.total.load(Ordering::Acquire) > KEEP {
                        pool.total.fetch_sub(1, Ordering::AcqRel);
                        return;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    pool.total.fetch_sub(1, Ordering::AcqRel);
                    return;
                }
            }
        })
        .expect("spawn a host-task worker");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_host_jobs_cannot_starve_body_reads() {
        let wake = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (entered, waiting) = mpsc::channel();
        let (finished, finish) = mpsc::channel();
        let mut all_entered = true;
        for _ in 0..MAX {
            let wake = wake.clone();
            let entered = entered.clone();
            let finished = finished.clone();
            run(move || {
                entered.send(()).unwrap();
                let (lock, changed) = &*wake;
                let guard = lock.lock().unwrap();
                let _guard = changed.wait_while(guard, |released| !*released).unwrap();
                finished.send(()).unwrap();
            });
            if waiting.recv_timeout(Duration::from_secs(5)).is_err() {
                all_entered = false;
                break;
            }
        }
        let (read, observed) = mpsc::channel();
        run_body(move || {
            read.send(()).unwrap();
        });
        let progressed = observed.recv_timeout(Duration::from_secs(2)).is_ok();
        // Always release the workers, even when the assertion will fail.
        let (lock, changed) = &*wake;
        *lock.lock().unwrap() = true;
        changed.notify_all();
        drop(finished);
        for _ in finish {}
        assert!(all_entered, "could not occupy host workers");
        assert!(progressed, "body read starved behind blocked fetch opens");
    }

    #[test]
    fn jobs_run_and_a_burst_does_not_serialize() {
        // Isolate this burst from tests that deliberately saturate the host pool.
        static BURST_POOL: OnceLock<Pool> = OnceLock::new();
        let wake = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (entered, waiting) = mpsc::channel();
        let (finished, finish) = mpsc::channel();
        for _ in 0..12 {
            let wake = wake.clone();
            let entered = entered.clone();
            let finished = finished.clone();
            run_on(pool(&BURST_POOL, MAX), move || {
                let _ = entered.send(());
                let (lock, changed) = &*wake;
                let guard = lock.lock().unwrap();
                let _guard = changed.wait_while(guard, |released| !*released).unwrap();
                let _ = finished.send(());
            });
        }
        drop(entered);
        drop(finished);
        // All twelve must enter before any can finish: direct evidence that
        // the pool grows for blocked work, independent of scheduling speed.
        let all_entered = (0..12).all(|_| waiting.recv_timeout(Duration::from_secs(5)).is_ok());
        let (lock, changed) = &*wake;
        *lock.lock().unwrap() = true;
        changed.notify_all();
        let all_finished = (0..12).all(|_| finish.recv_timeout(Duration::from_secs(5)).is_ok());
        assert!(all_entered, "burst serialized behind blocked jobs");
        assert!(all_finished, "released jobs did not finish");
    }
}
