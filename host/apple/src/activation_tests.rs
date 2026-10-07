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
