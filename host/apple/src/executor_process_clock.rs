//! The clock a child's supervision keeps (LLP 1133.007 §7): the wall's, or
//! under the agent the agent's, which moves only when the drive moves it.
//! Each supervisor waits for its next deadline (a restart's delay, a
//! readiness probe, the drain, the kill's escalation, a script's next step)
//! as a waiter here. On the agent's clock an advance wakes every waiter due
//! by then and returns once each has done its work and waits again, so a
//! `clock +500` sees what was due by then, and `clock settle` reaches each
//! deadline in turn.
use std::collections::BTreeMap;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long an advance waits for the work it woke: real children answer on
/// the wall, and a waiter that never returns must not hold the agent. A
/// supervisor blocked on delivery also holds it until this bound: a script
/// step that writes more than the runner's 1 MiB of undelivered output
/// waits for the runner to take some, which it does only after the advance
/// returns, so such a step costs the drive up to 3 s of wall time.
const ADVANCE_BOUND: Duration = Duration::from_secs(3);

/// One supervisor's wait.
struct Waiter {
    /// Its next deadline, in this clock's milliseconds; `None` waits for input alone.
    deadline: Option<f64>,
    /// Woken by an advance, or just joined, and not waiting (again) yet:
    /// an advance waits for it, so one made once a child's `spawn` landed
    /// sees what that child's first turn did.
    running: bool,
    /// What wakes it: a tick into its input.
    wake: Box<dyn Fn() + Send>,
}

#[derive(Default)]
struct State {
    /// The agent's time, in milliseconds.
    now: f64,
    waiters: BTreeMap<u64, Waiter>,
    next: u64,
}

/// A waiter's place on the clock, given up when dropped: after its
/// stream's last outcome is queued, so an advance that woke it waits for it.
pub(crate) struct Leave(pub(crate) std::sync::Arc<Clock>, pub(crate) u64);

impl Drop for Leave {
    fn drop(&mut self) {
        self.0.leave(self.1);
    }
}

/// A clock for supervision: the wall's, or the agent's.
pub(crate) struct Clock {
    agent: bool,
    started: Instant,
    state: Mutex<State>,
    changed: Condvar,
}

impl Clock {
    /// The wall's clock, in milliseconds since the executor started.
    pub(crate) fn wall() -> Clock {
        Clock::new(false)
    }

    /// The agent's clock, which [`Clock::advance`] moves.
    pub(crate) fn agent() -> Clock {
        Clock::new(true)
    }

    fn new(agent: bool) -> Clock {
        Clock {
            agent,
            started: Instant::now(),
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    /// Whether the agent's clock governs.
    pub(crate) fn is_agent(&self) -> bool {
        self.agent
    }

    /// Now, in this clock's milliseconds.
    pub(crate) fn now(&self) -> f64 {
        if self.agent {
            self.state.lock().unwrap().now
        } else {
            self.started.elapsed().as_secs_f64() * 1000.0
        }
    }

    /// A new waiter, woken through `wake` when an advance reaches its
    /// deadline; its id.
    pub(crate) fn join(&self, wake: Box<dyn Fn() + Send>) -> u64 {
        let mut state = self.state.lock().unwrap();
        state.next += 1;
        let id = state.next;
        state.waiters.insert(
            id,
            Waiter {
                deadline: None,
                running: true,
                wake,
            },
        );
        id
    }

    /// Waiter `id` has done what was due and waits for `deadline` now.
    pub(crate) fn wait_for(&self, id: u64, deadline: Option<f64>) {
        let mut state = self.state.lock().unwrap();
        if let Some(waiter) = state.waiters.get_mut(&id) {
            waiter.deadline = deadline;
            waiter.running = false;
        }
        self.changed.notify_all();
    }

    /// Waiter `id` is gone: its stream ended.
    pub(crate) fn leave(&self, id: u64) {
        self.state.lock().unwrap().waiters.remove(&id);
        self.changed.notify_all();
    }

    /// How long a wall-clock wait for `deadline` lasts; on the agent's
    /// clock, a short poll, since an advance wakes it.
    pub(crate) fn timeout(&self, deadline: Option<f64>) -> Duration {
        match (self.agent, deadline) {
            (false, Some(at)) => Duration::from_secs_f64(((at - self.now()) / 1000.0).max(0.0)),
            _ => Duration::from_millis(250),
        }
    }

    /// Whether `deadline` has come.
    pub(crate) fn passed(&self, deadline: Option<f64>) -> bool {
        deadline.is_some_and(|at| at <= self.now())
    }

    /// The earliest deadline any waiter keeps: when the agent's clock next
    /// has work.
    pub(crate) fn due(&self) -> Option<f64> {
        let state = self.state.lock().unwrap();
        (state.waiters.values())
            .filter_map(|w| w.deadline)
            .reduce(f64::min)
    }

    /// The agent's clock reads `now_ms`: wake every waiter due by then and
    /// return once each waits again (or the bound passes), in rounds, since
    /// work one does may make another due.
    pub(crate) fn advance(&self, now_ms: f64) {
        if !self.agent {
            return;
        }
        let bound = Instant::now() + ADVANCE_BOUND;
        let mut state = self.state.lock().unwrap();
        if now_ms > state.now {
            state.now = now_ms;
        }
        loop {
            let now = state.now;
            let mut woke = false;
            for waiter in state.waiters.values_mut() {
                if !waiter.running && waiter.deadline.is_some_and(|at| at <= now) {
                    waiter.running = true;
                    waiter.deadline = None;
                    (waiter.wake)();
                    woke = true;
                }
            }
            let running = state.waiters.values().any(|w| w.running);
            if !running && !woke {
                return;
            }
            let left = bound.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            state = self.changed.wait_timeout(state, left).unwrap().0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn an_advance_runs_what_is_due_and_returns_when_it_waits_again() {
        let clock = Arc::new(Clock::agent());
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let id = clock.join(Box::new(move || {
            let _ = tx.send(());
        }));
        clock.wait_for(id, Some(500.0));
        assert_eq!(clock.due(), Some(500.0));
        let worker = {
            let clock = clock.clone();
            std::thread::spawn(move || {
                rx.recv().unwrap();
                // Its work, then its next deadline.
                clock.wait_for(id, Some(1500.0));
                rx.recv().unwrap();
                clock.wait_for(id, None);
            })
        };
        clock.advance(100.0);
        assert_eq!(clock.due(), Some(500.0), "nothing was due by 100");
        clock.advance(600.0);
        assert_eq!(clock.due(), Some(1500.0));
        clock.advance(2000.0);
        assert_eq!(clock.due(), None);
        worker.join().unwrap();
        assert_eq!(clock.now(), 2000.0);
    }

    #[test]
    fn an_advance_waits_for_a_waiter_that_has_not_waited_yet() {
        // A supervisor joins, then does its first turn (a script's steps
        // at 0) before it first waits.
        let clock = Arc::new(Clock::agent());
        let id = clock.join(Box::new(|| {}));
        let turned = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker = {
            let (clock, turned) = (clock.clone(), turned.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                turned.store(true, std::sync::atomic::Ordering::Release);
                clock.wait_for(id, Some(100.0));
            })
        };
        clock.advance(0.0);
        assert!(turned.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(clock.due(), Some(100.0));
        worker.join().unwrap();
    }

    #[test]
    fn the_wall_clock_ignores_advances() {
        let clock = Clock::wall();
        clock.advance(1e9);
        assert!(clock.now() < 1e6);
        assert!(!clock.is_agent());
    }
}
