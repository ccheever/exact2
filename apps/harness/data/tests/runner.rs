//! The harness's values are what `shapes.contract` declares: a real
//! runner boots against it, takes `session()` and `animation(...)` through
//! the shape check, and asks `session()` again when the source announces.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataSource, Runner, RunnerError};
use harness_data::{Harness, Options};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const APP: &str = r#"
component App
  resource session = session() as shape Session
  resource frame = animation("fire", 12, 20, 4) as shape Frame
  view
    text `${session.model} ${session.phase} ${frame.title}`
"#;

fn entries(v: &Value) -> usize {
    // `Session.entries` is the tenth field.
    let Value::Record(fields) = v else {
        panic!("{v:?}")
    };
    match &fields[9] {
        Value::List(items) => items.len(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_runner_takes_the_shapes_and_the_push() {
    let source = format!("{}\n{APP}", include_str!("../../shapes.contract"));
    let plan = contract::compile(&source).expect("the app compiles");
    let harness = Harness::with_options(Options::offline(None));
    let mut handle = harness.clone();
    let mut r = Runner::boot(
        plan,
        harness,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .expect("boots: the values conform");
    let woke = Arc::new(AtomicUsize::new(0));
    let count = woke.clone();
    r.listen(Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
    }));
    r.data_ready().unwrap();
    let session = r.resource("session").expect("session").clone();
    assert_eq!(entries(&session), 1, "the banner");
    let Some(Value::Record(frame)) = r.resource("frame") else {
        panic!("no frame")
    };
    assert_eq!(frame[0], Value::str("fire"));

    // A send on another handle changes the session and announces it.
    handle.query("submit", &[Value::str("/help")]).unwrap();
    assert!(woke.load(Ordering::SeqCst) >= 1);
    let (_, error) = r.apply_announced();
    assert!(error.is_none(), "{error:?}");
    let session = r.resource("session").expect("session");
    assert_eq!(entries(session), 2, "the banner and the help");
}

#[test]
fn the_shape_check_is_live() {
    // The same session read as the wrong shape is not accepted, so the
    // test above proves conformance rather than passing anything through.
    let wrong = r#"
shape Wrong
  model: number
component App
  resource session = session() as shape Wrong
  view
    text `${session.model}`
"#;
    let plan = contract::compile(wrong).expect("compiles");
    let booted = Runner::boot(
        plan,
        Harness::with_options(Options::offline(None)),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    );
    match booted {
        Err(RunnerError::Shape { resource }) => assert_eq!(resource, "session"),
        Err(other) => panic!("refused for another reason: {other:?}"),
        Ok(_) => panic!("a session read as the wrong shape was accepted"),
    }
}
