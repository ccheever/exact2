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
const GRANTS: &str = "fs.read app:/data\nfs.write app:/data\nfs.read doc:/\nfs.write doc:/\nsqlite.open app:/data/notes.db\nnet.fetch https://example.test\nsecret.keep session\n";
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
    assert_eq!(
        text(value),
        "Unavailable:bake:storage is unavailable during bake"
    );
    assert_eq!(std::fs::read_dir(root.0.join("data")).unwrap().count(), 0);
    let mut unconfigured = Module::loaded(HBC.to_vec(), APP, GRANTS).unwrap();
    unconfigured.bind(&plan());
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(
        call(&mut unconfigured, &mut s, "bake", ""),
        "Unavailable:unsupported:storage is unsupported by this host"
    );
    // A drive that names no scratch store: the web's refusal, word for word.
    let mut agent = Module::new(HBC.to_vec(), APP, GRANTS).with_agent_seed(Some(1));
    agent.bind(&plan());
    agent.activate().unwrap();
    assert_eq!(
        call(&mut agent, &mut s, "bake", ""),
        "Unavailable:agent:storage is unavailable in agent mode unless the drive names a scratch store (--storage <name>)"
    );
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

/// Storage an answer starts and does not await still lands (kanban F22): a
/// write made before a value returned at once, and one queued behind the
/// module's storage chain and begun after the answer's own promise resolved.
/// The browser runs both to their end; so does Hermes, before the reply.
#[test]
fn storage_an_answer_does_not_await_still_lands() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "unawaited", "first"), "answered");
    assert_eq!(call(&mut m, &mut s, "read-at", "unawaited"), "first");
    assert_eq!(call(&mut m, &mut s, "queued", "second"), "answered");
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/queued/file")).unwrap(),
        "second"
    );
    assert_eq!(call(&mut m, &mut s, "read-at", "queued/file"), "second");
    assert_eq!(call(&mut m, &mut s, "queued-sync", "third"), "answered");
    assert_eq!(call(&mut m, &mut s, "read-at", "queued/file"), "third");
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

/// Ledger's pattern on a native host (ledger F12): every answer queued on one
/// promise chain, a mutation that `refreshes` a storage-backed listing (asked
/// when sent and again when it lands), a save that refuses bad input before
/// touching storage, and filter changes that replace a listing between its
/// storage steps, beside an independent read replaced mid-read by new
/// arguments or a refresh (minesweeper F10). Driven as the Apple bridge drives it: let-go work still
/// runs and its reply is dropped; held work runs when released.
#[test]
fn queued_answers_survive_refreshes_refusals_and_replaced_turns() {
    let root = Root::new();
    let plan = contract::compile(
        r#"
shape Result
  text: string
component App
  state q = "a"
  mutation saved as shape Result refreshes listing
  resource listing = work("count", q) as shape Result else work("placeholder", "")
  resource peeked = work("peek", q) as shape Result else work("placeholder", "")
  action save(op: string, value: string)
    send saved = work(op, value)
  action search(v: string)
    q = v
  action reload
    refresh peeked
  view
    column
      text listing.text testId="listing"
      text peeked.text testId="peeked"
      match saved
        case some(r)
          text r.text testId="saved"
        case none
          text "none" testId="saved"
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
    let mut work: std::collections::VecDeque<(u64, exact_runner::Work)> = Default::default();
    // What the bridge does after every commit: dispatch what was asked,
    // then run what a source released.
    fn emit(
        runner: &mut Runner<Module>,
        held: &mut std::collections::HashMap<u64, u64>,
        work: &mut std::collections::VecDeque<(u64, exact_runner::Work)>,
    ) {
        for request in runner.take_requests() {
            let token = request.request.continuation.expect("storage continuation");
            match runner.dispatch_work(token) {
                exact_runner::Dispatch::Run(w) => work.push_back((request.ticket, w)),
                exact_runner::Dispatch::Held => {
                    held.insert(token, request.ticket);
                }
                _ => panic!("native storage work runs or is held"),
            }
        }
        for (token, dispatch) in runner.release_work() {
            let exact_runner::Dispatch::Run(w) = dispatch else {
                panic!("released work runs")
            };
            if let Some(ticket) = held.remove(&token) {
                work.push_back((ticket, w));
            }
        }
    }
    // One completion, or every one when `all`.
    let step = |runner: &mut Runner<Module>,
                held: &mut std::collections::HashMap<u64, u64>,
                work: &mut std::collections::VecDeque<(u64, exact_runner::Work)>|
     -> bool {
        let Some((ticket, w)) = work.pop_front() else {
            return false;
        };
        let exact_runner::Work::Now(w) = w else {
            panic!("native work runs now")
        };
        let outcome = std::thread::spawn(w).join().unwrap();
        runner.fulfill(ticket, outcome).unwrap();
        emit(runner, held, work);
        true
    };
    let settle =
        |runner: &mut Runner<Module>,
         held: &mut std::collections::HashMap<u64, u64>,
         work: &mut std::collections::VecDeque<(u64, exact_runner::Work)>| {
            emit(runner, held, work);
            for _ in 0..400 {
                if !step(runner, held, work) {
                    break;
                }
            }
        };
    let read = |runner: &Runner<Module>, id: &str| {
        let key = runner.kernel().find_by_test_id(id)[0];
        runner
            .kernel()
            .node_by_key(key)
            .unwrap()
            .props
            .str(PropId::Text)
            .unwrap_or_default()
            .to_owned()
    };
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "listing"), "a:0");

    // A refused save, then a good one.
    runner
        .act("save", vec![Value::str("invalid"), Value::str("")])
        .unwrap();
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "saved"), "invalid");
    runner
        .act("save", vec![Value::str("serial"), Value::str("x")])
        .unwrap();
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "saved"), "x:1");
    assert_eq!(read(&runner, "listing"), "a:1");

    // A filter changed twice before anything lands, and once more while the
    // listing it replaced is between storage steps.
    runner.act("search", vec![Value::str("b")]).unwrap();
    emit(&mut runner, &mut held, &mut work);
    runner.act("search", vec![Value::str("c")]).unwrap();
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "listing"), "c:1");
    runner.act("search", vec![Value::str("d")]).unwrap();
    emit(&mut runner, &mut held, &mut work);
    step(&mut runner, &mut held, &mut work);
    runner.act("search", vec![Value::str("")]).unwrap();
    runner
        .act("save", vec![Value::str("serial"), Value::str("y")])
        .unwrap();
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "saved"), "y:2");
    assert_eq!(read(&runner, "listing"), ":2");
    // An independent read replaced mid-read, by new arguments or by a
    // refresh with the same ones, leaves later reads working (minesweeper F10).
    runner
        .act("save", vec![Value::str("file"), Value::str("peeked")])
        .unwrap();
    settle(&mut runner, &mut held, &mut work);
    for q in ["e", "f", "g"] {
        runner.act("search", vec![Value::str(q)]).unwrap();
        emit(&mut runner, &mut held, &mut work);
        step(&mut runner, &mut held, &mut work);
    }
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "peeked"), "g:peeked");
    assert_eq!(read(&runner, "listing"), "g:2");
    runner.act("search", vec![Value::str("h")]).unwrap();
    emit(&mut runner, &mut held, &mut work);
    runner.act("reload", vec![]).unwrap();
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(read(&runner, "peeked"), "h:peeked");
    assert_eq!(read(&runner, "listing"), "h:2");
    // Both mutations are sent while a listing owns a storage turn. Replacing
    // the first reply must not cancel the insert before its first JS step.
    runner.act("search", vec![Value::str("i")]).unwrap();
    emit(&mut runner, &mut held, &mut work);
    for value in ["first-queued", "second-queued"] {
        runner
            .act("save", vec![Value::str("serial"), Value::str(value)])
            .unwrap();
        emit(&mut runner, &mut held, &mut work);
    }
    settle(&mut runner, &mut held, &mut work);
    assert_eq!(
        read(&runner, "saved"),
        "second-queued:4",
        "{:?}",
        runner.journal().collect::<Vec<_>>()
    );
    assert_eq!(read(&runner, "listing"), "i:4");

    assert!(!runner.has_pending(), "everything settled");
    let refusals: Vec<&str> = runner
        .journal()
        .filter(|l| l.contains("pending on nothing") || l.contains("failed"))
        .collect();
    assert!(refusals.is_empty(), "{refusals:#?}");
}

/// A re-read the runner dropped before handing it out (files diary F18: a
/// mutation refreshing a folder's preview mid-walk) is forgotten here too: a
/// deferred call left parked under the walk's key took the walk's next
/// storage step, so the walk's turn never ended and every answer behind it
/// waited forever. A composer above (the mixed source) cannot name the
/// walk's dispatched token to `forgotten`, so only `discard` drops it.
#[test]
fn a_discarded_re_read_leaves_the_read_in_flight_its_steps() {
    use exact_runner::Target;
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let target = Target::Resource(0);
    let a = args("walk", "x");
    let mut answer = m.answer_for(target, &mut s, "work", &a).unwrap();
    // The re-read arrives while the walk is between storage steps: deferred
    // behind its turn, then dropped by the runner.
    let Answer::Later(again) = m.answer_for(target, &mut s, "work", &a).unwrap() else {
        panic!("a re-read behind an open turn waits")
    };
    m.discard(again.continuation.expect("a deferred continuation"));
    for _ in 0..100 {
        match answer {
            Answer::Now(v) => {
                assert_eq!(text(v), "x:24", "the walk ends with its own steps");
                assert_eq!(m.in_flight(), 0, "nothing is left parked");
                return;
            }
            Answer::Later(request) => {
                let token = request.continuation.expect("storage continuation");
                let work = m.continuation(token).expect("the walk's own step");
                let outcome = std::thread::spawn(work).join().unwrap();
                answer = m.parse_for(target, &mut s, "work", &a, outcome).unwrap();
            }
        }
    }
    panic!("the walk did not settle")
}

/// Each refusal names its reason as a code the web's adapters use too
/// (kanban F28); the message beside it may differ by host.
#[test]
fn storage_refusals_carry_a_stable_code() {
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    assert_eq!(call(&mut m, &mut s, "file", "x"), "x");
    let codes: Vec<String> = call(&mut m, &mut s, "codes", "")
        .lines()
        .map(|l| l.split(' ').take(2).collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(
        codes,
        [
            "Unavailable ENOENT",
            "Unavailable denied",
            "Unavailable ENOTDIR",
            "ok",
            "Unavailable denied"
        ]
    );
}

/// A folder the person chose is reached from TypeScript as from Rust (LLP
/// 1069.010 D1, files F2): the same `doc:` path, the same grants, the same
/// executor, and with no app storage at all (a drive with no scratch store),
/// where an `app:/` path is still refused as `agent`.
#[test]
fn a_document_the_person_chose_is_storage_without_app_directories() {
    let dir = Root::new();
    let folder = dir.0.join("chosen");
    std::fs::create_dir_all(folder.join("sub")).unwrap();
    std::fs::write(folder.join("a.txt"), "A").unwrap();
    std::fs::write(folder.join("sub/b.txt"), "B").unwrap();
    let doc = exact_data::documents::mint(&folder, 9101).unwrap();
    let mut s = Store::new(GRANTS, Vec::<(String, String)>::new());
    let expected = "a.txt=A sub/0 new=hi Unavailable ENOENT Unavailable denied Unavailable EISDIR Unavailable ENOTDIR Unavailable ENOTEMPTY Unavailable failed Unavailable failed Unavailable failed";
    let mut agent = Module::new(HBC.to_vec(), APP, GRANTS).with_agent_seed(Some(1));
    agent.set_budget_ms(f64::INFINITY);
    agent.bind(&plan());
    agent.activate().unwrap();
    assert_eq!(
        call(&mut agent, &mut s, "doc", &doc),
        format!("{expected} Unavailable agent")
    );
    let mut m = dir.module();
    m.activate().unwrap();
    assert_eq!(
        call(&mut m, &mut s, "doc", &doc),
        format!("{expected} Unavailable ENOENT")
    );
    assert!(!folder.join("new.txt").exists());
    exact_data::documents::forget(9101);
}

/// No later UI request is needed to finish retired writes. The oldest turn
/// and both superseded mutations are gone; external writes survive in order,
/// their discarded Store writes do not, and an uncommitted send never runs.
#[test]
fn retired_deferred_writes_finish_without_another_answer() {
    use exact_runner::Target;
    let root = Root::new();
    let mut m = root.module();
    m.activate().unwrap();
    let mut store = Store::new(GRANTS, [("session".into(), "original".into())]);
    assert!(matches!(
        m.answer_for(
            Target::Resource(0),
            &mut store,
            "work",
            &args("count", "open")
        )
        .unwrap(),
        Answer::Later(_)
    ));
    for value in ["first", "second"] {
        assert!(matches!(
            m.answer_for(
                Target::Mutation(0),
                &mut store,
                "work",
                &args("file-kept", value)
            )
            .unwrap(),
            Answer::Later(_)
        ));
    }
    let Answer::Later(discarded) = m
        .answer_for(
            Target::Mutation(1),
            &mut store,
            "work",
            &args("file", "uncommitted"),
        )
        .unwrap()
    else {
        panic!("queued")
    };
    m.discard(discarded.continuation.unwrap());
    m.forgotten(&store, &[]);
    assert_eq!(m.in_flight(), 0);
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/note")).unwrap(),
        "second"
    );
    assert_eq!(store.get("session"), Some("original"));
    assert!(m.take_logs().is_empty());
    // A later live mutation still commits its Store write normally.
    assert_eq!(call(&mut m, &mut store, "file-kept", "live"), "live");
    assert_eq!(store.get("session"), Some("live"));
}

#[test]
fn retired_writes_keep_submission_order_beside_live_targets_and_do_not_cross_unload() {
    use exact_runner::{InFlight, Target};
    fn complete(m: &mut Module, s: &mut Store, target: Target, a: &[Value], mut answer: Answer) {
        for _ in 0..100 {
            let Answer::Later(request) = answer else {
                return;
            };
            let outcome = m.continuation(request.continuation.unwrap()).unwrap()();
            answer = m.parse_for(target, s, "work", a, outcome).unwrap();
        }
        panic!("did not finish");
    }
    for unload in [false, true] {
        let root = Root::new();
        let mut m = root.module();
        m.activate().unwrap();
        let mut s = Store::new(GRANTS, [("session".into(), "original".into())]);
        let aa = args("ordered", "A");
        let bb = args("ordered", "B");
        let cc = args("ordered", "C");
        let owner_args = args("file", "owner");
        let owner = m
            .answer_for(Target::Resource(0), &mut s, "work", &owner_args)
            .unwrap();
        let older = m
            .answer_for(Target::Mutation(0), &mut s, "work", &aa)
            .unwrap();
        let _ = m
            .answer_for(Target::Mutation(1), &mut s, "work", &bb)
            .unwrap();
        let newer = m
            .answer_for(Target::Mutation(1), &mut s, "work", &cc)
            .unwrap();
        let token = |a: &Answer| match a {
            Answer::Later(r) => r.continuation,
            _ => None,
        };
        m.forgotten(
            &s,
            &[
                InFlight {
                    target: Target::Resource(0),
                    source: "work",
                    args: &owner_args,
                    continuation: token(&owner),
                },
                InFlight {
                    target: Target::Mutation(0),
                    source: "work",
                    args: &aa,
                    continuation: token(&older),
                },
                InFlight {
                    target: Target::Mutation(1),
                    source: "work",
                    args: &cc,
                    continuation: token(&newer),
                },
            ],
        );
        assert_eq!(m.in_flight(), 4, "three live calls and a retired write");
        if unload {
            m.unload();
            assert_eq!(m.in_flight(), 0);
            m.activate().unwrap();
            call(&mut m, &mut s, "file", "after reload");
            assert!(!root.0.join("data/ordered").exists());
        } else {
            complete(&mut m, &mut s, Target::Resource(0), &owner_args, owner);
            complete(&mut m, &mut s, Target::Mutation(0), &aa, older);
            assert_eq!(s.get("session"), Some("A"), "a live write commits normally");
            complete(&mut m, &mut s, Target::Mutation(1), &cc, newer);
            assert_eq!(
                std::fs::read_to_string(root.0.join("data/ordered")).unwrap(),
                "ABC"
            );
            assert_eq!(s.get("session"), Some("C"));
        }
    }
}
