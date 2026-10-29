//! One data-module turn's clocks (LLP 1027 §budgets, as amended): the turn
//! is held to its budget in the calling thread's CPU time, the module's own
//! work, with a wall-clock backstop for a turn that waits rather than
//! works. Under CPU load the calling thread is taken off the core mid-turn;
//! a wall-clock budget refused a turn of 60 ms of work that read 247 ms, and
//! the resource kept its old value. Still post hoc, as before: the budget
//! never ends a turn, an interrupt does (LLP 1048.000 D10).

use std::time::{Duration, Instant};

/// How many budgets of wall time a turn may take, whatever its CPU time, before it is refused:
/// a host-door call that blocks (an Apple hosted `native.call` waits on the main thread) costs no
/// CPU here, and stays bounded.
pub(crate) const WALL_BACKSTOP_BUDGETS: f64 = 10.0;

/// A turn's start on both clocks.
pub(crate) struct TurnClock {
    wall: Instant,
    cpu: Option<Duration>,
}

/// What a turn took.
pub(crate) struct Took {
    /// The calling thread's CPU time, or the wall time where there is no thread CPU clock.
    pub(crate) own_ms: f64,
    pub(crate) wall_ms: f64,
    /// Whether `own_ms` is CPU time.
    pub(crate) cpu: bool,
}

impl TurnClock {
    pub(crate) fn start() -> TurnClock {
        TurnClock {
            cpu: thread_cpu(),
            wall: Instant::now(),
        }
    }

    pub(crate) fn took(&self) -> Took {
        let wall_ms = self.wall.elapsed().as_secs_f64() * 1e3;
        match (self.cpu, thread_cpu()) {
            (Some(a), Some(b)) => Took {
                own_ms: b.saturating_sub(a).as_secs_f64() * 1e3,
                wall_ms,
                cpu: true,
            },
            _ => Took {
                own_ms: wall_ms,
                wall_ms,
                cpu: false,
            },
        }
    }
}

impl TurnClock {
    /// What the turn took against `budget_ms` (`Took::over`); a turn within it that still held a
    /// thread the UI waits on (`main_thread`: `Main` placement) past the budget on the wall clock
    /// is said in `journal`, beside the slow-load line, never refused (theme2's 15:57 ruling: a
    /// refusal after the fact shortens no freeze).
    pub(crate) fn judge(
        &self,
        budget_ms: f64,
        deadline: bool,
        main_thread: bool,
        source: &str,
        journal: &mut Vec<String>,
    ) -> Option<String> {
        let took = self.took();
        let over = took.over(budget_ms, deadline);
        if over.is_none() {
            journal.extend(took.held(main_thread, deadline, source, budget_ms));
        }
        over
    }
}

impl Took {
    /// The journal's line for a turn within its budget that held a thread the UI waits on past it.
    fn held(
        &self,
        main_thread: bool,
        deadline: bool,
        source: &str,
        budget_ms: f64,
    ) -> Option<String> {
        (main_thread && !deadline && self.wall_ms > budget_ms).then(|| {
            format!(
                "a data turn held the UI thread {:.0} ms ({:.0} ms of its own): `{source}`",
                self.wall_ms, self.own_ms
            )
        })
    }

    /// Why the turn is over `budget_ms`, or `None` when it is within it: its own time past the
    /// budget, or its wall time past the backstop. `deadline`: the budget is the time left to a
    /// snapshot's deadline, which is wall time, and is held as wall time.
    pub(crate) fn over(&self, budget_ms: f64, deadline: bool) -> Option<String> {
        if deadline {
            return (self.wall_ms > budget_ms).then(|| {
                format!(
                    "took {:.1} ms, over the {budget_ms} ms left to its deadline",
                    self.wall_ms
                )
            });
        }
        // (a zero budget refuses any turn in its own words: a coarse CPU clock can read 0)
        if self.own_ms > budget_ms || (budget_ms <= 0.0 && self.wall_ms > 0.0) {
            let clock = if self.cpu {
                format!(
                    "{:.1} ms of its own (CPU) time ({:.1} ms of wall)",
                    self.own_ms, self.wall_ms
                )
            } else {
                format!("{:.1} ms", self.own_ms)
            };
            return Some(format!(
                "took {clock}, over the {budget_ms} ms budget for one turn"
            ));
        }
        let backstop = budget_ms * WALL_BACKSTOP_BUDGETS;
        (self.wall_ms > backstop).then(|| {
            format!(
                "waited {:.1} ms of wall ({:.1} ms of its own), over the {backstop} ms a turn may take",
                self.wall_ms, self.own_ms
            )
        })
    }
}

/// This thread's CPU time, where the platform keeps one.
#[cfg(unix)]
fn thread_cpu() -> Option<Duration> {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid timespec for the call to fill.
    let read = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    (read == 0).then(|| Duration::new(t.tv_sec as u64, t.tv_nsec as u32))
}

/// This thread's CPU time (kernel and user), in the scheduler's ticks of about 15.6 ms: a turn's
/// CPU time reads in those steps here.
#[cfg(windows)]
fn thread_cpu() -> Option<Duration> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentThread, GetThreadTimes};
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: the current thread's pseudo-handle and four valid FILETIMEs to fill.
    let ok = unsafe {
        GetThreadTimes(
            GetCurrentThread(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    let hundreds = |f: FILETIME| (u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime);
    (ok != 0).then(|| Duration::from_nanos((hundreds(kernel) + hundreds(user)) * 100))
}

#[cfg(not(any(unix, windows)))]
fn thread_cpu() -> Option<Duration> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn took(own_ms: f64, wall_ms: f64) -> Took {
        Took {
            own_ms,
            wall_ms,
            cpu: true,
        }
    }

    #[test]
    fn a_turn_is_held_to_its_own_time_with_a_wall_backstop() {
        // Descheduled: 60 ms of work over 247 ms of wall is within 100 ms.
        assert!(took(60.0, 247.0).over(100.0, false).is_none());
        // Its own time past the budget is refused, whatever the wall.
        assert!(took(130.0, 140.0)
            .over(100.0, false)
            .unwrap()
            .contains("130.0 ms of its own (CPU) time"));
        // Waiting past the backstop is refused, with no CPU to speak of.
        assert!(took(2.0, 1200.0)
            .over(100.0, false)
            .unwrap()
            .contains("over the 1000 ms a turn may take"));
        // A snapshot's deadline is wall time.
        assert!(took(10.0, 300.0).over(250.0, true).is_some());
        assert!(took(10.0, 200.0).over(250.0, true).is_none());
        // A zero budget refuses in its own words even when a coarse clock reads no CPU.
        assert!(took(0.0, 3.0)
            .over(0.0, false)
            .unwrap()
            .contains("over the 0 ms budget for one turn"));
        // An unlimited budget never refuses.
        assert!(took(1e6, 1e7).over(f64::INFINITY, false).is_none());
    }

    #[test]
    fn this_threads_cpu_clock_counts_work_and_not_sleep() {
        let clock = TurnClock::start();
        std::thread::sleep(Duration::from_millis(60));
        let slept = clock.took();
        assert!(slept.wall_ms >= 60.0);
        if slept.cpu {
            assert!(slept.own_ms < 30.0, "{} ms of CPU asleep", slept.own_ms);
        }
        // Work until the clock reads 20 ms of its own, however long the machine takes to give it.
        let clock = TurnClock::start();
        let mut x = 0u64;
        while clock.took().cpu && clock.took().own_ms < 20.0 && clock.took().wall_ms < 30_000.0 {
            x = std::hint::black_box(x.wrapping_mul(6364136223846793005).wrapping_add(1));
        }
        let worked = clock.took();
        if worked.cpu {
            assert!(
                worked.own_ms >= 20.0,
                "{} ms of CPU in {} ms of wall",
                worked.own_ms,
                worked.wall_ms
            );
        }
    }

    #[test]
    fn a_turn_held_past_its_budget_is_said_only_where_the_ui_waits() {
        let held = took(20.0, 180.0);
        assert_eq!(
            held.held(true, false, "work", 100.0).unwrap(),
            "a data turn held the UI thread 180 ms (20 ms of its own): `work`"
        );
        assert!(
            held.held(false, false, "work", 100.0).is_none(),
            "a worker's turn holds no UI"
        );
        assert!(
            held.held(true, true, "work", 100.0).is_none(),
            "a snapshot has its deadline"
        );
        assert!(took(20.0, 60.0).held(true, false, "work", 100.0).is_none());
    }
}
