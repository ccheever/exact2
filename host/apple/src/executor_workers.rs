//! The portable adapter's workers share Exact's physical-worker reservation.
//! @ref LLP 1120#d4-linux-and-the-render-host-the-same-core-over-a-bounded-blocking-adapter — capacity precedes dispatch.
use super::*;

#[derive(Default)]
pub(crate) struct Budget(pub(super) Mutex<Vec<super::super::WorkerSlot>>);

#[cfg(any(not(target_vendor = "apple"), test))]
pub(super) use portable::Workers;

#[cfg(any(not(target_vendor = "apple"), test))]
mod portable {
    use super::*;
    use ibex2::stdlib::fetch_async::{BoundedWorkerPool, ReservedWorker, WorkerService};
    pub(in super::super) struct Workers {
        threads: usize,
        pool: Mutex<Option<Result<BoundedWorkerPool, String>>>,
        budget: Arc<Budget>,
    }
    impl Workers {
        pub(in super::super) fn new(threads: usize, budget: Arc<Budget>) -> Self {
            Self {
                threads,
                pool: Mutex::new(None),
                budget,
            }
        }
    }
    impl WorkerService for Workers {
        fn reserve(&self) -> Result<Box<dyn ReservedWorker>, HostError> {
            // Constructing an unused core starts no worker. This is called off
            // the executor state lock, only after its process slots were reserved.
            let mut pool = self.pool.lock().unwrap();
            if pool.is_none() {
                let mut slots = self.budget.0.lock().unwrap();
                *pool = Some(if slots.len() < self.threads {
                    Err("HTTP worker slots were not reserved".into())
                } else {
                    let first = slots.len() - self.threads;
                    let guards = slots.split_off(first);
                    drop(slots);
                    // Started threads own their slots, including idle retirement
                    // and partial construction failure. A failed pool never retries.
                    BoundedWorkerPool::with_worker_guards(guards, IN_FLIGHT)
                        .map_err(|error| format!("HTTP workers: {error}"))
                });
            }
            pool.as_ref()
                .expect("initialized")
                .as_ref()
                .map_err(|message| HostError::Failed(message.clone()))?
                .reserve()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::super::super::{WorkerSlot, LIVE_WORKERS};
        use super::*;
        use std::sync::{atomic::Ordering, mpsc};
        use std::time::{Duration, Instant};

        #[test]
        fn portable_slots_live_until_worker_exit_and_failed_creation_never_retries() {
            const CHILD: &str = "EXACT_WORKER_LIFETIME_CHILD";
            if std::env::var_os(CHILD).is_none() {
                let name = std::thread::current().name().unwrap().to_owned();
                let status = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([&name, "--exact", "--nocapture", "--test-threads=1"])
                    .env(CHILD, "1")
                    .status()
                    .unwrap();
                assert!(status.success());
                return;
            }
            assert_eq!(LIVE_WORKERS.load(Ordering::Acquire), 0);
            LIVE_WORKERS.fetch_add(2, Ordering::AcqRel);
            let budget = Arc::new(Budget(Mutex::new(vec![WorkerSlot, WorkerSlot])));
            let workers = Workers::new(2, budget.clone());
            let (entered, started) = mpsc::channel();
            let (release, blocked) = mpsc::channel();
            workers.reserve().unwrap().dispatch(Box::new(move || {
                entered.send(()).unwrap();
                blocked.recv().unwrap();
            }));
            started.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(
                budget.0.lock().unwrap().is_empty(),
                "each thread owns its slot"
            );
            drop((workers, budget));
            assert!((1..=2).contains(&LIVE_WORKERS.load(Ordering::Acquire)));
            release.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while LIVE_WORKERS.load(Ordering::Acquire) != 0 {
                assert!(Instant::now() < deadline, "worker slots did not retire");
                std::thread::sleep(Duration::from_millis(10));
            }

            let budget = Arc::new(Budget::default());
            let workers = Workers::new(1, budget.clone());
            assert!(workers.reserve().is_err());
            LIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
            budget.0.lock().unwrap().push(WorkerSlot);
            assert!(
                workers.reserve().is_err(),
                "failed initialization is terminal"
            );
            assert_eq!(budget.0.lock().unwrap().len(), 1, "no second construction");
            drop((workers, budget));
            assert_eq!(LIVE_WORKERS.load(Ordering::Acquire), 0);
        }
    }
}
