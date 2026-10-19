//! A data source that isn't ready at activation wakes the session when it is.

use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::{builder::PlanBuilder, Value};
use exact_runner::DataError;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

type Wake = Box<dyn FnOnce() + Send>;

/// Prepares in the background until `ready`; keeps the wakes it is given.
struct Loading {
    ready: Arc<AtomicBool>,
    waiting: Arc<Mutex<Vec<Wake>>>,
}
impl DataSource for Loading {
    fn app_id(&self) -> &str {
        "test.exact.activation"
    }
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn preload(&self) -> Result<bool, DataError> {
        Ok(self.ready.load(Ordering::SeqCst))
    }
    fn when_preloaded(&self, wake: Wake) {
        self.waiting.lock().unwrap().push(wake);
    }
}

#[test]
fn a_pending_activation_wakes_the_session_once_the_source_is_ready() {
    let mut builder = PlanBuilder::new(exact_kernel::SCHEMA_DIGEST, 1);
    builder.node(NodeType::View as u8, None, None, 0, &[], &[], None);
    let plan = builder.finish().unwrap().encode();
    let (ready, waiting) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(Mutex::new(Vec::new())),
    );
    let source = Loading {
        ready: ready.clone(),
        waiting: waiting.clone(),
    };
    let (mut host, _) = Host::boot(
        &plan,
        source,
        Box::new(MonospaceMeasurer::default()),
        10.0,
        10.0,
    )
    .unwrap();
    let wakes = Arc::new(AtomicUsize::new(0));
    let counted = wakes.clone();
    host.listen(Arc::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    }));
    assert!(host.activate_data().contains("\"pending\":true"));
    assert!(host.activate_data().contains("\"pending\":true"));
    let held = || std::mem::take(&mut *waiting.lock().unwrap());
    let mut first = held();
    assert_eq!(first.len(), 1, "a retry while loading adds no waiter");
    assert_eq!(wakes.load(Ordering::SeqCst), 0);
    first.pop().unwrap()();
    assert_eq!(wakes.load(Ordering::SeqCst), 1, "the session is woken");
    assert!(host.activate_data().contains("\"pending\":true"));
    let mut second = held();
    assert_eq!(
        second.len(),
        1,
        "a wake that finds it still loading asks again"
    );
    ready.store(true, Ordering::SeqCst);
    second.pop().unwrap()();
    assert_eq!(wakes.load(Ordering::SeqCst), 2);
    assert!(
        !host.activate_data().contains("\"pending\":true"),
        "and activates"
    );
}

struct SharedStoreSource {
    asks: Arc<Mutex<Vec<String>>>,
    activated: Arc<AtomicUsize>,
}
impl DataSource for SharedStoreSource {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn grants(&self) -> &str {
        "secret.keep favourite"
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        name: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        self.asks.lock().unwrap().push(name.into());
        Ok(exact_runner::Answer::Now(Value::record(vec![Value::str(
            if name == "favourite" {
                store.get("favourite").unwrap_or("none")
            } else {
                "pure"
            },
        )])))
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.activated.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn peer_store_refresh_reasks_readers_and_restores_persisted_slots_in_every_session() {
    use crate::store::{KeptStore, Platform};
    use exact_runner::StoreWrite;
    let plan = contract::compile("shape Text\n  value: string\ncomponent App\n  state local = 7\n  state starred = false persist\n  resource favourite = favourite() as shape Text\n  resource pure = pure() as shape Text\n  view\n    text `${favourite.value} ${pure.value} ${starred} ${local}`\n").unwrap().encode();
    crate::store::PLATFORM
        .with(|p| *p.borrow_mut() = Some(Arc::new(ibex2::secrets::MemoryStore::new())));
    let bindings = crate::store::endow("secret.keep favourite").unwrap();
    crate::store::PLATFORM.with(|p| *p.borrow_mut() = None);
    let platform = Platform::of(&bindings);
    let asks = Arc::new(Mutex::new(Vec::new()));
    let activated = Arc::new(AtomicUsize::new(0));
    let mut hosts: Vec<_> = (0..2)
        .map(|_| {
            Host::boot_stored(
                &plan,
                SharedStoreSource {
                    asks: asks.clone(),
                    activated: activated.clone(),
                },
                Box::new(MonospaceMeasurer::default()),
                300.0,
                200.0,
                None,
                platform.snapshot(Vec::new()).values,
                Some(platform.clone()),
            )
            .unwrap()
            .0
        })
        .collect();
    platform
        .write(&StoreWrite {
            name: "favourite".into(),
            value: Some(b"Before first pixel".to_vec()),
        })
        .unwrap();
    for host in &mut hosts {
        host.refresh_store();
        assert_eq!(activated.load(Ordering::SeqCst), 0);
    }
    for host in &mut hosts {
        host.activate_data();
        assert_eq!(
            host.runner.resource("favourite"),
            Some(&Value::record(vec![Value::str("Before first pixel")]))
        );
    }
    for host in &mut hosts {
        host.runner.measure(false, false);
    }
    let pure_asks = asks.lock().unwrap().iter().filter(|s| *s == "pure").count();
    platform
        .write(&StoreWrite {
            name: "favourite".into(),
            value: Some(b"Palo Alto".to_vec()),
        })
        .unwrap();
    platform
        .write(&StoreWrite {
            name: "exact.state.starred".into(),
            value: Some(b"true".to_vec()),
        })
        .unwrap();
    for host in &mut hosts {
        let batch = host.refresh_store();
        assert!(batch.contains("\"error\":null"), "{batch}");
        assert!(
            !batch.contains("\"seq\":"),
            "production carries no perf trailer"
        );
        assert!(!batch.contains("\"unchanged\":true"));
        assert!(
            batch.contains("Palo Alto"),
            "the unmeasured batch carries the changed reader: {batch}"
        );
        assert_eq!(
            host.runner.resource("favourite"),
            Some(&Value::record(vec![Value::str("Palo Alto")]))
        );
        assert_eq!(host.runner.persisted("starred"), Some(Value::Bool(true)));
        assert_eq!(
            host.runner
                .carry()
                .slots
                .iter()
                .find(|(name, _)| name == "local")
                .unwrap()
                .1,
            Value::Number(7.)
        );
    }
    platform
        .write(&StoreWrite {
            name: "favourite".into(),
            value: None,
        })
        .unwrap();
    for host in &mut hosts {
        host.refresh_store();
        assert_eq!(
            host.runner.resource("favourite"),
            Some(&Value::record(vec![Value::str("none")]))
        );
    }
    platform
        .write(&StoreWrite {
            name: "exact.state.starred".into(),
            value: None,
        })
        .unwrap();
    for host in &mut hosts {
        host.refresh_store();
        assert_eq!(
            host.runner.persisted("starred"),
            Some(Value::Bool(false)),
            "a peer deletion restores the initializer"
        );
        let asks_before = asks.lock().unwrap().len();
        assert!(host.refresh_store().contains("\"unchanged\":true"));
        assert_eq!(asks.lock().unwrap().len(), asks_before);
    }
    assert_eq!(
        asks.lock().unwrap().iter().filter(|s| *s == "pure").count(),
        pure_asks
    );
    assert_eq!(
        activated.load(Ordering::SeqCst),
        2,
        "refresh never activates the data module again"
    );
}

struct ChangingReader {
    asks: Arc<AtomicUsize>,
}
impl DataSource for ChangingReader {
    fn app_id(&self) -> &str {
        "test.exact.wp12-self"
    }
    fn grants(&self) -> &str {
        "secret.keep favourite"
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        store.get("favourite");
        let n = self.asks.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(exact_runner::Answer::Now(Value::record(vec![Value::str(
            &n.to_string(),
        )])))
    }
}

#[test]
fn app_side_writes_publish_without_reasking_a_time_varying_reader() {
    let app = "test.exact.wp12-self";
    if std::env::var("EXACT_WP12_WRITER_TEST").as_deref() != Ok("1") {
        // Environment is fixed before the child starts; parallel tests in
        // this process never observe a changed app group or writer identity.
        let root = std::env::temp_dir().join(format!("exact-wp12-self-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "host::activation_tests::app_side_writes_publish_without_reasking_a_time_varying_reader", "--test-threads=1"])
            .env("EXACT_WP12_WRITER_TEST", "1")
            .env("EXACT_APP_GROUP_ROOT", &root)
            .env("EXACT_APP_GROUP_APP", app)
            .env("EXACT_WIDGET_WRITER", app)
            .output().unwrap();
        let _ = std::fs::remove_dir_all(root);
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("1 passed"),
            "child ran no test: {:?}",
            result.stdout
        );
        return;
    }
    let root = std::path::PathBuf::from(std::env::var_os("EXACT_APP_GROUP_ROOT").unwrap());
    let plan = contract::compile("shape Text\n  value: string\ncomponent App\n  state starred = false persist\n  resource favourite = favourite() as shape Text\n  action star\n    starred = !starred\n  view\n    text favourite.value\n").unwrap().encode();
    crate::store::PLATFORM
        .with(|p| *p.borrow_mut() = Some(Arc::new(ibex2::secrets::MemoryStore::new())));
    let asks = Arc::new(AtomicUsize::new(0));
    let bindings = crate::store::endow_for("secret.keep favourite", app).unwrap();
    let platform = crate::store::Platform::of(&bindings);
    let (mut host, _) = Host::boot_stored(
        &plan,
        ChangingReader { asks: asks.clone() },
        Box::new(MonospaceMeasurer::default()),
        300.,
        200.,
        None,
        Vec::new(),
        Some(platform),
    )
    .unwrap();
    crate::store::PLATFORM.with(|p| *p.borrow_mut() = None);
    host.activate_data();
    let before = asks.load(Ordering::SeqCst);
    let receipt = host.runner.act("star", vec![]).unwrap();
    host.commit(&[Timed { at_ms: 0., receipt }], None);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let counter = || {
        std::fs::read_to_string(root.join("widget-store-generation"))
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };
    while counter() < 2 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(counter() >= 2, "boot and action publications reached disk");
    for _ in 0..3 {
        host.refresh_store();
    }
    assert_eq!(
        asks.load(Ordering::SeqCst),
        before,
        "an own ping cannot start a changing-answer loop"
    );
    host.secrets = None;
    assert_eq!(
        host.refresh_store(),
        "{\"ops\":[],\"unchanged\":true}",
        "an archive without I/O performs no commit"
    );
    // The publisher owns only queued paths; wait for both the boot and action counters before removal.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::fs::read_to_string(root.join("widget-store-generation"))
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
        < 2
        && std::time::Instant::now() < until
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// A source that is not ready at boot, so the runner keeps its answers, and
/// whose answer differs on every ask (a server time, a nonce).
struct DeferredChangingReader {
    asks: Arc<AtomicUsize>,
    ready: bool,
}
impl DataSource for DeferredChangingReader {
    fn grants(&self) -> &str {
        "secret.keep favourite"
    }
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.ready = true;
        Ok(())
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        _: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        store.get("favourite");
        let n = self.asks.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(exact_runner::Answer::Now(Value::record(vec![Value::str(
            &n.to_string(),
        )])))
    }
}

#[test]
fn sibling_sessions_keeping_different_answers_do_not_reask_each_other() {
    use crate::store::{KeptStore, Platform};
    let plan = contract::compile("shape Text\n  value: string\ncomponent App\n  resource favourite = favourite() as shape Text\n  view\n    text favourite.value\n").unwrap().encode();
    crate::store::PLATFORM
        .with(|p| *p.borrow_mut() = Some(Arc::new(ibex2::secrets::MemoryStore::new())));
    let bindings = crate::store::endow("secret.keep favourite").unwrap();
    crate::store::PLATFORM.with(|p| *p.borrow_mut() = None);
    let platform = Platform::of(&bindings);
    let asks = Arc::new(AtomicUsize::new(0));
    let mut hosts: Vec<_> = (0..2)
        .map(|_| {
            Host::boot_stored(
                &plan,
                DeferredChangingReader {
                    asks: asks.clone(),
                    ready: false,
                },
                Box::new(MonospaceMeasurer::default()),
                300.0,
                200.0,
                None,
                platform.snapshot(Vec::new()).values,
                Some(platform.clone()),
            )
            .unwrap()
            .0
        })
        .collect();
    for host in &mut hosts {
        host.activate_data();
    }
    crate::store::flush_kept();
    let kept = platform.snapshot(Vec::new()).values;
    assert!(
        kept.iter().any(|(name, _)| name.starts_with("exact.kept.")),
        "the sessions keep their answers: {kept:?}"
    );
    // Each session keeps its own answer to the one reader; a refresh that
    // finds the sibling's kept answer must not count it as a reader's input.
    let mut rounds = Vec::new();
    for _ in 0..3 {
        for host in &mut hosts {
            host.refresh_store();
            crate::store::flush_kept();
        }
        rounds.push(asks.load(Ordering::SeqCst));
    }
    assert!(
        rounds.windows(2).all(|w| w[0] == w[1]),
        "asks grew round after round: {rounds:?}"
    );
}

struct DeferredReader {
    ready: bool,
    later: bool,
    asks: Arc<AtomicUsize>,
    sends: Arc<AtomicUsize>,
    refused: bool,
}
impl DataSource for DeferredReader {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        unreachable!()
    }
    fn grants(&self) -> &str {
        "secret.keep favourite\nnet.fetch https://example.test"
    }
    fn ready(&self) -> bool {
        self.ready
    }
    fn activate(&mut self) -> Result<(), DataError> {
        self.ready = true;
        Ok(())
    }
    fn answer(
        &mut self,
        store: &mut exact_runner::Store,
        name: &str,
        _: &[Value],
    ) -> Result<exact_runner::Answer, DataError> {
        if name == "save" {
            self.sends.fetch_add(1, Ordering::SeqCst);
            return Ok(exact_runner::Answer::Now(Value::record(vec![Value::str(
                "sent",
            )])));
        }
        store.get("favourite");
        self.asks.fetch_add(1, Ordering::SeqCst);
        if self.later {
            return Ok(exact_runner::Answer::Later(exact_runner::Request::get(
                "https://example.test/value",
            )));
        }
        let value = if self.refused {
            "ok"
        } else {
            self.refused = true;
            "refuse"
        };
        Ok(exact_runner::Answer::Now(Value::record(vec![Value::str(
            value,
        )])))
    }
}

#[test]
fn deferred_peer_refresh_asks_once_and_a_refusal_can_retry_after_activation() {
    use crate::store::Platform;
    use exact_runner::StoreWrite;
    for later in [true, false] {
        let mut plan = contract::compile("shape Text\n  value: string\ncomponent App\n  state local = 0\n  mutation saved as shape Text\n  action save\n    send saved = save()\n  resource favourite = favourite() as shape Text\n  action tick\n    local = local + 1\n  task watch key=favourite.value == \"refuse\" ? 0 / 0 : 1\n    after(10000, tick)\n  view\n    text favourite.value\n").unwrap();
        plan.resources[0].reader = true;
        crate::store::PLATFORM
            .with(|p| *p.borrow_mut() = Some(Arc::new(ibex2::secrets::MemoryStore::new())));
        let bindings = crate::store::endow("secret.keep favourite").unwrap();
        crate::store::PLATFORM.with(|p| *p.borrow_mut() = None);
        let platform = Platform::of(&bindings);
        let asks = Arc::new(AtomicUsize::new(0));
        let sends = Arc::new(AtomicUsize::new(0));
        let (mut host, _) = Host::boot_stored(
            &plan.encode(),
            DeferredReader {
                ready: false,
                later,
                asks: asks.clone(),
                sends: sends.clone(),
                refused: false,
            },
            Box::new(MonospaceMeasurer::default()),
            300.,
            200.,
            None,
            Vec::new(),
            Some(platform.clone()),
        )
        .unwrap();
        if later {
            let receipt = host.runner.act("save", vec![]).unwrap();
            host.commit(&[Timed { at_ms: 0., receipt }], None);
            assert_eq!(sends.load(Ordering::SeqCst), 0);
        }
        platform
            .write(&StoreWrite {
                name: "favourite".into(),
                value: Some(b"peer".to_vec()),
            })
            .unwrap();
        host.secrets = Some(Box::new(RefusedRead {
            platform,
            first: std::cell::Cell::new(true),
        }));
        host.refresh_store();
        assert_eq!(asks.load(Ordering::SeqCst), 0);
        let batch = host.activate_data();
        assert!(host.data_activated);
        assert!(
            batch.contains("\"error\":null"),
            "an incomplete read does not fail activation: {batch}"
        );
        assert_eq!(
            sends.load(Ordering::SeqCst),
            usize::from(later),
            "a refused store read must not swallow an unsent mutation"
        );
        assert_eq!(
            asks.load(Ordering::SeqCst),
            1,
            "data_ready is the only activation ask: {batch}"
        );
        if later {
            assert_eq!(
                host.runner.take_requests().len(),
                1,
                "no discarded duplicate request is dispatched"
            );
        }
        assert!(
            host.store_refresh_pending,
            "the incomplete read remains pending after activation"
        );
        let retried = host.refresh_store();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&retried).unwrap()["unchanged"],
            true,
            "the successful retry with identical values asks nothing"
        );
        assert!(!host.store_refresh_pending);
        assert_eq!(asks.load(Ordering::SeqCst), 1);
    }
}

struct RefusedRead {
    platform: crate::store::Platform,
    first: std::cell::Cell<bool>,
}
impl crate::store::KeptStore for RefusedRead {
    fn snapshot(&self, previous: Vec<(String, Vec<u8>)>) -> crate::store::StoreRead {
        let mut read = self.platform.snapshot(previous);
        if self.first.replace(false) {
            read.complete = false;
        }
        read
    }
    fn write(&self, write: &exact_runner::StoreWrite) -> Result<(), String> {
        self.platform.write(write).map_err(|e| e.to_string())
    }
    fn kept_failures(&self) -> Vec<String> {
        self.platform.kept_failures()
    }
}

#[test]
fn refused_data_ready_survives_an_unrelated_change_and_store_refusals_are_journalled() {
    use crate::store::Platform;
    use exact_runner::StoreWrite;
    let plan = contract::compile("shape Text\n  value: string\ncomponent App\n  state divisor = 1 persist\n  resource favourite = favourite() as shape Text\n  task watch key=1 / divisor\n    after(10000, tick)\n  action tick\n    divisor = divisor\n  view\n    text favourite.value\n").unwrap().encode();
    crate::store::PLATFORM
        .with(|p| *p.borrow_mut() = Some(Arc::new(ibex2::secrets::MemoryStore::new())));
    let bindings = crate::store::endow("secret.keep favourite").unwrap();
    crate::store::PLATFORM.with(|p| *p.borrow_mut() = None);
    let platform = Platform::of(&bindings);
    let (mut host, _) = Host::boot_stored(
        &plan,
        DeferredReader {
            ready: false,
            later: false,
            asks: Arc::new(AtomicUsize::new(0)),
            sends: Arc::new(AtomicUsize::new(0)),
            refused: true,
        },
        Box::new(MonospaceMeasurer::default()),
        300.,
        200.,
        None,
        Vec::new(),
        Some(platform.clone()),
    )
    .unwrap();
    let put = |name: &str, value: &[u8]| {
        platform
            .write(&StoreWrite {
                name: name.into(),
                value: Some(value.to_vec()),
            })
            .unwrap()
    };
    put("exact.state.divisor", b"0");
    let ack: serde_json::Value = serde_json::from_str(&host.refresh_store()).unwrap();
    assert_eq!(
        ack["storeActivationPending"], true,
        "pre-pixel ack cannot consume the counter"
    );
    let batch: serde_json::Value = serde_json::from_str(&host.activate_data()).unwrap();
    assert!(
        batch["error"].is_string(),
        "the independent gate really refused data_ready: {batch}"
    );
    assert!(host.store_activation_pending);
    put("favourite", b"unrelated");
    host.refresh_store();
    assert!(
        host.store_activation_pending,
        "an unrelated change cannot discharge data_ready"
    );
    put("exact.state.divisor", b"1");
    let retry: serde_json::Value = serde_json::from_str(&host.refresh_store()).unwrap();
    assert!(retry["error"].is_null(), "{retry}");
    assert!(!host.store_activation_pending && !host.store_refresh_pending);
    put("exact.state.divisor", b"0");
    host.refresh_store();
    assert!(host
        .runner
        .journal()
        .any(|line| line.contains("store changed refused")));
}
