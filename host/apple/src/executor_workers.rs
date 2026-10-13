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
    use ibex2::stdlib::fetch_async::{BoundedWorkerPool, Job, ReservedWorker, WorkerService};
    pub(in super::super) struct Workers {
        threads: usize,
        pool: Mutex<Option<BoundedWorkerPool>>,
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
                *pool = Some(
                    BoundedWorkerPool::new(self.threads, IN_FLIGHT)
                        .map_err(|error| HostError::Failed(format!("HTTP workers: {error}")))?,
                );
            }
            let inner = pool.as_ref().expect("initialized").reserve()?;
            Ok(Box::new(Reservation {
                inner,
                budget: self.budget.clone(),
            }))
        }
    }
    struct Reservation {
        inner: Box<dyn ReservedWorker>,
        budget: Arc<Budget>,
    }
    impl ReservedWorker for Reservation {
        fn dispatch(self: Box<Self>, job: Job) {
            let Self { inner, budget } = *self;
            inner.dispatch(Box::new(move || {
                // A retired core cannot return these slots while DNS or another
                // accepted blocking job still occupies its worker.
                let _budget = budget;
                job();
            }));
        }
    }
}
