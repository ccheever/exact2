//! `native.later` (the module's long calls): through the data seam with the
//! storage fixture, whose module may attach a native implementation.
#![cfg(exact_js_engine)]
use super::storage::{args, finish, Root, GRANTS};
use exact_runner::{Answer, DataSource, Store};

/// `native.later`: the answer leaves as a native request, the host hands its
/// body to the module's handler, the module's own thread replies, and the
/// answer resumes with the value — or with the module's refusal.
#[test]
fn native_later_answers_from_the_modules_own_thread() {
    use std::sync::{mpsc, Mutex};
    /// Opened by the test once the handler has returned: the work waits on
    /// it, so a handler that ran the work before returning never sees it open.
    static GATE: Mutex<Option<mpsc::Receiver<()>>> = Mutex::new(None);
    struct Native;
    impl exact_js::NativeModule for Native {
        fn configure_storage(&mut self, _: exact_js::NativeStorage) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"text": format!("now:{}", request["value"].as_str().unwrap())}))
        }
        fn later(&mut self) -> Option<exact_js::LaterHandler> {
            Some(std::sync::Arc::new(|request, reply| {
                let value = request["value"].as_str().unwrap_or("").to_string();
                std::thread::spawn(move || {
                    let gate = GATE.lock().unwrap().take().expect("the test set a gate");
                    let opened = gate.recv_timeout(std::time::Duration::from_secs(30));
                    reply.send(if opened.is_err() {
                        Err("the handler returned only after its work".into())
                    } else if value == "bad" {
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
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
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
        let (tx, rx) = mpsc::channel();
        let (open, gate) = mpsc::channel();
        *GATE.lock().unwrap() = Some(gate);
        // Returns before its work runs: the work is on the module's thread.
        handler(
            request.body.clone(),
            exact_runner::Reply::new(move |outcome| tx.send(outcome).unwrap()),
        );
        open.send(()).unwrap();
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
        fn configure_storage(&mut self, _: exact_js::NativeStorage) -> Result<(), String> {
            Ok(())
        }
        fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"text": format!("now:{}", request["value"].as_str().unwrap())}))
        }
    }
    let root = Root::new();
    let mut m = root.module().with_native(|_| Box::new(Native));
    let mut s = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    m.activate().unwrap();
    let a = args("later", "quick");
    let answer = m.answer(&mut s, "work", &a).unwrap();
    assert!(
        matches!(answer, Answer::Now(_)),
        "it answered later: {answer:?}"
    );
    assert_eq!(finish(&mut m, &mut s, &a, answer), "now:quick");
}
