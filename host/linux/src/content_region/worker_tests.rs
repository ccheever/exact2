use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

struct Counted(Arc<AtomicUsize>);
impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct Job {
    value: usize,
    release: Option<mpsc::Receiver<()>>,
    owner: Counted,
}
struct Reply {
    value: usize,
    _owner: Counted,
}
struct TestWork(mpsc::Sender<usize>);
impl TextWork for TestWork {
    type Input = Job;
    type Output = Reply;
    fn execute(&mut self, input: Job) -> Reply {
        self.0.send(input.value).unwrap();
        if let Some(release) = input.release {
            release.recv().unwrap();
        }
        Reply {
            value: input.value,
            _owner: input.owner,
        }
    }
}
fn job(value: usize, drops: &Arc<AtomicUsize>) -> Job {
    Job {
        value,
        release: None,
        owner: Counted(drops.clone()),
    }
}
fn start(gate: &ThreadSlot<TestWork>) -> (Port<Job, Reply>, mpsc::Receiver<usize>) {
    let (tx, rx) = mpsc::channel();
    (gate.start(move || TestWork(tx)).unwrap(), rx)
}
fn until(mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < deadline, "worker lifecycle watchdog");
        std::thread::yield_now();
    }
}
fn arrived(rx: &mpsc::Receiver<usize>, value: usize) {
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), value);
}

#[test]
fn held_a_latest_c_replaces_b_without_extra_running_or_completion() {
    let gate = ThreadSlot::default();
    let (port, rx) = start(&gate);
    let drops = Arc::new(AtomicUsize::new(0));
    let (release, wait) = mpsc::channel();
    let mut a = job(1, &drops);
    a.release = Some(wait);
    port.submit(a).unwrap();
    arrived(&rx, 1);
    port.submit(job(2, &drops)).unwrap();
    let c = port.submit(job(3, &drops)).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1, "replaced B actually drops");
    assert_eq!(
        port.counts(),
        Counts {
            running: 1,
            pending: 1,
            completed: 0
        }
    );
    release.send(()).unwrap();
    arrived(&rx, 3);
    until(|| port.counts().completed == 1);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        2,
        "stale A drops before C publishes"
    );
    let (id, reply) = port.take().unwrap();
    assert_eq!((id, reply.value), (c, 3));
    assert!(port.take().is_none());
    drop(reply);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    drop(port);
    until(|| !gate.occupied());
}

#[test]
fn unread_completion_is_owned_and_supersession_drops_it_once() {
    let gate = ThreadSlot::default();
    let (port, rx) = start(&gate);
    let drops = Arc::new(AtomicUsize::new(0));
    port.submit(job(1, &drops)).unwrap();
    arrived(&rx, 1);
    until(|| port.counts().completed == 1);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(gate.occupied());
    assert!(gate.start(|| TestWork(mpsc::channel().0)).is_err());
    port.submit(job(2, &drops)).unwrap();
    arrived(&rx, 2);
    until(|| port.counts().completed == 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    drop(port);
    until(|| !gate.occupied());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn repeated_destroy_recreate_cannot_refund_indivisible_running_work() {
    let gate = ThreadSlot::default();
    let (port, rx) = start(&gate);
    let drops = Arc::new(AtomicUsize::new(0));
    let (release, wait) = mpsc::channel();
    let mut a = job(1, &drops);
    a.release = Some(wait);
    port.submit(a).unwrap();
    arrived(&rx, 1);
    port.submit(job(2, &drops)).unwrap();
    drop(port);
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "only pending drops on UI teardown"
    );
    for _ in 0..64 {
        assert!(gate.start(|| TestWork(mpsc::channel().0)).is_err());
        assert!(gate.occupied());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    release.send(()).unwrap();
    until(|| !gate.occupied());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    let (next, rx) = start(&gate);
    next.submit(job(3, &drops)).unwrap();
    arrived(&rx, 3);
    until(|| next.counts().completed == 1);
    drop(next);
    until(|| !gate.occupied());
    assert_eq!(drops.load(Ordering::SeqCst), 3);
}

#[test]
fn result_drop_reenters_mailbox_without_a_locked_destructor() {
    // try_lock proves payload destruction is outside the mailbox mutex.
    struct DropCheck(Box<dyn FnOnce() + Send>);
    impl Drop for DropCheck {
        fn drop(&mut self) {
            (std::mem::replace(&mut self.0, Box::new(|| {})))();
        }
    }
    struct Echo;
    impl TextWork for Echo {
        type Input = DropCheck;
        type Output = DropCheck;
        fn execute(&mut self, input: DropCheck) -> DropCheck {
            input
        }
    }
    let gate = ThreadSlot::default();
    let port = gate.start(|| Echo).unwrap();
    let shared = Arc::downgrade(&port.shared);
    let checked = Arc::new(AtomicUsize::new(0));
    let flag = checked.clone();
    port.submit(DropCheck(Box::new(move || {
        let shared = shared.upgrade().unwrap();
        assert!(shared.state.try_lock().is_ok());
        flag.fetch_add(1, Ordering::SeqCst);
    })))
    .unwrap();
    until(|| port.counts().completed == 1);
    port.cancel();
    assert_eq!(checked.load(Ordering::SeqCst), 1);
    drop(port);
    until(|| !gate.occupied());
}

#[test]
fn delivery_notifies_without_ui_poll_and_cancel_rejects_late_result() {
    use std::io::Read;
    let gate = ThreadSlot::default();
    let (port, rx) = start(&gate);
    let mut wake = port.wake.try_clone().unwrap();
    // Socket readiness is read directly; no repeated UI pump makes progress.
    wake.set_nonblocking(false).unwrap();
    wake.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    port.submit(job(1, &drops)).unwrap();
    arrived(&rx, 1);
    let mut byte = [0];
    wake.read_exact(&mut byte).unwrap();
    wake.set_nonblocking(true).unwrap();
    assert_eq!(port.take().unwrap().1.value, 1);
    let (release, wait) = mpsc::channel();
    let mut a = job(2, &drops);
    a.release = Some(wait);
    port.submit(a).unwrap();
    arrived(&rx, 2);
    port.cancel();
    release.send(()).unwrap();
    until(|| port.counts().running == 0);
    assert!(port.take().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
    drop(port);
    until(|| !gate.occupied());
}

#[test]
fn serial_exhaustion_preserves_admitted_work_and_drops_refused_input() {
    let gate = ThreadSlot::default();
    let (port, rx) = start(&gate);
    let drops = Arc::new(AtomicUsize::new(0));
    port.shared.state.lock().unwrap().serial = u64::MAX - 1;
    assert_eq!(port.submit(job(1, &drops)).unwrap(), u64::MAX);
    arrived(&rx, 1);
    until(|| port.counts().completed == 1);
    assert!(port.submit(job(2, &drops)).is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(port.take().unwrap().1.value, 1);
    drop(port);
    until(|| !gate.occupied());
}

#[test]
fn factory_preparation_does_not_run_or_wait_on_ui_admission() {
    let gate = ThreadSlot::default();
    let (entered, started) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let ui = std::thread::current().id();
    let port = gate
        .start(move || {
            assert_ne!(std::thread::current().id(), ui);
            entered.send(()).unwrap();
            wait.recv().unwrap();
            TestWork(mpsc::channel().0)
        })
        .unwrap();
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(port); // Does not join even while worker recipe construction is held.
    assert!(gate.occupied());
    assert!(gate.start(|| TestWork(mpsc::channel().0)).is_err());
    release.send(()).unwrap();
    until(|| !gate.occupied());
}

#[test]
fn repeated_sessions_reuse_one_service_thread_including_its_tls() {
    thread_local! { static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    let gate = ThreadSlot::default();
    let mut thread = None;
    for n in 1..=64 {
        let (tx, rx) = mpsc::channel();
        let port = gate
            .start(move || {
                let count = VISITS.with(|v| {
                    let n = v.get() + 1;
                    v.set(n);
                    n
                });
                tx.send((std::thread::current().id(), count)).unwrap();
                TestWork(mpsc::channel().0)
            })
            .unwrap();
        let (id, count) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if let Some(expected) = thread {
            assert_eq!(id, expected);
        } else {
            thread = Some(id);
        }
        assert_eq!(count, n);
        drop(port);
        until(|| !gate.occupied());
    }
}

#[test]
fn blocked_work_owner_destructor_keeps_process_admission_busy() {
    struct WorkDrop(mpsc::Sender<()>, mpsc::Receiver<()>);
    impl TextWork for WorkDrop {
        type Input = ();
        type Output = ();
        fn execute(&mut self, _: ()) {}
    }
    impl Drop for WorkDrop {
        fn drop(&mut self) {
            self.0.send(()).unwrap();
            let _ = self.1.recv();
        }
    }
    let gate = ThreadSlot::default();
    let (entered, started) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let port = gate.start(move || WorkDrop(entered, wait)).unwrap();
    drop(port);
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    for _ in 0..64 {
        assert!(gate
            .start(|| WorkDrop(mpsc::channel().0, mpsc::channel().1))
            .is_err());
    }
    release.send(()).unwrap();
    until(|| !gate.occupied());
}

#[test]
fn failed_factory_closes_and_wakes_session_then_service_accepts_replacement() {
    let gate = ThreadSlot::<TestWork>::default();
    let port = gate.start(|| panic!("injected preparation panic")).unwrap();
    until(|| port.is_closed());
    assert!(port.take().is_none());
    assert!(port.submit(job(1, &Arc::new(AtomicUsize::new(0)))).is_err());
    drop(port);
    until(|| !gate.occupied());
    let (next, rx) = start(&gate);
    let drops = Arc::new(AtomicUsize::new(0));
    next.submit(job(2, &drops)).unwrap();
    arrived(&rx, 2);
    until(|| next.counts().completed == 1);
    assert_eq!(next.take().unwrap().1.value, 2);
    drop(next);
    until(|| !gate.occupied());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn isolated_service_drop_closes_live_port_and_releases_owned_thread_tls() {
    thread_local! {
        static EXIT: std::cell::RefCell<Option<Counted>>=const {std::cell::RefCell::new(None)};
    }
    let exits = Arc::new(AtomicUsize::new(0));
    let on_exit = exits.clone();
    let (ready, started) = mpsc::channel();
    let gate = ThreadSlot::default();
    let port = gate
        .start(move || {
            EXIT.with(|x| *x.borrow_mut() = Some(Counted(on_exit)));
            ready.send(()).unwrap();
            TestWork(mpsc::channel().0)
        })
        .unwrap();
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(gate);
    assert!(port.is_closed());
    until(|| exits.load(Ordering::SeqCst) == 1);
    drop(port);
}

#[test]
fn only_factory_and_messages_cross_threads_worker_state_may_be_thread_local_rc() {
    struct Local(std::rc::Rc<std::cell::Cell<usize>>);
    impl TextWork for Local {
        type Input = usize;
        type Output = usize;
        fn execute(&mut self, value: usize) -> usize {
            self.0.set(self.0.get() + value);
            self.0.get()
        }
    }
    let gate = ThreadSlot::default();
    let port = gate
        .start(|| Local(std::rc::Rc::new(std::cell::Cell::new(0))))
        .unwrap();
    port.submit(3).unwrap();
    until(|| port.counts().completed == 1);
    assert_eq!(port.take().unwrap().1, 3);
    port.submit(4).unwrap();
    until(|| port.counts().completed == 1);
    assert_eq!(port.take().unwrap().1, 7);
    drop(port);
    until(|| !gate.occupied());
}
