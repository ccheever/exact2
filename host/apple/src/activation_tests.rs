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
                platform.snapshot(),
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
