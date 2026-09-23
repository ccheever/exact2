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
  action invoke(op: string, value: string) writes output
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
    assert!(call(&mut m, &mut s, "native", "validation").contains("unavailable during bake"));
    assert_eq!(
        std::fs::read_to_string(root.0.join("data/native.txt")).unwrap(),
        "after fetch"
    );
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
