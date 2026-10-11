//! The prompt is usable before Git answers; even a reply before listen is retained.
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, Native, Outcome, Response, Store};
use exact_terminal::{
    host::{Host, Mode},
    vt::Vt,
};
use harness_data::{
    shapes::{ContractValue, Session},
    Harness, Options,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{channel, Receiver},
    Arc,
};
use std::time::{Duration, Instant};

struct Delayed {
    data: Harness,
    release: Option<Receiver<()>>,
    done: Arc<AtomicBool>,
}
impl DataSource for Delayed {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        self.data.query(source, args)
    }
    fn answer(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        self.data.answer(store, source, args)
    }
    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        out: Outcome,
    ) -> Result<Answer, DataError> {
        self.data.parse(store, source, args, out)
    }
    fn native(&self) -> Option<Native> {
        self.data.native()
    }
    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        drop(self.data.continuation(token)?);
        let release = self.release.take().unwrap();
        let done = self.done.clone();
        Some(Box::new(move || {
            release.recv().unwrap();
            done.store(true, Ordering::SeqCst);
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: b"feature/startup-ready".to_vec(),
            })
        }))
    }
}

#[test]
fn prompt_works_before_metadata_and_an_early_reply_is_not_lost() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../terminal.contract");
    let plan = contract::compile_path_terminal(&path).unwrap();
    let mut harness = Harness::with_options(Options::offline(None));
    harness.query("setModel", &[Value::str("mock")]).unwrap();
    let (release, receive) = channel();
    let done = Arc::new(AtomicBool::new(false));
    let data = Delayed {
        data: harness.clone(),
        release: Some(receive),
        done: done.clone(),
    };
    let mut host = Host::boot(plan, data, Mode::Inline, 200, 30).unwrap();
    host.arm = false;
    let mut vt = Vt::new(&mut host);
    vt.render(&mut host);
    assert!(host.frame().grid.text().contains("Ask anything"));
    assert!(Session::from_value(&harness.session()).unwrap().entries[0].busy);
    host.focus(host.by_test_id("prompt"));
    host.paste("STARTUP_INPUT");
    vt.render(&mut host);
    assert!(vt.text(true).contains("STARTUP_INPUT"));
    assert!(!done.load(Ordering::SeqCst));
    release.send(()).unwrap();
    let start = Instant::now();
    while !done.load(Ordering::SeqCst) {
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    // The worker has answered before the event-loop wake is installed.
    host.listen(Arc::new(|| {}));
    while Session::from_value(&harness.session()).unwrap().entries[0].busy {
        assert!(start.elapsed() < Duration::from_secs(5));
        host.announced();
        std::thread::sleep(Duration::from_millis(1));
    }
    vt.render(&mut host);
    assert!(vt.text(true).contains("feature/startup-ready"));
    assert!(vt.text(true).contains("STARTUP_INPUT"));
    let due = host.timer_due_ms().expect("completed banner can retire");
    host.tick(due);
    host.announced();
    vt.render(&mut host);
    assert!(Session::from_value(&harness.session())
        .unwrap()
        .entries
        .is_empty());
    assert_eq!(
        vt.text(true).matches("feature/startup-ready").count(),
        2,
        "one banner in scrollback and one live status line; screen:\n{}",
        vt.text(true)
    );
}
