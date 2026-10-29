//! Hardware prepares before configuration; WARP starts only after failure or
//! the measured startup bound. @ref LLP 1149 D6.3, S2.9.
use super::*;
use std::sync::{mpsc, Arc};

// NucBox S2.8b: a driver-cache-cold screen displayed at 251–255 ms;
// the 100 ms bound opened WARP unnecessarily and cost up to 380 ms displayed.
const STARTUP_DEVICE_BUDGET: Duration = Duration::from_millis(300);

type Arrival = (bool, Option<Result<Prepared>>);

pub struct Startup {
    receive: mpsc::Receiver<Arrival>,
    workers: Vec<std::thread::JoinHandle<()>>,
    pending: [bool; 2],
    force_warp: bool,
    deadline: Instant,
}

impl Startup {
    pub fn start(force_warp: bool) -> Result<Self> {
        let now = Instant::now();
        let age = Duration::from_secs_f64(super::super::process_ms()? / 1000.);
        Self::start_with(
            force_warp,
            now + STARTUP_DEVICE_BUDGET.saturating_sub(age),
            Prepared::open,
        )
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    fn start_with(
        force_warp: bool,
        warp_at: Instant,
        open: impl Fn(bool) -> Result<Prepared> + Send + Sync + 'static,
    ) -> Result<Self> {
        let (send, receive) = mpsc::channel();
        let (schedule, wait) = mpsc::channel();
        let open = Arc::new(open);
        let mut workers = Vec::new();
        let primary = open.clone();
        let arrived = send.clone();
        workers.push(
            std::thread::Builder::new()
                .name("ink-device".into())
                .spawn(move || {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        primary(force_warp)
                    }))
                    .unwrap_or_else(|_| Err(Error::Device("device worker panicked".into())));
                    let success = result.is_ok();
                    let _ = arrived.send((force_warp, Some(result)));
                    let _ = schedule.send(success);
                })
                .map_err(|e| Error::Device(e.to_string()))?,
        );
        if !force_warp {
            workers.push(
                std::thread::Builder::new()
                    .name("ink-startup-warp".into())
                    .spawn(move || {
                        let result = if wait
                            .recv_timeout(warp_at.saturating_duration_since(Instant::now()))
                            == Ok(true)
                        {
                            None
                        } else {
                            Some(
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    open(true)
                                }))
                                .unwrap_or_else(|_| {
                                    Err(Error::Device("WARP worker panicked".into()))
                                }),
                            )
                        };
                        let _ = send.send((true, result));
                    })
                    .map_err(|e| Error::Device(e.to_string()))?,
            );
        }
        Ok(Self {
            receive,
            workers,
            pending: [!force_warp, true],
            force_warp,
            deadline: warp_at,
        })
    }

    pub fn force_warp(&self) -> bool {
        self.force_warp
    }
    pub fn pending(&self) -> bool {
        self.pending.iter().any(|p| *p)
    }
    pub fn hardware_pending(&self) -> bool {
        self.pending[0]
    }

    pub fn poll(&mut self) -> Option<(bool, Result<D3d11>)> {
        self.wait_until(Instant::now())
    }

    pub fn wait_until(&mut self, deadline: Instant) -> Option<(bool, Result<D3d11>)> {
        while self.pending() {
            match self
                .receive
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok((warp, result)) => {
                    self.pending[usize::from(warp)] = false;
                    if let Some(result) = result {
                        return Some((warp, result.and_then(D3d11::from_prepared)));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let warp = !self.pending[0];
                    self.pending[usize::from(warp)] = false;
                    return Some((
                        warp,
                        Err(Error::Device("startup worker disconnected".into())),
                    ));
                }
            }
        }
        None
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn both_worker_refusals_are_delivered() {
        let mut startup = Startup::start_with(false, Instant::now(), |warp| {
            Err(Error::Device(if warp { "WARP" } else { "hardware" }.into()))
        })
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut failures = Vec::new();
        while let Some((warp, result)) = startup.wait_until(deadline) {
            assert!(result.is_err());
            failures.push(warp);
        }
        failures.sort();
        assert_eq!(failures, [false, true]);
        assert!(!startup.pending());
    }

    #[test]
    fn warp_starts_at_the_deadline_without_a_waiter() {
        let (send, receive) = mpsc::channel();
        let began = Instant::now();
        let startup = Startup::start_with(false, began + Duration::from_millis(75), move |warp| {
            if warp {
                send.send(Instant::now()).unwrap();
            } else {
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(Error::Device("refused".into()))
        })
        .unwrap();
        let at = receive.recv_timeout(Duration::from_millis(250)).unwrap();
        assert!(at >= began + Duration::from_millis(75));
        drop(startup);
    }

    #[test]
    fn hardware_at_eighty_ms_wins_while_warp_is_pending() {
        let prepared = Mutex::new(Some(Prepared::open(true).unwrap()));
        let (release, wait) = mpsc::channel::<()>();
        let wait = Mutex::new(wait);
        let began = Instant::now();
        let mut startup =
            Startup::start_with(false, began + Duration::from_millis(75), move |warp| {
                if warp {
                    let _ = wait.lock().unwrap().recv_timeout(Duration::from_secs(2));
                    Err(Error::Device("unused WARP".into()))
                } else {
                    std::thread::sleep(Duration::from_millis(80));
                    Ok(prepared.lock().unwrap().take().unwrap())
                }
            })
            .unwrap();
        let arrived = startup.wait_until(began + Duration::from_millis(100));
        drop(release);
        let (warp, result) = arrived.expect("hardware must wake the bound's wait");
        assert!(!warp);
        assert!(result.is_ok());
        assert!(startup.pending());
        assert!(
            !startup.hardware_pending(),
            "late WARP must not gate hardware recovery"
        );
    }
}
