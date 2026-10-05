//! Real TypeScript answers through the data seam and the native storage provider.
#![cfg(exact_js_engine)]
use exact_js::Module;
use exact_kernel::{Kernel, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{Answer, DataSource, Event, Outcome, Response, Runner, Store};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
const HBC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/storage.hbc"));
const APP: &str = "dev.exact.storage-test";
const GRANTS: &str = "fs.read app:/data\nfs.write app:/data\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n";
fn plan() -> Plan {
    contract::compile(
        r#"
shape Result
  text: string
component App
  mutation output as shape Result
  action invoke(op: string, value: string)
    send output = work(op, value)
  view
    column
      button press=invoke("file", "from filesystem") testId="file"
        text "Save file"
      button press=invoke("add", "from SQLite") testId="sqlite"
        text "Save row"
      when pending(output)
        text "Working" testId="pending"
      match output
        case some(result)
          text result.text testId="result"
        case none
          text "storage" testId="empty"
"#,
    )
    .unwrap()
}
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "exact-storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn module(&self) -> Module {
        let mut m = Module::new(HBC.to_vec(), APP, GRANTS);
        // Functional fixtures carry no wall-clock budget; it is not a stable
        // gate on a shared test machine.
        m.set_budget_ms(f64::INFINITY);
        m.configure_storage(
            self.0.join("data"),
            self.0.join("cache"),
            self.0.join("tmp"),
        )
        .unwrap();
        m.bind(&plan());
        m
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn args(op: &str, value: &str) -> Vec<Value> {
    vec![Value::str(op), Value::str(value)]
}
fn text(v: Value) -> String {
    match v {
        Value::Record(fields) => fields[0].as_str().unwrap().into(),
        other => panic!("{other:?}"),
    }
}
fn response(body: &str) -> Outcome {
    Outcome::Response(Response {
        status: 200,
        headers: vec![],
        body: body.as_bytes().to_vec(),
    })
}
fn finish(m: &mut Module, s: &mut Store, a: &[Value], mut answer: Answer) -> String {
    for _ in 0..100 {
        match answer {
            Answer::Now(v) => return text(v),
            Answer::Later(request) => {
                let token = request
                    .continuation
                    .expect("storage never invents an HTTP request");
                let work = m.continuation(token).expect("live continuation");
                let outcome = std::thread::spawn(work).join().unwrap();
                answer = m.parse(s, "work", a, outcome).unwrap();
            }
        }
    }
    panic!("storage did not settle")
}
fn call(m: &mut Module, s: &mut Store, op: &str, value: &str) -> String {
    let a = args(op, value);
    let answer = m.answer(s, "work", &a).unwrap();
    finish(m, s, &a, answer)
}

#[test]
fn native_modules_are_deferred_retired_and_unavailable_to_validation() {
    struct Native {
        data: Option<PathBuf>,
    }
    impl exact_js::NativeModule for Native {
        fn configure_storage(
            &mut self,
            data: PathBuf,
            _: PathBuf,
            _: PathBuf,
        ) -> Result<(), String> {
            assert!(
                !data.exists(),
                "native configuration must precede storage activation"
            );
            self.data = Some(data);
            Ok(())
        }
        fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
            let path = self.data.as_ref().unwrap().join("native.txt");
            std::fs::write(path, request["value"].as_str().unwrap()).unwrap();
            Ok(serde_json::json!({"text":request["value"]}))
        }
    }
    let root = Root::new();
    let mut m = root
        .module()
        .with_native(|_| Box::new(Native { data: None }));
    assert!(!root.0.exists());
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    m.activate().unwrap();
    assert_eq!(call(&mut m, &mut s, "native", "persisted"), "persisted");
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/native.txt")).unwrap(),
        "persisted"
    );
    let a = args("native-fetch", "after fetch");
    assert!(matches!(
        m.answer(&mut s, "work", &a).unwrap(),
        Answer::Later(_)
    ));
    let answer = m.parse(&mut s, "work", &a, response("")).unwrap();
    assert_eq!(finish(&mut m, &mut s, &a, answer), "after fetch");
    assert!(
        text(m.query("work", &args("native", "bake")).unwrap()).contains("unavailable during bake")
    );
    m.activate_for_validation().unwrap();
    // Validation links no module, so the app is told so plainly: `native` is null.
    assert_eq!(
        call(&mut m, &mut s, "native", "validation"),
        "no native module"
    );
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/native.txt")).unwrap(),
        "after fetch"
    );
}

/// `native.later`: the answer leaves as a native request, the host hands its
/// body to the module's handler, the module's own thread replies, and the
/// answer resumes with the value — or with the module's refusal.
#[test]
fn native_later_answers_from_the_modules_own_thread() {
    struct Native;
    impl exact_js::NativeModule for Native {
        fn configure_storage(&mut self, _: PathBuf, _: PathBuf, _: PathBuf) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"text": format!("now:{}", request["value"].as_str().unwrap())}))
        }
        fn later(&mut self) -> Option<exact_js::LaterHandler> {
            Some(std::sync::Arc::new(|request, reply| {
                let value = request["value"].as_str().unwrap_or("").to_string();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    reply.send(if value == "bad" {
                        Err("the model is unavailable".into())
                    } else {
                        Ok(serde_json::json!({"text": value.to_uppercase()}))
                    });
                });
            }))
        }
    }
    let root = Root::new();
    let mut m = root.module().with_native(|_| Box::new(Native));
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    m.activate().unwrap();
    let native = m.native().expect("a module offers its native handle");
    for (value, expected) in [
        ("slow", "SLOW"),
        ("bad", "refused: the model is unavailable"),
    ] {
        let a = args("later", value);
        let Answer::Later(request) = m.answer(&mut s, "work", &a).unwrap() else {
            panic!("a long native call answers later")
        };
        assert!(request.is_native(), "{request:?}");
        let handler = native.handler().expect("filled at activation");
        let (tx, rx) = std::sync::mpsc::channel();
        // Returns at once: the work is on the module's thread.
        let started = std::time::Instant::now();
        handler(
            request.body.clone(),
            exact_runner::Reply::new(move |outcome| tx.send(outcome).unwrap()),
        );
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
        let outcome = rx.recv().unwrap();
        let answer = m.parse(&mut s, "work", &a, outcome).unwrap();
        assert_eq!(finish(&mut m, &mut s, &a, answer), expected);
    }
    // Unloaded, the handle is empty: a late request fails, it does not run.
    m.unload();
    assert!(native.handler().is_none());
}

fn native_reread_fixture() -> (Root, Runner<Module>, u64) {
    struct Native;
    impl exact_js::NativeModule for Native {
        fn configure_storage(&mut self, _: PathBuf, _: PathBuf, _: PathBuf) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, _: &serde_json::Value) -> Result<serde_json::Value, String> {
            unreachable!("this fixture only makes deferred calls")
        }
        fn later(&mut self) -> Option<exact_js::LaterHandler> {
            Some(std::sync::Arc::new(|_, _| {
                unreachable!("the test delivers replies")
            }))
        }
    }
    let root = Root::new();
    let plan = contract::compile(
        r#"
shape Result
  text: string
component App
  resource data = work("later-steps", "") as shape Result else work("placeholder", "")
  mutation saved as shape Result refreshes data
  mutation stored as shape Result
  action write
    send saved = work("later", "write")
  action writeFile
    send saved = work("file", "saved")
  action storageOnly
    send stored = work("file", "saved")
  action reload
    refresh data
  view
    text data.text testId="data"
"#,
    )
    .unwrap();
    let mut module = root.module().with_native(|_| Box::new(Native));
    module.bind(&plan);
    module.activate().unwrap();
    let mut runner = Runner::boot(
        plan,
        module,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let first = runner.take_requests();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].request.body, br#"{"value":"status"}"#);
    runner
        .fulfill(first[0].ticket, response(r#"{"text":"status"}"#))
        .unwrap();
    let second = runner.take_requests();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].request.body, br#"{"value":"events"}"#);
    (root, runner, second[0].ticket)
}

#[test]
fn a_discarded_native_reread_preserves_the_request_already_in_flight() {
    let (_root, mut runner, events) = native_reread_fixture();
    runner.act("write", vec![]).unwrap();
    let writes = runner.take_requests();
    assert_eq!(writes.len(), 1, "the speculative read is never dispatched");
    assert_eq!(writes[0].request.body, br#"{"value":"write"}"#);
    runner
        .fulfill(events, response(r#"{"text":"events"}"#))
        .unwrap();
    assert!(
        runner.take_requests().is_empty(),
        "the events reply must settle its own read"
    );
    let key = runner.kernel().find_by_test_id("data")[0];
    assert_eq!(
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("status:events")
    );
}

#[test]
fn a_storage_deferred_reread_preserves_the_native_request_already_in_flight() {
    let (_root, mut runner, events) = native_reread_fixture();
    runner.act("writeFile", vec![]).unwrap();
    let writes = runner.take_requests();
    assert_eq!(writes.len(), 1, "the storage-deferred read is discarded");
    assert!(writes[0].request.continuation.is_some());
    assert_eq!(
        runner.data().in_flight(),
        2,
        "only the write and original read remain"
    );
    runner
        .fulfill(events, response(r#"{"text":"events"}"#))
        .unwrap();
    assert!(runner.take_requests().is_empty());
    let key = runner.kernel().find_by_test_id("data")[0];
    assert_eq!(
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("status:events")
    );
}

#[test]
fn an_accepted_native_refresh_replaces_only_its_previous_call() {
    let (_root, mut runner, events) = native_reread_fixture();
    runner.act("reload", vec![]).unwrap();
    let admission = runner.take_requests();
    assert_eq!(admission.len(), 1);
    let token = admission[0]
        .request
        .continuation
        .expect("overlapping call waits for acceptance");
    assert_eq!(
        runner.data().in_flight(),
        1,
        "the commit retired the old call"
    );
    assert!(runner
        .fulfill(events, response(r#"{"text":"old-events"}"#))
        .unwrap()
        .is_none());
    let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) = runner.dispatch_work(token)
    else {
        panic!("the accepted call can be dispatched")
    };
    runner.fulfill(admission[0].ticket, work()).unwrap();
    let status = runner.take_requests();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].request.body, br#"{"value":"status"}"#);
    runner
        .fulfill(status[0].ticket, response(r#"{"text":"new-status"}"#))
        .unwrap();
    let events = runner.take_requests();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].request.body, br#"{"value":"events"}"#);
    runner
        .fulfill(events[0].ticket, response(r#"{"text":"new-events"}"#))
        .unwrap();
    let key = runner.kernel().find_by_test_id("data")[0];
    assert_eq!(
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("new-status:new-events")
    );
    assert_eq!(runner.data().in_flight(), 0);
}

#[test]
fn an_accepted_storage_deferred_refresh_never_answers_the_old_native_ticket() {
    let (_root, mut runner, old_events) = native_reread_fixture();
    runner.act("storageOnly", vec![]).unwrap();
    let mut writes = runner.take_requests();
    assert_eq!(writes.len(), 1);
    runner.act("reload", vec![]).unwrap();
    let admission = runner.take_requests();
    assert_eq!(admission.len(), 1);
    let token = admission[0]
        .request
        .continuation
        .expect("storage defers the refresh");
    assert!(matches!(
        runner.dispatch_work(token),
        exact_runner::Dispatch::Held
    ));
    assert_eq!(runner.data().in_flight(), 2, "the native call was replaced");
    assert!(runner
        .fulfill(old_events, response(r#"{"text":"old-events"}"#))
        .unwrap()
        .is_none());
    let mut released = None;
    for _ in 0..10 {
        let Some(write) = writes.pop() else { break };
        let work_token = write.request.continuation.expect("file work stays local");
        let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) =
            runner.dispatch_work(work_token)
        else {
            panic!("the open storage turn runs")
        };
        runner.fulfill(write.ticket, work()).unwrap();
        writes.extend(runner.take_requests());
        for (ready, dispatch) in runner.release_work() {
            assert_eq!(ready, token);
            let exact_runner::Dispatch::Run(exact_runner::Work::Now(work)) = dispatch else {
                panic!("the deferred answer is released")
            };
            released = Some(work);
        }
    }
    runner
        .fulfill(admission[0].ticket, released.expect("storage completed")())
        .unwrap();
    let status = runner.take_requests();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].request.body, br#"{"value":"status"}"#);
    runner
        .fulfill(status[0].ticket, response(r#"{"text":"fresh-status"}"#))
        .unwrap();
    let events = runner.take_requests();
    assert_eq!(events.len(), 1);
    runner
        .fulfill(events[0].ticket, response(r#"{"text":"fresh-events"}"#))
        .unwrap();
    let key = runner.kernel().find_by_test_id("data")[0];
    assert_eq!(
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text),
        Some("fresh-status:fresh-events")
    );
    assert_eq!(runner.data().in_flight(), 0);
}

/// A module that takes no long calls answers `native.later` through `call`.
#[test]
fn native_later_without_a_handler_answers_now() {
    struct Native;
    impl exact_js::NativeModule for Native {
        fn configure_storage(&mut self, _: PathBuf, _: PathBuf, _: PathBuf) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"text": format!("now:{}", request["value"].as_str().unwrap())}))
        }
    }
    let root = Root::new();
    let mut m = root.module().with_native(|_| Box::new(Native));
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    m.activate().unwrap();
    assert_eq!(call(&mut m, &mut s, "later", "quick"), "now:quick");
}

#[test]
fn storage_is_lazy_persistent_isolated_and_grant_checked() {
    let root = Root::new();
    let other = Root::new();
    let mut m = root.module();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert!(!m.is_loaded());
    assert!(!root.0.exists());
    m.activate().unwrap();
    assert_eq!(call(&mut m, &mut s, "file", "hello"), "hello");
    assert_eq!(call(&mut m, &mut s, "add", "remember"), "remember");
    assert_eq!(call(&mut m, &mut s, "rollback", "discard"), "remember");
    assert_eq!(call(&mut m, &mut s, "refused", ""), "denied");
    assert_eq!(
        call(&mut m, &mut s, "types", ""),
        "9223372036854775807/-9223372036854775808/1.25/0,255"
    );
    assert_eq!(
        call(&mut m, &mut s, "sql-refusals", ""),
        "Unavailable/Unavailable/Unavailable/Unavailable"
    );
    assert!(!root.0.join("cache/no").exists());
    m.unload();
    m.activate().unwrap();
    assert_eq!(call(&mut m, &mut s, "read", ""), "hello");
    assert_eq!(call(&mut m, &mut s, "list", ""), "remember");
    let mut isolated = other.module();
    isolated.activate().unwrap();
    assert_eq!(call(&mut isolated, &mut s, "list", ""), "");
}

#[test]
fn storage_refuses_during_bake_even_if_a_host_configured_it() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let value = m.query("work", &args("bake", "")).unwrap();
    assert!(text(value).contains("storage is unavailable during bake"));
    assert_eq!(std::fs::read_dir(root.0.join("data")).unwrap().count(), 0);
    let mut unconfigured = Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap();
    unconfigured.bind(&plan());
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert!(call(&mut unconfigured, &mut s, "bake", "").contains("unsupported by this host"));
}

#[test]
fn storage_then_fetch_then_storage_preserves_each_answer_context() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let aa = args("fetch", "a");
    let bb = args("fetch", "b");
    let mut pending = vec![
        (aa.clone(), m.answer(&mut s, "work", &aa).unwrap()),
        (bb.clone(), m.answer(&mut s, "work", &bb).unwrap()),
    ];
    let mut done = vec![];
    for _ in 0..100 {
        if pending.is_empty() {
            break;
        }
        let (a, answer) = pending.remove(0);
        match answer {
            Answer::Now(v) => done.push(text(v)),
            Answer::Later(request) => {
                let outcome = if let Some(token) = request.continuation {
                    std::thread::spawn(m.continuation(token).unwrap())
                        .join()
                        .unwrap()
                } else {
                    assert_eq!(
                        request.url,
                        format!("https://example.test/{}", a[1].as_str().unwrap())
                    );
                    response("reply")
                };
                let next = m.parse(&mut s, "work", &a, outcome).unwrap();
                pending.push((a, next));
            }
        }
    }
    done.sort();
    assert_eq!(done, vec!["a:reply", "b:reply"]);
    assert!(matches!(s.get("session"), Some("a" | "b")));
}

/// An app that serializes its storage work through one promise chain starts the
/// second answer's work in a microtask of the first. Each answer must still settle
/// with its own value.
#[test]
fn storage_chained_across_answers_settles_each_answer() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let aa = args("serial", "a");
    let bb = args("serial", "b");
    let mut pending = vec![
        (aa.clone(), m.answer(&mut s, "work", &aa).unwrap()),
        (bb.clone(), m.answer(&mut s, "work", &bb).unwrap()),
    ];
    let mut done = vec![];
    for _ in 0..100 {
        if pending.is_empty() {
            break;
        }
        let (a, answer) = pending.remove(0);
        match answer {
            Answer::Now(v) => done.push(text(v)),
            Answer::Later(request) => {
                let token = request.continuation.expect("storage continuation");
                let outcome = std::thread::spawn(m.continuation(token).unwrap())
                    .join()
                    .unwrap();
                let next = m.parse(&mut s, "work", &a, outcome).unwrap();
                pending.push((a, next));
            }
        }
    }
    done.sort();
    assert_eq!(done, vec!["a:1", "b:2"], "both answers settle, in order");
}

/// The same, through the runner and the host's dispatch: the second resource's
/// work is held while the first's storage turn is open, and released by the
/// commit that ends it, as a native host pumps them.
#[test]
fn runner_holds_an_answer_behind_an_open_storage_turn() {
    let root = Root::new();
    let plan = contract::compile(
        r#"
shape Result
  text: string
component App
  resource first = work("serial", "a") as shape Result else work("placeholder", "")
  resource second = work("serial", "b") as shape Result else work("placeholder", "")
  view
    column
      text first.text testId="first"
      text second.text testId="second"
"#,
    )
    .unwrap();
    let mut m = Module::new(HBC.to_vec(), APP, GRANTS);
    m.set_budget_ms(f64::INFINITY);
    m.configure_storage(
        root.0.join("data"),
        root.0.join("cache"),
        root.0.join("tmp"),
    )
    .unwrap();
    m.bind(&plan);
    m.activate().unwrap();
    let mut runner =
        Runner::boot(plan, m, Kernel::with_monospace(), Default::default(), "/").unwrap();
    let mut held = std::collections::HashMap::new();
    let mut ever_held = false;
    let mut work: Vec<(u64, exact_runner::Work)> = Vec::new();
    for _ in 0..200 {
        for request in runner.take_requests() {
            let token = request.request.continuation.expect("storage continuation");
            match runner.dispatch_work(token) {
                exact_runner::Dispatch::Run(w) => work.push((request.ticket, w)),
                exact_runner::Dispatch::Held => {
                    ever_held = true;
                    held.insert(token, request.ticket);
                }
                _ => panic!("native storage work runs or is held"),
            }
        }
        let Some((ticket, w)) = (if work.is_empty() {
            None
        } else {
            Some(work.remove(0))
        }) else {
            break;
        };
        let exact_runner::Work::Now(w) = w else {
            panic!("native work runs now")
        };
        let outcome = std::thread::spawn(w).join().unwrap();
        runner.fulfill(ticket, outcome).unwrap();
        for (token, dispatch) in runner.release_work() {
            let exact_runner::Dispatch::Run(w) = dispatch else {
                panic!("released work runs")
            };
            let ticket = held.remove(&token).expect("released work was held");
            work.push((ticket, w));
        }
    }
    assert!(ever_held, "the second answer waited for the first's turn");
    assert!(!runner.has_pending(), "both resources settled");
    let read = |id: &str| {
        let key = runner.kernel().find_by_test_id(id)[0];
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text)
            .map(str::to_owned)
    };
    let (a, b) = (read("first").unwrap(), read("second").unwrap());
    let mut both = vec![a, b];
    both.sort();
    assert_eq!(both, vec!["a:1", "b:2"]);
}

#[test]
fn unload_invalidates_continuations_and_configuration_survives_reload() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(
        call(&mut m, &mut s, "file", "before unload"),
        "before unload"
    );
    // Unload cannot undo an already-started external write, so probe cancellation with a read.
    let Answer::Later(r) = m.answer(&mut s, "work", &args("read", "")).unwrap() else {
        panic!("expected asynchronous work")
    };
    let token = r.continuation.unwrap();
    let wait = m.continuation(token).unwrap();
    m.unload();
    assert!(matches!(
        std::thread::spawn(wait).join().unwrap(),
        Outcome::Failed {
            kind: exact_runner::FailureKind::Aborted,
            ..
        }
    ));
    assert!(m.continuation(token).is_none());
    m.activate().unwrap();
    assert_eq!(call(&mut m, &mut s, "read", ""), "before unload");
    assert_eq!(call(&mut m, &mut s, "file", "again"), "again");
}

#[test]
fn runner_settles_storage_actions_from_an_unloaded_first_frame() {
    let root = Root::new();
    let mut runner = Runner::boot(
        plan(),
        root.module(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!runner.data().is_loaded(), "boot cannot run app bytecode");
    assert!(!root.0.exists(), "boot cannot open app storage");
    assert!(!runner.kernel().find_by_test_id("empty").is_empty());
    assert!(runner.take_requests().is_empty());

    // The presenter has produced its first frame; activate and recommit
    // through the same hooks used by the native host.
    runner.data().activate().unwrap();
    runner.data_ready().unwrap();
    assert!(runner.data().is_loaded());

    for (button, expected) in [("file", "from filesystem"), ("sqlite", "from SQLite")] {
        let key = runner.kernel().find_by_test_id(button)[0];
        let view = runner.kernel().node_by_key(key).unwrap().id;
        runner.dispatch(view, Event::Press).unwrap();
        assert!(runner.has_pending());
        assert!(!runner.kernel().find_by_test_id("pending").is_empty());
        let mut steps = 0;
        while runner.has_pending() && steps < 100 {
            let requests = runner.take_requests();
            assert!(
                !requests.is_empty(),
                "a pending storage action needs executor work"
            );
            for request in requests {
                assert_eq!(request.target, "output");
                let token = request
                    .request
                    .continuation
                    .expect("storage uses no synthetic URL");
                assert!(request.request.url.is_empty());
                let work = runner
                    .data()
                    .continuation(token)
                    .expect("owned native work");
                assert!(
                    runner.data().continuation(token).is_none(),
                    "work can only be taken once before settlement"
                );
                let outcome = std::thread::spawn(work).join().unwrap();
                assert!(runner.fulfill(request.ticket, outcome).unwrap().is_some());
                steps += 1;
            }
        }
        assert!(
            !runner.has_pending(),
            "storage must settle within the fixture bound"
        );
        assert!(steps > 0);
        assert!(runner.kernel().find_by_test_id("pending").is_empty());
        let key = runner.kernel().find_by_test_id("result")[0];
        assert_eq!(
            runner
                .kernel()
                .node_by_key(key)
                .unwrap()
                .props
                .str(PropId::Text),
            Some(expected)
        );
        assert_eq!(
            runner.slot("output"),
            Some(&Value::some(Value::record(vec![Value::str(expected)])))
        );
    }
    assert_eq!(
        std::fs::read(root.0.join("data/note")).unwrap(),
        b"from filesystem"
    );
    assert!(root.0.join("data/notes.db").is_file());
}

#[test]
fn storage_resource_refreshes_after_first_pixel_and_keeps_its_last_answer() {
    let source = contract::compile(
        r#"
shape Result
  text: string
component App
  resource library = work("library", "") as shape Result
  view
    text library.text
"#,
    )
    .unwrap();
    let baked = contract::bake(source, Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap()).unwrap();
    let root = Root::new();
    let mut runner = Runner::boot(
        baked.clone(),
        root.module(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(runner.resource_reads_store("library"));
    assert_eq!(text(runner.resource("library").unwrap().clone()), "empty");
    assert!(!runner.data().is_loaded());
    assert!(!root.0.exists());
    assert!(runner.take_requests().is_empty());
    std::fs::create_dir_all(root.0.join("data")).unwrap();
    std::fs::write(root.0.join("data/note"), "saved on this device").unwrap();
    runner.data().activate().unwrap();
    assert!(runner.data_ready().unwrap().is_some());
    for _ in 0..100 {
        if !runner.has_pending() {
            break;
        }
        for request in runner.take_requests() {
            let work = runner
                .data()
                .continuation(request.request.continuation.unwrap())
                .unwrap();
            let outcome = std::thread::spawn(work).join().unwrap();
            runner.fulfill(request.ticket, outcome).unwrap();
        }
    }
    assert!(!runner.has_pending());
    assert_eq!(
        text(runner.resource("library").unwrap().clone()),
        "saved on this device"
    );
    let snapshot = runner.store().snapshot();
    assert!(snapshot
        .iter()
        .any(|(name, _)| name == "exact.kept.library"));
    assert!(!snapshot.iter().any(|(name, _)| name == "session"));
    drop(runner);
    let mut next = Runner::boot_stored(
        baked,
        root.module(),
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!next.data().is_loaded());
    assert!(next.take_requests().is_empty());
    assert_eq!(
        text(next.resource("library").unwrap().clone()),
        "saved on this device"
    );
}

/// An actual TypeScript storage read uses supplied freshness time after a
/// returning frame admits its persisted answer by file identity. This is a
/// seam regression fixture, not an application consumer drive.
#[test]
fn returning_storage_status_keeps_identity_and_revalidates_with_current_context() {
    fn finish_status(runner: &mut Runner<Module>) -> Vec<String> {
        let mut fetched = Vec::new();
        for _ in 0..100 {
            if !runner.has_pending() {
                break;
            }
            let requests = runner.take_requests();
            assert!(!requests.is_empty());
            for request in requests {
                assert_eq!(request.target, "status");
                let outcome = if let Some(token) = request.request.continuation {
                    let work = runner.data().continuation(token).unwrap();
                    std::thread::spawn(work).join().unwrap()
                } else {
                    // The kept answer still stands while freshness revalidation
                    // has reached the network; no provisional value is written.
                    assert_eq!(text(runner.resource("status").unwrap().clone()), "saved A");
                    assert!(runner.take_store_writes().is_empty());
                    fetched.push(request.request.url);
                    response("current A")
                };
                runner.fulfill(request.ticket, outcome).unwrap();
            }
        }
        assert!(!runner.has_pending());
        fetched
    }

    let source = contract::compile(
        r#"
shape Result
  text: string
component App
  state file = "A"
  state minute = 0
  action sample(value: number)
    minute = value
  action choose(value: string)
    file = value
  resource status = work("status", file) with minute as shape Result
  view
    text status.text
"#,
    )
    .unwrap();
    let baked = contract::bake(source, Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap()).unwrap();
    assert_eq!(baked.resources[0].context, 1);
    let root = Root::new();
    std::fs::create_dir_all(root.0.join("data")).unwrap();
    std::fs::write(
        root.0.join("data/status-A"),
        r#"{"text":"saved A","expires":150}"#,
    )
    .unwrap();
    let mut first = Runner::boot(
        baked.clone(),
        root.module(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    first.act("sample", vec![Value::Number(100.)]).unwrap();
    first.data().activate().unwrap();
    first.data_ready().unwrap();
    assert!(
        finish_status(&mut first).is_empty(),
        "the saved status is still fresh at 100"
    );
    assert_eq!(text(first.resource("status").unwrap().clone()), "saved A");
    let snapshot = first.store().snapshot();
    let encoded = snapshot
        .iter()
        .find(|(name, _)| name == "exact.kept.status")
        .unwrap()
        .1
        .clone();
    let identity = Value::list(vec![Value::str("status"), Value::str("A")])
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(encoded.split_once('|').unwrap().0, identity);
    drop(first);

    let mut next = Runner::boot_stored(
        baked.clone(),
        root.module(),
        Kernel::with_monospace(),
        snapshot.clone(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert!(!next.data().is_loaded());
    assert_eq!(text(next.resource("status").unwrap().clone()), "saved A");
    next.act("sample", vec![Value::Number(200.)]).unwrap();
    next.act("sample", vec![Value::Number(300.)]).unwrap();
    assert_eq!(text(next.resource("status").unwrap().clone()), "saved A");
    assert!(next.take_requests().is_empty());
    assert!(next.take_store_writes().is_empty());
    next.data().activate().unwrap();
    next.data_ready().unwrap();
    assert_eq!(text(next.resource("status").unwrap().clone()), "saved A");
    assert_eq!(
        finish_status(&mut next),
        ["https://example.test/status/A?minute=300"]
    );
    assert_eq!(text(next.resource("status").unwrap().clone()), "current A");
    let writes = next.take_store_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].name, "exact.kept.status");
    assert!(writes[0].value.is_some());

    let mut other = Runner::boot_stored(
        baked,
        root.module(),
        Kernel::with_monospace(),
        snapshot,
        Default::default(),
        "/",
    )
    .unwrap();
    other.act("choose", vec![Value::str("B")]).unwrap();
    assert_eq!(text(other.resource("status").unwrap().clone()), "empty");
    other.act("choose", vec![Value::str("A")]).unwrap();
    assert_eq!(text(other.resource("status").unwrap().clone()), "empty");
    assert!(!other.data().is_loaded());
}
